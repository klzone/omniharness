import { useEffect, useRef } from "react";

/**
 * 应用内确认对话框。
 *
 * 不用 window.confirm：WebKitGTK 在真实窗口里会直接返回（实测见
 * tests/verification/desktop/evidence/od-kb-001-p2/），草稿会被静默丢弃。
 * 这里用普通 DOM 渲染，行为可预测、可被窗口测试定位，也避免阻塞渲染线程。
 */
export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = "继续",
  cancelLabel = "取消",
  onConfirm,
  onCancel,
  testId = "confirm-dialog",
}: {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
  testId?: string;
}) {
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    // 默认焦点放在「取消」：确认框出现时回车不会直接丢弃草稿。
    cancelRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-inverse-surface/35 backdrop-blur-[2px] p-6"
      data-testid={testId}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        className="w-full max-w-md rounded-2xl bg-white px-5 py-4 ring-1 ring-outline-variant/40 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.28)]"
      >
        <p className="text-[14px] font-semibold text-on-surface">{title}</p>
        <p className="mt-2 text-[12px] leading-relaxed text-on-surface-variant">{description}</p>
        <div className="mt-4 flex items-center justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary"
            data-testid={`${testId}-cancel`}
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary"
            data-testid={`${testId}-accept`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
