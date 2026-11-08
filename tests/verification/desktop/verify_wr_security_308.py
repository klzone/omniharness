#!/usr/bin/env python3
"""OD-WR-308 安全断言静态核对（不读用户数据、不调用模型）。

覆盖：红线/掩码接线、AI 无写通道、候选只进宿主暂存、端点默认关闭、
错误不回显原文、测试数据虚构边界、任务摘要注入上限。

用法：python3 tests/verification/desktop/verify_wr_security_308.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
ENGINE = REPO / "engine" / "omnidesk_engine"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    lib = read(RUST / "lib.rs")
    store = read(RUST / "task_store.rs")
    extract = read(RUST / "task_extract.rs")
    settings = read(RUST / "settings.rs")
    admission = read(RUST / "admission.rs")
    py = read(ENGINE / "task_extract.py")
    web = read(ENGINE / "web_api_adapter.py")

    check(
        "record_write_gate",
        "fn gate_user_text" in lib
        and "work_record_create_impl" in lib
        and "body, extra) = gate_user_text(&draft.body" in lib,
        "工作记录写入前统一过红线与掩码",
    )
    check(
        "task_write_gate",
        "task_append_progress_impl" in lib
        and "task_transition_impl" in lib
        and "task_candidate_link" in lib
        and "gate_user_text" in lib,
        "任务标题/进展/备注/关联进展都有闸门",
    )
    check(
        "analyze_prepare_gate",
        "worklog_record_analyze" in lib
        and "gate_user_text(&record.title" in lib
        and "gate_user_text(&record.body" in lib,
        "送模型前对记录标题与正文再过一次闸门",
    )
    check(
        "engine_submit_gate",
        "assert_input_allowed_with" in lib and "profile_rules::mask_privacy(&prompt_text" in lib,
        "引擎提交唯一汇合点仍有红线与掩码",
    )
    check(
        "engine_datamasker_wired",
        "DataMasker" in web and "session.masker.mask" in web,
        "引擎提交链路已接 DataMasker（既有 Task 4）",
    )
    check(
        "ai_has_no_write_channel",
        "open(" not in py
        and "Path(" not in py
        and "write" not in py
        and "fs" not in py,
        "task_extract.py 只组装提示词，无文件读写",
    )
    check(
        "candidate_staging_host_only",
        "stage_candidates" in extract and "stage_candidates" in store,
        "候选只由宿主校验后暂存",
    )
    check(
        "endpoint_default_closed",
        "pub local: bool" in settings
        and "pub private: bool" in settings
        and "pub public: bool" in settings
        and "model_permissions_default_closed_and_persist_per_category" in settings,
        "三类端点权限默认关闭且有单测",
    )
    check(
        "red_line_error_no_echo",
        "detail.push_str(&format!(\"{}（{} 处）；\", hit.label, hit.count))" in lib,
        "红线拒绝只报类别与数量，不回显原文",
    )
    check(
        "route_admission_wired",
        "endpoint_unconfirmed" in admission and "permissions" in admission,
        "端点准入与权限判定仍在提交前",
    )
    check(
        "task_summary_capped",
        "MAX_CONTEXT_TASKS" in extract and ".take(MAX_CONTEXT_TASKS)" in extract,
        "注入未完成任务摘要有上限",
    )
    check(
        "mask_receipts_count_only",
        "masked_id_card" in lib and "masked_phone" in lib,
        "回执只回传掩码类别与数量",
    )
    check(
        "no_real_material_in_new_modules",
        "用户成果" not in store
        and "用户成果" not in extract
        and "用户成果" not in py
        and "regulations" not in py,
        "新模块与测试不引用真实材料目录",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-308",
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
