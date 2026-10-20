// IPC-1 客户端：UI 只管调用 Rust 宿主暴露的 Tauri 命令 / 事件。
//
// UI 不接触 IPC-2，不接触 ACP，不接触 DSH。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { currentEngineSubmitDecision } from "./engineAdmission";
import type {
  AdmissionStatusResponse,
  AnalysisRunResponse,
  CancelOutcome,
  EngineDescriptor,
  EngineHealth,
  ExplanationSource,
  LocalRootInfo,
  MaterialRead,
  OfflineFlowResponse,
  ProjectSummary,
  SampleCatalog,
  SaveArtifactReceipt,
  SessionListing,
  SessionRecovery,
  SubmitTaskReceipt,
  TaskEvent,
  TaskLedgerListing,
  TaskRecovery,
  TaskResult,
  TaskStatus,
  TaskTypeCatalog,
  OfflineArtifactContent,
  OfflineArtifactEntry,
  OfflineFlowPackResponse,
  PermissionChangeResponse,
  UsageReadResult,
  UsageRecord,
  WeeklySummary,
  WorkflowSample,
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

export const host = {
  describe(): Promise<EngineDescriptor> {
    return invoke<EngineDescriptor>("engine_describe").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  health(): Promise<EngineHealth> {
    return invoke<EngineHealth>("engine_health").catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 只读：当前端点类别与用户授权的可核实事实。准入关闭时也可调用。 */
  admissionStatus(): Promise<AdmissionStatusResponse> {
    return invoke<AdmissionStatusResponse>("engine_admission_status").catch((e) => {
      throw new Error(normalizeError(e));
    });
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
   */
  listModelRoutes(): Promise<{
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
  }> {
    return invoke<{
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
    }>("task_list_model_routes").catch((e) => {
      throw new Error(normalizeError(e));
    });
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
   * 拉起已安装的 DSH Desktop（模型/凭据设置界面）。
   *
   * 固定动作、无参数：宿主只执行固定的 `gtk-launch` / `xdg-open`，不接受命令或路径；
   * 未安装时明确报错，不静默失败。宿主不读、不写、不打印任何凭证值。
   * 宿主会把 DSH 指向**引擎正在用的那份 DSH 数据根**，使两边共享同一份凭据与 Profile。
   */
  openDshSettings(): Promise<{
    ok: boolean;
    launched: string;
    pid: number;
    dsh_home: string;
    agents_home: string;
    credentials_file: string;
    log_path: string;
    note: string;
  }> {
    return invoke<{
      ok: boolean;
      launched: string;
      pid: number;
      dsh_home: string;
      agents_home: string;
      credentials_file: string;
      log_path: string;
      note: string;
    }>(
      "task_open_dsh_settings",
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

  /** 保存离线成果：必须人工确认，不覆盖已有文件，落仓库外本地数据目录。 */
  saveOffline(input: {
    scenario: string;
    title: string;
    markdown: string;
    userConfirmed: boolean;
    /** 成果是否包含模型初稿；只有实际取得模型文本时才可为 true。 */
    aiAssisted?: boolean;
  }): Promise<SaveArtifactReceipt> {
    return invoke<SaveArtifactReceipt>("task_save_offline", {
      request: {
        scenario: input.scenario,
        title: input.title,
        markdown: input.markdown,
        user_confirmed: input.userConfirmed,
        ai_assisted: input.aiAssisted ?? false,
      },
    }).catch((e) => {
      throw new Error(normalizeError(e));
    });
  },

  /** 已保存的离线成果（只读元数据，按时间倒序）。 */
  offlineArtifacts(): Promise<{ items: OfflineArtifactEntry[] }> {
    return invoke<{ items: OfflineArtifactEntry[] }>(
      "task_list_offline_artifacts",
    ).catch((e) => {
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
};

/** 订阅宿主转发上来的任务事件流。 */
export function onTaskEvent(
  handler: (event: TaskEvent) => void,
): Promise<UnlistenFn> {
  return listen<TaskEvent>("omni://task-event", (e) => handler(e.payload));
}
