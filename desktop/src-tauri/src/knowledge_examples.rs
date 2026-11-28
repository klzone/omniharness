//! OD-GROW-301 G9：把定稿章节里的范例段落送入知识审核队列。
//!
//! 这里只做候选搬运：提取可复用段落、跳过红线、隐私掩码，然后调用既有
//! `knowledge_review::stage` 进入待审核队列。真正的知识卡只有用户批准后
//! 才由既有审核路径落盘，本模块不直接创建知识卡。
//!
//! 现有准入要求卡片来源真实存在，因此来源固定写入知识库的 `inbox/写作范例/`
//! 源材料区；批准后由既有审核流程归档，不写入 `知识库/概念` 或 `知识库/方法`。

use std::fs;
use std::io::Write as _;
use std::path::Path;

use serde_json::json;

use crate::knowledge_ingest;
use crate::knowledge_review;
use crate::profile_rules::{mask_privacy, RulePolicy};
use crate::writing_section::WritingSectionConfirmed;

pub const MIN_EXAMPLE_CHARS: usize = 50;
pub const MAX_EXAMPLE_CHARS: usize = 300;
pub const MAX_EXAMPLES_PER_SECTION: usize = 3;
pub const EXAMPLE_TAG: &str = "写作范例";
const EXAMPLE_SOURCE_DIR: &str = "inbox/写作范例";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExampleStageReport {
    pub extracted: usize,
    pub skipped_redline: usize,
    pub staged_cards: usize,
    pub review_id: Option<String>,
    pub source_path: Option<String>,
}

/// 按空行分段，取 50–300 字符、非标题、非列表开头的段落，每章最多 3 段。
pub fn extract_example_paragraphs(body: &str) -> Vec<String> {
    let normalized = body.replace("\r\n", "\n");
    let mut paragraphs = Vec::new();
    for block in normalized.split("\n\n") {
        let text = block.trim();
        if text.is_empty() {
            continue;
        }
        let first_line = text.lines().next().unwrap_or("").trim_start();
        if first_line.starts_with('#') || is_list_item(first_line) {
            continue;
        }
        let chars = text.chars().count();
        if chars < MIN_EXAMPLE_CHARS || chars > MAX_EXAMPLE_CHARS {
            continue;
        }
        paragraphs.push(text.to_string());
        if paragraphs.len() == MAX_EXAMPLES_PER_SECTION {
            break;
        }
    }
    paragraphs
}

fn is_list_item(line: &str) -> bool {
    let mut chars = line.chars();
    match chars.next() {
        Some('-' | '*' | '+' | '•') => chars.next().is_some_and(char::is_whitespace),
        Some(first) if first.is_ascii_digit() => {
            let mut digits = 1usize;
            let mut rest = chars;
            while let Some(ch) = rest.next() {
                if ch.is_ascii_digit() {
                    digits += 1;
                    if digits > 3 {
                        return false;
                    }
                    continue;
                }
                return matches!(ch, '.' | '、' | ')')
                    && rest.next().is_some_and(char::is_whitespace);
            }
            false
        }
        _ => false,
    }
}

/// 从已确认章节提取候选，红线命中的段跳过，其余先掩码再暂存。
/// 任何错误由调用方记录，不改变章节确认结果。
pub fn stage_section_text(
    vault_root: &Path,
    review_dir: &Path,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
    writing_id: &str,
    section_no: u32,
    version: u64,
    content: &str,
) -> Result<ExampleStageReport, String> {
    let paragraphs = extract_example_paragraphs(content);
    let mut report = ExampleStageReport {
        extracted: paragraphs.len(),
        ..Default::default()
    };
    let mut accepted = Vec::new();
    for paragraph in paragraphs {
        if policy
            .reject_keywords()
            .iter()
            .any(|keyword| !keyword.is_empty() && paragraph.contains(keyword))
        {
            report.skipped_redline += 1;
            continue;
        }
        let (masked, _) = mask_privacy(&paragraph, policy);
        accepted.push(masked);
    }
    if accepted.is_empty() {
        return Ok(report);
    }

    knowledge_ingest::init_vault(vault_root)?;
    let source_rel = source_relative_path(writing_id, section_no, version);
    let source_path = vault_root.join(&source_rel);
    write_new_source(&source_path, &accepted.join("\n\n"))?;
    let payload = build_payload(
        writing_id,
        section_no,
        version,
        &source_rel,
        &accepted,
        vault_root,
    );
    let result_text = serde_json::to_string(&payload)
        .map_err(|error| format!("序列化写作范例候选失败：{error}"))?;
    let batch_sources = vec![source_rel.clone()];
    match knowledge_review::stage(
        vault_root,
        review_dir,
        &result_text,
        &batch_sources,
        policy,
        utc_offset_minutes,
    ) {
        Ok(item) => {
            report.staged_cards = item.card_count;
            report.review_id = Some(item.id);
            report.source_path = Some(source_rel);
            Ok(report)
        }
        Err(error) => {
            let _ = fs::remove_file(&source_path);
            Err(error)
        }
    }
}

/// 命令入口使用的完整版本：解析已配置知识库根与审核目录后暂存。
pub fn stage_confirmed_examples(
    paths: &crate::app_paths::AppPaths,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
    confirmed: &WritingSectionConfirmed,
) -> Result<ExampleStageReport, String> {
    stage_text_examples(
        paths,
        policy,
        utc_offset_minutes,
        &confirmed.writing_id,
        confirmed.section_no,
        confirmed.version,
        &confirmed.content,
    )
}

/// 审阅修订等其他确认路径使用的版本：只接受确认后的字段，不依赖回执类型。
pub fn stage_text_examples(
    paths: &crate::app_paths::AppPaths,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
    writing_id: &str,
    section_no: u32,
    version: u64,
    content: &str,
) -> Result<ExampleStageReport, String> {
    let vault_root = crate::configured_vault_root(paths)?;
    let review_dir = knowledge_review::dir(&paths.engine_state);
    stage_section_text(
        &vault_root,
        &review_dir,
        policy,
        utc_offset_minutes,
        writing_id,
        section_no,
        version,
        content,
    )
}

fn source_relative_path(writing_id: &str, section_no: u32, version: u64) -> String {
    let stem = format!("{writing_id}-第{section_no}章-v{version}");
    format!("{EXAMPLE_SOURCE_DIR}/{}.md", safe_file_stem(&stem))
}

fn write_new_source(path: &Path, text: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("范例来源没有父目录：{}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("创建范例来源目录失败（{}）：{error}", parent.display()))?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("写入范例来源失败（{}）：{error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("写入范例来源失败：{error}"))?;
    Ok(())
}

fn build_payload(
    writing_id: &str,
    section_no: u32,
    version: u64,
    source_rel: &str,
    paragraphs: &[String],
    vault_root: &Path,
) -> serde_json::Value {
    let mut cards = Vec::new();
    let mut index_lines = Vec::new();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let title = format!(
            "{EXAMPLE_TAG} {writing_id} 第{section_no}章 v{version} 第{}段",
            index + 1
        );
        let stem = safe_file_stem(&title);
        let relative_path = format!("知识库/概念/{stem}.md");
        let content = format!(
            "---\ntags: [{EXAMPLE_TAG}]\nsources: [{source_rel}]\n---\n{paragraph}"
        );
        index_lines.push(format!(
            "- [{title}](概念/{stem}.md) | 标签：{EXAMPLE_TAG} | 来源：{source_rel}"
        ));
        cards.push(json!({
            "relative_path": relative_path,
            "title": title,
            "content": content,
            "sources": [source_rel],
        }));
    }
    let source_file = source_rel.rsplit('/').next().unwrap_or(source_rel);
    let archive_to = format!("知识库/原文/{source_file}");
    let archive_moves = if vault_root.join(&archive_to).exists() {
        Vec::new()
    } else {
        vec![json!({"from": source_rel, "to": archive_to})]
    };
    json!({
        "cards": cards,
        "index_lines": index_lines,
        "archive_moves": archive_moves,
    })
}

fn safe_file_stem(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '（' | '）') {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let cleaned = out.trim_matches('-').to_string();
    if cleaned.chars().count() > 64 {
        cleaned.chars().take(64).collect()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::profile_rules::{parse_rules, ProfileRules};
    use crate::writing_outline::{self, OutlineConfirmRequest, OutlineSection, WritingOutline};
    use crate::writing_section::{SectionConfirmRequest, SectionDraft};
    use crate::writing_store::{self, WritingTaskDraft, TYPE_ANNUAL_SUMMARY};

    const NOW: f64 = 1_800_000_000.0;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-knowledge-examples-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn paragraph(chars: usize) -> String {
        "范".repeat(chars)
    }

    fn policy_with_redline(keyword: &str) -> RulePolicy {
        let mut rules: ProfileRules = parse_rules(crate::profile_rules::DEFAULT_RULES_JSON).unwrap();
        if !rules.global.redline_keywords.iter().any(|item| item == keyword) {
            rules.global.redline_keywords.push(keyword.to_string());
        }
        rules.validate().unwrap();
        RulePolicy::from_rules(&rules)
    }

    fn confirmed_section(work: &Path, content: &str) -> WritingSectionConfirmed {
        let task = writing_store::create_writing_task(
            work,
            &WritingTaskDraft {
                writing_id: Some("w-g9".to_string()),
                title: "虚构年度总结".to_string(),
                document_type: TYPE_ANNUAL_SUMMARY.to_string(),
                writing_purpose: "合成测试".to_string(),
                audience: "测试读者".to_string(),
                writing_period: "2026 年度".to_string(),
                topic: "合成主题".to_string(),
                requirements: String::new(),
                expected_length: "800 字".to_string(),
                writing_style: "平实".to_string(),
                selected_material_ids: vec![],
                selected_knowledge_ids: vec![],
                document_id: None,
            },
            NOW,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            work,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 1.0,
        )
        .unwrap();
        let outline = WritingOutline {
            title: task.title.clone(),
            writing_purpose: task.writing_purpose.clone(),
            core_points: vec!["合成观点".to_string()],
            sections: vec![OutlineSection {
                no: 1,
                heading: "工作概况".to_string(),
                sub_headings: vec![],
                focus: "合成章节".to_string(),
                material_ids: vec![],
                knowledge_ids: vec![],
                inherit_materials: true,
                expected_words: 300,
            }],
        };
        writing_outline::confirm_outline(
            work,
            &OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline,
                note: None,
            },
            NOW + 2.0,
        )
        .unwrap();
        crate::writing_section::confirm_section(
            work,
            &SectionConfirmRequest {
                writing_id: task.writing_id,
                draft: SectionDraft {
                    section_no: 1,
                    content: content.to_string(),
                    citations: vec![],
                    segments: vec![],
                },
                note: None,
                ai_draft_text: None,
            },
            NOW + 3.0,
        )
        .unwrap()
    }

    #[test]
    fn extraction_enforces_length_boundaries_and_skips_title_and_list() {
        let mut body = String::new();
        body.push_str(&paragraph(MIN_EXAMPLE_CHARS - 1));
        body.push_str("\n\n");
        body.push_str(&paragraph(MIN_EXAMPLE_CHARS));
        body.push_str("\n\n");
        body.push_str(&paragraph(MAX_EXAMPLE_CHARS));
        body.push_str("\n\n");
        body.push_str(&paragraph(MAX_EXAMPLE_CHARS + 1));
        body.push_str("\n\n## 标题段落正文足够长但属于标题\n\n");
        body.push_str(&format!("- {}\n\n", paragraph(MIN_EXAMPLE_CHARS)));
        body.push_str(&format!("1. {}\n\n", paragraph(MIN_EXAMPLE_CHARS)));
        let paragraphs = extract_example_paragraphs(&body);
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].chars().count(), MIN_EXAMPLE_CHARS);
        assert_eq!(paragraphs[1].chars().count(), MAX_EXAMPLE_CHARS);
    }

    #[test]
    fn extraction_caps_three_paragraphs_per_section() {
        let mut body = String::new();
        for index in 0..5 {
            body.push_str(&paragraph(MIN_EXAMPLE_CHARS + index));
            body.push_str("\n\n");
        }
        let paragraphs = extract_example_paragraphs(&body);
        assert_eq!(paragraphs.len(), MAX_EXAMPLES_PER_SECTION);
    }

    #[test]
    fn confirm_stages_non_redline_paragraphs() {
        let work = root("confirm-stage");
        let vault = work.join("vault");
        let review_dir = work.join("app-state").join(knowledge_review::REVIEW_DIR);
        let policy = policy_with_redline("机密");
        let body = format!(
            "{}\n\n{}含机密信息，整段应跳过。\n\n{}",
            paragraph(MIN_EXAMPLE_CHARS),
            paragraph(MIN_EXAMPLE_CHARS),
            paragraph(MIN_EXAMPLE_CHARS)
        );
        let confirmed = confirmed_section(&work.join("work"), &body);
        let report = stage_section_text(
            &vault,
            &review_dir,
            &policy,
            480,
            &confirmed.writing_id,
            confirmed.section_no,
            confirmed.version,
            &confirmed.content,
        )
        .unwrap();
        assert_eq!(report.extracted, 3);
        assert_eq!(report.skipped_redline, 1);
        assert_eq!(report.staged_cards, 2);
        let mut listing = knowledge_review::list(&review_dir);
        let item = listing.items.remove(0);
        assert_eq!(item.status, "pending");
        assert_eq!(item.card_count, 2);
        let source = report.source_path.expect("应返回来源路径");
        let source_text = fs::read_to_string(vault.join(&source)).unwrap();
        assert!(!source_text.contains("机密"));
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn confirm_keeps_success_when_example_stage_fails() {
        let work = root("confirm-stage-failure");
        let body = paragraph(MIN_EXAMPLE_CHARS);
        let request = SectionConfirmRequest {
            writing_id: "w-g9".to_string(),
            draft: SectionDraft {
                section_no: 1,
                content: body,
                citations: vec![],
                segments: vec![],
            },
            note: None,
            ai_draft_text: None,
        };
        let _ = confirmed_section(&work.join("work"), &paragraph(MIN_EXAMPLE_CHARS));
        let result = crate::writing_section::confirm_section_with_stage(
            &work.join("work"),
            &request,
            NOW + 4.0,
            |_| Err("合成暂存失败".to_string()),
        );
        assert!(result.is_ok(), "暂存失败不得影响章节确认");
        let _ = fs::remove_dir_all(&work);
    }
}
