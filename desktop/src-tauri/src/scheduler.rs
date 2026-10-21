//! 应用运行期调度器配置（OD-ARCH-046）。
//!
//! 两类触发：
//!   * **定时 AI 复盘**：到点后由前端触发一次模型调用；结果先落成“待确认写操作”，
//!     用户确认后才真正写文件。模型不可用 / 超时 / 权限关闭时**不写文件**，
//!     只回退到本地规则提醒，并把原因写进界面。
//!   * **心跳**：只做本地确定性扫描（逾期 / 超期），不调模型、不写文件。
//!
//! 本模块只负责**配置与到点判定**——不常驻、不起系统定时任务、不开线程。
//! 应用关闭期间不会执行；错过触发时下一次打开补跑一次（`last_*_at` 记录）。
//!
//! 配置存放在应用数据根 `config/scheduler.v1.json`，与成果目录分开。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::flow_pack::FlowPack;
use crate::task_shell;

pub const SCHEDULER_SCHEMA: &str = "omnidesk.scheduler.v1";
pub const SCHEDULER_FILE: &str = "scheduler.v1.json";
pub const DEFAULT_HEARTBEAT_MINUTES: u32 = 120;
pub const MIN_HEARTBEAT_MINUTES: u32 = 5;
pub const MAX_HEARTBEAT_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerConfig {
    pub schema: String,
    /// 定时 AI 复盘总开关。默认关闭。
    #[serde(default)]
    pub review_enabled: bool,
    /// daily / weekly / once
    #[serde(default = "default_cadence")]
    pub cadence: String,
    /// 触发小时（0-23，按用户本地时区）。
    #[serde(default = "default_hour")]
    pub hour: u32,
    /// 一次性触发的本地时间戳（秒）；仅 cadence=once 使用。
    #[serde(default)]
    pub once_at: Option<f64>,
    /// 用户本地时区相对 UTC 的分钟偏移（东八区 = 480）。
    #[serde(default)]
    pub utc_offset_minutes: i32,
    /// 心跳开关。默认开启，但只做本地规则扫描。
    #[serde(default = "default_true")]
    pub heartbeat_enabled: bool,
    #[serde(default = "default_heartbeat")]
    pub heartbeat_minutes: u32,
    #[serde(default)]
    pub last_review_at: f64,
    #[serde(default)]
    pub last_heartbeat_at: f64,
}

fn default_cadence() -> String {
    "weekly".to_string()
}

fn default_hour() -> u32 {
    9
}

fn default_true() -> bool {
    true
}

fn default_heartbeat() -> u32 {
    DEFAULT_HEARTBEAT_MINUTES
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            schema: SCHEDULER_SCHEMA.to_string(),
            review_enabled: false,
            cadence: default_cadence(),
            hour: default_hour(),
            once_at: None,
            utc_offset_minutes: 480,
            heartbeat_enabled: true,
            heartbeat_minutes: DEFAULT_HEARTBEAT_MINUTES,
            last_review_at: 0.0,
            last_heartbeat_at: 0.0,
        }
    }
}

impl SchedulerConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEDULER_SCHEMA {
            return Err(format!(
                "不支持的调度配置版本 {:?}；当前只接受 {:?}",
                self.schema, SCHEDULER_SCHEMA
            ));
        }
        if !matches!(self.cadence.as_str(), "daily" | "weekly" | "once") {
            return Err(format!("未知的复盘频率：{:?}", self.cadence));
        }
        if self.hour > 23 {
            return Err("触发小时必须在 0-23 之间。".to_string());
        }
        if self.cadence == "once" {
            let at = self.once_at.ok_or_else(|| "一次性触发需要给出时间。".to_string())?;
            if !at.is_finite() || at <= 0.0 {
                return Err("一次性触发时间不合法。".to_string());
            }
        }
        if self.utc_offset_minutes < -12 * 60 || self.utc_offset_minutes > 14 * 60 {
            return Err("时区偏移超出合理范围。".to_string());
        }
        if self.heartbeat_minutes < MIN_HEARTBEAT_MINUTES
            || self.heartbeat_minutes > MAX_HEARTBEAT_MINUTES
        {
            return Err(format!(
                "心跳间隔必须在 {}–{} 分钟之间。",
                MIN_HEARTBEAT_MINUTES, MAX_HEARTBEAT_MINUTES
            ));
        }
        Ok(())
    }
}

pub fn config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SCHEDULER_FILE)
}

pub fn load(config_dir: &Path) -> Result<SchedulerConfig, String> {
    let path = config_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SchedulerConfig::default()),
        Err(e) => return Err(format!("读取调度配置失败（{}）：{}", path.display(), e)),
    };
    let config: SchedulerConfig = serde_json::from_str(&text)
        .map_err(|e| format!("调度配置格式错误（{}）：{}", path.display(), e))?;
    config.validate()?;
    Ok(config)
}

pub fn write(config_dir: &Path, config: &SchedulerConfig) -> Result<(), String> {
    config.validate()?;
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建配置目录失败（{}）：{}", config_dir.display(), e))?;
    let target = config_path(config_dir);
    let temp = config_dir.join(format!("{}tmp-{}", SCHEDULER_FILE, std::process::id()));
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化调度配置失败：{}", e))?;
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入调度配置失败：{}", e))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入调度配置失败：{}", e))?;
        fs::rename(&temp, &target).map_err(|e| format!("替换调度配置失败：{}", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 本地时间拆分（不依赖外部 crate）：返回 (日期序数, 时, 分, 星期几 0=周日)。
fn local_parts(now: f64, offset_minutes: i32) -> (i64, u32, u32, u32) {
    let secs = now.floor() as i64 + offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    // 1970-01-01 是星期四（4）。0=周日。
    let weekday = ((days + 4).rem_euclid(7)) as u32;
    (days, hour, minute, weekday)
}

/// 定时复盘是否到点。纯计算，不读时钟、不写文件。
pub fn review_due(config: &SchedulerConfig, now: f64) -> bool {
    if !config.review_enabled {
        return false;
    }
    let (days, hour, _minute, weekday) = local_parts(now, config.utc_offset_minutes);
    match config.cadence.as_str() {
        "once" => match config.once_at {
            Some(at) => now >= at && config.last_review_at < at,
            None => false,
        },
        "daily" => {
            if hour < config.hour {
                return false;
            }
            let (last_days, ..) = local_parts(config.last_review_at, config.utc_offset_minutes);
            config.last_review_at <= 0.0 || last_days < days
        }
        "weekly" => {
            // 每周一（weekday == 1）。
            if weekday != 1 || hour < config.hour {
                return false;
            }
            let last = config.last_review_at;
            if last <= 0.0 {
                return true;
            }
            let (last_days, ..) = local_parts(last, config.utc_offset_minutes);
            last_days < days
        }
        _ => false,
    }
}

/// 心跳是否到点。默认 2 小时；上次执行时间缺失时立即算一次。
pub fn heartbeat_due(config: &SchedulerConfig, now: f64) -> bool {
    if !config.heartbeat_enabled {
        return false;
    }
    if config.last_heartbeat_at <= 0.0 {
        return true;
    }
    now - config.last_heartbeat_at >= config.heartbeat_minutes as f64 * 60.0
}

/// 给界面看的下一步时间说明（只做提示，不参与判定）。
pub fn schedule_hint(config: &SchedulerConfig) -> String {
    if !config.review_enabled {
        return "未开启".to_string();
    }
    match config.cadence.as_str() {
        "once" => match config.once_at {
            Some(at) => format!("一次性：{}", task_shell::utc_iso(at)),
            None => "一次性：时间未设置".to_string(),
        },
        "daily" => format!("每天 {:02}:00（本地时间）", config.hour),
        "weekly" => format!("每周一 {:02}:00（本地时间）", config.hour),
        _ => "未知频率".to_string(),
    }
}

/// 心跳扫描结果：与「手动逾期扫描」共用同一条确定性汇总。
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeartbeatScan {
    pub ok: bool,
    pub markdown: Option<String>,
    pub overdue: u64,
    pub total: u64,
    pub error: Option<String>,
}

/// 心跳扫描：只做本地确定性计算，不写文件、不调模型。
/// 输入为空时明确返回“没有可扫描事项”，不伪造 0 逾期为“一切正常”。
pub fn heartbeat_scan(items_text: &str, today: Option<&str>, pack: &FlowPack) -> HeartbeatScan {
    if items_text.trim().is_empty() {
        return HeartbeatScan {
            ok: false,
            markdown: None,
            overdue: 0,
            total: 0,
            error: Some("没有可扫描的事项（还没有输入跟进记录）。".to_string()),
        };
    }
    let response = task_shell::project_flow_with(items_text, today, pack);
    match response.summary {
        Some(summary) => HeartbeatScan {
            ok: response.ok,
            markdown: response.markdown,
            overdue: summary.overdue,
            total: summary.total,
            error: response.error,
        },
        None => HeartbeatScan {
            ok: false,
            markdown: response.markdown,
            overdue: 0,
            total: 0,
            error: response.error,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SchedulerConfig {
        SchedulerConfig {
            utc_offset_minutes: 480,
            ..SchedulerConfig::default()
        }
    }

    #[test]
    fn once_fires_after_time_and_only_once() {
        let mut c = config();
        c.review_enabled = true;
        c.cadence = "once".to_string();
        c.once_at = Some(1_800_000_060.0);
        assert!(!review_due(&c, 1_800_000_000.0), "未到点不应触发");
        assert!(review_due(&c, 1_800_000_060.0), "到点应触发");
        c.last_review_at = 1_800_000_061.0;
        assert!(!review_due(&c, 1_800_000_120.0), "已执行后不应重复触发");
    }

    #[test]
    fn disabled_review_never_fires() {
        let mut c = config();
        c.cadence = "once".to_string();
        c.once_at = Some(1_800_000_000.0);
        assert!(!review_due(&c, 1_900_000_000.0));
    }

    #[test]
    fn daily_fires_at_hour_once_per_day() {
        let mut c = config();
        c.review_enabled = true;
        c.cadence = "daily".to_string();
        c.hour = 9;
        // 2026-10-21 08:59 +08:00 => 00:59 UTC（epoch 1792544340）
        let before = 1_792_544_340.0;
        assert!(!review_due(&c, before));
        // 2026-10-21 09:01 +08:00 => 01:01 UTC
        let after = before + 120.0;
        assert!(review_due(&c, after));
        c.last_review_at = after;
        assert!(!review_due(&c, after + 3600.0));
    }

    #[test]
    fn heartbeat_interval_respected() {
        let mut c = config();
        c.heartbeat_minutes = 120;
        assert!(heartbeat_due(&c, 1_800_000_000.0), "从未跑过应立即扫描");
        c.last_heartbeat_at = 1_800_000_000.0;
        assert!(!heartbeat_due(&c, 1_800_000_000.0 + 60.0));
        assert!(heartbeat_due(&c, 1_800_000_000.0 + 7_200.0));
        c.heartbeat_enabled = false;
        assert!(!heartbeat_due(&c, 1_800_000_000.0 + 99_999.0));
    }

    #[test]
    fn config_round_trip_and_validation() {
        let dir = std::env::temp_dir().join(format!("omnidesk-scheduler-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let mut c = config();
        c.review_enabled = true;
        c.cadence = "weekly".to_string();
        write(&dir, &c).expect("应可写入");
        assert_eq!(load(&dir).unwrap(), c);

        let mut bad = c.clone();
        bad.hour = 24;
        assert!(write(&dir, &bad).is_err());
        let mut bad = c.clone();
        bad.heartbeat_minutes = 1;
        assert!(write(&dir, &bad).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn heartbeat_scan_matches_manual_scan() {
        let pack = crate::flow_pack::default_pack();
        let items = "合成事项 A | P1 | 2026-10-10 | in_progress | 补齐说明\n合成事项 B | P2 | 2026-10-30 | not_started | 排期";
        let manual = task_shell::project_flow_with(items, Some("2026-10-21"), &pack);
        let heartbeat = heartbeat_scan(items, Some("2026-10-21"), &pack);
        assert!(heartbeat.ok, "心跳扫描应成功：{:?}", heartbeat.error);
        assert_eq!(heartbeat.markdown, manual.markdown, "心跳与手动扫描必须同一结果");
        assert_eq!(heartbeat.overdue, 1);
        assert_eq!(heartbeat.total, 2);

        let empty = heartbeat_scan("   ", Some("2026-10-21"), &pack);
        assert!(!empty.ok);
        assert!(empty.markdown.is_none());
    }
}
