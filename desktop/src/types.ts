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
  /** 引擎会话号：续接既有会话时与传入的一致；新建时是新号。 */
  engine_session_id?: string | null;
  /**
   * 本轮取得会话的方式（OD-ARCH-034）：new=新建；continued=在同一活会话内继续；
   * resumed=重开已落盘会话；degraded=续接失败、已降级为新建会话（本轮没有继承旧上文）。
   */
  session_mode?: "new" | "continued" | "resumed" | "degraded" | string;
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
// 模型权限与端点事实（OD-ARCH-021）
//
// 这些值全部由宿主从实际 DSH 配置与本地设置读出，界面只负责显示与开启/撤销。
// 真正的放行由 Rust 宿主在其判定入口强制执行；前端不具备放行能力。
// ---------------------------------------------------------------------------

// 端点类别只回答“是不是本机回环”，**不作**组织归属承诺：地址特征无法证明端点
// 属于哪个组织，也无法证明数据是否离开所在组织。
export type ModelCategory = "local" | "private" | "public" | "unknown";

export interface ModelPermissions {
  local: boolean;
  private: boolean;
  public: boolean;
}

export interface AdmissionFacts {
  provider: string;
  model: string;
  endpoint: string | null;
  endpoint_host: string | null;
  category: ModelCategory;
  category_reason: string;
  basis: string[];
  leaves_device: boolean | null;
  configurable: boolean;
  permissions: ModelPermissions;
  allowed: boolean;
}

export interface AdmissionStatusResponse {
  ok: boolean;
  facts: AdmissionFacts | null;
  error: HostError | null;
  /** OD-ARCH-022：权限变更未确认停止旧引擎时为 true，此时宿主拒绝新提交。 */
  blocked?: boolean;
  block_reason?: string | null;
}

export interface PermissionChangeResponse {
  ok: boolean;
  permissions: ModelPermissions;
  engine_stopped: boolean;
  blocked?: boolean;
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
  /** env_override = 开发/测试隔离变量覆盖；user = 用户在设置里选的；default = 默认目录 */
  source: "env_override" | "user" | "default" | null;
  configured_root: string | null;
  env_override: string | null;
  settings_path: string;
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
  reuse_willingness: boolean;
}

export interface UsageReadResult {
  records: UsageRecord[];
  unparsable_lines: number;
  path: string;
}

// ---------------------------------------------------------------------------
// 离线工作场景（合成数据，不调用模型）
// ---------------------------------------------------------------------------

export type OfflineScenario = "weekly_report" | "material_writing" | "project_management";

export interface WeeklyItem {
  date: string;
  item: string;
  status: "done" | "pending" | "blocked";
}

export interface WeeklySummary {
  schema: string;
  total: number;
  done: number;
  pending: number;
  blocked: number;
  items: WeeklyItem[];
  notes: string[];
}

export interface WritingPlan {
  schema: string;
  purpose: string;
  audience: string;
  sections: string[];
  missing: string[];
  template_used: string | null;
}

export interface ProjectItem {
  item: string;
  owner: string;
  due: string;
  status: "not_started" | "in_progress" | "blocked" | "done";
  next_step: string;
  overdue: boolean;
}

export interface ProjectSummary {
  schema: string;
  total: number;
  done: number;
  in_progress: number;
  blocked: number;
  not_started: number;
  overdue: number;
  items: ProjectItem[];
  notes: string[];
}

/** 三个离线流程共用的响应外壳：model_step 恒为「受准入阻塞、未验证」。 */
export interface OfflineFlowResponse<T> {
  ok: boolean;
  markdown: string | null;
  model_step: string;
  error: string | null;
  summary?: T;
  plan?: WritingPlan;
}

export interface WorkflowSample {
  sample_id: string;
  title: string;
  note: string;
  content: string;
}

/** 离线流程包：只含展示文本，由仓库外的 config/offline-flows.v1.json 提供。 */
export interface OfflineFlowCopy {
  tab_label: string;
  input_label?: string;
  input_placeholder?: string;
  today_label?: string;
  today_placeholder?: string;
  purpose_label?: string;
  audience_label?: string;
  requirements_label?: string;
  facts_label?: string;
  sample_label: string;
  result_title: string;
  details_heading?: string;
  structure_heading?: string;
  pending_heading?: string;
  confirm_heading?: string;
  /** 可编辑的场景模型规则（OD-ARCH-026）；缺省时界面使用内置默认规则。 */
  draft?: { rules?: string; sections?: string } | null;
}

/** 用户明确选择的材料文件（只读、单文件、有界；不含任何目录扫描结果）。 */
export interface MaterialRead {
  path: string;
  file_name: string;
  bytes: number;
  chars: number;
  extension: string;
  sha256: string;
  content: string;
}

export interface OfflineFlowPack {
  schema: string;
  weekly: OfflineFlowCopy;
  writing: OfflineFlowCopy;
  project: OfflineFlowCopy;
}

export interface OfflineFlowPackResponse {
  ok: boolean;
  pack: OfflineFlowPack | null;
  path: string;
  created: boolean;
  error: string | null;
}

/** 已保存离线成果的只读摘要；不含正文。 */
export interface OfflineArtifactEntry {
  /** 相对数据根的路径，子目录成果会带目录前缀。 */
  relative_path: string;
  file_name: string;
  bytes: number;
  modified_at: number;
  scenario: string;
  /** 目录行只用于展示层级，不能打开。 */
  is_dir: boolean;
  /** 相对数据根的层级，顶层为 0。 */
  depth: number;
}

export interface OfflineArtifactContent {
  file_name: string;
  content: string;
}

// ---------------------------------------------------------------------------
// 会话连续性与任务恢复（OD-ARCH-017）
//
// 只含元数据，不含任何会话正文。字段与
// engine/omnidesk_engine/acp_adapter.py 的 SessionListing / TaskLedgerListing
// 一一对应；UI 不得自行推导“可恢复”。
// ---------------------------------------------------------------------------

/** 会话记录结构是否可解析；不代表 resume 一定成功。 */
export interface SessionSummary {
  engine_session_id: string;
  /** 不含路径或目录名的稳定展示标识。 */
  workspace_label: string;
  /** 不透明工作区标识；重开时原样回传，界面不得自行拼路径。 */
  workspace_token: string;
  workspace_exists?: boolean;
  metadata_valid: boolean;
  resume_status: "unverified" | "resumed" | "failed";
}

export interface SessionGroup {
  workspace_label: string;
  workspace_token: string;
  sessions: SessionSummary[];
}

export interface SessionListing {
  groups: SessionGroup[];
  total: number;
  skipped_invalid: number;
  next_cursor: string | null;
  note: string;
}

/** 重开会话的结果。resumed=true 只说明引擎接受了恢复，不表示任务续跑。 */
export interface SessionRecovery {
  engine_session_id: string;
  resumed: boolean;
  workspace_label: string;
  workspace_token: string;
  config_option_count: number;
  evidence: string;
  note: string;
}

/** 任务台账元数据。刻意不含 content / 事件流。 */
export interface TaskRecordSummary {
  omni_task_id: string;
  state: string;
  created_at: number;
  state_since: number;
  engine_session_id: string | null;
  cancel_phase: string;
  persistence: string;
}

export interface TaskLedgerListing {
  records: TaskRecordSummary[];
  skipped_invalid: number;
  limit: number;
  note: string;
}

export interface RecoveredTaskStatus {
  omni_task_id: string;
  status: TaskState;
  evidence_source: string;
  since: number;
  engine_session_id?: string | null;
  note: string;
}

/** 重启后的任务状态重建结果；replayed 恒为 false 才符合“不自动重放”。 */
export interface TaskRecovery {
  omni_task_id: string;
  status: RecoveredTaskStatus;
  replayed: boolean;
  evidence: string;
  note: string;
}

/** 各 DSH Profile 的模型候选（只含非机密字段；OD-ARCH-041 提取为具名类型以便缓存）。 */
export interface ModelRouteListing {
  ok: boolean;
  engine_profile: string;
  overlay: string;
  profiles: Array<{
    profile: string;
    error?: string;
    default: { provider: string | null; model: string | null } | null;
    providers: Array<{
      id: string;
      displayName: string;
      api: string | null;
      apiKeyEnv: string | null;
      baseURL: string | null;
      models: string[];
    }>;
  }>;
}

// ---------------------------------------------------------------------------
// 模型目录（OD-ARCH-048，web-api 通道直读 session/modelCatalog）
// ---------------------------------------------------------------------------

/** 一个模型支持的思考等级（DSH `reasoning.efforts`）。 */
export interface ModelCatalogEffort {
  id: string;
  name: string;
  description: string;
}

/** 目录里的单个模型（非机密：只有标识、名称、描述与思考等级）。 */
export interface ModelCatalogEntry {
  id: string;
  name: string;
  description: string;
  provider: string;
  provider_name: string;
  efforts: ModelCatalogEffort[];
  default_effort: string | null;
}

/** 按 provider 分组的模型目录。 */
export interface ModelCatalogGroup {
  id: string;
  name: string;
  models: ModelCatalogEntry[];
}

/**
 * 引擎模型目录。
 *
 * `available=false` 表示当前通道不支持（ACP）或目录读取失败，界面必须回退到
 * Profile 候选列表，而不是伪造一份目录。
 */
export interface EngineModelCatalog {
  available: boolean;
  channel: string;
  default?: { provider?: string | null; model?: string | null; effort?: string | null };
  routable_providers?: string[];
  groups?: ModelCatalogGroup[];
  models?: ModelCatalogEntry[];
  failures?: Array<Record<string, unknown>>;
  /** unavailable 时的真实原因（引擎错误码 + 消息），原样展示。 */
  reason?: string;
}

// ---------------------------------------------------------------------------
// 待确认写操作与调度（OD-ARCH-046）
// ---------------------------------------------------------------------------

/** 待确认写操作的展示视图：由宿主生成，**不含正文**。 */
export interface PendingWriteView {
  id: string;
  action: string;
  /** model / scheduler：只有这两类来源需要用户确认。 */
  source: string;
  scenario: string;
  title: string;
  summary: string;
  impact: string;
  chars: number;
  created_at: number;
  expires_at: number;
  /** pending / consumed / rejected / expired */
  status: string;
}

export interface PendingWriteListing {
  items: PendingWriteView[];
  /** 无法解析的记录数（只计数，不伪造内容）。 */
  broken: number;
}

export interface PendingWriteConfirmReceipt {
  ok: boolean;
  id: string;
  status: string;
  path: string;
  bytes: number;
  written_at: number;
}

export type SchedulerCadence = "daily" | "weekly" | "once";

export interface SchedulerConfig {
  schema: string;
  review_enabled: boolean;
  cadence: SchedulerCadence;
  hour: number;
  once_at: number | null;
  utc_offset_minutes: number;
  heartbeat_enabled: boolean;
  heartbeat_minutes: number;
  last_review_at: number;
  last_heartbeat_at: number;
}

export interface SchedulerSnapshot {
  config: SchedulerConfig;
  hint: string;
}

export interface SchedulerTick extends SchedulerSnapshot {
  now: number;
  review_due: boolean;
  heartbeat_due: boolean;
}

export interface HeartbeatScan {
  ok: boolean;
  markdown: string | null;
  overdue: number;
  total: number;
  error: string | null;
}
