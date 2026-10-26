/**
 * P4-S01 图谱转换层单测：node --experimental-strip-types --test knowledgeGraph.test.mjs
 *
 * 夹具是手写的索引快照字面量，不读磁盘、不依赖 Tauri；断言全部对照索引事实。
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  buildKnowledgeGraph,
  graphMatchesIndex,
  labelOf,
  directoryOf,
  normalizeTag,
  selectGraphView,
  virtualPathOf,
  MAX_LOCAL_DEPTH,
} from "./knowledgeGraph.ts";

function note(relativePath, extra = {}) {
  return {
    relative_path: relativePath,
    file_name: relativePath.slice(relativePath.lastIndexOf("/") + 1),
    sha256: "sha-" + relativePath,
    bytes: 10,
    backlink_count: 0,
    outlink_count: 0,
    orphan: true,
    lines: 1,
    tags: [],
    aliases: [],
    ...extra,
  };
}

function link(id, sourcePath, targetPath, extra = {}) {
  return {
    id,
    source_path: sourcePath,
    kind: "wikilink",
    target_raw: (targetPath ?? "").replace(/\.md$/, ""),
    display: null,
    anchor: null,
    block_anchor: false,
    status: targetPath ? "resolved" : "unresolved",
    target_path: targetPath ?? null,
    attachment_path: null,
    candidates: [],
    start_byte: 0,
    end_byte: 1,
    line: 1,
    context: "",
    anchor_status: null,
    anchor_line: null,
    ...extra,
  };
}

/** 一份覆盖全部状态的小快照。 */
function fixture() {
  const notes = [
    note("Home.md", { backlink_count: 2, outlink_count: 3, orphan: false, tags: ["index", "核心"] }),
    note("notes/Alpha.md", { backlink_count: 1, outlink_count: 1, orphan: false, tags: ["core"], aliases: ["A"] }),
    note("Beta.md", { backlink_count: 1, outlink_count: 0, orphan: false }),
    note("notes/Orphan.md", { orphan: true }),
  ];
  const links = [
    link("l1", "Home.md", "notes/Alpha.md", { anchor: "details", anchor_status: "exact", anchor_line: 3, line: 5 }),
    link("l2", "Home.md", "Beta.md"),
    link("l3", "Home.md", "notes/Alpha.md", { kind: "embed", line: 7 }),
    link("l4", "notes/Alpha.md", "Home.md", { block_anchor: true, anchor: "blk-1" }),
    link("l5", "Beta.md", "Home.md"),
    link("l6", "Home.md", null, {
      status: "unresolved",
      target_raw: "MissingNote",
      line: 9,
      context: "[[MissingNote]]",
    }),
    link("l7", "Home.md", null, {
      status: "ambiguous",
      target_raw: "Duplicated",
      line: 11,
      candidates: ["Duplicated.md", "notes/Duplicated.md"],
    }),
    link("l8", "Home.md", null, { status: "external", target_raw: "https://example.com", line: 13 }),
    link("l9", "Home.md", null, {
      status: "attachment",
      target_raw: "assets/a.png",
      attachment_path: "assets/a.png",
      line: 15,
    }),
  ];
  const stats = {
    notes: notes.length,
    notes_scanned: notes.length,
    bytes_read: 40,
    budget_bytes: 1024,
    links: links.length,
    resolved: 5,
    unresolved: 1,
    ambiguous: 1,
    external: 1,
    attachments: 1,
    orphans: 1,
    warnings: 0,
  };
  return {
    generation: 3,
    built_at: 100,
    root: "/tmp/vault",
    complete: true,
    incomplete_reason: null,
    notes,
    links,
    issues: [],
    stats,
    fingerprint: "fp",
  };
}

test("节点与边数量与索引 stats 逐项一致", () => {
  const index = fixture();
  const graph = buildKnowledgeGraph(index);
  assert.equal(graph.stats.notes, index.stats.notes);
  assert.equal(graph.stats.resolvedEdges, index.stats.resolved);
  assert.equal(graph.stats.orphans, index.stats.orphans);
  assert.equal(graph.stats.unresolved, index.stats.unresolved);
  assert.equal(graph.stats.ambiguous, index.stats.ambiguous);
  assert.equal(graph.stats.placeholders, 2);
  assert.equal(graph.stats.placeholderEdges, 2);
  assert.equal(graph.stats.danglingResolved, 0);
  assert.equal(graph.generation, 3);
  assert.ok(graphMatchesIndex(graph, index));
});

test("external 与 attachment 不入图", () => {
  const graph = buildKnowledgeGraph(fixture());
  const ids = new Set(graph.nodes.map((node) => node.id));
  assert.ok(![...ids].some((id) => id.includes("example.com")));
  assert.ok(![...ids].some((id) => id.includes("assets/a.png")));
  assert.equal(graph.edges.length, 7);
});

test("未解析与歧义生成可区分的虚节点和虚边", () => {
  const graph = buildKnowledgeGraph(fixture());
  const unresolved = graph.nodes.filter((node) => node.kind === "unresolved");
  const ambiguous = graph.nodes.filter((node) => node.kind === "ambiguous");
  assert.equal(unresolved.length, 1);
  assert.equal(ambiguous.length, 1);
  assert.equal(unresolved[0].path, null);
  assert.equal(unresolved[0].label, "MissingNote");
  assert.deepEqual(ambiguous[0].candidates, ["Duplicated.md", "notes/Duplicated.md"]);
  const placeholderEdge = graph.edges.find((edge) => edge.status === "ambiguous");
  assert.ok(placeholderEdge);
  assert.equal(placeholderEdge.target, ambiguous[0].id);
  assert.deepEqual(placeholderEdge.candidates, ["Duplicated.md", "notes/Duplicated.md"]);
});

test("虚节点标识不会与任何笔记路径冲突", () => {
  const graph = buildKnowledgeGraph(fixture());
  const noteIds = new Set(graph.nodes.filter((n) => n.kind === "note").map((n) => n.id));
  for (const node of graph.nodes.filter((n) => n.kind !== "note")) {
    assert.ok(!noteIds.has(node.id));
    assert.ok(node.id.startsWith("#virtual#"));
  }
});

test("边保留方向、锚点、类型与行号", () => {
  const graph = buildKnowledgeGraph(fixture());
  const anchored = graph.edges.find((edge) => edge.id === "l1");
  assert.equal(anchored.source, "Home.md");
  assert.equal(anchored.target, "notes/Alpha.md");
  assert.equal(anchored.anchor, "details");
  assert.equal(anchored.line, 5);
  const embed = graph.edges.find((edge) => edge.id === "l3");
  assert.equal(embed.kind, "embed");
  const block = graph.edges.find((edge) => edge.id === "l4");
  assert.equal(block.blockAnchor, true);
  assert.equal(block.source, "notes/Alpha.md");
  assert.equal(block.target, "Home.md");
});

test("孤立清单与索引 orphan 标记一致", () => {
  const graph = buildKnowledgeGraph(fixture());
  assert.deepEqual(graph.health.orphans, ["notes/Orphan.md"]);
  assert.equal(graph.nodes.find((n) => n.id === "notes/Orphan.md").orphan, true);
  assert.equal(graph.nodes.find((n) => n.id === "Home.md").orphan, false);
});

test("高连接度顺序按入链降序、同数按路径升序", () => {
  const graph = buildKnowledgeGraph(fixture());
  assert.deepEqual(graph.health.hubPaths, ["Home.md", "Beta.md", "notes/Alpha.md"]);
});

test("节点带标签、别名与目录", () => {
  const graph = buildKnowledgeGraph(fixture());
  const alpha = graph.nodes.find((n) => n.id === "notes/Alpha.md");
  assert.deepEqual(alpha.tags, ["core"]);
  assert.deepEqual(alpha.aliases, ["A"]);
  assert.equal(alpha.directory, "notes");
  assert.equal(alpha.label, "Alpha");
  assert.deepEqual(graph.tags, ["core", "index", "核心"]);
  assert.deepEqual(graph.directories, ["", "notes"]);
});

test("resolved 目标不在笔记集合时计为悬空且不造边", () => {
  const index = fixture();
  index.links.push(link("l10", "Home.md", "Missing.md"));
  index.stats.resolved += 1;
  const graph = buildKnowledgeGraph(index);
  assert.equal(graph.stats.danglingResolved, 1);
  assert.equal(graph.edges.filter((e) => e.id === "l10").length, 0);
  assert.equal(graphMatchesIndex(graph, index), false);
});

test("转换是纯函数：同输入同输出且不改入参", () => {
  const index = fixture();
  const snapshot = JSON.stringify(index);
  const first = JSON.stringify(buildKnowledgeGraph(index));
  const second = JSON.stringify(buildKnowledgeGraph(index));
  assert.equal(first, second);
  assert.equal(JSON.stringify(index), snapshot);
});

test("全局视图保留全部节点与边", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { showPlaceholders: true });
  assert.equal(view.nodes.length, graph.nodes.length);
  assert.equal(view.edges.length, graph.edges.length);
  assert.equal(view.hiddenEdges, 0);
  assert.equal(view.noteCount, 4);
  assert.equal(view.placeholderCount, 2);
  assert.equal(view.centerFilteredOut, false);
});

test("默认不画虚节点时虚边一并消失", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph);
  assert.equal(view.nodes.length, 4);
  assert.equal(view.edges.length, 5);
  assert.equal(view.placeholderCount, 0);
});

test("来源笔记被筛掉时，虚节点不单独留在图里", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { directory: "notes", showPlaceholders: true });
  assert.deepEqual(view.nodes.map((n) => n.id).sort(), ["notes/Alpha.md", "notes/Orphan.md"]);
  assert.equal(view.placeholderCount, 0);
  assert.ok(view.edges.every((edge) => edge.status === "resolved"));
  // hiddenEdges 是所有被筛掉的边（9 条入图边里只剩 2 条）。
  assert.equal(view.hiddenEdges, graph.edges.length - view.edges.length);
  assert.equal(view.hiddenEdges, 7);
});

test("局部视图默认 1 度，以中心笔记为起点", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, {}, { centerPath: "Beta.md", depth: 1 });
  const ids = view.nodes.map((node) => node.id).sort();
  assert.deepEqual(ids, ["Beta.md", "Home.md"]);
});

test("局部视图 2 度扩到邻居的邻居", () => {
  const graph = buildKnowledgeGraph(fixture());
  const one = selectGraphView(graph, {}, { centerPath: "Beta.md", depth: 1 });
  const two = selectGraphView(graph, {}, { centerPath: "Beta.md", depth: 2 });
  assert.equal(one.noteCount, 2);
  assert.equal(two.noteCount, 3);
  assert.ok(two.nodes.some((node) => node.id === "notes/Alpha.md"));
});

test("度数被夹在 1..MAX_LOCAL_DEPTH", () => {
  const graph = buildKnowledgeGraph(fixture());
  const zero = selectGraphView(graph, {}, { centerPath: "Beta.md", depth: 0 });
  const huge = selectGraphView(graph, {}, { centerPath: "Beta.md", depth: 99 });
  assert.ok(zero.nodes.length >= 2);
  assert.equal(MAX_LOCAL_DEPTH, 3);
  assert.ok(huge.nodes.length <= graph.nodes.length);
});

test("目录筛选只留该目录笔记，边随之收敛", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { directory: "notes" });
  assert.deepEqual(view.nodes.map((n) => n.id).sort(), ["notes/Alpha.md", "notes/Orphan.md"]);
  assert.equal(view.edges.length, 0);
  assert.ok(view.hiddenEdges > 0);
});

test("标签筛选忽略大小写与前置井号", () => {
  const graph = buildKnowledgeGraph(fixture());
  const lower = selectGraphView(graph, { tag: "core" });
  const upper = selectGraphView(graph, { tag: " #CORE " });
  assert.deepEqual(lower.nodes.map((n) => n.id), upper.nodes.map((n) => n.id));
  assert.deepEqual(lower.nodes.map((n) => n.id), ["notes/Alpha.md"]);
  assert.equal(normalizeTag(" #CORE "), "core");
});

test("孤立筛选与索引孤立清单一致", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { orphansOnly: true });
  assert.deepEqual(view.nodes.map((n) => n.id), graph.health.orphans);
});

test("高连接度筛选按入链阈值", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { minBacklinks: 2 });
  assert.deepEqual(view.nodes.map((n) => n.id), ["Home.md"]);
});

test("问题引用筛选只留带未解析或歧义的来源笔记", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { problematicOnly: true });
  assert.deepEqual(view.nodes.map((n) => n.id), ["Home.md"]);
});

test("中心笔记被筛掉时给出明确标志而不是空图", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, { tag: "core" }, { centerPath: "Home.md" });
  assert.equal(view.centerFilteredOut, true);
  // Home 自己没有 core 标签，但一度邻域里的 Alpha 有：中心被筛掉、邻域仍有内容。
  assert.equal(view.noteCount, 1);
  assert.deepEqual(
    view.nodes.map((node) => node.id),
    ["notes/Alpha.md"],
  );
  assert.equal(view.edges.length, 0);
});

test("未知中心路径返回空视图并标记 centerMissing，不冒充全局图", () => {
  const graph = buildKnowledgeGraph(fixture());
  const view = selectGraphView(graph, {}, { centerPath: "NotExist.md" });
  assert.equal(view.centerMissing, true);
  assert.equal(view.centerFilteredOut, false);
  assert.equal(view.noteCount, 0);
  assert.equal(view.placeholderCount, 0);
  assert.equal(view.nodes.length, 0);
  assert.equal(view.edges.length, 0);
});

test("已知中心不设 centerMissing，全局视图也不设", () => {
  const graph = buildKnowledgeGraph(fixture());
  assert.equal(selectGraphView(graph, {}, { centerPath: "Beta.md" }).centerMissing, false);
  assert.equal(selectGraphView(graph, {}, {}).centerMissing, false);
});

test("labelOf 与 directoryOf 的边界", () => {
  assert.equal(labelOf("A.md"), "A");
  assert.equal(labelOf("a/b.c.md"), "b.c");
  assert.equal(labelOf(".hidden"), ".hidden");
  assert.equal(directoryOf("A.md"), "");
  assert.equal(directoryOf("a/b/A.md"), "a/b");
});

test("virtualPathOf 含状态与引用 id，稳定可复现", () => {
  const index = fixture();
  const missing = index.links.find((l) => l.id === "l6");
  assert.equal(virtualPathOf(missing), "#virtual#unresolved#l6");
});
