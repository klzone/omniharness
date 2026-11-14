import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import { Icon, type IconName } from "../components/Icon";
import { KnowledgeLinkPanel } from "./KnowledgeLinkPanel";
import type {
  AvailabilityConfig,
  ScheduleEntryView,
  ScheduleGenerateReport,
  TaskDueKind,
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

const NEU_TILE =
  "bg-[#F9FAFD] shadow-[3px_3px_8px_rgba(160,175,200,0.22),-3px_-3px_8px_rgba(255,255,255,0.95)]";
const NEU_CARD =
  "bg-[#EFF3FA] shadow-[6px_6px_16px_rgba(156,173,199,0.30),-6px_-6px_16px_rgba(255,255,255,1)]";
const NEU_CARD_STRONG =
  "bg-[#F9FAFD] shadow-[9px_9px_24px_rgba(160,175,200,0.26),-9px_-9px_24px_rgba(255,255,255,1)]";
const NEU_INSET =
  "bg-[#E3E8F2] shadow-[inset_3px_3px_7px_rgba(160,175,200,0.28),inset_-3px_-3px_7px_rgba(255,255,255,0.95)]";
const GRAD_BLUE = "bg-[linear-gradient(90deg,#22D3EE_0%,#3B82F6_100%)]";
const AXIS = "text-[14px] font-semibold tracking-[0.08em] text-[#2B3242]";

const QUADRANT_TONE: Record<string, { icon: IconName; color: string }> = {
  Q1: { icon: "star", color: "text-[#3B82F6]" },
  Q2: { icon: "calendar", color: "text-[#0EA5E9]" },
  Q3: { icon: "clock", color: "text-[#F59E0B]" },
  Q4: { icon: "book", color: "text-[#8B5CF6]" },
};

const QUADRANT_SHORT: Record<string, string> = {
  Q1: "重要且紧急",
  Q2: "重要不紧急",
  Q3: "紧急不重要",
  Q4: "不重要不紧急",
  待评估: "待评估",
};

const RANGE_OPTIONS = [
  { key: "1m", label: "1 个月", months: 1 },
  { key: "3m", label: "3 个月", months: 3 },
  { key: "6m", label: "半年", months: 6 },
  { key: "1y", label: "1 年", months: 12 },
] as const;
type RangeKey = (typeof RANGE_OPTIONS)[number]["key"];

/** 按标题关键词挑图标：只做视觉区分，不参与任务语义判断。 */
function iconForTask(title: string, fallback: IconName): IconName {
  if (/报告|方案|提纲|模板|纪要|文稿|材料表/.test(title)) return "note";
  if (/台账|数据|核对|统计|分解表/.test(title)) return "checklist";
  if (/记录|归档|整理|档案/.test(title)) return "folder";
  if (/制度|知识|规范|要点/.test(title)) return "book";
  if (/会议|复盘|列席|排期/.test(title)) return "calendar";
  if (/报送|催办|汇报|上报/.test(title)) return "arrow-up";
  return fallback;
}

function statePercent(state: WorkTaskState): number {
  if (state === "已完成") return 100;
  if (state === "进行中") return 55;
  if (state === "暂停") return 35;
  if (state === "已取消") return 0;
  return 15;
}

function dayStart(value: string | Date): Date | null {
  const date = value instanceof Date ? new Date(value) : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  date.setHours(0, 0, 0, 0);
  return date;
}

function mmdd(date: Date): string {
  return date.getMonth() + 1 + "/" + date.getDate();
}

function isoDay(value: string | null): string {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate());
}

export function TaskQuadrantBoard({
  onNotice,
  variant = "page",
}: {
  onNotice?: (notice: Notice | null) => void;
  variant?: "page" | "panel";
}) {
  const [views, setViews] = useState<TaskQuadrantView[]>([]);
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const [editing, setEditing] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [range, setRange] = useState<RangeKey>("1m");
  const [importanceDraft, setImportanceDraft] = useState<TaskImportance>("待评估");
  const [quadrantDraft, setQuadrantDraft] = useState<TaskQuadrant | "自动">("自动");
  const [dueDraft, setDueDraft] = useState("");
  const [dueKindDraft, setDueKindDraft] = useState<TaskDueKind>("target");
  const [reasonDraft, setReasonDraft] = useState("");
  const [editorError, setEditorError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [localNotice, setLocalNotice] = useState<Notice | null>(null);

  function announce(notice: Notice | null) {
    setLocalNotice(notice);
    onNotice?.(notice);
  }

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([shell.taskQuadrantViews(LOCAL_OFFSET_MINUTES()), shell.taskList()])
      .then(([quadrants, listing]) => {
        if (!alive) return;
        setViews(quadrants);
        setTasks(listing.items);
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

  useEffect(() => {
    if (!editing) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setEditing(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [editing]);

  const taskMap = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);

  const displayViews = useMemo(
    () =>
      views.map((view) =>
        view.quadrant === "待评估" ? { ...view, quadrant: "Q2" as const } : view,
      ),
    [views],
  );

  const visibleViews = useMemo(() => {
    const needle = query.trim();
    if (!needle) return displayViews;
    return displayViews.filter((view) => {
      const title = taskMap.get(view.task_id)?.title ?? view.title;
      return title.includes(needle);
    });
  }, [displayViews, query, taskMap]);

  const completed = useMemo(
    () =>
      tasks
        .filter((task) => task.state === "已完成")
        .slice()
        .sort((a, b) => (b.updated_at > a.updated_at ? 1 : -1)),
    [tasks],
  );

  const today = useMemo(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d;
  }, [tick]);

  const stats = useMemo(() => {
    const todayKey = isoDay(today.toISOString());
    const created = tasks.filter((task) => isoDay(task.created_at) === todayKey).length;
    const running = tasks.filter((task) => task.state === "进行中").length;
    const done = tasks.filter((task) => task.state === "已完成").length;
    const overdue = tasks.filter(
      (task) =>
        task.due_date &&
        task.state !== "已完成" &&
        task.state !== "已取消" &&
        (dayStart(task.due_date)?.getTime() ?? Infinity) < today.getTime(),
    ).length;
    return { created, running, done, overdue };
  }, [tasks, today]);

  const timeline = useMemo(() => {
    const totalMonths = RANGE_OPTIONS.find((option) => option.key === range)?.months ?? 1;
    const start = new Date(today.getFullYear(), today.getMonth(), 1);
    const end = new Date(start.getFullYear(), start.getMonth() + totalMonths, 1);
    return { start, end, span: Math.max(1, end.getTime() - start.getTime()), totalMonths };
  }, [range, today]);

  const ticks = useMemo(
    () =>
      Array.from({ length: 7 }, (_, index) =>
        new Date(timeline.start.getTime() + (timeline.span * index) / 6),
      ),
    [timeline],
  );

  const ganttRows = useMemo(() => {
    const rows = tasks
      .filter((task) => task.due_date)
      .map((task) => {
        const start = dayStart(task.created_at) ?? timeline.start;
        const end = dayStart(task.due_date as string);
        if (!end) return null;
        const from = Math.max(start.getTime(), timeline.start.getTime());
        const to = Math.min(end.getTime(), timeline.end.getTime());
        if (to <= timeline.start.getTime() || from >= timeline.end.getTime()) return null;
        const left = ((from - timeline.start.getTime()) / timeline.span) * 100;
        const width = Math.max(2, ((to - from) / timeline.span) * 100);
        return { task, left, width, clamped: end.getTime() >= timeline.end.getTime() };
      })
      .filter((row): row is { task: WorkTask; left: number; width: number; clamped: boolean } => row !== null)
      .sort((a, b) => a.left - b.left);
    return rows.slice(0, 8);
  }, [tasks, timeline]);

  const todayPercent = ((today.getTime() - timeline.start.getTime()) / timeline.span) * 100;

  function openEditor(view: TaskQuadrantView) {
    const task = taskMap.get(view.task_id);
    setEditing(view.task_id);
    setImportanceDraft(task?.importance ?? "待评估");
    setQuadrantDraft(task?.quadrant_override ?? "自动");
    setDueDraft(isoDay(task?.due_date ?? null));
    setDueKindDraft(task?.due_kind ?? "target");
    setReasonDraft("");
    setEditorError(null);
    announce(null);
  }

  async function saveEditor() {
    if (!editing) return;
    const task = taskMap.get(editing);
    if (!task) return;
    const reason = reasonDraft.trim();
    const importanceChanged = importanceDraft !== task.importance;
    const quadrantChanged = (task.quadrant_override ?? "自动") !== quadrantDraft;
    const currentDue = isoDay(task.due_date);
    const dueChanged = currentDue !== dueDraft || (Boolean(dueDraft) && dueKindDraft !== task.due_kind);
    if (!importanceChanged && !quadrantChanged && !dueChanged) {
      setEditing(null);
      return;
    }
    if ((importanceChanged || dueChanged) && !reason) {
      setEditorError("调整重要性或截止日期时需要填写理由。");
      return;
    }
    setBusy(true);
    setEditorError(null);
    try {
      if (importanceChanged) await shell.taskSetImportance(editing, importanceDraft, reason);
      if (quadrantChanged) {
        await shell.taskSetQuadrant(editing, quadrantDraft === "自动" ? null : quadrantDraft, reason || undefined);
      }
      if (dueChanged) {
        await shell.taskSetDue(editing, dueDraft || null, dueKindDraft, reason);
      }
      announce({ kind: "ok", text: "任务已更新。" });
      setEditing(null);
      setTick((n) => n + 1);
    } catch (e) {
      setEditorError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  const editingTask = editing ? taskMap.get(editing) ?? null : null;
  const compact = variant === "panel";

  const arrowUp = (
    <span className="h-0 w-0 border-x-[4px] border-b-[5px] border-x-transparent border-b-[#AEB9CC]" />
  );
  const arrowDown = (
    <span className="h-0 w-0 border-x-[4px] border-t-[5px] border-x-transparent border-t-[#AEB9CC]" />
  );
  const arrowLeft = (
    <span className="h-0 w-0 border-y-[4px] border-r-[5px] border-y-transparent border-r-[#AEB9CC]" />
  );
  const arrowRight = (
    <span className="h-0 w-0 border-y-[4px] border-l-[5px] border-y-transparent border-l-[#AEB9CC]" />
  );

  const matrix = (
    <div
      className="grid w-full"
      style={{
        gridTemplateColumns: "26px minmax(0,1fr) 20px minmax(0,1fr)",
        gridTemplateRows: compact
          ? "20px 126px 18px 126px"
          : "26px 178px 20px 178px",
      }}
    >
      <span className={AXIS + " flex items-end justify-center pb-1.5"} style={{ gridColumn: 2, gridRow: 1 }}>
        紧急
      </span>
      <span className={AXIS + " flex items-end justify-center pb-1.5"} style={{ gridColumn: 4, gridRow: 1 }}>
        不紧急
      </span>
      <span
        className={AXIS + " flex items-center justify-center [writing-mode:vertical-rl] [text-orientation:upright]"}
        style={{ gridColumn: 1, gridRow: 2, letterSpacing: "0.3em" }}
      >
        重要
      </span>
      <span
        className={AXIS + " flex items-center justify-center [writing-mode:vertical-rl] [text-orientation:upright]"}
        style={{ gridColumn: 1, gridRow: 4, letterSpacing: "0.3em" }}
      >
        不重要
      </span>

      <div
        aria-hidden="true"
        className="flex flex-col items-center justify-center gap-0.5"
        style={{ gridColumn: 3, gridRow: "2 / 5" }}
      >
        {arrowUp}
        <span className="w-px flex-1 bg-[#C9D3E4]" />
        {arrowDown}
      </div>

      <div
        aria-hidden="true"
        className="flex items-center justify-center gap-0.5"
        style={{ gridColumn: "2 / 5", gridRow: 3 }}
      >
        {arrowLeft}
        <span className="h-px flex-1 bg-[#C9D3E4]" />
        {arrowRight}
      </div>

      {QUADRANT_COLUMNS.slice(0, 4).map((column) => {
        const items = visibleViews.filter((view) => view.quadrant === column.key);
        const tone = QUADRANT_TONE[column.key] ?? QUADRANT_TONE.Q1;
        const selected = column.key === "Q1";
        const leftColumn = column.key === "Q1" || column.key === "Q3";
        const topRow = column.key === "Q1" || column.key === "Q2";
        return (
          <section
            key={column.key}
            aria-label={column.label}
            data-quadrant-column={column.key}
            style={{ gridColumn: leftColumn ? 2 : 4, gridRow: topRow ? 2 : 4 }}
            className={
              "relative flex min-w-0 flex-col overflow-hidden " +
              (compact ? "rounded-[18px] px-3.5 py-3 " : "rounded-[22px] px-5 py-4 ") +
              (selected ? NEU_CARD_STRONG : NEU_CARD)
            }
          >
            {selected ? (
              <span
                aria-hidden="true"
                className="pointer-events-none absolute bottom-6 left-0 top-6 w-[3px] rounded-r-[3px] bg-[linear-gradient(180deg,#22D3EE,#3B82F6)]"
              />
            ) : null}
            <div className={(compact ? "flex flex-col gap-2.5" : "flex flex-col gap-3.5") + " min-h-0 overflow-y-auto pr-0.5"}>
              {items.length === 0 ? (
                <p className="m-auto text-[11px] text-[#A6AEBF]">暂无任务</p>
              ) : (
                items.map((view) => {
                  const task = taskMap.get(view.task_id);
                  const percent = statePercent(task?.state ?? "待开始");
                  return (
                    <article key={view.task_id} data-task-quadrant-row={view.task_id} className="min-w-0">
                      <button
                        type="button"
                        onClick={() => openEditor(view)}
                        title={view.title}
                        className={
                          "grid w-full items-center rounded-2xl bg-[linear-gradient(180deg,#FFFFFF_0%,#F5F9FF_100%)] text-left transition " +
                          "shadow-[6px_6px_14px_rgba(146,164,193,0.34),-6px_-6px_14px_rgba(255,255,255,1),inset_0_1px_0_rgba(255,255,255,1),inset_0_-1px_0_rgba(186,200,222,0.45)] " +
                          "hover:-translate-y-px hover:shadow-[8px_8px_18px_rgba(146,164,193,0.42),-7px_-7px_16px_rgba(255,255,255,1)] " +
                          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#3B82F6]/40 " +
                          (compact
                            ? "grid-cols-[26px_minmax(0,1fr)] gap-2 px-2 py-1.5"
                            : "grid-cols-[34px_minmax(0,1fr)_auto] gap-3 px-3 py-2.5")
                        }
                      >
                        <span
                          className={
                            "flex shrink-0 items-center justify-center bg-[#E9EEF8] " +
                            "shadow-[inset_2px_2px_5px_rgba(148,166,194,0.48),inset_-2px_-2px_5px_rgba(255,255,255,1)] " +
                            (compact ? "h-[26px] w-[26px] rounded-[8px] " : "h-[34px] w-[34px] rounded-[11px] ") +
                            tone.color
                          }
                        >
                          <Icon name={iconForTask(view.title, tone.icon)} className={compact ? "h-3.5 w-3.5" : "h-4 w-4"} />
                        </span>
                        <span className="min-w-0">
                          <span
                            title={view.title}
                            className={
                              "block truncate font-semibold leading-snug text-[#2B3242] " +
                              (compact ? "text-[12px]" : "text-[13.5px]")
                            }
                          >
                            {view.title}
                          </span>
                          <span
                            title={task?.due_date ? "截止 " + mmdd(dayStart(task.due_date) ?? new Date()) : task?.state}
                            className={
                              "mt-0.5 block truncate text-[10.5px] text-[#98A1B3] " + (compact ? "hidden" : "")
                            }
                          >
                            {task?.due_date ? "截止 " + mmdd(dayStart(task.due_date) ?? new Date()) : task?.state ?? "待开始"}
                          </span>
                        </span>
                        <span
                          className={
                            "mono shrink-0 rounded-full bg-[#3B82F6]/10 px-2.5 py-1 text-[10.5px] font-medium text-[#2563EB] " +
                            (compact ? "hidden" : "")
                          }
                        >
                          {percent}%
                        </span>
                      </button>
                    </article>
                  );
                })
              )}
            </div>
          </section>
        );
      })}
    </div>
  );

  const gantt = (
    <section aria-label="任务甘特图" className={"min-w-0 rounded-[24px] px-6 py-5 " + NEU_CARD_STRONG}>
      <div className="flex flex-wrap items-center justify-between gap-3 pb-4">
        <span className="text-[11px] tracking-[0.14em] text-[#98A1B3]">时间范围</span>
        <div className="flex gap-1.5">
          {RANGE_OPTIONS.map((option) => (
            <button
              key={option.key}
              type="button"
              onClick={() => setRange(option.key)}
              aria-pressed={range === option.key}
              className={
                "rounded-full px-3 py-1.5 text-[11px] transition " +
                (range === option.key
                  ? "bg-[#DCE9FF] font-medium text-[#2563EB] shadow-[inset_2px_2px_5px_rgba(160,175,200,0.32)]"
                  : NEU_TILE + " text-[#5B6579]")
              }
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-[minmax(150px,168px)_minmax(0,1fr)] items-center gap-4 pb-3">
        <span />
        <div className="grid grid-cols-7">
          {ticks.map((day, index) => (
            <span key={index} className="mono text-[10px] text-[#98A1B3]">
              {mmdd(day)}
            </span>
          ))}
        </div>
      </div>

      {ganttRows.length === 0 ? (
        <p className="pt-2 text-[11.5px] text-[#A6AEBF]">当前范围内没有带截止日期的任务。</p>
      ) : (
        <div className="relative flex flex-col">
          <span
            aria-hidden="true"
            className="pointer-events-none absolute -top-2 bottom-0 w-px bg-[repeating-linear-gradient(180deg,rgba(59,130,246,0.5)_0_5px,transparent_5px_11px)]"
            style={{
              left: "calc(min(168px, 100% - 1px) + 16px + (100% - min(168px, 100% - 1px) - 16px) * " + Math.max(0, Math.min(1, todayPercent / 100)) + ")",
            }}
          />
          {ganttRows.map(({ task, left, width, clamped }) => {
            const percent = statePercent(task.state);
            return (
              <div
                key={task.id}
                data-task-gantt-row={task.id}
                className="grid grid-cols-[minmax(150px,168px)_minmax(0,1fr)] items-center gap-4 border-b border-dashed border-[#DCE3F0] py-2.5 last:border-b-0 last:pb-0"
              >
                <div className="flex min-w-0 items-baseline gap-2">
                  <span title={task.title} className="truncate text-[12px] font-medium text-[#2B3242]">
                    {task.title}
                  </span>
                  <span className="mono shrink-0 text-[10px] text-[#98A1B3]">{percent}%</span>
                </div>
                <div className="relative h-[9px] rounded-full bg-[#E3E8F2] shadow-[inset_0_1px_2px_rgba(140,160,190,0.4)]">
                  <div
                    className={
                      "absolute top-0 h-[9px] bg-[linear-gradient(90deg,rgba(34,211,238,0.18),rgba(59,130,246,0.18))] " +
                      (clamped ? "rounded-l-full" : "rounded-full")
                    }
                    style={{ left: left + "%", width: width + "%" }}
                  >
                    <div className={"h-[9px] rounded-full " + GRAD_BLUE} style={{ width: percent + "%" }} />
                  </div>
                  {clamped ? (
                    <span
                      aria-hidden="true"
                      className="absolute right-0 top-1/2 h-0 w-0 -translate-y-1/2 border-y-[4px] border-l-[5px] border-y-transparent border-l-[#3B82F6]/70"
                    />
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );

  const statCards = [
    {
      key: "created",
      label: "今日新增",
      value: stats.created,
      icon: "clock" as const,
      tone: "text-[#0891B2]",
      tint: ["#EAF8FD", "#CFEDF8"],
    },
    {
      key: "running",
      label: "进行中",
      value: stats.running,
      icon: "arrow-right" as const,
      tone: "text-[#2563EB]",
      tint: ["#EDF3FF", "#D6E4FB"],
    },
    {
      key: "done",
      label: "已完成",
      value: stats.done,
      icon: "check" as const,
      tone: "text-[#0F766E]",
      tint: ["#EAF8F4", "#CFEDE6"],
    },
    {
      key: "overdue",
      label: "逾期",
      value: stats.overdue,
      icon: "clock" as const,
      tone: "text-[#B45309]",
      tint: ["#FDF4E6", "#F7E3C6"],
    },
  ];

  const statsPanel = (
    <div className="grid grid-cols-2 gap-3">
      {statCards.map((item) => (
        <div key={item.key} className={"relative overflow-hidden rounded-[20px] px-4 py-4 " + NEU_CARD}>
          <div className="flex items-center gap-3">
            <span
              className={"relative flex h-11 w-11 shrink-0 items-center justify-center rounded-full " + item.tone}
              style={{
                background:
                  "radial-gradient(140% 140% at 32% 20%, #FFFFFF 0%, " + item.tint[0] + " 46%, " + item.tint[1] + " 100%)",
                boxShadow:
                  "5px 5px 12px rgba(160,175,200,0.38), -5px -5px 12px rgba(255,255,255,1), inset 0 1px 1px rgba(255,255,255,0.95), inset 0 -2px 4px rgba(160,175,200,0.32)",
              }}
            >
              <span
                aria-hidden="true"
                className="pointer-events-none absolute left-[20%] top-[13%] h-[26%] w-[46%] rounded-full bg-white/80 blur-[1.5px]"
              />
              <Icon name={item.icon} className="relative h-[17px] w-[17px] drop-shadow-[0_1px_1px_rgba(59,130,246,0.25)]" />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[9.5px] font-medium tracking-[0.16em] text-[#949EB1] [text-shadow:0_1px_0_rgba(255,255,255,0.95)]">
                {item.label}
              </span>
              <span className="mt-1 block text-[24px] font-semibold leading-none text-[#5D687E] [text-shadow:0_1px_0_rgba(255,255,255,0.95)]">
                {item.value}
              </span>
            </span>
          </div>
        </div>
      ))}
    </div>
  );

  const completedPanel = (
    <aside aria-label="已完成任务" className={"rounded-[24px] px-5 py-4 " + NEU_CARD_STRONG}>
      <h3 className="mono pb-3 text-[10px] tracking-[0.18em] text-[#98A1B3]">已完成</h3>
      {completed.length === 0 ? (
        <p className="text-[11px] text-[#A6AEBF]">暂无已完成任务。</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {completed.slice(0, 5).map((task) => (
            <li key={task.id} className="flex items-center gap-3 rounded-xl px-1 py-2">
              <span className="flex h-[26px] w-[26px] shrink-0 items-center justify-center rounded-full bg-[linear-gradient(135deg,#22D3EE,#3B82F6)] text-white shadow-[0_3px_8px_rgba(59,130,246,0.28)]">
                <Icon name="check" className="h-3 w-3" />
              </span>
              <span className="min-w-0 flex-1">
                <span title={task.title} className="block truncate text-[12.5px] font-medium text-[#2B3242]">
                  {task.title}
                </span>
                <span className="mono mt-0.5 block text-[9.5px] text-[#A6AEBF]">{isoDay(task.updated_at)}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );

  if (variant === "panel") {
    return (
      <section aria-label="任务四象限" data-task-quadrant="" className="min-w-0">
        {error ? (
          <p className="mb-2 rounded-2xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
            读取四象限失败：{error}
          </p>
        ) : null}
        {matrix}
      </section>
    );
  }

  return (
    <section
      aria-label="任务四象限"
      data-task-quadrant=""
      className="relative mx-auto max-w-[1180px] overflow-hidden rounded-[30px] bg-[radial-gradient(120%_110%_at_50%_-10%,#F7FAFF_0%,#EEF1F7_55%,#E7ECF6_100%)] px-5 py-5 md:px-7 md:py-6"
    >
      <div className="pointer-events-none absolute -right-24 -top-28 h-64 w-64 rounded-full bg-[#3B82F6]/10 blur-3xl" />
      <div className="pointer-events-none absolute -bottom-28 -left-20 h-64 w-64 rounded-full bg-[#22D3EE]/10 blur-3xl" />

      <header className="relative flex flex-wrap items-end justify-between gap-4 pb-5">
        <div>
          <h2 className="text-[22px] font-semibold tracking-tight text-[#232A3A]">项目跟踪</h2>
          <p className="mt-1.5 text-[11.5px] text-[#6B7488]">
            {today.getFullYear()} 年 {today.getMonth() + 1} 月 {today.getDate()} 日
          </p>
        </div>
        <label className={"flex h-10 w-[240px] items-center gap-2 rounded-full px-4 " + NEU_INSET}>
          <Icon name="search" className="h-4 w-4 text-[#3B82F6]/80" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索任务"
            className="w-full bg-transparent text-[12px] text-[#2B3242] placeholder:text-[#98A1B3] focus:outline-none"
            aria-label="搜索任务"
          />
        </label>
      </header>

      {localNotice ? (
        <p
          role="status"
          className={
            "relative mb-3 rounded-2xl px-4 py-2 text-[11.5px] leading-relaxed " +
            (localNotice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-white/80 text-[#5B6579]")
          }
        >
          {localNotice.text}
        </p>
      ) : null}
      {error ? (
        <p className="relative mb-3 rounded-2xl bg-error-container/70 px-4 py-2 text-[11.5px] leading-relaxed text-on-error-container">
          读取四象限失败：{error}
        </p>
      ) : null}
      {loading ? <p className="relative pb-2 text-[11.5px] text-[#98A1B3]">正在读取任务…</p> : null}

      <div className="relative grid grid-cols-1 items-start gap-5 xl:grid-cols-[minmax(0,1fr)_300px]">
        <div className="flex min-w-0 flex-col gap-5">
          {matrix}
          {gantt}
        </div>
        <div className="flex min-w-0 flex-col gap-5">
          {statsPanel}
          {completedPanel}
        </div>
      </div>

      {editingTask ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-[#1B2434]/30 px-4"
          role="presentation"
          onClick={() => setEditing(null)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="编辑任务"
            onClick={(event) => event.stopPropagation()}
            className="w-full max-w-[520px] rounded-[26px] bg-[#F7F9FD] p-6 shadow-[0_34px_80px_-30px_rgba(30,50,90,0.5)]"
          >
            <div className="flex items-start gap-3 pb-4">
              <span className="flex h-10 w-10 items-center justify-center rounded-[13px] bg-[linear-gradient(135deg,#22D3EE,#3B82F6)] text-white shadow-[0_8px_18px_-8px_rgba(59,130,246,0.8)]">
                <Icon name="edit" className="h-4 w-4" />
              </span>
              <div className="min-w-0 flex-1">
                <h3 title={editingTask.title} className="truncate text-[15px] font-semibold text-[#232A3A]">
                  {editingTask.title}
                </h3>
                <p className="mt-1 text-[10.5px] text-[#98A1B3]">
                  {editingTask.state} · 创建于 {isoDay(editingTask.created_at)}
                </p>
              </div>
              <button
                type="button"
                aria-label="关闭"
                onClick={() => setEditing(null)}
                className={"flex h-9 w-9 items-center justify-center rounded-full text-[#6B7488] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>

            <div className="flex flex-col gap-4">
              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[#98A1B3]">重要性</p>
                <div className="flex flex-wrap gap-2">
                  {IMPORTANCE_CHOICES.map((level) => (
                    <button
                      key={level}
                      type="button"
                      onClick={() => setImportanceDraft(level)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (importanceDraft === level
                          ? "bg-[#DCE9FF] font-medium text-[#2563EB] shadow-[inset_2px_2px_5px_rgba(160,175,200,0.32)]"
                          : NEU_TILE + " text-[#5B6579]")
                      }
                    >
                      {level}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[#98A1B3]">象限</p>
                <div className="flex flex-wrap gap-2">
                  {QUADRANT_CHOICES.map((choice) => (
                    <button
                      key={choice}
                      type="button"
                      onClick={() => setQuadrantDraft(choice)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (quadrantDraft === choice
                          ? "bg-[#DCE9FF] font-medium text-[#2563EB] shadow-[inset_2px_2px_5px_rgba(160,175,200,0.32)]"
                          : NEU_TILE + " text-[#5B6579]")
                      }
                    >
                      {choice === "自动" ? "自动" : QUADRANT_SHORT[choice] ?? choice}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[#98A1B3]">截止日期</p>
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    type="date"
                    value={dueDraft}
                    onChange={(event) => setDueDraft(event.target.value)}
                    aria-label="截止日期"
                    className={"h-9 rounded-full px-3 text-[11.5px] text-[#2B3242] " + NEU_INSET}
                  />
                  {(["target", "hard"] as TaskDueKind[]).map((kind) => (
                    <button
                      key={kind}
                      type="button"
                      onClick={() => setDueKindDraft(kind)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (dueKindDraft === kind
                          ? "bg-[#DCE9FF] font-medium text-[#2563EB] shadow-[inset_2px_2px_5px_rgba(160,175,200,0.32)]"
                          : NEU_TILE + " text-[#5B6579]")
                      }
                    >
                      {kind === "hard" ? "硬截止" : "目标日"}
                    </button>
                  ))}
                </div>
              </div>

              <label className="block">
                <span className="block pb-2 text-[10.5px] tracking-[0.14em] text-[#98A1B3]">调整理由</span>
                <input
                  value={reasonDraft}
                  onChange={(event) => setReasonDraft(event.target.value)}
                  placeholder="调整重要性或截止日期时必填"
                  className={
                    "h-10 w-full rounded-2xl px-3.5 text-[11.5px] text-[#2B3242] placeholder:text-[#A6AEBF] " +
                    NEU_INSET
                  }
                />
              </label>

              {editorError ? (
                <p className="rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
                  {editorError}
                </p>
              ) : null}
            </div>

            <div className="mt-5 flex justify-end gap-2.5">
              <button
                type="button"
                onClick={() => setEditing(null)}
                className={"rounded-full px-4 py-2 text-[11.5px] text-[#5B6579] " + NEU_TILE}
              >
                取消
              </button>
              <button
                type="button"
                onClick={() => void saveEditor()}
                disabled={busy}
                className="rounded-full bg-[linear-gradient(135deg,#3B82F6,#2563EB)] px-5 py-2 text-[11.5px] font-medium text-white shadow-[0_10px_20px_-10px_rgba(37,99,235,0.9)] disabled:opacity-50"
              >
                {busy ? "保存中…" : "保存修改"}
              </button>
            </div>
          </div>
        </div>
      ) : null}
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
