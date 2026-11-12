#!/usr/bin/env python3
"""OD-WR-503 当日摘要：意图词表、前端接线与宿主契约的离线验证。

证据等级：
  * 脚本运行：编译真实的 desktop/src/workIntent.ts，用固定输入跑词表命中/不误判；
  * 静态代码检查：Skill 五段模板、宿主 schema/.bak/引用校验、IPC 与前端入口。

真实落盘、备份与失效引用由 cargo test daily_summary 覆盖（12 项）。

用法：python3 tests/verification/desktop/verify_wr_503.py [--json PATH]
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
TAURI = DESKTOP / "src-tauri" / "src"
SKILL = REPO / "skills" / "daily-summary" / "SKILL.md"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


INTENT_PROBE = r"""
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

const summaryInputs = [
  "生成今日摘要",
  "今日摘要",
  "总结今天",
  "今天的摘要",
  "当日摘要",
  "帮我生成今天的摘要",
  "  生成　今日摘要  ",
  "总结今天的工作",
];
for (const text of summaryInputs) {
  const got = mod.detectWorkIntent(text);
  check("summary:" + text, got === "daily-summary", got);
}

const reportInputs = ["生成周报", "总结本周工作", "今天顺便把周报写一下"];
for (const text of reportInputs) {
  const got = mod.detectWorkIntent(text);
  check("report:" + text, got === "report", got);
}

const noteInputs = [
  "总结一下这次会议",
  "今天开了个会",
  "今天的会议纪要",
  "当日工作总结",
  "记一下：周三交材料",
  "",
];
for (const text of noteInputs) {
  const got = mod.detectWorkIntent(text);
  check("note:" + (text || "<empty>"), got === "note", got);
}

check("describe_summary", mod.describeWorkIntent("daily-summary") === "按当日摘要生成",
  mod.describeWorkIntent("daily-summary"));

console.log(JSON.stringify({ cases }));
"""


def check_static(cases: list[dict], name: str, ok: bool, detail: str = "") -> None:
    cases.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    cases: list[dict] = []

    skill = SKILL.read_text(encoding="utf-8") if SKILL.is_file() else ""
    for title in ["今日工作概况", "重点事项", "重要思考", "需要跟进的问题", "与任务管理的关联"]:
        check_static(cases, "skill_section:" + title, title in skill)
    check_static(cases, "skill_requires_real_ids", "#记录编号" in skill and "只引用材料里出现过的编号" in skill)
    check_static(cases, "skill_no_fabrication", "不得虚构" in skill or "不得补造" in skill)

    intent_src = (SRC / "workIntent.ts").read_text(encoding="utf-8")
    for word in ["生成今日摘要", "今日摘要", "总结今天", "今天的摘要", "当日摘要"]:
        check_static(cases, "keyword_closed:" + word, '"' + word + '"' in intent_src)
    check_static(cases, "intent_three_values", 'export type WorkIntent = "report" | "note" | "daily-summary"' in intent_src)

    host = (TAURI / "daily_summary.rs").read_text(encoding="utf-8")
    check_static(cases, "host_schema", "omnidesk.daily-summary.v1" in host)
    check_static(cases, "host_collect_today", "pub fn collect_today" in host)
    check_static(cases, "host_filters_draft", "draft: Some(false)" in host)
    check_static(cases, "host_backup_bak", ".bak" in host and "write_replace_atomically" in host)
    check_static(cases, "host_rejects_unknown_ids", "不存在的记录编号" in host)
    check_static(cases, "host_requires_sections", "摘要缺少段落" in host)
    check_static(cases, "host_honest_empty", "当日无记录" in host)
    check_static(cases, "host_six_tests_plus", host.count("#[test]") >= 8, str(host.count("#[test]")))

    lib = (TAURI / "lib.rs").read_text(encoding="utf-8")
    check_static(cases, "module_registered", "pub mod daily_summary;" in lib)
    check_static(cases, "ipc_generate", "worklog_daily_summary_generate" in lib)
    check_static(cases, "ipc_read", "worklog_daily_summary_read" in lib)
    check_static(cases, "ipc_generate_in_handler", "worklog_daily_summary_generate," in lib)

    prompt = (SRC / "conversationPrompt.ts").read_text(encoding="utf-8")
    check_static(cases, "prompt_daily_summary_mode", '"daily-summary"' in prompt and "dailySummary" in prompt)
    check_static(cases, "prompt_five_sections", "今日工作概况" in prompt and "与任务管理的关联" in prompt)

    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    check_static(cases, "home_reads_records", "buildDailySummaryMaterial" in home)
    check_static(cases, "home_generate_ipc", "worklogDailySummaryGenerate" in home)
    check_static(cases, "home_regenerate_confirm", "daily-summary-regenerate-dialog" in home)

    library = (SRC / "shell" / "WorkRecordLibrary.tsx").read_text(encoding="utf-8")
    check_static(cases, "library_generate_entry", 'data-daily-summary-generate' in library)
    check_static(cases, "library_view_entry", 'data-daily-summary-view' in library)
    check_static(cases, "library_record_links", 'data-daily-summary-record' in library)

    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    check_static(cases, "api_generate", "worklogDailySummaryGenerate" in api)
    check_static(cases, "api_read", "worklogDailySummaryRead" in api)
    check_static(cases, "types_receipt", "DailySummaryReceipt" in types)
    check_static(cases, "types_read", "DailySummaryRead" in types)

    rules = (REPO / "samples" / "profiles" / "discipline.rules.json").read_text(encoding="utf-8")
    check_static(cases, "rules_binding", '"weekly-daily-summary"' in rules and '"daily-summary"' in rules)

    compiled_ok = True
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "intent"
        compiled = run(
            ["npx", "--no-install", "tsc", "--ignoreConfig", str(SRC / "workIntent.ts"), "--outDir", str(out),
             "--module", "commonjs", "--target", "es2020"],
            DESKTOP,
        )
        if compiled.returncode != 0:
            compiled_ok = False
            check_static(cases, "workIntent_compiles", False, compiled.stderr[-1200:])
        else:
            check_static(cases, "workIntent_compiles", True)
            probe = Path(tmp) / "probe.js"
            probe.write_text(INTENT_PROBE, encoding="utf-8")
            proc = run(["node", str(probe), str(out / "workIntent.js")], DESKTOP)
            if proc.returncode != 0:
                check_static(cases, "intent_probe_exit", False, proc.stderr[-1200:])
            else:
                check_static(cases, "intent_probe_exit", True)
                payload = json.loads(proc.stdout)
                cases.extend(payload["cases"])

    passed = sum(1 for c in cases if c["ok"])
    failed = [c for c in cases if not c["ok"]]
    print(json.dumps({"total": len(cases), "passed": passed, "failed": failed,
                      "cases": cases}, ensure_ascii=False, indent=1))
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps({"total": len(cases), "passed": passed,
                                         "failed": failed, "cases": cases},
                                        ensure_ascii=False, indent=1), encoding="utf-8")
    return 0 if not failed and compiled_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
