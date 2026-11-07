#!/usr/bin/env python3
"""OD-ARCH-141 S6：整理收件箱入口与引用展示的静态选择器检查。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PANEL = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"
HOME = REPO / "desktop/src/shell/HomeChat.tsx"
VAULT = REPO / "desktop/src/shell/KnowledgeVaultPage.tsx"
API = REPO / "desktop/src/api.ts"
TYPES = REPO / "desktop/src/types.ts"
INGEST_RS = REPO / "desktop/src-tauri/src/knowledge_ingest.rs"
LIB_RS = REPO / "desktop/src-tauri/src/lib.rs"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    panel = PANEL.read_text(encoding="utf-8") if PANEL.is_file() else ""
    home = HOME.read_text(encoding="utf-8")
    vault = VAULT.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    ingest_rs = INGEST_RS.read_text(encoding="utf-8")
    lib_rs = LIB_RS.read_text(encoding="utf-8")
    check("panel_exists", PANEL.is_file())
    for selector in ("knowledge-ingest-button", "knowledge-ingest-panel", "knowledge-ingest-report"):
        check("selector_" + selector.replace("-", "_"), 'data-testid="%s"' % selector in panel)
    check("panel_has_run_button", "开始整理" in panel and "重新扫描" in panel)
    # OD-ARCH-146 S3：整理入口从对话工具行迁到知识库页面。
    check("home_no_longer_renders_panel", "<KnowledgeIngestPanel" not in home)
    check("vault_renders_panel", "<KnowledgeIngestPanel onApplied=" in vault)
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
    # OD-KB-201 S1：raw 准入的宿主接线（前端入口在本阶段保持不变）。
    check("raw_dir_is_initialized", 'pub const RAW_DIR: &str = "raw"' in ingest_rs)
    check("raw_admit_backend_present", "pub fn admit_raw_files" in ingest_rs)
    check("raw_admit_command_registered", "fn knowledge_raw_admit" in lib_rs and "knowledge_raw_admit," in lib_rs)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
