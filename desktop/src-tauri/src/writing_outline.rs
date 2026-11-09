//! 大纲生成与确认（OD-WT-606）。
//!
//! 分工照 604 writing_parse、605 writing_collect 先例：
//!   * 宿主组大纲提示词（写作需求 + 已确认素材包 + 类型模板），模型调用仍在界面侧；
//!   * 模型输出只是候选，本模块提供 JSON 校验；用户可编辑后经 confirm 落盘；
//!   * outline.md 由 602 document_store 唯一写入口写入并留存版本快照；
//!   * 状态迁移只走 601 writing_store 的合法性表，确认动作使用
//!     `is_outline_confirmed=true` 才能进入「写作中」。
//!
//! 类型模板落仓库外的 `config/writing-templates.v1.json`（照 flow_pack 先例）。
//! 文件不存在时直接使用编译期内嵌默认值，不创建、不覆盖任何文件
//! （照 availability 的零写入先例）。模板文件存在即必须通过结构与文本校验，
//! 错误配置整包拒绝，不悄悄回退默认。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document_store::{self, SnapshotSpec, VersionScope, VersionSource};
use crate::material_store::{self, WritingMaterial, REVIEW_CONFIRMED};
use crate::writing_store::{self, StatusRecord, WritingTask, DOCUMENT_TYPES};

/// 类型模板 schema 标识。版本不符一律拒绝。
pub const TEMPLATES_SCHEMA: &str = "omnidesk.writing-templates.v1";
/// 类型模板文件名（位于应用数据根的 config/ 下）。
pub const TEMPLATES_FILE: &str = "writing-templates.v1.json";
/// 仓库内默认类型模板。编译期嵌入，作为无配置文件时的默认值。
pub const DEFAULT_TEMPLATES_JSON: &str =
    include_str!("../../../samples/workflows/writing-templates.v1.json");

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_PURPOSE_CHARS: usize = 400;
pub const MAX_CORE_POINTS: usize = 8;
pub const MAX_CORE_POINT_CHARS: usize = 200;
pub const MAX_SECTIONS: usize = 40;
pub const MAX_HEADING_CHARS: usize = 100;
pub const MAX_FOCUS_CHARS: usize = 400;
pub const MAX_SUB_HEADINGS: usize = 8;
pub const MAX_SUB_HEADING_CHARS: usize = 80;
pub const MAX_EXPECTED_WORDS: u32 = 100_000;
pub const MAX_NOTE_CHARS: usize = 200;

const MAX_TEMPLATES: usize = 12;
const MAX_TEMPLATE_SECTIONS: usize = 20;
const MAX_TEMPLATE_TEXT_CHARS: usize = 400;
const MAX_TEMPLATE_HEADING_CHARS: usize = 100;
const MAX_TEMPLATE_SUB_CHARS: usize = 80;
const MAX_DOCUMENT_TYPES_PER_TEMPLATE: usize = 12;
const MAX_PROMPT_MATERIAL_CHARS: usize = 2_000;
const MAX_PROMPT_QUOTE_CHARS: usize = 300;

/// 单个章节。`no` 是稳定编号，供分章写作与界面定位使用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineSection {
    pub no: u32,
    pub heading: String,
    #[serde(default)]
    pub sub_headings: Vec<String>,
    pub focus: String,
    #[serde(default)]
    pub material_ids: Vec<String>,
    #[serde(default)]
    pub knowledge_ids: Vec<String>,
    pub expected_words: u32,
}

/// 可编辑的大纲 JSON。字段名直接作为模型输出契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingOutline {
    pub title: String,
    pub writing_purpose: String,
    pub core_points: Vec<String>,
    pub sections: Vec<OutlineSection>,
}

/// 类型模板中的一个推荐章节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateSection {
    pub heading: String,
    #[serde(default)]
    pub sub_headings: Vec<String>,
    pub focus: String,
}

/// 一套文稿类型模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingTemplate {
    pub key: String,
    pub label: String,
    pub description: String,
    pub document_types: Vec<String>,
    pub sections: Vec<TemplateSection>,
}

/// 类型模板文件整体结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingTemplates {
    pub schema: String,
    pub templates: Vec<WritingTemplate>,
}

/// 已加载模板及其来源。
#[derive(Debug, Clone)]
pub struct LoadedTemplates {
    pub templates: WritingTemplates,
    pub path: String,
    /// true 表示 config 下没有文件，本次使用编译期内嵌默认值（零写入）。
    pub from_default: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OutlineGenerateRequest {
    pub writing_id: String,
}

/// 大纲提示词回执。模型调用仍由界面侧提交，本命令不写文件。
#[derive(Debug, Clone, Serialize)]
pub struct WritingOutlinePrompt {
    pub prompt: String,
    pub template_key: String,
    pub template_label: String,
    pub material_count: usize,
    pub knowledge_count: usize,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineConfirmRequest {
    pub writing_id: String,
    pub outline: WritingOutline,
    #[serde(default)]
    pub note: Option<String>,
}

/// 已确认大纲的读取/确认回执。
#[derive(Debug, Clone, Serialize)]
pub struct WritingOutlineConfirmed {
    pub writing_id: String,
    pub status: String,
    pub outline: WritingOutline,
    pub markdown: String,
    pub version: u64,
    pub updated_at: String,
    pub status_records: Vec<StatusRecord>,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

impl WritingTemplates {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TEMPLATES_SCHEMA {
            return Err(format!(
                "不支持的类型模板版本 {:?}；当前只接受 {:?}",
                self.schema, TEMPLATES_SCHEMA
            ));
        }
        if self.templates.is_empty() {
            return Err("类型模板不能为空。".to_string());
        }
        if self.templates.len() > MAX_TEMPLATES {
            return Err(format!("类型模板最多 {MAX_TEMPLATES} 套。"));
        }
        let mut keys = BTreeSet::new();
        let mut covered_types = BTreeSet::new();
        for (index, template) in self.templates.iter().enumerate() {
            let field = format!("templates[{index}]");
            let key = validate_template_text(&format!("{field}.key"), &template.key, 80)?;
            if !keys.insert(key.clone()) {
                return Err(format!("类型模板 key 重复：{key}"));
            }
            validate_template_text(&format!("{field}.label"), &template.label, 80)?;
            validate_template_text(
                &format!("{field}.description"),
                &template.description,
                MAX_TEMPLATE_TEXT_CHARS,
            )?;
            if template.document_types.is_empty() {
                return Err(format!("类型模板「{key}」没有绑定任何文稿类型。"));
            }
            if template.document_types.len() > MAX_DOCUMENT_TYPES_PER_TEMPLATE {
                return Err(format!(
                    "类型模板「{key}」绑定的文稿类型超过 {MAX_DOCUMENT_TYPES_PER_TEMPLATE} 个。"
                ));
            }
            for value in &template.document_types {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(format!("类型模板「{key}」的文稿类型不能为空。"));
                }
                if !DOCUMENT_TYPES.contains(&trimmed) {
                    return Err(format!(
                        "类型模板「{key}」绑定了未知文稿类型：{trimmed}"
                    ));
                }
                if !covered_types.insert(trimmed.to_string()) {
                    return Err(format!(
                        "文稿类型「{trimmed}」被多套类型模板重复绑定，无法确定使用哪一套。"
                    ));
                }
            }
            if template.sections.is_empty() {
                return Err(format!("类型模板「{key}」的章节骨架不能为空。"));
            }
            if template.sections.len() > MAX_TEMPLATE_SECTIONS {
                return Err(format!(
                    "类型模板「{key}」的章节骨架最多 {MAX_TEMPLATE_SECTIONS} 节。"
                ));
            }
            for (section_index, section) in template.sections.iter().enumerate() {
                let section_field = format!("{field}.sections[{section_index}]");
                validate_template_text(
                    &format!("{section_field}.heading"),
                    &section.heading,
                    MAX_TEMPLATE_HEADING_CHARS,
                )?;
                validate_template_text(
                    &format!("{section_field}.focus"),
                    &section.focus,
                    MAX_TEMPLATE_TEXT_CHARS,
                )?;
                if section.sub_headings.len() > MAX_SUB_HEADINGS {
                    return Err(format!(
                        "类型模板「{key}」章节「{}」的小节超过 {MAX_SUB_HEADINGS} 个。",
                        section.heading.trim()
                    ));
                }
                for value in &section.sub_headings {
                    validate_template_text(
                        &format!("{section_field}.sub_headings"),
                        value,
                        MAX_TEMPLATE_SUB_CHARS,
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn validate_template_text(name: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("类型模板字段 {name} 不能为空"));
    }
    if trimmed.chars().count() > max_chars {
        return Err(format!(
            "类型模板字段 {name} 超过 {max_chars} 个字符"
        ));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(format!("类型模板字段 {name} 不得含换行"));
    }
    let lowered = trimmed.to_lowercase();
    for needle in ["http://", "https://", "file://", "javascript:", "<script", "<iframe"] {
        if lowered.contains(needle) {
            return Err(format!(
                "类型模板字段 {name} 含不允许的内容（{needle}）；模板只能是纯文本"
            ));
        }
    }
    Ok(trimmed.to_string())
}

/// 解析内嵌默认模板。解析失败属于构建缺陷，测试会直接拦住。
pub fn default_templates() -> WritingTemplates {
    let templates: WritingTemplates =
        serde_json::from_str(DEFAULT_TEMPLATES_JSON).expect("内嵌默认类型模板必须是合法 JSON");
    templates
        .validate()
        .expect("内嵌默认类型模板必须通过结构与文本校验");
    templates
}

pub fn templates_path(config_dir: &Path) -> PathBuf {
    config_dir.join(TEMPLATES_FILE)
}

/// 加载类型模板：文件不存在时使用内嵌默认值，零写入；存在则读取并校验。
pub fn load_templates(config_dir: &Path) -> Result<LoadedTemplates, String> {
    if !config_dir.is_absolute() {
        return Err(format!(
            "类型模板目录必须是绝对路径：{}",
            config_dir.display()
        ));
    }
    let path = templates_path(config_dir);
    match fs::read_to_string(&path) {
        Ok(text) => {
            let templates: WritingTemplates = serde_json::from_str(&text).map_err(|e| {
                format!(
                    "类型模板格式错误（{}）：{}。该配置已被拒绝，未使用任何默认模板替代。",
                    path.display(),
                    e
                )
            })?;
            templates.validate().map_err(|e| {
                format!(
                    "类型模板内容无效（{}）：{}。该配置已被拒绝，未使用任何默认模板替代。",
                    path.display(),
                    e
                )
            })?;
            Ok(LoadedTemplates {
                templates,
                path: path.display().to_string(),
                from_default: false,
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LoadedTemplates {
            templates: default_templates(),
            path: path.display().to_string(),
            from_default: true,
        }),
        Err(error) => Err(format!(
            "读取类型模板失败（{}）：{}",
            path.display(),
            error
        )),
    }
}

/// 按文稿类型选择模板。未知类型给出可修正的错误，不做静默兜底。
pub fn template_for_document_type<'a>(
    templates: &'a WritingTemplates,
    document_type: &str,
) -> Result<&'a WritingTemplate, String> {
    let document_type = document_type.trim();
    let available: Vec<&str> = templates
        .templates
        .iter()
        .map(|template| template.key.as_str())
        .collect();
    templates
        .templates
        .iter()
        .find(|template| {
            template.document_types.iter().any(|value| value.trim() == document_type)
        })
        .ok_or_else(|| {
            format!(
                "没有为文稿类型「{document_type}」配置大纲模板；可在 {TEMPLATES_FILE} 中为它指定类型。当前模板：{}",
                available.join("、")
            )
        })
}

fn validate_outline_text(
    name: &str,
    value: &str,
    max_chars: usize,
) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("大纲字段 {name} 不能为空"));
    }
    if trimmed.chars().count() > max_chars {
        return Err(format!(
            "大纲字段 {name} 超过 {max_chars} 个字符"
        ));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(format!("大纲字段 {name} 不得含换行"));
    }
    Ok(trimmed.to_string())
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

/// 去掉首尾空白、空条目与重复 ID。校验仍按规范化后的内容执行。
pub fn normalize_outline(outline: &WritingOutline) -> WritingOutline {
    WritingOutline {
        title: outline.title.trim().to_string(),
        writing_purpose: outline.writing_purpose.trim().to_string(),
        core_points: outline
            .core_points
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        sections: outline
            .sections
            .iter()
            .map(|section| OutlineSection {
                no: section.no,
                heading: section.heading.trim().to_string(),
                sub_headings: section
                    .sub_headings
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect(),
                focus: section.focus.trim().to_string(),
                material_ids: normalize_ids(&section.material_ids),
                knowledge_ids: normalize_ids(&section.knowledge_ids),
                expected_words: section.expected_words,
            })
            .collect(),
    }
}

/// 只校验结构，不依赖具体任务的素材包。读取已确认大纲时使用。
pub fn validate_outline_shape(outline: &WritingOutline) -> Result<(), String> {
    validate_outline_text("title", &outline.title, MAX_TITLE_CHARS)?;
    validate_outline_text(
        "writing_purpose",
        &outline.writing_purpose,
        MAX_PURPOSE_CHARS,
    )?;
    if outline.core_points.is_empty() {
        return Err("大纲的 core_points 不能为空。".to_string());
    }
    if outline.core_points.len() > MAX_CORE_POINTS {
        return Err(format!("大纲的 core_points 最多 {MAX_CORE_POINTS} 条。"));
    }
    for (index, point) in outline.core_points.iter().enumerate() {
        validate_outline_text(
            &format!("core_points[{index}]"),
            point,
            MAX_CORE_POINT_CHARS,
        )?;
    }
    if outline.sections.is_empty() {
        return Err("大纲的 sections 不能为空。".to_string());
    }
    if outline.sections.len() > MAX_SECTIONS {
        return Err(format!("大纲最多 {MAX_SECTIONS} 章。"));
    }
    let mut seen_no = BTreeSet::new();
    for (index, section) in outline.sections.iter().enumerate() {
        let field = format!("sections[{index}]");
        if section.no == 0 {
            return Err(format!("大纲字段 {field}.no 必须是正整数"));
        }
        if !seen_no.insert(section.no) {
            return Err(format!("大纲章节编号重复：{}", section.no));
        }
        validate_outline_text(
            &format!("{field}.heading"),
            &section.heading,
            MAX_HEADING_CHARS,
        )?;
        validate_outline_text(&format!("{field}.focus"), &section.focus, MAX_FOCUS_CHARS)?;
        if section.expected_words == 0 {
            return Err(format!(
                "大纲章节「{}」的 expected_words 必须是正数",
                section.heading.trim()
            ));
        }
        if section.expected_words > MAX_EXPECTED_WORDS {
            return Err(format!(
                "大纲章节「{}」的 expected_words 超过 {MAX_EXPECTED_WORDS}",
                section.heading.trim()
            ));
        }
        if section.sub_headings.len() > MAX_SUB_HEADINGS {
            return Err(format!(
                "大纲章节「{}」的小节超过 {MAX_SUB_HEADINGS} 个",
                section.heading.trim()
            ));
        }
        for value in &section.sub_headings {
            validate_outline_text(
                &format!("{field}.sub_headings"),
                value,
                MAX_SUB_HEADING_CHARS,
            )?;
        }
    }
    Ok(())
}

/// 校验结构并确认所有素材/知识引用都在任务已选集合内。
pub fn validate_outline(outline: &WritingOutline, task: &WritingTask) -> Result<(), String> {
    validate_outline_shape(outline)?;
    for section in &outline.sections {
        for id in &section.material_ids {
            if !task.selected_material_ids.iter().any(|value| value == id) {
                return Err(format!(
                    "大纲章节「{}」引用了未加入素材包的素材编号：{id}",
                    section.heading.trim()
                ));
            }
        }
        for id in &section.knowledge_ids {
            if !task.selected_knowledge_ids.iter().any(|value| value == id) {
                return Err(format!(
                    "大纲章节「{}」引用了未加入素材包的知识编号：{id}",
                    section.heading.trim()
                ));
            }
        }
    }
    Ok(())
}

/// 校验模型输出的大纲 JSON。允许 Markdown 代码块包裹；任何失败都不落盘。
pub fn parse_outline_json(
    result_text: &str,
    task: &WritingTask,
) -> Result<WritingOutline, String> {
    let text = result_text.trim();
    if text.is_empty() {
        return Err("大纲生成结果为空。".to_string());
    }
    let value = crate::knowledge_ingest::parse_result_text(text)
        .map_err(|e| format!("大纲生成结果不是合法 JSON：{e}"))?;
    let raw: WritingOutline = serde_json::from_value(value)
        .map_err(|e| format!("大纲生成结果不符合约定结构：{e}"))?;
    let outline = normalize_outline(&raw);
    validate_outline(&outline, task)?;
    Ok(outline)
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

/// 组大纲提示词：写作需求 + 类型模板 + 已确认素材包 + 输出契约。
pub fn build_outline_prompt(
    task: &WritingTask,
    template: &WritingTemplate,
    materials: &[WritingMaterial],
) -> String {
    let mut prompt = String::new();
    let _ = writeln!(
        prompt,
        "你是中文公文写作助手。请为下面的文稿拟定章节大纲。只输出 JSON，不输出解释性文字。"
    );
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 一、写作需求");
    let _ = writeln!(prompt, "- 文稿类型：{}", task.document_type);
    let _ = writeln!(prompt, "- 标题：{}", task.title);
    let _ = writeln!(prompt, "- 写作目的：{}", task.writing_purpose);
    let _ = writeln!(prompt, "- 目标受众：{}", task.audience);
    let _ = writeln!(prompt, "- 时间范围：{}", task.writing_period);
    let _ = writeln!(prompt, "- 主题：{}", task.topic);
    let _ = writeln!(prompt, "- 预计篇幅：{}", task.expected_length);
    let _ = writeln!(prompt, "- 写作风格：{}", task.writing_style);
    let requirements: Vec<&str> = task
        .requirements
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if requirements.is_empty() {
        let _ = writeln!(prompt, "- 写作要求：（未填写）");
    } else {
        let _ = writeln!(prompt, "- 写作要求：");
        for line in requirements {
            let _ = writeln!(prompt, "  - {line}");
        }
    }
    let _ = writeln!(prompt);
    let _ = writeln!(
        prompt,
        "## 二、类型模板：{}（{}）",
        template.label, template.key
    );
    let _ = writeln!(prompt, "模板说明：{}", template.description);
    let _ = writeln!(prompt, "推荐章节骨架：");
    for (index, section) in template.sections.iter().enumerate() {
        let _ = writeln!(
            prompt,
            "{}. {} — 写作重点：{}",
            index + 1,
            section.heading,
            section.focus
        );
        if !section.sub_headings.is_empty() {
            let _ = writeln!(
                prompt,
                "   小节：{}",
                section.sub_headings.join("、")
            );
        }
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 三、已确认素材包");
    if materials.is_empty() {
        let _ = writeln!(
            prompt,
            "本次没有选中素材，只能引用下面列出的知识依据编号。"
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
    let _ = writeln!(prompt, "## 四、知识依据编号");
    if task.selected_knowledge_ids.is_empty() {
        let _ = writeln!(prompt, "（无）");
    } else {
        for id in &task.selected_knowledge_ids {
            let _ = writeln!(prompt, "- {id}");
        }
        let _ = writeln!(
            prompt,
            "知识正文未在本提示词内展开，只能作为编号引用，不得编造内容。"
        );
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 五、输出要求");
    prompt.push_str(
        "只输出一个 JSON 对象，字段固定如下，不得增加字段：\n\
{\"title\":\"标题\",\"writing_purpose\":\"写作目的\",\"core_points\":[\"核心观点\"],\"sections\":[{\"no\":1,\"heading\":\"章节标题\",\"sub_headings\":[\"小节标题\"],\"focus\":\"本章写作重点\",\"material_ids\":[\"素材编号\"],\"knowledge_ids\":[\"知识编号\"],\"expected_words\":1200}]}\n",
    );
    let _ = writeln!(prompt, "规则：");
    let _ = writeln!(prompt, "1. sections 不得为空；no 为正整数且不得重复。");
    let _ = writeln!(prompt, "2. heading、focus 不得为空；sub_headings 可为空数组。");
    let _ = writeln!(
        prompt,
        "3. material_ids 只能取上面列出的素材编号，knowledge_ids 只能取上面列出的知识编号；没有可引用项时用空数组。"
    );
    let _ = writeln!(prompt, "4. expected_words 为正整数，逐章给出预计字数。");
    let _ = writeln!(prompt, "5. 每章必须写清写作重点，并尽量与推荐骨架对应。");
    let _ = writeln!(
        prompt,
        "6. 不得虚构未提供的成果、数据、机构、人名、文号；计划不得写成已完成；相关性不得写成因果；不得空洞凑篇幅。"
    );
    prompt
}

/// 组装提示词前的宿主校验：任务状态、素材包非空、素材仍为已确认。
pub fn generate_outline_prompt(
    root: &Path,
    config_dir: &Path,
    request: &OutlineGenerateRequest,
) -> Result<WritingOutlinePrompt, String> {
    let writing_id = request.writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let task = writing_store::read_writing_task(root, writing_id)?;
    if !matches!(
        task.status.as_str(),
        writing_store::STATUS_COLLECTING_MATERIALS | writing_store::STATUS_OUTLINE_PENDING
    ) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」或「{}」可以生成大纲。",
            task.status,
            writing_store::STATUS_COLLECTING_MATERIALS,
            writing_store::STATUS_OUTLINE_PENDING
        ));
    }
    if task.selected_material_ids.is_empty() && task.selected_knowledge_ids.is_empty() {
        return Err(
            "素材包为空：请先在素材收集中采纳至少一条素材或知识依据，再生成大纲。".to_string(),
        );
    }
    let mut materials = Vec::new();
    for id in &task.selected_material_ids {
        let material = material_store::read_material(root, id)
            .map_err(|error| format!("读取素材包中的素材 {id} 失败：{error}"))?;
        if material.review_status != REVIEW_CONFIRMED {
            return Err(format!(
                "素材「{}」当前审核状态为「{}」，不属于可信素材包；请重新确认或移出素材包。",
                id, material.review_status
            ));
        }
        materials.push(material);
    }
    let loaded = load_templates(config_dir)?;
    let template = template_for_document_type(&loaded.templates, &task.document_type)?;
    let prompt = build_outline_prompt(&task, template, &materials);
    Ok(WritingOutlinePrompt {
        prompt,
        template_key: template.key.clone(),
        template_label: template.label.clone(),
        material_count: materials.len(),
        knowledge_count: task.selected_knowledge_ids.len(),
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

/// 把大纲渲染成 outline.md 的正文。格式固定，供 get 确定性反解。
pub fn render_outline_markdown(outline: &WritingOutline) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}", outline.title.trim());
    let _ = writeln!(out);
    let _ = writeln!(out, "写作目的：{}", outline.writing_purpose.trim());
    let _ = writeln!(out);
    let _ = writeln!(out, "## 核心观点");
    for point in &outline.core_points {
        let _ = writeln!(out, "- {}", point.trim());
    }
    for section in &outline.sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "## {}. {}", section.no, section.heading.trim());
        let _ = writeln!(out, "写作重点：{}", section.focus.trim());
        let _ = writeln!(out, "预计字数：{}", section.expected_words);
        let _ = writeln!(out, "小节：");
        if section.sub_headings.is_empty() {
            let _ = writeln!(out, "- （无）");
        } else {
            for value in &section.sub_headings {
                let _ = writeln!(out, "- {}", value.trim());
            }
        }
        let _ = writeln!(out, "素材编号：");
        if section.material_ids.is_empty() {
            let _ = writeln!(out, "- （无）");
        } else {
            for value in &section.material_ids {
                let _ = writeln!(out, "- {}", value.trim());
            }
        }
        let _ = writeln!(out, "知识编号：");
        if section.knowledge_ids.is_empty() {
            let _ = writeln!(out, "- （无）");
        } else {
            for value in &section.knowledge_ids {
                let _ = writeln!(out, "- {}", value.trim());
            }
        }
    }
    out.trim_end().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Preamble,
    Core,
    Section,
    Sub,
    Material,
    Knowledge,
}

fn parse_bullet(mode: ParseMode, value: &str) -> Option<(ParseMode, String)> {
    let value = value.trim();
    if value.is_empty() || value == "（无）" {
        return None;
    }
    match mode {
        ParseMode::Core | ParseMode::Sub | ParseMode::Material | ParseMode::Knowledge => {
            Some((mode, value.to_string()))
        }
        _ => None,
    }
}

/// 反解 outline.md。格式由本模块写入，解析失败说明文件被外部改动。
pub fn parse_outline_markdown(markdown: &str) -> Result<WritingOutline, String> {
    let mut title: Option<String> = None;
    let mut purpose: Option<String> = None;
    let mut core_points: Vec<String> = Vec::new();
    let mut sections: Vec<OutlineSection> = Vec::new();
    let mut current: Option<OutlineSection> = None;
    let mut mode = ParseMode::Preamble;

    for raw_line in markdown.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            if title.is_some() {
                return Err("大纲标题重复。".to_string());
            }
            title = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            if current.is_some() {
                sections.push(current.take().unwrap());
            }
            if rest.trim() == "核心观点" {
                mode = ParseMode::Core;
                continue;
            }
            let (no_text, heading) = rest
                .split_once(". ")
                .ok_or_else(|| format!("大纲章节标题格式不正确：{line}"))?;
            let no: u32 = no_text
                .trim()
                .parse()
                .map_err(|_| format!("大纲章节编号不是正整数：{no_text}"))?;
            current = Some(OutlineSection {
                no,
                heading: heading.trim().to_string(),
                sub_headings: Vec::new(),
                focus: String::new(),
                material_ids: Vec::new(),
                knowledge_ids: Vec::new(),
                expected_words: 0,
            });
            mode = ParseMode::Section;
            continue;
        }
        if let Some(rest) = line.strip_prefix("写作目的：") {
            purpose = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("写作重点：") {
            let section = current
                .as_mut()
                .ok_or_else(|| "写作重点出现在任何章节之前。".to_string())?;
            section.focus = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("预计字数：") {
            let section = current
                .as_mut()
                .ok_or_else(|| "预计字数出现在任何章节之前。".to_string())?;
            section.expected_words = rest
                .trim()
                .parse()
                .map_err(|_| format!("预计字数不是正整数：{}", rest.trim()))?;
            continue;
        }
        if line == "小节：" {
            mode = ParseMode::Sub;
            continue;
        }
        if line == "素材编号：" {
            mode = ParseMode::Material;
            continue;
        }
        if line == "知识编号：" {
            mode = ParseMode::Knowledge;
            continue;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            match mode {
                ParseMode::Core => {
                    if let Some((_, value)) = parse_bullet(mode, rest) {
                        core_points.push(value);
                    }
                }
                ParseMode::Sub | ParseMode::Material | ParseMode::Knowledge => {
                    if let Some((_, value)) = parse_bullet(mode, rest) {
                        let section = current
                            .as_mut()
                            .ok_or_else(|| "列表条目出现在任何章节之前。".to_string())?;
                        match mode {
                            ParseMode::Sub => section.sub_headings.push(value),
                            ParseMode::Material => section.material_ids.push(value),
                            ParseMode::Knowledge => section.knowledge_ids.push(value),
                            _ => {}
                        }
                    }
                }
                _ => return Err(format!("未知的大纲条目：{line}")),
            }
            continue;
        }
        return Err(format!("无法识别的大纲行：{line}"));
    }
    if let Some(section) = current.take() {
        sections.push(section);
    }
    let outline = normalize_outline(&WritingOutline {
        title: title.ok_or_else(|| "大纲缺少标题。".to_string())?,
        writing_purpose: purpose.ok_or_else(|| "大纲缺少写作目的。".to_string())?,
        core_points,
        sections,
    });
    validate_outline_shape(&outline)?;
    Ok(outline)
}

fn validate_note(note: Option<&str>) -> Result<Option<String>, String> {
    let Some(note) = note else { return Ok(None) };
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("大纲确认说明最多 {MAX_NOTE_CHARS} 字。"));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("大纲确认说明不得含换行。".to_string());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("大纲确认说明含控制字符。".to_string());
    }
    Ok(Some(trimmed.to_string()))
}

/// 用户确认大纲：校验 → outline.md + 快照 → 素材收集中→大纲待确认→写作中。
pub fn confirm_outline(
    root: &Path,
    request: &OutlineConfirmRequest,
    now: f64,
) -> Result<WritingOutlineConfirmed, String> {
    let writing_id = request.writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let mut task = writing_store::read_writing_task(root, writing_id)?;
    if !matches!(
        task.status.as_str(),
        writing_store::STATUS_COLLECTING_MATERIALS | writing_store::STATUS_OUTLINE_PENDING
    ) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」或「{}」可以确认大纲。",
            task.status,
            writing_store::STATUS_COLLECTING_MATERIALS,
            writing_store::STATUS_OUTLINE_PENDING
        ));
    }
    let outline = normalize_outline(&request.outline);
    validate_outline(&outline, &task)?;
    let note = validate_note(request.note.as_deref())?;
    if task.status == writing_store::STATUS_COLLECTING_MATERIALS
        && !writing_store::is_allowed_transition(
            writing_store::STATUS_COLLECTING_MATERIALS,
            writing_store::STATUS_OUTLINE_PENDING,
            false,
        )
    {
        return Err("状态表不再允许从素材收集中进入大纲待确认。".to_string());
    }
    let markdown = render_outline_markdown(&outline);
    let spec = SnapshotSpec {
        scope: VersionScope::Outline,
        source: VersionSource::User,
        label: "outline-confirmed",
        note: "大纲确认",
    };
    let entry = document_store::save_with_snapshot(root, writing_id, &spec, &markdown, now)
        .map_err(|error| format!("大纲写入失败：{error}"))?;
    let reason = note.as_deref().unwrap_or("确认大纲");
    if task.status == writing_store::STATUS_COLLECTING_MATERIALS {
        writing_store::transition_writing_status(
            root,
            writing_id,
            writing_store::STATUS_OUTLINE_PENDING,
            false,
            Some(reason),
            now,
        )?;
    }
    task = writing_store::transition_writing_status(
        root,
        writing_id,
        writing_store::STATUS_WRITING,
        true,
        Some(reason),
        now,
    )?;
    Ok(WritingOutlineConfirmed {
        writing_id: task.writing_id.clone(),
        status: task.status.clone(),
        outline,
        markdown,
        version: entry.version,
        updated_at: task.updated_at.clone(),
        status_records: task.status_records,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

/// 读取已确认大纲。没有 outline.md 时明确报错，不返回默认大纲。
pub fn read_confirmed_outline(
    root: &Path,
    writing_id: &str,
) -> Result<WritingOutlineConfirmed, String> {
    let writing_id = writing_id.trim();
    if writing_id.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    let task = writing_store::read_writing_task(root, writing_id)?;
    let markdown = document_store::read_outline(root, writing_id)?
        .ok_or_else(|| "尚未确认大纲：当前没有 outline.md。".to_string())?;
    let outline = parse_outline_markdown(&markdown)?;
    let manifest = document_store::read_manifest(root, writing_id)?;
    Ok(WritingOutlineConfirmed {
        writing_id: task.writing_id.clone(),
        status: task.status.clone(),
        outline,
        markdown,
        version: manifest.current_version,
        updated_at: manifest.updated_at,
        status_records: task.status_records,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_store::{MaterialDraft, ACTION_CONFIRM};
    use crate::writing_store::{WritingTaskDraft, STATUS_COLLECTING_MATERIALS, STATUS_OUTLINE_PENDING};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_400_000.0;

    struct Fixture {
        base: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-outline-{tag}-{}-{}",
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

        fn config(&self) -> &Path {
            &self.config
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
                source_quote: "虚构来源原文：完成三处试点。".to_string(),
                event_date: Some("2026-10-20".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(root, &material.material_id, ACTION_CONFIRM, None, NOW + 1.0)
            .unwrap();
        material.material_id
    }

    fn writing_task(
        root: &Path,
        material_ids: Vec<String>,
        knowledge_ids: Vec<String>,
    ) -> WritingTask {
        writing_store::create_writing_task(
            root,
            &WritingTaskDraft {
                writing_id: Some("w-606".to_string()),
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
                selected_knowledge_ids: knowledge_ids,
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn to_status(root: &Path, id: &str, status: &str) -> WritingTask {
        let mut task = writing_store::read_writing_task(root, id).unwrap();
        if task.status == writing_store::STATUS_NEEDS_ORGANIZING {
            task = writing_store::transition_writing_status(
                root,
                id,
                STATUS_COLLECTING_MATERIALS,
                false,
                None,
                NOW + 2.0,
            )
            .unwrap();
        }
        if task.status == STATUS_COLLECTING_MATERIALS && status == STATUS_OUTLINE_PENDING {
            task = writing_store::transition_writing_status(
                root,
                id,
                STATUS_OUTLINE_PENDING,
                false,
                None,
                NOW + 3.0,
            )
            .unwrap();
        }
        task
    }

    fn sample_outline(task: &WritingTask) -> WritingOutline {
        let material_id = task.selected_material_ids.first().cloned().unwrap_or_default();
        let knowledge_id = task.selected_knowledge_ids.first().cloned().unwrap_or_default();
        WritingOutline {
            title: task.title.clone(),
            writing_purpose: task.writing_purpose.clone(),
            core_points: vec!["试点推进总体平稳".to_string(), "仍有三项待补齐".to_string()],
            sections: vec![
                OutlineSection {
                    no: 1,
                    heading: "工作概况".to_string(),
                    sub_headings: vec!["总体进展".to_string()],
                    focus: "概括本年度工作范围与总体完成情况".to_string(),
                    material_ids: if material_id.is_empty() {
                        vec![]
                    } else {
                        vec![material_id.clone()]
                    },
                    knowledge_ids: if knowledge_id.is_empty() {
                        vec![]
                    } else {
                        vec![knowledge_id.clone()]
                    },
                    expected_words: 300,
                },
                OutlineSection {
                    no: 2,
                    heading: "问题与不足".to_string(),
                    sub_headings: vec![],
                    focus: "只写素材中明确记录的问题，不推测原因".to_string(),
                    material_ids: vec![],
                    knowledge_ids: vec![],
                    expected_words: 400,
                },
            ],
        }
    }

    #[test]
    fn default_templates_cover_four_sets() {
        let templates = default_templates();
        templates.validate().unwrap();
        assert_eq!(templates.templates.len(), 4);
        let keys: Vec<&str> = templates
            .templates
            .iter()
            .map(|template| template.key.as_str())
            .collect();
        assert!(keys.contains(&"工作总结"));
        assert!(keys.contains(&"经验交流"));
        assert!(keys.contains(&"调研报告"));
        assert!(keys.contains(&"专项汇报"));
        for template in &templates.templates {
            assert!(!template.sections.is_empty());
            for section in &template.sections {
                assert!(!section.focus.trim().is_empty());
            }
        }
    }

    #[test]
    fn load_templates_without_file_uses_embedded_default_and_writes_nothing() {
        let fixture = Fixture::new("default");
        let loaded = load_templates(fixture.config()).unwrap();
        assert!(loaded.from_default);
        assert!(
            !templates_path(fixture.config()).exists(),
            "无文件时不得创建默认配置"
        );
    }

    #[test]
    fn load_templates_reads_edited_file_and_rejects_bad_schema() {
        let fixture = Fixture::new("edited");
        let mut templates = default_templates();
        templates.templates[0].label = "自定义工作总结".to_string();
        fs::write(
            templates_path(fixture.config()),
            serde_json::to_string_pretty(&templates).unwrap(),
        )
        .unwrap();
        let loaded = load_templates(fixture.config()).unwrap();
        assert!(!loaded.from_default);
        assert_eq!(loaded.templates.templates[0].label, "自定义工作总结");

        templates.schema = "omnidesk.writing-templates.v99".to_string();
        fs::write(
            templates_path(fixture.config()),
            serde_json::to_string(&templates).unwrap(),
        )
        .unwrap();
        let error = load_templates(fixture.config()).unwrap_err();
        assert!(error.contains("不支持的类型模板版本"), "{error}");
    }

    #[test]
    fn template_mapping_covers_all_document_types() {
        let templates = default_templates();
        for document_type in DOCUMENT_TYPES {
            template_for_document_type(&templates, document_type).unwrap();
        }
        let error = template_for_document_type(&templates, "新闻稿").unwrap_err();
        assert!(error.contains("没有为文稿类型"), "{error}");
    }

    #[test]
    fn outline_shape_accepts_valid_and_rejects_empty_sections() {
        let fixture = Fixture::new("shape");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let outline = sample_outline(&task);
        validate_outline_shape(&outline).unwrap();
        let mut empty = outline.clone();
        empty.sections.clear();
        let error = validate_outline_shape(&empty).unwrap_err();
        assert!(error.contains("sections 不能为空"), "{error}");
    }

    #[test]
    fn outline_mapping_rejects_unselected_material_and_knowledge() {
        let fixture = Fixture::new("mapping");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let mut outline = sample_outline(&task);
        outline.sections[0].material_ids = vec!["m-not-selected".to_string()];
        let error = validate_outline(&outline, &task).unwrap_err();
        assert!(error.contains("未加入素材包的素材编号"), "{error}");

        let mut outline = sample_outline(&task);
        outline.sections[0].knowledge_ids = vec!["k-not-selected".to_string()];
        let error = validate_outline(&outline, &task).unwrap_err();
        assert!(error.contains("未加入素材包的知识编号"), "{error}");
    }

    #[test]
    fn outline_rejects_zero_expected_words_and_duplicate_section_no() {
        let fixture = Fixture::new("numbers");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let mut outline = sample_outline(&task);
        outline.sections[0].expected_words = 0;
        let error = validate_outline_shape(&outline).unwrap_err();
        assert!(error.contains("expected_words 必须是正数"), "{error}");

        let mut outline = sample_outline(&task);
        outline.sections[1].no = 1;
        let error = validate_outline_shape(&outline).unwrap_err();
        assert!(error.contains("章节编号重复"), "{error}");
    }

    #[test]
    fn scenario_four_every_section_has_focus_words_and_material_mapping() {
        let fixture = Fixture::new("scenario4");
        let material_id = confirmed_material(fixture.work(), "m-606", "虚构试点素材");
        let task = writing_task(
            fixture.work(),
            vec![material_id.clone()],
            vec!["k-606".to_string()],
        );
        let outline = sample_outline(&task);
        validate_outline(&outline, &task).unwrap();
        for section in &outline.sections {
            assert!(!section.heading.trim().is_empty());
            assert!(!section.focus.trim().is_empty(), "每章必须有写作重点");
            assert!(section.expected_words > 0, "每章必须有目标字数");
            for id in &section.material_ids {
                assert!(
                    task.selected_material_ids.iter().any(|value| value == id),
                    "素材映射必须来自任务素材包"
                );
            }
            for id in &section.knowledge_ids {
                assert!(
                    task.selected_knowledge_ids.iter().any(|value| value == id),
                    "知识映射必须来自任务素材包"
                );
            }
        }
        assert_eq!(outline.sections.len(), 2);
        assert!(outline
            .sections
            .iter()
            .any(|section| !section.material_ids.is_empty()));
    }

    #[test]
    fn prompt_contains_requirements_template_and_material_package() {
        let fixture = Fixture::new("prompt");
        let material_id = confirmed_material(fixture.work(), "m-prompt", "虚构素材甲");
        let task = writing_task(
            fixture.work(),
            vec![material_id.clone()],
            vec!["k-prompt".to_string()],
        );
        let templates = default_templates();
        let template = template_for_document_type(&templates, &task.document_type).unwrap();
        let material = material_store::read_material(fixture.work(), &material_id).unwrap();
        let prompt = build_outline_prompt(&task, template, &[material]);
        assert!(prompt.contains(&task.title), "缺少写作需求");
        assert!(prompt.contains("只写已确认事实"), "缺少写作要求");
        assert!(prompt.contains("推荐章节骨架"), "缺少模板结构");
        assert!(prompt.contains("工作概况"), "缺少模板章节");
        assert!(prompt.contains(&material_id), "缺少素材编号");
        assert!(prompt.contains("虚构素材正文"), "缺少素材正文");
        assert!(prompt.contains("k-prompt"), "缺少知识编号");
        assert!(prompt.contains("expected_words"), "缺少输出契约");
    }

    #[test]
    fn generate_rejects_wrong_status_and_empty_material_package() {
        let fixture = Fixture::new("generate-status");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let error = generate_outline_prompt(
            fixture.work(),
            fixture.config(),
            &OutlineGenerateRequest {
                writing_id: task.writing_id.clone(),
            },
        )
        .unwrap_err();
        assert!(
            error.contains("只有「素材收集中」或「大纲待确认」"),
            "{error}"
        );

        let task = to_status(fixture.work(), &task.writing_id, STATUS_COLLECTING_MATERIALS);
        let error = generate_outline_prompt(
            fixture.work(),
            fixture.config(),
            &OutlineGenerateRequest {
                writing_id: task.writing_id.clone(),
            },
        )
        .unwrap_err();
        assert!(error.contains("素材包为空"), "{error}");
    }

    #[test]
    fn generate_rejects_unconfirmed_material_in_package() {
        let fixture = Fixture::new("generate-unconfirmed");
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
        let task = writing_task(
            fixture.work(),
            vec![material.material_id.clone()],
            vec![],
        );
        let task = to_status(fixture.work(), &task.writing_id, STATUS_COLLECTING_MATERIALS);
        let error = generate_outline_prompt(
            fixture.work(),
            fixture.config(),
            &OutlineGenerateRequest {
                writing_id: task.writing_id.clone(),
            },
        )
        .unwrap_err();
        assert!(error.contains("不属于可信素材包"), "{error}");
    }

    #[test]
    fn markdown_roundtrip_preserves_sections() {
        let fixture = Fixture::new("roundtrip");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let outline = sample_outline(&task);
        let markdown = render_outline_markdown(&outline);
        let parsed = parse_outline_markdown(&markdown).unwrap();
        assert_eq!(parsed, outline);
    }

    #[test]
    fn confirm_persists_outline_snapshot_and_moves_to_writing() {
        let fixture = Fixture::new("confirm");
        let material_id = confirmed_material(fixture.work(), "m-confirm", "虚构确认素材");
        let task = writing_task(
            fixture.work(),
            vec![material_id.clone()],
            vec!["k-confirm".to_string()],
        );
        let task = to_status(fixture.work(), &task.writing_id, STATUS_COLLECTING_MATERIALS);
        let outline = sample_outline(&task);
        let confirmed = confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: outline.clone(),
                note: Some("用户已核对章节与素材映射".to_string()),
            },
            NOW + 10.0,
        )
        .unwrap();
        assert_eq!(confirmed.status, writing_store::STATUS_WRITING);
        assert_eq!(confirmed.version, 1);
        let stored = document_store::read_outline(fixture.work(), &task.writing_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored, render_outline_markdown(&outline));
        let versions = document_store::list_versions(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].scope, VersionScope::Outline);
        assert_eq!(versions[0].source, VersionSource::User);
        let after = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(after.status, writing_store::STATUS_WRITING);
        assert!(after
            .status_records
            .iter()
            .any(|record| record.action.contains("素材收集中 → 大纲待确认")));
        assert!(after
            .status_records
            .iter()
            .any(|record| record.action.contains("大纲待确认 → 写作中")));
    }

    #[test]
    fn confirm_from_outline_pending_uses_outline_confirmed_gate() {
        let fixture = Fixture::new("confirm-pending");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let task = to_status(fixture.work(), &task.writing_id, STATUS_OUTLINE_PENDING);
        let outline = sample_outline(&task);
        let confirmed = confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline,
                note: None,
            },
            NOW + 11.0,
        )
        .unwrap();
        assert_eq!(confirmed.status, writing_store::STATUS_WRITING);
        let after = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(
            after
                .status_records
                .iter()
                .filter(|record| record.action.contains("大纲待确认 → 写作中"))
                .count(),
            1
        );
    }

    #[test]
    fn confirm_rejects_writing_status_without_overwrite() {
        let fixture = Fixture::new("confirm-reject");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let task = to_status(fixture.work(), &task.writing_id, STATUS_OUTLINE_PENDING);
        writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            writing_store::STATUS_WRITING,
            true,
            None,
            NOW + 12.0,
        )
        .unwrap();
        let outline = sample_outline(&task);
        let error = confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline,
                note: None,
            },
            NOW + 13.0,
        )
        .unwrap_err();
        assert!(
            error.contains("只有「素材收集中」或「大纲待确认」"),
            "{error}"
        );
        assert!(!document_store::outline_path(fixture.work(), &task.writing_id)
            .unwrap()
            .exists());
    }

    #[test]
    fn get_returns_confirmed_outline_and_errors_before_confirmation() {
        let fixture = Fixture::new("get");
        let task = writing_task(fixture.work(), vec![], vec![]);
        let error = read_confirmed_outline(fixture.work(), &task.writing_id).unwrap_err();
        assert!(error.contains("尚未确认大纲"), "{error}");
        let task = to_status(fixture.work(), &task.writing_id, STATUS_COLLECTING_MATERIALS);
        let outline = sample_outline(&task);
        confirm_outline(
            fixture.work(),
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: outline.clone(),
                note: None,
            },
            NOW + 14.0,
        )
        .unwrap();
        let loaded = read_confirmed_outline(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(loaded.outline, outline);
        assert_eq!(loaded.status, writing_store::STATUS_WRITING);
        assert_eq!(loaded.version, 1);
        assert!(!loaded.markdown.is_empty());
    }
}
