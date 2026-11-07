#!/usr/bin/env python3
"""OD-KB-201 S4：驾驶舱任务记录、过期检查与检索抽检接线。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PAGE = REPO / "desktop/src/shell/KnowledgeVaultPage.tsx"
DASH = REPO / "desktop/src/shell/KnowledgeDashboard.tsx"
API = REPO / "desktop/src/api.ts"
TYPES = REPO / "desktop/src/types.ts"
LIB = REPO / "desktop/src-tauri/src/lib.rs"
QUALITY = REPO / "desktop/src-tauri/src/knowledge_quality.rs"
TASK_CORE = REPO / "engine/omnidesk_engine/task_core.py"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    dash = DASH.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    quality = QUALITY.read_text(encoding="utf-8") if QUALITY.is_file() else ""
    task_core = TASK_CORE.read_text(encoding="utf-8")

    check("quality_module_exists", QUALITY.is_file())
    check("quality_threshold_config", "knowledge-quality.v1.json" in quality and "expiry_days" in quality)
    check("quality_default_threshold", "DEFAULT_EXPIRY_DAYS: i64 = 180" in quality)
    check("quality_uses_index_and_governance", "INDEX_REL" in quality and "KIND_RAW_ADMISSIONS" in quality)
    check("quality_no_model_call", "omnidesk_engine" not in quality and "Command::new" not in quality)
    check("quality_command_registered", "knowledge_quality_report," in lib)

    check("parent_loads_quality", "knowledgeQualityReport" in page and "loadQuality" in page)
    check("dashboard_receives_quality", "quality={quality}" in page and "quality={quality}" in page)
    check("dashboard_task_error", 'data-testid="dashboard-task-error"' in dash and "record.error_message" in dash)
    check("dashboard_expired_list", "dashboard-expired-list" in dash and "dashboard-expired-item" in dash)
    check("dashboard_expired_todo", "dashboard-expired-todo" in dash)
    check("dashboard_task_health_risks", "过期" in dash and "失败" in dash and "项风险" in dash)
    check("dashboard_retrieval", "dashboard-quality-search" in dash and "dashboard-quality-search-hit" in dash)
    check("retrieval_uses_vault_search", "knowledgeVaultSearch" in dash)
    check("no_vector_search", "embedding" not in dash and "向量" in dash)

    check("api_quality_report", "knowledgeQualityReport" in api)
    check("types_quality_report", "KnowledgeQualityReport" in types and "KnowledgeExpiredItem" in types)
    check("task_summary_failure_fields", '"error_code": self.error_code' in task_core and '"error_message": self.error_message' in task_core)
    check("task_summary_intent_private", '"intent": self.intent' not in task_core)

    check("no_quality_page_tab", "hub-tab-quality" not in page and "AI 运行与质量" not in page)
    check("three_tabs_only", "role=\"tablist\"" in page and 'data-testid={`hub-tab-${page}`}' in page and '["dashboard", "知识驾驶舱"]' in page and '["review", "知识审核"]' in page and '["assets", "知识资产"]' in page)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
