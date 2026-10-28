#!/usr/bin/env python3
"""OD-ARCH-103：随手记补录日期语义化。

证据等级：
  * 脚本运行：编译真实的 desktop/src/workIntent.ts，用固定「今天」跑日期识别与纠正判定；
  * 静态代码检查：宿主命令注册、前端接线、Skill 规则、失败回执形状。

真实落盘行为（改日期搬块、备份、CRLF）由 cargo test worklog 覆盖。

用法：python3 tests/verification/desktop/verify_worklog_103.py [--json PATH]
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


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


DATE_PROBE = r"""
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}
// 宿主与前端统一用东八区（offsetMinutes = 480），按 UTC 时刻构造「今天」。
const at = (iso) => new Date(iso);
const WED = at("2026-10-28T02:00:00Z");   // 本地 2026-10-28 周三，ISO 2026-W44
const SUN = at("2026-11-01T02:00:00Z");   // 本地 2026-11-01 周日，仍属 2026-W44
const MON = at("2026-10-26T02:00:00Z");   // 本地 2026-10-26 周一
const guess = (text, now) => mod.extractEntryDate(text, now, 480);

// 1) 用户原始场景：周三补录周一上午的课
const monday = guess("今天下午在福建古田干部学院听课，题目是《古田会议是党和军队建设史上的里程碑》".replace("今天", "周一"), WED);
check("wed_monday_falls_back", monday && monday.date === "2026-10-26", monday && monday.date);
check("wed_monday_weekday_label", monday && monday.weekday === "周一", monday && monday.weekday);
check("wed_monday_source", monday && monday.source === "weekday", monday && monday.source);

// 2) 未来日期不猜
check("future_weekday_is_null", guess("周五交材料", WED) === null, "周五");
// 3) 上周/下周不认
check("last_week_is_null", guess("上周一开的会", WED) === null, "上周一");
check("next_week_is_null", guess("下周二再谈", WED) === null, "下周二");
// 4) 周日说「周日」→ 当天；「昨天」仍在本周
const sunday = guess("周日下午整理材料", SUN);
check("sunday_is_today", sunday && sunday.date === "2026-11-01", sunday && sunday.date);
const sat = guess("昨天下午开了个会", SUN);
check("sunday_yesterday_same_week", sat && sat.date === "2026-10-31", sat && sat.date);
// 5) 跨周的相对日不猜
check("monday_yesterday_cross_week", guess("昨天下午开了个会", MON) === null, "昨天跨到 W43");
// 6) 明确日期优先于星期
const explicit = guess("10月27日（周二）对接系统方", WED);
check("explicit_date_wins", explicit && explicit.date === "2026-10-27", explicit && explicit.date);
check("explicit_dashed", (guess("2026-10-26 周一的课", WED) || {}).date === "2026-10-26", "");
// 7) 无日期词 → null（回落当天）
check("no_date_is_null", guess("下午跟系统方对了需求", WED) === null, "");

// 8) 纠正口令
const r1 = mod.detectRedateInstruction("记错了，应该是周一", WED, 480);
check("correction_detected", r1 && r1.date === "2026-10-26", r1 && r1.date);
const r2 = mod.detectRedateInstruction("把上一条挪到周二", WED, 480);
check("correction_move_phrase", r2 && r2.date === "2026-10-27", r2 && r2.date);
check("correction_ignores_plain_note",
  mod.detectRedateInstruction("会议改成周五了", WED, 480) === null, "真实记录不能被当成改日期");
check("correction_ignores_long_text",
  mod.detectRedateInstruction("记错了，应该是周一，那天上午我在古田干部学院听了两节课，下午还讨论了材料", WED, 480) === null, "");
check("correction_needs_known_date",
  mod.detectRedateInstruction("记错了", WED, 480) === null, "");

// 9) 细节补充识别
check("continuation_true", mod.isNoteContinuation("补充两条：一是上午听课") === true, "");
check("continuation_false", mod.isNoteContinuation("另外我下午还开了个会") === false, "");

// 10) 周键与今天
check("iso_week_monday", mod.isoWeekKeyOf("2026-10-26") === "2026-W44", mod.isoWeekKeyOf("2026-10-26"));
check("iso_week_sunday", mod.isoWeekKeyOf("2026-11-01") === "2026-W44", mod.isoWeekKeyOf("2026-11-01"));
check("iso_week_next", mod.isoWeekKeyOf("2026-11-02") === "2026-W45", mod.isoWeekKeyOf("2026-11-02"));
check("today_key", mod.todayDateKey(WED, 480) === "2026-10-28", mod.todayDateKey(WED, 480));
check("date_phrase_found", mod.findDatePhrase("周五交材料") === "周五", mod.findDatePhrase("周五交材料"));
check("date_phrase_absent", mod.findDatePhrase("下午对了需求") === null, "");

console.log(JSON.stringify({ cases }));
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": str(detail)})

    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    worklog = (DESKTOP / "src-tauri" / "src" / "worklog.rs").read_text(encoding="utf-8")
    intent = (SRC / "workIntent.ts").read_text(encoding="utf-8")
    skill = (SKILLS / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")

    # ---- 静态：前端接线 ----
    #
    # OD-ARCH-104 起，随手记的「这句是指令还是内容」改由模型判定，界面不再用
    # detectRedateInstruction / isNoteContinuation 做前置正则。以下几条被 104 取代，
    # 改由 verify_worklog_104.py 断言（「模型判定到达之前不写盘」「只走批量移动」）；
    # 这里只保留仍然成立的部分：日期与文本工具还在、本机回执标签还在。
    check("home_keeps_find_date_phrase", "findDatePhrase(" in home)
    check("home_keeps_host_timezone", "utc_offset_minutes" in home)
    # 单条改日期的命令仍在（宿主能力保留）；界面改由 104 的批量通道统一执行，
    # 这条断言落在 API 层，不再要求 HomeChat 直接调用。
    check("api_keeps_redate_command", "worklogRedate(" in api and '"worklog_redate"' in api)
    check("home_marks_local_receipt", '"工作台本地处理 · 未调用模型"' in home)
    check("api_entry_date_param", "entryDate: entryDate ?? null" in api)
    check("api_redate_command", '"worklog_redate"' in api)
    check("types_append_index", "entry_index: number;" in types)
    check("types_redate_receipt", "export interface WorklogRedateReceipt" in types)

    # ---- 静态：宿主 ----
    check("lib_redate_command", "fn worklog_redate(" in lib)
    check("lib_registers_redate", "worklog_redate," in lib)
    check("lib_validates_entry_date", "worklog::assert_entry_date(&week, day, &today)" in lib)
    check("lib_append_failures_have_shape", lib.count('"entry_index": 0,') >= 5)
    check("worklog_assert_entry_date", "pub fn assert_entry_date" in worklog)
    check("worklog_move_entry", "pub fn move_entry(" in worklog)
    # 改日期走 move_entries 共用的一套写盘：备份命名 + 只增删允许的日期标题。
    check(
        "worklog_redate_backs_up",
        ".md.bak-" in worklog
        and 'format!("## {date}")' in worklog
        and 'format!("## {date}（续）")' in worklog,
    )
    check("worklog_shared_commit", "fn commit_week_file(" in worklog)
    check("worklog_conserves_items", "before_items != after_items" in worklog)
    check("worklog_no_truncate", "truncate(true)" not in worklog)
    check("worklog_rejects_cross_week", "跨周移动暂不支持" in worklog)
    check("intent_export_extract", "export function extractEntryDate(" in intent)
    check("intent_export_redate", "export function detectRedateInstruction(" in intent)
    check("intent_rejects_backreference_less", "backReference" in intent)

    # ---- 静态：Skill 规则 ----
    # OD-ARCH-104：日期不再由工作台从原文里「认出来」，改由模型判定 + 索引校验；
    # Skill 同步改成「先分类、再输出判定块」。
    check("skill_date_judged_by_model", "你判语义" in skill and "工作台落盘" in skill)
    check("skill_no_file_editing", "没有写文件的权限" in skill)
    check("skill_no_invented_tool", "worklog_redate" not in skill)
    check("skill_within_budget", len(skill) <= 2500, f"{len(skill)} 字符")

    # ---- 运行：日期识别与纠正判定 ----
    tmp = Path(tempfile.mkdtemp(prefix="od103-"))
    compiled = run(
        [
            "npx", "--no-install", "tsc", str(SRC / "workIntent.ts"),
            "--outDir", str(tmp / "intent"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    if compiled.returncode != 0:
        check("workIntent_compiles", False, compiled.stderr[-1200:])
    else:
        check("workIntent_compiles", True)
        probe = tmp / "probe.js"
        probe.write_text(DATE_PROBE, encoding="utf-8")
        proc = run(["node", str(probe), str(tmp / "intent" / "workIntent.js")], DESKTOP)
        if proc.returncode != 0:
            check("date_probe_exit", False, proc.stderr[-1200:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WORKLOG-103] " + c["name"] +
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

