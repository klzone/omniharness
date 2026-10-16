// IPC-1 客户端：UI 只管调用 Rust 宿主暴露的 Tauri 命令 / 事件。
//
// UI 不接触 IPC-2，不接触 ACP，不接触 DSH。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalysisRunResponse,
  CancelOutcome,
  EngineDescriptor,
  EngineHealth,
  ExplanationSource,
  LocalRootInfo,
  SampleCatalog,
  SaveArtifactReceipt,
  SubmitTaskReceipt,
  TaskEvent,
  TaskResult,
  TaskStatus,
  TaskTypeCatalog,
  UsageReadResult,
  UsageRecord,
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

  submitTask(input: {
    promptText: string;
    intent?: string;
    workspaceDir?: string;
    idempotencyKey?: string;
  }): Promise<SubmitTaskReceipt> {
    return invoke<SubmitTaskReceipt>("engine_submit_task", {
      promptText: input.promptText,
      intent: input.intent ?? "material_writing",
      workspaceDir: input.workspaceDir ?? null,
      idempotencyKey: input.idempotencyKey ?? null,
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
  }): Promise<UsageRecord> {
    return invoke<UsageRecord>("task_usage_record", {
      input: {
        task_type: input.taskType,
        completed: input.completed,
        usable: input.usable,
        rework_kind: input.reworkKind,
        blocker: input.blocker,
        duration_bucket: input.durationBucket,
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
};

/** 订阅宿主转发上来的任务事件流。 */
export function onTaskEvent(
  handler: (event: TaskEvent) => void,
): Promise<UnlistenFn> {
  return listen<TaskEvent>("omni://task-event", (e) => handler(e.payload));
}
