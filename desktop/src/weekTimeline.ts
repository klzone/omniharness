/**
 * 当周工作记录 → 时间线数据（OD-ARCH-091 W1）。
 *
 * 输入是宿主 `worklog_append` 写出来的 Markdown 原文，格式由 `worklog.rs::render_block`
 * 决定，不是机器 schema：
 *
 *     # 2026-W44
 *
 *     ## 2026-10-26
 *
 *     - **事**：今天做了什么
 *       **去向**：本周报引用
 *
 * 同一天再记一次会另起 `## 2026-10-26（续）`，不合并旧段。所以解析时必须把「续」段
 * 归回同一天，并保持文件里的先后顺序（追加顺序即发生顺序）。
 *
 * 诚实性：认不出的行**原样保留**在当天的 `raw` 里，界面上照原样显示、不做删改。
 * 少显示一条用户写下的记录，比显示得难看严重得多。
 */

/** 一条记录：「事」正文 + 去向。 */
export interface TimelineEntry {
  /** 「事」的正文；多行记录保留换行。 */
  text: string;
  /** 去向标签；缺失时为 null（旧文件或手写文件可能没有）。 */
  destination: string | null;
}

/** 一天的分组。 */
export interface TimelineDay {
  /** `YYYY-MM-DD`。 */
  date: string;
  entries: TimelineEntry[];
  /** 这一天里没解析成条目的原文行（照原样显示）。 */
  raw: string[];
  /** 这一天是否只有「（续）」段。 */
  continued: boolean;
}

export interface ParsedWorklog {
  /** 文件头 `# 2026-W44` 里的周标签；没有则为 null。 */
  week: string | null;
  /** 按文件顺序排列的日期（已合并同日「续」段）。 */
  days: TimelineDay[];
  /** 第一个 `## 日期` 之前的所有非空原文行。 */
  preamble: string[];
  /** 有记录的条目总数，便于界面显示「本周 4 条」。 */
  entryCount: number;
}

const DATE_HEADING = /^##\s+(\d{4}-\d{2}-\d{2})(?:\s*[（(]续[）)])?\s*$/;
const WEEK_HEADING = /^#\s+(\d{4}-W\d{2})\s*$/;
const ITEM_LINE = /^-\s+\*\*事\*\*[：:]\s?(.*)$/;
const DEST_LINE = /^\s+\*\*去向\*\*[：:]\s?(.*)$/;

/**
 * 解析一份周文件的 Markdown。
 *
 * 空内容返回空结构（周文件还没建，不是错误）。
 */
export function parseWorklog(content: string): ParsedWorklog {
  const out: ParsedWorklog = { week: null, days: [], preamble: [], entryCount: 0 };
  if (!content) return out;

  const byDate = new Map<string, TimelineDay>();
  let current: TimelineDay | null = null;
  /** 正在累积的条目；`**去向**` 行出现前，后续缩进/裸行都算它的续文。 */
  let pending: TimelineEntry | null = null;

  const dayFor = (date: string): TimelineDay => {
    const found = byDate.get(date);
    if (found) return found;
    const created: TimelineDay = { date, entries: [], raw: [], continued: false };
    byDate.set(date, created);
    out.days.push(created);
    return created;
  };

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+$/, "");
    if (!line.trim()) continue;

    const week = WEEK_HEADING.exec(line);
    if (week) {
      out.week = week[1];
      continue;
    }

    const heading = DATE_HEADING.exec(line);
    if (heading) {
      current = dayFor(heading[1]);
      if (/[（(]续[）)]/.test(line)) current.continued = true;
      pending = null;
      continue;
    }

    const item = ITEM_LINE.exec(line);
    if (item) {
      if (!current) {
        out.preamble.push(line);
        continue;
      }
      pending = { text: item[1], destination: null };
      current.entries.push(pending);
      out.entryCount += 1;
      continue;
    }

    const dest = DEST_LINE.exec(line);
    if (dest) {
      if (pending) {
        pending.destination = dest[1].trim() || null;
        pending = null;
      } else if (current) {
        current.raw.push(line);
      } else {
        out.preamble.push(line);
      }
      continue;
    }

    // 记录正文里的换行：宿主原样写入用户文本，第二行起没有前缀。
    if (pending && !pending.destination) {
      pending.text += "\n" + line.trim();
      continue;
    }
    if (current) current.raw.push(line);
    else out.preamble.push(line);
  }

  return out;
}

/** `2026-W44` → 数字周次；形状不对返回 null。 */
export function parseWeekLabel(week: string): { year: number; week: number } | null {
  const m = /^(\d{4})-W(\d{2})$/.exec(week.trim());
  if (!m) return null;
  const num = Number(m[2]);
  if (num < 1 || num > 53) return null;
  return { year: Number(m[1]), week: num };
}

function utcDate(year: number, month: number, day: number): Date {
  return new Date(Date.UTC(year, month - 1, day));
}

function isoDate(d: Date): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/**
 * ISO 周 → 该周周一的日期（UTC 计算，避免本机时区跨日漂移）。
 *
 * ISO 8601：包含 1 月 4 日的那一周是第 1 周，周一为一周之始。
 */
export function weekStart(week: string): Date | null {
  const parsed = parseWeekLabel(week);
  if (!parsed) return null;
  const jan4 = utcDate(parsed.year, 1, 4);
  const jan4Weekday = (jan4.getUTCDay() + 6) % 7; // 周一 = 0
  const week1Monday = new Date(jan4);
  week1Monday.setUTCDate(jan4.getUTCDate() - jan4Weekday);
  const monday = new Date(week1Monday);
  monday.setUTCDate(week1Monday.getUTCDate() + (parsed.week - 1) * 7);
  return monday;
}

/** 周标签 → 周一到周日的七个日期串（`YYYY-MM-DD`）。形状不对返回空数组。 */
export function weekDates(week: string): string[] {
  const start = weekStart(week);
  if (!start) return [];
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(start);
    d.setUTCDate(start.getUTCDate() + i);
    return isoDate(d);
  });
}

const WEEKDAY_LABELS = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

/** `2026-10-26` → `周一`。日期串不合法返回空。 */
export function weekdayLabel(date: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!m) return "";
  const d = utcDate(Number(m[1]), Number(m[2]), Number(m[3]));
  if (Number.isNaN(d.getTime())) return "";
  return WEEKDAY_LABELS[(d.getUTCDay() + 6) % 7];
}

/** `2026-10-26` → `10/26`。 */
export function shortDate(date: string): string {
  const m = /^\d{4}-(\d{2})-(\d{2})$/.exec(date);
  return m ? `${m[1]}/${m[2]}` : date;
}

/** 本机今天的日期串。 */
export function localToday(now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/**
 * 把当周七天铺成完整时间线：没记录的日子也占一行（灰点占位），
 * 否则「本周什么都没记」和「今天没记」在界面上分不清。
 *
 * 文件里有、但不在当周范围内的日期（跨周补记或手写错日期）追加在最后，
 * 标 `outOfWeek`，绝不悄悄丢掉。
 */
export function buildTimeline(
  parsed: ParsedWorklog,
  week: string,
): Array<TimelineDay & { outOfWeek: boolean }> {
  const skeleton = weekDates(week);
  if (skeleton.length === 0) {
    return parsed.days.map((d) => ({ ...d, outOfWeek: false }));
  }
  const known = new Map(parsed.days.map((d) => [d.date, d]));
  const rows: Array<TimelineDay & { outOfWeek: boolean }> = skeleton.map((date) => {
    const found = known.get(date);
    known.delete(date);
    return found ? { ...found, outOfWeek: false } : { date, entries: [], raw: [], continued: false, outOfWeek: false };
  });
  for (const left of parsed.days) {
    if (known.has(left.date)) rows.push({ ...left, outOfWeek: true });
  }
  return rows;
}

/** 本周是否一条记录都没有（空态判定，不算占位行）。 */
export function isEmptyTimeline(days: TimelineDay[]): boolean {
  return days.every((d) => d.entries.length === 0 && d.raw.length === 0);
}

// ---------------------------------------------------------------------------
// 周报状态（OD-ARCH-091 W2）
// ---------------------------------------------------------------------------

/** 只在文件名层面用到的最小字段；宿主回执里的完整结构可直接传入。 */
export interface WeekFileLike {
  week: string;
  file_name: string;
}

export type ReportState = "missing" | "draft" | "archived";

/**
 * 判定某一周的周报状态：纯文件名检查，不读正文、不调模型。
 *
 *   * 当周没有任何周报文件 → `missing`；
 *   * 当周有文件，且就是本周 → `draft`（还在写，可以继续改）；
 *   * 过去某周有文件 → `archived`（这一周的周报已经定稿）。
 *
 * 「已归档」是时间意义上的说法，不代表宿主冻结了文件。
 */
export function reportState<T extends WeekFileLike>(
  files: T[],
  week: string,
  currentWeek: string,
): { state: ReportState; files: T[] } {
  const mine = files.filter((f) => f.week === week);
  if (mine.length === 0) return { state: "missing", files: [] };
  return { state: week === currentWeek ? "draft" : "archived", files: mine };
}

/** `2026-W44` → `2026-10`；形状不对返回 null。 */
export function weekMonth(week: string): string | null {
  const start = weekStart(week);
  if (!start) return null;
  return `${start.getUTCFullYear()}-${String(start.getUTCMonth() + 1).padStart(2, "0")}`;
}
