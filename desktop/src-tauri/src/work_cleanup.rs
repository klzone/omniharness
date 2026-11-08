//! 临时层清理（OD-WR-313）。
//!
//! 候选规则（D-068）：
//!   * 记录：落在已归档报告来源周期内；
//!   * 任务：已完成或已取消，且超过 retention 天数；
//!   * 永不入选：draft 记录、被未完成任务引用的记录、还有待审核候选的记录。
//! 预览零写入；apply 只移动到 work/.trash/，不直接删除。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::task_store;
use crate::vault_archive::frontmatter_value;
use crate::worklog;
use crate::worklog_records;

pub const RETENTION_FILE: &str = "retention.v1.json";
pub const RETENTION_SCHEMA: &str = "omnidesk.work-retention.v1";
pub const DEFAULT_RETENTION_DAYS: i64 = 90;
pub const REPORT_ARCHIVE_DIR: &str = "报告";

#[derive(Debug, Clone, Serialize)]
pub struct CleanupItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub date: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupPreview {
    pub retention_days: i64,
    pub records: Vec<CleanupItem>,
    pub tasks: Vec<CleanupItem>,
    pub archived_periods: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupMoved {
    pub kind: String,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub ok: bool,
    pub moved: Vec<CleanupMoved>,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionFile {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    days: Option<i64>,
}

fn retention_path(root: &Path) -> PathBuf {
    root.join(RETENTION_FILE)
}

/// 读取 work/retention.v1.json；文件不存在时用默认 90 天。
pub fn load_retention(root: &Path) -> Result<i64, String> {
    let path = retention_path(root);
    if !path.exists() {
        return Ok(DEFAULT_RETENTION_DAYS);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取保留期配置失败（{}）：{}", path.display(), e))?;
    let parsed: RetentionFile = serde_json::from_str(&raw)
        .map_err(|e| format!("保留期配置格式错误（{}）：{}", path.display(), e))?;
    if let Some(schema) = parsed.schema.as_deref() {
        if schema != RETENTION_SCHEMA {
            return Err(format!("保留期配置 schema 不认识：{schema}"));
        }
    }
    let days = parsed.days.unwrap_or(DEFAULT_RETENTION_DAYS);
    if !(1..=3650).contains(&days) {
        return Err(format!("保留期天数必须在 1 到 3650 之间：{days}"));
    }
    Ok(days)
}

fn date_days(date: &str) -> Option<i64> {
    if !worklog_records::is_date_shape(date) {
        return None;
    }
    let year: i64 = date[0..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..10].parse().ok()?;
    Some(worklog::days_from_civil(year, month, day))
}

fn iso_seconds(ts: &str) -> Option<i64> {
    let date = ts.get(0..10)?;
    let time = ts.get(11..19)?;
    let days = date_days(date)?;
    let hour: i64 = time[0..2].parse().ok()?;
    let minute: i64 = time[3..5].parse().ok()?;
    let second: i64 = time[6..8].parse().ok()?;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// 周期 → 闭区间天数：YYYY-Www / YYYY-MM / YYYY。
fn period_range(period: &str) -> Option<(i64, i64)> {
    let text = period.trim();
    if let Some((year, week)) = text.split_once("-W") {
        let year: i64 = year.parse().ok()?;
        let week: i64 = week.parse().ok()?;
        if !(1..=53).contains(&week) {
            return None;
        }
        let jan4 = worklog::days_from_civil(year, 1, 4);
        let weekday = (jan4 + 3).rem_euclid(7) + 1;
        let start = jan4 - (weekday - 1) + (week - 1) * 7;
        return Some((start, start + 6));
    }
    if text.len() == 7 && text.as_bytes()[4] == b'-' {
        let year: i64 = text[0..4].parse().ok()?;
        let month: u32 = text[5..7].parse().ok()?;
        if !(1..=12).contains(&month) {
            return None;
        }
        let start = worklog::days_from_civil(year, month, 1);
        let end = if month == 12 {
            worklog::days_from_civil(year + 1, 1, 1) - 1
        } else {
            worklog::days_from_civil(year, month + 1, 1) - 1
        };
        return Some((start, end));
    }
    if text.len() == 4 {
        let year: i64 = text.parse().ok()?;
        return Some((
            worklog::days_from_civil(year, 1, 1),
            worklog::days_from_civil(year + 1, 1, 1) - 1,
        ));
    }
    None
}

fn archived_periods(
    vault_root: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Vec<(String, i64, i64)> {
    let Some(root) = vault_root else {
        warnings.push("没有配置知识库，按没有已归档报告处理。".to_string());
        return Vec::new();
    };
    let dir = root.join(REPORT_ARCHIVE_DIR);
    if !dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        warnings.push("读取知识库报告目录失败。".to_string());
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(period) = frontmatter_value(&content, "source_period") else {
            warnings.push(format!(
                "报告没有来源周期，未纳入清理：{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            continue;
        };
        match period_range(&period) {
            Some((start, end)) => out.push((period, start, end)),
            None => warnings.push(format!("报告来源周期无法解析：{period}")),
        }
    }
    out
}

fn pending_candidate_records(root: &Path) -> Result<BTreeSet<String>, String> {
    Ok(task_store::list_candidates(root)?
        .items
        .into_iter()
        .map(|candidate| candidate.record_id)
        .collect())
}

fn unfinished_task_ids(root: &Path) -> Result<BTreeSet<String>, String> {
    Ok(task_store::unfinished_tasks(root)?
        .into_iter()
        .map(|task| task.id)
        .collect())
}

/// 只读预览：不创建目录、不移动文件、不写保留期配置。
pub fn cleanup_preview(
    root: &Path,
    vault_root: Option<&Path>,
    now: f64,
) -> Result<CleanupPreview, String> {
    worklog_records::assert_work_root(root)?;
    let retention_days = load_retention(root)?;
    let mut warnings = Vec::new();
    let periods = archived_periods(vault_root, &mut warnings);
    let pending = pending_candidate_records(root)?;
    let unfinished = unfinished_task_ids(root)?;

    let records_listing =
        worklog_records::list_records(root, &worklog_records::RecordFilter::default())?;
    let mut records = Vec::new();
    for record in records_listing.items {
        if record.draft {
            continue;
        }
        if pending.contains(&record.id) {
            continue;
        }
        if record
            .linked_task_ids
            .iter()
            .any(|task_id| unfinished.contains(task_id))
        {
            continue;
        }
        let Some(record_day) = date_days(&record.record_date) else {
            continue;
        };
        let Some((period, _, _)) = periods
            .iter()
            .find(|(_, start, end)| record_day >= *start && record_day <= *end)
        else {
            continue;
        };
        records.push(CleanupItem {
            kind: "record".to_string(),
            id: record.id,
            title: record.title,
            date: record.record_date,
            reason: format!("已归档报告覆盖期：{period}"),
        });
    }

    let tasks_listing = task_store::list_tasks(root, &task_store::TaskFilter::default())?;
    let mut tasks = Vec::new();
    for task in tasks_listing.items {
        if task.state != task_store::STATE_DONE && task.state != task_store::STATE_CANCELLED {
            continue;
        }
        let Some(updated) = iso_seconds(&task.updated_at) else {
            warnings.push(format!("任务时间无法解析，未纳入清理：{}", task.id));
            continue;
        };
        if now.floor() as i64 - updated < retention_days * 86_400 {
            continue;
        }
        tasks.push(CleanupItem {
            kind: "task".to_string(),
            id: task.id,
            title: task.title,
            date: task.updated_at,
            reason: format!("{} 超过 {retention_days} 天", task.state),
        });
    }

    Ok(CleanupPreview {
        retention_days,
        records,
        tasks,
        archived_periods: periods.into_iter().map(|(period, _, _)| period).collect(),
        warnings,
    })
}

/// 执行清理：重新计算候选，然后逐项移入 .trash/；不做直接删除。
pub fn cleanup_apply(
    root: &Path,
    vault_root: Option<&Path>,
    now: f64,
) -> Result<CleanupReport, String> {
    let preview = cleanup_preview(root, vault_root, now)?;
    let mut moved = Vec::new();
    let mut failures = Vec::new();
    for item in &preview.records {
        match worklog_records::delete_record(root, &item.id, now) {
            Ok(trash) => moved.push(CleanupMoved {
                kind: "record".to_string(),
                id: item.id.clone(),
                path: trash.path,
            }),
            Err(message) => failures.push(format!("记录 {}：{message}", item.id)),
        }
    }
    for item in &preview.tasks {
        match task_store::delete_task(root, &item.id, now) {
            Ok(path) => moved.push(CleanupMoved {
                kind: "task".to_string(),
                id: item.id.clone(),
                path,
            }),
            Err(message) => failures.push(format!("任务 {}：{message}", item.id)),
        }
    }
    Ok(CleanupReport {
        ok: failures.is_empty(),
        moved,
        failures,
        warnings: preview.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklog_records::{RecordDraft, RecordPatch};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_300_000.0;
    const OFFSET: i32 = 480;

    struct Temp {
        work: PathBuf,
        vault: PathBuf,
    }

    impl Temp {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-wr313-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let work = base.join("work");
            let vault = base.join("vault");
            fs::create_dir_all(&work).unwrap();
            fs::create_dir_all(vault.join(REPORT_ARCHIVE_DIR)).unwrap();
            Self { work, vault }
        }

        fn add_report(&self, name: &str, period: &str) {
            let content = format!(
                "---\nsource_period: \"{period}\"\nsource_sha256: \"x\"\n---\n\n报告"
            );
            fs::write(self.vault.join(REPORT_ARCHIVE_DIR).join(name), content).unwrap();
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.work.parent().unwrap());
        }
    }

    fn record(temp: &Temp, date: &str, title: &str, draft: bool) -> worklog_records::WorkRecord {
        worklog_records::create_record(
            &temp.work,
            &RecordDraft {
                title: Some(title.to_string()),
                record_date: Some(date.to_string()),
                draft,
                body: "合成记录".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap()
    }

    #[test]
    fn retention_defaults_and_config_round_trip() {
        let temp = Temp::new("retention");
        assert_eq!(load_retention(&temp.work).unwrap(), 90);
        fs::write(
            temp.work.join(RETENTION_FILE),
            "{\"schema\":\"omnidesk.work-retention.v1\",\"days\":30}",
        )
        .unwrap();
        assert_eq!(load_retention(&temp.work).unwrap(), 30);
        fs::write(temp.work.join(RETENTION_FILE), "{\"days\":0}").unwrap();
        assert!(load_retention(&temp.work).is_err());
    }

    #[test]
    fn preview_selects_records_in_archived_period_only() {
        let temp = Temp::new("preview");
        temp.add_report("2026-W45-周报.md", "2026-W45");
        let inside = record(&temp, "2026-11-02", "周期内", false);
        let outside = record(&temp, "2026-11-10", "周期外", false);
        let draft = record(&temp, "2026-11-03", "草稿", true);
        let preview = cleanup_preview(&temp.work, Some(&temp.vault), NOW).unwrap();
        assert_eq!(preview.records.len(), 1);
        assert_eq!(preview.records[0].id, inside.id);
        assert!(!preview.records.iter().any(|item| item.id == outside.id));
        assert!(!preview.records.iter().any(|item| item.id == draft.id));
        assert!(!temp.work.join(".trash").exists(), "预览不得写盘");
    }

    #[test]
    fn preview_protects_unfinished_references_and_pending_candidates() {
        let temp = Temp::new("protect");
        temp.add_report("2026-11-月报.md", "2026-11");
        let linked = record(&temp, "2026-11-02", "被未完成任务引用", false);
        let task = task_store::create_task(
            &temp.work,
            &task_store::TaskDraft {
                title: "进行中的任务".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        task_store::transition_task(
            &temp.work,
            &task.id,
            task_store::STATE_IN_PROGRESS,
            None,
            NOW,
        )
        .unwrap();
        worklog_records::update_record(
            &temp.work,
            &linked.id,
            &RecordPatch {
                linked_task_ids: Some(vec![task.id.clone()]),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let pending = record(&temp, "2026-11-03", "有待审核候选", false);
        task_store::stage_candidates(
            &temp.work,
            &pending.id,
            &[task_store::CandidateInput {
                title: "候选".to_string(),
                source_quote: "合成记录".to_string(),
                ..Default::default()
            }],
            NOW,
        )
        .unwrap();
        let preview = cleanup_preview(&temp.work, Some(&temp.vault), NOW).unwrap();
        assert!(preview.records.is_empty(), "{:?}", preview.records);
    }

    #[test]
    fn preview_selects_old_completed_tasks_only() {
        let temp = Temp::new("tasks");
        let old = task_store::create_task(
            &temp.work,
            &task_store::TaskDraft {
                title: "旧任务".to_string(),
                ..Default::default()
            },
            NOW - 120.0 * 86_400.0,
        )
        .unwrap();
        task_store::transition_task(
            &temp.work,
            &old.id,
            task_store::STATE_IN_PROGRESS,
            None,
            NOW - 120.0 * 86_400.0,
        )
        .unwrap();
        task_store::transition_task(
            &temp.work,
            &old.id,
            task_store::STATE_DONE,
            None,
            NOW - 120.0 * 86_400.0,
        )
        .unwrap();
        let recent = task_store::create_task(
            &temp.work,
            &task_store::TaskDraft {
                title: "新任务".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        task_store::transition_task(
            &temp.work,
            &recent.id,
            task_store::STATE_CANCELLED,
            None,
            NOW,
        )
        .unwrap();
        let preview = cleanup_preview(&temp.work, Some(&temp.vault), NOW).unwrap();
        assert_eq!(preview.tasks.len(), 1);
        assert_eq!(preview.tasks[0].id, old.id);
    }

    #[test]
    fn apply_moves_candidates_to_trash() {
        let temp = Temp::new("apply");
        temp.add_report("2026-W45-周报.md", "2026-W45");
        let kept = record(&temp, "2026-11-09", "保留", false);
        let doomed = record(&temp, "2026-11-02", "移入回收", false);
        let report = cleanup_apply(&temp.work, Some(&temp.vault), NOW).unwrap();
        assert!(report.ok);
        assert_eq!(report.moved.len(), 1);
        assert_eq!(report.moved[0].id, doomed.id);
        assert!(report.moved[0].path.contains(".trash"));
        let listing = worklog_records::list_records(
            &temp.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, kept.id);
    }

    #[test]
    fn month_and_year_periods_parse() {
        assert!(period_range("2026-11").is_some());
        assert!(period_range("2026").is_some());
        assert!(period_range("2026-W45").is_some());
        assert!(period_range("随便写").is_none());
    }
}
