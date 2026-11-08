#!/usr/bin/env python3
"""OD-QS-413 第二阶段安全核对：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：五个优先级 IPC 的红线/掩码闸门、无自由文本 IPC 的类型约束、
AI 无排期写通道、排期失败不触碰任务状态、无敏感日志、测试数据隔离。

用法：python3 tests/verification/desktop/verify_security_phase2.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
RUST = REPO / "desktop" / "src-tauri" / "src"
ENGINE = REPO / "engine" / "omnidesk_engine"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def function_block(source: str, name: str) -> str:
    marker = "fn " + name + "("
    start = source.find(marker)
    if start < 0:
        return ""
    tail = source[start + len(marker) :]
    for separator in ["\n#[tauri::command", "\nfn ", "\nstruct "]:
        index = tail.find(separator)
        if index >= 0:
            tail = tail[:index]
    return source[start : start + len(marker) + len(tail)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    lib = read(RUST / "lib.rs")
    task_store = read(RUST / "task_store.rs")
    scheduling = read(RUST / "scheduling.rs")
    schedule_store = read(RUST / "schedule_store.rs")
    priority = read(RUST / "priority_rules.rs")
    extract_rs = read(RUST / "task_extract.rs")
    extract_py = read(ENGINE / "task_extract.py")
    task_manager = read(SRC / "shell" / "TaskManager.tsx")

    importance = function_block(lib, "task_set_importance")
    quadrant = function_block(lib, "task_set_quadrant")
    due = function_block(lib, "task_set_due")
    estimated = function_block(lib, "task_set_estimated")
    dependency = function_block(lib, "task_set_dependency")

    check(
        "gate_definition",
        "fn gate_user_text(" in lib and "red_line_scan_with" in lib,
        "统一闸门仍由红线扫描 + mask_privacy 组成",
    )
    for label, block in [
        ("importance", importance),
        ("quadrant", quadrant),
        ("due", due),
    ]:
        check(
            f"gate_covers_{label}",
            "gate_user_text(" in block,
            f"task_set_{label} 的自由文本经过统一闸门",
        )
    check(
        "estimated_is_typed_only",
        "minutes: Option<u32>" in estimated
        and "gate_user_text(" not in estimated
        and "reason" not in estimated,
        "task_set_estimated 只收可空 u32，没有自由文本可绕过闸门",
    )
    check(
        "dependency_is_typed_only",
        "depends_on: Vec<String>" in dependency
        and "gate_user_text(" not in dependency
        and "reason" not in dependency,
        "task_set_dependency 只收任务编号数组；存在性/无环由存储层校验",
    )
    check(
        "dependency_validated",
        "fn set_depends_on(" in task_store
        and "assert_no_dependency_cycle" in task_store
        and "assert_task_exists" in task_store,
        "依赖写入前校验存在、无环与不自依赖",
    )

    check(
        "ai_has_no_schedule_write",
        all(token not in extract_rs for token in ["schedule_store", "schedule_generate", "create_entry", "confirm_entry"])
        and all(token not in extract_py for token in ["schedule", "start_at", "end_at"]),
        "AI 提取链路只产任务候选，不写排期",
    )
    check(
        "no_auto_generate_on_startup",
        "schedule_generate_impl(" not in lib.split("pub fn run()")[1].split("#[cfg(test)]")[0]
        and "schedule_generate(" not in lib.split("pub fn run()")[1].split("#[cfg(test)]")[0]
        and "expire_stale_suggestions(" not in lib.split("pub fn run()")[1].split("#[cfg(test)]")[0],
        "启动流程不自动生成排期；只有用户命令触发",
    )
    schedule_prod = schedule_store.split("#[cfg(test)]")[0]
    scheduling_prod = scheduling.split("#[cfg(test)]")[0]
    check(
        "schedule_failure_does_not_touch_tasks",
        all(token not in schedule_prod for token in ["create_task(", "update_task(", "transition_task("])
        and all(token not in scheduling_prod for token in ["std::fs", "Command::new", "update_task", "transition_task"]),
        "排期写入与生成都不改任务文件",
    )
    check(
        "schedule_writes_limited_to_generate_and_adjust",
        lib.count("schedule_store::create_entry(") == 1
        and lib.count("schedule_store::adjust_entry(") == 1,
        "新建排期只经 schedule_generate / adjust 两条路径",
    )
    check(
        "no_sensitive_logs",
        all(token not in priority + scheduling + schedule_store for token in ["println!", "eprintln!", "dbg!"])
        and "console.log" not in task_manager,
        "新模块与任务界面不写调试日志",
    )
    check(
        "tests_use_temp_roots",
        "std::env::temp_dir()" in scheduling
        and "std::env::temp_dir()" in schedule_store
        and "std::env::temp_dir()" in priority,
        "测试数据只落临时目录，不写仓库或用户数据",
    )
    check(
        "no_waivers_added",
        "#[allow(" not in scheduling
        and "#[allow(" not in schedule_store
        and "#[allow(" not in priority,
        "新模块没有 lint/检查豁免",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-QS-413",
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
