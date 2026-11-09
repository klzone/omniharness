//! 分章节写作 IPC（OD-WT-607）。
//!
//! 照 606 writing_outline 模式：宿主组提示词（确认大纲该节 + 该章素材 +
//! 知识依据编号 + 上下章摘要），模型调用仍在界面侧；模型输出只是候选，
//! 由本模块做宿主校验，用户经 confirm 才写入正文与引用记录。
//!
//! 正文.md 仍只走 602 document_store::save_with_snapshot 唯一写入口；每次
//! 章节确认都保留完整正文版本快照。未确认章节不写占位，全文组合时才显式
//! 标注「（本章待写作）」，避免把占位当成已确认内容。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document_store::{self, SnapshotSpec, VersionScope, VersionSource};
use crate::material_store::{self, WritingMaterial, REVIEW_CONFIRMED};
use crate::writing_outline::{self, OutlineSection, WritingOutline};
use crate::writing_records::{self, CitationDraft, CitationSourceKind};
use crate::writing_store::{self, WritingTask};

/// material-writing Skill 四分法，顺序固定，供提示词与校验共用。
pub const SEGMENT_KINDS: &[&str] = &["事实", "分析", "判断", "建议"];
/// 修订指令类型白名单。
pub const REVISE_KINDS: &[&str] = &["精简", "扩展", "语气", "术语统一", "消除重复"];
/// 未确认章节在全文组合中的显式占位。
pub const SECTION_PLACEHOLDER: &str = "（本章待写作）";

pub const MAX_CONTENT_CHARS: usize = 60_000;
pub const MAX_CITATIONS: usize = 200;
pub const MAX_SEGMENTS: usize = 400;
pub const MAX_SOURCE_ID_CHARS: usize = 200;
pub const MAX_QUOTE_CHARS: usize = 20_000;
pub const MAX_SEGMENT_TEXT_CHARS: usize = 20_000;
pub const MAX_NOTE_CHARS: usize = 200;
pub const MAX_INSTRUCTION_CHARS: usize = 500;
pub const MAX_PROMPT_MATERIAL_CHARS: usize = 2_000;
pub const MAX_PROMPT_QUOTE_CHARS: usize = 300;
pub const MAX_SUMMARY_CHARS: usize = 200;

/// 章节稿引用：来源只允许任务已选素材或知识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionCitation {
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
}

/// 四分法标注。kind 由宿主按白名单校验，避免模型自造标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionSegment {
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub quote: Option<String>,
}

/// 章节稿候选。字段名直接作为模型输出契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionDraft {
    pub section_no: u32,
    pub content: String,
    #[serde(default)]
    pub citations: Vec<SectionCitation>,
    #[serde(default)]
    pub segments: Vec<SectionSegment>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionGenerateRequest {
    pub writing_id: String,
    pub section_no: u32,
}

/// 生成提示词回执。本命令不写任何文件。
#[derive(Debug, Clone, Serialize)]
pub struct WritingSectionPrompt {
    pub prompt: String,
    pub section_no: u32,
    pub heading: String,
    pub material_count: usize,
    pub knowledge_count: usize,
    pub previous_summary: String,
    pub next_summary: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionConfirmRequest {
    pub writing_id: String,
    pub draft: SectionDraft,
    #[serde(default)]
    pub note: Option<String>,
}

/// 章节确认/读取回执。
#[derive(Debug, Clone, Serialize)]
pub struct WritingSectionState {
    pub writing_id: String,
    pub section_no: u32,
    pub heading: String,
    pub content: Option<String>,
    pub confirmed: bool,
    pub status: String,
    pub version: u64,
    pub updated_at: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WritingSectionConfirmed {
    pub writing_id: String,
    pub section_no: u32,
    pub heading: String,
    pub content: String,
    pub status: String,
    pub version: u64,
    pub updated_at: String,
    pub citations_added: usize,
    pub all_sections_confirmed: bool,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionReviseRequest {
    pub writing_id: String,
    pub section_no: u32,
    #[serde(default)]
    pub paragraph_no: Option<u32>,
    pub instruction_kind: String,
    #[serde(default)]
    pub instruction: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WritingSectionRevisePrompt {
    pub prompt: String,
    pub section_no: u32,
    pub heading: String,
    pub paragraph_no: Option<u32>,
    pub instruction_kind: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

fn normalize_ids(values: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn normalized_text(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect()
}

fn clip_chars(value: &str, limit: usize) -> String {
    let chars: Vec<char> = value.trim().chars().collect();
    if chars.len() <= limit {
        return chars.iter().collect();
    }
    let mut out: String = chars[..limit].iter().collect();
    out.push('…');
    out
}

fn find_section<'a>(
    outline: &'a WritingOutline,
    section_no: u32,
) -> Result<&'a OutlineSection, String> {
    outline
        .sections
        .iter()
        .find(|section| section.no == section_no)
        .ok_or_else(|| format!("大纲中没有第 {section_no} 章。"))
}

fn validate_note(note: Option<&str>) -> Result<Option<String>, String> {
    let Some(note) = note else { return Ok(None) };
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("章节确认说明最多 {MAX_NOTE_CHARS} 字。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("章节确认说明含控制字符。".to_string());
    }
    Ok(Some(trimmed.to_string()))
}

fn validate_source_id(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("引用来源编号不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_SOURCE_ID_CHARS {
        return Err(format!("引用来源编号最多 {MAX_SOURCE_ID_CHARS} 字符。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("引用来源编号含控制字符。".to_string());
    }
    Ok(trimmed.to_string())
}

fn validate_quote(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field}不能为空。"));
    }
    if trimmed.chars().count() > MAX_QUOTE_CHARS {
        return Err(format!("{field}最多 {MAX_QUOTE_CHARS} 字符。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(format!("{field}含控制字符。"));
    }
    Ok(trimmed.to_string())
}

fn validate_segment_kind(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if !SEGMENT_KINDS.contains(&trimmed) {
        return Err(format!(
            "四分法标注类型「{trimmed}」不在允许范围（{}）。",
            SEGMENT_KINDS.join(" / ")
        ));
    }
    Ok(trimmed.to_string())
}

/// 宿主校验并规范化章节稿。
///
/// 关键防线：正文不能是引用原文的拼接；引用来源必须存在于任务已选集合；
/// 四分法标签只允许白名单；正文不得夹带 Markdown 一级/二级标题，以免破坏
/// 正文.md 的章节定位。
pub fn validate_section_draft(
    draft: &SectionDraft,
    task: &WritingTask,
    outline: &WritingOutline,
) -> Result<SectionDraft, String> {
    let section = find_section(outline, draft.section_no)?;
    let content = draft.content.trim();
    if content.is_empty() {
        return Err(format!("第 {} 章正文不能为空。", draft.section_no));
    }
    if content.chars().count() > MAX_CONTENT_CHARS {
        return Err(format!("章节正文最多 {MAX_CONTENT_CHARS} 字符。"));
    }
    if content.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
        return Err("章节正文含不允许的控制字符。".to_string());
    }
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return Err(
                "章节正文不得包含 Markdown 标题行；章节标题由大纲固定。".to_string(),
            );
        }
    }
    if draft.citations.len() > MAX_CITATIONS {
        return Err(format!("单章引用最多 {MAX_CITATIONS} 条。"));
    }
    if draft.segments.len() > MAX_SEGMENTS {
        return Err(format!("单章四分法标注最多 {MAX_SEGMENTS} 条。"));
    }
    let mut citations = Vec::with_capacity(draft.citations.len());
    let mut quote_join = String::new();
    let mut quote_values: Vec<String> = Vec::new();
    for (index, citation) in draft.citations.iter().enumerate() {
        let source_id = validate_source_id(&citation.source_id)
            .map_err(|e| format!("citations[{index}].source_id：{e}"))?;
        let quote = validate_quote(&citation.quote, "引用原文")?;
        match citation.source_kind {
            CitationSourceKind::Material => {
                if !task
                    .selected_material_ids
                    .iter()
                    .any(|value| value == &source_id)
                {
                    return Err(format!(
                        "citations[{index}] 引用了未加入素材包的素材编号：{source_id}"
                    ));
                }
            }
            CitationSourceKind::Knowledge => {
                if !task
                    .selected_knowledge_ids
                    .iter()
                    .any(|value| value == &source_id)
                {
                    return Err(format!(
                        "citations[{index}] 引用了未加入素材包的知识编号：{source_id}"
                    ));
                }
            }
        }
        quote_values.push(quote.clone());
        quote_join.push_str(&quote);
        citations.push(SectionCitation {
            source_kind: citation.source_kind,
            source_id,
            quote,
        });
    }
    let mut segments = Vec::with_capacity(draft.segments.len());
    for (index, segment) in draft.segments.iter().enumerate() {
        let kind = validate_segment_kind(&segment.kind)
            .map_err(|e| format!("segments[{index}].kind：{e}"))?;
        let text = segment.text.trim();
        if text.is_empty() {
            return Err(format!("segments[{index}].text 不能为空。"));
        }
        if text.chars().count() > MAX_SEGMENT_TEXT_CHARS {
            return Err(format!(
                "segments[{index}].text 最多 {MAX_SEGMENT_TEXT_CHARS} 字符。"
            ));
        }
        if text.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
            return Err(format!("segments[{index}].text 含控制字符。"));
        }
        let quote = match segment.quote.as_deref() {
            Some(value) if !value.trim().is_empty() => {
                let quote = validate_quote(value, "标注引用")?;
                quote_values.push(quote.clone());
                quote_join.push_str(&quote);
                Some(quote)
            }
            _ => None,
        };
        segments.push(SectionSegment {
            kind,
            text: text.to_string(),
            quote,
        });
    }
    let content_normalized = normalized_text(content);
    let single_quote_copy = quote_values
        .iter()
        .any(|quote| content_normalized == normalized_text(quote));
    if single_quote_copy
        || (!quote_join.is_empty() && content_normalized == normalized_text(&quote_join))
    {
        return Err(
            "章节正文只是引用原文的拼接，没有形成自己的表述；请改写后再确认。".to_string(),
        );
    }
    Ok(SectionDraft {
        section_no: section.no,
        content: content.to_string(),
        citations,
        segments,
    })
}

/// 校验模型输出的章节稿 JSON。允许 Markdown 代码块包裹；失败不落盘。
pub fn parse_section_draft_json(
    result_text: &str,
    task: &WritingTask,
    outline: &WritingOutline,
) -> Result<SectionDraft, String> {
    let text = result_text.trim();
    if text.is_empty() {
        return Err("章节稿生成结果为空。".to_string());
    }
    let value = crate::knowledge_ingest::parse_result_text(text)
        .map_err(|e| format!("章节稿生成结果不是合法 JSON：{e}"))?;
    let raw: SectionDraft = serde_json::from_value(value)
        .map_err(|e| format!("章节稿生成结果不符合约定结构：{e}"))?;
    validate_section_draft(&raw, task, outline)
}

fn parse_body_sections(body: &str) -> Result<BTreeMap<u32, String>, String> {
    let mut sections = BTreeMap::new();
    let mut current: Option<(u32, Vec<String>)> = None;
    let mut saw_title = false;
    for raw_line in body.lines() {
        let line = raw_line.trim_end();
        if let Some(rest) = line.trim_start().strip_prefix("## ") {
            if let Some((no, lines)) = current.take() {
                let content = lines.join("\n").trim().to_string();
                if sections.insert(no, content).is_some() {
                    return Err(format!("正文.md 中章节 {no} 重复出现。"));
                }
            }
            let (no_text, _heading) = rest
                .split_once(". ")
                .ok_or_else(|| format!("正文.md 章节标题格式不正确：{line}"))?;
            let no: u32 = no_text
                .trim()
                .parse()
                .map_err(|_| format!("正文.md 章节编号不是正整数：{no_text}"))?;
            current = Some((no, Vec::new()));
            continue;
        }
        if line.trim_start().starts_with("# ") {
            if current.is_none() && !saw_title {
                saw_title = true;
                continue;
            }
            return Err("正文.md 出现大纲章节之外的标题行。".to_string());
        }
        if let Some((_, lines)) = current.as_mut() {
            lines.push(line.to_string());
        }
    }
    if let Some((no, lines)) = current.take() {
        if sections.insert(no, lines.join("\n").trim().to_string()).is_some() {
            return Err(format!("正文.md 中章节 {no} 重复出现。"));
        }
    }
    Ok(sections)
}

fn render_section_heading(section: &OutlineSection) -> String {
    format!("## {}. {}", section.no, section.heading.trim())
}

/// 把已确认章节按大纲顺序渲染为正文.md。未确认章节不写入文件。
fn render_body(outline: &WritingOutline, sections: &BTreeMap<u32, String>) -> String {
    let mut out = format!("# {}\n", outline.title.trim());
    for section in &outline.sections {
        let Some(content) = sections.get(&section.no) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", render_section_heading(section));
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", content.trim());
    }
    out.trim_end().to_string()
}

/// 全文组合：按大纲顺序拼接已确认章节，未确认章节显式占位。
pub fn compose_full_text(outline: &WritingOutline, body: Option<&str>) -> String {
    let sections = body
        .map(parse_body_sections)
        .and_then(|result| result.ok())
        .unwrap_or_default();
    let mut out = format!("# {}\n", outline.title.trim());
    for section in &outline.sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", render_section_heading(section));
        let _ = writeln!(out);
        let content = sections
            .get(&section.no)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(SECTION_PLACEHOLDER);
        let _ = writeln!(out, "{content}");
    }
    out.trim_end().to_string()
}

/// 读取并组合全文，供 613 导出与 609 核验调用。
pub fn compose_full_text_for(root: &Path, writing_id: &str) -> Result<String, String> {
    let writing_id = writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let outline = writing_outline::read_confirmed_outline(root, writing_id)?.outline;
    let body = document_store::read_body(root, writing_id)?;
    if let Some(body) = body.as_deref() {
        parse_body_sections(body)?;
    }
    Ok(compose_full_text(&outline, body.as_deref()))
}

fn section_paragraphs(content: &str) -> Vec<String> {
    content
        .split("\n\n")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn paragraph_for_quote(content: &str, quote: &str) -> u32 {
    let normalized_quote = normalized_text(quote);
    section_paragraphs(content)
        .iter()
        .position(|paragraph| {
            let normalized = normalized_text(paragraph);
            normalized.contains(&normalized_quote)
        })
        .map(|index| index as u32 + 1)
        .unwrap_or(1)
}

fn section_summary(
    outline: &WritingOutline,
    sections: &BTreeMap<u32, String>,
    index: isize,
) -> String {
    if index < 0 || index as usize >= outline.sections.len() {
        return "（无）".to_string();
    }
    let section = &outline.sections[index as usize];
    match sections.get(&section.no) {
        Some(content) if !content.trim().is_empty() => format!(
            "{}：{}",
            section.heading.trim(),
            clip_chars(content, MAX_SUMMARY_CHARS)
        ),
        _ => format!(
            "{}（尚未写作；大纲重点：{}）",
            section.heading.trim(),
            clip_chars(&section.focus, MAX_SUMMARY_CHARS)
        ),
    }
}

/// 组章节提示词：确认大纲该节 + 该章素材 + 知识依据编号 + 上下章摘要。
pub fn build_section_prompt(
    task: &WritingTask,
    outline: &WritingOutline,
    section: &OutlineSection,
    materials: &[WritingMaterial],
    knowledge_ids: &[String],
    body: Option<&str>,
) -> String {
    let sections = body
        .map(parse_body_sections)
        .and_then(|result| result.ok())
        .unwrap_or_default();
    let index = outline
        .sections
        .iter()
        .position(|value| value.no == section.no)
        .map(|value| value as isize)
        .unwrap_or(-1);
    let previous = section_summary(outline, &sections, index - 1);
    let next = section_summary(outline, &sections, index + 1);
    let mut prompt = String::new();
    let _ = writeln!(
        prompt,
        "你是中文公文写作助手。请根据已确认的大纲和本章素材撰写第 {} 章正文。只输出 JSON，不输出解释性文字。",
        section.no
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 一、写作任务");
    let _ = writeln!(prompt, "- 文稿类型：{}", task.document_type);
    let _ = writeln!(prompt, "- 标题：{}", task.title);
    let _ = writeln!(prompt, "- 写作目的：{}", task.writing_purpose);
    let _ = writeln!(prompt, "- 目标受众：{}", task.audience);
    let _ = writeln!(prompt, "- 时间范围：{}", task.writing_period);
    let _ = writeln!(prompt, "- 预计篇幅：{}", task.expected_length);
    let _ = writeln!(prompt, "- 写作风格：{}", task.writing_style);
    if !task.requirements.trim().is_empty() {
        let _ = writeln!(prompt, "- 写作要求：");
        for line in task
            .requirements
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let _ = writeln!(prompt, "  - {line}");
        }
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 二、本章大纲（已确认）");
    let _ = writeln!(prompt, "- 章节号：{}", section.no);
    let _ = writeln!(prompt, "- 章节标题：{}", section.heading.trim());
    let _ = writeln!(prompt, "- 写作重点：{}", section.focus.trim());
    let _ = writeln!(prompt, "- 预计字数：{}", section.expected_words);
    if !section.sub_headings.is_empty() {
        let _ = writeln!(prompt, "- 小节：{}", section.sub_headings.join("、"));
    }
    let _ = writeln!(prompt, "- 上一章摘要：{previous}");
    let _ = writeln!(prompt, "- 下一章摘要：{next}");
    let _ = writeln!(
        prompt,
        "上下章摘要只用于衔接，不要复述或扩写其他章节的全文。"
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 三、本章素材");
    if materials.is_empty() {
        let _ = writeln!(
            prompt,
            "本章没有映射素材，只能使用下面列出的知识依据编号；不得编造事实。"
        );
    }
    for material in materials {
        let _ = writeln!(prompt);
        let _ = writeln!(prompt, "### 素材 {}", material.material_id);
        let _ = writeln!(prompt, "标题：{}", material.title);
        let _ = writeln!(prompt, "类型：{}", material.material_type);
        if let Some(date) = material.event_date.as_deref() {
            let _ = writeln!(prompt, "事件日期：{date}");
        }
        if !material.source_quote.trim().is_empty() {
            let _ = writeln!(
                prompt,
                "来源原文：{}",
                clip_chars(&material.source_quote, MAX_PROMPT_QUOTE_CHARS)
            );
        }
        let _ = writeln!(
            prompt,
            "正文：{}",
            clip_chars(&material.content, MAX_PROMPT_MATERIAL_CHARS)
        );
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 四、本章知识依据编号");
    if knowledge_ids.is_empty() {
        let _ = writeln!(prompt, "（无）");
    } else {
        for id in knowledge_ids {
            let _ = writeln!(prompt, "- {id}");
        }
        let _ = writeln!(
            prompt,
            "知识正文未在本提示词内展开，只能作为编号引用，不得编造内容。"
        );
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 五、四分法标注");
    prompt.push_str(
        "每一段都要判断它在文中的性质，并在 segments 中标注：\n\
- 事实：素材或知识中有直接依据的客观信息；\n\
- 分析：对事实的归纳、比较或原因推断；\n\
- 判断：对工作成效、问题性质或趋势的评价；\n\
- 建议：下一步措施、改进方向或请求事项。\n\
事实必须配 quote（引用素材原文或知识片段），分析与判断不得伪装成事实。\n",
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 六、禁止清单");
    prompt.push_str(
        "1. 不虚构工作成果、领导讲话、政策文件、文号、数据、机构名或人名。\n\
2. 计划、打算、拟办事项不得写成已经完成。\n\
3. 相关性不得写成因果关系。\n\
4. 不得空洞凑篇幅，不得重复堆砌同一意思。\n\
5. 不得把素材里的个别情况扩大为普遍结论。\n\
6. 引用只能来自上面列出的素材编号与知识编号，不得自造来源。\n",
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 七、输出要求");
    prompt.push_str(
        "只输出一个 JSON 对象，字段固定如下，不得增加字段：\n\
{\"section_no\":1,\"content\":\"本章正文\",\"citations\":[{\"source_kind\":\"material\",\"source_id\":\"素材编号\",\"quote\":\"引用原文\"}],\"segments\":[{\"kind\":\"事实\",\"text\":\"对应正文片段\",\"quote\":\"引用原文\"}]}\n",
    );
    let _ = writeln!(
        prompt,
        "规则：section_no 必须为 {}；content 不得为空，不得是引用原文的简单拼接；",
        section.no
    );
    let _ = writeln!(
        prompt,
        "citations.source_kind 只能是 material 或 knowledge，source_id 必须来自上面列出的编号；"
    );
    let _ = writeln!(
        prompt,
        "segments.kind 只能是 {}；quote 可省略。",
        SEGMENT_KINDS.join(" / ")
    );
    let _ = writeln!(
        prompt,
        "content 内不要写 Markdown 标题行；章节标题由大纲固定。"
    );
    prompt
}

fn effective_material_ids<'a>(task: &'a WritingTask, section: &'a OutlineSection) -> &'a [String] {
    if section.material_ids.is_empty() {
        &task.selected_material_ids
    } else {
        &section.material_ids
    }
}

fn effective_knowledge_ids<'a>(
    task: &'a WritingTask,
    section: &'a OutlineSection,
) -> &'a [String] {
    if section.knowledge_ids.is_empty() {
        &task.selected_knowledge_ids
    } else {
        &section.knowledge_ids
    }
}

fn load_section_materials(
    root: &Path,
    task: &WritingTask,
    section: &OutlineSection,
) -> Result<Vec<WritingMaterial>, String> {
    let ids = normalize_ids(effective_material_ids(task, section));
    let mut materials = Vec::with_capacity(ids.len());
    for id in ids {
        let material = material_store::read_material(root, &id)
            .map_err(|error| format!("读取本章素材 {id} 失败：{error}"))?;
        if material.review_status != REVIEW_CONFIRMED {
            return Err(format!(
                "素材「{}」当前审核状态为「{}」，不属于可信素材包；请重新确认或移出本章素材。",
                id, material.review_status
            ));
        }
        materials.push(material);
    }
    Ok(materials)
}

/// 生成章节提示词前的宿主校验：任务必须在写作中，大纲已确认且该节存在。
pub fn generate_section_prompt(
    root: &Path,
    request: &SectionGenerateRequest,
) -> Result<WritingSectionPrompt, String> {
    let writing_id = request.writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let task = writing_store::read_writing_task(root, writing_id)?;
    if task.status != writing_store::STATUS_WRITING {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」可以生成章节正文。",
            task.status,
            writing_store::STATUS_WRITING
        ));
    }
    let outline = writing_outline::read_confirmed_outline(root, writing_id)?.outline;
    let section = find_section(&outline, request.section_no)?.clone();
    let materials = load_section_materials(root, &task, &section)?;
    let body = document_store::read_body(root, writing_id)?;
    let knowledge_ids = normalize_ids(effective_knowledge_ids(&task, &section));
    let prompt = build_section_prompt(
        &task,
        &outline,
        &section,
        &materials,
        &knowledge_ids,
        body.as_deref(),
    );
    let sections = body
        .as_deref()
        .map(parse_body_sections)
        .and_then(|result| result.ok())
        .unwrap_or_default();
    let index = outline
        .sections
        .iter()
        .position(|value| value.no == section.no)
        .map(|value| value as isize)
        .unwrap_or(-1);
    Ok(WritingSectionPrompt {
        prompt,
        section_no: section.no,
        heading: section.heading.clone(),
        material_count: materials.len(),
        knowledge_count: knowledge_ids.len(),
        previous_summary: section_summary(&outline, &sections, index - 1),
        next_summary: section_summary(&outline, &sections, index + 1),
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

fn confirm_allowed(status: &str) -> bool {
    matches!(
        status,
        writing_store::STATUS_WRITING
            | writing_store::STATUS_FACT_CHECKING
            | writing_store::STATUS_PENDING_REVIEW
    )
}

/// 用户确认章节稿：校验 → 写正文.md + 快照 → 追加本章引用 → 全部确认后待核验。
pub fn confirm_section(
    root: &Path,
    request: &SectionConfirmRequest,
    now: f64,
) -> Result<WritingSectionConfirmed, String> {
    let writing_id = request.writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let mut task = writing_store::read_writing_task(root, writing_id)?;
    if !confirm_allowed(&task.status) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」「{}」或「{}」可以确认章节正文。",
            task.status,
            writing_store::STATUS_WRITING,
            writing_store::STATUS_FACT_CHECKING,
            writing_store::STATUS_PENDING_REVIEW
        ));
    }
    let confirmed_outline = writing_outline::read_confirmed_outline(root, writing_id)?;
    let outline = confirmed_outline.outline;
    let section = find_section(&outline, request.draft.section_no)?.clone();
    let draft = validate_section_draft(&request.draft, &task, &outline)?;
    let note = validate_note(request.note.as_deref())?;
    let body = document_store::read_body(root, writing_id)?;
    let mut sections = match body.as_deref() {
        Some(body) => parse_body_sections(body)?,
        None => BTreeMap::new(),
    };
    let outline_nos: BTreeSet<u32> = outline.sections.iter().map(|section| section.no).collect();
    for no in sections.keys() {
        if !outline_nos.contains(no) {
            return Err(format!(
                "正文.md 中存在大纲之外的章节 {no}；请先修复正文，避免覆盖未知内容。"
            ));
        }
    }
    let content = draft.content.clone();
    sections.insert(section.no, content.clone());
    let markdown = render_body(&outline, &sections);
    let label = format!("section-{}-confirmed", section.no);
    let spec = SnapshotSpec {
        scope: VersionScope::Section,
        source: VersionSource::User,
        label: &label,
        note: note.as_deref().unwrap_or("章节确认"),
    };
    let entry = document_store::save_with_snapshot(root, writing_id, &spec, &markdown, now)
        .map_err(|error| format!("章节正文写入失败：{error}"))?;
    let mut citations_added = 0usize;
    for citation in &draft.citations {
        let paragraph_no = paragraph_for_quote(&content, &citation.quote);
        writing_records::append_citation(
            root,
            writing_id,
            &CitationDraft {
                record_id: None,
                section_no: section.no,
                paragraph_no,
                source_kind: citation.source_kind,
                source_id: citation.source_id.clone(),
                quote: citation.quote.clone(),
            },
            now,
        )
        .map_err(|error| format!("正文已写入，但本章引用记录追加失败：{error}"))?;
        citations_added += 1;
    }
    let all_sections_confirmed = outline
        .sections
        .iter()
        .all(|section| sections.get(&section.no).is_some_and(|value| !value.trim().is_empty()));
    if task.status == writing_store::STATUS_WRITING && all_sections_confirmed {
        task = writing_store::transition_writing_status(
            root,
            writing_id,
            writing_store::STATUS_FACT_CHECKING,
            false,
            Some("全部章节已确认"),
            now,
        )?;
    }
    Ok(WritingSectionConfirmed {
        writing_id: task.writing_id.clone(),
        section_no: section.no,
        heading: section.heading.clone(),
        content,
        status: task.status.clone(),
        version: entry.version,
        updated_at: task.updated_at.clone(),
        citations_added,
        all_sections_confirmed,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

/// 读取当前正文中的指定章节。未确认时 content=None。
pub fn read_section_state(
    root: &Path,
    writing_id: &str,
    section_no: u32,
) -> Result<WritingSectionState, String> {
    let writing_id = writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let task = writing_store::read_writing_task(root, writing_id)?;
    let outline = writing_outline::read_confirmed_outline(root, writing_id)?.outline;
    let section = find_section(&outline, section_no)?.clone();
    let body = document_store::read_body(root, writing_id)?;
    let sections = match body.as_deref() {
        Some(body) => parse_body_sections(body)?,
        None => BTreeMap::new(),
    };
    let content = sections
        .get(&section_no)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let manifest = document_store::read_manifest(root, writing_id)?;
    Ok(WritingSectionState {
        writing_id: task.writing_id,
        section_no: section.no,
        heading: section.heading,
        confirmed: content.is_some(),
        content,
        status: task.status,
        version: manifest.current_version,
        updated_at: manifest.updated_at,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

fn validate_revise_kind(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if !REVISE_KINDS.contains(&trimmed) {
        return Err(format!(
            "修改指令类型「{trimmed}」不在允许范围（{}）。",
            REVISE_KINDS.join(" / ")
        ));
    }
    Ok(trimmed.to_string())
}

fn build_revise_prompt(
    task: &WritingTask,
    section: &OutlineSection,
    content: &str,
    paragraph_no: Option<u32>,
    instruction_kind: &str,
    instruction: &str,
) -> Result<String, String> {
    let paragraphs = section_paragraphs(content);
    let target = match paragraph_no {
        Some(no) => {
            if no == 0 || no as usize > paragraphs.len() {
                return Err(format!(
                    "第 {} 章只有 {} 个段落，段落序号 {no} 不存在。",
                    section.no,
                    paragraphs.len()
                ));
            }
            format!("第 {no} 段：{}", paragraphs[no as usize - 1])
        }
        None => "整章".to_string(),
    };
    let mut prompt = String::new();
    let _ = writeln!(
        prompt,
        "你是中文公文写作修订助手。请按用户指令修改第 {} 章{}，只输出 JSON，不输出解释性文字。",
        section.no,
        if paragraph_no.is_some() { "指定段落" } else { "整章" }
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 一、写作任务");
    let _ = writeln!(prompt, "- 文稿类型：{}", task.document_type);
    let _ = writeln!(prompt, "- 标题：{}", task.title);
    let _ = writeln!(prompt, "- 写作风格：{}", task.writing_style);
    let _ = writeln!(prompt, "- 时间范围：{}", task.writing_period);
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 二、章节");
    let _ = writeln!(prompt, "- 章节标题：{}", section.heading.trim());
    let _ = writeln!(prompt, "- 写作重点：{}", section.focus.trim());
    let _ = writeln!(prompt, "- 修改范围：{target}");
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 三、当前章节正文");
    let _ = writeln!(prompt, "{content}");
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 四、修订指令");
    let _ = writeln!(prompt, "- 指令类型：{instruction_kind}");
    if instruction.trim().is_empty() {
        let _ = writeln!(prompt, "- 用户补充说明：（未填写）");
    } else {
        let _ = writeln!(prompt, "- 用户补充说明：{}", instruction.trim());
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 五、修订原则");
    prompt.push_str(
        "1. 只按指令修改，不得新增未提供的事实、数据、机构、人名或政策依据。\n\
2. 计划不得改成已完成，相关性不得改成因果。\n\
3. 保持与上下章衔接，不复述其他章节全文。\n\
4. 事实性表述保留原有引用；不得删掉必要的来源依据。\n\
5. 精简只删冗余，不删事实；扩展只补充已有依据的展开，不编造。\n",
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 六、输出要求");
    prompt.push_str(
        "只输出一个 JSON 对象，字段与章节稿一致：\n\
{\"section_no\":1,\"content\":\"修订后的本章正文\",\"citations\":[{\"source_kind\":\"material\",\"source_id\":\"素材编号\",\"quote\":\"引用原文\"}],\"segments\":[{\"kind\":\"事实\",\"text\":\"对应正文片段\",\"quote\":\"引用原文\"}]}\n",
    );
    let _ = writeln!(
        prompt,
        "section_no 必须为 {}；content 不得为空，不得是引用原文的简单拼接；segments.kind 只能是 {}。",
        section.no,
        SEGMENT_KINDS.join(" / ")
    );
    Ok(prompt)
}

/// 构建修订提示词；应用修订仍走 confirm_section 同路径并保留原文快照。
pub fn revise_section_prompt(
    root: &Path,
    request: &SectionReviseRequest,
) -> Result<WritingSectionRevisePrompt, String> {
    let writing_id = request.writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let task = writing_store::read_writing_task(root, writing_id)?;
    if !confirm_allowed(&task.status) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」「{}」或「{}」可以构建章节修订提示词。",
            task.status,
            writing_store::STATUS_WRITING,
            writing_store::STATUS_FACT_CHECKING,
            writing_store::STATUS_PENDING_REVIEW
        ));
    }
    let outline = writing_outline::read_confirmed_outline(root, writing_id)?.outline;
    let section = find_section(&outline, request.section_no)?.clone();
    let instruction_kind = validate_revise_kind(&request.instruction_kind)?;
    if request.instruction.trim().chars().count() > MAX_INSTRUCTION_CHARS {
        return Err(format!("修订补充说明最多 {MAX_INSTRUCTION_CHARS} 字。"));
    }
    if request.instruction.chars().any(char::is_control) {
        return Err("修订补充说明含控制字符。".to_string());
    }
    let body = document_store::read_body(root, writing_id)?;
    let sections = match body.as_deref() {
        Some(body) => parse_body_sections(body)?,
        None => BTreeMap::new(),
    };
    let content = sections
        .get(&section.no)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "第 {} 章尚未确认，不能构建修订提示词；请先生成并确认章节稿。",
                section.no
            )
        })?;
    let prompt = build_revise_prompt(
        &task,
        &section,
        &content,
        request.paragraph_no,
        &instruction_kind,
        &request.instruction,
    )?;
    Ok(WritingSectionRevisePrompt {
        prompt,
        section_no: section.no,
        heading: section.heading,
        paragraph_no: request.paragraph_no,
        instruction_kind,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::material_store::{self, MaterialDraft};
    use crate::writing_outline::{OutlineConfirmRequest, OutlineSection};
    use crate::writing_store::{self, WritingTaskDraft};

    const NOW: f64 = 1_780_600_000.0;

    struct Fixture {
        base: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-section-{tag}-{}-{}",
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

        fn work(&self) -> &Path {
            &self.work
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn confirmed_material(root: &Path, id: &str, title: &str) -> String {
        let material = material_store::create_material(
            root,
            &MaterialDraft {
                material_id: Some(id.to_string()),
                title: title.to_string(),
                content: "虚构素材正文：本期完成三处试点，数据仅用于测试。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: format!("src-{id}"),
                source_quote: "完成三处试点。".to_string(),
                event_date: Some("2026-10-20".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        material.material_id
    }

    fn writing_task(root: &Path, material_ids: Vec<String>) -> writing_store::WritingTask {
        writing_store::create_writing_task(
            root,
            &WritingTaskDraft {
                writing_id: Some("w-607".to_string()),
                title: "虚构年度工作总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                writing_purpose: "向上级汇报本年度试点进展".to_string(),
                audience: "部门同事".to_string(),
                writing_period: "2026 年度".to_string(),
                topic: "试点推进".to_string(),
                requirements: "只写已确认事实\n三点以内".to_string(),
                expected_length: "1500 字".to_string(),
                writing_style: "简洁平实".to_string(),
                selected_material_ids: material_ids,
                selected_knowledge_ids: vec!["k-607".to_string()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn sample_outline(task: &writing_store::WritingTask) -> WritingOutline {
        let material_id = task
            .selected_material_ids
            .first()
            .cloned()
            .unwrap_or_default();
        WritingOutline {
            title: task.title.clone(),
            writing_purpose: task.writing_purpose.clone(),
            core_points: vec!["试点总体平稳".to_string()],
            sections: vec![
                OutlineSection {
                    no: 1,
                    heading: "工作概况".to_string(),
                    sub_headings: vec!["总体进展".to_string()],
                    focus: "概括本年度试点完成情况".to_string(),
                    material_ids: if material_id.is_empty() {
                        vec![]
                    } else {
                        vec![material_id]
                    },
                    knowledge_ids: vec!["k-607".to_string()],
                    expected_words: 300,
                },
                OutlineSection {
                    no: 2,
                    heading: "问题与不足".to_string(),
                    sub_headings: vec![],
                    focus: "只写素材中明确记录的问题".to_string(),
                    material_ids: vec![],
                    knowledge_ids: vec![],
                    expected_words: 200,
                },
            ],
        }
    }

    /// 建任务并确认大纲，返回写作中的任务与大纲。
    fn ready_fixture(tag: &str) -> (Fixture, writing_store::WritingTask, WritingOutline) {
        let fixture = Fixture::new(tag);
        let material_id = confirmed_material(fixture.work(), "m-607", "虚构试点素材");
        let task = writing_task(fixture.work(), vec![material_id]);
        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        let outline = sample_outline(&task);
        writing_outline::confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: outline.clone(),
                note: Some("虚构大纲确认".to_string()),
            },
            NOW + 3.0,
        )
        .unwrap();
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        (fixture, task, outline)
    }

    fn draft(section_no: u32, content: &str) -> SectionDraft {
        SectionDraft {
            section_no,
            content: content.to_string(),
            citations: vec![SectionCitation {
                source_kind: CitationSourceKind::Material,
                source_id: "m-607".to_string(),
                quote: "完成三处试点".to_string(),
            }],
            segments: vec![
                SectionSegment {
                    kind: "事实".to_string(),
                    text: "本期完成三处试点".to_string(),
                    quote: Some("完成三处试点".to_string()),
                },
                SectionSegment {
                    kind: "分析".to_string(),
                    text: "试点推进总体平稳".to_string(),
                    quote: None,
                },
            ],
        }
    }

    fn confirm(
        fixture: &Fixture,
        task: &writing_store::WritingTask,
        draft: SectionDraft,
        at: f64,
    ) -> WritingSectionConfirmed {
        confirm_section(
            fixture.work(),
            &SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft,
                note: Some("虚构章节确认".to_string()),
            },
            at,
        )
        .unwrap()
    }

    #[test]
    fn validate_rejects_empty_content_and_heading_lines() {
        let (fixture, task, outline) = ready_fixture("validate-empty");
        let mut empty = draft(1, "");
        empty.citations.clear();
        empty.segments.clear();
        let error = validate_section_draft(&empty, &task, &outline).unwrap_err();
        assert!(error.contains("不能为空"), "{error}");

        let mut heading = draft(1, "# 自造标题\n\n虚构正文");
        heading.citations.clear();
        heading.segments.clear();
        let error = validate_section_draft(&heading, &task, &outline).unwrap_err();
        assert!(error.contains("Markdown 标题行"), "{error}");
        drop(fixture);
    }

    #[test]
    fn validate_rejects_content_that_is_only_concatenated_quotes() {
        let (fixture, task, outline) = ready_fixture("validate-quotes");
        let copied = SectionDraft {
            section_no: 1,
            content: "完成三处试点".to_string(),
            citations: vec![SectionCitation {
                source_kind: CitationSourceKind::Material,
                source_id: "m-607".to_string(),
                quote: "完成三处试点".to_string(),
            }],
            segments: vec![],
        };
        let error = validate_section_draft(&copied, &task, &outline).unwrap_err();
        assert!(error.contains("引用原文的拼接"), "{error}");
        drop(fixture);
    }

    #[test]
    fn validate_rejects_unselected_material_and_knowledge_citations() {
        let (fixture, task, outline) = ready_fixture("validate-source");
        let mut bad_material = draft(1, "虚构正文段落。");
        bad_material.citations[0].source_id = "m-other".to_string();
        let error = validate_section_draft(&bad_material, &task, &outline).unwrap_err();
        assert!(error.contains("未加入素材包的素材编号"), "{error}");

        let mut bad_knowledge = draft(1, "虚构正文段落。");
        bad_knowledge.citations[0].source_kind = CitationSourceKind::Knowledge;
        bad_knowledge.citations[0].source_id = "k-other".to_string();
        let error = validate_section_draft(&bad_knowledge, &task, &outline).unwrap_err();
        assert!(error.contains("未加入素材包的知识编号"), "{error}");
        drop(fixture);
    }

    #[test]
    fn validate_rejects_segment_kind_outside_whitelist() {
        let (fixture, task, outline) = ready_fixture("validate-kind");
        let mut bad = draft(1, "虚构正文段落。");
        bad.segments[0].kind = "评价".to_string();
        let error = validate_section_draft(&bad, &task, &outline).unwrap_err();
        assert!(error.contains("四分法标注类型"), "{error}");
        assert!(error.contains("评价"), "{error}");
        drop(fixture);
    }

    #[test]
    fn parse_json_rejects_invalid_kind_and_unknown_fields() {
        let (fixture, task, outline) = ready_fixture("parse-json");
        let bad_kind = r#"{"section_no":1,"content":"虚构正文段落。","citations":[],"segments":[{"kind":"评论","text":"虚构"}]}"#;
        let error = parse_section_draft_json(bad_kind, &task, &outline).unwrap_err();
        assert!(error.contains("四分法标注类型"), "{error}");

        let unknown_field =
            r#"{"section_no":1,"content":"虚构正文段落。","citations":[],"segments":[],"extra":1}"#;
        let error = parse_section_draft_json(unknown_field, &task, &outline).unwrap_err();
        assert!(error.contains("不符合约定结构"), "{error}");
        drop(fixture);
    }

    #[test]
    fn body_roundtrip_keeps_sections_and_compose_marks_placeholder() {
        let (_fixture, _task, outline) = ready_fixture("roundtrip");
        let mut sections = BTreeMap::new();
        sections.insert(1, "第一段虚构内容。\n\n第二段虚构内容。".to_string());
        let markdown = render_body(&outline, &sections);
        let parsed = parse_body_sections(&markdown).unwrap();
        assert_eq!(parsed, sections);
        let full = compose_full_text(&outline, Some(&markdown));
        assert!(full.contains("## 1. 工作概况"), "{full}");
        assert!(full.contains("第一段虚构内容"), "{full}");
        assert!(full.contains("## 2. 问题与不足"), "{full}");
        assert!(full.contains(SECTION_PLACEHOLDER), "{full}");
    }

    #[test]
    fn scenario_five_sections_save_independently_and_compose() {
        let (fixture, task, outline) = ready_fixture("scenario5");
        let first = confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 5.0);
        assert_eq!(first.version, 2);
        assert_eq!(first.citations_added, 1);
        assert!(!first.all_sections_confirmed);
        let state1 = read_section_state(fixture.work(), &task.writing_id, 1).unwrap();
        let state2 = read_section_state(fixture.work(), &task.writing_id, 2).unwrap();
        assert!(state1.confirmed);
        assert!(state1.content.unwrap().contains("本期完成三处试点"));
        assert!(!state2.confirmed);
        assert!(state2.content.is_none());

        let body = document_store::read_body(fixture.work(), &task.writing_id)
            .unwrap()
            .unwrap();
        assert!(!body.contains("## 2. 问题与不足"), "未确认章节不得进正文文件");
        let full = compose_full_text_for(fixture.work(), &task.writing_id).unwrap();
        assert!(full.contains("本期完成三处试点"), "{full}");
        assert!(full.contains(SECTION_PLACEHOLDER), "{full}");
        let citations = writing_records::list_citations(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].section_no, 1);
        assert_eq!(citations[0].paragraph_no, 1);

        let second = confirm(&fixture, &task, draft(2, "仍有三项试点待补齐。"), NOW + 6.0);
        assert_eq!(second.status, writing_store::STATUS_FACT_CHECKING);
        assert!(second.all_sections_confirmed);
        let full = compose_full_text_for(fixture.work(), &task.writing_id).unwrap();
        assert!(!full.contains(SECTION_PLACEHOLDER), "{full}");
        assert!(full.contains("仍有三项试点待补齐"), "{full}");
        let citations = writing_records::list_citations(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[1].section_no, 2);
        drop(outline);
    }

    #[test]
    fn confirm_keeps_outline_order_when_later_section_confirmed_first() {
        let (fixture, task, _outline) = ready_fixture("order");
        confirm(&fixture, &task, draft(2, "第二章虚构内容。"), NOW + 5.0);
        confirm(&fixture, &task, draft(1, "第一章虚构内容。"), NOW + 6.0);
        let body = document_store::read_body(fixture.work(), &task.writing_id)
            .unwrap()
            .unwrap();
        let first = body.find("## 1. 工作概况").unwrap();
        let second = body.find("## 2. 问题与不足").unwrap();
        assert!(first < second, "{body}");
    }

    #[test]
    fn all_sections_confirmed_transitions_to_fact_checking_once() {
        let (fixture, task, _outline) = ready_fixture("transition");
        confirm(&fixture, &task, draft(1, "第一章虚构内容。"), NOW + 5.0);
        confirm(&fixture, &task, draft(2, "第二章虚构内容。"), NOW + 6.0);
        let after = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(after.status, writing_store::STATUS_FACT_CHECKING);
        let count = after
            .status_records
            .iter()
            .filter(|record| record.action.contains("写作中 → 待核验"))
            .count();
        assert_eq!(count, 1, "{:?}", after.status_records);
    }

    #[test]
    fn get_before_any_body_and_unknown_section_is_clear_error() {
        let (fixture, task, _outline) = ready_fixture("get");
        let state = read_section_state(fixture.work(), &task.writing_id, 1).unwrap();
        assert!(!state.confirmed);
        assert!(state.content.is_none());
        assert_eq!(state.version, 1, "只有大纲快照，正文尚未写入");
        let error = read_section_state(fixture.work(), &task.writing_id, 99).unwrap_err();
        assert!(error.contains("大纲中没有第 99 章"), "{error}");
    }

    #[test]
    fn generate_requires_writing_status_and_confirmed_outline() {
        let temp = Fixture::new("generate-status");
        let task = writing_task(temp.work(), vec![]);
        let error = generate_section_prompt(
            temp.work(),
            &SectionGenerateRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
            },
        )
        .unwrap_err();
        assert!(
            error.contains("只有「写作中」可以生成章节正文"),
            "{error}"
        );

        let task = writing_store::transition_writing_status(
            temp.work(),
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        let error = generate_section_prompt(
            temp.work(),
            &SectionGenerateRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
            },
        )
        .unwrap_err();
        assert!(error.contains("只有「写作中」可以生成章节正文"), "{error}");
    }

    #[test]
    fn generate_rejects_unconfirmed_material_in_section_package() {
        let fixture = Fixture::new("generate-pending");
        let material = material_store::create_material(
            fixture.work(),
            &MaterialDraft {
                material_id: Some("m-pending".to_string()),
                title: "虚构待审素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-pending".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task = writing_task(fixture.work(), vec![material.material_id.clone()]);
        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        let outline = sample_outline(&task);
        writing_outline::confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline,
                note: None,
            },
            NOW + 3.0,
        )
        .unwrap();
        let error = generate_section_prompt(
            fixture.work(),
            &SectionGenerateRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
            },
        )
        .unwrap_err();
        assert!(error.contains("不属于可信素材包"), "{error}");
    }

    #[test]
    fn prompt_contains_section_material_knowledge_summaries_and_prohibitions() {
        let (fixture, task, _outline) = ready_fixture("prompt");
        let prompt = generate_section_prompt(
            fixture.work(),
            &SectionGenerateRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
            },
        )
        .unwrap();
        assert_eq!(prompt.section_no, 1);
        assert_eq!(prompt.heading, "工作概况");
        assert_eq!(prompt.material_count, 1);
        assert_eq!(prompt.knowledge_count, 1);
        assert_eq!(prompt.previous_summary, "（无）");
        assert!(prompt.next_summary.contains("问题与不足"));
        let text = prompt.prompt;
        assert!(text.contains("本期完成三处试点"), "缺少本章素材正文");
        assert!(text.contains("m-607"), "缺少素材编号");
        assert!(text.contains("k-607"), "缺少知识编号");
        assert!(text.contains("四分法标注"));
        for kind in SEGMENT_KINDS {
            assert!(text.contains(kind), "缺少四分法标签 {kind}");
        }
        assert!(text.contains("不虚构工作成果、领导讲话、政策文件"));
        assert!(text.contains("计划、打算、拟办事项不得写成已经完成"));
        assert!(text.contains("相关性不得写成因果关系"));
        assert!(text.contains("不得空洞凑篇幅"));
        assert!(text.contains("不得重复堆砌"));
        assert!(text.contains("上一章摘要：（无）"));
        assert!(text.contains("下一章摘要：问题与不足"));
    }

    #[test]
    fn revise_prompt_targets_paragraph_and_apply_keeps_original_snapshot() {
        let (fixture, task, _outline) = ready_fixture("revise");
        confirm(
            &fixture,
            &task,
            draft(1, "本期完成三处试点。\n\n仍需补齐三项试点。"),
            NOW + 5.0,
        );
        let prompt = revise_section_prompt(
            fixture.work(),
            &SectionReviseRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
                paragraph_no: Some(2),
                instruction_kind: "精简".to_string(),
                instruction: "第二段压缩到一句话".to_string(),
            },
        )
        .unwrap();
        assert_eq!(prompt.section_no, 1);
        assert_eq!(prompt.paragraph_no, Some(2));
        assert_eq!(prompt.instruction_kind, "精简");
        assert!(prompt.prompt.contains("第 2 段：仍需补齐三项试点。"), "{}", prompt.prompt);
        assert!(prompt.prompt.contains("精简"));
        assert!(prompt.prompt.contains("第二段压缩到一句话"));

        let revised = confirm(
            &fixture,
            &task,
            draft(1, "本期完成三处试点。\n\n三项试点仍待补齐。"),
            NOW + 6.0,
        );
        assert_eq!(revised.version, 3);
        let version2 = document_store::read_version(fixture.work(), &task.writing_id, 2)
            .unwrap()
            .content;
        assert!(version2.contains("仍需补齐三项试点。"), "{version2}");
        let current = read_section_state(fixture.work(), &task.writing_id, 1)
            .unwrap()
            .content
            .unwrap();
        assert!(current.contains("三项试点仍待补齐。"));
    }

    #[test]
    fn revise_rejects_bad_kind_missing_section_and_out_of_range_paragraph() {
        let (fixture, task, _outline) = ready_fixture("revise-reject");
        confirm(&fixture, &task, draft(1, "第一段虚构内容。\n\n第二段虚构内容。"), NOW + 5.0);
        let base = SectionReviseRequest {
            writing_id: task.writing_id.clone(),
            section_no: 1,
            paragraph_no: None,
            instruction_kind: "精简".to_string(),
            instruction: String::new(),
        };
        let error = revise_section_prompt(
            fixture.work(),
            &SectionReviseRequest {
                instruction_kind: "重写".to_string(),
                ..base.clone()
            },
        )
        .unwrap_err();
        assert!(error.contains("不在允许范围"), "{error}");

        let error = revise_section_prompt(
            fixture.work(),
            &SectionReviseRequest {
                section_no: 2,
                ..base.clone()
            },
        )
        .unwrap_err();
        assert!(error.contains("尚未确认"), "{error}");

        let error = revise_section_prompt(
            fixture.work(),
            &SectionReviseRequest {
                paragraph_no: Some(99),
                ..base
            },
        )
        .unwrap_err();
        assert!(error.contains("段落序号 99 不存在"), "{error}");
    }
}
