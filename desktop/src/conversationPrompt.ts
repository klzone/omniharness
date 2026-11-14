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
import type { PeriodReviewPeriod } from "./workIntent";

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
  /**
   * 按流程分开的绑定（OD-ARCH-108）：同一场景的不同流程各有一套底线技能。
   *
   * 随手记与周报共用「工作记录」场景，但平台该注入的底线技能不同。意图在提交时
   * 才判定，所以两组一起带出来，按 mode 取用；缺省时回落到 skills。
   */
  skillsByFlow?: {
    note?: Array<{ id: string; content: string }> | null;
    report?: Array<{ id: string; content: string }> | null;
    summary?: Array<{ id: string; content: string }> | null;
    periodReview?: Array<{ id: string; content: string }> | null;
  } | null;
}

/** 周期复盘的标题深度（OD-WR-505），与宿主 review_store.rs 和 Skill 保持一致。 */
export const PERIOD_REVIEW_SECTIONS: Record<PeriodReviewPeriod, readonly string[]> = {
  weekly: [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
  ],
  monthly: [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
  ],
  quarterly: [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "跨月对照",
    "主题归纳",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
  ],
  annual: [
    "本周期工作概况",
    "重点工作事项",
    "工作成果与完成情况",
    "数据汇总",
    "趋势对照",
    "跨月对照",
    "主题归纳",
    "全年脉络",
    "问题与不足",
    "经验与思考",
    "未完成事项",
    "下周期建议",
    "长期建议",
  ],
};

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
/**
 * 取本轮该注入的 Skill（OD-ARCH-108）。
 *
 * 同一场景可以有多条流程（工作记录 = 随手记 + 周报），各自只绑自己的底线技能。
 * 按 mode 取对应那一组；该流程没有单独配置时回落到场景级 skills。
 * 当日摘要例外：没有 weekly-daily-summary 绑定时不预注入，避免混入随手记的底线技能。
 * 空数组与 null 都表示「这一轮不注入」，不在这里补默认值——补默认会绕过规则资产。
 */
export function presetSkillsFor(
  preset: ConversationPreset,
  mode: "note" | "report" | "daily-summary" | "period-review",
): Array<{ id: string; content: string }> | null {
  if (mode === "daily-summary") {
    // 摘要不复用场景级绑定：工作记录场景的通用底线是随手记的 memory-agent，
    // 它不是摘要口径。没配 weekly-daily-summary 就不预注入，交给下方输出契约兜底。
    const summary = preset.skillsByFlow?.summary;
    return Array.isArray(summary) ? summary : null;
  }
  if (mode === "period-review") {
    // 周期复盘只读 weekly-period-review 绑定，不借随手记的 memory-agent。
    const review = preset.skillsByFlow?.periodReview;
    return Array.isArray(review) ? review : null;
  }
  const flow = preset.skillsByFlow?.[mode];
  if (Array.isArray(flow)) return flow;
  return preset.skills ?? null;
}

export function composeConversationPrompt(input: {
  preset: ConversationPreset | null;
  turns: ConversationTurn[];
  current: string;
  continuation?: boolean;
  /**
   * 本场景的输入模式（OD-ARCH-068 S3，OD-ARCH-089 改为按输入判定）：
   *   report（默认）= 完整组装（场景规则 + 章节 + Profile 写作铁律）；
   *   note = 随手记：写盘已由宿主完成，这里只带一句缺项追问指令。
   *   daily-summary = 当日摘要：带当天正式记录、五段输出契约与摘要 Skill。
   *   period-review = 周期复盘：带周期材料、按周期深度的标题契约与复盘 Skill。
   *
   * 随手记是「先记下来」的低负担入口：带铁律与章节会让首字明显变慢，
   * 而用户此刻要的只是把事实留住、顺带补上缺的要素。周报仍走完整组装。
   */
  mode?: "note" | "report" | "daily-summary" | "period-review";
  /**
   * 当日摘要材料（OD-WR-503）：界面从宿主记录列表读出的当天正式记录全文索引。
   * 模型不碰文件，材料与输出契约由此处带进提示词。
   */
  dailySummary?: {
    date: string;
    material: string;
    recordCount: number;
  } | null;
  /** 周期复盘材料（OD-WR-505）：宿主汇集的记录、任务统计与既往复盘标题。 */
  periodReview?: {
    period: PeriodReviewPeriod;
    label: string;
    start: string;
    end: string;
    material: string;
    recordCount: number;
    taskCount: number;
    taskStats: { completed: number; uncompleted: number; overdue: number };
    priorReviews: string[];
  } | null;
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
  /** OD-ARCH-141 S5：已检索到的知识库上下文（≤3000 字）。 */
  knowledgeContext?: string | null;
}): string {
  const context = input.continuation
    ? "用户：" + input.current
    : buildContext(input.turns, input.current);
  const preset = input.preset;
  // knowledgeContext 已由 knowledgeRetrieval 带上引用约束标题，这里只负责放位置。
  const knowledgeBlock = input.knowledgeContext?.trim()
    ? [input.knowledgeContext.trim(), ""]
    : [];
  if (!preset) return [context, ...knowledgeBlock].filter(Boolean).join("\n");
  if (input.mode === "daily-summary") {
    // 当日摘要：宿主已经把当天正式记录整理成全文索引；这里只给数据、输出契约与
    // 绑定的 daily-summary Skill。写盘与引用校验由宿主完成，模型不碰文件。
    const summary = input.dailySummary ?? null;
    const skills = (presetSkillsFor(preset, "daily-summary") ?? [])
      .filter((s) => s.content.trim().length > 0)
      .slice(0, 1);
    const skillBlock: string[] = [];
    if (skills.length > 0) {
      const s = skills[0];
      const raw = s.content.trim();
      const trimmed =
        raw.length <= CONTEXT_MAX_CHARS
          ? raw
          : raw.slice(0, CONTEXT_MAX_CHARS) + "\n…（技能说明已截断）";
      skillBlock.push("配套技能指引（" + s.id + "）：", trimmed, "");
    }
    const materialLines = summary
      ? [
          "当日正式记录（" + summary.date + "，已排除草稿，共 " + summary.recordCount + " 条）：",
          summary.material.trim(),
          "",
        ]
      : [
          "当日正式记录：界面没有读出材料。",
          "没有材料时不得生成摘要，只说明无法读取当日记录。",
          "",
        ];
    const contract = [
      "输出契约：只返回 Markdown 正文，不要 frontmatter、路径或前言。",
      "严格使用以下五个二级标题，顺序不变：",
      "## 今日工作概况",
      "## 重点事项",
      "## 重要思考",
      "## 需要跟进的问题",
      "## 与任务管理的关联",
      "每段至少标注一个真实记录编号，格式为 #记录编号（例如 #wr-…）；",
      "只能引用上面材料里出现过的编号。记录没写到的事实、进度或结论不要写。",
      "材料为空时，五段都如实写「当日无记录」，不要编造。",
      "",
    ];
    return [
      ...contract,
      ...materialLines,
      ...skillBlock,
      ...knowledgeBlock,
      context,
    ].join("\n");
  }
  if (input.mode === "period-review") {
    // 周期复盘：材料与引用全部由宿主汇集/校验；模型只按周期深度写正文。
    const review = input.periodReview ?? null;
    const skills = (presetSkillsFor(preset, "period-review") ?? [])
      .filter((s) => s.content.trim().length > 0)
      .slice(0, 1);
    const skillBlock: string[] = [];
    if (skills.length > 0) {
      const s = skills[0];
      const raw = s.content.trim();
      const trimmed =
        raw.length <= CONTEXT_MAX_CHARS
          ? raw
          : raw.slice(0, CONTEXT_MAX_CHARS) + "\n…（技能说明已截断）";
      skillBlock.push("配套技能指引（" + s.id + "）：", trimmed, "");
    }
    const sections = review ? PERIOD_REVIEW_SECTIONS[review.period] : [];
    const materialLines = review
      ? [
          "复盘周期：" + review.period + "（" + review.label + "），" +
            review.start + " 至 " + review.end + "。",
          "宿主汇集：正式记录 " + review.recordCount + " 条，任务 " + review.taskCount +
            " 条（已完成 " + review.taskStats.completed + " / 未完成 " + review.taskStats.uncompleted +
            " / 逾期 " + review.taskStats.overdue + "）。",
          review.material.trim(),
          "",
        ]
      : [
          "复盘材料：界面没有读出材料。",
          "没有材料时不得生成复盘，只说明无法读取本周期记录。",
          "",
        ];
    const contract = [
      "输出契约：只返回 Markdown 正文，不要 frontmatter、路径或前言。",
      "严格使用以下二级标题，顺序不变：",
      ...sections.map((title) => "## " + title),
      "每段至少标注一个真实编号，格式为 #编号（记录 #wr-…，任务 #t-…）；",
      "只能引用上面材料里出现过的编号。材料没写到的事实、进度、数据或结论不要写。",
      "材料为空时，逐段如实写「本周期无记录」，不要编造。",
      "复盘建议不得自行转成任务；任务创建必须由用户另行确认。",
      "",
    ];
    return [
      ...contract,
      ...materialLines,
      ...skillBlock,
      ...knowledgeBlock,
      context,
    ].join("\n");
  }
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
    // OD-ARCH-108：取随手记这一条流程自己的底线技能，而不是场景的全部绑定。
    const noteSkills = (presetSkillsFor(preset, "note") ?? [])
      .filter((s) => s.content.trim().length > 0)
      .slice(0, 1);
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
          "已有记录（最近 14 天，每行：日期 #序号 正文，序号从 0 起）：",
          idx.entries.trim() || "（本周还没有记录）",
          "",
        ]
      : [];
    indexLines.push("补录窗口：最近 14 天（含上一周）；用户给出日期时按该日期补录，跨周也照此。");
    const headLines = noteSkills.length > 0
      ? [
          "按下方「配套技能指引」处理用户这一句；需要落盘的内容按该指引约定的格式交回，",
          "由工作台校验后写入。技能没说要做的，不要自行添加。",
        ]
      : ["本场景没有绑定技能：不要臆测判断口径，只用一句话说明你没有可用的处理指引。"];
    return [
      ...headLines,
      "",
      // 随手记只做判断：索引与技能正文已经在本消息里，模型不需要再读文件或联网。
      "本轮只做判断，不要调用任何工具：read / grep / glob / bash / write / edit / skill 与联网工具都不要用；",
      "下方已给出索引与技能指引，直接按它们输出判定块。",
      "",
      // 写盘边界对任何场景一致，属于平台契约，不下放给 Skill：
      // 模型没有文件通道，声称已写入只会误导用户。
      "写入由工作台执行：你不需要、也不允许再写文件或改写已有条目，",
      "也不要声称已经写入或已经移动。",
      "",
      ...indexLines,
      ...noteSkillLines,
      ...knowledgeBlock,
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
  const materialWithKnowledge = knowledgeBlock.length
    ? material + "\n\n" + knowledgeBlock.join("\n")
    : material;
  return buildDraftPrompt({
    sceneLabel: preset.label,
    purpose: "按用户在本轮对话中的要求完成",
    audience: "（由用户在对话中说明）",
    requirements: "",
    material: materialWithKnowledge,
    materials: [],
    sceneRules: preset.rules,
    sceneSections: preset.sections,
    writingRules: preset.writingRules ?? null,
    // OD-ARCH-108：周报只注入周报自己的底线技能（memory-report），
    // 不再把随手记那一条一起塞进来。
    skills: presetSkillsFor(preset, "report"),
  });
}
