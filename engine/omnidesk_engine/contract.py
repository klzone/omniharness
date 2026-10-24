"""OmniDesk 引擎接口契约的词汇与状态模型（对照 docs/OmniDesk-Engine-Interface-Contract.md）。

本模块只定义**语义**：状态枚举、合法转换、冲突仲裁优先级、事件与错误分类、
标识分层。它不包含任何 ACP/DSH 细节——那些只允许出现在 acp_adapter 里。

契约对应关系：
  * 第 2 节 标识分层        -> new_omni_task_id() 与 TaskStatus.engine_session_id
  * 第 3 节 状态模型        -> TaskState / TASK_STATES / TERMINAL_STATES / LEGAL_TRANSITIONS
  * 3.3   冲突仲裁优先级    -> EVIDENCE_PRIORITY / arbitrate
  * 3.4   三条禁止推断      -> FORBIDDEN_INFERENCES（文档化约束）
  * 4.4   归一化事件类型    -> EventType
  * 4.5   结果形态          -> ResultForm / ResultIntegrity
  * 4.6   取消五阶段        -> CancelPhase
  * 4.9   十三类错误码      -> EngineErrorCode / ERROR_ATTRIBUTES
"""

from __future__ import annotations

import enum
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Mapping, Optional, Sequence


# ---------------------------------------------------------------------------
# 2. 标识分层
# ---------------------------------------------------------------------------


def new_omni_task_id() -> str:
    """生成一个稳定的 omni_task_id（OmniDesk 自有，长期可引用）。

    契约第 2 节要求该标识由 OmniDesk 生成并持久化，且与引擎侧 session id 分离。
    这里用 "omni-" 前缀 + uuid4，保证不与任何引擎内部标识格式混淆。
    """
    return "omni-" + str(uuid.uuid4())


# ---------------------------------------------------------------------------
# 3. 状态模型
# ---------------------------------------------------------------------------


class TaskState(str, enum.Enum):
    """OmniDesk 任务状态集（契约 3.1）。"""

    QUEUED = "queued"
    STARTING = "starting"
    RUNNING = "running"
    CANCELLING = "cancelling"
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    CANCELLED = "cancelled"
    INTERRUPTED = "interrupted"
    UNKNOWN = "unknown"


TASK_STATES: tuple[TaskState, ...] = tuple(TaskState)

TERMINAL_STATES: frozenset[TaskState] = frozenset(
    {
        TaskState.SUCCEEDED,
        TaskState.FAILED,
        TaskState.CANCELLED,
        TaskState.INTERRUPTED,
        TaskState.UNKNOWN,
    }
)

# 契约 3.2 合法状态转换表。值集合表示可到达的目标状态。
LEGAL_TRANSITIONS: Mapping[TaskState, frozenset[TaskState]] = {
    TaskState.QUEUED: frozenset(
        {TaskState.STARTING, TaskState.RUNNING, TaskState.CANCELLED, TaskState.FAILED, TaskState.UNKNOWN}
    ),
    TaskState.STARTING: frozenset(
        {TaskState.RUNNING, TaskState.FAILED, TaskState.INTERRUPTED, TaskState.UNKNOWN}
    ),
    TaskState.RUNNING: frozenset(
        {
            TaskState.SUCCEEDED,
            TaskState.FAILED,
            TaskState.CANCELLING,
            TaskState.INTERRUPTED,
            TaskState.UNKNOWN,
        }
    ),
    TaskState.CANCELLING: frozenset(
        {TaskState.CANCELLED, TaskState.FAILED, TaskState.INTERRUPTED, TaskState.UNKNOWN}
    ),
    TaskState.SUCCEEDED: frozenset(),
    TaskState.FAILED: frozenset(),
    TaskState.CANCELLED: frozenset(),
    TaskState.INTERRUPTED: frozenset(),
    TaskState.UNKNOWN: frozenset(),
}


def is_legal_transition(src: TaskState, dst: TaskState) -> bool:
    """判断 src -> dst 是否为契约 3.2 允许的转换。"""
    if src == dst:
        return True
    return dst in LEGAL_TRANSITIONS.get(src, frozenset())


# 契约 3.3 冲突仲裁优先级（数字越小优先级越高）。
EVIDENCE_PRIORITY: Mapping[str, int] = {
    "engine_terminal": 1,   # 引擎明确给出的终态证据
    "engine_error": 2,      # 引擎明确给出的结构错误
    "operator_confirm": 3,  # 操作者显式动作的确认记录
    "process_state": 4,     # 客户端进程状态
    "local_inference": 5,   # 本地推断（不得单独作为终态依据）
}

# 契约 3.4 三条禁止推断（文档化约束，供代码注释与测试引用）。
FORBIDDEN_INFERENCES: tuple[str, ...] = (
    "进程退出不得直接等同于任务成功",
    "收到最终文本片段不得直接等同于任务成功",
    "kill 进程不得直接等同于优雅取消（须区分 cancelled 与 process_terminated）",
)


def arbitrate(candidates: Sequence[tuple[str, TaskState]]) -> Optional[tuple[str, TaskState]]:
    """按契约 3.3 在冲突证据中选出最高优先级者。

    candidates 为 (evidence_source, proposed_state) 序列，evidence_source 需取自
    EVIDENCE_PRIORITY 的键。返回优先级最高的一项；同优先级按传入顺序取首个。
    """
    best: Optional[tuple[str, TaskState]] = None
    best_rank = 10**9
    for source, state in candidates:
        rank = EVIDENCE_PRIORITY.get(source)
        if rank is None:
            # 未声明的证据来源一律视为最低优先级，避免伪造高等级依据。
            rank = EVIDENCE_PRIORITY["local_inference"] + 1
        if rank < best_rank:
            best_rank = rank
            best = (source, state)
    return best


# ---------------------------------------------------------------------------
# 4.4 归一化事件类型
# ---------------------------------------------------------------------------


class EventType(str, enum.Enum):
    """OmniDesk 归一化事件类型（契约 4.4）。

    这些是 OmniDesk 自己的词汇。标注 [归一化] 的类型并非底层原生事件，
    而是由适配层按明确推导规则产生，不得声称底层原生支持。
    """

    TASK_STARTED = "task_started"
    TEXT_DELTA = "text_delta"
    PROGRESS = "progress"
    TOOL_ACTIVITY = "tool_activity"
    TASK_COMPLETED = "task_completed"
    TASK_FAILED = "task_failed"
    CANCEL_REQUESTED = "cancel_requested"        # [归一化] 本地事件
    TASK_CANCELLED = "task_cancelled"
    TASK_INTERRUPTED = "task_interrupted"
    PROTOCOL_ERROR = "protocol_error"


@dataclass(frozen=True)
class EngineEvent:
    """归一化引擎事件（契约 4.4）。

    omni_task_id : OmniDesk 任务标识
    seq          : 单调递增序号，用于去重与重排
    occurred_at  : 事件发生时间（epoch 秒，浮点）
    type         : EventType
    payload      : 事件载荷；不得包含凭证或敏感材料全文
    """

    omni_task_id: str
    seq: int
    occurred_at: float
    type: EventType
    payload: Mapping[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "seq": self.seq,
            "occurred_at": self.occurred_at,
            "type": self.type.value,
            "payload": dict(self.payload),
        }


# ---------------------------------------------------------------------------
# 4.5 结果形态 / 完整性
# ---------------------------------------------------------------------------


class ResultForm(str, enum.Enum):
    """结果返回形态（契约 4.5）。"""

    COMPLETE = "complete"
    PARTIAL = "partial"
    NOT_READY = "not_ready"
    UNRECOVERABLE = "unrecoverable"


class ResultIntegrity(str, enum.Enum):
    """结果完整性标记（契约 4.5）。部分结果不得冒充完整成果。"""

    COMPLETE = "complete"
    PARTIAL = "partial"


@dataclass(frozen=True)
class TaskResult:
    omni_task_id: str
    form: ResultForm
    content: str
    persisted_at: Optional[float]
    integrity: ResultIntegrity
    sources: tuple[Mapping[str, Any], ...] = ()
    note: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "form": self.form.value,
            "content": self.content,
            "persisted_at": self.persisted_at,
            "integrity": self.integrity.value,
            "sources": [dict(s) for s in self.sources],
            "note": self.note,
        }


# ---------------------------------------------------------------------------
# 4.6 取消五阶段
# ---------------------------------------------------------------------------


class CancelPhase(str, enum.Enum):
    """取消五阶段（契约 4.6）。"""

    NOT_REQUESTED = "not_requested"
    REQUEST_SENT = "request_sent"
    ACKNOWLEDGED = "acknowledged"
    CONFIRMED_CANCELLED = "confirmed_cancelled"
    TIMEOUT_UNKNOWN = "timeout_unknown"


# ---------------------------------------------------------------------------
# 4.1 能力标记三态
# ---------------------------------------------------------------------------


class CapabilityState(str, enum.Enum):
    """能力支持程度（契约 4.1）。与证据等级 static/script/model/unverified 配合使用。"""

    PROTOCOL_VISIBLE = "protocol_visible"
    IMPLEMENTATION_SUPPORTED = "implementation_supported"
    VERIFIED = "verified"
    PARTIAL = "partial"
    UNVERIFIED = "unverified"
    UNSUPPORTED = "unsupported"


class EngineCapability(str, enum.Enum):
    """契约 4.1 的七项能力键。"""

    SUBMIT_TASK = "submit_task"
    GET_TASK_STATUS = "get_task_status"
    STREAM_EVENTS = "stream_events"
    CANCEL_TASK = "cancel_task"
    RECOVER_SESSION = "recover_session"
    RECOVER_TASK = "recover_task"
    GET_TASK_RESULT = "get_task_result"


@dataclass(frozen=True)
class EngineDescriptor:
    """契约 4.1 的 initialize() 输出。"""

    engine_kind: str
    engine_version: str
    protocol_version: str
    compatibility: str  # compatible | degraded | incompatible
    healthy: bool
    capabilities: Mapping[EngineCapability, CapabilityState]
    detail: Mapping[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "engine_kind": self.engine_kind,
            "engine_version": self.engine_version,
            "protocol_version": self.protocol_version,
            "compatibility": self.compatibility,
            "healthy": self.healthy,
            "capabilities": {k.value: v.value for k, v in self.capabilities.items()},
            "detail": dict(self.detail),
        }


# ---------------------------------------------------------------------------
# 4.3 任务状态
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class TaskStatus:
    """契约 4.3 的 get_task_status() 输出。"""

    omni_task_id: str
    status: TaskState
    evidence_source: str
    since: float
    last_event_seq: int
    partial_output_size: int
    engine_session_id: Optional[str] = None
    note: str = ""
    # OD-ARCH-066：submit → 首个 text-delta 的秒数（仅流式通道可测；未到达为 None）。
    ttfb_seconds: Optional[float] = None
    # 等待用户审批的请求数（>0 时界面显示「等待审批中」）。
    pending_approvals: int = 0
    # 本轮 turn/end 事件在 DSH 会话里的精确序号：会话分支（session/fork）的切点。
    # 没有到达终态、或通道不提供序号时为 None，界面据此禁用「分支」按钮。
    engine_turn_end_seq: Optional[int] = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "omni_task_id": self.omni_task_id,
            "status": self.status.value,
            "evidence_source": self.evidence_source,
            "since": self.since,
            "last_event_seq": self.last_event_seq,
            "partial_output_size": self.partial_output_size,
            "engine_session_id": self.engine_session_id,
            "note": self.note,
            "ttfb_seconds": self.ttfb_seconds,
            "pending_approvals": self.pending_approvals,
            "engine_turn_end_seq": self.engine_turn_end_seq,
        }


# ---------------------------------------------------------------------------
# 4.9 错误模型
# ---------------------------------------------------------------------------


class EngineErrorCode(str, enum.Enum):
    """契约 4.9 的十三类标准错误码。"""

    INVALID_ARGUMENT = "invalid_argument"
    ENGINE_UNAVAILABLE = "engine_unavailable"
    PROTOCOL_INCOMPATIBLE = "protocol_incompatible"
    UNKNOWN_SESSION = "unknown_session"
    UNKNOWN_TASK = "unknown_task"
    UNSUPPORTED = "unsupported"
    TASK_FAILED = "task_failed"
    CANCEL_FAILED = "cancel_failed"
    RECOVER_FAILED = "recover_failed"
    TIMEOUT = "timeout"
    EVENT_PARSE_ERROR = "event_parse_error"
    RESULT_PERSIST_FAILED = "result_persist_failed"
    STATE_UNCERTAIN = "state_uncertain"


# 每类错误的五属性：是否可重试 / 是否需人工介入 / 是否允许降级 / 是否可能重复执行 / 是否可能丢失已接收输出。
ERROR_ATTRIBUTES: Mapping[EngineErrorCode, Mapping[str, str]] = {
    EngineErrorCode.INVALID_ARGUMENT: {
        "retryable": "no", "needs_human": "no", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "no",
    },
    EngineErrorCode.ENGINE_UNAVAILABLE: {
        "retryable": "conditional", "needs_human": "yes", "degradable": "yes",
        "may_duplicate_execution": "no", "may_lose_output": "no",
    },
    EngineErrorCode.PROTOCOL_INCOMPATIBLE: {
        "retryable": "no", "needs_human": "yes", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "no",
    },
    EngineErrorCode.UNKNOWN_SESSION: {
        "retryable": "no", "needs_human": "no", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
    EngineErrorCode.UNKNOWN_TASK: {
        "retryable": "no", "needs_human": "no", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "no",
    },
    EngineErrorCode.UNSUPPORTED: {
        "retryable": "no", "needs_human": "no", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "no",
    },
    EngineErrorCode.TASK_FAILED: {
        "retryable": "conditional", "needs_human": "yes", "degradable": "yes",
        "may_duplicate_execution": "yes", "may_lose_output": "no",
    },
    EngineErrorCode.CANCEL_FAILED: {
        "retryable": "conditional", "needs_human": "yes", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
    EngineErrorCode.RECOVER_FAILED: {
        "retryable": "conditional", "needs_human": "yes", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
    EngineErrorCode.TIMEOUT: {
        "retryable": "conditional", "needs_human": "no", "degradable": "yes",
        "may_duplicate_execution": "yes", "may_lose_output": "yes",
    },
    EngineErrorCode.EVENT_PARSE_ERROR: {
        "retryable": "no", "needs_human": "no", "degradable": "yes",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
    EngineErrorCode.RESULT_PERSIST_FAILED: {
        "retryable": "conditional", "needs_human": "yes", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
    EngineErrorCode.STATE_UNCERTAIN: {
        "retryable": "no", "needs_human": "yes", "degradable": "no",
        "may_duplicate_execution": "no", "may_lose_output": "yes",
    },
}


class EngineError(Exception):
    """统一错误对象（契约 4.9）。

    携带标准错误码与五属性，便于上层做重试/降级/人工介入判定。
    """

    def __init__(
        self,
        code: EngineErrorCode,
        message: str,
        *,
        detail: Optional[Mapping[str, Any]] = None,
        omni_task_id: Optional[str] = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.detail = dict(detail or {})
        self.omni_task_id = omni_task_id

    @property
    def attributes(self) -> Mapping[str, str]:
        return ERROR_ATTRIBUTES[self.code]

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code.value,
            "message": self.message,
            "detail": self.detail,
            "omni_task_id": self.omni_task_id,
            "attributes": dict(self.attributes),
        }

    def __str__(self) -> str:  # pragma: no cover - 便于日志
        return f"[{self.code.value}] {self.message}"


def now() -> float:
    """统一时间源（epoch 秒），便于测试注入与对比。"""
    return time.time()

