#!/usr/bin/env python3
"""OD-ARCH-068：平台中立规则资产化的离线验证。

编译 `desktop/src/conversationPrompt.ts` 的真实源码，验证规则迁出后仍然能组装出
正确的提示词，并核对规则资产与流程包的对应关系。

覆盖：
1. 随手记（note）只发一句整理指令：不带铁律、不带章节、不带归档条目。
2. 整理成周报（report）仍带场景规则 + 章节 + Profile 写作铁律。
3. 未传 writingRules 时提示词里没有铁律段（平台不再硬编码任何业务规则）。
4. 规则资产文件结构：16 个词条（14 红线 + 2 隐私归类）、7 条铁律、
   三场景思考档、隐私默认 mask。
5. 规则资产与流程包不冲突：场景键一致；流程包仍负责周报章节。

**不**验证真实模型输出、宿主命令或窗口交互。不读取凭证，不访问网络。

用法：
  python3 tests/verification/desktop/verify_profile_rules_068.py [--json PATH]
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
SOURCE = DESKTOP / "src" / "conversationPrompt.ts"
RULES = REPO / "samples" / "profiles" / "discipline.rules.json"
FLOW_PACK = REPO / "samples" / "workflows" / "offline-flows.v1.json"

PROBE = r"""
const mod = require(process.argv[2]);
const rules = require(process.argv[3]);
const pack = require(process.argv[4]);

const cases = [];
function check(name, ok, detail) { cases.push({ name, ok: !!ok, detail: detail || "" }); }

const weekly = pack.weekly.draft;
const scene = rules.scenes.weekly;

const preset = {
  key: "weekly",
  label: "周报与随记",
  rules: scene.rules,
  sections: scene.sections,
  writingRules: rules.global.writing_rules,
  thinking: scene.thinking,
};
const turns = [{ role: "user", text: "本周标记 GAMMA-789" }];
const current = "本周完成了三项工作";

// 1) 随手记：轻装
const note = mod.composeConversationPrompt({ preset, turns, current, mode: "note" });
check("note_contains_current", note.includes(current), note.slice(0, 60));
check("note_has_no_writing_rules", !note.includes(rules.global.writing_rules[0]), "");
check("note_has_no_scene_rules", !note.includes(scene.rules.slice(0, 10)), "");
check("note_has_no_sections", !note.includes(scene.sections.split("\n")[0]), "");
check("note_is_short", note.length < 600, String(note.length));

// 2) 整理成周报：完整组装
const report = mod.composeConversationPrompt({ preset, turns, current, mode: "report" });
check("report_contains_writing_rules", rules.global.writing_rules.every((r) => report.includes(r)), "");
check("report_contains_scene_rules", report.includes(scene.rules.slice(0, 10)), "");
check("report_contains_sections", scene.sections.split("\n").every((s) => !s.trim() || report.includes(s)), "");
check("report_contains_context", report.includes(current), "");
check("report_longer_than_note", report.length > note.length, report.length + " vs " + note.length);

// 3) 默认模式即 report（不传 mode）
const implicit = mod.composeConversationPrompt({ preset, turns, current });
check("default_mode_is_report", implicit === report, "");

// 4) 未传 writingRules：平台不硬编码铁律
const noRules = mod.composeConversationPrompt({
  preset: { key: "weekly", label: "周报与随记", rules: scene.rules, sections: scene.sections },
  turns,
  current,
});
check("no_writing_rules_argument_means_no_rules_section", !noRules.includes(rules.global.writing_rules[0]), "");

console.log(JSON.stringify({ cases, failed: cases.filter((c) => !c.ok).length }));
"""


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=240)


def asset_checks() -> list[dict]:
    """规则资产文件本身的静态核对（文件事实，不是运行证据）。"""
    rules = json.loads(RULES.read_text(encoding="utf-8"))
    pack = json.loads(FLOW_PACK.read_text(encoding="utf-8"))
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    gl = rules["global"]
    total = (
        len(gl["redline_keywords"])
        + len(gl["privacy_keywords"]["id_card"])
        + len(gl["privacy_keywords"]["phone"])
    )
    check(
        "schema_is_v1_or_v2",
        rules["schema"] in ("omnidesk.profile-rules.v1", "omnidesk.profile-rules.v2"),
        rules["schema"],
    )
    check("profile_name", rules["profile"] == "discipline", rules["profile"])
    check("16_keywords", total == 16, str(total))
    check("7_writing_rules", len(gl["writing_rules"]) == 7, str(len(gl["writing_rules"])))
    check("privacy_defaults_to_mask", gl["privacy"] == {"id_card": "mask", "phone": "mask"}, json.dumps(gl["privacy"]))
    check(
        "scene_thinking_levels",
        rules["scenes"]["weekly"]["thinking"] == "low"
        and rules["scenes"]["writing"]["thinking"] == "low"
        and rules["scenes"]["project"]["thinking"] == "off",
        json.dumps({k: v["thinking"] for k, v in rules["scenes"].items()}),
    )
    check("scene_keys_match_flow_pack", set(rules["scenes"]) == {k for k in pack if k != "schema"}, "")
    # 迁移无损：规则资产里的场景规则/章节必须来自流程包原文。
    for key in ("weekly", "writing", "project"):
        if key not in pack:
            continue
        src = pack[key].get("draft", pack[key])
        check(
            f"scene_{key}_rules_migrated_verbatim",
            rules["scenes"][key]["rules"] == src.get("rules", ""),
            "与 offline-flows.v1 原文不一致",
        )
        check(
            f"scene_{key}_sections_migrated_verbatim",
            rules["scenes"][key].get("sections", "") == src.get("sections", ""),
            "与 offline-flows.v1 原文不一致",
        )
    return cases


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases = asset_checks()
    tmp = Path(tempfile.mkdtemp(prefix="od-rules068-"))
    compile_result = run(
        [
            "npx", "--no-install", "tsc", str(SOURCE),
            "--outDir", str(tmp / "out"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    compiled = tmp / "out" / "conversationPrompt.js"
    if compile_result.returncode != 0 or not compiled.exists():
        print(json.dumps({
            "compile_exit": compile_result.returncode,
            "compile_stderr": compile_result.stderr[-2000:],
            "cases": len(cases), "failed": len(cases) + 1, "result": "fail",
            "asset_cases": cases,
        }, ensure_ascii=False))
        return 1

    probe_path = tmp / "probe.js"
    probe_path.write_text(PROBE, encoding="utf-8")
    probe = run(["node", str(probe_path), str(compiled), str(RULES), str(FLOW_PACK)], DESKTOP)
    if probe.returncode != 0:
        print(json.dumps({
            "compile_exit": 0,
            "probe_exit": probe.returncode,
            "probe_stderr": probe.stderr[-2000:],
            "cases": len(cases), "failed": len(cases) + 1, "result": "fail",
        }, ensure_ascii=False))
        return 1

    probe_cases = json.loads(probe.stdout.strip().splitlines()[-1])
    all_cases = cases + probe_cases["cases"]
    failed = [c for c in all_cases if not c["ok"]]
    payload = {
        "compile_exit": 0,
        "cases": len(all_cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "asset_cases": cases,
        "runtime_cases": probe_cases["cases"],
        "failed_cases": failed,
    }
    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result")}, ensure_ascii=False))
    for c in failed:
        print("FAIL", c["name"], c["detail"])
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

