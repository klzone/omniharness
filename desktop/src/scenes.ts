// 场景注册表（OD-ARCH-033）。
//
// 三个工作场景共用同一套骨架与同一套对话；**场景只决定注入什么**：
//   * 对话开场预设（工作记录 / 文案 / 项目 的整理与起草规则）；
//   * 右栏看什么（成果档案，或「进度 / 阶段成果」两个页签）；
//   * 提醒/建议栏的规则提醒取自哪一份确定性汇总。
//
// 边界：场景**不**改变任何判定逻辑——进度与提醒一律来自既有的确定性汇总命令，
// 不调模型算、不编造数字；「AI 复盘建议」是可选的模型调用，默认关闭。

export type SceneKey = "weekly" | "writing" | "project";

export interface SceneDef {
  key: SceneKey;
  /** 场景名（对话与成果的归属标签）。 */
  label: string;
  /** Dock 上的入口名。 */
  dockLabel: string;
  /** 成果归档按这个 scenario 过滤（与宿主保存成果时的 scenario 一致）。 */
  scenario: "weekly_report" | "material_writing" | "project_management";
  /** 对话开场预设（与 conversationPrompt 的预设键一致）。 */
  preset: "weekly" | "writing" | "project";
  /** 是否带「进度」页签（项目跟进）。 */
  hasProgress: boolean;
  /** 规则提醒用哪份确定性汇总。 */
  summary: "weekly" | "project" | null;
  /** 右栏空态的一句话说明（诚实描述当前有哪些数据）。 */
  emptyHint: string;
  /**
   * 可选的英文副标题（OD-ARCH-093）。缺省或空串时整个副标题不渲染，不留空行。
   * 键：priority（四象限）/ log（本周记录）/ artifacts（成果文档库）/ history（会话历史）/
   * records / weekly（顶部前两项指标）/ report / overdue（周报状态与逾期）；
   * 写作场景另有 studio / overview / recent / continue / inspiration / pipeline / stale /
   * draft / finished / published（OD-ARCH-094）；项目跟进另有 projects / timeline（OD-ARCH-098）。
   */
  subtitles?: Partial<
    Record<
      | "priority"
      | "log"
      | "artifacts"
      | "history"
      | "records"
      | "weekly"
      | "report"
      | "overdue"
      | "studio"
      | "overview"
      | "recent"
      | "continue"
      | "inspiration"
      | "pipeline"
      | "stale"
      | "draft"
      | "finished"
      | "published"
      | "projects"
      | "timeline",
      string
    >
  >;
}

export const SCENES: Record<SceneKey, SceneDef> = {
  weekly: {
    key: "weekly",
    label: "工作记录",
    dockLabel: "工作记录",
    scenario: "weekly_report",
    preset: "weekly",
    hasProgress: false,
    summary: "weekly",
    emptyHint:
      "本场景还没有保存的成果。成果档案只列本场景自己保存的文件（成果目录下的平铺文件），不读你其它目录的内容。",
    subtitles: {
      priority: "PRIORITY",
      log: "WEEKLY LOG",
      artifacts: "ARTIFACTS",
      history: "HISTORY",
      records: "RECORDS",
      weekly: "FOR WEEKLY",
      report: "WEEKLY REPORT",
      overdue: "OVERDUE",
    },
  },
  writing: {
    key: "writing",
    label: "文案写作",
    dockLabel: "文案写作",
    scenario: "material_writing",
    preset: "writing",
    hasProgress: false,
    summary: null,
    emptyHint:
      "还没有写作成果。在首页开一段「文案写作」对话，把想法聊成形，保存后就会出现在这里；本页只读成果文件，不在这里编辑。",
    subtitles: {
      studio: "WRITING STUDIO",
      overview: "OVERVIEW",
      recent: "RECENT",
      continue: "CONTINUE",
      inspiration: "INSPIRATION",
      pipeline: "PIPELINE",
      artifacts: "ARTIFACTS",
      stale: "REVISIT",
      draft: "DRAFTS",
      finished: "FINISHED",
      published: "PUBLISHED",
    },
  },
  project: {
    key: "project",
    label: "项目跟进",
    dockLabel: "项目跟踪",
    scenario: "project_management",
    preset: "project",
    hasProgress: true,
    summary: "project",
    emptyHint:
      "本场景还没有保存的成果。左侧卡片与右侧时间线都来自既有的确定性汇总，不是模型算的。",
    subtitles: {
      projects: "PROJECTS",
      timeline: "TIMELINE",
    },
  },
};

export const SCENE_KEYS = Object.keys(SCENES) as SceneKey[];

export function isSceneKey(value: string | null | undefined): value is SceneKey {
  return !!value && value in SCENES;
}

/**
 * 场景标识的等价判断。
 *
 * 前端用下划线形状（`weekly_report`，也是保存时的请求值），而宿主列举成果时
 * 从文件名还原出的是连字符形状（`weekly-report`）。两者指的是同一场景，
 * 直接字符串比较会让右栏与提醒栏永远是空的，因此统一在这里归一后再比。
 */
export function sameScenario(a: string, b: string): boolean {
  const norm = (v: string) => v.trim().toLowerCase().replace(/-/g, "_");
  return norm(a) === norm(b);
}
