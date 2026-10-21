//! DSH 设置服务与应用内独立窗口的宿主侧支撑（OD-ARCH-050）。
//!
//! 背景：旧实现每次点击「打开 DSH 设置」都 `dsh web`（固定 3080、自动开系统浏览器）。
//! 结果：第一次留下的进程一直占着 3080，之后每次点击都 `EADDRINUSE`，界面却只说「已启动」。
//!
//! 现在的口径：
//!   * 端口用 `--port 0`（临时端口），一次性实例，不再抢固定端口；
//!   * `--no-open`：不开系统浏览器，由宿主开应用内窗口；
//!   * 会话令牌（URL 里的 `?token=`）只留在宿主内存：**不写日志、不进前端、不落盘**；
//!     服务进程的输出在写日志前统一打码；
//!   * 实例信息（pid/port）记在应用数据根，用于复用与退出清理；记录里不含令牌。
//!   * 复用只在本进程内有效：应用重启后旧实例会被识别为遗留并停止（本模块负责）。

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::engine_supervisor::EngineConfig;

/// 等 DSH 打印本地地址的上限（首次启动要装插件、跑 HMR 等）。
const STARTUP_TIMEOUT: Duration = Duration::from_secs(25);
/// 判断“端口还在听”的连接超时。
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// 已经跑起来的设置服务（令牌只在内存里）。
struct Running {
    pid: u32,
    port: u16,
    url: String,
}

/// 一次打开请求的结果（不含令牌，可安全回给前端）。
pub struct Opened {
    pub port: u16,
    pub reused: bool,
}

pub struct SettingsService {
    running: Mutex<Option<Running>>,
}

impl Default for SettingsService {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsService {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(None),
        }
    }

    /// 确保设置服务在跑，并返回它的端口。复用优先，其次新起一个临时端口实例。
    pub fn ensure(
        &self,
        config: &EngineConfig,
        log_path: &Path,
        record_path: &Path,
    ) -> Result<Opened, String> {
        if let Some(running) = self.reusable() {
            return Ok(Opened {
                port: running.port,
                reused: true,
            });
        }
        // 走到这里说明内存里没有可用实例：清掉遗留进程，再起一个新的。
        stop_recorded(record_path);
        let child = spawn(config, log_path)?;
        let pid = child.id();
        match wait_for_url(child, log_path) {
            Ok((url, port)) => {
                write_record(record_path, pid, port);
                let mut guard = self.running.lock().unwrap();
                *guard = Some(Running { pid, port, url });
                Ok(Opened {
                    port,
                    reused: false,
                })
            }
            Err(message) => Err(message),
        }
    }

    /// 取当前可用实例的 URL（含令牌，只在宿主内部使用）。
    pub fn current_url(&self) -> Option<String> {
        self.reusable().map(|r| r.url)
    }

    /// 停止本进程起过的实例，并清理记录（应用退出时调用）。
    pub fn stop(&self, record_path: &Path) {
        let mut guard = self.running.lock().unwrap();
        if let Some(running) = guard.take() {
            kill_pid(running.pid);
        }
        stop_recorded(record_path);
    }

    fn reusable(&self) -> Option<Running> {
        let guard = self.running.lock().unwrap();
        let running = guard.as_ref()?;
        if process_alive(running.pid) && port_listening(running.port) {
            Some(Running {
                pid: running.pid,
                port: running.port,
                url: running.url.clone(),
            })
        } else {
            None
        }
    }
}

/// 起一个 `dsh web --port 0 --no-open`，输出按行打码后写日志。
fn spawn(config: &EngineConfig, log_path: &Path) -> Result<Child, String> {
    let node = std::env::var("OMNIDESK_NODE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "node".to_string());
    if !config.dsh_cli.is_file() {
        return Err(format!(
            "找不到引擎运行时里的 DSH CLI：{}；请先完成运行时引导（scripts/bootstrap-runtime.sh）。",
            config.dsh_cli.display()
        ));
    }
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建日志目录失败（{}）：{}", parent.display(), e))?;
    }
    Command::new(&node)
        .arg(&config.dsh_cli)
        .arg("web")
        .arg("--port")
        .arg("0")
        .arg("--no-open")
        .env("DSH_HOME", &config.dsh_home)
        .env("DSH_AGENTS_HOME", &config.agents_home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 DSH 设置服务失败（{}）：{}", node, e))
}

/// 等 DSH 打出本地地址；期间把输出打码落日志。返回（URL, 端口）。
fn wait_for_url(mut child: Child, log_path: &Path) -> Result<(String, u16), String> {
    let (tx, rx) = mpsc::channel::<String>();
    if let Some(out) = child.stdout.take() {
        pump(out, log_path.to_path_buf(), Some(tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        pump(err, log_path.to_path_buf(), None);
    }
    match await_url(&rx, STARTUP_TIMEOUT) {
        Some(url) => {
            let port = port_of(&url)
                .ok_or_else(|| format!("无法从 DSH 输出解析端口（日志：{}）", log_path.display()))?;
            Ok((url, port))
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err(format!(
                "DSH 设置服务启动超时（{} 秒内没打印本地地址）。日志：{}",
                STARTUP_TIMEOUT.as_secs(),
                log_path.display()
            ))
        }
    }
}

fn await_url(rx: &Receiver<String>, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        match rx.recv_timeout(left.min(Duration::from_millis(500))) {
            Ok(url) => return Some(url),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// 逐行读子进程输出：打码后追加日志；命中地址行时把原始 URL 交给宿主内存。
fn pump<R: std::io::Read + Send + 'static>(
    stream: R,
    log_path: PathBuf,
    url_sink: Option<mpsc::Sender<String>>,
) {
    std::thread::spawn(move || {
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok();
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Some(url) = extract_url(&line) {
                if let Some(sink) = url_sink.as_ref() {
                    let _ = sink.send(url);
                }
            }
            if let Some(file) = log.as_mut() {
                let _ = writeln!(file, "{}", redact(&line));
            }
        }
    });
}

/// 从一行输出里找 `dsh web: http://127.0.0.1:<port>/?token=...`。
fn extract_url(line: &str) -> Option<String> {
    let idx = line.find("dsh web: ")?;
    let rest = line[idx + "dsh web: ".len()..].trim();
    let candidate = rest.split_whitespace().next()?;
    if candidate.starts_with("http://127.0.0.1:") && candidate.contains("token=") {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// 会话令牌不得进日志：`token=xxx` 一律替换为 `token=***`。
pub fn redact(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("token=") {
        out.push_str(&rest[..i + "token=".len()]);
        let after = &rest[i + "token=".len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || c == '&' || c == '"' || c == '\'')
            .unwrap_or(after.len());
        out.push_str("***");
        out.push_str(&after[end..]);
        rest = "";
    }
    out.push_str(rest);
    out
}

fn port_of(url: &str) -> Option<u16> {
    let rest = url.strip_prefix("http://127.0.0.1:")?;
    let port = rest.split('/').next()?;
    port.parse().ok()
}

fn process_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

fn port_listening(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr: SocketAddr = match format!("127.0.0.1:{}", port).parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok()
}

fn kill_pid(pid: u32) {
    let _ = Command::new("kill")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// 记录文件只存 pid/port/时间，不含令牌。
fn write_record(path: &Path, pid: u32, port: u16) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = serde_json::json!({
        "pid": pid,
        "port": port,
        "started_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
    });
    let _ = std::fs::write(
        path,
        serde_json::to_vec_pretty(&payload).unwrap_or_default(),
    );
}

/// 停掉记录里的遗留实例。只认命令行里同时含 `dsh` 与 `web` 的进程，避免 pid 复用误杀。
fn stop_recorded(record_path: &Path) {
    let Ok(text) = std::fs::read_to_string(record_path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        let _ = std::fs::remove_file(record_path);
        return;
    };
    if let Some(pid) = value.get("pid").and_then(|v| v.as_u64()) {
        if looks_like_dsh_web(pid as u32) {
            kill_pid(pid as u32);
        }
    }
    let _ = std::fs::remove_file(record_path);
}

fn looks_like_dsh_web(pid: u32) -> bool {
    let path = Path::new("/proc").join(pid.to_string()).join("cmdline");
    let Ok(raw) = std::fs::read(path) else {
        return false;
    };
    let cmdline = String::from_utf8_lossy(&raw).replace('\0', " ");
    cmdline.contains("dsh") && cmdline.contains("web")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_hides_session_token() {
        let line = "dsh web: http://127.0.0.1:3080/?token=abc123 secret";
        let redacted = redact(line);
        assert!(!redacted.contains("abc123"), "token 不得留在日志里：{}", redacted);
        assert!(redacted.contains("token=***"));
        assert!(redacted.contains("http://127.0.0.1:3080/"));
    }

    #[test]
    fn extract_url_reads_printed_address() {
        let line = "dsh web: http://127.0.0.1:41234/?token=xyz";
        assert_eq!(
            extract_url(line).as_deref(),
            Some("http://127.0.0.1:41234/?token=xyz")
        );
        assert_eq!(port_of(&extract_url(line).unwrap()), Some(41234));
        assert_eq!(extract_url("dsh web: opening the default browser"), None);
    }
}
