import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KnowledgeVaultIndex } from "../types";
import {
  buildKnowledgeGraph,
  graphMatchesIndex,
  selectGraphView,
  normalizeTag,
  MAX_LOCAL_DEPTH,
  DEFAULT_HUB_MIN_BACKLINKS,
  type GraphFilter,
  type GraphNode,
} from "./knowledgeGraph";
import {
  computeLayout,
  foldedNodeIds,
  foldedPositions,
  radiusOf,
  MAX_FULL_LAYOUT_NODES,
  type LayoutPoint,
} from "./knowledgeGraphLayout";

/**
 * 知识图谱视图（OD-KB-001 P4-S02 / P4-S03 / P4-S04）。
 *
 * 数据唯一来源是 P3 索引快照：每次渲染由 buildKnowledgeGraph 现算，不落盘、不请求后端。
 * 布局是确定性的纯函数，节点坐标只在前端模拟里产生；拖动只是视图内的临时偏移。
 *
 * 设计参考（screenshots-design-inspiration，2026-10-26）：Miro/Lucidchart 把缩放与视图控制
 * 做成画布内的浮动工具条；Algolia/FactSet 把筛选与计数收在同一窄栏，先给数字再给图。
 *
 * 规模降级：节点超过 MAX_FULL_LAYOUT_NODES 时只布局入链最多的核心节点，其余折叠并明确提示，
 * 保证 2000 篇级数据下首次渲染不长时间占住主线程。
 */

const ZOOM_MIN = 0.35;
const ZOOM_MAX = 2.6;
const LABEL_LIMIT = 90;
/**
 * 根目录在 `<select>` 里的占位值。
 *
 * 「全部目录」也用空串，若根目录同样用空串，两者会变成同一个选项：选「（根目录）」
 * 实际等于取消筛选。这里给根目录一个显式哨兵，再在生成筛选条件时翻译回空串。
 */
const ROOT_DIRECTORY = "\u0000root";

export interface KnowledgeGraphViewProps {
  index: KnowledgeVaultIndex | null;
  selectedPath: string | null;
  onOpenNote: (relativePath: string) => void;
  onRequestIndex: () => void;
  indexBusy: boolean;
  indexError: string | null;
}

type Scope = "global" | "local";

function nodeAriaLabel(node: GraphNode): string {
  if (node.kind === "unresolved") return `未解析引用：${node.label}`;
  if (node.kind === "ambiguous") return `歧义引用：${node.label}`;
  const parts = [`笔记：${node.path ?? node.label}`, `入链 ${node.backlinkCount}`];
  if (node.orphan) parts.push("孤立");
  if (node.backlinkCount >= DEFAULT_HUB_MIN_BACKLINKS) parts.push("高连接度");
  return parts.join("；");
}

function clampZoom(value: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, value));
}

function shortLabel(node: GraphNode): string {
  if (node.kind !== "note") return node.label === "" ? "（空目标）" : node.label;
  return node.label;
}

export function KnowledgeGraphView({
  index,
  selectedPath,
  onOpenNote,
  onRequestIndex,
  indexBusy,
  indexError,
}: KnowledgeGraphViewProps) {
  const [directory, setDirectory] = useState<string>("");
  const [tag, setTag] = useState<string>("");
  const [orphansOnly, setOrphansOnly] = useState(false);
  const [problematicOnly, setProblematicOnly] = useState(false);
  const [minBacklinks, setMinBacklinks] = useState<string>("");
  const [showPlaceholders, setShowPlaceholders] = useState(true);
  const [scope, setScope] = useState<Scope>("global");
  const [depth, setDepth] = useState(1);
  const [layoutBudget, setLayoutBudget] = useState(MAX_FULL_LAYOUT_NODES);
  const [size, setSize] = useState({ width: 880, height: 620 });
  const [view, setView] = useState({ k: 1, x: 0, y: 0 });
  const [overrides, setOverrides] = useState<Map<string, LayoutPoint>>(new Map());
  const [hovered, setHovered] = useState<string | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const canvasRef = useRef<HTMLDivElement | null>(null);
  const panRef = useRef<{ x: number; y: number; startX: number; startY: number } | null>(null);

  const graph = useMemo(() => (index ? buildKnowledgeGraph(index) : null), [index]);
  const consistent = useMemo(
    () => (graph && index ? graphMatchesIndex(graph, index) : true),
    [graph, index],
  );

  // 局部图谱必须有一个当前笔记；切换笔记时中心随之变化。
  const effectiveScope: Scope = scope === "local" && selectedPath ? "local" : "global";
  const filter: GraphFilter = useMemo(
    () => ({
      directory:
        directory === "" ? null : directory === ROOT_DIRECTORY ? "" : directory,
      tag: tag.trim() === "" ? null : normalizeTag(tag),
      orphansOnly,
      problematicOnly,
      minBacklinks: minBacklinks.trim() === "" ? null : Math.max(0, Number(minBacklinks) || 0),
      showPlaceholders,
    }),
    [directory, tag, orphansOnly, problematicOnly, minBacklinks, showPlaceholders],
  );

  // 必须缓存：拖动与悬停会频繁重渲染，若这里每次重算生产新对象，
  // 下面的布局 memo 会失效并在每次 pointermove 时重跑整张图的力导向。
  const opened = useMemo(
    () =>
      graph
        ? selectGraphView(graph, filter, {
            centerPath: effectiveScope === "local" ? selectedPath : null,
            depth,
          })
        : null,
    [graph, filter, effectiveScope, selectedPath, depth],
  );

  const layout = useMemo(() => {
    if (!opened) return null;
    return computeLayout(opened.nodes, opened.edges, {
      width: size.width,
      height: size.height,
      maxFullNodes: layoutBudget,
    });
  }, [opened, size.width, size.height, layoutBudget]);

  const folded = useMemo(() => {
    if (!opened || !layout || !layout.simplified) return [];
    return foldedNodeIds(opened.nodes, layout.coreNodeIds);
  }, [opened, layout]);

  const foldedPoints = useMemo(
    () => foldedPositions(new Set(folded), size.width, size.height),
    [folded, size.width, size.height],
  );

  // 布局变化（筛选、缩放尺寸、索引重建）后丢弃旧的拖动偏移。
  useEffect(() => {
    setOverrides(new Map());
  }, [opened?.nodes.length, opened?.edges.length, size.width, size.height, index?.generation]);

  // 换索引、范围或筛选后回到默认预算，避免上一张图手动展开的规模泄漏到新视图。
  useEffect(() => {
    setLayoutBudget(MAX_FULL_LAYOUT_NODES);
  }, [filter, effectiveScope, selectedPath, index?.generation]);

  useEffect(() => {
    // 画布在「没有索引」时不会挂载；effect 只跑一次的话，等索引出现后画布仍是
    // 默认 880×620，布局会把节点摆到真实画布之外。因此每次都重新取元素、先量一次。
    const node = canvasRef.current;
    if (!node) return;
    const measure = (width: number, height: number) => {
      if (width > 40 && height > 40) {
        setSize({ width: Math.round(width), height: Math.round(height) });
      }
    };
    const rect = node.getBoundingClientRect();
    measure(rect.width, rect.height);
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const box = entry.contentRect;
        measure(box.width, box.height);
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [graph]);

  const pointFor = useCallback(
    (id: string): LayoutPoint | null => {
      const override = overrides.get(id);
      if (override) return override;
      return layout?.positions.get(id) ?? null;
    },
    [layout, overrides],
  );

  const openNode = (node: GraphNode) => {
    // 只有笔记节点提供跳转；未解析/歧义是猜测，永不打开笔记。
    if (node.kind !== "note" || !node.path) return;
    onOpenNote(node.path);
  };

  const beginNodeDrag = (event: React.PointerEvent<HTMLButtonElement>, node: GraphNode) => {
    const start = pointFor(node.id);
    if (!start) return;
    event.stopPropagation();
    event.preventDefault();
    const originX = event.clientX;
    const originY = event.clientY;
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    setDragging(node.id);
    let moved = false;
    const onMove = (moveEvent: PointerEvent) => {
      const dx = (moveEvent.clientX - originX) / view.k;
      const dy = (moveEvent.clientY - originY) / view.k;
      if (Math.abs(dx) + Math.abs(dy) > 2) moved = true;
      setOverrides((current) => {
        const next = new Map(current);
        next.set(node.id, { x: start.x + dx, y: start.y + dy });
        return next;
      });
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      setDragging(null);
      // 拖动结束时不再触发点击打开：位置变化才是拖动意图。
      if (!moved) openNode(node);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const beginPan = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    panRef.current = { x: view.x, y: view.y, startX: event.clientX, startY: event.clientY };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const movePan = (event: React.PointerEvent<HTMLDivElement>) => {
    const pan = panRef.current;
    if (!pan) return;
    setView((current) => ({
      ...current,
      x: pan.x + (event.clientX - pan.startX),
      y: pan.y + (event.clientY - pan.startY),
    }));
  };

  const endPan = () => {
    panRef.current = null;
  };

  const zoomBy = (factor: number) => {
    setView((current) => {
      const k = clampZoom(current.k * factor);
      const cx = size.width / 2;
      const cy = size.height / 2;
      return {
        k,
        x: cx - ((cx - current.x) / current.k) * k,
        y: cy - ((cy - current.y) / current.k) * k,
      };
    });
  };

  const onWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const px = event.clientX - rect.left;
    const py = event.clientY - rect.top;
    setView((current) => {
      const k = clampZoom(current.k * (event.deltaY < 0 ? 1.12 : 1 / 1.12));
      return {
        k,
        x: px - ((px - current.x) / current.k) * k,
        y: py - ((py - current.y) / current.k) * k,
      };
    });
  };

  const resetView = () => setView({ k: 1, x: 0, y: 0 });

  const clearFilters = () => {
    setDirectory("");
    setTag("");
    setOrphansOnly(false);
    setProblematicOnly(false);
    setMinBacklinks("");
    setShowPlaceholders(true);
  };

  if (!index || !graph) {
    return (
      <section
        className="flex-1 min-h-0 flex items-center justify-center"
        data-testid="vault-graph-no-index"
      >
        <div className="max-w-md rounded-2xl bg-white/85 border border-white/90 px-6 py-8 text-center">
          <p className="text-[13px] font-semibold text-on-surface">还没有关系索引</p>
          <p
            className="mt-2 text-[11.5px] leading-relaxed text-on-surface-variant"
            role="status"
            aria-label="图谱只从当前关系索引绘制。先在合成知识库上构建索引，再回到这里查看节点与连线。"
          >
            图谱只从当前关系索引绘制。先在合成知识库上构建索引，再回到这里查看节点与连线。
          </p>
          <button
            className="mt-4 rounded-lg bg-primary px-4 py-2 text-[12px] text-on-primary disabled:opacity-45"
            onClick={onRequestIndex}
            disabled={indexBusy}
            data-testid="vault-graph-build-index"
          >
            {indexBusy ? "构建中…" : "构建关系索引"}
          </button>
          {indexError ? (
            <p className="mt-2 text-[11px] text-error" role="alert">
              索引构建失败：{indexError}
            </p>
          ) : null}
        </div>
      </section>
    );
  }

  const visibleNodes = opened?.nodes ?? [];
  const hoveredNeighbors = new Set<string>();
  if (hovered) {
    for (const edge of opened?.edges ?? []) {
      if (edge.source === hovered) hoveredNeighbors.add(edge.target);
      if (edge.target === hovered) hoveredNeighbors.add(edge.source);
    }
  }

  return (
    <div className="flex-1 min-h-0 grid grid-cols-1 xl:grid-cols-[minmax(232px,272px)_minmax(0,1fr)] gap-2.5" data-testid="vault-graph">
      <aside className="min-h-0 flex flex-col gap-2.5 overflow-y-auto">
        <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
          <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
            <h2 className="text-[12.5px] font-semibold text-on-surface">图谱筛选</h2>
            <span
              className="mono text-[10px] text-outline"
              role="status"
              aria-label={`关系图谱第 ${graph.generation} 代`}
            >
              第 {graph.generation} 代
            </span>
          </div>
          {!consistent ? (
            <p
              className="mt-2 rounded bg-error-container/60 px-2 py-1 text-[10px] leading-relaxed text-on-error-container"
              role="alert"
              data-testid="vault-graph-inconsistent"
            >
              图数据与索引统计不一致，已按只读方式展示；请重建索引后再判断关系。
            </p>
          ) : null}
          <p
            className="mt-2 text-[10.5px] leading-relaxed text-on-surface-variant"
            role="status"
            aria-label={`图谱视图：${opened?.noteCount ?? 0} 篇笔记；${opened?.placeholderCount ?? 0} 个未解析或歧义；${opened?.edges.length ?? 0} 条连线；筛选隐藏 ${opened?.hiddenEdges ?? 0} 条连线`}
            data-testid="vault-graph-counts"
          >
            笔记 <span className="mono text-on-surface">{opened?.noteCount ?? 0}</span> · 连线{" "}
            <span className="mono text-on-surface">{opened?.edges.length ?? 0}</span> · 虚拟{" "}
            <span className="mono text-on-surface">{opened?.placeholderCount ?? 0}</span>
            {opened && opened.hiddenEdges > 0 ? (
              <span className="ml-1 text-outline">（筛选隐藏 {opened.hiddenEdges} 条）</span>
            ) : null}
          </p>
          <p className="mt-1 text-[10px] text-outline">
            索引：{graph.stats.notes} 篇 / 已解析 {graph.stats.resolvedEdges} / 未解析{" "}
            {graph.stats.unresolved} / 歧义 {graph.stats.ambiguous} / 孤立 {graph.stats.orphans}
          </p>
          {graph.incompleteReason ? (
            <p className="mt-1.5 rounded bg-amber-50 px-2 py-1 text-[10px] leading-relaxed text-amber-900" data-testid="vault-graph-incomplete">
              索引不完整：{graph.incompleteReason}。图谱只覆盖已收录的笔记。
            </p>
          ) : null}

          <div className="mt-3">
            <span className="block text-[10px] text-outline">范围</span>
            <div className="mt-1 flex items-center gap-1">
              <button
                className={
                  "rounded px-2 py-1 text-[10.5px] " +
                  (effectiveScope === "global"
                    ? "bg-primary text-on-primary"
                    : "bg-surface-container text-on-surface-variant")
                }
                onClick={() => setScope("global")}
                data-testid="vault-graph-scope-global"
              >
                全局
              </button>
              <button
                className={
                  "rounded px-2 py-1 text-[10.5px] disabled:opacity-45 " +
                  (effectiveScope === "local"
                    ? "bg-primary text-on-primary"
                    : "bg-surface-container text-on-surface-variant")
                }
                onClick={() => setScope("local")}
                disabled={!selectedPath}
                data-testid="vault-graph-scope-local"
              >
                当前笔记
              </button>
              <label className="ml-auto flex items-center gap-1 text-[10px] text-outline">
                度数
                <select
                  className="rounded border border-outline-variant/50 bg-white px-1 py-0.5 mono text-[10px]"
                  value={depth}
                  onChange={(event) => setDepth(Math.min(MAX_LOCAL_DEPTH, Math.max(1, Number(event.target.value))))}
                  data-testid="vault-graph-depth"
                  aria-label="局部图谱度数"
                >
                  {[1, 2, 3].map((value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            {effectiveScope === "local" && selectedPath ? (
              <p
                className="mt-1 mono text-[9.5px] text-outline break-all"
                role="status"
                aria-label={`中心：${selectedPath}`}
                data-testid="vault-graph-center"
              >
                中心：{selectedPath}
              </p>
            ) : null}
          </div>

          <label className="mt-3 block text-[10px] text-outline" htmlFor="vault-graph-directory">
            目录
          </label>
          <select
            id="vault-graph-directory"
            className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 text-[10.5px]"
            value={directory}
            onChange={(event) => setDirectory(event.target.value)}
            data-testid="vault-graph-filter-directory"
            aria-label="目录筛选"
          >
            <option value="">全部目录</option>
            {graph.directories.map((item) => (
              <option key={item || "root"} value={item === "" ? ROOT_DIRECTORY : item}>
                {item === "" ? "（根目录）" : item}
              </option>
            ))}
          </select>

          <label className="mt-2 block text-[10px] text-outline" htmlFor="vault-graph-tag">
            标签
          </label>
          <input
            id="vault-graph-tag"
            className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 text-[10.5px]"
            value={tag}
            onChange={(event) => setTag(event.target.value)}
            placeholder={graph.tags.length > 0 ? graph.tags.slice(0, 3).join(" / ") : "没有标签"}
            list="vault-graph-tag-options"
            data-testid="vault-graph-filter-tag"
            aria-label="标签筛选"
          />
          <datalist id="vault-graph-tag-options">
            {graph.tags.map((item) => (
              <option key={item} value={item} />
            ))}
          </datalist>

          <label className="mt-2 block text-[10px] text-outline" htmlFor="vault-graph-min-backlinks">
            高连接度阈值（入链 ≥）
          </label>
          <input
            id="vault-graph-min-backlinks"
            className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 mono text-[10.5px]"
            value={minBacklinks}
            onChange={(event) => setMinBacklinks(event.target.value.replace(/[^0-9]/g, ""))}
            placeholder={String(DEFAULT_HUB_MIN_BACKLINKS)}
            inputMode="numeric"
            data-testid="vault-graph-filter-min-backlinks"
            aria-label="入链阈值"
          />

          <div className="mt-2.5 space-y-1">
            <label className="flex items-center gap-1.5 text-[10.5px] text-on-surface-variant">
              <input
                type="checkbox"
                checked={orphansOnly}
                onChange={(event) => setOrphansOnly(event.target.checked)}
                data-testid="vault-graph-filter-orphans"
              />
              只看孤立笔记
            </label>
            <label className="flex items-center gap-1.5 text-[10.5px] text-on-surface-variant">
              <input
                type="checkbox"
                checked={problematicOnly}
                onChange={(event) => setProblematicOnly(event.target.checked)}
                data-testid="vault-graph-filter-problematic"
              />
              只看有未解析/歧义引用
            </label>
            <label className="flex items-center gap-1.5 text-[10.5px] text-on-surface-variant">
              <input
                type="checkbox"
                checked={showPlaceholders}
                onChange={(event) => setShowPlaceholders(event.target.checked)}
                data-testid="vault-graph-filter-placeholders"
              />
              显示未解析/歧义虚节点
            </label>
          </div>
          <button
            className="mt-2 text-[10.5px] text-on-surface-variant hover:text-primary"
            onClick={clearFilters}
            data-testid="vault-graph-clear-filters"
          >
            清空筛选
          </button>
        </section>

        <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
          <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
            <h2 className="text-[12.5px] font-semibold text-on-surface">链接健康</h2>
            <span className="mono text-[10px] text-outline">
              {graph.health.unresolved.length + graph.health.ambiguous.length}
            </span>
          </div>
          <ul className="pt-2 space-y-1" data-testid="vault-graph-health">
            {graph.health.unresolved.slice(0, 5).map((item) => (
              <li key={item.id} className="rounded bg-amber-50/70 px-2 py-1 text-[10px] leading-relaxed text-amber-900">
                <span className="mono text-[9px]">未解析</span> {item.targetRaw || "（空目标）"}
                <span className="block mono text-[9px] text-outline break-all">
                  {item.sourcePath} · 第 {item.line} 行
                </span>
              </li>
            ))}
            {graph.health.ambiguous.slice(0, 5).map((item) => (
              <li key={item.id} className="rounded bg-amber-50/70 px-2 py-1 text-[10px] leading-relaxed text-amber-900">
                <span className="mono text-[9px]">歧义</span> {item.targetRaw}
                <span className="block mono text-[9px] text-outline break-all">
                  {item.sourcePath} · 第 {item.line} 行
                  {item.candidates.length > 0 ? ` · 候选 ${item.candidates.length}` : ""}
                </span>
              </li>
            ))}
            {graph.health.unresolved.length + graph.health.ambiguous.length === 0 ? (
              <li className="text-[10.5px] text-on-surface-variant">没有未解析或歧义引用。</li>
            ) : null}
          </ul>
          <p className="mt-2 text-[10px] text-outline">
            孤立 {graph.health.orphans.length} 篇 · 高连接度（入链 ≥ {DEFAULT_HUB_MIN_BACKLINKS}）
            {" "}
            {graph.nodes.filter((node) => node.kind === "note" && node.backlinkCount >= DEFAULT_HUB_MIN_BACKLINKS).length} 篇
          </p>
        </section>
      </aside>

      <section className="min-h-0 flex flex-col rounded-2xl bg-white/85 border border-white/90 overflow-hidden">
        <div className="flex-none flex flex-wrap items-center gap-2 px-3 py-2 border-b border-surface-variant/20">
          <h2 className="text-[12.5px] font-semibold text-on-surface">关系图谱</h2>
          <span className="mono text-[9.5px] text-outline">拖动节点 · 滚轮缩放 · 点击打开</span>
          <div className="ml-auto flex items-center gap-1">
            <button
              className="rounded border border-outline-variant/50 px-2 py-0.5 text-[11px] text-on-surface-variant hover:border-primary hover:text-primary"
              onClick={() => zoomBy(1 / 1.2)}
              aria-label="缩小图谱"
              data-testid="vault-graph-zoom-out"
            >
              −
            </button>
            <span
              className="mono w-10 text-center text-[9.5px] text-outline"
              role="status"
              aria-label={`缩放 ${Math.round(view.k * 100)}%`}
              data-testid="vault-graph-zoom-level"
            >
              {Math.round(view.k * 100)}%
            </span>
            <button
              className="rounded border border-outline-variant/50 px-2 py-0.5 text-[11px] text-on-surface-variant hover:border-primary hover:text-primary"
              onClick={() => zoomBy(1.2)}
              aria-label="放大图谱"
              data-testid="vault-graph-zoom-in"
            >
              +
            </button>
            <button
              className="ml-1 rounded border border-outline-variant/50 px-2 py-0.5 text-[10px] text-on-surface-variant hover:border-primary hover:text-primary"
              onClick={resetView}
              data-testid="vault-graph-zoom-reset"
            >
              适应窗口
            </button>
          </div>
        </div>

        {opened && opened.centerFilteredOut ? (
          <p
            className="flex-none border-b border-surface-variant/20 bg-amber-50 px-3 py-1.5 text-[10.5px] leading-relaxed text-amber-900"
            role="status"
            aria-label="当前笔记被筛选条件排除，图中只显示与它相邻、且满足条件的节点。"
            data-testid="vault-graph-center-filtered"
          >
            当前笔记被筛选条件排除，图中只显示与它相邻、且满足条件的节点。
          </p>
        ) : null}
        {opened && opened.centerMissing ? (
          <p
            className="flex-none border-b border-surface-variant/20 bg-amber-50 px-3 py-1.5 text-[10.5px] leading-relaxed text-amber-900"
            role="status"
            aria-label="当前笔记不在关系索引中，局部图谱为空。请重建索引后再查看它的关系；这里不会用全局图代替。"
            data-testid="vault-graph-center-missing"
          >
            当前笔记不在关系索引中，局部图谱为空。请重建索引后再查看它的关系；这里不会用全局图代替。
          </p>
        ) : null}
        {layout?.simplified ? (
          <div
            className="flex-none border-b border-surface-variant/20 bg-surface-container-low px-3 py-1.5 text-[10.5px] leading-relaxed text-on-surface-variant flex flex-wrap items-center gap-2"
            data-testid="vault-graph-simplified"
          >
            <span
              role="status"
              aria-label={`已简化显示：当前视图共 ${opened?.nodes.length ?? 0} 个节点，只布局入链最多的 ${layout.positions.size} 个节点，另有 ${layout.folded} 个未展开。可缩小范围、加筛选，或按需展开。`}
            >
              已简化显示：当前视图共 {opened?.nodes.length ?? 0} 个节点，只布局入链最多的{" "}
              {layout.positions.size} 个节点，另有 {layout.folded} 个未展开。
            </span>
            <button
              className="rounded border border-outline-variant/50 px-2 py-0.5 text-[10px] hover:border-primary hover:text-primary"
              onClick={() =>
                setLayoutBudget((current) =>
                  Math.min(opened?.nodes.length ?? current, current + MAX_FULL_LAYOUT_NODES),
                )
              }
              data-testid="vault-graph-expand-more"
            >
              展开更多
            </button>
          </div>
        ) : null}

        <div
          ref={canvasRef}
          className="relative flex-1 min-h-0 overflow-hidden bg-surface-container-lowest"
          data-testid="vault-graph-canvas"
          role="application"
          aria-label={`关系图谱画布：${opened?.noteCount ?? 0} 篇笔记，${opened?.edges.length ?? 0} 条连线`}
          onPointerDown={beginPan}
          onPointerMove={movePan}
          onPointerUp={endPan}
          onPointerCancel={endPan}
          onWheel={onWheel}
        >
          {visibleNodes.length === 0 ? (
            <div
              className="absolute inset-0 flex items-center justify-center px-6"
              role="status"
              aria-label={
                opened?.centerMissing
                  ? "当前笔记不在关系索引中。重建索引后，这里会画出以它为中心的局部关系。"
                  : effectiveScope === "local"
                  ? "当前笔记在这个范围内没有可显示的关系。"
                  : "当前筛选条件下没有节点。清空筛选或换一个目录看看。"
              }
              data-testid="vault-graph-empty"
            >
              <p className="max-w-sm text-center text-[11.5px] leading-relaxed text-on-surface-variant">
                {opened?.centerMissing
                  ? "当前笔记不在关系索引中。重建索引后，这里会画出以它为中心的局部关系。"
                  : effectiveScope === "local"
                  ? "当前笔记在这个范围内没有可显示的关系。"
                  : "当前筛选条件下没有节点。清空筛选或换一个目录看看。"}
              </p>
            </div>
          ) : (
            <div
              className="absolute inset-0 origin-top-left"
              style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.k})` }}
            >
              <svg
                className="absolute inset-0 w-full h-full pointer-events-none"
                aria-hidden="true"
              >
                <defs>
                  <marker id="vault-graph-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
                    <path d="M 0 0 L 10 5 L 0 10 z" fill="#94a3b8" />
                  </marker>
                  <marker id="vault-graph-arrow-warn" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
                    <path d="M 0 0 L 10 5 L 0 10 z" fill="#b45309" />
                  </marker>
                </defs>
                {(opened?.edges ?? []).map((edge) => {
                  const from = pointFor(edge.source);
                  const to = pointFor(edge.target);
                  if (!from || !to) return null;
                  const resolved = edge.status === "resolved";
                  const dim = hovered !== null && edge.source !== hovered && edge.target !== hovered;
                  return (
                    <line
                      key={edge.id}
                      x1={from.x}
                      y1={from.y}
                      x2={to.x}
                      y2={to.y}
                      stroke={resolved ? "#94a3b8" : "#d97706"}
                      strokeWidth={dim ? 0.5 : resolved ? 0.9 : 0.9}
                      strokeDasharray={resolved ? undefined : "3 3"}
                      strokeOpacity={dim ? 0.2 : 0.75}
                      markerEnd={`url(#${resolved ? "vault-graph-arrow" : "vault-graph-arrow-warn"})`}
                      data-testid="vault-graph-edge"
                      data-status={edge.status}
                    />
                  );
                })}
                {opened?.noteCount === 0 && (opened?.placeholderCount ?? 0) > 0 ? null : null}
              </svg>

              {visibleNodes.map((node) => {
                const point = pointFor(node.id);
                if (!point) return null;
                const radius = node.kind === "note" ? radiusOf(node) : 5;
                const isHovered = hovered === node.id;
                const dim = hovered !== null && !isHovered && !hoveredNeighbors.has(node.id);
                const isCenter = effectiveScope === "local" && node.id === selectedPath;
                return (
                  <button
                    key={node.id}
                    className={
                      "absolute rounded-full border transition-[background-color,box-shadow] " +
                      (node.kind === "note"
                        ? node.orphan
                          ? "bg-white border-outline-variant/80"
                          : "bg-primary/80 border-primary"
                        : "bg-amber-100 border-amber-600 border-dashed") +
                      (isCenter ? " ring-2 ring-primary/50 ring-offset-1" : "") +
                      (dragging === node.id ? " cursor-grabbing" : " cursor-grab")
                    }
                    style={{
                      left: point.x,
                      top: point.y,
                      width: radius * 2,
                      height: radius * 2,
                      transform: "translate(-50%, -50%)",
                      opacity: dim ? 0.35 : 1,
                      touchAction: "none",
                    }}
                    onPointerDown={(event) => beginNodeDrag(event, node)}
                    // 指针抬起已负责打开；只有键盘/读屏产生 detail=0 的 click 才在这里补上。
                    // 否则鼠标 click 会与 pointerup 重复触发；拖动后的 click 也一并被忽略。
                    onClick={(event) => {
                      if (event.detail === 0) openNode(node);
                    }}
                    onMouseEnter={() => setHovered(node.id)}
                    onMouseLeave={() => setHovered((current) => (current === node.id ? null : current))}
                    aria-label={nodeAriaLabel(node)}
                    title={nodeAriaLabel(node)}
                    data-testid="vault-graph-node"
                    data-kind={node.kind}
                    data-path={node.path ?? ""}
                    data-backlinks={node.backlinkCount}
                  />
                );
              })}

              {(opened?.noteCount ?? 0) <= LABEL_LIMIT
                ? visibleNodes.map((node) => {
                    const point = pointFor(node.id);
                    if (!point) return null;
                    return (
                      <span
                        key={`label-${node.id}`}
                        className="absolute max-w-[132px] truncate text-center text-[9.5px] leading-tight text-on-surface-variant pointer-events-none"
                        style={{
                          left: point.x,
                          top: point.y + (node.kind === "note" ? radiusOf(node) : 5) + 2,
                          transform: "translate(-50%, 0)",
                        }}
                        aria-hidden="true"
                      >
                        {shortLabel(node)}
                      </span>
                    );
                  })
                : null}

              {folded.map((id) => {
                const point = foldedPoints.get(id);
                if (!point) return null;
                return (
                  <span
                    key={`folded-${id}`}
                    className="absolute w-1 h-1 rounded-full bg-outline/40 pointer-events-none"
                    style={{ left: point.x, top: point.y, transform: "translate(-50%, -50%)" }}
                    aria-hidden="true"
                  />
                );
              })}
            </div>
          )}
        </div>

        <div className="flex-none border-t border-surface-variant/20 px-3 py-1.5 flex flex-wrap items-center gap-3 text-[9.5px] text-outline">
          <span className="flex items-center gap-1">
            <span className="inline-block w-2.5 h-2.5 rounded-full bg-primary/80" />笔记
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-2.5 h-2.5 rounded-full bg-white border border-outline-variant/80" />孤立
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-2.5 h-2.5 rounded-full bg-amber-100 border border-dashed border-amber-600" />
            未解析/歧义
          </span>
          <span className="ml-auto">
            {graph.stats.danglingResolved > 0
              ? `${graph.stats.danglingResolved} 条已解析引用缺少目标笔记（未画边）`
              : "连线方向 = 引用方向"}
          </span>
        </div>
      </section>
    </div>
  );
}

