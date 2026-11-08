//! 存量周文件一次性迁移（OD-WR-309）。
//!
//! 输入：library_root/工作记录/ 下的 YYYY-Www.md。
//! 输出：work/工作记录/ 中每条「事」一个记录文件；原周文件字节不动地移到
//! work/周文件存档/。
//!
//! 硬约束：
//!   * 解析认不出任何非空行就整体失败，不猜、不丢；
//!   * 先建记录并逐周核对条数，全部通过后才移动周文件；
//!   * 任一步失败：把已移动的周文件移回原位，并删除本轮新建的记录文件；
//!   * 成功后源文件已不在，重跑是空操作，因此幂等。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::sha256::sha256_hex;
use crate::worklog;
use crate::worklog_records::{self, RecordDraft};

pub const ARCHIVE_DIR: &str = "周文件存档";

#[derive(Debug, Clone, Serialize)]
pub struct PlannedEntry {
    pub date: String,
    pub text: String,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeekPlan {
    pub week: String,
    pub source: String,
    pub entries: Vec<PlannedEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationPreview {
    pub weeks: Vec<WeekPlan>,
    pub entry_count: usize,
    pub skipped: Vec<String>,
    pub archive_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationReport {
    pub ok: bool,
    pub records_created: usize,
    pub weeks_archived: usize,
    pub record_paths: Vec<String>,
    pub archived_files: Vec<String>,
    pub skipped: Vec<String>,
    pub error: Option<String>,
}

fn is_week_file(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".md")?;
    if worklog::assert_week_label(stem).is_ok() {
        Some(stem.to_string())
    } else {
        None
    }
}

fn date_heading_date(text: &str) -> Result<String, String> {
    let base = text
        .split(['（', '('])
        .next()
        .unwrap_or("")
        .trim();
    if !worklog_records::is_date_shape(base) {
        return Err(format!("日期标题不是 YYYY-MM-DD：{text}"));
    }
    Ok(base.to_string())
}

/// 解析一份周文件。认不出的非空行一律报错，避免迁移时丢内容。
fn parse_week(week: &str, content: &str) -> Result<Vec<PlannedEntry>, String> {
    let mut entries: Vec<PlannedEntry> = Vec::new();
    let mut current_date: Option<String> = None;
    let mut pending: Option<PlannedEntry> = None;
    let mut saw_header = false;

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            if saw_header || current_date.is_some() || pending.is_some() {
                return Err(format!("周文件 {week} 的标题位置不合法"));
            }
            if rest.trim() != week {
                return Err(format!(
                    "周文件 {week} 的标题是 {}，与文件名不一致",
                    rest.trim()
                ));
            }
            saw_header = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some(entry) = pending.take() {
                entries.push(entry);
            }
            current_date = Some(date_heading_date(rest.trim())?);
            continue;
        }
        if let Some(rest) = line.trim_start().strip_prefix("- **事**") {
            let rest = rest.trim_start();
            let text = rest
                .strip_prefix('：')
                .or_else(|| rest.strip_prefix(':'))
                .ok_or_else(|| format!("周文件 {week} 的事行缺少冒号：{}", line.trim()))?
                .trim()
                .to_string();
            if text.is_empty() {
                return Err(format!("周文件 {week} 有一条空记录"));
            }
            let date = current_date
                .clone()
                .ok_or_else(|| format!("周文件 {week} 的条目不在日期段下"))?;
            if let Some(entry) = pending.take() {
                entries.push(entry);
            }
            pending = Some(PlannedEntry {
                date,
                text,
                destination: None,
            });
            continue;
        }
        if let Some(rest) = line.trim_start().strip_prefix("**去向**") {
            let rest = rest.trim_start();
            let dest = rest
                .strip_prefix('：')
                .or_else(|| rest.strip_prefix(':'))
                .ok_or_else(|| format!("周文件 {week} 的去向行缺少冒号"))?
                .trim();
            let entry = pending
                .as_mut()
                .ok_or_else(|| format!("周文件 {week} 的去向行没有对应条目"))?;
            entry.destination = if dest.is_empty() {
                None
            } else {
                Some(dest.to_string())
            };
            continue;
        }
        // 正文多行：宿主原样写入，第二行起没有前缀。
        if let Some(entry) = pending.as_mut() {
            if entry.destination.is_none() {
                entry.text.push('\n');
                entry.text.push_str(line.trim());
                continue;
            }
        }
        return Err(format!(
            "周文件 {week} 含无法迁移的内容，已中止：{}",
            line.trim()
        ));
    }
    if let Some(entry) = pending.take() {
        entries.push(entry);
    }
    if !saw_header {
        return Err(format!("周文件 {week} 缺少 # {week} 标题"));
    }
    Ok(entries)
}

fn collect_weeks(library_root: &Path) -> Result<(Vec<WeekPlan>, Vec<String>), String> {
    let dir = library_root.join(worklog::WORKLOG_DIR);
    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    if !dir.exists() {
        return Ok((plans, skipped));
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取工作记录目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取工作记录目录项失败：{e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取目录项类型失败：{e}"))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(week) = is_week_file(&name) else {
            skipped.push(name);
            continue;
        };
        let path = entry.path();
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("读取周文件失败（{}）：{}", path.display(), e))?;
        let parsed = parse_week(&week, &content)?;
        plans.push(WeekPlan {
            week,
            source: path.display().to_string(),
            entries: parsed,
        });
    }
    plans.sort_by(|a, b| a.week.cmp(&b.week));
    Ok((plans, skipped))
}

fn source_label(week: &str, destination: Option<&str>) -> String {
    match destination {
        Some(dest) => format!("周文件迁移:{week}（去向：{dest}）"),
        None => format!("周文件迁移:{week}"),
    }
}

/// 只读预览：解析全部周文件，不创建、不移动任何文件。
pub fn preview(library_root: &Path, work_root: &Path) -> Result<MigrationPreview, String> {
    worklog_records::assert_work_root(work_root)?;
    let (plans, skipped) = collect_weeks(library_root)?;
    let entry_count = plans.iter().map(|plan| plan.entries.len()).sum();
    Ok(MigrationPreview {
        weeks: plans,
        entry_count,
        skipped,
        archive_dir: work_root.join(ARCHIVE_DIR).display().to_string(),
    })
}

fn remove_created(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn restore_archives(moved: &[(PathBuf, PathBuf)]) {
    for (source, target) in moved.iter().rev() {
        let _ = fs::rename(target, source);
    }
}

/// 执行迁移：建记录、核对条数、归档周文件；任一失败整体回滚。
pub fn migrate(library_root: &Path, work_root: &Path, now: f64) -> Result<MigrationReport, String> {
    let preview = preview(library_root, work_root)?;
    if preview.weeks.is_empty() {
        return Ok(MigrationReport {
            ok: true,
            records_created: 0,
            weeks_archived: 0,
            record_paths: Vec::new(),
            archived_files: Vec::new(),
            skipped: preview.skipped,
            error: None,
        });
    }
    let archive_dir = work_root.join(ARCHIVE_DIR);
    // 先做归档冲突预检，避免建完记录才发现目标已存在。
    for plan in &preview.weeks {
        let target = archive_dir.join(format!("{}.md", plan.week));
        if target.exists() {
            return Err(format!(
                "存档目标已存在，拒绝覆盖：{}（源文件未移动）",
                target.display()
            ));
        }
    }

    let mut created: Vec<PathBuf> = Vec::new();
    let mut per_week: BTreeMap<String, usize> = BTreeMap::new();
    for plan in &preview.weeks {
        let mut count = 0usize;
        for entry in &plan.entries {
            let draft = RecordDraft {
                id: None,
                title: None,
                record_date: Some(entry.date.clone()),
                category: None,
                tags: Vec::new(),
                source: Some(source_label(&plan.week, entry.destination.as_deref())),
                destination: entry
                    .destination
                    .clone()
                    .filter(|value| worklog::DESTINATIONS.contains(&value.as_str())),
                draft: false,
                body: entry.text.clone(),
                linked_task_ids: Vec::new(),
            };
            let record = match worklog_records::create_record(work_root, &draft, now, 0) {
                Ok(record) => record,
                Err(message) => {
                    remove_created(&created);
                    return Err(format!("创建迁移记录失败：{message}；已回滚本轮记录"));
                }
            };
            match worklog_records::record_path(work_root, &record.id) {
                Ok(path) => created.push(path),
                Err(message) => {
                    remove_created(&created);
                    return Err(format!("迁移记录路径不合法：{message}；已回滚本轮记录"));
                }
            }
            count += 1;
        }
        per_week.insert(plan.week.clone(), count);
    }

    for plan in &preview.weeks {
        let expected = plan.entries.len();
        if per_week.get(&plan.week).copied().unwrap_or(0) != expected {
            remove_created(&created);
            return Err(format!(
                "周文件 {} 的条数核对失败（解析 {expected} 条，写入 {} 条）；已回滚本轮记录",
                plan.week,
                per_week.get(&plan.week).copied().unwrap_or(0)
            ));
        }
    }

    if let Err(message) = fs::create_dir_all(&archive_dir) {
        remove_created(&created);
        return Err(format!("创建周文件存档目录失败：{message}；已回滚本轮记录"));
    }

    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for plan in &preview.weeks {
        let source = PathBuf::from(&plan.source);
        let target = archive_dir.join(format!("{}.md", plan.week));
        let bytes = match fs::read(&source) {
            Ok(bytes) => bytes,
            Err(e) => {
                restore_archives(&moved);
                remove_created(&created);
                return Err(format!("读取待归档周文件失败（{}）：{e}", source.display()));
            }
        };
        let hash = sha256_hex(&bytes);
        if let Err(e) = fs::rename(&source, &target) {
            restore_archives(&moved);
            remove_created(&created);
            return Err(format!(
                "移动周文件失败（{} → {}）：{e}；已回滚",
                source.display(),
                target.display()
            ));
        }
        let after = fs::read(&target).map(|b| sha256_hex(&b)).unwrap_or_default();
        if after != hash {
            let _ = fs::rename(&target, &source);
            restore_archives(&moved);
            remove_created(&created);
            return Err(format!(
                "周文件归档后字节不一致（{}），已回滚",
                target.display()
            ));
        }
        moved.push((source, target));
    }

    Ok(MigrationReport {
        ok: true,
        records_created: created.len(),
        weeks_archived: moved.len(),
        record_paths: created
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        archived_files: moved
            .iter()
            .map(|(_, target)| target.display().to_string())
            .collect(),
        skipped: preview.skipped,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_200_000.0;

    struct TempRoots {
        library: PathBuf,
        work: PathBuf,
    }

    impl TempRoots {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-wr309-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let library = base.join("library");
            let work = base.join("work");
            fs::create_dir_all(library.join("工作记录")).unwrap();
            fs::create_dir_all(&work).unwrap();
            Self { library, work }
        }

        fn write_week(&self, week: &str, content: &str) -> PathBuf {
            let path = self.library.join("工作记录").join(format!("{week}.md"));
            fs::write(&path, content).unwrap();
            path
        }
    }

    impl Drop for TempRoots {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.library.parent().unwrap());
        }
    }

    const WEEK: &str = "# 2026-W45\n\n## 2026-11-05\n\n- **事**：第一件事\n  **去向**：本周报引用\n\n## 2026-11-05（续）\n\n- **事**：第二件事\n  第二行\n  **去向**：仅记录\n";

    #[test]
    fn preview_parses_entries_without_writing() {
        let roots = TempRoots::new("preview");
        roots.write_week("2026-W45", WEEK);
        let preview = preview(&roots.library, &roots.work).unwrap();
        assert_eq!(preview.weeks.len(), 1);
        assert_eq!(preview.entry_count, 2);
        assert_eq!(preview.weeks[0].entries[0].date, "2026-11-05");
        assert_eq!(preview.weeks[0].entries[1].text, "第二件事\n第二行");
        assert_eq!(preview.weeks[0].entries[1].destination.as_deref(), Some("仅记录"));
        assert!(!roots.work.join("工作记录").exists(), "预览不得创建记录");
    }

    #[test]
    fn migrate_creates_records_and_archives_original_bytes() {
        let roots = TempRoots::new("apply");
        let source = roots.write_week("2026-W45", WEEK);
        let before = fs::read(&source).unwrap();
        let report = migrate(&roots.library, &roots.work, NOW).unwrap();
        assert!(report.ok);
        assert_eq!(report.records_created, 2);
        assert_eq!(report.weeks_archived, 1);
        let archive = roots.work.join(ARCHIVE_DIR).join("2026-W45.md");
        assert_eq!(fs::read(&archive).unwrap(), before, "归档字节必须原样");
        assert!(!source.exists(), "原周文件已移动");
        let listing = worklog_records::list_records(
            &roots.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert_eq!(listing.items.len(), 2);
        assert!(listing.items.iter().all(|record| record.record_date == "2026-11-05"));
        assert!(listing
            .items
            .iter()
            .all(|record| record.source.as_deref().unwrap_or("").starts_with("周文件迁移:2026-W45")));
        assert!(listing
            .items
            .iter()
            .any(|record| record.destination.as_deref() == Some("仅记录")));
    }

    #[test]
    fn migrate_is_idempotent_after_success() {
        let roots = TempRoots::new("idempotent");
        roots.write_week("2026-W45", WEEK);
        migrate(&roots.library, &roots.work, NOW).unwrap();
        let report = migrate(&roots.library, &roots.work, NOW + 60.0).unwrap();
        assert_eq!(report.records_created, 0);
        assert_eq!(report.weeks_archived, 0);
        let listing = worklog_records::list_records(
            &roots.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert_eq!(listing.items.len(), 2, "重跑不得新增记录");
    }

    #[test]
    fn parse_error_aborts_before_writing() {
        let roots = TempRoots::new("parse-error");
        let source = roots.write_week("2026-W45", "# 2026-W45\n\n## 2026-11-05\n\n这句没有条目前缀\n");
        let err = migrate(&roots.library, &roots.work, NOW).unwrap_err();
        assert!(err.contains("无法迁移"), "{err}");
        assert!(source.exists());
        let listing = worklog_records::list_records(
            &roots.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert!(listing.items.is_empty());
        assert!(!roots.work.join(ARCHIVE_DIR).exists());
    }

    #[test]
    fn existing_archive_target_aborts_without_records() {
        let roots = TempRoots::new("archive-collision");
        let source = roots.write_week("2026-W45", WEEK);
        fs::create_dir_all(roots.work.join(ARCHIVE_DIR)).unwrap();
        fs::write(roots.work.join(ARCHIVE_DIR).join("2026-W45.md"), "旧存档").unwrap();
        let err = migrate(&roots.library, &roots.work, NOW).unwrap_err();
        assert!(err.contains("存档目标已存在"), "{err}");
        assert!(source.exists());
        let listing = worklog_records::list_records(
            &roots.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert!(listing.items.is_empty());
    }

    #[test]
    fn archive_dir_failure_rolls_back_records() {
        let roots = TempRoots::new("archive-dir-failure");
        let source = roots.write_week("2026-W45", WEEK);
        fs::write(roots.work.join(ARCHIVE_DIR), "这是一个文件，不是目录").unwrap();
        let err = migrate(&roots.library, &roots.work, NOW).unwrap_err();
        assert!(err.contains("存档目录"), "{err}");
        assert!(source.exists(), "源文件不得移动");
        let listing = worklog_records::list_records(
            &roots.work,
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert!(listing.items.is_empty(), "本轮记录应已回滚");
    }

    #[test]
    fn invalid_week_file_names_are_skipped() {
        let roots = TempRoots::new("skip");
        roots.write_week("2026-W45", WEEK);
        fs::write(roots.library.join("工作记录").join("随手记.md"), "x").unwrap();
        fs::write(roots.library.join("工作记录").join("2026-W45.md.bak"), "x").unwrap();
        let preview = preview(&roots.library, &roots.work).unwrap();
        assert_eq!(preview.weeks.len(), 1);
        assert_eq!(preview.skipped.len(), 2);
    }

    #[test]
    fn crlf_week_file_parses_and_archives_same_bytes() {
        let roots = TempRoots::new("crlf");
        let crlf = WEEK.replace('\n', "\r\n");
        let source = roots.write_week("2026-W45", &crlf);
        let before = fs::read(&source).unwrap();
        let report = migrate(&roots.library, &roots.work, NOW).unwrap();
        assert_eq!(report.records_created, 2);
        assert_eq!(
            fs::read(roots.work.join(ARCHIVE_DIR).join("2026-W45.md")).unwrap(),
            before
        );
    }
}
