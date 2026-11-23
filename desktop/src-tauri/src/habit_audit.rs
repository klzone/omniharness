//! 习惯卡引用抽查与确定性复核（OD-GROW-301 H7 + G12）。
//!
//! H7：对已有 violation 统计的生效卡，用调用方提供的样本正文核对声称命中
//! 是否有原文依据；结果只追加 quote_audit 统计行，不拦截、不改动流程。
//! G12：整理运行时顺带扫描生效卡，产出「可复核 / 冲突」标记报告；只读，
//! 不自动停用、归档或改写任何卡片。
//!
//! 本模块的报告与统计只含卡片编号、状态与布尔标记，不含正文或命中原话。

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::habit_cards::{
    assert_habit_id, utc_iso8601_seconds, HabitCard, HabitCheck, HabitStatRow, HabitStore,
    STAT_QUOTE_AUDIT, STATUS_ACTIVE,
};
use crate::task_shell::utc_iso8601;

/// 生效卡超过该天数无注入命中且无新证据时标记「可复核」。
pub const REVIEW_DUE_DAYS: i64 = 90;
/// 统计行里表示违反记录的类型。
pub const VIOLATION_STAT_TYPE: &str = "violation";
/// 统计行里表示注入命中的类型。
pub const INJECT_STAT_TYPE: &str = "inject";

const SECS_PER_DAY: i64 = 86_400;

/// 一条只读样本正文；context_id 只用于调用方对照，不进入报告或统计。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteSample {
    pub context_id: String,
    pub body: String,
}

/// 单张卡的引用抽查状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteAuditStatus {
    /// 非生效卡、无短语检查，或本次没有 violation 记录。
    Skipped,
    /// 有 violation 记录但没有样本正文，不做估算。
    Unknown,
    /// 样本正文支持声称的命中。
    Matched,
    /// 样本正文找不到声称命中的依据。
    Mismatched,
}

/// 单张卡的抽查结果；不包含样本正文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteAuditResult {
    pub card_id: String,
    pub status: QuoteAuditStatus,
}

impl QuoteAuditResult {
    /// 只有 Matched / Mismatched 才有布尔结论；其余状态返回 None。
    pub fn mismatch(&self) -> Option<bool> {
        match self.status {
            QuoteAuditStatus::Matched => Some(false),
            QuoteAuditStatus::Mismatched => Some(true),
            _ => None,
        }
    }
}

/// 一轮抽查汇总；checked 只计有样本且完成判定的卡。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteAuditReport {
    pub checked: usize,
    pub mismatches: usize,
    pub skipped: usize,
    pub unknown: usize,
    pub items: Vec<QuoteAuditResult>,
}

impl QuoteAuditReport {
    /// 不相符比例 = 不相符卡数 / 已抽查卡数；没有已抽查卡时返回 None。
    pub fn mismatch_ratio(&self) -> Option<f64> {
        if self.checked == 0 {
            None
        } else {
            Some(self.mismatches as f64 / self.checked as f64)
        }
    }
}

/// 写入 habits-stats.jsonl 的引用抽查行。只记卡片编号与布尔标记，不含正文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteAuditStatRow {
    pub ts: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub mismatch: bool,
}

/// 抽查单张卡。调用方应只对已有 violation 记录的卡调用；没有样本时返回 Unknown。
///
/// - 禁用短语卡：任一样本正文包含任一声称禁用短语，视为相符。
/// - 必备短语卡：violation 表示「缺少必备短语」，任一样本缺少任一必备短语即相符；
///   所有样本都包含全部必备短语时视为不相符。
pub fn audit_quote(card: &HabitCard, sample_bodies: &[QuoteSample]) -> QuoteAuditResult {
    let status = if card.status != STATUS_ACTIVE {
        QuoteAuditStatus::Skipped
    } else {
        match card.check.as_ref() {
            Some(HabitCheck::BannedPhrases { phrases }) if !phrases.is_empty() => {
                if sample_bodies.is_empty() {
                    QuoteAuditStatus::Unknown
                } else if sample_bodies.iter().any(|sample| {
                    phrases
                        .iter()
                        .any(|phrase| !phrase.is_empty() && sample.body.contains(phrase))
                }) {
                    QuoteAuditStatus::Matched
                } else {
                    QuoteAuditStatus::Mismatched
                }
            }
            Some(HabitCheck::RequiredPhrases { phrases }) if !phrases.is_empty() => {
                if sample_bodies.is_empty() {
                    QuoteAuditStatus::Unknown
                } else if phrases.iter().any(|phrase| {
                    sample_bodies
                        .iter()
                        .any(|sample| !phrase.is_empty() && !sample.body.contains(phrase))
                }) {
                    QuoteAuditStatus::Matched
                } else {
                    QuoteAuditStatus::Mismatched
                }
            }
            _ => QuoteAuditStatus::Skipped,
        }
    };
    QuoteAuditResult {
        card_id: card.id.clone(),
        status,
    }
}

/// 抽查全部生效卡：只对统计里有 violation 记录的卡做引用核对；其余跳过。
pub fn audit_active_quotes(
    cards: &[HabitCard],
    stats: &[HabitStatRow],
    sample_bodies: &[QuoteSample],
) -> QuoteAuditReport {
    let mut report = QuoteAuditReport::default();
    for card in cards.iter().filter(|card| card.status == STATUS_ACTIVE) {
        let claimed_hit = stats.iter().any(|row| {
            row.kind == VIOLATION_STAT_TYPE && row.id.as_deref() == Some(card.id.as_str())
        });
        let result = if claimed_hit {
            audit_quote(card, sample_bodies)
        } else {
            QuoteAuditResult {
                card_id: card.id.clone(),
                status: QuoteAuditStatus::Skipped,
            }
        };
        match result.status {
            QuoteAuditStatus::Matched => report.checked += 1,
            QuoteAuditStatus::Mismatched => {
                report.checked += 1;
                report.mismatches += 1;
            }
            QuoteAuditStatus::Skipped => report.skipped += 1,
            QuoteAuditStatus::Unknown => report.unknown += 1,
        }
        report.items.push(result);
    }
    report
}

/// 把有结论的抽查结果追加到统计文件；Skipped / Unknown 不写估算行。
pub fn record_quote_audits(
    store: &HabitStore,
    report: &QuoteAuditReport,
    now: f64,
) -> Result<(), String> {
    let mut lines = Vec::new();
    for item in &report.items {
        let Some(mismatch) = item.mismatch() else {
            continue;
        };
        assert_habit_id(&item.card_id)?;
        let row = QuoteAuditStatRow {
            ts: utc_iso8601(now),
            kind: STAT_QUOTE_AUDIT.to_string(),
            id: item.card_id.clone(),
            mismatch,
        };
        let line = serde_json::to_string(&row)
            .map_err(|error| format!("引用抽查统计序列化失败：{error}"))?;
        lines.push(line);
    }
    if lines.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(store.root())
        .map_err(|error| format!("创建习惯目录失败（{}）：{error}", store.root().display()))?;
    let path = store.stats_path();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("打开统计文件失败（{}）：{error}", path.display()))?;
    for line in lines {
        file.write_all(line.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| format!("写入统计文件失败（{}）：{error}", path.display()))?;
    }
    file.sync_data()
        .map_err(|error| format!("同步统计文件失败（{}）：{error}", path.display()))
}

/// 读取统计文件里的引用抽查行；损坏行直接跳过，不影响其它指标。
pub fn read_quote_audits(store: &HabitStore) -> Vec<QuoteAuditStatRow> {
    let Ok(file) = File::open(store.stats_path()) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<QuoteAuditStatRow>(line.trim()).ok())
        .filter(|row| row.kind == STAT_QUOTE_AUDIT)
        .collect()
}

/// 单张卡的复核标记；两个布尔值互不影响。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratorFlags {
    pub review_due: bool,
    pub conflict: bool,
}

impl CuratorFlags {
    fn any(&self) -> bool {
        self.review_due || self.conflict
    }
}

/// 报告项：只含卡片编号与标记。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratorEntry {
    pub card_id: String,
    pub flags: CuratorFlags,
}

/// 确定性复核报告；只列出至少有一个标记的卡。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratorReport {
    pub items: Vec<CuratorEntry>,
}

/// 只读复核生效卡：
///
/// - review_due：统计窗口内没有该卡 inject 记录，且 last_seen_at 早于 90 天前；
/// - conflict：同 scope 的两张卡之间，同一短语一边出现在 banned_phrases、
///   另一边出现在 required_phrases，双方都标冲突。跨 scope 不计冲突。
///
/// stats_cursor 是调用方提供的统计行窗口；函数不读取磁盘、不改动任何文件。
pub fn review_active_cards(
    cards: &[HabitCard],
    stats_cursor: &[HabitStatRow],
    now: f64,
) -> CuratorReport {
    let active: Vec<&HabitCard> = cards
        .iter()
        .filter(|card| card.status == STATUS_ACTIVE)
        .collect();
    let mut flags: Vec<CuratorFlags> = vec![CuratorFlags::default(); active.len()];

    for (index, card) in active.iter().enumerate() {
        let injected = stats_cursor.iter().any(|row| {
            row.kind == INJECT_STAT_TYPE && row.id.as_deref() == Some(card.id.as_str())
        });
        let last_seen = utc_iso8601_seconds(&card.last_seen_at);
        if !injected {
            if let Some(last_seen) = last_seen {
                if now - last_seen as f64 > REVIEW_DUE_DAYS as f64 * SECS_PER_DAY as f64 {
                    flags[index].review_due = true;
                }
            }
        }
    }

    for left in 0..active.len() {
        for right in (left + 1)..active.len() {
            if active[left].scope != active[right].scope {
                continue;
            }
            let left_banned = banned_phrases(active[left]);
            let left_required = required_phrases(active[left]);
            let right_banned = banned_phrases(active[right]);
            let right_required = required_phrases(active[right]);
            if phrases_intersect(left_banned, right_required)
                || phrases_intersect(right_banned, left_required)
            {
                flags[left].conflict = true;
                flags[right].conflict = true;
            }
        }
    }

    let items = active
        .iter()
        .enumerate()
        .filter(|(index, _)| flags[*index].any())
        .map(|(index, card)| CuratorEntry {
            card_id: card.id.clone(),
            flags: flags[index],
        })
        .collect();
    CuratorReport { items }
}

fn banned_phrases(card: &HabitCard) -> &[String] {
    match card.check.as_ref() {
        Some(HabitCheck::BannedPhrases { phrases }) => phrases,
        _ => &[],
    }
}

fn required_phrases(card: &HabitCard) -> &[String] {
    match card.check.as_ref() {
        Some(HabitCheck::RequiredPhrases { phrases }) => phrases,
        _ => &[],
    }
}

fn phrases_intersect(left: &[String], right: &[String]) -> bool {
    left.iter().any(|phrase| {
        !phrase.is_empty()
            && right
                .iter()
                .any(|other| !other.is_empty() && other == phrase)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habit_cards::{
        HabitEvidence, HabitScope, HABIT_CARD_SCHEMA, KIND_PREFERENCE, ORIGIN_EXPLICIT,
    };
    use std::path::PathBuf;

    const NOW: f64 = 1_800_000_000.0;

    fn temp_store(tag: &str) -> (HabitStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-habit-audit-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base);
        (HabitStore::for_root(base.join("habits")), base)
    }

    fn card(
        id: &str,
        scope: HabitScope,
        check: Option<HabitCheck>,
        last_seen_at: String,
    ) -> HabitCard {
        HabitCard {
            schema: HABIT_CARD_SCHEMA.to_string(),
            id: id.to_string(),
            kind: KIND_PREFERENCE.to_string(),
            scope,
            text: "合成习惯".to_string(),
            check,
            evidence: HabitEvidence {
                thread_id: "thread-1".to_string(),
                message_id: "msg-1".to_string(),
                date: "2026-11-20".to_string(),
            },
            origin: ORIGIN_EXPLICIT.to_string(),
            status: STATUS_ACTIVE.to_string(),
            created_at: utc_iso8601(NOW),
            confirmed_at: Some(utc_iso8601(NOW)),
            last_seen_at,
            expect: None,
        }
    }

    fn banned_card(id: &str, phrase: &str) -> HabitCard {
        card(
            id,
            HabitScope::Global,
            Some(HabitCheck::BannedPhrases {
                phrases: vec![phrase.to_string()],
            }),
            utc_iso8601(NOW),
        )
    }

    fn required_card(id: &str, phrase: &str) -> HabitCard {
        card(
            id,
            HabitScope::Global,
            Some(HabitCheck::RequiredPhrases {
                phrases: vec![phrase.to_string()],
            }),
            utc_iso8601(NOW),
        )
    }

    fn sample(context_id: &str, body: &str) -> QuoteSample {
        QuoteSample {
            context_id: context_id.to_string(),
            body: body.to_string(),
        }
    }

    fn stat(kind: &str, id: &str) -> HabitStatRow {
        HabitStatRow {
            ts: utc_iso8601(NOW),
            kind: kind.to_string(),
            id: Some(id.to_string()),
            reason_code: None,
        }
    }

    #[test]
    fn banned_phrase_present_reports_zero_mismatch_ratio() {
        let report = audit_active_quotes(
            &[banned_card("hb1", "别急")],
            &[stat(VIOLATION_STAT_TYPE, "hb1")],
            &[sample("ctx-1", "合成正文里出现了别急。")],
        );
        assert_eq!(report.checked, 1);
        assert_eq!(report.mismatches, 0);
        assert_eq!(report.mismatch_ratio(), Some(0.0));
        assert_eq!(report.items[0].status, QuoteAuditStatus::Matched);
        assert_eq!(report.items[0].mismatch(), Some(false));
    }

    #[test]
    fn banned_phrase_absent_reports_full_mismatch_ratio() {
        let report = audit_active_quotes(
            &[banned_card("hb2", "别急")],
            &[stat(VIOLATION_STAT_TYPE, "hb2")],
            &[sample("ctx-2", "合成正文没有声称命中的短语。")],
        );
        assert_eq!(report.checked, 1);
        assert_eq!(report.mismatches, 1);
        assert_eq!(report.mismatch_ratio(), Some(1.0));
        assert_eq!(report.items[0].status, QuoteAuditStatus::Mismatched);
        assert_eq!(report.items[0].mismatch(), Some(true));
    }

    #[test]
    fn required_phrase_absent_supports_violation() {
        let report = audit_active_quotes(
            &[required_card("hq1", "先问我")],
            &[stat(VIOLATION_STAT_TYPE, "hq1")],
            &[sample("ctx-3", "合成正文缺少必备短语。")],
        );
        assert_eq!(report.items[0].status, QuoteAuditStatus::Matched);
        assert_eq!(report.mismatches, 0);
    }

    #[test]
    fn required_phrase_present_contradicts_violation() {
        let report = audit_active_quotes(
            &[required_card("hq2", "先问我")],
            &[stat(VIOLATION_STAT_TYPE, "hq2")],
            &[sample("ctx-4", "合成正文写明了先问我。")],
        );
        assert_eq!(report.items[0].status, QuoteAuditStatus::Mismatched);
        assert_eq!(report.mismatches, 1);
    }

    #[test]
    fn card_without_violation_record_is_skipped() {
        let report = audit_active_quotes(
            &[banned_card("hs1", "别急")],
            &[],
            &[sample("ctx-5", "合成正文里出现了别急。")],
        );
        assert_eq!(report.checked, 0);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.mismatch_ratio(), None);
        assert_eq!(report.items[0].status, QuoteAuditStatus::Skipped);
    }

    #[test]
    fn no_sample_bodies_returns_unknown_without_estimating() {
        let report = audit_active_quotes(
            &[banned_card("hu1", "别急")],
            &[stat(VIOLATION_STAT_TYPE, "hu1")],
            &[],
        );
        assert_eq!(report.checked, 0);
        assert_eq!(report.unknown, 1);
        assert_eq!(report.mismatch_ratio(), None);
        assert_eq!(report.items[0].status, QuoteAuditStatus::Unknown);
        assert_eq!(report.items[0].mismatch(), None);
    }

    #[test]
    fn paragraph_limit_cards_are_not_phrase_audited() {
        let report = audit_active_quotes(
            &[card(
                "hp1",
                HabitScope::Global,
                Some(HabitCheck::ParagraphMaxChars { limit: 120 }),
                utc_iso8601(NOW),
            )],
            &[stat(VIOLATION_STAT_TYPE, "hp1")],
            &[sample("ctx-6", "合成正文")],
        );
        assert_eq!(report.items[0].status, QuoteAuditStatus::Skipped);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn mixed_cards_report_fractional_ratio() {
        let report = audit_active_quotes(
            &[banned_card("hm1", "别急"), banned_card("hm2", "稍等")],
            &[
                stat(VIOLATION_STAT_TYPE, "hm1"),
                stat(VIOLATION_STAT_TYPE, "hm2"),
            ],
            &[sample("ctx-7", "合成正文里出现了别急。")],
        );
        assert_eq!(report.checked, 2);
        assert_eq!(report.mismatches, 1);
        assert_eq!(report.mismatch_ratio(), Some(0.5));
    }

    #[test]
    fn quote_audit_stats_contain_card_flag_and_no_body() {
        let (store, base) = temp_store("record");
        let report = audit_active_quotes(
            &[banned_card("hr1", "别急")],
            &[stat(VIOLATION_STAT_TYPE, "hr1")],
            &[sample("ctx-8", "合成正文没有声称命中的短语。")],
        );
        record_quote_audits(&store, &report, NOW).unwrap();
        let rows = read_quote_audits(&store);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, STAT_QUOTE_AUDIT);
        assert_eq!(rows[0].id, "hr1");
        assert!(rows[0].mismatch);
        let raw = std::fs::read_to_string(store.stats_path()).unwrap();
        assert!(raw.contains("\"type\":\"quote_audit\""));
        assert!(raw.contains("\"mismatch\":true"));
        assert!(!raw.contains("别急"));
        let listing = store.read_stats();
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].kind, STAT_QUOTE_AUDIT);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn review_due_boundary_is_strict_at_ninety_days() {
        let cards = vec![
            card(
                "hd89",
                HabitScope::Global,
                None,
                utc_iso8601(NOW - 89.0 * SECS_PER_DAY as f64),
            ),
            card(
                "hd90",
                HabitScope::Global,
                None,
                utc_iso8601(NOW - 90.0 * SECS_PER_DAY as f64),
            ),
            card(
                "hd91",
                HabitScope::Global,
                None,
                utc_iso8601(NOW - 91.0 * SECS_PER_DAY as f64),
            ),
        ];
        let report = review_active_cards(&cards, &[], NOW);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].card_id, "hd91");
        assert!(report.items[0].flags.review_due);
        assert!(!report.items[0].flags.conflict);
    }

    #[test]
    fn inject_record_prevents_review_due() {
        let old = card(
            "hi1",
            HabitScope::Global,
            None,
            utc_iso8601(NOW - 200.0 * SECS_PER_DAY as f64),
        );
        let report = review_active_cards(&[old], &[stat(INJECT_STAT_TYPE, "hi1")], NOW);
        assert!(report.items.is_empty());
    }

    #[test]
    fn same_scope_banned_and_required_conflict_both_cards() {
        let scope = HabitScope::Scene {
            scene: "work".to_string(),
        };
        let left = card(
            "hc1",
            scope.clone(),
            Some(HabitCheck::BannedPhrases {
                phrases: vec!["别急".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let right = card(
            "hc2",
            scope,
            Some(HabitCheck::RequiredPhrases {
                phrases: vec!["别急".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let report = review_active_cards(&[left, right], &[], NOW);
        assert_eq!(report.items.len(), 2);
        assert!(report.items.iter().all(|item| item.flags.conflict));
        assert_eq!(report.items[0].card_id, "hc1");
        assert_eq!(report.items[1].card_id, "hc2");
    }

    #[test]
    fn cross_scope_phrases_do_not_conflict() {
        let left = card(
            "hx1",
            HabitScope::Global,
            Some(HabitCheck::BannedPhrases {
                phrases: vec!["别急".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let right = card(
            "hx2",
            HabitScope::Scene {
                scene: "work".to_string(),
            },
            Some(HabitCheck::RequiredPhrases {
                phrases: vec!["别急".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let report = review_active_cards(&[left, right], &[], NOW);
        assert!(report.items.is_empty());
    }

    #[test]
    fn multiple_cards_and_phrases_mark_only_intersecting_pairs() {
        let scope = HabitScope::Scene {
            scene: "work".to_string(),
        };
        let banned = card(
            "hm1",
            scope.clone(),
            Some(HabitCheck::BannedPhrases {
                phrases: vec!["别急".to_string(), "先听我说".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let required_a = card(
            "hm2",
            scope.clone(),
            Some(HabitCheck::RequiredPhrases {
                phrases: vec!["先听我说".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let required_b = card(
            "hm3",
            scope,
            Some(HabitCheck::RequiredPhrases {
                phrases: vec!["先列清单".to_string()],
            }),
            utc_iso8601(NOW),
        );
        let report = review_active_cards(&[banned, required_a, required_b], &[], NOW);
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[0].card_id, "hm1");
        assert_eq!(report.items[1].card_id, "hm2");
        assert!(report.items.iter().all(|item| item.flags.conflict));
    }

    #[test]
    fn empty_input_returns_empty_curator_report() {
        let report = review_active_cards(&[], &[], NOW);
        assert!(report.items.is_empty());
    }
}
