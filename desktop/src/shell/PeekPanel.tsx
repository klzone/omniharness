import { useState, type ReactNode } from "react";

/**
 * 悬停展开的面板（OD-ARCH-038）。
 *
 * 场景页的「提醒/建议栏 + 成果/进度」与首页的「今日概览」在真实使用中占掉了太多纵向空间，
 * 而对话才是主工作面。这里把它们收成**一条细把手**：默认只留把手，鼠标划上去时面板浮在
 * 对话之上（两层，随时可展开），把手也可键盘聚焦展开。
 *
 * 事实性：面板内容是既有内容原样搬过来，不新增任何数据口径。
 */
export function PeekPanel({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  const [pinned, setPinned] = useState(false);

  return (
    <div className="group/peek relative z-20 shrink-0">
      {/* 展开态：面板浮在上层（必须在 group 内，group-hover 才能作用到它） */}
      <div
        className={
          "absolute left-0 right-0 bottom-full mb-2 transition-all duration-200 " +
          (pinned
            ? "opacity-100 translate-y-0 pointer-events-auto"
            : "opacity-0 translate-y-2 pointer-events-none group-hover/peek:opacity-100 group-hover/peek:translate-y-0 group-hover/peek:pointer-events-auto focus-within:opacity-100 focus-within:translate-y-0")
        }
      >
        <div className="rounded-2xl bg-white/95 backdrop-blur-sm ring-1 ring-outline-variant/30 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.28)] p-3">
          {children}
        </div>
      </div>

      {/* 把手：平时就这一条，划上去面板浮出 */}
      <div className="flex items-center justify-center gap-2 py-1">
        <button
          type="button"
          onClick={() => setPinned((v) => !v)}
          onBlur={() => setPinned(false)}
          className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full plate-soft-inset text-[10.5px] text-on-surface-variant font-label-caps hover:bg-surface-container transition-colors"
          title="悬停查看，或点击展开/收起"
        >
          <span>{label}</span>
          {hint ? <span className="text-[9.5px] text-outline">{hint}</span> : null}
          <span className="text-[9px] opacity-70">{pinned ? "▼" : "▲"}</span>
        </button>
      </div>
    </div>
  );
}
