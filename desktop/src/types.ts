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
  /** OD-ARCH-066：提交到首个文本增量的秒数；未到达为 null。 */
  ttfb_seconds?: number | null;
  /** 等待人工审批的请求数；>0 时界面显示「等待审批中」。 */
  pending_approvals?: number;
  /** OD-ARCH-079：本轮 turn/end 的会话事件序号，DSH 原生 session/fork 的切点。 */
  engine_turn_end_seq?: number | null;
}

/** OD-ARCH-079：DSH 原生 session/fork 的结果。 */
export interface SessionFork {
  parent_session_id: string;
  engine_session_id: string;
  at_seq: number | null;
  note: string;
}

/** 审批请求（归一化自引擎事件，见 OD-ARCH-066）。 */
export interface ApprovalRequest {
  approvalId: string;
  omniTaskId: string;
  toolName: string;
  callId: string;
  reason: string;
  /** pending=等待决定；allowed/rejected=已回传；cancelled=引擎撤回；error=回传失败。 */
  status: "pending" | "allowed" | "rejected" | "cancelled" | "error";
  error?: string;
}

export interface ApprovalRespondReceipt {
  omni_task_id: string;
  approval_id: string;
  decision: "allowed" | "rejected";
  pending_approvals: number;
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
  /**
   * 本次出站文本被自动掩码的类别计数（OD-ARCH-068）；只回传数量，不回显原文。
   * 本地线程仍保留用户原文，只有送往引擎的提示词被替换。
   */
  privacy_masked?: { id_card: number; phone: number } | null;
  /** 本次提交是否加载了 Profile 规则（false = 纯净底座，零关键词零铁律）。 */
  profile_rules_loaded?: boolean | null;
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
  /** 引擎子进程最后几行 stderr（OD-ARCH-050：启动失败时用于定位真实原因）。 */
  stderr_tail?: string[];
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

/**
 * 对话分类（OD-ARCH-062 S1）：由宿主 `settings.v1` 持久化，界面只读这份列表。
 *
 * `key` 是内部键（plain/weekly/writing/project，或 `custom:...`），换显示名不改键，
 * 既有线程标签与成果目录因此零迁移。`resolved_dir` 只在映射可用时给出；
 * 映射越界或不存在时 `resolve_error` 必须显式展示，界面不得静默降级。
 */
export interface ThreadCategoryView {
  key: string;
  label: string;
  folder: string | null;
  vault: string | null;
  builtin: boolean;
  resolved_dir: string | null;
  resolve_error: string | null;
}

export interface CategorySettingsPayload {
  ok: boolean;
  settings_path: string;
  categories: ThreadCategoryView[];
}

export interface CategoryInitReceipt {
  ok: boolean;
  created: number;
  items: Array<{ key: string; path: string; created: boolean }>;
  note: string;
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

/** 可作为 @ 引用的文件（只含元数据；OD-ARCH-077 S1）。 */
export interface ReferenceCandidate {
  path: string;
  file_name: string;
  bytes: number;
  extension: string;
  /** 该文件所属来源根（成果目录或知识库）。 */
  source_root: string;
}

export interface ReferenceRoot {
  kind: "artifacts" | "knowledge";
  label: string;
  path: string;
  exists: boolean;
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

/**
 * 场景状态条的聚合数字（OD-ARCH-060 S1/S5）。
 *
 * 全部由宿主机从已落盘的记录与成果元数据数出来：不读正文、不调模型。
 */
export interface SceneOverview {
  scenario: string;
  pending: number;
  pending_expired: number;
  pending_broken: number;
  artifacts_total: number;
  artifacts_this_week: number;
  artifacts_this_month: number;
  latest_artifact_at: number | null;
  week_start: string;
  week_deadline: string;
}

export interface SceneOverviewResponse {
  ok: boolean;
  now: number;
  items: SceneOverview[];
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

/** 纪检 Profile 声明里的一个场景入口（OD-ARCH-051 S3）。 */
export interface ProfileSceneEntry {
  id: "home" | "report" | "writing" | "project" | "sessions";
  label: string;
  icon: "brand" | "note" | "book" | "checklist" | "history";
  note?: string;
}

export interface ProfileDecl {
  schema: string;
  display_name: string;
  scenes: ProfileSceneEntry[];
}

/** `profileDecl()` 的返回值：声明本体 + 落盘路径 + 是否首次创建 + 告警。 */
// ---------------------------------------------------------------------------
// Profile 规则资产（OD-ARCH-068）
//
// 平台代码不内置任何行业规则：红线词表、写作铁律、场景规则与思考档、隐私策略
// 全部来自用户可编辑的 `<数据根>/profiles/discipline.rules.json`。
// 结构必须与 Rust `profile_rules::ProfileRules` 对齐（schema 校验在宿主侧）。
// ---------------------------------------------------------------------------

/** 隐私处理方式：mask=掩码后放行；reject=命中即拒绝；off=不处理。 */
export type PrivacyMode = "mask" | "reject" | "off";

/** 场景默认思考档；default 表示不写该参数（跟随模型）。 */
export type ThinkingLevel = "default" | "off" | "low" | "medium" | "high" | "max";

export interface ProfileSceneRules {
  /** 与 Rust `SceneRules` 对齐：多行文本，不是数组。 */
  rules: string;
  /** 期望输出章节（每行一个）；空串表示不强制章节。 */
  sections: string;
  thinking: ThinkingLevel;
}

export interface ProfileRules {
  schema: string;
  profile: string;
  global: {
    writing_rules: string[];
    redline_keywords: string[];
    privacy: { id_card: PrivacyMode; phone: PrivacyMode };
    privacy_keywords: { id_card: string[]; phone: string[] };
  };
  scenes: Record<string, ProfileSceneRules>;
}

/**
 * 一次读取规则资产的结果。
 *
 * `loaded=false` 有两种含义，靠 `error` 区分：
 *   无 error  → 文件不存在（纯净底座，零规则）；
 *   有 error  → 文件损坏（提交会被宿主拒绝，界面必须显式提示）。
 */
export interface ProfileRulesView {
  loaded: boolean;
  path: string;
  text: string | null;
  content_sha256: string | null;
  rules: ProfileRules | null;
  error: string | null;
  default_available: boolean;
}

export interface ProfileRulesSaveReceipt {
  ok: boolean;
  path: string;
  backup: string | null;
  digest: string;
  note: string;
}

/** 能力管理（OD-ARCH-072）：一个 Skill 的只读条目。 */
export interface CapabilitySkill {
  name: string;
  description: string | null;
  /** Unix 秒；null 表示读不到修改时间。 */
  modified_at: number | null;
  path: string;
}

/** 能力管理：一个插件（bundle）的当前状态。 */
export interface CapabilityPlugin {
  name: string;
  version: string | null;
  enabled: boolean;
  builtin: boolean;
  installed: boolean;
}

export interface CapabilityView {
  profile_dir: string;
  manifest_path: string;
  skills_dir: string;
  skills: CapabilitySkill[];
  plugins: CapabilityPlugin[];
  market_installed: boolean;
  market_enabled: boolean;
  market_version: string | null;
  notes: string[];
}

export interface CapabilityReceipt {
  ok: boolean;
  path: string;
  backup: string | null;
  note: string;
}

export interface ProfileDeclPayload {
  decl: ProfileDecl;
  path: string;
  created: boolean;
  /** 载入时文件内容的 SHA-256；用于判断运行期间文件是否被改动（需重启生效）。 */
  content_sha256: string;
  warnings: string[];
  /** 声明原文（设置页编辑框载入用；保存前由宿主重新校验）。 */
  text: string;
}

// ---------------------------------------------------------------------------
// 原生设置页：模型与端点（OD-ARCH-053）
//
// 这里**没有**密钥字段：密钥只经官方凭据接口写入 DSH 凭据服务，
// 宿主从不把它回传给界面。界面只知道「是否已配置 / 来自哪一层」。
// ---------------------------------------------------------------------------

export interface ModelEntry {
  id: string;
  name: string;
  /** 上下文窗口（token）。只写配置；端点是否真支持要另行核实。 */
  contextWindow?: number | null;
}

export interface ProviderSettings {
  /** provider 标识，也是 DSH 配置里的键名，必须唯一。 */
  provider: string;
  display_name: string;
  base_url: string;
  api: string;
  /** DSH 配置里的 apiKeyEnv 引用名（非机密），每个提供商必须不同。 */
  api_key_ref: string;
  models: ModelEntry[];
  reasoning_effort?: string | null;
}

export interface DefaultRoute {
  provider: string;
  model: string;
}

/** 多提供商模型档案（v2）。每个提供商各存模型、地址与凭据引用。 */
export interface ModelSettings {
  schema: string;
  providers: ProviderSettings[];
  default: DefaultRoute;
  /**
   * 旧版遗留字段：任务等待上限（秒）。2026-10-23 起宿主不再向引擎下发等待上限，
   * 任务只在正常完成、底座报错/断流或用户取消时结束。字段保留只为兼容旧配置，
   * 仍需原样回传，否则保存会丢掉旧值。
   */
  task_timeout_seconds?: number;
  /**
   * 旧版遗留字段：联网检索等待上限（秒）。同 task_timeout_seconds，已不再参与
   * 任何收尾判断，仅为兼容旧配置而保留。
   */
  search_task_timeout_seconds?: number;
}

/** 凭据状态。值永不下发，只有这三项事实。 */
export interface ModelKeyStatus {
  configured: boolean;
  source?: string | null;
  writable: boolean;
}

export interface ModelSettingsView {
  ok: boolean;
  settings: ModelSettings | null;
  error: string | null;
  config_path: string;
  overlay_path: string;
  overlay_present: boolean;
  app_root: string;
  /** 凭据引用名 → 状态。只含 configured/source/writable，永不含值。 */
  key_status: Record<string, ModelKeyStatus> | null;
  key_error: string | null;
  /** 旧版单提供商配置文件路径（仅在迁移时用到）。 */
  legacy_config_path?: string;
}

export interface ModelSettingsSaveReceipt {
  ok: boolean;
  config_path: string;
  overlay_path: string;
  key_written: boolean;
  key_error: string | null;
  engine_stopped: boolean;
  route: unknown;
  note: string;
}

/**
 * 「测试连接 / 拉模型列表」的结果（OD-ARCH-054 S1/S2）。
 *
 * 只描述这一次请求的结果，不代表设置已保存。`classification` 是宿主给出的
 * 失败归类；界面据此给出下一步建议，不自行推断原因。
 */
export type ModelProbeClass =
  | "ok"
  | "auth"
  | "network"
  | "format"
  | "unsupported"
  | "local";

export interface DiscoveredModel {
  id: string;
  name: string;
}

export interface ModelProbeReceipt {
  ok: boolean;
  classification: ModelProbeClass;
  message: string;
  models: DiscoveredModel[];
  note: string;
  /** 这次请求用的密钥来源：沿用已保存 / 本次输入 / 未带密钥。 */
  key_source?: "stored" | "input" | "none" | string;
  /** 提供商名命中 DSH 内置目录：返回内置表，未发起真实网络请求。 */
  builtin_catalog?: boolean;
  /** 实际沿用了哪个已保存路由（未沿用时为空）。 */
  provider_route?: string | null;
  /** 表单端点与已保存路由不一致，因此没有沿用已存密钥。 */
  route_mismatch?: boolean;
}

export interface ProfileDeclOpenReceipt {
  ok: boolean;
  opened: string;
  note: string;
}

export interface ProfileDeclSaveReceipt {
  ok: boolean;
  path: string;
  backup: string | null;
  digest: string;
  note: string;
}

/** 顶栏 Profile 徽标（OD-ARCH-072）：一个可选的引擎 Profile。 */
export interface ProfileChoiceOption {
  /** 目录名，也是 DSH 的 profile 名。 */
  name: string;
  /** 界面显示名（优先取 package.json 的 name）。 */
  label: string;
  /** 是否当前通道的默认 Profile（「标准模式」）。 */
  standard: boolean;
}

/** 只读视图：隔离数据根里真实存在的 Profile + 本次启动实际使用的那个。 */
export interface ProfileChoiceView {
  ok: boolean;
  channel: string;
  standard: string;
  /** 用户上次保存的选择；null = 标准模式。 */
  saved: string | null;
  /** 本次启动实际生效的 Profile。 */
  effective: string;
  options: ProfileChoiceOption[];
  /** 引擎配置读不出来时的原因；此时 effective 是推算值。 */
  warning: string | null;
  note: string;
}

export interface ProfileChoiceReceipt {
  ok: boolean;
  saved: string | null;
  effective_next_start: string;
  note: string;
}
