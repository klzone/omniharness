//! 个人工作时间配置（OD-QS-406）。
//!
//! 配置落在 <work_root>/availability.v1.json，schema omnidesk.availability.v1。
//! 没有文件时使用内置默认值且不写盘（干净底座纪律）；只有用户显式保存才创建文件。
//! 这里只描述可用时间，不接外部日历，也不生成排期。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{assert_work_root, is_date_shape};

pub const AVAILABILITY_FILE: &str = "availability.v1.json";
pub const AVAILABILITY_SCHEMA: &str = "omnidesk.availability.v1";
pub const DEFAULT_CAPACITY_HOURS: f64 = 6.0;
pub const MIN_CAPACITY_HOURS: f64 = 0.5;
pub const MAX_CAPACITY_HOURS: f64 = 12.0;

/// 一个连续时段，时间形状 HH:MM（本地时间）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeWindow {
    pub start: String,
    pub end: String,
}

/// 某一天的一段不可用时间（休假、外出等）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableBlock {
    pub date: String,
    pub start: String,
    pub end: String,
    pub reason: String,
}

/// 工作时间配置。缺字段按默认值补齐，兼容手改过的部分文件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvailabilityConfig {
    #[serde(default = "default_schema")]
    pub schema: String,
    /// 工作日：1=周一 … 7=周日。
    #[serde(default = "default_workdays")]
    pub workdays: Vec<u32>,
    #[serde(default = "default_windows")]
    pub daily_windows: Vec<TimeWindow>,
    #[serde(default)]
    pub breaks: Vec<TimeWindow>,
    #[serde(default)]
    pub unavailable: Vec<UnavailableBlock>,
    #[serde(default = "default_capacity")]
    pub daily_capacity_hours: f64,
    #[serde(default = "system_timezone_name")]
    pub timezone: String,
}

fn default_schema() -> String {
    AVAILABILITY_SCHEMA.to_string()
}

fn default_workdays() -> Vec<u32> {
    vec![1, 2, 3, 4, 5]
}

fn default_windows() -> Vec<TimeWindow> {
    vec![
        TimeWindow {
            start: "09:00".to_string(),
            end: "12:00".to_string(),
        },
        TimeWindow {
            start: "14:00".to_string(),
            end: "18:00".to_string(),
        },
    ]
}

fn default_capacity() -> f64 {
    DEFAULT_CAPACITY_HOURS
}

impl Default for AvailabilityConfig {
    fn default() -> Self {
        Self {
            schema: default_schema(),
            workdays: default_workdays(),
            daily_windows: default_windows(),
            breaks: Vec::new(),
            unavailable: Vec::new(),
            daily_capacity_hours: default_capacity(),
            timezone: system_timezone_name(),
        }
    }
}

/// 系统时区名：优先 TZ，其次 /etc/localtime 指向的 zoneinfo 路径，最后 UTC。
pub fn system_timezone_name() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let trimmed = tz.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(target) = fs::read_link("/etc/localtime") {
        let text = target.to_string_lossy();
        if let Some(index) = text.find("zoneinfo/") {
            let name = text[index + "zoneinfo/".len()..].trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    "UTC".to_string()
}

fn availability_path(root: &Path) -> PathBuf {
    root.join(AVAILABILITY_FILE)
}

fn minutes_of(value: &str, field: &str) -> Result<u32, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return Err(format!("{field}时间格式应为 HH:MM：{value:?}"));
    }
    let hour: u32 = value[0..2]
        .parse()
        .map_err(|_| format!("{field}时间格式应为 HH:MM：{value:?}"))?;
    let minute: u32 = value[3..5]
        .parse()
        .map_err(|_| format!("{field}时间格式应为 HH:MM：{value:?}"))?;
    if hour > 23 || minute > 59 {
        return Err(format!("{field}时间超出合理范围：{value:?}"));
    }
    Ok(hour * 60 + minute)
}

fn validate_window(window: &TimeWindow, field: &str) -> Result<(u32, u32), String> {
    let start = minutes_of(&window.start, field)?;
    let end = minutes_of(&window.end, field)?;
    if start >= end {
        return Err(format!(
            "{field}的结束时间必须晚于开始时间：{}–{}",
            window.start, window.end
        ));
    }
    Ok((start, end))
}

fn ensure_no_overlap(windows: &[TimeWindow], field: &str) -> Result<(), String> {
    let mut spans = Vec::new();
    for window in windows {
        spans.push(validate_window(window, field)?);
    }
    spans.sort_by_key(|span| span.0);
    for pair in spans.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(format!("{field}里有重叠时段，请先合并再保存。"));
        }
    }
    Ok(())
}

pub fn validate(config: &AvailabilityConfig) -> Result<(), String> {
    if config.schema != AVAILABILITY_SCHEMA {
        return Err(format!("工作时间配置 schema 不认识：{}", config.schema));
    }
    if config.workdays.is_empty() {
        return Err("至少选择一个工作日。".to_string());
    }
    let mut seen = HashSet::new();
    for day in &config.workdays {
        if !(1..=7).contains(day) {
            return Err(format!("工作日必须在 1 到 7 之间（1=周一）：{day}"));
        }
        if !seen.insert(*day) {
            return Err(format!("工作日重复：{day}"));
        }
    }
    if config.daily_windows.is_empty() {
        return Err("至少配置一个每日工作时段。".to_string());
    }
    ensure_no_overlap(&config.daily_windows, "每日工作时段")?;
    ensure_no_overlap(&config.breaks, "休息时段")?;
    for block in &config.unavailable {
        if !is_date_shape(&block.date) {
            return Err(format!("不可用日期形状不正确：{}", block.date));
        }
        validate_window(
            &TimeWindow {
                start: block.start.clone(),
                end: block.end.clone(),
            },
            "不可用时段",
        )?;
        if block.reason.trim().is_empty() {
            return Err(format!("不可用日期 {} 缺少原因。", block.date));
        }
    }
    if !config.daily_capacity_hours.is_finite()
        || config.daily_capacity_hours < MIN_CAPACITY_HOURS
        || config.daily_capacity_hours > MAX_CAPACITY_HOURS
    {
        return Err(format!(
            "每日容量必须在 {MIN_CAPACITY_HOURS} 到 {MAX_CAPACITY_HOURS} 小时之间：{}",
            config.daily_capacity_hours
        ));
    }
    let timezone = config.timezone.trim();
    if timezone.is_empty() {
        return Err("时区不能为空。".to_string());
    }
    if timezone.chars().count() > 64 || timezone.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("时区名不合法：{:?}", config.timezone));
    }
    Ok(())
}

/// 读取配置。文件不存在时返回默认值，不创建任何文件。
pub fn load(root: &Path) -> Result<AvailabilityConfig, String> {
    assert_work_root(root)?;
    let path = availability_path(root);
    if !path.exists() {
        return Ok(AvailabilityConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取工作时间配置失败（{}）：{}", path.display(), e))?;
    let config: AvailabilityConfig = serde_json::from_str(&raw)
        .map_err(|e| format!("工作时间配置格式错误（{}）：{}", path.display(), e))?;
    validate(&config)?;
    Ok(config)
}

/// 保存配置。先校验再原子替换；失败时旧文件保持不变。
pub fn save(root: &Path, config: AvailabilityConfig) -> Result<AvailabilityConfig, String> {
    assert_work_root(root)?;
    let mut config = config;
    config.schema = AVAILABILITY_SCHEMA.to_string();
    config.timezone = config.timezone.trim().to_string();
    for window in config
        .daily_windows
        .iter_mut()
        .chain(config.breaks.iter_mut())
    {
        window.start = window.start.trim().to_string();
        window.end = window.end.trim().to_string();
    }
    for block in config.unavailable.iter_mut() {
        block.date = block.date.trim().to_string();
        block.start = block.start.trim().to_string();
        block.end = block.end.trim().to_string();
        block.reason = block.reason.trim().to_string();
    }
    validate(&config)?;
    fs::create_dir_all(root)
        .map_err(|e| format!("创建工作时间配置目录失败（{}）：{}", root.display(), e))?;
    let target = availability_path(root);
    let temp = root.join(format!("{AVAILABILITY_FILE}.tmp-{}", std::process::id()));
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化工作时间配置失败：{e}"))?;
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入工作时间配置失败：{e}"))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入工作时间配置失败：{e}"))?;
        fs::rename(&temp, &target).map_err(|e| format!("替换工作时间配置失败：{e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-qs406-availability-{tag}-{}-{}",
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

    fn window(start: &str, end: &str) -> TimeWindow {
        TimeWindow {
            start: start.to_string(),
            end: end.to_string(),
        }
    }

    fn sample() -> AvailabilityConfig {
        AvailabilityConfig {
            schema: AVAILABILITY_SCHEMA.to_string(),
            workdays: vec![1, 2, 3, 4, 5, 6],
            daily_windows: vec![window("08:30", "12:00"), window("13:30", "17:30")],
            breaks: vec![window("10:00", "10:15")],
            unavailable: vec![UnavailableBlock {
                date: "2026-12-24".to_string(),
                start: "09:00".to_string(),
                end: "18:00".to_string(),
                reason: "休假".to_string(),
            }],
            daily_capacity_hours: 5.5,
            timezone: "Asia/Shanghai".to_string(),
        }
    }

    #[test]
    fn defaults_are_complete_and_do_not_write() {
        let temp = TempRoot::new("defaults");
        let config = load(temp.root()).unwrap();
        assert_eq!(config.workdays, vec![1, 2, 3, 4, 5]);
        assert_eq!(config.daily_windows.len(), 2);
        assert_eq!(config.daily_windows[0].start, "09:00");
        assert_eq!(config.daily_windows[0].end, "12:00");
        assert_eq!(config.daily_windows[1].start, "14:00");
        assert!(config.breaks.is_empty());
        assert!(config.unavailable.is_empty());
        assert_eq!(config.daily_capacity_hours, DEFAULT_CAPACITY_HOURS);
        assert!(!config.timezone.is_empty());
        assert!(!availability_path(temp.root()).exists(), "读取默认值不应创建文件");
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp = TempRoot::new("roundtrip");
        let saved = save(temp.root(), sample()).unwrap();
        let loaded = load(temp.root()).unwrap();
        assert_eq!(loaded, saved);
        assert_eq!(loaded.daily_capacity_hours, 5.5);
        let raw = fs::read_to_string(availability_path(temp.root())).unwrap();
        assert!(raw.contains("omnidesk.availability.v1"));
        assert!(raw.contains("Asia/Shanghai"));
    }

    #[test]
    fn partial_file_fills_defaults() {
        let temp = TempRoot::new("partial");
        fs::write(
            availability_path(temp.root()),
            "{\"workdays\":[6,7],\"daily_capacity_hours\":4}",
        )
        .unwrap();
        let config = load(temp.root()).unwrap();
        assert_eq!(config.workdays, vec![6, 7]);
        assert_eq!(config.daily_windows.len(), 2);
        assert_eq!(config.daily_capacity_hours, 4.0);
        assert_eq!(config.schema, AVAILABILITY_SCHEMA);
        assert!(!config.timezone.is_empty());
    }

    #[test]
    fn rejects_unknown_schema() {
        let temp = TempRoot::new("schema");
        fs::write(
            availability_path(temp.root()),
            "{\"schema\":\"other.v1\"}",
        )
        .unwrap();
        let err = load(temp.root()).unwrap_err();
        assert!(err.contains("schema 不认识"), "{err}");
    }

    #[test]
    fn rejects_bad_workday() {
        let mut config = sample();
        config.workdays = vec![0, 3];
        assert!(validate(&config).unwrap_err().contains("1 到 7"));
        config.workdays = vec![3, 3];
        assert!(validate(&config).unwrap_err().contains("重复"));
        config.workdays.clear();
        assert!(validate(&config).unwrap_err().contains("至少选择一个工作日"));
    }

    #[test]
    fn rejects_bad_time_shape_and_inverted_window() {
        let mut config = sample();
        config.daily_windows = vec![window("9:00", "12:00")];
        assert!(validate(&config).unwrap_err().contains("HH:MM"));
        config.daily_windows = vec![window("12:00", "09:00")];
        assert!(validate(&config).unwrap_err().contains("结束时间必须晚于开始时间"));
        config.daily_windows = vec![window("09:00", "24:00")];
        assert!(validate(&config).unwrap_err().contains("超出合理范围"));
    }

    #[test]
    fn rejects_overlapping_windows() {
        let mut config = sample();
        config.daily_windows = vec![window("09:00", "12:00"), window("11:00", "15:00")];
        let err = validate(&config).unwrap_err();
        assert!(err.contains("重叠"), "{err}");
        config.daily_windows = vec![window("09:00", "12:00")];
        config.breaks = vec![window("11:00", "13:00"), window("12:30", "14:00")];
        let err = validate(&config).unwrap_err();
        assert!(err.contains("休息时段") && err.contains("重叠"), "{err}");
    }

    #[test]
    fn rejects_capacity_outside_bounds() {
        for capacity in [0.0, -1.0, 12.5, f64::NAN] {
            let mut config = sample();
            config.daily_capacity_hours = capacity;
            let err = validate(&config).unwrap_err();
            assert!(err.contains("每日容量"), "{capacity}: {err}");
        }
        let mut config = sample();
        config.daily_capacity_hours = 12.0;
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn rejects_bad_unavailable_block() {
        let mut config = sample();
        config.unavailable = vec![UnavailableBlock {
            date: "2026-13-01".to_string(),
            start: "09:00".to_string(),
            end: "10:00".to_string(),
            reason: "休假".to_string(),
        }];
        assert!(validate(&config).unwrap_err().contains("日期形状"));
        config.unavailable = vec![UnavailableBlock {
            date: "2026-12-24".to_string(),
            start: "09:00".to_string(),
            end: "10:00".to_string(),
            reason: "   ".to_string(),
        }];
        assert!(validate(&config).unwrap_err().contains("缺少原因"));
        config.unavailable = vec![UnavailableBlock {
            date: "2026-12-24".to_string(),
            start: "18:00".to_string(),
            end: "09:00".to_string(),
            reason: "休假".to_string(),
        }];
        assert!(validate(&config).unwrap_err().contains("结束时间必须晚于开始时间"));
    }

    #[test]
    fn rejects_empty_timezone() {
        let mut config = sample();
        config.timezone = "   ".to_string();
        assert!(validate(&config).unwrap_err().contains("时区不能为空"));
        config.timezone = "Asia/Shang hai".to_string();
        assert!(validate(&config).unwrap_err().contains("时区名不合法"));
    }

    #[test]
    fn failed_save_keeps_previous_file_and_leaves_no_temp() {
        let temp = TempRoot::new("atomic");
        let first = save(temp.root(), sample()).unwrap();
        let before = fs::read_to_string(availability_path(temp.root())).unwrap();
        let mut broken = sample();
        broken.daily_windows = vec![window("09:00", "08:00")];
        let err = save(temp.root(), broken).unwrap_err();
        assert!(err.contains("结束时间"), "{err}");
        let after = fs::read_to_string(availability_path(temp.root())).unwrap();
        assert_eq!(before, after, "校验失败时旧文件保持不变");
        let leftovers: Vec<String> = fs::read_dir(temp.root())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert_eq!(load(temp.root()).unwrap(), first);
    }
}
