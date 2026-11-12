//! 周期复盘存储与校验（OD-WR-505）。
//!
//! 边界：
//!   * 只汇集周期内 `draft=false` 的正式记录，以及真实存在的任务；
//!   * 任务只做状态统计，不创建、不改状态；
//!   * 复盘正文由界面侧模型生成，宿主校验标题、引用编号与空周期声明；
//!   * 落盘到 `work/复盘/<period>/<start>_<end>.md`；
//!   * 重新生成只允许覆盖草稿；已确认的复盘绝不覆盖。
//!   * 覆盖草稿前先备份为 `.bak`，已有备份时追加时间后缀。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::material_store;
use crate::task_store::{self, Task, TaskFilter};
use crate::worklog_records::{self, RecordFilter, WorkRecord};
use crate::writing_store;

pub const REVIEW_DIR: &str = "复盘";
pub const REVIEW_SCHEMA: &str = "omnidesk.period-review.v1";
pub const STATUS_DRAFT: &str = "草稿";
pub const STATUS_CONFIRMED: &str = "已确认";

pub const PERIOD_WEEKLY: &str = "weekly";
pub const PERIOD_MONTHLY: &str = "monthly";
pub const PERIOD_QUARTERLY: &str = "quarterly";
pub const PERIOD_ANNUAL: &str = "annual";

pub const WEEKLY_SECTIONS: [&str; 7] = [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
];

pub const MONTHLY_SECTIONS: [&str; 9] = [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
];

pub const QUARTERLY_SECTIONS: [&str; 11] = [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "跨月对照",
    "主题归纳",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
];

pub const ANNUAL_SECTIONS: [&str; 13] = [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "跨月对照",
    "主题归纳",
    "全年脉络",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
    "长期建议",
];

const EMPTY_MARKERS: [&str; 6] = [
    "本周期无记录",
    "本周期没有记录",
    "周期内无记录",
    "没有正式记录",
    "暂无记录",
    "无记录",
];

/// 任务状态统计。只计数，不创建、不迁移任何任务。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStats {
    pub completed: usize,
    pub uncompleted: usize,
    pub overdue: usize,
}

/// 汇集结果。记录与任务均为全文/全字段，界面据此组装材料，宿主据此校验引用。
#[derive(Debug, Clone, Serialize)]
pub struct PeriodBundle {
    pub period: String,
    pub start: String,
    pub end: String,
    pub range: String,
    pub records: Vec<WorkRecord>,
    pub tasks: Vec<Task>,
    pub task_stats: TaskStats,
    pub prior_reviews: Vec<String>,
    pub material: String,
    pub record_count: usize,
    pub task_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodReviewReceipt {
    pub ok: bool,
    pub path: String,
    pub period: String,
    pub range: String,
    pub record_ids: Vec<String>,
    pub task_ids: Vec<String>,
    pub generated_at: String,
    pub status: String,
    pub confirmed_at: Option<String>,
    pub backup: Option<String>,
    pub records: usize,
    pub tasks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodReviewRead {
    pub exists: bool,
    pub path: String,
    pub period: String,
    pub range: String,
    pub status: Option<String>,
    pub record_ids: Vec<String>,
    pub task_ids: Vec<String>,
    pub generated_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub body: String,
    pub missing_record_ids: Vec<String>,
    pub missing_task_ids: Vec<String>,
    pub stale: bool,
}

/// 档案页列表项（OD-WR-506）：只含定位与状态，不含正文。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewArchiveItem {
    pub path: String,
    pub period: String,
    pub start: String,
    pub end: String,
    pub range: String,
    pub status: Option<String>,
    pub generated_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub record_ids: Vec<String>,
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodReviewListing {
    pub items: Vec<ReviewArchiveItem>,
    pub broken: usize,
}

/// 记录详情里的只读反向引用（OD-WR-506）。
///
/// kind=review 指向周期复盘；kind=writing 指向写作任务（经选中素材回指）。
/// 记录文件本身不写入任何反向指针，这里每次扫描得出。
#[derive(Debug, Clone, Serialize)]
pub struct RecordBacklink {
    pub kind: String,
    pub id: String,
    pub path: String,
    pub period: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub range: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordBacklinks {
    pub record_id: String,
    pub items: Vec<RecordBacklink>,
    pub broken: usize,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ReviewFront {
    #[serde(default)]
    schema: Option<String>,
    period: String,
    range: String,
    #[serde(default)]
    record_ids: Vec<String>,
    #[serde(default)]
    task_ids: Vec<String>,
    #[serde(default)]
    generated_at: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    confirmed_at: Option<String>,
}

pub fn parse_period(period: &str) -> Result<&'static str, String> {
    match period.trim() {
        PERIOD_WEEKLY => Ok(PERIOD_WEEKLY),
        PERIOD_MONTHLY => Ok(PERIOD_MONTHLY),
        PERIOD_QUARTERLY => Ok(PERIOD_QUARTERLY),
        PERIOD_ANNUAL => Ok(PERIOD_ANNUAL),
        other => Err(format!("复盘周期不认识：{other}")),
    }
}

pub fn sections_for(period: &str) -> Result<&'static [&'static str], String> {
    match parse_period(period)? {
        PERIOD_WEEKLY => Ok(&WEEKLY_SECTIONS),
        PERIOD_MONTHLY => Ok(&MONTHLY_SECTIONS),
        PERIOD_QUARTERLY => Ok(&QUARTERLY_SECTIONS),
        _ => Ok(&ANNUAL_SECTIONS),
    }
}

fn range_text(start: &str, end: &str) -> String {
    format!("{start} ~ {end}")
}

fn validate_range(start: &str, end: &str) -> Result<(), String> {
    if !worklog_records::is_date_shape(start) || !worklog_records::is_date_shape(end) {
        return Err(format!("复盘起止日期形状不正确（应为 YYYY-MM-DD）：{start} ~ {end}"));
    }
    if start > end {
        return Err(format!("复盘开始日期晚于结束日期：{start} ~ {end}"));
    }
    Ok(())
}

pub fn review_dir(root: &Path, period: &str) -> Result<PathBuf, String> {
    let period = parse_period(period)?;
    Ok(root.join(REVIEW_DIR).join(period))
}

/// 复盘文件 `<root>/复盘/<period>/<start>_<end>.md`。日期先校验形状。
pub fn review_path(root: &Path, period: &str, start: &str, end: &str) -> Result<PathBuf, String> {
    validate_range(start, end)?;
    Ok(review_dir(root, period)?.join(format!("{start}_{end}.md")))
}

fn date_prefix(value: &str) -> &str {
    value.get(0..10).unwrap_or("")
}

fn in_range(date: &str, start: &str, end: &str) -> bool {
    date.len() >= 10 && date >= start && date <= end
}

fn task_in_period(task: &Task, start: &str, end: &str) -> bool {
    in_range(date_prefix(&task.created_at), start, end)
        || in_range(date_prefix(&task.updated_at), start, end)
        || task
            .due_date
            .as_deref()
            .map(|due| in_range(due, start, end))
            .unwrap_or(false)
        || task
            .history
            .iter()
            .any(|change| in_range(date_prefix(&change.at), start, end))
}

fn task_is_overdue(task: &Task, today: &str) -> bool {
    if task.state == task_store::STATE_DONE || task.state == task_store::STATE_CANCELLED {
        return false;
    }
    task.due_date
        .as_deref()
        .map(|due| due < today)
        .unwrap_or(false)
}

/// 汇集一个周期。记录只看正式记录；任务只统计；既往复盘只取标题。
pub fn collect_period(
    root: &Path,
    period: &str,
    start: &str,
    end: &str,
) -> Result<PeriodBundle, String> {
    worklog_records::assert_work_root(root)?;
    let period = parse_period(period)?;
    validate_range(start, end)?;

    let listing = worklog_records::list_records(
        root,
        &RecordFilter {
            from_date: Some(start.to_string()),
            to_date: Some(end.to_string()),
            draft: Some(false),
            ..Default::default()
        },
    )?;
    let mut records: Vec<WorkRecord> = listing
        .items
        .into_iter()
        .filter(|record| !record.draft)
        .collect();
    records.sort_by(|a, b| {
        a.record_date
            .cmp(&b.record_date)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });

    let today = {
        let now = crate::task_shell::utc_iso8601(crate::task_shell::now_secs());
        now[0..10].to_string()
    };
    let all_tasks = task_store::list_tasks(root, &TaskFilter::default())?;
    let mut tasks: Vec<Task> = all_tasks
        .items
        .into_iter()
        .filter(|task| task_in_period(task, start, end) || task_is_overdue(task, &today))
        .collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id));

    let mut task_stats = TaskStats::default();
    for task in &tasks {
        if task.state == task_store::STATE_DONE {
            task_stats.completed += 1;
        } else if task.state != task_store::STATE_CANCELLED {
            task_stats.uncompleted += 1;
        }
        if task_is_overdue(task, &today) {
            task_stats.overdue += 1;
        }
    }

    let target = review_path(root, period, start, end)?;
    let prior_reviews = prior_review_titles(root, Some(&target))?;
    let material = build_material(period, start, end, &records, &tasks, &task_stats, &prior_reviews);
    Ok(PeriodBundle {
        period: period.to_string(),
        start: start.to_string(),
        end: end.to_string(),
        range: range_text(start, end),
        record_count: records.len(),
        task_count: tasks.len(),
        records,
        tasks,
        task_stats,
        prior_reviews,
        material,
    })
}

fn build_material(
    period: &str,
    start: &str,
    end: &str,
    records: &[WorkRecord],
    tasks: &[Task],
    stats: &TaskStats,
    prior: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "复盘周期：{period}，范围：{} ~ {start} 至 {end}\n\n",
        range_text(start, end)
    ));
    out.push_str(&format!("正式工作记录：{} 条（草稿已排除）\n", records.len()));
    if records.is_empty() {
        out.push_str("本周期无记录。\n");
    } else {
        for record in records {
            out.push_str(&format!("\n#{0}｜{1}\n", record.id, record.title));
            out.push_str(&format!("记录日期：{}\n", record.record_date));
            if let Some(category) = record.category.as_deref() {
                out.push_str(&format!("分类：{category}\n"));
            }
            if !record.tags.is_empty() {
                out.push_str(&format!("标签：{}\n", record.tags.join("、")));
            }
            if !record.linked_task_ids.is_empty() {
                out.push_str(&format!("关联任务：{}\n", record.linked_task_ids.join("、")));
            }
            out.push_str("正文：\n");
            out.push_str(record.body.trim());
            out.push('\n');
        }
    }

    out.push_str(&format!(
        "\n任务：{} 条｜已完成 {}｜未完成 {}｜逾期 {}\n",
        tasks.len(),
        stats.completed,
        stats.uncompleted,
        stats.overdue
    ));
    if tasks.is_empty() {
        out.push_str("本周期没有相关任务。\n");
    } else {
        for task in tasks {
            out.push_str(&format!("\n#{}｜{}\n", task.id, task.title));
            out.push_str(&format!("状态：{}\n", task.state));
            if let Some(due) = task.due_date.as_deref() {
                out.push_str(&format!("截止日期：{due}\n"));
            }
            if let Some(category) = task.category.as_deref() {
                out.push_str(&format!("分类：{category}\n"));
            }
        }
    }

    out.push_str("\n既往复盘标题：\n");
    if prior.is_empty() {
        out.push_str("（无）\n");
    } else {
        for title in prior {
            out.push_str(&format!("- {title}\n"));
        }
    }
    out
}

fn prior_review_titles(root: &Path, exclude: Option<&Path>) -> Result<Vec<String>, String> {
    let dir = root.join(REVIEW_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut titles: Vec<(String, String)> = Vec::new();
    let periods = fs::read_dir(&dir)
        .map_err(|e| format!("读取复盘目录失败（{}）：{e}", dir.display()))?;
    for period_entry in periods {
        let period_entry = period_entry.map_err(|e| format!("读取复盘目录项失败：{e}"))?;
        if !period_entry
            .file_type()
            .map_err(|e| format!("读取复盘目录项类型失败：{e}"))?
            .is_dir()
        {
            continue;
        }
        let files = fs::read_dir(period_entry.path())
            .map_err(|e| format!("读取复盘周期目录失败：{e}"))?;
        for file in files {
            let file = file.map_err(|e| format!("读取复盘文件项失败：{e}"))?;
            let path = file.path();
            if exclude.map(|skip| path.as_path() == skip).unwrap_or(false) {
                continue;
            }
            let is_markdown = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_markdown {
                continue;
            }
            let raw = fs::read_to_string(&path).unwrap_or_default();
            let (_, body) = worklog_records::split_frontmatter(&raw).unwrap_or((String::new(), raw));
            let title = body
                .lines()
                .find_map(|line| line.trim().strip_prefix("# ").map(str::trim))
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    path.file_stem()
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_else(|| "未命名复盘".to_string())
                });
            titles.push((path.to_string_lossy().to_string(), title));
        }
    }
    titles.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(titles.into_iter().map(|(_, title)| title).collect())
}

fn heading_of<'a>(line: &str, sections: &'a [&'a str]) -> Option<&'a str> {
    let mut value = line.trim();
    while let Some(rest) = value.strip_prefix('#') {
        value = rest.trim_start();
    }
    while let Some(rest) = value.strip_prefix('*') {
        value = rest.trim_start();
    }
    value = value.trim_end_matches('*').trim();
    value = value.trim_matches(|ch| {
        matches!(ch, '【' | '】' | '「' | '」' | '[' | ']' | ' ' | '\t')
    });
    let value = value.split(['：', ':']).next().unwrap_or(value).trim();
    sections.iter().find(|title| **title == value).copied()
}

/// 取出正文里的记录/任务引用编号。只有 `wr-` 与 `t-` 前缀算引用。
fn citation_ids(text: &str) -> (Vec<String>, Vec<String>) {
    let mut records: Vec<String> = Vec::new();
    let mut tasks: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('#') {
        let tail = &rest[at + 1..];
        let id: String = tail
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        let skip = id.len().max(1);
        if id.starts_with("wr-") {
            if !records.iter().any(|existing| existing == &id) {
                records.push(id);
            }
        } else if id.starts_with("t-") && !tasks.iter().any(|existing| existing == &id) {
            tasks.push(id);
        }
        rest = &tail[skip..];
    }
    (records, tasks)
}

fn validate_body(
    content: &str,
    period: &str,
    records: &[WorkRecord],
    tasks: &[Task],
) -> Result<(String, Vec<String>, Vec<String>), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("复盘正文为空，未写入".to_string());
    }
    let body = match worklog_records::split_frontmatter(trimmed) {
        Ok((_front, body)) => body,
        Err(_) => trimmed.to_string(),
    };
    if body.len() > 1024 * 1024 {
        return Err("复盘正文过大（超过 1MB），未写入".to_string());
    }
    let sections = sections_for(period)?;
    let mut found: Vec<(&str, String)> = Vec::new();
    let mut current: Option<&str> = None;
    let mut text = String::new();
    for line in body.lines() {
        if let Some(title) = heading_of(line, sections) {
            if found.iter().any(|(existing, _)| *existing == title) {
                return Err(format!("复盘段落重复：{title}，未写入"));
            }
            if let Some(previous) = current {
                found.push((previous, std::mem::take(&mut text)));
            }
            current = Some(title);
            continue;
        }
        if current.is_some() {
            text.push_str(line);
            text.push('\n');
        }
    }
    if let Some(previous) = current {
        found.push((previous, std::mem::take(&mut text)));
    }

    for title in sections {
        if !found.iter().any(|(existing, _)| existing == title) {
            return Err(format!("复盘缺少段落：{title}，未写入"));
        }
    }
    for title in sections {
        if found
            .iter()
            .find(|(existing, _)| existing == title)
            .map(|(_, section)| section.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(format!("复盘段落没有正文：{title}，未写入"));
        }
    }

    let known_records: Vec<&str> = records.iter().map(|record| record.id.as_str()).collect();
    let known_tasks: Vec<&str> = tasks.iter().map(|task| task.id.as_str()).collect();
    let (record_ids, task_ids) = citation_ids(&body);
    for id in &record_ids {
        if !known_records.iter().any(|existing| *existing == id.as_str()) {
            return Err(format!("复盘引用了不存在的记录编号：#{id}，未写入"));
        }
    }
    for id in &task_ids {
        if !known_tasks.iter().any(|existing| *existing == id.as_str()) {
            return Err(format!("复盘引用了不存在的任务编号：#{id}，未写入"));
        }
    }

    let empty = records.is_empty() && tasks.is_empty();
    for title in sections {
        let section = found
            .iter()
            .find(|(existing, _)| existing == title)
            .map(|(_, section)| section.as_str())
            .unwrap_or_default();
        let (section_records, section_tasks) = citation_ids(section);
        if empty {
            if !EMPTY_MARKERS.iter().any(|marker| section.contains(marker)) {
                return Err(format!(
                    "本周期没有记录或任务，段落必须如实说明「本周期无记录」：{title}，未写入"
                ));
            }
        } else if section_records.is_empty() && section_tasks.is_empty() {
            return Err(format!("复盘段落没有标注引用编号：{title}，未写入"));
        }
    }
    if empty && (!record_ids.is_empty() || !task_ids.is_empty()) {
        return Err("本周期没有记录或任务，复盘不应引用任何编号，未写入".to_string());
    }
    Ok((body.trim().to_string(), record_ids, task_ids))
}

fn render_review(front: &ReviewFront, body: &str) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!(
        "schema: {}\n",
        worklog_records::yaml_scalar(REVIEW_SCHEMA)
    ));
    out.push_str(&format!(
        "period: {}\n",
        worklog_records::yaml_scalar(&front.period)
    ));
    out.push_str(&format!(
        "range: {}\n",
        worklog_records::yaml_scalar(&front.range)
    ));
    if front.record_ids.is_empty() {
        out.push_str("record_ids: []\n");
    } else {
        out.push_str("record_ids:\n");
        for id in &front.record_ids {
            out.push_str(&format!("  - {}\n", worklog_records::yaml_scalar(id)));
        }
    }
    if front.task_ids.is_empty() {
        out.push_str("task_ids: []\n");
    } else {
        out.push_str("task_ids:\n");
        for id in &front.task_ids {
            out.push_str(&format!("  - {}\n", worklog_records::yaml_scalar(id)));
        }
    }
    out.push_str(&format!(
        "generated_at: {}\n",
        worklog_records::yaml_scalar(front.generated_at.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "status: {}\n",
        worklog_records::yaml_scalar(front.status.as_deref().unwrap_or(STATUS_DRAFT))
    ));
    if let Some(confirmed_at) = front.confirmed_at.as_deref() {
        out.push_str(&format!(
            "confirmed_at: {}\n",
            worklog_records::yaml_scalar(confirmed_at)
        ));
    }
    out.push_str("---\n\n");
    out.push_str(body.trim());
    out.push('\n');
    out
}

fn read_document(path: &Path) -> Result<(ReviewFront, String), String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取复盘失败（{}）：{e}", path.display()))?;
    let (front_raw, body) = worklog_records::split_frontmatter(&raw)?;
    let front: ReviewFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("复盘 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != REVIEW_SCHEMA {
            return Err(format!("复盘 schema 不认识：{schema}"));
        }
    }
    Ok((front, body))
}

fn backup_path(target: &Path, now: f64) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "review.md".to_string());
    let first = dir.join(format!("{name}.bak"));
    if !first.exists() {
        return first;
    }
    let stamp = now.floor() as i64;
    let stamped = dir.join(format!("{name}.bak-{stamp}"));
    if !stamped.exists() {
        return stamped;
    }
    let mut index = 1u64;
    loop {
        let candidate = dir.join(format!("{name}.bak-{stamp}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn backup_existing(target: &Path, now: f64) -> Result<Option<PathBuf>, String> {
    if !target.exists() {
        return Ok(None);
    }
    let meta = fs::symlink_metadata(target)
        .map_err(|e| format!("读取旧复盘失败（{}）：{e}", target.display()))?;
    if !meta.file_type().is_file() {
        return Err(format!("旧复盘不是普通文件，未覆盖：{}", target.display()));
    }
    let path = backup_path(target, now);
    fs::copy(target, &path).map_err(|e| {
        format!(
            "备份旧复盘失败（{} → {}）：{e}；主文件未改动",
            target.display(),
            path.display()
        )
    })?;
    Ok(Some(path))
}

/// 校验并落盘一份周期复盘草稿。
pub fn write_review(
    root: &Path,
    period: &str,
    start: &str,
    end: &str,
    content: &str,
    now: f64,
) -> Result<PeriodReviewReceipt, String> {
    worklog_records::assert_work_root(root)?;
    let bundle = collect_period(root, period, start, end)?;
    let (body, record_ids, task_ids) = validate_body(content, period, &bundle.records, &bundle.tasks)?;
    let target = review_path(root, period, start, end)?;

    if target.exists() {
        let (front, _) = read_document(&target)?;
        match front.status.as_deref() {
            Some(STATUS_DRAFT) => {}
            Some(STATUS_CONFIRMED) => {
                return Err(format!(
                    "该周期复盘已确认，不允许重新生成覆盖：{}",
                    target.display()
                ))
            }
            other => {
                return Err(format!(
                    "复盘状态不认识（{}），为防覆盖未写入：{}",
                    other.unwrap_or("空"),
                    target.display()
                ))
            }
        }
    }

    fs::create_dir_all(review_dir(root, period)?).map_err(|e| {
        format!(
            "创建复盘目录失败（{}）：{e}",
            review_dir(root, period).unwrap_or_default().display()
        )
    })?;
    let backup = backup_existing(&target, now)?;
    let generated_at = crate::task_shell::utc_iso8601(now);
    let front = ReviewFront {
        schema: Some(REVIEW_SCHEMA.to_string()),
        period: parse_period(period)?.to_string(),
        range: range_text(start, end),
        record_ids: record_ids.clone(),
        task_ids: task_ids.clone(),
        generated_at: Some(generated_at.clone()),
        status: Some(STATUS_DRAFT.to_string()),
        confirmed_at: None,
    };
    let rendered = render_review(&front, &body);
    if backup.is_some() {
        worklog_records::write_replace_atomically(&target, rendered.as_bytes())?;
    } else {
        worklog_records::write_new_atomically(&target, rendered.as_bytes())?;
    }

    Ok(PeriodReviewReceipt {
        ok: true,
        path: target.display().to_string(),
        period: front.period,
        range: front.range,
        record_ids,
        task_ids,
        generated_at,
        status: STATUS_DRAFT.to_string(),
        confirmed_at: None,
        backup: backup.map(|path| path.display().to_string()),
        records: bundle.records.len(),
        tasks: bundle.tasks.len(),
    })
}

/// 读取复盘；不存在是合法状态。
pub fn read_review(
    root: &Path,
    period: &str,
    start: &str,
    end: &str,
) -> Result<PeriodReviewRead, String> {
    worklog_records::assert_work_root(root)?;
    let period = parse_period(period)?;
    validate_range(start, end)?;
    let path = review_path(root, period, start, end)?;
    if !path.exists() {
        return Ok(PeriodReviewRead {
            exists: false,
            path: path.display().to_string(),
            period: period.to_string(),
            range: range_text(start, end),
            status: None,
            record_ids: Vec::new(),
            task_ids: Vec::new(),
            generated_at: None,
            confirmed_at: None,
            body: String::new(),
            missing_record_ids: Vec::new(),
            missing_task_ids: Vec::new(),
            stale: false,
        });
    }
    let (front, body) = read_document(&path)?;
    if front.period != period {
        return Err(format!(
            "复盘周期与目录不一致：目录 {period}，frontmatter {}",
            front.period
        ));
    }
    if front.range != range_text(start, end) {
        return Err(format!(
            "复盘范围与文件名不一致：文件 {}，frontmatter {}",
            range_text(start, end),
            front.range
        ));
    }
    for id in &front.record_ids {
        worklog_records::assert_record_id(id)?;
    }
    for id in &front.task_ids {
        if !worklog_records::is_safe_id(id) {
            return Err(format!("任务编号不合法：{id:?}"));
        }
    }
    let mut missing_record_ids = Vec::new();
    for id in &front.record_ids {
        if worklog_records::read_record(root, id).is_err() {
            missing_record_ids.push(id.clone());
        }
    }
    let mut missing_task_ids = Vec::new();
    for id in &front.task_ids {
        if task_store::read_task(root, id).is_err() {
            missing_task_ids.push(id.clone());
        }
    }
    Ok(PeriodReviewRead {
        exists: true,
        path: path.display().to_string(),
        period: period.to_string(),
        range: front.range,
        status: front.status,
        record_ids: front.record_ids,
        task_ids: front.task_ids,
        generated_at: front.generated_at,
        confirmed_at: front.confirmed_at,
        body,
        stale: !missing_record_ids.is_empty() || !missing_task_ids.is_empty(),
        missing_record_ids,
        missing_task_ids,
    })
}

fn archive_range(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let (start, end) = stem.split_once('_')?;
    if !worklog_records::is_date_shape(start)
        || !worklog_records::is_date_shape(end)
        || start > end
    {
        return None;
    }
    Some((start.to_string(), end.to_string()))
}

fn archive_item(path: &Path, period: &str, front: &ReviewFront) -> Option<ReviewArchiveItem> {
    let (start, end) = archive_range(path)?;
    if front.period != period || front.range != range_text(&start, &end) {
        return None;
    }
    Some(ReviewArchiveItem {
        path: path.display().to_string(),
        period: period.to_string(),
        start,
        end,
        range: front.range.clone(),
        status: front.status.clone(),
        generated_at: front.generated_at.clone(),
        confirmed_at: front.confirmed_at.clone(),
        record_ids: front.record_ids.clone(),
        task_ids: front.task_ids.clone(),
    })
}

fn body_title(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.trim().strip_prefix("# ").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 扫描复盘周期目录生成档案列表。坏文件计数后跳过，不中断读取。
pub fn list_reviews(root: &Path) -> Result<PeriodReviewListing, String> {
    worklog_records::assert_work_root(root)?;
    let dir = root.join(REVIEW_DIR);
    let mut items: Vec<ReviewArchiveItem> = Vec::new();
    let mut broken = 0usize;
    if !dir.exists() {
        return Ok(PeriodReviewListing { items, broken });
    }
    let periods = fs::read_dir(&dir)
        .map_err(|e| format!("读取复盘目录失败（{}）：{e}", dir.display()))?;
    for period_entry in periods {
        let period_entry = match period_entry {
            Ok(value) => value,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        let is_dir = match period_entry.file_type() {
            Ok(kind) => kind.is_dir(),
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        if !is_dir {
            continue;
        }
        let period_name = period_entry.file_name().to_string_lossy().to_string();
        let period = match parse_period(&period_name) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let files = match fs::read_dir(period_entry.path()) {
            Ok(value) => value,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        for file in files {
            let file = match file {
                Ok(value) => value,
                Err(_) => {
                    broken += 1;
                    continue;
                }
            };
            let path = file.path();
            let is_markdown = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_markdown {
                continue;
            }
            match read_document(&path) {
                Ok((front, _)) => match archive_item(&path, period, &front) {
                    Some(item) => items.push(item),
                    None => broken += 1,
                },
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        b.end
            .cmp(&a.end)
            .then_with(|| b.start.cmp(&a.start))
            .then_with(|| b.path.cmp(&a.path))
    });
    Ok(PeriodReviewListing { items, broken })
}

/// 记录详情的反向引用：扫描复盘 frontmatter 的 record_ids，
/// 以及写作任务经选中素材（来源为工作记录）回指该记录的条目。
/// 只读；不写任何文件，也不在记录文件里落反向指针。
pub fn record_backlinks(root: &Path, record_id: &str) -> Result<RecordBacklinks, String> {
    worklog_records::assert_work_root(root)?;
    worklog_records::assert_record_id(record_id)?;
    let mut items: Vec<RecordBacklink> = Vec::new();
    let mut broken = 0usize;

    let dir = root.join(REVIEW_DIR);
    if dir.exists() {
        let periods = fs::read_dir(&dir)
            .map_err(|e| format!("读取复盘目录失败（{}）：{e}", dir.display()))?;
        for period_entry in periods {
            let period_entry = match period_entry {
                Ok(value) => value,
                Err(_) => {
                    broken += 1;
                    continue;
                }
            };
            let is_dir = match period_entry.file_type() {
                Ok(kind) => kind.is_dir(),
                Err(_) => {
                    broken += 1;
                    continue;
                }
            };
            if !is_dir {
                continue;
            }
            let period_name = period_entry.file_name().to_string_lossy().to_string();
            let period = match parse_period(&period_name) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let files = match fs::read_dir(period_entry.path()) {
                Ok(value) => value,
                Err(_) => {
                    broken += 1;
                    continue;
                }
            };
            for file in files {
                let file = match file {
                    Ok(value) => value,
                    Err(_) => {
                        broken += 1;
                        continue;
                    }
                };
                let path = file.path();
                let is_markdown = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("md"))
                    .unwrap_or(false);
                if !is_markdown {
                    continue;
                }
                let (front, body) = match read_document(&path) {
                    Ok(value) => value,
                    Err(_) => {
                        broken += 1;
                        continue;
                    }
                };
                if !front.record_ids.iter().any(|id| id == record_id) {
                    continue;
                }
                match archive_item(&path, period, &front) {
                    Some(item) => items.push(RecordBacklink {
                        kind: "review".to_string(),
                        id: item.path.clone(),
                        path: item.path,
                        period: Some(item.period),
                        start: Some(item.start),
                        end: Some(item.end),
                        range: Some(item.range),
                        status: item.status,
                        title: body_title(&body),
                    }),
                    None => broken += 1,
                }
            }
        }
    }

    let material_ids: Vec<String> = match material_store::list_materials(root, &Default::default())
    {
        Ok(listing) => listing
            .items
            .into_iter()
            .filter(|material| {
                material.source_type == material_store::SOURCE_WORK_RECORD
                    && material.source_id == record_id
            })
            .map(|material| material.material_id)
            .collect(),
        Err(_) => Vec::new(),
    };
    if !material_ids.is_empty() {
        match writing_store::list_writing_tasks(root, &Default::default()) {
            Ok(listing) => {
                broken += listing.broken;
                for task in listing.items {
                    if !task
                        .selected_material_ids
                        .iter()
                        .any(|id| material_ids.iter().any(|hit| hit == id))
                    {
                        continue;
                    }
                    items.push(RecordBacklink {
                        kind: "writing".to_string(),
                        id: task.writing_id.clone(),
                        path: writing_store::writing_path(root, &task.writing_id)
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        period: None,
                        start: None,
                        end: None,
                        range: if task.writing_period.trim().is_empty() {
                            None
                        } else {
                            Some(task.writing_period.clone())
                        },
                        status: Some(task.status.clone()),
                        title: Some(task.title.clone()),
                    });
                }
            }
            Err(_) => broken += 1,
        }
    }

    items.sort_by(|a, b| {
        let rank = |kind: &str| if kind == "review" { 0u8 } else { 1u8 };
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| b.end.cmp(&a.end))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(RecordBacklinks {
        record_id: record_id.to_string(),
        items,
        broken,
    })
}

/// 确认草稿：草稿 → 已确认，并写 confirmed_at。重复确认幂等。
pub fn confirm_review(
    root: &Path,
    period: &str,
    start: &str,
    end: &str,
    now: f64,
) -> Result<PeriodReviewReceipt, String> {
    worklog_records::assert_work_root(root)?;
    let period = parse_period(period)?;
    validate_range(start, end)?;
    let path = review_path(root, period, start, end)?;
    if !path.exists() {
        return Err(format!("复盘不存在，无法确认：{}", path.display()));
    }
    let (mut front, body) = read_document(&path)?;
    if front.period != period || front.range != range_text(start, end) {
        return Err("复盘周期或范围与文件名不一致，未确认".to_string());
    }
    match front.status.as_deref() {
        Some(STATUS_CONFIRMED) => {
            return Ok(PeriodReviewReceipt {
                ok: true,
                path: path.display().to_string(),
                period: period.to_string(),
                range: range_text(start, end),
                record_ids: front.record_ids,
                task_ids: front.task_ids,
                generated_at: front.generated_at.unwrap_or_default(),
                status: STATUS_CONFIRMED.to_string(),
                confirmed_at: front.confirmed_at,
                backup: None,
                records: 0,
                tasks: 0,
            });
        }
        Some(STATUS_DRAFT) => {}
        other => {
            return Err(format!(
                "复盘状态不认识（{}），未确认：{}",
                other.unwrap_or("空"),
                path.display()
            ))
        }
    }
    let confirmed_at = crate::task_shell::utc_iso8601(now);
    front.status = Some(STATUS_CONFIRMED.to_string());
    front.confirmed_at = Some(confirmed_at.clone());
    let rendered = render_review(&front, &body);
    let backup = backup_existing(&path, now)?;
    worklog_records::write_replace_atomically(&path, rendered.as_bytes())?;
    Ok(PeriodReviewReceipt {
        ok: true,
        path: path.display().to_string(),
        period: period.to_string(),
        range: range_text(start, end),
        record_ids: front.record_ids,
        task_ids: front.task_ids,
        generated_at: front.generated_at.unwrap_or_default(),
        status: STATUS_CONFIRMED.to_string(),
        confirmed_at: Some(confirmed_at),
        backup: backup.map(|path| path.display().to_string()),
        records: 0,
        tasks: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_store::{MaterialDraft, SOURCE_WORK_RECORD};
    use crate::task_store::TaskDraft;
    use crate::worklog_records::RecordDraft;
    use crate::writing_store::{WritingTaskDraft, TYPE_MONTHLY_SUMMARY};
    use std::sync::atomic::{AtomicU64, Ordering};

    const OFFSET: i32 = 480;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-review-store-{tag}-{}-{}",
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

    fn noon(date: &str) -> f64 {
        let year: i64 = date[0..4].parse().unwrap();
        let month: u32 = date[5..7].parse().unwrap();
        let day: u32 = date[8..10].parse().unwrap();
        crate::worklog::days_from_civil(year, month, day) as f64 * 86_400.0 + 12.0 * 3_600.0
    }

    fn record(root: &Path, date: &str, title: &str, body: &str, draft: bool, now: f64) -> WorkRecord {
        worklog_records::create_record(
            root,
            &RecordDraft {
                title: Some(title.to_string()),
                record_date: Some(date.to_string()),
                body: body.to_string(),
                draft,
                ..Default::default()
            },
            now,
            OFFSET,
        )
        .unwrap()
    }

    fn task(root: &Path, title: &str, due: Option<&str>, now: f64) -> Task {
        task_store::create_task(
            root,
            &TaskDraft {
                title: title.to_string(),
                due_date: due.map(str::to_string),
                ..Default::default()
            },
            now,
        )
        .unwrap()
    }

    fn done_task(root: &Path, title: &str, now: f64) -> Task {
        let created = task(root, title, None, now);
        task_store::transition_task(root, &created.id, task_store::STATE_IN_PROGRESS, None, now + 1.0)
            .unwrap();
        task_store::transition_task(root, &created.id, task_store::STATE_DONE, None, now + 2.0)
            .unwrap()
    }

    fn weekly_text(record_id: &str, task_id: &str) -> String {
        format!(
            "## 本周期工作概况\n整理材料 #{record_id}。\n\n\
## 重点工作事项\n推进任务 #{task_id}。\n\n\
## 工作成果与完成情况\n完成记录 #{record_id}。\n\n\
## 问题与不足\n材料仍有缺项 #{record_id}。\n\n\
## 经验与思考\n先守住事实 #{record_id}。\n\n\
## 未完成事项\n任务未收尾 #{task_id}。\n\n\
## 下周期建议\n继续核对 #{record_id}。"
        )
    }

    fn text_for_period(period: &str, record_id: &str, task_id: &str) -> String {
        let sections = sections_for(period).unwrap();
        sections
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let id = if index % 2 == 0 { record_id } else { task_id };
                format!("## {title}\n本段只写材料事实 #{id}。")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    #[test]
    fn collect_period_filters_range_and_draft() {
        let temp = TempRoot::new("collect");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let b = record(temp.root(), "2026-11-10", "乙", "月中", false, now + 60.0);
        let draft = record(temp.root(), "2026-11-10", "草稿", "没写完", true, now + 90.0);
        record(temp.root(), "2026-10-31", "上月", "范围外", false, now);
        let bundle = collect_period(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        assert_eq!(bundle.records.len(), 2);
        assert_eq!(bundle.records[0].id, a.id);
        assert_eq!(bundle.records[1].id, b.id);
        assert!(!bundle.records.iter().any(|item| item.id == draft.id));
    }

    #[test]
    fn collect_period_counts_task_states_without_writing() {
        let temp = TempRoot::new("stats");
        let now = noon("2026-11-11");
        done_task(temp.root(), "已办", now);
        task(temp.root(), "在办", None, now + 1.0);
        task(temp.root(), "逾期", Some("2000-01-01"), now + 2.0);
        let before = task_store::list_tasks(temp.root(), &TaskFilter::default()).unwrap().items.len();
        let bundle = collect_period(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        assert_eq!(bundle.task_stats.completed, 1);
        assert_eq!(bundle.task_stats.uncompleted, 2);
        assert_eq!(bundle.task_stats.overdue, 1);
        let after = task_store::list_tasks(temp.root(), &TaskFilter::default()).unwrap().items.len();
        assert_eq!(before, after, "汇集只统计，不得创建任务");
    }

    #[test]
    fn collect_period_lists_prior_review_titles() {
        let temp = TempRoot::new("prior");
        let now = noon("2026-11-11");
        let october = noon("2026-10-15");
        let a = record(temp.root(), "2026-10-08", "甲", "十月材料", false, october);
        let t = task(temp.root(), "在办", None, october);
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-10-01",
            "2026-10-31",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            october,
        )
        .unwrap();
        let bundle = collect_period(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        assert_eq!(bundle.prior_reviews.len(), 1);
        assert!(bundle.prior_reviews[0].contains("2026-10-01"));
    }

    #[test]
    fn material_contains_records_tasks_and_counts() {
        let temp = TempRoot::new("material");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初材料", false, now);
        let t = task(temp.root(), "在办", None, now);
        let bundle = collect_period(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        assert!(bundle.material.contains(&a.id));
        assert!(bundle.material.contains(&t.id));
        assert!(bundle.material.contains("已完成 0"));
        assert!(bundle.material.contains("未完成 1"));
        assert!(bundle.material.contains("草稿已排除"));
    }

    #[test]
    fn write_persists_schema_fields_and_draft_status() {
        let temp = TempRoot::new("write");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let receipt = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        assert_eq!(receipt.status, STATUS_DRAFT);
        assert!(receipt.backup.is_none());
        assert_eq!(receipt.record_ids, vec![a.id.clone()]);
        assert_eq!(receipt.task_ids, vec![t.id.clone()]);
        let raw = fs::read_to_string(&receipt.path).unwrap();
        assert!(raw.contains("schema: \"omnidesk.period-review.v1\""));
        assert!(raw.contains("period: \"monthly\""));
        assert!(raw.contains("range: \"2026-11-01 ~ 2026-11-30\""));
        assert!(raw.contains("status: \"草稿\""));
        assert!(raw.contains("generated_at:"));
    }

    #[test]
    fn write_rejects_unknown_record_id_without_touching_disk() {
        let temp = TempRoot::new("bad-record");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let target = review_path(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        let bad = text_for_period(PERIOD_MONTHLY, &a.id, &t.id).replace(&a.id, "wr-999-1-1");
        let err = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &bad,
            now,
        )
        .unwrap_err();
        assert!(err.contains("不存在的记录编号"), "{err}");
        assert!(!target.exists());
    }

    #[test]
    fn write_rejects_unknown_task_id() {
        let temp = TempRoot::new("bad-task");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let bad = text_for_period(PERIOD_MONTHLY, &a.id, &t.id).replace(&t.id, "t-999-1-1");
        let err = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &bad,
            now,
        )
        .unwrap_err();
        assert!(err.contains("不存在的任务编号"), "{err}");
    }

    #[test]
    fn write_rejects_missing_section_for_period_depth() {
        let temp = TempRoot::new("missing");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let text = text_for_period(PERIOD_QUARTERLY, &a.id, &t.id)
            .replace("## 跨月对照", "## 随手对照");
        let err = write_review(
            temp.root(),
            PERIOD_QUARTERLY,
            "2026-10-01",
            "2026-12-31",
            &text,
            now,
        )
        .unwrap_err();
        assert!(err.contains("缺少段落"), "{err}");
    }

    #[test]
    fn write_rejects_fabricated_empty_period() {
        let temp = TempRoot::new("empty");
        let now = noon("2026-11-11");
        let sections = sections_for(PERIOD_WEEKLY).unwrap();
        let text = sections
            .iter()
            .map(|title| format!("## {title}\n本周做了很多事。"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let err = write_review(
            temp.root(),
            PERIOD_WEEKLY,
            "2026-11-09",
            "2026-11-15",
            &text,
            now,
        )
        .unwrap_err();
        assert!(err.contains("本周期无记录"), "{err}");
    }

    #[test]
    fn write_accepts_honest_empty_period() {
        let temp = TempRoot::new("honest-empty");
        let now = noon("2026-11-11");
        let sections = sections_for(PERIOD_ANNUAL).unwrap();
        let text = sections
            .iter()
            .map(|title| format!("## {title}\n本周期无记录。"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let receipt = write_review(
            temp.root(),
            PERIOD_ANNUAL,
            "2026-01-01",
            "2026-12-31",
            &text,
            now,
        )
        .unwrap();
        assert!(receipt.record_ids.is_empty());
        assert!(receipt.task_ids.is_empty());
    }

    #[test]
    fn regenerating_draft_backs_up_old_version() {
        let temp = TempRoot::new("regen");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let first = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        let old = fs::read_to_string(&first.path).unwrap();
        let second = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id).replace("本段只写材料事实", "本段从材料复核"),
            now + 3600.0,
        )
        .unwrap();
        let backup = second.backup.clone().unwrap();
        assert!(backup.contains(".bak"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), old);
    }

    #[test]
    fn confirmed_review_is_never_overwritten() {
        let temp = TempRoot::new("confirm-guard");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        let receipt = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        let confirmed = confirm_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            now + 60.0,
        )
        .unwrap();
        assert_eq!(confirmed.status, STATUS_CONFIRMED);
        let frozen = fs::read_to_string(&receipt.path).unwrap();
        assert!(frozen.contains("confirmed_at:"));
        let err = write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id).replace("本段只写材料事实", "试图覆盖"),
            now + 120.0,
        )
        .unwrap_err();
        assert!(err.contains("已确认"), "{err}");
        assert_eq!(fs::read_to_string(&receipt.path).unwrap(), frozen, "确认版字节必须不变");
    }

    #[test]
    fn confirm_is_idempotent_and_sets_timestamp() {
        let temp = TempRoot::new("confirm-idem");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-10", "甲", "本周材料", false, now);
        let t = task(temp.root(), "在办", None, now);
        write_review(
            temp.root(),
            PERIOD_WEEKLY,
            "2026-11-09",
            "2026-11-15",
            &text_for_period(PERIOD_WEEKLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        let first = confirm_review(
            temp.root(),
            PERIOD_WEEKLY,
            "2026-11-09",
            "2026-11-15",
            now + 30.0,
        )
        .unwrap();
        assert!(first.confirmed_at.is_some());
        let second = confirm_review(
            temp.root(),
            PERIOD_WEEKLY,
            "2026-11-09",
            "2026-11-15",
            now + 60.0,
        )
        .unwrap();
        assert_eq!(second.confirmed_at, first.confirmed_at, "重复确认不得改时间");
    }

    #[test]
    fn read_marks_stale_when_record_or_task_is_gone() {
        let temp = TempRoot::new("stale");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        assert!(!read_review(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap().stale);
        worklog_records::delete_record(temp.root(), &a.id, now + 10.0).unwrap();
        task_store::delete_task(temp.root(), &t.id, now + 10.0).unwrap();
        let stale = read_review(temp.root(), PERIOD_MONTHLY, "2026-11-01", "2026-11-30").unwrap();
        assert!(stale.stale);
        assert_eq!(stale.missing_record_ids, vec![a.id]);
        assert_eq!(stale.missing_task_ids, vec![t.id]);
    }

    #[test]
    fn period_depth_matches_skill_templates() {
        assert_eq!(sections_for(PERIOD_WEEKLY).unwrap().len(), 7);
        assert_eq!(sections_for(PERIOD_MONTHLY).unwrap().len(), 9);
        assert_eq!(sections_for(PERIOD_QUARTERLY).unwrap().len(), 11);
        assert_eq!(sections_for(PERIOD_ANNUAL).unwrap().len(), 13);
        assert!(sections_for(PERIOD_MONTHLY).unwrap().contains(&"数据汇总"));
        assert!(sections_for(PERIOD_QUARTERLY).unwrap().contains(&"跨月对照"));
        assert!(sections_for(PERIOD_ANNUAL).unwrap().contains(&"全年脉络"));
        assert!(sections_for(PERIOD_ANNUAL).unwrap().contains(&"长期建议"));
    }

    #[test]
    fn invalid_period_or_range_is_rejected() {
        let temp = TempRoot::new("bad-input");
        assert!(parse_period("daily").is_err());
        assert!(review_path(temp.root(), PERIOD_MONTHLY, "2026-11-30", "2026-11-01").is_err());
        assert!(collect_period(temp.root(), PERIOD_MONTHLY, "2026-11-1", "2026-11-30").is_err());
    }

    #[test]
    fn list_reviews_reads_documents_and_counts_broken() {
        let temp = TempRoot::new("archive-list");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let weekly_record = record(temp.root(), "2026-11-10", "乙", "本周", false, now + 1.0);
        let t = task(temp.root(), "在办", None, now);
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        write_review(
            temp.root(),
            PERIOD_WEEKLY,
            "2026-11-09",
            "2026-11-15",
            &text_for_period(PERIOD_WEEKLY, &weekly_record.id, &t.id),
            now + 2.0,
        )
        .unwrap();
        let broken_dir = temp.root().join(REVIEW_DIR).join(PERIOD_MONTHLY);
        fs::write(
            broken_dir.join("2026-12-01_2026-12-31.md"),
            "这不是复盘 frontmatter",
        )
        .unwrap();

        let listing = list_reviews(temp.root()).unwrap();
        assert_eq!(listing.items.len(), 2, "坏文件不进入列表");
        assert_eq!(listing.broken, 1, "坏文件计入 broken");
        assert_eq!(listing.items[0].period, PERIOD_MONTHLY);
        assert_eq!(listing.items[0].range, "2026-11-01 ~ 2026-11-30");
        assert_eq!(listing.items[0].status.as_deref(), Some(STATUS_DRAFT));
        assert_eq!(listing.items[0].record_ids, vec![a.id.clone()]);
        assert_eq!(listing.items[1].period, PERIOD_WEEKLY);
    }

    #[test]
    fn list_reviews_skips_backups_and_unknown_periods() {
        let temp = TempRoot::new("archive-skip");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let t = task(temp.root(), "在办", None, now);
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id).replace("本段只写材料事实", "本段复核材料"),
            now + 60.0,
        )
        .unwrap();
        let unknown = temp.root().join(REVIEW_DIR).join("daily");
        fs::create_dir_all(&unknown).unwrap();
        fs::write(unknown.join("2026-11-01_2026-11-30.md"), "任意内容").unwrap();

        let listing = list_reviews(temp.root()).unwrap();
        assert_eq!(listing.items.len(), 1, "备份与未知周期都不进入列表");
        assert_eq!(listing.broken, 0, "未知周期目录不当作坏文件");
    }

    #[test]
    fn record_backlinks_lists_referencing_reviews_only() {
        let temp = TempRoot::new("backlink-review");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let other = record(temp.root(), "2026-11-03", "乙", "未引用", false, now + 1.0);
        let t = task(temp.root(), "在办", None, now);
        write_review(
            temp.root(),
            PERIOD_MONTHLY,
            "2026-11-01",
            "2026-11-30",
            &text_for_period(PERIOD_MONTHLY, &a.id, &t.id),
            now,
        )
        .unwrap();

        let record_path = worklog_records::record_path(temp.root(), &a.id).unwrap();
        let before = fs::read_to_string(&record_path).unwrap();
        let links = record_backlinks(temp.root(), &a.id).unwrap();
        assert_eq!(fs::read_to_string(&record_path).unwrap(), before, "反链扫描不得改记录文件");
        assert_eq!(links.items.len(), 1);
        assert_eq!(links.items[0].kind, "review");
        assert_eq!(links.items[0].period.as_deref(), Some(PERIOD_MONTHLY));
        assert_eq!(links.items[0].status.as_deref(), Some(STATUS_DRAFT));
        assert_eq!(links.items[0].range.as_deref(), Some("2026-11-01 ~ 2026-11-30"));

        let empty = record_backlinks(temp.root(), &other.id).unwrap();
        assert!(empty.items.is_empty(), "未被引用的记录不得出现反链");
    }

    #[test]
    fn record_backlinks_follows_writing_selected_materials() {
        let temp = TempRoot::new("backlink-writing");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let material = material_store::create_material(
            temp.root(),
            &MaterialDraft {
                title: "月报素材".to_string(),
                content: "甲记录的要点".to_string(),
                material_type: "工作事实".to_string(),
                source_type: SOURCE_WORK_RECORD.to_string(),
                source_id: a.id.clone(),
                source_quote: "月初".to_string(),
                ..Default::default()
            },
            now,
        )
        .unwrap();
        writing_store::create_writing_task(
            temp.root(),
            &WritingTaskDraft {
                title: "十一月月报".to_string(),
                document_type: TYPE_MONTHLY_SUMMARY.to_string(),
                writing_period: "2026-11".to_string(),
                selected_material_ids: vec![material.material_id.clone()],
                ..Default::default()
            },
            now + 1.0,
        )
        .unwrap();

        let links = record_backlinks(temp.root(), &a.id).unwrap();
        let writing: Vec<&RecordBacklink> =
            links.items.iter().filter(|item| item.kind == "writing").collect();
        assert_eq!(writing.len(), 1);
        assert_eq!(writing[0].title.as_deref(), Some("十一月月报"));
        assert_eq!(writing[0].range.as_deref(), Some("2026-11"));
        assert!(writing[0].status.is_some());
    }

    #[test]
    fn record_backlinks_counts_broken_review_files() {
        let temp = TempRoot::new("backlink-broken");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-02", "甲", "月初", false, now);
        let dir = temp.root().join(REVIEW_DIR).join(PERIOD_MONTHLY);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("2026-11-01_2026-11-30.md"), "frontmatter 坏了").unwrap();
        let links = record_backlinks(temp.root(), &a.id).unwrap();
        assert!(links.items.is_empty());
        assert_eq!(links.broken, 1);
    }

    #[test]
    fn empty_listing_and_bad_record_id_is_rejected() {
        let temp = TempRoot::new("archive-empty");
        let listing = list_reviews(temp.root()).unwrap();
        assert!(listing.items.is_empty());
        assert_eq!(listing.broken, 0);
        assert!(record_backlinks(temp.root(), "../wr-1").is_err());
    }
}
