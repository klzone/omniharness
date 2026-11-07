/** 对话自动检索知识库（OD-ARCH-141 S5）。
 *
 * 纯函数：意图判定、上下文拼装与引用记录。不依赖 React/Tauri，便于 node 单测。
 */

import type {
  KnowledgeVaultNote,
  KnowledgeVaultSearchHit,
  KnowledgeVaultSearchResponse,
} from "./types";

export interface KnowledgeCitation {
  title: string;
  relative_path: string;
}

const INTENT_WORDS = [
  "规定",
  "制度",
  "流程",
  "怎么办",
  "依据",
  "办法",
  "条例",
  "通知",
  "标准",
  "政策",
  "专项检查",
  "审批",
  "频次",
  "处置",
];

export const KNOWLEDGE_CONTEXT_MAX_CHARS = 3000;
export const KNOWLEDGE_MAX_HITS = 3;
export const KNOWLEDGE_MAX_CARDS = 2;

/** 词表命中或显式 @知识库 才检索；其余普通对话不消耗 token。 */
export function detectKnowledgeIntent(text: string): boolean {
  const value = text.trim();
  if (!value) return false;
  if (value.includes("@知识库")) return true;
  return INTENT_WORDS.some((word) => value.includes(word));
}

/**
 * 检索命令只做字面子串匹配，不能把整句问话直接当关键词。
 * 取问句中最长的知识意图词（如「专项检查的流程是什么」→「专项检查」）；
 * 显式 @知识库 但没有内容时返回空串，调用方应跳过检索而不是返回全库。
 */
export function knowledgeSearchKeyword(text: string): string {
  const value = text.replace(/@知识库/g, " ").trim();
  if (!value) return "";
  const matches = INTENT_WORDS.filter((word) => value.includes(word)).sort(
    (a, b) => b.length - a.length,
  );
  return matches[0] ?? value;
}

/** 只注入 AI 建的知识卡，避免把 inbox 原文、用户笔记或索引页带进回答上下文。 */
export function isKnowledgeCardPath(relativePath: string): boolean {
  const path = relativePath.replace(/\\/g, "/");
  return /^知识库\/(概念|方法)\/[^/]+\.md$/i.test(path);
}

export function filterKnowledgeHits(hits: KnowledgeVaultSearchHit[]): KnowledgeVaultSearchHit[] {
  return hits.filter((hit) => isKnowledgeCardPath(hit.relative_path));
}

/** OD-KB-201 S5：命中口径必须区分「知识卡」与「原始资料/索引行」。 */
export function knowledgeNoticeText(totalHits: number, cardHits: number, cited: number): string {
  if (totalHits <= 0) {
    return "知识库没有命中相关条目；本轮未注入知识库上下文。";
  }
  if (cardHits <= 0) {
    return `知识库检索命中 ${totalHits} 条原始资料/索引行，但未命中 AI 知识卡；本轮未注入卡片上下文。`;
  }
  const extra = Math.max(0, totalHits - cardHits);
  const suffix = extra > 0 ? `另有 ${extra} 条原始资料/索引行命中，未注入。` : "";
  return `已检索知识库：${cardHits} 张知识卡命中，引用 ${cited} 张卡。${suffix}`;
}

export function citationTitle(hit: KnowledgeVaultSearchHit | KnowledgeVaultNote): string {
  const name = hit.file_name;
  return name.replace(/\.(md|markdown|txt)$/i, "");
}

export function citationsFromHits(hits: KnowledgeVaultSearchHit[], cards: number): KnowledgeCitation[] {
  return hits.slice(0, cards).map((hit) => ({
    title: citationTitle(hit),
    relative_path: hit.relative_path,
  }));
}

/** 只拼入命中的卡正文（最多两张），超过 3000 字截断并显式标注。 */
export function buildKnowledgeContext(
  hits: KnowledgeVaultSearchHit[],
  notes: KnowledgeVaultNote[],
  limit = KNOWLEDGE_CONTEXT_MAX_CHARS,
): string {
  if (hits.length === 0) return "";
  const cardHits = filterKnowledgeHits(hits).slice(0, KNOWLEDGE_MAX_CARDS);
  if (cardHits.length === 0) return "";
  const byPath = new Map(notes.map((note) => [note.relative_path, note]));
  const lines: string[] = [
    "以下为知识库检索结果（仅用于本轮回答；引用必须与卡片原文一致，不得编造条文）：",
  ];
  for (const hit of cardHits) {
    lines.push(`- ${citationTitle(hit)}（${hit.relative_path}）`);
    const note = byPath.get(hit.relative_path);
    const body = (note?.content ?? hit.snippet ?? "").trim();
    if (body) lines.push(body);
    lines.push("");
  }
  const joined = lines.join("\n");
  if (joined.length <= limit) return joined;
  return joined.slice(0, Math.max(0, limit - 20)) + "\n…（知识库上下文已按上限截断）";
}

/** 给调用方一个稳定的“是否注入”判断，避免把空上下文塞进提示词。 */
export function knowledgeContextFor(
  response: KnowledgeVaultSearchResponse | null,
  notes: KnowledgeVaultNote[],
): { context: string; citations: KnowledgeCitation[] } {
  if (!response || response.hits.length === 0) return { context: "", citations: [] };
  const hits = filterKnowledgeHits(response.hits).slice(0, KNOWLEDGE_MAX_HITS);
  if (hits.length === 0) return { context: "", citations: [] };
  const usedHits = hits.slice(0, KNOWLEDGE_MAX_CARDS);
  const paths = new Set(usedHits.map((hit) => hit.relative_path));
  const usedNotes = notes.filter((note) => paths.has(note.relative_path));
  return {
    context: buildKnowledgeContext(hits, usedNotes),
    citations: citationsFromHits(usedHits, usedHits.length),
  };
}
