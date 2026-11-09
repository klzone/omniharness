#!/usr/bin/env python3
"""OD-WT-614 归档与知识回流：静态源码检查（可选 cargo 定向回归）。

证据等级：静态源码检查；加 --cargo-tests 时实跑 614 新增 Rust 用例。
不开窗口、不调用模型、不读写用户知识库。

覆盖：vault_archive 文稿入口（干净 MD、报告目录、仅 .md）、writing_archive
状态门禁/敏感拒绝/manifest archive 回执/已归档终态、归档关联字段（场景十）、
knowledge_feedback 只收勾选内容且只入 raw 治理队列（场景十一）、
前端归档区块/确认弹窗/软引导/勾选提交、runtime/ 零改动、无新增豁免。

用法：python3 tests/verification/desktop/verify_writing_archive_614.py [--json PATH] [--cargo-tests]
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
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def git_status(path: str) -> str:
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--", path],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        return result.stdout.strip()
    except OSError:
        return "git-unavailable"


def count_tests(source: str, pattern: str) -> int:
    return len(re.findall(pattern, source))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    ap.add_argument("--cargo-tests", action="store_true")
    args = ap.parse_args()

    archive_path = RUST / "vault_archive.rs"
    lib_path = RUST / "lib.rs"
    archive = archive_path.read_text(encoding="utf-8")
    lib = lib_path.read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    panel_path = SRC / "shell" / "WritingArchivePanel.tsx"
    workbench_path = SRC / "shell" / "WritingWorkbench.tsx"
    panel = panel_path.read_text(encoding="utf-8") if panel_path.exists() else ""
    workbench = workbench_path.read_text(encoding="utf-8")

    production = archive.split("#[cfg(test)]")[0]
    writing_entry = production.split("pub fn archive_writing_final", 1)[1]
    archive_tests = archive.split("#[cfg(test)]")[1] if "#[cfg(test)]" in archive else ""
    lib_tests = lib.split("mod writing_archive_ipc_tests", 1)[1] if "mod writing_archive_ipc_tests" in lib else ""

    check(
        "vault_writing_entry",
        "pub fn archive_writing_final(" in production
        and "pub struct WritingArchiveMeta" in production
        and "WRITING_ARCHIVE_DIR" not in production
        and "REPORT_ARCHIVE_DIR" in production
        and 'format!("文稿-{cleaned}-{}.md"' in production,
        "vault_archive 新增文稿入口，目标 <vault_root>/报告/文稿-<title>-<date>.md",
    )
    check(
        "clean_markdown_gate",
        "crate::export_engine::strip_internal(&normalized)" in production
        and "strip_internal(&normalized);" in production
        and "stripped != normalized" in production
        and "干净 Markdown" in production,
        "复用 613 正式稿剥离函数校验干净 MD；内部编号/AI 标记拒绝",
    )
    check(
        "writing_frontmatter_associations",
        all(
            token in production
            for token in [
                '"writing_id"',
                '"writing_title"',
                '"writing_version"',
                '"selected_material_ids"',
                '"selected_knowledge_ids"',
                '"factcheck_counts"',
                '"export_receipt_count"',
                'WRITING_ARCHIVE_SCHEMA',
            ]
        ),
        "归档 Markdown frontmatter 记录任务/素材/知识/核验/版本/导出关联",
    )
    check(
        "archive_only_markdown",
        "assert_archive_name(&file_name)" in production
        and "ends_with(\".md\")" in production
        and ".docx" not in writing_entry
        and ".pdf" not in writing_entry
        and ".html" not in writing_entry,
        "文稿归档入口只接受 .md；DOCX/PDF/HTML 无写入路径",
    )

    check(
        "ipc_commands_registered",
        "fn writing_archive(" in lib
        and "fn writing_knowledge_feedback(" in lib
        and "writing_archive," in lib
        and "writing_knowledge_feedback," in lib,
        "两条 IPC 已定义并注册到 generate_handler",
    )
    check(
        "archive_status_gate",
        "task.status != writing_store::STATUS_FINALIZED" in lib
        and "只有「{}」的写作任务可以归档" in lib
        and "is_allowed_transition(" in lib
        and "STATUS_ARCHIVED" in lib,
        "硬门禁只校验已定稿，并复核 601 已定稿→已归档迁移合法性",
    )
    check(
        "archive_sensitive_reject",
        "归档前敏感词扫描命中 {total} 处" in lib
        and "已拒绝归档（不回显命中原文）" in lib
        and "task_shell::red_line_scan_with" in lib,
        "归档前与导出同口径敏感词扫描；命中拒绝且不回显原文",
    )
    check(
        "manifest_archive_receipt",
        'document_store::record_export_receipt(' in lib
        and '\"archive\",' in lib
        and "manifest 归档回执写入失败" in lib
        and "STATUS_ARCHIVED" in lib,
        "归档后按 602 追加 manifest export_receipts（format=archive），再推进已归档",
    )
    check(
        "scene_ten_receipt_fields",
        all(
            token in lib
            for token in [
                "pub struct WritingArchiveAssociation",
                "pub writing_id:",
                "pub title:",
                "pub selected_material_ids:",
                "pub selected_knowledge_ids:",
                "pub factcheck_counts:",
                "pub version:",
                "pub export_receipts:",
                "pub archive_path:",
            ]
        )
        and "writing_records::CHECK_CONCLUSIONS" in lib
        and "pub struct WritingArchiveOutcome" in lib
        and "pub vault:" in lib
        and "pub manifest_receipt:" in lib,
        "场景十：回执含任务/标题/素材/知识/核验八类统计/版本/导出清单/vault 路径",
    )
    check(
        "scene_ten_terminal",
        "writing_store::transition_writing_status(" in lib
        and "Some(\"用户归档\")" in lib
        and "写作任务已归档：归档为终态" in lib,
        "归档成功后推进已归档；重复归档拒绝且不追加回执",
    )
    check(
        "soft_export_guidance",
        "尚未导出。建议先导出；归档仍可继续。" in panel
        and "data-archive-export-list" in panel
        and "data-archive-export-empty" in panel
        and "can_archive" in panel,
        "归档页显示导出清单；未导出时软引导，不阻止归档",
    )
    check(
        "archive_confirm_dialog",
        "<ConfirmDialog" in panel
        and "writing-archive-confirm" in panel
        and "shell.writingArchive({ writing_id: writingId, preview: false })" in panel
        and "data-archive-button" in panel,
        "归档按钮只在确认弹窗后调用执行入口",
    )
    check(
        "frontend_mounted",
        "export function WritingArchivePanel" in panel
        and "import { WritingArchivePanel }" in workbench
        and 'key: "archive", label: "归档"' in workbench
        and "<WritingArchivePanel" in workbench,
        "归档区块已挂载写作任务详情页签",
    )

    check(
        "knowledge_feedback_gate",
        "gate_user_text(&selection.title" in lib
        and "gate_user_text(&selection.content" in lib
        and "gate_user_text(&selection.source_quote" in lib
        and "没有勾选的可复用内容；未提交任何治理通道" in lib,
        "知识回流只接收 request.selections，标题/内容/来源原文均过 gate_user_text",
    )
    feedback_impl = lib.split("fn writing_knowledge_feedback_impl", 1)[1].split("fn writing_archive(", 1)[0]
    check(
        "scene_eleven_raw_only",
        "knowledge_ingest::init_vault(vault)" in lib
        and "knowledge_ingest::admit_raw_files_with_policy(" in feedback_impl
        and "knowledge_review::decide(" not in feedback_impl
        and "knowledge_ingest::apply_payload" not in feedback_impl
        and "已入 raw 收件待审队列" in lib
        and "不自动生成知识条目" in lib,
        "场景十一：只入既有 raw 收件治理队列；不自动建卡/裁决/定稿",
    )
    check(
        "knowledge_feedback_frontend",
        "data-knowledge-feedback" in panel
        and "data-knowledge-feedback-list" in panel
        and "data-knowledge-feedback-check" in panel
        and "shell.writingKnowledgeFeedback" in panel
        and "selected.length === 0" in panel,
        "知识回流只提交勾选项；未勾选时禁用提交",
    )
    check(
        "api_types_wrappers",
        "WritingArchiveInput" in api
        and "WritingArchiveOutcome" in api
        and "WritingKnowledgeFeedbackInput" in api
        and "WritingKnowledgeFeedbackOutcome" in api
        and "writing_archive" in api
        and "writing_knowledge_feedback" in api
        and "export interface WritingArchiveAssociation" in types
        and "factcheck_counts: WritingFactCheckCount[]" in types
        and "export interface WritingKnowledgeFeedbackOutcome" in types,
        "types/api 封装归档回执与知识回流结果",
    )

    new_tests = count_tests(lib_tests, r"fn (archive_|knowledge_feedback_)") + count_tests(
        archive_tests, r"fn writing_archive_"
    )
    check(
        "new_rust_tests",
        new_tests >= 8,
        f"614 新增 Rust 用例 {new_tests} 项（要求 ≥8）",
    )
    check(
        "scene_ten_eleven_test_assertions",
        all(
            token in lib_tests
            for token in [
                "factcheck_counts.len(), 8",
                "selected_material_ids",
                "selected_knowledge_ids",
                "archive_second_attempt_is_rejected_without_duplicate_receipt",
                "未勾选内容不得进入 raw 收件",
                "回流不自动归档或定稿",
            ]
        ),
        "测试覆盖场景十（回执字段/终态）与场景十一（未勾选不进治理）",
    )
    check(
        "runtime_untouched",
        git_status("runtime") == "",
        "runtime/ 本轮零改动",
    )
    engine_refs = []
    engine_dir = REPO / "engine"
    if engine_dir.is_dir():
        for source in engine_dir.rglob("*.py"):
            try:
                if "writing_archive" in source.read_text(encoding="utf-8", errors="ignore"):
                    engine_refs.append(str(source.relative_to(REPO)))
            except OSError:
                continue
    check(
        "ai_no_archive_channel",
        not engine_refs,
        "引擎侧无 writing_archive 调用通道" if not engine_refs else f"引擎引用：{engine_refs[:3]}",
    )
    check(
        "no_new_exemptions",
        "#[ignore]" not in archive_tests
        and "#[ignore]" not in lib_tests
        and "skip" not in lib_tests.lower(),
        "新增测试无 ignore/skip 豁免",
    )

    cargo_ok = True
    cargo_note = "未运行（静态检查）"
    if args.cargo_tests:
        try:
            env = os.environ.copy()
            cargo_bin = str(Path.home() / ".cargo" / "bin")
            env["PATH"] = cargo_bin + os.pathsep + env.get("PATH", "")
            result = subprocess.run(
                ["cargo", "test", "--offline", "--lib", "writing_archive"],
                cwd=REPO / "desktop" / "src-tauri",
                env=env,
                capture_output=True,
                text=True,
                check=False,
                timeout=180,
            )
            cargo_ok = result.returncode == 0
            summary = [line for line in result.stdout.splitlines() if "test result" in line]
            cargo_note = "；".join(summary) or f"exit {result.returncode}"
        except (OSError, subprocess.SubprocessError) as exc:
            cargo_ok = False
            cargo_note = f"cargo 运行失败：{exc}"
    check("cargo_targeted_tests", cargo_ok, cargo_note)

    passed = sum(1 for _, ok, _ in CHECKS if ok)
    total = len(CHECKS)
    for name, ok, detail in CHECKS:
        print(f"[{'PASS' if ok else 'FAIL'}] {name}: {detail}")
    print(f"\n{passed}/{total} checks passed")
    if args.json:
        args.json.write_text(
            json.dumps(
                {
                    "task": "OD-WT-614",
                    "passed": passed,
                    "total": total,
                    "ok": passed == total,
                    "checks": [
                        {"name": name, "ok": ok, "detail": detail}
                        for name, ok, detail in CHECKS
                    ],
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
