#!/usr/bin/env python3
"""OD-WT-610 写作工作台页：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据、不写业务数据。
检查点覆盖任务卡：
  * WritingStudio 页签组新增「任务」工作台；
  * 九态分组、十二类筛选、进行中在前/已定稿与已归档置后；
  * 自然语言 -> 604 解析 -> 问题清单补全 -> 用户确认 -> 604 创建 -> 列表刷新；
  * 任务详情显示状态/类型/周期/素材数量，大纲与正文入口调用 606/607 只读命令；
  * 无任务/解析失败/读取失败/筛选为空各自空态；
  * 只经 shell/api 封装，不直连 Tauri invoke；不造演示数据。

用法：python3 tests/verification/desktop/verify_writing_ui_610.py [--json PATH]
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    workbench_path = SRC / "shell" / "WritingWorkbench.tsx"
    workbench = workbench_path.read_text(encoding="utf-8") if workbench_path.exists() else ""
    outline_editor = (SRC / "shell" / "WritingOutlineEditor.tsx").read_text(encoding="utf-8")
    section_editor = (SRC / "shell" / "WritingSectionEditor.tsx").read_text(encoding="utf-8")
    studio = (SRC / "shell" / "WritingStudio.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    lib = (REPO / "desktop" / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")

    check(
        "workbench_component_exists",
        workbench_path.exists() and "export function WritingWorkbench" in workbench,
        "新增 WritingWorkbench.tsx 并导出工作台组件",
    )
    check(
        "writing_studio_tab_integration",
        'import { WritingWorkbench } from "./WritingWorkbench"' in studio
        and 'data-writing-tab="workbench"' in studio
        and "<WritingWorkbench />" in studio
        and 'workspaceTab === "workbench"' in studio,
        "WritingStudio 页签组新增「任务」页签并渲染工作台",
    )
    check(
        "tab_group_keeps_pipeline_and_materials",
        'role="tablist"' in studio
        and 'data-writing-tab="pipeline"' in studio
        and 'data-writing-tab="materials"' in studio
        and 'aria-selected={workspaceTab === "pipeline"}' in studio
        and 'aria-selected={workspaceTab === "materials"}' in studio,
        "原稿件与素材篮页签保留，只新增任务视图",
    )
    check(
        "workbench_no_direct_invoke",
        "invoke<" not in workbench and "invoke(" not in workbench,
        "工作台只经 shell/api 封装，不直连 Tauri invoke",
    )
    check(
        "list_uses_writing_list",
        ".writingList({})" in workbench
        and 'data-writing-task-list=""' in workbench
        and "setItems(listing.items)" in workbench,
        "任务列表经 writingList 读取并落入列表状态",
    )
    check(
        "api_wrapper_and_command",
        "writingList(" in api
        and '"writing_list"' in api
        and "fn writing_list(" in lib
        and "writing_list," in lib
        and "list_writing_tasks" in lib,
        "api 封装 writingList 与宿主 writing_list 命令注册齐全",
    )
    check(
        "status_grouping_all_nine",
        all(
            status in workbench
            for status in (
                "需求待整理",
                "素材收集中",
                "大纲待确认",
                "写作中",
                "待核验",
                "待审核",
                "已定稿",
                "已归档",
                "已取消",
            )
        )
        and "STATUS_ORDER.map" in workbench
        and "data-writing-status-group={group.status}" in workbench,
        "九个写作状态都进入分组渲染",
    )
    check(
        "active_before_closed",
        workbench.index('"需求待整理"') < workbench.index('"已定稿"')
        and workbench.index('"写作中"') < workbench.index('"已归档"')
        and workbench.index('"待审核"') < workbench.index('"已定稿"'),
        "进行中状态排在已定稿/已归档之前",
    )
    check(
        "type_filter_all_twelve",
        all(
            value in workbench
            for value in (
                "月度总结",
                "年度总结",
                "专项汇报",
                "经验交流",
                "典型案例",
                "调研报告",
                "工作方案",
                "领导讲话",
                "工作简报",
                "专题分析",
                "会议发言",
                "个人述职",
            )
        )
        and "DOCUMENT_TYPES.map" in workbench
        and "data-writing-type-filter={type}" in workbench,
        "十二类文体都可筛选",
    )
    check(
        "type_filter_toggle",
        "setTypeFilter(typeFilter === type ? null : type)" in workbench
        and "aria-pressed={typeFilter === type}" in workbench,
        "类型筛选选中态可再次取消",
    )
    check(
        "create_flow_parse",
        "shell.writingRequirementPrepare(text)" in workbench
        and "host.submitTask({" in workbench
        and 'intent: "material_writing"' in workbench
        and "shell.writingRequirementApply(result.content)" in workbench,
        "自然语言先经 604 准备提示词、模型解析、宿主校验落地",
    )
    check(
        "create_flow_questions",
        "data-writing-questions=\"\"" in workbench
        and "draft.questions.map" in workbench
        and "data-writing-question-answer={index}" in workbench,
        "问题清单逐项渲染并可补全",
    )
    check(
        "create_flow_confirm",
        "data-writing-create-submit=\"\"" in workbench
        and "shell.writingCreateFromRequirement" in workbench
        and "!draft.title?.trim() || !draft.document_type" in workbench,
        "用户确认后调用 604 创建，类型与标题未定不能提交",
    )
    check(
        "list_refresh_after_create",
        "setReloadTick((n) => n + 1)" in workbench
        and "}, [reloadTick]);" in workbench,
        "创建成功后刷新任务列表",
    )
    check(
        "detail_meta",
        "data-writing-task-detail={task.writing_id}" in workbench
        and "task.selected_material_ids.length" in workbench
        and "task.selected_knowledge_ids.length" in workbench
        and "task.writing_period" in workbench,
        "详情显示状态/类型/周期/素材与知识数量",
    )
    check(
        "detail_outline_entry",
        "<WritingOutlineEditor writingId={task.writing_id}" in workbench
        and 'tab === "outline"' in workbench
        and ".writingOutlineGet(writingId)" in outline_editor
        and 'data-writing-outline-editor=""' in outline_editor,
        "详情大纲页签挂载 611 编辑器，编辑器调用 606 只读命令",
    )
    check(
        "detail_section_entry",
        "<WritingSectionEditor writingId={task.writing_id}" in workbench
        and 'tab === "section"' in workbench
        and ".writingSectionGet(writingId, selectedNo)" in section_editor
        and 'data-writing-section-editor=""' in section_editor,
        "详情正文页签挂载 611 编辑器，编辑器调用 607 只读命令",
    )
    check(
        "empty_state_no_tasks",
        "还没有写作任务。在上面描述要写什么，解析后创建。" in workbench
        and 'data-writing-empty=""' in workbench,
        "无任务空态真实，不展示占位任务",
    )
    check(
        "empty_state_filter",
        "这个类型下还没有任务。" in workbench,
        "类型筛选为空单独提示",
    )
    check(
        "empty_state_list_error",
        "读取写作任务失败：" in workbench
        and 'data-writing-list-error=""' in workbench
        and "正在读取写作任务…" in workbench,
        "读取中、读取失败与空态分开呈现",
    )
    check(
        "empty_state_parse_error",
        "解析失败：" in workbench
        and 'data-writing-parse-error=""' in workbench,
        "解析失败有独立错误态与重试入口",
    )
    check(
        "broken_files_reported",
        'data-writing-list-broken=""' in workbench
        and "未计入列表" in workbench,
        "损坏任务文件如实计数，不静默丢弃",
    )
    check(
        "sort_by_updated",
        "b.updated_at.localeCompare(a.updated_at)" in workbench,
        "同状态内按更新时间倒序",
    )
    check(
        "material_count_helper",
        "task.selected_material_ids.length + task.selected_knowledge_ids.length" in workbench,
        "素材包数量由任务已选素材与知识计数派生",
    )
    check(
        "no_fixture_data",
        not any(
            token in workbench
            for token in ("const SAMPLE", "mockTasks", "fixture", "示例任务", "演示任务")
        ),
        "组件内没有虚构任务或演示数据",
    )
    check(
        "style_uses_theme_tokens",
        "neu-inset-field" in workbench,
        "输入与新建表单使用既有 theme.css 新拟物令牌",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-610",
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

