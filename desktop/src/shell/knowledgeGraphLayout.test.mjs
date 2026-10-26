/**
 * P4-S02 / P4-S04 布局层单测：node --experimental-strip-types --test knowledgeGraphLayout.test.mjs
 *
 * 布局必须是确定的纯函数：同样输入必得同样坐标，坐标不出画布，迭代次数有上限。
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  computeLayout,
  autoIterations,
  coreNodeIds,
  foldedNodeIds,
  foldedPositions,
  initialPositions,
  radiusOf,
  stableHash,
  DEFAULT_ITERATIONS,
  MAX_FULL_LAYOUT_NODES,
  NODE_RADIUS,
  NODE_RADIUS_HUB,
  DEFAULT_HUB_THRESHOLD,
} from "./knowledgeGraphLayout.ts";

function node(id, backlinkCount = 0) {
  return {
    id,
    kind: "note",
    label: id.slice(id.lastIndexOf("/") + 1),
    path: id,
    directory: id.includes("/") ? id.slice(0, id.lastIndexOf("/")) : "",
    tags: [],
    aliases: [],
    backlinkCount,
    outlinkCount: 0,
    orphan: backlinkCount === 0,
    candidates: [],
  };
}

function edge(source, target, id = source + "->" + target) {
  return {
    id,
    source,
    target,
    status: "resolved",
    kind: "wikilink",
    anchor: null,
    blockAnchor: false,
    line: 1,
    candidates: [],
  };
}

function chain(count, prefix = "n") {
  const nodes = [];
  const edges = [];
  for (let i = 0; i < count; i += 1) nodes.push(node(prefix + i + ".md", i));
  for (let i = 1; i < count; i += 1) edges.push(edge(prefix + i + ".md", prefix + (i - 1) + ".md"));
  return { nodes, edges };
}

function inBounds(position, width, height, padding) {
  return (
    position.x >= padding - 0.001 &&
    position.x <= width - padding + 0.001 &&
    position.y >= padding - 0.001 &&
    position.y <= height - padding + 0.001
  );
}

test("stableHash 对同一输入稳定且落在 32 位无符号范围", () => {
  assert.equal(stableHash("notes/a.md"), stableHash("notes/a.md"));
  assert.notEqual(stableHash("notes/a.md"), stableHash("notes/b.md"));
  for (const value of ["", "a", "目录/笔记.md", "x".repeat(300)]) {
    const hash = stableHash(value);
    assert.ok(Number.isInteger(hash) && hash >= 0 && hash <= 0xffffffff);
  }
});

test("initialPositions 确定性、每点唯一、且不越界", () => {
  const { nodes } = chain(9);
  const a = initialPositions(nodes, 900, 600);
  const b = initialPositions(nodes, 900, 600);
  assert.equal(a.size, nodes.length);
  for (const n of nodes) {
    assert.deepEqual(a.get(n.id), b.get(n.id));
  }
  const seen = new Set([...a.values()].map((p) => p.x.toFixed(6) + "," + p.y.toFixed(6)));
  assert.equal(seen.size, nodes.length);
  for (const point of a.values()) assert.ok(inBounds(point, 900, 600, 0));
});

test("computeLayout 同样输入必得同样坐标（不含随机数）", () => {
  const { nodes, edges } = chain(12);
  const a = computeLayout(nodes, edges, { width: 960, height: 640 });
  const b = computeLayout(nodes, edges, { width: 960, height: 640 });
  assert.equal(a.iterations, b.iterations);
  assert.equal(a.simplified, b.simplified);
  assert.equal(a.positions.size, nodes.length);
  for (const n of nodes) assert.deepEqual(a.positions.get(n.id), b.positions.get(n.id));
});

test("默认迭代次数固定，且不超过节点规模对应的预算", () => {
  const { nodes, edges } = chain(6);
  assert.equal(computeLayout(nodes, edges, { width: 800, height: 600 }).iterations, DEFAULT_ITERATIONS);
  const capped = computeLayout(nodes, edges, { width: 800, height: 600, iterations: 5000 });
  assert.equal(capped.iterations, autoIterations(6));
  const zero = computeLayout(nodes, edges, { width: 800, height: 600, iterations: 0 });
  assert.equal(zero.iterations, 0);
  const half = computeLayout(nodes, edges, { width: 800, height: 600, iterations: 10 });
  assert.equal(half.iterations, 10);
});

test("迭代预算随规模下降，且始终有界", () => {
  assert.equal(autoIterations(10), DEFAULT_ITERATIONS);
  assert.ok(autoIterations(300) < autoIterations(200));
  assert.ok(autoIterations(600) < autoIterations(300));
  assert.ok(autoIterations(2000) > 0);
  for (const count of [0, 1, 250, 450, 600, 2000]) {
    assert.ok(autoIterations(count) <= DEFAULT_ITERATIONS);
  }
});

test("所有布局坐标都在画布内（含无边的孤立点）", () => {
  const nodes = chain(10).nodes;
  const withIsolates = [...nodes, node("lone/孤岛.md", 0), node("another.md", 0)];
  const layout = computeLayout(withIsolates, chain(10).edges, { width: 720, height: 520 });
  for (const n of withIsolates) {
    const point = layout.positions.get(n.id);
    assert.ok(point, "每个节点都要有坐标：" + n.id);
    assert.ok(inBounds(point, 720, 520, 20), "越界：" + n.id + " -> " + JSON.stringify(point));
  }
});

test("空图与单节点不抛错、不迭代", () => {
  const empty = computeLayout([], [], { width: 400, height: 300 });
  assert.equal(empty.positions.size, 0);
  assert.equal(empty.iterations, 0);
  assert.equal(empty.simplified, false);

  const single = computeLayout([node("only.md")], [], { width: 400, height: 300 });
  assert.equal(single.positions.size, 1);
  assert.equal(single.iterations, 0);
  assert.ok(inBounds(single.positions.get("only.md"), 400, 300, 20));
});

test("极小画布被夹到最小尺寸，坐标仍有限", () => {
  const { nodes, edges } = chain(4);
  const layout = computeLayout(nodes, edges, { width: 1, height: 1, iterations: 5 });
  for (const n of nodes) {
    const point = layout.positions.get(n.id);
    assert.ok(Number.isFinite(point.x) && Number.isFinite(point.y));
  }
});

test("coreNodeIds 按入链降序、同数按路径升序，且可截断", () => {
  const nodes = [node("b.md", 1), node("a.md", 5), node("c.md", 1), node("d.md", 0)];
  assert.deepEqual([...coreNodeIds(nodes, 2)], ["a.md", "b.md"]);
  assert.deepEqual([...coreNodeIds(nodes, 99)].length, 4);
  assert.equal(coreNodeIds(nodes, 0).size, 0);
});

test("超过节点阈值时简化：只布局核心，其余折叠计数正确", () => {
  const total = MAX_FULL_LAYOUT_NODES + 40;
  const nodes = [];
  for (let i = 0; i < total; i += 1) nodes.push(node("v/" + String(i).padStart(4, "0") + ".md", total - i));
  const edges = [edge(nodes[1].id, nodes[0].id)];
  const layout = computeLayout(nodes, edges, { width: 1000, height: 700, iterations: 30 });
  assert.equal(layout.simplified, true);
  assert.equal(layout.positions.size, MAX_FULL_LAYOUT_NODES);
  assert.equal(layout.folded, 40);
  // 入链最高的节点必须在核心集里。
  assert.ok(layout.positions.has(nodes[0].id));
  // 被折叠的网络里，入链最低的那些不参与力导向。
  assert.equal(layout.positions.has(nodes[total - 1].id), false);
});

test("恰好等于阈值的规模不简化", () => {
  const nodes = [];
  for (let i = 0; i < MAX_FULL_LAYOUT_NODES; i += 1) nodes.push(node("m/" + i + ".md", 0));
  const layout = computeLayout(nodes, [], { width: 900, height: 640, iterations: 1 });
  assert.equal(layout.simplified, false);
  assert.equal(layout.folded, 0);
  assert.equal(layout.positions.size, MAX_FULL_LAYOUT_NODES);
});

test("简化布局的核心坐标收在内圈，给外围折叠点留空间", () => {
  const total = MAX_FULL_LAYOUT_NODES + 10;
  const nodes = [];
  for (let i = 0; i < total; i += 1) nodes.push(node("s/" + i + ".md", i));
  const layout = computeLayout(nodes, [], { width: 1000, height: 700, iterations: 20 });
  const cx = 500;
  const cy = 350;
  for (const point of layout.positions.values()) {
    const distance = Math.hypot(point.x - cx, point.y - cy);
    assert.ok(distance <= Math.hypot(cx, cy), "核心点应收在内圈");
  }
});

test("foldedPositions 沿外围均匀分布且确定", () => {
  const folded = new Set(["z.md", "a.md", "m.md"]);
  const a = foldedPositions(folded, 800, 600);
  const b = foldedPositions(folded, 800, 600);
  assert.equal(a.size, 3);
  assert.deepEqual([...a.entries()], [...b.entries()]);
  for (const point of a.values()) assert.ok(inBounds(point, 800, 600, 0));
});

test("折叠集合只包含当前视图节点，不混入筛选隐藏的节点", () => {
  const visible = [node("a.md", 3), node("b.md", 2), node("c.md", 1), node("d.md", 0)];
  const hidden = [node("hidden-1.md", 9), node("hidden-2.md", 8)];
  const layout = computeLayout(visible, [], { width: 900, height: 640, maxFullNodes: 2, iterations: 1 });
  const folded = foldedNodeIds(visible, layout.coreNodeIds);
  assert.deepEqual(folded, ["c.md", "d.md"]);
  assert.equal(folded.length, layout.folded);
  assert.deepEqual([...new Set([...layout.coreNodeIds, ...folded])].sort(), visible.map((n) => n.id).sort());
  assert.ok(folded.every((id) => !hidden.some((h) => h.id === id)));
});

test("布局预算可逐批提高，全部展开后不再简化", () => {
  const nodes = [];
  for (let i = 0; i < 700; i += 1) nodes.push(node("b/" + String(i).padStart(4, "0") + ".md", i));
  const budget = computeLayout(nodes, [], { width: 900, height: 640, maxFullNodes: 600, iterations: 1 });
  assert.equal(budget.simplified, true);
  assert.equal(budget.folded, 100);
  const all = computeLayout(nodes, [], { width: 900, height: 640, maxFullNodes: 1200, iterations: 1 });
  assert.equal(all.simplified, false);
  assert.equal(all.folded, 0);
  assert.equal(all.positions.size, 700);
});

test("radiusOf 按入链数区分枢纽", () => {
  assert.equal(radiusOf(node("x.md", 0)), NODE_RADIUS);
  assert.ok(radiusOf(node("x.md", 1)) > NODE_RADIUS);
  assert.equal(radiusOf(node("x.md", DEFAULT_HUB_THRESHOLD)), NODE_RADIUS_HUB);
  assert.equal(radiusOf(node("x.md", 50)), NODE_RADIUS_HUB);
});
