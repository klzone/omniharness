//! 写作引用与核验记录（OD-WT-603）。
//!
//! 每个写作任务的记录放在 <work_root>/写作/<writing_id>/：citations.jsonl
//! 存段落引用，factcheck.jsonl 存事实核验结果。两者都是追加式 JSONL
//! （照 507 移除记录先例）：一条记录一行 JSON，追加时整行写入后
//! flush + fsync；不改写历史行，不引入数据库。
//!
//! 处理状态只能由用户动作改变：update_resolution 按行重写目标核验记录行，
//! 其余行字节不动。recompute_hint 是纯函数，只返回应存在的引用记录骨架，
//! 不落盘（609 消费）。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::sha256::sha256_hex;
use crate::worklog_records::{
    assert_work_root, is_safe_id, new_file_id, write_replace_atomically,
};

/// 写作任务目录（与 601/602 约定的 work/写作/ 根一致）。
pub const WRITING_DIR: &str = "写作";
pub const CITATIONS_FILE: &str = "citations.jsonl";
pub const FACTCHECK_FILE: &str = "factcheck.jsonl";

const MAX_SOURCE_ID_CHARS: usize = 200;
const MAX_QUOTE_CHARS: usize = 20_000;
const MAX_FACT_TEXT_CHARS: usize = 20_000;

/// 引用来源只有素材与知识两类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitationSourceKind {
    #[serde(rename = "material")]
    Material,
    #[serde(rename = "knowledge")]
    Knowledge,
}

impl CitationSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CitationSourceKind::Material => "material",
            CitationSourceKind::Knowledge => "knowledge",
        }
    }
}

/// 核验对象十类（D-086）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckObject {
    #[serde(rename = "时间")]
    Time,
    #[serde(rename = "数字")]
    Number,
    #[serde(rename = "机构名")]
    Institution,
    #[serde(rename = "人员职务")]
    PersonRole,
    #[serde(rename = "工作事项")]
    WorkItem,
    #[serde(rename = "完成情况")]
    Completion,
    #[serde(rename = "工作成果")]
    WorkResult,
    #[serde(rename = "政策依据")]
    PolicyBasis,
    #[serde(rename = "案例事实")]
    CaseFact,
    #[serde(rename = "统计口径")]
    StatisticsScope,
}

pub const CHECK_OBJECTS: &[CheckObject] = &[
    CheckObject::Time,
    CheckObject::Number,
    CheckObject::Institution,
    CheckObject::PersonRole,
    CheckObject::WorkItem,
    CheckObject::Completion,
    CheckObject::WorkResult,
    CheckObject::PolicyBasis,
    CheckObject::CaseFact,
    CheckObject::StatisticsScope,
];

impl CheckObject {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckObject::Time => "时间",
            CheckObject::Number => "数字",
            CheckObject::Institution => "机构名",
            CheckObject::PersonRole => "人员职务",
            CheckObject::WorkItem => "工作事项",
            CheckObject::Completion => "完成情况",
            CheckObject::WorkResult => "工作成果",
            CheckObject::PolicyBasis => "政策依据",
            CheckObject::CaseFact => "案例事实",
            CheckObject::StatisticsScope => "统计口径",
        }
    }

    pub fn from_label(label: &str) -> Result<Self, String> {
        let trimmed = label.trim();
        CHECK_OBJECTS
            .iter()
            .copied()
            .find(|item| item.as_str() == trimmed)
            .ok_or_else(|| {
                format!(
                    "核验对象不在白名单内：{trimmed:?}（允许：{}）",
                    labels_of(CHECK_OBJECTS, CheckObject::as_str)
                )
            })
    }
}

/// 核验结论八类（D-087）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckConclusion {
    #[serde(rename = "有直接来源支持")]
    DirectSource,
    #[serde(rename = "有间接来源支持")]
    IndirectSource,
    #[serde(rename = "来源存在冲突")]
    SourceConflict,
    #[serde(rename = "来源已过期")]
    SourceOutdated,
    #[serde(rename = "无法找到来源")]
    SourceMissing,
    #[serde(rename = "属于分析判断")]
    AnalysisJudgment,
    #[serde(rename = "属于工作计划")]
    WorkPlan,
    #[serde(rename = "属于表达性概括")]
    ExpressiveSummary,
}

pub const CHECK_CONCLUSIONS: &[CheckConclusion] = &[
    CheckConclusion::DirectSource,
    CheckConclusion::IndirectSource,
    CheckConclusion::SourceConflict,
    CheckConclusion::SourceOutdated,
    CheckConclusion::SourceMissing,
    CheckConclusion::AnalysisJudgment,
    CheckConclusion::WorkPlan,
    CheckConclusion::ExpressiveSummary,
];

impl CheckConclusion {
    pub fn as_str(self) -> &'static str {
        match self {
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

    pub fn from_label(label: &str) -> Result<Self, String> {
        let trimmed = label.trim();
        CHECK_CONCLUSIONS
            .iter()
            .copied()
            .find(|item| item.as_str() == trimmed)
            .ok_or_else(|| {
                format!(
                    "核验结论不在白名单内：{trimmed:?}（允许：{}）",
                    labels_of(CHECK_CONCLUSIONS, CheckConclusion::as_str)
                )
            })
    }

    /// 需要引用依据才算成立的结论；其余结论允许空依据。
    fn requires_basis(self) -> bool {
        matches!(
            self,
            CheckConclusion::DirectSource
                | CheckConclusion::IndirectSource
                | CheckConclusion::SourceConflict
                | CheckConclusion::SourceOutdated
        )
    }
}

/// 处理状态四类；只能经 update_resolution 由用户动作改变。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    #[serde(rename = "未处理")]
    Unresolved,
    #[serde(rename = "已接受")]
    Accepted,
    #[serde(rename = "已修改")]
    Modified,
    #[serde(rename = "已删除来源项")]
    SourceRemoved,
}

pub const RESOLUTION_STATUSES: &[ResolutionStatus] = &[
    ResolutionStatus::Unresolved,
    ResolutionStatus::Accepted,
    ResolutionStatus::Modified,
    ResolutionStatus::SourceRemoved,
];

impl ResolutionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ResolutionStatus::Unresolved => "未处理",
            ResolutionStatus::Accepted => "已接受",
            ResolutionStatus::Modified => "已修改",
            ResolutionStatus::SourceRemoved => "已删除来源项",
        }
    }

    pub fn from_label(label: &str) -> Result<Self, String> {
        let trimmed = label.trim();
        RESOLUTION_STATUSES
            .iter()
            .copied()
            .find(|item| item.as_str() == trimmed)
            .ok_or_else(|| {
                format!(
                    "处理状态不在白名单内：{trimmed:?}（允许：{}）",
                    labels_of(RESOLUTION_STATUSES, ResolutionStatus::as_str)
                )
            })
    }
}

fn labels_of<T: Copy>(items: &[T], label: fn(T) -> &'static str) -> String {
    items
        .iter()
        .map(|item| label(*item))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 段落引用记录（citations.jsonl 的一行）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationRecord {
    pub record_id: String,
    /// 章节号，1 起。
    pub section_no: u32,
    /// 段落序号，1 起。
    pub paragraph_no: u32,
    pub source_kind: CitationSourceKind,
    /// 素材 material_id 或知识库内相对路径。
    pub source_id: String,
    /// 引用原文（连续片段）。
    pub quote: String,
    pub linked_at: String,
}

/// 追加引用的输入；record_id 缺省时由宿主生成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationDraft {
    pub record_id: Option<String>,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
}

/// 核验依据：素材或知识条目 id + 引用原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactBasis {
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
}

/// 事实核验记录（factcheck.jsonl 的一行）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactCheckRecord {
    pub record_id: String,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub check_object: CheckObject,
    pub conclusion: CheckConclusion,
    #[serde(default)]
    pub basis: Vec<FactBasis>,
    /// 被核验的正文片段（可选；609 提取时填入）。
    #[serde(default)]
    pub fact_text: Option<String>,
    pub resolution: ResolutionStatus,
    pub checked_at: String,
}

/// 追加核验的输入；resolution 固定从「未处理」开始。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactCheckDraft {
    pub record_id: Option<String>,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub check_object: CheckObject,
    pub conclusion: CheckConclusion,
    pub basis: Vec<FactBasis>,
    pub fact_text: Option<String>,
}

/// 正文段落（全量重算输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphText {
    pub section_no: u32,
    pub paragraph_no: u32,
    pub text: String,
}

/// 素材/知识引用输入（全量重算输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationInput {
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
}

/// 应存在的引用记录骨架（不落盘）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationSkeleton {
    pub record_id: String,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
}

impl CitationSkeleton {
    /// 609 落盘时补上时间即可转成正式记录。
    pub fn to_record(&self, linked_at: &str) -> CitationRecord {
        CitationRecord {
            record_id: self.record_id.clone(),
            section_no: self.section_no,
            paragraph_no: self.paragraph_no,
            source_kind: self.source_kind,
            source_id: self.source_id.clone(),
            quote: self.quote.clone(),
            linked_at: linked_at.to_string(),
        }
    }
}

fn assert_writing_id(writing_id: &str) -> Result<(), String> {
    if !is_safe_id(writing_id) {
        return Err(format!("写作任务编号不合法：{writing_id:?}"));
    }
    Ok(())
}

fn assert_location(section_no: u32, paragraph_no: u32) -> Result<(), String> {
    if section_no == 0 {
        return Err("章节号必须从 1 开始。".to_string());
    }
    if paragraph_no == 0 {
        return Err("段落序号必须从 1 开始。".to_string());
    }
    Ok(())
}

fn validate_source_id(source_id: &str) -> Result<String, String> {
    let trimmed = source_id.trim();
    if trimmed.is_empty() {
        return Err("来源编号不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_SOURCE_ID_CHARS {
        return Err(format!(
            "来源编号过长（最多 {MAX_SOURCE_ID_CHARS} 字符）：{trimmed:?}"
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(format!("来源编号含控制字符：{trimmed:?}"));
    }
    Ok(trimmed.to_string())
}

fn validate_quote(quote: &str) -> Result<String, String> {
    let trimmed = quote.trim();
    if trimmed.is_empty() {
        return Err("引用原文不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_QUOTE_CHARS {
        return Err(format!("引用原文过长（最多 {MAX_QUOTE_CHARS} 字符）。"));
    }
    Ok(trimmed.to_string())
}

fn validate_record_id(record_id: &str) -> Result<(), String> {
    if !is_safe_id(record_id) {
        return Err(format!("记录编号不合法：{record_id:?}"));
    }
    Ok(())
}

/// 写作任务记录目录 <work_root>/写作/<writing_id>/。
pub fn writing_records_dir(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    assert_work_root(root)?;
    assert_writing_id(writing_id)?;
    Ok(root.join(WRITING_DIR).join(writing_id))
}

pub fn citations_path(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    Ok(writing_records_dir(root, writing_id)?.join(CITATIONS_FILE))
}

pub fn factcheck_path(root: &Path, writing_id: &str) -> Result<PathBuf, String> {
    Ok(writing_records_dir(root, writing_id)?.join(FACTCHECK_FILE))
}

fn append_json_line(path: &Path, line: &str) -> Result<(), String> {
    if line.contains('\n') || line.contains('\r') {
        return Err("JSONL 记录序列化后仍含换行，拒绝写入。".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("记录路径没有父目录：{}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建写作记录目录失败（{}）：{e}", parent.display()))?;

    if path.exists() {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("无法确认记录文件（{}）：{e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("记录文件不能是符号链接：{}", path.display()));
        }
        if !meta.is_file() {
            return Err(format!("记录路径不是普通文件：{}", path.display()));
        }
        if meta.len() > 0 {
            // 追加前确认旧文件以换行结尾；半截行不会被新记录掩盖。
            let mut file = fs::File::open(path)
                .map_err(|e| format!("读取记录文件失败（{}）：{e}", path.display()))?;
            file.seek(SeekFrom::End(-1))
                .map_err(|e| format!("定位记录文件尾部失败（{}）：{e}", path.display()))?;
            let mut last = [0u8; 1];
            file.read_exact(&mut last)
                .map_err(|e| format!("读取记录文件尾部失败（{}）：{e}", path.display()))?;
            if last[0] != b'\n' {
                return Err(format!(
                    "记录文件尾部不完整（{}），拒绝追加以免掩盖半截行。",
                    path.display()
                ));
            }
        }
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("打开记录文件失败（{}）：{e}", path.display()))?;
    // 整行（含换行）一次写入：O_APPEND 下 reducer 见到的永远是完整行。
    let mut payload = Vec::with_capacity(line.len() + 1);
    payload.extend_from_slice(line.as_bytes());
    payload.push(b'\n');
    file.write_all(&payload)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("追加记录失败（{}）：{e}", path.display()))
}

fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取写作记录失败（{}）：{e}", path.display()))?;
    let mut items = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let item = serde_json::from_str::<T>(line).map_err(|e| {
            format!(
                "写作记录第 {} 行不是合法 JSON（{}）：{e}",
                index + 1,
                path.display()
            )
        })?;
        items.push(item);
    }
    Ok(items)
}

fn validate_basis(
    basis: &[FactBasis],
    conclusion: CheckConclusion,
) -> Result<Vec<FactBasis>, String> {
    if conclusion.requires_basis() && basis.is_empty() {
        return Err(format!("结论「{}」必须带引用依据。", conclusion.as_str()));
    }
    let mut out = Vec::with_capacity(basis.len());
    for item in basis {
        out.push(FactBasis {
            source_kind: item.source_kind,
            source_id: validate_source_id(&item.source_id)?,
            quote: validate_quote(&item.quote)?,
        });
    }
    Ok(out)
}

/// 追加一条段落引用。linked_at 由宿主填 UTC ISO 8601。
pub fn append_citation(
    root: &Path,
    writing_id: &str,
    draft: &CitationDraft,
    now: f64,
) -> Result<CitationRecord, String> {
    assert_location(draft.section_no, draft.paragraph_no)?;
    let source_id = validate_source_id(&draft.source_id)?;
    let quote = validate_quote(&draft.quote)?;
    let record_id = match &draft.record_id {
        Some(id) => {
            validate_record_id(id)?;
            id.clone()
        }
        None => new_file_id("cite", now),
    };
    let record = CitationRecord {
        record_id,
        section_no: draft.section_no,
        paragraph_no: draft.paragraph_no,
        source_kind: draft.source_kind,
        source_id,
        quote,
        linked_at: crate::task_shell::utc_iso8601(now),
    };
    let line = serde_json::to_string(&record).map_err(|e| format!("序列化引用记录失败：{e}"))?;
    append_json_line(&citations_path(root, writing_id)?, &line)?;
    Ok(record)
}

/// 追加一条事实核验记录；处理状态固定为「未处理」。
pub fn append_fact_check(
    root: &Path,
    writing_id: &str,
    draft: &FactCheckDraft,
    now: f64,
) -> Result<FactCheckRecord, String> {
    assert_location(draft.section_no, draft.paragraph_no)?;
    let basis = validate_basis(&draft.basis, draft.conclusion)?;
    let fact_text = match &draft.fact_text {
        Some(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                if trimmed.chars().count() > MAX_FACT_TEXT_CHARS {
                    return Err(format!(
                        "核验片段过长（最多 {MAX_FACT_TEXT_CHARS} 字符）。"
                    ));
                }
                Some(trimmed.to_string())
            }
        }
        None => None,
    };
    let record_id = match &draft.record_id {
        Some(id) => {
            validate_record_id(id)?;
            id.clone()
        }
        None => new_file_id("fact", now),
    };
    let record = FactCheckRecord {
        record_id,
        section_no: draft.section_no,
        paragraph_no: draft.paragraph_no,
        check_object: draft.check_object,
        conclusion: draft.conclusion,
        basis,
        fact_text,
        resolution: ResolutionStatus::Unresolved,
        checked_at: crate::task_shell::utc_iso8601(now),
    };
    let line = serde_json::to_string(&record).map_err(|e| format!("序列化核验记录失败：{e}"))?;
    append_json_line(&factcheck_path(root, writing_id)?, &line)?;
    Ok(record)
}

pub fn list_citations(root: &Path, writing_id: &str) -> Result<Vec<CitationRecord>, String> {
    read_jsonl(&citations_path(root, writing_id)?)
}

/// 按段落查询引用；章节号与段落序号都精确匹配。
pub fn list_citations_for_paragraph(
    root: &Path,
    writing_id: &str,
    section_no: u32,
    paragraph_no: u32,
) -> Result<Vec<CitationRecord>, String> {
    assert_location(section_no, paragraph_no)?;
    Ok(list_citations(root, writing_id)?
        .into_iter()
        .filter(|record| record.section_no == section_no && record.paragraph_no == paragraph_no)
        .collect())
}

/// 按来源查询引用（素材或知识）。
pub fn list_citations_for_source(
    root: &Path,
    writing_id: &str,
    source_kind: CitationSourceKind,
    source_id: &str,
) -> Result<Vec<CitationRecord>, String> {
    let source_id = validate_source_id(source_id)?;
    Ok(list_citations(root, writing_id)?
        .into_iter()
        .filter(|record| record.source_kind == source_kind && record.source_id == source_id)
        .collect())
}

pub fn list_fact_checks(root: &Path, writing_id: &str) -> Result<Vec<FactCheckRecord>, String> {
    read_jsonl(&factcheck_path(root, writing_id)?)
}

pub fn list_fact_checks_for_paragraph(
    root: &Path,
    writing_id: &str,
    section_no: u32,
    paragraph_no: u32,
) -> Result<Vec<FactCheckRecord>, String> {
    assert_location(section_no, paragraph_no)?;
    Ok(list_fact_checks(root, writing_id)?
        .into_iter()
        .filter(|record| record.section_no == section_no && record.paragraph_no == paragraph_no)
        .collect())
}

/// 按八类结论查询核验结果。
pub fn list_fact_checks_for_conclusion(
    root: &Path,
    writing_id: &str,
    conclusion: CheckConclusion,
) -> Result<Vec<FactCheckRecord>, String> {
    Ok(list_fact_checks(root, writing_id)?
        .into_iter()
        .filter(|record| record.conclusion == conclusion)
        .collect())
}

/// 同上，接受中文标签字符串，供 IPC 层直接转发。
pub fn list_fact_checks_for_conclusion_label(
    root: &Path,
    writing_id: &str,
    conclusion: &str,
) -> Result<Vec<FactCheckRecord>, String> {
    list_fact_checks_for_conclusion(root, writing_id, CheckConclusion::from_label(conclusion)?)
}

/// 用户处理核验项：校验状态枚举后按行重写目标记录，其余行字节保留。
pub fn update_resolution(
    root: &Path,
    writing_id: &str,
    record_id: &str,
    resolution: &str,
) -> Result<FactCheckRecord, String> {
    let next = ResolutionStatus::from_label(resolution)?;
    validate_record_id(record_id)?;
    let path = factcheck_path(root, writing_id)?;
    if !path.exists() {
        return Err(format!("核验记录文件不存在：{}", path.display()));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取核验记录失败（{}）：{e}", path.display()))?;
    let mut output = String::with_capacity(raw.len());
    let mut updated: Option<FactCheckRecord> = None;
    for line in raw.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        if !body.trim().is_empty() {
            if let Ok(mut record) = serde_json::from_str::<FactCheckRecord>(body) {
                if record.record_id == record_id {
                    record.resolution = next;
                    let serialized = serde_json::to_string(&record)
                        .map_err(|e| format!("序列化核验记录失败：{e}"))?;
                    output.push_str(&serialized);
                    output.push('\n');
                    updated = Some(record);
                    continue;
                }
            }
        }
        output.push_str(line);
    }
    let updated = updated.ok_or_else(|| format!("找不到核验记录：{record_id}"))?;
    write_replace_atomically(&path, output.as_bytes())?;
    Ok(updated)
}

fn normalized_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn skeleton_id(section_no: u32, paragraph_no: u32, input: &CitationInput, quote: &str) -> String {
    let key = format!(
        "{section_no}\u{1f}{paragraph_no}\u{1f}{}\u{1f}{}\u{1f}{quote}",
        input.source_kind.as_str(),
        input.source_id.trim()
    );
    let hash = sha256_hex(key.as_bytes());
    format!("cite-{}", hash.chars().take(16).collect::<String>())
}

/// 全量重算骨架：正文段落含引用原文时，返回该段落应存在的引用记录。
///
/// 纯函数、不读写文件；按章节/段落/来源排序，重复输入只留一条。匹配时
/// 去掉全部空白，避免排版换行造成漏配。609 负责补齐 linked_at 后落盘。
pub fn recompute_hint(
    paragraphs: &[ParagraphText],
    citations: &[CitationInput],
) -> Vec<CitationSkeleton> {
    let mut skeletons: Vec<CitationSkeleton> = Vec::new();
    for paragraph in paragraphs {
        let paragraph_text = normalized_text(&paragraph.text);
        if paragraph_text.is_empty() {
            continue;
        }
        for input in citations {
            let quote = input.quote.trim();
            let quote_key = normalized_text(quote);
            if quote_key.is_empty() || !paragraph_text.contains(&quote_key) {
                continue;
            }
            let record_id = skeleton_id(
                paragraph.section_no,
                paragraph.paragraph_no,
                input,
                &quote_key,
            );
            let skeleton = CitationSkeleton {
                record_id,
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                source_kind: input.source_kind,
                source_id: input.source_id.trim().to_string(),
                quote: quote.to_string(),
            };
            if !skeletons
                .iter()
                .any(|existing| existing.record_id == skeleton.record_id)
            {
                skeletons.push(skeleton);
            }
        }
    }
    skeletons.sort_by(|a, b| {
        (a.section_no, a.paragraph_no)
            .cmp(&(b.section_no, b.paragraph_no))
            .then_with(|| a.source_kind.as_str().cmp(b.source_kind.as_str()))
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.quote.cmp(&b.quote))
    });
    skeletons
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_795_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wt603-writing-records-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn citation(section: u32, paragraph: u32, quote: &str) -> CitationDraft {
        CitationDraft {
            record_id: None,
            section_no: section,
            paragraph_no: paragraph,
            source_kind: CitationSourceKind::Material,
            source_id: "mat-synthetic-001".to_string(),
            quote: quote.to_string(),
        }
    }

    fn basis() -> Vec<FactBasis> {
        vec![FactBasis {
            source_kind: CitationSourceKind::Material,
            source_id: "mat-synthetic-001".to_string(),
            quote: "虚构来源引用。".to_string(),
        }]
    }

    fn fact(conclusion: CheckConclusion) -> FactCheckDraft {
        FactCheckDraft {
            record_id: None,
            section_no: 1,
            paragraph_no: 2,
            check_object: CheckObject::Number,
            conclusion,
            basis: if conclusion.requires_basis() {
                basis()
            } else {
                Vec::new()
            },
            fact_text: Some("虚构核验片段。".to_string()),
        }
    }

    #[test]
    fn append_citation_writes_one_jsonl_line_and_roundtrips() {
        let temp = TempRoot::new("append-cite");
        let record =
            append_citation(&temp.path, "wt-603-a", &citation(1, 1, "虚构引用原文。"), NOW)
                .unwrap();
        assert_eq!(record.linked_at, crate::task_shell::utc_iso8601(NOW));
        let path = citations_path(&temp.path, "wt-603-a").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw.matches('\n').count(), 1, "{raw}");
        assert!(raw.ends_with('\n'));
        assert_eq!(raw.lines().count(), 1);
        let listed = list_citations(&temp.path, "wt-603-a").unwrap();
        assert_eq!(listed, vec![record]);
    }

    #[test]
    fn append_citation_rejects_empty_quote_and_unsafe_source() {
        let temp = TempRoot::new("cite-invalid");
        let mut draft = citation(1, 1, "   ");
        let err = append_citation(&temp.path, "wt-603-a", &draft, NOW).unwrap_err();
        assert!(err.contains("引用原文"), "{err}");
        draft.quote = "虚构引用原文。".to_string();
        draft.source_id = "bad\nid".to_string();
        let err = append_citation(&temp.path, "wt-603-a", &draft, NOW).unwrap_err();
        assert!(err.contains("控制字符"), "{err}");
        assert!(!citations_path(&temp.path, "wt-603-a").unwrap().exists());
    }

    #[test]
    fn append_citation_honors_caller_record_id() {
        let temp = TempRoot::new("cite-id");
        let mut draft = citation(2, 3, "虚构引用原文。");
        draft.record_id = Some("cite-user-001".to_string());
        let record = append_citation(&temp.path, "wt-603-b", &draft, NOW).unwrap();
        assert_eq!(record.record_id, "cite-user-001");
        let listed = list_citations(&temp.path, "wt-603-b").unwrap();
        assert_eq!(listed[0].record_id, "cite-user-001");
    }

    #[test]
    fn append_refuses_truncated_tail_without_touching_bytes() {
        let temp = TempRoot::new("cite-truncated");
        let path = citations_path(&temp.path, "wt-603-c").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"record_id\":\"cite-1\"").unwrap();
        let before = fs::read(&path).unwrap();
        let err =
            append_citation(&temp.path, "wt-603-c", &citation(1, 1, "虚构引用原文。"), NOW)
                .unwrap_err();
        assert!(err.contains("尾部不完整"), "{err}");
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn citations_query_by_paragraph_and_section() {
        let temp = TempRoot::new("cite-para");
        append_citation(&temp.path, "wt-603-d", &citation(1, 1, "第一段引用。"), NOW).unwrap();
        append_citation(&temp.path, "wt-603-d", &citation(1, 2, "第二段引用。"), NOW).unwrap();
        append_citation(&temp.path, "wt-603-d", &citation(2, 1, "另一章引用。"), NOW).unwrap();
        let one = list_citations_for_paragraph(&temp.path, "wt-603-d", 1, 1).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].quote, "第一段引用。");
        let chapter_one = list_citations(&temp.path, "wt-603-d")
            .unwrap()
            .into_iter()
            .filter(|record| record.section_no == 1)
            .count();
        assert_eq!(chapter_one, 2);
    }

    #[test]
    fn citations_query_by_source_kind_and_id() {
        let temp = TempRoot::new("cite-source");
        let mut material = citation(1, 1, "素材引用。");
        material.source_id = "mat-synthetic-001".to_string();
        append_citation(&temp.path, "wt-603-e", &material, NOW).unwrap();
        let mut knowledge = citation(1, 1, "知识引用。");
        knowledge.source_kind = CitationSourceKind::Knowledge;
        knowledge.source_id = "notes/synthetic.md".to_string();
        append_citation(&temp.path, "wt-603-e", &knowledge, NOW).unwrap();
        let found = list_citations_for_source(
            &temp.path,
            "wt-603-e",
            CitationSourceKind::Knowledge,
            "notes/synthetic.md",
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].quote, "知识引用。");
        let none = list_citations_for_source(
            &temp.path,
            "wt-603-e",
            CitationSourceKind::Material,
            "mat-synthetic-002",
        )
        .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn missing_record_files_read_as_empty_for_queries() {
        let temp = TempRoot::new("missing");
        assert!(list_citations(&temp.path, "wt-603-f").unwrap().is_empty());
        assert!(list_fact_checks(&temp.path, "wt-603-f").unwrap().is_empty());
        let citations = list_citations_for_paragraph(&temp.path, "wt-603-f", 1, 1).unwrap();
        assert!(citations.is_empty());
    }

    #[test]
    fn append_fact_check_roundtrips_fields_and_defaults_unresolved() {
        let temp = TempRoot::new("fact-append");
        let draft = FactCheckDraft {
            record_id: Some("fact-user-001".to_string()),
            section_no: 2,
            paragraph_no: 4,
            check_object: CheckObject::Time,
            conclusion: CheckConclusion::DirectSource,
            basis: basis(),
            fact_text: Some("2026 年 3 月完成虚构试点。".to_string()),
        };
        let record = append_fact_check(&temp.path, "wt-603-g", &draft, NOW).unwrap();
        assert_eq!(record.resolution, ResolutionStatus::Unresolved);
        assert_eq!(record.checked_at, crate::task_shell::utc_iso8601(NOW));
        let listed = list_fact_checks(&temp.path, "wt-603-g").unwrap();
        assert_eq!(listed, vec![record]);
        let raw = fs::read_to_string(factcheck_path(&temp.path, "wt-603-g").unwrap()).unwrap();
        assert!(raw.contains("\"check_object\":\"时间\""), "{raw}");
        assert!(raw.contains("\"resolution\":\"未处理\""), "{raw}");
    }

    #[test]
    fn object_and_conclusion_labels_cover_whitelists() {
        assert_eq!(CHECK_OBJECTS.len(), 10);
        assert_eq!(CHECK_CONCLUSIONS.len(), 8);
        for object in CHECK_OBJECTS {
            assert_eq!(CheckObject::from_label(object.as_str()).unwrap(), *object);
        }
        for conclusion in CHECK_CONCLUSIONS {
            assert_eq!(
                CheckConclusion::from_label(conclusion.as_str()).unwrap(),
                *conclusion
            );
        }
        assert!(CheckObject::from_label("其他").is_err());
        assert!(CheckConclusion::from_label("已核实").is_err());
        assert_eq!(RESOLUTION_STATUSES.len(), 4);
    }

    #[test]
    fn update_resolution_rewrites_only_target_line() {
        let temp = TempRoot::new("resolve");
        let first = append_fact_check(
            &temp.path,
            "wt-603-h",
            &fact(CheckConclusion::DirectSource),
            NOW,
        )
        .unwrap();
        let second = append_fact_check(
            &temp.path,
            "wt-603-h",
            &fact(CheckConclusion::SourceMissing),
            NOW,
        )
        .unwrap();
        let path = factcheck_path(&temp.path, "wt-603-h").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let updated =
            update_resolution(&temp.path, "wt-603-h", &first.record_id, "已接受").unwrap();
        assert_eq!(updated.resolution, ResolutionStatus::Accepted);
        let after = fs::read_to_string(&path).unwrap();
        let untouched_second = serde_json::to_string(&second).unwrap();
        assert!(after.contains(&untouched_second), "{after}");
        let untouched_first = serde_json::to_string(&first).unwrap();
        assert!(before.contains(&untouched_first));
        assert_eq!(after.lines().count(), 2);
        let listed = list_fact_checks(&temp.path, "wt-603-h").unwrap();
        assert_eq!(listed[0].resolution, ResolutionStatus::Accepted);
        assert_eq!(listed[1].resolution, ResolutionStatus::Unresolved);
    }

    #[test]
    fn update_resolution_rejects_unknown_status_and_record() {
        let temp = TempRoot::new("resolve-invalid");
        let record =
            append_fact_check(&temp.path, "wt-603-i", &fact(CheckConclusion::WorkPlan), NOW)
                .unwrap();
        let err =
            update_resolution(&temp.path, "wt-603-i", &record.record_id, "待处理").unwrap_err();
        assert!(err.contains("白名单"), "{err}");
        let err = update_resolution(&temp.path, "wt-603-i", "fact-missing", "已接受").unwrap_err();
        assert!(err.contains("找不到"), "{err}");
        let err = update_resolution(&temp.path, "wt-603-i", "bad id!", "已接受").unwrap_err();
        assert!(err.contains("记录编号"), "{err}");
    }

    #[test]
    fn update_resolution_preserves_other_lines_byte_for_byte() {
        let temp = TempRoot::new("resolve-preserve");
        let path = factcheck_path(&temp.path, "wt-603-j").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let record = FactCheckRecord {
            record_id: "fact-target".to_string(),
            section_no: 1,
            paragraph_no: 1,
            check_object: CheckObject::CaseFact,
            conclusion: CheckConclusion::SourceMissing,
            basis: Vec::new(),
            fact_text: None,
            resolution: ResolutionStatus::Unresolved,
            checked_at: crate::task_shell::utc_iso8601(NOW),
        };
        let malformed = "not-json-tail";
        let raw = format!(
            "{}\n\n{malformed}",
            serde_json::to_string(&record).unwrap()
        );
        fs::write(&path, &raw).unwrap();
        let updated =
            update_resolution(&temp.path, "wt-603-j", "fact-target", "已修改").unwrap();
        assert_eq!(updated.resolution, ResolutionStatus::Modified);
        let after = fs::read_to_string(&path).unwrap();
        assert!(
            after.starts_with(&format!(
                "{}\n\n{malformed}",
                serde_json::to_string(&updated).unwrap()
            )),
            "{after}"
        );
    }

    #[test]
    fn fact_checks_query_by_conclusion_and_paragraph() {
        let temp = TempRoot::new("fact-query");
        append_fact_check(
            &temp.path,
            "wt-603-k",
            &fact(CheckConclusion::DirectSource),
            NOW,
        )
        .unwrap();
        let missing = append_fact_check(
            &temp.path,
            "wt-603-k",
            &fact(CheckConclusion::SourceMissing),
            NOW,
        )
        .unwrap();
        let by_conclusion =
            list_fact_checks_for_conclusion(&temp.path, "wt-603-k", CheckConclusion::SourceMissing)
                .unwrap();
        assert_eq!(by_conclusion.len(), 1);
        assert_eq!(by_conclusion[0].record_id, missing.record_id);
        let by_label =
            list_fact_checks_for_conclusion_label(&temp.path, "wt-603-k", "无法找到来源").unwrap();
        assert_eq!(by_label.len(), 1);
        let by_paragraph = list_fact_checks_for_paragraph(&temp.path, "wt-603-k", 1, 2).unwrap();
        assert_eq!(by_paragraph.len(), 2);
        assert!(list_fact_checks_for_paragraph(&temp.path, "wt-603-k", 1, 1)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn supported_conclusions_require_basis() {
        let temp = TempRoot::new("fact-basis");
        for conclusion in [
            CheckConclusion::DirectSource,
            CheckConclusion::IndirectSource,
            CheckConclusion::SourceConflict,
            CheckConclusion::SourceOutdated,
        ] {
            let mut draft = fact(conclusion);
            draft.basis = Vec::new();
            let err = append_fact_check(&temp.path, "wt-603-l", &draft, NOW).unwrap_err();
            assert!(err.contains("必须带引用依据"), "{err}");
        }
        for conclusion in [
            CheckConclusion::SourceMissing,
            CheckConclusion::AnalysisJudgment,
            CheckConclusion::WorkPlan,
            CheckConclusion::ExpressiveSummary,
        ] {
            let record =
                append_fact_check(&temp.path, "wt-603-l", &fact(conclusion), NOW).unwrap();
            assert_eq!(record.conclusion, conclusion);
            assert!(record.basis.is_empty());
        }
    }

    #[test]
    fn recompute_hint_finds_quotes_and_is_deterministic() {
        let paragraphs = vec![
            ParagraphText {
                section_no: 1,
                paragraph_no: 1,
                text: "本期完成第一轮试点，覆盖 3 个虚构班组。".to_string(),
            },
            ParagraphText {
                section_no: 1,
                paragraph_no: 2,
                text: "下一步扩大范围。".to_string(),
            },
        ];
        let citations = vec![CitationInput {
            source_kind: CitationSourceKind::Material,
            source_id: "mat-synthetic-001".to_string(),
            quote: "完成第一轮试点".to_string(),
        }];
        let first = recompute_hint(&paragraphs, &citations);
        let second = recompute_hint(&paragraphs, &citations);
        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].section_no, 1);
        assert_eq!(first[0].paragraph_no, 1);
        assert_eq!(first[0].source_id, "mat-synthetic-001");
        assert!(first[0].record_id.starts_with("cite-"));
        let record = first[0].to_record("2026-03-01T00:00:00Z");
        assert_eq!(record.quote, "完成第一轮试点");
    }

    #[test]
    fn recompute_hint_normalizes_whitespace_and_dedupes() {
        let paragraphs = vec![ParagraphText {
            section_no: 2,
            paragraph_no: 3,
            text: "试点\n覆盖 3 个 虚构班组".to_string(),
        }];
        let input = CitationInput {
            source_kind: CitationSourceKind::Knowledge,
            source_id: "notes/synthetic.md".to_string(),
            quote: "试点 覆盖 3 个虚构班组".to_string(),
        };
        let duplicates = vec![input.clone(), input.clone()];
        let hint = recompute_hint(&paragraphs, &duplicates);
        assert_eq!(hint.len(), 1);
        assert_eq!(hint[0].quote, "试点 覆盖 3 个虚构班组");
    }

    #[test]
    fn recompute_hint_skips_absent_and_empty_quotes() {
        let paragraphs = vec![ParagraphText {
            section_no: 1,
            paragraph_no: 1,
            text: "虚构正文".to_string(),
        }];
        let citations = vec![
            CitationInput {
                source_kind: CitationSourceKind::Material,
                source_id: "mat-synthetic-001".to_string(),
                quote: "不存在的片段".to_string(),
            },
            CitationInput {
                source_kind: CitationSourceKind::Material,
                source_id: "mat-synthetic-002".to_string(),
                quote: "   ".to_string(),
            },
        ];
        assert!(recompute_hint(&paragraphs, &citations).is_empty());
    }

    #[test]
    fn record_paths_require_safe_writing_id() {
        let temp = TempRoot::new("paths");
        assert!(writing_records_dir(&temp.path, "../escape").is_err());
        assert!(writing_records_dir(&temp.path, "wt 603").is_err());
        let ok = writing_records_dir(&temp.path, "wt-603-ok").unwrap();
        assert!(ok.starts_with(&temp.path));
        assert_eq!(
            ok.file_name().and_then(|name| name.to_str()),
            Some("wt-603-ok")
        );
    }
}
