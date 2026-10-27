/**
 * 事项四象限（OD-ARCH-093 S02；OD-ARCH-097 S01 重绘）。
 *
 * 版式：2×2 矩阵，**轴标放在矩阵框外**——上方 URGENT / NOT URGENT，左侧
 * IMPORTANT / NOT IMPORTANT（竖排）。参考图的骨架就在这里：轴标承担语义，
 * 格子里就只剩标题与条目，不需要再写一行「已过日期 · 进周报」的说明。
 *
 * 诚实性：格子里的分法是**规则推导**，不是模型判断；没有「去向」的条目不硬塞进四格。
 * 英文轴标只是版式语言，中文语义仍写在 `title` 与 `aria-label` 里，读屏不丢信息。
 */

import { useMemo } from "react";
import { QUADRANTS, itemsIn, type ClassifiedWeek, type QuadrantKey } from "../weeklyQuadrants";

const CELL_STYLE: Record<QuadrantKey, string> = {
  "past-report": "bg-primary text-on-primary",
  "future-report": "bg-primary-fixed/80 text-on-primary-fixed",
  "past-other": "bg-surface-container text-on-surface-variant",
  "future-other": "bg-surface-container-low text-on-surface-variant",
};

/** 两列轴标：左列=紧急（已过日期），右列=不急。 */
const COLUMNS = [
  { key: "urgent", label: "URGENT", cn: "紧急 · 已过日期" },
  { key: "not-urgent", label: "NOT URGENT", cn: "不紧急 · 今天及以后" },
];

/** 两行轴标：上排=重要（进周报），下排=不重要。 */
const ROWS = [
  { key: "important", label: "IMPORTANT", cn: "重要 · 进周报" },
  { key: "not-important", label: "NOT IMPORTANT", cn: "一般 · 其它去向" },
];

/** 行 × 列 → 格子键，与 `QUADRANTS` 的顺序一致。 */
const GRID: QuadrantKey[][] = [
  ["past-report", "future-report"],
  ["past-other", "future-other"],
];

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
  const byKey = useMemo(
    () => new Map(QUADRANTS.map((q) => [q.key, q])),
    [],
  );

  return (
    <section
      aria-label="事项四象限"
      className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
      data-weekly-quadrants=""
    >
      <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
        <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
          事项四象限
        </h2>
        {subtitle ? (
          <span className="text-[12px] uppercase tracking-[0.16em] text-outline">{subtitle}</span>
        ) : null}
        <span className="mono text-[12px] text-outline ml-auto shrink-0">
          {classified.items.length} 条
        </span>
      </div>

      {error ? (
        <p className="text-[12px] text-error leading-relaxed pt-3 pb-1">
          读取本周记录失败：{error}。这里不把读不到当成「没有事项」。
        </p>
      ) : null}

      {/* 轴标在框外：上面一行列标，左侧一列竖排行标，中间才是四格。 */}
      <div className="pt-3.5 grid grid-cols-[18px_minmax(0,1fr)_minmax(0,1fr)] gap-x-2 gap-y-1.5">
        <span aria-hidden="true" />
        {/* 轴标本身是排版文本，WebKit 不会把它单独暴露到无障碍树；
            用 role=group + aria-label 让「英文轴标 + 中文语义」都读得到。 */}
        {COLUMNS.map((col) => (
          <span
            key={col.key}
            className="pb-0.5 text-center mono text-[11px] uppercase tracking-[0.18em] text-outline"
            title={col.cn}
            role="group"
            aria-label={`${col.label} · ${col.cn}`}
          >
            {col.label}
          </span>
        ))}

        {ROWS.map((row, rowIndex) => (
          <div key={row.key} className="contents">
            {/* 竖排行标：旋转 90°，与参考图同构。 */}
            <span
              className="flex items-center justify-center"
              title={row.cn}
              role="group"
              aria-label={`${row.label} · ${row.cn}`}
            >
              <span className="mono text-[11px] uppercase tracking-[0.18em] text-outline [writing-mode:vertical-rl] rotate-180 whitespace-nowrap">
                {row.label}
              </span>
            </span>
            {GRID[rowIndex].map((key, colIndex) => {
              const q = byKey.get(key);
              if (!q) return <span key={key} />;
              const items = itemsIn(classified, key);
              const count = classified.counts[key];
              const pressed = active === key;
              return (
                <button
                  key={key}
                  onClick={() => onSelect(key)}
                  aria-pressed={pressed}
                  aria-label={`${row.cn} · ${COLUMNS[colIndex].cn} · ${q.title} · ${count} 条`}
                  title={`${q.title} · ${q.action}（点击只看这一格，再点取消）`}
                  data-quadrant={key}
                  className={
                    // 本项目的 `rounded-xl` 是 3rem（设计令牌），112px 高的格子会变成胶囊形；
                    // 这里按参考图的方角小圆角写死 8px。
                    "text-left rounded-[8px] border-0 px-3 pt-2.5 pb-2.5 min-h-[112px] flex flex-col " +
                    "transition shadow-none " +
                    CELL_STYLE[key] +
                    (pressed
                      ? " ring-2 ring-primary ring-offset-1 ring-offset-white"
                      : " hover:brightness-[0.98]")
                  }
                >
                  <div className="flex items-baseline gap-2">
                    <span className="mono text-[12px] font-semibold leading-none opacity-55">
                      {q.index}
                    </span>
                    <span className="text-[13.5px] font-semibold tracking-tight">{q.title}</span>
                  </div>
                  <ul className="pt-1.5 flex flex-col gap-1 min-w-0">
                    {items.slice(0, 2).map((i) => (
                      <li
                        key={i.id}
                        className="text-[12px] leading-relaxed truncate opacity-85"
                        title={i.text}
                      >
                        · {i.text}
                      </li>
                    ))}
                    {count > 2 ? (
                      <li className="text-[12px] opacity-60">还有 {count - 2} 条</li>
                    ) : null}
                    {count === 0 ? (
                      <li className="text-[12px] opacity-55">暂无</li>
                    ) : null}
                  </ul>
                </button>
              );
            })}
          </div>
        ))}
      </div>

      {loading && empty ? (
        <p className="pt-3.5 mt-3.5 border-t border-surface-variant/20 text-[12px] leading-[1.7] text-outline">
          正在读取本周记录…
        </p>
      ) : null}
    </section>
  );
}

