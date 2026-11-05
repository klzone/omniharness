/** OD-ARCH-141 S5 纯函数单测：node --experimental-strip-types --test knowledgeRetrieval.test.mjs */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  buildKnowledgeContext,
  citationsFromHits,
  detectKnowledgeIntent,
  filterKnowledgeHits,
  knowledgeContextFor,
  knowledgeSearchKeyword,
} from "./knowledgeRetrieval.ts";

const hit = (path, snippet = "摘要") => ({
  relative_path: path,
  file_name: path.split("/").pop(),
  bytes: 10,
  sha256: "sha",
  match_areas: ["body"],
  tags: [],
  property_count: 0,
  warning_count: 0,
  snippet,
  snippet_line: 1,
  keyword_hits: 1,
});

const note = (path, content) => ({
  relative_path: path,
  file_name: path.split("/").pop(),
  bytes: content.length,
  chars: content.length,
  extension: "md",
  sha256: "sha",
  content,
});

test("词表与显式 @知识库 触发检索", () => {
  assert.equal(detectKnowledgeIntent("专项检查的流程是什么"), true);
  assert.equal(detectKnowledgeIntent("请 @知识库 回答"), true);
  assert.equal(detectKnowledgeIntent("今天天气怎么样"), false);
});

test("无命中不生成上下文", () => {
  assert.equal(buildKnowledgeContext([], []), "");
  assert.deepEqual(knowledgeContextFor({ hits: [] }, []), { context: "", citations: [] });
});

test("上下文包含卡片路径与正文", () => {
  const text = buildKnowledgeContext([hit("知识库/概念/频次.md")], [note("知识库/概念/频次.md", "每季度一次。")]);
  assert.match(text, /知识库\/概念\/频次\.md/);
  assert.match(text, /每季度一次/);
});

test("上下文不超过 3000 字并显式截断", () => {
  const text = buildKnowledgeContext([hit("知识库/概念/长.md")], [note("知识库/概念/长.md", "长".repeat(5000))]);
  assert.ok(text.length <= 3000);
  assert.match(text, /已按上限截断/);
});

test("引用按命中顺序生成且不超过两张", () => {
  const citations = citationsFromHits([hit("a.md"), hit("b.md"), hit("c.md")], 2);
  assert.deepEqual(citations.map((c) => c.relative_path), ["a.md", "b.md"]);
  assert.equal(citations[0].title, "a");
});

test("整句问话抽成可命中的知识词", () => {
  assert.equal(knowledgeSearchKeyword("专项检查的流程是什么"), "专项检查");
  assert.equal(knowledgeSearchKeyword("请 @知识库 说明专项检查"), "专项检查");
  assert.equal(knowledgeSearchKeyword("@知识库"), "");
  assert.equal(knowledgeSearchKeyword("今天天气怎么样"), "今天天气怎么样");
});

test("只保留 AI 知识卡，不注入 inbox 或索引页", () => {
  const hits = [hit("知识库/概念/频次.md"), hit("inbox/原始材料.md"), hit("知识库/索引.md")];
  assert.deepEqual(filterKnowledgeHits(hits).map((h) => h.relative_path), ["知识库/概念/频次.md"]);
  assert.equal(buildKnowledgeContext(hits.slice(1), []), "");
});

test("知识上下文只引用前两张卡", () => {
  const hits = [hit("知识库/概念/a.md"), hit("知识库/方法/b.md"), hit("知识库/概念/c.md")];
  const result = knowledgeContextFor({ hits }, []);
  assert.equal(result.citations.length, 2);
  assert.deepEqual(result.citations.map((c) => c.relative_path), ["知识库/概念/a.md", "知识库/方法/b.md"]);
});
