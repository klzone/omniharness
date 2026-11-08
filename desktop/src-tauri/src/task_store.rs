//! 任务存储层（OD-WR-301）。
//!
//! 每个任务一个 Markdown 文件：<work_root>/任务/<task_id>.md。
//! 文件由 frontmatter、## 进展 与 ## 状态历史 三段组成；进展与历史只追加条目，
//! 既有条目不改写。状态迁移只接受白名单：待确认 / 待开始 / 进行中 / 已完成 /
//! 已取消 / 暂停。
//!
//! AI 任务候选先落 <work_root>/.candidates/（宿主暂存，扫描排除），确认后才生成
//! 正式任务文件。防重键为 (record_id, source_quote 哈希)，见 candidate_dedup_key。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{
    assert_record_id, assert_work_root, is_date_shape, is_safe_id, new_file_id, normalize_opt,
    split_frontmatter, write_new_atomically, write_replace_atomically, yaml_opt, yaml_scalar,
};

pub const TASK_SCHEMA: &str = "omnidesk.task.v1";
pub const TASKS_DIR: &str = "任务";
/// 候选暂存目录（work 根下，工作记录/任务扫描都不覆盖）。
pub const CANDIDATES_DIR: &str = ".candidates";
/// 候选处理完成后的归档子目录。
pub const CANDIDATES_DONE_DIR: &str = "已处理";
pub const CANDIDATE_SCHEMA: &str = "omnidesk.task-candidate.v1";

pub const STATE_PENDING_CONFIRM: &str = "待确认";
pub const STATE_NOT_STARTED: &str = "待开始";
pub const STATE_IN_PROGRESS: &str = "进行中";
pub const STATE_DONE: &str = "已完成";
pub const STATE_CANCELLED: &str = "已取消";
pub const STATE_PAUSED: &str = "暂停";
pub const TASK_STATES: &[&str] = &[
    STATE_PENDING_CONFIRM,
    STATE_NOT_STARTED,
    STATE_IN_PROGRESS,
    STATE_DONE,
    STATE_CANCELLED,
    STATE_PAUSED,
];

pub const CANDIDATE_PENDING: &str = "pending";
pub const CANDIDATE_CONFIRMED: &str = "confirmed";
pub const CANDIDATE_LINKED: &str = "linked";
pub const CANDIDATE_REJECTED: &str = "rejected";

const MAX_TITLE_CHARS: usize = 200;

/// 任务进展条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressEntry {
    pub at: String,
    pub text: String,
}

/// 状态迁移记录。note 可为空。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateChange {
    pub at: String,
    pub from: String,
    pub to: String,
    pub note: Option<String>,
}

/// 一个正式任务。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub source_record_id: Option<String>,
    pub source_quote: Option<String>,
    /// 与该任务关联过的记录（OD-WR-307 双向回写）；source_record_id 保留最初来源。
    pub linked_record_ids: Vec<String>,
    pub category: Option<String>,
    pub due_date: Option<String>,
    /// R8：本期只存不用。
    pub hint: Option<String>,
    pub progress: Vec<ProgressEntry>,
    pub history: Vec<StateChange>,
}

/// 新建任务入参。initial_state 只接受 待确认 / 待开始，缺省 待开始。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskDraft {
    pub id: Option<String>,
    pub title: String,
    pub initial_state: Option<String>,
    pub source_record_id: Option<String>,
    pub source_quote: Option<String>,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub hint: Option<String>,
}

/// 任务元数据更新补丁。None = 不修改；空字符串 = 清空。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskFilter {
    pub state: Option<String>,
    pub category: Option<String>,
    pub keyword: Option<String>,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskListing {
    pub items: Vec<Task>,
    pub broken: usize,
}

/// AI 候选入参（尚未落盘，因此没有 id / 状态 / 去重键）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CandidateInput {
    pub title: String,
    pub source_quote: String,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub note: Option<String>,
    pub confidence: Option<f64>,
    pub uncertainty: Option<String>,
    /// 模型建议关联的已有任务（307）。
    pub match_task_id: Option<String>,
    pub match_confidence: Option<f64>,
    pub match_reason: Option<String>,
    /// AI 建议的目标状态；只作建议，不自动应用。
    pub suggested_state: Option<String>,
}

/// 暂存的候选。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCandidate {
    pub schema: String,
    pub id: String,
    pub record_id: String,
    pub source_quote: String,
    pub title: String,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub note: Option<String>,
    pub confidence: Option<f64>,
    pub uncertainty: Option<String>,
    #[serde(default)]
    pub match_task_id: Option<String>,
    #[serde(default)]
    pub match_confidence: Option<f64>,
    #[serde(default)]
    pub match_reason: Option<String>,
    #[serde(default)]
    pub suggested_state: Option<String>,
    pub dedup_key: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub confirmed_task_id: Option<String>,
    pub decision_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StageReport {
    pub staged: Vec<TaskCandidate>,
    /// 因 (record_id, quote 哈希) 已存在而跳过的键。
    pub skipped_duplicates: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateListing {
    pub items: Vec<TaskCandidate>,
    pub broken: usize,
}

/// 确认候选时可修改的字段。None = 沿用候选原值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfirmEdits {
    pub title: Option<String>,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub note: Option<String>,
}

/// 采纳关联建议后的回执。写入可能半完成：ok=false 时按各布尔标志与 warnings
/// 明确告知哪一步没做成，便于用户手动补救；不做跨文件事务。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkReport {
    pub ok: bool,
    pub task: Option<Task>,
    pub record: Option<crate::worklog_records::WorkRecord>,
    pub candidate: Option<TaskCandidate>,
    pub progress_added: bool,
    pub task_linked: bool,
    pub record_linked: bool,
    pub candidate_archived: bool,
    pub partial: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

pub fn tasks_dir(root: &Path) -> PathBuf {
    root.join(TASKS_DIR)
}

pub fn trash_tasks_dir(root: &Path) -> PathBuf {
    root.join(crate::worklog_records::TRASH_DIR).join(TASKS_DIR)
}

pub fn candidates_dir(root: &Path) -> PathBuf {
    root.join(CANDIDATES_DIR)
}

fn candidates_done_dir(root: &Path) -> PathBuf {
    candidates_dir(root).join(CANDIDATES_DONE_DIR)
}

pub fn task_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) {
        return Err(format!("任务编号不合法：{id:?}"));
    }
    Ok(tasks_dir(root).join(format!("{id}.md")))
}

fn candidate_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) {
        return Err(format!("候选编号不合法：{id:?}"));
    }
    Ok(candidates_dir(root).join(format!("{id}.json")))
}

pub fn is_task_state(state: &str) -> bool {
    TASK_STATES.contains(&state)
}

/// 白名单迁移。已完成与已取消是终态，没有下一步。
pub fn allowed_next(state: &str) -> &'static [&'static str] {
    match state {
        STATE_PENDING_CONFIRM => &[STATE_NOT_STARTED, STATE_CANCELLED],
        STATE_NOT_STARTED => &[STATE_IN_PROGRESS, STATE_CANCELLED, STATE_PAUSED],
        STATE_IN_PROGRESS => &[STATE_DONE, STATE_CANCELLED, STATE_PAUSED],
        STATE_PAUSED => &[STATE_IN_PROGRESS, STATE_CANCELLED],
        _ => &[],
    }
}

pub fn is_allowed_transition(from: &str, to: &str) -> bool {
    allowed_next(from).contains(&to)
}

/// 未完成任务：待开始 / 进行中 / 暂停。待确认与终态不参与关联建议。
pub fn is_unfinished_state(state: &str) -> bool {
    matches!(state, STATE_NOT_STARTED | STATE_IN_PROGRESS | STATE_PAUSED)
}

/// 去重键：record_id + 来源片段原文的 SHA-256。
pub fn candidate_dedup_key(record_id: &str, source_quote: &str) -> String {
    format!(
        "{record_id}:{}",
        crate::sha256::sha256_hex(source_quote.trim().as_bytes())
    )
}

fn render_task(task: &Task) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(TASK_SCHEMA)));
    fm.push_str(&format!("id: {}\n", yaml_scalar(&task.id)));
    fm.push_str(&format!("title: {}\n", yaml_scalar(&task.title)));
    fm.push_str(&format!("state: {}\n", yaml_scalar(&task.state)));
    fm.push_str(&format!("created_at: {}\n", yaml_scalar(&task.created_at)));
    fm.push_str(&format!("updated_at: {}\n", yaml_scalar(&task.updated_at)));
    fm.push_str(&format!(
        "source_record_id: {}\n",
        yaml_opt(&task.source_record_id)
    ));
    fm.push_str(&format!("source_quote: {}\n", yaml_opt(&task.source_quote)));
    if task.linked_record_ids.is_empty() {
        fm.push_str("linked_record_ids: []\n");
    } else {
        fm.push_str("linked_record_ids:\n");
        for record_id in &task.linked_record_ids {
            fm.push_str(&format!("  - {}\n", yaml_scalar(record_id)));
        }
    }
    fm.push_str(&format!("category: {}\n", yaml_opt(&task.category)));
    fm.push_str(&format!("due_date: {}\n", yaml_opt(&task.due_date)));
    fm.push_str(&format!("hint: {}\n", yaml_opt(&task.hint)));
    let mut out = format!("---\n{fm}---\n\n## 进展\n\n");
    for entry in &task.progress {
        out.push_str(&format!("- {} | {}\n", entry.at, entry.text));
    }
    out.push_str("\n## 状态历史\n\n");
    for change in &task.history {
        match change.note.as_deref() {
            Some(note) => out.push_str(&format!(
                "- {} | {} → {} | {}\n",
                change.at, change.from, change.to, note
            )),
            None => out.push_str(&format!(
                "- {} | {} → {}\n",
                change.at, change.from, change.to
            )),
        }
    }
    out
}

fn parse_sections(body: &str) -> Result<(Vec<ProgressEntry>, Vec<StateChange>), String> {
    let mut progress = Vec::new();
    let mut history = Vec::new();
    let mut section = "";
    let mut seen_progress = false;
    let mut seen_history = false;
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "## 进展" {
            section = "progress";
            seen_progress = true;
            continue;
        }
        if line == "## 状态历史" {
            section = "history";
            seen_history = true;
            continue;
        }
        if line.starts_with("## ") {
            return Err(format!("任务文件出现未知分区：{line}"));
        }
        let item = match line.strip_prefix("- ") {
            Some(item) => item.trim(),
            None => return Err(format!("任务分区里有无法识别的行：{line}")),
        };
        match section {
            "progress" => {
                let (at, text) = item
                    .split_once('|')
                    .ok_or_else(|| format!("进展条目缺少分隔符：{line}"))?;
                let text = text.trim();
                if at.trim().is_empty() || text.is_empty() {
                    return Err(format!("进展条目不完整：{line}"));
                }
                progress.push(ProgressEntry {
                    at: at.trim().to_string(),
                    text: text.to_string(),
                });
            }
            "history" => {
                let mut parts = item.splitn(3, '|');
                let at = parts.next().unwrap_or("").trim().to_string();
                let arrow = parts
                    .next()
                    .ok_or_else(|| format!("状态历史缺少迁移字段：{line}"))?
                    .trim();
                let note = parts
                    .next()
                    .map(|note| note.trim().to_string())
                    .filter(|note| !note.is_empty());
                let (from, to) = arrow
                    .split_once('→')
                    .ok_or_else(|| format!("状态历史缺少箭头：{line}"))?;
                let from = from.trim();
                let to = to.trim();
                if at.is_empty() || !is_task_state(from) || !is_task_state(to) {
                    return Err(format!("状态历史条目不合法：{line}"));
                }
                history.push(StateChange {
                    at,
                    from: from.to_string(),
                    to: to.to_string(),
                    note,
                });
            }
            _ => return Err("任务内容不在任何分区里".to_string()),
        }
    }
    if !seen_progress || !seen_history {
        return Err("任务文件缺少 进展 或 状态历史 分区".to_string());
    }
    Ok((progress, history))
}

#[derive(Debug, Deserialize)]
struct TaskFront {
    #[serde(default)]
    schema: Option<String>,
    id: String,
    title: String,
    state: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    source_record_id: Option<String>,
    #[serde(default)]
    source_quote: Option<String>,
    #[serde(default)]
    linked_record_ids: Vec<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    hint: Option<String>,
}

fn parse_task(raw: &str) -> Result<Task, String> {
    let (front_raw, body) = split_frontmatter(raw)?;
    let front: TaskFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("任务 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != TASK_SCHEMA {
            return Err(format!("任务 schema 不认识：{schema}"));
        }
    }
    if !is_safe_id(&front.id) {
        return Err(format!("任务编号不合法：{:?}", front.id));
    }
    if !is_task_state(&front.state) {
        return Err(format!("任务状态不认识：{}", front.state));
    }
    if front.title.trim().is_empty() {
        return Err("任务标题为空".to_string());
    }
    let source_record_id = normalize_opt(front.source_record_id);
    if let Some(record_id) = source_record_id.as_deref() {
        assert_record_id(record_id)?;
    }
    let mut linked_record_ids: Vec<String> = Vec::new();
    for record_id in &front.linked_record_ids {
        let record_id = record_id.trim().to_string();
        if record_id.is_empty() {
            continue;
        }
        assert_record_id(&record_id)?;
        if !linked_record_ids.iter().any(|existing| existing == &record_id) {
            linked_record_ids.push(record_id);
        }
    }
    let due_date = normalize_opt(front.due_date);
    if let Some(date) = due_date.as_deref() {
        if !is_date_shape(date) {
            return Err(format!("任务截止日期形状不正确：{date}"));
        }
    }
    let (progress, history) = parse_sections(&body)?;
    Ok(Task {
        id: front.id,
        title: front.title.trim().to_string(),
        state: front.state,
        created_at: front.created_at,
        updated_at: front.updated_at,
        source_record_id,
        source_quote: normalize_opt(front.source_quote),
        linked_record_ids,
        category: normalize_opt(front.category),
        due_date,
        hint: normalize_opt(front.hint),
        progress,
        history,
    })
}

fn validate_due_date(value: &str) -> Result<(), String> {
    if !is_date_shape(value) {
        return Err(format!("截止日期形状不正确（应为 YYYY-MM-DD）：{value}"));
    }
    Ok(())
}

fn validate_source(root: &Path, record_id: &str, quote: &str) -> Result<(), String> {
    assert_record_id(record_id)?;
    let record = crate::worklog_records::read_record(root, record_id)?;
    if !record.body.contains(quote.trim()) {
        return Err(format!(
            "来源片段不在记录 {record_id} 的正文里：{}",
            quote.trim()
        ));
    }
    Ok(())
}

pub fn create_task(root: &Path, draft: &TaskDraft, now: f64) -> Result<Task, String> {
    assert_work_root(root)?;
    let title = draft.title.trim().to_string();
    if title.is_empty() {
        return Err("任务标题不能为空。".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("任务标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
    }
    let state = match normalize_opt(draft.initial_state.clone()) {
        Some(state) => {
            if state != STATE_PENDING_CONFIRM && state != STATE_NOT_STARTED {
                return Err(format!(
                    "新建任务只能从 待确认 或 待开始 开始，收到：{state}"
                ));
            }
            state
        }
        None => STATE_NOT_STARTED.to_string(),
    };
    let id = match normalize_opt(draft.id.clone()) {
        Some(id) => {
            if !is_safe_id(&id) {
                return Err(format!("任务编号不合法：{id:?}"));
            }
            id
        }
        None => new_file_id("t", now),
    };
    let source_record_id = normalize_opt(draft.source_record_id.clone());
    let source_quote = normalize_opt(draft.source_quote.clone());
    match (source_record_id.as_deref(), source_quote.as_deref()) {
        (Some(record_id), Some(quote)) => validate_source(root, record_id, quote)?,
        (Some(_), None) => {
            return Err("关联了记录的任务必须带来源片段 source_quote。".to_string())
        }
        (None, Some(_)) => {
            return Err("来源片段必须同时指定来源记录 source_record_id。".to_string())
        }
        (None, None) => {}
    }
    let due_date = normalize_opt(draft.due_date.clone());
    if let Some(date) = due_date.as_deref() {
        validate_due_date(date)?;
    }
    let created_at = crate::task_shell::utc_iso8601(now);
    let task = Task {
        id: id.clone(),
        title,
        state,
        created_at: created_at.clone(),
        updated_at: created_at,
        source_record_id,
        source_quote,
        linked_record_ids: Vec::new(),
        category: normalize_opt(draft.category.clone()),
        due_date,
        hint: normalize_opt(draft.hint.clone()),
        progress: Vec::new(),
        history: Vec::new(),
    };
    fs::create_dir_all(tasks_dir(root)).map_err(|e| format!("创建任务目录失败：{e}"))?;
    write_new_atomically(&task_path(root, &id)?, render_task(&task).as_bytes())?;
    Ok(task)
}

pub fn read_task(root: &Path, id: &str) -> Result<Task, String> {
    assert_work_root(root)?;
    let path = task_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取任务失败（{}）：{}", path.display(), e))?;
    parse_task(&raw)
}

fn matches_task_filter(filter: &TaskFilter, task: &Task) -> bool {
    if let Some(state) = filter.state.as_deref() {
        if task.state != state {
            return false;
        }
    }
    if let Some(category) = filter.category.as_deref() {
        if task.category.as_deref() != Some(category) {
            return false;
        }
    }
    if let Some(record_id) = filter.source_record_id.as_deref() {
        if task.source_record_id.as_deref() != Some(record_id) {
            return false;
        }
    }
    if let Some(keyword) = filter.keyword.as_deref() {
        let needle = keyword.trim().to_lowercase();
        if !needle.is_empty() {
            let mut haystack = format!(
                "{}\n{}\n{}",
                task.title,
                task.category.clone().unwrap_or_default(),
                task.source_quote.clone().unwrap_or_default()
            );
            for entry in &task.progress {
                haystack.push('\n');
                haystack.push_str(&entry.text);
            }
            if !haystack.to_lowercase().contains(&needle) {
                return false;
            }
        }
    }
    true
}

pub fn list_tasks(root: &Path, filter: &TaskFilter) -> Result<TaskListing, String> {
    assert_work_root(root)?;
    if let Some(state) = filter.state.as_deref() {
        if !is_task_state(state) {
            return Err(format!("任务状态不认识：{state}"));
        }
    }
    let dir = tasks_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if dir.exists() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取任务目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取任务目录项失败：{e}"))?;
            let file_type = entry
                .file_type()
                .map_err(|e| format!("读取目录项类型失败：{e}"))?;
            if !file_type.is_file() {
                continue;
            }
            let path = entry.path();
            let is_markdown = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_markdown {
                continue;
            }
            match fs::read_to_string(&path)
                .map_err(|e| format!("读取任务失败（{}）：{}", path.display(), e))
                .and_then(|raw| parse_task(&raw))
            {
                Ok(task) => {
                    if matches_task_filter(filter, &task) {
                        items.push(task);
                    }
                }
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(TaskListing { items, broken })
}

pub fn update_task(
    root: &Path,
    id: &str,
    patch: &TaskPatch,
    now: f64,
) -> Result<Task, String> {
    let mut task = read_task(root, id)?;
    if let Some(title) = patch.title.clone() {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err("任务标题不能为空。".to_string());
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!("任务标题过长（最多 {MAX_TITLE_CHARS} 字）。"));
        }
        task.title = title;
    }
    if let Some(category) = patch.category.clone() {
        task.category = normalize_opt(Some(category));
    }
    if let Some(due_date) = patch.due_date.clone() {
        let due_date = normalize_opt(Some(due_date));
        if let Some(date) = due_date.as_deref() {
            validate_due_date(date)?;
        }
        task.due_date = due_date;
    }
    if let Some(hint) = patch.hint.clone() {
        task.hint = normalize_opt(Some(hint));
    }
    task.updated_at = crate::task_shell::utc_iso8601(now);
    write_replace_atomically(&task_path(root, &task.id)?, render_task(&task).as_bytes())?;
    Ok(task)
}

/// 状态迁移：白名单外一律拒绝，且不写文件。
pub fn transition_task(
    root: &Path,
    id: &str,
    to: &str,
    note: Option<&str>,
    now: f64,
) -> Result<Task, String> {
    let mut task = read_task(root, id)?;
    if !is_task_state(to) {
        return Err(format!("任务状态不认识：{to}"));
    }
    if !is_allowed_transition(&task.state, to) {
        let next = allowed_next(&task.state);
        let next_text = if next.is_empty() {
            "无（终态）".to_string()
        } else {
            next.join(" / ")
        };
        return Err(format!(
            "非法状态迁移：{} → {}；允许的下一步：{}",
            task.state, to, next_text
        ));
    }
    let note = note
        .map(|note| note.trim().to_string())
        .filter(|note| !note.is_empty());
    task.history.push(StateChange {
        at: crate::task_shell::utc_iso8601(now),
        from: task.state.clone(),
        to: to.to_string(),
        note,
    });
    task.state = to.to_string();
    task.updated_at = crate::task_shell::utc_iso8601(now);
    write_replace_atomically(&task_path(root, &task.id)?, render_task(&task).as_bytes())?;
    Ok(task)
}

/// 追加一条进展；既有条目原样保留。
pub fn append_progress(root: &Path, id: &str, text: &str, now: f64) -> Result<Task, String> {
    let mut task = read_task(root, id)?;
    let text = text.trim();
    if text.is_empty() {
        return Err("进展内容不能为空。".to_string());
    }
    task.progress.push(ProgressEntry {
        at: crate::task_shell::utc_iso8601(now),
        text: text.to_string(),
    });
    task.updated_at = crate::task_shell::utc_iso8601(now);
    write_replace_atomically(&task_path(root, &task.id)?, render_task(&task).as_bytes())?;
    Ok(task)
}

/// 删除任务：只移动到 .trash/任务/。
pub fn delete_task(root: &Path, id: &str, now: f64) -> Result<String, String> {
    assert_work_root(root)?;
    let source = task_path(root, id)?;
    if !source.is_file() {
        return Err(format!("任务不存在，未删除：{id}"));
    }
    let trash = trash_tasks_dir(root);
    fs::create_dir_all(&trash).map_err(|e| format!("创建回收目录失败：{e}"))?;
    let stamp = now.floor() as i64;
    let mut target = trash.join(format!("{id}.{stamp}.md"));
    let mut attempt = 0u32;
    while target.exists() {
        attempt += 1;
        target = trash.join(format!("{id}.{stamp}-{attempt}.md"));
    }
    fs::rename(&source, &target).map_err(|e| {
        format!(
            "移动到回收目录失败（{} → {}）：{}",
            source.display(),
            target.display(),
            e
        )
    })?;
    Ok(target.display().to_string())
}

fn collect_dedup_keys(root: &Path) -> Result<HashSet<String>, String> {
    let mut keys = HashSet::new();
    for dir in [candidates_dir(root), candidates_done_dir(root)] {
        if !dir.exists() {
            continue;
        }
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取候选目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            if let Ok(candidate) = serde_json::from_str::<TaskCandidate>(&raw) {
                if candidate.status != CANDIDATE_REJECTED {
                    keys.insert(candidate.dedup_key);
                }
            }
        }
    }
    Ok(keys)
}

/// 暂存 AI 候选。来源片段必须是记录正文的连续片段；重复键跳过并如实回报。
pub fn stage_candidates(
    root: &Path,
    record_id: &str,
    inputs: &[CandidateInput],
    now: f64,
) -> Result<StageReport, String> {
    assert_work_root(root)?;
    assert_record_id(record_id)?;
    let record = crate::worklog_records::read_record(root, record_id)?;
    fs::create_dir_all(candidates_dir(root))
        .map_err(|e| format!("创建候选目录失败：{e}"))?;
    let mut existing = collect_dedup_keys(root)?;
    let mut report = StageReport::default();
    for (index, input) in inputs.iter().enumerate() {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err(format!("第 {} 个候选缺少标题。", index + 1));
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!("第 {} 个候选标题过长。", index + 1));
        }
        let quote = input.source_quote.trim().to_string();
        if quote.is_empty() {
            return Err(format!("第 {} 个候选缺少来源片段。", index + 1));
        }
        if !record.body.contains(&quote) {
            return Err(format!(
                "第 {} 个候选的来源片段不在记录正文里。",
                index + 1
            ));
        }
        let due_date = normalize_opt(input.due_date.clone());
        if let Some(date) = due_date.as_deref() {
            validate_due_date(date)?;
        }
        if let Some(confidence) = input.confidence {
            if !(0.0..=1.0).contains(&confidence) {
                return Err(format!(
                    "第 {} 个候选的置信度必须落在 0 到 1 之间。",
                    index + 1
                ));
            }
        }
        let match_task_id = normalize_opt(input.match_task_id.clone());
        if let Some(task_id) = match_task_id.as_deref() {
            if !is_safe_id(task_id) {
                return Err(format!("第 {} 个候选的关联任务编号不合法。", index + 1));
            }
            let task = read_task(root, task_id)?;
            if !is_unfinished_state(&task.state) {
                return Err(format!(
                    "第 {} 个候选建议关联的任务已结束或不可用。",
                    index + 1
                ));
            }
        }
        if input.match_confidence.is_some() && match_task_id.is_none() {
            return Err(format!(
                "第 {} 个候选给了关联置信度，却没有关联任务编号。",
                index + 1
            ));
        }
        if let Some(value) = input.match_confidence {
            if !(0.0..=1.0).contains(&value) {
                return Err(format!(
                    "第 {} 个候选的关联置信度必须落在 0 到 1 之间。",
                    index + 1
                ));
            }
        }
        let match_reason = normalize_opt(input.match_reason.clone());
        if let Some(reason) = match_reason.as_deref() {
            if reason.chars().count() > 200 {
                return Err(format!("第 {} 个候选的关联理由过长。", index + 1));
            }
        }
        let suggested_state = normalize_opt(input.suggested_state.clone());
        if let Some(state) = suggested_state.as_deref() {
            if !is_task_state(state) {
                return Err(format!("第 {} 个候选建议的状态不认识。", index + 1));
            }
            let task_id = match_task_id.as_deref().ok_or_else(|| {
                format!("第 {} 个候选建议了状态，却没有关联任务编号。", index + 1)
            })?;
            let task = read_task(root, task_id)?;
            if !is_allowed_transition(&task.state, state) {
                return Err(format!(
                    "第 {} 个候选建议的状态迁移不合法：{} → {}。",
                    index + 1,
                    task.state,
                    state
                ));
            }
        }
        let dedup_key = candidate_dedup_key(record_id, &quote);
        if existing.contains(&dedup_key) {
            report.skipped_duplicates.push(dedup_key);
            continue;
        }
        let id = new_file_id("tc", now);
        let candidate = TaskCandidate {
            schema: CANDIDATE_SCHEMA.to_string(),
            id: id.clone(),
            record_id: record_id.to_string(),
            source_quote: quote,
            title,
            category: normalize_opt(input.category.clone()),
            due_date,
            note: normalize_opt(input.note.clone()),
            confidence: input.confidence,
            uncertainty: normalize_opt(input.uncertainty.clone()),
            match_task_id,
            match_confidence: input.match_confidence,
            match_reason,
            suggested_state,
            dedup_key: dedup_key.clone(),
            status: CANDIDATE_PENDING.to_string(),
            created_at: crate::task_shell::utc_iso8601(now),
            decided_at: None,
            confirmed_task_id: None,
            decision_note: None,
        };
        let bytes = serde_json::to_string_pretty(&candidate)
            .map_err(|e| format!("序列化候选失败：{e}"))?;
        write_new_atomically(&candidate_path(root, &id)?, bytes.as_bytes())?;
        existing.insert(dedup_key);
        report.staged.push(candidate);
    }
    Ok(report)
}

pub fn list_candidates(root: &Path) -> Result<CandidateListing, String> {
    assert_work_root(root)?;
    let dir = candidates_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if dir.exists() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取候选目录失败（{}）：{}", dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取候选目录项失败：{e}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path)
                .map_err(|e| format!("读取候选失败：{e}"))
                .and_then(|raw| {
                    serde_json::from_str::<TaskCandidate>(&raw)
                        .map_err(|e| format!("候选不是约定 JSON：{e}"))
                })
            {
                Ok(candidate) => {
                    if candidate.status == CANDIDATE_PENDING {
                        items.push(candidate);
                    }
                }
                Err(_) => broken += 1,
            }
        }
    }
    items.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(CandidateListing { items, broken })
}

fn load_pending_candidate(root: &Path, id: &str) -> Result<TaskCandidate, String> {
    let path = candidate_path(root, id)?;
    let raw = fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("候选不存在或已处理：{id}")
        } else {
            format!("读取候选失败（{}）：{}", path.display(), e)
        }
    })?;
    let candidate: TaskCandidate = serde_json::from_str(&raw)
        .map_err(|e| format!("候选不是约定 JSON：{e}"))?;
    if candidate.status != CANDIDATE_PENDING {
        return Err(format!("候选已处理：{}", candidate.status));
    }
    Ok(candidate)
}

/// 读取一个待确认候选（IPC 用；不存在或已处理时返回可读错误）。
pub fn read_pending_candidate(root: &Path, id: &str) -> Result<TaskCandidate, String> {
    assert_work_root(root)?;
    load_pending_candidate(root, id)
}

fn write_decided_candidate(root: &Path, candidate: &TaskCandidate) -> Result<PathBuf, String> {
    let dir = candidates_done_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("创建候选归档目录失败：{e}"))?;
    let path = dir.join(format!("{}.json", candidate.id));
    let bytes =
        serde_json::to_string_pretty(candidate).map_err(|e| format!("序列化候选失败：{e}"))?;
    write_replace_atomically(&path, bytes.as_bytes())?;
    Ok(path)
}

fn task_id_from_candidate(candidate_id: &str) -> String {
    let suffix = candidate_id.strip_prefix("tc-").unwrap_or(candidate_id);
    format!("t-{suffix}")
}

/// 确认候选：生成正式任务（待开始），候选移到 已处理/。
pub fn confirm_candidate(
    root: &Path,
    candidate_id: &str,
    edits: &ConfirmEdits,
    now: f64,
) -> Result<Task, String> {
    let candidate = load_pending_candidate(root, candidate_id)?;
    let task_id = task_id_from_candidate(&candidate.id);
    if task_path(root, &task_id)?.exists() {
        return Err(format!("该候选已经生成过任务：{task_id}"));
    }
    let title = match normalize_opt(edits.title.clone()) {
        Some(title) => title,
        None => candidate.title.clone(),
    };
    let category = match &edits.category {
        Some(raw) => normalize_opt(Some(raw.clone())),
        None => candidate.category.clone(),
    };
    let due_date = match &edits.due_date {
        Some(raw) => normalize_opt(Some(raw.clone())),
        None => candidate.due_date.clone(),
    };
    let draft = TaskDraft {
        id: Some(task_id.clone()),
        title,
        initial_state: Some(STATE_NOT_STARTED.to_string()),
        source_record_id: Some(candidate.record_id.clone()),
        source_quote: Some(candidate.source_quote.clone()),
        category,
        due_date,
        hint: None,
    };
    let task = create_task(root, &draft, now)?;
    let mut decided = candidate.clone();
    decided.status = CANDIDATE_CONFIRMED.to_string();
    decided.decided_at = Some(crate::task_shell::utc_iso8601(now));
    decided.confirmed_task_id = Some(task.id.clone());
    decided.decision_note = normalize_opt(edits.note.clone());
    write_decided_candidate(root, &decided)?;
    let pending = candidate_path(root, &candidate.id)?;
    fs::remove_file(&pending).map_err(|e| format!("清理候选暂存失败：{e}"))?;
    Ok(task)
}

/// 未完成任务列表（供解析提示词注入与关联校验）。
pub fn unfinished_tasks(root: &Path) -> Result<Vec<Task>, String> {
    let listing = list_tasks(root, &TaskFilter::default())?;
    Ok(listing
        .items
        .into_iter()
        .filter(|task| is_unfinished_state(&task.state))
        .collect())
}

/// 采纳关联建议：追加进展 + 任务/记录双向 ID 回写 + 候选归档。
pub fn link_candidate(
    root: &Path,
    candidate_id: &str,
    progress_text: Option<&str>,
    note: Option<&str>,
    now: f64,
) -> Result<LinkReport, String> {
    let candidate = load_pending_candidate(root, candidate_id)?;
    let task_id = candidate
        .match_task_id
        .clone()
        .ok_or_else(|| "这条候选没有可采纳的关联建议。".to_string())?;
    let task = read_task(root, &task_id)?;
    if !is_unfinished_state(&task.state) {
        return Err("建议关联的任务已结束，不能再追加进展。".to_string());
    }
    let text = progress_text
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| candidate.title.trim().to_string());
    if text.is_empty() {
        return Err("追加进展的内容不能为空。".to_string());
    }
    let mut task = match append_progress(root, &task_id, &text, now) {
        Ok(task) => task,
        Err(message) => {
            return Err(format!("追加进展失败，未写入任何回链：{message}"));
        }
    };
    let mut report = LinkReport {
        ok: false,
        task: Some(task.clone()),
        record: None,
        candidate: None,
        progress_added: true,
        task_linked: false,
        record_linked: false,
        candidate_archived: false,
        partial: true,
        warnings: Vec::new(),
        error: None,
    };
    if !task
        .linked_record_ids
        .iter()
        .any(|id| id == &candidate.record_id)
    {
        task.linked_record_ids.push(candidate.record_id.clone());
    }
    task.updated_at = crate::task_shell::utc_iso8601(now);
    if let Err(message) =
        write_replace_atomically(&task_path(root, &task.id)?, render_task(&task).as_bytes())
    {
        report.task = Some(task);
        report.error = Some(format!("进展已追加，但任务未回链记录：{message}"));
        report
            .warnings
            .push("任务进展已写入；任务文件未记录来源记录 ID。".to_string());
        return Ok(report);
    }
    report.task = Some(task.clone());
    report.task_linked = true;

    let mut record = match crate::worklog_records::read_record(root, &candidate.record_id) {
        Ok(record) => record,
        Err(message) => {
            report.error = Some(format!(
                "进展与任务回链已完成，但记录读取失败：{message}"
            ));
            report
                .warnings
                .push("记录未回链任务 ID；请手动补救或人工核对。".to_string());
            return Ok(report);
        }
    };
    if !record.linked_task_ids.iter().any(|id| id == &task.id) {
        record.linked_task_ids.push(task.id.clone());
    }
    match crate::worklog_records::update_record(
        root,
        &candidate.record_id,
        &crate::worklog_records::RecordPatch {
            linked_task_ids: Some(record.linked_task_ids.clone()),
            ..Default::default()
        },
        now,
    ) {
        Ok(updated) => {
            report.record = Some(updated);
            report.record_linked = true;
        }
        Err(message) => {
            report.error = Some(format!(
                "进展与任务回链已完成，但记录回链失败：{message}"
            ));
            report
                .warnings
                .push("记录未回链任务 ID；请手动补救或人工核对。".to_string());
            return Ok(report);
        }
    }

    let mut decided = candidate.clone();
    decided.status = CANDIDATE_LINKED.to_string();
    decided.decided_at = Some(crate::task_shell::utc_iso8601(now));
    decided.confirmed_task_id = Some(task.id.clone());
    decided.decision_note = note
        .map(|note| note.trim().to_string())
        .filter(|note| !note.is_empty());
    let archive_result = write_decided_candidate(root, &decided).and_then(|_| {
        let pending = candidate_path(root, &candidate.id)?;
        fs::remove_file(&pending).map_err(|e| format!("清理候选暂存失败：{e}"))
    });
    report.candidate = Some(decided);
    match archive_result {
        Ok(()) => {
            report.candidate_archived = true;
            report.ok = true;
            report.partial = false;
            report.error = None;
        }
        Err(message) => {
            report.error = Some(format!("任务与记录已回链，但候选归档失败：{message}"));
            report.warnings.push(
                "候选未完全归档；不要重复采纳，请到 .candidates 手动清理。".to_string(),
            );
        }
    }
    Ok(report)
}

/// 拒绝关联建议：清空匹配字段，候选仍留在待审核（之后可采纳为新任务）。
pub fn dismiss_match(
    root: &Path,
    candidate_id: &str,
    _now: f64,
) -> Result<TaskCandidate, String> {
    let mut candidate = load_pending_candidate(root, candidate_id)?;
    if candidate.match_task_id.is_none() {
        return Err("这条候选没有关联建议可拒绝。".to_string());
    }
    candidate.match_task_id = None;
    candidate.match_confidence = None;
    candidate.match_reason = None;
    candidate.suggested_state = None;
    let bytes =
        serde_json::to_string_pretty(&candidate).map_err(|e| format!("序列化候选失败：{e}"))?;
    write_replace_atomically(&candidate_path(root, &candidate.id)?, bytes.as_bytes())?;
    Ok(candidate)
}

/// 拒绝候选：不生成任务，候选移到 已处理/。
pub fn reject_candidate(
    root: &Path,
    candidate_id: &str,
    note: Option<&str>,
    now: f64,
) -> Result<TaskCandidate, String> {
    let candidate = load_pending_candidate(root, candidate_id)?;
    let mut decided = candidate.clone();
    decided.status = CANDIDATE_REJECTED.to_string();
    decided.decided_at = Some(crate::task_shell::utc_iso8601(now));
    decided.decision_note = note
        .map(|note| note.trim().to_string())
        .filter(|note| !note.is_empty());
    write_decided_candidate(root, &decided)?;
    let pending = candidate_path(root, &candidate.id)?;
    fs::remove_file(&pending).map_err(|e| format!("清理候选暂存失败：{e}"))?;
    Ok(decided)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklog_records::{create_record, RecordDraft};

    const NOW: f64 = 1_780_000_000.0;
    const OFFSET: i32 = 480;
    const BODY: &str = "今天整理周报，明天写总结。";

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wr301-task-{tag}-{}-{}",
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

    fn seed_record(temp: &TempRoot) -> crate::worklog_records::WorkRecord {
        create_record(
            temp.root(),
            &RecordDraft {
                title: Some("工作记录".to_string()),
                body: BODY.to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap()
    }

    fn task_draft(title: &str) -> TaskDraft {
        TaskDraft {
            title: title.to_string(),
            ..Default::default()
        }
    }

    fn candidate_input(title: &str) -> CandidateInput {
        CandidateInput {
            title: title.to_string(),
            source_quote: "整理周报".to_string(),
            ..Default::default()
        }
    }

    fn candidate_with_match(title: &str, quote: &str, task_id: &str) -> CandidateInput {
        CandidateInput {
            title: title.to_string(),
            source_quote: quote.to_string(),
            match_task_id: Some(task_id.to_string()),
            match_confidence: Some(0.8),
            match_reason: Some("同一件事".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn create_task_writes_frontmatter_and_sections() {
        let temp = TempRoot::new("create");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        let raw = fs::read_to_string(task_path(temp.root(), &task.id).unwrap()).unwrap();
        assert!(raw.contains("schema: \"omnidesk.task.v1\""));
        assert!(raw.contains("## 进展"));
        assert!(raw.contains("## 状态历史"));
        assert_eq!(task.state, STATE_NOT_STARTED);
        assert!(task.progress.is_empty());
        assert!(task.history.is_empty());
    }

    #[test]
    fn create_rejects_unlisted_initial_state() {
        let temp = TempRoot::new("bad-state");
        let err = create_task(
            temp.root(),
            &TaskDraft {
                initial_state: Some(STATE_DONE.to_string()),
                ..task_draft("任务")
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("待确认 或 待开始"), "{err}");
    }

    #[test]
    fn create_rejects_empty_title() {
        let temp = TempRoot::new("empty-title");
        let err = create_task(temp.root(), &task_draft("   "), NOW).unwrap_err();
        assert!(err.contains("不能为空"), "{err}");
    }

    #[test]
    fn create_requires_source_record_and_quote_together() {
        let temp = TempRoot::new("source");
        let err = create_task(
            temp.root(),
            &TaskDraft {
                source_record_id: Some("wr-missing".to_string()),
                source_quote: Some("x".to_string()),
                ..task_draft("任务")
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("读取记录失败"), "{err}");
        let err = create_task(
            temp.root(),
            &TaskDraft {
                source_record_id: Some("wr-any".to_string()),
                ..task_draft("任务")
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("必须带来源片段"), "{err}");
        let err = create_task(
            temp.root(),
            &TaskDraft {
                source_quote: Some("片段".to_string()),
                ..task_draft("任务")
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("同时指定来源记录"), "{err}");
    }

    #[test]
    fn transition_records_history_and_state() {
        let temp = TempRoot::new("transition");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        let moved = transition_task(
            temp.root(),
            &task.id,
            STATE_IN_PROGRESS,
            Some("开始处理"),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(moved.state, STATE_IN_PROGRESS);
        assert_eq!(moved.history.len(), 1);
        assert_eq!(moved.history[0].from, STATE_NOT_STARTED);
        assert_eq!(moved.history[0].to, STATE_IN_PROGRESS);
        assert_eq!(moved.history[0].note.as_deref(), Some("开始处理"));
        let raw = fs::read_to_string(task_path(temp.root(), &task.id).unwrap()).unwrap();
        assert!(raw.contains("待开始 → 进行中"));
    }

    #[test]
    fn transition_rejects_illegal_jump_without_writing() {
        let temp = TempRoot::new("illegal");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        let path = task_path(temp.root(), &task.id).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let err = transition_task(temp.root(), &task.id, STATE_DONE, None, NOW).unwrap_err();
        assert!(err.contains("非法状态迁移"), "{err}");
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(before, after);
        assert_eq!(read_task(temp.root(), &task.id).unwrap().state, STATE_NOT_STARTED);
    }

    #[test]
    fn transition_rejects_same_state() {
        let temp = TempRoot::new("same");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        let err = transition_task(temp.root(), &task.id, STATE_NOT_STARTED, None, NOW).unwrap_err();
        assert!(err.contains("非法状态迁移"), "{err}");
    }

    #[test]
    fn completed_and_cancelled_are_terminal() {
        let temp = TempRoot::new("terminal");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, STATE_DONE, None, NOW).unwrap();
        let err =
            transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap_err();
        assert!(err.contains("终态"), "{err}");
        let other = create_task(temp.root(), &task_draft("另一个"), NOW).unwrap();
        transition_task(temp.root(), &other.id, STATE_CANCELLED, None, NOW).unwrap();
        let err = transition_task(temp.root(), &other.id, STATE_NOT_STARTED, None, NOW).unwrap_err();
        assert!(err.contains("终态"), "{err}");
    }

    #[test]
    fn append_progress_keeps_previous_entries() {
        let temp = TempRoot::new("progress");
        let task = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        append_progress(temp.root(), &task.id, "写了第一节", NOW).unwrap();
        let second = append_progress(temp.root(), &task.id, "写了第二节", NOW + 60.0).unwrap();
        assert_eq!(second.progress.len(), 2);
        assert_eq!(second.progress[0].text, "写了第一节");
        assert_eq!(second.progress[1].text, "写了第二节");
        let raw = fs::read_to_string(task_path(temp.root(), &task.id).unwrap()).unwrap();
        assert!(raw.contains("写了第一节"));
        assert!(raw.contains("写了第二节"));
    }

    #[test]
    fn list_tasks_filters_by_state_and_keyword() {
        let temp = TempRoot::new("list");
        let first = create_task(temp.root(), &task_draft("写周报"), NOW).unwrap();
        create_task(temp.root(), &task_draft("整理资料"), NOW).unwrap();
        transition_task(temp.root(), &first.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        let listing = list_tasks(
            temp.root(),
            &TaskFilter {
                state: Some(STATE_IN_PROGRESS.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, first.id);
        let listing = list_tasks(
            temp.root(),
            &TaskFilter {
                keyword: Some("资料".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].title, "整理资料");
    }

    #[test]
    fn staged_candidates_do_not_appear_in_task_list() {
        let temp = TempRoot::new("candidate-hidden");
        let record = seed_record(&temp);
        stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_input("整理周报")],
            NOW,
        )
        .unwrap();
        let listing = list_tasks(temp.root(), &TaskFilter::default()).unwrap();
        assert!(listing.items.is_empty(), "{:?}", listing.items);
        assert!(!tasks_dir(temp.root()).exists());
    }

    #[test]
    fn stage_candidates_persists_under_candidates_dir() {
        let temp = TempRoot::new("stage");
        let record = seed_record(&temp);
        let report = stage_candidates(
            temp.root(),
            &record.id,
            &[CandidateInput {
                title: "整理周报".to_string(),
                source_quote: "整理周报".to_string(),
                confidence: Some(0.8),
                uncertainty: Some("日期不明确".to_string()),
                ..Default::default()
            }],
            NOW,
        )
        .unwrap();
        assert_eq!(report.staged.len(), 1);
        let candidate = &report.staged[0];
        assert_eq!(candidate.status, CANDIDATE_PENDING);
        assert_eq!(candidate.record_id, record.id);
        assert!(candidate_path(temp.root(), &candidate.id).unwrap().exists());
        let listing = list_candidates(temp.root()).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, candidate.id);
    }

    #[test]
    fn stage_candidates_skips_duplicate_record_quote() {
        let temp = TempRoot::new("dedup");
        let record = seed_record(&temp);
        let first = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_input("整理周报")],
            NOW,
        )
        .unwrap();
        assert_eq!(first.staged.len(), 1);
        let second = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_input("整理周报")],
            NOW,
        )
        .unwrap();
        assert!(second.staged.is_empty());
        assert_eq!(second.skipped_duplicates.len(), 1);
        assert_eq!(second.skipped_duplicates[0], first.staged[0].dedup_key);
    }

    #[test]
    fn stage_rejects_quote_not_in_record() {
        let temp = TempRoot::new("quote");
        let record = seed_record(&temp);
        let err = stage_candidates(
            temp.root(),
            &record.id,
            &[CandidateInput {
                title: "编造任务".to_string(),
                source_quote: "记录里没有这句话".to_string(),
                ..Default::default()
            }],
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("不在记录正文里"), "{err}");
    }

    #[test]
    fn stage_rejects_unknown_record() {
        let temp = TempRoot::new("missing-record");
        let err = stage_candidates(
            temp.root(),
            "wr-missing",
            &[candidate_input("任务")],
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("读取记录失败"), "{err}");
    }

    #[test]
    fn confirm_candidate_creates_task_and_moves_file() {
        let temp = TempRoot::new("confirm");
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_input("整理周报")],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        let task = confirm_candidate(
            temp.root(),
            &candidate.id,
            &ConfirmEdits {
                title: Some("整理本周周报".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(task.title, "整理本周周报");
        assert_eq!(task.state, STATE_NOT_STARTED);
        assert_eq!(task.source_record_id.as_deref(), Some(record.id.as_str()));
        assert_eq!(task.source_quote.as_deref(), Some("整理周报"));
        assert!(task_path(temp.root(), &task.id).unwrap().exists());
        assert!(!candidate_path(temp.root(), &candidate.id).unwrap().exists());
        assert!(candidates_done_dir(temp.root())
            .join(format!("{}.json", candidate.id))
            .exists());
        let listing = list_tasks(temp.root(), &TaskFilter::default()).unwrap();
        assert_eq!(listing.items.len(), 1);
    }

    #[test]
    fn confirm_candidate_twice_is_rejected() {
        let temp = TempRoot::new("confirm-twice");
        let record = seed_record(&temp);
        let staged =
            stage_candidates(temp.root(), &record.id, &[candidate_input("任务")], NOW).unwrap();
        let candidate = staged.staged[0].clone();
        confirm_candidate(temp.root(), &candidate.id, &ConfirmEdits::default(), NOW).unwrap();
        let err = confirm_candidate(temp.root(), &candidate.id, &ConfirmEdits::default(), NOW)
            .unwrap_err();
        assert!(err.contains("不存在"), "{err}");
    }

    #[test]
    fn reject_candidate_creates_no_task() {
        let temp = TempRoot::new("reject");
        let record = seed_record(&temp);
        let staged =
            stage_candidates(temp.root(), &record.id, &[candidate_input("任务")], NOW).unwrap();
        let candidate = staged.staged[0].clone();
        let rejected =
            reject_candidate(temp.root(), &candidate.id, Some("不需要"), NOW).unwrap();
        assert_eq!(rejected.status, CANDIDATE_REJECTED);
        assert!(list_tasks(temp.root(), &TaskFilter::default())
            .unwrap()
            .items
            .is_empty());
        assert!(!candidate_path(temp.root(), &candidate.id).unwrap().exists());
        assert!(candidates_done_dir(temp.root())
            .join(format!("{}.json", candidate.id))
            .exists());
    }

    #[test]
    fn delete_task_moves_to_trash() {
        let temp = TempRoot::new("delete");
        let task = create_task(temp.root(), &task_draft("待删任务"), NOW).unwrap();
        let path = task_path(temp.root(), &task.id).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let moved = delete_task(temp.root(), &task.id, NOW).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(moved).unwrap(), before);
    }

    #[test]
    fn update_task_metadata_keeps_created_at() {
        let temp = TempRoot::new("update");
        let task = create_task(temp.root(), &task_draft("原标题"), NOW).unwrap();
        let updated = update_task(
            temp.root(),
            &task.id,
            &TaskPatch {
                title: Some("新标题".to_string()),
                category: Some("项目".to_string()),
                due_date: Some("2026-11-30".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.created_at, task.created_at);
        assert_ne!(updated.updated_at, task.updated_at);
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.category.as_deref(), Some("项目"));
        assert_eq!(updated.due_date.as_deref(), Some("2026-11-30"));
    }

    #[test]
    fn dedup_key_is_deterministic_and_record_scoped() {
        let a = candidate_dedup_key("wr-1", "同一句话");
        assert_eq!(a, candidate_dedup_key("wr-1", "同一句话"));
        assert_ne!(a, candidate_dedup_key("wr-2", "同一句话"));
        assert_ne!(a, candidate_dedup_key("wr-1", "另一句话"));
    }

    #[test]
    fn record_body_edit_breaks_candidate_staging() {
        let temp = TempRoot::new("edited");
        let record = seed_record(&temp);
        crate::worklog_records::update_record(
            temp.root(),
            &record.id,
            &crate::worklog_records::RecordPatch {
                body: Some("正文已改写".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let err = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_input("任务")],
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("不在记录正文里"), "{err}");
    }

    #[test]
    fn link_candidate_appends_progress_and_writebacks() {
        let temp = TempRoot::new("link");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        let linked = link_candidate(
            temp.root(),
            &candidate.id,
            Some("完成周报初稿"),
            Some("归到同一件事"),
            NOW + 60.0,
        )
        .unwrap();
        assert!(linked.ok);
        assert!(!linked.partial);
        let linked_task = linked.task.as_ref().unwrap();
        assert_eq!(linked_task.progress.len(), 1);
        assert_eq!(linked_task.progress[0].text, "完成周报初稿");
        assert!(linked_task.linked_record_ids.contains(&record.id));
        let record_after = crate::worklog_records::read_record(temp.root(), &record.id).unwrap();
        assert!(record_after.linked_task_ids.contains(&task.id));
        assert_eq!(
            linked.candidate.as_ref().unwrap().status,
            CANDIDATE_LINKED
        );
        assert!(list_candidates(temp.root()).unwrap().items.is_empty());
    }

    #[test]
    fn dismiss_match_keeps_candidate_pending() {
        let temp = TempRoot::new("dismiss");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        let cleared = dismiss_match(temp.root(), &candidate.id, NOW).unwrap();
        assert!(cleared.match_task_id.is_none());
        assert!(cleared.match_confidence.is_none());
        assert!(cleared.suggested_state.is_none());
        let pending = list_candidates(temp.root()).unwrap();
        assert_eq!(pending.items.len(), 1, "候选仍在待审核");
        assert_eq!(pending.items[0].match_task_id, None);
    }

    #[test]
    fn link_rejects_finished_task() {
        let temp = TempRoot::new("link-finished");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, STATE_DONE, None, NOW).unwrap();
        let err = link_candidate(temp.root(), &candidate.id, None, None, NOW).unwrap_err();
        assert!(err.contains("已结束"), "{err}");
    }

    #[test]
    fn stage_rejects_match_to_finished_task() {
        let temp = TempRoot::new("stage-finished");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        transition_task(temp.root(), &task.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &task.id, STATE_DONE, None, NOW).unwrap();
        let err = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("已结束或不可用"), "{err}");
    }

    #[test]
    fn unfinished_tasks_filters_terminal_states() {
        let temp = TempRoot::new("unfinished");
        let first = create_task(temp.root(), &task_draft("第一件"), NOW).unwrap();
        create_task(temp.root(), &task_draft("第二件"), NOW).unwrap();
        transition_task(temp.root(), &first.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        let third = create_task(temp.root(), &task_draft("第三件"), NOW).unwrap();
        transition_task(temp.root(), &third.id, STATE_IN_PROGRESS, None, NOW).unwrap();
        transition_task(temp.root(), &third.id, STATE_DONE, None, NOW).unwrap();
        let tasks = unfinished_tasks(temp.root()).unwrap();
        assert_eq!(tasks.len(), 2, "{:?}", tasks);
        assert!(!tasks.iter().any(|task| task.id == third.id));
    }

    #[test]
    fn three_day_records_link_one_task_without_losing_progress() {
        let temp = TempRoot::new("three-day");
        let task = create_task(temp.root(), &task_draft("推进周报"), NOW).unwrap();
        let mut records = Vec::new();
        for day in 1..=3 {
            let body = format!("第 {day} 天推进周报。");
            let record = create_record(
                temp.root(),
                &RecordDraft {
                    title: Some(format!("第 {day} 天")),
                    body: body.clone(),
                    ..Default::default()
                },
                NOW + (day as f64) * 86_400.0,
                OFFSET,
            )
            .unwrap();
            let staged = stage_candidates(
                temp.root(),
                &record.id,
                &[candidate_with_match("推进周报", "推进周报", &task.id)],
                NOW + (day as f64) * 86_400.0,
            )
            .unwrap();
            let linked = link_candidate(
                temp.root(),
                &staged.staged[0].id,
                Some(&format!("第 {day} 天推进")),
                None,
                NOW + (day as f64) * 86_400.0,
            )
            .unwrap();
            assert_eq!(linked.task.as_ref().unwrap().progress.len(), day);
            records.push(record);
        }
        let final_task = read_task(temp.root(), &task.id).unwrap();
        assert_eq!(final_task.progress.len(), 3, "三天的进展都要保留");
        assert_eq!(final_task.linked_record_ids.len(), 3);
        for (index, record) in records.iter().enumerate() {
            assert!(final_task.linked_record_ids.contains(&record.id));
            let stored = crate::worklog_records::read_record(temp.root(), &record.id).unwrap();
            assert!(stored.linked_task_ids.contains(&task.id));
            assert_eq!(final_task.progress[index].text, format!("第 {} 天推进", index + 1));
        }
    }

    #[test]
    fn link_reports_partial_when_record_writeback_fails() {
        let temp = TempRoot::new("partial-record");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        crate::worklog_records::delete_record(temp.root(), &record.id, NOW).unwrap();
        let report = link_candidate(temp.root(), &candidate.id, Some("推进"), None, NOW).unwrap();
        assert!(!report.ok);
        assert!(report.partial);
        assert!(report.progress_added);
        assert!(report.task_linked);
        assert!(!report.record_linked);
        assert!(report.record.is_none());
        assert!(report.error.is_some());
        assert!(!report.warnings.is_empty());
        let task_after = read_task(temp.root(), &task.id).unwrap();
        assert_eq!(task_after.progress.len(), 1, "进展已写入，半完成状态可见");
        assert!(list_candidates(temp.root()).unwrap().items.iter().any(|item| item.id == candidate.id));
    }

    #[test]
    fn link_reports_partial_when_candidate_archive_fails() {
        let temp = TempRoot::new("partial-archive");
        let task = create_task(temp.root(), &task_draft("整理周报"), NOW).unwrap();
        let record = seed_record(&temp);
        let staged = stage_candidates(
            temp.root(),
            &record.id,
            &[candidate_with_match("整理周报", "整理周报", &task.id)],
            NOW,
        )
        .unwrap();
        let candidate = staged.staged[0].clone();
        fs::write(candidates_done_dir(temp.root()), "占位文件").unwrap();
        let report = link_candidate(temp.root(), &candidate.id, Some("推进"), None, NOW).unwrap();
        assert!(!report.ok);
        assert!(report.partial);
        assert!(report.task_linked && report.record_linked);
        assert!(!report.candidate_archived);
        assert!(report.candidate.is_some());
        assert!(report.error.is_some());
        assert!(report.warnings.iter().any(|warning| warning.contains("不要重复采纳")));
        let task_after = read_task(temp.root(), &task.id).unwrap();
        assert_eq!(task_after.progress.len(), 1);
        let record_after = crate::worklog_records::read_record(temp.root(), &record.id).unwrap();
        assert!(record_after.linked_task_ids.contains(&task.id));
    }
}
