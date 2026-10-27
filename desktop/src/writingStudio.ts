// 写作场景的派生口径（OD-ARCH-094）。
//
// 这里只有**纯函数**：把宿主返回的成果元数据（status / type）与场景会话，
// 归一成界面用的中文标签、分组、90 天回顾判定与「继续创作」选择。
//
// 边界：
//   * 不读文件、不调模型、不写任何存储；文件本身仍是唯一事实来源；
//   * 缺省口径固定且可测：没有 frontmatter / 缺字段 / 非法值 → 成文 + 未分类；
//   * 时间判断由调用方传入 `now`（Unix 秒），便于用合成时钟做确定性验证。

import type { OfflineArtifactEntry } from "./types";

export type WritingStatus = "draft" | "finished" | "published";

export type WritingType = "insight" | "ai" | "blog" | "website" | "uncategorized";

/** 成果超过这么多天没修改，就进入「该回看」提醒（OD-ARCH-094 R3）。 */
export const STALE_DAYS = 90;

export const STALE_SECONDS = STALE_DAYS * 24 * 60 * 60;

/** 缺省口径写成常量，界面与验证脚本引用同一句话。 */
export const STATUS_DEFAULT_NOTE = "未标注状态的成果按「成文」显示";

export const WRITING_STATUSES: Array<{
  key: WritingStatus;
  label: string;
  subtitle: string;
  /** 状态的一句话说明，写在中栏组头下面。 */
  hint: string;
}> = [
  { key: "draft", label: "草稿", subtitle: "DRAFTS", hint: "还在写，或等下一次继续" },
  { key: "finished", label: "成文", subtitle: "FINISHED", hint: "已经成稿，可以回顾或发布" },
  { key: "published", label: "已发布", subtitle: "PUBLISHED", hint: "已经对外发出" },
];

const STATUS_ALIASES: Record<string, WritingStatus> = {
  draft: "draft",
  草稿: "draft",
  published: "published",
  已发布: "published",
  finished: "finished",
  成文: "finished",
};

const TYPE_ALIASES: Record<string, WritingType> = {
  insight: "insight",
  学习心得: "insight",
  ai: "ai",
  ai知识: "ai",
  "ai-knowledge": "ai",
  blog: "blog",
  博客: "blog",
  website: "website",
  网站内容: "website",
  uncategorized: "uncategorized",
  未分类: "uncategorized",
};

/** 状态归一：缺省与无法识别的值都落到「成文」，不报错、不丢条目。 */
export function writingStatus(value: string | null | undefined): WritingStatus {
  const key = (value ?? "").trim().toLowerCase();
  return STATUS_ALIASES[key] ?? "finished";
}

/** 内容类型归一：未知值落到「未分类」。 */
export function writingType(value: string | null | undefined): WritingType {
  const key = (value ?? "").trim().toLowerCase();
  return TYPE_ALIASES[key] ?? "uncategorized";
}

export function statusLabel(value: string | null | undefined): string {
  const key = writingStatus(value);
  return WRITING_STATUSES.find((s) => s.key === key)?.label ?? "成文";
}

const TYPE_LABELS: Record<WritingType, string> = {
  insight: "学习心得",
  ai: "AI 知识",
  blog: "blog",
  website: "网站内容",
  uncategorized: "未分类",
};

export function typeLabel(value: string | null | undefined): string {
  return TYPE_LABELS[writingType(value)];
}

/** 只算真正的文件行；目录行只用于层级展示，不能计入任何计数。 */
export function writingFiles(items: OfflineArtifactEntry[]): OfflineArtifactEntry[] {
  return items.filter((i) => !i.is_dir);
}

/**
 * 按状态分组。数组顺序沿用调用方给出的顺序（成果列表本来就按修改时间倒序），
 * 这里不再排序，避免同一个数字在两个组件里得出不同结果。
 */
export function groupByStatus(items: OfflineArtifactEntry[]): Record<WritingStatus, OfflineArtifactEntry[]> {
  const out: Record<WritingStatus, OfflineArtifactEntry[]> = {
    draft: [],
    finished: [],
    published: [],
  };
  for (const file of writingFiles(items)) {
    out[writingStatus(file.status)].push(file);
  }
  return out;
}

/** 分组的条数快照；三组合计必须等于文件总数（验证脚本会核对）。 */
export function statusCounts(items: OfflineArtifactEntry[]): Record<WritingStatus, number> {
  const groups = groupByStatus(items);
  return {
    draft: groups.draft.length,
    finished: groups.finished.length,
    published: groups.published.length,
  };
}

/**
 * 90 天未动：只有成文 / 已发布才算回顾提醒。草稿本来就在等继续写，不该被催。
 * 修改时间缺失（0）不判定，避免把「不知道」说成「很久没动」。
 */
export function isStale(
  entry: OfflineArtifactEntry,
  nowSeconds: number,
  staleSeconds: number = STALE_SECONDS,
): boolean {
  const status = writingStatus(entry.status);
  if (status === "draft") return false;
  if (!Number.isFinite(entry.modified_at) || entry.modified_at <= 0) return false;
  return nowSeconds - entry.modified_at > staleSeconds;
}

export function staleFiles(
  items: OfflineArtifactEntry[],
  nowSeconds: number,
  staleSeconds: number = STALE_SECONDS,
): OfflineArtifactEntry[] {
  return writingFiles(items).filter((f) => isStale(f, nowSeconds, staleSeconds));
}

export function isWritingFileStale(
  entry: OfflineArtifactEntry,
  nowSeconds: number,
  staleSeconds: number = STALE_SECONDS,
): boolean {
  return isStale(entry, nowSeconds, staleSeconds);
}

/**
 * 「继续创作」取最近修改、且还没发布的成果。
 * 全部已发布、或还没有成果时返回 null，界面改为引导去首页开一段新对话。
 */
export function continueTarget(items: OfflineArtifactEntry[]): OfflineArtifactEntry | null {
  let best: OfflineArtifactEntry | null = null;
  for (const file of writingFiles(items)) {
    if (writingStatus(file.status) === "published") continue;
    if (best === null || file.modified_at > best.modified_at) best = file;
  }
  return best;
}

/** 把一个 Unix 秒格式化成「刚刚 / 3 小时前 / 6 天前」；取不到就「—」。 */
export function relativeUpdated(seconds: number | null, nowSeconds: number): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) return "—";
  const diff = Math.max(0, nowSeconds - seconds);
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 24 * 3600) return `${Math.floor(diff / 3600)} 小时前`;
  const days = Math.floor(diff / (24 * 3600));
  if (days < 30) return `${days} 天前`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前`;
  return `${Math.floor(days / 365)} 年前`;
}

/** 列表卡上的短日期：2026-10-20；取不到就「—」。 */
export function shortDate(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  const d = new Date(seconds * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * 列表条目的兜底标题：成果自生成文件名是 `offline-material-writing-20261020-090000.md`，
 * 直接展示太长。这里只做**展示**用的缩写，不冒充正文标题；正文标题由列表懒加载补上。
 */
export function fallbackTitle(fileName: string): string {
  const stem = fileName.replace(/\.md$/i, "");
  const m = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(
    stem.replace(/^offline-[a-z-]+-/, ""),
  );
  if (!m) return fileName;
  const [, y, mo, d, h, mi] = m;
  return `${y}-${mo}-${d} ${h}:${mi} 的稿件`;
}

/** 从正文 frontmatter 取标题；没有就返回 null（列表退回文件名）。 */
export function titleFromContent(content: string): string | null {
  if (!content.startsWith("---")) return null;
  const lines = content.split(/\r?\n/);
  if (lines[0].trim() !== "---") return null;
  for (let i = 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim() === "---") break;
    const at = line.indexOf(":");
    if (at <= 0 || line.slice(0, at).trim() !== "title") continue;
    let value = line.slice(at + 1).trim();
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
      value = value.slice(1, -1).replace(/\\"/g, '"');
    } else if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) {
      value = value.slice(1, -1);
    }
    return value.trim() || null;
  }
  return null;
}
