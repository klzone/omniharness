#!/usr/bin/env python3
"""OD-ARCH-093：工作记录页按参考图重排（四栏指标带 + 三栏）。

2026-10-27 用户二次反馈后的修订也在这里断言：历史归档并入成果文档库（带「归档」徽标），
不再单独占一张卡；栏间距与卡片内边距放大；标题/正文/元信息三级字号。

证据等级：
  * 静态代码检查：四栏指标带、三栏 grid、四象限、两行截断、成果编号卡、副标题可缺省；
  * 脚本运行：编译真实的 `weekTimeline.ts` + `weeklyQuadrants.ts`，用合成周记录断言
    「四格计数 + 待归类 = 本周记录条数」，并逐条核对归类口径。

**不**调用模型、不开窗口。窗口级预览另走 `preview_weekly_093.py`（Xvfb 截图）。

用法：python3 tests/verification/desktop/verify_weekly_093.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"
EVID = HERE / "evidence" / "od-arch-093"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


def compile_ts(files: list[Path], out_dir: Path) -> subprocess.CompletedProcess[str]:
    return run(
        [
            "npx", "--no-install", "tsc", *[str(f) for f in files],
            "--outDir", str(out_dir), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )


PROBE = r"""
const tl = require(process.argv[2]);
const q = require(process.argv[3]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

const NL = String.fromCharCode(10);
// 宿主 worklog.rs::render_block 的真实写法：7 条，跨 4 天，含 1 条没有去向的旧式记录。
const WEEK = [
  "# 2026-W44", "",
  "## 2026-10-26", "",
  "- **事**：已过日期且进周报", "  **去向**：本周报引用", "",
  "- **事**：已过日期、去向归档", "  **去向**：归档", "",
  "- **事**：已过日期、没有去向", "",
  "## 2026-10-27", "",
  "- **事**：今天且进周报", "  **去向**：本周报引用", "",
  "## 2026-10-28", "",
  "- **事**：未来且进周报", "  **去向**：本周报引用", "",
  "- **事**：未来、去向知识卡", "  **去向**：知识卡", "",
  "- **事**：未来、去向 inbox", "  **去向**：inbox", "",
].join(NL);

const parsed = tl.parseWorklog(WEEK);
const today = "2026-10-27";
const c = q.classifyWorklog(parsed, today);

check("entry_count_seven", parsed.entryCount === 7, parsed.entryCount);
check("past_report_is_one", c.counts["past-report"] === 1, c.counts["past-report"]);
check("past_other_is_one", c.counts["past-other"] === 1, c.counts["past-other"]);
// 今天不算已过：今天 1 条 + 未来 1 条进周报，共 2 条落「排期」。
check("future_report_is_two", c.counts["future-report"] === 2, c.counts["future-report"]);
check("future_other_is_two", c.counts["future-other"] === 2, c.counts["future-other"]);
check("unclassified_is_one", c.unclassified === 1, c.unclassified);

const total = c.counts["past-report"] + c.counts["future-report"] +
  c.counts["past-other"] + c.counts["future-other"] + c.unclassified;
check("quadrant_sum_equals_entries", total === parsed.entryCount,
  String(total) + " vs " + String(parsed.entryCount));

const ids = new Set(c.items.map((i) => i.id));
check("ids_unique", ids.size === c.items.length, String(c.items.length));
check("ids_match_timeline_shape", c.items.every((i) => /^\d{4}-\d{2}-\d{2}#\d+$/.test(i.id)),
  c.items.length ? c.items[0].id : "none");

// 空周：四格全 0、待归类 0，不报错、不出现负数。
const empty = q.classifyWorklog(tl.parseWorklog("# 2026-W44" + NL), today);
check("empty_week_all_zero",
  empty.items.length === 0 && empty.unclassified === 0 &&
  Object.values(empty.counts).every((v) => v === 0), "empty");

// 无法判定的日期不能被当成「已过期」：日期串不合法按今天及以后处理。
const odd = tl.parseWorklog("# 2026-W44" + NL + NL + "## 2026-13-99" + NL + NL +
  "- **事**：形状不合法的日期" + NL + "  **去向**：本周报引用" + NL);
const oddOut = q.classifyWorklog(odd, today);
check("invalid_date_not_overdue", oddOut.counts["future-report"] === 1, oddOut.counts["future-report"]);

// 筛选集合与格子内容一致；取消筛选返回 null。
const only = q.visibleIdsFor(c, "past-report");
check("visible_ids_match_cell", only instanceof Set && only.size === c.counts["past-report"], only && only.size);
check("visible_ids_null_when_cleared", q.visibleIdsFor(c, null) === null, "null");
check("items_in_matches_count", q.itemsIn(c, "future-report").length === c.counts["future-report"], "");

// 四格标题必须是中文，序号固定 1~4。
check("quadrant_titles_chinese",
  q.QUADRANTS.map((x) => x.title).join("/") === "先做/排期/回看/可缓",
  q.QUADRANTS.map((x) => x.title).join("/"));
check("quadrant_indexes", q.QUADRANTS.map((x) => x.index).join("") === "1234", "");
check("report_destination_literal", q.REPORT_DESTINATION === "本周报引用", q.REPORT_DESTINATION);

process.stdout.write(JSON.stringify({ cases }) + "\n");
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    scene_tsx = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    strip_tsx = (SRC / "shell" / "SceneStatusStrip.tsx").read_text(encoding="utf-8")
    nav_tsx = (SRC / "shell" / "TopNav.tsx").read_text(encoding="utf-8")
    tl_tsx = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")
    quad_tsx = (SRC / "shell" / "WeeklyQuadrants.tsx").read_text(encoding="utf-8")
    art_tsx = (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8")
    panel_tsx = (SRC / "shell" / "WeeklyReportPanel.tsx").read_text(encoding="utf-8")
    scenes_ts = (SRC / "scenes.ts").read_text(encoding="utf-8")

    # ---- S01 四栏指标带 ----
    weekly_branch = strip_tsx.split('scene.key === "weekly" ? (')[1].split("</>")[0]
    check("band_four_metrics", weekly_branch.count("<BandMetric") == 4,
          str(weekly_branch.count("<BandMetric")))
    for icon in ('icon="note"', 'icon="checklist"', 'icon="clock"', 'icon="trend-up"'):
        check("band_icon" + icon.split('"')[1], icon in weekly_branch, "")
    check("band_has_subtitle_slot", "subtitle" in strip_tsx and "subtitle ?" in strip_tsx,
          "optional english subtitle")
    check("band_ratio_hidden_without_data",
          "width === null ? null :" in strip_tsx and "ratio={reportShare}" in strip_tsx,
          "no progress bar without numerator/denominator")
    check("band_unknown_is_dash", 'weekEntries === null ? "—"' in weekly_branch, "")
    check("band_cards_are_separate",
          "grid grid-cols-1 items-stretch" in strip_tsx
          and "sm:grid-cols-2" in strip_tsx and "xl:grid-cols-4" in strip_tsx,
          "四张独立卡片：手机单列、中窗两列、宽窗四列")
    check("band_narrower_than_nav",
          "max-w-6xl mx-auto" in strip_tsx and "max-w-7xl" in nav_tsx,
          "整排 1152px，比导航 1280px 略窄")
    check("band_gap_between_cards", "gap-3 sm:grid-cols-2 sm:gap-4" in strip_tsx, "卡与卡之间留白")
    check("band_gap_under_nav", "mt-4 mb-4 max-w-6xl" in strip_tsx, "与顶部导航留空")
    check("band_centered_with_nav", "md:-translate-x-8" in strip_tsx,
          "主区左内边距比导航外壳多 32px，位移后与导航同轴")
    check("band_card_is_horizontal",
          "relative flex items-start justify-between gap-3 min-w-0" in strip_tsx
          and "relative mt-auto pt-2 flex items-center" in strip_tsx,
          "横向长方形：左侧图标标题、右侧数字，底行说明")
    check("band_card_compact_height",
          "rounded-2xl border px-4 py-3 " in strip_tsx
          and "relative mt-1.5 h-[4px]" in strip_tsx,
          "内边距与柔光斑收紧，卡高约 100px")
    check("board_matches_nav_width",
          'max-w-7xl mx-auto md:-translate-x-8' in scene_tsx
          and 'data-board-width="nav"' in scene_tsx,
          "三栏与顶部导航同宽并同轴")
    check("artifact_toolbar_compact",
          "compactToolbar" in art_tsx and "<ArtifactExplorer" in scene_tsx
          and scene_tsx.count("compactToolbar") >= 1,
          "工作记录页用精简工具栏")
    check("artifact_range_hidden_in_compact",
          "compactToolbar ? null : (" in art_tsx,
          "精简模式隐藏时间范围")
    check("artifact_fixed_recent_month",
          "30 * 24 * 60 * 60" in art_tsx, "固定近 30 天")
    check("artifact_subdirs_toggle_restored",
          'checked={includeSubdirs}' in art_tsx and 'onToggleSubdirs(e.target.checked)' in art_tsx
          and 'readSubdirsPref(scene, scene === "weekly")' in scene_tsx,
          "工作记录页恢复含子目录开关且默认打开")
    check("artifact_search_kept_in_compact",
          'placeholder="搜索成果或归档"' in art_tsx, "搜索框保留")
    check("artifact_compact_window_not_stated",
          "本场景成果共" not in art_tsx and "标准 Markdown，可直接在 Obsidian" not in art_tsx,
          "移除右栏说明性计数与 Obsidian 提示")
    check("band_no_vertical_dividers",
          "w-px shrink-0 self-stretch bg-surface-variant/70" not in weekly_branch,
          "独立卡片不再用竖分隔线")
    check("band_four_distinct_tones",
          all(t in strip_tsx for t in ("violet: {", "cyan: {", "indigo: {", "amber: {"))
          and all(f'color="{c}"' in weekly_branch for c in ("violet", "cyan", "indigo", "amber")),
          "四张卡各一色")
    check("band_index_numbers",
          all(f'index="0{i}"' in weekly_branch for i in (1, 2, 3, 4)),
          "卡序号 01–04")
    check("other_scenes_unchanged_metrics",
          'scene.key === "project" ? "本月阶段成果" : "本周产出"' in strip_tsx, "")

    # ---- S02 四象限 ----
    check("quadrant_component_exists", quad_tsx.count("itemsIn(classified, q.key)") == 1, "")
    for title in ("先做", "排期", "回看", "可缓"):
        check("quadrant_title_" + title, f'title: "{title}"' in (SRC / "weeklyQuadrants.ts").read_text(encoding="utf-8"), "")
    check("quadrant_rule_removed_from_screen", "口径：" not in quad_tsx and "不调用模型" not in quad_tsx, "")
    check("quadrant_preview_single_line",
          "truncate opacity-85" in quad_tsx and "title={i.text}" in quad_tsx,
          "单行截断，悬浮可读全文")
    check("quadrant_click_filters", "onSelect(q.key)" in quad_tsx and "aria-pressed" in quad_tsx, "")
    check("quadrant_state_in_page",
          "useState<QuadrantKey | null>(null)" in scene_tsx and
          "setQuadrant((prev) => (prev === key ? null : key))" in scene_tsx,
          "再点取消")
    check("quadrant_reset_on_scene_change", "setQuadrant(null);" in scene_tsx, "")

    # ---- S03 三栏 + 历史对话归位 + 归档并入成果库 ----
    check("three_column_grid",
          "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)]" in scene_tsx, "")
    check("grid_areas_declared", "grid-template-areas" in scene_tsx, "wide window only")
    check("narrow_single_column", "grid grid-cols-1 gap-4" in scene_tsx, "")
    check("quadrant_in_left_column",
          scene_tsx.index("<WeeklyQuadrants") < scene_tsx.index("<WeeklyTimeline"), "quadrant first")
    check("threads_inside_board", "lg:[grid-area:left]" in scene_tsx and "max-lg:order-4" in scene_tsx,
          "history directly under the quadrants in the left column")
    check("threads_not_duplicated",
          scene_tsx.count("{threadsSection}") == 2, "once in weekly board, once for other scenes")
    check("threads_immediately_after_quadrants",
          scene_tsx.index("<WeeklyQuadrants") < scene_tsx.index("{threadsSection}"),
          "quadrants -> history in the left column")
    check("standalone_archive_card_removed",
          "<WeeklyArchive" not in scene_tsx and "WeeklyArchive" not in panel_tsx,
          "archive is not a separate card any more")
    check("narrow_order_is_quadrant_log_artifacts_history",
          all(t in scene_tsx for t in ("max-lg:order-1", "max-lg:order-2", "max-lg:order-3", "max-lg:order-4")),
          "single column: quadrants -> log -> artifacts -> history")
    check("columns_scroll_independently",
          "lg:max-h-[70vh] lg:overflow-y-auto" in scene_tsx
          and "flex-1 min-h-0 overflow-y-auto" in tl_tsx,
          "left column and timeline scroll on their own")
    check("timeline_no_nested_scroll_when_narrow",
          "max-h-[70vh] max-lg:max-h-none" in tl_tsx,
          "single column page does not nest a scrolling timeline")
    check("history_heading_chinese", "历史对话" in scene_tsx, "")
    check("threads_actions_kept", "去对话 · 新开" in scene_tsx and "scene-category-picker" in (SRC / "shell" / "CategoryPicker.tsx").read_text(encoding="utf-8"), "")
    check("history_category_picker_styled", "<CategoryPicker" in scene_tsx and "scene-category-picker" in (SRC / "shell" / "CategoryPicker.tsx").read_text(encoding="utf-8"), "")

    # ---- S04 中栏两行截断 + 右栏编号卡 ----
    check("entry_line_clamp_two",
          "line-clamp-2" in tl_tsx and "title={entry.text}" in tl_tsx, "全文仍可读")
    check("entry_card_style", "data-weekly-entry" in tl_tsx and "border-l-2" in tl_tsx, "")
    check("timeline_keeps_rail", "absolute left-[51px]" in tl_tsx, "")
    check("artifact_numbered_card",
          'padStart(2, "0")' in art_tsx and "data-artifact-card" in art_tsx, "")
    check("artifact_filters_kept", "按文件名搜索成果" in art_tsx and "含子目录" in art_tsx, "")
    check("artifact_list_view_kept", 'view === "list"' in art_tsx, "")

    # ---- S06 归档并入成果库（用户 2026-10-27 二次反馈）----
    check("archive_entries_channel",
          "archiveEntries" in art_tsx and "ArtifactArchiveEntry" in art_tsx,
          "成果库接收归档条目")
    check("archive_badge_rendered",
          'data-artifact-archive=""' in art_tsx and "归档" in art_tsx,
          "归档项带徽标")
    check("archive_searchable",
          "r.archive.kindLabel" in art_tsx and ".toLowerCase().includes(q)" in art_tsx,
          "搜索覆盖周报/周记录")
    check("archive_open_kept",
          "r.archive.onOpen" in art_tsx and "onOpenReport" in scene_tsx and "openWeekSource" in scene_tsx,
          "归档仍能打开周报与周记录")
    check("archive_merged_from_host_list",
          "archive.reports" in scene_tsx and "archive.weeks" in scene_tsx and
          'kindLabel: "周报"' in scene_tsx and 'kindLabel: "周记录"' in scene_tsx,
          "周报与周记录都进同一列表")
    check("archive_error_not_silent",
          "archiveError" in scene_tsx and "archiveError" in art_tsx, "读取失败如实显示")

    # ---- S08 展示层清理与可读性（OD-ARCH-095） ----
    check("pending_write_modal",
          'role="dialog"' in strip_tsx and 'w-[80vw]' in strip_tsx and 'aria-modal="true"' in strip_tsx,
          "待确认写入改为页面内 80% 宽弹窗")
    check("artifact_toolbar_two_rows",
          "placeholder=\"搜索成果或归档\"" in art_tsx and "items-center justify-end gap-1.5 pt-2" in art_tsx
          and "name=\"search\"" in art_tsx,
          "搜索行与视图/目录控制行分开")
    check("weekly_small_text_floor",
          "text-[12px]" in strip_tsx and "text-[12px]" in quad_tsx and "text-[12px]" in tl_tsx,
          "周报页主要小字至少 12px")

    # ---- S10 待确认并入逾期卡 / 方形底 / 分类浮层 / 删除位置（2026-10-27 第五次反馈）----
    weekly_band = strip_tsx.split('scene.key === "weekly" ?')[1].split('scene.key === "writing" ?')[0]
    check("pending_merged_into_overdue_card",
          "待确认 {waiting}" in weekly_band and "label=\"逾期\"" in weekly_band,
          "待确认入口是第四张「逾期」卡内的动作，不再另起模块")
    check("pending_trigger_bar_removed",
          "条等待决定" not in strip_tsx,
          "四卡下方的独立触发条已删除")
    pending_card = (SRC / "shell" / "PendingWriteCard.tsx").read_text(encoding="utf-8")
    check("pending_card_square_backdrop",
          "rounded-xl" not in pending_card and "rounded-md" not in pending_card,
          "弹窗条目底色与按钮改为方形")
    cat_tsx = (SRC / "shell" / "CategoryPicker.tsx").read_text(encoding="utf-8")
    check("category_menu_ported_to_body",
          "createPortal" in cat_tsx and "document.body" in cat_tsx and "fixed" in cat_tsx,
          "分类下拉挂到 body，不受滚动容器裁剪")
    check("category_menu_flips_to_fit_viewport",
          "window.innerHeight" in cat_tsx and "menuRef.current" in cat_tsx,
          "按真实高度上下翻转并夹在视口内")
    check("thread_delete_before_title",
          'aria-label={`删除对话：${t.title}`}' in scene_tsx
          and scene_tsx.index('aria-label={`删除对话：${t.title}`}') < scene_tsx.index("onClick={() => openThread(t.id)}"),
          "删除按钮排在单条对话标题之前")

    # ---- S07 留白与字号（用户二次反馈）----
    check("wider_column_gaps", "gap-4 lg:gap-5" in scene_tsx, "栏间距放大")
    check("roomier_card_padding", "px-4 py-4 lg:px-5 lg:py-5" in scene_tsx, "卡片内边距放大")
    check("band_larger_digits", 'text-[30px] leading-none' in strip_tsx, "数字卡 30px")
    check("band_long_value_shrinks",
          "value.length >= 4" in strip_tsx and 'text-[20px] leading-[1.05]' in strip_tsx,
          "长状态值自动降字号，不挤压标题与副标题")
    check("band_value_no_wrap", "tabular-nums whitespace-nowrap" in strip_tsx,
          "数值区不折行")
    check("band_larger_icon", "w-9 h-9 rounded-xl" in strip_tsx, "图标座 36px")
    check("section_title_hierarchy",
          all("text-[16px] font-semibold tracking-tight" in t for t in (quad_tsx, tl_tsx, scene_tsx)),
          "区块标题三级层级中的一级")
    check("subtitle_tracking",
          "tracking-[0.16em]" in quad_tsx and "tracking-[0.16em]" in tl_tsx,
          "英文副标题拉开字距")
    check("quadrant_roomier", "min-h-[128px]" in quad_tsx and "gap-2.5 pt-3.5" in quad_tsx, "四象限留白")
    check("timeline_roomier", "gap-2.5" in tl_tsx and "leading-[1.7]" in tl_tsx, "条目行距")

    # ---- 英文副标题可配置、缺省不渲染 ----
    check("subtitle_configurable", "subtitles?: Partial<" in scenes_ts, "scene config field")
    check("subtitle_optional_render",
          all(f"{name} ? (" in text for name, text in (
              ("subtitle", tl_tsx),
              ("def.subtitles?.artifacts", scene_tsx),
              ("def.subtitles?.history", scene_tsx),
          )) and "{subtitle ? (" in strip_tsx, "empty subtitle leaves no row")
    check("band_subtitles_from_scene_config",
          all(f"subtitle={{scene.subtitles?.{k}}}" in strip_tsx
              for k in ("records", "weekly", "report", "overdue")),
          "four band subtitles are configurable, not hard-coded")
    check("band_metric_ax_group",
          'role="group"' in strip_tsx and 'aria-label={`${label}' in strip_tsx,
          "WebKit 不单列四栏段落，用 group + aria-label 合成可读节点")
    check("timeline_ax_count_label",
          "aria-label=" in tl_tsx and '"本周记录 " + week' in tl_tsx,
          "卡片头用 aria-label 暴露可见/总数")
    check("subtitle_defaults",
          all(s in scenes_ts for s in ("WEEKLY LOG", "ARTIFACTS", "RECORDS", "OVERDUE")), "")

    # ---- 编译并执行真实归类模块 ----
    tmp = Path(tempfile.mkdtemp(prefix="od093-"))
    compiled = compile_ts([SRC / "weekTimeline.ts", SRC / "weeklyQuadrants.ts"], tmp / "q")
    if compiled.returncode != 0:
        check("quadrant_module_compiles", False, compiled.stderr[-1500:])
    else:
        check("quadrant_module_compiles", True, "")
        probe = tmp / "probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(
            ["node", str(probe), str(tmp / "q" / "weekTimeline.js"), str(tmp / "q" / "weeklyQuadrants.js")],
            DESKTOP,
        )
        if proc.returncode != 0:
            check("quadrant_probe_exit", False, (proc.stderr or proc.stdout)[-1500:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WEEKLY-093] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases)-len(failed)} failed={len(failed)}")

    EVID.mkdir(parents=True, exist_ok=True)
    out = Path(args.json) if args.json else EVID / "verify_weekly_093.json"
    out.write_text(json.dumps({"cases": cases, "failed": len(failed)}, ensure_ascii=False, indent=2),
                   encoding="utf-8")
    print(f"evidence -> {out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
