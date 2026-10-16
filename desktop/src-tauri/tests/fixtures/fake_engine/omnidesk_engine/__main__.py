#!/usr/bin/env python3
"""测试用假引擎：实现 IPC-2 的最小方法子集，行为完全确定，不接触任何真实资源。

用途：让 Rust 层（EngineChannel / EngineSupervisor）的集成测试不依赖真实 DSH 与模型。
它只实现：
  omni.initialize   -> 返回引擎描述符
  omni.submit_task  -> 返回任务回执，并推送一条 task_started 通知
  omni.echo         -> 原样回显 params（验证 request/response 对齐）
  omni.fail         -> 返回带五属性的引擎错误（验证错误传播）
  omni.shutdown     -> 返回关闭报告并退出
"""

from __future__ import annotations

import json
import sys
import time


def send(obj) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def notify(method, params) -> None:
    send({"jsonrpc": "2.0", "method": method, "params": params})


def main() -> int:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            frame = json.loads(line)
        except Exception:
            send({"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "parse"}})
            continue
        req_id = frame.get("id")
        method = frame.get("method")
        params = frame.get("params") or {}

        if method == "omni.initialize":
            send({
                "jsonrpc": "2.0", "id": req_id,
                "result": {
                    "protocol_version": "omnidesk-engine/1",
                    "server": "fake-engine",
                    "engine": {
                        "engine_kind": "fake-engine",
                        "engine_version": "0.0.0-test",
                        "protocol_version": "omnidesk-engine/1",
                        "compatibility": "compatible",
                        "healthy": True,
                        "capabilities": {"submit_task": "verified"},
                        "detail": {"fake": True},
                    },
                },
            })
        elif method == "omni.submit_task":
            task_id = "omni-test-task"
            send({"jsonrpc": "2.0", "id": req_id,
                  "result": {"omni_task_id": task_id, "status": "queued",
                             "accepted_at": time.time(), "note": "fake accepted"}})
            notify("omni.event", {"omni_task_id": task_id, "seq": 1,
                                  "occurred_at": time.time(), "type": "task_started",
                                  "payload": {}})
        elif method == "omni.echo":
            send({"jsonrpc": "2.0", "id": req_id, "result": {"echo": params}})
        elif method == "omni.fail":
            send({"jsonrpc": "2.0", "id": req_id, "error": {
                "code": -32000, "message": "task_failed",
                "data": {"engine_error_code": "task_failed",
                         "retryable": "conditional", "needs_human": "yes",
                         "degradable": "yes", "may_duplicate_execution": "yes",
                         "may_lose_output": "no", "omni_task_id": "omni-test-task"}}})
        elif method == "omni.slow":
            time.sleep(float(params.get("seconds", 5)))
            send({"jsonrpc": "2.0", "id": req_id, "result": {"slept": True}})
        elif method == "omni.shutdown":
            send({"jsonrpc": "2.0", "id": req_id,
                  "result": {"mode": "graceful", "in_flight_before": [], "cancelled": [],
                             "still_running": [], "connection_closed": True,
                             "process_exit_code": None, "forced_terminate": False,
                             "note": "fake shutdown"}})
            return 0
        else:
            send({"jsonrpc": "2.0", "id": req_id,
                  "error": {"code": -32601, "message": "method not found: " + str(method)}})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
