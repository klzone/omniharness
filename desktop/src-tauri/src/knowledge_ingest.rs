//! 知识库整理工作流（OD-ARCH-141）。
//!
//! 只负责三件确定性的宿主动作：初始化目录、校验模型返回的结构化结果、逐卡落盘并
//! 更新索引/归档原文。模型没有文件通道；所有路径、标题、长度和来源都在这里复核。

use std::fs;
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::knowledge_governance::{self, GovernanceEntry};
use crate::knowledge_vault::{self, VaultFile, VaultScan};
use crate::profile_rules::RulePolicy;
use crate::vault_note;
use crate::vault_search::{self, SearchQuery};
use crate::vault_write;

pub const KNOWLEDGE_DIR: &str = "知识库";
pub const CONCEPT_DIR: &str = "知识库/概念";
pub const METHOD_DIR: &str = "知识库/方法";
pub const ORIGINAL_DIR: &str = "知识库/原文";
pub const INBOX_DIR: &str = "inbox";
/// 原始资料目录（OD-KB-201 S1）。相对知识库根，可存 PDF 等任意文件。
pub const RAW_DIR: &str = "raw";
pub const RAW_SECTION: &str = "原始资料";
/// 单个 raw 上传文件的读取上限；超限拒绝，不静默截断。
pub const RAW_MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
pub const INDEX_REL: &str = "知识库/索引.md";
/// 卡片正文上限。建卡规范提示模型写 320 字内，宿主保留约 100 字容差。
pub const CARD_MAX_CHARS: usize = 420;
pub const INDEX_TEMPLATE: &str = "# 知识库索引\n\n## 概念\n\n## 方法\n\n## 原始资料\n";

/// 一次 raw 准入请求中的单个文件。
///
/// `name` 只接受一个普通文件名；`bytes` 原样写入 `raw/`。允许非 Markdown
/// 文件（PDF、图片等），因此这里不设扩展名白名单。
#[derive(Debug, Clone, Deserialize)]
pub struct RawFileInput {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmittedRawFile {
    /// IPC 送来的原始文件名；换名保存时索引行记录实际名，治理记录保留原名。
    pub original_name: String,
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
    /// 同名不同内容时是否以「原名-哈希前12位」换名保存。
    pub renamed: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RawAdmitReport {
    pub ok: bool,
    pub admitted: Vec<AdmittedRawFile>,
    pub rejected: Vec<IngestRejection>,
    pub index_updated: bool,
    pub index_path: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestInit {
    pub root: String,
    pub index_path: String,
    pub created_dirs: Vec<String>,
    pub index_created: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestPayload {
    #[serde(default)]
    pub cards: Vec<IngestCard>,
    #[serde(default)]
    pub index_lines: Vec<String>,
    #[serde(default)]
    pub archive_moves: Vec<ArchiveMove>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestCard {
    pub relative_path: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchiveMove {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestRejection {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedCard {
    pub relative_path: String,
    pub title: String,
    pub content: String,
    pub sources: Vec<String>,
    pub index_line: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IngestValidation {
    pub ok: bool,
    pub cards: Vec<ValidatedCard>,
    pub rejected_cards: Vec<IngestRejection>,
    pub index_lines: Vec<String>,
    pub archive_moves: Vec<ArchiveMove>,
    pub rejected_moves: Vec<IngestRejection>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreatedCard {
    pub relative_path: String,
    pub title: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ApplyReport {
    pub ok: bool,
    pub created: Vec<CreatedCard>,
    pub rejected_cards: Vec<IngestRejection>,
    pub index_updated: bool,
    pub moved: Vec<ArchiveMove>,
    pub move_errors: Vec<IngestRejection>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepareFile {
    pub relative_path: String,
    pub chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepareResult {
    pub prompt: String,
    pub files: Vec<PrepareFile>,
    /// 收件箱材料正文（红线扫描对象），与 prompt 来自同一批文件。
    pub user_input_text: String,
    pub index_path: String,
    /// 已有 approve_raw 且哈希未变的 raw 文件数量（不再进入批次）。
    pub skipped_processed: usize,
}

fn join_inbox_text(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(_, content)| content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn clean_rel(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("路径为空".to_string());
    }
    let path = Path::new(text);
    if path.is_absolute() {
        return Err(format!("只接受知识库内相对路径：{text}"));
    }
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy().to_string();
                if name.starts_with('.') {
                    return Err(format!("不能写入隐藏路径：{text}"));
                }
                parts.push(name);
            }
            _ => return Err(format!("路径包含非法片段：{text}")),
        }
    }
    Ok(parts.join("/"))
}

fn is_card_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    parts.len() == 3
        && parts[0] == KNOWLEDGE_DIR
        && (parts[1] == "概念" || parts[1] == "方法")
        && parts[2].ends_with(".md")
}

fn section_of(path: &str) -> &'static str {
    if path.starts_with(CONCEPT_DIR) { "概念" } else { "方法" }
}

fn card_file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn title_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn index_titles(index: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in index.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            continue;
        }
        let start = 3;
        if let Some(end) = trimmed[start..].find(']') {
            out.push(title_key(&trimmed[start..start + end]));
        }
    }
    out
}

fn existing_card_titles(scan: &VaultScan) -> Vec<String> {
    let mut out = Vec::new();
    for file in &scan.files {
        if file.relative_path.starts_with(CONCEPT_DIR) || file.relative_path.starts_with(METHOD_DIR) {
            if let Some(stem) = file.file_name.strip_suffix(".md") {
                out.push(title_key(stem));
            }
        }
    }
    out
}

pub fn init_vault(root: &Path) -> Result<IngestInit, String> {
    let mut created = Vec::new();
    for rel in [
        KNOWLEDGE_DIR,
        CONCEPT_DIR,
        METHOD_DIR,
        ORIGINAL_DIR,
        INBOX_DIR,
        RAW_DIR,
    ] {
        let path = root.join(rel);
        if !path.is_dir() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("创建目录失败（{}）：{}", path.display(), e))?;
            created.push(rel.to_string());
        }
    }
    let index = root.join(INDEX_REL);
    let index_created = if index.is_file() {
        false
    } else {
        fs::write(&index, INDEX_TEMPLATE)
            .map_err(|e| format!("写入索引失败（{}）：{}", index.display(), e))?;
        true
    };
    Ok(IngestInit {
        root: root.display().to_string(),
        index_path: index.display().to_string(),
        created_dirs: created,
        index_created,
    })
}

/// 把 Unix 秒按固定分钟偏移换算成 RFC3339 本地时间。
///
/// 纯函数，不读系统时区、不引时间库；负偏移与纪元前时间都按 civil 历法算。
pub fn rfc3339_local(unix_seconds: i64, utc_offset_minutes: i32) -> String {
    let shifted = unix_seconds + i64::from(utc_offset_minutes) * 60;
    let days = shifted.div_euclid(86_400);
    let seconds_of_day = shifted.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (offset_sign, offset_abs) = if utc_offset_minutes < 0 {
        ('-', -i64::from(utc_offset_minutes))
    } else {
        ('+', i64::from(utc_offset_minutes))
    };
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{offset_sign}{:02}:{:02}",
        offset_abs / 60,
        offset_abs % 60
    )
}

/// Howard Hinnant 的 days-from-civil 逆变换；`days` 以 1970-01-01 为 0。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 校验 raw 文件名：只许一个普通文件名，拒绝路径、隐藏名、空名与会破坏
/// 索引 Markdown 链接的字符。
fn validate_raw_name(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("文件名不能为空".to_string());
    }
    if text != raw {
        return Err(format!("文件名首尾不能有空白：{raw:?}"));
    }
    if text == "." || text == ".." || text.starts_with('.') {
        return Err(format!("不能使用隐藏文件名：{text}"));
    }
    if text.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '/' | '\\' | ':' | '<' | '>' | '|' | '?' | '*' | '[' | ']' | '(' | ')'
            )
    }) {
        return Err(format!("文件名含非法字符：{text}"));
    }
    let path = Path::new(text);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) if name.to_string_lossy() == text => Ok(text.to_string()),
        _ => Err(format!("只接受单个普通文件名：{text}")),
    }
}

fn raw_section_range(index: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = index.lines().collect();
    let heading = format!("## {RAW_SECTION}");
    let start = lines
        .iter()
        .position(|line| line.trim() == heading.as_str())?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("## "))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());
    Some((start, end))
}

fn line_sha256(line: &str) -> Option<String> {
    let rest = line
        .split_once("sha256：")
        .or_else(|| line.split_once("sha256:"))
        .map(|(_, rest)| rest)?;
    let token: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    (token.len() == 64).then(|| token.to_ascii_lowercase())
}

fn raw_section_hashes(index: &str) -> Vec<String> {
    let Some((start, end)) = raw_section_range(index) else {
        return Vec::new();
    };
    index
        .lines()
        .skip(start + 1)
        .take(end.saturating_sub(start + 1))
        .filter_map(line_sha256)
        .collect()
}

fn renamed_raw_name(name: &str, hash: &str) -> String {
    let short = &hash[..hash.len().min(12)];
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{short}.{ext}"),
        _ => format!("{name}-{short}"),
    }
}

fn raw_index_line(name: &str, hash: &str, bytes: u64, admitted_at: &str) -> String {
    // 用尖括号包住链接目标，文件名含空格时仍是合法 Markdown 链接。
    format!(
        "- [{name}](<{RAW_DIR}/{name}>) | 准入时间：{admitted_at} | sha256：{hash} | {bytes} 字节"
    )
}

fn append_raw_index_lines(index: &str, additions: &[String]) -> String {
    let mut lines: Vec<String> = index.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.extend(INDEX_TEMPLATE.lines().map(str::to_string));
    }
    let fresh: Vec<String> = additions
        .iter()
        .filter(|line| !lines.iter().any(|existing| existing.trim() == line.trim()))
        .cloned()
        .collect();
    if fresh.is_empty() {
        let mut unchanged = lines.join("\n");
        if !unchanged.ends_with('\n') {
            unchanged.push('\n');
        }
        return unchanged;
    }
    let heading = format!("## {RAW_SECTION}");
    if let Some(pos) = lines.iter().position(|line| line.trim() == heading) {
        let mut insert = pos + 1;
        while insert < lines.len() && !lines[insert].trim_start().starts_with("## ") {
            insert += 1;
        }
        // 追加到本节最后一个非空行之后，避免每条资料之间被空行隔开。
        while insert > pos + 1 && lines[insert - 1].trim().is_empty() {
            insert -= 1;
        }
        lines.splice(insert..insert, fresh);
    } else {
        if lines.last().map(|line| !line.trim().is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push(heading);
        lines.extend(fresh);
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

static RAW_TEMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn raw_temp_path(dir: &Path, name: &str) -> std::path::PathBuf {
    use std::sync::atomic::Ordering;
    let stamp = format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        RAW_TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    dir.join(format!(".{name}.{stamp}.tmp"))
}

/// raw 专用新建写入：临时文件 + fsync + 硬链接发布；目标已存在时硬链接原子失败，
/// 绝不覆盖。失败会清理临时文件。
fn write_raw_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("raw 目标没有父目录：{}", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "raw".to_string());
    let temp = raw_temp_path(dir, &name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("无法创建 raw 临时文件（{}）：{}", temp.display(), e))?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(format!("写入 raw 临时文件失败（{}）：{}", temp.display(), e));
    }
    // Windows 下已打开的文件不能被删除；发布/清理临时文件前先关闭句柄。
    drop(file);
    let result = match fs::hard_link(&temp, target) {
        Ok(()) => {
            let _ = fs::File::open(dir).and_then(|dir| dir.sync_all());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(format!("目标已存在，raw 新建不会覆盖：{}", target.display()))
        }
        Err(e) => Err(format!(
            "发布 raw 文件失败（{}）：{}；目标未被改动",
            target.display(),
            e
        )),
    };
    let _ = fs::remove_file(&temp);
    result
}

fn existing_file_hash(path: &Path) -> Result<String, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("无法确认 raw 目标状态（{}）：{}", path.display(), e))?;
    if meta.file_type().is_symlink() {
        return Err(format!("raw 目标位置是符号链接，拒绝写入：{}", path.display()));
    }
    if !meta.is_file() {
        return Err(format!("raw 目标位置不是普通文件：{}", path.display()));
    }
    let bytes = fs::read(path)
        .map_err(|e| format!("读取既有 raw 文件失败（{}）：{}", path.display(), e))?;
    Ok(crate::sha256::sha256_hex(&bytes))
}

/// 把一批原始资料准入到 `raw/`，并记录到索引「原始资料」小节。
///
/// 规则：同哈希拒绝；同名不同内容换名为「原名-哈希前12位」；目标永不覆盖。
/// 索引保存失败时回滚本轮新建的 raw 文件。
pub fn admit_raw_files(
    root: &Path,
    files: &[RawFileInput],
    utc_offset_minutes: i32,
) -> RawAdmitReport {
    admit_raw_files_with_policy(root, files, &RulePolicy::clean(), utc_offset_minutes)
}

/// 从文件选择框给出的绝对路径读取 raw 输入字节。只读普通文件，超限拒绝。
pub fn read_raw_file(path: &Path) -> Result<RawFileInput, String> {
    if !path.is_absolute() {
        return Err(format!("raw 上传只接受绝对路径：{}", path.display()));
    }
    let meta = fs::metadata(path)
        .map_err(|e| format!("无法读取文件信息（{}）：{}", path.display(), e))?;
    if !meta.is_file() {
        return Err(format!("不是普通文件，拒绝上传：{}", path.display()));
    }
    if meta.len() > RAW_MAX_INPUT_BYTES {
        return Err(format!(
            "文件 {} 字节，超过 raw 上传上限 {} 字节：{}",
            meta.len(),
            RAW_MAX_INPUT_BYTES,
            path.display()
        ));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| format!("路径没有文件名：{}", path.display()))?;
    let bytes = fs::read(path)
        .map_err(|e| format!("读取上传文件失败（{}）：{}", path.display(), e))?;
    Ok(RawFileInput { name, bytes })
}

/// 带 Profile 隐私策略的准入入口；`knowledge_raw_admit` 走这一条，使治理记录
/// 与工作记录使用同一套 mask_privacy 约束。
pub fn admit_raw_files_with_policy(
    root: &Path,
    files: &[RawFileInput],
    policy: &RulePolicy,
    utc_offset_minutes: i32,
) -> RawAdmitReport {
    let index_path = root.join(INDEX_REL);
    let mut report = RawAdmitReport {
        index_path: index_path.display().to_string(),
        ..Default::default()
    };
    let raw_dir = root.join(RAW_DIR);
    match fs::symlink_metadata(&raw_dir) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
        Ok(_) => {
            report.errors.push(format!(
                "raw 目录不是普通目录（{}）；请先初始化知识库",
                raw_dir.display()
            ));
            return report;
        }
        Err(e) => {
            report.errors.push(format!(
                "raw 目录不存在（{}）：{}；请先初始化知识库",
                raw_dir.display(),
                e
            ));
            return report;
        }
    }
    match fs::symlink_metadata(&index_path) {
        Ok(meta) if meta.file_type().is_file() && !meta.file_type().is_symlink() => {}
        Ok(_) => {
            report.errors.push(format!(
                "索引不是普通文件（{}）",
                index_path.display()
            ));
            return report;
        }
        Err(e) => {
            report.errors.push(format!(
                "读取索引失败（{}）：{}",
                index_path.display(),
                e
            ));
            return report;
        }
    }
    let index = match fs::read_to_string(&index_path) {
        Ok(index) => index,
        Err(e) => {
            report.errors.push(format!(
                "读取索引失败（{}）：{}",
                index_path.display(),
                e
            ));
            return report;
        }
    };
    let mut known_hashes = raw_section_hashes(&index);
    let now = now_unix_seconds();
    let admitted_at = rfc3339_local(now, utc_offset_minutes);
    let mut index_additions = Vec::new();
    let mut created_paths = Vec::new();

    for file in files {
        let name = match validate_raw_name(&file.name) {
            Ok(name) => name,
            Err(reason) => {
                report.rejected.push(IngestRejection {
                    relative_path: file.name.clone(),
                    reason,
                });
                continue;
            }
        };
        let hash = crate::sha256::sha256_hex(&file.bytes);
        if known_hashes.iter().any(|existing| existing == &hash) {
            report.rejected.push(IngestRejection {
                relative_path: format!("{RAW_DIR}/{name}"),
                reason: "重复资料：索引里已有相同 sha256".to_string(),
            });
            continue;
        }

        let original = raw_dir.join(&name);
        let mut final_name = name.clone();
        let mut renamed = false;
        if original.exists() || fs::symlink_metadata(&original).is_ok() {
            match existing_file_hash(&original) {
                Ok(existing) if existing == hash => {
                    report.rejected.push(IngestRejection {
                        relative_path: format!("{RAW_DIR}/{name}"),
                        reason: "重复资料：raw 里已有相同内容的同名文件".to_string(),
                    });
                    continue;
                }
                Ok(_) => {
                    final_name = renamed_raw_name(&name, &hash);
                    renamed = true;
                }
                Err(reason) => {
                    report.rejected.push(IngestRejection {
                        relative_path: format!("{RAW_DIR}/{name}"),
                        reason,
                    });
                    continue;
                }
            }
        }

        let target = raw_dir.join(&final_name);
        if fs::symlink_metadata(&target).is_ok() {
            report.rejected.push(IngestRejection {
                relative_path: format!("{RAW_DIR}/{final_name}"),
                reason: "换名后的 raw 目标已存在，拒绝覆盖".to_string(),
            });
            continue;
        }
        if let Err(reason) = write_raw_atomically(&target, &file.bytes) {
            report.rejected.push(IngestRejection {
                relative_path: format!("{RAW_DIR}/{final_name}"),
                reason,
            });
            continue;
        }

        let bytes = file.bytes.len() as u64;
        created_paths.push(target);
        known_hashes.push(hash.clone());
        index_additions.push(raw_index_line(&final_name, &hash, bytes, &admitted_at));
        report.admitted.push(AdmittedRawFile {
            original_name: name,
            relative_path: format!("{RAW_DIR}/{final_name}"),
            sha256: hash,
            bytes,
            renamed,
        });
    }

    if !index_additions.is_empty() {
        let next = append_raw_index_lines(&index, &index_additions);
        let expected = vault_write::content_sha256(&index);
        match vault_write::save_note(root, INDEX_REL, &next, &expected) {
            Ok(_) => report.index_updated = true,
            Err(e) => {
                report.errors.push(format!("索引更新失败：{e}"));
                for path in &created_paths {
                    let _ = fs::remove_file(path);
                }
                report.admitted.clear();
            }
        }
    }
    for admitted in &report.admitted {
        let entry = GovernanceEntry {
            action: "admit".to_string(),
            subject: admitted.relative_path.clone(),
            sha256: admitted.sha256.clone(),
            note: format!("原名：{}；{} 字节", admitted.original_name, admitted.bytes),
        };
        if let Err(e) = knowledge_governance::append_record_at(
            root,
            knowledge_governance::KIND_RAW_ADMISSIONS,
            &entry,
            policy,
            now,
            utc_offset_minutes,
        ) {
            report.errors.push(format!(
                "治理准入记录写入失败（{}）：{}",
                admitted.relative_path, e
            ));
        }
    }
    report.ok = report.errors.is_empty() && !report.admitted.is_empty();
    report
}

pub fn validate_payload(result: &Value, index: &str, scan: &VaultScan) -> IngestValidation {
    let mut report = IngestValidation::default();
    let payload: IngestPayload = match serde_json::from_value(result.clone()) {
        Ok(value) => value,
        Err(e) => {
            report.errors.push(format!("整理结果不是约定的 JSON：{e}"));
            return report;
        }
    };
    let mut known_titles = index_titles(index);
    known_titles.extend(existing_card_titles(scan));
    let source_paths: Vec<&str> = scan
        .files
        .iter()
        .filter(|f: &&VaultFile| {
            f.relative_path.starts_with("inbox/") || f.relative_path.starts_with("raw/")
        })
        .map(|f| f.relative_path.as_str())
        .collect();
    let mut seen_titles = Vec::new();
    let mut used_index_lines = Vec::new();

    for card in payload.cards {
        let path = match clean_rel(&card.relative_path) {
            Ok(path) => path,
            Err(reason) => {
                report.rejected_cards.push(IngestRejection {
                    relative_path: card.relative_path,
                    reason,
                });
                continue;
            }
        };
        if !is_card_path(&path) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "卡片只能落在 知识库/概念/ 或 知识库/方法/".to_string(),
            });
            continue;
        }
        let title = card.title.trim().to_string();
        if title.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "标题不能为空".to_string(),
            });
            continue;
        }
        let key = title_key(&title);
        if known_titles.iter().any(|t| t == &key) || seen_titles.iter().any(|t| t == &key) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("标题重复：{title}"),
            });
            continue;
        }
        let body_chars = vault_note::parse_note(&card.content).body.chars().count();
        if body_chars > CARD_MAX_CHARS {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("正文 {body_chars} 字，超过 {CARD_MAX_CHARS} 字上限"),
            });
            continue;
        }
        if card.sources.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "sources 不能为空".to_string(),
            });
            continue;
        }
        let mut missing = Vec::new();
        for source in &card.sources {
            let source = match clean_rel(source) {
                Ok(value) => value,
                Err(_) => {
                    missing.push(source.clone());
                    continue;
                }
            };
            if !source_paths.iter().any(|p| p == &source) {
                missing.push(source);
            }
        }
        if !missing.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("来源不存在：{}", missing.join(", ")),
            });
            continue;
        }
        let file_name = card_file_name(&path);
        let index_line = payload
            .index_lines
            .iter()
            .find(|line| line.contains(&title) && line.contains(file_name))
            .cloned();
        let Some(index_line) = index_line else {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "缺少与卡片标题/文件名对应的索引行".to_string(),
            });
            continue;
        };
        if used_index_lines.iter().any(|line| line == &index_line) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "索引行被多张卡重复使用".to_string(),
            });
            continue;
        }
        seen_titles.push(key);
        used_index_lines.push(index_line.clone());
        report.cards.push(ValidatedCard {
            relative_path: path,
            title,
            content: card.content,
            sources: card.sources,
            index_line: index_line.clone(),
        });
        report.index_lines.push(index_line);
    }

    for mv in payload.archive_moves {
        let from = clean_rel(&mv.from);
        let to = clean_rel(&mv.to);
        match (from, to) {
            (Ok(from), Ok(to))
                if from.starts_with("inbox/")
                    && to.starts_with("知识库/原文/")
                    && (from.ends_with(".md") || from.ends_with(".txt"))
                    && !source_paths.iter().any(|p| p == &to) =>
            {
                if source_paths.iter().any(|p| p == &from) {
                    report.archive_moves.push(ArchiveMove { from, to });
                } else {
                    report.rejected_moves.push(IngestRejection {
                        relative_path: mv.from,
                        reason: "待归档文件不在扫描结果里".to_string(),
                    });
                }
            }
            (from, to) => report.rejected_moves.push(IngestRejection {
                relative_path: mv.from,
                reason: format!(
                    "归档路径不合法（{} → {}）",
                    from.unwrap_or_else(|e| e),
                    to.unwrap_or_else(|e| e)
                ),
            }),
        }
    }

    report.ok = report.errors.is_empty()
        && !report.cards.is_empty()
        && report.rejected_cards.is_empty()
        && report.rejected_moves.is_empty();
    report
}

fn append_index_lines(index: &str, entries: &[(String, String)]) -> String {
    let mut lines: Vec<String> = index.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.extend(INDEX_TEMPLATE.lines().map(str::to_string));
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

pub fn apply_payload(root: &Path, result: &Value, scan: &VaultScan) -> ApplyReport {
    let index = fs::read_to_string(root.join(INDEX_REL)).unwrap_or_else(|_| INDEX_TEMPLATE.to_string());
    let validation = validate_payload(result, &index, scan);
    let mut report = ApplyReport {
        rejected_cards: validation.rejected_cards.clone(),
        errors: validation.errors.clone(),
        ..Default::default()
    };
    let mut entries: Vec<(String, String)> = Vec::new();
    for card in &validation.cards {
        // 审核通过后的归档会把 inbox/ 原文移到 知识库/原文/；卡片 frontmatter 里的
        // sources 同步改写，避免刚建好的卡片引用指向不存在的 inbox 路径。
        let content = rewrite_archived_sources(&card.content, &validation.archive_moves);
        match vault_write::create_note(root, &card.relative_path, &content) {
            Ok(note) => {
                report.created.push(CreatedCard {
                    relative_path: card.relative_path.clone(),
                    title: card.title.clone(),
                    bytes: note.bytes,
                });
                entries.push((section_of(&card.relative_path).to_string(), card.index_line.clone()));
            }
            Err(e) => report.rejected_cards.push(IngestRejection {
                relative_path: card.relative_path.clone(),
                reason: e,
            }),
        }
    }
    if !entries.is_empty() {
        let next = append_index_lines(&index, &entries);
        let expected = vault_write::content_sha256(&index);
        match vault_write::save_note(root, INDEX_REL, &next, &expected) {
            Ok(_) => report.index_updated = true,
            Err(e) => {
                report.errors.push(format!("索引更新失败：{e}"));
                for card in &report.created {
                    let _ = fs::remove_file(root.join(&card.relative_path));
                }
                report.created.clear();
            }
        }
    }
    if !entries.is_empty() && !report.index_updated {
        for mv in &validation.archive_moves {
            report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: "索引未更新，原文未归档".to_string(),
            });
        }
    } else {
    for mv in &validation.archive_moves {
        let source = match knowledge_vault::resolve_vault_file(root, &mv.from) {
            Ok(path) => path,
            Err(e) => {
                report.move_errors.push(IngestRejection {
                    relative_path: mv.from.clone(),
                    reason: e,
                });
                continue;
            }
        };
        let target = root.join(&mv.to);
        if target.exists() {
            report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: format!("归档目标已存在：{}", mv.to),
            });
            continue;
        }
        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                report.move_errors.push(IngestRejection {
                    relative_path: mv.from.clone(),
                    reason: format!("创建归档目录失败：{e}"),
                });
                continue;
            }
        }
        match fs::rename(&source, &target) {
            Ok(_) => report.moved.push(mv.clone()),
            Err(e) => report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: format!("移动原文失败：{e}"),
            }),
        }
    }
    }
    report.ok = report.errors.is_empty() && !report.created.is_empty();
    report
}

fn rewrite_archived_sources(content: &str, moves: &[ArchiveMove]) -> String {
    let mut out = content.to_string();
    for mv in moves {
        out = out.replace(&mv.from, &mv.to);
    }
    out
}

/// 整理批次选材：inbox/ 全部优先；raw/ 仅可读文本，排除已 approve 且哈希未变的文件；
/// 其余按修改时间新→旧；最多 5 份。
fn select_ingest_notes(root: &Path, scan: &VaultScan) -> Result<(Vec<VaultFile>, usize), String> {
    let mut inbox: Vec<VaultFile> = scan
        .files
        .iter()
        .filter(|file| file.kind == "note" && file.relative_path.starts_with("inbox/"))
        .cloned()
        .collect();
    inbox.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let admissions = knowledge_governance::list_records(root, knowledge_governance::KIND_RAW_ADMISSIONS)
        .map_err(|e| format!("读取 raw 准入台账失败：{e}"))?;
    let decisions = knowledge_governance::list_records(root, knowledge_governance::KIND_REVIEW_DECISIONS)
        .map_err(|e| format!("读取审核台账失败：{e}"))?;
    let mut admitted: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();
    for record in admissions.iter() {
        if record.action == "admit" {
            admitted.insert(
                record.subject.clone(),
                (record.sha256.to_ascii_lowercase(), record.at.clone()),
            );
        }
    }
    let mut approved: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for record in decisions.iter().filter(|record| record.action == "approve_raw") {
        approved.insert(record.subject.clone(), record.sha256.to_ascii_lowercase());
    }
    // 兼容旧版：从 approve_card 对应的卡片 frontmatter.sources 里回收 raw 来源。
    let mut approved_card_sources: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for record in decisions.iter() {
        if record.action != "approve_card" {
            continue;
        }
        let Ok(note) = knowledge_vault::read_vault_note(root, &record.subject) else {
            continue;
        };
        let parsed = vault_note::parse_note(&note.content);
        for property in &parsed.properties {
            if !property.key.eq_ignore_ascii_case("sources") {
                continue;
            }
            let values: Vec<String> = match &property.value {
                vault_note::PropertyValue::List(items) => items.clone(),
                vault_note::PropertyValue::Text(text) => text
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .collect(),
                _ => Vec::new(),
            };
            for value in values {
                if value.starts_with("raw/") {
                    approved_card_sources
                        .entry(value)
                        .and_modify(|current| {
                            if record.at > *current {
                                *current = record.at.clone();
                            }
                        })
                        .or_insert_with(|| record.at.clone());
                }
            }
        }
    }
    let mut skipped_processed = 0usize;
    let mut raw: Vec<VaultFile> = scan
        .files
        .iter()
        .filter(|file| file.kind == "note" && file.relative_path.starts_with("raw/"))
        .cloned()
        .collect();
    raw.retain(|file| {
        let Some((admission_hash, admission_at)) = admitted.get(&file.relative_path) else {
            return true;
        };
        let current = fs::read(root.join(&file.relative_path))
            .map(|bytes| crate::sha256::sha256_hex(&bytes))
            .ok();
        let Some(current) = current else {
            return true;
        };
        let prefix: String = current.chars().take(12).collect();
        if admission_hash != &prefix {
            return true;
        }
        let approved_hash = approved
            .get(&file.relative_path)
            .map(|value| value.as_str() == prefix)
            .unwrap_or(false);
        let approved_card = approved_card_sources
            .get(&file.relative_path)
            .map(|approved_at| approved_at > admission_at)
            .unwrap_or(false);
        if approved_hash || approved_card {
            skipped_processed += 1;
            false
        } else {
            true
        }
    });
    raw.sort_by(|a, b| {
        b.modified
            .unwrap_or(0)
            .cmp(&a.modified.unwrap_or(0))
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    inbox.extend(raw);
    inbox.truncate(5);
    Ok((inbox, skipped_processed))
}

pub fn prepare_inbox(
    root: &Path,
    dsh_home: &Path,
    python: &str,
    engine_dir: &Path,
    dictionary_path: Option<&Path>,
) -> Result<PrepareResult, String> {
    let scan = knowledge_vault::scan_vault(root, None)?;
    let (inbox, skipped_processed) = select_ingest_notes(root, &scan)?;
    if inbox.is_empty() {
        return Err(if skipped_processed > 0 {
            format!(
                "没有新的可整理材料；已处理跳过 {skipped_processed} 份（raw 内容未变，或已审核入卡）。"
            )
        } else {
            "inbox/ 与 raw/ 没有可整理的 Markdown/文本材料。".to_string()
        });
    }
    let mut files = Vec::new();
    for file in &inbox {
        let note = knowledge_vault::read_vault_note(root, &file.relative_path)?;
        files.push((file.relative_path.clone(), note.content));
    }
    let index_path = root.join(INDEX_REL);
    if !index_path.is_file() {
        init_vault(root)?;
    }
    let index = fs::read_to_string(&index_path)
        .map_err(|e| format!("读取索引失败（{}）：{}", index_path.display(), e))?;
    let mut spec_parts = Vec::new();
    for relative in [
        "knowledge-agent/SKILL.md",
        "knowledge-search/SKILL.md",
    ] {
        let path = dsh_home.join("skills").join(relative);
        if let Ok(text) = fs::read_to_string(&path) {
            spec_parts.push(text);
        }
    }
    if spec_parts.is_empty() {
        return Err(format!(
            "找不到建卡规范：{} / {}（请先完成首次启动的技能播种）",
            dsh_home.join("skills/knowledge-agent/SKILL.md").display(),
            dsh_home.join("skills/knowledge-search/SKILL.md").display()
        ));
    }
    let card_spec = spec_parts.join("\n\n---\n\n");
    let payload = serde_json::json!({
        "files": files.iter().map(|(path, content)| serde_json::json!({
            "relative_path": path,
            "content": content,
        })).collect::<Vec<_>>(),
        "index": index,
        "card_spec": card_spec,
        "dictionary_path": dictionary_path.map(|p| p.display().to_string()),
    });
    let mut child = Command::new(python)
        .arg("-m")
        .arg("omnidesk_engine.knowledge_ingest")
        .current_dir(engine_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动整理提示词组装失败（{}）：{}", python, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|e| format!("写入整理提示词请求失败：{}", e))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待整理提示词组装失败：{}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(format!("整理提示词组装失败：{}", tail));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("整理提示词组装返回的不是 JSON：{}", e))?;
    let prompt = value
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| "整理提示词组装结果缺少 prompt".to_string())?;
    Ok(PrepareResult {
        prompt: prompt.to_string(),
        files: files
            .iter()
            .map(|(path, content)| PrepareFile {
                relative_path: path.clone(),
                chars: content.chars().count(),
            })
            .collect(),
        user_input_text: join_inbox_text(&files),
        index_path: index_path.display().to_string(),
        skipped_processed,
    })
}

pub fn parse_result_text(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    let body = if trimmed.starts_with("```") {
        let first_newline = trimmed.find('\n').ok_or_else(|| "模型返回了空代码围栏".to_string())?;
        let rest = &trimmed[first_newline + 1..];
        rest.rsplit_once("```")
            .map(|(inner, _)| inner.trim())
            .ok_or_else(|| "模型返回的代码围栏没有闭合".to_string())?
    } else {
        trimmed
    };
    serde_json::from_str(body).map_err(|e| format!("整理结果不是合法 JSON：{}", e))
}

/// S7 冒烟：只用隔离根与合成材料，验证建卡后能被现有预算检索命中。
pub fn smoke_search(root: &Path, keyword: &str) -> Result<usize, String> {
    let response = vault_search::search_vault(
        root,
        &SearchQuery {
            keyword: keyword.to_string(),
            tags: Vec::new(),
            properties: Vec::new(),
        },
    )?;
    Ok(response.hits.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-ingest-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn seed_source(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn raw(name: &str, bytes: &[u8]) -> RawFileInput {
        RawFileInput {
            name: name.to_string(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn inbox_scan_text_is_joined_from_same_batch() {
        let files = vec![
            ("inbox/a.md".to_string(), "合成甲".to_string()),
            ("inbox/b.md".to_string(), "合成乙".to_string()),
        ];
        assert_eq!(join_inbox_text(&files), "合成甲\n\n合成乙");
    }

    #[test]
    fn ingest_batch_includes_inbox_then_raw_text_and_caps_five() {
        let root = root("batch-select");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/a.md", "a");
        seed_source(&root, "inbox/b.md", "b");
        seed_source(&root, "raw/c.md", "c");
        seed_source(&root, "raw/d.txt", "d");
        seed_source(&root, "raw/e.md", "e");
        seed_source(&root, "raw/f.md", "f");
        seed_source(&root, "raw/g.pdf", "g");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let (selected, skipped) = select_ingest_notes(&root, &scan).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(selected.len(), 5);
        assert_eq!(selected[0].relative_path, "inbox/a.md");
        assert_eq!(selected[1].relative_path, "inbox/b.md");
        assert!(selected.iter().any(|file| file.relative_path.starts_with("raw/")));
        assert!(selected.iter().all(|file| !file.relative_path.ends_with(".pdf")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ingest_batch_skips_approved_raw_until_hash_changes() {
        let root = root("processed-raw");
        init_vault(&root).unwrap();
        let report = admit_raw_files(&root, &[raw("processed.md", b"v1")], 480);
        assert!(report.ok, "{:?}", report.rejected);
        let hash: String = crate::sha256::sha256_hex(b"v1").chars().take(12).collect();
        let approve = GovernanceEntry {
            action: "approve_raw".to_string(),
            subject: "raw/processed.md".to_string(),
            sha256: hash,
            note: "synthetic".to_string(),
        };
        knowledge_governance::append_record_at(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
            &approve,
            &RulePolicy::clean(),
            crate::task_shell::now_secs() as i64 + 1,
            480,
        )
        .unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let (selected, skipped) = select_ingest_notes(&root, &scan).unwrap();
        assert_eq!(skipped, 1);
        assert!(selected.is_empty());

        fs::write(root.join("raw/processed.md"), b"v2").unwrap();
        seed_source(&root, "raw/no-record.md", "fresh");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let (selected, skipped) = select_ingest_notes(&root, &scan).unwrap();
        assert_eq!(skipped, 0);
        assert!(selected.iter().any(|file| file.relative_path == "raw/processed.md"));
        assert!(selected.iter().any(|file| file.relative_path == "raw/no-record.md"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ingest_batch_recognizes_approved_card_sources() {
        let root = root("processed-card-source");
        init_vault(&root).unwrap();
        let report = admit_raw_files(&root, &[raw("legacy.md", b"legacy")], 480);
        assert!(report.ok, "{:?}", report.rejected);
        seed_source(
            &root,
            "知识库/概念/旧审核卡.md",
            "---\ntags: [synthetic]\nsources: [raw/legacy.md]\n---\nSynthetic.",
        );
        let approve = GovernanceEntry {
            action: "approve_card".to_string(),
            subject: "知识库/概念/旧审核卡.md".to_string(),
            sha256: crate::sha256::sha256_hex(b"legacy")[..12].to_string(),
            note: "synthetic".to_string(),
        };
        knowledge_governance::append_record_at(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
            &approve,
            &RulePolicy::clean(),
            crate::task_shell::now_secs() as i64 + 1,
            480,
        )
        .unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let (selected, skipped) = select_ingest_notes(&root, &scan).unwrap();
        assert_eq!(skipped, 1);
        assert!(selected.is_empty());
        fs::write(root.join("raw/legacy.md"), b"legacy-v2").unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let (selected, skipped) = select_ingest_notes(&root, &scan).unwrap();
        assert_eq!(skipped, 0);
        assert!(selected.iter().any(|file| file.relative_path == "raw/legacy.md"));
        let _ = fs::remove_dir_all(&root);
    }

    fn card_payload(index_line: &str) -> Value {
        serde_json::json!({
            "cards": [{
                "relative_path": "知识库/概念/专项检查频次.md",
                "title": "专项检查频次",
                "content": "---\ntags: [监督, 专项检查]\nsources: [inbox/合成制度.md]\ndate: 2026-11-05\n---\n一类单位每季度不少于一次。",
                "sources": ["inbox/合成制度.md"]
            }],
            "index_lines": [index_line],
            "archive_moves": []
        })
    }

    #[test]
    fn init_creates_skeleton_and_is_idempotent() {
        let root = root("init");
        let first = init_vault(&root).unwrap();
        assert!(root.join(CONCEPT_DIR).is_dir());
        assert!(root.join(METHOD_DIR).is_dir());
        assert!(root.join(ORIGINAL_DIR).is_dir());
        assert!(root.join(INBOX_DIR).is_dir());
        assert!(root.join(RAW_DIR).is_dir());
        assert!(root.join(INDEX_REL).is_file());
        assert_eq!(first.created_dirs.len(), 6);
        let template = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        assert!(template.contains("## 原始资料"), "{template}");
        let before = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let second = init_vault(&root).unwrap();
        assert!(second.created_dirs.is_empty());
        assert!(!second.index_created);
        assert_eq!(fs::read_to_string(root.join(INDEX_REL)).unwrap(), before);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_accepts_legal_card_and_sources() {
        let root = root("valid");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let line = "- [专项检查频次](概念/专项检查频次.md) | 标签：监督, 专项检查 | 每季度一次";
        let report = validate_payload(&card_payload(line), &index, &scan);
        assert!(report.ok, "{:?}", report.rejected_cards);
        assert_eq!(report.cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_path_outside_card_dirs() {
        let root = root("badpath");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["relative_path"] = serde_json::json!("笔记/专项检查频次.md");
        let report = validate_payload(&payload, &index, &scan);
        assert!(!report.rejected_cards.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_duplicate_title_in_index() {
        let root = root("duptitle");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = "# 知识库索引\n\n## 概念\n- [专项检查频次](概念/旧卡.md) | 标签：监督 | 旧卡\n";
        let report = validate_payload(&card_payload("- [专项检查频次](概念/专项检查频次.md) | x"), index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_duplicate_title_within_batch() {
        let root = root("dupbatch");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        let mut second = payload["cards"][0].clone();
        second["relative_path"] = serde_json::json!("知识库/方法/专项检查频次.md");
        payload["cards"].as_array_mut().unwrap().push(second);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.cards.len(), 1);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_body_over_host_limit() {
        let root = root("long");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["content"] = serde_json::json!(format!("---\ntags: [监督]\n---\n{}", "长".repeat(CARD_MAX_CHARS + 1)));
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_accepts_body_at_host_limit() {
        let root = root("limit-ok");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["content"] = serde_json::json!(format!("---\ntags: [监督]\n---\n{}", "长".repeat(CARD_MAX_CHARS)));
        let report = validate_payload(&payload, &index, &scan);
        assert!(report.ok, "{:?}", report.rejected_cards);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_accepts_raw_attachment_as_source() {
        let root = root("rawsource");
        init_vault(&root).unwrap();
        seed_source(&root, "raw/合成制度.pdf", "synthetic pdf");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["sources"] = serde_json::json!(["raw/合成制度.pdf"]);
        payload["cards"][0]["content"] = serde_json::json!("---\ntags: [监督]\nsources: [raw/合成制度.pdf]\n---\nSynthetic.");
        let report = validate_payload(&payload, &index, &scan);
        assert!(report.ok, "{:?}", report.rejected_cards);
        assert_eq!(report.cards[0].sources, vec!["raw/合成制度.pdf"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_accepts_three_split_cards_with_cross_references() {
        let root = root("split-cards");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/拆分来源.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let parts = ["上", "中", "下"];
        let mut cards = Vec::new();
        let mut lines = Vec::new();
        for position in 0..parts.len() {
            let suffix = parts[position];
            let title = format!("拆分主题（{suffix}）");
            let path = format!("知识库/概念/{title}.md");
            let file = format!("{title}.md");
            let see = parts
                .iter()
                .filter(|other| **other != suffix)
                .map(|other| format!("拆分主题（{other}）"))
                .collect::<Vec<_>>()
                .join("、");
            cards.push(serde_json::json!({
                "relative_path": path,
                "title": title,
                "content": format!("---\ntags: [synthetic]\nsources: [inbox/拆分来源.md]\n---\n第 {position} 部分，合成正文。"),
                "sources": ["inbox/拆分来源.md"]
            }));
            lines.push(format!(
                "- [{title}](概念/{file}) | 标签：synthetic | 第 {} 部分 | 参见：{see}",
                position + 1
            ));
        }
        let payload = serde_json::json!({
            "cards": cards,
            "index_lines": lines,
            "archive_moves": []
        });
        let report = validate_payload(&payload, &index, &scan);
        assert!(report.ok, "{:?}", report.rejected_cards);
        assert_eq!(report.cards.len(), 3);
        assert!(report.index_lines.iter().all(|line| line.contains("参见：")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_missing_source() {
        let root = root("missing");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let report = validate_payload(&card_payload("- [专项检查频次](概念/专项检查频次.md) | x"), &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_empty_source_list() {
        let root = root("nosources");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["sources"] = serde_json::json!([]);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        assert!(report.rejected_cards[0].reason.contains("sources"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_missing_index_line() {
        let root = root("noindex");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let report = validate_payload(&card_payload("- [别的标题](概念/别的.md) | x"), &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_invalid_archive_move() {
        let root = root("movebad");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["archive_moves"] = serde_json::json!([{"from": "知识库/概念/x.md", "to": "知识库/原文/x.md"}]);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_moves.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_creates_card_updates_index_and_moves_source() {
        let root = root("apply");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "专项检查每季度一次");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let line = "- [专项检查频次](概念/专项检查频次.md) | 标签：监督, 专项检查 | 每季度一次";
        let mut payload = card_payload(line);
        payload["cards"][0]["sources"] = serde_json::json!(["inbox/合成制度.md"]);
        payload["archive_moves"] = serde_json::json!([{"from": "inbox/合成制度.md", "to": "知识库/原文/合成制度.md"}]);
        let report = apply_payload(&root, &payload, &scan);
        assert!(report.ok, "{:?} {:?}", report.errors, report.rejected_cards);
        assert!(root.join("知识库/概念/专项检查频次.md").is_file());
        assert!(!root.join("inbox/合成制度.md").exists());
        assert!(root.join("知识库/原文/合成制度.md").is_file());
        let card = fs::read_to_string(root.join("知识库/概念/专项检查频次.md")).unwrap();
        assert!(card.contains("sources: [知识库/原文/合成制度.md]"), "{card}");
        assert!(!card.contains("sources: [inbox/合成制度.md]"), "{card}");
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        assert!(index.contains("专项检查频次"));
        assert!(index.contains("概念/专项检查频次.md"));
        assert!(smoke_search(&root, "专项检查").unwrap() >= 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rfc3339_local_handles_offsets_epoch_and_leap_day() {
        assert_eq!(rfc3339_local(0, 0), "1970-01-01T00:00:00+00:00");
        assert_eq!(rfc3339_local(0, 480), "1970-01-01T08:00:00+08:00");
        assert_eq!(rfc3339_local(0, -330), "1969-12-31T18:30:00-05:30");
        assert_eq!(rfc3339_local(-1, 0), "1969-12-31T23:59:59+00:00");
        assert_eq!(rfc3339_local(1_709_164_800, 0), "2024-02-29T00:00:00+00:00");
        assert_eq!(rfc3339_local(1_709_164_800, 480), "2024-02-29T08:00:00+08:00");
    }

    #[test]
    fn admit_raw_creates_file_and_records_index() {
        let root = root("rawadmit");
        init_vault(&root).unwrap();
        let report = admit_raw_files(&root, &[raw("material.pdf", b"PDF-1")], 480);
        assert!(report.ok, "{:?} {:?}", report.errors, report.rejected);
        assert_eq!(report.admitted.len(), 1);
        let admitted = &report.admitted[0];
        assert_eq!(admitted.relative_path, "raw/material.pdf");
        assert_eq!(admitted.bytes, 5);
        assert_eq!(fs::read(root.join("raw/material.pdf")).unwrap(), b"PDF-1");
        assert!(report.index_updated);
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        assert!(index.contains("## 原始资料"), "{index}");
        assert!(index.contains("raw/material.pdf"), "{index}");
        assert!(index.contains(&admitted.sha256), "{index}");
        assert!(index.contains("5 字节"), "{index}");
        let line = index
            .lines()
            .find(|line| line.contains("raw/material.pdf"))
            .unwrap();
        assert!(line.contains("准入时间："), "{line}");
        assert!(line.contains("+08:00"), "{line}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_raw_writes_governance_admission_record() {
        let root = root("rawgovernance");
        init_vault(&root).unwrap();
        let report = admit_raw_files(&root, &[raw("material.pdf", b"PDF-1")], 480);
        assert!(report.ok, "{:?} {:?}", report.errors, report.rejected);
        let records = knowledge_governance::list_records(
            &root,
            knowledge_governance::KIND_RAW_ADMISSIONS,
        )
        .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action, "admit");
        assert_eq!(records[0].subject, "raw/material.pdf");
        assert_eq!(records[0].sha256, report.admitted[0].sha256[..12]);
        assert!(records[0].note.contains("原名：material.pdf"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn read_raw_file_reads_absolute_regular_file() {
        let root = root("rawpathread");
        let source = root.join("upload.pdf");
        fs::write(&source, b"PDF-2").unwrap();
        let input = read_raw_file(&source).unwrap();
        assert_eq!(input.name, "upload.pdf");
        assert_eq!(input.bytes, b"PDF-2");
        let missing = root.join("missing.pdf");
        assert!(read_raw_file(&missing).is_err());
        assert!(read_raw_file(Path::new("relative.pdf")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_rejects_duplicate_hash_and_leaves_index_unchanged() {
        let root = root("rawdup");
        init_vault(&root).unwrap();
        let first = admit_raw_files(&root, &[raw("first.md", b"same-content")], 480);
        assert!(first.ok, "{:?}", first.rejected);
        let before = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let second = admit_raw_files(&root, &[raw("second.md", b"same-content")], 480);
        assert!(second.admitted.is_empty());
        assert!(second.rejected[0].reason.contains("重复资料"));
        assert!(!second.index_updated);
        assert_eq!(fs::read_to_string(root.join(INDEX_REL)).unwrap(), before);
        assert!(!root.join("raw/second.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_renames_same_name_with_new_content_and_keeps_original() {
        let root = root("rawrename");
        init_vault(&root).unwrap();
        let first = admit_raw_files(&root, &[raw("report.bin", b"old")], 480);
        assert!(first.ok, "{:?}", first.rejected);
        let second = admit_raw_files(&root, &[raw("report.bin", b"new")], 480);
        assert!(second.ok, "{:?}", second.rejected);
        let admitted = &second.admitted[0];
        assert!(admitted.renamed);
        let hash = crate::sha256::sha256_hex(b"new");
        assert_eq!(
            admitted.relative_path,
            format!("raw/report-{}.bin", &hash[..12])
        );
        assert_eq!(fs::read(root.join("raw/report.bin")).unwrap(), b"old");
        assert_eq!(fs::read(root.join(&admitted.relative_path)).unwrap(), b"new");
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        assert!(index.contains(&admitted.relative_path), "{index}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_rejects_path_escape_hidden_and_empty_names() {
        let root = root("rawbadname");
        init_vault(&root).unwrap();
        let report = admit_raw_files(
            &root,
            &[
                raw("../escape.md", b"a"),
                raw("dir/file.md", b"b"),
                raw(".hidden.md", b"c"),
                raw("", b"d"),
            ],
            480,
        );
        assert!(report.admitted.is_empty());
        assert_eq!(report.rejected.len(), 4);
        assert!(!root.join("raw/../escape.md").exists());
        assert_eq!(
            fs::read_dir(root.join(RAW_DIR)).unwrap().count(),
            0
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_rejects_duplicate_hash_within_one_batch() {
        let root = root("rawbatchdup");
        init_vault(&root).unwrap();
        let report = admit_raw_files(
            &root,
            &[raw("a.md", b"batch-same"), raw("b.md", b"batch-same")],
            480,
        );
        assert_eq!(report.admitted.len(), 1);
        assert_eq!(report.rejected.len(), 1);
        assert_eq!(
            fs::read_to_string(root.join(INDEX_REL))
                .unwrap()
                .matches("- [")
                .count(),
            1
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn admit_rolls_back_raw_file_when_index_save_fails() {
        let root = root("rawrollback");
        init_vault(&root).unwrap();
        // 先把索引撑到超过单文件上限；追加准入行时 save_note 必然拒绝整次写入。
        let oversized = format!(
            "# 知识库索引\n\n## 原始资料\n{}\n",
            "x".repeat((crate::knowledge_vault::VAULT_MAX_NOTE_BYTES as usize) + 1)
        );
        fs::write(root.join(INDEX_REL), &oversized).unwrap();
        let report = admit_raw_files(&root, &[raw("rollback.bin", b"rollback")], 480);
        assert!(report.admitted.is_empty());
        assert!(!report.index_updated);
        assert!(!report.errors.is_empty());
        assert!(!root.join("raw/rollback.bin").exists());
        assert_eq!(fs::read_dir(root.join(RAW_DIR)).unwrap().count(), 0);
        assert!(fs::read_to_string(root.join(INDEX_REL)).unwrap().starts_with("# 知识库索引"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_keeps_valid_cards_when_one_is_rejected() {
        let root = root("partial");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        let mut bad = payload["cards"][0].clone();
        bad["relative_path"] = serde_json::json!("笔记/坏卡.md");
        bad["title"] = serde_json::json!("坏卡");
        payload["cards"].as_array_mut().unwrap().push(bad);
        let report = apply_payload(&root, &payload, &scan);
        assert_eq!(report.created.len(), 1);
        assert_eq!(report.rejected_cards.len(), 1);
        assert!(report.index_updated);
        let _ = fs::remove_dir_all(&root);
    }
}
