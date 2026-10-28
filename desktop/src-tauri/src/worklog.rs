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

/// 条目去向：五选一，与 `skills/memory-agent/SKILL.md` 的取值一致。
///
/// `仅记录`（OD-ARCH-099）是界面上取消「进周报」时写回的值：留在本周记录里，
/// 但既不算进周报统计，也不进周报上下文。
pub const DESTINATIONS: &[&str] = &["本周报引用", "仅记录", "知识卡", "inbox", "归档"];

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
    /// 这条记录在该日期段内的第几条（0 起）。补录纠正时用它定位（OD-ARCH-103 S3）。
    pub entry_index: usize,
    /// 实际落盘的正文（可能已掩码）。改日期时原样带回做一致性校验。
    pub entry_text: String,
    pub error: Option<String>,
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

/// 校验一条补录日期：形状正确、属于给定周文件、且不晚于今天。
///
/// 不合法一律返回错误，由调用方如实拒绝——**不静默回退成今天**。
/// 静默回退正是 OD-ARCH-103 要修的那个问题的变体：用户以为记到了周一，实际落到了周三。
pub fn assert_entry_date(week: &str, date: &str, today: &str) -> Result<(), String> {
    assert_week_label(week)?;
    if !is_date_shape(date) {
        return Err(format!("补录日期格式不正确（应为 YYYY-MM-DD）：{date}"));
    }
    if !is_date_shape(today) {
        return Err(format!("今天日期格式不正确（应为 YYYY-MM-DD）：{today}"));
    }
    let day = days_from_civil(parse_year(date), parse_month(date), parse_day(date));
    let today_day = days_from_civil(parse_year(today), parse_month(today), parse_day(today));
    if day > today_day {
        return Err(format!("补录日期晚于今天（{date} > {today}），没有写盘"));
    }
    let (y, w) = iso_week_of_days(day);
    let label = format!("{:04}-W{:02}", y, w);
    if label != week {
        return Err(format!(
            "补录日期 {date} 属于 {label}，不在当周文件 {week} 里，没有写盘。跨周补录暂不支持。"
        ));
    }
    Ok(())
}

fn parse_year(date: &str) -> i64 {
    date[..4].parse().unwrap_or(0)
}

fn parse_month(date: &str) -> u32 {
    date[5..7].parse().unwrap_or(1)
}

fn parse_day(date: &str) -> u32 {
    date[8..10].parse().unwrap_or(1)
}

/// 该日期段内已有的条目数（含「（续）」段）。
fn count_entries_on_date(content: &str, date: &str) -> usize {
    let mut current: Option<String> = None;
    let mut count = 0usize;
    for line in content.lines() {
        if let Some(found) = date_heading(line) {
            current = Some(found);
            continue;
        }
        if current.as_deref() == Some(date) && item_text(line).is_some() {
            count += 1;
        }
    }
    count
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
    // 这条落在该日期段里的序号（0 起）。同一天的第二条是 1，界面和改日期都按它定位。
    let entry_index = existing
        .as_ref()
        .map(|t| count_entries_on_date(t, date))
        .unwrap_or(0);

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
            entry_index,
            entry_text: trimmed.to_string(),
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
// 改去向（OD-ARCH-099 S02）：只改一条记录的 **去向** 行
// ---------------------------------------------------------------------------

/// 一条记录的去向行位置（行号 + 缩进 + 分隔符 + 原值）。
struct DestLine {
    line: usize,
    indent: String,
    sep: char,
    previous: String,
}

/// 错误信息里回显正文时截断，避免把整段记录带进提示。
fn clip(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 24 {
        return text.to_string();
    }
    let head: String = chars[..24].iter().collect();
    format!("{head}…")
}

/// 解析 "## 2026-10-26" 与 "## 2026-10-26（续）" 两种日期段标题。
fn date_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("##")?;
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let chars: Vec<char> = rest.trim().chars().collect();
    if chars.len() < 10 {
        return None;
    }
    let head: String = chars[..10].iter().collect();
    if !is_date_shape(&head) {
        return None;
    }
    let tail: String = chars[10..].iter().collect();
    let tail = tail.trim();
    if tail.is_empty() || tail == "（续）" || tail == "(续)" {
        Some(head)
    } else {
        None
    }
}

/// "- **事**：正文" → 正文；不是条目行返回 None。
fn item_text(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('-')?;
    let rest = rest.strip_prefix(|c: char| c.is_whitespace())?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("**事**")?;
    let rest = rest.strip_prefix('：').or_else(|| rest.strip_prefix(':'))?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// "  **去向**：本周报引用" → 取值；不是去向行返回 None。
fn dest_line(line: &str) -> Option<&str> {
    if !line.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = line.trim_start();
    let rest = rest.strip_prefix("**去向**")?;
    let rest = rest.strip_prefix('：').or_else(|| rest.strip_prefix(':'))?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// YYYY-MM-DD 形状且是真实存在的日期。
fn is_date_shape(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let y: i64 = match text[..4].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let m: u32 = match text[5..7].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let d: u32 = match text[8..10].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    if !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    // 该月最后一天：下个月 1 号的前一天。
    let last = if m == 12 {
        days_from_civil(y + 1, 1, 1) - 1
    } else {
        days_from_civil(y, m + 1, 1) - 1
    };
    days_from_civil(y, m, d) <= last
}

/// 定位一条条目的去向行。index 是「该日期段内第几条 - **事**」（0 起）。
///
/// 只在 date 对应的日期段里找（含「（续）」段）；越界、正文不符、没有去向行都拒绝。
fn locate_destination(
    lines: &[String],
    date: &str,
    index: usize,
    expect_text: &str,
) -> Result<DestLine, String> {
    let mut current: Option<String> = None;
    let mut seen = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        if let Some(found) = date_heading(&lines[i]) {
            current = Some(found);
            i += 1;
            continue;
        }
        if current.as_deref() != Some(date) {
            i += 1;
            continue;
        }
        if let Some(text) = item_text(&lines[i]) {
            if seen != index {
                seen += 1;
                i += 1;
                continue;
            }
            // 收集这一条的正文（含续行），到去向行或下一条/下一个日期为止。
            let mut body = vec![text.to_string()];
            let mut j = i + 1;
            let mut found: Option<(usize, String, char, String)> = None;
            while j < lines.len() {
                if date_heading(&lines[j]).is_some() || item_text(&lines[j]).is_some() {
                    break;
                }
                if let Some(rest) = dest_line(&lines[j]) {
                    let indent: String = lines[j]
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    let sep = if lines[j].contains('：') { '：' } else { ':' };
                    found = Some((j, indent, sep, rest.trim().to_string()));
                    break;
                }
                body.push(lines[j].trim().to_string());
                j += 1;
            }
            let Some((line_no, indent, sep, previous)) = found else {
                return Err(format!(
                    "第 {} 条记录没有「去向」行，界面不替你新增结构；可在原文补一行后再试",
                    index + 1
                ));
            };
            let actual = body.join("\n").trim().to_string();
            let expected = expect_text.trim().to_string();
            if actual != expected {
                return Err(format!(
                    "第 {} 条记录的正文与界面读到的不一致（文件：{} / 请求：{}），已拒绝改写",
                    index + 1,
                    clip(&actual),
                    clip(&expected)
                ));
            }
            return Ok(DestLine {
                line: line_no,
                indent,
                sep,
                previous,
            });
        }
        i += 1;
    }
    Err(format!(
        "在 {} 的日期段里只找到 {} 条记录，没有第 {} 条",
        date,
        seen,
        index + 1
    ))
}

/// 改去向的回执。失败时 ok=false、error 说明原因，文件一字未动。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DestinationReceipt {
    pub ok: bool,
    pub path: String,
    pub week: String,
    pub date: String,
    /// 该日期段内的第几条（0 起，与界面读到的顺序一致）。
    pub index: usize,
    pub destination: String,
    pub previous: Option<String>,
    pub backup: Option<String>,
    pub error: Option<String>,
}

/// 把某一条记录的「去向」改成 destination。
///
/// 只改那一行；其余字节必须逐行相同，否则回滚并报错。写前复制
/// YYYY-Www.md.bak-<epoch>，写入走同目录临时文件 + fsync + rename。
pub fn set_destination(
    library_root: &Path,
    week: &str,
    date: &str,
    index: usize,
    destination: &str,
    expect_text: &str,
) -> Result<DestinationReceipt, String> {
    assert_week_label(week)?;
    if !is_date_shape(date) {
        return Err(format!("日期格式不正确（应为 YYYY-MM-DD）：{date}"));
    }
    let dest = destination.trim();
    if !DESTINATIONS.contains(&dest) {
        return Err(format!(
            "去向取值不在允许列表内：{dest}（允许：{}）",
            DESTINATIONS.join(" / ")
        ));
    }
    if expect_text.trim().is_empty() {
        return Err("缺少待校验的正文，已拒绝改写".to_string());
    }
    let path = week_path(library_root, week);
    let original = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("当周工作记录不存在：{}", path.display()))
        }
        Err(e) => return Err(format!("读取工作记录失败（{}）：{}", path.display(), e)),
    };
    let content =
        String::from_utf8(original.clone()).map_err(|_| "工作记录不是 UTF-8 文本，已拒绝改写".to_string())?;
    let crlf = content.contains("\r\n");
    let eol = if crlf { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = content
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    // 重放一遍必须与原文逐字节相同，否则说明换行混用，直接拒绝而不去猜。
    if lines.join(eol) != content {
        return Err("工作记录里混用了不同换行符，已拒绝改写以免破坏其他行".to_string());
    }

    let target = locate_destination(&lines, date, index, expect_text)?;
    let previous = target.previous.clone();
    let new_line = format!("{}**去向**{}{}", target.indent, target.sep, dest);
    lines[target.line] = new_line.clone();
    let updated = lines.join(eol);
    if updated == content {
        return Err(format!("这条记录的去向已经是 {dest}，无需改写"));
    }

    // 备份，并回读核对。
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut backup = path.with_file_name(format!("{week}.md.bak-{stamp}"));
    let mut n = 1u32;
    while backup.exists() {
        backup = path.with_file_name(format!("{week}.md.bak-{stamp}-{n}"));
        n += 1;
    }
    fs::write(&backup, &original)
        .map_err(|e| format!("写备份失败（{}）：{}", backup.display(), e))?;
    match fs::read(&backup) {
        Ok(bytes) if bytes == original => {}
        Ok(_) => return Err("备份内容与原文件不一致，已中止改写".to_string()),
        Err(e) => return Err(format!("回读备份失败（{}）：{}", backup.display(), e)),
    }

    // 临时文件 + fsync + rename。
    let tmp = path.with_file_name(format!(".{week}.md.tmp-{}", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&tmp)
            .map_err(|e| format!("创建临时文件失败（{}）：{}", tmp.display(), e))?;
        file.write_all(updated.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入临时文件失败（{}）：{}", tmp.display(), e))?;
        drop(file);
        fs::rename(&tmp, &path)
            .map_err(|e| format!("替换工作记录失败（{}）：{}", path.display(), e))
    })();
    if let Err(message) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(message);
    }

    // 写后自证：内容等于预期、行数不变、除目标行外逐字节相同。
    let verify = (|| -> Result<(), String> {
        let read_back = fs::read(&path)
            .map_err(|e| format!("回读工作记录失败（{}）：{}", path.display(), e))?;
        let text = String::from_utf8(read_back)
            .map_err(|_| "回读内容不是 UTF-8，请人工核对".to_string())?;
        if text != updated {
            return Err("回读内容与预期改写结果不一致".to_string());
        }
        let mut got: Vec<&str> = text.split('\n').collect();
        let want: Vec<&str> = content.split('\n').collect();
        if got.len() != want.len() {
            return Err("回读行数与原文不一致".to_string());
        }
        if got[target.line].strip_suffix('\r').unwrap_or(got[target.line]) != new_line {
            return Err("目标行不是预期的新去向".to_string());
        }
        got[target.line] = want[target.line];
        if got != want {
            return Err("除目标行外还有其他行发生变化".to_string());
        }
        Ok(())
    })();

    if let Err(message) = verify {
        // 回滚：用备份内容原子写回。
        let rollback = (|| -> Result<(), String> {
            let back = fs::read(&backup)
                .map_err(|e| format!("读取备份失败（{}）：{}", backup.display(), e))?;
            let tmp = path.with_file_name(format!(".{week}.md.restore-{}", std::process::id()));
            let mut file = fs::File::create(&tmp)
                .map_err(|e| format!("创建回滚临时文件失败（{}）：{}", tmp.display(), e))?;
            file.write_all(&back)
                .and_then(|_| file.sync_all())
                .map_err(|e| format!("回滚写入失败（{}）：{}", tmp.display(), e))?;
            drop(file);
            fs::rename(&tmp, &path)
                .map_err(|e| format!("回滚替换失败（{}）：{}", path.display(), e))
        })();
        return Err(match rollback {
            Ok(()) => format!("改写自证失败，已用备份回滚：{message}"),
            Err(restore) => format!(
                "改写自证失败（{message}），回滚也失败（{restore}），备份在 {}",
                backup.display()
            ),
        });
    }

    Ok(DestinationReceipt {
        ok: true,
        path: path.display().to_string(),
        week: week.to_string(),
        date: date.to_string(),
        index,
        destination: dest.to_string(),
        previous: Some(previous),
        backup: Some(backup.display().to_string()),
        error: None,
    })
}

// ---------------------------------------------------------------------------
// 改日期（OD-ARCH-103 S3）：把一条记录从错误日期段挪到正确日期段
// ---------------------------------------------------------------------------

/// 改日期的回执。失败时 ok=false、error 说明原因，文件一字未动。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RedateReceipt {
    pub ok: bool,
    pub path: String,
    pub week: String,
    /// 原日期段。
    pub from_date: String,
    /// 目标日期段。
    pub to_date: String,
    /// 原段内的序号（0 起）。
    pub index: usize,
    /// 挪到目标段后是第几条（0 起）。
    pub new_index: usize,
    pub backup: Option<String>,
    pub error: Option<String>,
}

/// 定位一条记录的完整行块：`- **事**` 行到它的 `**去向**` 行（含）。
///
/// 返回 (起始行, 结束行, 正文)。正文与 `set_destination` 的解析口径一致：
/// 条目行文本 + 中间的续行，逐行去尾空格后用 `\n` 连接。
fn locate_entry_block(
    lines: &[String],
    date: &str,
    index: usize,
    expect_text: &str,
) -> Result<(usize, usize, String), String> {
    let mut current: Option<String> = None;
    let mut seen = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if let Some(found) = date_heading(line) {
            current = Some(found);
            continue;
        }
        if current.as_deref() != Some(date) {
            continue;
        }
        let Some(text) = item_text(line) else { continue };
        if seen != index {
            seen += 1;
            continue;
        }
        let mut body = vec![text.to_string()];
        let mut j = i + 1;
        while j < lines.len() {
            if date_heading(&lines[j]).is_some() || item_text(&lines[j]).is_some() {
                break;
            }
            if dest_line(&lines[j]).is_some() {
                let actual = body.join("\n").trim().to_string();
                let expected = expect_text.trim().to_string();
                if actual != expected {
                    return Err(format!(
                        "第 {} 条记录的正文与界面读到的不一致（文件：{} / 请求：{}），已拒绝改日期",
                        index + 1,
                        clip(&actual),
                        clip(&expected)
                    ));
                }
                return Ok((i, j, actual));
            }
            body.push(lines[j].trim().to_string());
            j += 1;
        }
        return Err(format!(
            "第 {} 条记录没有「去向」行，结构不完整；请在原文补一行后再试",
            index + 1
        ));
    }
    Err(format!(
        "在 {} 的日期段里只找到 {} 条记录，没有第 {} 条",
        date,
        seen,
        index + 1
    ))
}

/// 一条待移动的记录（OD-ARCH-104 批量移动）。
///
/// 用 date + index + expect_text 三元组定位：序号来自界面读到的当周索引，
/// expect_text 由索引里的正文填入，宿主据此确认「要搬的确实是我读到的那条」。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRequest {
    /// 原日期段。
    pub date: String,
    /// 原日期段内的序号（0 起，与界面索引一致）。
    pub index: usize,
    /// 目标日期段。
    pub to: String,
    /// 界面读到的正文；与文件内容不一致即整批拒绝。
    pub expect_text: String,
}

/// 批量移动里的单条结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MovedEntry {
    pub from_date: String,
    pub to_date: String,
    pub index: usize,
    pub new_index: usize,
    /// 移动后的正文（原样，不改写）。
    pub text: String,
}

/// 批量移动的回执。任一条不合规即整批拒绝，文件一字未动。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoveBatchReceipt {
    pub ok: bool,
    pub path: String,
    pub week: String,
    pub moves: Vec<MovedEntry>,
    pub backup: Option<String>,
    pub error: Option<String>,
}

/// 读出当周文件并拆行；换行口径必须一致，否则拒绝改写。
fn read_week_lines(
    library_root: &Path,
    week: &str,
) -> Result<(PathBuf, Vec<u8>, String, String, Vec<String>), String> {
    let path = week_path(library_root, week);
    let original = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("当周工作记录不存在：{}", path.display()))
        }
        Err(e) => return Err(format!("读取工作记录失败（{}）：{}", path.display(), e)),
    };
    let content = String::from_utf8(original.clone())
        .map_err(|_| "工作记录不是 UTF-8 文本，已拒绝改写".to_string())?;
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<String> = content
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    if lines.join(eol) != content {
        return Err("工作记录里混用了不同换行符，已拒绝改写以免破坏其他行".to_string());
    }
    Ok((path, original, content, eol.to_string(), lines))
}

/// 写前自证：条目数守恒、只增删了允许的日期标题、每条记录原样还在。
fn assert_structure_preserved(
    content: &str,
    updated: &str,
    allowed_removed: &[String],
    allowed_added: &[String],
    blocks: &[Vec<String>],
) -> Result<(), String> {
    let before_items = content.matches("- **事**：").count();
    let after_items = updated.matches("- **事**：").count();
    if before_items != after_items {
        return Err(format!(
            "改动会让条目数从 {before_items} 变成 {after_items}，已中止"
        ));
    }
    let compact = |text: &str| -> Vec<String> {
        text.split('\n')
            .map(|l| l.trim_end().to_string())
            .filter(|l| !l.trim().is_empty())
            .collect()
    };
    let before_lines = compact(content);
    let after_lines = compact(updated);
    let removed_set: Vec<String> = before_lines
        .iter()
        .filter(|l| !after_lines.contains(l))
        .cloned()
        .collect();
    let added_set: Vec<String> = after_lines
        .iter()
        .filter(|l| !before_lines.contains(l))
        .cloned()
        .collect();
    if !removed_set.iter().all(|l| l.starts_with("## ")) {
        return Err("改动会删掉条目正文或其它行，已中止".to_string());
    }
    if !added_set.iter().all(|l| l.starts_with("## ")) {
        return Err("改动会新增条目正文或其它行，已中止".to_string());
    }
    if !removed_set.iter().all(|l| allowed_removed.contains(l)) {
        return Err("改动会删掉别的日期标题，已中止".to_string());
    }
    if !added_set.iter().all(|l| allowed_added.contains(l)) {
        return Err("改动会新增别的日期标题，已中止".to_string());
    }
    for block in blocks {
        for line in block {
            if !updated.lines().any(|l| l.trim_end() == line.trim_end()) {
                return Err("被移动的正文没有原样出现在文件里，已中止".to_string());
            }
        }
    }
    Ok(())
}

/// 备份 + 临时文件 + fsync + rename + 回读自证。返回备份路径。
fn commit_week_file(
    path: &Path,
    week: &str,
    original: &[u8],
    updated: &str,
) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut backup = path.with_file_name(format!("{week}.md.bak-{stamp}"));
    let mut n = 1u32;
    while backup.exists() {
        backup = path.with_file_name(format!("{week}.md.bak-{stamp}-{n}"));
        n += 1;
    }
    fs::write(&backup, original).map_err(|e| format!("写备份失败（{}）：{}", backup.display(), e))?;
    match fs::read(&backup) {
        Ok(bytes) if bytes == original => {}
        Ok(_) => return Err("备份内容与原文件不一致，已中止改写".to_string()),
        Err(e) => return Err(format!("回读备份失败（{}）：{}", backup.display(), e)),
    }
    let tmp = path.with_file_name(format!(".{week}.md.tmp-{}", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&tmp)
            .map_err(|e| format!("创建临时文件失败（{}）：{}", tmp.display(), e))?;
        file.write_all(updated.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入临时文件失败（{}）：{}", tmp.display(), e))?;
        drop(file);
        fs::rename(&tmp, path).map_err(|e| format!("替换工作记录失败（{}）：{}", path.display(), e))
    })();
    if let Err(message) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(message);
    }
    let read_back = fs::read_to_string(path)
        .map_err(|e| format!("回读工作记录失败（{}）：{}", path.display(), e))?;
    if read_back != updated {
        let _ = fs::copy(&backup, path);
        return Err(format!(
            "回读内容与预期不一致，已用备份 {} 回滚",
            backup.display()
        ));
    }
    Ok(backup)
}

/// 删掉某个日期下「只有标题、没有任何记录」的段（含「（续）」段）。
///
/// 搬走一条记录后，它原来所在的段常常只剩一个空标题。留着会堆出一串
/// 「## 日期」与「## 日期（续）」，下次读取时既难看也难定位。这里只清理这一日期自己的段。
fn prune_empty_sections(lines: &mut Vec<String>, date: &str) {
    loop {
        let mut target: Option<(usize, usize)> = None;
        let mut i = 0usize;
        while i < lines.len() {
            let Some(found) = date_heading(&lines[i]) else {
                i += 1;
                continue;
            };
            // 段尾：下一个标题行（任意日期）。
            let mut end = i + 1;
            while end < lines.len() && date_heading(&lines[end]).is_none() {
                end += 1;
            }
            if found == date && !lines[i + 1..end].iter().any(|l| item_text(l).is_some()) {
                target = Some((i, end));
                break;
            }
            i = end;
        }
        let Some((start, end)) = target else { break };
        // 连同段尾空行一起删，避免留下连续空行。
        let mut stop = end;
        while stop < lines.len() && lines[stop].trim().is_empty() {
            stop += 1;
        }
        lines.drain(start..stop);
    }
}

/// 把一条记录块接到 to_date 段末尾：已有条目就续在其后，只有空标题就补进去，
/// 没有这个日期就起新段。
fn insert_block(lines: &mut Vec<String>, to_date: &str, block: &[String]) {
    let mut cur: Option<String> = None;
    let mut target_last: Option<usize> = None;
    let mut target_heading: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(found) = date_heading(line) {
            if found == to_date && target_heading.is_none() {
                target_heading = Some(i);
            }
            cur = Some(found);
            continue;
        }
        if cur.as_deref() == Some(to_date) && !line.trim().is_empty() {
            target_last = Some(i);
        }
    }
    match target_last {
        Some(last) => {
            let at = last + 1;
            let mut payload = vec![String::new()];
            payload.extend(block.iter().cloned());
            lines.splice(at..at, payload);
        }
        None if target_heading.is_some() => {
            let at = target_heading.unwrap() + 1;
            while at < lines.len() && lines[at].trim().is_empty() {
                lines.remove(at);
            }
            let mut payload = vec![String::new()];
            payload.extend(block.iter().cloned());
            lines.splice(at..at, payload);
        }
        None => {
            while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                lines.pop();
            }
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("## {to_date}"));
            lines.push(String::new());
            lines.extend(block.iter().cloned());
            lines.push(String::new());
        }
    }
}

/// 一次读入、全量校验、一次性重写：批量把记录从错误日期段挪到正确日期段。
///
/// 语义由模型判定，这里只负责「能不能安全搬」。任一条定位不上、正文不符、
/// 日期跨周或晚于今天、两条区域重叠，都整批拒绝，文件一字不动。
pub fn move_entries(
    library_root: &Path,
    week: &str,
    moves: &[MoveRequest],
    today: &str,
) -> Result<MoveBatchReceipt, String> {
    assert_week_label(week)?;
    if moves.is_empty() {
        return Err("没有要移动的记录".to_string());
    }
    let (path, original, content, eol, mut lines) = read_week_lines(library_root, week)?;

    // 1) 先把每一条都定位出来，在任何写盘动作之前完成全部校验。
    struct Located {
        request: MoveRequest,
        start: usize,
        end: usize,
        block: Vec<String>,
        text: String,
    }
    let mut located: Vec<Located> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for request in moves {
        if !is_date_shape(&request.date) || !is_date_shape(&request.to) {
            return Err("日期格式不正确（应为 YYYY-MM-DD），已拒绝移动".to_string());
        }
        if request.date == request.to {
            return Err(format!(
                "原日期与目标日期都是 {}，无需移动",
                request.date
            ));
        }
        if request.expect_text.trim().is_empty() {
            return Err("缺少待校验的正文，已拒绝移动".to_string());
        }
        let from_day = days_from_civil(
            parse_year(&request.date),
            parse_month(&request.date),
            parse_day(&request.date),
        );
        let (fy, fw) = iso_week_of_days(from_day);
        let from_week = format!("{fy:04}-W{fw:02}");
        if from_week != week {
            return Err(format!(
                "原日期 {} 属于 {from_week}，不在当周文件 {week} 里，未改动文件。",
                request.date
            ));
        }
        let to_day = days_from_civil(
            parse_year(&request.to),
            parse_month(&request.to),
            parse_day(&request.to),
        );
        let (ty, tw) = iso_week_of_days(to_day);
        let to_week = format!("{ty:04}-W{tw:02}");
        if to_week != week {
            return Err(format!(
                "目标日期 {} 属于 {to_week}，与当周文件 {week} 不同；跨周移动暂不支持，未改动文件。",
                request.to
            ));
        }
        assert_entry_date(week, &request.to, today)?;
        if !seen.insert((request.date.clone(), request.index)) {
            return Err(format!(
                "{} 的第 {} 条被要求移动两次，已拒绝",
                request.date,
                request.index + 1
            ));
        }
        let (start, end, text) =
            locate_entry_block(&lines, &request.date, request.index, &request.expect_text)?;
        located.push(Located {
            request: request.clone(),
            start,
            end,
            block: lines[start..=end].to_vec(),
            text,
        });
    }

    // 2) 区域不能重叠：重叠说明同一块被两条指令同时搬，语义上就不成立。
    let mut ranges: Vec<(usize, usize)> = located.iter().map(|l| (l.start, l.end)).collect();
    ranges.sort();
    for pair in ranges.windows(2) {
        if pair[1].0 <= pair[0].1 {
            return Err("两条移动指令指向了同一段记录，已拒绝".to_string());
        }
    }

    // 3) 从后往前摘掉，避免前面的下标失效。紧跟其后的空行一并去掉。
    let mut order: Vec<usize> = (0..located.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse(located[*i].start));
    for i in order {
        let start = located[i].start;
        let mut end = located[i].end + 1;
        while end < lines.len() && lines[end].trim().is_empty() {
            end += 1;
        }
        lines.drain(start..end);
    }

    // 4) 原段被搬空的日期，连同它的所有标题（含「（续）」）一起删掉，不留空壳。
    let mut touched_sources: Vec<String> = Vec::new();
    for item in &located {
        if !touched_sources.contains(&item.request.date) {
            touched_sources.push(item.request.date.clone());
        }
    }
    //    只处理本次涉及的日期：别人的段即使空着也保持原样，不越权清理。
    for date in &touched_sources {
        prune_empty_sections(&mut lines, date);
    }

    // 5) 按目标日期、原顺序依次接回；同一目标的多条保持给出顺序。
    let mut planned: Vec<(String, usize)> = located
        .iter()
        .enumerate()
        .map(|(i, l)| (l.request.to.clone(), i))
        .collect();
    planned.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut results: Vec<MovedEntry> = Vec::new();
    for (to_date, idx) in planned {
        let before = count_entries_on_date(&lines.join(&eol), &to_date);
        insert_block(&mut lines, &to_date, &located[idx].block);
        results.push(MovedEntry {
            from_date: located[idx].request.date.clone(),
            to_date: to_date.clone(),
            index: located[idx].request.index,
            new_index: before,
            text: located[idx].text.clone(),
        });
    }

    let updated = lines.join(&eol);
    if updated == content {
        return Err("这些记录已经都在目标日期段里，无需移动".to_string());
    }

    // 6) 自证：只允许增删与本次涉及的日期标题。
    let mut allowed_removed: Vec<String> = Vec::new();
    let mut allowed_added: Vec<String> = Vec::new();
    for date in &touched_sources {
        allowed_removed.push(format!("## {date}"));
        allowed_removed.push(format!("## {date}（续）"));
    }
    for item in &located {
        let to = &item.request.to;
        for candidate in [format!("## {to}"), format!("## {to}（续）")] {
            if !allowed_added.contains(&candidate) {
                allowed_added.push(candidate.clone());
            }
            if !allowed_removed.contains(&candidate) {
                allowed_removed.push(candidate);
            }
        }
    }
    let blocks: Vec<Vec<String>> = located.iter().map(|l| l.block.clone()).collect();
    assert_structure_preserved(&content, &updated, &allowed_removed, &allowed_added, &blocks)?;

    let backup = commit_week_file(&path, week, &original, &updated)?;
    Ok(MoveBatchReceipt {
        ok: true,
        path: path.display().to_string(),
        week: week.to_string(),
        moves: results,
        backup: Some(backup.display().to_string()),
        error: None,
    })
}

/// 单条改日期：批量版本的薄封装（保留既有回执形状与调用点）。
pub fn move_entry(
    library_root: &Path,
    week: &str,
    from_date: &str,
    index: usize,
    to_date: &str,
    expect_text: &str,
    today: &str,
) -> Result<RedateReceipt, String> {
    let batch = move_entries(
        library_root,
        week,
        &[MoveRequest {
            date: from_date.to_string(),
            index,
            to: to_date.to_string(),
            expect_text: expect_text.to_string(),
        }],
        today,
    )?;
    let moved = batch
        .moves
        .into_iter()
        .next()
        .ok_or_else(|| "没有可移动的记录".to_string())?;
    Ok(RedateReceipt {
        ok: true,
        path: batch.path,
        week: batch.week,
        from_date: moved.from_date,
        to_date: moved.to_date,
        index: moved.index,
        new_index: moved.new_index,
        backup: batch.backup,
        error: None,
    })
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

    // --- OD-ARCH-099 S02：改去向 ---

    /// 造一份两周各一条、同日两条的样本，用来验证「只改目标那条」。
    fn seed(root: &Path) -> PathBuf {
        append_entry(root, "2026-W44", "2026-10-26", "第一件事", "本周报引用").unwrap();
        append_entry(root, "2026-W44", "2026-10-26", "第二件事", "本周报引用").unwrap();
        append_entry(root, "2026-W44", "2026-10-27", "第三件事", "知识卡").unwrap();
        root.join(WORKLOG_DIR).join("2026-W44.md")
    }

    #[test]
    fn set_destination_changes_only_the_target_line() {
        let root = temp_root("dest-ok");
        let path = seed(&root);
        let before = fs::read_to_string(&path).unwrap();
        let receipt =
            set_destination(&root, "2026-W44", "2026-10-26", 1, "仅记录", "第二件事").unwrap();
        assert!(receipt.ok);
        assert_eq!(receipt.index, 1);
        assert_eq!(receipt.destination, "仅记录");
        assert_eq!(receipt.previous.as_deref(), Some("本周报引用"));
        let after = fs::read_to_string(&path).unwrap();

        let old_lines: Vec<&str> = before.split('\n').collect();
        let new_lines: Vec<&str> = after.split('\n').collect();
        assert_eq!(old_lines.len(), new_lines.len(), "行数变了");
        let diffs: Vec<usize> = (0..old_lines.len()).filter(|i| old_lines[*i] != new_lines[*i]).collect();
        assert_eq!(diffs.len(), 1, "不只有一行变了：{diffs:?}");
        assert_eq!(new_lines[diffs[0]], "  **去向**：仅记录");
        // 同日第一条与次日那条都没被带着改。
        assert!(after.contains("- **事**：第一件事\n  **去向**：本周报引用"));
        assert!(after.contains("- **事**：第三件事\n  **去向**：知识卡"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_backup_matches_original() {
        let root = temp_root("dest-bak");
        let path = seed(&root);
        let before = fs::read(&path).unwrap();
        let receipt = set_destination(&root, "2026-W44", "2026-10-27", 0, "仅记录", "第三件事").unwrap();
        let backup = receipt.backup.expect("应返回备份路径");
        assert_eq!(fs::read(&backup).unwrap(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_rejects_text_mismatch_without_writing() {
        let root = temp_root("dest-mismatch");
        let path = seed(&root);
        let before = fs::read(&path).unwrap();
        let err = set_destination(&root, "2026-W44", "2026-10-26", 0, "仅记录", "不是这条").unwrap_err();
        assert!(err.contains("正文"), "错误应指明正文不符：{err}");
        assert_eq!(fs::read(&path).unwrap(), before, "拒绝时文件不能变");
        // 失败路径不留备份文件。
        let leftovers: Vec<_> = fs::read_dir(root.join(WORKLOG_DIR))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert!(leftovers.is_empty(), "拒绝时不该留下备份");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_rejects_unknown_date_and_index() {
        let root = temp_root("dest-range");
        let path = seed(&root);
        let before = fs::read(&path).unwrap();
        assert!(set_destination(&root, "2026-W44", "2026-10-30", 0, "仅记录", "第一件事").is_err());
        let err = set_destination(&root, "2026-W44", "2026-10-26", 9, "仅记录", "第一件事").unwrap_err();
        assert!(err.contains("没有第"), "越界应说明找不到：{err}");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_rejects_entry_without_destination_line() {
        let root = temp_root("dest-nodest");
        let dir = root.join(WORKLOG_DIR);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-W44.md");
        fs::write(&path, "# 2026-W44\n\n## 2026-10-26\n\n- **事**：手写没有去向\n").unwrap();
        let before = fs::read(&path).unwrap();
        let err = set_destination(&root, "2026-W44", "2026-10-26", 0, "仅记录", "手写没有去向").unwrap_err();
        assert!(err.contains("去向"), "应说明缺去向行：{err}");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_rejects_unknown_value_and_missing_file() {
        let root = temp_root("dest-value");
        let path = seed(&root);
        let before = fs::read(&path).unwrap();
        assert!(set_destination(&root, "2026-W44", "2026-10-26", 0, "随便写", "第一件事").is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let err = set_destination(&root, "2026-W43", "2026-10-19", 0, "仅记录", "x").unwrap_err();
        assert!(err.contains("不存在"), "缺文件应说明：{err}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_keeps_crlf_and_continuation_sections() {
        let root = temp_root("dest-crlf");
        let dir = root.join(WORKLOG_DIR);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-W44.md");
        let original = "# 2026-W44\r\n\r\n## 2026-10-26\r\n\r\n- **事**：上午\r\n  **去向**：本周报引用\r\n\r\n## 2026-10-26（续）\r\n\r\n- **事**：下午\r\n  **去向**：本周报引用\r\n";
        fs::write(&path, original).unwrap();
        // 「（续）」段归回同一天，所以 2026-10-26 的第 2 条是下午那条。
        set_destination(&root, "2026-W44", "2026-10-26", 1, "仅记录", "下午").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("  **去向**：仅记录\r\n"), "应保留 CRLF");
        assert!(!after.contains("\n  **去向**：仅记录\n"), "不该引入 LF");
        assert_eq!(after.matches("\r\n").count(), original.matches("\r\n").count());
        assert!(after.contains("- **事**：上午\r\n  **去向**：本周报引用"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_is_idempotent_guarded() {
        let root = temp_root("dest-same");
        let path = seed(&root);
        let receipt = set_destination(&root, "2026-W44", "2026-10-26", 0, "仅记录", "第一件事").unwrap();
        let backup = receipt.backup.unwrap();
        let after_first = fs::read(&path).unwrap();
        let err = set_destination(&root, "2026-W44", "2026-10-26", 0, "仅记录", "第一件事").unwrap_err();
        assert!(err.contains("已经是"), "重复写应拒绝：{err}");
        assert_eq!(fs::read(&path).unwrap(), after_first);
        assert!(Path::new(&backup).exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn set_destination_matches_multi_line_entry_text() {
        let root = temp_root("dest-multiline");
        let dir = root.join(WORKLOG_DIR);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-W44.md");
        fs::write(
            &path,
            "# 2026-W44\n\n## 2026-10-26\n\n- **事**：第一行\n  第二行\n  **去向**：本周报引用\n",
        )
        .unwrap();
        // 正文两行：界面会带着换行一起发回，必须逐字吻合才允许改。
        set_destination(&root, "2026-W44", "2026-10-26", 0, "仅记录", "第一行\n第二行").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("  **去向**：仅记录"));
        assert!(after.contains("  第二行\n"));
        fs::remove_dir_all(&root).ok();
    }

    // -----------------------------------------------------------------------
    // OD-ARCH-103：补录日期校验 + 改日期
    // -----------------------------------------------------------------------

    #[test]
    fn assert_entry_date_accepts_same_week_and_past_days() {
        // 2026-10-28 是周三，属 2026-W44。
        assert!(assert_entry_date("2026-W44", "2026-10-26", "2026-10-28").is_ok());
        assert!(assert_entry_date("2026-W44", "2026-10-28", "2026-10-28").is_ok());
    }

    #[test]
    fn assert_entry_date_rejects_future_other_week_and_bad_shape() {
        let week = "2026-W44";
        let today = "2026-10-28";
        let future = assert_entry_date(week, "2026-10-30", today).unwrap_err();
        assert!(future.contains("晚于今天"), "应说明是未来日期：{future}");
        let other = assert_entry_date(week, "2026-10-19", today).unwrap_err();
        assert!(other.contains("2026-W43"), "应说明属于哪一周：{other}");
        let bad = assert_entry_date(week, "2026-10-32", today).unwrap_err();
        assert!(bad.contains("格式"), "应说明格式问题：{bad}");
        let slash = assert_entry_date(week, "2026/10/27", today).unwrap_err();
        assert!(slash.contains("格式"), "斜杠日期也应拒绝：{slash}");
        let bad_week = assert_entry_date("2026-W99", "2026-10-27", today).unwrap_err();
        assert!(bad_week.contains("周次"), "周标签也要校验：{bad_week}");
    }

    #[test]
    fn move_entry_pastes_block_into_existing_target_section() {
        let root = temp_root("redate-join");
        append_entry(&root, "2026-W44", "2026-10-26", "周一的课", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "周三的事", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");

        let receipt = move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-26", "周三的事", "2026-10-28").unwrap();
        assert!(receipt.ok && receipt.new_index == 1);
        let after = fs::read_to_string(&path).unwrap();
        let monday = after.find("## 2026-10-26").unwrap();
        assert!(after.find("## 2026-10-28").is_none(), "原段空掉后标题应删除：{after}");
        assert!(after.find("周三的事").unwrap() > monday, "记录应落在周一标题之后：{after}");
        assert_eq!(after.matches("- **事**：").count(), 2, "条目数守恒");
        assert!(after.contains("- **事**：周三的事\n  **去向**：本周报引用\n"), "正文与去向原样保留");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_creates_target_section_when_absent() {
        let root = temp_root("redate-new");
        append_entry(&root, "2026-W44", "2026-10-28", "记错日子的记录", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-27", "记错日子的记录", "2026-10-28").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("## 2026-10-27"), "应新建目标日期段：{after}");
        assert!(!after.contains("## 2026-10-28"), "原段应删除：{after}");
        assert_eq!(after.matches("- **事**：").count(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_keeps_other_entries_in_source_section() {
        let root = temp_root("redate-keep");
        append_entry(&root, "2026-W44", "2026-10-28", "第一条", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "第二条", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        // 把第 2 条（「（续）」段里的）挪到周一。
        move_entry(&root, "2026-W44", "2026-10-28", 1, "2026-10-26", "第二条", "2026-10-28").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("## 2026-10-28"), "原段还有记录，标题要保留：{after}");
        assert!(after.contains("第一条"));
        let monday = after.find("## 2026-10-26").unwrap();
        assert!(after.find("第二条").unwrap() > monday);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_rejects_text_mismatch_and_leaves_file_untouched() {
        let root = temp_root("redate-mismatch");
        append_entry(&root, "2026-W44", "2026-10-28", "真实内容", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        let before = fs::read(&path).unwrap();
        let err = move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-26", "别的内容", "2026-10-28").unwrap_err();
        assert!(err.contains("不一致"), "应说明正文对不上：{err}");
        assert_eq!(fs::read(&path).unwrap(), before, "拒绝时文件不能变");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_rejects_cross_week_target_and_same_date() {
        let root = temp_root("redate-cross");
        append_entry(&root, "2026-W44", "2026-10-28", "记录", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        let before = fs::read(&path).unwrap();
        let cross = move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-11-02", "记录", "2026-10-28").unwrap_err();
        assert!(cross.contains("跨周"), "应拒绝跨周：{cross}");
        let same = move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-28", "记录", "2026-10-28").unwrap_err();
        assert!(same.contains("无需"), "同一天应说明无需改：{same}");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_preserves_crlf() {
        let root = temp_root("redate-crlf");
        let dir = root.join(WORKLOG_DIR);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-W44.md");
        let original = "# 2026-W44\r\n\r\n## 2026-10-28\r\n\r\n- **事**：周三的记录\r\n  **去向**：本周报引用\r\n";
        fs::write(&path, original).unwrap();
        move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-26", "周三的记录", "2026-10-28").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("## 2026-10-26\r\n"), "应保留 CRLF：{after:?}");
        assert!(!after.contains("\n\n- **事**"), "不该引入裸 LF 空行：{after:?}");
        assert_eq!(after.matches("\r\n").count(), after.matches('\n').count());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_rejects_mixed_line_endings() {
        let root = temp_root("redate-mixed");
        let dir = root.join(WORKLOG_DIR);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-W44.md");
        fs::write(&path, "# 2026-W44\r\n\r\n## 2026-10-28\n\n- **事**：混用\n  **去向**：本周报引用\n").unwrap();
        let before = fs::read(&path).unwrap();
        let err = move_entry(&root, "2026-W44", "2026-10-28", 0, "2026-10-26", "混用", "2026-10-28").unwrap_err();
        assert!(err.contains("换行符"), "应拒绝混用换行：{err}");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(&root).ok();
    }

    // -----------------------------------------------------------------------
    // OD-ARCH-104：批量移动（模型判定 → 宿主一次重写）
    // -----------------------------------------------------------------------

    fn req(date: &str, index: usize, to: &str, text: &str) -> MoveRequest {
        MoveRequest {
            date: date.to_string(),
            index,
            to: to.to_string(),
            expect_text: text.to_string(),
        }
    }

    #[test]
    fn move_entries_moves_several_entries_in_one_rewrite() {
        let root = temp_root("batch-ok");
        append_entry(&root, "2026-W44", "2026-10-28", "上杭教学", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "参观紫金矿业", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "今天的新事", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        let backups_before = fs::read_dir(path.parent().unwrap()).unwrap().count();

        let receipt = move_entries(
            &root,
            "2026-W44",
            &[
                req("2026-10-28", 0, "2026-10-26", "上杭教学"),
                req("2026-10-28", 1, "2026-10-26", "参观紫金矿业"),
            ],
            "2026-10-28",
        )
        .unwrap();
        assert!(receipt.ok);
        assert_eq!(receipt.moves.len(), 2);
        assert_eq!(receipt.moves[0].new_index, 0);
        assert_eq!(receipt.moves[1].new_index, 1);
        let after = fs::read_to_string(&path).unwrap();
        // 按段判断：周三那一段里不能还有被搬走的两条，周一那一段里必须有。
        let monday_at = after.find("## 2026-10-26").unwrap();
        let wed_at = after.find("## 2026-10-28").unwrap();
        let (wed_part, mon_part) = if wed_at < monday_at {
            (&after[wed_at..monday_at], &after[monday_at..])
        } else {
            (&after[wed_at..], &after[monday_at..wed_at])
        };
        assert!(mon_part.contains("上杭教学"), "周一应含上杭教学：{after}");
        assert!(mon_part.contains("参观紫金矿业"), "周一应含紫金矿业：{after}");
        assert!(!wed_part.contains("上杭教学"), "搬走的记录不应留在周三：{after}");
        assert!(!wed_part.contains("参观紫金矿业"), "搬走的记录不应留在周三：{after}");
        assert!(wed_part.contains("今天的新事"), "当天其余记录应留在原处：{after}");
        assert_eq!(after.matches("- **事**：").count(), 3, "条目数守恒");
        // 一次重写只留一份备份。
        let backups_after = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .map(|x| x.file_name().to_string_lossy().contains(".md.bak-"))
                    .unwrap_or(false)
            })
            .count();
        assert!(backups_after >= 1, "应留下备份");
        assert!(backups_after <= 1, "一次批量只留一份备份，实际 {backups_after}");
        let _ = backups_before;
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entries_rejects_whole_batch_when_one_text_mismatches() {
        let root = temp_root("batch-mismatch");
        append_entry(&root, "2026-W44", "2026-10-28", "第一条", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "第二条", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        let before = fs::read_to_string(&path).unwrap();
        let err = move_entries(
            &root,
            "2026-W44",
            &[
                req("2026-10-28", 0, "2026-10-26", "第一条"),
                req("2026-10-28", 1, "2026-10-26", "对不上的内容"),
            ],
            "2026-10-28",
        )
        .unwrap_err();
        assert!(err.contains("不一致"), "应说明正文不一致：{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), before, "整批拒绝时文件必须一字不动");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entries_rejects_cross_week_and_future_targets() {
        let root = temp_root("batch-dates");
        append_entry(&root, "2026-W44", "2026-10-28", "记录", "本周报引用").unwrap();
        let cross = move_entries(
            &root,
            "2026-W44",
            &[req("2026-10-28", 0, "2026-11-02", "记录")],
            "2026-10-28",
        )
        .unwrap_err();
        assert!(cross.contains("跨周"), "跨周应拒绝：{cross}");
        let future = move_entries(
            &root,
            "2026-W44",
            &[req("2026-10-28", 0, "2026-10-30", "记录")],
            "2026-10-28",
        )
        .unwrap_err();
        assert!(future.contains("晚于今天"), "未来应拒绝：{future}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entries_rejects_out_of_range_index_and_duplicate_targets() {
        let root = temp_root("batch-index");
        append_entry(&root, "2026-W44", "2026-10-28", "只有一条", "本周报引用").unwrap();
        let out = move_entries(
            &root,
            "2026-W44",
            &[req("2026-10-28", 5, "2026-10-26", "只有一条")],
            "2026-10-28",
        )
        .unwrap_err();
        assert!(out.contains("没有第 6 条"), "越界应说明：{out}");
        let dup = move_entries(
            &root,
            "2026-W44",
            &[
                req("2026-10-28", 0, "2026-10-26", "只有一条"),
                req("2026-10-28", 0, "2026-10-27", "只有一条"),
            ],
            "2026-10-28",
        )
        .unwrap_err();
        assert!(dup.contains("移动两次"), "重复应拒绝：{dup}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entries_drops_emptied_source_section() {
        let root = temp_root("batch-empty");
        append_entry(&root, "2026-W44", "2026-10-26", "周一的事", "本周报引用").unwrap();
        append_entry(&root, "2026-W44", "2026-10-28", "周三唯一的记录", "本周报引用").unwrap();
        let path = week_path(&root, "2026-W44");
        move_entries(
            &root,
            "2026-W44",
            &[req("2026-10-28", 0, "2026-10-26", "周三唯一的记录")],
            "2026-10-28",
        )
        .unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(!after.contains("## 2026-10-28"), "空段应删掉标题：{after}");
        assert_eq!(after.matches("- **事**：").count(), 2);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entries_preserves_crlf() {
        let root = temp_root("batch-crlf");
        let path = week_path(&root, "2026-W44");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let body = "# 2026-W44\r\n\r\n## 2026-10-28\r\n\r\n- **事**：周三的记录\r\n  **去向**：本周报引用\r\n\r\n";
        fs::write(&path, body).unwrap();
        move_entries(
            &root,
            "2026-W44",
            &[req("2026-10-28", 0, "2026-10-26", "周三的记录")],
            "2026-10-28",
        )
        .unwrap();
        let after = fs::read(&path).unwrap();
        assert!(String::from_utf8_lossy(&after).contains("\r\n"));
        assert!(!String::from_utf8_lossy(&after).contains("\n\n## 2026-10-26"), "不应出现裸 LF");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn move_entry_reports_out_of_range_index() {
        let root = temp_root("redate-range");
        append_entry(&root, "2026-W44", "2026-10-28", "只有一条", "本周报引用").unwrap();
        let err = move_entry(&root, "2026-W44", "2026-10-28", 3, "2026-10-26", "只有一条", "2026-10-28").unwrap_err();
        assert!(err.contains("没有第"), "应报越界：{err}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn append_reports_entry_index_within_date() {
        let root = temp_root("index");
        let (a, _) = append_entry(&root, "2026-W44", "2026-10-26", "周一第一条", "本周报引用").unwrap();
        let (b, _) = append_entry(&root, "2026-W44", "2026-10-26", "周一第二条", "本周报引用").unwrap();
        let (c, _) = append_entry(&root, "2026-W44", "2026-10-27", "周二第一条", "本周报引用").unwrap();
        assert_eq!((a.entry_index, b.entry_index, c.entry_index), (0, 1, 0));
        assert_eq!(b.entry_text, "周一第二条");
        fs::remove_dir_all(&root).ok();
    }

}
