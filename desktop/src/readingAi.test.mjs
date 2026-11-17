/** OD-READ-001：读书 AI 提示词纯函数单测。 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  buildReadingConceptPrompt,
  buildReadingOrganizePrompt,
  readingConceptInputText,
  readingOrganizeInputText,
  stripMarkdownFence,
} from "./readingAi.ts";

const concept = {
  bookTitle: "合成书",
  chapterTitle: "第一章",
  paragraph: "前文。选中句。后文。",
  quote: "选中句",
  question: "这里的概念是什么意思？",
};

test("概念提示词只带当前段落并明确边界", () => {
  const prompt = buildReadingConceptPrompt(concept);
  assert.match(prompt, /《合成书》/);
  assert.match(prompt, /这里的概念是什么意思/);
  assert.match(prompt, /不要编造原文/);
  assert.match(prompt, /书外补充/);
  assert.ok(prompt.includes(concept.quote));
  assert.ok(prompt.length < 12_000);
});

test("概念提示词按上限截断超长段落", () => {
  const prompt = buildReadingConceptPrompt({
    ...concept,
    paragraph: "长".repeat(9_000),
  });
  assert.match(prompt, /已按长度截断/);
  assert.ok(prompt.length < 12_000);
});

test("整理提示词最多 80 条并保留未复核边界", () => {
  const annotations = Array.from({ length: 90 }, (_, index) => ({
    id: `an_${index}`,
    kind: index % 3 === 0 ? "highlight" : index % 3 === 1 ? "bookmark" : "thought",
    chapterTitle: "第一章",
    paragraphIndex: 0,
    quote: `引文 ${index}`,
    body: `想法 ${index}`,
    source: "user",
  }));
  const prompt = buildReadingOrganizePrompt({ bookTitle: "合成书", annotations });
  assert.ok(prompt.includes("an_0"));
  assert.ok(prompt.includes("an_79"));
  assert.ok(!prompt.includes("an_80"));
  assert.match(prompt, /其余 10 条未纳入/);
  assert.match(prompt, /不得补造/);
  assert.match(prompt, /未覆盖\/待核对/);
});

test("红线扫描文本不包含提示词模板", () => {
  assert.equal(readingConceptInputText(concept), "选中句\n这里的概念是什么意思？");
  const organize = readingOrganizeInputText({
    bookTitle: "合成书",
    annotations: [
      {
        id: "an_1",
        kind: "highlight",
        chapterTitle: "第一章",
        paragraphIndex: 0,
        quote: "引文",
        body: "想法",
        source: "user",
      },
    ],
  });
  assert.equal(organize, "引文\n想法");
  assert.ok(!organize.includes("整理笔记助手"));
});

test("整理稿整段围栏只剥一层", () => {
  const bt = "\x60";
  const fence = bt + bt + bt;
  const fenced = fence + "markdown\n# 标题\n\n内容\n" + fence;
  assert.equal(stripMarkdownFence(fenced), "# 标题\n\n内容");

  assert.equal(stripMarkdownFence("~~~\n# 标题\n~~~"), "# 标题");

  const inner = fence + "markdown\n# 标题\n\n" + fence + "text\n代码\n" + fence + "\n" + fence;
  assert.equal(stripMarkdownFence(inner), "# 标题\n\n" + fence + "text\n代码\n" + fence);

  const trailing = fence + "\n# 标题\n" + fence + "\n\n后记";
  assert.equal(stripMarkdownFence(trailing), trailing);

  assert.equal(stripMarkdownFence("# 标题"), "# 标题");
  assert.equal(stripMarkdownFence(""), "");
});
