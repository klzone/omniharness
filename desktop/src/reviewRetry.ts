/** 复盘失败重试的纯状态机（OD-GROW-301 H5）。 */

export type ReviewRetryPhase =
  | "idle"
  | "submitting"
  | "awaiting"
  | "ingesting"
  | "succeeded"
  | "failed";

export interface ReviewRetryState {
  /** 当前尝试序号：首轮为 0，重试轮为 1。 */
  attempt: number;
  phase: ReviewRetryPhase;
  failReason?: string | null;
}

export type ReviewStep =
  | "idle"
  | "submit"
  | "await"
  | "ingest"
  | "retry"
  | "failed";

const NON_RETRYABLE = /取消|cancelled|canceled|endpoint_unconfirmed|endpoint_unknown|red_?line|pending_write_blocked|workspace_unavailable|admission|准入|红线/i;

const RETRYABLE_VALIDATION =
  /复盘正文为空|复盘正文过大|复盘段落重复|复盘缺少段落|复盘段落没有正文|复盘引用了不存在的(?:记录|任务)编号|复盘段落没有标注引用编号|本周期没有记录或任务/;

/**
 * 只有模型未完成、正文校验失败或结果为空可以重试。
 * 用户取消与准入拒绝优先排除，未知错误保守地不重试。
 */
export function isRetryableReviewFailure(failReason: string | null | undefined): boolean {
  const reason = (failReason ?? "").trim();
  if (!reason || NON_RETRYABLE.test(reason)) return false;
  if (/^生成未完成[：:]/.test(reason)) return true;
  if (/模型没有返回复盘正文|结果为空/.test(reason)) return true;
  return RETRYABLE_VALIDATION.test(reason);
}

/** 在原提示词末尾追加一次重试说明。 */
export function reviewRetryPrompt(promptText: string, failReason: string): string {
  const reason = failReason.trim() || "未知原因";
  return (
    promptText.replace(/\s+$/, "") +
    "\n\n上一次尝试失败，原因：" +
    reason +
    "；请修正后重新输出"
  );
}

/** 表驱动的下一步判定；attempt=1 后不再产生 retry。 */
export function nextReviewStep(state: ReviewRetryState): ReviewStep {
  switch (state.phase) {
    case "idle":
      return "submit";
    case "submitting":
    case "awaiting":
      return "await";
    case "ingesting":
    case "succeeded":
      return "ingest";
    case "failed":
      return state.attempt < 1 && isRetryableReviewFailure(state.failReason)
        ? "retry"
        : "failed";
  }
}
