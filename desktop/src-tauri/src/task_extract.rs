//! 工作记录到任务与素材候选的宿主校验与提示词组装（OD-WR-304 / OD-KM-504）。
//!
//! 分工：
//!   * Python 只组装提示词（engine/omnidesk_engine/task_extract.py），不落盘；
//!   * 模型输出先由这里校验，任务候选交给 task_store 暂存，素材候选落
//!     `素材篮/.candidates/`；两者搭同一次分析调用，不新增外发通道；
//!   * 校验失败时把记录的 ai_analysis_status 置为 failed，正文逐字节不动；
//!   * 已解析过的记录拒绝重复解析。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::material_store;
use crate::task_store;
use crate::worklog_records::{
    self, assert_record_id, assert_work_root, is_safe_id, new_file_id, write_new_atomically,
    RecordPatch, WorkRecord, AI_ANALYZED, AI_FAILED,
};

pub const EXTRACT_SCHEMA: &str = "omnidesk.task-candidates.v1";
pub const MAX_RESULT_BYTES: usize = 1024 * 1024;
pub const MAX_CANDIDATES: usize = 20;
pub const MATERIAL_CANDIDATE_SCHEMA: &str = "omnidesk.material-candidates.v1";
/// 素材候选暂存目录：work/素材篮/.candidates/。
/// 素材篮整体已由 work_cleanup 排除；正式素材只读取顶层 .md，不会误读本目录。
pub const MATERIAL_CANDIDATES_DIR: &str = ".candidates";
pub const MATERIAL_CANDIDATE_PENDING: &str = "pending";
pub const MAX_MATERIAL_CANDIDATES: usize = 20;
const MAX_CONTEXT_TASKS: usize = 30;
const MAX_CONTEXT_MATERIALS: usize = 50;
const MAX_TITLE_CHARS: usize = 200;
const MAX_QUOTE_CHARS: usize = 500;
const MAX_CATEGORY_CHARS: usize = 32;
const MAX_NOTE_CHARS: usize = 500;
const MAX_UNCERTAINTY_CHARS: usize = 200;
const MAX_IMPORTANCE_REASON_CHARS: usize = 100;
const MAX_MATERIAL_CONTENT_CHARS: usize = 20_000;
const MAX_MATERIAL_QUOTE_CHARS: usize = 500;
const MAX_MATERIAL_HINTS: usize = 8;
const MAX_MATERIAL_HINT_CHARS: usize = 24;
const MAX_MATERIAL_MATCH_REASON_CHARS: usize = 200;

/// 解析准备结果：提示词与送模型前的原文（已过红线与掩码）。
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzePrompt {
    pub record_id: String,
    pub prompt: String,
    pub user_input_text: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

/// 解析落地结果：暂存了哪些候选、去重跳过了哪些键。
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeApplied {
    pub staged: Vec<task_store::TaskCandidate>,
    pub skipped_duplicates: Vec<String>,
    pub material_staged: Vec<MaterialCandidate>,
    pub material_skipped_duplicates: Vec<String>,
    pub material_duplicates: Vec<MaterialDuplicateLink>,
    pub record: WorkRecord,
}

/// 校验后的素材候选入参（尚未落盘）。
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialCandidateInput {
    pub material_type: String,
    pub content: String,
    pub source_quote: String,
    pub event_date: Option<String>,
    pub match_material_id: Option<String>,
    pub match_reason: Option<String>,
    pub knowledge_hints: Vec<String>,
    pub confidence: Option<f64>,
}

/// 暂存的素材候选。确认后才会由后续素材流生成正式 WritingMaterial。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialCandidate {
    pub schema: String,
    pub id: String,
    pub record_id: String,
    pub material_type: String,
    pub content: String,
    pub source_quote: String,
    pub source_location: Option<String>,
    pub event_date: Option<String>,
    pub match_material_id: Option<String>,
    pub match_reason: Option<String>,
    pub knowledge_hints: Vec<String>,
    pub confidence: Option<f64>,
    pub fingerprint: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialStageReport {
    pub staged: Vec<MaterialCandidate>,
    /// 因同记录同引用指纹已存在而跳过的指纹。
    pub skipped_duplicates: Vec<String>,
    /// 跳过的候选当前对应的既有素材或候选。
    pub duplicates: Vec<MaterialDuplicateLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialDuplicateLink {
    pub source_quote: String,
    pub fingerprint: String,
    pub existing_id: String,
    /// material = 正式素材；candidate = 尚未处理的候选。
    pub existing_kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialCandidateListing {
    pub items: Vec<MaterialCandidate>,
    pub broken: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPayload {
    candidates: Vec<RawCandidate>,
    #[serde(default)]
    material_candidates: Vec<RawMaterialCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCandidate {
    title: String,
    source_quote: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    uncertainty: Option<String>,
    #[serde(default)]
    match_task_id: Option<String>,
    #[serde(default)]
    match_confidence: Option<f64>,
    #[serde(default)]
    match_reason: Option<String>,
    #[serde(default)]
    suggested_state: Option<String>,
    #[serde(default)]
    importance: Option<String>,
    #[serde(default)]
    importance_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMaterialCandidate {
    material_type: String,
    content: String,
    source_quote: String,
    #[serde(default)]
    event_date: Option<String>,
    #[serde(default)]
    match_material_id: Option<String>,
    #[serde(default)]
    match_reason: Option<String>,
    #[serde(default)]
    knowledge_hints: Vec<String>,
    #[serde(default)]
    confidence: Option<f64>,
}

fn trim_opt(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// 已解析的记录不再重复发起解析；失败或未解析可以重试。
pub fn assert_analysis_allowed(record: &WorkRecord) -> Result<(), String> {
    if record.ai_analysis_status == AI_ANALYZED {
        return Err("这条记录已经解析过，不重复发起解析。".to_string());
    }
    Ok(())
}

/// 调用 Python 组装任务提取提示词。入参文本必须已经过红线与掩码。
pub fn build_prompt(
    python: &str,
    engine_dir: &Path,
    record: &WorkRecord,
    tasks: &[task_store::Task],
    title: &str,
    body: &str,
) -> Result<String, String> {
    build_prompt_with_materials(python, engine_dir, record, tasks, &[], title, body)
}

/// 与 `build_prompt` 相同，额外把既有素材 id+title 注入提示词（OD-KM-506）。
///
/// 只传编号、标题与类型，不传正文；模型据此判断候选是否与既有素材重复，
/// 最终仍由宿主按素材篮里的真实编号校验 match_material_id。
pub fn build_prompt_with_materials(
    python: &str,
    engine_dir: &Path,
    record: &WorkRecord,
    tasks: &[task_store::Task],
    materials: &[material_store::WritingMaterial],
    title: &str,
    body: &str,
) -> Result<String, String> {
    let task_rows: Vec<serde_json::Value> = tasks
        .iter()
        .take(MAX_CONTEXT_TASKS)
        .map(|task| {
            serde_json::json!({
                "id": task.id,
                "title": task.title,
                "state": task.state,
            })
        })
        .collect();
    let material_rows: Vec<serde_json::Value> = materials
        .iter()
        .take(MAX_CONTEXT_MATERIALS)
        .map(|material| {
            serde_json::json!({
                "id": material.material_id,
                "title": material.title,
                "material_type": material.material_type,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "schema": EXTRACT_SCHEMA,
        "record": {
            "id": record.id,
            "title": title,
            "record_date": record.record_date,
            "category": record.category,
            "tags": record.tags,
            "body": body,
        },
        "tasks": task_rows,
        "materials": material_rows,
    });
    let mut child = Command::new(python)
        .arg("-m")
        .arg("omnidesk_engine.task_extract")
        .current_dir(engine_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动任务提取提示词组装失败（{}）：{}", python, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|e| format!("写入任务提取提示词请求失败：{e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待任务提取提示词组装失败：{e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .lines()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!("任务提取提示词组装失败：{tail}"));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("任务提取提示词组装返回的不是 JSON：{e}"))?;
    let prompt = value
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "任务提取提示词组装结果缺少 prompt".to_string())?;
    Ok(prompt.to_string())
}

fn validate_candidate(
    raw: RawCandidate,
    record: &WorkRecord,
    tasks: &[task_store::Task],
    ordinal: usize,
) -> Result<task_store::CandidateInput, String> {
    let title = raw.title.trim().to_string();
    if title.is_empty() {
        return Err(format!("第 {ordinal} 条候选缺少标题。"));
    }
    if title.chars().count() > MAX_TITLE_CHARS || title.contains('\n') {
        return Err(format!("第 {ordinal} 条候选的标题过长或含换行。"));
    }
    let quote = raw.source_quote.trim().to_string();
    if quote.is_empty() {
        return Err(format!("第 {ordinal} 条候选缺少来源片段。"));
    }
    if quote.chars().count() > MAX_QUOTE_CHARS {
        return Err(format!("第 {ordinal} 条候选的来源片段过长。"));
    }
    if !record.body.contains(&quote) {
        return Err(format!(
            "第 {ordinal} 条候选的来源片段不是记录正文的连续原文，已拒绝。"
        ));
    }
    let category = trim_opt(raw.category);
    if let Some(value) = category.as_deref() {
        if value.chars().count() > MAX_CATEGORY_CHARS || value.contains('\n') {
            return Err(format!("第 {ordinal} 条候选的分类不合法。"));
        }
    }
    let due_date = trim_opt(raw.due_date);
    if let Some(date) = due_date.as_deref() {
        if !worklog_records::is_date_shape(date) {
            return Err(format!(
                "第 {ordinal} 条候选的截止日期形状不正确（应为 YYYY-MM-DD）。"
            ));
        }
        // D-065：模糊时间不得落精确日期。正文里没有这个字面日期就拒绝。
        if !record.body.contains(date) {
            return Err(format!(
                "第 {ordinal} 条候选的截止日期不在正文里，模糊时间不得落精确日期。"
            ));
        }
    }
    let note = trim_opt(raw.note);
    if let Some(value) = note.as_deref() {
        if value.chars().count() > MAX_NOTE_CHARS {
            return Err(format!("第 {ordinal} 条候选的备注过长。"));
        }
    }
    let uncertainty = trim_opt(raw.uncertainty);
    if let Some(value) = uncertainty.as_deref() {
        if value.chars().count() > MAX_UNCERTAINTY_CHARS {
            return Err(format!("第 {ordinal} 条候选的不确定说明过长。"));
        }
    }
    if let Some(confidence) = raw.confidence {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(format!("第 {ordinal} 条候选的置信度必须落在 0 到 1 之间。"));
        }
    }
    let match_task_id = trim_opt(raw.match_task_id);
    if let Some(task_id) = match_task_id.as_deref() {
        let matched = tasks.iter().find(|task| task.id == task_id).ok_or_else(|| {
            format!("第 {ordinal} 条候选建议的关联任务不存在或已完成。")
        })?;
        if !task_store::is_unfinished_state(&matched.state) {
            return Err(format!("第 {ordinal} 条候选建议的关联任务已完成。"));
        }
    }
    if raw.match_confidence.is_some() && match_task_id.is_none() {
        return Err(format!("第 {ordinal} 条候选给了关联置信度，却没有关联任务编号。"));
    }
    if let Some(value) = raw.match_confidence {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(format!("第 {ordinal} 条候选的关联置信度必须落在 0 到 1 之间。"));
        }
    }
    let match_reason = trim_opt(raw.match_reason);
    if let Some(reason) = match_reason.as_deref() {
        if reason.chars().count() > 200 {
            return Err(format!("第 {ordinal} 条候选的关联理由过长。"));
        }
    }
    let suggested_state = trim_opt(raw.suggested_state);
    if let Some(state) = suggested_state.as_deref() {
        if !task_store::is_task_state(state) {
            return Err(format!("第 {ordinal} 条候选建议的状态不认识。"));
        }
        let task_id = match_task_id.as_deref().ok_or_else(|| {
            format!("第 {ordinal} 条候选建议了状态，却没有关联任务编号。")
        })?;
        let matched = tasks.iter().find(|task| task.id == task_id).ok_or_else(|| {
            format!("第 {ordinal} 条候选建议的关联任务不存在或已完成。")
        })?;
        if !task_store::is_allowed_transition(&matched.state, state) {
            return Err(format!(
                "第 {ordinal} 条候选建议的状态迁移不合法：{} → {}。",
                matched.state, state
            ));
        }
    }
    let importance = trim_opt(raw.importance);
    let importance_reason = trim_opt(raw.importance_reason);
    match importance.as_deref() {
        Some(level) => {
            if !matches!(level, "高" | "中" | "低") {
                return Err(format!(
                    "第 {ordinal} 条候选的重要性只能是 高 / 中 / 低 或 null。"
                ));
            }
            let reason = importance_reason.as_deref().ok_or_else(|| {
                format!("第 {ordinal} 条候选给了重要性，却没有一句话依据。")
            })?;
            if reason.chars().count() > MAX_IMPORTANCE_REASON_CHARS || reason.contains('\n') {
                return Err(format!(
                    "第 {ordinal} 条候选的重要性依据过长或含换行（最多 {MAX_IMPORTANCE_REASON_CHARS} 字）。"
                ));
            }
        }
        None => {
            if importance_reason.is_some() {
                return Err(format!(
                    "第 {ordinal} 条候选没有重要性判断，却给了依据。"
                ));
            }
        }
    }
    Ok(task_store::CandidateInput {
        title,
        source_quote: quote,
        category,
        due_date,
        note,
        confidence: raw.confidence,
        uncertainty,
        match_task_id,
        match_confidence: raw.match_confidence,
        match_reason,
        suggested_state,
        importance,
        importance_reason,
    })
}

fn validate_material_candidate(
    raw: RawMaterialCandidate,
    record: &WorkRecord,
    materials: &[material_store::WritingMaterial],
    ordinal: usize,
) -> Result<MaterialCandidateInput, String> {
    let material_type = raw.material_type.trim().to_string();
    if !material_store::MATERIAL_TYPES.contains(&material_type.as_str()) {
        return Err(format!(
            "第 {ordinal} 条素材候选的类型不在十类白名单内。"
        ));
    }
    let content = raw.content.trim().to_string();
    if content.is_empty() {
        return Err(format!("第 {ordinal} 条素材候选缺少正文。"));
    }
    if content.chars().count() > MAX_MATERIAL_CONTENT_CHARS {
        return Err(format!("第 {ordinal} 条素材候选正文过长。"));
    }
    let quote = raw.source_quote.trim().to_string();
    if quote.is_empty() {
        return Err(format!("第 {ordinal} 条素材候选缺少来源片段。"));
    }
    if quote.chars().count() > MAX_MATERIAL_QUOTE_CHARS {
        return Err(format!("第 {ordinal} 条素材候选的来源片段过长。"));
    }
    if !record.body.contains(&quote) {
        return Err(format!(
            "第 {ordinal} 条素材候选的来源片段不是记录正文的连续原文，已拒绝。"
        ));
    }
    // 防复制冒充提炼：折叠空白或去掉全部空白后仍与原文相同即拒绝。
    if content_repeats_quote(&content, &quote) {
        return Err(format!(
            "第 {ordinal} 条素材候选的 content 与 source_quote 相同，未形成提炼。"
        ));
    }
    let event_date = trim_opt(raw.event_date);
    if let Some(date) = event_date.as_deref() {
        if !worklog_records::is_date_shape(date) {
            return Err(format!(
                "第 {ordinal} 条素材候选的事件日期形状不正确（应为 YYYY-MM-DD）。"
            ));
        }
        if !record.body.contains(date) {
            return Err(format!(
                "第 {ordinal} 条素材候选的事件日期不在正文里，不得凭空生成日期。"
            ));
        }
    }
    let match_material_id = trim_opt(raw.match_material_id);
    let match_reason = trim_opt(raw.match_reason);
    if let Some(material_id) = match_material_id.as_deref() {
        if !materials.iter().any(|material| material.material_id == material_id) {
            return Err(format!(
                "第 {ordinal} 条素材候选建议关联的素材不存在。"
            ));
        }
        let reason = match_reason.as_deref().ok_or_else(|| {
            format!("第 {ordinal} 条素材候选给了关联素材编号，却没有关联理由。")
        })?;
        if reason.chars().count() > MAX_MATERIAL_MATCH_REASON_CHARS || reason.contains('\n') {
            return Err(format!(
                "第 {ordinal} 条素材候选的关联理由过长或含换行（最多 {MAX_MATERIAL_MATCH_REASON_CHARS} 字）。"
            ));
        }
    } else if match_reason.is_some() {
        return Err(format!(
            "第 {ordinal} 条素材候选没有关联素材编号，却给了关联理由。"
        ));
    }
    let mut hints: Vec<String> = Vec::new();
    for hint in raw.knowledge_hints {
        let trimmed = hint.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_MATERIAL_HINT_CHARS
            || trimmed.chars().any(char::is_control)
        {
            return Err(format!(
                "第 {ordinal} 条素材候选的主题关键词过长或含控制字符（最多 {MAX_MATERIAL_HINT_CHARS} 字）。"
            ));
        }
        if !hints.iter().any(|existing| existing == &trimmed) {
            hints.push(trimmed);
        }
    }
    if hints.len() > MAX_MATERIAL_HINTS {
        return Err(format!(
            "第 {ordinal} 条素材候选的主题关键词过多（最多 {MAX_MATERIAL_HINTS} 个）。"
        ));
    }
    if let Some(confidence) = raw.confidence {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(format!(
                "第 {ordinal} 条素材候选的置信度必须落在 0 到 1 之间。"
            ));
        }
    }
    Ok(MaterialCandidateInput {
        material_type,
        content,
        source_quote: quote,
        event_date,
        match_material_id,
        match_reason,
        knowledge_hints: hints,
        confidence: raw.confidence,
    })
}

fn parse_payload(result_text: &str) -> Result<RawPayload, String> {
    let text = result_text.trim();
    if text.is_empty() {
        return Err("任务提取结果为空。".to_string());
    }
    if text.len() > MAX_RESULT_BYTES {
        return Err(format!(
            "任务提取结果 {} 字节，超过 {} 字节上限。",
            text.len(),
            MAX_RESULT_BYTES
        ));
    }
    let value = crate::knowledge_ingest::parse_result_text(text)
        .map_err(|e| format!("任务提取结果不是合法 JSON：{e}"))?;
    let payload: RawPayload = serde_json::from_value(value)
        .map_err(|e| format!("任务提取结果不符合约定结构：{e}"))?;
    Ok(payload)
}

fn content_repeats_quote(content: &str, quote: &str) -> bool {
    let normalized_content = material_store::normalize_quote(content);
    let normalized_quote = material_store::normalize_quote(quote);
    if normalized_content == normalized_quote {
        return true;
    }
    let compact = |value: &str| -> String {
        value.chars().filter(|ch| !ch.is_whitespace()).collect()
    };
    compact(&normalized_content) == compact(&normalized_quote)
}

fn validate_task_candidates(
    payload: RawPayload,
    record: &WorkRecord,
    tasks: &[task_store::Task],
) -> Result<Vec<task_store::CandidateInput>, String> {
    if payload.candidates.len() > MAX_CANDIDATES {
        return Err(format!(
            "候选过多：{} 条，超过 {} 条上限。",
            payload.candidates.len(),
            MAX_CANDIDATES
        ));
    }
    let mut out = Vec::new();
    for (index, raw) in payload.candidates.into_iter().enumerate() {
        out.push(validate_candidate(raw, record, tasks, index + 1)?);
    }
    Ok(out)
}

fn validate_material_candidates(
    payload: RawPayload,
    record: &WorkRecord,
    materials: &[material_store::WritingMaterial],
) -> Result<Vec<MaterialCandidateInput>, String> {
    if payload.material_candidates.len() > MAX_MATERIAL_CANDIDATES {
        return Err(format!(
            "素材候选过多：{} 条，超过 {} 条上限。",
            payload.material_candidates.len(),
            MAX_MATERIAL_CANDIDATES
        ));
    }
    let mut out = Vec::new();
    for (index, raw) in payload.material_candidates.into_iter().enumerate() {
        out.push(validate_material_candidate(raw, record, materials, index + 1)?);
    }
    Ok(out)
}

/// 校验模型输出：结构、枚举、原文片段与日期都必须对得上记录正文。
pub fn validate_task_payload(
    result_text: &str,
    record: &WorkRecord,
    tasks: &[task_store::Task],
) -> Result<Vec<task_store::CandidateInput>, String> {
    let payload = parse_payload(result_text)?;
    validate_task_candidates(payload, record, tasks)
}

/// 校验模型输出中的素材候选：类型白名单、原文连续性、content ≠ quote、
/// 日期真实性、关联素材存在性与主题关键词形状。
pub fn validate_material_payload(
    result_text: &str,
    record: &WorkRecord,
    materials: &[material_store::WritingMaterial],
) -> Result<Vec<MaterialCandidateInput>, String> {
    let payload = parse_payload(result_text)?;
    validate_material_candidates(payload, record, materials)
}

fn validate_analysis_payload(
    result_text: &str,
    record: &WorkRecord,
    tasks: &[task_store::Task],
    materials: &[material_store::WritingMaterial],
) -> Result<(Vec<task_store::CandidateInput>, Vec<MaterialCandidateInput>), String> {
    let payload = parse_payload(result_text)?;
    // 先校验任务，再校验素材；两者共用一次解析，错误时一律不落任何候选。
    let task_inputs = {
        let payload = RawPayload {
            candidates: payload.candidates,
            material_candidates: Vec::new(),
        };
        validate_task_candidates(payload, record, tasks)?
    };
    let material_inputs = {
        let payload = RawPayload {
            candidates: Vec::new(),
            material_candidates: payload.material_candidates,
        };
        validate_material_candidates(payload, record, materials)?
    };
    Ok((task_inputs, material_inputs))
}

pub fn material_candidates_dir(root: &Path) -> std::path::PathBuf {
    material_store::material_dir(root).join(MATERIAL_CANDIDATES_DIR)
}

pub fn material_candidate_path(root: &Path, id: &str) -> Result<std::path::PathBuf, String> {
    if !is_safe_id(id) {
        return Err(format!("素材候选编号不合法：{id:?}"));
    }
    Ok(material_candidates_dir(root).join(format!("{id}.json")))
}

/// 来源片段在记录正文中的起始位置；正文逐字未被改写时才有值。
fn quote_location(body: &str, quote: &str) -> Option<String> {
    let index = body.find(quote)?;
    let line = body[..index].matches('\n').count() + 1;
    let line_start = body[..index].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
    let column = body[line_start..index].chars().count() + 1;
    Some(format!("记录正文第 {line} 行第 {column} 列"))
}

fn material_effective_quote(material: &material_store::WritingMaterial) -> &str {
    if material.source_quote.trim().is_empty() {
        &material.content
    } else {
        &material.source_quote
    }
}

fn existing_material_fingerprints(root: &Path) -> Result<HashMap<String, String>, String> {
    let listing = material_store::list_materials(root, &material_store::MaterialFilter::default())?;
    let mut out = HashMap::new();
    for material in listing.items {
        let fingerprint = material_store::material_fingerprint(
            &material.source_id,
            material_effective_quote(&material),
        );
        out.entry(fingerprint).or_insert(material.material_id);
    }
    Ok(out)
}

fn pending_material_candidate_fingerprints(
    root: &Path,
) -> Result<HashMap<String, String>, String> {
    let dir = material_candidates_dir(root);
    let mut out = HashMap::new();
    if !dir.exists() {
        return Ok(out);
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取素材候选目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        if let Ok(candidate) = serde_json::from_str::<MaterialCandidate>(&raw) {
            if candidate.status == MATERIAL_CANDIDATE_PENDING {
                out.entry(candidate.fingerprint).or_insert(candidate.id);
            }
        }
    }
    Ok(out)
}

fn assert_material_candidate_input(
    root: &Path,
    input: &MaterialCandidateInput,
    record: &WorkRecord,
    ordinal: usize,
) -> Result<(), String> {
    if !material_store::MATERIAL_TYPES.contains(&input.material_type.as_str()) {
        return Err(format!("第 {ordinal} 条素材候选的类型不在十类白名单内。"));
    }
    if input.content.trim().is_empty() {
        return Err(format!("第 {ordinal} 条素材候选缺少正文。"));
    }
    if input.content.chars().count() > MAX_MATERIAL_CONTENT_CHARS {
        return Err(format!("第 {ordinal} 条素材候选正文过长。"));
    }
    let quote = input.source_quote.trim();
    if quote.is_empty() {
        return Err(format!("第 {ordinal} 条素材候选缺少来源片段。"));
    }
    if quote.chars().count() > MAX_MATERIAL_QUOTE_CHARS {
        return Err(format!("第 {ordinal} 条素材候选的来源片段过长。"));
    }
    if !record.body.contains(quote) {
        return Err(format!("第 {ordinal} 条素材候选的来源片段不在记录正文里。"));
    }
    if content_repeats_quote(&input.content, quote) {
        return Err(format!(
            "第 {ordinal} 条素材候选的 content 与 source_quote 相同，未形成提炼。"
        ));
    }
    if let Some(date) = input.event_date.as_deref() {
        if !worklog_records::is_date_shape(date) || !record.body.contains(date) {
            return Err(format!(
                "第 {ordinal} 条素材候选的事件日期形状不正确或不在正文里。"
            ));
        }
    }
    if let Some(material_id) = input.match_material_id.as_deref() {
        if material_store::read_material(root, material_id).is_err() {
            return Err(format!("第 {ordinal} 条素材候选建议关联的素材不存在。"));
        }
        let reason = input.match_reason.as_deref().ok_or_else(|| {
            format!("第 {ordinal} 条素材候选给了关联素材编号，却没有关联理由。")
        })?;
        if reason.chars().count() > MAX_MATERIAL_MATCH_REASON_CHARS || reason.contains('\n') {
            return Err(format!(
                "第 {ordinal} 条素材候选的关联理由过长或含换行（最多 {MAX_MATERIAL_MATCH_REASON_CHARS} 字）。"
            ));
        }
    } else if input.match_reason.is_some() {
        return Err(format!(
            "第 {ordinal} 条素材候选没有关联素材编号，却给了关联理由。"
        ));
    }
    if input.knowledge_hints.len() > MAX_MATERIAL_HINTS {
        return Err(format!(
            "第 {ordinal} 条素材候选的主题关键词过多（最多 {MAX_MATERIAL_HINTS} 个）。"
        ));
    }
    for hint in &input.knowledge_hints {
        let trimmed = hint.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_MATERIAL_HINT_CHARS
            || trimmed.chars().any(char::is_control)
        {
            return Err(format!(
                "第 {ordinal} 条素材候选的主题关键词过长或含控制字符（最多 {MAX_MATERIAL_HINT_CHARS} 字）。"
            ));
        }
    }
    if let Some(confidence) = input.confidence {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(format!(
                "第 {ordinal} 条素材候选的置信度必须落在 0 到 1 之间。"
            ));
        }
    }
    Ok(())
}

/// 暂存素材候选：落「素材篮/.candidates/」，不生成正式素材、不改审核状态。
/// 同记录同引用指纹已存在于正式素材或待处理候选时跳过并回报关联对象。
pub fn stage_material_candidates(
    root: &Path,
    record_id: &str,
    inputs: &[MaterialCandidateInput],
    now: f64,
) -> Result<MaterialStageReport, String> {
    if inputs.is_empty() {
        return Ok(MaterialStageReport::default());
    }
    assert_work_root(root)?;
    assert_record_id(record_id)?;
    let record = worklog_records::read_record(root, record_id)?;
    let dir = material_candidates_dir(root);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建素材候选目录失败（{}）：{}", dir.display(), e))?;
    let materials = existing_material_fingerprints(root)?;
    let mut pending = pending_material_candidate_fingerprints(root)?;
    let mut report = MaterialStageReport::default();
    for (index, input) in inputs.iter().enumerate() {
        assert_material_candidate_input(root, input, &record, index + 1)?;
        let fingerprint = material_store::material_fingerprint(record_id, &input.source_quote);
        let duplicate = materials
            .get(&fingerprint)
            .map(|id| (id.clone(), "material"))
            .or_else(|| pending.get(&fingerprint).map(|id| (id.clone(), "candidate")));
        if let Some((existing_id, existing_kind)) = duplicate {
            report.skipped_duplicates.push(fingerprint.clone());
            report.duplicates.push(MaterialDuplicateLink {
                source_quote: input.source_quote.clone(),
                fingerprint,
                existing_id,
                existing_kind: existing_kind.to_string(),
            });
            continue;
        }
        let id = new_file_id("mc", now);
        let candidate = MaterialCandidate {
            schema: MATERIAL_CANDIDATE_SCHEMA.to_string(),
            id: id.clone(),
            record_id: record_id.to_string(),
            material_type: input.material_type.clone(),
            content: input.content.clone(),
            source_quote: input.source_quote.clone(),
            source_location: quote_location(&record.body, &input.source_quote),
            event_date: input.event_date.clone(),
            match_material_id: input.match_material_id.clone(),
            match_reason: input.match_reason.clone(),
            knowledge_hints: input.knowledge_hints.clone(),
            confidence: input.confidence,
            fingerprint: fingerprint.clone(),
            status: MATERIAL_CANDIDATE_PENDING.to_string(),
            created_at: crate::task_shell::utc_iso8601(now),
        };
        let bytes = serde_json::to_string_pretty(&candidate)
            .map_err(|e| format!("序列化素材候选失败：{e}"))?;
        write_new_atomically(&material_candidate_path(root, &id)?, bytes.as_bytes())?;
        pending.insert(fingerprint, id);
        report.staged.push(candidate);
    }
    Ok(report)
}

pub fn read_material_candidate(root: &Path, id: &str) -> Result<MaterialCandidate, String> {
    assert_work_root(root)?;
    let path = material_candidate_path(root, id)?;
    let raw = fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("素材候选不存在或已处理：{id}")
        } else {
            format!("读取素材候选失败（{}）：{}", path.display(), e)
        }
    })?;
    serde_json::from_str(&raw).map_err(|e| format!("素材候选不是约定 JSON：{e}"))
}

pub fn list_material_candidates(root: &Path) -> Result<MaterialCandidateListing, String> {
    assert_work_root(root)?;
    let dir = material_candidates_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if dir.exists() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取素材候选目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取素材候选目录项失败：{e}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path)
                .map_err(|e| format!("读取素材候选失败：{e}"))
                .and_then(|raw| {
                    serde_json::from_str::<MaterialCandidate>(&raw)
                        .map_err(|e| format!("素材候选不是约定 JSON：{e}"))
                })
            {
                Ok(candidate) => {
                    if candidate.status == MATERIAL_CANDIDATE_PENDING {
                        items.push(candidate);
                    }
                }
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(MaterialCandidateListing { items, broken })
}

/// 模型调用失败或超时时由界面调用：只把状态置为 failed，正文不动。
pub fn mark_analysis_failed(
    root: &Path,
    record_id: &str,
    now: f64,
) -> Result<WorkRecord, String> {
    let record = worklog_records::read_record(root, record_id)?;
    if record.ai_analysis_status == AI_ANALYZED {
        return Err("这条记录已经解析过，不能回退为失败。".to_string());
    }
    worklog_records::update_record(
        root,
        record_id,
        &RecordPatch {
            ai_analysis_status: Some(AI_FAILED.to_string()),
            ..Default::default()
        },
        now,
    )
}

fn mark_failed(root: &Path, record_id: &str, now: f64) {
    let _ = mark_analysis_failed(root, record_id, now);
}

/// 解析落地：校验 → 暂存候选 → 记录标记为已解析；任何失败只改状态。
pub fn apply_analysis(
    root: &Path,
    record_id: &str,
    result_text: &str,
    now: f64,
) -> Result<AnalyzeApplied, String> {
    let record = worklog_records::read_record(root, record_id)?;
    assert_analysis_allowed(&record)?;
    let tasks = task_store::unfinished_tasks(root)?;
    let materials = material_store::list_materials(root, &material_store::MaterialFilter::default())?
        .items;
    let (task_inputs, material_inputs) =
        match validate_analysis_payload(result_text, &record, &tasks, &materials) {
        Ok(validated) => validated,
        Err(message) => {
            mark_failed(root, record_id, now);
            return Err(format!(
                "{message} 已标记为解析失败，记录正文未改动。"
            ));
        }
    };
    let staged = match task_store::stage_candidates(root, record_id, &task_inputs, now) {
        Ok(report) => report,
        Err(message) => {
            mark_failed(root, record_id, now);
            return Err(format!("暂存候选失败：{message}。已标记为解析失败，记录正文未改动。"));
        }
    };
    let material_report = match stage_material_candidates(root, record_id, &material_inputs, now) {
        Ok(report) => report,
        Err(message) => {
            mark_failed(root, record_id, now);
            return Err(format!(
                "暂存素材候选失败：{message}。已标记为解析失败，记录正文未改动。"
            ));
        }
    };
    let updated = worklog_records::update_record(
        root,
        record_id,
        &RecordPatch {
            ai_analysis_status: Some(AI_ANALYZED.to_string()),
            ..Default::default()
        },
        now,
    )
    .map_err(|e| format!("候选已暂存，但记录状态写入失败：{e}"))?;
    Ok(AnalyzeApplied {
        staged: staged.staged,
        skipped_duplicates: staged.skipped_duplicates,
        material_staged: material_report.staged,
        material_skipped_duplicates: material_report.skipped_duplicates,
        material_duplicates: material_report.duplicates,
        record: updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklog_records::{create_record, read_record, RecordDraft};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_100_000.0;
    const OFFSET: i32 = 480;
    const BODY: &str = "今天整理周报。2026-11-30 前交总结。会议纪要发给项目组。";

    struct TempRoot {
        path: std::path::PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wr304-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn seed(temp: &TempRoot) -> WorkRecord {
        create_record(
            temp.root(),
            &RecordDraft {
                title: Some("工作记录".to_string()),
                body: BODY.to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap()
    }

    fn seed_task(temp: &TempRoot) -> task_store::Task {
        task_store::create_task(
            temp.root(),
            &task_store::TaskDraft {
                title: "推进周报".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn payload(quote: &str, due: Option<&str>) -> String {
        serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": quote,
                "category": null,
                "due_date": due,
                "note": null,
                "confidence": 0.8,
                "uncertainty": null,
            }]
        })
        .to_string()
    }

    fn material_candidate(
        material_type: &str,
        content: &str,
        quote: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "material_type": material_type,
            "content": content,
            "source_quote": quote,
            "event_date": null,
            "match_material_id": null,
            "match_reason": null,
            "knowledge_hints": ["周报", "复盘"],
            "confidence": 0.7,
        })
    }

    fn material_payload(material_type: &str, content: &str, quote: &str) -> String {
        serde_json::json!({
            "candidates": [],
            "material_candidates": [material_candidate(material_type, content, quote)],
        })
        .to_string()
    }

    fn seed_material(
        temp: &TempRoot,
        record: &WorkRecord,
        quote: &str,
    ) -> material_store::WritingMaterial {
        material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "已有素材".to_string(),
                content: "已有素材正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: record.id.clone(),
                source_quote: quote.to_string(),
                confidence: Some(0.5),
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    #[test]
    fn validate_accepts_fenced_payload() {
        let temp = TempRoot::new("valid");
        let record = seed(&temp);
        let fence = "\u{60}\u{60}\u{60}";
        let text = format!(
            "{fence}json\n{}\n{fence}",
            payload("今天整理周报", Some("2026-11-30"))
        );
        let candidates = validate_task_payload(&text, &record, &[]).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_quote, "今天整理周报");
        assert_eq!(candidates[0].due_date.as_deref(), Some("2026-11-30"));
    }

    #[test]
    fn validate_rejects_quote_not_in_body() {
        let temp = TempRoot::new("quote");
        let record = seed(&temp);
        let err =
            validate_task_payload(&payload("记录里没有这句话", None), &record, &[]).unwrap_err();
        assert!(err.contains("连续原文"), "{err}");
    }

    #[test]
    fn validate_rejects_due_date_not_explicit_in_body() {
        let temp = TempRoot::new("due");
        let record = seed(&temp);
        let err =
            validate_task_payload(&payload("今天整理周报", Some("2026-11-29")), &record, &[])
                .unwrap_err();
        assert!(err.contains("模糊时间"), "{err}");
    }

    #[test]
    fn validate_rejects_bad_due_date_shape() {
        let temp = TempRoot::new("due-shape");
        let record = seed(&temp);
        let err =
            validate_task_payload(&payload("今天整理周报", Some("11月30日")), &record, &[])
                .unwrap_err();
        assert!(err.contains("形状不正确"), "{err}");
    }

    #[test]
    fn validate_rejects_bad_confidence() {
        let temp = TempRoot::new("confidence");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "confidence": 1.5,
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("置信度"), "{err}");
    }

    #[test]
    fn validate_rejects_unknown_fields() {
        let temp = TempRoot::new("unknown");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "extra": 1,
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("约定结构"), "{err}");
    }

    #[test]
    fn validate_rejects_empty_title() {
        let temp = TempRoot::new("title");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "   ",
                "source_quote": "今天整理周报",
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("缺少标题"), "{err}");
    }

    #[test]
    fn validate_rejects_too_many_candidates() {
        let temp = TempRoot::new("many");
        let record = seed(&temp);
        let mut candidates = Vec::new();
        for _ in 0..=MAX_CANDIDATES {
            candidates.push(serde_json::json!({
                "title": "整理周报",
                "source_quote": "今天整理周报",
            }));
        }
        let text = serde_json::json!({ "candidates": candidates }).to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("候选过多"), "{err}");
    }

    #[test]
    fn analysis_gate_allows_unanalyzed_and_failed() {
        let temp = TempRoot::new("gate");
        let mut record = seed(&temp);
        assert!(assert_analysis_allowed(&record).is_ok());
        record.ai_analysis_status = AI_FAILED.to_string();
        assert!(assert_analysis_allowed(&record).is_ok());
        record.ai_analysis_status = AI_ANALYZED.to_string();
        assert!(assert_analysis_allowed(&record).is_err());
    }

    #[test]
    fn apply_stages_candidates_and_marks_analyzed() {
        let temp = TempRoot::new("apply");
        let record = seed(&temp);
        let report =
            apply_analysis(temp.root(), &record.id, &payload("今天整理周报", None), NOW).unwrap();
        assert_eq!(report.staged.len(), 1);
        assert_eq!(report.record.ai_analysis_status, AI_ANALYZED);
        let candidates = task_store::list_candidates(temp.root()).unwrap();
        assert_eq!(candidates.items.len(), 1);
    }

    #[test]
    fn apply_marks_failed_without_touching_body() {
        let temp = TempRoot::new("failed");
        let record = seed(&temp);
        let err = apply_analysis(
            temp.root(),
            &record.id,
            &payload("这句不在正文里", None),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("解析失败"), "{err}");
        let after = read_record(temp.root(), &record.id).unwrap();
        assert_eq!(after.ai_analysis_status, AI_FAILED);
        assert_eq!(after.body, BODY);
        let candidates = task_store::list_candidates(temp.root()).unwrap();
        assert!(candidates.items.is_empty());
    }

    #[test]
    fn apply_rejects_second_run_after_analyzed() {
        let temp = TempRoot::new("repeat");
        let record = seed(&temp);
        apply_analysis(temp.root(), &record.id, &payload("今天整理周报", None), NOW).unwrap();
        let err =
            apply_analysis(temp.root(), &record.id, &payload("今天整理周报", None), NOW + 60.0)
                .unwrap_err();
        assert!(err.contains("已经解析过"), "{err}");
    }

    #[test]
    fn apply_allows_retry_after_failed() {
        let temp = TempRoot::new("retry");
        let record = seed(&temp);
        let err = apply_analysis(
            temp.root(),
            &record.id,
            &payload("这句不在正文里", None),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("解析失败"), "{err}");
        let report =
            apply_analysis(temp.root(), &record.id, &payload("今天整理周报", None), NOW + 60.0)
                .unwrap();
        assert_eq!(report.record.ai_analysis_status, AI_ANALYZED);
        assert_eq!(report.staged.len(), 1);
    }

    #[test]
    fn model_failure_marks_failed_only() {
        let temp = TempRoot::new("model-failed");
        let record = seed(&temp);
        let after = mark_analysis_failed(temp.root(), &record.id, NOW).unwrap();
        assert_eq!(after.ai_analysis_status, AI_FAILED);
        assert_eq!(after.body, BODY);
        assert_eq!(after.title, record.title);
    }

    #[test]
    fn empty_candidates_marks_analyzed() {
        let temp = TempRoot::new("empty");
        let record = seed(&temp);
        let report = apply_analysis(
            temp.root(),
            &record.id,
            "{\"candidates\": []}",
            NOW,
        )
        .unwrap();
        assert!(report.staged.is_empty());
        assert_eq!(report.record.ai_analysis_status, AI_ANALYZED);
    }

    #[test]
    fn validate_accepts_match_and_legal_state_suggestion() {
        let temp = TempRoot::new("match");
        let record = seed(&temp);
        let task = seed_task(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "推进周报",
                "source_quote": "今天整理周报",
                "confidence": 0.8,
                "match_task_id": task.id,
                "match_confidence": 0.7,
                "match_reason": "同一件事",
                "suggested_state": "进行中",
            }]
        })
        .to_string();
        let candidates = validate_task_payload(&text, &record, &[task.clone()]).unwrap();
        assert_eq!(candidates[0].match_task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(candidates[0].suggested_state.as_deref(), Some("进行中"));
    }

    #[test]
    fn validate_rejects_unknown_match_task() {
        let temp = TempRoot::new("match-unknown");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "推进周报",
                "source_quote": "今天整理周报",
                "match_task_id": "t-missing",
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("不存在或已完成"), "{err}");
    }

    #[test]
    fn validate_rejects_illegal_state_suggestion() {
        let temp = TempRoot::new("state-suggest");
        let record = seed(&temp);
        let task = seed_task(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "推进周报",
                "source_quote": "今天整理周报",
                "match_task_id": task.id,
                "suggested_state": "已完成",
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[task]).unwrap_err();
        assert!(err.contains("迁移不合法"), "{err}");
    }

    #[test]
    fn validate_rejects_match_confidence_without_task() {
        let temp = TempRoot::new("match-confidence");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "推进周报",
                "source_quote": "今天整理周报",
                "match_confidence": 0.6,
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("没有关联任务编号"), "{err}");
    }

    #[test]
    fn apply_stages_match_suggestion() {
        let temp = TempRoot::new("apply-match");
        let record = seed(&temp);
        let task = seed_task(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "推进周报",
                "source_quote": "今天整理周报",
                "match_task_id": task.id,
                "match_confidence": 0.9,
                "match_reason": "同一件事",
            }]
        })
        .to_string();
        let report = apply_analysis(temp.root(), &record.id, &text, NOW).unwrap();
        assert_eq!(report.staged.len(), 1);
        assert_eq!(report.staged[0].match_task_id.as_deref(), Some(task.id.as_str()));
        let stored = task_store::read_task(temp.root(), &task.id).unwrap();
        assert!(stored.progress.is_empty(), "阶段化候选不得自动改任务");
    }

    #[test]
    fn validate_accepts_importance_with_reason() {
        let temp = TempRoot::new("importance");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "importance": "高",
                "importance_reason": "本周评审要用",
            }]
        })
        .to_string();
        let candidates = validate_task_payload(&text, &record, &[]).unwrap();
        assert_eq!(candidates[0].importance.as_deref(), Some("高"));
        assert_eq!(candidates[0].importance_reason.as_deref(), Some("本周评审要用"));
    }

    #[test]
    fn validate_rejects_unknown_importance_level() {
        let temp = TempRoot::new("importance-level");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "importance": "待评估",
                "importance_reason": "不确定",
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("只能是 高 / 中 / 低"), "{err}");
    }

    #[test]
    fn validate_rejects_importance_without_reason_or_too_long() {
        let temp = TempRoot::new("importance-reason");
        let record = seed(&temp);
        let mut value = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "importance": "中",
            }]
        });
        let text = value.to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("没有一句话依据"), "{err}");
        value["candidates"][0]["importance_reason"] =
            serde_json::json!("这句依据写得过长".repeat(20));
        let err = validate_task_payload(&value.to_string(), &record, &[]).unwrap_err();
        assert!(err.contains("过长"), "{err}");
    }

    #[test]
    fn validate_rejects_reason_without_importance() {
        let temp = TempRoot::new("importance-orphan");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "importance": null,
                "importance_reason": "多余依据",
            }]
        })
        .to_string();
        let err = validate_task_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("没有重要性判断"), "{err}");
    }

    #[test]
    fn confirm_candidate_writes_ai_importance_source() {
        let temp = TempRoot::new("importance-confirm");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
                "importance": "高",
                "importance_reason": "本周评审要用",
            }]
        })
        .to_string();
        let report = apply_analysis(temp.root(), &record.id, &text, NOW).unwrap();
        let candidate = &report.staged[0];
        let confirmed = task_store::confirm_candidate(
            temp.root(),
            &candidate.id,
            &task_store::ConfirmEdits::default(),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(confirmed.importance, task_store::IMPORTANCE_HIGH);
        assert_eq!(
            confirmed.importance_source.as_deref(),
            Some(task_store::IMPORTANCE_SOURCE_AI)
        );
        assert_eq!(confirmed.importance_reason.as_deref(), Some("本周评审要用"));
    }

    #[test]
    fn validate_material_payload_accepts_candidate_and_hints() {
        let temp = TempRoot::new("mc-valid");
        let record = seed(&temp);
        let text = material_payload("工作事实", "周报整理已完成并归档", "今天整理周报");
        let candidates = validate_material_payload(&text, &record, &[]).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].material_type, "工作事实");
        assert_eq!(candidates[0].source_quote, "今天整理周报");
        assert_eq!(
            candidates[0].knowledge_hints,
            vec!["周报".to_string(), "复盘".to_string()]
        );
        assert_eq!(candidates[0].confidence, Some(0.7));
    }

    #[test]
    fn validate_material_payload_rejects_non_whitelist_type() {
        let temp = TempRoot::new("mc-type");
        let record = seed(&temp);
        let text = material_payload("新闻稿", "周报整理已完成并归档", "今天整理周报");
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("十类白名单"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_empty_or_missing_quote() {
        let temp = TempRoot::new("mc-quote-empty");
        let record = seed(&temp);
        let text = material_payload("工作事实", "周报整理已完成并归档", "   ");
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("缺少来源片段"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_quote_not_in_body() {
        let temp = TempRoot::new("mc-quote-body");
        let record = seed(&temp);
        let text = material_payload("工作事实", "周报整理已完成并归档", "记录里没有这句话");
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("连续原文"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_content_equal_to_quote() {
        let temp = TempRoot::new("mc-copy");
        let record = seed(&temp);
        let text = material_payload("工作事实", "今天整理周报", "今天整理周报");
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("content 与 source_quote 相同"), "{err}");
        let text = material_payload("工作事实", "今天 整理 周报", "今天整理周报");
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("content 与 source_quote 相同"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_bad_or_unstated_event_date() {
        let temp = TempRoot::new("mc-date");
        let record = seed(&temp);
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["event_date"] = serde_json::json!("11月30日");
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("形状不正确"), "{err}");
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["event_date"] = serde_json::json!("2026-11-29");
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("不在正文里"), "{err}");
    }

    #[test]
    fn validate_material_payload_accepts_existing_match_with_reason() {
        let temp = TempRoot::new("mc-match");
        let record = seed(&temp);
        let existing = seed_material(&temp, &record, "今天整理周报");
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "会议纪要发给项目组");
        item["match_material_id"] = serde_json::json!(existing.material_id);
        item["match_reason"] = serde_json::json!("同一周工作事项的后续进展");
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let candidates =
            validate_material_payload(&text, &record, std::slice::from_ref(&existing)).unwrap();
        assert_eq!(
            candidates[0].match_material_id.as_deref(),
            Some(existing.material_id.as_str())
        );
    }

    #[test]
    fn validate_material_payload_rejects_unknown_match_and_orphan_reason() {
        let temp = TempRoot::new("mc-match-bad");
        let record = seed(&temp);
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["match_material_id"] = serde_json::json!("m-missing");
        item["match_reason"] = serde_json::json!("同一事项");
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("素材不存在"), "{err}");
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["match_reason"] = serde_json::json!("多余理由");
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("没有关联素材编号"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_bad_hints() {
        let temp = TempRoot::new("mc-hints");
        let record = seed(&temp);
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["knowledge_hints"] = serde_json::json!([
            "一", "二", "三", "四", "五", "六", "七", "八", "九"
        ]);
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("主题关键词过多"), "{err}");
        let mut item = material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报");
        item["knowledge_hints"] =
            serde_json::json!(["这个主题关键词写得太长了已经超过二十四个汉字的限制"]);
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": [item] }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("主题关键词过长"), "{err}");
    }

    #[test]
    fn validate_material_payload_rejects_too_many_candidates() {
        let temp = TempRoot::new("mc-many");
        let record = seed(&temp);
        let mut items = Vec::new();
        for _ in 0..=MAX_MATERIAL_CANDIDATES {
            items.push(material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报"));
        }
        let text =
            serde_json::json!({ "candidates": [], "material_candidates": items }).to_string();
        let err = validate_material_payload(&text, &record, &[]).unwrap_err();
        assert!(err.contains("素材候选过多"), "{err}");
    }

    #[test]
    fn apply_stages_material_candidate_under_basket_staging() {
        let temp = TempRoot::new("mc-apply");
        let record = seed(&temp);
        let report = apply_analysis(
            temp.root(),
            &record.id,
            &material_payload("工作事实", "周报整理已完成并归档", "今天整理周报"),
            NOW,
        )
        .unwrap();
        assert_eq!(report.material_staged.len(), 1);
        assert_eq!(report.material_staged[0].status, MATERIAL_CANDIDATE_PENDING);
        assert_eq!(
            report.material_staged[0].source_location.as_deref(),
            Some("记录正文第 1 行第 1 列")
        );
        assert!(report.staged.is_empty());
        let staging = material_candidates_dir(temp.root());
        assert!(staging.exists());
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 1);
        let basket = material_store::material_dir(temp.root());
        for entry in fs::read_dir(&basket).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(
                path.extension().and_then(|ext| ext.to_str()),
                None,
                "正式素材篮顶层不应出现候选 md：{}",
                path.display()
            );
        }
    }

    #[test]
    fn apply_stages_task_and_material_candidates_in_one_run() {
        let temp = TempRoot::new("mc-mixed");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
            }],
            "material_candidates": [
                material_candidate("工作事实", "周报整理已完成并归档", "今天整理周报")
            ],
        })
        .to_string();
        let report = apply_analysis(temp.root(), &record.id, &text, NOW).unwrap();
        assert_eq!(report.staged.len(), 1);
        assert_eq!(report.material_staged.len(), 1);
        assert_eq!(task_store::list_candidates(temp.root()).unwrap().items.len(), 1);
        assert_eq!(
            list_material_candidates(temp.root()).unwrap().items.len(),
            1
        );
        assert_eq!(report.record.ai_analysis_status, AI_ANALYZED);
    }

    #[test]
    fn material_staging_skips_duplicate_pending_candidate() {
        let temp = TempRoot::new("mc-dup-pending");
        let record = seed(&temp);
        let inputs = vec![MaterialCandidateInput {
            material_type: "工作事实".to_string(),
            content: "周报整理已完成并归档".to_string(),
            source_quote: "今天整理周报".to_string(),
            event_date: None,
            match_material_id: None,
            match_reason: None,
            knowledge_hints: vec!["周报".to_string()],
            confidence: Some(0.7),
        }];
        let first = stage_material_candidates(temp.root(), &record.id, &inputs, NOW).unwrap();
        assert_eq!(first.staged.len(), 1);
        let second =
            stage_material_candidates(temp.root(), &record.id, &inputs, NOW + 60.0).unwrap();
        assert!(second.staged.is_empty());
        assert_eq!(second.skipped_duplicates.len(), 1);
        assert_eq!(second.duplicates[0].existing_kind, "candidate");
        assert_eq!(second.duplicates[0].existing_id, first.staged[0].id);
        assert_eq!(
            fs::read_dir(material_candidates_dir(temp.root())).unwrap().count(),
            1
        );
    }

    #[test]
    fn material_staging_links_existing_material_by_fingerprint() {
        let temp = TempRoot::new("mc-dup-material");
        let record = seed(&temp);
        let existing = seed_material(&temp, &record, "今天整理周报");
        let report = apply_analysis(
            temp.root(),
            &record.id,
            &material_payload("工作事实", "周报整理已完成并归档", "今天整理周报"),
            NOW,
        )
        .unwrap();
        assert!(report.material_staged.is_empty());
        assert_eq!(report.material_skipped_duplicates.len(), 1);
        assert_eq!(report.material_duplicates[0].existing_kind, "material");
        assert_eq!(report.material_duplicates[0].existing_id, existing.material_id);
        assert_eq!(
            material_store::list_materials(temp.root(), &Default::default())
                .unwrap()
                .items
                .len(),
            1
        );
        assert!(list_material_candidates(temp.root()).unwrap().items.is_empty());
    }

    #[test]
    fn invalid_material_candidate_fails_before_task_staging() {
        let temp = TempRoot::new("mc-invalid-atomic");
        let record = seed(&temp);
        let text = serde_json::json!({
            "candidates": [{
                "title": "整理周报",
                "source_quote": "今天整理周报",
            }],
            "material_candidates": [
                material_candidate("新闻稿", "周报整理已完成并归档", "今天整理周报")
            ],
        })
        .to_string();
        let err = apply_analysis(temp.root(), &record.id, &text, NOW).unwrap_err();
        assert!(err.contains("十类白名单"), "{err}");
        assert!(err.contains("解析失败"), "{err}");
        let after = read_record(temp.root(), &record.id).unwrap();
        assert_eq!(after.ai_analysis_status, AI_FAILED);
        assert_eq!(after.body, BODY);
        assert!(task_store::list_candidates(temp.root()).unwrap().items.is_empty());
        assert!(!material_candidates_dir(temp.root()).exists());
    }

    #[test]
    fn list_material_candidates_hides_broken_json_and_keeps_pending() {
        let temp = TempRoot::new("mc-list");
        let record = seed(&temp);
        let input = MaterialCandidateInput {
            material_type: "工作事实".to_string(),
            content: "周报整理已完成并归档".to_string(),
            source_quote: "今天整理周报".to_string(),
            event_date: None,
            match_material_id: None,
            match_reason: None,
            knowledge_hints: vec![],
            confidence: None,
        };
        stage_material_candidates(temp.root(), &record.id, &[input], NOW).unwrap();
        let dir = material_candidates_dir(temp.root());
        fs::write(dir.join("broken.json"), "{not json").unwrap();
        let listing = list_material_candidates(temp.root()).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.broken, 1);
        let read = read_material_candidate(temp.root(), &listing.items[0].id).unwrap();
        assert_eq!(read.fingerprint, listing.items[0].fingerprint);
    }
}
