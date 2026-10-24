#!/usr/bin/env python3
"""OD-ARCH-069 —— 工具可见化与超时治理场景（真实 DSH web 进程 + 合成端点）。

场景：
  V01 工具可见：工具调用期间适配层发出 tool_activity，且带工具名与状态。
  V02 显式设限（仅测试/脚本）：timeout_policy 显式给出的等待上限仍生效，
      到点落 unknown 且文案带秒数。生产默认不再下发该字段。
  V03 未知询问：非 approval 的 waterfall 只产生可见提示，不发任何应答帧。
  V04 台账边界：以上事件写入台账时不含工具入参/结果正文。
  V05 默认无上限：不设截止时静默只发 still_running 提示、状态不变；
      用户取消仍能正常落 cancelled。

安全边界：一次性 /tmp 数据根；零真实凭证；提示词与工具参数均为合成占位；
证据只保存归一化字段与判定，不保存 cookie / token / 工具正文。

用法：python3 run_visibility_scenarios.py [V01 V02 V03 V04]
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import _harness as H
from _harness import safe_call
from _mock_model_server import start_mock

from omnidesk_engine import SubmitTaskRequest, TaskState

EVIDENCE_DIR = H.ENGINE_INTERFACE_DIR / "evidence" / "web-api" / "visibility-scenarios"
TERMINAL = (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
            TaskState.INTERRUPTED, TaskState.UNKNOWN)


def _events(adapter, task_id):
    return [e.to_dict() for e in adapter.stream_events(
        task_id, until_terminal=False, timeout=0.0)]


def _wait_for(adapter, task_id, predicate, timeout=40.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for event in _events(adapter, task_id):
            if predicate(event):
                return event
        time.sleep(0.1)
    return None


def _wait_terminal(adapter, task_id, timeout=90.0):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        last = adapter.get_task_status(task_id)
        if last.status in TERMINAL:
            return last
        time.sleep(0.1)
    return last


def scenario(tag, title, body, *, mode, timeout_policy=None, default_timeout=90):
    root = H.new_root(tag)
    base_url, ctl, server = start_mock()
    ctl.set(mode=mode)
    record = {"scenario": tag, "title": title, "evidence_level": "script"}
    adapter = None
    try:
        overlay = H.write_overlay(root, tag, H.web_overlay_text(base_url))
        adapter = H.make_web_adapter(root, [overlay], init_timeout=60,
                                     session_timeout=45, default_timeout=default_timeout)
        ok, desc = safe_call(adapter.initialize)
        record["initialize_ok"] = ok
        if not ok:
            record["initialize_error"] = desc
            return record
        body(adapter, ctl, record, root, timeout_policy)
        return record
    except Exception as exc:  # noqa: BLE001
        record["unhandled"] = type(exc).__name__ + ": " + str(exc)
        return record
    finally:
        if adapter is not None:
            try:
                adapter.shutdown(mode="graceful", grace_period=5)
            except Exception:
                pass
        server.shutdown()
        H.drop_root(root)


def _submit(adapter, root, timeout_policy=None):
    return adapter.submit_task(SubmitTaskRequest(
        intent="visibility-scenario",
        workspace_dir=str(root["cwd"]),
        prompt_text="合成场景：请执行一次需要人工确认的工具调用。",
        timeout_policy=dict(timeout_policy or {})))


def v01(adapter, ctl, record, root, timeout_policy):
    receipt = _submit(adapter, root, timeout_policy)
    record["omni_task_id"] = receipt.omni_task_id
    event = _wait_for(
        adapter, receipt.omni_task_id,
        lambda e: e.get("type") == "tool_activity")
    record["tool_activity_received"] = event is not None
    if event is not None:
        payload = event.get("payload") or {}
        record["tool_event"] = {k: payload.get(k) for k in ("kind", "tool_name", "status")}
        record["tool_name_present"] = bool(payload.get("tool_name"))
        record["tool_name_is_synthetic"] = payload.get("tool_name") == ctl.tool_call_name
    approval = _wait_for(
        adapter, receipt.omni_task_id,
        lambda e: (e.get("payload") or {}).get("kind") == "approval_request")
    if approval is not None:
        safe_call(adapter.respond_approval, receipt.omni_task_id,
                  (approval.get("payload") or {}).get("approval_id"), True)
    final = _wait_terminal(adapter, receipt.omni_task_id)
    record["final_status"] = final.to_dict()
    record["pass"] = bool(event is not None and record.get("tool_name_present")
                          and record.get("tool_name_is_synthetic"))


def v02(adapter, ctl, record, root, timeout_policy):
    started = time.monotonic()
    receipt = _submit(adapter, root, timeout_policy)
    record["omni_task_id"] = receipt.omni_task_id
    final = _wait_terminal(adapter, receipt.omni_task_id, timeout=60)
    elapsed = time.monotonic() - started
    record["final_status"] = final.to_dict()
    record["elapsed_seconds"] = round(elapsed, 1)
    record["policy_seconds"] = (timeout_policy or {}).get("total_seconds")
    record["note"] = final.note
    record["note_has_seconds"] = str((timeout_policy or {}).get("total_seconds", "")) in (final.note or "")
    record["note_says_no_auto_retry"] = "未自动重试" in (final.note or "")
    asked = (timeout_policy or {}).get("total_seconds")
    record["pass"] = bool(
        final.status == TaskState.UNKNOWN
        and asked is not None
        and elapsed < asked + 30
        and record["note_has_seconds"] and record["note_says_no_auto_retry"])


def v03(adapter, ctl, record, root, timeout_policy):
    receipt = _submit(adapter, root, {"total_seconds": 12})
    record["omni_task_id"] = receipt.omni_task_id
    # 等事件流连上并认领会话后再注入一帧非 approval 询问。
    time.sleep(2.0)
    channel = adapter._approval_channel
    record["channel_open"] = channel is not None
    if channel is None:
        record["pass"] = False
        return record
    # 只注入帧，不发送任何应答。帧体为合成占位。
    channel._on_frame({
        "type": "waterfall",
        "event": "synthetic/unknown-ask",
        "eventId": "ask-synth-1",
        "agentId": receipt.engine_session_id or "",
        "payload": {"question": "synthetic placeholder question"},
    })
    event = _wait_for(
        adapter, receipt.omni_task_id,
        lambda e: (e.get("payload") or {}).get("kind") == "engine_ask", timeout=6)
    record["engine_ask_received"] = event is not None
    if event is not None:
        payload = event.get("payload") or {}
        record["ask_event"] = {k: payload.get(k) for k in ("kind", "event")}
        record["ask_type_is_original"] = payload.get("event") == "synthetic/unknown-ask"
        record["ask_payload_leaks_question"] = "synthetic placeholder question" in json.dumps(
            payload, ensure_ascii=False)
    # 任务仍在等（未自动应答）：本场景仍显式传 total_seconds=12，
    # 由这个显式截止落 unknown（生产默认不下发截止，见 V05）。
    final = _wait_terminal(adapter, receipt.omni_task_id, timeout=60)
    record["final_status"] = {"status": final.status.value, "note": final.note}
    record["still_waits_until_timeout"] = final.status == TaskState.UNKNOWN
    record["pass"] = bool(
        event is not None and record.get("ask_type_is_original")
        and not record.get("ask_payload_leaks_question")
        and record.get("still_waits_until_timeout"))


def v04(adapter, ctl, record, root, timeout_policy):
    receipt = _submit(adapter, root, timeout_policy)
    record["omni_task_id"] = receipt.omni_task_id
    approval = _wait_for(
        adapter, receipt.omni_task_id,
        lambda e: (e.get("payload") or {}).get("kind") == "approval_request")
    if approval is not None:
        safe_call(adapter.respond_approval, receipt.omni_task_id,
                  (approval.get("payload") or {}).get("approval_id"), True)
    _wait_terminal(adapter, receipt.omni_task_id, timeout=60)
    state_file = Path(root["state"]) / (receipt.omni_task_id + ".json")
    raw = state_file.read_text(encoding="utf-8") if state_file.exists() else ""
    record["ledger_exists"] = bool(raw)
    record["ledger_has_tool_activity"] = "tool_activity" in raw
    record["ledger_leaks_command"] = "echo synthetic-approval-probe" in raw
    record["ledger_leaks_reason"] = "synthetic approval probe, no real data" in raw
    record["pass"] = bool(record["ledger_exists"] and not record["ledger_leaks_command"]
                          and not record["ledger_leaks_reason"])


def v05(adapter, ctl, record, root, timeout_policy):
    """不设截止：长时间无输出只发提示，任务保持 running；用户取消照常生效。"""
    # 把静默提示间隔缩到 2 秒，避免测试等待一分钟。
    adapter._IDLE_NOTE_SECONDS = 2.0
    record["default_timeout"] = adapter._default_timeout
    receipt = _submit(adapter, root, timeout_policy)
    record["omni_task_id"] = receipt.omni_task_id
    note = _wait_for(
        adapter, receipt.omni_task_id,
        lambda e: (e.get("payload") or {}).get("kind") == "still_running",
        timeout=25)
    record["still_running_received"] = note is not None
    if note is not None:
        payload = note.get("payload") or {}
        record["still_running_event"] = {
            k: payload.get(k) for k in ("kind", "idle_seconds", "note")}
    time.sleep(8.0)
    mid = adapter.get_task_status(receipt.omni_task_id)
    record["state_after_8s_silence"] = mid.to_dict()
    still_running = mid.status not in TERMINAL
    t0 = time.monotonic()
    cancel = adapter.cancel_task(receipt.omni_task_id, timeout=20)
    record["cancel_elapsed_s"] = round(time.monotonic() - t0, 3)
    record["cancel_outcome"] = cancel.to_dict()
    final = _wait_terminal(adapter, receipt.omni_task_id, timeout=40)
    record["final_status"] = final.to_dict()
    record["pass"] = bool(
        record["default_timeout"] is None
        and record["still_running_received"]
        and still_running
        and final.status == TaskState.CANCELLED)


SCENARIOS = {
    "V01": ("工具调用期间 tool_activity 带工具名", v01, {"mode": "tool_call"}),
    "V02": ("超时按配置生效且文案带秒数", v02,
            {"mode": "hang", "timeout_policy": {"total_seconds": 8}}),
    "V03": ("未知 waterfall 询问只提示不应答", v03, {"mode": "hang"}),
    "V04": ("台账不写入工具正文", v04, {"mode": "tool_call"}),
    "V05": ("默认无上限：静默只提示，用户取消仍生效", v05,
            {"mode": "hang", "timeout_policy": {"total_seconds": 0},
             "default_timeout": None}),
}


def main() -> int:
    wanted = [a.upper() for a in sys.argv[1:]] or list(SCENARIOS)
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    results = []
    for key in wanted:
        title, body, opts = SCENARIOS[key]
        print("==", key, title)
        rec = scenario(key.lower(), title, body, **opts)
        results.append(rec)
        print("   pass=", rec.get("pass"), "final=",
              (rec.get("final_status") or {}).get("status"))
    out = EVIDENCE_DIR / "scenarios.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    print("evidence:", out)
    return 0 if all(r.get("pass") for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
