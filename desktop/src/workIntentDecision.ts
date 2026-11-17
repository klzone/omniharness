// 工作记录场景的意图判定契约（OD-ARCH-176 阶段 1）。
//
// 每一轮先由模型理解这句话是「要记的事实」「对助手的指令」「普通对话」
// 还是「复盘 / 周报 / 当日摘要」，再由宿主按判定执行。平台只认下面这份
// 统一契约，不替模型猜语义；解析失败或拿不准一律交回用户确认。
//
// 纯函数、无依赖：界面与离线验证脚本执行同一份源码。

import {
  parseVerdict,
  type WorkRecordVerdict,
} from "./workRecordVerdict";
import { parseRecordReviewValue, type RecordReview } from "./recordReview";
import type { PeriodReviewPeriod } from "./workIntent";
import {
  effortEvidenceIncludes,
  type WorkIntentEffortEvidence,
} from "./modelEffortEvidence";

export { effortEvidenceExcludes, effortEvidenceIncludes } from "./modelEffortEvidence";
export type { WorkIntentEffortEvidence } from "./modelEffortEvidence";

export const INTENT_OPEN = "<omnidesk-intent>";
export const INTENT_CLOSE = "</omnidesk-intent>";

/** 模型可以返回的意图；`uncertain` 表示 note/instruction 边界模糊。 */
export type ModelWorkIntent =
  | "note"
  | "instruction"
  | "chat"
  | "period-review"
  | "report"
  | "daily-summary"
  | "uncertain";

export interface WorkIntentContract {
  intent: ModelWorkIntent;
  /** 仅 note：与既有 omnidesk-verdict 同形的写入计划。 */
  record: WorkRecordVerdict | null;
  /** 仅 note：质量分析随同一调用返回，不二次调用。 */
  review: RecordReview | null;
  /** 给用户看的一句话；instruction/chat 轮就是完整回复。 */
  reply: string;
  /** 仅 period-review：模型判定的周期。 */
  period: PeriodReviewPeriod | null;
}

/** 宿主要执行的动作。`writes` 明确回答「这一轮会不会落盘」。 */
export type WorkIntentAction =
  | "note"
  | "reply"
  | "ask"
  | "period-review"
  | "report"
  | "daily-summary";

export interface WorkIntentRoute {
  action: WorkIntentAction;
  writes: boolean;
  reviewInline: boolean;
}

const FENCE = String.fromCharCode(96).repeat(3);
const FENCE_ALL = new RegExp(String.fromCharCode(96), "g");

const INTENTS: readonly ModelWorkIntent[] = [
  "note",
  "instruction",
  "chat",
  "period-review",
  "report",
  "daily-summary",
  "uncertain",
];

const PERIODS: readonly PeriodReviewPeriod[] = ["weekly", "monthly", "quarterly", "annual"];

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

/** 从模型完整回复里切出「给人看的正文」与意图块原文（流式未闭合时藏掉）。 */
export function splitWorkIntentContract(text: string): { visible: string; block: string | null } {
  const raw = text ?? "";
  const openAt = raw.indexOf(INTENT_OPEN);
  if (openAt < 0) return { visible: raw, block: null };
  const bodyStart = openAt + INTENT_OPEN.length;
  const closeAt = raw.indexOf(INTENT_CLOSE, bodyStart);
  const before = raw.slice(0, openAt);
  if (closeAt < 0) return { visible: before, block: null };
  const after = raw.slice(closeAt + INTENT_CLOSE.length);
  const visible = (before + after).replace(/\n{3,}/g, "\n\n").trim();
  return { visible, block: raw.slice(bodyStart, closeAt) };
}

/** 显示层：任何时候都不把意图块露给用户（含流式半截）。 */
export function workIntentDisplayText(text: string): string {
  return splitWorkIntentContract(text).visible;
}

/**
 * 解析统一意图契约。
 *
 * 只接受严格 JSON：`intent` 缺失或不在枚举内时按 uncertain 处理，
 * 交回宿主提问；`record` 不合规则置空，绝不把坏结构带进写入层。
 */
export function parseWorkIntentContract(
  text: string,
): { contract: WorkIntentContract | null; visible: string } {
  const split = splitWorkIntentContract(text);
  if (!split.block || !split.block.trim()) return { contract: null, visible: split.visible };
  const body = stripFence(split.block);
  if (!body.startsWith("{")) return { contract: null, visible: split.visible };
  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return { contract: null, visible: split.visible };
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { contract: null, visible: split.visible };
  }
  const obj = parsed as Record<string, unknown>;
  const rawIntent = typeof obj.intent === "string" ? obj.intent.trim() : "";
  const intent = (INTENTS as readonly string[]).includes(rawIntent)
    ? (rawIntent as ModelWorkIntent)
    : "uncertain";

  let record: WorkRecordVerdict | null = null;
  if (obj.record && typeof obj.record === "object" && !Array.isArray(obj.record)) {
    record = parseVerdict(JSON.stringify(obj.record));
  }
  const review = obj.review ? parseRecordReviewValue(obj.review) : null;
  const reply = typeof obj.reply_hint === "string" ? obj.reply_hint.trim() : "";
  const rawPeriod = typeof obj.period === "string" ? obj.period.trim() : "";
  const period = (PERIODS as readonly string[]).includes(rawPeriod)
    ? (rawPeriod as PeriodReviewPeriod)
    : null;

  return {
    contract: { intent, record, review, reply, period },
    visible: split.visible,
  };
}

/** 意图 → 宿主动作。note 是唯一会写盘的对话意图。 */
export function decideWorkIntentRoute(intent: ModelWorkIntent): WorkIntentRoute {
  if (intent === "note") return { action: "note", writes: true, reviewInline: true };
  if (intent === "instruction" || intent === "chat") {
    return { action: "reply", writes: false, reviewInline: false };
  }
  if (intent === "period-review") {
    return { action: "period-review", writes: true, reviewInline: false };
  }
  if (intent === "report") return { action: "report", writes: true, reviewInline: false };
  if (intent === "daily-summary") {
    return { action: "daily-summary", writes: true, reviewInline: false };
  }
  return { action: "ask", writes: false, reviewInline: false };
}

/**
 * 本轮提交的思考档位（OD-ARCH-176 阶段 1 第 6 条）。
 *
 * - 用户手动选过档位 → 按所选档位（`default` 视为不传）；
 * - 分类 / 随手记分析 / 指令回复 → 证据声明支持 off 时默认 `off`（模型直出）；
 * - 复盘 / 周报 / 当日摘要起草 → 不传，跟随模型默认。
 *
 * 返回 null 表示不显式设置 reasoningEffort。档位必须被证据「证明支持」才下发：
 * 真实 DSH 的 session/selectModel 会拒绝未声明模型的任何显式档位（含 off，
 * OD-ARCH-176 线上回归实测）；未声明模型本就没有思考可关，不传档位即等效直出。
 */
export function pickWorkIntentEffort(input: {
  intent: string;
  userSelected: boolean;
  selectedEffort: string;
  evidence?: WorkIntentEffortEvidence | null;
}): string | null {
  const selected = (input.selectedEffort ?? "").trim();
  if (input.userSelected) {
    if (!selected || selected === "default") return null;
    return effortEvidenceIncludes(input.evidence, selected) ? selected : null;
  }
  if (input.intent === "report" || input.intent === "period-review" || input.intent === "daily-summary") {
    return null;
  }
  return effortEvidenceIncludes(input.evidence, "off") ? "off" : null;
}
