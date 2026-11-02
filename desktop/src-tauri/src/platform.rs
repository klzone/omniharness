//! 跨平台适配（macOS / Windows / Linux）。
//!
//! 只集中三类差异，避免"打开目录"这类命令各写一份 Linux 专用实现：
//!   1. 家目录：Windows 没有 `HOME`，必须回退到 `USERPROFILE` 或 `HOMEDRIVE+HOMEPATH`；
//!   2. 系统打开器：macOS 用 `open`，Windows 用 `cmd /C start`，Linux 用 `xdg-open`；
//!   3. Python 解释器：Windows 习惯是 `python`，其余平台是 `python3`。
//!
//! 边界不变：打开器是固定程序，目标作为独立参数传入，不拼接 shell。

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// 家目录。Windows 上没有 `HOME`，回退顺序：USERPROFILE → HOMEDRIVE+HOMEPATH。
pub fn home_dir() -> Option<PathBuf> {
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            if !value.is_empty() {
                return Some(PathBuf::from(value));
            }
        }
    }
    let drive = std::env::var_os("HOMEDRIVE")?;
    let rest = std::env::var_os("HOMEPATH")?;
    let mut joined = drive.to_string_lossy().into_owned();
    joined.push_str(&rest.to_string_lossy());
    if joined.is_empty() {
        None
    } else {
        Some(PathBuf::from(joined))
    }
}

/// Python 解释器：尊重 `OMNIDESK_PYTHON`，否则按平台给默认值。
pub fn python_program() -> String {
    match std::env::var("OMNIDESK_PYTHON") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            if cfg!(windows) {
                "python".to_string()
            } else {
                "python3".to_string()
            }
        }
    }
}

/// 解析 Python 解释器的绝对路径，逻辑与 node 解析一致：
/// GUI 启动不继承 shell PATH，裸 "python3" 在 Finder 启动的 app 里会 ENOENT。
/// 顺序：OMNIDESK_PYTHON → PATH 探测 → /usr/bin 与 homebrew 常见位置。
pub fn resolve_python() -> Result<String, String> {
    if let Ok(value) = std::env::var("OMNIDESK_PYTHON") {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let fallback = python_program();
    let fallback_version_ok = Command::new(&fallback)
        .arg("-c")
        .arg("import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if fallback_version_ok {
        return Ok(fallback);
    }
    // PATH 里的 python3 版本过低时，按候选顺序找第一个 3.10+ 的解释器；
    // homebrew 安装根下的具体版本目录（python@3.12/3.13 等）也参与候选。
    let candidates = [
        PathBuf::from("/usr/bin/python3"),
        PathBuf::from("/opt/homebrew/bin/python3"),
        PathBuf::from("/usr/local/bin/python3"),
    ];
    let mut versioned: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/opt/homebrew/opt") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("python@3.") {
                versioned.push(entry.path().join("bin").join(&*name.replace("@", "")));
            }
        }
    }
    versioned.sort();
    for candidate in candidates.iter().chain(versioned.iter().rev()) {
        let ok = Command::new(candidate)
            .arg("-c")
            .arg("import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }
    Err(concat!(
        "未找到 python3。OmniDesk 的引擎需要 Python 3.10+。",
        "请安装 Python（https://www.python.org），",
        "或设置 OMNIDESK_PYTHON 指向 python3 的绝对路径。"
    )
    .to_string())
}

/// 仓库根（或免安装包根）。
///
/// 优先级：
///   1. `OMNIDESK_REPO_ROOT`；
///   2. 可执行文件所在目录向上找 `engine/omnidesk_engine` + `runtime/RUNTIME.lock`；
///   3. 编译期 `CARGO_MANIFEST_DIR` 上溯两级（仅开发构建有效）。
///
/// 第 2 条是给"解压即用"的发布包准备的：`CARGO_MANIFEST_DIR` 是构建机的路径，
/// 换一台电脑必然不存在；而发布包把 `engine/` 与 `runtime/` 放在可执行文件旁边。
pub fn repo_root() -> Option<PathBuf> {
    if let Some(raw) = std::env::var_os("OMNIDESK_REPO_ROOT") {
        if !raw.is_empty() {
            return Some(PathBuf::from(raw));
        }
    }
    if let Some(found) = repo_root_from_exe() {
        return Some(found);
    }
    let manifest = option_env!("CARGO_MANIFEST_DIR")?;
    PathBuf::from(manifest)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
}

fn repo_root_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;
    // 发布包：exe 在包根；macOS .app：Contents/MacOS → 上溯三级即包根；
    // 开发构建：desktop/src-tauri/target/release → 上溯五级即仓库根。
    for _ in 0..6 {
        if dir.join("engine").join("omnidesk_engine").is_dir()
            && dir.join("runtime").join("RUNTIME.lock").is_file()
        {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
    None
}

/// 用系统默认程序打开文件或目录。
pub fn open_target(target: &Path) -> Result<(), String> {
    open_str(&target.to_string_lossy())
}

/// 用系统默认程序打开链接。
pub fn open_url(url: &str) -> Result<(), String> {
    open_str(url)
}

fn open_str(target: &str) -> Result<(), String> {
    let mut child = spawn_opener(target)?;
    // 打开器存在但立刻失败（例如没有注册处理程序）时要让用户看到错误；
    // 短暂等待后仍在运行的视为已启动。
    let deadline = Instant::now() + Duration::from_millis(1200);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("系统打开器退出状态 {}", status)),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Ok(None) => return Ok(()),
            Err(e) => return Err(format!("无法确认打开结果（{}）", e)),
        }
    }
}

#[cfg(target_os = "macos")]
fn spawn_opener(target: &str) -> Result<Child, String> {
    Command::new("open")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("open：{}", e))
}

#[cfg(target_os = "windows")]
fn spawn_opener(target: &str) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("cmd");
    command.arg("/C");
    // `start` 的第一个带引号参数是窗口标题，必须给空串，否则带空格的路径会被当成标题。
    command.raw_arg(format!("start \"\" \"{}\"", target.replace('"', "")));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cmd /C start：{}", e))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_opener(target: &str) -> Result<Child, String> {
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<&str>::new()),
        ("gio", vec!["open"]),
    ] {
        match Command::new(program)
            .args(&args)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(last_error)
}


/// 解析 node 可执行文件的绝对路径。
///
/// 从 Finder/Launchpad 启动的 app 不继承 shell 的 PATH（homebrew 的
/// /opt/homebrew/bin 不在 GUI 进程的默认 PATH 里），直接 spawn "node" 会报
/// "No such file or directory (os error 2)"。这里按以下顺序解析：
///   1. OMNIDESK_NODE 环境变量（用户显式指定，优先级最高）；
///   2. PATH（命令行/终端启动时正常命中）；
///   3. 常见安装位置的探测（homebrew Apple Silicon / Intel、系统目录、nvm）。
/// 全部失败时返回带修复指引的错误。
pub fn resolve_node() -> Result<String, String> {
    if let Ok(value) = std::env::var("OMNIDESK_NODE") {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    // PATH 命中且能执行时直接用（保持旧行为）。
    if let Ok(output) = Command::new("node").arg("--version").output() {
        if output.status.success() {
            return Ok("node".to_string());
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = home_dir() {
        // nvm / fnm / volta 的默认 node 位置
        for pattern in [
            ".nvm/versions/node",
            ".volta/tools/image/node",
            ".local/share/fnm/node-versions",
        ] {
            let base = home.join(pattern);
            if let Ok(entries) = std::fs::read_dir(&base) {
                let mut versions: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .collect();
                versions.sort();
                if let Some(latest) = versions.pop() {
                    candidates.push(latest.join("bin/node"));
                }
            }
        }
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/node"));
    candidates.push(PathBuf::from("/usr/local/bin/node"));
    candidates.push(PathBuf::from("/usr/bin/node"));
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }
    Err(concat!(
        "未找到 node。OmniDesk 需要 Node.js 22+ 才能驱动 DSH 引擎。",
        "请安装 Node.js（https://nodejs.org），",
        "或在系统设置中为 OmniDesk 设置 OMNIDESK_NODE 环境变量指向 node 的绝对路径。"
    )
    .to_string())
}

// ---------------------------------------------------------------------------
// 进程核验后端（OD-ARCH-133）
//
// process_owner 需要知道三件事：本次开机是谁、某个 pid 的状态与进程组、某个
// 进程组里现在有哪些进程。Linux 直接读 procfs；macOS 没有 procfs，改用 ps 与
// sysctl 的等价输出。差异只留在这里，上层的「什么情况下可以杀」判定不分平台。
//
// 边界：只读查询，不发信号；ps 会 fork，调用点仅限启动回收 / 关闭兜底这种
// 低频路径，不进热路径。Windows 没有进程组语义（要用 Job Object，另立项），
// 这里统一返回「查不到」，让上层走「无法核验就不动」的保守分支。
// ---------------------------------------------------------------------------

/// 一条进程记录。state 取 ps / procfs 的状态字符，'Z' 表示僵尸。
#[derive(Debug, Clone)]
pub struct ProcRow {
    pub pid: i32,
    pub pgid: i32,
    pub state: char,
    /// 命令行。Linux 是 argv 数组；macOS 是整行参数串按空白切分。
    pub cmd: Vec<String>,
}

/// 本次内核启动的标识。跨平台不同源（Linux 是 UUID，macOS 是 darwin:<秒>），
/// 因此永远不会把一个平台的旧登记误认成本平台的。
#[cfg(target_os = "linux")]
pub fn boot_id() -> Option<String> {
    let raw = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(target_os = "macos")]
pub fn boot_id() -> Option<String> {
    let out = Command::new("/usr/sbin/sysctl")
        .arg("-n")
        .arg("kern.boottime")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_boottime_sec(&text).map(|sec| format!("darwin:{}", sec))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn boot_id() -> Option<String> {
    None
}

/// 从 { sec = 1793505130, usec = 109931 } Sun Nov  1 ... 里取整秒。
#[cfg(target_os = "macos")]
fn parse_boottime_sec(text: &str) -> Option<u64> {
    let start = text.find("sec")? + 3;
    let rest = text.get(start..)?;
    let eq = rest.find('=')? + 1;
    let digits: String = rest
        .get(eq..)?
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u64>().ok()
}

/// 进程状态字符与进程组。进程不存在 / 无权查看时返回 None。
#[cfg(target_os = "linux")]
pub fn process_stat(pid: i32) -> Option<(char, i32)> {
    if pid <= 0 {
        return None;
    }
    let raw = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let end = raw.rfind(')')?;
    let rest = raw.get(end + 1..)?.trim_start();
    let mut parts = rest.split_whitespace();
    let state = parts.next()?.chars().next()?;
    // rest 从 state 开始：state ppid pgrp ...
    let _ppid = parts.next()?;
    let pgrp: i32 = parts.next()?.parse().ok()?;
    Some((state, pgrp))
}

#[cfg(target_os = "macos")]
pub fn process_stat(pid: i32) -> Option<(char, i32)> {
    if pid <= 0 {
        return None;
    }
    let out = Command::new("/bin/ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("stat=,pgid=")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let mut parts = line.split_whitespace();
    let state = parts.next()?.chars().next()?;
    let pgid: i32 = parts.next()?.parse().ok()?;
    Some((state, pgid))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn process_stat(_pid: i32) -> Option<(char, i32)> {
    None
}

/// 命令行。macOS 没有 argv 数组，按空白切分（有损，因此不单独作为杀进程判据）。
#[cfg(target_os = "linux")]
pub fn process_cmd(pid: i32) -> Vec<String> {
    if pid <= 0 {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(format!("/proc/{}/cmdline", pid)) else {
        return Vec::new();
    };
    bytes
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect()
}

#[cfg(target_os = "macos")]
pub fn process_cmd(pid: i32) -> Vec<String> {
    if pid <= 0 {
        return Vec::new();
    }
    let out = Command::new("/bin/ps")
        .arg("-ww")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("args=")
        .output()
        .ok();
    let Some(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn process_cmd(_pid: i32) -> Vec<String> {
    Vec::new()
}

/// 完整的进程快照（含僵尸，由调用方过滤）。
#[cfg(target_os = "linux")]
pub fn process_table() -> Vec<ProcRow> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(pid) = name.parse::<i32>() else { continue };
        if let Some((state, pgid)) = process_stat(pid) {
            out.push(ProcRow {
                pid,
                pgid,
                state,
                cmd: process_cmd(pid),
            });
        }
    }
    out
}

#[cfg(target_os = "macos")]
pub fn process_table() -> Vec<ProcRow> {
    let mut out = Vec::new();
    let Ok(res) = Command::new("/bin/ps")
        .arg("-A")
        .arg("-ww")
        .arg("-o")
        .arg("pid=,pgid=,stat=,args=")
        .output()
    else {
        return out;
    };
    if !res.status.success() {
        return out;
    }
    for line in String::from_utf8_lossy(&res.stdout).lines() {
        let line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(4, char::is_whitespace);
        let (Some(pid), Some(pgid), Some(state)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(pid), Ok(pgid), Some(state)) = (
            pid.parse::<i32>(),
            pgid.parse::<i32>(),
            state.chars().next(),
        ) else {
            continue;
        };
        let cmd = parts
            .next()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        out.push(ProcRow {
            pid,
            pgid,
            state,
            cmd,
        });
    }
    out
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn process_table() -> Vec<ProcRow> {
    Vec::new()
}
