/**
 * 本周记录时间线（OD-ARCH-091 W1；OD-ARCH-092 字号 +2 并承接周报动作行）。
 *
 * 设计借鉴（screenshots-design-inspiration，2026-10-26）：Teamweek / Timepage /
 * Navigator 的日历时间线——按日分组、一条细连接轨、日期标题压弱，
 * 让「这一周记了什么」一眼可读，而不用一排碎片胶囊去猜。
 *
 * 诚实性：
 *   * 内容只来自宿主的周文件原文，解析不了的行原样显示（见 `weekTimeline.ts`）；
 *   * 没记录的日子照样占一行灰点，不把「今天没记」显示成「这一周是空的」；
 *   * 这里的按钮只是跳转，不写任何文件。
 *
 * 字号（OD-ARCH-092）：条目 13.5px、日期 12px、星期 11.5px、去向小标 11px。原先
 * 11.5/10 的一档在这台机器的 1280 宽窗口里偏小，用户要求整体加两号。
 */

import { useMemo, useState, type ReactNode } from "react";
import {
  buildTimeline,
  isEmptyTimeline,
  localToday,
  parseWorklog,
  shortDate,
  weekdayLabel,
} from "../weekTimeline";
import { REPORT_DESTINATION } from "../weeklyQuadrants";


/**
 * 一条记录的去向开关（OD-ARCH-099 S03）。
 *
 * 圆形点选、primary 紫，与时间线的圆点节奏一致；可见文案只在悬停提示里，
 * 不靠颜色单独表达（`aria-label` + `title` 保留完整语义）。键盘 Space/Enter 可切换，
 * 失败时显示宿主返回的原因。
 */
function DestinationSwitch({
  index,
  destination,
  text,
  disabled,
  onToggle,
}: {
  index: number;
  destination: string | null;
  text: string;
  disabled: boolean;
  onToggle: (next: boolean) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  // 没有去向行：不给可点控件，避免为它凭空插入一行改变既有结构。
  if (!destination) {
    return (
      <span
        className="shrink-0 mt-[3px] px-1.5 py-[2px] text-[11.5px] text-outline/70 border border-dashed border-outline-variant/60"
        title="这条记录在文件里没有「去向」行，可在原文补一行之后再来点选"
      >
        无去向
      </span>
    );
  }
  const checked = destination.trim() === REPORT_DESTINATION;
  return (
    <div className="shrink-0 flex flex-col items-start gap-1" data-destination-switch="">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={(checked ? "已进周报：" : "不进周报：") + text.slice(0, 20)}
        data-entry-index={index}
        disabled={busy || disabled}
        onClick={async () => {
          setFailure(null);
          setBusy(true);
          try {
            await onToggle(!checked);
          } catch (e) {
            setFailure(e instanceof Error ? e.message : String(e));
          } finally {
            setBusy(false);
          }
        }}
        title={
          checked
            ? "已进周报：计入「进周报条目」，整理周报时作为上下文。点一下改为不进。"
            : "不进周报：仍留在本周记录里。点一下改为进周报。"
        }
        className={
          "inline-flex items-center justify-center w-6 h-6 rounded-full border transition-colors duration-150 disabled:opacity-60 " +
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-1 " +
          (checked
            ? "border-primary bg-primary text-white hover:bg-primary/90"
            : "border-outline-variant/70 bg-white text-on-surface-variant hover:border-primary/50 hover:text-primary")
        }
      >
        <span
          aria-hidden
          className={
            "w-full h-full rounded-full flex items-center justify-center " +
            (checked ? "bg-primary" : "bg-transparent")
          }
        >
          {checked ? (
            <svg viewBox="0 0 12 12" className="w-[10px] h-[10px]" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M2.2 6.3 4.6 8.6 9.8 3.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          ) : null}
        </span>
      </button>
      {!checked && destination.trim() !== "仅记录" ? (
        <span className="text-[10.5px] text-outline/80 max-w-[80px] truncate" title={destination}>
          {destination}
        </span>
      ) : null}
      {failure ? (
        <span className="text-[10.5px] text-error max-w-[160px] leading-snug">{failure}</span>
      ) : null}
    </div>
  );
}


export function WeeklyTimeline({
  week,
  content,
  error,
  loading,
  onOpenSource,
  onRequestReport,
  statusRow,
  subtitle,
  visibleIds,
  filterLabel,
  onClearFilter,
  onToggleDestination,
}: {
  week: string;
  content: string;
  error: string | null;
  loading: boolean;
  onOpenSource: () => void;
  onRequestReport: () => void;
  /** 卡片头的动作行（OD-ARCH-092：周报状态并入这里，不再单独占一块）。 */
  statusRow?: ReactNode;
  /** 可选英文副标题（OD-ARCH-093，场景配置，缺省不渲染）。 */
  subtitle?: string;
  /** 四象限筛选：只显示这些条目 id；null 表示不筛选（OD-ARCH-093 S03）。 */
  visibleIds?: Set<string> | null;
  /** 当前筛选的中文说明，仅在筛选时显示。 */
  filterLabel?: string | null;
  onClearFilter?: () => void;
  /**
   * 点选「进周报」开关（OD-ARCH-099 S03）。
   *
   * 只改这条记录在周文件里的去向字段：选中写 本周报引用，取消写 仅记录。
   * 成功后由上层重读当周文件，统计卡与四象限随之更新。失败时抛出可读原因，
   * 这里只显示它，**不做**本地乐观改写，所以勾选状态天然停在服务端真实值。
   */
  onToggleDestination?: (args: {
    date: string;
    index: number;
    entry: { text: string; destination: string | null };
    next: boolean;
  }) => Promise<void>;
}) {
  const parsed = useMemo(() => parseWorklog(content), [content]);
  const rows = useMemo(() => buildTimeline(parsed, week), [parsed, week]);
  const empty = useMemo(() => isEmptyTimeline(parsed.days), [parsed]);
  const today = localToday();
  const filtered = visibleIds != null;
  /** 筛选后仍然显示的空日期行：不在筛选结果里就不占位置，避免大半屏空点。 */
  const shownRows = useMemo(() => {
    if (!filtered) {
      return rows.map((day) => ({ ...day, entryIndexes: null as number[] | null }));
    }
    return rows
      .map((day) => {
        // 保留原始下标：去向开关要按「这条是该日段内第几条」定位，筛选后不能重编号。
        const keep: number[] = [];
        day.entries.forEach((_, i) => {
          if (visibleIds!.has(`${day.date}#${i}`)) keep.push(i);
        });
        return {
          ...day,
          entries: keep.map((i) => day.entries[i]),
          entryIndexes: keep,
          raw: [],
        };
      })
      .filter((day) => day.entries.length > 0);
  }, [rows, filtered, visibleIds]);
  const filteredCount = shownRows.reduce((n, d) => n + d.entries.length, 0);

  // 桌面三栏时中栏独立滚动；窄窗单列时不限制高度，避免页面里再套一层滚动。
  return (
    <section
      className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0 flex flex-col min-h-0 max-h-[70vh] max-lg:max-h-none"
      data-weekly-timeline=""
      aria-label={
        "本周记录 " + week + " · " +
        (empty ? "暂无记录" : filtered ? `${filteredCount}/${parsed.entryCount} 条` : `${parsed.entryCount} 条`)
      }
    >
      <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
        <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
          本周记录
        </h2>
        {subtitle ? (
          <span className="text-[12px] uppercase tracking-[0.16em] text-outline">{subtitle}</span>
        ) : null}
        <span className="mono text-[12px] text-outline">
          {week}
          {empty ? "" : filtered ? ` · ${filteredCount}/${parsed.entryCount} 条` : ` · ${parsed.entryCount} 条`}
        </span>
        <button
          className="ml-auto text-[12px] text-outline hover:text-primary hover:underline shrink-0"
          onClick={onOpenSource}
          disabled={empty && !content}
          title="在工作记录文件里看原文"
        >
          打开本周原文
        </button>
      </div>

      {/* 卡片体自己滚动（OD-ARCH-093 S03）：三栏各自滚动，最长的中栏不再把整页推高。 */}
      <div className="flex-1 min-h-0 overflow-y-auto pt-3.5">
      {statusRow ? (
        <div className="pb-3.5 mb-3.5 border-b border-surface-variant/20">{statusRow}</div>
      ) : null}

      {filtered && filterLabel ? (
        <div className="pb-3 -mt-1 flex items-center gap-2">
          <span className="px-2 py-0.5 rounded-full text-[12px] bg-primary-fixed text-on-primary-fixed-variant">
            只看 {filterLabel}
          </span>
          {onClearFilter ? (
            <button className="text-[12px] text-outline hover:text-primary" onClick={onClearFilter}>
              取消筛选
            </button>
          ) : null}
        </div>
      ) : null}

      {error ? (
        <p className="text-[12px] text-error leading-relaxed pb-1">
          读取本周记录失败：{error}。这里不把它当成「本周没有记录」。
        </p>
      ) : null}

      {loading && !content ? (
        <p className="text-[12px] text-outline leading-relaxed">正在读取本周记录…</p>
      ) : empty && !error ? (
        <div className="pb-1">
          <p className="text-[13px] text-on-surface-variant leading-relaxed">
            本周还没有记录。在首页对话框直接写下做了什么，会自动落到{week}这一周。
          </p>
          <button
            className="mt-2 px-2.5 py-1 text-[12px] bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
            onClick={onRequestReport}
          >
            去写一条记录
          </button>
        </div>
      ) : filtered && filteredCount === 0 ? (
        <p className="text-[12px] text-outline leading-relaxed pb-1">
          这一格本周没有记录。点同一格或「取消筛选」回到全部。
        </p>
      ) : (
        <ol className="relative pl-[58px]">
          {/* 连接轨：一条细线，贯穿整周；比逐条卡片的边线更安静。 */}
          <span
            aria-hidden
            className="absolute left-[51px] top-2 bottom-2 w-px bg-outline-variant/45"
          />
          {shownRows.map((day) => {
            const hasContent = day.entries.length > 0 || day.raw.length > 0;
            const isToday = day.date === today;
            return (
              <li key={day.date + (day.continued ? "#c" : "")} className="relative pb-4 last:pb-0">
                <span
                  aria-hidden
                  className={
                    "absolute -left-[58px] top-[5px] ml-[46px] h-[9px] w-[9px] " +
                    (isToday
                      ? "bg-primary ring-2 ring-primary/20"
                      : hasContent
                        ? "bg-secondary"
                        : "border border-outline-variant/70 bg-white")
                  }
                />
                <div className="flex items-baseline gap-2">
                  <span
                    className={
                      "mono text-[12px] tracking-wide " +
                      (isToday ? "text-primary" : "text-outline")
                    }
                  >
                    {shortDate(day.date)}
                  </span>
                  <span className={"text-[12px] " + (isToday ? "text-primary" : "text-outline")}>
                    {weekdayLabel(day.date)}
                    {isToday ? " · 今天" : ""}
                    {day.outOfWeek ? " · 不在本周" : ""}
                  </span>
                </div>

                {hasContent ? (
                  <ul className="mt-2 flex flex-col gap-2.5">
                    {day.entries.map((entry, i) => {
                      // 筛选时保留原始下标：去向开关按「该日段内第几条」定位。
                      const sourceIndex = day.entryIndexes ? day.entryIndexes[i] : i;
                      return (
                      <li
                        key={sourceIndex}
                        data-weekly-entry=""
                        className={
                          "group flex items-start gap-2.5 rounded-2xl px-3.5 py-2.5 border-l-2 " +
                          (isToday
                            ? "border-primary bg-primary-fixed/35"
                            : "border-transparent bg-surface-container-low/70") +
                          " hover:bg-surface-container-low"
                        }
                      >
                        {/*
                         * 去向开关（OD-ARCH-099 S03）：由只读胶囊改成方形点选。选中 =
                         * 本条的「去向」是 本周报引用（计入第 2 张卡、进周报上下文），
                         * 未选 = 仅记录。这里只改那一行，不写周报正文。
                         */}
                        <DestinationSwitch
                          index={sourceIndex}
                          destination={entry.destination}
                          text={entry.text}
                          disabled={loading}
                          onToggle={(next) =>
                            onToggleDestination
                              ? onToggleDestination({ date: day.date, index: sourceIndex, entry, next })
                              : Promise.reject(new Error("当前页面没有接上改去向的命令"))
                          }
                        />
                        {/* 正文最多两行（OD-ARCH-093 S04）：截断只为版式，全文仍可通过
                            title 悬浮提示与「查看本周原文」读到，原文一律不改。 */}
                        <p
                          className="flex-1 min-w-0 text-[13.5px] text-on-surface-variant leading-[1.7] line-clamp-2 break-words"
                          title={entry.text}
                        >
                          {entry.text}
                        </p>
                      </li>
                      );
                    })}
                    {day.raw.map((line, i) => (
                      <li
                        key={"raw" + i}
                        className="px-2.5 py-1.5 -ml-2.5 rounded-lg bg-surface-container-low/50 text-[12px] text-outline leading-relaxed whitespace-pre-wrap break-words"
                        title="这行不符合记录格式，按原文显示，未做改动"
                      >
                        {line}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="mt-1 text-[12px] text-outline/60">—</p>
                )}
              </li>
            );
          })}
        </ol>
      )}

      {parsed.preamble.length > 0 ? (
        <div className="mt-2 border-t border-surface-variant/25 pt-1.5">
          <p className="text-[12px] text-outline pb-1">文件里这些内容不在任何日期下，原样显示：</p>
          <pre className="mono text-[12px] text-outline whitespace-pre-wrap leading-relaxed">
            {parsed.preamble.join("\n")}
          </pre>
        </div>
      ) : null}
      </div>
    </section>
  );
}
