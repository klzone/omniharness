#!/usr/bin/env python3
"""OD-ARCH-089：工作记录意图判定 + 随手记确定性落盘 + Skill 拆分。

证据等级：
  * 静态代码检查：源码里确实没有手动开关、命令确实注册、配置确实改成三个绑定；
  * 脚本运行：编译真实的 desktop/src/workIntent.ts 与 conversationPrompt.ts 执行断言。

**不**调用模型、不开窗口；真实落盘的行为在 cargo test worklog 里覆盖。

用法：python3 tests/verification/desktop/verify_worklog_089.py [--json PATH]
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
RULES = REPO / "samples" / "profiles" / "discipline.rules.json"
SKILLS = REPO / "skills"


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


INTENT_PROBE = r"""
const mod = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

// 1) 明确要周报 → report
const reportInputs = [
  "生成周报",
  "帮我写个周报",
  "整理成周报",
  "总结本周工作",
  "本周工作汇总一下",
  "这周小结",
  "把上周周报更新一下",
];
for (const text of reportInputs) {
  const got = mod.detectWorkIntent(text);
  check("report:" + text, got === "report", got);
}

// 2) 其余一律随手记 → note（保守口径：宁可多记一条，也不误进周报流程）
const noteInputs = [
  "下午在古田干部学院听课，题目是古田会议是党和军队建设史上的里程碑",
  "今天开了个会",
  "记一下：周三交材料",
  "总结一下这次会议",
  "本周三下午对接了系统方",
  "整理一下这个思路",
  "",
];
for (const text of noteInputs) {
  const got = mod.detectWorkIntent(text);
  check("note:" + (text || "<empty>"), got === "note", got);
}

// 3) 归一化：全角标点、空格不影响判定
check("fullwidth_and_spaces", mod.detectWorkIntent("  生成　周报  ") === "report",
  mod.detectWorkIntent("  生成　周报  "));
check("mixed_punctuation", mod.detectWorkIntent("生成，周报！") === "report",
  mod.detectWorkIntent("生成，周报！"));

// 4) 明示文案
check("describe_report", mod.describeWorkIntent("report") === "按周报生成",
  mod.describeWorkIntent("report"));
check("describe_note", mod.describeWorkIntent("note") === "按随手记记录",
  mod.describeWorkIntent("note"));

console.log(JSON.stringify({ cases }));
"""

PROMPT_PROBE = r"""
const fs = require("fs");
const cp = require(process.argv[2]);
const rules = require(process.argv[3]);
const skillText = fs.readFileSync(process.argv[4], "utf8");
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

const scene = rules.scenes.weekly;
const preset = {
  key: "weekly",
  label: "工作记录",
  rules: scene.rules,
  sections: scene.sections,
  writingRules: rules.global.writing_rules,
  skills: [{ id: "memory-agent", content: skillText }],
};
const turns = [{ role: "user", text: "上一轮说了点什么" }];
const current = "下午在古田干部学院听课";

const note = cp.composeConversationPrompt({
  preset, turns, current, mode: "note",
  noteIndex: {
    week: "2026-W44",
    today: "2026-10-28",
    entries: "- 2026-10-26 #0 周一上午赴上杭县城现场教学",
  },
});
const reportSections = scene.sections.split("\n").filter((s) => s.trim());

// OD-ARCH-105：平台提示词只带数据与写盘边界；业务判断（分类、六要素、JSON 形状）
// 必须来自注入的 Skill，而不是写死在平台里。
check("note_has_six_elements", ["时间", "地点", "人物", "事件", "影响", "感受"].every((k) => note.includes(k)), "");
check("note_six_elements_from_skill", note.includes(skillText.trim()), "");
check("note_carries_skill_id", note.includes("memory-agent"), "");
check("note_has_index_data", note.includes("当周已有记录"), "");
for (const word of ["判断原则", "六要素", "逐字取自用户原话", "omnidesk-verdict", "append", "moves", "「本周报引用」"]) {
  check("platform_owns_no_rule:" + word, !note.replace(skillText.trim(), "").includes(word), word);
}
check("note_states_write_boundary", note.includes("写入由工作台执行"), "");
check("note_has_no_report_sections",
  reportSections.every((s) => !note.includes(s)), reportSections.join("/"));
check("note_has_no_writing_rules",
  rules.global.writing_rules.every((r) => !note.includes(r)), "");
check("note_forbids_rewriting", note.includes("不允许再写文件"), "");

// 没绑定 Skill 的场景：不臆测口径，只说没有可用指引
const bare = cp.composeConversationPrompt({
  preset: { ...preset, skills: null }, turns, current, mode: "note",
});
check("note_no_skill_no_guess", bare.includes("不要臆测判断口径"), "");
check("note_no_skill_no_six_elements",
  ["时间", "地点", "人物", "事件", "影响", "感受"].every((k) => !bare.includes(k)), "");

// 周报模式：带当周记录
const reportWithLog = cp.composeConversationPrompt({
  preset, turns, current: "帮我生成周报", mode: "report",
  worklog: { week: "2026-W44", exists: true, content: "SYNTH-ENTRY-A" },
});
check("report_includes_worklog_content", reportWithLog.includes("SYNTH-ENTRY-A"), "");
check("report_labels_week", reportWithLog.includes("2026-W44"), "");
check("report_has_sections", reportWithLog.includes(reportSections[0]), "");

// 周报模式：当周没有记录，如实说明，不得虚构
const reportEmpty = cp.composeConversationPrompt({
  preset, turns, current: "帮我生成周报", mode: "report",
  worklog: { week: "2026-W44", exists: false, content: "" },
});
check("report_says_no_record", reportEmpty.includes("本周还没有记录"), "");

// 非周记录场景：不传 worklog 时行为与原来一致（不含「本周工作记录」段）
const plain = cp.composeConversationPrompt({ preset, turns, current, mode: "report" });
check("no_worklog_no_section", !plain.includes("本周工作记录（"), "");

// 长度账（OD-ARCH-105）：平台自己写的那部分必须足够短——它只是数据 + 边界 + Skill 原文。
// 这里用「去掉注入的 Skill 原文」后的字数衡量平台负担，并守住一个很紧的上限。
const platformOnly = note.replace(skillText.trim(), "");
check("platform_prompt_is_thin", platformOnly.length <= 700, platformOnly.length + " 字符（不含 Skill）");
check("note_prompt_within_budget", note.length <= 2500,
  note.length + " vs 上限 2500（周报 " + reportWithLog.length + "）");

console.log(JSON.stringify({ cases, sizes: { note: note.length, platform: platformOnly.length, report: reportWithLog.length } }));
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": str(detail)})

    # ---- 静态：界面不再有两个标签，接线走意图判定 ----
    home_chat = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    check("no_note_mode_state", "noteMode" not in home_chat, "")
    check("no_note_button_label", '"随手记"' not in home_chat, "")
    check("no_report_button_label", '"整理成周报"' not in home_chat, "")
    check("uses_detect_work_intent", "detectWorkIntent(" in home_chat, "")
    check("appends_before_model", "shell.worklogAppend(" in home_chat, "")
    check("reads_week_for_report", "shell.worklogReadWeek(" in home_chat, "")
    check("passes_worklog_into_prompt", "worklog," in home_chat, "")

    # OD-ARCH-105：判断口径只在 Skill，平台提示词不带业务规则
    conv = (SRC / "conversationPrompt.ts").read_text(encoding="utf-8")
    # 只查随手记分支 + 通用断言：report 分支里的去向标签属于「读取哪些记录」
    # 的数据筛选，仍是已知残留（见 REVIEW），不在本轮范围。
    for word in ("判断原则", "六要素", "omnidesk-verdict"):
        check(f"platform_has_no_rule:{word}", word not in conv, word)
    check("platform_no_note_verdict_schema", "append" not in conv, "")

    api = (SRC / "api.ts").read_text(encoding="utf-8")
    check("api_worklog_append", '"worklog_append"' in api, "")
    check("api_worklog_read_week", '"worklog_read_week"' in api, "")

    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    check("host_registers_append", "worklog_append," in lib, "")
    check("host_registers_read", "worklog_read_week," in lib, "")
    check("host_scans_redline_before_write",
          "red_line_scan_with(&text, &policy)" in lib, "")

    worklog_rs = (DESKTOP / "src-tauri" / "src" / "worklog.rs").read_text(encoding="utf-8")
    check("append_only_open_options", ".append(true)" in worklog_rs, "")
    check("never_truncates", "truncate(true)" not in worklog_rs, "")
    check("rejects_path_traversal", "assert_week_label" in worklog_rs, "")
    check("verifies_prefix_unchanged", "starts_with(before.as_str())" in worklog_rs, "")

    # ---- 静态：Skill 拆分与绑定 ----
    ma = (SKILLS / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")
    mr = (SKILLS / "memory-report" / "SKILL.md").read_text(encoding="utf-8")
    report_sections = ("本周概述", "事项进展", "问题与风险", "下步计划", "待确认事项")
    check("ma_has_six_elements",
          all(k in ma for k in ("时间", "地点", "人物", "事件", "影响", "感受")), "")
    check("ma_has_no_report_sections", not any(s in ma for s in report_sections), "")
    check("ma_points_to_report_skill", "memory-report" in ma, "")
    check("mr_has_five_sections", all(s in mr for s in report_sections), "")
    check("mr_confirm_gate", "只有用户明确确认后才落盘" in mr, "")
    check("mr_no_overwrite", "不覆盖" in mr, "")
    for name, text in (("memory-agent", ma), ("memory-report", mr)):
        check(f"{name}_within_budget", len(text) <= 2500, f"{len(text)} 字符")

    rules = json.loads(RULES.read_text(encoding="utf-8"))
    weekly = rules["skill_bindings"]["weekly"]
    check("weekly_three_bindings",
          weekly["skills"] == ["memory-agent", "memory-report", "knowledge-search"],
          str(weekly["skills"]))
    check("weekly_max_injected_unchanged", weekly["max_injected"] == 3, str(weekly["max_injected"]))
    check("anysearch_not_preinjected", "anysearch" not in weekly["skills"], str(weekly["skills"]))

    profile_rules = (DESKTOP / "src-tauri" / "src" / "profile_rules.rs").read_text(encoding="utf-8")
    check("allowlist_has_memory_report", '"memory-report"' in profile_rules, "")
    check("allowlist_has_anysearch", '"anysearch"' in profile_rules, "")

    # ---- 运行：意图判定 ----
    tmp = Path(tempfile.mkdtemp(prefix="od089-"))
    compiled = compile_ts([SRC / "workIntent.ts"], tmp / "intent")
    if compiled.returncode != 0:
        check("workIntent_compiles", False, compiled.stderr[-1200:])
    else:
        check("workIntent_compiles", True, "")
        probe = tmp / "intent-probe.js"
        probe.write_text(INTENT_PROBE, encoding="utf-8")
        proc = run(["node", str(probe), str(tmp / "intent" / "workIntent.js")], DESKTOP)
        if proc.returncode != 0:
            check("workIntent_probe_exit", False, proc.stderr[-1200:])
        else:
            for c in json.loads(proc.stdout.strip().splitlines()[-1])["cases"]:
                cases.append(c)

    # ---- 运行：提示词组装 ----
    compiled2 = compile_ts([SRC / "conversationPrompt.ts"], tmp / "cp")
    if compiled2.returncode != 0:
        check("conversationPrompt_compiles", False, compiled2.stderr[-1200:])
    else:
        check("conversationPrompt_compiles", True, "")
        probe2 = tmp / "cp-probe.js"
        probe2.write_text(PROMPT_PROBE, encoding="utf-8")
        proc2 = run(
            [
                "node", str(probe2), str(tmp / "cp" / "conversationPrompt.js"), str(RULES),
                str(SKILLS / "memory-agent" / "SKILL.md"),
            ],
            DESKTOP,
        )
        if proc2.returncode != 0:
            check("conversationPrompt_probe_exit", False, proc2.stderr[-1200:])
        else:
            payload = json.loads(proc2.stdout.strip().splitlines()[-1])
            cases.extend(payload["cases"])
            sizes = payload.get("sizes", {})
            check("note_prompt_measured", sizes.get("note", 0) > 0, json.dumps(sizes))

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[WORKLOG-089] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases) - len(failed)} failed={len(failed)}")

    if args.json:
        Path(args.json).write_text(
            json.dumps({"cases": cases, "failed": len(failed)}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

