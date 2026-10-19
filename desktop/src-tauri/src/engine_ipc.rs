//! Rust 宿主与 Python Engine 子进程之间的最小本地 IPC（Step 8）。
//!
//! 通道形态：**行分隔 JSON-RPC 2.0 over stdio**（契约 docs/OmniDesk-Desktop-Engine-IPC.md §1.2）。
//! Rust 只负责"把请求帧写出去、把响应与通知帧收回来"，不做任何 ACP / DSH 语义映射。
//!
//! 边界：
//!   * 不监听任何 TCP / UDP 端口（无网络面）；
//!   * 不修改、不复制 DSH Runtime；
//!   * 不读取、不打印、不落盘任何凭证。
//!
//! 事件（omni.event 通知）通过一个可阻塞取用的接收端交给上层；响应按 id 分派给
//! 对应的等待者。两者都在同一个读线程里完成，避免竞态。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 引擎域错误统一使用该 JSON-RPC 码（与 Python 侧 serve.py 一致）。
pub const ENGINE_ERROR_CODE: i64 = -32000;

/// 结构化 RPC 错误：保留 JSON-RPC 码、消息与 data（含引擎错误码与五属性）。
#[derive(Debug, Clone)]
pub struct EngineRpcError {
    pub code: i64,
    pub message: String,
    pub data: serde_json::Value,
}

impl EngineRpcError {
    /// 引擎错误码（data.engine_error_code），没有则退化为 JSON-RPC 码描述。
    pub fn engine_error_code(&self) -> String {
        if let Some(code) = self.data.get("engine_error_code").and_then(|v| v.as_str()) {
            return code.to_string();
        }
        if self.code == ENGINE_ERROR_CODE {
            "engine_error".to_string()
        } else {
            format!("jsonrpc_{}", self.code)
        }
    }

    /// 读取引擎错误五属性之一（retryable / needs_human / ...）。
    pub fn attribute(&self, key: &str) -> Option<String> {
        self.data
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn task_id(&self) -> Option<String> {
        self.data
            .get("omni_task_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// 通道级（非引擎返回）错误的构造入口。
    pub fn transport(message: impl Into<String>) -> Self {
        Self {
            code: ENGINE_ERROR_CODE,
            message: message.into(),
            data: serde_json::json!({
                "engine_error_code": "engine_unavailable",
                "retryable": "conditional",
                "needs_human": "yes",
                "degradable": "yes",
                "may_duplicate_execution": "no",
                "may_lose_output": "no",
            }),
        }
    }

    /// 宿主判定“引擎正在关闭”：请求**未发送**给引擎（OD-ARCH-024）。
    ///
    /// 与 `transport` 区分开，是为了让调用方和用户能分清两种结果：
    /// “请求发出去了但引擎没在”（`engine_unavailable`）与“压根没发，因为正在关闭”
    /// （`engine_shutting_down`）。后者可以明确告知用户重新打开权限后再试。
    pub fn shutting_down(message: impl Into<String>) -> Self {
        Self {
            code: ENGINE_ERROR_CODE,
            message: message.into(),
            data: serde_json::json!({
                "engine_error_code": "engine_shutting_down",
                "retryable": "yes",
                "needs_human": "no",
                "degradable": "no",
                "may_duplicate_execution": "no",
                "may_lose_output": "no",
                "detail": {
                    "reason": "engine_shutting_down",
                    "sent_to_engine": false,
                    "enforced_by": "host",
                },
            }),
        }
    }
}

impl std::fmt::Display for EngineRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}（{}）", self.engine_error_code(), self.message)
    }
}

impl std::error::Error for EngineRpcError {}

/// 引擎子进程通道。
pub struct EngineChannel {
    child: Child,
    stdin: Option<ChildStdin>,
    pending: Arc<Mutex<HashMap<u64, Sender<serde_json::Value>>>>,
    /// 通知接收端。用 `Arc<Mutex<..>>` 是为了让事件泵能拿到一个**独立句柄**，
    /// 在完全不持有上层锁的情况下等待通知（见 engine_bridge.rs 的锁饥饿修复）。
    notif_rx: Arc<Mutex<Receiver<serde_json::Value>>>,
    next_id: AtomicU64,
    exited: Arc<AtomicBool>,
    exit_code: Arc<Mutex<Option<i32>>>,
    stderr_tail: Arc<Mutex<Vec<String>>>,
    pid: u32,
}

impl EngineChannel {
    /// 启动引擎子进程并建立通道。
    ///
    /// args 为完整命令行（例如 ["python3", "-m", "omnidesk_engine"]）。
    /// cwd 必须是引擎包所在目录，envs 为显式下发的环境变量（含隔离数据根）。
    pub fn spawn(
        program: &str,
        args: &[String],
        cwd: &std::path::Path,
        envs: &[(String, String)],
    ) -> std::io::Result<Self> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in envs {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("engine stdin 不可用"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("engine stdout 不可用"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("engine stderr 不可用"))?;

        let pending: Arc<Mutex<HashMap<u64, Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let exited = Arc::new(AtomicBool::new(false));
        let exit_code: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
        let stderr_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (notif_tx, notif_rx) = channel::<serde_json::Value>();

        // 读线程：按 id 分派响应，按 method 分派通知。
        {
            let pending = Arc::clone(&pending);
            let exited = Arc::clone(&exited);
            let exit_code = Arc::clone(&exit_code);
            thread::Builder::new()
                .name("omnidesk-engine-stdout".to_string())
                .spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let Ok(frame) = serde_json::from_str::<serde_json::Value>(line) else {
                            // 未解析帧只作为诊断，不污染协议通道。
                            let _ = notif_tx.send(serde_json::json!({
                                "_unparsed": line,
                            }));
                            continue;
                        };
                        let has_method = frame.get("method").is_some();
                        if has_method {
                            let _ = notif_tx.send(frame);
                            continue;
                        }
                        if let Some(id) = frame.get("id").and_then(|v| v.as_u64()) {
                            let waiter = pending.lock().unwrap().remove(&id);
                            if let Some(tx) = waiter {
                                let _ = tx.send(frame);
                            }
                        }
                    }
                    // stdout 结束 => 子进程正在退出
                    exited.store(true, Ordering::SeqCst);
                    let _ = exit_code;
                })
                .expect("spawn stdout reader");
        }

        // stderr 线程：只保留末尾若干行用于诊断，绝不写入协议通道。
        {
            let tail = Arc::clone(&stderr_tail);
            thread::Builder::new()
                .name("omnidesk-engine-stderr".to_string())
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let mut buf = tail.lock().unwrap();
                        if buf.len() >= 200 {
                            buf.remove(0);
                        }
                        buf.push(line);
                    }
                })
                .expect("spawn stderr reader");
        }

        Ok(Self {
            child,
            stdin: Some(stdin),
            pending,
            notif_rx: Arc::new(Mutex::new(notif_rx)),
            next_id: AtomicU64::new(1),
            exited,
            exit_code,
            stderr_tail,
            pid,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// 发送一个 request 并同步等待其响应（带显式超时）。
    ///
    /// 超时映射为错误，绝不当作成功；响应与请求 id 一一对应。
    pub fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, EngineRpcError> {
        if !self.is_running() {
            return Err(EngineRpcError::transport(format!(
                "引擎进程已退出，无法发送 {}",
                method
            )));
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel::<serde_json::Value>();
        self.pending.lock().unwrap().insert(id, tx);

        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut payload = serde_json::to_vec(&frame).map_err(|e| {
            EngineRpcError::transport(format!("请求序列化失败：{}", e))
        })?;
        payload.push(b'\n');

        let write_result = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(&payload).and_then(|_| stdin.flush()),
            None => Err(std::io::Error::other("引擎 stdin 已关闭")),
        };
        if let Err(err) = write_result {
            self.pending.lock().unwrap().remove(&id);
            return Err(EngineRpcError::transport(format!(
                "写入引擎 stdin 失败：{}",
                err
            )));
        }

        match rx.recv_timeout(timeout) {
            Ok(frame) => {
                if let Some(error) = frame.get("error") {
                    return Err(Self::parse_error(error));
                }
                Ok(frame
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null))
            }
            Err(RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap().remove(&id);
                Err(EngineRpcError {
                    code: ENGINE_ERROR_CODE,
                    message: format!("{} 在 {:?} 内未返回响应", method, timeout),
                    data: serde_json::json!({
                        "engine_error_code": "timeout",
                        "retryable": "conditional",
                        "needs_human": "no",
                        "degradable": "yes",
                        "may_duplicate_execution": "yes",
                        "may_lose_output": "yes",
                    }),
                })
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.pending.lock().unwrap().remove(&id);
                Err(EngineRpcError::transport(
                    "引擎通道在响应到达前断开（进程可能已退出）",
                ))
            }
        }
    }

    fn parse_error(error: &serde_json::Value) -> EngineRpcError {
        EngineRpcError {
            code: error.get("code").and_then(|v| v.as_i64()).unwrap_or(ENGINE_ERROR_CODE),
            message: error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("引擎返回了未带消息的错误")
                .to_string(),
            data: error.get("data").cloned().unwrap_or(serde_json::Value::Null),
        }
    }

    /// 非阻塞取出一条通知帧（omni.event）。
    pub fn try_recv_notification(&self) -> Option<serde_json::Value> {
        self.notif_rx.lock().unwrap().try_recv().ok()
    }

    /// 带超时取出一条通知帧；用于事件泵。
    pub fn recv_notification_timeout(&self, timeout: Duration) -> Option<serde_json::Value> {
        self.notif_rx.lock().unwrap().recv_timeout(timeout).ok()
    }

    /// 通知接收端的共享句柄。
    ///
    /// 事件泵用它在**不持有任何上层锁**的前提下独立阻塞等待通知。
    /// 调用方不得在其它请求路径上长时间持有该锁。
    pub fn notification_receiver(&self) -> Arc<Mutex<Receiver<serde_json::Value>>> {
        Arc::clone(&self.notif_rx)
    }

    /// 进程是否已退出（stdout 关闭即视作退出）的共享标志。
    ///
    /// 事件泵用它判断「引擎是否还在」而无需再向上层取锁。
    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.exited)
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr_tail.lock().unwrap().clone()
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// 优雅终止：关闭 stdin（引擎据此收尾）→ 等待 → 必要时强杀。
    ///
    /// 返回是否发生强杀。**强杀不等于优雅取消**（契约 3.4）。
    pub fn terminate(&mut self, grace: Duration) -> std::io::Result<bool> {
        // 半关闭：丢弃 stdin 句柄，引擎读到 EOF 后自行收尾。
        self.stdin.take();

        let deadline = Instant::now() + grace;
        loop {
            match self.child.try_wait()? {
                Some(status) => {
                    *self.exit_code.lock().unwrap() = status.code();
                    self.exited.store(true, Ordering::SeqCst);
                    return Ok(false);
                }
                None => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }

        self.child.kill()?;
        let status = self.child.wait()?;
        *self.exit_code.lock().unwrap() = status.code();
        self.exited.store(true, Ordering::SeqCst);
        Ok(true)
    }

    /// 立即强杀（不等待）。仅用于测试与异常收尾。
    pub fn hard_kill(&mut self) -> std::io::Result<()> {
        self.child.kill()?;
        let status = self.child.wait()?;
        let mut code = self.exit_code.lock().unwrap();
        *code = status.code();
        Ok(())
    }

    pub fn wait_exit(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code();
                    let mut stored = self.exit_code.lock().unwrap();
                    *stored = code;
                    return code;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    thread::sleep(Duration::from_millis(30));
                }
                Err(_) => return None,
            }
        }
    }

    pub fn exit_code(&self) -> Option<i32> {
        *self.exit_code.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_error_code_prefers_engine_field() {
        let err = EngineRpcError {
            code: ENGINE_ERROR_CODE,
            message: "engine_unavailable".to_string(),
            data: serde_json::json!({ "engine_error_code": "engine_unavailable" }),
        };
        assert_eq!(err.engine_error_code(), "engine_unavailable");
        assert_eq!(err.attribute("retryable"), None);
    }

    #[test]
    fn transport_error_is_engine_unavailable() {
        let err = EngineRpcError::transport("boom");
        assert_eq!(err.engine_error_code(), "engine_unavailable");
        assert_eq!(err.attribute("may_duplicate_execution").as_deref(), Some("no"));
    }

    #[test]
    fn engine_error_code_falls_back_to_jsonrpc() {
        let err = EngineRpcError {
            code: -32601,
            message: "method not found".to_string(),
            data: serde_json::Value::Null,
        };
        assert_eq!(err.engine_error_code(), "jsonrpc_-32601");
    }
}
