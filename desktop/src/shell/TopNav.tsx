import { useCallback, useEffect, useRef, useState } from "react";
import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { ProfileChoiceOption, ProfileChoiceView } from "../types";
import { ThreeWeekCalendar } from "./ThreeWeekCalendar";
import { WindowControls } from "./WindowChrome";

/**
 * 顶栏（OD-ARCH-072 首页改版）。
 *
 * 三处结构变化：
 * 1. 左侧品牌固定为软件名「OmniDesk」；后面的 Profile 徽标是**独立**控件，
 *    只决定「下次启动引擎用哪个 Profile」，不再顶替品牌名。
 * 2. 日期区域从常驻展示改为**悬浮卡片入口**：点日期弹出三周节律，鼠标移开 1 秒淡出。
 * 3. 时钟仍是自持状态的小组件，tick 只重渲染这一小块，不影响品牌与按钮。
 *
 * 诚实性约束：
 * - 全局搜索、通知在本版本**尚未实现**，一律禁用并标注「未实现」，不做假入口；
 * - 「LOCAL IPC ACTIVE」来自真实引擎健康状态；
 * - Profile 列表来自宿主对隔离数据根的读盘结果；读不到就显示原因，
 *   不伪造可选 Profile，也不把「保存成功」说成「已切换」。
 */

/** 鼠标移开后多久淡出悬浮日程卡。 */
const CALENDAR_HIDE_MS = 1000;

function useNow(intervalMs: number): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), intervalMs);
    return () => window.clearInterval(timer);
  }, [intervalMs]);
  return now;
}

function dateLabel(now: Date): string {
  return now.toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "long",
  });
}

function timeLabel(now: Date): string {
  return now.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/**
 * 日期胶囊 + 三周节律悬浮卡。
 *
 * 交互（用户口径）：点日期出现，鼠标移开后 1 秒淡出；移回卡片或日期则取消淡出。
 * 淡出用 visibility 过渡实现，淡出期间仍可移回，不会「一闪就没」。
 */
function DateFlip({ now }: { now: Date }) {
  const [open, setOpen] = useState(false);
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<number | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const cancelHide = useCallback(() => {
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
  }, []);

  const scheduleHide = useCallback(() => {
    cancelHide();
    hideTimer.current = window.setTimeout(() => {
      setVisible(false);
      hideTimer.current = null;
    }, CALENDAR_HIDE_MS);
  }, [cancelHide]);

  const show = useCallback(() => {
    cancelHide();
    setOpen(true);
    setVisible(true);
  }, [cancelHide]);

  // 淡出结束后再卸载内容，保证过渡完整、也不留焦点陷阱。
  useEffect(() => {
    if (visible) return;
    const timer = window.setTimeout(() => setOpen(false), 220);
    return () => window.clearTimeout(timer);
  }, [visible]);

  useEffect(() => () => cancelHide(), [cancelHide]);

  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setVisible(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setVisible(false);
    };
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div
      className="relative"
      ref={rootRef}
      onMouseEnter={() => (visible ? cancelHide() : undefined)}
      onMouseLeave={scheduleHide}
    >
      <button
        type="button"
        onClick={() => (visible ? setVisible(false) : show())}
        aria-expanded={visible}
        aria-label={"日期与三周节律，今天 " + dateLabel(now)}
        data-testid="topnav-date"
        className="hidden lg:flex items-center space-x-2.5 px-3.5 py-1.5 rounded-full bg-surface-container-low/70 border border-white/80 hover:bg-surface-container-low transition-colors"
      >
        <Icon name="calendar" className="w-4 h-4 text-primary/70" />
        <span className="font-label-caps text-[12px] font-medium text-on-surface">
          {dateLabel(now)}
        </span>
        <span className="text-outline-variant text-[11px]">·</span>
        <span className="font-label-caps text-[11px] text-on-surface-variant font-medium tabular-nums">
          {timeLabel(now)}
        </span>
      </button>

      {open ? (
        <div
          data-testid="topnav-calendar-popover"
          className={
            "absolute right-0 top-full mt-2 w-[320px] rounded-2xl bg-white/95 backdrop-blur-sm ring-1 ring-outline-variant/30 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.25)] p-3.5 z-50 origin-top-right transition-all duration-200 " +
            (visible ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-1 pointer-events-none")
          }
        >
          <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
            <div className="flex items-center space-x-1.5">
              <Icon name="calendar" className="w-[13px] h-[13px] text-secondary" />
              <span className="font-headline-md text-[12px] font-semibold text-on-surface">
                日程节律
              </span>
            </div>
            <span
              className="inline-flex items-center px-1.5 py-0.5 rounded-full bg-surface-container text-outline font-label-caps text-[9px]"
              title="日程同步未实现：这里只按本机日期给出时间骨架"
            >
              未同步
            </span>
          </div>
          <div className="pt-2">
            <ThreeWeekCalendar now={now} />
          </div>
        </div>
      ) : null}
    </div>
  );
}

/**
 * Profile 徽标 + 选择弹层（OD-ARCH-072）。
 *
 * 选择只保存一个 Profile 名到应用数据根；改的是「下次启动引擎用哪个」，不重启、不改路由。
 * 结构上刻意与软件名分开：品牌永远是 OmniDesk，Profile 只是运行配置。
 */
function ProfileBadge() {
  const [view, setView] = useState<ProfileChoiceView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<ProfileChoiceOption | null>(null);
  const [saving, setSaving] = useState(false);
  const [receipt, setReceipt] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const load = useCallback(() => {
    shell
      .profileChoiceView()
      .then((r) => {
        setView(r);
        setError(r.warning ?? null);
      })
      .catch((e) => {
        setView(null);
        setError(e instanceof Error ? e.message : String(e));
      });
  }, []);

  useEffect(load, [load]);

  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
        setPending(null);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        setPending(null);
      }
    };
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const currentLabel = (() => {
    if (!view) return "读取中…";
    if (view.saved_out_of_whitelist) {
      return "不在清单（已回落默认） · " + String(view.saved);
    }
    const activeName = view.saved ?? view.standard;
    const hit = view.options.find((o) => o.name === activeName);
    return hit ? `${hit.label} (${hit.name})` : activeName;
  })();

  /**
   * 徽标上只显示一个短标签（OD-ARCH-078 S2 / OD-ARCH-082 S6）：
   * 并入 plate-soft 体系，不在按钮上铺开长文本。
   */
  const badgeLabel = (() => {
    if (!view) return "读取中";
    if (view.saved_out_of_whitelist) return "标准";
    if (!view.saved || view.saved === "omnidesk-base") return "标准";
    if (view.saved === "discipline") return "专业";
    return view.saved;
  })();

  const confirm = () => {
    if (!pending) return;
    const target = pending.standard ? null : pending.name;
    setSaving(true);
    shell
      .profileChoiceSet(target)
      .then((r) => {
        setReceipt(r.note);
        setPending(null);
        setOpen(false);
        load();
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setSaving(false));
  };

  return (
    <div className="relative" ref={rootRef}>
      <button
        type="button"
        onClick={() => {
          setReceipt(null);
          setOpen((v) => !v);
          if (!view) load();
        }}
        title={"引擎 Profile：" + currentLabel + "（只影响下次启动，不改模型路由）"}
        aria-haspopup="dialog"
        aria-expanded={open}
        data-testid="topnav-profile-badge"
        className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft text-primary hover:bg-surface-container transition-colors shadow-sm"
      >
        <span className="w-1.5 h-1.5 rounded-full bg-primary/70 flex-shrink-0" />
        <span className="font-label-caps text-[10.5px] tracking-wide whitespace-nowrap">
          {badgeLabel}
        </span>
        <Icon
          name="chevron-down"
          className={"w-2.5 h-2.5 opacity-70 transition-transform flex-shrink-0 " + (open ? "rotate-180" : "")}
        />
      </button>

      {open ? (
        <div
          role="dialog"
          aria-label="选择引擎 Profile"
          data-testid="topnav-profile-popover"
          className="absolute left-0 top-full mt-2 w-[300px] rounded-2xl bg-white/97 backdrop-blur-sm ring-1 ring-outline-variant/30 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.25)] p-2.5 z-50"
        >
          {pending ? (
            <div
              role="alertdialog"
              aria-label={`切换到「${pending.label}」？`}
              className="space-y-2.5 p-1"
            >
              <p className="text-[12px] text-on-surface font-medium" data-testid="topnav-profile-confirm-title">
                切换到「{pending.label}」？
              </p>
              <p className="text-[11px] text-on-surface-variant leading-relaxed">
                Profile 决定引擎加载哪些插件与能力。改动会在
                <span className="font-medium text-on-surface">下次启动引擎</span>
                时生效，当前正在运行的任务不受影响。
              </p>
              <div className="flex items-center justify-end gap-2 pt-0.5">
                <button
                  type="button"
                  onClick={() => setPending(null)}
                  disabled={saving}
                  className="px-3 py-1 rounded-full text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-50"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={confirm}
                  disabled={saving}
                  data-testid="topnav-profile-confirm"
                  className="px-3 py-1 rounded-full bg-primary text-on-primary text-[11.5px] font-medium disabled:opacity-50"
                >
                  {saving ? "保存中…" : "确认"}
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="px-1.5 pb-1.5 text-[10px] uppercase tracking-wide text-outline">
                下次启动使用 · 当前：{currentLabel}
              </div>
              {view?.saved_out_of_whitelist ? (
                <div className="mx-1 mb-2 p-2 rounded-lg bg-amber-500/10 border border-amber-500/30 text-[11px] text-amber-900 leading-relaxed">
                  当前保存的 Profile 不在可选清单，本次使用默认。
                </div>
              ) : null}
              {view === null ? (
                <p className="px-1.5 py-2 text-[11.5px] text-error leading-relaxed">
                  {error ?? "正在读取 Profile 清单…"}
                </p>
              ) : (
                <ul role="listbox" aria-label="可选 Profile" className="max-h-[260px] overflow-y-auto">
                  {view.options.map((o) => {
                    const on = o.standard ? !view.saved : view.saved === o.name;
                    return (
                      <li key={o.name}>
                        <button
                          type="button"
                          role="option"
                          aria-selected={on}
                          data-testid={"topnav-profile-" + o.name}
                          onClick={() => setPending(o)}
                          className={
                            "w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors " +
                            (on
                              ? "bg-primary/10 text-primary"
                              : "text-on-surface-variant hover:bg-surface-container")
                          }
                        >
                          <span className="flex-1 min-w-0">
                            <span className="block text-[11.5px] font-medium truncate">
                              {o.label}
                            </span>
                            <span className="block text-[10px] text-outline truncate font-mono">
                              {o.standard ? `通道默认 (${o.name})` : o.name}
                            </span>
                          </span>
                          {on ? <Icon name="check" className="w-[13px] h-[13px] flex-shrink-0" /> : null}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
              <p className="mt-1.5 pt-1.5 border-t border-surface-variant/20 px-1.5 text-[10px] text-outline leading-relaxed">
                {view
                  ? "本次启动实际使用：" + view.effective + "（" + view.channel + " 通道）"
                  : "读取隔离数据根失败时不会显示可选清单。"}
              </p>
              {error && view ? (
                <p className="px-1.5 pt-1 text-[10px] text-error leading-relaxed">{error}</p>
              ) : null}
            </>
          )}
        </div>
      ) : null}

      {receipt ? (
        <p
          className="absolute left-0 top-full mt-1.5 text-[10px] text-emerald-600 whitespace-nowrap z-40"
          role="status"
        >
          {receipt}
        </p>
      ) : null}
    </div>
  );
}

export interface TopNavProps {
  engineHealthy: boolean | null;
  engineLabel: string;
  onOpenSettings: () => void;
  searchQuery?: string;
  onSearchQueryChange?: (q: string) => void;
  searchMatchCount?: number;
  searchCurrentIndex?: number;
  onSearchPrev?: () => void;
  onSearchNext?: () => void;
  onSearchClear?: () => void;
}

export function TopNav({
  engineHealthy,
  engineLabel,
  onOpenSettings,
  searchQuery = "",
  onSearchQueryChange,
  searchMatchCount = 0,
  searchCurrentIndex = 0,
  onSearchPrev,
  onSearchNext,
  onSearchClear,
}: TopNavProps) {
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const onGlobalKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };
    document.addEventListener("keydown", onGlobalKey);
    return () => document.removeEventListener("keydown", onGlobalKey);
  }, []);
  // 时钟与日期共用一个刷新源；只在 lg 以上显示，与旧版一致。
  const now = useNow(30_000);

  return (
    <header className="w-full pt-4 px-6 sm:px-10 z-40 flex-shrink-0" data-tauri-drag-region>
      <nav
        className="bg-surface-container-lowest/80 backdrop-blur-2xl ring-1 ring-white/90 shadow-[0_8px_30px_-6px_rgba(15,23,42,0.04),0_2px_8px_rgba(79,70,229,0.02)] rounded-full w-full max-w-7xl mx-auto flex items-center justify-between px-6 py-2.5 gap-4"
        data-tauri-drag-region
      >
        {/* 品牌（固定软件名） + Profile 徽标 + 本地 IPC 状态 */}
        <div className="flex items-center space-x-3 shrink-0" data-tauri-drag-region>
          <div className="flex items-center space-x-2.5 text-on-surface tracking-tight">
            <img
              src="/omnidesk-icon.svg"
              alt="OmniDesk"
              draggable={false}
              decoding="async"
              className="w-8 h-8 rounded-[7px] ring-1 ring-black/5 shadow-xs object-cover flex-shrink-0"
            />
            <span className="font-headline-md text-[17px] font-semibold tracking-tight text-on-surface hidden sm:inline">
              OmniDesk
            </span>
          </div>

          <ProfileBadge />

          <span
            title={engineLabel}
            className={
              "hidden md:inline-flex items-center space-x-1 px-2 py-0.5 rounded-full font-label-caps text-[9.5px] tracking-wide border whitespace-nowrap " +
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
            <span>{engineHealthy ? "IPC ON" : "IPC OFF"}</span>
          </span>
        </div>

        {/* 当前会话内查找（OD-ARCH-082 S8） */}
        <div className="flex-1 max-w-md mx-6 hidden sm:block">
          <div className="flex items-center plate-soft-inset rounded-full px-3.5 py-1 text-on-surface transition-all focus-within:ring-1 focus-within:ring-primary/40">
            <Icon name="search" className="w-[16px] h-[16px] text-outline mr-2 shrink-0" />
            <input
              ref={searchInputRef}
              value={searchQuery}
              onChange={(e) => onSearchQueryChange?.(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  if (e.shiftKey) {
                    onSearchPrev?.();
                  } else {
                    onSearchNext?.();
                  }
                } else if (e.key === "Escape") {
                  onSearchClear?.();
                  searchInputRef.current?.blur();
                }
              }}
              className="bg-transparent border-0 p-0 text-[13px] font-body-sm text-on-surface placeholder:text-outline-variant focus:ring-0 focus:outline-none w-full"
              placeholder="当前会话内查找"
              type="text"
              aria-label="当前会话内查找"
              data-testid="topnav-search-input"
            />
            {searchQuery.trim() ? (
              <div className="flex items-center space-x-1 pl-2 text-[11px] text-on-surface-variant font-mono shrink-0 select-none">
                <span
                  role="status"
                  aria-label={`搜索计数 ${searchMatchCount > 0 ? `${searchCurrentIndex + 1}/${searchMatchCount}` : "0/0"}`}
                  data-testid="topnav-search-count"
                >
                  {searchMatchCount > 0 ? `${searchCurrentIndex + 1}/${searchMatchCount}` : "0/0"}
                </span>
                <button
                  type="button"
                  onClick={onSearchPrev}
                  disabled={searchMatchCount === 0}
                  title="上一个 (Shift+Enter / N)"
                  data-testid="topnav-search-prev"
                  className="p-0.5 hover:text-on-surface disabled:opacity-30 transition-opacity"
                >
                  <Icon name="chevron-up" className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={onSearchNext}
                  disabled={searchMatchCount === 0}
                  title="下一个 (Enter / M)"
                  data-testid="topnav-search-next"
                  className="p-0.5 hover:text-on-surface disabled:opacity-30 transition-opacity"
                >
                  <Icon name="chevron-down" className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={onSearchClear}
                  title="清空并退出 (Esc)"
                  data-testid="topnav-search-clear"
                  className="p-0.5 hover:text-on-surface text-outline hover:text-on-surface transition-colors"
                >
                  <Icon name="x" className="w-3.5 h-3.5" />
                </button>
              </div>
            ) : (
              <kbd className="font-label-caps text-[10px] text-outline bg-white/80 px-2 py-0.5 rounded-md shadow-[0_1px_2px_rgba(0,0,0,0.05)] border border-surface-container ml-2 shrink-0">
                ⌘K
              </kbd>
            )}
          </div>
        </div>

        {/* 右侧功能簇 */}
        <div className="flex items-center space-x-2.5 sm:space-x-3 shrink-0" data-tauri-drag-region>
          <DateFlip now={now} />

          <button
            onClick={onOpenSettings}
            className="w-8 h-8 rounded-full text-on-surface-variant hover:text-on-surface hover:bg-surface-container transition-all active:scale-95 flex items-center justify-center"
            title="系统偏好设置"
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

          {/* OD-ARCH-078 S2：删掉账户头像占位块——本版本没有身份系统，
              摆一个假头像只会让人以为已经登录。 */}

          <WindowControls />
        </div>
      </nav>
    </header>
  );
}

