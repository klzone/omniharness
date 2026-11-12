#!/usr/bin/env python3
"""OD-WR-509 工作记录分页双页签改造：静态契约验证。

证据等级：静态源码检查。不读用户数据、不调用模型、不开窗口。

覆盖：两页签与页头、右栏概览/复盘入口/快捷操作、时间线卡片、记录详情、
当日摘要、周期复盘生成入口、审核浮层、任务关联弹窗，以及旧页签已收起的边界。

用法：python3 tests/verification/desktop/verify_wr_509.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
SHELL = SRC / "shell"

CASES: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    page = read(SHELL / "WorkRecordPage.tsx")
    library = read(SHELL / "WorkRecordLibrary.tsx")
    archive = read(SHELL / "ReviewArchive.tsx")
    dialog = read(SHELL / "RecordTaskLinkDialog.tsx")
    review_panel = read(SHELL / "TaskReviewPanel.tsx")
    theme = read(SRC / "theme.css")

    # 1. 页头与两个页签
    check("tabs_two_only", 'key: "records"' in page and 'key: "reviews"' in page)
    check(
        "tabs_old_removed",
        not any(token in page for token in ('key: "tasks"', 'key: "weekly"', 'key: "review"')),
    )
    check(
        "tab_labels",
        'label: "工作记录"' in page and 'label: "周期复盘"' in page,
    )
    check("hero_title", "工作记录" in page and "wr-hero-title" in page)
    check("hero_sub", "记录工作轨迹，沉淀思考与成长" in page)
    check(
        "hero_month_count",
        "data-month-count={monthCount}" in page
        and "本月记录" in page
        and "onOverview={(overview) => setMonthCount(overview.month)}" in page,
    )
    check("tabs_icons", 'icon: "clock"' in page and 'icon: "trend-up"' in page)

    # 2. 右栏概览与四个快捷操作
    check("overview_card", "本月工作概览" in library and 'data-work-overview=""' in library)
    check(
        "overview_stats_three",
        'data-overview-stat="total"' in library
        and 'data-overview-stat="analyzed"' in library
        and 'data-overview-stat="pending"' in library,
    )
    check(
        "overview_filters_real_data",
        "focusOverview(\"analyzed\")" in library
        and "focusOverview(\"unanalyzed\")" in library
        and "applyMonth(todayLocal().slice(0, 7))" in library,
    )
    check("month_review_cta", "一键生成 · 本月复盘" in library)
    check(
        "quick_weekly",
        'data-quick-action="weekly-review"' in library
        and 'requestNewThread("weekly", "周复盘")' in library,
    )
    check(
        "quick_monthly",
        'data-quick-action="monthly-review"' in library
        and 'requestNewThread("weekly", "复盘本月")' in library,
    )
    check(
        "quick_organize_selected",
        'data-quick-action="organize"' in library
        and "disabled={!selected}" in library
        and "请整理工作记录 #" in library,
    )
    check(
        "quick_link_task",
        'data-quick-action="link-task"' in library and "openTaskLink" in library,
    )
    check("home_hint", "你可以在首页对话中，让 AI 帮你整理工作记录" in library)

    # 3. 时间线与卡片七要素、当日摘要
    check("timeline_grid", "wr-records-grid" in library and "wr-side" in library)
    check("timeline_groups", "groupRecordsByDate(listing.items)" in library)
    check("timeline_rail", ".od-record-group-head::before" in theme)
    check(
        "card_attrs_kept",
        all(
            token in library
            for token in (
                'data-record-row={record.id}',
                'data-record-card-title=""',
                'data-record-card-time=""',
                'data-record-card-category=""',
                'data-record-card-preview=""',
                'data-record-card-source=""',
                'data-record-linked-count=""',
                "data-record-status={record.ai_analysis_status}",
                'data-record-draft=""',
            )
        ),
    )
    check(
        "summary_kept",
        'data-daily-summary-generate=""' in library
        and 'data-daily-summary-view=""' in library
        and 'data-record-group-summary={group.date}' in library
        and 'data-daily-summary-record={id}' in library,
    )
    check(
        "filters_kept",
        all(
            token in library
            for token in (
                "from_date",
                "to_date",
                "category",
                "keywordDraft",
                "ai_analysis_status",
                "只看草稿",
            )
        ),
    )

    # 4. 记录详情与任务关联
    check(
        "detail_kept",
        "<MarkdownText text={selected.body}" in library
        and ".workRecordUpdate(" in library
        and ".workRecordDelete(" in library
        and 'data-raw-input=""' in library
        and 'data-revision-history=""' in library,
    )
    check(
        "detail_backlinks_kept",
        'data-record-backlinks=""' in library
        and 'data-record-backlink={item.kind}' in library,
    )
    check(
        "task_link_section",
        'data-record-task-links=""' in library and "管理关联" in library,
    )
    check(
        "task_link_dialog",
        "RecordTaskLinkDialog" in library and ".taskList({})" in dialog,
    )
    check(
        "task_link_writes_only_links",
        "shell.workRecordUpdate(record.id, { linked_task_ids: ids })" in dialog
        and "taskUpdate" not in dialog
        and "taskState" not in dialog,
    )
    check("task_link_missing_marked", "任务已不存在" in dialog and "缺失" in dialog)

    # 5. 审核浮层
    check(
        "audit_overlay",
        'data-work-audit=""' in page and "TaskReviewPanel" in page and "embedded" in page,
    )
    check(
        "audit_open_from_actions",
        "onOpenAudit" in library and 'data-open-audit=""' in library,
    )
    check(
        "audit_reloads_after_close",
        "setRefreshToken((value) => value + 1)" in page
        and "refreshToken" in library,
    )
    check(
        "panel_embedded_layout",
        "embedded?: boolean" in review_panel
        and "min-h-0 flex-1 overflow-y-auto px-4 pb-4" in review_panel,
    )

    # 6. 周期复盘页签
    check("review_archive_mounted", "<ReviewArchive" in page)
    check(
        "review_generate_menu",
        'data-review-generate-entry=""' in archive
        and 'data-review-generate={item.key}' in archive,
    )
    check(
        "review_generate_keywords",
        all(
            token in archive
            for token in ('"周复盘"', '"复盘本月"', '"复盘本季度"', '"复盘今年"')
        ),
    )
    check(
        "review_preefill_no_autosend",
        "requestNewThread(" in archive
        and "host.submitTask(" not in archive.split("function generate(period: PeriodReviewPeriod)")[1].split("function exportMarkdown")[0],
    )
    check(
        "review_detail_kept",
        "worklogPeriodReviewList" in archive
        and "<MarkdownText text={detail.body}" in archive
        and "worklogPeriodReviewGenerate" in archive,
    )
    check(
        "review_archive_surface",
        "wr-review-grid" in theme and "wr-review-list" in archive and "wr-review-detail" in archive,
    )

    # 7. 视觉层
    check(
        "visual_tokens",
        all(
            name in theme
            for name in (
                ".wr-hero",
                ".wr-tabbar",
                ".wr-records-grid",
                ".wr-side-card",
                ".wr-stat-grid",
                ".wr-review-cta",
                ".wr-quick-grid",
                ".wr-drawer",
                ".wr-dialog",
            )
        ),
    )
    check(
        "neumorphic_surfaces",
        "var(--od-neu-raise-2)" in theme
        and "var(--od-neu-inset-1)" in theme
        and "linear-gradient" in theme,
    )

    failed = [case for case in CASES if not case["ok"]]
    payload = {
        "plan": "OD-WR-509",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CASES),
        "passed_count": len(CASES) - len(failed),
        "failed": failed,
        "cases": CASES,
    }
    for case in CASES:
        print(("PASS  " if case["ok"] else "FAIL  ") + case["name"])
    print(f"\n{len(CASES) - len(failed)}/{len(CASES)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
