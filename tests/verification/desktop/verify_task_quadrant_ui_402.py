#!/usr/bin/env python3
"""OD-QS-410 任务四象限页 + 旧占位替换：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

用法：python3 tests/verification/desktop/verify_task_quadrant_ui_402.py [--json PATH]
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
    scene_page = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    week_timeline = (SRC / "weekTimeline.ts").read_text(encoding="utf-8")
    timeline_component = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")

    check(
        "old_placeholder_deleted",
        not (SRC / "weeklyQuadrants.ts").exists()
        and not (SRC / "shell" / "WeeklyQuadrants.tsx").exists(),
        "weeklyQuadrants.ts 与 WeeklyQuadrants.tsx 已删除",
    )
    referenced = []
    for path in SRC.rglob("*.ts*"):
        if "weeklyQuadrants" in path.name or "WeeklyQuadrants" in path.name:
            continue
        text = path.read_text(encoding="utf-8")
        if "weeklyQuadrants" in text or "WeeklyQuadrants" in text:
            referenced.append(str(path.relative_to(SRC)))
    check("no_reference_left", not referenced, f"无引用残留：{referenced}")

    check(
        "quadrant_board_present",
        "export function TaskQuadrantBoard" in task_manager
        and "data-task-quadrant" in task_manager,
        "TaskManager 提供四象限面板",
    )
    check(
        "four_plus_pending_columns",
        all(item in task_manager for item in ['key: "Q1"', 'key: "Q2"', 'key: "Q3"', 'key: "Q4"', 'key: "待评估"'])
        and "重要且紧急" in task_manager
        and "重要不紧急" in task_manager,
        "Q1–Q4 + 待评估均以文字标题呈现",
    )
    check(
        "scene_page_uses_task_board",
        'from "./TaskManager"' in scene_page
        and "<TaskQuadrantBoard />" in scene_page
        and "<WeeklyQuadrants" not in scene_page,
        "ScenePage 四象限区改用任务象限视图",
    )
    check(
        "quadrant_data_from_host",
        "taskQuadrantViews" in task_manager and "task_quadrant_views" not in task_manager,
        "象限数据经 api 客户端读取宿主派生结果",
    )
    check(
        "manual_adjust_uses_ipc",
        "taskSetImportance" in task_manager and "taskSetQuadrant" in task_manager,
        "手动调整走 405 IPC",
    )
    check(
        "task_rows_simplified",
        "截止 {task.due_date}" in task_manager and "<StateChip" in task_manager,
        "任务行显示标题、状态与截止日期",
    )
    check(
        "report_destination_moved",
        'export const REPORT_DESTINATION = "本周报引用"' in week_timeline
        and 'REPORT_DESTINATION' in timeline_component
        and 'from "../weekTimeline"' in timeline_component,
        "REPORT_DESTINATION 移至 weekTimeline 且时间线改从那里引用",
    )
    check(
        "no_color_only",
        all(label in task_manager for label in ["重要且紧急", "重要不紧急", "紧急不重要", "不重要不紧急"]),
        "象限以文字标题标识，不只靠颜色",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-QS-410",
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
