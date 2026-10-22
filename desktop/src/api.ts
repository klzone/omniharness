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
  ProfileDeclPayload,
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
  ModelRouteListing,
  EngineModelCatalog,
  ModelSettings,
  ModelSettingsSaveReceipt,
  ModelProbeReceipt,
  ModelSettingsView,
  ProfileDeclOpenReceipt,
  ProfileDeclSaveReceipt,
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

  /**
   * 只读：纪检 Profile 声明（OD-ARCH-051 S3）。
   *
   * 决定 Dock 上显示哪些业务场景入口。宿主负责首次创建默认文件、解析与校验；
   * 文件坏掉时这里会 reject，调用方必须显式提示并回退内置入口，不能静默吞掉。
   */
  profileDecl(): Promise<ProfileDeclPayload> {
    return invoke<ProfileDeclPayload>("profile_decl_load").catch((e) => {
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
   * 打开 DSH 自带的设置界面（模型提供方 / 默认模型 / 凭据登记）。
   *
   * OD-ARCH-050：宿主用临时端口起一个专用 `dsh web`（`--no-open`），再在应用内
   * 打开独立窗口「DSH 设置」。URL 里的会话令牌只留在宿主内存，不经过前端。
   */
  openDshSettings(): Promise<{
    ok: boolean;
    port: number;
    reused: boolean;
    log_path: string;
    note: string;
  }> {
    return invoke<{
      ok: boolean;
      port: number;
      reused: boolean;
      log_path: string;
      note: string;
    }>("task_open_dsh_settings").catch((e) => {
      throw new Error(normalizeError(e));
    });
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
    apiKey?: string | null;
  }): Promise<ModelSettingsSaveReceipt> {
    return invoke<ModelSettingsSaveReceipt>("model_settings_save", {
      settings: input.settings,
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
    baseUrl: string;
    api: string;
    apiKey?: string | null;
  }): Promise<ModelProbeReceipt> {
    return invoke<ModelProbeReceipt>("model_settings_probe", {
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
 * 订阅「DSH 设置窗口已关闭」（OD-ARCH-050）。
 *
 * 用户可能在设置窗口里改了 provider / 默认模型：关闭后需要重新读端点事实与模型目录，
 * 否则界面会停留在旧结果。
 */
export function onDshSettingsClosed(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen("omni://dsh-settings-closed", () => handler());
}
