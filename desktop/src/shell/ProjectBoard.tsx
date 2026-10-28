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
import type { ProjectItem, ProjectItemRecord, ProjectItemStatus } from "../types";
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
