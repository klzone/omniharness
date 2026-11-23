//! 每日习惯整理：摘要收集（OD-GROW-301 G2）。
//!
//! 本模块只做本地确定性收集：读取线程元数据与正文、按水位筛出新增用户发言、
//! 按当前 Profile 规则做红线分流、读取项目任务当日状态变化，并拼出摘要文本。
//! 不调用模型、不触发引擎、不接触 runtime/ 或 scheduler.rs。
//!
//! 配置在 <应用数据根>/config/consolidation.v1.json；水位与摘要落在
//! <应用数据根>/habits/ 下，复用 G1 的 consolidation-state.v1 与 pending-review.v1。

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::habit_cards::{
    assert_habit_id, ConsolidationState, HabitStore, PendingReviewItem,
    CONSOLIDATION_STATE_SCHEMA, PENDING_PENDING, PENDING_REVIEW_SCHEMA,
    PENDING_REVIEW_TEXT_MAX_CHARS, QUESTION_OPEN,
};
use crate::document_store;
use crate::profile_rules::{mask_privacy, RulePolicy};
use crate::sha256::sha256_hex;
use crate::task_shell::utc_iso8601;
use crate::task_store::{self, TaskFilter};
use crate::thread_store::{ThreadMetadata, ThreadStore};
use crate::worklog::civil_from_days;
use crate::worklog_records::write_replace_atomically;
use crate::writing_diff;
use crate::writing_store::WRITING_DIR;

pub const CONSOLIDATION_SCHEMA: &str = "omnidesk.consolidation.v1";
pub const CONSOLIDATION_FILE: &str = "consolidation.v1.json";
pub const DIGEST_SCHEMA: &str = "omnidesk.digest.v1";
pub const STATE_FILE: &str = "consolidation-state.v1.json";

pub const SOURCE_PLAIN_THREADS: &str = "plain_threads";
pub const SOURCE_PROJECT_TASKS: &str = "project_tasks";
pub const DEFAULT_UTC_OFFSET_MINUTES: i32 = 480;
/// 写作差异候选来源标记；最终候选仍走 G3 校验与用户确认。
pub const WRITING_DIFF_ORIGIN: &str = "writing_diff";
/// 同一被删片段至少出现在两份不同稿件才提候选。
pub const WRITING_DIFF_MIN_DOCUMENTS: usize = 2;

/// 问题与新一轮用户发言的字符二元组重叠达到该比例即视为再次提及。
pub const QUESTION_BIGRAM_THRESHOLD: f64 = 0.34;

/// 单条用户发言的处理上限；超过即视为无法判定，转待审定。
pub const MESSAGE_MAX_CHARS: usize = 10_000;
/// 待审定预览里的原因代码长度上限（与 G1 校验保持一致）。

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationConfig {
    pub schema: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_hour")]
    pub hour: u32,
    #[serde(default = "default_minute")]
    pub minute: u32,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
    #[serde(default = "default_min_user_messages")]
    pub min_user_messages: u32,
    #[serde(default = "default_min_chars")]
    pub min_chars: u32,
    #[serde(default = "default_max_threads")]
    pub max_threads: u32,
    #[serde(default = "default_max_digest_chars")]
    pub max_digest_chars: u32,
    #[serde(default = "default_utc_offset_minutes")]
    pub utc_offset_minutes: i32,
}

fn default_hour() -> u32 {
    21
}

fn default_minute() -> u32 {
    30
}

fn default_sources() -> Vec<String> {
    vec![
        SOURCE_PLAIN_THREADS.to_string(),
        SOURCE_PROJECT_TASKS.to_string(),
    ]
}

fn default_min_user_messages() -> u32 {
    6
}

fn default_min_chars() -> u32 {
    1500
}

fn default_max_threads() -> u32 {
    20
}

fn default_max_digest_chars() -> u32 {
    24_000
}

fn default_utc_offset_minutes() -> i32 {
    DEFAULT_UTC_OFFSET_MINUTES
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            schema: CONSOLIDATION_SCHEMA.to_string(),
            enabled: false,
            hour: default_hour(),
            minute: default_minute(),
            provider: None,
            model: None,
            sources: default_sources(),
            min_user_messages: default_min_user_messages(),
            min_chars: default_min_chars(),
            max_threads: default_max_threads(),
            max_digest_chars: default_max_digest_chars(),
            utc_offset_minutes: default_utc_offset_minutes(),
        }
    }
}

impl ConsolidationConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CONSOLIDATION_SCHEMA {
            return Err(format!(
                "不支持的整理配置版本 {:?}；当前只接受 {:?}",
                self.schema, CONSOLIDATION_SCHEMA
            ));
        }
        if self.hour > 23 {
            return Err("整理触发小时必须在 0-23 之间。".to_string());
        }
        if self.minute > 59 {
            return Err("整理触发分钟必须在 0-59 之间。".to_string());
        }
        if self.min_user_messages == 0 {
            return Err("最少用户发言数必须大于 0。".to_string());
        }
        if self.min_user_messages > 1000 {
            return Err("最少用户发言数过大（上限 1000）。".to_string());
        }
        if self.max_threads == 0 || self.max_threads > 500 {
            return Err("线程上限必须在 1-500 之间。".to_string());
        }
        if self.max_digest_chars < 100 || self.max_digest_chars > 2_000_000 {
            return Err("摘要字符上限必须在 100-2000000 之间。".to_string());
        }
        if self.utc_offset_minutes < -12 * 60 || self.utc_offset_minutes > 14 * 60 {
            return Err("时区偏移超出合理范围。".to_string());
        }
        if self.sources.is_empty() {
            return Err("整理来源不能为空。".to_string());
        }
        let mut seen = BTreeSet::new();
        for source in &self.sources {
            if !matches!(source.as_str(), SOURCE_PLAIN_THREADS | SOURCE_PROJECT_TASKS) {
                return Err(format!("未知的整理来源：{:?}", source));
            }
            if !seen.insert(source.as_str()) {
                return Err(format!("整理来源重复：{:?}", source));
            }
        }
        if let Some(provider) = &self.provider {
            validate_optional_text("provider", provider)?;
        }
        if let Some(model) = &self.model {
            validate_optional_text("model", model)?;
        }
        Ok(())
    }
}

fn validate_optional_text(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} 不能是空白字符串。"));
    }
    if trimmed.chars().count() > 200 {
        return Err(format!("{label} 过长（最多 200 字符）。"));
    }
    Ok(())
}

pub fn config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONSOLIDATION_FILE)
}

pub fn load_config(config_dir: &Path) -> Result<ConsolidationConfig, String> {
    let path = config_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConsolidationConfig::default());
        }
        Err(e) => return Err(format!("读取整理配置失败（{}）：{}", path.display(), e)),
    };
    let config: ConsolidationConfig = serde_json::from_str(&text)
        .map_err(|e| format!("整理配置格式错误（{}）：{}", path.display(), e))?;
    config.validate()?;
    Ok(config)
}

pub fn write_config(config_dir: &Path, config: &ConsolidationConfig) -> Result<(), String> {
    config.validate()?;
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建配置目录失败（{}）：{}", config_dir.display(), e))?;
    let target = config_path(config_dir);
    let temp = config_dir.join(format!("{}tmp-{}", CONSOLIDATION_FILE, std::process::id()));
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化整理配置失败：{}", e))?;
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入整理配置失败：{}", e))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入整理配置失败：{}", e))?;
        fs::rename(&temp, &target).map_err(|e| format!("替换整理配置失败：{}", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct DigestPaths {
    pub threads_root: PathBuf,
    pub work_root: PathBuf,
    pub habits_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DigestMessage {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DigestThread {
    pub thread_id: String,
    pub title: String,
    pub messages: Vec<DigestMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectChange {
    pub task_id: String,
    pub title: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DigestStats {
    pub thread_count: usize,
    pub message_count: usize,
    pub char_count: usize,
    pub skipped_threads: usize,
    pub redline_masked: usize,
    pub redline_pending: usize,
    pub truncated: usize,
    pub truncated_messages: usize,
    pub truncated_project_changes: usize,
    pub project_changes: usize,
    pub writing_diff_candidates: usize,
    pub truncated_writing_diffs: usize,
}

/// 一条跨稿重复的写作差异候选来源。只存片段与出处，不存整章全文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WritingDiffHint {
    pub origin: String,
    pub writing_id: String,
    pub section_no: u32,
    pub fragment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DigestPlan {
    pub local_date: String,
    pub threads: Vec<DigestThread>,
    pub project_changes: Vec<ProjectChange>,
    pub digest_text: String,
    pub stats: DigestStats,
    #[serde(skip)]
    pub pending: Vec<PendingReviewItem>,
    #[serde(skip)]
    pub watermarks: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DigestRecord {
    pub schema: String,
    pub date: String,
    pub generated_at: String,
    pub digest_text: String,
    pub stats: DigestStats,
    pub threads: Vec<DigestThread>,
    pub project_changes: Vec<ProjectChange>,
    pub pending_written: usize,
}

/// 只读取整理状态，不创建任何目录；预演与到点判定共用。
pub fn read_state_readonly(habits_root: &Path) -> Result<ConsolidationState, String> {
    let path = habits_root.join("state").join(STATE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConsolidationState::default());
        }
        Err(e) => return Err(format!("读取整理状态失败（{}）：{}", path.display(), e)),
    };
    let state: ConsolidationState = serde_json::from_str(&text)
        .map_err(|e| format!("整理状态损坏（{}）：{}", path.display(), e))?;
    if state.schema != CONSOLIDATION_STATE_SCHEMA {
        return Err(format!(
            "整理状态 schema 必须是 {}，实际是 {:?}",
            CONSOLIDATION_STATE_SCHEMA, state.schema
        ));
    }
    for thread_id in state.watermarks.keys() {
        assert_habit_id(thread_id)?;
    }
    Ok(state)
}

/// 到点判定：与 scheduler.rs 的每日规则一致，但只读本模块自己的状态。
pub fn tick_due(config: &ConsolidationConfig, state: &ConsolidationState, now: f64) -> bool {
    if !config.enabled {
        return false;
    }
    let (days, hour, minute) = local_parts(now, config.utc_offset_minutes);
    if hour * 60 + minute < config.hour * 60 + config.minute {
        return false;
    }
    match state
        .last_run_at
        .as_deref()
        .and_then(|value| parse_iso_utc(value))
    {
        Some(last) => local_parts(last as f64, config.utc_offset_minutes).0 < days,
        None => true,
    }
}

/// 扫描 work/写作/ 下至少两版的文稿，取最近两版做逐章节差异；
/// 同一被删片段出现在至少两份不同稿件时，才作为 writing_diff 候选来源。
/// 只保留文稿编号、章节号与片段，不返回整章全文；单份删除不提出。
pub fn collect_writing_diffs(work_root: &Path) -> Result<Vec<WritingDiffHint>, String> {
    let writing_root = work_root.join(WRITING_DIR);
    let meta = match fs::symlink_metadata(&writing_root) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "读取写作目录失败（{}）：{}",
                writing_root.display(),
                error
            ))
        }
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&writing_root)
        .map_err(|error| format!("读取写作目录失败（{}）：{}", writing_root.display(), error))?;
    let mut documents: Vec<(String, Vec<writing_diff::SectionDiff>)> = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let writing_id = entry.file_name().to_string_lossy().to_string();
        let Some(diffs) = reading_document_diffs(work_root, &writing_id) else {
            continue;
        };
        if diffs.is_empty() {
            continue;
        }
        documents.push((writing_id, diffs));
    }

    let mut hits: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
    for (writing_id, diffs) in &documents {
        for diff in diffs {
            for fragment in &diff.removed {
                hits.entry(fragment.clone())
                    .or_default()
                    .push((writing_id.clone(), diff.section_no));
            }
        }
    }

    let mut result = Vec::new();
    for (fragment, places) in hits {
        let distinct_documents = places
            .iter()
            .map(|(writing_id, _)| writing_id)
            .collect::<BTreeSet<_>>();
        if distinct_documents.len() < WRITING_DIFF_MIN_DOCUMENTS {
            continue;
        }
        for (writing_id, section_no) in places {
            result.push(WritingDiffHint {
                origin: WRITING_DIFF_ORIGIN.to_string(),
                writing_id,
                section_no,
                fragment: fragment.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.writing_id
            .cmp(&b.writing_id)
            .then_with(|| a.section_no.cmp(&b.section_no))
            .then_with(|| a.fragment.cmp(&b.fragment))
    });
    Ok(result)
}

/// 读取一份文稿最近两版的逐章节差异；不足两版或读取失败时返回 None。
fn reading_document_diffs(
    work_root: &Path,
    writing_id: &str,
) -> Option<Vec<writing_diff::SectionDiff>> {
    let versions = document_store::list_versions(work_root, writing_id).ok()?;
    if versions.len() < 2 {
        return None;
    }
    let older = &versions[versions.len() - 2];
    let newer = &versions[versions.len() - 1];
    let old_text = document_store::read_version(work_root, writing_id, older.version)
        .ok()?
        .content;
    let new_text = document_store::read_version(work_root, writing_id, newer.version)
        .ok()?
        .content;
    Some(writing_diff::section_diffs(&old_text, &new_text))
}

/// 纯收集：只读线程与任务；不写待审定、不推水位、不落摘要。
pub fn collect_digest(
    paths: &DigestPaths,
    config: &ConsolidationConfig,
    policy: &RulePolicy,
    now: f64,
) -> Result<DigestPlan, String> {
    config.validate()?;
    let state = read_state_readonly(&paths.habits_root)?;
    let mut watermarks = state.watermarks.clone();
    let mut pending: Vec<PendingReviewItem> = Vec::new();
    let mut threads: Vec<DigestThread> = Vec::new();
    let mut stats = DigestStats::default();
    let local_date = local_date_string(now, config.utc_offset_minutes);

    if config
        .sources
        .iter()
        .any(|source| source == SOURCE_PLAIN_THREADS)
    {
        collect_plain_threads(
            paths,
            config,
            policy,
            now,
            &mut watermarks,
            &mut pending,
            &mut threads,
            &mut stats,
        )?;
    }

    let mut project_changes = Vec::new();
    if config
        .sources
        .iter()
        .any(|source| source == SOURCE_PROJECT_TASKS)
    {
        let listing = task_store::list_tasks(&paths.work_root, &TaskFilter::default())?;
        let today = local_date_string(now, config.utc_offset_minutes);
        for task in listing.items {
            for change in &task.history {
                if local_date_from_iso(&change.at, config.utc_offset_minutes)
                    .as_deref()
                    == Some(today.as_str())
                {
                    project_changes.push(ProjectChange {
                        task_id: task.id.clone(),
                        title: task.title.clone(),
                        from: change.from.clone(),
                        to: change.to.clone(),
                    });
                }
            }
        }
    }
    stats.project_changes = project_changes.len();

    let (kept_threads, digest_text, truncated, truncated_messages, truncated_projects) =
        render_digest(threads, &project_changes, config.max_digest_chars as usize);
    let writing_diffs = collect_writing_diffs(&paths.work_root)?;
    let (digest_text, writing_diff_candidates, truncated_writing_diffs) =
        append_writing_diff_section(digest_text, &writing_diffs, config.max_digest_chars as usize);
    stats.thread_count = kept_threads.len();
    stats.message_count = kept_threads
        .iter()
        .map(|thread| thread.messages.len())
        .sum();
    stats.char_count = digest_text.chars().count();
    stats.truncated = truncated;
    stats.truncated_messages = truncated_messages;
    stats.truncated_project_changes = truncated_projects;
    stats.writing_diff_candidates = writing_diff_candidates;
    stats.truncated_writing_diffs = truncated_writing_diffs;

    Ok(DigestPlan {
        local_date,
        threads: kept_threads,
        project_changes,
        digest_text,
        stats,
        pending,
        watermarks,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_plain_threads(
    paths: &DigestPaths,
    config: &ConsolidationConfig,
    policy: &RulePolicy,
    now: f64,
    watermarks: &mut BTreeMap<String, u64>,
    pending: &mut Vec<PendingReviewItem>,
    threads: &mut Vec<DigestThread>,
    stats: &mut DigestStats,
) -> Result<(), String> {
    let store = ThreadStore::for_root(&paths.threads_root);
    let listing = store.list_metadata()?;
    let mut plain: Vec<ThreadMetadata> = listing
        .items
        .into_iter()
        .filter(|metadata| metadata.tag == "plain")
        .collect();
    plain.sort_by(|a, b| {
        b.updated_at
            .partial_cmp(&a.updated_at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    plain.truncate(config.max_threads as usize);

    for metadata in plain {
        let previous = watermarks.get(&metadata.id).copied().unwrap_or(0);
        let start = watermarks
            .get(&metadata.id)
            .map(|value| (*value as usize).saturating_add(1))
            .unwrap_or(0);
        let Some(value) = store.read_thread(&metadata.id)? else {
            stats.skipped_threads += 1;
            continue;
        };
        let messages = value
            .get("messages")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let next_watermark = if messages.is_empty() {
            previous
        } else {
            previous.max(messages.len() as u64 - 1)
        };
        watermarks.insert(metadata.id.clone(), next_watermark);

        let mut included = Vec::new();
        let mut included_chars = 0usize;
        let mut new_user_messages = 0usize;
        for (index, message) in messages.iter().enumerate() {
            if index < start {
                continue;
            }
            let Some(object) = message.as_object() else {
                continue;
            };
            if object.get("role").and_then(|value| value.as_str()) != Some("user") {
                continue;
            }
            new_user_messages += 1;
            let message_id = object
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let text = object
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if message_id.is_empty() {
                pending.push(build_pending_item(
                    &metadata.id,
                    "",
                    index,
                    text,
                    "invalid-message",
                    policy,
                    now,
                ));
                stats.redline_pending += 1;
                continue;
            }
            if text.trim().is_empty() {
                pending.push(build_pending_item(
                    &metadata.id,
                    &message_id,
                    index,
                    text,
                    "empty-text",
                    policy,
                    now,
                ));
                stats.redline_pending += 1;
                continue;
            }
            if text.chars().count() > MESSAGE_MAX_CHARS {
                pending.push(build_pending_item(
                    &metadata.id,
                    &message_id,
                    index,
                    text,
                    "oversize",
                    policy,
                    now,
                ));
                stats.redline_pending += 1;
                continue;
            }
            let (masked, counts) = mask_privacy(text, policy);
            if contains_reject_keyword(&masked, policy) {
                pending.push(build_pending_item(
                    &metadata.id,
                    &message_id,
                    index,
                    &masked,
                    "redline",
                    policy,
                    now,
                ));
                stats.redline_pending += 1;
                continue;
            }
            if counts.any() {
                stats.redline_masked += 1;
            }
            included_chars += masked.chars().count();
            included.push(DigestMessage {
                id: message_id,
                text: masked,
            });
        }

        if included.is_empty() || new_user_messages == 0 {
            stats.skipped_threads += 1;
            continue;
        }
        let meets_count = included.len() >= config.min_user_messages as usize;
        let meets_chars = included_chars >= config.min_chars as usize;
        if !meets_count && !meets_chars {
            stats.skipped_threads += 1;
            continue;
        }
        threads.push(DigestThread {
            thread_id: metadata.id,
            title: metadata.title,
            messages: included,
        });
    }
    Ok(())
}

fn build_pending_item(
    thread_id: &str,
    message_id: &str,
    index: usize,
    text: &str,
    reason_code: &str,
    policy: &RulePolicy,
    now: f64,
) -> PendingReviewItem {
    let preview = pending_preview(text, policy);
    let id_hash = sha256_hex(format!("{thread_id}\n{message_id}\n{index}").as_bytes());
    PendingReviewItem {
        schema: PENDING_REVIEW_SCHEMA.to_string(),
        id: format!("pending-{}", &id_hash[..40]),
        text: preview,
        thread_id: thread_id.to_string(),
        message_id: message_id.to_string(),
        reason_code: Some(reason_code.to_string()),
        status: PENDING_PENDING.to_string(),
        created_at: utc_iso8601(now),
        decided_at: None,
    }
}

/// 待审定只存脱敏预览：先按规则掩码，再把红线关键词换成占位文本。
fn pending_preview(text: &str, policy: &RulePolicy) -> String {
    let (mut preview, _) = mask_privacy(text, policy);
    for keyword in policy.reject_keywords() {
        if keyword.is_empty() {
            continue;
        }
        preview = preview.replace(keyword, "〔已隐去〕");
    }
    let preview = truncate_chars(preview.trim(), PENDING_REVIEW_TEXT_MAX_CHARS);
    if preview.is_empty() {
        "〔空消息〕".to_string()
    } else {
        preview
    }
}

fn contains_reject_keyword(text: &str, policy: &RulePolicy) -> bool {
    policy
        .reject_keywords()
        .iter()
        .any(|keyword| !keyword.is_empty() && text.contains(keyword.as_str()))
}

fn render_digest(
    threads: Vec<DigestThread>,
    project_changes: &[ProjectChange],
    max_chars: usize,
) -> (Vec<DigestThread>, String, usize, usize, usize) {
    let mut output = String::new();
    let mut kept_threads = Vec::new();
    let mut truncated_threads = 0usize;
    let mut truncated_messages = 0usize;

    for thread in threads {
        let header = format!("【线程】{}\n", one_line_text(&thread.title));
        if output.chars().count() + header.chars().count() > max_chars {
            truncated_threads += 1;
            truncated_messages += thread.messages.len();
            continue;
        }
        let mut block = header;
        let mut kept = Vec::new();
        let mut rest = thread.messages.into_iter();
        while let Some(message) = rest.next() {
            let line = format!(
                "- [{}] {}\n",
                one_line_text(&message.id),
                one_line_text(&message.text)
            );
            if output.chars().count() + block.chars().count() + line.chars().count() > max_chars {
                let dropped = 1 + rest.count();
                truncated_messages += dropped;
                truncated_threads += 1;
                break;
            }
            block.push_str(&line);
            kept.push(message);
        }
        if kept.is_empty() {
            continue;
        }
        output.push_str(&block);
        kept_threads.push(DigestThread {
            thread_id: thread.thread_id,
            title: thread.title,
            messages: kept,
        });
    }

    let mut truncated_projects = 0usize;
    if !project_changes.is_empty() {
        let section_header = "【项目任务状态变化】\n";
        if output.chars().count() + section_header.chars().count() <= max_chars {
            let mut section = section_header.to_string();
            let mut dropped = 0usize;
            for change in project_changes {
                let line = format!(
                    "- {}：{} → {}\n",
                    one_line_text(&change.title),
                    one_line_text(&change.from),
                    one_line_text(&change.to)
                );
                if output.chars().count() + section.chars().count() + line.chars().count()
                    > max_chars
                {
                    dropped += 1;
                    continue;
                }
                section.push_str(&line);
            }
            truncated_projects = dropped;
            if section != section_header {
                output.push_str(&section);
            }
        } else {
            truncated_projects = project_changes.len();
        }
    }

    (
        kept_threads,
        output,
        truncated_threads,
        truncated_messages,
        truncated_projects,
    )
}

/// 把跨稿重复的写作差异追加为摘要段落；超预算的行只计数，不截断正文。
fn append_writing_diff_section(
    output: String,
    hints: &[WritingDiffHint],
    max_chars: usize,
) -> (String, usize, usize) {
    if hints.is_empty() {
        return (output, 0, 0);
    }
    let section_header = "【写作差异】\n";
    let mut section = section_header.to_string();
    let mut kept = 0usize;
    let mut dropped = 0usize;
    for hint in hints {
        let line = format!(
            "- {} 第{}章：{}\n",
            one_line_text(&hint.writing_id),
            hint.section_no,
            one_line_text(&hint.fragment)
        );
        if output.chars().count() + section.chars().count() + line.chars().count() > max_chars {
            dropped += 1;
            continue;
        }
        section.push_str(&line);
        kept += 1;
    }
    if kept == 0 {
        return (output, 0, dropped);
    }
    let mut result = output;
    result.push_str(&section);
    (result, kept, dropped)
}

/// 执行收集并落盘：待审定条目、当日摘要文件、水位与最后运行时间。
pub fn apply_digest(
    paths: &DigestPaths,
    plan: &DigestPlan,
    now: f64,
) -> Result<DigestRecord, String> {
    let store = HabitStore::for_root(&paths.habits_root);
    let existing = store.list_pending_review()?;
    let mut known_ids: BTreeSet<String> = existing.items.into_iter().map(|item| item.id).collect();
    let mut pending_written = 0usize;
    for item in &plan.pending {
        if known_ids.contains(&item.id) {
            continue;
        }
        match store.create_pending_review(item.clone()) {
            Ok(_) => {
                known_ids.insert(item.id.clone());
                pending_written += 1;
            }
            Err(error) if error.contains("目标已存在") => {
                known_ids.insert(item.id.clone());
            }
            Err(error) => return Err(error),
        }
    }

    refresh_open_questions(paths, plan, now)?;

    let state = store.consolidation_state()?;
    let record = DigestRecord {
        schema: DIGEST_SCHEMA.to_string(),
        date: plan.local_date.clone(),
        generated_at: utc_iso8601(now),
        digest_text: plan.digest_text.clone(),
        stats: plan.stats.clone(),
        threads: plan.threads.clone(),
        project_changes: plan.project_changes.clone(),
        pending_written,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| format!("序列化整理摘要失败：{error}"))?;
    let target = paths
        .habits_root
        .join("state")
        .join(format!("digest-{}.json", plan.local_date));
    write_replace_atomically(&target, &bytes)?;

    let mut state = state;
    for (thread_id, watermark) in &plan.watermarks {
        state
            .watermarks
            .entry(thread_id.clone())
            .and_modify(|value| {
                if *watermark > *value {
                    *value = *watermark;
                }
            })
            .or_insert(*watermark);
    }
    state.last_run_at = Some(utc_iso8601(now));
    state.last_error = None;
    store.save_consolidation_state(&state)?;
    Ok(record)
}

/// 记录一次失败的收集；失败也记 last_run_at，避免同一天反复自动重试。
pub fn record_collect_failure(
    paths: &DigestPaths,
    now: f64,
    error: &str,
) -> Result<(), String> {
    let store = HabitStore::for_root(&paths.habits_root);
    let mut state = store.consolidation_state()?;
    state.last_run_at = Some(utc_iso8601(now));
    state.last_error = Some(truncate_chars(error, 500));
    store.save_consolidation_state(&state)
}

fn local_parts(now: f64, offset_minutes: i32) -> (i64, u32, u32) {
    let secs = now.floor() as i64 + offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = (rem / 3_600) as u32;
    let minute = ((rem % 3_600) / 60) as u32;
    (days, hour, minute)
}

pub fn local_date_string(now: f64, offset_minutes: i32) -> String {
    let (days, _, _) = local_parts(now, offset_minutes);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn local_date_from_iso(value: &str, offset_minutes: i32) -> Option<String> {
    parse_iso_utc(value).map(|secs| local_date_string(secs as f64, offset_minutes))
}

fn parse_iso_utc(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return None;
    }
    let year: i64 = value[0..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..10].parse().ok()?;
    let hour: i64 = value[11..13].parse().ok()?;
    let minute: i64 = value[14..16].parse().ok()?;
    let second: i64 = value[17..19].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
}

fn one_line_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 问题文本的字符二元组与消息文本重合比例（分母为问题自身的去重二元组数）。
pub fn question_bigram_overlap(question_text: &str, message_text: &str) -> f64 {
    let question = char_bigrams(question_text);
    if question.is_empty() {
        return 0.0;
    }
    let message = char_bigrams(message_text);
    let shared = question.intersection(&message).count();
    shared as f64 / question.len() as f64
}

/// 字符二元组的去重集合；ASCII 大小写归一，连续空白压成一个空格。
fn char_bigrams(text: &str) -> BTreeSet<(char, char)> {
    let mut normalized = Vec::new();
    let mut started = false;
    for c in text.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_whitespace() {
            if started && normalized.last() != Some(&' ') {
                normalized.push(' ');
            }
            continue;
        }
        started = true;
        normalized.push(c);
    }
    normalized
        .windows(2)
        .map(|window| (window[0], window[1]))
        .collect()
}

/// 把新一轮摘要的用户发言与仍为 open 的问题比对，命中即刷新 last_seen。
/// 只更新既有问题文件，不新建条目；返回刷新条数。
pub fn refresh_open_questions(
    paths: &DigestPaths,
    plan: &DigestPlan,
    now: f64,
) -> Result<usize, String> {
    let store = HabitStore::for_root(&paths.habits_root);
    let questions = store.list_open_questions()?;
    let mut refreshed = 0usize;
    for question in questions.items {
        if question.status != QUESTION_OPEN {
            continue;
        }
        let mentioned = plan
            .threads
            .iter()
            .flat_map(|thread| thread.messages.iter())
            .any(|message| {
                question_bigram_overlap(&question.text, &message.text)
                    >= QUESTION_BIGRAM_THRESHOLD
            });
        if mentioned {
            store.touch_open_question_last_seen(&question.id, now)?;
            refreshed += 1;
        }
    }
    Ok(refreshed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_rules::ProfileRules;
    use crate::task_store::{transition_task, TaskDraft, STATE_DONE, STATE_IN_PROGRESS,
        STATE_NOT_STARTED};
    use serde_json::json;

    const NOW: f64 = 1_795_440_600.0;

    struct TempBase {
        path: PathBuf,
    }

    impl TempBase {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "omnidesk-consolidation-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("threads")).unwrap();
            Self { path }
        }

        fn paths(&self) -> DigestPaths {
            DigestPaths {
                threads_root: self.path.join("threads"),
                work_root: self.path.join("work"),
                habits_root: self.path.join("habits"),
            }
        }
    }

    impl Drop for TempBase {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn synthetic_policy(reject: &str) -> RulePolicy {
        let rules: ProfileRules = serde_json::from_value(json!({
            "schema": "omnidesk.profile-rules.v1",
            "profile": "synthetic-consolidation",
            "global": {
                "redline_keywords": [reject],
                "privacy": {"id_card": "mask", "phone": "mask"}
            }
        }))
        .expect("合成规则资产应可解析");
        RulePolicy::from_rules(&rules)
    }

    fn write_thread(root: &Path, id: &str, tag: &str, title: &str, updated: f64, messages: &[(&str, &str)]) {
        let dir = root.join("_unassigned");
        fs::create_dir_all(&dir).unwrap();
        let messages: Vec<serde_json::Value> = messages
            .iter()
            .enumerate()
            .map(|(index, (role, text))| {
                json!({
                    "id": format!("m-{index}"),
                    "role": role,
                    "text": text
                })
            })
            .collect();
        let value = json!({
            "id": id,
            "tag": tag,
            "title": title,
            "preset": "demo",
            "messages": messages,
            "createdAt": updated - 10.0,
            "updatedAt": updated
        });
        fs::write(dir.join(format!("{id}.json")), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn config() -> ConsolidationConfig {
        ConsolidationConfig {
            min_user_messages: 1,
            min_chars: 0,
            max_threads: 10,
            ..ConsolidationConfig::default()
        }
    }

    fn snapshot_files(root: &Path) -> Vec<(String, u64)> {
        let mut out = Vec::new();
        fn walk(root: &Path, base: &Path, out: &mut Vec<(String, u64)>) {
            let Ok(entries) = fs::read_dir(root) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else if let Ok(meta) = fs::metadata(&path) {
                    out.push((
                        path.strip_prefix(base).unwrap().display().to_string(),
                        meta.len(),
                    ));
                }
            }
        }
        walk(root, root, &mut out);
        out.sort();
        out
    }

    #[test]
    fn config_defaults_and_atomic_round_trip() {
        let temp = TempBase::new("config");
        let config_dir = temp.path.join("config");
        let loaded = load_config(&config_dir).unwrap();
        assert_eq!(loaded.schema, CONSOLIDATION_SCHEMA);
        assert!(!loaded.enabled);
        assert_eq!(loaded.hour, 21);
        assert_eq!(loaded.minute, 30);
        assert_eq!(loaded.sources, vec![SOURCE_PLAIN_THREADS.to_string(), SOURCE_PROJECT_TASKS.to_string()]);
        write_config(&config_dir, &loaded).unwrap();
        let reloaded = load_config(&config_dir).unwrap();
        assert_eq!(reloaded, loaded);
    }

    #[test]
    fn config_rejects_unknown_fields_and_bad_values() {
        let temp = TempBase::new("config-bad");
        let config_dir = temp.path.join("config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join(CONSOLIDATION_FILE),
            r#"{"schema":"omnidesk.consolidation.v1","unknown":true}"#,
        )
        .unwrap();
        let error = load_config(&config_dir).unwrap_err();
        assert!(error.contains("unknown field"));

        fs::write(
            config_dir.join(CONSOLIDATION_FILE),
            r#"{"schema":"omnidesk.consolidation.v1","hour":24}"#,
        )
        .unwrap();
        let error = load_config(&config_dir).unwrap_err();
        assert!(error.contains("0-23"));

        fs::write(config_dir.join(CONSOLIDATION_FILE), "{not json").unwrap();
        assert!(load_config(&config_dir).is_err());
    }

    #[test]
    fn watermarks_only_read_new_user_messages_on_second_run() {
        let temp = TempBase::new("watermark");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let config = config();
        write_thread(
            &paths.threads_root,
            "thread-a",
            "plain",
            "线程甲",
            NOW,
            &[("user", "第一条"), ("assistant", "回应"), ("user", "第二条")],
        );
        let first = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(first.stats.message_count, 2);
        apply_digest(&paths, &first, NOW).unwrap();

        write_thread(
            &paths.threads_root,
            "thread-a",
            "plain",
            "线程甲",
            NOW + 60.0,
            &[
                ("user", "第一条"),
                ("assistant", "回应"),
                ("user", "第二条"),
                ("user", "第三条"),
            ],
        );
        let second = collect_digest(&paths, &config, &policy, NOW + 60.0).unwrap();
        assert_eq!(second.stats.message_count, 1);
        assert!(second.digest_text.contains("第三条"));
        assert!(!second.digest_text.contains("第一条"));
        apply_digest(&paths, &second, NOW + 60.0).unwrap();
        assert_eq!(
            read_state_readonly(&paths.habits_root).unwrap().watermarks["thread-a"],
            3
        );
    }

    #[test]
    fn threshold_accepts_either_message_count_or_char_count() {
        let temp = TempBase::new("threshold");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let mut config = config();
        config.min_user_messages = 6;
        config.min_chars = 1500;
        write_thread(
            &paths.threads_root,
            "count-thread",
            "plain",
            "计数线程",
            NOW,
            &[
                ("user", "一"), ("user", "二"), ("user", "三"),
                ("user", "四"), ("user", "五"), ("user", "六"),
            ],
        );
        let long_text = "字".repeat(1600);
        write_thread(
            &paths.threads_root,
            "char-thread",
            "plain",
            "字数线程",
            NOW - 1.0,
            &[("user", long_text.as_str())],
        );
        write_thread(
            &paths.threads_root,
            "short-thread",
            "plain",
            "短线程",
            NOW - 2.0,
            &[("user", "一"), ("user", "二")],
        );
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(plan.stats.thread_count, 2);
        assert!(plan.digest_text.contains("计数线程"));
        assert!(plan.digest_text.contains("字数线程"));
        assert!(!plan.digest_text.contains("短线程"));
        assert!(plan.stats.skipped_threads >= 1);
    }

    #[test]
    fn masked_privacy_passes_without_original_number() {
        let temp = TempBase::new("mask");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let config = config();
        let raw = "我的身份证号是110101199001011234，记一下";
        write_thread(
            &paths.threads_root,
            "mask-thread",
            "plain",
            "脱敏线程",
            NOW,
            &[("user", raw)],
        );
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(plan.stats.redline_masked, 1);
        assert!(plan.digest_text.contains('*'));
        assert!(!plan.digest_text.contains("110101199001011234"));
        assert!(plan.pending.is_empty());
    }

    #[test]
    fn redline_message_goes_to_pending_without_raw_text() {
        let temp = TempBase::new("pending");
        let paths = temp.paths();
        let policy = synthetic_policy("合成敏感词");
        let config = config();
        let raw = "请帮我整理合成敏感词材料里的时间线";
        write_thread(
            &paths.threads_root,
            "pending-thread",
            "plain",
            "待审定线程",
            NOW,
            &[("user", raw)],
        );
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(plan.stats.redline_pending, 1);
        assert_eq!(plan.stats.skipped_threads, 1);
        assert!(!plan.digest_text.contains("时间线"));
        assert_eq!(plan.pending.len(), 1);
        let item = &plan.pending[0];
        assert_eq!(item.reason_code.as_deref(), Some("redline"));
        assert_eq!(item.thread_id, "pending-thread");
        assert!(!item.text.contains("合成敏感词"));
        assert!(item.text.contains("已隐去"));
    }

    #[test]
    fn apply_writes_pending_and_state_but_preview_does_not_write() {
        let temp = TempBase::new("apply");
        let paths = temp.paths();
        let policy = synthetic_policy("合成敏感词");
        let config = config();
        write_thread(
            &paths.threads_root,
            "pending-thread",
            "plain",
            "待审定线程",
            NOW,
            &[("user", "请整理合成敏感词材料")],
        );
        let before = snapshot_files(&temp.path);
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        let after = snapshot_files(&temp.path);
        assert_eq!(before, after);

        let record = apply_digest(&paths, &plan, NOW).unwrap();
        assert_eq!(record.pending_written, 1);
        let store = HabitStore::for_root(&paths.habits_root);
        let pending = store.list_pending_review().unwrap();
        assert_eq!(pending.items.len(), 1);
        let state = store.consolidation_state().unwrap();
        assert_eq!(state.watermarks.get("pending-thread"), Some(&0));
        assert!(state.last_run_at.is_some());
    }

    #[test]
    fn digest_truncation_reports_threads_and_messages() {
        let temp = TempBase::new("truncate");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let mut config = config();
        config.max_digest_chars = 200;
        write_thread(
            &paths.threads_root,
            "long-thread",
            "plain",
            "长线程",
            NOW,
            &[
                ("user", &"甲".repeat(100)),
                ("user", &"乙".repeat(100)),
                ("user", &"丙".repeat(100)),
            ],
        );
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(plan.stats.thread_count, 1);
        assert_eq!(plan.stats.message_count, 1);
        assert_eq!(plan.stats.truncated, 1);
        assert_eq!(plan.stats.truncated_messages, 2);
        assert!(plan.digest_text.chars().count() <= 200);
    }

    #[test]
    fn tick_respects_enabled_time_and_daily_once() {
        let mut config = config();
        config.enabled = false;
        let state = ConsolidationState::default();
        assert!(!tick_due(&config, &state, NOW));

        config.enabled = true;
        assert!(!tick_due(&config, &state, NOW - 3_600.0));
        assert!(tick_due(&config, &state, NOW));

        let mut ran_today = ConsolidationState::default();
        ran_today.last_run_at = Some(utc_iso8601(NOW));
        assert!(!tick_due(&config, &ran_today, NOW + 1_800.0));

        let mut ran_yesterday = ConsolidationState::default();
        ran_yesterday.last_run_at = Some(utc_iso8601(NOW - 86_400.0));
        assert!(tick_due(&config, &ran_yesterday, NOW));
    }

    #[test]
    fn project_task_changes_only_include_today() {
        let temp = TempBase::new("projects");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let config = config();
        let draft = TaskDraft {
            id: Some("t-today".to_string()),
            title: "今日推进".to_string(),
            initial_state: Some(STATE_NOT_STARTED.to_string()),
            ..TaskDraft::default()
        };
        task_store::create_task(&paths.work_root, &draft, NOW).unwrap();
        transition_task(&paths.work_root, "t-today", STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(&paths.work_root, "t-today", STATE_DONE, None, NOW + 60.0).unwrap();

        let old_draft = TaskDraft {
            id: Some("t-old".to_string()),
            title: "昨日推进".to_string(),
            initial_state: Some(STATE_NOT_STARTED.to_string()),
            ..TaskDraft::default()
        };
        task_store::create_task(&paths.work_root, &old_draft, NOW - 86_400.0).unwrap();
        transition_task(
            &paths.work_root,
            "t-old",
            STATE_IN_PROGRESS,
            None,
            NOW - 86_400.0,
        )
        .unwrap();

        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert!(plan.digest_text.contains("今日推进"));
        assert!(plan.digest_text.contains("待开始 → 进行中"));
        assert!(plan.digest_text.contains("进行中 → 已完成"));
        assert!(!plan.digest_text.contains("昨日推进"));
        assert_eq!(plan.stats.project_changes, 2);
    }

    fn synthetic_open_question(
        id: &str,
        text: &str,
        status: &str,
        last_seen: f64,
    ) -> crate::habit_cards::OpenQuestion {
        crate::habit_cards::OpenQuestion {
            schema: crate::habit_cards::OPEN_QUESTION_SCHEMA.to_string(),
            id: id.to_string(),
            text: text.to_string(),
            thread_id: "question-thread".to_string(),
            message_id: "m-0".to_string(),
            first_seen: utc_iso8601(NOW - 30.0 * 86_400.0),
            last_seen: utc_iso8601(last_seen),
            status: status.to_string(),
            reminded: 0,
        }
    }

    fn digest_plan_with_message(text: &str) -> DigestPlan {
        DigestPlan {
            local_date: "2026-11-23".to_string(),
            threads: vec![DigestThread {
                thread_id: "question-thread".to_string(),
                title: "合成问题线程".to_string(),
                messages: vec![DigestMessage {
                    id: "m-0".to_string(),
                    text: text.to_string(),
                }],
            }],
            project_changes: Vec::new(),
            digest_text: text.to_string(),
            stats: DigestStats::default(),
            pending: Vec::new(),
            watermarks: BTreeMap::new(),
        }
    }

    #[test]
    fn question_bigram_overlap_fixes_threshold_boundary() {
        let question: String = (0..101u32)
            .map(|index| char::from_u32(0x4e00 + index).expect("合成汉字"))
            .collect();
        let at_threshold: String = question.chars().take(35).collect();
        let below: String = question.chars().take(34).collect();

        let overlap = question_bigram_overlap(&question, &at_threshold);
        assert!((overlap - 0.34).abs() < 1e-12, "实际重叠 {overlap}");
        assert!(overlap >= QUESTION_BIGRAM_THRESHOLD);
        assert!(question_bigram_overlap(&question, &below) < QUESTION_BIGRAM_THRESHOLD);
        assert_eq!(question_bigram_overlap(&question, "zzzz"), 0.0);
        assert_eq!(question_bigram_overlap("", "任意消息"), 0.0);
    }

    #[test]
    fn refresh_touches_only_open_questions_and_never_duplicates() {
        let temp = TempBase::new("question-refresh");
        let paths = temp.paths();
        let store = HabitStore::for_root(&paths.habits_root);
        store
            .create_open_question(synthetic_open_question(
                "q-open",
                "要不要保留这一章的内容",
                crate::habit_cards::QUESTION_OPEN,
                NOW - 3.0 * 86_400.0,
            ))
            .unwrap();
        store
            .create_open_question(synthetic_open_question(
                "q-closed",
                "要不要保留这一章的内容",
                crate::habit_cards::QUESTION_CLOSED,
                NOW - 3.0 * 86_400.0,
            ))
            .unwrap();

        let plan = digest_plan_with_message("要不要保留这一章的内容");
        assert_eq!(refresh_open_questions(&paths, &plan, NOW).unwrap(), 1);
        let first = store.list_open_questions().unwrap();
        assert_eq!(first.items.len(), 2);
        let open = first.items.iter().find(|item| item.id == "q-open").unwrap();
        assert_eq!(open.last_seen, utc_iso8601(NOW));
        let closed = first.items.iter().find(|item| item.id == "q-closed").unwrap();
        assert_eq!(closed.last_seen, utc_iso8601(NOW - 3.0 * 86_400.0));

        assert_eq!(
            refresh_open_questions(&paths, &plan, NOW + 60.0).unwrap(),
            1
        );
        let second = store.list_open_questions().unwrap();
        assert_eq!(second.items.len(), 2);
        let open = second.items.iter().find(|item| item.id == "q-open").unwrap();
        assert_eq!(open.last_seen, utc_iso8601(NOW + 60.0));

        let unrelated = digest_plan_with_message("zzzz 与问题无关");
        assert_eq!(
            refresh_open_questions(&paths, &unrelated, NOW + 120.0).unwrap(),
            0
        );
        let open = store.list_open_questions().unwrap();
        let item = open.items.iter().find(|item| item.id == "q-open").unwrap();
        assert_eq!(item.last_seen, utc_iso8601(NOW + 60.0));
    }

    #[test]
    fn apply_digest_refreshes_open_question_without_new_item() {
        let temp = TempBase::new("question-apply");
        let paths = temp.paths();
        let policy = synthetic_policy("红线词");
        let config = config();
        write_thread(
            &paths.threads_root,
            "question-thread",
            "plain",
            "合成问题线程",
            NOW,
            &[("user", "要不要保留这一章的内容")],
        );
        let store = HabitStore::for_root(&paths.habits_root);
        store
            .create_open_question(synthetic_open_question(
                "q-apply",
                "要不要保留这一章的内容",
                crate::habit_cards::QUESTION_OPEN,
                NOW - 2.0 * 86_400.0,
            ))
            .unwrap();

        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        apply_digest(&paths, &plan, NOW).unwrap();
        let questions = store.list_open_questions().unwrap();
        assert_eq!(questions.items.len(), 1);
        assert_eq!(questions.items[0].last_seen, utc_iso8601(NOW));

        apply_digest(&paths, &plan, NOW + 60.0).unwrap();
        let questions = store.list_open_questions().unwrap();
        assert_eq!(questions.items.len(), 1);
        assert_eq!(questions.items[0].last_seen, utc_iso8601(NOW + 60.0));
    }

    fn save_synthetic_body(work_root: &Path, writing_id: &str, label: &str, body: &str, now: f64) {
        let spec = document_store::SnapshotSpec {
            scope: document_store::VersionScope::Full,
            source: document_store::VersionSource::User,
            label,
            note: "合成版本",
        };
        document_store::save_with_snapshot(work_root, writing_id, &spec, body, now).unwrap();
    }

    #[test]
    fn writing_diffs_require_same_removed_fragment_in_two_documents() {
        let temp = TempBase::new("writing-diff");
        let paths = temp.paths();
        let shared_fragment = "被删的合成短语。";
        let older = format!("# 合成文稿\n\n## 1. 合成章节\n\n保留的合成句子。{shared_fragment}");
        let newer = "# 合成文稿\n\n## 1. 合成章节\n\n保留的合成句子。";
        for writing_id in ["w-g8-a", "w-g8-b"] {
            save_synthetic_body(&paths.work_root, writing_id, "v1", &older, NOW);
            save_synthetic_body(&paths.work_root, writing_id, "v2", newer, NOW + 60.0);
        }
        let single_older = "# 合成文稿\n\n## 1. 合成章节\n\n保留的合成句子。只有本稿删除的句子。";
        save_synthetic_body(&paths.work_root, "w-g8-c", "v1", single_older, NOW);
        save_synthetic_body(&paths.work_root, "w-g8-c", "v2", newer, NOW + 60.0);
        save_synthetic_body(&paths.work_root, "w-g8-one", "v1", &older, NOW);

        let hints = collect_writing_diffs(&paths.work_root).unwrap();
        assert_eq!(hints.len(), 2, "{hints:?}");
        assert!(hints.iter().all(|hint| hint.origin == WRITING_DIFF_ORIGIN));
        assert!(hints.iter().all(|hint| hint.fragment == shared_fragment));
        assert!(hints.iter().any(|hint| hint.writing_id == "w-g8-a"));
        assert!(hints.iter().any(|hint| hint.writing_id == "w-g8-b"));
        assert!(hints.iter().all(|hint| hint.writing_id != "w-g8-c"));
        assert!(hints.iter().all(|hint| hint.writing_id != "w-g8-one"));

        let policy = synthetic_policy("红线词");
        let config = config();
        let plan = collect_digest(&paths, &config, &policy, NOW).unwrap();
        assert_eq!(plan.stats.writing_diff_candidates, 2);
        assert_eq!(plan.stats.truncated_writing_diffs, 0);
        assert!(plan.digest_text.contains("【写作差异】"), "{}", plan.digest_text);
        assert!(plan.digest_text.contains(shared_fragment), "{}", plan.digest_text);
    }
}
