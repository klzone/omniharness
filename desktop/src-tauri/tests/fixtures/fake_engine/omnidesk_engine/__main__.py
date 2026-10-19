#!/usr/bin/env python3
"""测试用假引擎：实现 IPC-2 的最小方法子集，行为完全确定，不接触任何真实资源。

用途：让 Rust 层（EngineChannel / EngineSupervisor / 宿主生命周期）的集成测试
不依赖真实 DSH 与模型。它实现：
  omni.initialize      -> 返回引擎描述符
  omni.submit_task     -> 返回任务回执，并推送一条 task_started 通知
  omni.get_task_status -> 返回任务状态（可被取消改变）
  omni.get_task_result -> 返回任务结果
  omni.cancel_task     -> 记录取消并返回取消结果；只有收到本请求才算“已送达”
  omni.echo            -> 原样回显 params（验证 request/response 对齐）
  omni.fail            -> 返回带五属性的引擎错误（验证错误传播）
  omni.slow            -> 休眠指定秒数后返回（制造在途窗口）
  omni.shutdown        -> 返回关闭报告并退出

测试可观测性（均为可选环境变量，默认不启用）：
  OMNI_TEST_METHOD_LOG       收到的方法名按行追加到该文件（用于断言“取消是否送达”）
  OMNI_TEST_SHUTDOWN_DELAY_MS 处理 omni.shutdown 前先休眠，制造可预测的关闭窗口
"""

from __future__ import annotations

import json
import os
import sys
import time

METHOD_LOG = os.environ.get("OMNI_TEST_METHOD_LOG") or ""
SHUTDOWN_DELAY_MS = int(os.environ.get("OMNI_TEST_SHUTDOWN_DELAY_MS") or "0")

_tasks: dict[str, dict] = {}


def send(obj) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def notify(method, params) -> None:
    send({"jsonrpc": "2.0", "method": method, "params": params})


def log_method(method: str) -> None:
    if not METHOD_LOG:
        return
    try:
        with open(METHOD_LOG, "a", encoding="utf-8") as fh:
            fh.write(method + "\n")
    except OSError:
        pass


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
        log_method(str(method))

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
            _tasks[task_id] = {"state": "running", "cancel_phase": "not_requested"}
            send({"jsonrpc": "2.0", "id": req_id,
                  "result": {"omni_task_id": task_id, "status": "queued",
                             "accepted_at": time.time(), "note": "fake accepted"}})
            notify("omni.event", {"omni_task_id": task_id, "seq": 1,
                                  "occurred_at": time.time(), "type": "task_started",
                                  "payload": {}})
        elif method == "omni.get_task_status":
            task_id = str(params.get("omni_task_id") or "")
            task = _tasks.get(task_id)
            if task is None:
                send({"jsonrpc": "2.0", "id": req_id, "error": {
                    "code": -32000, "message": "unknown_task",
                    "data": {"engine_error_code": "unknown_task",
                             "omni_task_id": task_id}}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {
                    "omni_task_id": task_id, "status": task["state"],
                    "state_since": time.time(), "cancel_phase": task["cancel_phase"],
                    "note": "fake status"}})
        elif method == "omni.get_task_result":
            task_id = str(params.get("omni_task_id") or "")
            task = _tasks.get(task_id)
            if task is None:
                send({"jsonrpc": "2.0", "id": req_id, "error": {
                    "code": -32000, "message": "unknown_task",
                    "data": {"engine_error_code": "unknown_task",
                             "omni_task_id": task_id}}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {
                    "omni_task_id": task_id, "form": "complete", "integrity": "complete",
                    "content": "fake result content", "sources": [],
                    "persisted_at": None, "note": "fake result"}})
        elif method == "omni.cancel_task":
            task_id = str(params.get("omni_task_id") or "")
            task = _tasks.setdefault(task_id, {"state": "running",
                                               "cancel_phase": "not_requested"})
            task["state"] = "cancelled"
            task["cancel_phase"] = "confirmed_cancelled"
            send({"jsonrpc": "2.0", "id": req_id, "result": {
                "omni_task_id": task_id, "phase": "confirmed_cancelled",
                "requested_at": time.time(), "settled_at": time.time(),
                "final_state": "cancelled", "evidence": "fake cancel received",
                "note": "fake cancel"}})
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
            if SHUTDOWN_DELAY_MS > 0:
                # 制造可预测的“关闭进行中”窗口，供并发测试使用。
                time.sleep(SHUTDOWN_DELAY_MS / 1000.0)
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
