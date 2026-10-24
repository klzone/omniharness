import { useCallback, useEffect, useMemo, useRef, useState } from "react";

/**
 * 对话轮次索引条（OD-ARCH-079）。
 *
 * 对齐 DSH 原生 `TurnNavigator` 的可见行为，但不引入它的虚拟列表依赖：
 *   * 固定节距的刻度线，轮次多了在条内滚动，不压缩、不挤到底部输入框；
 *   * 当前轮次的刻度加深加长，鼠标移上去的刻度变宽；
 *   * 悬停/聚焦时在左侧浮出预览卡（问句 + 回答摘要），点刻度跳到该轮；
 *   * 条两端可滚动时做渐隐，避免出现「明明还有却看不到」的错觉。
 *
 * 只做导航，不改消息、不发请求；刻度数量与消息一一对应，没有真实轮次就不画。
 */

/** 相邻刻度中心的固定间距（px）。 */
const PITCH = 10;
/** 首尾刻度到条内边缘的留白（px）。 */
const INSET = 6;
/** 预览卡最大高度（px），用于把卡片夹在可视范围内。 */
const PREVIEW_H = 96;

export interface TurnRailItem {
  /** 该轮用户消息的 id，也是跳转锚点。 */
  id: string;
  /** 问句原文，用于预览与无障碍标签。 */
  prompt: string;
  /** 该轮回答摘要；没有回答时为空串。 */
  response?: string;
}

export interface TurnRailProps {
  items: TurnRailItem[];
  activeId: string | null;
  /** 正在生成的那一轮（刻度做呼吸提示）；没有则 null。 */
  busyId?: string | null;
  onJump: (id: string) => void;
}

function oneLine(text: string | undefined, limit: number): string {
  const flat = (text ?? "").replace(/\s+/g, " ").trim();
  if (flat.length <= limit) return flat;
  return flat.slice(0, limit) + "…";
}

export function TurnRail({ items, activeId, busyId, onJump }: TurnRailProps) {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const [preview, setPreview] = useState<{ index: number; top: number } | null>(null);
  const [fade, setFade] = useState({ top: false, bottom: false });

  const activeIndex = useMemo(() => items.findIndex((i) => i.id === activeId), [items, activeId]);

  /** 条内两端的渐隐状态只看条自己的滚动位置，与对话滚动无关。 */
  const syncFade = useCallback(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const top = el.scrollTop > 1;
    const bottom = el.scrollTop + el.clientHeight < el.scrollHeight - 1;
    setFade((prev) => (prev.top === top && prev.bottom === bottom ? prev : { top, bottom }));
  }, []);

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    syncFade();
    el.addEventListener("scroll", syncFade, { passive: true });
    const observer = new ResizeObserver(syncFade);
    observer.observe(el);
    return () => {
      el.removeEventListener("scroll", syncFade);
      observer.disconnect();
    };
  }, [syncFade, items.length]);

  /** 当前轮滚出条的可见范围时，把条自己滚到能看见它（不带动对话）。 */
  useEffect(() => {
    const el = scrollerRef.current;
    if (!el || activeIndex < 0) return;
    const center = INSET + activeIndex * PITCH + PITCH / 2;
    const top = el.scrollTop;
    const bottom = top + el.clientHeight;
    if (center < top + 24 || center > bottom - 24) {
      el.scrollTop = Math.max(0, center - el.clientHeight / 2);
    }
  }, [activeIndex]);

  const showPreview = useCallback((index: number) => {
    const el = scrollerRef.current;
    if (!el) return;
    const height = el.clientHeight;
    const center = INSET + index * PITCH + PITCH / 2 - el.scrollTop;
    const top = Math.max(0, Math.min(Math.max(0, height - PREVIEW_H), center - PREVIEW_H / 2));
    setPreview({ index, top });
  }, []);

  if (items.length < 2) return null;

  const shown = preview ? items[preview.index] : null;

  return (
    <div
      className="pointer-events-none absolute inset-y-1 right-0 hidden w-7 lg:block"
      data-testid="message-index-rail"
    >
      <div className="relative h-full w-full">
        <nav
          aria-label="对话轮次索引"
          className="turn-rail-frame pointer-events-auto absolute right-1 top-1/2 flex max-h-[min(420px,60%)] w-6 -translate-y-1/2 cursor-pointer flex-col"
          onPointerEnter={() => syncFade()}
          onPointerLeave={() => setPreview(null)}
        >
          <div
            ref={scrollerRef}
            className={
              "turn-rail-scroller relative max-h-full overflow-y-auto overscroll-contain" +
              (fade.top ? " turn-rail-fade-top" : "") +
              (fade.bottom ? " turn-rail-fade-bottom" : "")
            }
          >
            <div className="relative" style={{ height: INSET * 2 + items.length * PITCH }}>
              {items.map((item, index) => {
                const active = item.id === activeId;
                const busy = item.id === busyId;
                return (
                  <button
                    key={item.id}
                    type="button"
                    aria-label={`跳到第 ${index + 1} 轮：${oneLine(item.prompt, 24) || "未命名"}`}
                    aria-current={active ? "true" : undefined}
                    title={`第 ${index + 1} 轮 · ${oneLine(item.prompt, 20)}`}
                    onClick={() => onJump(item.id)}
                    onPointerEnter={() => showPreview(index)}
                    onFocus={() => showPreview(index)}
                    onBlur={() => setPreview(null)}
                    className="turn-rail-mark absolute left-0 flex w-full items-center justify-end border-0 bg-transparent p-0"
                    style={{ top: INSET + index * PITCH, height: PITCH }}
                  >
                    <span
                      className={
                        "turn-rail-tick block h-[2px] w-5 rounded-full" +
                        (active
                          ? " turn-rail-tick-active"
                          : preview?.index === index
                            ? " turn-rail-tick-preview"
                            : "") +
                        (busy ? " turn-rail-tick-busy" : "")
                      }
                    />
                  </button>
                );
              })}
            </div>
          </div>
          {shown ? (
            <div
              role="tooltip"
              className="pointer-events-none absolute right-6 w-[300px] max-w-[70vw] overflow-hidden rounded-xl border border-surface-variant/40 bg-surface-container-lowest px-3 py-2.5 shadow-lg"
              style={{ top: preview!.top }}
            >
              <p className="line-clamp-1 text-[12px] font-medium text-on-surface">
                {oneLine(shown.prompt, 60) || `第 ${preview!.index + 1} 轮`}
              </p>
              {shown.response ? (
                <p className="mt-1 line-clamp-3 text-[11.5px] leading-relaxed text-on-surface-variant">
                  {oneLine(shown.response, 140)}
                </p>
              ) : null}
            </div>
          ) : null}
        </nav>
      </div>
    </div>
  );
}
