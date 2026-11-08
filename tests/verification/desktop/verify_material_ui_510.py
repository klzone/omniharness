#!/usr/bin/env python3
"""OD-KM-510 素材篮页：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据、不写业务数据。
检查点覆盖任务卡：
  * 写作页「素材篮」页签与 WritingStudio 集成；
  * 类型分组、状态筛选、待审核置顶显式区分；
  * material_list / material_get / material_review / material_create 四个 506 封装；
  * 详情正文、来源引用、来源追溯与「打开原文」；
  * 批准/退回/拒绝/暂缓四个就地审核动作，退回与拒绝理由必填；
  * 用户收藏把正文作为 source_quote 传入；
  * 空态真实，不造数。

用法：python3 tests/verification/desktop/verify_material_ui_510.py [--json PATH]
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

    basket = (SRC / "shell" / "WritingMaterialBasket.tsx").read_text(encoding="utf-8")
    studio = (SRC / "shell" / "WritingStudio.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")

    check(
        "material_tab_in_writing_studio",
        'import { WritingMaterialBasket } from "./WritingMaterialBasket"' in studio
        and 'data-writing-tab="materials"' in studio
        and "<WritingMaterialBasket />" in studio
        and 'data-writing-pipeline=""' in studio,
        "写作页稿件卡内新增「素材篮」页签，并渲染独立素材组件",
    )
    check(
        "tab_group_keeps_pipeline",
        'role="tablist"' in studio
        and 'aria-selected={workspaceTab === "pipeline"}' in studio
        and 'aria-selected={workspaceTab === "materials"}' in studio
        and "setWorkspaceTab(\"pipeline\")" in studio,
        "页签只切换视图，稿件流程与筛选状态原样保留",
    )
    check(
        "basket_no_direct_invoke",
        "invoke<" not in basket and "invoke(" not in basket,
        "素材组件只经 shell/api 封装，不直连 Tauri invoke",
    )
    check(
        "list_uses_material_list",
        "shell" in basket and ".materialList({})" in basket and "data-material-list=" in basket,
        "素材列表经 506 的 materialList 读取",
    )
    check(
        "status_filter_all_five",
        all(status in basket for status in ("待审核", "已确认", "已退回", "已拒绝", "已归档"))
        and "data-material-status-filter={status}" in basket
        and "setStatusFilter(statusFilter === status ? null : status)" in basket,
        "五个审核状态都可筛选，选中态可再次取消",
    )
    check(
        "type_grouping_all_ten",
        all(t in basket for t in (
            "工作事实", "工作成果", "数据素材", "经验做法", "典型案例",
            "问题不足", "制度依据", "工作计划", "领导部署", "特色亮点",
        ))
        and "MATERIAL_TYPES.map" in basket
        and "data-material-group" in basket,
        "按素材十类分组渲染，分组只显示实际有内容的类型",
    )
    check(
        "pending_section_first_and_explicit",
        'data-material-pending=""' in basket
        and 'review_status === "待审核"' in basket
        and basket.index('data-material-pending=""') < basket.index('data-material-group='),
        "待审核单独成段并排在类型分组之前，有文字标签不只靠颜色",
    )
    check(
        "row_status_label_text",
        "data-material-status-label" in basket
        and "<StatusChip value={item.review_status} />" in basket,
        "每条素材显示审核状态文字标签",
    )
    check(
        "detail_uses_material_get",
        ".materialGet(selected)" in basket and "data-material-detail=" in basket,
        "详情经 materialGet 读取素材与追溯 payload",
    )
    check(
        "detail_shows_body_and_quote",
        "material.content" in basket
        and 'data-material-content=""' in basket
        and "引用原文" in basket
        and 'data-material-quote=""' in basket
        and "source.quote" in basket,
        "详情展示正文与素材保存的引用原文",
    )
    check(
        "source_trace_original_by_type",
        'case "work_record"' in basket
        and "shell.workRecordRead" in basket
        and 'case "task"' in basket
        and "shell.taskRead" in basket
        and 'case "knowledge"' in basket
        and "shell.knowledgeVaultReadNote" in basket
        and 'data-material-open-source=""' in basket,
        "「打开原文」按来源记录/任务/知识读取原件",
    )
    check(
        "source_peek_sheet_reused",
        'from "./SourcePeekSheet"' in basket
        and "<SourcePeekSheet" in basket
        and "open={peek !== null}" in basket,
        "原文预览复用既有 SourcePeekSheet",
    )
    check(
        "quote_missing_is_reported",
        "data-material-quote-missing" in basket and "引用未能在来源原文中定位" in basket,
        "引用定位失败时如实提示，不静默替换",
    )
    check(
        "review_uses_material_review",
        ".materialReview(materialId, action, reviewReason ?? null)" in basket
        and "shell.materialReview" in basket,
        "审核动作全部经 material_review IPC，不留前端直写",
    )
    check(
        "review_four_inline_actions",
        all(f'data-material-review-action="{action}"' in basket for action in (
            "confirm", "return", "reject", "defer"
        ))
        and "批准" in basket and "退回" in basket and "拒绝" in basket and "暂缓" in basket,
        "批准/退回/拒绝/暂缓在详情面板就地完成",
    )
    check(
        "return_reject_reason_required",
        'data-material-review-reason=""' in basket
        and "reviewAction === \"return\"" in basket
        and "reviewAction === \"reject\"" in basket
        and "!reason.trim()" in basket
        and "${reviewActionLabel}理由（必填）" in basket,
        "退回与拒绝先展开理由输入，空理由不能提交",
    )
    check(
        "create_uses_material_create",
        ".materialCreate({" in basket
        and 'source_type: "user"' in basket
        and "source_quote: content" in basket
        and 'data-material-create-submit=""' in basket,
        "用户收藏新建走 material_create，来源为 user 且正文同时作为 source_quote",
    )
    check(
        "create_requires_text",
        "const content = draftContent.trim()" in basket
        and '"收藏素材需要标题或正文。"' in basket
        and "disabled={busy || !draftContent.trim()}" in basket,
        "空内容不会提交，界面不预填示例素材",
    )
    check(
        "empty_state_is_real",
        "素材篮还是空的。从工作记录提取，或点「收藏素材」记录一条。" in basket
        and "这个筛选下还没有素材。" in basket
        and 'data-material-empty=""' in basket,
        "空态区分「本来为空」与「筛选为空」，不展示占位素材",
    )
    check(
        "loading_and_error_not_empty",
        "正在读取素材篮…" in basket
        and "读取素材篮失败" in basket
        and "重试" in basket,
        "读取中/失败与空态分开呈现",
    )
    check(
        "no_fixture_data",
        not any(token in basket for token in (
            "const SAMPLE", "mockMaterials", "fixture", "示例素材", "演示素材"
        )),
        "组件内没有虚构素材或演示数据",
    )
    check(
        "api_types_from_506_reused",
        all(name in api for name in ("materialList", "materialGet", "materialReview", "materialCreate"))
        and all(name in types for name in ("WritingMaterial", "MaterialDetail", "MaterialReviewAction")),
        "复用 506 的 api/types 封装，不新增重复类型",
    )
    check(
        "style_uses_theme_tokens",
        "neu-inset-field" in basket and "neu-chip" in basket,
        "输入与收藏按钮直接使用既有 theme.css 新拟物令牌",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-KM-510",
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
