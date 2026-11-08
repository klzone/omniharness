#!/usr/bin/env python3
"""知识库与材料写作页 UI 美化静态回归（2026-11-08）。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DASH = REPO / "desktop/src/shell/KnowledgeDashboard.tsx"
REVIEW = REPO / "desktop/src/shell/KnowledgeReviewCenter.tsx"
PAGE = REPO / "desktop/src/shell/KnowledgeVaultPage.tsx"
WRITING = REPO / "desktop/src/shell/WritingStudio.tsx"
SCENE = REPO / "desktop/src/shell/ScenePage.tsx"
STRIP = REPO / "desktop/src/shell/SceneStatusStrip.tsx"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    dash = DASH.read_text(encoding="utf-8")
    review = REVIEW.read_text(encoding="utf-8")
    page = PAGE.read_text(encoding="utf-8")
    writing = WRITING.read_text(encoding="utf-8")
    scene = SCENE.read_text(encoding="utf-8")
    strip = STRIP.read_text(encoding="utf-8")

    # 驾驶舱：指标带 + 主次双列 + 健康/检索真数据。
    check("dashboard_metric_band", "divide-y divide-outline-variant/20" in dash and "lg:divide-x" in dash)
    check("dashboard_duo_ratio", "minmax(0,1.35fr)_minmax(0,1fr)" in dash)
    check("dashboard_health_and_search", "dashboard-health-card" in dash and "dashboard-retrieval" in dash)

    # 审核：主从列表 + 右详情 + 固定动作条。
    check("review_master_detail", "lg:grid-cols-[340px_minmax(0,1fr)]" in review and "selectedKey" in review)
    check("review_queue_rows", "review-item" in review and "conflict-item" in review)
    check("review_fixed_actions", "review-approve" in review and "review-reject" in review)

    # 资产：两栏默认 + 1360 三栏 + 可折叠检查器 + 正文宽度。
    check("asset_two_pane_default", "lg:grid-cols-[minmax(230px,280px)_minmax(0,1fr)]" in page)
    check("asset_wide_inspector", "min-[1400px]:grid-cols-[minmax(250px,300px)_minmax(0,1fr)_minmax(280px,330px)]" in page)
    check("asset_inspector_toggle", "vault-inspector-toggle" in page and "vault-inspector" in page)
    check("asset_reading_width", "max-w-[860px]" in page)

    # 写作页：与知识库同一浅色卡面、靛青主色，不恢复暖纸/青绿。
    check("writing_card_unified", "bg-white/78" in writing and "rounded-[20px]" in writing)
    check("writing_no_warm_paper", "#fbf9f4" not in writing and "#fbf9f4" not in strip)
    check("writing_primary_accent", "accent=\"primary\"" in scene)
    check("writing_metric_unified", "bg-white/78" in strip and "border-white/90" in strip)

    # 周报 / 工作记录 / 首页不参与本次改造：保留原有分支与组件。
    check("weekly_branch_preserved", 'scene === "weekly" ? (' in scene and "WeeklyQuadrants" in scene)
    check("worklog_components_preserved", "WeeklyQuadrants" in scene and "worklog" in scene)
    check("home_not_in_scope", "HomeChat" not in scene or 'scene === "writing"' in scene)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
