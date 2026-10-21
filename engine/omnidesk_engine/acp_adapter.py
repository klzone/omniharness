"""OmniDesk 最小 ACP 引擎适配器（可丢弃技术原型）。

对应文档：
  * docs/OmniDesk-Engine-Interface-Contract.md   第 4 节九组最小接口
  * docs/OmniDesk-Engine-Interface-Test-Matrix.md  E01-E20

边界：
  * 只使用 Python 标准库；不修改 DSH Runtime；只通过公开 CLI 入口 --profile acp 通信。
  * 不读取、不打印、不复制任何凭证：凭证只由引擎自身在其 DSH_HOME 内读取。
  * ACP 内部消息结构不越过本层：上层只看到归一化事件、状态与错误。

已由源码证实（static）的 ACP 语义，本层据此实现：
  * session/prompt 只返回 stopReason；最终文本必须由 session/update 的
    agent_message_chunk 拼装。因此“任务结果”是 OmniDesk 自己组装的，
    OmniDesk 必须自己保证拼接的完整性与去重（不允许假定底层可靠投递）。
  * turnEndToStopReason：completed -> end_turn，max-tokens -> max_tokens，
    aborted -> end_turn，interrupted -> cancelled，blocked/error -> end_turn。
    因此 stopReason=end_turn 无法区分「正常完成」与「被阻断（blocked）」，
    这是本通道的已知语义缺口，适配层如实记录，不伪造更细的判定。
  * session/cancel 是通知帧（没有响应帧）。取消的可靠证据只能来自 prompt
    以 stopReason=cancelled 落定；否则只能报告 request_sent 或 timeout_unknown。
  * session/close 返回空对象；关闭后仍可 session/resume。
  * session/update 的 sessionUpdate 取值：agent_thought_chunk、
    agent_message_chunk、tool_call、tool_call_update、usage_update、
    config_option_update。
  * 引擎可发起 session/request_permission 客户端请求，客户端必须应答，
    否则该轮会一直等下去。

证据等级约定：static / script / model / unverified。
"""

from __future__ import annotations

import json
import os
import hashlib
import threading
import time
from pathlib import Path
from typing import Any, Iterator, Mapping, Optional, Sequence

from .contract import (
    TERMINAL_STATES,
    CapabilityState,
    CancelPhase,
    EngineCapability,
    EngineDescriptor,
    EngineError,
    EngineErrorCode,
    EventType,
    TaskState,
    new_omni_task_id,
    now,
)
from .task_core import (
    CancelOutcome,
    SessionListing,
    SessionRecovery,
    SessionSummary,
    ShutdownReport,
    SubmitTaskReceipt,
    SubmitTaskRequest,
    TaskCoreBase,
    TaskLedgerListing,
    TaskRecordSummary,
    TaskRecovery,
    _TaskRecord,
)
from .jsonrpc import JsonRpcPeer, PeerExit

__all__ = [
    "ACP_PROTOCOL_VERSION",
    "CLIENT_CAPS",
    "AcpEngineAdapter",
    "CancelOutcome",
    "SessionRecovery",
    "ShutdownReport",
    "SubmitTaskReceipt",
    "SubmitTaskRequest",
    "TaskRecovery",
]

ACP_PROTOCOL_VERSION = 1


def canonical_cwd(path: str, *, field: str = "cwd") -> str:
    """把工作目录收敛到唯一物理身份（单一规范化落点）。

    为什么需要：会话**存储**按原始 cwd 字符串分组（projectKey），而工作区注册表与
    ACP list/resume 按 realpath 分组。同一物理目录的别名写法（符号链接、多余斜杠、`..`）
    会被分成两个存储项目目录。在提交/恢复前统一 realpath 化，使两边身份一致。

    不可解析的路径（不存在或无法解析）按契约明确报错，不静默退化为字符串相等。
    """
    if not isinstance(path, str) or not os.path.isabs(path):
        raise EngineError(
            EngineErrorCode.INVALID_ARGUMENT,
            field + " must be an absolute path",
            detail={field: path},
        )
    try:
        return os.path.realpath(path, strict=True)
    except OSError as exc:
        raise EngineError(
            EngineErrorCode.INVALID_ARGUMENT,
            field + " does not resolve to an existing directory: " + str(exc),
            detail={field: path},
        ) from exc


def workspace_label_for(cwd: str, *, ordinal: int) -> str:
    """给界面一个稳定但不含目录名的展示标识。

    目录名本身可能含项目名或人名，因此连 basename 也不显示；只用序号
    加路径指纹前缀，既能区分不同工作区，又不泄露路径信息。
    """
    digest = hashlib.sha256(os.path.realpath(cwd).encode("utf-8")).hexdigest()[:8]
    return "工作区 " + str(ordinal) + " · " + digest


def workspace_token_for(cwd: str) -> str:
    """与 label 独立的不透明标识，用于把恢复请求绑定到用户看到的那一项。"""
    return hashlib.sha256(os.path.realpath(cwd).encode("utf-8")).hexdigest()[:16]


# 客户端能力声明：本原型不授予引擎读写本地文件的能力（最小权限）。
CLIENT_CAPS: Mapping[str, Any] = {"fs": {"readTextFile": False, "writeTextFile": False}}

# 适配层声明（4.1）。这些是「OmniDesk 侧声明」，与 Phase 3 运行验证结论分开记录；
# 只有 E01-E20 里真实跑过的才算 verified，本映射在报告中同步更新。
ADAPTER_CAPABILITIES: Mapping[EngineCapability, CapabilityState] = {
    EngineCapability.SUBMIT_TASK: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.GET_TASK_STATUS: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.STREAM_EVENTS: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.CANCEL_TASK: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.RECOVER_SESSION: CapabilityState.IMPLEMENTATION_SUPPORTED,
    # 恢复会话不等于恢复任务：跨进程只能重建「已知映射」，无法重建未落盘的轮次。
    EngineCapability.RECOVER_TASK: CapabilityState.PARTIAL,
    EngineCapability.GET_TASK_RESULT: CapabilityState.IMPLEMENTATION_SUPPORTED,
}


class AcpEngineAdapter(TaskCoreBase):
    """把 ACP v1（JSON-RPC stdio）映射为 OmniDesk 最小引擎契约的可替换适配器。

    只暴露归一化结果；调用方（未来的 UI / 领域层）看不到 ACP 字段名。
    """

    def __init__(
        self,
        *,
        command: Sequence[str],
        cwd: str,
        env: Optional[Mapping[str, str]] = None,
        state_dir: Optional[str] = None,
        permission_policy: str = "auto_reject_once",
        default_timeout: float = 120.0,
        session_timeout: float = 60.0,
        init_timeout: float = 30.0,
        engine_profile: Optional[str] = None,
    ) -> None:
        if not command:
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "command must be a non-empty list")
        if not os.path.isabs(cwd):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "cwd must be an absolute path")
        if permission_policy not in ("auto_allow_once", "auto_reject_once", "record_only"):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "unknown permission_policy")

        self._command = list(command)
        self._cwd = cwd
        self._env = dict(env) if env is not None else None
        super().__init__(state_dir=state_dir, default_timeout=default_timeout)
        self._permission_policy = permission_policy
        self._session_timeout = float(session_timeout)
        self._init_timeout = float(init_timeout)
        # 当前配置的 DSH Profile 名。只用于**报错可读性**（指出是哪个 Profile 没应答），
        # 不参与任何行为判定；也绝不据此猜测 Profile 能力（不读 DSH 私有 bundle 清单）。
        self._engine_profile = (
            str(engine_profile).strip() if engine_profile not in (None, "") else None
        )

        self._peer: Optional[JsonRpcPeer] = None
        self._spawn_error: Optional[EngineError] = None
        self._id_counter = 0
        self._sent_ids: set = set()
        # 会话列表产生的不透明工作区标识 → 真实目录。仅进程内使用，不回传路径。
        self._workspace_tokens: dict = {}
        self._workspace_labels: dict = {}
        self._unmatched_frames: list = []
        self._orphan_updates: list = []
        self._permission_requests: list = []

    # -- 构造便利方法 -------------------------------------------------------
    @classmethod
    def for_dsh(
        cls,
        dsh_bin: str,
        *,
        cwd: str,
        profile: str = "acp",
        patch_files: Sequence[str] = (),
        env: Optional[Mapping[str, str]] = None,
        **kwargs: Any,
    ) -> "AcpEngineAdapter":
        cmd = ["node", dsh_bin, "--profile", profile]
        for p in patch_files:
            cmd.extend(["--patch", p])
        kwargs.setdefault("engine_profile", profile)
        return cls(command=cmd, cwd=cwd, env=env, **kwargs)

    # -- 报错可读性（只影响 message/detail 文本，不参与行为判定）-------------

    def _profile_phrase(self, *, for_handshake: bool) -> str:
        """把「所配 Profile」写进错误文本；没有 Profile 信息时返回空串。"""
        if not self._engine_profile:
            return ""
        name = json.dumps(self._engine_profile)
        if for_handshake:
            return (
                ": OmniDesk expects profile "
                + name
                + " to speak ACP over stdio, but it never completed the ACP handshake"
            )
        return ": profile " + name + " is not responding on the ACP channel"

    def _profile_detail(self, method: str, *, handshake: bool) -> dict:
        """结构化补充：所配 Profile、OmniDesk 预期的协议，以及可读的一句话原因。

        说明：serve.py 的 JSON-RPC 错误帧把 `message` 字段填成错误码（如 "timeout"），
        `EngineError.message` 并不会原样过线。因此可读解释必须放进 `detail`，
        否则调用方只会看到一个光秃禿的 "timeout"。这里不改错误码、不改服务端错误映射。
        """
        detail: dict = {"method": method, "expected_protocol": "acp"}
        if self._engine_profile:
            detail["engine_profile"] = self._engine_profile
        if handshake:
            detail["handshake"] = "acp/initialize"
        if self._engine_profile and handshake:
            detail["reason"] = (
                "OmniDesk expects DSH profile "
                + self._engine_profile
                + " to speak ACP over stdio, but it never completed the ACP handshake "
                "within the bounded wait; a profile that does not serve ACP (for example "
                "a web-UI bundle) starts up but never answers the handshake"
            )
        elif handshake:
            detail["reason"] = (
                "OmniDesk expects the configured DSH profile to speak ACP over stdio, "
                "but the ACP handshake was not answered within the bounded wait"
            )
        else:
            detail["reason"] = (
                "no response arrived on the ACP channel before the bounded wait ended"
            )
        return detail

    # -- 进程生命周期 -------------------------------------------------------
    def _ensure_peer(self) -> JsonRpcPeer:
        with self._lock:
            if self._peer is not None:
                return self._peer
            if self._spawn_error is not None:
                raise self._spawn_error
            try:
                peer = JsonRpcPeer(
                    self._command, cwd=self._cwd, env=self._env, on_frame=self._on_frame
                )
            except Exception as exc:
                err = EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "failed to start engine process: " + type(exc).__name__ + ": " + str(exc),
                    detail={"command": self._command},
                )
                self._spawn_error = err
                raise err
            self._peer = peer
            return peer

    def _next_id(self) -> str:
        with self._lock:
            self._id_counter += 1
            rid = "omni-req-" + str(self._id_counter)
            self._sent_ids.add(rid)
            return rid

    def _request(
        self,
        method: str,
        params: Mapping[str, Any],
        timeout: float,
        omni_task_id: Optional[str] = None,
    ) -> Mapping[str, Any]:
        peer = self._ensure_peer()
        rid = self._next_id()
        try:
            peer.request(rid, method, params)
        except PeerExit as exc:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE, str(exc), omni_task_id=omni_task_id
            )
        frame = peer.wait_response(rid, timeout)
        if frame is None:
            code = peer.proc.poll()
            if code is not None:
                raise EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "engine exited (rc=" + str(code) + ") before responding to " + method,
                    detail={"stderr_tail": peer.stderr_text()[-2000:]},
                    omni_task_id=omni_task_id,
                )
            # 进程还活着但一句话都不说：最典型的原因是所配 Profile 不是 ACP Profile
            # （例如 web bundle：它会起来，却永不回应 stdio 上的 ACP 握手）。
            # 这里只把「所配 Profile + 预期协议」写清楚，让错误可读、可区分；
            # 判定依旧只看「有界等待内是否收到响应」，不读 DSH 私有 Profile 结构。
            handshake = method == "initialize"
            raise EngineError(
                EngineErrorCode.TIMEOUT,
                method
                + " did not respond within "
                + str(timeout)
                + "s"
                + self._profile_phrase(for_handshake=handshake),
                detail=self._profile_detail(method, handshake=handshake),
                omni_task_id=omni_task_id,
            )
        if "error" in frame:
            raise self._map_rpc_error(frame["error"], method, omni_task_id)
        result = frame.get("result")
        return result if isinstance(result, Mapping) else {}

    def _map_rpc_error(
        self, error: Mapping[str, Any], method: str, omni_task_id: Optional[str]
    ) -> EngineError:
        code = error.get("code")
        message = str(error.get("message") or "")
        low = message.lower()
        detail = {"rpc_code": code, "rpc_message": message, "method": method}
        if "is not resumable" in low or "unknown session" in low:
            mapped = EngineErrorCode.UNKNOWN_SESSION
        elif "session is already active" in low:
            mapped = EngineErrorCode.INVALID_ARGUMENT
        elif "session cwd does not match" in low:
            mapped = EngineErrorCode.INVALID_ARGUMENT
        elif "not supported" in low or code == -32601:
            mapped = EngineErrorCode.UNSUPPORTED
        elif "turn failed" in low:
            mapped = EngineErrorCode.TASK_FAILED
        elif "assistant output delivery failed" in low:
            mapped = EngineErrorCode.TASK_FAILED
        elif "bridge has been disposed" in low or "connection closed" in low:
            mapped = EngineErrorCode.ENGINE_UNAVAILABLE
        elif code == -32602:
            mapped = EngineErrorCode.INVALID_ARGUMENT
        elif code == -32603:
            mapped = EngineErrorCode.TASK_FAILED
        else:
            mapped = EngineErrorCode.ENGINE_UNAVAILABLE
        return EngineError(mapped, message or "engine returned an error", detail=detail, omni_task_id=omni_task_id)

    # -- 读侧分发 -----------------------------------------------------------
    def _on_frame(self, frame: Mapping[str, Any]) -> None:
        try:
            if "method" in frame and "id" in frame:
                self._handle_engine_request(frame)
                return
            if "method" in frame and "id" not in frame:
                if frame.get("method") == "session/update":
                    params = frame.get("params") or {}
                    if isinstance(params, Mapping):
                        self._handle_update(params)
                    return
                with self._lock:
                    self._unmatched_frames.append(dict(frame))
                return
            with self._lock:
                if frame.get("id") in self._sent_ids:
                    return
                self._unmatched_frames.append(dict(frame))
        except Exception as exc:  # 读线程不得因分发异常而中断
            with self._lock:
                self._unmatched_frames.append(
                    {"_dispatch_error": type(exc).__name__ + ": " + str(exc)}
                )

    def _handle_engine_request(self, frame: Mapping[str, Any]) -> None:
        peer = self._peer
        method = frame.get("method")
        rid = frame.get("id")
        params = frame.get("params") or {}
        if peer is None:
            return
        if method == "session/request_permission":
            options = params.get("options") or []
            chosen: Optional[str] = None
            if self._permission_policy == "auto_allow_once":
                for opt in options:
                    if opt.get("kind") in ("allow_once", "allow_always"):
                        chosen = opt.get("optionId")
                        break
                if chosen is None and options:
                    chosen = options[0].get("optionId")
            elif self._permission_policy == "auto_reject_once":
                for opt in options:
                    if opt.get("kind") in ("reject_once", "reject_always"):
                        chosen = opt.get("optionId")
                        break
            outcome = (
                {"outcome": "selected", "optionId": chosen}
                if chosen is not None
                else {"outcome": "cancelled"}
            )
            try:
                peer.respond(rid, {"outcome": outcome})
            except PeerExit:
                return
            call_id = ((params.get("toolCall") or {}) or {}).get("toolCallId")
            record = self._task_for_session(params.get("sessionId"))
            self._permission_requests.append(
                {"session_id": params.get("sessionId"), "tool_call_id": call_id, "outcome": outcome,
                 "policy": self._permission_policy}
            )
            if record is not None:
                self._emit(
                    record,
                    EventType.TOOL_ACTIVITY,
                    {
                        "kind": "permission_request",
                        "tool_call_id": call_id,
                        "policy": self._permission_policy,
                        "decision": outcome.get("outcome"),
                        "note": "权限请求来自引擎；本原型未把它作为一等契约，仅记录归一化事件",
                    },
                )
            return
        try:
            peer.respond_error(rid, -32601, "unsupported client method: " + str(method))
        except PeerExit:
            pass
        with self._lock:
            self._unmatched_frames.append({"_unsupported_client_method": str(method)})

    def _task_for_session(self, session_id: Optional[str]) -> Optional[_TaskRecord]:
        if not session_id:
            return None
        with self._lock:
            return self._sessions.get(session_id)

    def _handle_update(self, params: Mapping[str, Any]) -> None:
        session_id = params.get("sessionId")
        record = self._task_for_session(session_id)
        if record is None:
            with self._lock:
                self._orphan_updates.append(dict(params))
            return
        update = params.get("update")
        if not isinstance(update, Mapping):
            self._emit(
                record,
                EventType.PROTOCOL_ERROR,
                {"reason": "session/update 缺少可解析的 update 对象"},
            )
            return
        kind = update.get("sessionUpdate")
        if kind == "agent_message_chunk":
            content = update.get("content") or {}
            if not isinstance(content, Mapping) or content.get("type") != "text":
                self._emit(
                    record,
                    EventType.PROGRESS,
                    {"kind": "message_chunk_non_text", "content_type": str((content or {}).get("type"))},
                )
                return
            text = str(content.get("text") or "")
            if not text:
                return
            message_id = update.get("messageId")
            key = ("message", str(message_id), text)
            with self._lock:
                if key in record.dedup_seen:
                    record.dedup_dropped += 1
                    return
                record.dedup_seen.add(key)
                record.text_parts.append(text)
                record.text_len += len(text)
            self._emit(
                record,
                EventType.TEXT_DELTA,
                {"text": text, "message_id": message_id, "chars": len(text)},
            )
        elif kind == "agent_thought_chunk":
            content = update.get("content") or {}
            text = str((content or {}).get("text") or "") if isinstance(content, Mapping) else ""
            # 思考内容**不计入最终结果**，但要透出给界面（用户要在对话里看到思考过程）。
            # 文本本身是模型产出的思考，不含凭据；仍然按「思考」单独标注，不与最终正文混在一起。
            self._emit(
                record,
                EventType.PROGRESS,
                {
                    "kind": "thought_delta",
                    "chars": len(text),
                    "text": text,
                    "note": "思考内容不计入最终结果",
                },
            )
        elif kind == "tool_call":
            payload = {
                "kind": "tool_call",
                "tool_call_id": update.get("toolCallId"),
                "title": update.get("title"),
                "status": update.get("status"),
                "note": "工具被调用不等于工具已成功执行",
            }
            self._emit(record, EventType.TOOL_ACTIVITY, payload)
        elif kind == "tool_call_update":
            self._emit(
                record,
                EventType.TOOL_ACTIVITY,
                {
                    "kind": "tool_call_update",
                    "tool_call_id": update.get("toolCallId"),
                    "status": update.get("status"),
                },
            )
        elif kind == "usage_update":
            self._emit(
                record,
                EventType.PROGRESS,
                {"kind": "usage", "used": update.get("used"), "size": update.get("size")},
            )
        elif kind == "config_option_update":
            options = update.get("configOptions") or []
            self._emit(
                record,
                EventType.PROGRESS,
                {"kind": "config_option_update", "option_count": len(options)},
            )
        else:
            self._emit(
                record,
                EventType.PROGRESS,
                {"kind": "unmapped_update", "sessionUpdate": str(kind)},
            )

    # -- 4.1 initialize -----------------------------------------------------
    def initialize(self) -> EngineDescriptor:
        with self._lock:
            if self._descriptor is not None:
                return self._descriptor
        self._ensure_peer()
        result = self._request(
            "initialize",
            {"protocolVersion": ACP_PROTOCOL_VERSION, "clientCapabilities": CLIENT_CAPS},
            self._init_timeout,
        )
        agent_info = result.get("agentInfo") or {}
        capabilities = result.get("agentCapabilities") or {}
        session_caps = capabilities.get("sessionCapabilities") or {}
        protocol_version = result.get("protocolVersion")
        compatible = protocol_version == ACP_PROTOCOL_VERSION
        descriptor = EngineDescriptor(
            engine_kind="dsh-acp",
            engine_version=str(agent_info.get("version") or "unknown"),
            protocol_version="acp/" + str(protocol_version),
            compatibility="compatible" if compatible else "incompatible",
            healthy=True,
            capabilities=dict(ADAPTER_CAPABILITIES),
            detail={
                "agent_name": agent_info.get("name"),
                "advertised_session_capabilities": sorted(str(k) for k in session_caps),
                "advertised_mcp_http": bool((capabilities.get("mcpCapabilities") or {}).get("http")),
                "auth_methods": len(result.get("authMethods") or []),
                "command": self._command,
                "client_capabilities": dict(CLIENT_CAPS),
                "note": "capabilities 为适配层声明；实际通过与否见 E01-E20 执行状态",
            },
        )
        if not compatible:
            raise EngineError(
                EngineErrorCode.PROTOCOL_INCOMPATIBLE,
                "engine protocolVersion=" + str(protocol_version) + " != " + str(ACP_PROTOCOL_VERSION),
                detail=descriptor.detail,
            )
        with self._lock:
            self._descriptor = descriptor
        return descriptor

    # -- 4.2 submit_task ----------------------------------------------------
    def submit_task(self, request: SubmitTaskRequest) -> SubmitTaskReceipt:
        if not request.intent or not isinstance(request.intent, str):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "intent must be a non-empty string")
        # 单一规范化落点：后续的记录、session/new 与本地落盘均使用规范化后的路径。
        workspace_dir = canonical_cwd(request.workspace_dir, field="workspace_dir")
        text = request.prompt_text
        if text is None and request.input_file:
            try:
                text = Path(request.input_file).read_text(encoding="utf-8")
            except OSError as exc:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "input_file could not be read: " + str(exc),
                    detail={"input_file": request.input_file},
                )
        if not text:
            raise EngineError(
                EngineErrorCode.INVALID_ARGUMENT, "prompt_text or input_file is required"
            )

        with self._lock:
            if request.idempotency_key and request.idempotency_key in self._idempotency:
                existing = self._idempotency[request.idempotency_key]
                return SubmitTaskReceipt(
                    omni_task_id=existing.omni_task_id,
                    status=existing.state,
                    accepted_at=existing.created_at,
                    engine_session_id=existing.engine_session_id,
                    note="idempotency_key 命中，未重复提交",
                )
            omni_task_id = request.omni_task_id or new_omni_task_id()
            if omni_task_id in self._tasks:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "omni_task_id already exists; ids must not be reused",
                    detail={"omni_task_id": omni_task_id},
                )
            record = _TaskRecord(
                omni_task_id=omni_task_id,
                intent=request.intent,
                workspace_dir=workspace_dir,
                idempotency_key=request.idempotency_key,
                created_at=request.requested_at or now(),
                state_since=now(),
                state=TaskState.QUEUED,
                evidence_source="operator_confirm",
                timeouts=dict(request.timeout_policy),
            )
            self._tasks[omni_task_id] = record
            if request.idempotency_key:
                self._idempotency[request.idempotency_key] = record
        self._persist(record)

        # 会话取得：优先**续接既有会话**（真多轮），否则新建。
        #
        # OD-ARCH-034：ACP 本身支持在**同一个会话**里续发（session/prompt），
        # 此前这里每轮都 session/new，靠前端重放上文模拟多轮——那是我们自造的绕路。
        #
        # 两种续接方式（按 DSH 的真实语义区分，不猜）：
        #   * **continued**：会话在**本进程里还活着**（本适配层建过、引擎没关）
        #     → 直接再发一次 session/prompt；**不能**先 session/resume
        #       （DSH 的 resume 只接受「已落盘、可重开」的会话，对活会话会报 invalid_argument）。
        #   * **resumed**：会话不在本进程（例如应用重启后）→ session/resume 重开它，
        #     引擎侧从会话存储恢复上文，再 session/prompt 续发。
        # 两者都失败/不可用时**降级为新建会话**并如实标注，不让一次失败拖垮整轮对话
        # （此时前端仍会带上文重放，历史不丢）。
        session_id: Optional[str] = None
        session_mode = "new"
        resume_note = ""
        if request.engine_session_id:
            with self._lock:
                live_here = str(request.engine_session_id) in self._sessions
            if live_here:
                session_id = str(request.engine_session_id)
                session_mode = "continued"
            else:
                try:
                    self._request(
                        "session/resume",
                        {
                            "sessionId": request.engine_session_id,
                            "cwd": workspace_dir,
                            "mcpServers": [],
                        },
                        self._session_timeout,
                        omni_task_id=omni_task_id,
                    )
                    session_id = str(request.engine_session_id)
                    session_mode = "resumed"
                except EngineError as error:
                    session_mode = "degraded"
                    resume_note = (
                        "既有会话无法续接（已改为新建会话继续）：" + error.code.value
                    )

        if session_id is None:
            try:
                created = self._request(
                    "session/new",
                    {"cwd": workspace_dir, "mcpServers": []},
                    self._session_timeout,
                    omni_task_id=omni_task_id,
                )
            except EngineError as error:
                # 受理阶段就失败时，不能让记录停留在 queued：否则该任务既没有回执给
                # 调用方，又会被 recover_task 当成「进行中」的幻影任务。这里按契约
                # 把记录落定为相应终态后再抛出，保证失败可诊断且不留悬空状态。
                self._settle_failed(record, error)
                raise
            new_session_id = created.get("sessionId")
            if not new_session_id:
                error = EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "session/new did not return a session id",
                    detail={"result_keys": sorted(str(k) for k in created.keys())},
                    omni_task_id=omni_task_id,
                )
                self._settle_failed(record, error)
                raise error
            session_id = str(new_session_id)
            if session_mode != "degraded":
                session_mode = "new"
        with self._lock:
            record.engine_session_id = session_id
            record.session_mode = session_mode
            record.state = TaskState.STARTING
            record.evidence_source = "operator_confirm"
            record.state_since = now()
            self._sessions[session_id] = record
        self._persist(record)
        self._start_prompt(record, text)
        with self._lock:
            state = record.state
        if session_mode == "resumed":
            receipt_note = "已续接既有引擎会话并提交；等待引擎终态证据"
        elif session_mode == "continued":
            receipt_note = "已在同一引擎会话内继续本轮；等待引擎终态证据"
        elif session_mode == "degraded":
            receipt_note = resume_note + "；等待引擎终态证据"
        else:
            receipt_note = "适配层已建立会话并提交；等待引擎终态证据"
        return SubmitTaskReceipt(
            omni_task_id=omni_task_id,
            status=state,
            accepted_at=record.created_at,
            engine_session_id=record.engine_session_id,
            note=receipt_note,
            session_mode=session_mode,
        )

    def _start_prompt(self, record: _TaskRecord, text: str) -> None:
        peer = self._peer
        if peer is None:
            raise EngineError(EngineErrorCode.ENGINE_UNAVAILABLE, "engine not started")
        rid = self._next_id()
        with self._lock:
            record.prompt_request_id = rid
            record.state = TaskState.RUNNING
            record.state_since = now()
        try:
            peer.request(
                rid,
                "session/prompt",
                {"sessionId": record.engine_session_id, "prompt": [{"type": "text", "text": text}]},
            )
        except PeerExit as exc:
            self._settle(record, TaskState.INTERRUPTED, "process_state", note=str(exc))
            return
        # task_started 是 OmniDesk 归一化事件，不是 ACP 原生事件。
        # session_mode 如实记录本轮是新建会话还是续接既有会话（台账与证据用）。
        self._emit(
            record,
            EventType.TASK_STARTED,
            {
                "normalized": True,
                "reason": "session/prompt 已发出",
                "intent": record.intent,
                "session_mode": record.session_mode,
            },
        )
        worker = threading.Thread(target=self._prompt_worker, args=(record, rid), daemon=True)
        worker.start()

    def _task_timeout(self, record: _TaskRecord) -> float:
        policy = record.timeouts or {}
        total = policy.get("total_seconds")
        if total is None:
            return self._default_timeout
        return float(total)

    def _prompt_worker(self, record: _TaskRecord, rid: str) -> None:
        peer = self._peer
        if peer is None:
            return
        timeout = self._task_timeout(record)
        frame = peer.wait_response(rid, timeout)
        if frame is None:
            code = peer.proc.poll()
            if code is not None:
                self._settle(
                    record,
                    TaskState.INTERRUPTED,
                    "process_state",
                    note="引擎进程在给出终态前退出（rc=" + str(code) + "）；未自动重放",
                )
            else:
                self._settle(
                    record,
                    TaskState.UNKNOWN,
                    "timeout",
                    note="prompt 在 " + str(timeout) + "s 内未返回终态；未自动重放",
                )
            return
        if "error" in frame:
            error = self._map_rpc_error(frame["error"], "session/prompt", record.omni_task_id)
            self._settle_failed(record, error)
            return
        result = frame.get("result")
        stop_reason = (result or {}).get("stopReason") if isinstance(result, Mapping) else None
        if stop_reason == "cancelled":
            self._settle(
                record,
                TaskState.CANCELLED,
                "engine_terminal",
                note="引擎以 stopReason=cancelled 落定",
                stop_reason=str(stop_reason),
            )
        elif stop_reason == "max_tokens":
            self._settle(
                record,
                TaskState.SUCCEEDED,
                "engine_terminal",
                note="输出被 max tokens 截断；integrity=partial，不被视为完整成果",
                stop_reason=str(stop_reason),
                truncated=True,
            )
        elif stop_reason == "end_turn":
            self._settle(
                record,
                TaskState.SUCCEEDED,
                "engine_terminal",
                note="stopReason=end_turn；该通道无法区分 completed 与 blocked，属已知语义缺口",
                stop_reason=str(stop_reason),
            )
        else:
            self._settle(
                record,
                TaskState.UNKNOWN,
                "engine_terminal",
                note="未映射的 stopReason=" + str(stop_reason) + "；不猜测为成功",
                stop_reason=str(stop_reason),
            )

    # -- 4.6 cancel_task ----------------------------------------------------
    def cancel_task(self, omni_task_id: str, *, timeout: float = 15.0) -> CancelOutcome:
        record = self._require_task(omni_task_id)
        with self._lock:
            if record.state in TERMINAL_STATES:
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.NOT_REQUESTED,
                    requested_at=None,
                    settled_at=now(),
                    final_state=record.state,
                    evidence="already_terminal",
                    note="任务已终态，未发送取消请求",
                )
            peer = self._peer
            session_id = record.engine_session_id
            record.cancel_phase = CancelPhase.REQUEST_SENT
            record.cancel_requested_at = now()
            record.state = TaskState.CANCELLING
            record.state_since = now()
        self._emit(
            record,
            EventType.CANCEL_REQUESTED,
            {"normalized": True, "note": "session/cancel 是通知帧，无响应；acknowledged 阶段在本通道不可达"},
        )
        if peer is None or session_id is None:
            raise EngineError(
                EngineErrorCode.CANCEL_FAILED,
                "no active engine session to cancel",
                omni_task_id=omni_task_id,
            )
        try:
            peer.notify("session/cancel", {"sessionId": session_id})
        except PeerExit as exc:
            with self._lock:
                record.cancel_phase = CancelPhase.TIMEOUT_UNKNOWN
                record.cancel_settled_at = now()
            raise EngineError(
                EngineErrorCode.CANCEL_FAILED,
                "cancel could not be delivered: " + str(exc),
                omni_task_id=omni_task_id,
            )
        deadline = time.monotonic() + float(timeout)
        while time.monotonic() < deadline:
            with self._lock:
                state = record.state
                phase = record.cancel_phase
            if state == TaskState.CANCELLED:
                with self._lock:
                    record.cancel_phase = CancelPhase.CONFIRMED_CANCELLED
                    record.cancel_settled_at = now()
                    record.state_since = now()
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.CONFIRMED_CANCELLED,
                    requested_at=record.cancel_requested_at,
                    settled_at=record.cancel_settled_at,
                    final_state=TaskState.CANCELLED,
                    evidence="engine_terminal",
                    note="引擎以 stopReason=cancelled 落定",
                )
            if state in TERMINAL_STATES:
                with self._lock:
                    record.cancel_settled_at = now()
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.REQUEST_SENT,
                    requested_at=record.cancel_requested_at,
                    settled_at=record.cancel_settled_at,
                    final_state=state,
                    evidence="engine_terminal",
                    note="取消未被确认，任务已按其它终态落定；按 3.3 保留引擎终态",
                )
            if phase != CancelPhase.REQUEST_SENT:
                break
            time.sleep(0.05)
        with self._lock:
            settled = record.cancel_settled_at
            state = record.state
            if record.cancel_phase == CancelPhase.REQUEST_SENT:
                record.cancel_phase = CancelPhase.TIMEOUT_UNKNOWN
                record.cancel_settled_at = now()
                settled = record.cancel_settled_at
        return CancelOutcome(
            omni_task_id=omni_task_id,
            phase=CancelPhase.TIMEOUT_UNKNOWN,
            requested_at=record.cancel_requested_at,
            settled_at=settled,
            final_state=state,
            evidence="operator_confirm",
            note="取消请求已发送但未取得可靠终态证据；不得报告为取消成功",
        )

    # -- 4.7 session listing / recover_session / recover_task ---------------
    def _succeeded_provenance(self, record, *, stop_reason, persist_ok):
        """ACP 通道：成功结果的来源说明（保持 045 既有语义）。"""
        return (
            (
                {"kind": "engine_stop_reason", "value": str(stop_reason)},
                {"kind": "engine_terminal", "value": "end_turn 无法区分 completed/blocked"},
                {"kind": "persist", "value": "ok" if persist_ok else "failed"},
            ),
            "结果由 OmniDesk 从 agent_message_chunk 拼装"
            + ("；end_turn 语义存在缺口" if stop_reason == "end_turn" else ""),
        )

    def _probe_session(self, session_id: str, workspace_dir: str) -> None:
        self.recover_session(session_id, cwd=workspace_dir or self._cwd)

    def list_sessions(
        self,
        *,
        after_cursor: Optional[str] = None,
        cwd: Optional[str] = None,
        max_pages: int = 20,
    ) -> SessionListing:
        """通过公开的 `session/list` 列出会话**元数据**，按规范化工作区归组。

        约束：
          * 只使用 ACP 公开接口，不扫描、不解析 DSH 的会话持久化文件；
          * 只返回 sessionId / cwd / createdAt，不读取任何消息正文；
          * cwd 的显示值可能含真实路径，调用方（UI/报告）自行决定是否展示；
          * 无法解析 cwd 的记录计入 skipped_invalid，不猜测其工作区；
          * 分页沿 nextCursor 有界推进，避免无限循环。单页大小由引擎的
            sessionListPageSize 配置决定（公开 schema），适配层不覆盖该配置。
        """
        filter_cwd: Optional[str] = None
        if cwd is not None:
            filter_cwd = canonical_cwd(cwd)
        cursor = after_cursor
        entries: list[Mapping[str, Any]] = []
        skipped = 0
        pages = 0
        next_cursor: Optional[str] = None
        while pages < max(1, int(max_pages)):
            params: dict[str, Any] = {}
            if cursor:
                params["cursor"] = cursor
            if filter_cwd is not None:
                params["cwd"] = filter_cwd
            try:
                page = self._request("session/list", params, self._session_timeout)
            except EngineError as error:
                if error.code == EngineErrorCode.UNSUPPORTED:
                    raise EngineError(
                        EngineErrorCode.UNSUPPORTED,
                        "engine does not support session/list",
                        detail=dict(error.detail),
                    )
                raise EngineError(
                    EngineErrorCode.RECOVER_FAILED,
                    "session listing failed: " + error.message,
                    detail=dict(error.detail),
                ) from error
            pages += 1
            raw_sessions = page.get("sessions")
            if not isinstance(raw_sessions, list):
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list response has no sessions array",
                    detail={"result_keys": sorted(str(k) for k in page.keys())},
                )
            for item in raw_sessions:
                if not isinstance(item, Mapping):
                    skipped += 1
                    continue
                session_id = item.get("sessionId")
                raw_cwd = item.get("cwd")
                if not isinstance(session_id, str) or not session_id:
                    skipped += 1
                    continue
                if not isinstance(raw_cwd, str) or not os.path.isabs(raw_cwd):
                    skipped += 1
                    continue
                entries.append(
                    {
                        "sessionId": session_id,
                        # 展示与归组都使用物理身份；不存在的旧目录保留词法规范化结果。
                        "cwd": os.path.realpath(raw_cwd),
                        "cwdExists": os.path.isdir(raw_cwd),
                    }
                )
            raw_next = page.get("nextCursor")
            if not raw_next:
                next_cursor = None
                break
            if not isinstance(raw_next, str):
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list nextCursor is not a string",
                )
            if raw_next == cursor:
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list pagination did not advance",
                )
            cursor = raw_next
            next_cursor = raw_next
        grouped: dict[str, list[dict[str, Any]]] = {}
        for entry in entries:
            grouped.setdefault(str(entry["cwd"]), []).append(
                {
                    "engine_session_id": entry["sessionId"],
                    "workspace_label": workspace_label_for(
                        str(entry["cwd"]), ordinal=0
                    ),
                    "workspace_token": workspace_token_for(str(entry["cwd"])),
                    "created_at": None,
                    "workspace_exists": entry["cwdExists"],
                    "metadata_valid": True,
                    # 列表可见 != 可恢复；只有实际 resume 成功才可改变该值。
                    "resume_status": "unverified",
                }
            )
        groups = []
        for ordinal, (key, value) in enumerate(sorted(grouped.items()), start=1):
            label = workspace_label_for(key, ordinal=ordinal)
            token = workspace_token_for(key)
            with self._lock:
                self._workspace_tokens[token] = key
                self._workspace_labels[token] = label
            for session in value:
                session["workspace_label"] = label
                session["workspace_token"] = token
            groups.append({"workspace_label": label, "workspace_token": token, "sessions": value})
        groups = tuple(groups)
        return SessionListing(
            groups=groups,
            total=len(entries),
            skipped_invalid=skipped,
            next_cursor=next_cursor,
            note="只含会话元数据；列表可见不代表可恢复，重开结果以 session/resume 为准",
        )

    def recover_session(
        self,
        engine_session_id: str,
        *,
        cwd: Optional[str] = None,
        workspace_token: Optional[str] = None,
    ) -> SessionRecovery:
        with self._lock:
            known = self._sessions.get(engine_session_id)
            raw_cwd = cwd or (known.workspace_dir if known is not None else self._cwd)
            if workspace_token:
                # 界面只能回传它见过的不透明 token，不能凭自由路径重开会话。
                resolved = self._workspace_tokens.get(str(workspace_token))
                if resolved is None:
                    raise EngineError(
                        EngineErrorCode.UNKNOWN_SESSION,
                        "unknown workspace token; list sessions again",
                    )
                raw_cwd = resolved
        # 同一个规范化落点：恢复时也必须用 realpath，否则别名写法会被引擎当作另一个工作区。
        target_cwd = canonical_cwd(raw_cwd)
        try:
            result = self._request(
                "session/resume",
                {"sessionId": engine_session_id, "cwd": target_cwd, "mcpServers": []},
                self._session_timeout,
            )
        except EngineError as error:
            if error.code == EngineErrorCode.UNKNOWN_SESSION:
                raise
            raise EngineError(
                EngineErrorCode.RECOVER_FAILED,
                "session recovery failed: " + error.message,
                detail=dict(error.detail),
                omni_task_id=None,
            )
        options = result.get("configOptions") or []
        return SessionRecovery(
            engine_session_id=engine_session_id,
            resumed=True,
            cwd=target_cwd,
            config_option_count=len(options),
            evidence="engine_terminal",
            note="恢复的是既有会话；恢复不触发任何任务重放",
            workspace_token=workspace_token_for(target_cwd),
            workspace_label=self._workspace_labels.get(
                workspace_token_for(target_cwd), ""
            ),
        )

    # -- 4.8 shutdown -------------------------------------------------------
    def shutdown(
        self,
        *,
        mode: str = "graceful",
        grace_period: float = 5.0,
        cancel_in_flight: bool = False,
        cancel_timeout: Optional[float] = None,
    ) -> ShutdownReport:
        if mode not in ("graceful", "force", "detach"):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "unknown shutdown mode: " + str(mode))
        with self._lock:
            peer = self._peer
            in_flight = tuple(
                t.omni_task_id for t in self._tasks.values() if t.state not in TERMINAL_STATES
            )
        cancelled: list = []
        still_running: list = []
        if cancel_in_flight and in_flight and peer is not None:
            for task_id in in_flight:
                try:
                    outcome = self.cancel_task(
                        task_id, timeout=float(cancel_timeout if cancel_timeout is not None else grace_period)
                    )
                except EngineError:
                    still_running.append(task_id)
                    continue
                if outcome.phase == CancelPhase.CONFIRMED_CANCELLED:
                    cancelled.append(task_id)
                elif outcome.final_state is None or outcome.final_state not in TERMINAL_STATES:
                    still_running.append(task_id)
        with self._lock:
            remaining = tuple(
                t.omni_task_id for t in self._tasks.values() if t.state not in TERMINAL_STATES
            )
        for task_id in remaining:
            if task_id not in still_running:
                still_running.append(task_id)
        if peer is None:
            with self._lock:
                self._closed = True
            return ShutdownReport(
                mode=mode,
                in_flight_before=in_flight,
                cancelled=tuple(cancelled),
                still_running=tuple(still_running),
                connection_closed=False,
                process_exit_code=None,
                forced_terminate=False,
                note="引擎未启动，无连接可关闭",
            )
        if mode == "force":
            peer.hard_kill()
            with self._lock:
                self._closed = True
            return ShutdownReport(
                mode=mode,
                in_flight_before=in_flight,
                cancelled=tuple(cancelled),
                still_running=tuple(still_running),
                connection_closed=True,
                process_exit_code=peer.proc.returncode,
                forced_terminate=True,
                note="强制终止：可能丢失未落盘输出；下次启动不自动重放",
            )
        peer.close_stdin()
        exit_code = peer.wait_exit(float(grace_period))
        forced = False
        if exit_code is None and mode == "graceful":
            forced = True
            peer.proc.kill()
            exit_code = peer.wait_exit(5.0)
        if mode == "detach":
            peer.close_stdin()
        with self._lock:
            self._closed = True
        note = "优雅关闭"
        if forced:
            note = "宽限期内未结束，已强制终止；未落盘输出可能丢失"
        return ShutdownReport(
            mode=mode,
            in_flight_before=in_flight,
            cancelled=tuple(cancelled),
            still_running=tuple(still_running),
            connection_closed=True,
            process_exit_code=exit_code,
            forced_terminate=forced,
            note=note,
        )

    # -- 观测接口（证据采集用，不属于契约）---------------------------------
    def raw_frames(self) -> list:
        peer = self._peer
        return list(peer.snapshot()) if peer is not None else []

    def raw_sent_frames(self) -> list:
        peer = self._peer
        return list(peer.sent) if peer is not None else []

    def engine_stderr(self) -> str:
        peer = self._peer
        return peer.stderr_text() if peer is not None else ""

    def engine_exit_code(self) -> Optional[int]:
        peer = self._peer
        return peer.proc.returncode if peer is not None else None

    def unmatched_frames(self) -> list:
        with self._lock:
            return list(self._unmatched_frames)

    def orphan_updates(self) -> list:
        with self._lock:
            return list(self._orphan_updates)

    def permission_requests(self) -> list:
        with self._lock:
            return list(self._permission_requests)
