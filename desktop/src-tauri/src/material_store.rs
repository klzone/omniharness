//! 写作素材实体（OD-KM-501）。
//!
//! 每条素材一个 Markdown 文件：<work_root>/素材篮/<material_id>.md。
//! frontmatter 存素材字段，正文只有 `## 审核记录` 追加段；审核记录只追加，不改写。
//! 素材类型、审核状态、来源类型都是有限白名单；重复识别用确定性指纹
//! sha256(source_id + 归一化引用)，复用 `sha256.rs`，不引入数据库。
//! 创建一律落在「待审核」；审核状态只能经 `review_material` 由用户动作推进，
//! 存储层不提供 AI 直接写确认状态的通道。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::sha256::sha256_hex;
use crate::worklog_records::{
    assert_work_root, is_date_shape, is_safe_id, new_file_id, normalize_opt, split_frontmatter,
    write_new_atomically, write_replace_atomically, yaml_opt, yaml_scalar, TRASH_DIR,
};

pub const MATERIAL_SCHEMA: &str = "omnidesk.material.v1";
pub const MATERIAL_DIR: &str = "素材篮";

pub const TYPE_WORK_FACT: &str = "工作事实";
pub const TYPE_WORK_RESULT: &str = "工作成果";
pub const TYPE_DATA: &str = "数据素材";
pub const TYPE_EXPERIENCE: &str = "经验做法";
pub const TYPE_CASE: &str = "典型案例";
pub const TYPE_PROBLEM: &str = "问题不足";
pub const TYPE_POLICY: &str = "制度依据";
pub const TYPE_PLAN: &str = "工作计划";
pub const TYPE_DEPLOYMENT: &str = "领导部署";
pub const TYPE_HIGHLIGHT: &str = "特色亮点";
pub const MATERIAL_TYPES: &[&str] = &[
    TYPE_WORK_FACT,
    TYPE_WORK_RESULT,
    TYPE_DATA,
    TYPE_EXPERIENCE,
    TYPE_CASE,
    TYPE_PROBLEM,
    TYPE_POLICY,
    TYPE_PLAN,
    TYPE_DEPLOYMENT,
    TYPE_HIGHLIGHT,
];

pub const REVIEW_PENDING: &str = "待审核";
pub const REVIEW_CONFIRMED: &str = "已确认";
pub const REVIEW_RETURNED: &str = "已退回";
pub const REVIEW_REJECTED: &str = "已拒绝";
pub const REVIEW_ARCHIVED: &str = "已归档";
pub const REVIEW_STATUSES: &[&str] = &[
    REVIEW_PENDING,
    REVIEW_CONFIRMED,
    REVIEW_RETURNED,
    REVIEW_REJECTED,
    REVIEW_ARCHIVED,
];

pub const SOURCE_WORK_RECORD: &str = "work_record";
pub const SOURCE_TASK: &str = "task";
pub const SOURCE_PROJECT: &str = "project";
pub const SOURCE_KNOWLEDGE: &str = "knowledge";
pub const SOURCE_USER: &str = "user";
pub const SOURCE_TYPES: &[&str] = &[
    SOURCE_WORK_RECORD,
    SOURCE_TASK,
    SOURCE_PROJECT,
    SOURCE_KNOWLEDGE,
    SOURCE_USER,
];

pub const CONFIRMATION_UNCONFIRMED: &str = "未确认";
pub const CONFIRMATION_CONFIRMED: &str = "已确认";
pub const CONFIRMATION_STATUSES: &[&str] = &[CONFIRMATION_UNCONFIRMED, CONFIRMATION_CONFIRMED];

pub const ACTION_CONFIRM: &str = "确认";
pub const ACTION_RETURN: &str = "退回";
pub const ACTION_REJECT: &str = "拒绝";
pub const ACTION_DEFER: &str = "暂缓";
pub const ACTION_ARCHIVE: &str = "归档";
pub const ACTION_EDIT: &str = "编辑";

const MAX_ID_CHARS: usize = 80;
const MAX_TITLE_CHARS: usize = 200;
const MAX_CONTENT_CHARS: usize = 200_000;
const MAX_QUOTE_CHARS: usize = 20_000;
const MAX_LINK_ID_CHARS: usize = 200;
const MAX_TAG_CHARS: usize = 24;
const MAX_TAGS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub at: String,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WritingMaterial {
    pub schema: String,
    pub material_id: String,
    pub title: String,
    pub content: String,
    pub material_type: String,
    pub source_type: String,
    pub source_id: String,
    pub source_quote: String,
    pub source_location: Option<String>,
    pub related_task_ids: Vec<String>,
    pub related_project_id: Option<String>,
    pub related_knowledge_ids: Vec<String>,
    pub event_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub confidence: f64,
    pub review_status: String,
    pub confirmation_status: String,
    pub tags: Vec<String>,
    /// 不落 frontmatter，写在正文追加段；IPC 仍返回给界面。
    #[serde(default)]
    pub review_records: Vec<ReviewRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialDraft {
    pub material_id: Option<String>,
    pub title: String,
    pub content: String,
    pub material_type: String,
    pub source_type: String,
    pub source_id: String,
    pub source_quote: String,
    pub source_location: Option<String>,
    pub related_task_ids: Vec<String>,
    pub related_project_id: Option<String>,
    pub related_knowledge_ids: Vec<String>,
    pub event_date: Option<String>,
    pub confidence: Option<f64>,
    pub tags: Vec<String>,
}

/// 编辑补丁。None = 不修改；空字符串 = 清空。`reason` 写进审核记录。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub material_type: Option<String>,
    pub source_id: Option<String>,
    pub source_quote: Option<String>,
    pub source_location: Option<String>,
    pub related_task_ids: Option<Vec<String>>,
    pub related_project_id: Option<String>,
    pub related_knowledge_ids: Option<Vec<String>>,
    pub event_date: Option<String>,
    pub confidence: Option<f64>,
    pub tags: Option<Vec<String>>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialFilter {
    pub material_type: Option<String>,
    pub review_status: Option<String>,
    /// 事件日期闭区间，YYYY-MM-DD；只填一端也可。
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MaterialListing {
    pub items: Vec<WritingMaterial>,
    pub broken: usize,
}

pub fn material_dir(root: &Path) -> PathBuf {
    root.join(MATERIAL_DIR)
}

pub fn material_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) || id.chars().count() > MAX_ID_CHARS {
        return Err(format!("素材编号不合法：{id:?}"));
    }
    Ok(material_dir(root).join(format!("{id}.md")))
}

pub fn trash_material_dir(root: &Path) -> PathBuf {
    root.join(TRASH_DIR).join(MATERIAL_DIR)
}

/// 指纹前的引用归一化：折叠所有空白，去掉首尾空格。
pub fn normalize_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    out.trim_end().to_string()
}

/// 重复指纹：sha256(source_id + 归一化引用)，字段用 0x1F 分隔避免拼接歧义。
pub fn material_fingerprint(source_id: &str, source_quote: &str) -> String {
    let source = source_id.trim();
    let quote = normalize_quote(source_quote);
    sha256_hex(format!("{source}\u{1f}{quote}").as_bytes())
}

fn effective_quote(material: &WritingMaterial) -> &str {
    if material.source_quote.trim().is_empty() {
        &material.content
    } else {
        &material.source_quote
    }
}

fn fingerprint_of(material: &WritingMaterial) -> String {
    material_fingerprint(&material.source_id, effective_quote(material))
}

fn validate_material_type(value: &str) -> Result<(), String> {
    if !MATERIAL_TYPES.contains(&value) {
        return Err(format!("素材类型不认识：{value}"));
    }
    Ok(())
}

fn validate_source_type(value: &str) -> Result<(), String> {
    if !SOURCE_TYPES.contains(&value) {
        return Err(format!("来源类型不认识：{value}"));
    }
    Ok(())
}

fn validate_review_status(value: &str) -> Result<(), String> {
    if !REVIEW_STATUSES.contains(&value) {
        return Err(format!("审核状态不认识：{value}"));
    }
    Ok(())
}

fn validate_confirmation_status(value: &str) -> Result<(), String> {
    if !CONFIRMATION_STATUSES.contains(&value) {
        return Err(format!("确认标记不认识：{value}"));
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

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_CHARS {
            return Err(format!("标签过长（最多 {MAX_TAG_CHARS} 字）：{trimmed}"));
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    if out.len() > MAX_TAGS {
        return Err(format!("标签过多（最多 {MAX_TAGS} 个）"));
    }
    Ok(out)
}

fn normalize_single_id(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > MAX_LINK_ID_CHARS || trimmed.chars().any(|c| c.is_control()) {
        return Err(format!("{field}编号不合法：{trimmed:?}"));
    }
    Ok(trimmed.to_string())
}

fn validate_confidence(value: f64) -> Result<(), String> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!("置信度必须在 0 到 1 之间：{value}"));
    }
    Ok(())
}

fn validate_material(material: &WritingMaterial) -> Result<(), String> {
    if material.schema != MATERIAL_SCHEMA {
        return Err(format!("素材 schema 不认识：{}", material.schema));
    }
    if !is_safe_id(&material.material_id) || material.material_id.chars().count() > MAX_ID_CHARS {
        return Err(format!("素材编号不合法：{:?}", material.material_id));
    }
    if material.title.trim().is_empty() {
        return Err("素材标题不能为空。".to_string());
    }
    if material.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("素材标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
    }
    if material.content.trim().is_empty() {
        return Err("素材正文不能为空。".to_string());
    }
    if material.content.chars().count() > MAX_CONTENT_CHARS {
        return Err(format!("素材正文过长（最多 {MAX_CONTENT_CHARS} 字）。"));
    }
    validate_material_type(&material.material_type)?;
    validate_source_type(&material.source_type)?;
    if material.source_quote.trim().is_empty() {
        return Err("缺少来源原文引用 source_quote。".to_string());
    }
    if material.source_quote.chars().count() > MAX_QUOTE_CHARS {
        return Err(format!("来源引用过长（最多 {MAX_QUOTE_CHARS} 字）。"));
    }
    if material.source_id.trim().is_empty() {
        return Err("缺少来源编号 source_id。".to_string());
    }
    normalize_single_id(&material.source_id, "来源")?;
    if let Some(location) = material.source_location.as_deref() {
        if location.chars().count() > MAX_QUOTE_CHARS || location.chars().any(|c| c.is_control()) {
            return Err(format!("来源定位不合法：{location:?}"));
        }
    }
    if let Some(date) = material.event_date.as_deref() {
        if !is_date_shape(date) {
            return Err(format!("事件日期形状不正确：{date}"));
        }
    }
    if let Some(project) = material.related_project_id.as_deref() {
        normalize_single_id(project, "关联项目")?;
    }
    normalize_link_ids(&material.related_task_ids, "关联任务")?;
    normalize_link_ids(&material.related_knowledge_ids, "关联知识")?;
    normalize_tags(&material.tags)?;
    validate_confidence(material.confidence)?;
    validate_review_status(&material.review_status)?;
    validate_confirmation_status(&material.confirmation_status)?;
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

fn render_material(material: &WritingMaterial) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(&material.schema)));
    fm.push_str(&format!(
        "material_id: {}\n",
        yaml_scalar(&material.material_id)
    ));
    fm.push_str(&format!("title: {}\n", yaml_scalar(&material.title)));
    fm.push_str(&format!("content: {}\n", yaml_scalar(&material.content)));
    fm.push_str(&format!(
        "material_type: {}\n",
        yaml_scalar(&material.material_type)
    ));
    fm.push_str(&format!(
        "source_type: {}\n",
        yaml_scalar(&material.source_type)
    ));
    fm.push_str(&format!("source_id: {}\n", yaml_scalar(&material.source_id)));
    fm.push_str(&format!(
        "source_quote: {}\n",
        yaml_scalar(&material.source_quote)
    ));
    fm.push_str(&format!(
        "source_location: {}\n",
        yaml_opt(&material.source_location)
    ));
    render_list(&mut fm, "related_task_ids", &material.related_task_ids);
    fm.push_str(&format!(
        "related_project_id: {}\n",
        yaml_opt(&material.related_project_id)
    ));
    render_list(
        &mut fm,
        "related_knowledge_ids",
        &material.related_knowledge_ids,
    );
    fm.push_str(&format!(
        "event_date: {}\n",
        yaml_opt(&material.event_date)
    ));
    fm.push_str(&format!("created_at: {}\n", yaml_scalar(&material.created_at)));
    fm.push_str(&format!("updated_at: {}\n", yaml_scalar(&material.updated_at)));
    fm.push_str(&format!("confidence: {}\n", format_confidence(material.confidence)));
    fm.push_str(&format!(
        "review_status: {}\n",
        yaml_scalar(&material.review_status)
    ));
    fm.push_str(&format!(
        "confirmation_status: {}\n",
        yaml_scalar(&material.confirmation_status)
    ));
    render_list(&mut fm, "tags", &material.tags);
    let mut out = format!("---\n{fm}---\n\n## 审核记录\n\n");
    for record in &material.review_records {
        out.push_str(&format!(
            "- {} | {} | {}\n",
            record.at, record.action, record.reason
        ));
    }
    out
}

fn format_confidence(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{:.1}", value)
    } else {
        value.to_string()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialFront {
    #[serde(default)]
    schema: Option<String>,
    material_id: String,
    title: String,
    content: String,
    material_type: String,
    source_type: String,
    source_id: String,
    source_quote: String,
    #[serde(default)]
    source_location: Option<String>,
    #[serde(default)]
    related_task_ids: Vec<String>,
    #[serde(default)]
    related_project_id: Option<String>,
    #[serde(default)]
    related_knowledge_ids: Vec<String>,
    #[serde(default)]
    event_date: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    confidence: f64,
    review_status: String,
    confirmation_status: String,
    #[serde(default)]
    tags: Vec<String>,
}

fn parse_review_records(body: &str) -> Result<Vec<ReviewRecord>, String> {
    let mut records = Vec::new();
    let mut in_section = false;
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "## 审核记录" {
            in_section = true;
            continue;
        }
        if line.starts_with("## ") {
            return Err(format!("素材文件出现未知分区：{line}"));
        }
        if !in_section {
            return Err("素材内容不在 审核记录 分区里。".to_string());
        }
        let item = line
            .strip_prefix("- ")
            .ok_or_else(|| format!("审核记录里有无法识别的行：{line}"))?;
        let mut parts = item.splitn(3, '|');
        let at = parts
            .next()
            .ok_or_else(|| format!("审核记录缺少时间：{line}"))?
            .trim()
            .to_string();
        let action = parts
            .next()
            .ok_or_else(|| format!("审核记录缺少动作：{line}"))?
            .trim()
            .to_string();
        let reason = parts
            .next()
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if at.is_empty() || action.is_empty() {
            return Err(format!("审核记录不完整：{line}"));
        }
        records.push(ReviewRecord { at, action, reason });
    }
    if !in_section {
        return Err("素材文件缺少 审核记录 分区。".to_string());
    }
    Ok(records)
}

fn parse_material(raw: &str) -> Result<WritingMaterial, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: MaterialFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("素材 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != MATERIAL_SCHEMA {
            return Err(format!("素材 schema 不认识：{schema}"));
        }
    }
    let review_records = parse_review_records(&body)?;
    let material = WritingMaterial {
        schema: MATERIAL_SCHEMA.to_string(),
        material_id: front.material_id,
        title: front.title,
        content: front.content,
        material_type: front.material_type,
        source_type: front.source_type,
        source_id: front.source_id,
        source_quote: front.source_quote,
        source_location: normalize_opt(front.source_location),
        related_task_ids: front.related_task_ids,
        related_project_id: normalize_opt(front.related_project_id),
        related_knowledge_ids: front.related_knowledge_ids,
        event_date: normalize_opt(front.event_date),
        created_at: front.created_at,
        updated_at: front.updated_at,
        confidence: front.confidence,
        review_status: front.review_status,
        confirmation_status: front.confirmation_status,
        tags: front.tags,
        review_records,
    };
    validate_material(&material)?;
    Ok(material)
}

fn read_all(root: &Path) -> Result<(Vec<WritingMaterial>, usize), String> {
    assert_work_root(root)?;
    let dir = material_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if !dir.exists() {
        return Ok((items, broken));
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取素材篮目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取素材篮目录项失败：{e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        match fs::read_to_string(&path).and_then(|raw| {
            parse_material(&raw).map_err(|error| std::io::Error::other(error))
        }) {
            Ok(material) => items.push(material),
            Err(_) => broken += 1,
        }
    }
    Ok((items, broken))
}

fn duplicate_with(
    root: &Path,
    source_id: &str,
    quote: &str,
    exclude_id: Option<&str>,
) -> Result<Option<String>, String> {
    let fingerprint = material_fingerprint(source_id, quote);
    let (items, _) = read_all(root)?;
    for material in items {
        if Some(material.material_id.as_str()) == exclude_id {
            continue;
        }
        if fingerprint_of(&material) == fingerprint {
            return Ok(Some(material.material_id));
        }
    }
    Ok(None)
}

fn persist(root: &Path, material: &WritingMaterial) -> Result<(), String> {
    write_replace_atomically(
        &material_path(root, &material.material_id)?,
        render_material(material).as_bytes(),
    )
}

pub fn create_material(root: &Path, draft: &MaterialDraft, now: f64) -> Result<WritingMaterial, String> {
    assert_work_root(root)?;
    let title = draft.title.trim().to_string();
    let content = draft.content.trim().to_string();
    let material_type = draft.material_type.trim().to_string();
    let source_type = draft.source_type.trim().to_string();
    let mut source_id = draft.source_id.trim().to_string();
    if source_type == SOURCE_USER && source_id.is_empty() {
        source_id = SOURCE_USER.to_string();
    }
    let source_quote = draft.source_quote.trim().to_string();
    let id = match draft.material_id.clone().map(|value| value.trim().to_string()) {
        Some(id) if !id.is_empty() => {
            if !is_safe_id(&id) || id.chars().count() > MAX_ID_CHARS {
                return Err(format!("素材编号不合法：{id:?}"));
            }
            if material_path(root, &id)?.exists() {
                return Err(format!("素材已存在：{id}"));
            }
            id
        }
        _ => new_file_id("m", now),
    };
    let stamp = crate::task_shell::utc_iso8601(now);
    let confidence = draft.confidence.unwrap_or(if source_type == SOURCE_USER {
        1.0
    } else {
        0.0
    });
    let material = WritingMaterial {
        schema: MATERIAL_SCHEMA.to_string(),
        material_id: id.clone(),
        title,
        content,
        material_type,
        source_type,
        source_id,
        source_quote,
        source_location: normalize_opt(draft.source_location.clone()),
        related_task_ids: normalize_link_ids(&draft.related_task_ids, "关联任务")?,
        related_project_id: normalize_opt(draft.related_project_id.clone()),
        related_knowledge_ids: normalize_link_ids(
            &draft.related_knowledge_ids,
            "关联知识",
        )?,
        event_date: normalize_opt(draft.event_date.clone()),
        created_at: stamp.clone(),
        updated_at: stamp.clone(),
        confidence,
        review_status: REVIEW_PENDING.to_string(),
        confirmation_status: CONFIRMATION_UNCONFIRMED.to_string(),
        tags: normalize_tags(&draft.tags)?,
        review_records: vec![ReviewRecord {
            at: stamp,
            action: "创建".to_string(),
            reason: String::new(),
        }],
    };
    validate_material(&material)?;
    if let Some(existing) = duplicate_with(root, &material.source_id, effective_quote(&material), None)? {
        return Err(format!("已有同指纹素材（{existing}），不重复创建。"));
    }
    fs::create_dir_all(material_dir(root))
        .map_err(|e| format!("创建素材篮目录失败：{e}"))?;
    write_new_atomically(
        &material_path(root, &id)?,
        render_material(&material).as_bytes(),
    )?;
    Ok(material)
}

pub fn read_material(root: &Path, id: &str) -> Result<WritingMaterial, String> {
    assert_work_root(root)?;
    let path = material_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取素材失败（{}）：{}", path.display(), e))?;
    parse_material(&raw)
}

fn matches_filter(material: &WritingMaterial, filter: &MaterialFilter) -> bool {
    if let Some(value) = filter.material_type.as_deref() {
        if material.material_type != value.trim() {
            return false;
        }
    }
    if let Some(value) = filter.review_status.as_deref() {
        if material.review_status != value.trim() {
            return false;
        }
    }
    if let Some(from) = filter.date_from.as_deref() {
        match material.event_date.as_deref() {
            Some(date) if date >= from.trim() => {}
            _ => return false,
        }
    }
    if let Some(to) = filter.date_to.as_deref() {
        match material.event_date.as_deref() {
            Some(date) if date <= to.trim() => {}
            _ => return false,
        }
    }
    if let Some(project) = filter.project_id.as_deref() {
        if material.related_project_id.as_deref() != Some(project.trim()) {
            return false;
        }
    }
    if let Some(task) = filter.task_id.as_deref() {
        let task = task.trim();
        if !material.related_task_ids.iter().any(|id| id == task) {
            return false;
        }
    }
    true
}

fn validate_range(filter: &MaterialFilter) -> Result<(), String> {
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
    if let Some(value) = filter.material_type.as_deref() {
        validate_material_type(value.trim())?;
    }
    if let Some(value) = filter.review_status.as_deref() {
        validate_review_status(value.trim())?;
    }
    Ok(())
}

pub fn list_materials(root: &Path, filter: &MaterialFilter) -> Result<MaterialListing, String> {
    assert_work_root(root)?;
    validate_range(filter)?;
    let (items, broken) = read_all(root)?;
    let mut items: Vec<WritingMaterial> = items
        .into_iter()
        .filter(|material| matches_filter(material, filter))
        .collect();
    items.sort_by(|a, b| {
        let rank = |status: &str| if status == REVIEW_PENDING { 0u8 } else { 1u8 };
        rank(&a.review_status)
            .cmp(&rank(&b.review_status))
            .then_with(|| b.event_date.cmp(&a.event_date))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.material_id.cmp(&b.material_id))
    });
    Ok(MaterialListing { items, broken })
}

pub fn review_material(
    root: &Path,
    id: &str,
    action: &str,
    reason: Option<&str>,
    now: f64,
) -> Result<WritingMaterial, String> {
    let mut material = read_material(root, id)?;
    let action = action.trim().to_string();
    let reason = reason
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    match action.as_str() {
        ACTION_CONFIRM => {
            if material.review_status == REVIEW_CONFIRMED {
                return Err("素材已经是已确认状态。".to_string());
            }
            if material.review_status == REVIEW_ARCHIVED {
                return Err("已归档素材不能再确认。".to_string());
            }
            material.review_status = REVIEW_CONFIRMED.to_string();
            material.confirmation_status = CONFIRMATION_CONFIRMED.to_string();
        }
        ACTION_RETURN => {
            if reason.is_empty() {
                return Err("退回素材必须填写理由。".to_string());
            }
            if !matches!(
                material.review_status.as_str(),
                REVIEW_PENDING | REVIEW_CONFIRMED
            ) {
                return Err(format!("当前状态不能退回：{}", material.review_status));
            }
            material.review_status = REVIEW_RETURNED.to_string();
            material.confirmation_status = CONFIRMATION_UNCONFIRMED.to_string();
        }
        ACTION_REJECT => {
            if reason.is_empty() {
                return Err("拒绝素材必须填写理由。".to_string());
            }
            if !matches!(
                material.review_status.as_str(),
                REVIEW_PENDING | REVIEW_RETURNED
            ) {
                return Err(format!("当前状态不能拒绝：{}", material.review_status));
            }
            material.review_status = REVIEW_REJECTED.to_string();
            material.confirmation_status = CONFIRMATION_UNCONFIRMED.to_string();
        }
        ACTION_DEFER => {
            if material.review_status != REVIEW_PENDING {
                return Err(format!("只有待审核素材可以暂缓：{}", material.review_status));
            }
        }
        ACTION_ARCHIVE => {
            if material.review_status != REVIEW_CONFIRMED {
                return Err(format!("只有已确认素材可以归档：{}", material.review_status));
            }
            material.review_status = REVIEW_ARCHIVED.to_string();
        }
        _ => return Err(format!("审核动作不认识：{action}")),
    }
    let stamp = crate::task_shell::utc_iso8601(now);
    material.review_records.push(ReviewRecord {
        at: stamp.clone(),
        action: action.clone(),
        reason,
    });
    material.updated_at = stamp;
    validate_material(&material)?;
    persist(root, &material)?;
    Ok(material)
}

pub fn update_material(
    root: &Path,
    id: &str,
    patch: &MaterialPatch,
    now: f64,
) -> Result<WritingMaterial, String> {
    let mut material = read_material(root, id)?;
    if let Some(value) = patch.title.clone() {
        material.title = value.trim().to_string();
    }
    if let Some(value) = patch.content.clone() {
        material.content = value.trim().to_string();
    }
    if let Some(value) = patch.material_type.clone() {
        material.material_type = value.trim().to_string();
    }
    if let Some(value) = patch.source_id.clone() {
        let trimmed = value.trim().to_string();
        material.source_id = if trimmed.is_empty() && material.source_type == SOURCE_USER {
            SOURCE_USER.to_string()
        } else {
            trimmed
        };
    }
    if let Some(value) = patch.source_quote.clone() {
        material.source_quote = value.trim().to_string();
    }
    if let Some(value) = patch.source_location.clone() {
        material.source_location = normalize_opt(Some(value));
    }
    if let Some(value) = patch.related_task_ids.clone() {
        material.related_task_ids = normalize_link_ids(&value, "关联任务")?;
    }
    if let Some(value) = patch.related_project_id.clone() {
        material.related_project_id = normalize_opt(Some(value));
    }
    if let Some(value) = patch.related_knowledge_ids.clone() {
        material.related_knowledge_ids = normalize_link_ids(&value, "关联知识")?;
    }
    if let Some(value) = patch.event_date.clone() {
        material.event_date = normalize_opt(Some(value));
    }
    if let Some(value) = patch.confidence {
        material.confidence = value;
    }
    if let Some(value) = patch.tags.clone() {
        material.tags = normalize_tags(&value)?;
    }
    let stamp = crate::task_shell::utc_iso8601(now);
    material.review_records.push(ReviewRecord {
        at: stamp.clone(),
        action: ACTION_EDIT.to_string(),
        reason: patch
            .reason
            .as_deref()
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
    });
    material.updated_at = stamp;
    validate_material(&material)?;
    if let Some(existing) = duplicate_with(
        root,
        &material.source_id,
        effective_quote(&material),
        Some(id),
    )? {
        return Err(format!("已有同指纹素材（{existing}），不重复创建。"));
    }
    persist(root, &material)?;
    Ok(material)
}

/// 删除 = 移到 work/.trash/素材篮/，不改写字节。
pub fn delete_material(root: &Path, id: &str, now: f64) -> Result<String, String> {
    assert_work_root(root)?;
    let source = material_path(root, id)?;
    if !source.is_file() {
        return Err(format!("素材不存在，未删除：{id}"));
    }
    let trash = trash_material_dir(root);
    fs::create_dir_all(&trash).map_err(|e| format!("创建素材回收目录失败：{e}"))?;
    let stamp = now.floor() as i64;
    let mut target = trash.join(format!("{id}.{stamp}.md"));
    let mut attempt = 0u32;
    while target.exists() {
        attempt += 1;
        target = trash.join(format!("{id}.{stamp}-{attempt}.md"));
    }
    fs::rename(&source, &target).map_err(|e| {
        format!(
            "移动到素材回收目录失败（{} → {}）：{}",
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
                "omnidesk-km501-material-{tag}-{}-{}",
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

    fn draft(title: &str, quote: &str) -> MaterialDraft {
        MaterialDraft {
            title: title.to_string(),
            content: "合成素材正文。".to_string(),
            material_type: TYPE_WORK_FACT.to_string(),
            source_type: SOURCE_WORK_RECORD.to_string(),
            source_id: "r-001".to_string(),
            source_quote: quote.to_string(),
            ..Default::default()
        }
    }

    fn create(temp: &TempRoot, title: &str, quote: &str) -> WritingMaterial {
        create_material(temp.root(), &draft(title, quote), NOW).unwrap()
    }

    #[test]
    fn create_writes_frontmatter_and_review_section() {
        let temp = TempRoot::new("create");
        let material = create(&temp, "试点推进情况", "本周完成第一轮试点。");
        let raw = fs::read_to_string(material_path(temp.root(), &material.material_id).unwrap())
            .unwrap();
        assert!(raw.contains("schema: \"omnidesk.material.v1\""), "{raw}");
        assert!(raw.contains("## 审核记录"), "{raw}");
        assert_eq!(material.review_status, REVIEW_PENDING);
        assert_eq!(material.confirmation_status, CONFIRMATION_UNCONFIRMED);
        assert_eq!(material.review_records.len(), 1);
        assert_eq!(material.review_records[0].action, "创建");
    }

    #[test]
    fn create_defaults_confidence_by_source() {
        let temp = TempRoot::new("confidence-default");
        let extracted = create(&temp, "提取事实", "记录原文。");
        assert_eq!(extracted.confidence, 0.0);
        let mut user = draft("用户收藏", "用户提供的原文。");
        user.source_type = SOURCE_USER.to_string();
        user.source_id.clear();
        let material = create_material(temp.root(), &user, NOW).unwrap();
        assert_eq!(material.source_id, SOURCE_USER);
        assert_eq!(material.confidence, 1.0);
    }

    #[test]
    fn create_rejects_empty_fields_and_unknown_enums() {
        let temp = TempRoot::new("empty");
        let mut bad = draft("", "引用");
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("标题不能为空"));
        bad.title = "标题".to_string();
        bad.content.clear();
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("正文不能为空"));
        let mut bad = draft("标题", "");
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("source_quote"));
        bad.source_quote = "引用".to_string();
        bad.material_type = "未定义类型".to_string();
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("素材类型不认识"));
        bad.material_type = TYPE_WORK_FACT.to_string();
        bad.source_type = "other".to_string();
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("来源类型不认识"));
    }

    #[test]
    fn fingerprint_normalizes_whitespace_and_source() {
        let first = material_fingerprint(" r-001 ", "本周  完成 第一轮试点。");
        let second = material_fingerprint("r-001", "本周 完成\n第一轮试点。");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert_ne!(first, material_fingerprint("r-002", "本周 完成 第一轮试点。"));
    }

    #[test]
    fn duplicate_fingerprint_is_rejected() {
        let temp = TempRoot::new("duplicate");
        create(&temp, "第一次", "本周 完成 第一轮试点。");
        let err = create_material(
            temp.root(),
            &draft("换个标题", "本周\n完成   第一轮试点。"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("同指纹"), "{err}");
        assert_eq!(list_materials(temp.root(), &MaterialFilter::default()).unwrap().items.len(), 1);
    }

    #[test]
    fn different_source_with_same_quote_is_allowed() {
        let temp = TempRoot::new("source-diff");
        create(&temp, "记录来源", "同一句原文。");
        let mut other = draft("任务来源", "同一句原文。");
        other.source_type = SOURCE_TASK.to_string();
        other.source_id = "t-001".to_string();
        create_material(temp.root(), &other, NOW).unwrap();
        assert_eq!(list_materials(temp.root(), &MaterialFilter::default()).unwrap().items.len(), 2);
    }

    #[test]
    fn read_round_trips_all_fields_and_lists() {
        let temp = TempRoot::new("roundtrip");
        let mut source = draft("完整字段", "原文引用。");
        source.source_location = Some("第 2 段".to_string());
        source.related_task_ids = vec!["t-001".to_string(), "t-002".to_string()];
        source.related_project_id = Some("p-001".to_string());
        source.related_knowledge_ids = vec!["k-001".to_string()];
        source.event_date = Some("2026-11-09".to_string());
        source.confidence = Some(0.8);
        source.tags = vec!["试点".to_string(), "数据".to_string()];
        let created = create_material(temp.root(), &source, NOW).unwrap();
        let loaded = read_material(temp.root(), &created.material_id).unwrap();
        assert_eq!(loaded, created);
        assert_eq!(loaded.event_date.as_deref(), Some("2026-11-09"));
        assert_eq!(loaded.related_task_ids.len(), 2);
        assert_eq!(loaded.related_knowledge_ids, vec!["k-001"]);
        assert_eq!(loaded.tags, vec!["试点", "数据"]);
    }

    #[test]
    fn list_filters_by_type_and_status() {
        let temp = TempRoot::new("filter-type");
        let mut result = draft("成果", "成果原文。");
        result.material_type = TYPE_WORK_RESULT.to_string();
        result.source_id = "r-002".to_string();
        let result = create_material(temp.root(), &result, NOW).unwrap();
        let fact = create(&temp, "事实", "事实原文。");
        review_material(temp.root(), &result.material_id, ACTION_CONFIRM, None, NOW + 60.0)
            .unwrap();
        let confirmed = list_materials(
            temp.root(),
            &MaterialFilter {
                review_status: Some(REVIEW_CONFIRMED.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(confirmed.items.len(), 1);
        assert_eq!(confirmed.items[0].material_id, result.material_id);
        let facts = list_materials(
            temp.root(),
            &MaterialFilter {
                material_type: Some(TYPE_WORK_FACT.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(facts.items.len(), 1);
        assert_eq!(facts.items[0].material_id, fact.material_id);
        assert!(list_materials(
            temp.root(),
            &MaterialFilter {
                material_type: Some("未定义".to_string()),
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn list_filters_by_event_date_range() {
        let temp = TempRoot::new("filter-date");
        let mut early = draft("早期", "早期原文。");
        early.source_id = "r-early".to_string();
        early.event_date = Some("2026-11-01".to_string());
        let early = create_material(temp.root(), &early, NOW).unwrap();
        let mut late = draft("后期", "后期原文。");
        late.source_id = "r-late".to_string();
        late.event_date = Some("2026-11-20".to_string());
        let late = create_material(temp.root(), &late, NOW).unwrap();
        let listing = list_materials(
            temp.root(),
            &MaterialFilter {
                date_from: Some("2026-11-10".to_string()),
                date_to: Some("2026-11-30".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].material_id, late.material_id);
        assert!(!listing.items.iter().any(|item| item.material_id == early.material_id));
        let err = list_materials(
            temp.root(),
            &MaterialFilter {
                date_from: Some("2026-11-30".to_string()),
                date_to: Some("2026-11-01".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("不能晚于"));
    }

    #[test]
    fn list_filters_by_project_and_task() {
        let temp = TempRoot::new("filter-links");
        let mut linked = draft("关联素材", "关联原文。");
        linked.related_project_id = Some("p-001".to_string());
        linked.related_task_ids = vec!["t-001".to_string()];
        let linked = create_material(temp.root(), &linked, NOW).unwrap();
        let unlinked = create(&temp, "未关联", "未关联原文。");
        let by_project = list_materials(
            temp.root(),
            &MaterialFilter {
                project_id: Some("p-001".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_project.items.len(), 1);
        assert_eq!(by_project.items[0].material_id, linked.material_id);
        let by_task = list_materials(
            temp.root(),
            &MaterialFilter {
                task_id: Some("t-001".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_task.items.len(), 1);
        assert_eq!(by_task.items[0].material_id, linked.material_id);
        assert!(!by_task.items.iter().any(|item| item.material_id == unlinked.material_id));
    }

    #[test]
    fn list_puts_pending_first() {
        let temp = TempRoot::new("sort");
        let confirmed = create(&temp, "已确认", "确认原文。");
        review_material(temp.root(), &confirmed.material_id, ACTION_CONFIRM, None, NOW + 60.0)
            .unwrap();
        let pending = create(&temp, "待审核", "待审原文。");
        let listing = list_materials(temp.root(), &MaterialFilter::default()).unwrap();
        assert_eq!(listing.items[0].material_id, pending.material_id);
        assert_eq!(listing.items[1].material_id, confirmed.material_id);
    }

    #[test]
    fn review_confirm_sets_confirmation_status() {
        let temp = TempRoot::new("confirm");
        let material = create(&temp, "待确认素材", "待确认原文。");
        let reviewed = review_material(temp.root(), &material.material_id, ACTION_CONFIRM, None, NOW + 60.0)
            .unwrap();
        assert_eq!(reviewed.review_status, REVIEW_CONFIRMED);
        assert_eq!(reviewed.confirmation_status, CONFIRMATION_CONFIRMED);
        assert_eq!(reviewed.review_records.len(), 2);
    }

    #[test]
    fn review_return_and_reject_require_reason() {
        let temp = TempRoot::new("reason");
        let material = create(&temp, "待审素材", "待审原文。");
        let err = review_material(temp.root(), &material.material_id, ACTION_RETURN, Some("  "), NOW)
            .unwrap_err();
        assert!(err.contains("必须填写理由"));
        let returned = review_material(
            temp.root(),
            &material.material_id,
            ACTION_RETURN,
            Some("引用不完整"),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(returned.review_status, REVIEW_RETURNED);
        let err = review_material(temp.root(), &material.material_id, ACTION_REJECT, None, NOW)
            .unwrap_err();
        assert!(err.contains("必须填写理由"));
        let rejected = review_material(
            temp.root(),
            &material.material_id,
            ACTION_REJECT,
            Some("与事实不符"),
            NOW + 120.0,
        )
        .unwrap();
        assert_eq!(rejected.review_status, REVIEW_REJECTED);
    }

    #[test]
    fn review_records_are_append_only() {
        let temp = TempRoot::new("append");
        let material = create(&temp, "记录留痕", "留痕原文。");
        review_material(temp.root(), &material.material_id, ACTION_CONFIRM, None, NOW + 60.0)
            .unwrap();
        let archived = review_material(temp.root(), &material.material_id, ACTION_ARCHIVE, None, NOW + 120.0)
            .unwrap();
        assert_eq!(archived.review_records.len(), 3);
        assert_eq!(archived.review_records[0].action, "创建");
        assert_eq!(archived.review_records[1].action, ACTION_CONFIRM);
        assert_eq!(archived.review_records[2].action, ACTION_ARCHIVE);
        let raw = fs::read_to_string(material_path(temp.root(), &material.material_id).unwrap())
            .unwrap();
        assert_eq!(raw.matches("| 创建 |").count(), 1);
    }

    #[test]
    fn review_rejects_invalid_transition() {
        let temp = TempRoot::new("transition");
        let material = create(&temp, "非法流转", "非法流转原文。");
        assert!(review_material(temp.root(), &material.material_id, ACTION_ARCHIVE, None, NOW)
            .unwrap_err()
            .contains("只有已确认"));
        assert!(review_material(temp.root(), &material.material_id, "通过", None, NOW)
            .unwrap_err()
            .contains("审核动作不认识"));
        review_material(temp.root(), &material.material_id, ACTION_CONFIRM, None, NOW + 60.0)
            .unwrap();
        assert!(review_material(temp.root(), &material.material_id, ACTION_CONFIRM, None, NOW + 120.0)
            .unwrap_err()
            .contains("已经是已确认"));
    }

    #[test]
    fn update_keeps_created_at_and_appends_edit() {
        let temp = TempRoot::new("update");
        let material = create(&temp, "原标题", "原引用。");
        let updated = update_material(
            temp.root(),
            &material.material_id,
            &MaterialPatch {
                title: Some("新标题".to_string()),
                tags: Some(vec!["更新".to_string()]),
                reason: Some("补充标签".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.created_at, material.created_at);
        assert_eq!(updated.review_records.len(), 2);
        assert_eq!(updated.review_records[1].action, ACTION_EDIT);
        assert_eq!(updated.review_records[1].reason, "补充标签");
    }

    #[test]
    fn update_rechecks_fingerprint() {
        let temp = TempRoot::new("update-duplicate");
        create(&temp, "甲", "甲原文。");
        let mut other = draft("乙", "乙原文。");
        other.source_id = "r-002".to_string();
        let other = create_material(temp.root(), &other, NOW).unwrap();
        let err = update_material(
            temp.root(),
            &other.material_id,
            &MaterialPatch {
                source_id: Some("r-001".to_string()),
                source_quote: Some("甲原文。".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("同指纹"), "{err}");
        assert_eq!(read_material(temp.root(), &other.material_id).unwrap(), other);
    }

    #[test]
    fn delete_moves_to_trash_preserving_bytes() {
        let temp = TempRoot::new("delete");
        let material = create(&temp, "待回收", "回收原文。");
        let path = material_path(temp.root(), &material.material_id).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let trashed = delete_material(temp.root(), &material.material_id, NOW).unwrap();
        assert!(!path.exists());
        assert!(trashed.contains("素材篮"));
        let after = fs::read_to_string(&trashed).unwrap();
        assert_eq!(before, after);
        assert!(read_material(temp.root(), &material.material_id).is_err());
    }

    #[test]
    fn create_rejects_path_like_or_existing_id() {
        let temp = TempRoot::new("id");
        let mut bad = draft("标题", "引用。");
        bad.material_id = Some("../逃逸".to_string());
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("编号不合法"));
        let material = create(&temp, "已存在", "已存在原文。");
        let mut duplicate = draft("重复编号", "重复编号原文。");
        duplicate.material_id = Some(material.material_id.clone());
        assert!(create_material(temp.root(), &duplicate, NOW)
            .unwrap_err()
            .contains("素材已存在"));
    }

    #[test]
    fn rejects_bad_date_and_confidence() {
        let temp = TempRoot::new("date-confidence");
        let mut bad = draft("日期错误", "日期原文。");
        bad.event_date = Some("2026-13-01".to_string());
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("日期形状"));
        let mut bad = draft("置信度错误", "置信度原文。");
        bad.source_id = "r-003".to_string();
        bad.confidence = Some(1.5);
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("置信度"));
        let mut bad = draft("标签过多", "标签原文。");
        bad.source_id = "r-004".to_string();
        bad.tags = (0..20).map(|index| format!("标签{index}")).collect();
        assert!(create_material(temp.root(), &bad, NOW).unwrap_err().contains("标签过多"));
    }

    #[test]
    fn list_counts_broken_files_and_keeps_valid_items() {
        let temp = TempRoot::new("broken");
        let material = create(&temp, "有效素材", "有效原文。");
        fs::write(material_dir(temp.root()).join("broken.md"), "不是约定格式").unwrap();
        let listing = list_materials(temp.root(), &MaterialFilter::default()).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].material_id, material.material_id);
        assert_eq!(listing.broken, 1);
    }
}
