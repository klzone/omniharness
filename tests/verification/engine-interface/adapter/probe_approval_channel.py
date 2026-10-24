#!/usr/bin/env python3
"""OD-ARCH-066 S1 —— DSH 审批通道录制探针（合成端点、一次性数据根）。

目的：在写归一化代码之前，用真实 DSH web 进程确认
  * `$events` 事件流能否打开、首帧是否 `ready` 且带 clientId；
  * 触发一个需要审批的 bash 升级时，是否收到 `waterfall: approval/request`；
  * `agentId` 是否等于本任务的 engine_session_id；
  * `$events/result` 回 `allowed-once` / `rejected` 后，turn 如何落定。

证据只保留帧的**结构**（键名、事件名、outcome），不含 cookie / token / 命令正文。
用法：python3 probe_approval_channel.py
"""

from __future__ import annotations

import json
import sys
import threading
import time
from pathlib import Path

import _harness as H
from _harness import safe_call
from _mock_model_server import start_mock

from omnidesk_engine import SubmitTaskRequest
from omnidesk_engine.web_api_adapter import WsClient
from omnidesk_engine.contract import TaskState

EVIDENCE_DIR = H.ENGINE_INTERFACE_DIR / "evidence" / "web-api"
PROBE_DIR = EVIDENCE_DIR / "approval-probe"
TERMINAL = (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
            TaskState.INTERRUPTED, TaskState.UNKNOWN)


def _shape(value, depth=0):
    """把帧裁成纯结构：只留键与标量类型，字符串只保留短标识。"""
    if depth > 3:
        return "<deep>"
    if isinstance(value, dict):
        return {str(k): _shape(v, depth + 1) for k, v in sorted(value.items())}
    if isinstance(value, list):
        return [_shape(value[0], depth + 1)] if value else []
    if isinstance(value, str):
        return value if len(value) <= 64 else value[:16] + "…"
    return value


class EventsProbe:
    """常驻 `$events` 订阅，后台收帧。"""

    def __init__(self, root_url: str, cookie: str):
        ws_url = "ws://" + root_url[len("http://"):] + "/api/remote.mux"
        self.client = WsClient(ws_url, cookie, timeout=30.0)
        self.stream_id = "stream-probe-approval"
        self.frames: list = []
        self.ready: dict | None = None
        self.waterfalls = threading.Event()
        self._stop = threading.Event()
        self.client.send({"type": "open", "streamId": self.stream_id,
                          "endpoint": "$events", "payload": {"args": {}}})
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def _run(self):
        while not self._stop.is_set():
            try:
                message = self.client.recv(timeout=1.0)
            except TimeoutError:
                continue
            except (OSError, ValueError):
                return
            if message is None:
                return
            value = message.get("value")
            if not isinstance(value, dict):
                continue
            self.frames.append(value)
            if value.get("type") == "ready":
                self.ready = value
            if value.get("type") == "waterfall":
                self.waterfalls.set()

    def wait_approval(self, timeout=20.0):
        self.waterfalls.wait(timeout=timeout)
        for frame in self.frames:
            if frame.get("type") == "waterfall":
                return frame
        return None

    def close(self):
        self._stop.set()
        try:
            self.client.close()
        except Exception:
            pass


def run_case(tag: str, outcome: str, calls) -> dict:
    root = H.new_root(tag)
    base_url, ctl, server = start_mock()
    ctl.set(mode="tool_call")
    adapter = None
    probe = None
    record = {"case": tag, "decision": outcome, "evidence_level": "script"}
    try:
        overlay = H.write_overlay(root, tag, H.web_overlay_text(base_url))
        adapter = H.make_web_adapter(root, [overlay], init_timeout=60,
                                     session_timeout=45, default_timeout=90)
        ok, desc = safe_call(adapter.initialize)
        record["initialize_ok"] = ok
        if not ok:
            record["initialize_error"] = desc
            return record

        assert adapter._root and adapter._cookie
        probe = EventsProbe(adapter._root, adapter._cookie)
        deadline = time.monotonic() + 15
        while probe.ready is None and time.monotonic() < deadline:
            time.sleep(0.05)
        record["events_stream_ready"] = probe.ready is not None
        record["ready_frame_shape"] = _shape(probe.ready) if probe.ready else None
        if probe.ready is None:
            return record
        client_id = probe.ready.get("clientId")

        receipt = adapter.submit_task(SubmitTaskRequest(
            intent="probe-approval", workspace_dir=str(root["cwd"]),
            prompt_text="合成探针：请执行一次需要审批的工具调用。"))
        record["omni_task_id"] = receipt.omni_task_id
        record["engine_session_id"] = receipt.engine_session_id

        waterfall = probe.wait_approval(timeout=25)
        record["approval_request_received"] = waterfall is not None
        if waterfall is None:
            record["frames_seen"] = [_shape(f) for f in probe.frames[:20]]
            status = adapter.get_task_status(receipt.omni_task_id)
            record["status_without_approval"] = status.to_dict()
            return record

        record["waterfall_shape"] = _shape(waterfall)
        record["agent_id_equals_session_id"] = (
            str(waterfall.get("agentId")) == str(receipt.engine_session_id))
        request = waterfall.get("request") or {}
        record["approval_request_fields"] = sorted(str(k) for k in request.keys())
        record["approval_tool_name"] = request.get("toolName")
        record["approval_has_call_id"] = bool(request.get("callId"))
        record["approval_has_reason"] = bool(request.get("reason"))

        status_during = adapter.get_task_status(receipt.omni_task_id)
        record["status_while_waiting"] = status_during.to_dict()

        call_result = adapter._rpc("$events/result", {
            "clientId": client_id,
            "eventId": waterfall.get("eventId"),
            "outcome": {"kind": "result", "value": outcome},
        }, 20.0)
        record["result_rpc_value"] = call_result

        snaps = []
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            status = adapter.get_task_status(receipt.omni_task_id)
            snaps.append(status.to_dict())
            if status.status in TERMINAL:
                break
            time.sleep(0.2)
        record["final_status"] = snaps[-1] if snaps else None
        try:
            result = adapter.get_task_result(receipt.omni_task_id)
            record["final_result"] = {
                "form": getattr(result.form, "value", str(result.form)),
                "content_len": len(result.content or ""),
                "content": (result.content or "")[:200],
            }
        except Exception as exc:  # noqa: BLE001
            record["final_result_error"] = type(exc).__name__ + ": " + str(exc)
        record["model_requests"] = [
            {k: v for k, v in r.items() if k in ("message_count", "has_tool_message", "mode")}
            for r in ctl.new_requests()
        ]
        record["frames_after_decision"] = [
            _shape(f) for f in probe.frames if f.get("type") != "emit"][:10]
        return record
    except Exception as exc:  # noqa: BLE001
        record["unhandled"] = type(exc).__name__ + ": " + str(exc)
        return record
    finally:
        if probe is not None:
            probe.close()
        if adapter is not None:
            try:
                adapter.shutdown(mode="graceful", grace_period=5)
            except Exception:
                pass
        server.shutdown()
        H.drop_root(root)


def main() -> int:
    PROBE_DIR.mkdir(parents=True, exist_ok=True)
    cases = [run_case("allow", "allowed-once", None), run_case("reject", "rejected", None)]
    payload = {
        "plan": "OD-ARCH-066 S1",
        "purpose": "确认 DSH web 审批事件结构与应答通道（合成端点、一次性数据根）",
        "evidence_level": "script",
        "executed_at": time.time(),
        "cases": cases,
    }
    path = PROBE_DIR / "probe-facts.json"
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    print("evidence:", path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
