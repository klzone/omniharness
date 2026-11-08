//! 知识关联候选暂存与 frontmatter 写回（OD-KM-507）。
//!
//! 本模块把 505 的只读推荐结果落成候选（work/知识关联/.candidates/，照任务候选
//! 的 pending → 已处理 先例），再在用户确认后写记录/任务 frontmatter 的
//! related_knowledge 字段。知识库只读：这里只调用 505 的检索器，不写 vault。
//!
//! 写回只替换 frontmatter 中的 related_knowledge 块，frontmatter 其余行与正文
//! 原样保留；移除时若原文件没有该字段，不写盘、不新增行。移除留痕写在
//! work/知识关联/移除记录.jsonl，不改记录/任务文件。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::knowledge_link;
use crate::material_store;
use crate::task_store::{self, KnowledgeLink, Task};
use crate::worklog_records::{self, WorkRecord};

/// work 根下的知识关联目录。
pub const LINK_DIR: &str = "知识关联";
/// 候选暂存子目录（照任务候选 .candidates 先例）。
pub const CANDIDATES_DIR: &str = ".candidates";
/// 候选处理完成后的归档子目录。
pub const DONE_DIR: &str = "已处理";
pub const CANDIDATE_SCHEMA: &str = "omnidesk.knowledge-link-candidate.v1";
pub const CANDIDATE_PENDING: &str = "pending";
pub const CANDIDATE_CONFIRMED: &str = "confirmed";
/// 移除留痕文件名（位于 work/知识关联/）。
pub const REMOVE_TRACE_FILE: &str = "移除记录.jsonl";

/// 与 504 的 knowledge_hints 约定一致。
pub const MAX_HINTS: usize = 8;
pub const MAX_HINT_CHARS: usize = 24;

/// 关联对象只有记录与任务两类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkSubjectKind {
    #[serde(rename = "record")]
    Record,
    #[serde(rename = "task")]
    Task,
}

impl LinkSubjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkSubjectKind::Record => "record",
            LinkSubjectKind::Task => "task",
        }
    }
}

/// 一条暂存的知识关联候选。knowledge_id 是知识库内相对路径，可直接写成
/// 503 KnowledgeLink.id。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeLinkCandidate {
    pub schema: String,
    pub id: String,
    pub subject_kind: LinkSubjectKind,
    pub subject_id: String,
    pub subject_title: String,
    pub knowledge_id: String,
    pub title: String,
    pub reason: knowledge_link::LinkReason,
    pub reason_text: String,
    pub path: String,
    pub line: usize,
    pub dedup_key: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct KnowledgeLinkStageReport {
    pub subject_kind: String,
    pub subject_id: String,
    pub staged: Vec<KnowledgeLinkCandidate>,
    /// 已关联、已有待处理候选或没有新推荐时跳过的键。
    pub skipped_duplicates: Vec<String>,
}

/// 确认/移除回执。写入可能半完成：ok=false 时按 partial 与 warnings 说明
/// 哪一步没做成，不做跨文件事务。
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeLinkReport {
    pub ok: bool,
    pub partial: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub subject_kind: LinkSubjectKind,
    pub subject_id: String,
    pub link: Option<KnowledgeLink>,
    pub record: Option<WorkRecord>,
    pub task: Option<Task>,
    pub candidate: Option<KnowledgeLinkCandidate>,
    pub candidate_archived: bool,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeReference {
    /// record / task / material。
    pub source_kind: String,
    pub source_id: String,
    pub source_title: String,
    pub reason: Option<String>,
    pub linked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeLinkReferences {
    pub knowledge_id: String,
    pub records: Vec<KnowledgeReference>,
    pub tasks: Vec<KnowledgeReference>,
    pub materials: Vec<KnowledgeReference>,
    pub broken_records: usize,
    pub broken_tasks: usize,
    pub broken_materials: usize,
}

enum SubjectState {
    Record(WorkRecord),
    Task(Task),
}

impl SubjectState {
    fn title(&self) -> &str {
        match self {
            SubjectState::Record(record) => &record.title,
            SubjectState::Task(task) => &task.title,
        }
    }

    fn links(&self) -> &[KnowledgeLink] {
        match self {
            SubjectState::Record(record) => &record.related_knowledge,
            SubjectState::Task(task) => &task.related_knowledge,
        }
    }

    fn path(&self, root: &Path, id: &str) -> Result<PathBuf, String> {
        match self {
            SubjectState::Record(_) => worklog_records::record_path(root, id),
            SubjectState::Task(_) => task_store::task_path(root, id),
        }
    }
}

pub fn link_dir(root: &Path) -> PathBuf {
    root.join(LINK_DIR)
}

pub fn candidates_dir(root: &Path) -> PathBuf {
    link_dir(root).join(CANDIDATES_DIR)
}

fn done_dir(root: &Path) -> PathBuf {
    candidates_dir(root).join(DONE_DIR)
}

fn trace_path(root: &Path) -> PathBuf {
    link_dir(root).join(REMOVE_TRACE_FILE)
}

fn candidate_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !worklog_records::is_safe_id(id) {
        return Err(format!("知识关联候选编号不合法：{id:?}"));
    }
    Ok(candidates_dir(root).join(format!("{id}.json")))
}

/// 归一化 504 的 knowledge_hints：去空白、去重、上限 8 条、每条 24 字。
pub fn normalize_hints(hints: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for hint in hints {
        let trimmed = hint.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_HINT_CHARS {
            return Err(format!(
                "知识主题提示过长（最多 {MAX_HINT_CHARS} 字）：{trimmed:?}"
            ));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(format!("知识主题提示含控制字符：{trimmed:?}"));
        }
        if !out.iter().any(|existing| existing == &trimmed) {
            out.push(trimmed);
        }
    }
    if out.len() > MAX_HINTS {
        return Err(format!("知识主题提示过多（最多 {MAX_HINTS} 条）。"));
    }
    Ok(out)
}

fn read_subject(
    root: &Path,
    kind: LinkSubjectKind,
    subject_id: &str,
) -> Result<SubjectState, String> {
    match kind {
        LinkSubjectKind::Record => {
            worklog_records::read_record(root, subject_id).map(SubjectState::Record)
        }
        LinkSubjectKind::Task => {
            task_store::read_task(root, subject_id).map(SubjectState::Task)
        }
    }
}

fn dedup_key(kind: LinkSubjectKind, subject_id: &str, knowledge_id: &str) -> String {
    format!("{}:{}:{}", kind.as_str(), subject_id, knowledge_id)
}

fn read_candidate(root: &Path, id: &str) -> Result<KnowledgeLinkCandidate, String> {
    let path = candidate_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取知识关联候选失败（{}）：{}", path.display(), e))?;
    serde_json::from_str(&raw).map_err(|e| format!("知识关联候选格式错误：{e}"))
}

fn load_pending_candidate(root: &Path, id: &str) -> Result<KnowledgeLinkCandidate, String> {
    let candidate = read_candidate(root, id)?;
    if candidate.status != CANDIDATE_PENDING {
        return Err(format!("候选已处理：{}", candidate.status));
    }
    Ok(candidate)
}

fn pending_dedup_keys(root: &Path) -> std::collections::HashSet<String> {
    let mut keys = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(candidates_dir(root)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(candidate) = serde_json::from_str::<KnowledgeLinkCandidate>(&raw) {
                    if candidate.status == CANDIDATE_PENDING {
                        keys.insert(candidate.dedup_key);
                    }
                }
            }
        }
    }
    keys
}

fn write_decided_candidate(
    root: &Path,
    candidate: &KnowledgeLinkCandidate,
) -> Result<PathBuf, String> {
    let dir = done_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("创建知识关联候选归档目录失败：{e}"))?;
    let path = dir.join(format!("{}.json", candidate.id));
    let bytes = serde_json::to_string_pretty(candidate)
        .map_err(|e| format!("序列化知识关联候选失败：{e}"))?;
    worklog_records::write_replace_atomically(&path, bytes.as_bytes())?;
    Ok(path)
}

/// 对记录或任务生成候选并暂存。已关联的条目不会再出现；同对象同知识条目的
/// 待处理候选只保留一条。
pub fn suggest_and_stage(
    root: &Path,
    vault_root: &Path,
    kind: LinkSubjectKind,
    subject_id: &str,
    knowledge_hints: &[String],
    now: f64,
) -> Result<KnowledgeLinkStageReport, String> {
    worklog_records::assert_work_root(root)?;
    let subject = read_subject(root, kind, subject_id)?;
    let hints = normalize_hints(knowledge_hints)?;
    let candidates = match &subject {
        SubjectState::Record(record) => {
            knowledge_link::suggest_links_for_record(record, &hints, vault_root)?
        }
        SubjectState::Task(task) => {
            knowledge_link::suggest_links_for_task(task, &hints, vault_root)?
        }
    };

    let linked_ids: Vec<&str> = subject.links().iter().map(|link| link.id.as_str()).collect();
    let mut pending = pending_dedup_keys(root);
    let mut report = KnowledgeLinkStageReport {
        subject_kind: kind.as_str().to_string(),
        subject_id: subject_id.to_string(),
        ..Default::default()
    };

    for candidate in candidates {
        let key = dedup_key(kind, subject_id, &candidate.knowledge_id);
        if linked_ids.iter().any(|id| id == &candidate.knowledge_id) {
            report.skipped_duplicates.push(format!("{key}:already-linked"));
            continue;
        }
        if pending.contains(&key) {
            report.skipped_duplicates.push(format!("{key}:pending"));
            continue;
        }
        let staged = KnowledgeLinkCandidate {
            schema: CANDIDATE_SCHEMA.to_string(),
            id: worklog_records::new_file_id("kl", now),
            subject_kind: kind,
            subject_id: subject_id.to_string(),
            subject_title: subject.title().to_string(),
            knowledge_id: candidate.knowledge_id,
            title: candidate.title,
            reason: candidate.reason,
            reason_text: candidate.reason_text,
            path: candidate.path,
            line: candidate.line,
            dedup_key: key.clone(),
            status: CANDIDATE_PENDING.to_string(),
            created_at: crate::task_shell::utc_iso8601(now),
            decided_at: None,
            decision_note: None,
        };
        let bytes = serde_json::to_string_pretty(&staged)
            .map_err(|e| format!("序列化知识关联候选失败：{e}"))?;
        fs::create_dir_all(candidates_dir(root))
            .map_err(|e| format!("创建知识关联候选目录失败：{e}"))?;
        worklog_records::write_new_atomically(
            &candidate_path(root, &staged.id)?,
            bytes.as_bytes(),
        )?;
        pending.insert(key);
        report.staged.push(staged);
    }
    Ok(report)
}

fn archive_candidate(
    root: &Path,
    candidate: &KnowledgeLinkCandidate,
    reason: &str,
    now: f64,
) -> Result<KnowledgeLinkCandidate, String> {
    let mut decided = candidate.clone();
    decided.status = CANDIDATE_CONFIRMED.to_string();
    decided.decided_at = Some(crate::task_shell::utc_iso8601(now));
    decided.decision_note = Some(reason.trim().to_string());
    write_decided_candidate(root, &decided)?;
    let pending = candidate_path(root, &candidate.id)?;
    fs::remove_file(&pending).map_err(|e| format!("清理知识关联候选暂存失败：{e}"))?;
    Ok(decided)
}

/// 用户确认候选：把知识条目写进记录/任务 related_knowledge，候选移到已处理。
pub fn confirm_candidate(
    root: &Path,
    candidate_id: &str,
    reason: &str,
    now: f64,
) -> Result<KnowledgeLinkReport, String> {
    worklog_records::assert_work_root(root)?;
    let reason = reason.trim().to_string();
    task_store::validate_knowledge_reason(&reason)?;
    let candidate = load_pending_candidate(root, candidate_id)?;
    task_store::validate_knowledge_id(&candidate.knowledge_id)?;
    let subject = read_subject(root, candidate.subject_kind, &candidate.subject_id)?;
    if subject
        .links()
        .iter()
        .any(|link| link.id == candidate.knowledge_id)
    {
        return Err(format!(
            "知识条目已经关联到该{}：{}",
            if candidate.subject_kind == LinkSubjectKind::Record {
                "记录"
            } else {
                "任务"
            },
            candidate.knowledge_id
        ));
    }
    let link = KnowledgeLink {
        id: candidate.knowledge_id.clone(),
        reason: reason.clone(),
        linked_at: crate::task_shell::utc_iso8601(now),
    };
    let mut links = subject.links().to_vec();
    links.push(link.clone());
    let links = task_store::normalize_knowledge_links(&links)?;
    let path = subject.path(root, &candidate.subject_id)?;
    write_related_knowledge(&path, &links)?;

    let mut report = KnowledgeLinkReport {
        ok: false,
        partial: true,
        warnings: Vec::new(),
        error: None,
        subject_kind: candidate.subject_kind,
        subject_id: candidate.subject_id.clone(),
        link: Some(link),
        record: None,
        task: None,
        candidate: None,
        candidate_archived: false,
        removed: false,
    };
    match read_subject(root, candidate.subject_kind, &candidate.subject_id) {
        Ok(updated) => match updated {
            SubjectState::Record(record) => report.record = Some(record),
            SubjectState::Task(task) => report.task = Some(task),
        },
        Err(message) => {
            report
                .warnings
                .push(format!("关联已写入，但重新读取对象失败：{message}"));
        }
    }

    match archive_candidate(root, &candidate, &reason, now) {
        Ok(decided) => {
            report.candidate = Some(decided);
            report.candidate_archived = true;
            report.ok = true;
            report.partial = false;
            report.error = None;
        }
        Err(message) => {
            report.candidate = Some(candidate);
            report.error = Some(format!("关联已写入，但候选归档失败：{message}"));
            report.warnings.push(
                "候选未完全归档；不要重复确认，请到 .candidates 手动清理。".to_string(),
            );
        }
    }
    Ok(report)
}

#[derive(Serialize)]
struct RemoveTrace<'a> {
    at: &'a str,
    action: &'a str,
    subject_kind: LinkSubjectKind,
    subject_id: &'a str,
    knowledge_id: &'a str,
    removed: bool,
}

fn append_remove_trace(
    root: &Path,
    kind: LinkSubjectKind,
    subject_id: &str,
    knowledge_id: &str,
    removed: bool,
    now: f64,
) -> Result<(), String> {
    let dir = link_dir(root);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建知识关联目录失败（{}）：{e}", dir.display()))?;
    let trace = RemoveTrace {
        at: &crate::task_shell::utc_iso8601(now),
        action: "remove",
        subject_kind: kind,
        subject_id,
        knowledge_id,
        removed,
    };
    let mut line = serde_json::to_string(&trace)
        .map_err(|e| format!("序列化移除留痕失败：{e}"))?;
    line.push('\n');
    let path = trace_path(root);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开移除留痕失败（{}）：{}", path.display(), e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入移除留痕失败（{}）：{}", path.display(), e))
}

/// 移除记录/任务的一条知识关联。原文件没有该字段时不写盘；移除动作另写
/// work/知识关联/移除记录.jsonl 留痕。
pub fn remove_link(
    root: &Path,
    kind: LinkSubjectKind,
    subject_id: &str,
    knowledge_id: &str,
    now: f64,
) -> Result<KnowledgeLinkReport, String> {
    worklog_records::assert_work_root(root)?;
    task_store::validate_knowledge_id(knowledge_id)?;
    let subject = read_subject(root, kind, subject_id)?;
    let mut links = subject.links().to_vec();
    let removed_link = links
        .iter()
        .find(|link| link.id == knowledge_id)
        .cloned();
    links.retain(|link| link.id != knowledge_id);
    let removed = removed_link.is_some();
    let path = subject.path(root, subject_id)?;
    if removed {
        let links = task_store::normalize_knowledge_links(&links)?;
        write_related_knowledge(&path, &links)?;
    }

    let trace_result = append_remove_trace(root, kind, subject_id, knowledge_id, removed, now);
    let mut report = KnowledgeLinkReport {
        ok: trace_result.is_ok(),
        partial: trace_result.is_err(),
        warnings: Vec::new(),
        error: trace_result.as_ref().err().cloned(),
        subject_kind: kind,
        subject_id: subject_id.to_string(),
        link: removed_link,
        record: None,
        task: None,
        candidate: None,
        candidate_archived: false,
        removed,
    };
    if let Err(message) = trace_result {
        report
            .warnings
            .push(format!("关联移除结果已生效，但留痕写入失败：{message}"));
    }
    match read_subject(root, kind, subject_id) {
        Ok(updated) => match updated {
            SubjectState::Record(record) => report.record = Some(record),
            SubjectState::Task(task) => report.task = Some(task),
        },
        Err(message) => {
            report
                .warnings
                .push(format!("移除后重新读取对象失败：{message}"));
        }
    }
    Ok(report)
}

/// 反向查询：扫描 work/ 下的记录、任务、素材，返回引用该知识条目的清单。
/// 全部是读取时派生，不落盘。
pub fn references_for_knowledge(
    root: &Path,
    knowledge_id: &str,
) -> Result<KnowledgeLinkReferences, String> {
    worklog_records::assert_work_root(root)?;
    task_store::validate_knowledge_id(knowledge_id)?;
    let records = worklog_records::list_records(root, &worklog_records::RecordFilter::default())?;
    let tasks = task_store::list_tasks(root, &task_store::TaskFilter::default())?;
    let materials = material_store::list_materials(root, &material_store::MaterialFilter::default())?;

    let mut record_refs = Vec::new();
    for record in &records.items {
        if let Some(link) = record
            .related_knowledge
            .iter()
            .find(|link| link.id == knowledge_id)
        {
            record_refs.push(KnowledgeReference {
                source_kind: "record".to_string(),
                source_id: record.id.clone(),
                source_title: record.title.clone(),
                reason: Some(link.reason.clone()),
                linked_at: Some(link.linked_at.clone()),
            });
        }
    }

    let mut task_refs = Vec::new();
    for task in &tasks.items {
        if let Some(link) = task
            .related_knowledge
            .iter()
            .find(|link| link.id == knowledge_id)
        {
            task_refs.push(KnowledgeReference {
                source_kind: "task".to_string(),
                source_id: task.id.clone(),
                source_title: task.title.clone(),
                reason: Some(link.reason.clone()),
                linked_at: Some(link.linked_at.clone()),
            });
        }
    }

    let mut material_refs = Vec::new();
    for material in &materials.items {
        if material
            .related_knowledge_ids
            .iter()
            .any(|id| id == knowledge_id)
        {
            material_refs.push(KnowledgeReference {
                source_kind: "material".to_string(),
                source_id: material.material_id.clone(),
                source_title: material.title.clone(),
                reason: None,
                linked_at: None,
            });
        }
    }

    Ok(KnowledgeLinkReferences {
        knowledge_id: knowledge_id.to_string(),
        records: record_refs,
        tasks: task_refs,
        materials: material_refs,
        broken_records: records.broken,
        broken_tasks: tasks.broken,
        broken_materials: materials.broken,
    })
}

/// 只替换 frontmatter 的 related_knowledge 块；links 为空时删除该块。
/// 返回是否真的改写了文件。
pub fn write_related_knowledge(path: &Path, links: &[KnowledgeLink]) -> Result<bool, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取关联对象失败（{}）：{}", path.display(), e))?;
    let (next, changed) = rewrite_related_knowledge(&raw, links)?;
    if changed {
        worklog_records::write_replace_atomically(path, next.as_bytes())?;
    }
    Ok(changed)
}

fn frontmatter_bounds(raw: &str) -> Result<(usize, usize), String> {
    let start = if raw.starts_with('\u{feff}') { 3 } else { 0 };
    let rest = &raw[start..];
    let mut lines = rest.split_inclusive('\n');
    let first = lines.next().ok_or_else(|| "文件为空".to_string())?;
    let first_text = first.trim_end_matches(|c: char| c == '\n' || c == '\r');
    if first_text != "---" {
        return Err("缺少 frontmatter 起始围栏".to_string());
    }
    let inner_start = start + first.len();
    let mut offset = inner_start;
    for line in lines {
        let text = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
        if text == "---" {
            return Ok((inner_start, offset));
        }
        offset += line.len();
    }
    Err("frontmatter 没有闭合围栏".to_string())
}

fn strip_related_knowledge_block(inner: &str) -> (String, bool) {
    let mut out = String::with_capacity(inner.len());
    let mut in_block = false;
    let mut found = false;
    for line in inner.split_inclusive('\n') {
        let body = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
        if in_block {
            if body.starts_with(' ') || body.starts_with('\t') {
                continue;
            }
            in_block = false;
            out.push_str(line);
            continue;
        }
        if body == "related_knowledge:" {
            in_block = true;
            found = true;
            continue;
        }
        out.push_str(line);
    }
    (out, found)
}

fn render_related_knowledge_block(links: &[KnowledgeLink]) -> String {
    let mut out = String::from("related_knowledge:\n");
    for link in links {
        out.push_str(&format!(
            "  - id: {}\n    reason: {}\n    linked_at: {}\n",
            worklog_records::yaml_scalar(&link.id),
            worklog_records::yaml_scalar(&link.reason),
            worklog_records::yaml_scalar(&link.linked_at),
        ));
    }
    out
}

fn rewrite_related_knowledge(
    raw: &str,
    links: &[KnowledgeLink],
) -> Result<(String, bool), String> {
    let links = task_store::normalize_knowledge_links(links)?;
    let (inner_start, closing_start) = frontmatter_bounds(raw)?;
    let inner = &raw[inner_start..closing_start];
    let (mut stripped, found) = strip_related_knowledge_block(inner);
    if links.is_empty() && !found {
        return Ok((raw.to_string(), false));
    }
    if !links.is_empty() {
        if !stripped.ends_with('\n') {
            stripped.push('\n');
        }
        stripped.push_str(&render_related_knowledge_block(&links));
    }
    let next = format!(
        "{}{}{}",
        &raw[..inner_start],
        stripped,
        &raw[closing_start..]
    );
    Ok((next, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::{KnowledgeLink, TaskDraft};
    use crate::worklog_records::RecordDraft;
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_100_000.0;
    const OFFSET: i32 = 480;

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        vault: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-knowledge-link-store-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("work");
            let vault = base.join("vault");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(vault.join("notes")).unwrap();
            Self { base, root, vault }
        }

        fn note(&self, relative: &str, title: &str, body: &str) {
            let path = self.vault.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(
                path,
                format!("---\ntitle: {title}\n---\n\n{body}\n"),
            )
            .unwrap();
        }

        fn record(&self, title: &str) -> WorkRecord {
            worklog_records::create_record(
                &self.root,
                &RecordDraft {
                    title: Some(title.to_string()),
                    body: "虚构工作事实。".to_string(),
                    ..Default::default()
                },
                NOW,
                OFFSET,
            )
            .unwrap()
        }

        fn task(&self, title: &str) -> Task {
            task_store::create_task(
                &self.root,
                &TaskDraft {
                    title: title.to_string(),
                    ..Default::default()
                },
                NOW,
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn seed_knowledge(fx: &Fixture) {
        fx.note("notes/alpha.md", "Alpha 项目复盘", "Alpha 复盘提到了关键决策。");
        fx.note("notes/topic.md", "知识管理方法", "知识管理主题与分析建议一致。");
    }

    fn stage_one(fx: &Fixture, record: &WorkRecord, hint: &str) -> KnowledgeLinkCandidate {
        let report = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Record,
            &record.id,
            &[hint.to_string()],
            NOW,
        )
        .unwrap();
        assert_eq!(report.staged.len(), 1, "{report:?}");
        report.staged[0].clone()
    }

    #[test]
    fn suggest_stages_candidate_under_knowledge_link_candidates() {
        let fx = Fixture::new("stage");
        seed_knowledge(&fx);
        let record = fx.record("普通记录");
        let report = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Record,
            &record.id,
            &["Alpha".to_string()],
            NOW,
        )
        .unwrap();
        assert_eq!(report.staged.len(), 1);
        assert_eq!(report.staged[0].knowledge_id, "notes/alpha.md");
        assert_eq!(report.staged[0].status, CANDIDATE_PENDING);
        let path = candidate_path(&fx.root, &report.staged[0].id).unwrap();
        assert!(path.exists(), "候选必须落在 .candidates");
        assert!(path.starts_with(candidates_dir(&fx.root)));
    }

    #[test]
    fn suggest_skips_already_linked_and_pending_duplicates() {
        let fx = Fixture::new("dedupe");
        seed_knowledge(&fx);
        let task = fx.task("Alpha 任务");
        let task_path = task_store::task_path(&fx.root, &task.id).unwrap();
        write_related_knowledge(
            &task_path,
            &[KnowledgeLink {
                id: "notes/alpha.md".to_string(),
                reason: "虚构关联依据".to_string(),
                linked_at: "2026-02-02T08:15:00Z".to_string(),
            }],
        )
        .unwrap();
        let report = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Task,
            &task.id,
            &["Alpha".to_string()],
            NOW,
        )
        .unwrap();
        assert!(report.staged.is_empty(), "{report:?}");
        assert!(report.skipped_duplicates.iter().any(|key| key.ends_with(":already-linked")));

        let report = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Task,
            &task.id,
            &["知识管理".to_string()],
            NOW,
        )
        .unwrap();
        assert_eq!(report.staged.len(), 1);
        let again = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Task,
            &task.id,
            &["知识管理".to_string()],
            NOW,
        )
        .unwrap();
        assert!(again.staged.is_empty());
        assert!(again.skipped_duplicates.iter().any(|key| key.ends_with(":pending")));
    }

    #[test]
    fn hints_follow_material_candidate_shape() {
        assert_eq!(
            normalize_hints(&[" Alpha ".to_string(), "Alpha".to_string()]).unwrap(),
            vec!["Alpha"]
        );
        let too_many: Vec<String> = (0..=MAX_HINTS).map(|i| format!("h{i}")).collect();
        assert!(normalize_hints(&too_many).is_err());
        let too_long = vec!["字".repeat(MAX_HINT_CHARS + 1)];
        assert!(normalize_hints(&too_long).is_err());
        assert!(normalize_hints(&["带\n换行".to_string()]).is_err());
    }

    #[test]
    fn confirm_writes_record_link_and_archives_candidate() {
        let fx = Fixture::new("confirm");
        seed_knowledge(&fx);
        let record = fx.record("普通记录");
        let candidate = stage_one(&fx, &record, "Alpha");
        let report = confirm_candidate(&fx.root, &candidate.id, "用户确认的关联依据", NOW).unwrap();
        assert!(report.ok && !report.partial, "{report:?}");
        assert!(report.candidate_archived);
        let updated = worklog_records::read_record(&fx.root, &record.id).unwrap();
        assert_eq!(updated.related_knowledge.len(), 1);
        assert_eq!(updated.related_knowledge[0].id, "notes/alpha.md");
        assert_eq!(updated.related_knowledge[0].reason, "用户确认的关联依据");
        assert_eq!(
            updated.related_knowledge[0].linked_at,
            crate::task_shell::utc_iso8601(NOW)
        );
        assert!(!candidate_path(&fx.root, &candidate.id).unwrap().exists());
        assert!(done_dir(&fx.root)
            .join(format!("{}.json", candidate.id))
            .exists());
    }

    #[test]
    fn confirm_writes_task_link_and_keeps_task_sections() {
        let fx = Fixture::new("confirm-task");
        seed_knowledge(&fx);
        let task = fx.task("Alpha 任务");
        let staged = suggest_and_stage(
            &fx.root,
            &fx.vault,
            LinkSubjectKind::Task,
            &task.id,
            &["Alpha".to_string()],
            NOW,
        )
        .unwrap();
        assert_eq!(staged.staged.len(), 1);
        let candidate = staged.staged[0].clone();
        let report = confirm_candidate(&fx.root, &candidate.id, "用户确认的关联依据", NOW).unwrap();
        assert!(report.ok && report.task.is_some(), "{report:?}");
        let updated = task_store::read_task(&fx.root, &task.id).unwrap();
        assert_eq!(updated.related_knowledge.len(), 1);
        assert_eq!(updated.related_knowledge[0].id, "notes/alpha.md");
        let raw = fs::read_to_string(task_store::task_path(&fx.root, &task.id).unwrap()).unwrap();
        assert!(raw.contains("## 进展"), "{raw}");
        assert!(raw.contains("## 状态历史"), "{raw}");
    }

    #[test]
    fn confirm_requires_gated_user_reason() {
        let fx = Fixture::new("reason");
        seed_knowledge(&fx);
        let record = fx.record("普通记录");
        let candidate = stage_one(&fx, &record, "Alpha");
        let err = confirm_candidate(&fx.root, &candidate.id, "   ", NOW).unwrap_err();
        assert!(err.contains("理由"), "{err}");
        assert!(!worklog_records::read_record(&fx.root, &record.id)
            .unwrap()
            .related_knowledge
            .iter()
            .any(|link| link.id == "notes/alpha.md"));
    }

    #[test]
    fn confirm_returns_partial_when_archive_fails() {
        let fx = Fixture::new("partial");
        seed_knowledge(&fx);
        let record = fx.record("普通记录");
        let candidate = stage_one(&fx, &record, "Alpha");
        fs::write(done_dir(&fx.root), "not a directory").unwrap();
        let report = confirm_candidate(&fx.root, &candidate.id, "用户确认的关联依据", NOW).unwrap();
        assert!(!report.ok && report.partial, "{report:?}");
        assert!(report.error.is_some());
        assert!(!report.warnings.is_empty());
        assert!(worklog_records::read_record(&fx.root, &record.id)
            .unwrap()
            .related_knowledge
            .iter()
            .any(|link| link.id == "notes/alpha.md"));
    }

    #[test]
    fn remove_existing_link_removes_block_and_writes_trace() {
        let fx = Fixture::new("remove");
        let record = fx.record("普通记录");
        let path = worklog_records::record_path(&fx.root, &record.id).unwrap();
        write_related_knowledge(
            &path,
            &[KnowledgeLink {
                id: "notes/alpha.md".to_string(),
                reason: "虚构关联依据".to_string(),
                linked_at: "2026-02-02T08:15:00Z".to_string(),
            }],
        )
        .unwrap();
        let report =
            remove_link(&fx.root, LinkSubjectKind::Record, &record.id, "notes/alpha.md", NOW)
                .unwrap();
        assert!(report.ok && !report.partial, "{report:?}");
        assert!(report.removed);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("related_knowledge:"), "{raw}");
        let updated = worklog_records::read_record(&fx.root, &record.id).unwrap();
        assert!(updated.related_knowledge.is_empty());
        let trace = fs::read_to_string(trace_path(&fx.root)).unwrap();
        assert!(trace.contains("notes/alpha.md"));
        assert!(trace.contains("\"removed\":true"));
    }

    #[test]
    fn remove_absent_link_leaves_file_bytes_unchanged() {
        let fx = Fixture::new("noop");
        let record = fx.record("普通记录");
        let path = worklog_records::record_path(&fx.root, &record.id).unwrap();
        let before = fs::read(&path).unwrap();
        let report =
            remove_link(&fx.root, LinkSubjectKind::Record, &record.id, "notes/none.md", NOW)
                .unwrap();
        assert!(report.ok && !report.removed);
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "旧文件无该字段时不得新增行"
        );
        assert!(!fs::read_to_string(&path).unwrap().contains("related_knowledge"));
        let trace = fs::read_to_string(trace_path(&fx.root)).unwrap();
        assert!(trace.contains("\"removed\":false"));
    }

    #[test]
    fn reverse_query_finds_record_task_and_material_references() {
        let fx = Fixture::new("reverse");
        let record = fx.record("引用记录");
        let task = fx.task("引用任务");
        let record_path = worklog_records::record_path(&fx.root, &record.id).unwrap();
        let task_path = task_store::task_path(&fx.root, &task.id).unwrap();
        let link = KnowledgeLink {
            id: "notes/synthetic-knowledge.md".to_string(),
            reason: "虚构关联依据".to_string(),
            linked_at: "2026-02-02T08:15:00Z".to_string(),
        };
        write_related_knowledge(&record_path, std::slice::from_ref(&link)).unwrap();
        write_related_knowledge(&task_path, std::slice::from_ref(&link)).unwrap();
        material_store::create_material(
            &fx.root,
            &material_store::MaterialDraft {
                title: "引用素材".to_string(),
                content: "虚构素材正文。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_USER.to_string(),
                source_quote: "虚构来源引用。".to_string(),
                related_knowledge_ids: vec!["notes/synthetic-knowledge.md".to_string()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let refs = references_for_knowledge(&fx.root, "notes/synthetic-knowledge.md").unwrap();
        assert_eq!(refs.records.len(), 1);
        assert_eq!(refs.tasks.len(), 1);
        assert_eq!(refs.materials.len(), 1);
        assert_eq!(refs.records[0].source_kind, "record");
        assert_eq!(refs.tasks[0].source_kind, "task");
        assert_eq!(refs.materials[0].source_kind, "material");
        assert_eq!(refs.records[0].reason.as_deref(), Some("虚构关联依据"));
    }

    #[test]
    fn writing_link_preserves_body_and_unrelated_frontmatter_bytes() {
        let fx = Fixture::new("bytes");
        let record = fx.record("普通记录");
        let path = worklog_records::record_path(&fx.root, &record.id).unwrap();
        write_related_knowledge(
            &path,
            &[KnowledgeLink {
                id: "notes/alpha.md".to_string(),
                reason: "虚构关联依据".to_string(),
                linked_at: "2026-02-02T08:15:00Z".to_string(),
            }],
        )
        .unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("related_knowledge:"));
        assert!(after.contains("虚构工作事实。"));
        assert!(after.starts_with("---\n"));
        assert!(after.contains("title: \"普通记录\""));
    }
}
