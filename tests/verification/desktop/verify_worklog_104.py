#!/usr/bin/env python3
"""OD-ARCH-104：随手记改由模型判定「指令 / 内容」，代码只校验与落盘。

证据等级：
  * 脚本运行：编译真实的 desktop/src/workRecordVerdict.ts + weekTimeline.ts + workIntent.ts，
    用用户实际踩坑的三句话跑校验、判定解析、索引与显示剥离；
  * 静态代码检查：提示词、Skill、界面接线、宿主命令。

真实落盘（批量移动、备份、条目守恒、CRLF）由 cargo test worklog 覆盖；
模型判定准确率属于实测量，不在此脚本范围内。

用法：python3 tests/verification/desktop/verify_worklog_104.py [--json PATH]
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

// 当周文件：周一两条、周三两条（用户实际踩坑的场景）
const FILE = [
  "# 2026-W44",
  "",
  "## 2026-10-26",
  "",
  "- **事**：周一上午赴上杭县城现场教学",
  "  **去向**：本周报引用",
  "",
  "## 2026-10-28",
  "",
  "- **事**：明天下午专题课《新时代党内法规制度建设》",
  "  **去向**：本周报引用",
  "",
  "- **事**：上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录。",
  "  **去向**：本周报引用",
  "",
].join("\n");

const index = mod.buildWorklogIndex(FILE);
check("index_week", index.week === "2026-W44", index.week);
check("index_entries", index.entries.length === 3, index.entries.length);
check("index_first_monday", index.entries[0].date === "2026-10-26" && index.entries[0].index === 0, JSON.stringify(index.entries[0]));
check("index_wed_second", index.entries[2].date === "2026-10-28" && index.entries[2].index === 1, JSON.stringify(index.entries[2]));

const desc = mod.describeIndex(index, "2026-10-28");
check("describe_has_date", desc.includes("2026-10-26 #0"), desc);
check("describe_has_index1", desc.includes("2026-10-28 #1"), desc);
check("describe_empty", mod.describeIndex(mod.buildWorklogIndex(""), "2026-10-28") === "（本周还没有记录）", "");

// ---- 原场景：指令句不该产生正文，「上杭教学」「紫金矿业」应挪到周一 ----
const user1 = "上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录。";
const verdict1 = {
  action: "move",
  records: [],
  moves: [{ date: "2026-10-28", index: 1, to: "2026-10-26" }],
  reply: "已把这条挪到周一。",
};
let r = mod.validateVerdict(verdict1, { userText: user1, week: CTX.week, today: CTX.today }, index);
check("move_ok", r.ok === true, r.ok ? "" : r.reason);
check("move_expect_text", r.ok && r.plan.moves[0].expectText.includes("紫金矿业"), r.ok ? r.plan.moves[0].expectText : "");

// 把指令原样当成正文写下去 → 应当被拒绝：不是这条指令本身违例，而是它必须作为指令处理；
// 这里验证的是「模型想新建一条记录但日期/正文不匹配」时校验能挡住编造。
const invented = {
  action: "record",
  records: [{ text: "赴上杭县城学习生态文明思想", date: "2026-10-26" }],
  moves: [],
  reply: "ok",
};
r = mod.validateVerdict(invented, { userText: user1, week: CTX.week, today: CTX.today }, index);
check("reject_invented_text", r.ok === false, r.ok ? "应拒绝" : r.reason);

// 原话片段（含标点差异）可以通过
const verbatim = {
  action: "record",
  records: [{ text: "上杭教学和参观紫金矿业", date: "2026-10-26" }],
  moves: [],
  reply: "ok",
};
r = mod.validateVerdict(verbatim, { userText: user1, week: CTX.week, today: CTX.today }, index);
check("accept_verbatim_piece", r.ok === true, r.ok ? "" : r.reason);

// 标点差异不算编造
r = mod.validateVerdict(
  { action: "record", records: [{ text: "上杭教学, 参观紫金矿业", date: "2026-10-26" }], moves: [], reply: "" },
  { userText: "上杭教学，参观紫金矿业", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("accept_punct_diff", r.ok === true, r.ok ? "" : r.reason);

// 跨周 / 未来日期
r = mod.validateVerdict(
  { action: "record", records: [{ text: user1, date: "2026-10-01" }], moves: [], reply: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_cross_week", r.ok === false && r.reason.includes("不在"), r.reason);

r = mod.validateVerdict(
  { action: "record", records: [{ text: user1, date: "2026-10-30" }], moves: [], reply: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_future", r.ok === false && r.reason.includes("晚于今天"), r.reason);

// 索引越界
r = mod.validateVerdict(
  { action: "move", records: [], moves: [{ date: "2026-10-28", index: 9, to: "2026-10-26" }], reply: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_index_oob", r.ok === false && r.reason.includes("索引"), r.reason);

// 原日期==目标日期
r = mod.validateVerdict(
  { action: "move", records: [], moves: [{ date: "2026-10-28", index: 1, to: "2026-10-28" }], reply: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_same_date", r.ok === false, r.reason);

// 同一位置移两次
r = mod.validateVerdict(
  { action: "move", records: [], moves: [
      { date: "2026-10-28", index: 1, to: "2026-10-26" },
      { date: "2026-10-28", index: 1, to: "2026-10-27" },
    ], reply: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_duplicate_move", r.ok === false && r.reason.includes("两次"), r.reason);

// action 与内容自相矛盾
r = mod.validateVerdict({ action: "record", records: [], moves: [], reply: "" }, { userText: user1, week: "2026-W44", today: "2026-10-28" }, index);
check("reject_record_without_records", r.ok === false, r.reason);
r = mod.validateVerdict({ action: "move", records: [], moves: [], reply: "" }, { userText: user1, week: "2026-W44", today: "2026-10-28" }, index);
check("reject_move_without_moves", r.ok === false, r.reason);
r = mod.validateVerdict({ action: "answer", records: [{ text: "上杭教学和参观紫金矿业", date: "2026-10-26" }], moves: [], reply: "" }, { userText: user1, week: "2026-W44", today: "2026-10-28" }, index);
check("reject_answer_with_write", r.ok === false, r.reason);
r = mod.validateVerdict({ action: "confirm", records: [], moves: [], reply: "就是周一上午。" }, { userText: "就是周一上午", week: "2026-W44", today: "2026-10-28" }, index);
check("accept_confirm", r.ok === true, r.ok ? "" : r.reason);
r = mod.validateVerdict({ action: "answer", records: [], moves: [], reply: "这周有 3 条。" }, { userText: "这周记了几条", week: "2026-W44", today: "2026-10-28" }, index);
check("accept_answer", r.ok === true, r.ok ? "" : r.reason);

// ---- 判定块解析 ----
const full = "已记录。" + "\n" + mod.VERDICT_OPEN + "\n" +
  JSON.stringify({ action: "record", records: [{ text: "上杭教学和参观紫金矿业", date: "2026-10-26" }], moves: [], reply: "已记录" }) +
  "\n" + mod.VERDICT_CLOSE;
const split1 = mod.splitVerdict(full);
check("split_visible", split1.visible.trim() === "已记录。", JSON.stringify(split1.visible));
check("split_block", !!split1.block && split1.block.includes("action"), split1.block);
check("split_unclosed_hidden", mod.splitVerdict("已记录\n" + mod.VERDICT_OPEN + '{"action":"rec').visible.trim() === "已记录", "");
check("display_no_block", !mod.verdictDisplayText(full).includes("omnidesk-verdict"), "");
check("parse_fenced", (() => {
  const fence = String.fromCharCode(96).repeat(3);
  const p = mod.parseVerdict(fence + "json\n" + JSON.stringify({ action: "answer", records: [], moves: [], reply: "x" }) + "\n" + fence);
  return p && p.action === "answer";
})(), "");
check("parse_garbage_null", mod.parseVerdict("not json") === null, "");
check("parse_truncated_null", mod.parseVerdict('{"action":"record","records":[') === null, "");
check("parse_bad_action_null", mod.parseVerdict('{"action":"delete"}') === null, "");
check("parse_empty_null", mod.parseVerdict("") === null, "");

// 降级路径：没有判定块 → block 为 null，界面据此按原文原样记录
check("fallback_no_block", mod.splitVerdict("已记录。").block === null, "");

// ---- 文本工具 ----
check("comparable_ignores_punct", mod.comparableText("上杭教学，A-1 班") === "上杭教学A1班", mod.comparableText("上杭教学，A-1 班"));
check("short_text", mod.shortText("一二三四五六七八九十一二三四五六七八九十", 6) === "一二三四五六…", mod.shortText("一二三四五六七八九十一二三四五六七八九十", 6));
check("day_number_invalid", mod.dayNumberOf("2026-02-30") === null, "");
check("day_number_ok", typeof mod.dayNumberOf("2026-10-28") === "number", "");

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
    prompt = (SRC / "conversationPrompt.ts").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    worklog = (DESKTOP / "src-tauri" / "src" / "worklog.rs").read_text(encoding="utf-8")
    skill = (SKILLS / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")

    # ---- 静态：语义判断不再由前端正则兜着 ----
    check("home_no_redate_regex", "detectRedateInstruction(" not in home)
    check("home_no_continuation_regex", "isNoteContinuation(" not in home)
    check("home_reads_week_before_model", home.index("shell.worklogReadWeek()") < home.index("notePendingRef.current.set("))
    # 模型判定到达之前一个字节都不写：append 只允许出现在 executeNote 里
    # （降级兜底一处、按判定逐条写入一处）。
    exec_start = home.index("const executeNote = useCallback(")
    exec_end = home.index("const settleNote = useCallback(")
    append_calls = [
        i for i in range(len(home)) if home.startswith("shell.worklogAppend(", i)
    ]
    check("home_only_two_append_sites", len(append_calls) == 2, len(append_calls))
    check(
        "home_appends_only_after_verdict",
        all(exec_start < i < exec_end for i in append_calls),
        [home[max(0, i - 60):i][-40:] for i in append_calls],
    )
    check("home_validates_before_write", home.index("validateVerdict(verdict, ctx, ctx.index)") < home.index("shell.worklogMoveMany({"))
    check("home_uses_batch_move", "shell.worklogMoveMany({" in home)
    check("home_strips_verdict_buffer", "verdictDisplayText(text)" in home)
    # 回读失败时消息里还可能留着流式收到的判定块：终态执行前必须再剥一次。
    check(
        "home_strips_before_settle",
        home.index("text: verdictDisplayText(m.text)") < home.index("await settleNote(taskId, finalText"),
    )
    check("home_says_fallback", "模型未给出判定" in home and "未做指令识别" in home)
    check("home_local_receipt_label", '"工作台本地处理 · 未调用模型"' in home)
    check("home_cancel_writes_nothing", "本轮已取消，这句没有写入工作记录。" in home)

    # ---- 静态：提示词 ----
    check("prompt_has_note_index", "noteIndex" in prompt and "当周已有记录" in prompt)
    check("prompt_asks_verdict", "<omnidesk-verdict>" in prompt)
    check("prompt_lists_actions", all(k in prompt for k in ['"record|move|confirm|answer"', "records", "moves"]))
    check("prompt_rejects_invention", "逐字取自用户原话" in prompt)
    check("prompt_keeps_skill_injection", '"配套技能指引（" + s.id + "）："' in prompt)

    # ---- 静态：API / 类型 / 宿主 ----
    check("api_move_many", '"worklog_move_many"' in api)
    check("types_move_many", "export interface WorklogMoveManyReceipt" in types)
    check("lib_registers_move_many", "worklog_move_many," in lib)
    check("worklog_move_entries", "pub fn move_entries(" in worklog)
    check("worklog_batch_atomic", "locate_entry_block(&lines, &request.date, request.index, &request.expect_text)" in worklog)
    check("worklog_batch_cross_week", "跨周移动暂不支持" in worklog)
    check("worklog_batch_no_truncate", "truncate(true)" not in worklog)

    # ---- 静态：Skill ----
    check("skill_six_fields", all(k in skill for k in ["时间", "地点", "人物", "事件", "影响", "感受"]))
    check("skill_no_file_permission", "没有写文件的权限" in skill)
    check("skill_verdict_format", "<omnidesk-verdict>" in skill and "records" in skill and "moves" in skill)
    check("skill_no_report_sections", "本周概述" not in skill)
    check("skill_links_report", "memory-report" in skill)
    check("skill_within_budget", len(skill) <= 2500, f"{len(skill)} 字符")

    # ---- 运行：真实源码 ----
    tmp = Path(tempfile.mkdtemp(prefix="od104-"))
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
        print(("PASS " if c["ok"] else "FAIL ") + "[WORKLOG-104] " + c["name"] +
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
