//! 审阅修改 IPC（OD-WT-608）。
//!
//! 架构契合：薄封装 607 的修订确认通道 + 602 版本存储 + 603 记录；不新增
//! 模型通道。AI 审阅只产出问题清单（维度 + 章节/段落定位 + 描述 + 建议），
//! 不改写正文；用户选择某条问题后，修订稿仍经 607 `confirm_section` 的
//! 同一路径写入正文与快照，并在 601 状态记录追加应用留痕。
//!
//! 十维检查单：事实准确性 / 数据准确性 / 政策制度表述 / 逻辑结构 /
//! 章节衔接 / 内容重复 / 语言表达 / 文字规范 / 篇幅 / 与写作要求一致性。
//! 维度是宿主白名单，模型自造维度会在反序列化或校验时被拒绝。
//!
//! 版本能力全部复用 602：列表带 manifest 的每版说明 note 与相对上一同类型
//! 版本的段落差异摘要；对比读取两份快照；恢复把旧快照落为新当前版并新增
//! 快照，历史不删除，写作任务状态不变、只追加「恢复 vX」状态记录。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document_store::{
    self, DocumentKind, ParagraphDiff, VersionEntry, VersionScope, VersionSource,
};
use crate::writing_outline::{self, OutlineSection, WritingOutline};
use crate::writing_section::{self, SectionConfirmRequest, SectionDraft};
use crate::writing_store::{self, StatusRecord, WritingTask, WritingTaskPatch};

/// 十维审阅检查单，顺序固定，供提示词、白名单与界面共用。
pub const REVIEW_DIMENSIONS: &[&str] = &[
    "事实准确性",
    "数据准确性",
    "政策制度表述",
    "逻辑结构",
    "章节衔接",
    "内容重复",
    "语言表达",
    "文字规范",
    "篇幅",
    "与写作要求一致性",
];

pub const MAX_INSTRUCTION_CHARS: usize = 500;
pub const MAX_FINDING_ID_CHARS: usize = 40;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_SUGGESTION_CHARS: usize = 500;
pub const MAX_TRACE_CHARS: usize = 160;

/// 审阅维度枚举。序列化标签即中文维度名，界面无需再做映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDimension {
    #[serde(rename = "事实准确性")]
    FactAccuracy,
    #[serde(rename = "数据准确性")]
    DataAccuracy,
    #[serde(rename = "政策制度表述")]
    PolicyExpression,
    #[serde(rename = "逻辑结构")]
    LogicalStructure,
    #[serde(rename = "章节衔接")]
    SectionCohesion,
    #[serde(rename = "内容重复")]
    ContentRepetition,
    #[serde(rename = "语言表达")]
    LanguageExpression,
    #[serde(rename = "文字规范")]
    TextNorm,
    #[serde(rename = "篇幅")]
    Length,
    #[serde(rename = "与写作要求一致性")]
    RequirementConsistency,
}

impl ReviewDimension {
    pub const ALL: [ReviewDimension; 10] = [
        ReviewDimension::FactAccuracy,
        ReviewDimension::DataAccuracy,
        ReviewDimension::PolicyExpression,
        ReviewDimension::LogicalStructure,
        ReviewDimension::SectionCohesion,
        ReviewDimension::ContentRepetition,
        ReviewDimension::LanguageExpression,
        ReviewDimension::TextNorm,
        ReviewDimension::Length,
        ReviewDimension::RequirementConsistency,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ReviewDimension::FactAccuracy => "事实准确性",
            ReviewDimension::DataAccuracy => "数据准确性",
            ReviewDimension::PolicyExpression => "政策制度表述",
            ReviewDimension::LogicalStructure => "逻辑结构",
            ReviewDimension::SectionCohesion => "章节衔接",
            ReviewDimension::ContentRepetition => "内容重复",
            ReviewDimension::LanguageExpression => "语言表达",
            ReviewDimension::TextNorm => "文字规范",
            ReviewDimension::Length => "篇幅",
            ReviewDimension::RequirementConsistency => "与写作要求一致性",
        }
    }

    pub fn from_label(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        for dimension in Self::ALL {
            if dimension.label() == trimmed {
                return Ok(dimension);
            }
        }
        Err(format!(
            "审阅维度「{trimmed}」不在允许范围（{}）。",
            REVIEW_DIMENSIONS.join(" / ")
        ))
    }
}

/// 一条审阅问题。定位到章节号与章节内段落号（从 1 起）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFinding {
    pub finding_id: String,
    pub dimension: ReviewDimension,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub description: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewRequest {
    pub writing_id: String,
    /// 用户补充说明：审阅重点、必须核对的口径等。过宿主闸门后才进提示词。
    pub instruction: String,
}

/// 审阅提示词回执。本命令不写任何文件，模型调用仍在界面侧。
#[derive(Debug, Clone, Serialize)]
pub struct WritingReviewPrompt {
    pub prompt: String,
    pub writing_id: String,
    pub status: String,
    pub section_count: usize,
    pub confirmed_section_count: usize,
    pub dimension_count: usize,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

/// 用户应用某条审阅问题：修订稿走 607 确认通道写正文与快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewApplyRequest {
    pub writing_id: String,
    pub finding: ReviewFinding,
    /// 修订后的整章稿（用户编辑或经 607 修订通道产生）。
    pub draft: SectionDraft,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewFindingApplied {
    pub writing_id: String,
    pub finding_id: String,
    pub dimension: String,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub status: String,
    pub version: u64,
    pub updated_at: String,
    pub citations_added: usize,
    pub all_sections_confirmed: bool,
    pub trace: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

/// 单版摘要：manifest 元数据 + 相对上一同类型版本的段落差异。
#[derive(Debug, Clone, Serialize)]
pub struct VersionSummary {
    pub version: u64,
    pub at: String,
    pub source: VersionSource,
    pub scope: VersionScope,
    pub note: String,
    pub current: bool,
    pub content_sha256: String,
    pub diff: Option<VersionDiffSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionDiffSummary {
    pub previous_version: u64,
    pub changed_paragraphs: usize,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub changed_paragraph_nos: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WritingVersionList {
    pub writing_id: String,
    pub current_version: u64,
    pub updated_at: String,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionDiffRequest {
    pub writing_id: String,
    pub from_version: u64,
    pub to_version: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WritingVersionDiff {
    pub writing_id: String,
    pub from: VersionEntry,
    pub to: VersionEntry,
    pub diff: ParagraphDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRestoreRequest {
    pub writing_id: String,
    pub version: u64,
    /// 恢复必须由用户显式确认；false 时宿主直接拒绝，不做任何写入。
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WritingVersionRestored {
    pub writing_id: String,
    pub restored_version: u64,
    pub new_version: VersionEntry,
    pub status: String,
    pub updated_at: String,
    pub status_records: Vec<StatusRecord>,
}

fn normalize_writing_id(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("写作任务编号不能为空。".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_instruction(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.chars().count() > MAX_INSTRUCTION_CHARS {
        return Err(format!("审阅补充说明最多 {MAX_INSTRUCTION_CHARS} 字。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("审阅补充说明含控制字符。".to_string());
    }
    Ok(trimmed.to_string())
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

fn section_paragraphs(content: &str) -> Vec<String> {
    content
        .split("\n\n")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
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

fn review_allowed(status: &str) -> bool {
    matches!(
        status,
        writing_store::STATUS_WRITING
            | writing_store::STATUS_FACT_CHECKING
            | writing_store::STATUS_PENDING_REVIEW
    )
}

const DIMENSION_GUIDES: &[(&str, &str)] = &[
    ("事实准确性", "核对事实性表述是否有素材或知识依据；是否把计划写成已完成、把相关写成因果。"),
    ("数据准确性", "核对数字、比例、时间、排位与来源原文是否一致；是否存在无依据的估算。"),
    ("政策制度表述", "核对政策、制度、文号、机构名称与职务的表述是否准确；不得自造依据。"),
    ("逻辑结构", "检查观点与论据是否匹配，层次是否清晰，是否存在前后矛盾。"),
    ("章节衔接", "检查章节起承转合是否自然，是否存在突然跳转、重复引言或断裂。"),
    ("内容重复", "检查同一意思是否在多个段落或章节反复出现、堆砌凑篇幅。"),
    ("语言表达", "检查用词是否准确、句式是否通顺、语气是否符合文稿类型与受众。"),
    ("文字规范", "检查标点、数字用法、称谓、简称与固定表述是否符合公文规范。"),
    ("篇幅", "检查各章篇幅是否与预计字数和大纲分配明显失衡。"),
    ("与写作要求一致性", "逐条对照写作要求，检查是否遗漏、偏离或自行扩大要求。"),
];

/// 构建十维审阅提示词。只读任务、大纲与正文，不写任何文件。
pub fn build_review_prompt(
    root: &Path,
    request: &ReviewRequest,
) -> Result<WritingReviewPrompt, String> {
    let writing_id = normalize_writing_id(&request.writing_id)?;
    let task = writing_store::read_writing_task(root, &writing_id)?;
    if !review_allowed(&task.status) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」「{}」或「{}」可以发起审阅。",
            task.status,
            writing_store::STATUS_WRITING,
            writing_store::STATUS_FACT_CHECKING,
            writing_store::STATUS_PENDING_REVIEW
        ));
    }
    let outline = writing_outline::read_confirmed_outline(root, &writing_id)?.outline;
    let instruction = normalize_instruction(&request.instruction)?;
    let mut numbered = String::new();
    let mut confirmed_sections = 0usize;
    for section in &outline.sections {
        let state = writing_section::read_section_state(root, &writing_id, section.no)?;
        let _ = writeln!(numbered);
        let _ = writeln!(
            numbered,
            "### 第 {} 章：{}",
            section.no,
            section.heading.trim()
        );
        match state.content.as_deref() {
            Some(content) => {
                confirmed_sections += 1;
                for (index, paragraph) in section_paragraphs(content).iter().enumerate() {
                    let _ = writeln!(numbered, "{}. {}", index + 1, paragraph);
                }
            }
            None => {
                let _ = writeln!(
                    numbered,
                    "（本章尚未确认，正文中以「{}」占位，不属于本次审阅范围。）",
                    writing_section::SECTION_PLACEHOLDER
                );
            }
        }
    }
    let prompt = render_review_prompt(
        &task,
        &outline,
        &numbered,
        !instruction.is_empty(),
        &instruction,
    );
    Ok(WritingReviewPrompt {
        prompt,
        writing_id: task.writing_id,
        status: task.status,
        section_count: outline.sections.len(),
        confirmed_section_count: confirmed_sections,
        dimension_count: REVIEW_DIMENSIONS.len(),
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

fn render_review_prompt(
    task: &WritingTask,
    outline: &WritingOutline,
    numbered_text: &str,
    has_instruction: bool,
    instruction: &str,
) -> String {
    let mut prompt = String::new();
    let _ = writeln!(
        prompt,
        "你是中文公文审阅助手。请按下面的十维检查单审阅文稿，只输出问题清单，不输出改写后的正文。只输出 JSON，不输出解释性文字。"
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
    let _ = writeln!(prompt, "- 写作要求：");
    let requirement_lines: Vec<&str> = task
        .requirements
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if requirement_lines.is_empty() {
        let _ = writeln!(prompt, "  - （未填写）");
    } else {
        for line in requirement_lines {
            let _ = writeln!(prompt, "  - {line}");
        }
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 二、审阅口径");
    let _ = writeln!(prompt, "- 当前状态：{}", task.status);
    let _ = writeln!(prompt, "- 大纲标题：{}", outline.title.trim());
    let _ = writeln!(
        prompt,
        "- 只审阅下面已经确认的章节；「尚未确认」的占位内容不列入问题清单。"
    );
    if has_instruction {
        let _ = writeln!(prompt, "- 用户补充说明：{instruction}");
    } else {
        let _ = writeln!(prompt, "- 用户补充说明：（未填写）");
    }
    let _ = writeln!(prompt);
    let _ = writeln!(
        prompt,
        "## 三、待审阅正文（按章节、按空行分段，段号从 1 起）"
    );
    prompt.push_str(numbered_text.trim());
    prompt.push('\n');
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 四、十维检查单");
    for (index, (label, guide)) in DIMENSION_GUIDES.iter().enumerate() {
        let _ = writeln!(prompt, "{}. {label}：{guide}", index + 1);
    }
    let _ = writeln!(prompt);
    let _ = writeln!(prompt, "## 五、输出要求");
    prompt.push_str(
        "只输出一个 JSON 对象，字段固定如下，不得增加字段：\n\
{\"findings\":[{\"finding_id\":\"F1\",\"dimension\":\"事实准确性\",\"section_no\":1,\"paragraph_no\":2,\"description\":\"问题描述\",\"suggestion\":\"修改建议\"}]}\n",
    );
    let _ = writeln!(
        prompt,
        "dimension 只能是 {}；",
        REVIEW_DIMENSIONS.join(" / ")
    );
    prompt.push_str(
        "section_no 与 paragraph_no 必须指向上面的章节与段落；description 只描述问题，suggestion 只给修改方向或建议表述。\n\
没有问题时输出 {\"findings\":[]}。不得输出修改后的正文，不得替用户直接定稿。\n\
不得新增文稿与素材中没有的事实、数据、机构、人名或政策依据。\n",
    );
    prompt
}

fn validate_finding_text(value: &str, field: &str, limit: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > limit {
        return Err(format!("{field}最多 {limit} 字。"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(format!("{field}含控制字符。"));
    }
    Ok(trimmed.to_string())
}

fn validate_finding(
    finding: &ReviewFinding,
    outline: &WritingOutline,
) -> Result<ReviewFinding, String> {
    let finding_id = validate_finding_text(&finding.finding_id, "问题编号", MAX_FINDING_ID_CHARS)?;
    if finding_id.is_empty() {
        return Err("问题编号不能为空。".to_string());
    }
    let description =
        validate_finding_text(&finding.description, "问题描述", MAX_DESCRIPTION_CHARS)?;
    if description.is_empty() {
        return Err("问题描述不能为空。".to_string());
    }
    let suggestion =
        validate_finding_text(&finding.suggestion, "修改建议", MAX_SUGGESTION_CHARS)?;
    let section = find_section(outline, finding.section_no)?;
    if finding.paragraph_no == 0 {
        return Err("段落序号从 1 起，不能为 0。".to_string());
    }
    Ok(ReviewFinding {
        finding_id,
        dimension: finding.dimension,
        section_no: section.no,
        paragraph_no: finding.paragraph_no,
        description,
        suggestion,
    })
}

fn trace_for_finding(finding: &ReviewFinding) -> String {
    format!(
        "应用审阅问题 {}（{}）：{}",
        finding.finding_id,
        finding.dimension.label(),
        clip_chars(&finding.description, MAX_TRACE_CHARS)
    )
}

/// 用户应用一条审阅问题。
///
/// 修订稿统一走 607 `confirm_section`：正文写入与完整快照沿用同一条路径，
/// 引用记录同样只追加。写入成功后经由 601 `update_writing_task` 追加状态记录，
/// 形成「哪条问题被应用」的留痕；状态本身不迁移。
pub fn apply_finding(
    root: &Path,
    request: &ReviewApplyRequest,
    now: f64,
) -> Result<ReviewFindingApplied, String> {
    let writing_id = normalize_writing_id(&request.writing_id)?;
    let task = writing_store::read_writing_task(root, &writing_id)?;
    if !review_allowed(&task.status) {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」「{}」或「{}」可以应用审阅问题。",
            task.status,
            writing_store::STATUS_WRITING,
            writing_store::STATUS_FACT_CHECKING,
            writing_store::STATUS_PENDING_REVIEW
        ));
    }
    let outline = writing_outline::read_confirmed_outline(root, &writing_id)?.outline;
    let finding = validate_finding(&request.finding, &outline)?;
    if request.draft.section_no != finding.section_no {
        return Err(format!(
            "问题定位在第 {} 章，但修订稿是第 {} 章；请检查后再应用。",
            finding.section_no, request.draft.section_no
        ));
    }
    let state = writing_section::read_section_state(root, &writing_id, finding.section_no)?;
    let paragraphs = state
        .content
        .as_deref()
        .map(section_paragraphs)
        .unwrap_or_default();
    if paragraphs.is_empty() {
        return Err(format!(
            "第 {} 章尚未确认正文，没有可以应用审阅问题的段落。",
            finding.section_no
        ));
    }
    if finding.paragraph_no as usize > paragraphs.len() {
        return Err(format!(
            "第 {} 章只有 {} 个段落，段落序号 {} 不存在。",
            finding.section_no,
            paragraphs.len(),
            finding.paragraph_no
        ));
    }
    let confirmed = writing_section::confirm_section(
        root,
        &SectionConfirmRequest {
            writing_id: writing_id.clone(),
            draft: request.draft.clone(),
            note: request.note.clone(),
        },
        now,
    )?;
    let trace = trace_for_finding(&finding);
    let traced = writing_store::update_writing_task(
        root,
        &writing_id,
        &WritingTaskPatch {
            reason: Some(trace.clone()),
            ..Default::default()
        },
        now,
    )
    .map_err(|error| format!("修订已写入，但审阅留痕追加失败（需修复）：{error}"))?;
    Ok(ReviewFindingApplied {
        writing_id,
        finding_id: finding.finding_id,
        dimension: finding.dimension.label().to_string(),
        section_no: finding.section_no,
        paragraph_no: finding.paragraph_no,
        status: traced.status,
        version: confirmed.version,
        updated_at: traced.updated_at,
        citations_added: confirmed.citations_added,
        all_sections_confirmed: confirmed.all_sections_confirmed,
        trace,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
    })
}

fn diff_summary(
    previous: &mut Option<(u64, String)>,
    version: u64,
    content: &str,
) -> Option<VersionDiffSummary> {
    let summary = previous.as_ref().map(|(previous_version, previous_content)| {
        let diff = document_store::diff_paragraphs(previous_content, content);
        let mut added = 0usize;
        let mut removed = 0usize;
        let mut modified = 0usize;
        for change in &diff.changes {
            match change.kind {
                document_store::ParagraphChangeKind::Added => added += 1,
                document_store::ParagraphChangeKind::Removed => removed += 1,
                document_store::ParagraphChangeKind::Modified => modified += 1,
            }
        }
        VersionDiffSummary {
            previous_version: *previous_version,
            changed_paragraphs: diff.changes.len(),
            added,
            removed,
            modified,
            changed_paragraph_nos: diff.changed_paragraph_nos,
        }
    });
    previous.replace((version, content.to_string()));
    summary
}

/// 版本列表：manifest 每版说明 note + 相对上一同类型版本的段落差异摘要。
pub fn list_version_summaries(
    root: &Path,
    writing_id: &str,
) -> Result<WritingVersionList, String> {
    let writing_id = normalize_writing_id(writing_id)?;
    writing_store::read_writing_task(root, &writing_id)?;
    let manifest = document_store::read_manifest(root, &writing_id)?;
    let mut previous_outline: Option<(u64, String)> = None;
    let mut previous_body: Option<(u64, String)> = None;
    let mut versions = Vec::with_capacity(manifest.versions.len());
    for entry in &manifest.versions {
        let snapshot = document_store::read_version(root, &writing_id, entry.version)?;
        let diff = match entry.scope.target_kind() {
            DocumentKind::Outline => diff_summary(
                &mut previous_outline,
                entry.version,
                &snapshot.content,
            ),
            DocumentKind::Body => {
                diff_summary(&mut previous_body, entry.version, &snapshot.content)
            }
        };
        versions.push(VersionSummary {
            version: entry.version,
            at: entry.at.clone(),
            source: entry.source,
            scope: entry.scope,
            note: entry.note.clone(),
            current: entry.version == manifest.current_version,
            content_sha256: entry.content_sha256.clone(),
            diff,
        });
    }
    Ok(WritingVersionList {
        writing_id,
        current_version: manifest.current_version,
        updated_at: manifest.updated_at,
        versions,
    })
}

fn kind_label(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Outline => "大纲",
        DocumentKind::Body => "正文",
    }
}

/// 两份历史版本的段落级对比。不同文稿类型（大纲 / 正文）拒绝对比。
pub fn diff_versions(
    root: &Path,
    request: &VersionDiffRequest,
) -> Result<WritingVersionDiff, String> {
    let writing_id = normalize_writing_id(&request.writing_id)?;
    if request.from_version == 0 || request.to_version == 0 {
        return Err("版本号从 1 起，不能为 0。".to_string());
    }
    writing_store::read_writing_task(root, &writing_id)?;
    let from = document_store::read_version(root, &writing_id, request.from_version)?;
    let to = document_store::read_version(root, &writing_id, request.to_version)?;
    let from_kind = from.entry.scope.target_kind();
    let to_kind = to.entry.scope.target_kind();
    if from_kind != to_kind {
        return Err(format!(
            "版本 v{} 是{}，版本 v{} 是{}；不同文稿类型不能做段落对比。",
            from.entry.version,
            kind_label(from_kind),
            to.entry.version,
            kind_label(to_kind)
        ));
    }
    let diff = document_store::diff_paragraphs(&from.content, &to.content);
    Ok(WritingVersionDiff {
        writing_id,
        from: from.entry,
        to: to.entry,
        diff,
    })
}

/// 恢复历史版本：602 把旧快照落为新当前版并新增快照；601 状态不变、只追加留痕。
pub fn restore_version(
    root: &Path,
    request: &VersionRestoreRequest,
    now: f64,
) -> Result<WritingVersionRestored, String> {
    let writing_id = normalize_writing_id(&request.writing_id)?;
    if !request.confirmed {
        return Err("恢复历史版本需要用户显式确认。".to_string());
    }
    if request.version == 0 {
        return Err("版本号从 1 起，不能为 0。".to_string());
    }
    writing_store::read_writing_task(root, &writing_id)?;
    let manifest = document_store::read_manifest(root, &writing_id)?;
    if request.version == manifest.current_version {
        return Err(format!(
            "版本 v{} 就是当前版本，无需恢复。",
            request.version
        ));
    }
    // 先确认快照可读且指纹一致，再写新当前版。
    document_store::read_version(root, &writing_id, request.version)?;
    let entry = document_store::restore_version(root, &writing_id, request.version, now)?;
    let trace = format!("恢复 v{}", request.version);
    let task = writing_store::update_writing_task(
        root,
        &writing_id,
        &WritingTaskPatch {
            reason: Some(trace),
            ..Default::default()
        },
        now,
    )
    .map_err(|error| format!("版本已恢复，但状态记录追加失败（需修复）：{error}"))?;
    Ok(WritingVersionRestored {
        writing_id,
        restored_version: request.version,
        new_version: entry,
        status: task.status,
        updated_at: task.updated_at,
        status_records: task.status_records,
    })
}
