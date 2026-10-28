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
    // OD-ARCH-104：语义判断交给模型（这句话是指令还是内容），
    // 宿主只做校验与落盘。这里要求模型在正文之后给出一个结构化判定块，
    // 界面把它剥掉、校验通过后才写文件。
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
    return [
      "你是 OmniDesk 工作台的随手记助手，负责把用户这句话补进本周工作记录。",
      "用户这一句可能是一段事实，也可能是在指挥你",
      "（「这些都是周一的」「把上一条挪到周一」「刚才记错了」）。**分类是你的活**：",
      "先判断这句话里哪些是事实、哪些是指令，再决定写什么、挪什么。",
      "",
      "写入由工作台执行：你不需要、也不允许再写文件或改写已有条目；",
      "你只输出判定，界面校验通过后才落盘。",
      "",
      "判断原则：",
      "1. 一句话里混着事实与指令时，只把**事实**放进 records；指令部分只体现在 moves / reply。",
      "   例：「上杭教学和参观紫金矿业都是周一的行动」——「上杭教学」「参观紫金矿业」是事实，",
      "   「都是周一的」是日期指令；若这两条已在别的日期，用 moves 挪到周一。",
      "2. 用户说「都是周一的」「全部调整到周一」时，先在上面索引里找出对应条目再给 moves；",
      "   找不到就 action=answer 并说明，绝不要新建一条记录冒充。",
      "3. 无日期的细节句，若明显在续述上一条（「还参观了」「随后」「就是周一上午」），",
      "   跟随上一条的日期；全新主题用今天。",
      "4. 「就是周一上午」这类确认句只确认日期，不写新内容：action=confirm。",
      "5. 纯提问、查记录、要求解释：action=answer，不写文件。",
      "6. records 的 text 必须逐字取自用户原话的连续片段，不改写、不合并、不润色；",
      "   工作台会逐字比对，对不上整份判定作废。",
      "7. 同一句话不要既记一遍又挪一遍：已写过的内容用 moves 处理。",
      "",
      "输出格式（两段，先人话后判定）：",
      "第一段：一句给用户的话（含六要素 时间/地点/人物/事件/影响/感受 的缺项追问）。",
      "不要复述原文，不要扩写，不要生成周报，不要列清单；几项都齐就只说「已记录」。",
      "第二段：单独一行 <omnidesk-verdict>，紧跟 JSON，再单独一行 </omnidesk-verdict>：",
      '{"action":"record|move|confirm|answer",',
      ' "records":[{"text":"原话片段","date":"2026-10-26"}],',
      ' "moves":[{"date":"2026-10-28","index":0,"to":"2026-10-26"}],',
      ' "reply":"一句给用户的话"}',
      "日期一律 YYYY-MM-DD，只能是本周且不晚于今天；index 用索引里的 #序号。",
      "没有要写的就写 records:[]，没有要挪的就写 moves:[]。",
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
