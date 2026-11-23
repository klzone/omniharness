//! 每日习惯候选的宿主解析与校验（OD-GROW-301 G3）。
//!
//! Python 只负责组装提示词；这里解析模型返回的文本，逐条核对来源消息、
//! 信号词、作用域、长度、红线、重复与驳回记录。被拒条目只保留原因编号，
//! 不把候选正文带回错误或日志。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::consolidation::DigestPlan;
use crate::habit_cards::{
    rejection_blocks, HabitCard, HabitCheck, HabitEvidence, HabitScope, OpenQuestion,
    RejectionMemory, HABIT_CARD_SCHEMA, HABIT_TEXT_MAX_CHARS, KIND_FORMAT, KIND_PREFERENCE,
    KIND_RULE, OPEN_QUESTION_SCHEMA, OPEN_QUESTION_TEXT_MAX_CHARS, ORIGIN_EXPLICIT,
    ORIGIN_REPEATED, QUESTION_OPEN, STATUS_CANDIDATE,
};
use crate::knowledge_ingest;
use crate::profile_rules::RulePolicy;
use crate::sha256::sha256_hex;
use crate::task_shell::utc_iso8601;
use crate::task_store::{CandidateVerdict, VerdictCheck};

pub const HABIT_CANDIDATES_SCHEMA: &str = "omnidesk.habit-candidates.v1";
pub const HABIT_CANDIDATE_VALIDATOR: &str = "habit-candidate.v1";
pub const MAX_RESULT_BYTES: usize = 1024 * 1024;
pub const MAX_ITEMS: usize = 10;
pub const MAX_OPEN_QUESTIONS: usize = 10;
pub const CHECK_PHRASE_MAX_CHARS: usize = 20;
pub const CHECK_LIMIT_MIN: u64 = 10;
pub const CHECK_LIMIT_MAX: u64 = 2_000;

/// 明确长期要求的信号词；cue 必须整体等于其中一项。
pub const SIGNAL_WORDS: &[&str] = &[
    "以后",
    "一律",
    "总是",
    "从现在起",
    "记住",
    "别再",
    "不要再",
    "我习惯",
    "我喜欢",
    "我不喜欢",
    "以后都用",
    "每次都",
];

/// 未决问题必须包含的表达。
pub const UNCERTAIN_WORDS: &[&str] = &[
    "还没想好",
    "待定",
    "拿不准",
    "要不要",
    "再说",
    "回头再定",
    "没定",
];

/// 只有来源用户消息明确出现这些词时，global 作用域才有原文依据。
pub const GLOBAL_SCOPE_WORDS: &[&str] = &["所有", "一律", "任何"];

/// 明显是假设、转述或未采纳建议的标记。宿主只拒绝能明确看出的表达，
/// 不从语气或话题做语义猜测。
pub const NON_FACTUAL_MARKERS: &[&str] = &[
    "假设",
    "假如",
    "如果",
    "要是",
    "同事说",
    "听说",
    "据说",
    "转述",
    "AI 建议",
    "AI建议",
    "建议我",
    "我认为",
    "我觉得",
    "结论是",
];

pub const REASON_INVALID_SHAPE: &str = "INVALID_SHAPE";
pub const REASON_INVALID_KIND: &str = "INVALID_KIND";
pub const REASON_SCOPE_INVALID: &str = "SCOPE_INVALID";
pub const REASON_MESSAGE_NOT_FOUND: &str = "MESSAGE_NOT_FOUND";
pub const REASON_MESSAGE_NOT_USER: &str = "MESSAGE_NOT_USER";
pub const REASON_CUE_NOT_SUBSTRING: &str = "CUE_NOT_SUBSTRING";
pub const REASON_NO_SIGNAL: &str = "NO_SIGNAL";
pub const REASON_EMPTY_TEXT: &str = "EMPTY_TEXT";
pub const REASON_CONTROL_CHAR: &str = "CONTROL_CHAR";
pub const REASON_TEXT_TOO_LONG: &str = "TEXT_TOO_LONG";
pub const REASON_CHECK_INVALID: &str = "CHECK_INVALID";
pub const REASON_REDLINE: &str = "REDLINE";
pub const REASON_DUPLICATE: &str = "DUPLICATE";
pub const REASON_REJECTED_BEFORE: &str = "REJECTED_BEFORE";
pub const REASON_GLOBAL_DOWNGRADED: &str = "GLOBAL_DOWNGRADED";
pub const REASON_OVERFLOW: &str = "OVERFLOW";
pub const REASON_QUESTION_INVALID_SHAPE: &str = "QUESTION_INVALID_SHAPE";
pub const REASON_QUESTION_MESSAGE_NOT_FOUND: &str = "QUESTION_MESSAGE_NOT_FOUND";
pub const REASON_QUESTION_MESSAGE_NOT_USER: &str = "QUESTION_MESSAGE_NOT_USER";
pub const REASON_QUESTION_EMPTY_TEXT: &str = "QUESTION_EMPTY_TEXT";
pub const REASON_QUESTION_CONTROL_CHAR: &str = "QUESTION_CONTROL_CHAR";
pub const REASON_QUESTION_TEXT_TOO_LONG: &str = "QUESTION_TEXT_TOO_LONG";
pub const REASON_QUESTION_REDLINE: &str = "QUESTION_REDLINE";
pub const REASON_NO_UNCERTAIN: &str = "NO_UNCERTAIN";

/// 单条摘要消息的只读视图；role 用于宿主复核，实际 G2 摘要只收用户消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestMessageRef<'a> {
    pub id: &'a str,
    pub role: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestThreadRef<'a> {
    pub thread_id: &'a str,
    pub messages: Vec<DigestMessageRef<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestPlanRef<'a> {
    pub local_date: &'a str,
    pub threads: Vec<DigestThreadRef<'a>>,
}

impl<'a> DigestPlanRef<'a> {
    /// G2 的 DigestPlan 只包含用户消息，因此转换时固定标成 user。
    pub fn from_plan(plan: &'a DigestPlan) -> Self {
        Self {
            local_date: &plan.local_date,
            threads: plan
                .threads
                .iter()
                .map(|thread| DigestThreadRef {
                    thread_id: &thread.thread_id,
                    messages: thread
                        .messages
                        .iter()
                        .map(|message| DigestMessageRef {
                            id: &message.id,
                            role: "user",
                            text: &message.text,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// 去重只读取既有候选/生效卡文本，以及宿主确认过的两天以上重复文本。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingCardsRef {
    pub card_texts: Vec<String>,
    pub repeated_texts: BTreeMap<String, usize>,
}

impl ExistingCardsRef {
    pub fn from_cards(cards: &[HabitCard], repeated_texts: BTreeMap<String, usize>) -> Self {
        let card_texts = cards
            .iter()
            .filter(|card| {
                matches!(
                    card.status.as_str(),
                    crate::habit_cards::STATUS_CANDIDATE
                        | crate::habit_cards::STATUS_ACTIVE
                )
            })
            .map(|card| card.text.trim().to_string())
            .collect();
        Self {
            card_texts,
            repeated_texts,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RejectionRef {
    pub memory: RejectionMemory,
}

impl From<&RejectionMemory> for RejectionRef {
    fn from(memory: &RejectionMemory) -> Self {
        Self {
            memory: memory.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RejectedEntry {
    pub reason_code: String,
}

impl RejectedEntry {
    fn new(reason_code: &str) -> Self {
        Self {
            reason_code: reason_code.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StagedHabitCandidate {
    pub schema: String,
    pub id: String,
    pub kind: String,
    pub scope: HabitScope,
    pub text: String,
    pub check: Option<HabitCheck>,
    pub evidence: HabitEvidence,
    pub origin: String,
    pub status: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub reason_codes: Vec<String>,
    pub verdict: CandidateVerdict,
}

impl StagedHabitCandidate {
    /// 转成 G1 的卡片结构；确认时间和 expect 由后续用户决定流程填写。
    pub fn into_card(self) -> HabitCard {
        HabitCard {
            schema: self.schema,
            id: self.id,
            kind: self.kind,
            scope: self.scope,
            text: self.text,
            check: self.check,
            evidence: self.evidence,
            origin: self.origin,
            status: self.status,
            created_at: self.created_at,
            confirmed_at: None,
            last_seen_at: self.last_seen_at,
            expect: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StagedOpenQuestion {
    pub schema: String,
    pub id: String,
    pub text: String,
    pub thread_id: String,
    pub message_id: String,
    pub first_seen: String,
    pub last_seen: String,
    pub status: String,
    pub reminded: u64,
}

impl StagedOpenQuestion {
    pub fn into_question(self) -> OpenQuestion {
        OpenQuestion {
            schema: self.schema,
            id: self.id,
            text: self.text,
            thread_id: self.thread_id,
            message_id: self.message_id,
            first_seen: self.first_seen,
            last_seen: self.last_seen,
            status: self.status,
            reminded: self.reminded,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitParseResult {
    pub candidates: Vec<StagedHabitCandidate>,
    pub open_questions: Vec<StagedOpenQuestion>,
    pub rejected: Vec<RejectedEntry>,
    pub verdicts: Vec<CandidateVerdict>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    text: String,
    kind: String,
    scope_hint: String,
    message_id: String,
    cue: String,
    #[serde(default)]
    check: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOpenQuestion {
    text: String,
    message_id: String,
}

/// 解析并校验模型输出。JSON 形状错误返回明确错误；单条候选错误进入 rejected。
pub fn parse_habit_output(
    raw: &str,
    digest: &DigestPlanRef<'_>,
    policy: &RulePolicy,
    existing: &ExistingCardsRef,
    rejection: &RejectionRef,
    now: f64,
) -> Result<HabitParseResult, String> {
    let (items, questions) = parse_top_level(raw)?;
    let mut result = HabitParseResult {
        candidates: Vec::new(),
        open_questions: Vec::new(),
        rejected: Vec::new(),
        verdicts: Vec::new(),
    };

    for (index, value) in items.into_iter().enumerate() {
        if index >= MAX_ITEMS {
            result.rejected.push(RejectedEntry::new(REASON_OVERFLOW));
            continue;
        }
        match validate_item(value, digest, policy, existing, rejection, now) {
            Ok(candidate) => {
                result.verdicts.push(candidate.verdict.clone());
                result.candidates.push(candidate);
            }
            Err(reason_code) => result.rejected.push(RejectedEntry::new(reason_code)),
        }
    }

    for (index, value) in questions.into_iter().enumerate() {
        if index >= MAX_OPEN_QUESTIONS {
            result.rejected.push(RejectedEntry::new(REASON_OVERFLOW));
            continue;
        }
        match validate_open_question(value, digest, policy, now) {
            Ok(question) => result.open_questions.push(question),
            Err(reason_code) => result.rejected.push(RejectedEntry::new(reason_code)),
        }
    }

    Ok(result)
}

fn parse_top_level(raw: &str) -> Result<(Vec<Value>, Vec<Value>), String> {
    if raw.trim().is_empty() {
        return Err("习惯候选结果为空。".to_string());
    }
    if raw.len() > MAX_RESULT_BYTES {
        return Err(format!(
            "习惯候选结果 {} 字节，超过 {} 字节上限。",
            raw.len(),
            MAX_RESULT_BYTES
        ));
    }
    let value = knowledge_ingest::parse_result_text(raw)
        .map_err(|error| format!("习惯候选结果不是合法 JSON：{error}"))?;
    let mut object = match value {
        Value::Object(object) => object,
        _ => return Err("习惯候选结果顶层必须是 JSON 对象。".to_string()),
    };
    reject_unknown_top_level_fields(&object)?;
    let items = take_array(&mut object, "items")?;
    let questions = take_array(&mut object, "open_questions")?;
    Ok((items, questions))
}

fn reject_unknown_top_level_fields(object: &Map<String, Value>) -> Result<(), String> {
    for key in object.keys() {
        if key != "items" && key != "open_questions" {
            return Err(format!("习惯候选结果包含未知字段：{key}"));
        }
    }
    if !object.contains_key("items") {
        return Err("习惯候选结果缺少 items 数组。".to_string());
    }
    if !object.contains_key("open_questions") {
        return Err("习惯候选结果缺少 open_questions 数组。".to_string());
    }
    Ok(())
}

fn take_array(object: &mut Map<String, Value>, field: &str) -> Result<Vec<Value>, String> {
    match object.remove(field) {
        Some(Value::Array(items)) => Ok(items),
        _ => Err(format!("习惯候选结果的 {field} 必须是数组。")),
    }
}

fn validate_item(
    value: Value,
    digest: &DigestPlanRef<'_>,
    policy: &RulePolicy,
    existing: &ExistingCardsRef,
    rejection: &RejectionRef,
    now: f64,
) -> Result<StagedHabitCandidate, &'static str> {
    let item: RawItem = serde_json::from_value(value).map_err(|_| REASON_INVALID_SHAPE)?;
    if !matches!(item.kind.trim(), KIND_RULE | KIND_PREFERENCE | KIND_FORMAT) {
        return Err(REASON_INVALID_KIND);
    }
    if !matches!(item.scope_hint.trim(), "scene" | "global") {
        return Err(REASON_SCOPE_INVALID);
    }

    let message_id = item.message_id.trim();
    let (thread, message) = find_message(digest, message_id).ok_or(REASON_MESSAGE_NOT_FOUND)?;
    if message.role != "user" {
        return Err(REASON_MESSAGE_NOT_USER);
    }

    let cue = item.cue.trim();
    if cue.is_empty() || !message.text.contains(cue) {
        return Err(REASON_CUE_NOT_SUBSTRING);
    }
    let signal_whitelisted = SIGNAL_WORDS.contains(&cue);

    let text = item.text.trim().to_string();
    if text.is_empty() {
        return Err(REASON_EMPTY_TEXT);
    }
    if text.chars().any(char::is_control) {
        return Err(REASON_CONTROL_CHAR);
    }
    if text.chars().count() > HABIT_TEXT_MAX_CHARS {
        return Err(REASON_TEXT_TOO_LONG);
    }

    let check = parse_check(item.check).map_err(|_| REASON_CHECK_INVALID)?;
    if contains_reject_keyword(&text, policy)
        || check
            .as_ref()
            .map(|value| check_hits_redline(value, policy))
            .unwrap_or(false)
    {
        return Err(REASON_REDLINE);
    }
    if contains_non_factual_marker(&text) {
        return Err(REASON_NO_SIGNAL);
    }

    let repeated_days = existing
        .repeated_texts
        .get(text.as_str())
        .copied()
        .unwrap_or(0);
    if !signal_whitelisted && repeated_days < 2 {
        return Err(REASON_NO_SIGNAL);
    }

    if existing.card_texts.iter().any(|known| known == &text) {
        return Err(REASON_DUPLICATE);
    }
    if rejection_blocks(&rejection.memory, &text) {
        return Err(REASON_REJECTED_BEFORE);
    }

    let mut reason_codes = Vec::new();
    let scope_valid;
    let scope = match item.scope_hint.trim() {
        "global" if GLOBAL_SCOPE_WORDS.iter().any(|word| message.text.contains(word)) => {
            scope_valid = true;
            HabitScope::Global
        }
        "global" => {
            scope_valid = false;
            reason_codes.push(REASON_GLOBAL_DOWNGRADED.to_string());
            HabitScope::Scene {
                scene: thread.thread_id.to_string(),
            }
        }
        _ => {
            scope_valid = true;
            HabitScope::Scene {
                scene: thread.thread_id.to_string(),
            }
        }
    };

    let origin = if signal_whitelisted {
        ORIGIN_EXPLICIT
    } else {
        ORIGIN_REPEATED
    };
    let id = staged_id("habit", thread.thread_id, message_id, cue, &text);
    let checks = vec![
        VerdictCheck {
            id: "cue_in_source".to_string(),
            pass: true,
        },
        VerdictCheck {
            id: "signal_whitelisted".to_string(),
            pass: signal_whitelisted,
        },
        VerdictCheck {
            id: "scope_valid".to_string(),
            pass: scope_valid,
        },
        VerdictCheck {
            id: "not_duplicate".to_string(),
            pass: true,
        },
        VerdictCheck {
            id: "redline_clean".to_string(),
            pass: true,
        },
    ];
    let verdict = CandidateVerdict::new(HABIT_CANDIDATE_VALIDATOR, checks);
    let timestamp = utc_iso8601(now);
    Ok(StagedHabitCandidate {
        schema: HABIT_CARD_SCHEMA.to_string(),
        id,
        kind: item.kind.trim().to_string(),
        scope,
        text,
        check,
        evidence: HabitEvidence {
            thread_id: thread.thread_id.to_string(),
            message_id: message_id.to_string(),
            date: digest.local_date.to_string(),
        },
        origin: origin.to_string(),
        status: STATUS_CANDIDATE.to_string(),
        created_at: timestamp.clone(),
        last_seen_at: timestamp,
        reason_codes,
        verdict,
    })
}

fn validate_open_question(
    value: Value,
    digest: &DigestPlanRef<'_>,
    policy: &RulePolicy,
    now: f64,
) -> Result<StagedOpenQuestion, &'static str> {
    let item: RawOpenQuestion =
        serde_json::from_value(value).map_err(|_| REASON_QUESTION_INVALID_SHAPE)?;
    let message_id = item.message_id.trim();
    let (thread, message) =
        find_message(digest, message_id).ok_or(REASON_QUESTION_MESSAGE_NOT_FOUND)?;
    if message.role != "user" {
        return Err(REASON_QUESTION_MESSAGE_NOT_USER);
    }
    let text = item.text.trim().to_string();
    if text.is_empty() {
        return Err(REASON_QUESTION_EMPTY_TEXT);
    }
    if text.chars().any(char::is_control) {
        return Err(REASON_QUESTION_CONTROL_CHAR);
    }
    if text.chars().count() > OPEN_QUESTION_TEXT_MAX_CHARS {
        return Err(REASON_QUESTION_TEXT_TOO_LONG);
    }
    if contains_reject_keyword(&text, policy) {
        return Err(REASON_QUESTION_REDLINE);
    }
    if !UNCERTAIN_WORDS.iter().any(|word| text.contains(word)) {
        return Err(REASON_NO_UNCERTAIN);
    }
    let id = staged_id("question", thread.thread_id, message_id, "", &text);
    let timestamp = utc_iso8601(now);
    Ok(StagedOpenQuestion {
        schema: OPEN_QUESTION_SCHEMA.to_string(),
        id,
        text,
        thread_id: thread.thread_id.to_string(),
        message_id: message_id.to_string(),
        first_seen: timestamp.clone(),
        last_seen: timestamp,
        status: QUESTION_OPEN.to_string(),
        reminded: 0,
    })
}

fn find_message<'a, 's>(
    digest: &'a DigestPlanRef<'s>,
    message_id: &str,
) -> Option<(&'a DigestThreadRef<'s>, &'a DigestMessageRef<'s>)> {
    digest.threads.iter().find_map(|thread| {
        thread
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .map(|message| (thread, message))
    })
}

fn staged_id(prefix: &str, thread_id: &str, message_id: &str, cue: &str, text: &str) -> String {
    let hash = sha256_hex(format!("{thread_id}\n{message_id}\n{cue}\n{text}").as_bytes());
    format!("{prefix}-{}", &hash[..40])
}

fn parse_check(value: Option<Value>) -> Result<Option<HabitCheck>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    let object = value.as_object().ok_or(())?;
    let check_type = object.get("type").and_then(Value::as_str).ok_or(())?;
    match check_type {
        "banned_phrases" | "required_phrases" => {
            if object.len() != 2 || !object.contains_key("phrases") {
                return Err(());
            }
            let phrases = parse_check_phrases(object.get("phrases"))?;
            if check_type == "banned_phrases" {
                Ok(Some(HabitCheck::BannedPhrases { phrases }))
            } else {
                Ok(Some(HabitCheck::RequiredPhrases { phrases }))
            }
        }
        "paragraph_max_chars" => {
            if object.len() != 2 || !object.contains_key("limit") {
                return Err(());
            }
            let limit = object.get("limit").and_then(Value::as_u64).ok_or(())?;
            if !(CHECK_LIMIT_MIN..=CHECK_LIMIT_MAX).contains(&limit) {
                return Err(());
            }
            Ok(Some(HabitCheck::ParagraphMaxChars { limit }))
        }
        _ => Err(()),
    }
}

fn parse_check_phrases(value: Option<&Value>) -> Result<Vec<String>, ()> {
    let values = value.and_then(Value::as_array).ok_or(())?;
    if values.is_empty() {
        return Err(());
    }
    let mut phrases = Vec::new();
    for value in values {
        let phrase = value.as_str().ok_or(())?.trim();
        if phrase.is_empty()
            || phrase.chars().count() > CHECK_PHRASE_MAX_CHARS
            || phrase.chars().any(char::is_control)
        {
            return Err(());
        }
        phrases.push(phrase.to_string());
    }
    Ok(phrases)
}

fn check_hits_redline(check: &HabitCheck, policy: &RulePolicy) -> bool {
    match check {
        HabitCheck::BannedPhrases { phrases } | HabitCheck::RequiredPhrases { phrases } => {
            phrases
                .iter()
                .any(|phrase| contains_reject_keyword(phrase, policy))
        }
        HabitCheck::ParagraphMaxChars { .. } => false,
    }
}

fn contains_non_factual_marker(text: &str) -> bool {
    NON_FACTUAL_MARKERS.iter().any(|marker| text.contains(marker))
}

fn contains_reject_keyword(text: &str, policy: &RulePolicy) -> bool {
    policy
        .reject_keywords()
        .iter()
        .any(|keyword| !keyword.is_empty() && text.contains(keyword.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habit_cards::{
        normalize_rejection_phrase_hash, RejectionMemory, REJECTION_MEMORY_SCHEMA,
    };
    use crate::profile_rules::ProfileRules;
    use serde_json::json;
    use std::collections::BTreeMap;

    const NOW: f64 = 1_800_000_000.0;
    const LOCAL_DATE: &str = "2026-11-23";

    struct FixtureDigest {
        local_date: String,
        threads: Vec<FixtureThread>,
    }

    struct FixtureThread {
        thread_id: String,
        messages: Vec<(String, String, String)>,
    }

    impl FixtureDigest {
        fn standard() -> Self {
            Self {
                local_date: LOCAL_DATE.to_string(),
                threads: vec![
                    FixtureThread {
                        thread_id: "thread-1".to_string(),
                        messages: vec![
                            ("m-1".to_string(), "user".to_string(), "以后都用清单收尾，别再临时口头确认。".to_string()),
                            ("m-2".to_string(), "user".to_string(), "这次先按模板整理一下。".to_string()),
                            ("m-3".to_string(), "user".to_string(), "假设我每周总是提前交付吧。".to_string()),
                            ("m-4".to_string(), "user".to_string(), "同事说以后都用表格汇报。".to_string()),
                            ("m-5".to_string(), "user".to_string(), "AI 建议我以后都用语音记录。".to_string()),
                            ("m-6".to_string(), "user".to_string(), "以后所有报告都用同一模板。".to_string()),
                            ("m-7".to_string(), "user".to_string(), "按模板收尾就好。".to_string()),
                            ("m-8".to_string(), "user".to_string(), "以后处理机密清单。".to_string()),
                            ("m-9".to_string(), "user".to_string(), "这套方案还没想好要不要做。".to_string()),
                            ("m-10".to_string(), "user".to_string(), "以后先把工具清单列完。".to_string()),
                            ("m-asst".to_string(), "assistant".to_string(), "以后都用清单收尾。".to_string()),
                        ],
                    },
                ],
            }
        }

        fn as_ref(&self) -> DigestPlanRef<'_> {
            DigestPlanRef {
                local_date: &self.local_date,
                threads: self
                    .threads
                    .iter()
                    .map(|thread| DigestThreadRef {
                        thread_id: &thread.thread_id,
                        messages: thread
                            .messages
                            .iter()
                            .map(|(id, role, text)| DigestMessageRef {
                                id,
                                role,
                                text,
                            })
                            .collect(),
                    })
                    .collect(),
            }
        }
    }

    fn policy(redline: Option<&str>) -> RulePolicy {
        let keywords = redline.into_iter().collect::<Vec<_>>();
        let rules: ProfileRules = serde_json::from_value(json!({
            "schema": "omnidesk.profile-rules.v1",
            "profile": "synthetic-habit-extract",
            "global": {"redline_keywords": keywords}
        }))
        .expect("合成规则应可解析");
        RulePolicy::from_rules(&rules)
    }

    fn rejection_memory(texts: &[&str]) -> RejectionRef {
        let mut memory = RejectionMemory {
            schema: REJECTION_MEMORY_SCHEMA.to_string(),
            entries: BTreeMap::new(),
        };
        for text in texts {
            memory
                .entries
                .insert(normalize_rejection_phrase_hash(text), 2);
        }
        RejectionRef { memory }
    }

    fn existing(cards: &[&str], repeated: &[(&str, usize)]) -> ExistingCardsRef {
        ExistingCardsRef {
            card_texts: cards.iter().map(|text| text.to_string()).collect(),
            repeated_texts: repeated
                .iter()
                .map(|(text, days)| (text.to_string(), *days))
                .collect(),
        }
    }

    fn item(text: &str, kind: &str, scope: &str, message_id: &str, cue: &str, check: Value) -> Value {
        json!({
            "text": text,
            "kind": kind,
            "scope_hint": scope,
            "message_id": message_id,
            "cue": cue,
            "check": check,
        })
    }

    fn payload(items: Vec<Value>, questions: Vec<Value>) -> String {
        json!({"items": items, "open_questions": questions}).to_string()
    }

    fn raw_item(text: &str, scope: &str, message_id: &str, cue: &str) -> Value {
        item(text, KIND_PREFERENCE, scope, message_id, cue, Value::Null)
    }

    #[derive(Clone)]
    enum ExpectedScope {
        Scene,
        Global,
    }

    struct Case {
        name: &'static str,
        raw: String,
        redline: Option<&'static str>,
        existing: ExistingCardsRef,
        rejection: RejectionRef,
        candidates: usize,
        questions: usize,
        rejections: Vec<&'static str>,
        expected_text: Option<&'static str>,
        expected_scope: Option<ExpectedScope>,
        expected_origin: Option<&'static str>,
        expected_reason: Option<&'static str>,
        verdict_pass: Option<bool>,
    }

    fn case(
        name: &'static str,
        raw: String,
        candidates: usize,
        rejection: &'static str,
    ) -> Case {
        Case {
            name,
            raw,
            redline: None,
            existing: ExistingCardsRef::default(),
            rejection: RejectionRef::default(),
            candidates,
            questions: 0,
            rejections: if rejection.is_empty() {
                Vec::new()
            } else {
                vec![rejection]
            },
            expected_text: None,
            expected_scope: None,
            expected_origin: None,
            expected_reason: None,
            verdict_pass: None,
        }
    }

    #[test]
    fn table_driven_host_validation_has_at_least_twenty_cases() {
        let digest = FixtureDigest::standard();
        let mut overflow_items = Vec::new();
        for index in 0..12 {
            overflow_items.push(raw_item(
                &format!("习惯文本{index}"),
                "scene",
                "m-1",
                "以后",
            ));
        }

        let mut cases = vec![
            {
                let mut value = case(
                    "明确表述通过",
                    payload(vec![raw_item("以后都用清单收尾", "scene", "m-1", "以后")], vec![]),
                    1,
                    "",
                );
                value.expected_text = Some("以后都用清单收尾");
                value.expected_scope = Some(ExpectedScope::Scene);
                value.expected_origin = Some(ORIGIN_EXPLICIT);
                value.verdict_pass = Some(true);
                value
            },
            case(
                "一次性请求被拒",
                payload(vec![raw_item("按模板整理", "scene", "m-2", "这次")], vec![]),
                0,
                REASON_NO_SIGNAL,
            ),
            case(
                "假设类语句被拒",
                payload(vec![raw_item("假设我每周提前交付", "scene", "m-3", "总是")], vec![]),
                0,
                REASON_NO_SIGNAL,
            ),
            case(
                "转述被拒",
                payload(vec![raw_item("同事说以后都用表格", "scene", "m-4", "以后")], vec![]),
                0,
                REASON_NO_SIGNAL,
            ),
            case(
                "AI 建议未采纳被拒",
                payload(vec![raw_item("AI 建议我用语音记录", "scene", "m-5", "以后")], vec![]),
                0,
                REASON_NO_SIGNAL,
            ),
            {
                let mut value = case(
                    "编号不存在被拒",
                    payload(vec![raw_item("不存在的来源", "scene", "m-404", "以后")], vec![]),
                    0,
                    REASON_MESSAGE_NOT_FOUND,
                );
                value.expected_text = None;
                value
            },
            case(
                "信号词不在原文被拒",
                payload(vec![raw_item("无来源信号", "scene", "m-2", "以后")], vec![]),
                0,
                REASON_CUE_NOT_SUBSTRING,
            ),
            {
                let mut value = case(
                    "global 无依据降级",
                    payload(vec![raw_item("以后都用清单收尾", "global", "m-1", "以后")], vec![]),
                    1,
                    "",
                );
                value.expected_scope = Some(ExpectedScope::Scene);
                value.expected_reason = Some(REASON_GLOBAL_DOWNGRADED);
                value.verdict_pass = Some(false);
                value
            },
            {
                let mut value = case(
                    "global 有原文依据",
                    payload(vec![raw_item("所有报告用同一模板", "global", "m-6", "以后")], vec![]),
                    1,
                    "",
                );
                value.expected_scope = Some(ExpectedScope::Global);
                value.verdict_pass = Some(true);
                value
            },
            {
                let mut value = case(
                    "重复候选被拒",
                    payload(vec![raw_item("以后都用清单收尾", "scene", "m-1", "以后")], vec![]),
                    0,
                    REASON_DUPLICATE,
                );
                value.existing = existing(&["以后都用清单收尾"], &[]);
                value
            },
            {
                let mut value = case(
                    "已驳回两次被拒",
                    payload(vec![raw_item("以后都用清单收尾", "scene", "m-1", "以后")], vec![]),
                    0,
                    REASON_REJECTED_BEFORE,
                );
                value.rejection = rejection_memory(&["以后都用清单收尾"]);
                value
            },
            {
                let mut value = case(
                    "红线词被拒且不回显",
                    payload(vec![raw_item("以后处理机密清单", "scene", "m-8", "以后")], vec![]),
                    0,
                    REASON_REDLINE,
                );
                value.redline = Some("机密");
                value
            },
            {
                let mut value = case(
                    "check 短语命中红线被拒",
                    payload(vec![item("以后都按模板", KIND_PREFERENCE, "scene", "m-1", "以后", json!({"type": "banned_phrases", "phrases": ["机密词"]}))], vec![]),
                    0,
                    REASON_REDLINE,
                );
                value.redline = Some("机密");
                value
            },
            case(
                "text 超长被拒",
                payload(vec![raw_item(&"习".repeat(61), "scene", "m-1", "以后")], vec![]),
                0,
                REASON_TEXT_TOO_LONG,
            ),
            case(
                "空 text 被拒",
                payload(vec![raw_item("   ", "scene", "m-1", "以后")], vec![]),
                0,
                REASON_EMPTY_TEXT,
            ),
            case(
                "check 非法被拒",
                payload(vec![item("以后都列清单", KIND_PREFERENCE, "scene", "m-1", "以后", json!({"type": "paragraph_max_chars", "limit": 5}))], vec![]),
                0,
                REASON_CHECK_INVALID,
            ),
            {
                let mut value = case(
                    "check 合法（禁用短语）",
                    payload(vec![item("以后都列清单", KIND_PREFERENCE, "scene", "m-1", "以后", json!({"type": "banned_phrases", "phrases": ["临时口头确认"]}))], vec![]),
                    1,
                    "",
                );
                value.expected_text = Some("以后都列清单");
                value.verdict_pass = Some(true);
                value
            },
            {
                let mut value = case(
                    "check 合法（段落上限）",
                    payload(vec![item("以后每段不超过十行", KIND_FORMAT, "scene", "m-1", "以后", json!({"type": "paragraph_max_chars", "limit": 10}))], vec![]),
                    1,
                    "",
                );
                value.verdict_pass = Some(true);
                value
            },
            {
                let mut value = case(
                    "check 合法（必备短语）",
                    payload(vec![item("以后先列清单", KIND_PREFERENCE, "scene", "m-1", "以后", json!({"type": "required_phrases", "phrases": ["清单"]}))], vec![]),
                    1,
                    "",
                );
                value.verdict_pass = Some(true);
                value
            },
            {
                let mut value = case(
                    "未决问题通过",
                    payload(vec![], vec![json!({"text": "这套方案还没想好要不要做", "message_id": "m-9"})]),
                    0,
                    "",
                );
                value.rejections = Vec::new();
                value.questions = 1;
                value
            },
            {
                let mut value = case(
                    "未决问题缺不确定词被拒",
                    payload(vec![], vec![json!({"text": "这套方案下周开工", "message_id": "m-9"})]),
                    0,
                    REASON_NO_UNCERTAIN,
                );
                value.rejections = vec![REASON_NO_UNCERTAIN];
                value
            },
            {
                let long_question = format!("待定{}", "问".repeat(79));
                let mut value = case(
                    "未决问题超长被拒",
                    payload(vec![], vec![json!({"text": long_question, "message_id": "m-9"})]),
                    0,
                    REASON_QUESTION_TEXT_TOO_LONG,
                );
                value.rejections = vec![REASON_QUESTION_TEXT_TOO_LONG];
                value
            },
            {
                let mut value = case(
                    "JSON 损坏被拒",
                    "{not json".to_string(),
                    0,
                    "",
                );
                value.rejections = Vec::new();
                value
            },
            {
                let mut value = case(
                    "代码围栏内 JSON 可解析",
                    format!(
                        "```json\n{}\n```",
                        payload(
                            vec![raw_item("以后都用清单收尾", "scene", "m-1", "以后")],
                            vec![]
                        )
                    ),
                    1,
                    "",
                );
                value.expected_text = Some("以后都用清单收尾");
                value.verdict_pass = Some(true);
                value
            },
            {
                let mut value = case(
                    "items 超 10 截断",
                    payload(overflow_items, vec![]),
                    10,
                    "",
                );
                value.rejections = vec![REASON_OVERFLOW, REASON_OVERFLOW];
                value.verdict_pass = Some(true);
                value
            },
            case(
                "assistant 消息被拒",
                payload(vec![raw_item("以后都用清单收尾", "scene", "m-asst", "以后")], vec![]),
                0,
                REASON_MESSAGE_NOT_USER,
            ),
            case(
                "无信号且未重复被拒",
                payload(vec![raw_item("按模板收尾", "scene", "m-7", "按模板")], vec![]),
                0,
                REASON_NO_SIGNAL,
            ),
            {
                let mut value = case(
                    "两天重复无信号也接收",
                    payload(vec![raw_item("按模板收尾", "scene", "m-7", "按模板")], vec![]),
                    1,
                    "",
                );
                value.existing = existing(&[], &[("按模板收尾", 2)]);
                value.expected_origin = Some(ORIGIN_REPEATED);
                value.verdict_pass = Some(false);
                value
            },
            {
                let mut value = case(
                    "重复仅一天仍被拒",
                    payload(vec![raw_item("按模板收尾", "scene", "m-7", "按模板")], vec![]),
                    0,
                    REASON_NO_SIGNAL,
                );
                value.existing = existing(&[], &[("按模板收尾", 1)]);
                value
            },
            case(
                "scope_hint 非法被拒",
                payload(vec![raw_item("以后都清单", "team", "m-1", "以后")], vec![]),
                0,
                REASON_SCOPE_INVALID,
            ),
            case(
                "kind 非法被拒",
                payload(vec![item("以后都清单", "other", "scene", "m-1", "以后", Value::Null)], vec![]),
                0,
                REASON_INVALID_KIND,
            ),
            {
                let mut value = case(
                    "open_questions 超 10 截断",
                    payload(vec![], (0..12).map(|_| json!({"text": "方案待定", "message_id": "m-9"})).collect()),
                    0,
                    "",
                );
                value.questions = 10;
                value.rejections = vec![REASON_OVERFLOW, REASON_OVERFLOW];
                value
            },
        ];
        assert!(cases.len() >= 20, "表驱动样例必须至少 20 组");

        for case in cases.drain(..) {
            let policy = policy(case.redline);
            let digest_ref = digest.as_ref();
            let result = parse_habit_output(
                &case.raw,
                &digest_ref,
                &policy,
                &case.existing,
                &case.rejection,
                NOW,
            );
            match result {
                Err(error) => {
                    assert_eq!(case.name, "JSON 损坏被拒", "{} 意外解析失败：{error}", case.name);
                    assert!(error.contains("不是合法 JSON"), "{} 的错误应明确：{error}", case.name);
                }
                Ok(parsed) => {
                    assert_eq!(parsed.candidates.len(), case.candidates, "{} 候选数不符", case.name);
                    assert_eq!(
                        parsed.open_questions.len(),
                        case.questions,
                        "{} 未决问题数不符",
                        case.name
                    );
                    for expected in &case.rejections {
                        assert!(
                            parsed.rejected.iter().any(|entry| entry.reason_code == *expected),
                            "{} 缺少拒绝原因 {expected}：{:?}",
                            case.name,
                            parsed.rejected
                        );
                    }
                    if let Some(text) = case.expected_text {
                        assert_eq!(parsed.candidates[0].text, text, "{} 文本不符", case.name);
                    }
                    if let Some(scope) = &case.expected_scope {
                        match (scope, &parsed.candidates[0].scope) {
                            (ExpectedScope::Scene, HabitScope::Scene { scene }) => {
                                assert_eq!(scene, "thread-1", "{} 场景来源不符", case.name);
                            }
                            (ExpectedScope::Global, HabitScope::Global) => {}
                            _ => panic!("{} 作用域不符", case.name),
                        }
                    }
                    if let Some(origin) = case.expected_origin {
                        assert_eq!(parsed.candidates[0].origin, origin, "{} origin 不符", case.name);
                    }
                    if let Some(reason) = case.expected_reason {
                        assert!(
                            parsed.candidates[0].reason_codes.iter().any(|code| code == reason),
                            "{} 缺少降级原因 {reason}",
                            case.name
                        );
                    }
                    if let Some(pass) = case.verdict_pass {
                        assert_eq!(
                            parsed.candidates[0].verdict.conclusion == "pass",
                            pass,
                            "{} verdict 结论不符",
                            case.name
                        );
                        if pass {
                            assert_eq!(parsed.candidates[0].verdict.checks.len(), 5);
                            assert!(parsed.candidates[0]
                                .verdict
                                .checks
                                .iter()
                                .all(|check| check.pass));
                            assert_eq!(
                                parsed.candidates[0]
                                    .verdict
                                    .checks
                                    .iter()
                                    .map(|check| check.id.as_str())
                                    .collect::<Vec<_>>(),
                                vec![
                                    "cue_in_source",
                                    "signal_whitelisted",
                                    "scope_valid",
                                    "not_duplicate",
                                    "redline_clean"
                                ]
                            );
                        }
                    }
                    let serialized = serde_json::to_string(&parsed).unwrap();
                    if case.redline.is_some() {
                        assert!(!serialized.contains("机密"), "红线词不得回显");
                    }
                    if let Some(text) = case.expected_text {
                        assert!(parsed.candidates.iter().any(|candidate| candidate.text == text));
                    }
                }
            }
        }
    }

    #[test]
    fn staged_candidate_converts_to_g1_card_shape() {
        let digest = FixtureDigest::standard();
        let parsed = parse_habit_output(
            &payload(vec![raw_item("以后都用清单收尾", "scene", "m-1", "以后")], vec![]),
            &digest.as_ref(),
            &policy(None),
            &ExistingCardsRef::default(),
            &RejectionRef::default(),
            NOW,
        )
        .expect("合法 JSON 应能解析");
        let card = parsed.candidates[0].clone().into_card();
        assert_eq!(card.schema, HABIT_CARD_SCHEMA);
        assert_eq!(card.evidence.date, LOCAL_DATE);
        assert_eq!(card.origin, ORIGIN_EXPLICIT);
        assert_eq!(card.status, STATUS_CANDIDATE);
    }
}
