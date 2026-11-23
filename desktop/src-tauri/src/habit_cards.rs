//! 习惯整理存储层（OD-GROW-301 G1）。
//!
//! 存储根为 <应用数据根>/habits/，每张卡一个 JSON 文件。所有落盘路径都经过
//! 应用数据根边界校验与仓库边界校验；文件名只接受本模块生成规则内的安全 id。
//!
//! 本模块只提供存储与校验，不触发模型、不创建常驻线程、不修改引擎数据。

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app_paths::{resolve_real_path, AppPaths};
use crate::profile_rules::RulePolicy;
use crate::sha256::sha256_hex;
use crate::task_shell::{assert_local_root_allowed, assert_outside_repo, utc_iso8601};
use crate::worklog_records::{write_new_atomically, write_replace_atomically};
use crate::worklog::{civil_from_days, days_from_civil};

pub const HABIT_CARD_SCHEMA: &str = "omnidesk.habit-card.v1";
pub const OPEN_QUESTION_SCHEMA: &str = "omnidesk.open-question.v1";
pub const CONSOLIDATION_STATE_SCHEMA: &str = "omnidesk.consolidation-state.v1";
pub const REJECTION_MEMORY_SCHEMA: &str = "omnidesk.rejection-memory.v1";
pub const PENDING_REVIEW_SCHEMA: &str = "omnidesk.pending-review.v1";

pub const CANDIDATE_MAX: usize = 100;
pub const ACTIVE_MAX: usize = 80;
pub const HABIT_ID_MAX_LEN: usize = 64;
pub const HABIT_TEXT_MAX_CHARS: usize = 60;
pub const OPEN_QUESTION_TEXT_MAX_CHARS: usize = 80;
pub const PENDING_REVIEW_TEXT_MAX_CHARS: usize = 2000;
pub const CARD_FILE_MAX_BYTES: u64 = 256 * 1024;
pub const REJECTION_BLOCK_COUNT: u64 = 2;
/// 未决问题超过该天数未再被提及时进入搁置提醒。
pub const QUESTION_STALE_DAYS: i64 = 7;

pub const KIND_RULE: &str = "rule";
pub const KIND_PREFERENCE: &str = "preference";
pub const KIND_FORMAT: &str = "format";
pub const ORIGIN_EXPLICIT: &str = "explicit";
pub const ORIGIN_REPEATED: &str = "repeated";
pub const ORIGIN_WRITING_DIFF: &str = "writing_diff";
pub const STATUS_CANDIDATE: &str = "candidate";
pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_DISABLED: &str = "disabled";
pub const STATUS_ARCHIVED: &str = "archived";
pub const QUESTION_OPEN: &str = "open";
pub const QUESTION_CLOSED: &str = "closed";
pub const QUESTION_DROPPED: &str = "dropped";
pub const PENDING_PENDING: &str = "pending";
pub const PENDING_APPROVED: &str = "approved";
pub const PENDING_EXCLUDED: &str = "excluded";
pub const ACTION_ACCEPT: &str = "accept";
pub const ACTION_REJECT: &str = "reject";
pub const CARD_ACTION_DISABLE: &str = "disable";
pub const CARD_ACTION_ARCHIVE: &str = "archive";
pub const CARD_ACTION_REUSE: &str = "reuse";
pub const PENDING_ACTION_APPROVE: &str = "approve";
pub const PENDING_ACTION_EXCLUDE: &str = "exclude";

const STAT_INJECT: &str = "inject";
const STAT_VIOLATION: &str = "violation";
const STAT_ACCEPT: &str = "accept";
const STAT_REJECT: &str = "reject";
const STAT_CONSOLIDATION_RUN: &str = "consolidation_run";
const STAT_CONSOLIDATION_FAIL: &str = "consolidation_fail";
/// 引用抽查统计类型（OD-GROW-301 H7）。每行只记卡片编号与是否不相符。
pub const STAT_QUOTE_AUDIT: &str = "quote_audit";
const STAT_TYPES: &[&str] = &[
    STAT_INJECT,
    STAT_VIOLATION,
    STAT_ACCEPT,
    STAT_REJECT,
    STAT_CONSOLIDATION_RUN,
    STAT_CONSOLIDATION_FAIL,
    STAT_QUOTE_AUDIT,
];

const CANDIDATES_DIR: &str = "candidates";
const ACTIVE_DIR: &str = "active";
const ARCHIVE_DIR: &str = "archive";
const OPEN_QUESTIONS_DIR: &str = "open-questions";
const STATE_DIR: &str = "state";
const PENDING_REVIEW_DIR: &str = "pending-review";
const STATS_FILE: &str = "habits-stats.jsonl";
const CONSOLIDATION_STATE_FILE: &str = "consolidation-state.v1.json";
const REJECTION_MEMORY_FILE: &str = "rejection-memory.v1.json";
const SEVEN_DAYS_SECS: f64 = 7.0 * 86_400.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HabitScope {
    Global,
    Scene { scene: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HabitCheck {
    BannedPhrases { phrases: Vec<String> },
    ParagraphMaxChars { limit: u64 },
    RequiredPhrases { phrases: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitEvidence {
    pub thread_id: String,
    pub message_id: String,
    /// 本地日期 YYYY-MM-DD；只记录引用，不保存原话。
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HabitCard {
    pub schema: String,
    pub id: String,
    pub kind: String,
    pub scope: HabitScope,
    pub text: String,
    #[serde(default)]
    pub check: Option<HabitCheck>,
    pub evidence: HabitEvidence,
    pub origin: String,
    pub status: String,
    /// UTC ISO8601，例如 2026-11-08T04:00:00Z。
    pub created_at: String,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    pub last_seen_at: String,
    #[serde(default)]
    pub expect: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenQuestion {
    pub schema: String,
    pub id: String,
    pub text: String,
    pub thread_id: String,
    pub message_id: String,
    pub first_seen: String,
    pub last_seen: String,
    pub status: String,
    #[serde(default)]
    pub reminded: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReviewItem {
    pub schema: String,
    pub id: String,
    /// 脱敏预览；不保存原话。
    pub text: String,
    pub thread_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub reason_code: Option<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsolidationState {
    pub schema: String,
    /// thread_id -> 最后处理的用户消息下标。
    #[serde(default)]
    pub watermarks: BTreeMap<String, u64>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for ConsolidationState {
    fn default() -> Self {
        Self {
            schema: CONSOLIDATION_STATE_SCHEMA.to_string(),
            watermarks: BTreeMap::new(),
            last_run_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectionMemory {
    pub schema: String,
    /// sha256(normalized_phrase) -> 驳回次数。
    #[serde(default)]
    pub entries: BTreeMap<String, u64>,
}

impl Default for RejectionMemory {
    fn default() -> Self {
        Self {
            schema: REJECTION_MEMORY_SCHEMA.to_string(),
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitStatRow {
    pub ts: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HabitStatsListing {
    pub items: Vec<HabitStatRow>,
    pub skipped_invalid: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HabitCandidateDecision {
    pub id: String,
    pub action: String,
    pub scope: Option<HabitScope>,
    pub reason_code: Option<String>,
}

impl Default for HabitCandidateDecision {
    fn default() -> Self {
        Self {
            id: String::new(),
            action: String::new(),
            scope: None,
            reason_code: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitIssue {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitCandidateList {
    pub items: Vec<HabitCard>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitCandidateOutcome {
    pub id: String,
    pub outcome: String,
    pub card: Option<HabitCard>,
    pub rejection_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitCandidateDecisionResult {
    pub items: Vec<HabitCandidateOutcome>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitActiveList {
    pub items: Vec<HabitCard>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitCardUpdateResult {
    pub items: Vec<HabitCard>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitOpenQuestionList {
    pub items: Vec<OpenQuestion>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitOpenQuestionUpdateResult {
    pub items: Vec<OpenQuestion>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StaleOpenQuestion {
    pub question: OpenQuestion,
    /// 自 last_seen 的本地日期起算的搁置天数。
    pub idle_days: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StaleOpenQuestionList {
    pub items: Vec<StaleOpenQuestion>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitPendingReviewList {
    pub items: Vec<PendingReviewItem>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HabitPendingReviewDecisionResult {
    pub items: Vec<PendingReviewItem>,
    pub broken: usize,
    pub issues: Vec<HabitIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardSlot {
    Candidates,
    Active,
    Archive,
}

impl CardSlot {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Candidates => CANDIDATES_DIR,
            Self::Active => ACTIVE_DIR,
            Self::Archive => ARCHIVE_DIR,
        }
    }

    fn allows_status(self, status: &str) -> bool {
        match self {
            Self::Candidates => status == STATUS_CANDIDATE,
            Self::Active => status == STATUS_ACTIVE || status == STATUS_DISABLED,
            Self::Archive => status == STATUS_ARCHIVED,
        }
    }
}

pub struct HabitStore {
    root: PathBuf,
}

impl HabitStore {
    /// 由应用数据根构造，校验边界并建立习惯目录。
    pub fn resolve(paths: &AppPaths) -> Result<Self, String> {
        let store = Self::for_root(paths.root.join("habits"));
        store.assert_allowed()?;
        fs::create_dir_all(&store.root)
            .map_err(|e| format!("创建习惯目录失败（{}）：{}", store.root.display(), e))?;
        store.assert_allowed()?;
        for dir in [
            CANDIDATES_DIR,
            ACTIVE_DIR,
            ARCHIVE_DIR,
            OPEN_QUESTIONS_DIR,
            STATE_DIR,
            PENDING_REVIEW_DIR,
        ] {
            store.named_dir(dir)?;
        }
        store.assert_allowed()?;
        Ok(store)
    }

    /// 直接指定习惯根（测试用；不做创建）。
    pub fn for_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 统计文件路径，只读访问用于审计模块追加带布尔字段的统计行。
    pub fn stats_path(&self) -> PathBuf {
        self.root.join(STATS_FILE)
    }

    fn assert_allowed(&self) -> Result<(), String> {
        assert_local_root_allowed(&self.root)?;
        assert_outside_repo(&self.root)?;
        let resolved = resolve_real_path(&self.root)?;
        assert_local_root_allowed(&resolved)?;
        assert_outside_repo(&resolved)?;
        Ok(())
    }

    fn named_dir(&self, name: &str) -> Result<PathBuf, String> {
        self.assert_allowed()?;
        let dir = self.root.join(name);
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("创建习惯子目录失败（{}）：{}", dir.display(), e))?;
        }
        let meta = fs::symlink_metadata(&dir)
            .map_err(|e| format!("读取习惯子目录失败（{}）：{}", dir.display(), e))?;
        if !meta.file_type().is_dir() {
            return Err(format!("习惯子目录不是普通目录：{}", dir.display()));
        }
        let root_real = resolve_real_path(&self.root)?;
        let dir_real = resolve_real_path(&dir)?;
        if !dir_real.starts_with(&root_real) {
            return Err(format!(
                "习惯子目录真实落点越界（{} -> {}）",
                dir.display(),
                dir_real.display()
            ));
        }
        Ok(dir)
    }

    fn card_dir(&self, slot: CardSlot) -> Result<PathBuf, String> {
        self.named_dir(slot.dir_name())
    }

    fn card_path(&self, slot: CardSlot, id: &str) -> Result<PathBuf, String> {
        assert_habit_id(id)?;
        Ok(self.card_dir(slot)?.join(format!("{id}.json")))
    }

    /// 读取候选列表；七天未处理的候选在此转入归档，不生效。
    pub fn list_candidates(&self, now: f64) -> Result<HabitCandidateList, String> {
        let (mut items, issues) = self.read_cards(CardSlot::Candidates)?;
        let cutoff = utc_iso8601(now - SEVEN_DAYS_SECS);
        let mut kept = Vec::new();
        for mut card in items.drain(..) {
            if card.status == STATUS_CANDIDATE && card.created_at.as_str() < cutoff.as_str() {
                card.status = STATUS_ARCHIVED.to_string();
                self.persist_card(&card, Some(CardSlot::Candidates)).map_err(|e| {
                    format!("七天未处理候选无法归档（{}）：{}", card.id, e)
                })?;
                continue;
            }
            kept.push(card);
        }
        kept.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(HabitCandidateList {
            broken: issues.len(),
            issues,
            items: kept,
        })
    }

    /// 新建候选卡。候选上限 100；正文、检查短语与 expect 先过当前规则红线。
    pub fn create_candidate(
        &self,
        card: HabitCard,
        policy: &RulePolicy,
        now: f64,
    ) -> Result<HabitCard, String> {
        self.assert_allowed()?;
        validate_card(&card)?;
        check_card_policy(&card, policy)?;
        if card.status != STATUS_CANDIDATE {
            return Err(format!(
                "新建候选卡 status 必须是 {}，实际是 {}",
                STATUS_CANDIDATE, card.status
            ));
        }
        let listing = self.list_candidates(now)?;
        if listing.broken > 0 {
            return Err(format!(
                "候选目录有 {} 个损坏文件，拒绝新增候选：{}",
                listing.broken,
                first_issue_note(&listing.issues)
            ));
        }
        if listing.items.len() >= CANDIDATE_MAX {
            return Err(format!(
                "候选卡已达上限 {} 条，拒绝新增。请先处理现有候选。",
                CANDIDATE_MAX
            ));
        }
        if self.find_card(&card.id)?.is_some() {
            return Err(format!("习惯卡编号已存在，拒绝重复写入：{}", card.id));
        }
        self.persist_card(&card, None)?;
        Ok(card)
    }

    /// 批量接受或驳回候选。
    pub fn decide_candidates(
        &self,
        decisions: &[HabitCandidateDecision],
        policy: &RulePolicy,
        now: f64,
    ) -> Result<HabitCandidateDecisionResult, String> {
        self.assert_allowed()?;
        let (candidate_cards, issues) = self.read_cards(CardSlot::Candidates)?;
        if issues.len() > 0 {
            return Err(format!(
                "候选目录有 {} 个损坏文件，拒绝批量处理：{}",
                issues.len(),
                first_issue_note(&issues)
            ));
        }
        let (active_cards, active_issues) = self.read_cards(CardSlot::Active)?;
        if active_issues.len() > 0 {
            return Err(format!(
                "生效卡目录有 {} 个损坏文件，拒绝批量处理：{}",
                active_issues.len(),
                first_issue_note(&active_issues)
            ));
        }
        let active_now = active_cards
            .iter()
            .filter(|card| card.status == STATUS_ACTIVE)
            .count();

        let mut seen = BTreeMap::<String, ()>::new();
        let mut planned: Vec<(HabitCard, bool, Option<u64>)> = Vec::new();
        let mut accept_count = 0usize;
        let mut reject_texts: Vec<String> = Vec::new();
        for decision in decisions {
            assert_habit_id(&decision.id)?;
            if seen.insert(decision.id.clone(), ()).is_some() {
                return Err(format!("同一批次重复处理候选：{}", decision.id));
            }
            let source = candidate_cards
                .iter()
                .find(|card| card.id == decision.id)
                .ok_or_else(|| format!("候选不存在或已处理：{}", decision.id))?
                .clone();
            match decision.action.as_str() {
                ACTION_ACCEPT => {
                    let mut card = source;
                    if let Some(scope) = &decision.scope {
                        card.scope = scope.clone();
                    }
                    card.status = STATUS_ACTIVE.to_string();
                    if card.confirmed_at.is_none() {
                        card.confirmed_at = Some(utc_iso8601(now));
                    }
                    card.last_seen_at = utc_iso8601(now);
                    validate_card(&card)?;
                    check_card_policy(&card, policy)?;
                    accept_count += 1;
                    planned.push((card, true, None));
                }
                ACTION_REJECT => {
                    validate_reason_code(decision.reason_code.as_deref())?;
                    let mut card = source;
                    card.status = STATUS_ARCHIVED.to_string();
                    reject_texts.push(card.text.clone());
                    planned.push((card, false, None));
                }
                other => {
                    return Err(format!(
                        "候选决定只能是 {} 或 {}，实际是 {:?}",
                        ACTION_ACCEPT, ACTION_REJECT, other
                    ));
                }
            }
        }
        if active_now + accept_count > ACTIVE_MAX {
            return Err(format!(
                "接受后生效卡将达 {} 张，超过上限 {}；本批次未写入。",
                active_now + accept_count,
                ACTIVE_MAX
            ));
        }

        let rejection_counts = if reject_texts.is_empty() {
            Vec::new()
        } else {
            self.record_rejections(&reject_texts)?
        };
        let mut rejection_index = 0usize;
        let mut outcomes = Vec::new();
        for (card, accepted, _) in planned {
            let source_slot = CardSlot::Candidates;
            self.persist_card(&card, Some(source_slot))?;
            let reason = decisions
                .iter()
                .find(|decision| decision.id == card.id)
                .and_then(|decision| decision.reason_code.clone());
            if accepted {
                self.append_stat(STAT_ACCEPT, Some(&card.id), None, now)?;
                outcomes.push(HabitCandidateOutcome {
                    id: card.id.clone(),
                    outcome: STATUS_ACTIVE.to_string(),
                    card: Some(card.clone()),
                    rejection_count: None,
                });
            } else {
                let count = rejection_counts
                    .get(rejection_index)
                    .copied()
                    .unwrap_or(0);
                rejection_index += 1;
                self.append_stat(STAT_REJECT, Some(&card.id), reason.as_deref(), now)?;
                outcomes.push(HabitCandidateOutcome {
                    id: card.id.clone(),
                    outcome: STATUS_ARCHIVED.to_string(),
                    card: Some(card),
                    rejection_count: Some(count),
                });
            }
        }
        Ok(HabitCandidateDecisionResult {
            items: outcomes,
            broken: 0,
            issues: Vec::new(),
        })
    }

    /// 生效卡列表（含停用卡；归档卡不出现）。
    pub fn list_active(&self) -> Result<HabitActiveList, String> {
        let (mut items, issues) = self.read_cards(CardSlot::Active)?;
        items.sort_by(|a, b| {
            let ac = a.confirmed_at.as_deref().unwrap_or("");
            let bc = b.confirmed_at.as_deref().unwrap_or("");
            bc.cmp(ac)
                .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(HabitActiveList {
            broken: issues.len(),
            issues,
            items,
        })
    }

    /// 停用、归档或复用一张卡；只移动文件并改状态，不删除。
    pub fn update_card(
        &self,
        id: &str,
        action: &str,
        policy: &RulePolicy,
        now: f64,
    ) -> Result<HabitCardUpdateResult, String> {
        assert_habit_id(id)?;
        let (mut card, source_slot) = self
            .find_card_with_slot(id)?
            .ok_or_else(|| format!("习惯卡不存在：{id}"))?;
        match action {
            CARD_ACTION_DISABLE => match card.status.as_str() {
                STATUS_ACTIVE => {
                    check_card_policy(&card, policy)?;
                    card.status = STATUS_DISABLED.to_string();
                    self.persist_card(&card, Some(source_slot))?;
                }
                STATUS_DISABLED => {}
                _ => {
                    return Err(format!(
                        "只有生效卡可停用，当前状态是 {}：{}",
                        card.status, id
                    ));
                }
            },
            CARD_ACTION_ARCHIVE => {
                if card.status != STATUS_ARCHIVED {
                    if card.status == STATUS_CANDIDATE {
                        return Err(format!(
                            "候选卡请走候选接受或驳回，不能直接归档：{id}"
                        ));
                    }
                    check_card_policy(&card, policy)?;
                    card.status = STATUS_ARCHIVED.to_string();
                    self.persist_card(&card, Some(source_slot))?;
                }
            }
            CARD_ACTION_REUSE => {
                if card.status != STATUS_ACTIVE {
                    if card.status == STATUS_CANDIDATE {
                        return Err(format!(
                            "候选卡请先接受，不能直接复用：{id}"
                        ));
                    }
                    check_card_policy(&card, policy)?;
                    let active_count = self.count_active()?;
                    if active_count >= ACTIVE_MAX {
                        return Err(format!(
                            "生效卡已达上限 {} 张，拒绝复用：{}",
                            ACTIVE_MAX, id
                        ));
                    }
                    card.status = STATUS_ACTIVE.to_string();
                    if card.confirmed_at.is_none() {
                        card.confirmed_at = Some(utc_iso8601(now));
                    }
                    card.last_seen_at = utc_iso8601(now);
                    self.persist_card(&card, Some(source_slot))?;
                }
            }
            other => {
                return Err(format!(
                    "卡片操作只能是 {} / {} / {}，实际是 {:?}",
                    CARD_ACTION_DISABLE, CARD_ACTION_ARCHIVE, CARD_ACTION_REUSE, other
                ));
            }
        }
        Ok(HabitCardUpdateResult {
            items: vec![card],
            broken: 0,
            issues: Vec::new(),
        })
    }

    /// 未决问题列表，按最后出现时间倒序。
    pub fn list_open_questions(&self) -> Result<HabitOpenQuestionList, String> {
        let (mut items, issues) = self.read_open_questions()?;
        items.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.id.cmp(&b.id)));
        Ok(HabitOpenQuestionList {
            broken: issues.len(),
            issues,
            items,
        })
    }

    pub fn set_open_question_status(
        &self,
        id: &str,
        status: &str,
    ) -> Result<HabitOpenQuestionUpdateResult, String> {
        assert_habit_id(id)?;
        assert_open_question_status(status)?;
        let dir = self.named_dir(OPEN_QUESTIONS_DIR)?;
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(format!("未决问题不存在：{id}"));
        }
        let mut question = self.read_open_question_file(&path)?;
        if question.status != status {
            question.status = status.to_string();
            let bytes = serde_json::to_vec_pretty(&question)
                .map_err(|e| format!("序列化未决问题失败：{e}"))?;
            write_replace_atomically(&path, &bytes)?;
        }
        Ok(HabitOpenQuestionUpdateResult {
            items: vec![question],
            broken: 0,
            issues: Vec::new(),
        })
    }

    /// 新一轮摘要再次提及问题时刷新 last_seen；只更新仍为 open 的问题。
    pub fn touch_open_question_last_seen(
        &self,
        id: &str,
        now: f64,
    ) -> Result<HabitOpenQuestionUpdateResult, String> {
        assert_habit_id(id)?;
        let dir = self.named_dir(OPEN_QUESTIONS_DIR)?;
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(format!("未决问题不存在：{id}"));
        }
        let mut question = self.read_open_question_file(&path)?;
        if question.status != QUESTION_OPEN {
            return Ok(HabitOpenQuestionUpdateResult {
                items: vec![question],
                broken: 0,
                issues: Vec::new(),
            });
        }
        let next = utc_iso8601(now);
        if question.last_seen.as_str() < next.as_str() {
            question.last_seen = next;
            let bytes = serde_json::to_vec_pretty(&question)
                .map_err(|e| format!("序列化未决问题失败：{e}"))?;
            write_replace_atomically(&path, &bytes)?;
        }
        Ok(HabitOpenQuestionUpdateResult {
            items: vec![question],
            broken: 0,
            issues: Vec::new(),
        })
    }

    /// 超过七天未再被提及的 open 问题，按搁置天数倒序返回。
    pub fn stale_open_questions(
        &self,
        now: f64,
        utc_offset_minutes: i32,
    ) -> Result<StaleOpenQuestionList, String> {
        let (items, issues) = self.read_open_questions()?;
        let today = local_date_string_from_secs(now.floor() as i64, utc_offset_minutes);
        let mut stale = Vec::new();
        for question in items {
            if question.status != QUESTION_OPEN {
                continue;
            }
            let Some(idle_days) = open_question_idle_days(
                &question.last_seen,
                &today,
                utc_offset_minutes,
            ) else {
                continue;
            };
            if idle_days > QUESTION_STALE_DAYS {
                stale.push(StaleOpenQuestion {
                    question,
                    idle_days,
                });
            }
        }
        stale.sort_by(|a, b| {
            b.idle_days
                .cmp(&a.idle_days)
                .then_with(|| a.question.id.cmp(&b.question.id))
        });
        Ok(StaleOpenQuestionList {
            items: stale,
            broken: issues.len(),
            issues,
        })
    }

    /// 待审定列表只返回仍待审定的条目；损坏文件逐条报告。
    pub fn list_pending_review(&self) -> Result<HabitPendingReviewList, String> {
        let (all, issues) = self.read_pending_reviews()?;
        let mut items: Vec<PendingReviewItem> = all
            .into_iter()
            .filter(|item| item.status == PENDING_PENDING)
            .collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
        Ok(HabitPendingReviewList {
            broken: issues.len(),
            issues,
            items,
        })
    }

    pub fn decide_pending_review(
        &self,
        id: &str,
        action: &str,
        now: f64,
    ) -> Result<HabitPendingReviewDecisionResult, String> {
        assert_habit_id(id)?;
        let status = match action {
            PENDING_ACTION_APPROVE => PENDING_APPROVED,
            PENDING_ACTION_EXCLUDE => PENDING_EXCLUDED,
            other => {
                return Err(format!(
                    "待审定操作只能是 {} 或 {}，实际是 {:?}",
                    PENDING_ACTION_APPROVE, PENDING_ACTION_EXCLUDE, other
                ));
            }
        };
        let dir = self.named_dir(PENDING_REVIEW_DIR)?;
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(format!("待审定条目不存在：{id}"));
        }
        let mut item = self.read_pending_review_file(&path)?;
        if item.status != PENDING_PENDING && item.status != status {
            return Err(format!(
                "待审定条目已处理为 {}，不能改为 {}：{}",
                item.status, status, id
            ));
        }
        if item.status == PENDING_PENDING {
            item.status = status.to_string();
            item.decided_at = Some(utc_iso8601(now));
            let bytes = serde_json::to_vec_pretty(&item)
                .map_err(|e| format!("序列化待审定条目失败：{e}"))?;
            write_replace_atomically(&path, &bytes)?;
        }
        Ok(HabitPendingReviewDecisionResult {
            items: vec![item],
            broken: 0,
            issues: Vec::new(),
        })
    }

    pub fn create_open_question(
        &self,
        question: OpenQuestion,
    ) -> Result<OpenQuestion, String> {
        validate_open_question(&question)?;
        let dir = self.named_dir(OPEN_QUESTIONS_DIR)?;
        let target = dir.join(format!("{}.json", question.id));
        let bytes = serde_json::to_vec_pretty(&question)
            .map_err(|e| format!("序列化未决问题失败：{e}"))?;
        write_new_atomically(&target, &bytes)?;
        Ok(question)
    }

    pub fn create_pending_review(
        &self,
        item: PendingReviewItem,
    ) -> Result<PendingReviewItem, String> {
        validate_pending_review(&item)?;
        if item.status != PENDING_PENDING {
            return Err(format!(
                "新建待审定条目 status 必须是 {}，实际是 {}",
                PENDING_PENDING, item.status
            ));
        }
        let dir = self.named_dir(PENDING_REVIEW_DIR)?;
        let target = dir.join(format!("{}.json", item.id));
        let bytes = serde_json::to_vec_pretty(&item)
            .map_err(|e| format!("序列化待审定条目失败：{e}"))?;
        write_new_atomically(&target, &bytes)?;
        Ok(item)
    }

    pub fn consolidation_state(&self) -> Result<ConsolidationState, String> {
        let dir = self.named_dir(STATE_DIR)?;
        let path = dir.join(CONSOLIDATION_STATE_FILE);
        if !path.exists() {
            return Ok(ConsolidationState::default());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("读取整理状态失败（{}）：{}", path.display(), e))?;
        let state: ConsolidationState = serde_json::from_str(&raw)
            .map_err(|e| format!("整理状态损坏（{}）：{}", path.display(), e))?;
        validate_consolidation_state(&state)?;
        Ok(state)
    }

    pub fn save_consolidation_state(&self, state: &ConsolidationState) -> Result<(), String> {
        validate_consolidation_state(state)?;
        let dir = self.named_dir(STATE_DIR)?;
        let path = dir.join(CONSOLIDATION_STATE_FILE);
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|e| format!("序列化整理状态失败：{e}"))?;
        write_replace_atomically(&path, &bytes)
    }

    pub fn rejection_memory(&self) -> Result<RejectionMemory, String> {
        let dir = self.named_dir(STATE_DIR)?;
        let path = dir.join(REJECTION_MEMORY_FILE);
        if !path.exists() {
            return Ok(RejectionMemory::default());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("读取驳回记录失败（{}）：{}", path.display(), e))?;
        let memory: RejectionMemory = serde_json::from_str(&raw)
            .map_err(|e| format!("驳回记录损坏（{}）：{}", path.display(), e))?;
        validate_rejection_memory(&memory)?;
        Ok(memory)
    }

    pub fn save_rejection_memory(&self, memory: &RejectionMemory) -> Result<(), String> {
        validate_rejection_memory(memory)?;
        let dir = self.named_dir(STATE_DIR)?;
        let path = dir.join(REJECTION_MEMORY_FILE);
        let bytes = serde_json::to_vec_pretty(memory)
            .map_err(|e| format!("序列化驳回记录失败：{e}"))?;
        write_replace_atomically(&path, &bytes)
    }

    /// 记录一组驳回短语，返回每个短语累计后的次数。
    pub fn record_rejections(&self, texts: &[String]) -> Result<Vec<u64>, String> {
        let mut memory = self.rejection_memory()?;
        let mut counts = Vec::new();
        for text in texts {
            let hash = normalize_rejection_phrase_hash(text);
            let count = memory.entries.entry(hash).or_insert(0);
            *count = count.saturating_add(1);
            counts.push(*count);
        }
        self.save_rejection_memory(&memory)?;
        Ok(counts)
    }

    pub fn append_stat(
        &self,
        kind: &str,
        id: Option<&str>,
        reason_code: Option<&str>,
        now: f64,
    ) -> Result<(), String> {
        if !valid_stat_type(kind) {
            return Err(format!("统计类型不合法：{:?}", kind));
        }
        if let Some(id) = id {
            assert_habit_id(id)?;
        }
        if let Some(reason) = reason_code {
            validate_reason_code(Some(reason))?;
        }
        let path = self.root.join(STATS_FILE);
        self.assert_allowed()?;
        fs::create_dir_all(&self.root)
            .map_err(|e| format!("创建习惯目录失败（{}）：{}", self.root.display(), e))?;
        let row = HabitStatRow {
            ts: utc_iso8601(now),
            kind: kind.to_string(),
            id: id.map(str::to_string),
            reason_code: reason_code.map(str::to_string),
        };
        let line = serde_json::to_string(&row)
            .map_err(|e| format!("统计行序列化失败：{e}"))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("打开统计文件失败（{}）：{}", path.display(), e))?;
        file.write_all(line.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_data())
            .map_err(|e| format!("写入统计文件失败（{}）：{}", path.display(), e))
    }

    pub fn read_stats(&self) -> HabitStatsListing {
        let mut items = Vec::new();
        let mut skipped_invalid = 0usize;
        let path = self.root.join(STATS_FILE);
        if !path.exists() {
            return HabitStatsListing {
                items,
                skipped_invalid,
            };
        }
        let Ok(file) = fs::File::open(&path) else {
            return HabitStatsListing {
                items,
                skipped_invalid: 1,
            };
        };
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else {
                skipped_invalid += 1;
                continue;
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<HabitStatRow>(trimmed) {
                Ok(row) if validate_stat_row(&row).is_ok() => items.push(row),
                _ => skipped_invalid += 1,
            }
        }
        HabitStatsListing {
            items,
            skipped_invalid,
        }
    }

    fn count_active(&self) -> Result<usize, String> {
        let (cards, issues) = self.read_cards(CardSlot::Active)?;
        if !issues.is_empty() {
            return Err(format!(
                "生效卡目录有 {} 个损坏文件，无法统计：{}",
                issues.len(),
                first_issue_note(&issues)
            ));
        }
        Ok(cards
            .iter()
            .filter(|card| card.status == STATUS_ACTIVE)
            .count())
    }

    fn read_cards(&self, slot: CardSlot) -> Result<(Vec<HabitCard>, Vec<HabitIssue>), String> {
        self.assert_allowed()?;
        let dir = self.root.join(slot.dir_name());
        if !dir.exists() {
            return Ok((Vec::new(), Vec::new()));
        }
        let meta = fs::symlink_metadata(&dir)
            .map_err(|e| format!("读取习惯目录失败（{}）：{}", dir.display(), e))?;
        if !meta.file_type().is_dir() {
            return Err(format!("习惯目录不是普通目录：{}", dir.display()));
        }
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取习惯目录失败（{}）：{}", dir.display(), e))?;
        let mut cards = Vec::new();
        let mut issues = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    issues.push(HabitIssue {
                        file: "<目录项不可读>".to_string(),
                        reason: format!("读取目录项失败：{e}"),
                    });
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if !habit_id_allowed(id) {
                issues.push(HabitIssue {
                    file: name,
                    reason: "文件名不是本存储的安全 id".to_string(),
                });
                continue;
            }
            match self.read_card_file(&entry.path(), slot) {
                Ok(card) => cards.push(card),
                Err(reason) => issues.push(HabitIssue {
                    file: name,
                    reason,
                }),
            }
        }
        cards.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
        Ok((cards, issues))
    }

    fn read_card_file(&self, path: &Path, slot: CardSlot) -> Result<HabitCard, String> {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("读取文件属性失败：{e}"))?;
        if !meta.file_type().is_file() {
            return Err("不是普通文件".to_string());
        }
        if meta.len() > CARD_FILE_MAX_BYTES {
            return Err("文件超过大小上限".to_string());
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("读取失败：{e}"))?;
        let mut value: Value =
            serde_json::from_str(&raw).map_err(|e| format!("JSON 解析失败：{e}"))?;
        normalize_legacy_card_value(&mut value)?;
        let card: HabitCard = serde_json::from_value(value)
            .map_err(|e| format!("结构不合规：{e}"))?;
        validate_card(&card)?;
        if !slot.allows_status(&card.status) {
            return Err(format!(
                "状态 {} 不应出现在 {} 目录",
                card.status,
                slot.dir_name()
            ));
        }
        Ok(card)
    }

    fn find_card(&self, id: &str) -> Result<Option<HabitCard>, String> {
        Ok(self.find_card_with_slot(id)?.map(|(card, _)| card))
    }

    fn find_card_with_slot(
        &self,
        id: &str,
    ) -> Result<Option<(HabitCard, CardSlot)>, String> {
        assert_habit_id(id)?;
        let mut found: Option<(HabitCard, CardSlot)> = None;
        for slot in [CardSlot::Candidates, CardSlot::Active, CardSlot::Archive] {
            let path = self.card_path(slot, id)?;
            if !path.exists() {
                continue;
            }
            if found.is_some() {
                return Err(format!("同一习惯卡编号出现在多个目录：{id}"));
            }
            let card = self
                .read_card_file(&path, slot)
                .map_err(|e| format!("读取习惯卡失败（{}）：{}", path.display(), e))?;
            found = Some((card, slot));
        }
        Ok(found)
    }

    fn persist_card(&self, card: &HabitCard, from: Option<CardSlot>) -> Result<(), String> {
        validate_card(card)?;
        let target_slot = slot_for_status(&card.status)
            .ok_or_else(|| format!("习惯卡状态不合法：{}", card.status))?;
        let target = self.card_path(target_slot, &card.id)?;
        let bytes = serde_json::to_vec_pretty(card)
            .map_err(|e| format!("序列化习惯卡失败：{e}"))?;
        if from == Some(target_slot) {
            write_replace_atomically(&target, &bytes)?;
            return Ok(());
        }
        if target.exists() {
            return Err(format!(
                "目标位置已有同编号习惯卡，拒绝覆盖：{}",
                target.display()
            ));
        }
        write_new_atomically(&target, &bytes)?;
        if let Some(old_slot) = from {
            let old_path = self.card_path(old_slot, &card.id)?;
            if old_path != target {
                let old_meta = fs::symlink_metadata(&old_path);
                let remove_result = match old_meta {
                    Ok(meta) if meta.file_type().is_file() => fs::remove_file(&old_path),
                    Ok(_) => {
                        let _ = fs::remove_file(&target);
                        return Err(format!(
                            "旧习惯卡不是普通文件，拒绝留下重复记录：{}",
                            old_path.display()
                        ));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e),
                };
                if let Err(e) = remove_result {
                    let _ = fs::remove_file(&target);
                    return Err(format!(
                        "清理旧习惯卡失败（{}）：{}；新文件已撤回",
                        old_path.display(),
                        e
                    ));
                }
            }
        }
        Ok(())
    }

    fn read_open_questions(&self) -> Result<(Vec<OpenQuestion>, Vec<HabitIssue>), String> {
        let dir = self.named_dir(OPEN_QUESTIONS_DIR)?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取未决问题目录失败（{}）：{}", dir.display(), e))?;
        let mut items = Vec::new();
        let mut issues = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    issues.push(HabitIssue {
                        file: "<目录项不可读>".to_string(),
                        reason: format!("读取目录项失败：{e}"),
                    });
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if !habit_id_allowed(id) {
                issues.push(HabitIssue {
                    file: name,
                    reason: "文件名不是本存储的安全 id".to_string(),
                });
                continue;
            }
            match self.read_open_question_file(&entry.path()) {
                Ok(item) => items.push(item),
                Err(reason) => issues.push(HabitIssue { file: name, reason }),
            }
        }
        Ok((items, issues))
    }

    fn read_open_question_file(&self, path: &Path) -> Result<OpenQuestion, String> {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("读取文件属性失败：{e}"))?;
        if !meta.file_type().is_file() {
            return Err("不是普通文件".to_string());
        }
        if meta.len() > CARD_FILE_MAX_BYTES {
            return Err("文件超过大小上限".to_string());
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("读取失败：{e}"))?;
        let item: OpenQuestion =
            serde_json::from_str(&raw).map_err(|e| format!("JSON 解析失败：{e}"))?;
        validate_open_question(&item)?;
        Ok(item)
    }

    fn read_pending_reviews(
        &self,
    ) -> Result<(Vec<PendingReviewItem>, Vec<HabitIssue>), String> {
        let dir = self.named_dir(PENDING_REVIEW_DIR)?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取待审定目录失败（{}）：{}", dir.display(), e))?;
        let mut items = Vec::new();
        let mut issues = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    issues.push(HabitIssue {
                        file: "<目录项不可读>".to_string(),
                        reason: format!("读取目录项失败：{e}"),
                    });
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if !habit_id_allowed(id) {
                issues.push(HabitIssue {
                    file: name,
                    reason: "文件名不是本存储的安全 id".to_string(),
                });
                continue;
            }
            match self.read_pending_review_file(&entry.path()) {
                Ok(item) => items.push(item),
                Err(reason) => issues.push(HabitIssue { file: name, reason }),
            }
        }
        Ok((items, issues))
    }

    fn read_pending_review_file(&self, path: &Path) -> Result<PendingReviewItem, String> {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("读取文件属性失败：{e}"))?;
        if !meta.file_type().is_file() {
            return Err("不是普通文件".to_string());
        }
        if meta.len() > CARD_FILE_MAX_BYTES {
            return Err("文件超过大小上限".to_string());
        }
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("读取失败：{e}"))?;
        let item: PendingReviewItem =
            serde_json::from_str(&raw).map_err(|e| format!("JSON 解析失败：{e}"))?;
        validate_pending_review(&item)?;
        Ok(item)
    }
}

fn first_issue_note(issues: &[HabitIssue]) -> String {
    issues
        .first()
        .map(|issue| format!("{}：{}", issue.file, issue.reason))
        .unwrap_or_else(|| "无".to_string())
}

fn slot_for_status(status: &str) -> Option<CardSlot> {
    match status {
        STATUS_CANDIDATE => Some(CardSlot::Candidates),
        STATUS_ACTIVE | STATUS_DISABLED => Some(CardSlot::Active),
        STATUS_ARCHIVED => Some(CardSlot::Archive),
        _ => None,
    }
}

/// 习惯卡与文件 id：字母数字、减号、下划线，不以点开头，最多 64 字符。
pub fn habit_id_allowed(id: &str) -> bool {
    if id.is_empty() || id.len() > HABIT_ID_MAX_LEN || id.starts_with('.') {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn assert_habit_id(id: &str) -> Result<(), String> {
    if habit_id_allowed(id) {
        Ok(())
    } else {
        Err(format!(
            "习惯卡编号不合法（只允许字母、数字、- 与 _，最多 {} 字符）：{:?}",
            HABIT_ID_MAX_LEN, id
        ))
    }
}

pub fn default_scope_for_thread(thread_id: &str) -> HabitScope {
    HabitScope::Scene {
        scene: thread_id.to_string(),
    }
}

pub fn validate_card(card: &HabitCard) -> Result<(), String> {
    if card.schema != HABIT_CARD_SCHEMA {
        return Err(format!(
            "习惯卡 schema 必须是 {}，实际是 {:?}",
            HABIT_CARD_SCHEMA, card.schema
        ));
    }
    assert_habit_id(&card.id)?;
    if !matches!(card.kind.as_str(), KIND_RULE | KIND_PREFERENCE | KIND_FORMAT) {
        return Err(format!(
            "习惯卡 kind 必须是 {} / {} / {}，实际是 {:?}",
            KIND_RULE, KIND_PREFERENCE, KIND_FORMAT, card.kind
        ));
    }
    match &card.scope {
        HabitScope::Global => {}
        HabitScope::Scene { scene } => {
            if scene.trim().is_empty() || scene.chars().count() > 120 {
                return Err("习惯卡 scope.scene 必须非空且不超过 120 字符".to_string());
            }
        }
    }
    let text_chars = card.text.chars().count();
    if text_chars == 0 || text_chars > HABIT_TEXT_MAX_CHARS {
        return Err(format!(
            "习惯卡 text 必须为 1..={} 字符，实际 {} 字符",
            HABIT_TEXT_MAX_CHARS, text_chars
        ));
    }
    if let Some(check) = &card.check {
        match check {
            HabitCheck::BannedPhrases { phrases } | HabitCheck::RequiredPhrases { phrases } => {
                if phrases.is_empty() {
                    return Err("检查短语列表不能为空".to_string());
                }
                if phrases.iter().any(|phrase| phrase.trim().is_empty()) {
                    return Err("检查短语不能为空".to_string());
                }
            }
            HabitCheck::ParagraphMaxChars { limit } => {
                if *limit == 0 {
                    return Err("段落字数上限必须大于 0".to_string());
                }
            }
        }
    }
    if card.evidence.thread_id.trim().is_empty()
        || card.evidence.thread_id.chars().count() > 120
    {
        return Err("习惯卡 evidence.thread_id 必须非空且不超过 120 字符".to_string());
    }
    if card.evidence.message_id.trim().is_empty()
        || card.evidence.message_id.chars().count() > 120
    {
        return Err("习惯卡 evidence.message_id 必须非空且不超过 120 字符".to_string());
    }
    if !crate::worklog_records::is_date_shape(&card.evidence.date) {
        return Err(format!(
            "习惯卡 evidence.date 必须是 YYYY-MM-DD，实际是 {:?}",
            card.evidence.date
        ));
    }
    if !matches!(
        card.origin.as_str(),
        ORIGIN_EXPLICIT | ORIGIN_REPEATED | ORIGIN_WRITING_DIFF
    ) {
        return Err(format!(
            "习惯卡 origin 必须是 explicit / repeated / writing_diff，实际是 {:?}",
            card.origin
        ));
    }
    if !matches!(
        card.status.as_str(),
        STATUS_CANDIDATE | STATUS_ACTIVE | STATUS_DISABLED | STATUS_ARCHIVED
    ) {
        return Err(format!("习惯卡 status 不合法：{:?}", card.status));
    }
    assert_iso8601_shape("created_at", &card.created_at)?;
    if let Some(confirmed_at) = &card.confirmed_at {
        assert_iso8601_shape("confirmed_at", confirmed_at)?;
    }
    assert_iso8601_shape("last_seen_at", &card.last_seen_at)?;
    if let Some(expect) = &card.expect {
        if expect.trim().is_empty() || expect.chars().count() > HABIT_TEXT_MAX_CHARS {
            return Err(format!(
                "习惯卡 expect 必须为 1..={} 个字符",
                HABIT_TEXT_MAX_CHARS
            ));
        }
    }
    Ok(())
}

fn validate_open_question(question: &OpenQuestion) -> Result<(), String> {
    if question.schema != OPEN_QUESTION_SCHEMA {
        return Err(format!(
            "未决问题 schema 必须是 {}，实际是 {:?}",
            OPEN_QUESTION_SCHEMA, question.schema
        ));
    }
    assert_habit_id(&question.id)?;
    let chars = question.text.chars().count();
    if chars == 0 || chars > OPEN_QUESTION_TEXT_MAX_CHARS {
        return Err(format!(
            "未决问题 text 必须为 1..={} 字符，实际 {} 字符",
            OPEN_QUESTION_TEXT_MAX_CHARS, chars
        ));
    }
    if question.thread_id.trim().is_empty() || question.message_id.trim().is_empty() {
        return Err("未决问题必须带来源线程与消息编号".to_string());
    }
    assert_iso8601_shape("first_seen", &question.first_seen)?;
    assert_iso8601_shape("last_seen", &question.last_seen)?;
    assert_open_question_status(&question.status)
}

fn assert_open_question_status(status: &str) -> Result<(), String> {
    if matches!(status, QUESTION_OPEN | QUESTION_CLOSED | QUESTION_DROPPED) {
        Ok(())
    } else {
        Err(format!(
            "未决问题 status 必须是 open / closed / dropped，实际是 {:?}",
            status
        ))
    }
}

/// last_seen 所在本地日期到 today 的自然日差；任一日期形状不合法返回 None。
pub fn open_question_idle_days(
    last_seen: &str,
    today: &str,
    utc_offset_minutes: i32,
) -> Option<i64> {
    let seen = local_date_string_from_utc(last_seen, utc_offset_minutes)?;
    let seen_days = date_days(&seen)?;
    let today_days = date_days(today)?;
    Some(today_days - seen_days)
}

fn local_date_string_from_utc(value: &str, utc_offset_minutes: i32) -> Option<String> {
    Some(local_date_string_from_secs(
        parse_utc_iso8601_seconds(value)?,
        utc_offset_minutes,
    ))
}

fn local_date_string_from_secs(secs: i64, utc_offset_minutes: i32) -> String {
    let days = (secs + utc_offset_minutes as i64 * 60).div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn date_days(date: &str) -> Option<i64> {
    if !crate::worklog_records::is_date_shape(date) {
        return None;
    }
    let year: i64 = date[0..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..10].parse().ok()?;
    Some(days_from_civil(year, month, day))
}

fn parse_utc_iso8601_seconds(value: &str) -> Option<i64> {
    if !is_iso8601_utc(value) {
        return None;
    }
    let year: i64 = value[0..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..10].parse().ok()?;
    let hour: i64 = value[11..13].parse().ok()?;
    let minute: i64 = value[14..16].parse().ok()?;
    let second: i64 = value[17..19].parse().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// 将 UTC ISO8601 时间解析为 Unix 秒；形状或数值不合法时返回 None。
///
/// 供只读审计模块比较时间距离，不写入、不修改既有时间字段。
pub fn utc_iso8601_seconds(value: &str) -> Option<i64> {
    parse_utc_iso8601_seconds(value)
}

fn validate_pending_review(item: &PendingReviewItem) -> Result<(), String> {
    if item.schema != PENDING_REVIEW_SCHEMA {
        return Err(format!(
            "待审定条目 schema 必须是 {}，实际是 {:?}",
            PENDING_REVIEW_SCHEMA, item.schema
        ));
    }
    assert_habit_id(&item.id)?;
    let chars = item.text.chars().count();
    if chars == 0 || chars > PENDING_REVIEW_TEXT_MAX_CHARS {
        return Err(format!(
            "待审定预览必须为 1..={} 字符，实际 {} 字符",
            PENDING_REVIEW_TEXT_MAX_CHARS, chars
        ));
    }
    if item.thread_id.trim().is_empty() || item.thread_id.chars().count() > 120 {
        return Err("待审定条目必须带来源线程编号".to_string());
    }
    if !matches!(item.status.as_str(), PENDING_PENDING | PENDING_APPROVED | PENDING_EXCLUDED) {
        return Err(format!("待审定条目 status 不合法：{:?}", item.status));
    }
    assert_iso8601_shape("created_at", &item.created_at)?;
    if let Some(decided_at) = &item.decided_at {
        assert_iso8601_shape("decided_at", decided_at)?;
    }
    if let Some(reason) = &item.reason_code {
        validate_reason_code(Some(reason))?;
    }
    Ok(())
}

fn validate_consolidation_state(state: &ConsolidationState) -> Result<(), String> {
    if state.schema != CONSOLIDATION_STATE_SCHEMA {
        return Err(format!(
            "整理状态 schema 必须是 {}，实际是 {:?}",
            CONSOLIDATION_STATE_SCHEMA, state.schema
        ));
    }
    for thread_id in state.watermarks.keys() {
        assert_habit_id(thread_id)?;
    }
    if let Some(last_run_at) = &state.last_run_at {
        assert_iso8601_shape("last_run_at", last_run_at)?;
    }
    Ok(())
}

fn validate_rejection_memory(memory: &RejectionMemory) -> Result<(), String> {
    if memory.schema != REJECTION_MEMORY_SCHEMA {
        return Err(format!(
            "驳回记录 schema 必须是 {}，实际是 {:?}",
            REJECTION_MEMORY_SCHEMA, memory.schema
        ));
    }
    for (hash, count) in &memory.entries {
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("驳回记录包含非法短语哈希".to_string());
        }
        if *count == 0 {
            return Err("驳回记录次数必须大于 0".to_string());
        }
    }
    Ok(())
}

fn validate_stat_row(row: &HabitStatRow) -> Result<(), String> {
    if !valid_stat_type(&row.kind) {
        return Err("统计类型不合法".to_string());
    }
    assert_iso8601_shape("ts", &row.ts)?;
    if let Some(id) = &row.id {
        assert_habit_id(id)?;
    }
    if let Some(reason) = &row.reason_code {
        validate_reason_code(Some(reason))?;
    }
    Ok(())
}

fn valid_stat_type(kind: &str) -> bool {
    STAT_TYPES.contains(&kind)
}

fn validate_reason_code(reason: Option<&str>) -> Result<(), String> {
    let Some(reason) = reason else {
        return Ok(());
    };
    if reason.is_empty()
        || reason.len() > 32
        || !reason
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("驳回原因代码不合法：{:?}", reason));
    }
    Ok(())
}

fn assert_iso8601_shape(label: &str, value: &str) -> Result<(), String> {
    if is_iso8601_utc(value) {
        Ok(())
    } else {
        Err(format!(
            "{label} 必须是 UTC ISO8601（YYYY-MM-DDTHH:MM:SSZ），实际是 {:?}",
            value
        ))
    }
}

fn is_iso8601_utc(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|index| bytes[*index].is_ascii_digit())
}

fn normalize_legacy_card_value(value: &mut Value) -> Result<(), String> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "习惯卡不是 JSON 对象".to_string())?;
    if obj.contains_key("scope") {
        return Ok(());
    }
    let thread_id = obj
        .get("evidence")
        .and_then(|evidence| evidence.get("thread_id"))
        .and_then(|thread_id| thread_id.as_str())
        .unwrap_or_default()
        .to_string();
    if thread_id.is_empty() {
        return Err("旧版习惯卡缺少 scope，且没有可用的来源线程".to_string());
    }
    obj.insert(
        "scope".to_string(),
        json!({"type": "scene", "scene": thread_id}),
    );
    Ok(())
}

fn check_card_policy(card: &HabitCard, policy: &RulePolicy) -> Result<(), String> {
    if contains_reject_keyword(&card.text, policy) {
        return Err(redline_message());
    }
    if let Some(check) = &card.check {
        match check {
            HabitCheck::BannedPhrases { phrases } | HabitCheck::RequiredPhrases { phrases } => {
                for phrase in phrases {
                    if contains_reject_keyword(phrase, policy) {
                        return Err(redline_message());
                    }
                }
            }
            HabitCheck::ParagraphMaxChars { .. } => {}
        }
    }
    if let Some(expect) = &card.expect {
        if contains_reject_keyword(expect, policy) {
            return Err(redline_message());
        }
    }
    Ok(())
}

fn redline_message() -> String {
    "习惯卡文本命中当前规则红线关键词，已拒绝保存；错误信息不回显命中词。".to_string()
}

/// 存储层的简单命中判断：任一非空 reject 关键词出现在文本中即拒绝。
fn contains_reject_keyword(text: &str, policy: &RulePolicy) -> bool {
    policy
        .reject_keywords()
        .iter()
        .any(|keyword| !keyword.is_empty() && text.contains(keyword.as_str()))
}

/// 驳回去重使用的规范化短语：去掉首尾空白并把连续空白压成一个空格。
pub fn normalize_rejection_phrase(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

pub fn normalize_rejection_phrase_hash(text: &str) -> String {
    sha256_hex(normalize_rejection_phrase(text).as_bytes())
}

/// 次数达到两次的短语不再作为候选提出。
pub fn rejection_blocks(memory: &RejectionMemory, text: &str) -> bool {
    let hash = normalize_rejection_phrase_hash(text);
    memory
        .entries
        .get(&hash)
        .copied()
        .unwrap_or(0)
        >= REJECTION_BLOCK_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_rules::ProfileRules;
    use serde_json::json;

    const NOW: f64 = 1_800_000_000.0;

    fn temp_root(tag: &str) -> (HabitStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-habit-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&base);
        (HabitStore::for_root(base.join("habits")), base)
    }

    fn synthetic_policy(keyword: &str) -> RulePolicy {
        let rules: ProfileRules = serde_json::from_value(json!({
            "schema": "omnidesk.profile-rules.v1",
            "profile": "synthetic-habits",
            "global": {"redline_keywords": [keyword]}
        }))
        .expect("合成规则资产应可解析");
        RulePolicy::from_rules(&rules)
    }

    fn card(id: &str, status: &str, text: &str) -> HabitCard {
        HabitCard {
            schema: HABIT_CARD_SCHEMA.to_string(),
            id: id.to_string(),
            kind: KIND_PREFERENCE.to_string(),
            scope: default_scope_for_thread("thread-1"),
            text: text.to_string(),
            check: None,
            evidence: HabitEvidence {
                thread_id: "thread-1".to_string(),
                message_id: "msg-1".to_string(),
                date: "2026-11-20".to_string(),
            },
            origin: ORIGIN_EXPLICIT.to_string(),
            status: status.to_string(),
            created_at: utc_iso8601(NOW),
            confirmed_at: None,
            last_seen_at: utc_iso8601(NOW),
            expect: None,
        }
    }

    fn write_raw(store: &HabitStore, slot: CardSlot, card: &HabitCard) {
        let dir = store.card_dir(slot).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{}.json", card.id)),
            serde_json::to_vec_pretty(card).unwrap(),
        )
        .unwrap();
    }

    fn issue_contains(issues: &[HabitIssue], file: &str, needle: &str) {
        let issue = issues
            .iter()
            .find(|issue| issue.file == file)
            .unwrap_or_else(|| panic!("未报告文件 {file}：{issues:?}"));
        assert!(
            issue.reason.contains(needle),
            "{} 的原因 {:?} 不含 {}",
            file,
            issue.reason,
            needle
        );
    }

    #[test]
    fn validates_lengths_enums_and_bounds() {
        let (store, base) = temp_root("validate");
        let policy = RulePolicy::clean();
        let mut bad = card("h1", STATUS_CANDIDATE, &"字".repeat(61));
        assert!(store.create_candidate(bad.clone(), &policy, NOW).is_err());
        bad.text = "合成文本".to_string();
        bad.id = "../escape".to_string();
        assert!(store.create_candidate(bad, &policy, NOW).is_err());
        let mut bad = card("h2", STATUS_CANDIDATE, "合成文本");
        bad.kind = "unknown".to_string();
        assert!(store.create_candidate(bad, &policy, NOW).is_err());
        let mut bad = card("h3", STATUS_CANDIDATE, "合成文本");
        bad.check = Some(HabitCheck::ParagraphMaxChars { limit: 0 });
        assert!(store.create_candidate(bad, &policy, NOW).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_over_candidate_and_active_limits() {
        let (store, base) = temp_root("candidate-limit");
        let policy = RulePolicy::clean();
        for index in 0..CANDIDATE_MAX {
            let mut item = card(&format!("c{index}"), STATUS_CANDIDATE, "合成候选");
            item.created_at = utc_iso8601(NOW);
            write_raw(&store, CardSlot::Candidates, &item);
        }
        assert_eq!(
            store.list_candidates(NOW).unwrap().items.len(),
            CANDIDATE_MAX
        );
        let error = store
            .create_candidate(card("cover", STATUS_CANDIDATE, "合成候选"), &policy, NOW)
            .unwrap_err();
        assert!(error.contains("上限"), "error={error}");
        let _ = fs::remove_dir_all(&base);

        let (store, base) = temp_root("active-limit");
        for index in 0..ACTIVE_MAX {
            let mut item = card(&format!("a{index}"), STATUS_ACTIVE, "合成生效");
            item.confirmed_at = Some(utc_iso8601(NOW));
            write_raw(&store, CardSlot::Active, &item);
        }
        let pending = card("one-more", STATUS_CANDIDATE, "合成候选");
        store.create_candidate(pending, &policy, NOW).unwrap();
        let error = store
            .decide_candidates(
                &[HabitCandidateDecision {
                    id: "one-more".to_string(),
                    action: ACTION_ACCEPT.to_string(),
                    scope: None,
                    reason_code: None,
                }],
                &policy,
                NOW,
            )
            .unwrap_err();
        assert!(error.contains("上限"), "error={error}");
        assert_eq!(store.list_active().unwrap().items.len(), ACTIVE_MAX);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn redline_rejects_and_does_not_echo_keyword() {
        let (store, base) = temp_root("redline");
        let policy = synthetic_policy("合成敏感词");
        let mut item = card("hred", STATUS_CANDIDATE, "包含合成敏感词的合成样本");
        assert!(store.create_candidate(item.clone(), &policy, NOW).is_err());
        item.text = "普通合成文本".to_string();
        item.check = Some(HabitCheck::BannedPhrases {
            phrases: vec!["合成敏感词".to_string()],
        });
        let error = store
            .create_candidate(item.clone(), &policy, NOW)
            .unwrap_err();
        assert!(!error.contains("合成敏感词"), "error={error}");
        item.check = None;
        item.expect = Some("合成敏感词".to_string());
        let error = store.create_candidate(item, &policy, NOW).unwrap_err();
        assert!(!error.contains("合成敏感词"), "error={error}");
        assert!(contains_reject_keyword("普通文本", &policy) == false);
        assert!(contains_reject_keyword("含合成敏感词", &policy));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn batch_accept_and_reject_with_scope_and_rejection_memory() {
        let (store, base) = temp_root("decide");
        let policy = RulePolicy::clean();
        store
            .create_candidate(card("hacc", STATUS_CANDIDATE, "偏好短句"), &policy, NOW)
            .unwrap();
        store
            .create_candidate(card("hrej1", STATUS_CANDIDATE, "重复出现的短语"), &policy, NOW)
            .unwrap();
        store
            .create_candidate(card("hrej2", STATUS_CANDIDATE, "重复出现的短语"), &policy, NOW)
            .unwrap();
        let result = store
            .decide_candidates(
                &[
                    HabitCandidateDecision {
                        id: "hacc".to_string(),
                        action: ACTION_ACCEPT.to_string(),
                        scope: Some(HabitScope::Global),
                        reason_code: None,
                    },
                    HabitCandidateDecision {
                        id: "hrej1".to_string(),
                        action: ACTION_REJECT.to_string(),
                        scope: None,
                        reason_code: Some("too_absolute".to_string()),
                    },
                ],
                &policy,
                NOW,
            )
            .unwrap();
        assert_eq!(result.items.len(), 2);
        let accepted = &result.items[0];
        assert_eq!(accepted.outcome, STATUS_ACTIVE);
        assert_eq!(accepted.card.as_ref().unwrap().scope, HabitScope::Global);
        assert_eq!(result.items[1].outcome, STATUS_ARCHIVED);
        assert_eq!(result.items[1].rejection_count, Some(1));
        let memory = store.rejection_memory().unwrap();
        assert!(!rejection_blocks(&memory, "重复出现的短语"));
        store
            .decide_candidates(
                &[HabitCandidateDecision {
                    id: "hrej2".to_string(),
                    action: ACTION_REJECT.to_string(),
                    scope: None,
                    reason_code: Some("not_mine".to_string()),
                }],
                &policy,
                NOW,
            )
            .unwrap();
        let memory = store.rejection_memory().unwrap();
        assert!(rejection_blocks(&memory, "重复出现的短语"));
        assert_eq!(store.list_candidates(NOW).unwrap().items.len(), 0);
        assert_eq!(store.list_active().unwrap().items.len(), 1);
        let stats = store.read_stats();
        assert_eq!(stats.skipped_invalid, 0);
        assert_eq!(
            stats.items.iter().filter(|row| row.kind == STAT_ACCEPT).count(),
            1
        );
        assert_eq!(
            stats.items.iter().filter(|row| row.kind == STAT_REJECT).count(),
            2
        );
        assert!(stats
            .items
            .iter()
            .any(|row| row.reason_code.as_deref() == Some("too_absolute")));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn seven_day_candidate_is_archived_on_list_without_activation() {
        let (store, base) = temp_root("stale");
        let policy = RulePolicy::clean();
        let old = card("hold", STATUS_CANDIDATE, "旧候选");
        write_raw(
            &store,
            CardSlot::Candidates,
            &HabitCard {
                created_at: utc_iso8601(NOW - 8.0 * 86_400.0),
                ..old
            },
        );
        let fresh = card("hfresh", STATUS_CANDIDATE, "新候选");
        write_raw(&store, CardSlot::Candidates, &fresh);
        let listing = store.list_candidates(NOW).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, "hfresh");
        assert!(store
            .card_path(CardSlot::Candidates, "hold")
            .unwrap()
            .exists()
            == false);
        let archived = store
            .read_card_file(
                &store.card_path(CardSlot::Archive, "hold").unwrap(),
                CardSlot::Archive,
            )
            .unwrap();
        assert_eq!(archived.status, STATUS_ARCHIVED);
        assert_eq!(store.list_active().unwrap().items.len(), 0);
        let _ = fs::remove_dir_all(&base);
        let _ = policy;
    }

    #[test]
    fn corrupted_files_are_reported_with_name_and_reason() {
        let (store, base) = temp_root("corrupt");
        write_raw(&store, CardSlot::Candidates, &card("hgood", STATUS_CANDIDATE, "好卡片"));
        let dir = store.card_dir(CardSlot::Candidates).unwrap();
        fs::write(dir.join("hbroken.json"), "{not json").unwrap();
        fs::write(dir.join("bad name.json"), "{}").unwrap();
        let listing = store.list_candidates(NOW).unwrap();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].id, "hgood");
        assert_eq!(listing.broken, 2);
        issue_contains(&listing.issues, "hbroken.json", "JSON 解析失败");
        issue_contains(&listing.issues, "bad name.json", "文件名");
        assert_eq!(store.create_candidate(card("hnew", STATUS_CANDIDATE, "另一张"), &RulePolicy::clean(), NOW).unwrap_err().contains("损坏文件"), true);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn old_v1_card_without_optional_fields_reads_with_default_scope() {
        let (store, base) = temp_root("legacy");
        let dir = store.card_dir(CardSlot::Active).unwrap();
        let legacy = json!({
            "schema": HABIT_CARD_SCHEMA,
            "id": "hlegacy",
            "kind": KIND_RULE,
            "text": "旧版规则",
            "evidence": {
                "thread_id": "thread-legacy",
                "message_id": "msg-legacy",
                "date": "2026-11-19"
            },
            "origin": ORIGIN_EXPLICIT,
            "status": STATUS_ACTIVE,
            "created_at": utc_iso8601(NOW),
            "last_seen_at": utc_iso8601(NOW)
        });
        fs::write(
            dir.join("hlegacy.json"),
            serde_json::to_vec_pretty(&legacy).unwrap(),
        )
        .unwrap();
        let listing = store.list_active().unwrap();
        assert_eq!(listing.broken, 0);
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].check, None);
        assert_eq!(listing.items[0].expect, None);
        assert_eq!(listing.items[0].confirmed_at, None);
        assert_eq!(
            listing.items[0].scope,
            HabitScope::Scene {
                scene: "thread-legacy".to_string()
            }
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn failed_atomic_replace_leaves_no_half_file_or_temp() {
        let (store, base) = temp_root("atomic");
        let target_dir = store.card_dir(CardSlot::Candidates).unwrap();
        let target = target_dir.join("hblocked.json");
        fs::create_dir_all(&target).unwrap();
        let bytes = serde_json::to_vec_pretty(&card("hblocked", STATUS_CANDIDATE, "合成")).unwrap();
        assert!(write_replace_atomically(&target, &bytes).is_err());
        let names: Vec<String> = fs::read_dir(&target_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["hblocked.json".to_string()]);
        assert!(target.is_dir(), "目标应保持原状");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn stats_append_and_skip_corrupt_lines() {
        let (store, base) = temp_root("stats");
        store.append_stat(STAT_INJECT, Some("h1"), None, NOW).unwrap();
        store
            .append_stat(STAT_VIOLATION, Some("h1"), Some("too_absolute"), NOW)
            .unwrap();
        let path = store.root().join(STATS_FILE);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{not json}\n").unwrap();
        file.write_all(br#"{"ts":"bad","type":"inject"}"#).unwrap();
        let listing = store.read_stats();
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.skipped_invalid, 2);
        assert_eq!(listing.items[0].kind, STAT_INJECT);
        assert_eq!(listing.items[1].reason_code.as_deref(), Some("too_absolute"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn updates_disable_archive_and_reuse_without_deleting() {
        let (store, base) = temp_root("updates");
        let policy = RulePolicy::clean();
        let mut item = card("hup", STATUS_ACTIVE, "合成规则");
        item.confirmed_at = Some(utc_iso8601(NOW));
        write_raw(&store, CardSlot::Active, &item);
        let disabled = store
            .update_card("hup", CARD_ACTION_DISABLE, &policy, NOW)
            .unwrap();
        assert_eq!(disabled.items[0].status, STATUS_DISABLED);
        assert!(store.card_path(CardSlot::Active, "hup").unwrap().exists());
        assert_eq!(store.list_active().unwrap().items.len(), 1);
        store
            .update_card("hup", CARD_ACTION_ARCHIVE, &policy, NOW)
            .unwrap();
        assert!(!store.card_path(CardSlot::Active, "hup").unwrap().exists());
        assert!(store.card_path(CardSlot::Archive, "hup").unwrap().exists());
        let reused = store
            .update_card("hup", CARD_ACTION_REUSE, &policy, NOW)
            .unwrap();
        assert_eq!(reused.items[0].status, STATUS_ACTIVE);
        assert!(store.card_path(CardSlot::Active, "hup").unwrap().exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn open_questions_pending_review_and_consolidation_state_roundtrip() {
        let (store, base) = temp_root("other-stores");
        let question = OpenQuestion {
            schema: OPEN_QUESTION_SCHEMA.to_string(),
            id: "q1".to_string(),
            text: "要不要保留这个章节".to_string(),
            thread_id: "thread-1".to_string(),
            message_id: "msg-1".to_string(),
            first_seen: utc_iso8601(NOW),
            last_seen: utc_iso8601(NOW),
            status: QUESTION_OPEN.to_string(),
            reminded: 0,
        };
        store.create_open_question(question).unwrap();
        let updated = store
            .set_open_question_status("q1", QUESTION_CLOSED)
            .unwrap();
        assert_eq!(updated.items[0].status, QUESTION_CLOSED);
        let pending = PendingReviewItem {
            schema: PENDING_REVIEW_SCHEMA.to_string(),
            id: "p1".to_string(),
            text: "已脱敏的合成预览".to_string(),
            thread_id: "thread-1".to_string(),
            message_id: "msg-1".to_string(),
            reason_code: Some("uncertain".to_string()),
            status: PENDING_PENDING.to_string(),
            created_at: utc_iso8601(NOW),
            decided_at: None,
        };
        store.create_pending_review(pending).unwrap();
        assert_eq!(store.list_pending_review().unwrap().items.len(), 1);
        let decided = store
            .decide_pending_review("p1", PENDING_ACTION_APPROVE, NOW)
            .unwrap();
        assert_eq!(decided.items[0].status, PENDING_APPROVED);
        assert_eq!(store.list_pending_review().unwrap().items.len(), 0);
        assert_eq!(store.list_open_questions().unwrap().items.len(), 1);

        let mut state = store.consolidation_state().unwrap();
        state.watermarks.insert("thread-1".to_string(), 3);
        state.last_run_at = Some(utc_iso8601(NOW));
        store.save_consolidation_state(&state).unwrap();
        let loaded = store.consolidation_state().unwrap();
        assert_eq!(loaded.watermarks.get("thread-1"), Some(&3));
        assert_eq!(loaded.last_run_at, state.last_run_at);
        let _ = fs::remove_dir_all(&base);
    }

    fn synthetic_question(id: &str, status: &str, last_seen: f64) -> OpenQuestion {
        OpenQuestion {
            schema: OPEN_QUESTION_SCHEMA.to_string(),
            id: id.to_string(),
            text: "要不要保留这一章".to_string(),
            thread_id: "thread-1".to_string(),
            message_id: "msg-1".to_string(),
            first_seen: utc_iso8601(NOW - 30.0 * 86_400.0),
            last_seen: utc_iso8601(last_seen),
            status: status.to_string(),
            reminded: 0,
        }
    }

    #[test]
    fn open_question_touch_is_atomic_and_only_updates_open() {
        let (store, base) = temp_root("question-touch");
        store
            .create_open_question(synthetic_question("q-open", QUESTION_OPEN, NOW))
            .unwrap();
        store
            .create_open_question(synthetic_question("q-closed", QUESTION_CLOSED, NOW))
            .unwrap();
        let dir = base.join("habits").join("open-questions");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);

        let touched = store
            .touch_open_question_last_seen("q-open", NOW + 120.0)
            .unwrap();
        assert_eq!(touched.items[0].last_seen, utc_iso8601(NOW + 120.0));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        for entry in fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(name.ends_with(".json"), "残留非目标文件：{name}");
        }
        let raw = fs::read_to_string(dir.join("q-open.json")).unwrap();
        let parsed: OpenQuestion = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.last_seen, utc_iso8601(NOW + 120.0));

        let closed = store
            .touch_open_question_last_seen("q-closed", NOW + 120.0)
            .unwrap();
        assert_eq!(closed.items[0].last_seen, utc_iso8601(NOW));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn stale_open_questions_exclude_seventh_day_and_include_eighth() {
        let (store, base) = temp_root("question-stale");
        store
            .create_open_question(synthetic_question("q7", QUESTION_OPEN, NOW - 7.0 * 86_400.0))
            .unwrap();
        store
            .create_open_question(synthetic_question("q8", QUESTION_OPEN, NOW - 8.0 * 86_400.0))
            .unwrap();
        store
            .create_open_question(synthetic_question("q9-closed", QUESTION_CLOSED, NOW - 9.0 * 86_400.0))
            .unwrap();

        let stale = store.stale_open_questions(NOW, 480).unwrap();
        assert_eq!(stale.items.len(), 1);
        assert_eq!(stale.items[0].question.id, "q8");
        assert_eq!(stale.items[0].idle_days, 8);

        let today = local_date_string_from_secs(NOW as i64, 480);
        assert_eq!(
            open_question_idle_days(&utc_iso8601(NOW - 7.0 * 86_400.0), &today, 480),
            Some(7)
        );
        assert_eq!(
            open_question_idle_days(&utc_iso8601(NOW - 8.0 * 86_400.0), &today, 480),
            Some(8)
        );
        assert_eq!(open_question_idle_days("not-a-date", &today, 480), None);
        assert_eq!(open_question_idle_days(&utc_iso8601(NOW), "bad", 480), None);
        let _ = fs::remove_dir_all(&base);
    }
}
