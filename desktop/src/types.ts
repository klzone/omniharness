// OmniDesk 桌面 UI 与 Rust 宿主之间的归一化类型（IPC-1）。
//
// 重要：这些类型的取值必须与 engine/omnidesk_engine/contract.py 一致。
// 本文件**不得**出现任何 ACP / DSH 内部字段名（sessionId、stopReason、
// session/update 等），那些字段封在 Python 引擎适配层内，不越过 IPC-2。

export type TaskState =
  | "queued"
  | "starting"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "interrupted"
  | "unknown";

export const TERMINAL_STATES: readonly TaskState[] = [
  "succeeded",
  "failed",
  "cancelled",
  "interrupted",
  "unknown",
];

export type EventType =
  | "task_started"
  | "text_delta"
  | "progress"
  | "tool_activity"
  | "task_completed"
  | "task_failed"
  | "cancel_requested"
  | "task_cancelled"
  | "task_interrupted"
  | "protocol_error";

/** 契约 §4.6：取消的五个阶段。只有 confirmed_cancelled 是取消成功的可靠证据。 */
export type CancelPhase =
  | "not_requested"
  | "request_sent"
  | "acknowledged"
  | "confirmed_cancelled"
  | "timeout_unknown";

export type ResultForm = "complete" | "partial" | "not_ready" | "unrecoverable";
export type ResultIntegrity = "complete" | "partial";

/** 与 Rust 侧 serde 序列化字段对应的任务事件（IPC-1 的 omni://task-event）。 */
export interface TaskEvent {
  omni_task_id: string;
  seq: number;
  occurred_at: number;
  type: EventType;
  payload: Record<string, unknown>;
}

export interface TaskStatus {
  omni_task_id: string;
  status: TaskState;
  state_since: number;
  cancel_phase: CancelPhase;
  note: string;
}

export interface TaskResult {
  omni_task_id: string;
  form: ResultForm;
  integrity: ResultIntegrity;
  content: string;
  content_omitted?: boolean;
  content_length?: number;
  sources: Array<Record<string, unknown>>;
  persisted_at: number | null;
  stop_reason?: string | null;
  note: string;
}

export interface SubmitTaskReceipt {
  omni_task_id: string;
  status: TaskState;
  accepted_at: number;
  note: string;
}

export interface CancelOutcome {
  omni_task_id: string;
  phase: CancelPhase;
  requested_at: number | null;
  settled_at: number;
  final_state: TaskState | null;
  evidence: string;
  note: string;
}

export interface EngineDescriptor {
  engine_kind: string;
  engine_version: string;
  protocol_version: string;
  compatibility: string;
  healthy: boolean;
  capabilities: Record<string, string>;
  detail: Record<string, unknown>;
}

/** 宿主侧结构化错误：Rust 把 EngineError 归一化成这个形状后交给 UI。 */
export interface HostError {
  engine_error_code: string;
  message: string;
  retryable?: string;
  needs_human?: string;
  degradable?: string;
  may_duplicate_execution?: string;
  may_lose_output?: string;
  omni_task_id?: string | null;
  detail?: Record<string, unknown>;
}

export interface EngineHealth {
  running: boolean;
  engine_version: string | null;
  protocol_version: string | null;
  compatibility: string | null;
  app_data_root: string;
  dsh_home: string | null;
  dsh_agents_home: string | null;
  engine_pid: number | null;
  note: string;
}

// ---------------------------------------------------------------------------
// 任务外壳（omnidesk-task-shell/1）
//
// 这些类型对应 desktop/src-tauri/src/task_shell.rs 的对外结构。
// 数值由本地确定性代码计算，UI 只展示，绝不自行统计或补齐。
// ---------------------------------------------------------------------------

/** 任务类型状态：只有 implemented 可执行；reserved 仅保留边界。 */
export type TaskTypeStatus = "implemented" | "reserved";

export interface TaskTypeInfo {
  task_type: string;
  title: string;
  status: TaskTypeStatus;
  data_scope: string;
  rule_asset: string | null;
  note: string;
}

export interface TaskTypeCatalog {
  shell_version: string;
  analysis_schema: string;
  types: TaskTypeInfo[];
}

export interface SampleInfo {
  sample_id: string;
  title: string;
  note: string;
  content: string;
}

export interface SampleCatalog {
  samples: SampleInfo[];
}

export interface CategoryStat {
  category: string;
  count: number;
  sum: number;
  mean: number;
  min: number;
  max: number;
}

export interface AnalysisResult {
  schema: string;
  rows: number;
  distinct_categories: number;
  total: number;
  mean: number;
  min: number;
  max: number;
  top_category: string | null;
  by_category: CategoryStat[];
  fingerprint: string;
}

/** 红线命中只含类别与计数，不回显命中原文。 */
export interface RedLineHit {
  kind: string;
  count: number;
  label: string;
}

export interface AnalysisRunResponse {
  ok: boolean;
  result: AnalysisResult | null;
  red_line_hits: RedLineHit[];
  error: string | null;
}

/**
 * 解释来源。只保留调用方可如实自证的两种：
 *   * user —— 使用者手写；
 *   * model_unverified —— 模型输出，但本阶段无法在代码层复核其成功终态。
 * 可伪造的 model（凭调用方传入的 task_state 字符串判定）已移除；
 * 可信模型来源需另行设计，不在当前范围。
 */
export type ExplanationSource = "user" | "model_unverified";

export interface SaveArtifactReceipt {
  path: string;
  fingerprint: string;
  bytes: number;
  written_at: number;
}

export interface LocalRootInfo {
  resolved: boolean;
  artifact_root: string | null;
  usage_log: string;
  app_data_root: string;
  error: string | null;
}

export interface UsageRecord {
  record_id: string;
  schema: string;
  task_type: string;
  recorded_at: number;
  completed: boolean;
  usable: boolean;
  rework_kind: string;
  blocker: string;
  duration_bucket: string | null;
}

export interface UsageReadResult {
  records: UsageRecord[];
  unparsable_lines: number;
  path: string;
}
