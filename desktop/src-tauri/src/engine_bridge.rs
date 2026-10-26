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
use std::sync::mpsc::Receiver;
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
    /// 仅测试：取得 supervisor 锁之前的同步点（OD-ARCH-024 的确定性时序测试）。
    #[cfg(test)]
    pre_lock_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl EngineBridge {
    pub fn new(supervisor: Arc<Mutex<EngineSupervisor>>) -> Self {
        Self {
            supervisor,
            pump_running: Arc::new(AtomicBool::new(false)),
            initialized: false,
            descriptor: None,
            #[cfg(test)]
            pre_lock_hook: None,
        }
    }

    pub fn supervisor(&self) -> Arc<Mutex<EngineSupervisor>> {
        Arc::clone(&self.supervisor)
    }

    /// 启动子进程并完成 omni.initialize（幂等）。返回引擎描述符。
    ///
    /// 必须传入宿主签发的准入凭据：未通过端点与授权判定时，上层拿不到凭据，
    /// 也就无法初始化引擎。
    pub fn ensure_initialized(
        &mut self,
        sink: &EventSink,
        grant: &crate::admission::AdmissionGrant,
    ) -> Result<serde_json::Value, serde_json::Value> {
        // OD-ARCH-023：凭据必须绑定“当前权限世代”。未盖章的凭据（在引擎调用守卫
        // 之外取得的准入结果）不得用于启动引擎。
        if grant.epoch() == crate::admission::GRANT_UNSTAMPED_EPOCH {
            return Err(serde_json::json!({
                "engine_error_code": "grant_not_stamped",
                "message": "已拒绝启动引擎：准入凭据未绑定当前权限世代。未创建子进程、未提交任务。",
                "detail": { "reason": "grant_not_stamped", "enforced_by": "host" },
            }));
        }
        let mut guard = self.supervisor.lock().unwrap();
        if !guard.is_running() {
            guard.start(grant).map_err(|e| {
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
            // 调用方此时**已持有** supervisor 锁，因此把句柄就地取出后传进去。
            // （泵不可自行加锁：本互斥锁不可重入。）
            let handle = guard.notification_handle();
            self.start_pump(sink, handle);
            return Ok(descriptor);
        }
        Ok(self
            .descriptor
            .clone()
            .unwrap_or(serde_json::Value::Null))
    }

    /// 启动事件泵。
    ///
    /// **关键（锁饥饿修复）**：泵只借用一次 supervisor 锁，换取一个独立的通知句柄；
    /// 此后它**不再触碰 supervisor 互斥锁**。旧实现对每一次 500ms 等待都持锁，
    /// 超时后立即重新加锁，形成紧循环，把 GTK 主线程（`engine_health`）永久饿死，
    /// 表现为窗口整体「无响应」。
    fn start_pump(
        &self,
        sink: &EventSink,
        handle: Option<(Arc<Mutex<Receiver<serde_json::Value>>>, Arc<AtomicBool>)>,
    ) {
        if self.pump_running.swap(true, Ordering::SeqCst) {
            return;
        }
        // 取不到句柄说明引擎尚未启动，本轮不起泵。
        let Some((notif_rx, exited)) = handle else {
            self.pump_running.store(false, Ordering::SeqCst);
            return;
        };

        let running = Arc::clone(&self.pump_running);
        let sink = Arc::clone(sink);
        thread::Builder::new()
            .name("omnidesk-event-pump".to_string())
            .spawn(move || loop {
                // 只锁通知接收端本身（不涉及 supervisor），超时即释放。
                let frame = notif_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_millis(500))
                    .ok();

                let Some(frame) = frame else {
                    // 超时无事件属正常空转；子进程退出则明确声明事件流中断。
                    if exited.load(Ordering::SeqCst) {
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

    /// 回传用户对一次审批请求的决定（OD-ARCH-066）。
    pub fn approval_respond(
        &mut self,
        omni_task_id: &str,
        approval_id: &str,
        allow: bool,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.approval_respond",
            serde_json::json!({
                "omni_task_id": omni_task_id,
                "approval_id": approval_id,
                "allow": allow,
            }),
            Duration::from_secs(30),
        )
    }

    /// 回传用户对结构化一次性提问的答案（OD-ARCH-088）。
    /// 答案只走本次 IPC；宿主不落盘，也不写日志。
    pub fn question_answer(
        &mut self,
        omni_task_id: &str,
        question_id: &str,
        answers: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.question_answer",
            serde_json::json!({
                "omni_task_id": omni_task_id,
                "question_id": question_id,
                "answers": answers,
            }),
            Duration::from_secs(30),
        )
    }

    /// 列出引擎已持久化的会话元数据（不返回正文）。
    pub fn list_sessions(
        &mut self,
        cwd: Option<&str>,
        after_cursor: Option<&str>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut params = serde_json::json!({});
        if let Some(value) = cwd {
            params["cwd"] = serde_json::json!(value);
        }
        if let Some(value) = after_cursor {
            params["after_cursor"] = serde_json::json!(value);
        }
        self.rpc("omni.list_sessions", params, Duration::from_secs(30))
    }

    /// 重开一个既有会话（resume）。不代表任务续跑。
    pub fn recover_session(
        &mut self,
        engine_session_id: &str,
        workspace_token: Option<&str>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut params = serde_json::json!({ "engine_session_id": engine_session_id });
        if let Some(value) = workspace_token {
            params["workspace_token"] = serde_json::json!(value);
        }
        self.rpc("omni.recover_session", params, Duration::from_secs(60))
    }

    /// 会话分支（DSH 原生 session/fork）：按精确切点拿到一条子会话。
    /// 失败原样上抛，不在宿主侧退回本地重放。
    pub fn fork_session(
        &mut self,
        engine_session_id: &str,
        at_seq: Option<u64>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let mut params = serde_json::json!({ "engine_session_id": engine_session_id });
        if let Some(seq) = at_seq {
            params["at_seq"] = serde_json::json!(seq);
        }
        self.rpc("omni.fork_session", params, Duration::from_secs(60))
    }

    /// 列出 OmniDesk 自己落盘的任务台账元数据。
    pub fn list_task_records(&mut self, limit: u32) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.list_task_records",
            serde_json::json!({ "limit": limit }),
            Duration::from_secs(30),
        )
    }

    /// 重启后恢复可证实状态；无终态证据的任务返回 unknown，不自动重放。
    pub fn recover_task(&mut self, omni_task_id: &str) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.recover_task",
            serde_json::json!({ "omni_task_id": omni_task_id }),
            Duration::from_secs(60),
        )
    }

    /// 模型目录（OD-ARCH-048）：只有 web-api 通道能从 DSH `session/modelCatalog` 直读；
    /// ACP 通道由引擎返回 `unsupported`，上层据此回退到 Profile 候选列表。
    pub fn list_models(&mut self, refresh: bool) -> Result<serde_json::Value, serde_json::Value> {
        self.rpc(
            "omni.list_models",
            serde_json::json!({ "refresh": refresh }),
            Duration::from_secs(60),
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
        // 仅测试用同步点：在**取得 supervisor 锁之前**执行，用于确定性地复现
        // “命令已进入发送路径、尚未持锁，关闭先拿到锁”的时序（OD-ARCH-024）。
        #[cfg(test)]
        {
            let hook = self.pre_lock_hook.clone();
            if let Some(hook) = hook {
                hook();
            }
        }
        let mut guard = self.supervisor.lock().unwrap();
        // 关闭中优先于“未初始化”：两段式关闭会先摘除通道、后重置 initialized，
        // 期间必须稳定地返回“关闭中、未发送”，而不是等关闭结束后才报普通错误。
        if guard.is_stopping() {
            return Err(error_to_json(&EngineRpcError::shutting_down(
                "引擎正在关闭，本次请求未发送。请等待关闭完成后重试。",
            )));
        }
        if !self.initialized {
            return Err(serde_json::json!({
                "engine_error_code": "engine_unavailable",
                "message": "引擎未运行或已停止（可能正被关闭），本次请求未发送。请先重新初始化引擎后再试。",
                "detail": {},
            }));
        }
        guard
            .with_channel(|channel| channel.request(method, params, timeout))
            .map_err(|e| error_to_json(&e))
    }

    /// 关闭引擎（两段式，OD-ARCH-024）。
    ///
    /// 摘除通道在 supervisor 锁内完成，耗时的协议关闭与进程收尾在**锁外**执行。
    /// 因此关闭期间任何状态/结果/取消命令都能立刻拿锁并得到明确结论，不会被
    /// 排队到关闭结束。
    ///
    /// **调用方不得在持有宿主锁时等待本方法**：虽然本方法自身不长时间持锁，但它的
    /// 等待会一直占用调用方手里的宿主锁，从而把并发的状态/结果/取消命令堵住。
    /// 宿主生产路径请统一用 `lib.rs` 的 `stop_engine()` / `on_window_destroyed_shutdown()`。
    pub fn shutdown(&mut self, grace: Duration) -> Result<serde_json::Value, String> {
        let supervisor = Arc::clone(&self.supervisor);
        let engine_state_dir = supervisor.lock().unwrap().config().engine_state_dir.clone();
        let detached = {
            let mut guard = supervisor.lock().unwrap();
            guard.begin_stop()
        };
        let outcome = match detached {
            None => Ok((false, None)),
            Some(channel) => EngineSupervisor::finish_stop(channel, grace, &engine_state_dir),
        };
        {
            let mut guard = supervisor.lock().unwrap();
            guard.end_stop();
        }
        // 通道已被摘除：无论关闭成功与否，旧引擎都不再可用，如实重置自身状态。
        self.mark_stopped();
        let (forced, exit_code) = outcome?;
        Ok(serde_json::json!({
            "forced_terminate": forced,
            "process_exit_code": exit_code,
            "note": if forced { "超出优雅等待，已强制终止" } else { "引擎已优雅退出" },
        }))
    }

    /// 仅重置“已初始化”状态（OD-ARCH-024）。
    ///
    /// 供宿主在**不持有 supervisor 锁的**关闭等待之后调用，使后续命令得到
    /// 如实的 `engine_unavailable`（“未运行或已停止”），而不是含糊的成功/超时。
    pub fn mark_stopped(&mut self) {
        self.initialized = false;
        self.descriptor = None;
        self.pump_running.store(false, Ordering::SeqCst);
    }

    /// 仅测试：安装一个在取得 supervisor 锁之前执行的回调（生产构建不存在）。
    #[cfg(test)]
    pub(crate) fn set_pre_lock_hook(&mut self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        self.pre_lock_hook = hook;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_supervisor::{supervisor_with_fake_channel, EngineConfig};
    use std::path::PathBuf;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    fn fake_config() -> EngineConfig {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        let app = std::env::temp_dir().join("omnidesk-host-tests").join("app");
        EngineConfig {
            python: std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string()),
            engine_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("fake_engine"),
            dsh_cli: PathBuf::from("/bin/true"),
            dsh_home: home.join("OmniDesk").join("runtime").join("dsh-home"),
            agents_home: home.join("OmniDesk").join("runtime").join("agents-home"),
            engine_state_dir: app.join("engine-state"),
            result_dir: app.join("engine-state").join("results"),
            profile: "web".to_string(),
            channel: "web-api".to_string(),
            patch_files: vec![],
        }
    }

    /// 事件泵运行期间，`health()` 必须在有限时间内返回。
    ///
    /// 回归背景：旧事件泵在**持有 supervisor 互斥锁**时做 500ms 阻塞等待，超时后立即
    /// 重新加锁形成紧循环，把 GTK 主线程上的 `engine_health` 永久饿死，窗口表现为
    /// 整体「无响应」。本测试在旧实现下会明确失败（10 秒内拿不到锁）。
    #[test]
    fn event_pump_does_not_starve_host_lock() {
        let supervisor = supervisor_with_fake_channel(fake_config());
        let bridge = EngineBridge::new(Arc::new(Mutex::new(supervisor)));
        let shared = Arc::new(Mutex::new(bridge));
        let sink: EventSink = Arc::new(|_| {});

        {
            let mut guard = shared.lock().unwrap();
            guard
                .ensure_initialized(&sink, &crate::admission::AdmissionGrant::for_test())
                .expect("假引擎应能完成 omni.initialize 并启动事件泵");
        }

        for round in 0..5 {
            let (tx, rx) = channel();
            {
                let shared = Arc::clone(&shared);
                thread::Builder::new()
                    .name(format!("health-probe-{}", round))
                    .spawn(move || {
                        let start = Instant::now();
                        let health = {
                            let mut guard = shared.lock().unwrap();
                            guard.health("/tmp/omnidesk-host-tests/app")
                        };
                        let _ = tx.send((health, start.elapsed()));
                    })
                    .expect("spawn health probe");
            }

            let (health, elapsed) = rx
                .recv_timeout(Duration::from_secs(10))
                .expect("health() 在 10 秒内未返回：宿主锁被事件泵饿死（旧缺陷重现）");
            assert_eq!(health["running"], true, "引擎应在运行");
            assert!(
                elapsed < Duration::from_secs(2),
                "第 {} 轮 health() 耗时 {:?}，超出可接受范围（疑似锁饥饿）",
                round,
                elapsed
            );
        }

        let mut guard = shared.lock().unwrap();
        let outcome = guard.shutdown(Duration::from_secs(5)).expect("关闭引擎");
        assert_eq!(outcome["forced_terminate"], false, "假引擎应能优雅退出");
    }

    /// OD-ARCH-023：未绑定权限世代的准入凭据不得启动引擎。
    ///
    /// 这堵住“在引擎调用守卫之外取得准入结果、之后再用来启动引擎”的路径；
    /// 用真实假引擎验证：拒绝时**不创建子进程**。
    #[test]
    fn unstamped_grant_cannot_start_engine() {
        let supervisor = supervisor_with_fake_channel(fake_config());
        let bridge = EngineBridge::new(Arc::new(Mutex::new(supervisor)));
        let shared = Arc::new(Mutex::new(bridge));
        let sink: EventSink = Arc::new(|_| {});

        let mut guard = shared.lock().unwrap();
        let err = guard
            .ensure_initialized(&sink, &crate::admission::AdmissionGrant::unstamped_for_test())
            .expect_err("未绑定世代的凭据必须被拒绝");
        let health_before = guard.health("/tmp/omnidesk-host-tests/app");

        // 对照组：盖章后的凭据正常初始化；先收尾再断言，避免任何断言失败时
        // 留下子进程阻塞测试框架。
        let init = guard.ensure_initialized(&sink, &crate::admission::AdmissionGrant::for_test());
        let health_after = guard.health("/tmp/omnidesk-host-tests/app");
        let shutdown = guard.shutdown(Duration::from_secs(5));

        assert_eq!(err["engine_error_code"], "grant_not_stamped");
        assert_eq!(err["detail"]["enforced_by"], "host");
        // 拒绝发生在启动/初始化之前：子进程未初始化。
        assert_eq!(health_before["note"], "引擎子进程在运行（未初始化）");
        assert!(health_before["engine_version"].is_null());

        init.expect("持当前世代的凭据应能启动假引擎");
        assert_eq!(health_after["note"], "引擎已初始化并在运行");
        assert!(!health_after["engine_version"].is_null());
        let _ = shutdown;
    }
}
