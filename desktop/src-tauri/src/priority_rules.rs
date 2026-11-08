//! 紧急性规则引擎（OD-QS-402）。
//!
//! 纯函数、无 IO：输入 Task 与当前时间，输出紧急性等级与依据文本。
//! 规则口径（D-075）：
//!   * 逾期 → 高，并注明逾期天数；
//!   * hard due 按工作日节奏分档：2 个工作日内为高、5 个工作日内为中，其余为低；
//!   * target due 在同日期下降一档（目标时间不是硬截止）；
//!   * due 为空时只认封闭时间表达（明天/后天/下周X/本月底/下月底/X月X日前），
//!     推定结果标注「推定」；封闭集之外一律待评估。
//! 时间基准：日期字符串是本地日历日；now 通过显式 utc_offset_minutes 换算，
//! 未给偏移时按 UTC（compute_urgency 保持任务卡的两参签名）。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use serde::Serialize;

use crate::task_store::{
    list_tasks, Task, TaskFilter, DUE_KIND_TARGET, IMPORTANCE_HIGH, IMPORTANCE_PENDING,
    QUADRANT_Q1, QUADRANT_Q2, QUADRANT_Q3, QUADRANT_Q4, STATE_CANCELLED, STATE_DONE,
    STATE_PAUSED,
};
use crate::worklog::{civil_from_days, days_from_civil};

pub const URGENCY_HIGH: &str = "高";
pub const URGENCY_MEDIUM: &str = "中";
pub const URGENCY_LOW: &str = "低";
pub const URGENCY_PENDING: &str = "待评估";

pub const QUADRANT_PENDING: &str = "待评估";

/// 2 个工作日内必须给结果；5 个工作日内进入本周收尾节奏。
/// 阈值按工作日计数，避免固定 24/48 小时在周末与跨时区时失真。
pub const HIGH_BUSINESS_DAYS: i64 = 2;
pub const MEDIUM_BUSINESS_DAYS: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrgencyAssessment {
    pub level: String,
    pub reason: String,
    /// 实际参与判断的截止日期（可能来自推定）。
    pub due_date: Option<String>,
    /// 是否由 hint 的时间表达推定而来。
    pub inferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredDue {
    pub date: String,
    pub label: String,
}

fn parse_date(date: &str) -> Option<(i64, u32, u32)> {
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }
    let year: i64 = date[0..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..10].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let (y, m, d) = civil_from_days(days);
    if (y, m, d) != (year, month, day) {
        return None;
    }
    Some((year, month, day))
}

fn date_string(year: i64, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

fn day_number(date: &str) -> Option<i64> {
    parse_date(date).map(|(y, m, d)| days_from_civil(y, m, d))
}

fn weekday(date: &str) -> Option<u32> {
    day_number(date).map(|days| (days + 3).rem_euclid(7) as u32 + 1)
}

fn add_days(date: &str, delta: i64) -> Option<String> {
    let days = day_number(date)? + delta;
    let (y, m, d) = civil_from_days(days);
    Some(date_string(y, m, d))
}

fn last_day_of_month(year: i64, month: u32) -> String {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let (y, m, d) = civil_from_days(days_from_civil(next_year, next_month, 1) - 1);
    date_string(y, m, d)
}

fn workdays_between(from: &str, to: &str) -> i64 {
    let start = match day_number(from) {
        Some(value) => value,
        None => return 0,
    };
    let end = match day_number(to) {
        Some(value) => value,
        None => return 0,
    };
    if end <= start {
        return 0;
    }
    let mut count = 0;
    for day in (start + 1)..=end {
        let weekday = (day + 3).rem_euclid(7);
        if weekday < 5 {
            count += 1;
        }
    }
    count
}

fn weekday_from_text(text: &str) -> Option<u32> {
    match text {
        "一" => Some(1),
        "二" => Some(2),
        "三" => Some(3),
        "四" => Some(4),
        "五" => Some(5),
        "六" => Some(6),
        "日" | "天" => Some(7),
        "1" => Some(1),
        "2" => Some(2),
        "3" => Some(3),
        "4" => Some(4),
        "5" => Some(5),
        "6" => Some(6),
        "7" => Some(7),
        _ => None,
    }
}

fn parse_month_day(text: &str) -> Option<(u32, u32)> {
    let month_index = text.find('月')?;
    let day_index = text.find('日')?;
    if day_index <= month_index {
        return None;
    }
    let month_text = text[..month_index].trim();
    let day_text = text[month_index + '月'.len_utf8()..day_index].trim();
    if month_text.is_empty() || day_text.is_empty() {
        return None;
    }
    let month: u32 = month_text.parse().ok()?;
    let day: u32 = day_text.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((month, day))
}

/// 封闭时间表达集。模式外返回 None（调用方按待评估处理）。
pub fn infer_due_from_hint(hint: &str, base_date: &str) -> Option<InferredDue> {
    let text = hint.trim();
    let (base_year, _, _) = parse_date(base_date)?;
    if text.contains("后天") {
        return Some(InferredDue {
            date: add_days(base_date, 2)?,
            label: "后天".to_string(),
        });
    }
    if text.contains("明天") {
        return Some(InferredDue {
            date: add_days(base_date, 1)?,
            label: "明天".to_string(),
        });
    }
    if let Some(index) = text.find("下周") {
        let rest = &text[index + "下周".len()..];
        let target = weekday_from_text(rest.chars().next()?.to_string().as_str())?;
        let base_weekday = weekday(base_date)?;
        let next_monday = day_number(base_date)? - (base_weekday as i64 - 1) + 7;
        let target_day = next_monday + target as i64 - 1;
        let (y, m, d) = civil_from_days(target_day);
        return Some(InferredDue {
            date: date_string(y, m, d),
            label: format!("下周{target}"),
        });
    }
    if text.contains("本月底") {
        let (year, month, _) = parse_date(base_date)?;
        return Some(InferredDue {
            date: last_day_of_month(year, month),
            label: "本月底".to_string(),
        });
    }
    if text.contains("下月底") {
        let (year, month, _) = parse_date(base_date)?;
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        return Some(InferredDue {
            date: last_day_of_month(next_year, next_month),
            label: "下月底".to_string(),
        });
    }
    if let Some((month, day)) = parse_month_day(text) {
        let mut year = base_year;
        let mut date = date_string(year, month, day);
        if parse_date(&date).is_none() {
            return None;
        }
        if day_number(&date)? < day_number(base_date)? {
            year += 1;
            date = date_string(year, month, day);
            if parse_date(&date).is_none() {
                return None;
            }
        }
        return Some(InferredDue {
            date,
            label: format!("{month}月{day}日前"),
        });
    }
    None
}

fn downgrade(level: &str) -> &'static str {
    match level {
        URGENCY_HIGH => URGENCY_MEDIUM,
        URGENCY_MEDIUM => URGENCY_LOW,
        _ => URGENCY_LOW,
    }
}

fn level_for_workdays(workdays: i64) -> &'static str {
    if workdays <= HIGH_BUSINESS_DAYS {
        URGENCY_HIGH
    } else if workdays <= MEDIUM_BUSINESS_DAYS {
        URGENCY_MEDIUM
    } else {
        URGENCY_LOW
    }
}

/// 任务卡签名：compute_urgency(task, now)。按 UTC 日历日换算「今天」。
pub fn compute_urgency(task: &Task, now: f64) -> UrgencyAssessment {
    compute_urgency_at(task, now, 0)
}

/// 带本地偏移的版本：日期判断按本地日历日，不静默改用模型服务器时区。
pub fn compute_urgency_at(task: &Task, now: f64, utc_offset_minutes: i32) -> UrgencyAssessment {
    let secs = now.floor() as i64 + utc_offset_minutes as i64 * 60;
    let today_days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(today_days);
    let today = date_string(year, month, day);

    let (due, inferred_label) = match task.due_date.as_deref() {
        Some(date) if parse_date(date).is_some() => (Some(date.to_string()), None),
        _ => {
            let base = task
                .created_at
                .get(0..10)
                .filter(|value| parse_date(value).is_some())
                .unwrap_or(today.as_str());
            match task.hint.as_deref().and_then(|hint| infer_due_from_hint(hint, base)) {
                Some(inferred) => (Some(inferred.date), Some(inferred.label)),
                None => (None, None),
            }
        }
    };

    let due = match due {
        Some(due) => due,
        None => {
            return UrgencyAssessment {
                level: URGENCY_PENDING.to_string(),
                reason: "没有截止日期，也未识别到封闭时间表达（待评估）。".to_string(),
                due_date: None,
                inferred: false,
            }
        }
    };

    let due_days = match day_number(&due) {
        Some(value) => value,
        None => {
            return UrgencyAssessment {
                level: URGENCY_PENDING.to_string(),
                reason: "截止日期形状无法识别（待评估）。".to_string(),
                due_date: None,
                inferred: false,
            }
        }
    };
    let today_number = day_number(&today).unwrap_or(today_days);
    let calendar_days = due_days - today_number;
    let inferred = inferred_label.is_some();
    let target = inferred || task.due_kind == DUE_KIND_TARGET;

    let (level, reason) = if calendar_days < 0 {
        let overdue = -calendar_days;
        (
            URGENCY_HIGH.to_string(),
            format!("已逾期 {overdue} 天（截止 {due}）。"),
        )
    } else if calendar_days == 0 {
        (URGENCY_HIGH.to_string(), format!("今天截止（{due}）。"))
    } else {
        let workdays = workdays_between(&today, &due);
        let base_level = level_for_workdays(workdays);
        let level = if target {
            downgrade(base_level).to_string()
        } else {
            base_level.to_string()
        };
        let kind_note = if target { "目标时间，非硬截止" } else { "硬截止" };
        (level, format!("{due} 截止，还剩 {workdays} 个工作日（{kind_note}）。"))
    };

    let reason = match inferred_label {
        Some(label) => format!("按提示「{label}」推定到 {due}（推定）；{reason}"),
        None => reason,
    };
    UrgencyAssessment {
        level,
        reason,
        due_date: Some(due),
        inferred,
    }
}

/// 四象限视图（OD-QS-403）。象限不落盘，每次读取重算；override 除外。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuadrantView {
    pub task_id: String,
    pub title: String,
    pub quadrant: String,
    pub importance: String,
    pub urgency: String,
    pub importance_reason: Option<String>,
    pub urgency_reason: String,
    pub evaluated_at: String,
    pub is_override: bool,
    /// 已完成 / 已取消 / 暂停 为 false；暂停仍保留重要性。
    pub active: bool,
}

fn quadrant_from_dimensions(important: bool, urgent: bool) -> &'static str {
    match (important, urgent) {
        (true, true) => QUADRANT_Q1,
        (true, false) => QUADRANT_Q2,
        (false, true) => QUADRANT_Q3,
        (false, false) => QUADRANT_Q4,
    }
}

/// 分类规则：只有「高」算重要 / 紧急；中与低归入另一侧。
/// 任一维待评估 → 象限待评估，不强行归类；override 优先并标「用户指定」。
pub fn classify_quadrant(task: &Task, urgency: &UrgencyAssessment, now: f64) -> QuadrantView {
    let importance_pending = task.importance == IMPORTANCE_PENDING;
    let urgency_pending = urgency.level == URGENCY_PENDING;
    let important = task.importance == IMPORTANCE_HIGH;
    let urgent = urgency.level == URGENCY_HIGH;
    let derived = if importance_pending || urgency_pending {
        QUADRANT_PENDING
    } else {
        quadrant_from_dimensions(important, urgent)
    };
    let (quadrant, is_override) = match task.quadrant_override.as_deref() {
        Some(value) => (value.to_string(), true),
        None => (derived.to_string(), false),
    };
    let active = !matches!(
        task.state.as_str(),
        STATE_DONE | STATE_CANCELLED | STATE_PAUSED
    );
    QuadrantView {
        task_id: task.id.clone(),
        title: task.title.clone(),
        quadrant,
        importance: task.importance.clone(),
        urgency: urgency.level.clone(),
        importance_reason: task.importance_reason.clone(),
        urgency_reason: urgency.reason.clone(),
        evaluated_at: crate::task_shell::utc_iso8601(now),
        is_override,
        active,
    }
}

fn quadrant_rank(quadrant: &str) -> u8 {
    match quadrant {
        QUADRANT_Q1 => 1,
        QUADRANT_Q2 => 2,
        QUADRANT_Q3 => 3,
        QUADRANT_Q4 => 4,
        _ => 5,
    }
}

/// 批量读取：并行任务按 active、象限序、标题排序，便于界面直接渲染。
pub fn list_quadrant_views(root: &std::path::Path, now: f64) -> Result<Vec<QuadrantView>, String> {
    list_quadrant_views_at(root, now, 0)
}

pub fn list_quadrant_views_at(
    root: &std::path::Path,
    now: f64,
    utc_offset_minutes: i32,
) -> Result<Vec<QuadrantView>, String> {
    let listing = list_tasks(root, &TaskFilter::default())?;
    let mut views: Vec<QuadrantView> = listing
        .items
        .iter()
        .map(|task| {
            let urgency = compute_urgency_at(task, now, utc_offset_minutes);
            classify_quadrant(task, &urgency, now)
        })
        .collect();
    views.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| quadrant_rank(&a.quadrant).cmp(&quadrant_rank(&b.quadrant)))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });
    Ok(views)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::DUE_KIND_HARD;
    use crate::task_store::{create_task, TaskDraft};
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-qs402-priority-{tag}-{}-{}",
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

    fn secs(year: i64, month: u32, day: u32, hour: i64) -> f64 {
        (days_from_civil(year, month, day) * 86_400 + hour * 3_600) as f64
    }

    fn task_at(temp: &TempRoot, created: f64) -> Task {
        create_task(
            temp.root(),
            &TaskDraft {
                title: "虚构任务".to_string(),
                ..Default::default()
            },
            created,
        )
        .unwrap()
    }

    #[test]
    fn explicit_due_far_away_is_low() {
        let temp = TempRoot::new("far");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-12-20".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_LOW);
        assert!(result.reason.contains("2026-12-20"), "{}", result.reason);
    }

    #[test]
    fn explicit_due_within_two_workdays_is_high() {
        let temp = TempRoot::new("near");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-11-10".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_HIGH);
        assert!(result.reason.contains("2 个工作日"), "{}", result.reason);
    }

    #[test]
    fn target_due_is_one_level_lower() {
        let temp = TempRoot::new("target");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-11-10".to_string());
        task.due_kind = DUE_KIND_TARGET.to_string();
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_MEDIUM);
        assert!(result.reason.contains("目标时间"), "{}", result.reason);
    }

    #[test]
    fn overdue_is_high_with_days() {
        let temp = TempRoot::new("overdue");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-11-05".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_HIGH);
        assert!(result.reason.contains("逾期 3 天"), "{}", result.reason);
    }

    #[test]
    fn due_today_is_high() {
        let temp = TempRoot::new("today");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-11-08".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_HIGH);
        assert!(result.reason.contains("今天截止"), "{}", result.reason);
    }

    #[test]
    fn no_due_and_no_hint_is_pending() {
        let temp = TempRoot::new("pending");
        let task = task_at(&temp, secs(2026, 11, 8, 12));
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.level, URGENCY_PENDING);
        assert!(!result.inferred);
        assert!(result.due_date.is_none());
    }

    #[test]
    fn hint_tomorrow_is_inferred_and_marked() {
        let temp = TempRoot::new("tomorrow");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.hint = Some("明天交初稿".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2026-11-09"));
        assert!(result.inferred);
        assert!(result.reason.contains("推定"), "{}", result.reason);
        assert_eq!(result.level, URGENCY_MEDIUM, "推定按目标时间降一档");
    }

    #[test]
    fn hint_month_end_and_next_month_end() {
        let temp = TempRoot::new("month-end");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.hint = Some("本月底前整理完".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2026-11-30"));
        task.hint = Some("下月底前交".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2026-12-31"));
    }

    #[test]
    fn hint_next_week_weekday_uses_next_week() {
        let temp = TempRoot::new("next-week");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.hint = Some("下周五前给结论".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2026-11-13"));
    }

    #[test]
    fn hint_tomorrow_afternoon_uses_date_only() {
        let temp = TempRoot::new("tomorrow-afternoon");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.hint = Some("明天下午整理会议记录".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2026-11-09"));
        assert!(result.inferred);
    }

    #[test]
    fn hint_soon_urgent_or_unclear_is_pending() {
        let temp = TempRoot::new("pending-hints");
        for hint in ["近期处理", "尽快", "没有时间要求", "有空再说"] {
            let mut task = task_at(&temp, secs(2026, 11, 8, 12));
            task.hint = Some(hint.to_string());
            let result = compute_urgency(&task, secs(2026, 11, 8, 12));
            assert_eq!(result.level, URGENCY_PENDING, "{hint}");
            assert!(result.due_date.is_none(), "{hint}");
        }
    }

    #[test]
    fn month_day_hint_crosses_year_when_past() {
        let temp = TempRoot::new("cross-year");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.hint = Some("1月5日前提交".to_string());
        let result = compute_urgency(&task, secs(2026, 11, 8, 12));
        assert_eq!(result.due_date.as_deref(), Some("2027-01-05"));
    }

    #[test]
    fn month_end_handles_leap_february() {
        let temp = TempRoot::new("leap");
        let mut task = task_at(&temp, secs(2024, 2, 28, 12));
        task.hint = Some("本月底前完成".to_string());
        let result = compute_urgency(&task, secs(2024, 2, 28, 12));
        assert_eq!(result.due_date.as_deref(), Some("2024-02-29"));
    }

    #[test]
    fn local_offset_changes_today_boundary() {
        let temp = TempRoot::new("timezone");
        let mut task = task_at(&temp, secs(2026, 11, 8, 12));
        task.due_date = Some("2026-11-09".to_string());
        let now = secs(2026, 11, 8, 20);
        let utc = compute_urgency_at(&task, now, 0);
        assert!(!utc.reason.contains("今天截止"), "{}", utc.reason);
        let local = compute_urgency_at(&task, now, 480);
        assert!(local.reason.contains("今天截止"), "{}", local.reason);
    }

    #[test]
    fn month_end_crosses_december() {
        let temp = TempRoot::new("december");
        let mut task = task_at(&temp, secs(2026, 12, 15, 12));
        task.hint = Some("下月底前交".to_string());
        let result = compute_urgency(&task, secs(2026, 12, 15, 12));
        assert_eq!(result.due_date.as_deref(), Some("2027-01-31"));
    }

    fn prioritized(
        temp: &TempRoot,
        importance: &str,
        due: Option<&str>,
        kind: &str,
    ) -> Task {
        let mut task = task_at(temp, secs(2026, 11, 8, 12));
        task.importance = importance.to_string();
        if importance != IMPORTANCE_PENDING {
            task.importance_source = Some(crate::task_store::IMPORTANCE_SOURCE_RULE.to_string());
            task.importance_reason = Some("虚构依据".to_string());
        }
        task.due_date = due.map(|value| value.to_string());
        task.due_kind = kind.to_string();
        task
    }

    fn view_of(task: &Task, now: f64) -> QuadrantView {
        let urgency = compute_urgency(task, now);
        classify_quadrant(task, &urgency, now)
    }

    #[test]
    fn important_and_urgent_is_q1() {
        let temp = TempRoot::new("q1");
        let task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_Q1);
        assert!(view.active);
        assert!(!view.is_override);
        assert_eq!(view.importance_reason.as_deref(), Some("虚构依据"));
    }

    #[test]
    fn important_not_urgent_is_q2() {
        let temp = TempRoot::new("q2");
        let task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-12-20"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_Q2);
    }

    #[test]
    fn urgent_not_important_is_q3() {
        let temp = TempRoot::new("q3");
        let task = prioritized(&temp, "低", Some("2026-11-10"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_Q3);
    }

    #[test]
    fn neither_is_q4() {
        let temp = TempRoot::new("q4");
        let task = prioritized(&temp, "低", Some("2026-12-20"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_Q4);
    }

    #[test]
    fn pending_importance_is_not_forced() {
        let temp = TempRoot::new("pending-importance");
        let task = prioritized(&temp, IMPORTANCE_PENDING, Some("2026-11-10"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_PENDING);
        assert_eq!(view.urgency, URGENCY_HIGH);
    }

    #[test]
    fn pending_urgency_is_not_forced() {
        let temp = TempRoot::new("pending-urgency");
        let task = prioritized(&temp, IMPORTANCE_HIGH, None, DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_PENDING);
        assert_eq!(view.urgency, URGENCY_PENDING);
    }

    #[test]
    fn unrecognized_hint_keeps_quadrant_pending() {
        let temp = TempRoot::new("pending-hint");
        let mut task = prioritized(&temp, IMPORTANCE_HIGH, None, DUE_KIND_HARD);
        task.hint = Some("近期处理".to_string());
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_PENDING);
        assert!(view.urgency_reason.contains("待评估"), "{}", view.urgency_reason);
    }

    #[test]
    fn target_due_changes_quadrant_for_same_date() {
        let temp = TempRoot::new("target-quadrant");
        let hard = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        let target = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_TARGET);
        let hard_view = view_of(&hard, secs(2026, 11, 8, 12));
        let target_view = view_of(&target, secs(2026, 11, 8, 12));
        assert_eq!(hard_view.quadrant, QUADRANT_Q1);
        assert_eq!(target_view.quadrant, QUADRANT_Q2);
        assert_ne!(hard_view.urgency, target_view.urgency);
    }

    #[test]
    fn overdue_but_low_importance_is_q3_not_q1() {
        let temp = TempRoot::new("overdue-q3");
        let task = prioritized(&temp, "低", Some("2026-11-05"), DUE_KIND_HARD);
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.urgency, URGENCY_HIGH);
        assert_eq!(view.quadrant, QUADRANT_Q3);
        assert_ne!(view.quadrant, QUADRANT_Q1);
    }

    #[test]
    fn override_wins_and_is_marked_user_specified() {
        let temp = TempRoot::new("override");
        let mut task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        task.quadrant_override = Some(crate::task_store::QUADRANT_Q2.to_string());
        let view = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(view.quadrant, QUADRANT_Q2);
        assert!(view.is_override);
        task.quadrant_override = None;
        let cleared = view_of(&task, secs(2026, 11, 8, 12));
        assert_eq!(cleared.quadrant, QUADRANT_Q1);
        assert!(!cleared.is_override);
    }

    #[test]
    fn done_and_cancelled_are_inactive() {
        let temp = TempRoot::new("inactive");
        let done_task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        crate::task_store::transition_task(
            temp.root(),
            &done_task.id,
            crate::task_store::STATE_IN_PROGRESS,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        crate::task_store::transition_task(
            temp.root(),
            &done_task.id,
            crate::task_store::STATE_DONE,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        let done = processed(&temp, &done_task.id);
        assert!(!done.active);

        let cancelled_task = prioritized(&temp, "低", Some("2026-11-10"), DUE_KIND_HARD);
        crate::task_store::transition_task(
            temp.root(),
            &cancelled_task.id,
            crate::task_store::STATE_CANCELLED,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        let cancelled = processed(&temp, &cancelled_task.id);
        assert!(!cancelled.active);
    }

    #[test]
    fn paused_keeps_importance_but_is_inactive() {
        let temp = TempRoot::new("paused");
        let task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        crate::task_store::set_importance(
            temp.root(),
            &task.id,
            IMPORTANCE_HIGH,
            crate::task_store::IMPORTANCE_SOURCE_RULE,
            Some("虚构依据"),
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        crate::task_store::transition_task(
            temp.root(),
            &task.id,
            crate::task_store::STATE_PAUSED,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        let view = processed(&temp, &task.id);
        assert!(!view.active);
        assert_eq!(view.importance, IMPORTANCE_HIGH, "暂停仍保留重要性");
    }

    fn processed(temp: &TempRoot, id: &str) -> QuadrantView {
        let task = crate::task_store::read_task(temp.root(), id).unwrap();
        view_of(&task, secs(2026, 11, 8, 12))
    }

    #[test]
    fn list_sorts_active_before_inactive_and_uses_offset() {
        let temp = TempRoot::new("list-quadrants");
        let q3 = prioritized(&temp, "低", Some("2026-11-10"), DUE_KIND_HARD);
        let done_task = prioritized(&temp, IMPORTANCE_HIGH, Some("2026-11-10"), DUE_KIND_HARD);
        crate::task_store::transition_task(
            temp.root(),
            &done_task.id,
            crate::task_store::STATE_IN_PROGRESS,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        crate::task_store::transition_task(
            temp.root(),
            &done_task.id,
            crate::task_store::STATE_DONE,
            None,
            secs(2026, 11, 8, 12),
        )
        .unwrap();
        let views = list_quadrant_views_at(temp.root(), secs(2026, 11, 8, 12), 480).unwrap();
        assert_eq!(views.len(), 2);
        assert!(views[0].task_id == q3.id || views[0].task_id == done_task.id);
        assert_eq!(views[0].task_id, q3.id, "活跃任务排在已结束任务之前");
        assert!(!views[0].evaluated_at.is_empty());
        assert!(!views[1].active);
    }
}
