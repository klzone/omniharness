import { useCallback, useEffect, useRef, useState } from "react";
import { onProfilePlugins, onProfileRegistryChanged, shell } from "../api";
import { Icon } from "../components/Icon";
import type {
  ProfileChoiceOption,
  ProfileChoiceView,
  ProfileMissingPlugin,
} from "../types";
import { ThreeWeekCalendar } from "./ThreeWeekCalendar";
import { WindowControls } from "./WindowChrome";

/**
 * 顶栏（OD-ARCH-072 首页改版）。
 *
 * 三处结构变化：
 * 1. 左侧品牌固定为软件名「OmniDesk」；后面的 Profile 徽标是**独立**控件，
 *    决定「用哪个 Profile 作为工作台」，切换即换引擎（OD-ARCH-110），不再顶替品牌名。
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
              data-hover-hint="日程同步未实现：这里只按本机日期给出时间骨架"
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
 * 选择保存一个 Profile 名到应用数据根，并**立即停掉旧引擎**（OD-ARCH-110）：
 * 下一条任务按新 Profile 启动，场景声明、规则与注入 Skill 随之一并切换。
 * 结构上刻意与软件名分开：品牌永远是 OmniDesk，Profile 只是运行配置。
 */
function ProfileBadge({ onProfileChanged }: { onProfileChanged?: () => void }) {
  const [view, setView] = useState<ProfileChoiceView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<ProfileChoiceOption | null>(null);
  const [saving, setSaving] = useState(false);
  const [receipt, setReceipt] = useState<string | null>(null);
  // 新建自定义 Profile（OD-ARCH-109 补充）：默认折叠，不挤占选择列表。
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [copyFrom, setCopyFrom] = useState("");
  // 切换后仍未就绪的插件（OD-ARCH-114）：切换不静默联网，缺什么摆到台面上。
  const [missing, setMissing] = useState<{
    profile: string;
    label: string;
    items: ProfileMissingPlugin[];
  } | null>(null);
  const [installing, setInstalling] = useState(false);
  const [installNote, setInstallNote] = useState<string | null>(null);
  // 后台自动补装进行中（OD-ARCH-121）：切换回执先秒回，装完再由事件收尾。
  const [autofill, setAutofill] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  // 事件回调只在挂载时订阅一次，读当前清单要靠 ref（OD-ARCH-124）。
  // 冷启动自愈发生在订阅之后、用户还没开过弹层，闭包里的 view 会一直是 null。
  const viewRef = useRef<ProfileChoiceView | null>(null);

  useEffect(() => {
    viewRef.current = view;
  }, [view]);

  /**
   * 后台补装结束（OD-ARCH-121）。
   *
   * 只处理「自动」事件；手动点「安装」走命令回执，不走这里。
   * 装齐 → 收起缺件卡并说明结果；仍缺 → 用最新探测结果刷新列表，保留安装入口。
   */
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;
    onProfilePlugins((ev) => {
      if (!alive || !ev.automatic) return;
      setAutofill(false);
      if (ev.error) {
        setInstallNote("自动补齐未执行：" + ev.error);
        return;
      }
      const failed = ev.results.filter((x) => !x.ok).length;
      if (ev.ok && ev.still_missing.length === 0) {
        setMissing(null);
        setInstallNote(null);
        setReceipt(
          ev.attempted.length > 0
            ? `已在后台补齐 ${ev.attempted.length} 个插件。`
            : "插件已就绪。",
        );
        return;
      }
      setMissing((prev) => {
        const items =
          ev.still_missing.length > 0
            ? ev.still_missing.map((p) => ({
                package: p.package,
                status: "missing" as const,
                detail: p.detail,
              }))
            : prev?.items ?? [];
        if (prev) return { ...prev, items };
        if (items.length === 0) return prev;
        // 冷启动补装失败：此前没有切换回执，卡片得从空状态建出来，
        // 否则用户只看到一句「仍有 N 个未就绪」，却看不到缺的是哪几个。
        const known = viewRef.current;
        const label =
          known?.options.find((o) => o.name === ev.profile)?.label ||
          known?.custom?.find((c) => c.name === ev.profile)?.label ||
          ev.profile;
        return { profile: ev.profile, label, items };
      });
      if (ev.still_missing.length > 0) {
        setReceipt(`「${ev.profile}」还有 ${ev.still_missing.length} 个插件未就绪。`);
      }
      setInstallNote(
        `后台已尝试 ${ev.attempted.length} 个，仍有 ${ev.still_missing.length} 个未就绪` +
          (failed > 0 ? `（${failed} 项命令失败）` : "") +
          "。可稍后重试。",
      );
    })
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // 订阅失败不影响手动安装入口，静默即可。
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

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

  // 清单变了就重读（OD-ARCH-125）：设置页里移除或从回收站恢复后，
  // 顶栏原来只在挂载时拉一次，用户回到顶栏看到的还是旧清单，切不过去。
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;
    onProfileRegistryChanged(() => {
      if (alive) load();
    })
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => {
        /* 订阅失败不影响手动打开弹层时的那次读取 */
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [load]);

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
    // 自建 Profile 优先显示登记的中文名，徽标本身仍保持短。
    return view.custom?.find((c) => c.name === view.saved)?.label || view.saved;
  })();

  const confirm = () => {
    if (!pending) return;
    const target = pending.standard ? null : pending.name;
    setSaving(true);
    shell
      .profileChoiceSet(target)
      .then((r) => {
        // 切换即装配（OD-ARCH-109）：回执要说清补了什么，而不是只说「已保存」。
        const assembly = r.assembly;
        const changes = r.assembled_changes ?? 0;
        const detail = assembly
          ? assembly.standard
            ? "基础标准：不写入任何资产。"
            : changes > 0
              ? `已装配 ${changes} 项（共 ${assembly.items.length} 项）`
              : `资产已就绪（${assembly.items.length} 项）`
          : "";
        // 已在后台自动补装（OD-ARCH-121）：明确说出来，避免用户以为「缺了就不管」。
        const auto = r.plugins_auto_installing === true;
        setReceipt(
          [r.note, detail, auto ? "缺失插件已在后台自动补齐，完成后会提示结果。" : ""]
            .filter(Boolean)
            .join(" "),
        );
        setAutofill(auto);
        // 缺什么都摆出来：自动补齐时这张卡改成进度说明，失败后仍由用户点「安装」。
        const gaps = r.plugins_missing ?? [];
        setMissing(
          gaps.length > 0
            ? { profile: pending.name, label: pending.label, items: gaps }
            : null,
        );
        setInstallNote(null);
        // 切换已生效：让 App 重读场景声明，Dock 与场景入口立刻换到新 Profile。
        if (r.engine_stopped !== false) onProfileChanged?.();
        setPending(null);
        // 还缺插件时留在弹层里，把「装什么」直接给出来；齐了就关。
        setOpen(gaps.length > 0);
        load();
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setSaving(false));
  };

  /** 一次补齐缺的插件：逐项回执，失败不中断其余，安装后重新探测。 */
  const ensurePlugins = () => {
    if (!missing || installing) return;
    setInstalling(true);
    setInstallNote(null);
    shell
      .profilePluginsEnsure(missing.profile)
      .then((r) => {
        const failed = r.results.filter((x) => !x.ok).length;
        if (r.attempted.length === 0) {
          setInstallNote("现在没有需要安装的插件。");
        } else if (r.ok) {
          setInstallNote(`已装齐 ${r.attempted.length} 个插件。`);
        } else {
          setInstallNote(
            `已尝试 ${r.attempted.length} 个，仍有 ${r.still_missing.length} 个未就绪` +
              (failed > 0 ? `（${failed} 项安装命令失败）` : "") +
              "。",
          );
        }
        if (r.ok || r.still_missing.length === 0) {
          setMissing(null);
        } else if (missing) {
          setMissing({
            ...missing,
            items: r.still_missing.map((p) => ({
              package: p.package,
              status: "missing" as const,
              detail: p.detail,
            })),
          });
        }
      })
      .catch((e) =>
        setInstallNote(e instanceof Error ? e.message : String(e)),
      )
      .finally(() => setInstalling(false));
  };

  /** 新建 / 复制：成功后刷新列表，并把新 Profile 直接列为待切换项。 */
  const createProfile = () => {
    const name = newName.trim().toLowerCase();
    if (!name) {
      setError("先填一个 Profile 名（小写字母、数字、连字符、下划线）。");
      return;
    }
    setSaving(true);
    setError(null);
    shell
      .profileCreate(name, newLabel.trim() || name, copyFrom || null)
      .then((r) => {
        setReceipt(r.note);
        setCreating(false);
        setNewName("");
        setNewLabel("");
        setCopyFrom("");
        setPending({
          name: r.profile.name,
          label: r.profile.label,
          standard: false,
        });
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
        data-hover-hint={"引擎 Profile：" + currentLabel + "（切换即生效：旧引擎停止，下一条任务按新 Profile 运行）"}
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
          {receipt ? (
            <p
              className="mx-1 mb-2 px-2 py-1.5 text-[10.5px] leading-relaxed text-emerald-700 bg-emerald-500/8 ring-1 ring-emerald-500/20 break-words"
              role="status"
              aria-label={receipt}
            >
              {receipt}
            </p>
          ) : null}
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
                Profile 决定引擎加载哪些插件、场景与规则。确认后
                <span className="font-medium text-on-surface">立即切换</span>
                ：旧引擎会被停止，正在生成的内容随之中断，下一条任务按新 Profile 运行。
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
                  {saving ? "切换中…" : "确认"}
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="px-1.5 pb-1.5 text-[10px] uppercase tracking-wide text-outline">
                切换后立即生效 · 当前：{currentLabel}
              </div>
              {view?.saved_out_of_whitelist ? (
                <div className="mx-1 mb-2 p-2 rounded-lg bg-amber-500/10 border border-amber-500/30 text-[11px] text-amber-900 leading-relaxed">
                  当前保存的 Profile 不在可选清单，本次使用默认。
                </div>
              ) : null}
              {missing ? (
                <div
                  className="mx-1 mb-2 border border-outline-variant/40 bg-surface-container/50 p-2"
                  data-testid="topnav-profile-missing-plugins"
                >
                  <p className="text-[11px] font-medium text-on-surface">
                    「{missing.label}」还缺 {missing.items.length} 个插件
                  </p>
                  <p className="mt-0.5 text-[10px] text-on-surface-variant leading-relaxed">
                    {autofill
                      ? "配置与 Skill 已写入；这些包正在后台联网补齐，装完会在这里更新结果。"
                      : "配置与 Skill 已写入，这些包还没装上。装齐后该 Profile 的插件能力才完整。"}
                  </p>
                  <ul className="mt-1.5 space-y-0.5">
                    {missing.items.map((p) => (
                      <li
                        key={p.package}
                        className="flex items-baseline gap-1.5 text-[10px] text-outline"
                      >
                        <span className="font-mono text-on-surface-variant truncate">
                          {p.package}
                        </span>
                        <span className="flex-shrink-0">{p.status}</span>
                      </li>
                    ))}
                  </ul>
                  <div className="mt-2 flex items-center gap-2">
                    <button
                      type="button"
                      onClick={ensurePlugins}
                      disabled={installing || autofill}
                      data-testid="topnav-profile-install-missing"
                      data-hover-hint="会联网，通过 DSH 官方插件通道装到当前 Profile"
                      className="px-2.5 py-1 bg-primary text-on-primary text-[11px] font-medium disabled:opacity-50"
                    >
                      {autofill ? "后台补齐中…" : installing ? "安装中…" : "安装缺失插件"}
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setMissing(null);
                        setInstallNote(null);
                      }}
                      disabled={installing}
                      className="px-2 py-1 text-[11px] text-on-surface-variant hover:bg-surface-container disabled:opacity-50"
                    >
                      稍后
                    </button>
                  </div>
                  {installNote ? (
                    <p className="mt-1.5 text-[10px] text-on-surface-variant leading-relaxed">
                      {installNote}
                    </p>
                  ) : null}
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
                  ? "当前生效：" + view.effective + "（" + view.channel + " 通道）"
                  : "读取隔离数据根失败时不会显示可选清单。"}
              </p>
              {error && view ? (
                <p className="px-1.5 pt-1 text-[10px] text-error leading-relaxed">{error}</p>
              ) : null}

              {/* 自建 Profile：默认折叠，展开才占地方 */}
              <div className="mt-1.5 pt-1.5 border-t border-surface-variant/20">
                {creating ? (
                  <div className="space-y-1.5 px-1" data-testid="topnav-profile-create-form">
                    <input
                      className="w-full rounded-lg border border-outline-variant/40 bg-white px-2 py-1 text-[11.5px] outline-none focus:border-primary"
                      placeholder="名字，如 my-work"
                      value={newName}
                      onChange={(e) => setNewName(e.target.value)}
                      data-testid="topnav-profile-new-name"
                      aria-label="新 Profile 名字"
                    />
                    <input
                      className="w-full rounded-lg border border-outline-variant/40 bg-white px-2 py-1 text-[11.5px] outline-none focus:border-primary"
                      placeholder="显示名（可选），如 我的工作台"
                      value={newLabel}
                      onChange={(e) => setNewLabel(e.target.value)}
                      data-testid="topnav-profile-new-label"
                      aria-label="新 Profile 显示名"
                    />
                    <select
                      className="w-full rounded-lg border border-outline-variant/40 bg-white px-2 py-1 text-[11.5px] outline-none focus:border-primary"
                      value={copyFrom}
                      onChange={(e) => setCopyFrom(e.target.value)}
                      data-testid="topnav-profile-copy-from"
                      aria-label="从哪个 Profile 复制"
                    >
                      <option value="">从基础标准起手（干净底座）</option>
                      {(view?.options ?? [])
                        .filter((o) => o.name !== view?.standard)
                        .map((o) => (
                          <option key={o.name} value={o.name}>
                            复制：{o.label}
                          </option>
                        ))}
                    </select>
                    <p className="text-[10px] text-outline leading-relaxed">
                      只带配置与场景规则，不带已装插件；切过去后在设置 →「Profile 资产」里按需安装。
                    </p>
                    <div className="flex items-center justify-end gap-2">
                      <button
                        type="button"
                        onClick={() => {
                          setCreating(false);
                          setError(null);
                        }}
                        disabled={saving}
                        className="px-2.5 py-1 rounded-full text-[11px] text-on-surface-variant hover:bg-surface-container disabled:opacity-50"
                      >
                        取消
                      </button>
                      <button
                        type="button"
                        onClick={createProfile}
                        disabled={saving}
                        data-testid="topnav-profile-create-submit"
                        className="px-2.5 py-1 rounded-full bg-primary text-on-primary text-[11px] font-medium disabled:opacity-50"
                      >
                        {saving ? "创建中…" : "创建"}
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => {
                      setCreating(true);
                      setError(null);
                    }}
                    data-testid="topnav-profile-create-toggle"
                    className="w-full text-left px-2 py-1 rounded-lg text-[11px] text-primary hover:bg-surface-container"
                  >
                    ＋ 新建自定义 Profile
                  </button>
                )}
              </div>
            </>
          )}
        </div>
      ) : null}

      {/* 弹层关着时（插件齐全）才在徽标下方摆回执；弹层开着时回执在弹层里，
          避免两条文字互相压住、还被窗口右边缘裁掉（OD-ARCH-121 实测）。 */}
      {receipt && !open ? (
        <p
          className="absolute left-0 top-full mt-1.5 text-[10px] text-emerald-600 whitespace-nowrap z-40"
          role="status"
          aria-label={receipt}
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
  /** 顶栏切换 Profile 成功后的回调（OD-ARCH-110）：让 App 重读场景声明。 */
  onProfileChanged?: () => void;
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
  onProfileChanged,
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

          <ProfileBadge onProfileChanged={onProfileChanged} />

          <span
            data-hover-hint={engineLabel}
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
                  if (searchMatchCount === 0) return; // 0 命中时为空操作
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
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={onSearchPrev}
                  disabled={searchMatchCount === 0}
                  aria-label="上一处匹配 (Shift+Enter)"
                  data-testid="topnav-search-prev"
                  className="p-0.5 hover:text-on-surface disabled:opacity-30 transition-opacity"
                >
                  <Icon name="chevron-up" className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={onSearchNext}
                  disabled={searchMatchCount === 0}
                  aria-label="下一处匹配 (Enter)"
                  data-testid="topnav-search-next"
                  className="p-0.5 hover:text-on-surface disabled:opacity-30 transition-opacity"
                >
                  <Icon name="chevron-down" className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={onSearchClear}
                  aria-label="清空并退出搜索 (Esc)"
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
            data-hover-hint="系统偏好设置"
            aria-label="系统偏好设置"
          >
            <Icon name="settings" className="w-[19px] h-[19px]" />
          </button>

          <button
            disabled
            className="w-8 h-8 rounded-full text-on-surface-variant/40 flex items-center justify-center cursor-not-allowed"
            data-hover-hint="未实现：通知与待办"
            aria-label="未实现：通知与待办"
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

