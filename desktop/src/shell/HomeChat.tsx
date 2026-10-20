import { useCallback, useEffect, useRef, useState } from "react";
import { host, onTaskEvent, shell } from "../api";
import { composeConversationPrompt } from "../conversationPrompt";
import { currentEngineSubmitDecision } from "../engineAdmission";
import { TERMINAL_STATES, type AdmissionFacts, type TaskState } from "../types";
import { Icon } from "../components/Icon";
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
const POLL_INTERVAL_MS = 1500;
/** 连续失败多少次后停止自动轮询，避免界面永久停在「生成中」。 */
const MAX_POLL_FAILURES = 5;
/** 单个任务最长等待（毫秒）。超时不代表成功，也不代表已终止。 */
const MAX_WAIT_MS = 10 * 60 * 1000;
/** 终态后读取结果的重试次数。 */
const RESULT_RETRIES = 3;

type AdmissionState = "checking" | "ready" | "blocked";

let seq = 0;
const nextId = () => "m" + ++seq;

function enabledCategories(facts: AdmissionFacts): string[] {
  const p = facts.permissions;
  return (["local", "private", "public"] as const).filter((k) => p[k]);
}

export interface HomeChatProps {
  onNavigate: (scene: Scene) => void;
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
}

export function HomeChat({
  onNavigate,
  preset = null,
  projectDir = null,
  threadKey = "default",
  initialMessages = [],
  onMessagesChange,
  onTaskCreated,
}: HomeChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [cancelNote, setCancelNote] = useState<string | null>(null);
  const [workspaceDir, setWorkspaceDir] = useState<string | null>(null);
  const [workspaceNote, setWorkspaceNote] = useState<string | null>(null);
  /** 引擎上报的上下文用量（ACP `usage_update` → progress/usage 的 used/size）；未上报时为 null。 */
  const [usage, setUsage] = useState<{ used: number; size: number } | null>(null);
  const [admission, setAdmission] = useState<AdmissionState>("checking");
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);
  const [admissionError, setAdmissionError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // 切换会话（threadKey 变化）时载入该会话的消息。
  useEffect(() => {
    setMessages(initialMessages);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadKey]);

  // 消息变化回传给工作区（本地持久化）。
  useEffect(() => {
    onMessagesChange?.(messages);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages]);

  const canSubmit = admission === "ready" && !busy;

  const refreshAdmission = useCallback(async () => {
    try {
      const status = await host.admissionStatus();
      if (!status.ok || !status.facts) {
        setFacts(null);
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
      setFacts(null);
      setAdmission("blocked");
      setAdmissionError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refreshAdmission();
  }, [refreshAdmission]);

  // 引擎要求 workspace_dir 为绝对路径。宿主在端点未获准时会拒绝准备，
  // 此时如实记录拒绝原因并把工作目录置空，不用「仅解析出的路径」冒充可用目录。
  const resolveWorkspaceDir = useCallback(async (): Promise<string | null> => {
    try {
      const prep = await shell.prepareWorkspaceDir();
      if (prep.ok && prep.workspace_dir) {
        setWorkspaceNote(null);
        return prep.workspace_dir;
      }
      setWorkspaceNote(prep.error ?? "宿主未返回可用的工作目录。");
      return null;
    } catch (e) {
      setWorkspaceNote(e instanceof Error ? e.message : String(e));
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
        const p = event.payload as { kind?: unknown; used?: unknown; size?: unknown };
        if (p.kind === "usage" && typeof p.used === "number" && typeof p.size === "number") {
          setUsage({ used: p.used, size: p.size });
        }
        return;
      }
      if (event.type !== "text_delta") return;
      const delta = String((event.payload as { text?: unknown }).text ?? "");
      if (!delta) return;
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

  // 轮询终态：有界失败与有界等待，不静默吞错、不把未知状态写成成功。
  useEffect(() => {
    if (!activeTaskId) return;
    const taskId = activeTaskId;
    let stopped = false;
    let failures = 0;
    const startedAt = Date.now();

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
      if (Date.now() - startedAt > MAX_WAIT_MS) {
        settle();
        markNotice(
          "等待已超过 " +
            Math.round(MAX_WAIT_MS / 60000) +
            " 分钟：任务可能仍在后台运行，状态未确认。可点『重新查询』继续。",
        );
        return;
      }
      try {
        const status = await host.taskStatus(taskId);
        if (stopped) return;
        failures = 0;
        setMessages((prev) =>
          prev.map((m) =>
            m.taskId === taskId ? { ...m, state: status.status, stopReason: null } : m,
          ),
        );
        if (TERMINAL_STATES.includes(status.status)) {
          settle();
          await loadResult(taskId, status.status);
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

  const submitText = useCallback(
    async (raw: string) => {
      const text = raw.trim();
      if (!text || busy) return;
      const decision = currentEngineSubmitDecision(text);
      if (!decision.allowed) {
        setMessages((prev) => [
          ...prev,
          { id: nextId(), role: "user", text },
          { id: nextId(), role: "assistant", text: "", error: decision.reason },
        ]);
        return;
      }
      setCancelNote(null);
      setMessages((prev) => [...prev, { id: nextId(), role: "user", text }]);
      setBusy(true);
      try {
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
          throw new Error("本地数据根未解析，无法为引擎提供工作目录（请检查应用数据根设置）。");
        }
        // 提示词：普通对话发上下文本身；带预设的对话用同一套组装器把场景规则、
        // 期望章节与上文（上一轮对话）一起带进去。
        const promptText = composeConversationPrompt({
          preset,
          turns: messages.map((m) => ({ role: m.role, text: m.text })),
          current: text,
        });
        const receipt = await host.submitTask({ promptText, workspaceDir: dir });
        setMessages((prev) => [
          ...prev,
          {
            id: nextId(),
            role: "assistant",
            text: "",
            taskId: receipt.omni_task_id,
            state: receipt.status,
          },
        ]);
        setActiveTaskId(receipt.omni_task_id);
        // 这一轮由引擎新建了一个 DSH 会话：把任务号报给上层，便于在工作台里关联到
        // 「项目与会话」里的引擎会话（引擎会话号从任务台账读，不改宿主协议）。
        onTaskCreated?.(receipt.omni_task_id);
      } catch (e) {
        setBusy(false);
        setMessages((prev) => [
          ...prev,
          { id: nextId(), role: "assistant", text: "", error: String(e) },
        ]);
        // 提交被拒绝常常意味着权限/准入发生了变化：重新读一遍宿主事实。
        void refreshAdmission();
      }
    },
    [busy, workspaceDir, resolveWorkspaceDir, refreshAdmission, messages, preset, projectDir, onTaskCreated],
  );

  const send = useCallback(() => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    void submitText(text);
  }, [input, submitText]);

  /** 手动重试：重新进入轮询（已终态的任务会立即走到结果回读）。 */
  const retryQuery = useCallback((taskId: string) => {
    setCancelNote(null);
    setMessages((prev) =>
      prev.map((m) => (m.taskId === taskId ? { ...m, notice: null, retryable: false } : m)),
    );
    setBusy(true);
    setActiveTaskId(taskId);
  }, []);

  const cancel = useCallback(async () => {
    if (!activeTaskId) return;
    try {
      const outcome = await host.cancelTask(activeTaskId);
      setCancelNote(outcome.note || "");
    } catch (e) {
      setCancelNote(String(e));
    }
  }, [activeTaskId]);

  const enabled = facts ? enabledCategories(facts) : [];
  const categoryText = facts ? (CATEGORY_LABEL[facts.category] ?? facts.category) : "类别未知";
  /**
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

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* 对话流 */}
      <div
        ref={scrollRef}
        className="chat-fade-mask chat-scroll flex-1 min-h-0 overflow-y-auto pr-2 pb-3 space-y-3.5 flex flex-col"
      >
        {messages.map((m) =>
            m.role === "user" ? (
              <div key={m.id} className="flex items-start justify-end space-x-3 max-w-2xl ml-auto">
                <div className="bg-primary text-on-primary rounded-2xl rounded-tr-sm px-4 py-2.5 text-[13px] shadow-sm leading-relaxed whitespace-pre-wrap">
                  {m.text}
                </div>
                <div className="w-7 h-7 rounded-full bg-surface-container-high ring-1 ring-primary/20 flex items-center justify-center text-primary flex-shrink-0">
                  <Icon name="brand" className="w-4 h-4" />
                </div>
              </div>
            ) : (
              <div key={m.id} className="flex items-start space-x-3 max-w-3xl">
                <div className="w-7 h-7 rounded-full bg-gradient-to-tr from-primary to-secondary flex items-center justify-center text-white flex-shrink-0 shadow-xs">
                  <Icon name="bot" className="w-[15px] h-[15px]" />
                </div>
                <div className="plate-sculpted rounded-2xl rounded-tl-sm p-4 text-[13px] text-on-surface shadow-sm space-y-2.5 min-w-0">
                  <div className="flex items-center space-x-2 text-primary font-medium">
                    <Icon name="shield-check" className="w-[15px] h-[15px]" />
                    <span>
                      模型输出 · 未复核
                      {m.state ? " · " + STATE_LABEL[m.state] : ""}
                      {m.stopReason ? " · stop=" + m.stopReason : ""}
                    </span>
                  </div>

                  {m.error ? (
                    <div className="rounded-lg bg-error-container text-on-error-container px-3 py-2 text-[12px] leading-relaxed">
                      {m.error}
                    </div>
                  ) : m.text ? (
                    <p className="text-on-surface-variant leading-relaxed whitespace-pre-wrap break-words">
                      {m.text}
                    </p>
                  ) : m.notice ? (
                    <p className="text-outline leading-relaxed">未收到可显示的输出。</p>
                  ) : (
                    <p className="text-outline leading-relaxed">正在生成…</p>
                  )}

                  {m.notice ? (
                    <div className="rounded-lg bg-surface-container-low text-on-surface-variant px-3 py-2 text-[12px] leading-relaxed space-y-1.5">
                      <p>{m.notice}</p>
                      {m.retryable && m.taskId ? (
                        <button
                          onClick={() => m.taskId && retryQuery(m.taskId)}
                          className="inline-flex items-center space-x-1 px-2 py-0.5 rounded-full bg-surface-container text-primary font-label-caps text-[10px] hover:bg-surface-container-high transition-colors"
                        >
                          <span>重新查询</span>
                        </button>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </div>
          ),
        )}
      </div>

      {/* 指令坞 */}
      <div className="plate-sculpted p-3 sm:p-3.5 shadow-[0_16px_40px_-10px_rgba(15,23,42,0.06)] border border-white flex-shrink-0 rounded-xl">
        <div className="flex items-center space-x-3 px-1">
          <Icon name="spark" className="w-5 h-5 text-primary flex-shrink-0" />
          <input
            className="w-full bg-transparent border-0 p-0 text-[14px] font-body-md text-on-surface placeholder:text-outline/60 focus:ring-0 focus:outline-none"
            placeholder={
              canSubmit ? "与 OmniDesk 协同探索、下达任务或记录灵感…" : "模型权限未开启：可在设置中确认端点后启用"
            }
            value={input}
            disabled={busy}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
          />
        </div>
        <div className="mt-2.5 pt-2 border-t border-surface-variant/30 flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center space-x-1.5">
            <button
              disabled
              title="未实现"
              className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft-inset text-on-surface-variant/50 font-label-caps text-[11px] cursor-not-allowed"
            >
              <Icon name="folder" className="w-[13px] h-[13px]" />
              <span>挂载库</span>
            </button>
            <button
              disabled
              title="未实现"
              className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft-inset text-on-surface-variant/50 font-label-caps text-[11px] cursor-not-allowed"
            >
              <Icon name="mic" className="w-[13px] h-[13px]" />
              <span>语音</span>
            </button>
          </div>
          <div className="flex items-center space-x-2">
            {busy ? (
              <button
                onClick={cancel}
                className="px-3 py-1.5 rounded-full bg-surface-container text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container-high transition-colors"
              >
                取消生成
              </button>
            ) : null}
            <span className="font-label-caps text-[10px] text-outline hidden sm:inline">
              {canSubmit ? "↵ 发送" : "提交已关闭"}
            </span>
            <button
              onClick={send}
              disabled={!canSubmit || !input.trim()}
              className="w-8 h-8 rounded-full bg-primary text-on-primary flex items-center justify-center hover:bg-primary-container active:scale-95 shadow-sm transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              title={canSubmit ? "发送指令" : "当前权限下不发送输入"}
            >
              <Icon name="arrow-up" className="w-4 h-4" />
            </button>
          </div>
        </div>
        {cancelNote !== null ? (
          <div className="mt-2 font-label-caps text-[10px] text-outline">取消：{cancelNote || "已请求"}</div>
        ) : null}
      </div>

      {/* 底部状态行：模型权限 + Token 上下文用量，一行显示。
          顶部「LOCAL IPC ACTIVE」表示引擎 IPC 健康；这里表示权限与用量，含义不同。
          Token 只显示引擎通过 progress/usage 上报的真实 used/size，不自行估算。 */}
      <div className="mt-1.5 flex flex-wrap items-center gap-x-5 gap-y-1 px-1 flex-shrink-0">
        <span className="inline-flex items-center gap-1.5 min-w-0">
          <Icon name="shield-check" className="w-[13px] h-[13px] text-primary/80 flex-shrink-0" />
          <span className="font-label-caps text-[10px] text-outline truncate" title={permissionDetail}>
            {admission === "checking"
              ? "模型权限：正在读取宿主事实…"
              : admission === "ready"
                ? `模型权限：已开启 · ${categoryText}`
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
        {workspaceNote ? (
          <span className="font-label-caps text-[10px] text-outline truncate" title={workspaceNote}>
            工作目录：{workspaceNote}
          </span>
        ) : null}
      </div>
    </div>
  );
}
