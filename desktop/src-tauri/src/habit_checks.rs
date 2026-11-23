//! 习惯检查（OD-GROW-301 H3）。
//!
//! 读取 G1 生效卡（status=active）里带 check 的规则，在复盘确认与写作审阅
//! 路径上做软检查：违规只追加 `habits-stats.jsonl` 的 `violation` 行并随回执
//! 返回，不阻止原流程，也不把正文写进报告或统计。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::habit_cards::{HabitCheck, HabitScope, HabitStore, STATUS_ACTIVE};
use crate::task_shell::now_secs;

/// 统计行类型：习惯检查命中。
pub const VIOLATION_STAT_TYPE: &str = "violation";

pub const KIND_BANNED_PHRASES: &str = "banned_phrases";
pub const KIND_PARAGRAPH_MAX_CHARS: &str = "paragraph_max_chars";
pub const KIND_REQUIRED_PHRASES: &str = "required_phrases";

pub const DETAIL_BANNED_PHRASE_MATCH: &str = "banned_phrase_match";
pub const DETAIL_PARAGRAPH_OVER_LIMIT: &str = "paragraph_over_limit";
pub const DETAIL_REQUIRED_PHRASE_MISSING: &str = "required_phrase_missing";

/// 单条违规。只含卡片编号、检查类型与原因代码，不含正文或命中的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitCheckViolation {
    pub card_id: String,
    pub kind: String,
    pub detail_code: String,
}

/// 检查回执。`checked_cards` 只列真正参与检查的生效卡编号。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitCheckReport {
    pub checked_cards: Vec<String>,
    pub violations: Vec<HabitCheckViolation>,
}

impl HabitCheckReport {
    pub fn empty() -> Self {
        Self::default()
    }
}

/// 按场景读取生效卡并执行三类检查。
///
/// `scope_scene` 为 `None` 时只检查 global 卡；`Some(scene)` 时额外检查
/// scope.scene 与之一致的卡。存储读取失败（例如目录损坏）按空报告处理，
/// 不让习惯检查阻断复盘或写作主流程。
pub fn check_habits(
    scope_scene: Option<&str>,
    body: &str,
    store: &HabitStore,
) -> HabitCheckReport {
    let Ok(listing) = store.list_active() else {
        return HabitCheckReport::empty();
    };
    let paragraphs = paragraphs_of(body);
    let mut report = HabitCheckReport::empty();
    for card in listing.items {
        if card.status != STATUS_ACTIVE {
            continue;
        }
        let Some(check) = card.check.as_ref() else {
            continue;
        };
        if !scope_matches(&card.scope, scope_scene) {
            continue;
        }
        report.checked_cards.push(card.id.clone());
        let Some((kind, detail_code)) = evaluate(check, body, &paragraphs) else {
            continue;
        };
        let violation = HabitCheckViolation {
            card_id: card.id.clone(),
            kind: kind.to_string(),
            detail_code: detail_code.to_string(),
        };
        // 统计失败不回滚检查结论，也不阻断复盘/写作主流程；只追加，不存正文。
        let _ = store.append_stat(
            VIOLATION_STAT_TYPE,
            Some(&card.id),
            Some(detail_code),
            now_secs(),
        );
        report.violations.push(violation);
    }
    report
}

/// 由工作根推断习惯存储位置（只读探测，不创建目录）。
///
/// 优先使用 `OMNIDESK_APP_DATA_ROOT`；默认应用布局下回落到工作根的
/// 同级 `app/habits`；最后兼容工作根本身位于应用根内的布局。
pub fn store_for_work_root(work_root: &Path) -> Option<HabitStore> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(raw) = std::env::var_os("OMNIDESK_APP_DATA_ROOT") {
        let app_root = PathBuf::from(raw);
        if app_root.is_absolute() {
            candidates.push(app_root.join("habits"));
        }
    }
    if let Some(parent) = work_root.parent() {
        candidates.push(parent.join("app").join("habits"));
    }
    candidates.push(work_root.join("habits"));
    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .map(HabitStore::for_root)
}

fn scope_matches(scope: &HabitScope, scope_scene: Option<&str>) -> bool {
    match scope {
        HabitScope::Global => true,
        HabitScope::Scene { scene } => scope_scene == Some(scene.as_str()),
    }
}

fn evaluate(
    check: &HabitCheck,
    body: &str,
    paragraphs: &[String],
) -> Option<(&'static str, &'static str)> {
    match check {
        HabitCheck::BannedPhrases { phrases } => {
            let hit = phrases.iter().any(|phrase| body.contains(phrase.as_str()));
            hit.then_some((KIND_BANNED_PHRASES, DETAIL_BANNED_PHRASE_MATCH))
        }
        HabitCheck::ParagraphMaxChars { limit } => {
            let limit = *limit as usize;
            let over = paragraphs
                .iter()
                .any(|paragraph| paragraph.chars().count() > limit);
            over.then_some((KIND_PARAGRAPH_MAX_CHARS, DETAIL_PARAGRAPH_OVER_LIMIT))
        }
        HabitCheck::RequiredPhrases { phrases } => {
            let missing = phrases
                .iter()
                .any(|phrase| !body.contains(phrase.as_str()));
            missing.then_some((KIND_REQUIRED_PHRASES, DETAIL_REQUIRED_PHRASE_MISSING))
        }
    }
}

/// 按空行分段；段内保留换行，段首尾空白不影响计数。
fn paragraphs_of(body: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut paragraphs, &mut current);
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line.trim_end());
        }
    }
    push_paragraph(&mut paragraphs, &mut current);
    paragraphs
}

fn push_paragraph(paragraphs: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        paragraphs.push(trimmed.to_string());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::habit_cards::{
        HabitCard, HabitEvidence, HABIT_CARD_SCHEMA, KIND_PREFERENCE, ORIGIN_EXPLICIT,
    };
    use std::fs;

    const NOW: f64 = 1_800_000_000.0;

    fn temp_store(tag: &str) -> (HabitStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-habit-checks-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&base);
        (HabitStore::for_root(base.join("habits")), base)
    }

    fn card(id: &str, scope: HabitScope, check: HabitCheck) -> HabitCard {
        HabitCard {
            schema: HABIT_CARD_SCHEMA.to_string(),
            id: id.to_string(),
            kind: KIND_PREFERENCE.to_string(),
            scope,
            text: "合成习惯".to_string(),
            check: Some(check),
            evidence: HabitEvidence {
                thread_id: "thread-1".to_string(),
                message_id: "msg-1".to_string(),
                date: "2026-11-20".to_string(),
            },
            origin: ORIGIN_EXPLICIT.to_string(),
            status: STATUS_ACTIVE.to_string(),
            created_at: crate::task_shell::utc_iso8601(NOW),
            confirmed_at: Some(crate::task_shell::utc_iso8601(NOW)),
            last_seen_at: crate::task_shell::utc_iso8601(NOW),
            expect: None,
        }
    }

    fn write_active(store: &HabitStore, cards: &[HabitCard]) {
        let dir = store.root().join("active");
        fs::create_dir_all(&dir).unwrap();
        for card in cards {
            fs::write(
                dir.join(format!("{}.json", card.id)),
                serde_json::to_vec_pretty(card).unwrap(),
            )
            .unwrap();
        }
    }

    fn banned(id: &str, phrase: &str) -> HabitCard {
        card(
            id,
            HabitScope::Global,
            HabitCheck::BannedPhrases {
                phrases: vec![phrase.to_string()],
            },
        )
    }

    #[test]
    fn banned_phrases_hit_and_miss() {
        let (store, base) = temp_store("banned");
        write_active(&store, &[banned("hb", "合成禁用短语")]);
        let hit = check_habits(Some("report"), "正文包含合成禁用短语。", &store);
        assert_eq!(hit.checked_cards, vec!["hb".to_string()]);
        assert_eq!(hit.violations.len(), 1);
        assert_eq!(hit.violations[0].kind, KIND_BANNED_PHRASES);
        assert_eq!(hit.violations[0].detail_code, DETAIL_BANNED_PHRASE_MATCH);
        let miss = check_habits(Some("report"), "普通合成正文。", &store);
        assert!(miss.violations.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn paragraph_limit_passes_at_exact_length_and_fails_one_over() {
        let (store, base) = temp_store("paragraph");
        write_active(
            &store,
            &[card(
                "hp",
                HabitScope::Global,
                HabitCheck::ParagraphMaxChars { limit: 10 },
            )],
        );
        let exact = check_habits(None, "abcdefghij", &store);
        assert!(exact.violations.is_empty());
        let over = check_habits(None, "abcdefghijk", &store);
        assert_eq!(over.violations.len(), 1);
        assert_eq!(over.violations[0].kind, KIND_PARAGRAPH_MAX_CHARS);
        assert_eq!(over.violations[0].detail_code, DETAIL_PARAGRAPH_OVER_LIMIT);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn required_phrases_missing_and_complete() {
        let (store, base) = temp_store("required");
        write_active(
            &store,
            &[card(
                "hr",
                HabitScope::Global,
                HabitCheck::RequiredPhrases {
                    phrases: vec!["合成必备短语".to_string()],
                },
            )],
        );
        let missing = check_habits(None, "普通合成正文。", &store);
        assert_eq!(missing.violations.len(), 1);
        assert_eq!(missing.violations[0].kind, KIND_REQUIRED_PHRASES);
        assert_eq!(
            missing.violations[0].detail_code,
            DETAIL_REQUIRED_PHRASE_MISSING
        );
        let complete = check_habits(None, "正文含合成必备短语。", &store);
        assert!(complete.violations.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn scope_filters_global_scene_match_and_other_scene() {
        let (store, base) = temp_store("scope");
        write_active(
            &store,
            &[
                banned("hg", "合成全局短语"),
                card(
                    "hs",
                    HabitScope::Scene {
                        scene: "report".to_string(),
                    },
                    HabitCheck::BannedPhrases {
                        phrases: vec!["合成场景短语".to_string()],
                    },
                ),
                card(
                    "ho",
                    HabitScope::Scene {
                        scene: "material_writing".to_string(),
                    },
                    HabitCheck::BannedPhrases {
                        phrases: vec!["合成其他短语".to_string()],
                    },
                ),
            ],
        );
        let report = check_habits(Some("report"), "合成正文", &store);
        assert_eq!(
            report.checked_cards,
            vec!["hg".to_string(), "hs".to_string()]
        );
        assert!(report
            .checked_cards
            .iter()
            .all(|id| id != "ho"));
        let only_global = check_habits(None, "合成正文", &store);
        assert_eq!(only_global.checked_cards, vec!["hg".to_string()]);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn violation_is_appended_to_stats() {
        let (store, base) = temp_store("stats");
        write_active(&store, &[banned("hb", "合成禁用短语")]);
        let report = check_habits(None, "正文包含合成禁用短语。", &store);
        assert_eq!(report.violations.len(), 1);
        let stats = store.read_stats();
        assert_eq!(stats.skipped_invalid, 0);
        let row = stats
            .items
            .iter()
            .find(|row| row.kind == VIOLATION_STAT_TYPE)
            .expect("violation 行应已写入");
        assert_eq!(row.id.as_deref(), Some("hb"));
        assert_eq!(row.reason_code.as_deref(), Some(DETAIL_BANNED_PHRASE_MATCH));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn no_active_cards_yields_empty_report() {
        let (store, base) = temp_store("empty");
        let report = check_habits(Some("report"), "合成正文", &store);
        assert!(report.checked_cards.is_empty());
        assert!(report.violations.is_empty());
        assert_eq!(store.read_stats().items.len(), 0);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn report_does_not_contain_body_text() {
        let (store, base) = temp_store("no-body");
        write_active(&store, &[banned("hb", "合成禁用短语")]);
        let marker = "合成正文不应出现在报告";
        let body = format!("{marker}，并包含合成禁用短语。");
        let report = check_habits(None, &body, &store);
        let rendered = serde_json::to_string(&report).unwrap();
        assert!(!rendered.contains(marker), "{} ", rendered);
        assert!(!rendered.contains("合成禁用短语"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn disabled_or_checkless_cards_are_not_checked() {
        let (store, base) = temp_store("skip");
        let mut disabled = banned("hd", "合成禁用短语");
        disabled.status = crate::habit_cards::STATUS_DISABLED.to_string();
        let mut checkless = banned("hc", "合成禁用短语");
        checkless.check = None;
        write_active(&store, &[disabled, checkless]);
        let report = check_habits(None, "正文包含合成禁用短语。", &store);
        assert!(report.checked_cards.is_empty());
        assert!(report.violations.is_empty());
        let _ = fs::remove_dir_all(&base);
    }
}
