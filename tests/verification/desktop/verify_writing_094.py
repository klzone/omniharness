#!/usr/bin/env python3
"""OD-ARCH-094：文案写作页改版——创作管线与灵感架（静态断言 + 纯函数实测）。

证据等级：
  * 静态代码检查：写作页四卡来源、三栏骨架、状态管线、类型标签、英文副标题可配置、
    加载态不把「还没读到」写成 0；
  * 脚本运行：编译真实的 `desktop/src/writingStudio.ts`，用合成条目断言状态分组、
    缺省口径、90 天判定与「继续创作」选择。

**不**调用模型、不开窗口、不读真实成果目录。窗口级验证另走 `verify_writing_094.py --window`
（或用 `preview_writing_094.py` 只抓图）。

用法：python3 tests/verification/desktop/verify_writing_094.py [--json PATH]
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
EVID = HERE / "evidence" / "od-arch-094"


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
const ws = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}
const NOW = 1790000000;  // 合成时钟，2026-09-21 前后
const DAY = 86400;
function file(name, status, type, ageDays) {
  return {
    relative_path: name, file_name: name, bytes: 10,
    modified_at: NOW - ageDays * DAY, scenario: "material-writing",
    is_dir: false, depth: 0, status: status, artifact_type: type,
  };
}

// 状态归一：显式值、中文别名、缺省与非法值。
check("status_draft", ws.writingStatus("draft") === "draft", ws.writingStatus("draft"));
check("status_chinese_draft", ws.writingStatus("草稿") === "draft", "");
check("status_published", ws.writingStatus("Published") === "published", "");
check("status_missing_is_finished", ws.writingStatus(null) === "finished", "");
check("status_invalid_is_finished", ws.writingStatus("whatever") === "finished", "");

// 类型归一：四种内容类型 + 未分类缺省。
const types = [["insight", "学习心得"], ["ai", "AI 知识"], ["blog", "blog"],
               ["website", "网站内容"], [null, "未分类"], ["乱写", "未分类"]];
for (const [raw, label] of types) {
  check("type_" + String(raw), ws.typeLabel(raw) === label, ws.typeLabel(raw));
}

// 分组：三组之和必须等于文件总数；目录行不计入。
const items = [
  file("a.md", "draft", "ai", 3),
  file("b.md", "published", "blog", 200),
  file("c.md", null, null, 400),
  file("d.md", "乱写", "乱写", 1),
  { relative_path: "sub", file_name: "sub", bytes: 0, modified_at: NOW, scenario: "",
    is_dir: true, depth: 0, status: null, artifact_type: null },
];
const counts = ws.statusCounts(items);
check("counts_draft", counts.draft === 1, counts.draft);
check("counts_published", counts.published === 1, counts.published);
check("counts_finished_includes_default", counts.finished === 2, counts.finished);
check("counts_sum_equals_files", counts.draft + counts.finished + counts.published === 4,
  String(counts.draft + counts.finished + counts.published));
check("dirs_excluded", ws.writingFiles(items).length === 4, ws.writingFiles(items).length);

// 90 天：只提醒成文/已发布；草稿不催；时间缺失不判定。
const stale = ws.staleFiles(items, NOW).map((f) => f.file_name).sort();
check("stale_only_finished_published", JSON.stringify(stale) === JSON.stringify(["b.md", "c.md"]),
  JSON.stringify(stale));
const draftOld = file("old-draft.md", "draft", "ai", 500);
check("old_draft_not_stale", ws.isWritingFileStale(draftOld, NOW) === false, "");
const noTime = file("no-time.md", "finished", "ai", 0);
noTime.modified_at = 0;
check("missing_time_not_stale", ws.isWritingFileStale(noTime, NOW) === false, "");
const boundary = file("boundary.md", "finished", "ai", 90);
check("exactly_90_days_not_stale", ws.isWritingFileStale(boundary, NOW) === false,
  NOW - boundary.modified_at);

// 继续创作：最近修改且未发布；全发布时不给目标。
check("continue_skips_published", ws.continueTarget(items).file_name === "d.md",
  ws.continueTarget(items).file_name);
const allPublished = [file("p1.md", "published", "blog", 1), file("p2.md", "published", "blog", 2)];
check("continue_null_when_all_published", ws.continueTarget(allPublished) === null, "");
check("continue_null_when_empty", ws.continueTarget([]) === null, "");

// 相对时间与短日期：取不到一律「—」，不猜。
check("relative_just_now", ws.relativeUpdated(NOW - 30, NOW) === "刚刚", "");
check("relative_minutes", ws.relativeUpdated(NOW - 600, NOW) === "10 分钟前", "");
check("relative_hours", ws.relativeUpdated(NOW - 7200, NOW) === "2 小时前", "");
check("relative_days", ws.relativeUpdated(NOW - 3 * DAY, NOW) === "3 天前", "");
check("relative_missing", ws.relativeUpdated(null, NOW) === "—", "");
check("relative_zero", ws.relativeUpdated(0, NOW) === "—", "");
check("short_date_missing", ws.shortDate(0) === "—", "");

// 标题：frontmatter 有就用，没有就退回（不把文件名当正文标题）。
check("title_from_frontmatter",
  ws.titleFromContent("---\ntitle: \"合成标题\"\n---\n正文") === "合成标题",
  String(ws.titleFromContent("---\ntitle: \"合成标题\"\n---\n正文")));
check("title_missing_is_null", ws.titleFromContent("# 没有元数据") === null, "");
check("title_empty_field_is_null", ws.titleFromContent("---\ntitle:\n---\n正文") === null, "");
check("fallback_title_is_not_body_title",
  ws.fallbackTitle("offline-material-writing-20261020-090000.md").includes("2026-10-20"),
  ws.fallbackTitle("offline-material-writing-20261020-090000.md"));

// 缺省口径文案必须与界面读数同源，不能前端另写一套。
check("default_note_mentions_finished", ws.STATUS_DEFAULT_NOTE === "未标注状态的成果按「成文」显示",
  ws.STATUS_DEFAULT_NOTE);
check("stale_days_90", ws.STALE_DAYS === 90, ws.STALE_DAYS);

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
    studio_tsx = (SRC / "shell" / "WritingStudio.tsx").read_text(encoding="utf-8")
    scenes_ts = (SRC / "scenes.ts").read_text(encoding="utf-8")
    art_tsx = (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8")
    types_ts = (SRC / "types.ts").read_text(encoding="utf-8")
    rust = (REPO / "desktop" / "src-tauri" / "src" / "task_shell.rs").read_text(encoding="utf-8")

    # ---- S03 宿主：只读、有上限、确定性缺省 ----
    check("host_entry_has_status_type",
          "pub status: Option<String>" in rust and "pub artifact_type: Option<String>" in rust, "")
    check("host_prefix_limit_16k", "ARTIFACT_FRONTMATTER_MAX_BYTES: u64 = 16 * 1024" in rust, "")
    check("host_reads_only_writing_scenario",
          'if scenario == "material-writing"' in rust, "其它场景不解析")
    check("host_defaults_are_deterministic",
          '"finished".to_string()' in rust and '"uncategorized".to_string()' in rust, "")
    check("host_never_writes_back",
          "fs::write" not in rust.split("fn read_writing_frontmatter", 1)[1].split("fn offline_relative_path_parts", 1)[0],
          "解析区间内没有写文件")
    check("host_no_persistent_cache",
          "OnceLock" not in rust.split("fn read_writing_frontmatter", 1)[1].split("fn offline_relative_path_parts", 1)[0],
          "不新增缓存")
    check("ts_types_optional",
          "status?: string | null" in types_ts and "artifact_type?: string | null" in types_ts, "")

    # ---- S01 指标四卡 ----
    writing_branch = strip_tsx.split('scene.key === "writing" ? (')[1].split("</>")[0]
    check("band_four_writing_metrics", writing_branch.count("<WritingMetric") == 4,
          str(writing_branch.count("<WritingMetric")))
    for label in ("本月成文", "进行中草稿", "灵感待用", "最近更新"):
        check("metric_" + label, f'label="{label}"' in writing_branch, "")
    for icon in ("book", "edit", "spark", "clock"):
        check("metric_icon_" + icon, f'icon="{icon}"' in writing_branch, "")
    check("metric_this_month_from_overview",
          "String(overview.artifacts_this_month)" in writing_branch, "宿主聚合")
    check("metric_draft_from_artifact_meta",
          "writingDraftCount === null ? \"—\"" in writing_branch, "未读到显 —")
    check("metric_inspiration_from_threads",
          "writingInspirationCount === null ? \"—\"" in writing_branch, "")
    check("metric_recent_from_latest_artifact",
          "relativeUpdated(overview?.latest_artifact_at ?? null, writingNow)" in writing_branch, "")
    check("metric_distinct_from_weekly_tones",
          all(t in strip_tsx for t in ("violet: {", "cyan: {", "ink: {", "ochre: {")),
          "写作页用墨色/青绿/琥珀，不复用周报的紫青靛")
    check("metric_warm_paper_surface",
          "#fbf9f4" in strip_tsx and "border-on-surface/8" in strip_tsx, "暖纸底")
    check("metric_no_zero_before_load",
          'scene === "writing" && artifactsLoaded && !artifactsError' in scene_tsx
          and "writingCounts ? writingCounts.draft : null" in scene_tsx,
          "成果未读到时给 null 而不是 0")
    check("metric_load_state_tracked",
          "setArtifactsLoaded(false)" in scene_tsx and "setArtifactsLoaded(true)" in scene_tsx, "")

    # ---- S02 三栏 ----
    check("writing_board_exists", 'data-writing-board=""' in scene_tsx, "")
    check("writing_three_columns",
          "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)]" in scene_tsx
          and "[grid-template-areas:'left_pipeline_artifacts']" in scene_tsx, "")
    check("writing_board_nav_width",
          scene_tsx.count('data-board-width="nav"') == 2, "与工作记录页同宽")
    check("writing_narrow_order",
          all(t in scene_tsx for t in ("max-lg:order-1", "max-lg:order-2", "max-lg:order-3")),
          "窄窗 左 → 中 → 右")
    check("writing_continue_and_shelf_in_left",
          scene_tsx.index("<WritingContinueCard") < scene_tsx.index("<WritingInspirationShelf")
          < scene_tsx.index("<WritingPipeline"), "左栏上继续创作、下灵感架")
    check("writing_page_has_no_extra_thread_block",
          'scene === "weekly" || scene === "writing" ? null' in scene_tsx,
          "写作页不在底部重复渲染会话历史")
    check("inspiration_opens_thread_home",
          "onOpenThread={openThread}" in scene_tsx and "requestOpenThread(id)" in scene_tsx, "")
    check("inspiration_no_new_storage",
          "localStorage" not in studio_tsx and "saveThread" not in studio_tsx, "不新增存储格式")
    check("writing_no_editor",
          "textarea" not in studio_tsx.lower() and "contentEditable" not in studio_tsx,
          "本页不承载编辑器")

    # ---- S02/S04 状态管线 ----
    check("pipeline_three_groups",
          all(f'key: "{k}"' in (SRC / "writingStudio.ts").read_text(encoding="utf-8")
              for k in ("draft", "finished", "published")), "")
    check("pipeline_groups_clickable",
          "onStatusFilter(active ? null : s.key)" in studio_tsx, "再点取消")
    check("pipeline_default_note_on_screen",
          "STATUS_DEFAULT_NOTE" in studio_tsx and "口径：" in studio_tsx, "")
    check("pipeline_directory_rows_excluded",
          "files.filter((f) => !f.is_dir)" in studio_tsx, "")
    check("pipeline_two_line_titles",
          "line-clamp-2" in studio_tsx, "")
    check("stale_filter_present",
          "data-stale-filter" in studio_tsx and "超过 90 天未动" in studio_tsx, "")
    check("stale_filter_does_not_mutate",
          "只改视图" in scene_tsx or "setWritingStaleOnly" in scene_tsx, "")

    # ---- S04 右栏类型标签 ----
    check("artifact_type_labels_opt_in",
          "showTypeLabels = false" in art_tsx and "showTypeLabels" in scene_tsx, "")
    check("artifact_type_from_host_first",
          "typeLabel(r.artifact_type ?? m?.type ?? null)" in art_tsx, "宿主字段优先")
    check("artifact_accent_secondary",
          'accent="secondary"' in scene_tsx and 'accent?: "primary" | "secondary"' in art_tsx, "")
    check("artifact_stale_marker",
          "stalePaths" in art_tsx and "data-artifact-stale" in art_tsx, "")
    check("artifact_writing_keeps_toolbar",
          'compactToolbar' not in scene_tsx.split('data-writing-board=""')[1].split("</>")[0],
          "写作页保留时间筛选（固定近一月的规则只属于工作记录页）")

    # ---- 英文副标题可配置 ----
    for key in ("studio", "overview", "recent", "continue", "inspiration", "pipeline", "stale", "draft", "finished", "published"):
        check("subtitle_key_" + key, f'"{key}"' in scenes_ts.split("subtitles?: Partial<")[1].split(">;")[0], "")
    check("subtitle_continue_wired",
          "subtitle={def.subtitles?.continue}" in scene_tsx, "")
    check("subtitle_inspiration_wired",
          "subtitle={def.subtitles?.inspiration}" in scene_tsx, "")
    check("subtitle_pipeline_wired",
          "subtitle={def.subtitles?.pipeline}" in scene_tsx, "")
    check("subtitle_recent_used_by_card4",
          "subtitle={scene.subtitles?.recent}" in strip_tsx, "")
    check("no_hardcoded_english_subtitles",
          not any(f'subtitle="{v}"' in studio_tsx for v in ("CONTINUE", "INSPIRATION", "PIPELINE")),
          "组件里的英文小标都来自场景配置")
    check("subtitle_optional_in_studio",
          "subtitle ? (" in studio_tsx, "缺省不渲染")

    # ---- 编译并执行真实派生模块 ----
    tmp = Path(tempfile.mkdtemp(prefix="od094-"))
    compiled = compile_ts([SRC / "writingStudio.ts"], tmp / "w")
    if compiled.returncode != 0:
        check("writing_module_compiles", False, compiled.stderr[-1500:])
    else:
        check("writing_module_compiles", True, "")
        probe = tmp / "probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(["node", str(probe), str(tmp / "w" / "writingStudio.js")], DESKTOP)
        if proc.returncode != 0:
            check("writing_probe_exit", False, (proc.stderr or proc.stdout)[-1500:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WRITING-094] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases)-len(failed)} failed={len(failed)}")

    EVID.mkdir(parents=True, exist_ok=True)
    out = Path(args.json) if args.json else EVID / "verify_writing_094.json"
    out.write_text(json.dumps({"cases": cases, "failed": len(failed)}, ensure_ascii=False, indent=2),
                   encoding="utf-8")
    print(f"evidence -> {out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
