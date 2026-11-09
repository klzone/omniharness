#!/usr/bin/env python3
"""OD-WT-613 导出引擎与样式模板：静态源码检查 + 可选真实导出回归。

证据等级：静态源码检查；加 --cargo-tests 时会实跑 export_engine 的
真实导出临时目录用例（Rust 测试自行清理临时目录，不读用户数据）。

覆盖：MD 三态剥离与来源标记、导出前敏感词扫描拒绝、DOCX office-docx /
textutil 双路径回执、PDF soffice 超时与打印流兜底、公众号 HTML 渲染与
模板校验、三套样式加载、状态迁移门禁与定稿快照、citations/factcheck
记录查询、前端封装、runtime/ 零改动、无新增豁免。

用法：python3 tests/verification/desktop/verify_writing_export_613.py [--json PATH] [--cargo-tests]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"
STYLES = REPO / "samples" / "workflows" / "export-styles"
HELPER = REPO / "desktop" / "src-tauri" / "scripts" / "writing_docx_export.py"

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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    ap.add_argument("--cargo-tests", action="store_true")
    args = ap.parse_args()

    engine_path = RUST / "export_engine.rs"
    source = engine_path.read_text(encoding="utf-8")
    production = source.split("#[cfg(test)]")[0]
    tests = source.split("#[cfg(test)]")[1]
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    helper = HELPER.read_text(encoding="utf-8")
    gongwen = (STYLES / "gongwen.md").read_text(encoding="utf-8")
    reading = (STYLES / "reading.md").read_text(encoding="utf-8")
    html = (STYLES / "gongzhonghao.html").read_text(encoding="utf-8")

    check(
        "module_registered",
        "pub mod export_engine;" in lib
        and "pub fn export_writing(" in production
        and "pub struct ExportRequest" in production,
        "lib.rs 已挂 export_engine，导出入口与入参存在",
    )
    check(
        "ipc_commands_registered",
        all(
            item in lib
            for item in [
                "fn writing_status_transition(",
                "fn writing_records_query(",
                "fn writing_export(",
                "writing_status_transition,",
                "writing_records_query,",
                "writing_export,",
            ]
        ),
        "三条 IPC 已定义并注册到 generate_handler",
    )
    check(
        "outcome_fields",
        all(
            f"pub {field}:" in source.split("pub struct ExportOutcome", 1)[1].split("}", 1)[0]
            for field in ["writing_id", "format", "mode", "path", "engine", "simplified", "receipt", "notes"]
        ),
        "回执含路径、引擎、简化标记、manifest 回执与说明",
    )
    check(
        "format_and_mode_whitelists",
        all(
            item in production
            for item in [
                'FORMAT_MD: &str = "md"',
                'FORMAT_DOCX: &str = "docx"',
                'FORMAT_PDF: &str = "pdf"',
                'FORMAT_HTML: &str = "html"',
                'MODE_FORMAL: &str = "formal"',
                'MODE_REVIEW: &str = "review"',
                'MODE_WORKING: &str = "working"',
            ]
        )
        and "fn normalize_format(" in production
        and "fn normalize_mode(" in production,
        "四种格式与三种 Markdown 模式的封闭白名单",
    )
    check(
        "markdown_three_states",
        "pub fn strip_internal(" in production
        and "pub fn build_review_markdown(" in production
        and "pub fn build_working_markdown(" in production
        and "(FORMAT_MD, MODE_FORMAL) => strip_internal" in production
        and "(FORMAT_MD, MODE_REVIEW) => build_review_markdown" in production
        and "(FORMAT_MD, MODE_WORKING)" in production,
        "正式稿剥离 / 审阅稿来源标记 / 工作稿核验摘要三条分支",
    )
    check(
        "formal_strip_markers",
        all(
            token in production
            for token in [
                "frontmatter",
                "<!--",
                "【",
                "〔",
                "内部编号",
                "AI 调试",
                "AI调试",
                "Debug:",
                "模型:",
                '"wt-"',
                '"m-"',
                '"k-"',
                '"citation-"',
                '"version-"',
                "collapse_blank_lines",
            ]
        ),
        "正式稿剥离 frontmatter、内部编号、来源标记与全部内部前缀",
    )
    check(
        "review_markers_and_details",
        "〔来源：" in production
        and "## 来源标记" in production
        and "段落标记未匹配，仅列在来源明细" in production
        and "citation_marker(" in production,
        "审阅稿逐段加来源标记，文末来源明细，未匹配显式说明",
    )
    check(
        "working_summary",
        "## 核验结果摘要" in production
        and "本次导出前没有核验记录。" in production
        and "待处理" in production
        and ".take(200)" in production,
        "工作稿带核验统计、逐条摘要与 200 条展开上限",
    )
    check(
        "export_status_gate",
        "STATUS_FINALIZED | writing_store::STATUS_ARCHIVED" in production
        and "只有「" in production
        and "的文稿可以导出" in production,
        "只有已定稿或已归档的任务可导出",
    )
    scan_index = production.find("red_line_scan_with")
    render_index = production.find("let rendered = match format")
    check(
        "redline_scan_before_render",
        scan_index > 0
        and render_index > scan_index
        and "敏感词扫描命中" in production
        and "不回显命中原文" in production,
        "渲染前先过当前 Profile 敏感词扫描；命中即拒绝且不回显原文",
    )
    check(
        "manifest_receipt",
        "record_export_receipt(" in production
        and "md-formal" in production
        and "md-review" in production
        and "md-working" in production
        and "manifest 回执写入失败" in production,
        "导出后写 manifest export_receipts，三态 MD 分别记 md-formal/review/working",
    )
    check(
        "target_dir_guard",
        "fn resolve_target_dir(" in production
        and "必须是绝对路径" in production
        and "不允许包含 .. 逃逸片段" in production
        and 'app_root.join(EXPORT_DIR).join("写作")' in production,
        "目标目录只接受绝对路径、拒绝 ..，缺省落应用数据根 exports/写作/<id>/",
    )
    check(
        "docx_dual_path",
        all(
            token in production
            for token in [
                '"office-docx"',
                '"textutil"',
                "simplified: true",
                "textutil 简化版：公文页边距与字体层级不一定与 Word 版完全一致。",
                "office-docx 脚本未产出可用 DOCX，已转 textutil 简化版。",
                "python_has_docx(",
            ]
        )
        and "DOCX_HELPER" in production,
        "DOCX 优先 python-docx，失败如实标注 textutil 简化版",
    )
    check(
        "pdf_soffice_timeout_and_fallback",
        all(
            token in production
            for token in [
                "SOFFICE_TIMEOUT",
                "soffice_timeout",
                "--convert-to",
                '"pdf"',
                "cupsfilter",
                "print_flow",
                "%PDF-",
                "未完成（已终止）",
                "soffice 失败或超时，转打印流兜底",
            ]
        ),
        "PDF 走 soffice headless，超时/失败回执并转打印流，校验 PDF 签名",
    )
    check(
        "wechat_html_render",
        all(
            token in production
            for token in [
                "pub fn render_wechat_html(",
                "render_inline_html(",
                "escape_html(",
                "{{content}}",
                "{{title}}",
                "<h1",
                "<blockquote",
                "• ",
            ]
        )
        and "javascript:" in production
        and "不得引用远程资源" in production,
        "公众号 HTML 由宿主确定性渲染并转义；模板拒绝脚本、iframe 与远程资源",
    )
    check(
        "style_loading",
        all(
            token in production
            for token in [
                'STYLE_DIR: &str = "writing-export-styles"',
                'include_str!("../../../samples/workflows/export-styles/gongwen.md")',
                'include_str!("../../../samples/workflows/export-styles/reading.md")',
                'include_str!("../../../samples/workflows/export-styles/gongzhonghao.html")',
                "MAX_STYLE_BYTES",
                "MAX_TEMPLATE_BYTES",
                "pub fn load_doc_style(",
                "pub fn load_html_template(",
                "body_font",
                "heading_font",
                "body_size_pt",
                "line_height",
                "margin_cm",
                "max_width_cm",
                "heading_align",
            ]
        ),
        "样式包可编辑副本优先、编译期默认兜底，字段与大小上限齐全",
    )
    check(
        "style_files_complete",
        "body_font:" in gongwen
        and "heading_align:" in gongwen
        and "max_width_cm:" in reading
        and "{{content}}" in html
        and "{{title}}" in html,
        "三套样式文件存在且含必需键与 HTML 占位符",
    )
    check(
        "docx_helper_contract",
        "from docx import Document" in helper
        and "qn(\"w:eastAsia\")" in helper
        and "paragraph_format.line_spacing" in helper
        and "reopened = Document(sys.argv[2])" in helper
        and "len(reopened.paragraphs) < len(blocks)" in helper
        and "import json" in helper,
        "DOCX 脚本设置 eastAsia 字体、行距与重开校验，不调模型/网络",
    )
    check(
        "status_transition_gate_and_snapshot",
        "is_allowed_transition(&task.status, &to, outline_confirmed)" in lib
        and "export_engine::finalize_snapshot(root, &writing_id, now)" in lib
        and "STATUS_PENDING_REVIEW" in lib
        and "STATUS_FINALIZED" in lib
        and "gate_user_text(&value, policy, \"状态迁移理由\")" in lib,
        "状态迁移按 601 合法表，待审核→已定稿先写 final 快照，理由过闸门",
    )
    check(
        "records_query_filters",
        all(
            token in production
            for token in [
                "pub struct RecordsQueryRequest",
                "pub fn query_records(",
                'matches!(kind, None | Some("citations"))',
                'matches!(kind, None | Some("factcheck"))',
                "section_no",
                "paragraph_no",
                "CheckConclusion::from_label(conclusion)",
                "ResolutionStatus::from_label(resolution)",
            ]
        ),
        "citations/factcheck 只读查询支持类型、章节、段落、结论与处理状态过滤",
    )
    check(
        "api_wrappers",
        all(
            item in api
            for item in [
                "writingStatusTransition(",
                'invoke<WritingTask>("writing_status_transition", { request: input })',
                "writingRecordsQuery(input: WritingRecordsQueryInput): Promise<WritingRecordsQueryResult>",
                'invoke<WritingRecordsQueryResult>("writing_records_query", { request: input })',
                "writingExport(input: WritingExportInput): Promise<WritingExportOutcome>",
                'invoke<WritingExportOutcome>("writing_export", { request: input })',
            ]
        )
        and "WritingExportInput," in api
        and "WritingRecordsQueryInput," in api,
        "前端三条封装与类型 import 齐全，不绕过 api 客户端",
    )
    check(
        "frontend_types",
        all(
            item in types
            for item in [
                'export type WritingExportFormat = "md" | "docx" | "pdf" | "html";',
                'export type WritingExportMode = "formal" | "review" | "working";',
                "export interface WritingExportInput",
                "export interface WritingExportOutcome",
                "export interface WritingStatusTransitionInput",
                "export interface WritingCitationRecord",
                "export interface WritingRecordsQueryInput",
                "export interface WritingRecordsQueryResult",
            ]
        ),
        "前端类型与 613 IPC 回执同口径",
    )
    check(
        "required_tests_present",
        all(
            name in tests
            for name in [
                "formal_markdown_strips_frontmatter_and_internal_markers",
                "export_rejects_redline_hits_without_echo",
                "docx_textutil_fallback_marks_simplified",
                "docx_office_path_receipt_is_not_simplified",
                "pdf_soffice_timeout_reports_without_fake_success",
                "export_html_writes_template_output",
                "styles_load_defaults_from_embedded_samples",
                "finalize_snapshot_writes_final_version",
                "query_records_filters_citations_and_fact_checks",
            ]
        )
        and "export_formal_md_writes_file_and_receipt" in tests
        and "export_review_md_includes_citations" in tests
        and "export_working_md_includes_factchecks" in tests,
        "八项任务卡验收点均有模块内测试，含真实导出临时目录用例",
    )
    test_count = tests.count("#[test]")
    check(
        "test_count_at_least_20",
        test_count >= 20,
        f"export_engine 模块内测试 {test_count} 项（任务卡要求 ≥14，交付 ≥20）",
    )
    check(
        "no_network_or_vault_write",
        all(
            token not in production
            for token in ["reqwest", "ureq", "TcpStream", "knowledge_vault", "vault_write", "std::net"]
        ),
        "导出引擎不发起网络请求，不写知识库",
    )
    check(
        "runtime_untouched",
        git_status("runtime") == "",
        "runtime/ 零改动",
    )
    check(
        "no_new_exemptions",
        all(
            marker not in source + helper + api + types
            for marker in ["@ts-ignore", "@ts-expect-error", "eslint-disable", "ts-nocheck"]
        ),
        "导出引擎、脚本与前端封装未新增类型/lint 豁免",
    )
    check(
        "synthetic_only",
        all(
            token not in source + helper
            for token in ["13800138000", "110101", "张三", "李四", "真实姓名"]
        ),
        "测试数据全部虚构，不含真实个人号码或姓名夹具",
    )
    check(
        "evidence_dir",
        (HERE / "od-wt-613").is_dir(),
        "证据目录 tests/verification/desktop/od-wt-613/ 存在",
    )

    if args.cargo_tests:
        result = subprocess.run(
            [
                "bash",
                "-lc",
                'export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && '
                "RUST_TEST_THREADS=4 cargo test --offline --lib export_engine",
            ],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        ok = result.returncode == 0
        tail = "\n".join(result.stdout.strip().splitlines()[-3:])
        check("cargo_export_engine_tests", ok, tail)

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-613",
        "evidence_level": "静态源码检查" + (" + cargo 运行测试" if args.cargo_tests else ""),
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": name, "ok": ok, "detail": detail} for name, ok, detail in CHECKS],
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
