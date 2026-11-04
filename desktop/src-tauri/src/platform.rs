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

/// Windows 的「不创建控制台窗口」创建标志（OD-ARCH-134）。
///
/// OmniDesk 是 GUI 子系统程序，它拉起的 python / node / cmd 都是控制台子系统。
/// 不带这个标志时，Windows 会为每个子进程新开一个黑色控制台窗口；双击启动时
/// 就表现为「终端一闪而过」。
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 给即将 spawn 的命令加上「不弹控制台」，其余平台是空操作。
pub fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = command;
    command
}

/// Python 版本门槛探针：3.10 及以上退出码 0。
const PY_VERSION_PROBE: &str = "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)";
/// 取解释器版本号（major.minor.patch）。
const PY_VERSION_PRINT: &str = "import sys; print('%d.%d.%d' % sys.version_info[:3])";

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
    if python_is_supported(Path::new(&fallback)) {
        return Ok(fallback);
    }
    // PATH 里的解释器版本过低、或 GUI 启动根本没有 PATH 时，按平台候选顺序找
    // 第一个 3.10+ 的解释器（OD-ARCH-134：Windows 补 python.org 安装位置与 py 启动器）。
    for candidate in python_candidates() {
        if python_is_supported(&candidate) {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }
    Err(concat!(
        "未找到 Python 3.10+。OmniDesk 的引擎需要 Python 3.10 或更高版本。",
        "请安装 Python（https://www.python.org/downloads/），",
        "或设置 OMNIDESK_PYTHON 指向解释器的绝对路径。"
    )
    .to_string())
}

/// 解释器能不能用：只问「版本够不够」，不解析输出。
fn python_is_supported(program: &Path) -> bool {
    let mut cmd = Command::new(program);
    cmd.arg("-c").arg(PY_VERSION_PROBE);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// 候选解释器路径，按平台给出。调用方逐个 probe 版本。
fn python_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    #[cfg(not(target_os = "windows"))]
    {
        out.push(PathBuf::from("/usr/bin/python3"));
        out.push(PathBuf::from("/opt/homebrew/bin/python3"));
        out.push(PathBuf::from("/usr/local/bin/python3"));
        // homebrew 安装根下的具体版本目录（python@3.12/3.13 等）也参与候选。
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
        out.extend(versioned.into_iter().rev());
    }
    #[cfg(target_os = "windows")]
    {
        // python.org 安装器的两种落点：按用户安装（LOCALAPPDATA）与全机安装。
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            out.extend(python_dirs_under(
                &PathBuf::from(local).join("Programs").join("Python"),
            ));
        }
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            out.extend(python_dirs_under(&PathBuf::from(pf)));
        }
        // 最后问官方 py 启动器：`py -3` 能直接报出真实解释器路径。
        if let Some(path) = python_from_launcher() {
            out.push(path);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 运行时依赖探测（OD-ARCH-134）
//
// 发布包不自带 node / python。缺依赖时引擎起不来，而失败只写在日志里，用户
// 双击后看到的是「闪一下就没」。这里把「找得到吗、版本够不够」变成一次可展示
// 的事实，界面据此给出安装入口，而不是让用户猜。
// ---------------------------------------------------------------------------

/// 一项运行时依赖的探测结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DependencyStatus {
    pub name: &'static str,
    /// 找到且版本达标。
    pub ok: bool,
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub requirement: &'static str,
    pub hint: &'static str,
    pub download: &'static str,
}

/// 引擎启动的两个前置依赖。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnvironmentReport {
    pub ok: bool,
    pub node: DependencyStatus,
    pub python: DependencyStatus,
    /// 安装完整性：engine/ 与 runtime/ 是否随包就位（OD-ARCH-135）。
    pub assets: DependencyStatus,
}

/// 探测 node / python：只读，不安装、不改环境。
pub fn environment_report() -> EnvironmentReport {
    let node = match resolve_node() {
        Ok(path) => {
            let version = run_version(&path, &["--version"]);
            let ok = version
                .as_deref()
                .and_then(node_major)
                .map(|major| major >= 22)
                .unwrap_or(false);
            DependencyStatus {
                name: "node",
                ok,
                found: true,
                path: Some(path),
                version,
                requirement: "Node.js 22+",
                hint: "版本低于 22 时 DSH 底座无法运行。装好新版后重启 OmniDesk；也可以设置 OMNIDESK_NODE 指向 node 的绝对路径。",
                download: "https://nodejs.org",
            }
        }
        Err(_) => DependencyStatus {
            name: "node",
            ok: false,
            found: false,
            path: None,
            version: None,
            requirement: "Node.js 22+",
            hint: "安装 Node.js 22 或更高版本后重启 OmniDesk；也可以设置 OMNIDESK_NODE 指向 node 的绝对路径。",
            download: "https://nodejs.org",
        },
    };
    let python = match resolve_python() {
        Ok(path) => {
            let version = run_version(&path, &["-c", PY_VERSION_PRINT]);
            let ok = version
                .as_deref()
                .and_then(python_major_minor)
                .map(|(major, minor)| (major, minor) >= (3, 10))
                .unwrap_or(false);
            DependencyStatus {
                name: "python",
                ok,
                found: true,
                path: Some(path),
                version,
                requirement: "Python 3.10+",
                hint: "版本低于 3.10 时引擎无法启动。装好新版后重启 OmniDesk；也可以设置 OMNIDESK_PYTHON 指向解释器的绝对路径。",
                download: "https://www.python.org/downloads/",
            }
        }
        Err(_) => DependencyStatus {
            name: "python",
            ok: false,
            found: false,
            path: None,
            version: None,
            requirement: "Python 3.10+",
            hint: "安装 Python 3.10 或更高版本后重启 OmniDesk；也可以设置 OMNIDESK_PYTHON 指向解释器的绝对路径。",
            download: "https://www.python.org/downloads/",
        },
    };
    // 安装完整性：Windows 安装器曾只装 exe，引擎目录根本不存在。这里把
    // 「包是不是完整的」也作为一项前置条件，缺了就让用户重装，而不是看着
    // 窗口发呆。仓库开发态天然命中，不影响本机构建。
    let assets = match repo_root() {
        Some(root)
            if root.join("engine").join("omnidesk_engine").is_dir()
                && root.join("runtime").join("RUNTIME.lock").is_file() =>
        {
            DependencyStatus {
                name: "assets",
                ok: true,
                found: true,
                path: Some(root.display().to_string()),
                version: None,
                requirement: "运行时目录",
                hint: "",
                download: "https://github.com/klzone/omniharness/releases",
            }
        }
        found_root => DependencyStatus {
            name: "assets",
            ok: false,
            found: found_root.is_some(),
            path: found_root.map(|root| root.display().to_string()),
            version: None,
            requirement: "运行时目录",
            hint: "安装不完整：缺少 engine/ 或 runtime/。重新运行安装程序即可恢复，已保存的对话与设置不受影响。",
            download: "https://github.com/klzone/omniharness/releases",
        },
    };
    EnvironmentReport {
        ok: node.ok && python.ok && assets.ok,
        node,
        python,
        assets,
    }
}

/// 运行一个只读版本命令，返回去掉首尾空白的 stdout。失败返回 None。
fn run_version(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// `v22.19.0` → 22。
fn node_major(version: &str) -> Option<u32> {
    version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// `3.12.4` → (3, 12)。
fn python_major_minor(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// 在给定目录下找 `Python3*` 子目录里的 python.exe（Windows 专用）。
#[cfg(target_os = "windows")]
fn python_dirs_under(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Python3") {
                let exe = entry.path().join("python.exe");
                if exe.is_file() {
                    found.push(exe);
                }
            }
        }
    }
    // 目录名里带版本号，倒序即新版本优先。
    found.sort();
    found.reverse();
    found
}

/// 用官方启动器 `py -3` 问出解释器绝对路径（Windows 专用）。
#[cfg(target_os = "windows")]
fn python_from_launcher() -> Option<PathBuf> {
    let mut cmd = Command::new("py");
    cmd.args(["-3", "-c", "import sys; print(sys.executable)"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
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
    // 编译期路径只对开发构建有意义。发布包落到这里说明运行时没找到，
    // 宁可返回 None 让上层报「安装不完整」，也不要把构建机的路径
    // （/Users/runner/work/...）显示给用户（OD-ARCH-136）。
    let manifest = option_env!("CARGO_MANIFEST_DIR")?;
    let candidate = PathBuf::from(manifest)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())?;
    if looks_like_bundle_root(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

/// 某个目录像不像「带运行时的安装根」：引擎包与底座版本锁同时存在。
fn looks_like_bundle_root(dir: &Path) -> bool {
    dir.join("engine").join("omnidesk_engine").is_dir()
        && dir.join("runtime").join("RUNTIME.lock").is_file()
}

fn repo_root_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    find_bundle_root(exe.parent()?)
}

/// 从某个目录（通常是可执行文件所在目录）向上找「带运行时的安装根」。
///
/// 两条布局都要认：
///   * 免安装包 / Windows 安装：`engine/` 与 `runtime/` 与可执行文件同级；
///   * macOS `.app`：资源在 `Contents/Resources/`（OD-ARCH-136）。
fn find_bundle_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    // 发布包：exe 在包根；macOS .app：Contents/MacOS → 上溯三级即包根；
    // 开发构建：desktop/src-tauri/target/release → 上溯五级即仓库根。
    for _ in 0..7 {
        if looks_like_bundle_root(dir) {
            return Some(dir.to_path_buf());
        }
        // macOS .app 的资源在 Contents/Resources（OD-ARCH-136）：包里自带运行时后，
        // 应用可以直接拖进 /Applications，不再依赖同目录的 engine/runtime。
        if let Some(resources) = dir.parent().map(|p| p.join("Resources")) {
            if looks_like_bundle_root(&resources) {
                return Some(resources);
            }
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
    // 不加这个标志，双击启动时会跟着弹一个黑框（OD-ARCH-134）。
    hide_console(&mut command);
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
    let mut probe = Command::new("node");
    probe.arg("--version");
    hide_console(&mut probe);
    if let Ok(output) = probe.output() {
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
    #[cfg(not(target_os = "windows"))]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/node"));
        candidates.push(PathBuf::from("/usr/local/bin/node"));
        candidates.push(PathBuf::from("/usr/bin/node"));
    }
    #[cfg(target_os = "windows")]
    candidates.extend(windows_node_candidates());
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

/// Windows 上的 node 安装位置（OD-ARCH-134）。
///
/// 双击启动时 GUI 进程的 PATH 里通常没有 node：官方安装器写的是机器级 PATH，
/// 而 nvm-windows / scoop 只改用户 PATH。这里直接按常见落点找可执行文件。
#[cfg(target_os = "windows")]
fn windows_node_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(key) {
            out.push(PathBuf::from(base).join("nodejs").join("node.exe"));
        }
    }
    // nvm-windows：%APPDATA%\nvm\v20.11.1\node.exe，版本目录倒序取最新。
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let base = PathBuf::from(appdata).join("nvm");
        if let Ok(entries) = std::fs::read_dir(&base) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().starts_with('v'))
                        .unwrap_or(false)
                })
                .collect();
            versions.sort();
            out.extend(versions.into_iter().rev().map(|dir| dir.join("node.exe")));
        }
    }
    // scoop 与「官方 zip 解压到用户目录」的常见形态。
    if let Some(home) = home_dir() {
        out.push(
            home.join("scoop")
                .join("apps")
                .join("nodejs")
                .join("current")
                .join("node.exe"),
        );
        out.push(home.join("nodejs").join("node.exe"));
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 版本解析只认「主版本」，前缀 v 可有可无（node 输出 `v22.19.0`，有的发行版不给 v）。
    #[test]
    fn node_major_parses_common_forms() {
        assert_eq!(node_major("v22.19.0"), Some(22));
        assert_eq!(node_major("20.11.1"), Some(20));
        assert_eq!(node_major("  24.0.0\n"), Some(24));
        assert_eq!(node_major(""), None);
        assert_eq!(node_major("not-a-version"), None);
    }

    #[test]
    fn python_major_minor_parses_common_forms() {
        assert_eq!(python_major_minor("3.12.4"), Some((3, 12)));
        assert_eq!(python_major_minor("3.9.6"), Some((3, 9)));
        assert_eq!(python_major_minor("3"), None);
        assert_eq!(python_major_minor("garbage"), None);
    }

    /// 防闪窗只是加一个 Windows 创建标志：程序名与参数必须原样保留。
    #[test]
    fn hide_console_keeps_program_and_args() {
        let mut cmd = Command::new("omnidesk-nonexistent-probe");
        cmd.arg("--version").arg("a b");
        let returned = hide_console(&mut cmd);
        assert_eq!(returned.get_program(), "omnidesk-nonexistent-probe");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["--version".to_string(), "a b".to_string()]);
    }

    /// 版本命令跑不起来时必须返回 None，而不是 panic 或空串。
    #[test]
    fn run_version_returns_none_for_missing_program() {
        assert_eq!(run_version("omnidesk-nonexistent-probe", &["--version"]), None);
    }

    /// 体检结果必须自洽：ok 蕴含 found；缺依赖时仍带下载页与修复提示。
    #[test]
    fn environment_report_is_consistent() {
        // 体检读进程环境（OMNIDESK_REPO_ROOT / OMNIDESK_NODE…），与其它写环境的
        // 用例共用一把锁，否则并行时会读到别人的临时根（OD-ARCH-135）。
        let _guard = crate::test_env_lock::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let report = environment_report();
        for item in [&report.node, &report.python] {
            assert!(item.found || !item.ok, "{}：ok 不能先于 found", item.name);
            assert!(item.requirement.contains("3.10") || item.requirement.contains("22"));
            assert!(item.download.starts_with("https://"));
            assert!(!item.hint.is_empty());
            if !item.ok {
                assert!(item.hint.contains("OMNIDESK_"), "{}：缺依赖时要给可操作的修复路径", item.name);
            }
        }
        assert_eq!(
            report.ok,
            report.node.ok && report.python.ok && report.assets.ok
        );
    }

    /// 安装完整性是第三项：仓库开发态天然带 engine/ 与 runtime/RUNTIME.lock，
    /// 因此这里必须判为完整；整体 ok 要同时看三项。
    #[test]
    fn environment_report_includes_install_assets() {
        let _guard = crate::test_env_lock::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // 明确指向仓库根：既不依赖宿主环境，也不受其它用例残留影响。
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("仓库根")
            .to_path_buf();
        let previous = std::env::var_os("OMNIDESK_REPO_ROOT");
        std::env::set_var("OMNIDESK_REPO_ROOT", &repo);
        let report = environment_report();
        match previous {
            Some(value) => std::env::set_var("OMNIDESK_REPO_ROOT", value),
            None => std::env::remove_var("OMNIDESK_REPO_ROOT"),
        }
        assert_eq!(report.assets.name, "assets");
        assert!(report.assets.ok, "仓库根应包含 engine/ 与 runtime/RUNTIME.lock");
        assert_eq!(
            report.ok,
            report.node.ok && report.python.ok && report.assets.ok
        );
    }

    /// 造一个「带运行时」的目录骨架。
    fn make_bundle(root: &Path) {
        std::fs::create_dir_all(root.join("engine").join("omnidesk_engine")).unwrap();
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        std::fs::write(root.join("runtime").join("RUNTIME.lock"), "runtime:\n").unwrap();
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omnidesk-bundle-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Windows 安装版 / 免安装包：运行时与可执行文件同级。
    #[test]
    fn find_bundle_root_accepts_portable_layout() {
        let root = scratch("portable");
        make_bundle(&root);
        assert_eq!(find_bundle_root(&root), Some(root.clone()));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// macOS .app：资源在 Contents/Resources，可执行文件在 Contents/MacOS。
    #[test]
    fn find_bundle_root_accepts_app_resources_layout() {
        let root = scratch("app");
        let macos = root.join("OmniDesk.app").join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        make_bundle(&root.join("OmniDesk.app").join("Contents").join("Resources"));
        assert_eq!(
            find_bundle_root(&macos),
            Some(root.join("OmniDesk.app").join("Contents").join("Resources"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 只有目录、没有 RUNTIME.lock 的空壳不算安装根：宁可报「不完整」。
    #[test]
    fn find_bundle_root_rejects_incomplete_layout() {
        let root = scratch("empty");
        std::fs::create_dir_all(root.join("engine").join("omnidesk_engine")).unwrap();
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        assert_eq!(find_bundle_root(&root), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
