#!/usr/bin/env python3
"""OD-WR-303 工作记录页面：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：页面接线与三个视图、列表筛选、快速记录、详情渲染/编辑/删除/解析入口、
关联任务数与 Aetheria 卡面。运行时行为由 npm build 与后续人工界面验收覆盖。

用法：python3 tests/verification/desktop/verify_work_record_ui_301.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
RUST = REPO / "desktop" / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    app = read(SRC / "App.tsx")
    page = read(SRC / "shell" / "WorkRecordPage.tsx")
    library = read(SRC / "shell" / "WorkRecordLibrary.tsx")
    store = read(RUST / "worklog_records.rs")

    check(
        "app_routes_report_to_work_record_page",
        'scene === "report" ?' in app and "<WorkRecordPage" in app,
        "App 把工作记录场景接到新页面",
    )
    check(
        "page_has_work_views",
        all(
            key in page
            for key in ('key: "records"', 'key: "review"', 'key: "tasks"', 'key: "weekly"')
        ),
        "记录 / 审核 / 任务 / 周视图四个视图",
    )
    check(
        "page_mounts_library",
        "<WorkRecordLibrary" in page and "<TaskManager" in page,
        "页面挂载记录库与任务管理",
    )
    check(
        "list_reads_host",
        "shell" in library and ".workRecordList(filter)" in library,
        "列表走宿主 IPC（含筛选）",
    )
    check(
        "filters_complete",
        all(
            token in library
            for token in ("from_date", "to_date", "category", "keyword", "ai_analysis_status")
        ),
        "日期、分类、关键词、解析状态筛选齐全",
    )
    check(
        "quick_record_writes_host",
        ".workRecordCreate({" in library and "record_date: quickDate" in library,
        "快速记录经宿主写盘并带可选日期",
    )
    check(
        "draft_toggle_present",
        "quickDraft" in library and "只看草稿" in library,
        "快速记录可标草稿，列表可按草稿筛选",
    )
    check(
        "detail_renders_markdown",
        "<MarkdownText text={selected.body}" in library,
        "详情正文按 Markdown 渲染",
    )
    check(
        "detail_edit_and_delete",
        ".workRecordUpdate(" in library and ".workRecordDelete(" in library,
        "详情支持编辑与删除（进回收目录）",
    )
    check(
        "parse_entry_wired_to_analyze",
        "data-parse-entry" in library
        and ".workRecordAnalyzePrepare(" in library
        and ".workRecordAnalyzeApply(" in library
        and ".workRecordAnalyzeFailed(" in library
        and "host.submitTask(" in library,
        "解析入口已接上 304 的三段 IPC 与引擎提交",
    )
    check(
        "linked_task_count_visible",
        "linked_task_ids.length" in library,
        "列表与详情显示关联任务数",
    )
    check(
        "host_sorts_by_date",
        "record_date" in store and "b.record_date" in store,
        "宿主按记录日期排序，界面不另造顺序",
    )
    check(
        "aetheria_card_surface",
        "bg-white/80" in library and "rounded-[20px]" in library and "text-on-surface" in library,
        "沿用 Aetheria 浅色卡面与靛青主色",
    )
    check(
        "load_error_is_visible",
        "读取记录失败" in library and "role=\"status\"" in library,
        "读盘失败原样提示，不显示成空列表",
    )
    check(
        "no_english_subtitles",
        not any(word in library + page for word in ("WORK RECORDS", "QUICK NOTE", "DRAFTS")),
        "不再叠英文副标题",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-303",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
