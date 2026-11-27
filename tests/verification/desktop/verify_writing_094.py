#!/usr/bin/env python3
"""OD-ARCH-094：文案写作页改版——稿件与写作对话（静态断言 + 纯函数实测）。

证据等级：
  * 静态代码检查：写作页四卡来源、三栏骨架、稿件分组、类型标签、无英文副标题、
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

    # ---- OD-WD-001 写作台 ----
    desk_tsx = (SRC / "shell" / "WritingDesk.tsx").read_text(encoding="utf-8")
    theme_css = (SRC / "theme.css").read_text(encoding="utf-8")
    workbench = (SRC / "shell" / "WritingWorkbench.tsx").read_text(encoding="utf-8")
    section_editor = (SRC / "shell" / "WritingSectionEditor.tsx").read_text(encoding="utf-8")

    check("writing_desk_component", "export function WritingDesk" in desk_tsx, "")
    check(
        "writing_desk_three_zones",
        all(token in desk_tsx for token in ("wd-page", "wd-body", "wd-rail", "wd-paper")),
        "页头 + 左大纲 + 中纸面 + 右上下文",
    )
    check("writing_desk_paper_entry", 'data-writing-paper=""' in desk_tsx, "")
    check(
        "writing_desk_outline_rail",
        'data-writing-outline-rail=""' in desk_tsx and "本章提示" in desk_tsx,
        "",
    )
    check(
        "writing_desk_context_rail",
        'data-writing-context-rail=""' in desk_tsx and "本章上下文" in desk_tsx,
        "",
    )
    check(
        "writing_desk_library_drawer",
        'data-writing-library=""' in desk_tsx
        and 'data-writing-library-tab={item.key}' in desk_tsx
        and all(f'key: "{tab}", label:' in desk_tsx for tab in ("tasks", "artifacts", "threads")),
        "稿件 / 成果 / 对话三页签",
    )
    check(
        "writing_desk_history_entries",
        "← 所有稿件" in desk_tsx and "所有稿件" in desk_tsx and "对话" in desk_tsx,
        "左上角回稿件库，右上角对话",
    )
    check(
        "writing_desk_reuses_editors",
        all(
            name in desk_tsx
            for name in (
                "WritingOutlineEditor",
                "WritingSectionEditor",
                "WritingFactCheckPanel",
                "WritingVersionPanel",
                "WritingArchivePanel",
                "WritingMaterialBasket",
            )
        ),
        "复用既有编辑器，放进抽屉",
    )
    check(
        "writing_desk_five_stage",
        all(label in desk_tsx for label in ("需求", "大纲", "写作", "核验", "定稿")),
        "",
    )
    check(
        "writing_desk_neu_ui",
        all(
            token in desk_tsx
            for token in (
                "wr-primary-button",
                "wr-ghost-button",
                "wr-icon-button",
                "var(--od-neu-sunken)",
            )
        )
        and all(
            token in theme_css
            for token in ("var(--od-neu-surface-high)", "var(--od-neu-raise-2)")
        ),
        "新拟物按钮与材质",
    )
    check(
        "writing_desk_report_serif",
        "var(--font-report-serif)" in theme_css,
        "纸面正文用报告宋体",
    )
    check(
        "writing_desk_theme_classes",
        all(cls in theme_css for cls in (".wd-page", ".wd-rail", ".wd-paper", ".wd-drawer")),
        "",
    )
    check(
        "scene_uses_writing_desk",
        'import { WritingDesk } from "./WritingDesk"' in scene_tsx and "<WritingDesk" in scene_tsx,
        "",
    )
    check(
        "scene_old_writing_board_removed",
        "<WritingPipeline" not in scene_tsx
        and "<WritingContinueCard" not in scene_tsx
        and "<WritingInspirationShelf" not in scene_tsx,
        "旧三栏管理页退出主画布",
    )
    check(
        "workbench_opens_desk",
        "onOpenTask" in workbench and "onOpenTask(created.task.writing_id)" in workbench,
        "稿件库选中任务后进入写作台",
    )
    check(
        "section_initial_section",
        "initialSectionNo" in section_editor and "initialSectionNo !== undefined" in section_editor,
        "章节抽屉可定位到当前章",
    )
    check(
        "artifact_explorer_in_library",
        "showTypeLabels" in desk_tsx and "stalePaths" in desk_tsx,
        "成果页签复用成果浏览器",
    )
    collect_panel = (SRC / "shell" / "WritingCollectPanel.tsx").read_text(encoding="utf-8")
    lib_rs = (REPO / "desktop" / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    websearch_rs = (REPO / "desktop" / "src-tauri" / "src" / "writing_websearch.rs").read_text(encoding="utf-8")
    check(
        "writing_demand_transition",
        "需求待整理" in desk_tsx
        and 'to: "素材收集中"' in desk_tsx
        and "confirmDemand" in desk_tsx,
        "需求确认后可进入素材收集",
    )
    check(
        "writing_collect_panel",
        "WritingCollectPanel" in desk_tsx
        and "writingCollectMaterials" in collect_panel
        and "writingSelectMaterials" in collect_panel,
        "素材候选可采纳进可信素材包",
    )
    check(
        "writing_collect_websearch",
        "writingWebSearch" in collect_panel
        and "data-writing-websearch-input" in collect_panel
        and "data-writing-websearch-save" in collect_panel
        and "materialCreate" in collect_panel,
        "素材收集内可联网检索并按收藏存入素材篮",
    )
    check(
        "writing_websearch_host_gate",
        'gate_user_text(&request.query, &policy, "素材检索词")' in lib_rs
        and "writing_websearch::web_search(" in lib_rs
        and "locate_cli" in websearch_rs,
        "检索词过宿主闸门，CLI 缺失时如实报错",
    )
    check(
        "writing_next_step",
        "selectedPackCount" in desk_tsx
        and "确认需求，开始收集素材" in desk_tsx
        and "收集素材" in desk_tsx,
        "状态对应下一步动作",
    )
    check(
        "writing_conversation_handoff",
        "onUseForWriting" in desk_tsx
        and "initialRequirement" in workbench
        and "带入写作" in studio_tsx,
        "写作对话可带入新建需求",
    )
    check(
        "writing_requirement_autoparse",
        "autoParsedRef" in workbench
        and "parseRequirement(initialRequirement)" in workbench,
        "从对话带入后自动开始整理需求",
    )
    check(
        "writing_requirement_fallback",
        "localRequirementDraft" in workbench
        and "data-writing-fallback-note" in workbench
        and "自动整理没有完成" in workbench,
        "模型输出不合格时按原文兜底，仍可创建",
    )
    check(
        "writing_create_enters_collect",
        'to: "素材收集中"' in workbench
        and "创建任务时已核对需求" in workbench
        and "创建并开始收集素材" in workbench,
        "创建即确认需求，直接进入素材收集",
    )
    check(
        "writing_compose_surface",
        "const composing" in desk_tsx
        and "composeOnly" in workbench
        and "composeOnly ? null" in workbench,
        "带入写作只显示新建需求，不混在稿件列表里",
    )
    check(
        "writing_next_step_panel",
        ".wd-next" in theme_css
        and "data-writing-next" in desk_tsx
        and "下一步 · 第" in desk_tsx,
        "页面固定显示当前步骤与唯一主行动",
    )
    check(
        "writing_desk_flow",
        "FLOW_STEPS" in desk_tsx
        and ".wd-flow" in theme_css
        and "写作流程" in desk_tsx,
        "六步流程按顺序展示，当前步高亮",
    )
    check(
        "writing_desk_early_focus",
        "is-early" in desk_tsx
        and ".wd-body.is-early" in theme_css
        and "{outline ? (" in desk_tsx,
        "需求与素材阶段收起右栏，聚焦当前一步",
    )
    check(
        "writing_desk_paper_empty",
        "wd-paper-empty" in theme_css
        and "这一步完成后进入" in desk_tsx,
        "纸面空态直接给出当前步骤与后续步骤",
    )
    check("no_subtitle_props_in_studio", "subtitle" not in studio_tsx, "区块标题只用中文")
    check(
        "no_subtitle_props_in_scene",
        "def.subtitles" not in scene_tsx.split('scene === "writing" ? (')[1].split('scene === "weekly" ? (')[0],
        "写作台不接场景副标题",
    )
    check(
        "no_writing_english_keys",
        "studio:" not in scenes_ts.split('writing: {')[1].split("project: {")[0],
        "写作场景配置不再带英文小标",
    )

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
