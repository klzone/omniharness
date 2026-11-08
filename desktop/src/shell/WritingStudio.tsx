// 写作页三栏（OD-ARCH-094 S02/S04）。
//
// 视觉定位：创作台与知识库统一为 Aetheria 浅色卡面、靛青主色和细分隔线；
// 区块标题只用中文；状态用短标签，不再叠英文副标题或解释性说明。
//
// 边界：本组件不承载编辑器，也不承载对话；打开成果只读宿主文件，续写回首页对话。

import { useEffect, useMemo, useRef, useState } from "react";
import { shell } from "../api";
import type { SceneThread } from "../sceneThreads";
import type { OfflineArtifactEntry } from "../types";
import {
  WRITING_STATUSES,
  continueTarget,
  fallbackTitle,
  groupByStatus,
  isWritingFileStale,
  relativeUpdated,
  shortDate,
  staleFiles,
  statusLabel,
  typeLabel,
  writingStatus,
  titleFromContent,
  type WritingStatus,
} from "../writingStudio";

const CARD =
  "bg-white/78 " +
  "border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)] " +
  "px-4 py-4 lg:px-5 lg:py-5 min-w-0";

function CardHead({
  title,
  count,
}: {
  title: string;
  count?: string;
}) {
  return (
    <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
      <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
        {title}
      </h2>
      {count ? (
        <span className="mono text-[10.5px] text-outline ml-auto shrink-0">{count}</span>
      ) : null}
    </div>
  );
}
function TypeChip({ value }: { value: string | null | undefined }) {
  const label = typeLabel(value);
  return (
    <span
      className="shrink-0 px-1.5 py-[1px] rounded-full text-[9.5px] text-primary bg-primary/10"
      data-hover-hint={"内容类型：" + label}
      data-type-label={label}
    >
      {label}
    </span>
  );
}

function StatusChip({ value }: { value: string | null | undefined }) {
  const status = writingStatus(value);
  const skin =
    status === "draft"
      ? "text-amber-700 bg-amber-500/10"
      : status === "published"
        ? "text-tertiary bg-tertiary/10"
        : "text-on-surface-variant bg-surface-container-high";
  return (
    <span
      className={"shrink-0 px-1.5 py-[1px] rounded-full text-[9.5px] " + skin}
      data-status-label={status}
    >
      {statusLabel(value)}
    </span>
  );
}

/**
 * 左栏上：继续创作。
 * 取最近修改、且没有标记已发布的成果；没有成果时给出去首页开新对话的入口，
 * 不在这里伪造一篇草稿。
 */
export function WritingContinueCard({
  files,
  nowSeconds,
  loading = false,
  onOpen,
  onContinueThread,
  onNewThread,
}: {
  files: OfflineArtifactEntry[];
  nowSeconds: number;
  /** 成果元数据还没读到：给占位，不说成「还没有稿件」。 */
  loading?: boolean;
  onOpen: (relativePath: string) => void;
  onContinueThread: () => void;
  onNewThread: () => void;
}) {
  const target = continueTarget(files);
  /** 正文标题只读目标这一份文件；读不到就退回文件名缩写，不阻塞首屏。 */
  const [targetTitle, setTargetTitle] = useState<string | null>(null);
  const targetPath = target?.relative_path ?? null;
  useEffect(() => {
    if (!targetPath) {
      setTargetTitle(null);
      return;
    }
    let alive = true;
    shell
      .readOfflineArtifact(targetPath)
      .then((r) => {
        if (alive) setTargetTitle(titleFromContent(r.content));
      })
      .catch(() => {
        if (alive) setTargetTitle(null);
      });
    return () => {
      alive = false;
    };
  }, [targetPath]);
  return (
    <section aria-label="继续创作" className={CARD} data-writing-continue="">
      <CardHead title="继续创作" count={loading ? "读取中" : target ? "最近一稿" : "待开始"} />
      {loading ? (
        <p className="mt-3.5 text-[12px] text-outline leading-relaxed">正在读取本机成果…</p>
      ) : target ? (
        <div
          className="mt-3.5 rounded-2xl border border-amber-500/20 bg-white/82 px-4 py-4"
          data-continue-target={target.relative_path}
        >
          <div className="flex items-center gap-1.5 flex-wrap">
            <StatusChip value={target.status} />
            <TypeChip value={target.artifact_type} />
            <span className="mono ml-auto text-[10px] text-outline">
              {relativeUpdated(target.modified_at, nowSeconds)}
            </span>
          </div>
          <p
            className="mt-2.5 font-headline-md text-[15px] leading-[1.5] text-on-surface line-clamp-2 break-words"
            data-hover-hint={target.relative_path}
          >
            {targetTitle || fallbackTitle(target.file_name)}
          </p>
          <p className="mt-1 text-[10.5px] text-outline truncate" data-hover-hint={target.relative_path}>
            {shortDate(target.modified_at)} · {target.file_name}
          </p>
          <div className="mt-3 flex items-center gap-2">
            <button
              className="px-3 py-1.5 rounded-full text-[11px] bg-on-surface text-inverse-on-surface hover:opacity-90"
              onClick={() => onOpen(target.relative_path)}
            >
              打开这一稿
            </button>
            <button
              className="px-3 py-1.5 rounded-full text-[11px] border border-primary/30 text-primary hover:bg-primary/10"
              onClick={onContinueThread}
              data-hover-hint="回首页，接着最近一段写作对话往下写"
            >
              继续对话
            </button>
          </div>
        </div>
      ) : (
        <div className="mt-3.5">
          <p className="text-[12px] text-on-surface-variant leading-relaxed">还没有稿件。</p>
          <button
            className="mt-3 px-3.5 py-1.5 rounded-full text-[11px] bg-primary text-on-primary hover:opacity-90"
            onClick={onNewThread}
          >
            去写作
          </button>
        </div>
      )}
    </section>
  );
}

/**
 * 左栏下：写作对话。
 * 数据就是既有的 writing 场景会话，不新建存储；常显、可继续对话。
 */
export function WritingInspirationShelf({
  threads,
  nowSeconds,
  onOpenThread,
  onNewThread,
}: {
  threads: SceneThread[];
  nowSeconds: number;
  onOpenThread: (id: string) => void;
  onNewThread: () => void;
}) {
  return (
    <section aria-label="写作对话" className={CARD} data-writing-inspiration="">
      <CardHead
        title="写作对话"
        count={threads.length > 0 ? `${threads.length} 段` : "空"}
      />
      {threads.length === 0 ? (
        <div className="pt-3.5">
          <p className="text-[11.5px] text-outline leading-relaxed">还没有写作对话。</p>
          <button
            className="mt-2.5 text-[11px] text-primary hover:underline"
            onClick={onNewThread}
          >
            新对话
          </button>
        </div>
      ) : (
        <ul className="pt-3 flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
          {threads.slice(0, 12).map((t, i) => (
            <li key={t.id}>
              <button
                onClick={() => onOpenThread(t.id)}
                className="group w-full text-left rounded-xl px-2.5 py-2 hover:bg-surface-container-low border border-transparent hover:border-primary/20 min-w-0"
                data-hover-hint={t.title}
                data-inspiration-thread={t.id}
              >
                <span className="flex items-baseline gap-2 min-w-0">
                  <span className="mono shrink-0 text-[9.5px] text-outline/70">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="min-w-0 flex-1 text-[12px] text-on-surface line-clamp-2 leading-snug break-words">
                    {t.title || "未命名对话"}
                  </span>
                </span>
                <span className="mt-1 flex items-center gap-2 pl-[22px] text-[10px] text-outline">
                  <span>{relativeUpdated(t.updatedAt, nowSeconds)}</span>
                  <span className="ml-auto text-primary opacity-0 group-hover:opacity-100">
                    继续对话 →
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * 中栏：稿件。
 * 组头同时是筛选器（再点一次取消）；缺省口径直接写在组头下面，
 * 「超过 90 天未动」的提醒可以独立筛出条目。
 */
export function WritingPipeline({
  files,
  nowSeconds,
  metadataLoading = false,
  statusFilter,
  staleOnly,
  onStatusFilter,
  onStaleToggle,
  onOpen,
}: {
  files: OfflineArtifactEntry[];
  nowSeconds: number;
  /** 成果元数据还没读到：三组计数显示「—」，不显示 0。 */
  metadataLoading?: boolean;
  statusFilter: WritingStatus | null;
  staleOnly: boolean;
  onStatusFilter: (next: WritingStatus | null) => void;
  onStaleToggle: () => void;
  onOpen: (relativePath: string) => void;
}) {
  const groups = groupByStatus(files);
  const stale = staleFiles(files, nowSeconds);
  const stalePaths = new Set(stale.map((s) => s.relative_path));
  const total = files.filter((f) => !f.is_dir).length;

  /** 视图派生的筛选：先按状态分组，再按「仅看久未修改」收窄；不修改原数据。 */
  const base = statusFilter ? groups[statusFilter] : files.filter((f) => !f.is_dir);
  const visible = staleOnly ? base.filter((f) => stalePaths.has(f.relative_path)) : base;
  const sorted = [...visible].sort((a, b) => b.modified_at - a.modified_at);

  /**
   * 标题懒加载：只读当前列表里需要的文件，读完缓存；失败就退回文件名缩写。
   * 与右栏成果库各自读取同一份只读命令，不新增通道，也不把正文留在状态里。
   */
  const [titles, setTitles] = useState<Record<string, string | null>>({});
  const loadingRef = useRef<Set<string>>(new Set());
  const wantedKey = sorted.slice(0, 24).map((f) => f.relative_path).join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const wanted = useMemo(() => sorted.slice(0, 24), [wantedKey]);
  useEffect(() => {
    let alive = true;
    const todo = wanted.filter((f) => !(f.relative_path in titles) && !loadingRef.current.has(f.relative_path));
    if (todo.length === 0) return;
    const run = async () => {
      for (const f of todo) {
        if (!alive) return;
        loadingRef.current.add(f.relative_path);
        try {
          const r = await shell.readOfflineArtifact(f.relative_path);
          if (!alive) return;
          setTitles((prev) => ({ ...prev, [f.relative_path]: titleFromContent(r.content) }));
        } catch {
          if (!alive) return;
          setTitles((prev) => ({ ...prev, [f.relative_path]: null }));
        } finally {
          loadingRef.current.delete(f.relative_path);
        }
      }
    };
    void run();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wantedKey]);

  const chip = (active: boolean) =>
    "px-2.5 py-1 rounded-full text-[11px] leading-none transition border " +
    (active
      ? "bg-on-surface text-inverse-on-surface border-on-surface"
      : "bg-surface-container-low text-on-surface-variant border-transparent hover:bg-surface-container");

  return (
    <section aria-label="稿件" className={CARD + " flex flex-col"} data-writing-pipeline="">
      <CardHead
        title="稿件"
        count={metadataLoading ? "读取中" : `${total} 份稿件`}
      />

      {/*
       * 稿件流程：三格阶段 + 连接符，先给出「一批稿子在哪一段」的整体画面，
       * 再往下才是条目。点击任一阶段收窄列表，再点一次取消。
       */}
      <div className="pt-3.5 grid grid-cols-3 gap-1.5" data-pipeline-stages="">
        {WRITING_STATUSES.map((s, i) => {
          const active = statusFilter === s.key;
          const tone =
            s.key === "draft"
              ? "border-amber-500/25 bg-amber-500/5"
              : s.key === "published"
                ? "border-tertiary/20 bg-tertiary/5"
                : "border-primary/20 bg-primary/5";
          const numTone =
            s.key === "draft" ? "text-amber-700" : s.key === "published" ? "text-tertiary" : "text-primary";
          return (
            <button
              key={s.key}
              onClick={() => onStatusFilter(active ? null : s.key)}
              aria-pressed={active}
              data-status-filter={s.key}
              data-hover-hint={s.hint}
              className={
                "relative text-left rounded-xl border px-3 py-2.5 transition min-w-0 " +
                (active
                  ? "ring-2 ring-offset-1 ring-on-surface/25 " + tone
                  : tone + " hover:brightness-[0.99] hover:ring-1 hover:ring-on-surface/10")
              }
            >
              {i < WRITING_STATUSES.length - 1 ? (
                <span
                  aria-hidden="true"
                  className="absolute -right-[7px] top-1/2 z-10 -translate-y-1/2 text-[10px] text-outline/60"
                >
                  ›
                </span>
              ) : null}
              <span className="flex items-baseline gap-1.5">
                <span className={"font-headline-lg text-[22px] leading-none font-semibold tabular-nums " + numTone}>
                  {metadataLoading ? "—" : groups[s.key].length}
                </span>
                <span className="text-[10.5px] text-on-surface-variant">份</span>
              </span>
              <span className="mt-1 block text-[11.5px] font-medium text-on-surface">{s.label}</span>
            </button>
          );
        })}
      </div>

      <div className="pt-2 flex items-center gap-2">
        <button
          className={chip(statusFilter === null)}
          onClick={() => onStatusFilter(null)}
          aria-pressed={statusFilter === null}
        >
          全部 {total}
        </button>
        {statusFilter ? (
          <span className="text-[10px] text-outline truncate">只看{statusLabel(statusFilter)}</span>
        ) : null}
      </div>

      <div className="pt-2 flex items-center gap-2 min-w-0">
        {stale.length > 0 ? (
          <button
            className={
              "shrink-0 ml-auto text-[10.5px] rounded-full px-2 py-0.5 border transition " +
              (staleOnly
                ? "bg-amber-500/15 text-amber-700 border-amber-500/30"
                : "text-amber-700 border-amber-500/20 hover:bg-amber-500/10")
            }
            onClick={onStaleToggle}
            aria-pressed={staleOnly}
            data-stale-filter={staleOnly ? "on" : "off"}
          >
            {stale.length} 篇超过 90 天未动
          </button>
        ) : null}
      </div>

      <div className="mt-3.5 flex-1 min-h-0 max-h-[52vh] overflow-y-auto pr-0.5">
        {sorted.length === 0 ? (
          <p className="text-[11.5px] text-outline leading-relaxed">
            {statusFilter === null && !staleOnly
              ? "还没有稿件。"
              : staleOnly
                ? "没有超过 90 天未动的稿件，先放下这条提醒。"
                : "这一组还没有稿件。"}
          </p>
        ) : (
          <ul className="flex flex-col gap-2" data-pipeline-list="">
            {sorted.map((f, i) => {
              const staleFile = isWritingFileStale(f, nowSeconds);
              return (
                <li key={f.relative_path}>
                  <button
                    onClick={() => onOpen(f.relative_path)}
                    className={
                      "group w-full text-left rounded-xl border px-3.5 py-2.5 min-w-0 transition " +
                      (staleFile
                        ? "border-amber-500/20 bg-amber-500/5 hover:bg-amber-500/10"
                        : "border-surface-variant/25 bg-surface-container-low/40 hover:bg-surface-container-low hover:border-primary/25")
                    }
                    data-hover-hint={f.relative_path}
                    data-pipeline-item={f.relative_path}
                  >
                    <span className="flex items-baseline gap-2 min-w-0">
                      <span className="mono shrink-0 text-[10px] text-outline/70 w-5">
                        {String(i + 1).padStart(2, "0")}
                      </span>
                      <span className="min-w-0 flex-1 text-[12.5px] font-medium text-on-surface line-clamp-2 leading-snug break-words">
                        {titles[f.relative_path] || fallbackTitle(f.file_name)}
                      </span>
                      <StatusChip value={f.status} />
                    </span>
                    <span className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 pl-7 text-[10px] text-outline">
                      <TypeChip value={f.artifact_type} />
                      <span>{shortDate(f.modified_at)}</span>
                      {staleFile ? (
                        <span className="text-amber-700" data-stale-badge="">
                          90 天未动
                        </span>
                      ) : null}
                      <span className="ml-auto text-primary opacity-0 group-hover:opacity-100">
                        打开
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
