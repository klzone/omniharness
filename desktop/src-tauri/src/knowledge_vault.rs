//! 知识库只读访问（OD-KB-001 P1）。
//!
//! 边界（由代码强制，不是提示词约定）：
//!   * 根必须是用户显式选择的、已存在的绝对目录；不在仓库、应用数据根或
//!     DSH / agent / Codex 私有目录内，真实落点同样受检；
//!   * 扫描只读：只做 read_dir / metadata，不建立持久索引，不写任何文件；
//!   * 不跟随符号链接，隐藏目录与明确排除目录整棵跳过并单独计数；
//!   * 读取只接受根内相对路径，拒绝绝对路径、\`..\`、任一层的符号链接与非普通文件；
//!   * 笔记只接受 UTF-8 Markdown/纯文本，单文件不超过 512 KiB；附件只列元数据；
//!   * 每条跳过与失败都有可解释原因，超出记录上限时明确标记截断。
//!
//! 与 \`library_root\` 无关：知识库根不参与工作记录、周报、成果与 @ 引用解析。

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::app_paths::resolve_real_path;
use crate::task_shell::{assert_local_root_allowed, assert_outside_repo, MATERIAL_MAX_BYTES};

/// 扫描深度上限（根为 0）。
pub const VAULT_MAX_DEPTH: usize = 12;
/// 单次扫描最多读取的目录条目数（含被跳过的），防止异常库拖垮界面。
pub const VAULT_MAX_VISITED: usize = 20_000;
/// 单次扫描最多返回的文件条目数。
pub const VAULT_MAX_FILES: usize = 2_000;
/// 跳过项与错误项的记录上限（计数不受此限制）。
pub const VAULT_MAX_DETAILS: usize = 200;
/// 笔记正文读取上限，沿用材料读取上限，不做静默截断。
pub const VAULT_MAX_NOTE_BYTES: u64 = MATERIAL_MAX_BYTES;

/// 可作为笔记正文读取的扩展名。
pub const VAULT_NOTE_EXTS: &[&str] = &["md", "markdown", "txt"];

/// 明确排除的目录名（除所有 \`.\` 开头的隐藏目录外）。
pub const VAULT_EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "venv",
    ".venv",
    "__pycache__",
    ".obsidian",
    ".trash",
    ".git",
];

/// 禁止作为知识库根的位置片段（与既有本地数据边界同一口径）。
const FORBIDDEN_ROOT_FRAGMENTS: &[&str] = &[
    "/.dsh",
    "/.agents",
    "/.codex",
    "/.config/dsh desktop",
    "/.local/share/io.github.hairyf.deepseek-harness-desktop",
    "deepseek-harness",
    "/myworkstation",
    "omniharness",
];

/// 一个可展示的文件条目（不含正文）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultFile {
    /// 相对根的 POSIX 风格路径，读取时原样传回。
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub bytes: u64,
    /// 最后修改时间（Unix 秒）；取不到时为 None。
    pub modified: Option<u64>,
    /// note = 可读正文；attachment = 只列元数据。
    pub kind: &'static str,
}

/// 被跳过的条目（原因固定，便于界面聚合）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkippedItem {
    pub relative_path: String,
    pub reason: &'static str,
}

/// 单项失败（读取目录或元数据失败）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultError {
    pub relative_path: String,
    pub message: String,
}

/// 扫描统计。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ScanStats {
    pub visited_entries: usize,
    pub visited_dirs: usize,
    pub files: usize,
    pub notes: usize,
    pub attachments: usize,
    pub total_bytes: u64,
    pub skipped_hidden: usize,
    pub skipped_excluded: usize,
    pub skipped_symlink: usize,
    pub skipped_non_file: usize,
    /// 超过深度上限而被跳过的目录（只跳过该子树，不中止整次扫描）。
    pub skipped_depth: usize,
    pub errors: usize,
    /// 是否因为上限而提前停止。
    pub truncated: bool,
    /// 跳过/错误明细是否被截断记录。
    pub details_truncated: bool,
}

/// 一次扫描的完整结果（只在内存，不落盘）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultScan {
    pub root: String,
    pub files: Vec<VaultFile>,
    pub skipped: Vec<SkippedItem>,
    pub errors: Vec<VaultError>,
    pub stats: ScanStats,
}

/// 读取一个笔记文件的结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultNote {
    pub relative_path: String,
    pub file_name: String,
    pub bytes: u64,
    pub chars: usize,
    pub extension: String,
    pub sha256: String,
    pub content: String,
}

/// 知识库根配置状态的即时核对结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VaultRootStatus {
    pub configured: bool,
    pub root: Option<String>,
    pub exists: bool,
    pub is_dir: bool,
    pub error: Option<String>,
}

fn normalize_for_match(path: &Path) -> String {
    path.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// 允许作为知识库根的校验：绝对路径、不存在禁止片段、仓库外、应用数据根外、
/// 已存在且是目录。不创建目录，不改动任何文件。
pub fn assert_vault_root_allowed(path: &Path, app_root: Option<&Path>) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("知识库根必须是绝对路径：{}", path.display()));
    }
    assert_local_root_allowed(path)?;
    let normalized = normalize_for_match(path);
    for fragment in FORBIDDEN_ROOT_FRAGMENTS {
        if normalized.contains(fragment) {
            return Err(format!(
                "知识库根不能位于被禁止的目录（DSH / agent / Codex / 仓库内）：{}",
                path.display()
            ));
        }
    }
    assert_outside_repo(path)?;
    if let Some(app_root) = app_root {
        let app_norm = resolve_real_path(app_root)?;
        let path_norm = resolve_real_path(path)?;
        if path_norm.starts_with(&app_norm) {
            return Err(format!(
                "知识库根不能放在应用自身数据目录内：{}（应用数据根 {}）",
                path.display(),
                app_root.display()
            ));
        }
    }
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("知识库根不存在或不可访问：{}（{}）", path.display(), e))?;
    if meta.file_type().is_symlink() {
        return Err(format!("知识库根不能是符号链接：{}", path.display()));
    }
    if !meta.is_dir() {
        return Err(format!("知识库根必须是目录：{}", path.display()));
    }
    let real = resolve_real_path(path)?;
    // 元数据可读不等于目录可列出：权限不足时必须在这里如实失败，而不是先报
    // “目录可用”，等扫描时才发现一个文件也读不到。只打开目录句柄，不读条目。
    fs::read_dir(&real).map_err(|e| {
        format!(
            "知识库根不可读（缺少目录读取权限）：{}（{}）",
            path.display(),
            e
        )
    })?;
    Ok(real)
}

/// 根是否必须排除的目录。
fn excluded_dir_name(name: &str) -> Option<&'static str> {
    if name.starts_with('.') {
        return Some("hidden");
    }
    if VAULT_EXCLUDED_DIRS.contains(&name) {
        return Some("excluded");
    }
    None
}

/// 供写入通道复用同一条排除口径：名字是否属于隐藏或明确排除的目录。
pub fn is_excluded_dir_name(name: &str) -> bool {
    excluded_dir_name(name).is_some()
}

/// 相对路径（POSIX 分隔符）；只用于展示与作为读取键。
fn relative_posix(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// 只读扫描所选根。
///
/// 不跟随符号链接；隐藏目录与排除目录整棵跳过；只收集普通文件元数据。
pub fn scan_vault(root: &Path, app_root: Option<&Path>) -> Result<VaultScan, String> {
    let root_real = assert_vault_root_allowed(root, app_root)?;
    let mut scan = VaultScan {
        root: root_real.display().to_string(),
        files: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
        stats: ScanStats::default(),
    };
    let mut stack = vec![root_real.clone()];
    while let Some(dir) = stack.pop() {
        if scan.stats.truncated {
            break;
        }
        let depth = relative_posix(&root_real, &dir)
            .split('/')
            .filter(|s| !s.is_empty())
            .count();
        if depth > VAULT_MAX_DEPTH {
            // 只放弃这一棵过深的子树，其他分支继续；原因进跳过明细，不静默丢数据。
            scan.stats.skipped_depth += 1;
            push_skip(&mut scan, &root_real, &dir, "depth");
            continue;
        }
        scan.stats.visited_dirs += 1;
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                scan.stats.errors += 1;
                push_error(&mut scan, &root_real, &dir, format!("无法读取目录：{}", e));
                continue;
            }
        };
        // 排序保证结果稳定，不依赖文件系统顺序。
        let mut children: Vec<(fs::DirEntry, fs::FileType)> = Vec::new();
        for entry in entries {
            scan.stats.visited_entries += 1;
            if scan.stats.visited_entries > VAULT_MAX_VISITED {
                scan.stats.truncated = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    scan.stats.errors += 1;
                    push_error(&mut scan, &root_real, &dir, format!("无法读取目录条目：{}", e));
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    scan.stats.errors += 1;
                    push_error(
                        &mut scan,
                        &root_real,
                        &entry.path(),
                        format!("无法读取条目的文件类型：{}", e),
                    );
                    continue;
                }
            };
            children.push((entry, file_type));
        }
        children.sort_by_key(|(entry, _)| entry.file_name());
        for (entry, file_type) in children {
            if scan.stats.truncated {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_symlink() {
                scan.stats.skipped_symlink += 1;
                push_skip(&mut scan, &root_real, &path, "symlink");
                continue;
            }
            if file_type.is_dir() {
                if let Some(reason) = excluded_dir_name(&name) {
                    match reason {
                        "hidden" => scan.stats.skipped_hidden += 1,
                        _ => scan.stats.skipped_excluded += 1,
                    }
                    push_skip(&mut scan, &root_real, &path, reason);
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                scan.stats.skipped_non_file += 1;
                push_skip(&mut scan, &root_real, &path, "non_regular");
                continue;
            }
            if scan.files.len() >= VAULT_MAX_FILES {
                scan.stats.truncated = true;
                break;
            }
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(e) => {
                    scan.stats.errors += 1;
                    push_error(&mut scan, &root_real, &path, format!("无法读取文件信息：{}", e));
                    continue;
                }
            };
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let kind = if VAULT_NOTE_EXTS.contains(&extension.as_str()) {
                "note"
            } else {
                "attachment"
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            scan.stats.files += 1;
            scan.stats.total_bytes = scan.stats.total_bytes.saturating_add(meta.len());
            if kind == "note" {
                scan.stats.notes += 1;
            } else {
                scan.stats.attachments += 1;
            }
            scan.files.push(VaultFile {
                relative_path: relative_posix(&root_real, &path),
                file_name: name,
                extension,
                bytes: meta.len(),
                modified,
                kind,
            });
        }
    }
    scan.files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(scan)
}

fn push_skip(scan: &mut VaultScan, root: &Path, path: &Path, reason: &'static str) {
    if scan.skipped.len() >= VAULT_MAX_DETAILS {
        scan.stats.details_truncated = true;
        return;
    }
    scan.skipped.push(SkippedItem {
        relative_path: relative_posix(root, path),
        reason,
    });
}

fn push_error(scan: &mut VaultScan, root: &Path, path: &Path, message: String) {
    if scan.errors.len() >= VAULT_MAX_DETAILS {
        scan.stats.details_truncated = true;
        return;
    }
    scan.errors.push(VaultError {
        relative_path: relative_posix(root, path),
        message,
    });
}

/// 把根内相对路径解析为真实文件，逐层拒绝符号链接、\`..\` 与根外逃逸。
pub fn resolve_vault_file(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let raw = relative.trim();
    if raw.is_empty() {
        return Err("请提供知识库内的相对路径。".to_string());
    }
    let rel = Path::new(raw);
    if rel.is_absolute() {
        return Err(format!("只接受知识库内的相对路径：{}", raw));
    }
    let mut current = root.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(name) => {
                let name_text = name.to_string_lossy();
                if name_text.is_empty() || name_text == "." || name_text == ".." {
                    return Err(format!("路径包含非法片段：{}", raw));
                }
                current.push(name);
                let meta = fs::symlink_metadata(&current)
                    .map_err(|e| format!("知识库内找不到该文件：{}（{}）", raw, e))?;
                if meta.file_type().is_symlink() {
                    return Err(format!("拒绝读取符号链接：{}", raw));
                }
                // 与扫描口径一致：隐藏目录与明确排除目录不允许绕过结果列表直接
                // 点名进入（否则 .obsidian/ 被跳过了却仍能按正文打开）。根目录本身
                // 由用户显式选择，不在此处否决。
                if current != root {
                    if meta.is_dir() {
                        if let Some(reason) = excluded_dir_name(&name_text) {
                            return Err(format!("拒绝读取被排除的目录（{}）：{}", reason, raw));
                        }
                    }
                }
            }
            _ => return Err(format!("路径包含非法片段：{}", raw)),
        }
    }
    let root_real = resolve_real_path(root)?;
    let resolved = resolve_real_path(&current)?;
    if !resolved.starts_with(&root_real) || resolved == root_real {
        return Err(format!("拒绝读取知识库根之外的文件：{}", raw));
    }
    let meta = fs::metadata(&resolved)
        .map_err(|e| format!("无法读取文件信息：{}（{}）", raw, e))?;
    if !meta.is_file() {
        return Err(format!("不是普通文件，拒绝读取：{}", raw));
    }
    Ok(resolved)
}

/// 只读读取一个根内笔记文件。附件（非 Markdown/纯文本）不允许按正文读取。
pub fn read_vault_note(root: &Path, relative: &str) -> Result<VaultNote, String> {
    let resolved = resolve_vault_file(root, relative)?;
    let extension = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !VAULT_NOTE_EXTS.contains(&extension.as_str()) {
        return Err(format!(
            "只支持按正文读取 {}；{} 属于附件，只能查看元数据。",
            VAULT_NOTE_EXTS.join(" / "),
            extension
        ));
    }
    let meta = fs::metadata(&resolved)
        .map_err(|e| format!("无法读取文件信息：{}（{}）", resolved.display(), e))?;
    if meta.len() > VAULT_MAX_NOTE_BYTES {
        return Err(format!(
            "文件 {} 字节，超过单文件上限 {} 字节；请自行拆分后再打开。",
            meta.len(),
            VAULT_MAX_NOTE_BYTES
        ));
    }
    // 不依赖前面的 metadata 长度作为唯一上限：文件可能在检查后增长。
    // 最多读取上限 + 1 字节，以便在增长竞态下仍有硬内存/读取上界。
    let file = fs::File::open(&resolved)
        .map_err(|e| format!("无法读取文件：{}（{}）", resolved.display(), e))?;
    let mut bytes = Vec::with_capacity((VAULT_MAX_NOTE_BYTES as usize).saturating_add(1));
    file.take(VAULT_MAX_NOTE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取文件：{}（{}）", resolved.display(), e))?;
    if bytes.len() as u64 > VAULT_MAX_NOTE_BYTES {
        return Err(format!(
            "文件超过单文件上限 {} 字节；请自行拆分后再打开。",
            VAULT_MAX_NOTE_BYTES
        ));
    }
    let content = String::from_utf8(bytes).map_err(|_| {
        format!(
            "文件不是合法 UTF-8 文本，拒绝按正文读取：{}",
            resolved.display()
        )
    })?;
    let file_name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved.display().to_string());
    Ok(VaultNote {
        relative_path: relative_posix(root, &resolved),
        file_name,
        bytes: meta.len(),
        chars: content.chars().count(),
        extension,
        sha256: crate::sha256::sha256_hex(content.as_bytes()),
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-vault-{}-{}",
                tag,
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("vault");
            fs::create_dir_all(root.join("notes")).unwrap();
            fs::create_dir_all(root.join(".obsidian")).unwrap();
            fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
            Self { base, root }
        }

        fn write(&self, rel: &str, body: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, body).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    /// 临时收紧某目录权限，Drop 时恢复，保证夹具可清理。
    #[cfg(unix)]
    struct PermGuard {
        path: PathBuf,
        original: u32,
    }

    #[cfg(unix)]
    impl PermGuard {
        fn lock(path: &Path, mode: u32) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let original = fs::metadata(path).unwrap().permissions().mode();
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
            Self {
                path: path.to_path_buf(),
                original,
            }
        }

        /// root 用户不受权限位限制；此时返回 false，由调用方跳过而不是伪造通过。
        fn enforced(&self) -> bool {
            fs::read_dir(&self.path).is_err()
        }
    }

    #[cfg(unix)]
    impl Drop for PermGuard {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.original));
        }
    }

    fn names(scan: &VaultScan) -> Vec<&str> {
        scan.files.iter().map(|f| f.relative_path.as_str()).collect()
    }

    #[test]
    fn scans_notes_and_lists_attachments_without_reading_them() {
        let fx = Fixture::new("scan");
        fx.write("notes/a.md", "# 合成");
        fx.write("notes/b.markdown", "正文");
        fx.write("notes/c.txt", "纯文本");
        fx.write("notes/pic.png", "not really a png");
        let scan = scan_vault(&fx.root, None).unwrap();
        assert_eq!(
            names(&scan),
            vec!["notes/a.md", "notes/b.markdown", "notes/c.txt", "notes/pic.png"]
        );
        assert_eq!(scan.stats.notes, 3);
        assert_eq!(scan.stats.attachments, 1);
        assert_eq!(scan.files[3].kind, "attachment");
        assert_eq!(scan.files[0].kind, "note");
        assert_eq!(scan.stats.errors, 0);
    }

    #[test]
    fn skips_hidden_and_excluded_directories_with_counts() {
        let fx = Fixture::new("skip-dirs");
        fx.write("keep.md", "x");
        fx.write(".obsidian/app.json", "{}");
        fx.write(".hidden/secret.md", "x");
        fx.write("node_modules/pkg/index.md", "x");
        let scan = scan_vault(&fx.root, None).unwrap();
        assert_eq!(names(&scan), vec!["keep.md"]);
        assert!(scan.stats.skipped_hidden >= 2, "{:?}", scan.stats);
        assert_eq!(scan.stats.skipped_excluded, 1);
        assert!(scan
            .skipped
            .iter()
            .any(|s| s.relative_path == ".obsidian" && s.reason == "hidden"));
        assert!(scan
            .skipped
            .iter()
            .any(|s| s.relative_path == "node_modules" && s.reason == "excluded"));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_files_and_directories() {
        let fx = Fixture::new("symlink");
        fx.write("real.md", "x");
        let outside = fx.base.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("outside.md"), "outer").unwrap();
        std::os::unix::fs::symlink(outside.join("outside.md"), fx.root.join("link.md")).unwrap();
        std::os::unix::fs::symlink(&outside, fx.root.join("linkdir")).unwrap();
        let scan = scan_vault(&fx.root, None).unwrap();
        assert_eq!(names(&scan), vec!["real.md"]);
        assert_eq!(scan.stats.skipped_symlink, 2);
        assert!(read_vault_note(&fx.root, "link.md").is_err());
        assert!(read_vault_note(&fx.root, "linkdir/outside.md").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn skips_non_regular_files() {
        let fx = Fixture::new("fifo");
        let fifo = fx.root.join("pipe.md");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return;
        }
        let scan = scan_vault(&fx.root, None).unwrap();
        assert!(names(&scan).is_empty(), "{:?}", names(&scan));
        assert_eq!(scan.stats.skipped_non_file, 1);
    }

    /// 权限不足的根必须在准入阶段失败，而不是显示“可用”后扫出空结果。
    /// 以 root 运行时权限位不生效，跳过而不是伪造通过。
    #[cfg(unix)]
    #[test]
    fn unreadable_root_is_rejected_at_admission() {
        let fx = Fixture::new("unreadable-root");
        fx.write("a.md", "x");
        let guard = PermGuard::lock(&fx.root, 0o000);
        if !guard.enforced() {
            return;
        }
        let err = assert_vault_root_allowed(&fx.root, None).unwrap_err();
        assert!(err.contains("不可读"), "{err}");
        assert!(scan_vault(&fx.root, None).is_err());
    }

    /// 扫描中遇到不可读的子目录：记为逐项错误，其余可读部分照常返回。
    #[cfg(unix)]
    #[test]
    fn unreadable_subdirectory_becomes_an_error_entry() {
        let fx = Fixture::new("unreadable-subdir");
        fx.write("keep.md", "x");
        fx.write("locked/inner.md", "x");
        let locked = fx.root.join("locked");
        let guard = PermGuard::lock(&locked, 0o000);
        if !guard.enforced() {
            return;
        }
        let scan = scan_vault(&fx.root, None).unwrap();
        assert_eq!(names(&scan), vec!["keep.md"]);
        assert_eq!(scan.stats.errors, 1, "{:?}", scan.stats);
        assert!(scan
            .errors
            .iter()
            .any(|e| e.relative_path == "locked" && e.message.contains("无法读取目录")));
    }

    #[test]
    fn deep_subtree_is_skipped_without_aborting_scan() {
        let fx = Fixture::new("depth");
        fx.write("shallow.md", "x");
        fx.write("sibling/also-shallow.md", "x");
        let mut deep = String::new();
        for level in 0..=VAULT_MAX_DEPTH {
            deep.push_str(&format!("d{level}/"));
        }
        fx.write(&format!("{deep}too-deep.md"), "x");
        let scan = scan_vault(&fx.root, None).unwrap();
        // 超深子树只跳过自己，其余分支不受影响，也不把整次扫描标为截断。
        assert_eq!(scan.stats.skipped_depth, 1, "{:?}", scan.stats);
        assert!(!scan.stats.truncated, "{:?}", scan.stats);
        assert!(names(&scan).contains(&"shallow.md"), "{:?}", names(&scan));
        assert!(names(&scan).contains(&"sibling/also-shallow.md"), "{:?}", names(&scan));
        assert!(!names(&scan).iter().any(|n| n.contains("too-deep.md")));
        assert!(scan
            .skipped
            .iter()
            .any(|s| s.reason == "depth" && !s.relative_path.is_empty()));
    }

    #[test]
    fn detail_list_is_capped_and_flagged() {
        let fx = Fixture::new("details");
        for i in 0..(VAULT_MAX_DETAILS + 5) {
            fx.write(&format!(".hidden{i}/x.md"), "x");
        }
        let scan = scan_vault(&fx.root, None).unwrap();
        assert!(scan.stats.skipped_hidden > VAULT_MAX_DETAILS, "{:?}", scan.stats);
        assert_eq!(scan.skipped.len(), VAULT_MAX_DETAILS);
        assert!(scan.stats.details_truncated);
    }

    #[test]
    fn rejects_path_traversal_absolute_and_outside_paths() {
        let fx = Fixture::new("traversal");
        fx.write("ok.md", "x");
        assert!(resolve_vault_file(&fx.root, "ok.md").is_ok());
        for bad in ["../ok.md", "/etc/passwd", "notes/../../ok.md", "", "./ok.md"] {
            assert!(
                resolve_vault_file(&fx.root, bad).is_err(),
                "应拒绝 {bad}"
            );
        }
    }

    #[test]
    fn read_note_reports_size_and_encoding_limits() {
        let fx = Fixture::new("limits");
        fx.write("big.md", &"a".repeat((VAULT_MAX_NOTE_BYTES as usize) + 8));
        let err = read_vault_note(&fx.root, "big.md").unwrap_err();
        assert!(err.contains("上限"), "{err}");
        let pane = fx.root.join("bin.md");
        fs::write(&pane, [0xff, 0xfe, 0x00, 0x01]).unwrap();
        let err = read_vault_note(&fx.root, "bin.md").unwrap_err();
        assert!(err.contains("UTF-8"), "{err}");
        let png = fx.root.join("pic.png");
        fs::write(&png, "x").unwrap();
        let err = read_vault_note(&fx.root, "pic.png").unwrap_err();
        assert!(err.contains("附件"), "{err}");
    }

    /// 扫描时整棵跳过的目录，也不能被相对路径直接点名读取。
    #[test]
    fn read_note_refuses_excluded_and_hidden_directories() {
        let fx = Fixture::new("excluded-read");
        fx.write("ok.md", "x");
        fx.write(".obsidian/plugin.md", "should not be readable");
        fx.write(".trash/old.md", "should not be readable");
        fx.write("node_modules/dep.md", "should not be readable");
        fx.write("notes/kept.md", "kept");
        for bad in [
            ".obsidian/plugin.md",
            ".trash/old.md",
            "notes/../node_modules/dep.md",
            "node_modules/dep.md",
        ] {
            assert!(
                read_vault_note(&fx.root, bad).is_err(),
                "应拒绝读取被排除目录中的 {bad}"
            );
        }
        assert!(read_vault_note(&fx.root, "notes/kept.md").is_ok());
        assert!(read_vault_note(&fx.root, "ok.md").is_ok());
    }

    #[test]
    fn scan_is_read_only() {
        let fx = Fixture::new("readonly");
        fx.write("a.md", "x");
        let before: Vec<_> = fs::read_dir(&fx.root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        let _ = scan_vault(&fx.root, None).unwrap();
        let after: Vec<_> = fs::read_dir(&fx.root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(before, after);
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(fx.root.join("a.md"))
            .unwrap();
        f.write_all(b"").unwrap();
    }

    #[test]
    fn root_validation_rejects_relative_missing_repo_and_forbidden() {
        let fx = Fixture::new("root");
        assert!(assert_vault_root_allowed(Path::new("relative"), None).is_err());
        assert!(assert_vault_root_allowed(&fx.base.join("missing"), None).is_err());
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
            .to_path_buf();
        assert!(assert_vault_root_allowed(&repo, None).is_err());
        assert!(assert_vault_root_allowed(Path::new("/home/u/.dsh/vault"), None).is_err());
        assert!(assert_vault_root_allowed(Path::new("/home/u/.agents/v"), None).is_err());
        let app = fx.base.join("app");
        fs::create_dir_all(app.join("config")).unwrap();
        assert!(assert_vault_root_allowed(&app, Some(&app)).is_err());
        assert!(assert_vault_root_allowed(&fx.root, None).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn root_must_not_be_symlink() {
        let fx = Fixture::new("root-link");
        let link = fx.base.join("link-root");
        std::os::unix::fs::symlink(&fx.root, &link).unwrap();
        let err = assert_vault_root_allowed(&link, None).unwrap_err();
        assert!(err.contains("符号链接"), "{err}");
    }
}

