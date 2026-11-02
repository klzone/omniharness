#!/usr/bin/env python3
"""OD-ARCH-132：随手记的补充内容并入上一条（不再落成两条）。

证据等级：
  * 脚本运行：编译真实的 desktop/src/workRecordVerdict.ts，用用户实际踩坑的两句话
    跑判定解析、校验与索引定位；
  * 静态代码检查：宿主 merge_entry、命令接线、前端执行顺序、Skill 规则。

真实落盘（只改目标行、原子替换、备份、CRLF）由 cargo test worklog 覆盖。
模型是否把「还参观了…」判成 merge 属于实测量，不在此脚本范围内。

用法：python3 tests/verification/desktop/verify_worklog_132.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"
SKILLS = REPO / "skills"


def run(cmd, cwd):
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


PROBE = r"""
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

const CTX = { userText: "", week: "2026-W44", today: "2026-10-28" };

const FILE = [
  "# 2026-W44",
  "",
  "## 2026-10-26",
  "",
  "- **事**：周一上午赴上杭县城现场教学",
  "  **去向**：本周报引用",
  "",
  "- **事**：参观了紫金矿业生产基地",
  "  **去向**：本周报引用",
  "",
].join("\n");

const index = mod.buildWorklogIndex(FILE);
check("index_two_entries", index.entries.length === 2, index.entries.length);

const user = "还参观了紫金矿业生产基地和矿区";

// ---- 解析：merge 是合法通道 ----
let p = mod.parseVerdict(JSON.stringify({ merge: [{ date: "2026-10-26", index: 0, text: "还参观了紫金矿业生产基地" }], note: "已并入" }));
check("parse_merge_ok", p !== null && p.merge.length === 1, JSON.stringify(p));
check("parse_merge_text", p && p.merge[0].text === "还参观了紫金矿业生产基地", p ? p.merge[0].text : "");
check("parse_merge_defaults_empty", (() => {
  const q = mod.parseVerdict(JSON.stringify({ append: [], note: "" }));
  return q !== null && Array.isArray(q.merge) && q.merge.length === 0;
})(), "");
check("parse_merge_bad_type_null", mod.parseVerdict('{"merge":"x"}') === null, "");
check("parse_merge_missing_text_null", mod.parseVerdict('{"merge":[{"date":"2026-10-26","index":0}]}') === null, "");
check("parse_merge_bad_index_null", mod.parseVerdict('{"merge":[{"date":"2026-10-26","index":1.5,"text":"x"}]}') === null, "");

// ---- 校验：通过时带出 expectText ----
let r = mod.validateVerdict(p, { userText: user, week: "2026-W44", today: "2026-10-28" }, index);
check("merge_ok", r.ok === true, r.ok ? "" : r.reason);
check("merge_expect_text", r.ok && r.plan.merge[0].expectText === "周一上午赴上杭县城现场教学", r.ok ? r.plan.merge[0].expectText : "");
check("merge_keeps_index", r.ok && r.plan.merge[0].index === 0, "");

// 编造（不是用户原话）→ 拒绝
r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-26", index: 0, text: "参观了矿区并了解矿产知识" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_invented", r.ok === false && r.reason.includes("原话"), r.reason);

// 索引里没有 → 拒绝
r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-27", index: 0, text: "还参观了紫金矿业生产基地" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_missing_index", r.ok === false && r.reason.includes("索引里没有"), r.reason);

// 同一条补两次 → 拒绝
r = mod.validateVerdict(
  { append: [], move: [], merge: [
      { date: "2026-10-26", index: 0, text: "还参观了紫金矿业生产基地" },
      { date: "2026-10-26", index: 0, text: "和矿区" },
    ], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_twice", r.ok === false && r.reason.includes("两次"), r.reason);

// 记录里已经有这段 → 不重复并入
r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-26", index: 1, text: "参观了紫金矿业" }], note: "" },
  { userText: "参观了紫金矿业", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_duplicate_content", r.ok === false && r.reason.includes("已经有"), r.reason);

// 同一段话既并入又追加 → 拒绝
r = mod.validateVerdict(
  { append: [{ text: "还参观了紫金矿业生产基地", date: "2026-10-26" }], move: [],
    merge: [{ date: "2026-10-26", index: 0, text: "还参观了紫金矿业生产基地" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_with_append", r.ok === false, r.reason);

// 同一条既移动又并入 → 拒绝
r = mod.validateVerdict(
  { append: [], move: [{ date: "2026-10-26", index: 0, to: "2026-10-27" }],
    merge: [{ date: "2026-10-26", index: 0, text: "还参观了紫金矿业生产基地" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_with_move", r.ok === false && r.reason.includes("既移动又并入"), r.reason);

// 跨周 / 未来日期
r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-19", index: 0, text: "还参观了紫金矿业生产基地" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_cross_week", r.ok === false && r.reason.includes("不在"), r.reason);

r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-30", index: 0, text: "还参观了紫金矿业生产基地" }], note: "" },
  { userText: user, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_future", r.ok === false && r.reason.includes("晚于今天"), r.reason);

// 并入后过长 → 拒绝
r = mod.validateVerdict(
  { append: [], move: [], merge: [{ date: "2026-10-26", index: 0, text: "还参观了紫金矿业生产基地" + "和".repeat(1200) }], note: "" },
  { userText: "还参观了紫金矿业生产基地" + "和".repeat(1200), week: "2026-W44", today: "2026-10-28" },
  index,
);
check("merge_reject_too_long", r.ok === false && r.reason.includes("过长"), r.reason);

// 旧判定（没有 merge 字段）仍然可用
r = mod.validateVerdict(
  { append: [{ text: "周一上午赴上杭县城现场教学", date: "2026-10-26" }], move: [], note: "" },
  { userText: "周一上午赴上杭县城现场教学", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("legacy_append_still_ok", r.ok === true && r.plan.merge.length === 0, r.ok ? "" : r.reason);

// 回一句话：三个数组都空
r = mod.validateVerdict(
  { append: [], move: [], merge: [], note: "明天下午专题课。" },
  { userText: "明天下午专题课。", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reply_only_ok", r.ok === true && r.plan.append.length + r.plan.move.length + r.plan.merge.length === 0, "");

console.log(JSON.stringify({ cases }));
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default="")
    args = ap.parse_args()

    cases = []

    def check(name, ok, detail=""):
        cases.append({"name": name, "ok": bool(ok), "detail": str(detail)})

    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    worklog = (DESKTOP / "src-tauri" / "src" / "worklog.rs").read_text(encoding="utf-8")
    skill = (SKILLS / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")

    # ---- 静态：宿主 ----
    check("worklog_merge_entry", "pub fn merge_entry(" in worklog)
    check("worklog_merge_same_line", 'format!("{line}；{addition}")' in worklog)
    check("worklog_merge_no_truncate", "truncate(true)" not in worklog)
    check("worklog_merge_guards_lines", "fn assert_only_target_line_changed" in worklog)
    check("worklog_merge_rejects_multiline", "补充内容只能是一行" in worklog)
    check("worklog_merge_atomic", "commit_week_file(&path, week, &original, &updated)" in worklog)
    check("worklog_merge_locator_reuses_move", "locate_entry_block(&lines, date, index, expect_text)" in worklog)

    # ---- 静态：命令 ----
    check("lib_merge_command", "fn worklog_merge(" in lib)
    check("lib_merge_registered", "worklog_merge," in lib)
    check("lib_merge_red_line", "red_line_scan_with(&text, &policy)" in lib)
    check("lib_merge_masks", "mask_privacy(&text, &policy)" in lib)

    # ---- 静态：前端 ----
    check("api_merge", '"worklog_merge"' in api and "worklogMerge(" in api)
    check("types_merge_receipt", "export interface WorklogMergeReceipt" in types)
    check("home_calls_merge", "shell.worklogMerge({" in home)
    move_at = home.index("shell.worklogMoveMany({")
    merge_at = home.index("shell.worklogMerge({")
    append_at = home.index("shell.worklogAppend(record.text")
    check("home_merge_before_move_before_append", merge_at < move_at < append_at, (merge_at, move_at, append_at))
    check(
        "home_empty_guard_includes_merge",
        "plan.append.length === 0 && plan.move.length === 0 && plan.merge.length === 0" in home,
    )
    check("home_reports_merge_line", '"· 并入 "' in home)

    # ---- 静态：Skill ----
    check("skill_merge_rule", "merge" in skill and "不新起条目" in skill)
    check("skill_merge_json", '"merge":[{"date"' in skill)
    check("skill_four_fields", "四个字段都可省" in skill)
    check("skill_within_budget", len(skill) <= 2500, f"{len(skill)} 字符")

    # ---- 运行：真实源码 ----
    tmp = Path(tempfile.mkdtemp(prefix="od132-"))
    compiled = run(
        [
            "npx", "--no-install", "tsc", str(SRC / "workRecordVerdict.ts"),
            "--outDir", str(tmp / "out"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    if compiled.returncode != 0:
        check("workRecordVerdict_compiles", False, compiled.stderr[-1500:])
    else:
        check("workRecordVerdict_compiles", True)
        probe = tmp / "probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(["node", str(probe), str(tmp / "out" / "workRecordVerdict.js")], DESKTOP)
        if proc.returncode != 0:
            check("verdict_probe_exit", False, proc.stderr[-1500:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WORKLOG-132] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases) - len(failed)} failed={len(failed)}")

    if args.json:
        Path(args.json).write_text(
            json.dumps({"cases": cases, "passed": not failed}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
