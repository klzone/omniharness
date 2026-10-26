/**
 * P4 性能基准（OD-KB-001 P4-S04）。
 *
 * 只用合成索引快照，不读任何真实知识库。度量转换 + 首次布局的耗时，
 * 并核对 2000 篇规模下是否按登记阈值降级。
 *
 * 用法：node --experimental-strip-types bench/knowledge-graph-p4.mjs
 * 结果打印为 JSON 行，由 verify_knowledge_vault_p4.py 收集进证据。
 */

import { performance } from "node:perf_hooks";

import { buildKnowledgeGraph, selectGraphView } from "../src/shell/knowledgeGraph.ts";
import { computeLayout } from "../src/shell/knowledgeGraphLayout.ts";

const WIDTH = 1100;
const HEIGHT = 720;

function syntheticIndex(noteCount, edgeCount) {
  const notes = [];
  for (let i = 0; i < noteCount; i += 1) {
    const directory = "d" + (i % 12);
    const path = directory + "/note-" + String(i).padStart(4, "0") + ".md";
    notes.push({
      relative_path: path,
      file_name: "note-" + String(i).padStart(4, "0") + ".md",
      sha256: "sha-" + i,
      bytes: 400,
      backlink_count: 0,
      outlink_count: 0,
      orphan: true,
      lines: 12,
      tags: ["t" + (i % 7)],
      aliases: [],
    });
  }
  const links = [];
  for (let i = 0; i < edgeCount; i += 1) {
    const source = notes[i % noteCount];
    const target = notes[(i * 7 + 3) % noteCount];
    links.push({
      id: "link-" + i,
      source_path: source.relative_path,
      kind: "wikilink",
      target_raw: target.relative_path.replace(/\.md$/, ""),
      display: null,
      anchor: null,
      block_anchor: false,
      status: "resolved",
      target_path: target.relative_path,
      attachment_path: null,
      candidates: [],
      start_byte: 0,
      end_byte: 10,
      line: (i % 12) + 1,
      context: "",
      anchor_status: null,
      anchor_line: null,
    });
  }
  // 统计必须与转换层自检口径一致：入链/出链/孤立由引用重新数一遍。
  const inbound = new Map();
  const outbound = new Map();
  for (const link of links) {
    if (link.status !== "resolved") continue;
    inbound.set(link.target_path, (inbound.get(link.target_path) ?? 0) + 1);
    outbound.set(link.source_path, (outbound.get(link.source_path) ?? 0) + 1);
  }
  for (const note of notes) {
    note.backlink_count = inbound.get(note.relative_path) ?? 0;
    note.outlink_count = outbound.get(note.relative_path) ?? 0;
    note.orphan = note.backlink_count === 0;
  }
  const orphans = notes.filter((note) => note.orphan).length;
  return {
    generation: 7,
    built_at: 0,
    root: "/tmp/synthetic",
    complete: true,
    incomplete_reason: null,
    notes,
    links,
    issues: [],
    stats: {
      notes: notes.length,
      notes_scanned: notes.length,
      bytes_read: notes.length * 400,
      budget_bytes: 32 * 1024 * 1024,
      links: links.length,
      resolved: links.length,
      unresolved: 0,
      ambiguous: 0,
      external: 0,
      attachments: 0,
      orphans,
      warnings: 0,
    },
    fingerprint: "synthetic",
  };
}

function measure(noteCount, edgeCount) {
  const index = syntheticIndex(noteCount, edgeCount);

  const t0 = performance.now();
  const graph = buildKnowledgeGraph(index);
  const convertMs = performance.now() - t0;

  const t1 = performance.now();
  const view = selectGraphView(graph, { showPlaceholders: true }, {});
  const filterMs = performance.now() - t1;

  const t2 = performance.now();
  const layout = computeLayout(view.nodes, view.edges, { width: WIDTH, height: HEIGHT });
  const layoutMs = performance.now() - t2;

  const t3 = performance.now();
  const local = selectGraphView(
    graph,
    { showPlaceholders: true },
    { centerPath: graph.nodes[0].id, depth: 2 },
  );
  const localLayout = computeLayout(local.nodes, local.edges, { width: WIDTH, height: HEIGHT });
  const localMs = performance.now() - t3;

  const t4 = performance.now();
  const again = computeLayout(view.nodes, view.edges, { width: WIDTH, height: HEIGHT });
  const relayoutMs = performance.now() - t4;
  let stable = again.positions.size === layout.positions.size;
  for (const [id, point] of layout.positions) {
    const other = again.positions.get(id);
    if (!other || other.x !== point.x || other.y !== point.y) {
      stable = false;
      break;
    }
  }

  return {
    notes: noteCount,
    edges: edgeCount,
    convertMs: Number(convertMs.toFixed(1)),
    filterMs: Number(filterMs.toFixed(1)),
    layoutMs: Number(layoutMs.toFixed(1)),
    firstViewMs: Number((convertMs + filterMs + layoutMs).toFixed(1)),
    relayoutMs: Number(relayoutMs.toFixed(1)),
    localViewMs: Number(localMs.toFixed(1)),
    localNodes: local.nodes.length,
    iterations: layout.iterations,
    placed: layout.positions.size,
    simplified: layout.simplified,
    folded: layout.folded,
    stable,
    graphStats: graph.stats,
    edgeCount: graph.edges.length,
  };
}

const cases = [
  [200, 500],
  [1000, 2500],
  [2000, 5000],
];
for (const [notes, edges] of cases) {
  // 预热一轮，避免把 JIT 首次编译算进结论。
  measure(notes, edges);
  console.log(JSON.stringify({ case: notes + "notes", ...measure(notes, edges) }));
}

