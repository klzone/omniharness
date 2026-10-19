import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import { WindowControls } from "./WindowChrome";

/**
 * 顶栏时钟（OD-ARCH-027 Step 4）。
 *
 * 原先 30 s 一次的 setState 放在 TopNav 里，会让整个顶栏（品牌、图标、按钮）跟着重渲染；
 * 这里把时钟抽成自持状态的小组件，使 tick 只重渲染这一小块。显示内容与更新周期不变。
 */
function NavClock() {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const dateText = now.toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  });
  const timeText = now.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });

  return (
    <div className="hidden lg:flex items-center space-x-2.5 px-3.5 py-1.5 rounded-full bg-surface-container-low/70 border border-white/80">
      <Icon name="calendar" className="w-4 h-4 text-primary/70" />
      <span className="font-label-caps text-[12px] font-medium text-on-surface">
        {dateText}
      </span>
      <span className="text-outline-variant text-[11px]">·</span>
      <span className="font-label-caps text-[11px] text-on-surface-variant font-medium tabular-nums">
        {timeText}
      </span>
    </div>
  );
}

/**
 * 审查说明（诚实性约束）：
 * - 全局搜索、通知、天气在本版本**尚未实现**，一律为禁用态并带 `title="未实现"`，
 *   不使用可点击的假入口（项目硬规则：系统须区分待实现/已实现）。
 * - 「LOCAL IPC ACTIVE」指示来自真实引擎健康状态，而非写死。
 * - 日期/时间取自本机时钟（真实）。
 */
export function TopNav({
  engineHealthy,
  engineLabel,
  onOpenSettings,
}: {
  engineHealthy: boolean | null;
  engineLabel: string;
  onOpenSettings: () => void;
}) {
  return (
    <header
      className="w-full pt-4 px-6 sm:px-10 z-40 flex-shrink-0"
      data-tauri-drag-region
    >
      <nav
        className="bg-surface-container-lowest/80 backdrop-blur-2xl ring-1 ring-white/90 shadow-[0_8px_30px_-6px_rgba(15,23,42,0.04),0_2px_8px_rgba(79,70,229,0.02)] rounded-full w-full max-w-7xl mx-auto flex items-center justify-between px-6 py-2.5 gap-4"
        data-tauri-drag-region
      >
        {/* 品牌 + 本地 IPC 状态（真实状态） */}
        <div className="flex items-center space-x-3.5 shrink-0" data-tauri-drag-region>
          <div className="flex items-center space-x-3 text-on-surface tracking-tight">
            <div className="w-8 h-8 rounded-full bg-gradient-to-tr from-primary to-primary-container flex items-center justify-center text-on-primary shadow-sm">
              <Icon name="brand" className="w-[18px] h-[18px]" />
            </div>
            <span className="font-headline-md text-[17px] font-semibold tracking-tight text-on-surface hidden sm:inline">
              OmniDesk Discipline
            </span>
          </div>
          <span
            title={engineLabel}
            className={
              "hidden md:inline-flex items-center space-x-1.5 px-2.5 py-0.5 rounded-full font-label-caps text-[10px] tracking-wider border " +
              (engineHealthy
                ? "bg-emerald-500/10 text-emerald-600 border-emerald-500/20"
                : "bg-amber-500/10 text-amber-600 border-amber-500/20")
            }
          >
            <span
              className={
                "w-1.5 h-1.5 rounded-full " +
                (engineHealthy ? "bg-emerald-500 status-dot-pulse" : "bg-amber-500")
              }
            />
            <span>{engineHealthy ? "LOCAL IPC ACTIVE" : "ENGINE OFFLINE"}</span>
          </span>
        </div>

        {/* 全局搜索：未实现 —— 禁用态，不做假入口 */}
        <div className="flex-1 max-w-md mx-6 hidden sm:block">
          <div
            className="flex items-center plate-soft-inset rounded-full px-4 py-1.5 opacity-60 cursor-not-allowed"
            title="未实现：全局检索"
          >
            <Icon name="search" className="w-[18px] h-[18px] text-outline mr-2.5" />
            <input
              disabled
              className="bg-transparent border-0 p-0 text-[13px] font-body-sm text-on-surface placeholder:text-outline-variant focus:ring-0 focus:outline-none w-full cursor-not-allowed"
              placeholder="全局检索（未实现）"
              type="text"
            />
            <kbd className="font-label-caps text-[10px] text-outline bg-white/80 px-2 py-0.5 rounded-md shadow-[0_1px_2px_rgba(0,0,0,0.05)] border border-surface-container ml-2">
              ⌘K
            </kbd>
          </div>
        </div>

        {/* 右侧功能簇 */}
        <div className="flex items-center space-x-2.5 sm:space-x-3 shrink-0" data-tauri-drag-region>
          <NavClock />

          <button
            onClick={onOpenSettings}
            className="w-8 h-8 rounded-full text-on-surface-variant hover:text-on-surface hover:bg-surface-container transition-all active:scale-95 flex items-center justify-center"
            title="系统偏好设置 / 引擎技术切片"
          >
            <Icon name="settings" className="w-[19px] h-[19px]" />
          </button>

          <button
            disabled
            className="w-8 h-8 rounded-full text-on-surface-variant/40 flex items-center justify-center cursor-not-allowed"
            title="未实现：通知与待办"
          >
            <Icon name="bell" className="w-[19px] h-[19px]" />
          </button>

          <div className="h-4 w-px bg-outline-variant/30 hidden sm:block" />

          <div
            className="flex items-center space-x-2 pl-0.5"
            title="本地账户（尚未接入身份系统）"
          >
            <div className="w-8 h-8 rounded-full ring-2 ring-primary/20 shadow-sm flex items-center justify-center bg-gradient-to-tr from-surface-container-high to-surface-container text-primary">
              <Icon name="brand" className="w-4 h-4" />
            </div>
          </div>

          {/* 自绘窗口按钮（frameless）：最小化 / 最大化还原 / 关闭 */}
          <WindowControls />
        </div>
      </nav>
    </header>
  );
}
