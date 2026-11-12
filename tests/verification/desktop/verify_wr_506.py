#!/usr/bin/env python3
"""OD-WR-506 复盘档案页与记录反链：静态契约验证。

证据等级：
  * 静态源码检查：Tab 挂载、档案列表/筛选、详情要素、反链区块、导出与
    已确认重生成禁用、IPC 注册与只读扫描边界；
  * 运行证据由验收命令单独提供：tsc/build/cargo test 的实际输出落在
    tests/verification/desktop/od-wr-506/。

用法：python3 tests/verification/desktop/verify_wr_506.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
TAURI = REPO / "desktop" / "src-tauri" / "src"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    page = (SRC / "shell" / "WorkRecordPage.tsx").read_text(encoding="utf-8")
    archive = (SRC / "shell" / "ReviewArchive.tsx").read_text(encoding="utf-8")
    library = (SRC / "shell" / "WorkRecordLibrary.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    lib = (TAURI / "lib.rs").read_text(encoding="utf-8")
    host = (TAURI / "review_store.rs").read_text(encoding="utf-8")

    # 1. Tab 挂载与既有审核语义隔离
    check("tab_reviews_declared", '{ key: "reviews", label: "周期复盘", icon: "trend-up" }' in page)
    check(
        "tab_review_kept",
        "<TaskReviewPanel" in page and "embedded" in page and 'data-work-audit=""' in page,
    )
    check("tab_archive_mounted", "<ReviewArchive" in page and "ReviewArchive" in page.split("<ReviewArchive")[0])
    check("tab_focus_review_state", "focusReview" in page and "setFocusReview" in page)
    check("tab_open_review_from_records", "onOpenReview={(target)" in page)
    check("tab_record_jump_kept", "setFocusRecordId(recordId)" in page)

    # 2. 档案列表与筛选
    check("archive_lists_host", "worklogPeriodReviewList" in archive)
    check("archive_period_chips", "PERIOD_FILTERS" in archive and "data-review-period-filter" in archive)
    check("archive_period_values", all(v in archive for v in ('weekly', 'monthly', 'quarterly', 'annual')))
    check("archive_status_chips", "data-review-status-filter" in archive and '"草稿"' in archive and '"已确认"' in archive)
    check("archive_range_filter", "data-review-from" in archive and "data-review-to" in archive)
    check("archive_range_match", "item.end < fromDate" in archive and "item.start > toDate" in archive)
    check("archive_empty_state", "data-review-empty" in archive and "还没有复盘文档" in archive)
    check("archive_broken_notice", "data-review-broken" in archive and "listing.broken" in archive)
    check("archive_row_three_facts", "PERIOD_LABEL[item.period]" in archive and "item.range" in archive and "item.status" in archive)

    # 3. 详情要素
    check("detail_read_host", "worklogPeriodReviewRead" in archive)
    check("detail_markdown", "<MarkdownText text={detail.body}" in archive)
    check("detail_status_and_times", "data-review-detail-status" in archive and "generated_at" in archive and "confirmed_at" in archive)
    check("detail_record_chips", "data-review-record={id}" in archive and "onOpenRecord(id)" in archive)
    check("detail_task_list", "data-review-task={id}" in archive and "taskList" in archive)
    check("detail_stale", "data-review-stale" in archive and "missing_record_ids" in archive)
    check("detail_export", "data-review-export" in archive and "anchor.download" in archive and "text/markdown" in archive)
    check("detail_export_name", 'selected.start + "_" + selected.end + ".md"' in archive)

    # 4. 草稿重新生成与已确认禁用
    check("regen_uses_existing_chain", "worklogPeriodReviewCollect" in archive and "composeConversationPrompt" in archive and "worklogPeriodReviewGenerate" in archive)
    check("regen_uses_submit", "host.submitTask" in archive and "host.taskResult" in archive)
    check("regen_draft_only", 'detail.status !== "草稿"' in archive)
    check("regen_disabled_confirmed", "disabled={busy || detail.status !== \"草稿\"}" in archive)
    check("regen_confirmed_hint", "已确认复盘不可覆盖" in archive and "data-review-confirmed-lock" in archive)
    check("regen_guard_before_submit", "if (detail.status !== \"草稿\")" in archive)

    # 5. 记录反链
    check("backlink_api_called", "worklogRecordBacklinks" in library)
    check("backlink_block", "data-record-backlinks" in library and "关联复盘 / 文稿" in library)
    check("backlink_rows", "data-record-backlink={item.kind}" in library)
    check("backlink_review_jump", "onOpenReview({" in library and "setTab(\"reviews\")" in page)
    check("backlink_empty", "data-backlinks-empty" in library)
    check("backlink_error", "data-backlinks-error" in library)
    check("backlink_broken", "backlinks.broken" in library)

    # 6. IPC / 类型 / 只读边界
    check("api_list", "worklogPeriodReviewList" in api and "worklog_period_review_list" in api)
    check("api_backlinks", "worklogRecordBacklinks" in api and "worklog_record_backlinks" in api)
    check("api_backlinks_camel", "recordId" in api)
    check("types_listing", "PeriodReviewListing" in types and "ReviewArchiveItem" in types)
    check("types_backlinks", "RecordBacklinks" in types and "RecordBacklink" in types)
    check("ipc_commands", "fn worklog_period_review_list" in lib and "fn worklog_record_backlinks" in lib)
    check("ipc_registered", "worklog_period_review_list," in lib and "worklog_record_backlinks," in lib)
    check("host_list_fn", "pub fn list_reviews" in host)
    check("host_backlink_fn", "pub fn record_backlinks" in host)
    check("host_backlink_reviews", "front.record_ids.iter().any(|id| id == record_id)" in host)
    check("host_backlink_writing", "SOURCE_WORK_RECORD" in host and "selected_material_ids" in host)
    check("host_backlink_validation", "assert_record_id(record_id)" in host)

    backlink_block = host.split("pub fn record_backlinks", 1)[1].split("pub fn confirm_review", 1)[0]
    check(
        "host_backlink_read_only",
        "write_replace" not in backlink_block and "write_new" not in backlink_block and "fs::write" not in backlink_block,
    )
    check("host_write_path_unchanged", "已确认，不允许重新生成覆盖" in host and "backup_existing" in host)
    check("host_broken_tolerant", "broken += 1" in host and "list_reviews" in host)
    check("host_tests_min", host.count("#[test]") >= 22, str(host.count("#[test]")))
    for name in (
        "list_reviews_reads_documents_and_counts_broken",
        "list_reviews_skips_backups_and_unknown_periods",
        "record_backlinks_lists_referencing_reviews_only",
        "record_backlinks_follows_writing_selected_materials",
        "record_backlinks_counts_broken_review_files",
        "empty_listing_and_bad_record_id_is_rejected",
    ):
        check("host_test:" + name, "fn " + name in host)

    failed = [case for case in cases if not case["ok"]]
    result = {
        "plan": "OD-WR-506",
        "evidence_level": "静态源码检查（运行证据见同目录 README 与命令输出）",
        "passed": not failed,
        "total": len(cases),
        "passed_count": len(cases) - len(failed),
        "failed": failed,
        "cases": cases,
    }
    for case in cases:
        print(("PASS  " if case["ok"] else "FAIL  ") + case["name"])
    print(f"\n{len(cases) - len(failed)}/{len(cases)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
