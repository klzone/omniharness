// 成果文档库视图（OD-ARCH-060 S2）。
//
// 只列**本场景自己保存**的成果（调用方已按 scenario 过滤）。三条约束：
//  1. 列表页只读元数据（路径、大小、修改时间），标题/字数**按当前页懒加载**，
//     不会为了渲染首屏把整个成果目录的正文读一遍；
//  2. 标题来自文件里的 frontmatter（`title:`），拿不到就退回文件名——
//     用户手改过、或 058 之前写的老文件都属于这一类，按「无元数据」降级显示，不报错；
//  3. 分页固定每页 24 条；超过 100 个文件时先按页看，不在首屏做全量读取。

import { useEffect, useMemo, useRef, useState } from "react";
import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { OfflineArtifactEntry } from "../types";

const PAGE_SIZE = 24;
const HARD_CAP = 500;

/**
 * 归档条目（OD-ARCH-093 修订）：历史周报 / 历史周记录不再单独占一张卡，
 * 而是并入成果文档库的同一个列表，用「归档」徽标区分。
 *
 * 归档只带元数据与一个只读打开动作：预览仍由调用方走各自宿主的只读命令，
 * 这里不拼内容、不猜标题。
 */
export interface ArtifactArchiveEntry {
  /** 列表键：`report:2026-W43-周报.md` / `week:2026-W43.md`，与成果路径不会撞。 */
  key: string;
  /** 来源类型，原样显示：周报 / 周记录。 */
  kindLabel: string;
  week: string;
  file_name: string;
  path: string;
  modified_at: number;
  onOpen: () => void;
}

/** 成果与归档共用一行；归档没有正文元数据，标题按周标签生成。 */
type Row =
  | {
      kind: "artifact";
      key: string;
      file_name: string;
      relative_path: string;
      modified_at: number;
      bytes: number;
      /** 宿主已解析的内容类型（写作页用）；未标注为 null。 */
      artifact_type: string | null;
      onOpen: () => void;
    }
  | {
      kind: "archive";
      key: string;
      file_name: string;
      relative_path: string;
      modified_at: number;
      archive: ArtifactArchiveEntry;
    };

interface ArtifactMeta {
  title: string | null;
  /** 正文字数（不含空白与 frontmatter）；未读到正文时为 null。 */
  chars: number | null;
  /** frontmatter 里声明的场景标签；老文件没有。 */
  tag: string | null;
  /** frontmatter 里的内容类型（写作页用于类型标签）；老文件没有。 */
  type: string | null;
  /** 读取失败的原因（例如文件被用户在 Obsidian 里删掉）。 */
  error: string | null;
}

function splitFrontmatter(body: string): { meta: string | null; rest: string } {
  if (!body.startsWith("---\n")) return { meta: null, rest: body };
  const end = body.indexOf("\n---\n", 4);
  if (end < 0) return { meta: null, rest: body };
  return { meta: body.slice(4, end), rest: body.slice(end + 5) };
}

/** 只解析我们自己的小集合字段；不做通用 YAML。 */
function field(meta: string | null, key: string): string | null {
  if (!meta) return null;
  for (const line of meta.split("\n")) {
    const at = line.indexOf(":");
    if (at <= 0) continue;
    if (line.slice(0, at).trim() !== key) continue;
    let value = line.slice(at + 1).trim();
    if (value.startsWith("\"") && value.endsWith("\"") && value.length >= 2) {
      value = value.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, "\\");
    }
    return value || null;
  }
  return null;
}

function countChars(text: string): number {
  return Array.from(text.replace(/\s+/g, "")).length;
}

function dateText(secs: number): string {
  if (!Number.isFinite(secs) || secs <= 0) return "—";
  const d = new Date(secs * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 本周一 00:00 与本月 1 日 00:00 的 Unix 秒（本机时区）。 */
function windows(now: Date): { week: number; month: number } {
  const monday = new Date(now);
  const shift = (now.getDay() + 6) % 7;
  monday.setHours(0, 0, 0, 0);
  monday.setDate(monday.getDate() - shift);
  const first = new Date(now.getFullYear(), now.getMonth(), 1, 0, 0, 0, 0);
  return { week: monday.getTime() / 1000, month: first.getTime() / 1000 };
}

export function ArtifactExplorer({
  files,
  archiveEntries = [],
  archiveError = null,
  archiveSkipped = 0,
  emptyHint,
  includeSubdirs,
  onToggleSubdirs,
  compactToolbar = false,
  showTypeLabels = false,
  stalePaths,
  accent = "primary",
  onOpen,
}: {
  files: OfflineArtifactEntry[];
  /** 历史周报 / 周记录（OD-ARCH-093 修订：与成果合并为一个列表）。 */
  archiveEntries?: ArtifactArchiveEntry[];
  /** 归档目录读取失败的原因；与成果错误分开显示，不混成「没有成果」。 */
  archiveError?: string | null;
  /** 文件名不合约定、被宿主跳过的归档文件数（原「历史归档」卡的尾部提示）。 */
  archiveSkipped?: number;
  emptyHint: string;
  includeSubdirs: boolean;
  onToggleSubdirs: (next: boolean) => void;
  /**
   * 精简工具栏（OD-ARCH-095，仅工作记录页的右栏用）：不显示时间范围筛选，固定看最近
   * 30 天；搜索、卡片/列表和「含子目录」仍保留，避免把目录范围藏成不可见状态。
   */
  compactToolbar?: boolean;
  /**
   * 显示内容类型标签（OD-ARCH-094 S04，仅写作页）：类型由宿主解析 frontmatter 得到，
   * 缺省「未分类」。其它场景保持原样，不额外增加一行元信息。
   */
  showTypeLabels?: boolean;
  /** 超过 90 天未动的成果路径（写作页）：只做视觉标记，筛选由调用方处理。 */
  stalePaths?: Set<string>;
  /** 强调色（写作页用青绿，避免与工作记录页的紫完全一样）。 */
  accent?: "primary" | "secondary";
  onOpen: (relativePath: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [range, setRange] = useState<"all" | "week" | "month">("all");
  const [view, setView] = useState<"cards" | "list">("cards");
  const [page, setPage] = useState(0);
  const [meta, setMeta] = useState<Record<string, ArtifactMeta>>({});
  const loadingRef = useRef<Set<string>>(new Set());

  const now = useMemo(() => new Date(), []);
  const since = useMemo(() => windows(now), [now]);

  /** 成果与归档并成同一个列表：用户要的是「成果展」，归档只是其中一个标记。 */
  const rows = useMemo<Row[]>(() => {
    const artifacts: Row[] = files
      .filter((f) => !f.is_dir)
      .map((f) => ({
        kind: "artifact" as const,
        key: "file:" + f.relative_path,
        file_name: f.file_name,
        relative_path: f.relative_path,
        modified_at: f.modified_at,
        bytes: f.bytes,
        artifact_type: f.artifact_type ?? null,
        onOpen: () => onOpen(f.relative_path),
      }));
    const archived: Row[] = archiveEntries.map((a) => ({
      kind: "archive" as const,
      key: a.key,
      file_name: a.file_name,
      relative_path: a.path || a.file_name,
      modified_at: a.modified_at,
      archive: a,
    }));
    return [...artifacts, ...archived].sort((a, b) => b.modified_at - a.modified_at);
  }, [files, archiveEntries, onOpen]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    // 精简模式固定「近一个月」：按 30 天滚动窗口，不是自然月。
    const from = compactToolbar
      ? now.getTime() / 1000 - 30 * 24 * 60 * 60
      : range === "week"
        ? since.week
        : range === "month"
          ? since.month
          : 0;
    return rows
      .filter((r) => (from > 0 ? r.modified_at >= from : true))
      .filter((r) => {
        if (!q) return true;
        const hay =
          r.kind === "archive"
            ? `${r.file_name} ${r.relative_path} ${r.archive.kindLabel} ${r.archive.week} 归档`
            : `${r.file_name} ${r.relative_path}`;
        return hay.toLowerCase().includes(q);
      });
  }, [rows, query, range, since, compactToolbar, now]);

  const capped = filtered.slice(0, HARD_CAP);
  const pageCount = Math.max(1, Math.ceil(capped.length / PAGE_SIZE));
  const current = Math.min(page, pageCount - 1);
  const visible = capped.slice(current * PAGE_SIZE, current * PAGE_SIZE + PAGE_SIZE);
  const currentKey = visible.map((v) => v.key).join("|");

  // 标题/字数只给当前页的条目读；已经读过的条目不再重复读。
  useEffect(() => {
    let alive = true;
    const todo = visible
      .filter((v) => v.kind === "artifact")
      .filter((v) => !meta[v.relative_path] && !loadingRef.current.has(v.relative_path));
    if (todo.length === 0) return;
    const run = async () => {
      for (const item of todo) {
        if (!alive) return;
        loadingRef.current.add(item.relative_path);
        try {
          const content = await shell.readOfflineArtifact(item.relative_path);
          if (!alive) return;
          const { meta: fm, rest } = splitFrontmatter(content.content);
          setMeta((prev) => ({
            ...prev,
            [item.relative_path]: {
              title: field(fm, "title"),
              chars: countChars(rest),
              tag: field(fm, "scenario"),
              type: field(fm, "type"),
              error: null,
            },
          }));
        } catch (e) {
          if (!alive) return;
          setMeta((prev) => ({
            ...prev,
            [item.relative_path]: {
              title: null,
              chars: null,
              tag: null,
              type: null,
              error: e instanceof Error ? e.message : String(e),
            },
          }));
        } finally {
          loadingRef.current.delete(item.relative_path);
        }
      }
    };
    void run();
    return () => {
      alive = false;
    };
    // meta 变化不再触发整体重读：只依赖当前页与过滤结果。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentKey]);

  useEffect(() => {
    setPage(0);
  }, [query, range]);

  const chip = (active: boolean) =>
    "px-3 py-1.5 rounded-full text-[12px] leading-none transition " +
    (active
      ? accent === "secondary"
        ? "bg-secondary text-on-secondary shadow-[0_1px_2px_rgba(0,104,122,0.25)]"
        : "bg-primary text-on-primary shadow-[0_1px_2px_rgba(53,37,205,0.25)]"
      : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high");

  const focusRing = accent === "secondary" ? "focus:ring-secondary/25" : "focus:ring-primary/25";
  const typeLabel = (value: string | null | undefined): string => {
    switch ((value ?? "").trim().toLowerCase()) {
      case "insight":
      case "学习心得":
        return "学习心得";
      case "ai":
      case "ai知识":
      case "ai-knowledge":
        return "AI 知识";
      case "blog":
      case "博客":
        return "blog";
      case "website":
      case "网站内容":
        return "网站内容";
      default:
        return "未分类";
    }
  };

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="pb-3.5 border-b border-surface-variant/25">
        <div className="flex min-w-0 items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <input
              className={"w-full rounded-xl border border-surface-variant/50 bg-surface-container-lowest/80 px-3 py-2 pr-10 text-[12px] placeholder:text-outline/80 focus:outline-none focus:ring-2 " + focusRing}
              placeholder="搜索成果或归档"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label="按文件名搜索成果与归档"
            />
            <Icon
              name="search"
              className="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-outline"
            />
          </div>
          {compactToolbar ? null : (
            <div className="hidden shrink-0 items-center gap-1.5 sm:flex">
              <button className={chip(range === "all")} onClick={() => setRange("all")}>
                全部
              </button>
              <button className={chip(range === "week")} onClick={() => setRange("week")}>
                本周
              </button>
              <button className={chip(range === "month")} onClick={() => setRange("month")}>
                本月
              </button>
            </div>
          )}
        </div>
        {!compactToolbar ? (
          <div className="flex items-center justify-end gap-1.5 pt-2 sm:hidden">
            <button className={chip(range === "all")} onClick={() => setRange("all")}>
              全部
            </button>
            <button className={chip(range === "week")} onClick={() => setRange("week")}>
              本周
            </button>
            <button className={chip(range === "month")} onClick={() => setRange("month")}>
              本月
            </button>
          </div>
        ) : null}
        <div className="flex items-center justify-end gap-1.5 pt-2">
          <button
            className={chip(view === "cards")}
            onClick={() => setView("cards")}
            aria-pressed={view === "cards"}
            data-artifact-view="cards"
          >
            卡片
          </button>
          <button
            className={chip(view === "list")}
            onClick={() => setView("list")}
            aria-pressed={view === "list"}
            data-artifact-view="list"
          >
            列表
          </button>
          <label
            className="ml-1 flex cursor-pointer select-none items-center gap-1.5 text-[12px] text-on-surface-variant"
            title="打开后连子目录里的成果一起列；关闭只看最外层"
          >
            <input
              type="checkbox"
              className="h-4 w-4 accent-current"
              checked={includeSubdirs}
              onChange={(e) => onToggleSubdirs(e.target.checked)}
            />
            含子目录
          </label>
        </div>
      </div>

      <div className="pt-3 pb-3 space-y-1">
        {filtered.length > HARD_CAP ? (
          <p className="text-[12px] text-outline leading-relaxed">结果较多，仅显示前 {HARD_CAP} 条。</p>
        ) : null}
        {archiveError ? (
          <p className="text-[12px] text-error leading-relaxed">读取历史归档失败：{archiveError}</p>
        ) : null}
        {archiveSkipped > 0 ? (
          <p className="text-[12px] text-outline/80 leading-relaxed">
            另有 {archiveSkipped} 个归档文件名不符合约定，未列入、也未改动。
          </p>
        ) : null}
      </div>

      <div className="min-h-0 overflow-y-auto flex-1 pb-1">
        {filtered.length === 0 ? (
          <p className="text-[12px] text-outline leading-relaxed pt-1">{emptyHint}</p>
        ) : view === "cards" ? (
          <div className="flex flex-col gap-2.5">
            {visible.map((r, i) => {
              const number = String(current * PAGE_SIZE + i + 1).padStart(2, "0");
              if (r.kind === "archive") {
                return (
                  <button
                    key={r.key}
                    onClick={r.archive.onOpen}
                    data-artifact-card=""
                    data-artifact-archive=""
                    className="group text-left rounded-2xl border border-surface-variant/25 bg-surface-container-low/45 hover:bg-surface-container-low hover:border-primary/25 px-4 py-3.5 flex items-start gap-3.5 min-w-0 transition"
                    title={r.relative_path}
                  >
                    <span className="mono shrink-0 w-9 text-[15px] leading-none pt-0.5 text-outline group-hover:text-primary">
                      {number}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline gap-2 min-w-0">
                        <span className="text-[13.5px] font-medium text-on-surface truncate">
                          {r.archive.week} {r.archive.kindLabel}
                        </span>
                        <span className="shrink-0 px-1.5 py-[1px] rounded-full text-[12px] text-on-surface-variant bg-surface-container-high">
                          归档
                        </span>
                      </span>
                      {/*
                       * 只留标题与时间（OD-ARCH-097 S04）：文件名与标题常常同名，
                       * 并排显示就是重复。完整路径仍在 title 悬浮提示里。
                       */}
                      <span className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12px] text-outline">
                        <span>{dateText(r.modified_at)}</span>
                      </span>
                    </span>
                  </button>
                );
              }
              const m = meta[r.relative_path];
              const stale = stalePaths?.has(r.relative_path) ?? false;
              return (
                <button
                  key={r.key}
                  onClick={r.onOpen}
                  data-artifact-card=""
                  className={
                    "group text-left rounded-2xl border px-4 py-3.5 flex items-start gap-3.5 min-w-0 transition " +
                    (stale
                      ? "border-amber-500/20 bg-amber-500/5 hover:bg-amber-500/10"
                      : "border-transparent bg-surface-container-low/55 hover:bg-surface-container-low hover:border-surface-variant/30")
                  }
                  title={r.relative_path}
                  data-artifact-stale={stale ? "" : undefined}
                >
                  {/* 编号卡（OD-ARCH-093 S04）：大号序号 + 标题 + 文件名/时间，和参考图右栏一致。 */}
                  <span className="mono shrink-0 w-9 text-[15px] leading-none pt-0.5 text-outline group-hover:text-primary">
                    {number}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-[13.5px] font-medium text-on-surface line-clamp-2 leading-[1.55] break-words">
                      {m ? m.title ?? r.file_name : "正在读取标题…"}
                    </span>
                    {/* 只留标题与时间（OD-ARCH-097 S04）：文件名退到 title 悬浮提示，
                        字数/字节这类次要信息不再占版面。 */}
                    <span className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12px] text-outline">
                      <span>{dateText(r.modified_at)}</span>
                      {showTypeLabels ? (
                        <span
                          className="px-1.5 py-[1px] rounded-full text-[12px] text-secondary bg-secondary/10"
                          data-artifact-type={typeLabel(r.artifact_type ?? m?.type ?? null)}
                        >
                          {typeLabel(r.artifact_type ?? m?.type ?? null)}
                        </span>
                      ) : null}
                      {stale ? <span className="text-amber-700">· 90 天未动</span> : null}
                      {m?.error ? <span className="text-error">· 读取失败</span> : null}
                      {m && !m.tag ? (
                        <span className="px-1.5 py-[1px] rounded-full bg-surface-container">无元数据</span>
                      ) : null}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        ) : (
          <ul className="flex flex-col divide-y divide-surface-variant/20" data-artifact-list="">
            {visible.map((r) => {
              const m = r.kind === "artifact" ? meta[r.relative_path] : undefined;
              const stale = r.kind === "artifact" && (stalePaths?.has(r.relative_path) ?? false);
              const primary =
                r.kind === "archive"
                  ? `${r.archive.week} ${r.archive.kindLabel}`
                  : m
                    ? m.title ?? r.file_name
                    : "正在读取标题…";
              return (
                <li key={r.key} className="flex items-center gap-3 py-2.5 first:pt-0 min-w-0">
                  {r.kind === "archive" ? (
                    <span className="shrink-0 px-1.5 py-[1px] rounded-full text-[12px] text-on-surface-variant bg-surface-container-high">
                      归档
                    </span>
                  ) : null}
                  <button
                    className="min-w-0 flex-1 text-left group"
                    title={r.relative_path}
                    onClick={r.kind === "archive" ? r.archive.onOpen : r.onOpen}
                  >
                    <span className="block text-[12.5px] text-on-surface truncate group-hover:text-primary">
                      {primary}
                    </span>
                  </button>
                  <span className="text-[12px] text-outline shrink-0 whitespace-nowrap">
                    {dateText(r.modified_at)}
                  </span>
                  {r.kind === "artifact" ? (
                    <>
                      {showTypeLabels ? (
                        <span className="shrink-0 px-1.5 py-[1px] rounded-full text-[12px] text-secondary bg-secondary/10">
                          {typeLabel(r.artifact_type ?? m?.type ?? null)}
                        </span>
                      ) : null}
                      {stale ? (
                        <span className="text-[12px] text-amber-700 shrink-0 whitespace-nowrap">90 天未动</span>
                      ) : null}
                    </>
                  ) : null}
                </li>
              );
            })}
          </ul>
        )}

        {pageCount > 1 ? (
          <div className="flex items-center gap-2.5 pt-3.5">
            <button
              className="text-[12px] px-2.5 py-1 rounded-full bg-surface-container disabled:opacity-40"
              disabled={current === 0}
              onClick={() => setPage(current - 1)}
            >
              上一页
            </button>
            <span className="text-[12px] text-outline">
              第 {current + 1} / {pageCount} 页
            </span>
            <button
              className="text-[12px] px-2.5 py-1 rounded-full bg-surface-container disabled:opacity-40"
              disabled={current >= pageCount - 1}
              onClick={() => setPage(current + 1)}
            >
              下一页
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
