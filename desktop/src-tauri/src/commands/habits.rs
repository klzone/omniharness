//! 习惯整理命令（OD-GROW-301 G1）。
//!
//! 命令层只负责取应用数据根、读取当前规则策略并调用 habit_cards 存储层；
//! 不生成模型提示词，不触发引擎任务。

use tauri::State;

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
