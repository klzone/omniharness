#!/usr/bin/env python3
"""OD-KB-201 S5：真实端到端三缺陷的静态回归断言。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
INGEST_RS = REPO / "desktop/src-tauri/src/knowledge_ingest.rs"
ENGINE_SPEC = REPO / "engine/omnidesk_engine/knowledge_ingest.py"
SKILL = REPO / "skills/knowledge-search/SKILL.md"
AGENT_SKILL = REPO / "skills/knowledge-agent/SKILL.md"
HUB = REPO / "desktop/src/shell/knowledgeHub.ts"
HOME = REPO / "desktop/src/shell/HomeChat.tsx"
RETRIEVAL = REPO / "desktop/src/knowledgeRetrieval.ts"
RETRIEVAL_TEST = REPO / "desktop/src/knowledgeRetrieval.test.mjs"
HUB_TEST = REPO / "desktop/src/knowledgeHub.test.mjs"
REVIEW_RS = REPO / "desktop/src-tauri/src/knowledge_review.rs"
REVIEW_UI = REPO / "desktop/src/shell/KnowledgeReviewCenter.tsx"
INGEST_PANEL = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    ingest = INGEST_RS.read_text(encoding="utf-8")
    engine = ENGINE_SPEC.read_text(encoding="utf-8")
    skill = SKILL.read_text(encoding="utf-8")
    agent_skill = AGENT_SKILL.read_text(encoding="utf-8")
    hub = HUB.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    retrieval = RETRIEVAL.read_text(encoding="utf-8")
    retrieval_test = RETRIEVAL_TEST.read_text(encoding="utf-8")
    hub_test = HUB_TEST.read_text(encoding="utf-8")
    review_rs = REVIEW_RS.read_text(encoding="utf-8")
    review_ui = REVIEW_UI.read_text(encoding="utf-8")
    ingest_panel = INGEST_PANEL.read_text(encoding="utf-8")

    # A：规范与校验同口径。
    check("spec_sources_inbox_raw", '"sources": ["inbox/来源文件.md 或 raw/来源文件.pdf"]' in engine)
    check("spec_says_batch_sources", "inbox/ 或 raw/" in engine and "知识库/原文/" in engine)
    check("spec_body_320", "正文不超过 320 字" in engine)
    check("skill_body_320", "正文 ≤ 320 字" in skill)
    check("skill_sources_inbox", "sources: [inbox/xxx.md]" in skill)
    check("agent_skill_body_320", "不超过 320 字" in agent_skill and "原标题（上）/（中）/（下）" in agent_skill)
    check("host_body_420", "pub const CARD_MAX_CHARS: usize = 420" in ingest)
    check("split_suffix_consistent", all("原标题（上）/（中）/（下）" in text for text in (engine, skill)))
    check("split_see_reference", "参见" in engine and "参见" in skill)
    check("split_self_limit", "拆出的每张卡正文同样 ≤320 字" in engine and "拆出的每张卡正文同样 ≤320 字" in skill and "拆出的每张卡正文同样 ≤320 字" in agent_skill)
    check("host_sources_inbox_raw", 'f.relative_path.starts_with("inbox/")' in ingest and 'f.relative_path.starts_with("raw/")' in ingest)
    check("batch_includes_raw_notes", "fn select_ingest_notes" in ingest and 'file.relative_path.starts_with("raw/")' in ingest)
    check("batch_inbox_first_raw_newest", "inbox.sort_by" in ingest and "b.modified" in ingest and "truncate(5)" in ingest)
    check("archive_rewrites_sources", "rewrite_archived_sources" in ingest and "validation.archive_moves" in ingest)
    check("archive_rewrite_test", "sources: [知识库/原文/合成制度.md]" in ingest)
    check("raw_source_test", "validator_accepts_raw_attachment_as_source" in ingest)
    check("partial_stage_keeps_valid_cards", "validation.cards.is_empty()" in review_rs and "rejected_cards: validation.rejected_cards.clone()" in review_rs)
    check("review_shows_skipped_cards", "review-stage-warnings" in review_ui and "item.rejected_cards" in review_ui)
    check("ingest_poll_waits_900", "i < 900" in ingest_panel and "不会自动取消" in ingest_panel)
    check("host_auto_split", "fn auto_split_overlong" in review_rs and "宿主自动拆分" in review_rs)
    check("split_links_normalized", "fn normalize_split_links" in review_rs and "拆分卡基名链接" in review_rs)
    check("stage_fills_missing_sources", "fill_missing_sources" in review_rs and "prep.files.map" in ingest_panel)
    check("processed_raw_excluded", "approve_raw" in ingest and "skipped_processed" in ingest)
    check("processed_old_cards_excluded", "approved_card_sources" in ingest and "approve_card" in ingest)
    check("processed_raw_count_ui", "skipped_processed" in ingest_panel and "已处理跳过" in ingest_panel)
    check("processed_raw_approval_record", "action: \"approve_raw\"" in review_rs)

    # B：raw 索引行不进审核冲突队列。
    check("hub_excludes_raw_index_links", "endsWith(\"知识库/索引.md\")" in hub and "startsWith(\"raw/\")" in hub)
    check("hub_exclusion_test", "索引.md 指向 raw/ 的未解析链接不算知识冲突" in hub_test)

    # C：检索 notice 区分卡片与原始资料行。
    check("retrieval_notice_helper", "knowledgeNoticeText" in retrieval)
    check("home_uses_notice_helper", "knowledgeNoticeText" in home and "cardHits" in home)
    check("notice_test", "S5：检索 notice 区分知识卡与原始资料/索引行" in retrieval_test)

    # 附加：旧档案重复 key 的显示修复。
    check("message_ids_unique", "normalizeLoadedMessages" in home and "Date.now().toString(36)" in home)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
