// OmniDesk 任务工作台（唯一已实现的纵向试点：脱敏数据分析与生态研判）。
//
// 五段固定骨架：输入 → 确定性统计 → 解释 → 人工确认 → 保存到仓库外本地目录。
//
// 硬规矩（本文件只是 UI，真正强制在 desktop/src-tauri/src/task_shell.rs）：
//   1. 数值只来自 task_run_analysis，UI 不做任何统计或补齐；
//   2. 保存前必须人工勾选确认，并如实选择解释来源；
//   3. 使用记录只记枚举与布尔，不记输入输出正文。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
// 页面局部样式：仅作用于本组件的 .wb-v2 子树，不影响首页与其它页面。
import "./workbench-ui.css";
import { host, onTaskEvent, shell } from "./api";
import {
  currentEngineSubmitDecision,
} from "./engineAdmission";
import {
  TERMINAL_STATES,
  type AnalysisResult,
  type CancelOutcome,
  type ExplanationSource,
  type LocalRootInfo,
  type SampleInfo,
  type SaveArtifactReceipt,
  type TaskEvent,
  type TaskState,
  type TaskStatus,
  type TaskTypeCatalog,
  type UsageRecord,
} from "./types";

/** 与 Rust 侧 fmt_num 保持一致：整数不带小数点，其余最多 6 位小数并去尾零。 */
function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return "NaN";
  if (Number.isInteger(value)) return String(value);
  return value
    .toFixed(6)
    .replace(/0+$/, "")
    .replace(/\.$/, "");
}

const SOURCE_LABEL: Record<ExplanationSource, string> = {
  user: "手写（user）",
  model_unverified: "模型输出 · 未复核（model_unverified）",
};

// 下列两张表与「引擎技术切片」视图保持同一套中文措辞。
// 刻意在本文件内重复，避免工作台依赖技术切片视图（后者可随时移除）。
const STATE_LABEL: Record<TaskState, string> = {
  queued: "排队中",
  starting: "启动中",
  running: "执行中",
  cancelling: "取消中",
  succeeded: "已完成",
  failed: "已失败",
  cancelled: "已取消",
  interrupted: "已中断",
  unknown: "状态未知",
};

/** 取消阶段的文案：只有 confirmed_cancelled 才算取消成功，请求发出不等于取消成功。 */
const CANCEL_PHASE_LABEL: Record<string, string> = {
  not_requested: "未请求取消",
  request_sent: "取消请求已发送（尚未确认）",
  acknowledged: "引擎已收到取消请求（非最终确认）",
  confirmed_cancelled: "取消已确认",
  timeout_unknown: "取消结果未知（超时）",
};

/** 等待引擎终态的轮询上限：超时只报「未确认终态」，不写成成功。 */
const ENGINE_WAIT_MS = 180_000;

async function pollUntilTerminal(omniTaskId: string): Promise<TaskStatus> {
  const deadline = Date.now() + ENGINE_WAIT_MS;
  let last = await host.taskStatus(omniTaskId);
  let consecutiveFailures = 0;
  while (!TERMINAL_STATES.includes(last.status)) {
    if (Date.now() >= deadline) {
      throw new Error(
        "等待引擎终态超时（任务 " +
          omniTaskId +
          " 最近状态：" +
          last.status +
          "）。任务可能仍在执行，输出未回填；可点击「取消引擎任务」或稍后重试。",
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 1200));
    try {
      last = await host.taskStatus(omniTaskId);
      consecutiveFailures = 0;
    } catch (e) {
      consecutiveFailures += 1;
      if (consecutiveFailures >= 3) {
        throw new Error("连续 3 次查询引擎任务状态失败：" + String(e));
      }
    }
  }
  return last;
}

// #region engine-cancel-model
//
// 取消入口的纯状态模型。刻意不依赖 React、Tauri 或其它模块，便于用确定性
// 前端测试直接执行本段真实源码（tests/verification/desktop/verify_engine_cancel.py）。
// 这里只决定“按钮能不能点、显示什么”，不判定任务是否真的取消成功。

export interface EngineCancelButtonState {
  enabled: boolean;
  label: string;
  hint: string | null;
}

/** 按宿主任务状态与取消请求的在途状态，给出取消按钮的唯一判定。 */
export function engineCancelButtonState(input: {
  hasTask: boolean;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  /** 本次任务是否已经发出过取消请求（无论宿主是否已回读结果）。 */
  requestSent: boolean;
}): EngineCancelButtonState {
  if (!input.hasTask) {
    return { enabled: false, label: "取消引擎任务", hint: null };
  }
  if (input.requestInFlight) {
    return {
      enabled: false,
      label: "正在请求取消…",
      hint: "取消请求已发出，正在等待宿主回读状态；请求发出不等于取消成功。",
    };
  }
  if (!input.status) {
    return {
      enabled: false,
      label: "等待任务状态…",
      hint: "尚未取得宿主状态，暂不能取消。",
    };
  }
  if (
    input.status === "succeeded" ||
    input.status === "failed" ||
    input.status === "cancelled" ||
    input.status === "interrupted" ||
    input.status === "unknown"
  ) {
    return {
      enabled: false,
      label: "任务已结束",
      hint: "任务已进入终态，不能再取消。",
    };
  }
  // 已在取消中：只有拿到明确的 timeout_unknown 才允许重试一次。
  // 请求已发出但宿主尚未给出终态时重复发送，只会制造并发取消请求。
  if (input.status === "cancelling") {
    if (input.lastPhase === "timeout_unknown") {
      return {
        enabled: true,
        label: "重试取消",
        hint: "上次取消在超时内未获终态确认，允许重试一次；仍以宿主终态为准。",
      };
    }
    return {
      enabled: false,
      label: "等待取消结果…",
      hint: "取消请求已发出，正在等待宿主终态；请求发出不等于取消成功。",
    };
  }
  // 宿主状态可能滞后于请求：本任务已经发过取消，就不再允许第二次。
  if (input.requestSent) {
    return {
      enabled: false,
      label: "等待取消结果…",
      hint: "取消请求已发出，尚未取得终态证据；不会重复发送。",
    };
  }
  // queued / starting / running：首次请求前允许取消。
  return {
    enabled: true,
    label: "取消引擎任务",
    hint: "请求取消不等于取消成功；结果以宿主回读的终态为准。",
  };
}

/**
 * 解释一次取消结果。只有 confirmed_cancelled 或宿主最终态为 cancelled
 * 才可显示“取消已确认”；请求已发送、其它终态、超时都按未确认处理。
 */
export function describeEngineCancelOutcome(outcome: {
  phase: string;
  final_state: string | null;
  note: string;
}): { confirmed: boolean; message: string } {
  if (outcome.phase === "confirmed_cancelled" || outcome.final_state === "cancelled") {
    return {
      confirmed: true,
      message: "取消已确认：宿主报告任务已取消。",
    };
  }
  const stateText = outcome.final_state ? "，宿主最终态为 " + outcome.final_state : "";
  return {
    confirmed: false,
    message:
      "取消未确认（阶段：" +
      outcome.phase +
      stateText +
      "）。" +
      (outcome.note ? outcome.note + "。" : ""),
  };
}

/**
 * 取消入口的唯一放行判定。组件在调用宿主前执行本函数，测试则用宿主替身
 * 直接驱动它并统计真实 IPC 次数。按钮可见性不能代替本判定：即使状态模型
 * 认为可点，这里也必须独立拒绝重复请求。
 */
export function engineCancelGate(input: {
  hasTask: boolean;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  requestSent: boolean;
}): { send: boolean; reason: string } {
  if (!input.hasTask) return { send: false, reason: "no_task" };
  if (input.requestInFlight) return { send: false, reason: "request_in_flight" };
  if (!input.status) return { send: false, reason: "no_status" };
  if (
    input.status === "succeeded" ||
    input.status === "failed" ||
    input.status === "cancelled" ||
    input.status === "interrupted" ||
    input.status === "unknown"
  ) {
    return { send: false, reason: "terminal" };
  }
  if (input.status === "cancelling") {
    return input.lastPhase === "timeout_unknown"
      ? { send: true, reason: "retry_after_timeout" }
      : { send: false, reason: "awaiting_cancel_result" };
  }
  if (input.requestSent) return { send: false, reason: "already_requested" };
  return { send: true, reason: "first_request" };
}

/** 取消请求的最小宿主接口；测试用计数替身实现同一契约。 */
export interface EngineCancelHostLike {
  cancelTask(taskId: string): Promise<any>;
  taskStatus(taskId: string): Promise<any>;
}

export interface EngineCancelSnapshot {
  taskId: string | null;
  status: string | null;
  requestInFlight: boolean;
  lastPhase: string | null;
  requestSent: boolean;
}

export interface EngineCancelEffects {
  host: EngineCancelHostLike;
  onRequestStarted: () => void;
  onOutcome: (outcome: any) => void;
  onStatus: (status: any) => void;
  onError: (message: string) => void;
  onSettled: () => void;
}

/**
 * 组件与测试共用的取消发送路径。返回是否真的发出了 IPC：
 * `skipped` 表示被放行判定拦下，调用方不得据此外推任务状态。
 */
export async function runEngineCancelRequest(
  snapshot: EngineCancelSnapshot,
  effects: EngineCancelEffects,
): Promise<"sent" | "skipped"> {
  const taskId = snapshot.taskId;
  // taskId 是否存在决定 hasTask；调用方不再重复传入，避免两处不一致。
  const gateInput = { ...snapshot, hasTask: !!taskId };
  const gate = engineCancelGate(gateInput);
  if (!gate.send || !taskId) return "skipped";
  effects.onRequestStarted();
  try {
    try {
      const outcome = await effects.host.cancelTask(taskId);
      effects.onOutcome(outcome);
    } catch (e) {
      effects.onError(
        "取消请求未获确认：" +
          String(e) +
          "。任务可能仍在执行，解释框保持不变。可稍后重试取消或等待任务终态。",
      );
    }
    // 状态回读是独立一步：回读失败不得把已确认的取消结果改写成“未确认”。
    try {
      effects.onStatus(await effects.host.taskStatus(taskId));
    } catch {
      /* 状态也读不到时保留原状态，不猜测。 */
    }
  } finally {
    effects.onSettled();
  }
  return "sent";
}

// #endregion engine-cancel-model

const REWORK_LABEL: Record<string, string> = {
  none: "无需返工",
  structure: "结构问题",
  facts: "事实/数据问题",
  wording: "表述问题",
  format: "格式问题",
  requirements_misread: "要求理解偏差",
};

const BLOCKER_LABEL: Record<string, string> = {
  none: "无",
  engine_unavailable: "引擎不可用",
  credentials_missing: "模型凭证缺失",
  input_unclear: "输入不清晰",
  template_missing: "缺少适配模板",
  other_technical: "其他技术问题",
};

const DURATION_LABEL: Record<string, string> = {
  under_5m: "5 分钟以内",
  "5_15m": "5–15 分钟",
  "15_30m": "15–30 分钟",
  "30_60m": "30–60 分钟",
  over_60m: "60 分钟以上",
};

const REWORK_OPTIONS = Object.keys(REWORK_LABEL);
const BLOCKER_OPTIONS = Object.keys(BLOCKER_LABEL);
const DURATION_OPTIONS = Object.keys(DURATION_LABEL);

function formatTime(seconds: number): string {
  if (!seconds) return "";
  return new Date(seconds * 1000).toLocaleString("zh-CN", { hour12: false });
}

export default function Workbench() {
  const [catalog, setCatalog] = useState<TaskTypeCatalog | null>(null);
  const [samples, setSamples] = useState<SampleInfo[]>([]);
  const [localRoot, setLocalRoot] = useState<LocalRootInfo | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const [taskType, setTaskType] = useState("desensitized_analysis");
  const [inputText, setInputText] = useState("");
  const [activeSample, setActiveSample] = useState<string | null>(null);

  const [analysis, setAnalysis] = useState<AnalysisResult | null>(null);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [redLineHits, setRedLineHits] = useState<
    Array<{ kind: string; count: number; label: string }>
  >([]);

  const [promptText, setPromptText] = useState<string | null>(null);
  const [promptError, setPromptError] = useState<string | null>(null);

  // 引擎解释子流程（阶段 3 的一条支路）。
  //   * 只把「阶段 3 冻结提示词」交给引擎，交互仍然只有 IPC-1；
  //   * 终态一律以宿主返回的状态快照为准，事件只用于显示与流式文本；
  //   * 这里任何字段都不参与统计，也不改变统计指纹。
  const [engineTaskId, setEngineTaskId] = useState<string | null>(null);
  const [engineStatus, setEngineStatus] = useState<TaskStatus | null>(null);
  const [engineEvents, setEngineEvents] = useState<TaskEvent[]>([]);
  const [engineCancel, setEngineCancel] = useState<CancelOutcome | null>(null);
  // 本次任务是否已经成功发出过取消请求。宿主状态可能滞后，因此不能只看
  // status/cancel_phase 判断“是否请求过”；这个标志只在明确超时后允许一次重试。
  const [engineCancelRequested, setEngineCancelRequested] = useState(false);
  const [engineBusy, setEngineBusy] = useState(false);
  // 取消请求本身是否在途。与 engineBusy 分开：任务正在生成时仍应允许发出取消。
  const [engineCancelBusy, setEngineCancelBusy] = useState(false);
  // 同步防重入：React 状态更新是异步的，快速连点可能在同一帧内通过
  // engineCancelBusy 检查，因此再留一把同步锁。
  const cancelInFlightRef = useRef(false);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [engineNote, setEngineNote] = useState<string | null>(null);
  const [engineWorkspace, setEngineWorkspace] = useState<string | null>(null);

  const [explanation, setExplanation] = useState("");
  const [source, setSource] = useState<ExplanationSource>("user");

  const [confirmed, setConfirmed] = useState(false);
  const [confirmNote, setConfirmNote] = useState("");

  const [receipt, setReceipt] = useState<SaveArtifactReceipt | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [usage, setUsage] = useState<UsageRecord[]>([]);
  const [usagePath, setUsagePath] = useState<string | null>(null);
  const [usageUnparsable, setUsageUnparsable] = useState(0);
  const [usageError, setUsageError] = useState<string | null>(null);
  const [usageBusy, setUsageBusy] = useState(false);
  const [recorded, setRecorded] = useState(false);

  const [recordCompleted, setRecordCompleted] = useState(true);
  const [recordUsable, setRecordUsable] = useState(true);
  const [recordRework, setRecordRework] = useState("none");
  const [recordBlocker, setRecordBlocker] = useState("none");
  const [recordDuration, setRecordDuration] = useState("");
  const [recordReuseWillingness, setRecordReuseWillingness] = useState(false);

  const refreshUsage = useCallback(async () => {
    try {
      const view = await shell.usageView();
      setUsage(view.records);
      setUsagePath(view.path);
      setUsageUnparsable(view.unparsable_lines);
      setUsageError(null);
    } catch (e) {
      setUsageError(String(e));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [types, sampleCatalog, root] = await Promise.all([
          shell.taskTypes(),
          shell.samples(),
          shell.localRootInfo(),
        ]);
        if (cancelled) return;
        setCatalog(types);
        setSamples(sampleCatalog.samples);
        setLocalRoot(root);
      } catch (e) {
        if (!cancelled) setBootError(String(e));
      }
    })();
    void refreshUsage();
    return () => {
      cancelled = true;
    };
  }, [refreshUsage]);

  /** 清空引擎解释子流程的全部状态（输入或统计变化时调用）。 */
  const resetEngineRun = useCallback(() => {
    setEngineTaskId(null);
    setEngineStatus(null);
    setEngineEvents([]);
    setEngineCancel(null);
    setEngineCancelRequested(false);
    setEngineCancelBusy(false);
    setEngineError(null);
    setEngineNote(null);
  }, []);

  // 订阅宿主转发的事件。只累积当前任务的事件，并按 seq 去重排序。
  // 这里不据事件判定成功，终态一律回读宿主状态快照。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let closed = false;
    onTaskEvent((event) => {
      setEngineEvents((prev) => {
        if (prev.some((e) => e.seq === event.seq)) return prev;
        return [...prev, event].sort((a, b) => a.seq - b.seq);
      });
    })
      .then((fn) => {
        if (closed) fn();
        else unlisten = fn;
      })
      .catch(() => {
        /* 订阅失败不阻塞界面；提交时会给出明确错误。 */
      });
    return () => {
      closed = true;
      if (unlisten) unlisten();
    };
  }, []);

  /** 流式文本仅供预览；回填解释框一律用引擎的最终结果，避免半截文本被当成成品。 */
  const streamedExplanation = useMemo(
    () =>
      engineEvents
        .filter((e) => e.type === "text_delta")
        .map((e) => String((e.payload as { text?: unknown }).text ?? ""))
        .join(""),
    [engineEvents],
  );

  /** 载入合成样例：同时清空下游产物，避免解释挂在旧统计上。 */
  const loadSample = useCallback((sample: SampleInfo) => {
    setInputText(sample.content);
    setActiveSample(sample.sample_id);
    setAnalysis(null);
    setAnalysisError(null);
    setRedLineHits([]);
    setPromptText(null);
    setPromptError(null);
    setExplanation("");
    setConfirmed(false);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
    resetEngineRun();
  }, [resetEngineRun]);

  const handleInputChange = useCallback((value: string) => {
    setInputText(value);
    setActiveSample(null);
    // 输入变了，之前算出的统计与提示词立即失效，防止落到过期指纹上。
    setAnalysis(null);
    setAnalysisError(null);
    setRedLineHits([]);
    setPromptText(null);
    setPromptError(null);
    setConfirmed(false);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
    // 引擎解释同样属于下游产物：输入一变就作废，避免把旧输出贴到新统计上。
    resetEngineRun();
  }, [resetEngineRun]);

  const runAnalysis = useCallback(async () => {
    setAnalysisError(null);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
    // 重新统计即作废旧的引擎解释（指纹可能已变）。
    resetEngineRun();
    try {
      const response = await shell.runAnalysis(inputText);
      setRedLineHits(response.red_line_hits);
      if (response.ok && response.result) {
        setAnalysis(response.result);
        setConfirmed(false);
      } else {
        setAnalysis(null);
        setAnalysisError(response.error ?? "统计失败（未返回原因）");
      }
    } catch (e) {
      setAnalysis(null);
      setAnalysisError(String(e));
    }
  }, [inputText]);

  const buildPrompt = useCallback(async () => {
    if (!analysis) return;
    setPromptError(null);
    try {
      const response = await shell.explanationPrompt({
        taskType,
        inputText,
        fingerprint: analysis.fingerprint,
      });
      setPromptText(response.prompt);
    } catch (e) {
      setPromptText(null);
      setPromptError(String(e));
    }
  }, [analysis, taskType, inputText]);

  /**
   * 阶段 3 的引擎支路：把已经冻结的解释提示词交给引擎，终态后回填解释框。
   *
   * 硬规矩：
   *   * 提示词只能来自 shell.explanationPrompt（本地重算 + 指纹比对），UI 不自行拼提示词；
   *   * 终态只认宿主回读的状态快照，事件仅用于显示与流式预览；
   *   * 失败/取消/超时都不写成功，也不动用户已写的解释；
   *   * 成功回填时来源固定为 model_unverified，界面不提供任何「已复核」来源。
   */
  const runEngineExplanation = useCallback(async () => {
    if (!analysis) return;
    setEngineBusy(true);
    setEngineError(null);
    setEngineNote(null);
    setEngineCancel(null);
    setEngineCancelRequested(false);
    setEngineEvents([]);
    setEngineTaskId(null);
    setEngineStatus(null);
    try {
      // 0) 前端只拦空文本；端点类别与模型权限由宿主在提交时按实际配置判定。
      const admission = currentEngineSubmitDecision(inputText);
      if (!admission.allowed) {
        setEngineError(admission.reason);
        return;
      }
      // 1) 冻结提示词（指纹不符会在这里就失败，不会把别的统计交给模型）。
      const frozen = await shell.explanationPrompt({
        taskType,
        inputText,
        fingerprint: analysis.fingerprint,
      });
      setPromptText(frozen.prompt);

      // 2) 准备隔离工作目录（引擎要求目录已存在且可解析）。
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error ?? "无法准备引擎工作目录");
      }
      setEngineWorkspace(workspace.workspace_dir);

      // 3) 提交任务。intent 用任务类型标识（只被记录，不改变引擎行为）。
      const receipt = await host.submitTask({
        promptText: frozen.prompt,
        intent: taskType,
        workspaceDir: workspace.workspace_dir,
        // 红线扫描只针对用户原始输入；提示词里的规则正文本身含边界说明用词。
        userInputText: inputText,
      });
      const taskId = receipt.omni_task_id;
      setEngineTaskId(taskId);
      setEngineStatus({
        omni_task_id: taskId,
        status: receipt.status,
        state_since: receipt.accepted_at,
        cancel_phase: "not_requested",
        note: receipt.note,
      });

      // 4) 等待终态（超时只报未确认终态，不写成成功）。
      const terminal = await pollUntilTerminal(taskId);
      setEngineStatus(terminal);

      // 5) 取最终结果。半截流式文本不作数。
      const result = await host.taskResult(taskId);
      if (terminal.status === "succeeded") {
        if (result.content_omitted) {
          setEngineNote(
            "引擎已完成，但输出过长（" +
              String(result.content_length ?? "未知") +
              " 字符），已由引擎自行保存在 OmniDesk 应用数据目录，未回填到解释框。" +
              "请缩短输入后重试，或自行撰写解释。",
          );
        } else if (result.content.trim().length === 0) {
          setEngineError(
            "引擎报告已完成，但没有返回任何文本；未回填解释框。可重试或改用手写。",
          );
        } else {
          setExplanation(result.content);
          setSource("model_unverified");
          setConfirmed(false);
          setEngineNote(
            "已回填引擎输出。来源固定记为 model_unverified（未复核）：" +
              "本阶段无法在代码层证明该输出与引擎成功终态一一对应，请人工复核静态数字后再确认。",
          );
        }
      } else {
        // 失败 / 取消 / 中断 / 状态未知：明确报原因，不写成功，也不改写解释框。
        setEngineError(
          "引擎任务未成功（状态：" +
            terminal.status +
            "）。" +
            (terminal.note ? "状态说明：" + terminal.note + "。" : "") +
            (result.note ? "结果说明：" + result.note + "。" : "") +
            "解释框保持不变，可重试或改为手写。",
        );
      }
    } catch (e) {
      setEngineError(String(e));
    } finally {
      setEngineBusy(false);
    }
  }, [analysis, taskType, inputText]);

  /** 请求取消当前引擎任务；请求发出不等于取消成功，随后回读真实状态。 */
  const cancelEngineExplanation = useCallback(async () => {
    // 放行判定与测试共用同一实现：重复点击、取消中、终态和超时重试都在
    // runEngineCancelRequest 内独立拦截，不依赖按钮是否被禁用。
    await runEngineCancelRequest(
      {
        taskId: engineTaskId ?? null,
        status: engineStatus?.status ?? null,
        requestInFlight: cancelInFlightRef.current || engineCancelBusy,
        lastPhase: engineCancel?.phase ?? null,
        requestSent: engineCancelRequested,
      },
      {
        host,
        onRequestStarted: () => {
          // 同步锁必须早于首个 await，否则快速双击会发出两次取消。
          cancelInFlightRef.current = true;
          setEngineCancelBusy(true);
          setEngineError(null);
          // 重试前清掉上一次的取消结果，避免在途重试仍被当作“可再次重试”。
          setEngineCancel(null);
          setEngineCancelRequested(true);
        },
        onOutcome: (outcome) => setEngineCancel(outcome as CancelOutcome),
        onStatus: (status) => setEngineStatus(status as TaskStatus),
        onError: (message) => setEngineError(message),
        onSettled: () => {
          cancelInFlightRef.current = false;
          setEngineCancelBusy(false);
        },
      },
    );
  }, [engineTaskId, engineStatus, engineCancel, engineCancelRequested]);

  const engineCancelView = engineCancelButtonState({
    hasTask: !!engineTaskId,
    status: engineStatus?.status ?? null,
    requestInFlight: engineCancelBusy,
    lastPhase: engineCancel?.phase ?? null,
    requestSent: engineCancelRequested,
  });

  const handleSave = useCallback(async () => {
    if (!analysis) return;
    setSaveError(null);
    setReceipt(null);
    try {
      const saved = await shell.saveArtifact({
        taskType,
        inputText,
        explanationText: explanation,
        explanationSource: source,
        // 人工确认由界面的勾选显式给出；Rust 保存入口会再次强制校验该布尔值。
        // UI 无法自行证明任务在引擎里成功终态，因此不提供任何"已复核模型来源"选项。
        userConfirmed: confirmed,
        confirmationNote: confirmNote,
        fingerprint: analysis.fingerprint,
      });
      setReceipt(saved);
    } catch (e) {
      setSaveError(String(e));
    }
  }, [analysis, taskType, inputText, explanation, source, confirmed, confirmNote]);

  const recordUsage = useCallback(async () => {
    setUsageBusy(true);
    setUsageError(null);
    try {
      await shell.recordUsage({
        taskType,
        completed: recordCompleted,
        usable: recordUsable,
        reworkKind: recordRework,
        blocker: recordBlocker,
        durationBucket: recordDuration || null,
        reuseWillingness: recordReuseWillingness,
      });
      setRecorded(true);
      await refreshUsage();
    } catch (e) {
      setUsageError(String(e));
    } finally {
      setUsageBusy(false);
    }
  }, [
    taskType,
    recordCompleted,
    recordUsable,
    recordRework,
    recordBlocker,
    recordDuration,
    recordReuseWillingness,
    refreshUsage,
  ]);

  const deleteRecord = useCallback(
    async (recordId: string) => {
      setUsageBusy(true);
      try {
        await shell.usageDelete(recordId);
        await refreshUsage();
      } catch (e) {
        setUsageError(String(e));
      } finally {
        setUsageBusy(false);
      }
    },
    [refreshUsage],
  );

  const clearRecords = useCallback(async () => {
    setUsageBusy(true);
    try {
      await shell.usageClear();
      await refreshUsage();
    } catch (e) {
      setUsageError(String(e));
    } finally {
      setUsageBusy(false);
    }
  }, [refreshUsage]);

  const implementedTypes = useMemo(
    () => (catalog?.types ?? []).filter((t) => t.status === "implemented"),
    [catalog],
  );
  const reservedTypes = useMemo(
    () => (catalog?.types ?? []).filter((t) => t.status !== "implemented"),
    [catalog],
  );

  const canSave = !!analysis && confirmed && explanation.trim().length > 0;

  return (
    <div className="workbench wb-v2">
      {bootError ? <div className="alert">加载任务外壳失败：{bootError}</div> : null}

      <section className="pane wb-header">
        <h2 className="pane-title">
          任务类型
          {catalog ? (
            <span className="count mono">{catalog.shell_version}</span>
          ) : null}
        </h2>
        <div className="type-row">
          <select
            className="select"
            value={taskType}
            onChange={(e) => setTaskType(e.target.value)}
          >
            {implementedTypes.map((t) => (
              <option key={t.task_type} value={t.task_type}>
                {t.title}
              </option>
            ))}
          </select>
          <span className="pill succeeded">已实现</span>
          {reservedTypes.length > 0 ? (
            <span className="hint">
              未实现（仅保留边界，不代表获得授权）：
              {reservedTypes.map((t) => t.title).join("、")}
            </span>
          ) : null}
        </div>
        <p className="hint">
          已实现类型的数据范围：仅合成 / 公开 / 经明确批准的数据。禁止案件材料、线索或未公开调查信息。
        </p>
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 1 · 输入
          <span className="count">合成样例或可处理数据</span>
        </h2>
        <div className="actions">
          {samples.map((sample) => (
            <button
              key={sample.sample_id}
              className={activeSample === sample.sample_id ? "primary" : "ghost"}
              onClick={() => loadSample(sample)}
              title={sample.note}
            >
              {sample.title}
            </button>
          ))}
        </div>
        <textarea
          className="prompt"
          value={inputText}
          onChange={(e) => handleInputChange(e.target.value)}
          placeholder={"类别,数值\n示例甲,10\n示例乙,20"}
          rows={8}
        />
        <div className="actions">
          <button className="primary" onClick={runAnalysis} disabled={!inputText.trim()}>
            运行确定性统计
          </button>
        </div>
        {redLineHits.length > 0 ? (
          <div className="alert">
            红线扫描命中（启发式，仅提示，不代表合规判断；不回显命中原文）：
            <ul>
              {redLineHits.map((hit) => (
                <li key={hit.kind}>
                  {hit.label} — {hit.count} 处
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 2 · 确定性统计
          <span className="count">本地代码计算，不调用模型</span>
        </h2>
        {analysisError ? <div className="alert">{analysisError}</div> : null}
        {analysis ? (
          <>
            <div className="status-card">
              <div className="status-row">
                <span className="k">行数</span>
                <span className="v mono">{analysis.rows}</span>
              </div>
              <div className="status-row">
                <span className="k">类别数</span>
                <span className="v mono">{analysis.distinct_categories}</span>
              </div>
              <div className="status-row">
                <span className="k">合计 / 平均</span>
                <span className="v mono">
                  {formatNumber(analysis.total)} / {formatNumber(analysis.mean)}
                </span>
              </div>
              <div className="status-row">
                <span className="k">最小 / 最大</span>
                <span className="v mono">
                  {formatNumber(analysis.min)} / {formatNumber(analysis.max)}
                </span>
              </div>
              <div className="status-row">
                <span className="k">最高类别</span>
                <span className="v mono">{analysis.top_category ?? "（无）"}</span>
              </div>
              <div className="status-row">
                <span className="k">指纹</span>
                <span className="v mono">{analysis.fingerprint}</span>
              </div>
            </div>
            <table className="stat-table">
              <thead>
                <tr>
                  <th>类别</th>
                  <th>条数</th>
                  <th>合计</th>
                  <th>平均</th>
                  <th>最小</th>
                  <th>最大</th>
                </tr>
              </thead>
              <tbody>
                {analysis.by_category.map((stat) => (
                  <tr key={stat.category}>
                    <td>{stat.category}</td>
                    <td className="mono">{stat.count}</td>
                    <td className="mono">{formatNumber(stat.sum)}</td>
                    <td className="mono">{formatNumber(stat.mean)}</td>
                    <td className="mono">{formatNumber(stat.min)}</td>
                    <td className="mono">{formatNumber(stat.max)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        ) : (
          <div className="empty">
            尚未计算。统计值只能由本地确定性代码产生，模型不参与计算。
          </div>
        )}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 3 · 解释
          <span className="count">只解释，不重算</span>
        </h2>
        <div className="actions">
          <button className="ghost" onClick={buildPrompt} disabled={!analysis}>
            生成解释提示词
          </button>
          {promptText ? (
            <button
              className="ghost"
              onClick={() => {
                void navigator.clipboard.writeText(promptText);
              }}
            >
              复制提示词
            </button>
          ) : null}
          <button
            className="primary"
            onClick={runEngineExplanation}
            disabled={!analysis || engineBusy}
          >
            {engineBusy ? "引擎生成中…" : "用引擎生成解释"}
          </button>
        </div>
        <p className="hint">
          提交时由宿主按当前模型端点与你在设置中开启的权限判定。未开启或被拒绝时，
          引擎不会启动，输入不会发送；可继续使用「生成解释提示词」手工处理。
        </p>
        {engineError ? <div className="alert">{engineError}</div> : null}
        {engineNote ? <p className="hint">{engineNote}</p> : null}
        {engineTaskId ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">引擎任务</span>
              <span className="v mono">{engineTaskId}</span>
            </div>
            <div className="status-row">
              <span className="k">状态</span>
              <span className="v">
                {engineStatus ? (
                  <>
                    <span className={"pill " + engineStatus.status}>
                      {STATE_LABEL[engineStatus.status]}
                    </span>
                    {engineStatus.note ? (
                      <span className="hint">{engineStatus.note}</span>
                    ) : null}
                  </>
                ) : (
                  "—"
                )}
              </span>
            </div>
            <div className="status-row">
              <span className="k">取消</span>
              <span className="v">
                {engineCancel
                  ? CANCEL_PHASE_LABEL[engineCancel.phase] ?? engineCancel.phase
                  : (CANCEL_PHASE_LABEL[
                      engineStatus?.cancel_phase ?? "not_requested"
                    ] ?? engineStatus?.cancel_phase ?? "not_requested")}
                {engineCancel?.note ? (
                  <span className="hint">{engineCancel.note}</span>
                ) : null}
              </span>
            </div>
            {engineWorkspace ? (
              <div className="status-row">
                <span className="k">工作目录</span>
                <span className="v mono">{engineWorkspace}</span>
              </div>
            ) : null}
            <div className="actions">
              <button
                className="ghost"
                onClick={cancelEngineExplanation}
                disabled={!engineCancelView.enabled}
              >
                {engineCancelView.label}
              </button>
              {engineCancelView.hint ? (
                <span className="hint">{engineCancelView.hint}</span>
              ) : null}
            </div>
            {engineCancel ? (
              <p className="hint">
                {describeEngineCancelOutcome(engineCancel).message}
              </p>
            ) : null}
            {streamedExplanation ? (
              <details className="tech">
                <summary>
                  引擎流式文本预览（{engineEvents.length} 条事件；仅供预览，回填以最终结果为准）
                </summary>
                <pre className="text">{streamedExplanation}</pre>
              </details>
            ) : null}
          </div>
        ) : null}
        {promptError ? <div className="alert">{promptError}</div> : null}
        {promptText ? (
          <details className="tech">
            <summary>解释提示词（含冻结统计块，可直接交给模型）</summary>
            <pre className="text">{promptText}</pre>
          </details>
        ) : (
          <p className="hint">
            提示词里的统计块是冻结文本：模型只能解释，不得重算或补充数字。
            当前会话未配置模型凭证时，可自行把提示词交给已获准的模型，再把结果贴回下方。
          </p>
        )}
        <label className="field">
          <span className="field-label">解释来源</span>
          <select
            className="select"
            value={source}
            onChange={(e) => setSource(e.target.value as ExplanationSource)}
          >
            <option value="user">{SOURCE_LABEL.user}</option>
            <option value="model_unverified">{SOURCE_LABEL.model_unverified}</option>
          </select>
        </label>
        <textarea
          className="prompt"
          value={explanation}
          onChange={(e) => setExplanation(e.target.value)}
          placeholder="在此粘贴或撰写解释。不得引入统计块之外的数字。"
          rows={7}
        />
        <p className="hint">
          本阶段只提供「手写」与「模型输出 · 未复核」两种来源。可核验的模型成功来源尚未实现，
          因此不会出现任何"已复核模型"选项，避免把无法证明的状态写成事实。
        </p>
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 4 · 人工确认
          <span className="count">未确认不得保存</span>
        </h2>
        <label className="check">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          <span>
            我已人工复核解释与确定性统计一致，并确认本任务只使用合成 / 公开 / 经明确批准的数据。
          </span>
        </label>
        <label className="field">
          <span className="field-label">确认备注（可选）</span>
          <input
            className="text-input"
            value={confirmNote}
            onChange={(e) => setConfirmNote(e.target.value)}
            placeholder="例如：已核对合计与类别排序"
          />
        </label>

        <h2 className="pane-title">
          阶段 5 · 保存
          <span className="count">仓库外本地目录</span>
        </h2>
        <p className="hint mono">
          产物目录：{localRoot?.artifact_root ?? (localRoot?.error ?? "—")}
        </p>
        <div className="actions">
          <button className="primary" onClick={handleSave} disabled={!canSave}>
            保存产物
          </button>
        </div>
        {!canSave ? (
          <p className="hint">
            需先完成确定性统计、填写解释并勾选人工确认。
          </p>
        ) : null}
        {saveError ? <div className="alert">{saveError}</div> : null}
        {receipt ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">已保存</span>
              <span className="v mono">{receipt.path}</span>
            </div>
            <div className="status-row">
              <span className="k">字节</span>
              <span className="v mono">{receipt.bytes}</span>
            </div>
            <div className="status-row">
              <span className="k">写入时间</span>
              <span className="v mono">{formatTime(receipt.written_at)}</span>
            </div>
          </div>
        ) : null}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          使用记录
          <span className="count">不含输入输出正文</span>
        </h2>
        <p className="hint mono">记录文件：{usagePath ?? "—"}</p>
        <div className="record-form">
          <label className="check">
            <input
              type="checkbox"
              checked={recordCompleted}
              onChange={(e) => setRecordCompleted(e.target.checked)}
            />
            <span>任务完成</span>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={recordUsable}
              onChange={(e) => setRecordUsable(e.target.checked)}
            />
            <span>结果可用</span>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={recordReuseWillingness}
              onChange={(e) => setRecordReuseWillingness(e.target.checked)}
            />
            <span>类似任务还愿意再用</span>
          </label>
          <label className="field">
            <span className="field-label">返工类别</span>
            <select
              className="select"
              value={recordRework}
              onChange={(e) => setRecordRework(e.target.value)}
            >
              {REWORK_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {REWORK_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">障碍</span>
            <select
              className="select"
              value={recordBlocker}
              onChange={(e) => setRecordBlocker(e.target.value)}
            >
              {BLOCKER_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {BLOCKER_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">耗时（可选）</span>
            <select
              className="select"
              value={recordDuration}
              onChange={(e) => setRecordDuration(e.target.value)}
            >
              <option value="">不填写</option>
              {DURATION_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {DURATION_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <button className="ghost" onClick={recordUsage} disabled={usageBusy}>
            记录一次使用
          </button>
          {recorded ? <span className="hint">已记录。</span> : null}
        </div>

        {usageError ? <div className="alert">{usageError}</div> : null}
        {usageUnparsable > 0 ? (
          <p className="hint">有 {usageUnparsable} 行无法解析，已跳过。</p>
        ) : null}
        <div className="actions">
          <button className="ghost" onClick={refreshUsage} disabled={usageBusy}>
            刷新
          </button>
          <button
            className="ghost"
            onClick={clearRecords}
            disabled={usageBusy || usage.length === 0}
          >
            清空全部记录
          </button>
        </div>
        {usage.length === 0 ? (
          <div className="empty">暂无使用记录。</div>
        ) : (
          <table className="stat-table">
            <thead>
              <tr>
                <th>时间</th>
                <th>完成</th>
                <th>可用</th>
                <th>返工</th>
                <th>障碍</th>
                <th>耗时</th>
                <th>愿意再用</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {usage.map((record) => (
                <tr key={record.record_id}>
                  <td className="mono">{formatTime(record.recorded_at)}</td>
                  <td>{record.completed ? "是" : "否"}</td>
                  <td>{record.usable ? "是" : "否"}</td>
                  <td>{REWORK_LABEL[record.rework_kind] ?? record.rework_kind}</td>
                  <td>{BLOCKER_LABEL[record.blocker] ?? record.blocker}</td>
                  <td>
                    {record.duration_bucket
                      ? DURATION_LABEL[record.duration_bucket] ??
                        record.duration_bucket
                      : "—"}
                  </td>
                  <td>{record.reuse_willingness ? "是" : "否"}</td>
                  <td>
                    <button
                      className="ghost tiny"
                      onClick={() => deleteRecord(record.record_id)}
                      disabled={usageBusy}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
