import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { shell } from "../api";
import { Icon } from "../components/Icon";
import { SCENES, isSceneKey } from "../scenes";
import {
  createThread,
  listThreads,
  retagThread,
  saveThreadMessages,
  setThreadEngineSession,
  takePendingThreadRequest,
  type SceneThread,
  type ThreadTag,
} from "../sceneThreads";
import type { OfflineFlowPack, ProfileSceneEntry } from "../types";
import { useThreadCategories, type ThreadCategory } from "../threadCategories";
import type { Scene } from "./Dock";
import { HomeChat, type ChatPreset } from "./HomeChat";
import { SceneScaffold } from "./SceneScaffold";
import { ThreeWeekCalendar } from "./ThreeWeekCalendar";

/**
 * 首页画布（OD-ARCH-026 首页版式回归：对话为主 + 顶部三模块）。
 *
 * 版式来自 `_design/stitch_omnidesk`（只读设计原件）：
 *   顶部问候 → 一行三模块（机器人 + 今日重点推进 + 日程节律）→ 下半屏整幅对话。
 *
 * 与设计稿的**刻意差异**（诚实性 / 性能约束）：
 * - 「今日重点推进」（OD-ARCH-060 S5）只显示宿主聚合出来的真实计数（待确认/逾期），
 *   没有数据就写「今天没有要处理的」，读不到就如实报错；不展示演示性待办。
 * - 「日程节律」复用三周日历：只按本机日期给出已过/今天/将来的骨架，徽标「未同步」。
 * - **默认静态**（OD-ARCH-027 Step 2/4）：不使用浮动吉祥物动画、脉冲点动画，
 *   卡片也不用背景模糊；层次由半透明底色 + 描边 + 阴影表达。要加动效须另行批准。
 * - 不显示右上角的装饰性状态串（「OMNIDESK · LOCAL FIRST」等），此处只保留真实信息。
 */
function greeting(date: Date): string {
  const h = date.getHours();
  if (h < 6) return "夜深了";
  if (h < 12) return "早上好";
  if (h < 14) return "中午好";
  if (h < 18) return "下午好";
  return "晚上好";
}

/**
 * 对话归类的选项（OD-ARCH-061 S1，次序与显示名改由宿主配置决定，OD-ARCH-062 S1）。
 *
 * 声明里隐藏了某个业务场景时，对应的内置分类不出现；「未分类」与自定义分类
 * 不受声明影响（自定义分类只进归类下拉，不加 Dock 入口）。
 */
function sceneOptions(
  categories: ThreadCategory[],
  declared: ProfileSceneEntry[] | null | undefined,
): Array<{ key: ThreadTag; label: string }> {
  const hidden: Record<string, string> = {
    weekly: "report",
    writing: "writing",
    project: "project",
  };
  const visible = declared ? new Set(declared.map((d) => d.id as string)) : null;
  return categories
    .filter((c) => {
      const declId = hidden[c.key];
      return !(visible && declId && !visible.has(declId));
    })
    .map((c) => ({ key: c.key, label: c.label }));
}

export function HomeCanvas({
  onNavigate,
  declaredScenes = null,
}: {
  onNavigate: (scene: Scene) => void;
  declaredScenes?: ProfileSceneEntry[] | null;
}) {
  const now = new Date();

  /**
   * 「今日重点推进」的数据（OD-ARCH-060 S5）：跨场景待确认 + 逾期汇总。
   * 来自宿主的 `scene_overview` 聚合命令（数已落盘记录），不调模型、不编造待办。
   * 读取失败时如实显示失败原因，不当作「今天没有要处理的」。
   */
  const [today, setToday] = useState<{ pending: number; expired: number; broken: number } | null>(null);
  const [todayError, setTodayError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    shell
      .sceneOverview()
      .then((r) => {
        if (!alive) return;
        const items = r.items ?? [];
        setToday({
          pending: items.reduce((n, i) => n + i.pending, 0),
          expired: items.reduce((n, i) => n + i.pending_expired, 0),
          broken: items.reduce((n, i) => n + i.pending_broken, 0),
        });
        setTodayError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setToday(null);
        setTodayError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, []);

  // 对话中枢（OD-ARCH-057）：整个应用只有这里一个对话框。场景、历史对话都在这里选。
  const [activeTag, setActiveTag] = useState<ThreadTag>("plain");
  const [threads, setThreads] = useState<SceneThread[]>(() => listThreads());
  const [activeId, setActiveId] = useState<string | null>(() => listThreads()[0]?.id ?? null);
  /**
   * 对话组件的挂载键：只在**用户主动切换**（选旧对话 / 新建 / 从场景页过来）时变。
   * 首轮消息后自动补建线程时不改它，否则会重挂组件、把刚发出的对话清空。
   */
  const [viewKey, setViewKey] = useState<string>(() => listThreads()[0]?.id ?? "new");
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);
  /** 已补建但还没进入 state 的线程号（同一 tick 内多次回调要用同一个）。 */
  const pendingIdRef = useRef<string | null>(null);

  useEffect(() => {
    shell
      .offlineFlowPack()
      .then((r) => setPack(r.ok ? r.pack : null))
      .catch(() => setPack(null));
  }, []);

  const refresh = useCallback(() => setThreads(listThreads()), []);

  // 从场景页「去对话」过来：一次性请求槽决定是打开旧对话还是新建。
  useEffect(() => {
    const req = takePendingThreadRequest();
    if (req.open) {
      const t = listThreads().find((x) => x.id === req.open);
      if (t) {
        setActiveId(t.id);
        setActiveTag(t.tag);
        setViewKey(t.id);
      }
    } else if (req.create) {
      const t = createThread({ tag: req.create, preset: req.create });
      setActiveId(t.id);
      setActiveTag(req.create);
      setViewKey(t.id);
    }
    refresh();
  }, [refresh]);

  const active = activeId ? threads.find((t) => t.id === activeId) ?? null : null;
  const effectiveTag: ThreadTag = active ? active.tag : activeTag;

  const preset: ChatPreset | null = useMemo(() => {
    // 只有三个业务场景带开场预设；未分类与自定义分类都按普通对话进行（OD-ARCH-062 S1）。
    if (!isSceneKey(effectiveTag)) return null;
    const def = SCENES[effectiveTag];
    const flow = pack ? pack[def.preset] : null;
    return {
      key: def.preset,
      label: def.label,
      rules: flow?.draft?.rules ?? null,
      sections: flow?.draft?.sections ?? null,
    };
  }, [effectiveTag, pack]);

  const categories = useThreadCategories();
  const options = useMemo(
    () => sceneOptions(categories, declaredScenes),
    [categories, declaredScenes],
  );

  /** 改本段对话的场景标签：已有对话直接改挂（复用 retagThread），新对话先建再挂。 */
  const changeTag = useCallback(
    (tag: string) => {
      const next = tag as ThreadTag;
      setActiveTag(next);
      if (!activeId) return;
      retagThread(activeId, next);
      refresh();
    },
    [activeId, refresh],
  );

  const newThread = useCallback(() => {
    // 自定义分类只记归属，开场预设仍按普通对话（不改场景规则）。
    const t = createThread({ tag: activeTag, preset: isSceneKey(activeTag) ? activeTag : "plain" });
    setActiveId(t.id);
    setViewKey(t.id);
    pendingIdRef.current = null;
    refresh();
  }, [activeTag, refresh]);

  // 第一轮消息到达且还没有线程记录时补建一条，避免「聊完没进档案」。
  const ensureThread = useCallback(() => {
    if (activeId) return activeId;
    if (pendingIdRef.current) return pendingIdRef.current;
    const t = createThread({ tag: activeTag, preset: isSceneKey(activeTag) ? activeTag : "plain" });
    pendingIdRef.current = t.id;
    setActiveId(t.id);
    refresh();
    return t.id;
  }, [activeId, activeTag, refresh]);

  return (
    <SceneScaffold
      upper={
        <>
      {/* 上部：问候 + 一行三模块（收进悬停面板，把纵向空间让给对话） */}
      <section className="flex flex-col flex-shrink-0 pt-1 pb-1 mb-1">
        <div className="pb-3 flex flex-col sm:flex-row sm:items-baseline justify-between gap-2">
          <h1 className="font-display-hero text-2xl sm:text-3xl font-light text-on-surface tracking-tight">
            {greeting(now)}
            <span className="text-on-surface-variant font-light text-base ml-3 tracking-normal">
              · 欢迎回到 OmniDesk
            </span>
          </h1>
        </div>

        <div className="relative flex flex-col lg:flex-row items-stretch gap-4">
          {/* 左：机器人（本地透明背景资源），独立一列、底部对齐 */}
          <div className="hidden lg:flex w-[112px] shrink-0 flex-col items-center justify-end">
            <img
              src="/mascot.png"
              alt="OmniDesk 吉祥物"
              className="w-[112px] h-auto drop-shadow-[0_16px_24px_rgba(79,70,229,0.18)]"
            />
          </div>

          {/* 中：今日重点推进（诚实空态，保留版式） */}
          <div className="flex-1 bg-white/80 p-5 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] rounded flex flex-col justify-between min-h-[220px]">
            <div className="flex items-center justify-between pb-2.5 border-b border-surface-variant/20">
              <div className="flex items-center space-x-2">
                <div className="w-6 h-6 rounded-full bg-primary/10 text-primary flex items-center justify-center">
                  <Icon name="checklist" className="w-[15px] h-[15px]" />
                </div>
                <h2 className="font-headline-md text-[13px] font-semibold text-on-surface">
                  今日重点推进
                </h2>
                {today && today.pending + today.expired > 0 ? (
                  <span className="px-1.5 py-0.5 rounded-full bg-primary/10 text-primary font-label-caps text-[9px]">
                    {today.pending + today.expired} 条待处理
                  </span>
                ) : null}
              </div>
            </div>
            <div className="flex-1 flex flex-col items-center justify-center text-center gap-3 py-5">
              <div className="w-11 h-11 rounded-full bg-surface-container-low flex items-center justify-center text-outline">
                <Icon name="history" className="w-5 h-5" />
              </div>
              {todayError ? (
                <p className="text-[13px] text-error max-w-md leading-relaxed">
                  读取本机状态失败：{todayError}。这不等于「今天没有要处理的」。
                </p>
              ) : today === null ? (
                <p className="text-[13px] text-outline max-w-md leading-relaxed">正在读取本机状态…</p>
              ) : today.pending + today.expired === 0 ? (
                <p className="text-[13px] text-on-surface-variant max-w-md leading-relaxed">
                  今天没有要处理的。
                  {today.broken > 0 ? `另有 ${today.broken} 条待确认记录无法解析（见场景页）。` : ""}
                </p>
              ) : (
                <p className="text-[13px] text-on-surface-variant max-w-md leading-relaxed">
                  有 {today.pending} 条待确认写入等你决定
                  {today.expired > 0 ? `，${today.expired} 条已过期` : ""}。
                  打开对应场景页，在顶部状态条点「待确认写入」查看。
                </p>
              )}
              {/* 任务工作台已下线（OD-ARCH-061 S4）：同一份计数在场景页顶部状态条里可看。 */}
            </div>
          </div>

          {/* 右：日程节律（三周日期骨架，不显示业务内容） */}
          <div className="w-full lg:w-[330px] flex-shrink-0 bg-white/80 p-3.5 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] rounded flex flex-col justify-between min-h-[220px]">
            <div className="flex items-center justify-between pb-1.5 border-b border-surface-variant/20">
              <div className="flex items-center space-x-1.5">
                <div className="w-5 h-5 rounded-full bg-secondary-fixed/50 text-secondary flex items-center justify-center">
                  <Icon name="calendar" className="w-[13px] h-[13px]" />
                </div>
                <span className="font-headline-md text-[12px] font-semibold text-on-surface">
                  日程节律
                </span>
              </div>
              <span
                className="inline-flex items-center px-1.5 py-0.5 rounded-full bg-surface-container text-outline font-label-caps text-[9px]"
                title="日程同步未实现"
              >
                未同步
              </span>
            </div>

            <div className="py-1.5 w-full">
              <ThreeWeekCalendar now={now} />
            </div>
          </div>
        </div>
      </section>
        </>
      }
      chat={
        /* 下部：整幅对话（全宽），消息区顶部向上渐隐由 chat-fade-mask 提供 */
        <HomeChat
          key={viewKey}
          onNavigate={onNavigate}
          preset={preset}
          sceneOptions={options}
          activeTag={effectiveTag}
          onChangeTag={changeTag}
          onNewThread={newThread}
          threadKey={viewKey}
          initialMessages={active?.messages ?? []}
          engineSessionId={active?.engineSessionId ?? null}
          onEngineSessionId={(sid) => {
            const id = ensureThread();
            setThreadEngineSession(id, sid);
            refresh();
          }}
          onMessagesChange={(messages) => {
            // 空白新对话不外建档：只在本线程已有内容时才落盘。
            if (!activeId && messages.length === 0) return;
            const id = ensureThread();
            saveThreadMessages(id, messages);
            refresh();
          }}
        />
      }
    />
  );
}
