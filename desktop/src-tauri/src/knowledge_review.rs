//! 知识审核暂存与裁决（OD-KB-201 S3）。
//!
//! 模型整理结果先进入宿主暂存区，未经人工裁决不写知识库：
//!   * 暂存记录落 `<应用数据根>/engine-state/knowledge-reviews/*.json`（仓库外）；
//!   * 同时在 vault 的 `governance/review-decisions.v1.md` 追加一条 propose 记录；
//!   * 通过审批后才调用既有 `knowledge_ingest::apply_payload` 落卡、更新索引、归档原文；
//!   * 驳回/需要修改只落治理决策记录，不碰知识库文件。
//!
//! 模型与前端都没有直接写治理文件的命令；裁决必须经由这里的受控动作。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::knowledge_governance::{self, GovernanceEntry, GovernanceRecord};
use crate::knowledge_ingest::{
    self, parse_result_text, validate_payload, ApplyReport, IngestCard, IngestPayload,
    IngestRejection,
};
use crate::profile_rules::RulePolicy;
use crate::vault_note;

pub const REVIEW_SCHEMA: &str = "omnidesk.knowledge-review.v1";
pub const REVIEW_DIR: &str = "knowledge-reviews";
const REVIEW_MAX_RESULT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewRecord {
    pub schema: String,
    pub id: String,
    pub result_text: String,
    pub created_at: f64,
    pub status: String,
    pub decided_at: Option<f64>,
    #[serde(default)]
    pub rejected_cards: Vec<IngestRejection>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub id: String,
    pub status: String,
    pub created_at: f64,
    pub decided_at: Option<f64>,
    pub card_count: usize,
    pub cards: Vec<IngestCard>,
    pub source_paths: Vec<String>,
    pub parse_error: Option<String>,
    pub rejected_cards: Vec<IngestRejection>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReviewListing {
    pub items: Vec<ReviewItem>,
    pub broken: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewDecisionReport {
    pub review: ReviewItem,
    pub apply: Option<ApplyReport>,
    pub governance_errors: Vec<String>,
}

pub fn dir(engine_state: &Path) -> PathBuf {
    engine_state.join(REVIEW_DIR)
}

fn record_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("审核记录编号不合法。".to_string());
    }
    Ok(dir.join(format!("{id}.json")))
}

fn parse_cards(result_text: &str) -> Result<IngestPayload, String> {
    let value = parse_result_text(result_text)?;
    serde_json::from_value(value).map_err(|e| format!("整理结果不是约定的 JSON：{e}"))
}

fn item_from_record(record: &ReviewRecord) -> ReviewItem {
    match parse_cards(&record.result_text) {
        Ok(payload) => {
            let mut sources: Vec<String> = payload
                .cards
                .iter()
                .flat_map(|card| card.sources.iter().cloned())
                .collect();
            sources.sort();
            sources.dedup();
            ReviewItem {
                id: record.id.clone(),
                status: record.status.clone(),
                created_at: record.created_at,
                decided_at: record.decided_at,
                card_count: payload.cards.len(),
                cards: payload.cards,
                source_paths: sources,
                parse_error: None,
                rejected_cards: record.rejected_cards.clone(),
                warnings: record.warnings.clone(),
            }
        }
        Err(e) => ReviewItem {
            id: record.id.clone(),
            status: record.status.clone(),
            created_at: record.created_at,
            decided_at: record.decided_at,
            card_count: 0,
            cards: Vec::new(),
            source_paths: Vec::new(),
            parse_error: Some(e),
            rejected_cards: record.rejected_cards.clone(),
            warnings: record.warnings.clone(),
        },
    }
}

fn load_record(dir: &Path, id: &str) -> Result<ReviewRecord, String> {
    let path = record_path(dir, id)?;
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取审核记录失败（{}）：{}", path.display(), e))?;
    let record: ReviewRecord = serde_json::from_str(&text)
        .map_err(|e| format!("审核记录格式错误（{}）：{}", path.display(), e))?;
    if record.schema != REVIEW_SCHEMA {
        return Err(format!("不支持的审核记录版本：{:?}", record.schema));
    }
    Ok(record)
}

fn create_record(dir: &Path, record: &ReviewRecord) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("创建审核暂存目录失败：{e}"))?;
    let path = record_path(dir, &record.id)?;
    let content = serde_json::to_vec_pretty(record)
        .map_err(|e| format!("序列化审核记录失败：{e}"))?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("创建审核记录失败（{}）：{}", path.display(), e))?;
    use std::io::Write;
    file.write_all(&content)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("写入审核记录失败（{}）：{}", path.display(), e))?;
    Ok(())
}

fn write_record(dir: &Path, record: &ReviewRecord) -> Result<(), String> {
    let path = record_path(dir, &record.id)?;
    let content = serde_json::to_vec_pretty(record)
        .map_err(|e| format!("序列化审核记录失败：{e}"))?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> Result<(), String> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入审核记录失败（{}）：{}", temp.display(), e))?;
        file.write_all(&content)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入审核记录失败（{}）：{}", temp.display(), e))?;
        fs::rename(&temp, &path).map_err(|e| format!("替换审核记录失败：{e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn new_id(now: f64) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = format!("{now:.0}")
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    format!(
        "kr-{stamp}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn validation_error(index: &str, scan: &crate::knowledge_vault::VaultScan, value: &serde_json::Value) -> Vec<String> {
    let report = validate_payload(value, index, scan);
    if report.ok {
        return Vec::new();
    }
    let mut out: Vec<String> = report.errors;
    if report.cards.is_empty() && out.is_empty() {
        out.push("整理结果没有可审核的卡片".to_string());
    }
    for rejected in report.rejected_cards.iter().take(5) {
        out.push(format!("{}：{}", rejected.relative_path, rejected.reason));
    }
    for rejected in report.rejected_moves.iter().take(5) {
        out.push(format!("归档 {}：{}", rejected.relative_path, rejected.reason));
    }
    out
}

fn is_split_boundary(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '；' | '\n')
}

/// 宿主兜底切分：模型偶尔仍会写出 420–1260 字的单卡。这里只在能切成 2–3 段时
/// 自动拆卡；更长的内容仍交回模型拆分（stage 会以超限卡警告跳过）。
fn split_body(body: &str, parts: usize, max_chars: usize) -> Option<Vec<String>> {
    if parts < 2 || parts > 3 {
        return None;
    }
    let chars: Vec<char> = body.chars().collect();
    if chars.len() <= max_chars {
        return None;
    }
    let mut boundaries = Vec::with_capacity(parts + 1);
    boundaries.push(0usize);
    for part in 1..parts {
        let target = chars.len() * part / parts;
        let mut chosen = target;
        let window = 20usize.min(max_chars / 2);
        for delta in 0..=window {
            let after = target.saturating_add(delta).min(chars.len() - 1);
            if after > 0 && is_split_boundary(chars[after - 1]) {
                chosen = after;
                break;
            }
            if delta <= target {
                let before = target - delta;
                if before > 0 && is_split_boundary(chars[before - 1]) {
                    chosen = before;
                    break;
                }
            }
        }
        boundaries.push(chosen);
    }
    boundaries.push(chars.len());
    let mut out = Vec::new();
    for window in boundaries.windows(2) {
        let start = window[0];
        let end = window[1];
        if start >= end {
            return None;
        }
        let piece: String = chars[start..end].iter().collect();
        let piece = piece.trim().to_string();
        if piece.is_empty() || piece.chars().count() > max_chars {
            return None;
        }
        out.push(piece);
    }
    if out.len() == parts {
        Some(out)
    } else {
        None
    }
}

fn split_suffix(index: usize, total: usize) -> &'static str {
    if total == 2 {
        if index == 0 { "上" } else { "下" }
    } else {
        match index {
            0 => "上",
            1 => "中",
            _ => "下",
        }
    }
}

fn auto_split_overlong(payload: IngestPayload, max_chars: usize) -> (IngestPayload, Vec<String>) {
    let original_lines = payload.index_lines.clone();
    let cards = payload.cards;
    let mut output_cards = Vec::new();
    let mut output_lines = Vec::new();
    let mut warnings = Vec::new();
    for card in cards {
        let file_name = card
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&card.relative_path)
            .to_string();
        let base_line = original_lines
            .iter()
            .find(|line| line.contains(&card.title) && line.contains(&file_name))
            .cloned()
            .unwrap_or_default();
        let parsed = vault_note::parse_note(&card.content);
        let body_chars = parsed.body.chars().count();
        if body_chars <= max_chars || body_chars > max_chars * 3 {
            output_cards.push(card);
            output_lines.push(base_line);
            continue;
        }
        let parts = (body_chars + max_chars - 1) / max_chars;
        let Some(chunks) = split_body(&parsed.body, parts, max_chars) else {
            output_cards.push(card);
            output_lines.push(base_line);
            continue;
        };
        let prefix = &card.content[..parsed.body_start_byte.min(card.content.len())];
        let titles: Vec<String> = chunks
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}（{}）", card.title, split_suffix(index, chunks.len())))
            .collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let suffix = split_suffix(index, chunks.len());
            let title = format!("{}（{suffix}）", card.title);
            let (stem, ext) = card
                .relative_path
                .rsplit_once('.')
                .unwrap_or((&card.relative_path, "md"));
            let split_path = format!("{stem}（{suffix}）.{ext}");
            let split_file = split_path.rsplit('/').next().unwrap_or(&split_path).to_string();
            let mut content = String::new();
            content.push_str(prefix);
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(chunk);
            if !content.ends_with('\n') {
                content.push('\n');
            }
            let sees = titles
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join("、");
            let link_target = split_path
                .strip_prefix("知识库/")
                .unwrap_or(&split_path)
                .to_string();
            let line = if base_line.is_empty() {
                format!("- [{title}]({link_target}) | 标签：拆分 | 由宿主自动拆分 | 参见：{sees}")
            } else {
                format!(
                    "{} | 参见：{sees}",
                    base_line
                        .replace(&card.title, &title)
                        .replace(&file_name, &split_file)
                )
            };
            output_cards.push(IngestCard {
                relative_path: split_path,
                title,
                content,
                sources: card.sources.clone(),
            });
            output_lines.push(line);
        }
        warnings.push(format!(
            "宿主自动拆分：{}（{} 字）→ {} 张",
            card.title,
            body_chars,
            chunks.len()
        ));
    }
    (
        IngestPayload {
            cards: output_cards,
            index_lines: output_lines,
            archive_moves: payload.archive_moves,
        },
        warnings,
    )
}

fn split_suffix_rank(title: &str) -> Option<(String, usize)> {
    for (suffix, rank) in [("（上）", 0), ("（中）", 1), ("（下）", 2)] {
        if let Some(base) = title.strip_suffix(suffix) {
            return Some((base.to_string(), rank));
        }
    }
    None
}

/// 模型拆卡后常把跨卡引用留在基名（[[原标题]]），但实际文件已变成（上）/（下）。
/// 这里把基名引用展开到真实拆分卡，避免制造未解析链接。
fn normalize_split_links(payload: &mut IngestPayload, warnings: &mut Vec<String>) {
    let mut groups: std::collections::BTreeMap<String, Vec<(usize, String)>> =
        std::collections::BTreeMap::new();
    for card in &payload.cards {
        if let Some((base, rank)) = split_suffix_rank(&card.title) {
            groups.entry(base).or_default().push((rank, card.title.clone()));
        }
    }
    for titles in groups.values_mut() {
        titles.sort_by_key(|(rank, _)| *rank);
    }
    let groups: Vec<(String, Vec<String>)> = groups
        .into_iter()
        .filter(|(_, titles)| titles.len() > 1)
        .map(|(base, titles)| (base, titles.into_iter().map(|(_, title)| title).collect()))
        .collect();
    if groups.is_empty() {
        return;
    }
    let mut replacements = 0usize;
    for card in &mut payload.cards {
        for (base, titles) in &groups {
            let pattern = format!("[[{base}]]");
            if !card.content.contains(&pattern) {
                continue;
            }
            let targets: Vec<String> = titles
                .iter()
                .filter(|title| title != &&card.title)
                .map(|title| format!("[[{title}]]"))
                .collect();
            if targets.is_empty() {
                continue;
            }
            card.content = card.content.replace(&pattern, &targets.join("、"));
            replacements += 1;
        }
    }
    for line in &mut payload.index_lines {
        for (base, titles) in &groups {
            let pattern = format!("[[{base}]]");
            if line.contains(&pattern) {
                let targets = titles
                    .iter()
                    .map(|title| format!("[[{title}]]"))
                    .collect::<Vec<_>>()
                    .join("、");
                *line = line.replace(&pattern, &targets);
                replacements += 1;
            }
        }
    }
    if replacements > 0 {
        warnings.push(format!(
            "已展开 {replacements} 处拆分卡基名链接（实际指向（上）/（中）/（下）卡片）"
        ));
    }
}

fn sources_from_frontmatter(content: &str) -> Vec<String> {
    let parsed = vault_note::parse_note(content);
    for property in &parsed.properties {
        if !property.key.eq_ignore_ascii_case("sources") {
            continue;
        }
        return match &property.value {
            vault_note::PropertyValue::List(items) => items.clone(),
            vault_note::PropertyValue::Text(text) => text
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
            _ => Vec::new(),
        };
    }
    Vec::new()
}

/// 模型偶尔漏写 `sources` 字段（但 frontmatter 有时仍在）。先用卡片自己的
/// frontmatter，再回退到本批次实际送入的文件，保证来源可追溯且不整批拒绝。
fn fill_missing_sources(payload: &mut IngestPayload, batch_sources: &[String]) {
    let valid_batch: Vec<String> = batch_sources
        .iter()
        .filter(|source| source.starts_with("inbox/") || source.starts_with("raw/"))
        .cloned()
        .collect();
    for card in &mut payload.cards {
        if !card.sources.is_empty() {
            continue;
        }
        let from_frontmatter = sources_from_frontmatter(&card.content);
        if !from_frontmatter.is_empty() {
            card.sources = from_frontmatter;
        } else if !valid_batch.is_empty() {
            card.sources = valid_batch.clone();
        }
    }
}

/// 将一个模型整理结果暂存为待审核项；校验通过后才进入审核队列。
pub fn stage(
    root: &Path,
    review_dir: &Path,
    result_text: &str,
    batch_sources: &[String],
    policy: &RulePolicy,
    utc_offset_minutes: i32,
) -> Result<ReviewItem, String> {
    if result_text.trim().is_empty() {
        return Err("整理结果为空，未进入审核。".to_string());
    }
    if result_text.len() > REVIEW_MAX_RESULT_BYTES {
        return Err(format!(
            "整理结果 {} 字节，超过 {} 字节上限。",
            result_text.len(),
            REVIEW_MAX_RESULT_BYTES
        ));
    }
    let value = parse_result_text(result_text)?;
    let mut parsed_payload: IngestPayload = serde_json::from_value(value)
        .map_err(|e| format!("整理结果不是约定的 JSON：{e}"))?;
    fill_missing_sources(&mut parsed_payload, batch_sources);
    let (mut payload, mut split_warnings) =
        auto_split_overlong(parsed_payload, crate::knowledge_ingest::CARD_MAX_CHARS);
    normalize_split_links(&mut payload, &mut split_warnings);
    let value = serde_json::to_value(&payload)
        .map_err(|e| format!("序列化整理结果失败：{e}"))?;
    let index = fs::read_to_string(root.join(knowledge_ingest::INDEX_REL))
        .map_err(|e| format!("读取知识库索引失败：{e}"))?;
    let scan = crate::knowledge_vault::scan_vault(root, None)?;
    let validation = validate_payload(&value, &index, &scan);
    if validation.cards.is_empty() {
        let problems = validation_error(&index, &scan, &value);
        return Err(format!(
            "整理结果未通过宿主校验，未进入审核：{}",
            if problems.is_empty() {
                "没有可审核的卡片".to_string()
            } else {
                problems.join("；")
            }
        ));
    }
    // 部分卡片超限/重复时，只把通过校验的卡送入审核；被拒卡片作为警告随审核项展示，
    // 避免一张长卡把整批材料全部挡住。
    let valid_sources: Vec<&str> = validation
        .cards
        .iter()
        .flat_map(|card| card.sources.iter().map(String::as_str))
        .collect();
    let archive_moves: Vec<knowledge_ingest::ArchiveMove> = validation
        .archive_moves
        .iter()
        .filter(|mv| valid_sources.iter().any(|source| *source == mv.from.as_str()))
        .cloned()
        .collect();
    let mut warnings: Vec<String> = validation.errors.clone();
    warnings.extend(split_warnings);
    for rejected in validation.rejected_moves.iter().take(5) {
        warnings.push(format!("归档 {}：{}", rejected.relative_path, rejected.reason));
    }
    let filtered = serde_json::json!({
        "cards": validation.cards.iter().map(|card| serde_json::json!({
            "relative_path": card.relative_path,
            "title": card.title,
            "content": card.content,
            "sources": card.sources,
        })).collect::<Vec<_>>(),
        "index_lines": validation.index_lines,
        "archive_moves": archive_moves,
    });
    let filtered_text = serde_json::to_string(&filtered)
        .map_err(|e| format!("序列化审核暂存结果失败：{e}"))?;
    let now = crate::task_shell::now_secs();
    let id = new_id(now);
    let record = ReviewRecord {
        schema: REVIEW_SCHEMA.to_string(),
        id: id.clone(),
        result_text: filtered_text.clone(),
        created_at: now,
        status: "pending".to_string(),
        decided_at: None,
        rejected_cards: validation.rejected_cards.clone(),
        warnings,
    };
    create_record(review_dir, &record)?;
    let item = item_from_record(&record);
    let hash = crate::sha256::sha256_hex(filtered_text.as_bytes());
    let entry = GovernanceEntry {
        action: "propose".to_string(),
        subject: format!("review/{id}"),
        sha256: hash,
        note: format!(
            "待审核：{} 张卡；跳过 {} 张",
            item.card_count,
            item.rejected_cards.len()
        ),
    };
    if let Err(e) = knowledge_governance::append_record_at(
        root,
        knowledge_governance::KIND_REVIEW_DECISIONS,
        &entry,
        policy,
        now as i64,
        utc_offset_minutes,
    ) {
        let _ = fs::remove_file(record_path(review_dir, &record.id)?);
        return Err(format!("治理提案写入失败，暂存已回滚：{e}"));
    }
    Ok(item)
}

/// 列出全部审核项（含已裁决项），损坏文件跳过并计数。
pub fn list(review_dir: &Path) -> ReviewListing {
    if !review_dir.exists() {
        return ReviewListing::default();
    }
    let mut items = Vec::new();
    let mut broken = 0u64;
    let entries = match fs::read_dir(review_dir) {
        Ok(entries) => entries,
        Err(_) => return ReviewListing { items, broken: 1 },
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                broken += 1;
                continue;
            }
        };
        let path = entry.path();
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
        let record: ReviewRecord = match serde_json::from_str::<ReviewRecord>(&text) {
            Ok(record) if record.schema == REVIEW_SCHEMA => record,
            _ => {
                broken += 1;
                continue;
            }
        };
        items.push(item_from_record(&record));
    }
    items.sort_by(|a, b| {
        b.created_at
            .partial_cmp(&a.created_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ReviewListing { items, broken }
}

fn decision_action(decision: &str) -> Result<(&'static str, &'static str), String> {
    match decision.trim().to_ascii_lowercase().as_str() {
        "approve" => Ok(("approve", "approved")),
        "request_changes" => Ok(("request_changes", "request_changes")),
        "reject" => Ok(("reject", "rejected")),
        other => Err(format!(
            "未知审核动作：{other}；只接受 approve / request_changes / reject"
        )),
    }
}

/// 对一条暂存审核项作出裁决。通过时才调用既有整理落盘链路。
pub fn decide(
    root: &Path,
    review_dir: &Path,
    id: &str,
    decision: &str,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
    app_root: Option<&Path>,
) -> Result<ReviewDecisionReport, String> {
    let mut record = load_record(review_dir, id)?;
    if record.status != "pending" {
        return Err(format!("审核项 {id} 已结算（{}），不能重复裁决。", record.status));
    }
    let (action, next_status) = decision_action(decision)?;
    let now = crate::task_shell::now_secs();
    let mut governance_errors = Vec::new();
    let mut apply = None;

    if action == "approve" {
        let scan = crate::knowledge_vault::scan_vault(root, app_root)?;
        let index = fs::read_to_string(root.join(knowledge_ingest::INDEX_REL))
            .map_err(|e| format!("读取知识库索引失败：{e}"))?;
        let value = parse_result_text(&record.result_text)?;
        let payload: IngestPayload = serde_json::from_value(value.clone())
            .map_err(|e| format!("整理结果不是约定的 JSON：{e}"))?;
        let problems = validation_error(&index, &scan, &value);
        if !problems.is_empty() {
            return Err(format!(
                "审核项 {id} 落盘前校验失败，未写入：{}",
                problems.join("；")
            ));
        }
        let report = knowledge_ingest::apply_payload(root, &value, &scan);
        if !report.ok {
            return Err(format!(
                "审核项 {id} 落盘未完成：{}",
                report.errors.join("；")
            ));
        }
        let entry = GovernanceEntry {
            action: "approve".to_string(),
            subject: format!("review/{id}"),
            sha256: crate::sha256::sha256_hex(record.result_text.as_bytes()),
            note: format!(
                "通过：新建 {} 张；归档 {} 份",
                report.created.len(),
                report.moved.len()
            ),
        };
        if let Err(e) = knowledge_governance::append_record_at(
            root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
            &entry,
            policy,
            now as i64,
            utc_offset_minutes,
        ) {
            governance_errors.push(format!("审核决策记录写入失败：{e}"));
        }
        for card in &report.created {
            let card_hash = crate::knowledge_vault::read_vault_note(root, &card.relative_path)
                .map(|note| note.sha256)
                .unwrap_or_else(|_| "-".to_string());
            let entry = GovernanceEntry {
                action: "approve_card".to_string(),
                subject: card.relative_path.clone(),
                sha256: card_hash,
                note: format!("来源 review/{id}"),
            };
            if let Err(e) = knowledge_governance::append_record_at(
                root,
                knowledge_governance::KIND_REVIEW_DECISIONS,
                &entry,
                policy,
                now as i64,
                utc_offset_minutes,
            ) {
                governance_errors.push(format!("卡片审核记录写入失败：{e}"));
            }
        }
        let raw_sources: std::collections::BTreeSet<String> = payload
            .cards
            .iter()
            .flat_map(|card| card.sources.iter().cloned())
            .filter(|source| source.starts_with("raw/"))
            .collect();
        for source in raw_sources {
            let hash = fs::read(root.join(&source))
                .map(|bytes| crate::sha256::sha256_hex(&bytes))
                .unwrap_or_default();
            let entry = GovernanceEntry {
                action: "approve_raw".to_string(),
                subject: source.clone(),
                sha256: hash,
                note: format!("来源 review/{id}"),
            };
            if let Err(e) = knowledge_governance::append_record_at(
                root,
                knowledge_governance::KIND_REVIEW_DECISIONS,
                &entry,
                policy,
                now as i64,
                utc_offset_minutes,
            ) {
                governance_errors.push(format!("raw 已处理记录写入失败（{source}）：{e}"));
            }
        }
        apply = Some(report);
    } else {
        let entry = GovernanceEntry {
            action: action.to_string(),
            subject: format!("review/{id}"),
            sha256: crate::sha256::sha256_hex(record.result_text.as_bytes()),
            note: "未写入知识库".to_string(),
        };
        if let Err(e) = knowledge_governance::append_record_at(
            root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
            &entry,
            policy,
            now as i64,
            utc_offset_minutes,
        ) {
            return Err(format!("审核决策记录写入失败，状态保持待审核：{e}"));
        }
    }

    record.status = next_status.to_string();
    record.decided_at = Some(now);
    write_record(review_dir, &record)?;
    Ok(ReviewDecisionReport {
        review: item_from_record(&record),
        apply,
        governance_errors,
    })
}

/// 冲突/异常的人工裁决记录；只写 governance，不改知识文件。
pub fn resolve_conflict(
    root: &Path,
    subject: &str,
    decision: &str,
    note: &str,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
) -> Result<GovernanceRecord, String> {
    let decision = decision.trim();
    if !matches!(decision, "resolved" | "deferred" | "needs_more") {
        return Err("冲突裁决只接受 resolved / deferred / needs_more".to_string());
    }
    if subject.trim().is_empty() {
        return Err("冲突裁决必须给出对象标识。".to_string());
    }
    let note = if note.trim().is_empty() {
        decision.to_string()
    } else {
        format!("{}；{}", decision, note.trim())
    };
    let entry = GovernanceEntry {
        action: "resolve_conflict".to_string(),
        subject: subject.trim().to_string(),
        sha256: "-".to_string(),
        note,
    };
    knowledge_governance::append_record(
        root,
        knowledge_governance::KIND_REVIEW_DECISIONS,
        &entry,
        policy,
        utc_offset_minutes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-review-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn seed_review_fixture(root: &Path) -> String {
        knowledge_ingest::init_vault(root).unwrap();
        let inbox = root.join("inbox/source.md");
        fs::create_dir_all(inbox.parent().unwrap()).unwrap();
        fs::write(&inbox, "synthetic source").unwrap();
        serde_json::to_string(&json!({
            "cards": [{
                "relative_path": "知识库/概念/审核样例.md",
                "title": "审核样例",
                "content": "---\ntags: [synthetic]\nsources: [inbox/source.md]\n---\nSynthetic fact.",
                "sources": ["inbox/source.md"]
            }],
            "index_lines": ["- [审核样例](概念/审核样例.md) | 标签：synthetic | 合成"],
            "archive_moves": [{"from": "inbox/source.md", "to": "知识库/原文/source.md"}]
        }))
        .unwrap()
    }

    #[test]
    fn stage_then_reject_updates_governance_without_writing_card() {
        let root = root("reject");
        let result = seed_review_fixture(&root);
        let review_dir = dir(&root.join("app-state"));
        let policy = RulePolicy::clean();
        let item = stage(&root, &review_dir, &result, &[], &policy, 480).unwrap();
        assert_eq!(item.status, "pending");
        assert_eq!(item.card_count, 1);
        let before = knowledge_governance::list_records(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
        )
        .unwrap();
        assert!(before.iter().any(|record| record.action == "propose"));
        let report = decide(&root, &review_dir, &item.id, "reject", &policy, 480, None).unwrap();
        assert_eq!(report.review.status, "rejected");
        assert!(report.apply.is_none());
        assert!(!root.join("知识库/概念/审核样例.md").exists());
        assert!(root.join("inbox/source.md").exists());
        let records = knowledge_governance::list_records(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
        )
        .unwrap();
        assert!(records.iter().any(|record| record.action == "reject"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn approve_applies_cards_and_resolves_review() {
        let root = root("approve");
        let result = seed_review_fixture(&root);
        let review_dir = dir(&root.join("app-state"));
        let policy = RulePolicy::clean();
        let item = stage(&root, &review_dir, &result, &[], &policy, 480).unwrap();
        let report = decide(&root, &review_dir, &item.id, "approve", &policy, 480, None).unwrap();
        assert_eq!(report.review.status, "approved");
        let apply = report.apply.unwrap();
        assert!(apply.ok);
        assert!(root.join("知识库/概念/审核样例.md").is_file());
        assert!(!root.join("inbox/source.md").exists());
        assert!(knowledge_ingest::smoke_search(&root, "Synthetic").unwrap() >= 1);
        let records = knowledge_governance::list_records(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
        )
        .unwrap();
        assert!(records.iter().any(|record| record.action == "approve_card"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_auto_splits_moderately_overlong_card() {
        let root = root("partial-stage");
        let base = seed_review_fixture(&root);
        let mut value: serde_json::Value = serde_json::from_str(&base).unwrap();
        value["cards"].as_array_mut().unwrap().push(serde_json::json!({
            "relative_path": "知识库/方法/长卡.md",
            "title": "长卡",
            "content": format!("---\ntags: [synthetic]\nsources: [inbox/source.md]\n---\n{}", "长。".repeat(224)),
            "sources": ["inbox/source.md"]
        }));
        value["index_lines"].as_array_mut().unwrap().push(serde_json::json!(
            "- [长卡](方法/长卡.md) | 标签：synthetic | 长"
        ));
        let result = serde_json::to_string(&value).unwrap();
        let review_dir = dir(&root.join("app-state"));
        let policy = RulePolicy::clean();
        let item = stage(&root, &review_dir, &result, &[], &policy, 480).unwrap();
        assert_eq!(item.card_count, 3, "1 张正常卡 + 长卡拆成 2 张");
        assert!(item.rejected_cards.is_empty(), "{:?}", item.rejected_cards);
        assert!(item.warnings.iter().any(|warning| warning.contains("宿主自动拆分")));
        assert!(item.cards.iter().any(|card| card.title == "长卡（上）"));
        assert!(item.cards.iter().any(|card| card.title == "长卡（下）"));
        assert!(item
            .cards
            .iter()
            .filter(|card| card.title.starts_with("长卡"))
            .all(|card| crate::vault_note::parse_note(&card.content).body.chars().count()
                <= crate::knowledge_ingest::CARD_MAX_CHARS));
        assert!(item.cards.iter().all(|card| card.sources == vec!["inbox/source.md"]));
        let report = decide(&root, &review_dir, &item.id, "approve", &policy, 480, None).unwrap();
        assert_eq!(report.review.status, "approved");
        assert!(root.join("知识库/概念/审核样例.md").is_file());
        assert!(root.join("知识库/方法/长卡（上）.md").is_file());
        assert!(root.join("知识库/方法/长卡（下）.md").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_rejects_all_invalid_cards() {
        let root = root("all-invalid");
        let base = seed_review_fixture(&root);
        let mut value: serde_json::Value = serde_json::from_str(&base).unwrap();
        value["cards"][0]["content"] = serde_json::json!(format!(
            "---\ntags: [synthetic]\nsources: [inbox/source.md]\n---\n{}",
            "长".repeat(crate::knowledge_ingest::CARD_MAX_CHARS * 4)
        ));
        let result = serde_json::to_string(&value).unwrap();
        let review_dir = dir(&root.join("app-state"));
        assert!(stage(&root, &review_dir, &result, &[], &RulePolicy::clean(), 480).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn approve_records_processed_raw_source() {
        let root = root("processed-source");
        knowledge_ingest::init_vault(&root).unwrap();
        fs::write(root.join("raw/source.md"), b"raw source").unwrap();
        let result = serde_json::to_string(&serde_json::json!({
            "cards": [{
                "relative_path": "知识库/方法/raw 来源样例.md",
                "title": "raw 来源样例",
                "content": "---\ntags: [synthetic]\nsources: [raw/source.md]\n---\nSynthetic raw source.",
                "sources": ["raw/source.md"]
            }],
            "index_lines": ["- [raw 来源样例](方法/raw 来源样例.md) | 标签：synthetic | 合成"],
            "archive_moves": []
        })).unwrap();
        let review_dir = dir(&root.join("app-state"));
        let policy = RulePolicy::clean();
        let item = stage(&root, &review_dir, &result, &[], &policy, 480).unwrap();
        let report = decide(&root, &review_dir, &item.id, "approve", &policy, 480, None).unwrap();
        assert_eq!(report.review.status, "approved");
        let records = knowledge_governance::list_records(
            &root,
            knowledge_governance::KIND_REVIEW_DECISIONS,
        )
        .unwrap();
        let processed = records
            .iter()
            .find(|record| record.action == "approve_raw" && record.subject == "raw/source.md")
            .expect("approve_raw 必须记录 raw 来源");
        assert_eq!(
            processed.sha256,
            crate::sha256::sha256_hex(b"raw source")[..12]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_fills_batch_source_when_model_omits_sources() {
        let root = root("fill-sources");
        knowledge_ingest::init_vault(&root).unwrap();
        fs::write(root.join("raw/source.md"), b"raw source").unwrap();
        let result = serde_json::to_string(&serde_json::json!({
            "cards": [{
                "relative_path": "知识库/概念/漏写来源.md",
                "title": "漏写来源",
                "content": "---\ntags: [synthetic]\n---\nSynthetic.",
                "sources": []
            }],
            "index_lines": ["- [漏写来源](概念/漏写来源.md) | 标签：synthetic | 合成"],
            "archive_moves": []
        })).unwrap();
        let review_dir = dir(&root.join("app-state"));
        let batch = vec!["raw/source.md".to_string()];
        let item = stage(&root, &review_dir, &result, &batch, &RulePolicy::clean(), 480).unwrap();
        assert_eq!(item.cards[0].sources, vec!["raw/source.md".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn split_base_links_expand_to_real_split_cards() {
        let mut payload = IngestPayload {
            cards: vec![
                IngestCard {
                    relative_path: "知识库/方法/上下文（上）.md".to_string(),
                    title: "上下文（上）".to_string(),
                    content: "合成上".to_string(),
                    sources: vec!["raw/a.md".to_string()],
                },
                IngestCard {
                    relative_path: "知识库/方法/上下文（下）.md".to_string(),
                    title: "上下文（下）".to_string(),
                    content: "合成下".to_string(),
                    sources: vec!["raw/a.md".to_string()],
                },
                IngestCard {
                    relative_path: "知识库/方法/推理（上）.md".to_string(),
                    title: "推理（上）".to_string(),
                    content: "参见 [[上下文]]".to_string(),
                    sources: vec!["raw/b.md".to_string()],
                },
            ],
            index_lines: vec!["- [推理（上）](方法/推理（上）.md) | 参见：[[上下文]]".to_string()],
            archive_moves: Vec::new(),
        };
        let mut warnings = Vec::new();
        normalize_split_links(&mut payload, &mut warnings);
        assert!(!payload.cards[2].content.contains("[[上下文]]"));
        assert!(payload.cards[2].content.contains("[[上下文（上）]]"));
        assert!(payload.cards[2].content.contains("[[上下文（下）]]"));
        assert!(!payload.index_lines[0].contains("[[上下文]]"));
        assert!(payload.index_lines[0].contains("[[上下文（上）]]"));
        assert!(warnings.iter().any(|warning| warning.contains("拆分卡基名链接")));
    }

    #[test]
    fn broken_review_file_is_reported_not_fatal() {
        let root = root("broken");
        let review_dir = dir(&root.join("app-state"));
        fs::create_dir_all(&review_dir).unwrap();
        fs::write(review_dir.join("broken.json"), "{not-json").unwrap();
        let listing = list(&review_dir);
        assert_eq!(listing.items.len(), 0);
        assert_eq!(listing.broken, 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn conflict_resolution_appends_governance_record() {
        let root = root("conflict");
        knowledge_ingest::init_vault(&root).unwrap();
        let policy = RulePolicy::clean();
        let record = resolve_conflict(
            &root,
            "conflict:duplicate:sample.md",
            "resolved",
            "保留左侧来源",
            &policy,
            480,
        )
        .unwrap();
        assert_eq!(record.action, "resolve_conflict");
        assert!(record.note.contains("resolved"));
        let _ = fs::remove_dir_all(&root);
    }
}
