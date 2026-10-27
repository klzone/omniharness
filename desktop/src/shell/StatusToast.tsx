import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../components/Icon";

/**
 * 右下角状态提示条（OD-ARCH-096）。
 *
 * 形态按用户要求做成**矮平长方形**：单行高度、整宽偏窄、四角直角，
 * 与对话轮次索引的悬停预览卡（OD-ARCH-083 的 `.turn-rail-tooltip`）同源，
 * 不用圆角气泡。挂在 body 上，避免被滚动容器或底部输入区裁切。
 *
 * 时长 60 秒后自动关闭；用户可随时点右侧按钮关掉。同一时刻只显示一条：
 * 新提示替换旧提示并重新计时。
 */

export const STATUS_TOAST_MS = 60_000;

export type StatusToastTone = "success" | "error" | "info";

export type StatusToastMessage = {
  /** 每次提示递增，用来重置计时；同一段文字再次出现也能重新计时。 */
  key: number;
  text: string;
  tone: StatusToastTone;
};

const TONE_BAR: Record<StatusToastTone, string> = {
  success: "bg-secondary",
  error: "bg-error",
  info: "bg-primary",
};

const TONE_ICON: Record<StatusToastTone, { name: "check" | "shield-check" | "spark"; cls: string }> = {
  success: { name: "check", cls: "text-secondary" },
  error: { name: "shield-check", cls: "text-error" },
  info: { name: "spark", cls: "text-primary" },
};

export function StatusToast({
  message,
  onDismiss,
}: {
  message: StatusToastMessage | null;
  onDismiss: () => void;
}) {
  const timerRef = useRef<number | null>(null);
  // 关闭回调每次渲染都是新函数；用 ref 持有，避免把它放进计时依赖里把 60 秒反复重置。
  const dismissRef = useRef(onDismiss);
  dismissRef.current = onDismiss;

  useEffect(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (!message) return;
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      dismissRef.current();
    }, STATUS_TOAST_MS);
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
    // message.key 每次提示都会变：同一段文字再次出现也要重新计时。
  }, [message]);

  if (!message) return null;

  const tone = TONE_ICON[message.tone];

  return createPortal(
    <div
      className="fixed bottom-4 right-4 z-[110] flex max-w-[min(560px,calc(100vw-2rem))] items-center gap-2.5 border border-outline-variant/70 bg-surface-container-lowest px-3 py-2 shadow-[0_10px_28px_-12px_rgba(15,23,42,0.35)]"
      style={{ borderRadius: 0 }}
      role={message.tone === "error" ? "alert" : "status"}
      aria-live={message.tone === "error" ? "assertive" : "polite"}
      /*
       * 读屏与自动化都靠这个名称拿到提示正文：WebKit 不会把容器里 <p> 的文字
       * 暴露成可访问文本（实测 AT-SPI 里只有空 paragraph），只给 aria-live 的话
       * 屏幕阅读器读不出内容。名称里带上完整文案，才真正可读、可断言。
       */
      aria-label={message.text}
      data-testid="status-toast"
      data-tone={message.tone}
    >
      <span aria-hidden="true" className={"h-4 w-[3px] flex-shrink-0 " + TONE_BAR[message.tone]} />
      <Icon name={tone.name} className={"h-3.5 w-3.5 flex-shrink-0 " + tone.cls} />
      <p className="min-w-0 flex-1 text-[11.5px] leading-snug text-on-surface">{message.text}</p>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="关闭提示"
        title="关闭提示"
        data-testid="status-toast-close"
        className="flex-shrink-0 p-0.5 text-outline transition-colors hover:text-on-surface"
      >
        <Icon name="x" className="h-3.5 w-3.5" />
      </button>
    </div>,
    document.body,
  );
}
