// 随手记判定（OD-ARCH-104）：模型判语义，代码只守边界。
//
// 背景：OD-ARCH-089/103 用前端正则判断「这句是指令还是内容」。规则没覆盖到的说法
// 一律当正文写进文件——用户说「上杭教学和参观紫金矿业都是本周一的行动，你全部调整
// 至周一的记录。」被原样记成一条记录，随后几句也各自错日、错条。
// 语义判断交给模型；能不能写、写到哪，留在宿主这一侧用确定性规则校验。
//
// 本模块只做四件确定性的事：
//   1. 从模型回复里切出 <omnidesk-verdict> 判定块（流式渲染时同步剥掉）；
//   2. 解析块内 JSON；
//   3. 校验：正文必须是用户原话的连续片段、日期在本 ISO 周且不晚于今天、
//      moves 的 date+index 必须能在当周索引里对上。任一不合规，整份判定作废；
//   4. 把当周索引整理成提示词文本。
//
// 不判断语义：这里没有「指令」「补充」「纠正」的词表。

import { isoWeekKeyOf } from "./workIntent";
import { parseWorklog } from "./weekTimeline";

export const VERDICT_OPEN = "<omnidesk-verdict>";
export const VERDICT_CLOSE = "</omnidesk-verdict>";

/** 提示词里每条正文的截断上限（字符）。 */
export const INDEX_TEXT_MAX = 80;
/** 提示词里最多带多少条索引。 */
export const INDEX_MAX_ENTRIES = 60;
/** 提示词里索引覆盖的天数。 */
export const INDEX_DAYS = 7;
/** 单条记录正文上限（字符）。 */
export const RECORD_TEXT_MAX = 500;

const DAY_MS = 86_400_000;
/** 去重键分隔符（用字符串构造，避免源码里出现控制字符）。 */
const SEP = "|";
const FENCE = String.fromCharCode(96).repeat(3);
const FENCE_ALL = new RegExp(String.fromCharCode(96), "g");

/** 当周索引里的一条：日期 + 当日序号（0 起）+ 正文。 */
export interface WorklogIndexEntry {
  date: string;
  index: number;
  text: string;
  destination: string | null;
}

export interface WorklogIndex {
  /** 文件头里的周标签；没有则为空串。 */
  week: string;
  entries: WorklogIndexEntry[];
}

export type VerdictAction = "record" | "move" | "confirm" | "answer";

export interface VerdictRecord {
  /** 必须是用户原话里的连续片段。 */
  text: string;
  /** YYYY-MM-DD，本 ISO 周内、不晚于今天。 */
  date: string;
}

export interface VerdictMove {
  /** 原日期段。 */
  date: string;
  /** 原日期段里的序号（0 起，与索引一致）。 */
  index: number;
  /** 目标日期段。 */
  to: string;
}

export interface WorkRecordVerdict {
  action: VerdictAction;
  records: VerdictRecord[];
  moves: VerdictMove[];
  /** 一句给用户的话（含缺项追问）。 */
  reply: string;
}

/** 校验通过后的执行计划：move 带上从索引取到的正文，供宿主做一致性校验。 */
export interface VerdictPlan {
  action: VerdictAction;
  records: VerdictRecord[];
  moves: Array<VerdictMove & { expectText: string }>;
  reply: string;
}

export type VerdictCheck = { ok: true; plan: VerdictPlan } | { ok: false; reason: string };

const ACTIONS: readonly VerdictAction[] = ["record", "move", "confirm", "answer"];

/** 从模型完整回复里切出「给人看的正文」与判定块原文。 */
export function splitVerdict(text: string): { visible: string; block: string | null } {
  const raw = text ?? "";
  const openAt = raw.indexOf(VERDICT_OPEN);
  if (openAt < 0) return { visible: raw, block: null };
  const bodyStart = openAt + VERDICT_OPEN.length;
  const closeAt = raw.indexOf(VERDICT_CLOSE, bodyStart);
  const before = raw.slice(0, openAt);
  if (closeAt < 0) {
    // 还没写完（流式）：把未闭合的块从显示里藏掉，避免用户看到半截 JSON。
    return { visible: before, block: null };
  }
  const after = raw.slice(closeAt + VERDICT_CLOSE.length);
  const visible = (before + after).replace(/\n{3,}/g, "\n\n").trim();
  return { visible, block: raw.slice(bodyStart, closeAt) };
}

/** 显示用正文：任何时候都不会把判定块露出来。 */
export function verdictDisplayText(text: string): string {
  return splitVerdict(text).visible;
}

/** 去掉判定块外层可能包着的代码围栏，容忍模型多写一层。 */
function stripFence(body: string): string {
  let text = body.trim();
  if (!text.startsWith(FENCE)) return text;
  const firstBreak = text.indexOf("\n");
  if (firstBreak < 0) return text.replace(FENCE_ALL, "").trim();
  text = text.slice(firstBreak + 1);
  const lastFence = text.lastIndexOf(FENCE);
  if (lastFence >= 0) text = text.slice(0, lastFence);
  return text.trim();
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function asText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

/**
 * 解析判定块。
 *
 * 只接受严格 JSON：宁可判为「未给出判定」，也不要猜模型的意思去写文件。
 */
export function parseVerdict(block: string | null): WorkRecordVerdict | null {
  if (!block || !block.trim()) return null;
  const body = stripFence(block);
  if (!body.startsWith("{")) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const obj = parsed as Record<string, unknown>;
  const action = asText(obj.action) as VerdictAction;
  if (!ACTIONS.includes(action)) return null;
  const records: VerdictRecord[] = [];
  for (const item of asArray(obj.records)) {
    if (!item || typeof item !== "object") return null;
    const entry = item as Record<string, unknown>;
    const text = asText(entry.text);
    const date = asText(entry.date);
    if (!text || !date) return null;
    records.push({ text, date });
  }
  const moves: VerdictMove[] = [];
  for (const item of asArray(obj.moves)) {
    if (!item || typeof item !== "object") return null;
    const entry = item as Record<string, unknown>;
    const date = asText(entry.date);
    const to = asText(entry.to);
    const index = typeof entry.index === "number" ? entry.index : Number.NaN;
    if (!date || !to || !Number.isInteger(index) || index < 0) return null;
    moves.push({ date, index, to });
  }
  const reply = typeof obj.reply === "string" ? obj.reply.trim() : "";
  return { action, records, moves, reply };
}

/** 日历日序号；形状或真实性不对返回 null。 */
export function dayNumberOf(date: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec((date ?? "").trim());
  if (!m) return null;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const d = Number(m[3]);
  const ms = Date.UTC(y, mo - 1, d);
  const probe = new Date(ms);
  if (
    probe.getUTCFullYear() !== y ||
    probe.getUTCMonth() !== mo - 1 ||
    probe.getUTCDate() !== d
  ) {
    return null;
  }
  return Math.floor(ms / DAY_MS);
}

/**
 * 只保留文字与数字，用于「这句是不是用户原话的连续片段」这一项比对。
 *
 * 去标点、去空白：模型把「，」写成「,」或删掉空格不算编造；但改写、合并、
 * 凭空补一句，比对就过不去。
 */
export function comparableText(input: string): string {
  let out = "";
  for (const ch of input ?? "") {
    if (/[\p{L}\p{N}]/u.test(ch)) out += ch;
  }
  return out;
}

export interface VerdictContext {
  /** 用户本轮原文（不含附件正文）。 */
  userText: string;
  /** 当前 ISO 周标签，如 2026-W44。 */
  week: string;
  /** 今天，YYYY-MM-DD（与宿主同一时区口径）。 */
  today: string;
}

/**
 * 校验一份判定。不通过就整份作废，由调用方走降级路径。
 *
 * 这里不做语义判断，只回答「这份判定能不能安全执行」。
 */
export function validateVerdict(
  verdict: WorkRecordVerdict | null,
  ctx: VerdictContext,
  index: WorklogIndex,
): VerdictCheck {
  if (!verdict) return { ok: false, reason: "没有可用的判定" };
  const todayDay = dayNumberOf(ctx.today);
  if (todayDay === null) return { ok: false, reason: "今天日期不合法：" + ctx.today };
  if (!/^\d{4}-W\d{2}$/.test(ctx.week)) return { ok: false, reason: "周标签不合法：" + ctx.week };

  const checkDate = (date: string, label: string): string | null => {
    const day = dayNumberOf(date);
    if (day === null) return label + " 的日期不合法：" + date;
    if (isoWeekKeyOf(date) !== ctx.week) return label + " 的日期 " + date + " 不在 " + ctx.week;
    if (day > todayDay) return label + " 的日期 " + date + " 晚于今天 " + ctx.today;
    return null;
  };

  const userComparable = comparableText(ctx.userText);
  const records: VerdictRecord[] = [];
  const seenRecords = new Set<string>();
  for (const record of verdict.records) {
    const text = record.text.trim();
    if (!text) return { ok: false, reason: "记录正文为空" };
    if (text.length > RECORD_TEXT_MAX) {
      return { ok: false, reason: "记录正文过长（" + text.length + " 字符），已拒绝" };
    }
    const bad = checkDate(record.date, "记录");
    if (bad) return { ok: false, reason: bad };
    const piece = comparableText(text);
    if (!piece) return { ok: false, reason: "记录正文没有实质内容" };
    // 防编造：正文必须是用户原话的一段连续文字。
    if (!userComparable.includes(piece)) {
      return { ok: false, reason: "记录正文不是用户原话的连续片段：" + text.slice(0, 40) };
    }
    const key = record.date + SEP + piece;
    if (seenRecords.has(key)) continue;
    seenRecords.add(key);
    records.push({ text, date: record.date });
  }

  const moves: Array<VerdictMove & { expectText: string }> = [];
  const seenMoves = new Set<string>();
  for (const move of verdict.moves) {
    const fromBad = checkDate(move.date, "被移动的记录");
    if (fromBad) return { ok: false, reason: fromBad };
    const toBad = checkDate(move.to, "目标日期");
    if (toBad) return { ok: false, reason: toBad };
    if (move.date === move.to) {
      return { ok: false, reason: "原日期与目标日期相同：" + move.date };
    }
    const key = move.date + "#" + move.index;
    if (seenMoves.has(key)) {
      return { ok: false, reason: "同一位置被移动了两次：" + key };
    }
    seenMoves.add(key);
    const found = index.entries.find((e) => e.date === move.date && e.index === move.index);
    if (!found) {
      return {
        ok: false,
        reason: "索引里没有 " + move.date + " 的第 " + (move.index + 1) + " 条记录",
      };
    }
    if (!found.text.trim()) {
      return { ok: false, reason: "索引里的这条记录正文为空，无法校验" };
    }
    moves.push({ date: move.date, index: move.index, to: move.to, expectText: found.text });
  }

  // action 必须与内容一致：自相矛盾的判定说明模型没想清楚，不能拿去写文件。
  if (verdict.action === "record" && records.length === 0) {
    return { ok: false, reason: "判定为 record 却没有记录" };
  }
  if (verdict.action === "move" && moves.length === 0) {
    return { ok: false, reason: "判定为 move 却没有要移动的条目" };
  }
  if (verdict.action === "confirm" && (records.length > 0 || moves.length > 0)) {
    return { ok: false, reason: "判定为 confirm 却带着写入或移动" };
  }
  if (verdict.action === "answer" && (records.length > 0 || moves.length > 0)) {
    return { ok: false, reason: "判定为 answer 却带着写入或移动" };
  }

  return { ok: true, plan: { action: verdict.action, records, moves, reply: verdict.reply } };
}

/** 把当周记录解析成索引（日期按文件顺序，序号为当日第几条，0 起）。 */
export function buildWorklogIndex(content: string): WorklogIndex {
  const parsed = parseWorklog(content || "");
  const entries: WorklogIndexEntry[] = [];
  for (const day of parsed.days) {
    day.entries.forEach((entry, i) => {
      entries.push({
        date: day.date,
        index: i,
        text: entry.text.trim(),
        destination: entry.destination,
      });
    });
  }
  return { week: parsed.week ?? "", entries };
}

/**
 * 索引 → 提示词文本。
 *
 * 只带最近若干天、每条截断，避免把小窗口塞满。序号必须与文件里一致：
 * 模型返回的 moves 用 date+index 定位，宿主再拿索引里的正文做一致性校验。
 */
export function describeIndex(index: WorklogIndex, today: string): string {
  const todayDay = dayNumberOf(today);
  const entries = todayDay === null
    ? index.entries
    : index.entries.filter((e) => {
        const day = dayNumberOf(e.date);
        return day !== null && day <= todayDay && todayDay - day < INDEX_DAYS;
      });
  if (entries.length === 0) return "（本周还没有记录）";
  const picked = entries.slice(-INDEX_MAX_ENTRIES);
  const lines: string[] = [];
  for (const entry of picked) {
    const text = entry.text.replace(/\s+/g, " ").trim();
    const clipped = text.length > INDEX_TEXT_MAX ? text.slice(0, INDEX_TEXT_MAX) + "…" : text;
    lines.push("- " + entry.date + " #" + entry.index + " " + clipped);
  }
  return lines.join("\n");
}

/** 供界面回执使用：一条记录的简短标识。 */
export function shortText(text: string, max = 24): string {
  const flat = (text ?? "").replace(/\s+/g, " ").trim();
  return flat.length > max ? flat.slice(0, max) + "…" : flat;
}
