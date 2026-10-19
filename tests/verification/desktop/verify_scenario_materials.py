#!/usr/bin/env python3
"""OD-ARCH-026 —— 三类工作场景的真实模型链路验证：所选材料进入模型任务。

验证链路（与界面「工作场景」同形）：

    用户输入 + 用户**明确选择**的材料文件 + 可编辑流程包规则
      → 真实源码 desktop/src/aiDraft.ts 组装的提示词（用 node 运行它的编译产物）
        → 引擎 `omni.submit_task` → ACP → DSH CLI → 本机回环合成端点
          → 终态 → `omni.get_task_result`

关键点：提示词由**真实源码**产出，不是本脚本拼的；材料正文必须出现在合成端点实际收到的
请求里（probe 命中），否则判定失败。

覆盖：三类场景各一次成功（材料参与请求、结果可见），一次失败（error_400），一次取消（hang）。

证据等级：script（真实引擎子进程 + 真实 DSH CLI + 本机回环合成端点 + 真实前端源码产物）。
安全边界：一次性 /tmp 数据根；合成材料全部虚构；不读/不打印/不复制凭证；不访问外网；
         不触碰 ~/.dsh、~/.agents、~/OmniDesk、~/YvanAI 或仓库内文件。

用法：
  python3 tests/verification/desktop/verify_scenario_materials.py [--json PATH]
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
SOURCE = DESKTOP / "src" / "aiDraft.ts"
sys.path.insert(0, str(HERE))

from run_desktop_e2e import (  # noqa: E402
    EngineProcess,
    drop_root,
    initialize_params,
    new_root,
    validate_isolated_root,
    wait_terminal,
    write_overlay,
)
from _mock_model_server import start_mock  # noqa: E402

# 每类场景一个独立的合成材料标记；只有真正进入模型请求才会被合成端点命中。
SCENARIOS = [
    ("weekly_report", "合成周报", "OD26-WEEKLY-材料标记-合成", "合成周材料.md", "按周报结构整理。"),
    ("material_writing", "合成文案", "OD26-WRITING-材料标记-合成", "合成文案材料.md", "按材料目的与受众起草。"),
    ("project_management", "合成项目", "OD26-PROJECT-材料标记-合成", "合成项目清单.csv", "整理项目进展与逾期。"),
]

PROBE_JS = r"""
const m = require(__MODULE__);
const scenarios = __SCENARIOS__;
const out = scenarios.map((s) => ({
  intent: s.intent,
  marker: s.marker,
  name: s.name,
  prompt: m.buildDraftPrompt({
    sceneLabel: s.label,
    purpose: '向合成评审组汇报合成进度',
    audience: '合成评审组',
    requirements: '不超过两页，不含真实数据',
    material: '合成输入：事项 A 已完成。',
    materials: [{ name: s.name, sha256: 'c'.repeat(64), content: '材料正文 ' + s.marker }],
    sceneRules: s.rules,
    sceneSections: '概览\n明细\n待确认事项',
  }),
}));
process.stdout.write(JSON.stringify(out));
"""


def build_real_prompts(report: dict) -> list[dict]:
    """用真实 aiDraft.ts 源码组装三类场景的提示词。"""
    tmp = Path(tempfile.mkdtemp(prefix="od26-materials-"))
    compile_result = subprocess.run(
        [
            "npx", "--no-install", "tsc", str(SOURCE),
            "--outDir", str(tmp / "out"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        cwd=str(DESKTOP), capture_output=True, text=True, timeout=180,
    )
    compiled = tmp / "out" / "aiDraft.js"
    if compile_result.returncode != 0 or not compiled.exists():
        report["compile"] = {"exit": compile_result.returncode, "stderr": compile_result.stderr[-1500:]}
        return []
    report["compile"] = {"exit": 0, "module": str(compiled)}

    payload = [
        {"intent": intent, "label": label, "marker": marker, "name": name, "rules": rules}
        for intent, label, marker, name, rules in SCENARIOS
    ]
    probe = tmp / "probe.cjs"
    probe.write_text(
        PROBE_JS.replace("__MODULE__", json.dumps(str(compiled))).replace(
            "__SCENARIOS__", json.dumps(payload, ensure_ascii=False)
        ),
        encoding="utf-8",
    )
    run = subprocess.run(["node", str(probe)], cwd=str(tmp), capture_output=True, text=True, timeout=120)
    try:
        out = json.loads(run.stdout)
    except Exception:
        report["probe"] = {"exit": run.returncode, "stderr": run.stderr[-1500:]}
        return []
    report["probe"] = {"exit": run.returncode, "count": len(out)}
    return out


def _run_engine_scenario(prompt: str, marker: str, intent: str, mode: str, hang: float = 0.0) -> dict:
    """在隔离根中真实启动引擎并跑一次任务，返回可核验事实。"""
    base_url, ctl, server = start_mock()
    ctl.set(mode=mode, chunks=["合成", "初稿", "正文"], delay=0.0, probes=[marker], hang_seconds=hang)
    root = new_root("material_" + intent)
    entry: dict = {
        "intent": intent,
        "mode": mode,
        "root": str(root["base"]),
        "isolation_problems": validate_isolated_root(root),
    }
    engine = EngineProcess("material_" + intent, root)
    try:
        overlay = write_overlay(root, base_url)
        _init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize_ok"] = err is None
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        receipt, err = engine.request(
            "omni.submit_task",
            {"intent": intent, "workspace_dir": str(root["cwd"]), "prompt_text": prompt},
            timeout=60,
        )
        entry["submit_ok"] = err is None
        if err is not None:
            entry["verdict"] = "fail"
            return entry
        task_id = receipt["omni_task_id"]

        if mode == "hang":
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                st, e = engine.request("omni.get_task_status", {"omni_task_id": task_id}, timeout=20)
                if e is None and st.get("status") == "running":
                    break
                time.sleep(0.2)
            outcome, err = engine.request(
                "omni.cancel_task", {"omni_task_id": task_id, "timeout_seconds": 15}, timeout=60
            )
            entry["cancel"] = {"ok": err is None, "phase": (outcome or {}).get("phase")}

        snaps = wait_terminal(engine, task_id, timeout=120)
        final = (snaps[-1] if snaps else {}).get("status")
        entry["final_status"] = final

        result, err = engine.request("omni.get_task_result", {"omni_task_id": task_id}, timeout=60)
        entry["result_form"] = (result or {}).get("form")
        entry["result_content"] = (result or {}).get("content")

        requests = list(ctl.requests)
        entry["model_request_count"] = len(requests)
        entry["material_probe_hit"] = any(marker in (r.get("probe_hits") or []) for r in requests)
        entry["material_in_last_user"] = any(marker in (r.get("last_user") or "") for r in requests)

        common = {
            "initialize_ok": entry.get("initialize_ok"),
            "submit_ok": entry.get("submit_ok"),
            "model_request_recorded": len(requests) > 0,
            # probe 命中即证明材料正文进入了合成端点实际收到的请求；last_user 只作参考记录
            # （DSH/ACP 可能把提示词拆成多条消息，最后一条用户消息未必含原文）。
            "material_entered_model_request": entry["material_probe_hit"],
            "no_isolation_violation": not entry["isolation_problems"],
        }
        if mode == "ok":
            checks = {
                **common,
                "reached_succeeded": final == "succeeded",
                "result_visible": "合成" in (entry.get("result_content") or ""),
            }
        elif mode == "error_400":
            # 失败路径：不得伪报成功，也不得把结果内容当真。
            checks = {
                **common,
                "reached_failed": final == "failed",
                "no_fake_success": "合成" not in (entry.get("result_content") or ""),
            }
        else:  # hang + cancel
            # 取消发生在模型请求之前（hang 下请求尚未发出即被取消），
            # 因此这里**不**要求 probe 命中；材料进入模型请求由 ok/error 路径证明。
            phase = (entry.get("cancel") or {}).get("phase")
            checks = {
                "initialize_ok": entry.get("initialize_ok"),
                "submit_ok": entry.get("submit_ok"),
                "cancel_request_recorded": phase in (
                    "request_sent", "confirmed_cancelled", "timeout_unknown", "acknowledged"
                ),
                "phase_not_misreported": phase != "not_requested",
                "terminal_not_succeeded": final != "succeeded",
                "no_isolation_violation": not entry["isolation_problems"],
            }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    report: dict = {"prompts_from_real_source": None, "scenarios": []}
    prompts = build_real_prompts(report)
    if not prompts:
        print(json.dumps(report, ensure_ascii=False, indent=2)[-2000:])
        print("RESULT: fail（无法从真实源码取得提示词）")
        return 1
    report["prompts_from_real_source"] = {
        "count": len(prompts),
        "material_in_prompt": all(p["marker"] in p["prompt"] for p in prompts),
    }

    # 三类场景成功
    for p in prompts:
        report["scenarios"].append(
            _run_engine_scenario(p["prompt"], p["marker"], p["intent"], mode="ok")
        )
    # 失败与取消（用第一个场景的提示词，材料照旧参与请求）
    first = prompts[0]
    report["scenarios"].append(
        _run_engine_scenario(first["prompt"], first["marker"], first["intent"], mode="error_400")
    )
    report["scenarios"].append(
        _run_engine_scenario(first["prompt"], first["marker"], first["intent"], mode="hang", hang=90.0)
    )

    failed = [s for s in report["scenarios"] if s.get("verdict") != "pass"]
    for s in failed:
        print("FAIL", s.get("intent"), s.get("mode"))
        print(json.dumps(s, ensure_ascii=False, indent=2))
    print(
        f"prompts={len(prompts)} scenarios={len(report['scenarios'])} failed={len(failed)}"
    )
    print("RESULT:", "pass" if not failed and report["prompts_from_real_source"]["material_in_prompt"] else "fail")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
