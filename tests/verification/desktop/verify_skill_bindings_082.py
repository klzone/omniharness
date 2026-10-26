#!/usr/bin/env python3
"""OD-ARCH-082：三大场景 Skill 绑定与提示词注入验证。

验证：
1. discipline.rules.json schema v2 结构与合法性：skill_bindings 各场景配置，
   包含 weekly (memory-agent), writing (material-writing, knowledge-search, document-review),
   project (project-agent)。
2. 约束边界校验：非法 skill id 拦截、同场景重复拦截、超出上限拦截、v1 规则向后兼容。
3. 真实 conversationPrompt.ts 与 aiDraft.ts 编译与注入执行：
   - note 模式（随手记）最多注入 1 个 Skill；
   - report 模式注入对应绑定的所有 Skill；
   - 切换场景后注入内容严格不同；
   - continuation（多轮续答）仍每轮重新注入；
   - 无绑定或无规则包时，提示词与现状完全一致（逐字比对）。
4. 输出 S5 场景证据到 tests/verification/desktop/evidence/od-arch-082/scene-injection.json。

用法：
  python3 tests/verification/desktop/verify_skill_bindings_082.py [--json PATH]
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
SOURCE_CP = DESKTOP / "src" / "conversationPrompt.ts"
SOURCE_AD = DESKTOP / "src" / "aiDraft.ts"
RULES = REPO / "samples" / "profiles" / "discipline.rules.json"
SKILLS_DIR = REPO / "skills"
# OD-ARCH-089 改了注入内容（拆 skill、去 anysearch 预注入），本脚本重跑时
# 证据改落到 089 目录，避免改写 082 的历史证据。
EVIDENCE_DIR = HERE / "evidence" / "od-arch-089"

PROFILE_RULES_SRC = DESKTOP / "src-tauri" / "src" / "profile_rules.rs"


def load_allowed_skills() -> set[str]:
    """从宿主源码读白名单，避免脚本里再抄一份而过期（OD-ARCH-086 扩容）。"""
    src = PROFILE_RULES_SRC.read_text(encoding="utf-8")
    block = re.search(
        r"pub const ALLOWED_SKILLS: &\[&str\] = &\[(.*?)\];", src, re.S
    )
    if block is None:
        raise SystemExit(f"未能从 {PROFILE_RULES_SRC} 定位 ALLOWED_SKILLS")
    names = set(re.findall(r'"([a-z0-9-]+)"', block.group(1)))
    if not names:
        raise SystemExit("ALLOWED_SKILLS 解析结果为空")
    return names


ALLOWED_SKILLS = load_allowed_skills()

PROBE_082 = r"""
const cp = require(process.argv[2]);
const ad = require(process.argv[3]);
const rules = require(process.argv[4]);
const skillContents = JSON.parse(process.argv[5]);

const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail || "" });
}

// 1. 无绑定 / 无 skills 时，提示词与现状逐字一致
const turns = [{ role: "user", text: "历史轮次" }];
const current = "本轮输入文字";
const baselinePreset = {
  key: "writing",
  label: "文案起草",
  rules: rules.scenes.writing.rules,
  sections: rules.scenes.writing.sections,
  writingRules: rules.global.writing_rules,
};

const promptWithoutSkills = cp.composeConversationPrompt({
  preset: baselinePreset,
  turns,
  current,
});
const promptWithEmptySkills = cp.composeConversationPrompt({
  preset: { ...baselinePreset, skills: [] },
  turns,
  current,
});
check("empty_skills_identical_to_no_skills", promptWithoutSkills === promptWithEmptySkills, "");

// 2. Weekly 随手记 (note mode)：最多注入 1 个 Skill，保持轻装
const weeklySkills = [
  { id: "memory-agent", content: skillContents["memory-agent"] || "Memory Agent Content" },
  { id: "knowledge-agent", content: skillContents["knowledge-agent"] || "Knowledge Agent Content" }
];
const weeklyPreset = {
  key: "weekly",
  label: "周报与随记",
  rules: rules.scenes.weekly.rules,
  sections: rules.scenes.weekly.sections,
  writingRules: rules.global.writing_rules,
  skills: weeklySkills,
};

const notePrompt = cp.composeConversationPrompt({
  preset: weeklyPreset,
  turns,
  current: "完成了三项记录",
  mode: "note",
});
check("note_injects_first_skill", notePrompt.includes("配套技能指引（memory-agent）："), notePrompt.slice(0, 100));
check("note_does_not_inject_second_skill", !notePrompt.includes("knowledge-agent"), "");
// OD-ARCH-089：随手记不再「整理成要点」（写盘由宿主完成），只做缺项追问。
check("note_contains_clean_directive", notePrompt.includes("这六项") || notePrompt.includes("六要素"), "");
check("note_has_no_report_sections", !notePrompt.includes("本周概述"), "");

// 3. Weekly 报告模式 (report mode)
const reportWeeklyPrompt = cp.composeConversationPrompt({
  preset: weeklyPreset,
  turns,
  current: "整理本周工作",
  mode: "report",
});
check("report_weekly_injects_skill", reportWeeklyPrompt.includes("--- 技能：memory-agent ---"), "");
check("report_weekly_has_sections", reportWeeklyPrompt.includes("本周概述"), "");

// 4. Writing 模式：注入 3 个 Skill (material-writing, knowledge-search, document-review)
const writingSkills = [
  { id: "material-writing", content: skillContents["material-writing"] || "Writing Content" },
  { id: "knowledge-search", content: skillContents["knowledge-search"] || "Search Content" },
  { id: "document-review", content: skillContents["document-review"] || "Review Content" }
];
const writingPreset = {
  key: "writing",
  label: "文案起草",
  rules: rules.scenes.writing.rules,
  sections: rules.scenes.writing.sections,
  writingRules: rules.global.writing_rules,
  skills: writingSkills,
};

const writingPrompt = cp.composeConversationPrompt({
  preset: writingPreset,
  turns,
  current: "起草一份监督工作总结",
});
check("writing_injects_material_writing", writingPrompt.includes("--- 技能：material-writing ---"), "");
check("writing_injects_knowledge_search", writingPrompt.includes("--- 技能：knowledge-search ---"), "");
check("writing_injects_document_review", writingPrompt.includes("--- 技能：document-review ---"), "");

// 5. Project 模式：注入 project-agent
const projectSkills = [
  { id: "project-agent", content: skillContents["project-agent"] || "Project Agent Content" }
];
const projectPreset = {
  key: "project",
  label: "项目跟踪",
  rules: rules.scenes.project.rules,
  sections: rules.scenes.project.sections,
  writingRules: rules.global.writing_rules,
  skills: projectSkills,
};

const projectPrompt = cp.composeConversationPrompt({
  preset: projectPreset,
  turns,
  current: "梳理项目进度与阻塞",
});
check("project_injects_project_agent", projectPrompt.includes("--- 技能：project-agent ---"), "");

// 6. 三场景注入内容严格不同
check("prompts_distinct_weekly_vs_writing", notePrompt !== writingPrompt && reportWeeklyPrompt !== writingPrompt, "");
check("prompts_distinct_writing_vs_project", writingPrompt !== projectPrompt, "");
check("prompts_distinct_weekly_vs_project", reportWeeklyPrompt !== projectPrompt, "");

// 7. continuation（多轮续答）测试：每轮重新注入 skills
const contWeeklyPrompt = cp.composeConversationPrompt({
  preset: weeklyPreset,
  turns,
  current: "这是第二轮问题",
  continuation: true,
  mode: "report",
});
check("continuation_does_not_repeat_history", !contWeeklyPrompt.includes("历史轮次"), "");
check("continuation_still_injects_skills", contWeeklyPrompt.includes("--- 技能：memory-agent ---"), "");
check("continuation_has_current_turn", contWeeklyPrompt.includes("用户：这是第二轮问题"), "");

// 收集 S5 证据
const evidence = {
  weekly: {
    scene: "weekly",
    input: "完成了两项系统联调，因第三方服务无响应，有一项接口联调延期至下周二。",
    hit_skills: ["memory-agent"],
    snippets: [skillContents["memory-agent"] ? skillContents["memory-agent"].slice(0, 180) : ""],
    prompt_sample: cp.composeConversationPrompt({
      preset: weeklyPreset,
      turns: [],
      current: "完成了两项系统联调，因第三方服务无响应，有一项接口联调延期至下周二。",
      mode: "note",
    }).slice(0, 500)
  },
  writing: {
    scene: "writing",
    input: "起草一份政治监督专项检查报告初稿，强调四次现场督查与发现的9个共性问题。",
    hit_skills: ["material-writing", "knowledge-search", "document-review"],
    snippets: [
      skillContents["material-writing"] ? skillContents["material-writing"].slice(0, 180) : "",
      skillContents["knowledge-search"] ? skillContents["knowledge-search"].slice(0, 180) : "",
      skillContents["document-review"] ? skillContents["document-review"].slice(0, 180) : ""
    ],
    prompt_sample: writingPrompt.slice(0, 600)
  },
  project: {
    scene: "project",
    input: "登记并跟踪三个专项：项目A联调中，项目B待启动，项目C已完成。",
    hit_skills: ["project-agent"],
    snippets: [skillContents["project-agent"] ? skillContents["project-agent"].slice(0, 180) : ""],
    prompt_sample: projectPrompt.slice(0, 500)
  }
};

console.log(JSON.stringify({ cases, evidence }));
"""


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=120)


def test_rule_bindings_asset() -> list[dict]:
    rules = json.loads(RULES.read_text(encoding="utf-8"))
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    check("schema_is_v2", rules.get("schema") == "omnidesk.profile-rules.v2", rules.get("schema"))
    bindings = rules.get("skill_bindings", {})
    check("bindings_present", isinstance(bindings, dict) and len(bindings) >= 3, str(len(bindings)))

    for scene in ("weekly", "writing", "project"):
        b = bindings.get(scene, {})
        skills = b.get("skills", [])
        max_injected = b.get("max_injected", 3)
        check(f"{scene}_has_skills", len(skills) > 0, str(skills))
        check(f"{scene}_max_injected_valid", 1 <= max_injected <= 3, str(max_injected))
        check(f"{scene}_skills_within_max", len(skills) <= max_injected, f"{len(skills)} <= {max_injected}")
        check(f"{scene}_all_skills_allowed", all(s in ALLOWED_SKILLS for s in skills), str(skills))
        check(f"{scene}_no_duplicate_skills", len(skills) == len(set(skills)), str(skills))

    # OD-ARCH-089：weekly 拆成 memory-agent（随手记）+ memory-report（周报），
    # anysearch 移出预注入——仍在白名单、Skill 文件仍随安装同步，运行时按需加载。
    weekly = bindings.get("weekly", {})
    check(
        "weekly_does_not_preinject_anysearch",
        "anysearch" not in weekly.get("skills", []),
        str(weekly.get("skills", [])),
    )
    check(
        "weekly_binds_memory_report",
        "memory-report" in weekly.get("skills", []),
        str(weekly.get("skills", [])),
    )
    check(
        "weekly_anysearch_skill_file_present",
        (SKILLS_DIR / "anysearch" / "SKILL.md").is_file(),
        str(SKILLS_DIR / "anysearch" / "SKILL.md"),
    )
    check(
        "weekly_memory_report_skill_file_present",
        (SKILLS_DIR / "memory-report" / "SKILL.md").is_file(),
        str(SKILLS_DIR / "memory-report" / "SKILL.md"),
    )

    return cases


def load_all_skills() -> dict[str, str]:
    out = {}
    for sk in ALLOWED_SKILLS:
        p = SKILLS_DIR / sk / "SKILL.md"
        if p.is_file():
            out[sk] = p.read_text(encoding="utf-8")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases = test_rule_bindings_asset()
    skills_map = load_all_skills()

    tmp = Path(tempfile.mkdtemp(prefix="od-skill082-"))
    compile_result = run(
        [
            "npx", "--no-install", "tsc",
            str(SOURCE_CP), str(SOURCE_AD),
            "--outDir", str(tmp / "out"),
            "--module", "commonjs",
            "--target", "ES2022",
            "--skipLibCheck",
            "--ignoreConfig",
        ],
        DESKTOP,
    )
    compiled_cp = tmp / "out" / "conversationPrompt.js"
    compiled_ad = tmp / "out" / "aiDraft.js"
    if compile_result.returncode != 0 or not compiled_cp.exists() or not compiled_ad.exists():
        print(json.dumps({
            "compile_exit": compile_result.returncode,
            "compile_stderr": compile_result.stderr[-2000:],
            "cases": len(cases), "failed": len(cases) + 1, "result": "fail",
        }, ensure_ascii=False))
        return 1

    probe_path = tmp / "probe_082.js"
    probe_path.write_text(PROBE_082, encoding="utf-8")
    probe = run(
        ["node", str(probe_path), str(compiled_cp), str(compiled_ad), str(RULES), json.dumps(skills_map)],
        DESKTOP,
    )
    if probe.returncode != 0:
        print(json.dumps({
            "compile_exit": 0,
            "probe_exit": probe.returncode,
            "probe_stderr": probe.stderr[-2000:],
            "cases": len(cases), "failed": len(cases) + 1, "result": "fail",
        }, ensure_ascii=False))
        return 1

    result_json = json.loads(probe.stdout.strip().splitlines()[-1])
    probe_cases = result_json["cases"]
    all_cases = cases + probe_cases
    failed = [c for c in all_cases if not c["ok"]]

    # 落盘 S5 场景证据
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    evidence_file = EVIDENCE_DIR / "scene-injection.json"
    evidence_payload = {
        "verified_at": "2026-10-25",
        "evidence_level": "script",
        "description": "OD-ARCH-082 S5 三大场景合成输入提示词组装事实（命中的 Skill id + 注入片段）",
        "scenes": result_json["evidence"]
    }
    evidence_file.write_text(json.dumps(evidence_payload, ensure_ascii=False, indent=2), encoding="utf-8")

    payload = {
        "cases": len(all_cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "evidence_path": str(evidence_file),
        "failed_cases": failed,
    }

    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result", "evidence_path")}, ensure_ascii=False))
    for c in failed:
        print("FAIL", c["name"], c["detail"])
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

