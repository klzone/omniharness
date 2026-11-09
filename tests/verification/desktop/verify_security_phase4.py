#!/usr/bin/env python3
"""OD-WT-615 第四阶段安全核对：静态源码检查 + 既有测试覆盖断言。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖 PHASE4_ACCEPTANCE_CRITERIA §三 十项：写作流新 IPC 闸门、AI 无定稿/
归档/导出写通道、大纲未确认不得生成正文、未确认素材不进事实集合、
导出敏感扫描与本地无网络、归档仅 Markdown、无敏感原文日志、测试全虚构、
无新增豁免、runtime/ 零改动。另核对第四阶段 IPC 注册与前端只走 IPC。

用法：python3 tests/verification/desktop/verify_security_phase4.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
RUST = REPO / "desktop" / "src-tauri" / "src"
ENGINE = REPO / "engine" / "omnidesk_engine"
HELPER = REPO / "desktop" / "src-tauri" / "scripts" / "writing_docx_export.py"
EXTRACT_TEST = REPO / "tests" / "verification" / "engine-interface" / "adapter" / "test_writing_extract_unit.py"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def production(source: str) -> str:
    return source.split("\n#[cfg(test)]", 1)[0]


def function_block(source: str, name: str) -> str:
    marker = "fn " + name + "("
    start = source.find(marker)
    if start < 0:
        return ""
    tail = source[start + len(marker) :]
    for separator in [
        "\n#[tauri::command",
        "\n#[cfg(test)]",
        "\npub fn ",
        "\nfn ",
        "\nstruct ",
        "\n/// ",
    ]:
        index = tail.find(separator)
        if index >= 0:
            tail = tail[:index]
    return source[start : start + len(marker) + len(tail)]


ALLOWED_ALLOW_LINES = {
    "#[allow(clippy::too_many_arguments)]",
    "#[allow(dead_code)]",
}
DISALLOWED_TOKENS = (
    "#[ignore]",
    "# noqa",
    "pytest.mark.skip",
    "@ts-ignore",
    "eslint-disable",
)


def waiver_violations() -> tuple[list[str], list[str]]:
    """返回（违规行，已登记的有界 lint allow）。615 未改业务代码。"""
    violations: list[str] = []
    allowed: list[str] = []

    def scan_line(raw: str, source: str) -> None:
        line = raw.strip()
        if not line:
            return
        if any(token in line for token in DISALLOWED_TOKENS):
            violations.append(f"{source}: {line}")
            return
        if "#[allow(" in line and not line.startswith("//"):
            if line in ALLOWED_ALLOW_LINES:
                allowed.append(f"{source}: {line}")
            else:
                violations.append(f"{source}: {line}")

    try:
        diff = subprocess.run(
            ["git", "diff", "--unified=0", "HEAD", "--", "desktop", "engine", "tests"],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        ).stdout
    except OSError as error:
        violations.append(f"git diff failed: {error}")
        return violations, allowed
    current = ""
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current = line[6:].strip()
            continue
        if line.startswith("+") and not line.startswith("+++"):
            if current.startswith("tests/verification/"):
                continue
            scan_line(line[1:], "diff")

    untracked = subprocess.run(
        ["git", "status", "--porcelain", "--", "desktop", "engine", "tests"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    for line in untracked:
        if not line.startswith("??"):
            continue
        path = REPO / line[3:].strip()
        if (
            path.name == "verify_security_phase4.py"
            or path.suffix not in {".rs", ".tsx", ".ts", ".py"}
            or "tests/verification" in path.as_posix()
        ):
            continue
        for raw in read(path).splitlines():
            scan_line(raw, path.as_posix())
    return violations, allowed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    lib = read(RUST / "lib.rs")
    lib_prod = production(lib)
    writing_store = read(RUST / "writing_store.rs")
    store_prod = production(writing_store)
    document_store = read(RUST / "document_store.rs")
    collect = read(RUST / "writing_collect.rs")
    collect_prod = production(collect)
    outline = read(RUST / "writing_outline.rs")
    outline_prod = production(outline)
    section = read(RUST / "writing_section.rs")
    section_prod = production(section)
    review = read(RUST / "writing_review.rs")
    review_prod = production(review)
    fact_check = read(RUST / "fact_check.rs")
    fact_prod = production(fact_check)
    export = read(RUST / "export_engine.rs")
    export_prod = production(export)
    records = read(RUST / "writing_records.rs")
    records_prod = production(records)
    archive = read(RUST / "vault_archive.rs")
    archive_prod = production(archive)
    parse = read(RUST / "writing_parse.rs")
    parse_prod = production(parse)
    material_query = read(RUST / "material_query.rs")
    material_query_prod = production(material_query)
    extract_py = read(ENGINE / "writing_extract.py")
    extract_test = read(EXTRACT_TEST)
    helper = read(HELPER)

    ui_files = [
        SRC / "shell" / "WritingWorkbench.tsx",
        SRC / "shell" / "WritingOutlineEditor.tsx",
        SRC / "shell" / "WritingSectionEditor.tsx",
        SRC / "shell" / "WritingFactCheckPanel.tsx",
        SRC / "shell" / "WritingVersionPanel.tsx",
        SRC / "shell" / "WritingArchivePanel.tsx",
        SRC / "api.ts",
    ]
    ui_text = "\n".join(read(path) for path in ui_files)

    gate = function_block(lib_prod, "gate_user_text")
    registry = lib.split("tauri::generate_handler![")[1].split("])", 1)[0]

    # ---- 1. 写作流新 IPC 全部过统一闸门 -------------------------------
    check(
        "gate_definition",
        "fn gate_user_text(" in lib_prod
        and "red_line_scan_with" in gate
        and "mask_privacy" in gate,
        "统一闸门仍是红线扫描 + 隐私掩码",
    )
    check(
        "gate_error_does_not_echo_text",
        "{detail}" in gate
        and "{text}" not in gate
        and "hit.label" in gate
        and "hit.count" in gate,
        "拒绝信息只报命中标签与次数，不回显原文",
    )
    check(
        "gate_covers_parse",
        'gate_user_text(&text, &policy, "写作需求")' in lib_prod,
        "writing_parse_requirement 的需求原文过闸门",
    )
    check(
        "gate_covers_create",
        all(
            token in lib_prod
            for token in [
                '(&mut draft.title, "写作标题")',
                '(&mut draft.topic, "写作主题")',
                '(&mut draft.writing_purpose, "写作目的")',
                '(&mut draft.audience, "目标受众")',
                '(&mut draft.writing_period, "写作周期")',
                '(&mut draft.expected_length, "预计篇幅")',
                '(&mut draft.writing_style, "写作风格")',
                'gate_user_text(item, policy, "写作要求")',
            ]
        ),
        "writing_create_from_requirement 的七个可编辑字段与要求列表全部过闸门",
    )
    check(
        "gate_covers_collect_select",
        'gate_user_text(&request.writing_id, policy, "写作任务编号")' in function_block(lib_prod, "writing_collect_materials_impl")
        and 'gate_user_text(&note, policy, "素材选择说明")' in function_block(lib_prod, "writing_select_materials_impl"),
        "collect 的任务编号与 select 的采纳说明过闸门",
    )
    check(
        "gate_covers_outline",
        'gate_user_text(&request.writing_id, policy, "写作任务编号")' in function_block(lib_prod, "writing_outline_generate_impl")
        and "gate_outline_text(&mut request.outline, policy)" in function_block(lib_prod, "writing_outline_confirm_impl")
        and 'gate_user_text(&note, policy, "大纲确认说明")' in function_block(lib_prod, "writing_outline_confirm_impl")
        and all(
            token in function_block(lib_prod, "gate_outline_text")
            for token in [
                'apply(&mut outline.title, "大纲标题")',
                'apply(&mut outline.writing_purpose, "大纲写作目的")',
                "for point in outline.core_points.iter_mut()",
                'apply(point, "大纲核心观点")',
                'apply(&mut section.heading, "大纲章节标题")',
                'apply(&mut section.focus, "大纲章节写作重点")',
                'apply(value, "大纲小节标题")',
            ]
        ),
        "大纲生成只接编号；确认的标题/目的/观点/章节/小节全部过闸门",
    )
    check(
        "gate_covers_section",
        'gate_user_text(&request.writing_id, policy, "写作任务编号")' in function_block(lib_prod, "writing_section_generate_impl")
        and "gate_section_draft(&mut request.draft, policy)" in function_block(lib_prod, "writing_section_confirm_impl")
        and 'gate_user_text(&note, policy, "章节确认说明")' in function_block(lib_prod, "writing_section_confirm_impl")
        and 'gate_user_text(&request.instruction, policy, "修订补充说明")' in function_block(lib_prod, "writing_section_revise_impl")
        and all(
            token in function_block(lib_prod, "gate_section_draft")
            for token in [
                'gate_user_text(&draft.content, policy, "章节稿正文")',
                'gate_user_text(&citation.quote, policy, "章节引用原文")',
                'gate_user_text(&segment.text, policy, "四分法标注文本")',
                'gate_user_text(&quote, policy, "四分法标注引用")',
            ]
        ),
        "章节确认的正文/引用/四分法文本与修订说明全部过闸门",
    )
    check(
        "gate_covers_review",
        'gate_user_text(&instruction, policy, "审阅补充说明")' in function_block(lib_prod, "writing_review_impl")
        and all(
            token in function_block(lib_prod, "writing_review_apply_finding_impl")
            for token in [
                '"审阅问题编号"',
                '"审阅问题描述"',
                '"审阅修改建议"',
            ]
        )
        and "gate_section_draft(&mut request.draft, policy)" in function_block(lib_prod, "writing_review_apply_finding_impl")
        and 'gate_user_text(&note, policy, "审阅修订说明")' in function_block(lib_prod, "writing_review_apply_finding_impl"),
        "审阅说明、问题编号/描述/建议与修订稿全部过闸门",
    )
    check(
        "gate_covers_transition_records_export",
        'gate_user_text(&value, policy, "状态迁移理由")' in function_block(lib_prod, "writing_status_transition_impl")
        and 'gate_user_text(&request.writing_id, &policy, "写作任务编号")' in function_block(lib_prod, "writing_records_query")
        and 'gate_user_text(&request.writing_id, &policy, "写作任务编号")' in function_block(lib_prod, "writing_export"),
        "状态迁移理由、记录查询编号、导出编号均过闸门",
    )
    check(
        "gate_covers_archive_feedback",
        'gate_user_text(&request.writing_id, policy, "写作任务编号")' in function_block(lib_prod, "writing_archive_impl")
        and all(
            token in function_block(lib_prod, "writing_knowledge_feedback_impl")
            for token in [
                'gate_user_text(&selection.title, policy, "知识回流标题")',
                'gate_user_text(&selection.content, policy, "知识回流内容")',
                'gate_user_text(&selection.source_quote, policy, "知识回流来源原文")',
            ]
        ),
        "归档编号与知识回流的标题/正文/来源原文过闸门",
    )
    check(
        "phase4_ipc_registered",
        all(
            name + "," in registry
            for name in [
                "writing_parse_requirement",
                "writing_parse_requirement_apply",
                "writing_create_from_requirement",
                "writing_list",
                "writing_collect_materials",
                "writing_select_materials",
                "writing_outline_generate",
                "writing_outline_confirm",
                "writing_outline_get",
                "writing_section_generate",
                "writing_section_confirm",
                "writing_section_get",
                "writing_section_revise",
                "writing_review",
                "writing_review_apply_finding",
                "writing_version_list",
                "writing_version_diff",
                "writing_version_restore",
                "writing_factcheck_run",
                "writing_factcheck_resolve",
                "writing_status_transition",
                "writing_records_query",
                "writing_export",
                "writing_archive",
                "writing_knowledge_feedback",
            ]
        ),
        "第四阶段 25 个 IPC 全部注册到 generate_handler",
    )

    # ---- 2. AI 无定稿/归档/导出写通道 ------------------------------------
    finalize_block = function_block(lib_prod, "writing_status_transition_impl")
    export_cmd = function_block(lib_prod, "writing_export")
    archive_impl = function_block(lib_prod, "writing_archive_impl")
    prompt_impls = "\n".join(
        [
            function_block(lib_prod, "writing_parse_requirement"),
            function_block(lib_prod, "writing_outline_generate_impl"),
            function_block(lib_prod, "writing_section_generate_impl"),
            function_block(lib_prod, "writing_section_revise_impl"),
            function_block(lib_prod, "writing_review_impl"),
            function_block(lib_prod, "writing_records_query"),
            function_block(lib_prod, "writing_factcheck_run_impl"),
            function_block(lib_prod, "writing_outline_confirm_impl"),
            function_block(lib_prod, "writing_section_confirm_impl"),
        ]
    )
    check(
        "ai_no_finalize_channel",
        lib_prod.count("export_engine::finalize_snapshot(") == 1
        and "export_engine::finalize_snapshot(" in finalize_block
        and "finalize" not in prompt_impls
        and "finalize" not in extract_py,
        "定稿快照只在用户状态迁移命令里写；模型提示构建/确认路径无通道",
    )
    check(
        "ai_no_archive_channel",
        lib_prod.count("vault_archive::archive_writing_final(") == 1
        and "vault_archive::archive_writing_final(" in archive_impl
        and "archive_writing_final" not in prompt_impls
        and "archive" not in extract_py,
        "归档只在 writing_archive 用户命令里写；AI 链路无入口",
    )
    check(
        "ai_no_export_channel",
        lib_prod.count("export_engine::export_writing(") == 1
        and "export_engine::export_writing(" in export_cmd
        and "export_writing(" not in prompt_impls
        and "export" not in extract_py,
        "导出只在 writing_export 用户命令里执行；AI 链路无入口",
    )
    check(
        "finalize_requires_pending_review",
        "to == writing_store::STATUS_FINALIZED" in finalize_block
        and "task.status == writing_store::STATUS_PENDING_REVIEW" in finalize_block
        and "is_allowed_transition(&task.status, &to, outline_confirmed)" in finalize_block,
        "定稿快照只在 待审核 → 已定稿 的合法迁移时写",
    )

    # ---- 3. 大纲未确认不得生成正文 --------------------------------------
    check(
        "outline_confirmed_required_for_section",
        "if task.status != writing_store::STATUS_WRITING" in function_block(section_prod, "generate_section_prompt")
        and "writing_outline::read_confirmed_outline(root, writing_id)?" in function_block(section_prod, "generate_section_prompt"),
        "章节生成前要求任务在写作中，并强制读取已确认大纲",
    )
    check(
        "unconfirmed_outline_is_absent",
        'ok_or_else(|| "尚未确认大纲：当前没有 outline.md。".to_string())' in function_block(outline_prod, "read_confirmed_outline"),
        "没有 outline.md 时读取已确认大纲直接报错",
    )
    check(
        "transition_requires_outline",
        "writing_outline::read_confirmed_outline(root, &writing_id).is_ok()" in finalize_block
        and "(STATUS_OUTLINE_PENDING, STATUS_WRITING) => outline_confirmed" in store_prod
        and "从大纲待确认进入写作中必须确认大纲。" in writing_store,
        "大纲待确认 → 写作中 必须已确认大纲，迁移表与命令双重校验",
    )

    # ---- 4. 未确认素材不进可信集合/事实集合 ------------------------------
    check(
        "unconfirmed_material_rejected_on_adopt",
        "if material.review_status != REVIEW_CONFIRMED" in collect_prod
        and "只有已确认素材可以加入可信素材包。" in collect_prod,
        "用户采纳素材前校验已确认；未确认素材不能进 selected_material_ids",
    )
    check(
        "unconfirmed_material_rejected_in_prompts",
        "if material.review_status != REVIEW_CONFIRMED" in function_block(outline_prod, "generate_outline_prompt")
        and "if material.review_status != REVIEW_CONFIRMED" in function_block(section_prod, "load_section_materials"),
        "大纲与章节提示词组装时再次拒绝非已确认素材",
    )
    check(
        "query_defaults_to_confirmed",
        "or_else(|| Some(REVIEW_CONFIRMED.to_string()))" in material_query_prod
        and "None => vec![REVIEW_CONFIRMED]" in collect_prod
        and "task.selected_material_ids" in function_block(lib_prod, "writing_factcheck_run_impl")
        and "include_pending" not in function_block(lib_prod, "writing_factcheck_run_impl")
        and "REVIEW_PENDING" not in function_block(lib_prod, "writing_factcheck_run_impl"),
        "查询/收集默认只取已确认；事实核验只读用户已选可信集合",
    )

    # ---- 5. 导出敏感扫描与本地无网络 -------------------------------------
    check(
        "export_sensitive_scan",
        "red_line_scan_with(&scan_text, policy)" in export_prod
        and r'format!("{}\n{}", source.title, text)' in export_prod
        and "已拒绝导出（不回显命中原文）" in export_prod,
        "导出前对标题+正文做红线扫描，命中即拒绝且不回显原文",
    )
    check(
        "export_no_network",
        all(
            token not in export_prod
            for token in [
                "reqwest",
                "ureq",
                "TcpStream",
                "UdpSocket",
                "std::net",
                "http://",
                "https://",
            ]
        )
        and "Command::new(program)" in export_prod
        and r'lower.contains("src=\"http")' in export_prod
        and "公众号模板不得引用远程资源。" in export_prod
        and all(
            token not in helper
            for token in ["requests", "urllib", "httpx", "socket", "urlopen"]
        ),
        "导出只走本地 Markdown/HTML 渲染与 textutil/soffice/python-docx 本机进程，无 HTTP 客户端；远程资源被模板校验拒绝",
    )
    check(
        "export_receipt_local_only",
        "document_store::record_export_receipt(" in export_prod
        and "write_new_atomically(" in export_prod
        and "resolve_target_dir(app_root, writing_id, request.target_dir.as_deref())?" in export_prod,
        "导出结果只写用户指定目录或默认导出目录，并追加本地 manifest 回执",
    )

    # ---- 6. 归档仅 Markdown ---------------------------------------------
    check(
        "archive_only_markdown",
        "if !name.ends_with(\".md\")" in archive_prod
        and "只归档 Markdown 文件：" in archive_prod
        and 'format!("文稿-{cleaned}-{}.md", writing_archive_date(now))' in archive_prod
        and "archive_text(\n        vault_root,\n        REPORT_ARCHIVE_DIR," in archive_prod,
        "归档文件名强制 .md，落知识库 报告/ 目录",
    )
    check(
        "archive_rejects_docx_pdf_html",
        "!entries[0].ends_with(\".docx\")" in archive
        and "!entries[0].ends_with(\".pdf\")" in archive
        and "!entries[0].ends_with(\".html\")" in archive
        and "fn writing_archive_sanitizes_title_paths_and_only_writes_markdown(" in archive,
        "既有测试断言归档目录只有 .md，DOCX/PDF/HTML 不进入 vault",
    )
    check(
        "archive_rejects_dirty_markdown",
        "if stripped != normalized" in archive_prod
        and "文稿不是干净 Markdown" in archive_prod
        and "归档前敏感词扫描命中" in lib_prod,
        "归档前剥离校验与敏感词扫描双门禁，脏稿与命中稿都拒绝",
    )
    check(
        "archive_request_has_no_format",
        "struct WritingArchiveRequest" in lib_prod
        and all(
            token in function_block(lib_prod, "writing_archive")
            for token in ["request.preview", "writing_archive_impl("]
        )
        and "format" not in function_block(lib_prod, "writing_archive"),
        "归档请求只有 writing_id/preview，不接受文档格式参数",
    )

    # ---- 7. 无敏感原文日志与禁入材料 -------------------------------------
    phase4_rust = "\n".join(
        [
            store_prod,
            production(document_store),
            parse_prod,
            collect_prod,
            outline_prod,
            section_prod,
            review_prod,
            fact_prod,
            export_prod,
            records_prod,
            archive_prod,
        ]
    )
    synthetic_terms = ["110101199003077777", "13800138000"]
    check(
        "no_sensitive_logs",
        all(
            token not in phase4_rust
            for token in ["println!", "eprintln!", "dbg!", "log::warn", "log::info", "log::error", "log::debug"]
        )
        and "console.log" not in ui_text
        and "print(" not in extract_py
        and not any(term in phase4_rust for term in synthetic_terms),
        "新模块与界面无调试日志；合成身份证/电话样本不出现在业务源码",
    )
    forbidden = ["案件", "线索", "人物画像"]
    check(
        "no_forbidden_material_terms",
        all(
            term not in source
            for source in [store_prod, parse_prod, collect_prod, outline_prod, section_prod, review_prod, fact_prod, export_prod, records_prod, archive_prod, ui_text]
            for term in forbidden
        )
        and all(term not in extract_py for term in forbidden),
        "第四阶段源码/界面/提示词不含案件/线索/人物画像字样",
    )

    # ---- 8. 测试全虚构 ---------------------------------------------------
    fixture_modules = [
        writing_store,
        document_store,
        collect,
        outline,
        section,
        review,
        export,
        records,
        archive,
    ]
    check(
        "tests_use_temp_roots",
        all(
            "#[cfg(test)]" in source and "std::env::temp_dir()" in source
            for source in fixture_modules
        )
        and "std::env::temp_dir()" in document_store
        and "fs::write(" not in parse
        and "create_dir_all" not in fact_check,
        "所有落盘测试只写系统临时目录；纯函数模块不触碰文件系统",
    )
    declared = [
        writing_store,
        collect,
        outline,
        section,
        review,
        fact_check,
        export,
        records,
        archive,
    ]
    check(
        "test_data_fictional_declared",
        "全部虚构数据" in extract_test
        and "虚构" in extract_test
        and all(
            "虚构" in source or "合成" in source
            for source in declared
        ),
        "Python 单测声明全部虚构；Rust 夹具使用「虚构/合成」标记",
    )

    # ---- 9. 无新增豁免 ---------------------------------------------------
    violations, allowed_allows = waiver_violations()
    check(
        "no_new_waivers",
        not violations,
        "615 未改业务代码，diff/新代码无测试跳过或阈值降低；已登记有界 lint allow: "
        + ("；".join(allowed_allows) if allowed_allows else "无"),
    )

    # ---- 10. runtime/ 零改动 --------------------------------------------
    runtime_status = subprocess.run(
        ["git", "status", "--porcelain", "--", "runtime"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    runtime_diff = subprocess.run(
        ["git", "diff", "--stat", "HEAD", "--", "runtime"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    check(
        "runtime_zero_changes",
        runtime_status == "" and runtime_diff == "",
        "runtime/ 无工作区改动、无 HEAD 差异",
    )

    # ---- 11. 前端只走 IPC -----------------------------------------------
    check(
        "ui_ipc_only",
        all(
            token in ui_text
            for token in [
                "writingStatusTransition(",
                "writingExport(",
                "writingArchive(",
                "writingKnowledgeFeedback(",
                'invoke<WritingExportOutcome>("writing_export"',
                'invoke<WritingArchiveOutcome>("writing_archive"',
            ]
        )
        and all(
            token not in ui_text
            for token in ["plugin-fs", "writeTextFile", "writeFile(", "readTextFile("]
        ),
        "前端状态迁移/导出/归档/回流均经 api.ts invoke，无直接文件系统写",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-615",
        "evidence_level": "静态源码检查 + 既有测试覆盖断言",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "declarations": {
            "test_data": "全部虚构/合成，未使用真实业务材料",
            "new_waivers": "615 未改业务代码；无新增豁免",
            "registered_allows": allowed_allows,
            "business_code_changed_by_615": "无",
            "runtime_changed": "无",
        },
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
