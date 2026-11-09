//! 写作素材智能收集（OD-WT-605）。
//!
//! 纯查询组装器：组合 509 `material_query`、知识库词法检索与 508 项目概览，
//! 不新建索引、不落盘、不外发。输出的是推荐候选，不是可信集合；只有
//! `review_status=已确认` 的素材才能成为 `suitable_for_direct_quote=true`，
//! 也只有显式 `include_pending=true` 时待审核素材才会出现在结果里。
//!
//! 用户在 `writing_select_materials` 中的采纳会写回 601 写作任务的
//! `selected_material_ids` / `selected_knowledge_ids`，并由 601 追加状态记录。
//! 排除与重点标注同样是用户动作，记录在追加状态记录里，不新增独立实体
//! （D-085）。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::knowledge_link::{MAX_LINK_CANDIDATES, MAX_LINK_QUERY_TERMS};
use crate::material_query::{self, MaterialQueryInput, MaterialQueryRelations, MaterialQuerySource};
use crate::material_store::{
    self, MaterialFilter, REVIEW_CONFIRMED, REVIEW_PENDING, SOURCE_KNOWLEDGE, SOURCE_USER,
};
use crate::vault_search::{self, SearchQuery};
use crate::writing_store::{self, WritingTask, WritingTaskPatch};

const SUMMARY_CHARS: usize = 120;
const MAX_TERMS: usize = 24;
const MAX_LINK_ID_CHARS: usize = 200;

/// 五类区分标注。序列化值直接给界面使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialClass {
    ConfirmedFact,
    AiRecommended,
    PendingReview,
    ReferenceOnly,
    UserExternal,
}

impl MaterialClass {
    pub fn label(self) -> &'static str {
        match self {
            MaterialClass::ConfirmedFact => "已确认事实",
            MaterialClass::AiRecommended => "AI 推荐",
            MaterialClass::PendingReview => "待审核",
            MaterialClass::ReferenceOnly => "仅供参考知识",
            MaterialClass::UserExternal => "用户外部资料",
        }
    }
}

/// 推荐依据类型（D-079 组合口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationBasis {
    #[serde(rename = "已确认匹配")]
    ConfirmedMatch,
    #[serde(rename = "主题相关")]
    TopicRelated,
    #[serde(rename = "项目关联")]
    ProjectRelated,
    #[serde(rename = "知识支撑")]
    KnowledgeSupport,
}

impl RecommendationBasis {
    pub fn label(self) -> &'static str {
        match self {
            RecommendationBasis::ConfirmedMatch => "已确认匹配",
            RecommendationBasis::TopicRelated => "主题相关",
            RecommendationBasis::ProjectRelated => "项目关联",
            RecommendationBasis::KnowledgeSupport => "知识支撑",
        }
    }
}

/// 一条推荐理由：依据类型 + 一句话证据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationReason {
    pub basis: RecommendationBasis,
    pub detail: String,
}

/// 同一标题关键词、不同年月的事件日期冲突对。只提示，不合并。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialConflict {
    pub left_id: String,
    pub left_title: String,
    pub left_period: String,
    pub right_id: String,
    pub right_title: String,
    pub right_period: String,
    pub hint: String,
}

/// 冲突检测的最小输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictMaterial {
    pub material_id: String,
    pub title: String,
    pub event_date: Option<String>,
}

/// 单条推荐候选。素材与知识条目共用这个形状，`kind` 区分。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectedItem {
    pub kind: String,
    pub class: MaterialClass,
    pub class_label: String,
    pub material_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub title: String,
    pub content_summary: String,
    pub source: MaterialQuerySource,
    pub event_date: Option<String>,
    pub updated_at: Option<String>,
    pub review_status: String,
    pub related: MaterialQueryRelations,
    pub reasons: Vec<RecommendationReason>,
    pub reason_text: String,
    pub conflict_ids: Vec<String>,
    pub conflict_hint: Option<String>,
    pub suitable_for_direct_quote: bool,
    pub pending_review: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectedMaterials {
    pub writing_id: String,
    pub writing_title: String,
    pub topic: String,
    pub filter: MaterialFilter,
    pub include_pending: bool,
    pub items: Vec<CollectedItem>,
    pub conflicts: Vec<MaterialConflict>,
    pub selected_material_ids: Vec<String>,
    pub selected_knowledge_ids: Vec<String>,
    pub material_count: usize,
    pub knowledge_count: usize,
    pub pending_count: usize,
    pub conflict_count: usize,
    pub broken_materials: usize,
    pub knowledge_note: Option<String>,
}

/// 收集命令入参。过滤条件不落盘、不走红线闸门（照 506 R5 口径）；
/// `writing_id` 由 IPC 层过闸门后再进入这里。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectRequest {
    pub writing_id: String,
    pub filter: MaterialFilter,
    pub knowledge_hints: Vec<String>,
    pub include_pending: bool,
}

/// 素材选择入参。采纳加入可信素材包；排除移除；重点标注写入追加状态记录。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectionRequest {
    pub writing_id: String,
    pub adopt_material_ids: Vec<String>,
    pub adopt_knowledge_ids: Vec<String>,
    pub exclude_material_ids: Vec<String>,
    pub exclude_knowledge_ids: Vec<String>,
    pub highlight_material_ids: Vec<String>,
    pub highlight_knowledge_ids: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectionOutcome {
    pub writing_id: String,
    pub selected_material_ids: Vec<String>,
    pub selected_knowledge_ids: Vec<String>,
    pub status: String,
    pub updated_at: String,
    pub status_records: Vec<writing_store::StatusRecord>,
    pub changed: bool,
    pub summary: String,
}

fn normalize_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn summary_of(content: &str) -> String {
    let trimmed = content.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= SUMMARY_CHARS {
        trimmed.to_string()
    } else {
        let mut out: String = chars[..SUMMARY_CHARS].iter().collect();
        out.push('…');
        out
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if !values.iter().any(|existing| existing == trimmed) {
        values.push(trimmed.to_string());
    }
}

fn remove_value(values: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    values.retain(|existing| existing.as_str() != trimmed);
}

/// 写作主题词：标题、主题、目的与 AI 主题提示词。过长文本按分隔符拆短语。
fn topic_terms(task: &WritingTask, knowledge_hints: &[String]) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut push = |text: &str| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if terms.len() >= MAX_TERMS {
            return;
        }
        if !terms.iter().any(|existing| existing.to_lowercase() == trimmed.to_lowercase()) {
            terms.push(trimmed.to_string());
        }
    };
    push(&task.topic);
    push(&task.title);
    push(&task.writing_purpose);
    for hint in knowledge_hints {
        push(hint);
    }
    for field in [&task.topic, &task.title, &task.writing_purpose] {
        for part in field.split(|ch: char| !ch.is_alphanumeric()) {
            let part = part.trim();
            if part.chars().count() >= 2 && part.chars().count() <= 32 {
                push(part);
            }
        }
    }
    terms
}

fn term_hit(terms: &[String], item: &material_query::MaterialQueryItem) -> Option<String> {
    let title = item.title.to_lowercase();
    let content = item.content.to_lowercase();
    for term in terms {
        let needle = term.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        if title.contains(&needle) || content.contains(&needle) {
            return Some(term.clone());
        }
    }
    None
}

fn title_key(title: &str) -> Option<String> {
    let mut key = String::new();
    for ch in title.to_lowercase().chars() {
        if ch.is_alphanumeric() && !ch.is_ascii_digit() {
            key.push(ch);
        }
    }
    if key.chars().count() >= 2 {
        Some(key)
    } else {
        None
    }
}

fn month_of(date: &str) -> Option<String> {
    let trimmed = date.trim();
    if trimmed.len() >= 7 {
        Some(trimmed[0..7].to_string())
    } else {
        None
    }
}

/// 纯函数：同一标题关键词 + 不同事件年月 → 冲突对。
pub fn detect_conflicts(items: &[ConflictMaterial]) -> Vec<MaterialConflict> {
    let mut conflicts = Vec::new();
    for i in 0..items.len() {
        let Some(left_key) = title_key(&items[i].title) else {
            continue;
        };
        let Some(left_period) = items[i].event_date.as_deref().and_then(month_of) else {
            continue;
        };
        for right in items.iter().skip(i + 1) {
            let Some(right_key) = title_key(&right.title) else {
                continue;
            };
            let Some(right_period) = right.event_date.as_deref().and_then(month_of) else {
                continue;
            };
            if left_key == right_key && left_period != right_period {
                conflicts.push(MaterialConflict {
                    left_id: items[i].material_id.clone(),
                    left_title: items[i].title.clone(),
                    left_period: left_period.clone(),
                    right_id: right.material_id.clone(),
                    right_title: right.title.clone(),
                    right_period: right_period.clone(),
                    hint: format!(
                        "同一标题关键词出现不同时间范围（{} 与 {}）；请确认口径，不自动合并。",
                        left_period, right_period
                    ),
                });
            }
        }
    }
    conflicts
}

fn material_class(material: &material_query::MaterialQueryItem) -> MaterialClass {
    if material.review_status == REVIEW_PENDING {
        return MaterialClass::PendingReview;
    }
    match material.source.source_type.as_str() {
        SOURCE_USER => MaterialClass::UserExternal,
        SOURCE_KNOWLEDGE => MaterialClass::ReferenceOnly,
        _ if material.review_status == REVIEW_CONFIRMED => MaterialClass::ConfirmedFact,
        _ => MaterialClass::ReferenceOnly,
    }
}

fn suitable_for_direct_quote(item: &material_query::MaterialQueryItem) -> bool {
    item.review_status == REVIEW_CONFIRMED
        && item.content.trim() != item.source.source_quote.trim()
}

fn material_to_item(
    material: material_query::MaterialQueryItem,
    task: &WritingTask,
    terms: &[String],
    project_match: bool,
    knowledge_ids: &BTreeSet<String>,
    project_name: Option<&str>,
) -> CollectedItem {
    let class = material_class(&material);
    let mut reasons: Vec<RecommendationReason> = Vec::new();
    let topic_hit = term_hit(terms, &material);

    if material.review_status == REVIEW_CONFIRMED {
        reasons.push(RecommendationReason {
            basis: RecommendationBasis::ConfirmedMatch,
            detail: "审核已确认，已在可信素材范围内。".to_string(),
        });
    }
    if let Some(term) = topic_hit.as_deref() {
        reasons.push(RecommendationReason {
            basis: RecommendationBasis::TopicRelated,
            detail: format!("主题词「{term}」命中素材标题或正文。"),
        });
    }
    if project_match {
        reasons.push(RecommendationReason {
            basis: RecommendationBasis::ProjectRelated,
            detail: project_name
                .map(|name| format!("素材关联项目「{name}」"))
                .unwrap_or_else(|| "素材与当前项目范围关联。".to_string()),
        });
    }
    let linked_knowledge = material
        .related
        .knowledge
        .iter()
        .find(|entity| knowledge_ids.contains(&entity.id));
    if let Some(entity) = linked_knowledge {
        reasons.push(RecommendationReason {
            basis: RecommendationBasis::KnowledgeSupport,
            detail: format!(
                "素材关联知识 {}：该知识在当前主题检索中有命中。",
                entity.title.as_deref().unwrap_or(&entity.id)
            ),
        });
    }

    let selected = task
        .selected_material_ids
        .iter()
        .any(|id| id == &material.material_id);
    let reason_text = reasons
        .iter()
        .map(|reason| format!("{}：{}", reason.basis.label(), reason.detail))
        .collect::<Vec<_>>()
        .join("；");
    let content_summary = summary_of(&material.content);
    let display_title = material.title.clone();
    let suitable = suitable_for_direct_quote(&material);
    CollectedItem {
        kind: "material".to_string(),
        class,
        class_label: class.label().to_string(),
        material_id: Some(material.material_id.clone()),
        knowledge_id: None,
        title: display_title,
        content_summary,
        source: material.source,
        event_date: material.event_date,
        // 509 查询回执没有 updated_at；这里不把事件日期伪装成更新时间。
        updated_at: None,
        review_status: material.review_status,
        related: material.related,
        reasons,
        reason_text,
        conflict_ids: Vec::new(),
        conflict_hint: None,
        suitable_for_direct_quote: suitable,
        pending_review: class == MaterialClass::PendingReview,
        selected,
    }
}

fn knowledge_items(
    vault: Option<&Path>,
    knowledge_hints: &[String],
    selected_knowledge: &[String],
) -> (Vec<CollectedItem>, Option<String>) {
    let mut terms: Vec<String> = Vec::new();
    for hint in knowledge_hints {
        let trimmed = hint.trim();
        if !trimmed.is_empty()
            && !terms.iter().any(|existing| existing.to_lowercase() == trimmed.to_lowercase())
        {
            if terms.len() >= MAX_LINK_QUERY_TERMS {
                break;
            }
            terms.push(trimmed.to_string());
        }
    }
    if terms.is_empty() {
        return (Vec::new(), None);
    }
    let Some(vault_root) = vault else {
        return (
            Vec::new(),
            Some("知识库未配置或不可读，未执行知识支撑检索。".to_string()),
        );
    };

    let mut items: Vec<CollectedItem> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut note: Option<String> = None;
    for term in &terms {
        if items.len() >= MAX_LINK_CANDIDATES {
            note = Some(format!("知识支撑候选已封顶 {} 条。", MAX_LINK_CANDIDATES));
            break;
        }
        let response = match vault_search::search_vault(
            vault_root,
            &SearchQuery {
                keyword: term.clone(),
                ..Default::default()
            },
        ) {
            Ok(response) => response,
            Err(error) => {
                note = Some(format!("知识检索失败：{error}"));
                break;
            }
        };
        for hit in response.hits {
            if items.len() >= MAX_LINK_CANDIDATES {
                note = Some(format!("知识支撑候选已封顶 {} 条。", MAX_LINK_CANDIDATES));
                break;
            }
            if !seen.insert(hit.relative_path.clone()) {
                continue;
            }
            let line = hit.snippet_line;
            let location = line.map(|line| format!("第 {line} 行"));
            let snippet = hit
                .snippet
                .clone()
                .unwrap_or_else(|| "知识条目与主题词相关。".to_string());
            let selected = selected_knowledge.iter().any(|id| id == &hit.relative_path);
            items.push(CollectedItem {
                kind: "knowledge".to_string(),
                class: MaterialClass::AiRecommended,
                class_label: MaterialClass::AiRecommended.label().to_string(),
                material_id: None,
                knowledge_id: Some(hit.relative_path.clone()),
                title: hit.file_name.clone(),
                content_summary: summary_of(&snippet),
                source: MaterialQuerySource {
                    source_type: SOURCE_KNOWLEDGE.to_string(),
                    source_id: hit.relative_path.clone(),
                    source_quote: snippet.clone(),
                    source_location: location,
                },
                event_date: None,
                updated_at: None,
                review_status: "知识条目（未审核）".to_string(),
                related: MaterialQueryRelations::default(),
                reasons: vec![RecommendationReason {
                    basis: RecommendationBasis::KnowledgeSupport,
                    detail: format!("AI 主题提示词「{term}」命中知识库。"),
                }],
                reason_text: format!(
                    "知识支撑：AI 主题提示词「{term}」命中知识库。"
                ),
                conflict_ids: Vec::new(),
                conflict_hint: None,
                suitable_for_direct_quote: false,
                pending_review: false,
                selected,
            });
        }
    }
    (items, note)
}

fn query_for(filter: &MaterialFilter, status: &str) -> MaterialQueryInput {
    MaterialQueryInput {
        keyword: String::new(),
        material_type: filter.material_type.clone(),
        date_from: filter.date_from.clone(),
        date_to: filter.date_to.clone(),
        project_id: filter.project_id.clone(),
        review_status: Some(status.to_string()),
    }
}

/// 默认口径：只查已确认素材；知识库检索只在显式需要知识支撑时传入时执行。
/// 这个入口不接收 vault，也不能打开待审核，适合独立调用或测试默认边界。
pub fn collect_materials(
    root: &Path,
    filter: &MaterialFilter,
    writing_id: &str,
    knowledge_hints: &[String],
) -> Result<CollectedMaterials, String> {
    collect_materials_from(root, None, filter, writing_id, knowledge_hints, false)
}

/// 完整收集入口：vault 可选；`include_pending` 只能显式打开待审核候选。
pub fn collect_materials_from(
    root: &Path,
    vault: Option<&Path>,
    filter: &MaterialFilter,
    writing_id: &str,
    knowledge_hints: &[String],
    include_pending: bool,
) -> Result<CollectedMaterials, String> {
    let task = writing_store::read_writing_task(root, writing_id)?;
    let terms = topic_terms(&task, knowledge_hints);

    let statuses: Vec<&str> = match filter.review_status.as_deref() {
        Some(REVIEW_PENDING) if !include_pending => {
            return Err("待审核素材需要显式 include_pending=true 才会进入推荐列表。".to_string());
        }
        Some(status) => vec![status],
        None if include_pending => vec![REVIEW_CONFIRMED, REVIEW_PENDING],
        None => vec![REVIEW_CONFIRMED],
    };

    let (project_name, project_task_ids, project_material_ids) = if let Some(project_id) =
        normalize_opt(filter.project_id.as_deref())
    {
        let overview = crate::project_overview_impl(root, &project_id)
            .map_err(|error| format!("关联项目概览不可读：{error}"))?;
        let task_ids: BTreeSet<String> = overview.tasks.iter().map(|task| task.id.clone()).collect();
        let material_ids: BTreeSet<String> = overview
            .materials
            .iter()
            .map(|material| material.id.clone())
            .collect();
        (Some(overview.project.name), task_ids, material_ids)
    } else {
        (None, BTreeSet::new(), BTreeSet::new())
    };

    let mut material_items: Vec<material_query::MaterialQueryItem> = Vec::new();
    let mut broken = 0usize;
    for status in &statuses {
        let query = query_for(filter, status);
        let response = material_query::query_materials(root, vault, &query)?;
        broken = broken.max(response.broken);
        for item in response.items {
            if let Some(task_id) = normalize_opt(filter.task_id.as_deref()) {
                if !item.related.tasks.iter().any(|entity| entity.id == task_id) {
                    continue;
                }
            }
            material_items.push(item);
        }
    }

    let (knowledge, knowledge_note) = knowledge_items(vault, knowledge_hints, &task.selected_knowledge_ids);
    let knowledge_ids: BTreeSet<String> = knowledge
        .iter()
        .filter_map(|item| item.knowledge_id.clone())
        .collect();

    let mut items: Vec<CollectedItem> = material_items
        .into_iter()
        .map(|material| {
            let project_match = project_name.is_some()
                && (project_material_ids.contains(&material.material_id)
                    || material.related.project.as_ref().map(|entity| entity.found).unwrap_or(false)
                    || material
                        .related
                        .tasks
                        .iter()
                        .any(|entity| project_task_ids.contains(&entity.id)));
            material_to_item(
                material,
                &task,
                &terms,
                project_match,
                &knowledge_ids,
                project_name.as_deref(),
            )
        })
        .collect();
    items.extend(knowledge);

    let conflict_inputs: Vec<ConflictMaterial> = items
        .iter()
        .filter_map(|item| {
            item.material_id.as_ref().map(|id| ConflictMaterial {
                material_id: id.clone(),
                title: item.title.clone(),
                event_date: item.event_date.clone(),
            })
        })
        .collect();
    let conflicts = detect_conflicts(&conflict_inputs);
    for conflict in &conflicts {
        let hint = conflict.hint.clone();
        for item in &mut items {
            if let Some(id) = item.material_id.as_ref() {
                let is_left = id == &conflict.left_id;
                let is_right = id == &conflict.right_id;
                if is_left || is_right {
                    let other = if is_left {
                        conflict.right_id.clone()
                    } else {
                        conflict.left_id.clone()
                    };
                    push_unique(&mut item.conflict_ids, &other);
                    item.conflict_hint = Some(match item.conflict_hint.take() {
                        Some(existing) => format!("{existing}；{hint}"),
                        None => hint.clone(),
                    });
                }
            }
        }
    }

    items.sort_by(|left, right| {
        let rank = |class: MaterialClass| match class {
            MaterialClass::ConfirmedFact => 0u8,
            MaterialClass::UserExternal => 1u8,
            MaterialClass::ReferenceOnly => 2u8,
            MaterialClass::AiRecommended => 3u8,
            MaterialClass::PendingReview => 4u8,
        };
        right
            .selected
            .cmp(&left.selected)
            .then_with(|| rank(left.class).cmp(&rank(right.class)))
            .then_with(|| right.event_date.cmp(&left.event_date))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.source.source_id.cmp(&right.source.source_id))
    });

    let material_count = items.iter().filter(|item| item.kind == "material").count();
    let knowledge_count = items.iter().filter(|item| item.kind == "knowledge").count();
    let pending_count = items.iter().filter(|item| item.pending_review).count();
    Ok(CollectedMaterials {
        writing_id: task.writing_id.clone(),
        writing_title: task.title.clone(),
        topic: task.topic.clone(),
        filter: filter.clone(),
        include_pending,
        items,
        conflict_count: conflicts.len(),
        conflicts,
        selected_material_ids: task.selected_material_ids.clone(),
        selected_knowledge_ids: task.selected_knowledge_ids.clone(),
        material_count,
        knowledge_count,
        pending_count,
        broken_materials: broken,
        knowledge_note,
    })
}

fn validate_link_id(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("素材或知识编号不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_LINK_ID_CHARS
        || trimmed.chars().any(|ch| ch.is_control())
    {
        return Err(format!("素材或知识编号不合法：{trimmed:?}"));
    }
    Ok(trimmed.to_string())
}

fn apply_ids(
    target: &mut Vec<String>,
    adopt: &[String],
    exclude: &[String],
) -> Result<usize, String> {
    let mut changed = 0usize;
    for id in exclude {
        let id = validate_link_id(id)?;
        let before = target.len();
        remove_value(target, &id);
        if target.len() != before {
            changed += 1;
        }
    }
    for id in adopt {
        let id = validate_link_id(id)?;
        if !target.iter().any(|existing| existing == &id) {
            target.push(id);
            changed += 1;
        }
    }
    Ok(changed)
}

/// 应用用户选择。采纳素材先校验 `review_status=已确认`，未确认素材不能进可信集合。
/// 所有变更都经由 601 `update_writing_task` 落盘，状态记录只追加。
pub fn apply_selection(
    root: &Path,
    request: &SelectionRequest,
    now: f64,
) -> Result<SelectionOutcome, String> {
    let task = writing_store::read_writing_task(root, &request.writing_id)?;
    let mut selected_materials = task.selected_material_ids.clone();
    let mut selected_knowledge = task.selected_knowledge_ids.clone();

    for id in &request.adopt_material_ids {
        let id = validate_link_id(id)?;
        let material = material_store::read_material(root, &id)
            .map_err(|error| format!("采纳素材前读取失败：{error}"))?;
        if material.review_status != REVIEW_CONFIRMED {
            return Err(format!(
                "素材「{}」当前审核状态为「{}」；只有已确认素材可以加入可信素材包。",
                id, material.review_status
            ));
        }
    }

    let material_changes = apply_ids(
        &mut selected_materials,
        &request.adopt_material_ids,
        &request.exclude_material_ids,
    )?;
    let knowledge_changes = apply_ids(
        &mut selected_knowledge,
        &request.adopt_knowledge_ids,
        &request.exclude_knowledge_ids,
    )?;

    let mut summary_parts: Vec<String> = Vec::new();
    if !request.adopt_material_ids.is_empty() {
        summary_parts.push(format!("采纳素材：{}", request.adopt_material_ids.join("、")));
    }
    if !request.exclude_material_ids.is_empty() {
        summary_parts.push(format!("排除素材：{}", request.exclude_material_ids.join("、")));
    }
    if !request.adopt_knowledge_ids.is_empty() {
        summary_parts.push(format!("采纳知识：{}", request.adopt_knowledge_ids.join("、")));
    }
    if !request.exclude_knowledge_ids.is_empty() {
        summary_parts.push(format!("排除知识：{}", request.exclude_knowledge_ids.join("、")));
    }
    let mut highlights: Vec<String> = Vec::new();
    for id in &request.highlight_material_ids {
        let id = validate_link_id(id)?;
        highlights.push(format!("素材 {id}"));
    }
    for id in &request.highlight_knowledge_ids {
        let id = validate_link_id(id)?;
        highlights.push(format!("知识 {id}"));
    }
    if !highlights.is_empty() {
        summary_parts.push(format!("标注重点：{}", highlights.join("、")));
    }
    let note = normalize_opt(request.note.as_deref());
    if let Some(note) = note.as_deref() {
        summary_parts.push(format!("说明：{note}"));
    }
    let movement = material_changes + knowledge_changes;
    let has_annotation = !highlights.is_empty() || note.is_some();
    if movement == 0 && !has_annotation {
        return Ok(SelectionOutcome {
            writing_id: task.writing_id.clone(),
            selected_material_ids: task.selected_material_ids.clone(),
            selected_knowledge_ids: task.selected_knowledge_ids.clone(),
            status: task.status.clone(),
            updated_at: task.updated_at.clone(),
            status_records: task.status_records.clone(),
            changed: false,
            summary: "没有新的素材选择变更。".to_string(),
        });
    }

    let summary = summary_parts.join("；");
    let patch = WritingTaskPatch {
        selected_material_ids: Some(selected_materials),
        selected_knowledge_ids: Some(selected_knowledge),
        reason: Some(format!("素材包选择：{summary}")),
        ..Default::default()
    };
    let updated = writing_store::update_writing_task(root, &request.writing_id, &patch, now)?;
    Ok(SelectionOutcome {
        writing_id: updated.writing_id.clone(),
        selected_material_ids: updated.selected_material_ids.clone(),
        selected_knowledge_ids: updated.selected_knowledge_ids.clone(),
        status: updated.status.clone(),
        updated_at: updated.updated_at.clone(),
        status_records: updated.status_records.clone(),
        changed: true,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_store;
    use std::fs;
    use std::path::PathBuf;

    const NOW: f64 = 1_780_200_000.0;
    const CONFIRM_AT: f64 = 1_780_200_100.0;

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        vault: PathBuf,
        writing_id: String,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-collect-{tag}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("work");
            let vault = base.join("vault");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&vault).unwrap();
            let task = writing_store::create_writing_task(
                &root,
                &writing_store::WritingTaskDraft {
                    writing_id: Some("w-605".to_string()),
                    title: "虚构季度工作总结".to_string(),
                    document_type: writing_store::TYPE_MONTHLY_SUMMARY.to_string(),
                    writing_purpose: "虚构汇报用途".to_string(),
                    audience: "虚构受众".to_string(),
                    writing_period: "2025 年第三季度".to_string(),
                    topic: "季度工作总结".to_string(),
                    requirements: "虚构写作要求".to_string(),
                    expected_length: "两页".to_string(),
                    writing_style: "简洁".to_string(),
                    ..Default::default()
                },
                NOW,
            )
            .unwrap();
            Self {
                base,
                root,
                vault,
                writing_id: task.writing_id,
            }
        }

        fn root(&self) -> &Path {
            &self.root
        }

        fn write_vault(&self, rel: &str, body: &str) {
            let path = self.vault.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, body).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn draft(
        title: &str,
        content: &str,
        source_type: &str,
        quote: &str,
        event_date: Option<&str>,
    ) -> material_store::MaterialDraft {
        material_store::MaterialDraft {
            title: title.to_string(),
            content: content.to_string(),
            material_type: material_store::TYPE_WORK_FACT.to_string(),
            source_type: source_type.to_string(),
            source_id: format!("src-{title}"),
            source_quote: quote.to_string(),
            source_location: Some("虚构位置：第 2 行".to_string()),
            event_date: event_date.map(str::to_string),
            ..Default::default()
        }
    }

    fn create(fx: &Fixture, draft: material_store::MaterialDraft) -> String {
        material_store::create_material(fx.root(), &draft, NOW)
            .unwrap()
            .material_id
    }

    fn confirm(fx: &Fixture, id: &str) {
        material_store::review_material(
            fx.root(),
            id,
            material_store::ACTION_CONFIRM,
            None,
            CONFIRM_AT,
        )
        .unwrap();
    }

    fn collect(fx: &Fixture, filter: MaterialFilter, hints: &[String], pending: bool) -> CollectedMaterials {
        collect_materials_from(
            fx.root(),
            Some(&fx.vault),
            &filter,
            &fx.writing_id,
            hints,
            pending,
        )
        .unwrap()
    }

    #[test]
    fn defaults_to_confirmed_only_and_never_surfaces_pending() {
        let fx = Fixture::new("default-confirmed");
        let confirmed_id = create(
            &fx,
            draft("已确认素材", "季度工作总结的已确认内容", material_store::SOURCE_WORK_RECORD, "季度工作总结", None),
        );
        confirm(&fx, &confirmed_id);
        let pending_id = create(
            &fx,
            draft("待审核素材", "季度工作总结的待审核内容", material_store::SOURCE_WORK_RECORD, "季度工作总结", None),
        );

        let response = collect(&fx, MaterialFilter::default(), &[], false);
        assert_eq!(response.material_count, 1);
        assert_eq!(response.pending_count, 0);
        assert!(!response
            .items
            .iter()
            .any(|item| item.material_id.as_deref() == Some(pending_id.as_str())));
        assert_eq!(
            response.items[0].material_id.as_deref(),
            Some(confirmed_id.as_str())
        );
        assert_eq!(response.items[0].class, MaterialClass::ConfirmedFact);
        assert!(response.items[0].suitable_for_direct_quote);
    }

    #[test]
    fn include_pending_marks_pending_review_and_disables_direct_quote() {
        let fx = Fixture::new("include-pending");
        let pending_id = create(
            &fx,
            draft("待审核事实", "季度工作总结的待审核内容", material_store::SOURCE_WORK_RECORD, "待审核引用", None),
        );
        let response = collect(&fx, MaterialFilter::default(), &[], true);
        assert_eq!(response.pending_count, 1);
        let item = response
            .items
            .iter()
            .find(|item| item.material_id.as_deref() == Some(pending_id.as_str()))
            .unwrap();
        assert_eq!(item.class, MaterialClass::PendingReview);
        assert!(item.pending_review);
        assert!(!item.suitable_for_direct_quote);
        assert_eq!(item.review_status, REVIEW_PENDING);
    }

    #[test]
    fn explicit_pending_filter_requires_include_pending() {
        let fx = Fixture::new("pending-requires-flag");
        let err = collect_materials_from(
            fx.root(),
            Some(&fx.vault),
            &MaterialFilter {
                review_status: Some(REVIEW_PENDING.to_string()),
                ..Default::default()
            },
            &fx.writing_id,
            &[],
            false,
        )
        .unwrap_err();
        assert!(err.contains("include_pending=true"), "{err}");
    }

    #[test]
    fn direct_quote_requires_content_different_from_source_quote() {
        let fx = Fixture::new("quote-equal");
        let same_id = create(
            &fx,
            draft("同文素材", "完全一致的引用", material_store::SOURCE_USER, "完全一致的引用", None),
        );
        confirm(&fx, &same_id);
        let other_id = create(
            &fx,
            draft("改写素材", "整理后的正文", material_store::SOURCE_USER, "原始引用", None),
        );
        confirm(&fx, &other_id);
        let response = collect(&fx, MaterialFilter::default(), &[], false);
        let same = response
            .items
            .iter()
            .find(|item| item.material_id.as_deref() == Some(same_id.as_str()))
            .unwrap();
        let other = response
            .items
            .iter()
            .find(|item| item.material_id.as_deref() == Some(other_id.as_str()))
            .unwrap();
        assert!(!same.suitable_for_direct_quote);
        assert!(other.suitable_for_direct_quote);
        assert_eq!(same.class, MaterialClass::UserExternal);
    }

    #[test]
    fn conflict_pure_function_flags_same_title_different_month_only() {
        let items = vec![
            ConflictMaterial { material_id: "m-1".into(), title: "季度营收数据".into(), event_date: Some("2025-03-10".into()) },
            ConflictMaterial { material_id: "m-2".into(), title: "季度营收数据（2025）".into(), event_date: Some("2025-04-10".into()) },
            ConflictMaterial { material_id: "m-3".into(), title: "季度营收数据".into(), event_date: Some("2025-03-20".into()) },
            ConflictMaterial { material_id: "m-4".into(), title: "人员结构数据".into(), event_date: Some("2025-05-10".into()) },
        ];
        let conflicts = detect_conflicts(&items);
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.iter().any(|pair| pair.left_id == "m-1" && pair.right_id == "m-2"));
        assert!(conflicts.iter().any(|pair| pair.left_id == "m-2" && pair.right_id == "m-3"));
        assert!(!conflicts.iter().any(|pair| pair.left_id == "m-1" && pair.right_id == "m-3"));
        assert!(!conflicts.iter().any(|pair| pair.left_id == "m-4"));
        for conflict in &conflicts {
            assert!(conflict.hint.contains("不自动合并"));
        }
    }

    #[test]
    fn collect_surfaces_conflicts_without_merging_items() {
        let fx = Fixture::new("collect-conflict");
        let first = create(
            &fx,
            draft("季度营收数据", "三月的虚构数据", material_store::SOURCE_WORK_RECORD, "三月数据", Some("2025-03-15")),
        );
        confirm(&fx, &first);
        let second = create(
            &fx,
            draft("季度营收数据", "四月的虚构数据", material_store::SOURCE_WORK_RECORD, "四月数据", Some("2025-04-15")),
        );
        confirm(&fx, &second);
        let response = collect(&fx, MaterialFilter::default(), &[], false);
        assert_eq!(response.material_count, 2, "冲突不合并，两条都保留");
        assert_eq!(response.conflicts.len(), 1);
        let first_item = response
            .items
            .iter()
            .find(|item| item.material_id.as_deref() == Some(first.as_str()))
            .unwrap();
        assert!(first_item.conflict_hint.as_deref().unwrap().contains("2025-03"));
        assert!(first_item.conflict_hint.as_deref().unwrap().contains("2025-04"));
        assert_eq!(first_item.conflict_ids, vec![second]);
    }

    #[test]
    fn project_filter_uses_project_overview_for_project_relation() {
        let fx = Fixture::new("project-overview");
        let project = project_store::create_project(
            fx.root(),
            &project_store::ProjectDraft {
                project_id: Some("p-605".to_string()),
                name: "虚构项目丙".to_string(),
                goal: "虚构目标".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let mut material = draft(
            "项目素材",
            "季度工作总结中的项目内容",
            material_store::SOURCE_PROJECT,
            "项目引用",
            None,
        );
        material.related_project_id = Some(project.project_id.clone());
        let id = create(&fx, material);
        confirm(&fx, &id);
        let response = collect(
            &fx,
            MaterialFilter {
                project_id: Some(project.project_id.clone()),
                ..Default::default()
            },
            &[],
            false,
        );
        assert_eq!(response.material_count, 1);
        assert!(response.items[0]
            .reasons
            .iter()
            .any(|reason| reason.basis == RecommendationBasis::ProjectRelated));
        assert_eq!(response.items[0].related.project.as_ref().unwrap().title.as_deref(), Some("虚构项目丙"));
    }

    #[test]
    fn knowledge_hints_add_ai_recommended_support_and_material_basis() {
        let fx = Fixture::new("knowledge-support");
        fx.write_vault("notes/q3.md", "---\ntitle: 虚构知识\n---\n\n季度工作总结知识条目。\n");
        let mut material = draft(
            "知识关联素材",
            "季度工作总结的素材正文",
            material_store::SOURCE_WORK_RECORD,
            "素材引用",
            None,
        );
        material.related_knowledge_ids = vec!["notes/q3.md".to_string()];
        let id = create(&fx, material);
        confirm(&fx, &id);
        let response = collect(&fx, MaterialFilter::default(), &["季度工作总结".to_string()], false);
        assert!(response.knowledge_count >= 1);
        assert!(response
            .items
            .iter()
            .any(|item| item.kind == "knowledge" && item.class == MaterialClass::AiRecommended));
        let material_item = response
            .items
            .iter()
            .find(|item| item.material_id.as_deref() == Some(id.as_str()))
            .unwrap();
        assert!(material_item
            .reasons
            .iter()
            .any(|reason| reason.basis == RecommendationBasis::KnowledgeSupport));
    }

    #[test]
    fn knowledge_source_material_is_reference_only_and_not_direct_quote_target() {
        let fx = Fixture::new("reference-only");
        let id = create(
            &fx,
            draft("知识来源素材", "知识条目整理的内容", material_store::SOURCE_KNOWLEDGE, "知识引用", None),
        );
        confirm(&fx, &id);
        let response = collect(&fx, MaterialFilter::default(), &[], false);
        assert_eq!(response.items[0].class, MaterialClass::ReferenceOnly);
        assert!(response.items[0].suitable_for_direct_quote);
        assert_eq!(response.items[0].class_label, "仅供参考知识");
    }

    #[test]
    fn selection_adopts_only_confirmed_and_appends_status_record() {
        let fx = Fixture::new("selection-adopt");
        let confirmed_id = create(
            &fx,
            draft("可采纳素材", "已确认正文", material_store::SOURCE_WORK_RECORD, "已确认引用", None),
        );
        confirm(&fx, &confirmed_id);
        let pending_id = create(
            &fx,
            draft("不可采纳素材", "待审核正文", material_store::SOURCE_WORK_RECORD, "待审核引用", None),
        );
        let before = writing_store::read_writing_task(fx.root(), &fx.writing_id).unwrap();

        let outcome = apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                adopt_material_ids: vec![confirmed_id.clone()],
                adopt_knowledge_ids: vec!["notes/q3.md".to_string()],
                highlight_material_ids: vec![confirmed_id.clone()],
                note: Some("虚构重点说明".to_string()),
                ..Default::default()
            },
            NOW + 10.0,
        )
        .unwrap();
        assert!(outcome.changed);
        assert_eq!(outcome.selected_material_ids, vec![confirmed_id.clone()]);
        assert_eq!(outcome.selected_knowledge_ids, vec!["notes/q3.md".to_string()]);
        assert_eq!(outcome.status_records.len(), before.status_records.len() + 1);
        let last = outcome.status_records.last().unwrap();
        assert_eq!(last.action, writing_store::ACTION_EDIT);
        assert!(last.reason.contains("标注重点"));
        assert!(last.reason.contains("虚构重点说明"));

        let err = apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                adopt_material_ids: vec![pending_id.clone()],
                ..Default::default()
            },
            NOW + 20.0,
        )
        .unwrap_err();
        assert!(err.contains("只有已确认素材"), "{err}");
        let after = writing_store::read_writing_task(fx.root(), &fx.writing_id).unwrap();
        assert_eq!(after.selected_material_ids, vec![confirmed_id], "拒绝后不得写入选集");
        assert!(!after.selected_material_ids.iter().any(|id| id == &pending_id));
    }

    #[test]
    fn selection_exclude_removes_ids_and_records_it() {
        let fx = Fixture::new("selection-exclude");
        let id = create(
            &fx,
            draft("可排除素材", "已确认正文", material_store::SOURCE_WORK_RECORD, "已确认引用", None),
        );
        confirm(&fx, &id);
        apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                adopt_material_ids: vec![id.clone()],
                ..Default::default()
            },
            NOW + 10.0,
        )
        .unwrap();
        let outcome = apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                exclude_material_ids: vec![id.clone()],
                ..Default::default()
            },
            NOW + 20.0,
        )
        .unwrap();
        assert!(outcome.selected_material_ids.is_empty());
        assert!(outcome.status_records.last().unwrap().reason.contains("排除素材"));
    }

    #[test]
    fn collect_is_read_only_for_material_files() {
        let fx = Fixture::new("read-only");
        let id = create(
            &fx,
            draft("只读素材", "季度工作总结正文", material_store::SOURCE_WORK_RECORD, "素材引用", None),
        );
        confirm(&fx, &id);
        let path = material_store::material_path(fx.root(), &id).unwrap();
        let before = fs::read(&path).unwrap();
        collect(&fx, MaterialFilter::default(), &["季度工作总结".to_string()], false);
        let after = fs::read(&path).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn payload_carries_source_time_status_relations_reason_conflict_and_class() {
        let fx = Fixture::new("payload-shape");
        let id = create(
            &fx,
            draft("载荷素材", "季度工作总结正文", material_store::SOURCE_WORK_RECORD, "素材引用", Some("2025-03-01")),
        );
        confirm(&fx, &id);
        let response = collect(&fx, MaterialFilter::default(), &[], false);
        let payload = serde_json::to_value(&response).unwrap();
        assert!(payload.get("items").is_some());
        assert!(payload.get("conflicts").is_some());
        let item = &payload["items"][0];
        for key in [
            "kind",
            "class",
            "class_label",
            "content_summary",
            "source",
            "event_date",
            "review_status",
            "related",
            "reasons",
            "reason_text",
            "conflict_ids",
            "conflict_hint",
            "suitable_for_direct_quote",
        ] {
            assert!(item.get(key).is_some(), "payload 缺少字段 {key}");
        }
        assert_eq!(item["class"], "confirmed_fact");
        for key in ["source_type", "source_id", "source_quote", "source_location"] {
            assert!(item["source"].get(key).is_some(), "source 缺少字段 {key}");
        }
        assert_eq!(item["related"]["tasks"].as_array().unwrap().len(), 0);
        assert!(item["related"].get("project").is_some());
        assert!(item["related"].get("knowledge").is_some());
    }

    #[test]
    fn selection_no_op_does_not_append_a_status_record() {
        let fx = Fixture::new("selection-no-op");
        let id = create(
            &fx,
            draft("已采纳素材", "已确认正文", material_store::SOURCE_WORK_RECORD, "已确认引用", None),
        );
        confirm(&fx, &id);
        apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                adopt_material_ids: vec![id.clone()],
                ..Default::default()
            },
            NOW + 10.0,
        )
        .unwrap();
        let before = writing_store::read_writing_task(fx.root(), &fx.writing_id).unwrap();
        let outcome = apply_selection(
            fx.root(),
            &SelectionRequest {
                writing_id: fx.writing_id.clone(),
                adopt_material_ids: vec![id],
                ..Default::default()
            },
            NOW + 20.0,
        )
        .unwrap();
        assert!(!outcome.changed);
        assert_eq!(outcome.status_records.len(), before.status_records.len());
    }
}
