#!/usr/bin/env python3
"""OD-ARCH-099：周报页四项微调的静态断言。

证据等级：静态代码检查（读源码文本、正则与结构计数）。不开窗口、不调用模型、
不读真实工作记录。窗口级证据由 `preview_weekly_093.py` 的截图提供，
本次补充 `evidence/od-arch-099/`。

用法：python3 tests/verification/desktop/verify_weekly_099.py [--json PATH]
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
RUST = DESKTOP / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def strip_comments(src: str) -> str:
    """去掉 // 与 /* */ 注释：断言针对渲染出来的内容，不能被解释性注释命中。"""
    without_block = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"(?m)^\s*//.*$", "", without_block)


def band_metric_body(src: str) -> str:
    start = src.find("function BandMetric(")
    if start < 0:
        return ""
    end = src.find("function WritingMetric(", start)
    return src[start:end if end > 0 else len(src)]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    strip = (SRC / "shell" / "SceneStatusStrip.tsx").read_text(encoding="utf-8")
    timeline = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")
    scene = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    art = (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8")
    sheet = (SRC / "shell" / "SourcePeekSheet.tsx").read_text(encoding="utf-8")
    worklog = (RUST / "worklog.rs").read_text(encoding="utf-8")
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    skill = (REPO / "skills" / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")

    band = band_metric_body(strip)
    timeline_jsx = strip_comments(timeline)
    art_jsx = strip_comments(art)

    # --- S01 指标卡：图标独占左列，其下无文字 ---
    check("band_has_icon_column", "data-band-icon-column" in band, "指标卡有独立图标列")
    check("band_icon_column_fixed_width", "w-14" in band and "shrink-0" in band)
    check("band_icon_seat_larger", 'w-14 h-14 rounded-2xl' in band, "图标座 56×56")
    check("band_icon_glyph_larger", 'Icon name={icon} className="w-7 h-7"' in band, "图标本体 28px")
    icon_col = band[band.find("data-band-icon-column") : band.find("data-band-icon-column") + 700]
    check(
        "band_icon_column_no_text",
        "{label}" not in icon_col and "{value}" not in icon_col and "{index}" not in icon_col,
        "图标列内除图标外不渲染文字",
    )
    check("band_two_column_stretch", "flex items-stretch gap-3.5" in band, "左右两列等高对齐")
    check("band_content_column_after_icon", band.find("data-band-icon-column") < band.find("flex-1 min-w-0 flex flex-col"))
    check(
        "band_writing_metric_untouched",
        "data-band-icon-column" not in strip[strip.find("function WritingMetric(") :],
        "写作页指标卡未套用新结构",
    )

    # --- S02 去向点选：方形开关 + 宿主受控单行改写 ---
    check("dest_role_switch", 'role="switch"' in timeline_jsx and "aria-checked={checked}" in timeline_jsx)
    check("dest_entry_index_attr", "data-entry-index={index}" in timeline_jsx, "开关带原始条目下标")
    check("dest_keeps_source_index", "entryIndexes" in timeline_jsx and "sourceIndex" in timeline_jsx,
          "筛选时用原始下标，不错位")
    check("dest_records_only_value", "仅记录" in timeline_jsx and "仅记录" in scene)
    check("dest_missing_is_static", "无去向" in timeline_jsx and "border-dashed" in timeline_jsx)
    check("dest_square_button", "rounded-none" in timeline_jsx and "rounded-t-2xl" not in timeline_jsx)
    check("dest_failure_inline", "failure" in timeline_jsx and "text-error" in timeline_jsx)
    check("dest_keyboard_ready", "focus-visible:ring-2" in timeline_jsx, "键盘焦点可见")
    check("scene_toggle_wired", "onToggleDestination={toggleDestination}" in scene)
    check("scene_reloads_week", "reloadWeek" in scene, "成功后重读当周文件，统计随之更新")

    check("rust_set_destination", "pub fn set_destination" in worklog)
    check("rust_destination_receipt", "pub struct DestinationReceipt" in worklog)
    check("rust_backup_suffix", ".md.bak-" in worklog, "写前备份")
    check("rust_only_record_destination", '"仅记录"' in worklog, "新增仅记录取值")
    check("rust_expect_text_guard", "expect_text" in worklog and "正文" in worklog, "按正文校验目标条目")
    check("rust_atomic_replace", "sync_all" in worklog and "rename" in worklog)
    check("rust_command_registered", "worklog_set_destination" in lib and "worklog_set_destination," in lib)
    check("api_set_destination", "worklogSetDestination" in api and '"worklog_set_destination"' in api)
    check("types_destination_receipt", "WorklogDestinationReceipt" in types)
    check("skill_documents_new_value", "仅记录" in skill)

    # --- S04 成果卡：序号块居中放大、深一档底色、标题单行 ---
    check("artifact_has_number_block", art_jsx.count('data-artifact-number=""') == 2, "卡片与归档两条分支都有序号块")
    check("artifact_number_size", "w-12 h-12" in art and "text-[22px]" in art, "48×48、22px")
    check("artifact_number_square", "rounded-[8px]" in art and "items-center justify-center" in art)
    check("artifact_number_backing", "bg-surface-container-highest/80" in art, "序号区底色深于正文区")
    check("artifact_title_single_line", 'text-on-surface truncate' in art and "line-clamp-2" not in art_jsx)
    check("artifact_title_tooltip", "title={m ? m.title ?? r.file_name : undefined}" in art or
          "title={m.title ?? r.file_name}" in art, "完整标题仍在悬浮提示")

    # --- S04 预览浮层：居中、方角、四边边框、独立滚动 ---
    check("sheet_centered", "items-center" in sheet and "items-end" not in sheet)
    check("sheet_max_height", "max-h-[80vh]" in sheet and "overflow-y-auto" in sheet)
    check("sheet_four_sides_bordered", "border border-outline-variant/60" in sheet and "border-b-0" not in sheet)
    check("sheet_keeps_close_paths", 'event.key === "Escape"' in sheet and "returnFocusRef" in sheet)

    # --- 护栏：本轮不改的东西 ---
    check("no_report_write", "写周报" not in worklog, "宿主不写周报正文")
    check("quadrant_rules_unchanged", "past-report" in (SRC / "shell" / "WeeklyQuadrants.tsx").read_text(encoding="utf-8"))

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-099",
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
