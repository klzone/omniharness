//! 排期实体（OD-QS-407）。
//!
//! 每个排期条目一个 Markdown 文件：<work_root>/排期/<entry_id>.md。
//! frontmatter 存条目字段，正文只有 `## 调整记录` 追加段；调整记录只追加，不改写。
//! 排期状态与任务状态解耦（D-071）：任务完成/取消后，未来的条目在读取时派生
//! 「已失效」，文件不改写。
//! 时间统一存 ISO 8601 UTC（YYYY-MM-DDTHH:MM:SSZ），排序与重叠判断不依赖本地时区。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::task_store::{self, Task};
use crate::worklog::{civil_from_days, days_from_civil};
use crate::worklog_records::{
    assert_work_root, is_date_shape, is_safe_id, new_file_id, normalize_opt, split_frontmatter,
    write_new_atomically, write_replace_atomically, yaml_opt, yaml_scalar,
};

pub const SCHEDULE_SCHEMA: &str = "omnidesk.schedule.v1";
pub const SCHEDULE_DIR: &str = "排期";
pub const GENERATION_FILE: &str = "generation.v1.json";
pub const GENERATION_SCHEMA: &str = "omnidesk.schedule-generation.v1";

pub const STATUS_SUGGESTED: &str = "已建议";
pub const STATUS_CONFIRMED: &str = "已确认";
pub const STATUS_ADJUSTED: &str = "已调整";
pub const STATUS_CANCELLED: &str = "已取消";
pub const STATUS_DONE: &str = "已完成";
/// 只由读取派生，不落盘。
pub const STATUS_INVALID: &str = "已失效";
pub const ENTRY_STATUSES: &[&str] = &[
    STATUS_SUGGESTED,
    STATUS_CONFIRMED,
    STATUS_ADJUSTED,
    STATUS_CANCELLED,
    STATUS_DONE,
];

const MAX_DURATION_MINUTES: i64 = 1440;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdjustmentRecord {
    pub at: String,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub schema: String,
    pub id: String,
    pub task_id: String,
    /// ISO 8601 UTC，例如 2026-11-09T01:00:00Z。
    pub start_at: String,
    pub end_at: String,
    pub duration: u32,
    pub status: String,
    pub is_locked: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub adjustment_reason: Option<String>,
    /// 不落 frontmatter，写在正文追加段；IPC 仍返回给界面。
    #[serde(default)]
    pub adjustments: Vec<AdjustmentRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleDraft {
    pub id: Option<String>,
    pub task_id: String,
    pub start_at: String,
    pub end_at: String,
    /// 可空；给了就必须与起止时间算出的分钟数一致。
    pub duration: Option<u32>,
    pub is_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleEntryView {
    #[serde(flatten)]
    pub entry: ScheduleEntry,
    pub effective_status: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScheduleListing {
    pub items: Vec<ScheduleEntryView>,
    pub broken: usize,
    /// 最近一次生成时间；没有生成过为 null。
    pub generated_at: Option<String>,
    /// 任务在生成后又被改过，界面应提示重新生成。
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdjustmentReport {
    pub old: ScheduleEntry,
    pub new: ScheduleEntry,
}

pub fn schedule_dir(root: &Path) -> PathBuf {
    root.join(SCHEDULE_DIR)
}

fn generation_path(root: &Path) -> PathBuf {
    schedule_dir(root).join(GENERATION_FILE)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationFile {
    #[serde(default)]
    schema: Option<String>,
    generated_at: String,
}

/// 记录最近一次生成时间。界面用它判断任务是否在生成后又被改过。
pub fn mark_generated_at(root: &Path, now: f64) -> Result<String, String> {
    assert_work_root(root)?;
    fs::create_dir_all(schedule_dir(root))
        .map_err(|e| format!("创建排期目录失败：{e}"))?;
    let generated_at = crate::task_shell::utc_iso8601(now);
    let payload = serde_json::json!({
        "schema": GENERATION_SCHEMA,
        "generated_at": generated_at,
    });
    let target = generation_path(root);
    let temp = schedule_dir(root).join(format!("{GENERATION_FILE}.tmp-{}", std::process::id()));
    let content = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("序列化生成时间失败：{e}"))?;
    let result = (|| -> Result<(), String> {
        fs::write(&temp, content).map_err(|e| format!("写入生成时间失败：{e}"))?;
        fs::rename(&temp, &target).map_err(|e| format!("替换生成时间失败：{e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(generated_at)
}

pub fn generated_at(root: &Path) -> Result<Option<String>, String> {
    assert_work_root(root)?;
    let path = generation_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取生成时间失败（{}）：{}", path.display(), e))?;
    let parsed: GenerationFile = serde_json::from_str(&raw)
        .map_err(|e| format!("生成时间格式错误（{}）：{}", path.display(), e))?;
    if let Some(schema) = parsed.schema.as_deref() {
        if schema != GENERATION_SCHEMA {
            return Err(format!("生成时间 schema 不认识：{schema}"));
        }
    }
    Ok(Some(parsed.generated_at))
}

/// 任务列表里最大的 updated_at 晚于生成时间 → stale。
pub fn is_stale(root: &Path, tasks: &[Task]) -> Result<bool, String> {
    let generated = generated_at(root)?;
    let Some(generated) = generated else {
        return Ok(true);
    };
    let max_updated = tasks
        .iter()
        .map(|task| task.updated_at.as_str())
        .max()
        .unwrap_or("");
    Ok(!max_updated.is_empty() && max_updated > generated.as_str())
}

/// 任务已完成/取消后，把仍处于「已建议」的条目置为「已取消」并追加过期记录。
/// 锁定条目不动（锁定优先）。
pub fn expire_stale_suggestions(
    root: &Path,
    tasks: &[Task],
    now: f64,
) -> Result<Vec<String>, String> {
    assert_work_root(root)?;
    let states: HashMap<&str, &str> = tasks
        .iter()
        .map(|task| (task.id.as_str(), task.state.as_str()))
        .collect();
    let mut expired = Vec::new();
    for mut entry in read_all(root)? {
        if entry.status != STATUS_SUGGESTED || entry.is_locked {
            continue;
        }
        let state = match states.get(entry.task_id.as_str()) {
            Some(state) => *state,
            None => continue,
        };
        if !matches!(state, task_store::STATE_DONE | task_store::STATE_CANCELLED) {
            continue;
        }
        let stamp = crate::task_shell::utc_iso8601(now);
        entry.status = STATUS_CANCELLED.to_string();
        entry.adjustment_reason = Some("任务已结束，建议过期".to_string());
        entry.adjustments.push(AdjustmentRecord {
            at: stamp.clone(),
            action: "过期".to_string(),
            reason: format!("任务状态：{state}"),
        });
        entry.updated_at = stamp;
        persist(root, &entry)?;
        expired.push(entry.id);
    }
    Ok(expired)
}

pub fn list_entries(root: &Path) -> Result<Vec<ScheduleEntry>, String> {
    assert_work_root(root)?;
    read_all(root)
}

fn entry_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) {
        return Err(format!("排期条目编号不合法：{id:?}"));
    }
    Ok(schedule_dir(root).join(format!("{id}.md")))
}

fn parse_timestamp(value: &str) -> Result<i64, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return Err(format!("排期时间格式应为 YYYY-MM-DDTHH:MM:SSZ：{value:?}"));
    }
    let date = &value[0..10];
    if !is_date_shape(date) {
        return Err(format!("排期时间里的日期不合法：{date}"));
    }
    let year: i64 = date[0..4].parse().map_err(|_| "排期日期不合法".to_string())?;
    let month: u32 = date[5..7].parse().map_err(|_| "排期日期不合法".to_string())?;
    let day: u32 = date[8..10].parse().map_err(|_| "排期日期不合法".to_string())?;
    let hour: i64 = value[11..13]
        .parse()
        .map_err(|_| format!("排期时间不合法：{value:?}"))?;
    let minute: i64 = value[14..16]
        .parse()
        .map_err(|_| format!("排期时间不合法：{value:?}"))?;
    let second: i64 = value[17..19]
        .parse()
        .map_err(|_| format!("排期时间不合法：{value:?}"))?;
    if bytes[13] != b':' || bytes[16] != b':' || hour > 23 || minute > 59 || second > 59 {
        return Err(format!("排期时间不合法：{value:?}"));
    }
    let days = days_from_civil(year, month, day);
    let (y, m, d) = civil_from_days(days);
    if (y, m, d) != (year, month, day) {
        return Err(format!("排期日期不合法：{date}"));
    }
    Ok(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn duration_between(start_at: &str, end_at: &str) -> Result<u32, String> {
    let start = parse_timestamp(start_at)?;
    let end = parse_timestamp(end_at)?;
    if end <= start {
        return Err("排期结束时间必须晚于开始时间。".to_string());
    }
    let seconds = end - start;
    if seconds % 60 != 0 {
        return Err("排期时长必须落在整分钟。".to_string());
    }
    let minutes = seconds / 60;
    if minutes > MAX_DURATION_MINUTES {
        return Err(format!("单个排期条目不能超过 {MAX_DURATION_MINUTES} 分钟。"));
    }
    Ok(minutes as u32)
}

fn is_active_status(status: &str) -> bool {
    matches!(status, STATUS_SUGGESTED | STATUS_CONFIRMED)
}

fn validate_status(status: &str) -> Result<(), String> {
    if !ENTRY_STATUSES.contains(&status) {
        return Err(format!("排期状态不认识：{status}"));
    }
    Ok(())
}

fn render_entry(entry: &ScheduleEntry) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(&entry.schema)));
    fm.push_str(&format!("id: {}\n", yaml_scalar(&entry.id)));
    fm.push_str(&format!("task_id: {}\n", yaml_scalar(&entry.task_id)));
    fm.push_str(&format!("start_at: {}\n", yaml_scalar(&entry.start_at)));
    fm.push_str(&format!("end_at: {}\n", yaml_scalar(&entry.end_at)));
    fm.push_str(&format!("duration: {}\n", entry.duration));
    fm.push_str(&format!("status: {}\n", yaml_scalar(&entry.status)));
    fm.push_str(&format!("is_locked: {}\n", entry.is_locked));
    fm.push_str(&format!("created_at: {}\n", yaml_scalar(&entry.created_at)));
    fm.push_str(&format!("updated_at: {}\n", yaml_scalar(&entry.updated_at)));
    fm.push_str(&format!(
        "adjustment_reason: {}\n",
        yaml_opt(&entry.adjustment_reason)
    ));
    let mut out = format!("---\n{fm}---\n\n## 调整记录\n\n");
    for record in &entry.adjustments {
        out.push_str(&format!(
            "- {} | {} | {}\n",
            record.at, record.action, record.reason
        ));
    }
    out
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFront {
    #[serde(default)]
    schema: Option<String>,
    id: String,
    task_id: String,
    start_at: String,
    end_at: String,
    duration: u32,
    status: String,
    #[serde(default)]
    is_locked: bool,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    adjustment_reason: Option<String>,
}

fn parse_adjustments(body: &str) -> Result<Vec<AdjustmentRecord>, String> {
    let mut records = Vec::new();
    let mut in_section = false;
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "## 调整记录" {
            in_section = true;
            continue;
        }
        if line.starts_with("## ") {
            return Err(format!("排期文件出现未知分区：{line}"));
        }
        if !in_section {
            return Err("排期内容不在 调整记录 分区里。".to_string());
        }
        let item = line
            .strip_prefix("- ")
            .ok_or_else(|| format!("调整记录里有无法识别的行：{line}"))?;
        let mut parts = item.splitn(3, '|');
        let at = parts
            .next()
            .ok_or_else(|| format!("调整记录缺少时间：{line}"))?
            .trim()
            .to_string();
        let action = parts
            .next()
            .ok_or_else(|| format!("调整记录缺少动作：{line}"))?
            .trim()
            .to_string();
        let reason = parts.next().map(|value| value.trim().to_string()).unwrap_or_default();
        if at.is_empty() || action.is_empty() {
            return Err(format!("调整记录不完整：{line}"));
        }
        records.push(AdjustmentRecord { at, action, reason });
    }
    if !in_section {
        return Err("排期文件缺少 调整记录 分区。".to_string());
    }
    Ok(records)
}

fn parse_entry(raw: &str) -> Result<ScheduleEntry, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: EntryFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("排期 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != SCHEDULE_SCHEMA {
            return Err(format!("排期 schema 不认识：{schema}"));
        }
    }
    if !is_safe_id(&front.id) {
        return Err(format!("排期条目编号不合法：{:?}", front.id));
    }
    if !is_safe_id(&front.task_id) {
        return Err(format!("排期任务编号不合法：{:?}", front.task_id));
    }
    validate_status(&front.status)?;
    let duration = duration_between(&front.start_at, &front.end_at)?;
    if duration != front.duration {
        return Err(format!(
            "排期时长与起止时间不一致：字段 {} vs 实际 {duration}",
            front.duration
        ));
    }
    let adjustments = parse_adjustments(&body)?;
    Ok(ScheduleEntry {
        schema: SCHEDULE_SCHEMA.to_string(),
        id: front.id,
        task_id: front.task_id,
        start_at: front.start_at,
        end_at: front.end_at,
        duration: front.duration,
        status: front.status,
        is_locked: front.is_locked,
        created_at: front.created_at,
        updated_at: front.updated_at,
        adjustment_reason: normalize_opt(front.adjustment_reason),
        adjustments,
    })
}

fn overlaps(first_start: i64, first_end: i64, second_start: i64, second_end: i64) -> bool {
    first_start < second_end && second_start < first_end
}

fn active_conflict(
    root: &Path,
    task_id: &str,
    start_at: &str,
    end_at: &str,
    exclude_id: Option<&str>,
) -> Result<Option<String>, String> {
    let start = parse_timestamp(start_at)?;
    let end = parse_timestamp(end_at)?;
    for entry in read_all(root)? {
        if Some(entry.id.as_str()) == exclude_id {
            continue;
        }
        if entry.task_id != task_id || !is_active_status(&entry.status) {
            continue;
        }
        let other_start = match parse_timestamp(&entry.start_at) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let other_end = match parse_timestamp(&entry.end_at) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if overlaps(start, end, other_start, other_end) {
            return Ok(Some(entry.id));
        }
    }
    Ok(None)
}

pub fn create_entry(root: &Path, draft: &ScheduleDraft, now: f64) -> Result<ScheduleEntry, String> {
    assert_work_root(root)?;
    let task_id = draft.task_id.trim().to_string();
    if !is_safe_id(&task_id) {
        return Err(format!("排期任务编号不合法：{task_id:?}"));
    }
    let task: Task = task_store::read_task(root, &task_id)?;
    if matches!(task.state.as_str(), task_store::STATE_DONE | task_store::STATE_CANCELLED) {
        return Err(format!("任务{}，不能再创建新的排期。", task.state));
    }
    let start_at = draft.start_at.trim().to_string();
    let end_at = draft.end_at.trim().to_string();
    let duration = duration_between(&start_at, &end_at)?;
    if let Some(given) = draft.duration {
        if given != duration {
            return Err(format!(
                "排期时长与起止时间不一致：字段 {given} vs 实际 {duration}"
            ));
        }
    }
    if let Some(conflict) = active_conflict(root, &task_id, &start_at, &end_at, None)? {
        return Err(format!(
            "同一任务在重叠时段已有活跃排期（{conflict}），不能重复排期。"
        ));
    }
    let id = match normalize_opt(draft.id.clone()) {
        Some(id) => {
            if !is_safe_id(&id) {
                return Err(format!("排期条目编号不合法：{id:?}"));
            }
            if entry_path(root, &id)?.exists() {
                return Err(format!("排期条目已存在：{id}"));
            }
            id
        }
        None => new_file_id("s", now),
    };
    let stamp = crate::task_shell::utc_iso8601(now);
    let entry = ScheduleEntry {
        schema: SCHEDULE_SCHEMA.to_string(),
        id: id.clone(),
        task_id,
        start_at,
        end_at,
        duration,
        status: STATUS_SUGGESTED.to_string(),
        is_locked: draft.is_locked.unwrap_or(false),
        created_at: stamp.clone(),
        updated_at: stamp,
        adjustment_reason: None,
        adjustments: Vec::new(),
    };
    fs::create_dir_all(schedule_dir(root))
        .map_err(|e| format!("创建排期目录失败：{e}"))?;
    write_new_atomically(&entry_path(root, &id)?, render_entry(&entry).as_bytes())?;
    Ok(entry)
}

pub fn read_entry(root: &Path, id: &str) -> Result<ScheduleEntry, String> {
    assert_work_root(root)?;
    let path = entry_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取排期失败（{}）：{}", path.display(), e))?;
    parse_entry(&raw)
}

fn read_all(root: &Path) -> Result<Vec<ScheduleEntry>, String> {
    let dir = schedule_dir(root);
    let mut items = Vec::new();
    if !dir.exists() {
        return Ok(items);
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取排期目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取排期目录项失败：{e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(parsed) = parse_entry(&raw) {
                items.push(parsed);
            }
        }
    }
    Ok(items)
}

fn persist(root: &Path, entry: &ScheduleEntry) -> Result<(), String> {
    write_replace_atomically(&entry_path(root, &entry.id)?, render_entry(entry).as_bytes())
}

pub fn confirm_entry(root: &Path, id: &str, now: f64) -> Result<ScheduleEntry, String> {
    let mut entry = read_entry(root, id)?;
    if entry.status == STATUS_CONFIRMED {
        return Err("排期已经是已确认状态。".to_string());
    }
    if entry.status != STATUS_SUGGESTED {
        return Err(format!("只有已建议的排期可以确认，当前是 {}。", entry.status));
    }
    entry.status = STATUS_CONFIRMED.to_string();
    entry.updated_at = crate::task_shell::utc_iso8601(now);
    persist(root, &entry)?;
    Ok(entry)
}

pub fn lock_entry(root: &Path, id: &str, now: f64) -> Result<ScheduleEntry, String> {
    let mut entry = read_entry(root, id)?;
    if !is_active_status(&entry.status) {
        return Err(format!("只有已建议或已确认的排期可以锁定，当前是 {}。", entry.status));
    }
    if entry.is_locked {
        return Err("排期已经锁定。".to_string());
    }
    entry.is_locked = true;
    entry.updated_at = crate::task_shell::utc_iso8601(now);
    persist(root, &entry)?;
    Ok(entry)
}

pub fn cancel_entry(
    root: &Path,
    id: &str,
    reason: Option<&str>,
    now: f64,
) -> Result<ScheduleEntry, String> {
    let mut entry = read_entry(root, id)?;
    if !is_active_status(&entry.status) {
        return Err(format!("只有已建议或已确认的排期可以取消，当前是 {}。", entry.status));
    }
    if entry.is_locked {
        return Err("锁定条目不可取消。".to_string());
    }
    let reason = reason.map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
    entry.status = STATUS_CANCELLED.to_string();
    entry.adjustment_reason = reason.clone();
    entry.adjustments.push(AdjustmentRecord {
        at: crate::task_shell::utc_iso8601(now),
        action: "取消".to_string(),
        reason: reason.unwrap_or_default(),
    });
    entry.updated_at = crate::task_shell::utc_iso8601(now);
    persist(root, &entry)?;
    Ok(entry)
}

pub fn mark_completed(root: &Path, id: &str, now: f64) -> Result<ScheduleEntry, String> {
    let mut entry = read_entry(root, id)?;
    if !matches!(entry.status.as_str(), STATUS_SUGGESTED | STATUS_CONFIRMED) {
        return Err(format!("当前状态不能标记完成：{}", entry.status));
    }
    entry.status = STATUS_DONE.to_string();
    entry.updated_at = crate::task_shell::utc_iso8601(now);
    persist(root, &entry)?;
    Ok(entry)
}

/// 调整 = 旧条目置「已调整」并追加记录 + 新条目（已建议）。
pub fn adjust_entry(
    root: &Path,
    id: &str,
    start_at: &str,
    end_at: &str,
    reason: &str,
    now: f64,
) -> Result<AdjustmentReport, String> {
    let mut old = read_entry(root, id)?;
    if !is_active_status(&old.status) {
        return Err(format!("当前状态不能调整：{}", old.status));
    }
    if old.is_locked {
        return Err("锁定条目不可调整。".to_string());
    }
    let reason = reason.trim().to_string();
    if reason.is_empty() {
        return Err("调整排期必须填写理由。".to_string());
    }
    let start_at = start_at.trim().to_string();
    let end_at = end_at.trim().to_string();
    let duration = duration_between(&start_at, &end_at)?;
    if let Some(conflict) = active_conflict(root, &old.task_id, &start_at, &end_at, Some(id))? {
        return Err(format!(
            "同一任务在重叠时段已有活跃排期（{conflict}），不能调整到该时段。"
        ));
    }
    let previous = old.clone();
    let stamp = crate::task_shell::utc_iso8601(now);
    old.status = STATUS_ADJUSTED.to_string();
    old.adjustment_reason = Some(reason.clone());
    old.adjustments.push(AdjustmentRecord {
        at: stamp.clone(),
        action: "调整".to_string(),
        reason: reason.clone(),
    });
    old.updated_at = stamp.clone();
    persist(root, &old)?;
    let draft = ScheduleDraft {
        id: None,
        task_id: old.task_id.clone(),
        start_at,
        end_at,
        duration: Some(duration),
        is_locked: Some(false),
    };
    match create_entry(root, &draft, now) {
        Ok(new) => Ok(AdjustmentReport { old, new }),
        Err(error) => {
            let _ = persist(root, &previous);
            Err(error)
        }
    }
}

/// 读取派生状态：任务完成/取消后，未来条目显示「已失效」，文件不改写。
pub fn effective_status(entry: &ScheduleEntry, task_state: &str, now: f64) -> String {
    if matches!(entry.status.as_str(), STATUS_ADJUSTED | STATUS_CANCELLED | STATUS_DONE) {
        return entry.status.clone();
    }
    let future = parse_timestamp(&entry.end_at)
        .map(|end| end > now.floor() as i64)
        .unwrap_or(false);
    if future
        && matches!(task_state, task_store::STATE_DONE | task_store::STATE_CANCELLED)
    {
        return STATUS_INVALID.to_string();
    }
    entry.status.clone()
}

pub fn read_entry_view(root: &Path, id: &str, now: f64) -> Result<ScheduleEntryView, String> {
    let entry = read_entry(root, id)?;
    let task = task_store::read_task(root, &entry.task_id)?;
    Ok(ScheduleEntryView {
        effective_status: effective_status(&entry, &task.state, now),
        entry,
    })
}

pub fn list_entry_views(root: &Path, now: f64) -> Result<ScheduleListing, String> {
    assert_work_root(root)?;
    let dir = schedule_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if dir.exists() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取排期目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取排期目录项失败：{e}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let parsed = fs::read_to_string(&path)
                .map_err(|e| format!("读取排期失败：{e}"))
                .and_then(|raw| parse_entry(&raw));
            match parsed {
                Ok(entry) => {
                    let state = task_store::read_task(root, &entry.task_id)
                        .map(|task| task.state)
                        .unwrap_or_else(|_| String::new());
                    items.push(ScheduleEntryView {
                        effective_status: effective_status(&entry, &state, now),
                        entry,
                    });
                }
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        a.entry
            .start_at
            .cmp(&b.entry.start_at)
            .then_with(|| a.entry.id.cmp(&b.entry.id))
    });
    let tasks = task_store::list_tasks(root, &task_store::TaskFilter::default())?.items;
    let generated_at = generated_at(root)?;
    let stale = is_stale(root, &tasks)?;
    Ok(ScheduleListing {
        items,
        broken,
        generated_at,
        stale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::{create_task, transition_task, TaskDraft, STATE_IN_PROGRESS};
    use std::path::PathBuf;

    const NOW: f64 = 1_790_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-qs407-schedule-{tag}-{}-{}",
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

    fn task(temp: &TempRoot, title: &str) -> Task {
        create_task(
            temp.root(),
            &TaskDraft {
                title: title.to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn draft(task_id: &str, start: &str, end: &str) -> ScheduleDraft {
        ScheduleDraft {
            task_id: task_id.to_string(),
            start_at: start.to_string(),
            end_at: end.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn create_writes_frontmatter_and_adjustment_section() {
        let temp = TempRoot::new("create");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:30:00Z"),
            NOW,
        )
        .unwrap();
        assert_eq!(entry.duration, 90);
        assert_eq!(entry.status, STATUS_SUGGESTED);
        assert!(!entry.is_locked);
        let raw = fs::read_to_string(entry_path(temp.root(), &entry.id).unwrap()).unwrap();
        assert!(raw.contains("schema: \"omnidesk.schedule.v1\""), "{raw}");
        assert!(raw.contains("## 调整记录"), "{raw}");
    }

    #[test]
    fn create_requires_existing_total_task() {
        let temp = TempRoot::new("missing-task");
        let err = create_entry(
            temp.root(),
            &draft("t-missing", "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("读取任务失败") || err.contains("不存在"), "{err}");
    }

    #[test]
    fn create_rejects_inverted_or_mismatched_duration() {
        let temp = TempRoot::new("duration");
        let task = task(&temp, "写周报");
        let err = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T02:00:00Z", "2026-11-09T01:00:00Z"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("结束时间必须晚于开始时间"), "{err}");
        let mut mismatch = draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z");
        mismatch.duration = Some(30);
        let err = create_entry(temp.root(), &mismatch, NOW).unwrap_err();
        assert!(err.contains("不一致"), "{err}");
    }

    #[test]
    fn create_rejects_overlapping_active_entry_for_same_task() {
        let temp = TempRoot::new("overlap");
        let task = task(&temp, "写周报");
        create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let err = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:30:00Z", "2026-11-09T02:30:00Z"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("重叠时段"), "{err}");
    }

    #[test]
    fn adjacent_entry_and_other_task_are_allowed() {
        let temp = TempRoot::new("adjacent");
        let first = task(&temp, "写周报");
        let second = task(&temp, "整理资料");
        create_entry(
            temp.root(),
            &draft(&first.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        create_entry(
            temp.root(),
            &draft(&first.id, "2026-11-09T02:00:00Z", "2026-11-09T03:00:00Z"),
            NOW,
        )
        .unwrap();
        create_entry(
            temp.root(),
            &draft(&second.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
    }

    #[test]
    fn confirm_suggested_entry() {
        let temp = TempRoot::new("confirm");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let confirmed = confirm_entry(temp.root(), &entry.id, NOW + 60.0).unwrap();
        assert_eq!(confirmed.status, STATUS_CONFIRMED);
        let raw = fs::read_to_string(entry_path(temp.root(), &entry.id).unwrap()).unwrap();
        assert!(raw.contains("status: \"已确认\""), "{raw}");
    }

    #[test]
    fn confirm_rejects_non_suggested_status() {
        let temp = TempRoot::new("confirm-bad");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        cancel_entry(temp.root(), &entry.id, Some("不做了"), NOW).unwrap();
        let err = confirm_entry(temp.root(), &entry.id, NOW).unwrap_err();
        assert!(err.contains("只有已建议"), "{err}");
    }

    #[test]
    fn adjust_marks_old_and_creates_new_suggestion() {
        let temp = TempRoot::new("adjust");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let report = adjust_entry(
            temp.root(),
            &entry.id,
            "2026-11-09T03:00:00Z",
            "2026-11-09T04:30:00Z",
            "与会议冲突",
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(report.old.status, STATUS_ADJUSTED);
        assert_eq!(report.old.adjustments.len(), 1);
        assert_eq!(report.new.status, STATUS_SUGGESTED);
        assert_eq!(report.new.duration, 90);
        let raw = fs::read_to_string(entry_path(temp.root(), &entry.id).unwrap()).unwrap();
        assert!(raw.contains("调整 | 与会议冲突"), "{raw}");
    }

    #[test]
    fn adjust_requires_reason_and_rejects_locked() {
        let temp = TempRoot::new("adjust-guard");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let err = adjust_entry(
            temp.root(),
            &entry.id,
            "2026-11-09T03:00:00Z",
            "2026-11-09T04:00:00Z",
            "   ",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("必须填写理由"), "{err}");
        lock_entry(temp.root(), &entry.id, NOW).unwrap();
        let err = adjust_entry(
            temp.root(),
            &entry.id,
            "2026-11-09T03:00:00Z",
            "2026-11-09T04:00:00Z",
            "冲突",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("锁定"), "{err}");
    }

    #[test]
    fn cancelled_entry_no_longer_blocks_overlap() {
        let temp = TempRoot::new("cancel");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let cancelled = cancel_entry(temp.root(), &entry.id, Some("临时取消"), NOW).unwrap();
        assert_eq!(cancelled.status, STATUS_CANCELLED);
        assert_eq!(cancelled.adjustments.len(), 1);
        create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
    }

    #[test]
    fn lock_only_active_entries() {
        let temp = TempRoot::new("lock");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let locked = lock_entry(temp.root(), &entry.id, NOW).unwrap();
        assert!(locked.is_locked);
        let err = lock_entry(temp.root(), &entry.id, NOW).unwrap_err();
        assert!(err.contains("已经锁定"), "{err}");
    }

    #[test]
    fn completed_task_derives_invalid_without_rewriting_file() {
        let temp = TempRoot::new("derive");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            1_790_000_000.0,
        )
        .unwrap();
        let path = entry_path(temp.root(), &entry.id).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, task_store::STATE_DONE, None, NOW).unwrap();
        let view = read_entry_view(temp.root(), &entry.id, 1_790_100_000.0).unwrap();
        assert_eq!(view.effective_status, STATUS_INVALID);
        assert_eq!(view.entry.status, STATUS_SUGGESTED);
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(before, after, "派生状态不应改写文件");
    }

    #[test]
    fn list_reports_broken_and_sorts_by_start() {
        let temp = TempRoot::new("list");
        let task = task(&temp, "写周报");
        create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T03:00:00Z", "2026-11-09T04:00:00Z"),
            NOW,
        )
        .unwrap();
        create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        fs::write(schedule_dir(temp.root()).join("bad.md"), "没有 frontmatter").unwrap();
        let listing = list_entry_views(temp.root(), NOW).unwrap();
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.broken, 1);
        assert_eq!(listing.items[0].entry.start_at, "2026-11-09T01:00:00Z");
    }

    #[test]
    fn rejects_bad_timestamp_and_done_task() {
        let temp = TempRoot::new("guards");
        let task = task(&temp, "写周报");
        let err = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09 01:00", "2026-11-09 02:00"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("格式应为"), "{err}");
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, task_store::STATE_DONE, None, NOW).unwrap();
        let err = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-10T01:00:00Z", "2026-11-10T02:00:00Z"),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("不能再创建"), "{err}");
    }

    #[test]
    fn mark_completed_requires_active_status() {
        let temp = TempRoot::new("complete");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let done = mark_completed(temp.root(), &entry.id, NOW).unwrap();
        assert_eq!(done.status, STATUS_DONE);
        let err = mark_completed(temp.root(), &entry.id, NOW).unwrap_err();
        assert!(err.contains("不能标记完成"), "{err}");
    }

    #[test]
    fn generation_timestamp_round_trips_and_detects_stale() {
        let temp = TempRoot::new("generation");
        let task = task(&temp, "写周报");
        assert!(is_stale(temp.root(), &[task.clone()]).unwrap(), "没有生成记录时视为过期");
        mark_generated_at(temp.root(), NOW).unwrap();
        assert_eq!(
            generated_at(temp.root()).unwrap().as_deref(),
            Some(crate::task_shell::utc_iso8601(NOW).as_str())
        );
        assert!(!is_stale(temp.root(), &[task.clone()]).unwrap());
        let updated = crate::task_store::update_task(
            temp.root(),
            &task.id,
            &crate::task_store::TaskPatch {
                title: Some("改过标题".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert!(is_stale(temp.root(), &[updated]).unwrap());
    }

    #[test]
    fn expire_turns_done_task_suggestion_into_cancelled_entry() {
        let temp = TempRoot::new("expire");
        let task = task(&temp, "写周报");
        let entry = create_entry(
            temp.root(),
            &draft(&task.id, "2026-11-10T01:00:00Z", "2026-11-10T02:00:00Z"),
            NOW,
        )
        .unwrap();
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, task_store::STATE_DONE, None, NOW).unwrap();
        let done = crate::task_store::read_task(temp.root(), &task.id).unwrap();
        let expired = expire_stale_suggestions(temp.root(), &[done], NOW + 60.0).unwrap();
        assert_eq!(expired, vec![entry.id.clone()]);
        let stored = read_entry(temp.root(), &entry.id).unwrap();
        assert_eq!(stored.status, STATUS_CANCELLED);
        assert_eq!(
            stored.adjustment_reason.as_deref(),
            Some("任务已结束，建议过期")
        );
        assert_eq!(stored.adjustments.last().unwrap().action, "过期");
    }

    #[test]
    fn expire_skips_active_and_locked_suggestions() {
        let temp = TempRoot::new("expire-skip");
        let active = task(&temp, "活跃任务");
        let locked = task(&temp, "锁定任务");
        let active_entry = create_entry(
            temp.root(),
            &draft(&active.id, "2026-11-10T01:00:00Z", "2026-11-10T02:00:00Z"),
            NOW,
        )
        .unwrap();
        let locked_entry = create_entry(
            temp.root(),
            &draft(&locked.id, "2026-11-10T03:00:00Z", "2026-11-10T04:00:00Z"),
            NOW,
        )
        .unwrap();
        lock_entry(temp.root(), &locked_entry.id, NOW).unwrap();
        transition_task(temp.root(), &locked.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &locked.id, task_store::STATE_DONE, None, NOW).unwrap();
        transition_task(temp.root(), &active.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        let tasks = vec![
            crate::task_store::read_task(temp.root(), &active.id).unwrap(),
            crate::task_store::read_task(temp.root(), &locked.id).unwrap(),
        ];
        let expired = expire_stale_suggestions(temp.root(), &tasks, NOW + 60.0).unwrap();
        assert!(expired.is_empty());
        assert_eq!(read_entry(temp.root(), &active_entry.id).unwrap().status, STATUS_SUGGESTED);
        assert_eq!(read_entry(temp.root(), &locked_entry.id).unwrap().status, STATUS_SUGGESTED);
    }
}
