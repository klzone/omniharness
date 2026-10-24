#!/usr/bin/env python3
"""OD-ARCH-066 —— 审批链路端到端场景（真实 DSH web 进程 + 合成模型端点）。

场景：
  A01 允许：审批卡片事件到达 → 允许 → 任务继续并完成。
  A02 拒绝：审批卡片事件到达 → 拒绝 → turn 按引擎语义落定（如实记录，不预设失败）。
  A03 等待态：审批未答复时状态里出现「等待审批中」与 pending_approvals 计数。
  A04 非法答复：非本任务的审批编号被拒绝，且不误改任务状态。
  A05 TTFB：任务台账里出现 ttfb_seconds，且为首个文本增量时的时间。

安全边界：一次性 /tmp 数据根；零真实凭证；提示词与工具参数均为合成占位；
证据只保存归一化字段与判定，不保存 cookie / token / 工具正文。

用法：python3 run_approval_scenarios.py [A01 A02 ...]
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import _harness as H
from _harness import safe_call
from _mock_model_server import start_mock

from omnidesk_engine import EngineError, SubmitTaskRequest, TaskState

EVIDENCE_DIR = H.ENGINE_INTERFACE_DIR / "evidence" / "web-api" / "approval-scenarios"
TERMINAL = (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
            TaskState.INTERRUPTED, TaskState.UNKNOWN)


def _events(adapter, task_id):
    # until_terminal=False 是「持续等待新事件」的流式语义；轮询快照要给 timeout=0
    # 才会在吐完积压后立即返回，否则这里会永久阻塞。
    return [e.to_dict() for e in adapter.stream_events(
        task_id, until_terminal=False, timeout=0.0)]


def _wait_approval(adapter, task_id, timeout=30.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for event in _events(adapter, task_id):
            payload = event.get("payload") or {}
            if payload.get("kind") == "approval_request":
                return payload
        time.sleep(0.1)
    return None


def _wait_terminal(adapter, task_id, timeout=60.0):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        last = adapter.get_task_status(task_id)
        if last.status in TERMINAL:
            return last
        time.sleep(0.1)
    return last


def scenario(tag, title, body):
    root = H.new_root(tag)
    base_url, ctl, server = start_mock()
    ctl.set(mode="tool_call")
    record = {"scenario": tag, "title": title, "evidence_level": "script"}
    adapter = None
    try:
        overlay = H.write_overlay(root, tag, H.web_overlay_text(base_url))
        adapter = H.make_web_adapter(root, [overlay], init_timeout=60,
                                     session_timeout=45, default_timeout=90)
        ok, desc = safe_call(adapter.initialize)
        record["initialize_ok"] = ok
        if not ok:
            record["initialize_error"] = desc
            return record
        body(adapter, ctl, record, root)
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


def _submit(adapter, root):
    return adapter.submit_task(SubmitTaskRequest(
        intent="approval-scenario", workspace_dir=str(root["cwd"]),
        prompt_text="合成场景：请执行一次需要人工确认的工具调用。"))


def a01(adapter, ctl, record, root):
    receipt = _submit(adapter, root)
    record["omni_task_id"] = receipt.omni_task_id
    payload = _wait_approval(adapter, receipt.omni_task_id)
    record["approval_event_received"] = payload is not None
    if payload is None:
        record["final_status"] = _wait_terminal(adapter, receipt.omni_task_id, 20).to_dict()
        return
    record["approval_event"] = {
        k: payload.get(k) for k in ("kind", "tool_name", "call_id") if k in payload
    }
    record["approval_reason_present"] = bool(payload.get("reason"))
    status = adapter.get_task_status(receipt.omni_task_id)
    record["status_while_waiting"] = {
        "status": status.status.value,
        "pending_approvals": status.pending_approvals,
        "note": status.note,
    }
    record["note_mentions_waiting"] = "等待审批中" in status.note
    ok, desc = safe_call(adapter.respond_approval, receipt.omni_task_id,
                         payload["approval_id"], True)
    record["respond_ok"] = ok
    record["respond_error"] = None if ok else desc
    final = _wait_terminal(adapter, receipt.omni_task_id)
    record["final_status"] = final.to_dict()
    result = adapter.get_task_result(receipt.omni_task_id)
    record["final_text"] = (result.content or "")[:120]
    record["ttfb_seconds"] = final.ttfb_seconds
    decided = [e for e in _events(adapter, receipt.omni_task_id)
               if (e.get("payload") or {}).get("kind") == "approval_decided"]
    record["decided_event_decision"] = (
        (decided[-1].get("payload") or {}).get("decision") if decided else None)
    record["model_requests"] = [
        {k: r.get(k) for k in ("message_count", "has_tool_message")}
        for r in ctl.new_requests()
    ]
    record["pass"] = bool(
        payload is not None and ok and final.status == TaskState.SUCCEEDED
        and record["note_mentions_waiting"] and record["decided_event_decision"] == "allowed"
        and final.ttfb_seconds is not None)


def a02(adapter, ctl, record, root):
    receipt = _submit(adapter, root)
    record["omni_task_id"] = receipt.omni_task_id
    payload = _wait_approval(adapter, receipt.omni_task_id)
    record["approval_event_received"] = payload is not None
    if payload is None:
        record["final_status"] = _wait_terminal(adapter, receipt.omni_task_id, 20).to_dict()
        record["pass"] = False
        return
    ok, desc = safe_call(adapter.respond_approval, receipt.omni_task_id,
                         payload["approval_id"], False)
    record["respond_ok"] = ok
    record["respond_error"] = None if ok else desc
    final = _wait_terminal(adapter, receipt.omni_task_id)
    record["final_status"] = final.to_dict()
    result = adapter.get_task_result(receipt.omni_task_id)
    record["final_text"] = (result.content or "")[:120]
    record["pending_after"] = final.pending_approvals
    record["pass"] = bool(ok and final.status in TERMINAL and final.pending_approvals == 0)


def a03(adapter, ctl, record, root):
    """等待态：任务台账/状态在有审批时给出可观察的等待标记。"""
    receipt = _submit(adapter, root)
    payload = _wait_approval(adapter, receipt.omni_task_id)
    record["approval_event_received"] = payload is not None
    if payload is None:
        record["pass"] = False
        return
    status = adapter.get_task_status(receipt.omni_task_id)
    record["status"] = status.status.value
    record["pending_approvals"] = status.pending_approvals
    record["note"] = status.note
    # 落盘里只应有计数，不应出现命令正文或原因原文。
    state_file = Path(root["state"]) / (receipt.omni_task_id + ".json")
    raw = state_file.read_text(encoding="utf-8") if state_file.exists() else ""
    record["ledger_has_pending_count"] = "pending_approvals" in raw
    record["ledger_leaks_command_text"] = "echo synthetic-approval-probe" in raw
    record["ledger_leaks_reason_text"] = "synthetic approval probe, no real data" in raw
    safe_call(adapter.respond_approval, receipt.omni_task_id, payload["approval_id"], False)
    _wait_terminal(adapter, receipt.omni_task_id, 40)
    record["pass"] = bool(
        payload is not None and status.pending_approvals == 1
        and "等待审批中" in status.note and record["ledger_has_pending_count"]
        and not record["ledger_leaks_command_text"] and not record["ledger_leaks_reason_text"])


def a04(adapter, ctl, record, root):
    receipt = _submit(adapter, root)
    payload = _wait_approval(adapter, receipt.omni_task_id)
    record["approval_event_received"] = payload is not None
    if payload is None:
        record["pass"] = False
        return
    # 伪造一个不存在的审批编号：必须被拒绝且不消耗真实待审批项。
    ok, desc = safe_call(adapter.respond_approval, receipt.omni_task_id, "ev-not-real", True)
    record["bogus_approval_rejected"] = not ok
    record["bogus_error"] = desc
    status = adapter.get_task_status(receipt.omni_task_id)
    record["pending_after_bogus"] = status.pending_approvals
    safe_call(adapter.respond_approval, receipt.omni_task_id, payload["approval_id"], False)
    _wait_terminal(adapter, receipt.omni_task_id, 40)
    record["pass"] = bool(not ok and status.pending_approvals == 1
                          and "not pending" in str(desc))


def a05(adapter, ctl, record, root):
    ctl.set(mode="ok")
    receipt = _submit(adapter, root)
    final = _wait_terminal(adapter, receipt.omni_task_id)
    record["final_status"] = final.to_dict()
    record["ttfb_seconds"] = final.ttfb_seconds
    deltas = [e for e in _events(adapter, receipt.omni_task_id) if e.get("type") == "text_delta"]
    # 首个 text_delta 的事件时间应当不早于 ttfb 起点；只核验数值存在且合理。
    record["delta_events"] = len(deltas)
    record["ttfb_is_positive"] = isinstance(final.ttfb_seconds, float) and final.ttfb_seconds >= 0
    state_file = Path(root["state"]) / (receipt.omni_task_id + ".json")
    raw = state_file.read_text(encoding="utf-8") if state_file.exists() else ""
    record["ledger_has_ttfb"] = "ttfb_seconds" in raw
    record["pass"] = bool(final.status == TaskState.SUCCEEDED and record["ttfb_is_positive"]
                          and record["ledger_has_ttfb"] and record["delta_events"] > 0)


SCENARIOS = {
    "A01": ("允许审批后任务继续并完成", a01),
    "A02": ("拒绝审批后按引擎语义落定", a02),
    "A03": ("等待审批时状态可见且台账不泄露正文", a03),
    "A04": ("伪造审批编号被拒绝且不消耗待办", a04),
    "A05": ("首字延迟写入台账", a05),
}


def main() -> int:
    wanted = [a.upper() for a in sys.argv[1:]] or list(SCENARIOS)
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    results = []
    for key in wanted:
        title, body = SCENARIOS[key]
        print("==", key, title)
        rec = scenario(key.lower(), title, body)
        results.append(rec)
        print("   pass=", rec.get("pass"), "final=",
              (rec.get("final_status") or {}).get("status"))
    out = EVIDENCE_DIR / "scenarios.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    print("evidence:", out)
    return 0 if all(r.get("pass") for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
