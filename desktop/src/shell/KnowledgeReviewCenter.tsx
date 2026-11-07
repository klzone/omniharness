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
  const [openId, setOpenId] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [quotes, setQuotes] = useState<Record<string, string>>({});
  const pending = items.filter((item) => item.status === "pending");

  useEffect(() => {
    const item = items.find((entry) => entry.id === openId && entry.status === "pending");
    if (!item) return;
    const readable = item.source_paths.filter((path) => /\.(md|markdown|txt)$/i.test(path));
    if (readable.length === 0) return;
    let cancelled = false;
    void Promise.all(
      readable.map(async (path) => {
        try {
          const note = await shell.knowledgeVaultReadNote(path);
          const body = note.content.replace(/^---[\s\S]*?---\s*/, "").trim();
          return [path, body.slice(0, 220)] as const;
        } catch (error) {
          return [path, `无法读取：${error instanceof Error ? error.message : String(error)}`] as const;
        }
      }),
    ).then((rows) => {
      if (cancelled) return;
      setQuotes((prev) => ({ ...prev, ...Object.fromEntries(rows) }));
    });
    return () => {
      cancelled = true;
    };
  }, [openId, items]);

  const decide = async (id: string, decision: "approve" | "request_changes" | "reject") => {
    if (busyAction) return;
    setBusyAction(`${id}:${decision}`);
    try {
      await onDecide(id, decision);
      setOpenId(null);
    } finally {
      setBusyAction(null);
    }
  };

  const resolve = async (
    subject: string,
    decision: "resolved" | "deferred" | "needs_more",
  ) => {
    if (busyAction) return;
    setBusyAction(`${subject}:${decision}`);
    try {
      await onResolveConflict(subject, decision);
      setOpenId(null);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <section className="flex-1 min-h-0 overflow-y-auto" data-testid="knowledge-review-center">
      <div className="flex items-baseline gap-3 mb-3">
        <h2 className="text-[18px] font-semibold tracking-tight text-on-surface">知识审核</h2>
        <span className="mono text-[10px] text-outline">Review Center</span>
        <button
          type="button"
          className="ml-auto rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary"
          onClick={onRefresh}
          disabled={busy}
          data-testid="review-refresh"
        >
          {busy ? "刷新中…" : "刷新队列"}
        </button>
      </div>

      {error ? (
        <p className="mb-2 rounded-lg bg-error-container/60 px-3 py-2 text-[11px] text-on-error-container" role="alert">
          {error}
        </p>
      ) : null}
      {message ? (
        <p className="mb-2 rounded-lg bg-surface-container-low px-3 py-2 text-[11px] text-on-surface-variant" role="status">
          {message}
        </p>
      ) : null}

      <div className="flex gap-1 border-b border-outline-variant/50 mb-3" role="tablist" aria-label="知识审核队列">
        <button
          type="button"
          role="tab"
          aria-selected={queue === "new"}
          data-testid="review-queue-tab-new"
          onClick={() => {
            onQueueChange("new");
            setOpenId(null);
          }}
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
          onClick={() => {
            onQueueChange("conflict");
            setOpenId(null);
          }}
          className={
            "border-b-2 px-3.5 py-2 text-[12.5px] " +
            (queue === "conflict" ? "border-primary font-medium text-primary" : "border-transparent text-on-surface-variant hover:text-on-surface")
          }
        >
          冲突与异常
          <span className="ml-1.5 rounded-md bg-error-container px-1.5 py-0.5 mono text-[9.5px] text-on-error-container">{conflicts.length}</span>
        </button>
      </div>

      {queue === "new" ? (
        <div className="space-y-2.5" data-testid="review-queue-new">
          {pending.length === 0 ? (
            <div className="rounded-[20px] border border-white/80 bg-white/70 px-5 py-8 text-center text-[12px] text-outline">
              暂无待确认的新知识。整理收件箱后，AI 提取结果会先进入这里。
            </div>
          ) : (
            pending.map((item) => {
              const open = openId === item.id;
              return (
                <article
                  key={item.id}
                  className="overflow-hidden rounded-[20px] border border-white/80 bg-white/70 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]"
                  data-testid="review-item"
                  data-id={item.id}
                >
                  <button
                    type="button"
                    className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]"
                    onClick={() => setOpenId(open ? null : item.id)}
                    aria-expanded={open}
                  >
                    <span className="flex-none rounded-md bg-primary/10 px-2 py-0.5 mono text-[9.5px] text-primary">新增</span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12.5px] font-medium text-on-surface">
                        {item.cards[0]?.title || item.id}
                      </span>
                      <span className="block truncate text-[10.5px] text-outline">
                        来源 {item.source_paths.join("、") || "暂无"} · AI 提取 {item.card_count} 张
                      </span>
                    </span>
                    <span className="mono text-[9.5px] text-outline">{timeText(item.created_at)}</span>
                    <span className="text-[12px] text-outline">›</span>
                  </button>
                  {open ? (
                    <div className="border-t border-outline-variant/30 bg-surface-container-low/45 px-4 py-3" data-testid="review-detail">
                      {item.parse_error ? (
                        <p className="mb-2 text-[11px] text-error">解析失败：{item.parse_error}</p>
                      ) : null}
                      {item.rejected_cards.length > 0 || item.warnings.length > 0 ? (
                        <div className="mb-2 rounded-lg bg-amber-50 px-2.5 py-2 text-[10.5px] leading-relaxed text-amber-900" data-testid="review-stage-warnings">
                          {item.rejected_cards.length > 0 ? (
                            <p>已跳过 {item.rejected_cards.length} 张未通过宿主校验的卡片（不落盘）。</p>
                          ) : null}
                          {item.rejected_cards.slice(0, 5).map((rejected) => (
                            <p key={rejected.relative_path} className="break-all">× {rejected.relative_path}：{rejected.reason}</p>
                          ))}
                          {item.warnings.slice(0, 5).map((warning) => (
                            <p key={warning} className="break-all">× {warning}</p>
                          ))}
                        </div>
                      ) : null}
                      <p className="mb-1.5 mono text-[9.5px] tracking-wider text-outline">AI 提取结果（供审核）</p>
                      {item.cards.map((card) => (
                        <div key={card.relative_path} className="mb-3 rounded-xl bg-white/85 px-3 py-2.5 shadow-sm">
                          <p className="text-[12px] font-semibold text-on-surface">{card.title}</p>
                          <p className="mono text-[9.5px] text-outline break-all">{card.relative_path}</p>
                          <pre className="mt-1.5 max-h-52 overflow-y-auto whitespace-pre-wrap break-words mono text-[10.5px] leading-relaxed text-on-surface-variant">
                            {card.content}
                          </pre>
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
                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          type="button"
                          data-testid="review-approve"
                          disabled={Boolean(busyAction) || busy}
                          onClick={() => void decide(item.id, "approve")}
                          className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
                        >
                          {busyAction === `${item.id}:approve` ? "落盘中…" : "通过并纳入"}
                        </button>
                        <button
                          type="button"
                          data-testid="review-request-changes"
                          disabled={Boolean(busyAction) || busy}
                          onClick={() => void decide(item.id, "request_changes")}
                          className="rounded-lg border border-outline-variant/60 bg-white/75 px-3 py-1.5 text-[11.5px] text-on-surface-variant disabled:opacity-45"
                        >
                          需要修改
                        </button>
                        <button
                          type="button"
                          data-testid="review-reject"
                          disabled={Boolean(busyAction) || busy}
                          onClick={() => void decide(item.id, "reject")}
                          className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-45"
                        >
                          驳回
                        </button>
                        <span className="ml-auto mono text-[9.5px] text-outline">尚未写入知识库</span>
                      </div>
                    </div>
                  ) : null}
                </article>
              );
            })
          )}
        </div>
      ) : (
        <div className="space-y-2.5" data-testid="review-queue-conflict">
          {conflicts.length === 0 ? (
            <div className="rounded-[20px] border border-white/80 bg-white/70 px-5 py-8 text-center text-[12px] text-outline">
              暂未发现同名条目、扫描异常或未解析链接。
            </div>
          ) : (
            conflicts.map((conflict) => {
              const open = openId === conflict.key;
              return (
                <article
                  key={conflict.key}
                  className="overflow-hidden rounded-[20px] border border-white/80 bg-white/70 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]"
                  data-testid="conflict-item"
                  data-key={conflict.key}
                >
                  <button
                    type="button"
                    className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]"
                    onClick={() => setOpenId(open ? null : conflict.key)}
                    aria-expanded={open}
                  >
                    <span className="flex-none rounded-md bg-error-container px-2 py-0.5 mono text-[9.5px] text-on-error-container">
                      {conflict.kind === "duplicate" ? "同名" : conflict.kind === "link" ? "链接" : "异常"}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12.5px] font-medium text-on-surface">{conflict.title}</span>
                      <span className="block truncate text-[10.5px] text-outline">{conflict.detail}</span>
                    </span>
                    <span className="text-[12px] text-outline">›</span>
                  </button>
                  {open ? (
                    <div className="border-t border-outline-variant/30 bg-surface-container-low/45 px-4 py-3" data-testid="conflict-detail">
                      {conflict.left ? (
                        <div className="mb-2 rounded-lg border-l-2 border-primary/60 bg-white/80 px-3 py-2 text-[11px] text-on-surface-variant break-all">
                          来源 A：{conflict.left}
                        </div>
                      ) : null}
                      {conflict.right ? (
                        <div className="mb-2 rounded-lg border-l-2 border-error/50 bg-white/80 px-3 py-2 text-[11px] text-on-surface-variant break-all">
                          来源 B：{conflict.right}
                        </div>
                      ) : null}
                      <p className="mb-2 text-[10.5px] leading-relaxed text-outline">{conflict.detail}</p>
                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          type="button"
                          data-testid="conflict-resolve"
                          disabled={Boolean(busyAction)}
                          onClick={() => void resolve(conflict.key, "resolved")}
                          className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
                        >
                          标记已处理
                        </button>
                        <button
                          type="button"
                          data-testid="conflict-defer"
                          disabled={Boolean(busyAction)}
                          onClick={() => void resolve(conflict.key, "deferred")}
                          className="rounded-lg border border-outline-variant/60 bg-white/75 px-3 py-1.5 text-[11.5px] text-on-surface-variant disabled:opacity-45"
                        >
                          保留待定
                        </button>
                        <button
                          type="button"
                          data-testid="conflict-needs-more"
                          disabled={Boolean(busyAction)}
                          onClick={() => void resolve(conflict.key, "needs_more")}
                          className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-45"
                        >
                          需要更多依据
                        </button>
                      </div>
                    </div>
                  ) : null}
                </article>
              );
            })
          )}
        </div>
      )}
    </section>
  );
}
