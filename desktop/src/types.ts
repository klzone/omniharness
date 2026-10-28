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

/** 提问选项（OD-ARCH-088）：label 是回传值，description 只用于展示。 */
export interface QuestionOption {
  label: string;
  description?: string;
}

/** 一道可问作答的题目；multi_select 为 true 时可多选。 */
export interface QuestionItem {
  id: string;
  question: string;
  detail?: string;
  header?: string;
  options?: QuestionOption[];
  multi_select: boolean;
}

/**
 * 待答提问卡（归一化自 `PROGRESS kind="question_request"`）。
 * pending=等待作答；submitting=已提交待宿主确认；answered=宿主确认送达；
 * failed=回传失败可重试；closed=引擎撤回/超时或任务已终态，不可再答。
 */
export interface QuestionRequest {
  questionId: string;
  omniTaskId: string;
  callId: string;
  questions: QuestionItem[];
  answerable: boolean;
  violations: string[];
  status: "pending" | "submitting" | "answered" | "failed" | "closed";
  /** closed 的原因：cancelled（引擎撤回/超时）或 task_terminal（任务已结束）。 */
  closedReason?: "cancelled" | "task_terminal" | string;
  error?: string;
  /** 取消任务请求本身的失败原因（提问仍在等，不代表已关闭）。 */
  cancelError?: string;
  note?: string;
}

/** 一条答案；selected 为选项 label，custom 为自定义补充文本。 */
export interface QuestionAnswerItem {
  id: string;
  selected: string[];
  custom?: string;
}

export interface QuestionAnswerReceipt {
  omni_task_id: string;
  question_id: string;
  answered_at: number;
  pending_questions: number;
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

/** 知识库根状态（OD-KB-001 P1）：只描述已保存的选择与目录是否可用，不含文件内容。 */
export interface KnowledgeVaultRootStatus {
  configured: boolean;
  root: string | null;
  exists: boolean;
  is_dir: boolean;
  error: string | null;
}

/** 知识库内一个文件的元数据。note = 可读正文；attachment = 只列元数据。 */
export interface KnowledgeVaultFile {
  relative_path: string;
  file_name: string;
  extension: string;
  bytes: number;
  modified: number | null;
  kind: "note" | "attachment";
}

export interface KnowledgeVaultSkipped {
  relative_path: string;
  reason: string;
}

export interface KnowledgeVaultError {
  relative_path: string;
  message: string;
}

/** 一次只读扫描的统计。每次扫描都在内存里，不落盘、不建索引。 */
export interface KnowledgeVaultScanStats {
  visited_entries: number;
  visited_dirs: number;
  files: number;
  notes: number;
  attachments: number;
  total_bytes: number;
  skipped_hidden: number;
  skipped_excluded: number;
  skipped_symlink: number;
  skipped_non_file: number;
  /** 超过深度上限而跳过的目录子树（只跳过该子树）。 */
  skipped_depth: number;
  errors: number;
  truncated: boolean;
  details_truncated: boolean;
}

export interface KnowledgeVaultScan {
  root: string;
  files: KnowledgeVaultFile[];
  skipped: KnowledgeVaultSkipped[];
  errors: KnowledgeVaultError[];
  stats: KnowledgeVaultScanStats;
}

/** 单篇笔记正文（只读）。sha256 供界面显示来源事实，不保存副本。 */
export interface KnowledgeVaultNote {
  relative_path: string;
  file_name: string;
  bytes: number;
  chars: number;
  extension: string;
  sha256: string;
  content: string;
}

/** P2-S01：Frontmatter 属性值。数字保留 YAML 原文形态，避免展示时变形。 */
export type KnowledgeVaultPropertyValue =
  | { kind: "text" | "number"; value: string }
  | { kind: "bool"; value: boolean }
  | { kind: "null" }
  | { kind: "list"; value: string[] };

export interface KnowledgeVaultProperty {
  key: string;
  value: KnowledgeVaultPropertyValue;
}

export interface KnowledgeVaultParseWarning {
  code: string;
  message: string;
  key?: string | null;
}

export interface KnowledgeVaultFrontmatterSlice {
  raw: string;
  start_byte: number;
  end_byte: number;
  start_line: number;
  end_line: number;
  status: string;
}

/** 一篇笔记的只读解析结果；`raw` 与 `body` 都是原文切片，不做规范化。 */
export interface KnowledgeVaultParsedNote {
  raw: string;
  has_frontmatter: boolean;
  frontmatter: KnowledgeVaultFrontmatterSlice | null;
  body: string;
  body_start_byte: number;
  body_start_line: number;
  properties: KnowledgeVaultProperty[];
  tags: string[];
  aliases: string[];
  warnings: KnowledgeVaultParseWarning[];
}

export interface KnowledgeVaultSaveOutcome {
  relative_path: string;
  bytes: number;
  sha256: string;
  content: string;
  race_window: string;
}

export interface KnowledgeVaultPropertyFilter {
  key: string;
  value: string;
}

export interface KnowledgeVaultSearchQuery {
  keyword: string;
  tags: string[];
  properties: KnowledgeVaultPropertyFilter[];
}

export interface KnowledgeVaultSearchHit {
  relative_path: string;
  file_name: string;
  bytes: number;
  sha256: string;
  match_areas: string[];
  tags: string[];
  property_count: number;
  warning_count: number;
  snippet: string | null;
  /** 原文件行号（1 起），包含 Frontmatter 占用的行数。 */
  snippet_line: number | null;
  keyword_hits: number;
}

export interface KnowledgeVaultSearchSkip {
  relative_path: string;
  reason: string;
}

export interface KnowledgeVaultSearchResponse {
  hits: KnowledgeVaultSearchHit[];
  notes_available: number;
  notes_examined: number;
  bytes_read: number;
  budget_bytes: number;
  budget_exhausted: boolean;
  hits_truncated: boolean;
  skipped: KnowledgeVaultSearchSkip[];
  query: KnowledgeVaultSearchQuery;
}

/** P3-S02：索引里的一个笔记节点。orphan = 没有其他笔记指向它。 */
export interface KnowledgeVaultIndexNote {
  relative_path: string;
  file_name: string;
  sha256: string;
  bytes: number;
  backlink_count: number;
  outlink_count: number;
  orphan: boolean;
  lines: number;
  /** Frontmatter 标签（小写、去重）；P4 图谱按标签筛选使用。 */
  tags: string[];
  /** Frontmatter 别名（保留原始大小写）。 */
  aliases: string[];
}

/** 一条引用的解析状态。只有 resolved 且 target_path 非空时才能跳转。 */
export type KnowledgeVaultLinkStatus =
  | "resolved"
  | "unresolved"
  | "ambiguous"
  | "external"
  | "attachment";

export type KnowledgeVaultAnchorStatus = "exact" | "unlocated";

export interface KnowledgeVaultIndexLink {
  id: string;
  source_path: string;
  /** wikilink / embed / markdown_link / markdown_image / autolink */
  kind: string;
  target_raw: string;
  display: string | null;
  anchor: string | null;
  block_anchor: boolean;
  status: KnowledgeVaultLinkStatus;
  /** 只有 resolved 时有值；其余状态不得作为跳转目标。 */
  target_path: string | null;
  /** attachment 时的根内路径；仅用于说明，不打开正文。 */
  attachment_path: string | null;
  candidates: string[];
  start_byte: number;
  end_byte: number;
  line: number;
  context: string;
  anchor_status: KnowledgeVaultAnchorStatus | null;
  anchor_line: number | null;
}

export interface KnowledgeVaultIndexIssue {
  relative_path: string;
  code: string;
  message: string;
}

export interface KnowledgeVaultIndexStats {
  notes: number;
  notes_scanned: number;
  bytes_read: number;
  budget_bytes: number;
  links: number;
  resolved: number;
  unresolved: number;
  ambiguous: number;
  external: number;
  attachments: number;
  orphans: number;
  warnings: number;
}

/** 内存索引快照；complete=false 时界面必须显式提示不完整。 */
export interface KnowledgeVaultIndex {
  generation: number;
  built_at: number;
  root: string;
  complete: boolean;
  incomplete_reason: string | null;
  notes: KnowledgeVaultIndexNote[];
  links: KnowledgeVaultIndexLink[];
  issues: KnowledgeVaultIndexIssue[];
  stats: KnowledgeVaultIndexStats;
  fingerprint: string;
}

export interface KnowledgeVaultNoteLinks {
  generation: number;
  note: KnowledgeVaultIndexNote;
  outgoing: KnowledgeVaultIndexLink[];
  backlinks: KnowledgeVaultIndexLink[];
}

/** 库根信息（OD-ARCH-086）：周报、工作记录与 @ 引用的共同根。
 *  知识库另由独立的 knowledge_vault_root 配置（OD-KB-001 P1）。 */
export interface LibraryRootInfo {
  resolved: boolean;
  library_root: string | null;
  /** env_override = 环境变量覆盖；user = 设置里选的；default = 默认 ~/OmniDesk/库 */
  source: "env_override" | "user" | "default" | null;
  configured_root: string | null;
  env_override: string | null;
  default_root: string | null;
  settings_path: string;
  error: string | null;
}

/**
 * 随手记追加回执（OD-ARCH-089 S2）。
 *
 * 写入由宿主确定性完成，界面按回执显示真实结果：`ok=false` 时不能假装已记录。
 * `masked` 只报类别与数量，不回显原文。
 */
export interface WorklogAppendReceipt {
  ok: boolean;
  path: string | null;
  week: string;
  date: string;
  appended: boolean;
  was_new_file: boolean;
  continued: boolean;
  masked: boolean;
  masked_id_card: number;
  masked_phone: number;
  /** 这条记录在该日期段内的第几条（0 起）；改日期时用它定位。 */
  entry_index: number;
  /** 实际落盘的正文（可能已掩码）；改日期时原样带回做一致性校验。 */
  entry_text: string;
  error: string | null;
}

/**
 * 改日期回执（OD-ARCH-103 S3）。
 *
 * 只在本周文件内把一条记录从原日期段挪到目标日期段，正文不改写。
 * 失败时 `ok=false` 且文件一字未动。
 */
export interface WorklogRedateReceipt {
  ok: boolean;
  path: string | null;
  week: string;
  from_date: string;
  to_date: string;
  index: number;
  new_index: number;
  backup: string | null;
  error: string | null;
}

/**
 * 改去向回执（OD-ARCH-099 S02）。
 *
 * 界面点选「进周报」只改这一条记录在周文件里的去向字段；不写周报正文。
 * 失败时 `ok=false` 且文件一字未动，`error` 原样显示给用户。
 */
export interface WorklogDestinationReceipt {
  ok: boolean;
  path: string | null;
  week: string;
  date: string;
  index: number;
  destination: string;
  previous: string | null;
  backup: string | null;
  error: string | null;
}

/** 读当周工作记录（周报「有米」用）。文件不存在时 `exists=false`、`content` 为空。 */
export interface WorklogWeekRead {
  ok: boolean;
  exists: boolean;
  week: string;
  path: string | null;
  content: string;
  error: string | null;
}

/**
 * 一份周文件 / 周报文件的元数据（OD-ARCH-091 W3）。
 *
 * 只有宿主扫描目录得出的事实：文件名、字节数、修改时间。文件名不合约定的一律跳过，
 * 由宿主统计进 `skipped`，前端不猜、不改名。
 */
export interface WorklogWeekFile {
  /** 归一后的 ISO 周，形如 `2026-W44`。 */
  week: string;
  file_name: string;
  path: string;
  bytes: number;
  modified_at: number;
}

/** 归档清单（OD-ARCH-091 W3）：周记录与周报两份文件列表，均按周倒序、新在前。 */
export interface WorklogArchiveList {
  ok: boolean;
  library_root: string | null;
  /** 宿主按当前时区与偏移算出的当周标签。 */
  current_week: string;
  weeks: WorklogWeekFile[];
  reports: WorklogWeekFile[];
  /** 因文件名不合约定而被跳过的文件数（两份目录合计）。 */
  skipped: number;
  error: string | null;
}

/** 读一份周报正文（OD-ARCH-091）。文件名必须是 `2026-W44-周报.md` 形状。 */
export interface WorklogReportRead {
  ok: boolean;
  exists: boolean;
  file_name: string;
  path: string | null;
  content: string;
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
  /** 可选开始日期（OD-ARCH-098）。旧式五列行没有这一列，宿主返回 null。 */
  start: string | null;
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

/**
 * 项目事项的本地记录（OD-ARCH-102 S01）：应用数据根下的 project-items.v1.json。
 * 与 `ProjectItem` 的差别是不含 `overdue`（按当天现算），并带创建/更新时间。
 */
export type ProjectItemStatus = ProjectItem["status"];

export interface ProjectItemRecord {
  item: string;
  owner: string;
  due: string;
  start: string | null;
  status: ProjectItemStatus;
  next_step: string;
  created_at: number;
  updated_at: number;
}

export interface ProjectItemsListing {
  /** 实际存储文件（绝对路径；用于如实显示与排障）。 */
  path: string;
  items: ProjectItemRecord[];
  /** 首次使用或从 `.bak` 读回等情况的说明；正常时为 null。 */
  note: string | null;
  recovered_from_backup: boolean;
}

export interface ProjectItemsSaveReceipt {
  path: string;
  bytes: number;
  items: ProjectItemRecord[];
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
  kind: "artifacts" | "library" | "knowledge";
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
  /**
   * 写作成果的创作状态（OD-ARCH-094 S03）：`draft` / `finished` / `published`。
   * 只有 `material-writing` 场景会由宿主解析 frontmatter；其它场景为 null。
   */
  status?: string | null;
  /** 写作成果的内容类型：`insight` / `ai` / `blog` / `website` / `uncategorized`。 */
  artifact_type?: string | null;
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

export interface SceneSkillBinding {
  skills: string[];
  max_injected?: number;
}

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
  skill_bindings?: Record<string, SceneSkillBinding>;
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
  skill_contents?: Record<string, string>;
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
  /** 保存值是否在白名单之外（OD-ARCH-082 S6：不在白名单时本次回落默认）。 */
  saved_out_of_whitelist?: boolean;
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
