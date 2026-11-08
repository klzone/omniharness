#!/usr/bin/env python3
"""OD-WR-306 AI 任务审核工作台：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：审核视图挂载、候选列表、置信度与不确定原因、原文证据高亮、字段就地编辑、
采纳/拒绝、来源跳转、批量采纳走同一 IPC 校验、Aetheria 两栏布局。

用法：python3 tests/verification/desktop/verify_task_review_ui_303.py [--json PATH]
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


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    page = read(SRC / "shell" / "WorkRecordPage.tsx")
    panel = read(SRC / "shell" / "TaskReviewPanel.tsx")
    library = read(SRC / "shell" / "WorkRecordLibrary.tsx")

    check(
        "review_tab_mounted",
        'key: "review"' in page and "<TaskReviewPanel" in page,
        "工作记录页新增审核视图并挂载审核台",
    )
    check(
        "candidate_list_from_host",
        ".taskCandidateList()" in panel and "data-candidate-list" in panel,
        "待审核列表直接消费宿主候选",
    )
    check(
        "confidence_visible",
        "confidenceText(" in panel and "data-candidate-confidence" in panel,
        "列表与详情显示置信度",
    )
    check(
        "uncertainty_visible",
        "selected.uncertainty" in panel and "不确定原因" in panel,
        "详情展开不确定原因",
    )
    check(
        "evidence_highlight",
        "EvidenceBody" in panel and "<mark" in panel and "source_quote" in panel,
        "原文证据按连续片段高亮",
    )
    check(
        "inline_edit_fields",
        all(text in panel for text in ("任务标题", "分类", "截止日期", "备注")),
        "字段就地编辑",
    )
    check(
        "confirm_single_uses_host",
        ".taskCandidateConfirm(selected.id" in panel and "title: title.trim()" in panel,
        "采纳时带可改字段，交给宿主同一校验",
    )
    check(
        "reject_single_uses_host",
        ".taskCandidateReject(selected.id" in panel,
        "拒绝经宿主落定",
    )
    check(
        "source_entry",
        "data-candidate-source" in panel and "onOpenRecord(selected.record_id)" in panel,
        "来源采纳入口可跳回记录",
    )
    check(
        "batch_confirm_same_validation",
        "confirmBatch" in panel
        and ".taskCandidateConfirm(id, {})" in panel
        and "data-candidate-batch" in panel,
        "批量采纳逐条走同一个确认 IPC",
    )
    check(
        "review_actions_visible",
        "data-candidate-actions" in panel and "采纳" in panel and "拒绝" in panel,
        "采纳与拒绝动作可见",
    )
    check(
        "two_column_aetheria_layout",
        "lg:grid-cols" in panel and "lg:sticky" in panel and "bg-white/80" in panel,
        "参照知识审核中心的两栏 Aetheria 布局",
    )
    check(
        "load_and_action_errors_visible",
        "读取候选失败" in panel and 'role="status"' in panel,
        "读盘与动作失败原样提示",
    )
    check(
        "empty_state",
        "没有待审核候选" in panel,
        "空队列如实显示",
    )
    check(
        "parse_button_feeds_review",
        ".workRecordAnalyzePrepare(" in library
        and "onCandidatesReady" in library
        and "onCandidatesReady" in page,
        "解析完成后切到审核视图",
    )
    check(
        "no_english_subtitles",
        not any(word in panel for word in ("REVIEW", "CANDIDATES", "AI TASKS")),
        "不再叠英文副标题",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-306",
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
