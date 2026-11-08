//! 排期引擎（OD-QS-408）。
//!
//! 纯函数、不落盘（落盘归 407/409）。输入活跃任务、象限视图、工作时间配置、
//! 既有已确认/锁定条目；产出建议、冲突与未排期原因。
//! 顺序：锁定与已确认固定 > 硬截止 > 依赖先行 > Q1 > Q2 > Q3 > Q4；
//! 每日容量按配置的 80% 用于自动建议，留 20% 机动；Q4 每天最多占 25%。
//! 时间在内部用 UTC 秒，输入窗口按显式本地偏移换算，不依赖模型服务器时区。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::availability::AvailabilityConfig;
use crate::priority_rules::{QuadrantView, QUADRANT_PENDING, URGENCY_PENDING};
use crate::schedule_store::{
    ScheduleEntry, STATUS_ADJUSTED, STATUS_CANCELLED, STATUS_CONFIRMED, STATUS_DONE,
};
use crate::task_store::{
    Task, QUADRANT_Q1, QUADRANT_Q2, QUADRANT_Q3, QUADRANT_Q4, STATE_CANCELLED, STATE_DONE,
    STATE_PAUSED, STATE_PENDING_CONFIRM,
};
use crate::worklog::{civil_from_days, days_from_civil};

pub const HORIZON_DAYS: i64 = 14;
pub const SLOT_GRANULARITY_MINUTES: i64 = 30;
/// 自动建议最多使用每日容量的 80%，其余留作机动。
pub const BUFFER_RATIO: f64 = 0.8;
/// Q4 每天最多占自动容量的 25%。
pub const Q4_SHARE: f64 = 0.25;

const MINUTE: i64 = 60;
const DAY: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub task_id: String,
    pub title: String,
    pub quadrant: String,
    pub importance: String,
    pub start_at: String,
    pub end_at: String,
    pub duration: u32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchedulingConflict {
    pub kind: String,
    pub task_ids: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnscheduledTask {
    pub task_id: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchedulingResult {
    pub suggestions: Vec<Suggestion>,
    pub conflicts: Vec<SchedulingConflict>,
    pub unscheduled: Vec<UnscheduledTask>,
    pub horizon_days: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: i64,
    end: i64,
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

fn date_string(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32 + 1
}

fn iso_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(DAY);
    let rem = seconds.rem_euclid(DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn parse_iso(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return None;
    }
    let (year, month, day) = parse_date(&value[0..10])?;
    let hour: i64 = value[11..13].parse().ok()?;
    let minute: i64 = value[14..16].parse().ok()?;
    let second: i64 = value[17..19].parse().ok()?;
    if bytes[13] != b':' || bytes[16] != b':' || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(days_from_civil(year, month, day) * DAY + hour * 3_600 + minute * 60 + second)
}

fn minutes_of(value: &str) -> Option<i64> {
    if value.len() != 5 || value.as_bytes()[2] != b':' {
        return None;
    }
    let hour: i64 = value[0..2].parse().ok()?;
    let minute: i64 = value[3..5].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn local_window(days: i64, start: &str, end: &str, offset_minutes: i32) -> Option<Span> {
    let start = minutes_of(start)?;
    let end = minutes_of(end)?;
    let shift = offset_minutes as i64 * MINUTE;
    Some(Span {
        start: days * DAY + start * MINUTE - shift,
        end: days * DAY + end * MINUTE - shift,
    })
}

fn split(span: Span, blocker: Span) -> Vec<Span> {
    if blocker.start >= span.end || blocker.end <= span.start {
        return vec![span];
    }
    let mut parts = Vec::new();
    if blocker.start > span.start {
        parts.push(Span {
            start: span.start,
            end: blocker.start,
        });
    }
    if blocker.end < span.end {
        parts.push(Span {
            start: blocker.end,
            end: span.end,
        });
    }
    parts
}

fn subtract(spans: Vec<Span>, blockers: &[Span]) -> Vec<Span> {
    let mut current = spans;
    for blocker in blockers {
        current = current
            .into_iter()
            .flat_map(|span| split(span, *blocker))
            .collect();
    }
    current
}

fn blocks_for_day(availability: &AvailabilityConfig, day: i64, offset: i32) -> Vec<Span> {
    let date = date_string(day);
    let mut blockers = Vec::new();
    for window in &availability.breaks {
        if let Some(span) = local_window(day, &window.start, &window.end, offset) {
            blockers.push(span);
        }
    }
    for block in &availability.unavailable {
        if block.date == date {
            if let Some(span) = local_window(day, &block.start, &block.end, offset) {
                blockers.push(span);
            }
        }
    }
    blockers
}

fn free_windows(
    availability: &AvailabilityConfig,
    day: i64,
    offset: i32,
    occupied: &[Span],
) -> Vec<Span> {
    if !availability.workdays.contains(&weekday(day)) {
        return Vec::new();
    }
    let mut windows = Vec::new();
    for window in &availability.daily_windows {
        if let Some(span) = local_window(day, &window.start, &window.end, offset) {
            windows.push(span);
        }
    }
    let blockers = blocks_for_day(availability, day, offset);
    let mut windows = subtract(windows, &blockers);
    windows = subtract(windows, occupied);
    windows
}

fn fixed_span(entry: &ScheduleEntry) -> Option<Span> {
    if entry.is_locked
        || matches!(entry.status.as_str(), STATUS_CONFIRMED)
    {
        if matches!(
            entry.status.as_str(),
            STATUS_CANCELLED | STATUS_ADJUSTED | STATUS_DONE
        ) {
            return None;
        }
        let start = parse_iso(&entry.start_at)?;
        let end = parse_iso(&entry.end_at)?;
        if end <= start {
            return None;
        }
        return Some(Span { start, end });
    }
    None
}

fn overlap(first: Span, second: Span) -> bool {
    first.start < second.end && second.start < first.end
}

fn local_day_of(seconds: i64, offset: i32) -> i64 {
    (seconds + offset as i64 * MINUTE).div_euclid(DAY)
}

fn day_overlap_minutes(span: Span, day: i64, offset: i32) -> i64 {
    let shift = offset as i64 * MINUTE;
    let day_start = day * DAY - shift;
    let day_end = day_start + DAY;
    let start = span.start.max(day_start);
    let end = span.end.min(day_end);
    if end <= start {
        return 0;
    }
    (end - start).div_euclid(MINUTE)
}

fn importance_rank(value: &str) -> i32 {
    match value {
        "高" => 3,
        "中" => 2,
        "低" => 1,
        _ => 0,
    }
}

fn quadrant_rank(value: &str) -> u8 {
    match value {
        QUADRANT_Q1 => 1,
        QUADRANT_Q2 => 2,
        QUADRANT_Q3 => 3,
        QUADRANT_Q4 => 4,
        _ => 5,
    }
}

fn scheduled_active(task: &Task) -> bool {
    !matches!(
        task.state.as_str(),
        STATE_DONE | STATE_CANCELLED | STATE_PAUSED | STATE_PENDING_CONFIRM
    )
}

pub fn generate_suggestions(
    tasks: &[Task],
    quadrants: &[QuadrantView],
    availability: &AvailabilityConfig,
    confirmed: &[ScheduleEntry],
    now: f64,
) -> SchedulingResult {
    generate_suggestions_at(tasks, quadrants, availability, confirmed, now, 0)
}

pub fn generate_suggestions_at(
    tasks: &[Task],
    quadrants: &[QuadrantView],
    availability: &AvailabilityConfig,
    confirmed: &[ScheduleEntry],
    now: f64,
    utc_offset_minutes: i32,
) -> SchedulingResult {
    let offset = utc_offset_minutes;
    let now_seconds = now.floor() as i64;
    let today = local_day_of(now_seconds, offset);
    let views: HashMap<&str, &QuadrantView> = quadrants
        .iter()
        .map(|view| (view.task_id.as_str(), view))
        .collect();
    let task_map: HashMap<&str, &Task> = tasks
        .iter()
        .map(|task| (task.id.as_str(), task))
        .collect();

    let mut conflicts = Vec::new();
    let mut unscheduled = Vec::new();
    let mut suggestions = Vec::new();

    let fixed: Vec<(String, Span, &ScheduleEntry)> = confirmed
        .iter()
        .filter_map(|entry| fixed_span(entry).map(|span| (entry.task_id.clone(), span, entry)))
        .collect();

    for (index, (task_id, span, entry)) in fixed.iter().enumerate() {
        for (other_id, other, other_entry) in fixed.iter().skip(index + 1) {
            if overlap(*span, *other) {
                let kind = if entry.is_locked || other_entry.is_locked {
                    "锁定冲突"
                } else {
                    "时间重叠"
                };
                conflicts.push(SchedulingConflict {
                    kind: kind.to_string(),
                    task_ids: vec![task_id.clone(), other_id.clone()],
                    message: format!(
                        "{} 与 {} 的固定排期重叠：{}–{}",
                        task_id, other_id, entry.start_at, entry.end_at
                    ),
                });
            }
        }
    }

    for task in tasks {
        for dep in &task.depends_on {
            if let (Some(successor), Some(predecessor)) =
                (fixed.iter().find(|(id, _, _)| id == &task.id), fixed.iter().find(|(id, _, _)| id == dep))
            {
                if successor.1.start < predecessor.1.end {
                    conflicts.push(SchedulingConflict {
                        kind: "依赖倒置".to_string(),
                        task_ids: vec![task.id.clone(), dep.clone()],
                        message: format!("固定排期里 {} 早于前置 {}", task.id, dep),
                    });
                }
            }
        }
    }

    let full_cap = (availability.daily_capacity_hours * 60.0).floor() as i64;
    let cap_per_day = (full_cap as f64 * BUFFER_RATIO).floor() as i64;
    let q4_cap = (cap_per_day as f64 * Q4_SHARE).floor() as i64;
    let mut day_used: BTreeMap<i64, i64> = BTreeMap::new();
    for (_, span, _) in &fixed {
        let day = local_day_of(span.start, offset);
        let used = day_overlap_minutes(*span, day, offset);
        *day_used.entry(day).or_insert(0) += used;
    }
    for (day, used) in &day_used {
        if *used > full_cap {
            conflicts.push(SchedulingConflict {
                kind: "超容量".to_string(),
                task_ids: Vec::new(),
                message: format!("{} 的固定排期已占 {used} 分钟，超过每日容量 {full_cap} 分钟。", date_string(*day)),
            });
        }
    }

    let max_window = availability
        .daily_windows
        .iter()
        .filter_map(|window| {
            let start = minutes_of(&window.start)?;
            let end = minutes_of(&window.end)?;
            if end > start { Some(end - start) } else { None }
        })
        .max()
        .unwrap_or(0);

    let candidates: Vec<&Task> = tasks.iter().filter(|task| scheduled_active(task)).collect();
    let candidate_ids: HashSet<&str> = candidates.iter().map(|task| task.id.as_str()).collect();

    // Kahn 拓扑：环内节点整体进冲突与未排期。
    let mut indegree: HashMap<&str, usize> = HashMap::new();
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for task in &candidates {
        indegree.entry(task.id.as_str()).or_insert(0);
        for dep in &task.depends_on {
            if candidate_ids.contains(dep.as_str()) {
                *indegree.entry(task.id.as_str()).or_insert(0) += 1;
                children.entry(dep.as_str()).or_default().push(task.id.as_str());
            }
        }
    }
    let mut ready: Vec<&str> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let sort_key = |id: &str| {
        let task = task_map[id];
        let view = views.get(id);
        let due = task
            .due_date
            .as_deref()
            .and_then(parse_date)
            .map(|(y, m, d)| days_from_civil(y, m, d))
            .unwrap_or(i64::MAX / 2);
        let importance = importance_rank(&task.importance);
        let quadrant = view.map(|view| quadrant_rank(&view.quadrant)).unwrap_or(5);
        (due, -importance, quadrant, id.to_string())
    };
    ready.sort_by_key(|id| sort_key(*id));
    let mut order: Vec<&str> = Vec::new();
    while let Some(id) = ready.first().copied() {
        ready.remove(0);
        order.push(id);
        if let Some(kids) = children.get(id) {
            for kid in kids {
                if let Some(count) = indegree.get_mut(kid) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(*kid);
                    }
                }
            }
            ready.sort_by_key(|id| sort_key(*id));
        }
    }
    if order.len() != candidates.len() {
        let remaining: Vec<&Task> = candidates
            .iter()
            .filter(|task| !order.contains(&task.id.as_str()))
            .copied()
            .collect();
        let ids: Vec<String> = remaining.iter().map(|task| task.id.clone()).collect();
        conflicts.push(SchedulingConflict {
            kind: "依赖环".to_string(),
            task_ids: ids.clone(),
            message: "任务依赖形成环，环内任务不排期。".to_string(),
        });
        for task in remaining {
            unscheduled.push(UnscheduledTask {
                task_id: task.id.clone(),
                title: task.title.clone(),
                reason: "依赖关系形成环".to_string(),
            });
        }
    }

    let mut occupied: Vec<Span> = fixed.iter().map(|(_, span, _)| *span).collect();
    let mut dep_ends: HashMap<String, i64> = HashMap::new();
    let mut q4_used: BTreeMap<i64, i64> = BTreeMap::new();

    for id in order {
        let task = task_map[id];
        let view = match views.get(id) {
            Some(view) => view,
            None => {
                unscheduled.push(UnscheduledTask {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    reason: "缺少象限视图".to_string(),
                });
                continue;
            }
        };
        if view.quadrant == QUADRANT_PENDING || view.urgency == URGENCY_PENDING {
            unscheduled.push(UnscheduledTask {
                task_id: task.id.clone(),
                title: task.title.clone(),
                reason: "重要性或紧急性待评估".to_string(),
            });
            continue;
        }
        let duration = match task.estimated_minutes {
            Some(minutes) if minutes > 0 => minutes as i64,
            _ => {
                unscheduled.push(UnscheduledTask {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    reason: "缺少预计耗时".to_string(),
                });
                continue;
            }
        };
        if duration > max_window {
            unscheduled.push(UnscheduledTask {
                task_id: task.id.clone(),
                title: task.title.clone(),
                reason: format!("超过单日最大可用时段（{max_window} 分钟），建议拆分"),
            });
            continue;
        }
        let mut earliest = now_seconds;
        let mut unmet = false;
        for dep in &task.depends_on {
            if let Some(end) = dep_ends.get(dep) {
                earliest = earliest.max(*end);
                continue;
            }
            if let Some(dep_task) = task_map.get(dep.as_str()) {
                if matches!(dep_task.state.as_str(), STATE_DONE | STATE_CANCELLED) {
                    continue;
                }
                if candidate_ids.contains(dep.as_str()) {
                    unmet = true;
                    break;
                }
            }
            unmet = true;
            break;
        }
        if unmet {
            unscheduled.push(UnscheduledTask {
                task_id: task.id.clone(),
                title: task.title.clone(),
                reason: "依赖未满足".to_string(),
            });
            continue;
        }
        let deadline = task
            .due_date
            .as_deref()
            .and_then(parse_date)
            .map(|(y, m, d)| days_from_civil(y, m, d));
        if task.due_kind == "hard" {
            if let Some(day) = deadline {
                if day < local_day_of(now_seconds, offset) {
                    conflicts.push(SchedulingConflict {
                        kind: "截止不可满足".to_string(),
                        task_ids: vec![task.id.clone()],
                        message: format!("{} 已超过硬截止 {}", task.id, date_string(day)),
                    });
                    unscheduled.push(UnscheduledTask {
                        task_id: task.id.clone(),
                        title: task.title.clone(),
                        reason: "硬截止已过".to_string(),
                    });
                    continue;
                }
            }
        }
        let mut placed = false;
        let mut blocked_by_deadline = false;
        for day in today..(today + HORIZON_DAYS) {
            if let Some(deadline_day) = deadline {
                if task.due_kind == "hard" && day > deadline_day {
                    blocked_by_deadline = true;
                    break;
                }
            }
            let remaining_cap = cap_per_day - day_used.get(&day).copied().unwrap_or(0);
            if remaining_cap < duration {
                continue;
            }
            if view.quadrant == QUADRANT_Q4 {
                let used_q4 = q4_used.get(&day).copied().unwrap_or(0);
                if used_q4 + duration > q4_cap {
                    continue;
                }
            }
            let windows = free_windows(availability, day, offset, &occupied);
            for window in windows {
                let granularity = SLOT_GRANULARITY_MINUTES * MINUTE;
                let mut start = window.start.max(earliest);
                let remainder = start.rem_euclid(granularity);
                if remainder != 0 {
                    start += granularity - remainder;
                }
                let end = start + duration * MINUTE;
                if end > window.end {
                    continue;
                }
                if start >= window.end {
                    continue;
                }
                let span = Span { start, end };
                if occupied.iter().any(|other| overlap(span, *other)) {
                    continue;
                }
                suggestions.push(Suggestion {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    quadrant: view.quadrant.clone(),
                    importance: task.importance.clone(),
                    start_at: iso_utc(start),
                    end_at: iso_utc(end),
                    duration: duration as u32,
                    reason: format!(
                        "{}；{}；每日容量剩余 {remaining_cap} 分钟",
                        view.quadrant, view.urgency_reason
                    ),
                });
                occupied.push(span);
                *day_used.entry(day).or_insert(0) += duration;
                if view.quadrant == QUADRANT_Q4 {
                    *q4_used.entry(day).or_insert(0) += duration;
                }
                dep_ends.insert(task.id.clone(), end);
                placed = true;
                break;
            }
            if placed {
                break;
            }
        }
        if !placed {
            if blocked_by_deadline {
                conflicts.push(SchedulingConflict {
                    kind: "截止不可满足".to_string(),
                    task_ids: vec![task.id.clone()],
                    message: format!("{} 在硬截止前没有足够可用时间", task.id),
                });
                unscheduled.push(UnscheduledTask {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    reason: "截止前容量不足".to_string(),
                });
            } else {
                unscheduled.push(UnscheduledTask {
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    reason: "可用时间不足".to_string(),
                });
            }
        }
    }

    suggestions.sort_by(|a, b| {
        a.start_at
            .cmp(&b.start_at)
            .then_with(|| a.task_id.cmp(&b.task_id))
    });
    SchedulingResult {
        suggestions,
        conflicts,
        unscheduled,
        horizon_days: HORIZON_DAYS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::availability::{TimeWindow, UnavailableBlock};
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
                "omnidesk-qs408-scheduling-{tag}-{}-{}",
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

    const NOW: f64 = 1_791_000_000.0;

    fn first_day() -> String {
        date_string(local_day_of(NOW.floor() as i64, 0))
    }

    fn task(temp: &TempRoot, title: &str) -> Task {
        let mut task = create_task(
            temp.root(),
            &TaskDraft {
                title: title.to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        task.state = crate::task_store::STATE_IN_PROGRESS.to_string();
        task
    }

    fn availability() -> AvailabilityConfig {
        AvailabilityConfig {
            workdays: vec![1, 2, 3, 4, 5, 6, 7],
            daily_windows: vec![TimeWindow {
                start: "09:00".to_string(),
                end: "18:00".to_string(),
            }],
            breaks: vec![TimeWindow {
                start: "12:00".to_string(),
                end: "13:00".to_string(),
            }],
            unavailable: Vec::new(),
            daily_capacity_hours: 8.0,
            timezone: "UTC".to_string(),
            ..Default::default()
        }
    }

    fn view(task: &Task, quadrant: &str, urgency: &str, importance: &str) -> QuadrantView {
        QuadrantView {
            task_id: task.id.clone(),
            title: task.title.clone(),
            quadrant: quadrant.to_string(),
            importance: importance.to_string(),
            urgency: urgency.to_string(),
            importance_reason: Some("虚构依据".to_string()),
            urgency_reason: "虚构紧急性依据".to_string(),
            evaluated_at: "2026-11-08T00:00:00Z".to_string(),
            is_override: false,
            active: true,
        }
    }

    fn entry(task_id: &str, start: &str, end: &str, status: &str, locked: bool) -> ScheduleEntry {
        ScheduleEntry {
            schema: "omnidesk.schedule.v1".to_string(),
            id: format!("s-{task_id}"),
            task_id: task_id.to_string(),
            start_at: start.to_string(),
            end_at: end.to_string(),
            duration: 60,
            status: status.to_string(),
            is_locked: locked,
            created_at: "2026-11-08T00:00:00Z".to_string(),
            updated_at: "2026-11-08T00:00:00Z".to_string(),
            adjustment_reason: None,
            adjustments: Vec::new(),
        }
    }

    fn run(_temp: &TempRoot, tasks: &[Task], quadrants: &[QuadrantView], confirmed: &[ScheduleEntry]) -> SchedulingResult {
        generate_suggestions(tasks, quadrants, &availability(), confirmed, NOW)
    }

    #[test]
    fn single_task_gets_first_free_slot() {
        let temp = TempRoot::new("single");
        let mut task = task(&temp, "写周报");
        task.estimated_minutes = Some(90);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let result = run(&temp, &[task], &views, &[]);
        assert_eq!(result.suggestions.len(), 1);
        assert_eq!(result.suggestions[0].duration, 90);
        assert!(!result.suggestions[0].start_at.is_empty());
    }

    #[test]
    fn same_deadline_high_importance_first() {
        let temp = TempRoot::new("importance-order");
        let mut low = task(&temp, "低重要");
        low.estimated_minutes = Some(60);
        low.due_date = Some("2026-11-10".to_string());
        low.importance = "低".to_string();
        let mut high = task(&temp, "高重要");
        high.estimated_minutes = Some(60);
        high.due_date = Some("2026-11-10".to_string());
        high.importance = "高".to_string();
        let views = vec![
            view(&low, QUADRANT_Q3, "高", "低"),
            view(&high, QUADRANT_Q1, "高", "高"),
        ];
        let tasks = vec![low, high];
        let result = run(&temp, &tasks, &views, &[]);
        assert_eq!(result.suggestions.len(), 2);
        assert_eq!(result.suggestions[0].task_id, tasks[1].id);
    }

    #[test]
    fn dependency_precedes_successor() {
        let temp = TempRoot::new("dependency");
        let mut first = task(&temp, "前置");
        first.estimated_minutes = Some(60);
        let mut second = task(&temp, "后置");
        second.estimated_minutes = Some(60);
        second.depends_on = vec![first.id.clone()];
        let views = vec![
            view(&first, QUADRANT_Q1, "高", "高"),
            view(&second, QUADRANT_Q1, "高", "高"),
        ];
        let result = run(&temp, &[first.clone(), second.clone()], &views, &[]);
        let first_item = result.suggestions.iter().find(|item| item.task_id == first.id).unwrap();
        let second_item = result.suggestions.iter().find(|item| item.task_id == second.id).unwrap();
        assert!(second_item.start_at >= first_item.end_at, "{first_item:?} {second_item:?}");
    }

    #[test]
    fn dependency_cycle_reports_conflict_and_unscheduled() {
        let temp = TempRoot::new("cycle");
        let mut first = task(&temp, "甲");
        first.estimated_minutes = Some(60);
        let mut second = task(&temp, "乙");
        second.estimated_minutes = Some(60);
        first.depends_on = vec![second.id.clone()];
        second.depends_on = vec![first.id.clone()];
        let views = vec![
            view(&first, QUADRANT_Q1, "高", "高"),
            view(&second, QUADRANT_Q1, "高", "高"),
        ];
        let result = run(&temp, &[first, second], &views, &[]);
        assert!(result.suggestions.is_empty());
        assert!(result.conflicts.iter().any(|conflict| conflict.kind == "依赖环"));
        assert_eq!(result.unscheduled.len(), 2);
    }

    #[test]
    fn missing_estimate_is_unscheduled() {
        let temp = TempRoot::new("no-estimate");
        let task = task(&temp, "没有耗时");
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let result = run(&temp, &[task], &views, &[]);
        assert!(result.suggestions.is_empty());
        assert!(result.unscheduled[0].reason.contains("缺少预计耗时"));
    }

    #[test]
    fn too_long_suggests_split() {
        let temp = TempRoot::new("split");
        let mut task = task(&temp, "太长");
        task.estimated_minutes = Some(10 * 60);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let result = run(&temp, &[task], &views, &[]);
        assert!(result.suggestions.is_empty());
        assert!(result.unscheduled[0].reason.contains("建议拆分"));
    }

    #[test]
    fn confirmed_entry_is_not_rescheduled_and_blocks_overlap() {
        let temp = TempRoot::new("confirmed");
        let mut task = task(&temp, "已确认任务");
        task.estimated_minutes = Some(60);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let fixed = vec![entry(
            &task.id,
            &format!("{}T09:00:00Z", first_day()),
            &format!("{}T10:00:00Z", first_day()),
            STATUS_CONFIRMED,
            false,
        )];
        let result = run(&temp, &[task.clone()], &views, &fixed);
        assert_eq!(result.suggestions.len(), 1);
        assert!(
            result.suggestions[0].start_at >= format!("{}T10:00:00Z", first_day())
        );
    }

    #[test]
    fn locked_and_confirmed_overlap_is_lock_conflict() {
        let temp = TempRoot::new("lock-conflict");
        let first = task(&temp, "甲");
        let second = task(&temp, "乙");
        let fixed = vec![
            entry(&first.id, "2026-11-08T09:00:00Z", "2026-11-08T10:00:00Z", STATUS_CONFIRMED, true),
            entry(&second.id, "2026-11-08T09:30:00Z", "2026-11-08T10:30:00Z", STATUS_CONFIRMED, false),
        ];
        let views = vec![view(&first, QUADRANT_Q1, "高", "高"), view(&second, QUADRANT_Q1, "高", "高")];
        let result = run(&temp, &[first, second], &views, &fixed);
        assert!(result.conflicts.iter().any(|conflict| conflict.kind == "锁定冲突"));
    }

    #[test]
    fn daily_capacity_respects_buffer_and_spills_to_next_day() {
        let temp = TempRoot::new("capacity");
        let mut first = task(&temp, "甲");
        first.estimated_minutes = Some(300);
        let mut second = task(&temp, "乙");
        second.estimated_minutes = Some(300);
        let views = vec![
            view(&first, QUADRANT_Q1, "高", "高"),
            view(&second, QUADRANT_Q1, "高", "高"),
        ];
        let result = run(&temp, &[first, second], &views, &[]);
        assert_eq!(result.suggestions.len(), 2);
        assert!(result.suggestions[1].start_at[0..10] != result.suggestions[0].start_at[0..10]);
    }

    #[test]
    fn q4_is_capped_when_q1_fills_the_day() {
        let temp = TempRoot::new("q4-cap");
        let mut q1 = task(&temp, "Q1");
        q1.estimated_minutes = Some(360);
        let mut q4 = task(&temp, "Q4");
        q4.estimated_minutes = Some(60);
        let views = vec![
            view(&q1, QUADRANT_Q1, "高", "高"),
            view(&q4, QUADRANT_Q4, "低", "低"),
        ];
        let result = run(&temp, &[q4.clone(), q1], &views, &[]);
        let q4_suggestion = result.suggestions.iter().find(|item| item.task_id == q4.id).unwrap();
        assert!(!q4_suggestion.start_at[0..10].is_empty());
        assert!(result.suggestions.iter().any(|item| item.task_id == q4.id));
    }

    #[test]
    fn weekend_is_skipped() {
        let temp = TempRoot::new("weekend");
        let mut task = task(&temp, "周末任务");
        task.estimated_minutes = Some(60);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let mut config = availability();
        config.workdays = vec![1, 2, 3, 4, 5];
        let result = generate_suggestions(&[task], &views, &config, &[], NOW);
        assert_eq!(result.suggestions.len(), 1);
        let day = parse_iso(&result.suggestions[0].start_at).unwrap();
        let weekday = weekday(local_day_of(day, 0));
        assert!(weekday <= 5, "{weekday}");
    }

    #[test]
    fn unavailable_and_breaks_are_skipped() {
        let temp = TempRoot::new("unavailable");
        let mut task = task(&temp, "避开不可用");
        task.estimated_minutes = Some(60);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let mut config = availability();
        let first_day = date_string(local_day_of(NOW.floor() as i64, 0));
        config.unavailable = vec![UnavailableBlock {
            date: first_day,
            start: "09:00".to_string(),
            end: "18:00".to_string(),
            reason: "休假".to_string(),
        }];
        let result = generate_suggestions(&[task], &views, &config, &[], NOW);
        assert_eq!(result.suggestions.len(), 1);
        let start = parse_iso(&result.suggestions[0].start_at).unwrap();
        assert!(local_day_of(start, 0) > local_day_of(NOW.floor() as i64, 0));
    }

    #[test]
    fn conflict_from_overlapping_confirmed_entries() {
        let temp = TempRoot::new("overlap-conflict");
        let first = task(&temp, "甲");
        let second = task(&temp, "乙");
        let fixed = vec![
            entry(&first.id, "2026-11-08T09:00:00Z", "2026-11-08T10:00:00Z", STATUS_CONFIRMED, false),
            entry(&second.id, "2026-11-08T09:30:00Z", "2026-11-08T10:30:00Z", STATUS_CONFIRMED, false),
        ];
        let views = vec![view(&first, QUADRANT_Q1, "高", "高"), view(&second, QUADRANT_Q2, "低", "高")];
        let result = run(&temp, &[first, second], &views, &fixed);
        assert!(result.conflicts.iter().any(|conflict| conflict.kind == "时间重叠"));
    }

    #[test]
    fn deadline_without_enough_time_is_conflict_and_unscheduled() {
        let temp = TempRoot::new("deadline");
        let mut task = task(&temp, "硬截止");
        task.estimated_minutes = Some(400);
        task.due_date = Some(first_day());
        task.due_kind = "hard".to_string();
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let result = run(&temp, &[task], &views, &[]);
        assert!(result.conflicts.iter().any(|conflict| conflict.kind == "截止不可满足"));
        assert!(result.unscheduled.iter().any(|item| item.reason.contains("截止")));
    }

    #[test]
    fn q4_does_not_preempt_q2() {
        let temp = TempRoot::new("q2-first");
        let mut q2 = task(&temp, "Q2");
        q2.estimated_minutes = Some(60);
        let mut q4 = task(&temp, "Q4");
        q4.estimated_minutes = Some(60);
        let views = vec![
            view(&q2, QUADRANT_Q2, "低", "高"),
            view(&q4, QUADRANT_Q4, "低", "低"),
        ];
        let result = run(&temp, &[q4.clone(), q2.clone()], &views, &[]);
        let q2_item = result.suggestions.iter().find(|item| item.task_id == q2.id).unwrap();
        let q4_item = result.suggestions.iter().find(|item| item.task_id == q4.id).unwrap();
        assert!(
            q2_item.start_at <= q4_item.start_at,
            "q2={} q4={}",
            q2_item.start_at,
            q4_item.start_at
        );
    }

    #[test]
    fn pending_quadrant_is_unscheduled() {
        let temp = TempRoot::new("pending");
        let mut task = task(&temp, "待评估");
        task.estimated_minutes = Some(60);
        let views = vec![view(&task, QUADRANT_PENDING, URGENCY_PENDING, "待评估")];
        let result = run(&temp, &[task], &views, &[]);
        assert!(result.suggestions.is_empty());
        assert!(result.unscheduled[0].reason.contains("待评估"));
    }

    #[test]
    fn suggestion_never_overlaps_confirmed_plan() {
        let temp = TempRoot::new("no-overlap");
        let mut task = task(&temp, "任务");
        task.estimated_minutes = Some(60);
        let views = vec![view(&task, QUADRANT_Q1, "高", "高")];
        let mut config = availability();
        config.daily_capacity_hours = 8.0;
        let fixed = vec![entry(
            &task.id,
            &format!("{}T10:00:00Z", first_day()),
            &format!("{}T11:00:00Z", first_day()),
            STATUS_CONFIRMED,
            true,
        )];
        let result = generate_suggestions(&[task], &views, &config, &fixed, NOW);
        assert_eq!(result.suggestions.len(), 1);
        assert!(!result.suggestions[0]
            .start_at
            .starts_with(&format!("{}T10", first_day())));
    }
}
