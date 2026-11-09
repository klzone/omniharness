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
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 引擎域错误统一使用该 JSON-RPC 码（与 Python 侧 serve.py 一致）。
pub const ENGINE_ERROR_CODE: i64 = -32000;
/// 引擎 stderr 诊断日志：单文件 1 MiB，保留当前 + 3 份滚动副本。
pub const ENGINE_STDERR_MAX_BYTES: u64 = 1024 * 1024;
const ENGINE_STDERR_BACKUPS: usize = 3;

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
    /// OD-SYS-701：Windows 上引擎及孙进程所在的 Job Object。
    ///
    /// 句柄随本通道 Drop 关闭；kill-on-close 让系统回收整棵进程树。
    /// 非 Windows 是零大小空实现，不产生任何系统调用。
    job: crate::job_object::JobObject,
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
        cwd: &Path,
        envs: &[(String, String)],
    ) -> std::io::Result<Self> {
        Self::spawn_with_stderr_log(program, args, cwd, envs, None)
    }

    /// 同 `spawn`，但把引擎 stderr 追加到滚动诊断日志（OD-ARCH-145 A）。
    pub fn spawn_with_stderr_log(
        program: &str,
        args: &[String],
        cwd: &Path,
        envs: &[(String, String)],
        stderr_log: Option<&Path>,
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
        // Windows：引擎是控制台子系统程序，不隐藏控制台会弹黑框（OD-ARCH-134）。
        crate::platform::hide_console(&mut cmd);

        let mut child = cmd.spawn()?;
        let pid = child.id();
        // OD-SYS-701：spawn 后立刻挂进 Job Object。Windows 没有 POSIX 进程组，
        // 这是退出/强杀时回收 node 孙进程的唯一系统级保证；挂接失败则不继续
        // 运行无保护的引擎。非 Windows 分支为零开销空操作。
        let job = match crate::job_object::JobObject::create_for_process(pid) {
            Ok(job) => job,
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::other(format!(
                    "引擎子进程 Job Object 挂接失败：{}",
                    err
                )));
            }
        };
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
            let log_path = stderr_log.map(Path::to_path_buf);
            thread::Builder::new()
                .name("omnidesk-engine-stderr".to_string())
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if let Some(path) = &log_path {
                            append_stderr_line(path, &line);
                        }
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
            job,
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

    /// 是否由 Windows Job Object 保护（非 Windows 恒为 false，仅供测试断言）。
    pub fn job_protected(&self) -> bool {
        self.job.attached()
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

fn stderr_backup_path(path: &Path, index: usize) -> PathBuf {
    let mut raw = path.as_os_str().to_os_string();
    raw.push(format!(".{}", index));
    PathBuf::from(raw)
}

fn rotate_stderr_log(path: &Path) {
    let _ = fs::remove_file(stderr_backup_path(path, ENGINE_STDERR_BACKUPS));
    for index in (1..ENGINE_STDERR_BACKUPS).rev() {
        let _ = fs::rename(
            stderr_backup_path(path, index),
            stderr_backup_path(path, index + 1),
        );
    }
    let _ = fs::rename(path, stderr_backup_path(path, 1));
}

fn append_stderr_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::metadata(path)
        .map(|meta| meta.len() > ENGINE_STDERR_MAX_BYTES)
        .unwrap_or(false)
    {
        rotate_stderr_log(path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", stamp, line);
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

    #[test]
    fn stderr_log_is_bounded_and_rotated() {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-stderr-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let path = base.join("logs/engine-stderr.log");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![b'x'; ENGINE_STDERR_MAX_BYTES as usize + 1]).unwrap();
        append_stderr_line(&path, "合成诊断行");
        assert!(stderr_backup_path(&path, 1).is_file(), "超限后应滚动旧日志");
        let current = fs::read_to_string(&path).unwrap();
        assert!(current.contains("合成诊断行"));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn spawn_with_stderr_log_captures_child_stderr() {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-stderr-spawn-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let log = base.join("logs/engine-stderr.log");
        let python = std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string());
        let code = "import sys; sys.stderr.write('合成引擎启动信息\\n'); sys.stderr.flush()";
        let mut channel = EngineChannel::spawn_with_stderr_log(
            &python,
            &["-c".to_string(), code.to_string()],
            &std::env::temp_dir(),
            &[],
            Some(&log),
        )
        .expect("应能启动合成 stderr 子进程");
        let mut captured = String::new();
        for _ in 0..40 {
            captured = fs::read_to_string(&log).unwrap_or_default();
            if captured.contains("合成引擎启动信息") {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = channel.wait_exit(Duration::from_secs(2));
        assert!(captured.contains("合成引擎启动信息"), "日志应包含子进程 stderr");
        let _ = fs::remove_dir_all(base);
    }
}
