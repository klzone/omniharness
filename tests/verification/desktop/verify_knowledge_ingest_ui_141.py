#!/usr/bin/env python3
"""OD-ARCH-141 S6：整理收件箱入口与引用展示的静态选择器检查。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PANEL = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"
HOME = REPO / "desktop/src/shell/HomeChat.tsx"
API = REPO / "desktop/src/api.ts"
TYPES = REPO / "desktop/src/types.ts"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    panel = PANEL.read_text(encoding="utf-8") if PANEL.is_file() else ""
    home = HOME.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    check("panel_exists", PANEL.is_file())
    for selector in ("knowledge-ingest-button", "knowledge-ingest-panel", "knowledge-ingest-report"):
        check("selector_" + selector.replace("-", "_"), 'data-testid="%s"' % selector in panel)
    check("panel_has_run_button", "开始整理" in panel and "重新扫描" in panel)
    check("home_renders_panel", "<KnowledgeIngestPanel />" in home)
    check("home_renders_citations", 'data-testid="knowledge-citations"' in home)
    check("home_renders_citation_button", 'data-testid="knowledge-citation"' in home)
    check("home_opens_citation_preview", 'data-testid="knowledge-citation-preview"' in home)
    check("citation_uses_read_note", "knowledgeVaultReadNote(citation.relative_path)" in home)
    for command in ("knowledge_ingest_init", "knowledge_ingest_prepare", "knowledge_ingest_apply"):
        check("api_" + command, command in api)
    check("types_present", "KnowledgeIngestPrepare" in types and "KnowledgeIngestApplyReport" in types)
    check("retrieval_intent_wired", "detectKnowledgeIntent" in home and "buildKnowledgeContext" in home)
    check("retrieval_keyword_wired", "knowledgeSearchKeyword" in home and "filterKnowledgeHits" in home)
    check("ingest_scan_uses_inbox_text", "userInputText: prep.user_input_text" in panel)
    check("ingest_prepare_exposes_inbox_text", "user_input_text: string" in types)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
