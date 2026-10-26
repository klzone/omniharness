//! 可重建的 Vault 链接索引与锚点定位（OD-KB-001 P3-S02 / P3-S03）。
//!
//! 索引只在内存：每次显式构建都从当前扫描结果与文件原文重新解析，构建结束即
//! 得到一个不可变快照。没有持久数据库、没有后台 watcher，也不写知识库目录。
//!
//! 一致性边界：构建时先用 P1 扫描拿到文件清单，再逐篇有界读取。若同一文件在
//! 扫描后变化（长度或哈希与扫描时不一致），整次快照标记 `incomplete` 并记录原因，
//! 不发布“看起来完整”的部分结果。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::knowledge_vault::{read_vault_note_within, scan_vault, VaultFile, VaultScan};
use crate::vault_link::{is_external_target, parse_links_report, LINK_MAX_PER_NOTE};
use crate::vault_note::{parse_note, ParsedNote};

/// 单次构建最多读取的总字节数（与搜索同一 32 MiB 口径）。
pub const INDEX_MAX_BYTES: u64 = 32 * 1024 * 1024;
/// 单次构建最多处理的笔记数。
pub const INDEX_MAX_NOTES: usize = 2_000;
/// 单次构建最多返回的引用条数。
pub const INDEX_MAX_LINKS: usize = 20_000;

/// 链接解析状态。
pub const RESOLVED: &str = "resolved";
pub const UNRESOLVED: &str = "unresolved";
pub const AMBIGUOUS: &str = "ambiguous";
pub const EXTERNAL: &str = "external";
pub const ATTACHMENT: &str = "attachment";

/// 锚点定位状态。
pub const LOCATED: &str = "exact";
pub const UNLOCATED: &str = "unlocated";

/// 索引里的一个笔记节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexNote {
    pub relative_path: String,
    pub file_name: String,
    pub sha256: String,
    pub bytes: u64,
    /// 被其他笔记引用的次数（解析成功、且来源不是自身）。
    pub backlink_count: usize,
    /// 本篇指向其他笔记的引用数（解析成功、不含自身与附件/外部）。
    pub outlink_count: usize,
    /// 没有任何其他笔记指向它。
    pub orphan: bool,
    pub lines: usize,
    /// Frontmatter 里规范化后的标签（小写、去重、保持首次出现顺序）。
    /// P4 图谱按标签筛选需要它；索引构建时已解析，不额外读盘。
    #[serde(default)]
    pub tags: Vec<String>,
    /// Frontmatter 里声明的别名，保留原始大小写。
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// 一条已解析的引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexLink {
    pub id: String,
    pub source_path: String,
    pub kind: String,
    pub target_raw: String,
    pub display: Option<String>,
    pub anchor: Option<String>,
    pub block_anchor: bool,
    pub status: String,
    /// 解析到的笔记路径；unresolved/ambiguous/external/attachment 时为 None。
    pub target_path: Option<String>,
    /// 附件目标的根内路径（status=attachment 时有值）。
    pub attachment_path: Option<String>,
    /// 歧义时的候选路径，按字典序。
    pub candidates: Vec<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub context: String,
    pub anchor_status: Option<String>,
    /// 锚点定位到的原文件行号；定位失败为 None。
    pub anchor_line: Option<usize>,
}

/// 一次构建的问题记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexIssue {
    pub relative_path: String,
    pub code: String,
    pub message: String,
}

/// 索引构建统计。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStats {
    pub notes: usize,
    pub notes_scanned: usize,
    pub bytes_read: u64,
    pub budget_bytes: u64,
    pub links: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub ambiguous: usize,
    pub external: usize,
    pub attachments: usize,
    pub orphans: usize,
    pub warnings: usize,
}

/// 一次构建的完整快照。`generation` 每次构建递增，界面用它判断是否过期。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkIndex {
    pub generation: u64,
    pub built_at: u64,
    pub root: String,
    /// 快照是否完整。不完整时 `incomplete_reason` 说明原因，界面不得显示为完整关系。
    pub complete: bool,
    pub incomplete_reason: Option<String>,
    pub notes: Vec<IndexNote>,
    pub links: Vec<IndexLink>,
    pub issues: Vec<IndexIssue>,
    pub stats: IndexStats,
    /// 快照指纹：所有输入笔记路径与内容哈希的确定性汇总。
    pub fingerprint: String,
}

/// 一篇笔记的链接视图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteLinks {
    pub generation: u64,
    pub note: IndexNote,
    /// 本篇指向别处的引用（含未解析/歧义/外部/附件）。
    pub outgoing: Vec<IndexLink>,
    /// 其他笔记指向本篇的引用。
    pub backlinks: Vec<IndexLink>,
}

/// 解析一个目标的只读事实，供单元测试与界面解释复用。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    path: Option<String>,
    status: &'static str,
    candidates: Vec<String>,
    /// 附件目标在根内的相对路径；仅 `attachment` 状态有值。
    attachment: Option<String>,
}

/// 标题锚点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingAnchor {
    pub slug: String,
    pub text: String,
    pub level: usize,
    pub line: usize,
}

/// 块锚点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockAnchor {
    pub id: String,
    pub line: usize,
}

/// 从正文抽取标题锚点（跳过围栏代码块）。
pub fn heading_anchors(body: &str, body_start_line: usize) -> Vec<HeadingAnchor> {
    let mut out = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut fence: Option<char> = None;
    for (idx, raw_line) in body.split_inclusive('\n').enumerate() {
        let line = raw_line.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        let Some((level, text)) = atx_heading(trimmed) else {
            continue;
        };
        let base = slugify(&text);
        let seen = counts.entry(base.clone()).or_insert(0);
        let slug = if *seen == 0 { base } else { format!("{base}-{seen}") };
        *seen += 1;
        out.push(HeadingAnchor {
            slug,
            text,
            level,
            line: body_start_line + idx,
        });
    }
    out
}

fn atx_heading(trimmed: &str) -> Option<(usize, String)> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some((hashes, text))
}

/// Obsidian 风格标题 slug：小写、空白折叠为 `-`、保留文字与 `_-`。
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.trim().chars() {
        if c.is_whitespace() {
            pending_dash = true;
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '-' {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            for lower in c.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// 从正文抽取显式块标识（`^id` 位于行尾）。
pub fn block_anchors(body: &str, body_start_line: usize) -> Vec<BlockAnchor> {
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    for (idx, raw_line) in body.split_inclusive('\n').enumerate() {
        let line = raw_line.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            continue;
        }
        let Some(at) = line.rfind('^') else {
            continue;
        };
        let id = line[at + 1..].trim();
        if id.is_empty() || !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            continue;
        }
        out.push(BlockAnchor {
            id: id.to_string(),
            line: body_start_line + idx,
        });
    }
    out
}

/// 解析锚点：返回 (状态, 行号)。缺失锚点只回退到笔记顶部，不跨笔记。
pub fn resolve_anchor(
    doc: &ParsedNote,
    anchor: &str,
) -> (String, Option<usize>) {
    let want = anchor.trim();
    if want.starts_with('^') {
        let id = want.trim_start_matches('^').trim();
        let blocks = block_anchors(&doc.body, doc.body_start_line);
        return match blocks.into_iter().find(|b| b.id == id) {
            Some(b) => (LOCATED.to_string(), Some(b.line)),
            None => (UNLOCATED.to_string(), None),
        };
    }
    let headings = heading_anchors(&doc.body, doc.body_start_line);
    let slug = slugify(want);
    if let Some(h) = headings.iter().find(|h| h.slug == slug) {
        return (LOCATED.to_string(), Some(h.line));
    }
    let lower = want.to_lowercase();
    if let Some(h) = headings.iter().find(|h| h.text.to_lowercase() == lower) {
        return (LOCATED.to_string(), Some(h.line));
    }
    (UNLOCATED.to_string(), None)
}

/// 一个待读文件。
struct LoadedNote {
    path: String,
    file_name: String,
    sha256: String,
    bytes: u64,
    doc: ParsedNote,
    lines: usize,
}

fn stem_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_string(),
        None => name.to_string(),
    }
}

/// 去掉扩展名的完整相对路径（保留目录），用于 `[[目录/笔记]]` 这类目标。
fn path_without_ext(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty() && !ext.is_empty() && !ext.contains('/') => stem,
        _ => path,
    }
}

fn ext_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    }
}

fn is_note_ext(ext: &str) -> bool {
    matches!(ext, "md" | "markdown" | "txt")
}

/// 归一化 POSIX 相对路径：去掉空段与 `.`，`..` 回退上一段。
/// 越过根部的 `..` 保留下来，随后自然匹配不到根内文件。
fn normalize_posix(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|last| *last != "..") {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out.join("/")
}

/// 按固定优先级把目标解析到笔记/附件。候选按字典序返回，保证确定性。
fn resolve_target(target: &str, source: &str, notes: &[&LoadedNote], all_files: &[VaultFile]) -> Target {
    if target.trim().is_empty() {
        return Target {
            path: Some(source.to_string()),
            status: RESOLVED,
            candidates: vec![],
            attachment: None,
        };
    }
    if is_external_target(target) {
        return Target { path: None, status: EXTERNAL, candidates: vec![], attachment: None };
    }
    let normalized = normalize_posix(&target.trim().replace('\\', "/"));
    let want_ext = ext_of(&normalized);
    let wants_file = (!want_ext.is_empty() && is_note_ext(&want_ext))
        || all_files
            .iter()
            .any(|f| f.relative_path == normalized || f.file_name == normalized);

    let collect = |pred: &dyn Fn(&LoadedNote) -> bool| -> Vec<String> {
        let mut v: Vec<String> = notes.iter().filter(|n| pred(n)).map(|n| n.path.clone()).collect();
        v.sort();
        v
    };
    let resolved = |path: String| Target {
        candidates: vec![path.clone()],
        path: Some(path),
        status: RESOLVED,
        attachment: None,
    };
    let ambiguous = |candidates: Vec<String>| Target {
        path: None,
        status: AMBIGUOUS,
        candidates,
        attachment: None,
    };

    // 第 1 层：完整相对路径（大小写敏感）。
    let exact_path = collect(&|n| n.path == normalized);
    if exact_path.len() == 1 {
        return resolved(exact_path[0].clone());
    }
    if exact_path.len() > 1 {
        return ambiguous(exact_path);
    }

    // 第 2 层：带扩展名按文件名精确匹配。
    if wants_file {
        let by_name = collect(&|n| n.file_name == normalized);
        if by_name.len() == 1 {
            return resolved(by_name[0].clone());
        }
        if by_name.len() > 1 {
            return ambiguous(by_name);
        }
    }

    // 第 3 层：相对来源目录的路径。
    if normalized.contains('/') {
        let source_dir = source.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let joined = if source_dir.is_empty() {
            normalized.clone()
        } else {
            normalize_posix(&format!("{}/{}", source_dir, normalized))
        };
        let by_join = collect(&|n| n.path == joined);
        if by_join.len() == 1 {
            return resolved(by_join[0].clone());
        }
        if !wants_file {
            let by_joined_stem = collect(&|n| path_without_ext(&n.path) == joined);
            if by_joined_stem.len() == 1 {
                return resolved(by_joined_stem[0].clone());
            }
            if by_joined_stem.len() > 1 {
                return ambiguous(by_joined_stem);
            }
        }
        // 路径后缀唯一匹配。
        let suffix = format!("/{}", normalized);
        let by_suffix = collect(&|n| n.path.ends_with(&suffix));
        if by_suffix.len() == 1 {
            return resolved(by_suffix[0].clone());
        }
        if by_suffix.len() > 1 {
            return ambiguous(by_suffix);
        }
        if !wants_file {
            // `[[目录/笔记]]`：带扩展名的路径等同于后缀匹配，去掉扩展名后再比一次。
            let by_suffix_stem = collect(&|n| {
                path_without_ext(&n.path).ends_with(&suffix)
            });
            if by_suffix_stem.len() == 1 {
                return resolved(by_suffix_stem[0].clone());
            }
            if by_suffix_stem.len() > 1 {
                return ambiguous(by_suffix_stem);
            }
        }
    }

    // 第 4 层：省略扩展名时按主干精确匹配。
    if !wants_file {
        let by_stem = collect(&|n| stem_of(&n.path) == normalized);
        if by_stem.len() == 1 {
            return resolved(by_stem[0].clone());
        }
        if by_stem.len() > 1 {
            return ambiguous(by_stem);
        }
    }

    // 第 5 层：Frontmatter 别名精确匹配。
    let by_alias = collect(&|n| n.doc.aliases.iter().any(|a| a == &normalized));
    if by_alias.len() == 1 {
        return resolved(by_alias[0].clone());
    }
    if by_alias.len() > 1 {
        return ambiguous(by_alias);
    }

    // 第 6 层：大小写不敏感主干 / 文件名 / 别名。
    let lower = normalized.to_lowercase();
    let by_ci = collect(&|n| {
        stem_of(&n.path).to_lowercase() == lower
            || n.file_name.to_lowercase() == lower
            || n.doc.aliases.iter().any(|a| a.to_lowercase() == lower)
    });
    if by_ci.len() == 1 {
        return resolved(by_ci[0].clone());
    }
    if by_ci.len() > 1 {
        return ambiguous(by_ci);
    }

    // 第 7 层：附件。附件不建笔记节点，只记录根内路径。
    let attachment_hits: Vec<&VaultFile> = all_files
        .iter()
        .filter(|f| {
            f.kind != "note"
                && (f.relative_path == normalized || f.file_name == normalized)
        })
        .collect();
    if attachment_hits.len() == 1 {
        return Target {
            path: None,
            status: ATTACHMENT,
            candidates: vec![],
            attachment: Some(attachment_hits[0].relative_path.clone()),
        };
    }
    if attachment_hits.len() > 1 {
        let mut candidates: Vec<String> = attachment_hits
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();
        candidates.sort();
        return ambiguous(candidates);
    }
    Target { path: None, status: UNRESOLVED, candidates: vec![], attachment: None }
}

/// 构建索引的输入选项。
#[derive(Debug, Clone, Default)]
pub struct IndexBuildOptions {
    /// 上一代快照编号；新快照在此基础上加一。
    pub previous_generation: u64,
    /// 本次构建的读取字节预算；None 使用 [`INDEX_MAX_BYTES`]。
    /// 只允许收紧，传入更大的值仍按上限截断。
    pub byte_budget: Option<u64>,
}

/// 从一次扫描结果构建索引。扫描的跳过项、错误和明细截断都会进入 `issues`，
/// 并让快照显式标记为不完整；不会静默丢掉看不见的文件。
pub fn build_from_scan(
    root: &Path,
    scan: &VaultScan,
    options: IndexBuildOptions,
    now: u64,
) -> LinkIndex {
    build_index_inner(root, &scan.files, &scan.root, Some(scan), options, now)
}

/// 从已扫描文件构建索引（不携带扫描级跳过/错误明细的兼容入口）。
/// 读取受 [`INDEX_MAX_BYTES`] 与单篇 512 KiB 上限约束。
pub fn build_index(
    root: &Path,
    files: &[VaultFile],
    root_label: &str,
    options: IndexBuildOptions,
    now: u64,
) -> LinkIndex {
    build_index_inner(root, files, root_label, None, options, now)
}

fn build_index_inner(
    root: &Path,
    files: &[VaultFile],
    root_label: &str,
    scan: Option<&VaultScan>,
    options: IndexBuildOptions,
    now: u64,
) -> LinkIndex {
    let budget = options
        .byte_budget
        .unwrap_or(INDEX_MAX_BYTES)
        .min(INDEX_MAX_BYTES);
    let mut notes: Vec<LoadedNote> = Vec::new();
    let mut issues: Vec<IndexIssue> = Vec::new();
    let mut bytes_read = 0u64;
    let mut warnings = 0usize;
    let mut incomplete: Option<String> = None;
    if let Some(scan) = scan {
        for skipped in &scan.skipped {
            issues.push(IndexIssue {
                relative_path: skipped.relative_path.clone(),
                code: format!("scan_skipped_{}", skipped.reason),
                message: format!("扫描跳过该条目（{}）", skipped.reason),
            });
        }
        for error in &scan.errors {
            issues.push(IndexIssue {
                relative_path: error.relative_path.clone(),
                code: "scan_error".to_string(),
                message: error.message.clone(),
            });
        }
        if scan.stats.truncated {
            incomplete = Some("扫描达到条目上限，文件清单不完整".to_string());
        }
        if scan.stats.details_truncated && incomplete.is_none() {
            incomplete = Some("扫描跳过/错误明细被截断，问题清单不完整".to_string());
        }
    }
    let candidates: Vec<&VaultFile> = files.iter().filter(|f| f.kind == "note").collect();
    let notes_scanned = candidates.len();
    if candidates.len() > INDEX_MAX_NOTES {
        issues.push(IndexIssue {
            relative_path: String::new(),
            code: "note_limit".to_string(),
            message: format!(
                "笔记数 {} 超过索引上限 {}，本次只读取前 {} 篇",
                candidates.len(), INDEX_MAX_NOTES, INDEX_MAX_NOTES
            ),
        });
        incomplete.get_or_insert_with(|| "笔记数达到索引上限，快照不完整".to_string());
    }
    for file in candidates.iter().take(INDEX_MAX_NOTES) {
        let remaining = budget.saturating_sub(bytes_read);
        if remaining == 0 {
            incomplete.get_or_insert_with(|| "读取预算耗尽，快照不完整".to_string());
            break;
        }
        // 扫描后、读取前核对元数据；读取后再次核对，缩小同大小改写的窗口。
        if let Err(message) = metadata_matches_scan(root, file) {
            issues.push(IndexIssue {
                relative_path: file.relative_path.clone(),
                code: "changed_before_read".to_string(),
                message: message.clone(),
            });
            incomplete.get_or_insert_with(|| format!("{} {}", file.relative_path, message));
            break;
        }
        let read = read_vault_note_within(root, &file.relative_path, remaining);
        bytes_read = bytes_read.saturating_add(read.bytes_read);
        if read.budget_exhausted {
            issues.push(IndexIssue {
                relative_path: file.relative_path.clone(),
                code: "budget_exhausted".to_string(),
                message: format!("剩余读取预算 {} 字节，未读入该笔记", remaining),
            });
            incomplete.get_or_insert_with(|| "读取预算耗尽，快照不完整".to_string());
            break;
        }
        let note = match read.note {
            Ok(note) => note,
            Err(message) => {
                issues.push(IndexIssue {
                    relative_path: file.relative_path.clone(),
                    code: "read_failed".to_string(),
                    message: message.clone(),
                });
                incomplete.get_or_insert_with(|| format!("{} 读取失败：{}", file.relative_path, message));
                break;
            }
        };
        // 扫描后内容变化：长度与扫描元数据不一致时拒绝发布完整快照。
        if note.bytes != file.bytes {
            issues.push(IndexIssue {
                relative_path: file.relative_path.clone(),
                code: "changed_after_scan".to_string(),
                message: format!("扫描记录 {} 字节，读取到 {} 字节", file.bytes, note.bytes),
            });
            incomplete.get_or_insert_with(|| format!("{} 在扫描后发生变化", file.relative_path));
            break;
        }
        if let Err(message) = metadata_matches_scan(root, file) {
            issues.push(IndexIssue {
                relative_path: file.relative_path.clone(),
                code: "changed_during_read".to_string(),
                message: message.clone(),
            });
            incomplete.get_or_insert_with(|| format!("{} {}", file.relative_path, message));
            break;
        }
        let doc = parse_note(&note.content);
        warnings += doc.warnings.len();
        notes.push(LoadedNote {
            path: file.relative_path.clone(),
            file_name: file.file_name.clone(),
            sha256: note.sha256.clone(),
            bytes: note.bytes,
            lines: note.content.split_inclusive('\n').count(),
            doc,
        });
        if notes.len() >= INDEX_MAX_NOTES {
            break;
        }
    }
    let note_refs: Vec<&LoadedNote> = notes.iter().collect();
    let mut links: Vec<IndexLink> = Vec::new();
    for note in &notes {
        let report = parse_links_report(&note.doc.body, note.doc.body_start_byte, note.doc.body_start_line);
        for issue in &report.issues {
            issues.push(IndexIssue {
                relative_path: note.path.clone(),
                code: issue.code.clone(),
                message: format!("第 {} 行：{}", issue.line, issue.message),
            });
        }
        if report.truncated {
            incomplete.get_or_insert_with(|| {
                format!("{} 的引用数达到单篇上限 {}", note.path, LINK_MAX_PER_NOTE)
            });
        }
        for (ordinal, r) in report.links.into_iter().enumerate() {
            if links.len() >= INDEX_MAX_LINKS {
                issues.push(IndexIssue {
                    relative_path: note.path.clone(),
                    code: "link_total_limit".to_string(),
                    message: format!("引用总数达到上限 {}", INDEX_MAX_LINKS),
                });
                incomplete.get_or_insert_with(|| "引用总数达到上限，快照不完整".to_string());
                break;
            }
            let target = resolve_target(&r.target, &note.path, &note_refs, files);
            let (anchor_status, anchor_line) = match (&r.anchor, target.path.as_deref()) {
                (Some(anchor), Some(path)) => {
                    if let Some(dest) = notes.iter().find(|n| n.path == path) {
                        let (status, line) = resolve_anchor(&dest.doc, anchor);
                        (Some(status), line)
                    } else {
                        (Some(UNLOCATED.to_string()), None)
                    }
                }
                _ => (None, None),
            };
            links.push(IndexLink {
                id: format!("{}#{}:{}", note.path, r.line, ordinal),
                source_path: note.path.clone(),
                kind: r.kind.clone(),
                target_raw: r.target.clone(),
                display: r.display.clone(),
                anchor: r.anchor.clone(),
                block_anchor: r.block_anchor,
                status: target.status.to_string(),
                target_path: target.path,
                attachment_path: target.attachment,
                candidates: target.candidates,
                start_byte: r.start_byte,
                end_byte: r.end_byte,
                line: r.line,
                context: r.context.clone(),
                anchor_status,
                anchor_line,
            });
        }
    }
    finish_snapshot(
        root_label,
        notes,
        links,
        issues,
        bytes_read,
        notes_scanned,
        warnings,
        budget,
        incomplete,
        options,
        now,
    )
}

/// 比较当前文件元数据与扫描时的长度/修改时间，发现扫描后变化。
///
/// 修改时间只有秒级精度；同秒且同长度的改写无法靠这一步识别，因此快照仍会
/// 携带内容哈希指纹，供调用方比对和后续复核。
fn metadata_matches_scan(root: &Path, file: &VaultFile) -> Result<(), String> {
    let meta = fs::metadata(root.join(&file.relative_path))
        .map_err(|error| format!("无法重新读取文件信息：{}", error))?;
    if !meta.is_file() {
        return Err("文件不再是普通文件".to_string());
    }
    if meta.len() != file.bytes {
        return Err(format!(
            "长度从 {} 字节变为 {} 字节",
            file.bytes,
            meta.len()
        ));
    }
    if let (Some(expected), Ok(actual)) = (file.modified, meta.modified()) {
        let actual = actual
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if actual != expected {
            return Err(format!("修改时间从 {} 变为 {}（Unix 秒）", expected, actual));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_snapshot(
    root_label: &str,
    notes: Vec<LoadedNote>,
    links: Vec<IndexLink>,
    issues: Vec<IndexIssue>,
    bytes_read: u64,
    notes_scanned: usize,
    warnings: usize,
    budget: u64,
    incomplete: Option<String>,
    options: IndexBuildOptions,
    now: u64,
) -> LinkIndex {
    let mut out_notes: Vec<IndexNote> = Vec::new();
    for n in &notes {
        let outgoing = links
            .iter()
            .filter(|l| l.source_path == n.path && l.status == RESOLVED && l.target_path.as_deref() != Some(n.path.as_str()))
            .count();
        let backlink = links
            .iter()
            .filter(|l| l.status == RESOLVED && l.target_path.as_deref() == Some(n.path.as_str()) && l.source_path != n.path)
            .count();
        out_notes.push(IndexNote {
            relative_path: n.path.clone(),
            file_name: n.file_name.clone(),
            sha256: n.sha256.clone(),
            bytes: n.bytes,
            backlink_count: backlink,
            outlink_count: outgoing,
            orphan: backlink == 0,
            lines: n.lines,
            tags: n.doc.tags.clone(),
            aliases: n.doc.aliases.clone(),
        });
    }
    out_notes.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut stats = IndexStats {
        notes: out_notes.len(),
        notes_scanned,
        bytes_read,
        budget_bytes: budget,
        links: links.len(),
        warnings,
        ..Default::default()
    };
    for link in &links {
        match link.status.as_str() {
            RESOLVED => stats.resolved += 1,
            UNRESOLVED => stats.unresolved += 1,
            AMBIGUOUS => stats.ambiguous += 1,
            EXTERNAL => stats.external += 1,
            ATTACHMENT => stats.attachments += 1,
            _ => {}
        }
    }
    stats.orphans = out_notes.iter().filter(|n| n.orphan).count();
    let fingerprint = fingerprint_of(&out_notes);
    LinkIndex {
        generation: options.previous_generation.saturating_add(1),
        built_at: now,
        root: root_label.to_string(),
        complete: incomplete.is_none(),
        incomplete_reason: incomplete,
        notes: out_notes,
        links,
        issues,
        stats,
        fingerprint,
    }
}

/// 快照指纹只由路径与内容哈希决定；同内容重建得到同一指纹。
pub fn fingerprint_of(notes: &[IndexNote]) -> String {
    let mut sorted: Vec<(&str, &str)> = notes.iter().map(|n| (n.relative_path.as_str(), n.sha256.as_str())).collect();
    sorted.sort();
    let mut payload = String::new();
    for (path, sha) in sorted {
        payload.push_str(path);
        payload.push('\0');
        payload.push_str(sha);
        payload.push('\n');
    }
    crate::sha256::sha256_hex(payload.as_bytes())
}

/// 读取单篇笔记的关系视图。
pub fn note_links(index: &LinkIndex, relative_path: &str) -> Option<NoteLinks> {
    let note = index.notes.iter().find(|n| n.relative_path == relative_path)?.clone();
    let outgoing: Vec<IndexLink> = index.links.iter().filter(|l| l.source_path == relative_path).cloned().collect();
    let backlinks: Vec<IndexLink> = index
        .links
        .iter()
        .filter(|l| l.status == RESOLVED && l.target_path.as_deref() == Some(relative_path) && l.source_path != relative_path)
        .cloned()
        .collect();
    Some(NoteLinks {
        generation: index.generation,
        note,
        outgoing,
        backlinks,
    })
}

/// 构建后返回的孤儿笔记路径（按字典序）。
pub fn orphans(index: &LinkIndex) -> Vec<String> {
    index.notes.iter().filter(|n| n.orphan).map(|n| n.relative_path.clone()).collect()
}

/// 供测试与宿主复用的扫描构建入口。
pub fn build_from_vault(
    root: &Path,
    app_root: Option<&Path>,
    options: IndexBuildOptions,
    now: u64,
) -> Result<LinkIndex, String> {
    let scan = scan_vault(root, app_root)?;
    Ok(build_from_scan(root, &scan, options, now))
}

#[cfg(test)]
mod tests;
