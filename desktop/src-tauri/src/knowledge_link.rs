//! 知识推荐检索器（OD-KM-505）。
//!
//! 输入一条工作记录或任务、504 提供的主题提示词（knowledge_hints）与知识库根，
//! 输出仅供 507 确认流使用的候选列表。本模块不落盘、不注册 IPC、不写知识库，
//! 只调用 vault_search 的只读词法检索与 knowledge_vault 的只读扫描接口。
//!
//! 依据来源固定为三类，优先级从高到低：
//!   * 实体关联：记录标签 / 分类 / 项目编号与知识条目的标签、标题或别名一致，
//!     或知识条目的标题 / 别名在记录原文中出现；
//!   * AI 主题建议：504 给出的主题提示词在知识条目中命中；
//!   * 记录关键词检索：记录标题、分类、标签与正文短语在知识条目中命中。
//!
//! 同一知识条目按 knowledge_id 去重，保留优先级最高的依据；候选按依据优先级
//! 排序，最多返回 MAX_LINK_CANDIDATES 条。每条候选都带知识库内的相对路径与
//! 原文行号，行号取词法命中的正文行，或 Frontmatter / 路径命中时定位到的原始行。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::knowledge_vault::read_vault_note;
use crate::task_store::Task;
use crate::vault_note::{normalize_tag, parse_note, ParsedNote, PropertyValue};
use crate::vault_search::{search_vault, SearchHit, SearchQuery};
use crate::worklog_records::WorkRecord;

/// 单次推荐最多返回的候选数。依据：一次确认流最多呈现 8 条，避免候选清单
/// 压过用户判断，同时给实体、AI 主题与关键词检索留出可见名额。
pub const MAX_LINK_CANDIDATES: usize = 8;

/// 单次推荐最多执行的词法查询数。每个词独立查询，保持依据归属可解释；
/// 上限避免一条长记录把扫描次数放大到不可控。
pub const MAX_LINK_QUERY_TERMS: usize = 24;

const MIN_ENTITY_NAME_CHARS: usize = 2;
const MAX_ENTITY_NAME_CHARS: usize = 64;
const MAX_REASON_CHARS: usize = 100;
const MAX_PHRASE_CHARS: usize = 32;

/// 候选的依据来源。序列化后直接使用界面上的中文标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkReason {
    #[serde(rename = "实体关联")]
    Entity,
    #[serde(rename = "AI 主题建议")]
    AiTopic,
    #[serde(rename = "记录关键词检索")]
    RecordKeyword,
}

impl LinkReason {
    pub fn label(self) -> &'static str {
        match self {
            LinkReason::Entity => "实体关联",
            LinkReason::AiTopic => "AI 主题建议",
            LinkReason::RecordKeyword => "记录关键词检索",
        }
    }

    fn rank(self) -> u8 {
        match self {
            LinkReason::Entity => 0,
            LinkReason::AiTopic => 1,
            LinkReason::RecordKeyword => 2,
        }
    }
}

/// 从记录或任务抽取出的检索素材。纯数据，不含磁盘句柄。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkSource {
    pub title: String,
    pub body: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub project_id: Option<String>,
}

impl LinkSource {
    pub fn from_record(record: &WorkRecord) -> Self {
        Self {
            title: record.title.clone(),
            body: record.body.clone(),
            category: record.category.clone(),
            tags: record.tags.clone(),
            project_id: None,
        }
    }

    pub fn from_task(task: &Task) -> Self {
        let mut body = String::new();
        if let Some(quote) = task.source_quote.as_deref() {
            body.push_str(quote);
        }
        if let Some(reason) = task.importance_reason.as_deref() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(reason);
        }
        Self {
            title: task.title.clone(),
            body,
            category: task.category.clone(),
            tags: Vec::new(),
            project_id: task.project_id.clone(),
        }
    }

    fn searchable_text(&self) -> String {
        let mut parts = vec![self.title.clone(), self.body.clone()];
        if let Some(category) = self.category.as_deref() {
            parts.push(category.to_string());
        }
        if !self.tags.is_empty() {
            parts.push(self.tags.join(" "));
        }
        if let Some(project) = self.project_id.as_deref() {
            parts.push(project.to_string());
        }
        parts.join("\n")
    }

    /// 结构化实体值：标签、分类与项目编号。标题不参与实体相等判断，
    /// 避免把普通标题整句当成实体名。
    fn entity_values(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |value: String| {
            let value = value.trim().trim_start_matches('#').trim().to_string();
            if !value.is_empty()
                && !out
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&value))
            {
                out.push(value);
            }
        };
        for tag in &self.tags {
            push(normalize_tag(tag));
        }
        if let Some(category) = self.category.as_deref() {
            push(category.to_string());
        }
        if let Some(project) = self.project_id.as_deref() {
            push(project.to_string());
        }
        out
    }
}

/// suggest_links 的输入对象：工作记录或任务。
#[derive(Debug, Clone, Copy)]
pub enum LinkSubject<'a> {
    Record(&'a WorkRecord),
    Task(&'a Task),
}

impl LinkSubject<'_> {
    pub fn source(self) -> LinkSource {
        match self {
            LinkSubject::Record(record) => LinkSource::from_record(record),
            LinkSubject::Task(task) => LinkSource::from_task(task),
        }
    }
}

/// 一条知识推荐候选。knowledge_id 为知识库内相对路径，可直接写入 503 的
/// KnowledgeLink.id；line 为 1 起的原始文件行号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkCandidate {
    pub knowledge_id: String,
    pub title: String,
    pub reason: LinkReason,
    pub reason_text: String,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
struct QueryTerm {
    text: String,
    /// 词本身的来源；实体相等判断优先于这个标签。
    origin: LinkReason,
}

#[derive(Debug, Clone)]
struct NoteMeta {
    title: String,
    aliases: Vec<String>,
    tags: Vec<String>,
    raw: String,
}

/// 对记录或任务生成知识候选。只读扫描与词法检索，不写任何文件。
///
/// 错误仅在知识库根不可扫描时返回；单篇读取失败会退化为文件名标题与第 1 行，
/// 不把可检索命中静默丢掉。
pub fn suggest_links(
    subject: LinkSubject<'_>,
    knowledge_hints: &[String],
    vault_root: &Path,
) -> Result<Vec<LinkCandidate>, String> {
    let source = subject.source();
    let terms = build_terms(&source, knowledge_hints);
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let mut candidates: Vec<LinkCandidate> = Vec::new();
    let mut metas: HashMap<String, NoteMeta> = HashMap::new();

    for term in &terms {
        let response = search_vault(
            vault_root,
            &SearchQuery {
                keyword: term.text.clone(),
                ..Default::default()
            },
        )?;
        for hit in response.hits {
            let meta = load_meta(vault_root, &hit, &mut metas);
            let candidate = candidate_from_hit(&source, term, &hit, &meta);
            insert_candidate(&mut candidates, candidate);
        }
    }

    candidates.sort_by(|left, right| {
        left.reason
            .rank()
            .cmp(&right.reason.rank())
            .then_with(|| left.knowledge_id.cmp(&right.knowledge_id))
    });
    candidates.truncate(MAX_LINK_CANDIDATES);
    Ok(candidates)
}

pub fn suggest_links_for_record(
    record: &WorkRecord,
    knowledge_hints: &[String],
    vault_root: &Path,
) -> Result<Vec<LinkCandidate>, String> {
    suggest_links(LinkSubject::Record(record), knowledge_hints, vault_root)
}

pub fn suggest_links_for_task(
    task: &Task,
    knowledge_hints: &[String],
    vault_root: &Path,
) -> Result<Vec<LinkCandidate>, String> {
    suggest_links(LinkSubject::Task(task), knowledge_hints, vault_root)
}

fn build_terms(source: &LinkSource, knowledge_hints: &[String]) -> Vec<QueryTerm> {
    let mut out: Vec<QueryTerm> = Vec::new();
    let mut push = |text: String, origin: LinkReason| {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let normalized = normalize_for_match(&text);
        if normalized.is_empty() {
            return;
        }
        if let Some(existing) = out
            .iter_mut()
            .find(|term| normalize_for_match(&term.text) == normalized)
        {
            // 同一词既来自 AI 主题又来自记录关键词时，保留更高的可解释来源。
            if origin.rank() < existing.origin.rank() {
                existing.origin = origin;
            }
            return;
        }
        if out.len() < MAX_LINK_QUERY_TERMS {
            out.push(QueryTerm { text, origin });
        }
    };

    // 504 的主题建议优先进入查询，确保后续关键词上限不会挤掉 AI 依据。
    for hint in knowledge_hints {
        push(hint.to_string(), LinkReason::AiTopic);
    }
    for entity in source.entity_values() {
        push(entity, LinkReason::RecordKeyword);
    }
    push(source.title.clone(), LinkReason::RecordKeyword);
    if let Some(category) = source.category.as_deref() {
        push(category.to_string(), LinkReason::RecordKeyword);
    }
    for tag in &source.tags {
        push(normalize_tag(tag), LinkReason::RecordKeyword);
    }
    if let Some(project) = source.project_id.as_deref() {
        push(project.to_string(), LinkReason::RecordKeyword);
    }
    for phrase in split_phrases(&source.title) {
        push(phrase, LinkReason::RecordKeyword);
    }
    for phrase in split_phrases(&source.body) {
        push(phrase, LinkReason::RecordKeyword);
    }
    out
}

fn split_phrases(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split(is_phrase_separator) {
        let trimmed = part.trim();
        let chars = trimmed.chars().count();
        if (MIN_ENTITY_NAME_CHARS..=MAX_PHRASE_CHARS).contains(&chars) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn is_phrase_separator(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(
            c,
            '，' | '。'
                | '；'
                | '：'
                | '！'
                | '？'
                | '、'
                | '（'
                | '）'
                | '【'
                | '】'
                | '「'
                | '」'
                | '《'
                | '》'
                | '—'
                | '…'
        )
}

fn load_meta(
    vault_root: &Path,
    hit: &SearchHit,
    cache: &mut HashMap<String, NoteMeta>,
) -> NoteMeta {
    if let Some(meta) = cache.get(&hit.relative_path) {
        return meta.clone();
    }
    let meta = match read_vault_note(vault_root, &hit.relative_path) {
        Ok(note) => {
            let doc = parse_note(&note.content);
            NoteMeta {
                title: title_from_note(&doc, &hit.file_name),
                aliases: doc.aliases.clone(),
                tags: doc.tags.clone(),
                raw: doc.raw.clone(),
            }
        }
        Err(_) => NoteMeta {
            title: fallback_title(&hit.file_name),
            aliases: Vec::new(),
            tags: hit.tags.clone(),
            raw: String::new(),
        },
    };
    cache.insert(hit.relative_path.clone(), meta.clone());
    meta
}

fn title_from_note(doc: &ParsedNote, file_name: &str) -> String {
    doc.properties
        .iter()
        .find(|property| property.key.eq_ignore_ascii_case("title"))
        .map(|property| property_text(&property.value).trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback_title(file_name))
}

fn property_text(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text(text) => text.clone(),
        PropertyValue::Number(number) => number.clone(),
        PropertyValue::Bool(boolean) => boolean.to_string(),
        PropertyValue::Null => String::new(),
        PropertyValue::List(items) => items.join(" "),
    }
}

fn fallback_title(file_name: &str) -> String {
    file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .to_string()
}

fn candidate_from_hit(
    source: &LinkSource,
    term: &QueryTerm,
    hit: &SearchHit,
    meta: &NoteMeta,
) -> LinkCandidate {
    let entity = entity_evidence(source, meta);
    let (reason, reason_text) = match entity {
        Some(evidence) => (
            LinkReason::Entity,
            bounded_reason(format!("{}：{}", LinkReason::Entity.label(), evidence)),
        ),
        None if term.origin == LinkReason::AiTopic => (
            LinkReason::AiTopic,
            bounded_reason(format!(
                "{}：「{}」命中知识条目",
                LinkReason::AiTopic.label(),
                term.text
            )),
        ),
        None => (
            LinkReason::RecordKeyword,
            bounded_reason(format!(
                "{}：「{}」命中知识条目",
                LinkReason::RecordKeyword.label(),
                term.text
            )),
        ),
    };
    let line = hit
        .snippet_line
        .or_else(|| locate_line(&meta.raw, &term.text))
        .unwrap_or(1);
    LinkCandidate {
        knowledge_id: hit.relative_path.clone(),
        title: meta.title.clone(),
        reason,
        reason_text,
        path: hit.relative_path.clone(),
        line,
    }
}

fn entity_evidence(source: &LinkSource, meta: &NoteMeta) -> Option<String> {
    let source_text = normalize_for_match(&source.searchable_text());
    let entities = source.entity_values();

    for entity in &entities {
        let normalized = normalize_for_match(entity);
        if normalized.is_empty() {
            continue;
        }
        if meta
            .tags
            .iter()
            .any(|tag| normalize_for_match(tag) == normalized)
        {
            return Some(format!("记录实体「{}」与知识标签一致", entity));
        }
        if normalize_for_match(&meta.title) == normalized {
            return Some(format!("记录实体「{}」与知识标题一致", entity));
        }
        if meta
            .aliases
            .iter()
            .any(|alias| normalize_for_match(alias) == normalized)
        {
            return Some(format!("记录实体「{}」与知识别名一致", entity));
        }
    }

    for name in std::iter::once(&meta.title).chain(meta.aliases.iter()) {
        let normalized = normalize_for_match(name);
        let chars = normalized.chars().count();
        if chars >= MIN_ENTITY_NAME_CHARS
            && chars <= MAX_ENTITY_NAME_CHARS
            && source_text.contains(&normalized)
        {
            return Some(format!("记录提到知识实体「{}」", name));
        }
    }
    None
}

fn normalize_for_match(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn locate_line(raw: &str, needle: &str) -> Option<usize> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }
    let lower = needle.to_lowercase();
    raw.lines()
        .position(|line| line.to_lowercase().contains(&lower))
        .map(|index| index + 1)
}

fn bounded_reason(text: String) -> String {
    if text.chars().count() <= MAX_REASON_CHARS {
        return text;
    }
    let mut out: String = text.chars().take(MAX_REASON_CHARS - 1).collect();
    out.push('…');
    out
}

fn insert_candidate(out: &mut Vec<LinkCandidate>, candidate: LinkCandidate) {
    if let Some(existing) = out
        .iter_mut()
        .find(|existing| existing.knowledge_id == candidate.knowledge_id)
    {
        if candidate.reason.rank() < existing.reason.rank() {
            *existing = candidate;
        }
        return;
    }
    out.push(candidate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::KnowledgeLink;
    use std::fs;

    struct Fixture {
        base: std::path::PathBuf,
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-knowledge-link-{tag}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("vault");
            fs::create_dir_all(root.join("notes")).unwrap();
            Self { base, root }
        }

        fn write(&self, rel: &str, body: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, body).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn seed(fx: &Fixture) {
        fx.write(
            "notes/alpha.md",
            "---\ntitle: 项目 Alpha 复盘\ntags: [项目/Alpha, 复盘]\naliases: [Alpha 复盘]\n---\n\nAlpha 复盘正文提到关键决策。\n",
        );
        fx.write(
            "notes/knowledge-topic.md",
            "---\ntitle: 知识管理方法\ntags: [知识管理]\n---\n\n知识管理主题与记录分析建议一致。\n",
        );
        fx.write(
            "notes/writing.md",
            "---\ntitle: 写作素材整理\ntags: [写作]\n---\n\n写作素材整理方法与记录关键词相同。\n",
        );
        fx.write(
            "notes/unrelated.md",
            "---\ntitle: 无关注记\n---\n\n完全无关的正文。\n",
        );
        fx.write("assets/pic.png", "not a note");
    }

    #[test]
    fn entity_tag_match_is_ranked_before_ai_topic() {
        let fx = Fixture::new("entity-order");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Task(&task_with("项目 Alpha 复盘", Some("项目/Alpha"))),
            &["知识管理".to_string()],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 2, "{candidates:?}");
        assert_eq!(candidates[0].knowledge_id, "notes/alpha.md");
        assert_eq!(candidates[0].reason, LinkReason::Entity);
        assert!(candidates[0].reason_text.starts_with("实体关联："));
        assert_eq!(candidates[1].knowledge_id, "notes/knowledge-topic.md");
        assert_eq!(candidates[1].reason, LinkReason::AiTopic);
        assert!(candidates[1].reason_text.starts_with("AI 主题建议："));
    }

    #[test]
    fn ai_hint_candidate_has_ai_reason_and_original_line() {
        let fx = Fixture::new("ai-reason");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("普通记录", "", &[], None)),
            &["知识管理".to_string()],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].knowledge_id, "notes/knowledge-topic.md");
        assert_eq!(candidates[0].title, "知识管理方法");
        assert_eq!(candidates[0].reason, LinkReason::AiTopic);
        assert_eq!(candidates[0].path, "notes/knowledge-topic.md");
        assert_eq!(candidates[0].line, 6, "正文命中行应映射回原文件行号");
        assert!(candidates[0].reason_text.contains("知识管理"));
        assert!(matches!(candidates[0].reason.label(), "AI 主题建议"));
    }

    #[test]
    fn record_keyword_search_is_the_lowest_reason() {
        let fx = Fixture::new("keyword");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("写作素材", "", &[], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].knowledge_id, "notes/writing.md");
        assert_eq!(candidates[0].reason, LinkReason::RecordKeyword);
        assert!(candidates[0]
            .reason_text
            .starts_with("记录关键词检索："));
    }

    #[test]
    fn dedupe_keeps_the_highest_priority_reason() {
        let fx = Fixture::new("dedupe");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with(
                "项目 Alpha 复盘",
                "项目 Alpha 复盘",
                &["项目/Alpha"],
                None,
            )),
            &["Alpha".to_string(), "项目 Alpha".to_string()],
            &fx.root,
        )
        .unwrap();
        let alpha: Vec<&LinkCandidate> = candidates
            .iter()
            .filter(|candidate| candidate.knowledge_id == "notes/alpha.md")
            .collect();
        assert_eq!(alpha.len(), 1, "同一知识条目只保留一条候选");
        assert_eq!(alpha[0].reason, LinkReason::Entity);
    }

    #[test]
    fn candidates_are_sorted_by_reason_and_capped_at_eight() {
        let fx = Fixture::new("cap");
        fx.write(
            "notes/entity.md",
            "---\ntitle: 实体核心条目\ntags: [实体/核心]\n---\n\n主题正文。\n",
        );
        fx.write(
            "notes/ai-a.md",
            "---\ntitle: 主题甲\n---\n\n主题正文。\n",
        );
        fx.write(
            "notes/ai-b.md",
            "---\ntitle: 主题乙\n---\n\n主题正文。\n",
        );
        for index in 0..10 {
            fx.write(
                &format!("notes/keyword-{index:02}.md"),
                &format!("---\ntitle: 关键词条目 {index}\n---\n\n主题正文。\n"),
            );
        }
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("主题", "", &["实体/核心"], None)),
            &["主题甲".to_string(), "主题乙".to_string()],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), MAX_LINK_CANDIDATES);
        assert_eq!(MAX_LINK_CANDIDATES, 8);
        assert_eq!(candidates[0].knowledge_id, "notes/entity.md");
        assert_eq!(candidates[0].reason, LinkReason::Entity);
        assert_eq!(candidates[1].reason, LinkReason::AiTopic);
        assert_eq!(candidates[2].reason, LinkReason::AiTopic);
        assert!(candidates[3..]
            .iter()
            .all(|candidate| candidate.reason == LinkReason::RecordKeyword));
    }

    #[test]
    fn empty_source_and_hints_return_no_candidates_instead_of_all_notes() {
        let fx = Fixture::new("empty");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("", "", &[], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn body_location_uses_original_file_line_number() {
        let fx = Fixture::new("line");
        fx.write(
            "notes/line.md",
            "---\ntitle: 行号笔记\ntags: [行号]\n---\n\n第一行。\n第二行提到 Needle 这个词。\n",
        );
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("NEEDLE", "", &[], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].line, 7);
        assert_eq!(candidates[0].path, "notes/line.md");
    }

    #[test]
    fn frontmatter_only_match_reports_the_frontmatter_line() {
        let fx = Fixture::new("frontmatter-line");
        fx.write(
            "notes/tagged.md",
            "---\ntitle: 标签笔记\ntags: [唯一标签]\n---\n\n正文没有该标签。\n",
        );
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("", "", &["唯一标签"], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, LinkReason::Entity);
        assert_eq!(candidates[0].line, 3, "标签命中应定位到原始 Frontmatter 行");
    }

    #[test]
    fn alias_mentioned_in_source_is_entity_association() {
        let fx = Fixture::new("alias");
        fx.write(
            "notes/alias.md",
            "---\ntitle: 别名笔记\naliases: [复盘代号A]\ntags: [别名]\n---\n\n正文不含别名。\n",
        );
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("复盘代号A", "", &[], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].knowledge_id, "notes/alias.md");
        assert_eq!(candidates[0].reason, LinkReason::Entity);
        assert!(candidates[0].reason_text.contains("复盘代号A"));
    }

    #[test]
    fn note_without_title_uses_file_stem_as_candidate_title() {
        let fx = Fixture::new("title-fallback");
        fx.write("notes/无名笔记.md", "没有 Frontmatter 的正文。\n");
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("无名笔记", "", &[], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "无名笔记");
        assert_eq!(candidates[0].knowledge_id, "notes/无名笔记.md");
    }

    #[test]
    fn matching_is_case_insensitive_for_tags() {
        let fx = Fixture::new("case");
        fx.write(
            "notes/case.md",
            "---\ntitle: 大小写笔记\ntags: [project/alpha]\n---\n\n大小写正文。\n",
        );
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("", "", &["PROJECT/ALPHA"], None)),
            &[],
            &fx.root,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, LinkReason::Entity);
    }

    #[test]
    fn unrelated_notes_and_attachments_are_not_recommended() {
        let fx = Fixture::new("unrelated");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with("完全无关的检索词", "", &[], None)),
            &["完全无关的主题".to_string()],
            &fx.root,
        )
        .unwrap();
        assert!(candidates.is_empty(), "{candidates:?}");
    }

    #[test]
    fn record_and_task_sources_extract_the_expected_fields() {
        let record = record_with("记录标题", "记录正文", &["记录标签"], Some("调研"));
        let source = LinkSource::from_record(&record);
        assert_eq!(source.title, "记录标题");
        assert_eq!(source.body, "记录正文");
        assert_eq!(source.tags, vec!["记录标签"]);
        assert_eq!(source.category.as_deref(), Some("调研"));
        assert_eq!(source.project_id, None);

        let task = task_with("任务标题", Some("项目-A"));
        let source = LinkSource::from_task(&task);
        assert_eq!(source.title, "任务标题");
        assert_eq!(source.project_id.as_deref(), Some("项目-A"));
        assert!(source.tags.is_empty());
        assert!(source.body.is_empty());
    }

    #[test]
    fn every_candidate_reason_text_starts_with_its_enum_label() {
        let fx = Fixture::new("reason-labels");
        seed(&fx);
        let candidates = suggest_links(
            LinkSubject::Record(&record_with(
                "写作素材整理",
                "知识管理",
                &["项目/Alpha"],
                None,
            )),
            &["知识管理".to_string()],
            &fx.root,
        )
        .unwrap();
        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert!(
                candidate
                    .reason_text
                    .starts_with(&format!("{}：", candidate.reason.label())),
                "{candidate:?}"
            );
            assert!(candidate.reason_text.chars().count() <= MAX_REASON_CHARS);
        }
    }

    fn record_with(title: &str, body: &str, tags: &[&str], category: Option<&str>) -> WorkRecord {
        WorkRecord {
            id: "record-synthetic".to_string(),
            title: title.to_string(),
            record_date: "2026-11-09".to_string(),
            created_at: "2026-11-09T00:00:00Z".to_string(),
            updated_at: "2026-11-09T00:00:00Z".to_string(),
            category: category.map(|value| value.to_string()),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            source: None,
            raw_input: None,
            raw_source: None,
            destination: None,
            ai_analysis_status: "analyzed".to_string(),
            linked_task_ids: Vec::new(),
            draft: false,
            related_knowledge: Vec::<KnowledgeLink>::new(),
            body: body.to_string(),
        }
    }

    fn task_with(title: &str, project_id: Option<&str>) -> Task {
        Task {
            id: "task-synthetic".to_string(),
            title: title.to_string(),
            state: "待开始".to_string(),
            created_at: "2026-11-09T00:00:00Z".to_string(),
            updated_at: "2026-11-09T00:00:00Z".to_string(),
            source_record_id: None,
            source_quote: None,
            linked_record_ids: Vec::new(),
            category: None,
            due_date: None,
            hint: None,
            importance: "待评估".to_string(),
            importance_source: None,
            importance_reason: None,
            due_kind: "hard".to_string(),
            depends_on: Vec::new(),
            estimated_minutes: None,
            parent_id: None,
            quadrant_override: None,
            project_id: project_id.map(|value| value.to_string()),
            related_knowledge: Vec::<KnowledgeLink>::new(),
            progress: Vec::new(),
            history: Vec::new(),
            adjustments: Vec::new(),
        }
    }
}
