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
