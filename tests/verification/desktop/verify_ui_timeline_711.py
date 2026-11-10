#!/usr/bin/env python3
"""OD-UI-711：前端执行时间线的静态接线 + 纯函数行为检查。

证据等级：文件事实 + 脚本运行。纯函数部分用 tsc 单独编译 timelineModel.ts，
再由 node 执行追加/聚合/摘要语义；界面展开展开/收起与正文顺序用源码结构检查。
不读取凭证、不访问用户数据、不启动真实模型。
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DESKTOP = REPO / "desktop"
HOME = DESKTOP / "src/shell/HomeChat.tsx"
MODEL = DESKTOP / "src/shell/timelineModel.ts"
THEME = DESKTOP / "src/theme.css"
TSC = DESKTOP / "node_modules/.bin/tsc"

CASES: list[dict[str, object]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=False)


def behavioral_checks() -> None:
    if not TSC.exists():
        check("tsc_available", False, str(TSC))
        return

    with tempfile.TemporaryDirectory(prefix="od711-timeline-") as raw_tmp:
        tmp = Path(raw_tmp)
        compiled = run(
            [
                str(TSC),
                "--ignoreConfig",
                str(MODEL),
                "--outDir",
                str(tmp),
                "--target",
                "ES2022",
                "--module",
                "ES2022",
                "--moduleResolution",
                "bundler",
                "--skipLibCheck",
            ],
            DESKTOP,
        )
        if compiled.returncode != 0:
            check("timeline_model_compiles", False, compiled.stderr.strip()[:400])
            return
        check("timeline_model_compiles", True)

        (tmp / "package.json").write_text('{"type":"module"}\n', encoding="utf-8")
        (tmp / "run.mjs").write_text(
            r'''
import {
  addUnknownChunk,
  appendThoughtDelta,
  appendToolActivity,
  formatTimelineTime,
  timelineSummary,
  visibleTimelineEntries,
} from "./timelineModel.js";

function expect(name, condition, detail = "") {
  console.log((condition ? "PASS " : "FAIL ") + name + (detail ? " " + detail : ""));
}

// 1. tool/call + tool/result 无 callId：归为同一条命令，状态历史两步。
let t = appendToolActivity([], {
  seq: 1, at: 100, eventKind: "tool/call", toolName: "bash", title: "bash", status: "running",
});
t = appendToolActivity(t, { seq: 2, at: 105, eventKind: "tool/result", status: "" });
expect("tool_result_merges_without_call_id", t.length === 1 && t[0].statusHistory?.length === 2,
  JSON.stringify(t));
expect("tool_history_labels", t[0].statusHistory?.[0]?.status === "执行中" &&
  t[0].statusHistory?.[1]?.status === "结果", JSON.stringify(t[0].statusHistory));
expect("tool_history_has_time", t[0].statusHistory?.[1]?.at === 105);

// 2. 有 callId 时按调用标识归并。
let c = appendToolActivity([], {
  seq: 3, at: 200, eventKind: "tool/call", toolName: "web_search", callId: "call-1", status: "pending",
});
c = appendToolActivity(c, {
  seq: 4, at: 204, eventKind: "tool/result", callId: "call-1", status: "succeeded",
});
expect("tool_call_id_merge", c.length === 1 && c[0].status === "完成" && c[0].statusHistory?.length === 2,
  JSON.stringify(c));

// 3. 两次独立 call 即使同名也应是两条命令。
let two = appendToolActivity([], { seq: 5, at: 300, eventKind: "tool/call", toolName: "bash" });
two = appendToolActivity(two, { seq: 6, at: 301, eventKind: "tool/call", toolName: "bash" });
expect("tool_separate_calls", two.length === 2, JSON.stringify(two));

// 4. 思考增量累积；工具之后另起一段。
let th = appendThoughtDelta([], { seq: 7, at: 400, text: "甲" });
th = appendThoughtDelta(th, { seq: 8, at: 401, text: "乙" });
expect("thought_accumulates", th.length === 1 && th[0].text === "甲乙", JSON.stringify(th));
th = appendToolActivity(th, { seq: 9, at: 402, eventKind: "tool/call", toolName: "bash" });
th = appendThoughtDelta(th, { seq: 10, at: 403, text: "丙" });
expect("thought_new_segment_after_tool", th.filter((e) => e.kind === "thought").length === 2,
  JSON.stringify(th));

// 5. unknown_chunk 只聚合一条计数，正常界面不展示 note。
let u = [];
for (let i = 0; i < 3; i += 1) u = addUnknownChunk(u, { seq: 11 + i, at: 500 + i });
expect("unknown_chunk_aggregates", u.length === 1 && u[0].kind === "note" && u[0].count === 3,
  JSON.stringify(u));
expect("unknown_chunk_count_label", u[0].status === "3 个后台块");
expect("unknown_chunk_not_rendered", visibleTimelineEntries(u).length === 0);

// 6. 摘要只数命令与思考，后台块不掺入。
expect("summary_counts", timelineSummary(th) === "执行了 1 条命令 · 2 段思考", timelineSummary(th));
expect("summary_excludes_notes", timelineSummary([...th, ...u]) === timelineSummary(th));

// 7. 时间格式：epoch 秒转本地时分秒。
expect("format_time_seconds", /^\d{2}:\d{2}:\d{2}$/.test(formatTimelineTime(1700000000)));
expect("format_time_missing", formatTimelineTime(undefined) === "");

// 8. 追加不修改传入数组。
const before = [];
appendToolActivity(before, { at: 600, eventKind: "tool/call", toolName: "bash" });
expect("append_does_not_mutate_input", before.length === 0);
''' ,
            encoding="utf-8",
        )

        node = run(["node", "run.mjs"], tmp)
        if node.returncode != 0:
            check("timeline_behavior_runs", False, (node.stderr or node.stdout).strip()[:400])
            return
        check("timeline_behavior_runs", True)
        for line in node.stdout.splitlines():
            if line.startswith("PASS ") or line.startswith("FAIL "):
                name = line[5:].split(" ", 1)[0]
                check(name, line.startswith("PASS "), "" if line.startswith("PASS ") else line)


def runtime_zero_change() -> None:
    diff = run(["git", "diff", "--name-only", "HEAD", "--", "runtime"], REPO)
    status = run(["git", "status", "--porcelain", "--", "runtime"], REPO)
    check("runtime_no_diff", diff.returncode == 0 and not diff.stdout.strip(), diff.stdout.strip())
    check("runtime_no_status", status.returncode == 0 and not status.stdout.strip(), status.stdout.strip())


def main() -> int:
    for path, name in ((HOME, "homechat_exists"), (MODEL, "timeline_model_exists"), (THEME, "theme_exists")):
        check(name, path.exists(), str(path))

    home = HOME.read_text(encoding="utf-8") if HOME.exists() else ""
    model = MODEL.read_text(encoding="utf-8") if MODEL.exists() else ""
    theme = THEME.read_text(encoding="utf-8") if THEME.exists() else ""

    check("message_has_optional_timeline", "timeline?: TimelineEntry[];" in home)
    check("timeline_kinds", 'export type TimelineKind = "tool" | "thought" | "note";' in model
          and "kind: TimelineKind;" in model)
    check("tool_activity_appends", "timeline: appendToolActivity(m.timeline ?? [], {" in home)
    check("thought_delta_appends", "appendThoughtDelta(m.timeline ?? [], {" in home)
    check("unknown_chunk_aggregates", "addUnknownChunk(m.timeline ?? [], {" in home)
    check("old_thought_state_removed", "setThoughts" not in home and "thoughts[" not in home)

    start = home.find('if (event.type !== "text_delta") return;')
    end = home.find(".then((fn)", start)
    text_block = home[start:end] if start >= 0 and end >= 0 else ""
    check("text_delta_not_in_timeline", bool(text_block) and "appendThoughtDelta" not in text_block
          and "appendToolActivity" not in text_block and "timeline:" not in text_block)

    tool_start = home.find('if (event.type === "tool_activity")')
    tool_end = home.find('if (event.type !== "text_delta")', tool_start)
    tool_block = home[tool_start:tool_end] if tool_start >= 0 and tool_end >= 0 else ""
    check("tool_payload_has_no_input_or_result", bool(tool_block) and all(token not in tool_block for token in
          ("arguments", "rawInput", "result", "input")))

    check("timeline_component", "function TimelineView(" in home and "function TimelineEntryRow(" in home)
    check("timeline_before_body",
          home.find("<TimelineView timeline={m.timeline}") >= 0
          and home.find("<TimelineView timeline={m.timeline}") < home.find("<AssistantText"))
    check("tool_glyph", '">_"' in home)
    check("thought_glyph", '"@@"' in home)
    check("tool_kind_word", '"命令"' in home)
    check("thought_kind_word", '"思考"' in home)
    check("chevron_expand", 'name="chevron-down"' in home and "od-timeline-chevron" in home)
    check("entry_aria_expanded", "aria-expanded={expanded}" in home)
    check("summary_aria_expanded", "aria-expanded={open}" in home)
    check("status_history_render", "statusHistory.map(" in home and "formatTimelineTime(change.at)" in home)
    check("thought_text_render", 'className="od-timeline-thought"' in home)
    check("summary_copy", "timelineSummary(timeline)" in home
          and "执行了 ${tools} 条命令 · ${thoughts} 段思考" in model)
    check("summary_collapse", "{terminal ? (" in home and "!terminal || open ? list : null" in home)
    check("notes_hidden", "visibleTimelineEntries" in model and 'entry.kind === "tool" || entry.kind === "thought"' in model)
    check("old_message_fallback", "!m.timeline || m.timeline.length === 0" in home and "m.toolNote" in home)
    check("empty_timeline_returns_null", "if (visible.length === 0) return null;" in home)

    timeline_src = home[home.find("function TimelineEntryRow"):home.find("function enabledCategories")] if "function TimelineEntryRow" in home else ""
    check("timeline_no_auto_stop", bool(timeline_src) and all(token not in timeline_src for token in
          ("cancelTask", "setInterval", "setTimeout", "自动停止")))
    check("timeline_model_no_io", all(token not in model for token in ("invoke(", "fetch(", "setInterval", "setTimeout")))

    check("theme_timeline_classes", all(name in theme for name in
          (".od-timeline {", ".od-timeline-row", ".od-timeline-chevron.is-open", ".od-timeline-thought")))
    check("theme_uses_tokens", "var(--color-outline)" in theme and "var(--color-on-surface-variant)" in theme)

    behavioral_checks()
    runtime_zero_change()

    failed = [case for case in CASES if not case["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"],
                             (" " + str(case["detail"])) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    print(json.dumps({"total": len(CASES), "failed": len(failed)}, ensure_ascii=False))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
