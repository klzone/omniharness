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

}
