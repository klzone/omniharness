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

// 当周文件：周一一条、周三两条（用户实际踩坑的场景）
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

const user1 = "上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录。";

// ---- 通用协议：只有 append / move / note，没有业务分类 ----
let r = mod.validateVerdict(
  { append: [], move: [{ date: "2026-10-28", index: 1, to: "2026-10-26" }], note: "已挪到周一。" },
  { userText: user1, week: CTX.week, today: CTX.today },
  index,
);
check("move_ok", r.ok === true, r.ok ? "" : r.reason);
check("move_expect_text", r.ok && r.plan.move[0].expectText.includes("紫金矿业"), r.ok ? r.plan.move[0].expectText : "");
check("move_no_action_field", r.ok && r.plan.action === undefined, r.ok ? String(r.plan.action) : "");

// 编造正文（不是用户原话）→ 拒绝
r = mod.validateVerdict(
  { append: [{ text: "赴上杭县城学习生态文明思想", date: "2026-10-26" }], move: [], note: "" },
  { userText: user1, week: CTX.week, today: CTX.today },
  index,
);
check("reject_invented_text", r.ok === false, r.ok ? "应拒绝" : r.reason);

// 原话片段可以通过
r = mod.validateVerdict(
  { append: [{ text: "上杭教学和参观紫金矿业", date: "2026-10-26" }], move: [], note: "ok" },
  { userText: user1, week: CTX.week, today: CTX.today },
  index,
);
check("accept_verbatim_piece", r.ok === true, r.ok ? "" : r.reason);

// 标点差异不算编造
r = mod.validateVerdict(
  { append: [{ text: "上杭教学, 参观紫金矿业", date: "2026-10-26" }], move: [], note: "" },
  { userText: "上杭教学，参观紫金矿业", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("accept_punct_diff", r.ok === true, r.ok ? "" : r.reason);

// 两个数组都空 = 只回一句话，是合法判定（确认、追问、回答）
r = mod.validateVerdict(
  { append: [], move: [], note: "就是周一上午，已确认。" },
  { userText: "就是周一上午", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("accept_reply_only", r.ok === true, r.ok ? "" : r.reason);
check("accept_reply_only_empty", r.ok && r.plan.append.length === 0 && r.plan.move.length === 0, "");

// 追加与移动可以同时出现在一份判定里（先挪后加由宿主保证顺序）
r = mod.validateVerdict(
  { append: [{ text: "参观了紫金矿业生产基地", date: "2026-10-26" }],
    move: [{ date: "2026-10-28", index: 1, to: "2026-10-26" }], note: "" },
  { userText: "参观了紫金矿业生产基地", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("accept_append_and_move", r.ok === true && r.plan.append.length === 1 && r.plan.move.length === 1, r.ok ? "" : r.reason);

// 跨周 / 未来日期
r = mod.validateVerdict(
  { append: [{ text: user1, date: "2026-10-01" }], move: [], note: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_cross_week", r.ok === false && r.reason.includes("不在"), r.reason);

r = mod.validateVerdict(
  { append: [{ text: user1, date: "2026-10-30" }], move: [], note: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_future", r.ok === false && r.reason.includes("晚于今天"), r.reason);

// 索引越界
r = mod.validateVerdict(
  { append: [], move: [{ date: "2026-10-28", index: 9, to: "2026-10-26" }], note: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_index_oob", r.ok === false && r.reason.includes("索引"), r.reason);

// 原日期==目标日期
r = mod.validateVerdict(
  { append: [], move: [{ date: "2026-10-28", index: 1, to: "2026-10-28" }], note: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_same_date", r.ok === false, r.reason);

// 同一位置移两次
r = mod.validateVerdict(
  { append: [], move: [
      { date: "2026-10-28", index: 1, to: "2026-10-26" },
      { date: "2026-10-28", index: 1, to: "2026-10-27" },
    ], note: "" },
  { userText: user1, week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_duplicate_move", r.ok === false && r.reason.includes("两次"), r.reason);

// 同一段话既追加又移动 → 作废（会写重）
r = mod.validateVerdict(
  { append: [{ text: "明天下午专题课《新时代党内法规制度建设》", date: "2026-10-26" }],
    move: [{ date: "2026-10-28", index: 0, to: "2026-10-26" }], note: "" },
  { userText: "明天下午专题课《新时代党内法规制度建设》", week: "2026-W44", today: "2026-10-28" },
  index,
);
check("reject_append_and_move_same", r.ok === false && r.reason.includes("既追加又移动"), r.reason);

// ---- 判定块解析 ----
const full = "已记录。" + "\n" + mod.VERDICT_OPEN + "\n" +
  JSON.stringify({ append: [{ text: "上杭教学和参观紫金矿业", date: "2026-10-26" }], move: [], note: "已记录" }) +
  "\n" + mod.VERDICT_CLOSE;
const split1 = mod.splitVerdict(full);
check("split_visible", split1.visible.trim() === "已记录。", JSON.stringify(split1.visible));
check("split_block", !!split1.block && split1.block.includes("append"), split1.block);
check("split_unclosed_hidden", mod.splitVerdict("已记录\n" + mod.VERDICT_OPEN + '{"append":[{"te').visible.trim() === "已记录", "");
check("display_no_block", !mod.verdictDisplayText(full).includes("omnidesk-verdict"), "");
check("parse_fenced", (() => {
  const fence = String.fromCharCode(96).repeat(3);
  const p = mod.parseVerdict(fence + "json\n" + JSON.stringify({ append: [], move: [], note: "x" }) + "\n" + fence);
  return p && p.note === "x" && p.append.length === 0;
})(), "");
check("parse_garbage_null", mod.parseVerdict("not json") === null, "");
check("parse_truncated_null", mod.parseVerdict('{"append":[{"text":"a"') === null, "");
check("parse_empty_null", mod.parseVerdict("") === null, "");
// 旧协议字段不再被接受：action 不参与判定，形状不对就作废
check("parse_legacy_action_null", (() => {
  const p = mod.parseVerdict('{"action":"answer","records":[],"moves":[],"reply":"x"}');
  return p !== null && p.append.length === 0 && p.move.length === 0 && p.note === "";
})(), "");
check("parse_bad_append_type_null", mod.parseVerdict('{"append":"x"}') === null, "");

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
    check("prompt_keeps_skill_injection", '"配套技能指引（" + s.id + "）："' in prompt)
    # OD-ARCH-105：平台提示词只给数据 + 写盘边界，判断口径全部来自 Skill。
    # 运行时的「注意点提示词里没有业务词」由 verify_worklog_089 的探针断言
    # （它会真正组装一次 note 提示词并去掉注入的 Skill 再比对）；这里只守住
    # 「平台不再自己描述判定协议」这一点——旧的四分类 schema 一旦回来，
    # 平台就又成了专用工具。
    check("prompt_no_verdict_schema", "omnidesk-verdict" not in prompt, "")
    check("prompt_no_action_enum", "record|move|confirm|answer" not in prompt, "")
    check("prompt_states_write_boundary", "写入由工作台执行" in prompt, "")
    check("prompt_no_skill_no_guess", "不要臆测判断口径" in prompt, "")

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
    check("skill_verdict_format", "<omnidesk-verdict>" in skill and "append" in skill and "move" in skill)
    check("skill_no_legacy_action", '"record|move|confirm|answer"' not in skill and "reply" not in skill, "")
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
