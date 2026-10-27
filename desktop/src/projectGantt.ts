/**
 * 项目甘特时间轴的纯函数层（OD-ARCH-098 S02）。
 *
 * 只做「时间 → 坐标」的确定性换算，不碰 React、不读时钟、不读文件。
 * `today` 一律由调用方传入，同一份输入永远给同一份输出。
 *
 * 诚实性约束：事项条目只有「期限」，开始日期是可选的第六条字段。
 * 没有开始日期就画单日里程碑，**不**用期限倒推一段虚构工期。
 */

import type { ProjectItem } from "./types";

/** 一天 = 86400000 ms；用 UTC 日序数避免本地时区把日期推前/推后一天。 */
const DAY_MS = 86_400_000;

/** 时间轴至少展示的天数，避免单条事项撑出一根空轴。 */
export const MIN_WINDOW_DAYS = 14;

/** 时间轴两侧留白（天），让首尾条目不贴边。 */
export const WINDOW_PAD_DAYS = 2;

/** 超过这个天数就切周刻度（日刻度会挤成一片）。 */
export const DAY_TICK_MAX_DAYS = 30;

export type GanttStatus = ProjectItem["status"];

export interface GanttWindow {
  /** 起止日序数（含两端）。 */
  startDay: number;
  endDay: number;
  /** 窗口覆盖的天数，至少 1。 */
  dayCount: number;
  /** 起止的 ISO 日期，方便界面显示。 */
  start: string;
  end: string;
  /** 刻度粒度：日或周。 */
  granularity: "day" | "week";
}

export interface GanttTick {
  /** ISO 日期（刻度所在的那一天）。 */
  date: string;
  /** 显示文案：日刻度 MM-DD，周刻度 MM-DD 起。 */
  label: string;
  /** 距窗口左边的百分比（0–100）。 */
  leftPct: number;
}

export interface GanttBar {
  item: ProjectItem;
  /** range = 有开始日期的区间条；milestone = 只有期限的单日菱形。 */
  kind: "range" | "milestone";
  /** 条/点的左缘百分比。 */
  leftPct: number;
  /** 区间条宽度百分比；里程碑为 0。 */
  widthPct: number;
  /** 期限所在位置的百分比（里程碑就是它自己）。 */
  duePct: number;
  /** 区间天数；里程碑为 0。 */
  spanDays: number;
  /** 落在窗口内（窗口两侧的留白可能不含极早/极晚条目）。 */
  inWindow: boolean;
}

/** 严格解析 YYYY-MM-DD，非法或日历上不存在返回 null。 */
export function toDayNumber(text: string | null | undefined): number | null {
  if (!text) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (!m) return null;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const ms = Date.UTC(year, month - 1, day);
  const back = new Date(ms);
  if (
    back.getUTCFullYear() !== year ||
    back.getUTCMonth() !== month - 1 ||
    back.getUTCDate() !== day
  ) {
    return null;
  }
  return Math.round(ms / DAY_MS);
}

/** 日序数转回 YYYY-MM-DD。 */
export function fromDayNumber(day: number): string {
  const d = new Date(day * DAY_MS);
  const y = String(d.getUTCFullYear()).padStart(4, "0");
  const mo = String(d.getUTCMonth() + 1).padStart(2, "0");
  const da = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${mo}-${da}`;
}

/** MM-DD 短标签。 */
function shortLabel(day: number): string {
  return fromDayNumber(day).slice(5);
}

/** 周一作为一周的起点（0 = 周日）。 */
function isoWeekday(day: number): number {
  // 1970-01-01 是周四 → 4。
  return (((day + 4) % 7) + 7) % 7;
}

/**
 * 计算时间窗：覆盖所有条目的开始/期限与今天，两侧各留 2 天，至少 14 天。
 * 空列表也要给出一根可渲染的轴（以今天居中）。
 */
export function ganttWindow(items: ProjectItem[], today: string): GanttWindow {
  const todayDay = toDayNumber(today);
  const days: number[] = [];
  for (const item of items) {
    const due = toDayNumber(item.due);
    if (due !== null) days.push(due);
    const start = toDayNumber(item.start ?? null);
    if (start !== null) days.push(start);
  }
  if (todayDay !== null) days.push(todayDay);
  if (days.length === 0) {
    // 连今天都解析不了时给一根 14 天的相对轴，不抛错打断整页渲染。
    const base = 0;
    return {
      startDay: base,
      endDay: base + MIN_WINDOW_DAYS - 1,
      dayCount: MIN_WINDOW_DAYS,
      start: fromDayNumber(base),
      end: fromDayNumber(base + MIN_WINDOW_DAYS - 1),
      granularity: "day",
    };
  }
  let lo = Math.min(...days) - WINDOW_PAD_DAYS;
  let hi = Math.max(...days) + WINDOW_PAD_DAYS;
  if (hi - lo + 1 < MIN_WINDOW_DAYS) {
    const grow = MIN_WINDOW_DAYS - (hi - lo + 1);
    lo -= Math.floor(grow / 2);
    hi += grow - Math.floor(grow / 2);
  }
  const dayCount = hi - lo + 1;
  return {
    startDay: lo,
    endDay: hi,
    dayCount,
    start: fromDayNumber(lo),
    end: fromDayNumber(hi),
    granularity: dayCount > DAY_TICK_MAX_DAYS ? "week" : "day",
  };
}

/** 窗口内的百分比位置（0–100，夹取在窗口内）。 */
export function pctOf(day: number, win: GanttWindow): number {
  if (win.dayCount <= 0) return 0;
  // 首尾两天分别落在 0% 与 100%：跨度按 dayCount - 1 算，否则最后一天永远差一格。
  const raw = ((day - win.startDay) / Math.max(1, win.dayCount - 1)) * 100;
  return Math.max(0, Math.min(100, raw));
}

/** 刻度：日粒度每 2 天一个（窄窗也读得清），周粒度对齐到周一。 */
export function ganttTicks(win: GanttWindow, maxTicks = 8): GanttTick[] {
  const ticks: GanttTick[] = [];
  if (win.granularity === "week") {
    // 从窗口内第一个周一开始，每 7 天一个刻度。
    let day = win.startDay;
    while (isoWeekday(day) !== 1 && day <= win.endDay) day += 1;
    for (; day <= win.endDay; day += 7) {
      ticks.push({ date: fromDayNumber(day), label: shortLabel(day), leftPct: pctOf(day, win) });
    }
    if (ticks.length === 0) {
      ticks.push({
        date: win.start,
        label: shortLabel(win.startDay),
        leftPct: 0,
      });
    }
    return ticks;
  }
  const step = Math.max(1, Math.ceil(win.dayCount / Math.max(1, maxTicks)));
  for (let i = 0; i < win.dayCount; i += step) {
    const day = win.startDay + i;
    ticks.push({ date: fromDayNumber(day), label: shortLabel(day), leftPct: pctOf(day, win) });
  }
  return ticks;
}

/**
 * 单条事项的条/点几何。
 *
 * * 有合法开始日期且不晚于期限 → 区间条。
 * * 没有开始日期、或日期解析不出来 → 期限处的菱形里程碑。
 */
export function layoutItem(item: ProjectItem, win: GanttWindow): GanttBar {
  const dueDay = toDayNumber(item.due);
  const startDay = toDayNumber(item.start ?? null);
  if (startDay !== null && dueDay !== null && startDay <= dueDay) {
    const spanDays = dueDay - startDay;
    // 单日区间也要有可见宽度：按天比例算完后给一个最小宽度，否则会缩成看不见的线。
    const widthDays = Math.max(1, spanDays + 1);
    const leftPct = pctOf(startDay, win);
    const rawWidth = ((widthDays - 1) / Math.max(1, win.dayCount - 1)) * 100;
    const minWidth = 1.2;
    const widthPct = Math.max(0, Math.min(100 - leftPct, Math.max(minWidth, rawWidth)));
    return {
      item,
      kind: "range",
      leftPct,
      widthPct,
      duePct: pctOf(dueDay, win),
      spanDays,
      inWindow: true,
    };
  }
  const left = dueDay === null ? 0 : pctOf(dueDay, win);
  return {
    item,
    kind: "milestone",
    leftPct: left,
    widthPct: 0,
    duePct: left,
    spanDays: 0,
    inWindow: dueDay === null ? false : dueDay >= win.startDay && dueDay <= win.endDay,
  };
}

/** 批量排布，顺序与输入一致（与左栏卡片一一对应）。 */
export function layoutItems(items: ProjectItem[], win: GanttWindow): GanttBar[] {
  return items.map((item) => layoutItem(item, win));
}

/** 今天竖线的位置；今天不在窗口内返回 null（不画一条假线）。 */
export function todayPct(today: string, win: GanttWindow): number | null {
  const day = toDayNumber(today);
  if (day === null || day < win.startDay || day > win.endDay) return null;
  return pctOf(day, win);
}

/** 里程碑判定（界面与测试共用一套口径）。 */
export function isMilestone(item: ProjectItem): boolean {
  const due = toDayNumber(item.due);
  const start = toDayNumber(item.start ?? null);
  return due === null || start === null || start > due;
}

/** 搜索过滤：按事项标题与负责人代号匹配，空查询返回全部。 */
export function filterItems(items: ProjectItem[], query: string): ProjectItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  return items.filter(
    (i) =>
      i.item.toLowerCase().includes(q) ||
      i.owner.toLowerCase().includes(q) ||
      i.next_step.toLowerCase().includes(q),
  );
}
