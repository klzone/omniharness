/**
 * 项目跟进页（OD-ARCH-098 S03 → OD-ARCH-102 S02）：左「项目事项 + 历史对话」+
 * 右「甘特时间线 / 阶段成果」。
 *
 * 参考图是任务跟踪看板（左列表、右按行对齐的甘特）。这里只搬版式骨架：
 * 左栏承载「这件事是谁的、期限、下一步」，右栏承载「它落在时间轴的哪儿」。
 *
 * 诚实性：
 *   * 事项由用户在本页录入，存到应用数据根下的 project-items.v1.json；不调模型；
 *   * 事项行没有进度字段，因此界面**不显示任何百分比**，只画状态色带；
 *   * 没有开始日期的事项画成单日菱形里程碑，不倒推一段虚构工期；
 *   * 录入前右栏不画空轴，只给一句引导。
 */

import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Icon } from "../components/Icon";
import { shell } from "../api";
import type {
  MaterialDetail,
  ProjectInfo,
  ProjectKnowledgeRef,
  ProjectOverview,
  ProjectRefItem,
  ProjectStatus,
  ProjectItem,
  ProjectItemRecord,
  ProjectItemStatus,
  WritingMaterial,
  WritingMaterialReviewStatus,
} from "../types";
import {
  filterItems,
  ganttTicks,
  ganttWindow,
  layoutItems,
  recordsToItems,
  todayPct,
  toDayNumber,
  type GanttBar,
} from "../projectGantt";
import type { SceneThread } from "../sceneThreads";
import { ConfirmDialog } from "./ConfirmDialog";

const STATUS_LABEL: Record<ProjectItemStatus, string> = {
  not_started: "未开始",
  in_progress: "进行中",
  blocked: "受阻",
  done: "已完成",
};

/** 状态 → 视觉：徽标、色带、甘特条。全部走既有语义色，不引入参考图的珊瑚色。 */
const STATUS_STYLE: Record<
  ProjectItemStatus,
  { chip: string; rail: string; bar: string; dot: string }
> = {
  not_started: {
    chip: "bg-surface-container text-on-surface-variant",
    rail: "bg-outline-variant",
    bar: "bg-outline/45",
    dot: "bg-outline/60",
  },
  in_progress: {
    chip: "bg-primary/10 text-primary",
    rail: "bg-primary/70",
    bar: "bg-primary/75",
    dot: "bg-primary",
  },
  blocked: {
    chip: "bg-amber-500/15 text-amber-700",
    rail: "bg-amber-500",
    bar: "bg-amber-500/80",
    dot: "bg-amber-600",
  },
  done: {
    chip: "bg-secondary/10 text-secondary",
    rail: "bg-secondary/70",
    bar: "bg-secondary/70",
    dot: "bg-secondary",
  },
};

function statusOf(value: string): ProjectItemStatus {
  return value in STATUS_STYLE ? (value as ProjectItemStatus) : "not_started";
}

/** 期限是否已过：以宿主算好的 overdue 为准，不在这里另算一套口径。 */
function dateText(date: string | null | undefined): string {
  if (!date) return "未填期限";
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  return m ? `${m[1]}.${m[2]}.${m[3]}` : date;
}

function daysBetween(a: string, b: string): number | null {
  const x = toDayNumber(a);
  const y = toDayNumber(b);
  if (x === null || y === null) return null;
  return y - x;
}

/** 一条卡片的辅助文案：距期限还剩几天（只描述，不做提醒）。 */
function dueHint(item: ProjectItem, today: string): string {
  const diff = daysBetween(today, item.due);
  if (diff === null) return "";
  if (item.status === "done") return "已完成";
  if (diff === 0) return "今天到期";
  if (diff > 0) return `还有 ${diff} 天`;
  return `已过 ${-diff} 天`;
}

export function ProjectBoard({
  records,
  today,
  error,
  loading,
  saving,
  storageNote,
  threads,
  subtitleProjects,
  subtitleTimeline,
  subtitleHistory,
  rightView,
  onRightView,
  onSave,
  onOpenThread,
  onNewThread,
  artifactsSlot,
}: {
  /** 本机事项记录（OD-ARCH-102）：读写都走宿主命令，页面不调模型。 */
  records: ProjectItemRecord[];
  today: string;
  /** 读取失败时的原文；不当成「没有事项」。 */
  error: string | null;
  loading: boolean;
  /** 保存进行中：按钮置灰，避免连点写出两份。 */
  saving: boolean;
  /** 首次使用、从备份恢复等说明。 */
  storageNote: string | null;
  /** 本场景的历史对话（点击回首页继续）。 */
  threads: SceneThread[];
  subtitleProjects?: string;
  subtitleTimeline?: string;
  subtitleHistory?: string;
  /** 右栏次级视图：时间线 / 阶段成果（OD-ARCH-098 R5，保留既有成果入口）。 */
  rightView: "timeline" | "artifacts";
  onRightView: (view: "timeline" | "artifacts") => void;
  /** 整批写回；成功后调用方刷新 records。 */
  onSave: (items: ProjectItemRecord[]) => Promise<void>;
  onOpenThread: (id: string) => void;
  onNewThread: () => void;
  /** 阶段成果内容由调用方注入，避免这里再引一次成果目录命令。 */
  artifactsSlot?: ReactNode;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const [formOpen, setFormOpen] = useState(false);
  /** 正在修改的原事项名（null = 新增）。改名时按旧名定位。 */
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
  const [formError, setFormError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<ProjectItemRecord | null>(null);

  const items = useMemo(() => recordsToItems(records, today), [records, today]);
  const visible = useMemo(() => filterItems(items, query), [items, query]);
  const win = useMemo(() => ganttWindow(visible, today), [visible, today]);
  const bars = useMemo(() => layoutItems(visible, win), [visible, win]);
  const ticks = useMemo(() => ganttTicks(win), [win]);
  const nowPct = useMemo(() => todayPct(today, win), [today, win]);

  /** 点左栏卡片 → 右栏高亮并滚到对应行（只改视图）。 */
  useEffect(() => {
    if (!selected) return;
    rowRefs.current.get(selected)?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const empty = items.length === 0;
  const noMatch = !empty && visible.length === 0;

  function startCreate() {
    setEditingKey(null);
    setDraft(EMPTY_DRAFT);
    setFormError(null);
    setFormOpen(true);
  }

  function startEdit(record: ProjectItemRecord) {
    setEditingKey(record.item);
    setDraft(draftFrom(record));
    setFormError(null);
    setFormOpen(true);
  }

  async function submitDraft() {
    const next: Draft = {
      item: draft.item.trim(),
      owner: draft.owner.trim(),
      due: draft.due.trim(),
      start: draft.start.trim(),
      status: draft.status,
      next_step: draft.next_step.trim(),
    };
    const problem = validateDraft(next, records, editingKey);
    if (problem) {
      setFormError(problem);
      return;
    }
    const now = Math.floor(Date.now() / 1000);
    const previous = editingKey ? records.find((r) => r.item === editingKey) : undefined;
    const record: ProjectItemRecord = {
      item: next.item,
      owner: next.owner,
      due: next.due,
      start: next.start ? next.start : null,
      status: next.status,
      next_step: next.next_step,
      created_at: previous ? previous.created_at : now,
      updated_at: now,
    };
    const out = editingKey
      ? records.map((r) => (r.item === editingKey ? record : r))
      : records.concat(record);
    try {
      await onSave(out);
      setFormOpen(false);
      setEditingKey(null);
      setDraft(EMPTY_DRAFT);
      setFormError(null);
      setSelected(next.item);
    } catch (e) {
      setFormError(e instanceof Error ? e.message : String(e));
    }
  }

  async function confirmDelete() {
    if (!pendingDelete) return;
    const gone = pendingDelete.item;
    try {
      await onSave(records.filter((r) => r.item !== gone));
      if (selected === gone) setSelected(null);
      setPendingDelete(null);
      setFormError(null);
    } catch (e) {
      setPendingDelete(null);
      setFormError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div
      className={
        "grid grid-cols-1 gap-4 lg:gap-5 items-start max-w-7xl mx-auto md:-translate-x-8 " +
        "lg:grid-cols-[360px_minmax(0,1fr)]"
      }
      data-project-board=""
      data-board-width="nav"
    >
      {/* 项目概览（OD-KM-512）：选择器 + 508 反向派生计数 + 506 素材列表；不接管事项表。 */}
      <ProjectInsight />

      {/* ---------- 左栏：上「项目事项」、下「历史对话」（OD-ARCH-102 S02） ---------- */}
      <div className="min-w-0 flex flex-col gap-4 max-lg:order-1 lg:max-h-[74vh] lg:overflow-y-auto">
        <section
          aria-label="项目事项"
          className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0 flex flex-col"
        >
          <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
            <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
              项目事项
            </h2>
            {subtitleProjects ? (
              <span className="text-[12px] uppercase tracking-[0.16em] text-outline">
                {subtitleProjects}
              </span>
            ) : null}
            <span className="mono text-[12px] text-outline ml-auto shrink-0">
              {query.trim() ? visible.length + "/" + items.length : items.length} 条
            </span>
            <button
              onClick={startCreate}
              className="shrink-0 border border-primary/40 px-2 py-[3px] text-[11.5px] text-primary hover:bg-primary/5"
              aria-expanded={formOpen && editingKey === null}
            >
              + 新增
            </button>
          </div>

          {storageNote ? (
            <p className="pt-2.5 text-[11.5px] leading-relaxed text-outline" data-project-storage-note="">
              {storageNote}
            </p>
          ) : null}

          {formOpen ? (
            <form
              className="mt-3 border border-outline-variant/50 bg-surface-container-low/45 px-3 py-3"
              data-project-form={editingKey ? "edit" : "create"}
              onSubmit={(e) => {
                e.preventDefault();
                void submitDraft();
              }}
            >
              <p className="text-[12px] font-semibold text-on-surface">
                {editingKey ? "修改事项" : "新增事项"}
              </p>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <label className="col-span-2 block">
                  <span className="block text-[11.5px] text-outline">事项</span>
                  <input
                    value={draft.item}
                    onChange={(e) => setDraft({ ...draft, item: e.target.value })}
                    aria-label="事项"
                    placeholder="例如：完成章节提纲"
                    className={FIELD}
                  />
                </label>
                <label className="block">
                  <span className="block text-[11.5px] text-outline">负责人代号</span>
                  <input
                    value={draft.owner}
                    onChange={(e) => setDraft({ ...draft, owner: e.target.value })}
                    aria-label="负责人代号"
                    placeholder="P1"
                    className={FIELD + " mono"}
                  />
                </label>
                <label className="block">
                  <span className="block text-[11.5px] text-outline">状态</span>
                  <select
                    value={draft.status}
                    onChange={(e) => setDraft({ ...draft, status: statusOf(e.target.value) })}
                    aria-label="状态"
                    className={FIELD}
                  >
                    {(Object.keys(STATUS_LABEL) as ProjectItemStatus[]).map((st) => (
                      <option key={st} value={st}>
                        {STATUS_LABEL[st]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="block">
                  <span className="block text-[11.5px] text-outline">期限</span>
                  <input
                    type="date"
                    value={draft.due}
                    onChange={(e) => setDraft({ ...draft, due: e.target.value })}
                    aria-label="期限"
                    className={FIELD + " mono"}
                  />
                </label>
                <label className="block">
                  <span className="block text-[11.5px] text-outline">开始（可选）</span>
                  <input
                    type="date"
                    value={draft.start}
                    onChange={(e) => setDraft({ ...draft, start: e.target.value })}
                    aria-label="开始日期"
                    className={FIELD + " mono"}
                  />
                </label>
                <label className="col-span-2 block">
                  <span className="block text-[11.5px] text-outline">下一步</span>
                  <input
                    value={draft.next_step}
                    onChange={(e) => setDraft({ ...draft, next_step: e.target.value })}
                    aria-label="下一步"
                    placeholder="一句话写清下一个动作"
                    className={FIELD}
                  />
                </label>
              </div>
              {formError ? (
                <p className="mt-2 text-[11.5px] leading-relaxed text-error" data-project-form-error="">
                  {formError}
                </p>
              ) : null}
              <div className="mt-2.5 flex items-center justify-end gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setFormOpen(false);
                    setFormError(null);
                  }}
                  className="border border-outline-variant/60 px-2.5 py-1 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="bg-primary px-3 py-1 text-[11.5px] text-on-primary disabled:opacity-50"
                >
                  {saving ? "保存中…" : "保存"}
                </button>
              </div>
            </form>
          ) : null}

          <label className="mt-3 flex items-center gap-2 border border-outline-variant/60 bg-surface-container-low/70 px-2.5 py-1.5">
            <Icon name="search" className="w-3.5 h-3.5 text-outline" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索事项、负责人或下一步"
              aria-label="搜索项目事项"
              className="min-w-0 flex-1 bg-transparent text-[12.5px] text-on-surface outline-none placeholder:text-outline/70"
            />
            {query ? (
              <button
                className="text-[12px] text-outline hover:text-on-surface"
                onClick={() => setQuery("")}
                aria-label="清空搜索"
              >
                <Icon name="x" className="w-3 h-3" />
              </button>
            ) : null}
          </label>

          <div className="mt-3 flex flex-col gap-2.5">
            {error ? (
              <p className="text-[12px] text-error leading-relaxed" data-project-error="">
                读取本机事项失败：{error}。这里不把它当成「没有事项」。
              </p>
            ) : loading ? (
              <p className="text-[12px] text-outline leading-relaxed">正在读取本机事项…</p>
            ) : formError && !formOpen ? (
              <p className="text-[12px] text-error leading-relaxed">{formError}</p>
            ) : null}

            {empty ? (
              <p className="text-[12px] text-outline leading-relaxed" data-project-empty="">
                还没有项目事项。点右上角「+ 新增」录入第一条；事项只存在本机，不调用模型。
              </p>
            ) : noMatch ? (
              <p className="text-[12px] text-outline leading-relaxed">
                没有匹配「{query.trim()}」的事项。清空搜索看全部。
              </p>
            ) : (
              visible.map((item) => {
                const st = statusOf(item.status);
                const style = STATUS_STYLE[st];
                const active = selected === item.item;
                const record = records.find((r) => r.item === item.item);
                return (
                  <div
                    key={item.item}
                    data-project-card={st}
                    className={
                      "relative bg-surface-container-low/55 min-w-0 transition " +
                      (active ? "ring-1 ring-primary/60 bg-surface-container-low" : "")
                    }
                  >
                    {/* 状态色带：只表达状态，不是进度百分比。 */}
                    <span
                      aria-hidden="true"
                      className={"absolute left-0 top-0 bottom-0 w-[3px] " + style.rail}
                    />
                    <button
                      onClick={() => setSelected(active ? null : item.item)}
                      aria-pressed={active}
                      aria-label={
                        item.item +
                        " · " +
                        STATUS_LABEL[st] +
                        " · 负责人 " +
                        item.owner +
                        " · 期限 " +
                        item.due +
                        (item.overdue ? " · 已逾期" : "")
                      }
                      className="block w-full text-left px-3.5 pt-3 pb-1.5 min-w-0"
                    >
                      <span className="flex items-start gap-2 min-w-0">
                        <span className="min-w-0 flex-1 text-[13.5px] font-medium text-on-surface leading-[1.5] line-clamp-2 break-words">
                          {item.item}
                        </span>
                        <span className={"shrink-0 px-1.5 py-[1px] text-[11.5px] " + style.chip}>
                          {STATUS_LABEL[st]}
                        </span>
                      </span>
                      <span className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-outline">
                        <span className="mono">{item.owner}</span>
                        <span aria-hidden="true" className="text-outline-variant">·</span>
                        <span className={item.overdue ? "text-error" : ""}>{dateText(item.due)}</span>
                        <span className={item.overdue ? "text-error" : "text-outline/80"}>
                          {dueHint(item, today)}
                        </span>
                      </span>
                      <span
                        className="mt-1 block text-[12px] text-on-surface-variant leading-[1.6] line-clamp-1"
                        data-hover-hint={item.next_step}
                      >
                        下一步 · {item.next_step}
                      </span>
                    </button>
                    <div className="flex items-center gap-3 pl-3.5 pr-2.5 pb-2">
                      <button
                        onClick={() => record && startEdit(record)}
                        className="text-[11.5px] text-outline hover:text-primary"
                        data-project-action="edit"
                      >
                        修改
                      </button>
                      <button
                        onClick={() => record && setPendingDelete(record)}
                        className="text-[11.5px] text-outline hover:text-error"
                        data-project-action="delete"
                      >
                        删除
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </section>

        <section
          aria-label="历史对话"
          className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
          data-project-history=""
        >
          <div className="flex items-baseline gap-2.5 pb-3 border-b border-surface-variant/25">
            <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
              历史对话
            </h2>
            {subtitleHistory ? (
              <span className="text-[12px] uppercase tracking-[0.16em] text-outline">
                {subtitleHistory}
              </span>
            ) : null}
            <span className="mono text-[12px] text-outline ml-auto shrink-0">
              {threads.length} 条
            </span>
          </div>
          <div className="pt-2.5 flex flex-col gap-0.5">
            {threads.length === 0 ? (
              <p className="text-[12px] text-outline leading-relaxed">
                还没有这个场景的对话。对话只在首页发生，场景选「项目跟进」即可。
              </p>
            ) : (
              threads.slice(0, 8).map((t) => (
                <button
                  key={t.id}
                  onClick={() => onOpenThread(t.id)}
                  className="min-w-0 text-left px-1.5 py-1.5 text-[12px] text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                  data-hover-hint={t.title}
                >
                  <span className="block truncate">{t.title}</span>
                </button>
              ))
            )}
          </div>
          <button
            className="mt-2 text-[12px] text-primary hover:underline"
            onClick={onNewThread}
            data-hover-hint="回首页，按本场景开一段新对话"
          >
            去对话 · 新开
          </button>
        </section>
      </div>

      {/* ------------------------------ 右栏：甘特时间线 ------------------------------ */}
      <section
        aria-label="项目时间线"
        className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0 flex flex-col lg:max-h-[74vh] max-lg:order-2"
        data-project-gantt=""
      >
        <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
          <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
            {rightView === "timeline" ? "项目时间线" : "阶段成果"}
          </h2>
          {subtitleTimeline ? (
            <span className="text-[12px] uppercase tracking-[0.16em] text-outline">
              {subtitleTimeline}
            </span>
          ) : null}
          <span className="mono text-[12px] text-outline ml-auto shrink-0">
            {rightView === "timeline"
              ? win.start.slice(5).replace("-", ".") + " – " + win.end.slice(5).replace("-", ".")
              : items.length + " 项"}
          </span>
          {/* 次级切换：时间线与阶段成果同处右栏，不另开页面、不挤占左栏。 */}
          <div role="group" aria-label="右栏视图" className="flex items-center gap-1 shrink-0">
            {(
              [
                ["timeline", "时间线"],
                ["artifacts", "阶段成果"],
              ] as const
            ).map(([view, label]) => (
              <button
                key={view}
                onClick={() => onRightView(view)}
                aria-pressed={rightView === view}
                className={
                  "px-2 py-[3px] text-[11.5px] transition " +
                  (rightView === view
                    ? "bg-primary text-on-primary"
                    : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
                }
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        {rightView === "artifacts" ? (
          <div className="pt-3.5 flex-1 min-h-0 overflow-y-auto">{artifactsSlot}</div>
        ) : empty ? (
          <div className="pt-3.5 flex-1 flex flex-col items-start justify-center gap-1.5" data-project-gantt-empty="">
            <p className="text-[13px] text-on-surface-variant leading-relaxed">
              在左侧录入第一条项目事项后，这里会出现项目时间线。
            </p>
            <p className="text-[12px] text-outline leading-relaxed">
              写了开始日期画区间条，只写期限画单日里程碑；事项只存在本机。
            </p>
          </div>
        ) : noMatch ? (
          <p className="pt-3.5 text-[12.5px] text-outline leading-relaxed">
            搜索没有命中，时间线随左栏一起清空。
          </p>
        ) : (
          <div className="pt-3 flex-1 min-h-0 overflow-auto">
            <div className="min-w-[520px]">
              {/* 日期轴 */}
              <div className="flex items-end gap-2 pb-1.5 border-b border-surface-variant/30">
                <span className="w-[148px] shrink-0 text-[11px] uppercase tracking-[0.16em] text-outline">
                  事项 / 负责人
                </span>
                <div className="relative flex-1 h-4" data-gantt-axis="">
                  {ticks.map((t) => (
                    <span
                      key={t.date}
                      className="absolute top-0 -translate-x-1/2 mono text-[11px] text-outline whitespace-nowrap"
                      style={{ left: t.leftPct + "%" }}
                      data-hover-hint={t.date}
                    >
                      {t.label}
                    </span>
                  ))}
                </div>
              </div>

              {/* 行 */}
              <div className="relative">
                {nowPct !== null ? (
                  <span
                    aria-hidden="true"
                    data-gantt-today=""
                    className="absolute top-0 bottom-0 w-px bg-error/70 z-10"
                    style={{ left: "calc(156px + (100% - 156px) * " + nowPct / 100 + ")" }}
                    data-hover-hint={"今天 " + today}
                  />
                ) : null}
                {bars.map((bar, index) => (
                  <GanttRow
                    key={bar.item.item + index}
                    bar={bar}
                    active={selected === bar.item.item}
                    onSelect={() =>
                      setSelected(selected === bar.item.item ? null : bar.item.item)
                    }
                    registerRef={(el) => {
                      if (el) rowRefs.current.set(bar.item.item, el);
                      else rowRefs.current.delete(bar.item.item);
                    }}
                  />
                ))}
              </div>
            </div>
          </div>
        )}

        <div className="mt-3 pt-2.5 border-t border-surface-variant/20 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[11.5px] text-outline">
          {(Object.keys(STATUS_LABEL) as ProjectItemStatus[]).map((st) => (
            <span key={st} className="inline-flex items-center gap-1.5">
              <span aria-hidden="true" className={"w-2 h-2 " + STATUS_STYLE[st].dot} />
              {STATUS_LABEL[st]}
            </span>
          ))}
          <span className="inline-flex items-center gap-1.5">
            <span aria-hidden="true" className="w-2 h-2 rotate-45 bg-on-surface-variant/70" />
            单日里程碑
          </span>
          {nowPct !== null ? (
            <span className="inline-flex items-center gap-1.5">
              <span aria-hidden="true" className="w-px h-3 bg-error/70" />
              今天
            </span>
          ) : null}
        </div>
      </section>

      <ConfirmDialog
        open={pendingDelete !== null}
        title="删除这条项目事项？"
        description={
          pendingDelete
            ? "将从本机事项文件里删掉「" + pendingDelete.item + "」，时间线同步移除。"
            : ""
        }
        confirmLabel="删除"
        onConfirm={() => void confirmDelete()}
        onCancel={() => setPendingDelete(null)}
        testId="project-delete-dialog"
      />
    </div>
  );
}

/* ============================== 项目概览与素材 =============================== */

/** 素材审核状态的展示顺序：待审核置顶，其余按用户决策路径排列。 */
const REVIEW_ORDER: WritingMaterialReviewStatus[] = [
  "待审核",
  "已确认",
  "已退回",
  "已拒绝",
  "已归档",
];

const REVIEW_STYLE: Record<
  WritingMaterialReviewStatus,
  { chip: string; dot: string }
> = {
  待审核: { chip: "bg-amber-500/15 text-amber-700", dot: "bg-amber-500" },
  已确认: { chip: "bg-secondary/10 text-secondary", dot: "bg-secondary" },
  已退回: { chip: "bg-error/10 text-error", dot: "bg-error" },
  已拒绝: { chip: "bg-surface-container text-on-surface-variant", dot: "bg-outline" },
  已归档: { chip: "bg-surface-container text-outline", dot: "bg-outline/60" },
};

const PROJECT_STATUS_STYLE: Record<ProjectStatus, string> = {
  进行中: "bg-primary/10 text-primary",
  已完成: "bg-secondary/10 text-secondary",
  已搁置: "bg-surface-container text-on-surface-variant",
};

function projectDateText(value: string | null | undefined): string {
  if (!value) return "未设";
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  return m ? `${m[1]}.${m[2]}.${m[3]}` : value;
}

function readError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function knowledgeLabel(id: string): string {
  const parts = id.split(/[\\/]/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : id;
}

/** 项目概览：只读选择既有项目，计数与列表全部来自 508 反向派生。 */
function ProjectInsight() {
  const [projects, setProjects] = useState<ProjectInfo[] | null>(null);
  const [projectListError, setProjectListError] = useState<string | null>(null);
  const [projectListBroken, setProjectListBroken] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const [overview, setOverview] = useState<ProjectOverview | null>(null);
  const [overviewLoading, setOverviewLoading] = useState(false);
  const [overviewError, setOverviewError] = useState<string | null>(null);

  const [materials, setMaterials] = useState<WritingMaterial[]>([]);
  const [materialsLoading, setMaterialsLoading] = useState(false);
  const [materialsError, setMaterialsError] = useState<string | null>(null);
  const [materialsBroken, setMaterialsBroken] = useState(0);

  const [openMaterialId, setOpenMaterialId] = useState<string | null>(null);
  const [materialDetail, setMaterialDetail] = useState<MaterialDetail | null>(null);
  const [materialDetailError, setMaterialDetailError] = useState<string | null>(null);
  const [jumpError, setJumpError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    shell
      .projectList()
      .then((listing) => {
        if (!alive) return;
        setProjects(listing.items);
        setProjectListBroken(listing.broken);
        setSelectedId((previous) => {
          if (previous && listing.items.some((p) => p.project_id === previous)) {
            return previous;
          }
          return listing.items[0]?.project_id ?? null;
        });
      })
      .catch((error) => {
        if (!alive) return;
        setProjectListError(readError(error));
        setProjects(null);
      });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (!selectedId) {
      setOverview(null);
      setOverviewError(null);
      setOverviewLoading(false);
      return;
    }
    let alive = true;
    const id = selectedId;
    setOverviewLoading(true);
    setOverviewError(null);
    setOverview(null);
    shell
      .projectOverview(id)
      .then((value) => {
        if (alive) setOverview(value);
      })
      .catch((error) => {
        if (alive) setOverviewError(readError(error));
      })
      .finally(() => {
        if (alive) setOverviewLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selectedId]);

  useEffect(() => {
    if (!selectedId) {
      setMaterials([]);
      setMaterialsError(null);
      setMaterialsLoading(false);
      setMaterialsBroken(0);
      return;
    }
    let alive = true;
    setMaterialsLoading(true);
    setMaterialsError(null);
    setMaterials([]);
    shell
      .materialList({ project_id: selectedId })
      .then((listing) => {
        if (!alive) return;
        setMaterials(listing.items);
        setMaterialsBroken(listing.broken);
      })
      .catch((error) => {
        if (!alive) return;
        setMaterialsError(readError(error));
        setMaterialsBroken(0);
      })
      .finally(() => {
        if (alive) setMaterialsLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selectedId]);

  useEffect(() => {
    if (!openMaterialId) {
      setMaterialDetail(null);
      setMaterialDetailError(null);
      return;
    }
    let alive = true;
    const id = openMaterialId;
    setMaterialDetail(null);
    setMaterialDetailError(null);
    setJumpError(null);
    shell
      .materialGet(id)
      .then((detail) => {
        if (alive) setMaterialDetail(detail);
      })
      .catch((error) => {
        if (alive) setMaterialDetailError(readError(error));
      });
    return () => {
      alive = false;
    };
  }, [openMaterialId]);

  const selected = projects?.find((project) => project.project_id === selectedId) ?? null;
  const shownProject = overview?.project ?? selected;
  const associations = overview
    ? overview.tasks.length + overview.records.length + overview.knowledge.length
    : 0;

  const materialGroups = useMemo(() => {
    return REVIEW_ORDER.map((status) => ({
      status,
      items: materials.filter((material) => material.review_status === status),
    })).filter((group) => group.items.length > 0);
  }, [materials]);

  function selectProject(id: string | null) {
    setSelectedId(id);
    setOpenMaterialId(null);
    setMaterialDetail(null);
    setMaterialDetailError(null);
    setJumpError(null);
  }

  function jumpToProject(projectId: string) {
    const exists = projects?.some((project) => project.project_id === projectId) ?? false;
    if (!exists) {
      setJumpError("该项目不在当前项目列表中，无法跳转。");
      return;
    }
    selectProject(projectId);
  }

  return (
    <>
      <section
        aria-label="项目概览"
        data-project-overview=""
        className="lg:col-span-2 bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
      >
        <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
          <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
            项目概览
          </h2>
          {projects ? (
            <span className="mono text-[12px] text-outline ml-auto shrink-0">
              {projects.length} 个项目
            </span>
          ) : null}
        </div>

        {projectListError ? (
          <p
            className="pt-3 text-[12px] text-error leading-relaxed"
            data-project-overview-error="list"
          >
            读取项目列表失败：{projectListError}。这里不把它当成「没有项目」。
          </p>
        ) : projects === null ? (
          <p className="pt-3 text-[12px] text-outline leading-relaxed">正在读取项目…</p>
        ) : projects.length === 0 ? (
          <div className="pt-3 flex flex-col gap-1.5" data-project-overview-empty="">
            <p className="text-[13px] text-on-surface-variant leading-relaxed">
              还没有项目。
            </p>
            <p className="text-[12px] text-outline leading-relaxed">
              创建项目并关联任务、记录或知识后，这里会按项目归集成果。
            </p>
          </div>
        ) : (
          <div className="pt-3 grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,1.95fr)]">
            <div className="min-w-0">
              <label className="block">
                <span className="block text-[11.5px] text-outline">项目</span>
                <select
                  value={selectedId ?? ""}
                  onChange={(event) => selectProject(event.target.value || null)}
                  aria-label="选择项目"
                  data-project-selector=""
                  className={FIELD + " mt-1"}
                >
                  {projects.map((project) => (
                    <option key={project.project_id} value={project.project_id}>
                      {project.name + " · " + project.status}
                    </option>
                  ))}
                </select>
              </label>

              {shownProject ? (
                <div className="mt-3" data-project-summary="">
                  <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
                    <span
                      className={
                        "shrink-0 px-1.5 py-[1px] text-[11.5px] " +
                        PROJECT_STATUS_STYLE[shownProject.status]
                      }
                      data-project-status={shownProject.status}
                    >
                      {shownProject.status}
                    </span>
                    <span
                      className="mono text-[12px] text-outline"
                      data-project-dates=""
                    >
                      {projectDateText(shownProject.start)} → {projectDateText(shownProject.end)}
                    </span>
                  </div>
                  <p
                    className="mt-2 text-[12.5px] text-on-surface-variant leading-[1.65] break-words"
                    data-project-goal=""
                  >
                    {shownProject.goal || "未填写目标"}
                  </p>
                  <p className="mt-1.5 mono text-[11px] text-outline/80 break-all">
                    {shownProject.project_id}
                  </p>
                </div>
              ) : null}
            </div>

            <div className="min-w-0">
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2" data-project-counts="">
                <ProjectCount label="任务" kind="tasks" value={overview?.task_count} />
                <ProjectCount label="记录" kind="records" value={overview?.record_count} />
                <ProjectCount label="素材" kind="materials" value={overview?.material_count} />
                <ProjectCount label="知识" kind="knowledge" value={overview?.knowledge_count} />
              </div>

              {overviewLoading ? (
                <p className="mt-3 text-[12px] text-outline leading-relaxed">正在读取关联…</p>
              ) : overviewError ? (
                <p
                  className="mt-3 text-[12px] text-error leading-relaxed"
                  data-project-overview-error="detail"
                >
                  读取项目概览失败：{overviewError}。这里不把它当成「没有关联」。
                </p>
              ) : overview ? (
                associations === 0 ? (
                  <p className="mt-3 text-[12px] text-outline leading-relaxed" data-project-refs-empty="">
                    这个项目还没有关联任务、记录或知识。
                  </p>
                ) : (
                  <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-3">
                    <ProjectRefGroup title="任务" items={overview.tasks} />
                    <ProjectRefGroup title="记录" items={overview.records} />
                    <ProjectKnowledgeGroup knowledge={overview.knowledge} />
                  </div>
                )
              ) : null}

              {overview &&
              overview.broken_tasks + overview.broken_records + overview.broken_materials > 0 ? (
                <p className="mt-2 text-[11.5px] text-outline leading-relaxed">
                  另有 {overview.broken_tasks} 个任务、{overview.broken_records} 条记录、
                  {overview.broken_materials} 条素材读取失败，未计入统计。
                </p>
              ) : null}
            </div>
          </div>
        )}

        {projectListBroken > 0 ? (
          <p className="mt-2 text-[11.5px] text-outline leading-relaxed">
            另有 {projectListBroken} 个项目文件读取失败，未进入选择器。
          </p>
        ) : null}
      </section>

      {selectedId ? (
        <section
          aria-label="项目素材"
          data-project-materials=""
          className="lg:col-span-2 bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
        >
          <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
            <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
              素材
            </h2>
            <span className="mono text-[12px] text-outline ml-auto shrink-0">
              {overview ? overview.material_count : materialsLoading ? "—" : materials.length} 条
            </span>
          </div>

          {materialsError ? (
            <p className="pt-3 text-[12px] text-error leading-relaxed" data-project-materials-error="">
              读取素材失败：{materialsError}。这里不把它当成「没有素材」。
            </p>
          ) : materialsLoading ? (
            <p className="pt-3 text-[12px] text-outline leading-relaxed">正在读取素材…</p>
          ) : materials.length === 0 ? (
            <p className="pt-3 text-[12px] text-outline leading-relaxed" data-project-materials-empty="">
              这个项目没有可展示的素材。记录分析或手动收藏的素材会归集到这里。
            </p>
          ) : (
            <div
              className={
                "pt-3 grid grid-cols-1 gap-4 " +
                (openMaterialId ? "lg:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]" : "")
              }
            >
              <div className="min-w-0 flex flex-col gap-3 max-h-[360px] overflow-y-auto pr-1">
                {materialGroups.map((group) => {
                  const style = REVIEW_STYLE[group.status];
                  return (
                    <div key={group.status} data-project-material-group={group.status}>
                      <div className="flex items-center gap-2 pb-1.5 border-b border-surface-variant/25">
                        <span aria-hidden="true" className={"w-2 h-2 " + style.dot} />
                        <span className="text-[12px] font-semibold text-on-surface">
                          {group.status}
                        </span>
                        <span className="mono text-[11.5px] text-outline">{group.items.length}</span>
                      </div>
                      <div className="mt-1 flex flex-col">
                        {group.items.map((material) => {
                          const active = openMaterialId === material.material_id;
                          return (
                            <button
                              key={material.material_id}
                              onClick={() =>
                                setOpenMaterialId(active ? null : material.material_id)
                              }
                              aria-pressed={active}
                              data-project-material-item={material.review_status}
                              className={
                                "min-w-0 text-left px-2 py-1.5 border-b border-surface-variant/15 last:border-b-0 transition " +
                                (active
                                  ? "bg-primary/5 ring-1 ring-primary/40"
                                  : "hover:bg-surface-container-low/60")
                              }
                            >
                              <span className="flex items-baseline gap-2 min-w-0">
                                <span className="min-w-0 flex-1 text-[12.5px] text-on-surface leading-[1.5] truncate">
                                  {material.title}
                                </span>
                                <span className="shrink-0 mono text-[11px] text-outline">
                                  {material.material_type}
                                </span>
                                {material.event_date ? (
                                  <span className="shrink-0 mono text-[11px] text-outline">
                                    {projectDateText(material.event_date)}
                                  </span>
                                ) : null}
                              </span>
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </div>

              {openMaterialId ? (
                <div
                  className="min-w-0 border border-outline-variant/50 bg-surface-container-low/45 px-3 py-3"
                  data-project-material-detail=""
                >
                  {materialDetailError ? (
                    <p className="text-[12px] text-error leading-relaxed">
                      读取素材详情失败：{materialDetailError}
                    </p>
                  ) : !materialDetail ? (
                    <p className="text-[12px] text-outline leading-relaxed">
                      正在读取素材详情…
                    </p>
                  ) : (
                    <MaterialDetailView
                      detail={materialDetail}
                      jumpError={jumpError}
                      onJumpProject={jumpToProject}
                      onClose={() => setOpenMaterialId(null)}
                    />
                  )}
                </div>
              ) : null}
            </div>
          )}

          {materialsBroken > 0 ? (
            <p className="mt-2 text-[11.5px] text-outline leading-relaxed">
              另有 {materialsBroken} 条素材读取失败，未进入列表。
            </p>
          ) : null}
        </section>
      ) : null}
    </>
  );
}

function ProjectCount({
  label,
  kind,
  value,
}: {
  label: string;
  kind: string;
  value: number | undefined;
}) {
  return (
    <div
      className="border border-outline-variant/40 bg-surface-container-low/45 px-2.5 py-1.5"
      data-project-count={kind}
    >
      <span className="block mono text-[18px] leading-none text-on-surface">
        {value === undefined ? "—" : value}
      </span>
      <span className="block mt-1 text-[11.5px] text-outline">{label}</span>
    </div>
  );
}

function ProjectRefGroup({ title, items }: { title: string; items: ProjectRefItem[] }) {
  return (
    <div className="min-w-0" data-project-ref-group={title}>
      <div className="flex items-baseline gap-2 pb-1 border-b border-surface-variant/25">
        <span className="text-[11.5px] text-outline">{title}</span>
        <span className="mono text-[11.5px] text-outline ml-auto">{items.length}</span>
      </div>
      {items.length > 0 ? (
        <div className="mt-1 max-h-[108px] overflow-y-auto">
          {items.map((item) => (
            <p
              key={item.id}
              className="text-[12px] text-on-surface-variant leading-[1.5] truncate"
              data-hover-hint={item.id}
            >
              {item.title}
            </p>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function ProjectKnowledgeGroup({ knowledge }: { knowledge: ProjectKnowledgeRef[] }) {
  return (
    <div className="min-w-0" data-project-ref-group="知识">
      <div className="flex items-baseline gap-2 pb-1 border-b border-surface-variant/25">
        <span className="text-[11.5px] text-outline">知识</span>
        <span className="mono text-[11.5px] text-outline ml-auto">{knowledge.length}</span>
      </div>
      {knowledge.length > 0 ? (
        <div className="mt-1 max-h-[108px] overflow-y-auto">
          {knowledge.map((entry) => (
            <p
              key={entry.knowledge_id}
              className="flex items-baseline gap-2 text-[12px] text-on-surface-variant leading-[1.5]"
              data-hover-hint={entry.knowledge_id}
            >
              <span className="min-w-0 flex-1 truncate mono">{knowledgeLabel(entry.knowledge_id)}</span>
              <span className="shrink-0 mono text-[11px] text-outline">
                {entry.references.length} 引用
              </span>
            </p>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function MaterialDetailView({
  detail,
  jumpError,
  onJumpProject,
  onClose,
}: {
  detail: MaterialDetail;
  jumpError: string | null;
  onJumpProject: (projectId: string) => void;
  onClose: () => void;
}) {
  const material = detail.material;
  const review = REVIEW_STYLE[material.review_status];
  const project = detail.related_project;
  return (
    <div className="min-w-0">
      <div className="flex items-start gap-2">
        <p className="min-w-0 flex-1 text-[13px] font-semibold text-on-surface leading-[1.5] break-words">
          {material.title}
        </p>
        <button
          onClick={onClose}
          className="shrink-0 text-[11.5px] text-outline hover:text-on-surface"
          aria-label="关闭素材详情"
        >
          关闭
        </button>
      </div>
      <div className="mt-1.5 flex flex-wrap items-center gap-2 text-[11.5px]">
        <span className={"px-1.5 py-[1px] " + review.chip}>{material.review_status}</span>
        <span className="text-outline">{material.material_type}</span>
        {material.event_date ? (
          <span className="mono text-outline">{projectDateText(material.event_date)}</span>
        ) : null}
      </div>
      <p className="mt-2 text-[12px] text-on-surface leading-[1.7] whitespace-pre-wrap break-words max-h-[180px] overflow-y-auto">
        {material.content}
      </p>
      {detail.source.quote ? (
        <div className="mt-2 pl-2 border-l-2 border-outline-variant/60">
          <p className="text-[11.5px] text-outline">来源引用</p>
          <p className="mt-0.5 text-[12px] text-on-surface-variant leading-[1.6] whitespace-pre-wrap break-words max-h-[108px] overflow-y-auto">
            {detail.source.quote}
          </p>
        </div>
      ) : null}
      {detail.source.entity.title ? (
        <p className="mt-2 text-[11.5px] text-outline leading-relaxed break-words">
          来源 · {detail.source.entity.title}
          {detail.source.location ? " · " + detail.source.location : ""}
        </p>
      ) : null}
      {project ? (
        project.found && project.id ? (
          <div className="mt-3">
            <button
              onClick={() => onJumpProject(project.id)}
              data-material-project-jump={project.id}
              className="border border-primary/40 px-2 py-[3px] text-[11.5px] text-primary hover:bg-primary/5"
            >
              查看项目 · {project.title ?? project.id}
            </button>
            {jumpError ? (
              <p className="mt-1 text-[11.5px] text-error leading-relaxed">{jumpError}</p>
            ) : null}
          </div>
        ) : (
          <p className="mt-2 text-[11.5px] text-outline leading-relaxed">
            {project.note ?? "所属项目文件未找到。"}
          </p>
        )
      ) : null}
    </div>
  );
}

/* ================================ 表单辅助 ================================= */

/** 输入框共用边框：与页面其它卡片同一套描边，不用浏览器默认样式。 */
const FIELD =
  "mt-0.5 block w-full border border-outline-variant/60 bg-white px-2 py-1 text-[12px] text-on-surface outline-none focus:border-primary";

interface Draft {
  item: string;
  owner: string;
  due: string;
  start: string;
  status: ProjectItemStatus;
  next_step: string;
}

const EMPTY_DRAFT: Draft = {
  item: "",
  owner: "",
  due: "",
  start: "",
  status: "not_started",
  next_step: "",
};

function draftFrom(record: ProjectItemRecord): Draft {
  return {
    item: record.item,
    owner: record.owner,
    due: record.due,
    start: record.start ?? "",
    status: record.status,
    next_step: record.next_step,
  };
}

/** 日期必须是真实存在的 ISO 日期（与服务端同一口径，客户端先给一句人话）。 */
function validDate(value: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!m) return false;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const d = Number(m[3]);
  const dt = new Date(Date.UTC(y, mo - 1, d));
  return (
    dt.getUTCFullYear() === y && dt.getUTCMonth() === mo - 1 && dt.getUTCDate() === d
  );
}

/** 服务端仍会整批校验；这里先拦住最常见的空值、代号与日期顺序。 */
function validateDraft(
  draft: Draft,
  records: ProjectItemRecord[],
  editingKey: string | null,
): string | null {
  if (!draft.item) return "事项不能为空。";
  if (draft.item.length > 80) return "事项最多 80 个字符。";
  const duplicate = records.some((r) => r.item === draft.item && r.item !== editingKey);
  if (duplicate) return "已经有同名事项，换一个名称再保存。";
  if (!/^P\d+$/.test(draft.owner)) return "负责人代号用 P 加数字，例如 P1。";
  if (!validDate(draft.due)) return "期限要填真实存在的日期（YYYY-MM-DD）。";
  if (draft.start && !validDate(draft.start)) return "开始日期要填真实存在的日期（YYYY-MM-DD）。";
  if (draft.start && draft.start > draft.due) return "开始日期不能晚于期限。";
  if (!draft.next_step) return "下一步不能为空。";
  return null;
}

function GanttRow({
  bar,
  active,
  onSelect,
  registerRef,
}: {
  bar: GanttBar;
  active: boolean;
  onSelect: () => void;
  registerRef: (el: HTMLDivElement | null) => void;
}) {
  const item = bar.item;
  const st = statusOf(item.status);
  const style = STATUS_STYLE[st];
  const label = `${item.item} · ${STATUS_LABEL[st]} · 期限 ${item.due}${
    item.overdue ? " · 已逾期" : ""
  }`;
  return (
    <div
      ref={registerRef}
      data-gantt-row={bar.kind}
      className={
        "group flex items-center gap-2 py-[7px] border-b border-surface-variant/15 last:border-b-0 transition " +
        (active ? "bg-primary/5" : "hover:bg-surface-container-low/60")
      }
    >
      <button
        className="w-[148px] shrink-0 text-left min-w-0 flex items-center gap-1.5"
        onClick={onSelect}
        aria-pressed={active}
        aria-label={label}
        data-hover-hint={`${label}\n下一步：${item.next_step}`}
      >
        <span aria-hidden="true" className={"w-1.5 h-1.5 shrink-0 " + style.dot} />
        <span className="min-w-0">
          <span className="block text-[12.5px] text-on-surface leading-[1.45] truncate">
            {item.item}
          </span>
          <span className="block mono text-[11px] text-outline leading-[1.4]">
            {item.owner}
          </span>
        </span>
      </button>
      <div className="relative flex-1 h-6 min-w-0">
        {bar.kind === "range" ? (
          <span
            className={
              "absolute top-1/2 -translate-y-1/2 h-[14px] " +
              style.bar +
              (item.overdue ? " ring-1 ring-error/60" : "")
            }
            style={{ left: `${bar.leftPct}%`, width: `${bar.widthPct}%` }}
            data-hover-hint={`${item.start ?? ""} → ${item.due}`}
          />
        ) : (
          <span
            className={
              "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2.5 h-2.5 rotate-45 " +
              style.bar +
              (item.overdue ? " ring-1 ring-error/70" : "")
            }
            style={{ left: `${bar.leftPct}%` }}
            data-hover-hint={`里程碑 ${item.due}`}
          />
        )}
        <span
          className="absolute top-1/2 -translate-y-1/2 mono text-[11px] text-outline whitespace-nowrap"
          style={{ left: `calc(${bar.duePct}% + 10px)` }}
        >
          {item.due.slice(5)}
        </span>
      </div>
      {item.overdue ? (
        <span className="shrink-0 text-[11px] text-error whitespace-nowrap">逾期</span>
      ) : null}
    </div>
  );
}
