/**
 * 事项四象限（OD-ARCH-093 S02）。
 *
 * 版式参考用户给出的效率看板：2×2 矩阵，每格一个大编号 + 中文动作名 + 该格条目预览。
 * 配色只用 `theme.css` 既有令牌，按「先做 → 排期 → 回看 → 可缓」由主色向 surface 递减。
 *
 * 诚实性：格子里的分法是**规则推导**，不是模型判断。格下方的口径说明必须与
 * `weeklyQuadrants.ts::classifyWorklog` 的实际逻辑一致；没有「去向」的条目单列
 * 「待归类」，不硬塞进四格。
 */

import { useMemo } from "react";
import { QUADRANTS, itemsIn, type ClassifiedWeek, type QuadrantKey } from "../weeklyQuadrants";

const CELL_STYLE: Record<QuadrantKey, string> = {
  "past-report": "bg-primary text-on-primary",
  "future-report": "bg-primary-fixed/80 text-on-primary-fixed",
  "past-other": "bg-surface-container text-on-surface-variant",
  "future-other": "bg-surface-container-low text-on-surface-variant",
};

const CELL_COUNT_STYLE: Record<QuadrantKey, string> = {
  "past-report": "text-on-primary/70",
  "future-report": "text-primary",
  "past-other": "text-outline",
  "future-other": "text-outline",
};

export function WeeklyQuadrants({
  classified,
  active,
  onSelect,
  subtitle,
  loading,
  error,
}: {
  classified: ClassifiedWeek;
  /** 当前筛选的格子；null 表示未筛选。 */
  active: QuadrantKey | null;
  onSelect: (key: QuadrantKey) => void;
  /** 可选英文副标题（场景配置，缺省不渲染）。 */
  subtitle?: string;
  loading: boolean;
  error: string | null;
}) {
  const empty = useMemo(
    () => classified.items.length === 0 && classified.unclassified === 0,
    [classified],
  );

  return (
    <section
      aria-label="事项四象限"
      className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
    >
      <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
        <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
          事项四象限
        </h2>
        {subtitle ? (
          <span className="text-[9.5px] uppercase tracking-[0.16em] text-outline">{subtitle}</span>
        ) : null}
        <span className="mono text-[10.5px] text-outline ml-auto shrink-0">
          {classified.items.length} 条
        </span>
      </div>

      {error ? (
        <p className="text-[11.5px] text-error leading-relaxed pt-3 pb-1">
          读取本周记录失败：{error}。这里不把读不到当成「没有事项」。
        </p>
      ) : null}

      <div className="grid grid-cols-2 gap-2.5 pt-3.5">
        {QUADRANTS.map((q) => {
          const items = itemsIn(classified, q.key);
          const count = classified.counts[q.key];
          const pressed = active === q.key;
          return (
            <button
              key={q.key}
              onClick={() => onSelect(q.key)}
              aria-pressed={pressed}
              title={`${q.title} · ${q.action}（点击只看这一格，再点取消）`}
              className={
                "text-left rounded-2xl border-0 px-3.5 pt-3.5 pb-3.5 min-h-[128px] flex flex-col " +
                "transition shadow-none " +
                CELL_STYLE[q.key] +
                (pressed ? " ring-2 ring-primary ring-offset-1 ring-offset-white" : " hover:brightness-[0.98]")
              }
            >
              <div className="flex items-baseline gap-2.5">
                <span className="mono text-[18px] font-semibold leading-none opacity-60">{q.index}</span>
                <span className="text-[14px] font-semibold tracking-tight">{q.title}</span>
                <span className={"ml-auto mono text-[14px] " + CELL_COUNT_STYLE[q.key]}>{count}</span>
              </div>
              <div className="text-[10px] leading-relaxed opacity-70 pt-1.5">{q.action}</div>
              <ul className="pt-2.5 flex flex-col gap-1 min-w-0">
                {items.slice(0, 2).map((i) => (
                  <li
                    key={i.id}
                    className="text-[11.5px] leading-relaxed truncate opacity-85"
                    title={i.text}
                  >
                    · {i.text}
                  </li>
                ))}
                {count > 2 ? (
                  <li className="text-[10.5px] opacity-65">还有 {count - 2} 条</li>
                ) : null}
                {count === 0 ? (
                  <li className="text-[10.5px] opacity-60">暂无</li>
                ) : null}
              </ul>
            </button>
          );
        })}
      </div>

      <div className="pt-3.5 mt-3.5 border-t border-surface-variant/20 text-[10px] leading-[1.7] text-outline">
        {loading && empty
          ? "正在读取本周记录…"
          : empty
            ? "本周还没有可归类记录。"
            : null}
        {classified.unclassified > 0 ? (
          <span className="text-on-surface-variant">
            待归类 {classified.unclassified} 条（没写「去向」，不猜重要性）。
          </span>
        ) : null}
        <span className="block pt-1">
          口径：已过日期 = 先做/回看；去向为「本周报引用」= 进周报。按日期与去向确定性归类，不调用模型。
        </span>
      </div>
    </section>
  );
}
