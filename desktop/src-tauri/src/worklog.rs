//! 工作记录确定性落盘（OD-ARCH-089 S2）。
//!
//! 事实：在 OD-ARCH-089 之前，`库/工作记录/` 的创建与追加**只存在于**模型自行调工具的
//! 行为里，产品层没有任何保证。模型一旦漏写或改写，用户的随手记就没了。
//! 本模块把「记下来」变成宿主职责：模型负责追问缺项，写盘由这里确定性完成。
//!
//! 硬约束：
//!   * **只追加**：`OpenOptions::append(true)`，绝不 truncate、绝不重写既有字节；
//!   * 同一天第二次记录另起 `## YYYY-MM-DD（续）` 段，不合并、不改旧行；
//!   * 周次按**本地时区**的 ISO 周（周一为首日），偏移复用调度配置的 `utc_offset_minutes`；
//!   * 写入前过红线扫描；命中即拒绝，不静默落盘；身份证/手机号按规则掩码后才落盘。
//!
//! 证据等级：本文件为 static（源码事实）；`#[cfg(test)]` 部分为 script（cargo test 运行）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 工作记录目录名（库根下）。
pub const WORKLOG_DIR: &str = "工作记录";

/// 周报目录名（库根下，由 `skills/memory-report/SKILL.md` 约定）。
pub const REPORT_DIR: &str = "周报";

/// 条目去向：四选一，与 `skills/memory-agent/SKILL.md` 写死的取值一致。
pub const DESTINATIONS: &[&str] = &["本周报引用", "知识卡", "inbox", "归档"];

/// 默认去向：随手记先算「本周报引用」，周报侧再按实质进展筛。
pub const DEFAULT_DESTINATION: &str = "本周报引用";

/// 一次追加的回执。字段直接序列化给前端，前端据此显示真实结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppendReceipt {
    pub ok: bool,
    /// 实际写入的绝对路径（失败时为尝试写入的路径）。
    pub path: String,
    /// ISO 周，形如 `2026-W43`。
    pub week: String,
    /// 本地日期，形如 `2026-10-26`。
    pub date: String,
    /// 本次是否发生了追加（false = 被拒绝或失败）。
    pub appended: bool,
    /// 本次是否新建了当周文件。
    pub was_new_file: bool,
    /// 本次段落是否用了「（续）」。
    pub continued: bool,
    /// 落盘文本是否被掩码过（只报类别与数量，不回显原文）。
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
    pub error: Option<String>,
}

impl AppendReceipt {
    fn failure(path: &Path, week: &str, date: &str, error: String) -> Self {
        Self {
            ok: false,
            path: path.display().to_string(),
            week: week.to_string(),
            date: date.to_string(),
            appended: false,
            was_new_file: false,
            continued: false,
            masked: false,
            masked_id_card: 0,
            masked_phone: 0,
            error: Some(error),
        }
    }
}

// ---------------------------------------------------------------------------
// 日期与 ISO 周（不引入 chrono；沿用 task_shell 的 Hinnant 口径）
// ---------------------------------------------------------------------------

/// 由天数（1970-01-01 起）换算 (年, 月, 日)。Howard Hinnant 的 civil_from_days。
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// (年, 月, 日) → 天数（1970-01-01 起）。与 `civil_from_days` 互逆。
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// ISO 周：返回 (ISO 年, 周次)。以该周周四所在年份为准，周一为首日。
pub fn iso_week_of_days(days: i64) -> (i64, u32) {
    // 1970-01-01 是星期四。(days + 3) % 7 + 1 → 1=周一 … 7=周日。
    let weekday = (days + 3).rem_euclid(7) + 1;
    let thursday = days - (weekday - 1) + 3;
    let (year, _, _) = civil_from_days(thursday);
    let jan1 = days_from_civil(year, 1, 1);
    let week = (thursday - jan1) / 7 + 1;
    (year, week as u32)
}

/// 由 Unix 秒 + 本地时区偏移得到本地日期与 ISO 周。
pub fn local_week(now: f64, offset_minutes: i32) -> (i64, u32, u32, i64) {
    let secs = now.floor() as i64 + offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (iy, iw) = iso_week_of_days(days);
    (y, m, d, (iy as i64) * 100 + iw as i64)
}

/// `YYYY-Www` 形式的周标签。
pub fn week_label(now: f64, offset_minutes: i32) -> String {
    let secs = now.floor() as i64 + offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let (y, w) = iso_week_of_days(days);
    format!("{:04}-W{:02}", y, w)
}

/// `YYYY-MM-DD` 形式的本地日期。
pub fn date_label(now: f64, offset_minutes: i32) -> String {
    let secs = now.floor() as i64 + offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// 当周工作记录文件路径。`week` 形如 `2026-W43`。
pub fn week_path(library_root: &Path, week: &str) -> PathBuf {
    library_root.join(WORKLOG_DIR).join(format!("{week}.md"))
}

// ---------------------------------------------------------------------------
// 追加
// ---------------------------------------------------------------------------

/// 校验周标签形状：`YYYY-Www`，周次 01–53。防止路径穿越与拼错。
pub fn assert_week_label(week: &str) -> Result<(), String> {
    let bytes = week.as_bytes();
    let ok = bytes.len() == 8
        && bytes[..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5] == b'W'
        && bytes[6..8].iter().all(|b| b.is_ascii_digit());
    if !ok {
        return Err(format!("周标签格式不正确（应为 YYYY-Www）：{week}"));
    }
    let w: u32 = week[6..8].parse().map_err(|_| format!("周次无法解析：{week}"))?;
    if !(1..=53).contains(&w) {
        return Err(format!("周次超出范围 01–53：{week}"));
    }
    Ok(())
}

/// 渲染本次追加的段落。
fn render_block(date: &str, text: &str, destination: &str, already_has_date: bool) -> String {
    let mut out = String::new();
    if already_has_date {
        out.push_str(&format!("## {date}（续）\n\n"));
    } else {
        out.push_str(&format!("## {date}\n\n"));
    }
    out.push_str(&format!("- **事**：{}\n", text.trim()));
    out.push_str(&format!("  **去向**：{}\n\n", destination.trim()));
    out
}

/// 追加一条随手记到当周文件。
///
/// `text` 是用户原文（已过红线与掩码，由调用方决定）；本函数只负责写盘与回执。
pub fn append_entry(
    library_root: &Path,
    week: &str,
    date: &str,
    text: &str,
    destination: &str,
) -> Result<(AppendReceipt, String), String> {
    assert_week_label(week)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("记录内容为空，未写入".to_string());
    }
    let dest = if DESTINATIONS.contains(&destination.trim()) {
        destination.trim()
    } else {
        DEFAULT_DESTINATION
    };
    let dir = library_root.join(WORKLOG_DIR);
    let path = dir.join(format!("{week}.md"));

    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("读取当周工作记录失败（{}）：{}", path.display(), e)),
    };
    let was_new_file = existing.is_none();
    let before_len = existing.as_ref().map(|s| s.len()).unwrap_or(0);

    // 头部：新建文件时写周标题；既有文件一字不改。
    let mut payload = String::new();
    if was_new_file {
        payload.push_str(&format!("# {week}\n\n"));
    }
    let date_heading = format!("## {date}");
    let already_has_date = existing
        .as_ref()
        .map(|t| {
            t.lines().any(|line| {
                let line = line.trim_end();
                line == date_heading
                    || line.starts_with(&format!("{date_heading}（"))
                    || line.starts_with(&format!("{date_heading}("))
            })
        })
        .unwrap_or(false);
    payload.push_str(&render_block(date, trimmed, dest, already_has_date));

    fs::create_dir_all(&dir).map_err(|e| format!("创建工作记录目录失败（{}）：{}", dir.display(), e))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开工作记录失败（{}）：{}", path.display(), e))?;
    file.write_all(payload.as_bytes())
        .map_err(|e| format!("写入工作记录失败（{}）：{}", path.display(), e))?;
    file.flush()
        .map_err(|e| format!("刷新工作记录失败（{}）：{}", path.display(), e))?;

    // 自证：旧字节必须一字未变，文件只增长。
    let after = fs::read_to_string(&path)
        .map_err(|e| format!("回读工作记录失败（{}）：{}", path.display(), e))?;
    if let Some(before) = existing.as_ref() {
        if !after.starts_with(before.as_str()) {
            return Err(format!(
                "工作记录疑似被重写：{} 的原有内容发生变化，请人工核对",
                path.display()
            ));
        }
    }
    if after.len() != before_len + payload.len() {
        return Err(format!(
            "工作记录写入长度不符（{} → {}，写入 {} 字节），疑似覆盖",
            before_len,
            after.len(),
            payload.len()
        ));
    }

    Ok((
        AppendReceipt {
            ok: true,
            path: path.display().to_string(),
            week: week.to_string(),
            date: date.to_string(),
            appended: true,
            was_new_file,
            continued: already_has_date,
            masked: false,
            masked_id_card: 0,
            masked_phone: 0,
            error: None,
        },
        payload,
    ))
}

// ---------------------------------------------------------------------------
// 读取（周报用「有米」）
// ---------------------------------------------------------------------------

/// 读当周工作记录的原始内容。文件不存在时 `content` 为空、`exists` 为 false。
pub fn read_week(library_root: &Path, week: &str) -> Result<(bool, String, PathBuf), String> {
    assert_week_label(week)?;
    let path = week_path(library_root, week);
    match fs::read_to_string(&path) {
        Ok(text) => Ok((true, text, path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((false, String::new(), path)),
        Err(e) => Err(format!("读取工作记录失败（{}）：{}", path.display(), e)),
    }
}

// ---------------------------------------------------------------------------
// 归档列表（OD-ARCH-091 W1/W3）
// ---------------------------------------------------------------------------

/// 一个周文件或周报文件的元数据。只列文件系统事实，不读正文、不调模型。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WeekFile {
    /// ISO 周标签，形如 `2026-W44`。
    pub week: String,
    /// 文件名（不含目录）。
    pub file_name: String,
    /// 绝对路径。
    pub path: String,
    pub bytes: u64,
    /// 修改时间（Unix 秒，取不到时为 0）。
    pub modified_at: f64,
}

/// 列目录下所有 `YYYY-Www.md`。文件名不合规的**跳过并计数**，不猜、不改名。
fn scan_week_files(dir: &Path) -> Result<(Vec<WeekFile>, u64), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // 目录还不存在 = 一条都没有，不是错误。
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(format!("读取目录失败（{}）：{}", dir.display(), e)),
    };
    let mut out = Vec::new();
    let mut skipped = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => {
                skipped += 1;
                continue;
            }
        };
        let stem = match name.strip_suffix(".md") {
            Some(s) => s.to_string(),
            None => {
                skipped += 1;
                continue;
            }
        };
        if assert_week_label(&stem).is_err() {
            skipped += 1;
            continue;
        }
        let meta = entry.metadata().ok();
        out.push(WeekFile {
            week: stem,
            file_name: name,
            path: path.display().to_string(),
            bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            modified_at: meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
        });
    }
    // 新的在前：周标签字典序与时间序一致。
    out.sort_by(|a, b| b.week.cmp(&a.week));
    Ok((out, skipped))
}

/// 列 `库/工作记录/` 下的全部周文件。
pub fn list_weeks(library_root: &Path) -> Result<(Vec<WeekFile>, u64), String> {
    scan_week_files(&library_root.join(WORKLOG_DIR))
}

/// 列 `库/周报/` 下的全部周报。文件名形如 `2026-W44-周报.md`，可选 `-2` 后缀。
pub fn list_reports(library_root: &Path) -> Result<(Vec<WeekFile>, u64), String> {
    let dir = library_root.join(REPORT_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(format!("读取周报目录失败（{}）：{}", dir.display(), e)),
    };
    let mut out = Vec::new();
    let mut skipped = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => {
                skipped += 1;
                continue;
            }
        };
        let weeks = match report_file_week(&name) {
            Some(week) => week,
            None => {
                skipped += 1;
                continue;
            }
        };
        let meta = entry.metadata().ok();
        out.push(WeekFile {
            week: weeks,
            file_name: name,
            path: path.display().to_string(),
            bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            modified_at: meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
        });
    }
    out.sort_by(|a, b| b.week.cmp(&a.week).then(b.file_name.cmp(&a.file_name)));
    Ok((out, skipped))
}

/// 从周报文件名取 ISO 周：`2026-W44-周报.md`、`2026-W44-周报-2.md` → `2026-W44`。
pub fn report_file_week(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".md")?;
    let week = stem.get(..8)?;
    if assert_week_label(week).is_err() {
        return None;
    }
    let rest = &stem[8..];
    // 只接受 `-周报` 与 `-周报-N`，其余一律当非法，不猜。
    let ok = match rest.strip_prefix("-周报") {
        Some("") => true,
        Some(tail) => match tail.strip_prefix('-') {
            Some(n) => !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()),
            None => false,
        },
        None => false,
    };
    if ok {
        Some(week.to_string())
    } else {
        None
    }
}

/// 读单个周报正文。`file_name` 必须先通过 `report_file_week` 校验，防止路径穿越。
pub fn read_report(library_root: &Path, file_name: &str) -> Result<(bool, String, PathBuf), String> {
    if report_file_week(file_name).is_none() {
        return Err(format!("周报文件名不合规，拒绝读取：{file_name}"));
    }
    let path = library_root.join(REPORT_DIR).join(file_name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok((true, text, path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((false, String::new(), path)),
        Err(e) => Err(format!("读取周报失败（{}）：{}", path.display(), e)),
    }
}

// ---------------------------------------------------------------------------
// 测试：追加不覆盖、同日续段、跨周命名
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "od089-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn iso_week_matches_known_dates() {
        // 2026-10-26 是周一，属 ISO 2026-W44。
        let days = days_from_civil(2026, 10, 26);
        assert_eq!(iso_week_of_days(days), (2026, 44));
        // 2026-01-01 是周四，属 2026-W01。
        let days = days_from_civil(2026, 1, 1);
        assert_eq!(iso_week_of_days(days), (2026, 1));
        // 2021-01-01 是周五，按 ISO 属 2020-W53。
        let days = days_from_civil(2021, 1, 1);
        assert_eq!(iso_week_of_days(days), (2020, 53));
    }

    #[test]
    fn civil_roundtrip() {
        for _ in 0..1 {
            let days = days_from_civil(2026, 2, 28);
            assert_eq!(civil_from_days(days), (2026, 2, 28));
            let days = days_from_civil(2026, 3, 1);
            assert_eq!(civil_from_days(days), (2026, 3, 1));
            let days = days_from_civil(2024, 2, 29);
            assert_eq!(civil_from_days(days), (2024, 2, 29));
        }
    }

    #[test]
    fn append_never_rewrites_existing_bytes() {
        let root = temp_root("append");
        let (r1, _) = append_entry(&root, "2026-W44", "2026-10-26", "第一件事", "本周报引用").unwrap();
        assert!(r1.appended && r1.was_new_file && !r1.continued);
        let path = root.join(WORKLOG_DIR).join("2026-W44.md");
        let first = fs::read_to_string(&path).unwrap();

        let (r2, _) = append_entry(&root, "2026-W44", "2026-10-26", "第二件事", "本周报引用").unwrap();
        assert!(r2.appended && !r2.was_new_file && r2.continued);
        let second = fs::read_to_string(&path).unwrap();

        assert!(second.starts_with(&first), "旧字节被改动");
        assert_eq!(second.matches("- **事**：").count(), 2);
        assert!(second.contains("## 2026-10-26（续）"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn different_days_do_not_use_continued_marker() {
        let root = temp_root("days");
        append_entry(&root, "2026-W44", "2026-10-26", "周一的事", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-27", "周二的事", "本周报引用").unwrap();
        let text = fs::read_to_string(root.join(WORKLOG_DIR).join("2026-W44.md")).unwrap();
        assert!(!text.contains("（续）"));
        assert_eq!(text.matches("- **事**：").count(), 2);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn week_boundary_splits_into_two_files() {
        let root = temp_root("weeks");
        // 2026-10-25 是周日（W43 最后一天），2026-10-26 是周一（W44 第一天）。
        let (a, _) = append_entry(&root, "2026-W43", "2026-10-25", "周日", "本周报引用").unwrap();
        let (b, _) = append_entry(&root, "2026-W44", "2026-10-26", "周一", "本周报引用").unwrap();
        assert!(a.path.ends_with("2026-W43.md"));
        assert!(b.path.ends_with("2026-W44.md"));
        assert!(b.was_new_file);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_bad_week_and_empty_text() {
        assert!(assert_week_label("2026-W44").is_ok());
        assert!(assert_week_label("../../etc/passwd").is_err());
        assert!(assert_week_label("2026-W99").is_err());
        let root = temp_root("reject");
        assert!(append_entry(&root, "2026-W44", "2026-10-26", "   ", "inbox").is_err());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn read_missing_week_is_not_an_error() {
        let root = temp_root("read");
        let (exists, content, _) = read_week(&root, "2026-W44").unwrap();
        assert!(!exists);
        assert!(content.is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn list_weeks_is_empty_when_dir_missing() {
        let root = temp_root("listempty");
        let (weeks, skipped) = list_weeks(&root).unwrap();
        assert!(weeks.is_empty());
        assert_eq!(skipped, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn list_weeks_sorts_newest_first_and_skips_bad_names() {
        let root = temp_root("listsort");
        append_entry(&root, "2026-W43", "2026-10-20", "上周的事", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-26", "这周的事", "本周报引用").unwrap();
        let dir = root.join(WORKLOG_DIR);
        // 不合规文件名：不猜、不改名，只跳过并计数。
        fs::write(dir.join("随手记.md"), "x").unwrap();
        fs::write(dir.join("2026-W99.md"), "x").unwrap();
        fs::write(dir.join("2026-W44.txt"), "x").unwrap();
        let (weeks, skipped) = list_weeks(&root).unwrap();
        assert_eq!(weeks.len(), 2);
        assert_eq!(weeks[0].week, "2026-W44");
        assert_eq!(weeks[1].week, "2026-W43");
        assert_eq!(skipped, 3);
        assert!(weeks[0].bytes > 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn list_reports_accepts_suffix_and_rejects_other_names() {
        let root = temp_root("reports");
        let dir = root.join(REPORT_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("2026-W44-周报.md"), "# 周报").unwrap();
        fs::write(dir.join("2026-W44-周报-2.md"), "# 周报二").unwrap();
        fs::write(dir.join("2026-W43-周报.md"), "# 上周").unwrap();
        fs::write(dir.join("随便写.md"), "x").unwrap();
        fs::write(dir.join("2026-W44-草稿.md"), "x").unwrap();
        let (reports, skipped) = list_reports(&root).unwrap();
        assert_eq!(reports.len(), 3);
        assert_eq!(skipped, 2);
        assert!(reports.iter().all(|r| r.week == "2026-W44" || r.week == "2026-W43"));
        // 同周的两份都在，且新的周在前。
        assert_eq!(reports[0].week, "2026-W44");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn report_file_week_parses_only_the_agreed_shape() {
        assert_eq!(report_file_week("2026-W44-周报.md").as_deref(), Some("2026-W44"));
        assert_eq!(report_file_week("2026-W44-周报-2.md").as_deref(), Some("2026-W44"));
        assert_eq!(report_file_week("2026-W44-周报-x.md"), None);
        assert_eq!(report_file_week("2026-W44.md"), None);
        assert_eq!(report_file_week("../../etc/passwd.md"), None);
        assert_eq!(report_file_week("2026-W99-周报.md"), None);
        assert_eq!(report_file_week("2026-W44-周报.txt"), None);
    }

    #[test]
    fn read_report_refuses_traversal_names() {
        let root = temp_root("readreport");
        assert!(read_report(&root, "../../etc/passwd").is_err());
        assert!(read_report(&root, "2026-W44-周报.md").is_ok());
        let (exists, content, _) = read_report(&root, "2026-W44-周报.md").unwrap();
        assert!(!exists);
        assert!(content.is_empty());
        fs::remove_dir_all(&root).ok();
    }
}
