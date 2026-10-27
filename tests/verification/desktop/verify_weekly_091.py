#!/usr/bin/env python3
"""OD-ARCH-091：周报分页改版（时间线 + 报告状态 + 历史归档）。

证据等级：
  * 静态代码检查：命令已注册、chips 区已移除、其他两个场景的分支未被改写；
  * 脚本运行：编译真实的 desktop/src/weekTimeline.ts 执行断言（解析真实写入格式、
    续段合并、坏行保留、周日期换算、周报三态判定）。

**不**调用模型、不开窗口。宿主的目录扫描与路径校验在 cargo test worklog 里覆盖；
窗口级预览另走 S4（Xvfb 截图）。

用法：python3 tests/verification/desktop/verify_weekly_091.py [--json PATH]
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
EVID = HERE / "evidence" / "od-arch-091"


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

// ---- A. 解析宿主真实写入格式（与 worklog.rs::render_block 逐字同形）----
const WEEK = [
  "# 2026-W44",
  "",
  "## 2026-10-26",
  "",
  "- **事**：上午在合成会场听课，主题是合成标记甲",
  "  **去向**：本周报引用",
  "",
  "## 2026-10-27",
  "",
  "- **事**：合成标记乙：把材料交到合成窗口",
  "  **去向**：知识卡",
  "",
  "## 2026-10-27（续）",
  "",
  "- **事**：合成标记丙",
  "  **去向**：本周报引用",
  "",
].join("\n");

const p = mod.parseWorklog(WEEK);
check("week_header", p.week === "2026-W44", p.week);
check("day_count_merges_suffix", p.days.length === 2, p.days.map((d) => d.date).join(","));
check("entry_count", p.entryCount === 3, String(p.entryCount));

const d1 = p.days.find((d) => d.date === "2026-10-26");
check("entry_text_exact", !!d1 && d1.entries[0].text === "上午在合成会场听课，主题是合成标记甲", d1 && d1.entries[0].text);
check("entry_destination", !!d1 && d1.entries[0].destination === "本周报引用", d1 && d1.entries[0].destination);

const d2 = p.days.find((d) => d.date === "2026-10-27");
check("continued_same_day", !!d2 && d2.entries.length === 2, d2 && String(d2.entries.length));
check("continued_order_kept", !!d2 && /乙/.test(d2.entries[0].text) && /丙/.test(d2.entries[1].text),
  d2 && d2.entries.map((e) => e.text).join(" | "));
check("continued_flag", !!d2 && d2.continued === true, d2 && String(d2.continued));
check("first_day_not_continued", !!d1 && d1.continued === false, d1 && String(d1.continued));
check("no_raw_when_clean", p.days.every((d) => d.raw.length === 0), JSON.stringify(p.days.map((d) => d.raw)));

// ---- B. 多行记录：用户原文里的换行不能被吞掉 ----
const MULTI = [
  "# 2026-W44",
  "",
  "## 2026-10-28",
  "",
  "- **事**：第一行合成文本",
  "第二行合成文本",
  "  **去向**：inbox",
  "",
].join("\n");
const pm = mod.parseWorklog(MULTI);
const dm = pm.days[0];
check("multiline_single_entry", !!dm && dm.entries.length === 1, dm && String(dm.entries.length));
check("multiline_text", !!dm && dm.entries[0].text.includes("第一行") && dm.entries[0].text.includes("第二行"), dm && dm.entries[0].text);
check("multiline_destination", !!dm && dm.entries[0].destination === "inbox", dm && dm.entries[0].destination);

// ---- C. 坏行不丢弃 ----
const BROKEN = [
  "# 2026-W44",
  "",
  "这是一行没有日期归属的合成文本",
  "",
  "## 2026-10-29",
  "",
  "没有前缀的一行合成文本",
  "### 更深的标题也认不出",
  "",
].join("\n");
const pb = mod.parseWorklog(BROKEN);
check("preamble_kept", pb.preamble.length === 1 && pb.preamble[0].includes("没有日期归属"), JSON.stringify(pb.preamble));
check("bad_line_kept_in_day", pb.days[0].raw.length === 2, JSON.stringify(pb.days[0].raw));
check("bad_line_text_intact", pb.days[0].raw[0].includes("没有前缀"), pb.days[0].raw[0]);
check("entry_count_zero_when_broken", pb.entryCount === 0, String(pb.entryCount));

// ---- D. 空内容与空日期段 ----
const pe = mod.parseWorklog("");
check("empty_ok", pe.days.length === 0 && pe.week === null && pe.entryCount === 0, JSON.stringify(pe));
const pz = mod.parseWorklog("# 2026-W44\n\n## 2026-10-26\n");
check("empty_day_ok", pz.days.length === 1 && pz.days[0].entries.length === 0, JSON.stringify(pz.days));

// ---- E. 周日期换算（ISO 8601，周一为首日）----
const w44 = mod.weekDates("2026-W44");
check("week_dates_len", w44.length === 7, String(w44.length));
check("week_start_monday", w44[0] === "2026-10-26", w44[0]);
check("week_end_sunday", w44[6] === "2026-11-01", w44[6]);
const w01 = mod.weekDates("2026-W01");
check("week01_monday", w01[0] === "2025-12-29", w01[0]);
check("bad_week_label", mod.weekDates("2026-W99").length === 0, "should be empty");
check("bad_week_shape", mod.weekDates("x").length === 0, "should be empty");
check("weekday_label", mod.weekdayLabel("2026-10-26") === "周一", mod.weekdayLabel("2026-10-26"));
check("weekday_label_sunday", mod.weekdayLabel("2026-11-01") === "周日", mod.weekdayLabel("2026-11-01"));
check("short_date", mod.shortDate("2026-10-26") === "10/26", mod.shortDate("2026-10-26"));

// ---- F. 时间线铺满七天 + 跨周记录不丢 ----
const OVER = mod.parseWorklog([
  "# 2026-W44",
  "",
  "## 2026-10-26",
  "",
  "- **事**：本周内合成记录",
  "  **去向**：本周报引用",
  "",
  "## 2026-10-20",
  "",
  "- **事**：补记的上一周合成记录",
  "  **去向**：本周报引用",
  "",
].join("\n"));
const tl = mod.buildTimeline(OVER, "2026-W44");
check("timeline_has_seven_plus", tl.length === 8, String(tl.length));
check("timeline_placeholder_days", tl.slice(0, 7).filter((d) => d.entries.length === 0).length === 6,
  JSON.stringify(tl.slice(0, 7).map((d) => d.entries.length)));
const outRow = tl.find((d) => d.date === "2026-10-20");
check("out_of_week_kept", !!outRow && outRow.outOfWeek === true, outRow && String(outRow.outOfWeek));
check("out_of_week_has_entry", !!outRow && outRow.entries.length === 1, outRow && String(outRow.entries.length));

check("empty_timeline_true", mod.isEmptyTimeline(mod.parseWorklog("# 2026-W44\n").days), "no days = empty");
check("empty_timeline_false", !mod.isEmptyTimeline(OVER.days), "has days with entries");

// ---- G. 周报三态（纯文件名判定）----
const files = [
  { week: "2026-W44", file_name: "2026-W44-周报.md" },
  { week: "2026-W43", file_name: "2026-W43-周报.md" },
];
check("state_missing", mod.reportState([], "2026-W44", "2026-W44").state === "missing", "empty -> missing");
const rDraft = mod.reportState(files, "2026-W44", "2026-W44");
check("state_draft", rDraft.state === "draft", rDraft.state);
check("state_draft_files", rDraft.files.length === 1, String(rDraft.files.length));
check("state_archived", mod.reportState(files, "2026-W43", "2026-W44").state === "archived", "past week");
const dup = mod.reportState([
  { week: "2026-W44", file_name: "2026-W44-周报.md" },
  { week: "2026-W44", file_name: "2026-W44-周报-2.md" },
], "2026-W44", "2026-W44");
check("state_dup_count", dup.files.length === 2, String(dup.files.length));
check("state_ignores_other_week", mod.reportState(files, "2026-W45", "2026-W44").state === "missing", "should be missing");

check("week_month", mod.weekMonth("2026-W44") === "2026-10", mod.weekMonth("2026-W44"));
check("week_month_bad", mod.weekMonth("x") === null, mod.weekMonth("x"));

process.stdout.write(JSON.stringify({ cases }) + "\n");
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    tmp = Path(tempfile.mkdtemp(prefix="od091-"))

    # ---- 静态：宿主命令已注册 ----
    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    check("cmd_archive_registered", "worklog_archive_list," in lib, "")
    check("cmd_read_report_registered", "worklog_read_report," in lib, "")
    worklog_rs = (DESKTOP / "src-tauri" / "src" / "worklog.rs").read_text(encoding="utf-8")
    check("rs_has_list_weeks", "pub fn list_weeks" in worklog_rs, "")
    check("rs_has_list_reports", "pub fn list_reports" in worklog_rs, "")
    check("rs_has_traversal_guard", "pub fn read_report" in worklog_rs, "")

    # ---- 静态：前端接线与旧区块移除 ----
    scene_tsx = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    check("chips_removed", "本期记录碎片" not in scene_tsx, "old chips block should be gone")
    check("chips_token_removed", "weekFragments" not in scene_tsx, "")
    check("timeline_wired", "WeeklyTimeline" in scene_tsx, "")
    check("status_wired", "WeeklyReportStatus" in scene_tsx, "")
    # 091 的「历史归档」在 093 修订后并入成果库，不再有独立 WeeklyArchive 组件；
    # 归档能力本身（worklog_archive_list、往期周报/周记录可打开）仍是 091 的契约。
    check("archive_wired", "archiveEntries" in scene_tsx and "worklogArchiveList" in scene_tsx, "")
    # OD-ARCH-092 取消 weekly 页签；OD-ARCH-093 扩为三栏同屏，页签只剩 project/writing 用。
    check("weekly_no_timeline_tab", '"timeline"' not in scene_tsx, "weekly tab should be gone in 092")
    check("weekly_three_columns", "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)]" in scene_tsx, "")
    check("archive_threads_in_left_column", "max-lg:order-4" in scene_tsx, "history in the left column")
    # 其他两个场景的既有区块必须原样保留。
    check("progress_tab_kept", "def.hasProgress ? (" in scene_tsx, "")
    check("artifacts_tab_kept", 'tab === "artifacts"' in scene_tsx, "")
    check("threads_section_kept", "会话历史" in scene_tsx, "")
    check("announcement_kept", "AnnouncementBar" in scene_tsx, "")

    scene_threads = (SRC / "sceneThreads.ts").read_text(encoding="utf-8")
    check("prefill_slot_added", "pendingNewThreadPrompt" in scene_threads, "")
    check("prefill_returned", "prompt: string | null" in scene_threads, "")
    home_chat = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    check("home_chat_initial_input", "initialInput" in home_chat, "")
    check("home_chat_prefills_state", "useState(initialInput)" in home_chat, "")
    home_canvas = (SRC / "shell" / "HomeCanvas.tsx").read_text(encoding="utf-8")
    check("home_canvas_passes_prompt", "initialInput={initialInput}" in home_canvas, "")

    api_ts = (SRC / "api.ts").read_text(encoding="utf-8")
    check("api_archive_list", "worklogArchiveList()" in api_ts, "")
    check("api_read_report", "worklogReadReport(" in api_ts, "")

    # ---- 运行：编译并执行真实解析模块 ----
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
        print(("PASS " if c["ok"] else "FAIL ") + "[WEEKLY-091] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases)-len(failed)} failed={len(failed)}")

    EVID.mkdir(parents=True, exist_ok=True)
    out = Path(args.json) if args.json else EVID / "verify_weekly_091.json"
    out.write_text(
        json.dumps({"cases": cases, "failed": len(failed)}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"evidence -> {out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
