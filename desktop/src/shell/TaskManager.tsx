import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import { KnowledgeLinkPanel } from "./KnowledgeLinkPanel";
import type {
  AvailabilityConfig,
  ScheduleEntryView,
  ScheduleGenerateReport,
  TaskImportance,
  TaskListing,
  TaskQuadrant,
  TaskQuadrantView,
  WorkTask,
  WorkTaskState,
} from "../types";

type Notice = { kind: "ok" | "err"; text: string };
type TaskView = "list" | "quadrant" | "today" | "week";

const LOCAL_OFFSET_MINUTES = () => -new Date().getTimezoneOffset();

const TASK_VIEWS: Array<{ key: TaskView; label: string }> = [
  { key: "list", label: "任务" },
  { key: "quadrant", label: "四象限" },
  { key: "today", label: "今日" },
  { key: "week", label: "本周" },
];

const QUADRANT_COLUMNS: Array<{
  key: TaskQuadrant | "待评估";
  label: string;
  hint: string;
}> = [
  { key: "Q1", label: "重要且紧急", hint: "先做" },
  { key: "Q2", label: "重要不紧急", hint: "排期" },
  { key: "Q3", label: "紧急不重要", hint: "批量处理" },
  { key: "Q4", label: "不重要不紧急", hint: "少占时间" },
  { key: "待评估", label: "待评估", hint: "补齐重要性或截止" },
];

const IMPORTANCE_CHOICES: TaskImportance[] = ["高", "中", "低", "待评估"];
const QUADRANT_CHOICES: Array<TaskQuadrant | "自动"> = ["自动", "Q1", "Q2", "Q3", "Q4"];

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
  const [view, setView] = useState<TaskView>("list");
  const [quadrantViews, setQuadrantViews] = useState<TaskQuadrantView[]>([]);
  const [scheduleEntries, setScheduleEntries] = useState<ScheduleEntryView[]>([]);
  const [detailImportance, setDetailImportance] = useState<TaskImportance>("待评估");
  const [detailImportanceReason, setDetailImportanceReason] = useState("");
  const [detailQuadrant, setDetailQuadrant] = useState<TaskQuadrant | "自动">("自动");
  const [detailQuadrantReason, setDetailQuadrantReason] = useState("");
  const [estimateInput, setEstimateInput] = useState("");
  const [dependsInput, setDependsInput] = useState("");
  const [dueInput, setDueInput] = useState("");
  const [dueKind, setDueKind] = useState<"hard" | "target">("hard");
  const [dueReason, setDueReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [targetState, setTargetState] = useState<WorkTaskState>("待开始");
  const [note, setNote] = useState("");
  const [progressText, setProgressText] = useState("");

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([
      shell.taskList(stateFilter ? { state: stateFilter } : {}),
      shell.taskQuadrantViews(LOCAL_OFFSET_MINUTES()),
      shell.scheduleList(),
    ])
      .then(([result, quadrants, schedules]) => {
        if (!alive) return;
        setListing(result);
        setQuadrantViews(quadrants);
        setScheduleEntries(schedules.items);
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
  const selectedView = quadrantViews.find((item) => item.task_id === selectedId) ?? null;
  const selectedSchedules = scheduleEntries
    .filter((entry) => entry.task_id === selectedId)
    .sort((a, b) => a.start_at.localeCompare(b.start_at));
  const titles = new Map(listing.items.map((task) => [task.id, task.title]));

  useEffect(() => {
    if (!selected) return;
    const next = NEXT_STATES[selected.state];
    setTargetState(next.length > 0 ? next[0] : selected.state);
    setNote("");
    setDetailImportance(selected.importance);
    setDetailImportanceReason(selected.importance_reason ?? "");
    setDetailQuadrant(selected.quadrant_override ?? "自动");
    setDetailQuadrantReason("");
    setEstimateInput(selected.estimated_minutes ? String(selected.estimated_minutes) : "");
    setDependsInput(selected.depends_on.join(", "));
    setDueInput(selected.due_date ?? "");
    setDueKind(selected.due_kind);
    setDueReason("");
  }, [selectedId, selected?.updated_at]);

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

  async function saveDetailImportance() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.taskSetImportance(selected.id, detailImportance, detailImportanceReason.trim());
      setNotice({ kind: "ok", text: "重要性已更新，并记录调整理由。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveDetailQuadrant() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.taskSetQuadrant(
        selected.id,
        detailQuadrant === "自动" ? null : detailQuadrant,
        detailQuadrantReason.trim() || undefined,
      );
      setNotice({ kind: "ok", text: "象限已更新。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveEstimate() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const minutes = estimateInput.trim() === "" ? null : Number(estimateInput);
      await shell.taskSetEstimated(selected.id, minutes);
      setNotice({ kind: "ok", text: minutes === null ? "预计耗时已清除。" : "预计耗时已保存。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveDepends() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const dependsOn = dependsInput
        .split(/[,，\s]+/)
        .map((item) => item.trim())
        .filter(Boolean);
      await shell.taskSetDependency(selected.id, dependsOn);
      setNotice({ kind: "ok", text: "前置任务已更新。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveDue() {
    if (!selected || busy || !dueReason.trim()) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.taskSetDue(selected.id, dueInput.trim() || null, dueKind, dueReason.trim());
      setNotice({ kind: "ok", text: "截止时间已更新。" });
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

      <div className="max-w-7xl mx-auto md:-translate-x-8 mb-4 flex flex-wrap items-center gap-1.5" role="tablist" data-task-views="">
        {TASK_VIEWS.map((item) => (
          <button
            key={item.key}
            role="tab"
            aria-selected={view === item.key}
            onClick={() => setView(item.key)}
            className={
              "rounded-full px-3.5 py-1.5 text-[11.5px] font-medium transition-colors " +
              (view === item.key
                ? "bg-primary text-on-primary"
                : "text-on-surface-variant hover:bg-surface-container")
            }
          >
            {item.label}
          </button>
        ))}
      </div>

      {view === "list" ? (
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
                            {shortTime(task.updated_at)} · {task.id}
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

              <section aria-label="优先级与依据" className="mt-3" data-task-priority="">
                <h3 className="text-[12px] font-semibold text-on-surface">优先级与依据</h3>
                <div className="mt-1.5 flex flex-wrap items-center gap-1.5 text-[10.5px] text-on-surface-variant">
                  <span className="rounded-full bg-surface-container px-2 py-0.5">
                    重要 {selectedView?.importance ?? selected.importance}
                    {selected.importance_source ? ` · ${selected.importance_source}` : ""}
                  </span>
                  <span className="rounded-full bg-surface-container px-2 py-0.5">
                    紧急 {selectedView?.urgency ?? "待评估"}
                  </span>
                  <span className="rounded-full bg-surface-container px-2 py-0.5">
                    象限 {selectedView?.quadrant ?? "待评估"}
                  </span>
                  {selected.quadrant_override ? (
                    <span className="rounded-full bg-primary-fixed/70 px-2 py-0.5 text-on-primary-fixed-variant">
                      用户指定
                    </span>
                  ) : null}
                </div>
                <p className="mt-1 text-[10.5px] leading-relaxed text-outline">
                  重要依据：{selectedView?.importance_reason ?? selected.importance_reason ?? "待评估"}
                </p>
                <p className="text-[10.5px] leading-relaxed text-outline">
                  紧急依据：{selectedView?.urgency_reason ?? "待评估"}
                </p>
                <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
                  <div className="flex flex-col gap-1.5">
                    <label className="text-[10px] text-outline" htmlFor="detail-importance">重要性</label>
                    <select
                      id="detail-importance"
                      className={FIELD + " w-full"}
                      value={detailImportance}
                      onChange={(e) => setDetailImportance(e.target.value as TaskImportance)}
                    >
                      {IMPORTANCE_CHOICES.map((level) => (
                        <option key={level} value={level}>{level}</option>
                      ))}
                    </select>
                    <input
                      className={FIELD + " w-full"}
                      placeholder="理由（必填）"
                      value={detailImportanceReason}
                      onChange={(e) => setDetailImportanceReason(e.target.value)}
                      aria-label="重要性调整理由"
                    />
                    <button
                      className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                      disabled={busy || !detailImportanceReason.trim()}
                      onClick={() => void saveDetailImportance()}
                    >
                      保存重要性
                    </button>
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <label className="text-[10px] text-outline" htmlFor="detail-quadrant">象限指定</label>
                    <select
                      id="detail-quadrant"
                      className={FIELD + " w-full"}
                      value={detailQuadrant}
                      onChange={(e) => setDetailQuadrant(e.target.value as TaskQuadrant | "自动")}
                    >
                      {QUADRANT_CHOICES.map((choice) => (
                        <option key={choice} value={choice}>{choice}</option>
                      ))}
                    </select>
                    <input
                      className={FIELD + " w-full"}
                      placeholder="理由（可选）"
                      value={detailQuadrantReason}
                      onChange={(e) => setDetailQuadrantReason(e.target.value)}
                      aria-label="象限调整理由"
                    />
                    <button
                      className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                      disabled={busy}
                      onClick={() => void saveDetailQuadrant()}
                    >
                      保存象限
                    </button>
                  </div>
                </div>
              </section>

              <section aria-label="截止与预计耗时" className="mt-3" data-task-time="">
                <h3 className="text-[12px] font-semibold text-on-surface">截止与预计耗时</h3>
                <div className="mt-1.5 grid grid-cols-1 md:grid-cols-2 gap-2">
                  <div className="flex flex-col gap-1.5">
                    <input
                      className={FIELD + " w-full"}
                      type="date"
                      value={dueInput}
                      onChange={(e) => setDueInput(e.target.value)}
                      aria-label="截止日期"
                    />
                    <select
                      className={FIELD + " w-full"}
                      value={dueKind}
                      onChange={(e) => setDueKind(e.target.value as "hard" | "target")}
                      aria-label="截止性质"
                    >
                      <option value="hard">硬截止</option>
                      <option value="target">目标时间</option>
                    </select>
                    <input
                      className={FIELD + " w-full"}
                      placeholder="调整理由（必填）"
                      value={dueReason}
                      onChange={(e) => setDueReason(e.target.value)}
                      aria-label="截止调整理由"
                    />
                    <button
                      className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                      disabled={busy || !dueReason.trim()}
                      onClick={() => void saveDue()}
                    >
                      保存截止
                    </button>
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <input
                      className={FIELD + " w-full"}
                      type="number"
                      min={1}
                      max={1440}
                      placeholder="预计分钟数；留空清除"
                      value={estimateInput}
                      onChange={(e) => setEstimateInput(e.target.value)}
                      aria-label="预计耗时分钟"
                    />
                    <button
                      className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                      disabled={busy}
                      onClick={() => void saveEstimate()}
                    >
                      保存耗时
                    </button>
                  </div>
                </div>
              </section>

              <section aria-label="依赖与父子" className="mt-3" data-task-dependencies="">
                <h3 className="text-[12px] font-semibold text-on-surface">依赖与父子</h3>
                {selected.depends_on.length === 0 ? (
                  <p className="mt-1 text-[10.5px] text-outline">没有前置任务。</p>
                ) : (
                  <ul className="mt-1 flex flex-wrap gap-1.5 text-[10.5px]" data-task-depend-list="">
                    {selected.depends_on.map((id) => (
                      <li key={id} className="rounded-full bg-surface-container px-2 py-0.5">
                        前置：{titles.get(id) ?? id}
                      </li>
                    ))}
                  </ul>
                )}
                <p className="mt-1 text-[10.5px] text-outline">
                  父任务：{selected.parent_id ? titles.get(selected.parent_id) ?? selected.parent_id : "无"}
                </p>
                <div className="mt-1.5 flex items-end gap-2">
                  <input
                    className={FIELD + " min-w-0 flex-1"}
                    placeholder="前置任务编号，多个用逗号分隔"
                    value={dependsInput}
                    onChange={(e) => setDependsInput(e.target.value)}
                    aria-label="前置任务编号"
                  />
                  <button
                    className="rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                    disabled={busy}
                    onClick={() => void saveDepends()}
                  >
                    保存依赖
                  </button>
                </div>
              </section>

              <section aria-label="排期历史" className="mt-3" data-task-schedule-history="">
                <h3 className="text-[12px] font-semibold text-on-surface">排期历史与调整记录</h3>
                {selectedSchedules.length === 0 ? (
                  <p className="mt-1 text-[10.5px] text-outline">还没有排期记录。</p>
                ) : (
                  <div className="mt-1.5 flex flex-col gap-1.5">
                    {selectedSchedules.map((entry) => (
                      <div key={entry.id} className="rounded-lg bg-surface-container/40 px-2 py-1.5" data-task-schedule={entry.id}>
                        <div className="flex flex-wrap items-center gap-x-2 text-[10.5px]">
                          <span className="mono">{shortTime(entry.start_at)}–{localClock(entry.end_at)}</span>
                          <span className="rounded-full bg-white px-1.5 py-0.5">{entry.status}</span>
                          {entry.effective_status !== entry.status ? (
                            <span className="text-outline">派生：{entry.effective_status}</span>
                          ) : null}
                          {entry.is_locked ? <span className="text-outline">锁定</span> : null}
                          <span className="mono text-outline">{entry.duration}m</span>
                        </div>
                        {entry.adjustment_reason ? (
                          <p className="mt-0.5 text-[10px] text-outline">原因：{entry.adjustment_reason}</p>
                        ) : null}
                        {entry.adjustments.length > 0 ? (
                          <ul className="mt-0.5 flex flex-col gap-0.5 text-[10px] text-outline">
                            {entry.adjustments.map((record, index) => (
                              <li key={index}>
                                {shortTime(record.at)} · {record.action}
                                {record.reason ? ` · ${record.reason}` : ""}
                              </li>
                            ))}
                          </ul>
                        ) : null}
                      </div>
                    ))}
                  </div>
                )}
              </section>

              <KnowledgeLinkPanel
                subjectKind="task"
                subjectId={selected.id}
                links={selected.related_knowledge}
                onNotice={setNotice}
                onLinksChanged={(links) => {
                  setListing((prev) => ({
                    ...prev,
                    items: prev.items.map((item) =>
                      item.id === selected.id ? { ...item, related_knowledge: links } : item,
                    ),
                  }));
                }}
              />

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
      ) : null}

      {view === "quadrant" ? (
        <div className="max-w-7xl mx-auto md:-translate-x-8">
          <TaskQuadrantBoard onNotice={setNotice} />
        </div>
      ) : null}

      {view === "today" || view === "week" ? (
        <div className="max-w-7xl mx-auto md:-translate-x-8">
          <SchedulePlanner view={view} onNotice={setNotice} />
        </div>
      ) : null}
    </div>
  );
}

export function TaskQuadrantBoard({ onNotice }: { onNotice?: (notice: Notice | null) => void }) {
  const [views, setViews] = useState<TaskQuadrantView[]>([]);
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [schedule, setSchedule] = useState<ScheduleEntryView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [editing, setEditing] = useState<string | null>(null);
  const [importanceDraft, setImportanceDraft] = useState<TaskImportance>("待评估");
  const [importanceReason, setImportanceReason] = useState("");
  const [quadrantDraft, setQuadrantDraft] = useState<TaskQuadrant | "自动">("自动");
  const [quadrantReason, setQuadrantReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [localNotice, setLocalNotice] = useState<Notice | null>(null);

  function announce(notice: Notice | null) {
    setLocalNotice(notice);
    onNotice?.(notice);
  }

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([
      shell.taskQuadrantViews(LOCAL_OFFSET_MINUTES()),
      shell.taskList(),
      shell.scheduleList(),
    ])
      .then(([quadrants, listing, schedules]) => {
        if (!alive) return;
        setViews(quadrants);
        setTasks(listing.items);
        setSchedule(schedules.items);
        setError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [tick]);

  const taskMap = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);
  const byTask = useMemo(() => {
    const map = new Map<string, ScheduleEntryView[]>();
    for (const entry of schedule) {
      if (entry.effective_status === "已取消" || entry.effective_status === "已调整") continue;
      const list = map.get(entry.task_id) ?? [];
      list.push(entry);
      map.set(entry.task_id, list);
    }
    return map;
  }, [schedule]);

  function openEditor(view: TaskQuadrantView) {
    const task = taskMap.get(view.task_id);
    setEditing(view.task_id);
    setImportanceDraft(view.importance);
    setImportanceReason(task?.importance_reason ?? "");
    setQuadrantDraft(task?.quadrant_override ?? "自动");
    setQuadrantReason("");
    announce(null);
  }

  async function saveImportance(id: string) {
    setBusy(true);
    announce(null);
    try {
      await shell.taskSetImportance(id, importanceDraft, importanceReason.trim());
      announce({ kind: "ok", text: "重要性已更新，并记录调整理由。" });
      setEditing(null);
      setTick((n) => n + 1);
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveQuadrant(id: string) {
    setBusy(true);
    announce(null);
    try {
      await shell.taskSetQuadrant(
        id,
        quadrantDraft === "自动" ? null : quadrantDraft,
        quadrantReason.trim() || undefined,
      );
      announce({ kind: "ok", text: "象限已更新。" });
      setEditing(null);
      setTick((n) => n + 1);
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-label="任务四象限" data-task-quadrant="">
      {localNotice ? (
        <p
          role="status"
          className={
            "mb-3 rounded-2xl px-4 py-2 text-[11.5px] leading-relaxed " +
            (localNotice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-surface-container text-on-surface-variant")
          }
        >
          {localNotice.text}
        </p>
      ) : null}
      {error ? (
        <p className="mb-3 rounded-2xl bg-error-container/70 px-4 py-2 text-[11.5px] leading-relaxed text-on-error-container">
          读取四象限失败：{error}
        </p>
      ) : null}
      {loading ? <p className="mb-3 text-[12px] text-outline">正在读取任务…</p> : null}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-5 gap-3 items-start">
        {QUADRANT_COLUMNS.map((column) => {
          const items = views.filter((view) => view.quadrant === column.key);
          return (
            <section
              key={column.key}
              aria-label={column.label}
              data-quadrant-column={column.key}
              className={CARD + " px-3 py-3 min-w-0"}
            >
              <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/25">
                <h3 className="text-[13px] font-semibold tracking-tight text-on-surface">
                  {column.key} · {column.label}
                </h3>
                <span className="mono text-[10px] text-outline ml-auto">{items.length}</span>
              </div>
              <p className="pt-1.5 text-[10.5px] text-outline">{column.hint}</p>
              <div className="pt-2 flex flex-col gap-2">
                {items.length === 0 ? (
                  <p className="text-[11px] text-outline">暂无</p>
                ) : (
                  items.map((view) => {
                    const task = taskMap.get(view.task_id);
                    const entries = byTask.get(view.task_id) ?? [];
                    const confirmed = entries.some(
                      (entry) => entry.is_locked || entry.status === "已确认",
                    );
                    const suggested = entries.some((entry) => entry.status === "已建议");
                    return (
                      <article
                        key={view.task_id}
                        data-task-quadrant-row={view.task_id}
                        className={
                          "rounded-xl border border-outline-variant/40 bg-white/70 px-2.5 py-2 " +
                          (view.active ? "" : "opacity-70")
                        }
                      >
                        <div className="flex items-start gap-2">
                          <span className="min-w-0 flex-1 text-[12px] font-medium leading-snug text-on-surface">
                            {view.title}
                          </span>
                          {task ? <StateChip state={task.state} /> : null}
                        </div>
                        <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[10px] text-on-surface-variant">
                          <span className="rounded-full bg-surface-container px-1.5 py-0.5">重要 {view.importance}</span>
                          <span className="rounded-full bg-surface-container px-1.5 py-0.5">紧急 {view.urgency}</span>
                          {task?.due_date ? <span>截止 {task.due_date}</span> : null}
                          {view.is_override ? (
                            <span className="rounded-full bg-primary-fixed/70 px-1.5 py-0.5 text-on-primary-fixed-variant">
                              用户指定
                            </span>
                          ) : null}
                          {confirmed ? (
                            <span className="rounded-full bg-secondary-container/70 px-1.5 py-0.5">已确认</span>
                          ) : suggested ? (
                            <span className="rounded-full bg-surface-container px-1.5 py-0.5">已建议</span>
                          ) : null}
                          {!view.active ? <span className="text-outline">不参与待办</span> : null}
                        </div>
                        <p className="mt-1 text-[10px] leading-relaxed text-outline line-clamp-2" data-hover-hint={view.urgency_reason}>
                          {view.importance_reason ?? "重要性待评估"} · {view.urgency_reason}
                        </p>
                        <div className="mt-1.5 flex items-center gap-2">
                          <button
                            className="text-[10.5px] text-primary hover:underline"
                            onClick={() => openEditor(view)}
                          >
                            调整
                          </button>
                        </div>
                        {editing === view.task_id ? (
                          <div className="mt-2 pt-2 border-t border-surface-variant/25 flex flex-col gap-1.5">
                            <label className="text-[10px] text-outline" htmlFor={`imp-${view.task_id}`}>
                              重要性
                            </label>
                            <select
                              id={`imp-${view.task_id}`}
                              className={FIELD + " w-full"}
                              value={importanceDraft}
                              onChange={(e) => setImportanceDraft(e.target.value as TaskImportance)}
                            >
                              {IMPORTANCE_CHOICES.map((level) => (
                                <option key={level} value={level}>{level}</option>
                              ))}
                            </select>
                            <input
                              className={FIELD + " w-full"}
                              placeholder="理由（必填）"
                              value={importanceReason}
                              onChange={(e) => setImportanceReason(e.target.value)}
                              aria-label="重要性理由"
                            />
                            <button
                              className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                              disabled={busy || !importanceReason.trim()}
                              onClick={() => void saveImportance(view.task_id)}
                            >
                              保存重要性
                            </button>

                            <label className="mt-1 text-[10px] text-outline" htmlFor={`quad-${view.task_id}`}>
                              象限指定
                            </label>
                            <select
                              id={`quad-${view.task_id}`}
                              className={FIELD + " w-full"}
                              value={quadrantDraft}
                              onChange={(e) => setQuadrantDraft(e.target.value as TaskQuadrant | "自动")}
                            >
                              {QUADRANT_CHOICES.map((choice) => (
                                <option key={choice} value={choice}>{choice}</option>
                              ))}
                            </select>
                            <input
                              className={FIELD + " w-full"}
                              placeholder="理由（可选）"
                              value={quadrantReason}
                              onChange={(e) => setQuadrantReason(e.target.value)}
                              aria-label="象限调整理由"
                            />
                            <button
                              className="self-start rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                              disabled={busy}
                              onClick={() => void saveQuadrant(view.task_id)}
                            >
                              保存象限
                            </button>
                          </div>
                        ) : null}
                      </article>
                    );
                  })
                )}
              </div>
            </section>
          );
        })}
      </div>
    </section>
  );
}

function localDayKey(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 10);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function localClock(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function todayKey(): string {
  const date = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function weekDays(): Array<{ key: string; label: string }> {
  const today = new Date();
  const monday = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const offset = (today.getDay() + 6) % 7;
  monday.setDate(monday.getDate() - offset);
  const labels = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
  return labels.map((label, index) => {
    const date = new Date(monday);
    date.setDate(monday.getDate() + index);
    const pad = (n: number) => String(n).padStart(2, "0");
    return { label, key: `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` };
  });
}

function SchedulePlanner({
  view,
  onNotice,
}: {
  view: "today" | "week";
  onNotice: (notice: Notice | null) => void;
}) {
  const [listing, setListing] = useState<Awaited<ReturnType<typeof shell.scheduleList>> | null>(null);
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [availability, setAvailability] = useState<AvailabilityConfig | null>(null);
  const [report, setReport] = useState<ScheduleGenerateReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    Promise.all([
      shell.scheduleList(),
      shell.taskList(),
      shell.availabilityGet(),
    ])
      .then(([schedules, taskListing, config]) => {
        if (!alive) return;
        setListing(schedules);
        setTasks(taskListing.items);
        setAvailability(config);
        setError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setError(errText(e));
      })
      .finally(() => {});
    return () => {
      alive = false;
    };
  }, [tick]);

  async function regenerate() {
    setBusy(true);
    onNotice(null);
    try {
      const result = await shell.scheduleGenerate(LOCAL_OFFSET_MINUTES());
      setReport(result);
      onNotice({
        kind: "ok",
        text: `已生成 ${result.created.length} 条建议；过期 ${result.expired.length} 条。`,
      });
      setTick((n) => n + 1);
    } catch (e) {
      onNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function confirmEntry(id: string) {
    setBusy(true);
    try {
      await shell.scheduleConfirm(id);
      onNotice({ kind: "ok", text: "排期已确认。" });
      setTick((n) => n + 1);
    } catch (e) {
      onNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  const taskMap = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);
  const entries = listing?.items ?? [];
  const today = todayKey();
  const days = useMemo(() => weekDays(), [view]);
  const todayEntries = entries.filter((entry) => localDayKey(entry.start_at) === today);
  const todayWindowLabels = useMemo(() => {
    if (!availability) return [];
    const weekday = new Date().getDay() === 0 ? 7 : new Date().getDay();
    if (!availability.workdays.includes(weekday)) return [];
    return availability.daily_windows.map((window) => `${window.start}–${window.end}`);
  }, [availability]);
  const usedMinutes = todayEntries
    .filter((entry) => entry.effective_status !== "已失效" && entry.effective_status !== "已取消")
    .reduce((sum, entry) => sum + entry.duration, 0);
  const capacity = availability?.daily_capacity_hours ?? 0;
  const remaining = Math.max(0, capacity - usedMinutes / 60);
  const activeTaskIds = new Set(
    entries
      .filter((entry) => entry.status === "已建议" || entry.status === "已确认")
      .map((entry) => entry.task_id),
  );
  const weekUnscheduled = report
    ? report.unscheduled
    : tasks
        .filter((task) => task.importance === "高" && !activeTaskIds.has(task.id))
        .map((task) => ({ task_id: task.id, title: task.title, reason: "尚未排期" }));
  const overdue = tasks.filter((task) => {
    if (!task.due_date) return false;
    if (task.state === "已完成" || task.state === "已取消") return false;
    return task.due_date < today;
  });
  const dueThisWeek = tasks.filter((task) => {
    if (!task.due_date) return false;
    return days.some((day) => day.key === task.due_date);
  });
  const dueThisWeekActive = dueThisWeek.filter(
    (task) => task.state !== "已完成" && task.state !== "已取消",
  );

  return (
    <section aria-label={view === "today" ? "今日计划" : "本周计划"} data-schedule-view={view}>
      {error ? (
        <p className="mb-3 rounded-2xl bg-error-container/70 px-4 py-2 text-[11.5px] text-on-error-container">
          读取排期失败：{error}
        </p>
      ) : null}

      <div className={CARD + " px-4 py-4"}>
        <div className="flex flex-wrap items-baseline gap-3 pb-3 border-b border-surface-variant/25">
          <h2 className="text-[16px] font-semibold tracking-tight text-on-surface">
            {view === "today" ? `今日 · ${today}` : "本周计划"}
          </h2>
          {listing?.generated_at ? (
            <span className="mono text-[10px] text-outline">生成 {shortTime(listing.generated_at)}</span>
          ) : null}
          {listing?.stale && listing.generated_at ? (
            <span className="rounded-full bg-error-container/60 px-2 py-0.5 text-[10px] text-on-error-container">
              任务已变化，建议重新生成
            </span>
          ) : null}
          <button
            className="ml-auto rounded-full bg-primary px-3.5 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
            disabled={busy}
            onClick={() => void regenerate()}
          >
            {busy ? "生成中…" : "重新生成建议"}
          </button>
        </div>

        {view === "today" ? (
          <div className="pt-3 flex flex-col gap-4">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-outline">
              <span>可用时段：{todayWindowLabels.length > 0 ? todayWindowLabels.join("、") : "今天不工作"}</span>
              <span>剩余容量：{remaining.toFixed(1)} / {capacity.toFixed(1)} 小时</span>
            </div>
            {listing && !listing.generated_at ? (
              <p className="text-[11px] text-on-surface-variant">
                还没有生成过排期建议；点「重新生成建议」会按工作时间与任务优先级补一轮建议。
              </p>
            ) : null}
            {listing?.stale && listing.generated_at ? (
              <p className="text-[11px] text-on-surface-variant">
                任务列表在生成后发生了更新；重新生成只会补新建议，已确认与锁定安排保持不动。
              </p>
            ) : null}
            {report && report.conflicts.length > 0 ? (
              <div className="rounded-xl bg-error-container/40 px-3 py-2" data-schedule-conflicts="">
                <h3 className="text-[11.5px] font-semibold text-on-error-container">冲突</h3>
                <ul className="mt-1 flex flex-col gap-1 text-[10.5px] leading-relaxed text-on-error-container">
                  {report.conflicts.map((conflict, index) => (
                    <li key={index}>· {conflict.kind}：{conflict.message}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
              {(["已确认", "已建议"] as const).map((group) => {
                const items = todayEntries.filter((entry) =>
                  group === "已确认"
                    ? entry.status === "已确认" || entry.is_locked
                    : entry.status === "已建议",
                );
                return (
                  <div key={group} className="rounded-xl bg-surface-container/40 px-3 py-2.5">
                    <h3 className="text-[11.5px] font-semibold text-on-surface">{group} · {items.length}</h3>
                    {items.length === 0 ? (
                      <p className="mt-1 text-[10.5px] text-outline">今天没有{group}安排。</p>
                    ) : (
                      <div className="mt-1.5 flex flex-col gap-1.5">
                        {items.map((entry) => {
                          const task = taskMap.get(entry.task_id);
                          return (
                            <div key={entry.id} className="flex items-center gap-2 text-[11px]" data-schedule-entry={entry.id}>
                              <span className="mono text-[10px] text-outline">{localClock(entry.start_at)}</span>
                              <span className="min-w-0 flex-1 truncate text-on-surface">{task?.title ?? entry.task_id}</span>
                              <span className="mono text-[10px] text-outline">{entry.duration}m</span>
                              {entry.status === "已建议" ? (
                                <button
                                  className="rounded-full border border-outline-variant/50 px-2 py-0.5 text-[10px] hover:bg-white"
                                  disabled={busy}
                                  onClick={() => void confirmEntry(entry.id)}
                                >
                                  确认
                                </button>
                              ) : null}
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        ) : (
          <div className="pt-3 flex flex-col gap-4">
            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-7 gap-2" data-schedule-week="">
              {days.map((day) => {
                const dayEntries = entries.filter((entry) => localDayKey(entry.start_at) === day.key);
                return (
                  <div key={day.key} className="rounded-xl border border-outline-variant/30 bg-white/60 px-2 py-2 min-h-[96px]">
                    <div className="flex items-baseline gap-1.5 pb-1.5 border-b border-surface-variant/20">
                      <span className="text-[11px] font-semibold text-on-surface">{day.label}</span>
                      <span className="mono text-[9.5px] text-outline ml-auto">{day.key.slice(5)}</span>
                    </div>
                    <div className="pt-1.5 flex flex-col gap-1">
                      {dayEntries.length === 0 ? (
                        <span className="text-[10px] text-outline">暂无</span>
                      ) : (
                        dayEntries.slice(0, 5).map((entry) => (
                          <span
                            key={entry.id}
                            className="truncate rounded-md bg-primary-fixed/50 px-1.5 py-1 text-[10px] text-on-primary-fixed-variant"
                            data-hover-hint={taskMap.get(entry.task_id)?.title ?? entry.task_id}
                          >
                            {localClock(entry.start_at)} {taskMap.get(entry.task_id)?.title ?? entry.task_id}
                          </span>
                        ))
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-3">
              <div className="rounded-xl bg-surface-container/40 px-3 py-2.5">
                <h3 className="text-[11.5px] font-semibold text-on-surface">本周截止</h3>
                {dueThisWeekActive.length === 0 ? (
                  <p className="mt-1 text-[10.5px] text-outline">本周没有截止任务。</p>
                ) : (
                  <ul className="mt-1.5 flex flex-col gap-1 text-[10.5px]">
                    {dueThisWeekActive.map((task) => (
                      <li key={task.id} className="flex items-baseline gap-1.5">
                        <span className="mono text-[10px] text-outline">{task.due_date}</span>
                        <span className="min-w-0 flex-1 truncate">{task.title}</span>
                        <span className="text-outline">{task.importance}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="rounded-xl bg-surface-container/40 px-3 py-2.5">
                <h3 className="text-[11.5px] font-semibold text-on-surface">未排期的重要任务</h3>
                {weekUnscheduled.length === 0 ? (
                  <p className="mt-1 text-[10.5px] text-outline">生成后显示未排期原因。</p>
                ) : (
                  <ul className="mt-1.5 flex flex-col gap-1 text-[10.5px]">
                    {weekUnscheduled.map((item, index) => (
                      <li key={index} className="truncate">· {item.title}：{item.reason}</li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="rounded-xl bg-surface-container/40 px-3 py-2.5">
                <h3 className="text-[11.5px] font-semibold text-on-surface">逾期风险</h3>
                {overdue.length === 0 ? (
                  <p className="mt-1 text-[10.5px] text-outline">没有已逾期的未完成任务。</p>
                ) : (
                  <ul className="mt-1.5 flex flex-col gap-1 text-[10.5px]">
                    {overdue.map((task) => (
                      <li key={task.id} className="flex items-baseline gap-1.5">
                        <span className="mono text-[10px] text-error">{task.due_date}</span>
                        <span className="min-w-0 flex-1 truncate">{task.title}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
