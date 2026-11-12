//! 当日摘要存储与校验（OD-WR-503）。
//!
//! 边界：
//!   * 只读当天 `record_date` 匹配且 `draft=false` 的正式记录；
//!   * 摘要由界面侧模型生成，宿主只做五段结构、引用编号与空记录声明的校验；
//!   * 落盘到 `work/摘要/YYYY-MM-DD.md`，frontmatter schema 固定；
//!   * 重新生成先备份旧文件为 `.bak`（已有备份时追加时间后缀，绝不覆盖旧备份），
//!     再原子替换主文件；不删除任何旧字节。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{self, RecordFilter, WorkRecord};

/// 摘要目录（work 根下）。
pub const SUMMARY_DIR: &str = "摘要";
pub const SUMMARY_SCHEMA: &str = "omnidesk.daily-summary.v1";
pub const STATUS_DRAFT: &str = "草稿";

/// 五段标题。顺序即写入顺序，宿主按这五个名字校验。
pub const SECTION_TITLES: [&str; 5] = [
    "今日工作概况",
    "重点事项",
    "重要思考",
    "需要跟进的问题",
    "与任务管理的关联",
];

/// 没有记录时允许的如实声明词；至少出现一个，避免模型凭空编造。
const EMPTY_MARKERS: [&str; 6] = [
    "当日无记录",
    "今日无记录",
    "当天没有记录",
    "今天没有记录",
    "暂无记录",
    "没有正式记录",
];

/// 落盘回执。
#[derive(Debug, Clone, Serialize)]
pub struct DailySummaryReceipt {
    pub ok: bool,
    pub path: String,
    pub date: String,
    pub record_ids: Vec<String>,
    pub generated_at: String,
    pub status: String,
    /// 重新生成时旧文件的备份路径；首次写入为 None。
    pub backup: Option<String>,
    pub records: usize,
}

/// 读取结果。文件不存在时 `exists=false`，不建文件、不猜内容。
#[derive(Debug, Clone, Serialize)]
pub struct DailySummaryRead {
    pub exists: bool,
    pub path: String,
    pub date: String,
    pub status: Option<String>,
    pub record_ids: Vec<String>,
    pub generated_at: Option<String>,
    pub body: String,
    /// 引用过、但当前记录库里已经找不到的记录编号。
    pub missing_record_ids: Vec<String>,
    /// 有失效引用即为 true；界面据此提示摘要待复核。
    pub stale: bool,
}

#[derive(Debug, Deserialize)]
struct SummaryFront {
    #[serde(default)]
    schema: Option<String>,
    date: String,
    #[serde(default)]
    record_ids: Vec<String>,
    #[serde(default)]
    generated_at: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// 摘要目录 `<work_root>/摘要`。
pub fn summary_dir(root: &Path) -> PathBuf {
    root.join(SUMMARY_DIR)
}

/// 摘要文件 `<work_root>/摘要/<date>.md`。日期先校验形状，调用方不能拼路径。
pub fn summary_path(root: &Path, date: &str) -> Result<PathBuf, String> {
    if !worklog_records::is_date_shape(date) {
        return Err(format!("摘要日期形状不正确（应为 YYYY-MM-DD）：{date}"));
    }
    Ok(summary_dir(root).join(format!("{date}.md")))
}

/// 汇集当天正式记录（排除草稿），按创建时间升序返回全文。
///
/// 这是唯一的数据入口：模型只应从这份材料生成摘要，宿主也用同一份集合校验引用。
pub fn collect_today(root: &Path, date: &str) -> Result<Vec<WorkRecord>, String> {
    if !worklog_records::is_date_shape(date) {
        return Err(format!("摘要日期形状不正确（应为 YYYY-MM-DD）：{date}"));
    }
    let listing = worklog_records::list_records(
        root,
        &RecordFilter {
            from_date: Some(date.to_string()),
            to_date: Some(date.to_string()),
            draft: Some(false),
            ..Default::default()
        },
    )?;
    let mut items: Vec<WorkRecord> = listing
        .items
        .into_iter()
        .filter(|record| !record.draft)
        .collect();
    items.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(items)
}

/// 校验模型交回的摘要正文，返回去掉前置 frontmatter 后的干净正文与引用编号。
fn validate_body(content: &str, records: &[WorkRecord]) -> Result<(String, Vec<String>), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("摘要正文为空，未写入".to_string());
    }
    let body = match worklog_records::split_frontmatter(trimmed) {
        Ok((_front, body)) => body,
        Err(_) => trimmed.to_string(),
    };
    if body.len() > 512 * 1024 {
        return Err("摘要正文过大（超过 512KB），未写入".to_string());
    }

    let mut sections: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut current: Option<&'static str> = None;
    let mut text = String::new();
    for line in body.lines() {
        if let Some(title) = heading_of(line) {
            if sections.contains_key(title) {
                return Err(format!("摘要段落重复：{title}，未写入"));
            }
            if let Some(previous) = current {
                sections.insert(previous, std::mem::take(&mut text));
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
        sections.insert(previous, std::mem::take(&mut text));
    }

    for title in SECTION_TITLES {
        if !sections.contains_key(title) {
            return Err(format!("摘要缺少段落：{title}，未写入"));
        }
    }
    for title in SECTION_TITLES {
        if sections
            .get(title)
            .map(|section| section.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(format!("摘要段落没有正文：{title}，未写入"));
        }
    }

    let known: BTreeMap<&str, &WorkRecord> =
        records.iter().map(|record| (record.id.as_str(), record)).collect();
    let mut ids: Vec<String> = Vec::new();
    // 正文里出现的所有 #wr-… 都必须真实存在，正文前言里的引用也不能漏检。
    for id in citation_ids(&body) {
        if !known.contains_key(id.as_str()) {
            return Err(format!(
                "摘要引用了不存在的记录编号：#{id}，未写入"
            ));
        }
        if !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    for title in SECTION_TITLES {
        let section = sections.get(title).cloned().unwrap_or_default();
        if !records.is_empty() && citation_ids(&section).is_empty() {
            return Err(format!("摘要段落没有标注引用记录：{title}，未写入"));
        }
    }

    if records.is_empty() {
        if !ids.is_empty() {
            return Err("当日没有正式记录，摘要不应引用任何记录编号，未写入".to_string());
        }
        if !EMPTY_MARKERS.iter().any(|marker| body.contains(marker)) {
            return Err("当日没有正式记录，摘要必须如实说明「当日无记录」，未写入".to_string());
        }
    }

    Ok((body.trim().to_string(), ids))
}

/// 段落标题识别：接受 `## 今日工作概况`、`**今日工作概况**`、`【今日工作概况】` 等常见写法。
fn heading_of(line: &str) -> Option<&'static str> {
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
    SECTION_TITLES.iter().find(|title| **title == value).copied()
}

/// 取出 `#wr-…` 形式的引用编号。`#标签` 不等于记录引用，直接忽略。
fn citation_ids(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('#') {
        let tail = &rest[at + 1..];
        let id: String = tail
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        let skip = id.len().max(1);
        if id.starts_with("wr-") && !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
        rest = &tail[skip..];
    }
    ids
}

fn render_summary(
    date: &str,
    record_ids: &[String],
    generated_at: &str,
    body: &str,
) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!(
        "schema: {}\n",
        worklog_records::yaml_scalar(SUMMARY_SCHEMA)
    ));
    out.push_str(&format!("date: {}\n", worklog_records::yaml_scalar(date)));
    if record_ids.is_empty() {
        out.push_str("record_ids: []\n");
    } else {
        out.push_str("record_ids:\n");
        for id in record_ids {
            out.push_str(&format!("  - {}\n", worklog_records::yaml_scalar(id)));
        }
    }
    out.push_str(&format!(
        "generated_at: {}\n",
        worklog_records::yaml_scalar(generated_at)
    ));
    out.push_str(&format!(
        "status: {}\n",
        worklog_records::yaml_scalar(STATUS_DRAFT)
    ));
    out.push_str("---\n\n");
    out.push_str(body.trim());
    out.push('\n');
    out
}

/// 旧摘要的备份路径：优先 `<date>.md.bak`；已存在时用时间戳后缀，绝不覆盖旧备份。
fn backup_path(target: &Path, now: f64) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "summary.md".to_string());
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

/// 校验并落盘一次当日摘要。
pub fn write_summary(
    root: &Path,
    date: &str,
    content: &str,
    now: f64,
) -> Result<DailySummaryReceipt, String> {
    worklog_records::assert_work_root(root)?;
    let records = collect_today(root, date)?;
    let (body, record_ids) = validate_body(content, &records)?;
    let generated_at = crate::task_shell::utc_iso8601(now);
    let rendered = render_summary(date, &record_ids, &generated_at, &body);
    let target = summary_path(root, date)?;
    fs::create_dir_all(summary_dir(root))
        .map_err(|e| format!("创建摘要目录失败（{}）：{e}", summary_dir(root).display()))?;

    let backup = if target.exists() {
        let meta = fs::symlink_metadata(&target)
            .map_err(|e| format!("读取旧摘要失败（{}）：{e}", target.display()))?;
        if !meta.file_type().is_file() {
            return Err(format!("旧摘要不是普通文件，未覆盖：{}", target.display()));
        }
        let path = backup_path(&target, now);
        fs::copy(&target, &path).map_err(|e| {
            format!(
                "备份旧摘要失败（{} → {}）：{e}；主文件未改动",
                target.display(),
                path.display()
            )
        })?;
        Some(path)
    } else {
        None
    };

    if backup.is_some() {
        worklog_records::write_replace_atomically(&target, rendered.as_bytes())?;
    } else {
        worklog_records::write_new_atomically(&target, rendered.as_bytes())?;
    }

    Ok(DailySummaryReceipt {
        ok: true,
        path: target.display().to_string(),
        date: date.to_string(),
        record_ids,
        generated_at,
        status: STATUS_DRAFT.to_string(),
        backup: backup.map(|path| path.display().to_string()),
        records: records.len(),
    })
}

/// 读取当日摘要；文件不存在是合法状态，返回 `exists=false`。
pub fn read_summary(root: &Path, date: &str) -> Result<DailySummaryRead, String> {
    worklog_records::assert_work_root(root)?;
    let path = summary_path(root, date)?;
    if !path.exists() {
        return Ok(DailySummaryRead {
            exists: false,
            path: path.display().to_string(),
            date: date.to_string(),
            status: None,
            record_ids: Vec::new(),
            generated_at: None,
            body: String::new(),
            missing_record_ids: Vec::new(),
            stale: false,
        });
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取摘要失败（{}）：{e}", path.display()))?;
    let (front_raw, body) = worklog_records::split_frontmatter(&raw)?;
    let front: SummaryFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("摘要 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != SUMMARY_SCHEMA {
            return Err(format!("摘要 schema 不认识：{schema}"));
        }
    }
    if front.date != date {
        return Err(format!(
            "摘要日期与文件名不一致：文件 {date}，frontmatter {}",
            front.date
        ));
    }
    for id in &front.record_ids {
        worklog_records::assert_record_id(id)?;
    }
    let mut missing_record_ids = Vec::new();
    for id in &front.record_ids {
        if worklog_records::read_record(root, id).is_err() {
            missing_record_ids.push(id.clone());
        }
    }
    Ok(DailySummaryRead {
        exists: true,
        path: path.display().to_string(),
        date: date.to_string(),
        status: front.status,
        record_ids: front.record_ids,
        generated_at: front.generated_at,
        body,
        stale: !missing_record_ids.is_empty(),
        missing_record_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklog_records::RecordDraft;
    use std::sync::atomic::{AtomicU64, Ordering};

    const OFFSET: i32 = 480;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-daily-summary-{tag}-{}-{}",
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

    fn record(
        root: &Path,
        date: &str,
        title: &str,
        body: &str,
        draft: bool,
        now: f64,
    ) -> WorkRecord {
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

    fn summary_text(ids: &[&str]) -> String {
        let first = ids.first().copied().unwrap_or("wr-0-0-0");
        format!(
            "## 今日工作概况\n今天完成了记录整理 #{first}。\n\n\
## 重点事项\n整理并核对材料 #{first}。\n\n\
## 重要思考\n先留住事实再归纳 #{first}。\n\n\
## 需要跟进的问题\n补充未完成事项 #{first}。\n\n\
## 与任务管理的关联\n已关联任务 #{first}。"
        )
    }

    #[test]
    fn collect_today_only_returns_formal_records() {
        let temp = TempRoot::new("collect");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let b = record(temp.root(), "2026-11-11", "乙", "下午开会", false, now + 60.0);
        let draft = record(temp.root(), "2026-11-11", "草稿", "没写完", true, now + 120.0);
        record(temp.root(), "2026-11-10", "昨天", "别的日子", false, now);
        let items = collect_today(temp.root(), "2026-11-11").unwrap();
        assert_eq!(items.len(), 2, "草稿与其他日期都不能进材料");
        assert_eq!(items[0].id, a.id);
        assert_eq!(items[1].id, b.id);
        assert!(!items.iter().any(|item| item.id == draft.id));
    }

    #[test]
    fn collect_today_empty_is_ok() {
        let temp = TempRoot::new("empty-collect");
        assert!(collect_today(temp.root(), "2026-11-11").unwrap().is_empty());
    }

    #[test]
    fn write_persists_schema_date_ids_and_draft_status() {
        let temp = TempRoot::new("write");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let receipt = write_summary(temp.root(), "2026-11-11", &summary_text(&[&a.id]), now).unwrap();
        assert_eq!(receipt.record_ids, vec![a.id.clone()]);
        assert_eq!(receipt.status, STATUS_DRAFT);
        assert!(receipt.backup.is_none());
        let raw = fs::read_to_string(&receipt.path).unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("schema: \"omnidesk.daily-summary.v1\""));
        assert!(raw.contains("status: \"草稿\""));
        assert!(raw.contains(&format!("  - \"{}\"", a.id)));
        assert!(raw.contains("## 今日工作概况"));
    }

    #[test]
    fn write_rejects_unknown_record_id_without_touching_disk() {
        let temp = TempRoot::new("unknown-id");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let target = summary_path(temp.root(), "2026-11-11").unwrap();
        let bad = summary_text(&[&a.id]).replace(&a.id, "wr-999-1-1");
        let err = write_summary(temp.root(), "2026-11-11", &bad, now).unwrap_err();
        assert!(err.contains("不存在的记录编号"), "{err}");
        assert!(!target.exists(), "校验失败时不能留下半份摘要");
    }

    #[test]
    fn write_rejects_missing_section() {
        let temp = TempRoot::new("missing-section");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let mut text = summary_text(&[&a.id]);
        text = text.replace("## 重要思考", "## 随手想法");
        let err = write_summary(temp.root(), "2026-11-11", &text, now).unwrap_err();
        assert!(err.contains("缺少段落"), "{err}");
    }

    #[test]
    fn write_rejects_section_without_citation() {
        let temp = TempRoot::new("no-citation");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let mut text = summary_text(&[&a.id]);
        text = text.replace("## 重点事项\n整理并核对材料", "## 重点事项\n整理并核对材料");
        text = text.replace(&format!("整理并核对材料 #{}", a.id), "整理并核对材料");
        let err = write_summary(temp.root(), "2026-11-11", &text, now).unwrap_err();
        assert!(err.contains("没有标注引用记录"), "{err}");
    }

    #[test]
    fn empty_records_require_honest_empty_statement() {
        let temp = TempRoot::new("empty-validate");
        let now = noon("2026-11-11");
        let fabricated = "## 今日工作概况\n今天完成了整理。\n\n\
## 重点事项\n完成了整理。\n\n\
## 重要思考\n没有记录也能总结。\n\n\
## 需要跟进的问题\n无。\n\n\
## 与任务管理的关联\n无。";
        let err = write_summary(temp.root(), "2026-11-11", &fabricated, now).unwrap_err();
        assert!(err.contains("当日无记录"), "{err}");
        let honest = "## 今日工作概况\n当日无记录。\n\n\
## 重点事项\n当日无记录。\n\n\
## 重要思考\n当日无记录。\n\n\
## 需要跟进的问题\n当日无记录。\n\n\
## 与任务管理的关联\n当日无记录。";
        let receipt = write_summary(temp.root(), "2026-11-11", honest, now).unwrap();
        assert!(receipt.record_ids.is_empty());
        assert_eq!(receipt.records, 0);
    }

    #[test]
    fn regenerate_backs_up_old_file_and_keeps_both_versions() {
        let temp = TempRoot::new("backup");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let first = write_summary(temp.root(), "2026-11-11", &summary_text(&[&a.id]), now).unwrap();
        let old = fs::read_to_string(&first.path).unwrap();
        let second_text = summary_text(&[&a.id]).replace("今天完成了记录整理", "今天完成了材料核对");
        let second =
            write_summary(temp.root(), "2026-11-11", &second_text, now + 3600.0).unwrap();
        let backup = second.backup.clone().unwrap();
        assert!(backup.contains(".bak"), "{backup}");
        assert_eq!(fs::read_to_string(&backup).unwrap(), old, "备份必须是旧版原文");
        let current = fs::read_to_string(&second.path).unwrap();
        assert!(current.contains("今天完成了材料核对"));
        assert!(!current.contains("今天完成了记录整理"), "主文件应被新版替换");
        // 再生成一次不得覆盖上一份备份。
        let third =
            write_summary(temp.root(), "2026-11-11", &summary_text(&[&a.id]), now + 7200.0).unwrap();
        let third_backup = third.backup.unwrap();
        assert_ne!(third_backup, backup);
        assert_eq!(fs::read_to_string(&backup).unwrap(), old);
        assert!(fs::read_to_string(&third_backup).unwrap().contains("今天完成了材料核对"));
    }

    #[test]
    fn read_missing_summary_reports_exists_false() {
        let temp = TempRoot::new("read-missing");
        let read = read_summary(temp.root(), "2026-11-11").unwrap();
        assert!(!read.exists);
        assert!(read.body.is_empty());
        assert!(!read.path.is_empty());
    }

    #[test]
    fn read_marks_stale_when_referenced_record_is_gone() {
        let temp = TempRoot::new("stale");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        write_summary(temp.root(), "2026-11-11", &summary_text(&[&a.id]), now).unwrap();
        let read = read_summary(temp.root(), "2026-11-11").unwrap();
        assert!(read.exists);
        assert_eq!(read.record_ids, vec![a.id.clone()]);
        assert!(!read.stale);
        worklog_records::delete_record(temp.root(), &a.id, now + 60.0).unwrap();
        let stale = read_summary(temp.root(), "2026-11-11").unwrap();
        assert!(stale.stale);
        assert_eq!(stale.missing_record_ids, vec![a.id]);
    }

    #[test]
    fn invalid_date_is_rejected_before_any_write() {
        let temp = TempRoot::new("bad-date");
        let err = write_summary(temp.root(), "2026-11-1", "x", noon("2026-11-11")).unwrap_err();
        assert!(err.contains("日期形状"), "{err}");
        assert!(summary_path(temp.root(), "2026-11-1").is_err());
    }

    #[test]
    fn frontmatter_from_model_is_stripped_before_validation() {
        let temp = TempRoot::new("strip-front");
        let now = noon("2026-11-11");
        let a = record(temp.root(), "2026-11-11", "甲", "上午整理材料", false, now);
        let content = format!(
            "---\nschema: model-output\ndate: 2026-11-11\n---\n\n{}",
            summary_text(&[&a.id])
        );
        let receipt = write_summary(temp.root(), "2026-11-11", &content, now).unwrap();
        let raw = fs::read_to_string(&receipt.path).unwrap();
        assert!(raw.contains(SUMMARY_SCHEMA));
        assert!(!raw.contains("model-output"));
    }
}
