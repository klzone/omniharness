// 历史会话页的纯展示逻辑（OD-UI-714）。
//
// 这里只把本机线程按当前 Profile 的分类表分组，不读写磁盘、不调用模型。
// 分类表以后会增加、改名或暂时缺少某个键；线程记录不能因此消失，
// 所以未知标签会落到「其他分类」块，而不是被过滤掉。

import type { SceneThread, ThreadTag } from "./sceneThreads";
import type { ThreadCategory } from "./threadCategories";
import { SCENES } from "./scenes.ts";

const BUILTIN_LABELS: Record<string, string> = {
  plain: "未分类",
  weekly: SCENES.weekly.label,
  writing: SCENES.writing.label,
  project: SCENES.project.label,
};

const EXTRA_ORDER = ["plain", "weekly", "writing", "project"];

export interface SessionThreadGroup {
  key: string;
  label: string;
  hint: string;
  threads: SceneThread[];
}

/** 分类显示名：宿主分类优先，其次内置场景名，最后才用原始键。 */
export function sessionCategoryLabel(
  tag: ThreadTag | string,
  categories: ThreadCategory[],
): string {
  const known = categories.find((category) => category.key === tag);
  if (known) return known.label;
  if (BUILTIN_LABELS[tag]) return BUILTIN_LABELS[tag];
  if (tag.startsWith("custom:")) return tag.slice("custom:".length) || "自定义分类";
  return tag || "未分类";
}

function sortThreads(threads: SceneThread[]): SceneThread[] {
  return [...threads].sort((a, b) => {
    if (a.updatedAt !== b.updatedAt) return b.updatedAt - a.updatedAt;
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}

function extraOrder(key: string): number {
  const index = EXTRA_ORDER.indexOf(key);
  return index >= 0 ? index : EXTRA_ORDER.length;
}

/**
 * 按当前分类表分组：分类顺序沿用宿主顺序，空分类不渲染；
 * 分类表里没有的标签追加在末尾，保证切 Profile 后旧对话仍可见。
 */
export function groupSessionThreads(
  threads: SceneThread[],
  categories: ThreadCategory[],
): SessionThreadGroup[] {
  const buckets = new Map<string, SceneThread[]>();
  for (const thread of threads) {
    const bucket = buckets.get(thread.tag);
    if (bucket) bucket.push(thread);
    else buckets.set(thread.tag, [thread]);
  }

  const fromCategories: SessionThreadGroup[] = [];
  for (const category of categories) {
    const items = buckets.get(category.key);
    if (!items || items.length === 0) continue;
    fromCategories.push({
      key: category.key,
      label: category.label,
      hint: category.hint,
      threads: sortThreads(items),
    });
    buckets.delete(category.key);
  }

  const extras = Array.from(buckets.keys()).sort((a, b) => {
    const order = extraOrder(a) - extraOrder(b);
    if (order !== 0) return order;
    return a.localeCompare(b);
  });

  const extraGroups = extras.map((key) => ({
    key,
    label: sessionCategoryLabel(key, categories),
    hint: "这个分类不在当前 Profile 的分类表里；对话记录仍保留并显示在这里。",
    threads: sortThreads(buckets.get(key) ?? []),
  }));

  return [...fromCategories, ...extraGroups];
}

/** 卡片摘要：最后一条有正文的消息，压成一行；没有正文时如实显示空态。 */
export function threadPreview(
  messages: ReadonlyArray<{ text?: string | null }>,
): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const text = (messages[index]?.text ?? "")
      .replace(/\r\n/g, "\n")
      .replace(/\s+/g, " ")
      .trim();
    if (!text) continue;
    return text.length > 88 ? text.slice(0, 88) + "…" : text;
  }
  return "还没有消息";
}

/** 更新时间：今天 / 昨天 / 本年 / 跨年，全部按本机时区显示。 */
export function threadUpdatedLabel(updatedAt: number, now = Date.now()): string {
  if (!Number.isFinite(updatedAt) || updatedAt <= 0) return "时间未知";
  const date = new Date(updatedAt);
  if (Number.isNaN(date.getTime())) return "时间未知";

  const nowDate = new Date(now);
  const pad = (value: number) => String(value).padStart(2, "0");
  const time = pad(date.getHours()) + ":" + pad(date.getMinutes());
  const sameDay =
    date.getFullYear() === nowDate.getFullYear() &&
    date.getMonth() === nowDate.getMonth() &&
    date.getDate() === nowDate.getDate();
  if (sameDay) return "今天 " + time;

  const yesterday = new Date(
    nowDate.getFullYear(),
    nowDate.getMonth(),
    nowDate.getDate() - 1,
  );
  const isYesterday =
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate();
  if (isYesterday) return "昨天 " + time;

  if (date.getFullYear() === nowDate.getFullYear()) {
    return `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
  }
  return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`;
}
