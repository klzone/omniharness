"""通道无关的任务/会话核心（ACP 与 Web API 两个适配器共用）。

这里只放**与底层协议无关**的部分：任务记录的字段、事件缓冲、终态仲裁、
结果组装、本地台账。任何 ACP / HTTP / WebSocket 细节都不许出现在本模块。

抽出的原因（OD-ARCH-048）：新增 Web API 通道时若复制一份 900 行适配器，
两条通道的状态语义会各自漂移；这里保持单一权威实现，通道只负责“把底层
协议翻译成这些方法调用”。
"""

from __future__ import annotations

import json
import os
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterator, Mapping, Optional

from .contract import (
    EVIDENCE_PRIORITY,
    FORBIDDEN_INFERENCES,
    TERMINAL_STATES,
    CancelPhase,
    EngineDescriptor,
    EngineError,
    EngineErrorCode,
    EngineEvent,
    EventType,
    ResultForm,
    ResultIntegrity,
    TaskResult,
    TaskState,
    TaskStatus,
    now,
)


# ---------------------------------------------------------------------------
# 请求 / 回执 / 结果对象（契约 4.2、4.6、4.7、4.8）
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SubmitTaskRequest:
    """契约 4.2 的 SubmitTaskRequest。

    prompt_text 与 input_file 二者至少其一：input_file 由 OmniDesk 自己读取后
    作为受控载荷发送（引擎不直接访问该路径）。

    engine_session_id（OD-ARCH-034）：给出时**续接该既有会话**，实现同一会话
    内的真多轮；不给时新建会话。续接失败会降级为新建会话并在回执里说明。
    """

    intent: str
    workspace_dir: str
    omni_task_id: Optional[str] = None
    idempotency_key: Optional[str] = None
    prompt_text: Optional[str] = None
    input_file: Optional[str] = None
    timeout_policy: Mapping[str, float] = field(default_factory=dict)
    requested_at: Optional[float] = None
    engine_session_id: Optional[str] = None


@dataclass(frozen=True)
class SubmitTaskReceipt:
    omni_task_id: str
    status: TaskState
    accepted_at: float
    engine_session_id: Optional[str] = None
    note: str = ""
    # new / continued / resumed / degraded（degraded = 续接失败后新建）。
    session_mode: str = "new"

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "status": self.status.value,
            "accepted_at": self.accepted_at,
            "engine_session_id": self.engine_session_id,
            "note": self.note,
            "session_mode": self.session_mode,
        }


@dataclass(frozen=True)
class CancelOutcome:
    omni_task_id: str
    phase: CancelPhase
    requested_at: Optional[float]
    settled_at: Optional[float]
    final_state: Optional[TaskState]
    evidence: str
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "phase": self.phase.value,
            "requested_at": self.requested_at,
            "settled_at": self.settled_at,
            "final_state": self.final_state.value if self.final_state else None,
            "evidence": self.evidence,
            "note": self.note,
        }


@dataclass(frozen=True)
class SessionRecovery:
    engine_session_id: str
    resumed: bool
    cwd: str
    config_option_count: int
    evidence: str
    note: str = ""
    workspace_token: str = ""
    workspace_label: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "engine_session_id": self.engine_session_id,
            "resumed": self.resumed,
            "workspace_token": self.workspace_token,
            "workspace_label": self.workspace_label,
            "config_option_count": self.config_option_count,
            "evidence": self.evidence,
            "note": self.note,
        }


@dataclass(frozen=True)
class SessionFork:
    """一次会话分支的结果（DSH 原生 session/fork）。

    子会话在引擎侧继承切点前的完整事件前缀；OmniDesk 只保存新会话号，
    后续续接该子会话，不再重放历史。
    """

    parent_session_id: str
    engine_session_id: str
    at_seq: Optional[int] = None
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "parent_session_id": self.parent_session_id,
            "engine_session_id": self.engine_session_id,
            "at_seq": self.at_seq,
            "note": self.note,
        }


@dataclass(frozen=True)
class SessionSummary:
    """一个可发现会话的**元数据**。不包含任何消息正文。"""

    engine_session_id: str
    workspace_dir: str
    workspace_label: str = ""
    workspace_token: str = ""
    created_at: Optional[float] = None
    metadata_valid: bool = True
    resume_status: str = "unverified"

    def to_dict(self) -> dict[str, Any]:
        return {
            "engine_session_id": self.engine_session_id,
            "workspace_label": self.workspace_label,
            "workspace_token": self.workspace_token,
            "created_at": self.created_at,
            "metadata_valid": self.metadata_valid,
            "resume_status": self.resume_status,
        }


@dataclass(frozen=True)
class SessionListing:
    groups: tuple[dict[str, Any], ...]
    total: int
    skipped_invalid: int
    next_cursor: Optional[str]
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "groups": list(self.groups),
            "total": self.total,
            "skipped_invalid": self.skipped_invalid,
            "next_cursor": self.next_cursor,
            "note": self.note,
        }


@dataclass(frozen=True)
class TaskRecordSummary:
    """本地任务台账的一条**元数据**记录（不含内容正文与事件）。"""

    omni_task_id: str
    state: str
    intent: str
    created_at: float
    state_since: float
    engine_session_id: Optional[str]
    cancel_phase: str
    persistence: str
    session_mode: str = "new"
    ttfb_seconds: Optional[float] = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "state": self.state,
            "created_at": self.created_at,
            "state_since": self.state_since,
            "engine_session_id": self.engine_session_id,
            "cancel_phase": self.cancel_phase,
            "persistence": self.persistence,
            "session_mode": self.session_mode,
            "ttfb_seconds": self.ttfb_seconds,
        }


@dataclass(frozen=True)
class TaskLedgerListing:
    records: tuple[TaskRecordSummary, ...]
    skipped_invalid: int
    limit: int
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "records": [r.to_dict() for r in self.records],
            "skipped_invalid": self.skipped_invalid,
            "limit": self.limit,
            "note": self.note,
        }


@dataclass(frozen=True)
class TaskRecovery:
    omni_task_id: str
    status: TaskStatus
    replayed: bool
    evidence: str
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "status": self.status.to_dict(),
            "replayed": self.replayed,
            "evidence": self.evidence,
            "note": self.note,
        }


@dataclass(frozen=True)
class ShutdownReport:
    mode: str
    in_flight_before: tuple[str, ...]
    cancelled: tuple[str, ...]
    still_running: tuple[str, ...]
    connection_closed: bool
    process_exit_code: Optional[int]
    forced_terminate: bool
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "mode": self.mode,
            "in_flight_before": list(self.in_flight_before),
            "cancelled": list(self.cancelled),
            "still_running": list(self.still_running),
            "connection_closed": self.connection_closed,
            "process_exit_code": self.process_exit_code,
            "forced_terminate": self.forced_terminate,
            "note": self.note,
        }


@dataclass
class _TaskRecord:
    omni_task_id: str
    intent: str
    workspace_dir: str
    idempotency_key: Optional[str]
    created_at: float
    state_since: float
    state: TaskState
    evidence_source: str
    engine_session_id: Optional[str] = None
    session_mode: str = "new"
    seq: int = 0
    events: list = field(default_factory=list)
    text_parts: list = field(default_factory=list)
    text_len: int = 0
    last_event_at: float = 0.0
    stop_reason: Optional[str] = None
    error_code: Optional[str] = None
    error_message: Optional[str] = None
    cancel_phase: CancelPhase = CancelPhase.NOT_REQUESTED
    cancel_requested_at: Optional[float] = None
    cancel_settled_at: Optional[float] = None
    prompt_request_id: Optional[str] = None
    terminal_events: list = field(default_factory=list)
    dedup_seen: set = field(default_factory=set)
    dedup_dropped: int = 0
    truncated: bool = False
    persist_ok: bool = True
    persist_error: Optional[str] = None
    persisted_at: Optional[float] = None
    timeouts: Mapping[str, float] = field(default_factory=dict)
    # OD-ARCH-066：submit → 首个 text-delta 的秒数；以及待人工审批的请求。
    ttfb_seconds: Optional[float] = None
    submitted_monotonic: Optional[float] = None
    pending_approvals: dict = field(default_factory=dict)
    # 本轮 turn/end 的会话事件序号：会话分支（session/fork）的精确切点。
    turn_end_seq: Optional[int] = None


class TaskCoreBase:
    """任务缓冲、终态仲裁与结果组装。子类只实现协议侧的方法。"""

    # 子类覆盖：结果来源说明（不同通道的证据不同）。
    engine_kind = "unknown"

    def __init__(self, *, state_dir: Optional[str] = None,
                 default_timeout: Optional[float] = 120.0) -> None:
        self._state_dir = state_dir
        # None = 不按墙钟自动收尾；具体通道自行解释（子类可覆盖为无上限）。
        self._default_timeout = None if default_timeout is None else float(default_timeout)
        self._lock = threading.RLock()
        self._cond = threading.Condition(self._lock)
        self._tasks: dict = {}
        self._sessions: dict = {}
        self._idempotency: dict = {}
        self._descriptor: Optional[EngineDescriptor] = None
        self._closed = False

    # -- 事件与终态 ---------------------------------------------------------
    def _emit(self, record: _TaskRecord, event_type: EventType, payload: Mapping[str, Any]) -> None:
        with self._cond:
            record.seq += 1
            occurred = now()
            record.last_event_at = occurred
            record.events.append(
                EngineEvent(
                    omni_task_id=record.omni_task_id,
                    seq=record.seq,
                    occurred_at=occurred,
                    type=event_type,
                    payload=dict(payload),
                )
            )
            self._cond.notify_all()

    def _settle(
        self,
        record: _TaskRecord,
        state: TaskState,
        evidence: str,
        *,
        note: str = "",
        stop_reason: Optional[str] = None,
        error_code: Optional[str] = None,
        error_message: Optional[str] = None,
        truncated: bool = False,
    ) -> bool:
        with self._lock:
            if record.state in TERMINAL_STATES:
                existing_rank = EVIDENCE_PRIORITY.get(record.evidence_source, 9)
                new_rank = EVIDENCE_PRIORITY.get(evidence, 9)
                if new_rank >= existing_rank:
                    record.terminal_events.append(
                        {"ignored_state": state.value, "reason": "已有更高或同级终态证据",
                         "evidence": evidence}
                    )
                    return False
            record.state = state
            record.evidence_source = evidence
            record.state_since = now()
            record.stop_reason = stop_reason
            record.error_code = error_code
            record.error_message = error_message
            record.truncated = truncated
        if state == TaskState.SUCCEEDED:
            self._emit(
                record,
                EventType.TASK_COMPLETED,
                {"stop_reason": stop_reason, "integrity": "partial" if truncated else "complete",
                 "truncated": truncated, "note": note},
            )
        elif state == TaskState.FAILED:
            self._emit(
                record,
                EventType.TASK_FAILED,
                {"error_code": error_code, "message": error_message, "note": note},
            )
        elif state == TaskState.CANCELLED:
            self._emit(record, EventType.TASK_CANCELLED, {"stop_reason": stop_reason, "note": note})
        else:
            self._emit(record, EventType.TASK_INTERRUPTED, {"state": state.value, "note": note})
        self._persist(record)
        return True

    def _settle_failed(self, record: _TaskRecord, error: EngineError) -> None:
        state = TaskState.FAILED
        if error.code == EngineErrorCode.ENGINE_UNAVAILABLE:
            state = TaskState.INTERRUPTED
        elif error.code == EngineErrorCode.TIMEOUT:
            state = TaskState.UNKNOWN
        self._settle(
            record,
            state,
            "engine_error",
            note=error.message,
            error_code=error.code.value,
            error_message=error.message,
        )

    # -- 落盘（原型级本地状态；生产须由 OmniDesk 自有数据根承担）------------
    def _persist(self, record: _TaskRecord) -> None:
        if self._state_dir is None:
            record.persist_ok = True
            return
        try:
            base = Path(self._state_dir)
            base.mkdir(parents=True, exist_ok=True)
            path = base / (record.omni_task_id + ".json")
            tmp = base / (record.omni_task_id + ".json.tmp")
            with self._lock:
                payload = self._record_payload(record)
            tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
            os.replace(tmp, path)
            with self._lock:
                record.persist_ok = True
                record.persist_error = None
                record.persisted_at = now()
        except OSError as exc:
            with self._lock:
                record.persist_ok = False
                record.persist_error = type(exc).__name__ + ": " + str(exc)

    def _record_payload(self, record: _TaskRecord) -> dict:
        return {
            "omni_task_id": record.omni_task_id,
            "intent": record.intent,
            "workspace_dir": record.workspace_dir,
            "idempotency_key": record.idempotency_key,
            "engine_session_id": record.engine_session_id,
            "session_mode": record.session_mode,
            "state": record.state.value,
            "evidence_source": record.evidence_source,
            "created_at": record.created_at,
            "state_since": record.state_since,
            "seq": record.seq,
            "stop_reason": record.stop_reason,
            "error_code": record.error_code,
            "error_message": record.error_message,
            "cancel_phase": record.cancel_phase.value,
            "truncated": record.truncated,
            "persisted_at": record.persisted_at,
            # 只存耗时数字与审批计数，不存请求正文、命令或原因文本。
            "ttfb_seconds": record.ttfb_seconds,
            "pending_approvals": len(record.pending_approvals),
            "turn_end_seq": record.turn_end_seq,
            "content": "".join(record.text_parts),
        }

    def _read_persisted(self, omni_task_id: str) -> Optional[Mapping[str, Any]]:
        if self._state_dir is None:
            return None
        path = Path(self._state_dir) / (omni_task_id + ".json")
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None
        return data if isinstance(data, Mapping) else None

    # -- 4.3 get_task_status ------------------------------------------------
    def _require_task(self, omni_task_id: str) -> _TaskRecord:
        with self._lock:
            record = self._tasks.get(omni_task_id)
        if record is None:
            raise EngineError(
                EngineErrorCode.UNKNOWN_TASK,
                "unknown omni_task_id: " + str(omni_task_id),
                detail={"omni_task_id": omni_task_id},
            )
        return record

    def get_task_status(self, omni_task_id: str) -> TaskStatus:
        record = self._require_task(omni_task_id)
        with self._lock:
            return TaskStatus(
                omni_task_id=record.omni_task_id,
                status=record.state,
                evidence_source=record.evidence_source,
                since=record.state_since,
                last_event_seq=record.seq,
                partial_output_size=record.text_len,
                engine_session_id=record.engine_session_id,
                note=self._status_note(record),
                ttfb_seconds=record.ttfb_seconds,
                pending_approvals=len(record.pending_approvals),
                engine_turn_end_seq=record.turn_end_seq,
            )

    def _status_note(self, record: _TaskRecord) -> str:
        parts = []
        # 终态任务的待审批计数不再当作「等待审批中」展示（请求可能已被引擎撤回）。
        if record.pending_approvals and record.state not in TERMINAL_STATES:
            tools = sorted({str(v.get("tool_name") or "?") for v in record.pending_approvals.values()})
            parts.append("等待审批中：" + ", ".join(tools))
        if record.stop_reason:
            parts.append("stop_reason=" + str(record.stop_reason))
        if record.error_message:
            parts.append("error=" + str(record.error_message))
        if record.dedup_dropped:
            parts.append("dropped_duplicate_chunks=" + str(record.dedup_dropped))
        if not record.persist_ok:
            parts.append("persist_failed=" + str(record.persist_error))
        return "; ".join(parts)

    # -- 4.4 stream_events --------------------------------------------------
    def stream_events(
        self,
        omni_task_id: str,
        *,
        poll_interval: float = 0.05,
        timeout: Optional[float] = None,
        until_terminal: bool = True,
        include_backlog: bool = True,
        after_seq: Optional[int] = None,
    ) -> Iterator[EngineEvent]:
        record = self._require_task(omni_task_id)
        index = 0
        if include_backlog:
            with self._lock:
                backlog = list(record.events)
            for event in backlog:
                if after_seq is None or event.seq > after_seq:
                    yield event
            index = len(backlog)
        deadline = None if timeout is None else time.monotonic() + timeout
        while True:
            with self._lock:
                pending = record.events[index:]
                if not pending:
                    if until_terminal and record.state in TERMINAL_STATES:
                        return
                    remaining = None if deadline is None else deadline - time.monotonic()
                    if remaining is not None and remaining <= 0:
                        return
                    wait_for = poll_interval if remaining is None else min(poll_interval, remaining)
                    self._cond.wait(timeout=max(0.0, wait_for))
                    continue
                index = len(record.events)
            for event in pending:
                yield event

    # -- 4.5 get_task_result ------------------------------------------------
    def _succeeded_provenance(
        self, record: _TaskRecord, *, stop_reason: str, persist_ok: bool
    ) -> tuple[tuple[dict, ...], str]:
        """成功结果的来源与说明。子类按通道覆盖。"""
        return (
            ({"kind": "engine_stop_reason", "value": str(stop_reason)},
             {"kind": "persist", "value": "ok" if persist_ok else "failed"}),
            "结果由引擎终态确定",
        )

    def get_task_result(self, omni_task_id: str, *, require_persisted: bool = False) -> TaskResult:
        record = self._require_task(omni_task_id)
        with self._lock:
            state = record.state
            content = "".join(record.text_parts)
            truncated = record.truncated
            persist_ok = record.persist_ok
            persist_error = record.persist_error
            persisted_at = record.persisted_at
            stop_reason = record.stop_reason
            error_code = record.error_code
        if state in (TaskState.QUEUED, TaskState.STARTING, TaskState.RUNNING, TaskState.CANCELLING):
            return TaskResult(
                omni_task_id=omni_task_id,
                form=ResultForm.NOT_READY,
                content=content,
                persisted_at=persisted_at,
                integrity=ResultIntegrity.PARTIAL,
                sources=(({"kind": "status", "value": state.value},)),
                note="任务尚未取得终态",
            )
        if state == TaskState.SUCCEEDED:
            integrity = ResultIntegrity.PARTIAL if truncated else ResultIntegrity.COMPLETE
            form = ResultForm.PARTIAL if truncated else ResultForm.COMPLETE
            if require_persisted and not persist_ok:
                raise EngineError(
                    EngineErrorCode.RESULT_PERSIST_FAILED,
                    "engine completed but local persistence failed: " + str(persist_error),
                    detail={"persist_error": persist_error, "content_chars": len(content)},
                    omni_task_id=omni_task_id,
                )
            sources, note = self._succeeded_provenance(
                record, stop_reason=str(stop_reason), persist_ok=persist_ok
            )
            return TaskResult(
                omni_task_id=omni_task_id,
                form=form,
                content=content,
                persisted_at=persisted_at,
                integrity=integrity,
                sources=sources,
                note=note,
            )
        if state == TaskState.UNKNOWN:
            return TaskResult(
                omni_task_id=omni_task_id,
                form=ResultForm.UNRECOVERABLE,
                content=content,
                persisted_at=persisted_at,
                integrity=ResultIntegrity.PARTIAL,
                sources=({"kind": "status", "value": state.value},),
                note="无法确认执行结果；不重跑任务伪造恢复结果",
            )
        if not content:
            return TaskResult(
                omni_task_id=omni_task_id,
                form=ResultForm.UNRECOVERABLE,
                content="",
                persisted_at=persisted_at,
                integrity=ResultIntegrity.PARTIAL,
                sources=({"kind": "status", "value": state.value},
                         {"kind": "error", "value": str(error_code)}),
                note="终态非成功且未收到任何输出",
            )
        return TaskResult(
            omni_task_id=omni_task_id,
            form=ResultForm.PARTIAL,
            content=content,
            persisted_at=persisted_at,
            integrity=ResultIntegrity.PARTIAL,
            sources=({"kind": "status", "value": state.value},
                     {"kind": "error", "value": str(error_code)}),
            note="已保留已接收输出，但不构成完整成果",
        )

    # -- 本地台账 -----------------------------------------------------------
    def list_task_records(self, *, limit: int = 100) -> TaskLedgerListing:
        limit = max(1, min(int(limit), 1000))
        if self._state_dir is None:
            return TaskLedgerListing(
                records=(), skipped_invalid=0, limit=limit,
                note="未配置引擎状态目录，无本地任务台账",
            )
        base = Path(self._state_dir)
        try:
            files = sorted(base.glob("omni-*.json"))
        except OSError as exc:
            raise EngineError(
                EngineErrorCode.RESULT_PERSIST_FAILED,
                "task ledger is unreadable: " + type(exc).__name__,
            ) from exc
        records: list[TaskRecordSummary] = []
        skipped = 0
        for path in files:
            try:
                data = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                skipped += 1
                continue
            if not isinstance(data, Mapping):
                skipped += 1
                continue
            task_id = data.get("omni_task_id")
            if not isinstance(task_id, str) or not task_id:
                skipped += 1
                continue
            state = str(data.get("state") or "unknown")
            records.append(
                TaskRecordSummary(
                    omni_task_id=task_id,
                    state=state,
                    intent=str(data.get("intent") or ""),
                    created_at=float(data.get("created_at") or 0.0),
                    state_since=float(data.get("state_since") or 0.0),
                    engine_session_id=(
                        str(data.get("engine_session_id"))
                        if data.get("engine_session_id")
                        else None
                    ),
                    cancel_phase=str(data.get("cancel_phase") or "not_requested"),
                    persistence="recorded",
                    session_mode=str(data.get("session_mode") or "new"),
                    ttfb_seconds=(
                        float(data["ttfb_seconds"])
                        if isinstance(data.get("ttfb_seconds"), (int, float))
                        else None
                    ),
                )
            )
        records.sort(key=lambda r: r.created_at, reverse=True)
        return TaskLedgerListing(
            records=tuple(records[:limit]),
            skipped_invalid=skipped,
            limit=limit,
            note="任务台账仅含元数据；恢复结果以 recover_task 为准，不自动重放",
        )

    # -- 4.7 recover_task ---------------------------------------------------
    def _probe_session(self, session_id: str, workspace_dir: str) -> None:
        """子类实现：确认会话是否仍可用。不可用时抛 EngineError。"""
        raise EngineError(
            EngineErrorCode.UNSUPPORTED, "session probing is not implemented for this channel"
        )

    def recover_task(self, omni_task_id: str, *, probe_session: bool = True) -> TaskRecovery:
        with self._lock:
            record = self._tasks.get(omni_task_id)
        if record is not None:
            with self._lock:
                state = record.state
            if state in TERMINAL_STATES:
                return TaskRecovery(
                    omni_task_id=omni_task_id,
                    status=self.get_task_status(omni_task_id),
                    replayed=False,
                    evidence=record.evidence_source,
                    note="进程内已知任务的终态",
                )
            return TaskRecovery(
                omni_task_id=omni_task_id,
                status=self.get_task_status(omni_task_id),
                replayed=False,
                evidence="process_state",
                note="进程内任务仍在进行；未自动重放",
            )
        persisted = self._read_persisted(omni_task_id)
        if persisted is None:
            raise EngineError(
                EngineErrorCode.UNKNOWN_TASK,
                "unknown omni_task_id (no local record): " + str(omni_task_id),
                detail={"omni_task_id": omni_task_id},
            )
        persisted_state = str(persisted.get("state") or "unknown")
        session_id = persisted.get("engine_session_id")
        if persisted_state in {s.value for s in TERMINAL_STATES}:
            status = TaskStatus(
                omni_task_id=omni_task_id,
                status=TaskState(persisted_state),
                evidence_source="engine_terminal",
                since=float(persisted.get("state_since") or 0.0),
                last_event_seq=int(persisted.get("seq") or 0),
                partial_output_size=len(str(persisted.get("content") or "")),
                engine_session_id=session_id,
                note="来自本地记录（前次运行取得的引擎终态证据）",
            )
            return TaskRecovery(
                omni_task_id=omni_task_id,
                status=status,
                replayed=False,
                evidence="engine_terminal",
                note="按本地记录恢复终态；未自动重放",
            )
        probe_note = ""
        probe_evidence = "process_state"
        if probe_session and session_id:
            try:
                self._probe_session(
                    str(session_id), str(persisted.get("workspace_dir") or "")
                )
                probe_note = "；会话仍可 resume（说明会话存活，但不代表该轮有终态证据）"
            except EngineError as error:
                probe_evidence = "engine_error"
                probe_note = "；会话不可恢复（" + error.code.value + "），该轮无法确认结果"
        status = TaskStatus(
            omni_task_id=omni_task_id,
            status=TaskState.UNKNOWN,
            evidence_source=probe_evidence,
            since=float(persisted.get("state_since") or 0.0),
            last_event_seq=int(persisted.get("seq") or 0),
            partial_output_size=len(str(persisted.get("content") or "")),
            engine_session_id=session_id,
            note="非终态记录：无引擎终态证据，标为 unknown，不自动重放" + probe_note,
        )
        return TaskRecovery(
            omni_task_id=omni_task_id,
            status=status,
            replayed=False,
            evidence=probe_evidence,
            note="恢复仅重建可发现状态；不自动重放中断的任务",
        )

    # -- 观测 ---------------------------------------------------------------
    def descriptor(self) -> Optional[EngineDescriptor]:
        with self._lock:
            return self._descriptor

    def task_records(self) -> list:
        with self._lock:
            return [self._record_payload(t) for t in self._tasks.values()]

    def forbidden_inferences(self) -> tuple:
        return FORBIDDEN_INFERENCES

    def _in_flight_ids(self) -> tuple[str, ...]:
        with self._lock:
            return tuple(t.omni_task_id for t in self._tasks.values() if t.state not in TERMINAL_STATES)

