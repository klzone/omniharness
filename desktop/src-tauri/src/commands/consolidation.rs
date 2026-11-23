//! 每日整理命令（OD-GROW-301 G2）。
//!
//! 命令层只负责取应用数据根、当前规则策略与工作记录根，再调用 consolidation；
//! 不触发模型、不提交引擎任务。

use tauri::State;

use crate::app_paths::AppPaths;
use crate::consolidation::{
    apply_digest, collect_digest, load_config, read_state_readonly, record_collect_failure,
    tick_due, write_config, ConsolidationConfig, DigestPaths, DigestRecord, DigestStats,
};
use crate::profile_rules::RulePolicy;
use crate::task_shell::now_secs;
use crate::SharedHost;

fn host_paths(state: &State<'_, SharedHost>) -> Result<AppPaths, String> {
    let host = state
        .lock()
        .map_err(|_| "宿主状态锁不可用。".to_string())?;
    Ok(host.paths.clone())
}

fn policy(state: &State<'_, SharedHost>) -> Result<RulePolicy, String> {
    crate::current_rule_policy(state)
}

fn digest_paths(paths: &AppPaths) -> Result<DigestPaths, String> {
    let work_root = crate::settings::effective_working_root(&paths.config)?.root;
    Ok(DigestPaths {
        threads_root: paths.threads.clone(),
        work_root,
        habits_root: paths.root.join("habits"),
    })
}

#[tauri::command(async)]
pub(crate) fn consolidation_config_get(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let paths = host_paths(&state)?;
    let config = load_config(&paths.config)?;
    Ok(serde_json::json!({ "config": config }))
}

#[tauri::command(async)]
pub(crate) fn consolidation_config_set(
    state: State<'_, SharedHost>,
    config: ConsolidationConfig,
) -> Result<serde_json::Value, String> {
    let paths = host_paths(&state)?;
    write_config(&paths.config, &config)?;
    let saved = load_config(&paths.config)?;
    Ok(serde_json::json!({ "config": saved }))
}

#[tauri::command(async)]
pub(crate) fn consolidation_tick(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let paths = host_paths(&state)?;
    let config = load_config(&paths.config)?;
    let state = read_state_readonly(&paths.root.join("habits"))?;
    let now = now_secs();
    Ok(serde_json::json!({
        "now": now,
        "due": tick_due(&config, &state, now),
    }))
}

#[tauri::command(async)]
pub(crate) fn consolidation_preview(
    state: State<'_, SharedHost>,
) -> Result<DigestStats, String> {
    let paths = host_paths(&state)?;
    let config = load_config(&paths.config)?;
    let policy = policy(&state)?;
    let digest_paths = digest_paths(&paths)?;
    let now = now_secs();
    let plan = collect_digest(&digest_paths, &config, &policy, now)?;
    Ok(plan.stats)
}

#[tauri::command(async)]
pub(crate) fn consolidation_collect(
    state: State<'_, SharedHost>,
) -> Result<DigestStats, String> {
    let paths = host_paths(&state)?;
    let now = now_secs();
    let result = (|| -> Result<DigestRecord, String> {
        let config = load_config(&paths.config)?;
        let policy = policy(&state)?;
        let digest_paths = digest_paths(&paths)?;
        let plan = collect_digest(&digest_paths, &config, &policy, now)?;
        apply_digest(&digest_paths, &plan, now)
    })();
    if let Err(error) = &result {
        if let Ok(digest_paths) = digest_paths(&paths) {
            let _ = record_collect_failure(&digest_paths, now, error);
        }
    }
    result.map(|record| record.stats)
}
