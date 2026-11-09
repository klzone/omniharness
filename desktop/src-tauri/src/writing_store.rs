//! 写作任务实体（OD-WT-601）。
//!
//! 每个写作任务一个 Markdown 文件：<work_root>/写作/<writing_id>.md。
//! frontmatter 存任务字段，正文只有 `## 状态记录` 追加段；状态记录只追加。
//! 九态与十二类文体都是有限白名单；状态只能经 `transition_writing_status`
//! 按合法性表推进，非法迁移拒绝并把拒绝尝试追加到状态记录。
//! 不引入数据库，不引用知识库写路径，不改变前三阶段交付结构。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{
    assert_work_root, is_date_shape, is_safe_id, new_file_id, normalize_opt, split_frontmatter,
    write_new_atomically, write_replace_atomically, yaml_opt, yaml_scalar, TRASH_DIR,
};

pub const WRITING_SCHEMA: &str = "omnidesk.writing.v1";
pub const WRITING_DIR: &str = "写作";

pub const STATUS_NEEDS_ORGANIZING: &str = "需求待整理";
pub const STATUS_COLLECTING_MATERIALS: &str = "素材收集中";
pub const STATUS_OUTLINE_PENDING: &str = "大纲待确认";
pub const STATUS_WRITING: &str = "写作中";
pub const STATUS_FACT_CHECKING: &str = "待核验";
pub const STATUS_PENDING_REVIEW: &str = "待审核";
pub const STATUS_FINALIZED: &str = "已定稿";
pub const STATUS_ARCHIVED: &str = "已归档";
pub const STATUS_CANCELLED: &str = "已取消";
pub const WRITING_STATUSES: &[&str] = &[
    STATUS_NEEDS_ORGANIZING,
    STATUS_COLLECTING_MATERIALS,
    STATUS_OUTLINE_PENDING,
    STATUS_WRITING,
    STATUS_FACT_CHECKING,
    STATUS_PENDING_REVIEW,
    STATUS_FINALIZED,
    STATUS_ARCHIVED,
    STATUS_CANCELLED,
];

pub const TYPE_MONTHLY_SUMMARY: &str = "月度总结";
pub const TYPE_ANNUAL_SUMMARY: &str = "年度总结";
pub const TYPE_SPECIAL_REPORT: &str = "专项汇报";
pub const TYPE_EXPERIENCE_EXCHANGE: &str = "经验交流";
pub const TYPE_TYPICAL_CASE: &str = "典型案例";
pub const TYPE_RESEARCH_REPORT: &str = "调研报告";
pub const TYPE_WORK_PLAN: &str = "工作方案";
pub const TYPE_LEADER_SPEECH: &str = "领导讲话";
pub const TYPE_WORK_BRIEF: &str = "工作简报";
pub const TYPE_TOPIC_ANALYSIS: &str = "专题分析";
pub const TYPE_MEETING_SPEECH: &str = "会议发言";
pub const TYPE_PERSONAL_REPORT: &str = "个人述职";
pub const DOCUMENT_TYPES: &[&str] = &[
    TYPE_MONTHLY_SUMMARY,
    TYPE_ANNUAL_SUMMARY,
    TYPE_SPECIAL_REPORT,
    TYPE_EXPERIENCE_EXCHANGE,
    TYPE_TYPICAL_CASE,
    TYPE_RESEARCH_REPORT,
    TYPE_WORK_PLAN,
    TYPE_LEADER_SPEECH,
    TYPE_WORK_BRIEF,
    TYPE_TOPIC_ANALYSIS,
    TYPE_MEETING_SPEECH,
    TYPE_PERSONAL_REPORT,
];

pub const ACTION_CREATE: &str = "创建";
pub const ACTION_EDIT: &str = "编辑";
pub const ACTION_TRANSITION: &str = "迁移";
pub const ACTION_REJECT: &str = "拒绝迁移";

const MAX_ID_CHARS: usize = 80;
const MAX_TITLE_CHARS: usize = 200;
const MAX_TEXT_CHARS: usize = 20_000;
const MAX_LINK_ID_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusRecord {
    pub at: String,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritingTask {
    pub schema: String,
    pub writing_id: String,
    pub title: String,
    pub document_type: String,
    pub writing_purpose: String,
    pub audience: String,
    pub writing_period: String,
    pub topic: String,
    pub requirements: String,
    pub expected_length: String,
    pub writing_style: String,
    pub selected_material_ids: Vec<String>,
    pub selected_knowledge_ids: Vec<String>,
    pub document_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    /// 不落 frontmatter，写在正文追加段；IPC 仍返回给界面。
    #[serde(default)]
    pub status_records: Vec<StatusRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingTaskDraft {
    pub writing_id: Option<String>,
    pub title: String,
    pub document_type: String,
    pub writing_purpose: String,
    pub audience: String,
    pub writing_period: String,
    pub topic: String,
    pub requirements: String,
    pub expected_length: String,
    pub writing_style: String,
    pub selected_material_ids: Vec<String>,
    pub selected_knowledge_ids: Vec<String>,
    pub document_id: Option<String>,
}

/// 编辑补丁。None = 不修改；空字符串 = 清空可选项。`reason` 写进状态记录。
/// 状态不在补丁里，只能经 `transition_writing_status` 推进。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingTaskPatch {
    pub title: Option<String>,
    pub document_type: Option<String>,
    pub writing_purpose: Option<String>,
    pub audience: Option<String>,
    pub writing_period: Option<String>,
    pub topic: Option<String>,
    pub requirements: Option<String>,
    pub expected_length: Option<String>,
    pub writing_style: Option<String>,
    pub selected_material_ids: Option<Vec<String>>,
    pub selected_knowledge_ids: Option<Vec<String>>,
    pub document_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingTaskFilter {
    pub status: Option<String>,
    pub document_type: Option<String>,
    /// 创建时间闭区间，YYYY-MM-DD；只填一端也可。
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WritingTaskListing {
    pub items: Vec<WritingTask>,
    pub broken: usize,
}

pub fn writing_dir(root: &Path) -> PathBuf {
    root.join(WRITING_DIR)
}

pub fn writing_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) || id.chars().count() > MAX_ID_CHARS {
        return Err(format!("写作任务编号不合法：{id:?}"));
    }
    Ok(writing_dir(root).join(format!("{id}.md")))
}

pub fn trash_writing_dir(root: &Path) -> PathBuf {
    root.join(TRASH_DIR).join(WRITING_DIR)
}

fn validate_document_type(value: &str) -> Result<(), String> {
    if !DOCUMENT_TYPES.contains(&value) {
        return Err(format!("文稿类型不认识：{value}"));
    }
    Ok(())
}

fn validate_status(value: &str) -> Result<(), String> {
    if !WRITING_STATUSES.contains(&value) {
        return Err(format!("写作状态不认识：{value}"));
    }
    Ok(())
}

fn normalize_link_ids(ids: &[String], field: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_LINK_ID_CHARS
            || trimmed.chars().any(|c| c.is_control())
        {
            return Err(format!("{field}编号不合法：{trimmed:?}"));
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

fn validate_text(value: &str, field: &str) -> Result<(), String> {
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("{field}过长（最多 {MAX_TEXT_CHARS} 字）。"));
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(format!("{field}含不允许的控制字符。"));
    }
    Ok(())
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
        && is_date_shape(&value[0..10])
}

fn validate_writing_task(task: &WritingTask) -> Result<(), String> {
    if task.schema != WRITING_SCHEMA {
        return Err(format!("写作任务 schema 不认识：{}", task.schema));
    }
    if !is_safe_id(&task.writing_id) || task.writing_id.chars().count() > MAX_ID_CHARS {
        return Err(format!("写作任务编号不合法：{:?}", task.writing_id));
    }
    if task.title.trim().is_empty() {
        return Err("写作任务标题不能为空。".to_string());
    }
    if task.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("写作任务标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
    }
    for (value, field) in [
        (&task.writing_purpose, "写作目的"),
        (&task.audience, "受众"),
        (&task.writing_period, "写作周期"),
        (&task.topic, "主题"),
        (&task.requirements, "写作要求"),
        (&task.expected_length, "预计篇幅"),
        (&task.writing_style, "写作风格"),
    ] {
        validate_text(value, field)?;
    }
    validate_document_type(&task.document_type)?;
    normalize_link_ids(&task.selected_material_ids, "选中素材")?;
    normalize_link_ids(&task.selected_knowledge_ids, "选中知识")?;
    if let Some(id) = task.document_id.as_deref() {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err("文档编号不能为空。".to_string());
        }
        if trimmed.chars().count() > MAX_LINK_ID_CHARS
            || trimmed.chars().any(|c| c.is_control())
        {
            return Err(format!("文档编号不合法：{trimmed:?}"));
        }
    }
    validate_status(&task.status)?;
    if !is_timestamp_shape(&task.created_at) {
        return Err(format!("创建时间形状不正确：{}", task.created_at));
    }
    if !is_timestamp_shape(&task.updated_at) {
        return Err(format!("更新时间形状不正确：{}", task.updated_at));
    }
    for record in &task.status_records {
        if !is_timestamp_shape(&record.at) {
            return Err(format!("状态记录时间形状不正确：{}", record.at));
        }
        if record.action.trim().is_empty() {
            return Err("状态记录缺少动作。".to_string());
        }
    }
    Ok(())
}

fn render_list(fm: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        fm.push_str(&format!("{key}: []\n"));
        return;
    }
    fm.push_str(&format!("{key}:\n"));
    for value in values {
        fm.push_str(&format!("  - {}\n", yaml_scalar(value)));
    }
}

fn sanitize_record_text(value: &str) -> String {
    value.replace('\r', " ").replace('\n', " ").trim().to_string()
}

fn sanitize_action(value: &str) -> String {
    sanitize_record_text(value).replace('|', "／")
}

fn render_writing_task(task: &WritingTask) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(&task.schema)));
    fm.push_str(&format!("writing_id: {}\n", yaml_scalar(&task.writing_id)));
    fm.push_str(&format!("title: {}\n", yaml_scalar(&task.title)));
    fm.push_str(&format!(
        "document_type: {}\n",
        yaml_scalar(&task.document_type)
    ));
    fm.push_str(&format!(
        "writing_purpose: {}\n",
        yaml_scalar(&task.writing_purpose)
    ));
    fm.push_str(&format!("audience: {}\n", yaml_scalar(&task.audience)));
    fm.push_str(&format!(
        "writing_period: {}\n",
        yaml_scalar(&task.writing_period)
    ));
    fm.push_str(&format!("topic: {}\n", yaml_scalar(&task.topic)));
    fm.push_str(&format!(
        "requirements: {}\n",
        yaml_scalar(&task.requirements)
    ));
    fm.push_str(&format!(
        "expected_length: {}\n",
        yaml_scalar(&task.expected_length)
    ));
    fm.push_str(&format!(
        "writing_style: {}\n",
        yaml_scalar(&task.writing_style)
    ));
    render_list(
        &mut fm,
        "selected_material_ids",
        &task.selected_material_ids,
    );
    render_list(
        &mut fm,
        "selected_knowledge_ids",
        &task.selected_knowledge_ids,
    );
    fm.push_str(&format!(
        "document_id: {}\n",
        yaml_opt(&task.document_id)
    ));
    fm.push_str(&format!("status: {}\n", yaml_scalar(&task.status)));
    fm.push_str(&format!("created_at: {}\n", yaml_scalar(&task.created_at)));
    fm.push_str(&format!("updated_at: {}\n", yaml_scalar(&task.updated_at)));
    let mut out = format!("---\n{fm}---\n\n## 状态记录\n\n");
    for record in &task.status_records {
        out.push_str(&format!(
            "- {} | {} | {}\n",
            record.at, record.action, record.reason
        ));
    }
    out
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WritingFront {
    #[serde(default)]
    schema: Option<String>,
    writing_id: String,
    title: String,
    document_type: String,
    writing_purpose: String,
    audience: String,
    writing_period: String,
    topic: String,
    requirements: String,
    expected_length: String,
    writing_style: String,
    #[serde(default)]
    selected_material_ids: Vec<String>,
    #[serde(default)]
    selected_knowledge_ids: Vec<String>,
    #[serde(default)]
    document_id: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

fn parse_status_records(body: &str) -> Result<Vec<StatusRecord>, String> {
    let mut records = Vec::new();
    let mut in_section = false;
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "## 状态记录" {
            in_section = true;
            continue;
        }
        if line.starts_with("## ") {
            return Err(format!("写作任务文件出现未知分区：{line}"));
        }
        if !in_section {
            return Err("写作任务正文不在 状态记录 分区里。".to_string());
        }
        let item = line
            .strip_prefix("- ")
            .ok_or_else(|| format!("状态记录里有无法识别的行：{line}"))?;
        let mut parts = item.splitn(3, '|');
        let at = parts
            .next()
            .ok_or_else(|| format!("状态记录缺少时间：{line}"))?
            .trim()
            .to_string();
        let action = parts
            .next()
            .ok_or_else(|| format!("状态记录缺少动作：{line}"))?
            .trim()
            .to_string();
        let reason = parts
            .next()
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if at.is_empty() || action.is_empty() {
            return Err(format!("状态记录不完整：{line}"));
        }
        records.push(StatusRecord { at, action, reason });
    }
    if !in_section {
        return Err("写作任务文件缺少 状态记录 分区。".to_string());
    }
    Ok(records)
}

fn parse_writing_task(raw: &str) -> Result<WritingTask, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: WritingFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("写作任务 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != WRITING_SCHEMA {
            return Err(format!("写作任务 schema 不认识：{schema}"));
        }
    }
    let status_records = parse_status_records(&body)?;
    let task = WritingTask {
        schema: WRITING_SCHEMA.to_string(),
        writing_id: front.writing_id,
        title: front.title,
        document_type: front.document_type,
        writing_purpose: front.writing_purpose,
        audience: front.audience,
        writing_period: front.writing_period,
        topic: front.topic,
        requirements: front.requirements,
        expected_length: front.expected_length,
        writing_style: front.writing_style,
        selected_material_ids: front.selected_material_ids,
        selected_knowledge_ids: front.selected_knowledge_ids,
        document_id: normalize_opt(front.document_id),
        status: front.status,
        created_at: front.created_at,
        updated_at: front.updated_at,
        status_records,
    };
    validate_writing_task(&task)?;
    Ok(task)
}

fn read_all(root: &Path) -> Result<(Vec<WritingTask>, usize), String> {
    assert_work_root(root)?;
    let dir = writing_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if !dir.exists() {
        return Ok((items, broken));
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取写作目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取写作目录项失败：{e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        match fs::read_to_string(&path).and_then(|raw| {
            parse_writing_task(&raw).map_err(std::io::Error::other)
        }) {
            Ok(task) => items.push(task),
            Err(_) => broken += 1,
        }
    }
    Ok((items, broken))
}

fn persist(root: &Path, task: &WritingTask) -> Result<(), String> {
    write_replace_atomically(
        &writing_path(root, &task.writing_id)?,
        render_writing_task(task).as_bytes(),
    )
}

pub fn create_writing_task(
    root: &Path,
    draft: &WritingTaskDraft,
    now: f64,
) -> Result<WritingTask, String> {
    assert_work_root(root)?;
    let id = match draft.writing_id.clone().map(|value| value.trim().to_string()) {
        Some(id) if !id.is_empty() => {
            if !is_safe_id(&id) || id.chars().count() > MAX_ID_CHARS {
                return Err(format!("写作任务编号不合法：{id:?}"));
            }
            if writing_path(root, &id)?.exists() {
                return Err(format!("写作任务已存在：{id}"));
            }
            id
        }
        _ => new_file_id("w", now),
    };
    let stamp = crate::task_shell::utc_iso8601(now);
    let task = WritingTask {
        schema: WRITING_SCHEMA.to_string(),
        writing_id: id,
        title: draft.title.trim().to_string(),
        document_type: draft.document_type.trim().to_string(),
        writing_purpose: draft.writing_purpose.trim().to_string(),
        audience: draft.audience.trim().to_string(),
        writing_period: draft.writing_period.trim().to_string(),
        topic: draft.topic.trim().to_string(),
        requirements: draft.requirements.trim().to_string(),
        expected_length: draft.expected_length.trim().to_string(),
        writing_style: draft.writing_style.trim().to_string(),
        selected_material_ids: normalize_link_ids(
            &draft.selected_material_ids,
            "选中素材",
        )?,
        selected_knowledge_ids: normalize_link_ids(
            &draft.selected_knowledge_ids,
            "选中知识",
        )?,
        document_id: normalize_opt(draft.document_id.clone()),
        status: STATUS_NEEDS_ORGANIZING.to_string(),
        created_at: stamp.clone(),
        updated_at: stamp.clone(),
        status_records: vec![StatusRecord {
            at: stamp,
            action: ACTION_CREATE.to_string(),
            reason: String::new(),
        }],
    };
    validate_writing_task(&task)?;
    fs::create_dir_all(writing_dir(root))
        .map_err(|e| format!("创建写作目录失败：{e}"))?;
    write_new_atomically(
        &writing_path(root, &task.writing_id)?,
        render_writing_task(&task).as_bytes(),
    )?;
    Ok(task)
}

pub fn read_writing_task(root: &Path, id: &str) -> Result<WritingTask, String> {
    assert_work_root(root)?;
    let path = writing_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取写作任务失败（{}）：{}", path.display(), e))?;
    parse_writing_task(&raw)
}

fn matches_filter(task: &WritingTask, filter: &WritingTaskFilter) -> bool {
    if let Some(value) = filter.status.as_deref() {
        if task.status != value.trim() {
            return false;
        }
    }
    if let Some(value) = filter.document_type.as_deref() {
        if task.document_type != value.trim() {
            return false;
        }
    }
    let created_date = task.created_at.get(0..10);
    if let Some(from) = filter.date_from.as_deref() {
        match created_date {
            Some(date) if date >= from.trim() => {}
            _ => return false,
        }
    }
    if let Some(to) = filter.date_to.as_deref() {
        match created_date {
            Some(date) if date <= to.trim() => {}
            _ => return false,
        }
    }
    true
}

fn validate_filter(filter: &WritingTaskFilter) -> Result<(), String> {
    for (value, field) in [
        (filter.date_from.as_deref(), "起始日期"),
        (filter.date_to.as_deref(), "结束日期"),
    ] {
        if let Some(date) = value {
            if !is_date_shape(date.trim()) {
                return Err(format!("{field}形状不正确：{date}"));
            }
        }
    }
    if let (Some(from), Some(to)) = (filter.date_from.as_deref(), filter.date_to.as_deref()) {
        if from.trim() > to.trim() {
            return Err("起始日期不能晚于结束日期。".to_string());
        }
    }
    if let Some(value) = filter.status.as_deref() {
        validate_status(value.trim())?;
    }
    if let Some(value) = filter.document_type.as_deref() {
        validate_document_type(value.trim())?;
    }
    Ok(())
}

fn status_rank(status: &str) -> usize {
    WRITING_STATUSES
        .iter()
        .position(|value| *value == status)
        .unwrap_or(usize::MAX)
}

pub fn list_writing_tasks(
    root: &Path,
    filter: &WritingTaskFilter,
) -> Result<WritingTaskListing, String> {
    assert_work_root(root)?;
    validate_filter(filter)?;
    let (items, broken) = read_all(root)?;
    let mut items: Vec<WritingTask> = items
        .into_iter()
        .filter(|task| matches_filter(task, filter))
        .collect();
    items.sort_by(|a, b| {
        status_rank(&a.status)
            .cmp(&status_rank(&b.status))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.writing_id.cmp(&b.writing_id))
    });
    Ok(WritingTaskListing { items, broken })
}

/// 状态迁移合法性表。
///
/// * 大纲待确认 → 写作中必须由调用方传入已确认大纲的动作；
/// * 归档只能从已定稿进入；
/// * 已取消与已归档都是终态；取消可从其余任何状态进入。
pub fn is_allowed_transition(from: &str, to: &str, outline_confirmed: bool) -> bool {
    if to == STATUS_CANCELLED {
        return from != STATUS_ARCHIVED && from != STATUS_CANCELLED;
    }
    match (from, to) {
        (STATUS_NEEDS_ORGANIZING, STATUS_COLLECTING_MATERIALS) => true,
        (STATUS_COLLECTING_MATERIALS, STATUS_OUTLINE_PENDING) => true,
        (STATUS_OUTLINE_PENDING, STATUS_WRITING) => outline_confirmed,
        (STATUS_WRITING, STATUS_FACT_CHECKING) => true,
        (STATUS_FACT_CHECKING, STATUS_PENDING_REVIEW) => true,
        (STATUS_FACT_CHECKING, STATUS_WRITING) => true,
        (STATUS_PENDING_REVIEW, STATUS_FINALIZED) => true,
        (STATUS_PENDING_REVIEW, STATUS_WRITING) => true,
        (STATUS_FINALIZED, STATUS_ARCHIVED) => true,
        _ => false,
    }
}

fn allowed_next_text(from: &str, outline_confirmed: bool) -> String {
    let mut steps: Vec<&str> = Vec::new();
    match from {
        STATUS_NEEDS_ORGANIZING => steps.push(STATUS_COLLECTING_MATERIALS),
        STATUS_COLLECTING_MATERIALS => steps.push(STATUS_OUTLINE_PENDING),
        STATUS_OUTLINE_PENDING => {
            if outline_confirmed {
                steps.push(STATUS_WRITING);
            } else {
                steps.push("写作中（需先确认大纲）");
            }
        }
        STATUS_WRITING => steps.push(STATUS_FACT_CHECKING),
        STATUS_FACT_CHECKING => {
            steps.push(STATUS_PENDING_REVIEW);
            steps.push(STATUS_WRITING);
        }
        STATUS_PENDING_REVIEW => {
            steps.push(STATUS_FINALIZED);
            steps.push(STATUS_WRITING);
        }
        STATUS_FINALIZED => steps.push(STATUS_ARCHIVED),
        _ => {}
    }
    if from != STATUS_ARCHIVED && from != STATUS_CANCELLED {
        steps.push(STATUS_CANCELLED);
    }
    if steps.is_empty() {
        "无（终态）".to_string()
    } else {
        steps.join(" / ")
    }
}

fn transition_error(from: &str, to: &str, outline_confirmed: bool) -> String {
    if from == STATUS_OUTLINE_PENDING && to == STATUS_WRITING && !outline_confirmed {
        return "从大纲待确认进入写作中必须确认大纲。".to_string();
    }
    if to == STATUS_ARCHIVED && from != STATUS_FINALIZED {
        return format!("只有已定稿可以归档：当前状态 {from}。");
    }
    if to == STATUS_CANCELLED && from == STATUS_ARCHIVED {
        return "已归档任务不能取消。".to_string();
    }
    format!(
        "非法状态迁移：{from} → {to}；允许的下一步：{}",
        allowed_next_text(from, outline_confirmed)
    )
}

/// 非法迁移拒绝时追加一条记录，状态保持不变。
fn record_rejection(
    root: &Path,
    mut task: WritingTask,
    to: &str,
    reason: Option<&str>,
    now: f64,
    message: String,
) -> String {
    let stamp = crate::task_shell::utc_iso8601(now);
    let from = task.status.clone();
    let target = if to.trim().is_empty() {
        "（空）".to_string()
    } else {
        sanitize_action(to)
    };
    let detail = reason
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| message.clone());
    task.status_records.push(StatusRecord {
        at: stamp.clone(),
        action: format!("{}：{from} → {target}", ACTION_REJECT),
        reason: sanitize_record_text(&detail),
    });
    task.updated_at = stamp;
    match validate_writing_task(&task).and_then(|_| persist(root, &task)) {
        Ok(()) => message,
        Err(error) => format!("{message}；拒绝记录写入失败：{error}"),
    }
}

pub fn transition_writing_status(
    root: &Path,
    id: &str,
    to: &str,
    outline_confirmed: bool,
    reason: Option<&str>,
    now: f64,
) -> Result<WritingTask, String> {
    assert_work_root(root)?;
    let mut task = read_writing_task(root, id)?;
    let to = to.trim().to_string();
    if !WRITING_STATUSES.contains(&to.as_str()) {
        let message = format!("写作状态不认识：{to}");
        return Err(record_rejection(root, task, &to, reason, now, message));
    }
    if task.status == to {
        let message = format!("状态未变化：{to}。");
        return Err(record_rejection(root, task, &to, reason, now, message));
    }
    if !is_allowed_transition(&task.status, &to, outline_confirmed) {
        let message = transition_error(&task.status, &to, outline_confirmed);
        return Err(record_rejection(root, task, &to, reason, now, message));
    }
    let from = task.status.clone();
    let stamp = crate::task_shell::utc_iso8601(now);
    let detail = reason
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    task.status_records.push(StatusRecord {
        at: stamp.clone(),
        action: format!("{ACTION_TRANSITION}：{from} → {to}"),
        reason: sanitize_record_text(&detail),
    });
    task.status = to;
    task.updated_at = stamp;
    validate_writing_task(&task)?;
    persist(root, &task)?;
    Ok(task)
}

pub fn update_writing_task(
    root: &Path,
    id: &str,
    patch: &WritingTaskPatch,
    now: f64,
) -> Result<WritingTask, String> {
    let mut task = read_writing_task(root, id)?;
    if let Some(value) = patch.title.clone() {
        task.title = value.trim().to_string();
    }
    if let Some(value) = patch.document_type.clone() {
        task.document_type = value.trim().to_string();
    }
    if let Some(value) = patch.writing_purpose.clone() {
        task.writing_purpose = value.trim().to_string();
    }
    if let Some(value) = patch.audience.clone() {
        task.audience = value.trim().to_string();
    }
    if let Some(value) = patch.writing_period.clone() {
        task.writing_period = value.trim().to_string();
    }
    if let Some(value) = patch.topic.clone() {
        task.topic = value.trim().to_string();
    }
    if let Some(value) = patch.requirements.clone() {
        task.requirements = value.trim().to_string();
    }
    if let Some(value) = patch.expected_length.clone() {
        task.expected_length = value.trim().to_string();
    }
    if let Some(value) = patch.writing_style.clone() {
        task.writing_style = value.trim().to_string();
    }
    if let Some(value) = patch.selected_material_ids.clone() {
        task.selected_material_ids = normalize_link_ids(&value, "选中素材")?;
    }
    if let Some(value) = patch.selected_knowledge_ids.clone() {
        task.selected_knowledge_ids = normalize_link_ids(&value, "选中知识")?;
    }
    if let Some(value) = patch.document_id.clone() {
        task.document_id = normalize_opt(Some(value));
    }
    let stamp = crate::task_shell::utc_iso8601(now);
    let detail = patch
        .reason
        .as_deref()
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    task.status_records.push(StatusRecord {
        at: stamp.clone(),
        action: ACTION_EDIT.to_string(),
        reason: sanitize_record_text(&detail),
    });
    task.updated_at = stamp;
    validate_writing_task(&task)?;
    persist(root, &task)?;
    Ok(task)
}

/// 删除 = 移到 work/.trash/写作/，不改写字节。
pub fn delete_writing_task(root: &Path, id: &str, now: f64) -> Result<String, String> {
    assert_work_root(root)?;
    let source = writing_path(root, id)?;
    if !source.is_file() {
        return Err(format!("写作任务不存在，未删除：{id}"));
    }
    let trash = trash_writing_dir(root);
    fs::create_dir_all(&trash).map_err(|e| format!("创建写作回收目录失败：{e}"))?;
    let stamp = now.floor() as i64;
    let mut target = trash.join(format!("{id}.{stamp}.md"));
    let mut attempt = 0u32;
    while target.exists() {
        attempt += 1;
        target = trash.join(format!("{id}.{stamp}-{attempt}.md"));
    }
    fs::rename(&source, &target).map_err(|e| {
        format!(
            "移动到写作回收目录失败（{} → {}）：{}",
            source.display(),
            target.display(),
            e
        )
    })?;
    Ok(target.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const NOW: f64 = 1_795_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wt601-writing-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

    fn draft(title: &str) -> WritingTaskDraft {
        WritingTaskDraft {
            title: title.to_string(),
            document_type: TYPE_MONTHLY_SUMMARY.to_string(),
            writing_purpose: "总结本月试点进展".to_string(),
            audience: "部门同事".to_string(),
            writing_period: "2026-11".to_string(),
            topic: "合成试点推进情况".to_string(),
            requirements: "只写已确认事实，三点以内。".to_string(),
            expected_length: "1200 字".to_string(),
            writing_style: "简洁、平实".to_string(),
            ..Default::default()
        }
    }

    fn create(temp: &TempRoot, title: &str) -> WritingTask {
        create_writing_task(temp.root(), &draft(title), NOW).unwrap()
    }

    fn walk_to_outline(temp: &TempRoot, task: &WritingTask, now: f64) -> WritingTask {
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_COLLECTING_MATERIALS,
            false,
            None,
            now,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_OUTLINE_PENDING,
            false,
            None,
            now + 60.0,
        )
        .unwrap()
    }

    #[test]
    fn create_writes_frontmatter_and_status_section() {
        let temp = TempRoot::new("create");
        let task = create(&temp, "十一月试点总结");
        let path = writing_path(temp.root(), &task.writing_id).unwrap();
        assert!(path.starts_with(writing_dir(temp.root())));
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("schema: \"omnidesk.writing.v1\""), "{raw}");
        assert!(raw.contains("## 状态记录"), "{raw}");
        assert_eq!(task.status, STATUS_NEEDS_ORGANIZING);
        assert_eq!(task.status_records.len(), 1);
        assert_eq!(task.status_records[0].action, ACTION_CREATE);
        assert_eq!(task.created_at, task.updated_at);
    }

    #[test]
    fn document_type_whitelist_has_twelve_and_rejects_unknown() {
        let temp = TempRoot::new("types");
        assert_eq!(DOCUMENT_TYPES.len(), 12);
        for (index, doc_type) in DOCUMENT_TYPES.iter().enumerate() {
            let mut item = draft(&format!("文体{index}"));
            item.document_type = doc_type.to_string();
            item.topic = format!("合成主题{index}");
            create_writing_task(temp.root(), &item, NOW).unwrap();
        }
        let mut bad = draft("未知文体");
        bad.document_type = "未定义文体".to_string();
        assert!(create_writing_task(temp.root(), &bad, NOW)
            .unwrap_err()
            .contains("文稿类型不认识"));
    }

    #[test]
    fn create_rejects_empty_title_and_bad_id() {
        let temp = TempRoot::new("empty");
        let mut empty = draft("");
        assert!(create_writing_task(temp.root(), &empty, NOW)
            .unwrap_err()
            .contains("标题不能为空"));
        empty.title = "标题".to_string();
        empty.writing_id = Some("../逃逸".to_string());
        assert!(create_writing_task(temp.root(), &empty, NOW)
            .unwrap_err()
            .contains("编号不合法"));
    }

    #[test]
    fn create_rejects_existing_id() {
        let temp = TempRoot::new("existing");
        let task = create(&temp, "已存在任务");
        let mut duplicate = draft("重复编号");
        duplicate.writing_id = Some(task.writing_id.clone());
        assert!(create_writing_task(temp.root(), &duplicate, NOW)
            .unwrap_err()
            .contains("写作任务已存在"));
    }

    #[test]
    fn read_round_trips_all_fields_and_lists() {
        let temp = TempRoot::new("roundtrip");
        let mut source = draft("完整字段");
        source.selected_material_ids = vec![
            " m-001 ".to_string(),
            "m-001".to_string(),
            "m-002".to_string(),
        ];
        source.selected_knowledge_ids = vec!["k-001".to_string()];
        source.document_id = Some("doc-001".to_string());
        let created = create_writing_task(temp.root(), &source, NOW).unwrap();
        let loaded = read_writing_task(temp.root(), &created.writing_id).unwrap();
        assert_eq!(loaded, created);
        assert_eq!(loaded.selected_material_ids, vec!["m-001", "m-002"]);
        assert_eq!(loaded.selected_knowledge_ids, vec!["k-001"]);
        assert_eq!(loaded.document_id.as_deref(), Some("doc-001"));
    }

    #[test]
    fn list_filters_by_status_and_type() {
        let temp = TempRoot::new("filter-type");
        let mut annual = draft("年度总结草稿");
        annual.document_type = TYPE_ANNUAL_SUMMARY.to_string();
        annual.topic = "合成年度主题".to_string();
        let annual = create_writing_task(temp.root(), &annual, NOW).unwrap();
        let monthly = create(&temp, "月度总结草稿");
        transition_writing_status(
            temp.root(),
            &monthly.writing_id,
            STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 60.0,
        )
        .unwrap();
        let collecting = list_writing_tasks(
            temp.root(),
            &WritingTaskFilter {
                status: Some(STATUS_COLLECTING_MATERIALS.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(collecting.items.len(), 1);
        assert_eq!(collecting.items[0].writing_id, monthly.writing_id);
        let annuals = list_writing_tasks(
            temp.root(),
            &WritingTaskFilter {
                document_type: Some(TYPE_ANNUAL_SUMMARY.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(annuals.items.len(), 1);
        assert_eq!(annuals.items[0].writing_id, annual.writing_id);
        assert!(list_writing_tasks(
            temp.root(),
            &WritingTaskFilter {
                status: Some("未定义状态".to_string()),
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn list_filters_by_created_date_range() {
        let temp = TempRoot::new("filter-date");
        let early = create(&temp, "早期任务");
        let mut late_draft = draft("后期任务");
        late_draft.topic = "合成后期主题".to_string();
        let late = create_writing_task(temp.root(), &late_draft, NOW + 86_400.0 * 10.0).unwrap();
        let late_date = late.created_at[0..10].to_string();
        let listing = list_writing_tasks(
            temp.root(),
            &WritingTaskFilter {
                date_from: Some(late_date.clone()),
                date_to: Some(late_date),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].writing_id, late.writing_id);
        assert!(!listing
            .items
            .iter()
            .any(|item| item.writing_id == early.writing_id));
        assert!(list_writing_tasks(
            temp.root(),
            &WritingTaskFilter {
                date_from: Some("2026-11-30".to_string()),
                date_to: Some("2026-11-01".to_string()),
                ..Default::default()
            }
        )
        .unwrap_err()
        .contains("不能晚于"));
    }

    #[test]
    fn list_counts_broken_files_and_keeps_valid_items() {
        let temp = TempRoot::new("broken");
        let task = create(&temp, "有效任务");
        fs::write(writing_dir(temp.root()).join("broken.md"), "不是约定格式").unwrap();
        let listing = list_writing_tasks(temp.root(), &WritingTaskFilter::default()).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].writing_id, task.writing_id);
        assert_eq!(listing.broken, 1);
    }

    #[test]
    fn list_sorts_by_status_then_update_time() {
        let temp = TempRoot::new("sort");
        let archived_source = create(&temp, "归档来源");
        walk_to_outline(&temp, &archived_source, NOW);
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_WRITING,
            true,
            None,
            NOW + 120.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_FACT_CHECKING,
            false,
            None,
            NOW + 180.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_PENDING_REVIEW,
            false,
            None,
            NOW + 240.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_FINALIZED,
            false,
            None,
            NOW + 300.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_ARCHIVED,
            false,
            None,
            NOW + 360.0,
        )
        .unwrap();
        let pending = create(&temp, "待整理任务");
        let listing = list_writing_tasks(temp.root(), &WritingTaskFilter::default()).unwrap();
        assert_eq!(listing.items[0].writing_id, pending.writing_id);
        assert_eq!(listing.items[1].writing_id, archived_source.writing_id);
    }

    #[test]
    fn happy_path_walks_nine_states_with_confirmed_outline() {
        let temp = TempRoot::new("happy");
        let task = create(&temp, "完整流程");
        walk_to_outline(&temp, &task, NOW);
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_WRITING,
            true,
            Some("大纲已确认"),
            NOW + 120.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_FACT_CHECKING,
            false,
            None,
            NOW + 180.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_PENDING_REVIEW,
            false,
            None,
            NOW + 240.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_FINALIZED,
            false,
            None,
            NOW + 300.0,
        )
        .unwrap();
        let archived = transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_ARCHIVED,
            false,
            None,
            NOW + 360.0,
        )
        .unwrap();
        assert_eq!(archived.status, STATUS_ARCHIVED);
        assert_eq!(archived.status_records.len(), 8);
        assert_eq!(archived.status_records[0].action, ACTION_CREATE);
        assert_eq!(
            archived.status_records.last().unwrap().action,
            "迁移：已定稿 → 已归档"
        );
    }

    #[test]
    fn outline_gate_rejects_until_confirmed_and_logs_attempt() {
        let temp = TempRoot::new("outline-gate");
        let task = create(&temp, "大纲门禁");
        walk_to_outline(&temp, &task, NOW);
        let err = transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_WRITING,
            false,
            Some("误操作"),
            NOW + 120.0,
        )
        .unwrap_err();
        assert!(err.contains("必须确认大纲"), "{err}");
        let rejected = read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(rejected.status, STATUS_OUTLINE_PENDING);
        assert_eq!(rejected.status_records.len(), 4);
        assert!(rejected.status_records[3]
            .action
            .starts_with(ACTION_REJECT));
        assert_eq!(rejected.status_records[3].reason, "误操作");
        let confirmed = transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_WRITING,
            true,
            None,
            NOW + 180.0,
        )
        .unwrap();
        assert_eq!(confirmed.status, STATUS_WRITING);
    }

    #[test]
    fn archive_is_rejected_until_finalized_and_logs_attempt() {
        let temp = TempRoot::new("archive-gate");
        let task = create(&temp, "归档门禁");
        transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 60.0,
        )
        .unwrap();
        let err = transition_writing_status(
            temp.root(),
            &task.writing_id,
            STATUS_ARCHIVED,
            false,
            None,
            NOW + 120.0,
        )
        .unwrap_err();
        assert!(err.contains("只有已定稿可以归档"), "{err}");
        let rejected = read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(rejected.status, STATUS_COLLECTING_MATERIALS);
        assert_eq!(rejected.status_records.len(), 3);
        assert!(rejected.status_records[2].action.starts_with(ACTION_REJECT));
    }

    #[test]
    fn cancel_is_allowed_from_non_archived_states_only() {
        let temp = TempRoot::new("cancel");
        let draft_task = create(&temp, "从需求取消");
        let cancelled = transition_writing_status(
            temp.root(),
            &draft_task.writing_id,
            STATUS_CANCELLED,
            false,
            Some("需求取消"),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(cancelled.status, STATUS_CANCELLED);
        let err = transition_writing_status(
            temp.root(),
            &draft_task.writing_id,
            STATUS_CANCELLED,
            false,
            None,
            NOW + 120.0,
        )
        .unwrap_err();
        assert!(err.contains("状态未变化"), "{err}");

        let archived_source = create(&temp, "已归档不可取消");
        walk_to_outline(&temp, &archived_source, NOW);
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_WRITING,
            true,
            None,
            NOW + 120.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_FACT_CHECKING,
            false,
            None,
            NOW + 180.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_PENDING_REVIEW,
            false,
            None,
            NOW + 240.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_FINALIZED,
            false,
            None,
            NOW + 300.0,
        )
        .unwrap();
        transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_ARCHIVED,
            false,
            None,
            NOW + 360.0,
        )
        .unwrap();
        let err = transition_writing_status(
            temp.root(),
            &archived_source.writing_id,
            STATUS_CANCELLED,
            false,
            None,
            NOW + 420.0,
        )
        .unwrap_err();
        assert!(err.contains("已归档任务不能取消"), "{err}");
        let archived = read_writing_task(temp.root(), &archived_source.writing_id).unwrap();
        assert_eq!(archived.status, STATUS_ARCHIVED);
    }

    #[test]
    fn unknown_target_status_is_rejected_and_logged() {
        let temp = TempRoot::new("unknown-status");
        let task = create(&temp, "未知状态目标");
        let err = transition_writing_status(
            temp.root(),
            &task.writing_id,
            "不存在的状态",
            false,
            None,
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("写作状态不认识"), "{err}");
        let rejected = read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(rejected.status, STATUS_NEEDS_ORGANIZING);
        assert_eq!(rejected.status_records.len(), 2);
        assert!(rejected.status_records[1].action.starts_with(ACTION_REJECT));
    }

    #[test]
    fn status_records_are_append_only() {
        let temp = TempRoot::new("append");
        let task = create(&temp, "追加记录");
        walk_to_outline(&temp, &task, NOW);
        let after = read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(after.status_records.len(), 3);
        assert_eq!(after.status_records[0].action, ACTION_CREATE);
        assert_eq!(
            after.status_records[1].action,
            "迁移：需求待整理 → 素材收集中"
        );
        assert_eq!(
            after.status_records[2].action,
            "迁移：素材收集中 → 大纲待确认"
        );
        let raw = fs::read_to_string(writing_path(temp.root(), &task.writing_id).unwrap())
            .unwrap();
        assert_eq!(raw.matches("| 创建 |").count(), 1);
        assert_eq!(raw.matches("| 迁移：").count(), 2);
        assert!(!raw.contains("status_records.clear"));
    }

    #[test]
    fn update_keeps_created_at_and_appends_edit() {
        let temp = TempRoot::new("update");
        let task = create(&temp, "原标题");
        let updated = update_writing_task(
            temp.root(),
            &task.writing_id,
            &WritingTaskPatch {
                title: Some("新标题".to_string()),
                document_id: Some("doc-002".to_string()),
                reason: Some("补充文稿编号".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.created_at, task.created_at);
        assert_eq!(updated.status, task.status);
        assert_eq!(updated.status_records.len(), 2);
        assert_eq!(updated.status_records[1].action, ACTION_EDIT);
        assert_eq!(updated.status_records[1].reason, "补充文稿编号");
        assert_eq!(updated.document_id.as_deref(), Some("doc-002"));
    }

    #[test]
    fn update_normalizes_selected_material_and_knowledge_ids() {
        let temp = TempRoot::new("update-links");
        let task = create(&temp, "选中编号");
        let updated = update_writing_task(
            temp.root(),
            &task.writing_id,
            &WritingTaskPatch {
                selected_material_ids: Some(vec![
                    " m-001 ".to_string(),
                    "m-001".to_string(),
                    "".to_string(),
                    "m-002".to_string(),
                ]),
                selected_knowledge_ids: Some(vec![
                    "k-001".to_string(),
                    "k-001".to_string(),
                ]),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.selected_material_ids, vec!["m-001", "m-002"]);
        assert_eq!(updated.selected_knowledge_ids, vec!["k-001"]);
    }

    #[test]
    fn delete_moves_to_trash_preserving_bytes() {
        let temp = TempRoot::new("delete");
        let task = create(&temp, "待回收任务");
        let path = writing_path(temp.root(), &task.writing_id).unwrap();
        let before = fs::read(&path).unwrap();
        let trashed = delete_writing_task(temp.root(), &task.writing_id, NOW).unwrap();
        assert!(!path.exists());
        assert!(trashed.contains("写作"));
        let after = fs::read(&trashed).unwrap();
        assert_eq!(before, after);
        assert!(read_writing_task(temp.root(), &task.writing_id).is_err());
    }

    #[test]
    fn read_rejects_unknown_frontmatter_field() {
        let temp = TempRoot::new("deny-unknown");
        let task = create(&temp, "拒绝未知字段");
        let path = writing_path(temp.root(), &task.writing_id).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let tampered = raw.replacen("title:", "extra: \"外来字段\"\ntitle:", 1);
        fs::write(&path, tampered).unwrap();
        let err = read_writing_task(temp.root(), &task.writing_id).unwrap_err();
        assert!(err.contains("不是约定字段"), "{err}");
    }

    #[test]
    fn read_rejects_unknown_status_in_file() {
        let temp = TempRoot::new("unknown-file-status");
        let task = create(&temp, "拒绝未知状态");
        let path = writing_path(temp.root(), &task.writing_id).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        fs::write(&path, raw.replacen("status: \"需求待整理\"", "status: \"随便写\"", 1))
            .unwrap();
        let err = read_writing_task(temp.root(), &task.writing_id).unwrap_err();
        assert!(err.contains("写作状态不认识"), "{err}");
    }
}
