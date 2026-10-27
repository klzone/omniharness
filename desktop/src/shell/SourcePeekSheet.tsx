import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../components/Icon";

/**
 * 原文 / 周报 / 成果的只读预览浮层（OD-ARCH-097 S03）。
 *
 * 以前这块内容是页面流里的一个 `<section>`，一打开就把下面的三栏往下推，
 * 周报页的空间被挤掉。现在改成贴在窗口底部的卡片：主区高度和滚动位置都不动，
 * 内容自己滚动，看完关掉就回到原位。
 *
 * 行为：右上角关闭按钮、Esc、点遮罩三种方式都能关；打开时焦点移到关闭按钮，
 * 关闭后焦点还给触发它的元素。`role="dialog"` + `aria-modal`，读屏知道这是浮层。
 */
export function SourcePeekSheet({
  open,
  title,
  body,
  onClose,
}: {
  open: boolean;
  /** 文件名或「2026-W44 工作记录」这类来源名，同时作为浮层的可访问名称。 */
  title: string;
  body: string;
  onClose: () => void;
}) {
  const closeRef = useRef<HTMLButtonElement | null>(null);
  /** 打开前的焦点元素：关闭后归还，键盘用户不会掉回页面顶部。 */
  const returnFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    returnFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      returnFocusRef.current?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-[90] flex items-end justify-center bg-on-surface/25 px-4 pb-0 pt-6 backdrop-blur-[2px]"
      role="presentation"
      data-testid="source-peek"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-label={title}
        data-source-peek-card=""
        className={
          "flex max-h-[70vh] w-full max-w-4xl flex-col overflow-hidden " +
          // 方角（OD-ARCH-097 S03）：与本产品的方角提示条、工具提示同一套语言，不用圆角气泡。
          "rounded-none border border-b-0 border-outline-variant/60 bg-surface-container-lowest " +
          "shadow-[0_-12px_48px_-24px_rgba(15,23,42,0.45)]"
        }
      >
        <div className="flex items-start gap-3 border-b border-surface-variant/30 px-4 py-3">
          {/* 顶部一道短横线：暗示这是可以往下拖走的浮层，与底部贴合。 */}
          <span
            aria-hidden="true"
            className="mt-[2px] h-[3px] w-8 shrink-0 self-center bg-outline-variant/70"
          />
          <div className="min-w-0 flex-1">
            <p className="text-[12px] uppercase tracking-[0.16em] text-outline">原文预览</p>
            <p className="mt-1 mono truncate text-[12.5px] text-on-surface" title={title}>
              {title}
            </p>
          </div>
          <button
            ref={closeRef}
            type="button"
            onClick={onClose}
            aria-label="关闭预览"
            title="关闭预览（Esc）"
            data-testid="source-peek-close"
            className="flex shrink-0 items-center gap-1 px-2 py-1 text-[12px] text-outline transition-colors hover:text-on-surface"
          >
            <Icon name="x" className="h-3.5 w-3.5" />
            关闭
          </button>
        </div>
        <pre
          className={
            "min-h-0 flex-1 overflow-y-auto whitespace-pre-wrap px-4 py-3 " +
            "text-[12px] leading-[1.75] text-on-surface-variant"
          }
          data-testid="source-peek-body"
        >
          {body}
        </pre>
      </section>
    </div>,
    document.body,
  );
}

