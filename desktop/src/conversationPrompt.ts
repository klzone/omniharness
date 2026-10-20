// 对话流工作区的提示词组装（OD-ARCH-031 Part B）。
//
// 刻意不依赖 React、Tauri 或任何传输层：组件与离线验证脚本执行同一份源码。
//
// 事实与边界：
//   * 工作台对话的**每一轮**都是一次独立任务（宿主按 cwd 新建 DSH 会话）；
//     连续性由这里把上文重放进下一轮提示实现——**不是**同一个引擎会话里的多轮。
//   * 上文有长度上限，超出时只保留最近的部分并显式标注截断。
//   * 场景预设的规则来自可编辑的流程包；**不可关闭**的写作规则由 aiDraft 的组装器统一带上。

import { buildDraftPrompt } from "./aiDraft";

export interface ConversationTurn {
  role: "user" | "assistant";
  text: string;
}

export interface ConversationPreset {
  key: string;
  label: string;
  /** 来自可编辑流程包的场景规则；缺省时由组装器回落内置默认。 */
  rules: string | null;
  /** 期望输出章节（每行一个）。 */
  sections: string | null;
}

/** 重放进下一轮提示的上文上限（字符）。 */
export const CONTEXT_MAX_CHARS = 2500;

/** 参与重放的轮次窗口：最近 4 条用户消息 + 最近 2 条助手回复。 */
export const CONTEXT_USER_TURNS = 4;
export const CONTEXT_ASSISTANT_TURNS = 2;

/**
 * 把本轮之前的内容与本轮输入拼成一份上下文。
 *
 * 只带用户与助手的**文本**；不带任务标识、内部状态、路径或任何正文之外的元数据。
 *
 * 窗口化说明：工作台每轮是**独立任务**（引擎侧每次新建会话），连续性靠这里重放上文实现，
 * 所以重放越多、提示词越长、模型越慢。这里只保留最近若干轮 + 字符上限，
 * 换来「上一轮说了什么」还在，同时不让提示词无限膨胀。
 */
export function buildContext(turns: ConversationTurn[], current: string): string {
  const users = turns.filter((t) => t.role === "user" && t.text).slice(-CONTEXT_USER_TURNS);
  const assistants = turns.filter((t) => t.role === "assistant" && t.text).slice(-CONTEXT_ASSISTANT_TURNS);
  const picked = [...users, ...assistants].sort((a, b) => turns.indexOf(a) - turns.indexOf(b));
  const parts: string[] = [];
  for (const t of picked) {
    parts.push((t.role === "user" ? "用户：" : "助手：") + t.text);
  }
  parts.push("用户：" + current);
  const joined = parts.join("\n");
  if (joined.length <= CONTEXT_MAX_CHARS) return joined;
  return "…（上文已按长度截断，只保留最近部分）\n" + joined.slice(-CONTEXT_MAX_CHARS);
}

/**
 * 组装一次提交用的提示词。
 *
 * - 无预设（普通对话）：发上下文本身，保持既有行为。
 * - 有预设：走与三场景同一套组装器，把场景规则、期望章节与上下文一起带上。
 */
export function composeConversationPrompt(input: {
  preset: ConversationPreset | null;
  turns: ConversationTurn[];
  current: string;
}): string {
  const context = buildContext(input.turns, input.current);
  const preset = input.preset;
  if (!preset) return context;
  return buildDraftPrompt({
    sceneLabel: preset.label,
    purpose: "按用户在本轮对话中的要求完成",
    audience: "（由用户在对话中说明）",
    requirements: "",
    material: context,
    materials: [],
    sceneRules: preset.rules,
    sceneSections: preset.sections,
  });
}
