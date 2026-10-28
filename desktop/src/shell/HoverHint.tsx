import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/**
 * 全局悬停说明（OD-ARCH-101）。
 *
 * 原生 `title` 会画出系统自带的气泡，颜色、字体、圆角都不受应用控制，
 * 也无法在禁用控件上稳定出现。这里用一次事件委托接管所有
 * `data-hover-hint` 元素：鼠标停住约 0.3 秒后在触发元素附近画一张
 * 与 StatusToast / 轮次索引卡同源的直角白底卡片。
 *
 * 几点实现约束：
 *   * 只挂一次，挂在 App 顶层；卡片 portal 到 body，不被滚动容器裁切；
 *   * 指针命中用 elementFromPoint 兜底：禁用按钮不派发 hover 事件也能提示；
 *   * 卡片本身 pointer-events-none，不会抢走点击或造成移出闪烁；
 *   * 展示期间给触发元素补 aria-describedby，移出时恢复原值。
 */

const SHOW_DELAY_MS = 320;
const GAP = 6;
const MARGIN = 8;
const MAX_WIDTH = 320;

type Anchor = {
  element: HTMLElement;
  text: string;
  rect: DOMRect;
};

function readableText(value: string | null | undefined): string {
  return (value ?? "").trim();
}

/** 从指针位置找到承载说明的元素；禁用控件同样能命中。 */
function hintAt(x: number, y: number): HTMLElement | null {
  const hit = document.elementFromPoint(x, y) as HTMLElement | null;
  // 命中点落在某个带说明的子节点里（禁用控件的内部文字/图标）时向上找。
  // 禁用控件不派发 mouseenter，但 elementFromPoint 仍能命中它，
  // 所以这里不需要再全表扫描，避免每次移动都读取几十个元素的几何。
  let element: HTMLElement | null = hit;
  while (element) {
    if (element.hasAttribute("data-hover-hint")) return element;
    element = element.parentElement;
  }
  return null;
}

export function HoverHint() {
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const tipRef = useRef<HTMLDivElement | null>(null);
  const timerRef = useRef<number | null>(null);
  const currentRef = useRef<HTMLElement | null>(null);
  // 已排队、尚未到点的触发元素。没有它时，指针在同一个控件内轻微移动会
  // 反复重置计时器，说明可能永远不出现。
  const pendingRef = useRef<HTMLElement | null>(null);
  const describedRef = useRef<{ element: HTMLElement; previous: string | null } | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const releaseDescribedBy = useCallback(() => {
    const saved = describedRef.current;
    if (!saved) return;
    if (saved.previous === null) saved.element.removeAttribute("aria-describedby");
    else saved.element.setAttribute("aria-describedby", saved.previous);
    describedRef.current = null;
  }, []);

  const hide = useCallback(() => {
    clearTimer();
    pendingRef.current = null;
    currentRef.current = null;
    setAnchor(null);
    setPos(null);
    releaseDescribedBy();
  }, [clearTimer, releaseDescribedBy]);

  const show = useCallback((element: HTMLElement) => {
    const text = readableText(element.getAttribute("data-hover-hint"));
    if (!text) {
      hide();
      return;
    }
    // 同一元素重复进入（指针在子节点间移动）不重新计时、不闪。
    // 已展示或已排队都直接返回，避免持续微动把提示一直往后推。
    if (currentRef.current === element) return;
    if (pendingRef.current === element && timerRef.current !== null) return;
    // 换到另一个控件时立即收起旧说明，避免新旧两张卡同时挂着。
    if (currentRef.current && currentRef.current !== element) {
      currentRef.current = null;
      setAnchor(null);
      setPos(null);
      releaseDescribedBy();
    }
    clearTimer();
    pendingRef.current = element;
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      pendingRef.current = null;
      currentRef.current = element;
      setPos(null);
      setAnchor({ element, text, rect: element.getBoundingClientRect() });
    }, SHOW_DELAY_MS);
  }, [clearTimer, hide, releaseDescribedBy]);

  // 指针：每次移动都做一次命中测试，这样禁用按钮也能提示。
  useEffect(() => {
    let frame = 0;
    let last = { x: -1, y: -1 };
    const onMove = (event: PointerEvent) => {
      last = { x: event.clientX, y: event.clientY };
      if (frame) return;
      frame = window.requestAnimationFrame(() => {
        frame = 0;
        const element = hintAt(last.x, last.y);
        if (!element) {
          hide();
          return;
        }
        show(element);
      });
    };
    const onLeave = () => hide();
    const onDown = () => hide();
    const onScroll = () => hide();
    const onResize = () => hide();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    document.addEventListener("pointerleave", onLeave);
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    window.addEventListener("keydown", onKey);
    document.addEventListener("visibilitychange", onLeave);
    return () => {
      window.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerleave", onLeave);
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("visibilitychange", onLeave);
      if (frame) window.cancelAnimationFrame(frame);
      clearTimer();
    };
  }, [clearTimer, hide, show]);

  // 键盘：焦点进入带说明的控件时立即展示，离开即收起。
  useEffect(() => {
    const onFocusIn = (event: FocusEvent) => {
      const element = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-hover-hint]");
      if (!element) return;
      // 鼠标点击也会让按钮获得焦点，但那不是「键盘浏览」。只对 :focus-visible
      // 展示，避免点完按钮后提示卡一直挂着。
      try {
        if (!element.matches(":focus-visible")) return;
      } catch {
        return;
      }
      clearTimer();
      pendingRef.current = null;
      const text = readableText(element.getAttribute("data-hover-hint"));
      if (!text) return;
      currentRef.current = element;
      setPos(null);
      setAnchor({ element, text, rect: element.getBoundingClientRect() });
    };
    const onFocusOut = () => hide();
    document.addEventListener("focusin", onFocusIn);
    document.addEventListener("focusout", onFocusOut);
    return () => {
      document.removeEventListener("focusin", onFocusIn);
      document.removeEventListener("focusout", onFocusOut);
    };
  }, [clearTimer, hide]);

  // 展示期间把说明接到触发元素上（读屏可读），收起时还原。
  useEffect(() => {
    releaseDescribedBy();
    if (!anchor) return;
    describedRef.current = {
      element: anchor.element,
      previous: anchor.element.getAttribute("aria-describedby"),
    };
    anchor.element.setAttribute("aria-describedby", "hover-hint-tooltip");
    return () => releaseDescribedBy();
  }, [anchor, releaseDescribedBy]);

  // 两段式定位：先按最大宽度渲染一次，量到真实尺寸后再夹进视口。
  useLayoutEffect(() => {
    if (!anchor || !tipRef.current) {
      setPos(null);
      return;
    }
    const rect = anchor.rect;
    const tip = tipRef.current.getBoundingClientRect();
    const viewportW = window.innerWidth;
    const viewportH = window.innerHeight;
    const above = rect.top - tip.height - GAP;
    const top = above >= MARGIN ? above : Math.min(rect.bottom + GAP, viewportH - tip.height - MARGIN);
    const centered = rect.left + rect.width / 2 - tip.width / 2;
    const left = Math.max(MARGIN, Math.min(centered, viewportW - tip.width - MARGIN));
    setPos({ left, top: Math.max(MARGIN, top) });
  }, [anchor]);

  if (!anchor) return null;

  return createPortal(
    <div
      id="hover-hint-tooltip"
      ref={tipRef}
      role="tooltip"
      aria-label={anchor.text}
      data-testid="hover-hint"
      data-hover-hint-layer=""
      className="pointer-events-none fixed z-[105] w-max border border-outline-variant/70 bg-surface-container-lowest px-2.5 py-1.5 text-[11.5px] leading-relaxed text-on-surface shadow-[0_10px_28px_-12px_rgba(15,23,42,0.35)] whitespace-pre-line break-words"
      style={{
        borderRadius: 0,
        maxWidth: `min(${MAX_WIDTH}px, calc(100vw - ${MARGIN * 2}px))`,
        left: pos?.left ?? 0,
        top: pos?.top ?? 0,
        visibility: pos ? "visible" : "hidden",
      }}
    >
      {anchor.text}
    </div>,
    document.body,
  );
}
