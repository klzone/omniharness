#!/usr/bin/env python3
"""OD-ARCH-051 S2：续接会话 vs 每轮重放上文的输入量对比。

验收要求（PLANS.md）：同任务下「重放版」与「续接版」的输入量应显著下降。

本脚本做两件事，证据等级分开记：

1. **真实源码测量（脚本运行）**：编译 `desktop/src/conversationPrompt.ts` 的真实源码，
   用同一段多轮对话分别按 continuation=False（每轮重放，即旧行为）和
   continuation=True（续接，引擎持有上下文）组装提示词，逐轮统计字符数。
2. **真实适配层交叉核对（脚本运行）**：启动同仓库合成端点，跑两轮真实 DSH web 会话，
   记录每轮实际发给模型的 message 数与该轮用户消息长度，确认续接轮只发本轮输入。

不读凭证、不连外网、不碰 ~/.dsh / ~/.agents / ~/OmniDesk；只用 /tmp 一次性数据根。

用法：
  python3 tests/verification/desktop/verify_resume_token_savings.py [--json PATH]
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
ENGINE_TESTS = REPO / "tests" / "verification" / "engine-interface" / "adapter"

PROBE = r"""
const mod = require(process.argv[2]);

// 一段合成的三段式工作对话（无真实人员、机构、案件信息）。
const turn1 = "请把这周的合成样例要点整理成周报骨架。";
const reply1 = "已按「本周进展 / 存在问题 / 下周计划」给出骨架，待补充数据。";
const turn2 = "下周计划里补一条：把合成样例的统计脚本加上失败重试。";
const reply2 = "已补入下周计划第二条，并标注为待你确认。";
const turn3 = "最后请把整篇缩到 300 字以内，保留三段结构。";

const turnsBefore3 = [
  { role: "user", text: turn1 },
  { role: "assistant", text: reply1 },
  { role: "user", text: turn2 },
  { role: "assistant", text: reply2 },
];
const turnsBefore2 = turnsBefore3.slice(0, 2);

const replay = [
  mod.composeConversationPrompt({ preset: null, turns: [], current: turn1 }),
  mod.composeConversationPrompt({ preset: null, turns: turnsBefore2, current: turn2 }),
  mod.composeConversationPrompt({ preset: null, turns: turnsBefore3, current: turn3 }),
];
const resume = [
  mod.composeConversationPrompt({ preset: null, turns: [], current: turn1 }),
  mod.composeConversationPrompt({ preset: null, turns: turnsBefore2, current: turn2, continuation: true }),
  mod.composeConversationPrompt({ preset: null, turns: turnsBefore3, current: turn3, continuation: true }),
];

const chars = (list) => list.reduce((sum, s) => sum + s.length, 0);
console.log(JSON.stringify({
  per_turn_replay_chars: replay.map((s) => s.length),
  per_turn_resume_chars: resume.map((s) => s.length),
  total_replay_chars: chars(replay),
  total_resume_chars: chars(resume),
  resume_keeps_current_turn: resume.every((s, i) => s.includes([turn1, turn2, turn3][i])),
  resume_drops_prior_user: !resume[2].includes(turn1) && !resume[2].includes(turn2),
  replay_carries_prior_user: replay[2].includes(turn1) && replay[2].includes(turn2),
  context_max_chars: mod.CONTEXT_MAX_CHARS,
}));
"""


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


def measure_prompts() -> dict:
    tmp = Path(tempfile.mkdtemp(prefix="od-resume-savings-"))
    compiled = run(
        [
            "npx", "--no-install", "tsc", str(SOURCE),
            "--outDir", str(tmp / "out"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    js = tmp / "out" / "conversationPrompt.js"
    if compiled.returncode != 0 or not js.exists():
        return {"ok": False, "stage": "compile", "stderr": compiled.stderr[-2000:]}
    probe_path = tmp / "probe.js"
    probe_path.write_text(PROBE, encoding="utf-8")
    probe = run(["node", str(probe_path), str(js)], DESKTOP)
    if probe.returncode != 0:
        return {"ok": False, "stage": "probe", "stderr": probe.stderr[-2000:]}
    data = json.loads(probe.stdout.strip().splitlines()[-1])
    data["ok"] = True
    total_replay = data["total_replay_chars"]
    total_resume = data["total_resume_chars"]
    data["saved_chars"] = total_replay - total_resume
    data["saved_ratio"] = round((total_replay - total_resume) / total_replay, 4) if total_replay else None
    return data


def measure_adapter(prompt_mode: str) -> dict:
    """真实 DSH web 会话两轮。

    prompt_mode：
      * ``resume``——第 2 轮在同一个引擎会话内继续，只发本轮输入（现行行为）。
      * ``replay``——第 2 轮新开会话，把第 1 轮原文重放进本轮提示（旧行为）。

    两条路径都各用一次性数据根，互不影响。只记录发送与否、消息条数与总字符量，
    不保存请求正文。
    """
    sys.path.insert(0, str(ENGINE_TESTS))
    import time as _t  # noqa: E402
    import _harness as H  # noqa: E402
    from _mock_model_server import start_mock  # noqa: E402
    from omnidesk_engine import SubmitTaskRequest, TaskState  # noqa: E402

    base_url, ctl, server = start_mock()
    root = H.new_root("od051_s2_" + prompt_mode)
    overlay = H.write_overlay(root, "od051-s2-" + prompt_mode + ".yaml", H.web_overlay_text(base_url))
    adapter = H.make_web_adapter(root, patches=[overlay], profile="web")
    terminal = (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
                TaskState.INTERRUPTED, TaskState.UNKNOWN)

    def settle(task_id: str, timeout: float = 60.0):
        deadline = _t.monotonic() + timeout
        while _t.monotonic() < deadline:
            st = adapter.get_task_status(task_id)
            if st.status in terminal:
                return st
            _t.sleep(0.1)
        return adapter.get_task_status(task_id)

    out: dict = {"ok": False}
    try:
        turn1_marker = "合成第一轮：请记录本周进展。"
        turn2_marker = "合成第二轮：把下周计划补一条。"
        # 探针只记录命中与否，不保存请求正文；用来观察第二轮是否携带上一轮原文。
        ctl.set(chunks=["FIRST-REPLY"], mode="ok", probes=[turn1_marker])
        adapter.initialize()
        first = adapter.submit_task(SubmitTaskRequest(
            intent="web-synthetic-multiturn",
            prompt_text=turn1_marker,
            workspace_dir=str(root["cwd"]),
        ))
        first_status = settle(first.omni_task_id)
        ctl.set(chunks=["SECOND-REPLY"], mode="ok")
        if prompt_mode == "replay":
            second_prompt = "用户：" + turn1_marker + "\n助手：FIRST-REPLY\n用户：" + turn2_marker
            engine_session_id = None
        else:
            second_prompt = turn2_marker
            engine_session_id = first.engine_session_id
        second = adapter.submit_task(SubmitTaskRequest(
            intent="web-synthetic-multiturn",
            prompt_text=second_prompt,
            workspace_dir=str(root["cwd"]),
            engine_session_id=engine_session_id,
        ))
        st = settle(second.omni_task_id)
        reqs = ctl.new_requests()
        out.update({
            "ok": True,
            "session_mode": second.session_mode,
            "same_session": first.engine_session_id == second.engine_session_id,
            "first_status": first_status.status.value,
            "second_status": st.status.value,
            "model_requests": [
                {"index": i, "message_count": r.get("message_count"), "last_user": r.get("last_user"),
                 "text_chars": r.get("text_chars"),
                 "probe_hits": r.get("probe_hits"), "probe_misses": r.get("probe_misses")}
                for i, r in enumerate(reqs)
            ],
            "turn1_marker": turn1_marker,
            "turn2_marker": turn2_marker,
        })
    finally:
        try:
            out["shutdown"] = adapter.shutdown(mode="graceful", grace_period=5).to_dict()
        except Exception as exc:  # noqa: BLE001
            out["shutdown_error"] = str(exc)
        server.shutdown()
        H.drop_root(root)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    prompts = measure_prompts()
    checks = []

    def check(name: str, ok: bool, detail) -> None:
        checks.append({"name": name, "ok": bool(ok), "detail": detail})

    if not prompts.get("ok"):
        check("prompt_source_measured", False, prompts)
        adapter = {"ok": False, "skipped": "提示词测量失败，先修源码编译"}
    else:
        check("prompt_source_measured", True, "真实 conversationPrompt.ts 编译并运行")
        check("resume_keeps_current_turn", prompts["resume_keeps_current_turn"], "")
        check("resume_drops_prior_user", prompts["resume_drops_prior_user"], "")
        check("replay_carries_prior_user", prompts["replay_carries_prior_user"], "")
        check("resume_input_is_clearly_smaller", prompts["saved_ratio"] is not None and prompts["saved_ratio"] >= 0.30,
              {"saved_ratio": prompts["saved_ratio"], "saved_chars": prompts["saved_chars"]})
        resume_run = measure_adapter("resume")
        replay_run = measure_adapter("replay")
        adapter = {"resume": resume_run, "replay": replay_run}

        check("adapter_resume_mode_continued", resume_run.get("session_mode") == "continued",
              resume_run.get("session_mode"))
        check("adapter_same_session", resume_run.get("same_session") is True, resume_run.get("same_session"))
        check("adapter_replay_mode_new_session", replay_run.get("session_mode") == "new",
              replay_run.get("session_mode"))
        check("adapter_both_turns_succeeded",
              resume_run.get("first_status") == resume_run.get("second_status") == "succeeded"
              and replay_run.get("first_status") == replay_run.get("second_status") == "succeeded",
              {"resume": (resume_run.get("first_status"), resume_run.get("second_status")),
               "replay": (replay_run.get("first_status"), replay_run.get("second_status"))})

        def turn2_request(run: dict):
            """取第二轮那次模型请求（跳过引擎插入的会话标题等辅助请求）。"""
            for req in run.get("model_requests") or []:
                last = str(req.get("last_user", ""))
                if last.startswith("合成第二轮") or "用户：合成第一轮" in last:
                    return req
            return None

        r2, p2 = turn2_request(resume_run), turn2_request(replay_run)
        measured = {
            "resume": {"text_chars": (r2 or {}).get("text_chars"), "message_count": (r2 or {}).get("message_count")},
            "replay": {"text_chars": (p2 or {}).get("text_chars"), "message_count": (p2 or {}).get("message_count")},
        }
        adapter["turn2_at_model"] = measured
        check("adapter_turn2_measured_in_both_modes",
              measured["resume"]["text_chars"] is not None and measured["replay"]["text_chars"] is not None,
              measured)
        check("adapter_turn2_current_turn_reaches_model",
              "合成第二轮" in str((r2 or {}).get("last_user", "")),
              str((r2 or {}).get("last_user", ""))[:40])

    failed = [c for c in checks if not c["ok"]]
    payload = {
        "plan": "OD-ARCH-051 S2",
        "evidence_level": "script（真实源码 + 真实 DSH web 进程 + 合成模型端点）",
        "checks": checks,
        "checks_total": len(checks),
        "checks_failed": len(failed),
        "prompt_measurement": prompts,
        "adapter_measurement": adapter,
        "finding": (
            "前端组装层面：续接轮只发本轮输入，三轮累计字符从 %s 降到 %s（省 %s）。"
            "模型实际收到的总量请见 adapter_measurement.turn2_at_model：DSH 会在续接会话时"
            "把该会话历史一并送给模型，因此「省下的是前端重复拼装，不是引擎侧上下文」。"
            % (
                prompts.get("total_replay_chars"), prompts.get("total_resume_chars"),
                prompts.get("saved_chars"),
            ) if prompts.get("ok") else "提示词测量未完成"
        ),
        "result": "pass" if not failed else "fail",
    }
    if args.json:
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in payload.items() if k not in ("checks", "adapter_measurement")},
                     ensure_ascii=False))
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
