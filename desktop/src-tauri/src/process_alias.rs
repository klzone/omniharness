//! Windows 进程名归一：用副本名启动 node / python（OD-PROC-149a）。
//!
//! Windows 的镜像名由可执行文件决定，运行期改不了；任务管理器只看镜像名。
//! 因此把解析到的 `node.exe` / `python.exe` 复制成 `omnidesk-node.exe` /
//! `omnidesk-python.exe`，引擎链就统一落在 `omnidesk-*` 名字下。
//!
//! 边界：
//!   * 只在 Windows 生效；其它平台 [select_for_engine] 直接透传原名，零副作用。
//!   * 不改 Job Object / 回收逻辑；副本只是换个启动路径。
//!   * 不打包解释器，源文件仍是用户自己的 node / python。
//!   * 用户显式指定 `OMNIDESK_NODE` / `OMNIDESK_PYTHON` 时不复制、不接管。
//!   * 任何 IO 失败或探针失败都回退原名；副本只是显示层优化，不能拦住启动。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

/// 回退开关：`OMNIDESK_PROC_ALIAS=off`（或 false / 0）时完全跳过副本。
pub const SWITCH_ENV: &str = "OMNIDESK_PROC_ALIAS";
pub const PYTHON_ALIAS_FILE: &str = "omnidesk-python.exe";
pub const NODE_ALIAS_FILE: &str = "omnidesk-node.exe";

/// 副本探针脚本。
///
/// 比 `import sys` 多要一个 `encodings`：`sys` 是内建模块，单独导入成功
/// 并不能证明副本还能找到标准库；`encodings` 是解释器启动就要用的标准库包。
const PYTHON_PROBE: &str = "import sys, encodings; print(sys.prefix)";

/// 一次启动选择的结果：用哪个解释器、给子进程补哪些环境变量。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub python: String,
    pub node: Option<String>,
    /// 需要覆盖给引擎子进程的环境变量（PATH 前缀、OMNIDESK_NODE）。
    pub env: Vec<(String, String)>,
    /// 便于日志与测试的可读说明（不含凭证）。
    pub notes: Vec<String>,
}

impl Selection {
    fn passthrough(python: &str) -> Self {
        Self {
            python: python.to_string(),
            ..Self::default()
        }
    }
}

/// python 副本能不能跑的探针；测试注入假实现，避免依赖真实安装。
pub trait PythonProbe {
    fn import_sys(&self, program: &Path, path_prefix: Option<&Path>) -> bool;
}

struct RealProbe;

impl PythonProbe for RealProbe {
    fn import_sys(&self, program: &Path, path_prefix: Option<&Path>) -> bool {
        let mut cmd = Command::new(program);
        cmd.arg("-c").arg(PYTHON_PROBE);
        // 副本和源解释器不在同一目录：DLL 与标准库都靠 PATH 找，探针要
        // 和运行时用同一份 PATH，否则探针通过、真启动却失败。
        if let Some(dir) = path_prefix {
            cmd.env("PATH", prefixed_path(dir));
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::platform::hide_console(&mut cmd);
        matches!(cmd.status(), Ok(status) if status.success())
    }
}

/// 引擎启动前的选择入口：Windows 下准备副本，其它平台原样透传。
///
/// `python` 是 [crate::platform::resolve_python] 的结果（原名或绝对路径）。
/// node 在这里才解析：`from_env` 被设置页等只读路径频繁调用，不该每次都探测 node。
pub fn select_for_engine(data_root: &Path, python: &str) -> Selection {
    if !cfg!(windows) {
        return Selection::passthrough(python);
    }
    if switch_is_off(std::env::var(SWITCH_ENV).ok().as_deref()) {
        let mut selection = Selection::passthrough(python);
        note(&mut selection, "OMNIDESK_PROC_ALIAS 已关闭，引擎使用原解释器名");
        log_line(data_root, "副本已跳过（OMNIDESK_PROC_ALIAS 关闭）");
        return selection;
    }
    let node = crate::platform::resolve_node().ok();
    let selection = select_with(
        data_root,
        python,
        node.as_deref(),
        env_is_set("OMNIDESK_PYTHON"),
        env_is_set("OMNIDESK_NODE"),
        true,
        &RealProbe,
    );
    for line in &selection.notes {
        log_line(data_root, line);
    }
    selection
}

/// 纯逻辑主体：`windows` 与探针都可注入，单测在任意平台都能覆盖。
pub(crate) fn select_with(
    data_root: &Path,
    python: &str,
    node: Option<&str>,
    python_user_managed: bool,
    node_user_managed: bool,
    windows: bool,
    probe: &dyn PythonProbe,
) -> Selection {
    let mut selection = Selection::passthrough(python);
    if !windows {
        return selection;
    }
    let bin_dir = alias_dir(data_root);

    if python_user_managed {
        note(
            &mut selection,
            "OMNIDESK_PYTHON 已显式指定，python 保持原名（不复制副本）",
        );
    } else {
        match select_python(&bin_dir, python, probe) {
            Ok((target, prefix)) => {
                note(
                    &mut selection,
                    &format!("python 使用副本 {}（源 {python}）", target.display()),
                );
                selection
                    .env
                    .push(("PATH".to_string(), prefixed_path(&prefix)));
                selection.python = target.display().to_string();
            }
            Err(reason) => note(&mut selection, &format!("python 副本不可用，回退原名：{reason}")),
        }
    }

    if node_user_managed {
        note(
            &mut selection,
            "OMNIDESK_NODE 已显式指定，node 保持原名（不复制副本）",
        );
    } else if let Some(node) = node {
        match select_node(&bin_dir, node) {
            Ok(target) => {
                note(
                    &mut selection,
                    &format!("node 使用副本 {}（源 {node}）", target.display()),
                );
                selection.env.push((
                    "OMNIDESK_NODE".to_string(),
                    target.display().to_string(),
                ));
                selection.node = Some(target.display().to_string());
            }
            Err(reason) => note(&mut selection, &format!("node 副本不可用，保持原名：{reason}")),
        }
    }

    selection
}

fn select_python(
    bin_dir: &Path,
    python: &str,
    probe: &dyn PythonProbe,
) -> Result<(PathBuf, PathBuf), String> {
    let source = absolute_program(python)
        .ok_or_else(|| format!("无法解析解释器的绝对路径（{python}）"))?;
    let prefix = source
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("解释器路径没有父目录（{}）", source.display()))?;
    let target = bin_dir.join(PYTHON_ALIAS_FILE);
    refresh_copy(&source, &target)?;
    if !probe.import_sys(&target, Some(&prefix)) {
        // 探针失败：删掉副本，下一次启动会重新尝试，行为回到原名。
        remove_alias(&target);
        return Err(format!("副本探针未通过（{}）", target.display()));
    }
    Ok((target, prefix))
}

fn select_node(bin_dir: &Path, node: &str) -> Result<PathBuf, String> {
    let source = absolute_program(node)
        .ok_or_else(|| format!("无法解析 node 的绝对路径（{node}）"))?;
    let target = bin_dir.join(NODE_ALIAS_FILE);
    refresh_copy(&source, &target)?;
    Ok(target)
}

/// 副本目录：`<data_root>/runtime/bin/`。
pub fn alias_dir(data_root: &Path) -> PathBuf {
    data_root.join("runtime").join("bin")
}

fn marker_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(".json");
    PathBuf::from(name)
}

struct Stamp {
    len: u64,
    secs: u64,
    nanos: u32,
}

fn stamp_of(path: &Path) -> Result<Stamp, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("读取源文件信息失败（{}）：{}", path.display(), e))?;
    let modified = meta
        .modified()
        .map_err(|e| format!("读取源文件修改时间失败（{}）：{}", path.display(), e))?;
    let since = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("源文件修改时间早于纪元（{}）：{}", path.display(), e))?;
    Ok(Stamp {
        len: meta.len(),
        secs: since.as_secs(),
        nanos: since.subsec_nanos(),
    })
}

/// 目标缺失、或 marker 记录的「源路径 + len + mtime」与现状不一致时重新复制。
fn refresh_copy(source: &Path, target: &Path) -> Result<(), String> {
    let stamp = stamp_of(source)?;
    let marker = marker_path(target);
    if target.is_file() && marker_matches(&marker, source, &stamp) {
        return Ok(());
    }
    let dir = target
        .parent()
        .ok_or_else(|| format!("副本目标没有父目录（{}）", target.display()))?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("创建副本目录失败（{}）：{}", dir.display(), e))?;
    let staging = target.with_extension("tmp");
    std::fs::copy(source, &staging).map_err(|e| {
        let _ = std::fs::remove_file(&staging);
        format!(
            "复制 {} → {} 失败：{}",
            source.display(),
            staging.display(),
            e
        )
    })?;
    // Windows 上 rename 不能覆盖已存在的文件；单实例场景，先删再换即可。
    if target.exists() {
        std::fs::remove_file(target)
            .map_err(|e| format!("替换旧副本失败（{}）：{}", target.display(), e))?;
    }
    std::fs::rename(&staging, target).map_err(|e| {
        let _ = std::fs::remove_file(&staging);
        format!("落位副本失败（{}）：{}", target.display(), e)
    })?;
    let marker_body = serde_json::json!({
        "source": source.display().to_string(),
        "alias": target.display().to_string(),
        "len": stamp.len,
        "mtime_secs": stamp.secs,
        "mtime_nanos": stamp.nanos,
    });
    std::fs::write(&marker, format!("{marker_body}\n"))
        .map_err(|e| format!("写副本标记失败（{}）：{}", marker.display(), e))?;
    Ok(())
}

fn marker_matches(marker: &Path, source: &Path, stamp: &Stamp) -> bool {
    let Ok(raw) = std::fs::read_to_string(marker) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let recorded_source = value.get("source").and_then(|v| v.as_str()).unwrap_or("");
    Path::new(recorded_source) == source
        && value.get("len").and_then(|v| v.as_u64()) == Some(stamp.len)
        && value.get("mtime_secs").and_then(|v| v.as_u64()) == Some(stamp.secs)
        && value.get("mtime_nanos").and_then(|v| v.as_u64()) == Some(u64::from(stamp.nanos))
}

fn remove_alias(target: &Path) {
    let _ = std::fs::remove_file(target);
    let _ = std::fs::remove_file(marker_path(target));
}

/// 把程序名解析成绝对路径：绝对路径直接用，裸名字搜索 PATH。
fn absolute_program(program: &str) -> Option<PathBuf> {
    let trimmed = program.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return path.is_file().then_some(path);
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        let absolute = std::env::current_dir().ok()?.join(&path);
        return absolute.is_file().then_some(absolute);
    }
    let mut names = vec![trimmed.to_string()];
    if cfg!(windows) {
        names.push(format!("{trimmed}.exe"));
    }
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 子进程 PATH：副本所在目录在前，原解释器目录紧随其后。
fn prefixed_path(dir: &Path) -> String {
    let separator = if cfg!(windows) { ';' } else { ':' };
    match std::env::var("PATH") {
        Ok(existing) if !existing.trim().is_empty() => {
            format!("{}{separator}{existing}", dir.display())
        }
        _ => dir.display().to_string(),
    }
}

fn env_is_set(key: &str) -> bool {
    std::env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn switch_is_off(raw: Option<&str>) -> bool {
    raw.map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "off" | "false" | "0" | "no"
        )
    })
    .unwrap_or(false)
}

fn note(selection: &mut Selection, line: &str) {
    selection.notes.push(line.to_string());
}

/// 诊断日志：stderr + `<data_root>/logs/process-alias.log`，写不进去不影响启动。
fn log_line(data_root: &Path, line: &str) {
    eprintln!("[OmniDesk][process-alias] {line}");
    let dir = data_root.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("process-alias.log"))
    {
        use std::io::Write;
        let stamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        let _ = writeln!(file, "{stamp} {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    struct FakeProbe {
        ok: bool,
        seen: Mutex<Vec<(PathBuf, Option<PathBuf>)>>,
    }

    impl FakeProbe {
        fn new(ok: bool) -> Self {
            Self {
                ok,
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    impl PythonProbe for FakeProbe {
        fn import_sys(&self, program: &Path, path_prefix: Option<&Path>) -> bool {
            self.seen.lock().unwrap().push((
                program.to_path_buf(),
                path_prefix.map(Path::to_path_buf),
            ));
            self.ok
        }
    }

    fn fixture(tag: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "od149a-{tag}-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_source(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn copies_missing_target_and_records_marker() {
        let dir = fixture("copy");
        let source = write_source(&dir, "python.exe", b"fake-python-v1");
        let probe = FakeProbe::new(true);
        let selection = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        let target = alias_dir(&dir).join(PYTHON_ALIAS_FILE);
        assert_eq!(selection.python, target.display().to_string());
        assert_eq!(std::fs::read(&target).unwrap(), b"fake-python-v1");
        let marker = std::fs::read_to_string(marker_path(&target)).unwrap();
        let recorded: serde_json::Value = serde_json::from_str(&marker).unwrap();
        assert_eq!(recorded["source"].as_str(), source.to_str());
        assert_eq!(recorded["len"].as_u64(), Some(14), "标记内容：{marker}");
        let path_entry = selection
            .env
            .iter()
            .find(|(key, _)| key == "PATH")
            .expect("python 副本必须补 PATH");
        assert!(path_entry.1.starts_with(&dir.display().to_string()));
        assert_eq!(
            probe.seen.lock().unwrap().first(),
            Some(&(target.clone(), Some(dir.clone())))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refreshes_when_source_len_changes() {
        let dir = fixture("refresh");
        let source = write_source(&dir, "python.exe", b"v1");
        let probe = FakeProbe::new(true);
        let first = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        let target = PathBuf::from(&first.python);
        assert_eq!(std::fs::read(&target).unwrap(), b"v1");
        // 同长度也要刷新：这里先改内容再补一个字节，确保 len 与 mtime 都变。
        std::fs::write(&source, b"v2-longer").unwrap();
        let _ = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"v2-longer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refreshes_when_marker_mismatches() {
        let dir = fixture("marker");
        let source = write_source(&dir, "python.exe", b"body-1");
        let probe = FakeProbe::new(true);
        let first = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        let target = PathBuf::from(&first.python);
        // 手改副本与标记，模拟「标记说一致、文件其实旧了」。
        std::fs::write(&target, b"stale").unwrap();
        let marker = marker_path(&target);
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
        value["len"] = serde_json::json!(1);
        std::fs::write(&marker, value.to_string()).unwrap();
        let _ = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"body-1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_failure_removes_copy_and_falls_back() {
        let dir = fixture("probe");
        let source = write_source(&dir, "python.exe", b"fake-python");
        let probe = FakeProbe::new(false);
        let selection = select_with(
            &dir,
            source.to_str().unwrap(),
            None,
            false,
            false,
            true,
            &probe,
        );
        assert_eq!(selection.python, source.display().to_string());
        let target = alias_dir(&dir).join(PYTHON_ALIAS_FILE);
        assert!(!target.exists(), "探针失败必须删掉副本");
        assert!(!marker_path(&target).exists(), "探针失败必须删掉标记");
        assert!(selection.env.iter().all(|(key, _)| key != "PATH"));
        assert!(selection.notes.iter().any(|line| line.contains("回退原名")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_managed_paths_are_not_copied() {
        let dir = fixture("user");
        let source = write_source(&dir, "python.exe", b"fake-python");
        let node = write_source(&dir, "node.exe", b"fake-node");
        let probe = FakeProbe::new(true);
        let selection = select_with(
            &dir,
            source.to_str().unwrap(),
            Some(node.to_str().unwrap()),
            true,
            true,
            true,
            &probe,
        );
        assert_eq!(selection.python, source.display().to_string());
        assert_eq!(selection.node, None);
        assert!(selection.env.is_empty());
        assert!(!alias_dir(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_windows_is_passthrough() {
        let dir = fixture("passthrough");
        let source = write_source(&dir, "python.exe", b"fake-python");
        let probe = FakeProbe::new(true);
        let selection = select_with(
            &dir,
            source.to_str().unwrap(),
            Some("node"),
            false,
            false,
            false,
            &probe,
        );
        assert_eq!(selection.python, source.display().to_string());
        assert!(selection.env.is_empty());
        assert!(selection.notes.is_empty());
        assert!(probe.seen.lock().unwrap().is_empty());
        assert!(!alias_dir(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_copy_sets_env_override() {
        let dir = fixture("node");
        let node = write_source(&dir, "node.exe", b"fake-node");
        let probe = FakeProbe::new(true);
        let selection = select_with(
            &dir,
            "python-not-touched",
            Some(node.to_str().unwrap()),
            true,
            false,
            true,
            &probe,
        );
        let target = alias_dir(&dir).join(NODE_ALIAS_FILE);
        assert_eq!(std::fs::read(&target).unwrap(), b"fake-node");
        assert_eq!(selection.node, Some(target.display().to_string()));
        assert!(selection
            .env
            .iter()
            .any(|(key, value)| key == "OMNIDESK_NODE" && *value == target.display().to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switch_values() {
        for off in ["off", "OFF", " false ", "0", "no"] {
            assert!(switch_is_off(Some(off)), "{off} 应视为关闭");
        }
        for on in ["on", "1", "yes", ""] {
            assert!(!switch_is_off(Some(on)), "{on} 不应视为关闭");
        }
        assert!(!switch_is_off(None));
    }
}
