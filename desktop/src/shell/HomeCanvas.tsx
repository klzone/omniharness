import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { pendingWrites, shell } from "../api";
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
import type {
  OfflineFlowPack,
  PendingWriteView,
  ProfileRulesView,
  ProfileSceneEntry,
} from "../types";
import { useThreadCategories, type ThreadCategory } from "../threadCategories";
import type { Scene } from "./Dock";
import { HomeChat, type ChatMessage, type ChatPreset } from "./HomeChat";
import { SceneScaffold } from "./SceneScaffold";

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
/**
 * 规则资产里的场景规则是**多行文本**（与 Rust `SceneRules` 一致）。
 * 这里仍容忍数组形态：早期草稿与本机已存在的规则文件可能是数组，
 * 直接 .join 会在两者不一致时抛错并让整个首页白屏（OD-ARCH-068 回归）。
 */
function ruleText(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? value : null;
  }
  if (Array.isArray(value)) {
    const lines = value.filter((x): x is string => typeof x === "string" && x.trim().length > 0);
    return lines.length > 0 ? lines.join("\n") : null;
  }
  return null;
}

/**
 * 待办轮播（OD-ARCH-072）：问候语下方只显示**一条**真实待办，约 5 秒换下一条。
 *
 * 数据来自宿主登记的待确认写入（pending_write_list），只显示标题与所属场景，
 * 不展示正文。一条都没有时不占位（高度为 0），也不编造演示待办。
 * 窗口失焦/隐藏时暂停计时，避免后台无意义重渲染。
 */
const TODO_ROTATE_MS = 5000;

function TodoTicker({
  items,
  error,
  loading,
  onOpen,
}: {
  items: PendingWriteView[];
  error: string | null;
  loading: boolean;
  onOpen: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [shown, setShown] = useState(true);

  // 条数变化时回到第一条，避免索引越界显示空白。
  useEffect(() => {
    setIndex((prev) => (items.length === 0 ? 0 : Math.min(prev, items.length - 1)));
  }, [items.length]);

  useEffect(() => {
    if (items.length <= 1) return;
    let fadeTimer: number | null = null;
    const rotate = () => {
      if (document.visibilityState !== "visible" || !document.hasFocus()) return;
      setShown(false);
      fadeTimer = window.setTimeout(() => {
        setIndex((prev) => (prev + 1) % items.length);
        setShown(true);
      }, 180);
    };
    const timer = window.setInterval(rotate, TODO_ROTATE_MS);
    return () => {
      window.clearInterval(timer);
      if (fadeTimer !== null) window.clearTimeout(fadeTimer);
    };
  }, [items.length]);

  if (error) {
    return (
      <p className="text-[11.5px] text-error pb-2" role="status">
        读取本机待办失败：{error}
      </p>
    );
  }
  if (loading) {
    return <p className="text-[11.5px] text-outline pb-2">正在读取本机待办…</p>;
  }
  if (items.length === 0) return null;

  const current = items[Math.min(index, items.length - 1)];
  return (
    <div className="pb-2 min-h-[22px] flex items-center gap-2" data-testid="home-todo-ticker">
      <Icon name="checklist" className="w-[13px] h-[13px] text-primary/70 flex-shrink-0" />
      <p
        className={
          "text-[12px] text-on-surface-variant truncate transition-opacity duration-150 " +
          (shown ? "opacity-100" : "opacity-0")
        }
        title={current.title}
      >
        {current.title}
        <span className="text-outline"> · {current.scenario || "未分类"}</span>
      </p>
      {items.length > 1 ? (
        <span className="text-[10px] text-outline tabular-nums flex-shrink-0">
          {index + 1}/{items.length}
        </span>
      ) : null}
      <button
        type="button"
        onClick={onOpen}
        className="ml-auto text-[10.5px] text-primary hover:underline flex-shrink-0"
      >
        查看
      </button>
    </div>
  );
}

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

export interface HomeCanvasProps {
  onNavigate: (scene: Scene) => void;
  declaredScenes?: ProfileSceneEntry[] | null;
  searchQuery?: string;
  searchCurrentIndex?: number;
  onSearchMatchesChange?: (count: number) => void;
}

export function HomeCanvas({
  onNavigate,
  declaredScenes = null,
  searchQuery = "",
  searchCurrentIndex = 0,
  onSearchMatchesChange,
}: HomeCanvasProps) {
  const now = new Date();

  /**
   * 待办（OD-ARCH-072）：问候语下方轮播的真实条目。
   *
   * 来源是宿主登记的待确认写入（只读列表，读不到正文），不调模型、不编造待办。
   * 读取失败如实显示原因，不当作「没有待办」。
   */
  const [todos, setTodos] = useState<PendingWriteView[]>([]);
  const [todoLoading, setTodoLoading] = useState(true);
  const [todoError, setTodoError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    pendingWrites
      .list()
      .then((r) => {
        if (!alive) return;
        setTodos((r.items ?? []).filter((i) => i.status === "pending"));
        setTodoError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setTodos([]);
        setTodoError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (alive) setTodoLoading(false);
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
  /**
   * Profile 规则资产（OD-ARCH-068）：场景规则/章节/思考档与写作铁律的唯一来源。
   * 没加载 = 纯净底座（不输出铁律、不套场景规则）；读取失败也要如实告知，
   * 不能悄悄按“没有规则”使用。
   */
  const [rulesView, setRulesView] = useState<ProfileRulesView | null>(null);
  /** 已补建但还没进入 state 的线程号（同一 tick 内多次回调要用同一个）。 */
  const pendingIdRef = useRef<string | null>(null);

  useEffect(() => {
    shell
      .offlineFlowPack()
      .then((r) => setPack(r.ok ? r.pack : null))
      .catch(() => setPack(null));
  }, []);

  // 每次回到首页都重读一次规则资产：设置页保存后无需重启，回来即生效。
  useEffect(() => {
    let alive = true;
    shell
      .profileRules()
      .then((r) => {
        if (alive) setRulesView(r);
      })
      .catch(() => {
        if (alive) setRulesView(null);
      });
    return () => {
      alive = false;
    };
  }, []);

  const refresh = useCallback(() => setThreads(listThreads()), []);

  // 从场景页「去对话」过来：一次性请求槽决定是打开旧对话还是新建。
  // OD-ARCH-091：请求可以带一句预填草稿（例如「整理成周报」），只放进输入框，不自动发送。
  const [initialInput, setInitialInput] = useState<string>("");
  useEffect(() => {
    const req = takePendingThreadRequest();
    if (req.open) {
      const t = listThreads().find((x) => x.id === req.open);
      if (t) {
        setActiveId(t.id);
        setActiveTag(t.tag);
        setViewKey(t.id);
        setInitialInput("");
      }
    } else if (req.create) {
      const t = createThread({ tag: req.create, preset: req.create });
      setActiveId(t.id);
      setActiveTag(req.create);
      setViewKey(t.id);
      setInitialInput(req.prompt ?? "");
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
    // Profile 规则优先，流程包只作缺省回退（OD-ARCH-068）。
    const scene = rulesView?.rules?.scenes?.[def.preset] ?? null;
    const bound = rulesView?.rules?.skill_bindings?.[def.preset];
    const maxInjected = bound?.max_injected ?? 3;
    const boundSkillIds = (bound?.skills ?? []).slice(0, maxInjected);
    const boundSkills = boundSkillIds
      .map((id) => ({ id, content: rulesView?.skill_contents?.[id] ?? "" }))
      .filter((s) => Boolean(s.content));

    return {
      key: def.preset,
      label: def.label,
      rules: scene ? ruleText(scene.rules) : flow?.draft?.rules ?? null,
      sections: scene ? ruleText(scene.sections) : flow?.draft?.sections ?? null,
      writingRules: rulesView?.rules?.global?.writing_rules ?? null,
      thinking: scene?.thinking ?? null,
      rulesLoaded: Boolean(rulesView?.loaded),
      skills: boundSkills.length > 0 ? boundSkills : null,
    };
  }, [effectiveTag, pack, rulesView]);

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
    // 手动新开对话不带任何预填草稿（OD-ARCH-091）。
    setInitialInput("");
    pendingIdRef.current = null;
    refresh();
  }, [activeTag, refresh]);

  /**
   * 分支到新对话（OD-ARCH-079）：引擎已在 DSH 侧建好子会话，这里新建一条同一场景的线程，
   * 只把截至切点的消息带过去用于展示，并保存子会话号；后续消息直接续接子会话，不重放历史。
   */
  const branchThread = useCallback(
    (carried: ChatMessage[], childSessionId: string) => {
      const t = createThread({ tag: activeTag, preset: isSceneKey(activeTag) ? activeTag : "plain" });
      saveThreadMessages(t.id, carried);
      setThreadEngineSession(t.id, childSessionId);
      // 先刷新线程列表，切过去时 active 能立刻带上子会话号与消息。
      refresh();
      setActiveId(t.id);
      setViewKey(t.id);
      setInitialInput("");
      pendingIdRef.current = null;
    },
    [activeTag, refresh],
  );

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
      {/*
        上部（OD-ARCH-072）：只有问候 + 一条待办。
        「今日重点推进」卡片与常驻日程卡已移除；日程改到顶栏日期悬浮卡，
        待办改成问候语下方单条轮播，无待办时不占位。
      */}
      <section className="flex flex-col flex-shrink-0 pt-1 pb-1 mb-1">
        <div className="pb-1 flex flex-col sm:flex-row sm:items-baseline justify-between gap-2">
          <h1 className="font-display-hero text-2xl sm:text-3xl font-light text-on-surface tracking-tight">
            {greeting(now)}
            <span className="text-on-surface-variant font-light text-base ml-3 tracking-normal">
              · 欢迎回到 OmniDesk
            </span>
          </h1>
        </div>
        <TodoTicker
          items={todos}
          error={todoError}
          loading={todoLoading}
          onOpen={() => onNavigate("sessions")}
        />
      </section>
        </>
      }
      chat={
        /* 下部：对话（全宽）。OD-ARCH-078 S2：去掉左侧吉祥物图片，
           顶部「晚上好 · 欢迎回到 OmniDesk」已经是常驻问候，不必再放一张装饰图。 */
        <div className="flex flex-1 min-h-0 gap-3">
          <div className="flex-1 min-w-0 flex flex-col min-h-0">
        <HomeChat
          key={viewKey}
          onNavigate={onNavigate}
          preset={preset}
          sceneOptions={options}
          activeTag={effectiveTag}
          onChangeTag={changeTag}
          onNewThread={newThread}
          onBranchNewThread={branchThread}
          threadKey={viewKey}
          initialMessages={active?.messages ?? []}
          initialInput={initialInput}
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
          searchQuery={searchQuery}
          searchCurrentIndex={searchCurrentIndex}
          onSearchMatchesChange={onSearchMatchesChange}
        />
          </div>
        </div>
      }
    />
  );
}
