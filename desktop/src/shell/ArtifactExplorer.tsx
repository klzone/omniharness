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
import type { OfflineArtifactEntry } from "../types";

const PAGE_SIZE = 24;
const HARD_CAP = 500;

interface ArtifactMeta {
  title: string | null;
  /** 正文字数（不含空白与 frontmatter）；未读到正文时为 null。 */
  chars: number | null;
  /** frontmatter 里声明的场景标签；老文件没有。 */
  tag: string | null;
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
  emptyHint,
  includeSubdirs,
  onToggleSubdirs,
  onOpen,
}: {
  files: OfflineArtifactEntry[];
  emptyHint: string;
  includeSubdirs: boolean;
  onToggleSubdirs: (next: boolean) => void;
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

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const from = range === "week" ? since.week : range === "month" ? since.month : 0;
    return files
      .filter((f) => !f.is_dir)
      .filter((f) => (from > 0 ? f.modified_at >= from : true))
      .filter((f) => (q ? f.file_name.toLowerCase().includes(q) || f.relative_path.toLowerCase().includes(q) : true))
      .sort((a, b) => b.modified_at - a.modified_at);
  }, [files, query, range, since]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const current = Math.min(page, pageCount - 1);
  const visible = filtered.slice(current * PAGE_SIZE, current * PAGE_SIZE + PAGE_SIZE);

  // 标题/字数只给当前页的条目读；已经读过的条目不再重复读。
  useEffect(() => {
    let alive = true;
    const todo = visible.filter((v) => !meta[v.relative_path] && !loadingRef.current.has(v.relative_path));
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
  }, [visible.map((v) => v.relative_path).join("|")]);

  useEffect(() => {
    setPage(0);
  }, [query, range]);

  const chip = (active: boolean) =>
    "px-2 py-0.5 rounded-full text-[10.5px] " +
    (active ? "bg-primary text-on-primary" : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high");

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="flex flex-wrap items-center gap-1.5 pb-1.5 border-b border-surface-variant/10">
        <input
          className="px-2 py-1 rounded-md border border-surface-variant/30 bg-surface-container-lowest/70 text-[11px] w-[180px]"
          placeholder="搜文件名"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="按文件名搜索成果"
        />
        <button className={chip(range === "all")} onClick={() => setRange("all")}>
          全部
        </button>
        <button className={chip(range === "week")} onClick={() => setRange("week")}>
          本周
        </button>
        <button className={chip(range === "month")} onClick={() => setRange("month")}>
          本月
        </button>
        <div className="ml-auto flex items-center gap-1">
          <button className={chip(view === "cards")} onClick={() => setView("cards")}>
            卡片
          </button>
          <button className={chip(view === "list")} onClick={() => setView("list")}>
            列表
          </button>
          <label
            className="ml-1 flex items-center gap-1 text-[10.5px] text-on-surface-variant cursor-pointer select-none"
            title="打开后连子目录里的成果一起列；关闭只看最外层"
          >
            <input
              type="checkbox"
              className="accent-current"
              checked={includeSubdirs}
              onChange={(e) => onToggleSubdirs(e.target.checked)}
            />
            含子目录
          </label>
        </div>
      </div>

      <p className="pt-1.5 text-[10px] text-outline leading-relaxed">
        只列本场景保存的成果，共 {filtered.length} 份
        {filtered.length > HARD_CAP ? `（只显示前 ${HARD_CAP} 份）` : ""}。
        成果是标准 Markdown，可在 Obsidian 里把设置页显示的目录「打开文件夹作为 vault」直接浏览。
      </p>

      <div className="pt-1.5 min-h-0 overflow-y-auto flex-1">
        {filtered.length === 0 ? (
          <p className="text-[11px] text-outline leading-relaxed">{emptyHint}</p>
        ) : view === "cards" ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-2">
            {visible.map((f) => {
              const m = meta[f.relative_path];
              return (
                <button
                  key={f.relative_path}
                  onClick={() => onOpen(f.relative_path)}
                  className="text-left rounded-xl border border-surface-variant/30 bg-white/70 hover:bg-white px-3 py-2 flex flex-col gap-1 min-h-[86px]"
                  title={f.relative_path}
                >
                  <span className="text-[12px] text-on-surface line-clamp-2 leading-snug">
                    {m ? m.title ?? f.file_name : "正在读取标题…"}
                  </span>
                  <span className="text-[10px] text-outline truncate">{f.file_name}</span>
                  <span className="mt-auto flex items-center gap-1.5 text-[10px] text-outline">
                    <span>{dateText(f.modified_at)}</span>
                    {m?.chars != null ? <span>· {m.chars} 字</span> : null}
                    {m?.error ? <span className="text-error truncate">· 读取失败</span> : null}
                    {m && !m.tag ? <span className="px-1 rounded bg-surface-container">无元数据</span> : null}
                  </span>
                </button>
              );
            })}
          </div>
        ) : (
          <ul className="flex flex-col gap-0.5">
            {visible.map((f) => {
              return (
                <li key={f.relative_path} className="flex items-center gap-2">
                  <button
                    className="mono text-[11px] text-on-surface-variant truncate text-left hover:text-primary"
                    title={f.relative_path}
                    onClick={() => onOpen(f.relative_path)}
                  >
                    {f.file_name}
                  </button>
                  <span className="text-[10px] text-outline shrink-0">{dateText(f.modified_at)}</span>
                  <span className="text-[10px] text-outline shrink-0">{f.bytes} 字节</span>
                </li>
              );
            })}
          </ul>
        )}

        {pageCount > 1 ? (
          <div className="flex items-center gap-2 pt-2">
            <button
              className="text-[10.5px] px-2 py-0.5 rounded bg-surface-container disabled:opacity-40"
              disabled={current === 0}
              onClick={() => setPage(current - 1)}
            >
              上一页
            </button>
            <span className="text-[10px] text-outline">
              第 {current + 1} / {pageCount} 页
            </span>
            <button
              className="text-[10.5px] px-2 py-0.5 rounded bg-surface-container disabled:opacity-40"
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
