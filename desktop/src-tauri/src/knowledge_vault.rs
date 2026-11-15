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
    /// 实际读入并解析的字节数；不采用可能过期的扫描或元数据长度。
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
                // OD-KB-201 S2：vault 根下的 governance/ 是宿主治理记录，不是
                // 知识资产；只跳过这一层，不把任意同名子目录一并排除。
                if dir == root_real && name == crate::knowledge_governance::GOVERNANCE_DIR {
                    scan.stats.skipped_excluded += 1;
                    push_skip(&mut scan, &root_real, &path, "governance");
                    continue;
                }
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

/// 一次有界读取的结果。失败时也带回已经实际读入的字节数，供调用方记账。
pub struct BoundedNoteRead {
    pub note: Result<VaultNote, String>,
    pub bytes_read: u64,
    /// 文件内容超过本次读取预算时为 true；预算外不会返回部分正文。
    pub budget_exhausted: bool,
}

impl BoundedNoteRead {
    fn failed(message: String, bytes_read: u64, budget_exhausted: bool) -> Self {
        Self {
            note: Err(message),
            bytes_read,
            budget_exhausted,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BoundedReadCompleteness {
    Complete,
    BudgetExceeded,
    ChangedDuringRead,
}

fn classify_bounded_read(bytes_read: u64, actual_len: u64, cap: u64) -> BoundedReadCompleteness {
    if actual_len > cap {
        BoundedReadCompleteness::BudgetExceeded
    } else if bytes_read != actual_len {
        BoundedReadCompleteness::ChangedDuringRead
    } else {
        BoundedReadCompleteness::Complete
    }
}

/// 只读读取一个根内笔记文件。附件（非 Markdown/纯文本）不允许按正文读取。
pub fn read_vault_note(root: &Path, relative: &str) -> Result<VaultNote, String> {
    read_vault_note_within(root, relative, VAULT_MAX_NOTE_BYTES).note
}

/// 在调用方给定的剩余预算内读取一篇笔记，最多读入 `budget` 字节。
///
/// 单篇 512 KiB 上限仍然生效，读取上限取两者较小值。文件超过预算时返回
/// `budget_exhausted = true`，绝不把截断内容当成完整正文；失败结果也带上实际
/// 读入字节数，避免调用方按扫描元数据误记预算。
pub fn read_vault_note_within(root: &Path, relative: &str, budget: u64) -> BoundedNoteRead {
    let resolved = match resolve_vault_file(root, relative) {
        Ok(path) => path,
        Err(e) => return BoundedNoteRead::failed(e, 0, false),
    };
    let extension = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !VAULT_NOTE_EXTS.contains(&extension.as_str()) {
        return BoundedNoteRead::failed(format!(
            "只支持按正文读取 {}；{} 属于附件，只能查看元数据。",
            VAULT_NOTE_EXTS.join(" / "),
            extension
        ), 0, false);
    }
    let meta = match fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(e) => {
            return BoundedNoteRead::failed(
                format!("无法读取文件信息：{}（{}）", resolved.display(), e),
                0,
                false,
            )
        }
    };
    if meta.len() > VAULT_MAX_NOTE_BYTES {
        return BoundedNoteRead::failed(format!(
            "文件 {} 字节，超过单文件上限 {} 字节；请自行拆分后再打开。",
            meta.len(),
            VAULT_MAX_NOTE_BYTES
        ), 0, false);
    }
    if meta.len() > budget {
        return BoundedNoteRead::failed(
            format!(
                "文件 {} 字节，超过本次搜索剩余读取预算 {} 字节；未读入部分内容。",
                meta.len(),
                budget
            ),
            0,
            true,
        );
    }
    // 不依赖前面的 metadata 长度作为唯一上限：文件可能在检查后增长。
    // 最多读取 min(预算, 单篇上限) 字节；随后用已打开文件的元数据判断是否被
    // 截断，不额外多读 1 字节，因此实际读入量不会越过预算。
    let mut file = match fs::File::open(&resolved) {
        Ok(file) => file,
        Err(e) => {
            return BoundedNoteRead::failed(
                format!("无法读取文件：{}（{}）", resolved.display(), e),
                0,
                false,
            )
        }
    };
    let cap = budget.min(VAULT_MAX_NOTE_BYTES);
    let capacity = meta.len().min(cap) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    if let Err(e) = file.by_ref().take(cap).read_to_end(&mut bytes) {
        return BoundedNoteRead::failed(
            format!("无法读取文件：{}（{}）", resolved.display(), e),
            bytes.len() as u64,
            false,
        );
    }
    let bytes_read = bytes.len() as u64;
    // 用已打开文件句柄的元数据判断是否被截断，避免路径在打开后被替换。
    let actual_len = match file.metadata() {
        Ok(meta) => meta.len(),
        Err(e) => {
            return BoundedNoteRead::failed(
                format!("无法读取文件信息：{}（{}）", resolved.display(), e),
                bytes_read,
                false,
            )
        }
    };
    if actual_len > VAULT_MAX_NOTE_BYTES {
        return BoundedNoteRead::failed(format!(
            "文件超过单文件上限 {} 字节；请自行拆分后再打开。",
            VAULT_MAX_NOTE_BYTES
        ), bytes_read, false);
    }
    match classify_bounded_read(bytes_read, actual_len, cap) {
        BoundedReadCompleteness::BudgetExceeded => {
            return BoundedNoteRead::failed(
                format!(
                    "文件 {} 字节，超过本次搜索剩余读取预算 {} 字节；未读入部分内容。",
                    actual_len, cap
                ),
                bytes_read,
                true,
            );
        }
        BoundedReadCompleteness::ChangedDuringRead => {
            return BoundedNoteRead::failed(
                format!(
                    "文件在读取期间发生变化（已读入 {} 字节，当前长度 {} 字节）；未返回部分内容。",
                    bytes_read, actual_len
                ),
                bytes_read,
                false,
            );
        }
        BoundedReadCompleteness::Complete => {}
    }
    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => {
            return BoundedNoteRead::failed(
                format!(
                    "文件不是合法 UTF-8 文本，拒绝按正文读取：{}",
                    resolved.display()
                ),
                bytes_read,
                false,
            )
        }
    };
    let file_name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved.display().to_string());
    BoundedNoteRead {
        note: Ok(VaultNote {
            relative_path: relative_posix(root, &resolved),
            file_name,
            bytes: bytes_read,
            chars: content.chars().count(),
            extension,
            sha256: crate::sha256::sha256_hex(content.as_bytes()),
            content,
        }),
        bytes_read,
        budget_exhausted: false,
    }
}

// ---------------------------------------------------------------------------
// workspace 暂存 → 库根同步（OD-ARCH-152，方案 A）
//
// 背景：DSH 会话沙箱的可写根只有 session.cwd 与临时目录，知识库根在 workspace
// 之外。模型直接写库根会触发一次人工提权审批，用户实测 7 个文件耗时 45 分钟。
// 方案 A 不改 runtime/、不放宽沙箱：模型只写 <workspace>/staging/kb/，由宿主
// 进程读取暂存卡、按既有写入通道复核后落库并合并索引。
//
// 边界与 `vault_write::create_note` 同一口径：只接受知识库内相对路径、仅
// 知识库/概念/ 与 知识库/方法/、UTF-8、单卡正文不超过 CARD_MAX_CHARS；
// 目标已存在记为冲突，绝不覆盖。
// ---------------------------------------------------------------------------

/// 暂存目录（相对本地数据根 / 引擎任务 workspace）。
pub const SYNC_STAGING_REL: &str = "staging/kb";

/// 单次同步读取的暂存卡上限，防止异常目录拖垮宿主调用。
pub const SYNC_MAX_CARDS: usize = 200;

/// 已落库的一张卡。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncCreatedCard {
    pub relative_path: String,
    pub title: String,
    pub bytes: u64,
}

/// 冲突或错误项：相对路径 + 可读原因，和知识整理报告保持同一形状。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SyncIssue {
    pub relative_path: String,
    pub reason: String,
}

/// 一次 workspace 同步的结构化结果。
///
/// `index_updated` 表示「本次新建的卡都已经在索引里」：没有新卡时按已完成处理；
/// 索引写入失败时会把新建卡回滚并置为 false，不留半套结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct SyncReport {
    pub ok: bool,
    pub created: Vec<SyncCreatedCard>,
    pub conflicts: Vec<SyncIssue>,
    pub errors: Vec<SyncIssue>,
    pub index_updated: bool,
    pub staging_dir: String,
    pub note: String,
}

/// 暂存卡的相对路径只允许镜像库内位置：`知识库/概念/x.md` 或 `知识库/方法/x.md`。
fn check_staged_card_path(relative: &str) -> Result<(String, String, String), String> {
    let normalized = relative.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    if parts.len() != 3
        || parts[0] != crate::knowledge_ingest::KNOWLEDGE_DIR
        || (parts[1] != "概念" && parts[1] != "方法")
    {
        return Err(format!(
            "暂存卡必须放在 staging/kb/知识库/概念/ 或 staging/kb/知识库/方法/：{relative}"
        ));
    }
    let file_name = parts[2];
    if !file_name.ends_with(".md") || file_name == ".md" {
        return Err(format!("暂存卡必须是 .md 文件：{relative}"));
    }
    if file_name.starts_with('.') {
        return Err(format!("暂存卡不能使用隐藏文件名：{relative}"));
    }
    if file_name.chars().any(|c| matches!(c, '[' | ']' | '|' | '\n' | '\r')) {
        return Err(format!("文件名含索引分隔符，无法生成索引行：{relative}"));
    }
    let title = file_name
        .strip_suffix(".md")
        .unwrap_or(file_name)
        .trim()
        .to_string();
    if title.is_empty() {
        return Err(format!("卡片标题不能为空：{relative}"));
    }
    let section = parts[1].to_string();
    Ok((normalized, section, title))
}

/// 索引行摘要：取正文第一段非空文字，最多 40 字。
fn card_summary(body: &str) -> String {
    for line in body.lines() {
        let text = line.trim().trim_start_matches('#').trim();
        if text.is_empty() {
            continue;
        }
        let chars: Vec<char> = text.chars().take(40).collect();
        return chars.into_iter().collect();
    }
    "待补充摘要".to_string()
}

/// 索引里是否已有指向这张卡的链接行。用于幂等：同标题同路径不重复追加。
fn index_has_card_link(index: &str, section: &str, file_name: &str) -> bool {
    let needle = format!("]({section}/{file_name})");
    index.lines().any(|line| line.trim().contains(&needle))
}

/// 与 `knowledge_ingest::apply_payload` 的索引追加语义保持一致：向对应小节末尾
/// 插入，忽略与现有行完全相同的条目，最后保证文件以换行结尾。那份实现是模块私有
/// 的，这里保留同一份规则；改动其中一处时必须同步另一处。
fn append_sync_index_lines(index: &str, entries: &[(String, String)]) -> String {
    let mut lines: Vec<String> = index.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.extend(
            crate::knowledge_ingest::INDEX_TEMPLATE
                .lines()
                .map(str::to_string),
        );
    }
    for section in ["概念", "方法"] {
        let heading = format!("## {section}");
        let additions: Vec<String> = entries
            .iter()
            .filter(|(s, _)| s == section)
            .map(|(_, line)| line.clone())
            .filter(|line| !lines.iter().any(|existing| existing.trim() == line.trim()))
            .collect();
        if additions.is_empty() {
            continue;
        }
        if let Some(pos) = lines.iter().position(|line| line.trim() == heading) {
            let mut insert = pos + 1;
            while insert < lines.len() && !lines[insert].trim_start().starts_with("## ") {
                insert += 1;
            }
            let mut block = Vec::new();
            if insert > pos + 1 && !lines[insert - 1].trim().is_empty() {
                block.push(String::new());
            }
            block.extend(additions);
            lines.splice(insert..insert, block);
        } else {
            if lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            lines.push(heading);
            lines.extend(additions);
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 递归收集暂存卡。符号链接计入错误并跳过；隐藏目录与排除目录整棵跳过，不跟随。
fn collect_staged_cards(staging: &Path, report: &mut SyncReport) -> Vec<(String, PathBuf)> {
    let mut cards = Vec::new();
    let mut stack = vec![staging.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                report.errors.push(SyncIssue {
                    relative_path: relative_posix(staging, &dir),
                    reason: format!("无法读取暂存目录：{e}"),
                });
                continue;
            }
        };
        let mut children: Vec<(fs::DirEntry, fs::FileType)> = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    report.errors.push(SyncIssue {
                        relative_path: relative_posix(staging, &dir),
                        reason: format!("无法读取暂存目录条目：{e}"),
                    });
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    report.errors.push(SyncIssue {
                        relative_path: relative_posix(staging, &entry.path()),
                        reason: format!("无法读取暂存条目类型：{e}"),
                    });
                    continue;
                }
            };
            children.push((entry, file_type));
        }
        children.sort_by_key(|(entry, _)| entry.file_name());
        for (entry, file_type) in children {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_symlink() {
                report.errors.push(SyncIssue {
                    relative_path: relative_posix(staging, &path),
                    reason: "暂存路径是符号链接，已跳过且未写入".to_string(),
                });
                continue;
            }
            if file_type.is_dir() {
                if name.starts_with('.') || is_excluded_dir_name(&name) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                report.errors.push(SyncIssue {
                    relative_path: relative_posix(staging, &path),
                    reason: "暂存条目不是普通文件，已跳过".to_string(),
                });
                continue;
            }
            if !name.ends_with(".md") {
                // 暂存目录里的非卡片文件不参与同步，也不当成失败：可能只是模型
                // 留下的草稿；错误列表只记录会影响落库的条目。
                continue;
            }
            if cards.len() >= SYNC_MAX_CARDS {
                report.errors.push(SyncIssue {
                    relative_path: relative_posix(staging, &path),
                    reason: format!("单次同步最多 {SYNC_MAX_CARDS} 张卡，其余暂存卡未处理"),
                });
                return cards;
            }
            cards.push((relative_posix(staging, &path), path));
        }
    }
    cards.sort_by(|a, b| a.0.cmp(&b.0));
    cards
}

/// 从 workspace 暂存目录同步卡片到知识库根。
///
/// 成功的暂存卡在卡片与索引都落盘后**删除**：workspace 是任务临时区，保留副本
/// 会与库内版本分叉，后续人工改卡也看不出哪份是准的。失败、冲突与超限的暂存卡
/// 原样保留，供用户处理后重试，因此不会静默丢掉模型产出。
pub fn sync_from_workspace(workspace_dir: &Path, root: &Path) -> Result<SyncReport, String> {
    if !workspace_dir.is_absolute() {
        return Err(format!(
            "workspace 目录必须是绝对路径：{}",
            workspace_dir.display()
        ));
    }
    let workspace_meta = fs::symlink_metadata(workspace_dir)
        .map_err(|e| format!("workspace 目录不可访问：{}（{e}）", workspace_dir.display()))?;
    if workspace_meta.file_type().is_symlink() {
        return Err(format!(
            "workspace 目录不能是符号链接：{}",
            workspace_dir.display()
        ));
    }
    if !workspace_meta.is_dir() {
        return Err(format!(
            "workspace 目录必须是目录：{}",
            workspace_dir.display()
        ));
    }
    assert_local_root_allowed(workspace_dir)?;
    assert_outside_repo(workspace_dir)?;

    let root = assert_vault_root_allowed(root, None)?;
    let staging = workspace_dir.join(SYNC_STAGING_REL);
    let mut report = SyncReport {
        staging_dir: staging.display().to_string(),
        index_updated: true,
        ..Default::default()
    };
    match fs::symlink_metadata(&staging) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(format!("暂存目录不能是符号链接：{}", staging.display()));
        }
        Ok(meta) if !meta.is_dir() => {
            return Err(format!("暂存路径不是目录：{}", staging.display()));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            report.ok = true;
            report.note = "workspace 里没有暂存卡，无需同步。".to_string();
            return Ok(report);
        }
        Err(e) => {
            return Err(format!("暂存目录不可访问：{}（{e}）", staging.display()));
        }
    }

    // 与 knowledge_ingest::init_vault 同一落点：先保证写入通道需要的目录与索引存在。
    for dir in [
        crate::knowledge_ingest::KNOWLEDGE_DIR,
        crate::knowledge_ingest::CONCEPT_DIR,
        crate::knowledge_ingest::METHOD_DIR,
    ] {
        let path = root.join(dir);
        if !path.is_dir() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("创建知识库目录失败（{}）：{e}", path.display()))?;
        }
    }
    let index_rel = crate::knowledge_ingest::INDEX_REL;
    let index_path = root.join(index_rel);
    if !index_path.is_file() {
        crate::vault_write::create_note(&root, index_rel, crate::knowledge_ingest::INDEX_TEMPLATE)?;
    }
    let original_index = fs::read_to_string(&index_path)
        .map_err(|e| format!("读取知识库索引失败（{}）：{e}", index_path.display()))?;

    let staged = collect_staged_cards(&staging, &mut report);
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut synced_sources: Vec<(String, PathBuf)> = Vec::new();
    for (relative, path) in staged {
        let (relative, section, title) = match check_staged_card_path(&relative) {
            Ok(value) => value,
            Err(reason) => {
                report.errors.push(SyncIssue {
                    relative_path: relative,
                    reason,
                });
                continue;
            }
        };
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) => {
                report.errors.push(SyncIssue {
                    relative_path: relative.clone(),
                    reason: format!("无法读取暂存卡信息：{e}"),
                });
                continue;
            }
        };
        if meta.len() > VAULT_MAX_NOTE_BYTES {
            report.errors.push(SyncIssue {
                relative_path: relative.clone(),
                reason: format!(
                    "暂存卡 {} 字节，超过单文件上限 {} 字节",
                    meta.len(),
                    VAULT_MAX_NOTE_BYTES
                ),
            });
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                report.errors.push(SyncIssue {
                    relative_path: relative.clone(),
                    reason: format!("暂存卡不是可读的 UTF-8 文本：{e}"),
                });
                continue;
            }
        };
        let parsed = crate::vault_note::parse_note(&content);
        if !parsed.has_frontmatter {
            report.errors.push(SyncIssue {
                relative_path: relative.clone(),
                reason: "暂存卡缺少 frontmatter（至少需要 tags/date），未落库".to_string(),
            });
            continue;
        }
        let body_chars = parsed.body.chars().count();
        if body_chars > crate::knowledge_ingest::CARD_MAX_CHARS {
            report.errors.push(SyncIssue {
                relative_path: relative.clone(),
                reason: format!(
                    "正文 {body_chars} 字，超过 {} 字上限；请拆卡后重试",
                    crate::knowledge_ingest::CARD_MAX_CHARS
                ),
            });
            continue;
        }
        if root.join(&relative).exists() {
            report.conflicts.push(SyncIssue {
                relative_path: relative.clone(),
                reason: "库里已有同路径卡片，同步不覆盖；请改名或人工合并".to_string(),
            });
            continue;
        }
        let file_name = relative.rsplit('/').next().unwrap_or(relative.as_str()).to_string();
        if index_has_card_link(&original_index, &section, &file_name) {
            // 索引行已存在但卡文件不在：不能静默补空行，按冲突交给人工判断。
            report.conflicts.push(SyncIssue {
                relative_path: relative.clone(),
                reason: "索引里已有这张卡的链接行，但库内缺文件；未自动补写".to_string(),
            });
            continue;
        }
        match crate::vault_write::create_note(&root, &relative, &content) {
            Ok(note) => {
                let tags = if parsed.tags.is_empty() {
                    "未分类".to_string()
                } else {
                    parsed.tags.join(", ")
                };
                entries.push((
                    section.clone(),
                    format!(
                        "- [{title}]({section}/{file_name}) | 标签：{tags} | {}",
                        card_summary(&parsed.body)
                    ),
                ));
                report.created.push(SyncCreatedCard {
                    relative_path: relative.clone(),
                    title,
                    bytes: note.bytes,
                });
                synced_sources.push((relative, path));
            }
            Err(reason) => report.errors.push(SyncIssue {
                relative_path: relative,
                reason,
            }),
        }
    }

    if !entries.is_empty() {
        let next = append_sync_index_lines(&original_index, &entries);
        let expected = crate::vault_write::content_sha256(&original_index);
        match crate::vault_write::save_note(&root, index_rel, &next, &expected) {
            Ok(_) => {
                // 只有索引也落盘后才删除暂存卡，避免「卡进了库、索引没进」的半套状态。
                for (relative_path, source) in synced_sources {
                    if let Err(e) = fs::remove_file(&source) {
                        report.errors.push(SyncIssue {
                            relative_path,
                            reason: format!("删除已同步暂存卡失败（库内卡片已保留）：{e}"),
                        });
                    }
                }
            }
            Err(e) => {
                report.index_updated = false;
                report.errors.push(SyncIssue {
                    relative_path: index_rel.to_string(),
                    reason: format!("索引更新失败：{e}"),
                });
                for card in &report.created {
                    let _ = fs::remove_file(root.join(&card.relative_path));
                }
                report.created.clear();
            }
        }
    }

    report.ok = report.errors.is_empty() && report.conflicts.is_empty();
    report.note = format!(
        "已同步 {} 张；冲突 {}；错误 {}",
        report.created.len(),
        report.conflicts.len(),
        report.errors.len()
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn bounded_read_completeness_rejects_growth_after_eof() {
        assert_eq!(
            classify_bounded_read(100, 120, 200),
            BoundedReadCompleteness::ChangedDuringRead,
            "EOF 后文件增长但仍在预算内时，也不能把旧内容当完整正文"
        );
        assert_eq!(
            classify_bounded_read(100, 201, 200),
            BoundedReadCompleteness::BudgetExceeded
        );
        assert_eq!(
            classify_bounded_read(100, 100, 200),
            BoundedReadCompleteness::Complete
        );
    }

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

