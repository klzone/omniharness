#!/usr/bin/env python3
"""OD-QS-411 今日 / 本周计划页：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

用法：python3 tests/verification/desktop/verify_schedule_ui_403.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    task_manager = (SRC / "shell" / "TaskManager.tsx").read_text(encoding="utf-8")

    check(
        "today_and_week_tabs",
        '{ key: "today", label: "今日" }' in task_manager
        and '{ key: "week", label: "本周" }' in task_manager,
        "任务页有今日/本周页签",
    )
    check(
        "planner_uses_host_data",
        "shell.scheduleList()" in task_manager
        and "shell.availabilityGet()" in task_manager
        and "shell.taskList()" in task_manager,
        "计划页从宿主读取排期、工作时间与任务",
    )
    check(
        "regenerate_flow",
        "shell.scheduleGenerate" in task_manager
        and "重新生成建议" in task_manager,
        "有计划重新生成入口",
    )
    check(
        "stale_prompt",
        "任务已变化，建议重新生成" in task_manager
        and "listing?.stale" in task_manager,
        "任务变化后提示重新生成",
    )
    check(
        "today_capacity_and_windows",
        "剩余容量" in task_manager and "可用时段" in task_manager and "daily_capacity_hours" in task_manager,
        "今日显示可用时段与剩余容量",
    )
    check(
        "confirmed_and_suggested_groups",
        '(["已确认", "已建议"] as const)' in task_manager
        and "scheduleConfirm" in task_manager,
        "今日区分已确认与 AI 建议，建议可确认",
    )
    check(
        "conflicts_surface",
        "report.conflicts" in task_manager and "冲突" in task_manager,
        "冲突显式显示",
    )
    check(
        "week_seven_day_distribution",
        "function weekDays()" in task_manager
        and "xl:grid-cols-7" in task_manager
        and "周日" in task_manager,
        "本周显示七日分布",
    )
    check(
        "week_cards",
        all(item in task_manager for item in ["本周截止", "未排期的重要任务", "逾期风险"]),
        "本周显示截止、未排期重要任务与逾期风险",
    )
    check(
        "honest_empty_state",
        "暂无" in task_manager and "还没有生成过排期建议" in task_manager,
        "空态真实，不造数",
    )
    check(
        "local_timezone_offset",
        "LOCAL_OFFSET_MINUTES" in task_manager
        and "scheduleGenerate(LOCAL_OFFSET_MINUTES())" in task_manager,
        "生成建议时显式传本地时区偏移",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-QS-411",
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
