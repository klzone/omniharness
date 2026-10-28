#!/usr/bin/env python3
"""OD-ARCH-102：项目跟进页 v3（本机事项 + 左栏双区）的静态断言。

证据等级：静态代码检查（读源码文本、正则、结构计数）+ Rust 数据层单测实跑
（cargo test --offline --lib project_items）。不开窗口、不读真实知识库。
窗口级证据见 preview_project_102.py。

用法：python3 tests/verification/desktop/verify_project_102.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import os
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

    board = strip_comments((SRC / "shell" / "ProjectBoard.tsx").read_text(encoding="utf-8"))
    scene = strip_comments((SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8"))
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    gantt = (SRC / "projectGantt.ts").read_text(encoding="utf-8")
    store = (TAURI / "project_items.rs").read_text(encoding="utf-8")
    lib = (TAURI / "lib.rs").read_text(encoding="utf-8")
    shell = (TAURI / "task_shell.rs").read_text(encoding="utf-8")

    # --- S01 数据层 ---
    check(
        "schema_constant",
        'PROJECT_ITEMS_SCHEMA: &str = "omnidesk.project-items.v1"' in store,
    )
    check("file_constant", 'PROJECT_ITEMS_FILE: &str = "project-items.v1.json"' in store)
    check("deny_unknown_fields", "deny_unknown_fields" in store)
    check("backup_written", '".bak"' in store)
    check("atomic_rename", "std::fs::rename(&temp, target)" in store)
    check("corrupt_falls_back", "主文件损坏" in store and "recovered_from_backup" in store)
    check("whole_batch_rejected", "整批" in store and "第 " in store)
    check("created_at_preserved", "created_at" in store and "updated_at" in store)
    check("max_items_bounded", "PROJECT_ITEMS_MAX_COUNT" in store)
    check(
        "store_tests",
        all(
            name in store
            for name in (
                "first_read_of_missing_file_is_empty_not_error",
                "saves_and_reads_back_in_order",
                "invalid_item_rejects_whole_batch",
                "duplicate_item_names_are_refused",
                "corrupt_main_file_falls_back_to_backup",
                "wrong_schema_is_refused",
                "created_at_is_preserved_and_updated_at_moves",
            )
        ),
    )

    # --- 校验函数从汇总里抽出，旧链路复用同一套规则 ---
    for fn in (
        "validate_project_owner",
        "validate_project_due",
        "validate_project_start",
        "validate_project_status",
    ):
        check("task_shell_" + fn, "pub fn " + fn in shell)
    check("summary_reuses_owner_check", "validate_project_owner(line_no, cells[1])?" in shell)

    # --- 命令注册 ---
    check("lib_module", "pub mod project_items;" in lib)
    check("read_command", "fn project_items_read(" in lib)
    check("save_command", "fn project_items_save(" in lib)
    check("commands_registered", "project_items_read," in lib and "project_items_save," in lib)

    # --- 前端类型与 API ---
    check("types_record", "export interface ProjectItemRecord" in types)
    check("types_listing", "export interface ProjectItemsListing" in types)
    check("api_read", 'invoke<ProjectItemsListing>("project_items_read")' in api)
    check("api_save", '"project_items_save"' in api)
    check("overdue_computed_not_stored", "overdue = due !== null" in gantt)
    check("records_to_items", "export function recordsToItems" in gantt)

    # --- S02 页面版式 ---
    check("board_two_column", "lg:grid-cols-[360px_minmax(0,1fr)]" in board)
    check("left_has_projects_section", 'aria-label="项目事项"' in board)
    check("left_has_history_section", 'aria-label="历史对话"' in board)
    check("history_inside_left_column", "data-project-history" in board)
    check(
        "project_history_subtitle",
        "history: \"HISTORY\"" in (SRC / "scenes.ts").read_text(encoding="utf-8"),
    )
    check("props_use_records", "records: ProjectItemRecord[]" in board)
    check("create_button", "+ 新增" in board)
    check(
        "form_fields",
        all(
            x in board
            for x in (
                'aria-label="事项"',
                'aria-label="负责人代号"',
                'aria-label="期限"',
                'aria-label="下一步"',
            )
        ),
    )
    check("edit_action", 'data-project-action="edit"' in board)
    check("delete_action", 'data-project-action="delete"' in board)
    check("delete_confirm", "<ConfirmDialog" in board and 'testId="project-delete-dialog"' in board)
    check(
        "client_validation",
        "开始日期不能晚于期限" in board and "负责人代号用 P 加数字" in board,
    )
    check("saves_through_callback", "await onSave(" in board)

    # --- 右栏：空态引导、切换保留 ---
    check("empty_state_guide", "在左侧录入第一条项目事项后，这里会出现项目时间线" in board)
    check("empty_hides_axis", 'data-project-gantt-empty=""' in board)
    check("artifacts_switch", '"artifacts"' in board and "阶段成果" in board)
    check("gantt_rows_kept", "data-gantt-row={bar.kind}" in board and "rotate-45" in board)

    # --- 指标卡隐藏（仅 project） ---
    check("no_metric_strip_for_project", 'scene === "project" ? null : (' in scene)
    check("weekly_still_has_strip", "weekEntries={weekEntries}" in scene)
    check("project_reads_items", ".projectItemsRead()" in scene)
    check("project_saves_items", ".projectItemsSave(" in scene)
    check("project_summary_effect_removed", ".projectSummary(itemsText, null)" not in scene)
    check(
        "project_history_not_duplicated",
        'scene === "weekly" || scene === "writing" || scene === "project" ? null' in scene,
    )
    check("scene_passes_records", "records={projectRecords}" in scene)

    # --- 回归护栏 ---
    check("weekly_quadrants_kept", (SRC / "shell" / "WeeklyQuadrants.tsx").is_file())

    # --- 数据层单测实跑 ---
    # 工具链脚本输出的是 shell 片段（含 $PATH 展开），按 shell 执行而不是逐行解析。
    unit = subprocess.run(
        [
            "bash",
            "-lc",
            'eval "$(bash /mnt/data/Code/MyWorkStation/scripts/setup-desktop-toolchain.sh --print-env)"'
            ' && cd /mnt/data/Code/MyWorkStation/desktop/src-tauri'
            ' && cargo test --offline --lib project_items',
        ],
        capture_output=True,
        text=True,
        timeout=900,
    )
    store_run = re.search(r"test result: ok\. (\d+) passed; 0 failed", unit.stdout or "")
    ok_unit = unit.returncode == 0 and store_run is not None and int(store_run.group(1)) >= 9
    check("store_unit_tests_pass", ok_unit, (unit.stdout or unit.stderr)[-500:])

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-102",
        "passed": passed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail.strip() if detail and not ok else ""))
    print("\n" + str(len(CHECKS) - len(failed)) + "/" + str(len(CHECKS)) + " 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
