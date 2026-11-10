// OD-UI-711：助手消息的执行时间线（只做展示，不保存工具入参/结果原文）。
//
// 数据只来自已归一化的任务事件：tool_activity 的展示字段、thought_delta 的
// 思考文本、unknown_chunk 的类型计数。这里是纯函数，便于独立验证追加语义，
// 也避免把事件归一化逻辑散落在 3000 行的 HomeChat 组件里。

export type TimelineKind = "tool" | "thought" | "note";

export interface TimelineStatusChange {
  status: string;
  at: number;
}

export interface TimelineEntry {
  kind: TimelineKind;
  seq?: number;
  title?: string;
  status?: string;
  text?: string;
  /** 事件发生时间（epoch 秒，沿用宿主 TaskEvent.occurred_at）。 */
  at?: number;
  /** 工具调用标识：只用于把同一调用的状态变化归到一条命令。 */
  callId?: string;
  /** 工具状态变化历史；状态变化时追加，不覆盖旧记录。 */
  statusHistory?: TimelineStatusChange[];
  /** note 条目聚合的未知后台块数量；界面不逐条渲染。 */
  count?: number;
}

const STATUS_LABELS: Record<string, string> = {
  call: "调用",
  result: "结果",
  update: "更新",
  running: "执行中",
  pending: "等待",
  queued: "排队",
  succeeded: "完成",
  success: "完成",
  completed: "完成",
  done: "完成",
  failed: "失败",
  error: "失败",
  cancelled: "已取消",
  canceled: "已取消",
  aborted: "已中断",
};

/** 把引擎给的短状态翻成界面用词；没有对应词时原样保留。 */
export function timelineStatusLabel(status: string): string {
  const raw = status.trim();
  if (!raw) return "更新";
  return STATUS_LABELS[raw.toLowerCase()] ?? raw;
}

function eventKindStatus(eventKind: string): string {
  const kind = eventKind.trim().toLowerCase();
  if (kind === "tool/call") return "调用";
  if (kind === "tool/result") return "结果";
  if (kind.startsWith("tool/")) {
    const suffix = kind.slice("tool/".length);
    return STATUS_LABELS[suffix] ?? (suffix || "更新");
  }
  return "更新";
}

export interface ToolActivityInput {
  seq?: number;
  at: number;
  eventKind?: string;
  toolName?: string;
  title?: string;
  status?: string;
  callId?: string;
}

/** 追加一条工具事件。同一 callId 的状态变化挂在同一条目上。 */
export function appendToolActivity(
  timeline: readonly TimelineEntry[],
  input: ToolActivityInput,
): TimelineEntry[] {
  const next = (Array.isArray(timeline) ? timeline : []).slice();
  const rawStatus = (input.status ?? "").trim();
  const status = rawStatus
    ? timelineStatusLabel(rawStatus)
    : eventKindStatus(input.eventKind ?? "");
  const title = (input.title ?? "").trim() || (input.toolName ?? "").trim() || "未知工具";
  const callId = (input.callId ?? "").trim();
  const isCall = (input.eventKind ?? "").toLowerCase() === "tool/call";

  let index = -1;
  if (callId) {
    index = next.findIndex((entry) => entry.kind === "tool" && entry.callId === callId);
  }
  if (index < 0 && !isCall) {
    // 结果事件在当前通道不一定带 callId；挂到最近一条命令，避免「结果」
    // 被渲染成第二条命令，也避免把空工具名写成一条新记录。
    for (let i = next.length - 1; i >= 0; i -= 1) {
      if (next[i].kind === "tool") {
        index = i;
        break;
      }
    }
  }

  if (index >= 0) {
    const entry = next[index];
    const history = entry.statusHistory ?? [];
    const lastStatus = history.length > 0 ? history[history.length - 1].status : entry.status;
    const changed = status !== "" && status !== lastStatus;
    next[index] = {
      ...entry,
      title: title || entry.title,
      status,
      seq: entry.seq ?? input.seq,
      at: input.at,
      callId: entry.callId ?? (callId || undefined),
      statusHistory: changed ? [...history, { status, at: input.at }] : history,
    };
    return next;
  }

  return [
    ...next,
    {
      kind: "tool",
      seq: input.seq,
      title,
      status,
      at: input.at,
      callId: callId || undefined,
      statusHistory: status ? [{ status, at: input.at }] : [],
    },
  ];
}

export interface ThoughtDeltaInput {
  seq?: number;
  at: number;
  text: string;
}

/** 思考增量并入最近一段思考；工具调用之后到达的思考另起一段。 */
export function appendThoughtDelta(
  timeline: readonly TimelineEntry[],
  input: ThoughtDeltaInput,
): TimelineEntry[] {
  const text = input.text ?? "";
  const base = Array.isArray(timeline) ? timeline : [];
  if (!text) return base.slice();
  const next = base.slice();
  let lastIndex = -1;
  for (let i = next.length - 1; i >= 0; i -= 1) {
    if (next[i].kind !== "note") {
      lastIndex = i;
      break;
    }
  }
  if (lastIndex >= 0 && next[lastIndex].kind === "thought") {
    const entry = next[lastIndex];
    next[lastIndex] = {
      ...entry,
      seq: entry.seq ?? input.seq,
      text: (entry.text ?? "") + text,
      at: input.at,
    };
    return next;
  }
  next.push({
    kind: "thought",
    seq: input.seq,
    title: "思考过程",
    text,
    at: input.at,
  });
  return next;
}

export interface UnknownChunkInput {
  seq?: number;
  at: number;
}

/** unknown_chunk 不逐条建条目：只维护一条聚合计数，正常界面不渲染它。 */
export function addUnknownChunk(
  timeline: readonly TimelineEntry[],
  input: UnknownChunkInput,
): TimelineEntry[] {
  const next = (Array.isArray(timeline) ? timeline : []).slice();
  const index = next.findIndex((entry) => entry.kind === "note");
  if (index >= 0) {
    const entry = next[index];
    const count = (entry.count ?? 0) + 1;
    next[index] = {
      ...entry,
      seq: entry.seq ?? input.seq,
      count,
      status: `${count} 个后台块`,
      at: input.at,
    };
    return next;
  }
  next.push({
    kind: "note",
    seq: input.seq,
    title: "后台事件",
    status: "1 个后台块",
    count: 1,
    at: input.at,
  });
  return next;
}

/** 正常时间线只展示命令与思考；unknown_chunk 聚合仍留在数据里供诊断。 */
export function visibleTimelineEntries(
  timeline: readonly TimelineEntry[],
): Array<TimelineEntry & { kind: "tool" | "thought" }> {
  const entries = Array.isArray(timeline) ? timeline : [];
  return entries.filter(
    (entry): entry is TimelineEntry & { kind: "tool" | "thought" } =>
      entry.kind === "tool" || entry.kind === "thought",
  );
}

export interface TimelineCounts {
  tools: number;
  thoughts: number;
}

export function timelineCounts(timeline: readonly TimelineEntry[]): TimelineCounts {
  const entries = Array.isArray(timeline) ? timeline : [];
  let tools = 0;
  let thoughts = 0;
  for (const entry of entries) {
    if (entry.kind === "tool") tools += 1;
    else if (entry.kind === "thought") thoughts += 1;
  }
  return { tools, thoughts };
}

/** 任务结束后的折叠摘要；后台块不参与计数，避免被当成命令。 */
export function timelineSummary(timeline: readonly TimelineEntry[]): string {
  const { tools, thoughts } = timelineCounts(timeline);
  return `执行了 ${tools} 条命令 · ${thoughts} 段思考`;
}

/** 时间线里的时刻只到秒；事件时间可能是 epoch 秒，也兼容毫秒。 */
export function formatTimelineTime(at?: number): string {
  if (typeof at !== "number" || !Number.isFinite(at) || at <= 0) return "";
  const ms = at < 1e12 ? at * 1000 : at;
  const date = new Date(ms);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}
