//! 读书阅读笔记的本地存储（OD-READ-001）。
//!
//! 书目与批注只落仓库外的阅读目录，默认 `~/OmniDesk/reading/`：
//!   books/<book_id>.json        正文、章节、批注、已保存整理稿摘要
//!   meta/<book_id>.json         书架列表用的轻量摘要
//!   notes/<book_id>/<note>.md   用户确认保存的整理稿
//!   exports/<book>-笔记-<time>.md 确定性导出的读书笔记
//!
//! 边界：只读用户显式选择的源文件；不扫描目录；不写知识库；不把正文放进日志。
//! 字符偏移一律按 Unicode 标量（Rust `char` / 前端 code point）计，避免中文与 emoji
//! 被 UTF-8 字节或 UTF-16 代理对切坏。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app_paths::AppPaths;
use crate::platform::home_dir;
use crate::settings::assert_artifact_root_allowed;
use crate::task_shell::{
    assert_local_root_allowed, assert_outside_repo, read_reading_source, utc_iso8601,
    ReadingSource,
};
use crate::worklog_records::{
    is_safe_id, write_new_atomically, write_replace_atomically,
};

pub const READING_SCHEMA: &str = "omnidesk.reading.v1";
pub const READING_ROOT_ENV: &str = "OMNIDESK_READING_ROOT";
pub const DEFAULT_READING_SUBDIR: &str = "OmniDesk/reading";

pub const KIND_HIGHLIGHT: &str = "highlight";
pub const KIND_BOOKMARK: &str = "bookmark";
pub const KIND_THOUGHT: &str = "thought";
pub const ANNOTATION_KINDS: &[&str] = &[KIND_HIGHLIGHT, KIND_BOOKMARK, KIND_THOUGHT];

const SOURCE_USER: &str = "user";
const SOURCE_AI: &str = "ai";
const SOURCE_KINDS: &[&str] = &[SOURCE_USER, SOURCE_AI];
const COLORS: &[&str] = &["amber", "blue", "green", "pink"];
const MAX_QUOTE_CHARS: usize = 4_000;
const MAX_BODY_CHARS: usize = 20_000;
const MAX_NOTE_CHARS: usize = 2_000_000;
const MAX_BOOK_CHARS: usize = 20_000_000;

static ID_SEQ: AtomicU64 = AtomicU64::new(0);
/// progress/<id>.json 是「进度 + 每日计时」的读改写文件；串行化避免滚动保存与计时落盘互相覆盖。
static PROGRESS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingChapter {
    pub title: String,
    pub paragraphs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingProgress {
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub updated_at: String,
    /// 按 UTC 日期（YYYY-MM-DD）累计的真实阅读秒数；旧版进度文件没有这个字段。
    #[serde(default)]
    pub daily_seconds: BTreeMap<String, u64>,
}

impl Default for ReadingProgress {
    fn default() -> Self {
        Self {
            percent: 0.0,
            updated_at: String::new(),
            daily_seconds: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingAnnotation {
    pub id: String,
    pub kind: String,
    pub chapter_index: usize,
    pub paragraph_index: usize,
    #[serde(default)]
    pub start: usize,
    #[serde(default)]
    pub end: usize,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub color: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

fn default_source() -> String {
    SOURCE_USER.to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingAnnotationDraft {
    pub kind: String,
    pub chapter_index: usize,
    pub paragraph_index: usize,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub quote: String,
    pub body: String,
    pub color: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingAnnotationPatch {
    pub body: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingSavedNote {
    pub note_id: String,
    pub title: String,
    pub created_at: String,
    pub relative_path: String,
    pub bytes: u64,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
    #[serde(default)]
    pub verified_citations: usize,
    #[serde(default)]
    pub unverified_citations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingBook {
    #[serde(default = "schema_default")]
    pub schema: String,
    pub book_id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub source_format: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub source_sha256: String,
    /// 导入时从 EPUB OPF 提取的封面文件名（meta/ 下），无封面为空。
    #[serde(default)]
    pub cover: String,
    pub imported_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub chapters: Vec<ReadingChapter>,
    #[serde(default)]
    pub annotations: Vec<ReadingAnnotation>,
    #[serde(default)]
    pub notes: Vec<ReadingSavedNote>,
    #[serde(default)]
    pub progress: ReadingProgress,
    /// 正文媒体标记版本：0 = 媒体提取上线前的旧正文；1 = 已带 odmedia/![img] 标记。
    #[serde(default)]
    pub media_version: u32,
}

fn schema_default() -> String {
    READING_SCHEMA.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingBookMeta {
    #[serde(default = "schema_default")]
    pub schema: String,
    pub book_id: String,
    pub title: String,
    pub author: String,
    pub source_name: String,
    pub source_format: String,
    pub imported_at: String,
    pub updated_at: String,
    pub chapter_count: usize,
    pub paragraph_count: usize,
    pub char_count: usize,
    pub annotation_count: usize,
    pub note_count: usize,
    /// thought 批注数；`None` 表示旧版 meta 没写过这个字段，聚合时回读正文补算。
    #[serde(default)]
    pub thought_count: Option<usize>,
    /// 封面文件名（meta/ 下）；旧版书目没有封面。
    #[serde(default)]
    pub cover: Option<String>,
    pub progress_percent: f64,
    #[serde(default)]
    pub media_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingImportResult {
    pub book: ReadingBookMeta,
    pub existing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingLibrary {
    pub root: String,
    pub books: Vec<ReadingBookMeta>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingNoteFile {
    pub file_name: String,
    pub relative_path: String,
    pub content: String,
    pub bytes: u64,
}

/// 首页阅读统计（OD-READ-003）。`books` 只算本地书籍，订阅文章篇数由 rss_store 另计。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingStats {
    pub books: usize,
    /// 读完口径：progress_percent >= 0.995（与界面显示的 100% 完成一致）。
    pub finished: usize,
    pub chars: usize,
    pub annotations: usize,
    pub notes: usize,
    pub thoughts: usize,
    /// 本月（UTC）真实阅读分钟数，按秒累计后四舍五入。
    pub month_minutes: u64,
    /// 从今天向前连续有阅读秒数的天数；今天还没有记录时从昨天起算。
    pub streak_days: u32,
}

/// 旧正文回填结果：更新后的书 + 未能按引文重新定位的批注数。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingRefreshResult {
    pub book: ReadingBook,
    pub unresolved: usize,
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn new_id(prefix: &str, now: f64) -> String {
    let stamp = (now.max(0.0) * 1000.0) as u64;
    let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{stamp}-{seq}")
}

/// 阅读目录：开发/测试可用 `OMNIDESK_READING_ROOT` 指向一次性根；正式默认在仓库外。
pub fn resolve_reading_root(paths: &AppPaths) -> Result<PathBuf, String> {
    let env_override = std::env::var_os(READING_ROOT_ENV).filter(|v| !v.is_empty());
    let root = match env_override {
        Some(raw) => {
            let path = PathBuf::from(raw);
            if !path.is_absolute() {
                return Err(format!("{} 必须是绝对路径", READING_ROOT_ENV));
            }
            assert_local_root_allowed(&path)?;
            assert_outside_repo(&path)?;
            path
        }
        None => {
            let home = home_dir().ok_or_else(|| "无法确定用户主目录，无法定位阅读目录".to_string())?;
            let path = home.join(DEFAULT_READING_SUBDIR);
            assert_artifact_root_allowed(&path, Some(&paths.root))?;
            path
        }
    };
    if !root.is_absolute() {
        return Err(format!("阅读目录必须是绝对路径：{}", root.display()));
    }
    fs::create_dir_all(&root)
        .map_err(|e| format!("创建阅读目录失败（{}）：{}", root.display(), e))?;
    for name in ["books", "meta", "progress", "notes", "exports"] {
        fs::create_dir_all(root.join(name))
            .map_err(|e| format!("创建阅读子目录 {} 失败：{}", name, e))?;
    }
    // 创建后再校验一次，避免软链接把真实落点引到禁区。
    if std::env::var_os(READING_ROOT_ENV).filter(|v| !v.is_empty()).is_some() {
        assert_local_root_allowed(&root)?;
        assert_outside_repo(&root)?;
    } else {
        assert_artifact_root_allowed(&root, Some(&paths.root))?;
    }
    Ok(root)
}

fn assert_store_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Err(format!("阅读目录必须是绝对路径：{}", root.display()));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn harden_private(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
pub(crate) fn harden_private(_path: &Path, _mode: u32) {}

pub(crate) fn ensure_store_dirs(root: &Path) -> Result<(), String> {
    assert_store_root(root)?;
    for name in ["books", "meta", "progress", "notes", "exports"] {
        fs::create_dir_all(root.join(name))
            .map_err(|e| format!("创建阅读子目录 {} 失败（{}）：{}", name, root.display(), e))?;
        harden_private(&root.join(name), 0o700);
    }
    harden_private(root, 0o700);
    Ok(())
}

fn book_path(root: &Path, book_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    Ok(root.join("books").join(format!("{book_id}.json")))
}

fn meta_path(root: &Path, book_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    Ok(root.join("meta").join(format!("{book_id}.json")))
}

fn progress_path(root: &Path, book_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    Ok(root.join("progress").join(format!("{book_id}.json")))
}

fn safe_file_stem(title: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in title.trim().chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' || ('\u{4e00}'..='\u{9fff}').contains(&ch) {
            out.push(ch);
        }
        if out.chars().count() >= 80 {
            break;
        }
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed
    }
}

fn book_meta(book: &ReadingBook) -> ReadingBookMeta {
    let paragraphs: usize = book.chapters.iter().map(|c| c.paragraphs.len()).sum();
    let chars: usize = book
        .chapters
        .iter()
        .flat_map(|c| c.paragraphs.iter())
        .map(|p| p.chars().count())
        .sum();
    ReadingBookMeta {
        schema: READING_SCHEMA.to_string(),
        book_id: book.book_id.clone(),
        title: book.title.clone(),
        author: book.author.clone(),
        source_name: book.source_name.clone(),
        source_format: book.source_format.clone(),
        imported_at: book.imported_at.clone(),
        updated_at: book.updated_at.clone(),
        chapter_count: book.chapters.len(),
        paragraph_count: paragraphs,
        char_count: chars,
        annotation_count: book.annotations.len(),
        note_count: book.notes.len(),
        thought_count: Some(book.annotations.iter().filter(|a| a.kind == KIND_THOUGHT).count()),
        cover: if book.cover.trim().is_empty() { None } else { Some(book.cover.clone()) },
        progress_percent: book.progress.percent,
        media_version: book.media_version,
    }
}

fn apply_chapters(source: &ReadingSource) -> Vec<ReadingChapter> {
    if !source.chapters.is_empty() {
        let chapters: Vec<ReadingChapter> = source
            .chapters
            .iter()
            .filter_map(|chapter| {
                let paragraphs = split_paragraphs(&chapter.content);
                if paragraphs.is_empty() {
                    None
                } else {
                    Some(ReadingChapter {
                        title: if chapter.title.trim().is_empty() {
                            "正文".to_string()
                        } else {
                            chapter.title.trim().to_string()
                        },
                        paragraphs,
                    })
                }
            })
            .collect();
        if !chapters.is_empty() {
            return chapters;
        }
    }
    split_chapters(&source.content, &source.file_name)
}

fn split_chapters(content: &str, file_name: &str) -> Vec<ReadingChapter> {
    let fallback_title = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "正文".to_string());
    let mut chapters: Vec<(String, Vec<String>)> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();
    for line in content.lines() {
        if let Some(title) = heading_title(line) {
            if let Some(prev) = current_title.take() {
                chapters.push((prev, std::mem::take(&mut current_lines)));
            } else if !current_lines.iter().any(|l| !l.trim().is_empty()) {
                current_lines.clear();
            }
            current_title = Some(title);
            continue;
        }
        current_lines.push(line.to_string());
    }
    if let Some(prev) = current_title.take() {
        chapters.push((prev, std::mem::take(&mut current_lines)));
    } else {
        chapters.push((fallback_title.clone(), current_lines));
    }
    let mut out: Vec<ReadingChapter> = chapters
        .into_iter()
        .filter_map(|(title, lines)| {
            let body = lines.join("\n");
            let paragraphs = split_paragraphs(&body);
            if paragraphs.is_empty() {
                None
            } else {
                Some(ReadingChapter { title, paragraphs })
            }
        })
        .collect();
    if out.is_empty() {
        let paragraphs = split_paragraphs(content);
        if !paragraphs.is_empty() {
            out.push(ReadingChapter {
                title: fallback_title,
                paragraphs,
            });
        }
    }
    out
}

fn heading_title(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = trimmed[hashes..].trim();
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    let chars = trimmed.chars().count();
    if chars <= 48
        && trimmed.starts_with('第')
        && trimmed
            .chars()
            .take(20)
            .any(|c| c == '章' || c == '节' || c == '卷' || c == '篇')
    {
        return Some(trimmed.to_string());
    }
    let lower = trimmed.to_lowercase();
    if chars <= 80 && (lower.starts_with("chapter ") || lower.starts_with("part ")) {
        return Some(trimmed.to_string());
    }
    None
}

fn split_paragraphs(content: &str) -> Vec<String> {
    let has_blank_sep = content.lines().any(|line| line.trim().is_empty());
    if has_blank_sep {
        let mut out: Vec<String> = Vec::new();
        let mut block: Vec<&str> = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                if !block.is_empty() {
                    let joined = block.join("\n").trim().to_string();
                    if !joined.is_empty() {
                        out.push(joined);
                    }
                    block.clear();
                }
            } else {
                block.push(line);
            }
        }
        if !block.is_empty() {
            let joined = block.join("\n").trim().to_string();
            if !joined.is_empty() {
                out.push(joined);
            }
        }
        out
    } else {
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }
}

fn paragraph_text<'a>(book: &'a ReadingBook, chapter: usize, paragraph: usize) -> Result<&'a str, String> {
    let chapter = book
        .chapters
        .get(chapter)
        .ok_or_else(|| format!("章节索引越界：{chapter}"))?;
    chapter
        .paragraphs
        .get(paragraph)
        .map(|p| p.as_str())
        .ok_or_else(|| format!("段落索引越界：{paragraph}"))
}

fn resolve_span(text: &str, start: usize, end: usize, quote: &str) -> Result<(usize, usize, String), String> {
    let chars: Vec<char> = text.chars().collect();
    let quote = quote.trim();
    if quote.is_empty() {
        return Err("划线的引文为空。".to_string());
    }
    if start < end && end <= chars.len() {
        let selected: String = chars[start..end].iter().collect();
        if selected == quote {
            return Ok((start, end, selected));
        }
        if selected.trim() == quote {
            let leading = selected.chars().take_while(|c| c.is_whitespace()).count();
            let trailing = selected.chars().rev().take_while(|c| c.is_whitespace()).count();
            let new_start = start + leading;
            let new_end = end.saturating_sub(trailing);
            if new_start < new_end {
                return Ok((new_start, new_end, quote.to_string()));
            }
        }
    }
    // 原文可能因导入规范化略有差异：按引文在段落中重新定位，仍找不到就拒绝。
    let needle: Vec<char> = quote.chars().collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return Err("引文与当前段落不一致，请重新选择。".to_string());
    }
    let mut found: Option<usize> = None;
    let mut matches = 0usize;
    for i in 0..=chars.len().saturating_sub(needle.len()) {
        if chars[i..i + needle.len()] == needle[..] {
            matches += 1;
            if found.is_none() {
                found = Some(i);
            }
        }
    }
    if matches == 1 {
        if let Some(i) = found {
            return Ok((i, i + needle.len(), quote.to_string()));
        }
    }
    if matches > 1 {
        return Err("引文在当前段落中出现多次，无法安全定位；请重新选择更明确的片段。".to_string());
    }
    Err("引文在当前段落中找不到；原文可能已变化，请重新选择。".to_string())
}

fn normalize_color(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    if COLORS.contains(&value.as_str()) {
        value
    } else {
        "amber".to_string()
    }
}

const MAX_COVER_BYTES: usize = 2 * 1024 * 1024;

fn cover_extension(mime: &str, file_name: &str) -> Option<&'static str> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/avif" => Some("avif"),
        _ => {
            let ext = Path::new(file_name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())?;
            match ext.as_str() {
                "png" => Some("png"),
                "jpg" | "jpeg" => Some("jpg"),
                "gif" => Some("gif"),
                "webp" => Some("webp"),
                "bmp" => Some("bmp"),
                "avif" => Some("avif"),
                _ => None,
            }
        }
    }
}

/// 封面只从导入时已解析出的 OPF cover-image 字节写入 meta/；失败不阻断正文导入。
fn store_cover(root: &Path, book_id: &str, cover: &crate::task_shell::ReadingSourceCover) -> Option<String> {
    if cover.bytes.is_empty() || cover.bytes.len() > MAX_COVER_BYTES {
        return None;
    }
    let ext = cover_extension(&cover.mime, &cover.file_name)?;
    let file_name = format!("{book_id}.cover.{ext}");
    let target = root.join("meta").join(&file_name);
    let write = if target.exists() {
        write_replace_atomically(&target, &cover.bytes)
    } else {
        write_new_atomically(&target, &cover.bytes)
    };
    if write.is_err() {
        return None;
    }
    harden_private(&target, 0o600);
    Some(file_name)
}

pub fn cover_path(root: &Path, book_id: &str, cover: &str) -> Result<PathBuf, String> {
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    let name = Path::new(cover).file_name().and_then(|value| value.to_str()).unwrap_or("");
    if name != cover || !cover.starts_with(&format!("{book_id}.cover.")) {
        return Err("封面文件名不合法。".to_string());
    }
    Ok(root.join("meta").join(cover))
}

fn cover_mime_for(file_name: &str) -> Option<&'static str> {
    match Path::new(file_name).extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn read_cover_payload(
    root: &Path,
    book_id: &str,
    cover: &str,
) -> Result<crate::task_shell::ReadingMediaPayload, String> {
    let path = cover_path(root, book_id, cover)?;
    let bytes = fs::read(&path).map_err(|e| format!("读取封面失败（{}）：{}", path.display(), e))?;
    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
        return Err("封面大小不合法。".to_string());
    }
    let mime = cover_mime_for(cover).ok_or_else(|| "封面格式不在白名单内。".to_string())?;
    Ok(crate::task_shell::ReadingMediaPayload {
        mime: mime.to_string(),
        bytes: bytes.len(),
        data_base64: crate::task_shell::base64_encode(&bytes),
    })
}

/// 旧书目导入时没有落封面：按需回读源文件，把提取到的 cover 写进 meta/ 并更新摘要。
fn extract_cover_from_source(root: &Path, book_id: &str) -> Option<String> {
    let book = read_book_at(root, book_id).ok()?;
    if book.source_format == "rss" || book.source_path.trim().is_empty() {
        return None;
    }
    let source = read_reading_source(&book.source_path).ok()?;
    let cover = source.cover.as_ref()?;
    store_cover(root, book_id, cover)
}

/// 读 meta/ 下的封面字节，返回前端可直接转 data URL 的 payload。
///
/// meta.cover 缺失或缓存文件损坏时，从真实源文件重新提取一次；提取失败仍按
/// 「这本书没有封面」返回，让前端落到首字占位而不是显示内部错误。
pub fn read_cover_at(root: &Path, book_id: &str) -> Result<crate::task_shell::ReadingMediaPayload, String> {
    let mut meta = read_meta_at(root, book_id)?;
    let cached = meta
        .cover
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    if let Some(cover) = cached {
        if let Ok(payload) = read_cover_payload(root, book_id, &cover) {
            return Ok(payload);
        }
    }
    let extracted = extract_cover_from_source(root, book_id)
        .ok_or_else(|| "这本书没有封面。".to_string())?;
    let payload = read_cover_payload(root, book_id, &extracted)?;
    meta.cover = Some(extracted);
    let raw = serde_json::to_vec_pretty(&meta).map_err(|e| format!("序列化书目摘要失败：{e}"))?;
    write_replace_atomically(&meta_path(root, book_id)?, &raw)
        .map_err(|e| format!("保存封面索引失败：{e}"))?;
    harden_private(&meta_path(root, book_id)?, 0o600);
    Ok(payload)
}

fn remove_tracked_file(
    path: &Path,
    label: String,
    removed: &mut Vec<String>,
    failures: &mut Vec<String>,
) {
    match fs::remove_file(path) {
        Ok(()) => removed.push(label),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => failures.push(format!("{label}：{e}")),
    }
}

/// 本地书籍改名：只改写 books/<book_id>.json 与 meta/<book_id>.json 的 title。
/// 订阅文章（source_format = rss）由文章元数据管理，拒绝改名。
pub fn rename_book_at(root: &Path, book_id: &str, title: &str) -> Result<ReadingBookMeta, String> {
    assert_store_root(root)?;
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    let next_title = title.trim();
    let title_chars = next_title.chars().count();
    if !(1..=300).contains(&title_chars) {
        return Err("书名需为 1 到 300 个字符。".to_string());
    }
    let existing = read_meta_at(root, book_id)
        .map_err(|e| format!("这本书不存在或摘要损坏，未改名：{e}"))?;
    if existing.source_format == "rss" {
        return Err("订阅文章的标题跟随订阅源，不能改名。".to_string());
    }

    let book_target = book_path(root, book_id)?;
    let meta_target = meta_path(root, book_id)?;
    let book_raw = fs::read_to_string(&book_target)
        .map_err(|e| format!("读取书籍失败（{}）：{}", book_target.display(), e))?;
    let meta_raw = fs::read_to_string(&meta_target)
        .map_err(|e| format!("读取书目摘要失败（{}）：{}", meta_target.display(), e))?;
    let mut book_value: serde_json::Value = serde_json::from_str(&book_raw)
        .map_err(|e| format!("书籍文件格式不正确（{}）：{}", book_target.display(), e))?;
    let mut meta_value: serde_json::Value = serde_json::from_str(&meta_raw)
        .map_err(|e| format!("书目摘要格式不正确（{}）：{}", meta_target.display(), e))?;
    let book_id_matches = book_value
        .get("book_id")
        .and_then(|value| value.as_str())
        == Some(book_id)
        && meta_value
            .get("book_id")
            .and_then(|value| value.as_str())
            == Some(book_id);
    if !book_id_matches {
        return Err(format!("书籍文件编号不一致：{book_id}"));
    }
    if !book_value.is_object() || !meta_value.is_object() {
        return Err("书籍文件结构不正确，未改名。".to_string());
    }
    book_value["title"] = serde_json::Value::String(next_title.to_string());
    meta_value["title"] = serde_json::Value::String(next_title.to_string());

    let book_bytes = serde_json::to_vec_pretty(&book_value)
        .map_err(|e| format!("序列化书籍失败：{e}"))?;
    let meta_bytes = serde_json::to_vec_pretty(&meta_value)
        .map_err(|e| format!("序列化书目摘要失败：{e}"))?;
    write_replace_atomically(&book_target, &book_bytes)?;
    write_replace_atomically(&meta_target, &meta_bytes)?;
    harden_private(&book_target, 0o600);
    harden_private(&meta_target, 0o600);

    serde_json::from_value(meta_value)
        .map_err(|e| format!("书目摘要格式不正确（{}）：{}", meta_target.display(), e))
}

/// 删除一本本地书籍：正文、摘要、进度、笔记目录与封面；exports/ 始终保留。
///
/// 只接受安全编号；meta 标记为 rss 的订阅文章一律拒绝。删除按项进行，
/// 中途失败时返回已删项与失败项，便于用户判断目录实际状态。
pub fn delete_book_at(root: &Path, book_id: &str) -> Result<(), String> {
    assert_store_root(root)?;
    if !is_safe_id(book_id) {
        return Err(format!("书编号不合法：{book_id:?}"));
    }
    let meta = read_meta_at(root, book_id)
        .map_err(|e| format!("这本书不存在或摘要损坏，未删除任何文件：{e}"))?;
    if meta.source_format == "rss" {
        return Err("订阅文章不能从书架删除；请到订阅管理里处理订阅源。".to_string());
    }
    let mut removed: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    // 封面可能有多份旧扩展名；按前缀清掉，避免孤儿文件继续占用空间。
    let meta_dir = root.join("meta");
    match fs::read_dir(&meta_dir) {
        Ok(entries) => {
            let prefix = format!("{book_id}.cover.");
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) {
                    remove_tracked_file(
                        &entry.path(),
                        format!("meta/{name}"),
                        &mut removed,
                        &mut failures,
                    );
                }
            }
        }
        Err(e) => failures.push(format!("读取封面目录失败：{e}")),
    }

    let notes_dir = root.join("notes").join(book_id);
    if notes_dir.exists() {
        match fs::remove_dir_all(&notes_dir) {
            Ok(()) => removed.push(format!("notes/{book_id}/")),
            Err(e) => failures.push(format!("notes/{book_id}/：{e}")),
        }
    }

    if let Ok(path) = progress_path(root, book_id) {
        remove_tracked_file(&path, format!("progress/{book_id}.json"), &mut removed, &mut failures);
    }
    if let Ok(path) = book_path(root, book_id) {
        remove_tracked_file(&path, format!("books/{book_id}.json"), &mut removed, &mut failures);
    }
    // 摘要最后删：中途失败时 meta 还在，用户至少还能重试一次删除。
    if let Ok(path) = meta_path(root, book_id) {
        remove_tracked_file(&path, format!("meta/{book_id}.json"), &mut removed, &mut failures);
    }

    if failures.is_empty() {
        return Ok(());
    }
    let removed_text = if removed.is_empty() {
        "无".to_string()
    } else {
        removed.join("、")
    };
    Err(format!(
        "删除《{}》未完全成功。已删除：{}；失败：{}",
        meta.title,
        removed_text,
        failures.join("、")
    ))
}
pub fn import_book_at(root: &Path, source: ReadingSource, now: f64) -> Result<ReadingImportResult, String> {
    ensure_store_dirs(root)?;
    if source.content.chars().count() > MAX_BOOK_CHARS {
        return Err(format!("正文超过 {MAX_BOOK_CHARS} 字符阅读上限，请拆分后导入。"));
    }
    let book_id = format!("bk_{}", &source.sha256[..16.min(source.sha256.len())]);
    if let Ok(meta) = read_meta_at(root, &book_id) {
        return Ok(ReadingImportResult { book: meta, existing: true });
    }
    // 正文已在、meta 丢失（例如被手工删除）：从正文重建摘要，不重复写正文。
    if let Ok(existing) = read_book_at(root, &book_id) {
        write_book(root, &existing)?;
        return Ok(ReadingImportResult {
            book: book_meta(&existing),
            existing: true,
        });
    }
    let title = Path::new(&source.file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未命名".to_string());
    let chapters = apply_chapters(&source);
    if chapters.is_empty() {
        return Err(format!("「{}」没有可读的章节正文。", source.file_name));
    }
    let cover_file = source
        .cover
        .as_ref()
        .and_then(|cover| store_cover(root, &book_id, cover));
    let imported_at = utc_iso8601(now);
    let book = ReadingBook {
        schema: READING_SCHEMA.to_string(),
        book_id: book_id.clone(),
        title,
        author: source.author.trim().to_string(),
        source_name: source.file_name,
        source_format: source.extension,
        source_path: source.path,
        source_sha256: source.sha256,
        cover: cover_file.unwrap_or_default(),
        imported_at: imported_at.clone(),
        updated_at: imported_at,
        chapters,
        annotations: Vec::new(),
        notes: Vec::new(),
        progress: ReadingProgress::default(),
        media_version: 1,
    };
    write_book(root, &book)?;
    Ok(ReadingImportResult { book: book_meta(&book), existing: false })
}

pub fn import_book(paths: &AppPaths, path: &str) -> Result<ReadingImportResult, String> {
    let root = resolve_reading_root(paths)?;
    let source = read_reading_source(path)?;
    import_book_at(&root, source, now_secs())
}

pub fn list_books_at(root: &Path) -> Result<Vec<ReadingBookMeta>, String> {
    assert_store_root(root)?;
    let dir = root.join("meta");
    let mut books: Vec<ReadingBookMeta> = Vec::new();
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取阅读书目目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else { continue };
        if let Ok(meta) = serde_json::from_str::<ReadingBookMeta>(&raw) {
            books.push(meta);
        }
    }
    books.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.title.cmp(&b.title)));
    Ok(books)
}

pub fn library(paths: &AppPaths) -> Result<ReadingLibrary, String> {
    let root = resolve_reading_root(paths)?;
    Ok(ReadingLibrary {
        root: root.display().to_string(),
        books: list_books_at(&root)?,
    })
}

/// 首页统计聚合。所有数字都来自真实书目摘要/正文，不补零也不估算。
/// 「闪记」按用户当轮语义 = `kind == "thought"` 的批注数。
pub fn reading_stats_at(root: &Path) -> Result<ReadingStats, String> {
    reading_stats_at_now(root, now_secs())
}

fn reading_stats_at_now(root: &Path, now: f64) -> Result<ReadingStats, String> {
    assert_store_root(root)?;
    let books = list_books_at(root)?;
    let mut stats = ReadingStats {
        books: 0,
        finished: 0,
        chars: 0,
        annotations: 0,
        notes: 0,
        thoughts: 0,
        month_minutes: 0,
        streak_days: 0,
    };
    let mut daily_seconds: BTreeMap<String, u64> = BTreeMap::new();
    for meta in books {
        // 书籍本数不含订阅文章；其余计数覆盖书架里的全部阅读条目。
        if meta.source_format != "rss" {
            stats.books += 1;
            if meta.progress_percent >= 0.995 {
                stats.finished += 1;
            }
        }
        stats.chars += meta.char_count;
        stats.annotations += meta.annotation_count;
        stats.notes += meta.note_count;
        stats.thoughts += match meta.thought_count {
            Some(count) => count,
            // 旧版 meta 没有 thought_count：回读正文补算，避免把历史闪记显示成 0。
            None => read_book_at(root, &meta.book_id)?
                .annotations
                .iter()
                .filter(|annotation| annotation.kind == KIND_THOUGHT)
                .count(),
        };
        for (day, seconds) in read_progress_at(root, &meta.book_id).daily_seconds {
            *daily_seconds.entry(day).or_insert(0) += seconds;
        }
    }
    let today = utc_iso8601(now);
    let month_prefix = &today[..7];
    let month_secs: u64 = daily_seconds
        .iter()
        .filter(|(day, _)| day.get(..7) == Some(month_prefix))
        .map(|(_, seconds)| *seconds)
        .sum();
    stats.month_minutes = (month_secs + 30) / 60;
    let today_key = &today[..10];
    let today_secs = daily_seconds.get(today_key).copied().unwrap_or(0);
    let mut day_num = (now.max(0.0) / 86_400.0).floor() as i64;
    if today_secs == 0 {
        day_num -= 1;
    }
    while daily_seconds
        .get(&utc_iso8601(day_num as f64 * 86_400.0)[..10])
        .copied()
        .unwrap_or(0)
        > 0
    {
        stats.streak_days += 1;
        day_num -= 1;
    }
    Ok(stats)
}

fn read_meta_at(root: &Path, book_id: &str) -> Result<ReadingBookMeta, String> {
    let path = meta_path(root, book_id)?;
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取书目摘要失败（{}）：{}", path.display(), e))?;
    serde_json::from_str(&raw).map_err(|e| format!("书目摘要格式不正确（{}）：{}", path.display(), e))
}

pub fn read_book_at(root: &Path, book_id: &str) -> Result<ReadingBook, String> {
    assert_store_root(root)?;
    let path = book_path(root, book_id)?;
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取书籍失败（{}）：{}", path.display(), e))?;
    let mut book: ReadingBook = serde_json::from_str(&raw)
        .map_err(|e| format!("书籍文件格式不正确（{}）：{}", path.display(), e))?;
    if book.book_id != book_id {
        return Err(format!("书籍文件编号不一致：{} != {}", book.book_id, book_id));
    }
    // 阅读进度单独落盘，滚动时不重写整本书的大 JSON。
    if let Ok(raw) = fs::read_to_string(progress_path(root, book_id)?) {
        if let Ok(progress) = serde_json::from_str::<ReadingProgress>(&raw) {
            book.progress = progress;
        }
    }
    Ok(book)
}

pub(crate) fn write_book(root: &Path, book: &ReadingBook) -> Result<(), String> {
    ensure_store_dirs(root)?;
    let path = book_path(root, &book.book_id)?;
    let meta_target = meta_path(root, &book.book_id)?;
    let full = serde_json::to_vec_pretty(book).map_err(|e| format!("序列化书籍失败：{e}"))?;
    let meta = serde_json::to_vec_pretty(&book_meta(book)).map_err(|e| format!("序列化书目摘要失败：{e}"))?;
    if path.exists() {
        write_replace_atomically(&path, &full)?;
    } else {
        write_new_atomically(&path, &full)?;
    }
    write_replace_atomically(&meta_target, &meta)?;
    harden_private(&path, 0o600);
    harden_private(&meta_target, 0o600);
    Ok(())
}

/// 旧正文回填：按引文把批注移动到新章节中的同一段/同章位置。
///
/// 只调整 chapter_index / paragraph_index / start / end，不改批注结构与引文；
/// 找不到唯一匹配时保留原位置并计入 unresolved。
fn relocate_annotations(chapters: &[ReadingChapter], annotations: &mut [ReadingAnnotation]) -> usize {
    let mut unresolved = 0usize;
    for annotation in annotations.iter_mut() {
        let quote = annotation.quote.trim();
        if quote.is_empty() {
            continue;
        }
        let Some(chapter) = chapters.get(annotation.chapter_index) else {
            unresolved += 1;
            continue;
        };
        if let Some(text) = chapter.paragraphs.get(annotation.paragraph_index) {
            if let Ok((start, end, matched)) =
                resolve_span(text, annotation.start, annotation.end, quote)
            {
                annotation.start = start;
                annotation.end = end;
                annotation.quote = matched;
                continue;
            }
        }
        let mut found = None;
        for (paragraph_index, text) in chapter.paragraphs.iter().enumerate() {
            if let Ok((start, end, matched)) = resolve_span(text, 0, 0, quote) {
                found = Some((paragraph_index, start, end, matched));
                break;
            }
        }
        match found {
            Some((paragraph_index, start, end, matched)) => {
                annotation.paragraph_index = paragraph_index;
                annotation.start = start;
                annotation.end = end;
                annotation.quote = matched;
            }
            None => unresolved += 1,
        }
    }
    unresolved
}

fn mark_media_version(root: &Path, book_id: &str) {
    if let Ok(mut book) = read_book_at(root, book_id) {
        if book.media_version < 1 {
            book.media_version = 1;
            let _ = write_book(root, &book);
        }
    }
}

/// 重新抽取 EPUB/MOBI 正文，补齐旧版导入缺失的媒体标记与作者，并按引文重定位批注。
///
/// 任何失败都会把 media_version 置 1，避免每次打开旧书都重复抽取；失败原因原样返回。
pub fn refresh_content_at(root: &Path, book_id: &str) -> Result<ReadingRefreshResult, String> {
    let result = refresh_content_inner(root, book_id);
    if result.is_err() {
        mark_media_version(root, book_id);
    }
    result
}

fn refresh_content_inner(root: &Path, book_id: &str) -> Result<ReadingRefreshResult, String> {
    assert_store_root(root)?;
    let mut book = read_book_at(root, book_id)?;
    if book.source_format != "epub" && book.source_format != "mobi" {
        return Err("只有 EPUB / MOBI 书籍支持正文媒体回填。".to_string());
    }
    if book.source_path.trim().is_empty() {
        return Err("源文件路径为空，无法回填正文媒体。".to_string());
    }
    let source = read_reading_source(&book.source_path)
        .map_err(|e| format!("重新读取源文件失败：{e}"))?;
    let chapters = apply_chapters(&source);
    if chapters.is_empty() {
        return Err("源文件没有可读的章节正文，无法回填。".to_string());
    }
    let unresolved = relocate_annotations(&chapters, &mut book.annotations);
    book.chapters = chapters;
    if !source.author.trim().is_empty() {
        book.author = source.author.trim().to_string();
    }
    if let Some(cover) = source.cover.as_ref() {
        if let Some(file_name) = store_cover(root, book_id, cover) {
            book.cover = file_name;
        }
    }
    if !source.sha256.is_empty() {
        book.source_sha256 = source.sha256;
    }
    book.media_version = 1;
    write_book(root, &book)?;
    Ok(ReadingRefreshResult { book, unresolved })
}

pub fn set_progress_at(root: &Path, book_id: &str, percent: f64, now: f64) -> Result<ReadingBookMeta, String> {
    ensure_store_dirs(root)?;
    if !percent.is_finite() {
        return Err("阅读进度必须是有限数值。".to_string());
    }
    let percent = percent.clamp(0.0, 1.0);
    let at = utc_iso8601(now);
    let _guard = PROGRESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut progress = read_progress_at(root, book_id);
    progress.percent = percent;
    progress.updated_at = at.clone();
    let raw = serde_json::to_vec_pretty(&progress).map_err(|e| format!("序列化阅读进度失败：{e}"))?;
    write_replace_atomically(&progress_path(root, book_id)?, &raw)?;
    harden_private(&progress_path(root, book_id)?, 0o600);
    let mut meta = read_meta_at(root, book_id)?;
    meta.progress_percent = percent;
    meta.updated_at = at;
    let meta_raw = serde_json::to_vec_pretty(&meta).map_err(|e| format!("序列化书目摘要失败：{e}"))?;
    write_replace_atomically(&meta_path(root, book_id)?, &meta_raw)?;
    harden_private(&meta_path(root, book_id)?, 0o600);
    Ok(meta)
}

fn read_progress_at(root: &Path, book_id: &str) -> ReadingProgress {
    progress_path(root, book_id)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<ReadingProgress>(&raw).ok())
        .unwrap_or_default()
}

/// 单次最多累计一小时；秒数按 UTC 日期写入 progress 文件，不改正文与批注。
pub fn add_reading_time_at(root: &Path, book_id: &str, seconds: u64) -> Result<ReadingProgress, String> {
    let day = utc_iso8601(now_secs());
    add_reading_time_on(root, book_id, seconds, &day[..10])
}

/// 记录一次真实阅读时长。day 形如 2026-11-19，仅供内部测试注入日期。
fn add_reading_time_on(
    root: &Path,
    book_id: &str,
    seconds: u64,
    day: &str,
) -> Result<ReadingProgress, String> {
    ensure_store_dirs(root)?;
    if seconds > 3_600 {
        return Err("单次阅读计时最多 3600 秒。".to_string());
    }
    if seconds == 0 {
        return Ok(read_progress_at(root, book_id));
    }
    if day.len() != 10 || !day.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
        return Err(format!("阅读计时日期不合法：{day:?}"));
    }
    let _guard = PROGRESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut progress = read_progress_at(root, book_id);
    let entry = progress.daily_seconds.entry(day.to_string()).or_insert(0);
    *entry = entry.saturating_add(seconds);
    let path = progress_path(root, book_id)?;
    let raw = serde_json::to_vec_pretty(&progress).map_err(|e| format!("序列化阅读进度失败：{e}"))?;
    write_replace_atomically(&path, &raw)?;
    harden_private(&path, 0o600);
    Ok(progress)
}

pub fn add_annotation_at(
    root: &Path,
    book_id: &str,
    draft: ReadingAnnotationDraft,
    now: f64,
) -> Result<ReadingAnnotation, String> {
    let mut book = read_book_at(root, book_id)?;
    let kind = draft.kind.trim().to_lowercase();
    if !ANNOTATION_KINDS.contains(&kind.as_str()) {
        return Err(format!("批注类型不认识：{}", draft.kind));
    }
    let paragraph = paragraph_text(&book, draft.chapter_index, draft.paragraph_index)?.to_string();
    let body = draft.body.trim().to_string();
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(format!("想法超过 {MAX_BODY_CHARS} 字符上限。"));
    }
    if kind == KIND_THOUGHT && body.is_empty() {
        return Err("思考碎片不能为空。".to_string());
    }
    let mut start = 0usize;
    let mut end = 0usize;
    let mut quote = draft.quote.trim().to_string();
    if kind == KIND_HIGHLIGHT {
        let requested_start = draft.start.unwrap_or(0);
        let requested_end = draft.end.unwrap_or(0);
        let (s, e, q) = resolve_span(&paragraph, requested_start, requested_end, &quote)?;
        start = s;
        end = e;
        quote = q;
    } else if kind == KIND_THOUGHT && (!quote.is_empty() || draft.start.is_some() || draft.end.is_some()) {
        let requested_start = draft.start.unwrap_or(0);
        let requested_end = draft.end.unwrap_or(0);
        if quote.is_empty() && requested_end > requested_start {
            quote = paragraph
                .chars()
                .skip(requested_start)
                .take(requested_end.saturating_sub(requested_start))
                .collect();
        }
        if !quote.is_empty() {
            let (s, e, q) = resolve_span(&paragraph, requested_start, requested_end, &quote)?;
            start = s;
            end = e;
            quote = q;
        }
    } else if kind == KIND_BOOKMARK {
        if quote.is_empty() {
            quote = paragraph.chars().take(80).collect();
        }
    }
    if quote.chars().count() > MAX_QUOTE_CHARS {
        return Err(format!("引文超过 {MAX_QUOTE_CHARS} 字符上限。"));
    }
    let source = if draft.source.trim().to_lowercase() == SOURCE_AI {
        SOURCE_AI
    } else if draft.source.trim().is_empty() || SOURCE_KINDS.contains(&draft.source.trim().to_lowercase().as_str()) {
        SOURCE_USER
    } else {
        return Err(format!("批注来源不认识：{}", draft.source));
    }
    .to_string();
    let at = utc_iso8601(now);
    let annotation = ReadingAnnotation {
        id: new_id("an", now),
        kind,
        chapter_index: draft.chapter_index,
        paragraph_index: draft.paragraph_index,
        start,
        end,
        quote,
        body,
        color: normalize_color(&draft.color),
        source,
        created_at: at.clone(),
        updated_at: at,
    };
    book.annotations.push(annotation.clone());
    book.updated_at = annotation.created_at.clone();
    write_book(root, &book)?;
    Ok(annotation)
}

pub fn update_annotation_at(
    root: &Path,
    book_id: &str,
    annotation_id: &str,
    patch: ReadingAnnotationPatch,
    now: f64,
) -> Result<ReadingAnnotation, String> {
    if !is_safe_id(annotation_id) {
        return Err(format!("批注编号不合法：{annotation_id:?}"));
    }
    let mut book = read_book_at(root, book_id)?;
    let item = book
        .annotations
        .iter_mut()
        .find(|a| a.id == annotation_id)
        .ok_or_else(|| format!("没有这条批注：{annotation_id}"))?;
    if let Some(body) = patch.body {
        let body = body.trim().to_string();
        if body.chars().count() > MAX_BODY_CHARS {
            return Err(format!("想法超过 {MAX_BODY_CHARS} 字符上限。"));
        }
        if item.kind == KIND_THOUGHT && body.is_empty() {
            return Err("思考碎片不能为空。".to_string());
        }
        item.body = body;
    }
    if let Some(color) = patch.color {
        item.color = normalize_color(&color);
    }
    item.updated_at = utc_iso8601(now);
    let updated = item.clone();
    book.updated_at = updated.updated_at.clone();
    write_book(root, &book)?;
    Ok(updated)
}

pub fn delete_annotation_at(root: &Path, book_id: &str, annotation_id: &str, now: f64) -> Result<ReadingBookMeta, String> {
    if !is_safe_id(annotation_id) {
        return Err(format!("批注编号不合法：{annotation_id:?}"));
    }
    let mut book = read_book_at(root, book_id)?;
    let before = book.annotations.len();
    book.annotations.retain(|a| a.id != annotation_id);
    if book.annotations.len() == before {
        return Err(format!("没有这条批注：{annotation_id}"));
    }
    book.updated_at = utc_iso8601(now);
    write_book(root, &book)?;
    Ok(book_meta(&book))
}

const MAX_UNVERIFIED_CITATIONS: usize = 20;

/// 抽出整理稿里的 Markdown 引用行正文（去掉 > 前缀与首尾空白）。
fn organized_citation_lines(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix('>') else {
            continue;
        };
        let content = rest.trim_start_matches('>').trim();
        if !content.is_empty() {
            out.push(content.to_string());
        }
    }
    out
}

fn strip_wrapping_quote_pair(value: &str) -> Option<String> {
    let first = value.chars().next()?;
    let last = value.chars().last()?;
    if first == last {
        if !matches!(first, '"' | '\'') {
            return None;
        }
        if value.matches(first).count() != 2 {
            return None;
        }
    } else if !matches!(
        (first, last),
        ('“', '”') | ('‘', '’') | ('「', '」') | ('『', '』') | ('《', '》')
    ) {
        return None;
    }
    let start = first.len_utf8();
    let end = value.len().checked_sub(last.len_utf8())?;
    if start >= end {
        return None;
    }
    let inner = value[start..end].trim();
    if inner.is_empty() || inner == value {
        return None;
    }
    Some(inner.to_string())
}

/// 规范化引文：trim、连续空白压成一个空格、去成对引号包裹。
fn normalize_citation_text(raw: &str) -> String {
    let mut value: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    for _ in 0..3 {
        let Some(inner) = strip_wrapping_quote_pair(&value) else {
            break;
        };
        value = inner;
    }
    value.trim().to_string()
}

/// 逐条核对引用行是否能在本书批注的引文或批注所在段落原文中找到。
/// 返回（引用行总数，未核验行原文）。
fn audit_organized_citations(book: &ReadingBook, markdown: &str) -> (usize, Vec<String>) {
    let mut sources: Vec<String> = Vec::new();
    for annotation in &book.annotations {
        let quote = normalize_citation_text(&annotation.quote);
        if !quote.is_empty() {
            sources.push(quote);
        }
        if let Some(chapter) = book.chapters.get(annotation.chapter_index) {
            if let Some(paragraph) = chapter.paragraphs.get(annotation.paragraph_index) {
                let text = normalize_citation_text(paragraph);
                if !text.is_empty() {
                    sources.push(text);
                }
            }
        }
    }
    let lines = organized_citation_lines(markdown);
    let total = lines.len();
    let mut unverified: Vec<String> = Vec::new();
    for line in lines {
        let normalized = normalize_citation_text(&line);
        if normalized.is_empty() {
            continue;
        }
        let found = sources
            .iter()
            .any(|source| source == &normalized || source.contains(&normalized));
        if !found {
            unverified.push(line);
        }
    }
    (total, unverified)
}

fn cap_unverified_citations(mut items: Vec<String>) -> Vec<String> {
    if items.len() <= MAX_UNVERIFIED_CITATIONS {
        return items;
    }
    let total = items.len();
    items.truncate(MAX_UNVERIFIED_CITATIONS - 1);
    items.push(format!("……另有 {} 条", total - (MAX_UNVERIFIED_CITATIONS - 1)));
    items
}

/// 确定性引文核验：提取整理稿中的引用行，与批注引文及所在段落比对。
/// 未核验列表最多 20 条，超出部分合并为一条「……另有 N 条」。
pub fn verify_organized_markdown(book: &ReadingBook, markdown: &str) -> Vec<String> {
    cap_unverified_citations(audit_organized_citations(book, markdown).1)
}

fn has_frontmatter(markdown: &str) -> bool {
    markdown
        .lines()
        .next()
        .map(|line| line.trim() == "---")
        .unwrap_or(false)
}

/// 返回 frontmatter 结束（闭合的 --- 行）之后的字节偏移。
fn frontmatter_end(markdown: &str) -> Option<usize> {
    let mut segments = markdown.split_inclusive('\n');
    let first = segments.next()?;
    if first.trim() != "---" {
        return None;
    }
    let mut offset = first.len();
    for segment in segments {
        offset += segment.len();
        if segment.trim() == "---" {
            return Some(offset);
        }
    }
    None
}

fn yaml_quote(value: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => {
                out.push('\\');
                out.push('\\');
            }
            '"' => {
                out.push('\\');
                out.push('"');
            }
            '\n' => {
                out.push('\\');
                out.push('n');
            }
            '\r' => {
                out.push('\\');
                out.push('r');
            }
            '\t' => {
                out.push('\\');
                out.push('t');
            }
            c if c.is_control() => {
                out.push('\\');
                out.push('u');
                out.push_str(&format!("{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn build_frontmatter(title: &str, created_at: &str, book: &ReadingBook, annotation_count: usize) -> String {
    [
        "---".to_string(),
        format!("title: {}", yaml_quote(title)),
        format!("created: {}", yaml_quote(created_at)),
        format!("book: {}", yaml_quote(&book.title)),
        format!("source: {}", yaml_quote(&book.source_name)),
        format!("annotation_count: {annotation_count}"),
        "---".to_string(),
    ]
    .join("\n")
}

/// 提示块放在 frontmatter 之后（frontmatter 必须保持文件首行）；无 frontmatter 时放文件顶部。
fn insert_citation_warning(content: &str, warning: &str) -> String {
    if let Some(end) = frontmatter_end(content) {
        let head = content[..end].trim_end();
        let tail = content[end..].trim_start_matches(|c: char| c == '\r' || c == '\n');
        if tail.is_empty() {
            format!("{head}\n\n{warning}")
        } else {
            format!("{head}\n\n{warning}\n\n{tail}")
        }
    } else {
        format!("{warning}\n\n{content}")
    }
}

pub fn save_note_at(
    root: &Path,
    book_id: &str,
    title: &str,
    markdown: &str,
    annotation_ids: Vec<String>,
    now: f64,
) -> Result<ReadingSavedNote, String> {
    ensure_store_dirs(root)?;
    let mut book = read_book_at(root, book_id)?;
    let markdown = markdown.trim();
    if markdown.is_empty() {
        return Err("整理稿不能为空。".to_string());
    }
    if markdown.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("整理稿超过 {MAX_NOTE_CHARS} 字符上限。"));
    }
    let note_id = new_id("nt", now);
    let note_title = if title.trim().is_empty() { "AI 整理稿" } else { title.trim() };
    let created_at = utc_iso8601(now);
    let annotation_count = annotation_ids.len();
    let (citation_total, unverified_all) = audit_organized_citations(&book, markdown);
    let unverified_total = unverified_all.len();
    let verified_citations = citation_total.saturating_sub(unverified_total);
    let mut content = String::from(markdown);
    if !has_frontmatter(markdown) {
        let frontmatter = build_frontmatter(note_title, &created_at, &book, annotation_count);
        content = format!("{frontmatter}\n\n{markdown}");
    }
    if unverified_total > 0 {
        let warning = format!(
            "> ⚠ 引文核验：{unverified_total} 条未能在本书批注或原文中找到；本稿未通过核验，请人工复核。"
        );
        content = insert_citation_warning(&content, &warning);
    }
    let dir = root.join("notes").join(book_id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建整理稿目录失败：{e}"))?;
    let target = dir.join(format!("{note_id}.md"));
    let bytes = content.as_bytes();
    let byte_len = bytes.len() as u64;
    write_new_atomically(&target, bytes)?;
    harden_private(&target, 0o600);
    let relative_path = format!("notes/{}/{}.md", book_id, note_id);
    let note = ReadingSavedNote {
        note_id,
        title: note_title.to_string(),
        created_at,
        relative_path,
        bytes: byte_len,
        annotation_ids,
        verified_citations,
        unverified_citations: cap_unverified_citations(unverified_all),
    };
    book.notes.push(note.clone());
    book.updated_at = note.created_at.clone();
    write_book(root, &book)?;
    Ok(note)
}

pub fn read_note_at(root: &Path, book_id: &str, note_id: &str) -> Result<String, String> {
    if !is_safe_id(book_id) || !is_safe_id(note_id) {
        return Err("整理稿路径参数不合法。".to_string());
    }
    let path = root.join("notes").join(book_id).join(format!("{note_id}.md"));
    fs::read_to_string(&path).map_err(|e| format!("读取整理稿失败（{}）：{}", path.display(), e))
}

pub fn export_notes_at(root: &Path, book_id: &str, now: f64) -> Result<ReadingNoteFile, String> {
    ensure_store_dirs(root)?;
    let book = read_book_at(root, book_id)?;
    let markdown = render_book_markdown(&book);
    let stamp = utc_iso8601(now)
        .replace('-', "")
        .replace(':', "")
        .replace('T', "-")
        .replace('Z', "");
    let stem = safe_file_stem(&book.title, "book");
    let dir = root.join("exports");
    fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败：{e}"))?;
    let file_name = format!("{stem}-读书笔记-{stamp}.md");
    let relative_path = format!("exports/{file_name}");
    let target = dir.join(&file_name);
    let bytes = markdown.as_bytes();
    let byte_len = bytes.len() as u64;
    write_new_atomically(&target, bytes)?;
    harden_private(&target, 0o600);
    Ok(ReadingNoteFile {
        file_name,
        relative_path,
        content: markdown,
        bytes: byte_len,
    })
}

fn render_book_markdown(book: &ReadingBook) -> String {
    let mut lines = vec![
        format!("# 《{}》读书笔记", book.title),
        String::new(),
        format!("来源：{}（{}）", book.source_name, book.source_format),
        format!("导出：{}", utc_iso8601(now_secs())),
        String::new(),
    ];
    let mut by_chapter: Vec<(usize, Vec<&ReadingAnnotation>)> = Vec::new();
    for annotation in &book.annotations {
        if let Some((_, items)) = by_chapter.iter_mut().find(|(i, _)| *i == annotation.chapter_index) {
            items.push(annotation);
        } else {
            by_chapter.push((annotation.chapter_index, vec![annotation]));
        }
    }
    by_chapter.sort_by_key(|(i, _)| *i);
    for (chapter_index, items) in by_chapter {
        let title = book
            .chapters
            .get(chapter_index)
            .map(|c| c.title.clone())
            .unwrap_or_else(|| format!("第 {} 章", chapter_index + 1));
        lines.push(format!("## {title}"));
        lines.push(String::new());
        for item in items {
            if item.kind == KIND_HIGHLIGHT || (item.kind == KIND_THOUGHT && !item.quote.is_empty()) {
                for line in item.quote.lines() {
                    lines.push(format!("> {line}"));
                }
                lines.push(String::new());
            }
            if item.kind == KIND_BOOKMARK {
                lines.push(format!("- 标记：{}", item.quote));
            }
            if !item.body.is_empty() {
                let label = if item.source == SOURCE_AI { "AI 整理" } else if item.kind == KIND_THOUGHT { "想法" } else { "批注" };
                lines.push(format!("**{label}**：{}", item.body));
            }
            lines.push(String::new());
        }
    }
    if book.notes.is_empty() {
        lines.push("> 尚未保存 AI 整理稿。".to_string());
    } else {
        lines.push("## 已保存的整理稿".to_string());
        lines.push(String::new());
        for note in &book.notes {
            lines.push(format!("- {} · {}（{} 字节）", note.title, note.relative_path, note.bytes));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omnidesk-reading-{}-{}-{}",
            tag,
            std::process::id(),
            ID_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn source(content: &str) -> ReadingSource {
        ReadingSource {
            path: "/tmp/book.md".to_string(),
            file_name: "book.md".to_string(),
            bytes: content.len() as u64,
            extension: "md".to_string(),
            sha256: crate::sha256::sha256_hex(content.as_bytes()),
            author: String::new(),
            content: content.to_string(),
            chapters: Vec::new(),
            cover: None,
        }
    }

    #[test]
    fn imports_chapters_and_deduplicates_by_hash() {
        let root = temp_root("import");
        fs::create_dir_all(&root).unwrap();
        let content = "# 第一章\n\n第一段。\n\n第二段。\n\n# 第二章\n\n另一章。";
        let first = import_book_at(&root, source(content), 1_760_000_000.0).unwrap();
        assert!(!first.existing);
        assert_eq!(first.book.chapter_count, 2);
        assert_eq!(first.book.paragraph_count, 3);
        let second = import_book_at(&root, source(content), 1_760_000_100.0).unwrap();
        assert!(second.existing);
        assert_eq!(first.book.book_id, second.book.book_id);
        assert_eq!(first.book.media_version, 1);
    }

    #[test]
    fn renames_book_in_body_and_meta_but_rejects_rss_and_bad_titles() {
        let root = temp_root("rename");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文。"), 1_760_000_000.0).unwrap();
        let book_id = imported.book.book_id.clone();
        let renamed = rename_book_at(&root, &book_id, "  新书名  ").unwrap();
        assert_eq!(renamed.title, "新书名");
        assert_eq!(renamed.chapter_count, imported.book.chapter_count);
        assert_eq!(renamed.annotation_count, imported.book.annotation_count);
        let book = read_book_at(&root, &book_id).unwrap();
        assert_eq!(book.title, "新书名");
        assert_eq!(book.chapters.len(), imported.book.chapter_count);
        assert!(rename_book_at(&root, &book_id, "   ").is_err());
        let long = "字".repeat(301);
        assert!(rename_book_at(&root, &book_id, &long).is_err());
        let _ = fs::remove_dir_all(root);
    }

    fn day_key(secs: f64) -> String {
        utc_iso8601(secs)[..10].to_string()
    }

    #[test]
    fn accumulates_reading_time_month_minutes_and_streak() {
        let root = temp_root("time");
        fs::create_dir_all(&root).unwrap();
        let now = 1_760_000_000.0;
        let imported = import_book_at(&root, source("# 章\n\n正文。"), now).unwrap();
        let book_id = imported.book.book_id.clone();
        let today = day_key(now);
        let yesterday = day_key(now - 86_400.0);
        add_reading_time_on(&root, &book_id, 15, &today).unwrap();
        add_reading_time_on(&root, &book_id, 45, &today).unwrap();
        add_reading_time_on(&root, &book_id, 3_600, &yesterday).unwrap();
        assert!(add_reading_time_on(&root, &book_id, 3_601, &today).is_err());

        let progress = read_progress_at(&root, &book_id);
        assert_eq!(progress.daily_seconds.get(&today), Some(&60));
        assert_eq!(progress.daily_seconds.get(&yesterday), Some(&3_600));

        set_progress_at(&root, &book_id, 0.42, now + 10.0).unwrap();
        let kept = read_progress_at(&root, &book_id);
        assert_eq!(kept.daily_seconds.get(&today), Some(&60), "写进度不得丢掉 daily_seconds");

        let stats = reading_stats_at_now(&root, now + 20.0).unwrap();
        assert_eq!(stats.month_minutes, 61);
        assert_eq!(stats.streak_days, 2);
    }

    #[test]
    fn streak_starts_yesterday_when_today_has_no_reading() {
        let root = temp_root("streak");
        fs::create_dir_all(&root).unwrap();
        let now = 1_760_000_000.0;
        let imported = import_book_at(&root, source("# 章\n\n正文。"), now).unwrap();
        let book_id = imported.book.book_id.clone();
        let yesterday = day_key(now - 86_400.0);
        let before = day_key(now - 172_800.0);
        add_reading_time_on(&root, &book_id, 60, &yesterday).unwrap();
        add_reading_time_on(&root, &book_id, 60, &before).unwrap();
        let stats = reading_stats_at_now(&root, now).unwrap();
        assert_eq!(stats.streak_days, 2);
    }

    #[test]
    fn relocates_annotations_by_quote_within_chapter() {
        let mut annotation = ReadingAnnotation {
            id: "a1".to_string(),
            kind: KIND_HIGHLIGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: 0,
            end: 8,
            quote: "一段引文".to_string(),
            body: String::new(),
            color: "amber".to_string(),
            source: SOURCE_USER.to_string(),
            created_at: "2026-11-19T00:00:00Z".to_string(),
            updated_at: "2026-11-19T00:00:00Z".to_string(),
        };
        let chapters = vec![ReadingChapter {
            title: "第一章".to_string(),
            paragraphs: vec!["新增的前言".to_string(), "这里有一段引文出现。".to_string()],
        }];
        let unresolved = relocate_annotations(&chapters, std::slice::from_mut(&mut annotation));
        assert_eq!(unresolved, 0);
        assert_eq!(annotation.paragraph_index, 1);
        assert_eq!(annotation.start, 3);
        assert_eq!(annotation.end, 7);

        annotation.quote = "不存在的引文".to_string();
        let unresolved = relocate_annotations(&chapters, std::slice::from_mut(&mut annotation));
        assert_eq!(unresolved, 1);
        assert_eq!(annotation.paragraph_index, 1, "找不到时保留原位置");
    }

    #[test]
    fn failed_media_refresh_marks_version_to_avoid_repeats() {
        let root = temp_root("refresh-fail");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文。"), 1_760_000_000.0).unwrap();
        let book_id = imported.book.book_id.clone();
        let mut book = read_book_at(&root, &book_id).unwrap();
        book.source_format = "mobi".to_string();
        book.source_path = root.join("missing.mobi").display().to_string();
        book.media_version = 0;
        write_book(&root, &book).unwrap();
        assert!(refresh_content_at(&root, &book_id).is_err());
        assert_eq!(read_book_at(&root, &book_id).unwrap().media_version, 1);
        assert_eq!(read_meta_at(&root, &book_id).unwrap().media_version, 1);
    }

    #[test]
    fn anchors_annotations_with_code_point_offsets() {
        let root = temp_root("anchor");
        fs::create_dir_all(&root).unwrap();
        let content = "# 章\n\n😀 开头，后面是概念。";
        let imported = import_book_at(&root, source(content), 1_760_000_000.0).unwrap();
        let draft = ReadingAnnotationDraft {
            kind: KIND_HIGHLIGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: Some(3),
            end: Some(9),
            quote: "后面是概念".to_string(),
            body: String::new(),
            color: "blue".to_string(),
            source: "user".to_string(),
        };
        let annotation = add_annotation_at(&root, &imported.book.book_id, draft, 1_760_000_010.0).unwrap();
        let book = read_book_at(&root, &imported.book.book_id).unwrap();
        let paragraph = &book.chapters[0].paragraphs[0];
        let slice: String = paragraph.chars().skip(annotation.start).take(annotation.end - annotation.start).collect();
        assert_eq!(slice, "后面是概念");
        assert_eq!(annotation.quote, "后面是概念");
    }

    #[test]
    fn mismatched_quote_is_relocated_or_rejected() {
        let root = temp_root("quote");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\nA 概念 B"), 1_760_000_000.0).unwrap();
        let mut draft = ReadingAnnotationDraft {
            kind: KIND_HIGHLIGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: Some(0),
            end: Some(2),
            quote: "概念".to_string(),
            body: String::new(),
            color: String::new(),
            source: String::new(),
        };
        let annotation = add_annotation_at(&root, &imported.book.book_id, draft.clone(), 1_760_000_010.0).unwrap();
        assert_eq!(annotation.start, 2);
        assert_eq!(annotation.end, 4);
        draft.quote = "不存在".to_string();
        assert!(add_annotation_at(&root, &imported.book.book_id, draft, 1_760_000_020.0).is_err());
    }

    #[test]
    fn bookmarks_thoughts_update_delete_progress_and_export() {
        let root = temp_root("mutation");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文一。\n\n正文二。"), 1_760_000_000.0).unwrap();
        let id = imported.book.book_id.clone();
        let bookmark = add_annotation_at(
            &root,
            &id,
            ReadingAnnotationDraft {
                kind: KIND_BOOKMARK.to_string(),
                chapter_index: 0,
                paragraph_index: 0,
                ..ReadingAnnotationDraft::default()
            },
            1_760_000_010.0,
        )
        .unwrap();
        let thought = add_annotation_at(
            &root,
            &id,
            ReadingAnnotationDraft {
                kind: KIND_THOUGHT.to_string(),
                chapter_index: 0,
                paragraph_index: 1,
                body: "这里需要再查概念。".to_string(),
                ..ReadingAnnotationDraft::default()
            },
            1_760_000_020.0,
        )
        .unwrap();
        let updated = update_annotation_at(
            &root,
            &id,
            &thought.id,
            ReadingAnnotationPatch { body: Some("已查：概念指……".to_string()), color: Some("green".to_string()) },
            1_760_000_030.0,
        )
        .unwrap();
        assert_eq!(updated.body, "已查：概念指……");
        set_progress_at(&root, &id, 0.42, 1_760_000_040.0).unwrap();
        assert!((read_book_at(&root, &id).unwrap().progress.percent - 0.42).abs() < 1e-9);
        let note = save_note_at(&root, &id, "整理", "# 整理稿\n\n- 主题", vec![], 1_760_000_050.0).unwrap();
        let saved_note = read_note_at(&root, &id, &note.note_id).unwrap();
        assert!(saved_note.starts_with("---\ntitle: \"整理\"\n"));
        assert!(saved_note.contains("# 整理稿\n\n- 主题"));
        assert_eq!(note.verified_citations, 0);
        assert!(note.unverified_citations.is_empty());
        delete_annotation_at(&root, &id, &bookmark.id, 1_760_000_060.0).unwrap();
        let exported = export_notes_at(&root, &id, 1_760_000_070.0).unwrap();
        assert!(exported.content.contains("已查：概念指……"));
        assert!(exported.content.contains("已保存的整理稿"));
    }

    fn seed_highlight_book(tag: &str, paragraph: &str, quote: &str) -> (PathBuf, ReadingBook) {
        let root = temp_root(tag);
        fs::create_dir_all(&root).unwrap();
        let content = format!("# 章\n\n{paragraph}");
        let imported = import_book_at(&root, source(&content), 1_760_000_000.0).unwrap();
        let id = imported.book.book_id.clone();
        let start = paragraph
            .find(quote)
            .map(|byte| paragraph[..byte].chars().count())
            .unwrap_or(0);
        let draft = ReadingAnnotationDraft {
            kind: KIND_HIGHLIGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: Some(start),
            end: Some(start + quote.chars().count()),
            quote: quote.to_string(),
            ..ReadingAnnotationDraft::default()
        };
        add_annotation_at(&root, &id, draft, 1_760_000_010.0).unwrap();
        let book = read_book_at(&root, &id).unwrap();
        (root, book)
    }

    #[test]
    fn verifies_citations_against_annotations_and_paragraphs() {
        let (_root, book) = seed_highlight_book("verify-all", "A 概念 B 与 C。", "概念");
        let markdown = "## 值得回看的原句\n\n> 概念\n> 「A 概念 B 与 C。」\n\n> 与 C";
        assert!(verify_organized_markdown(&book, markdown).is_empty());
    }

    #[test]
    fn flags_citations_missing_from_book() {
        let (_root, book) = seed_highlight_book("verify-miss", "A 概念 B", "概念");
        let markdown = "> 概念\n> 不存在的引文";
        let unverified = verify_organized_markdown(&book, markdown);
        assert_eq!(unverified, vec!["不存在的引文".to_string()]);
        let (total, all) = audit_organized_citations(&book, markdown);
        assert_eq!(total, 2);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn normalizes_chinese_whitespace_and_quotes() {
        let (_root, book) = seed_highlight_book("verify-space", "中文　引文 与 空格", "中文　引文");
        assert!(verify_organized_markdown(&book, ">  「中文    引文」").is_empty());
        assert_eq!(verify_organized_markdown(&book, "> 「中文引文」").len(), 1);
    }

    #[test]
    fn reports_no_citations_and_no_annotations() {
        let root = temp_root("verify-none");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文一。"), 1_760_000_000.0).unwrap();
        let book = read_book_at(&root, &imported.book.book_id).unwrap();
        assert!(verify_organized_markdown(&book, "## 概览\n\n没有引用行。").is_empty());
        assert_eq!(
            verify_organized_markdown(&book, "> 没有批注的一句"),
            vec!["没有批注的一句".to_string()]
        );
    }

    #[test]
    fn caps_unverified_list_at_twenty() {
        let root = temp_root("verify-cap");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文一。"), 1_760_000_000.0).unwrap();
        let book = read_book_at(&root, &imported.book.book_id).unwrap();
        let markdown: String = (0..25).map(|i| format!("> 编造引文 {i}\n")).collect();
        let unverified = verify_organized_markdown(&book, &markdown);
        assert_eq!(unverified.len(), 20);
        assert_eq!(unverified[19], "……另有 6 条");
    }

    #[test]
    fn save_note_adds_frontmatter_when_missing_and_keeps_existing() {
        let root = temp_root("frontmatter");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文。"), 1_760_000_000.0).unwrap();
        let id = imported.book.book_id.clone();
        let note = save_note_at(
            &root,
            &id,
            "整理",
            "# 正文\n\n- 主题",
            vec!["an_1".to_string(), "an_2".to_string()],
            1_760_000_050.0,
        )
        .unwrap();
        let saved = read_note_at(&root, &id, &note.note_id).unwrap();
        assert!(saved.starts_with("---\ntitle: \"整理\"\n"));
        assert!(saved.contains("book: \"book\"\n"));
        assert!(saved.contains("source: \"book.md\"\n"));
        assert!(saved.contains("annotation_count: 2\n---\n\n# 正文\n\n- 主题"));

        let existing = "---\ntitle: 已有\n---\n\n# 正文\n\n- 主题";
        let second = save_note_at(&root, &id, "整理", existing, vec![], 1_760_000_060.0).unwrap();
        assert_eq!(read_note_at(&root, &id, &second.note_id).unwrap(), existing);
    }

    #[test]
    fn save_note_warns_about_unverified_citations_without_blocking() {
        let (root, book) = seed_highlight_book("warn", "A 概念 B", "概念");
        let note = save_note_at(
            &root,
            &book.book_id,
            "整理",
            "## 原句\n\n> 概念\n\n> 编造引文",
            vec![],
            1_760_000_050.0,
        )
        .unwrap();
        assert_eq!(note.verified_citations, 1);
        assert_eq!(note.unverified_citations, vec!["编造引文".to_string()]);
        let saved = read_note_at(&root, &book.book_id, &note.note_id).unwrap();
        assert!(saved.starts_with("---\n"));
        let warning_at = saved.find("⚠ 引文核验：1 条未能").unwrap();
        let frontmatter_close = saved.find("\n---\n").unwrap();
        assert!(warning_at > frontmatter_close);
    }

    #[test]
    fn rejects_unsafe_book_ids() {
        let root = temp_root("ids");
        fs::create_dir_all(&root).unwrap();
        assert!(read_book_at(&root, "../escape").is_err());
        assert!(read_book_at(&root, "bk/a").is_err());
    }

    #[test]
    fn summarizes_stats_with_thoughts_and_excludes_rss_from_book_count() {
        let root = temp_root("stats");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n一段正文。\n\n第二段。"), 1_760_000_000.0).unwrap();
        let book_id = imported.book.book_id.clone();
        let highlight = ReadingAnnotationDraft {
            kind: KIND_HIGHLIGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: Some(0),
            end: Some(5),
            quote: "一段正文。".to_string(),
            ..ReadingAnnotationDraft::default()
        };
        add_annotation_at(&root, &book_id, highlight, 1_760_000_010.0).unwrap();
        let thought = ReadingAnnotationDraft {
            kind: KIND_THOUGHT.to_string(),
            chapter_index: 0,
            paragraph_index: 1,
            start: Some(0),
            end: Some(4),
            quote: "第二段。".to_string(),
            body: "闪记内容".to_string(),
            ..ReadingAnnotationDraft::default()
        };
        add_annotation_at(&root, &book_id, thought, 1_760_000_020.0).unwrap();
        set_progress_at(&root, &book_id, 0.996, 1_760_000_030.0).unwrap();
        save_note_at(&root, &book_id, "整理", "# 整理\n\n没有引用", vec![], 1_760_000_040.0).unwrap();

        let mut rss_source = source("# RSS\n\n订阅正文。");
        rss_source.extension = "rss".to_string();
        rss_source.file_name = "rss.md".to_string();
        let rss = import_book_at(&root, rss_source, 1_760_000_050.0).unwrap();

        let stats = reading_stats_at(&root).unwrap();
        assert_eq!(stats.books, 1, "RSS 文章不计入书籍本数");
        assert_eq!(stats.finished, 1);
        assert_eq!(stats.chars, imported.book.char_count + rss.book.char_count);
        assert_eq!(stats.annotations, 2);
        assert_eq!(stats.notes, 1);
        assert_eq!(stats.thoughts, 1, "闪记口径 = thought 批注");
    }

    #[test]
    fn imports_opf_cover_into_meta_and_serves_payload() {
        let root = temp_root("cover");
        fs::create_dir_all(&root).unwrap();
        let mut sample = source("# 章\n\n正文。");
        sample.cover = Some(crate::task_shell::ReadingSourceCover {
            file_name: "cover.jpg".to_string(),
            mime: "image/jpeg".to_string(),
            bytes: vec![0xff, 0xd8, 0xff, 0x01, 0x02, 0x03],
        });
        let imported = import_book_at(&root, sample, 1_760_000_000.0).unwrap();
        let meta = read_meta_at(&root, &imported.book.book_id).unwrap();
        let cover = meta.cover.expect("应记录封面文件名");
        assert_eq!(cover, format!("{}.cover.jpg", imported.book.book_id));
        let path = root.join("meta").join(&cover);
        assert_eq!(fs::read(&path).unwrap(), vec![0xff, 0xd8, 0xff, 0x01, 0x02, 0x03]);
        let payload = read_cover_at(&root, &imported.book.book_id).unwrap();
        assert_eq!(payload.mime, "image/jpeg");
        assert_eq!(payload.bytes, 6);
        assert_eq!(crate::task_shell::base64_decode(&payload.data_base64).unwrap().len(), 6);
    }

    #[test]
    fn deletes_local_files_keeps_exports_and_rejects_rss() {
        let root = temp_root("delete");
        fs::create_dir_all(&root).unwrap();
        let imported = import_book_at(&root, source("# 章\n\n正文。"), 1_770_000_000.0).unwrap();
        let book_id = imported.book.book_id.clone();
        set_progress_at(&root, &book_id, 0.4, 1_770_000_010.0).unwrap();
        let notes_dir = root.join("notes").join(&book_id);
        fs::create_dir_all(&notes_dir).unwrap();
        fs::write(notes_dir.join("note.md"), "# 笔记").unwrap();
        let cover_name = format!("{book_id}.cover.png");
        fs::write(root.join("meta").join(&cover_name), b"\x89PNG\r\n").unwrap();
        fs::write(root.join("exports").join("keep.md"), "导出保留").unwrap();

        delete_book_at(&root, &book_id).unwrap();
        assert!(!root.join("books").join(format!("{book_id}.json")).exists());
        assert!(!root.join("meta").join(format!("{book_id}.json")).exists());
        assert!(!root.join("meta").join(&cover_name).exists());
        assert!(!root.join("progress").join(format!("{book_id}.json")).exists());
        assert!(!notes_dir.exists());
        assert!(root.join("exports").join("keep.md").exists(), "导出文件必须保留");

        let mut rss_source = source("# RSS\n\n订阅正文。");
        rss_source.extension = "rss".to_string();
        rss_source.file_name = "rss.md".to_string();
        let rss = import_book_at(&root, rss_source, 1_770_000_020.0).unwrap();
        let rss_id = rss.book.book_id.clone();
        assert!(delete_book_at(&root, &rss_id).is_err(), "订阅文章一律拒绝删除");
        assert!(root.join("books").join(format!("{rss_id}.json")).exists());

        assert!(delete_book_at(&root, "bk/a").is_err(), "不安全编号必须拒绝");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn end_to_end_epub_cover_and_media_under_env_root() {
        let dir = temp_root("epub_e2e");
        fs::create_dir_all(&dir).unwrap();
        let epub = dir.join("media.epub");
        let data_root = dir.join("reading-store");
        let script = r#"
import sys, zipfile
path = sys.argv[1]
with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("mimetype", "application/epub+zip")
    z.writestr("META-INF/container.xml", '<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>')
    z.writestr("OEBPS/content.opf", '<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:creator>合成作者</dc:creator></metadata><manifest><item id="c1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/><item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/><item id="pic" href="images/pic.png" media-type="image/png"/></manifest><spine><itemref idref="c1"/></spine></package>')
    z.writestr("OEBPS/text/ch1.xhtml", '<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><h1>第一章</h1><p>前文<img src="../images/pic.png"/>后文</p></body></html>')
    z.writestr("OEBPS/images/pic.png", b"\x89PNG\r\n")
    z.writestr("OEBPS/images/cover.jpg", b"\xff\xd8\xff\xe0cover")
"#;
        let built = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&epub)
            .output();
        let Ok(output) = built else {
            let _ = fs::remove_dir_all(&dir);
            return;
        };
        if !output.status.success() {
            let _ = fs::remove_dir_all(&dir);
            return;
        }
        let previous = std::env::var_os(READING_ROOT_ENV);
        std::env::set_var(READING_ROOT_ENV, &data_root);
        let result = (|| {
            let paths = AppPaths::resolve().map_err(|e| format!("路径解析失败：{e}"))?;
            let imported = import_book(&paths, epub.to_str().unwrap())?;
            assert_eq!(imported.book.author, "合成作者");
            let root = resolve_reading_root(&paths)?;
            let meta = read_meta_at(&root, &imported.book.book_id)?;
            let cover = meta.cover.ok_or_else(|| "没有封面".to_string())?;
            let cover_path = root.join("meta").join(&cover);
            assert!(cover_path.is_file(), "封面应落在 meta/<book_id>.cover.*");
            eprintln!(
                "OD-READ-003 e2e: book={} root={} cover={} cover_exists={}",
                imported.book.book_id,
                root.display(),
                cover,
                cover_path.is_file()
            );
            // 模拟旧书目：摘要没记封面、缓存文件也不在；读取时应从源 EPUB 回提取。
            fs::remove_file(&cover_path).map_err(|e| format!("移除封面失败：{e}"))?;
            let mut legacy = read_meta_at(&root, &imported.book.book_id)?;
            legacy.cover = None;
            let legacy_raw = serde_json::to_vec_pretty(&legacy)
                .map_err(|e| format!("序列化旧摘要失败：{e}"))?;
            fs::write(root.join("meta").join(format!("{}.json", imported.book.book_id)), legacy_raw)
                .map_err(|e| format!("写入旧摘要失败：{e}"))?;
            let recovered = read_cover_at(&root, &imported.book.book_id)?;
            assert_eq!(recovered.mime, "image/jpeg");
            let rewritten = read_meta_at(&root, &imported.book.book_id)?;
            assert!(rewritten.cover.is_some(), "回提取后应写回 meta.cover");
            let book = read_book_at(&root, &imported.book.book_id)?;
            assert!(book.chapters[0].paragraphs.iter().any(|p| p.contains("![img](odmedia:OEBPS/images/pic.png)")));
            let payload = crate::task_shell::read_reading_media(&book.source_path, "OEBPS/images/pic.png")?;
            eprintln!("OD-READ-003 e2e: media mime={} bytes={}", payload.mime, payload.bytes);
            assert_eq!(payload.mime, "image/png");
            assert_eq!(payload.bytes, 6);

            // 模拟媒体提取上线前的旧书目：正文无标记、media_version=0；打开时回填。
            let mut legacy_book = read_book_at(&root, &imported.book.book_id)?;
            legacy_book.chapters = vec![ReadingChapter {
                title: "旧正文".to_string(),
                paragraphs: vec!["前文后文".to_string()],
            }];
            legacy_book.author = String::new();
            legacy_book.media_version = 0;
            write_book(&root, &legacy_book)?;
            let draft = ReadingAnnotationDraft {
                kind: KIND_HIGHLIGHT.to_string(),
                chapter_index: 0,
                paragraph_index: 0,
                start: Some(0),
                end: Some(2),
                quote: "前文".to_string(),
                ..ReadingAnnotationDraft::default()
            };
            add_annotation_at(&root, &imported.book.book_id, draft, 1_760_000_020.0)?;
            let refreshed = refresh_content_at(&root, &imported.book.book_id)?;
            assert_eq!(refreshed.book.media_version, 1);
            assert_eq!(refreshed.book.author, "合成作者");
            assert_eq!(refreshed.unresolved, 0);
            assert!(refreshed.book.chapters[0].paragraphs.iter().any(|p| p.contains("![img](odmedia:OEBPS/images/pic.png)")));
            let annotation = refreshed.book.annotations.first().ok_or_else(|| "批注丢失".to_string())?;
            let moved = refreshed.book.chapters[0]
                .paragraphs
                .get(annotation.paragraph_index)
                .ok_or_else(|| "批注段落越界".to_string())?;
            assert_eq!(annotation.quote, "前文");
            assert!(moved.chars().skip(annotation.start).take(annotation.end - annotation.start).collect::<String>() == "前文");
            Ok::<(), String>(())
        })();
        match previous {
            Some(value) => std::env::set_var(READING_ROOT_ENV, value),
            None => std::env::remove_var(READING_ROOT_ENV),
        }
        result.unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
