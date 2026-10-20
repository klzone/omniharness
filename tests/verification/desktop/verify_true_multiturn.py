#!/usr/bin/env python3
"""OD-ARCH-034 端到端验证：真多轮（同一引擎会话内续发，不重放上文）。

用真实锁定版本的 DSH 引擎（公开 ACP）+ 本机回环**合成**模型端点：
  * 第一轮：session/new；
  * 第二轮：session/resume + session/prompt（**同一个会话**），工作台不再重放上文；
  * 第三轮：适配层**重启后**（新进程、同一数据根）仍能 resume 同一会话；
  * 另测：会话号失效时**降级为新建会话**（不抛错、不丢历史）。

关键证据：合成端点收到的第 2 次请求里**消息条数变多** ⇒ 引擎侧确实带着上文继续对话，
而不是靠调用方把上文重放进提示词。

安全边界：一次性 /tmp 数据根；合成端点只监听 127.0.0.1；不读不写真实凭证；
不触碰 ~/.dsh、~/OmniDesk；素材全为合成占位文本。

证据等级：script（真实引擎进程 + 合成端点）。

用法：
    python3 tests/verification/desktop/verify_true_multiturn.py [--json PATH]
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ADAPTER_DIR = REPO / "tests" / "verification" / "engine-interface" / "adapter"
sys.path.insert(0, str(REPO / "engine"))
sys.path.insert(0, str(ADAPTER_DIR))

import _harness as H  # noqa: E402
from _harness import method_counts, safe_call  # noqa: E402
from _mock_model_server import start_mock  # noqa: E402

from omnidesk_engine import SubmitTaskRequest, TaskState  # noqa: E402

TURN1 = "SYNTH-TURN-ONE-MARKER 请用一句话说明合成占位材料的写作要点。"
TURN2 = "SYNTH-TURN-TWO-MARKER 请再补一句关于合成占位材料的补充说明。"
TURN3 = "SYNTH-TURN-THREE-MARKER 请把前面的要点合并成一句话。"

TERMINAL = {
    TaskState.SUCCEEDED,
    TaskState.FAILED,
    TaskState.CANCELLED,
    TaskState.INTERRUPTED,
    TaskState.UNKNOWN,
}


def wait_terminal(adapter, task_id: str, timeout: float = 90.0) -> str | None:
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        status = adapter.get_task_status(task_id)
        last = status.status
        if last in TERMINAL:
            return last.value
        time.sleep(0.1)
    return last.value if last else None


def prompt_texts(adapter) -> list:
    """从适配层发出的原始帧里取出每次 session/prompt 的文本（仅用于核对，不写入证据正文）。"""
    out = []
    for frame in adapter.raw_sent_frames():
        if not isinstance(frame, dict):
            continue
        if frame.get("method") != "session/prompt":
            continue
        params = frame.get("params") or {}
        blocks = params.get("prompt") or []
        text = "".join(
            b.get("text", "") for b in blocks if isinstance(b, dict)
        )
        out.append(text)
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args()

    checks: list[dict] = []
    artifacts: dict = {}

    def check(name: str, ok: bool, detail=None) -> bool:
        entry = {"check": name, "pass": bool(ok)}
        if detail is not None:
            entry["detail"] = detail
        checks.append(entry)
        print(("  PASS " if ok else "  FAIL ") + name + ("" if detail is None else "  " + json.dumps(detail, ensure_ascii=False)))
        return bool(ok)

    root = H.new_root("arch034")
    base_url, ctl, server = start_mock()
    # 探针：合成端点只记录「标记是否出现在请求里」，不保存正文。
    ctl.set(probes=[TURN1, TURN2, TURN3])
    overlay = H.write_overlay(root, "arch034", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(
        root, [overlay], init_timeout=60, session_timeout=60, default_timeout=90
    )

    session_id = None
    try:
        ok_init, desc = safe_call(adapter.initialize)
        check("initialize_ok", ok_init, desc if not ok_init else "ok")
        if not ok_init:
            raise SystemExit("engine initialize failed")

        # ---- 第一轮：新建会话 ----
        ok1, receipt1 = safe_call(
            adapter.submit_task,
            SubmitTaskRequest(
                intent="synthetic-multiturn",
                workspace_dir=str(root["cwd"]),
                prompt_text=TURN1,
            ),
        )
        check("turn1_submit_ok", ok1, receipt1 if not ok1 else "ok")
        if not ok1:
            raise SystemExit("turn1 submit failed")
        session_id = receipt1.engine_session_id
        check("turn1_has_session_id", bool(session_id), bool(session_id))
        counts1 = method_counts(adapter.raw_sent_frames())
        check("turn1_used_session_new", counts1.get("session/new", 0) == 1, counts1)
        check("turn1_no_resume_yet", counts1.get("session/resume", 0) == 0, counts1)
        status1 = wait_terminal(adapter, receipt1.omni_task_id)
        check("turn1_reached_success", status1 == TaskState.SUCCEEDED.value, status1)

        reqs_after_turn1 = ctl.new_requests()
        msgs_turn1 = reqs_after_turn1[-1]["message_count"] if reqs_after_turn1 else 0
        artifacts["message_count_turn1"] = msgs_turn1

        # ---- 第二轮：续接同一会话（真多轮）----
        ok2, receipt2 = safe_call(
            adapter.submit_task,
            SubmitTaskRequest(
                intent="synthetic-multiturn",
                workspace_dir=str(root["cwd"]),
                prompt_text=TURN2,
                engine_session_id=session_id,
            ),
        )
        check("turn2_submit_ok", ok2, receipt2 if not ok2 else "ok")
        if not ok2:
            raise SystemExit("turn2 submit failed")
        check("turn2_kept_same_session", receipt2.engine_session_id == session_id, receipt2.engine_session_id)
        check("turn2_note_says_same_session", "同一引擎会话" in receipt2.note, receipt2.note)
        counts2 = method_counts(adapter.raw_sent_frames())
        artifacts["sent_methods_after_turn2"] = counts2
        # 同一进程内会话还活着：直接再发 session/prompt，不该再调 session/resume
        # （DSH 的 resume 只接受已落盘、可重开的会话，对活会话会报 invalid_argument）。
        check("turn2_did_not_call_resume_on_live_session", counts2.get("session/resume", 0) == 0, counts2)
        check("turn2_did_not_create_new_session", counts2.get("session/new", 0) == 1, counts2)
        status2 = wait_terminal(adapter, receipt2.omni_task_id)
        check("turn2_reached_success", status2 == TaskState.SUCCEEDED.value, status2)

        # 关键证据：合成端点收到的第 2 次请求消息条数变多 ⇒ 引擎侧带着上文继续。
        reqs_after_turn2 = ctl.new_requests()
        msgs_turn2 = reqs_after_turn2[-1]["message_count"] if reqs_after_turn2 else 0
        artifacts["message_count_turn2"] = msgs_turn2
        check(
            "engine_carried_history_into_turn2",
            msgs_turn2 > msgs_turn1,
            {"turn1": msgs_turn1, "turn2": msgs_turn2},
        )
        last = reqs_after_turn2[-1] if reqs_after_turn2 else {}
        check(
            "turn2_request_contains_new_text",
            TURN2 in (last.get("probe_hits") or []),
            {"probe_hits": last.get("probe_hits")},
        )
        # 引擎侧第 2 次请求**应当**带上第 1 轮内容（这正是「真多轮」的证据：
        # 上文由引擎会话持有，不是靠调用方重放进提示词）。
        check(
            "engine_side_request_carries_turn1_history",
            TURN1 in (last.get("probe_hits") or []),
            {"probe_hits": last.get("probe_hits")},
        )

        # 工作台侧：第二轮**不再重放**第一轮内容（提示词里只有本轮输入）。
        prompts = prompt_texts(adapter)
        artifacts["prompt_count"] = len(prompts)
        check("two_prompts_sent", len(prompts) == 2, len(prompts))
        check("turn2_prompt_has_new_text", TURN2 in prompts[-1], prompts[-1][:60])
        check("turn2_prompt_has_no_replay_of_turn1", TURN1 not in prompts[-1], prompts[-1][:60])

        # 台账如实记录本轮是续接。
        ok_ledger, ledger = safe_call(adapter.list_task_records)
        if ok_ledger:
            modes = {r.omni_task_id: r.to_dict().get("session_mode") for r in ledger.records}
            artifacts["ledger_session_modes"] = list(modes.values())
            check(
                "ledger_records_session_modes",
                modes.get(receipt1.omni_task_id) == "new"
                and modes.get(receipt2.omni_task_id) == "continued",
                modes,
            )
    finally:
        artifacts["first_shutdown"] = adapter.shutdown(mode="graceful", grace_period=5).to_dict()

    # ---- 第三轮：适配层重启后仍能续接同一会话 ----
    adapter2 = None
    try:
        # 端点保持不变（同一个合成端点进程），因此会话的模型配置仍然有效。
        overlay2 = H.write_overlay(root, "arch034b", H.mock_overlay_text(base_url))
        adapter2 = H.make_adapter(
            root, [overlay2], init_timeout=60, session_timeout=60, default_timeout=60
        )
        ok_init2, desc2 = safe_call(adapter2.initialize)
        check("restart_initialize_ok", ok_init2, desc2 if not ok_init2 else "ok")

        ok3, receipt3 = safe_call(
            adapter2.submit_task,
            SubmitTaskRequest(
                intent="synthetic-multiturn",
                workspace_dir=str(root["cwd"]),
                prompt_text=TURN3,
                engine_session_id=session_id,
            ),
        )
        check("turn3_submit_ok_after_restart", ok3, receipt3 if not ok3 else "ok")
        if ok3:
            check("turn3_kept_same_session", receipt3.engine_session_id == session_id, receipt3.engine_session_id)
            counts3 = method_counts(adapter2.raw_sent_frames())
            artifacts["sent_methods_after_turn3"] = counts3
            check("turn3_used_session_resume", counts3.get("session/resume", 0) == 1, counts3)
            check("turn3_did_not_create_new_session", counts3.get("session/new", 0) == 0, counts3)
            check("turn3_note_says_resumed", "续接" in receipt3.note, receipt3.note)
            status3 = wait_terminal(adapter2, receipt3.omni_task_id)
            check("turn3_reached_success", status3 == TaskState.SUCCEEDED.value, status3)
            reqs3 = ctl.new_requests()
            msgs_turn3 = reqs3[-1]["message_count"] if reqs3 else 0
            artifacts["message_count_turn3"] = msgs_turn3
            check(
                "engine_history_survived_restart",
                msgs_turn3 > msgs_turn2,
                {"turn2": msgs_turn2, "turn3": msgs_turn3},
            )
    finally:
        if adapter2 is not None:
            artifacts["second_shutdown"] = adapter2.shutdown(mode="graceful", grace_period=5).to_dict()

    # ---- 降级：会话号失效 → 新建会话继续，不抛错 ----
    adapter3 = None
    try:
        overlay3 = H.write_overlay(root, "arch034c", H.mock_overlay_text(base_url))
        adapter3 = H.make_adapter(
            root, [overlay3], init_timeout=60, session_timeout=60, default_timeout=60
        )
        ok_init3, _ = safe_call(adapter3.initialize)
        check("degraded_initialize_ok", ok_init3, "ok")
        ok4, receipt4 = safe_call(
            adapter3.submit_task,
            SubmitTaskRequest(
                intent="synthetic-multiturn",
                workspace_dir=str(root["cwd"]),
                prompt_text=TURN3,
                engine_session_id="synthetic-session-does-not-exist",
            ),
        )
        check("degraded_submit_does_not_raise", ok4, receipt4 if not ok4 else "ok")
        if ok4:
            check("degraded_note_explains", "新建会话" in receipt4.note, receipt4.note)
            check(
                "degraded_created_new_session",
                receipt4.engine_session_id not in (None, "synthetic-session-does-not-exist"),
                receipt4.engine_session_id,
            )
            counts4 = method_counts(adapter3.raw_sent_frames())
            check("degraded_used_new_session", counts4.get("session/new", 0) == 1, counts4)
            status4 = wait_terminal(adapter3, receipt4.omni_task_id)
            check("degraded_reached_success", status4 == TaskState.SUCCEEDED.value, status4)
    finally:
        if adapter3 is not None:
            artifacts["third_shutdown"] = adapter3.shutdown(mode="graceful", grace_period=5).to_dict()
        server.shutdown()
        H.drop_root(root)

    failed = [c for c in checks if not c["pass"]]
    payload = {
        "plan": "OD-ARCH-034",
        "evidence_level": "script (real engine process + loopback synthetic model endpoint)",
        "checks_total": len(checks),
        "checks_failed": len(failed),
        "checks": checks,
        "artifacts": artifacts,
        "result": "pass" if checks and not failed else "fail",
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(
            json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    print("checks: %d total, %d failed; RESULT: %s" % (len(checks), len(failed), payload["result"]))
    return 0 if payload["result"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
