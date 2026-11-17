/** 读书页的 AI 提示词组装（OD-READ-001）。
 *
 * 与页面组件分离，便于离线验证；本文件不访问网络、不读文件、不保存任何内容。
 * 原则：只把用户当前选中/已留下的文字交给模型；整理稿只作草稿，由用户显式保存。
 */

export interface ReadingConceptInput {
  bookTitle: string;
  chapterTitle: string;
  paragraph: string;
  quote: string;
  question: string;
}

export interface ReadingAnnotationForAi {
  id: string;
  kind: "highlight" | "bookmark" | "thought";
  chapterTitle: string;
  paragraphIndex: number;
  quote: string;
  body: string;
  source: "user" | "ai";
}

export interface ReadingOrganizeInput {
  bookTitle: string;
  annotations: ReadingAnnotationForAi[];
}

const clip = (text: string, max: number): string => {
  const value = text.trim();
  const chars = Array.from(value);
  return chars.length <= max ? value : `${chars.slice(0, max).join("")}\n…（已按长度截断）`;
};

/** 选中文字后的「解释 / 查概念」提示词；只带当前段落与选中引文。 */
export function buildReadingConceptPrompt(input: ReadingConceptInput): string {
  const question = input.question.trim() || "请解释这段文字，并说明其中的关键概念。";
  return [
    "你在陪一位读者读书。请基于下面提供的书内上下文回答，不要假装读过未提供的章节。",
    `书名：《${input.bookTitle}》`,
    `章节：${input.chapterTitle || "未标注"}`,
    "所在段落（原文）：",
    clip(input.paragraph, 6000),
    "",
    "读者选中的文字：",
    clip(input.quote, 4000),
    "",
    `读者的问题：${clip(question, 2000)}`,
    "",
    "请按以下顺序回答：1. 先用两三句话直接回答；2. 解释选中文字里的关键概念；3. 结合所在段落说明它在说什么；4. 如涉及书中未提供的背景，请明确标为“书外补充”，不要编造原文。",
    "如果问题无法从给定上下文确定，直接说需要更多上下文，不要猜。",
  ].join("\n");
}

/** 把一本书的划线/标记/想法整理成可继续编辑的 Markdown 草稿。 */
export function buildReadingOrganizePrompt(input: ReadingOrganizeInput): string {
  const annotations = input.annotations.slice(0, 80);
  const lines: string[] = [
    "你是阅读笔记整理助手。下面是读者在一本书里亲手留下的划线、标记和想法；请只整理这些内容。",
    `书名：《${input.bookTitle}》`,
    "",
    "读者批注（按章节/段落排列）：",
  ];
  for (const item of annotations) {
    const where = `${item.chapterTitle || "未标注章节"} · 第 ${item.paragraphIndex + 1} 段`;
    const label = item.kind === "highlight" ? "划线" : item.kind === "bookmark" ? "标记" : "想法";
    lines.push(`--- ${where} · ${label} · ${item.id} · ${item.source === "ai" ? "AI 辅助" : "读者"} ---`);
    if (item.quote) lines.push(`原文：${clip(item.quote, 1200)}`);
    if (item.body) lines.push(`批注：${clip(item.body, 1200)}`);
    lines.push("");
  }
  lines.push(
    "请输出一份中文 Markdown 阅读笔记，必须包含以下小节：",
    "1. `## 一句话概览`",
    "2. `## 主题脉络`（合并相近的划线，指出主题之间的关系）",
    "3. `## 概念与解释`（只解释批注里出现的概念；不确定的写“待核对”）",
    "4. `## 值得回看的原句`（逐字引用划线，用 `>` 标出，不要改写原文）",
    "5. `## 我的思考`（只归纳读者已有想法，新增的解释与读者原话分开写）",
    "6. `## 待查问题`",
    "",
    "约束：不得补造书中没有的原文、页码、作者观点或事实；引用必须来自上面的批注；",
    "无法确认的内容标为“待核对”；最后列一节 `## 未覆盖/待核对`，说明哪些段落或问题没有足够材料。",
  );
  if (input.annotations.length > annotations.length) {
    lines.push(`本次只整理了前 ${annotations.length} 条，其余 ${input.annotations.length - annotations.length} 条未纳入。`);
  }
  return lines.join("\n");
}

/** 红线扫描与提交入口使用的用户原文；不包含提示词模板。 */
export function readingConceptInputText(input: ReadingConceptInput): string {
  return [clip(input.quote, 4000), clip(input.question, 2000)]
    .filter((part) => part.trim().length > 0)
    .join("\n");
}

/** 剥掉整段包裹的单个 Markdown 围栏（三个反引号或三个波浪线）；只剥一层，内部围栏保留。 */
export function stripMarkdownFence(markdown: string): string {
  const text = markdown.trim();
  if (!text) return "";
  const lines = text.split(/\r?\n/);
  if (lines.length < 2) return text;
  const first = lines[0].trim();
  const last = lines[lines.length - 1].trim();
  const open = /^(\x60{3,}|~{3,})(.*)$/.exec(first);
  if (!open) return text;
  const marker = open[1];
  const fenceChar = marker[0];
  const close = fenceChar === "\x60" ? /^\x60{3,}$/ : /^~{3,}$/;
  const closeMatch = close.exec(last);
  if (!closeMatch || closeMatch[0].length < marker.length) return text;
  return lines.slice(1, -1).join("\n").trim();
}

export function readingOrganizeInputText(input: ReadingOrganizeInput): string {
  return input.annotations
    .slice(0, 80)
    .map((item) => `${clip(item.quote, 1200)}\n${clip(item.body, 1200)}`)
    .filter((part) => part.trim().length > 0)
    .join("\n");
}
