//! 待确认写操作（OD-ARCH-046 写确认制）。
//!
//! 凡是**模型或定时任务**发起的写动作（当前只有“保存离线成果”一种），都必须先在
//! 宿主侧登记一条待确认记录，再由用户在前端点“确认”。写入时宿主校验：
//!   * 记录存在且状态为 pending；
//!   * 未超过 24 小时有效期；
//!   * 请求内容（场景 / 标题 / 正文 / 是否模型辅助）与登记内容完全一致。
//!
//! 校验不通过一律拒绝；人工在界面上点“保存”的既有路径不经过本模块。
//! 这一层不依赖提示词，也不接受前端自述的“已确认”。
//!
//! 存储：应用数据根的 `engine-state/pending-writes/<id>.json`（仓库外）。
//! 记录里含待写入正文；确认 / 放弃 / 过期后保留记录本身作为审计线索，不写进仓库。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::task_shell::{utc_iso, SaveOfflineRequest, WriteSource};

pub const PENDING_WRITE_SCHEMA: &str = "omnidesk.pending-write.v1";
/// 确认有效期：24 小时。过期记录不能再执行写动作。
pub const PENDING_TTL_SECS: f64 = 24.0 * 3600.0;
const MAX_SUMMARY_CHARS: usize = 400;
const MAX_IMPACT_CHARS: usize = 800;
const MAX_MARKDOWN_BYTES: usize = 512 * 1024;

/// 写动作的负载。当前只有保存离线成果，字段与 `SaveOfflineRequest` 对齐。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingWritePayload {
    pub scenario: String,
    pub title: String,
    pub markdown: String,
    #[serde(default)]
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingWriteRecord {
    pub schema: String,
    pub id: String,
    /// 动作标识；未知动作一律拒绝，不做默认解释。
    pub action: String,
    /// model / scheduler；human 不允许走这条链路。
    pub source: String,
    pub summary: String,
    pub impact: String,
    pub payload: PendingWritePayload,
    pub created_at: f64,
    pub expires_at: f64,
    /// pending / consumed / rejected / expired
    pub status: String,
    pub resolved_at: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePendingWrite {
    pub action: String,
    pub source: String,
    pub summary: String,
    pub impact: String,
    pub payload: PendingWritePayload,
}

pub fn dir(engine_state: &Path) -> PathBuf {
    engine_state.join("pending-writes")
}

fn record_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    // id 只允许生成器给出的字符集；任何 `..`、分隔符或点号一律拒绝。
    if id.is_empty() || id.len() > 80 {
        return Err("待确认记录编号不合法。".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("待确认记录编号不合法。".to_string());
    }
    Ok(dir.join(format!("{}.json", id)))
}

pub fn validate_new(request: &CreatePendingWrite) -> Result<(), String> {
    if request.action != "save_offline" {
        return Err(format!("未知的待确认写动作：{:?}", request.action));
    }
    if !matches!(request.source.as_str(), "model" | "scheduler") {
        return Err("只有模型或定时任务发起的写动作需要确认记录；人工保存不走此链路。".to_string());
    }
    if request.summary.trim().is_empty() {
        return Err("待确认记录必须给出动作摘要。".to_string());
    }
    if request.summary.chars().count() > MAX_SUMMARY_CHARS {
        return Err("动作摘要过长，拒绝登记。".to_string());
    }
    if request.impact.trim().is_empty() {
        return Err("待确认记录必须说明影响范围。".to_string());
    }
    if request.impact.chars().count() > MAX_IMPACT_CHARS {
        return Err("影响范围说明过长，拒绝登记。".to_string());
    }
    if request.payload.markdown.trim().is_empty() {
        return Err("待写入正文为空，拒绝登记。".to_string());
    }
    if request.payload.markdown.len() > MAX_MARKDOWN_BYTES {
        return Err("待写入正文超过 512 KiB，拒绝登记。".to_string());
    }
    if !matches!(
        request.payload.scenario.as_str(),
        "weekly_report" | "material_writing" | "project_management"
    ) {
        return Err(format!("未知离线场景：{:?}", request.payload.scenario));
    }
    Ok(())
}

/// 登记一条待确认记录。只新建文件，绝不覆盖既有记录。
pub fn create(dir: &Path, request: &CreatePendingWrite, now: f64) -> Result<PendingWriteRecord, String> {
    validate_new(request)?;
    fs::create_dir_all(dir).map_err(|e| format!("创建待确认目录失败：{}", e))?;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = utc_iso(now)
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    let id = format!("pw-{}-{}-{}", stamp, std::process::id(), seq);
    let record = PendingWriteRecord {
        schema: PENDING_WRITE_SCHEMA.to_string(),
        id: id.clone(),
        action: request.action.clone(),
        source: request.source.clone(),
        summary: request.summary.trim().to_string(),
        impact: request.impact.trim().to_string(),
        payload: request.payload.clone(),
        created_at: now,
        expires_at: now + PENDING_TTL_SECS,
        status: "pending".to_string(),
        resolved_at: None,
    };
    let target = record_path(dir, &id)?;
    let content = serde_json::to_vec_pretty(&record)
        .map_err(|e| format!("序列化待确认记录失败：{}", e))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| format!("创建待确认记录失败（{}）：{}", target.display(), e))?;
    file.write_all(&content)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("写入待确认记录失败：{}", e))?;
    Ok(record)
}

pub fn load(dir: &Path, id: &str) -> Result<PendingWriteRecord, String> {
    let path = record_path(dir, id)?;
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取待确认记录失败（{}）：{}", path.display(), e))?;
    let record: PendingWriteRecord = serde_json::from_str(&text)
        .map_err(|e| format!("待确认记录格式错误（{}）：{}", path.display(), e))?;
    if record.schema != PENDING_WRITE_SCHEMA {
        return Err(format!("不支持的待确认记录版本：{:?}", record.schema));
    }
    Ok(record)
}

fn write_record(dir: &Path, record: &PendingWriteRecord) -> Result<(), String> {
    let path = record_path(dir, &record.id)?;
    let content = serde_json::to_vec_pretty(record)
        .map_err(|e| format!("序列化待确认记录失败：{}", e))?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入待确认记录失败：{}", e))?;
        file.write_all(&content)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入待确认记录失败：{}", e))?;
        fs::rename(&temp, &path).map_err(|e| format!("替换待确认记录失败：{}", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 过期判定：只对 pending 生效；已结算记录保持原状态。
fn effective_status(record: &PendingWriteRecord, now: f64) -> &'static str {
    if record.status == "pending" && now > record.expires_at {
        "expired"
    } else {
        match record.status.as_str() {
            "pending" => "pending",
            "consumed" => "consumed",
            "rejected" => "rejected",
            "expired" => "expired",
            _ => "unknown",
        }
    }
}

fn settle_expired(dir: &Path, record: &mut PendingWriteRecord, now: f64) {
    if record.status == "pending" && now > record.expires_at {
        record.status = "expired".to_string();
        record.resolved_at = Some(now);
        let _ = write_record(dir, record);
    }
}

/// 列出全部待确认记录（新的在前）。解析失败的记录跳过并计数，不伪造内容。
pub fn list(dir: &Path, now: f64) -> Result<(Vec<PendingWriteRecord>, u64), String> {
    if !dir.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut records = Vec::new();
    let mut broken = 0u64;
    let read_dir = fs::read_dir(dir).map_err(|e| format!("读取待确认目录失败：{}", e))?;
    for item in read_dir {
        let item = match item {
            Ok(item) => item,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        let mut record: PendingWriteRecord = match serde_json::from_str(&text) {
            Ok(record) => record,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        settle_expired(dir, &mut record, now);
        records.push(record);
    }
    records.sort_by(|a, b| {
        b.created_at
            .partial_cmp(&a.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok((records, broken))
}

/// 校验一条记录能否执行指定的写动作。
///
/// 不满足任一条件即拒绝：编号存在、动作匹配、状态 pending、未过期、来源非人工、
/// 负载与请求逐字段一致。
pub fn validate_for_write(
    dir: &Path,
    id: &str,
    expected: &PendingWritePayload,
    now: f64,
) -> Result<PendingWriteRecord, String> {
    let mut record = load(dir, id)?;
    if record.action != "save_offline" {
        return Err(format!("待确认记录的动作不是保存成果：{:?}", record.action));
    }
    if record.source == "human" {
        return Err("人工写动作不应携带确认记录。".to_string());
    }
    settle_expired(dir, &mut record, now);
    match record.status.as_str() {
        "pending" => {}
        "expired" => {
            return Err(format!(
                "该确认记录已于 {} 过期（有效期 24 小时），未执行写入。",
                utc_iso(record.expires_at)
            ))
        }
        "consumed" => return Err("该确认记录已执行过，不能重复写入。".to_string()),
        "rejected" => return Err("该确认记录已被放弃，未执行写入。".to_string()),
        other => return Err(format!("待确认记录状态不可用：{:?}", other)),
    }
    if &record.payload != expected {
        return Err("待写入内容与登记时的内容不一致，拒绝执行。".to_string());
    }
    Ok(record)
}

pub fn mark_resolved(dir: &Path, id: &str, status: &str, now: f64) -> Result<PendingWriteRecord, String> {
    if !matches!(status, "consumed" | "rejected") {
        return Err(format!("不允许把确认记录标记为 {:?}", status));
    }
    let mut record = load(dir, id)?;
    if record.status != "pending" {
        return Err(format!("记录当前状态为 {:?}，不能再次结算。", record.status));
    }
    record.status = status.to_string();
    record.resolved_at = Some(now);
    write_record(dir, &record)?;
    Ok(record)
}

/// 供命令层复用的强制校验：来源为人工时不应带确认号。
pub fn assert_source_matches(source: WriteSource, pending_id: &Option<String>) -> Result<(), String> {
    match source {
        WriteSource::Human => {
            if pending_id.is_some() {
                return Err("人工保存不应携带待确认编号。".to_string());
            }
            Ok(())
        }
        WriteSource::Model | WriteSource::Scheduler => {
            if pending_id.as_deref().unwrap_or_default().is_empty() {
                return Err(
                    "模型或定时任务发起的写入必须先取得用户确认（缺少待确认记录编号），已拒绝。"
                        .to_string(),
                );
            }
            Ok(())
        }
    }
}

/// 与 `save_offline_artifact` 共用的场景子目录名。
pub fn scenario_dir(scenario: &str) -> Option<&'static str> {
    match scenario {
        "weekly_report" => Some("weekly-report"),
        "material_writing" => Some("material-writing"),
        "project_management" => Some("project-management"),
        _ => None,
    }
}

/// 人工确认卡片的展示状态（不含正文，避免把正文复制到日志/清单里）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingWriteView {
    pub id: String,
    pub action: String,
    pub source: String,
    pub scenario: String,
    pub title: String,
    pub summary: String,
    pub impact: String,
    pub chars: usize,
    pub created_at: f64,
    pub expires_at: f64,
    pub status: String,
}

impl PendingWriteRecord {
    pub fn view(&self, now: f64) -> PendingWriteView {
        PendingWriteView {
            id: self.id.clone(),
            action: self.action.clone(),
            source: self.source.clone(),
            scenario: self.payload.scenario.clone(),
            title: self.payload.title.clone(),
            summary: self.summary.clone(),
            impact: self.impact.clone(),
            chars: self.payload.markdown.chars().count(),
            created_at: self.created_at,
            expires_at: self.expires_at,
            status: effective_status(self, now).to_string(),
        }
    }
}

impl SaveOfflineRequest {
    /// 由一条已通过校验的记录构造实际写请求（仅供宿主内部使用）。
    pub fn from_record(record: &PendingWriteRecord) -> Self {
        SaveOfflineRequest {
            scenario: record.payload.scenario.clone(),
            title: record.payload.title.clone(),
            markdown: record.payload.markdown.clone(),
            user_confirmed: true,
            ai_assisted: record.payload.ai_assisted,
            source: if record.source == "scheduler" {
                WriteSource::Scheduler
            } else {
                WriteSource::Model
            },
            pending_id: Some(record.id.clone()),
        }
    }
}

/// 执行入口终检（OD-ARCH-049 第 3 步）。
///
/// 任务真正触达引擎之前做一次状态校验，检查结果只有两种：
///   * 通过：任务原样执行（返回 `Ok(())`，调用方不改写任何内容）；
///   * 不通过：**整条任务拒绝**（返回指名道姓的错误），绝不“跳过部分内容继续执行”。
///
/// 拦截范围最小化：只有与任务属于**同一场景成果目录**的未决确认卡才拦截。
/// 其它场景的卡片不挡任务；过期 / 已确认 / 已放弃的卡片也不挡。
pub fn scene_gate(dir: &Path, scenario: &str, now: f64) -> Result<(), String> {
    let (records, broken) = list(dir, now)?;
    if broken > 0 {
        // 读不出来的记录无法证明“没有挡路卡片”，这里选择拦住并说清原因，
        // 而不是当作没有记录继续执行。
        return Err(format!(
            "有 {} 条待确认记录无法解析（{}），无法确认能否开始任务；请先核对并处理这些记录。",
            broken,
            dir.display()
        ));
    }
    match records
        .into_iter()
        .find(|record| record.status == "pending" && record.payload.scenario == scenario)
    {
        Some(record) => Err(format!(
            "有一条未确认写入（{}），请先处理：确认或放弃该确认卡后再开始任务。",
            record.summary
        )),
        None => Ok(()),
    }
}

/// 把任务入口的 intent 映射到离线场景；非场景任务返回 `None`（不进入拦截范围）。
///
/// 只接受三个场景标识（下划线或连字符写法）；其它 intent 一律不拦，
/// 因为提交入口无法据此判断资源归属。
pub fn scene_for_intent(intent: &str) -> Option<&'static str> {
    match intent.trim().replace('-', "_").as_str() {
        "weekly_report" => Some("weekly_report"),
        "material_writing" => Some("material_writing"),
        "project_management" => Some("project_management"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omnidesk-pending-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(markdown: &str) -> CreatePendingWrite {
        CreatePendingWrite {
            action: "save_offline".to_string(),
            source: "model".to_string(),
            summary: "保存模型复盘建议".to_string(),
            impact: "在数据根 weekly-report/ 下新建一个文件，不覆盖既有文件".to_string(),
            payload: PendingWritePayload {
                scenario: "weekly_report".to_string(),
                title: "AI 复盘建议 · 合成".to_string(),
                markdown: markdown.to_string(),
                ai_assisted: true,
            },
        }
    }

    #[test]
    fn create_and_consume_once() {
        let dir = temp_dir("once");
        let record = create(&dir, &request("合成正文"), 1_800_000_000.0).expect("应可登记");
        assert_eq!(record.status, "pending");
        assert_eq!(record.expires_at - record.created_at, PENDING_TTL_SECS);

        let validated = validate_for_write(&dir, &record.id, &record.payload, 1_800_000_010.0)
            .expect("未过期且内容一致应通过");
        assert_eq!(validated.id, record.id);
        mark_resolved(&dir, &record.id, "consumed", 1_800_000_020.0).expect("应可结算");
        let err = validate_for_write(&dir, &record.id, &record.payload, 1_800_000_030.0).unwrap_err();
        assert!(err.contains("已执行过"), "{}", err);
    }

    #[test]
    fn expired_record_is_refused() {
        let dir = temp_dir("expired");
        let record = create(&dir, &request("合成正文"), 1_800_000_000.0).unwrap();
        let err = validate_for_write(
            &dir,
            &record.id,
            &record.payload,
            1_800_000_000.0 + PENDING_TTL_SECS + 1.0,
        )
        .unwrap_err();
        assert!(err.contains("过期"), "{}", err);
        let (listed, _) = list(&dir, 1_800_000_000.0 + PENDING_TTL_SECS + 1.0).unwrap();
        assert_eq!(listed[0].status, "expired");
    }

    #[test]
    fn payload_mismatch_is_refused() {
        let dir = temp_dir("mismatch");
        let record = create(&dir, &request("登记时的正文"), 1_800_000_000.0).unwrap();
        let mut tampered = record.payload.clone();
        tampered.markdown = "前端改过的正文".to_string();
        let err = validate_for_write(&dir, &record.id, &tampered, 1_800_000_100.0).unwrap_err();
        assert!(err.contains("不一致"), "{}", err);
    }

    #[test]
    fn human_source_and_unknown_action_are_refused() {
        let dir = temp_dir("source");
        let mut human = request("合成正文");
        human.source = "human".to_string();
        assert!(create(&dir, &human, 1_800_000_000.0).is_err());

        let mut unknown = request("合成正文");
        unknown.action = "delete_everything".to_string();
        assert!(create(&dir, &unknown, 1_800_000_000.0).is_err());

        assert!(assert_source_matches(WriteSource::Model, &None).is_err());
        assert!(assert_source_matches(WriteSource::Scheduler, &Some("".to_string())).is_err());
        assert!(assert_source_matches(WriteSource::Human, &Some("pw-x".to_string())).is_err());
        assert!(assert_source_matches(WriteSource::Human, &None).is_ok());
        assert!(assert_source_matches(WriteSource::Model, &Some("pw-x".to_string())).is_ok());
    }

    #[test]
    fn reject_settles_record() {
        let dir = temp_dir("reject");
        let record = create(&dir, &request("合成正文"), 1_800_000_000.0).unwrap();
        let settled = mark_resolved(&dir, &record.id, "rejected", 1_800_000_050.0).unwrap();
        assert_eq!(settled.status, "rejected");
        assert!(mark_resolved(&dir, &record.id, "rejected", 1_800_000_060.0).is_err());
    }

    /// OD-ARCH-049 第 3 步验收一：同场景未决卡 → 整条任务拒绝，且错误指名卡片的摘要。
    #[test]
    fn scene_gate_blocks_same_scene_pending_card_with_named_error() {
        let dir = temp_dir("gate-same");
        let record = create(&dir, &request("合成正文"), 1_800_000_000.0).unwrap();
        let error = scene_gate(&dir, "weekly_report", 1_800_000_010.0)
            .expect_err("同场景有未决确认卡时必须拒绝任务");
        assert!(
            error.contains("有一条未确认写入"),
            "错误应指名未确认写入：{}",
            error
        );
        assert!(
            error.contains(&record.summary),
            "错误应带上卡片摘要以便定位：{}",
            error
        );
    }

    /// OD-ARCH-049 第 3 步验收二：其它场景的未决卡不挡本场景任务。
    #[test]
    fn scene_gate_ignores_other_scenes() {
        let dir = temp_dir("gate-other");
        create(&dir, &request("合成正文"), 1_800_000_000.0).unwrap();
        assert!(scene_gate(&dir, "material_writing", 1_800_000_010.0).is_ok());
        assert!(scene_gate(&dir, "project_management", 1_800_000_010.0).is_ok());
    }

    /// 已结算 / 已过期的卡不再挡任务；无法解析的记录则拦下并说清原因（不静默放行）。
    #[test]
    fn scene_gate_respects_settled_expired_and_broken_records() {
        let dir = temp_dir("gate-settled");
        let record = create(&dir, &request("合成正文"), 1_800_000_000.0).unwrap();
        mark_resolved(&dir, &record.id, "rejected", 1_800_000_010.0).unwrap();
        assert!(scene_gate(&dir, "weekly_report", 1_800_000_020.0).is_ok());

        // 过期：超过 24 小时有效期后不再挡任务。
        let second = create(&dir, &request("另一份正文"), 1_800_000_030.0).unwrap();
        let after_ttl = second.expires_at + 1.0;
        assert!(scene_gate(&dir, "weekly_report", after_ttl).is_ok());

        // 坏记录：无法证明没有挡路卡片，必须拒绝并给出原因。
        fs::write(dir.join("pw-broken.json"), b"{\"schema\":\"oms").unwrap();
        let error = scene_gate(&dir, "material_writing", after_ttl)
            .expect_err("有无法解析的记录时必须拒绝");
        assert!(error.contains("无法解析"), "error={}", error);
    }

    #[test]
    fn scene_for_intent_only_maps_known_scenes() {
        assert_eq!(scene_for_intent("weekly_report"), Some("weekly_report"));
        assert_eq!(scene_for_intent("weekly-report"), Some("weekly_report"));
        assert_eq!(scene_for_intent("material_writing"), Some("material_writing"));
        assert_eq!(
            scene_for_intent("project_management"),
            Some("project_management")
        );
        assert_eq!(scene_for_intent("desensitized_analysis"), None);
        assert_eq!(scene_for_intent(""), None);
    }
}
