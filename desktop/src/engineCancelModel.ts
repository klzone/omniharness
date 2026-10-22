// #region engine-cancel-model
//
// 取消入口的纯状态模型。刻意不依赖 React、Tauri 或其它模块，便于用确定性
// 前端测试直接执行本段真实源码（tests/verification/desktop/verify_engine_cancel.py）。
// 这里只决定“按钮能不能点、显示什么”，不判定任务是否真的取消成功。

export interface EngineCancelButtonState {
  enabled: boolean;
  label: string;
  hint: string | null;
}

/** 按宿主任务状态与取消请求的在途状态，给出取消按钮的唯一判定。 */
export function engineCancelButtonState(input: {
  hasTask: boolean;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  /** 本次任务是否已经发出过取消请求（无论宿主是否已回读结果）。 */
  requestSent: boolean;
  /**
   * 已提交但引擎尚未返回任务号（冷启动）。缺省 false，保持原判定不变；
   * 首页对话需要在此期间允许“放弃本次提交”。
   */
  starting?: boolean;
}): EngineCancelButtonState {
  if (!input.hasTask) {
    if (input.starting) {
      return {
        enabled: true,
        label: "引擎启动中…（点此放弃）",
        hint: "放弃后会在拿到任务号时立即请求取消；请求发出不等于引擎已经停止。",
      };
    }
    return { enabled: false, label: "取消引擎任务", hint: null };
  }
  if (input.requestInFlight) {
    return {
      enabled: false,
      label: "正在请求取消…",
      hint: "取消请求已发出，正在等待宿主回读状态；请求发出不等于取消成功。",
    };
  }
  if (!input.status) {
    return {
      enabled: false,
      label: "等待任务状态…",
      hint: "尚未取得宿主状态，暂不能取消。",
    };
  }
  if (
    input.status === "succeeded" ||
    input.status === "failed" ||
    input.status === "cancelled" ||
    input.status === "interrupted" ||
    input.status === "unknown"
  ) {
    return {
      enabled: false,
      label: "任务已结束",
      hint: "任务已进入终态，不能再取消。",
    };
  }
  // 已在取消中：只有拿到明确的 timeout_unknown 才允许重试一次。
  // 请求已发出但宿主尚未给出终态时重复发送，只会制造并发取消请求。
  if (input.status === "cancelling") {
    if (input.lastPhase === "timeout_unknown") {
      return {
        enabled: true,
        label: "重试取消",
        hint: "上次取消在超时内未获终态确认，允许重试一次；仍以宿主终态为准。",
      };
    }
    return {
      enabled: false,
      label: "等待取消结果…",
      hint: "取消请求已发出，正在等待宿主终态；请求发出不等于取消成功。",
    };
  }
  // 宿主状态可能滞后于请求：本任务已经发过取消，就不再允许第二次。
  if (input.requestSent) {
    return {
      enabled: false,
      label: "等待取消结果…",
      hint: "取消请求已发出，尚未取得终态证据；不会重复发送。",
    };
  }
  // queued / starting / running：首次请求前允许取消。
  return {
    enabled: true,
    label: "取消引擎任务",
    hint: "请求取消不等于取消成功；结果以宿主回读的终态为准。",
  };
}

/**
 * 解释一次取消结果。只有 confirmed_cancelled 或宿主最终态为 cancelled
 * 才可显示“取消已确认”；请求已发送、其它终态、超时都按未确认处理。
 */
export function describeEngineCancelOutcome(outcome: {
  phase: string;
  final_state: string | null;
  note: string;
}): { confirmed: boolean; message: string } {
  if (outcome.phase === "confirmed_cancelled" || outcome.final_state === "cancelled") {
    return {
      confirmed: true,
      message: "取消已确认：宿主报告任务已取消。",
    };
  }
  const stateText = outcome.final_state ? "，宿主最终态为 " + outcome.final_state : "";
  return {
    confirmed: false,
    message:
      "取消未确认（阶段：" +
      outcome.phase +
      stateText +
      "）。" +
      (outcome.note ? outcome.note + "。" : ""),
  };
}

/**
 * 取消入口的唯一放行判定。组件在调用宿主前执行本函数，测试则用宿主替身
 * 直接驱动它并统计真实 IPC 次数。按钮可见性不能代替本判定：即使状态模型
 * 认为可点，这里也必须独立拒绝重复请求。
 */
export function engineCancelGate(input: {
  hasTask: boolean;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  requestSent: boolean;
}): { send: boolean; reason: string } {
  if (!input.hasTask) return { send: false, reason: "no_task" };
  if (input.requestInFlight) return { send: false, reason: "request_in_flight" };
  if (!input.status) return { send: false, reason: "no_status" };
  if (
    input.status === "succeeded" ||
    input.status === "failed" ||
    input.status === "cancelled" ||
    input.status === "interrupted" ||
    input.status === "unknown"
  ) {
    return { send: false, reason: "terminal" };
  }
  if (input.status === "cancelling") {
    return input.lastPhase === "timeout_unknown"
      ? { send: true, reason: "retry_after_timeout" }
      : { send: false, reason: "awaiting_cancel_result" };
  }
  if (input.requestSent) return { send: false, reason: "already_requested" };
  return { send: true, reason: "first_request" };
}

/** 取消请求的最小宿主接口；测试用计数替身实现同一契约。 */
export interface EngineCancelHostLike {
  cancelTask(taskId: string): Promise<any>;
  taskStatus(taskId: string): Promise<any>;
}

export interface EngineCancelSnapshot {
  taskId: string | null;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  requestSent: boolean;
}

export interface EngineCancelEffects {
  host: EngineCancelHostLike;
  onRequestStarted: () => void;
  onOutcome: (outcome: any) => void;
  onStatus: (status: any) => void;
  onError: (message: string) => void;
  onSettled: () => void;
}

/**
 * 组件与测试共用的取消发送路径。返回是否真的发出了 IPC：
 * `skipped` 表示被放行判定拦下，调用方不得据此外推任务状态。
 */
export async function runEngineCancelRequest(
  snapshot: EngineCancelSnapshot,
  effects: EngineCancelEffects,
): Promise<"sent" | "skipped"> {
  const taskId = snapshot.taskId;
  // taskId 是否存在决定 hasTask；调用方不再重复传入，避免两处不一致。
  const gateInput = { ...snapshot, hasTask: !!taskId };
  const gate = engineCancelGate(gateInput);
  if (!gate.send || !taskId) return "skipped";
  effects.onRequestStarted();
  try {
    try {
      const outcome = await effects.host.cancelTask(taskId);
      effects.onOutcome(outcome);
    } catch (e) {
      effects.onError(
        "取消请求未获确认：" +
          String(e) +
          "。任务可能仍在执行，解释框保持不变。可稍后重试取消或等待任务终态。",
      );
    }
    // 状态回读是独立一步：回读失败不得把已确认的取消结果改写成“未确认”。
    try {
      effects.onStatus(await effects.host.taskStatus(taskId));
    } catch {
      /* 状态也读不到时保留原状态，不猜测。 */
    }
  } finally {
    effects.onSettled();
  }
  return "sent";
}

// #endregion engine-cancel-model
