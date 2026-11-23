// 工作记录场景的输入意图判定（OD-ARCH-089 S1）。
//
// 纯函数、无依赖：界面与离线验证脚本执行同一份源码。
//
// 用户口径（2026-10-26）：
//   1. 只有**明确**要求生成周报、或总结本周工作的指令才进周报流程；
//   2. 其余所有输入都按工作随手记处理，逐条增量落盘；
//   3. 界面上不再提供「随手记 / 整理成周报」两个标签，意图完全由输入决定。
//
// 判定偏保守：宁可当随手记（多记一条不会丢事实），也不要误进周报流程。

export type WorkIntent = "report" | "note" | "period-review";

/**
 * 明确指向「周报」的措辞。出现即进周报流程。
 *
 * 只收「周报」这个词本身，不做「总结」这类宽泛推断——
 * 「总结一下这个会」是一次随手记，不是周报。
 */
export const REPORT_KEYWORDS: readonly string[] = [
  "生成周报",
  "写周报",
  "出周报",
  "做周报",
  "整周报",
  "整理成周报",
  "整理周报",
  "起草周报",
  "周报生成",
  "周报草稿",
  "本周周报",
  "这周周报",
  "上周周报",
  "周报",
];

/**
 * 「总结/汇总本周工作」的措辞。必须同时出现「本周/这周/上周」限定词，
 * 才能说明指的是周工作，而不是某一次会议。
 */
export const REPORT_WEEK_SCOPES: readonly string[] = ["本周", "这周", "上周", "本星期", "这一周"];

export const REPORT_SUMMARY_WORDS: readonly string[] = [
  "总结",
  "汇总",
  "小结",
  "回顾",
  "梳理",
];

/**
 * 明确指向「周期复盘」的措辞（OD-WR-505）。
 *
 * 和周报一样是封闭枚举：只有用户把范围说成月/季/年（或明确说周复盘）才进
 * 复盘流程。「总结一下本月会议」这类没有周期范围的句子仍是随手记。
 * 词表里刻意不收「复盘」单字，避免把一次普通复盘记录误判成复盘报告。
 */
/**
 * 周期复盘词条（OD-HARDEN-2043 T13）：结构化表，取代按数组下标切片判断周期。
 * offset 表示往前推几个自然周期（1 = 上一个）。
 */
export interface PeriodReviewKeyword {
  keyword: string;
  period: PeriodReviewPeriod;
  offset?: number;
}

export const PERIOD_REVIEW_TABLE: readonly PeriodReviewKeyword[] = [
  { keyword: "周复盘", period: "weekly" },
  { keyword: "复盘本周", period: "weekly" },
  { keyword: "本周复盘", period: "weekly" },
  { keyword: "这周复盘", period: "weekly" },
  { keyword: "上周复盘", period: "weekly", offset: 1 },
  { keyword: "复盘上周", period: "weekly", offset: 1 },
  { keyword: "月度总结", period: "monthly" },
  { keyword: "月度复盘", period: "monthly" },
  { keyword: "月报", period: "monthly" },
  { keyword: "本月复盘", period: "monthly" },
  { keyword: "复盘本月", period: "monthly" },
  { keyword: "本月总结", period: "monthly" },
  { keyword: "这个月复盘", period: "monthly" },
  { keyword: "复盘这个月", period: "monthly" },
  { keyword: "上月复盘", period: "monthly", offset: 1 },
  { keyword: "复盘上月", period: "monthly", offset: 1 },
  { keyword: "上个月复盘", period: "monthly", offset: 1 },
  { keyword: "季度复盘", period: "quarterly" },
  { keyword: "季度总结", period: "quarterly" },
  { keyword: "复盘本季", period: "quarterly" },
  { keyword: "本季复盘", period: "quarterly" },
  { keyword: "本季度复盘", period: "quarterly" },
  { keyword: "复盘本季度", period: "quarterly" },
  { keyword: "年度总结", period: "annual" },
  { keyword: "年度复盘", period: "annual" },
  { keyword: "复盘今年", period: "annual" },
  { keyword: "今年复盘", period: "annual" },
  { keyword: "本年总结", period: "annual" },
  { keyword: "全年总结", period: "annual" },
  { keyword: "复盘全年", period: "annual" },
];

/** 词表字符串视图（既有引用与「关键词命中即复盘」判定沿用）。 */
export const PERIOD_REVIEW_KEYWORDS: readonly string[] = PERIOD_REVIEW_TABLE.map((entry) => entry.keyword);

/**
 * 在词表里取最长命中的词条。
 *
 * 长词更具体：「上月复盘」比「月复盘」多带偏移，插入短词时不能反过来把长词遮住。
 */
export function matchPeriodReviewEntry(
  text: string,
  table: readonly PeriodReviewKeyword[] = PERIOD_REVIEW_TABLE,
): PeriodReviewKeyword | null {
  let best: PeriodReviewKeyword | null = null;
  for (const entry of table) {
    const keyword = normalizeIntentText(entry.keyword);
    if (!text.includes(keyword)) continue;
    if (!best || keyword.length > normalizeIntentText(best.keyword).length) best = entry;
  }
  return best;
}

export type PeriodReviewPeriod = "weekly" | "monthly" | "quarterly" | "annual";

export interface PeriodReviewTarget {
  period: PeriodReviewPeriod;
  /** 周期起始日（含），`YYYY-MM-DD`。 */
  start: string;
  /** 周期结束日（含），`YYYY-MM-DD`。 */
  end: string;
  /** 给界面/提示词看的周期说法，例如「本月」「2026 Q4」。 */
  label: string;
}

/** 归一化：全角转半角、去空白、统一小写。中英标点混写不影响判定。 */
export function normalizeIntentText(input: string): string {
  return input
    .replace(/[\uFF01-\uFF5E]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) - 0xfee0))
    .replace(/\u3000/g, " ")
    .replace(/\s+/g, "")
    .toLowerCase();
}

/**
 * 判定本轮输入该走哪条路。
 *
 * @param input 用户本轮原文（不含附件正文；判定只看用户自己说的话）。
 */
export function detectWorkIntent(input: string): WorkIntent {
  const text = normalizeIntentText(input || "");
  if (!text) return "note";

  // 「周报」二字足够明确：用户说周报就是在要周报。
  if (REPORT_KEYWORDS.some((k) => text.includes(normalizeIntentText(k)))) return "report";

  // 周期复盘：月度/季度/年度，或明确说周复盘。放在周报之后，周报词不会被吞。
  if (PERIOD_REVIEW_KEYWORDS.some((k) => text.includes(normalizeIntentText(k)))) {
    return "period-review";
  }

  // 没有「周报」二字时，才看「总结本周」这类措辞，且必须带周范围限定词。
  const hasScope = REPORT_WEEK_SCOPES.some((s) => text.includes(normalizeIntentText(s)));
  if (!hasScope) return "note";
  const hasSummary = REPORT_SUMMARY_WORDS.some((w) => text.includes(w));
  return hasSummary ? "report" : "note";
}

/** 界面上明示本轮走哪条路：用户不必靠猜，误判一眼可见。 */
export function describeWorkIntent(intent: WorkIntent): string {
  if (intent === "report") return "按周报生成";
  if (intent === "period-review") return "按周期复盘生成";
  return "按随手记记录";
}

/** 由本地日历算出周期复盘的起止日期。只认完整自然周期，不猜偏移。 */
export function detectPeriodReviewTarget(
  input: string,
  now: Date = new Date(),
  offsetMinutes?: number,
  table: readonly PeriodReviewKeyword[] = PERIOD_REVIEW_TABLE,
): PeriodReviewTarget | null {
  const text = normalizeIntentText((input || "").trim());
  if (!text) return null;
  // OD-ARCH-182 修复：词表只收固定搭配，「复盘一下上周的工作」这类说法会漏配。
  // 只要出现总结动词 + 明确的相对周期词，也按复盘处理；范围仍由宿主演算。
  // T13：周期只看结构化命中的词条，不再依赖数组下标。
  const matched = matchPeriodReviewEntry(text, table);
  const keywordHit = matched !== null;
  const summaryHit = ["复盘", "总结", "回顾", "梳理", "汇总", "小结"].some((w) =>
    text.includes(w),
  );
  if (!keywordHit && !summaryHit) return null;

  const cal = calendarOf(now, offsetMinutes);
  const month = new Date(cal.todayDay * DAY_MS).getUTCMonth() + 1;
  const pad = (n: number) => String(n).padStart(2, "0");
  const weeksBack = text.includes("上上周")
    ? 2
    : /上周|上一周|前一周/.test(text)
      ? 1
      : matched?.period === "weekly"
        ? matched.offset ?? 0
        : 0;
  const weeklyHit =
    matched?.period === "weekly" ||
    (summaryHit && /周/.test(text));
  if (weeklyHit || weeksBack > 0) {
    const monday = mondayOf(cal) - 7 * weeksBack;
    return {
      period: "weekly",
      start: fromDayNumber(monday),
      end: fromDayNumber(monday + 6),
      label: weeksBack === 0 ? "本周" : weeksBack === 1 ? "上周" : "上上周",
    };
  }
  if (/月度|月报|本月|这个月|上个月|上月/.test(text)) {
    const back =
      matched?.period === "monthly" && matched.offset
        ? matched.offset
        : /上个月|上月/.test(text)
          ? 1
          : 0;
    const year = cal.todayYear - (month - 1 - back < 0 ? 1 : 0);
    const targetMonth = (((month - 1 - back) % 12) + 12) % 12 + 1;
    return {
      period: "monthly",
      start: String(year) + "-" + pad(targetMonth) + "-01",
      end: String(year) + "-" + pad(targetMonth) + "-" + pad(lastDayOfMonth(year, targetMonth)),
      label: back === 0 ? "本月" : "上月",
    };
  }
  if (/季度|本季|上个季度|上季度/.test(text)) {
    const back = /上个季度|上季度/.test(text) ? 1 : 0;
    const quarterIndex = Math.floor((month - 1) / 3) - back;
    const year = cal.todayYear + Math.floor(quarterIndex / 4);
    const quarter = (((quarterIndex % 4) + 4) % 4) + 1;
    const startMonth = (quarter - 1) * 3 + 1;
    const endMonth = startMonth + 2;
    return {
      period: "quarterly",
      start: String(year) + "-" + pad(startMonth) + "-01",
      end: String(year) + "-" + pad(endMonth) + "-" + pad(lastDayOfMonth(year, endMonth)),
      label: String(year) + " Q" + quarter,
    };
  }
  if (/年度|全年|今年|本年|去年/.test(text)) {
    const year = cal.todayYear - (/去年/.test(text) ? 1 : 0);
    return {
      period: "annual",
      start: String(year) + "-01-01",
      end: String(year) + "-12-31",
      label: String(year) + " 年",
    };
  }
  if (!keywordHit) return null;
  if (matched?.period === "monthly") {
    return {
      period: "monthly",
      start: String(cal.todayYear) + "-" + pad(month) + "-01",
      end:
        String(cal.todayYear) + "-" + pad(month) + "-" + pad(lastDayOfMonth(cal.todayYear, month)),
      label: "本月",
    };
  }
  if (matched?.period === "quarterly") {
    const quarter = Math.floor((month - 1) / 3);
    const startMonth = quarter * 3 + 1;
    const endMonth = startMonth + 2;
    return {
      period: "quarterly",
      start: String(cal.todayYear) + "-" + pad(startMonth) + "-01",
      end:
        String(cal.todayYear) + "-" + pad(endMonth) + "-" + pad(lastDayOfMonth(cal.todayYear, endMonth)),
      label: String(cal.todayYear) + " Q" + (quarter + 1),
    };
  }
  return {
    period: "annual",
    start: String(cal.todayYear) + "-01-01",
    end: String(cal.todayYear) + "-12-31",
    label: String(cal.todayYear) + " 年",
  };
}

/**
 * 由模型判出的周期算出起止日期OD-ARCH-176）。
 *
 * 词表只认用户明确说了「复盘/总结」的说法；模型指出「帮我看看本周干得怎么样」
 * 这类没有词表命中的周期时，宿主仍按完整自然周期取范围，不猜偏移。
 */
export function periodReviewTargetForPeriod(
  period: PeriodReviewPeriod,
  now: Date = new Date(),
  offsetMinutes?: number,
): PeriodReviewTarget {
  const cal = calendarOf(now, offsetMinutes);
  const month = new Date(cal.todayDay * DAY_MS).getUTCMonth() + 1;
  if (period === "weekly") {
    const monday = mondayOf(cal);
    return {
      period: "weekly",
      start: fromDayNumber(monday),
      end: fromDayNumber(monday + 6),
      label: "本周",
    };
  }
  if (period === "monthly") {
    return {
      period: "monthly",
      start: `${cal.todayYear}-${String(month).padStart(2, "0")}-01`,
      end: `${cal.todayYear}-${String(month).padStart(2, "0")}-${String(lastDayOfMonth(cal.todayYear, month)).padStart(2, "0")}`,
      label: "本月",
    };
  }
  if (period === "quarterly") {
    const quarter = Math.floor((month - 1) / 3);
    const startMonth = quarter * 3 + 1;
    const endMonth = startMonth + 2;
    return {
      period: "quarterly",
      start: `${cal.todayYear}-${String(startMonth).padStart(2, "0")}-01`,
      end: `${cal.todayYear}-${String(endMonth).padStart(2, "0")}-${String(lastDayOfMonth(cal.todayYear, endMonth)).padStart(2, "0")}`,
      label: `${cal.todayYear} Q${quarter + 1}`,
    };
  }
  return {
    period: "annual",
    start: `${cal.todayYear}-01-01`,
    end: `${cal.todayYear}-12-31`,
    label: `${cal.todayYear} 年`,
  };
}

function lastDayOfMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

// ---------------------------------------------------------------------------
// 补录日期识别（OD-ARCH-103 S1）
//
// 事实：在 OD-ARCH-103 之前，随手记的落盘日期由宿主固定取「当天」，输入里的
// 「周一上午」没有任何通道传到写入层，于是周三补录周一的课会记到周三标题下。
//
// 这里只做**确定性**识别：认出明确、无歧义的日期说法，算出当前 ISO 周内的那一天。
// 认不出、指向未来、或指向别的周，一律返回 null，交回当天——不猜。
// ---------------------------------------------------------------------------

export interface EntryDateGuess {
  /** `YYYY-MM-DD`，已保证在当前 ISO 周内、且不晚于今天。 */
  date: string;
  /** `周一` … `周日`。 */
  weekday: string;
  /** 命中的写法：明确日期 / 星期几 / 相对日。 */
  source: "explicit" | "weekday" | "relative";
}

const WEEKDAY_NAMES = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

const WEEKDAY_INDEX: Record<string, number> = {
  一: 0, 二: 1, 三: 2, 四: 3, 五: 4, 六: 5, 日: 6, 天: 6,
};

const DAY_MS = 86_400_000;

/** 一次日期计算所依赖的「今天」，按传入时区偏移（分钟）取本地日期。 */
interface Calendar {
  todayDay: number;
  todayYear: number;
  /** 1 = 周一 … 7 = 周日。 */
  todayIsoWeekday: number;
}

function calendarOf(now: Date, offsetMinutes?: number): Calendar {
  const shifted = Number.isFinite(offsetMinutes)
    ? new Date(now.getTime() + (offsetMinutes as number) * 60_000)
    : now;
  const y = Number.isFinite(offsetMinutes) ? shifted.getUTCFullYear() : shifted.getFullYear();
  const m = Number.isFinite(offsetMinutes) ? shifted.getUTCMonth() : shifted.getMonth();
  const d = Number.isFinite(offsetMinutes) ? shifted.getUTCDate() : shifted.getDate();
  const todayDay = Math.floor(Date.UTC(y, m, d) / DAY_MS);
  // 1970-01-01 是周四：(day + 3) % 7 + 1 → 1 = 周一。
  const todayIsoWeekday = (((todayDay + 3) % 7) + 7) % 7 + 1;
  return { todayDay, todayYear: y, todayIsoWeekday };
}

function fromDayNumber(day: number): string {
  const d = new Date(day * DAY_MS);
  const y = String(d.getUTCFullYear()).padStart(4, "0");
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${dd}`;
}

function dayOfDate(year: number, month: number, day: number): number | null {
  if (!Number.isFinite(year) || !Number.isFinite(month) || !Number.isFinite(day)) return null;
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const ms = Date.UTC(year, month - 1, day);
  const probe = new Date(ms);
  // 2 月 30 日这类会被 Date 顺延，逐项回读一致才算合法日期。
  if (probe.getUTCFullYear() !== year || probe.getUTCMonth() !== month - 1 || probe.getUTCDate() !== day) {
    return null;
  }
  return Math.floor(ms / DAY_MS);
}

/** 当前 ISO 周的周一（天数）。 */
function mondayOf(cal: Calendar): number {
  return cal.todayDay - (cal.todayIsoWeekday - 1);
}

/**
 * `YYYY-MM-DD` → `YYYY-Www`（ISO 周）。
 *
 * 前端只用它做一件事：判断「上一条记录的日期」与今天是否属于同一个周文件。
 * 跨周时不再继承日期，交给用户说清楚，避免把上周的记录悄悄写进本周文件。
 */
/** 由周标签推出上一周的标签（跨年时回退到上一年最后一周）。 */
export function previousWeekLabel(week: string): string | null {
  const m = /^(\d{4})-W(\d{2})$/.exec((week || "").trim());
  if (!m) return null;
  const year = Number(m[1]);
  const num = Number(m[2]);
  if (!Number.isFinite(year) || !Number.isFinite(num) || num < 1) return null;
  if (num > 1) return year + "-W" + String(num - 1).padStart(2, "0");
  return isoWeekKeyOf(String(year - 1) + "-12-28");
}

export function isoWeekKeyOf(date: string): string | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec((date || "").trim());
  if (!m) return null;
  const day = dayOfDate(Number(m[1]), Number(m[2]), Number(m[3]));
  if (day === null) return null;
  // 1970-01-01 是周四：ISO 周以该周周四所在年份为准。
  const weekday = (((day + 3) % 7) + 7) % 7 + 1;
  const thursday = day - (weekday - 1) + 3;
  const year = new Date(thursday * DAY_MS).getUTCFullYear();
  const jan1 = Math.floor(Date.UTC(year, 0, 1) / DAY_MS);
  const week = Math.floor((thursday - jan1) / 7) + 1;
  return `${String(year).padStart(4, "0")}-W${String(week).padStart(2, "0")}`;
}

/** 当前本地日期（`YYYY-MM-DD`），与 {@link extractEntryDate} 用同一套时区口径。 */
export function todayDateKey(now: Date = new Date(), offsetMinutes?: number): string {
  return fromDayNumber(calendarOf(now, offsetMinutes).todayDay);
}

function guessForDay(day: number, cal: Calendar, source: EntryDateGuess["source"]): EntryDateGuess | null {
  // 补录窗口覆盖最近 14 天（当前周与上一周）；未来日期一律不猜。
  if (day > cal.todayDay) return null;
  if (cal.todayDay - day > 13) return null;
  const weekdayIndex = (((day + 3) % 7) + 7) % 7;
  return {
    date: fromDayNumber(day),
    weekday: WEEKDAY_NAMES[weekdayIndex] ?? WEEKDAY_NAMES[0],
    source,
  };
}

/** 明确日期：`2026-10-26` 或 `10月26日/号`（当年）。 */
function explicitDate(text: string, cal: Calendar): EntryDateGuess | null {
  const dashed = /(\d{4})-(\d{1,2})-(\d{1,2})/.exec(text);
  if (dashed) {
    const day = dayOfDate(Number(dashed[1]), Number(dashed[2]), Number(dashed[3]));
    if (day !== null) {
      const hit = guessForDay(day, cal, "explicit");
      if (hit) return hit;
    }
  }
  const slash = /(\d{4})\/(\d{1,2})\/(\d{1,2})/.exec(text);
  if (slash) {
    const day = dayOfDate(Number(slash[1]), Number(slash[2]), Number(slash[3]));
    if (day !== null) {
      const hit = guessForDay(day, cal, "explicit");
      if (hit) return hit;
    }
  }
  const monthDay = /(\d{1,2})\s*月\s*(\d{1,2})\s*[日号]/.exec(text);
  if (monthDay) {
    const day = dayOfDate(cal.todayYear, Number(monthDay[1]), Number(monthDay[2]));
    if (day !== null) {
      const hit = guessForDay(day, cal, "explicit");
      if (hit) return hit;
    }
  }
  return null;
}

/**
 * 星期几：`周一` / `星期一` / `礼拜一`。
 *
 * 前置词决定要不要认：`上周/下周/每周` 都不是本周里的某一天，直接跳过；
 * `本周/这周/这个星期` 与裸写法同义。
 */
function weekdayDate(text: string, cal: Calendar): EntryDateGuess | null {
  const re = /(上上|下下|上|下|本|这|每|次)?\s*(?:周|星期|礼拜)\s*([一二三四五六日天])/g;
  const monday = mondayOf(cal);
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const prefix = m[1] ?? "";
    if (prefix === "上上" || prefix === "下下" || prefix === "上" || prefix === "下" || prefix === "每") {
      continue;
    }
    const offset = WEEKDAY_INDEX[m[2]];
    if (offset === undefined) continue;
    const hit = guessForDay(monday + offset, cal, "weekday");
    if (hit) return hit;
  }
  return null;
}

/** 相对日：今天 / 昨天 / 前天。跨周时同样不猜。 */
function relativeDate(text: string, cal: Calendar): EntryDateGuess | null {
  if (/今天|今日|今儿/.test(text)) return guessForDay(cal.todayDay, cal, "relative");
  if (/昨天|昨日|昨儿/.test(text)) return guessForDay(cal.todayDay - 1, cal, "relative");
  if (/前天/.test(text)) return guessForDay(cal.todayDay - 2, cal, "relative");
  return null;
}

/**
 * 从一句话里认出「这条记录属于哪一天」。
 *
 * 优先级：明确日期 > 星期几 > 今天/昨天/前天。
 * 语义上更具体的说法优先——「今天补记周一的课」应该落到周一，而不是今天。
 *
 * @param text 用户本轮原文。
 * @param now 当前时刻（测试可注入）。
 * @param offsetMinutes 本地时区偏移分钟（与宿主 `utc_offset_minutes` 一致）；
 *        省略时用浏览器本地时区。
 */
/**
 * 是否是「直接可以落盘的陈述句」。
 *
 * 只排除确认、提问和明显指令；其余按陈述句处理，由宿主先原文落盘，
 * 需要时再交给模型分析。判定偏保守：拿不准就走模型，不抢写。
 */
export function isPlainStatementNote(text: string): boolean {
  const raw = (text || "").trim();
  if (!raw) return false;
  const flat = normalizeIntentText(raw);
  if (flat.length < 6) return false;
  if (/[?？]/.test(raw)) return false;
  if (/(吗|呢|是否|什么|怎么|为什么|哪|多少|几个|几次|几天|有没有|要不要|需不需要)/.test(raw)) {
    return false;
  }
  if (/^(嗯|好|好的|行|可以|对|是|不是|不用|不要|取消|确认|合并|继续|请把|帮我)/.test(flat)) {
    return false;
  }
  if (/(把|将|挪|移到|改到|改成|删除|删掉|去掉|整理|生成|起草|写一份|做一份|查一下|查查|找一下|看看|总结|复盘|摘要|周报)/.test(raw)) {
    return false;
  }
  return true;
}

export function extractEntryDate(
  text: string,
  now: Date = new Date(),
  offsetMinutes?: number,
): EntryDateGuess | null {
  const raw = (text || "").trim();
  if (!raw) return null;
  const cal = calendarOf(now, offsetMinutes);
  return explicitDate(raw, cal) ?? weekdayDate(raw, cal) ?? relativeDate(raw, cal);
}

export interface RedateInstruction {
  /** 目标日期（当前 ISO 周内、不晚于今天）。 */
  date: string;
  weekday: string;
}

/** 用户在对上一条记录做日期纠正时的说法。 */
const CORRECTION_MARKERS: readonly string[] = [
  "记错",
  "写错",
  "录错",
  "应该是",
  "其实是",
  "不是",
  "改成",
  "改为",
  "挪到",
  "移到",
  "归到",
  "更正",
];

/** 纠正句里允许出现的填充词；去掉它们与日期后几乎不剩内容，才算「纯纠正」。 */
const CORRECTION_FILLERS: readonly string[] = [
  "这条",
  "那条",
  "记录",
  "刚才",
  "上一",
  "一条",
  "的",
  "了",
  "吧",
  "啊",
  "呢",
  "请",
  "帮我",
  "我",
  "你",
  "把",
  "它",
  "日期",
  "时间",
];

/**
 * 认出「这是一句改日期的话」，而不是一条新的工作记录。
 *
 * 例：「记错了，应该是周一」「改成周一上午」。
 * 只有同时满足三条才认：有纠正动词、能算出目标日期、去掉动词与日期后没剩下实质内容。
 * 目标日期在句尾时优先取句尾——「把周一那节课改成周五」里的目标在后半句。
 */
export function detectRedateInstruction(
  text: string,
  now: Date = new Date(),
  offsetMinutes?: number,
): RedateInstruction | null {
  const raw = (text || "").trim();
  if (!raw || raw.length > 40) return null;
  // 必须明确回指上一条，或整句就是在下纠正口令；否则「会议改成周五了」这种
  // 真实记录会被误判成改日期，把上一条挪走。
  const backReference = /这条|那条|上一条|上一|刚才|记错|写错|录错|更正|应该是|其实是|不是/.test(
    raw,
  );
  const startsWithCorrection = /^(请|帮我)?\s*(改成|改为|挪到|移到|归到|换到)/.test(raw);
  if (!backReference && !startsWithCorrection) return null;
  let lastMarker = -1;
  let lastMarkerLength = 0;
  for (const marker of CORRECTION_MARKERS) {
    const at = raw.lastIndexOf(marker);
    if (at > lastMarker) {
      lastMarker = at;
      lastMarkerLength = marker.length;
    }
  }
  if (lastMarker < 0) return null;
  const tail = raw.slice(lastMarker + lastMarkerLength);
  const guess = extractEntryDate(tail, now, offsetMinutes) ?? extractEntryDate(raw, now, offsetMinutes);
  if (!guess) return null;

  let rest = raw;
  for (const marker of CORRECTION_MARKERS) rest = rest.split(marker).join("");
  for (const filler of CORRECTION_FILLERS) rest = rest.split(filler).join("");
  rest = rest
    .replace(/(上上|下下|上|下|本|这|每|次)?\s*(?:周|星期|礼拜)\s*[一二三四五六日天]/g, "")
    .replace(/\d{4}[-/]\d{1,2}[-/]\d{1,2}/g, "")
    .replace(/\d{1,2}\s*月\s*\d{1,2}\s*[日号]/g, "")
    .replace(/今天|今日|今儿|昨天|昨日|昨儿|前天/g, "")
    .replace(/[\s，,。.！!？?、：:；;\u3000]/g, "");
  if (rest.length > 6) return null;
  return { date: guess.date, weekday: guess.weekday };
}

/**
 * 这句话是不是在给上一条记录补细节（而不是一件新事）。
 *
 * 只认那些明确回指上一句的说法：「补充两条」「还有」「细节是」。
 * 「另外」「顺便」不算——它们经常引出一件当天的新事，继承上一条日期会记错。
 */
export function isNoteContinuation(text: string): boolean {
  const raw = (text || "").trim();
  if (!raw) return false;
  return /补充|补上|补记|再补|续上|接着|还有|细节|同上|那条|这条|上面那条|刚才那/.test(raw);
}

/**
 * 句子里出现的日期说法（原样），用于向用户说明「这句里的日期我没敢采用」。
 *
 * {@link extractEntryDate} 返回 null 有两种原因：句子根本没提日期，或提了但指向
 * 未来／别的周。后者必须如实告诉用户，不能悄悄按今天记完就算。
 */
export function findDatePhrase(text: string): string | null {
  const raw = (text || "").trim();
  if (!raw) return null;
  const patterns: RegExp[] = [
    /\d{4}[-/]\d{1,2}[-/]\d{1,2}/,
    /\d{1,2}\s*月\s*\d{1,2}\s*[日号]/,
    /(上上|下下|上|下|本|这|每|次)?\s*(?:周|星期|礼拜)\s*[一二三四五六日天]/,
    /今天|今日|今儿|昨天|昨日|昨儿|前天/,
  ];
  for (const re of patterns) {
    const m = re.exec(raw);
    if (m) return m[0].replace(/\s+/g, "");
  }
  return null;
}
