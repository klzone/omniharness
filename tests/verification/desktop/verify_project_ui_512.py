#!/usr/bin/env python3
"""OD-KM-512 项目成果视图：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：ProjectBoard 追加概览而非重做事项表；project_list/project_overview/
material_list/material_get 调用；项目选择器；目标/状态/起止与四类计数；
素材按审核状态客户端分组且计数来自概览；素材详情项目跳转；无项目、
无关联、无素材各自空态；读失败不伪装为空；API 类型契约；无豁免、
无直接 invoke 与模型调用。

用法：python3 tests/verification/desktop/verify_project_ui_512.py [--json PATH]
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

    board = (SRC / "shell" / "ProjectBoard.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")

    check(
        "project_insight_mounted",
        "<ProjectInsight />" in board
        and 'data-project-overview=""' in board
        and 'data-project-materials=""' in board,
        "ProjectBoard 内挂载项目概览与素材区块",
    )
    check(
        "project_items_kept",
        all(
            item in board
            for item in [
                "data-project-board=\"\"",
                "data-project-card={st}",
                "data-project-gantt=\"\"",
                "recordsToItems(records, today)",
                "function confirmDelete()",
                "onSave(records.filter",
            ]
        ),
        "既有项目事项表、甘特与保存/删除路径未被重做",
    )
    check(
        "project_list_called",
        ".projectList()" in board and "setProjects(listing.items)" in board,
        "概览通过 shell.projectList 读取项目列表",
    )
    check(
        "overview_called",
        ".projectOverview(id)" in board and "setOverview(value)" in board,
        "选中项目后调用 508 project_overview",
    )
    check(
        "materials_filtered_by_project",
        ".materialList({ project_id: selectedId })" in board,
        "素材列表按选中项目过滤，未在客户端另行编造归属",
    )
    check(
        "material_detail_called",
        ".materialGet(id)" in board and "setMaterialDetail(detail)" in board,
        "点开素材时调用 material_get 读取正文与来源追溯",
    )
    check(
        "project_selector",
        'data-project-selector=""' in board
        and 'value={selectedId ?? ""}' in board
        and "selectProject(event.target.value || null)" in board,
        "项目选择器以 project_id 驱动概览与素材",
    )
    check(
        "overview_fields",
        all(
            item in board
            for item in [
                'data-project-goal=""',
                'data-project-dates=""',
                'data-project-status={shownProject.status}',
                "projectDateText(shownProject.start)",
                "projectDateText(shownProject.end)",
                '{shownProject.goal || "未填写目标"}',
            ]
        ),
        "概览展示目标、状态与起止日期，缺省值如实说明",
    )
    check(
        "four_counts",
        all(
            item in board
            for item in [
                '<ProjectCount label="任务" kind="tasks" value={overview?.task_count} />',
                '<ProjectCount label="记录" kind="records" value={overview?.record_count} />',
                '<ProjectCount label="素材" kind="materials" value={overview?.material_count} />',
                '<ProjectCount label="知识" kind="knowledge" value={overview?.knowledge_count} />',
                'data-project-count={kind}',
            ]
        ),
        "任务/记录/素材/知识四类计数均来自 project_overview",
    )
    check(
        "association_lists",
        all(
            item in board
            for item in [
                '<ProjectRefGroup title="任务" items={overview.tasks} />',
                '<ProjectRefGroup title="记录" items={overview.records} />',
                '<ProjectKnowledgeGroup knowledge={overview.knowledge} />',
                'data-project-ref-group="知识"',
                "knowledgeLabel(entry.knowledge_id)",
            ]
        ),
        "任务/记录/知识列表完整展示反向派生结果",
    )
    check(
        "review_grouping",
        all(
            status in board
            for status in ["待审核", "已确认", "已退回", "已拒绝", "已归档"]
        )
        and "REVIEW_ORDER.map((status)" in board
        and "materials.filter((material) => material.review_status === status)" in board,
        "素材在客户端按审核状态分组",
    )
    order_block = board.split("const REVIEW_ORDER", 1)[1].split("];", 1)[0]
    check(
        "pending_first",
        0 <= order_block.find('"待审核"') < order_block.find('"已确认"'),
        "待审核分组置顶",
    )
    check(
        "overview_material_count",
        "{overview ? overview.material_count : materialsLoading ? \"—\" : materials.length} 条" in board
        and 'data-project-material-group={group.status}' in board,
        "素材区块标题计数取概览全审核状态口径，分组另行标注",
    )
    check(
        "material_rows",
        'data-project-material-item={material.review_status}' in board
        and "setOpenMaterialId(active ? null : material.material_id)" in board,
        "素材行按审核状态标注并可展开详情",
    )
    check(
        "material_detail_view",
        all(
            item in board
            for item in [
                'data-project-material-detail=""',
                "material.content",
                "detail.source.quote",
                "detail.source.entity.title",
                "detail.source.location",
            ]
        ),
        "素材详情展示正文、来源引用与原文定位",
    )
    check(
        "project_jump_from_material",
        'data-material-project-jump={project.id}' in board
        and "onJumpProject(project.id)" in board
        and "function jumpToProject(projectId: string)" in board
        and "projects?.some((project) => project.project_id === projectId)" in board
        and "selectProject(projectId)" in board,
        "素材详情提供项目跳转入口，并校验目标项目在列表中",
    )
    check(
        "empty_no_projects",
        "projects.length === 0" in board
        and 'data-project-overview-empty=""' in board
        and "创建项目并关联任务、记录或知识后" in board,
        "无项目时给出独立空态",
    )
    check(
        "empty_no_associations",
        "associations === 0" in board
        and 'data-project-refs-empty=""' in board
        and "这个项目还没有关联任务、记录或知识" in board,
        "项目存在但无关联时给出独立空态",
    )
    check(
        "empty_no_materials",
        "materials.length === 0" in board
        and 'data-project-materials-empty=""' in board
        and "这个项目没有可展示的素材" in board,
        "项目存在但无素材时给出独立空态",
    )
    check(
        "errors_not_empty",
        all(
            item in board
            for item in [
                'data-project-overview-error="list"',
                'data-project-overview-error="detail"',
                'data-project-materials-error=""',
                "这里不把它当成「没有项目」",
                "这里不把它当成「没有关联」",
                "这里不把它当成「没有素材」",
            ]
        ),
        "读取失败与空态分开呈现",
    )
    check(
        "broken_counts_surfaced",
        "projectListBroken" in board
        and "overview.broken_tasks" in board
        and "materialsBroken" in board,
        "损坏文件数与静默丢弃区分",
    )
    check(
        "api_wrappers",
        all(
            name in api
            for name in [
                "projectCreate(draft: ProjectDraftInput)",
                "projectList(): Promise<ProjectListing>",
                "projectOverview(projectId: string): Promise<ProjectOverview>",
                "materialList(filter: MaterialFilter = {}): Promise<MaterialListing>",
                "materialGet(id: string): Promise<MaterialDetail>",
            ]
        ),
        "复用 508/506 的 api 封装，不新增重复 IPC",
    )
    check(
        "type_contract",
        all(
            item in types
            for item in [
                "export interface ProjectOverview",
                "task_count: number;",
                "record_count: number;",
                "material_count: number;",
                "knowledge_count: number;",
                "export interface MaterialFilter",
                "project_id?: string;",
                "export interface MaterialDetail",
                "related_project: MaterialTraceEntity | null;",
            ]
        ),
        "项目概览与素材详情类型契约齐全",
    )
    check(
        "no_direct_invoke",
        "invoke(" not in board,
        "页面只经 shell 封装调用，不直连 IPC",
    )
    check(
        "no_model_call",
        all(
            item not in board
            for item in ["fetch(", "task_run", "worklog_record_analyze(", "materialReview("]
        ),
        "本视图只读；不触发模型、不在本区块推进审核状态",
    )
    check(
        "no_exemptions",
        all(
            item not in board
            for item in ["@ts-ignore", "@ts-expect-error", "eslint-disable", "pytest.mark.skip"]
        ),
        "未新增类型检查或测试豁免",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-KM-512",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [
            {"name": name, "ok": ok, "detail": detail}
            for name, ok, detail in CHECKS
        ],
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
