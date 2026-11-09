#!/usr/bin/env python3
"""OD-WT-611 大纲编辑器 + 分章写作界面：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：两个组件文件存在与 export 接口、606/607 IPC 接线、大纲树字段与确认前编辑、
确认后锁定与重新生成确认弹窗、章节选择联动、章节生成/修订预览/确认链路、
AI 建议与用户确认标注、右栏本章素材与 603 引用列表、SourcePeekSheet 复用、
609 annotations 随核验请求传入、真实空态、组件不直连 Tauri invoke、
未引用 runtime/、未使用 window.confirm。

用法：python3 tests/verification/desktop/verify_writing_editor_611.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SHELL = REPO / "desktop" / "src" / "shell"

OUTLINE = SHELL / "WritingOutlineEditor.tsx"
SECTION = SHELL / "WritingSectionEditor.tsx"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    outline = OUTLINE.read_text(encoding="utf-8") if OUTLINE.exists() else ""
    section = SECTION.read_text(encoding="utf-8") if SECTION.exists() else ""

    check(
        "components_exist",
        OUTLINE.exists() and SECTION.exists(),
        "两个组件文件在 desktop/src/shell/",
    )
    check(
        "outline_export_interface",
        "export interface WritingOutlineEditorProps" in outline
        and "export function WritingOutlineEditor(" in outline
        and "writingId: string" in outline
        and "onOutlineConfirmed?" in outline,
        "导出组件与挂载 props（writingId / onOutlineConfirmed / className）",
    )
    check(
        "section_export_interface",
        "export interface WritingSectionEditorProps" in section
        and "export function WritingSectionEditor(" in section
        and "writingId: string" in section
        and "onSectionConfirmed?" in section,
        "导出组件与挂载 props（writingId / onSectionConfirmed / className）",
    )
    check(
        "outline_uses_606_api",
        ".writingOutlineGet(writingId)" in outline
        and ".writingOutlineGenerate({ writing_id: writingId })" in outline
        and ".writingOutlineConfirm({" in outline
        and "writing_id: writingId" in outline,
        "只经 writingOutlineGenerate/Confirm/Get 三条 606 IPC",
    )
    check(
        "outline_tree_fields",
        all(
            token in outline
            for token in [
                "section.heading",
                "section.sub_headings",
                "section.focus",
                "section.material_ids.length",
                "section.knowledge_ids.length",
                "section.expected_words",
            ]
        ),
        "左栏树展示章节/子标题/重点/素材映射/预计字数",
    )
    check(
        "outline_form_editable",
        "function patchOutline(" in outline
        and "function patchSection(" in outline
        and "<LabeledField" in outline
        and "disabled={locked}" in outline
        and "data-outline-add" in outline
        and "data-outline-remove" in outline,
        "确认前按 JSON 字段拆表单编辑，可增删章节",
    )
    check(
        "outline_lock_after_confirm",
        "setMode(\"confirmed\")" in outline
        and "const locked = mode === \"confirmed\"" in outline
        and 'data-outline-mode={mode}' in outline
        and "disabled={locked}" in outline,
        "确认后 mode=confirmed，整块字段锁定",
    )
    check(
        "outline_regenerate_gate",
        "data-outline-regenerate" in outline
        and "<ConfirmDialog" in outline
        and 'testId="outline-regenerate-confirm"' in outline
        and "重新生成" in outline,
        "锁定后换稿必须显式「重新生成」并经应用内确认弹窗",
    )
    check(
        "outline_ai_vs_user_badge",
        'data-outline-badge="ai"' in outline
        and 'data-outline-badge="confirmed"' in outline
        and "AI 建议" in outline
        and "用户已确认" in outline,
        "大纲建议与用户确认状态显式区分（照 410 标注先例）",
    )
    check(
        "outline_real_empty_state",
        "还没有大纲" in outline
        and "正在读取大纲" in outline
        and "生成大纲" in outline,
        "空态区分读取中与尚未生成，不伪造默认大纲",
    )
    check(
        "outline_model_call_pattern",
        "host.submitTask({" in outline
        and "intent: \"material_writing\"" in outline
        and "TERMINAL_STATES.includes" in outline
        and "host.taskResult(" in outline,
        "宿主返回提示词后，界面侧走既有 submitTask 轮询取结果",
    )
    check(
        "outline_source_peek",
        'from "./SourcePeekSheet"' in outline
        and "<SourcePeekSheet" in outline
        and "shell.materialGet(id)" in outline
        and "shell.knowledgeVaultReadNote(id)" in outline,
        "右栏素材映射可点开原素材与知识原文",
    )
    check(
        "section_uses_607_api",
        ".writingSectionGet(writingId, selectedNo)" in section
        and ".writingSectionGenerate({" in section
        and ".writingSectionConfirm({ writing_id: writingId, draft })" in section
        and ".writingSectionRevise({" in section,
        "只经 607 Generate/Confirm/Get/Revise 四条 IPC",
    )
    check(
        "section_chapter_select",
        "data-section-tree-item" in section
        and "onClick={() => setSelectedNo(section.no)}" in section
        and "outline.outline.sections.find" in section,
        "左栏章节选择驱动中栏与右栏",
    )
    check(
        "section_generate_preview_confirm",
        "data-section-generate" in section
        and "data-section-draft" in section
        and "setDraft(parseSectionDraft(text, selected.no))" in section
        and "data-section-confirm" in section
        and "确认采用" in section,
        "生成→预览→用户确认链路完整",
    )
    check(
        "section_revise_kinds",
        'const REVISE_KINDS: WritingReviseKind[] = ["精简", "扩展", "语气", "术语统一", "消除重复"]' in section
        and "data-section-revise-kind" in section
        and "data-section-revise-instruction" in section,
        "修订下拉五类 + 自由文本补充说明",
    )
    check(
        "section_revise_apply_flow",
        'data-section-revise=""' in section
        and "data-draft-origin={draftOrigin}" in section
        and 'draftOrigin === "revised" ? "应用修订"' in section,
        "修订建议同样先预览，确认应用才写正文",
    )
    check(
        "section_ai_vs_user_badge",
        "AI 建议" in section
        and "用户确认" in section
        and "data-section-confirmed" in section
        and "data-draft-origin" in section,
        "未确认稿标 AI 建议，已确认正文标用户确认",
    )
    check(
        "section_right_materials",
        "data-section-materials" in section
        and "data-section-material=" in section
        and "data-section-knowledge=" in section
        and "effectiveMaterialIds" in section
        and "沿用任务素材" in section,
        "右栏本章素材列表；章节未映射时显式沿用任务已选素材",
    )
    check(
        "section_citations_603",
        "data-section-citations" in section
        and "data-section-citation=" in section
        and "source_kind === \"material\" ? \"素材\" : \"知识\"" in section
        and "confirmedCitations" in section,
        "右栏本章引用列表按 603 来源类型区分素材/知识",
    )
    check(
        "section_factcheck_annotations",
        "function annotationsFor(draft: WritingSectionDraft): FactCheckAnnotationInput[]" in section
        and "shell.writingFactcheckRun({" in section
        and "annotations," in section
        and "stats.annotations_used" in section,
        "609 遗留：确认后自动核验并随车传入四分法 annotations",
    )
    check(
        "section_source_peek",
        'from "./SourcePeekSheet"' in section
        and "<SourcePeekSheet" in section
        and "data-section-citation-source" in section
        and "shell.materialGet(id)" in section
        and "shell.knowledgeVaultReadNote(id)" in section,
        "右栏素材与引用来源都可点开 SourcePeekSheet",
    )
    check(
        "section_real_empty_states",
        all(
            token in section
            for token in [
                "尚未确认大纲。",
                "确认大纲后再开始分章写作。",
                "本章还没有稿件。",
                "本章没有素材。",
                "本章还没有引用。",
            ]
        ),
        "大纲缺失、章节无稿、无素材、无引用均有真实空态",
    )
    check(
        "no_direct_tauri_invoke",
        "invoke(" not in outline
        and "invoke(" not in section
        and "@tauri-apps" not in outline
        and "@tauri-apps" not in section,
        "组件只经 api.ts 的 shell/host，不直连 Tauri",
    )
    check(
        "no_window_confirm",
        "window.confirm" not in outline and "window.confirm" not in section,
        "应用内确认框替代 window.confirm",
    )
    check(
        "no_runtime_reference",
        "runtime/" not in outline and "runtime/" not in section,
        "不引用 runtime/ 内 DSH Runtime",
    )
    check(
        "no_direct_writes",
        all(
            token not in (outline + section)
            for token in ["saveArtifact", "offlineArtifactSave", "materialReview", "writeFile"]
        ),
        "没有绕过确认门的直接写路径",
    )
    check(
        "no_commit_or_push",
        "git commit" not in outline and "git push" not in section,
        "组件源码不含提交/推送动作",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-611",
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
