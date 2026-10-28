// 对话流工作区的提示词组装（OD-ARCH-031 Part B；OD-ARCH-034 真多轮）。
//
// 刻意不依赖 React、Tauri 或任何传输层：组件与离线验证脚本执行同一份源码。
//
// 事实与边界：
//   * **续接既有引擎会话时**（OD-ARCH-034）：同一个 DSH 会话里继续对话，上下文由引擎
//     自己持有，**这里不再重放上文**——只发本轮输入。
//   * **新建会话时**（首轮，或旧线程没有会话号、或续接失败降级）：连续性靠这里把上文
//     重放进下一轮提示实现，上文有长度上限，超出时只保留最近的部分并显式标注截断。
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
  /** Profile 规则资产里的写作铁律；null/空数组 = 不输出该段（纯净底座）。 */
  writingRules?: string[] | null;
  /** 场景默认思考档（由界面在空闲时应用，不在这里使用）。 */
  thinking?: string | null;
  /** 场景绑定的 Skill 清单（OD-ARCH-082 S4 注入链路）。 */
  skills?: Array<{ id: string; content: string }> | null;
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
 * 窗口化说明：只在**新建会话**时用（引擎侧还没有上文）。重放越多、提示词越长、模型越慢，
 * 所以这里只保留最近若干轮 + 字符上限，换来「上一轮说了什么」还在，
 * 同时不让提示词无限膨胀。
 */
export function buildContext(turns: ConversationTurn[], current: string): string {
  const users = turns.filter((t) => t.role === "user" && t.text).slice(-CONTEXT_USER_TURNS);
  const assistants = turns.filter((t) => t.role === "assistant" && t.text).slice(-CONTEXT_ASSISTANT_TURNS);
  const picked = [...users, ...assistants].sort((a, b) => turns.indexOf(a) - turns.indexOf(b));
  const parts: string[] = [];
  for (const t of picked) {
    parts.push((t.role === "user" ? "用户：" : "助手：") + t.text);
  }
  // 只对**历史**做长度截断；本轮输入必须完整保留。
  //
  // 本轮输入里可能带着用户引用的文件原文（OD-ARCH-077）。如果和上文一起截断，
  // 长附件会被削成尾部一小段，模型看不到完整依据，却仍然会“照着写”，
  // 这比明确拒绝更危险。附件整体长度由界面在发送前单独校验（有明确上限）。
  const history = parts.join("\n");
  const trimmed =
    history.length <= CONTEXT_MAX_CHARS
      ? history
      : "…（上文已按长度截断，只保留最近部分）\n" + history.slice(-CONTEXT_MAX_CHARS);
  return (trimmed ? trimmed + "\n" : "") + "用户：" + current;
}

/**
 * 组装一次提交用的提示词。
 *
 * - 无预设（普通对话）：发上下文本身，保持既有行为。
 * - 有预设：走与三场景同一套组装器，把场景规则、期望章节与上下文一起带上。
 * - continuation=true（OD-ARCH-034）：**续接既有引擎会话**，引擎侧已持有完整上文，
 *   这里只发本轮输入，不再重放上文（避免提示词膨胀与重复内容）。
 */
export function composeConversationPrompt(input: {
  preset: ConversationPreset | null;
  turns: ConversationTurn[];
  current: string;
  continuation?: boolean;
  /**
   * 本场景的输入模式（OD-ARCH-068 S3，OD-ARCH-089 改为按输入判定）：
   *   report（默认）= 完整组装（场景规则 + 章节 + Profile 写作铁律）；
   *   note = 随手记：写盘已由宿主完成，这里只带一句缺项追问指令。
   *
   * 随手记是「先记下来」的低负担入口：带铁律与章节会让首字明显变慢，
   * 而用户此刻要的只是把事实留住、顺带补上缺的要素。周报仍走完整组装。
   */
  mode?: "note" | "report";
  /**
   * 当周工作记录（OD-ARCH-089 S2）：report 模式由界面先读出来传进来，
   * 模型不必猜「这周做了什么」。null = 没读（非周记录场景）。
   */
  worklog?: { week: string; exists: boolean; content: string } | null;
  /**
   * 随手记判定所需的当周索引（OD-ARCH-104）：今天、当周标签、已有条目清单。
   *
   * 索引来自宿主读到的真实文件（`describeIndex` 整理），模型据此判断一句话是
   * 记下来、还是把某几条挪到别的日期——它看不到文件本身，只能看这份索引。
   */
  noteIndex?: { week: string; today: string; entries: string } | null;
}): string {
  const context = input.continuation
    ? "用户：" + input.current
    : buildContext(input.turns, input.current);
  const preset = input.preset;
  if (!preset) return context;
  if (input.mode === "note") {
    // OD-ARCH-105：平台只给数据与写盘边界，判断口径全部来自绑定 Skill。
    //
    // OmniDesk 是通用工作台：哪个场景、怎么判断一句话、输出什么形状，
    // 都属于场景资产（Skill / 流程包），不属于平台。平台在这里只做三件事：
    //   1. 把宿主读到的当周记录整理成索引交出去（数据）；
    //   2. 说明写入由宿主执行、模型不碰文件（写盘边界，对所有场景一致）；
    //   3. 把绑定的 Skill 原文注入（口径）。
    // 换一套 Skill 就是另一套工作流，代码一行不用改；没有绑定时模型只回人话，
    // 界面按兜底路径记录原文。此前把业务口径抄在提示词里，既与 Skill 重复，
    // 也让平台带上业务限制——本轮移除。
    // OD-ARCH-082 S4：随手记分支保持轻装，最多注入一个绑定 Skill。
    const noteSkills = (preset.skills ?? []).filter((s) => s.content.trim().length > 0).slice(0, 1);
    const noteSkillLines: string[] = [];
    if (noteSkills.length > 0) {
      const s = noteSkills[0];
      const raw = s.content.trim();
      const trimmed =
        raw.length <= CONTEXT_MAX_CHARS ? raw : raw.slice(0, CONTEXT_MAX_CHARS) + "\n…（技能说明已截断）";
      noteSkillLines.push("配套技能指引（" + s.id + "）：");
      noteSkillLines.push(trimmed);
      noteSkillLines.push("");
    }
    const idx = input.noteIndex ?? null;
    const indexLines = idx
      ? [
          "今天是 " + idx.today + "，当周文件是 " + idx.week + "。",
          "当周已有记录（每行：日期 #序号 正文，序号从 0 起）：",
          idx.entries.trim() || "（本周还没有记录）",
          "",
        ]
      : [];
    const headLines = noteSkills.length > 0
      ? [
          "按下方「配套技能指引」处理用户这一句；需要落盘的内容按该指引约定的格式交回，",
          "由工作台校验后写入。技能没说要做的，不要自行添加。",
        ]
      : ["本场景没有绑定技能：不要臆测判断口径，只用一句话说明你没有可用的处理指引。"];
    return [
      ...headLines,
      "",
      // 写盘边界对任何场景一致，属于平台契约，不下放给 Skill：
      // 模型没有文件通道，声称已写入只会误导用户。
      "写入由工作台执行：你不需要、也不允许再写文件或改写已有条目，",
      "也不要声称已经写入或已经移动。",
      "",
      ...indexLines,
      ...noteSkillLines,
      context,
    ].join("\n");
  }
  const material = input.worklog
    ? (input.worklog.exists
        ? [
            context,
            "",
            `本周工作记录（${input.worklog.week}，原样提供，只取去向为「本周报引用」的条目）：`,
            input.worklog.content.trim(),
          ].join("\n")
        : [
            context,
            "",
            `本周工作记录（${input.worklog.week}）：当周文件还不存在，本周没有记录。`,
            "没有记录就如实说明「本周还没有记录」，不得虚构事项。",
          ].join("\n"))
    : context;
  return buildDraftPrompt({
    sceneLabel: preset.label,
    purpose: "按用户在本轮对话中的要求完成",
    audience: "（由用户在对话中说明）",
    requirements: "",
    material,
    materials: [],
    sceneRules: preset.rules,
    sceneSections: preset.sections,
    writingRules: preset.writingRules ?? null,
    skills: preset.skills ?? null,
  });
}
