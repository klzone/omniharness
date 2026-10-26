/**
 * 知识图谱数据转换（OD-KB-001 P4-S01）。
 *
 * 唯一事实来源是 P3 的 LinkIndex 内存快照：本模块只做纯转换，不读文件、不发请求，
 * 不缓存到任何存储。索引重建后整张图随之失效并重算，因此不会出现图与链接事实不一致。
 *
 * 转换口径：
 * - 笔记节点 = 索引里的笔记，一一对应，不合并、不推断。
 * - 实边 = status 为 resolved 且目标落在索引内的引用，保留方向、锚点与行号。
 * - unresolved / ambiguous 生成带状态标记的虚节点与其来源边；候选目标是猜测，
 *   因此虚节点永不提供跳转。
 * - external / attachment 不入图：它们不是知识库内的笔记关系。
 */

import type {
  KnowledgeVaultIndex,
  KnowledgeVaultIndexLink,
  KnowledgeVaultIndexNote,
  KnowledgeVaultLinkStatus,
} from "../types";

export type GraphNodeKind = "note" | "unresolved" | "ambiguous";

export interface GraphNode {
  /** 笔记节点用相对路径；虚节点用 virtualPathOf(link) 生成的不冲突标识。 */
  id: string;
  kind: GraphNodeKind;
  /** 笔记节点为去扩展名的文件名；虚节点为未解析/歧义的原始目标。 */
  label: string;
  /** 笔记相对路径；虚节点为 null（它不指向任何可信文件）。 */
  path: string | null;
  /** 所在目录；虚节点为空串。 */
  directory: string;
  tags: string[];
  aliases: string[];
  backlinkCount: number;
  outlinkCount: number;
  orphan: boolean;
  /** 虚节点上记录的歧义候选，按索引给出的顺序。 */
  candidates: string[];
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  /** resolved 为实边；unresolved / ambiguous 为虚边。 */
  status: KnowledgeVaultLinkStatus;
  /** 引用语法类型：wikilink / embed / markdown_link / markdown_image / autolink。 */
  kind: string;
  /** 目标标题锚点（不含 #）；无锚点时为 null。 */
  anchor: string | null;
  blockAnchor: boolean;
  /** 引用在来源笔记里的行号（1 起）。 */
  line: number;
  /** 歧义候选路径；其他状态为空。 */
  candidates: string[];
}

export interface GraphHealthItem {
  id: string;
  status: KnowledgeVaultLinkStatus;
  sourcePath: string;
  targetRaw: string;
  display: string | null;
  line: number;
  context: string;
  candidates: string[];
}

export interface GraphHealth {
  unresolved: GraphHealthItem[];
  ambiguous: GraphHealthItem[];
  /** 没有任何入链的笔记路径，与索引 orphan 标记同源。 */
  orphans: string[];
  /** 入链数最高的笔记，按入链降序、路径升序；用于高连接度视图。 */
  hubPaths: string[];
}

export interface KnowledgeGraph {
  generation: number;
  complete: boolean;
  incompleteReason: string | null;
  /** 索引所属知识库根目录。 */
  root: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  health: GraphHealth;
  /** 全部标签，去重并按字典序。 */
  tags: string[];
  /** 全部目录，去重并按字典序；根目录为空串。 */
  directories: string[];
  stats: {
    /** 笔记节点数，应等于索引 stats.notes。 */
    notes: number;
    /** 虚节点数，应等于索引 unresolved + ambiguous。 */
    placeholders: number;
    /** 实边数，应等于索引 stats.resolved。 */
    resolvedEdges: number;
    /** 虚边数，应等于索引 unresolved + ambiguous。 */
    placeholderEdges: number;
    orphans: number;
    unresolved: number;
    ambiguous: number;
    /** 索引标为 resolved 但目标不在笔记集合内的引用数；正常应为 0。 */
    danglingResolved: number;
    /** 来源笔记不在笔记集合内的边数；正常应为 0。 */
    danglingSource: number;
  };
}

/** 高连接度视图的默认入链阈值。 */
export const DEFAULT_HUB_MIN_BACKLINKS = 2;

/** 类型守卫：把 GraphNode 收窄为笔记节点（path 非空）。 */
export function isNoteNode(node: GraphNode): node is GraphNode & { kind: "note"; path: string } {
  return node.kind === "note" && node.path !== null;
}

/** 目录部分；根目录返回空串，避免出现容易误读的 "./"。 */
export function directoryOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? "" : path.slice(0, index);
}

/** 文件名去扩展名，作为节点默认展示名。 */
export function labelOf(path: string): string {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** 虚节点标识：前缀带 # 且含引用 id，不会与知识库相对路径冲突。 */
export function virtualPathOf(link: KnowledgeVaultIndexLink): string {
  return "#virtual#" + link.status + "#" + link.id;
}

function toNode(note: KnowledgeVaultIndexNote): GraphNode {
  return {
    id: note.relative_path,
    kind: "note",
    label: labelOf(note.relative_path),
    path: note.relative_path,
    directory: directoryOf(note.relative_path),
    tags: [...note.tags],
    aliases: [...note.aliases],
    backlinkCount: note.backlink_count,
    outlinkCount: note.outlink_count,
    orphan: note.orphan,
    candidates: [],
  };
}

function toPlaceholder(link: KnowledgeVaultIndexLink): GraphNode {
  return {
    id: virtualPathOf(link),
    kind: link.status === "ambiguous" ? "ambiguous" : "unresolved",
    label: link.target_raw === "" ? "（空目标）" : link.target_raw,
    path: null,
    directory: "",
    tags: [],
    aliases: [],
    backlinkCount: 0,
    outlinkCount: 0,
    orphan: false,
    candidates: [...link.candidates],
  };
}

function toHealthItem(link: KnowledgeVaultIndexLink): GraphHealthItem {
  return {
    id: link.id,
    status: link.status,
    sourcePath: link.source_path,
    targetRaw: link.target_raw,
    display: link.display,
    line: link.line,
    context: link.context,
    candidates: [...link.candidates],
  };
}

/**
 * 把索引快照转成图谱数据。纯函数：同样输入必得同样输出，可直接用于单测。
 */
export function buildKnowledgeGraph(index: KnowledgeVaultIndex): KnowledgeGraph {
  const nodes: GraphNode[] = index.notes.map(toNode);
  const noteIds = new Set(nodes.map((node) => node.id));

  const edges: GraphEdge[] = [];
  const placeholders: GraphNode[] = [];
  const unresolved: GraphHealthItem[] = [];
  const ambiguous: GraphHealthItem[] = [];
  let danglingResolved = 0;

  for (const link of index.links) {
    if (link.status === "resolved") {
      const target = link.target_path;
      const sourceKnown = noteIds.has(link.source_path);
      if (!target || !noteIds.has(target) || !sourceKnown) {
        danglingResolved += 1;
        continue;
      }
      edges.push({
        id: link.id,
        source: link.source_path,
        target,
        status: "resolved",
        kind: link.kind,
        anchor: link.anchor,
        blockAnchor: link.block_anchor,
        line: link.line,
        candidates: [],
      });
      continue;
    }
    if (link.status === "unresolved" || link.status === "ambiguous") {
      const placeholder = toPlaceholder(link);
      placeholders.push(placeholder);
      const sourceOk = noteIds.has(link.source_path);
      if (sourceOk) {
        edges.push({
          id: link.id,
          source: link.source_path,
          target: placeholder.id,
          status: link.status,
          kind: link.kind,
          anchor: link.anchor,
          blockAnchor: link.block_anchor,
          line: link.line,
          candidates: [...link.candidates],
        });
      }
      if (link.status === "unresolved") unresolved.push(toHealthItem(link));
      else ambiguous.push(toHealthItem(link));
    }
    // external / attachment 不入图，也不进健康项：它们不是知识库内的笔记关系。
  }

  edges.sort(compareEdges);
  unresolved.sort(compareHealth);
  ambiguous.sort(compareHealth);
  placeholders.sort((a, b) => a.id.localeCompare(b.id));

  const allNodes = [...nodes, ...placeholders];
  const orphans = nodes
    .filter((node) => node.orphan)
    .map((node) => node.id)
    .sort();
  const hubPaths = nodes
    .filter((node) => node.backlinkCount > 0)
    .sort((a, b) => b.backlinkCount - a.backlinkCount || a.id.localeCompare(b.id))
    .map((node) => node.id);
  const tags = [...new Set(nodes.flatMap((node) => node.tags))].sort();
  const directories = [...new Set(nodes.map((node) => node.directory))].sort();

  return {
    generation: index.generation,
    complete: index.complete,
    incompleteReason: index.incomplete_reason,
    root: index.root,
    nodes: allNodes,
    edges,
    health: { unresolved, ambiguous, orphans, hubPaths },
    tags,
    directories,
    stats: {
      notes: nodes.length,
      placeholders: placeholders.length,
      resolvedEdges: edges.filter((edge) => edge.status === "resolved").length,
      placeholderEdges: edges.filter((edge) => edge.status !== "resolved").length,
      orphans: orphans.length,
      unresolved: unresolved.length,
      ambiguous: ambiguous.length,
      danglingResolved,
      danglingSource: 0,
    },
  };
}

function compareEdges(a: GraphEdge, b: GraphEdge): number {
  if (a.source !== b.source) return a.source.localeCompare(b.source);
  if (a.target !== b.target) return a.target.localeCompare(b.target);
  if (a.line !== b.line) return a.line - b.line;
  return a.id.localeCompare(b.id);
}

function compareHealth(a: GraphHealthItem, b: GraphHealthItem): number {
  if (a.sourcePath !== b.sourcePath) return a.sourcePath.localeCompare(b.sourcePath);
  if (a.line !== b.line) return a.line - b.line;
  return a.id.localeCompare(b.id);
}

/**
 * 图谱与索引的一致性自检：笔记数、实边数、虚节点/虚边数与孤立数逐项相等。
 * 界面在渲染前调用；不成立时按「索引异常」处理，不把不一致的图当作事实展示。
 */
export function graphMatchesIndex(graph: KnowledgeGraph, index: KnowledgeVaultIndex): boolean {
  const expectPlaceholders = index.stats.unresolved + index.stats.ambiguous;
  return (
    graph.generation === index.generation &&
    graph.stats.notes === index.stats.notes &&
    graph.stats.notes === index.notes.length &&
    graph.stats.resolvedEdges === index.stats.resolved &&
    graph.stats.placeholders === expectPlaceholders &&
    graph.stats.placeholderEdges === expectPlaceholders &&
    graph.stats.orphans === index.stats.orphans &&
    graph.stats.unresolved === index.stats.unresolved &&
    graph.stats.ambiguous === index.stats.ambiguous &&
    graph.stats.danglingResolved === 0
  );
}

export interface GraphFilter {
  /** 目录精确匹配；空串表示根目录；null 或省略表示不限。 */
  directory?: string | null;
  /** 标签精确匹配（忽略大小写与前置 #）；null 或省略表示不限。 */
  tag?: string | null;
  /** 只保留没有任何入链的笔记。 */
  orphansOnly?: boolean;
  /** 只保留入链数不少于该值的笔记；null 表示不限。 */
  minBacklinks?: number | null;
  /** 只保留至少有一条未解析或歧义引用的笔记。 */
  problematicOnly?: boolean;
  /** 是否把未解析/歧义虚节点画进视图。 */
  showPlaceholders?: boolean;
}

export interface GraphScope {
  /** 中心笔记相对路径；null 表示全局视图。 */
  centerPath?: string | null;
  /** 局部图谱度数（按无向邻接计算）。默认 1，上限 MAX_LOCAL_DEPTH。 */
  depth?: number;
}

export interface GraphView {
  nodes: GraphNode[];
  edges: GraphEdge[];
  /** 因筛选被去掉的边数，用于向用户解释「边为什么变少」。 */
  hiddenEdges: number;
  /** 请求了局部视图但中心笔记不在当前索引中；界面必须说明原因，不能退回全局图。 */
  centerMissing: boolean;
  /** 中心笔记被筛选条件排除时为 true，界面需要提示而不是画空图。 */
  centerFilteredOut: boolean;
  /** 视图内笔记节点数（不含虚节点）。 */
  noteCount: number;
  /** 视图内虚节点数。 */
  placeholderCount: number;
}

export const MAX_LOCAL_DEPTH = 3;

/** 规范化标签：去空白、去前置 #、转小写。 */
export function normalizeTag(tag: string): string {
  return tag.trim().replace(/^#/, "").toLowerCase();
}

/**
 * 计算当前视图：先取局部邻域，再套用筛选。
 * 只影响视图，不改图谱事实；边只在两端节点都保留时才保留，因此不会出现悬空边。
 */
export function selectGraphView(
  graph: KnowledgeGraph,
  filter: GraphFilter = {},
  scope: GraphScope = {},
): GraphView {
  const center = scope.centerPath ?? null;
  const centerExists = center !== null && graph.nodes.some((node) => node.id === center);
  const inScope = new Set<string>();

  // 局部模式的中心必须来自同一份索引快照。刚新建、尚未重建索引的笔记不在图里，
  // 这时退回全局图会让人误以为看到的是当前笔记的关系，因此返回明确的空视图。
  if (center !== null && !centerExists) {
    return {
      nodes: [],
      edges: [],
      hiddenEdges: 0,
      centerMissing: true,
      centerFilteredOut: false,
      noteCount: 0,
      placeholderCount: 0,
    };
  }

  if (center && centerExists) {
    const depth = Math.min(MAX_LOCAL_DEPTH, Math.max(1, Math.floor(scope.depth ?? 1)));
    const adjacency = new Map<string, Set<string>>();
    for (const edge of graph.edges) {
      if (!adjacency.has(edge.source)) adjacency.set(edge.source, new Set());
      if (!adjacency.has(edge.target)) adjacency.set(edge.target, new Set());
      adjacency.get(edge.source)!.add(edge.target);
      adjacency.get(edge.target)!.add(edge.source);
    }
    inScope.add(center);
    let frontier = [center];
    for (let step = 0; step < depth; step += 1) {
      const next: string[] = [];
      for (const id of frontier) {
        for (const neighbor of adjacency.get(id) ?? []) {
          if (!inScope.has(neighbor)) {
            inScope.add(neighbor);
            next.push(neighbor);
          }
        }
      }
      if (next.length === 0) break;
      frontier = next;
    }
  } else {
    for (const node of graph.nodes) inScope.add(node.id);
  }

  const tag = filter.tag ? normalizeTag(filter.tag) : null;
  const problematic = new Set<string>();
  if (filter.problematicOnly) {
    for (const item of graph.health.unresolved) problematic.add(item.sourcePath);
    for (const item of graph.health.ambiguous) problematic.add(item.sourcePath);
  }
  const minBacklinks = filter.minBacklinks ?? null;
  const showPlaceholders = filter.showPlaceholders === true;

  const filtered = graph.nodes.filter((node) => {
    if (!inScope.has(node.id)) return false;
    if (node.kind !== "note") return showPlaceholders;
    const path = node.path ?? "";
    if (filter.directory !== undefined && filter.directory !== null && node.directory !== filter.directory) {
      return false;
    }
    if (tag && !node.tags.some((item) => normalizeTag(item) === tag)) return false;
    if (filter.orphansOnly && !node.orphan) return false;
    if (minBacklinks !== null && node.backlinkCount < minBacklinks) return false;
    if (filter.problematicOnly && !problematic.has(path)) return false;
    return true;
  });

  const kept = new Set(filtered.map((node) => node.id));
  const edges = graph.edges.filter((edge) => kept.has(edge.source) && kept.has(edge.target));
  // 虚节点只为承载它那条引用边：来源被筛掉后，孤立地画出「未解析引用」会误导。
  const linked = new Set<string>();
  for (const edge of edges) {
    linked.add(edge.source);
    linked.add(edge.target);
  }
  const nodes = filtered.filter((node) => node.kind === "note" || linked.has(node.id));
  const scopeEdges = graph.edges.filter((edge) => inScope.has(edge.source) && inScope.has(edge.target));
  const notes = nodes.filter((node) => node.kind === "note");

  return {
    nodes,
    edges,
    hiddenEdges: scopeEdges.length - edges.length,
    centerMissing: false,
    centerFilteredOut: center !== null && centerExists && !nodes.some((node) => node.id === center),
    noteCount: notes.length,
    placeholderCount: nodes.length - notes.length,
  };
}
