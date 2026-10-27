/**
 * 项目跟进页（OD-ARCH-098 S03）：左「项目卡列表」+ 右「甘特时间线」。
 *
 * 参考图是任务跟踪看板（左列表、右按行对齐的甘特）。这里只搬版式骨架：
 * 左栏承载「这件事是谁的、期限、下一步」，右栏承载「它落在时间轴的哪儿」。
 *
 * 诚实性：
 *   * 数据全部来自宿主的确定性汇总 `task_project_summary`，页面不调模型；
 *   * 事项行没有进度字段，因此界面**不显示任何百分比**，只画状态色带；
 *   * 没有开始日期的事项画成单日菱形里程碑，不倒推一段虚构工期；
 *   * 页面只读：事项在首页对话里写，这里不做编辑、拖拽与改期。
 */

import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Icon } from "../components/Icon";
import type { ProjectItem, ProjectSummary } from "../types";
import {
  filterItems,
  ganttTicks,
  ganttWindow,
  layoutItems,
  todayPct,
  toDayNumber,
  type GanttBar,
} from "../projectGantt";

const STATUS_LABEL: Record<ProjectItem["status"], string> = {
  not_started: "未开始",
  in_progress: "进行中",
  blocked: "受阻",
  done: "已完成",
};

/** 状态 → 视觉：徽标、色带、甘特条。全部走既有语义色，不引入参考图的珊瑚色。 */
const STATUS_STYLE: Record<
  ProjectItem["status"],
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

function statusOf(value: string): ProjectItem["status"] {
  return value in STATUS_STYLE ? (value as ProjectItem["status"]) : "not_started";
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
  summary,
  today,
  error,
  loading,
  subtitleProjects,
  subtitleTimeline,
  rightView,
  onRightView,
  artifactsSlot,
}: {
  summary: ProjectSummary | null;
  today: string;
  error: string | null;
  loading: boolean;
  subtitleProjects?: string;
  subtitleTimeline?: string;
  /** 右栏次级视图：时间线 / 阶段成果（OD-ARCH-098 R5，保留既有成果入口）。 */
  rightView: "timeline" | "artifacts";
  onRightView: (view: "timeline" | "artifacts") => void;
  /** 阶段成果内容由调用方注入，避免这里再引一次成果目录命令。 */
  artifactsSlot?: ReactNode;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());

  const items = summary?.items ?? [];
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

  return (
    <div
      className={
        "grid grid-cols-1 gap-4 lg:gap-5 items-start max-w-7xl mx-auto md:-translate-x-8 " +
        "lg:grid-cols-[340px_minmax(0,1fr)]"
      }
      data-project-board=""
      data-board-width="nav"
    >
      {/* ------------------------------ 左栏：项目卡列表 ------------------------------ */}
      <section
        aria-label="项目事项"
        className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0 flex flex-col lg:max-h-[70vh] max-lg:order-1"
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
            {query.trim() ? `${visible.length}/${items.length}` : items.length} 条
          </span>
        </div>

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

        <div className="mt-3 flex-1 min-h-0 overflow-y-auto flex flex-col gap-2.5">
          {error || summary === null ? (
            <p className="text-[12px] text-error leading-relaxed">
              {loading
                ? "正在汇总项目事项…"
                : error
                  ? "确定性汇总没有产出：" + error + "。这里不把它当成「没有事项」。"
                  : "还没有可汇总的事项。在首页对话里按「事项 | 负责人代号 | 期限 | 状态 | 下一步」写，这里按你写的内容汇总——确定性计算，不调模型。"}
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
              return (
                <button
                  key={item.item}
                  onClick={() => setSelected(active ? null : item.item)}
                  aria-pressed={active}
                  aria-label={`${item.item} · ${STATUS_LABEL[st]} · 负责人 ${item.owner} · 期限 ${item.due}${item.overdue ? " · 已逾期" : ""}`}
                  data-project-card={st}
                  className={
                    "group relative text-left bg-surface-container-low/55 hover:bg-surface-container-low px-3.5 py-3 min-w-0 transition " +
                    (active ? "ring-1 ring-primary/60 bg-surface-container-low" : "")
                  }
                >
                  {/* 状态色带：只表达状态，不是进度百分比。 */}
                  <span aria-hidden="true" className={"absolute left-0 top-0 bottom-0 w-[3px] " + style.rail} />
                  <span className="flex items-start gap-2 min-w-0">
                    <span className="min-w-0 flex-1 text-[13.5px] font-medium text-on-surface leading-[1.5] line-clamp-2 break-words">
                      {item.item}
                    </span>
                    <span
                      className={"shrink-0 px-1.5 py-[1px] text-[11.5px] " + style.chip}
                    >
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
                  <span className="mt-1 block text-[12px] text-on-surface-variant leading-[1.6] line-clamp-1" title={item.next_step}>
                    下一步 · {item.next_step}
                  </span>
                </button>
              );
            })
          )}
        </div>
      </section>

      {/* ------------------------------ 右栏：甘特时间线 ------------------------------ */}
      <section
        aria-label="项目时间线"
        className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0 flex flex-col lg:max-h-[70vh] max-lg:order-2"
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
              ? `${win.start.slice(5).replace("-", ".")} – ${win.end.slice(5).replace("-", ".")}`
              : `${items.length} 项`}
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
          <p className="pt-3.5 text-[12.5px] text-outline leading-relaxed">
            还没有可画进时间轴的事项。左栏的条目会按期限落在这里：写了开始日期画区间条，只写期限画单日里程碑。
          </p>
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
                      style={{ left: `${t.leftPct}%` }}
                      title={t.date}
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
                    style={{ left: `calc(156px + (100% - 156px) * ${nowPct / 100})` }}
                    title={`今天 ${today}`}
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
          {(Object.keys(STATUS_LABEL) as ProjectItem["status"][]).map((st) => (
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
    </div>
  );
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
        title={`${label}\n下一步：${item.next_step}`}
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
            title={`${item.start ?? ""} → ${item.due}`}
          />
        ) : (
          <span
            className={
              "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2.5 h-2.5 rotate-45 " +
              style.bar +
              (item.overdue ? " ring-1 ring-error/70" : "")
            }
            style={{ left: `${bar.leftPct}%` }}
            title={`里程碑 ${item.due}`}
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
