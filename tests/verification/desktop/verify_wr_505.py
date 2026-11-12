#!/usr/bin/env python3
"""OD-WR-505 周期复盘：意图词表与宿主/前端契约的离线验证。

证据等级：
  * 脚本运行：编译真实 workIntent.ts，验证周期词表命中、周报不被吞、普通记录不误判；
  * 静态源码检查：宿主 schema/汇集/确认/已确认不覆盖、Skill 深度模板、IPC 与前端接线。

真实落盘、备份、失效引用与确认流由 cargo test review_store 覆盖（16 项）。

用法：python3 tests/verification/desktop/verify_wr_505.py [--json PATH]
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
SKILL = REPO / "skills" / "period-review" / "SKILL.md"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


PROBE = r"""
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

const periodInputs = [
  "月度总结", "月报", "季度复盘", "季度总结", "年度总结", "年度复盘",
  "复盘本月", "本月复盘", "复盘本季", "复盘今年", "周复盘", "本周复盘",
];
for (const text of periodInputs) {
  const got = mod.detectWorkIntent(text);
  check("period:" + text, got === "period-review", got);
}

const reportInputs = ["生成周报", "总结本周工作", "今天顺便把周报写一下", "周报复盘"];
for (const text of reportInputs) {
  const got = mod.detectWorkIntent(text);
  check("report_not_swallowed:" + text, got === "report", got);
}

const noteInputs = [
  "总结一下这次会议", "今天的会议纪要", "当日工作总结", "复盘一下这个方案", "",
];
for (const text of noteInputs) {
  const got = mod.detectWorkIntent(text);
  check("note:" + (text || "<empty>"), got === "note", got);
}

const now = new Date(2026, 10, 11, 12, 0, 0);
const monthly = mod.detectPeriodReviewTarget("复盘本月", now);
check("target_monthly", monthly && monthly.period === "monthly" && monthly.start === "2026-11-01" && monthly.end === "2026-11-30", JSON.stringify(monthly));
const quarterly = mod.detectPeriodReviewTarget("复盘本季", now);
check("target_quarterly", quarterly && quarterly.start === "2026-10-01" && quarterly.end === "2026-12-31", JSON.stringify(quarterly));
const annual = mod.detectPeriodReviewTarget("年度总结", now);
check("target_annual", annual && annual.start === "2026-01-01" && annual.end === "2026-12-31", JSON.stringify(annual));
const weekly = mod.detectPeriodReviewTarget("本周复盘", now);
check("target_weekly", weekly && weekly.start === "2026-11-09" && weekly.end === "2026-11-15", JSON.stringify(weekly));

console.log(JSON.stringify({ cases }));
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    skill = SKILL.read_text(encoding="utf-8") if SKILL.is_file() else ""
    check("skill_exists", SKILL.is_file())
    for title in [
        "本周期工作概况", "重点工作事项", "工作成果与完成情况", "数据汇总",
        "趋势对照", "跨月对照", "主题归纳", "全年脉络", "长期建议",
    ]:
        check("skill_section:" + title, title in skill)
    check("skill_real_ids", "#wr-" in skill and "#t-" in skill)
    check("skill_no_fabrication", "不得虚构" in skill and "不补造" in skill)

    host = (TAURI / "review_store.rs").read_text(encoding="utf-8")
    check("host_schema", "omnidesk.period-review.v1" in host)
    check("host_collect", "pub fn collect_period" in host)
    check("host_confirm", "pub fn confirm_review" in host)
    check("host_filters_draft", "draft: Some(false)" in host)
    check("host_backup_bak", ".bak" in host and "write_replace_atomically" in host)
    check("host_rejects_unknown_record", "不存在的记录编号" in host)
    check("host_rejects_unknown_task", "不存在的任务编号" in host)
    check(
        "host_confirmed_never_overwritten",
        "已确认，不允许重新生成覆盖" in host
        and "fn confirmed_review_is_never_overwritten" in host,
    )
    check("host_confirm_timestamp", "confirmed_at" in host)
    check("host_depth_weekly", "WEEKLY_SECTIONS: [&str; 7]" in host)
    check("host_depth_monthly", "MONTHLY_SECTIONS: [&str; 9]" in host)
    check("host_depth_quarterly", "QUARTERLY_SECTIONS: [&str; 11]" in host)
    check("host_depth_annual", "ANNUAL_SECTIONS: [&str; 13]" in host)
    check("host_tests_12plus", host.count("#[test]") >= 12, str(host.count("#[test]")))

    lib = (TAURI / "lib.rs").read_text(encoding="utf-8")
    check("module_registered", "pub mod review_store;" in lib)
    for name in (
        "worklog_period_review_collect",
        "worklog_period_review_generate",
        "worklog_period_review_read",
        "worklog_period_review_confirm",
    ):
        check("ipc:" + name, name in lib and name + "," in lib)

    prompt = (SRC / "conversationPrompt.ts").read_text(encoding="utf-8")
    check("prompt_mode", '"period-review"' in prompt and "periodReview" in prompt)
    check("prompt_depth", "PERIOD_REVIEW_SECTIONS" in prompt and "全年脉络" in prompt)

    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    check("home_collect", "worklogPeriodReviewCollect" in home)
    check("home_generate", "worklogPeriodReviewGenerate" in home)
    check("home_regenerate_dialog", "period-review-regenerate-dialog" in home)
    check("home_confirmed_reject", "已确认，不能重新生成覆盖" in home)

    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    check("api_generate", "worklogPeriodReviewGenerate" in api)
    check("api_confirm", "worklogPeriodReviewConfirm" in api)
    check("types_receipt", "PeriodReviewReceipt" in types)
    check("types_read", "PeriodReviewRead" in types)

    rules = (REPO / "samples" / "profiles" / "discipline.rules.json").read_text(encoding="utf-8")
    check("rules_binding", '"weekly-period-review"' in rules and '"period-review"' in rules)
    check("rules_weekly_report_kept", '"weekly-report"' in rules and '"memory-report"' in rules)

    seed = (TAURI / "profile_seed.rs").read_text(encoding="utf-8")
    check("seed_skill", 'name: "period-review"' in seed)

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "intent"
        compiled = run(
            [
                "npx", "--no-install", "tsc", "--ignoreConfig", str(SRC / "workIntent.ts"),
                "--outDir", str(out), "--module", "commonjs", "--target", "es2020",
            ],
            DESKTOP,
        )
        if compiled.returncode != 0:
            check("intent_compile", False, compiled.stderr[-1200:])
        else:
            check("intent_compile", True)
            probe = Path(tmp) / "probe.js"
            probe.write_text(PROBE, encoding="utf-8")
            proc = run(["node", str(probe), str(out / "workIntent.js")], DESKTOP)
            if proc.returncode != 0:
                check("intent_probe", False, proc.stderr[-1200:])
            else:
                check("intent_probe", True)
                payload = json.loads(proc.stdout)
                cases.extend(payload["cases"])

    failed = [case for case in cases if not case["ok"]]
    result = {
        "plan": "OD-WR-505",
        "evidence_level": "脚本运行 + 静态源码检查",
        "passed": not failed,
        "total": len(cases),
        "passed_count": len(cases) - len(failed),
        "failed": failed,
        "cases": cases,
    }
    for case in cases:
        print(("PASS  " if case["ok"] else "FAIL  ") + case["name"])
    print(f"\n{len(cases) - len(failed)}/{len(cases)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
