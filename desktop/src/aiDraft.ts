// AI 初稿子流程的纯状态模型与提示词组装。
//
// 刻意不依赖 React、Tauri 或任何传输层：组件与离线验证脚本执行同一份源码。
// 这里只决定「能不能提交/取消、显示什么状态、提示词由什么组成」，不伪造模型结果。
//
// 约束：模型调用一律经 api.ts 的 host.submitTask（含前端准入闸门与宿主准入）；
// 本文件不新建通信层，也不读取任何文件、凭证或模型配置。

export type AiDraftPhase =
  | "idle"
  | "running"
  | "refused"
  | "failed"
  | "cancelled"
  | "done";

export const AI_TERMINAL_STATES = [
  "succeeded",
  "failed",
  "cancelled",
  "interrupted",
  "unknown",
];

export interface AiMaterial {
  /** 材料来源文件名（用于提示词与来源记录）。 */
  name: string;
  /** 材料内容的 SHA-256（完整值；写入提示词时只取前 12 位）。 */
  sha256: string;
  /** 材料正文（原样进入提示词，不做解析或改写）。 */
  content: string;
}

export interface AiDraftInput {
  /** 界面上的场景名（来自可编辑流程包），只用于提示词开头与结果标注。 */
  sceneLabel: string;
  purpose: string;
  audience: string;
  requirements: string;
  material: string;
  /** 用户在界面**明确选择**的文件材料；内容原样进入提示词。 */
  materials?: AiMaterial[];
  /** 来自可编辑流程包的场景规则；缺省时使用内置默认规则。 */
  sceneRules?: string | null;
  /** 来自可编辑流程包的期望输出章节（每行一个）。 */
  sceneSections?: string | null;
  /**
   * Profile 规则资产里的写作铁律（OD-ARCH-068）。
   *
   * 平台本身不内置任何行业规则：没有加载 Profile 规则时不输出这一段；
   * 传空数组同样不输出。规则文本只来自用户可编辑的规则文件。
   */
  writingRules?: string[] | null;
}

/**
 * 流程包未提供规则时的内置默认规则。只描述“怎么整理”，不改变任何确定性计算。
 * 注意：这些是**可替换**的场景规则；下方 buildDraftPrompt 里的「写作规则」不可由流程包关闭。
 */
export const DEFAULT_SCENE_RULES =
  "按材料目的与受众组织内容：先判断用途与受众，再决定结构；逐项归纳事实，明确区分事实、分析、判断与建议。";

/**
 * 组装写作提示词。
 *
 * 组成：用户输入 + 用户明确选择的材料文件（附文件名与哈希）+ 可编辑的场景规则
 * + 不可关闭的写作规则。只带入用户在界面提供的内容；不声称已读取模板、风格库或历史范例。
 * 事实类内容原样放入，并要求模型不得补造数据、文号、文件名或工作成效。
 */
export function buildDraftPrompt(input: AiDraftInput): string {
  const lines: string[] = [];
  lines.push("你为 OmniDesk 工作台起草一份中文工作材料。");
  lines.push("场景：" + input.sceneLabel.trim());
  lines.push("材料目的：" + (input.purpose.trim() || "（未填写，请先提问）"));
  lines.push("受众：" + (input.audience.trim() || "（未填写，请先提问）"));
  lines.push("明确要求：" + (input.requirements.trim() || "（未填写）"));
  lines.push("");
  lines.push("可用素材（原样提供，除此之外不得补充）：");
  lines.push(input.material.trim() || "（未提供素材）");

  const materials = (input.materials ?? []).filter((m) => m.content.trim().length > 0);
  if (materials.length > 0) {
    lines.push("");
    lines.push("用户明确选择的材料文件（原样提供；不得据此推断文件未写出的内容）：");
    for (const m of materials) {
      lines.push("--- 文件：" + m.name + "（sha256:" + m.sha256.slice(0, 12) + "）---");
      lines.push(m.content.trim());
      lines.push("--- 文件结束 ---");
    }
  }

  const sceneRules = (input.sceneRules ?? "").trim();
  const sceneSections = (input.sceneSections ?? "").trim();
  lines.push("");
  lines.push("场景规则（可在工作台流程包中调整）：");
  lines.push(sceneRules || DEFAULT_SCENE_RULES);
  if (sceneSections) {
    lines.push("");
    lines.push("期望输出章节（每行一个，可在流程包中调整）：");
    lines.push(sceneSections);
  }

  lines.push("");
  const writingRules = (input.writingRules ?? []).map((r) => r.trim()).filter((r) => r.length > 0);
  if (writingRules.length > 0) {
    lines.push("写作规则（来自当前 Profile 规则资产）：");
    writingRules.forEach((rule, idx) => lines.push(`${idx + 1}. ${rule}`));
  }
  lines.push("");
  lines.push(
    "输出：先给结构（章节列表），再给初稿正文；最后列出「待确认事项」和「事实来源」（含所用材料文件名）。",
  );
  return lines.join("\n");
}

/** 提交被拒绝（准入关闭、端点未确认等）时的状态。原文原因直接展示，不改写为成功。 */
export function describeSubmitFailure(message: string): {
  phase: "refused";
  message: string;
} {
  const detail = message.trim() || "提交被拒绝，未说明原因。";
  return {
    phase: "refused",
    message:
      detail +
      "\n本次没有生成初稿；输入框内容仍保留在本地界面，未写入任何成果文件。",
  };
}

/** 取消按钮的放行判定：等待中的取消不重复发送。 */
export function cancelGate(input: {
  hasTask: boolean;
  phase: AiDraftPhase;
  status: string | null;
  requestSent: boolean;
}): { send: boolean; reason: string } {
  if (!input.hasTask) return { send: false, reason: "no_task" };
  if (input.requestSent) return { send: false, reason: "already_requested" };
  if (input.status && AI_TERMINAL_STATES.includes(input.status)) {
    return { send: false, reason: "terminal" };
  }
  if (input.phase !== "running") return { send: false, reason: "not_running" };
  return { send: true, reason: "first_request" };
}

/** 只有成功终态且正文非空才允许把文本放进编辑区；其余一律不给正文。 */
export function draftFromTerminal(input: {
  status: string;
  content: string;
  contentOmitted: boolean;
  contentLength: number | null;
  note: string;
}): { phase: AiDraftPhase; text: string | null; message: string } {
  if (input.status !== "succeeded") {
    return {
      phase: input.status === "cancelled" ? "cancelled" : "failed",
      text: null,
      message:
        "任务未成功（状态：" +
        input.status +
        "）。" +
        (input.note ? "说明：" + input.note + "。" : "") +
        "编辑区未写入模型文本。",
    };
  }
  if (input.contentOmitted) {
    return {
      phase: "failed",
      text: null,
      message:
        "引擎报告完成，但输出过长（" +
        String(input.contentLength ?? "未知") +
        " 字符）未返回正文；编辑区保持不变。请缩短素材后重试。",
    };
  }
  if (input.content.trim().length === 0) {
    return {
      phase: "failed",
      text: null,
      message: "引擎报告完成，但没有返回任何文本；编辑区保持不变。",
    };
  }
  return {
    phase: "done",
    text: input.content,
    message: "已取得模型初稿，来源记为「模型生成、未复核」。请逐项核对事实与数据后人工确认再保存。",
  };
}

// 宿主的最小接口：由 api.ts 的 host 实现，测试用替身实现同一契约。
export interface AiDraftHost {
  submitTask(input: {
    promptText: string;
    intent?: string;
    /** 用户原始素材，红线扫描只针对它；不包含拼接的规则与模板。 */
    userInputText?: string;
    /** 引擎要求的工作目录（绝对路径）；由调用方先经宿主准备后传入。 */
    workspaceDir?: string;
  }): Promise<{
    omni_task_id: string;
    status: string;
  }>;
  taskStatus(omniTaskId: string): Promise<{ status: string; note: string }>;
  taskResult(omniTaskId: string): Promise<{
    content: string;
    content_omitted?: boolean;
    content_length?: number;
    note: string;
  }>;
  cancelTask(omniTaskId: string): Promise<{
    phase: string;
    final_state: string | null;
    note: string;
  }>;
}

export interface AiDraftEffects {
  onPhase: (phase: AiDraftPhase) => void;
  onTaskId: (id: string | null) => void;
  onStatus: (status: string | null) => void;
  onDraft: (text: string | null) => void;
  onMessage: (message: string | null) => void;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * 一次提交的完整路径：提交 → 轮询终态 → 取结果。
 * 只有终态为 succeeded 且正文非空才回填；其余只报原因，不写成功。
 */
export async function runAiDraftSubmission(
  host: AiDraftHost,
  input: AiDraftInput,
  intent: string,
  effects: AiDraftEffects,
  options?: {
    intervalMs?: number;
    /** 等待上限（毫秒）。缺省不设上限：任务只按引擎终态收尾，用户取消是唯一主动终止。 */
    timeoutMs?: number | null;
    wait?: (ms: number) => Promise<void>;
    /** 引擎工作目录（绝对路径）；缺省时交由宿主/引擎决定。 */
    workspaceDir?: string | null;
  },
): Promise<void> {
  const interval = options?.intervalMs ?? 1200;
  // 2026-10-23：缺省不再按墙钟放弃等待（底座跑八分钟以上属正常）。
  const timeout = options?.timeoutMs ?? null;
  const wait = options?.wait ?? sleep;
  effects.onDraft(null);
  effects.onMessage(null);
  effects.onStatus(null);
  effects.onTaskId(null);
  effects.onPhase("running");
  let taskId: string;
  try {
    const receipt = await host.submitTask({
      promptText: buildDraftPrompt(input),
      intent,
      // 红线扫描只针对用户原文与用户明确选择的材料正文；不含拼进提示词的规则与模板。
      userInputText: [
        input.purpose,
        input.audience,
        input.requirements,
        input.material,
        ...(input.materials ?? []).map((m) => m.content),
      ]
        .filter((part) => part.trim().length > 0)
        .join("\n"),
      workspaceDir: options?.workspaceDir ?? undefined,
    });
    taskId = receipt.omni_task_id;
    effects.onTaskId(taskId);
    effects.onStatus(receipt.status);
  } catch (e) {
    const refusal = describeSubmitFailure(String(e));
    effects.onPhase(refusal.phase);
    effects.onMessage(refusal.message);
    return;
  }

  const deadline = timeout === null ? null : Date.now() + timeout;
  let status = "";
  for (;;) {
    const snapshot = await host.taskStatus(taskId);
    status = snapshot.status;
    effects.onStatus(status);
    if (AI_TERMINAL_STATES.includes(status)) break;
    if (deadline !== null && Date.now() >= deadline) {
      effects.onPhase("failed");
      effects.onMessage(
        "等待终态超时（最近状态：" +
          status +
          "）。任务可能仍在执行；未回填任何文本，可取消或稍后重试。",
      );
      return;
    }
    await wait(interval);
  }

  const result = await host.taskResult(taskId);
  const outcome = draftFromTerminal({
    status,
    content: result.content,
    contentOmitted: !!result.content_omitted,
    contentLength: result.content_length ?? null,
    note: result.note,
  });
  effects.onPhase(outcome.phase);
  effects.onMessage(outcome.message);
  if (outcome.text !== null) effects.onDraft(outcome.text);
}

/** 请求取消：请求发出不等于取消成功，最终以宿主回读的状态为准。 */
export async function requestAiDraftCancel(
  host: AiDraftHost,
  snapshot: { taskId: string | null; phase: AiDraftPhase; status: string | null },
  effects: {
    onRequestSent: () => void;
    onMessage: (message: string) => void;
    onStatus: (status: string | null) => void;
  },
  requestSent: boolean,
): Promise<"sent" | "skipped"> {
  const gate = cancelGate({
    hasTask: !!snapshot.taskId,
    phase: snapshot.phase,
    status: snapshot.status,
    requestSent,
  });
  if (!gate.send || !snapshot.taskId) return "skipped";
  effects.onRequestSent();
  try {
    const outcome = await host.cancelTask(snapshot.taskId);
    const confirmed =
      outcome.phase === "confirmed_cancelled" || outcome.final_state === "cancelled";
    effects.onMessage(
      confirmed
        ? "取消已确认：宿主报告任务已取消。"
        : "取消未确认（阶段：" +
            outcome.phase +
            "）。" +
            (outcome.note ? outcome.note + "。" : "") +
            "任务可能仍在执行。",
    );
  } catch (e) {
    effects.onMessage("取消请求未获确认：" + String(e) + "。任务可能仍在执行。");
  }
  try {
    effects.onStatus((await host.taskStatus(snapshot.taskId)).status);
  } catch {
    /* 状态读不到时保留原值，不猜测。 */
  }
  return "sent";
}
