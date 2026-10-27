/**
 * 工作记录四象限归类（OD-ARCH-093 S02）。
 *
 * 依据：当周记录文件只有「日期」与「去向」两个可判定字段
 * （`worklog.rs::render_block` 固定写出 `- **事**：…` 与 `**去向**：…`），
 * 没有重要度、紧急度或完成状态。因此这里**不臆造评分**，只用两个可见事实推导：
 *
 *   * 是否「已过日期」：记录所属日期早于今天；
 *   * 是否「进周报」：去向恰为 `本周报引用`（周报 Skill 唯一采用的去向）。
 *
 * 没有「去向」的行不硬塞进四格，统一计入 `unclassified`，保证界面上的
 * 「四格计数 + 待归类 = 本周记录条数」始终成立。
 */

import type { ParsedWorklog } from "./weekTimeline";

export type QuadrantKey =
  | "past-report"
  | "future-report"
  | "past-other"
  | "future-other";

/** 四格的中文标题与一句话动作，顺序即「先做 → 排期 → 回看 → 可缓」。 */
export const QUADRANTS: Array<{
  key: QuadrantKey;
  index: string;
  title: string;
  action: string;
}> = [
  { key: "past-report", index: "1", title: "先做", action: "已过日期 · 进周报" },
  { key: "future-report", index: "2", title: "排期", action: "今天及以后 · 进周报" },
  { key: "past-other", index: "3", title: "回看", action: "已过日期 · 其它去向" },
  { key: "future-other", index: "4", title: "可缓", action: "今天及以后 · 其它去向" },
];

export interface QuadrantItem {
  /** `日期#序号`，与时间线条目一一对应，用于点击筛选。 */
  id: string;
  date: string;
  text: string;
  destination: string | null;
  /** null 表示没有去向、未归入四格。 */
  quadrant: QuadrantKey | null;
}

export interface ClassifiedWeek {
  items: QuadrantItem[];
  counts: Record<QuadrantKey, number>;
  unclassified: number;
}

/** 记录去向是否会被周报引用（与 `skills/memory-report` 一致）。 */
export const REPORT_DESTINATION = "本周报引用";

/**
 * 把当周解析结果归入四格。
 *
 * `today` 必须是 `YYYY-MM-DD`；日期串不合法时按「不早于今天」处理，
 * 即不把无法判定的日期当成已过期。
 */
export function classifyWorklog(parsed: ParsedWorklog, today: string): ClassifiedWeek {
  const counts: Record<QuadrantKey, number> = {
    "past-report": 0,
    "future-report": 0,
    "past-other": 0,
    "future-other": 0,
  };
  const items: QuadrantItem[] = [];
  let unclassified = 0;
  const validToday = /^\d{4}-\d{2}-\d{2}$/.test(today);

  for (const day of parsed.days) {
    const date = day.date;
    const validDate = /^\d{4}-\d{2}-\d{2}$/.test(date);
    day.entries.forEach((entry, index) => {
      const destination = entry.destination ? entry.destination.trim() : null;
      let quadrant: QuadrantKey | null = null;
      if (destination) {
        const important = destination === REPORT_DESTINATION;
        const urgent = validDate && validToday ? date < today : false;
        quadrant = urgent
          ? important
            ? "past-report"
            : "past-other"
          : important
            ? "future-report"
            : "future-other";
        counts[quadrant] += 1;
      } else {
        unclassified += 1;
      }
      items.push({ id: `${date}#${index}`, date, text: entry.text, destination, quadrant });
    });
  }

  return { items, counts, unclassified };
}

/** 某一格里的条目（用于格子内预览与筛选）。 */
export function itemsIn(classified: ClassifiedWeek, key: QuadrantKey): QuadrantItem[] {
  return classified.items.filter((i) => i.quadrant === key);
}

/** 点击某格后交给时间线的可见条目集合；再次点击同一格返回 null（取消筛选）。 */
export function visibleIdsFor(
  classified: ClassifiedWeek,
  key: QuadrantKey | null,
): Set<string> | null {
  if (!key) return null;
  return new Set(itemsIn(classified, key).map((i) => i.id));
}
