import { useEffect, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { TaskListing, WorkTask, WorkTaskState } from "../types";

type Notice = { kind: "ok" | "err"; text: string };

const ALL_STATES: WorkTaskState[] = [
  "待确认",
  "待开始",
  "进行中",
  "已完成",
  "已取消",
  "暂停",
];

const STATE_CLASS: Record<WorkTaskState, string> = {
  待确认: "bg-primary-fixed/70 text-on-primary-fixed-variant",
  待开始: "bg-surface-container text-on-surface-variant",
  进行中: "bg-primary/10 text-primary",
  已完成: "bg-secondary-container/60 text-on-secondary-container",
  已取消: "bg-surface-container text-outline",
  暂停: "bg-tertiary-fixed/60 text-on-tertiary-fixed-variant",
};

/** 与宿主 task_store 的白名单保持一致；宿主仍是最终裁决者。 */
const NEXT_STATES: Record<WorkTaskState, WorkTaskState[]> = {
  待确认: ["待开始", "已取消"],
  待开始: ["进行中", "暂停", "已取消"],
  进行中: ["已完成", "暂停", "已取消"],
  已完成: [],
  已取消: [],
  暂停: ["进行中", "已取消"],
};

const CARD = "bg-white/80 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "rounded-full border border-outline-variant/50 bg-white px-3 py-1.5 text-[11.5px] text-on-surface-variant focus:outline-none focus:ring-1 focus:ring-primary/30";

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function shortTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    date.getMonth() +
    1 +
    "-" +
    date.getDate() +
    " " +
    pad(date.getHours()) +
    ":" +
    pad(date.getMinutes())
  );
}

function StateChip({ state }: { state: WorkTaskState }) {
  return (
    <span
      className={"shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium " + STATE_CLASS[state]}
      data-task-state={state}
    >
      {state}
    </span>
  );
}

export function TaskManager({ onOpenRecord }: { onOpenRecord: (recordId: string) => void }) {
  const [listing, setListing] = useState<TaskListing>({ items: [], broken: 0 });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [stateFilter, setStateFilter] = useState<WorkTaskState | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [busy, setBusy] = useState(false);
  const [targetState, setTargetState] = useState<WorkTaskState>("待开始");
  const [note, setNote] = useState("");
  const [progressText, setProgressText] = useState("");

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .taskList(stateFilter ? { state: stateFilter } : {})
      .then((result) => {
        if (!alive) return;
        setListing(result);
        setLoadError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setLoadError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [stateFilter, reloadTick]);

  // 列表刷新后保留仍可见的选中项；筛选把选中项排除时，自动落到第一条。
  useEffect(() => {
    const stillVisible = selectedId !== null && listing.items.some((task) => task.id === selectedId);
    if (stillVisible) return;
    setSelectedId(listing.items.length > 0 ? listing.items[0].id : null);
  }, [listing.items, selectedId]);

  const selected: WorkTask | null =
    listing.items.find((task) => task.id === selectedId) ?? null;

  useEffect(() => {
    if (!selected) return;
    const next = NEXT_STATES[selected.state];
    setTargetState(next.length > 0 ? next[0] : selected.state);
    setNote("");
  }, [selectedId]);

  async function changeState() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const saved = await shell.taskTransition(selected.id, targetState, note || undefined);
      setSelectedId(saved.task.id);
      setNote("");
      const next = NEXT_STATES[saved.task.state];
      setTargetState(next.length > 0 ? next[0] : saved.task.state);
      setNotice({ kind: "ok", text: "已变更为 " + saved.task.state + "。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function addProgress() {
    if (!selected || busy || !progressText.trim()) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.taskAppendProgress(selected.id, progressText);
      setProgressText("");
      setNotice({ kind: "ok", text: "已添加进展。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
      {notice ? (
        <div
          role="status"
          data-task-error={notice.kind === "err" ? "" : undefined}
          className={
            "max-w-7xl mx-auto md:-translate-x-8 mb-3 rounded-2xl px-4 py-2 text-[11.5px] leading-relaxed " +
            (notice.kind === "err"
              ? "bg-error-container/70 text-on-error-container ring-1 ring-error/30"
              : "bg-surface-container text-on-surface-variant ring-1 ring-outline-variant/40")
          }
        >
          {notice.text}
        </div>
      ) : null}

      <div className="max-w-7xl mx-auto md:-translate-x-8 grid grid-cols-1 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)] gap-4 lg:gap-5 items-start">
        <div className="min-w-0 flex flex-col gap-4">
          <section aria-label="按状态筛选" className={CARD + " px-3.5 py-3"}>
            <div className="flex flex-wrap items-center gap-1.5" data-task-filter="">
              <button
                onClick={() => setStateFilter(null)}
                className={
                  "rounded-full px-3 py-1.5 text-[11px] font-medium transition-colors " +
                  (stateFilter === null
                    ? "bg-primary text-on-primary"
                    : "text-on-surface-variant hover:bg-surface-container")
                }
              >
                全部
              </button>
              {ALL_STATES.map((state) => (
                <button
                  key={state}
                  onClick={() => setStateFilter(state)}
                  className={
                    "rounded-full px-3 py-1.5 text-[11px] font-medium transition-colors " +
                    (stateFilter === state
                      ? "bg-primary text-on-primary"
                      : "text-on-surface-variant hover:bg-surface-container")
                  }
                >
                  {state}
                </button>
              ))}
            </div>
          </section>

          <section
            aria-label="任务列表"
            className={CARD + " px-3 py-3 flex flex-col min-h-[320px] max-h-[66vh]"}
          >
            <div className="flex items-baseline gap-2 px-1.5 pb-2.5 border-b border-surface-variant/25">
              <h2 className="text-[15px] font-semibold tracking-tight text-on-surface">任务</h2>
              <span className="mono text-[10.5px] text-outline ml-auto">
                {listing.items.length} 条
              </span>
            </div>
            {loadError ? (
              <p className="px-1.5 pt-3 text-[11.5px] leading-relaxed text-error">
                读取任务失败：{loadError}
              </p>
            ) : loading ? (
              <p className="px-1.5 pt-3 text-[11.5px] text-outline">正在读取任务…</p>
            ) : (
              <div className="pt-2 flex-1 min-h-0 overflow-y-auto flex flex-col gap-1" data-task-list="">
                {listing.items.length === 0 ? (
                  <p className="px-1.5 pt-3 text-[12px] text-outline">还没有任务。</p>
                ) : (
                  listing.items.map((task) => {
                    const active = task.id === selectedId;
                    return (
                      <button
                        key={task.id}
                        onClick={() => setSelectedId(task.id)}
                        data-task-row={task.id}
                        className={
                          "w-full text-left px-3 py-2.5 rounded-xl flex items-center gap-3 transition-colors " +
                          (active ? "bg-primary-fixed/60" : "hover:bg-surface-container/60")
                        }
                      >
                        <span className="min-w-0 flex-1">
                          <span className="block text-[12.5px] font-medium text-on-surface truncate">
                            {task.title}
                          </span>
                          <span className="block mt-0.5 mono text-[10px] text-outline">
                            {shortTime(task.updated_at)}
                          </span>
                        </span>
                        <StateChip state={task.state} />
                      </button>
                    );
                  })
                )}
                {listing.broken > 0 ? (
                  <p className="px-1.5 pt-2 text-[10.5px] text-outline">
                    {listing.broken} 个文件无法读取，未计入列表。
                  </p>
                ) : null}
              </div>
            )}
          </section>
        </div>

        <section
          aria-label="任务详情"
          className={CARD + " px-4 py-4 lg:px-5 lg:py-5 lg:sticky lg:top-2 max-h-[78vh] flex flex-col"}
          data-task-detail=""
        >
          {!selected ? (
            <p className="text-[12px] text-outline">选择一个任务查看详情。</p>
          ) : (
            <div className="flex flex-col min-h-0">
              <div className="flex items-start gap-2.5">
                <h2 className="min-w-0 flex-1 text-[18px] font-semibold leading-snug tracking-tight text-on-surface" data-hover-hint={selected.title}>
                  {selected.title}
                </h2>
                <StateChip state={selected.state} />
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-outline">
                {selected.due_date ? (
                  <span className="flex items-center gap-1">
                    <Icon name="calendar" className="h-3.5 w-3.5" />
                    截止 {selected.due_date}
                  </span>
                ) : null}
                {selected.category ? <span>{selected.category}</span> : null}
                {selected.source_record_id ? (
                  <button
                    onClick={() => {
                      const recordId = selected.source_record_id;
                      if (recordId) onOpenRecord(recordId);
                    }}
                    className="text-primary hover:underline"
                    data-task-source=""
                  >
                    来源记录
                  </button>
                ) : null}
              </div>
              {selected.source_quote ? (
                <blockquote className="mt-2.5 rounded-xl bg-surface-container/60 px-3 py-2 text-[11.5px] leading-relaxed text-on-surface-variant">
                  {selected.source_quote}
                </blockquote>
              ) : null}

              <div className="mt-3 flex-1 min-h-0 overflow-y-auto flex flex-col gap-3">
                <section aria-label="进展">
                  <h3 className="text-[12px] font-semibold text-on-surface">进展</h3>
                  {selected.progress.length === 0 ? (
                    <p className="mt-1 text-[11px] text-outline">还没有进展。</p>
                  ) : (
                    <div className="mt-1.5 flex flex-col gap-1.5" data-task-progress="">
                      {selected.progress.map((entry, index) => (
                        <div key={index} className="flex gap-2 items-baseline">
                          <span className="mono text-[10px] text-outline shrink-0">
                            {shortTime(entry.at)}
                          </span>
                          <span className="text-[11.5px] leading-relaxed text-on-surface-variant">
                            {entry.text}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                  <div className="mt-2 flex items-end gap-2">
                    <textarea
                      value={progressText}
                      onChange={(e) => setProgressText(e.target.value)}
                      placeholder="写一条进展…"
                      className="min-h-[36px] flex-1 resize-y rounded-xl border border-outline-variant/50 bg-white px-3 py-2 text-[11.5px] leading-relaxed text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30"
                    />
                    <button
                      onClick={() => void addProgress()}
                      disabled={busy || !progressText.trim()}
                      className="rounded-full bg-primary px-3 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
                    >
                      添加
                    </button>
                  </div>
                </section>

                <section aria-label="状态历史">
                  <h3 className="text-[12px] font-semibold text-on-surface">状态历史</h3>
                  {selected.history.length === 0 ? (
                    <p className="mt-1 text-[11px] text-outline">还没有状态变更。</p>
                  ) : (
                    <div className="mt-1.5 border-l border-outline-variant/40 ml-1 pl-3 flex flex-col gap-2.5" data-task-history="">
                      {selected.history.map((change, index) => (
                        <div key={index} className="relative">
                          <span className="absolute -left-[17px] top-1.5 h-2 w-2 rounded-full bg-primary/60" />
                          <div className="text-[11.5px] text-on-surface-variant">
                            <span className="font-medium text-on-surface">{change.from}</span>
                            {" → "}
                            <span className="font-medium text-on-surface">{change.to}</span>
                          </div>
                          <div className="mono text-[10px] text-outline">{shortTime(change.at)}</div>
                          {change.note ? (
                            <div className="mt-0.5 text-[11px] leading-relaxed text-on-surface-variant">
                              {change.note}
                            </div>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  )}
                </section>
              </div>

              <div className="mt-3 pt-3 border-t border-surface-variant/25 flex flex-wrap items-center gap-2">
                <select
                  value={targetState}
                  onChange={(e) => setTargetState(e.target.value as WorkTaskState)}
                  aria-label="目标状态"
                  className={FIELD}
                >
                  {ALL_STATES.map((state) => (
                    <option key={state} value={state}>
                      {state}
                    </option>
                  ))}
                </select>
                <input
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                  placeholder="备注（可选）"
                  aria-label="状态备注"
                  className={FIELD + " min-w-[160px] flex-1"}
                />
                <button
                  onClick={() => void changeState()}
                  disabled={busy}
                  className="rounded-full bg-primary px-4 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
                >
                  变更
                </button>
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
