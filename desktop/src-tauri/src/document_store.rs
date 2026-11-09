//! 文稿与版本存储（OD-WT-602）。
//!
//! 每个写作任务的文稿放在 <work_root>/写作/<writing_id>/：
//! outline.md（大纲）、正文.md（正文）、versions/v<n>_<label>.md（历史快照）
//! 与 manifest.json（当前版本、版本条目、导出回执）。目录约定与 601 的
//! `writing_store::WRITING_DIR` 对齐；JSON manifest 的读写照
//! availability.v1.json 的先读后校验、临时文件原子替换模式。
//!
//! 正文更新只经 `save_with_snapshot`：同一函数先写不可覆盖的快照，再替换
//! 当前正文，最后更新 manifest。模块内不提供裸写正文入口，也不删除任何
//! 历史快照。恢复由 `restore_version` 把旧快照内容落为新当前版并新增快照。
//!
//! 段落差异 `diff_paragraphs` 是纯函数：按空行分段、按序号逐段比较，
//! 返回变更段序号与前后文本，不引外部 diff 库。
//!
//! 安全边界：写作编号先过 `is_safe_id`（只允许 ASCII 字母数字与 -/_），
//! 所有派生路径都断言在 root/写作/ 之下；拒绝符号链接目录与文件，
//! 避免通过链接逃逸 work/写作/。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::sha256::sha256_hex;
use crate::task_shell::utc_iso8601;
use crate::worklog_records::{
    assert_work_root, is_safe_id, split_frontmatter, write_new_atomically,
    write_replace_atomically, yaml_scalar,
};
use crate::writing_store::WRITING_DIR;

pub const DOCUMENT_SCHEMA: &str = "omnidesk.document.v1";
pub const MANIFEST_SCHEMA: &str = "omnidesk.document_manifest.v1";
pub const OUTLINE_FILE: &str = "outline.md";
pub const BODY_FILE: &str = "正文.md";
pub const VERSIONS_DIR: &str = "versions";
pub const MANIFEST_FILE: &str = "manifest.json";

const MAX_ID_CHARS: usize = 80;
const MAX_LABEL_CHARS: usize = 80;
const MAX_NOTE_CHARS: usize = 500;
const MAX_FORMAT_CHARS: usize = 32;
const MAX_EXPORT_PATH_CHARS: usize = 2048;
const MAX_CONTENT_BYTES: usize = 8 * 1024 * 1024;

/// 文稿种类：大纲与正文。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentKind {
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "body")]
    Body,
}

impl DocumentKind {
    pub fn file_name(self) -> &'static str {
        match self {
            DocumentKind::Outline => OUTLINE_FILE,
            DocumentKind::Body => BODY_FILE,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            DocumentKind::Outline => "outline",
            DocumentKind::Body => "body",
        }
    }
}

/// 版本来源只有用户与 AI 两类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionSource {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "ai")]
    Ai,
}

impl VersionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionSource::User => "user",
            VersionSource::Ai => "ai",
        }
    }
}

/// 快照范围：大纲、单章/片段、全文。Section 与 Full 都落在正文.md。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionScope {
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "section")]
    Section,
    #[serde(rename = "full")]
    Full,
}

impl VersionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionScope::Outline => "outline",
            VersionScope::Section => "section",
            VersionScope::Full => "full",
        }
    }

    pub fn target_kind(self) -> DocumentKind {
        match self {
            VersionScope::Outline => DocumentKind::Outline,
            VersionScope::Section | VersionScope::Full => DocumentKind::Body,
        }
    }
}

/// 一次快照的元数据。label 只用于快照文件名，note 是给人看的说明。
#[derive(Debug, Clone, Copy)]
pub struct SnapshotSpec<'a> {
    pub scope: VersionScope,
    pub source: VersionSource,
    pub label: &'a str,
    pub note: &'a str,
}

/// manifest.json 的一条版本记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionEntry {
    pub version: u64,
    pub at: String,
    pub source: VersionSource,
    pub scope: VersionScope,
    pub note: String,
    /// 快照正文的 SHA-256；读取时校验，防止快照被无意篡改。
    #[serde(default)]
    pub content_sha256: String,
}

/// 导出回执：记录曾从哪一版导出何种格式到哪里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportReceipt {
    pub format: String,
    pub at: String,
    pub path: String,
}

/// manifest.json 的完整结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentManifest {
    #[serde(default = "manifest_schema_default")]
    pub schema: String,
    #[serde(default)]
    pub writing_id: String,
    pub current_version: u64,
    #[serde(default)]
    pub versions: Vec<VersionEntry>,
    #[serde(default)]
    pub export_receipts: Vec<ExportReceipt>,
    #[serde(default)]
    pub updated_at: String,
}

fn manifest_schema_default() -> String {
    MANIFEST_SCHEMA.to_string()
}

/// 读取历史快照的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionContent {
    pub entry: VersionEntry,
    pub content: String,
}

/// 一个段落的变化类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParagraphChangeKind {
    #[serde(rename = "新增")]
    Added,
    #[serde(rename = "删除")]
    Removed,
    #[serde(rename = "修改")]
    Modified,
}

impl ParagraphChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ParagraphChangeKind::Added => "新增",
            ParagraphChangeKind::Removed => "删除",
            ParagraphChangeKind::Modified => "修改",
        }
    }
}

/// 单个变更段：序号从新文本（或较短一侧的末段）的 1 起。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphChange {
    pub paragraph_no: u32,
    pub kind: ParagraphChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// 两版文本的段落级差异。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphDiff {
    pub changed_paragraph_nos: Vec<u32>,
    pub changes: Vec<ParagraphChange>,
}

/// 写作任务文稿目录 <work_root>/写作/<writing_id>/。
pub fn document_dir(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    assert_work_root(root)?;
    assert_writing_id(writing_id)?;
    let writing_root = root.join(WRITING_DIR);
    assert_not_symlink(&writing_root)?;
    let dir = writing_root.join(writing_id);
    assert_under(&writing_root, &dir)?;
    Ok(dir)
}

pub fn outline_path(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    Ok(document_dir(root, writing_id)?.join(OUTLINE_FILE))
}

pub fn body_path(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    Ok(document_dir(root, writing_id)?.join(BODY_FILE))
}

pub fn versions_dir(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    let dir = document_dir(root, writing_id)?;
    let versions = dir.join(VERSIONS_DIR);
    assert_under(&dir, &versions)?;
    Ok(versions)
}

pub fn manifest_path(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    Ok(document_dir(root, writing_id)?.join(MANIFEST_FILE))
}

fn document_path(
    root: &Path,
    writing_id: &str,
    kind: DocumentKind,
) -> Result<PathBuf, String> {
    Ok(document_dir(root, writing_id)?.join(kind.file_name()))
}

fn version_path(
    root: &Path,
    writing_id: &str,
    version: u64,
    label: &str,
) -> Result<PathBuf, String> {
    let dir = versions_dir(root, writing_id)?;
    let path = dir.join(format!("v{version}_{label}.md"));
    assert_under(&dir, &path)?;
    Ok(path)
}

fn assert_writing_id(writing_id: &str) -> Result<(), String> {
    if !is_safe_id(writing_id) || writing_id.chars().count() > MAX_ID_CHARS {
        return Err(format!("写作任务编号不合法：{writing_id:?}"));
    }
    Ok(())
}

fn assert_under(base: &Path, target: &Path) -> Result<(), String> {
    if !target.starts_with(base) {
        return Err(format!(
            "文稿路径逃逸出写作目录（{} 不在 {} 之下）。",
            target.display(),
            base.display()
        ));
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "拒绝使用符号链接目录（{}），以免文稿路径逃逸。",
                    path.display()
                ));
            }
            if !meta.is_dir() {
                return Err(format!("文稿目录不是目录：{}", path.display()));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)
                .map_err(|e| format!("创建文稿目录失败（{}）：{}", path.display(), e))?;
            let meta = fs::symlink_metadata(path)
                .map_err(|e| format!("检查文稿目录失败（{}）：{}", path.display(), e))?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(format!("文稿目录不是普通目录：{}", path.display()));
            }
            Ok(())
        }
        Err(error) => Err(format!("检查文稿目录失败（{}）：{}", path.display(), error)),
    }
}

fn assert_not_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "拒绝读取符号链接（{}），以免文稿路径逃逸。",
            path.display()
        )),
        _ => Ok(()),
    }
}

fn read_regular_text(path: &Path, field: &str) -> Result<String, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("读取{field}失败（{}）：{}", path.display(), e))?;
    if !meta.file_type().is_file() {
        return Err(format!("{field}不是普通文件：{}", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("读取{field}失败（{}）：{}", path.display(), e))
}

fn canonical_content(content: &str) -> String {
    content
        .trim_end_matches(|c: char| c == '\n' || c == '\r')
        .to_string()
}

fn validate_content(content: &str) -> Result<(), String> {
    if content.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "文稿内容过长（最多 {} 字节）。",
            MAX_CONTENT_BYTES
        ));
    }
    if content.chars().any(|c| {
        c.is_control() && c != '\n' && c != '\r' && c != '\t'
    }) {
        return Err("文稿内容含不允许的控制字符。".to_string());
    }
    Ok(())
}

fn validate_note(note: &str) -> Result<String, String> {
    let trimmed = note.trim();
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("版本说明过长（最多 {MAX_NOTE_CHARS} 字）。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("版本说明含控制字符。".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_label(label: &str) -> Result<String, String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Ok("snapshot".to_string());
    }
    if !is_safe_id(trimmed) || trimmed.chars().count() > MAX_LABEL_CHARS {
        return Err(format!(
            "快照标签不合法（只允许字母、数字、- 与 _，最多 {MAX_LABEL_CHARS} 字符）：{label:?}"
        ));
    }
    Ok(trimmed.to_string())
}

fn digits(value: &str, start: usize, end: usize) -> bool {
    value
        .get(start..end)
        .map(|part| part.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or(false)
}

fn is_timestamp_shape(value: &str) -> bool {
    if value.len() != 20 {
        return false;
    }
    let bytes = value.as_bytes();
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && digits(value, 0, 4)
        && digits(value, 5, 7)
        && digits(value, 8, 10)
        && digits(value, 11, 13)
        && digits(value, 14, 16)
        && digits(value, 17, 19)
        && crate::worklog_records::is_date_shape(&value[0..10])
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn validate_timestamp(value: &str, field: &str) -> Result<(), String> {
    if !is_timestamp_shape(value) {
        return Err(format!("{field}时间形状不正确（应类似 2026-11-09T12:00:00Z）：{value}"));
    }
    Ok(())
}

fn render_document(
    kind: DocumentKind,
    writing_id: &str,
    version: u64,
    at: &str,
    content: &str,
    content_sha256: &str,
) -> String {
    let mut front = String::new();
    front.push_str(&format!("schema: {}\n", yaml_scalar(DOCUMENT_SCHEMA)));
    front.push_str(&format!(
        "writing_id: {}\n",
        yaml_scalar(writing_id)
    ));
    front.push_str(&format!("kind: {}\n", yaml_scalar(kind.as_str())));
    front.push_str(&format!("version: {version}\n"));
    front.push_str(&format!("updated_at: {}\n", yaml_scalar(at)));
    front.push_str(&format!(
        "content_sha256: {}\n",
        yaml_scalar(content_sha256)
    ));
    if content.is_empty() {
        format!("---\n{front}---\n")
    } else {
        format!("---\n{front}---\n\n{content}\n")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentFront {
    #[serde(default)]
    schema: Option<String>,
    writing_id: String,
    kind: DocumentKind,
    version: u64,
    updated_at: String,
    #[serde(default)]
    content_sha256: Option<String>,
}

fn parse_document(
    raw: &str,
    writing_id: &str,
    expected: DocumentKind,
) -> Result<String, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: DocumentFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("文稿 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != DOCUMENT_SCHEMA {
            return Err(format!("文稿 schema 不认识：{schema}"));
        }
    }
    if front.writing_id != writing_id || !is_safe_id(&front.writing_id) {
        return Err(format!(
            "文稿 frontmatter 的写作编号与目录不一致：{:?}",
            front.writing_id
        ));
    }
    if front.kind != expected {
        return Err(format!(
            "文稿种类不一致：文件写的是 {}，读取要求 {}。",
            front.kind.as_str(),
            expected.as_str()
        ));
    }
    if front.version == 0 {
        return Err("文稿版本号必须从 1 开始。".to_string());
    }
    validate_timestamp(&front.updated_at, "文稿更新")?;
    if let Some(hash) = front.content_sha256.as_deref() {
        if !is_sha256_hex(hash) {
            return Err(format!("文稿内容指纹形状不正确：{hash}"));
        }
        if sha256_hex(body.as_bytes()) != hash {
            return Err("文稿内容指纹不匹配，文件可能被外部修改。".to_string());
        }
    }
    Ok(body)
}

/// 读取当前大纲正文；文件不存在时返回 None。
pub fn read_outline(root: &Path, writing_id: &str) -> Result<Option<String>, String> {
    read_document(root, writing_id, DocumentKind::Outline)
}

/// 读取当前正文；文件不存在时返回 None。
pub fn read_body(root: &Path, writing_id: &str) -> Result<Option<String>, String> {
    read_document(root, writing_id, DocumentKind::Body)
}

pub fn read_document(
    root: &Path,
    writing_id: &str,
    kind: DocumentKind,
) -> Result<Option<String>, String> {
    let dir = document_dir(root, writing_id)?;
    let path = document_path(root, writing_id, kind)?;
    assert_not_symlink(&dir)?;
    assert_not_symlink(&path)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = read_regular_text(&path, kind.as_str())?;
    Ok(Some(parse_document(&raw, writing_id, kind)?))
}

fn fresh_manifest(writing_id: &str) -> DocumentManifest {
    DocumentManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        writing_id: writing_id.to_string(),
        current_version: 0,
        versions: Vec::new(),
        export_receipts: Vec::new(),
        updated_at: String::new(),
    }
}

fn validate_export_format(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("导出格式不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_FORMAT_CHARS {
        return Err(format!("导出格式过长（最多 {MAX_FORMAT_CHARS} 字）。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("导出格式含控制字符。".to_string());
    }
    Ok(trimmed.to_string())
}

fn validate_export_path(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("导出路径不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_EXPORT_PATH_CHARS {
        return Err(format!(
            "导出路径过长（最多 {MAX_EXPORT_PATH_CHARS} 字符）。"
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("导出路径含控制字符。".to_string());
    }
    Ok(trimmed.to_string())
}

fn validate_manifest(manifest: &DocumentManifest, writing_id: &str) -> Result<(), String> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(format!("文稿 manifest schema 不认识：{}", manifest.schema));
    }
    if manifest.writing_id != writing_id || !is_safe_id(&manifest.writing_id) {
        return Err(format!(
            "文稿 manifest 的写作编号与目录不一致：{:?}",
            manifest.writing_id
        ));
    }
    let mut expected = 1u64;
    for entry in &manifest.versions {
        if entry.version != expected {
            return Err(format!(
                "版本条目必须从 1 连续递增：期望 {expected}，实际 {}。",
                entry.version
            ));
        }
        validate_timestamp(&entry.at, "版本记录")?;
        validate_note(&entry.note)?;
        if !entry.content_sha256.is_empty() && !is_sha256_hex(&entry.content_sha256) {
            return Err(format!(
                "版本 {} 的内容指纹形状不正确。",
                entry.version
            ));
        }
        expected += 1;
    }
    let expected_current = expected - 1;
    if manifest.current_version != expected_current {
        return Err(format!(
            "当前版本号与版本条目不一致：current_version={}，条目应有 {}。",
            manifest.current_version, expected_current
        ));
    }
    for receipt in &manifest.export_receipts {
        validate_export_format(&receipt.format)?;
        validate_export_path(&receipt.path)?;
        validate_timestamp(&receipt.at, "导出回执")?;
    }
    if !manifest.updated_at.is_empty() {
        validate_timestamp(&manifest.updated_at, "manifest 更新")?;
    }
    Ok(())
}

/// 读取 manifest.json；文件不存在时返回空 manifest（版本 0），不创建文件。
pub fn read_manifest(root: &Path, writing_id: &str) -> Result<DocumentManifest, String> {
    let dir = document_dir(root, writing_id)?;
    let path = manifest_path(root, writing_id)?;
    assert_not_symlink(&dir)?;
    assert_not_symlink(&path)?;
    if !path.exists() {
        let fresh = fresh_manifest(writing_id);
        validate_manifest(&fresh, writing_id)?;
        return Ok(fresh);
    }
    let raw = read_regular_text(&path, "文稿 manifest")?;
    let mut manifest: DocumentManifest = serde_json::from_str(&raw)
        .map_err(|e| format!("文稿 manifest 格式错误（{}）：{}", path.display(), e))?;
    // 兼容只写规定字段的 manifest：schema 与 writing_id 缺省时按目录补齐。
    if manifest.schema.is_empty() {
        manifest.schema = MANIFEST_SCHEMA.to_string();
    }
    if manifest.writing_id.is_empty() {
        manifest.writing_id = writing_id.to_string();
    }
    validate_manifest(&manifest, writing_id)?;
    Ok(manifest)
}

static MANIFEST_TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn write_manifest_atomically(path: &Path, manifest: &DocumentManifest) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("manifest 没有父目录：{}", path.display()))?;
    ensure_directory(dir)?;
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化文稿 manifest 失败：{e}"))?;
    let temp = dir.join(format!(
        ".manifest.json.tmp-{}-{}",
        std::process::id(),
        MANIFEST_TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| format!("创建 manifest 临时文件失败（{}）：{}", temp.display(), e))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入 manifest 临时文件失败（{}）：{}", temp.display(), e))?;
        drop(file);
        fs::rename(&temp, path)
            .map_err(|e| format!("替换 manifest 失败（{}）：{}", path.display(), e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn scan_snapshot_versions(dir: &Path) -> Result<Vec<u64>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    assert_not_symlink(dir)?;
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("读取版本目录失败（{}）：{}", dir.display(), e))?;
    let mut versions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取版本目录项失败：{e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("读取版本文件类型失败：{e}"))?
            .is_file()
        {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix('v') else {
            continue;
        };
        let Some((number, _)) = rest.split_once('_') else {
            continue;
        };
        if !name.ends_with(".md") || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit())
        {
            continue;
        }
        if let Ok(version) = number.parse::<u64>() {
            if version > 0 {
                versions.push(version);
            }
        }
    }
    versions.sort_unstable();
    Ok(versions)
}

fn ensure_no_unregistered_snapshots(
    root: &Path,
    writing_id: &str,
    current_version: u64,
) -> Result<(), String> {
    let dir = versions_dir(root, writing_id)?;
    let max = scan_snapshot_versions(&dir)?.last().copied();
    if let Some(max) = max {
        if max > current_version {
            return Err(format!(
                "版本目录里存在 manifest 未登记的版本 v{max}；为保护历史，拒绝继续写入。请先修复 manifest.json。"
            ));
        }
    }
    Ok(())
}

/// 保存文稿内容并新增版本快照。
///
/// 这是模块内唯一的当前文稿写入入口：先写不可覆盖的快照，再替换当前
/// 大纲或正文，最后更新 manifest。快照内容与 manifest 都用该内容的
/// SHA-256 登记，读取历史版本时校验。
pub fn save_with_snapshot(
    root: &Path,
    writing_id: &str,
    spec: &SnapshotSpec<'_>,
    content: &str,
    now: f64,
) -> Result<VersionEntry, String> {
    let dir = document_dir(root, writing_id)?;
    ensure_directory(&dir)?;
    let versions = versions_dir(root, writing_id)?;
    ensure_directory(&versions)?;
    validate_content(content)?;
    let label = normalize_label(spec.label)?;
    let note = validate_note(spec.note)?;
    let canonical = canonical_content(content);
    let mut manifest = read_manifest(root, writing_id)?;
    ensure_no_unregistered_snapshots(root, writing_id, manifest.current_version)?;
    let version = manifest.current_version + 1;
    let at = utc_iso8601(now);
    let content_sha256 = sha256_hex(canonical.as_bytes());
    let snapshot_path = version_path(root, writing_id, version, &label)?;
    write_new_atomically(&snapshot_path, canonical.as_bytes())?;
    let kind = spec.scope.target_kind();
    let current_path = document_path(root, writing_id, kind)?;
    let rendered = render_document(kind, writing_id, version, &at, &canonical, &content_sha256);
    if let Err(error) = write_replace_atomically(&current_path, rendered.as_bytes()) {
        let _ = fs::remove_file(&snapshot_path);
        return Err(format!("写入当前文稿失败：{error}"));
    }
    let entry = VersionEntry {
        version,
        at: at.clone(),
        source: spec.source,
        scope: spec.scope,
        note,
        content_sha256,
    };
    manifest.versions.push(entry.clone());
    manifest.current_version = version;
    manifest.updated_at = at;
    validate_manifest(&manifest, writing_id)?;
    write_manifest_atomically(&manifest_path(root, writing_id)?, &manifest).map_err(|e| {
        format!("当前文稿与快照已写入，但 manifest 更新失败（需修复）：{e}")
    })?;
    Ok(entry)
}

/// 版本列表按版本号升序返回。
pub fn list_versions(root: &Path, writing_id: &str) -> Result<Vec<VersionEntry>, String> {
    Ok(read_manifest(root, writing_id)?.versions)
}

fn find_snapshot_path(dir: &Path, version: u64) -> Result<PathBuf, String> {
    assert_not_symlink(dir)?;
    if !dir.exists() {
        return Err(format!("版本目录不存在：{}", dir.display()));
    }
    let prefix = format!("v{version}_");
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("读取版本目录失败（{}）：{}", dir.display(), e))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取版本目录项失败：{e}"))?;
        if !entry
            .file_type()
            .map_err(|e| format!("读取版本文件类型失败：{e}"))?
            .is_file()
        {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".md") {
            candidates.push(entry.path());
        }
    }
    match candidates.len() {
        0 => Err(format!("找不到版本 v{version} 的快照文件。")),
        1 => Ok(candidates.remove(0)),
        _ => Err(format!("版本 v{version} 有多份快照文件，拒绝猜测。")),
    }
}

/// 读取历史版本并校验内容指纹。
pub fn read_version(
    root: &Path,
    writing_id: &str,
    version: u64,
) -> Result<VersionContent, String> {
    let manifest = read_manifest(root, writing_id)?;
    let entry = manifest
        .versions
        .iter()
        .find(|entry| entry.version == version)
        .cloned()
        .ok_or_else(|| format!("版本 v{version} 不在 manifest 里。"))?;
    let dir = versions_dir(root, writing_id)?;
    let path = find_snapshot_path(&dir, version)?;
    let content = read_regular_text(&path, "历史版本")?;
    if !entry.content_sha256.is_empty()
        && sha256_hex(content.as_bytes()) != entry.content_sha256
    {
        return Err(format!(
            "版本 v{version} 的内容指纹不匹配，快照可能被外部修改：{}",
            path.display()
        ));
    }
    Ok(VersionContent { entry, content })
}

/// 恢复旧版：旧快照内容成为新的当前版，并新增一条历史快照；旧文件全部保留。
pub fn restore_version(
    root: &Path,
    writing_id: &str,
    version: u64,
    now: f64,
) -> Result<VersionEntry, String> {
    let snapshot = read_version(root, writing_id, version)?;
    let label = format!("restore-v{version}");
    let note = format!("恢复自版本 {version}");
    let spec = SnapshotSpec {
        scope: snapshot.entry.scope,
        source: VersionSource::User,
        label: &label,
        note: &note,
    };
    save_with_snapshot(root, writing_id, &spec, &snapshot.content, now)
}

/// 追加一条导出回执，不改动当前版本号与版本历史。
pub fn record_export_receipt(
    root: &Path,
    writing_id: &str,
    format: &str,
    path: &str,
    now: f64,
) -> Result<(DocumentManifest, ExportReceipt), String> {
    let dir = document_dir(root, writing_id)?;
    ensure_directory(&dir)?;
    let format = validate_export_format(format)?;
    let path = validate_export_path(path)?;
    let at = utc_iso8601(now);
    let receipt = ExportReceipt {
        format,
        at: at.clone(),
        path,
    };
    let mut manifest = read_manifest(root, writing_id)?;
    manifest.export_receipts.push(receipt.clone());
    manifest.updated_at = at;
    validate_manifest(&manifest, writing_id)?;
    write_manifest_atomically(&manifest_path(root, writing_id)?, &manifest)?;
    Ok((manifest, receipt))
}

fn split_paragraphs(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }
    paragraphs
}

/// 按空行分段比对两版文本，返回变更段序号与每段前后文本。
pub fn diff_paragraphs(old: &str, new: &str) -> ParagraphDiff {
    let before = split_paragraphs(old);
    let after = split_paragraphs(new);
    let mut diff = ParagraphDiff::default();
    let total = before.len().max(after.len());
    for index in 0..total {
        let old_paragraph = before.get(index);
        let new_paragraph = after.get(index);
        if old_paragraph == new_paragraph {
            continue;
        }
        let kind = match (old_paragraph, new_paragraph) {
            (None, Some(_)) => ParagraphChangeKind::Added,
            (Some(_), None) => ParagraphChangeKind::Removed,
            (Some(_), Some(_)) => ParagraphChangeKind::Modified,
            (None, None) => continue,
        };
        let paragraph_no = (index + 1) as u32;
        diff.changed_paragraph_nos.push(paragraph_no);
        diff.changes.push(ParagraphChange {
            paragraph_no,
            kind,
            before: old_paragraph.cloned(),
            after: new_paragraph.cloned(),
        });
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const NOW: f64 = 1_795_000_000.0;
    const WID: &str = "wt-602-demo";

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wt602-document-{tag}-{}-{}",
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

    fn spec<'a>(
        scope: VersionScope,
        source: VersionSource,
        label: &'a str,
        note: &'a str,
    ) -> SnapshotSpec<'a> {
        SnapshotSpec {
            scope,
            source,
            label,
            note,
        }
    }

    fn save(
        temp: &TempRoot,
        scope: VersionScope,
        source: VersionSource,
        label: &str,
        note: &str,
        content: &str,
        now: f64,
    ) -> VersionEntry {
        save_with_snapshot(
            temp.root(),
            WID,
            &spec(scope, source, label, note),
            content,
            now,
        )
        .unwrap()
    }

    fn snapshot_files(temp: &TempRoot) -> Vec<PathBuf> {
        let dir = versions_dir(temp.root(), WID).unwrap();
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
            .collect();
        files.sort();
        files
    }

    #[test]
    fn save_writes_current_snapshot_and_manifest() {
        let temp = TempRoot::new("save");
        let content = "## 第一章\n\n第一段正文。";
        let entry = save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "chapter-1",
            "章节确认",
            content,
            NOW,
        );
        assert_eq!(entry.version, 1);
        assert_eq!(entry.content_sha256, sha256_hex(content.as_bytes()));
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some(content));
        let body_raw = fs::read_to_string(body_path(temp.root(), WID).unwrap()).unwrap();
        assert!(body_raw.contains("schema: \"omnidesk.document.v1\""), "{body_raw}");
        assert!(body_raw.contains("kind: \"body\""), "{body_raw}");
        let files = snapshot_files(&temp);
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("v1_chapter-1.md"));
        assert_eq!(fs::read_to_string(&files[0]).unwrap(), content);
        let manifest = read_manifest(temp.root(), WID).unwrap();
        assert_eq!(manifest.current_version, 1);
        assert_eq!(manifest.versions, vec![entry.clone()]);
        assert_eq!(manifest.versions[0].source, VersionSource::Ai);
        assert_eq!(manifest.versions[0].scope, VersionScope::Full);
    }

    #[test]
    fn snapshot_modify_snapshot_restore_keeps_full_history() {
        let temp = TempRoot::new("restore");
        let first = "第一段。\n\n第二段。";
        let second = "第一段。\n\n第二段已修改。";
        let v1 = save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "draft-1",
            "初稿",
            first,
            NOW,
        );
        let v2 = save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "draft-2",
            "修改稿",
            second,
            NOW + 60.0,
        );
        assert_eq!(v2.version, 2);
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some(second));
        let restored = restore_version(temp.root(), WID, 1, NOW + 120.0).unwrap();
        assert_eq!(restored.version, 3);
        assert_eq!(restored.source, VersionSource::User);
        assert_eq!(restored.scope, VersionScope::Full);
        assert_eq!(restored.note, "恢复自版本 1");
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some(first));
        let files = snapshot_files(&temp);
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|path| path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("v1_draft-1.md")));
        assert!(files.iter().any(|path| path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("v2_draft-2.md")));
        assert!(files.iter().any(|path| path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("v3_restore-v1.md")));
        let manifest = read_manifest(temp.root(), WID).unwrap();
        assert_eq!(manifest.current_version, 3);
        assert_eq!(manifest.versions.len(), 3);
        assert_eq!(manifest.versions[0], v1);
        assert_eq!(manifest.versions[1], v2);
        assert_eq!(fs::read_to_string(&files[0]).unwrap(), first);
        assert_eq!(read_version(temp.root(), WID, 1).unwrap().content, first);
    }

    #[test]
    fn restore_unknown_version_changes_nothing() {
        let temp = TempRoot::new("restore-missing");
        let content = "唯一版本。";
        save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "only",
            "唯一",
            content,
            NOW,
        );
        let err = restore_version(temp.root(), WID, 9, NOW + 60.0).unwrap_err();
        assert!(err.contains("不在 manifest"), "{err}");
        let manifest = read_manifest(temp.root(), WID).unwrap();
        assert_eq!(manifest.current_version, 1);
        assert_eq!(manifest.versions.len(), 1);
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some(content));
    }

    #[test]
    fn outline_and_body_use_separate_files_and_versions() {
        let temp = TempRoot::new("outline-body");
        let outline = "## 大纲\n\n1. 背景\n2. 进展";
        let body = "## 正文\n\n试点进展平稳。";
        let v1 = save(
            &temp,
            VersionScope::Outline,
            VersionSource::Ai,
            "outline-confirm",
            "大纲确认",
            outline,
            NOW,
        );
        let v2 = save(
            &temp,
            VersionScope::Section,
            VersionSource::Ai,
            "chapter-1",
            "章节确认",
            body,
            NOW + 60.0,
        );
        assert_eq!(v1.scope, VersionScope::Outline);
        assert_eq!(v1.scope.target_kind(), DocumentKind::Outline);
        assert_eq!(v2.scope, VersionScope::Section);
        assert_eq!(v2.scope.target_kind(), DocumentKind::Body);
        assert_eq!(read_outline(temp.root(), WID).unwrap().as_deref(), Some(outline));
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some(body));
        assert!(outline_path(temp.root(), WID).unwrap().exists());
        assert!(body_path(temp.root(), WID).unwrap().exists());
        assert_eq!(read_manifest(temp.root(), WID).unwrap().current_version, 2);
    }

    #[test]
    fn missing_document_reads_as_none_without_creating_files() {
        let temp = TempRoot::new("missing");
        assert!(read_outline(temp.root(), WID).unwrap().is_none());
        assert!(read_body(temp.root(), WID).unwrap().is_none());
        let manifest = read_manifest(temp.root(), WID).unwrap();
        assert_eq!(manifest.current_version, 0);
        assert!(manifest.versions.is_empty());
        assert!(!document_dir(temp.root(), WID).unwrap().exists());
    }

    #[test]
    fn document_paths_require_safe_id_and_stay_under_writing_dir() {
        let temp = TempRoot::new("paths");
        for bad in ["../escape", "a/b", "a\\b", "", "a b"] {
            assert!(document_dir(temp.root(), bad).is_err(), "{bad}");
        }
        let dir = document_dir(temp.root(), WID).unwrap();
        assert!(dir.starts_with(temp.root().join(WRITING_DIR)));
        assert_eq!(dir.file_name().and_then(|name| name.to_str()), Some(WID));
        let result = save_with_snapshot(
            temp.root(),
            "../escape",
            &spec(VersionScope::Full, VersionSource::Ai, "x", "x"),
            "正文",
            NOW,
        );
        assert!(result.is_err());
        assert!(!temp.root().join("escape").exists());
    }

    #[test]
    fn manifest_roundtrips_and_rejects_bad_schema() {
        let temp = TempRoot::new("manifest");
        save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "v1",
            "初稿",
            "正文一。",
            NOW,
        );
        let path = manifest_path(temp.root(), WID).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(json["schema"], MANIFEST_SCHEMA);
        assert_eq!(json["writing_id"], WID);
        assert_eq!(json["current_version"], 1);
        assert_eq!(json["versions"][0]["source"], "ai");
        assert_eq!(json["versions"][0]["scope"], "full");
        let mut manifest = read_manifest(temp.root(), WID).unwrap();
        manifest.schema = "bad.schema".to_string();
        fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        assert!(read_manifest(temp.root(), WID)
            .unwrap_err()
            .contains("schema 不认识"));
    }

    #[test]
    fn manifest_with_only_specified_fields_is_compatible() {
        let temp = TempRoot::new("manifest-compat");
        let dir = document_dir(temp.root(), WID).unwrap();
        fs::create_dir_all(&dir).unwrap();
        let raw = serde_json::json!({
            "current_version": 1,
            "versions": [{
                "version": 1,
                "at": "2026-11-09T12:00:00Z",
                "source": "ai",
                "scope": "full",
                "note": "初稿"
            }],
            "export_receipts": [{
                "format": "md",
                "at": "2026-11-09T12:01:00Z",
                "path": "/tmp/od-wt-602-demo/正式稿.md"
            }]
        });
        fs::write(
            manifest_path(temp.root(), WID).unwrap(),
            serde_json::to_string_pretty(&raw).unwrap(),
        )
        .unwrap();
        let manifest = read_manifest(temp.root(), WID).unwrap();
        assert_eq!(manifest.schema, MANIFEST_SCHEMA);
        assert_eq!(manifest.writing_id, WID);
        assert_eq!(manifest.current_version, 1);
        assert_eq!(manifest.versions.len(), 1);
        assert!(manifest.versions[0].content_sha256.is_empty());
        assert_eq!(manifest.export_receipts.len(), 1);
    }

    #[test]
    fn export_receipts_append_without_moving_current_version() {
        let temp = TempRoot::new("exports");
        save(
            &temp,
            VersionScope::Full,
            VersionSource::User,
            "final",
            "定稿",
            "定稿正文。",
            NOW,
        );
        let (manifest, docx) = record_export_receipt(
            temp.root(),
            WID,
            "docx",
            "/tmp/od-wt-602-demo/正式稿.docx",
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(docx.format, "docx");
        assert_eq!(manifest.current_version, 1);
        assert_eq!(manifest.export_receipts.len(), 1);
        let (manifest, pdf) = record_export_receipt(
            temp.root(),
            WID,
            "pdf",
            "/tmp/od-wt-602-demo/阅读版.pdf",
            NOW + 120.0,
        )
        .unwrap();
        assert_eq!(manifest.current_version, 1);
        assert_eq!(manifest.export_receipts.len(), 2);
        assert_eq!(manifest.export_receipts[1], pdf);
        assert_eq!(list_versions(temp.root(), WID).unwrap().len(), 1);
        let err = record_export_receipt(temp.root(), WID, "", "/tmp/x", NOW).unwrap_err();
        assert!(err.contains("格式不能为空"), "{err}");
    }

    #[test]
    fn diff_marks_changed_paragraph_with_before_and_after() {
        let old = "第一段。\n\n第二段原始内容。\n\n第三段。";
        let new = "第一段。\n\n第二段修改内容。\n\n第三段。";
        let diff = diff_paragraphs(old, new);
        assert_eq!(diff.changed_paragraph_nos, vec![2]);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, ParagraphChangeKind::Modified);
        assert_eq!(diff.changes[0].before.as_deref(), Some("第二段原始内容。"));
        assert_eq!(diff.changes[0].after.as_deref(), Some("第二段修改内容。"));
    }

    #[test]
    fn diff_reports_added_and_removed_paragraphs() {
        let base = "第一段。\n\n第二段。";
        let added = "第一段。\n\n第二段。\n\n第三段新增。";
        let diff = diff_paragraphs(base, added);
        assert_eq!(diff.changed_paragraph_nos, vec![3]);
        assert_eq!(diff.changes[0].kind, ParagraphChangeKind::Added);
        assert!(diff.changes[0].before.is_none());
        assert_eq!(diff.changes[0].after.as_deref(), Some("第三段新增。"));

        let removed = diff_paragraphs(added, base);
        assert_eq!(removed.changed_paragraph_nos, vec![3]);
        assert_eq!(removed.changes[0].kind, ParagraphChangeKind::Removed);
        assert_eq!(removed.changes[0].before.as_deref(), Some("第三段新增。"));
        assert!(removed.changes[0].after.is_none());
    }

    #[test]
    fn diff_ignores_blank_line_count_and_crlf() {
        let old = "第一段。\n\n第二段。";
        let new = "第一段。\r\n\r\n\r\n第二段。";
        let diff = diff_paragraphs(old, new);
        assert!(diff.changed_paragraph_nos.is_empty());
        assert!(diff.changes.is_empty());
        let same = diff_paragraphs("", "");
        assert!(same.changed_paragraph_nos.is_empty());
    }

    #[test]
    fn snapshot_label_and_note_are_validated() {
        let temp = TempRoot::new("labels");
        let err = save_with_snapshot(
            temp.root(),
            WID,
            &spec(VersionScope::Full, VersionSource::Ai, "../escape", "说明"),
            "正文",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("快照标签不合法"), "{err}");
        let err = save_with_snapshot(
            temp.root(),
            WID,
            &spec(VersionScope::Full, VersionSource::Ai, "ok", "说明\n第二行"),
            "正文",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("控制字符"), "{err}");
        let entry = save_with_snapshot(
            temp.root(),
            WID,
            &spec(VersionScope::Full, VersionSource::Ai, "", "无标签"),
            "正文",
            NOW,
        )
        .unwrap();
        assert_eq!(entry.version, 1);
        let files = snapshot_files(&temp);
        assert!(files[0]
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("v1_snapshot.md"));
    }

    #[test]
    fn tampered_snapshot_is_rejected_but_remains_on_disk() {
        let temp = TempRoot::new("tamper");
        save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "v1",
            "初稿",
            "原始正文。",
            NOW,
        );
        let files = snapshot_files(&temp);
        let path = &files[0];
        fs::write(path, "被篡改的正文。").unwrap();
        let err = read_version(temp.root(), WID, 1).unwrap_err();
        assert!(err.contains("指纹不匹配"), "{err}");
        assert!(path.exists(), "历史文件不得因校验失败被删除");
        assert_eq!(list_versions(temp.root(), WID).unwrap().len(), 1);
    }

    #[test]
    fn unregistered_snapshot_blocks_new_version_without_overwriting_history() {
        let temp = TempRoot::new("orphan");
        save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "v1",
            "初稿",
            "正文一。",
            NOW,
        );
        let dir = versions_dir(temp.root(), WID).unwrap();
        let orphan = dir.join("v2_orphan.md");
        fs::write(&orphan, "未登记的版本。").unwrap();
        let err = save_with_snapshot(
            temp.root(),
            WID,
            &spec(VersionScope::Full, VersionSource::Ai, "v2", "第二版"),
            "正文二。",
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("未登记"), "{err}");
        assert!(orphan.exists());
        assert_eq!(read_body(temp.root(), WID).unwrap().as_deref(), Some("正文一。"));
        assert_eq!(read_manifest(temp.root(), WID).unwrap().current_version, 1);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_document_directory() {
        use std::os::unix::fs::symlink;

        let temp = TempRoot::new("symlink");
        let outside = temp.root().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let writing_root = temp.root().join(WRITING_DIR);
        fs::create_dir_all(&writing_root).unwrap();
        let link = writing_root.join(WID);
        symlink(&outside, &link).unwrap();
        let err = save_with_snapshot(
            temp.root(),
            WID,
            &spec(VersionScope::Full, VersionSource::Ai, "v1", "初稿"),
            "正文",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("符号链接"), "{err}");
        assert!(!outside.join(BODY_FILE).exists());
        assert!(!outside.join(VERSIONS_DIR).exists());
    }

    #[test]
    fn manifest_tamper_with_nonconsecutive_versions_is_rejected() {
        let temp = TempRoot::new("manifest-order");
        save(
            &temp,
            VersionScope::Full,
            VersionSource::Ai,
            "v1",
            "初稿",
            "正文一。",
            NOW,
        );
        let mut manifest = read_manifest(temp.root(), WID).unwrap();
        manifest.current_version = 2;
        manifest.versions[0].version = 2;
        let path = manifest_path(temp.root(), WID).unwrap();
        fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        assert!(read_manifest(temp.root(), WID)
            .unwrap_err()
            .contains("连续递增"));
    }
}
