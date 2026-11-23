/**
 * 习惯候选的铃铛数字与首页提醒规则（OD-GROW-301 G5）。
 *
 * 只做确定性换算：不读文件、不调模型、不碰 localStorage。
 * 日期口径与宿主整理配置一致，按 UTC 偏移换算成本地日期。
 */

import { consolidationLocalDate } from "./consolidationFlow.ts";
import type { HabitCard, OpenQuestion } from "./types";

export const HABIT_HOME_DISMISSED_KEY = "omnidesk.habits.homeNoticeDismissed";
export const HABITS_OPEN_EVENT = "omnidesk:habits-open";
export const HABITS_CHANGED_EVENT = "omnidesk:habits-changed";

export interface HabitBellNotice {
  count_items: string;
  redBadge: boolean;
  overdueQuestions: number;
}

export interface OpenQuestionAge {
  question: OpenQuestion;
  idleDays: number;
  overdue: boolean;
}

export function systemUtcOffsetMinutes(now: Date = new Date()): number {
  return -now.getTimezoneOffset();
}

export function localDateFromUtcIso(iso: string, utcOffsetMinutes: number): string {
  const parsed = Date.parse(iso);
  if (!Number.isFinite(parsed)) return "";
  return consolidationLocalDate(parsed / 1000, utcOffsetMinutes);
}

export function localToday(nowSeconds: number, utcOffsetMinutes: number): string {
  return consolidationLocalDate(nowSeconds, utcOffsetMinutes);
}

function daysBetween(today: string, earlier: string): number {
  const left = Date.parse(today + "T00:00:00Z");
  const right = Date.parse(earlier + "T00:00:00Z");
  if (!Number.isFinite(left) || !Number.isFinite(right)) return 0;
  return Math.max(0, Math.round((left - right) / 86_400_000));
}

/** 只排序仍为 open 的问题：7 天未动的排前面，其余按搁置天数从旧到新。 */
export function sortOpenQuestions(
  questions: OpenQuestion[],
  today: string,
  utcOffsetMinutes: number,
): OpenQuestionAge[] {
  return questions
    .filter((question) => question.status === "open")
    .map((question) => {
      const seen = localDateFromUtcIso(question.last_seen, utcOffsetMinutes);
      const idleDays = daysBetween(today, seen);
      return { question, idleDays, overdue: idleDays >= 7 };
    })
    .sort((a, b) => {
      if (a.overdue !== b.overdue) return a.overdue ? -1 : 1;
      if (a.idleDays !== b.idleDays) return b.idleDays - a.idleDays;
      return a.question.id.localeCompare(b.question.id);
    });
}

export function habitBellNotice(
  candidates: HabitCard[],
  questions: OpenQuestion[],
  today: string,
  utcOffsetMinutes: number,
): HabitBellNotice | null {
  const candidateCount = candidates.filter((card) => card.status === "candidate").length;
  const open = sortOpenQuestions(questions, today, utcOffsetMinutes);
  if (candidateCount === 0 && open.length === 0) return null;
  return {
    count_items: `${candidateCount} 条习惯候选，${open.length} 个未决问题`,
    redBadge: candidateCount + open.length > 0,
    overdueQuestions: open.filter((item) => item.overdue).length,
  };
}

/** 当天新建、且仍未处理的候选数量。 */
export function todayCandidateCount(
  candidates: HabitCard[],
  today: string,
  utcOffsetMinutes: number,
): number {
  return candidates.filter(
    (card) =>
      card.status === "candidate" &&
      localDateFromUtcIso(card.created_at, utcOffsetMinutes) === today,
  ).length;
}

/** 当天有新候选且今天尚未关闭提醒时显示首页提醒条。 */
export function shouldShowHomeNotice(
  candidates: HabitCard[],
  today: string,
  dismissedDate: string | null,
  utcOffsetMinutes: number,
): boolean {
  return dismissedDate !== today && todayCandidateCount(candidates, today, utcOffsetMinutes) > 0;
}

export function openHabitWorkspace(): void {
  window.dispatchEvent(new CustomEvent(HABITS_OPEN_EVENT));
}

export function notifyHabitsChanged(): void {
  window.dispatchEvent(new CustomEvent(HABITS_CHANGED_EVENT));
}

