//! 写作逐章节确定性差异（OD-GROW-301 G8）。
//!
//! 纯函数：不读写文件、不调用模型。按正文.md 的 `## n.` 分节拆章，
//! 章内按空行分段，产出被删除与新增的句子片段；片段按 100 字符切分，
//! 不含整章全文。整理模块只拿这里的片段做跨稿重复判定。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::writing_section::parse_body_sections;

/// 单个差异片段的最大字符数。
pub const MAX_FRAGMENT_CHARS: usize = 100;

/// 一章的差异结果；removed/added 都是句子片段。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionDiff {
    pub section_no: u32,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

/// 比较两份正文的逐章节差异。
///
/// 文本无法按 `## n.` 拆章时返回空列表；只返回有变化的章节，
/// 章节按编号升序，片段按出现顺序，同章内去重。
pub fn section_diffs(text_a: &str, text_b: &str) -> Vec<SectionDiff> {
    let sections_a = match parse_body_sections(text_a) {
        Ok(sections) => sections,
        Err(_) => return Vec::new(),
    };
    let sections_b = match parse_body_sections(text_b) {
        Ok(sections) => sections,
        Err(_) => return Vec::new(),
    };
    let mut section_nos: BTreeSet<u32> = sections_a.keys().copied().collect();
    section_nos.extend(sections_b.keys().copied());

    let mut diffs = Vec::new();
    for section_no in section_nos {
        let content_a = sections_a.get(&section_no).map(String::as_str).unwrap_or("");
        let content_b = sections_b.get(&section_no).map(String::as_str).unwrap_or("");
        let diff = section_diff(section_no, content_a, content_b);
        if diff.removed.is_empty() && diff.added.is_empty() {
            continue;
        }
        diffs.push(diff);
    }
    diffs
}

fn section_diff(section_no: u32, content_a: &str, content_b: &str) -> SectionDiff {
    let paragraphs_a = split_paragraphs(content_a);
    let paragraphs_b = split_paragraphs(content_b);
    let set_a: BTreeSet<&str> = paragraphs_a.iter().map(String::as_str).collect();
    let set_b: BTreeSet<&str> = paragraphs_b.iter().map(String::as_str).collect();

    let sentences_a = collect_sentences(&paragraphs_a);
    let sentences_b = collect_sentences(&paragraphs_b);

    let mut removed = Vec::new();
    for paragraph in paragraphs_a.iter().filter(|value| !set_b.contains(value.as_str())) {
        for fragment in sentences(paragraph) {
            if !sentences_b.contains(&fragment) && !removed.contains(&fragment) {
                removed.push(fragment);
            }
        }
    }
    let mut added = Vec::new();
    for paragraph in paragraphs_b.iter().filter(|value| !set_a.contains(value.as_str())) {
        for fragment in sentences(paragraph) {
            if !sentences_a.contains(&fragment) && !added.contains(&fragment) {
                added.push(fragment);
            }
        }
    }
    SectionDiff {
        section_no,
        removed,
        added,
    }
}

/// 按空行分段，段内每行保留换行，两端空白去掉。
fn split_paragraphs(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }
    paragraphs
        .into_iter()
        .map(|paragraph| paragraph.trim().to_string())
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

fn collect_sentences(paragraphs: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for paragraph in paragraphs {
        out.extend(sentences(paragraph));
    }
    out
}

/// 按中文句末标点与换行切句；单句超过上限时按字符切成多个片段。
fn sentences(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buffer = String::new();
    for ch in paragraph.chars() {
        if ch == '\n' || matches!(ch, '。' | '！' | '？' | '!' | '?' | '；') {
            let trimmed = buffer.trim();
            if !trimmed.is_empty() {
                if ch == '\n' {
                    push_chunks(trimmed, &mut out);
                } else {
                    push_chunks(&format!("{trimmed}{ch}"), &mut out);
                }
            }
            buffer.clear();
        } else {
            buffer.push(ch);
        }
    }
    let trimmed = buffer.trim();
    if !trimmed.is_empty() {
        push_chunks(trimmed, &mut out);
    }
    out
}

fn push_chunks(text: &str, out: &mut Vec<String>) {
    let mut chunk = String::new();
    let mut chars = 0usize;
    for ch in text.chars() {
        chunk.push(ch);
        chars += 1;
        if chars >= MAX_FRAGMENT_CHARS {
            out.push(chunk.trim().to_string());
            chunk.clear();
            chars = 0;
        }
    }
    let trimmed = chunk.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(sections: &[(u32, &str)]) -> String {
        let mut out = String::from("# 合成文稿\n");
        for (no, content) in sections {
            out.push_str(&format!("\n## {}. 合成章节\n\n{}", no, content));
        }
        out
    }

    #[test]
    fn unchanged_text_has_no_diff() {
        let text = body(&[(1, "第一段合成内容。\n\n第二段合成内容。")]);
        assert!(section_diffs(&text, &text).is_empty());
        assert_eq!(section_diffs("", ""), Vec::new());
    }

    #[test]
    fn unchanged_sentences_are_not_reported_when_neighbour_changes() {
        let a = body(&[(1, "保留的合成句子。删除的合成句子。")]);
        let b = body(&[(1, "保留的合成句子。")]);
        let diffs = section_diffs(&a, &b);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].section_no, 1);
        assert_eq!(diffs[0].removed, vec!["删除的合成句子。".to_string()]);
        assert!(diffs[0].added.is_empty());
    }

    #[test]
    fn removed_and_added_sections_are_reported() {
        let a = body(&[(1, "第一章第一段。\n\n第一章第二段。"), (2, "第二章合成内容。")]);
        let b = body(&[(1, "第一章第一段。\n\n第一章改写后的第二段。"), (3, "第三章合成内容。")]);
        let diffs = section_diffs(&a, &b);
        assert_eq!(diffs.len(), 3, "{diffs:?}");

        assert_eq!(diffs[0].section_no, 1);
        assert_eq!(diffs[0].removed, vec!["第一章第二段。".to_string()]);
        assert_eq!(diffs[0].added, vec!["第一章改写后的第二段。".to_string()]);

        assert_eq!(diffs[1].section_no, 2);
        assert_eq!(diffs[1].removed, vec!["第二章合成内容。".to_string()]);
        assert!(diffs[1].added.is_empty());

        assert_eq!(diffs[2].section_no, 3);
        assert!(diffs[2].removed.is_empty());
        assert_eq!(diffs[2].added, vec!["第三章合成内容。".to_string()]);
    }

    #[test]
    fn long_fragment_is_split_below_limit() {
        let mut long: String = (0..250u32)
            .map(|index| char::from_u32(0x4e00 + index).expect("合成汉字"))
            .collect();
        long.push('。');
        let a = body(&[(1, long.as_str())]);
        let b = body(&[(1, "另一段合成内容。")]);
        let diffs = section_diffs(&a, &b);
        assert_eq!(diffs.len(), 1);
        assert!(
            diffs[0].removed.iter().all(|fragment| fragment.chars().count() <= MAX_FRAGMENT_CHARS),
            "{:?}",
            diffs[0].removed
        );
        assert!(
            diffs[0].removed.iter().map(|fragment| fragment.chars().count()).sum::<usize>() >= 250,
            "{:?}",
            diffs[0].removed
        );
    }

    #[test]
    fn malformed_text_returns_empty_without_panicking() {
        assert!(section_diffs("没有分节标记的正文", "## 1. 合成章节").is_empty());
    }
}
