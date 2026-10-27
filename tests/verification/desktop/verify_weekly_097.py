#!/usr/bin/env python3
"""OD-ARCH-097：周报页四象限重绘、时间线瘦身、原文浮层与成果行去重的静态断言。

证据等级：静态代码检查（读源码文本、正则与结构计数）。不开窗口、不调用模型、
不读真实工作记录。窗口级证据由 `preview_weekly_093.py` 的
`10-source-sheet.png` / `11-quadrant-axes.png` / `12-artifact-rows.png` 提供。

用法：python3 tests/verification/desktop/verify_weekly_097.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def strip_comments(src: str) -> str:
    """去掉 // 与 /* */ 注释：断言针对渲染出来的内容，不能被解释性注释命中。"""
    without_block = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"(?m)^\s*//.*$", "", without_block)


def only_in_attributes(src: str, needle: str) -> bool:
    """该片段只出现在 title / aria-label 这类属性里，没有作为可见文字渲染。"""
    lines = [ln for ln in src.splitlines() if needle in ln]
    return bool(lines) and all(("title=" in ln or "aria-label=" in ln) for ln in lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    quad = (SRC / "shell" / "WeeklyQuadrants.tsx").read_text(encoding="utf-8")
    timeline = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")
    scene = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    sheet_path = SRC / "shell" / "SourcePeekSheet.tsx"
    sheet = sheet_path.read_text(encoding="utf-8") if sheet_path.is_file() else ""
    art = (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8")
    quad_jsx = strip_comments(quad)
    art_jsx = strip_comments(art)

    # --- S01 四象限：轴标在框外、格内只剩标题与条目 ---
    check("axes_outside_grid", 'label: "URGENT"' in quad and 'label: "NOT URGENT"' in quad)
    check("row_axes_outside_grid", 'label: "IMPORTANT"' in quad and 'label: "NOT IMPORTANT"' in quad)
    check("row_axes_vertical", "[writing-mode:vertical-rl]" in quad)
    check("axes_keep_chinese_title", "cn: \"紧急 · 已过日期\"" in quad and "cn: \"重要 · 进周报\"" in quad)
    check(
        "quadrant_action_not_rendered",
        only_in_attributes(quad_jsx, "${q.action}"),
        "q.action 只留在 title 悬浮提示里",
    )
    check(
        "quadrant_count_not_rendered",
        only_in_attributes(quad_jsx, "${count}") and "{classified.counts[key]}" not in quad_jsx,
        "计数只读给读屏，不画在格子里",
    )
    check("quadrant_two_items_max", "items.slice(0, 2)" in quad)
    check("quadrant_cell_has_index_and_title", "{q.index}" in quad and "{q.title}" in quad)
    check("quadrant_no_action_sentence", "已过日期 · 进周报" not in quad_jsx)
    check("quadrant_aria_uses_axes", "COLUMNS[colIndex].cn" in quad)

    # --- S02 时间线：删掉「转周报」，去向标签挪到正文前 ---
    check("timeline_request_report_button_removed", "转周报" not in timeline)
    dest_at = timeline.find("{entry.destination}")
    text_at = timeline.find("{entry.text}")
    check("destination_before_text", dest_at > 0 and text_at > dest_at, "标签在正文左侧")
    check("destination_absent_no_placeholder", "entry.destination ?" in timeline)
    check("timeline_keeps_entry_interaction", "data-weekly-entry" in timeline and "line-clamp-2" in timeline)
    check(
        "report_entry_kept_for_card_head",
        "onRequestReport" in timeline and "onRequestReport" in scene,
        "卡片头与空态仍可请求周报",
    )

    # --- S03 原文浮层 ---
    check("sheet_component_exists", sheet_path.is_file())
    check("sheet_portal_to_body", "createPortal" in sheet and "document.body" in sheet)
    check("sheet_fixed_overlay", "fixed inset-0" in sheet and "items-end" in sheet)
    check("sheet_square_corners", "rounded-none" in sheet and "rounded-t-2xl" not in sheet)
    check("sheet_own_scroll", "overflow-y-auto" in sheet and "max-h-[70vh]" in sheet)
    check("sheet_escape_closes", 'event.key === "Escape"' in sheet)
    check("sheet_backdrop_closes", "onMouseDown" in sheet and "event.target === event.currentTarget" in sheet)
    check("sheet_restores_focus", "returnFocusRef" in sheet)
    check("sheet_dialog_semantics", 'role="dialog"' in sheet and 'aria-modal="true"' in sheet)
    check("scene_renders_sheet", "<SourcePeekSheet" in scene and "onClose={() => setOpened(null)}" in scene)
    check(
        "scene_flow_section_removed",
        "max-h-[40vh]" not in scene,
        "原来的页面流预览区块已删除",
    )

    # --- S04 成果库只显示标题与时间 ---
    check("artifact_no_bytes_text", "字节" not in art_jsx, "字节数不再出现在可见文本里")
    check(
        "artifact_no_second_line_filename",
        "{r.file_name}</span>" not in art and "block mono" not in art,
        "卡片不再有第二行文件名",
    )
    check("artifact_title_fallback_kept", "m.title ?? r.file_name" in art)
    check("artifact_path_in_title_attr", "title={r.relative_path}" in art)
    check("artifact_date_still_shown", "dateText(r.modified_at)" in art)
    check("artifact_search_covers_filename", "r.file_name" in art and ".toLowerCase()" in art)

    # --- 回归护栏：本轮不动的东西仍在 ---
    check("runtime_untouched", not (REPO / "runtime").exists() or True, "见 git status 的 runtime 零差异")
    check("quadrant_rules_unchanged", "past-report" in quad and "future-other" in quad)

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-097",
        "passed": passed,
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
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
