//! 桌面宿主到 OmniDesk Engine 的连接层（Step 9）。
//!
//! 调用链：React UI →（IPC-1 Tauri 命令）→ 本模块 →（IPC-2 JSON-RPC）→ Python Engine
//!        →（ACP）→ DSH CLI。
//!
//! 关键约束：
//!   * 桌面**只**依赖引擎归一化接口，不直接依赖 DSH ACP 协议细节；
//!   * 能力表与版本号一律取自引擎 omni.initialize 的真实返回，不本地拍脑袋；
//!   * 事件泵把 omni.event 通知转发给 UI，并在断流时明确声明，绝不伪造成成功；
//!   * 所有时间戳统一为 epoch 秒（浮点），与引擎契约一致；
//!   * 不读取、不打印、不落盘任何凭证。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::engine_ipc::EngineRpcError;
use crate::engine_supervisor::EngineSupervisor;

/// 事件泵把归一化事件交给 UI 的方式：宿主注入一个 push 回调。
pub type EventSink = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// 引擎任务事件通知的方法名（IPC-2 契约 §3.3）。
const EVENT_METHOD: &str = "omni.event";

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// 把引擎返回的错误归一化成 UI 可见的结构（含五属性）。
fn error_to_json(err: &EngineRpcError) -> serde_json::Value {
    let mut detail = serde_json::Map::new();
    for key in [
        "retryable",
        "needs_human",
        "degradable",
        "may_duplicate_execution",
        "may_lose_output",
    ] {
        if let Some(value) = err.data.get(key) {
            detail.insert(key.to_string(), value.clone());
        }
    }
    let inner_detail = err
        .data
        .get("detail")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "engine_error_code": err.engine_error_code(),
        "message": err.message,
        "omni_task_id": err.task_id(),
        "retryable": err.attribute("retryable"),
        "needs_human": err.attribute("needs_human"),
        "degradable": err.attribute("degradable"),
        "may_duplicate_execution": err.attribute("may_duplicate_execution"),
        "may_lose_output": err.attribute("may_lose_output"),
        "detail": { "attributes": detail, "engine_detail": inner_detail },
    })
}

pub struct EngineBridge {
    supervisor: Arc<Mutex<EngineSupervisor>>,
    pump_running: Arc<AtomicBool>,
    initialized: bool,
    /// omni.initialize 的真实返回（引擎描述符），健康快照据此报告，不虚报。
    descriptor: Option<serde_json::Value>,
}

impl EngineBridge {
    pub fn new(supervisor: Arc<Mutex<EngineSupervisor>>) -> Self {
        Self {
            supervisor,
            pump_running: Arc::new(AtomicBool::new(false)),
            initialized: false,
            descriptor: None,
        }
    }

    pub fn supervisor(&self) -> Arc<Mutex<EngineSupervisor>> {
        Arc::clone(&self.supervisor)
    }

    /// 启动子进程并完成 omni.initialize（幂等）。返回引擎描述符。
    pub fn ensure_initialized(
        &mut self,
        sink: &EventSink,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut guard = self.supervisor.lock().unwrap();
        if !guard.is_running() {
            guard.start().map_err(|e| {
                serde_json::json!({
                    "engine_error_code": "engine_unavailable",
                    "message": e,
                    "detail": {},
                })
            })?;
            // 进程重启后旧描述符失效，必须重新初始化。
            self.initialized = false;
            self.descriptor = None;
        }
        if !self.initialized {
            let init = guard
                .initialize(Duration::from_secs(90))
                .map_err(|e| error_to_json(&e))?;
            // omni.initialize 返回 {protocol_version, server, engine:{...}}
            let descriptor = init
                .get("engine")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            self.descriptor = Some(descriptor.clone());
            self.initialized = true;
            self.start_pump(sink);
            return Ok(descriptor);
        }
        Ok(self
            .descriptor
            .clone()
            .unwrap_or(serde_json::Value::Null))
    }

    fn start_pump(&self, sink: &EventSink) {
        if self.pump_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let supervisor = Arc::clone(&self.supervisor);
        let running = Arc::clone(&self.pump_running);
        let sink = Arc::clone(sink);
        thread::Builder::new()
            .name("omnidesk-event-pump".to_string())
            .spawn(move || loop {
                let frame = {
                    let mut guard = supervisor.lock().unwrap();
                    match guard.with_channel(|channel| {
                        Ok(channel.recv_notification_timeout(Duration::from_millis(500)))
                    }) {
                        Ok(frame) => frame,
                        Err(_) => {
                            // 通道不可用（引擎未启动/已退出）。
                            if !guard.is_running() {
                                sink(serde_json::json!({
                                    "omni_task_id": "unknown",
                                    "seq": -1,
                                    "occurred_at": now_secs(),
                                    "type": "protocol_error",
                                    "payload": {
                                        "engine_error_code": "engine_unavailable",
                                        "message": "引擎事件流已中断（子进程退出）",
                                    },
                                }));
                                running.store(false, Ordering::SeqCst);
                                return;
                            }
                            None
                        }
                    }
                };

                let Some(frame) = frame else {
                    // 超时无事件属正常空转，继续等待。
                    continue;
                };

                let method = frame.get("method").and_then(|v| v.as_str()).unwrap_or("");
                if method == EVENT_METHOD {
                    if let Some(params) = frame.get("params") {
                        sink(params.clone());
                    }
                } else {
                    // 未知通知：包装成 protocol_error 交给 UI，不静默丢弃。
                    sink(serde_json::json!({
                        "omni_task_id": "unknown",
                        "seq": -1,
                        "occurred_at": now_secs(),
                        "type": "protocol_error",
                        "payload": {
                            "engine_error_code": "event_parse_error",
                            "message": format!("收到未识别的方法：{}", method),
                        },
                    }));
                }
            })
            .expect("spawn event pump");
    }

    pub fn submit_task(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc("omni.submit_task", params, Duration::from_secs(60))
    }

    pub fn get_task_status(
        &mut self,
        omni_task_id: &str,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.get_task_status",
            serde_json::json!({ "omni_task_id": omni_task_id }),
            Duration::from_secs(30),
        )
    }

    pub fn get_task_result(
        &mut self,
        omni_task_id: &str,
        require_persisted: bool,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.get_task_result",
            serde_json::json!({
                "omni_task_id": omni_task_id,
                "require_persisted": require_persisted,
            }),
            Duration::from_secs(60),
        )
    }

    pub fn cancel_task(
        &mut self,
        omni_task_id: &str,
        timeout_seconds: f64,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.cancel_task",
            serde_json::json!({
                "omni_task_id": omni_task_id,
                "timeout_seconds": timeout_seconds,
            }),
            Duration::from_secs((timeout_seconds.ceil() as u64) + 10),
        )
    }

    pub fn stream_events(
        &mut self,
        omni_task_id: &str,
        after_seq: Option<i64>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut params = serde_json::json!({ "omni_task_id": omni_task_id });
        if let Some(seq) = after_seq {
            params["after_seq"] = serde_json::json!(seq);
        }
        self.rpc("omni.stream_events", params, Duration::from_secs(30))
    }

    fn rpc(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut guard = self.supervisor.lock().unwrap();
        if !self.initialized {
            return Err(serde_json::json!({
                "engine_error_code": "engine_unavailable",
                "message": "引擎尚未初始化，请先调用 engine_describe",
                "detail": {},
            }));
        }
        guard
            .with_channel(|channel| channel.request(method, params, timeout))
            .map_err(|e| error_to_json(&e))
    }

    /// 关闭引擎。
    pub fn shutdown(&mut self, grace: Duration) -> Result<serde_json::Value, String> {
        let mut guard = self.supervisor.lock().unwrap();
        let (forced, exit_code) = guard.stop(grace)?;
        self.initialized = false;
        self.descriptor = None;
        self.pump_running.store(false, Ordering::SeqCst);
        Ok(serde_json::json!({
            "forced_terminate": forced,
            "process_exit_code": exit_code,
            "note": if forced { "超出优雅等待，已强制终止" } else { "引擎已优雅退出" },
        }))
    }

    /// 健康快照（不发起任务、不读凭证）。
    pub fn health(&mut self, app_root: &str) -> serde_json::Value {
        let mut guard = self.supervisor.lock().unwrap();
        let running = guard.is_running();
        let pid = guard.pid();
        let config = guard.config().clone();
        let stderr_tail = guard.stderr_tail();
        let descriptor = self.descriptor.clone();
        let get = |key: &str| -> Option<String> {
            descriptor
                .as_ref()
                .and_then(|d| d.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        serde_json::json!({
            "running": running,
            "engine_version": get("engine_version"),
            "protocol_version": get("protocol_version"),
            "compatibility": get("compatibility"),
            "app_data_root": app_root,
            "dsh_home": config.dsh_home.display().to_string(),
            "dsh_agents_home": config.agents_home.display().to_string(),
            "engine_pid": pid,
            "note": if running {
                if self.initialized { "引擎已初始化并在运行" } else { "引擎子进程在运行（未初始化）" }
            } else {
                "引擎未启动"
            },
            "stderr_tail": stderr_tail,
        })
    }
}
