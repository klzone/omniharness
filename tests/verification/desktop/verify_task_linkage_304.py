#!/usr/bin/env python3
"""OD-WR-307 记录 ↔ 任务联动：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：解析注入未完成任务摘要、候选关联建议字段、宿主校验、采纳关联追加进展与
双向 ID 回写、拒绝关联保留候选、状态建议只读、三日记事归一任务的用例。

用法：python3 tests/verification/desktop/verify_task_linkage_304.py [--json PATH]
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

    extract = read(RUST / "task_extract.rs")
    store = read(RUST / "task_store.rs")
    lib = read(RUST / "lib.rs")
    py = read(REPO / "engine" / "omnidesk_engine" / "task_extract.py")
    panel = read(SRC / "shell" / "TaskReviewPanel.tsx")
    api = read(SRC / "api.ts")

    check(
        "prompt_injects_unfinished_tasks",
        '"tasks": task_rows' in extract
        and "unfinished_tasks(" in extract
        and "tasks: Sequence[Mapping[str, Any]]" in py
        and "已有未完成任务" in py,
        "解析提示词注入未完成任务摘要",
    )
    check(
        "candidate_match_fields",
        all(
            field in store
            for field in ("match_task_id", "match_confidence", "match_reason", "suggested_state")
        ),
        "候选携带关联建议字段",
    )
    check(
        "host_validates_match_target",
        "tasks.iter().find" in extract and "is_unfinished_state" in extract,
        "宿主只接受未完成任务作为关联目标",
    )
    check(
        "suggested_state_is_suggestion_only",
        "is_allowed_transition" in extract
        and "不会自动应用" in panel
        and ".taskTransition(" not in panel,
        "状态建议校验合法性但不自动应用",
    )
    check(
        "link_appends_progress",
        "append_progress(root, &task_id, &text, now)" in store,
        "采纳关联先追加进展",
    )
    check(
        "bidirectional_writeback",
        "linked_record_ids.push" in store
        and "record.linked_task_ids.push" in store
        and "linked_task_ids: Some" in store,
        "任务与记录双向 ID 回写",
    )
    check(
        "dismiss_match_keeps_pending",
        "pub fn dismiss_match" in store
        and "match_task_id = None" in store
        and "taskCandidateDismissMatch" in panel,
        "拒绝关联后候选仍在待审核",
    )
    check(
        "review_ui_match_section",
        "data-candidate-match" in panel
        and "match_confidence" in panel
        and "关联置信度低" in panel,
        "审核台显示关联目标、置信度与低置信提示",
    )
    check(
        "review_ui_link_action",
        "taskCandidateLink" in panel
        and "采纳关联" in panel
        and "data-candidate-link" in panel,
        "审核台可采纳关联并追加进展",
    )
    check(
        "review_ui_source_and_candidate_reject",
        "data-candidate-source" in panel and "拒绝候选" in panel,
        "来源跳转与拒绝候选动作保留",
    )
    check(
        "batch_confirm_does_not_link",
        "taskCandidateConfirm(id, {})" in panel and "taskCandidateLink" not in panel.split("confirmBatch")[1].split("async function")[0],
        "批量采纳不会绕过关联建议",
    )
    check(
        "ipc_registered",
        "fn task_candidate_link" in lib
        and "fn task_candidate_dismiss_match" in lib
        and "task_candidate_link," in lib
        and "task_candidate_dismiss_match," in lib,
        "关联采纳与拒绝关联 IPC 已注册",
    )
    check(
        "api_methods",
        "taskCandidateLink(" in api and "taskCandidateDismissMatch(" in api,
        "前端客户端有对应方法",
    )
    check(
        "three_day_single_task_test",
        "three_day_records_link_one_task_without_losing_progress" in store,
        "三日记事归一到同一任务的用例存在",
    )
    check(
        "progress_history_preserved",
        "三天的进展都要保留" in store and "append_progress_keeps_previous_entries" in store,
        "进展只追加、不覆盖历史",
    )
    check(
        "link_report_shape",
        "pub struct LinkReport" in store
        and "pub ok: bool" in store
        and "pub partial: bool" in store
        and "pub warnings: Vec<String>" in store
        and "pub record: Option<crate::worklog_records::WorkRecord>" in store,
        "采纳关联回执含成功标志、半完成标志与各项状态",
    )
    check(
        "partial_writeback_receipts_tested",
        "link_reports_partial_when_record_writeback_fails" in store
        and "link_reports_partial_when_candidate_archive_fails" in store
        and "不要重复采纳" in store,
        "记录回链或候选归档失败时报告半完成状态",
    )
    check(
        "ui_shows_partial_result",
        "if (!report.ok)" in panel and "report.warnings" in panel,
        "审核台把半完成原因与补救提示原样显示",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-307",
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
