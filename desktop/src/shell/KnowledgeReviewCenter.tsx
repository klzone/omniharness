import { useEffect, useState } from "react";

import { shell } from "../api";
import type { KnowledgeReviewItem } from "../types";
import type { KnowledgeConflict } from "./knowledgeHub";

function timeText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "暂无";
  return new Date(seconds * 1000).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function queueKey(queue: "new" | "conflict", item: KnowledgeReviewItem | KnowledgeConflict) {
  return queue === "new" ? (item as KnowledgeReviewItem).id : (item as KnowledgeConflict).key;
}

export function KnowledgeReviewCenter({
  items,
  conflicts,
  busy,
  error,
  message,
  queue,
  onQueueChange,
  onDecide,
  onResolveConflict,
  onRefresh,
}: {
  items: KnowledgeReviewItem[];
  conflicts: KnowledgeConflict[];
  busy: boolean;
  error: string | null;
  message: string | null;
  queue: "new" | "conflict";
  onQueueChange: (queue: "new" | "conflict") => void;
  onDecide: (id: string, decision: "approve" | "request_changes" | "reject") => Promise<void>;
  onResolveConflict: (
    subject: string,
    decision: "resolved" | "deferred" | "needs_more",
    note?: string,
  ) => Promise<void>;
  onRefresh: () => void;
}) {
  const pending = items.filter((item) => item.status === "pending");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [quotes, setQuotes] = useState<Record<string, string>>({});

  useEffect(() => {
    const list: Array<KnowledgeReviewItem | KnowledgeConflict> = queue === "new" ? pending : conflicts;
    if (selectedKey && list.some((item) => queueKey(queue, item) === selectedKey)) return;
    setSelectedKey(list[0] ? queueKey(queue, list[0]) : null);
  }, [queue, items, conflicts, selectedKey, pending]);

  const selectedReview = queue === "new" ? pending.find((item) => item.id === selectedKey) ?? null : null;
  const selectedConflict = queue === "conflict" ? conflicts.find((item) => item.key === selectedKey) ?? null : null;

  useEffect(() => {
    if (!selectedReview) return;
    const readable = selectedReview.source_paths.filter((path) => /\.(md|markdown|txt)$/i.test(path));
    if (readable.length === 0) return;
    let cancelled = false;
    void Promise.all(
      readable.map(async (path) => {
        try {
          const note = await shell.knowledgeVaultReadNote(path);
          const body = note.content.replace(/^---[\s\S]*?---\s*/, "").trim();
          return [path, body.slice(0, 260)] as const;
        } catch (e) {
          return [path, `无法读取：${e instanceof Error ? e.message : String(e)}`] as const;
        }
      }),
    ).then((rows) => {
      if (!cancelled) setQuotes((prev) => ({ ...prev, ...Object.fromEntries(rows) }));
    });
    return () => {
      cancelled = true;
    };
  }, [selectedReview]);

  const decide = async (id: string, decision: "approve" | "request_changes" | "reject") => {
    if (busyAction) return;
    setBusyAction(`${id}:${decision}`);
    try {
      await onDecide(id, decision);
    } finally {
      setBusyAction(null);
    }
  };

  const resolve = async (subject: string, decision: "resolved" | "deferred" | "needs_more") => {
    if (busyAction) return;
    setBusyAction(`${subject}:${decision}`);
    try {
      await onResolveConflict(subject, decision);
    } finally {
      setBusyAction(null);
    }
  };

  const rows: Array<{ key: string; title: string; desc: string; meta: string; badge: string }> =
    queue === "new"
      ? pending.map((item) => ({
          key: item.id,
          title: item.cards[0]?.title || item.id,
          desc: `${item.card_count} 张卡 · ${item.source_paths.join("、") || "暂无来源"}`,
          meta: timeText(item.created_at),
          badge: item.rejected_cards.length > 0 ? `跳过 ${item.rejected_cards.length}` : "待确认",
        }))
      : conflicts.map((conflict) => ({
          key: conflict.key,
          title: conflict.title,
          desc: conflict.detail,
          meta: conflict.right ? "需对照" : "需检查",
          badge: conflict.kind === "duplicate" ? "同名" : conflict.kind === "link" ? "链接" : "异常",
        }));

  return (
    <section className="flex-1 min-h-0 overflow-y-auto" data-testid="knowledge-review-center">
      <div className="mb-3 flex items-baseline gap-3">
        <h2 className="text-[18px] font-semibold tracking-tight text-on-surface">知识审核</h2>
        <span className="mono text-[10px] text-outline">Review Center</span>
        <button
          type="button"
          className="ml-auto rounded-lg border border-outline-variant/60 bg-white/55 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
          onClick={onRefresh}
          disabled={busy}
          data-testid="review-refresh"
        >
          {busy ? "刷新中…" : "刷新队列"}
        </button>
      </div>

      {error ? (
        <p className="mb-2 rounded-xl bg-error-container/60 px-3 py-2 text-[11px] text-on-error-container" role="alert">
          {error}
        </p>
      ) : null}
      {message ? (
        <p className="mb-2 rounded-xl bg-surface-container-low px-3 py-2 text-[11px] text-on-surface-variant" role="status">
          {message}
        </p>
      ) : null}

      <div className="mb-3 flex gap-1 border-b border-outline-variant/45" role="tablist" aria-label="知识审核队列">
        <button
          type="button"
          role="tab"
          aria-selected={queue === "new"}
          data-testid="review-queue-tab-new"
          onClick={() => onQueueChange("new")}
          className={
            "border-b-2 px-3.5 py-2 text-[12.5px] " +
            (queue === "new" ? "border-primary font-medium text-primary" : "border-transparent text-on-surface-variant hover:text-on-surface")
          }
        >
          新知识确认
          <span className="ml-1.5 rounded-md bg-outline-variant/35 px-1.5 py-0.5 mono text-[9.5px]">{pending.length}</span>
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={queue === "conflict"}
          data-testid="review-queue-tab-conflict"
          onClick={() => onQueueChange("conflict")}
          className={
            "border-b-2 px-3.5 py-2 text-[12.5px] " +
            (queue === "conflict" ? "border-primary font-medium text-primary" : "border-transparent text-on-surface-variant hover:text-on-surface")
          }
        >
          冲突与异常
          <span className="ml-1.5 rounded-md bg-error-container px-1.5 py-0.5 mono text-[9.5px] text-on-error-container">{conflicts.length}</span>
        </button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-[20px] border border-white/80 bg-white/70 px-5 py-10 text-center text-[12px] text-outline">
          暂无待处理项。整理材料或刷新扫描后，结果会进入这里。
        </div>
      ) : (
        <div className="grid min-h-0 gap-3 lg:h-[calc(100%-7.5rem)] lg:grid-cols-[340px_minmax(0,1fr)]" data-testid={queue === "new" ? "review-queue-new" : "review-queue-conflict"}>
          <div className="min-h-0 overflow-hidden rounded-[20px] border border-white/80 bg-white/72 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]">
            <div className="flex items-baseline gap-2 border-b border-outline-variant/20 px-3.5 py-2.5">
              <h3 className="text-[12px] font-semibold text-on-surface">{queue === "new" ? "待确认队列" : "异常队列"}</h3>
              <span className="mono ml-auto text-[9.5px] text-outline">按时间排序</span>
            </div>
            <div className="max-h-[42vh] overflow-y-auto lg:max-h-[calc(100vh-16rem)]">
              {rows.map((row) => (
                <button
                  key={row.key}
                  type="button"
                  onClick={() => setSelectedKey(row.key)}
                  className={
                    "w-full border-b border-outline-variant/18 px-3.5 py-3 text-left transition-colors " +
                    (selectedKey === row.key ? "bg-primary/[0.07]" : "hover:bg-surface-container-low/60")
                  }
                  data-testid={queue === "new" ? "review-item" : "conflict-item"}
                  data-key={row.key}
                  data-id={queue === "new" ? row.key : undefined}
                  aria-current={selectedKey === row.key ? "true" : undefined}
                >
                  <span className="flex items-center gap-2">
                    <span className={"flex-none rounded px-1.5 py-0.5 mono text-[9px] " + (queue === "new" ? "bg-primary/10 text-primary" : "bg-error-container text-on-error-container")}>
                      {row.badge}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[11.5px] font-medium text-on-surface">{row.title}</span>
                    <span className="mono flex-none text-[9px] text-outline">{row.meta}</span>
                  </span>
                  <span className="mt-1 block truncate text-[10px] text-outline">{row.desc}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="flex min-h-0 flex-col overflow-hidden rounded-[20px] border border-white/80 bg-white/78 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]" data-testid={queue === "new" ? "review-detail" : "conflict-detail"}>
            {queue === "new" && selectedReview ? (
              <>
                <div className="flex-none border-b border-outline-variant/20 px-4 py-3">
                  <div className="flex items-center gap-2">
                    <span className="rounded-md bg-primary/10 px-2 py-0.5 mono text-[9.5px] text-primary">新增</span>
                    <h3 className="min-w-0 flex-1 truncate text-[14px] font-semibold text-on-surface">{selectedReview.cards[0]?.title || selectedReview.id}</h3>
                    <span className="mono text-[9.5px] text-outline">{timeText(selectedReview.created_at)}</span>
                  </div>
                  <p className="mt-1 text-[10.5px] text-outline">来源 {selectedReview.source_paths.join("、") || "暂无"}</p>
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
                  {selectedReview.parse_error ? <p className="mb-2 text-[11px] text-error">解析失败：{selectedReview.parse_error}</p> : null}
                  {selectedReview.rejected_cards.length > 0 || selectedReview.warnings.length > 0 ? (
                    <div className="mb-3 rounded-xl bg-amber-50 px-3 py-2 text-[10.5px] leading-relaxed text-amber-900" data-testid="review-stage-warnings">
                      {selectedReview.rejected_cards.length > 0 ? <p>已跳过 {selectedReview.rejected_cards.length} 张未通过宿主校验的卡片（不落盘）。</p> : null}
                      {selectedReview.rejected_cards.slice(0, 6).map((rejected) => (
                        <p key={rejected.relative_path} className="break-all">× {rejected.relative_path}：{rejected.reason}</p>
                      ))}
                      {selectedReview.warnings.slice(0, 6).map((warning) => (
                        <p key={warning} className="break-all">× {warning}</p>
                      ))}
                    </div>
                  ) : null}
                  <p className="mb-1.5 mono text-[9.5px] tracking-wider text-outline">AI 提取结果（供审核）</p>
                  {selectedReview.cards.map((card) => (
                    <div key={card.relative_path} className="mb-3 rounded-xl bg-white/90 px-3.5 py-3 shadow-[0_8px_20px_-18px_rgba(15,23,42,0.35)]">
                      <p className="text-[12px] font-semibold text-on-surface">{card.title}</p>
                      <p className="mono text-[9.5px] text-outline break-all">{card.relative_path}</p>
                      <pre className="mt-1.5 max-h-56 overflow-y-auto whitespace-pre-wrap break-words mono text-[10.5px] leading-relaxed text-on-surface-variant">{card.content}</pre>
                      {card.sources.map((source) => (
                        <div key={source} className="mt-1.5 rounded-lg border-l-2 border-primary-container bg-white/80 px-2.5 py-1.5">
                          <p className="mono text-[9.5px] text-primary break-all">{source}</p>
                          <p className="mt-0.5 text-[10.5px] leading-relaxed text-on-surface-variant" data-testid="review-source-quote">
                            {quotes[source] || "原文引用待读取（附件需在资产页打开）。"}
                          </p>
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
                <div className="flex flex-wrap items-center gap-2 border-t border-outline-variant/20 bg-white/60 px-4 py-3">
                  <button type="button" data-testid="review-approve" disabled={Boolean(busyAction) || busy} onClick={() => void decide(selectedReview.id, "approve")} className="rounded-lg bg-primary px-3.5 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45">{busyAction === `${selectedReview.id}:approve` ? "落盘中…" : "通过并纳入"}</button>
                  <button type="button" data-testid="review-request-changes" disabled={Boolean(busyAction) || busy} onClick={() => void decide(selectedReview.id, "request_changes")} className="rounded-lg border border-outline-variant/60 bg-white/75 px-3 py-1.5 text-[11.5px] text-on-surface-variant disabled:opacity-45">需要修改</button>
                  <button type="button" data-testid="review-reject" disabled={Boolean(busyAction) || busy} onClick={() => void decide(selectedReview.id, "reject")} className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-45">驳回</button>
                  <span className="ml-auto mono text-[9.5px] text-outline">尚未写入知识库</span>
                </div>
              </>
            ) : queue === "conflict" && selectedConflict ? (
              <>
                <div className="flex-none border-b border-outline-variant/20 px-4 py-3">
                  <div className="flex items-center gap-2">
                    <span className="rounded-md bg-error-container px-2 py-0.5 mono text-[9.5px] text-on-error-container">{selectedConflict.kind === "duplicate" ? "同名" : selectedConflict.kind === "link" ? "链接" : "异常"}</span>
                    <h3 className="min-w-0 flex-1 truncate text-[14px] font-semibold text-on-surface">{selectedConflict.title}</h3>
                  </div>
                  <p className="mt-1 text-[10.5px] text-outline">{selectedConflict.detail}</p>
                </div>
                <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-4 py-3">
                  {selectedConflict.left ? <div className="rounded-xl border-l-2 border-primary/60 bg-white/85 px-3 py-2.5 text-[11px] text-on-surface-variant break-all">来源 A：{selectedConflict.left}</div> : null}
                  {selectedConflict.right ? <div className="rounded-xl border-l-2 border-error/50 bg-white/85 px-3 py-2.5 text-[11px] text-on-surface-variant break-all">来源 B：{selectedConflict.right}</div> : null}
                  <p className="text-[10.5px] leading-relaxed text-outline">{selectedConflict.detail}</p>
                </div>
                <div className="flex flex-wrap items-center gap-2 border-t border-outline-variant/20 bg-white/60 px-4 py-3">
                  <button type="button" data-testid="conflict-resolve" disabled={Boolean(busyAction)} onClick={() => void resolve(selectedConflict.key, "resolved")} className="rounded-lg bg-primary px-3.5 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45">标记已处理</button>
                  <button type="button" data-testid="conflict-defer" disabled={Boolean(busyAction)} onClick={() => void resolve(selectedConflict.key, "deferred")} className="rounded-lg border border-outline-variant/60 bg-white/75 px-3 py-1.5 text-[11.5px] text-on-surface-variant disabled:opacity-45">保留待定</button>
                  <button type="button" data-testid="conflict-needs-more" disabled={Boolean(busyAction)} onClick={() => void resolve(selectedConflict.key, "needs_more")} className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-45">需要更多依据</button>
                </div>
              </>
            ) : (
              <div className="flex flex-1 items-center justify-center px-6 text-center text-[12px] text-outline">从左侧选择一条记录查看详情。</div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
