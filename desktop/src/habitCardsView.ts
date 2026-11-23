/**
 * 习惯候选的界面状态（OD-GROW-301 G5）。
 *
 * 这里的函数只负责把「用户在面板上的选择」换成宿主接受的决定，
 * 不读文件、不调命令，便于在 Node 单测里直接验证。
 */

import type { HabitCandidateDecision, HabitCard, HabitScope } from "./types";

export type HabitRejectReason = "too_absolute" | "not_mine";

export const HABIT_REJECT_REASONS: Array<{ code: HabitRejectReason; label: string }> = [
  { code: "too_absolute", label: "太绝对" },
  { code: "not_mine", label: "不是我说的" },
];

export function scopeLabel(scope: HabitScope): string {
  return scope.type === "global" ? "全局" : "仅本场景";
}

/** 候选卡来自哪个场景；缺省按来源线程的场景处理。 */
export function sceneOfCard(card: HabitCard): string {
  return card.scope.type === "scene" ? card.scope.scene : "";
}

/** 默认按候选自身的范围；用户点一次切到全局，再点切回本场景。 */
export function defaultScopeForCard(card: HabitCard): HabitScope {
  return card.scope;
}

export function toggleCandidateScope(scope: HabitScope, scene: string): HabitScope {
  return scope.type === "global"
    ? { type: "scene", scene }
    : { type: "global" };
}

export function setScopeChoice(
  choices: Record<string, HabitScope>,
  id: string,
  scope: HabitScope,
): Record<string, HabitScope> {
  return { ...choices, [id]: scope };
}

export function acceptDecision(id: string, scope: HabitScope): HabitCandidateDecision {
  return { id, action: "accept", scope };
}

export function rejectDecision(
  id: string,
  reason: HabitRejectReason | null,
): HabitCandidateDecision {
  return { id, action: "reject", reason_code: reason };
}

export function acceptAllDecisions(
  cards: HabitCard[],
  choices: Record<string, HabitScope>,
): HabitCandidateDecision[] {
  return cards.map((card) => acceptDecision(card.id, choices[card.id] ?? defaultScopeForCard(card)));
}

