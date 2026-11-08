//! 工作记录到任务候选的宿主校验与提示词组装（OD-WR-304）。
//!
//! 分工：
//!   * Python 只组装提示词（engine/omnidesk_engine/task_extract.py），不落盘；
//!   * 模型输出先由这里校验，再交给 task_store 暂存；
//!   * 校验失败时把记录的 ai_analysis_status 置为 failed，正文逐字节不动；
//!   * 已解析过的记录拒绝重复解析。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::task_store;
use crate::worklog_records::{
    self, RecordPatch, WorkRecord, AI_ANALYZED, AI_FAILED,
};

pub const EXTRACT_SCHEMA: &str = "omnidesk.task-candidates.v1";
pub const MAX_RESULT_BYTES: usize = 1024 * 1024;
pub const MAX_CANDIDATES: usize = 20;
const MAX_CONTEXT_TASKS: usize = 30;
const MAX_TITLE_CHARS: usize = 200;
const MAX_QUOTE_CHARS: usize = 500;
const MAX_CATEGORY_CHARS: usize = 32;
const MAX_NOTE_CHARS: usize = 500;
const MAX_UNCERTAINTY_CHARS: usize = 200;
const MAX_IMPORTANCE_REASON_CHARS: usize = 100;

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
    pub record: WorkRecord,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPayload {
    candidates: Vec<RawCandidate>,
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

/// 校验模型输出：结构、枚举、原文片段与日期都必须对得上记录正文。
pub fn validate_task_payload(
    result_text: &str,
    record: &WorkRecord,
    tasks: &[task_store::Task],
) -> Result<Vec<task_store::CandidateInput>, String> {
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
    let candidates = match validate_task_payload(result_text, &record, &tasks) {
        Ok(candidates) => candidates,
        Err(message) => {
            mark_failed(root, record_id, now);
            return Err(format!(
                "{message} 已标记为解析失败，记录正文未改动。"
            ));
        }
    };
    let staged = match task_store::stage_candidates(root, record_id, &candidates, now) {
        Ok(report) => report,
        Err(message) => {
            mark_failed(root, record_id, now);
            return Err(format!("暂存候选失败：{message}。已标记为解析失败，记录正文未改动。"));
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
}
