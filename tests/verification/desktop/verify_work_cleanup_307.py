#!/usr/bin/env python3
"""OD-WR-313 临时层清理：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：候选规则、draft/未完成引用/待审核候选保护、retention 配置、
预览零写入、apply 只移入 .trash、IPC 注册与设置页确认。

用法：python3 tests/verification/desktop/verify_work_cleanup_307.py [--json PATH]
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

    cleanup = read(RUST / "work_cleanup.rs")
    lib = read(RUST / "lib.rs")
    settings_panel = read(SRC / "SettingsPanel.tsx")
    api = read(SRC / "api.ts")

    check(
        "module_present",
        "pub fn cleanup_preview(" in cleanup and "pub fn cleanup_apply(" in cleanup,
        "清理模块提供预览与执行",
    )
    check(
        "retention_default_and_config",
        "DEFAULT_RETENTION_DAYS: i64 = 90" in cleanup
        and "retention.v1.json" in read(SRC / "SettingsPanel.tsx") + cleanup
        and 'RETENTION_FILE: &str = "retention.v1.json"' in cleanup,
        "保留期默认 90 天并可配置",
    )
    check(
        "records_from_archived_periods",
        "archived_periods(" in cleanup
        and "source_period" in cleanup
        and "已归档报告覆盖期" in cleanup,
        "记录只按已归档报告覆盖期入选",
    )
    check(
        "period_week_month_year",
        'split_once("-W")' in cleanup
        and "text.len() == 7" in cleanup
        and "text.len() == 4" in cleanup,
        "周/月/年周期都能解析",
    )
    check(
        "draft_protected",
        "if record.draft" in cleanup and "continue" in cleanup,
        "draft 记录永不入选",
    )
    check(
        "unfinished_task_reference_protected",
        "unfinished.contains(task_id)" in cleanup,
        "被未完成任务引用的记录永不入选",
    )
    check(
        "pending_candidate_protected",
        "pending.contains(&record.id)" in cleanup,
        "还有待审核候选的记录永不入选",
    )
    check(
        "tasks_done_or_cancelled_and_old",
        "STATE_DONE" in cleanup
        and "STATE_CANCELLED" in cleanup
        and "retention_days * 86_400" in cleanup,
        "任务只清理完成/取消且超过保留期",
    )
    check(
        "preview_is_read_only",
        "delete_record" not in cleanup.split("pub fn cleanup_apply(")[0]
        and "delete_task" not in cleanup.split("pub fn cleanup_apply(")[0],
        "预览路径不含移动/删除动作",
    )
    check(
        "apply_moves_to_trash",
        "worklog_records::delete_record" in cleanup
        and "task_store::delete_task" in cleanup
        and ".trash" in cleanup,
        "执行只移入回收目录",
    )
    check(
        "ipc_registered",
        "fn work_cleanup_preview" in lib
        and "fn work_cleanup_apply" in lib
        and "work_cleanup_preview," in lib
        and "work_cleanup_apply," in lib,
        "预览与执行 IPC 已注册",
    )
    check(
        "apply_requires_confirm",
        '"清理会移动记录与任务，必须显式确认' in lib,
        "执行必须显式确认",
    )
    check(
        "api_methods",
        "workCleanupPreview(" in api and "workCleanupApply(" in api,
        "前端客户端有对应方法",
    )
    check(
        "settings_confirm_flow",
        "预览可清理内容" in settings_panel
        and "我已确认：这些内容只会移到回收目录" in settings_panel,
        "设置页有预览与确认执行",
    )
    check(
        "no_startup_auto_cleanup",
        lib.count("work_cleanup::cleanup_apply(") == 1,
        "启动流程不自动清理",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-313",
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
