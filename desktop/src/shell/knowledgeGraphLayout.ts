/**
 * 图谱布局（OD-KB-001 P4-S02 / P4-S04）。
 *
 * 布局是纯函数：由节点与边算出坐标，不使用随机数，因此同一快照每次得到同样的图，
 * 单测与窗口验证都能复现。力导向模拟有固定迭代上限，超过就停在当前状态，
 * 不会让主线程无限计算。
 *
 * 降级：节点超过 MAX_FULL_LAYOUT_NODES 时只把「核心节点」（入链多的）做力导向布局，
 * 其余节点折叠到外围并计数。界面据此提示「已简化显示」。
 */

import type { GraphEdge, GraphNode } from "./knowledgeGraph";

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface LayoutOptions {
  width: number;
  height: number;
  /** 力导向迭代上限（还会被节点规模再压一次）。 */
  iterations?: number;
  /** 节点数超过该值时启用简化布局。 */
  maxFullNodes?: number;
}

export interface GraphLayout {
  positions: Map<string, LayoutPoint>;
  /** 实际使用的迭代次数。 */
  iterations: number;
  /** 是否简化了布局（节点过多）。 */
  simplified: boolean;
  /** 简化时被折叠掉的节点数。 */
  folded: number;
  /** 参与力导向布局的节点 id；简化时即核心集。 */
  coreNodeIds: string[];
}

/** 力导向默认迭代轮数；小图用它。 */
export const DEFAULT_ITERATIONS = 220;
/** 超过这个节点数就简化布局。 */
export const MAX_FULL_LAYOUT_NODES = 600;

/** 高连接度节点的默认入链阈值。 */
export const DEFAULT_HUB_THRESHOLD = 2;
export const NODE_RADIUS = 7;
export const NODE_RADIUS_HUB = 11;
export const LAYOUT_PADDING = 36;

/**
 * 规模越大，迭代轮数越少：每轮成本随节点与边数增长，仍要给 2000 篇级的
 * 首次渲染留出预算。轮数只取决于节点数，因此布局仍是确定性的。
 */
export function autoIterations(nodeCount: number): number {
  if (nodeCount <= 250) return DEFAULT_ITERATIONS;
  if (nodeCount <= 450) return 150;
  return 90;
}

/** 稳定字符串散列，用于确定性初始摆放（不用随机数）。 */
export function stableHash(value: string): number {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

/**
 * 确定性初始位置：按 id 排序后分布到两层同心圆，再加一点散列抖动。
 * 用圆环而不是随机点，是为了让首帧就有可读结构。
 */
export function initialPositions(
  nodes: GraphNode[],
  width: number,
  height: number,
): Map<string, LayoutPoint> {
  const positions = new Map<string, LayoutPoint>();
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.max(20, Math.min(width, height) / 2 - LAYOUT_PADDING);
  const sorted = [...nodes].sort((a, b) => a.id.localeCompare(b.id));
  const count = Math.max(1, sorted.length);
  for (let index = 0; index < sorted.length; index += 1) {
    const node = sorted[index];
    const ring = index % 2 === 0 ? 1 : 0.62;
    const angle =
      (2 * Math.PI * index) / count + ((stableHash(node.id) % 1000) / 1000) * 0.12;
    positions.set(node.id, {
      x: cx + Math.cos(angle) * radius * ring,
      y: cy + Math.sin(angle) * radius * ring,
    });
  }
  return positions;
}

/** 挑选「核心节点」：入链多的优先，其余按路径，保证确定性。 */
export function coreNodeIds(nodes: GraphNode[], limit: number): Set<string> {
  return new Set(
    [...nodes]
      .sort((a, b) => b.backlinkCount - a.backlinkCount || a.id.localeCompare(b.id))
      .slice(0, Math.max(0, limit))
      .map((node) => node.id),
  );
}

/**
 * 当前视图里被折叠的节点。必须只从视图节点计算：图谱可能先筛选出 800 个节点再简化，
 * 若从完整图的 2000 个节点算，会把筛选隐藏的节点也画成外围提示点。
 */
export function foldedNodeIds(
  nodes: GraphNode[],
  coreNodeIds: Iterable<string>,
): string[] {
  const placed = new Set(coreNodeIds);
  return nodes.filter((node) => !placed.has(node.id)).map((node) => node.id);
}

/** 计算布局。纯函数，无 I/O；同样输入必得同样坐标。 */
export function computeLayout(
  nodes: GraphNode[],
  edges: GraphEdge[],
  options: LayoutOptions,
): GraphLayout {
  const width = Math.max(80, options.width);
  const height = Math.max(80, options.height);
  const maxFull = options.maxFullNodes ?? MAX_FULL_LAYOUT_NODES;

  if (nodes.length === 0) {
    return {
      positions: new Map(),
      iterations: 0,
      simplified: false,
      folded: 0,
      coreNodeIds: [],
    };
  }

  const simplified = nodes.length > maxFull;
  const core = simplified ? coreNodeIds(nodes, maxFull) : null;
  const layoutNodes = core ? nodes.filter((node) => core.has(node.id)) : nodes;
  const positions = initialPositions(layoutNodes, width, height);
  const ids = layoutNodes.map((node) => node.id);

  if (layoutNodes.length <= 1) {
    return {
      positions,
      iterations: 0,
      simplified,
      folded: nodes.length - layoutNodes.length,
      coreNodeIds: ids,
    };
  }

  const requested = options.iterations ?? DEFAULT_ITERATIONS;
  const iterations = Math.max(
    0,
    Math.min(requested, autoIterations(layoutNodes.length), 1000),
  );
  simulate(layoutNodes, edges, positions, width, height, iterations, core !== null);
  return {
    positions,
    iterations,
    simplified,
    folded: nodes.length - layoutNodes.length,
    coreNodeIds: ids,
  };
}

/**
 * 简化版 Fruchterman-Reingold：斥力 + 边引力 + 向心收缩，坐标夹在图幅内。
 * 只做固定轮数，不做收敛判定，迭代上限即成本上限。
 *
 * 斥力用均匀格点做局部近似：每个节点只和相邻 3×3 格里的节点互斥。
 * 这样每轮随节点数近似线性增长，而不是 O(n²)。
 */
function simulate(
  nodes: GraphNode[],
  edges: GraphEdge[],
  positions: Map<string, LayoutPoint>,
  width: number,
  height: number,
  iterations: number,
  shrinkCore: boolean,
): void {
  const count = nodes.length;
  const index = new Map<string, number>();
  nodes.forEach((node, i) => index.set(node.id, i));

  const xs = new Float64Array(count);
  const ys = new Float64Array(count);
  nodes.forEach((node, i) => {
    const point = positions.get(node.id)!;
    xs[i] = point.x;
    ys[i] = point.y;
  });

  const pairs: Array<[number, number]> = [];
  for (const edge of edges) {
    const a = index.get(edge.source);
    const b = index.get(edge.target);
    if (a !== undefined && b !== undefined && a !== b) pairs.push([a, b]);
  }

  const cx = width / 2;
  const cy = height / 2;
  const k = Math.sqrt((width * height) / Math.max(1, count));
  const padding = Math.min(LAYOUT_PADDING, Math.min(width, height) / 6);
  const cell = Math.max(1, k);
  const cols = Math.max(1, Math.ceil(width / cell) + 2);

  const dx = new Float64Array(count);
  const dy = new Float64Array(count);
  const grid = new Map<number, number[]>();
  let temperature = Math.min(width, height) / 8;

  for (let step = 0; step < iterations; step += 1) {
    dx.fill(0);
    dy.fill(0);
    grid.clear();
    for (let i = 0; i < count; i += 1) {
      const key =
        Math.floor((ys[i] + padding) / cell) * cols + Math.floor((xs[i] + padding) / cell);
      const bucket = grid.get(key);
      if (bucket) bucket.push(i);
      else grid.set(key, [i]);
    }

    for (let i = 0; i < count; i += 1) {
      const gx = Math.floor((xs[i] + padding) / cell);
      const gy = Math.floor((ys[i] + padding) / cell);
      for (let oy = -1; oy <= 1; oy += 1) {
        for (let ox = -1; ox <= 1; ox += 1) {
          const bucket = grid.get((gy + oy) * cols + (gx + ox));
          if (!bucket) continue;
          for (const j of bucket) {
            if (j <= i) continue;
            repel(i, j, xs, ys, dx, dy, k);
          }
        }
      }
    }

    for (const [a, b] of pairs) {
      const vx = xs[a] - xs[b];
      const vy = ys[a] - ys[b];
      const dist = Math.hypot(vx, vy);
      if (dist < 0.01) continue;
      const force = (dist * dist) / k;
      const fx = (vx / dist) * force;
      const fy = (vy / dist) * force;
      dx[a] -= fx;
      dy[a] -= fy;
      dx[b] += fx;
      dy[b] += fy;
    }

    // 向心收缩：防止孤立点被斥力推到画布外。
    for (let i = 0; i < count; i += 1) {
      dx[i] += (cx - xs[i]) * 0.06;
      dy[i] += (cy - ys[i]) * 0.06;
    }

    for (let i = 0; i < count; i += 1) {
      const len = Math.hypot(dx[i], dy[i]);
      if (len < 0.0001) continue;
      const limited = Math.min(len, temperature);
      xs[i] = clamp(xs[i] + (dx[i] / len) * limited, padding, width - padding);
      ys[i] = clamp(ys[i] + (dy[i] / len) * limited, padding, height - padding);
    }
    temperature = Math.max(0.6, temperature * 0.985);
  }

  if (shrinkCore) {
    // 简化模式下把核心收进内圈，给外围折叠提示留出空间。
    for (let i = 0; i < count; i += 1) {
      xs[i] = cx + (xs[i] - cx) * 0.62;
      ys[i] = cy + (ys[i] - cy) * 0.62;
    }
  }

  nodes.forEach((node, i) => {
    positions.set(node.id, { x: xs[i], y: ys[i] });
  });
}

function repel(
  i: number,
  j: number,
  xs: Float64Array,
  ys: Float64Array,
  dx: Float64Array,
  dy: Float64Array,
  k: number,
): void {
  let vx = xs[i] - xs[j];
  let vy = ys[i] - ys[j];
  let dist = Math.hypot(vx, vy);
  if (dist < 0.01) {
    // 重合点按索引给一个确定方向的微小分离，避免除零。
    vx = ((i + 1) % 7) * 0.01 - 0.03;
    vy = ((j + 1) % 5) * 0.01 - 0.02;
    dist = Math.hypot(vx, vy) || 0.01;
  }
  const force = (k * k) / dist;
  const fx = (vx / dist) * force;
  const fy = (vy / dist) * force;
  dx[i] += fx;
  dy[i] += fy;
  dx[j] -= fx;
  dy[j] -= fy;
}

function clamp(value: number, min: number, max: number): number {
  if (max <= min) return min;
  return Math.min(max, Math.max(min, value));
}

/** 简化布局里被折叠的节点，沿外围圆环均匀放一圈（仅用作提示点）。 */
export function foldedPositions(
  folded: Set<string>,
  width: number,
  height: number,
): Map<string, LayoutPoint> {
  const out = new Map<string, LayoutPoint>();
  const remaining = [...folded].sort();
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.max(10, Math.min(width, height) / 2 - LAYOUT_PADDING / 2);
  const count = Math.max(1, remaining.length);
  remaining.forEach((id, i) => {
    const angle = (2 * Math.PI * i) / count;
    out.set(id, { x: cx + Math.cos(angle) * radius, y: cy + Math.sin(angle) * radius });
  });
  return out;
}

/** 依据入链数决定半径：高连接度节点更大，便于识别枢纽。 */
export function radiusOf(node: GraphNode): number {
  if (node.backlinkCount >= DEFAULT_HUB_THRESHOLD) return NODE_RADIUS_HUB;
  if (node.backlinkCount > 0) return NODE_RADIUS + 1.5;
  return NODE_RADIUS;
}

