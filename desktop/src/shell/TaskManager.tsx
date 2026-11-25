import { useEffect, useMemo, useRef, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import { KnowledgeLinkPanel } from "./KnowledgeLinkPanel";
import { ConfirmDialog } from "./ConfirmDialog";
import { ProjectMaintenance } from "./ProjectMaintenance";
import { ProjectReviewCard, type ProjectReviewSeed } from "./ProjectReviewCard";
import {
  autoCollapseGroups,
  buildProjectGanttGroups,
  summaryRangeLabel,
} from "../projectGanttGroups";
import {
  projectWindow,
  taskProgressRatio,
} from "../projectGanttProgress";
import {
  listThreadsByTag,
  onThreadStoreChange,
  requestOpenThread,
  type SceneThread,
} from "../sceneThreads";
import type { Scene } from "./Dock";
import type {
  AvailabilityConfig,
  ProjectInfo,
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
      className="pj-state-dot shrink-0"
      data-task-state={state}
      title={state}
      aria-label={state}
    />
  );
}

function taskStateIcon(
  state: WorkTaskState,
): "check" | "clock" | "checklist" | "stop" | "x" {
  if (state === "已完成") return "check";
  if (state === "进行中") return "clock";
  if (state === "暂停") return "stop";
  if (state === "已取消") return "x";
  return "checklist";
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
        <div className="mx-auto">
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

/*
 * OD-UI-716：项目跟踪的视觉层。
 * 布局仍是「四象限 + 甘特 + 统计 / 已完成」，但材质改回工作记录页的暖白新拟物：
 * 象牙灰底、近白凸起、品牌靛蓝只用在小面积强调；不再使用冷蓝渐变与光斑。
 * 甘特条表达状态，不伪造时间进度百分比。
 */
const NEU_TILE =
  "bg-[color:var(--od-neu-sunken)] shadow-[var(--od-neu-inset-1)]";
const NEU_INSET =
  "bg-[color:var(--od-neu-sunken)] shadow-[var(--od-neu-inset-1)]";

/** 甘特状态色语义表（OD-UI-900 后条内以实底色区分，此表供语义引用与测试锚定）。 */
export const TASK_STATE_BAR: Record<WorkTaskState, string> = {
  待确认: "od-task-bar--pending-confirm",
  待开始: "od-task-bar--not-started",
  进行中: "od-task-bar--in-progress",
  已完成: "od-task-bar--done",
  已取消: "od-task-bar--cancelled",
  暂停: "od-task-bar--paused",
};

const TASK_PLAN_BAR: Record<WorkTaskState, string> = {
  待确认: "od-gantt-plan--pending-confirm",
  待开始: "od-gantt-plan--not-started",
  进行中: "od-gantt-plan--in-progress",
  已完成: "od-gantt-plan--done",
  已取消: "od-gantt-plan--cancelled",
  暂停: "od-gantt-plan--paused",
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

function dayStart(value: string | Date): Date | null {
  const date = value instanceof Date ? new Date(value) : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  date.setHours(0, 0, 0, 0);
  return date;
}

function mmdd(date: Date): string {
  return date.getMonth() + 1 + "/" + date.getDate();
}

function threadDate(updatedAt: number): string {
  const date = new Date(updatedAt);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (value: number) => String(value).padStart(2, "0");
  return pad(date.getMonth() + 1) + "/" + pad(date.getDate());
}

/** 毫秒时间戳转本地 YYYY-MM-DD，用于项目汇总带区间文字。 */
function ganttDay(ms: number): string {
  const date = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate());
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
  onNavigate,
  variant = "page",
}: {
  onNotice?: (notice: Notice | null) => void;
  onNavigate?: (scene: Scene) => void;
  variant?: "page" | "panel";
}) {
  const [views, setViews] = useState<TaskQuadrantView[]>([]);
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
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
  const [completeConfirm, setCompleteConfirm] = useState(false);
  const [reviewSeed, setReviewSeed] = useState<ProjectReviewSeed>(null);
  /** 甘特组折叠：用户手动覆盖默认折叠（默认值在超过 30 行时自动给出）。 */
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  /** 左栏四象限：默认展开紧急重要与重要不紧急，其余折叠。 */
  const [expandedQuadrants, setExpandedQuadrants] = useState<Record<string, boolean>>({
    Q1: true,
    Q2: true,
    Q3: false,
    Q4: false,
  });
  const [statusModal, setStatusModal] = useState<"done" | "running" | "overdue" | null>(null);
  const [deleteTaskId, setDeleteTaskId] = useState<string | null>(null);
  const [projectThreads, setProjectThreads] = useState<SceneThread[]>([]);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [quickTaskOpen, setQuickTaskOpen] = useState(false);
  const [quickTaskTitle, setQuickTaskTitle] = useState("");
  const [quickTaskDue, setQuickTaskDue] = useState("");
  const [quickTaskProject, setQuickTaskProject] = useState("");
  const [quickTaskBusy, setQuickTaskBusy] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);
  const [manageProjectId, setManageProjectId] = useState("");
  const [newProjectNonce, setNewProjectNonce] = useState(0);
  const [projectCardLimit, setProjectCardLimit] = useState(3);
  const projectRowRef = useRef<HTMLDivElement | null>(null);

  function announce(notice: Notice | null) {
    setLocalNotice(notice);
    onNotice?.(notice);
  }

  useEffect(() => {
    if (!manageOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setManageOpen(false);
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [manageOpen]);

  useEffect(() => {
    if (!statusModal) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setStatusModal(null);
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [statusModal]);

  useEffect(() => {
    const load = () => {
      setProjectThreads(
        listThreadsByTag("project")
          .slice()
          .sort((a, b) => (b.updatedAt > a.updatedAt ? 1 : -1)),
      );
    };
    load();
    const unsubscribe = onThreadStoreChange(load);
    return () => {
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    const element = projectRowRef.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      setProjectCardLimit(width >= 980 ? 4 : 3);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [projects.length, tasks.length]);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([
      shell.taskQuadrantViews(LOCAL_OFFSET_MINUTES()),
      shell.taskList(),
      shell.projectList(),
    ])
      .then(([quadrants, listing, projectListing]) => {
        if (!alive) return;
        setViews(quadrants);
        setTasks(listing.items);
        setProjects(projectListing.items);
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

  /** OD-UI-900：快捷新建弹层的 Escape 关闭。 */
  useEffect(() => {
    if (!quickTaskOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setQuickTaskOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [quickTaskOpen]);

  const nowMs = useMemo(() => Date.now(), [tick, range]);
  const taskMap = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);
  const projectMap = useMemo(
    () => new Map(projects.map((project) => [project.project_id, project])),
    [projects],
  );

  const displayViews = useMemo(() =>
    views.map((view) =>
      view.quadrant === "待评估" ? { ...view, quadrant: "Q2" as const } : view,
    ), [views]);

  const today = useMemo(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d;
  }, [tick]);

  const visibleViews = useMemo(() => {
    const needle = query.trim();
    const archived = new Set<WorkTaskState>(["已完成", "已取消"]);
    let base = displayViews.filter((view) => {
      const task = taskMap.get(view.task_id);
      return !task || !archived.has(task.state);
    });
    if (needle) {
      base = base.filter((view) => {
        const title = taskMap.get(view.task_id)?.title ?? view.title;
        return title.includes(needle);
      });
    }
    return base;
  }, [displayViews, query, taskMap]);

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

  /** 甘特分组行：坐标换算与排序都在 projectGanttGroups 纯函数里，组件只负责渲染。 */
  const ganttGroups = useMemo(
    () =>
      buildProjectGanttGroups(tasks, projects, {
        start: timeline.start.getTime(),
        end: timeline.end.getTime(),
        span: timeline.span,
      }),
    [tasks, projects, timeline],
  );

  /** 项目汇总带：计划区间浅带 + 时间进度填充，区间缺省时补 90 天。 */
  const ganttSummaries = useMemo(
    () =>
      new Map(
        ganttGroups.map((group) => {
          const project = projectMap.get(group.projectId);
          const fallbackStartMs = group.rows.length
            ? Math.min(...group.rows.map((row) => row.startMs))
            : timeline.start.getTime();
          const window = projectWindow(project, fallbackStartMs);
          const from = Math.max(window.startMs, timeline.start.getTime());
          const to = Math.min(window.endMs, timeline.end.getTime());
          const visibleFrom = to > from ? from : Math.min(...group.rows.map((row) => row.startMs));
          const visibleTo = to > from ? to : Math.max(...group.rows.map((row) => row.endMs));
          return [
            group.projectId,
            {
              leftPct: ((visibleFrom - timeline.start.getTime()) / timeline.span) * 100,
              widthPct: Math.max(2, ((visibleTo - visibleFrom) / timeline.span) * 100),
              start: ganttDay(visibleFrom),
              end: ganttDay(visibleTo),
              // OD-UI-900 评审修订：进度槽改用「已完成任务占比」，不再用时间流逝比。
              ratio:
                group.rows.length === 0
                  ? 0
                  : group.rows.filter((row) => row.task.state === "已完成").length /
                    group.rows.length,
            },
          ] as const;
        }),
      ),
    [ganttGroups, projectMap, timeline, nowMs],
  );

  /** 总行数超过 30 时默认折叠未挂接组与最早结束的组。 */
  const autoCollapsedIds = useMemo(() => autoCollapseGroups(ganttGroups), [ganttGroups]);
  const collapsedCount = ganttGroups.filter(
    (group) => collapsedGroups[group.projectId] ?? autoCollapsedIds.includes(group.projectId),
  ).length;

  function isGroupCollapsed(projectId: string): boolean {
    return collapsedGroups[projectId] ?? autoCollapsedIds.includes(projectId);
  }

  function toggleGroup(projectId: string) {
    const current = isGroupCollapsed(projectId);
    setCollapsedGroups((previous) => ({ ...previous, [projectId]: !current }));
  }

  const projectCards = useMemo(() => {
    return projects
      .slice()
      .sort((a, b) => (b.updated_at > a.updated_at ? 1 : -1))
      .map((project) => {
        const rows = tasks.filter((task) => task.project_id === project.project_id);
        const done = rows.filter((task) => task.state === "已完成").length;
        const fallbackStart =
          (project.start ? dayStart(project.start)?.getTime() : null) ?? nowMs;
        const window = projectWindow(project, fallbackStart);
        const span = Math.max(1, window.endMs - window.startMs);
        const ratio = Math.max(0, Math.min(1, (nowMs - window.startMs) / span));
        return { project, total: rows.length, done, ratio };
      });
  }, [projects, tasks, nowMs]);

  const visibleProjectCards = projectCards.slice(
    0,
    Math.max(1, Math.min(projectCardLimit, projectCards.length)),
  );

  const statusTasks = useMemo(() => {
    if (!statusModal) return [];
    if (statusModal === "done") {
      return tasks
        .filter((task) => task.state === "已完成")
        .slice()
        .sort((a, b) => (b.updated_at > a.updated_at ? 1 : -1));
    }
    if (statusModal === "running") {
      return tasks
        .filter((task) => task.state === "进行中")
        .slice()
        .sort((a, b) => (a.due_date ?? "9999").localeCompare(b.due_date ?? "9999"));
    }
    return tasks
      .filter(
        (task) =>
          task.due_date &&
          task.state !== "已完成" &&
          task.state !== "已取消" &&
          (dayStart(task.due_date)?.getTime() ?? Infinity) < nowMs,
      )
      .slice()
      .sort((a, b) => (a.due_date ?? "").localeCompare(b.due_date ?? ""));
  }, [statusModal, tasks, nowMs]);

  const todayPercent = ((today.getTime() - timeline.start.getTime()) / timeline.span) * 100;

  /** 四象限与甘特行共用同一个编辑弹窗入口。 */
  function openEditor(taskId: string) {
    const task = taskMap.get(taskId);
    setEditing(taskId);
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

  function requestComplete() {
    if (!editingTask || busy) return;
    setEditorError(null);
    setCompleteConfirm(true);
  }

  async function markComplete() {
    if (!editing || busy) return;
    const task = taskMap.get(editing);
    if (!task) return;
    setBusy(true);
    setEditorError(null);
    try {
      const preState: WorkTaskState | null =
        task.state === "待确认"
          ? "待开始"
          : task.state === "待开始" || task.state === "暂停"
            ? "进行中"
            : null;
      if (preState) {
        try {
          await shell.taskTransition(editing, preState);
        } catch (e) {
          setEditorError("先转为" + preState + "失败：" + errText(e));
          return;
        }
      }
      try {
        await shell.taskTransition(editing, "已完成");
      } catch (e) {
        setEditorError(
          (preState ? "已转为" + preState + "，但" : "") + "标记完成失败：" + errText(e),
        );
        if (preState) setTick((n) => n + 1);
        return;
      }
      setCompleteConfirm(false);
      setEditing(null);
      setReviewSeed({ projectId: task.project_id ?? "", nonce: Date.now() });
      announce({
        kind: "ok",
        text: task.project_id
          ? "任务已完成并归档；可在下方「项目复盘」生成复盘。"
          : "任务已完成并归档；该任务未挂接项目，可先在「项目」区挂接后再复盘。",
      });
      setTick((n) => n + 1);
    } finally {
      setBusy(false);
    }
  }

  /** OD-UI-900：从页头扇形「+」直接建任务（可带截止日期与项目挂接）。 */
  async function createQuickTask() {
    const title = quickTaskTitle.trim();
    if (!title) return;
    setQuickTaskBusy(true);
    try {
      await shell.taskCreate({
        title,
        due_date: quickTaskDue || null,
        project_id: quickTaskProject || null,
      });
      setQuickTaskOpen(false);
      setQuickTaskTitle("");
      setQuickTaskDue("");
      setQuickTaskProject("");
      announce({ kind: "ok", text: "已新建任务「" + title + "」。" });
      setTick((n) => n + 1);
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setQuickTaskBusy(false);
    }
  }

  async function removeTask() {
    if (!deleteTaskId || busy) return;
    setBusy(true);
    try {
      await shell.taskDelete(deleteTaskId);
      announce({ kind: "ok", text: "任务已删除。" });
      setDeleteTaskId(null);
      setEditing(null);
      setTick((n) => n + 1);
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
      setDeleteTaskId(null);
    } finally {
      setBusy(false);
    }
  }

  const editingTask = editing ? taskMap.get(editing) ?? null : null;

  const quadrantList = (
    <div className="pj-quads">
      {QUADRANT_COLUMNS.slice(0, 4).map((column) => {
        const items = visibleViews.filter((view) => view.quadrant === column.key);
        const expanded = expandedQuadrants[column.key];
        const important = column.key === "Q1" || column.key === "Q2";
        const urgent = column.key === "Q1" || column.key === "Q3";
        return (
          <section
            key={column.key}
            aria-label={column.label}
            data-quadrant-column={column.key}
            className="pj-quad"
          >
            <button
              type="button"
              className="pj-quad-head"
              aria-expanded={expanded}
              onClick={() =>
                setExpandedQuadrants((previous) => ({
                  ...previous,
                  [column.key]: !previous[column.key],
                }))
              }
            >
              <span className={"pj-quad-dot is-" + column.key} aria-hidden="true" />
              <span className="pj-quad-name">{column.label}</span>
              <span className="pj-quad-count">{items.length}</span>
              <Icon
                name={expanded ? "chevron-up" : "chevron-down"}
                className="pj-quad-chev"
              />
            </button>
            {expanded ? (
              <div className="pj-quad-list">
                {items.length === 0 ? (
                  <span className="pj-quad-empty" aria-label="此象限暂无任务">
                    0
                  </span>
                ) : (
                  items.map((view) => {
                    const task = taskMap.get(view.task_id);
                    const state = task?.state ?? "待开始";
                    const due = task?.due_date ? dayStart(task.due_date) : null;
                    const overdue =
                      due !== null &&
                      state !== "已完成" &&
                      state !== "已取消" &&
                      due.getTime() < nowMs;
                    const overdueDays =
                      overdue && due
                        ? Math.max(1, Math.floor((nowMs - due.getTime()) / 86_400_000))
                        : 0;
                    return (
                      <button
                        key={view.task_id}
                        type="button"
                        data-task-quadrant-row={view.task_id}
                        onClick={() => openEditor(view.task_id)}
                        title={view.title}
                        className="pj-quad-row"
                      >
                        <StateChip state={state} />
                        {important ? (
                          <span className="pj-quad-flag is-important" title="重要" aria-label="重要">
                            <Icon name="star" className="h-3 w-3" />
                          </span>
                        ) : null}
                        {urgent ? (
                          <span className="pj-quad-flag is-urgent" title="紧急" aria-label="紧急">
                            <Icon name="bell" className="h-3 w-3" />
                          </span>
                        ) : null}
                        <span className="pj-quad-row-title">{view.title}</span>
                        <span className={overdue ? "pj-quad-row-side is-overdue" : "pj-quad-row-side"}>
                          {overdue
                            ? "逾期 " + overdueDays + " 天"
                            : task?.due_date && due
                              ? mmdd(due)
                              : "—"}
                        </span>
                      </button>
                    );
                  })
                )}
              </div>
            ) : null}
          </section>
        );
      })}
    </div>
  );

  const projectHistory = (
    <section className="pj-history">
      <button
        type="button"
        className="pj-quad-head"
        aria-expanded={historyOpen}
        onClick={() => setHistoryOpen((open) => !open)}
      >
        <span className="pj-history-mark" aria-hidden="true">
          <Icon name="history" className="h-3.5 w-3.5" />
        </span>
        <span className="pj-quad-name">历史会话</span>
        <span className="pj-quad-count">{projectThreads.length}</span>
        <Icon
          name={historyOpen ? "chevron-up" : "chevron-down"}
          className="pj-quad-chev"
        />
      </button>
      {historyOpen ? (
        <div className="pj-quad-list">
          {projectThreads.length === 0 ? (
            <span className="pj-quad-empty">0</span>
          ) : (
            projectThreads.map((thread) => (
              <button
                key={thread.id}
                type="button"
                className="pj-history-row"
                title={thread.title}
                onClick={() => {
                  requestOpenThread(thread.id);
                  onNavigate?.("home");
                }}
              >
                <Icon name="chat" className="h-3 w-3 shrink-0" />
                <span className="pj-history-title">{thread.title || "未命名对话"}</span>
                <span className="pj-history-date mono">{threadDate(thread.updatedAt)}</span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </section>
  );

  const gantt = (
    <section aria-label="任务甘特图" data-pj-gantt="" className="pj-panel min-w-0">
      <div className="pj-panel-head">
        <span className="pj-panel-ic" aria-hidden="true">
          <Icon name="calendar" className="h-3.5 w-3.5" />
        </span>
        <div className="range-switch-pj ml-auto flex gap-1.5">
          {RANGE_OPTIONS.map((option) => (
            <button
              key={option.key}
              type="button"
              onClick={() => setRange(option.key)}
              aria-pressed={range === option.key}
              className={
                "rounded-full px-3 py-1.5 text-[11px] transition " +
                (range === option.key
                  ? "bg-[color:var(--od-neu-accent-soft)] font-medium text-[color:var(--od-neu-accent-ink)] shadow-[var(--od-neu-inset-1)]"
                  : NEU_TILE + " text-[color:var(--od-neu-ink-2)]")
              }
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      <div className="pj-gantt">
        {ganttGroups.length === 0 ? (
          <p className="pt-2 text-[11.5px] text-[color:var(--od-neu-ink-3)]">
            当前范围内没有带截止日期的任务。
          </p>
        ) : (
          <div className="relative flex flex-col" data-pj-gantt-body="">
            <div className="pj-gantt-scale" aria-hidden="true">
              <span />
              <span className="pj-gantt-scale-cells">
                {ticks.map((day, index) => (
                  <span
                    key={index}
                    className={
                      "pj-gantt-scale-lbl" +
                      (day.toDateString() === today.toDateString() ? " is-today" : "")
                    }
                    style={{ left: (index / 6) * 100 + "%" }}
                  >
                    {mmdd(day)}
                  </span>
                ))}
              </span>
            </div>
            <span
              aria-hidden="true"
              className="pj-today-line pointer-events-none absolute -top-2 bottom-0"
              style={{
                left:
                  "calc(var(--pj-rail) + (100% - var(--pj-rail)) * " +
                  Math.max(0, Math.min(1, todayPercent / 100)) + ")",
              }}
            >
              <span className="pj-today-dot" />
            </span>
            {ganttGroups.map((group) => {
              const collapsed = isGroupCollapsed(group.projectId);
              const summary = ganttSummaries.get(group.projectId) ?? {
                ...group.summary,
                ratio: 0,
              };
              const rangeLabel = summaryRangeLabel(summary);
              return (
                <div
                  key={group.projectId || "unattached"}
                  data-project-gantt-group={group.projectId}
                  data-collapsed={collapsed}
                  className="pj-proj-row"
                >
                  <button
                    type="button"
                    onClick={() => toggleGroup(group.projectId)}
                    aria-expanded={!collapsed}
                    className="pj-proj-rail text-left"
                    data-project-gantt-group-header={group.projectId}
                  >
                    <span className="flex min-w-0 items-center gap-1">
                      <Icon
                        name={collapsed ? "chevron-down" : "chevron-up"}
                        className="h-3 w-3 shrink-0 text-[color:var(--od-neu-ink-3)]"
                      />
                      <span
                        title={group.projectName}
                        className="pj-proj-name truncate"
                      >
                        {group.projectName}
                      </span>
                    </span>
                    <span className="pj-proj-range">
                      {group.rows.length} 项 · {rangeLabel}
                    </span>
                  </button>
                  <div className="pj-proj-canvas">
                    {collapsed ? (
                      <div className="pj-lane is-compact" />
                    ) : (
                      group.rows.map(({ task, left, width, clamped }) => {
                        const progress = taskProgressRatio(task, nowMs);
                        const done = task.state === "已完成";
                        const ghost = task.state === "待开始" || task.state === "待确认" || task.state === "已取消";
                        const solid = done ? "is-ink" : ghost ? "is-ghost" : "is-accent";
                        const dueDay = task.due_date ? dayStart(task.due_date) : null;
                        const overdue =
                          !done &&
                          task.state !== "已取消" &&
                          dueDay !== null &&
                          dueDay.getTime() < nowMs;
                        return (
                          <div className="pj-lane" key={task.id}>
                            <button
                              type="button"
                              key={task.id}
                              data-task-gantt-row={task.id}
                              onClick={() => openEditor(task.id)}
                              title={task.title + " · " + task.state}
                              className={
                                "pj-bar " + solid +
                                (overdue ? " is-overdue" : "") +
                                (clamped && left <= 0 ? " is-clamped-left" : "") +
                                (clamped && left + width >= 100 ? " is-clamped-right" : "")
                              }
                              style={{ left: left + "%", width: Math.max(8, width) + "%" }}
                            >
                              <span className="pj-bar-name">{task.title}</span>
                              <span className="pj-bar-meta">
                                <span
                                  aria-hidden="true"
                                  data-task-gantt-plan={task.id}
                                  className={
                                    "pj-bar-fill " + TASK_PLAN_BAR[task.state]
                                  }
                                  style={{ width: Math.round(width * progress * 100) + "%" }}
                                />
                              </span>
                            </button>
                            <div
                              className="pj-bar-tools"
                              style={{ left: "calc(" + Math.min(99, left + width) + "% - 14px)" }}
                            >
                              <button
                                type="button"
                                className="pj-bar-tool"
                                aria-label={"修改任务 " + task.title}
                                data-hover-hint="修改任务"
                                onClick={() => openEditor(task.id)}
                              >
                                <Icon name="edit" className="h-3 w-3" />
                              </button>
                              <button
                                type="button"
                                className="pj-bar-tool is-danger"
                                aria-label={"删除任务 " + task.title}
                                data-hover-hint="删除任务"
                                onClick={() => setDeleteTaskId(task.id)}
                              >
                                <Icon name="trash" className="h-3 w-3" />
                              </button>
                            </div>
                          </div>
                        );
                      })
                    )}
                  </div>
                </div>
              );
            })}
            {collapsedCount > 0 ? (
              <p className="pt-3 text-[10.5px] text-[color:var(--od-neu-ink-3)]">
                已折叠 {collapsedCount} 组，展开查看
              </p>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );

  const projectCardsBlock = (
    <section className="pj-project-cards" aria-label="最近项目">
      {visibleProjectCards.length === 0 ? (
        <p className="pj-project-empty">暂无项目</p>
      ) : (
        <div
          ref={projectRowRef}
          className="pj-project-card-row"
          style={{
            gridTemplateColumns: `repeat(${visibleProjectCards.length}, minmax(0, 1fr))`,
          }}
        >
          {visibleProjectCards.map(({ project, total, done, ratio }) => (
            <article className="pj-project-card" data-project-card={project.project_id} key={project.project_id}>
              <div className="pj-project-card-head">
                <span
                  className="pj-state-dot"
                  data-task-state={project.status}
                  title={project.status}
                  aria-label={project.status}
                />
                <h3 title={project.name}>{project.name}</h3>
              </div>
              <div className="pj-project-card-progress">
                <span>进度</span>
                <span className="mono">{Math.round(ratio * 100)}%</span>
              </div>
              <span className="pj-project-card-track" aria-hidden="true">
                <span style={{ width: Math.round(ratio * 100) + "%" }} />
              </span>
              <div className="pj-project-card-foot">
                <div className="pj-project-tools">
                  <button
                    type="button"
                    className="pj-project-tool"
                    aria-label={"管理项目 " + project.name}
                    data-hover-hint="管理项目与任务"
                    onClick={() => {
                      setManageProjectId(project.project_id);
                      setManageOpen(true);
                    }}
                  >
                    <Icon name="branch" className="h-3 w-3" />
                  </button>
                  <button
                    type="button"
                    className="pj-project-tool"
                    aria-label={"生成项目复盘 " + project.name}
                    data-hover-hint="生成项目复盘"
                    onClick={() =>
                      setReviewSeed({ projectId: project.project_id, nonce: Date.now() })}
                  >
                    <Icon name="refresh" className="h-3 w-3" />
                  </button>
                </div>
                <span className="pj-project-count mono">
                  {done}/{total} 任务
                </span>
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );

  if (variant === "panel") {
    return (
      <section aria-label="任务四象限" data-task-quadrant="" className="min-w-0">
        {error ? (
          <p className="mb-2 rounded-2xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
            读取四象限失败：{error}
          </p>
        ) : null}
        {quadrantList}
      </section>
    );
  }

  return (
    <section
      aria-label="项目跟踪"
      data-task-quadrant=""
      className="pj-page relative mx-auto max-w-[1600px]"
    >
      <div className="pj-head">
        <div className="pj-head-left">
          <h2 className="pj-head-title">项目跟踪</h2>
          <div className="pj-statusbar" data-pj-statusbar="">
            <button
              type="button"
              className="pj-chip"
              data-pj-chip-done=""
              onClick={() => setStatusModal("done")}
              title={"已完成 " + stats.done + " 项"}
            >
              <span className="pj-chip-ic">
                <Icon name="check" className="h-3.5 w-3.5" />
              </span>
              <span className="pj-chip-num">{stats.done}</span>
              <span className="pj-chip-label">已完成</span>
            </button>
            <button
              type="button"
              className="pj-chip"
              data-pj-chip-running=""
              onClick={() => setStatusModal("running")}
              title={"进行中 " + stats.running + " 项"}
            >
              <span className="pj-chip-ic">
                <Icon name="clock" className="h-3.5 w-3.5" />
              </span>
              <span className="pj-chip-num">{stats.running}</span>
              <span className="pj-chip-label">进行中</span>
            </button>
            <button
              type="button"
              className={"pj-chip" + (stats.overdue > 0 ? " is-warn" : "")}
              data-pj-chip-overdue=""
              onClick={() => setStatusModal("overdue")}
              title={"逾期 " + stats.overdue + " 项"}
            >
              <span className="pj-chip-ic">
                <Icon name="bell" className="h-3.5 w-3.5" />
              </span>
              <span className="pj-chip-num">{stats.overdue}</span>
              <span className="pj-chip-label">逾期</span>
            </button>
          </div>
        </div>
        <div className="pj-head-actions">
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索任务"
            aria-label="搜索任务"
            className={
              "h-8 w-[150px] rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] placeholder:text-[color:var(--od-neu-ink-3)] focus:outline-none " +
              NEU_INSET
            }
          />
          <button
            type="button"
            className="pj-fab"
            aria-label="项目管理"
            data-hover-hint="项目管理"
            onClick={() => {
              setManageProjectId("");
              setNewProjectNonce((value) => value + 1);
              setManageOpen(true);
            }}
          >
            <Icon name="folder" className="h-4 w-4" />
          </button>
        </div>
      </div>

      {localNotice ? (
        <p
          role="status"
          className={
            "relative mb-3 rounded-2xl px-4 py-2 text-[11.5px] leading-relaxed " +
            (localNotice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-[color:var(--od-neu-sunken)] text-[color:var(--od-neu-ink-2)]")
          }
        >
          {localNotice.text}
        </p>
      ) : null}
      {error ? (
        <p className="relative mb-3 rounded-2xl bg-error-container/70 px-4 py-2 text-[11.5px] leading-relaxed text-on-error-container">
          读取任务失败：{error}
        </p>
      ) : null}
      {loading ? (
        <p className="relative pb-2 text-[11.5px] text-[color:var(--od-neu-ink-3)]">正在读取任务…</p>
      ) : null}

      <div className="pj-main relative">
        <aside className="pj-panel pj-left-panel min-w-0" data-pj-quadrant-panel="">
          <div className="pj-left-head">
            <span className="pj-panel-ic" aria-hidden="true">
              <Icon name="checklist" className="h-3.5 w-3.5" />
            </span>
            <h3 className="sr-only">任务</h3>
            <span className="pj-left-count mono">{visibleViews.length}</span>
            <button
              type="button"
              className="pj-fab"
              aria-label="新建任务"
              data-hover-hint="新建任务"
              onClick={() => {
                setQuickTaskProject("");
                setQuickTaskOpen(true);
              }}
            >
              <Icon name="plus" className="h-4 w-4" />
            </button>
          </div>
          {quadrantList}
          {projectHistory}
        </aside>

        <div className="pj-right min-w-0">
          {gantt}
          {projectCardsBlock}
        </div>
      </div>

      {statusModal ? (
        <div
          className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
          role="presentation"
          onClick={() => setStatusModal(null)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label={statusModal === "done" ? "已完成任务" : statusModal === "running" ? "进行中任务" : "逾期任务"}
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog flex max-h-[80vh] w-full max-w-[560px] flex-col overflow-hidden p-6"
          >
            <div className="flex items-center gap-3 pb-4">
              <span className="pj-panel-ic" aria-hidden="true">
                <Icon name={statusModal === "done" ? "check" : statusModal === "running" ? "clock" : "bell"} className="h-3.5 w-3.5" />
              </span>
              <h3 className="text-[13px] font-semibold text-[color:var(--od-neu-ink)]">
                {statusModal === "done" ? "已完成任务" : statusModal === "running" ? "进行中任务" : "逾期任务"}
              </h3>
              <span className="mono text-[11px] text-[color:var(--od-neu-ink-3)]">{statusTasks.length}</span>
              <button
                type="button"
                aria-label="关闭"
                data-hover-hint="关闭"
                onClick={() => setStatusModal(null)}
                className={"ml-auto flex h-9 w-9 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>
            <div className="pj-status-list">
              {statusTasks.length === 0 ? (
                <p className="pj-status-empty">暂无</p>
              ) : (
                statusTasks.map((task) => {
                  const due = task.due_date ? dayStart(task.due_date) : null;
                  const overdue =
                    due !== null &&
                    task.state !== "已完成" &&
                    task.state !== "已取消" &&
                    due.getTime() < nowMs;
                  return (
                    <button
                      key={task.id}
                      type="button"
                      className="pj-status-row"
                      onClick={() => {
                        setStatusModal(null);
                        openEditor(task.id);
                      }}
                    >
                      <span className="pj-quad-row-ic" data-task-state={task.state} aria-hidden="true">
                        <Icon name={taskStateIcon(task.state)} className="h-3.5 w-3.5" />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="pj-quad-row-title">{task.title}</span>
                        <span className="pj-quad-row-meta">
                          <span className={overdue ? "pj-quad-row-state is-overdue" : "pj-quad-row-state"}>
                            {task.state}
                          </span>
                          {task.due_date ? (
                            <>
                              <span className="pj-quad-meta-dot" aria-hidden="true" />
                              <Icon name="clock" className="h-2.5 w-2.5" />
                              <span className="mono">{mmdd(due ?? new Date(task.due_date))}</span>
                            </>
                          ) : null}
                        </span>
                      </span>
                    </button>
                  );
                })
              )}
            </div>
          </div>
        </div>
      ) : null}

      {manageOpen ? (
        <div
          className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
          role="presentation"
          onClick={() => setManageOpen(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="项目与任务管理"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog flex max-h-[86vh] w-full max-w-[720px] flex-col overflow-y-auto p-6"
          >
            <div className="flex items-center gap-3 pb-3">
              <span className="pj-panel-ic" aria-hidden="true">
                <Icon name="branch" className="h-3.5 w-3.5" />
              </span>
              <h3 className="text-[16px] font-semibold text-[color:var(--od-neu-ink)]">项目与任务</h3>
              <button
                type="button"
                aria-label="关闭"
                data-hover-hint="关闭"
                onClick={() => setManageOpen(false)}
                className={"ml-auto flex h-9 w-9 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>
            <ProjectMaintenance
              focusProjectId={manageProjectId || null}
              newProjectNonce={newProjectNonce}
              onNotice={announce}
              onChanged={() => setTick((n) => n + 1)}
            />
          </div>
        </div>
      ) : null}

      {reviewSeed ? (
        <div
          className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
          role="presentation"
          onClick={() => setReviewSeed(null)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="项目复盘"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog flex max-h-[86vh] w-full max-w-[560px] flex-col overflow-y-auto p-5"
          >
            <div className="flex items-center gap-3 pb-3">
              <span className="pj-panel-ic" aria-hidden="true">
                <Icon name="refresh" className="h-3.5 w-3.5" />
              </span>
              <h3 className="text-[16px] font-semibold text-[color:var(--od-neu-ink)]">项目复盘</h3>
              <button
                type="button"
                aria-label="关闭"
                data-hover-hint="关闭"
                onClick={() => setReviewSeed(null)}
                className={"ml-auto flex h-9 w-9 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>
            <ProjectReviewCard
              seed={reviewSeed}
              onNotice={announce}
              onChanged={() => setTick((n) => n + 1)}
            />
          </div>
        </div>
      ) : null}

      {quickTaskOpen ? (
        <div
          className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
          role="presentation"
          onClick={() => setQuickTaskOpen(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="新建任务"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog w-full max-w-[420px] p-6"
          >
            <div className="flex items-start gap-3 pb-4">
              <span className="flex h-10 w-10 items-center justify-center rounded-[13px] bg-[color:var(--od-neu-accent)] text-white shadow-[var(--od-neu-raise-1)]">
                <Icon name="edit" className="h-4 w-4" />
              </span>
              <div className="min-w-0 flex-1">
                <h3 className="text-[15px] font-semibold text-[color:var(--od-neu-ink)]">新建任务</h3>
              </div>
              <button
                type="button"
                aria-label="关闭"
                data-hover-hint="关闭"
                onClick={() => setQuickTaskOpen(false)}
                className={"flex h-9 w-9 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>
            <div className="flex flex-col gap-4">
              <input
                value={quickTaskTitle}
                onChange={(event) => setQuickTaskTitle(event.target.value)}
                placeholder="任务标题"
                aria-label="任务标题"
                autoFocus
                className={
                  "h-10 w-full rounded-2xl px-3.5 text-[12.5px] text-[color:var(--od-neu-ink)] placeholder:text-[color:var(--od-neu-ink-3)] focus:outline-none " +
                  NEU_INSET
                }
              />
              <div className="flex gap-2.5">
                <input
                  type="date"
                  value={quickTaskDue}
                  onChange={(event) => setQuickTaskDue(event.target.value)}
                  aria-label="截止日期（可选）"
                  className={"h-10 flex-1 rounded-2xl px-3.5 text-[12px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                />
                <select
                  value={quickTaskProject}
                  onChange={(event) => setQuickTaskProject(event.target.value)}
                  aria-label="挂接项目（可选）"
                  className={"h-10 max-w-[160px] flex-1 rounded-2xl px-3 text-[12px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                >
                  <option value="">不挂接项目</option>
                  {projects.map((project) => (
                    <option key={project.project_id} value={project.project_id}>
                      {project.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="mt-5 flex justify-end gap-2.5">
              <button
                type="button"
                onClick={() => setQuickTaskOpen(false)}
                className={"rounded-full px-4 py-2 text-[11.5px] text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                取消
              </button>
              <button
                type="button"
                onClick={() => void createQuickTask()}
                disabled={quickTaskBusy || !quickTaskTitle.trim()}
                className="rounded-full bg-[color:var(--od-neu-accent)] px-5 py-2 text-[11.5px] font-medium text-white shadow-[var(--od-neu-raise-1)] disabled:opacity-50"
              >
                {quickTaskBusy ? "创建中…" : "创建任务"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <ConfirmDialog
        open={Boolean(deleteTaskId)}
        title="删除任务？"
        description={"「" + (deleteTaskId ? taskMap.get(deleteTaskId)?.title ?? "" : "") + "」删除后不可恢复。"}
        confirmLabel="删除"
        onConfirm={() => void removeTask()}
        onCancel={() => setDeleteTaskId(null)}
        testId="task-delete-confirm"
      />

      <ConfirmDialog
        open={completeConfirm && Boolean(editingTask)}
        title="标记任务完成？"
        description={
          "「" + (editingTask?.title ?? "") +
          "」完成后将从四象限归档，只出现在「已完成」列表。"
        }
        confirmLabel="标记完成"
        onConfirm={() => void markComplete()}
        onCancel={() => setCompleteConfirm(false)}
        testId="task-complete-confirm"
      />

      {editingTask ? (
        <div
          className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
          role="presentation"
          onClick={() => setEditing(null)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="编辑任务"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog w-full max-w-[520px] p-6"
          >
            <div className="flex items-start gap-3 pb-4">
              <span className="flex h-10 w-10 items-center justify-center rounded-[13px] bg-[color:var(--od-neu-accent)] text-white shadow-[var(--od-neu-raise-1)]">
                <Icon name="edit" className="h-4 w-4" />
              </span>
              <div className="min-w-0 flex-1">
                <h3 title={editingTask.title} className="truncate text-[15px] font-semibold text-[color:var(--od-neu-ink)]">
                  {editingTask.title}
                </h3>
                <p className="mt-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">
                  {editingTask.state} · 创建于 {isoDay(editingTask.created_at)}
                </p>
              </div>
              <button
                type="button"
                aria-label="关闭"
                data-hover-hint="关闭"
                onClick={() => setEditing(null)}
                className={"flex h-9 w-9 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>

            <div className="flex flex-col gap-4">
              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[color:var(--od-neu-ink-3)]">重要性</p>
                <div className="flex flex-wrap gap-2">
                  {IMPORTANCE_CHOICES.map((level) => (
                    <button
                      key={level}
                      type="button"
                      onClick={() => setImportanceDraft(level)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (importanceDraft === level
                          ? "bg-[color:var(--od-neu-accent-soft)] font-medium text-[color:var(--od-neu-accent-ink)] shadow-[var(--od-neu-inset-1)]"
                          : NEU_TILE + " text-[color:var(--od-neu-ink-2)]")
                      }
                    >
                      {level}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[color:var(--od-neu-ink-3)]">象限</p>
                <div className="flex flex-wrap gap-2">
                  {QUADRANT_CHOICES.map((choice) => (
                    <button
                      key={choice}
                      type="button"
                      onClick={() => setQuadrantDraft(choice)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (quadrantDraft === choice
                          ? "bg-[color:var(--od-neu-accent-soft)] font-medium text-[color:var(--od-neu-accent-ink)] shadow-[var(--od-neu-inset-1)]"
                          : NEU_TILE + " text-[color:var(--od-neu-ink-2)]")
                      }
                    >
                      {choice === "自动" ? "自动" : QUADRANT_SHORT[choice] ?? choice}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <p className="pb-2 text-[10.5px] tracking-[0.14em] text-[color:var(--od-neu-ink-3)]">截止日期</p>
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    type="date"
                    value={dueDraft}
                    onChange={(event) => setDueDraft(event.target.value)}
                    aria-label="截止日期"
                    className={"h-9 rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                  />
                  {(["target", "hard"] as TaskDueKind[]).map((kind) => (
                    <button
                      key={kind}
                      type="button"
                      onClick={() => setDueKindDraft(kind)}
                      className={
                        "rounded-full px-3.5 py-1.5 text-[11.5px] transition " +
                        (dueKindDraft === kind
                          ? "bg-[color:var(--od-neu-accent-soft)] font-medium text-[color:var(--od-neu-accent-ink)] shadow-[var(--od-neu-inset-1)]"
                          : NEU_TILE + " text-[color:var(--od-neu-ink-2)]")
                      }
                    >
                      {kind === "hard" ? "硬截止" : "目标日"}
                    </button>
                  ))}
                </div>
              </div>

              <label className="block">
                <span className="block pb-2 text-[10.5px] tracking-[0.14em] text-[color:var(--od-neu-ink-3)]">调整理由</span>
                <input
                  value={reasonDraft}
                  onChange={(event) => setReasonDraft(event.target.value)}
                  placeholder="调整重要性或截止日期时必填"
                  className={
                    "h-10 w-full rounded-2xl px-3.5 text-[11.5px] text-[color:var(--od-neu-ink)] placeholder:text-[color:var(--od-neu-ink-3)] " +
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

            <div className="mt-5 flex items-center justify-between gap-2.5">
              {editingTask.state !== "已完成" && editingTask.state !== "已取消" ? (
                <button
                  type="button"
                  onClick={requestComplete}
                  disabled={busy}
                  className="rounded-full bg-[color:var(--od-neu-accent-soft)] px-4 py-2 text-[11.5px] font-medium text-[color:var(--od-neu-accent-ink)] disabled:opacity-50"
                  data-task-complete-entry=""
                >
                  标记完成
                </button>
              ) : (
                <span />
              )}
              <div className="flex gap-2.5">
                <button
                  type="button"
                  onClick={() => setEditing(null)}
                  className={"rounded-full px-4 py-2 text-[11.5px] text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={() => void saveEditor()}
                  disabled={busy}
                  className="rounded-full bg-[color:var(--od-neu-accent)] px-5 py-2 text-[11.5px] font-medium text-white shadow-[var(--od-neu-raise-1)] disabled:opacity-50"
                >
                  {busy ? "保存中…" : "保存修改"}
                </button>
              </div>
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
