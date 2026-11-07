#!/usr/bin/env python3
"""OD-KB-201 S3：知识库三页结构、审核流、资产只读与 raw 上传接线。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PAGE = REPO / "desktop/src/shell/KnowledgeVaultPage.tsx"
DASH = REPO / "desktop/src/shell/KnowledgeDashboard.tsx"
REVIEW = REPO / "desktop/src/shell/KnowledgeReviewCenter.tsx"
INGEST = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"
API = REPO / "desktop/src/api.ts"
TYPES = REPO / "desktop/src/types.ts"
LIB = REPO / "desktop/src-tauri/src/lib.rs"
REVIEW_RS = REPO / "desktop/src-tauri/src/knowledge_review.rs"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    dash = DASH.read_text(encoding="utf-8")
    review = REVIEW.read_text(encoding="utf-8")
    ingest = INGEST.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    review_rs = REVIEW_RS.read_text(encoding="utf-8") if REVIEW_RS.is_file() else ""

    for tab in ("dashboard", "review", "assets"):
        check("tab_" + tab, 'data-testid={`hub-tab-${page}`}' in page and f'["{tab}",' in page)
    check("no_graph_first_level", "vault-view-graph" not in page and "vault-view-notes" not in page)
    check("graph_only_from_dashboard", "onOpenGraph" in page and "dashboard-graph-view" in page and "graph-back-dashboard" in page)

    for stat in ("raw", "wiki", "review", "conflict", "task-fail"):
        check("stat_" + stat, f'"dashboard-stat-{stat}"' in dash)
    check("dashboard_duo", 'data-testid="dashboard-duo"' in dash and "dashboard-todo-card" in dash and "dashboard-ai-work-card" in dash)
    for health in ("source", "association", "index", "task"):
        check("health_" + health, f'data-testid={{`dashboard-health-${{cell.id}}`}}' in dash and '"source"' in dash)
    check("empty_recent_is_honest", "暂无运行记录。" in dash)

    check("review_two_queues", "review-queue-tab-new" in review and "review-queue-tab-conflict" in review)
    check("review_expand_detail", "review-detail" in review and "conflict-detail" in review)
    check("review_source_quote", "review-source-quote" in review and "knowledgeVaultReadNote(path)" in review)
    for action in ("review-approve", "review-request-changes", "review-reject"):
        check("review_action_" + action, action in review)
    check("review_decision_uses_host", "knowledgeReviewDecide" in page and "knowledgeConflictResolve" in page)

    check("asset_readonly_default", 'setMode("preview")' in page)
    check("asset_explicit_edit", "vault-edit-start" in page and 'setMode("edit")' in page)
    check("asset_wiki_and_raw_groups", "Wiki 知识条目" in page and "raw/ 原始资料" in page)
    check("asset_page_root", 'data-testid="knowledge-assets"' in page)

    check("raw_upload_entry", "vault-raw-upload" in page and "knowledgeRawAdmitPaths" in page)
    check("raw_upload_dialog", "openFileDialog" in page and "multiple: true" in page)
    check("ingest_stages_not_applies", "knowledgeReviewStage" in ingest and "knowledgeIngestApply" not in ingest)

    for name in ("knowledgeRawAdmitPaths", "knowledgeGovernanceList", "knowledgeReviewStage", "knowledgeReviewList", "knowledgeReviewDecide", "knowledgeConflictResolve"):
        check("api_" + name, name in api)
    check("types_review_added", "KnowledgeReviewItem" in types and "KnowledgeRawAdmitReport" in types and "KnowledgeGovernanceRecord" in types)

    for command in ("knowledge_raw_admit_paths", "knowledge_governance_list", "knowledge_review_stage", "knowledge_review_list", "knowledge_review_decide", "knowledge_conflict_resolve"):
        check("registered_" + command, command + "," in lib)
    check("review_approve_uses_existing_apply", "knowledge_ingest::apply_payload" in review_rs)
    check("review_stage_writes_governance", "propose" in review_rs and "KIND_REVIEW_DECISIONS" in review_rs)
    check("no_governance_write_ipc", "knowledge_governance_append" not in api and "knowledge_governance_write" not in api)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
