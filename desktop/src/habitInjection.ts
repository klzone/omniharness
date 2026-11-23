// OD-GROW-301 G6：把确认生效的习惯卡注入对话提示词（纯函数，不依赖 React/Tauri）。
//
// 口径（与 G6 任务书一致）：
//   * 只注入 status=active 的卡；候选、停用、归档卡一律不出现；
//   * 作用域匹配：global 全场景生效，scene 只在卡片声明的场景生效；
//   * 最近确认优先，最多 8 张，卡文字合计最多 600 字；
//   * 超出数量或字数的卡整张跳过（不截半句），只记编号，不把正文带出去。
//
// 统计落盘：宿主目前没有“习惯注入统计”追加命令，这里只在 localStorage 的
// omnidesk.habits.lastInjected 里滚动保留最近 50 行（日期 + 卡编号）。
// 正式统计落盘待 G11 补宿主命令后迁移。

import type { HabitCard } from "./types";

export const HABIT_INJECT_INTRO =
  "以下是用户确认过的个人习惯，仅在适用时遵守；与用户当前明确指令冲突时以当前指令为准；拿不准是否适用时先问：";

export const HABIT_MAX_CARDS = 8;
export const HABIT_MAX_CHARS = 600;
export const HABIT_LAST_INJECTED_KEY = "omnidesk.habits.lastInjected";
export const HABIT_LAST_INJECTED_MAX_ROWS = 50;

export interface HabitInjectionResult {
  /** 原提示词；有注入时前面多一段习惯说明。 */
  prompt: string;
  /** 本轮真正注入的卡编号，按注入顺序。 */
  injected: string[];
  /** 因数量或字数上限被整张跳过的卡编号。 */
  skipped: string[];
}

export interface HabitInjectionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function charCount(text: string): number {
  return [...text].length;
}

/** 取出本轮可注入的卡：生效、范围匹配、最近确认优先。 */
export function selectHabitCards(cards: HabitCard[], scene: string): HabitCard[] {
  return cards
    .filter((card) => card.status === "active")
    .filter((card) => card.scope.type === "global" || card.scope.scene === scene)
    .slice()
    .sort((a, b) => {
      const left = a.confirmed_at ?? "";
      const right = b.confirmed_at ?? "";
      if (left !== right) return right.localeCompare(left);
      return a.id.localeCompare(b.id);
    });
}

export function injectHabits(
  prompt: string,
  cards: HabitCard[],
  scene: string,
): HabitInjectionResult {
  const injected: string[] = [];
  const skipped: string[] = [];
  const lines: string[] = [];
  let chars = 0;

  for (const card of selectHabitCards(cards, scene)) {
    const text = card.text.trim();
    const cost = charCount(text);
    if (injected.length >= HABIT_MAX_CARDS || chars + cost > HABIT_MAX_CHARS) {
      skipped.push(card.id);
      continue;
    }
    injected.push(card.id);
    chars += cost;
    lines.push("- " + text);
  }

  if (injected.length === 0) {
    return { prompt, injected, skipped };
  }
  const block = [HABIT_INJECT_INTRO, ...lines].join("\n");
  return { prompt: block + "\n\n" + prompt, injected, skipped };
}

/** 记录行只放日期与卡编号，不放卡正文。 */
export function injectionLogLine(ids: string[], date: string): string {
  return date + "\t" + ids.join(",");
}

/** 滚动保留最近 maxRows 行。 */
export function rollInjectionLog(
  previous: string | null,
  line: string,
  maxRows: number = HABIT_LAST_INJECTED_MAX_ROWS,
): string[] {
  const rows = (previous ?? "")
    .split("\n")
    .filter((row) => row.trim().length > 0);
  rows.push(line);
  return rows.slice(-maxRows);
}

function localDate(now: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return [now.getFullYear(), pad(now.getMonth() + 1), pad(now.getDate())].join("-");
}

/**
 * 把本轮注入的卡编号追加进 localStorage 滚动记录。
 * 没有注入、没有 localStorage 或写入失败时都安静返回，不影响对话提交。
 */
export function recordHabitInjection(
  ids: string[],
  options: { storage?: HabitInjectionStorage | null; now?: Date } = {},
): void {
  if (ids.length === 0) return;
  const storage =
    options.storage ?? (typeof localStorage === "undefined" ? null : localStorage);
  if (!storage) return;
  try {
    const rows = rollInjectionLog(
      storage.getItem(HABIT_LAST_INJECTED_KEY),
      injectionLogLine(ids, localDate(options.now ?? new Date())),
    );
    storage.setItem(HABIT_LAST_INJECTED_KEY, rows.join("\n"));
  } catch (error) {
    console.warn("习惯注入记录未写入 localStorage：", error);
  }
}
