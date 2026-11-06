//! 知识库整理工作流（OD-ARCH-141）。
//!
//! 只负责三件确定性的宿主动作：初始化目录、校验模型返回的结构化结果、逐卡落盘并
//! 更新索引/归档原文。模型没有文件通道；所有路径、标题、长度和来源都在这里复核。

use std::fs;
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::knowledge_vault::{self, VaultFile, VaultScan};
use crate::vault_note;
use crate::vault_search::{self, SearchQuery};
use crate::vault_write;

pub const KNOWLEDGE_DIR: &str = "知识库";
pub const CONCEPT_DIR: &str = "知识库/概念";
pub const METHOD_DIR: &str = "知识库/方法";
pub const ORIGINAL_DIR: &str = "知识库/原文";
pub const INBOX_DIR: &str = "inbox";
pub const INDEX_REL: &str = "知识库/索引.md";
pub const CARD_MAX_CHARS: usize = 300;
pub const INDEX_TEMPLATE: &str = "# 知识库索引\n\n## 概念\n\n## 方法\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestInit {
    pub root: String,
    pub index_path: String,
    pub created_dirs: Vec<String>,
    pub index_created: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestPayload {
    #[serde(default)]
    pub cards: Vec<IngestCard>,
    #[serde(default)]
    pub index_lines: Vec<String>,
    #[serde(default)]
    pub archive_moves: Vec<ArchiveMove>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestCard {
    pub relative_path: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArchiveMove {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestRejection {
    pub relative_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedCard {
    pub relative_path: String,
    pub title: String,
    pub content: String,
    pub sources: Vec<String>,
    pub index_line: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IngestValidation {
    pub ok: bool,
    pub cards: Vec<ValidatedCard>,
    pub rejected_cards: Vec<IngestRejection>,
    pub index_lines: Vec<String>,
    pub archive_moves: Vec<ArchiveMove>,
    pub rejected_moves: Vec<IngestRejection>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreatedCard {
    pub relative_path: String,
    pub title: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ApplyReport {
    pub ok: bool,
    pub created: Vec<CreatedCard>,
    pub rejected_cards: Vec<IngestRejection>,
    pub index_updated: bool,
    pub moved: Vec<ArchiveMove>,
    pub move_errors: Vec<IngestRejection>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepareFile {
    pub relative_path: String,
    pub chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepareResult {
    pub prompt: String,
    pub files: Vec<PrepareFile>,
    /// 收件箱材料正文（红线扫描对象），与 prompt 来自同一批文件。
    pub user_input_text: String,
    pub index_path: String,
}

fn join_inbox_text(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(_, content)| content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn clean_rel(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("路径为空".to_string());
    }
    let path = Path::new(text);
    if path.is_absolute() {
        return Err(format!("只接受知识库内相对路径：{text}"));
    }
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy().to_string();
                if name.starts_with('.') {
                    return Err(format!("不能写入隐藏路径：{text}"));
                }
                parts.push(name);
            }
            _ => return Err(format!("路径包含非法片段：{text}")),
        }
    }
    Ok(parts.join("/"))
}

fn is_card_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    parts.len() == 3
        && parts[0] == KNOWLEDGE_DIR
        && (parts[1] == "概念" || parts[1] == "方法")
        && parts[2].ends_with(".md")
}

fn section_of(path: &str) -> &'static str {
    if path.starts_with(CONCEPT_DIR) { "概念" } else { "方法" }
}

fn card_file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn title_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn index_titles(index: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in index.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            continue;
        }
        let start = 3;
        if let Some(end) = trimmed[start..].find(']') {
            out.push(title_key(&trimmed[start..start + end]));
        }
    }
    out
}

fn existing_card_titles(scan: &VaultScan) -> Vec<String> {
    let mut out = Vec::new();
    for file in &scan.files {
        if file.relative_path.starts_with(CONCEPT_DIR) || file.relative_path.starts_with(METHOD_DIR) {
            if let Some(stem) = file.file_name.strip_suffix(".md") {
                out.push(title_key(stem));
            }
        }
    }
    out
}

pub fn init_vault(root: &Path) -> Result<IngestInit, String> {
    let mut created = Vec::new();
    for rel in [KNOWLEDGE_DIR, CONCEPT_DIR, METHOD_DIR, ORIGINAL_DIR, INBOX_DIR] {
        let path = root.join(rel);
        if !path.is_dir() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("创建目录失败（{}）：{}", path.display(), e))?;
            created.push(rel.to_string());
        }
    }
    let index = root.join(INDEX_REL);
    let index_created = if index.is_file() {
        false
    } else {
        fs::write(&index, INDEX_TEMPLATE)
            .map_err(|e| format!("写入索引失败（{}）：{}", index.display(), e))?;
        true
    };
    Ok(IngestInit {
        root: root.display().to_string(),
        index_path: index.display().to_string(),
        created_dirs: created,
        index_created,
    })
}

pub fn validate_payload(result: &Value, index: &str, scan: &VaultScan) -> IngestValidation {
    let mut report = IngestValidation::default();
    let payload: IngestPayload = match serde_json::from_value(result.clone()) {
        Ok(value) => value,
        Err(e) => {
            report.errors.push(format!("整理结果不是约定的 JSON：{e}"));
            return report;
        }
    };
    let mut known_titles = index_titles(index);
    known_titles.extend(existing_card_titles(scan));
    let source_paths: Vec<&str> = scan
        .files
        .iter()
        .filter(|f: &&VaultFile| f.kind == "note")
        .map(|f| f.relative_path.as_str())
        .collect();
    let mut seen_titles = Vec::new();
    let mut used_index_lines = Vec::new();

    for card in payload.cards {
        let path = match clean_rel(&card.relative_path) {
            Ok(path) => path,
            Err(reason) => {
                report.rejected_cards.push(IngestRejection {
                    relative_path: card.relative_path,
                    reason,
                });
                continue;
            }
        };
        if !is_card_path(&path) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "卡片只能落在 知识库/概念/ 或 知识库/方法/".to_string(),
            });
            continue;
        }
        let title = card.title.trim().to_string();
        if title.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "标题不能为空".to_string(),
            });
            continue;
        }
        let key = title_key(&title);
        if known_titles.iter().any(|t| t == &key) || seen_titles.iter().any(|t| t == &key) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("标题重复：{title}"),
            });
            continue;
        }
        let body_chars = vault_note::parse_note(&card.content).body.chars().count();
        if body_chars > CARD_MAX_CHARS {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("正文 {body_chars} 字，超过 {CARD_MAX_CHARS} 字上限"),
            });
            continue;
        }
        if card.sources.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "sources 不能为空".to_string(),
            });
            continue;
        }
        let mut missing = Vec::new();
        for source in &card.sources {
            let source = match clean_rel(source) {
                Ok(value) => value,
                Err(_) => {
                    missing.push(source.clone());
                    continue;
                }
            };
            if !source_paths.iter().any(|p| p == &source) {
                missing.push(source);
            }
        }
        if !missing.is_empty() {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: format!("来源不存在：{}", missing.join(", ")),
            });
            continue;
        }
        let file_name = card_file_name(&path);
        let index_line = payload
            .index_lines
            .iter()
            .find(|line| line.contains(&title) && line.contains(file_name))
            .cloned();
        let Some(index_line) = index_line else {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "缺少与卡片标题/文件名对应的索引行".to_string(),
            });
            continue;
        };
        if used_index_lines.iter().any(|line| line == &index_line) {
            report.rejected_cards.push(IngestRejection {
                relative_path: path,
                reason: "索引行被多张卡重复使用".to_string(),
            });
            continue;
        }
        seen_titles.push(key);
        used_index_lines.push(index_line.clone());
        report.cards.push(ValidatedCard {
            relative_path: path,
            title,
            content: card.content,
            sources: card.sources,
            index_line: index_line.clone(),
        });
        report.index_lines.push(index_line);
    }

    for mv in payload.archive_moves {
        let from = clean_rel(&mv.from);
        let to = clean_rel(&mv.to);
        match (from, to) {
            (Ok(from), Ok(to))
                if from.starts_with("inbox/")
                    && to.starts_with("知识库/原文/")
                    && (from.ends_with(".md") || from.ends_with(".txt"))
                    && !source_paths.iter().any(|p| p == &to) =>
            {
                if source_paths.iter().any(|p| p == &from) {
                    report.archive_moves.push(ArchiveMove { from, to });
                } else {
                    report.rejected_moves.push(IngestRejection {
                        relative_path: mv.from,
                        reason: "待归档文件不在扫描结果里".to_string(),
                    });
                }
            }
            (from, to) => report.rejected_moves.push(IngestRejection {
                relative_path: mv.from,
                reason: format!(
                    "归档路径不合法（{} → {}）",
                    from.unwrap_or_else(|e| e),
                    to.unwrap_or_else(|e| e)
                ),
            }),
        }
    }

    report.ok = report.errors.is_empty()
        && !report.cards.is_empty()
        && report.rejected_cards.is_empty()
        && report.rejected_moves.is_empty();
    report
}

fn append_index_lines(index: &str, entries: &[(String, String)]) -> String {
    let mut lines: Vec<String> = index.lines().map(str::to_string).collect();
    if lines.is_empty() {
        lines.extend(INDEX_TEMPLATE.lines().map(str::to_string));
    }
    for section in ["概念", "方法"] {
        let heading = format!("## {section}");
        let additions: Vec<String> = entries
            .iter()
            .filter(|(s, _)| s == section)
            .map(|(_, line)| line.clone())
            .filter(|line| !lines.iter().any(|existing| existing.trim() == line.trim()))
            .collect();
        if additions.is_empty() {
            continue;
        }
        if let Some(pos) = lines.iter().position(|line| line.trim() == heading) {
            let mut insert = pos + 1;
            while insert < lines.len() && !lines[insert].trim_start().starts_with("## ") {
                insert += 1;
            }
            let mut block = Vec::new();
            if insert > pos + 1 && !lines[insert - 1].trim().is_empty() {
                block.push(String::new());
            }
            block.extend(additions);
            lines.splice(insert..insert, block);
        } else {
            if lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            lines.push(heading);
            lines.extend(additions);
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn apply_payload(root: &Path, result: &Value, scan: &VaultScan) -> ApplyReport {
    let index = fs::read_to_string(root.join(INDEX_REL)).unwrap_or_else(|_| INDEX_TEMPLATE.to_string());
    let validation = validate_payload(result, &index, scan);
    let mut report = ApplyReport {
        rejected_cards: validation.rejected_cards.clone(),
        errors: validation.errors.clone(),
        ..Default::default()
    };
    let mut entries: Vec<(String, String)> = Vec::new();
    for card in &validation.cards {
        match vault_write::create_note(root, &card.relative_path, &card.content) {
            Ok(note) => {
                report.created.push(CreatedCard {
                    relative_path: card.relative_path.clone(),
                    title: card.title.clone(),
                    bytes: note.bytes,
                });
                entries.push((section_of(&card.relative_path).to_string(), card.index_line.clone()));
            }
            Err(e) => report.rejected_cards.push(IngestRejection {
                relative_path: card.relative_path.clone(),
                reason: e,
            }),
        }
    }
    if !entries.is_empty() {
        let next = append_index_lines(&index, &entries);
        let expected = vault_write::content_sha256(&index);
        match vault_write::save_note(root, INDEX_REL, &next, &expected) {
            Ok(_) => report.index_updated = true,
            Err(e) => {
                report.errors.push(format!("索引更新失败：{e}"));
                for card in &report.created {
                    let _ = fs::remove_file(root.join(&card.relative_path));
                }
                report.created.clear();
            }
        }
    }
    if !entries.is_empty() && !report.index_updated {
        for mv in &validation.archive_moves {
            report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: "索引未更新，原文未归档".to_string(),
            });
        }
    } else {
    for mv in &validation.archive_moves {
        let source = match knowledge_vault::resolve_vault_file(root, &mv.from) {
            Ok(path) => path,
            Err(e) => {
                report.move_errors.push(IngestRejection {
                    relative_path: mv.from.clone(),
                    reason: e,
                });
                continue;
            }
        };
        let target = root.join(&mv.to);
        if target.exists() {
            report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: format!("归档目标已存在：{}", mv.to),
            });
            continue;
        }
        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                report.move_errors.push(IngestRejection {
                    relative_path: mv.from.clone(),
                    reason: format!("创建归档目录失败：{e}"),
                });
                continue;
            }
        }
        match fs::rename(&source, &target) {
            Ok(_) => report.moved.push(mv.clone()),
            Err(e) => report.move_errors.push(IngestRejection {
                relative_path: mv.from.clone(),
                reason: format!("移动原文失败：{e}"),
            }),
        }
    }
    }
    report.ok = report.errors.is_empty() && !report.created.is_empty();
    report
}

pub fn prepare_inbox(
    root: &Path,
    dsh_home: &Path,
    python: &str,
    engine_dir: &Path,
    dictionary_path: Option<&Path>,
) -> Result<PrepareResult, String> {
    let scan = knowledge_vault::scan_vault(root, None)?;
    let mut inbox: Vec<_> = scan
        .files
        .iter()
        .filter(|f| f.kind == "note" && f.relative_path.starts_with("inbox/"))
        .collect();
    inbox.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    inbox.truncate(5);
    if inbox.is_empty() {
        return Err("收件箱没有可整理的 Markdown/文本材料。".to_string());
    }
    let mut files = Vec::new();
    for file in &inbox {
        let note = knowledge_vault::read_vault_note(root, &file.relative_path)?;
        files.push((file.relative_path.clone(), note.content));
    }
    let index_path = root.join(INDEX_REL);
    if !index_path.is_file() {
        init_vault(root)?;
    }
    let index = fs::read_to_string(&index_path)
        .map_err(|e| format!("读取索引失败（{}）：{}", index_path.display(), e))?;
    let mut spec_parts = Vec::new();
    for relative in [
        "knowledge-agent/SKILL.md",
        "knowledge-search/SKILL.md",
    ] {
        let path = dsh_home.join("skills").join(relative);
        if let Ok(text) = fs::read_to_string(&path) {
            spec_parts.push(text);
        }
    }
    if spec_parts.is_empty() {
        return Err(format!(
            "找不到建卡规范：{} / {}（请先完成首次启动的技能播种）",
            dsh_home.join("skills/knowledge-agent/SKILL.md").display(),
            dsh_home.join("skills/knowledge-search/SKILL.md").display()
        ));
    }
    let card_spec = spec_parts.join("\n\n---\n\n");
    let payload = serde_json::json!({
        "files": files.iter().map(|(path, content)| serde_json::json!({
            "relative_path": path,
            "content": content,
        })).collect::<Vec<_>>(),
        "index": index,
        "card_spec": card_spec,
        "dictionary_path": dictionary_path.map(|p| p.display().to_string()),
    });
    let mut child = Command::new(python)
        .arg("-m")
        .arg("omnidesk_engine.knowledge_ingest")
        .current_dir(engine_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动整理提示词组装失败（{}）：{}", python, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|e| format!("写入整理提示词请求失败：{}", e))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待整理提示词组装失败：{}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(format!("整理提示词组装失败：{}", tail));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("整理提示词组装返回的不是 JSON：{}", e))?;
    let prompt = value
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| "整理提示词组装结果缺少 prompt".to_string())?;
    Ok(PrepareResult {
        prompt: prompt.to_string(),
        files: files
            .iter()
            .map(|(path, content)| PrepareFile {
                relative_path: path.clone(),
                chars: content.chars().count(),
            })
            .collect(),
        user_input_text: join_inbox_text(&files),
        index_path: index_path.display().to_string(),
    })
}

pub fn parse_result_text(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    let body = if trimmed.starts_with("```") {
        let first_newline = trimmed.find('\n').ok_or_else(|| "模型返回了空代码围栏".to_string())?;
        let rest = &trimmed[first_newline + 1..];
        rest.rsplit_once("```")
            .map(|(inner, _)| inner.trim())
            .ok_or_else(|| "模型返回的代码围栏没有闭合".to_string())?
    } else {
        trimmed
    };
    serde_json::from_str(body).map_err(|e| format!("整理结果不是合法 JSON：{}", e))
}

/// S7 冒烟：只用隔离根与合成材料，验证建卡后能被现有预算检索命中。
pub fn smoke_search(root: &Path, keyword: &str) -> Result<usize, String> {
    let response = vault_search::search_vault(
        root,
        &SearchQuery {
            keyword: keyword.to_string(),
            tags: Vec::new(),
            properties: Vec::new(),
        },
    )?;
    Ok(response.hits.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-ingest-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn seed_source(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn inbox_scan_text_is_joined_from_same_batch() {
        let files = vec![
            ("inbox/a.md".to_string(), "合成甲".to_string()),
            ("inbox/b.md".to_string(), "合成乙".to_string()),
        ];
        assert_eq!(join_inbox_text(&files), "合成甲\n\n合成乙");
    }

    fn card_payload(index_line: &str) -> Value {
        serde_json::json!({
            "cards": [{
                "relative_path": "知识库/概念/专项检查频次.md",
                "title": "专项检查频次",
                "content": "---\ntags: [监督, 专项检查]\nsources: [知识库/原文/合成制度.md]\ndate: 2026-11-05\n---\n一类单位每季度不少于一次。",
                "sources": ["知识库/原文/合成制度.md"]
            }],
            "index_lines": [index_line],
            "archive_moves": []
        })
    }

    #[test]
    fn init_creates_skeleton_and_is_idempotent() {
        let root = root("init");
        let first = init_vault(&root).unwrap();
        assert!(root.join(CONCEPT_DIR).is_dir());
        assert!(root.join(METHOD_DIR).is_dir());
        assert!(root.join(ORIGINAL_DIR).is_dir());
        assert!(root.join(INBOX_DIR).is_dir());
        assert!(root.join(INDEX_REL).is_file());
        assert_eq!(first.created_dirs.len(), 5);
        let before = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let second = init_vault(&root).unwrap();
        assert!(second.created_dirs.is_empty());
        assert!(!second.index_created);
        assert_eq!(fs::read_to_string(root.join(INDEX_REL)).unwrap(), before);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_accepts_legal_card_and_sources() {
        let root = root("valid");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let line = "- [专项检查频次](概念/专项检查频次.md) | 标签：监督, 专项检查 | 每季度一次";
        let report = validate_payload(&card_payload(line), &index, &scan);
        assert!(report.ok, "{:?}", report.rejected_cards);
        assert_eq!(report.cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_path_outside_card_dirs() {
        let root = root("badpath");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["relative_path"] = serde_json::json!("笔记/专项检查频次.md");
        let report = validate_payload(&payload, &index, &scan);
        assert!(!report.rejected_cards.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_duplicate_title_in_index() {
        let root = root("duptitle");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = "# 知识库索引\n\n## 概念\n- [专项检查频次](概念/旧卡.md) | 标签：监督 | 旧卡\n";
        let report = validate_payload(&card_payload("- [专项检查频次](概念/专项检查频次.md) | x"), index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_duplicate_title_within_batch() {
        let root = root("dupbatch");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        let mut second = payload["cards"][0].clone();
        second["relative_path"] = serde_json::json!("知识库/方法/专项检查频次.md");
        payload["cards"].as_array_mut().unwrap().push(second);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.cards.len(), 1);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_body_over_300_chars() {
        let root = root("long");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["content"] = serde_json::json!(format!("---\ntags: [监督]\n---\n{}", "长".repeat(301)));
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_missing_source() {
        let root = root("missing");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let report = validate_payload(&card_payload("- [专项检查频次](概念/专项检查频次.md) | x"), &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_empty_source_list() {
        let root = root("nosources");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["cards"][0]["sources"] = serde_json::json!([]);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        assert!(report.rejected_cards[0].reason.contains("sources"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_missing_index_line() {
        let root = root("noindex");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let report = validate_payload(&card_payload("- [别的标题](概念/别的.md) | x"), &index, &scan);
        assert_eq!(report.rejected_cards.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validator_rejects_invalid_archive_move() {
        let root = root("movebad");
        init_vault(&root).unwrap();
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        payload["archive_moves"] = serde_json::json!([{"from": "知识库/概念/x.md", "to": "知识库/原文/x.md"}]);
        let report = validate_payload(&payload, &index, &scan);
        assert_eq!(report.rejected_moves.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_creates_card_updates_index_and_moves_source() {
        let root = root("apply");
        init_vault(&root).unwrap();
        seed_source(&root, "inbox/合成制度.md", "专项检查每季度一次");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let line = "- [专项检查频次](概念/专项检查频次.md) | 标签：监督, 专项检查 | 每季度一次";
        let mut payload = card_payload(line);
        payload["cards"][0]["sources"] = serde_json::json!(["inbox/合成制度.md"]);
        payload["archive_moves"] = serde_json::json!([{"from": "inbox/合成制度.md", "to": "知识库/原文/合成制度.md"}]);
        let report = apply_payload(&root, &payload, &scan);
        assert!(report.ok, "{:?} {:?}", report.errors, report.rejected_cards);
        assert!(root.join("知识库/概念/专项检查频次.md").is_file());
        assert!(!root.join("inbox/合成制度.md").exists());
        assert!(root.join("知识库/原文/合成制度.md").is_file());
        let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
        assert!(index.contains("专项检查频次"));
        assert!(index.contains("概念/专项检查频次.md"));
        assert!(smoke_search(&root, "专项检查").unwrap() >= 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_keeps_valid_cards_when_one_is_rejected() {
        let root = root("partial");
        init_vault(&root).unwrap();
        seed_source(&root, "知识库/原文/合成制度.md", "合成材料");
        let scan = knowledge_vault::scan_vault(&root, None).unwrap();
        let mut payload = card_payload("- [专项检查频次](概念/专项检查频次.md) | x");
        let mut bad = payload["cards"][0].clone();
        bad["relative_path"] = serde_json::json!("笔记/坏卡.md");
        bad["title"] = serde_json::json!("坏卡");
        payload["cards"].as_array_mut().unwrap().push(bad);
        let report = apply_payload(&root, &payload, &scan);
        assert_eq!(report.created.len(), 1);
        assert_eq!(report.rejected_cards.len(), 1);
        assert!(report.index_updated);
        let _ = fs::remove_dir_all(&root);
    }
}
