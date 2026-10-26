//! 已扫描笔记集合上的筛选与关键词搜索（OD-KB-001 P2-S05）。
//!
//! 不建立持久索引：每次查询都从 P1 扫描结果出发，在内存里读取、解析、匹配，
//! 查询结束即丢弃。规则固定如下，界面不得另行解释：
//!   * 关键词：Unicode 大小写不敏感的字面子串匹配，不做分词、正则或模糊匹配；
//!   * 标签：比较前统一小写。筛选 `项目` 同时命中 `项目` 与它的下级标签 `项目/alpha`；
//!     多个标签取交集（AND）；
//!   * 属性：键名大小写不敏感且必须相等，值大小写不敏感地做子串匹配；
//!     只给键表示「存在该属性」。多个属性条件取交集；
//!   * 关键词与标签、属性同时给出时取交集；
//!   * 命中区域：path / body / property / tag，按固定顺序返回；
//!   * 正文命中给出行号与一行上下文；无效 Frontmatter 不影响正文搜索。
//!
//! 资源边界：最多检查 P1 扫描返回的 2,000 个文件；每次查询最多读取
//! [`SEARCH_MAX_BYTES`] 字节，单篇仍受 512 KiB 上限约束。预算耗尽时停止读取并
//! 明确返回 `budget_exhausted`，不静默给出「没有更多结果」的假象。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::knowledge_vault::{read_vault_note_within, scan_vault, VaultFile};
use crate::vault_note::{parse_note, ParsedNote, Property, PropertyValue};

/// 单次查询最多读取的笔记正文字节数（32 MiB）。
pub const SEARCH_MAX_BYTES: u64 = 32 * 1024 * 1024;
/// 单次查询最多返回的命中条数。
pub const SEARCH_MAX_HITS: usize = 500;
/// 正文上下文最多保留的字符数。
const CONTEXT_CHARS: usize = 120;

/// 一个属性筛选条件。`value` 为空表示只要求该属性存在。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyFilter {
    pub key: String,
    #[serde(default)]
    pub value: String,
}

/// 查询条件。全部字段为空时返回所有可读笔记。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub properties: Vec<PropertyFilter>,
}

impl SearchQuery {
    /// 去掉空白并规范标签，用于界面回显与匹配。
    pub fn normalized(&self) -> SearchQuery {
        SearchQuery {
            keyword: self.keyword.trim().to_string(),
            tags: self
                .tags
                .iter()
                .map(|t| crate::vault_note::normalize_tag(t))
                .filter(|t| !t.is_empty())
                .collect(),
            properties: self
                .properties
                .iter()
                .filter(|p| !p.key.trim().is_empty())
                .map(|p| PropertyFilter {
                    key: p.key.trim().to_string(),
                    value: p.value.trim().to_string(),
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        let q = self.normalized();
        q.keyword.is_empty() && q.tags.is_empty() && q.properties.is_empty()
    }
}

/// 一条命中。`match_areas` 固定为 path / body / property / tag 的子集。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub relative_path: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub match_areas: Vec<String>,
    pub tags: Vec<String>,
    pub property_count: usize,
    pub warning_count: usize,
    /// 正文命中的一行上下文（关键词在正文命中时给出）。
    pub snippet: Option<String>,
    /// 正文命中所在行号（1 起）。
    pub snippet_line: Option<usize>,
    pub keyword_hits: usize,
}

/// 因预算或读取失败没有检查的文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchSkip {
    pub relative_path: String,
    pub reason: String,
}

/// 一次查询的实测统计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub notes_available: usize,
    pub notes_examined: usize,
    pub bytes_read: u64,
    pub budget_bytes: u64,
    pub budget_exhausted: bool,
    /// 命中条数达到上限时为 true；不是读取预算耗尽。
    pub hits_truncated: bool,
    pub skipped: Vec<SearchSkip>,
    pub query: SearchQuery,
}

fn property_text(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text(s) => s.clone(),
        PropertyValue::Number(n) => n.clone(),
        PropertyValue::Bool(b) => b.to_string(),
        PropertyValue::Null => String::new(),
        PropertyValue::List(items) => items.join(" "),
    }
}

fn property_matches(property: &Property, filter: &PropertyFilter) -> bool {
    if !property.key.eq_ignore_ascii_case(filter.key.trim()) {
        return false;
    }
    if filter.value.trim().is_empty() {
        return true;
    }
    property_text(&property.value)
        .to_lowercase()
        .contains(&filter.value.trim().to_lowercase())
}

/// 标签筛选：相等或属于其下级；多个条件全部满足。
fn tag_matches(note_tags: &[String], wanted: &str) -> bool {
    note_tags.iter().any(|tag| {
        tag == wanted || (tag.starts_with(wanted) && tag.as_bytes().get(wanted.len()) == Some(&b'/'))
    })
}

fn keyword_in_properties(doc: &ParsedNote, needle: &str) -> bool {
    doc.properties.iter().any(|p| {
        p.key.to_lowercase().contains(needle)
            || property_text(&p.value).to_lowercase().contains(needle)
    })
}

fn keyword_in_tags(doc: &ParsedNote, needle: &str) -> bool {
    doc.tags.iter().any(|tag| tag.contains(needle))
}

/// 找正文第一处命中：返回行号与该行上下文。
fn body_context(body: &str, needle: &str) -> Option<(usize, String)> {
    for (idx, line) in body.lines().enumerate() {
        if line.to_lowercase().contains(needle) {
            let trimmed = line.trim();
            let chars: Vec<char> = trimmed.chars().collect();
            let text = if chars.len() > CONTEXT_CHARS {
                let mut head: String = chars[..CONTEXT_CHARS].iter().collect();
                head.push('…');
                head
            } else {
                trimmed.to_string()
            };
            return Some((idx + 1, text));
        }
    }
    None
}

fn count_keyword_in_body(body: &str, needle: &str) -> usize {
    let lower = body.to_lowercase();
    if needle.is_empty() || !lower.contains(needle) {
        return 0;
    }
    lower.match_indices(needle).count()
}

fn evaluate(file: &VaultFile, doc: &ParsedNote, query: &SearchQuery) -> Option<SearchHit> {
    let needle = query.keyword.to_lowercase();
    for wanted in &query.tags {
        if !tag_matches(&doc.tags, wanted) {
            return None;
        }
    }
    for filter in &query.properties {
        if !doc.properties.iter().any(|p| property_matches(p, filter)) {
            return None;
        }
    }

    let mut areas: Vec<String> = Vec::new();
    let path_hit = !needle.is_empty()
        && (file.relative_path.to_lowercase().contains(&needle)
            || file.file_name.to_lowercase().contains(&needle));
    let property_hit = !needle.is_empty() && keyword_in_properties(doc, &needle);
    let tag_hit = !needle.is_empty() && keyword_in_tags(doc, &needle);
    let context = if needle.is_empty() {
        None
    } else {
        body_context(&doc.body, &needle)
    };
    let body_hits = count_keyword_in_body(&doc.body, &needle);

    if !needle.is_empty() {
        if path_hit {
            areas.push("path".to_string());
        }
        if body_hits > 0 {
            areas.push("body".to_string());
        }
        if property_hit {
            areas.push("property".to_string());
        }
        if tag_hit {
            areas.push("tag".to_string());
        }
        if areas.is_empty() {
            return None;
        }
    }

    Some(SearchHit {
        relative_path: file.relative_path.clone(),
        file_name: file.file_name.clone(),
        bytes: doc.raw.len() as u64,
        sha256: crate::sha256::sha256_hex(doc.raw.as_bytes()),
        match_areas: areas,
        tags: doc.tags.clone(),
        property_count: doc.properties.len(),
        warning_count: doc.warnings.len(),
        snippet: context.as_ref().map(|(_, text)| text.clone()),
        // 展示给用户的应是原文件行号，不是正文内部行号：加上 Frontmatter 占用的行数。
        snippet_line: context.map(|(line, _)| doc.body_start_line + line - 1),
        keyword_hits: body_hits,
    })
}

/// 在扫描结果上执行查询。
pub fn search_scanned(
    root: &Path,
    files: &[VaultFile],
    query: &SearchQuery,
) -> SearchResponse {
    let query = query.normalized();
    let notes: Vec<&VaultFile> = files.iter().filter(|f| f.kind == "note").collect();
    let mut response = SearchResponse {
        hits: Vec::new(),
        notes_available: notes.len(),
        notes_examined: 0,
        bytes_read: 0,
        budget_bytes: SEARCH_MAX_BYTES,
        budget_exhausted: false,
        hits_truncated: false,
        skipped: Vec::new(),
        query: query.clone(),
    };
    let mark_remaining_budget = |response: &mut SearchResponse, from: usize| {
        for rest in notes.iter().skip(from) {
            response.skipped.push(SearchSkip {
                relative_path: rest.relative_path.clone(),
                reason: "budget".to_string(),
            });
        }
    };
    for (index, file) in notes.iter().enumerate() {
        // 扫描元数据只能作为提示：预算必须按实际读入字节强制执行，因为文件可能
        // 在扫描后增长。读取前用剩余额度做上限，读取后按实际读入量记账。
        let remaining = SEARCH_MAX_BYTES.saturating_sub(response.bytes_read);
        // 保守快路径：扫描元数据已经超出剩余额度时直接跳过，不再发起读取。
        // 它只会少读，不能把过期的小值当成读取许可。
        if file.bytes > remaining {
            response.budget_exhausted = true;
            mark_remaining_budget(&mut response, index);
            break;
        }
        let read = read_vault_note_within(root, &file.relative_path, remaining);
        response.bytes_read = response.bytes_read.saturating_add(read.bytes_read);
        if read.budget_exhausted {
            response.budget_exhausted = true;
            // 超预算的文件没有返回任何正文；它和它后面的文件都算未检查。
            mark_remaining_budget(&mut response, index);
            break;
        }
        let note = match read.note {
            Ok(note) => note,
            Err(e) => {
                response.skipped.push(SearchSkip {
                    relative_path: file.relative_path.clone(),
                    reason: e,
                });
                continue;
            }
        };
        response.notes_examined += 1;
        let doc = parse_note(&note.content);
        if let Some(hit) = evaluate(file, &doc, &query) {
            if response.hits.len() >= SEARCH_MAX_HITS {
                response.hits_truncated = true;
                break;
            }
            response.hits.push(hit);
        }
    }
    response
}

/// 扫描并查询。
pub fn search_vault(root: &Path, query: &SearchQuery) -> Result<SearchResponse, String> {
    let scan = scan_vault(root, None)?;
    Ok(search_scanned(root, &scan.files, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture {
        base: std::path::PathBuf,
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-vault-search-{tag}-{}",
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
            "---\ntitle: Alpha 方案\ntags: [项目/Alpha, 待办]\nstatus: 进行中\n---\n\nAlpha 正文包含关键词 Needle。\n第二行。\n",
        );
        fx.write(
            "notes/beta.md",
            "---\ntitle: Beta\ntags: [项目/Beta]\nstatus: 完成\n---\n\n只有 beta 正文。\n",
        );
        fx.write(
            "notes/broken.md",
            "---\ntitle: [未闭合\n---\n正文仍然包含 needle 与小写。\n",
        );
        fx.write("notes/plain.md", "没有属性，也没有关键词。\n");
        fx.write("assets/pic.png", "not a note");
    }

    #[test]
    fn keyword_is_case_insensitive_and_reports_body_location() {
        let fx = Fixture::new("keyword");
        seed(&fx);
        let r = search_vault(&fx.root, &SearchQuery {
            keyword: "NEEDLE".into(),
            ..Default::default()
        })
        .unwrap();
        let paths: Vec<&str> = r.hits.iter().map(|h| h.relative_path.as_str()).collect();
        assert_eq!(paths, vec!["notes/alpha.md", "notes/broken.md"]);
        assert_eq!(r.hits[0].match_areas, vec!["body"]);
        assert_eq!(r.hits[0].keyword_hits, 1);
        // 行号是原文件行号：4 行 Frontmatter + 闭围栏 + 空行之后，正文命中在第 7 行。
        assert_eq!(r.hits[0].snippet_line, Some(7));
        assert!(r.hits[0].snippet.as_deref().unwrap().contains("Needle"));
        // 无效 Frontmatter 不丢正文搜索能力。
        assert_eq!(r.hits[1].match_areas, vec!["body"]);
        assert!(r.hits[1].warning_count >= 1);
        assert_eq!(r.notes_available, 4);
        assert_eq!(r.notes_examined, 4);
    }

    #[test]
    fn tag_filter_matches_descendants_and_requires_all_tags() {
        let fx = Fixture::new("tags");
        seed(&fx);
        let parent = search_vault(&fx.root, &SearchQuery {
            tags: vec!["项目".into()],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(parent.hits.len(), 2, "{:?}", parent.hits);

        let both = search_vault(&fx.root, &SearchQuery {
            tags: vec!["项目/alpha".into(), "待办".into()],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(both.hits.len(), 1);
        assert_eq!(both.hits[0].relative_path, "notes/alpha.md");

        let none = search_vault(&fx.root, &SearchQuery {
            tags: vec!["项目/alpha".into(), "不存在".into()],
            ..Default::default()
        })
        .unwrap();
        assert!(none.hits.is_empty());
    }

    #[test]
    fn property_filter_and_keyword_combine_with_and_semantics() {
        let fx = Fixture::new("props");
        seed(&fx);
        let by_status = search_vault(&fx.root, &SearchQuery {
            properties: vec![PropertyFilter {
                key: "STATUS".into(),
                value: "进行".into(),
            }],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(by_status.hits.len(), 1);
        assert_eq!(by_status.hits[0].relative_path, "notes/alpha.md");

        let combined = search_vault(&fx.root, &SearchQuery {
            keyword: "needle".into(),
            tags: vec!["项目".into()],
            properties: vec![PropertyFilter {
                key: "status".into(),
                value: "进行".into(),
            }],
        })
        .unwrap();
        assert_eq!(combined.hits.len(), 1);
        assert_eq!(combined.hits[0].relative_path, "notes/alpha.md");

        // 只给键：要求属性存在。
        let exists = search_vault(&fx.root, &SearchQuery {
            properties: vec![PropertyFilter {
                key: "title".into(),
                value: String::new(),
            }],
            ..Default::default()
        })
        .unwrap();
        // broken.md 的 Frontmatter 无效，不暴露 title 属性；正文搜索仍然可用。
        assert_eq!(exists.hits.len(), 2, "{:?}", exists.hits);
        assert!(exists
            .hits
            .iter()
            .all(|h| h.relative_path != "notes/broken.md"));
    }

    #[test]
    fn keyword_reports_property_tag_and_path_areas() {
        let fx = Fixture::new("areas");
        seed(&fx);
        let r = search_vault(&fx.root, &SearchQuery {
            keyword: "alpha".into(),
            ..Default::default()
        })
        .unwrap();
        let alpha = r.hits.iter().find(|h| h.relative_path == "notes/alpha.md").unwrap();
        assert_eq!(alpha.match_areas, vec!["path", "body", "property", "tag"]);
    }

    #[test]
    fn empty_query_lists_all_notes_only() {
        let fx = Fixture::new("all");
        seed(&fx);
        let r = search_vault(&fx.root, &SearchQuery::default()).unwrap();
        assert_eq!(r.hits.len(), 4);
        assert!(r.hits.iter().all(|h| h.match_areas.is_empty()));
        assert!(!r.hits.iter().any(|h| h.relative_path.ends_with("pic.png")));
    }

    #[test]
    fn budget_exhaustion_is_reported_not_hidden() {
        let fx = Fixture::new("budget");
        // 构造超过预算的合成文件元数据：不真的写 32 MiB 正文，而是直接给
        // 扫描条目一个超预算的 bytes 值，验证预算判定在读取之前生效。
        let files = vec![VaultFile {
            relative_path: "huge.md".into(),
            file_name: "huge.md".into(),
            extension: "md".into(),
            bytes: SEARCH_MAX_BYTES + 1,
            modified: None,
            kind: "note",
        }];
        let r = search_scanned(&fx.root, &files, &SearchQuery::default());
        assert!(r.budget_exhausted);
        assert_eq!(r.notes_examined, 0);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.skipped[0].reason, "budget");
    }

    /// 扫描之后文件增长时，预算必须按实际读入量生效，而不是相信过期的
    /// `VaultFile.bytes`。70 篇各 512 KiB（单篇上限）共 35 MiB，必须在恰好
    /// 读满 32 MiB 后停止，且剩余文件明确列为未检查，绝不返回部分正文。
    #[test]
    fn grown_files_are_capped_by_actual_bytes_read() {
        let fx = Fixture::new("grown-budget");
        // 最后一篇保持很小：预算耗尽后即使后续文件放得下，也必须停止，避免用
        // “跳过超预算文件”的方式拼出超出契约的部分结果集。
        let notes = 65usize;
        for i in 0..notes {
            fx.write(&format!("notes/n{i:03}.md"), "seed");
        }
        // 扫描发生在文件增长之前：元数据严重低估实际大小。
        let scan = scan_vault(&fx.root, None).unwrap();
        assert_eq!(scan.stats.notes, notes);
        let filler = "a".repeat(crate::knowledge_vault::VAULT_MAX_NOTE_BYTES as usize);
        for i in 0..(notes - 1) {
            fx.write(&format!("notes/n{i:03}.md"), &filler);
        }
        fx.write(&format!("notes/n{:03}.md", notes - 1), "tiny");

        let r = search_scanned(&fx.root, &scan.files, &SearchQuery::default());
        assert!(r.bytes_read <= r.budget_bytes, "{} > {}", r.bytes_read, r.budget_bytes);
        assert_eq!(r.bytes_read, SEARCH_MAX_BYTES, "应恰好用满预算后停止");
        assert!(r.budget_exhausted);
        assert_eq!(r.notes_examined, 64, "32 MiB / 512 KiB");
        assert_eq!(r.hits.len(), r.notes_examined, "已检查的文件都应完整返回");
        let bottom = r
            .skipped
            .iter()
            .filter(|s| s.reason == "budget")
            .count();
        assert_eq!(bottom, 1, "预算耗尽后剩余文件必须全部列出");
        assert_eq!(r.skipped[0].relative_path, format!("notes/n{:03}.md", notes - 1));
        assert_eq!(
            r.notes_examined + bottom,
            r.notes_available,
            "已检查与未检查之和必须覆盖全部可用笔记"
        );
    }

    /// 单篇读取也必须有硬上限：剩余额度小于文件大小时不返回任何正文字节，
    /// 并把已读入量如实带回调用方记账。
    #[test]
    fn bounded_read_never_returns_partial_content() {
        let fx = Fixture::new("bounded");
        fx.write("notes/a.md", &"x".repeat(100));
        let read = read_vault_note_within(&fx.root, "notes/a.md", 40);
        assert!(read.budget_exhausted);
        assert!(read.note.is_err());
        assert_eq!(read.bytes_read, 0, "超预算时不应读入部分正文");

        let exact = read_vault_note_within(&fx.root, "notes/a.md", 100);
        assert!(!exact.budget_exhausted);
        assert_eq!(exact.bytes_read, 100);
        assert_eq!(exact.note.unwrap().content.len(), 100);
    }

    /// P2-S05 合成规模实测：1200 篇笔记，在 32 MiB 读取预算内查询三次并打印中位数。
    /// 这是本机观测值，不是性能目标；规模、字节数与命中数一并输出，便于复核。
    #[test]
    fn synthetic_scale_search_metric() {
        let fx = Fixture::new("scale");
        let notes = 1200usize;
        // 每篇约 14 KiB 合成正文：1200 篇合计约 16 MiB，低于 32 MiB 预算。
        // 首次用 700 行时实测 38.7 MB 超出预算、只检查到 1039 篇，故按实测收敛样本。
        let filler = "合成正文段落，仅用于规模测量。\n".repeat(450);
        let total_bytes = std::cell::Cell::new(0u64);
        for i in 0..notes {
            let content = format!(
                "---\ntitle: 合成 {i}\ntags: [项目/规模]\nstatus: {}\n---\n\n{}{}\n",
                if i % 7 == 0 { "进行中" } else { "完成" },
                filler,
                if i % 11 == 0 { "命中词 needle。" } else { "普通结尾。" }
            );
            total_bytes.set(total_bytes.get() + content.len() as u64);
            fx.write(&format!("notes/n{i:04}.md"), &content);
        }
        let query = SearchQuery {
            keyword: "needle".into(),
            tags: vec!["项目".into()],
            properties: vec![PropertyFilter {
                key: "status".into(),
                value: "进行".into(),
            }],
        };
        let mut runs = Vec::new();
        let mut last = None;
        for _ in 0..3 {
            let started = std::time::Instant::now();
            let r = search_vault(&fx.root, &query).unwrap();
            runs.push(started.elapsed().as_secs_f64() * 1000.0);
            last = Some(r);
        }
        let r = last.unwrap();
        let mut sorted = runs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[1];
        let metric = format!(
            "{{\"notes\":{},\"total_bytes\":{},\"bytes_read\":{},\"budget_bytes\":{},\"budget_exhausted\":{},\"hits\":{},\"runs_ms\":[{:.3},{:.3},{:.3}],\"median_ms\":{:.3},\"platform\":\"{} {}\",\"profile\":\"debug\"}}",
            notes,
            total_bytes.get(),
            r.bytes_read,
            r.budget_bytes,
            r.budget_exhausted,
            r.hits.len(),
            runs[0],
            runs[1],
            runs[2],
            median,
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        println!("KB_P2_SEARCH_METRIC {metric}");
        assert_eq!(r.notes_examined, notes, "预算内的合成笔记应全部检查");
        assert!(!r.budget_exhausted, "合成规模必须落在读取预算内");
        assert!(r.bytes_read <= SEARCH_MAX_BYTES);
        assert!(r.hits.len() > 0);
    }
}
