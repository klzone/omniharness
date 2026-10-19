/**
 * 引擎提交准入闸门 —— 前端唯一事实源（OD-ARCH-005）。
 *
 * 背景：IPC-1 的 `engine_health` 只报告进程与路径，不报告「模型端点是否已被
 * 核实为仅合成/已批准的隔离端点」。在宿主能提供该事实之前，任何把用户文本
 * 送进引擎的 UI 入口都必须关闭。
 *
 * 约束（照实说明其强度）：
 *   * 这是**前端代码级闸门**：所有经过 `api.ts` 的提交都会在这里被拦下，
 *     覆盖界面点击、脚本注入 DOM、以及未来新增的 UI 调用点。
 *   * 它**不是**端点隔离证明，也不是对改造版构建的安全边界；真正的强制
 *     控制只能来自宿主（Rust）与端点侧。本条不得在报告里写成「已实现隔离」。
 */

export const ENDPOINT_UNCONFIRMED_REASON =
  "当前入口未获端点隔离确认，已拒绝提交。输入未发送到任何模型。";

export type EngineSubmitRefusalCode = "endpoint_unconfirmed" | "empty_text";

export type EngineSubmitDecision =
  | { allowed: true }
  | { allowed: false; code: EngineSubmitRefusalCode; reason: string };

/**
 * 纯函数：把「是否已确认端点隔离」作为显式参数，便于用真实运行验证两条分支，
 * 而不需要在测试中改动全局常量。
 */
export function evaluateEngineSubmit(
  text: string,
  endpointIsolationConfirmed: boolean,
): EngineSubmitDecision {
  if (!endpointIsolationConfirmed) {
    return {
      allowed: false,
      code: "endpoint_unconfirmed",
      reason: ENDPOINT_UNCONFIRMED_REASON,
    };
  }
  if (text.trim().length === 0) {
    return { allowed: false, code: "empty_text", reason: "请先输入任务内容。" };
  }
  return { allowed: true };
}

/**
 * 当前构建的前端预检（OD-ARCH-021）：只拦空文本，不做授权判断。
 *
 * 端点类别与用户授权由宿主按实际配置判定，前端没有可信事实源，也不再复制一份
 * 会与宿主不一致的开关。提交仍以宿主返回的 `endpoint_unconfirmed` /
 * `endpoint_unknown` 为准。
 */
export function currentEngineSubmitDecision(text: string): EngineSubmitDecision {
  return evaluateEngineSubmit(text, true);
}
