/** OD-KB-201 S5：知识冲突构建纯函数单测。 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { buildKnowledgeConflicts } from "./shell/knowledgeHub.ts";

const file = (path, kind = "note") => ({
  relative_path: path,
  file_name: path.split("/").pop(),
  extension: path.split(".").pop(),
  bytes: 10,
  modified: null,
  kind,
});

const index = (links, issues = []) => ({
  generation: 1,
  built_at: 1,
  root: "/vault",
  complete: true,
  incomplete_reason: null,
  notes: [],
  links,
  issues,
  stats: {},
  fingerprint: "f",
});

const link = (source, target, status = "unresolved") => ({
  id: "l",
  source_path: source,
  kind: "link",
  target_raw: target,
  display: null,
  anchor: null,
  block_anchor: false,
  status,
  target_path: null,
  attachment_path: null,
  candidates: [],
  start_byte: 0,
  end_byte: 1,
  line: 1,
  context: "",
  anchor_status: null,
  anchor_line: null,
});

test("索引.md 指向 raw/ 的未解析链接不算知识冲突", () => {
  const conflicts = buildKnowledgeConflicts(
    { files: [], skipped: [], errors: [], stats: {} },
    index([link("知识库/索引.md", "raw/old.pdf")]),
    [],
  );
  assert.equal(conflicts.length, 0);
});

test("普通未解析链接仍进入冲突队列", () => {
  const conflicts = buildKnowledgeConflicts(
    { files: [], skipped: [], errors: [], stats: {} },
    index([link("知识库/概念/甲.md", "概念/不存在.md")]),
    [],
  );
  assert.equal(conflicts.length, 1);
  assert.equal(conflicts[0].kind, "link");
});

test("同名条目产生冲突，已有 resolved 记录后不再显示", () => {
  const scan = {
    files: [file("知识库/概念/甲/同名.md"), file("知识库/方法/乙/同名.md")],
    skipped: [],
    errors: [],
    stats: {},
  };
  const key = "conflict:duplicate:同名.md";
  assert.equal(buildKnowledgeConflicts(scan, null, []).length, 1);
  assert.equal(
    buildKnowledgeConflicts(scan, null, [
      { at: "now", action: "resolve_conflict", subject: key, sha256: "-", note: "resolved；已合并" },
    ]).length,
    0,
  );
});
