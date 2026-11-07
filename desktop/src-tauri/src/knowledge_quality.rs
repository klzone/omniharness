//! 知识库质量检查（OD-KB-201 S4）。
//!
//! 只做确定性检查，不调用模型：
//!   * 索引行里的 YYYY-MM-DD 日期；
//!   * raw 准入治理记录里的准入日期；
//!   * 超过可配置天数阈值的条目标记为「可能过期」。
//!
//! 阈值配置在 `<应用数据根>/config/knowledge-quality.v1.json`：
//! `{"schema":"omnidesk.knowledge-quality.v1","expiry_days":180}`；
//! 文件缺失时使用 180 天默认值，损坏时明确报错（不静默放宽）。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::knowledge_governance::{self, GovernanceRecord};
use crate::knowledge_ingest::{rfc3339_local, INDEX_REL};

pub const QUALITY_CONFIG_FILE: &str = "knowledge-quality.v1.json";
pub const QUALITY_SCHEMA: &str = "omnidesk.knowledge-quality.v1";
pub const DEFAULT_EXPIRY_DAYS: i64 = 180;
const MAX_EXPIRY_DAYS: i64 = 3650;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualityConfig {
    pub schema: String,
    pub expiry_days: i64,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            schema: QUALITY_SCHEMA.to_string(),
            expiry_days: DEFAULT_EXPIRY_DAYS,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct QualityConfigFile {
    schema: String,
    #[serde(default = "default_expiry_days")]
    expiry_days: i64,
}

fn default_expiry_days() -> i64 {
    DEFAULT_EXPIRY_DAYS
}

pub fn load_config(config_dir: &Path) -> Result<QualityConfig, String> {
    let path = config_dir.join(QUALITY_CONFIG_FILE);
    if !path.exists() {
        return Ok(QualityConfig::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取质量配置失败（{}）：{}", path.display(), e))?;
    let file: QualityConfigFile = serde_json::from_str(&text)
        .map_err(|e| format!("质量配置格式错误（{}）：{}", path.display(), e))?;
    if file.schema != QUALITY_SCHEMA {
        return Err(format!(
            "不支持的质量配置版本：{:?}；只接受 {:?}",
            file.schema, QUALITY_SCHEMA
        ));
    }
    if file.expiry_days < 1 || file.expiry_days > MAX_EXPIRY_DAYS {
        return Err(format!(
            "过期阈值必须在 1–{MAX_EXPIRY_DAYS} 天之间，实际是 {}",
            file.expiry_days
        ));
    }
    Ok(QualityConfig {
        schema: file.schema,
        expiry_days: file.expiry_days,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpiredItem {
    /// card = 索引里的知识条目；raw = 原始资料。
    pub kind: String,
    pub subject: String,
    pub date: String,
    pub age_days: i64,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct QualityReport {
    pub threshold_days: i64,
    pub scanned_at: i64,
    pub expired: Vec<ExpiredItem>,
    pub index_lines: usize,
    pub raw_records: usize,
}

/// Howard Hinnant 的 days-from-civil；输入非法日期返回 None。
fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day > max_day {
        return None;
    }
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn date_of(text: &str) -> Option<(String, i64)> {
    let bytes = text.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    for start in 0..=bytes.len() - 10 {
        let slice = &bytes[start..start + 10];
        if slice[4] != b'-' || slice[7] != b'-' {
            continue;
        }
        if !slice
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        {
            continue;
        }
        let year = text[start..start + 4].parse::<i64>().ok()?;
        let month = text[start + 5..start + 7].parse::<i64>().ok()?;
        let day = text[start + 8..start + 10].parse::<i64>().ok()?;
        if let Some(days) = days_from_civil(year, month, day) {
            return Some((text[start..start + 10].to_string(), days));
        }
    }
    None
}

fn link_target(line: &str) -> Option<String> {
    let start = line.find("](")? + 2;
    let end = line[start..].find(')')? + start;
    let raw = line[start..end].trim();
    let target = raw
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or(raw)
        .trim();
    if target.is_empty() || target.starts_with("http://") || target.starts_with("https://") {
        return None;
    }
    if target.starts_with("概念/") || target.starts_with("方法/") || target.starts_with("原文/") {
        return Some(format!("知识库/{target}"));
    }
    if target.starts_with("raw/") || target.starts_with("知识库/") {
        return Some(target.to_string());
    }
    Some(target.to_string())
}

fn subject_kind(subject: &str) -> &'static str {
    if subject.starts_with("raw/") { "raw" } else { "card" }
}

fn collect_index_dates(
    index: &str,
    dates: &mut BTreeMap<(String, String), (i64, String, String)>,
) -> usize {
    let mut count = 0;
    for line in index.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            continue;
        }
        let Some(subject) = link_target(trimmed) else {
            continue;
        };
        let Some((date, days)) = date_of(trimmed) else {
            continue;
        };
        count += 1;
        let kind = subject_kind(&subject);
        let key = (kind.to_string(), subject);
        let entry = dates.entry(key).or_insert((days, date.clone(), "索引".to_string()));
        if days < entry.0 {
            entry.0 = days;
            entry.1 = date;
        }
    }
    count
}

fn collect_raw_records(
    records: &[GovernanceRecord],
    dates: &mut BTreeMap<(String, String), (i64, String, String)>,
) -> usize {
    let mut count = 0;
    for record in records {
        if record.action != "admit" || !record.subject.starts_with("raw/") {
            continue;
        }
        let Some((date, days)) = date_of(&record.at) else {
            continue;
        };
        count += 1;
        let key = ("raw".to_string(), record.subject.clone());
        let entry = dates.entry(key).or_insert((days, date.clone(), "治理准入".to_string()));
        if days < entry.0 {
            entry.0 = days;
            entry.1 = date;
            entry.2 = "治理准入".to_string();
        }
    }
    count
}

/// 固定时间注入的过期检查；阈值按自然日计算，未来日期不算过期。
pub fn check_expiry(
    root: &Path,
    now_unix: i64,
    utc_offset_minutes: i32,
    threshold_days: i64,
) -> Result<QualityReport, String> {
    if threshold_days < 1 || threshold_days > MAX_EXPIRY_DAYS {
        return Err(format!(
            "过期阈值必须在 1–{MAX_EXPIRY_DAYS} 天之间，实际是 {threshold_days}"
        ));
    }
    let local = rfc3339_local(now_unix, utc_offset_minutes);
    let Some((_, today)) = date_of(&local) else {
        return Err("无法计算当前本地日期".to_string());
    };
    let index = fs::read_to_string(root.join(INDEX_REL)).unwrap_or_default();
    let mut dates: BTreeMap<(String, String), (i64, String, String)> = BTreeMap::new();
    let index_lines = collect_index_dates(&index, &mut dates);
    let raw = knowledge_governance::list_records(root, knowledge_governance::KIND_RAW_ADMISSIONS)
        .unwrap_or_default();
    let raw_records = collect_raw_records(&raw, &mut dates);

    let mut expired: Vec<ExpiredItem> = dates
        .into_iter()
        .filter_map(|((kind, subject), (days, date, source))| {
            let age_days = today - days;
            if age_days > threshold_days {
                Some(ExpiredItem {
                    kind,
                    subject,
                    date,
                    age_days,
                    source,
                })
            } else {
                None
            }
        })
        .collect();
    expired.sort_by(|a, b| b.age_days.cmp(&a.age_days).then_with(|| a.subject.cmp(&b.subject)));
    Ok(QualityReport {
        threshold_days,
        scanned_at: now_unix,
        expired,
        index_lines,
        raw_records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_governance::{GovernanceEntry, KIND_RAW_ADMISSIONS};
    use std::path::PathBuf;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-quality-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn day(year: i64, month: i64, day: i64) -> i64 {
        days_from_civil(year, month, day).unwrap() * 86_400
    }

    fn seed(root: &Path) {
        crate::knowledge_ingest::init_vault(root).unwrap();
        let index = "# 知识库索引\n\n## 概念\n- [Old](概念/old.md) | date: 2020-01-01\n- [Fresh](概念/fresh.md) | date: 2026-10-20\n\n## 原始资料\n- [old.pdf](<raw/old.pdf>) | 准入时间：2020-01-01T00:00:00+00:00 | sha256:abc | 1 字节\n";
        fs::write(root.join(INDEX_REL), index).unwrap();
        let entry = GovernanceEntry {
            action: "admit".to_string(),
            subject: "raw/old.pdf".to_string(),
            sha256: "abcdef012345".to_string(),
            note: "old raw".to_string(),
        };
        knowledge_governance::append_record_at(
            root,
            KIND_RAW_ADMISSIONS,
            &entry,
            &crate::profile_rules::RulePolicy::clean(),
            day(2020, 1, 1),
            0,
        )
        .unwrap();
    }

    #[test]
    fn expiry_threshold_uses_fixed_clock_and_natural_days() {
        let root = root("expiry");
        seed(&root);
        let report = check_expiry(&root, day(2026, 11, 7), 0, 180).unwrap();
        assert_eq!(report.threshold_days, 180);
        assert!(report.expired.iter().any(|item| item.subject == "知识库/概念/old.md"));
        assert!(report.expired.iter().any(|item| item.subject == "raw/old.pdf"));
        assert!(!report.expired.iter().any(|item| item.subject == "知识库/概念/fresh.md"));
        assert!(report.index_lines >= 3);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn exactly_threshold_days_is_not_expired_but_one_more_is() {
        let root = root("boundary");
        crate::knowledge_ingest::init_vault(&root).unwrap();
        let threshold = 30;
        let today = day(2026, 11, 7);
        let exactly = today - threshold * 86_400;
        let over = today - (threshold + 1) * 86_400;
        let exactly_date = rfc3339_local(exactly, 0);
        let over_date = rfc3339_local(over, 0);
        let index = format!(
            "# 知识库索引\n\n## 概念\n- [Exactly](概念/exactly.md) | date: {}\n- [Over](概念/over.md) | date: {}\n",
            &exactly_date[..10],
            &over_date[..10]
        );
        fs::write(root.join(INDEX_REL), index).unwrap();
        let report = check_expiry(&root, today, 0, threshold).unwrap();
        assert!(!report.expired.iter().any(|item| item.subject.contains("exactly")));
        assert!(report.expired.iter().any(|item| item.subject.contains("over")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_dates_are_ignored() {
        let root = root("invalid");
        crate::knowledge_ingest::init_vault(&root).unwrap();
        let index = "# 知识库索引\n\n## 概念\n- [Bad](概念/bad.md) | date: 2026-13-40\n- [None](概念/none.md) | no-date\n";
        fs::write(root.join(INDEX_REL), index).unwrap();
        let report = check_expiry(&root, day(2026, 11, 7), 0, 30).unwrap();
        assert!(report.expired.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_config_uses_default_and_bad_config_is_error() {
        let root = root("config");
        let config_dir = root.join("config");
        let loaded = load_config(&config_dir).unwrap();
        assert_eq!(loaded.expiry_days, DEFAULT_EXPIRY_DAYS);
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join(QUALITY_CONFIG_FILE),
            "{\"schema\":\"omnidesk.knowledge-quality.v1\",\"expiry_days\":0}",
        )
        .unwrap();
        assert!(load_config(&config_dir).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
