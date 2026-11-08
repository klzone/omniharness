#!/usr/bin/env python3
"""OD-WR-311 周视图与周报改读记录库：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：结构化周视图 IPC、ScenePage 时间线与四象限改用记录条目、周报读取来源、
旧文件回退保留。

用法：python3 tests/verification/desktop/verify_week_from_records_306.py [--json PATH]
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

    quick = read(RUST / "quicklog.rs")
    lib = read(RUST / "lib.rs")
    scene = read(SRC / "shell" / "ScenePage.tsx")
    timeline = read(SRC / "shell" / "WeeklyTimeline.tsx")
    week = read(SRC / "weekTimeline.ts")
    api = read(SRC / "api.ts")
    chat = read(SRC / "shell" / "HomeChat.tsx")

    check(
        "structured_week_view_exists",
        "pub struct WeekView" in quick and "pub fn week_view(" in quick,
        "宿主提供结构化周视图",
    )
    check(
        "week_view_ipc",
        "fn worklog_week_view" in lib and "worklog_week_view," in lib,
        "结构化周视图 IPC 已注册",
    )
    check(
        "api_and_types",
        "worklogWeekView(" in api and "WorklogWeekView" in read(SRC / "types.ts"),
        "前端客户端与类型齐全",
    )
    check(
        "scene_uses_records",
        "shell.worklogWeekView()" in scene and "parseWeekView" in scene,
        "ScenePage 从记录条目构造时间线数据",
    )
    check(
        "quadrants_use_records",
        "countReportEntries(parsedWeek" in scene
        and "weekReportEntries={weekReportEntries}" in scene,
        "记录四象限占位已由任务象限替换；进周报统计仍消费同一份解析结果",
    )
    check(
        "timeline_accepts_parsed",
        "parsed?: ReturnType<typeof parseWorklog>" in timeline
        and "parsed ?? parseWorklog(content)" in timeline
        and "parsed={parsedWeek}" in scene,
        "时间线优先使用结构化条目",
    )
    check(
        "parse_week_view_helper",
        "export function parseWeekView(" in week and "WorklogWeekView" in week,
        "结构化条目可无损转成时间线结构",
    )
    check(
        "report_reads_records",
        '"source": "records"' in lib and "shell.worklogReadWeek()" in chat,
        "周报读取走记录库渲染，不再依赖周文件",
    )
    check(
        "records_priority_with_legacy_fallback",
        "const view = await shell.worklogWeekView();" in scene
        and "const legacy = await shell.worklogReadWeek();" in scene
        and '"source": "legacy"' in lib,
        "记录库优先，旧周文件仅作回退",
    )
    check(
        "archive_includes_record_weeks",
        "quicklog::list_record_weeks" in lib,
        "归档周列表合并记录库周",
    )
    check(
        "no_direct_week_file_dependency",
        "worklog::week_path" not in scene and "parseWorklog(parsedWeek" not in scene,
        "界面不再直接依赖周文件路径",
    )
    check(
        "empty_state_preserved",
        "暂无记录" in timeline,
        "无记录时仍如实显示空态",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-311",
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
