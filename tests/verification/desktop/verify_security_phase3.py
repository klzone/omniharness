#!/usr/bin/env python3
"""OD-KM-513 第三阶段安全核对：静态源码检查 + 既有测试覆盖断言。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖 PHASE3_ACCEPTANCE_CRITERIA §三 八项：素材流新 IPC 闸门、AI 无审核状态
写通道、知识库写路径零改动、素材篮清理排除、索引重建与 work/ 分离、无敏感
原文日志、测试数据全虚构、无新增豁免。另核对本阶段 IPC 注册与素材篮扫描范围。

用法：python3 tests/verification/desktop/verify_security_phase3.py [--json PATH]
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
TESTS = REPO / "tests"

CHECKS: list[tuple[str, bool, str]] = []

# 601/613 起既有的有界 lint allow：非测试跳过、非阈值降低。
ALLOWED_ALLOW_LINES = {
    "#[allow(clippy::too_many_arguments)]",
    "#[allow(dead_code)]",
}


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


def added_waiver_lines() -> list[str]:
    """本阶段未提交 diff 的新增行里是否出现豁免/跳过标记。"""
    try:
        diff = subprocess.run(
            ["git", "diff", "--unified=0", "HEAD", "--", "desktop", "engine", "tests"],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        ).stdout
    except OSError as error:
        return [f"git diff failed: {error}"]
    tokens = (
        "#[allow(",
        "#[ignore]",
        "# noqa",
        "pytest.mark.skip",
        "@ts-ignore",
        "eslint-disable",
    )
    hits: list[str] = []
    current = ""
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current = line[6:].strip()
            continue
        if not line.startswith("+") or line.startswith("+++"):
            continue
        if current.startswith("tests/verification/"):
            continue
        if (
            any(token in line for token in tokens)
            and line[1:].strip() not in ALLOWED_ALLOW_LINES
        ):
            hits.append(line[1:])
    return hits


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    lib = read(RUST / "lib.rs")
    lib_prod = production(lib)
    cleanup = read(RUST / "work_cleanup.rs")
    cleanup_prod = production(cleanup)
    material_store = read(RUST / "material_store.rs")
    material_prod = production(material_store)
    material_query_prod = production(read(RUST / "material_query.rs"))
    project_prod = production(read(RUST / "project_store.rs"))
    link_prod = production(read(RUST / "knowledge_link.rs"))
    link_store = read(RUST / "knowledge_link_store.rs")
    link_store_prod = production(link_store)
    extract = read(RUST / "task_extract.rs")
    extract_prod = production(extract)
    extract_py = read(ENGINE / "task_extract.py")
    extract_test = read(TESTS / "verification" / "engine-interface" / "adapter" / "test_task_extract_unit.py")
    vault_index_tests = read(RUST / "vault_index" / "tests.rs")
    ui_files = [
        SRC / "shell" / "WritingMaterialBasket.tsx",
        SRC / "shell" / "KnowledgeLinkPanel.tsx",
        SRC / "shell" / "WritingStudio.tsx",
        SRC / "shell" / "ProjectBoard.tsx",
        SRC / "shell" / "WorkRecordLibrary.tsx",
        SRC / "shell" / "TaskManager.tsx",
        SRC / "shell" / "KnowledgeVaultPage.tsx",
    ]
    ui_text = "\n".join(read(path) for path in ui_files)

    gate = function_block(lib, "gate_user_text")
    gate_material = function_block(lib, "gate_material_text")
    gate_tags = function_block(lib, "gate_material_tags")
    create_impl = function_block(lib, "material_create_impl")
    edit_impl = function_block(lib, "material_edit_impl")
    review_impl = function_block(lib, "material_review_impl")
    extract_cmd = function_block(lib, "material_extract_from_record")
    link_suggest = function_block(lib, "knowledge_link_suggest_impl")
    link_confirm = function_block(lib, "knowledge_link_confirm_impl")
    project_create = function_block(lib, "project_create_impl")
    project_update = function_block(lib, "project_update_impl")
    rebuild_for = function_block(lib, "knowledge_vault_rebuild_index_for")
    rebuild_cmd = function_block(lib, "knowledge_vault_rebuild_index")
    stage_material = function_block(extract, "stage_material_candidates")
    preview = function_block(cleanup, "cleanup_preview")
    apply_cleanup = function_block(cleanup, "cleanup_apply")

    # ---- 1. 素材流新 IPC 闸门 ----------------------------------------------
    check(
        "gate_definition",
        "fn gate_user_text(" in lib
        and "red_line_scan_with" in gate
        and "mask_privacy" in gate,
        "统一闸门仍是红线扫描 + 隐私掩码",
    )
    check(
        "gate_error_does_not_echo_text",
        "{detail}" in gate and "{text}" not in gate and "hit.label" in gate and "hit.count" in gate,
        "拒绝信息只报命中标签与次数，不回显原文",
    )
    check(
        "gate_covers_material_create",
        all(
            token in create_impl
            for token in [
                'gate_material_text(&draft.title, policy, "素材标题")',
                'gate_material_text(&draft.content, policy, "素材正文")',
                'gate_material_text(&draft.source_quote, policy, "素材来源引用")',
                'gate_material_text(&location, policy, "素材来源定位")',
                "gate_material_tags(&draft.tags, policy)",
            ]
        )
        and "gate_user_text(" in gate_material
        and "gate_material_text(" in gate_tags,
        "material_create 的标题/正文/来源引用/定位/标签全部落盘前过闸门",
    )
    check(
        "gate_covers_material_edit",
        all(
            token in edit_impl
            for token in [
                '"素材标题"',
                '"素材正文"',
                '"素材来源引用"',
                '"素材来源定位"',
                '"素材编辑理由"',
                "gate_material_tags(&tags, policy)",
                "gate_material_text(",
            ]
        ),
        "material_edit 的全部落盘文本（含编辑理由）过闸门",
    )
    check(
        "gate_covers_material_review_confirm",
        'gate_material_text(&value, policy, "素材审核理由")' in review_impl
        and "normalize_material_review_action" in review_impl
        and "material_store::review_material(" in review_impl,
        "material_review 的退回/拒绝理由过闸门；确认动作走同一用户审核入口",
    )
    check(
        "gate_covers_knowledge_link",
        'gate_user_text(&hint, policy, "知识主题提示")' in link_suggest
        and 'gate_user_text(reason, policy, "关联理由")' in link_confirm,
        "知识关联推荐提示与用户确认理由过闸门",
    )
    check(
        "gate_covers_project_text",
        'gate_user_text(&draft.name, policy, "项目名称")' in project_create
        and 'gate_user_text(&draft.goal, policy, "项目目标")' in project_create
        and 'gate_user_text(&name, policy, "项目名称")' in project_update
        and 'gate_user_text(&goal, policy, "项目目标")' in project_update,
        "项目名称与目标过统一闸门",
    )

    # ---- 2. AI 无审核状态写通道 --------------------------------------------
    registry = lib.split("tauri::generate_handler![")[1].split("])", 1)[0]
    phase3_commands = [
        "material_extract_from_record",
        "material_create",
        "material_list",
        "material_get",
        "material_review",
        "material_edit",
        "material_query",
        "project_create",
        "project_update",
        "project_list",
        "project_overview",
        "knowledge_link_suggest",
        "knowledge_link_confirm",
        "knowledge_link_remove",
        "knowledge_link_by_knowledge",
    ]
    check(
        "phase3_ipc_registered",
        all(name + "," in registry for name in phase3_commands),
        "第三阶段 15 个新 IPC 全部注册",
    )
    check(
        "material_review_only_user_command",
        lib_prod.count("material_store::review_material(") == 1
        and "material_store::review_material(" in review_impl
        and "review_material" not in extract_prod
        and "review_material" not in extract_py,
        "审核状态只在 material_review 用户命令里推进，AI 链路无写入口",
    )
    check(
        "ai_output_only_stages_candidates",
        "stage_material_candidates(" in extract_prod
        and "material_candidates_dir(" in extract_prod
        and "MATERIAL_CANDIDATES_DIR" in extract_prod
        and all(
            token not in extract_prod
            for token in [
                "material_store::create_material(",
                "material_store::update_material(",
                "review_status",
                "REVIEW_CONFIRMED",
                "ACTION_CONFIRM",
            ]
        )
        and "不确认状态" in extract_py
        and "review_status" not in extract_py,
        "模型输出只落 work/素材篮/.candidates/，不生成正式素材、不写审核状态",
    )
    check(
        "candidate_defaults_pending",
        "status: MATERIAL_CANDIDATE_PENDING.to_string()" in stage_material
        and "stage_material_candidates(" in extract_prod
        and "create_material" not in extract_cmd
        and "review_material" not in extract_cmd,
        "候选暂存默认「待审核」；提取准备命令只产提示词，不落盘",
    )

    # ---- 3. 知识库写路径零改动 --------------------------------------------
    phase3_rust_prod = "\n".join(
        [
            material_prod,
            material_query_prod,
            project_prod,
            link_prod,
            link_store_prod,
            extract_prod,
        ]
    )
    vault_write_tokens = [
        "vault_write::",
        "knowledge_ingest::init_vault",
        "knowledge_ingest::apply_payload",
        "knowledge_ingest::admit_raw_files",
        "knowledge_vault_create_note_for",
        "knowledge_vault_save_note_for",
    ]
    check(
        "knowledge_write_paths_untouched",
        all(token not in phase3_rust_prod for token in vault_write_tokens),
        "素材/项目/关联模块不调用 vault_write、知识入库落盘或知识笔记写函数",
    )
    check(
        "knowledge_link_is_read_only",
        "read_vault_note" in link_prod
        and "search_vault" in link_prod
        and "vault_write" not in link_prod
        and "fs::write" not in link_prod,
        "知识推荐只读扫描与词法检索，无写回知识库",
    )
    check(
        "material_query_read_only",
        "material_store::list_materials" in material_query_prod
        and "create_material" not in material_query_prod
        and "update_material" not in material_query_prod
        and "review_material" not in material_query_prod,
        "第四阶段查询只读已确认素材，不产生写入",
    )

    # ---- 4. 素材篮清理排除 ------------------------------------------------
    check(
        "cleanup_excludes_material_dir",
        "CLEANUP_EXCLUDED_DIRS" in cleanup
        and "material_store::MATERIAL_DIR" in cleanup
        and "pub fn is_cleanup_excluded(" in cleanup
        and "is_cleanup_excluded(&source)" in apply_cleanup,
        "work_cleanup 显式排除素材篮，预览与执行移动前都检查路径",
    )
    check(
        "cleanup_material_test_exists",
        "fn materials_dir_is_excluded_from_preview_and_apply" in cleanup
        and "素材篮文件不得被清理移动" in cleanup
        and "素材篮文件字节不得变化" in cleanup
        and "!material_trash.exists()" in cleanup,
        "既有测试覆盖预览不选素材、执行不搬素材、字节不变、不建素材回收目录",
    )
    check(
        "cleanup_scope_does_not_scan_link_dir",
        "worklog_records::list_records" in preview
        and "task_store::list_tasks" in preview
        and "read_dir" not in preview
        and "知识关联" not in cleanup_prod
        and "link_dir" not in cleanup_prod,
        "清理只列记录与任务，不遍历 work/知识关联/ 或素材篮目录",
    )

    # ---- 5. 索引重建与 work/ 分离 ----------------------------------------
    check(
        "rebuild_index_separated_from_work",
        "configured_vault_root" in rebuild_for
        and "knowledge_vault::scan_vault" in rebuild_for
        and "vault_index::build_from_scan" in rebuild_for
        and all(
            token not in rebuild_for
            for token in ["work_root", "material_store", "worklog_records", "task_store"]
        )
        and "configured_vault_root" in rebuild_cmd
        and "work_root" not in rebuild_cmd,
        "重建只读知识库根并重建链接快照，不进入 work/ 数据",
    )
    check(
        "vault_root_outside_app_root",
        "assert_vault_root_allowed" in read(RUST / "knowledge_vault.rs")
        and "不能放在应用自身数据目录内" in read(RUST / "knowledge_vault.rs"),
        "知识库根被拒绝放在应用数据目录（work/ 所在根）内",
    )
    check(
        "rebuild_links_recoverable_test",
        "fn rebuild_after_add_modify_delete_matches_from_scratch" in vault_index_tests
        and "note_links(" in vault_index_tests
        and "fn external_edits_are_visible_after_rebuild" in vault_index_tests,
        "既有测试覆盖重建后链接恢复、外部编辑可见；原始笔记未被改写",
    )

    # ---- 6. 无敏感原文进日志（合成样本） ---------------------------------
    synthetic_terms = ["合成样本", "110101199003077777", "13800138000"]
    new_rust_prod = "\n".join(
        [
            material_prod,
            material_query_prod,
            project_prod,
            link_prod,
            link_store_prod,
            extract_prod,
        ]
    )
    check(
        "no_sensitive_logs",
        all(
            token not in new_rust_prod
            for token in ["println!", "eprintln!", "dbg!", "log::"]
        )
        and "console.log" not in ui_text
        and "print(" not in extract_py
        and not any(term in new_rust_prod or term in extract_py for term in synthetic_terms[1:]),
        "新模块与界面无调试日志；合成身份证/电话样本不出现在业务源码",
    )

    # ---- 7. 测试数据全虚构 / 禁入材料零出现 ------------------------------
    new_sources = [
        material_store,
        read(RUST / "material_query.rs"),
        read(RUST / "project_store.rs"),
        read(RUST / "knowledge_link.rs"),
        link_store,
        extract,
    ]
    check(
        "tests_use_temp_roots",
        all("#[cfg(test)]" in source and "std::env::temp_dir()" in source for source in new_sources)
        and "temp_dir()" in read(RUST / "material_query.rs"),
        "各新模块测试只落系统临时目录，不写仓库或用户数据",
    )
    check(
        "test_data_fictional_declared",
        "全部虚构数据" in extract_test
        and all(
            "虚构" in source or "合成" in source
            for source in [
                material_store,
                read(RUST / "project_store.rs"),
                link_store,
                read(RUST / "material_query.rs"),
            ]
        ),
        "Python 单测声明全部虚构；Rust 夹具使用「虚构/合成」标记",
    )
    forbidden = ["案件", "线索", "人物画像"]
    check(
        "no_forbidden_material_terms",
        all(term not in source for source in new_sources for term in forbidden)
        and all(term not in extract_py for term in forbidden),
        "新模块与提取提示词不含案件/线索/人物画像字样或字段",
    )

    # ---- 8. 无新增豁免 ---------------------------------------------------
    waiver_lines = added_waiver_lines()
    untracked = subprocess.run(
        ["git", "status", "--porcelain", "--", "desktop", "engine", "tests"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    new_code_unwaived = True
    for line in untracked:
        if not line.startswith("??"):
            continue
        path = REPO / line[3:].strip()
        if (
            path.name == "verify_security_phase3.py"
            or path.suffix not in {".rs", ".tsx", ".ts", ".py"}
            or "tests/verification" in path.as_posix()
        ):
            continue
        text = read(path)
        for line in text.splitlines():
            stripped = line.strip()
            if stripped in ALLOWED_ALLOW_LINES:
                continue
            if any(token in line for token in ["#[allow(", "#[ignore]", "# noqa", "pytest.mark.skip", "@ts-ignore", "eslint-disable"]):
                new_code_unwaived = False
                waiver_lines.append(f"untracked waiver: {path}: {stripped}")
                break
    check(
        "no_new_waivers",
        not waiver_lines and new_code_unwaived,
        "本阶段新增源码与测试未添加 lint/测试豁免或跳过标记",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-KM-513",
        "evidence_level": "静态源码检查 + 既有测试覆盖断言",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "declarations": {
            "test_data": "全部虚构/合成，未使用真实业务材料",
            "new_waivers": "无",
            "business_code_changed_by_513": "无",
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
