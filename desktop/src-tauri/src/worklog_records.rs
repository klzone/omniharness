//! 工作记录存储层（OD-WR-301）。
//!
//! 每条记录一个 Markdown 文件：<work_root>/工作记录/<record_id>.md。
//! frontmatter 固定字段：id / title / record_date / created_at / updated_at /
//! category / tags / source / ai_analysis_status / linked_task_ids / draft。
//! 原始素材（raw_input / raw_source）只在创建时写入，旧文件缺字段时为空；
//! 每次 update_record 追加 work/工作记录/<id>/revisions.jsonl 一行修订记录。
//!
//! 边界：
//!   * 路径只由宿主按校验过的 id 拼接，调用方不能传相对路径或目录名；
//!   * 新建走「临时文件 + fsync + 硬链接发布」，目标已存在即拒绝，绝不覆盖；
//!   * 改写走「临时文件 + fsync + rename」，失败清理临时文件；
//!   * 删除只移动到 .trash/工作记录/，不删除任何字节。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::task_store::KnowledgeLink;

/// 临时层根目录名（$HOME/OmniDesk/work）。仅用于文档与调用方约定。
pub const WORK_DIR: &str = "work";
/// 工作记录目录（work 根下）。
pub const RECORDS_DIR: &str = "工作记录";
/// 回收目录（work 根下）。删除只移动，不清理。
pub const TRASH_DIR: &str = ".trash";
pub const RECORD_SCHEMA: &str = "omnidesk.worklog-record.v1";
/// 修订历史文件（记录目录内，每条记录一个子目录）。
pub const REVISIONS_FILE: &str = "revisions.jsonl";

/// AI 解析状态（D-064 门禁）。unanalyzed 是新建记录的默认值。
pub const AI_UNANALYZED: &str = "unanalyzed";
pub const AI_ANALYZED: &str = "analyzed";
pub const AI_FAILED: &str = "failed";
pub const AI_STATUSES: &[&str] = &[AI_UNANALYZED, AI_ANALYZED, AI_FAILED];

const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_TAG_CHARS: usize = 32;
const MAX_TAGS: usize = 24;
const MAX_ID_CHARS: usize = 80;

/// 一条工作记录。字段顺序与 frontmatter 一致。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRecord {
    pub id: String,
    pub title: String,
    /// 本地日期 YYYY-MM-DD。
    pub record_date: String,
    /// UTC ISO8601，例如 2026-11-08T04:00:00Z。
    pub created_at: String,
    pub updated_at: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    /// 创建时的原始输入（OD-WR-501）；只在创建时写入，修订不改写。
    #[serde(default)]
    pub raw_input: Option<String>,
    /// 原始输入的来源描述，例如「首页对话 2026-11-11 09:30」。
    #[serde(default)]
    pub raw_source: Option<String>,
    /// 随手记的去向标签（OD-WR-310）；普通记录为 None。
    pub destination: Option<String>,
    pub ai_analysis_status: String,
    pub linked_task_ids: Vec<String>,
    pub draft: bool,
    /// 关联的知识条目（OD-KM-503）；旧文件缺该字段时为空。
    #[serde(default)]
    pub related_knowledge: Vec<KnowledgeLink>,
    pub body: String,
}

/// 新建记录的入参。id 省略时由宿主生成；title 省略时取正文首行。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordDraft {
    pub id: Option<String>,
    pub title: Option<String>,
    pub record_date: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    /// 创建时的原始输入与来源；后续修订没有改写入参。
    pub raw_input: Option<String>,
    pub raw_source: Option<String>,
    pub destination: Option<String>,
    pub draft: bool,
    pub body: String,
    pub linked_task_ids: Vec<String>,
}

/// 记录更新补丁。None = 不修改；空字符串 = 清空可选项。
/// 原始素材不在补丁里：raw_input / raw_source 创建后不可被修订覆盖。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordPatch {
    pub title: Option<String>,
    pub record_date: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub ai_analysis_status: Option<String>,
    pub linked_task_ids: Option<Vec<String>>,
    pub draft: Option<bool>,
    pub body: Option<String>,
}

/// revisions.jsonl 的一行（OD-WR-501）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionEntry {
    /// UTC ISO8601，与修订后的 updated_at 相同。
    pub at: String,
    /// 本次实际变化的字段名（snake_case）。
    pub changed_fields: Vec<String>,
    /// 修订前正文的 SHA-256（小写十六进制）。
    pub old_body_sha256: String,
    /// 人类可读的修订说明。
    pub note: String,
}

/// 界面读取的修订摘要：不暴露旧正文哈希等内部细节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSummary {
    /// UTC ISO8601，本次修订完成时间。
    pub at: String,
    /// 本次实际变化的字段名（snake_case）。
    pub changed_fields: Vec<String>,
    /// 人类可读的修订说明。
    pub note: String,
}

/// 列表过滤条件。日期为闭区间，比较 record_date 字符串（形状已校验）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordFilter {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub category: Option<String>,
    pub keyword: Option<String>,
    pub ai_analysis_status: Option<String>,
    pub draft: Option<bool>,
}

/// 列表结果。broken 是读不出或 frontmatter 不合法的文件数，如实计数。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordListing {
    pub items: Vec<WorkRecord>,
    pub broken: usize,
}

/// 删除回执：文件被移到回收目录后的位置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashMove {
    pub id: String,
    pub path: String,
}

/// 记录目录 <work_root>/工作记录。
pub fn records_dir(root: &Path) -> PathBuf {
    root.join(RECORDS_DIR)
}

/// 单条记录的修订目录 <work_root>/工作记录/<id>。
pub fn revision_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    assert_record_id(id)?;
    Ok(records_dir(root).join(id))
}

/// 修订历史文件 <work_root>/工作记录/<id>/revisions.jsonl。
pub fn revisions_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    Ok(revision_dir(root, id)?.join(REVISIONS_FILE))
}

/// 读取一条记录的修订历史。没有 revisions.jsonl 是旧记录的合法状态，返回空列表；
/// 文件存在但某一行损坏时整次读取失败，避免界面把缺失的历史误报为「没有修订」。
pub fn read_revision_summaries(root: &Path, id: &str) -> Result<Vec<RevisionSummary>, String> {
    assert_work_root(root)?;
    let path = revisions_path(root, id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取修订历史失败（{}）：{}", path.display(), e))?;
    let mut summaries = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: RevisionEntry = serde_json::from_str(line).map_err(|e| {
            format!(
                "修订历史第 {} 行无法解析（{}）：{}",
                index + 1,
                path.display(),
                e
            )
        })?;
        summaries.push(RevisionSummary {
            at: entry.at,
            changed_fields: entry.changed_fields,
            note: entry.note,
        });
    }
    Ok(summaries)
}

/// 回收目录 <work_root>/.trash/工作记录。
pub fn trash_records_dir(root: &Path) -> PathBuf {
    root.join(TRASH_DIR).join(RECORDS_DIR)
}

/// 记录文件路径。id 必须先通过形状校验。
pub fn record_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    assert_record_id(id)?;
    Ok(records_dir(root).join(format!("{id}.md")))
}

/// id 形状：非空、不超过 80 字符、只含 ASCII 字母数字与 - / _。
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_ID_CHARS
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn assert_record_id(id: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err(format!(
            "记录编号不合法（只允许字母、数字、- 与 _，最多 {MAX_ID_CHARS} 字符）：{id:?}"
        ));
    }
    Ok(())
}

/// 工作目录必须是绝对路径，避免相对路径把文件落到进程当前目录。
pub(crate) fn assert_work_root(root: &Path) -> Result<(), String> {
    if root.as_os_str().is_empty() {
        return Err("工作目录为空，无法读写工作记录。".to_string());
    }
    if !root.is_absolute() {
        return Err(format!("工作目录必须是绝对路径：{}", root.display()));
    }
    Ok(())
}

/// 日期形状 YYYY-MM-DD，并校验月/日范围（不做跨月日数校验）。
pub fn is_date_shape(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !date[0..4].bytes().all(|b| b.is_ascii_digit())
        || !date[5..7].bytes().all(|b| b.is_ascii_digit())
        || !date[8..10].bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let month: u32 = date[5..7].parse().unwrap_or(0);
    let day: u32 = date[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_path(dir: &Path, name: &str) -> PathBuf {
    let stamp = format!(
        "{}-{}",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    dir.join(format!(".{name}.{stamp}.tmp"))
}

/// 新建写入：临时文件 + fsync + 硬链接发布；目标已存在时硬链接原子失败，绝不覆盖。
pub(crate) fn write_new_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("目标没有父目录：{}", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "record".to_string());
    let temp = temp_path(dir, &name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("无法创建临时文件（{}）：{}", temp.display(), e))?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(format!("写入临时文件失败（{}）：{}", temp.display(), e));
    }
    drop(file);
    let result = match fs::hard_link(&temp, target) {
        Ok(()) => {
            let _ = fs::File::open(dir).and_then(|dir| dir.sync_all());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
            "目标已存在，新建不会覆盖：{}",
            target.display()
        )),
        Err(e) => Err(format!(
            "发布文件失败（{}）：{}；目标未被改动",
            target.display(),
            e
        )),
    };
    let _ = fs::remove_file(&temp);
    result
}

/// 改写：临时文件 + fsync + rename；失败清理临时文件，原文件不动。
pub(crate) fn write_replace_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("目标没有父目录：{}", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "record".to_string());
    let temp = temp_path(dir, &name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("无法创建临时文件（{}）：{}", temp.display(), e))?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(format!("写入临时文件失败（{}）：{}", temp.display(), e));
    }
    drop(file);
    match fs::rename(&temp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&temp);
            Err(format!(
                "替换文件失败（{}）：{}；原文件保持不变",
                target.display(),
                e
            ))
        }
    }
}

/// 追加前检查修订历史：只认普通文件，旧文件必须以换行结尾，半截行不被掩盖。
fn prepare_revision_log(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("修订记录路径没有父目录：{}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建修订记录目录失败（{}）：{e}", parent.display()))?;
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(format!(
                "无法确认修订记录文件（{}）：{e}",
                path.display()
            ))
        }
    };
    if meta.file_type().is_symlink() {
        return Err(format!("修订记录不能是符号链接：{}", path.display()));
    }
    if !meta.is_file() {
        return Err(format!("修订记录路径不是普通文件：{}", path.display()));
    }
    if meta.len() > 0 {
        let mut file = fs::File::open(path)
            .map_err(|e| format!("读取修订记录失败（{}）：{e}", path.display()))?;
        file.seek(SeekFrom::End(-1))
            .map_err(|e| format!("定位修订记录尾部失败（{}）：{e}", path.display()))?;
        let mut last = [0u8; 1];
        file.read_exact(&mut last)
            .map_err(|e| format!("读取修订记录尾部失败（{}）：{e}", path.display()))?;
        if last[0] != b'\n' {
            return Err(format!(
                "修订记录尾部不完整（{}），拒绝追加以免掩盖半截行。",
                path.display()
            ));
        }
    }
    Ok(())
}

/// 单次 write_all 追加整行，随后 flush + sync_all；不改写历史行。
fn append_revision_line(path: &Path, entry: &RevisionEntry) -> Result<(), String> {
    let line = serde_json::to_string(entry)
        .map_err(|e| format!("序列化修订记录失败：{e}"))?;
    if line.contains('\n') || line.contains('\r') {
        return Err("修订记录序列化后仍含换行，拒绝写入。".to_string());
    }
    let mut payload = Vec::with_capacity(line.len() + 1);
    payload.extend_from_slice(line.as_bytes());
    payload.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("打开修订记录失败（{}）：{e}", path.display()))?;
    file.write_all(&payload)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("追加修订记录失败（{}）：{e}", path.display()))
}

/// YAML 双引号标量：用 JSON 转义保证引号、换行、控制字符都能稳定往返。
pub(crate) fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

pub(crate) fn yaml_opt(value: &Option<String>) -> String {
    match value {
        Some(text) => yaml_scalar(text),
        None => "null".to_string(),
    }
}

/// 可选文本归一化：去空白；空白视作未设置。
pub(crate) fn normalize_opt(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// 生成带前缀的 id：<prefix>-<秒>-<进程号>-<序号>。
pub(crate) fn new_file_id(prefix: &str, now: f64) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}-{}-{}-{}",
        now.floor() as i64,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_CHARS {
            return Err(format!("标签过长（最多 {MAX_TAG_CHARS} 字）：{trimmed}"));
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    if out.len() > MAX_TAGS {
        return Err(format!("标签过多（最多 {MAX_TAGS} 个）"));
    }
    Ok(out)
}

pub(crate) fn normalize_link_ids(ids: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !is_safe_id(trimmed) {
            return Err(format!("关联任务编号不合法：{trimmed:?}"));
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

/// 正文首行作为标题；超长截断到 40 字并加省略号。
fn derive_title(body: &str) -> String {
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let cleaned = first.trim_start_matches('#').trim();
    let mut out = String::new();
    for (index, ch) in cleaned.chars().enumerate() {
        if index >= 40 {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn render_record(record: &WorkRecord) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(RECORD_SCHEMA)));
    fm.push_str(&format!("id: {}\n", yaml_scalar(&record.id)));
    fm.push_str(&format!("title: {}\n", yaml_scalar(&record.title)));
    fm.push_str(&format!("record_date: {}\n", yaml_scalar(&record.record_date)));
    fm.push_str(&format!("created_at: {}\n", yaml_scalar(&record.created_at)));
    fm.push_str(&format!("updated_at: {}\n", yaml_scalar(&record.updated_at)));
    fm.push_str(&format!("category: {}\n", yaml_opt(&record.category)));
    if record.tags.is_empty() {
        fm.push_str("tags: []\n");
    } else {
        fm.push_str("tags:\n");
        for tag in &record.tags {
            fm.push_str(&format!("  - {}\n", yaml_scalar(tag)));
        }
    }
    fm.push_str(&format!("source: {}\n", yaml_opt(&record.source)));
    if let Some(raw_input) = record.raw_input.as_deref() {
        fm.push_str(&format!("raw_input: {}\n", yaml_scalar(raw_input)));
    }
    if let Some(raw_source) = record.raw_source.as_deref() {
        fm.push_str(&format!("raw_source: {}\n", yaml_scalar(raw_source)));
    }
    fm.push_str(&format!("destination: {}\n", yaml_opt(&record.destination)));
    fm.push_str(&format!(
        "ai_analysis_status: {}\n",
        yaml_scalar(&record.ai_analysis_status)
    ));
    if record.linked_task_ids.is_empty() {
        fm.push_str("linked_task_ids: []\n");
    } else {
        fm.push_str("linked_task_ids:\n");
        for id in &record.linked_task_ids {
            fm.push_str(&format!("  - {}\n", yaml_scalar(id)));
        }
    }
    fm.push_str(&format!("draft: {}\n", record.draft));
    if !record.related_knowledge.is_empty() {
        fm.push_str("related_knowledge:\n");
        for link in &record.related_knowledge {
            fm.push_str(&format!("  - id: {}\n", yaml_scalar(&link.id)));
            fm.push_str(&format!("    reason: {}\n", yaml_scalar(&link.reason)));
            fm.push_str(&format!("    linked_at: {}\n", yaml_scalar(&link.linked_at)));
        }
    }
    let body = record
        .body
        .trim_end_matches(|c: char| c == '\n' || c == '\r');
    if body.is_empty() {
        format!("---\n{fm}---\n")
    } else {
        format!("---\n{fm}---\n\n{body}\n")
    }
}

/// 拆出 frontmatter 原文与正文。正文去掉一个分隔空行和尾部换行。
pub fn split_frontmatter(raw: &str) -> Result<(String, String), String> {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().ok_or_else(|| "文件为空".to_string())?;
    let first_text = first.trim_end_matches(|c: char| c == '\r' || c == '\n');
    if first_text != "---" {
        return Err("缺少 frontmatter 起始围栏".to_string());
    }
    let mut offset = first.len();
    let mut inner = String::new();
    for line in lines {
        let line_text = line.trim_end_matches(|c: char| c == '\r' || c == '\n');
        if line_text == "---" {
            let rest = &text[offset + line.len()..];
            let mut body = rest.to_string();
            if let Some(stripped) = body.strip_prefix('\n') {
                body = stripped.to_string();
            }
            let body = body
                .trim_end_matches(|c: char| c == '\n' || c == '\r')
                .to_string();
            return Ok((inner, body));
        }
        inner.push_str(line);
        offset += line.len();
    }
    Err("frontmatter 没有闭合围栏".to_string())
}

#[derive(Debug, Deserialize)]
struct RecordFront {
    #[serde(default)]
    schema: Option<String>,
    id: String,
    title: String,
    record_date: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    raw_input: Option<String>,
    #[serde(default)]
    raw_source: Option<String>,
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    ai_analysis_status: Option<String>,
    #[serde(default)]
    linked_task_ids: Vec<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    related_knowledge: Vec<KnowledgeLink>,
}

fn parse_record(raw: &str) -> Result<WorkRecord, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: RecordFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("记录 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != RECORD_SCHEMA {
            return Err(format!("记录 schema 不认识：{schema}"));
        }
    }
    assert_record_id(&front.id)?;
    if !is_date_shape(&front.record_date) {
        return Err(format!(
            "记录日期形状不正确（应为 YYYY-MM-DD）：{}",
            front.record_date
        ));
    }
    let status = front
        .ai_analysis_status
        .clone()
        .unwrap_or_else(|| AI_UNANALYZED.to_string());
    if !AI_STATUSES.contains(&status.as_str()) {
        return Err(format!("记录解析状态不认识：{status}"));
    }
    if front.title.trim().is_empty() {
        return Err("记录标题为空".to_string());
    }
    let tags = normalize_tags(&front.tags)?;
    let linked_task_ids = normalize_link_ids(&front.linked_task_ids)?;
    let related_knowledge = crate::task_store::normalize_knowledge_links(&front.related_knowledge)?;
    Ok(WorkRecord {
        id: front.id,
        title: front.title.trim().to_string(),
        record_date: front.record_date,
        created_at: front.created_at,
        updated_at: front.updated_at,
        category: normalize_opt(front.category),
        tags,
        source: normalize_opt(front.source),
        raw_input: normalize_opt(front.raw_input),
        raw_source: normalize_opt(front.raw_source),
        destination: normalize_opt(front.destination),
        ai_analysis_status: status,
        linked_task_ids,
        draft: front.draft,
        related_knowledge,
        body,
    })
}

/// 新建一条记录。标题与正文至少有一个非空；日期缺省取本地当天。
pub fn create_record(
    root: &Path,
    draft: &RecordDraft,
    now: f64,
    utc_offset_minutes: i32,
) -> Result<WorkRecord, String> {
    assert_work_root(root)?;
    let body = draft
        .body
        .trim_end_matches(|c: char| c == '\n' || c == '\r')
        .to_string();
    if body.len() > MAX_BODY_BYTES {
        return Err(format!(
            "记录正文 {} 字节，超过 {} 字节上限。",
            body.len(),
            MAX_BODY_BYTES
        ));
    }
    let title = match normalize_opt(draft.title.clone()) {
        Some(title) => title,
        None => derive_title(&body),
    };
    if title.is_empty() {
        return Err("记录标题和正文不能同时为空。".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("记录标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
    }
    let record_date = match normalize_opt(draft.record_date.clone()) {
        Some(date) => {
            if !is_date_shape(&date) {
                return Err(format!("记录日期形状不正确（应为 YYYY-MM-DD）：{date}"));
            }
            date
        }
        None => crate::worklog::date_label(now, utc_offset_minutes),
    };
    let id = match normalize_opt(draft.id.clone()) {
        Some(id) => {
            assert_record_id(&id)?;
            id
        }
        None => new_file_id("wr", now),
    };
    let created_at = crate::task_shell::utc_iso8601(now);
    let record = WorkRecord {
        id: id.clone(),
        title,
        record_date,
        created_at: created_at.clone(),
        updated_at: created_at,
        category: normalize_opt(draft.category.clone()),
        tags: normalize_tags(&draft.tags)?,
        source: normalize_opt(draft.source.clone()),
        raw_input: normalize_opt(draft.raw_input.clone()),
        raw_source: normalize_opt(draft.raw_source.clone()),
        destination: normalize_opt(draft.destination.clone()),
        ai_analysis_status: AI_UNANALYZED.to_string(),
        linked_task_ids: normalize_link_ids(&draft.linked_task_ids)?,
        draft: draft.draft,
        related_knowledge: Vec::new(),
        body,
    };
    fs::create_dir_all(records_dir(root))
        .map_err(|e| format!("创建工作记录目录失败：{e}"))?;
    write_new_atomically(&record_path(root, &id)?, render_record(&record).as_bytes())?;
    Ok(record)
}

/// 读取一条记录；文件缺失或 frontmatter 不合法都返回错误。
pub fn read_record(root: &Path, id: &str) -> Result<WorkRecord, String> {
    assert_work_root(root)?;
    let path = record_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取记录失败（{}）：{}", path.display(), e))?;
    parse_record(&raw)
}

fn matches_filter(filter: &RecordFilter, record: &WorkRecord) -> bool {
    if let Some(from) = filter.from_date.as_deref() {
        if record.record_date.as_str() < from {
            return false;
        }
    }
    if let Some(to) = filter.to_date.as_deref() {
        if record.record_date.as_str() > to {
            return false;
        }
    }
    if let Some(category) = filter.category.as_deref() {
        if record.category.as_deref() != Some(category) {
            return false;
        }
    }
    if let Some(status) = filter.ai_analysis_status.as_deref() {
        if record.ai_analysis_status != status {
            return false;
        }
    }
    if let Some(draft) = filter.draft {
        if record.draft != draft {
            return false;
        }
    }
    if let Some(keyword) = filter.keyword.as_deref() {
        let needle = keyword.trim().to_lowercase();
        if !needle.is_empty() {
            let haystack = format!(
                "{}\n{}\n{}\n{}",
                record.title,
                record.body,
                record.category.clone().unwrap_or_default(),
                record.tags.join(" ")
            )
            .to_lowercase();
            if !haystack.contains(&needle) {
                return false;
            }
        }
    }
    true
}

/// 列出记录：日期倒序，再按创建时间倒序。坏文件计数不中断列表。
pub fn list_records(root: &Path, filter: &RecordFilter) -> Result<RecordListing, String> {
    assert_work_root(root)?;
    if let Some(from) = filter.from_date.as_deref() {
        if !is_date_shape(from) {
            return Err(format!("起始日期形状不正确（应为 YYYY-MM-DD）：{from}"));
        }
    }
    if let Some(to) = filter.to_date.as_deref() {
        if !is_date_shape(to) {
            return Err(format!("结束日期形状不正确（应为 YYYY-MM-DD）：{to}"));
        }
    }
    if let (Some(from), Some(to)) = (filter.from_date.as_deref(), filter.to_date.as_deref()) {
        if from > to {
            return Err(format!("日期区间颠倒：{from} > {to}"));
        }
    }
    if let Some(status) = filter.ai_analysis_status.as_deref() {
        if !AI_STATUSES.contains(&status) {
            return Err(format!("记录解析状态不认识：{status}"));
        }
    }
    let dir = records_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if dir.exists() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取工作记录目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取工作记录目录项失败：{e}"))?;
            let file_type = entry
                .file_type()
                .map_err(|e| format!("读取目录项类型失败：{e}"))?;
            if !file_type.is_file() {
                continue;
            }
            let path = entry.path();
            let is_markdown = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_markdown {
                continue;
            }
            match fs::read_to_string(&path)
                .map_err(|e| format!("读取记录失败（{}）：{}", path.display(), e))
                .and_then(|raw| parse_record(&raw))
            {
                Ok(record) => {
                    if matches_filter(filter, &record) {
                        items.push(record);
                    }
                }
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        b.record_date
            .cmp(&a.record_date)
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(RecordListing { items, broken })
}

/// 更新记录。created_at 不变；updated_at 由调用方时间推进。
/// 成功写入后追加一条修订记录；原始素材只读，不参与补丁改写。
pub fn update_record(
    root: &Path,
    id: &str,
    patch: &RecordPatch,
    now: f64,
) -> Result<WorkRecord, String> {
    let mut record = read_record(root, id)?;
    let before = record.clone();
    if let Some(title) = patch.title.clone() {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err("记录标题不能为空。".to_string());
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!("记录标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
        }
        record.title = title;
    }
    if let Some(date) = patch.record_date.clone() {
        let date = date.trim().to_string();
        if !is_date_shape(&date) {
            return Err(format!("记录日期形状不正确（应为 YYYY-MM-DD）：{date}"));
        }
        record.record_date = date;
    }
    if let Some(category) = patch.category.clone() {
        record.category = normalize_opt(Some(category));
    }
    if let Some(tags) = patch.tags.clone() {
        record.tags = normalize_tags(&tags)?;
    }
    if let Some(source) = patch.source.clone() {
        record.source = normalize_opt(Some(source));
    }
    if let Some(destination) = patch.destination.clone() {
        record.destination = normalize_opt(Some(destination));
    }
    if let Some(status) = patch.ai_analysis_status.clone() {
        let status = status.trim().to_string();
        if !AI_STATUSES.contains(&status.as_str()) {
            return Err(format!("记录解析状态不认识：{status}"));
        }
        record.ai_analysis_status = status;
    }
    if let Some(ids) = patch.linked_task_ids.clone() {
        record.linked_task_ids = normalize_link_ids(&ids)?;
    }
    if let Some(draft) = patch.draft {
        record.draft = draft;
    }
    if let Some(body) = patch.body.clone() {
        let body = body
            .trim_end_matches(|c: char| c == '\n' || c == '\r')
            .to_string();
        if body.len() > MAX_BODY_BYTES {
            return Err(format!(
                "记录正文 {} 字节，超过 {} 字节上限。",
                body.len(),
                MAX_BODY_BYTES
            ));
        }
        if body.trim().is_empty() && record.title.trim().is_empty() {
            return Err("记录标题和正文不能同时为空。".to_string());
        }
        record.body = body;
    }
    let mut changed_fields: Vec<String> = Vec::new();
    if record.title != before.title {
        changed_fields.push("title".to_string());
    }
    if record.record_date != before.record_date {
        changed_fields.push("record_date".to_string());
    }
    if record.category != before.category {
        changed_fields.push("category".to_string());
    }
    if record.tags != before.tags {
        changed_fields.push("tags".to_string());
    }
    if record.source != before.source {
        changed_fields.push("source".to_string());
    }
    if record.destination != before.destination {
        changed_fields.push("destination".to_string());
    }
    if record.ai_analysis_status != before.ai_analysis_status {
        changed_fields.push("ai_analysis_status".to_string());
    }
    if record.linked_task_ids != before.linked_task_ids {
        changed_fields.push("linked_task_ids".to_string());
    }
    if record.draft != before.draft {
        changed_fields.push("draft".to_string());
    }
    if record.body != before.body {
        changed_fields.push("body".to_string());
    }
    record.updated_at = crate::task_shell::utc_iso8601(now);
    let revision_path = revisions_path(root, &record.id)?;
    prepare_revision_log(&revision_path)?;
    let path = record_path(root, &record.id)?;
    write_replace_atomically(&path, render_record(&record).as_bytes())?;
    let note = if changed_fields.is_empty() {
        "无字段变更".to_string()
    } else {
        format!("修改 {}", changed_fields.join(","))
    };
    let revision = RevisionEntry {
        at: record.updated_at.clone(),
        changed_fields,
        old_body_sha256: crate::sha256::sha256_hex(before.body.as_bytes()),
        note,
    };
    if let Err(message) = append_revision_line(&revision_path, &revision) {
        return Err(format!(
            "记录已更新，但修订记录没有写入：{message}。记录文件：{}",
            path.display()
        ));
    }
    Ok(record)
}

/// 删除记录：只移动到 .trash/工作记录/，不改写字节。
pub fn delete_record(root: &Path, id: &str, now: f64) -> Result<TrashMove, String> {
    assert_work_root(root)?;
    let source = record_path(root, id)?;
    if !source.is_file() {
        return Err(format!("记录不存在，未删除：{id}"));
    }
    let trash = trash_records_dir(root);
    fs::create_dir_all(&trash).map_err(|e| format!("创建回收目录失败：{e}"))?;
    let stamp = now.floor() as i64;
    let mut target = trash.join(format!("{id}.{stamp}.md"));
    let mut attempt = 0u32;
    while target.exists() {
        attempt += 1;
        target = trash.join(format!("{id}.{stamp}-{attempt}.md"));
    }
    fs::rename(&source, &target).map_err(|e| {
        format!(
            "移动到回收目录失败（{} → {}）：{}",
            source.display(),
            target.display(),
            e
        )
    })?;
    Ok(TrashMove {
        id: id.to_string(),
        path: target.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_780_000_000.0;
    const OFFSET: i32 = 480;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wr301-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn draft(title: &str, body: &str) -> RecordDraft {
        RecordDraft {
            title: Some(title.to_string()),
            body: body.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn create_writes_frontmatter_and_body() {
        let temp = TempRoot::new("create");
        let record = create_record(temp.root(), &draft("周会结论", "决定了三件事。"), NOW, OFFSET)
            .expect("创建失败");
        let raw = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("schema: \"omnidesk.worklog-record.v1\""));
        assert!(raw.contains("决定了三件事。"));
        assert_eq!(record.ai_analysis_status, AI_UNANALYZED);
        assert!(!record.draft);
    }

    #[test]
    fn create_uses_local_date_by_default() {
        let temp = TempRoot::new("date");
        let record = create_record(temp.root(), &draft("记录", "正文"), NOW, OFFSET).unwrap();
        assert_eq!(record.record_date, crate::worklog::date_label(NOW, OFFSET));
    }

    #[test]
    fn create_derives_title_from_first_body_line() {
        let temp = TempRoot::new("title");
        let record = create_record(
            temp.root(),
            &RecordDraft {
                body: "# 整理周报\n\n第二段".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        assert_eq!(record.title, "整理周报");
        assert_eq!(record.body, "# 整理周报\n\n第二段");
    }

    #[test]
    fn create_rejects_empty_title_and_body() {
        let temp = TempRoot::new("empty");
        let err = create_record(
            temp.root(),
            &RecordDraft {
                body: "   \n  ".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap_err();
        assert!(err.contains("不能同时为空"), "{err}");
    }

    #[test]
    fn create_rejects_duplicate_explicit_id() {
        let temp = TempRoot::new("dup");
        let first = create_record(
            temp.root(),
            &RecordDraft {
                id: Some("wr-dup-1".to_string()),
                title: Some("第一条".to_string()),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        assert_eq!(first.id, "wr-dup-1");
        let err = create_record(
            temp.root(),
            &RecordDraft {
                id: Some("wr-dup-1".to_string()),
                title: Some("第二条".to_string()),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap_err();
        assert!(err.contains("已存在"), "{err}");
    }

    #[test]
    fn create_rejects_path_like_id() {
        let temp = TempRoot::new("pathid");
        let err = create_record(
            temp.root(),
            &RecordDraft {
                id: Some("../escape".to_string()),
                title: Some("记录".to_string()),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap_err();
        assert!(err.contains("不合法"), "{err}");
    }

    #[test]
    fn read_round_trips_optional_fields_and_lists() {
        let temp = TempRoot::new("roundtrip");
        let created = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("复盘会议".to_string()),
                category: Some("项目".to_string()),
                tags: vec!["工作".to_string(), "复盘".to_string(), "工作".to_string()],
                source: Some("周会".to_string()),
                draft: true,
                body: "结论一\n结论二".to_string(),
                linked_task_ids: vec!["t-1".to_string()],
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let read = read_record(temp.root(), &created.id).unwrap();
        assert_eq!(read.category.as_deref(), Some("项目"));
        assert_eq!(read.tags, vec!["工作".to_string(), "复盘".to_string()]);
        assert_eq!(read.source.as_deref(), Some("周会"));
        assert!(read.draft);
        assert_eq!(read.linked_task_ids, vec!["t-1".to_string()]);
        assert_eq!(read.body, "结论一\n结论二");
    }

    #[test]
    fn list_counts_broken_files_and_keeps_valid_items() {
        let temp = TempRoot::new("broken");
        let record = create_record(temp.root(), &draft("好记录", "正文"), NOW, OFFSET).unwrap();
        fs::create_dir_all(records_dir(temp.root())).unwrap();
        fs::write(records_dir(temp.root()).join("bad.md"), "没有 frontmatter").unwrap();
        let listing = list_records(temp.root(), &RecordFilter::default()).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, record.id);
        assert_eq!(listing.broken, 1);
    }

    #[test]
    fn update_keeps_created_at_and_changes_fields() {
        let temp = TempRoot::new("update");
        let created = create_record(temp.root(), &draft("原标题", "原正文"), NOW, OFFSET).unwrap();
        let updated = update_record(
            temp.root(),
            &created.id,
            &RecordPatch {
                title: Some("新标题".to_string()),
                category: Some("项目".to_string()),
                ai_analysis_status: Some(AI_ANALYZED.to_string()),
                body: Some("新正文".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.created_at, created.created_at);
        assert_ne!(updated.updated_at, created.updated_at);
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.ai_analysis_status, AI_ANALYZED);
        assert_eq!(updated.body, "新正文");
        assert_eq!(updated.category.as_deref(), Some("项目"));
    }

    #[test]
    fn update_rejects_unknown_record() {
        let temp = TempRoot::new("update-missing");
        let err = update_record(temp.root(), "wr-missing", &RecordPatch::default(), NOW).unwrap_err();
        assert!(err.contains("读取记录失败"), "{err}");
    }

    #[test]
    fn update_rejects_bad_date_and_status() {
        let temp = TempRoot::new("update-bad");
        let created = create_record(temp.root(), &draft("记录", "正文"), NOW, OFFSET).unwrap();
        let err = update_record(
            temp.root(),
            &created.id,
            &RecordPatch {
                record_date: Some("2026-13-01".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("日期形状"), "{err}");
        let err = update_record(
            temp.root(),
            &created.id,
            &RecordPatch {
                ai_analysis_status: Some("bogus".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("解析状态不认识"), "{err}");
    }

    #[test]
    fn delete_moves_to_trash_preserving_bytes() {
        let temp = TempRoot::new("delete");
        let created = create_record(temp.root(), &draft("待删", "正文内容"), NOW, OFFSET).unwrap();
        let path = record_path(temp.root(), &created.id).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let moved = delete_record(temp.root(), &created.id, NOW).unwrap();
        assert!(!path.exists());
        let after = fs::read_to_string(&moved.path).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn delete_twice_reports_missing_record() {
        let temp = TempRoot::new("delete-twice");
        let created = create_record(temp.root(), &draft("待删", "正文"), NOW, OFFSET).unwrap();
        delete_record(temp.root(), &created.id, NOW).unwrap();
        let err = delete_record(temp.root(), &created.id, NOW).unwrap_err();
        assert!(err.contains("不存在"), "{err}");
    }

    #[test]
    fn list_filters_by_date_category_and_keyword() {
        let temp = TempRoot::new("filter");
        let first = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("周一复盘".to_string()),
                record_date: Some("2026-11-02".to_string()),
                category: Some("项目".to_string()),
                body: "讨论了验收口径".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let second = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("周二记录".to_string()),
                record_date: Some("2026-11-03".to_string()),
                category: Some("日常".to_string()),
                body: "整理了资料".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let listing = list_records(
            temp.root(),
            &RecordFilter {
                from_date: Some("2026-11-03".to_string()),
                to_date: Some("2026-11-03".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, second.id);
        let listing = list_records(
            temp.root(),
            &RecordFilter {
                category: Some("项目".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, first.id);
        let listing = list_records(
            temp.root(),
            &RecordFilter {
                keyword: Some("验收".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, first.id);
    }

    #[test]
    fn list_rejects_inverted_date_range() {
        let temp = TempRoot::new("range");
        let err = list_records(
            temp.root(),
            &RecordFilter {
                from_date: Some("2026-11-05".to_string()),
                to_date: Some("2026-11-01".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("颠倒"), "{err}");
    }

    #[test]
    fn list_filters_draft_and_analysis_status() {
        let temp = TempRoot::new("status");
        let normal = create_record(temp.root(), &draft("正式", "正文"), NOW, OFFSET).unwrap();
        let draft_record = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("草稿".to_string()),
                draft: true,
                body: "正文".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        update_record(
            temp.root(),
            &normal.id,
            &RecordPatch {
                ai_analysis_status: Some(AI_ANALYZED.to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let listing = list_records(
            temp.root(),
            &RecordFilter {
                draft: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, draft_record.id);
        let listing = list_records(
            temp.root(),
            &RecordFilter {
                ai_analysis_status: Some(AI_ANALYZED.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, normal.id);
    }

    #[test]
    fn list_sorts_by_record_date_desc() {
        let temp = TempRoot::new("sort");
        let older = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("旧".to_string()),
                record_date: Some("2026-11-01".to_string()),
                body: "正文".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let newer = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("新".to_string()),
                record_date: Some("2026-11-07".to_string()),
                body: "正文".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let listing = list_records(temp.root(), &RecordFilter::default()).unwrap();
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.items[0].id, newer.id);
        assert_eq!(listing.items[1].id, older.id);
    }

    #[test]
    fn quoting_round_trips_special_characters() {
        let temp = TempRoot::new("quote");
        let created = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("他说\"好\"：标题".to_string()),
                body: "- 列表\n包含 : 冒号\n# 井号\n结尾\\反斜杠".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let read = read_record(temp.root(), &created.id).unwrap();
        assert_eq!(read.title, created.title);
        assert_eq!(read.body, created.body);
    }

    #[test]
    fn create_leaves_no_temp_files() {
        let temp = TempRoot::new("temp");
        create_record(temp.root(), &draft("记录", "正文"), NOW, OFFSET).unwrap();
        let entries = fs::read_dir(records_dir(temp.root())).unwrap();
        let names: Vec<String> = entries
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].ends_with(".md"));
    }

    fn seed_record_without_links(temp: &TempRoot, id: &str) {
        fs::create_dir_all(records_dir(temp.root())).unwrap();
        let raw = format!(
            "---\nschema: \"omnidesk.worklog-record.v1\"\nid: \"{id}\"\ntitle: \"旧记录\"\nrecord_date: \"2026-02-01\"\ncreated_at: \"2026-02-01T09:00:00Z\"\nupdated_at: \"2026-02-01T10:00:00Z\"\ncategory: \"写作\"\ntags:\n  - \"虚构\"\nsource: \"手动\"\ndestination: null\nai_analysis_status: \"analyzed\"\nlinked_task_ids:\n  - \"t-503-1\"\ndraft: false\n---\n\n今天整理虚构周报。\n"
        );
        fs::write(record_path(temp.root(), id).unwrap(), raw).unwrap();
    }

    fn seed_record_with_links(temp: &TempRoot, id: &str) {
        seed_record_without_links(temp, id);
        let path = record_path(temp.root(), id).unwrap();
        let raw = fs::read_to_string(&path).unwrap().replace(
            "draft: false\n---\n",
            "draft: false\nrelated_knowledge:\n  - id: \"notes/synthetic-knowledge.md\"\n    reason: \"做法被该条目引用\"\n    linked_at: \"2026-02-02T08:15:00Z\"\n---\n",
        );
        fs::write(&path, raw).unwrap();
    }

    #[test]
    fn legacy_record_without_related_knowledge_reads_default() {
        let temp = TempRoot::new("links-legacy-default");
        seed_record_without_links(&temp, "wr-503-legacy");
        let record = read_record(temp.root(), "wr-503-legacy").unwrap();
        assert!(record.related_knowledge.is_empty());
    }

    #[test]
    fn legacy_record_load_save_is_byte_identical() {
        let temp = TempRoot::new("links-legacy-bytes");
        seed_record_without_links(&temp, "wr-503-legacy-bytes");
        let path = record_path(temp.root(), "wr-503-legacy-bytes").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let record = read_record(temp.root(), "wr-503-legacy-bytes").unwrap();
        write_replace_atomically(&path, render_record(&record).as_bytes()).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "旧记录文件 load→save 应逐字节一致");
    }

    #[test]
    fn record_with_related_knowledge_round_trips() {
        let temp = TempRoot::new("links-round-trip");
        seed_record_with_links(&temp, "wr-503-links");
        let path = record_path(temp.root(), "wr-503-links").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let record = read_record(temp.root(), "wr-503-links").unwrap();
        assert_eq!(record.related_knowledge.len(), 1);
        assert_eq!(record.related_knowledge[0].id, "notes/synthetic-knowledge.md");
        assert_eq!(record.related_knowledge[0].reason, "做法被该条目引用");
        assert_eq!(record.related_knowledge[0].linked_at, "2026-02-02T08:15:00Z");
        write_replace_atomically(&path, render_record(&record).as_bytes()).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "带关联字段的记录应稳定往返");
    }

    #[test]
    fn record_parse_rejects_empty_knowledge_reason() {
        let temp = TempRoot::new("links-empty-reason");
        seed_record_with_links(&temp, "wr-503-empty-reason");
        let path = record_path(temp.root(), "wr-503-empty-reason").unwrap();
        let raw = fs::read_to_string(&path)
            .unwrap()
            .replace("做法被该条目引用", "");
        fs::write(&path, raw).unwrap();
        let err = read_record(temp.root(), "wr-503-empty-reason").unwrap_err();
        assert!(err.contains("必须填写理由"), "{err}");
    }

    #[test]
    fn new_record_omits_related_knowledge_until_set() {
        let temp = TempRoot::new("links-new-record");
        let record = create_record(temp.root(), &draft("虚构记录", "虚构正文"), NOW, OFFSET).unwrap();
        let raw = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(!raw.contains("related_knowledge"), "{raw}");
        assert!(record.related_knowledge.is_empty());
    }

    fn read_revisions(root: &Path, id: &str) -> Vec<RevisionEntry> {
        let path = revisions_path(root, id).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<RevisionEntry>(line).unwrap())
            .collect()
    }

    #[test]
    fn legacy_record_without_raw_fields_load_save_is_byte_identical() {
        let temp = TempRoot::new("raw-legacy-bytes");
        seed_record_without_links(&temp, "wr-501-legacy");
        let path = record_path(temp.root(), "wr-501-legacy").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let record = read_record(temp.root(), "wr-501-legacy").unwrap();
        assert_eq!(record.raw_input, None);
        assert_eq!(record.raw_source, None);
        write_replace_atomically(&path, render_record(&record).as_bytes()).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "旧记录缺 raw 字段时 load→save 应逐字节一致");
    }

    #[test]
    fn new_record_omits_raw_fields_until_set() {
        let temp = TempRoot::new("raw-new-record");
        let record = create_record(temp.root(), &draft("虚构记录", "虚构正文"), NOW, OFFSET).unwrap();
        let raw = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(!raw.contains("raw_input"), "{raw}");
        assert!(!raw.contains("raw_source"), "{raw}");
        assert_eq!(record.raw_input, None);
        assert_eq!(record.raw_source, None);
    }

    #[test]
    fn create_writes_raw_input_and_source_to_frontmatter() {
        let temp = TempRoot::new("raw-create");
        let record = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("整理后的记录".to_string()),
                raw_input: Some("原始需求：整理今天的会议结论".to_string()),
                raw_source: Some("首页对话 2026-11-11 09:30".to_string()),
                body: "会议结论已整理。".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let raw = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(raw.contains("raw_input: \"原始需求：整理今天的会议结论\""), "{raw}");
        assert!(raw.contains("raw_source: \"首页对话 2026-11-11 09:30\""), "{raw}");
        let read = read_record(temp.root(), &record.id).unwrap();
        assert_eq!(read.raw_input.as_deref(), Some("原始需求：整理今天的会议结论"));
        assert_eq!(read.raw_source.as_deref(), Some("首页对话 2026-11-11 09:30"));
    }

    #[test]
    fn blank_raw_input_is_normalized_to_none_and_omitted() {
        let temp = TempRoot::new("raw-blank");
        let record = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("记录".to_string()),
                raw_input: Some("   \n  ".to_string()),
                raw_source: Some(" ".to_string()),
                body: "正文".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let raw = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(!raw.contains("raw_input"), "{raw}");
        assert!(!raw.contains("raw_source"), "{raw}");
        assert_eq!(record.raw_input, None);
        assert_eq!(record.raw_source, None);
    }

    #[test]
    fn patch_has_no_raw_input_entry_and_update_preserves_raw() {
        let temp = TempRoot::new("raw-preserved");
        let record = create_record(
            temp.root(),
            &RecordDraft {
                title: Some("初始记录".to_string()),
                raw_input: Some("原始素材不可被修订覆盖".to_string()),
                raw_source: Some("首页对话 2026-11-11 09:30".to_string()),
                body: "初始正文".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        // 字段清单写全且不用 ..Default：若将来给 RecordPatch 增加 raw 入口，本测试无法编译。
        let patch = RecordPatch {
            title: None,
            record_date: None,
            category: None,
            tags: None,
            source: None,
            destination: None,
            ai_analysis_status: None,
            linked_task_ids: None,
            draft: None,
            body: Some("修订后的正文".to_string()),
        };
        let updated = update_record(temp.root(), &record.id, &patch, NOW + 60.0).unwrap();
        assert_eq!(updated.raw_input.as_deref(), Some("原始素材不可被修订覆盖"));
        assert_eq!(updated.raw_source.as_deref(), Some("首页对话 2026-11-11 09:30"));
        let read = read_record(temp.root(), &record.id).unwrap();
        assert_eq!(read.raw_input.as_deref(), Some("原始素材不可被修订覆盖"));
        assert_eq!(read.raw_source.as_deref(), Some("首页对话 2026-11-11 09:30"));
        assert_eq!(read.body, "修订后的正文");
    }

    #[test]
    fn update_appends_revision_with_changed_fields_and_old_body_hash() {
        let temp = TempRoot::new("revision-line");
        let record = create_record(temp.root(), &draft("原标题", "原正文"), NOW, OFFSET).unwrap();
        update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                title: Some("新标题".to_string()),
                body: Some("新正文".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        let revisions = read_revisions(temp.root(), &record.id);
        assert_eq!(revisions.len(), 1, "每次成功修订只追加一行");
        assert_eq!(revisions[0].at, crate::task_shell::utc_iso8601(NOW + 60.0));
        assert_eq!(revisions[0].changed_fields, vec!["title".to_string(), "body".to_string()]);
        assert_eq!(
            revisions[0].old_body_sha256,
            crate::sha256::sha256_hex("原正文".as_bytes())
        );
        assert!(revisions[0].note.contains("title"), "{}", revisions[0].note);
        let raw = fs::read_to_string(revisions_path(temp.root(), &record.id).unwrap()).unwrap();
        assert!(raw.ends_with('\n'), "JSONL 每行都应以换行结尾");
    }

    #[test]
    fn successive_updates_hash_the_previous_body_each_time() {
        let temp = TempRoot::new("revision-chain");
        let record = create_record(temp.root(), &draft("记录", "第一版正文"), NOW, OFFSET).unwrap();
        update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                body: Some("第二版正文".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                body: Some("第三版正文".to_string()),
                ..Default::default()
            },
            NOW + 120.0,
        )
        .unwrap();
        let revisions = read_revisions(temp.root(), &record.id);
        assert_eq!(revisions.len(), 2);
        assert_eq!(
            revisions[0].old_body_sha256,
            crate::sha256::sha256_hex("第一版正文".as_bytes())
        );
        assert_eq!(
            revisions[1].old_body_sha256,
            crate::sha256::sha256_hex("第二版正文".as_bytes())
        );
        assert_eq!(revisions[0].changed_fields, vec!["body".to_string()]);
        assert_eq!(revisions[1].changed_fields, vec!["body".to_string()]);
    }

    #[test]
    fn empty_patch_still_records_an_update() {
        let temp = TempRoot::new("revision-empty");
        let record = create_record(temp.root(), &draft("记录", "正文"), NOW, OFFSET).unwrap();
        update_record(temp.root(), &record.id, &RecordPatch::default(), NOW + 60.0).unwrap();
        let revisions = read_revisions(temp.root(), &record.id);
        assert_eq!(revisions.len(), 1);
        assert!(revisions[0].changed_fields.is_empty());
        assert_eq!(revisions[0].note, "无字段变更");
    }

    #[test]
    fn truncated_revision_tail_blocks_update_without_touching_record() {
        let temp = TempRoot::new("revision-truncated");
        let record = create_record(temp.root(), &draft("记录", "原正文"), NOW, OFFSET).unwrap();
        let rev_path = revisions_path(temp.root(), &record.id).unwrap();
        fs::create_dir_all(rev_path.parent().unwrap()).unwrap();
        fs::write(&rev_path, "{\"at\":\"2026-11-11T00:00:00Z\"").unwrap();
        let record_path_before = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        let err = update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                body: Some("不应写入的正文".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("尾部不完整"), "{err}");
        let record_path_after = fs::read_to_string(record_path(temp.root(), &record.id).unwrap()).unwrap();
        assert_eq!(record_path_before, record_path_after, "修订日志不完整时记录文件不动");
        assert_eq!(fs::read_to_string(&rev_path).unwrap(), "{\"at\":\"2026-11-11T00:00:00Z\"");
    }

    #[test]
    fn missing_revision_history_reads_as_empty_for_legacy_record() {
        let temp = TempRoot::new("revision-summary-missing");
        let record = create_record(temp.root(), &draft("旧记录", "旧正文"), NOW, OFFSET).unwrap();
        assert!(!revisions_path(temp.root(), &record.id).unwrap().exists());
        let summaries = read_revision_summaries(temp.root(), &record.id).unwrap();
        assert!(summaries.is_empty(), "没有修订文件不等于读取失败");
    }

    #[test]
    fn revision_summaries_keep_order_and_omit_body_hash() {
        let temp = TempRoot::new("revision-summary-order");
        let record = create_record(temp.root(), &draft("记录", "第一版正文"), NOW, OFFSET).unwrap();
        update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                body: Some("第二版正文".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        update_record(
            temp.root(),
            &record.id,
            &RecordPatch {
                title: Some("新标题".to_string()),
                tags: Some(vec!["复盘".to_string()]),
                ..Default::default()
            },
            NOW + 120.0,
        )
        .unwrap();

        let summaries = read_revision_summaries(temp.root(), &record.id).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].at, crate::task_shell::utc_iso8601(NOW + 60.0));
        assert_eq!(summaries[0].changed_fields, vec!["body".to_string()]);
        assert_eq!(summaries[1].changed_fields, vec!["title".to_string(), "tags".to_string()]);
        let serialized = serde_json::to_string(&summaries).unwrap();
        assert!(!serialized.contains("old_body_sha256"), "{serialized}");
        assert!(!serialized.contains("sha"), "{serialized}");
    }

    #[test]
    fn corrupt_revision_line_fails_instead_of_hiding_history() {
        let temp = TempRoot::new("revision-summary-corrupt");
        let record = create_record(temp.root(), &draft("记录", "正文"), NOW, OFFSET).unwrap();
        let path = revisions_path(temp.root(), &record.id).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not-json}\n").unwrap();
        let err = read_revision_summaries(temp.root(), &record.id).unwrap_err();
        assert!(err.contains("第 1 行无法解析"), "{err}");
    }
}
