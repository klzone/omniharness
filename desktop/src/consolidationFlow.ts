/**
 * 每日整理运行的纯状态机（OD-GROW-301 G4）。
 *
 * 运行器只负责问宿主到点没有、调用整理命令与提交模型；是否进入下一步、
 * 何时让位、何时停止，全部由这里的确定性规则判定。规则不写进提示词。
 */

export type ConsolidationPhase =
  | "idle"
  | "prepare"
  | "submit"
  | "await"
  | "ingest"
  | "done"
  | "failed";

export type ConsolidationStep = ConsolidationPhase;

export interface ConsolidationState {
  phase: ConsolidationPhase;
  /** 宿主 consolidation_tick 的到点判定。 */
  tickDue: boolean;
  /** 本次摘要是空文本；空摘要记完成，不调模型。 */
  digestEmpty: boolean;
  /** 宿主拒绝了本次提交。失败不自动重试。 */
  submitRejected: boolean;
  /** 任务已到终态（不管成功或失败）。 */
  taskDone: boolean;
  taskSucceeded: boolean;
  /** 等待超过 1800 秒；超时按失败处理。 */
  timedOut: boolean;
  /** 最近一次整理尝试的本地日期（YYYY-MM-DD）。 */
  attemptDate: string | null;
  /** 工作台里是否有在途对话；有就让位。 */
  runningChat: boolean;
  /** 今天的本地日期；用于每日最多一次的纯函数判定。 */
  today: string;
}

export function emptyConsolidationState(today: string): ConsolidationState {
  return {
    phase: "idle",
    tickDue: false,
    digestEmpty: false,
    submitRejected: false,
    taskDone: false,
    taskSucceeded: false,
    timedOut: false,
    attemptDate: null,
    runningChat: false,
    today,
  };
}

/**
 * 下一步判定。
 *
 * 每日最多一次是最高优先的闸门：今天已经尝试过（attemptDate === today）时，
 * 不管从哪个相位被重复触发，都回到 idle，不产生第二次提交。
 */
export function nextStep(state: ConsolidationState): ConsolidationStep {
  if (state.attemptDate !== null && state.attemptDate === state.today) {
    return "idle";
  }

  switch (state.phase) {
    case "idle":
      return state.tickDue && !state.runningChat ? "prepare" : "idle";
    case "prepare":
      return state.digestEmpty ? "done" : "submit";
    case "submit":
      return state.submitRejected ? "failed" : "await";
    case "await":
      if (state.timedOut) return "failed";
      if (!state.taskDone) return "await";
      return state.taskSucceeded ? "ingest" : "failed";
    case "ingest":
      return "done";
    case "done":
      return "done";
    case "failed":
      return "failed";
  }
}

/**
 * 按 UTC 偏移把 Unix 秒换算成整理配置使用的本地日期。
 * 宿主摘要文件按同一口径命名为 digest-<date>.json。
 */
export function consolidationLocalDate(
  nowSeconds: number,
  utcOffsetMinutes: number,
): string {
  if (!Number.isFinite(nowSeconds) || !Number.isFinite(utcOffsetMinutes)) return "";
  const shifted = new Date((Math.floor(nowSeconds) + utcOffsetMinutes * 60) * 1000);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${shifted.getUTCFullYear()}-${pad(shifted.getUTCMonth() + 1)}-${pad(
    shifted.getUTCDate(),
  )}`;
}

export const CONSOLIDATION_LAST_RUN_KEY = "omnidesk.consolidation.lastRun";
export const CONSOLIDATION_LAST_OUTPUT_KEY = "omnidesk.consolidation.lastOutput";
export const CONSOLIDATION_NOTICE_EVENT = "omnidesk:consolidation-notice";

/** G5 接铃铛时消费的运行通知；detail 不含模型正文。 */
export interface ConsolidationNotice {
  kind: "failure" | "done" | "empty";
  message: string;
  at: number;
  threads?: number;
  messages?: number;
  chars?: number;
  outputChars?: number;
}
