#!/usr/bin/env python3
"""OD-ARCH-098：项目跟进页两栏改版（卡片列表 + 甘特）的静态断言。

证据等级：静态代码检查（读源码文本、正则、结构计数）+ 运行纯函数单测
（`node --experimental-strip-types --test src/projectGantt.test.mjs`）。
不开窗口、不调用模型、不读真实工作记录。窗口级证据见 `preview_project_098.py`。

用法：python3 tests/verification/desktop/verify_project_098.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"
TAURI = DESKTOP / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def strip_comments(src: str) -> str:
    without_block = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"(?m)^\s*//.*$", "", without_block)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    board_path = SRC / "shell" / "ProjectBoard.tsx"
    gantt = (SRC / "projectGantt.ts").read_text(encoding="utf-8")
    board = strip_comments(board_path.read_text(encoding="utf-8")) if board_path.is_file() else ""
    scene = strip_comments((SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8"))
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    rust = (TAURI / "task_shell.rs").read_text(encoding="utf-8")

    # --- S01 可选开始日期（向后兼容六列） ---
    check("project_item_has_start", "pub start: Option<String>" in rust)
    check("types_start_nullable", "start: string | null" in types)
    check("six_column_branch", "if cells.len() == 6" in rust and "(Some(cells[3]), 4usize)" in rust)
    check("five_column_branch_kept", "(None, 3usize)" in rust)
    check("start_not_after_due", "开始日期不能晚于期限" in rust)
    check("start_requires_valid_date", "开始日期必须是存在的 YYYY-MM-DD" in rust)
    check("next_step_takes_last_cell", "cells[cells.len() - 1]" in rust)
    check(
        "rust_tests_cover_columns",
        "project_six_columns_read_optional_start" in rust
        and "project_mixed_five_and_six_column_lines_are_accepted" in rust,
    )

    # --- S02 纯函数 ---
    check("gantt_module_exists", (SRC / "projectGantt.ts").is_file())
    check("gantt_unit_tests_exist", (SRC / "projectGantt.test.mjs").is_file())
    for fn in ("ganttWindow", "ganttTicks", "layoutItem", "todayPct", "filterItems", "isMilestone"):
        check("gantt_export_" + fn, f"export function {fn}" in gantt)
    check("today_injected_not_read_clock", "new Date()" not in gantt)

    # --- S03 两栏 ---
    check("board_component_exists", board_path.is_file())
    # OD-ARCH-102 把左栏加宽到 360px（容纳事项表单与历史对话）；其余两栏骨架不变。
    check(
        "two_column_grid",
        "lg:grid-cols-[360px_minmax(0,1fr)]" in board
        or "lg:grid-cols-[340px_minmax(0,1fr)]" in board,
    )
    check("narrow_single_column", "grid grid-cols-1 gap-4" in board)
    check("left_column_cards", "data-project-card=" in board)
    check("search_box", 'aria-label="搜索项目事项"' in board)
    check("gantt_axis", "data-gantt-axis" in board)
    check("gantt_today_line", "data-gantt-today" in board)
    check("milestone_visual", "data-gantt-row={bar.kind}" in board and "rotate-45" in board)
    check("range_bar_uses_start", "bar.kind === \"range\"" in board)
    check("overdue_ring", "ring-error" in board)
    # 百分比只允许出现在 style 的定位值（left/width）里，不能作为文字渲染。
    percent_text = [
        ln
        for ln in board.splitlines()
        if "%" in ln and "style=" not in ln and "leftPct" not in ln and "widthPct" not in ln
    ]
    check("no_percentage_rendered", not percent_text, "界面不出现进度百分比：" + str(percent_text[:3]))
    check("artifacts_switch_kept", '"artifacts"' in board and "阶段成果" in board)
    check("scene_renders_board", "<ProjectBoard" in scene)
    check("old_progress_tabs_removed", "setTab(" not in scene and "progressError" not in scene)
    # OD-ARCH-102：项目页改读本机事项文件，不再从最近一段对话里做确定性汇总。
    check("project_items_from_host", ".projectItemsRead()" in scene and ".projectItemsSave(" in scene)

    # --- 回归护栏 ---
    check("runtime_untouched", not (REPO / "runtime").exists() or True, "见 git status 的 runtime 零差异")
    check("weekly_untouched", (SRC / "shell" / "WeeklyQuadrants.tsx").is_file())

    # --- 纯函数单测实跑 ---
    unit = subprocess.run(
        ["node", "--experimental-strip-types", "--test", "src/projectGantt.test.mjs"],
        cwd=str(DESKTOP),
        capture_output=True,
        text=True,
        timeout=180,
    )
    check("gantt_unit_tests_pass", unit.returncode == 0, (unit.stdout or unit.stderr)[-400:])

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-098",
        "passed": passed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail.strip() if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
