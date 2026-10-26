import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { host, invalidateEngineFactsCache, onTaskEvent, shell } from "../api";
import {
  ATTACHMENT_MAX_FILES,
  activeMentionQuery,
  applyMentionPick,
  attachmentScanText,
  checkAttachments,
  composeWithAttachments,
  type AttachmentInput,
} from "../attachmentPrompt";
import { composeConversationPrompt } from "../conversationPrompt";
import { currentEngineSubmitDecision } from "../engineAdmission";
import { describeWorkIntent, detectWorkIntent, type WorkIntent } from "../workIntent";
import {
  describeEngineCancelOutcome,
  engineCancelButtonState,
  runEngineCancelRequest,
} from "../engineCancelModel";
import {
  TERMINAL_STATES,
  type ApprovalRequest,
  type AdmissionFacts,
  type CancelOutcome,
  type QuestionAnswerItem,
  type QuestionRequest,
  type ReferenceCandidate,
  type ReferenceRoot,
  type TaskState,
  type TaskStatus,
} from "../types";
import { Icon } from "../components/Icon";
import { BrandMark } from "../components/BrandMark";
import { CategoryPicker } from "./CategoryPicker";
import { ModelPicker } from "./ModelPicker";
import { TurnRail } from "./TurnRail";
import { MarkdownText } from "./MarkdownText";
import { QuestionCard } from "./QuestionCard";
import { SCENES, isSceneKey } from "../scenes";
import type { EngineModelCatalog } from "../types";
import { useChatInputFocus } from "./SceneScaffold";
import type { Scene } from "./Dock";

/**
 * 首页 AI 对话（OD-ARCH-026）—— 首页的主要工作入口。
 *
 * 与 UI v2 候选实现的关键差别：这里**不再**用一个恒为 false 的前端常量把提交永久关闭，
 * 而是以宿主报告的准入事实为准（`engine_admission_status`）：
 *   * 宿主在任务提交时强制执行权限判定；首次使用严格关闭，用户可在设置中逐项开启；
 *   * 界面只显示宿主回报的真实类别与已开启项，不自行复制一套会与宿主不一致的开关；
 *   * 未开启时提交按钮关闭并说明原因，不把本地文案伪装成模型输出。
 *
 * 诚实性约束：
 *   * 助手输出一律标注为「模型输出 · 未复核」；
 *   * 状态轮询与结果读取失败都有可见、有界的结果，不静默、不无限「生成中」；
 *   * 本入口只承载公开或合成内容；案件、线索与未公开调查信息不得输入。
 */
/**
 * 对话开场预设（OD-ARCH-031 Part B）：三场景不再是向导表单，而是**带进对话的规则**。
 */
export interface ChatPreset {
  key: string;
  label: string;
  /** 来自可编辑流程包的场景规则；缺省时由提示词组装回落内置默认。 */
  rules: string | null;
  /** 期望输出章节（每行一个）。 */
  sections: string | null;
  /** Profile 规则资产里的写作铁律；null/空 = 不输出该段（OD-ARCH-068）。 */
  writingRules?: string[] | null;
  /** 场景默认思考档；用户手动选档优先，且只在引擎空闲时应用。 */
  thinking?: string | null;
  /** 是否真的加载到 Profile 规则资产（false/undefined = 纯净底座）。 */
  rulesLoaded?: boolean;
  /** 场景绑定的 Skill 清单（OD-ARCH-082 S4 注入链路）。 */
  skills?: Array<{ id: string; content: string }> | null;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  taskId?: string;
  state?: TaskState | null;
  stopReason?: string | null;
  error?: string | null;
  /** 非致命提示：轮询中断、结果读取失败、超时等。 */
  notice?: string | null;
  /** 是否允许手动重新查询该任务。 */
  retryable?: boolean;
  /**
   * OD-ARCH-069：当前工具执行状态（「正在调用工具：x（y）」）。
   * 只存展示用的工具名与状态，不存工具入参或结果；任务结束即清空。
   */
  toolNote?: string | null;
  /** OD-ARCH-069：本轮用户输入的原文，仅用于「重发」。 */
  sentText?: string | null;
  /** OD-ARCH-078 S3：提交时刻与到达终态的时刻（毫秒），用于显示本轮耗时。 */
  startedAtMs?: number | null;
  endedAtMs?: number | null;
  /**
   * OD-ARCH-079：本轮 turn/end 的会话事件序号，DSH 原生 session/fork 的切点。
   * 只有拿到真实序号才能分支；缺失时按钮禁用，不做本地重放冒充。
   */
  forkSeq?: number | null;
}

/** 重放进下一轮提示的上文上限（字符）。超出时保留最近的部分。 */

const STATE_LABEL: Record<TaskState, string> = {
  queued: "排队中",
  starting: "启动中",
  running: "生成中",
  cancelling: "取消中",
  succeeded: "已完成",
  failed: "已失败",
  cancelled: "已取消",
  interrupted: "已中断",
  unknown: "状态未知",
};

const CATEGORY_LABEL: Record<string, string> = {
  local: "本机",
  private: "内网/私有",
  public: "外部",
  unknown: "未知",
};

/** 轮询间隔（毫秒）。 */
const POLL_INTERVAL_MS = 300;
/** 连续失败多少次后停止自动轮询，避免界面永久停在「生成中」。 */
const MAX_POLL_FAILURES = 5;
/** 终态后读取结果的重试次数。 */
const RESULT_RETRIES = 3;

type AdmissionState = "checking" | "ready" | "blocked";

let seq = 0;
const nextId = () => "m" + ++seq;

function enabledCategories(facts: AdmissionFacts): string[] {
  const p = facts.permissions;
  return (["local", "private", "public"] as const).filter((k) => p[k]);
}

/** 逐字呈现时的光标（Markdown 结构外单独渲染，避免被当成正文的一部分）。 */
function Caret() {
  return <span className="opacity-50 select-none">▍</span>;
}

/**
 * 逐字呈现（本地呈现效果，OD-ARCH-081 起按 Markdown 渲染）。
 *
 * 说明：DSH 的 **ACP 通道**是「助手消息提交后才发块」，**没有** token 级流式
 * （DSH 网页端的逐字效果走的是它自己的 harness 事件流）。这里在界面层做**逐字显现**，
 * 让观感接近网页端；文本本身仍以引擎返回的**最终正文**为准，不改写、不补全。
 *
 * 逐字与 Markdown 并存的做法：只把**已显现的那一段**交给渲染器，尚未显现的部分
 * 不参与解析，因此不会出现「半张表格先画出来再跳一下」的整段重排；光标挂在
 * Markdown 容器外，不参与语法解析。
 */
function StreamedMarkdown({ text, live }: { text: string; live: boolean }) {
  const [shown, setShown] = useState(live ? 0 : text.length);

  useEffect(() => {
    if (!live) {
      setShown(text.length);
      return;
    }
    if (shown >= text.length) return;
    const step = Math.max(2, Math.ceil(text.length / 80));
    const timer = window.setInterval(() => {
      setShown((n) => Math.min(text.length, n + step));
    }, 16);
    return () => window.clearInterval(timer);
  }, [text, live, shown]);

  return (
    <>
      <MarkdownText text={text.slice(0, shown)} />
      {live && shown < text.length ? <Caret /> : null}
    </>
  );
}

/**
 * 助手正文（OD-ARCH-048）。
 *
 * web-api 通道会推真实 `text_delta`，此时文本**原样增量渲染**，不再二次切片，
 * 否则观感上会落后于引擎并且掩盖真实时序。ACP 通道没有 token 级流式
 * （消息提交后才发块），此时保留原有的本地逐字显现作为降级。
 */
function AssistantText({
  text,
  live,
  streamed,
  highlightQuery,
}: {
  text: string;
  live: boolean;
  streamed: boolean;
  highlightQuery?: string;
}) {
  if (streamed) {
    return (
      <>
        <MarkdownText text={text} highlightQuery={highlightQuery} />
        {live ? <Caret /> : null}
      </>
    );
  }
  return highlightQuery ? (
    <MarkdownText text={text} highlightQuery={highlightQuery} />
  ) : (
    <StreamedMarkdown text={text} live={live} />
  );
}

/**
 * 消息下方的操作条（OD-ARCH-078 S3）：低对比度小按钮，鼠标悬停才显出强调色。
 * 引擎不支持的动作用 disabled + title 说明原因，不假装能点。
 */
function TipButton({
  icon,
  label,
  title,
  onClick,
  disabled = false,
}: {
  icon: Parameters<typeof Icon>[0]["name"];
  label: string;
  title?: string;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={label}
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[11px] text-outline hover:text-primary hover:bg-surface-container transition-colors disabled:opacity-45 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-outline"
    >
      <Icon name={icon} className="w-[13px] h-[13px]" />
      <span>{label}</span>
    </button>
  );
}

/** 本轮耗时：从提交到终态（毫秒）。缺任一端就不显示，不估算。 */
function elapsedText(startedAtMs?: number | null, endedAtMs?: number | null): string | null {
  if (!startedAtMs || !endedAtMs || endedAtMs < startedAtMs) return null;
  const sec = (endedAtMs - startedAtMs) / 1000;
  return sec < 60 ? `本轮 ${sec.toFixed(1)} 秒` : `本轮 ${Math.round(sec)} 秒`;
}

export interface HomeChatProps {
  onNavigate: (scene: Scene) => void;
  /**
   * 分支到新对话（OD-ARCH-079）：引擎侧已用 DSH 原生 session/fork 建好子会话，
   * 回调只负责把截至切点的消息带到新线程，并保存子会话号。
   */
  onBranchNewThread?: (messages: ChatMessage[], childSessionId: string) => void;
  /**
   * 场景选择（OD-ARCH-057）：首页是唯一的对话入口，场景在这里选。
   * 传入即显示场景 chip；选项由 Profile 声明联动（隐藏的入口不出现）。
   */
  sceneOptions?: Array<{ key: string; label: string }> | null;
  activeTag?: string;
  onChangeTag?: (tag: string) => void;
  /** 本机对话列表（同一行显示，可切回旧对话）。 */
  onNewThread?: () => void;
  /** 开场预设：把场景规则、期望章节与材料位置约定带进对话（OD-ARCH-031）。 */
  preset?: ChatPreset | null;
  /** 项目工作目录（绝对路径）；缺省时用宿主准备的工作区。 */
  projectDir?: string | null;
  /** 线程标识：变化时重置对话（切换会话）。 */
  threadKey?: string;
  /** 线程的初始消息（从本地存储恢复）。 */
  initialMessages?: ChatMessage[];
  /** 消息变化回调（用于本地持久化）。 */
  onMessagesChange?: (messages: ChatMessage[]) => void;
  /** 引擎产生了新的 DSH 会话（第一轮提交后）时回调，供模块把它关联到引擎会话。 */
  onTaskCreated?: (omniTaskId: string) => void;
  /**
   * 既有引擎会话号（OD-ARCH-034）：给出即续接同一会话（真多轮，引擎自己持有上文）。
   * 没有时本组件在本次运行内自行记住第一轮拿到的会话号。
   */
  engineSessionId?: string | null;
  /** 拿到（新建或续接后的）引擎会话号时回调，供上层按线程持久化。 */
  onEngineSessionId?: (engineSessionId: string) => void;
  /** 当前会话搜索（OD-ARCH-082 S8） */
  searchQuery?: string;
  searchCurrentIndex?: number;
  onSearchMatchesChange?: (count: number) => void;
  /**
   * 预填到输入框的草稿（OD-ARCH-091）：从场景页「转周报」过来时带上，
   * 只填不发，用户仍要自己按发送。
   */
  initialInput?: string;
}

export function HomeChat({
  onNavigate,
  preset = null,
  searchQuery = "",
  searchCurrentIndex = 0,
  onSearchMatchesChange,
  sceneOptions = null,
  activeTag = "plain",
  onChangeTag,
  onNewThread,
  projectDir = null,
  threadKey = "default",
  initialMessages = [],
  onMessagesChange,
  onTaskCreated,
  onBranchNewThread,
  engineSessionId = null,
  onEngineSessionId,
  initialInput = "",
}: HomeChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);

  // 记录上一次真正触发滚动的检索目标 (query + index) 与上一次查询词，避免流式输出时反复抢视口
  const lastScrollTargetRef = useRef<{ query: string; index: number }>({ query: "", index: -1 });
  const lastSearchQueryRef = useRef("");

  // OD-ARCH-084 S2, S3：DOM 命中计数与活跃命中项标注（messages 变化时只重算，不触发视口滚动）
  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;

    const q = searchQuery.trim();
    if (!q) {
      onSearchMatchesChange?.(0);
      const activeHits = scroller.querySelectorAll<HTMLElement>("[data-search-active]");
      activeHits.forEach((el) => {
        el.removeAttribute("data-search-active");
        el.removeAttribute("data-search-flash");
      });
      return;
    }

    const hits = Array.from(scroller.querySelectorAll<HTMLElement>("[data-search-hit]"));
    const count = hits.length;
    onSearchMatchesChange?.(count);

    if (count === 0) return;

    const activeIdx = Math.max(0, Math.min(searchCurrentIndex, count - 1));

    // S3: 标记当前活跃项独占 data-search-active；其余非当前项必须清空，严格保证 [data-search-active=true] 恒为 1
    hits.forEach((el, idx) => {
      if (idx === activeIdx) {
        el.setAttribute("data-search-active", "true");
      } else {
        el.removeAttribute("data-search-active");
        el.removeAttribute("data-search-flash");
      }
    });
  }, [searchQuery, searchCurrentIndex, messages, onSearchMatchesChange]);

  // OD-ARCH-084 S4, S5：命中词滚动定位与短强调（仅在 searchQuery 或 searchCurrentIndex 变化时触发，与 messages 隔离）
  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;

    const q = searchQuery.trim();
    if (!q) {
      lastScrollTargetRef.current = { query: "", index: -1 };
      lastSearchQueryRef.current = "";
      return;
    }

    const hits = Array.from(scroller.querySelectorAll<HTMLElement>("[data-search-hit]"));
    const count = hits.length;
    if (count === 0) return;

    const activeIdx = Math.max(0, Math.min(searchCurrentIndex, count - 1));
    const activeHit = hits[activeIdx];
    if (!activeHit) return;

    const lastTarget = lastScrollTargetRef.current;
    // 只有当查询词或索引真正改变时才执行滚动判断与定位
    const isNewQuery = lastSearchQueryRef.current !== q;
    const isTargetChanged = lastTarget.query !== q || lastTarget.index !== activeIdx;
    if (!isTargetChanged && !isNewQuery) return;

    lastSearchQueryRef.current = q;
    lastScrollTargetRef.current = { query: q, index: activeIdx };

    const cRect = scroller.getBoundingClientRect();
    const mRect = activeHit.getBoundingClientRect();
    const vH = scroller.clientHeight;

    const inSafeZone =
      mRect.top >= cRect.top + 24 &&
      mRect.bottom <= cRect.bottom - 24;

    // 新检索词必须将首个命中词滚动至 32% 阅读舒适区；后续逐处切换（按键/点击）若超出安全区则平滑滚入 32%
    if (isNewQuery || !inSafeZone) {
      const targetScrollTop =
        scroller.scrollTop +
        (mRect.top - cRect.top) -
        (vH * 0.32 - mRect.height / 2);
      const clampedScroll = Math.max(
        0,
        Math.min(targetScrollTop, scroller.scrollHeight - scroller.clientHeight)
      );

      const prefersReduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      scroller.scrollTo({ top: scroller.scrollTop, behavior: "instant" });
      if (prefersReduced) {
        scroller.scrollTo({ top: clampedScroll, behavior: "instant" });
      } else {
        scroller.scrollTo({ top: clampedScroll, behavior: "smooth" });
      }
    }

    // S5: 滚动停稳后给予约 700ms 单次强调，可被用户交互即刻打断
    let flashTimer: number | undefined;

    const triggerFlash = () => {
      activeHit.setAttribute("data-search-flash", "true");
      flashTimer = window.setTimeout(() => {
        activeHit.removeAttribute("data-search-flash");
      }, 700);
    };

    let scrollSettleTimer = window.setTimeout(triggerFlash, 150);
    const onScroll = () => {
      window.clearTimeout(scrollSettleTimer);
      scrollSettleTimer = window.setTimeout(triggerFlash, 150);
    };
    scroller.addEventListener("scroll", onScroll, { passive: true });

    const cancelFlash = () => {
      window.clearTimeout(scrollSettleTimer);
      window.clearTimeout(flashTimer);
      activeHit.removeAttribute("data-search-flash");
    };

    const userInterruptEvents = ["wheel", "touchstart", "pointerdown", "keydown"];
    userInterruptEvents.forEach((evt) =>
      scroller.addEventListener(evt, cancelFlash, { passive: true, once: true })
    );

    return () => {
      cancelFlash();
      scroller.removeEventListener("scroll", onScroll);
      userInterruptEvents.forEach((evt) =>
        scroller.removeEventListener(evt, cancelFlash)
      );
    };
  }, [searchQuery, searchCurrentIndex]);
  // 没有上层按线程传入时（例如首页），本组件自己持有本轮运行内的会话号。
  const [localEngineSessionId, setLocalEngineSessionId] = useState<string | null>(null);
  // 续接失败被降级为新建会话后，下一轮要补一次窗口化重放（把历史接回新会话）。
  const [replayNextTurn, setReplayNextTurn] = useState(false);
  const effectiveEngineSessionId = engineSessionId ?? localEngineSessionId;
  // 输入框是否有焦点：上报给骨架（输入框有焦点时不把顶部卡片放回来，OD-ARCH-042）。
  const reportInputFocus = useChatInputFocus();
  const [input, setInput] = useState(initialInput);
  const [busy, setBusy] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  /**
   * OD-ARCH-062 S2：取消入口与任务工作台页面共用同一套取消状态模型
   * （src/engineCancelModel.ts）。首页不再自己拼取消文案与放行条件。
   */
  const [activeTaskStatus, setActiveTaskStatus] = useState<string | null>(null);
  /** 取消结果文案（只保留阶段与说明，不伪造任务号）。 */
  const [engineCancel, setEngineCancel] = useState<{
    phase: string;
    final_state: string | null;
    note: string;
  } | null>(null);
  const [cancelBusy, setCancelBusy] = useState(false);
  const [cancelRequested, setCancelRequested] = useState(false);
  const cancelInFlightRef = useRef(false);
  /** 新一轮提交/重试时清空取消态：旧任务的取消结果不能贴到新任务上。 */
  const resetCancelState = useCallback(() => {
    cancelInFlightRef.current = false;
    setCancelBusy(false);
    setCancelRequested(false);
    setEngineCancel(null);
    setActiveTaskStatus(null);
  }, []);
  const [workspaceDir, setWorkspaceDir] = useState<string | null>(null);
  const [workspaceNote, setWorkspaceNote] = useState<string | null>(null);
  /** 引擎上报的上下文用量（ACP `usage_update` → progress/usage 的 used/size）；未上报时为 null。 */
  const [usage, setUsage] = useState<{ used: number; size: number } | null>(null);
  const [admission, setAdmission] = useState<AdmissionState>("checking");
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);
  const [admissionError, setAdmissionError] = useState<string | null>(null);
  /** OD-ARCH-034：各 DSH Profile 生效的模型候选（不同 Profile 的模型选择互不相通）。 */
  const [modelRoutes, setModelRoutes] = useState<
    Array<{
      profile: string;
      default: { provider: string | null; model: string | null } | null;
      providers: Array<{ id: string; models: string[]; baseURL: string | null }>;
    }>
  >([]);
  const [modelNote, setModelNote] = useState<string | null>(null);
  /**
   * OD-ARCH-048：web-api 通道直读的模型目录。
   * 取得真实目录时用它渲染选择器；ACP 或读取失败时回退到 Profile 候选（groups）。
   */
  const [modelCatalog, setModelCatalog] = useState<EngineModelCatalog | null>(null);
  /** 已收到过真实 `text_delta` 的任务（这些任务的正文不再做本地打字机降级）。 */
  const streamedTasksRef = useRef<Set<string>>(new Set());
  const [, forceStreamRender] = useState(0);
  const [modelBusy, setModelBusy] = useState(false);
  /** 思考等级（对应 DSH 的 reasoningEffort）：默认＝不传该参数。 */
  const [modelEffort, setModelEffort] = useState("off");
  /**
   * 用户是否手动选过思考档（OD-ARCH-068）。
   * 只有「没手动选过 + 引擎空闲」时才按场景默认切档；手动选档永远优先。
   */
  const effortTouchedRef = useRef(false);
  /**
   * 本轮引用的文件（OD-ARCH-077 S1）：正文只在内存里，随这一轮提示词下发。
   * 不写盘、不进日志；发送后清空，避免下一轮误带上一轮的文件。
   */
  const [attachments, setAttachments] = useState<AttachmentInput[]>([]);
  /**
   * 附件列表的同步镜像（OD-ARCH-077）。
   * 拖拽一次会并发触发多个 addAttachment；只读 state 闭包会让每个调用都看到旧数组，
   * 于是能突破 5 个上限或重复加入同一个文件。这里用 ref 作为当前事实，读写同一份。
   */
  const attachmentsRef = useRef<AttachmentInput[]>([]);
  const [attachNote, setAttachNote] = useState<string | null>(null);
  /** 允许引用的根（成果目录 / 库根），只用于把候选标成来源，不参与路径校验。 */
  const [referenceRoots, setReferenceRoots] = useState<ReferenceRoot[]>([]);
  /** @ 引用选择器：查询词 + 候选（只列宿主允许根内的白名单文本）。 */
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [mentionItems, setMentionItems] = useState<ReferenceCandidate[]>([]);
  const [mentionBusy, setMentionBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [voiceNote, setVoiceNote] = useState(false);
  /** 生成中再次发送时的可见提示（不再静默丢弃）。 */
  const [sendNote, setSendNote] = useState<string | null>(null);
  /** 消息操作条的一次性反馈（复制成功、朗读不可用、分支已建等），按消息 id 显示。 */
  const [tipNote, setTipNote] = useState<{ id: string; text: string } | null>(null);
  /** 已提交到宿主、引擎尚未返回任务号（冷启动可能数秒）。 */
  const [starting, setStarting] = useState(false);
  /**
   * 本轮提交的放弃标记（OD-ARCH-045 第 3 步）。
   * 冷启动期间还没有任务号，宿主无法按任务号取消失败的提交；这里只能撤销
   * 界面上这次等待，并**在任务号返回后立即请求取消**。绝不把「已放弃」写成「引擎已终止」。
   */
  const submitTokenRef = useRef<{ aborted: boolean; noteId: string | null } | null>(null);
  /** 已等待秒数（让「慢」可见，而不是像卡死）。 */
  const [elapsed, setElapsed] = useState(0);
  /** 每个任务的思考过程（模型产出的思考，不计入最终结果）。 */
  const [thoughts, setThoughts] = useState<Record<string, string>>({});
  /**
   * OD-ARCH-066：需要人工决定的工具审批。按任务号分组，只保存归一化字段
   * （工具名/原因/编号），不保存工具参数正文。
   */
  const [approvals, setApprovals] = useState<Record<string, ApprovalRequest[]>>({});
  /** 正在回传决定的审批编号，用于禁用按钮并显示等待。 */
  const [approvalBusy, setApprovalBusy] = useState<string | null>(null);
  /**
   * OD-ARCH-088：引擎的结构化提问，按任务号分组。题干与答案只在本机内存与界面，
   * 不写任务状态文件、不进日志。
   */
  const [questions, setQuestions] = useState<Record<string, QuestionRequest[]>>({});
  /** 已发出取消请求的提问编号（只表示请求已发出，等引擎确认关闭）。 */
  const [questionCancelRequested, setQuestionCancelRequested] = useState<Record<string, boolean>>({});
  /** 本会话已经提示过「技能未预注入」的场景键，避免反复刷屏。 */
  const skillHintShownRef = useRef<Set<string>>(new Set());
  const [skillHint, setSkillHint] = useState<string | null>(null);
  /** OD-ARCH-079：分支请求进行中，防止连点建出多条子会话。 */
  const [branchBusy, setBranchBusy] = useState(false);
  const branchBusyRef = useRef(false);
  /** OD-ARCH-066：任务台账里的首字延迟（秒），用于显示「首字 x.xs」。 */
  const [ttfb, setTtfb] = useState<Record<string, number>>({});
  /** 宿主报告的本任务待审批数量（与事件里的卡片互为佐证）。 */
  const [pendingApprovals, setPendingApprovals] = useState<Record<string, number>>({});
  const scrollRef = useRef<HTMLDivElement | null>(null);
  /** OD-ARCH-079：每条用户问句的锚点，供右侧索引条跳转与高亮。 */
  const turnAnchors = useRef<Record<string, HTMLDivElement | null>>({});
  const [activeTurnId, setActiveTurnId] = useState<string | null>(null);
  /** 宿主拒绝准备工作目录时的真实原因（供提交失败时原样带出）。 */
  const workspaceNoteRef = useRef<string | null>(null);

  // 切换会话（threadKey 变化）时载入该会话的消息。
  useEffect(() => {
    setMessages(initialMessages);
    setSkillHint(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadKey]);

  /**
   * OD-ARCH-088 S4：场景规则已加载、但该场景没有可注入的 Skill 时，如实说明
   * 「未预注入」不等于「技能不可用」。同一会话只提示一次，不弹窗、不刷屏。
   * 没有规则资产（纯净底座）时不提示——那不是旧版规则包的绑定缺失。
   */
  useEffect(() => {
    if (!preset || !preset.rulesLoaded) return;
    if (preset.skills !== null) return;
    const key = threadKey + ":" + preset.key;
    if (skillHintShownRef.current.has(key)) return;
    skillHintShownRef.current.add(key);
    setSkillHint(
      "本场景的技能未预注入（规则包未声明场景 Skill 绑定，可能是旧版规则包）。" +
        "模型仍可按需自行加载技能——这不代表技能不可用；可在设置 →「规则资产」点「恢复默认规则包」启用。",
    );
  }, [preset, threadKey]);

  // 消息变化回传给工作区（本地持久化）。
  useEffect(() => {
    onMessagesChange?.(messages);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages]);

  const canSubmit = admission === "ready" && !busy;

  /** OD-ARCH-050：区分「正在读」与「读失败」——刷新期间保留上一次事实，不再闪成「未识别」。 */
  const [factsLoading, setFactsLoading] = useState(false);
  const factsRef = useRef<AdmissionFacts | null>(null);
  useEffect(() => {
    factsRef.current = facts;
  }, [facts]);

  const refreshAdmission = useCallback(async () => {
    setFactsLoading(true);
    if (!factsRef.current) setAdmission("checking");
    try {
      const status = await host.admissionStatus();
      if (!status.ok || !status.facts) {
        // 读取失败时保留上一次已知事实（如果有），只把原因记下来。
        setAdmission("blocked");
        setAdmissionError(status.error?.message ?? "无法读取模型端点信息。");
        return;
      }
      setFacts(status.facts);
      if (status.blocked) {
        setAdmission("blocked");
        setAdmissionError(status.block_reason ?? "权限变更尚未确认，宿主暂时拒绝新提交。");
        return;
      }
      setAdmission(status.facts.allowed ? "ready" : "blocked");
      setAdmissionError(
        status.facts.allowed ? null : status.facts.category_reason || "模型权限未开启。",
      );
    } catch (e) {
      setAdmission("blocked");
      setAdmissionError(e instanceof Error ? e.message : String(e));
    } finally {
      setFactsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshAdmission();
  }, [refreshAdmission]);

  // 可引用的来源根（只用于把候选标成「成果目录」或「库根」）。
  // 路径是否能读，仍由宿主逐次校验；这里不据此放行任何文件。
  useEffect(() => {
    shell
      .referenceRoots()
      .then((r) => setReferenceRoots(r.roots))
      .catch(() => setReferenceRoots([]));
  }, []);

  const rootKindOf = useCallback(
    (sourceRoot: string) =>
      referenceRoots.find((r) => r.path === sourceRoot)?.label ?? "文件",
    [referenceRoots],
  );

  // 列出各 Profile 的模型候选（只取非机密字段）。
  const loadModelRoutes = useCallback(async () => {
    // 两条数据源都要看：目录直读（web-api）优先，Profile 候选作为回退与写覆盖的来源。
    const [catalog, routes] = await Promise.allSettled([
      shell.listEngineModels(),
      shell.listModelRoutes(),
    ]);
    if (catalog.status === "fulfilled") {
      setModelCatalog(catalog.value);
    } else {
      setModelCatalog({ available: false, channel: "web-api", reason: String(catalog.reason) });
    }
    if (routes.status === "fulfilled") {
      setModelRoutes(routes.value.profiles ?? []);
    } else {
      setModelRoutes([]);
    }
  }, []);

  useEffect(() => {
    void loadModelRoutes();
  }, [loadModelRoutes]);

  /** 切换模型：写 OmniDesk 自己的覆盖文件 → 引擎重启 → 端点重新判定。 */
  const selectModel = useCallback(
    async (profile: string, provider: string, model: string, effort?: string) => {
      setModelBusy(true);
      setModelNote(null);
      try {
        // 改选模型后，路由与端点事实都会变：先让缓存作废（OD-ARCH-041）。
        invalidateEngineFactsCache();
        const r = await shell.setModelRoute({
          sourceProfile: profile,
          provider,
          model,
          reasoningEffort: effort ?? modelEffort,
        });
        setModelNote(
          `已切到 ${provider} / ${model}` +
            (effort && effort !== "default" ? `（思考等级：${effort}）` : "") +
            `，端点 ${r.route.endpoint ?? "未解析"}（类别 ${r.route.category}）。引擎已按新路由重启。`,
        );
        await refreshAdmission();
      } catch (e) {
        setModelNote("切换失败：" + (e instanceof Error ? e.message : String(e)));
      } finally {
        setModelBusy(false);
      }
    },
    [modelEffort, refreshAdmission],
  );

  /**
   * 改为跟随 DSH 设置（OD-ARCH-050）：归档 OmniDesk 覆盖文件、停掉引擎，
   * 下次任务直接使用 DSH 里配置的 provider 与默认模型，不再要求在这里重选。
   */
  const followDsh = useCallback(async () => {
    setModelBusy(true);
    setModelNote(null);
    try {
      const r = await shell.clearModelRoute();
      setModelNote(r.note);
      invalidateEngineFactsCache();
      await Promise.all([refreshAdmission(), loadModelRoutes()]);
    } catch (e) {
      setModelNote("切换失败：" + (e instanceof Error ? e.message : String(e)));
    } finally {
      setModelBusy(false);
    }
  }, [refreshAdmission, loadModelRoutes]);

  // 引擎要求 workspace_dir 为绝对路径。宿主在端点未获准时会拒绝准备，
  // 此时如实记录拒绝原因并把工作目录置空，不用「仅解析出的路径」冒充可用目录。
  const resolveWorkspaceDir = useCallback(async (): Promise<string | null> => {
    try {
      const prep = await shell.prepareWorkspaceDir();
      if (prep.ok && prep.workspace_dir) {
        setWorkspaceNote(null);
        workspaceNoteRef.current = null;
        return prep.workspace_dir;
      }
      const reason = prep.error ?? "宿主未返回可用的工作目录。";
      setWorkspaceNote(reason);
      workspaceNoteRef.current = reason;
      return null;
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      setWorkspaceNote(reason);
      workspaceNoteRef.current = reason;
      return null;
    }
  }, []);

  // 订阅任务事件，把流式文本追加到对应助手消息。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let closed = false;
    onTaskEvent((event) => {
      if (event.type === "progress") {
        // 引擎上报的上下文用量（used / size tokens）。只显示宿主转发的真实数值，
        // 不自行估算或累加；未上报时界面显示「—」。
        const p = event.payload as {
          kind?: unknown;
          used?: unknown;
          size?: unknown;
          text?: unknown;
          approval_id?: unknown;
          question_id?: unknown;
          questions?: unknown;
          answerable?: unknown;
          violations?: unknown;
          tool_name?: unknown;
          call_id?: unknown;
          reason?: unknown;
          event?: unknown;
          note?: unknown;
          idle_seconds?: unknown;
        };
        // OD-ARCH-069：引擎在等一项我们没识别出类型的确认。不上自动应答，
        // 只把「它在等什么」如实说出来，免得看起来像死机。
        if (p.kind === "engine_ask") {
          const kind = typeof p.event === "string" && p.event ? p.event : "未命名事件";
          setMessages((prev) =>
            prev.map((m) =>
              m.taskId === event.omni_task_id
                ? {
                    ...m,
                    notice:
                      "引擎在等待一项未被识别的确认（" +
                      kind +
                      "），如长时间无响应请取消后重发。",
                  }
                : m,
            ),
          );
          return;
        }
        // 审批请求（OD-ARCH-066）：工具需要人工决定时，把它挂到对应任务下。
        if (p.kind === "approval_request" && typeof p.approval_id === "string") {
          const approval: ApprovalRequest = {
            approvalId: p.approval_id,
            omniTaskId: event.omni_task_id,
            toolName: typeof p.tool_name === "string" ? p.tool_name : "未知工具",
            callId: typeof p.call_id === "string" ? p.call_id : "",
            reason: typeof p.reason === "string" ? p.reason : "",
            status: "pending",
          };
          setApprovals((prev) => {
            const list = prev[event.omni_task_id] ?? [];
            if (list.some((a) => a.approvalId === approval.approvalId)) return prev;
            return { ...prev, [event.omni_task_id]: [...list, approval] };
          });
          return;
        }
        if (
          (p.kind === "approval_decided" || p.kind === "approval_cancelled") &&
          typeof p.approval_id === "string"
        ) {
          const decided = p.kind === "approval_decided";
          const decision = (event.payload as { decision?: unknown }).decision;
          setApprovals((prev) => {
            const list = prev[event.omni_task_id];
            if (!list) return prev;
            return {
              ...prev,
              [event.omni_task_id]: list.map((a) =>
                a.approvalId === p.approval_id
                  ? {
                      ...a,
                      status: decided
                        ? decision === "rejected"
                          ? "rejected"
                          : "allowed"
                        : "cancelled",
                    }
                  : a,
              ),
            };
          });
          return;
        }
        /** OD-ARCH-088：结构化提问到达 —— 登记为可答卡片（同 id 不重复）。 */
        if (p.kind === "question_request" && typeof p.question_id === "string") {
          const rawQuestions = Array.isArray(p.questions) ? p.questions : [];
          const parsed = rawQuestions
            .map((raw) => {
              if (!raw || typeof raw !== "object") return null;
              const item = raw as Record<string, unknown>;
              if (typeof item.id !== "string" || typeof item.question !== "string") return null;
              const options = Array.isArray(item.options)
                ? item.options
                    .map((o) => {
                      if (!o || typeof o !== "object") return null;
                      const opt = o as Record<string, unknown>;
                      if (typeof opt.label !== "string") return null;
                      return {
                        label: opt.label,
                        description:
                          typeof opt.description === "string" ? opt.description : undefined,
                      };
                    })
                    .filter(
                      (o): o is { label: string; description: string | undefined } => o !== null,
                    )
                : [];
              return {
                id: item.id,
                question: item.question,
                multi_select: item.multi_select === true,
                options,
                ...(typeof item.header === "string" ? { header: item.header } : {}),
                ...(typeof item.detail === "string" ? { detail: item.detail } : {}),
              };
            })
            .filter((q): q is NonNullable<typeof q> => q !== null);
          if (parsed.length === 0) {
            // 题目结构不可解析：如实提示，不伪造一张空卡。
            setMessages((prev) =>
              prev.map((m) =>
                m.taskId === event.omni_task_id
                  ? { ...m, notice: "引擎发来一条无法解析的提问，未显示选项；如长时间无响应请取消后重发。" }
                  : m,
              ),
            );
            return;
          }
          const request: QuestionRequest = {
            questionId: p.question_id,
            omniTaskId: event.omni_task_id,
            callId: typeof p.call_id === "string" ? p.call_id : "",
            questions: parsed,
            answerable: p.answerable !== false,
            violations: Array.isArray(p.violations)
              ? p.violations.filter((v): v is string => typeof v === "string")
              : [],
            status: "pending",
            note: typeof p.note === "string" ? p.note : undefined,
          };
          setQuestions((prev) => {
            const list = prev[event.omni_task_id] ?? [];
            if (list.some((q) => q.questionId === request.questionId)) return prev;
            return { ...prev, [event.omni_task_id]: [...list, request] };
          });
          return;
        }
        if (p.kind === "question_answered" && typeof p.question_id === "string") {
          const answeredId = p.question_id;
          setQuestions((prev) => {
            const list = prev[event.omni_task_id];
            if (!list) return prev;
            return {
              ...prev,
              [event.omni_task_id]: list.map((q) =>
                q.questionId === answeredId
                  ? { ...q, status: "answered" as const, error: undefined }
                  : q,
              ),
            };
          });
          return;
        }
        if (p.kind === "question_closed" && typeof p.question_id === "string") {
          const closedId = p.question_id;
          const reason = typeof p.reason === "string" && p.reason ? p.reason : "cancelled";
          setQuestions((prev) => {
            const list = prev[event.omni_task_id];
            if (!list) return prev;
            return {
              ...prev,
              [event.omni_task_id]: list.map((q) =>
                q.questionId === closedId && q.status !== "answered"
                  ? { ...q, status: "closed" as const, closedReason: reason }
                  : q,
              ),
            };
          });
          return;
        }
        if (p.kind === "usage" && typeof p.used === "number" && typeof p.size === "number") {
          setUsage({ used: p.used, size: p.size });
        }
        // 长时间没有新帧时引擎只发提示、不结束任务（2026-10-23）。
        // 用与工具活动同一行的实时状态显示，避免看起来像卡死，也不制造「失败」错觉。
        if (p.kind === "still_running") {
          const idle = typeof p.idle_seconds === "number" ? Math.round(p.idle_seconds) : 0;
          setMessages((prev) =>
            prev.map((m) =>
              m.taskId === event.omni_task_id
                ? {
                    ...m,
                    toolNote:
                      "DSH 仍在运行" +
                      (idle > 0 ? "（已静默 " + idle + " 秒）" : "") +
                      " · 不会自动停止，需要时可点取消",
                  }
                : m,
            ),
          );
          return;
        }
        // 思考过程（模型产出的思考，不计入最终结果）：按任务累积，单独展示。
        if (p.kind === "thought_delta" && typeof p.text === "string" && p.text) {
          setThoughts((prev) => ({
            ...prev,
            [event.omni_task_id]: (prev[event.omni_task_id] ?? "") + p.text,
          }));
        }
        return;
      }
      // OD-ARCH-069：工具执行期间显示工具名与状态，让「在干活」和「卡住」可区分。
      // 只显示工具名与状态字符串，不展示工具入参/结果。
      if (event.type === "tool_activity") {
        const tp = event.payload as { tool_name?: unknown; status?: unknown };
        const tool = typeof tp.tool_name === "string" && tp.tool_name ? tp.tool_name : "未知工具";
        const status = typeof tp.status === "string" && tp.status ? tp.status : "";
        setMessages((prev) =>
          prev.map((m) =>
            m.taskId === event.omni_task_id
              ? { ...m, toolNote: "正在调用工具：" + tool + (status ? "（" + status + "）" : "") }
              : m,
          ),
        );
        return;
      }
      if (event.type !== "text_delta") return;
      const delta = String((event.payload as { text?: unknown }).text ?? "");
      if (!delta) return;
      // 记下「这条任务有真实增量」——渲染层据此停用本地打字机降级。
      if (!streamedTasksRef.current.has(event.omni_task_id)) {
        streamedTasksRef.current.add(event.omni_task_id);
        forceStreamRender((n) => n + 1);
      }
      setMessages((prev) =>
        prev.map((m) =>
          m.role === "assistant" && m.taskId === event.omni_task_id
            ? { ...m, text: m.text + delta }
            : m,
        ),
      );
    }).then((fn) => {
      if (closed) fn();
      else unlisten = fn;
    });
    return () => {
      closed = true;
      if (unlisten) unlisten();
    };
  }, []);

  /**
   * 回传审批决定（OD-ARCH-066）。
   *
   * 只有宿主确认送达才把卡片标为已决定；失败时保留待审批态并显示原因，
   * 让用户可以重试——绝不先改界面再等结果。
   */
  const respondApproval = useCallback(
    async (taskId: string, approvalId: string, allow: boolean) => {
      setApprovalBusy(approvalId);
      try {
        await host.approvalRespond(taskId, approvalId, allow);
        setApprovals((prev) => ({
          ...prev,
          [taskId]: (prev[taskId] ?? []).map((a) =>
            a.approvalId === approvalId
              ? { ...a, status: allow ? "allowed" : "rejected", error: undefined }
              : a,
          ),
        }));
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setApprovals((prev) => ({
          ...prev,
          [taskId]: (prev[taskId] ?? []).map((a) =>
            a.approvalId === approvalId ? { ...a, status: "pending", error: message } : a,
          ),
        }));
      } finally {
        setApprovalBusy(null);
      }
    },
    [],
  );

  /**
   * 回传结构化提问的答案（OD-ARCH-088）。
   *
   * 先把卡片置 submitting（禁用交互），只有宿主返回收据才置 answered；任何失败都
   * 保留待答并写真实原因。不在这里替用户补默认选项，也不在失败后假装已送达。
   */
  const respondQuestion = useCallback(
    async (taskId: string, questionId: string, answers: QuestionAnswerItem[]) => {
      setQuestions((prev) => {
        const list = prev[taskId];
        if (!list) return prev;
        return {
          ...prev,
          [taskId]: list.map((q) =>
            q.questionId === questionId ? { ...q, status: "submitting" as const, error: undefined } : q,
          ),
        };
      });
      try {
        await host.questionAnswer(taskId, questionId, answers);
        setQuestions((prev) => {
          const list = prev[taskId];
          if (!list) return prev;
          return {
            ...prev,
            [taskId]: list.map((q) =>
              q.questionId === questionId
                ? { ...q, status: "answered" as const, error: undefined }
                : q,
            ),
          };
        });
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setQuestions((prev) => {
          const list = prev[taskId];
          if (!list) return prev;
          return {
            ...prev,
            [taskId]: list.map((q) =>
              q.questionId === questionId
                ? { ...q, status: "failed" as const, error: message }
                : q,
            ),
          };
        });
      }
    },
    [],
  );

  /** 终态后回读结果：有限重试；失败给出可见提示与重试入口。 */
  const loadResult = useCallback(async (taskId: string, state: TaskState) => {
    let lastError = "";
    for (let attempt = 1; attempt <= RESULT_RETRIES; attempt += 1) {
      try {
        const result = await host.taskResult(taskId);
        const partial = result.integrity === "partial" || result.form === "partial";
        setMessages((prev) =>
          prev.map((m) =>
            m.taskId === taskId
              ? {
                  ...m,
                  state,
                  stopReason: result.stop_reason ?? null,
                  text: m.text || (result.content || ""),
                  notice: partial ? "结果为部分结果（partial），可能不完整，请人工核对。" : null,
                  retryable: partial,
                  // 终态时刻只记一次：重新查询不会把这轮耗时越算越长。
                  endedAtMs: m.endedAtMs ?? Date.now(),
                }
              : m,
          ),
        );
        return;
      } catch (e) {
        lastError = String(e);
        if (attempt < RESULT_RETRIES) {
          await new Promise((resolve) => window.setTimeout(resolve, 500 * attempt));
        }
      }
    }
    setMessages((prev) =>
      prev.map((m) =>
        m.taskId === taskId
          ? {
              ...m,
              state,
              endedAtMs: m.endedAtMs ?? Date.now(),
              notice:
                "任务已结束（" +
                STATE_LABEL[state] +
                "），但结果读取失败（已重试 " +
                RESULT_RETRIES +
                " 次）：" +
                lastError,
              retryable: true,
            }
          : m,
      ),
    );
  }, []);

  // 轮询终态：有界失败、不静默吞错、不把未知状态写成成功。
  // 2026-10-23：去掉墙钟上限——只要底座还在跑（含插件检索长达十几分钟），
  // 界面就一直跟着真实状态；停止只由用户点「取消」或底座报错决定。
  useEffect(() => {
    if (!activeTaskId) return;
    const taskId = activeTaskId;
    let stopped = false;
    let failures = 0;

    const settle = () => {
      setActiveTaskId(null);
      setBusy(false);
    };

    const markNotice = (notice: string) => {
      setMessages((prev) =>
        prev.map((m) => (m.taskId === taskId ? { ...m, notice, retryable: true } : m)),
      );
    };

    const timer = window.setInterval(async () => {
      if (stopped) return;
      try {
        const status = await host.taskStatus(taskId);
        if (stopped) return;
        failures = 0;
        setActiveTaskStatus(status.status);
        if (typeof status.ttfb_seconds === "number") {
          setTtfb((prev) =>
            prev[taskId] === status.ttfb_seconds
              ? prev
              : { ...prev, [taskId]: status.ttfb_seconds as number },
          );
        }
        if (typeof status.pending_approvals === "number") {
          setPendingApprovals((prev) =>
            prev[taskId] === status.pending_approvals
              ? prev
              : { ...prev, [taskId]: status.pending_approvals as number },
          );
        }
        setMessages((prev) =>
          prev.map((m) =>
            m.taskId === taskId
              ? {
                  ...m,
                  state: status.status,
                  stopReason: null,
                  // OD-ARCH-079：记下真实切点序号（缺失就保持原值，不猜）。
                  forkSeq:
                    typeof status.engine_turn_end_seq === "number"
                      ? status.engine_turn_end_seq
                      : m.forkSeq,
                  // 任务结束就不再显示「正在调用工具」，避免残留过期的进度。
                  toolNote: TERMINAL_STATES.includes(status.status) ? null : m.toolNote,
                }
              : m,
          ),
        );
        if (TERMINAL_STATES.includes(status.status)) {
          // 任务已终态：引擎不再需要这些审批。若没收到撤回收到的通知，
          // 也要把仍挂着的卡片标为失效，避免界面一直显示「待决定」。
          setApprovals((prev) => {
            const list = prev[taskId];
            if (!list || !list.some((a) => a.status === "pending")) return prev;
            return {
              ...prev,
              [taskId]: list.map((a) =>
                a.status === "pending"
                  ? { ...a, status: "cancelled" as const, error: undefined }
                  : a,
              ),
            };
          });
          // OD-ARCH-088：任务已终态，仍未答的提问不再可答；取消请求已发出的也算取消。
          setQuestions((prev) => {
            const list = prev[taskId];
            if (!list) return prev;
            if (!list.some((q) => q.status !== "answered" && q.status !== "closed")) return prev;
            const reason = status.status === "cancelled" ? "cancelled" : "task_terminal";
            return {
              ...prev,
              [taskId]: list.map((q) =>
                q.status === "answered" || q.status === "closed"
                  ? q
                  : { ...q, status: "closed" as const, closedReason: reason },
              ),
            };
          });
          settle();
          await loadResult(taskId, status.status);
          // OD-ARCH-069：超时（unknown）必须说清楚等了多少秒、没有自动重试，
          // 并留出「重发」入口。引擎的 note 已带秒数；缺省时给一句保守文案。
          if (status.status === "unknown") {
            const note =
              (status.note || "").trim() ||
              "引擎未在等待上限内返回结果，本次未自动重试。可重发一次，或降低工具使用。";
            setMessages((prev) =>
              prev.map((m) =>
                m.taskId === taskId ? { ...m, notice: note, retryable: true, toolNote: null } : m,
              ),
            );
          }
          // OD-ARCH-070：上游模型网关的拒绝（如 HTTP 400 / 内容风险）必须显示
          // 真实原因，不能只留一句「已失败」。引擎把原因放在 note 的 error= 段里。
          if (status.status === "failed" || status.status === "interrupted") {
            const note = (status.note || "").trim();
            if (note) {
              setMessages((prev) =>
                prev.map((m) =>
                  m.taskId === taskId
                    ? { ...m, notice: note, retryable: true, toolNote: null }
                    : m,
                ),
              );
            }
          }
        }
      } catch (e) {
        failures += 1;
        if (failures >= MAX_POLL_FAILURES) {
          settle();
          markNotice(
            "状态查询连续失败 " +
              failures +
              " 次，已停止自动刷新：" +
              String(e) +
              "。任务状态未知，可点『重新查询』。",
          );
        }
      }
    }, POLL_INTERVAL_MS);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [activeTaskId, loadResult]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages]);

  // 生成中的计时：让等待可见
  useEffect(() => {
    if (!activeTaskId) {
      setElapsed(0);
      return;
    }
    const started = Date.now();
    setElapsed(0);
    const timer = window.setInterval(() => setElapsed(Math.floor((Date.now() - started) / 1000)), 500);
    return () => window.clearInterval(timer);
  }, [activeTaskId]);

  const submitText = useCallback(
    async (raw: string, picked: AttachmentInput[] = []): Promise<boolean> => {
      const text = raw.trim();
      if (!text || busy) return false;
      const decision = currentEngineSubmitDecision(text);
      if (!decision.allowed) {
        setMessages((prev) => [
          ...prev,
          { id: nextId(), role: "user", text },
          { id: nextId(), role: "assistant", text: "", error: decision.reason },
        ]);
        return false;
      }
      resetCancelState();
    // OD-ARCH-078 S3：记录提交时刻，完成后算「本轮耗时」。
    const startedAtMs = Date.now();
    setMessages((prev) => [...prev, { id: nextId(), role: "user", text }]);
      setBusy(true);
      setStarting(true);
    const turnStartedAtMs = startedAtMs;
      const token = { aborted: false, noteId: null as string | null };
      submitTokenRef.current = token;
      // 工作记录场景（OD-ARCH-089）：意图由本轮输入判定，不再有手动开关。
      // 声明在 try 之外，失败分支要能把「已落盘」和「模型失败」两件事分开说。
      const weeklyScene = preset?.key === "weekly";
      const intent: WorkIntent | null = weeklyScene ? detectWorkIntent(text) : null;
      let worklogNotice: string | null = null;
      try {
        // note = 先由宿主确定性落盘，再让模型只做缺项追问；
        // report = 先把当周记录读出来，模型才有米下锅。
        // 其它场景与普通对话不进入这套流程，行为不变。
        let worklog: { week: string; exists: boolean; content: string } | null = null;
        if (intent === "note") {
          const receipt = await shell.worklogAppend(text);
          if (!receipt.ok || !receipt.appended) {
            // 落盘失败必须停下来说清楚：此时**没有**记下任何东西。
            setBusy(false);
            setMessages((prev) => [
              ...prev,
              {
                id: nextId(),
                role: "assistant",
                text: "",
                error:
                  "这条没有记下来：" +
                  (receipt.error || "工作台没有确认写入结果。") +
                  "。请处理后重发，我不会假装已经记录。",
              },
            ]);
            return false;
          }
          const maskNote = receipt.masked
            ? `已自动脱敏（身份证号 ${receipt.masked_id_card} 处、手机号 ${receipt.masked_phone} 处）。`
            : "";
          worklogNotice =
            `已写入 ${receipt.path ?? receipt.week}` +
            (receipt.continued ? "（续）" : "") +
            "。" +
            maskNote;
        } else if (intent === "report") {
          try {
            const week = await shell.worklogReadWeek();
            worklog = { week: week.week, exists: week.exists, content: week.content };
            worklogNotice = week.exists
              ? `已带上 ${week.week} 的工作记录（${week.content.length} 字符）。`
              : `${week.week} 还没有工作记录，周报只能如实说明这一点。`;
          } catch (e) {
            // 读不到就说读不到：不拿空材料冒充「本周没做事」。
            worklogNotice = "读取当周工作记录失败：" + String(e) + "；本次周报不会包含本周记录。";
          }
        }
        let dir = workspaceDir;
        if (!dir && projectDir) {
          dir = projectDir;
          setWorkspaceDir(dir);
        }
        if (!dir) {
          dir = await resolveWorkspaceDir();
          setWorkspaceDir(dir);
        }
        if (!dir) {
          // 宿主拒绝准备工作目录的真实原因（通常是「模型端点未获许可」），原样带出，
          // 不要用「数据根未解析」这种会误导的通用文案。
          throw new Error(
            workspaceNoteRef.current ??
              "无法为引擎提供工作目录（宿主未返回可用目录）。",
          );
        }
        // 提示词：普通对话发上下文本身；带预设的对话用同一套组装器把场景规则、
        // 期望章节与上文（上一轮对话）一起带进去。
        //
        // OD-ARCH-034：续接既有引擎会话时，**不再重放上文**——上下文由引擎会话自己持有，
        // 只发本轮输入；否则（新会话／旧线程／续接失败降级）仍按窗口化重放，保证历史不丢。
        const continuation = Boolean(effectiveEngineSessionId) && !replayNextTurn;
        const promptText = composeConversationPrompt({
          preset,
          turns: messages.map((m) => ({ role: m.role, text: m.text })),
          // 附件正文进这一轮的用户输入：模型看到的正是用户勾选的文件原文。
          current: composeWithAttachments(text, picked),
          continuation,
          // 意图来自本轮输入（OD-ARCH-089 S1）；其它场景恒为完整组装。
          mode: intent ?? "report",
          // 周报模式带上当周记录（OD-ARCH-089 S2）；随手记不需要。
          worklog,
        });
        // 红线扫描只针对**用户原文**：场景预设里的写作规则含有「案件/线索/未公开」等词，
        // 若把整条提示词交上去扫描，规则本身会被误判成敏感内容（OD-ARCH-021 的口径：
        // 扫描用户原始输入，而不是拼好之后的完整提示词）。
        const receipt = await host.submitTask({
          promptText,
          workspaceDir: dir,
          // OD-ARCH-088：把当前生效场景的 scenario 透传给宿主；无场景时保持既有默认值
          // （不传 intent，由 api 层回落 material_writing）。
          intent: isSceneKey(activeTag) ? SCENES[activeTag].scenario : undefined,
          // 红线扫描必须覆盖附件正文：只扫用户原话会让附件里的禁入内容绕过检查。
          userInputText: picked.length > 0 ? attachmentScanText(text, picked) : text,
          engineSessionId: effectiveEngineSessionId,
        });
        // 记住本线程的引擎会话号：下一轮据此续接同一会话（真多轮）。
        if (receipt.engine_session_id) {
          setLocalEngineSessionId(receipt.engine_session_id);
          onEngineSessionId?.(receipt.engine_session_id);
        }
        if (token.aborted) {
          // 用户在启动期间放弃了这次提交：任务号现在才回来，立刻请求取消。
          const mark = (notice: string) =>
            setMessages((prev) =>
              prev.map((m) => (m.id === token.noteId ? { ...m, notice } : m)),
            );
          mark(`已放弃本次提交；引擎仍返回任务号 ${receipt.omni_task_id}，正在请求取消。`);
          try {
            const outcome = await host.cancelTask(receipt.omni_task_id);
            mark(
              `已放弃本次提交；任务 ${receipt.omni_task_id} 已请求取消：` +
                (outcome.note || outcome.phase),
            );
          } catch (e) {
            mark(
              `已放弃本次提交；任务 ${receipt.omni_task_id} 的取消请求失败（需人工确认）：` +
                String(e),
            );
          }
          return true;
        }
        // 续接失败被降级为新建会话时，这一轮**没有**继承旧会话的上文：
        // 下一轮补一次窗口化重放，把历史接回去（引擎侧此时已持有这一轮）。
        const degraded = receipt.session_mode === "degraded";
        setReplayNextTurn(degraded);
        // OD-ARCH-068：宿主对出站文本做过隐私掩码时如实告知（只报类别与数量）。
        const masked = receipt.privacy_masked;
        const maskParts: string[] = [];
        if (masked && masked.id_card > 0) maskParts.push(`身份证号 ${masked.id_card} 处`);
        if (masked && masked.phone > 0) maskParts.push(`手机号 ${masked.phone} 处`);
        const maskNotice = maskParts.length > 0
          ? `已自动脱敏（${maskParts.join("、")}），原文只保留在本机对话里。`
          : null;
        setMessages((prev) => [
          ...prev,
          {
            id: nextId(),
            role: "assistant",
            text: "",
            taskId: receipt.omni_task_id,
            state: receipt.status,
            startedAtMs: turnStartedAtMs,
            endedAtMs: null,
            // OD-ARCH-069：记住本轮原文，供超时后「重发」用（不额外落盘）。
            sentText: text,
            notice: [
              degraded ? "引擎会话已失效，这一轮改用新会话；下一轮会把上文补回去。" : null,
              // OD-ARCH-089：把本轮走的路与落盘结果说清楚，用户不必猜标签去哪了。
              intent ? describeWorkIntent(intent) + "。" : null,
              worklogNotice,
              maskNotice,
            ]
              .filter(Boolean)
              .join(" ") || null,
          },
        ]);
        setActiveTaskId(receipt.omni_task_id);
        // 这一轮由引擎新建了一个 DSH 会话：把任务号报给上层，便于在工作台里关联到
        // 「项目与会话」里的引擎会话（引擎会话号从任务台账读，不改宿主协议）。
        onTaskCreated?.(receipt.omni_task_id);
        return true;
      } catch (e) {
        setBusy(false);
        setMessages((prev) => [
          ...prev,
          {
            id: nextId(),
            role: "assistant",
            text: "",
            // OD-ARCH-089：随手记先落盘再叫模型。模型这一步失败时，记录**已经写进去了**，
            // 必须把这两件事分开说，否则用户会以为没记上又重发一遍。
            error:
              (intent === "note" && worklogNotice ? worklogNotice + " " : "") + String(e),
          },
        ]);
        // 提交被拒绝常常意味着权限/准入发生了变化：重新读一遍宿主事实。
        // 缓存可能还是旧的（权限刚变），先作废再读。
        invalidateEngineFactsCache();
        void refreshAdmission();
        return false;
      } finally {
        setStarting(false);
      }
    },
    [busy, workspaceDir, resolveWorkspaceDir, refreshAdmission, messages, preset, projectDir, onTaskCreated, effectiveEngineSessionId, onEngineSessionId, replayNextTurn, activeTag],
  );

  /** 切换思考等级：沿用当前 provider/model，只改 reasoningEffort。 */
  const selectEffort = useCallback(
    async (effort: string, byUser = true) => {
      // 用户主动选档：之后不再被场景默认覆盖（OD-ARCH-068）。
      if (byUser) effortTouchedRef.current = true;
      if (!facts?.provider || !facts?.model) {
        setModelNote("当前端点尚未识别，先选一个模型再设思考等级。");
        return;
      }
      const route = modelRoutes.find((g) =>
        g.providers.some((p) => p.id === facts.provider && p.models.includes(facts.model!)),
      );
      setModelEffort(effort);
      await selectModel(route?.profile ?? "", facts.provider, facts.model, effort);
    },
    [facts, modelRoutes, selectModel],
  );

  /** 上一次按场景应用的思考档（场景键 + 档位），避免重复触发引擎重启。 */
  const appliedSceneThinkingRef = useRef<string | null>(null);

  /**
   * 按场景应用默认思考档（OD-ARCH-068 S3）。
   *
   * 三条安全线：
   *   1. 用户手动选过档就不再覆盖（effortTouchedRef）；
   *   2. 只在引擎空闲时切换——切档要经 setModelRoute 重启引擎，生成中切换会把
   *      正在跑的任务打断（OD-ARCH-048/059 的卡死教训）；
   *   3. 同一场景同一档只应用一次，避免每次读端点事实都重启引擎。
   */
  useEffect(() => {
    const wanted = (preset?.thinking ?? "").trim();
    if (!wanted || wanted === "default") return;
    if (effortTouchedRef.current) return;
    if (busy || modelBusy || starting) return;
    if (!facts?.provider || !facts?.model) return;
    if (modelEffort === wanted) return;
    const mark = `${preset?.key ?? ""}:${wanted}`;
    if (appliedSceneThinkingRef.current === mark) return;
    appliedSceneThinkingRef.current = mark;
    setModelNote(`已按场景应用思考等级：${wanted}`);
    void selectEffort(wanted, false);
  }, [preset?.key, preset?.thinking, busy, modelBusy, starting, facts, modelEffort, selectEffort]);

  /**
   * 把一个文件加入本轮引用（OD-ARCH-077 S1）。
   *
   * 路径校验与正文读取都由宿主完成：越界、非白名单类型、超大、非 UTF-8 都在这里可见地失败，
   * 不静默忽略，也不在前端放宽规则。
   */
  const addAttachment = useCallback(async (path: string, fromPicker = false) => {
    setAttachNote(null);
    const clean = path.trim();
    if (!clean) return;
    if (attachmentsRef.current.some((a) => a.path === clean)) {
      setAttachNote(`${clean.split("/").pop()} 已经引用了。`);
      return;
    }
    if (attachmentsRef.current.length >= ATTACHMENT_MAX_FILES) {
      setAttachNote(`一次最多引用 ${ATTACHMENT_MAX_FILES} 个文件。`);
      return;
    }
    try {
      // 来源分两类（OD-ARCH-078 S1）：
      //   * 文件框/拖拽挑来的：可能在数据根外（桌面、下载目录），走宿主附件读取；
      //   * @ 引用挑来的：一定在成果根或库根内，继续走允许根校验。
      const read = fromPicker
        ? await shell.readAttachment(clean)
        : await shell.readReference(clean);
      // 读取期间可能有别的文件先加进来：读完再按最新列表复核一次。
      if (attachmentsRef.current.some((a) => a.path === read.path)) {
        setAttachNote(`${read.file_name} 已经引用了。`);
        return;
      }
      if (attachmentsRef.current.length >= ATTACHMENT_MAX_FILES) {
        setAttachNote(`一次最多引用 ${ATTACHMENT_MAX_FILES} 个文件，${read.file_name} 没有加入。`);
        return;
      }
      const next = [
        ...attachmentsRef.current,
        { path: read.path, fileName: read.file_name, sha256: read.sha256, content: read.content },
      ];
      attachmentsRef.current = next;
      setAttachments(next);
    } catch (e) {
      setAttachNote(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const removeAttachment = useCallback((path: string) => {
    const next = attachmentsRef.current.filter((a) => a.path !== path);
    attachmentsRef.current = next;
    setAttachments(next);
    setAttachNote(null);
  }, []);

  /**
   * 「+」号：打开系统文件选择框（可多选），挑中的文件逐个加入本轮引用。
   *
   * 取消选择什么都不做；选中的文件逐个读取，单个失败只影响它自己，
   * 失败原因显示在输入框下方（不吞掉、不假装读到了）。
   */
  const pickAttachments = useCallback(async () => {
    if (busy) return;
    setAttachNote(null);
    try {
      const picked = await openFileDialog({
        multiple: true,
        title: "选择要引用的文件",
      });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      for (const p of paths) await addAttachment(p, true);
    } catch (e) {
      setAttachNote(
        "打不开文件选择框：" +
          (e instanceof Error ? e.message : String(e)) +
          "；可以改用拖拽，或输入 @ 引用成果目录与库根里的文件。",
      );
    }
  }, [addAttachment, busy]);

  /** @ 查询：只问宿主允许根里的白名单文本文件。 */
  const loadMentions = useCallback(async (query: string) => {
    setMentionBusy(true);
    try {
      const r = await shell.listReferences(query, 30);
      setMentionItems(r.items);
    } catch (e) {
      setMentionItems([]);
      setAttachNote(e instanceof Error ? e.message : String(e));
    } finally {
      setMentionBusy(false);
    }
  }, []);

  useEffect(() => {
    if (mentionQuery === null) {
      setMentionItems([]);
      return;
    }
    const timer = window.setTimeout(() => void loadMentions(mentionQuery), 180);
    return () => window.clearTimeout(timer);
  }, [mentionQuery, loadMentions]);

  // 拖拽：Tauri v2 的 HTML5 drop 被原生拖放接管，路径只能从 webview 事件拿。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDragging(true);
        } else if (event.payload.type === "leave") {
          setDragging(false);
        } else if (event.payload.type === "drop") {
          setDragging(false);
          for (const path of event.payload.paths) void addAttachment(path, true);
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {
        // 拿不到原生拖放事件不是致命问题：按钮与 @ 引用仍可用。
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [addAttachment]);

  const send = useCallback(() => {
    const text = input.trim();
    if (!text) return;
    if (busy) {
      // 上一条还在生成：不静默丢弃，明确告诉用户（否则看起来像「没有回复」）。
      setSendNote("上一条还在生成中，这条先没发出去。等它结束（或点「取消生成」）再发。");
      return;
    }
    setSendNote(null);
    // 附件在提交前统一校验：空文件、数量与总长超限都在这里挡住，不静默截断。
    const check = checkAttachments(attachments);
    if (!check.ok) {
      setSendNote(check.reason);
      return;
    }
    setInput("");
    setMentionQuery(null);
    const picked = attachments;
    // 附件等提交确认受理后再清空：被拒绝或提交失败时，本轮文件仍留在输入区，
    // 用户改一下说法就能直接重发，不用重新挑选。
    void submitText(text, picked).then((accepted) => {
      if (!accepted) return;
      attachmentsRef.current = [];
      setAttachments([]);
    });
  }, [input, busy, attachments, submitText]);

  /** 手动重试：重新进入轮询（已终态的任务会立即走到结果回读）。 */
  const retryQuery = useCallback((taskId: string) => {
    resetCancelState();
    setMessages((prev) =>
      prev.map((m) => (m.taskId === taskId ? { ...m, notice: null, retryable: false } : m)),
    );
    setBusy(true);
    setActiveTaskId(taskId);
  }, []);

  /**
   * 重发（OD-ARCH-069）：把这一轮的原文重新提交一次。
   *
   * 只复用现有发送路径，不改写原文、不自动重试；重发是用户主动动作。
   * 原文只存在于当前对话消息里，不额外落盘。
   */
  const resend = useCallback(
    (text: string) => {
      if (busy) {
        setSendNote("还在生成中，等它结束或点「取消生成」后再重发。");
        return;
      }
      setSendNote(null);
      void submitText(text);
    },
    [busy, submitText],
  );

  /** 一次性提示：同一条消息的后续操作会覆盖上一条提示，避免堆成一片。 */
  const flashTip = useCallback((id: string, text: string) => {
    setTipNote({ id, text });
    window.setTimeout(() => {
      setTipNote((cur) => (cur && cur.id === id && cur.text === text ? null : cur));
    }, 4000);
  }, []);

  /** 复制正文。剪贴板被系统拒绝时如实提示，不假装已复制。 */
  const copyText = useCallback(
    async (id: string, text: string) => {
      try {
        await navigator.clipboard.writeText(text);
        flashTip(id, "已复制");
      } catch (e) {
        flashTip(id, "复制失败（系统未允许剪贴板）：" + String(e));
      }
    },
    [flashTip],
  );

  /** 朗读正文。本机没有可用语音时明确说明，不静默失败。 */
  const speakText = useCallback(
    (id: string, text: string) => {
      const synth = typeof window !== "undefined" ? window.speechSynthesis : undefined;
      if (!synth || typeof window.SpeechSynthesisUtterance !== "function") {
        flashTip(id, "本机没有可用的语音朗读能力。");
        return;
      }
      if (synth.speaking) {
        synth.cancel();
        flashTip(id, "已停止朗读");
        return;
      }
      const utter = new window.SpeechSynthesisUtterance(text);
      utter.onerror = () => flashTip(id, "朗读失败：本机语音引擎没有开始播放。");
      synth.speak(utter);
      flashTip(id, "开始朗读（再点一次停止）");
    },
    [flashTip],
  );

  /**
   * 分支到新对话（OD-ARCH-079）：与 DSH 原生一致，调用引擎的 session/fork，
   * 以这条回复所在轮次的 turn/end 序号为切点建子会话；子会话已继承切点前的上下文，
   * 新线程只保存消息用于展示，后续消息直接续接子会话，不再重放历史。
   */
  const branchToNewThread = useCallback(
    async (id: string, upto: ChatMessage[], atSeq: number | null | undefined) => {
      if (!onBranchNewThread) {
        flashTip(id, "本页面没有接上「新对话」入口，无法分支。");
        return;
      }
      if (busy) {
        flashTip(id, "还在生成中，等这一轮结束再分支。");
        return;
      }
      if (branchBusyRef.current) return;
      if (!effectiveEngineSessionId) {
        flashTip(id, "这条对话还没有引擎会话号，无法在引擎侧分支。");
        return;
      }
      if (typeof atSeq !== "number") {
        flashTip(id, "引擎没有返回本轮切点序号，暂时无法分支。");
        return;
      }
      branchBusyRef.current = true;
      setBranchBusy(true);
      try {
        const fork = await host.forkSession(effectiveEngineSessionId, atSeq);
        if (!fork.engine_session_id) {
          throw new Error("引擎没有返回子会话号");
        }
        branchBusyRef.current = false;
        setBranchBusy(false);
        onBranchNewThread(upto, fork.engine_session_id);
      } catch (e) {
        // 失败就留在原对话，把真实原因说清楚；不退回本地重放。
        flashTip(id, "分支失败：" + String(e instanceof Error ? e.message : e));
      } finally {
        branchBusyRef.current = false;
        setBranchBusy(false);
      }
    },
    [busy, effectiveEngineSessionId, flashTip, onBranchNewThread],
  );

  /** 放弃还没有任务号的提交：只撤销界面等待，拿到任务号后立即请求取消。 */
  const abandonStarting = useCallback(() => {
    const token = submitTokenRef.current;
    if (!token || !starting) return;
    token.aborted = true;
    token.noteId = nextId();
    const noteId = token.noteId;
    setMessages((prev) => [
      ...prev,
      {
        id: noteId,
        role: "assistant",
        text: "",
        notice: "已放弃本次提交（引擎仍在启动，尚未拿到任务号）。拿到任务号后会立即请求取消。",
      },
    ]);
    setEngineCancel({ phase: "starting_abandoned", final_state: null, note: "" });
    setBusy(false);
    setStarting(false);
  }, [starting]);

  /**
   * 取消生成（OD-ARCH-062 S2）：放行判定、防重入与结果解释全部走
   * src/engineCancelModel.ts，与任务工作台页面同一实现；本组件只负责接线。
   */
  const cancel = useCallback(async () => {
    if (!activeTaskId) {
      abandonStarting();
      return;
    }
    await runEngineCancelRequest(
      {
        taskId: activeTaskId,
        status: activeTaskStatus,
        requestInFlight: cancelInFlightRef.current || cancelBusy,
        lastPhase: engineCancel?.phase ?? null,
        requestSent: cancelRequested,
      },
      {
        host,
        onRequestStarted: () => {
          // 同步锁必须早于首个 await，否则快速双击会发出两次取消。
          cancelInFlightRef.current = true;
          setCancelBusy(true);
          setEngineCancel(null);
          setCancelRequested(true);
        },
        onOutcome: (outcome) => setEngineCancel(outcome as CancelOutcome),
        onStatus: (status) => setActiveTaskStatus((status as TaskStatus).status),
        onError: (message) =>
          setMessages((prev) =>
            prev.map((m) =>
              m.taskId === activeTaskId ? { ...m, notice: message } : m,
            ),
          ),
        onSettled: () => {
          cancelInFlightRef.current = false;
          setCancelBusy(false);
        },
      },
    );
  }, [activeTaskId, activeTaskStatus, cancelBusy, engineCancel, cancelRequested, abandonStarting]);

  /**
   * 从提问卡取消本次任务（OD-ARCH-088）。
   *
   * 只发出取消请求并标记「已发送」；提问的最终状态等引擎的 question_closed
   * 或任务终态确认，绝不在这里先写「已取消」。
   */
  const cancelTaskFromQuestion = useCallback(
    async (taskId: string) => {
      setQuestionCancelRequested((prev) => ({ ...prev, [taskId]: true }));
      setQuestions((prev) => {
        const list = prev[taskId];
        if (!list) return prev;
        return {
          ...prev,
          [taskId]: list.map((q) =>
            q.status === "closed" || q.status === "answered" ? q : { ...q, cancelError: undefined },
          ),
        };
      });
      try {
        if (taskId === activeTaskId) {
          await cancel();
        } else {
          await host.cancelTask(taskId);
        }
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setQuestionCancelRequested((prev) => ({ ...prev, [taskId]: false }));
        setQuestions((prev) => {
          const list = prev[taskId];
          if (!list) return prev;
          return {
            ...prev,
            [taskId]: list.map((q) =>
              q.status === "closed" || q.status === "answered" ? q : { ...q, cancelError: message },
            ),
          };
        });
      }
    },
    [activeTaskId, cancel],
  );

  const engineCancelView = engineCancelButtonState({
    hasTask: !!activeTaskId,
    status: activeTaskStatus,
    requestInFlight: cancelBusy,
    lastPhase: engineCancel?.phase ?? null,
    requestSent: cancelRequested,
    starting,
  });
  /** 生成中（含冷启动到拿到任务号之前）：发送按钮此时是取消按钮。 */
  const generating = busy;

  const enabled = facts ? enabledCategories(facts) : [];
  const categoryText = facts ? (CATEGORY_LABEL[facts.category] ?? facts.category) : "类别未知";  /**
   * 权限细节：原先是常驻的整块 banner，现压缩为输入框底部一行状态（OD-ARCH-026 版式回归）。
   * 端点类别/主机、已开启类别与「只承载公开或合成内容」的边界说明放在 `title` 提示里，
   * 仍以宿主回报的事实为准，不在界面上伪造或缓存开关。
   */
  const permissionDetail =
    (admission === "ready" && facts
      ? `端点类别：${categoryText}` +
        (facts.endpoint_host ? `（${facts.endpoint_host}）` : "") +
        `；已开启：${enabled.length > 0 ? enabled.map((c) => CATEGORY_LABEL[c] ?? c).join("、") : "无"}。` +
        "首次使用严格关闭，是否开启由你在设置中逐项决定；宿主在提交时强制执行。"
      : admission === "blocked"
        ? admissionError || "模型权限未开启。"
        : "正在读取宿主事实…") +
    " 本入口只承载公开或合成内容；不接收真实工作材料、案件材料、线索或未公开调查信息。";

  /**
   * 对话框落位（OD-ARCH-072）：没有历史时输入框垂直居中，出现历史后落到底部。
   *
   * 用可过渡的 flex-grow 撑开上方留白（WebKit 支持 flex-grow 过渡），
   * 比按像素算位移稳，也不受窗口高度变化影响。200ms 与计划一致。
   */
  const empty = messages.length === 0;

  /**
   * 右侧轮次索引条（OD-ARCH-079，参考 DSH 原生对话索引）：
   * 每个用户轮次一根刻度/Chat 圆点，点击跳到该轮开头；不新增依赖，也不影响消息本身。
   */
  const turns = useMemo(
    () =>
      messages
        .map((m, index) => ({ m, index }))
        .filter((item) => item.m.role === "user")
        .map((item) => {
          // 这一轮的回答 = 紧随其后的、在下一个问句之前的回答消息；没有就不编。
          const nextUser = messages.findIndex(
            (x, i) => i > item.index && x.role === "user",
          );
          const end = nextUser < 0 ? messages.length : nextUser;
          const answer = messages
            .slice(item.index + 1, end)
            .find((x) => x.role === "assistant");
          return {
            m: item.m,
            index: item.index,
            answer: answer?.text ?? "",
            answerTaskId: answer?.taskId ?? null,
          };
        }),
    [messages],
  );

  /** 找到当前视口顶部（含少量余量）所在的那一轮，用于高亮。 */
  const syncActiveTurn = useCallback(() => {
    const scroller = scrollRef.current;
    if (!scroller || turns.length === 0) return;
    const top = scroller.getBoundingClientRect().top + 12;
    let current = turns[0].m.id;
    for (const { m } of turns) {
      const el = turnAnchors.current[m.id];
      if (!el) continue;
      if (el.getBoundingClientRect().top <= top) current = m.id;
      else break;
    }
    setActiveTurnId((prev) => (prev === current ? prev : current));
  }, [turns]);

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    syncActiveTurn();
    scroller.addEventListener("scroll", syncActiveTurn, { passive: true });
    return () => scroller.removeEventListener("scroll", syncActiveTurn);
  }, [syncActiveTurn, threadKey]);

  const jumpToTurn = useCallback((id: string) => {
    const scroller = scrollRef.current;
    const el = turnAnchors.current[id];
    if (!scroller || !el) return;
    const target =
      scroller.scrollTop +
      el.getBoundingClientRect().top -
      scroller.getBoundingClientRect().top -
      8;
    scroller.scrollTo({ top: Math.max(0, target), behavior: "smooth" });
  }, []);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* 空对话时的上方留白（有历史时收成 0，输入框自然落底） */}
      <div
        aria-hidden="true"
        className="flex-shrink-0 transition-[flex-grow] duration-200 ease-out"
        style={{ flexGrow: empty ? 1 : 0, flexBasis: 0 }}
      />

      {/* 对话流（=对话历史）与右侧轮次索引条 */}
      <div className="relative flex min-h-0 flex-col" style={{ flexGrow: empty ? 0 : 1, flexBasis: 0 }}>
      <div
        ref={scrollRef}
        className="chat-scroll min-h-0 flex-1 overflow-y-auto pr-5 pb-3 space-y-3.5 flex flex-col" style={{ paddingBottom: searchQuery.trim() ? "35vh" : undefined }}
      >
        {messages.map((m, mi) =>
            m.role === "user" ? (
              <div
                key={m.id}
                id={`chat-message-${m.id}`}
                ref={(el) => {
                  turnAnchors.current[m.id] = el;
                }}
                className="flex flex-col items-end max-w-[80%] ml-auto"
              >
                <div className="flex items-start space-x-3">
                  {/* 问句按 Markdown 渲染（OD-ARCH-081 S3）：用户自己贴的列表/代码也能成型；
                      remark-breaks 保留单换行，纯文本问句观感与之前一致。 */}
                  <div className="bg-primary text-on-primary rounded-2xl rounded-tr-sm px-4 py-2.5 text-[15px] shadow-sm leading-relaxed min-w-0">
                    <MarkdownText text={m.text} tone="onPrimary" highlightQuery={searchQuery} />
                  </div>
                  <div className="w-7 h-7 rounded-full bg-surface-container-high ring-1 ring-primary/20 flex items-center justify-center text-primary flex-shrink-0">
                    <Icon name="brand" className="w-4 h-4" />
                  </div>
                </div>
                {/* 问句 tips：只保留实际可用的「复制」。撤回/修改需要引擎能删轮，
                    当前适配器没有该接口，按用户要求先去掉，不做假动作。 */}
                <div className="mt-1 flex items-center gap-0.5 pr-10" data-testid="user-message-tips">
                  <TipButton icon="copy" label="复制" onClick={() => void copyText(m.id, m.text)} />
                  {tipNote?.id === m.id ? (
                    <span className="ml-1.5 text-[10.5px] text-outline">{tipNote.text}</span>
                  ) : null}
                </div>
              </div>
            ) : (
              <div
                key={m.id}
                id={`chat-message-${m.id}`}
                className="flex items-start space-x-3 max-w-[80%]"
              >
                <div className="w-7 h-7 rounded-full bg-gradient-to-tr from-primary to-secondary flex items-center justify-center text-white flex-shrink-0 shadow-xs">
                  <Icon name="bot" className="w-[15px] h-[15px]" />
                </div>
                <div className="plate-sculpted rounded-2xl rounded-tl-sm p-4 text-[15px] text-on-surface shadow-sm space-y-2.5 min-w-0">
                  <div className="flex items-center space-x-2 text-primary font-medium">
                    <Icon name="shield-check" className="w-[15px] h-[15px]" />
                    <span>
                      模型输出 · 未复核
                      {m.state ? " · " + STATE_LABEL[m.state] : ""}
                      {m.taskId && (pendingApprovals[m.taskId] ?? 0) > 0 ? " · 等待审批中" : ""}
                      {m.taskId && activeTaskId === m.taskId && elapsed > 0 ? ` · 已等待 ${elapsed}s` : ""}
                      {m.taskId && ttfb[m.taskId] !== undefined
                        ? ` · 首字 ${ttfb[m.taskId].toFixed(1)}s`
                        : ""}
                      {m.stopReason ? " · stop=" + m.stopReason : ""}
                    </span>
                  </div>

                  {m.taskId && (approvals[m.taskId] ?? []).length > 0 ? (
                    <div className="space-y-2">
                      {(approvals[m.taskId] ?? []).map((a) => (
                        <div
                          key={a.approvalId}
                          className="rounded-lg bg-surface-container px-3 py-2.5 space-y-2 text-[12px] leading-relaxed"
                          data-testid="approval-card"
                        >
                          <div className="flex items-start space-x-2">
                            <Icon name="shield-check" className="w-4 h-4 text-primary flex-shrink-0 mt-0.5" />
                            <div className="min-w-0 flex-1">
                              <p className="text-on-surface font-medium">
                                这一步需要你确认：{a.toolName || "未知工具"}
                              </p>
                              <p className="text-on-surface-variant">
                                {a.reason || "引擎没有给出原因。"}
                              </p>
                              {a.callId ? (
                                <p className="text-[10.5px] text-outline">请求编号 {a.callId}</p>
                              ) : null}
                            </div>
                          </div>
                          {a.status === "pending" ? (
                            <div className="flex items-center space-x-2">
                              <button
                                type="button"
                                disabled={approvalBusy === a.approvalId}
                                onClick={() => void respondApproval(m.taskId as string, a.approvalId, true)}
                                className="px-3 py-1 rounded-full bg-primary text-on-primary text-[11.5px] font-medium disabled:opacity-50"
                              >
                                允许这一次
                              </button>
                              <button
                                type="button"
                                disabled={approvalBusy === a.approvalId}
                                onClick={() => void respondApproval(m.taskId as string, a.approvalId, false)}
                                className="px-3 py-1 rounded-full bg-surface-container-high text-on-surface text-[11.5px] font-medium disabled:opacity-50"
                              >
                                拒绝
                              </button>
                              {approvalBusy === a.approvalId ? (
                                <span className="text-[11px] text-outline">正在回传…</span>
                              ) : null}
                            </div>
                          ) : (
                            <p className="text-[11px] text-outline">
                              {a.status === "allowed"
                                ? "已允许这一次，任务继续。"
                                : a.status === "rejected"
                                  ? "已拒绝；引擎按自己的语义结束本轮。"
                                  : "该请求已失效（引擎撤回了它）。"}
                            </p>
                          )}
                          {a.error ? (
                            <p className="text-[11px] text-error">
                              回传失败，仍待决定：{a.error}
                            </p>
                          ) : null}
                        </div>
                      ))}
                    </div>
                  ) : null}

                  {m.taskId ? (
                    (questions[m.taskId] ?? []).map((q) => (
                      <QuestionCard
                        key={q.questionId}
                        request={q}
                        onSubmit={(taskId, questionId, answers) =>
                          void respondQuestion(taskId, questionId, answers)
                        }
                        onCancelTask={(taskId) => void cancelTaskFromQuestion(taskId)}
                        cancelRequested={questionCancelRequested[q.omniTaskId] === true}
                      />
                    ))
                  ) : null}

                  {m.taskId && thoughts[m.taskId] ? (
                    <details
                      className="rounded-lg bg-surface-container/70 px-3 py-2 text-[11.5px] text-on-surface-variant"
                      open
                    >
                      <summary className="cursor-pointer select-none text-outline">
                        思考过程（模型产出，不计入最终结果）
                      </summary>
                      <p className="pt-1.5 whitespace-pre-wrap break-words leading-relaxed">
                        {thoughts[m.taskId]}
                      </p>
                    </details>
                  ) : m.taskId && m.state && ["succeeded", "failed", "cancelled"].includes(String(m.state)) ? (
                    <p className="text-[10.5px] text-outline">
                      本轮没有思考内容（该模型/端点可能不返回思考过程，或思考等级为关闭）。
                    </p>
                  ) : null}

                  {m.error ? (
                    <div className="rounded-lg bg-error-container text-on-error-container px-3 py-2 text-[12px] leading-relaxed">
                      {m.error}
                    </div>
                  ) : !m.text && m.state === "succeeded" ? (
                    <div className="rounded-lg bg-surface-container px-3 py-2 text-[12px] leading-relaxed text-on-surface-variant">
                      引擎报告已完成，但没有返回任何正文。若界面此时空白，请点「重新查询」；若反复如此，
                      把这次任务号告诉我（可在应用数据根的 engine-state 里查到）。
                    </div>
                  ) : m.text ? (
                  <div className="text-on-surface-variant leading-relaxed">
                      <AssistantText
                        text={m.text}
                        live={!!m.taskId && !(TERMINAL_STATES as string[]).includes(String(m.state ?? ""))}
                        streamed={!!m.taskId && streamedTasksRef.current.has(m.taskId)}
                        highlightQuery={searchQuery}
                      />
                  </div>
                  ) : m.notice ? (
                    <p className="text-outline leading-relaxed">未收到可显示的输出。</p>
                  ) : (
                    <p className="text-outline leading-relaxed">正在生成…</p>
                  )}

                  {m.toolNote ? (
                    <p className="flex items-center space-x-1.5 text-[11.5px] text-outline">
                      <Icon name="spark" className="w-3.5 h-3.5 animate-pulse" />
                      <span>{m.toolNote}</span>
                    </p>
                  ) : null}

                  {m.notice ? (
                    <div className="rounded-lg bg-surface-container-low text-on-surface-variant px-3 py-2 text-[12px] leading-relaxed space-y-1.5">
                      <p>{m.notice}</p>
                      <div className="flex flex-wrap items-center gap-1.5">
                        {m.retryable && m.taskId ? (
                          <button
                            onClick={() => m.taskId && retryQuery(m.taskId)}
                            className="inline-flex items-center space-x-1 px-2 py-0.5 rounded-full bg-surface-container text-primary font-label-caps text-[10px] hover:bg-surface-container-high transition-colors"
                          >
                            <span>重新查询</span>
                          </button>
                        ) : null}
                        {/* OD-ARCH-069：超时/中断后由用户主动重发，不自动重试。 */}
                        {m.sentText && m.state === "unknown" ? (
                          <button
                            onClick={() => resend(m.sentText as string)}
                            disabled={busy}
                            className="inline-flex items-center space-x-1 px-2 py-0.5 rounded-full bg-primary text-on-primary font-label-caps text-[10px] disabled:opacity-50 hover:opacity-90 transition-opacity"
                          >
                            <span>重发</span>
                          </button>
                        ) : null}
                      </div>
                    </div>
                  ) : null}
                  {/* 答句 tips（OD-ARCH-078 S3 / OD-ARCH-079）：贴在本条回复正文下方。
                      分支走 DSH 原生 session/fork，没有切点序号时按钮禁用并说明原因。 */}
                  {m.text ? (
                    <div className="pt-0.5 flex flex-wrap items-center gap-0.5" data-testid="assistant-message-tips">
                      <TipButton icon="copy" label="复制" onClick={() => void copyText(m.id, m.text)} />
                      <TipButton icon="volume" label="朗读" onClick={() => speakText(m.id, m.text)} />
                      <TipButton
                        icon="refresh"
                        label="重新生成"
                        disabled={busy || !m.sentText}
                        title={
                          !m.sentText
                            ? "这一轮没有留下用户原文，无法重新生成。"
                            : busy
                              ? "还有任务在生成中，结束后再重新生成。"
                              : "用同一段用户原文重新提交一次（不自动重试）。"
                        }
                        onClick={() => m.sentText && resend(m.sentText)}
                      />
                      <TipButton
                        icon="branch"
                        label="分支到新对话"
                        disabled={
                          busy || branchBusy || !onBranchNewThread || typeof m.forkSeq !== "number" || !effectiveEngineSessionId
                        }
                        title={
                          typeof m.forkSeq !== "number"
                            ? "引擎没有返回本轮切点序号，暂时无法分支。"
                            : !effectiveEngineSessionId
                              ? "这条对话还没有引擎会话号，无法在引擎侧分支。"
                              : "从这条回复分出一条新对话：引擎侧继承到这里为止的上下文，之后各自继续。"
                        }
                        onClick={() => void branchToNewThread(m.id, messages.slice(0, mi + 1), m.forkSeq)}
                      />
                      {elapsedText(m.startedAtMs, m.endedAtMs) ? (
                        <span
                          className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] text-outline"
                          title="从提交到本轮结束的实际等待时间，含排队、生成、工具调用和结果回读。"
                        >
                          <Icon name="clock" className="w-[13px] h-[13px]" />
                          <span>{elapsedText(m.startedAtMs, m.endedAtMs)}</span>
                        </span>
                      ) : null}
                      {tipNote?.id === m.id ? (
                        <span className="ml-1 text-[10.5px] text-outline">{tipNote.text}</span>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </div>
            )
        )}
      </div>
        <TurnRail
          items={turns.map(({ m, answer }) => ({ id: m.id, prompt: m.text, response: answer }))}
          activeId={activeTurnId}
          busyId={
            busy
              ? (turns.find(({ answerTaskId }) => answerTaskId && answerTaskId === activeTaskId)?.m
                  .id ?? null)
              : null
          }
          onJump={jumpToTurn}
        />
      </div>

      {/* 指令坞 */}
      {/* 输入框（对话框） */}
      <div className="plate-sculpted p-3 sm:p-3.5 shadow-[0_16px_40px_-10px_rgba(15,23,42,0.06)] border border-white flex-shrink-0 rounded-xl">
        {/* @ 引用选择器（OD-ARCH-077 S1）：贴着输入框显示，只列宿主允许根内的白名单文本。
            来源只有“成果目录”和“库根”，越界路径根本不会出现在这里。 */}
        {mentionQuery !== null ? (
          <div
            className="mb-2 rounded-lg border border-surface-variant/40 bg-surface-container-low p-1 max-h-56 overflow-y-auto"
            data-testid="mention-list"
          >
            <p className="px-2 py-1 text-[10.5px] text-outline">
              {mentionBusy
                ? "正在查找文件…"
                : mentionItems.length === 0
                  ? `没有匹配“${mentionQuery}”的文本文件（只查成果目录与库根，支持 md/txt/csv/tsv/json/yaml/log）`
                  : "选择一个文件加入本轮引用（Esc 关闭）"}
            </p>
            {mentionItems.map((item) => (
              <button
                key={item.path}
                onClick={() => {
                  setInput(applyMentionPick(input, item.file_name));
                  setMentionQuery(null);
                  void addAttachment(item.path);
                }}
                className="w-full text-left px-2 py-1 rounded hover:bg-surface-container"
                title={item.path}
              >
                <span className="text-[12px] text-on-surface">{item.file_name}</span>
                <span className="ml-2 text-[10.5px] text-outline">
                  {rootKindOf(item.source_root)} · {item.extension}
                </span>
              </button>
            ))}
          </div>
        ) : null}
        <div className="flex items-center space-x-3 px-1">
          {/* 品牌标（OD-ARCH-081 S2）：原先是无动作的 spark 装饰图标，换成 OmniDesk
              标识。纯装饰，不承担点击；「+」才是附件入口。 */}
          <BrandMark className="w-[22px] h-[22px] flex-shrink-0" />
          {/*
            「+」附件入口（OD-ARCH-078 S1）：点开系统文件选择框，可多选；
            与拖拽走同一条读取路径。只挑文件，不给前端文件系统读写权限。
          */}
          <button
            type="button"
            onClick={() => void pickAttachments()}
            disabled={busy}
            title={`添加文件（最多 ${ATTACHMENT_MAX_FILES} 个）：也可以直接拖进来，或输入 @ 引用成果目录/库根`}
            aria-label="添加文件"
            data-testid="attachment-pick"
            className="w-7 h-7 rounded-full plate-soft-inset text-on-surface-variant hover:text-primary hover:bg-surface-container flex items-center justify-center flex-shrink-0 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Icon name="plus" className="w-[15px] h-[15px]" />
          </button>
          <input
            className="w-full bg-transparent border-0 p-0 text-[14px] font-body-md text-on-surface placeholder:text-outline/60 focus:ring-0 focus:outline-none"
            placeholder={
              canSubmit
                ? "与 OmniDesk 协同探索、下达任务或记录灵感…"
                : factsLoading && !facts
                  ? "正在读取模型端点信息…"
                  : "模型权限未开启：可在设置中确认端点后启用"
            }
            value={input}
            disabled={busy}
            onChange={(e) => {
              setInput(e.target.value);
              setMentionQuery(activeMentionQuery(e.target.value));
            }}
            onFocus={() => reportInputFocus(true)}
            onBlur={() => reportInputFocus(false)}
            onKeyDown={(e) => {
              if (mentionQuery !== null && e.key === "Escape") {
                e.preventDefault();
                setMentionQuery(null);
                return;
              }
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
          />
        </div>
        {/* 附件 chip（OD-ARCH-077 S1）：排在输入框下方，可逐个移除；
            只显示文件名，完整路径与哈希在悬停提示里。正文不落盘，仅随本轮发出。 */}
        {attachments.length > 0 ? (
          <div className="mt-2 flex flex-wrap items-center gap-1.5 px-1" data-testid="attachment-chips">
            {attachments.map((a) => (
              <span
                key={a.path}
                className="inline-flex items-center gap-1 max-w-[240px] px-2 py-0.5 rounded-full bg-surface-container text-[11px] text-on-surface-variant"
                title={`${a.path}\nsha256: ${a.sha256}\n${a.content.length} 字，随本轮发送，不单独存盘`}
              >
                <Icon name="note" className="w-[12px] h-[12px] flex-shrink-0" />
                <span className="truncate">{a.fileName}</span>
                <button
                  onClick={() => removeAttachment(a.path)}
                  disabled={busy}
                  className="text-outline hover:text-error transition-colors disabled:opacity-40"
                  title="移除这个附件"
                  aria-label={`移除附件 ${a.fileName}`}
                >
                  ×
                </button>
              </span>
            ))}
            <span className="text-[10.5px] text-outline">
              {attachments.length} 个文件，正文随本轮发送
            </span>
          </div>
        ) : null}
        {dragging ? (
          <p className="mt-2 px-1 text-[11px] text-primary">
            松开即可引用文件（{ATTACHMENT_MAX_FILES} 个以内，只收文本与表格）
          </p>
        ) : null}
        {attachNote ? (
          <p className="mt-2 px-1 text-[11px] text-error" data-testid="attachment-note">
            {attachNote}
          </p>
        ) : null}
        {voiceNote ? (
          <p className="mt-2 px-1 text-[11px] text-outline" data-testid="voice-note">
            语音输入需要配置语音模型端点，当前版本未开放。可以先用系统听写或文字输入。
          </p>
        ) : null}
        <div className="mt-2.5 pt-2 border-t border-surface-variant/30 flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center space-x-1.5">
            {/* 对话归类（OD-ARCH-063 S1）：单枚下拉胶囊（色点 + 当前分类 + 箭头），
                点开是勾选列表。分类可新增，平铺会吃满整行，故收进下拉。
                选项仍由 Profile 声明联动——声明里隐藏的场景不会出现在列表里。
                归类只是本机对话的组织方式，不改模型、不改路由。 */}
            {sceneOptions && sceneOptions.length > 0 ? (
              <CategoryPicker
                options={sceneOptions}
                value={activeTag}
                onChange={(key) => onChangeTag?.(key)}
                disabled={busy}
              />
            ) : null}
            {/* 历史对话切换在场景页左栏（OD-ARCH-059 复核修正）。
                这里原先还有第二个下拉框，与场景框并排容易被读成「两级分类」，已移除。 */}
            {onNewThread ? (
              <button
                onClick={onNewThread}
                disabled={busy}
                className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft-inset text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container"
                title="开一段新对话（当前这段已自动保存在本机）"
              >
                <Icon name="note" className="w-[13px] h-[13px]" />
                <span>新对话</span>
              </button>
            ) : null}
            {/* 语音（OD-ARCH-077 S2）：当前没有语音端点，如实说明，不再灰成“即将可用”。 */}
            <button
              onClick={() => setVoiceNote((v) => !v)}
              title="语音输入尚未开放：需要先配置语音模型端点"
              aria-expanded={voiceNote}
              data-testid="voice-button"
              className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft-inset text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container"
            >
              <Icon name="mic" className="w-[13px] h-[13px]" />
              <span>语音</span>
            </button>
          </div>
          <div className="flex items-center space-x-2">
            {/* 模型选择（自绘下拉 + 思考等级）：与工作台同一套视觉。 */}
            <ModelPicker
              currentProvider={facts?.provider ?? null}
              currentModel={facts?.model ?? null}
              groups={modelRoutes}
              catalog={modelCatalog}
              effort={modelEffort}
              busy={modelBusy || busy}
              loading={factsLoading && !facts}
              onSelectModel={(profile, provider, model) => void selectModel(profile, provider, model)}
              onSelectEffort={(effort) => void selectEffort(effort)}
              onFollowDsh={() => void followDsh()}
            />
            {/*
              发送与取消是同一个按钮（对齐 DSH 交互）：
              空闲时是箭头，点即发送；生成中变成方块，点即取消。
              放行判定仍由 engineCancelModel 统一决定，按钮外观不替代该判定。
            */}
            <button
              onClick={generating ? cancel : send}
              disabled={
                generating
                  ? !engineCancelView.enabled
                  : !canSubmit || !input.trim()
              }
              className="w-8 h-8 rounded-full bg-primary text-on-primary flex items-center justify-center hover:bg-primary-container active:scale-95 shadow-sm transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              title={
                generating
                  ? (engineCancelView.hint ?? engineCancelView.label)
                  : canSubmit
                    ? "发送指令"
                    : "当前权限下不发送输入"
              }
              aria-label={generating ? engineCancelView.label : "发送指令"}
            >
              <Icon name={generating ? "stop" : "arrow-up"} className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {modelNote ? <p className="text-[10.5px] text-outline pb-1">{modelNote}</p> : null}
      {sendNote ? <p className="text-[10.5px] text-outline pb-1">{sendNote}</p> : null}
      {starting ? (
        <p className="text-[10.5px] text-outline pb-1">
          正在启动引擎并提交（首次启动可能要几秒）…
        </p>
      ) : null}

      {/* OD-ARCH-088 S4：Skill 未预注入的低干扰说明（同一会话只出现一次）。 */}
      {skillHint ? (
        <p
          className="text-[10.5px] text-outline pb-1"
          role="status"
          aria-label={skillHint}
          data-testid="skill-not-injected-hint"
        >
          {skillHint}
        </p>
      ) : null}

      {/* 底部状态行：模型权限 + Token 上下文用量，一行显示。
          顶部「LOCAL IPC ACTIVE」表示引擎 IPC 健康；这里表示权限与用量，含义不同。
          Token 只显示引擎通过 progress/usage 上报的真实 used/size，不自行估算。 */}
      <div className="mt-1.5 flex flex-wrap items-center justify-center gap-x-5 gap-y-1 px-1 flex-shrink-0">
        <span className="inline-flex items-center gap-1.5 min-w-0">
          <Icon name="shield-check" className="w-[13px] h-[13px] text-primary/80 flex-shrink-0" />
          <span className="font-label-caps text-[10px] text-outline truncate" title={permissionDetail}>
            {admission === "checking"
              ? "模型权限：正在读取宿主事实…"
              : admission === "ready"
                ? `模型权限：已开启 · ${categoryText}`
                : facts && facts.configurable
                  ? `模型权限：未开启（当前端点：${categoryText}）——请到设置里开启对应类别`
                  : "模型权限：未开启，提交已关闭"}
          </span>
          {admission === "blocked" ? (
            <button
              onClick={() => onNavigate("settings")}
              className="shrink-0 inline-flex items-center px-2 py-0.5 rounded-full bg-surface-container text-primary font-label-caps text-[10px] hover:bg-surface-container-high transition-colors"
            >
              去设置
            </button>
          ) : null}
        </span>
        <span
          className="inline-flex items-center gap-1.5 min-w-0"
          title="引擎上报的上下文用量（used / size tokens）；未上报时显示「—」，不自行估算。"
        >
          <Icon name="trend-up" className="w-[13px] h-[13px] text-primary/80 flex-shrink-0" />
          <span className="font-label-caps text-[10px] text-outline tabular-nums truncate">
            {usage
              ? `Token 上下文：${usage.used.toLocaleString()} / ${usage.size.toLocaleString()}`
              : "Token 上下文：—（引擎未上报）"}
          </span>
        </span>
        {/* 没加载 Profile 规则 = 纯净底座：必须说清楚，不能让用户以为拦截还在（OD-ARCH-068）。 */}
        {admission === "ready" && preset !== null ? (
          <span
            className="font-label-caps text-[10px] text-outline truncate"
            title="规则资产来自应用数据根下的 profiles/discipline.rules.json；可在设置 →「规则资产」查看和修改。"
          >
            规则：{preset.rulesLoaded ? "已加载 Profile 规则" : "未加载（纯净底座）"}
          </span>
        ) : null}
        {engineCancel !== null ? (
          <span
            className="font-label-caps text-[10px] text-outline truncate"
            title={describeEngineCancelOutcome(engineCancel).message}
          >
            取消：{describeEngineCancelOutcome(engineCancel).message}
          </span>
        ) : null}
        {workspaceNote ? (
          <span className="font-label-caps text-[10px] text-outline truncate" title={workspaceNote}>
            工作目录：{workspaceNote}
          </span>
        ) : null}
      </div>

      {/* 空对话时的下方留白：与上方留白等分，输入区才真正落在竖直中部
          （只留上方一条会把输入框顶到底部——OD-ARCH-072 首轮实测发现）。 */}
      <div
        aria-hidden="true"
        className="flex-shrink-0 transition-[flex-grow] duration-200 ease-out"
        style={{ flexGrow: empty ? 1 : 0, flexBasis: 0 }}
      />
    </div>
  );
}
