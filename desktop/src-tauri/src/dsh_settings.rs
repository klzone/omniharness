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

use crate::local_http;

/// 设置页一次「测试连接 / 拉模型列表」的结果（OD-ARCH-054）。
///
/// `classification` 只用于界面分组，固定为下列之一：
/// `ok` / `auth` / `network` / `format` / `unsupported` / `local`。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ModelProbe {
    pub classification: String,
    pub message: String,
    pub models: Vec<DiscoveredModel>,
    /// 这次请求用的是哪来的密钥：stored / input / none（只描述来源，不含值）。
    pub key_source: String,
    /// 该 provider 命中 DSH 内置目录时为 true：返回内置模型表，未发起真实网络请求。
    pub builtin_catalog: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
}

/// 等 DSH 打印本地地址的上限（首次启动要装插件、跑 HMR 等）。
const STARTUP_TIMEOUT: Duration = Duration::from_secs(25);
/// 判断“端口还在听”的连接超时。
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// 已经跑起来的设置服务（令牌只在内存里）。
struct Running {
    pid: u32,
    port: u16,
    url: String,
    /// 启动签名（Profile + 补丁文件的内容指纹）。配置变了必须重建进程，
    /// 否则设置服务仍按旧路由解析凭据，探测结果与引擎不一致（OD-ARCH-065 S1）。
    signature: String,
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
        let signature = launch_signature(config);
        if let Some(running) = self.reusable() {
            if running.signature == signature {
                return Ok(Opened {
                    port: running.port,
                    reused: true,
                });
            }
            // 配置已变：停掉旧实例再按新参数起，避免复用一份过期路由。
            let mut guard = self.running.lock().unwrap();
            if let Some(old) = guard.take() {
                kill_pid(old.pid);
            }
        }
        // 走到这里说明内存里没有可用实例：清掉遗留进程，再起一个新的。
        stop_recorded(record_path);
        let child = spawn(config, log_path)?;
        let pid = child.id();
        match wait_for_url(child, log_path) {
            Ok((url, port)) => {
                write_record(record_path, pid, port);
                let mut guard = self.running.lock().unwrap();
                *guard = Some(Running {
                    pid,
                    port,
                    url,
                    signature,
                });
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
                signature: running.signature.clone(),
            })
        } else {
            None
        }
    }

    /// 用一个**内存中**的会话调用 DSH 官方 `credentials` RPC（OD-ARCH-053 S1）。
    ///
    /// 为什么不用引擎通道：用户配置新 provider 时往往还没开启对应端点的模型权限，
    /// 此时引擎起不来，写 key 会死锁在准入上。这里直接对本机 DSH 实例走官方
    /// `credentials/set|describe|unset`（`packages/api/settings-controller/src/credentials.ts`），
    /// 不需要模型权限；会话 cookie 只在本次调用内存在，不落盘、不进前端。
    /// 失败时只回报状态码与 DSH 自己的错误文本，不回显请求体（里面是密钥）。
    pub fn credentials_rpc(
        &self,
        config: &EngineConfig,
        log_path: &Path,
        record_path: &Path,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if !matches!(method, "credentials/describe" | "credentials/set" | "credentials/unset") {
            return Err(format!("拒绝调用未知的凭据方法：{}", method));
        }
        let opened = self.ensure(config, log_path, record_path)?;
        let url = self
            .reusable()
            .map(|r| r.url)
            .ok_or_else(|| "DSH 设置服务已启动，但地址没有留在内存里；请重试。".to_string())?;
        let cookie = exchange_token(opened.port, &url)?;
        let body = serde_json::json!({
            "type": "client-request",
            "rpcId": format!("omni-settings-{}", std::process::id()),
            "method": method,
            "payload": { "args": args },
        })
        .to_string();
        let response = local_http::request(
            opened.port,
            "POST",
            &format!("/api/{}", method),
            &[
                ("Content-Type", "application/json".to_string()),
                ("Cookie", cookie),
            ],
            Some(&body),
            Duration::from_secs(20),
        )?;
        if response.status == 401 || response.status == 403 {
            return Err(format!(
                "DSH 拒绝了本地会话（HTTP {}）；请重试，若仍失败请重启应用。",
                response.status
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
            format!(
                "DSH 凭据接口返回的不是 JSON（HTTP {}）：{}",
                response.status, e
            )
        })?;
        let result = parsed.get("result").cloned().unwrap_or(serde_json::Value::Null);
        if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let code = result
                .pointer("/error/code")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let message = result
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("DSH 凭据接口拒绝本次请求");
            return Err(format!("{}（{}）", message, code));
        }
        Ok(result.get("value").cloned().unwrap_or(serde_json::Value::Null))
    }

    /// 拉取某个端点声明的模型列表，并做一次真正的连接测试（OD-ARCH-054 S1/S2）。
    ///
    /// 为什么不从 Rust 直接请求 base_url：宿主只允许 http 回环（`local_http`），
    /// 真实端点几乎都是 https。DSH 自带 `llm/discoverModels`（`packages/llm/llm-pi-ai/src/discovery.ts`），
    /// 由它发请求、自己处理 TLS 与鉴权头；宿主只负责把草稿端点传进去、把失败归类。
    ///
    /// 草稿里**不含**落在磁盘上的密钥：`api_key` 只在本次调用内使用，不落盘、不记录。
    /// 不传 `provider`，避免命中内置 catalog 而绕过真实网络请求。
    pub fn probe_models(
        &self,
        config: &EngineConfig,
        log_path: &Path,
        record_path: &Path,
        base_url: &str,
        api: &str,
        provider: Option<&str>,
        api_key: Option<&str>,
        timeout: Duration,
    ) -> Result<ModelProbe, String> {
        let mut draft = serde_json::json!({ "baseURL": base_url, "api": api });
        let typed = api_key.filter(|k| !k.trim().is_empty());
        if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
            draft["apiKey"] = serde_json::Value::String(key.to_string());
        }
        if let Some(name) = provider.filter(|p| !p.trim().is_empty()) {
            draft["provider"] = serde_json::Value::String(name.trim().to_string());
        }
        let provider_named = provider.map(|p| !p.trim().is_empty()).unwrap_or(false);
        let key_source = if typed.is_some() {
            "input"
        } else if provider_named {
            "stored"
        } else {
            "none"
        };
        let opened = self.ensure(config, log_path, record_path)?;
        let url = self
            .reusable()
            .map(|r| r.url)
            .ok_or_else(|| "DSH 设置服务已启动，但地址没有留在内存里；请重试。".to_string())?;
        let cookie = exchange_token(opened.port, &url)?;
        let body = serde_json::json!({
            "type": "client-request",
            "rpcId": format!("omni-probe-{}", std::process::id()),
            "method": "llm/discoverModels",
            // Gateway 的 Remote 参数是**按名字**传的（`args` 必须是对象），
            // 传数组会被 `gateway/arguments-invalid` 直接拒掉——实测踩到过。
            "payload": { "args": { "settingsNs": "llm-pi-ai", "request": draft } },
        })
        .to_string();
        let response = match local_http::request(
            opened.port,
            "POST",
            "/api/llm/discoverModels",
            &[
                ("Content-Type", "application/json".to_string()),
                ("Cookie", cookie),
            ],
            Some(&body),
            timeout,
        ) {
            Ok(response) => response,
            Err(message) => {
                // 请求还没到 DSH 就失败，或 DSH 迟迟不返回：都归为「网络/超时」。
                return Ok(ModelProbe {
                    classification: "network".to_string(),
                    message: format!(
                        "{} 秒内没有拿到结果：{}",
                        timeout.as_secs(),
                        message
                    ),
                    models: Vec::new(),
                    key_source: key_source.to_string(),
                    builtin_catalog: false,
                });
            }
        };
        if response.status == 401 || response.status == 403 {
            return Err(format!(
                "DSH 拒绝了本地会话（HTTP {}）；请重试，若仍失败请重启应用。",
                response.status
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
            format!(
                "DSH 模型发现接口返回的不是 JSON（HTTP {}）：{}",
                response.status, e
            )
        })?;
        let result = parsed.get("result").cloned().unwrap_or(serde_json::Value::Null);
        let mut probe = classify_probe(&result);
        probe.key_source = key_source.to_string();
        Ok(probe)
    }
}

/// 把 `llm/discoverModels` 的 JSON-RPC 结果翻译成界面能直接显示的判定。
///
/// 分类口径（失败原因只有三类起步，另加两种「不是端点的问题」）：
///   * `ok`          —— 拿到了模型列表；
///   * `auth`        —— 401/403 或 DSH 明说检查 API Key；
///   * `network`     —— 连不上、超时、DNS/TLS 之类；
///   * `format`      —— 通了，但返回的不是可读的模型列表；
///   * `unsupported` —— 该协议没有可读的列表接口（DSH 的 DISCOVERY_UNSUPPORTED）；
///   * `local`       —— 本机 DSH 侧的问题（找不到发现服务等）。
///
/// 只读错误文本做归类，不改写 DSH 的原因，避免界面比底层知道得更多。
pub fn classify_probe(result: &serde_json::Value) -> ModelProbe {
    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        let models = result
            .get("value")
            .and_then(|v| v.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        let id = row.get("id").and_then(|v| v.as_str())?.to_string();
                        let name = row
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&id)
                            .to_string();
                        Some(DiscoveredModel { id, name })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let message = if models.is_empty() {
            "端点可以连通，但没有返回任何模型；可以直接手填模型 id。".to_string()
        } else {
            format!("连接成功，读到 {} 个模型。", models.len())
        };
        return ModelProbe {
            classification: "ok".to_string(),
            message,
            models,
            key_source: "none".to_string(),
            builtin_catalog: false,
        };
    }
    let code = result
        .pointer("/error/code")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let message = result
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .unwrap_or("DSH 没有给出失败原因");
    let lower = message.to_ascii_lowercase();
    let classification = if lower.contains("check the api key")
        || lower.contains("answered 401")
        || lower.contains("answered 403")
        || lower.contains("invalid_credential")
        || lower.contains("missing_credential")
    {
        "auth"
    } else if lower.contains("answered 404") || lower.contains("answered 405") {
        // 连通了，但那个路径不提供模型列表：多半是地址少了/多了 `/v1`。
        "format"
    } else if lower.contains("could not reach")
        || lower.contains("aborted")
        || lower.contains("timed out")
        || lower.contains("certificate")
        || lower.contains("dns")
    {
        "network"
    } else if code == "DISCOVERY_UNSUPPORTED" || code == "NO_DISCOVERY" {
        "unsupported"
    } else if lower.contains("neither a \"data\" array nor a \"models\" object")
        || lower.contains("not json")
        || lower.contains("has no model listing")
    {
        "format"
    } else if code == "INVALID_DISCOVERY" || code == "DISCOVERY_FAILED" {
        "network"
    } else {
        "local"
    };
    ModelProbe {
        classification: classification.to_string(),
        message: format!("{}（{}）", message, code),
        models: Vec::new(),
        key_source: "none".to_string(),
        builtin_catalog: false,
    }
}

/// 用 URL 里的会话令牌换 cookie（303 + `Set-Cookie`），只在本进程内存中使用。
fn exchange_token(port: u16, url: &str) -> Result<String, String> {
    let path = match url.find("/?token=") {
        Some(i) => &url[i..],
        None => return Err("DSH 设置服务地址里没有会话令牌；请重启应用后重试。".to_string()),
    };
    let response = local_http::request(port, "GET", path, &[], None, Duration::from_secs(10))?;
    let cookie = response
        .header_all("set-cookie")
        .iter()
        .filter_map(|c| c.split(';').next())
        .map(|c| c.trim())
        .filter(|c| c.contains('='))
        .collect::<Vec<_>>()
        .join("; ");
    if cookie.is_empty() {
        return Err(format!(
            "DSH 没有返回本地会话 cookie（HTTP {}）",
            response.status
        ));
    }
    Ok(cookie)
}

/// 起一个 `dsh --profile <p> --port 0 --no-open`，输出按行打码后写日志。
///
/// 注意：`web` **不是** dsh 的子命令。0.2.1-alpha.2 的 CLI 只有 `plugin` 一个子命令，
/// `web` 会被当成 app 参数而报 “too many arguments”；`--port/--no-open` 必须直接跟在
/// Profile 与 `--patch` 之后。实测 `dsh --profile web --port 0 --no-open` 会打印
/// `dsh web: http://127.0.0.1:<port>/?token=...`（OD-ARCH-065）。
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
    // 参数只取自 EngineConfig：Profile 与补丁文件已在宿主侧校验，
    // 不接受任何外部输入（OD-ARCH-065 S1）。
    // \`--profile\` / \`--patch\` 是 launcher 级参数，必须排在 \`web\` 之前。
    let mut command = Command::new(&node);
    command
        .arg(&config.dsh_cli)
        .arg("--profile")
        .arg(&config.profile);
    for patch in &config.patch_files {
        command.arg("--patch").arg(patch);
    }
    command
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

/// 设置服务进程的启动签名：Profile 名 + 每个补丁文件的路径与内容摘要。
///
/// 覆盖文件每次保存都会换内容，签名随之改变，旧设置服务进程不会被误复用
/// （否则探测仍按旧路由解析凭据，结果与引擎不一致）。
fn launch_signature(config: &EngineConfig) -> String {
    let mut parts: Vec<String> = vec![config.profile.clone()];
    for patch in &config.patch_files {
        let digest = std::fs::read(patch)
            .map(|bytes| crate::sha256::sha256_hex(&bytes))
            .unwrap_or_else(|_| "missing".to_string());
        parts.push(format!("{}#{}", patch.display(), digest));
    }
    parts.join("|")
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

    // —— OD-ARCH-054：模型发现结果归类（纯函数，不联网） ——

    fn probe_of(value: serde_json::Value) -> ModelProbe {
        classify_probe(&value)
    }

    #[test]
    fn probe_lists_discovered_models() {
        let probe = probe_of(serde_json::json!({
            "ok": true,
            "value": [
                { "id": "synthetic-model", "name": "Synthetic" },
                { "id": "no-name" }
            ]
        }));
        assert_eq!(probe.classification, "ok");
        assert_eq!(probe.models.len(), 2);
        assert_eq!(probe.models[1].name, "no-name");
        assert!(probe.message.contains("2"));
    }

    #[test]
    fn probe_reports_empty_catalogue_as_success() {
        let probe = probe_of(serde_json::json!({ "ok": true, "value": [] }));
        assert_eq!(probe.classification, "ok");
        assert!(probe.models.is_empty());
        assert!(probe.message.contains("手填"));
    }

    #[test]
    fn probe_classifies_wrong_key_as_auth() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": {
                "code": "DISCOVERY_FAILED",
                "message": "https://example.invalid/v1/models answered 401; check the API key"
            }
        }));
        assert_eq!(probe.classification, "auth");
        assert!(probe.models.is_empty());
        assert!(probe.message.contains("DISCOVERY_FAILED"));
    }

    #[test]
    fn probe_classifies_unreachable_as_network() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": {
                "code": "DISCOVERY_FAILED",
                "message": "could not reach http://127.0.0.1:9/v1/models"
            }
        }));
        assert_eq!(probe.classification, "network");
    }

    #[test]
    fn probe_classifies_bad_listing_shape_as_format() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": {
                "code": "DISCOVERY_FAILED",
                "message": "the endpoint's model listing has neither a \"data\" array nor a \"models\" object; enter this provider's models by hand"
            }
        }));
        assert_eq!(probe.classification, "format");
    }

    #[test]
    fn probe_classifies_wrong_path_as_format() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": {
                "code": "DISCOVERY_FAILED",
                "message": "http://127.0.0.1:1/v1/models answered 404"
            }
        }));
        assert_eq!(probe.classification, "format");
    }

    #[test]
    fn probe_classifies_unsupported_protocol() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": {
                "code": "DISCOVERY_UNSUPPORTED",
                "message": "pi-ai protocol \"x\" has no model listing this build can read; enter this provider's models by hand"
            }
        }));
        assert_eq!(probe.classification, "unsupported");
    }

    #[test]
    fn probe_classifies_unknown_failure_as_local() {
        let probe = probe_of(serde_json::json!({
            "ok": false,
            "error": { "code": "gateway/internal", "message": "boom" }
        }));
        assert_eq!(probe.classification, "local");
    }
}
