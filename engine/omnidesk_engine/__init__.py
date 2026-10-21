"""OmniDesk 引擎适配层（最小原型）。

本包实现 docs/OmniDesk-Engine-Interface-Contract.md 定义的最小引擎接口，
作为 OmniDesk 自有应用与底层执行引擎之间的可替换边界。

设计约束：
  * 只依赖 Python 标准库；不引入第三方依赖。
  * 不修改 DSH Runtime；所有对 DSH 的调用都通过公开 CLI 入口（`--profile acp`）。
  * 不读取、不打印、不复制任何凭证内容。
  * 引擎内部消息结构不越过本层：上层只看到归一化事件、状态与错误。

证据等级约定（与项目其它部分一致）：static / script / model / unverified。
"""

from .contract import (
    TASK_STATES,
    TERMINAL_STATES,
    CapabilityState,
    CancelPhase,
    EngineCapability,
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
    new_omni_task_id,
)
from .acp_adapter import (
    ACP_PROTOCOL_VERSION,
    CLIENT_CAPS,
    AcpEngineAdapter,
    CancelOutcome,
    SessionRecovery,
    SessionListing,
    SessionSummary,
    ShutdownReport,
    SubmitTaskReceipt,
    SubmitTaskRequest,
    TaskLedgerListing,
    TaskRecordSummary,
    TaskRecovery,
)
from .web_api_adapter import WEB_API_PROTOCOL_VERSION, WebApiAdapter

__all__ = [
    "ACP_PROTOCOL_VERSION",
    "CLIENT_CAPS",
    "AcpEngineAdapter",
    "CancelOutcome",
    "CapabilityState",
    "CancelPhase",
    "EngineCapability",
    "EngineDescriptor",
    "EngineError",
    "EngineErrorCode",
    "EngineEvent",
    "EventType",
    "ResultForm",
    "ResultIntegrity",
    "SessionRecovery",
    "SessionListing",
    "SessionSummary",
    "TaskLedgerListing",
    "TaskRecordSummary",
    "ShutdownReport",
    "SubmitTaskReceipt",
    "SubmitTaskRequest",
    "TASK_STATES",
    "TERMINAL_STATES",
    "TaskRecovery",
    "TaskResult",
    "TaskState",
    "TaskStatus",
    "WEB_API_PROTOCOL_VERSION",
    "WebApiAdapter",
    "new_omni_task_id",
]
