//! 随手记链路切换到记录库（OD-WR-310）。
//!
//! 判定块协议、IPC 参数与回执形状不变；写入对象从周文件换到 work/工作记录/。
//! 周索引按记录的 record_date 所属 ISO 周聚合；周文件的旧读函数仍保留给迁移。

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::worklog::{
    self, AppendReceipt, DestinationReceipt, MergeReceipt, MovedEntry, MoveBatchReceipt,
    MoveRequest, RedateReceipt, DEFAULT_DESTINATION, DESTINATIONS,
};
use crate::worklog_records::{
    self, RecordDraft, RecordFilter, RecordPatch, WorkRecord,
};

#[derive(Debug, Clone, Serialize)]
pub struct WeekEntry {
    pub record_id: String,
    pub date: String,
    pub created_at: String,
    pub text: String,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordWeek {
    pub week: String,
    pub entries: usize,
    pub updated_at: String,
}

fn date_days(date: &str) -> Result<i64, String> {
    if !worklog_records::is_date_shape(date) {
        return Err(format!("日期格式不正确（应为 YYYY-MM-DD）：{date}"));
    }
    let year: i64 = date[0..4].parse().unwrap_or(0);
    let month: u32 = date[5..7].parse().unwrap_or(1);
    let day: u32 = date[8..10].parse().unwrap_or(1);
    Ok(worklog::days_from_civil(year, month, day))
}

pub fn week_of_date(date: &str) -> Result<String, String> {
    let (year, week) = worklog::iso_week_of_days(date_days(date)?);
    Ok(format!("{year:04}-W{week:02}"))
}

pub fn week_entries(root: &Path, week: &str) -> Result<Vec<WeekEntry>, String> {
    worklog::assert_week_label(week)?;
    let listing = worklog_records::list_records(root, &RecordFilter::default())?;
    let mut entries: Vec<WeekEntry> = listing
        .items
        .into_iter()
        .filter(|record| week_of_date(&record.record_date).map(|w| w == week).unwrap_or(false))
        .map(|record| WeekEntry {
            record_id: record.id,
            date: record.record_date,
            created_at: record.created_at,
            text: record.body,
            destination: record.destination,
        })
        .collect();
    entries.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
    Ok(entries)
}

pub fn list_record_weeks(root: &Path) -> Result<Vec<RecordWeek>, String> {
    let listing = worklog_records::list_records(root, &RecordFilter::default())?;
    let mut weeks: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for record in listing.items {
        let Ok(week) = week_of_date(&record.record_date) else {
            continue;
        };
        let entry = weeks.entry(week).or_insert((0, String::new()));
        entry.0 += 1;
        if record.updated_at > entry.1 {
            entry.1 = record.updated_at;
        }
    }
    Ok(weeks
        .into_iter()
        .map(|(week, (entries, updated_at))| RecordWeek {
            week,
            entries,
            updated_at,
        })
        .collect())
}

fn locate(
    root: &Path,
    week: &str,
    date: &str,
    index: usize,
    expect_text: &str,
) -> Result<(WeekEntry, WorkRecord), String> {
    worklog::assert_week_label(week)?;
    if !worklog_records::is_date_shape(date) {
        return Err(format!("日期格式不正确（应为 YYYY-MM-DD）：{date}"));
    }
    let entries = week_entries(root, week)?;
    let same_day: Vec<&WeekEntry> = entries.iter().filter(|entry| entry.date == date).collect();
    let entry = same_day
        .get(index)
        .ok_or_else(|| format!("{date} 的第 {} 条不存在", index + 1))?;
    if entry.text.trim() != expect_text.trim() {
        return Err(format!("{date} 第 {} 条的正文与界面带回的不一致，未写入", index + 1));
    }
    let record = worklog_records::read_record(root, &entry.record_id)?;
    Ok(((*entry).clone(), record))
}

pub fn append(
    root: &Path,
    date: &str,
    text: &str,
    destination: Option<&str>,
    now: f64,
    offset_minutes: i32,
) -> Result<AppendReceipt, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("记录内容为空，未写入".to_string());
    }
    let week = worklog::week_label(now, offset_minutes);
    let today = worklog::date_label(now, offset_minutes);
    worklog::assert_entry_date(&week, date, &today)?;
    let dest = destination
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_DESTINATION);
    let dest = if DESTINATIONS.contains(&dest) {
        dest.to_string()
    } else {
        DEFAULT_DESTINATION.to_string()
    };
    let index = week_entries(root, &week)?
        .iter()
        .filter(|entry| entry.date == date)
        .count();
    let draft = RecordDraft {
        id: None,
        title: None,
        record_date: Some(date.to_string()),
        category: None,
        tags: Vec::new(),
        source: Some("随手记".to_string()),
        destination: Some(dest.clone()),
        draft: false,
        body: text.to_string(),
        linked_task_ids: Vec::new(),
    };
    let record = worklog_records::create_record(root, &draft, now, offset_minutes)?;
    Ok(AppendReceipt {
        ok: true,
        path: worklog_records::record_path(root, &record.id)?
            .display()
            .to_string(),
        week,
        date: date.to_string(),
        appended: true,
        was_new_file: false,
        continued: index > 0,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
        entry_index: index,
        entry_text: text.to_string(),
        error: None,
    })
}

pub fn set_destination(
    root: &Path,
    week: &str,
    date: &str,
    index: usize,
    destination: &str,
    expect_text: &str,
    now: f64,
) -> Result<DestinationReceipt, String> {
    let dest = destination.trim();
    if !DESTINATIONS.contains(&dest) {
        return Err(format!("去向不认识：{destination:?}"));
    }
    let (_entry, record) = locate(root, week, date, index, expect_text)?;
    let previous = record.destination.clone();
    let updated = worklog_records::update_record(
        root,
        &record.id,
        &RecordPatch {
            destination: Some(dest.to_string()),
            ..Default::default()
        },
        now,
    )?;
    Ok(DestinationReceipt {
        ok: true,
        path: worklog_records::record_path(root, &updated.id)?
            .display()
            .to_string(),
        week: week.to_string(),
        date: date.to_string(),
        index,
        destination: dest.to_string(),
        previous,
        backup: None,
        error: None,
    })
}

fn new_index_on(root: &Path, week: &str, date: &str, record_id: &str) -> Result<usize, String> {
    let entries = week_entries(root, week)?;
    Ok(entries
        .iter()
        .filter(|entry| entry.date == date)
        .position(|entry| entry.record_id == record_id)
        .unwrap_or(0))
}

pub fn redate(
    root: &Path,
    week: &str,
    from_date: &str,
    index: usize,
    to_date: &str,
    expect_text: &str,
    now: f64,
    offset_minutes: i32,
) -> Result<RedateReceipt, String> {
    let today = worklog::date_label(now, offset_minutes);
    let (_entry, record) = locate(root, week, from_date, index, expect_text)?;
    if from_date == to_date {
        return Err(format!("原日期与目标日期都是 {from_date}，无需移动"));
    }
    worklog::assert_entry_date(week, to_date, &today)?;
    let updated = worklog_records::update_record(
        root,
        &record.id,
        &RecordPatch {
            record_date: Some(to_date.to_string()),
            ..Default::default()
        },
        now,
    )?;
    let new_index = new_index_on(root, week, to_date, &updated.id)?;
    Ok(RedateReceipt {
        ok: true,
        path: worklog_records::record_path(root, &updated.id)?
            .display()
            .to_string(),
        week: week.to_string(),
        from_date: from_date.to_string(),
        to_date: to_date.to_string(),
        index,
        new_index,
        backup: None,
        error: None,
    })
}

pub fn move_many(
    root: &Path,
    week: &str,
    moves: &[MoveRequest],
    today: &str,
    now: f64,
) -> Result<MoveBatchReceipt, String> {
    worklog::assert_week_label(week)?;
    if moves.is_empty() {
        return Err("没有要移动的记录".to_string());
    }
    struct Located {
        request: MoveRequest,
        entry: WeekEntry,
        record: WorkRecord,
    }
    let mut located: Vec<Located> = Vec::new();
    let mut seen: BTreeMap<(String, usize), ()> = BTreeMap::new();
    for request in moves {
        if !worklog_records::is_date_shape(&request.date)
            || !worklog_records::is_date_shape(&request.to)
        {
            return Err("日期格式不正确（应为 YYYY-MM-DD），已拒绝移动".to_string());
        }
        if request.date == request.to {
            return Err(format!(
                "原日期与目标日期都是 {}，无需移动",
                request.date
            ));
        }
        if !seen.insert((request.date.clone(), request.index), ()).is_none() {
            return Err(format!(
                "{} 的第 {} 条被要求移动两次，已拒绝",
                request.date,
                request.index + 1
            ));
        }
        if week_of_date(&request.date)? != week {
            return Err(format!(
                "原日期 {} 不属于 {week}，未改动任何记录。",
                request.date
            ));
        }
        if week_of_date(&request.to)? != week {
            return Err(format!(
                "目标日期 {} 不属于 {week}；跨周移动暂不支持，未改动任何记录。",
                request.to
            ));
        }
        worklog::assert_entry_date(week, &request.to, today)?;
        let (entry, record) =
            locate(root, week, &request.date, request.index, &request.expect_text)?;
        located.push(Located {
            request: request.clone(),
            entry,
            record,
        });
    }

    let snapshots: Vec<WorkRecord> = located.iter().map(|item| item.record.clone()).collect();
    let mut applied: Vec<usize> = Vec::new();
    for (index, item) in located.iter().enumerate() {
        match worklog_records::update_record(
            root,
            &item.record.id,
            &RecordPatch {
                record_date: Some(item.request.to.clone()),
                ..Default::default()
            },
            now,
        ) {
            Ok(_) => applied.push(index),
            Err(message) => {
                for done in applied.iter().rev() {
                    let old = &snapshots[*done];
                    let _ = worklog_records::update_record(
                        root,
                        &old.id,
                        &RecordPatch {
                            record_date: Some(old.record_date.clone()),
                            ..Default::default()
                        },
                        now,
                    );
                }
                return Err(format!("移动失败：{message}；已回滚本批改动"));
            }
        }
    }

    let mut results: Vec<MovedEntry> = Vec::new();
    for item in &located {
        results.push(MovedEntry {
            from_date: item.request.date.clone(),
            to_date: item.request.to.clone(),
            index: item.request.index,
            new_index: new_index_on(root, week, &item.request.to, &item.record.id)?,
            text: item.entry.text.clone(),
        });
    }
    Ok(MoveBatchReceipt {
        ok: true,
        path: worklog_records::records_dir(root).display().to_string(),
        week: week.to_string(),
        moves: results,
        backup: None,
        error: None,
    })
}

pub fn merge(
    root: &Path,
    week: &str,
    date: &str,
    index: usize,
    expect_text: &str,
    addition: &str,
    now: f64,
) -> Result<MergeReceipt, String> {
    let addition = addition.trim();
    if addition.is_empty() {
        return Err("补充内容为空，未写入".to_string());
    }
    if addition.contains('\n') || addition.contains('\r') {
        return Err("补充内容只能是一行，已拒绝并入".to_string());
    }
    let (_entry, record) = locate(root, week, date, index, expect_text)?;
    let body = record.body.trim_end().to_string();
    let merged = if body.ends_with('；') || body.ends_with(';') {
        format!("{body}{addition}")
    } else {
        format!("{body}；{addition}")
    };
    if merged == body {
        return Err("补充内容与原句重复，未写入".to_string());
    }
    let updated = worklog_records::update_record(
        root,
        &record.id,
        &RecordPatch {
            body: Some(merged.clone()),
            ..Default::default()
        },
        now,
    )?;
    Ok(MergeReceipt {
        ok: true,
        path: worklog_records::record_path(root, &updated.id)?
            .display()
            .to_string(),
        week: week.to_string(),
        date: date.to_string(),
        index,
        entry_text: merged,
        added: addition.to_string(),
        merged: true,
        masked: false,
        masked_id_card: 0,
        masked_phone: 0,
        backup: None,
        error: None,
    })
}

/// 把一周记录渲染成旧周文件格式；周索引与后续周报可以继续按同一解析口径读取。
pub fn render_week_markdown(root: &Path, week: &str) -> Result<String, String> {
    let entries = week_entries(root, week)?;
    if entries.is_empty() {
        return Ok(String::new());
    }
    let mut out = format!("# {week}\n\n");
    let mut last_date: Option<&str> = None;
    for entry in &entries {
        if last_date == Some(entry.date.as_str()) {
            out.push_str(&format!("## {}（续）\n\n", entry.date));
        } else {
            out.push_str(&format!("## {}\n\n", entry.date));
        }
        out.push_str(&format!("- **事**：{}\n", entry.text.trim()));
        if let Some(destination) = entry.destination.as_deref() {
            out.push_str(&format!("  **去向**：{destination}\n"));
        }
        out.push('\n');
        last_date = Some(entry.date.as_str());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    const OFFSET: i32 = 480;

    fn noon(date: &str) -> f64 {
        date_days(date).unwrap() as f64 * 86_400.0 + 12.0 * 3_600.0
    }

    struct TempRoot {
        path: std::path::PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wr310-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
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

    #[test]
    fn append_writes_record_with_destination_and_index() {
        let temp = TempRoot::new("append");
        let now = noon("2026-11-03");
        let first = append(temp.root(), "2026-11-02", "第一件事", None, now, OFFSET).unwrap();
        assert_eq!(first.entry_index, 0);
        assert_eq!(first.date, "2026-11-02");
        assert_eq!(first.week, week_of_date("2026-11-02").unwrap());
        let second = append(
            temp.root(),
            "2026-11-02",
            "第二件事",
            Some("仅记录"),
            now + 60.0,
            OFFSET,
        )
        .unwrap();
        assert_eq!(second.entry_index, 1);
        let record = worklog_records::read_record(temp.root(), &first.path.split('/').last().unwrap().trim_end_matches(".md")).unwrap();
        assert_eq!(record.destination.as_deref(), Some(DEFAULT_DESTINATION));
        assert_eq!(record.source.as_deref(), Some("随手记"));
    }

    #[test]
    fn set_destination_requires_matching_text() {
        let temp = TempRoot::new("dest");
        let now = noon("2026-11-03");
        append(temp.root(), "2026-11-02", "第一件事", None, now, OFFSET).unwrap();
        let week = week_of_date("2026-11-02").unwrap();
        let err = set_destination(
            temp.root(),
            &week,
            "2026-11-02",
            0,
            "仅记录",
            "对不上",
            now,
        )
        .unwrap_err();
        assert!(err.contains("不一致"), "{err}");
        let receipt = set_destination(
            temp.root(),
            &week,
            "2026-11-02",
            0,
            "仅记录",
            "第一件事",
            now,
        )
        .unwrap();
        assert_eq!(receipt.previous.as_deref(), Some(DEFAULT_DESTINATION));
        assert_eq!(receipt.destination, "仅记录");
    }

    #[test]
    fn redate_moves_within_week_and_reports_new_index() {
        let temp = TempRoot::new("redate");
        let now = noon("2026-11-05");
        append(temp.root(), "2026-11-02", "第一件事", None, now, OFFSET).unwrap();
        append(
            temp.root(),
            "2026-11-03",
            "已有的",
            None,
            now + 60.0,
            OFFSET,
        )
        .unwrap();
        let week = week_of_date("2026-11-02").unwrap();
        let receipt = redate(
            temp.root(),
            &week,
            "2026-11-02",
            0,
            "2026-11-03",
            "第一件事",
            now,
            OFFSET,
        )
        .unwrap();
        assert_eq!(receipt.new_index, 0, "created_at 更早的记录排前面");
        let entries = week_entries(temp.root(), &week).unwrap();
        assert_eq!(entries.len(), 2);
        let cross = redate(
            temp.root(),
            &week,
            "2026-11-03",
            0,
            "2026-11-09",
            "第一件事",
            now,
            OFFSET,
        )
        .unwrap_err();
        assert!(cross.contains("不属于") || cross.contains("补录日期"), "{cross}");
    }

    #[test]
    fn merge_appends_single_line_without_overwriting() {
        let temp = TempRoot::new("merge");
        let now = noon("2026-11-03");
        append(temp.root(), "2026-11-02", "第一件事", None, now, OFFSET).unwrap();
        let week = week_of_date("2026-11-02").unwrap();
        let receipt = merge(
            temp.root(),
            &week,
            "2026-11-02",
            0,
            "第一件事",
            "补充一句",
            now,
        )
        .unwrap();
        assert_eq!(receipt.entry_text, "第一件事；补充一句");
        let entries = week_entries(temp.root(), &week).unwrap();
        assert_eq!(entries[0].text, "第一件事；补充一句");
        let err = merge(
            temp.root(),
            &week,
            "2026-11-02",
            0,
            "第一件事；补充一句",
            "换行\n内容",
            now,
        )
        .unwrap_err();
        assert!(err.contains("只能是一行"), "{err}");
    }

    #[test]
    fn move_many_rolls_back_when_one_request_mismatches() {
        let temp = TempRoot::new("move");
        let now = noon("2026-11-05");
        append(temp.root(), "2026-11-02", "甲", None, now, OFFSET).unwrap();
        append(temp.root(), "2026-11-03", "乙", None, now, OFFSET).unwrap();
        let week = week_of_date("2026-11-02").unwrap();
        let err = move_many(
            temp.root(),
            &week,
            &[
                MoveRequest {
                    date: "2026-11-02".to_string(),
                    index: 0,
                    to: "2026-11-04".to_string(),
                    expect_text: "甲".to_string(),
                },
                MoveRequest {
                    date: "2026-11-03".to_string(),
                    index: 0,
                    to: "2026-11-04".to_string(),
                    expect_text: "对不上".to_string(),
                },
            ],
            "2026-11-05",
            now,
        )
        .unwrap_err();
        assert!(err.contains("不一致"), "{err}");
        let entries = week_entries(temp.root(), &week).unwrap();
        assert_eq!(entries.iter().filter(|entry| entry.date == "2026-11-04").count(), 0);
    }

    #[test]
    fn week_markdown_matches_legacy_shape() {
        let temp = TempRoot::new("render");
        let now = noon("2026-11-05");
        append(temp.root(), "2026-11-02", "甲", None, now, OFFSET).unwrap();
        append(temp.root(), "2026-11-02", "乙", Some("仅记录"), now + 60.0, OFFSET).unwrap();
        let week = week_of_date("2026-11-02").unwrap();
        let markdown = render_week_markdown(temp.root(), &week).unwrap();
        assert!(markdown.starts_with("# 2026-W45"), "{markdown}");
        assert!(markdown.contains("- **事**：甲"));
        assert!(markdown.contains("- **事**：乙"));
        assert!(markdown.contains("**去向**：仅记录"));
        assert!(markdown.contains("## 2026-11-02（续）"));
    }

    #[test]
    fn list_record_weeks_aggregates_by_iso_week() {
        let temp = TempRoot::new("weeks");
        let now = noon("2026-11-05");
        append(temp.root(), "2026-11-02", "甲", None, now, OFFSET).unwrap();
        append(temp.root(), "2026-11-03", "乙", None, now, OFFSET).unwrap();
        let weeks = list_record_weeks(temp.root()).unwrap();
        assert_eq!(weeks.len(), 1);
        assert_eq!(weeks[0].week, week_of_date("2026-11-02").unwrap());
        assert_eq!(weeks[0].entries, 2);
    }
}
