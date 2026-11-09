//! 文稿导出引擎（OD-WT-613）。
//!
//! 架构契合：Markdown 导出是纯宿主函数；DOCX 优先走 office-docx /
//! python-docx 的确定性脚本，失败时退回 macOS textutil 并标注 simplified；
//! PDF 走 soffice headless，失败时退回 CUPS 打印流；公众号 HTML 由宿主
//! 确定性渲染。所有输入都来自本机文稿目录，导出过程不发起网络请求。
//!
//! 样式包读取 <应用数据根>/config/writing-export-styles/ 下的可编辑副本；
//! 文件不存在时使用编译期嵌入的默认样式，不自动写盘。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::document_store::{self, SnapshotSpec, VersionScope, VersionSource};
use crate::profile_rules::RulePolicy;
use crate::task_shell;
use crate::worklog_records::is_safe_id;
use crate::writing_outline;
use crate::writing_records::{self, CheckConclusion, CitationRecord, CitationSourceKind};
use crate::writing_section;
use crate::writing_store;

pub const FORMAT_MD: &str = "md";
pub const FORMAT_DOCX: &str = "docx";
pub const FORMAT_PDF: &str = "pdf";
pub const FORMAT_HTML: &str = "html";
pub const MODE_FORMAL: &str = "formal";
pub const MODE_REVIEW: &str = "review";
pub const MODE_WORKING: &str = "working";

pub const EXPORT_DIR: &str = "exports";
pub const STYLE_DIR: &str = "writing-export-styles";
pub const DEFAULT_GONGWEN_STYLE: &str = include_str!("../../../samples/workflows/export-styles/gongwen.md");
pub const DEFAULT_READING_STYLE: &str = include_str!("../../../samples/workflows/export-styles/reading.md");
pub const DEFAULT_GONGZHONGHAO_TEMPLATE: &str =
    include_str!("../../../samples/workflows/export-styles/gongzhonghao.html");
const DOCX_HELPER: &str = include_str!("../scripts/writing_docx_export.py");

const MAX_STYLE_BYTES: usize = 64 * 1024;
const MAX_TEMPLATE_BYTES: usize = 128 * 1024;
const MAX_TARGET_CHARS: usize = 2048;
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(90);
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const SOFFICE_TIMEOUT: Duration = Duration::from_secs(120);

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// writing_export 的入参。target_dir 缺省时使用应用数据根下的默认导出目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub writing_id: String,
    pub format: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub target_dir: Option<String>,
}

/// 一次导出的回执；路径与 manifest 的 export_receipts 同口径。
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    pub writing_id: String,
    pub format: String,
    pub mode: String,
    pub path: String,
    pub engine: String,
    pub simplified: bool,
    pub receipt: document_store::ExportReceipt,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocStyle {
    pub body_font: String,
    pub heading_font: String,
    pub body_size_pt: f64,
    pub line_height: f64,
    pub margin_cm: f64,
    #[serde(default)]
    pub max_width_cm: Option<f64>,
    #[serde(default = "default_heading_align")]
    pub heading_align: String,
}

fn default_heading_align() -> String {
    "left".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    pub kind: String,
    pub text: String,
}

/// 外部命令路径与超时。测试可以显式构造，不依赖进程环境。
#[derive(Debug, Clone)]
pub struct ExportTools {
    pub python_candidates: Vec<String>,
    pub textutil: Option<String>,
    pub soffice: Option<String>,
    pub cupsfilter: Option<String>,
    pub soffice_timeout: Duration,
    pub tool_timeout: Duration,
}

impl ExportTools {
    pub fn discover() -> Self {
        let mut python_candidates = Vec::new();
        for key in ["OMNIDESK_EXPORT_PYTHON", "OMNIDESK_PYTHON"] {
            if let Ok(value) = std::env::var(key) {
                let value = value.trim().to_string();
                if !value.is_empty() {
                    python_candidates.push(value);
                }
            }
        }
        for candidate in ["python3", "python"] {
            if find_on_path(candidate).is_some() && !python_candidates.iter().any(|v| v == candidate) {
                python_candidates.push(candidate.to_string());
            }
        }

        let textutil = std::env::var("OMNIDESK_TEXTUTIL")
            .ok()
            .filter(|v| Path::new(v).is_file())
            .or_else(|| {
                let path = PathBuf::from("/usr/bin/textutil");
                path.is_file().then(|| path.to_string_lossy().to_string())
            });
        let soffice = std::env::var("OMNIDESK_SOFFICE")
            .ok()
            .filter(|v| Path::new(v).is_file())
            .or_else(|| find_on_path("soffice"))
            .or_else(|| {
                [
                    "/Applications/LibreOffice.app/Contents/MacOS/soffice",
                    "/opt/homebrew/bin/soffice",
                    "/usr/local/bin/soffice",
                ]
                .iter()
                .find(|path| Path::new(path).is_file())
                .map(|path| path.to_string())
            });
        let cupsfilter = std::env::var("OMNIDESK_CUPSFILTER")
            .ok()
            .filter(|v| Path::new(v).is_file())
            .or_else(|| {
                let path = PathBuf::from("/usr/sbin/cupsfilter");
                path.is_file().then(|| path.to_string_lossy().to_string())
            });
        Self {
            python_candidates,
            textutil,
            soffice,
            cupsfilter,
            soffice_timeout: SOFFICE_TIMEOUT,
            tool_timeout: DEFAULT_TOOL_TIMEOUT,
        }
    }
}

fn find_on_path(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct.to_string_lossy().to_string());
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
struct ExportSource {
    title: String,
    markdown: String,
    citations: Vec<CitationRecord>,
    checks: Vec<writing_records::FactCheckRecord>,
}

#[derive(Debug, Clone)]
struct Rendered {
    bytes: Vec<u8>,
    engine: String,
    simplified: bool,
    notes: Vec<String>,
}

/// 导出入口。调用方已在 IPC 层完成 writing_id 闸门与目标目录初筛。
#[allow(clippy::too_many_arguments)]
pub fn export_writing(
    root: &Path,
    app_root: &Path,
    config_dir: &Path,
    request: &ExportRequest,
    policy: &RulePolicy,
    tools: &ExportTools,
    now: f64,
) -> Result<ExportOutcome, String> {
    let writing_id = request.writing_id.trim();
    if !is_safe_id(writing_id) {
        return Err(format!("写作任务编号不合法：{writing_id:?}"));
    }
    let format = normalize_format(&request.format)?;
    let mode = normalize_mode(format, request.mode.as_deref())?;

    let task = writing_store::read_writing_task(root, writing_id)?;
    if !matches!(
        task.status.as_str(),
        writing_store::STATUS_FINALIZED | writing_store::STATUS_ARCHIVED
    ) {
        return Err(format!(
            "只有「{}」或「{}」的文稿可以导出；当前状态为「{}」。",
            writing_store::STATUS_FINALIZED,
            writing_store::STATUS_ARCHIVED,
            task.status
        ));
    }

    let target_dir = resolve_target_dir(app_root, writing_id, request.target_dir.as_deref())?;
    let source = load_source(root, writing_id)?;
    let text = match (format, mode) {
        (FORMAT_MD, MODE_FORMAL) => strip_internal(&source.markdown),
        (FORMAT_MD, MODE_REVIEW) => build_review_markdown(&source.markdown, &source.citations),
        (FORMAT_MD, MODE_WORKING) => {
            let reviewed = build_review_markdown(&source.markdown, &source.citations);
            build_working_markdown(&reviewed, &source.checks)
        }
        _ => strip_internal(&source.markdown),
    };
    let scan_text = format!("{}\n{}", source.title, text);
    let hits = task_shell::red_line_scan_with(&scan_text, policy);
    if !hits.is_empty() {
        let total: u64 = hits.iter().map(|hit| hit.count).sum();
        return Err(format!(
            "导出前敏感词扫描命中 {total} 处，已拒绝导出（不回显命中原文）。"
        ));
    }

    let rendered = match format {
        FORMAT_MD => Rendered {
            bytes: text.as_bytes().to_vec(),
            engine: "host-markdown".to_string(),
            simplified: false,
            notes: Vec::new(),
        },
        FORMAT_HTML => {
            let template = load_html_template(config_dir)?;
            Rendered {
                bytes: render_wechat_html(&source.title, &text, &template).into_bytes(),
                engine: "host-html".to_string(),
                simplified: false,
                notes: Vec::new(),
            }
        }
        FORMAT_DOCX => {
            let style = load_doc_style(config_dir, StyleKind::Gongwen)?;
            convert_docx(&source.title, &text, &style, tools, false)?
        }
        FORMAT_PDF => convert_pdf(&source.title, &text, config_dir, tools)?,
        _ => unreachable!(),
    };

    let path = target_path(&target_dir, writing_id, format, now, &rendered.bytes)?;
    let path_text = path.to_string_lossy().to_string();
    let receipt_label = match (format, mode) {
        (FORMAT_MD, MODE_REVIEW) => "md-review",
        (FORMAT_MD, MODE_WORKING) => "md-working",
        (FORMAT_MD, _) => "md-formal",
        _ => format,
    };
    let (_manifest, receipt) = document_store::record_export_receipt(
        root,
        writing_id,
        receipt_label,
        &path_text,
        now,
    )
    .map_err(|error| {
        format!("导出文件已写入 {}，但 manifest 回执写入失败：{error}", path.display())
    })?;

    Ok(ExportOutcome {
        writing_id: writing_id.to_string(),
        format: format.to_string(),
        mode: mode.to_string(),
        path: path_text,
        engine: rendered.engine,
        simplified: rendered.simplified,
        receipt,
        notes: rendered.notes,
    })
}

fn normalize_format(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        FORMAT_MD => Ok(FORMAT_MD),
        FORMAT_DOCX => Ok(FORMAT_DOCX),
        FORMAT_PDF => Ok(FORMAT_PDF),
        FORMAT_HTML => Ok(FORMAT_HTML),
        other => Err(format!(
            "导出格式不认识：{other:?}；只支持 md / docx / pdf / html。"
        )),
    }
}

fn normalize_mode(format: &str, raw: Option<&str>) -> Result<&'static str, String> {
    let mode = raw.map(str::trim).filter(|v| !v.is_empty()).unwrap_or(MODE_FORMAL);
    let mode = match mode {
        MODE_FORMAL => MODE_FORMAL,
        MODE_REVIEW => MODE_REVIEW,
        MODE_WORKING => MODE_WORKING,
        other => {
            return Err(format!(
                "导出模式不认识：{other:?}；只支持 formal / review / working。"
            ))
        }
    };
    if format != FORMAT_MD && mode != MODE_FORMAL {
        return Err("只有 Markdown 支持正式稿 / 审阅稿 / 工作稿三态；其他格式固定为正式稿。"
            .to_string());
    }
    Ok(mode)
}

fn load_source(root: &Path, writing_id: &str) -> Result<ExportSource, String> {
    let confirmed = writing_outline::read_confirmed_outline(root, writing_id)?;
    let markdown = writing_section::compose_full_text_for(root, writing_id)?;
    let citations = writing_records::list_citations(root, writing_id)?;
    let checks = writing_records::list_fact_checks(root, writing_id)?;
    Ok(ExportSource {
        title: confirmed.outline.title.trim().to_string(),
        markdown,
        citations,
        checks,
    })
}

fn resolve_target_dir(
    app_root: &Path,
    writing_id: &str,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    match requested.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => {
            if raw.chars().count() > MAX_TARGET_CHARS {
                return Err(format!("导出目录过长（最多 {MAX_TARGET_CHARS} 字符）。"));
            }
            if raw.chars().any(char::is_control) {
                return Err("导出目录含控制字符。".to_string());
            }
            let candidate = PathBuf::from(raw);
            if !candidate.is_absolute() {
                return Err("导出目录必须是绝对路径；相对路径会落到进程当前目录。".to_string());
            }
            if candidate.components().any(|part| matches!(part, Component::ParentDir)) {
                return Err("导出目录不允许包含 .. 逃逸片段。".to_string());
            }
            let canonical = fs::canonicalize(&candidate)
                .map_err(|error| format!("导出目录不存在或不可访问（{}）：{error}", candidate.display()))?;
            if !canonical.is_dir() {
                return Err("导出目标不是目录。".to_string());
            }
            Ok(canonical)
        }
        None => {
            let dir = app_root.join(EXPORT_DIR).join("写作").join(writing_id);
            fs::create_dir_all(&dir)
                .map_err(|error| format!("创建默认导出目录失败（{}）：{error}", dir.display()))?;
            Ok(dir)
        }
    }
}

fn unique_file_name(
    writing_id: &str,
    now: f64,
    extension: &str,
    dir: &Path,
) -> PathBuf {
    let stamp = format!("{:.0}", now.max(0.0));
    for index in 0..1000 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("-{index}")
        };
        let name = format!("{writing_id}-{stamp}{suffix}.{extension}");
        let path = dir.join(&name);
        if !path.exists() {
            return path;
        }
    }
    dir.join(format!("{writing_id}-{stamp}-{}.{extension}", std::process::id()))
}

fn target_path(dir: &Path, writing_id: &str, format: &str, now: f64, bytes: &[u8]) -> Result<PathBuf, String> {
    let extension = match format {
        FORMAT_MD => "md",
        FORMAT_DOCX => "docx",
        FORMAT_PDF => "pdf",
        FORMAT_HTML => "html",
        _ => return Err("导出格式不认识。".to_string()),
    };
    let path = unique_file_name(writing_id, now, extension, dir);
    if !path.starts_with(dir) {
        return Err("导出文件名逃逸出目标目录，已拒绝。".to_string());
    }
    write_new_atomically(&path, bytes)?;
    Ok(path)
}

fn write_new_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::worklog_records::write_new_atomically(target, bytes)
}

// ---------------------------------------------------------------------------
// Markdown 三态
// ---------------------------------------------------------------------------

/// 正式稿剥离：frontmatter、HTML 注释、内部编号、来源标记与 AI 调试行。
pub fn strip_internal(text: &str) -> String {
    let mut value = text.replace("\r\n", "\n").replace('\r', "\n");
    if let Some(rest) = value.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            value = rest[end + 4..].trim_start_matches('\n').to_string();
        }
    }
    remove_between(&mut value, "<!--", "-->", None);
    remove_between(&mut value, "【", "】", Some("来源"));
    remove_between(&mut value, "〔", "〕", Some("来源"));
    remove_between(&mut value, "[", "]", Some("来源"));
    remove_between(&mut value, "[", "]", Some("AI"));
    remove_between(&mut value, "（", "）", Some("来源"));
    remove_between(&mut value, "(", ")", Some("来源"));

    let mut kept = Vec::new();
    for raw_line in value.lines() {
        let line = raw_line.trim_end();
        let probe = line.trim();
        if probe.starts_with("内部编号")
            || probe.starts_with("AI 调试")
            || probe.starts_with("AI调试")
            || probe.starts_with("Debug:")
            || probe.starts_with("debug:")
            || probe.starts_with("模型:")
        {
            continue;
        }
        let cleaned = strip_internal_tokens(line) + "\n";
        kept.push(cleaned);
    }
    let mut out = kept.concat();
    out = out
        .replace("（AI 生成）", "")
        .replace("（AI生成）", "")
        .replace("AI 草稿", "")
        .replace("AI草稿", "")
        .trim()
        .to_string();
    collapse_blank_lines(out)
}

fn remove_between(value: &mut String, open: &str, close: &str, required: Option<&str>) {
    let mut from = 0usize;
    loop {
        if from > value.len() {
            return;
        }
        let Some(rel_start) = value[from..].find(open) else { return };
        let start = from + rel_start;
        let Some(rel_end) = value[start + open.len()..].find(close) else { return };
        let end = start + open.len() + rel_end + close.len();
        let inner = &value[start..end];
        let should_remove = required.map(|word| inner.contains(word)).unwrap_or(true);
        if should_remove {
            value.replace_range(start..end, "");
            from = start;
        } else {
            from = start + open.len();
        }
    }
}

fn strip_internal_tokens(line: &str) -> String {
    const PREFIXES: &[&str] = &[
        "wt-", "m-", "k-", "doc-", "finding-", "fc-", "citation-", "record-", "version-",
        "material-", "knowledge-", "ai-",
    ];
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            let start = index;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '-' || chars[index] == '_')
            {
                index += 1;
            }
            let token: String = chars[start..index].iter().collect();
            let lower = token.to_ascii_lowercase();
            let internal = PREFIXES
                .iter()
                .any(|prefix| lower.starts_with(prefix) && token.len() > prefix.len());
            if !internal {
                out.push_str(&token);
            }
        } else {
            out.push(ch);
            index += 1;
        }
    }
    out.trim_end().to_string()
}

fn collapse_blank_lines(value: String) -> String {
    let mut out = String::new();
    let mut blank = false;
    for line in value.lines() {
        let empty = line.trim().is_empty();
        if empty {
            if !blank && !out.is_empty() {
                out.push('\n');
            }
            blank = true;
        } else {
            out.push_str(line.trim_end());
            out.push('\n');
            blank = false;
        }
    }
    out.trim().to_string()
}

fn source_label(kind: CitationSourceKind) -> &'static str {
    match kind {
        CitationSourceKind::Material => "素材",
        CitationSourceKind::Knowledge => "知识",
    }
}

fn citation_marker(citation: &CitationRecord) -> String {
    format!(
        "〔来源：{} {}〕",
        source_label(citation.source_kind),
        citation.source_id
    )
}

/// 审阅稿：在对应段落末尾加来源标记，并在文末附来源明细。
pub fn build_review_markdown(markdown: &str, citations: &[CitationRecord]) -> String {
    let mut lines: Vec<String> = markdown.lines().map(|v| v.to_string()).collect();
    let mut section_no = 0u32;
    let mut paragraph_no = 0u32;
    let mut in_paragraph = false;
    let mut seen: Vec<(u32, u32)> = Vec::new();
    for line in lines.iter_mut() {
        let probe = line.trim();
        if let Some(rest) = probe.strip_prefix("## ") {
            section_no = rest
                .split_once(". ")
                .and_then(|(no, _)| no.trim().parse::<u32>().ok())
                .unwrap_or(section_no);
            paragraph_no = 0;
            in_paragraph = false;
            continue;
        }
        if probe.starts_with("# ") {
            in_paragraph = false;
            continue;
        }
        if probe.is_empty() {
            in_paragraph = false;
            continue;
        }
        if !in_paragraph {
            paragraph_no += 1;
            in_paragraph = true;
        }
        let matching: Vec<&CitationRecord> = citations
            .iter()
            .filter(|citation| citation.section_no == section_no && citation.paragraph_no == paragraph_no)
            .collect();
        if !matching.is_empty() {
            for citation in &matching {
                line.push_str(&citation_marker(citation));
                seen.push((citation.section_no, citation.paragraph_no));
            }
        }
    }
    let mut out = lines.join("\n");
    if !citations.is_empty() {
        out.push_str("\n\n## 来源标记\n\n");
        for citation in citations {
            let location = format!(
                "第 {} 章第 {} 段",
                citation.section_no, citation.paragraph_no
            );
            let quote = clip_text(&citation.quote, 80);
            out.push_str(&format!(
                "- {location} · {} {} · {quote}\n",
                source_label(citation.source_kind),
                citation.source_id
            ));
            if !seen.contains(&(citation.section_no, citation.paragraph_no)) {
                out.push_str("  （段落标记未匹配，仅列在来源明细）\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// 工作稿：审阅标记 + 核验结果摘要。
pub fn build_working_markdown(markdown: &str, checks: &[writing_records::FactCheckRecord]) -> String {
    let mut out = markdown.trim_end().to_string();
    out.push_str("\n\n## 核验结果摘要\n\n");
    if checks.is_empty() {
        out.push_str("本次导出前没有核验记录。\n");
        return out;
    }
    let unprocessed = checks
        .iter()
        .filter(|record| record.resolution == writing_records::ResolutionStatus::Unresolved)
        .count();
    out.push_str(&format!(
        "共 {} 条，待处理 {} 条。\n\n",
        checks.len(),
        unprocessed
    ));
    let mut counts: Vec<(CheckConclusion, usize)> = Vec::new();
    for record in checks {
        if let Some(entry) = counts.iter_mut().find(|(kind, _)| *kind == record.conclusion) {
            entry.1 += 1;
        } else {
            counts.push((record.conclusion, 1));
        }
    }
    for (kind, count) in counts {
        out.push_str(&format!("- {}：{} 条\n", conclusion_label(kind), count));
    }
    out.push('\n');
    for record in checks.iter().take(200) {
        let fact = record
            .fact_text
            .as_deref()
            .map(|value| clip_text(value, 80))
            .unwrap_or_else(|| "（未记录原文）".to_string());
        out.push_str(&format!(
            "- 第 {} 章第 {} 段 · {} · {} · {}\n",
            record.section_no,
            record.paragraph_no,
            conclusion_label(record.conclusion),
            resolution_label(record.resolution),
            fact
        ));
    }
    if checks.len() > 200 {
        out.push_str(&format!("- 其余 {} 条未在摘要展开\n", checks.len() - 200));
    }
    out.trim_end().to_string()
}

fn conclusion_label(value: CheckConclusion) -> &'static str {
    match value {
        CheckConclusion::DirectSource => "有直接来源支持",
        CheckConclusion::IndirectSource => "有间接来源支持",
        CheckConclusion::SourceConflict => "来源存在冲突",
        CheckConclusion::SourceOutdated => "来源已过期",
        CheckConclusion::SourceMissing => "无法找到来源",
        CheckConclusion::AnalysisJudgment => "属于分析判断",
        CheckConclusion::WorkPlan => "属于工作计划",
        CheckConclusion::ExpressiveSummary => "属于表达性概括",
    }
}

fn resolution_label(value: writing_records::ResolutionStatus) -> &'static str {
    match value {
        writing_records::ResolutionStatus::Unresolved => "未处理",
        writing_records::ResolutionStatus::Accepted => "已接受",
        writing_records::ResolutionStatus::Modified => "已修改",
        writing_records::ResolutionStatus::SourceRemoved => "已删除来源项",
    }
}

fn clip_text(value: &str, limit: usize) -> String {
    let cleaned = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() <= limit {
        cleaned
    } else {
        let mut out: String = chars[..limit].iter().collect();
        out.push('…');
        out
    }
}

// ---------------------------------------------------------------------------
// 样式包
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Gongwen,
    Reading,
}

impl StyleKind {
    pub fn file_name(self) -> &'static str {
        match self {
            StyleKind::Gongwen => "gongwen.md",
            StyleKind::Reading => "reading.md",
        }
    }

    fn default_text(self) -> &'static str {
        match self {
            StyleKind::Gongwen => DEFAULT_GONGWEN_STYLE,
            StyleKind::Reading => DEFAULT_READING_STYLE,
        }
    }
}

pub fn style_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(STYLE_DIR)
}

pub fn load_doc_style(config_dir: &Path, kind: StyleKind) -> Result<DocStyle, String> {
    let path = style_dir(config_dir).join(kind.file_name());
    let text = if path.exists() {
        let metadata = fs::metadata(&path).map_err(|e| format!("读取样式文件失败（{}）：{e}", path.display()))?;
        if metadata.len() as usize > MAX_STYLE_BYTES {
            return Err(format!("样式文件过大（最多 {MAX_STYLE_BYTES} 字节）：{}", path.display()));
        }
        fs::read_to_string(&path).map_err(|e| format!("读取样式文件失败（{}）：{e}", path.display()))?
    } else {
        kind.default_text().to_string()
    };
    parse_doc_style(&text, kind)
}

fn parse_doc_style(text: &str, kind: StyleKind) -> Result<DocStyle, String> {
    let mut values = std::collections::BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("<!--") {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        values.insert(key.trim().to_string(), value.trim().to_string());
    }
    let body_font = style_font(&values, "body_font")?;
    let heading_font = style_font(&values, "heading_font")?;
    let body_size_pt = style_number(&values, "body_size_pt", 8.0, 72.0)?;
    let line_height = style_number(&values, "line_height", 1.0, 3.0)?;
    let margin_cm = style_number(&values, "margin_cm", 0.5, 5.0)?;
    let max_width_cm = if kind == StyleKind::Reading {
        Some(style_number(&values, "max_width_cm", 6.0, 40.0)?)
    } else {
        values
            .get("max_width_cm")
            .map(|_| style_number(&values, "max_width_cm", 6.0, 40.0))
            .transpose()?
    };
    let heading_align = values
        .get("heading_align")
        .cloned()
        .unwrap_or_else(default_heading_align);
    if !matches!(heading_align.as_str(), "left" | "center" | "right" | "justify") {
        return Err(format!("heading_align 不认识：{heading_align:?}"));
    }
    Ok(DocStyle {
        body_font,
        heading_font,
        body_size_pt,
        line_height,
        margin_cm,
        max_width_cm,
        heading_align,
    })
}

fn style_font(values: &std::collections::BTreeMap<String, String>, key: &str) -> Result<String, String> {
    let value = values
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("样式缺少 {key}"))?;
    if value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(format!("样式字段 {key} 含控制字符或过长"));
    }
    Ok(value)
}

fn style_number(
    values: &std::collections::BTreeMap<String, String>,
    key: &str,
    min: f64,
    max: f64,
) -> Result<f64, String> {
    let raw = values.get(key).ok_or_else(|| format!("样式缺少 {key}"))?;
    let value: f64 = raw
        .parse()
        .map_err(|_| format!("样式字段 {key} 不是数字：{raw:?}"))?;
    if !value.is_finite() || value < min || value > max {
        return Err(format!("样式字段 {key} 超出范围 {min}..{max}：{value}"));
    }
    Ok(value)
}

pub fn load_html_template(config_dir: &Path) -> Result<String, String> {
    let path = style_dir(config_dir).join("gongzhonghao.html");
    let text = if path.exists() {
        let metadata = fs::metadata(&path).map_err(|e| format!("读取公众号模板失败（{}）：{e}", path.display()))?;
        if metadata.len() as usize > MAX_TEMPLATE_BYTES {
            return Err(format!("公众号模板过大（最多 {MAX_TEMPLATE_BYTES} 字节）：{}", path.display()));
        }
        fs::read_to_string(&path).map_err(|e| format!("读取公众号模板失败（{}）：{e}", path.display()))?
    } else {
        DEFAULT_GONGZHONGHAO_TEMPLATE.to_string()
    };
    validate_html_template(&text)?;
    Ok(text)
}

fn validate_html_template(text: &str) -> Result<(), String> {
    if !text.contains("{{content}}") {
        return Err("公众号模板缺少 {{content}} 占位符。".to_string());
    }
    let lower = text.to_ascii_lowercase();
    for marker in ["<script", "javascript:", "<iframe", "data:text/html"] {
        if lower.contains(marker) {
            return Err(format!("公众号模板含不允许的内容：{marker}"));
        }
    }
    if lower.contains("src=\"http")
        || lower.contains("src='http")
        || lower.contains("href=\"http")
        || lower.contains("href='http")
    {
        return Err("公众号模板不得引用远程资源。".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 公众号 HTML / DOCX HTML
// ---------------------------------------------------------------------------

pub fn parse_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    fn flush(paragraph: &mut Vec<String>, blocks: &mut Vec<Block>) {
        if paragraph.is_empty() {
            return;
        }
        blocks.push(Block {
            kind: "p".to_string(),
            text: paragraph.join("\n"),
        });
        paragraph.clear();
    }
    for raw in markdown.lines() {
        let line = raw.trim_end();
        let probe = line.trim();
        if probe.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if let Some(rest) = probe.strip_prefix("### ") {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block { kind: "h2".to_string(), text: rest.trim().to_string() });
        } else if let Some(rest) = probe.strip_prefix("## ") {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block { kind: "h2".to_string(), text: rest.trim().to_string() });
        } else if let Some(rest) = probe.strip_prefix("# ") {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block { kind: "h1".to_string(), text: rest.trim().to_string() });
        } else if let Some(rest) = probe.strip_prefix("> ") {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block { kind: "quote".to_string(), text: rest.trim().to_string() });
        } else if let Some(rest) = probe.strip_prefix("- ").or_else(|| probe.strip_prefix("* ")) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block { kind: "bullet".to_string(), text: rest.trim().to_string() });
        } else if is_ordered_item(probe) {
            flush(&mut paragraph, &mut blocks);
            let text = probe.split_once(". ").map(|(_, text)| text).unwrap_or(probe);
            blocks.push(Block { kind: "bullet".to_string(), text: text.trim().to_string() });
        } else {
            paragraph.push(line.trim().to_string());
        }
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

fn is_ordered_item(line: &str) -> bool {
    let Some((head, _)) = line.split_once(". ") else { return false };
    !head.is_empty() && head.chars().all(|c| c.is_ascii_digit())
}

pub fn render_wechat_html(title: &str, markdown: &str, template: &str) -> String {
    let blocks = parse_blocks(markdown);
    let mut content = String::new();
    for block in &blocks {
        match block.kind.as_str() {
            "h1" => content.push_str(&format!(
                "<h1 style=\"font-size:22px;line-height:1.45;font-weight:700;text-align:center;margin:0 0 24px;color:#111827;\">{}</h1>\n",
                render_inline_html(&block.text)
            )),
            "h2" => content.push_str(&format!(
                "<h2 style=\"font-size:18px;line-height:1.6;font-weight:700;margin:32px 0 14px;padding-left:10px;border-left:3px solid #2563eb;color:#111827;\">{}</h2>\n",
                render_inline_html(&block.text)
            )),
            "quote" => content.push_str(&format!(
                "<blockquote style=\"margin:18px 0;padding:12px 14px;background:#f6f8fa;border-left:3px solid #d0d7de;color:#57606a;font-size:15px;line-height:1.8;\">{}</blockquote>\n",
                render_inline_html(&block.text)
            )),
            "bullet" => content.push_str(&format!(
                "<p style=\"margin:0 0 8px;padding-left:1.2em;text-indent:-1.2em;font-size:16px;line-height:1.85;color:#374151;\">• {}</p>\n",
                render_inline_html(&block.text)
            )),
            _ => content.push_str(&format!(
                "<p style=\"margin:0 0 18px;font-size:16px;line-height:1.9;letter-spacing:0.02em;text-align:justify;color:#374151;\">{}</p>\n",
                render_inline_html(&block.text)
            )),
        }
    }
    template
        .replace("{{title}}", &escape_html(title))
        .replace("{{content}}", content.trim_end())
}

fn render_inline_html(text: &str) -> String {
    let escaped = escape_html(text);
    let mut out = String::new();
    let mut bold = false;
    let mut rest = escaped.as_str();
    while let Some(index) = rest.find("**") {
        out.push_str(&rest[..index]);
        if bold {
            out.push_str("</strong>");
        } else {
            out.push_str("<strong style=\"font-weight:600;color:#111827;\">");
        }
        bold = !bold;
        rest = &rest[index + 2..];
    }
    out.push_str(rest);
    if bold {
        out.push_str("</strong>");
    }
    out.replace('\n', "<br/>")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_docx_html(title: &str, markdown: &str, style: &DocStyle) -> String {
    let blocks = parse_blocks(markdown);
    let body_style = format!(
        "font-family:'{}';font-size:{}pt;line-height:{};margin:{}cm;",
        escape_html(&style.body_font),
        style.body_size_pt,
        style.line_height,
        style.margin_cm
    );
    let heading_style = format!(
        "font-family:'{}';font-weight:bold;text-align:{};",
        escape_html(&style.heading_font),
        style.heading_align
    );
    let mut out = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body style=\"{}\">\n",
        escape_html(title),
        body_style
    );
    for block in &blocks {
        match block.kind.as_str() {
            "h1" => out.push_str(&format!(
                "<h1 style=\"{}font-size:{}pt;margin:0 0 14pt;\">{}</h1>\n",
                heading_style,
                style.body_size_pt + 5.0,
                escape_html(&plain_text(&block.text))
            )),
            "h2" => out.push_str(&format!(
                "<h2 style=\"{}font-size:{}pt;text-align:left;margin:14pt 0 8pt;\">{}</h2>\n",
                heading_style,
                style.body_size_pt + 2.0,
                escape_html(&plain_text(&block.text))
            )),
            "quote" => out.push_str(&format!(
                "<blockquote style=\"font-family:'{}';font-size:{}pt;line-height:{};margin:8pt 1cm;\">{}</blockquote>\n",
                escape_html(&style.body_font),
                style.body_size_pt - 1.0,
                style.line_height,
                escape_html(&plain_text(&block.text)).replace('\n', "<br/>")
            )),
            "bullet" => out.push_str(&format!(
                "<p style=\"font-family:'{}';font-size:{}pt;line-height:{};margin:0 0 3pt;text-indent:-0.9cm;padding-left:0.9cm;\">• {}</p>\n",
                escape_html(&style.body_font),
                style.body_size_pt,
                style.line_height,
                escape_html(&plain_text(&block.text))
            )),
            _ => out.push_str(&format!(
                "<p style=\"font-family:'{}';font-size:{}pt;line-height:{};margin:0 0 6pt;text-indent:2em;text-align:justify;\">{}</p>\n",
                escape_html(&style.body_font),
                style.body_size_pt,
                style.line_height,
                escape_html(&plain_text(&block.text)).replace('\n', "<br/>")
            )),
        }
    }
    out.push_str("</body></html>\n");
    out
}

fn plain_text(value: &str) -> String {
    value.replace("**", "")
}

// ---------------------------------------------------------------------------
// DOCX / PDF
// ---------------------------------------------------------------------------

fn convert_docx(
    title: &str,
    markdown: &str,
    style: &DocStyle,
    tools: &ExportTools,
    reading: bool,
) -> Result<Rendered, String> {
    let temp = temp_dir("docx")?;
    let blocks = parse_blocks(markdown);
    let payload = serde_json::json!({
        "title": title,
        "blocks": blocks,
        "style": style,
    });
    let payload_path = temp.join("payload.json");
    fs::write(&payload_path, serde_json::to_vec(&payload).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写入 DOCX 负载失败：{e}"))?;
    let office_out = temp.join("office.docx");
    let helper_path = temp.join("writing_docx_export.py");
    fs::write(&helper_path, DOCX_HELPER).map_err(|e| format!("写入 DOCX 辅助脚本失败：{e}"))?;

    let mut notes = Vec::new();
    let office_python = tools
        .python_candidates
        .iter()
        .find(|python| python_has_docx(python));
    if let Some(python) = office_python {
        let args = vec![
            helper_path.to_string_lossy().to_string(),
            payload_path.to_string_lossy().to_string(),
            office_out.to_string_lossy().to_string(),
        ];
        let office_ok = run_process(python, &args, tools.tool_timeout, None, None)
            .map(|output| output.success)
            .unwrap_or(false);
        if office_ok
            && office_out.is_file()
            && office_out.metadata().map(|m| m.len() > 0).unwrap_or(false)
        {
            let bytes = fs::read(&office_out).map_err(|e| format!("读取 office-docx 产物失败：{e}"))?;
            let mut rendered = Rendered {
                bytes,
                engine: "office-docx".to_string(),
                simplified: false,
                notes,
            };
            if reading {
                rendered.notes.push("阅读版式".to_string());
            }
            let _ = fs::remove_dir_all(&temp);
            return Ok(rendered);
        }
        notes.push("office-docx 脚本未产出可用 DOCX，已转 textutil 简化版。".to_string());
    } else {
        notes.push("未找到可导入 python-docx 的解释器，使用 textutil 简化版。".to_string());
    }

    let Some(textutil) = tools.textutil.as_deref() else {
        let _ = fs::remove_dir_all(&temp);
        return Err(format!("{}\n且本机没有 textutil 兜底。", notes.join(" ")));
    };
    let html_path = temp.join("document.html");
    fs::write(&html_path, render_docx_html(title, markdown, style))
        .map_err(|e| format!("写入 DOCX HTML 中间件失败：{e}"))?;
    let textutil_out = temp.join("textutil.docx");
    let args = vec![
        "-convert".to_string(),
        "docx".to_string(),
        html_path.to_string_lossy().to_string(),
        "-output".to_string(),
        textutil_out.to_string_lossy().to_string(),
    ];
    let textutil_ok = run_process(textutil, &args, tools.tool_timeout, None, None)
        .map(|output| output.success)
        .unwrap_or(false);
    if !textutil_ok {
        let _ = fs::remove_dir_all(&temp);
        return Err("textutil 转换失败。".to_string());
    }
    if !textutil_out.is_file() || textutil_out.metadata().map(|m| m.len() == 0).unwrap_or(true) {
        let _ = fs::remove_dir_all(&temp);
        return Err("textutil 没有产出生效的 DOCX。".to_string());
    }
    let bytes = fs::read(&textutil_out).map_err(|e| format!("读取 textutil 产物失败：{e}"))?;
    notes.push("textutil 简化版：公文页边距与字体层级不一定与 Word 版完全一致。".to_string());
    let _ = fs::remove_dir_all(&temp);
    Ok(Rendered {
        bytes,
        engine: "textutil".to_string(),
        simplified: true,
        notes,
    })
}

fn convert_pdf(
    title: &str,
    markdown: &str,
    config_dir: &Path,
    tools: &ExportTools,
) -> Result<Rendered, String> {
    let style = load_doc_style(config_dir, StyleKind::Reading)?;
    let docx = convert_docx(title, markdown, &style, tools, true)?;
    let mut notes = Vec::new();
    if docx.simplified {
        notes.push("阅读版中间 DOCX 为 textutil 简化版。".to_string());
    }
    let temp = temp_dir("pdf")?;
    let docx_path = temp.join("reading.docx");
    fs::write(&docx_path, &docx.bytes).map_err(|e| format!("写入 PDF 中间 DOCX 失败：{e}"))?;
    let out_dir = temp.join("pdf-out");
    fs::create_dir_all(&out_dir).map_err(|e| format!("创建 PDF 临时目录失败：{e}"))?;

    if let Some(soffice) = tools.soffice.as_deref() {
        let profile = temp.join("profile");
        let args = vec![
            "--headless".to_string(),
            "--norestore".to_string(),
            format!("-env:UserInstallation=file://{}", profile.display()),
            "--convert-to".to_string(),
            "pdf".to_string(),
            "--outdir".to_string(),
            out_dir.to_string_lossy().to_string(),
            docx_path.to_string_lossy().to_string(),
        ];
        match run_process(soffice, &args, tools.soffice_timeout, None, None) {
            Ok(_) => {
                let pdf_path = out_dir.join("reading.pdf");
                if pdf_path.is_file() && pdf_path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
                    let bytes = fs::read(&pdf_path).map_err(|e| format!("读取 soffice PDF 失败：{e}"))?;
                    if bytes.starts_with(b"%PDF-") {
                        let _ = fs::remove_dir_all(&temp);
                        return Ok(Rendered {
                            bytes,
                            engine: "soffice".to_string(),
                            simplified: docx.simplified,
                            notes,
                        });
                    }
                    notes.push("soffice 产出的文件不是 PDF 签名，转打印流兜底。".to_string());
                } else {
                    notes.push("soffice 未产出 PDF，转打印流兜底。".to_string());
                }
            }
            Err(error) => {
                notes.push(format!("soffice 失败或超时，转打印流兜底：{}", clip_text(&error, 120)));
            }
        }
    } else {
        notes.push("未找到 soffice，直接使用打印流兜底。".to_string());
    }

    let Some(cupsfilter) = tools.cupsfilter.as_deref() else {
        let _ = fs::remove_dir_all(&temp);
        return Err(format!("{}\n且本机没有 cupsfilter 打印流兜底。", notes.join(" ")));
    };
    let html_path = temp.join("reading.html");
    fs::write(&html_path, render_docx_html(title, markdown, &style))
        .map_err(|e| format!("写入打印流 HTML 中间件失败：{e}"))?;
    let pdf_out = temp.join("print.pdf");
    let args = vec![
        "-m".to_string(),
        "application/pdf".to_string(),
        html_path.to_string_lossy().to_string(),
    ];
    match run_process(cupsfilter, &args, tools.tool_timeout, Some(&pdf_out), None) {
        Ok(_) if pdf_out.is_file() => {
            let bytes = fs::read(&pdf_out).map_err(|e| format!("读取打印流 PDF 失败：{e}"))?;
            if bytes.starts_with(b"%PDF-") {
                let _ = fs::remove_dir_all(&temp);
                return Ok(Rendered {
                    bytes,
                    engine: "print_flow".to_string(),
                    simplified: true,
                    notes,
                });
            }
            let _ = fs::remove_dir_all(&temp);
            Err("打印流产出的文件不是 PDF 签名。".to_string())
        }
        Ok(_) => {
            let _ = fs::remove_dir_all(&temp);
            Err("打印流没有产出 PDF。".to_string())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            Err(format!("{}\n打印流失败：{error}", notes.join(" ")))
        }
    }
}

fn python_has_docx(python: &str) -> bool {
    let args = vec!["-c".to_string(), "import docx".to_string()];
    run_process(python, &args, PROBE_TIMEOUT, None, None)
        .map(|output| output.success)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 进程与临时目录
// ---------------------------------------------------------------------------

struct ProcessOutput {
    success: bool,
    #[allow(dead_code)]
    code: Option<i32>,
    #[allow(dead_code)]
    stdout: String,
    stderr: String,
}

fn run_process(
    program: &str,
    args: &[String],
    timeout: Duration,
    stdout_path: Option<&Path>,
    stderr_path: Option<&Path>,
) -> Result<ProcessOutput, String> {
    let temp = temp_dir("proc")?;
    let out_path = stdout_path
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.join("stdout.txt"));
    let err_path = stderr_path
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.join("stderr.txt"));
    let stdout_file = File::create(&out_path)
        .map_err(|e| format!("创建命令输出文件失败（{}）：{e}", out_path.display()))?;
    let stderr_file = File::create(&err_path)
        .map_err(|e| format!("创建命令错误文件失败（{}）：{e}", err_path.display()))?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| format!("无法启动 {program}：{e}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = read_limited(&out_path).unwrap_or_default();
                let stderr = read_limited(&err_path).unwrap_or_default();
                let _ = fs::remove_dir_all(&temp);
                return Ok(ProcessOutput {
                    success: status.success(),
                    code: status.code(),
                    stdout,
                    stderr,
                });
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = read_limited(&err_path).unwrap_or_default();
                let _ = fs::remove_dir_all(&temp);
                return Err(format!(
                    "{program} 在 {} 秒内未完成（已终止）。{}",
                    timeout.as_secs_f64(),
                    clip_text(&stderr, 120)
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_dir_all(&temp);
                return Err(format!("等待 {program} 失败：{error}"));
            }
        }
    }
}

fn read_limited(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("读取命令输出失败（{}）：{e}", path.display()))?;
    let mut buffer = Vec::new();
    file.take(16 * 1024)
        .read_to_end(&mut buffer)
        .map_err(|e| format!("读取命令输出失败（{}）：{e}", path.display()))?;
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

fn temp_dir(tag: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "omnidesk-export-{tag}-{}-{}",
        std::process::id(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).map_err(|e| format!("创建导出临时目录失败（{}）：{e}", path.display()))?;
    Ok(path)
}

/// 定稿联动：先写 label=final 的正文快照，再走 601 的合法迁移。
pub fn finalize_snapshot(root: &Path, writing_id: &str, now: f64) -> Result<u64, String> {
    let body = document_store::read_body(root, writing_id)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "正文为空，不能写入定稿快照。".to_string())?;
    let spec = SnapshotSpec {
        scope: VersionScope::Full,
        source: VersionSource::User,
        label: "final",
        note: "定稿快照",
    };
    let entry = document_store::save_with_snapshot(root, writing_id, &spec, &body, now)?;
    Ok(entry.version)
}

// ---------------------------------------------------------------------------
// 只读记录查询
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordsQueryRequest {
    pub writing_id: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub section_no: Option<u32>,
    #[serde(default)]
    pub paragraph_no: Option<u32>,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordsQueryResult {
    pub writing_id: String,
    pub citations: Vec<CitationRecord>,
    pub fact_checks: Vec<writing_records::FactCheckRecord>,
}

pub fn query_records(root: &Path, request: &RecordsQueryRequest) -> Result<RecordsQueryResult, String> {
    let writing_id = request.writing_id.trim();
    if !is_safe_id(writing_id) {
        return Err(format!("写作任务编号不合法：{writing_id:?}"));
    }
    let kind = request.kind.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let include_citations = matches!(kind, None | Some("citations"));
    let include_checks = matches!(kind, None | Some("factcheck"));
    if kind.is_some() && !include_citations && !include_checks {
        return Err("记录类型只支持 citations 或 factcheck。".to_string());
    }
    let mut citations = if include_citations {
        writing_records::list_citations(root, writing_id)?
    } else {
        Vec::new()
    };
    let mut fact_checks = if include_checks {
        writing_records::list_fact_checks(root, writing_id)?
    } else {
        Vec::new()
    };
    if let Some(section_no) = request.section_no {
        citations.retain(|row| row.section_no == section_no);
        fact_checks.retain(|row| row.section_no == section_no);
    }
    if let Some(paragraph_no) = request.paragraph_no {
        citations.retain(|row| row.paragraph_no == paragraph_no);
        fact_checks.retain(|row| row.paragraph_no == paragraph_no);
    }
    if let Some(conclusion) = request.conclusion.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        let target = CheckConclusion::from_label(conclusion)?;
        fact_checks.retain(|row| row.conclusion == target);
    }
    if let Some(resolution) = request.resolution.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        let target = writing_records::ResolutionStatus::from_label(resolution)?;
        fact_checks.retain(|row| row.resolution == target);
    }
    Ok(RecordsQueryResult {
        writing_id: writing_id.to_string(),
        citations,
        fact_checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::profile_rules::{GlobalRules, ProfileRules};
    use crate::writing_outline::{OutlineConfirmRequest, OutlineSection, WritingOutline};
    use crate::writing_section::{SectionConfirmRequest, SectionDraft};
    use crate::writing_store::WritingTaskDraft;

    const NOW: f64 = 1_780_700_000.0;

    struct Fixture {
        base: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-export-engine-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let work = base.join("work");
            let config = base.join("config");
            fs::create_dir_all(&work).unwrap();
            fs::create_dir_all(&config).unwrap();
            Self { base, work, config }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn idle_tools() -> ExportTools {
        ExportTools {
            python_candidates: Vec::new(),
            textutil: None,
            soffice: None,
            cupsfilter: None,
            soffice_timeout: Duration::from_millis(200),
            tool_timeout: Duration::from_millis(200),
        }
    }

    fn active_fixture(tag: &str) -> (Fixture, writing_store::WritingTask) {
        let fixture = Fixture::new(tag);
        let task = writing_store::create_writing_task(
            &fixture.work,
            &WritingTaskDraft {
                writing_id: Some(format!("w-{tag}")),
                title: "虚构工作简报".to_string(),
                document_type: writing_store::TYPE_WORK_BRIEF.to_string(),
                writing_purpose: "仅用于导出引擎测试的虚构内容".to_string(),
                audience: "虚构同事".to_string(),
                writing_period: "2026 年度".to_string(),
                topic: "虚构试点".to_string(),
                requirements: "只写虚构事实".to_string(),
                expected_length: "800 字".to_string(),
                writing_style: "简洁平实".to_string(),
                selected_material_ids: vec!["m-613".to_string()],
                selected_knowledge_ids: Vec::new(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            &fixture.work,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 1.0,
        )
        .unwrap();
        writing_outline::confirm_outline(
            &fixture.work,
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: WritingOutline {
                    title: task.title.clone(),
                    writing_purpose: task.writing_purpose.clone(),
                    core_points: vec!["虚构试点总体平稳".to_string()],
                    sections: vec![OutlineSection {
                        no: 1,
                        heading: "工作概况".to_string(),
                        sub_headings: vec!["虚构进展".to_string()],
                        focus: "只概括虚构素材".to_string(),
                        material_ids: vec!["m-613".to_string()],
                        knowledge_ids: Vec::new(),
                        expected_words: 300,
                    }],
                },
                note: Some("虚构大纲确认".to_string()),
            },
            NOW + 2.0,
        )
        .unwrap();
        let task = writing_store::read_writing_task(&fixture.work, &task.writing_id).unwrap();
        (fixture, task)
    }

    fn finalized_fixture(
        tag: &str,
        content: &str,
        citations: bool,
    ) -> (Fixture, writing_store::WritingTask) {
        let (fixture, task) = active_fixture(tag);
        writing_section::confirm_section(
            &fixture.work,
            &SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: SectionDraft {
                    section_no: 1,
                    content: content.to_string(),
                    citations: if citations {
                        vec![writing_section::SectionCitation {
                            source_kind: CitationSourceKind::Material,
                            source_id: "m-613".to_string(),
                            quote: "完成三处试点".to_string(),
                        }]
                    } else {
                        Vec::new()
                    },
                    segments: Vec::new(),
                },
                note: Some("虚构章节确认".to_string()),
            },
            NOW + 3.0,
        )
        .unwrap();
        writing_store::transition_writing_status(
            &fixture.work,
            &task.writing_id,
            writing_store::STATUS_PENDING_REVIEW,
            false,
            Some("虚构核验完成"),
            NOW + 4.0,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            &fixture.work,
            &task.writing_id,
            writing_store::STATUS_FINALIZED,
            false,
            Some("虚构定稿"),
            NOW + 5.0,
        )
        .unwrap();
        (fixture, task)
    }

    fn target_dir(fixture: &Fixture) -> String {
        let dir = fixture.base.join("out");
        fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    fn export_request(task: &writing_store::WritingTask, format: &str, mode: Option<&str>, dir: &str) -> ExportRequest {
        ExportRequest {
            writing_id: task.writing_id.clone(),
            format: format.to_string(),
            mode: mode.map(|value| value.to_string()),
            target_dir: Some(dir.to_string()),
        }
    }

    #[cfg(unix)]
    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn fact_basisless(
        section_no: u32,
        paragraph_no: u32,
        conclusion: CheckConclusion,
        text: &str,
    ) -> writing_records::FactCheckDraft {
        writing_records::FactCheckDraft {
            record_id: None,
            section_no,
            paragraph_no,
            check_object: writing_records::CheckObject::Time,
            conclusion,
            basis: Vec::new(),
            fact_text: Some(text.to_string()),
        }
    }

    #[test]
    fn formal_markdown_strips_frontmatter_and_internal_markers() {
        let source = "---\nschema: omnidesk.document.v1\nstatus: 已定稿\n---\n\n# 虚构标题\n\n第一段〔来源：素材 m-613〕。\n\n内部编号 wt-613-09\n\n（AI 生成）第二段保留。\n";
        let clean = strip_internal(source);
        assert!(clean.starts_with("# 虚构标题"), "{clean}");
        assert!(clean.contains("第一段。"), "{clean}");
        assert!(clean.contains("第二段保留。"), "{clean}");
        for marker in ["schema:", "内部编号", "wt-613", "〔来源", "AI 生成", "AI 草稿"] {
            assert!(!clean.contains(marker), "正式稿仍含 {marker}：{clean}");
        }
    }

    #[test]
    fn formal_markdown_keeps_ordinary_bracket_text() {
        let source = "# 标题\n\n会议〔2026〕第 3 号，材料如下（便于核对）。\n";
        let clean = strip_internal(source);
        assert!(clean.contains("会议〔2026〕第 3 号"), "{clean}");
        assert!(clean.contains("（便于核对）"), "{clean}");
    }

    fn citation(paragraph_no: u32) -> CitationRecord {
        CitationRecord {
            record_id: format!("cite-{paragraph_no}"),
            section_no: 1,
            paragraph_no,
            source_kind: CitationSourceKind::Material,
            source_id: "m-613".to_string(),
            quote: "完成三处试点".to_string(),
            linked_at: "2026-11-09T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn review_markdown_adds_source_markers_and_details() {
        let markdown = "# 标题\n\n## 1. 工作概况\n\n第一段。\n\n第二段。";
        let out = build_review_markdown(markdown, &[citation(1), citation(2)]);
        assert!(out.contains("第一段。〔来源：素材 m-613〕"), "{out}");
        assert!(out.contains("第二段。〔来源：素材 m-613〕"), "{out}");
        assert!(out.contains("## 来源标记"), "{out}");
        assert!(out.contains("第 1 章第 1 段"), "{out}");
        assert!(!out.contains("段落标记未匹配"), "{out}");
    }

    #[test]
    fn review_markdown_lists_unmatched_citation() {
        let markdown = "# 标题\n\n## 1. 工作概况\n\n只有一段。";
        let out = build_review_markdown(markdown, &[citation(9)]);
        assert!(out.contains("第 1 章第 9 段"), "{out}");
        assert!(out.contains("段落标记未匹配，仅列在来源明细"), "{out}");
    }

    #[test]
    fn working_markdown_summarizes_checks() {
        let markdown = "# 标题\n\n正文。";
        let records = vec![
            writing_records::FactCheckRecord {
                record_id: "fc-1".to_string(),
                section_no: 1,
                paragraph_no: 1,
                check_object: writing_records::CheckObject::Time,
                conclusion: CheckConclusion::AnalysisJudgment,
                basis: Vec::new(),
                fact_text: Some("这是虚构判断".to_string()),
                resolution: writing_records::ResolutionStatus::Unresolved,
                checked_at: "2026-11-09T00:00:00Z".to_string(),
            },
            writing_records::FactCheckRecord {
                record_id: "fc-2".to_string(),
                section_no: 1,
                paragraph_no: 2,
                check_object: writing_records::CheckObject::Number,
                conclusion: CheckConclusion::SourceMissing,
                basis: Vec::new(),
                fact_text: Some("虚构数字 3 处".to_string()),
                resolution: writing_records::ResolutionStatus::Accepted,
                checked_at: "2026-11-09T00:00:00Z".to_string(),
            },
        ];
        let out = build_working_markdown(markdown, &records);
        assert!(out.contains("## 核验结果摘要"), "{out}");
        assert!(out.contains("共 2 条，待处理 1 条。"), "{out}");
        assert!(out.contains("属于分析判断：1 条"), "{out}");
        assert!(out.contains("无法找到来源：1 条"), "{out}");
        assert!(out.contains("已接受"), "{out}");
    }

    #[test]
    fn working_markdown_handles_empty_checks() {
        let out = build_working_markdown("# 标题\n\n正文。", &[]);
        assert!(out.contains("本次导出前没有核验记录。"), "{out}");
    }

    #[test]
    fn styles_load_defaults_from_embedded_samples() {
        let fixture = Fixture::new("style-default");
        let gongwen = load_doc_style(&fixture.config, StyleKind::Gongwen).unwrap();
        assert_eq!(gongwen.body_font, "仿宋");
        assert_eq!(gongwen.heading_font, "黑体");
        assert_eq!(gongwen.heading_align, "center");
        let reading = load_doc_style(&fixture.config, StyleKind::Reading).unwrap();
        assert_eq!(reading.body_font, "思源宋体");
        assert_eq!(reading.max_width_cm, Some(12.0));
    }

    #[test]
    fn styles_load_custom_file() {
        let fixture = Fixture::new("style-custom");
        let dir = style_dir(&fixture.config);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("gongwen.md"),
            "body_font: 测试正文\nheading_font: 测试标题\nbody_size_pt: 18\nline_height: 1.5\nmargin_cm: 3\nheading_align: left\n",
        )
        .unwrap();
        let style = load_doc_style(&fixture.config, StyleKind::Gongwen).unwrap();
        assert_eq!(style.body_font, "测试正文");
        assert_eq!(style.body_size_pt, 18.0);
        assert_eq!(style.margin_cm, 3.0);
        assert_eq!(style.heading_align, "left");
    }

    #[test]
    fn styles_reject_missing_or_out_of_range_values() {
        let missing = parse_doc_style("heading_font: 黑体\n", StyleKind::Gongwen).unwrap_err();
        assert!(missing.contains("body_font"), "{missing}");
        let range = parse_doc_style(
            "body_font: 仿宋\nheading_font: 黑体\nbody_size_pt: 900\nline_height: 1.5\nmargin_cm: 2\n",
            StyleKind::Gongwen,
        )
        .unwrap_err();
        assert!(range.contains("body_size_pt"), "{range}");
        let align = parse_doc_style(
            "body_font: 仿宋\nheading_font: 黑体\nbody_size_pt: 16\nline_height: 1.5\nmargin_cm: 2\nheading_align: middle\n",
            StyleKind::Gongwen,
        )
        .unwrap_err();
        assert!(align.contains("heading_align"), "{align}");
    }

    #[test]
    fn html_template_default_and_rejects_unsafe() {
        let fixture = Fixture::new("template-default");
        let template = load_html_template(&fixture.config).unwrap();
        assert!(template.contains("{{content}}"));
        assert!(template.contains("zh-CN"));
        let unsafe_template = validate_html_template("<p><script>alert(1)</script>{{content}}</p>").unwrap_err();
        assert!(unsafe_template.contains("script"), "{unsafe_template}");
        let missing = validate_html_template("<p>没有占位符</p>").unwrap_err();
        assert!(missing.contains("{{content}}"), "{missing}");
        let remote = validate_html_template("<p><img src=\"http://example.com/x.png\">{{content}}</p>").unwrap_err();
        assert!(remote.contains("远程资源"), "{remote}");
    }

    #[test]
    fn wechat_html_escapes_and_renders_blocks() {
        let out = render_wechat_html("<虚构标题>", "# 标题\n\n**重点** <script>\n\n- 条目", "<div>{{content}}</div><title>{{title}}</title>");
        assert!(out.contains("<h1"), "{out}");
        assert!(out.contains("<strong"), "{out}");
        assert!(out.contains("&lt;script&gt;"), "{out}");
        assert!(out.contains("• 条目"), "{out}");
        assert!(out.contains("&lt;虚构标题&gt;"), "{out}");
        assert!(!out.contains("<script>"), "{out}");
    }

    #[test]
    fn formats_and_modes_reject_unknown() {
        assert!(normalize_format("rtf").is_err());
        assert!(normalize_format("MD").unwrap() == FORMAT_MD);
        assert!(normalize_mode(FORMAT_HTML, Some("review")).is_err());
        assert!(normalize_mode(FORMAT_MD, Some("draft")).is_err());
        assert_eq!(normalize_mode(FORMAT_DOCX, None).unwrap(), MODE_FORMAL);
    }

    #[test]
    fn target_dir_rejects_relative_and_parent_escape() {
        let fixture = Fixture::new("target-dir");
        let relative = resolve_target_dir(&fixture.base, "w-target", Some("relative/out")).unwrap_err();
        assert!(relative.contains("绝对路径"), "{relative}");
        let parent = resolve_target_dir(&fixture.base, "w-target", Some("/tmp/../out")).unwrap_err();
        assert!(parent.contains(".."), "{parent}");
    }

    #[test]
    fn unique_file_name_avoids_existing_files() {
        let fixture = Fixture::new("unique-name");
        let dir = fixture.base.join("out");
        fs::create_dir_all(&dir).unwrap();
        let first = unique_file_name("w-unique", NOW, "md", &dir);
        fs::write(&first, b"x").unwrap();
        let second = unique_file_name("w-unique", NOW, "md", &dir);
        assert_ne!(first, second);
        assert!(second.to_string_lossy().ends_with("-1.md"), "{}", second.display());
    }

    #[test]
    fn export_requires_finalized_or_archived_status() {
        let (fixture, task) = active_fixture("export-gate");
        let dir = target_dir(&fixture);
        let request = export_request(&task, "md", None, &dir);
        let error = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &RulePolicy::clean(),
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap_err();
        assert!(error.contains("已定稿"), "{error}");
        assert!(error.contains("写作中"), "{error}");
    }

    #[test]
    fn export_formal_md_writes_file_and_receipt() {
        let (fixture, task) = finalized_fixture(
            "formal",
            "本期完成三处试点，数据仅用于测试。〔来源：素材 m-613〕\n\n内部编号 wt-613-01\n\n（AI 生成）下一段仍为虚构。",
            false,
        );
        let dir = target_dir(&fixture);
        let request = export_request(&task, "md", Some("formal"), &dir);
        let outcome = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &RulePolicy::clean(),
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap();
        let text = fs::read_to_string(&outcome.path).unwrap();
        assert!(text.contains("# 虚构工作简报"), "{text}");
        assert!(text.contains("本期完成三处试点"), "{text}");
        assert!(!text.contains("内部编号"), "{text}");
        assert!(!text.contains("wt-613"), "{text}");
        assert!(!text.contains("〔来源"), "{text}");
        assert!(!text.contains("AI 生成"), "{text}");
        assert_eq!(outcome.engine, "host-markdown");
        assert!(!outcome.simplified);
        assert_eq!(outcome.receipt.format, "md-formal");
        let manifest = document_store::read_manifest(&fixture.work, &task.writing_id).unwrap();
        assert_eq!(manifest.export_receipts.last().unwrap().path, outcome.path);
    }

    #[test]
    fn export_review_md_includes_citations() {
        let (fixture, task) = finalized_fixture(
            "review",
            "本期完成三处试点，数据仅用于测试。\n\n下一段只做虚构说明。",
            true,
        );
        let dir = target_dir(&fixture);
        let request = export_request(&task, "md", Some("review"), &dir);
        let outcome = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &RulePolicy::clean(),
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap();
        let text = fs::read_to_string(&outcome.path).unwrap();
        assert!(text.contains("〔来源：素材 m-613〕"), "{text}");
        assert!(text.contains("## 来源标记"), "{text}");
        assert_eq!(outcome.receipt.format, "md-review");
    }

    #[test]
    fn export_working_md_includes_factchecks() {
        let (fixture, task) = finalized_fixture(
            "working",
            "本期完成三处试点，数据仅用于测试。",
            false,
        );
        writing_records::append_fact_check(
            &fixture.work,
            &task.writing_id,
            &fact_basisless(
                1,
                1,
                CheckConclusion::AnalysisJudgment,
                "这是虚构判断",
            ),
            NOW + 6.0,
        )
        .unwrap();
        let dir = target_dir(&fixture);
        let request = export_request(&task, "md", Some("working"), &dir);
        let outcome = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &RulePolicy::clean(),
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap();
        let text = fs::read_to_string(&outcome.path).unwrap();
        assert!(text.contains("## 核验结果摘要"), "{text}");
        assert!(text.contains("共 1 条，待处理 1 条。"), "{text}");
        assert!(text.contains("属于分析判断：1 条"), "{text}");
        assert_eq!(outcome.receipt.format, "md-working");
    }

    #[test]
    fn export_rejects_redline_hits_without_echo() {
        let (fixture, task) = finalized_fixture(
            "redline",
            "这段虚构内容包含内部红线四个字。",
            false,
        );
        let rules = ProfileRules {
            schema: crate::profile_rules::PROFILE_RULES_SCHEMA.to_string(),
            profile: "测试规则".to_string(),
            global: GlobalRules {
                redline_keywords: vec!["内部红线".to_string()],
                ..Default::default()
            },
            scenes: Default::default(),
            skill_bindings: Default::default(),
        };
        let policy = RulePolicy::from_rules(&rules);
        let dir = target_dir(&fixture);
        let request = export_request(&task, "md", None, &dir);
        let error = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &policy,
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap_err();
        assert!(error.contains("敏感词扫描命中"), "{error}");
        assert!(!error.contains("内部红线"), "错误信息不得回显命中原文：{error}");
        assert_eq!(fs::read_dir(&fixture.base.join("out")).unwrap().count(), 0);
    }

    #[test]
    fn export_html_writes_template_output() {
        let (fixture, task) = finalized_fixture(
            "html",
            "本期完成三处试点，数据仅用于测试。\n\n第二段虚构说明。",
            false,
        );
        let dir = target_dir(&fixture);
        let request = export_request(&task, "html", None, &dir);
        let outcome = export_writing(
            &fixture.work,
            &fixture.base,
            &fixture.config,
            &request,
            &RulePolicy::clean(),
            &idle_tools(),
            NOW + 10.0,
        )
        .unwrap();
        let text = fs::read_to_string(&outcome.path).unwrap();
        assert!(text.starts_with("<!doctype html>"), "{text}");
        assert!(text.contains("<title>虚构工作简报</title>"), "{text}");
        assert!(text.contains("本期完成三处试点"), "{text}");
        assert_eq!(outcome.engine, "host-html");
        assert_eq!(outcome.receipt.format, "html");
    }

    #[cfg(unix)]
    #[test]
    fn docx_textutil_fallback_marks_simplified() {
        let fixture = Fixture::new("docx-textutil");
        let script = fixture.base.join("textutil.sh");
        write_script(&script, "printf 'PK-fake-docx' > \"$5\"");
        let tools = ExportTools {
            textutil: Some(script.to_string_lossy().to_string()),
            tool_timeout: Duration::from_secs(5),
            ..idle_tools()
        };
        let style = load_doc_style(&fixture.config, StyleKind::Gongwen).unwrap();
        let rendered = convert_docx("虚构标题", "# 标题\n\n正文。", &style, &tools, false).unwrap();
        assert_eq!(rendered.engine, "textutil");
        assert!(rendered.simplified);
        assert!(rendered.notes.iter().any(|note| note.contains("简化版")));
        assert!(!rendered.bytes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn docx_office_path_receipt_is_not_simplified() {
        let fixture = Fixture::new("docx-office");
        let script = fixture.base.join("python.sh");
        write_script(
            &script,
            "if [ \"$1\" = \"-c\" ]; then exit 0; fi\nprintf 'PK-fake-office' > \"$3\"",
        );
        let tools = ExportTools {
            python_candidates: vec![script.to_string_lossy().to_string()],
            tool_timeout: Duration::from_secs(5),
            ..idle_tools()
        };
        let style = load_doc_style(&fixture.config, StyleKind::Gongwen).unwrap();
        let rendered = convert_docx("虚构标题", "# 标题\n\n正文。", &style, &tools, false).unwrap();
        assert_eq!(rendered.engine, "office-docx");
        assert!(!rendered.simplified);
        assert!(rendered.notes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn pdf_soffice_timeout_reports_without_fake_success() {
        let fixture = Fixture::new("pdf-timeout");
        let textutil = fixture.base.join("textutil.sh");
        write_script(&textutil, "printf 'PK-fake-docx' > \"$5\"");
        let soffice = fixture.base.join("soffice.sh");
        write_script(&soffice, "sleep 1");
        let tools = ExportTools {
            textutil: Some(textutil.to_string_lossy().to_string()),
            soffice: Some(soffice.to_string_lossy().to_string()),
            soffice_timeout: Duration::from_millis(200),
            tool_timeout: Duration::from_secs(5),
            ..idle_tools()
        };
        let error = convert_pdf("虚构标题", "虚构正文。", &fixture.config, &tools).unwrap_err();
        assert!(error.contains("未完成"), "{error}");
        assert!(error.contains("cupsfilter"), "{error}");
        assert!(error.contains("超时"), "{error}");
    }

    #[test]
    fn query_records_filters_citations_and_fact_checks() {
        let (fixture, task) = finalized_fixture(
            "query",
            "本期完成三处试点，数据仅用于测试。\n\n下一段只做虚构说明。",
            false,
        );
        writing_records::append_citation(
            &fixture.work,
            &task.writing_id,
            &writing_records::CitationDraft {
                record_id: None,
                section_no: 1,
                paragraph_no: 1,
                source_kind: CitationSourceKind::Material,
                source_id: "m-613".to_string(),
                quote: "完成三处试点".to_string(),
            },
            NOW + 6.0,
        )
        .unwrap();
        writing_records::append_citation(
            &fixture.work,
            &task.writing_id,
            &writing_records::CitationDraft {
                record_id: None,
                section_no: 1,
                paragraph_no: 2,
                source_kind: CitationSourceKind::Material,
                source_id: "m-613".to_string(),
                quote: "虚构说明".to_string(),
            },
            NOW + 7.0,
        )
        .unwrap();
        writing_records::append_fact_check(
            &fixture.work,
            &task.writing_id,
            &fact_basisless(1, 1, CheckConclusion::AnalysisJudgment, "虚构判断"),
            NOW + 8.0,
        )
        .unwrap();
        writing_records::append_fact_check(
            &fixture.work,
            &task.writing_id,
            &fact_basisless(1, 2, CheckConclusion::SourceMissing, "虚构数字"),
            NOW + 9.0,
        )
        .unwrap();

        let citations = query_records(
            &fixture.work,
            &RecordsQueryRequest {
                writing_id: task.writing_id.clone(),
                kind: Some("citations".to_string()),
                section_no: Some(1),
                paragraph_no: Some(1),
                conclusion: None,
                resolution: None,
            },
        )
        .unwrap();
        assert_eq!(citations.citations.len(), 1);
        assert!(citations.fact_checks.is_empty());

        let missing = query_records(
            &fixture.work,
            &RecordsQueryRequest {
                writing_id: task.writing_id.clone(),
                kind: Some("factcheck".to_string()),
                section_no: None,
                paragraph_no: None,
                conclusion: Some("无法找到来源".to_string()),
                resolution: None,
            },
        )
        .unwrap();
        assert_eq!(missing.fact_checks.len(), 1);
        assert!(missing.citations.is_empty());

        let unknown = query_records(
            &fixture.work,
            &RecordsQueryRequest {
                writing_id: task.writing_id.clone(),
                kind: Some("outline".to_string()),
                section_no: None,
                paragraph_no: None,
                conclusion: None,
                resolution: None,
            },
        )
        .unwrap_err();
        assert!(unknown.contains("citations"), "{unknown}");
    }

    #[test]
    fn finalize_snapshot_writes_final_version() {
        let (fixture, task) = finalized_fixture(
            "finalize",
            "本期完成三处试点，数据仅用于测试。",
            false,
        );
        let version = finalize_snapshot(&fixture.work, &task.writing_id, NOW + 20.0).unwrap();
        let manifest = document_store::read_manifest(&fixture.work, &task.writing_id).unwrap();
        let entry = manifest
            .versions
            .iter()
            .find(|entry| entry.version == version)
            .unwrap();
        assert_eq!(entry.scope, VersionScope::Full);
        assert_eq!(entry.source, VersionSource::User);
        assert_eq!(entry.note, "定稿快照");
    }
}

