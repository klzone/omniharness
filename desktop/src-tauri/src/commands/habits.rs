//! 习惯整理命令（OD-GROW-301 G1）。
//!
//! 命令层只负责取应用数据根、读取当前规则策略并调用 habit_cards 存储层；
//! 不生成模型提示词，不触发引擎任务。

use tauri::State;

use crate::consolidation::read_digest_record;
use crate::habit_audit::{
    audit_active_quotes, record_quote_audits, review_active_cards, CuratorReport, QuoteAuditReport,
    QuoteSample,
};
use crate::habit_cards::{
    HabitActiveList, HabitCandidateDecision, HabitCandidateDecisionResult, HabitCandidateList,
    HabitCardUpdateResult, HabitOpenQuestionList, HabitOpenQuestionUpdateResult,
    HabitPendingReviewDecisionResult, HabitPendingReviewList, HabitStore,
};
use crate::profile_rules::RulePolicy;
use crate::task_shell::now_secs;
use crate::SharedHost;

fn store(state: &State<'_, SharedHost>) -> Result<HabitStore, String> {
    let paths = {
        let host = state
            .lock()
            .map_err(|_| "宿主状态锁不可用。".to_string())?;
        host.paths.clone()
    };
    HabitStore::resolve(&paths)
}

fn policy(state: &State<'_, SharedHost>) -> Result<RulePolicy, String> {
    crate::current_rule_policy(state)
}

#[tauri::command(async)]
pub(crate) fn habit_candidate_list(
    state: State<'_, SharedHost>,
) -> Result<HabitCandidateList, String> {
    store(&state)?.list_candidates(now_secs())
}

#[tauri::command(async)]
pub(crate) fn habit_candidate_decide(
    state: State<'_, SharedHost>,
    decisions: Vec<HabitCandidateDecision>,
) -> Result<HabitCandidateDecisionResult, String> {
    let policy = policy(&state)?;
    store(&state)?.decide_candidates(&decisions, &policy, now_secs())
}

#[tauri::command(async)]
pub(crate) fn habit_active_list(
    state: State<'_, SharedHost>,
) -> Result<HabitActiveList, String> {
    store(&state)?.list_active()
}

#[tauri::command(async)]
pub(crate) fn habit_card_update(
    state: State<'_, SharedHost>,
    id: String,
    action: String,
) -> Result<HabitCardUpdateResult, String> {
    let policy = policy(&state)?;
    store(&state)?.update_card(&id, &action, &policy, now_secs())
}

#[tauri::command(async)]
pub(crate) fn habit_open_question_list(
    state: State<'_, SharedHost>,
) -> Result<HabitOpenQuestionList, String> {
    store(&state)?.list_open_questions()
}

#[tauri::command(async)]
pub(crate) fn habit_open_question_set_status(
    state: State<'_, SharedHost>,
    id: String,
    status: String,
) -> Result<HabitOpenQuestionUpdateResult, String> {
    store(&state)?.set_open_question_status(&id, &status)
}

#[tauri::command(async)]
pub(crate) fn habit_pending_review_list(
    state: State<'_, SharedHost>,
) -> Result<HabitPendingReviewList, String> {
    store(&state)?.list_pending_review()
}

#[tauri::command(async)]
pub(crate) fn habit_pending_review_decide(
    state: State<'_, SharedHost>,
    id: String,
    action: String,
) -> Result<HabitPendingReviewDecisionResult, String> {
    store(&state)?.decide_pending_review(&id, &action, now_secs())
}

/// 记录本轮注入生效卡的编号（G6 统计落盘）；只追加 inject 行，不存正文。
#[tauri::command(async)]
pub(crate) fn habit_stat_inject(
    state: State<'_, SharedHost>,
    ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    let store = store(&state)?;
    let now = now_secs();
    let mut recorded = 0usize;
    for id in ids.iter().map(|id| id.trim()).filter(|id| !id.is_empty()) {
        store.append_stat(crate::habit_audit::INJECT_STAT_TYPE, Some(id), None, now)?;
        recorded += 1;
    }
    Ok(serde_json::json!({ "recorded": recorded }))
}

/// 确定性复核生效卡（G12）：只返回可复核/冲突标记，不改卡、不删卡。
#[tauri::command(async)]
pub(crate) fn habit_curator_review(
    state: State<'_, SharedHost>,
) -> Result<CuratorReport, String> {
    let store = store(&state)?;
    let cards = store.list_active()?.items;
    let stats = store.read_stats().items;
    Ok(review_active_cards(&cards, &stats, now_secs()))
}

/// 抽查生效卡在当日整理样本里的引用依据（H7），结论写入 quote_audit 统计。
#[tauri::command(async)]
pub(crate) fn habit_quote_audit(
    state: State<'_, SharedHost>,
    date: String,
) -> Result<QuoteAuditReport, String> {
    let store = store(&state)?;
    let record = read_digest_record(store.root(), &date)?;
    let cards = store.list_active()?.items;
    let stats = store.read_stats().items;
    let samples: Vec<QuoteSample> = record
        .threads
        .iter()
        .flat_map(|thread| {
            thread
                .messages
                .iter()
                .filter(|message| !message.text.trim().is_empty())
                .map(|message| QuoteSample {
                    context_id: thread.thread_id.clone(),
                    body: message.text.clone(),
                })
        })
        .collect();
    let report = audit_active_quotes(&cards, &stats, &samples);
    record_quote_audits(&store, &report, now_secs())?;
    Ok(report)
}
