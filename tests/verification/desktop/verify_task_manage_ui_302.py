#!/usr/bin/env python3
"""OD-WR-305 任务管理页面：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：六状态筛选、列表与详情、来源跳转、进展与状态历史、状态变更与非法迁移
报错可见、Aetheria 卡面。运行时由 npm build 与人工界面验收覆盖。

用法：python3 tests/verification/desktop/verify_task_manage_ui_302.py [--json PATH]
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

STATES = ("待确认", "待开始", "进行中", "已完成", "已取消", "暂停")


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    page = read(SRC / "shell" / "WorkRecordPage.tsx")
    manager = read(SRC / "shell" / "TaskManager.tsx")
    store = read(RUST / "task_store.rs")

    check(
        "tab_mounts_task_manager",
        "<TaskManager" in page and 'tab === "tasks"' in page,
        "工作记录页的「任务」视图挂载任务管理",
    )
    check(
        "six_states_declared",
        all(state in manager for state in STATES) and "ALL_STATES" in manager,
        "六个状态全部出现在筛选与变更控件里",
    )
    check(
        "state_filter_chips",
        "setStateFilter(state)" in manager and "全部" in manager,
        "六状态筛选 + 全部",
    )
    check(
        "list_reads_host_with_filter",
        ".taskList(stateFilter ? { state: stateFilter } : {})" in manager,
        "列表按状态走宿主过滤",
    )
    check(
        "detail_shows_history_timeline",
        "data-task-history" in manager and "状态历史" in manager,
        "详情展示状态历史时间线",
    )
    check(
        "detail_lists_progress",
        "data-task-progress" in manager and "selected.progress" in manager,
        "详情展示进展列表",
    )
    check(
        "source_jump_wired",
        "source_record_id" in manager and "onOpenRecord(recordId)" in manager,
        "来源记录可跳回记录视图",
    )
    check(
        "transition_uses_host",
        ".taskTransition(" in manager,
        "状态变更经宿主白名单校验",
    )
    check(
        "illegal_transition_error_visible",
        "data-task-error" in manager and "notice.kind === \"err\"" in manager,
        "宿主拒绝的非法迁移原文显示在页面",
    )
    check(
        "progress_append_uses_host",
        ".taskAppendProgress(" in manager,
        "进展追加经宿主落盘",
    )
    check(
        "ui_mirrors_whitelist",
        "NEXT_STATES" in manager and "待确认: [" in manager,
        "界面镜像白名单，宿主仍是最终裁决",
    )
    check(
        "host_states_match",
        all('"' + state + '"' in store for state in STATES) and "STATE_PENDING_CONFIRM" in store,
        "宿主六状态常量齐全",
    )
    check(
        "aetheria_card_surface",
        "bg-white/80" in manager and "rounded-[20px]" in manager and "text-on-surface" in manager,
        "沿用 Aetheria 浅色卡面与靛青主色",
    )
    check(
        "load_error_is_visible",
        "读取任务失败" in manager,
        "读盘失败原样提示，不显示成空列表",
    )
    check(
        "no_english_subtitles",
        not any(word in manager for word in ("TASKS", "TO DO", "IN PROGRESS")),
        "不再叠英文副标题",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-305",
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
