//! 相关工作记录检索（OD-HAR-212）。
//!
//! 只为「分析一条记录时补一点历史上下文」服务：只读、确定性、有上限，
//! 不建索引、不写文件、不调用模型。
//!
//! 隐私边界：交给模型的历史命中只有**元数据**（编号/标题/日期/分类/标签/理由），
//! 不附带其他记录的正文片段。理由只写命中依据，不回显正文。

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::worklog_records::{self, RecordFilter, WorkRecord};

/// 一次分析最多注入几条历史记录。
pub const MAX_HISTORY_HITS: usize = 5;
/// 历史块的总字符上限（与提示词的其它段落分开控制）。
pub const MAX_HISTORY_CHARS: usize = 2000;
/// 最多扫描多少条记录，防止异常规模拖慢分析。
pub const MAX_HISTORY_SCAN: usize = 200;
/// 标题/标签的展示上限，避免一条命中撑满预算。
pub const MAX_TITLE_CHARS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryHit {
    pub id: String,
    pub title: String,
    pub record_date: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    /// 命中依据（分类/标签/关键词），不含正文片段。
    pub reason: String,
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect::<String>() + "…"
}

/// 从标题与正文前缀抽取 2 字片段（只保留字母/数字/汉字），用于粗略的词面重合。
fn bigrams(text: &str) -> BTreeSet<String> {
    let cleaned: Vec<char> = text
        .chars()
        .take(4000)
        .filter(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(c))
        .collect();
    let mut out = BTreeSet::new();
    for window in cleaned.windows(2) {
        let gram: String = window.iter().collect();
        if gram.chars().count() == 2 {
            out.insert(gram);
        }
    }
    out
}

/// 找与当前记录相关的历史记录（排除自身与草稿）。
pub fn related_history(root: &Path, current: &WorkRecord, limit: usize) -> Vec<HistoryHit> {
    let capped = limit.clamp(1, MAX_HISTORY_HITS);
    let listing = match worklog_records::list_records(root, &RecordFilter::default()) {
        Ok(listing) => listing,
        Err(_) => return Vec::new(),
    };
    let grams = bigrams(&format!("{}\n{}", current.title, current.body));
    let mut hits: Vec<(i32, HistoryHit)> = Vec::new();
    for record in listing.items.iter().take(MAX_HISTORY_SCAN) {
        if record.id == current.id || record.draft {
            continue;
        }
        let mut score = 0i32;
        let mut reasons: Vec<String> = Vec::new();
        let shared_tags: Vec<String> = record
            .tags
            .iter()
            .filter(|tag| current.tags.iter().any(|mine| mine == *tag))
            .take(3)
            .cloned()
            .collect();
        if !shared_tags.is_empty() {
            score += 2 * shared_tags.len() as i32;
            reasons.push(format!("同标签：{}", shared_tags.join("、")));
        }
        if let (Some(mine), Some(theirs)) = (current.category.as_deref(), record.category.as_deref()) {
            if mine == theirs {
                score += 3;
                reasons.push("同分类".to_string());
            }
        }
        let other_grams = bigrams(&format!("{}\n{}", record.title, record.body));
        let overlap = grams.intersection(&other_grams).count();
        if overlap >= 2 {
            score += std::cmp::min(overlap as i32, 10);
            reasons.push("词面重合".to_string());
        }
        if score <= 0 {
            continue;
        }
        hits.push((
            score,
            HistoryHit {
                id: record.id.clone(),
                title: truncate_chars(&record.title, MAX_TITLE_CHARS),
                record_date: record.record_date.clone(),
                category: record.category.clone(),
                tags: record.tags.iter().take(6).cloned().collect(),
                reason: reasons.join("；"),
            },
        ));
    }
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.record_date.cmp(&a.1.record_date))
            .then_with(|| a.1.id.cmp(&b.1.id))
    });
    hits.truncate(capped);
    let mut out: Vec<HistoryHit> = Vec::new();
    let mut used = 0usize;
    for (_, hit) in hits {
        let row_chars = hit.id.chars().count()
            + hit.title.chars().count()
            + hit.record_date.chars().count()
            + hit.reason.chars().count()
            + hit.tags.iter().map(|t| t.chars().count()).sum::<usize>()
            + 24;
        if used + row_chars > MAX_HISTORY_CHARS && !out.is_empty() {
            break;
        }
        used += row_chars;
        out.push(hit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "od-har-212-history-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn seed(root: &Path, title: &str, body: &str, category: Option<&str>, tags: &[&str], draft: bool) -> WorkRecord {
        worklog_records::create_record(
            root,
            &worklog_records::RecordDraft {
                id: None,
                title: Some(title.to_string()),
                record_date: Some("2026-11-12".to_string()),
                category: category.map(str::to_string),
                tags: tags.iter().map(|tag| tag.to_string()).collect(),
                draft,
                body: body.to_string(),
                ..Default::default()
            },
            1_796_000_000.0,
            480,
        )
        .unwrap()
    }

    #[test]
    fn finds_same_category_and_tag_history_but_skips_self_and_drafts() {
        let root = temp_root("basic");
        let current = seed(&root, "整理周报", "今天整理周报，补充项目进展。", Some("文案"), &["周报"], false);
        let related = seed(&root, "周报素材", "整理周报所需的数据。", Some("文案"), &["周报"], false);
        let draft = seed(&root, "草稿周报", "还没写完。", Some("文案"), &["周报"], true);
        let unrelated = seed(&root, "值班安排", "下周值班表已确认。", Some("行政"), &[], false);
        let hits = related_history(&root, &current, 5);
        assert_eq!(hits.len(), 1, "只应有同分类/同标签的正式记录");
        assert_eq!(hits[0].id, related.id);
        assert!(!hits.iter().any(|hit| hit.id == draft.id || hit.id == current.id || hit.id == unrelated.id));
        assert!(hits[0].reason.contains("同分类"));
    }

    #[test]
    fn caps_hits_and_keeps_order_deterministic() {
        let root = temp_root("cap");
        let current = seed(&root, "整理周报", "整理周报。", Some("文案"), &["周报"], false);
        for index in 0..7 {
            seed(&root, &format!("周报素材{index}"), "整理周报数据。", Some("文案"), &["周报"], false);
        }
        let hits = related_history(&root, &current, 3);
        assert_eq!(hits.len(), 3);
        let again = related_history(&root, &current, 3);
        assert_eq!(hits, again, "同一输入必须得到同一顺序");
    }

    #[test]
    fn no_history_is_empty_not_error() {
        let root = temp_root("empty");
        let current = seed(&root, "独立记录", "没有同类内容。", None, &[], false);
        assert!(related_history(&root, &current, 5).is_empty());
    }
}
