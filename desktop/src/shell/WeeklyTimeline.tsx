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

import { useMemo, type ReactNode } from "react";
import {
  buildTimeline,
  isEmptyTimeline,
  localToday,
  parseWorklog,
  shortDate,
  weekdayLabel,
} from "../weekTimeline";

export function WeeklyTimeline({
  week,
  content,
  error,
  loading,
  onOpenSource,
  onRequestReport,
  statusRow,
}: {
  week: string;
  content: string;
  error: string | null;
  loading: boolean;
  onOpenSource: () => void;
  onRequestReport: () => void;
  /** 卡片头的动作行（OD-ARCH-092：周报状态并入这里，不再单独占一块）。 */
  statusRow?: ReactNode;
}) {
  const parsed = useMemo(() => parseWorklog(content), [content]);
  const rows = useMemo(() => buildTimeline(parsed, week), [parsed, week]);
  const empty = useMemo(() => isEmptyTimeline(parsed.days), [parsed]);
  const today = localToday();

  return (
    <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-3 min-w-0">
      <div className="flex items-baseline gap-2 pb-2">
        <h2 className="font-headline-md text-[14px] font-semibold text-on-surface">本周记录</h2>
        <span className="mono text-[11px] text-outline">
          {week}
          {empty ? "" : ` · ${parsed.entryCount} 条`}
        </span>
        <button
          className="ml-auto text-[11px] text-outline hover:text-primary shrink-0"
          onClick={onOpenSource}
          disabled={empty && !content}
          title="在工作记录文件里看原文"
        >
          打开本周原文
        </button>
      </div>

      {statusRow ? (
        <div className="pb-2 mb-2 border-b border-surface-variant/25">{statusRow}</div>
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
            className="mt-2 px-2.5 py-1 text-[11px] bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
            onClick={onRequestReport}
          >
            去写一条记录
          </button>
        </div>
      ) : (
        <ol className="relative pl-[54px]">
          {/* 连接轨：一条细线，贯穿整周；比逐条卡片的边线更安静。 */}
          <span
            aria-hidden
            className="absolute left-[47px] top-2 bottom-2 w-px bg-outline-variant/45"
          />
          {rows.map((day) => {
            const hasContent = day.entries.length > 0 || day.raw.length > 0;
            const isToday = day.date === today;
            return (
              <li key={day.date + (day.continued ? "#c" : "")} className="relative pb-2.5 last:pb-0">
                <span
                  aria-hidden
                  className={
                    "absolute -left-[54px] top-[5px] ml-[42px] h-[9px] w-[9px] " +
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
                  <span className={"text-[11.5px] " + (isToday ? "text-primary" : "text-outline")}>
                    {weekdayLabel(day.date)}
                    {isToday ? " · 今天" : ""}
                    {day.outOfWeek ? " · 不在本周" : ""}
                  </span>
                </div>

                {hasContent ? (
                  <ul className="mt-1 flex flex-col gap-1">
                    {day.entries.map((entry, i) => (
                      <li
                        key={i}
                        className="group flex items-start gap-2 px-2 py-1 -ml-2 hover:bg-surface-container-low/70"
                      >
                        <p className="flex-1 min-w-0 text-[13.5px] text-on-surface-variant leading-relaxed whitespace-pre-wrap break-words">
                          {entry.text}
                        </p>
                        {entry.destination ? (
                          <span
                            className="shrink-0 mt-[2px] px-1 py-[1px] text-[11px] text-outline border border-outline-variant/60"
                            title="这条记录的归处（写在记录文件里）"
                          >
                            {entry.destination}
                          </span>
                        ) : null}
                        <button
                          className="shrink-0 mt-[2px] text-[11px] text-outline opacity-0 group-hover:opacity-100 focus:opacity-100 hover:text-primary"
                          onClick={onRequestReport}
                          title="回首页，把记录整理成周报"
                        >
                          转周报
                        </button>
                      </li>
                    ))}
                    {day.raw.map((line, i) => (
                      <li
                        key={"raw" + i}
                        className="px-2 py-1 -ml-2 bg-surface-container-low/50 text-[12px] text-outline leading-relaxed whitespace-pre-wrap break-words"
                        title="这行不符合记录格式，按原文显示，未做改动"
                      >
                        {line}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="mt-0.5 text-[12px] text-outline/60">—</p>
                )}
              </li>
            );
          })}
        </ol>
      )}

      {parsed.preamble.length > 0 ? (
        <div className="mt-2 border-t border-surface-variant/25 pt-1.5">
          <p className="text-[11px] text-outline pb-1">文件里这些内容不在任何日期下，原样显示：</p>
          <pre className="mono text-[11.5px] text-outline whitespace-pre-wrap leading-relaxed">
            {parsed.preamble.join("\n")}
          </pre>
        </div>
      ) : null}
    </section>
  );
}
