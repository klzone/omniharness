//! 场景状态汇总（OD-ARCH-060 S1/S5）。
//!
//! 只做**聚合**：把宿主机已经落盘的待确认写入记录与成果条目按场景数一遍，
//! 再加上本机日期算出的本周/本月窗口。这里不调用模型、不读正文、不猜业务含义，
//! 因此界面上出现的每个数字都可以指回一条确定的文件或记录。

use crate::pending_writes::PendingWriteView;
use crate::task_shell::{utc_parts, OfflineArtifactEntry};

/// 一个场景的状态条数字。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneOverview {
    pub scenario: String,
    /// 等待用户决定的待确认写入条数。
    pub pending: u64,
    /// 其中已经过期、只能放弃的条数。
    pub pending_expired: u64,
    /// 待确认目录里无法解析的记录数（只计数，不猜内容）。
    pub pending_broken: u64,
    /// 本场景已保存成果总数。
    pub artifacts_total: u64,
    /// 本场景本周（周一 00:00 起，UTC）保存的成果数。
    pub artifacts_this_week: u64,
    /// 本场景本月保存的成果数。
    pub artifacts_this_month: u64,
    /// 本场景最近一次保存时间（Unix 秒）；没有成果时为 null。
    pub latest_artifact_at: Option<f64>,
    /// 本周周一与周五的日期（YYYY-MM-DD），供「本期·周五截止」这类文案使用。
    pub week_start: String,
    pub week_deadline: String,
}

/// 场景标识归一：宿主列举成果时给的是连字符形状（`weekly-report`），
/// 前端与保存请求用的是下划线形状（`weekly_report`）。
pub fn normalize_scenario(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

/// Unix 秒 → 当天 00:00（UTC）的「自 1970-01-01 起的天数」。
fn day_index(secs: f64) -> i64 {
    (secs.floor() as i64).div_euclid(86_400)
}

/// 本周周一的天数下标（周一为一周第一天）。
fn week_start_days(secs: f64) -> i64 {
    let days = day_index(secs);
    // 1970-01-01 是星期四；换算成「周一 = 0」后得到星期序号。
    let weekday = (days + 3).rem_euclid(7);
    days - weekday
}

fn date_text(days: i64) -> String {
    let (y, m, d, ..) = utc_parts((days * 86_400) as f64);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// 聚合一个场景的状态数字。
///
/// * `view` 是宿主已经结算过有效期的待确认记录视图；
/// * `artifacts` 是成果目录的元数据列表（目录行会被忽略）；
/// * `now` 由调用方显式传入，便于用固定时间做单测。
pub fn summarize(
    scenario: &str,
    view: &[PendingWriteView],
    artifacts: &[OfflineArtifactEntry],
    now: f64,
) -> SceneOverview {
    let want = normalize_scenario(scenario);
    let mut pending = 0u64;
    let mut pending_expired = 0u64;
    for item in view {
        if normalize_scenario(&item.scenario) != want {
            continue;
        }
        match item.status.as_str() {
            "pending" => pending += 1,
            "expired" => pending_expired += 1,
            _ => {}
        }
    }

    let week_start = week_start_days(now);
    let week_start_secs = (week_start * 86_400) as f64;
    let week_end_secs = ((week_start + 7) * 86_400) as f64;
    let (cur_y, cur_m, ..) = utc_parts(now);

    let mut artifacts_total = 0u64;
    let mut artifacts_this_week = 0u64;
    let mut artifacts_this_month = 0u64;
    let mut latest: Option<f64> = None;
    for entry in artifacts {
        if entry.is_dir || normalize_scenario(&entry.scenario) != want {
            continue;
        }
        artifacts_total += 1;
        let at = entry.modified_at;
        if at >= week_start_secs && at < week_end_secs {
            artifacts_this_week += 1;
        }
        let (y, m, ..) = utc_parts(at);
        if y == cur_y && m == cur_m {
            artifacts_this_month += 1;
        }
        if latest.map(|v| at > v).unwrap_or(true) {
            latest = Some(at);
        }
    }

    SceneOverview {
        scenario: want,
        pending,
        pending_expired,
        pending_broken: 0,
        artifacts_total,
        artifacts_this_week,
        artifacts_this_month,
        latest_artifact_at: latest,
        week_start: date_text(week_start),
        week_deadline: date_text(week_start + 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(relative: &str, scenario: &str, at: f64) -> OfflineArtifactEntry {
        OfflineArtifactEntry {
            relative_path: relative.to_string(),
            file_name: relative.rsplit('/').next().unwrap_or(relative).to_string(),
            bytes: 10,
            modified_at: at,
            scenario: scenario.to_string(),
            is_dir: false,
            depth: 0,
        }
    }

    fn view(id: &str, scenario: &str, status: &str) -> PendingWriteView {
        PendingWriteView {
            id: id.to_string(),
            action: "save_offline".to_string(),
            source: "scheduler".to_string(),
            scenario: scenario.to_string(),
            title: "t".to_string(),
            summary: "s".to_string(),
            impact: "i".to_string(),
            chars: 1,
            created_at: 0.0,
            expires_at: 1.0,
            status: status.to_string(),
        }
    }

    // 2026-10-21（周三）12:00 UTC
    const NOW: f64 = 1_792_584_000.0;

    #[test]
    fn counts_pending_per_scene_and_ignores_others() {
        let views = vec![
            view("a", "weekly_report", "pending"),
            view("b", "weekly-report", "expired"),
            view("c", "material_writing", "pending"),
            view("d", "weekly_report", "consumed"),
        ];
        let out = summarize("weekly_report", &views, &[], NOW);
        assert_eq!(out.pending, 1);
        assert_eq!(out.pending_expired, 1);
        assert_eq!(out.artifacts_total, 0);
    }

    #[test]
    fn week_and_month_windows_are_inclusive_bounded() {
        // 同一周内周一 00:00 与周日 23:59 都算本周；再往前一秒不算。
        let start = (week_start_days(NOW) * 86_400) as f64;
        let items = vec![
            artifact("a", "weekly-report", start),
            artifact("b", "weekly-report", start + 6.0 * 86_400.0 + 60.0),
            artifact("c", "weekly-report", start - 1.0),
            artifact("d", "material-writing", NOW),
        ];
        let out = summarize("weekly_report", &[], &items, NOW);
        assert_eq!(out.artifacts_total, 3);
        assert_eq!(out.artifacts_this_week, 2);
        assert_eq!(out.latest_artifact_at, Some(start + 6.0 * 86_400.0 + 60.0));
    }

    #[test]
    fn week_dates_point_at_monday_and_friday() {
        let out = summarize("weekly_report", &[], &[], NOW);
        // 2026-10-19 是周一，10-23 是周五。
        assert_eq!(out.week_start, "2026-10-19");
        assert_eq!(out.week_deadline, "2026-10-23");
    }

    #[test]
    fn directory_rows_are_not_counted() {
        let mut dir = artifact("2026", "", 0.0);
        dir.is_dir = true;
        let out = summarize("weekly_report", &[], &[dir], NOW);
        assert_eq!(out.artifacts_total, 0);
    }
}
