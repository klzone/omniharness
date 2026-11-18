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
}

impl Default for ReadingProgress {
    fn default() -> Self {
        Self {
            percent: 0.0,
            updated_at: String::new(),
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
    pub progress_percent: f64,
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
        progress_percent: book.progress.percent,
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
    let imported_at = utc_iso8601(now);
    let book = ReadingBook {
        schema: READING_SCHEMA.to_string(),
        book_id: book_id.clone(),
        title,
        author: String::new(),
        source_name: source.file_name,
        source_format: source.extension,
        source_path: source.path,
        source_sha256: source.sha256,
        imported_at: imported_at.clone(),
        updated_at: imported_at,
        chapters,
        annotations: Vec::new(),
        notes: Vec::new(),
        progress: ReadingProgress::default(),
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

pub fn set_progress_at(root: &Path, book_id: &str, percent: f64, now: f64) -> Result<ReadingBookMeta, String> {
    ensure_store_dirs(root)?;
    if !percent.is_finite() {
        return Err("阅读进度必须是有限数值。".to_string());
    }
    let percent = percent.clamp(0.0, 1.0);
    let at = utc_iso8601(now);
    let progress = ReadingProgress {
        percent,
        updated_at: at.clone(),
    };
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
            content: content.to_string(),
            chapters: Vec::new(),
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
}
