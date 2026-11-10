// IPC-1 客户端：UI 只管调用 Rust 宿主暴露的 Tauri 命令 / 事件。
//
// UI 不接触 IPC-2，不接触 ACP，不接触 DSH。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { currentEngineSubmitDecision } from "./engineAdmission";
import type {
  AdmissionStatusResponse,
  AnalysisRunResponse,
  AvailabilityConfig,
  CancelOutcome,
  CapabilityReceipt,
  CapabilityView,
  CategoryInitReceipt,
  CategorySettingsPayload,
  EngineDescriptor,
  EngineHealth,
  EnvironmentReport,
  ExplanationSource,
  LibraryRootInfo,
  LinkReport,
  MigrationPreview,
  MigrationReport,
  WorkingRootInfo,
  KnowledgeVaultNote,
  KnowledgeVaultIndex,
  KnowledgeVaultNoteLinks,
  KnowledgeVaultParsedNote,
  KnowledgeVaultRootStatus,
  KnowledgeVaultSaveOutcome,
  KnowledgeVaultScan,
  KnowledgeVaultSearchQuery,
  KnowledgeVaultSearchResponse,
  KnowledgeLinkReferences,
  KnowledgeLinkReport,
  KnowledgeLinkStageReport,
  KnowledgeLinkSubjectKind,
  KnowledgeIngestApplyReport,
  KnowledgeIngestInit,
  KnowledgeIngestPrepare,
  KnowledgeGovernanceRecord,
  KnowledgeRawAdmitReport,
  KnowledgeReviewDecision,
  KnowledgeReviewItem,
  KnowledgeReviewListing,
  KnowledgeQualityReport,
  LocalRootInfo,
  MaterialRead,
  OfflineFlowResponse,
  ProjectItemRecord,
  ProjectItemsListing,
  ProjectItemsSaveReceipt,
  ProjectDraftInput,
  ProjectInfo,
  ProjectListing,
  ProjectOverview,
  ProjectPatchInput,
  ProjectSummary,
  MaterialDetail,
  MaterialDraftInput,
  MaterialFilter,
  MaterialListing,
  MaterialPatchInput,
  MaterialQueryInput,
  MaterialQueryResponse,
  MaterialReviewAction,
  WritingCollectInput,
  WritingCollectResponse,
  WritingSelectInput,
  WritingSelectResult,
  WritingMaterial,
  WritingRequirementApplied,
  WritingRequirementDraft,
  WritingRequirementPrompt,
  WritingTask,
  WritingTaskCreated,
  WritingOutlineConfirmInput,
  WritingOutlineConfirmed,
  WritingOutlineGenerateInput,
  WritingOutlinePrompt,
  WritingSectionConfirmInput,
  WritingSectionConfirmed,
  WritingSectionGenerateInput,
  WritingSectionPrompt,
  WritingSectionReviseInput,
  WritingSectionRevisePrompt,
  WritingSectionState,
  FactCheckRunInput,
  FactCheckRunResult,
  FactCheckResolveInput,
  FactCheckResolveResult,
  WritingReviewApplyInput,
  WritingReviewApplied,
  WritingReviewInput,
  WritingReviewPrompt,
  WritingVersionDiff,
  WritingVersionDiffInput,
  WritingVersionList,
  WritingVersionRestoreInput,
  WritingVersionRestored,
  WritingExportInput,
  WritingExportOutcome,
  WritingArchiveInput,
  WritingArchiveOutcome,
  WritingKnowledgeFeedbackInput,
  WritingKnowledgeFeedbackOutcome,
  WritingStatusTransitionInput,
  WritingRecordsQueryInput,
  WritingRecordsQueryResult,
  ProfileDeclPayload,
  ParkedSkillView,
  ProfileAssetsView,
  ProfileAssetsRepairReceipt,
  ProfileAssemblyPrefs,
  ProfilePluginsAutofillEvent,
  ProfileDeclSavedEvent,
  ProfilePluginInstallReceipt,
  ProfilePluginsEnsureReceipt,
  ProfilePluginAddReceipt,
  ProfileRulesSaveReceipt,
  SkillBindingChoice,
  ProfileRulesView,
  ReferenceCandidate,
  ReferenceRoot,
  SampleCatalog,
  SaveArtifactReceipt,
  SessionListing,
  SessionFork,
  SessionRecovery,
  SubmitTaskReceipt,
  TaskEvent,
  TaskLedgerListing,
  TaskRecovery,
  TaskResult,
  TaskStatus,
  ApprovalRespondReceipt,
  QuestionAnswerReceipt,
  QuestionAnswerItem,
  TaskTypeCatalog,
  OfflineArtifactContent,
  OfflineArtifactEntry,
  OfflineFlowPackResponse,
  PendingWriteConfirmReceipt,
  PendingWriteListing,
  PendingWriteView,
  PermissionChangeResponse,
  SchedulerConfig,
  SchedulerSnapshot,
  SchedulerTick,
  SceneOverviewResponse,
  HeartbeatScan,
  UsageReadResult,
  UsageRecord,
  WeeklySummary,
  WorkflowSample,
  CandidateListing,
  AnalyzeApplied,
  AnalyzePrompt,
  ArchiveReceipt,
  CleanupPreview,
  CleanupReport,
  ConfirmEdits,
  RecordDraft,
  RecordFilter,
  RecordListing,
  RecordPatch,
  ScheduleAdjustmentReport,
  ScheduleEntry,
  ScheduleGenerateReport,
  ScheduleListing,
  TaskCandidate,
  TaskDeleted,
  TaskDraft,
  TaskDueKind,
  TaskFilter,
  TaskImportance,
  TaskListing,
  TaskPatch,
  TaskQuadrant,
  TaskQuadrantView,
  TaskSaved,
  TrashMove,
  WorkRecord,
  WorkRecordSaved,
  WorkTask,
  WorkTaskState,
  WorklogAppendReceipt,
  WorklogDestinationReceipt,
  WorklogRedateReceipt,
  WorklogMoveManyReceipt,
  WorklogMergeReceipt,
  WorklogWeekRead,
  WorklogWeekView,
  WorklogArchiveList,
  WorklogReportRead,
  ModelRouteListing,
  EngineModelCatalog,
  ModelSettings,
  ModelSettingsSaveReceipt,
  ModelProbeReceipt,
  ModelSettingsView,
  ProfileDeclOpenReceipt,
  ProfileDeclSaveReceipt,
  ProfileDeclStarter,
  ProfileChoiceView,
  ProfileChoiceReceipt,
  ProfileCreateReceipt,
  ProfileRemoveReceipt,
  ProfileRenameReceipt,
  ProfileTrashEntry,
  ProfileTrashRestoreReceipt,
  ProfileExportReceipt,
  ProfileImportReceipt,
  ProfileBundleListing,
} from "./types";

function normalizeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const maybe = error as { message?: unknown; engine_error_code?: unknown };
    if (typeof maybe.message === "string") {
      return typeof maybe.engine_error_code === "string"
        ? maybe.engine_error_code + "：" + maybe.message
        : maybe.message;
    }
    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }
  return String(error);
}


// ---------------------------------------------------------------------------
// 端点事实与模型候选的缓存（OD-ARCH-041）
//
// 这两项都要让宿主起一个 node 进程（~1-2 秒）：
//   * 端点分类（engine_admission_status → route_classifier）
//   * 模型候选列表（task_list_model_routes → model-routes.mjs）
//
// 场景切换会重新挂载对话区，于是每次切换都重拉一遍——同步执行时还会占住主线程，
// 表现为整页卡住 2-6 秒（实测：首页 → 工作记录页 6.07s，回到首页 3.51s）。
//
// 处置：宿主侧改成异步命令（不占 UI 线程）+ 这里加缓存（不必重复拉）。
// 失效点：改选模型/思考等级后立刻作废（模型与端点都可能变）。
// ---------------------------------------------------------------------------

/** 端点事实的缓存有效期（毫秒）。权限开关的变化由设置页自己刷新。 */
const ADMISSION_CACHE_MS = 2000;

let admissionCache: { at: number; value: AdmissionStatusResponse } | null = null;
let admissionInflight: Promise<AdmissionStatusResponse> | null = null;

let modelRoutesCache: ModelRouteListing | null = null;
let modelRoutesInflight: Promise<ModelRouteListing> | null = null;

// OD-ARCH-048：模型目录（engine_list_models）也会触达引擎——首次要冷启动 DSH web
// 进程。首页每次挂载都重拉，来回切换场景就会反复等待。目录只在改选模型或通道
// 变化时才会变，因此与模型候选一样缓存到 invalidateEngineFactsCache 为止。
let engineCatalogCache: EngineModelCatalog | null = null;
let engineCatalogInflight: Promise<EngineModelCatalog> | null = null;

/** 改选模型/思考等级后调用：相关事实与候选都可能变化。 */
export function invalidateEngineFactsCache(): void {
  admissionCache = null;
  admissionInflight = null;
  modelRoutesCache = null;
  modelRoutesInflight = null;
  engineCatalogCache = null;
  engineCatalogInflight = null;
}

export const host = {
  describe(): Promise<EngineDescriptor> {
    return invoke<EngineDescriptor>("engine_describe").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 运行时依赖体检（OD-ARCH-134）：只读探测 node / python 是否就绪。
   *
   * 启动页只在 `ok === false` 时显示「环境检查」，把安装入口摆到用户面前；
   * 依赖齐全时这次调用不产生任何界面变化。
   */
  environmentCheck(): Promise<EnvironmentReport> {
    return invoke<EnvironmentReport>("environment_check").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  health(): Promise<EngineHealth> {
    return invoke<EngineHealth>("engine_health").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：当前端点类别与用户授权的可核实事实。准入关闭时也可调用。
   *
   * 带短缓存（OD-ARCH-041）：宿主要起 node 进程分类端点（~1-2 秒），
   * 场景切换时反复拉取会卡住界面。失效点见 invalidateEngineFactsCache。
   */
  admissionStatus(): Promise<AdmissionStatusResponse> {
    const now = Date.now();
    if (admissionCache && now - admissionCache.at < ADMISSION_CACHE_MS) {
      return Promise.resolve(admissionCache.value);
    }
    if (!admissionInflight) {
      admissionInflight = invoke<AdmissionStatusResponse>("engine_admission_status")
        .then((value) => {
          admissionCache = { at: Date.now(), value };
          return value;
        })
        .catch((e) => {
          throw new Error(normalizeError(e));
        })
        .finally(() => {
          admissionInflight = null;
        });
    }
    return admissionInflight;
  },

  /**
   * 停止引擎进程（OD-ARCH-022 恢复路径）。
   * 权限变更未能确认旧引擎停止时会置位屏障并拒绝新提交；确认停止后屏障自动清除。
   */
  shutdownEngine(): Promise<{
    forced_terminate: boolean;
    process_exit_code: number | null;
    note: string;
  }> {
    return invoke<{ forced_terminate: boolean; process_exit_code: number | null; note: string }>(
      "engine_shutdown",
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 开启/关闭某一类端点的模型权限；未知类别由宿主拒绝。 */
  setModelPermission(
    category: "local" | "private" | "public",
    enabled: boolean,
  ): Promise<PermissionChangeResponse> {
    return invoke<PermissionChangeResponse>("model_permission_set", {
      category,
      enabled,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 一次性撤销全部模型权限。 */
  revokeAllModelPermissions(): Promise<PermissionChangeResponse> {
    return invoke<PermissionChangeResponse>("model_permission_revoke_all").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  submitTask(input: {
    promptText: string;
    intent?: string;
    workspaceDir?: string;
    idempotencyKey?: string;
    /** 用户原始输入（红线扫描对象）；只传用户写/粘贴的内容，不传提示词模板。 */
    userInputText?: string;
    /** 既有引擎会话号（OD-ARCH-034）：给出即续接同一会话（真多轮）；不给则新建。 */
    engineSessionId?: string | null;
  }): Promise<SubmitTaskReceipt> {
    // 前端只拦空文本（OD-ARCH-021）；端点类别与授权由宿主按实际配置强制判定。
    const decision = currentEngineSubmitDecision(input.promptText);
    if (!decision.allowed) {
      return Promise.reject(new Error(decision.reason));
    }
    return invoke<SubmitTaskReceipt>("engine_submit_task", {
      promptText: input.promptText,
      intent: input.intent ?? "material_writing",
      workspaceDir: input.workspaceDir ?? null,
      idempotencyKey: input.idempotencyKey ?? null,
      userInputText: input.userInputText ?? null,
      engineSessionId: input.engineSessionId ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskStatus(omniTaskId: string): Promise<TaskStatus> {
    return invoke<TaskStatus>("engine_task_status", { omniTaskId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskResult(omniTaskId: string): Promise<TaskResult> {
    return invoke<TaskResult>("engine_task_result", { omniTaskId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  cancelTask(omniTaskId: string, timeoutSeconds = 15): Promise<CancelOutcome> {
    return invoke<CancelOutcome>("engine_cancel_task", {
      omniTaskId,
      timeoutSeconds,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 回传审批决定（OD-ARCH-066）。失败时界面须保留待审批态。 */
  approvalRespond(
    omniTaskId: string,
    approvalId: string,
    allow: boolean,
  ): Promise<ApprovalRespondReceipt> {
    return invoke<ApprovalRespondReceipt>("engine_approval_respond", {
      omniTaskId,
      approvalId,
      allow,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 回传用户对结构化一次性提问的答案（OD-ARCH-088）。
   * 只有宿主返回收据才表示送达；失败时界面保留卡片供重试。
   */
  questionAnswer(
    omniTaskId: string,
    questionId: string,
    answers: QuestionAnswerItem[],
  ): Promise<QuestionAnswerReceipt> {
    return invoke<QuestionAnswerReceipt>("engine_question_answer", {
      omniTaskId,
      questionId,
      answers,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 会话清单（只含元数据）。需要启动引擎，宿主准入关闭时会直接拒绝。
   * 只读：不修改来源会话文件或注册表。
   */
  listSessions(input?: {
    cwd?: string | null;
    afterCursor?: string | null;
  }): Promise<SessionListing> {
    return invoke<SessionListing>("engine_list_sessions", {
      cwd: input?.cwd ?? null,
      afterCursor: input?.afterCursor ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 重开既有会话。恢复会话不等于恢复任务，不触发任何重放。 */
  recoverSession(
    engineSessionId: string,
    workspaceToken: string | null,
  ): Promise<SessionRecovery> {
    return invoke<SessionRecovery>("engine_recover_session", {
      engineSessionId,
      workspaceToken,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 会话分支（DSH 原生 session/fork）：按精确轮次序号建一条子会话。
   * `atSeq` 是本轮 turn/end 的会话事件序号；缺省时由引擎取最近一个完整轮次。
   * 失败原样抛给界面，不退回本地重放。
   */
  forkSession(engineSessionId: string, atSeq?: number | null): Promise<SessionFork> {
    return invoke<SessionFork>("engine_fork_session", {
      engineSessionId,
      atSeq: atSeq ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** OmniDesk 自己落盘的任务台账元数据（不含正文）。 */
  taskRecords(limit = 100): Promise<TaskLedgerListing> {
    return invoke<TaskLedgerListing>("engine_task_records", { limit }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 重启后重建可证实状态；没有终态证据的任务返回 unknown，且不重放。 */
  recoverTask(omniTaskId: string): Promise<TaskRecovery> {
    return invoke<TaskRecovery>("engine_recover_task", { omniTaskId }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },
};

/**
 * 任务外壳客户端（IPC-1）。
 *
 * 这些命令只涉及 OmniDesk 自己的概念，不接触引擎或 DSH。
 */
export const shell = {
  /**
   * 当前登录用户的显示名（只用于首页问候）。
   *
   * 宿主只读系统信息：macOS 取 `id -F`，其他平台回退 USER/USERNAME；
   * 读不到返回空串，界面就不带称呼，不编造名字。
   */
  desktopUserName(): Promise<string> {
    return invoke<string>("desktop_user_name").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskTypes(): Promise<TaskTypeCatalog> {
    return invoke<TaskTypeCatalog>("task_types").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  samples(): Promise<SampleCatalog> {
    return invoke<SampleCatalog>("task_samples").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  explanationRules(): Promise<{ rules: string }> {
    return invoke<{ rules: string }>("task_explanation_rules").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  localRootInfo(): Promise<LocalRootInfo> {
    return invoke<LocalRootInfo>("task_local_root_info").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 库根信息（OD-ARCH-086）：当前生效路径、来源与已保存选择。 */
  libraryRootInfo(): Promise<LibraryRootInfo> {
    return invoke<LibraryRootInfo>("task_library_root_info").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 追加一条随手记（OD-ARCH-089 S2）。
   *
   * 写盘由宿主确定性完成（只追加、不重写），模型只负责缺项追问。
   * 回执里 `ok=false` 必须原样呈现给用户，不能假装已记录。
   */
  worklogAppend(
    text: string,
    destination?: string,
    entryDate?: string,
  ): Promise<WorklogAppendReceipt> {
    return invoke<WorklogAppendReceipt>("worklog_append", {
      text,
      destination: destination ?? null,
      // OD-ARCH-103：输入里认出的补录日期（当前 ISO 周内、不晚于今天）；
      // null 表示没有认出，由宿主用当天，行为与 089 一致。
      entryDate: entryDate ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 改一条记录的「去向」（OD-ARCH-099 S02）。
   *
   * 只改周文件里属于这条记录的那一行。`expectText` 是界面读到的正文，宿主用它
   * 校验定位，对不上就拒绝并原样返回原因；`ok=false` 时文件没有变化。
   */
  worklogSetDestination(args: {
    week: string;
    date: string;
    index: number;
    destination: string;
    expectText: string;
  }): Promise<WorklogDestinationReceipt> {
    return invoke<WorklogDestinationReceipt>("worklog_set_destination", {
      week: args.week,
      date: args.date,
      index: args.index,
      destination: args.destination,
      expectText: args.expectText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 改一条记录的日期（OD-ARCH-103 S3）。
   *
   * 引擎看不到宿主命令，所以这条由界面在识别出纯日期纠正时直接调用，不叫模型。
   * `expectText` 是落盘回执里的正文，宿主用它校验定位；对不上就拒绝且不改文件。
   */
  worklogRedate(args: {
    week: string;
    fromDate: string;
    index: number;
    toDate: string;
    expectText: string;
  }): Promise<WorklogRedateReceipt> {
    return invoke<WorklogRedateReceipt>("worklog_redate", {
      week: args.week,
      fromDate: args.fromDate,
      index: args.index,
      toDate: args.toDate,
      expectText: args.expectText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 批量改日期（OD-ARCH-104）。
   *
   * 随手记的语义判定由模型给出，界面校验通过后把 moves 一次性交给宿主：
   * 一次读入、逐条按 `expectText` 校验、全部通过才重写一次。任一条对不上
   * 就整批拒绝，`ok=false` 且文件一字未动。
   */
  worklogMoveMany(args: {
    week: string;
    moves: Array<{ date: string; index: number; to: string; expectText: string }>;
  }): Promise<WorklogMoveManyReceipt> {
    return invoke<WorklogMoveManyReceipt>("worklog_move_many", {
      week: args.week,
      moves: args.moves,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 把一句补充并入已有记录（OD-ARCH-132）。
   *
   * 语义由模型判定（这句是不是在补充上一条），这里只执行：按 date + index +
   * expectText 定位，对不上就拒绝且文件一字未动；成功时只有目标行变长。
   */
  worklogMerge(args: {
    week: string;
    date: string;
    index: number;
    expectText: string;
    text: string;
  }): Promise<WorklogMergeReceipt> {
    return invoke<WorklogMergeReceipt>("worklog_merge", {
      week: args.week,
      date: args.date,
      index: args.index,
      expectText: args.expectText,
      text: args.text,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 读当周工作记录（周报用）。只读：不存在时 `exists=false`，不建文件。 */
  worklogReadWeek(week?: string): Promise<WorklogWeekRead> {
    return invoke<WorklogWeekRead>("worklog_read_week", { week: week ?? null }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 结构化周视图（OD-WR-311）：按记录库 ISO 周过滤，不解析 Markdown。 */
  worklogWeekView(week?: string): Promise<WorklogWeekView> {
    return invoke<WorklogWeekView>("worklog_week_view", { week: week ?? null }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 列周记录与周报归档（OD-ARCH-091 W3）。
   *
   * 只扫文件系统元数据，不读正文、不调模型。目录不存在返回空列表而不是错误；
   * 文件名不合约定（`2026-W99.md`、`随手记.md`……）由宿主跳过并在 `skipped` 计数，
   * 界面照实说明，不改名、不猜测。
   */
  worklogArchiveList(): Promise<WorklogArchiveList> {
    return invoke<WorklogArchiveList>("worklog_archive_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 读一份周报正文（OD-ARCH-091）。
   *
   * `fileName` 形如 `2026-W44-周报.md`；宿主先校验文件名再拼接路径，
   * 所以这里不拼路径、不做字符串清洗，交出去的只能是文件名。
   */
  worklogReadReport(fileName: string): Promise<WorklogReportRead> {
    return invoke<WorklogReportRead>("worklog_read_report", { fileName }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 报告归档进知识库「报告/」；同名同来源幂等（OD-WR-312）。 */
  archiveReportToVault(fileName: string, period?: string): Promise<ArchiveReceipt> {
    return invoke<ArchiveReceipt>("archive_report_to_vault", {
      fileName,
      period: period ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 已发布成果归档进知识库「归档成果/」；草稿拒绝。 */
  archiveArtifactToVault(relativePath: string): Promise<ArchiveReceipt> {
    return invoke<ArchiveReceipt>("archive_artifact_to_vault", {
      relativePath,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 临时层清理预览：只读，不写盘（OD-WR-313）。 */
  workCleanupPreview(): Promise<CleanupPreview> {
    return invoke<CleanupPreview>("work_cleanup_preview").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 临时层清理执行：必须 confirm；只移动到 .trash/。 */
  workCleanupApply(confirm: boolean): Promise<CleanupReport> {
    return invoke<CleanupReport>("work_cleanup_apply", { confirm }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 工作时间配置（OD-QS-406）：没有文件时返回默认值，不写盘。 */
  availabilityGet(): Promise<AvailabilityConfig> {
    return invoke<AvailabilityConfig>("availability_get").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存工作时间配置：宿主校验后原子替换，失败时旧文件保持不变。 */
  availabilitySet(config: AvailabilityConfig): Promise<AvailabilityConfig> {
    return invoke<AvailabilityConfig>("availability_set", { config }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  // -------------------------------------------------------------------------
  // 工作记录 / 任务 / 候选（OD-WR-302）
  //
  // 红线扫描与 mask_privacy 由宿主执行：命中红线时 invoke 直接 reject，
  // 错误里不回显原文；成功时回执带 masked 计数。
  // -------------------------------------------------------------------------

  workRecordCreate(draft: RecordDraft): Promise<WorkRecordSaved> {
    return invoke<WorkRecordSaved>("work_record_create", { draft }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  workRecordRead(id: string): Promise<WorkRecord> {
    return invoke<WorkRecord>("work_record_read", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  workRecordUpdate(id: string, patch: RecordPatch): Promise<WorkRecordSaved> {
    return invoke<WorkRecordSaved>("work_record_update", { id, patch }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 解析准备：宿主校验门禁、过红线与掩码后返回提示词。 */
  workRecordAnalyzePrepare(recordId: string): Promise<AnalyzePrompt> {
    return invoke<AnalyzePrompt>("worklog_record_analyze", { recordId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 解析落地：校验模型输出并暂存候选；失败时宿主把记录置为解析失败。 */
  workRecordAnalyzeApply(recordId: string, resultText: string): Promise<AnalyzeApplied> {
    return invoke<AnalyzeApplied>("worklog_record_analyze_apply", {
      recordId,
      resultText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 模型调用失败或超时：只把记录置为解析失败，正文不动。 */
  workRecordAnalyzeFailed(recordId: string): Promise<WorkRecord> {
    return invoke<WorkRecord>("worklog_record_analyze_failed", { recordId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  workRecordDelete(id: string): Promise<TrashMove> {
    return invoke<TrashMove>("work_record_delete", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  workRecordList(filter: RecordFilter = {}): Promise<RecordListing> {
    return invoke<RecordListing>("work_record_list", { filter }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskCreate(draft: TaskDraft): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_create", { draft }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskRead(id: string): Promise<WorkTask> {
    return invoke<WorkTask>("task_read", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskUpdate(id: string, patch: TaskPatch): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_update", { id, patch }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskTransition(
    id: string,
    toState: WorkTaskState,
    note?: string,
  ): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_transition", {
      id,
      toState,
      note: note ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskAppendProgress(id: string, text: string): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_append_progress", { id, text }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户调整重要性（OD-QS-405）：source 固定 user，理由必填。 */
  taskSetImportance(id: string, importance: TaskImportance, reason: string): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_set_importance", { id, importance, reason }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户指定或清除象限；quadrant 为 null 表示回到规则派生。 */
  taskSetQuadrant(
    id: string,
    quadrant: TaskQuadrant | null,
    reason?: string,
  ): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_set_quadrant", {
      id,
      quadrant,
      reason: reason ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户调整截止日期与性质；理由必填。 */
  taskSetDue(
    id: string,
    dueDate: string | null,
    dueKind: TaskDueKind,
    reason: string,
  ): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_set_due", {
      id,
      dueDate,
      dueKind,
      reason,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户补充预计耗时；null 表示清除。 */
  taskSetEstimated(id: string, minutes: number | null): Promise<WorkTask> {
    return invoke<WorkTask>("task_set_estimated", { id, minutes }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户设置前置任务；存在性与无环由宿主校验。 */
  taskSetDependency(id: string, dependsOn: string[]): Promise<WorkTask> {
    return invoke<WorkTask>("task_set_dependency", { id, dependsOn }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 任务四象限视图（OD-QS-410）：只读派生，不落盘。 */
  taskQuadrantViews(utcOffsetMinutes?: number): Promise<TaskQuadrantView[]> {
    return invoke<TaskQuadrantView[]>("task_quadrant_views", {
      utcOffsetMinutes: utcOffsetMinutes ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 重新生成排期建议（OD-QS-409）：不动已确认/锁定，只补新建议。 */
  scheduleGenerate(utcOffsetMinutes?: number): Promise<ScheduleGenerateReport> {
    return invoke<ScheduleGenerateReport>("schedule_generate", {
      utcOffsetMinutes: utcOffsetMinutes ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  scheduleConfirm(id: string): Promise<ScheduleEntry> {
    return invoke<ScheduleEntry>("schedule_confirm", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  scheduleAdjust(
    id: string,
    startAt: string,
    endAt: string,
    reason: string,
  ): Promise<ScheduleAdjustmentReport> {
    return invoke<ScheduleAdjustmentReport>("schedule_adjust", {
      id,
      startAt,
      endAt,
      reason,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  scheduleCancel(id: string, reason?: string): Promise<ScheduleEntry> {
    return invoke<ScheduleEntry>("schedule_cancel", {
      id,
      reason: reason ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  scheduleLock(id: string): Promise<ScheduleEntry> {
    return invoke<ScheduleEntry>("schedule_lock", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 列出排期条目及派生状态；任务完成/取消后显示「已失效」。 */
  scheduleList(): Promise<ScheduleListing> {
    return invoke<ScheduleListing>("schedule_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskDelete(id: string): Promise<TaskDeleted> {
    return invoke<TaskDeleted>("task_delete", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskList(filter: TaskFilter = {}): Promise<TaskListing> {
    return invoke<TaskListing>("task_list", { filter }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskCandidateList(): Promise<CandidateListing> {
    return invoke<CandidateListing>("task_candidate_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskCandidateConfirm(
    candidateId: string,
    edits: ConfirmEdits = {},
  ): Promise<TaskSaved> {
    return invoke<TaskSaved>("task_candidate_confirm", {
      candidateId,
      edits,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  taskCandidateReject(candidateId: string, note?: string): Promise<TaskCandidate> {
    return invoke<TaskCandidate>("task_candidate_reject", {
      candidateId,
      note: note ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 采纳关联建议：追加进展并双向回写任务/记录 ID（OD-WR-307）。 */
  taskCandidateLink(
    candidateId: string,
    progressText?: string,
    note?: string,
  ): Promise<LinkReport> {
    return invoke<LinkReport>("task_candidate_link", {
      candidateId,
      progressText: progressText ?? null,
      note: note ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 拒绝关联建议：清空匹配字段，候选继续留在待审核。 */
  taskCandidateDismissMatch(candidateId: string): Promise<TaskCandidate> {
    return invoke<TaskCandidate>("task_candidate_dismiss_match", {
      candidateId,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 知识关联推荐（OD-KM-507）：只产生候选，落 work/知识关联/.candidates/。
   * knowledgeHints 来自 504 的 knowledge_hints（字符串数组，最多 8 条）。
   */
  knowledgeLinkSuggest(
    subjectKind: KnowledgeLinkSubjectKind,
    subjectId: string,
    knowledgeHints: string[] = [],
  ): Promise<KnowledgeLinkStageReport> {
    return invoke<KnowledgeLinkStageReport>("knowledge_link_suggest", {
      subjectKind,
      subjectId,
      knowledgeHints,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户确认候选：理由必填并过宿主闸门，写入 related_knowledge。 */
  knowledgeLinkConfirm(
    candidateId: string,
    reason: string,
  ): Promise<KnowledgeLinkReport> {
    return invoke<KnowledgeLinkReport>("knowledge_link_confirm", {
      candidateId,
      reason,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 移除记录/任务的一条知识关联；不移除时原文件字节不变。 */
  knowledgeLinkRemove(
    subjectKind: KnowledgeLinkSubjectKind,
    subjectId: string,
    knowledgeId: string,
  ): Promise<KnowledgeLinkReport> {
    return invoke<KnowledgeLinkReport>("knowledge_link_remove", {
      subjectKind,
      subjectId,
      knowledgeId,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 反向查询：知识条目相对路径 → 引用它的记录/任务/素材（读取时派生）。 */
  knowledgeLinkByKnowledge(knowledgeId: string): Promise<KnowledgeLinkReferences> {
    return invoke<KnowledgeLinkReferences>("knowledge_link_by_knowledge", {
      knowledgeId,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 对话分类（OD-ARCH-062 S1）：读宿主持久化的分类表。
   *
   * 这是唯一的分类来源；显示名、归档文件夹与 vault 绑定都以这里为准。
   * 读取失败时调用方必须显式提示，不能悄悄回退到固定四类。
   */
  categorySettingsLoad(): Promise<CategorySettingsPayload> {
    return invoke<CategorySettingsPayload>("category_settings_load").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 改一个分类：只传要改的字段；folder/vault 传空字符串表示清除映射。 */
  categoryUpdate(patch: {
    key: string;
    label?: string;
    folder?: string;
    vault?: string;
  }): Promise<{ ok: boolean; count: number }> {
    return invoke<{ ok: boolean; count: number }>("category_update", { ...patch }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 新增自定义分类：内部键由宿主生成，新增分类不会出现在 Dock。 */
  categoryAdd(input: { label: string; folder?: string }): Promise<{ ok: boolean; key: string }> {
    return invoke<{ ok: boolean; key: string }>("category_add", { ...input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 删除自定义分类。内置四类会被宿主拒绝。 */
  categoryRemove(key: string): Promise<{ ok: boolean }> {
    return invoke<{ ok: boolean }>("category_remove", { key }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只创建尚不存在的分类目录；已有目录与文件原样保留。 */
  categoryInitFolders(): Promise<CategoryInitReceipt> {
    return invoke<CategoryInitReceipt>("category_init_folders").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：当前 Profile 的场景声明（OD-ARCH-051 S3，OD-ARCH-106 改为随 Profile 挂载）。
   *
   * 决定 Dock 上显示哪些业务场景入口。**返回 null 表示这个 Profile 没有场景声明**——
   * 基础标准（干净 DSH 底座）下这是正常状态，不是错误；宿主不会自动创建文件。
   * 只有解析/校验失败才 reject，调用方必须显式提示并回退内置入口，不能静默吞掉。
   */
  profileDecl(): Promise<ProfileDeclPayload | null> {
    return invoke<ProfileDeclPayload | null>("profile_decl_load").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：声明文件当前的内容指纹（OD-ARCH-052 第 2 步）。
   *
   * 与启动时载入的 `content_sha256` 比对，即可知道用户是否已在磁盘上改过声明。
   * 只返回指纹或 null（文件不存在），不创建文件、不改配置。
   */
  profileDeclDigest(): Promise<string | null> {
    return invoke<string | null>("profile_decl_digest").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：引擎 Profile 可选清单与本次启动实际使用的 Profile（OD-ARCH-072）。
   *
   * 只列隔离数据根里真实存在（含 package.json）的目录；标准 Profile 即使缺失也列出，
   * 由界面按存在性说明。不会创建目录、不装依赖、不改配置。
   */
  profileChoiceView(): Promise<ProfileChoiceView> {
    return invoke<ProfileChoiceView>("profile_choice_view").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 切换 Profile（null = 回到标准模式，OD-ARCH-110）。
   *
   * 写设置 → 装配资产 → 立即停止旧引擎，下一条任务按新 Profile 启动。
   * 返回的 engine_stopped 为 false 时表示旧引擎未确认停止，界面须如实提示。
   */
  profileChoiceSet(profile: string | null): Promise<ProfileChoiceReceipt> {
    return invoke<ProfileChoiceReceipt>("profile_choice_set", { profile }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 新建 / 复制自定义 Profile（OD-ARCH-109 补充）。
   *
   * `copyFrom = null` 从基础标准起手；否则复制那个 Profile 的配置与场景规则。
   * 只写隔离数据根，不装插件、不覆盖已有目录。
   */
  profileCreate(
    name: string,
    label: string,
    copyFrom: string | null,
  ): Promise<ProfileCreateReceipt> {
    return invoke<ProfileCreateReceipt>("profile_create", {
      name,
      label,
      copyFrom,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 移除自定义 Profile：移入回收目录（可恢复），出厂/当前 Profile 会被拒绝。 */
  profileRemove(name: string): Promise<ProfileRemoveReceipt> {
    return invoke<ProfileRemoveReceipt>("profile_remove", { name }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 改自定义 Profile 的显示名（OD-ARCH-126）。
   *
   * 只改登记表的 label 与场景声明的 display_name；目录名与 --profile 标识不变，
   * 因此 Skill 绑定、插件清单、线程分类与会话目录都不受影响。
   */
  profileRename(name: string, label: string): Promise<ProfileRenameReceipt> {
    return invoke<ProfileRenameReceipt>("profile_rename", { name, label }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：回收站里可恢复的自定义 Profile（OD-ARCH-125）。
   *
   * 与「已停放 Skill」同口径，只列清单，不建目录、不改文件、不联网。
   */
  profileTrashList(): Promise<ProfileTrashEntry[]> {
    return invoke<ProfileTrashEntry[]>("profile_trash_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 从回收站恢复一个 Profile：两次本地改名加一次登记写回。
   *
   * 同名已存在时拒绝且不动任何文件；不联网、不装插件。
   */
  profileTrashRestore(id: string): Promise<ProfileTrashRestoreReceipt> {
    return invoke<ProfileTrashRestoreReceipt>("profile_trash_restore", { id }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 把一个自定义 Profile 导出成单文件迁移包（OD-ARCH-127）。
   *
   * 纯本地读写：不联网、不装插件；包里不含 `node_modules`。
   * `destDir` 省略时落到应用数据根下的 `exports/`。
   */
  profileExport(
    name: string,
    destDir?: string | null,
  ): Promise<ProfileExportReceipt> {
    return invoke<ProfileExportReceipt>("profile_export", {
      name,
      destDir: destDir ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 从迁移包导入一个自定义 Profile（OD-ARCH-127）。
   *
   * 先校验后落盘，同名一律拒绝；插件本体不进包，导入后按清单提示安装。
   */
  profileImport(
    path: string,
    renameTo?: string | null,
    label?: string | null,
  ): Promise<ProfileImportReceipt> {
    return invoke<ProfileImportReceipt>("profile_import", {
      path,
      renameTo: renameTo ?? null,
      label: label ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只读：导出目录里已有的迁移包，供设置页一键导入。 */
  profileExportList(): Promise<ProfileBundleListing[]> {
    return invoke<ProfileBundleListing[]>("profile_export_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在系统文件管理器里打开导出目录（不存在时先建）。 */
  profileExportOpen(): Promise<{ ok: boolean; opened: string }> {
    return invoke<{ ok: boolean; opened: string }>("profile_export_open").catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 只读：某个 Profile 的出厂资产就绪情况（OD-ARCH-109）。
   *
   * 不传 `profile` 时看当前运行中的实例；传了就预览该 Profile。只判定不写盘，
   * 写动作只发生在切换 Profile 与启动引擎时。
   */
  profileAssetsView(profile?: string | null): Promise<ProfileAssetsView> {
    return invoke<ProfileAssetsView>("profile_assets_view", {
      profile: profile ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 就地修复某个 Profile 的资产（OD-ARCH-113）。
   *
   * 只在用户点「修复缺失 / 恢复出厂内容」时调用：补齐缺失项，被用户改过的文件
   * 先备份成 \`.bak-<epoch>\` 再按出厂内容写回。不联网、不装插件。
   */
  profileAssetsRepair(
    profile?: string | null,
  ): Promise<ProfileAssetsRepairReceipt> {
    return invoke<ProfileAssetsRepairReceipt>("profile_assets_repair", {
      profile: profile ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：停在 `profiles/.parked-skills/` 里的 Skill（OD-ARCH-117）。
   *
   * 切换 Profile 时不属于目标的 Skill 会被移入停放区（不删除），切回时自动归位。
   * 这个调用只列清单，不建目录、不改文件。
   */
  profileAssetsParked(): Promise<ParkedSkillView[]> {
    return invoke<ParkedSkillView[]>("profile_assets_parked").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：Profile 装配偏好（OD-ARCH-121）。目前只有「切换时自动补齐缺失插件」。
   *
   * 读不出来时宿主按默认「开启」返回，不会因为坏文件悄悄退回「只提示」。
   */
  profileAssemblyPrefs(): Promise<ProfileAssemblyPrefs> {
    return invoke<ProfileAssemblyPrefs>("profile_assembly_prefs").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存「切换时自动补齐缺失插件」开关；其余设置原样保留。 */
  profileAssemblyPrefsSet(autoInstallPlugins: boolean): Promise<ProfileAssemblyPrefs> {
    return invoke<ProfileAssemblyPrefs>("profile_assembly_prefs_set", {
      autoInstallPlugins,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 通过 DSH 官方插件通道安装某个 Profile 缺的插件（OD-ARCH-109）。
   *
   * 会联网、会写 profile 目录，只由用户点「安装」触发；失败时回执里带原样输出。
   */
  profilePluginInstall(
    profile: string,
    pkg: string,
  ): Promise<ProfilePluginInstallReceipt> {
    return invoke<ProfilePluginInstallReceipt>("profile_plugin_install", {
      profile,
      package: pkg,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 一次补齐某个 Profile 缺失的插件（OD-ARCH-114）。
   *
   * 只装探测为缺失/版本不符的包，单个失败不中断其余；安装后重新探测并回报
   * 仍然缺的项。会联网、只由用户点「安装」触发。
   */
  profilePluginsEnsure(
    profile?: string | null,
  ): Promise<ProfilePluginsEnsureReceipt> {
    return invoke<ProfilePluginsEnsureReceipt>("profile_plugins_ensure", {
      profile: profile ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 给自建 Profile 声明一个插件（OD-ARCH-128）。
   *
   * 本地已就绪就只补登记；缺件才走 DSH 插件通道联网装，装成功才写登记表。
   * 基础标准与出厂 Profile 都会被宿主拒绝，理由放在错误文案里。
   */
  profilePluginsAdd(
    profile: string | null,
    pkg: string,
  ): Promise<ProfilePluginAddReceipt> {
    return invoke<ProfilePluginAddReceipt>("profile_plugins_add", {
      profile,
      package: pkg,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存新的成果目录。只影响之后新建的成果，不迁移、不覆盖既有文件。 */
  setArtifactRoot(path: string): Promise<{
    ok: boolean;
    artifact_root: string;
    note: string;
  }> {
    return invoke<{ ok: boolean; artifact_root: string; note: string }>(
      "task_set_artifact_root",
      { path },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存新的库根。只影响之后的 @ 引用与技能写入，不迁移、不覆盖既有文件。 */
  setLibraryRoot(path: string): Promise<{
    ok: boolean;
    library_root: string;
    note: string;
  }> {
    return invoke<{ ok: boolean; library_root: string; note: string }>(
      "task_set_library_root",
      { path },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 工作记录根信息（OD-WR-315）：当前生效路径、来源与已保存选择。 */
  workRootInfo(): Promise<WorkingRootInfo> {
    return invoke<WorkingRootInfo>("work_root_info").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 存量周文件迁移预览：只解析、不写盘（OD-WR-309）。 */
  worklogMigratePreview(): Promise<MigrationPreview> {
    return invoke<MigrationPreview>("worklog_migrate_preview").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 执行存量周文件迁移；confirm 必须为 true。 */
  worklogMigrateApply(confirm: boolean): Promise<MigrationReport> {
    return invoke<MigrationReport>("worklog_migrate_apply", { confirm }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存工作记录根。只影响之后新建的记录与任务。 */
  setWorkingRoot(path: string): Promise<{
    ok: boolean;
    working_root: string;
    note: string;
  }> {
    return invoke<{ ok: boolean; working_root: string; note: string }>(
      "task_set_working_root",
      { path },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在系统文件管理器中打开当前成果目录（只读）。 */
  /** 知识库根状态（OD-KB-001 P1）：只读，不扫描、不读正文。 */
  knowledgeVaultStatus(): Promise<KnowledgeVaultRootStatus> {
    return invoke<KnowledgeVaultRootStatus>("knowledge_vault_status").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存知识库根。宿主只写设置文件，不创建目录、不做写入探针。 */
  setKnowledgeVaultRoot(path: string): Promise<{
    ok: boolean;
    knowledge_vault_root: string;
    note: string;
  }> {
    return invoke<{ ok: boolean; knowledge_vault_root: string; note: string }>(
      "knowledge_vault_set_root",
      { path },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 清除知识库根选择；不动知识库内任何文件。 */
  clearKnowledgeVaultRoot(): Promise<{ ok: boolean }> {
    return invoke<{ ok: boolean }>("knowledge_vault_clear_root").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只读扫描或刷新已保存的知识库根；结果只在内存，不建立索引。 */
  knowledgeVaultScan(): Promise<KnowledgeVaultScan> {
    return invoke<KnowledgeVaultScan>("knowledge_vault_scan").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 读取知识库内一篇 Markdown/纯文本笔记；附件只列元数据，不读正文。 */
  knowledgeVaultReadNote(relativePath: string): Promise<KnowledgeVaultNote> {
    return invoke<KnowledgeVaultNote>("knowledge_vault_read_note", {
      relativePath,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 解析 Frontmatter、标签与正文边界（只读）。 */
  knowledgeVaultParseNote(
    relativePath: string,
  ): Promise<KnowledgeVaultParsedNote> {
    return invoke<KnowledgeVaultParsedNote>("knowledge_vault_parse_note", {
      relativePath,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 新建一篇根内 Markdown 笔记；目标已存在时宿主拒绝，绝不覆盖。 */
  knowledgeVaultCreateNote(
    relativePath: string,
    content: string,
  ): Promise<KnowledgeVaultNote> {
    return invoke<KnowledgeVaultNote>("knowledge_vault_create_note", {
      relativePath,
      content,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存已有笔记；expectedSha256 来自打开时读取，冲突时宿主不写入。 */
  knowledgeVaultSaveNote(
    relativePath: string,
    content: string,
    expectedSha256: string,
  ): Promise<KnowledgeVaultSaveOutcome> {
    return invoke<KnowledgeVaultSaveOutcome>("knowledge_vault_save_note", {
      relativePath,
      content,
      expectedSha256,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在已扫描笔记上按关键词、标签与属性筛选；不建立持久索引。 */
  knowledgeVaultSearch(
    query: KnowledgeVaultSearchQuery,
  ): Promise<KnowledgeVaultSearchResponse> {
    return invoke<KnowledgeVaultSearchResponse>("knowledge_vault_search", {
      query,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用系统处理程序打开已校验的根内附件；不读取附件正文。 */
  knowledgeVaultOpenAttachment(relativePath: string): Promise<string> {
    return invoke<string>("knowledge_vault_open_attachment", {
      relativePath,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 显式重建内存链接索引（P3-S04）；不写知识库、不启动后台监听。 */
  knowledgeVaultRebuildIndex(): Promise<KnowledgeVaultIndex> {
    return invoke<KnowledgeVaultIndex>("knowledge_vault_rebuild_index").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 读取当前已发布索引；没有索引时为 null，不触发重建。 */
  knowledgeVaultIndexStatus(): Promise<KnowledgeVaultIndex | null> {
    return invoke<KnowledgeVaultIndex | null>("knowledge_vault_index_status").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只读当前索引里一篇笔记的正向链接与反向链接。 */
  knowledgeVaultNoteLinks(relativePath: string): Promise<KnowledgeVaultNoteLinks> {
    return invoke<KnowledgeVaultNoteLinks>("knowledge_vault_note_links", {
      relativePath,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 建立收件箱/概念/方法/原文骨架与空索引（幂等）。 */
  knowledgeIngestInit(): Promise<KnowledgeIngestInit> {
    return invoke<KnowledgeIngestInit>("knowledge_ingest_init").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 收件箱前 5 份材料脱敏后组装整理提示词。 */
  knowledgeIngestPrepare(): Promise<KnowledgeIngestPrepare> {
    return invoke<KnowledgeIngestPrepare>("knowledge_ingest_prepare").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 校验模型 JSON 并逐卡落盘、更新索引、归档原文。 */
  knowledgeIngestApply(resultText: string): Promise<KnowledgeIngestApplyReport> {
    return invoke<KnowledgeIngestApplyReport>("knowledge_ingest_apply", {
      resultText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 把文件字节列表准入 raw/（S1 宿主命令；前端通常走路径版本）。 */
  knowledgeRawAdmit(
    files: Array<{ name: string; bytes: number[] }>,
  ): Promise<KnowledgeRawAdmitReport> {
    return invoke<KnowledgeRawAdmitReport>("knowledge_raw_admit", { files }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 从系统文件选择框给出的绝对路径读取并准入 raw/。 */
  knowledgeRawAdmitPaths(paths: string[]): Promise<KnowledgeRawAdmitReport> {
    return invoke<KnowledgeRawAdmitReport>("knowledge_raw_admit_paths", { paths }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只读治理记录。写治理文件没有前端命令。 */
  knowledgeGovernanceList(kind: string): Promise<KnowledgeGovernanceRecord[]> {
    return invoke<KnowledgeGovernanceRecord[]>("knowledge_governance_list", { kind }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 整理结果先进入待审核队列，不直接写知识库。 */
  knowledgeReviewStage(resultText: string, sourcePaths: string[] = []): Promise<KnowledgeReviewItem> {
    return invoke<KnowledgeReviewItem>("knowledge_review_stage", { resultText, sourcePaths }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  knowledgeReviewList(): Promise<KnowledgeReviewListing> {
    return invoke<KnowledgeReviewListing>("knowledge_review_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  knowledgeReviewDecide(id: string, decision: string): Promise<KnowledgeReviewDecision> {
    return invoke<KnowledgeReviewDecision>("knowledge_review_decide", { id, decision }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  knowledgeConflictResolve(
    subject: string,
    decision: "resolved" | "deferred" | "needs_more",
    note = "",
  ): Promise<KnowledgeGovernanceRecord> {
    return invoke<KnowledgeGovernanceRecord>("knowledge_conflict_resolve", {
      subject,
      decision,
      note,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 确定性过期检查；不调用模型。 */
  knowledgeQualityReport(): Promise<KnowledgeQualityReport> {
    return invoke<KnowledgeQualityReport>("knowledge_quality_report").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在系统文件管理器中打开当前成果目录（只读）。 */
  openArtifactRoot(): Promise<{ ok: boolean; opened: string }> {
    return invoke<{ ok: boolean; opened: string }>(
      "task_open_artifact_root",
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 列出各 DSH Profile 生效的 provider / 模型（只取非机密字段；不读凭证值）。
   *
   * 不同 Profile 的**模型选择互不相通**（只有凭据共享），所以这里把各 Profile 的候选都列出来。
   *
   * 带缓存（OD-ARCH-041）。
   *
   * 宿主侧同样要起 node 进程（~1-2 秒）；候选只在改选模型时变化，
   * 因此缓存到下次 invalidateEngineFactsCache 为止。
   */
  listModelRoutes(): Promise<ModelRouteListing> {
    if (modelRoutesCache) return Promise.resolve(modelRoutesCache);
    if (!modelRoutesInflight) {
      modelRoutesInflight = invoke<ModelRouteListing>("task_list_model_routes")
        .catch((e) => {
          throw new Error(normalizeError(e));
        })
        .finally(() => {
          modelRoutesInflight = null;
        })
        .then((value) => {
          modelRoutesCache = value;
          return value;
        });
    }

    return modelRoutesInflight;
  },

  /**
   * 选定模型：把该 Profile 的 provider 配置 + 选中的模型写成 OmniDesk 自己的覆盖文件
   * （只含非机密字段），并重启引擎、重新判定端点。
   */
  setModelRoute(input: {
    sourceProfile: string;
    provider: string;
    model: string;
    /** 思考等级（对应 DSH 的 reasoningEffort）；默认 "default"＝不传该参数。 */
    reasoningEffort?: string;
  }): Promise<{
    ok: boolean;
    route: { provider: string; model: string; endpoint: string | null; category: string };
    note: string;
  }> {
    return invoke<{
      ok: boolean;
      route: { provider: string; model: string; endpoint: string | null; category: string };
      note: string;
    }>("task_set_model_route", {
      sourceProfile: input.sourceProfile,
      provider: input.provider,
      model: input.model,
      reasoningEffort: input.reasoningEffort ?? "default",
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 模型目录（OD-ARCH-048）。
   *
   * web-api 通道直读 DSH `session/modelCatalog`；ACP 通道不支持时返回
   * `available: false` 并带上真实原因，界面回退到 Profile 候选列表。
   *
   * 不读、不转发任何凭证：目录只有 provider/模型标识/思考等级。
   */
  listEngineModels(refresh = false): Promise<EngineModelCatalog> {
    if (!refresh && engineCatalogCache) return Promise.resolve(engineCatalogCache);
    if (!refresh && engineCatalogInflight) return engineCatalogInflight;
    const request = invoke<{
      channel?: string;
      default?: { provider?: string | null; model?: string | null; effort?: string | null };
      routable_providers?: string[];
      groups?: EngineModelCatalog["groups"];
      models?: EngineModelCatalog["models"];
      failures?: Array<Record<string, unknown>>;
    }>("engine_list_models", { refresh })
      .then((value) => ({
        available: true,
        channel: value.channel ?? "web-api",
        default: value.default,
        routable_providers: value.routable_providers,
        groups: value.groups ?? [],
        models: value.models ?? [],
        failures: value.failures ?? [],
      }) as EngineModelCatalog)
      .catch((e) => {
        const reason = normalizeError(e);
        // 通道不支持是「正常的回退」，不是故障：如实标注并让界面用 Profile 候选。
        const unsupported = /unsupported|only available on the web-api/i.test(reason);
        return {
          available: false,
          channel: unsupported ? "acp" : "web-api",
          reason: unsupported
            ? "当前通道不支持直读模型目录（ACP）：已回退到 Profile 候选列表。"
            : reason,
        } as EngineModelCatalog;
      });

    // refresh=true 视为强制重取：结果直接返回，不写共享缓存。
    if (refresh) return request;
    engineCatalogInflight = request.finally(() => {
      engineCatalogInflight = null;
    });
    void engineCatalogInflight.then(
      (value) => {
        // 失败结果不缓存：下次挂载应能重新尝试（例如引擎当时还没起来）。
        if (value.available) engineCatalogCache = value;
      },
      () => undefined,
    );
    return engineCatalogInflight;
  },

  /**
   * 改为「跟随 DSH 设置」（OD-ARCH-050 S5）。
   *
   * 把 OmniDesk 自己的模型覆盖文件归档到旁边（不删除），并停掉当前引擎：
   * 下次任务会按 DSH Profile 里配置的 provider / 默认模型启动。
   */
  clearModelRoute(): Promise<{
    ok: boolean;
    archived: string | null;
    note: string;
  }> {
    return invoke<{ ok: boolean; archived: string | null; note: string }>(
      "task_clear_model_route",
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  // -------------------------------------------------------------------------
  // 原生设置页（OD-ARCH-053）。密钥只在保存时单向传入，永不回读。
  // -------------------------------------------------------------------------

  /** 读取模型配置与非机密凭据状态（已配置 / 来源 / 是否可写）。 */
  modelSettingsLoad(): Promise<ModelSettingsView> {
    return invoke<ModelSettingsView>("model_settings_load").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 保存模型配置；`apiKey` 非空时同时写入 DSH 凭据服务。
   * 宿主返回体只有 `key_written` 与错误原因，不含密钥本身。
   */
  modelSettingsSave(input: {
    settings: ModelSettings;
    /** 密钥要写进哪个凭据引用；留空或不属于本配置时宿主拒绝写密钥。 */
    apiKeyRef?: string | null;
    apiKey?: string | null;
  }): Promise<ModelSettingsSaveReceipt> {
    return invoke<ModelSettingsSaveReceipt>("model_settings_save", {
      settings: input.settings,
      apiKeyRef: input.apiKeyRef ?? null,
      apiKey: input.apiKey ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在系统编辑器里打开场景声明文件。宿主不接受任何路径参数。 */
  /**
   * 测试连接并顺带拉模型列表（OD-ARCH-054 S1/S2）。
   *
   * 由 DSH 发真实请求；`apiKey` 只用于这一次探测，不落盘。失败也返回结构化结果
   * （分类 + 原因），网络/鉴权错误不算异常抛出。
   */
  modelSettingsProbe(input: {
    /** 已保存的提供商名：端点与它一致时 DSH 才会沿用该路由的凭据。 */
    provider?: string | null;
    baseUrl: string;
    api: string;
    apiKey?: string | null;
  }): Promise<ModelProbeReceipt> {
    return invoke<ModelProbeReceipt>("model_settings_probe", {
      provider: input.provider ?? null,
      baseUrl: input.baseUrl,
      api: input.api,
      apiKey: input.apiKey ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  profileDeclOpen(): Promise<ProfileDeclOpenReceipt> {
    return invoke<ProfileDeclOpenReceipt>("profile_decl_open").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存场景声明：宿主先校验，失败拒绝写；通过则备份旧文件后原子写。 */
  profileDeclSave(text: string): Promise<ProfileDeclSaveReceipt> {
    return invoke<ProfileDeclSaveReceipt>("profile_decl_save", { text }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读：取一份起步场景声明模板（OD-ARCH-122）。
   *
   * 只在内存里生成文本，不写盘。用户还要在声明页点「校验并保存」才会落文件。
   * `has_existing` 为真表示当前 Profile 已有声明，保存会覆盖（旧文件先备份）。
   */
  profileDeclStarter(label?: string): Promise<ProfileDeclStarter> {
    return invoke<ProfileDeclStarter>("profile_decl_starter", {
      label: label ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },
  /**
   * 只读：Profile 规则资产（OD-ARCH-068）。
   *
   * 文件不存在时 `loaded=false` 且无 error（纯净底座）；文件损坏时 `error` 有值，
   * 此时提交会被宿主拒绝——界面必须显式提示，不能当作“没有规则”继续跑。
   */
  profileRules(): Promise<ProfileRulesView> {
    return invoke<ProfileRulesView>("profile_rules_load").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存规则资产：宿主先完整校验，失败整次拒绝、旧文件保留；通过则备份并原子写。 */
  profileRulesSave(text: string): Promise<ProfileRulesSaveReceipt> {
    return invoke<ProfileRulesSaveReceipt>("profile_rules_save", { text }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 恢复出厂默认规则包（同样先备份旧文件）。 */
  profileRulesRestoreDefault(): Promise<ProfileRulesSaveReceipt> {
    return invoke<ProfileRulesSaveReceipt>("profile_rules_restore_default").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 保存「场景 → Skill」绑定（OD-ARCH-129）。
   *
   * 只替换规则包里的 skill_bindings，其余规则内容由宿主原样保留；
   * 基础标准会被宿主拒绝，界面据此禁用入口。
   */
  profileSkillBindingsSave(
    bindings: SkillBindingChoice[],
  ): Promise<ProfileRulesSaveReceipt> {
    return invoke<ProfileRulesSaveReceipt>("profile_skill_bindings_save", { bindings }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 打开当前 Profile 自带的 Skill 源目录（OD-ARCH-131）。
   * 首次点才创建目录；基础标准由宿主拒绝。
   */
  profileSkillSourceOpen(): Promise<{
    ok: boolean;
    opened?: string;
    path?: string;
    note: string;
  }> {
    return invoke<{
      ok: boolean;
      opened?: string;
      path?: string;
      note: string;
    }>("profile_skill_source_open").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 按名字生成一份 Profile 自带 Skill 的起步模板（已存在不覆盖）。 */
  profileSkillScaffold(name: string): Promise<{
    ok: boolean;
    created: boolean;
    path: string;
    note: string;
  }> {
    return invoke<{
      ok: boolean;
      created: boolean;
      path: string;
      note: string;
    }>("profile_skill_scaffold", { name }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 能力管理（OD-ARCH-072）：只读读取 Skill 与插件清单。
   *
   * 读的是隔离实例自己的 `~/OmniDesk/runtime/dsh-home/`，不碰原版 DSH 数据根。
   */
  capabilities(): Promise<CapabilityView> {
    return invoke<CapabilityView>("capability_view").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 插件启停：只改 profile 的 bundles，写前宿主自动备份；重启后生效。 */
  capabilitySetPlugin(input: {
    name: string;
    enabled: boolean;
  }): Promise<CapabilityReceipt> {
    return invoke<CapabilityReceipt>("capability_set_plugin", input).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 在系统文件管理器打开 Skill 目录（固定路径，无参数）。 */
  capabilityOpenSkills(): Promise<{ ok: boolean; opened: string; note: string }> {
    return invoke<{ ok: boolean; opened: string; note: string }>(
      "capability_open_skills",
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  runAnalysis(inputText: string): Promise<AnalysisRunResponse> {
    return invoke<AnalysisRunResponse>("task_run_analysis", { inputText }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  explanationPrompt(input: {
    taskType: string;
    inputText: string;
    fingerprint: string;
  }): Promise<{ prompt: string }> {
    return invoke<{ prompt: string }>("task_explanation_prompt", {
      taskType: input.taskType,
      inputText: input.inputText,
      fingerprint: input.fingerprint,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 幂等准备引擎任务的隔离工作目录（本地数据根），并返回其绝对路径。
   * 只创建空目录；不做任何写入，也不改变任何配置。
   */
  prepareWorkspaceDir(): Promise<{
    ok: boolean;
    workspace_dir: string | null;
    error: string | null;
  }> {
    return invoke<{ ok: boolean; workspace_dir: string | null; error: string | null }>(
      "task_prepare_workspace_dir",
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  saveArtifact(input: {
    taskType: string;
    inputText: string;
    explanationText: string;
    explanationSource: ExplanationSource;
    userConfirmed: boolean;
    confirmationNote: string;
    fingerprint: string;
  }): Promise<SaveArtifactReceipt> {
    return invoke<SaveArtifactReceipt>("task_save_artifact", {
      request: {
        task_type: input.taskType,
        input_text: input.inputText,
        explanation_text: input.explanationText,
        explanation_source: input.explanationSource,
        user_confirmed: input.userConfirmed,
        confirmation_note: input.confirmationNote,
        fingerprint: input.fingerprint,
      },
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  recordUsage(input: {
    taskType: string;
    completed: boolean;
    usable: boolean;
    reworkKind: string;
    blocker: string;
    durationBucket: string | null;
    reuseWillingness: boolean;
  }): Promise<UsageRecord> {
    return invoke<UsageRecord>("task_usage_record", {
      input: {
        task_type: input.taskType,
        completed: input.completed,
        usable: input.usable,
        rework_kind: input.reworkKind,
        blocker: input.blocker,
        duration_bucket: input.durationBucket,
        reuse_willingness: input.reuseWillingness,
      },
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  usageView(): Promise<UsageReadResult> {
    return invoke<UsageReadResult>("task_usage_view").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  usageDelete(recordId: string): Promise<{ deleted: number }> {
    return invoke<{ deleted: number }>("task_usage_delete", { recordId }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  usageClear(): Promise<{ cleared: boolean }> {
    return invoke<{ cleared: boolean }>("task_usage_clear").catch((e) => {
      throw new Error(normalizeError(e));
      });
  },

  /** 编译期嵌入的纯虚拟流程样例（周记录、项目跟进）。 */
  workflowSamples(): Promise<{ samples: WorkflowSample[] }> {
    return invoke<{ samples: WorkflowSample[] }>("workflow_samples").catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 读取仓库外流程包；失败时整次拒绝，界面不得退回看似成功的默认文案。 */
  offlineFlowPack(): Promise<OfflineFlowPackResponse> {
    return invoke<OfflineFlowPackResponse>("offline_flow_pack").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  weeklySummary(inputText: string): Promise<OfflineFlowResponse<WeeklySummary>> {
    return invoke<OfflineFlowResponse<WeeklySummary>>("task_weekly_summary", {
      inputText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  writingPlan(input: {
    purpose: string;
    audience: string;
    requirements: string;
    facts: string;
  }): Promise<OfflineFlowResponse<never>> {
    return invoke<OfflineFlowResponse<never>>("task_writing_plan", {
      purpose: input.purpose,
      audience: input.audience,
      requirements: input.requirements,
      facts: input.facts,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  projectSummary(
    inputText: string,
    today: string | null,
  ): Promise<OfflineFlowResponse<ProjectSummary>> {
    return invoke<OfflineFlowResponse<ProjectSummary>>("task_project_summary", {
      inputText,
      today,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 项目事项清单（OD-ARCH-102 S01）：应用数据根下的 project-items.v1.json。
   * 只读写这一个文件；`today` 不参与存储，逾期在界面按当天现算。
   */
  projectItemsRead(): Promise<ProjectItemsListing> {
    return invoke<ProjectItemsListing>("project_items_read").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  projectItemsSave(items: ProjectItemRecord[]): Promise<ProjectItemsSaveReceipt> {
    return invoke<ProjectItemsSaveReceipt>("project_items_save", { items }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 新建项目（OD-KM-508）：名称与目标过宿主闸门后才落盘。 */
  projectCreate(draft: ProjectDraftInput): Promise<ProjectInfo> {
    return invoke<ProjectInfo>("project_create", { draft }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  projectUpdate(id: string, patch: ProjectPatchInput): Promise<ProjectInfo> {
    return invoke<ProjectInfo>("project_update", { id, patch }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  projectList(): Promise<ProjectListing> {
    return invoke<ProjectListing>("project_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 项目概览：任务/记录/素材/知识全部反向派生，素材计数来自素材篮只读查询。 */
  projectOverview(projectId: string): Promise<ProjectOverview> {
    return invoke<ProjectOverview>("project_overview", { projectId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  // -------------------------------------------------------------------------
  // 写作素材（OD-KM-506）。AI 只经既有分析链路暂存候选，审核状态只由用户动作推进。
  // -------------------------------------------------------------------------

  /**
   * AI 素材提取准备：复用 worklog_record_analyze 的门禁与提示词（已注入既有素材清单）。
   * 模型调用仍由界面提交；结果经 workRecordAnalyzeApply 落暂存候选，不写正式素材。
   */
  materialExtractFromRecord(recordId: string): Promise<AnalyzePrompt> {
    return invoke<AnalyzePrompt>("material_extract_from_record", { recordId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户主动收藏；标题、正文、来源引用与标签过宿主闸门后才落盘。 */
  materialCreate(draft: MaterialDraftInput): Promise<WritingMaterial> {
    return invoke<WritingMaterial>("material_create", { draft }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  materialList(filter: MaterialFilter = {}): Promise<MaterialListing> {
    return invoke<MaterialListing>("material_list", { filter }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 素材详情：带来源追溯（源记录/任务/知识定位与引用原文）。 */
  materialGet(id: string): Promise<MaterialDetail> {
    return invoke<MaterialDetail>("material_get", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 审核：confirm / return / reject / defer / archive；退回与拒绝必须给理由。 */
  materialReview(
    id: string,
    action: MaterialReviewAction,
    reason?: string | null,
  ): Promise<WritingMaterial> {
    return invoke<WritingMaterial>("material_review", { id, action, reason }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 编辑正文/类型/标签等；用户文本过闸门，并追加一条审核记录。 */
  materialEdit(id: string, patch: MaterialPatchInput): Promise<WritingMaterial> {
    return invoke<WritingMaterial>("material_edit", { id, patch }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 第四阶段素材查询（OD-KM-509）：默认只返回已确认素材。
   * 关键词只在宿主本地匹配标题与正文，不落盘、不外发。
   */
  materialQuery(query: MaterialQueryInput = {}): Promise<MaterialQueryResponse> {
    return invoke<MaterialQueryResponse>("material_query", { query }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 智能素材收集（OD-WT-605）：默认只返回已确认素材；待审核需显式打开。
   * 查询条件不落盘、不外发；写作任务编号在宿主侧过闸门。
   */
  writingCollectMaterials(input: WritingCollectInput): Promise<WritingCollectResponse> {
    return invoke<WritingCollectResponse>("writing_collect_materials", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 用户采纳/排除/标注重点：写回素材包，状态记录只追加。 */
  writingSelectMaterials(input: WritingSelectInput): Promise<WritingSelectResult> {
    return invoke<WritingSelectResult>("writing_select_materials", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 写作需求解析准备（OD-WT-604）：宿主过门禁、红线与掩码后返回提示词。
   * 模型调用由界面侧提交，与工作记录解析同一条既有通道。
   */
  writingRequirementPrepare(text: string): Promise<WritingRequirementPrompt> {
    return invoke<WritingRequirementPrompt>("writing_parse_requirement", { text }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 解析落地：校验模型输出并返回草稿；不落盘，等待用户确认补全。 */
  writingRequirementApply(resultText: string): Promise<WritingRequirementApplied> {
    return invoke<WritingRequirementApplied>("writing_parse_requirement_apply", {
      resultText,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户确认后创建写作任务，初始状态＝需求待整理。 */
  writingCreateFromRequirement(
    draft: WritingRequirementDraft,
  ): Promise<WritingTaskCreated> {
    return invoke<WritingTaskCreated>("writing_create_from_requirement", { draft }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 写作任务列表（OD-WT-610 只读）：排序与状态/类型白名单由 601 宿主校验。 */
  writingList(
    filter: {
      status?: string;
      document_type?: string;
      date_from?: string;
      date_to?: string;
    } = {},
  ): Promise<{ items: WritingTask[]; broken: number }> {
    return invoke<{ items: WritingTask[]; broken: number }>("writing_list", {
      filter,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 大纲生成准备（OD-WT-606）：宿主校验任务状态与素材包后返回提示词。
   * 模型调用仍由界面侧提交；本命令不写文件。
   */
  writingOutlineGenerate(input: WritingOutlineGenerateInput): Promise<WritingOutlinePrompt> {
    return invoke<WritingOutlinePrompt>("writing_outline_generate", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 确认可编辑大纲：写 outline.md + 版本快照，并迁移到写作中。 */
  writingOutlineConfirm(input: WritingOutlineConfirmInput): Promise<WritingOutlineConfirmed> {
    return invoke<WritingOutlineConfirmed>("writing_outline_confirm", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 读取已确认大纲；未确认时返回明确错误。 */
  writingOutlineGet(writingId: string): Promise<WritingOutlineConfirmed> {
    return invoke<WritingOutlineConfirmed>("writing_outline_get", { writingId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 章节生成准备（OD-WT-607）：宿主校验任务状态、已确认大纲与本章素材后返回
   * 提示词。模型调用仍由界面侧提交；本命令不写文件。
   */
  writingSectionGenerate(input: WritingSectionGenerateInput): Promise<WritingSectionPrompt> {
    return invoke<WritingSectionPrompt>("writing_section_generate", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 用户确认章节稿：写正文.md + 完整正文快照，并追加本章引用。 */
  writingSectionConfirm(input: WritingSectionConfirmInput): Promise<WritingSectionConfirmed> {
    return invoke<WritingSectionConfirmed>("writing_section_confirm", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 读取当前正文中的指定章节；未确认时 content 为 null。 */
  writingSectionGet(writingId: string, sectionNo: number): Promise<WritingSectionState> {
    return invoke<WritingSectionState>("writing_section_get", { writingId, sectionNo }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 构建章节修订提示词；应用修订仍走 writingSectionConfirm。 */
  writingSectionRevise(input: WritingSectionReviseInput): Promise<WritingSectionRevisePrompt> {
    return invoke<WritingSectionRevisePrompt>("writing_section_revise", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 运行事实核验（OD-WT-609）：确定性引擎读正文与已选素材/知识依据，
   * 结论经 603 追加到 factcheck.jsonl；返回完整清单、本次新追加数与统计。
   * annotations 为章节生成侧四分法标注，由界面把最近稿件标注随车传入。
   */
  writingFactcheckRun(input: FactCheckRunInput): Promise<FactCheckRunResult> {
    return invoke<FactCheckRunResult>("writing_factcheck_run", { request: input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 用户处理单条核验结论：已接受/已修改/已删除来源项，走 603 按行重写。 */
  writingFactcheckResolve(input: FactCheckResolveInput): Promise<FactCheckResolveResult> {
    return invoke<FactCheckResolveResult>("writing_factcheck_resolve", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 十维审阅准备（OD-WT-608）：宿主校验任务状态（写作中/待核验/待审核）后
   * 返回提示词。AI 只产出问题清单，不改写正文；模型调用仍由界面侧提交。
   */
  writingReview(input: WritingReviewInput): Promise<WritingReviewPrompt> {
    return invoke<WritingReviewPrompt>("writing_review", { request: input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 用户应用某条审阅问题：修订稿走 607 同一确认路径写正文与快照，
   * 并在写作任务状态记录追加「应用审阅问题 …」留痕。
   */
  writingReviewApplyFinding(
    input: WritingReviewApplyInput,
  ): Promise<WritingReviewApplied> {
    return invoke<WritingReviewApplied>("writing_review_apply_finding", {
      request: input,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 版本列表：manifest 每版说明 note + 相对上一同类型版本的段落差异摘要。 */
  writingVersionList(writingId: string): Promise<WritingVersionList> {
    return invoke<WritingVersionList>("writing_version_list", { writingId }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 两份历史版本的段落级对比；不同文稿类型（大纲/正文）会被拒绝。 */
  writingVersionDiff(input: WritingVersionDiffInput): Promise<WritingVersionDiff> {
    return invoke<WritingVersionDiff>("writing_version_diff", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /** 恢复历史版本（须显式确认）：新增快照，历史保留，状态不变并追加「恢复 vX」。 */
  writingVersionRestore(
    input: WritingVersionRestoreInput,
  ): Promise<WritingVersionRestored> {
    return invoke<WritingVersionRestored>("writing_version_restore", {
      request: input,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 用户触发的写作状态迁移（OD-WT-613）：合法迁移表由 601 宿主校验；
   * 待审核→已定稿先写 label=final 的正文快照。AI 无调用通道。
   */
  writingStatusTransition(
    input: WritingStatusTransitionInput,
  ): Promise<WritingTask> {
    return invoke<WritingTask>("writing_status_transition", { request: input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** citations / factcheck 只读查询（OD-WT-613）：按类型、章节、段落与结论收窄。 */
  writingRecordsQuery(input: WritingRecordsQueryInput): Promise<WritingRecordsQueryResult> {
    return invoke<WritingRecordsQueryResult>("writing_records_query", { request: input }).catch(
      (e) => {
        throw new Error(normalizeError(e));
      },
    );
  },

  /**
   * 文稿导出（OD-WT-613）：MD 三态 / 公文 DOCX / 阅读 PDF / 公众号 HTML。
   * 导出前宿主做敏感词扫描，产物只写本地目标目录并追加 manifest 回执。
   */
  writingExport(input: WritingExportInput): Promise<WritingExportOutcome> {
    return invoke<WritingExportOutcome>("writing_export", { request: input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 文稿归档（OD-WT-614）：preview 只读清单；执行时宿主校验已定稿、敏感扫描、
   * 写知识库「报告/」、追加 manifest archive 回执并推进到已归档。AI 无调用通道。
   */
  writingArchive(input: WritingArchiveInput): Promise<WritingArchiveOutcome> {
    return invoke<WritingArchiveOutcome>("writing_archive", { request: input }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 知识回流（OD-WT-614）：只提交用户勾选内容，进既有 raw 收件待审队列；
   * 不自动生成知识条目，也不自动定稿。
   */
  writingKnowledgeFeedback(
    input: WritingKnowledgeFeedbackInput,
  ): Promise<WritingKnowledgeFeedbackOutcome> {
    return invoke<WritingKnowledgeFeedbackOutcome>("writing_knowledge_feedback", {
      request: input,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 保存离线成果：必须人工确认，不覆盖已有文件，落仓库外本地数据目录。 */
  saveOffline(input: {
    scenario: string;
    title: string;
    markdown: string;
    userConfirmed: boolean;
    /** 成果是否包含模型初稿；只有实际取得模型文本时才可为 true。 */
    aiAssisted?: boolean;
    /**
     * 写动作来源（OD-ARCH-046）。缺省 human；模型 / 定时来源必须同时给出
     * 宿主发出的待确认编号，否则宿主直接拒绝。界面上的“确认”按钮应调用
     * pendingConfirm，而不是带上 userConfirmed 直接写。
     */
    source?: "human" | "model" | "scheduler";
    pendingId?: string | null;
  }): Promise<SaveArtifactReceipt> {
    return invoke<SaveArtifactReceipt>("task_save_offline", {
      request: {
        scenario: input.scenario,
        title: input.title,
        markdown: input.markdown,
        user_confirmed: input.userConfirmed,
        ai_assisted: input.aiAssisted ?? false,
        source: input.source ?? "human",
        pending_id: input.pendingId ?? null,
      },
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 已保存的离线成果（只读元数据，按时间倒序）。 */
  offlineArtifacts(includeSubdirs = false): Promise<{ items: OfflineArtifactEntry[] }> {
    return invoke<{ items: OfflineArtifactEntry[] }>(
      "task_list_offline_artifacts",
      { includeSubdirs },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 场景状态汇总（OD-ARCH-060 S1/S5）：待确认条数、本周/本月成果计数与本周截止日。
   * 全部由宿主从已落盘的元数据数出来，不读正文、不调模型。
   */
  sceneOverview(scenarios?: string[]): Promise<SceneOverviewResponse> {
    return invoke<SceneOverviewResponse>("scene_overview", {
      scenarios: scenarios ?? null,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 重新打开一条已保存成果；只读，不改写原文件。 */
  readOfflineArtifact(fileName: string): Promise<OfflineArtifactContent> {
    return invoke<OfflineArtifactContent>("task_read_offline_artifact", {
      fileName,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 只读读取用户**明确给出绝对路径**的一个材料文件（OD-ARCH-026）。
   * 一次一个文件，不列目录、不递归；白名单扩展名、大小上限与 UTF-8 由宿主强制。
   */
  readMaterial(path: string): Promise<MaterialRead> {
    return invoke<MaterialRead>("task_read_material", { path }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 可引用来源根（成果目录 + 知识库）；由宿主解析，前端不指定根（OD-ARCH-077）。 */
  referenceRoots(): Promise<{ roots: ReferenceRoot[] }> {
    return invoke<{ roots: ReferenceRoot[] }>("task_reference_roots").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 列出可引用文件（有界遍历，只列宿主白名单内的文本类型）。 */
  listReferences(
    query?: string,
    limit?: number,
  ): Promise<{ items: ReferenceCandidate[]; limit: number; roots: string[] }> {
    return invoke<{ items: ReferenceCandidate[]; limit: number; roots: string[] }>(
      "task_list_references",
      { query: query ?? null, limit: limit ?? null },
    ).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 读取一个允许根内的附件正文；越界路径由宿主拒绝，不在此处放宽。 */
  readReference(path: string): Promise<MaterialRead> {
    return invoke<MaterialRead>("task_read_reference", { path }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /**
   * 读取用户在系统文件框里显式挑出的附件（OD-ARCH-078 S1）。
   * 来源不限数据根（桌面等也可以）；禁区、仓库内、大小与「是否真文本」仍由宿主判定。
   */
  readAttachment(path: string): Promise<MaterialRead> {
    return invoke<MaterialRead>("task_read_attachment", { path }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },
};

/**
 * 待确认写操作（OD-ARCH-046）。
 *
 * 模型 / 定时任务只能“登记”一条待确认记录，不能直接写成果文件。用户在确认卡片上
 * 点“确认”后，由宿主重新从磁盘校验该记录（存在、未过期、未消费、正文一致）才落盘。
 */
export const pendingWrites = {
  create(input: {
    action: "save_offline";
    source: "model" | "scheduler";
    summary: string;
    impact: string;
    payload: {
      scenario: string;
      title: string;
      markdown: string;
      ai_assisted?: boolean;
    };
  }): Promise<PendingWriteView> {
    return invoke<PendingWriteView>("pending_write_create", {
      request: {
        action: input.action,
        source: input.source,
        summary: input.summary,
        impact: input.impact,
        payload: {
          scenario: input.payload.scenario,
          title: input.payload.title,
          markdown: input.payload.markdown,
          ai_assisted: input.payload.ai_assisted ?? false,
        },
      },
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  list(): Promise<PendingWriteListing> {
    return invoke<PendingWriteListing>("pending_write_list").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  confirm(id: string): Promise<PendingWriteConfirmReceipt> {
    return invoke<PendingWriteConfirmReceipt>("pending_write_confirm", { id }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  reject(id: string): Promise<{ ok: boolean; id: string; status: string }> {
    return invoke<{ ok: boolean; id: string; status: string }>("pending_write_reject", {
      id,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },
};

/**
 * 应用运行期调度（OD-ARCH-046）。
 *
 * 宿主只保存配置并判定是否到点；触发由前端运行期定时器负责，不做常驻进程。
 */
export const scheduler = {
  getConfig(): Promise<SchedulerSnapshot> {
    return invoke<SchedulerSnapshot>("scheduler_config_get").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  setConfig(config: SchedulerConfig): Promise<SchedulerSnapshot> {
    return invoke<SchedulerSnapshot>("scheduler_config_set", { config }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  tick(): Promise<SchedulerTick> {
    return invoke<SchedulerTick>("scheduler_tick").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  markReviewed(): Promise<SchedulerSnapshot> {
    return invoke<SchedulerSnapshot>("scheduler_mark_reviewed").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  markHeartbeat(): Promise<SchedulerSnapshot> {
    return invoke<SchedulerSnapshot>("scheduler_mark_heartbeat").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只做本地确定性扫描，不调模型、不写文件。 */
  heartbeatScan(inputText: string, today: string | null = null): Promise<HeartbeatScan> {
    return invoke<HeartbeatScan>("scheduler_heartbeat_scan", {
      inputText,
      today,
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },
};

/** 订阅宿主转发上来的任务事件流。 */
export function onTaskEvent(
  handler: (event: TaskEvent) => void,
): Promise<UnlistenFn> {
  return listen<TaskEvent>("omni://task-event", (e) => handler(e.payload));
}

/**
 * 订阅后台自动补装插件的结束事件（OD-ARCH-121）。
 *
 * 切换命令只负责「已在后台开始」，装完（或失败）才从这条事件回报；
 * 前端据此清掉缺件卡或刷新为「仍缺哪些」。
 */
export function onProfilePlugins(
  handler: (event: ProfilePluginsAutofillEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProfilePluginsAutofillEvent>("omni://profile-plugins", (e) =>
    handler(e.payload),
  );
}

/**
 * 订阅「设置页保存了场景声明」（OD-ARCH-123）。
 *
 * 只在**应用内保存**时触发；外部手改 decl.yml 仍走指纹提示、不热重载。
 * 界面收到后必须重读声明，否则用户刚保存完 Dock 还是旧入口。
 */
export function onProfileDeclSaved(
  handler: (event: ProfileDeclSavedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProfileDeclSavedEvent>("omni://profile-decl", (e) =>
    handler(e.payload),
  );
}

/**
 * 订阅「自定义 Profile 清单变了」（OD-ARCH-125）。
 *
 * 新建、移除、从回收站恢复都会触发。收到后重读一次清单即可；事件本身不带数据。
 */
export function onProfileRegistryChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("omni://profile-registry", () => handler());
}

/**
 * 用系统默认浏览器打开正文里的外部链接（OD-ARCH-081）。
 * 宿主只放行 http / https；其余 scheme 直接返回错误，不会触达系统。
 */
export function openExternalUrl(url: string): Promise<{ ok: boolean; opened: string }> {
  return invoke<{ ok: boolean; opened: string }>("open_external_url", { url }).catch((e) => {
    throw new Error(normalizeError(e));
  });
}

