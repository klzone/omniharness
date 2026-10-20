#!/usr/bin/env python3
"""OD-ARCH-031 Part B：对话流提示词组装的离线验证。

直接编译 `desktop/src/conversationPrompt.ts` 的真实源码，用 Node 跑确定性分支：

1. 普通对话：提示词 = 上下文本身（含上一轮用户与助手的话）。
2. 带预设：提示词里同时出现**流程包的场景规则**、**期望章节**与上下文。
3. 长上下文：超过上限时只保留最近部分，并显式标注截断。
4. 预设路径**不丢**不可关闭的写作规则（禁入案件材料那条仍在）。
5. OD-ARCH-034：续接既有引擎会话时**不重放上文**（普通对话与带预设两条路径）。

**不**验证真实模型输出、真实宿主或窗口交互。不读取凭证，不访问网络，不启动引擎。

用法：
  python3 tests/verification/desktop/verify_conversation_prompt.py [--json PATH]
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
FLOW_PACK = REPO / "samples" / "workflows" / "offline-flows.v1.json"

PROBE = r"""
const mod = require(process.argv[2]);
const pack = require(process.argv[3]);

const weekly = pack.weekly.draft;
const preset = { key: "weekly", label: "周报与随记", rules: weekly.rules, sections: weekly.sections };
const turns = [
  { role: "user", text: "第一轮标记 ALPHA-123" },
  { role: "assistant", text: "助手第一轮回应 BETA-456" },
];
const cases = [];

function check(name, ok, detail) { cases.push({ name, ok: !!ok, detail: detail || "" }); }

// 1) 普通对话：提示词 = 上下文本身
const plain = mod.composeConversationPrompt({ preset: null, turns, current: "第二轮提问" });
check("plain_contains_prev_user", plain.includes("第一轮标记 ALPHA-123"), plain.slice(0, 80));
check("plain_contains_prev_assistant", plain.includes("助手第一轮回应 BETA-456"), "");
check("plain_contains_current", plain.includes("第二轮提问"), "");
check("plain_has_no_scene_rules", !plain.includes(weekly.rules.slice(0, 12)), "");

// 2) 带预设：规则 + 章节 + 上下文 都在
const withPreset = mod.composeConversationPrompt({ preset, turns, current: "第二轮提问" });
check("preset_contains_rules", withPreset.includes(weekly.rules), "");
check("preset_contains_sections", weekly.sections.split("\n").every((s) => !s.trim() || withPreset.includes(s)), "");
check("preset_contains_context", withPreset.includes("第一轮标记 ALPHA-123") && withPreset.includes("第二轮提问"), "");
check("preset_keeps_forbidden_rule", withPreset.includes("不处理案件材料"), "");

// 3) 长上下文：截断并标注
const long = [{ role: "user", text: "旧内容 OLD-MARKER " + "x".repeat(6000) }];
const truncated = mod.composeConversationPrompt({ preset: null, turns: long, current: "新内容 NEW-MARKER" });
check("truncated_marked", truncated.includes("上文已按长度截断"), "");
check("truncated_drops_oldest", !truncated.includes("OLD-MARKER"), "");
check("truncated_keeps_newest", truncated.includes("NEW-MARKER"), "");
check("truncated_within_budget", truncated.length <= mod.CONTEXT_MAX_CHARS + 200, String(truncated.length));

// 3b) 轮次窗口：只重放最近若干轮（更早的历史不进提示词）
const many = [];
for (let i = 1; i <= 6; i += 1) many.push({ role: "user", text: "U" + i });
const windowed = mod.composeConversationPrompt({ preset: null, turns: many, current: "NOW" });
check("window_drops_old_user_turns", !windowed.includes("U1"), windowed.slice(0, 80));
check("window_keeps_recent_user_turns", windowed.includes("U6") && windowed.includes("U3"), "");
check("window_bounded_chars", windowed.length <= mod.CONTEXT_MAX_CHARS + 200, String(windowed.length));

// 4) OD-ARCH-034：续接既有引擎会话时**不重放上文**（上下文由引擎会话持有）
const continued = mod.composeConversationPrompt({ preset: null, turns, current: "第三轮提问", continuation: true });
check("continuation_contains_current", continued.includes("第三轮提问"), continued.slice(0, 80));
check("continuation_drops_prev_user", !continued.includes("第一轮标记 ALPHA-123"), continued.slice(0, 80));
check("continuation_drops_prev_assistant", !continued.includes("助手第一轮回应 BETA-456"), "");
check("continuation_is_short", continued.length <= 200, String(continued.length));

// 4b) 续接 + 预设：场景规则仍在，但不带上文
const continuedPreset = mod.composeConversationPrompt({ preset, turns, current: "第三轮提问", continuation: true });
check("continuation_preset_keeps_rules", continuedPreset.includes(weekly.rules), "");
check("continuation_preset_keeps_sections", weekly.sections.split("\n").every((s) => !s.trim() || continuedPreset.includes(s)), "");
check("continuation_preset_drops_context", !continuedPreset.includes("第一轮标记 ALPHA-123"), "");
check("continuation_preset_keeps_current", continuedPreset.includes("第三轮提问"), "");
check("continuation_preset_keeps_forbidden_rule", continuedPreset.includes("不处理案件材料"), "");

console.log(JSON.stringify({ cases, failed: cases.filter((c) => !c.ok).length }));
"""

def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=240)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    tmp = Path(tempfile.mkdtemp(prefix="od-convprompt-"))
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
        payload = {
            "compile_exit": compile_result.returncode,
            "compile_stderr": compile_result.stderr[-2000:],
            "cases": 0, "failed": -1, "result": "fail",
        }
        print(json.dumps(payload, ensure_ascii=False))
        return 1

    probe_path = tmp / "probe.js"
    probe_path.write_text(PROBE, encoding="utf-8")
    probe = run(
        ["node", str(probe_path), str(compiled), str(FLOW_PACK)],
        DESKTOP,
    )
    if probe.returncode != 0:
        payload = {
            "compile_exit": 0,
            "probe_exit": probe.returncode,
            "probe_stderr": probe.stderr[-2000:],
            "cases": 0, "failed": -1, "result": "fail",
        }
        print(json.dumps(payload, ensure_ascii=False))
        return 1

    parsed = json.loads(probe.stdout.strip().splitlines()[-1])
    payload = {
        "compile_exit": 0,
        "probe_exit": 0,
        "cases": len(parsed["cases"]),
        "failed": parsed["failed"],
        "detail": parsed["cases"],
        "result": "pass" if parsed["failed"] == 0 else "fail",
    }
    if args.json:
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in payload.items() if k != "detail"}, ensure_ascii=False))
    return 0 if payload["result"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
