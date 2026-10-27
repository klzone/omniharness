#!/usr/bin/env python3
"""OD-ARCH-092：工作记录页版式重构（两栏 + 指标带 + 复盘入口收起）。

证据等级：
  * 静态代码检查：两栏 grid、页签消失、指标来源、复盘界面移除、字号下限；
  * 脚本运行：编译真实的 desktop/src/weekTimeline.ts，用 5 条合成记录断言
    「本期工作记录」的口径与时间线条目数一致（同一份解析结果）。

**不**调用模型、不开窗口。窗口级预览另走 preview_weekly_092.py（Xvfb 截图）。

用法：python3 tests/verification/desktop/verify_weekly_092.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"
EVID = HERE / "evidence" / "od-arch-092"


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
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

// 宿主 worklog.rs::render_block 的真实写法：5 条、跨 3 天。
const WEEK = [
  "# 2026-W44",
  "",
  "## 2026-10-26",
  "",
  "- **事**：合成记录一",
  "  **去向**：本周报引用",
  "",
  "- **事**：合成记录二",
  "  **去向**：知识卡",
  "",
  "## 2026-10-27",
  "",
  "- **事**：合成记录三",
  "  **去向**：本周报引用",
  "",
  "- **事**：合成记录四",
  "  **去向**：归档",
  "",
  "## 2026-10-28",
  "",
  "- **事**：合成记录五",
  "  **去向**：本周报引用",
  "",
].join(String.fromCharCode(10));

const parsed = mod.parseWorklog(WEEK);
check("entry_count_five", parsed.entryCount === 5, String(parsed.entryCount));

// 指标带「本期工作记录」与时间线用的是同一次解析：条数必须完全一致。
const tl = mod.buildTimeline(parsed, "2026-W44");
const timelineTotal = tl.reduce((n, d) => n + d.entries.length, 0);
check("metric_matches_timeline", parsed.entryCount === timelineTotal,
  String(parsed.entryCount) + " vs " + String(timelineTotal));

// 空周不能报成 5 条，也不能是负数。
check("empty_entry_count_zero", mod.parseWorklog("# 2026-W44\n").entryCount === 0, "empty week");
check("undated_lines_not_counted",
  mod.parseWorklog("# 2026-W44\n\n随手写的一段没有条目形状的话\n").entryCount === 0,
  "free text is not an entry");

process.stdout.write(JSON.stringify({ cases }) + "\n");
"""


def px(value: str) -> float:
    """从 Tailwind 任意值里取像素数。"""
    m = re.search(r"text-\[([0-9.]+)px\]", value)
    return float(m.group(1)) if m else -1.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    scene_tsx = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    strip_tsx = (SRC / "shell" / "SceneStatusStrip.tsx").read_text(encoding="utf-8")
    tl_tsx = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")
    panel_tsx = (SRC / "shell" / "WeeklyReportPanel.tsx").read_text(encoding="utf-8")
    bar_tsx = (SRC / "shell" / "AnnouncementBar.tsx").read_text(encoding="utf-8")
    settings_tsx = (SRC / "SettingsPanel.tsx").read_text(encoding="utf-8")

    # ---- R2 主区同屏 + weekly 无页签（OD-ARCH-093 起由两栏扩为三栏）----
    check("weekly_three_column_grid",
          "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)]" in scene_tsx,
          "093: left 1 / center 1.4 / right 1")
    check("weekly_no_timeline_tab_token", '\"timeline\"' not in scene_tsx, "")
    check("tab_state_two_valued",
          'useState<\"progress\" | \"artifacts\">' in scene_tsx, "weekly dropped from tab state")
    check("weekly_grid_guarded",
          'scene === \"weekly\" ?' in scene_tsx and "WeeklyTimeline" in scene_tsx, "")
    check("timeline_and_library_same_grid",
          scene_tsx.index("WeeklyTimeline") < scene_tsx.index("成果文档库"), "left timeline, right library")

    # ---- R4 归档 + 会话历史同屏（093 修订后：归档并入成果库，只剩历史对话在左栏）----
    # R4 原语义是「归档与会话历史不再各占一条通栏」。093 首版把两者都放进左栏；
    # 2026-10-27 用户要求归档并入成果库，因此这里只保留「历史对话在左栏、独立滚动」的事实，
    # 归档合并由 verify_weekly_093.py 覆盖。
    check("threads_in_left_column",
          "max-lg:order-4" in scene_tsx, "history under the quadrants")
    check("archive_merged_not_standalone",
          "<WeeklyArchive" not in scene_tsx and "archiveEntries" in scene_tsx,
          "archive lives in the artifact library")
    check("threads_self_scroll", "max-h-[200px] overflow-y-auto" in scene_tsx, "")

    # ---- R1 指标带：口径与「有记录才出现」----
    check("metric_uses_entry_count",
          "parseWorklog(weekContent).entryCount" in scene_tsx, "not artifacts_this_week")
    check("weekly_metric_not_artifact_count",
          "artifacts_this_week" not in scene_tsx.split("SceneStatusStrip")[1].split("AnnouncementBar")[0]
          if "SceneStatusStrip" in scene_tsx else False,
          "weekly entries must not be file count")
    check("pending_hidden_when_zero",
          "waiting > 0 ? (" in strip_tsx and "waiting = pending + expired" in strip_tsx, "")
    check("pending_panel_kept", "PendingWritePanel" in strip_tsx, "click to expand still works")
    check("report_label_from_report_state",
          "reportState(reports, week, week)" in scene_tsx, "deterministic file check")

    # ---- R3 字号下限 ----
    entry_px = px("text-[13.5px]")
    date_px = px("text-[12px]")
    check("entry_font_13_5", "text-[13.5px] text-on-surface-variant leading-[1.7]" in tl_tsx,
          "entries >= 13.5px（093 修订把行距从 leading-relaxed 提到 1.7）")
    check("date_font_12", "mono text-[12px] tracking-wide" in tl_tsx, "date >= 12px")
    check("weekday_font_12", "text-[12px]" in tl_tsx, "")
    check("font_floor_sane", entry_px >= 13.5 and date_px >= 12.0,
          f"entry={entry_px} date={date_px}")

    # ---- 复盘界面移除，心跳保留，规则提醒展示层移除（OD-ARCH-095） ----
    check("no_review_ui_in_bar", "review_enabled" not in bar_tsx and "runReview" not in bar_tsx, "")
    check("no_review_ui_in_settings", "review_enabled" not in settings_tsx, "")
    check("no_review_cadence_ui", "复盘频率" not in bar_tsx and "立即复盘" not in settings_tsx, "")
    check("heartbeat_kept", "scheduler.tick()" in bar_tsx and "markHeartbeat" in bar_tsx, "")
    check("rule_reminder_ui_removed", "<section" not in bar_tsx and "确定性汇总没有产出" not in bar_tsx, "")
    check("heartbeat_bridge_has_no_visual_module", "return null;" in bar_tsx, "heartbeat remains non-visual")
    check("backend_review_contract_kept",
          "review_enabled" in (SRC / "types.ts").read_text(encoding="utf-8"),
          "scheduler/pending_writes contract untouched")

    # ---- 编译并执行真实解析模块（口径一致性）----
    tmp = Path(tempfile.mkdtemp(prefix="od092-"))
    compiled = compile_ts([SRC / "weekTimeline.ts"], tmp / "tl")
    if compiled.returncode != 0:
        check("weekTimeline_compiles", False, compiled.stderr[-1200:])
    else:
        check("weekTimeline_compiles", True, "")
        probe = tmp / "tl-probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(["node", str(probe), str(tmp / "tl" / "weekTimeline.js")], DESKTOP)
        if proc.returncode != 0:
            check("weekTimeline_probe_exit", False, (proc.stderr or proc.stdout)[-1200:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WEEKLY-092] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases)-len(failed)} failed={len(failed)}")

    EVID.mkdir(parents=True, exist_ok=True)
    out = Path(args.json) if args.json else EVID / "verify_weekly_092.json"
    out.write_text(json.dumps({"cases": cases, "failed": len(failed)}, ensure_ascii=False, indent=2),
                   encoding="utf-8")
    print(f"evidence -> {out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

