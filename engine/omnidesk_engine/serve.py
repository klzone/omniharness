"""OmniDesk 引擎子进程入口（IPC-2 服务端）。

职责：把 engine/omnidesk_engine/ 里已有的 AcpEngineAdapter 包成一个
**由 Rust 宿主启动的独立子进程**，与宿主按 docs/OmniDesk-Desktop-Engine-IPC.md
第 3 节的七类消息交换行分隔 JSON-RPC 2.0。

严格边界（见 IPC 契约 §0）：
  * 本进程只被 OmniDesk 宿主启动；不注册 systemd，不做常驻服务。
  * 只经 stdin/stdout 通信；不监听任何 TCP/UDP 端口。
  * 不修改、不复制 DSH Runtime；对 DSH 的所有调用都经适配层的 --profile acp CLI。
  * 不读取、不打印、不复制任何凭证；凭证仅由 DSH 自身在其 DSH_HOME 内读取。
  * ACP 内部字段（sessionId / stopReason / 方法名）不越过本层，只输出归一化字段。

事件投递：任务事件是**推送**（omni.event 通知），另提供 omni.stream_events
作拉取式补漏。两者都只是同一批 EngineEvent 的不同取用方式。

用法（由宿主启动，一般不手工调用）：
  python3 -m omnidesk_engine.serve
"""

from __future__ import annotations

import json
import os
import sys
import threading
import time
from pathlib import Path
from typing import Any, Mapping, Optional

if __package__ in (None, ""):  # 允许 python3 path/to/serve.py 直接跑
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    from omnidesk_engine import (
        AcpEngineAdapter,
        EngineError,
        EngineErrorCode,
        SubmitTaskRequest,
        TaskState,
    )
    from omnidesk_engine.contract import TERMINAL_STATES
else:
    from . import (
        AcpEngineAdapter,
        EngineError,
        EngineErrorCode,
        SubmitTaskRequest,
        TaskState,
    )
    from .contract import TERMINAL_STATES

PROTOCOL_VERSION = "omnidesk-engine/1"
SERVER_NAME = "omnidesk-engine"

# JSON-RPC 2.0 保留错误码
PARSE_ERROR = -32700
INVALID_REQUEST = -32600
METHOD_NOT_FOUND = -32601
INVALID_PARAMS = -32602
INTERNAL_ERROR = -32603
# 引擎域错误统一用 -32000，具体原因放在 data.engine_error_code
ENGINE_ERROR = -32000


def _log(message: str) -> None:
    """所有诊断信息只走 stderr；stdout 是协议通道，绝不能被污染。"""
    sys.stderr.write("[omnidesk-engine] " + message + "\n")
    sys.stderr.flush()


class EngineService:
    """IPC-2 服务端：把归一化方法映射到 AcpEngineAdapter。"""

    def __init__(self, *, stdin, stdout) -> None:
        self._stdin = stdin
        self._stdout = stdout
        self._write_lock = threading.Lock()
        self._adapter: Optional[AcpEngineAdapter] = None
        self._initialized = False
        self._cwd: Optional[str] = None
        self._stop = threading.Event()
        self._pumps: dict = {}
        self._pump_lock = threading.Lock()
        # 结果落盘根（大文本按路径返回）
        self._result_dir: Optional[str] = None

    # -- 传输 ---------------------------------------------------------------
    def _send(self, obj: Mapping[str, Any]) -> None:
        payload = json.dumps(obj, ensure_ascii=False)
        with self._write_lock:
            self._stdout.write(payload + "\n")
            self._stdout.flush()

    def _result(self, req_id: Any, result: Mapping[str, Any]) -> None:
        self._send({"jsonrpc": "2.0", "id": req_id, "result": dict(result)})

    def _error(self, req_id: Any, code: int, message: str, data: Any = None) -> None:
        err: dict = {"code": code, "message": message}
        if data is not None:
            err["data"] = data
        self._send({"jsonrpc": "2.0", "id": req_id, "error": err})

    def _notify(self, method: str, params: Mapping[str, Any]) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": dict(params)})

    # -- 错误归一化 ---------------------------------------------------------
    def _engine_error_data(self, error: EngineError) -> dict:
        data = dict(error.attributes)
        data["engine_error_code"] = (
            error.code.value if isinstance(error.code, EngineErrorCode) else str(error.code)
        )
        if error.omni_task_id:
            data["omni_task_id"] = error.omni_task_id
        if error.detail:
            data["detail"] = dict(error.detail)
        return data

    def _fail(self, req_id: Any, error: EngineError) -> None:
        self._error(req_id, ENGINE_ERROR, error.code.value, self._engine_error_data(error))

    def _reject(self, req_id: Any, code: EngineErrorCode, message: str, **detail) -> None:
        self._fail(
            req_id,
            EngineError(code, message, detail=dict(detail) if detail else None),
        )

    # -- 事件泵：把适配器事件推成 omni.event 通知 ---------------------------
    def _start_pump(self, omni_task_id: str) -> None:
        with self._pump_lock:
            if omni_task_id in self._pumps:
                return
            stop = threading.Event()
            self._pumps[omni_task_id] = stop

        adapter = self._adapter
        if adapter is None:
            return

        def run() -> None:
            try:
                for event in adapter.stream_events(omni_task_id, timeout=3600.0):
                    if stop.is_set():
                        break
                    self._notify("omni.event", event.to_dict())
            except EngineError as error:
                self._notify(
                    "omni.event",
                    {
                        "omni_task_id": omni_task_id,
                        "seq": -1,
                        "occurred_at": time.time(),
                        "type": "protocol_error",
                        "payload": self._engine_error_data(error),
                    },
                )
            except Exception as exc:  # pragma: no cover - 防御
                _log("event pump failed: " + type(exc).__name__ + ": " + str(exc))

        thread = threading.Thread(target=run, name="omni-pump-" + omni_task_id, daemon=True)
        thread.start()

    def _stop_pump(self, omni_task_id: str) -> None:
        with self._pump_lock:
            stop = self._pumps.pop(omni_task_id, None)
        if stop is not None:
            stop.set()

    # -- 方法实现 -----------------------------------------------------------
    def _require_adapter(self, req_id: Any) -> Optional[AcpEngineAdapter]:
        if self._adapter is None:
            self._reject(
                req_id,
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "engine is not initialized; call omni.initialize first",
            )
            return None
        return self._adapter

    def m_initialize(self, req_id: Any, params: Mapping[str, Any]) -> None:
        if self._initialized:
            descriptor = self._adapter.initialize() if self._adapter else None
            self._result(
                req_id,
                {
                    "protocol_version": PROTOCOL_VERSION,
                    "server": SERVER_NAME,
                    "note": "already initialized",
                    "engine": descriptor.to_dict() if descriptor else None,
                },
            )
            return

        protocol = params.get("protocol_version")
        if protocol != PROTOCOL_VERSION:
            self._reject(
                req_id,
                EngineErrorCode.PROTOCOL_INCOMPATIBLE,
                "protocol_version mismatch: host=" + str(protocol) + " engine=" + PROTOCOL_VERSION,
            )
            return

        dsh = params.get("dsh") or {}
        cli_entry = dsh.get("cli_entry")
        if not cli_entry or not os.path.isabs(str(cli_entry)):
            self._reject(
                req_id,
                EngineErrorCode.INVALID_ARGUMENT,
                "params.dsh.cli_entry must be an absolute path",
            )
            return

        requested_cwd = params.get("cwd")
        cwd = str(requested_cwd) if requested_cwd else os.getcwd()
        if not os.path.isabs(cwd):
            self._reject(req_id, EngineErrorCode.INVALID_ARGUMENT, "params.cwd must be absolute")
            return

        # 隔离根由宿主显式下发；本进程不猜测、不读取宿主环境里的 DSH_HOME。
        env = dict(os.environ)
        if dsh.get("dsh_home"):
            env["DSH_HOME"] = str(dsh["dsh_home"])
        if dsh.get("agents_home"):
            env["DSH_AGENTS_HOME"] = str(dsh["agents_home"])

        patch_files = [str(p) for p in (dsh.get("patch_files") or [])]
        adapter = AcpEngineAdapter.for_dsh(
            str(cli_entry),
            cwd=cwd,
            profile=str(dsh.get("profile") or "acp"),
            patch_files=patch_files,
            env=env,
            state_dir=params.get("engine_state_dir"),
        )
        self._cwd = cwd
        result_dir = params.get("result_dir") or params.get("engine_state_dir")
        self._result_dir = str(result_dir) if result_dir else None

        try:
            descriptor = adapter.initialize()
        except EngineError as error:
            # 初始化失败：不留半初始化状态
            try:
                adapter.shutdown(mode="force", grace_period=2.0)
            except Exception:
                pass
            self._fail(req_id, error)
            return

        self._adapter = adapter
        self._initialized = True
        self._result(
            req_id,
            {
                "protocol_version": PROTOCOL_VERSION,
                "server": SERVER_NAME,
                "engine": descriptor.to_dict(),
            },
        )

    def m_submit_task(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            request = SubmitTaskRequest(
                intent=str(params.get("intent") or ""),
                workspace_dir=str(params.get("workspace_dir") or ""),
                omni_task_id=params.get("omni_task_id"),
                idempotency_key=params.get("idempotency_key"),
                prompt_text=params.get("prompt_text"),
                input_file=params.get("input_file"),
                timeout_policy=params.get("timeout_policy") or {},
            )
            receipt = adapter.submit_task(request)
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._start_pump(receipt.omni_task_id)
        self._result(req_id, receipt.to_dict())

    def m_get_task_status(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            status = adapter.get_task_status(str(params.get("omni_task_id") or ""))
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, status.to_dict())

    def m_stream_events(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            after_seq = params.get("after_seq")
            events = [
                e.to_dict()
                for e in adapter.stream_events(
                    str(params.get("omni_task_id") or ""),
                    until_terminal=False,
                    include_backlog=True,
                    after_seq=int(after_seq) if after_seq is not None else None,
                )
            ]
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, {"events": events})

    def m_cancel_task(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            outcome = adapter.cancel_task(
                str(params.get("omni_task_id") or ""),
                timeout=float(params.get("timeout_seconds") or 15.0),
            )
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._stop_pump(str(params.get("omni_task_id") or ""))
        self._result(req_id, outcome.to_dict())

    def m_get_task_result(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            result = adapter.get_task_result(
                str(params.get("omni_task_id") or ""),
                require_persisted=bool(params.get("require_persisted", False)),
            )
        except EngineError as error:
            self._fail(req_id, error)
            return
        payload = result.to_dict()
        # 大文本按「落盘路径 + 元数据」返回，正文可通过 omni.read_result 按需取
        text = payload.get("content") or ""
        if self._result_dir and len(text) > 8192 and payload.get("persisted_at"):
            payload["content_omitted"] = True
            payload["content_length"] = len(text)
        self._result(req_id, payload)

    def m_get_task_result_text(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            result = adapter.get_task_result(str(params.get("omni_task_id") or ""))
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, {"content": result.content})

    def m_recover_session(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            recovery = adapter.recover_session(
                str(params.get("engine_session_id") or ""),
                cwd=params.get("cwd"),
            )
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, recovery.to_dict())

    def m_recover_task(self, req_id: Any, params: Mapping[str, Any]) -> None:
        adapter = self._require_adapter(req_id)
        if adapter is None:
            return
        try:
            recovery = adapter.recover_task(str(params.get("omni_task_id") or ""))
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, recovery.to_dict())

    def m_shutdown(self, req_id: Any, params: Mapping[str, Any]) -> None:
        if self._adapter is None:
            self._result(
                req_id,
                {
                    "mode": str(params.get("mode") or "graceful"),
                    "in_flight_before": [],
                    "cancelled": [],
                    "still_running": [],
                    "connection_closed": False,
                    "process_exit_code": None,
                    "forced_terminate": False,
                    "note": "引擎未启动，无连接可关闭",
                },
            )
            self._stop.set()
            return
        try:
            report = self._adapter.shutdown(
                mode=str(params.get("mode") or "graceful"),
                grace_period=float(params.get("grace_period_seconds") or 10.0),
            )
        except EngineError as error:
            self._fail(req_id, error)
            return
        self._result(req_id, report.to_dict())
        self._stop.set()

    # -- 主循环 -------------------------------------------------------------
    DISPATCH = {
        "omni.initialize": "m_initialize",
        "omni.submit_task": "m_submit_task",
        "omni.get_task_status": "m_get_task_status",
        "omni.stream_events": "m_stream_events",
        "omni.cancel_task": "m_cancel_task",
        "omni.get_task_result": "m_get_task_result",
        "omni.get_task_result_text": "m_get_task_result_text",
        "omni.recover_session": "m_recover_session",
        "omni.recover_task": "m_recover_task",
        "omni.shutdown": "m_shutdown",
    }

    def handle(self, frame: Mapping[str, Any]) -> None:
        req_id = frame.get("id")
        method = frame.get("method")
        params = frame.get("params") or {}

        # 宿主只应发 request（都带 id）；通知帧按通知处理
        handler_name = self.DISPATCH.get(str(method))
        if handler_name is None:
            if req_id is not None:
                self._error(req_id, METHOD_NOT_FOUND, "method not found: " + str(method))
            return
        if not isinstance(params, Mapping):
            self._error(req_id, INVALID_PARAMS, "params must be an object")
            return
        try:
            getattr(self, handler_name)(req_id, params)
        except EngineError as error:
            if req_id is not None:
                self._fail(req_id, error)
        except Exception as exc:
            _log("internal error in " + str(method) + ": " + type(exc).__name__ + ": " + str(exc))
            if req_id is not None:
                self._error(req_id, INTERNAL_ERROR, type(exc).__name__ + ": " + str(exc))

    def run(self) -> int:
        for line in self._stdin:
            line = line.strip()
            if not line:
                continue
            try:
                frame = json.loads(line)
            except Exception as exc:
                self._error(None, PARSE_ERROR, "invalid JSON: " + str(exc))
                continue
            if not isinstance(frame, Mapping):
                self._error(None, INVALID_REQUEST, "frame must be a JSON object")
                continue
            if self._stop.is_set():
                break
            self.handle(frame)
        # stdin 关闭 => 宿主退出 => 本进程收尾
        self._finalize()
        return 0

    def _finalize(self) -> None:
        with self._pump_lock:
            stops = list(self._pumps.values())
            self._pumps.clear()
        for stop in stops:
            stop.set()
        if self._adapter is not None:
            try:
                self._adapter.shutdown(mode="graceful", grace_period=5.0)
            except Exception as exc:
                _log("shutdown on exit failed: " + type(exc).__name__ + ": " + str(exc))


def main(argv: Optional[list] = None) -> int:
    _log("starting; pid=" + str(os.getpid()) + "; protocol=" + PROTOCOL_VERSION)
    service = EngineService(stdin=sys.stdin, stdout=sys.stdout)
    try:
        return service.run()
    except KeyboardInterrupt:  # pragma: no cover
        service._finalize()
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
