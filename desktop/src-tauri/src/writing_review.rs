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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::material_store::{self, MaterialDraft};
    use crate::writing_outline::{OutlineConfirmRequest, OutlineSection};
    use crate::writing_store::{WritingTaskDraft, STATUS_COLLECTING_MATERIALS};

    const NOW: f64 = 1_780_600_000.0;

    struct Fixture {
        base: PathBuf,
        work: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-review-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let work = base.join("work");
            fs::create_dir_all(&work).unwrap();
            Self { base, work }
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
                content: "虚构素材正文：本期完成三处试点，覆盖两个班组。".to_string(),
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

    fn writing_task(root: &Path, material_ids: Vec<String>) -> WritingTask {
        writing_store::create_writing_task(
            root,
            &WritingTaskDraft {
                writing_id: Some("w-608".to_string()),
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
                selected_knowledge_ids: vec![],
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn sample_outline(task: &WritingTask) -> WritingOutline {
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
                    knowledge_ids: vec![],
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

    fn ready_fixture(tag: &str) -> (Fixture, WritingTask, WritingOutline) {
        let fixture = Fixture::new(tag);
        let material_id = confirmed_material(fixture.work(), "m-608", "虚构试点素材");
        let task = writing_task(fixture.work(), vec![material_id]);
        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            STATUS_COLLECTING_MATERIALS,
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
            citations: vec![writing_section::SectionCitation {
                source_kind: crate::writing_records::CitationSourceKind::Material,
                source_id: "m-608".to_string(),
                quote: "完成三处试点。".to_string(),
            }],
            segments: vec![
                writing_section::SectionSegment {
                    kind: "事实".to_string(),
                    text: "本期完成三处试点。".to_string(),
                    quote: Some("完成三处试点。".to_string()),
                },
                writing_section::SectionSegment {
                    kind: "分析".to_string(),
                    text: "试点推进总体平稳。".to_string(),
                    quote: None,
                },
            ],
        }
    }

    fn confirm(
        fixture: &Fixture,
        task: &WritingTask,
        draft: SectionDraft,
        at: f64,
    ) -> writing_section::WritingSectionConfirmed {
        writing_section::confirm_section(
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

    fn finding(section_no: u32, paragraph_no: u32) -> ReviewFinding {
        ReviewFinding {
            finding_id: "F1".to_string(),
            dimension: ReviewDimension::FactAccuracy,
            section_no,
            paragraph_no,
            description: "试点覆盖范围表述不完整。".to_string(),
            suggestion: "补充班组范围，避免读者误以为覆盖全部班组。".to_string(),
        }
    }

    fn apply(
        fixture: &Fixture,
        task: &WritingTask,
        finding: ReviewFinding,
        content: &str,
        at: f64,
    ) -> ReviewFindingApplied {
        let section_no = finding.section_no;
        apply_finding(
            fixture.work(),
            &ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding,
                draft: draft(section_no, content),
                note: None,
            },
            at,
        )
        .unwrap()
    }

    #[test]
    fn dimensions_match_ten_item_whitelist() {
        assert_eq!(REVIEW_DIMENSIONS.len(), 10);
        for (index, dimension) in ReviewDimension::ALL.iter().enumerate() {
            assert_eq!(dimension.label(), REVIEW_DIMENSIONS[index]);
        }
        assert_eq!(
            ReviewDimension::from_label(" 数据准确性 ").unwrap(),
            ReviewDimension::DataAccuracy
        );
        let error = ReviewDimension::from_label("自造维度").unwrap_err();
        assert!(error.contains("不在允许范围"), "{error}");
    }

    #[test]
    fn review_prompt_lists_ten_dimensions_and_only_confirmed_text() {
        let (fixture, task, _outline) = ready_fixture("prompt");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let prompt = build_review_prompt(
            fixture.work(),
            &ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: "重点核对数据与口径。".to_string(),
            },
        )
        .unwrap();
        for dimension in REVIEW_DIMENSIONS {
            assert!(prompt.prompt.contains(dimension), "缺少维度 {dimension}");
        }
        assert!(prompt.prompt.contains("本期完成三处试点"), "{}", prompt.prompt);
        assert!(
            prompt.prompt.contains("本章尚未确认") && prompt.prompt.contains("（本章待写作）"),
            "{}",
            prompt.prompt
        );
        assert!(prompt.prompt.contains("重点核对数据与口径"), "{}", prompt.prompt);
        assert!(prompt.prompt.contains("只写已确认事实"), "{}", prompt.prompt);
        assert!(prompt.prompt.contains("不得输出修改后的正文"), "{}", prompt.prompt);
        assert!(prompt.prompt.contains("\"findings\""), "{}", prompt.prompt);
        assert_eq!(prompt.status, writing_store::STATUS_WRITING);
        assert_eq!(prompt.section_count, 2);
        assert_eq!(prompt.confirmed_section_count, 1);
        assert_eq!(prompt.dimension_count, 10);
        assert!(!prompt.masked);
    }

    #[test]
    fn review_prompt_rejects_wrong_status_empty_id_and_unconfirmed_outline() {
        let empty = Fixture::new("prompt-empty");
        let error = build_review_prompt(
            empty.work(),
            &ReviewRequest {
                writing_id: "   ".to_string(),
                instruction: String::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("不能为空"), "{error}");

        let fixture = Fixture::new("prompt-status");
        let material_id = confirmed_material(fixture.work(), "m-608-status", "虚构素材");
        let task = writing_task(fixture.work(), vec![material_id]);
        let error = build_review_prompt(
            fixture.work(),
            &ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: String::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("需求待整理"), "{error}");
        assert!(error.contains("可以发起审阅"), "{error}");

        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            writing_store::STATUS_OUTLINE_PENDING,
            false,
            None,
            NOW + 3.0,
        )
        .unwrap();
        // 仅迁移状态、不写 outline.md，模拟状态已进入写作中但大纲文件缺失。
        let task = writing_store::transition_writing_status(
            fixture.work(),
            &task.writing_id,
            writing_store::STATUS_WRITING,
            true,
            Some("测试用状态迁移"),
            NOW + 4.0,
        )
        .unwrap();
        let error = build_review_prompt(
            fixture.work(),
            &ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: String::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("尚未确认大纲"), "{error}");
    }

    #[test]
    fn review_instruction_length_and_control_chars_are_rejected() {
        let (fixture, task, _outline) = ready_fixture("instruction");
        let long = "说明".repeat(300);
        let error = build_review_prompt(
            fixture.work(),
            &ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: long,
            },
        )
        .unwrap_err();
        assert!(error.contains("最多 500 字"), "{error}");

        let error = build_review_prompt(
            fixture.work(),
            &ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: "说明\u{0007}".to_string(),
            },
        )
        .unwrap_err();
        assert!(error.contains("控制字符"), "{error}");
    }

    #[test]
    fn finding_rejects_unknown_dimension_and_bad_locations() {
        let (_fixture, _task, outline) = ready_fixture("finding");
        let raw = serde_json::json!({
            "finding_id": "F1",
            "dimension": "自造维度",
            "section_no": 1,
            "paragraph_no": 1,
            "description": "虚构问题",
            "suggestion": "虚构建议"
        });
        let error = serde_json::from_value::<ReviewFinding>(raw)
            .unwrap_err()
            .to_string();
        assert!(error.contains("自造维度"), "{error}");

        let mut unknown_field = serde_json::json!({
            "finding_id": "F1",
            "dimension": "事实准确性",
            "section_no": 1,
            "paragraph_no": 1,
            "description": "虚构问题",
            "suggestion": "虚构建议",
            "extra": 1
        });
        assert!(serde_json::from_value::<ReviewFinding>(unknown_field.clone()).is_err());
        assert!(unknown_field.as_object_mut().unwrap().remove("extra").is_some());

        let mut bad = finding(1, 1);
        bad.description = "   ".to_string();
        let error = validate_finding(&bad, &outline).unwrap_err();
        assert!(error.contains("问题描述不能为空"), "{error}");

        let mut bad = finding(99, 1);
        let error = validate_finding(&bad, &outline).unwrap_err();
        assert!(error.contains("大纲中没有第 99 章"), "{error}");

        bad = finding(1, 0);
        let error = validate_finding(&bad, &outline).unwrap_err();
        assert!(error.contains("段落序号从 1 起"), "{error}");
    }

    #[test]
    fn apply_finding_confirms_via_section_path_and_appends_trace() {
        let (fixture, task, _outline) = ready_fixture("apply");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let applied = apply(
            &fixture,
            &task,
            finding(1, 1),
            "本期完成三处试点，覆盖两个班组。\n\n试点推进总体平稳。",
            NOW + 5.0,
        );
        assert_eq!(applied.finding_id, "F1");
        assert_eq!(applied.dimension, "事实准确性");
        assert_eq!(applied.version, 3);
        assert_eq!(applied.citations_added, 1);
        assert_eq!(applied.status, writing_store::STATUS_WRITING);
        assert!(applied.trace.contains("应用审阅问题 F1"), "{}", applied.trace);
        assert!(applied.trace.contains("事实准确性"), "{}", applied.trace);

        let state = writing_section::read_section_state(fixture.work(), &task.writing_id, 1).unwrap();
        assert!(state.content.unwrap().contains("覆盖两个班组"));
        let versions = document_store::list_versions(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(versions.len(), 3);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(task.status, writing_store::STATUS_WRITING);
        assert_eq!(task.status_records.len(), 5);
        let last = task.status_records.last().unwrap();
        assert!(last.reason.contains("应用审阅问题 F1"), "{}", last.reason);
    }

    #[test]
    fn apply_finding_rejects_section_mismatch_unconfirmed_and_bad_paragraph() {
        let (fixture, task, _outline) = ready_fixture("apply-reject");
        let error = apply_finding(
            fixture.work(),
            &ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding: finding(1, 1),
                draft: draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"),
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap_err();
        assert!(error.contains("尚未确认正文"), "{error}");

        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 5.0);
        let error = apply_finding(
            fixture.work(),
            &ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding: finding(1, 1),
                draft: draft(2, "第二章虚构正文。"),
                note: None,
            },
            NOW + 6.0,
        )
        .unwrap_err();
        assert!(error.contains("问题定位在第 1 章"), "{error}");

        let error = apply_finding(
            fixture.work(),
            &ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding: finding(1, 5),
                draft: draft(1, "本期完成三处试点。"),
                note: None,
            },
            NOW + 7.0,
        )
        .unwrap_err();
        assert!(error.contains("只有 2 个段落"), "{error}");
    }

    #[test]
    fn version_list_includes_note_and_paragraph_diff_summary() {
        let (fixture, task, _outline) = ready_fixture("version-list");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        confirm(&fixture, &task, draft(2, "仍有三项试点待补齐。"), NOW + 5.0);
        let list = list_version_summaries(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(list.versions.len(), 3);
        assert_eq!(list.current_version, 3);
        assert!(list.versions.iter().all(|version| !version.note.trim().is_empty()));
        assert_eq!(list.versions[0].scope, VersionScope::Outline);
        assert_eq!(list.versions[0].note, "大纲确认");
        assert_eq!(list.versions[1].scope, VersionScope::Section);
        assert_eq!(list.versions[1].note, "虚构章节确认");
        assert!(list.versions[1].diff.is_none());
        let diff = list.versions[2].diff.as_ref().expect("第二版正文应有差异摘要");
        assert_eq!(diff.previous_version, 2);
        assert!(diff.added >= 1, "{diff:?}");
        assert!(diff.changed_paragraphs >= 1);
        assert!(list.versions[2].current);
    }

    #[test]
    fn version_diff_compares_body_versions_and_rejects_cross_kind() {
        let (fixture, task, _outline) = ready_fixture("version-diff");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        confirm(&fixture, &task, draft(2, "仍有三项试点待补齐。"), NOW + 5.0);
        let diff = diff_versions(
            fixture.work(),
            &VersionDiffRequest {
                writing_id: task.writing_id.clone(),
                from_version: 2,
                to_version: 3,
            },
        )
        .unwrap();
        assert_eq!(diff.from.version, 2);
        assert_eq!(diff.to.version, 3);
        assert!(!diff.diff.changes.is_empty());
        let error = diff_versions(
            fixture.work(),
            &VersionDiffRequest {
                writing_id: task.writing_id.clone(),
                from_version: 1,
                to_version: 2,
            },
        )
        .unwrap_err();
        assert!(error.contains("不同文稿类型"), "{error}");
        let error = diff_versions(
            fixture.work(),
            &VersionDiffRequest {
                writing_id: task.writing_id.clone(),
                from_version: 2,
                to_version: 99,
            },
        )
        .unwrap_err();
        assert!(error.contains("不在 manifest"), "{error}");
    }

    #[test]
    fn restore_requires_confirmation_and_keeps_status_with_trace() {
        let (fixture, task, _outline) = ready_fixture("restore");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        confirm(&fixture, &task, draft(2, "仍有三项试点待补齐。"), NOW + 5.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(task.status, writing_store::STATUS_FACT_CHECKING);
        let error = restore_version(
            fixture.work(),
            &VersionRestoreRequest {
                writing_id: task.writing_id.clone(),
                version: 2,
                confirmed: false,
            },
            NOW + 6.0,
        )
        .unwrap_err();
        assert!(error.contains("需要用户显式确认"), "{error}");
        assert_eq!(
            document_store::read_manifest(fixture.work(), &task.writing_id)
                .unwrap()
                .current_version,
            3
        );

        let restored = restore_version(
            fixture.work(),
            &VersionRestoreRequest {
                writing_id: task.writing_id.clone(),
                version: 2,
                confirmed: true,
            },
            NOW + 7.0,
        )
        .unwrap();
        assert_eq!(restored.restored_version, 2);
        assert_eq!(restored.new_version.version, 4);
        assert_eq!(restored.new_version.note, "恢复自版本 2");
        assert_eq!(restored.status, writing_store::STATUS_FACT_CHECKING);
        assert_eq!(
            restored.status_records.last().unwrap().reason,
            "恢复 v2"
        );
        let list = list_version_summaries(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(list.versions.len(), 4);
        assert_eq!(list.current_version, 4);
        assert!(list.versions.iter().any(|version| version.version == 2));
        assert!(list.versions.iter().any(|version| version.version == 3));
        let state = writing_section::read_section_state(fixture.work(), &task.writing_id, 2).unwrap();
        assert!(!state.confirmed);
    }

    #[test]
    fn scenario_eight_versions_queryable_restorable_and_history_kept() {
        let (fixture, task, _outline) = ready_fixture("scenario8");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        confirm(&fixture, &task, draft(2, "仍有三项试点待补齐。"), NOW + 5.0);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        let first = apply(
            &fixture,
            &task,
            finding(1, 1),
            "本期完成三处试点，覆盖两个班组。\n\n试点推进总体平稳。",
            NOW + 6.0,
        );
        assert_eq!(first.version, 4);
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        let second = apply(
            &fixture,
            &task,
            ReviewFinding {
                finding_id: "F2".to_string(),
                dimension: ReviewDimension::LanguageExpression,
                section_no: 1,
                paragraph_no: 2,
                description: "结论表述偏口语。".to_string(),
                suggestion: "改为书面表述。".to_string(),
            },
            "本期完成三处试点，覆盖两个班组。\n\n试点工作总体平稳推进。",
            NOW + 7.0,
        );
        assert_eq!(second.version, 5);

        let diff = diff_versions(
            fixture.work(),
            &VersionDiffRequest {
                writing_id: task.writing_id.clone(),
                from_version: 2,
                to_version: 4,
            },
        )
        .unwrap();
        assert!(!diff.diff.changes.is_empty());

        let restored = restore_version(
            fixture.work(),
            &VersionRestoreRequest {
                writing_id: task.writing_id.clone(),
                version: 2,
                confirmed: true,
            },
            NOW + 8.0,
        )
        .unwrap();
        assert_eq!(restored.new_version.version, 6);
        assert_eq!(restored.status, writing_store::STATUS_FACT_CHECKING);

        let list = list_version_summaries(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(list.current_version, 6);
        assert_eq!(list.versions.len(), 6);
        assert!(list.versions.iter().all(|version| !version.note.trim().is_empty()));
        for version in 1..=6 {
            assert!(
                list.versions.iter().any(|item| item.version == version),
                "版本 v{version} 不应丢失"
            );
        }
        assert_eq!(list.versions[5].note, "恢复自版本 2");
        assert!(list.versions.iter().any(|item| item.version == 4));
        assert!(list.versions.iter().any(|item| item.version == 5));

        let current = document_store::read_body(fixture.work(), &task.writing_id)
            .unwrap()
            .unwrap();
        assert!(current.contains("本期完成三处试点。"), "{current}");
        assert!(!current.contains("问题与不足"), "恢复后应回到 v2 的正文结构");
        let old = document_store::read_version(fixture.work(), &task.writing_id, 4).unwrap();
        assert!(old.content.contains("覆盖两个班组"), "历史版本不应被改写");
        let task = writing_store::read_writing_task(fixture.work(), &task.writing_id).unwrap();
        assert_eq!(task.status, writing_store::STATUS_FACT_CHECKING);
        assert!(task
            .status_records
            .iter()
            .any(|record| record.reason == "恢复 v2"));
    }

    #[test]
    fn restore_current_version_is_rejected() {
        let (fixture, task, _outline) = ready_fixture("restore-current");
        confirm(&fixture, &task, draft(1, "本期完成三处试点。\n\n试点推进总体平稳。"), NOW + 4.0);
        let error = restore_version(
            fixture.work(),
            &VersionRestoreRequest {
                writing_id: task.writing_id.clone(),
                version: 2,
                confirmed: true,
            },
            NOW + 5.0,
        )
        .unwrap_err();
        assert!(error.contains("就是当前版本"), "{error}");
    }
}
