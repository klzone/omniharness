#!/usr/bin/env python3
"""OD-ARCH-088 S4 场景 intent 透传断言（静态源码 + 真实 scenes.ts 编译执行）。

证据等级：静态代码检查 + 脚本运行（Node 执行编译后的真实 scenes.ts）。
不启动窗口，也不调用模型；窗口侧的 intent 落盘断言在 verify_ui_088.py。

验证点：
  1. HomeChat.tsx 的 submitTask() 确实传了 intent，取值来自当前生效场景；
  2. scenes.ts 运行时映射：weekly→weekly_report、writing→material_writing、
     project→project_management；
  3. 非场景（未分类/自定义）时不传 intent，api.ts 回落 material_writing；
  4. HomeCanvas 把生效场景（active 线程优先）交给 HomeChat，而不是只传初始 tag。

用法：python3 tests/verification/desktop/verify_intent_088.py [--json PATH]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SCENES_TS = DESKTOP / "src" / "scenes.ts"
HOME_CHAT = DESKTOP / "src" / "shell" / "HomeChat.tsx"
HOME_CANVAS = DESKTOP / "src" / "shell" / "HomeCanvas.tsx"
API_TS = DESKTOP / "src" / "api.ts"
EVID = HERE / "evidence" / "od-arch-088"

PROBE = r"""
const scenes = require(process.argv[2]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : detail });
}

const mapping = {
  weekly: "weekly_report",
  writing: "material_writing",
  project: "project_management",
};
for (const [tag, expected] of Object.entries(mapping)) {
  const actual = scenes.SCENES[tag] && scenes.SCENES[tag].scenario;
  check("scenario_mapping_" + tag, actual === expected, { actual, expected });
}

// 复刻 HomeChat 的表达式，确认只有识别到的场景才给 intent。
function intentFor(activeTag) {
  return scenes.isSceneKey(activeTag) ? scenes.SCENES[activeTag].scenario : undefined;
}
check("intent_weekly", intentFor("weekly") === "weekly_report", intentFor("weekly"));
check("intent_writing", intentFor("writing") === "material_writing", intentFor("writing"));
check("intent_project", intentFor("project") === "project_management", intentFor("project"));
check("intent_plain_is_undefined", intentFor("plain") === undefined, String(intentFor("plain")));
check("intent_custom_is_undefined", intentFor("custom-x") === undefined, String(intentFor("custom-x")));
check("intent_empty_is_undefined", intentFor("") === undefined, String(intentFor("")));

// 非场景时 api 层必须维持既有默认值（不因本轮改动变成 undefined）。
const fallback = (input) => (input ?? "material_writing");
check("api_fallback_undefined", fallback(undefined) === "material_writing", fallback(undefined));
check("api_keeps_explicit", fallback("weekly_report") === "weekly_report", fallback("weekly_report"));

process.stdout.write(JSON.stringify({ cases }));
"""


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=180)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail=None, desc: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail, "desc": desc})

    home_chat = HOME_CHAT.read_text(encoding="utf-8")
    api = API_TS.read_text(encoding="utf-8")
    canvas = HOME_CANVAS.read_text(encoding="utf-8")

    # 1. 提交调用点确实带 intent，且取值来自 SCENES[activeTag].scenario。
    submit_at = home_chat.find("const receipt = await host.submitTask({")
    check("submit_task_call_present", submit_at >= 0, submit_at, "HomeChat 找到 submitTask 调用点")
    call_block = home_chat[submit_at : submit_at + 900] if submit_at >= 0 else ""
    check(
        "submit_task_passes_intent",
        "intent:" in call_block,
        "intent:" in call_block,
        "submitTask 参数含 intent",
    )
    check(
        "intent_uses_scene_scenario",
        "SCENES[activeTag].scenario" in call_block,
        "SCENES[activeTag].scenario" in call_block,
        "intent 取当前生效场景的 scenario",
    )
    check(
        "intent_guarded_by_is_scene_key",
        "isSceneKey(activeTag)" in call_block,
        "isSceneKey(activeTag)" in call_block,
        "非场景不传 intent（保持既有默认值）",
    )
    check(
        "scenes_imported_in_home_chat",
        "SCENES" in home_chat and "isSceneKey" in home_chat,
        True,
        "HomeChat 从 scenes.ts 导入场景表",
    )

    # 2. api.ts 的缺省回落没有被本轮改动破坏。
    check(
        "api_default_intent_material_writing",
        'intent: input.intent ?? "material_writing"' in api,
        'intent: input.intent ?? "material_writing"' in api,
        "api 层缺省仍为 material_writing",
    )
    check(
        "api_intent_optional",
        "intent?: string" in api,
        "intent?: string" in api,
        "intent 为可选字段，旧调用方不受影响",
    )

    # 3. 场景键的来源：HomeCanvas 把生效 tag（active 线程优先）交给 HomeChat。
    check(
        "canvas_passes_effective_tag",
        "activeTag={effectiveTag}" in canvas,
        "activeTag={effectiveTag}" in canvas,
        "HomeChat 收到的是生效场景，而不是初始分类",
    )
    check(
        "canvas_effective_tag_prefers_active_thread",
        "const effectiveTag: ThreadTag = active ? active.tag : activeTag;" in canvas,
        True,
        "已打开会话以会话自身的 tag 为准",
    )

    # 4. 编译真实 scenes.ts 并执行映射断言。
    tmp = Path(tempfile.mkdtemp(prefix="od-intent088-"))
    compile_result = run(
        [
            "npx",
            "--no-install",
            "tsc",
            str(SCENES_TS),
            "--outDir",
            str(tmp / "out"),
            "--module",
            "commonjs",
            "--target",
            "ES2022",
            "--skipLibCheck",
            "--ignoreConfig",
        ],
        DESKTOP,
    )
    compiled = tmp / "out" / "scenes.js"
    check(
        "scenes_compiled",
        compile_result.returncode == 0 and compiled.exists(),
        {"exit": compile_result.returncode, "stderr": compile_result.stderr[-1200:]},
        "tsc 编译 scenes.ts",
    )

    if compiled.exists():
        probe_path = tmp / "probe_intent_088.js"
        probe_path.write_text(PROBE, encoding="utf-8")
        probe = run(["node", str(probe_path), str(compiled)], DESKTOP)
        check("scenes_probe_exit", probe.returncode == 0, probe.returncode, "Node 执行映射断言")
        if probe.returncode == 0:
            try:
                payload = json.loads(probe.stdout.strip().splitlines()[-1])
            except Exception as exc:  # noqa: BLE001
                check("scenes_probe_parse", False, str(exc), "解析 Node 探针输出")
            else:
                for item in payload.get("cases", []):
                    check(item["name"], item["ok"], item.get("detail"), "scenes.ts 运行时断言")
        else:
            check("scenes_probe_output", False, probe.stderr[-1200:], "Node 探针失败")

    failed = [c for c in cases if not c["ok"]]
    facts = {
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "静态代码检查 + 脚本运行（编译真实 scenes.ts 并在 Node 中执行）",
        "repo_head": run(["git", "rev-parse", "HEAD"], REPO).stdout.strip(),
        "files": {
            "home_chat": str(HOME_CHAT.relative_to(REPO)),
            "home_canvas": str(HOME_CANVAS.relative_to(REPO)),
            "api": str(API_TS.relative_to(REPO)),
            "scenes": str(SCENES_TS.relative_to(REPO)),
        },
        "cases": len(cases),
        "passed": len(cases) - len(failed),
        "failed": len(failed),
        "failures": [c["name"] for c in failed],
        "checks": cases,
    }
    EVID.mkdir(parents=True, exist_ok=True)
    out = Path(args.json) if args.json else EVID / "intent.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")

    for case in cases:
        print(("PASS" if case["ok"] else "FAIL") + " [INTENT-088] " + case["name"])
    print(f"cases={len(cases)} passed={len(cases) - len(failed)} failed={len(failed)}")
    print(f"evidence={out}")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
