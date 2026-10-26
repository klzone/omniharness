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

export type WorkIntent = "report" | "note";

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

  // 没有「周报」二字时，才看「总结本周」这类措辞，且必须带周范围限定词。
  const hasScope = REPORT_WEEK_SCOPES.some((s) => text.includes(normalizeIntentText(s)));
  if (!hasScope) return "note";
  const hasSummary = REPORT_SUMMARY_WORDS.some((w) => text.includes(w));
  return hasSummary ? "report" : "note";
}

/** 界面上明示本轮走哪条路：用户不必靠猜，误判一眼可见。 */
export function describeWorkIntent(intent: WorkIntent): string {
  return intent === "report" ? "按周报生成" : "按随手记记录";
}
