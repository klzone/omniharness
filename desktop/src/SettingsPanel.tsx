// 设置（OD-ARCH-061 S2）：唯一设置入口，按功能分页：
// 模型与端点 / 对话分类 / 成果与文档库 / 权限 / 引擎诊断 / 关于（高级）。
//
// 只负责展示与调用宿主命令；路径校验、协议校验、凭据写入、原子落盘都在 Rust 侧完成。
// 这里不拼接 shell、不读写文件，也不推断“设置已生效”——一律以宿主返回为准。
// 密钥是单向输入：保存后立刻清空输入框，界面只显示“已配置 / 来源”。

import { useCallback, useEffect, useState, type ReactNode } from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import {
  host,
  invalidateEngineFactsCache,
  onProfilePlugins,
  scheduler as schedulerApi,
  shell,
} from "./api";
import { PendingWritePanel } from "./shell/PendingWriteCard";
import { EngineTechSlice } from "./shell/EngineTechSlice";
import { refreshThreadCategories } from "./threadCategories";
import { SCENES } from "./scenes";
import type {
  AdmissionFacts,
  CapabilityView,
  CapabilitySkill,
  CategoryInitReceipt,
  KnowledgeVaultRootStatus,
  KnowledgeVaultScan,
  LibraryRootInfo,
  LocalRootInfo,
  ModelCategory,
  ModelEntry,
  ModelPermissions,
  ModelProbeReceipt,
  ModelSettings,
  ModelSettingsView,
  ProviderSettings,
  ProfileChoiceView,
  ProfileAssetsView,
  ParkedSkillView,
  ProfileTrashEntry,
  ProfileBundleListing,
  ProfileAssetsRepairReceipt,
  ProfilePluginInstallReceipt,
  ProfilePluginAddReceipt,
  ProfileCustomEntry,
  ProfileDeclPayload,
  ProfileRulesView,
  SchedulerConfig,
  SchedulerSnapshot,
  ThreadCategoryView,
} from "./types";

const SOURCE_LABEL: Record<string, string> = {
  user: "你在设置中选择的目录",
  default: "默认目录",
  env_override: "开发/测试隔离变量覆盖（本条设置暂不生效）",
};

/** 设置分页（OD-ARCH-061 S2）。 */
type SettingsTab =
  | "model"
  | "assets"
  | "categories"
  | "rules"
  | "capabilities"
  | "library"
  | "permissions"
  | "engine"
  | "about";

const SETTINGS_TABS: Array<{ key: SettingsTab; label: string }> = [
  { key: "model", label: "模型与端点" },
  { key: "assets", label: "Profile 资产" },
  { key: "categories", label: "对话分类" },
  { key: "rules", label: "规则资产" },
  { key: "capabilities", label: "能力管理" },
  { key: "library", label: "成果与文档库" },
  { key: "permissions", label: "权限" },
  { key: "engine", label: "引擎诊断" },
  { key: "about", label: "关于" },
];

// 类别只区分“本机回环 / 非回环”，不承诺组织归属。
const CATEGORY_LABEL: Record<ModelCategory, string> = {
  local: "本机回环",
  private: "非回环 · 私有/链路本地地址",
  public: "非回环 · 公网地址",
  unknown: "无法确认",
};

const API_CHOICES: Array<{ value: string; label: string }> = [
  { value: "openai-completions", label: "openai-completions（兼容 /chat/completions）" },
  { value: "openai-responses", label: "openai-responses（兼容 /responses）" },
  { value: "anthropic-messages", label: "anthropic-messages（Anthropic 原生）" },
];

const EFFORT_CHOICES: Array<{ value: string; label: string }> = [
  { value: "default", label: "跟随模型默认" },
  { value: "low", label: "低（快、省）" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "max", label: "最高" },
];

const PERMISSION_ITEMS: Array<{
  key: keyof ModelPermissions;
  title: string;
  detail: string;
}> = [
  {
    key: "local",
    title: "本机模型",
    detail: "模型跑在你自己这台电脑上，内容不往外发。这一项最安全，可以放心开。",
  },
  {
    key: "private",
    title: "局域网 / 单位内网",
    detail:
      "地址不是本机，也不是公网，通常是单位内网或家里的局域网。但从地址看不出它到底属于谁、中途会不会转到别处，开之前请自己确认这个模型的授权和数据去向。",
  },
  {
    key: "public",
    title: "公网模型服务",
    detail:
      "指的是互联网上的模型服务（如各家云 API）。内容会发到你电脑以外，可能经过单位代理，也可能出境。开之前请自己确认这家服务商的授权和数据去向。",
  },
];

/** 连接测试的失败归类 → 给用户看的下一步（只描述可做的事）。 */
const PROBE_LABEL: Record<string, string> = {
  ok: "连接成功",
  auth: "鉴权失败",
  network: "连不上",
  format: "地址能连上，但返回的不是可读的模型列表",
  unsupported: "该协议不支持列模型",
  local: "本机 DSH 侧的问题",
};

/** 这次探测到底用了哪把密钥（OD-ARCH-065 S1 的三态口径）。 */
const KEY_SOURCE_LABEL: Record<string, string> = {
  stored: "沿用该提供商已保存的密钥",
  input: "使用本次输入的新密钥（不保存）",
  none: "没有带密钥",
};

/** 卡片头：左侧标题与状态摘要，右侧主操作。 */
function CardHead({
  title,
  status,
  children,
}: {
  title: string;
  status?: string | null;
  children?: ReactNode;
}) {
  return (
    <div className="card-head">
      <div className="card-head-main">
        <h2 className="pane-title">{title}</h2>
        {/* 状态行是这一页唯一的进度信号，读屏与验收脚本都靠它；
            没有 `role="status"` 时 WebKit 的无障碍树会把它当成装饰性文本丢掉。 */}
        {status ? (
          <p className="card-status mono" role="status" aria-live="polite">
            {status}
          </p>
        ) : null}
      </div>
      {children ? <div className="actions card-head-actions">{children}</div> : null}
    </div>
  );
}

/** 场景键 → 用户看得懂的名字；未知键原样显示，不猜。 */
function sceneLabel(key: string): string {
  // OD-ARCH-108：工作记录场景按流程拆了绑定，标签要让人看出是哪一条流程。
  if (key === "weekly-note") return SCENES.weekly.label + " · 随手记";
  if (key === "weekly-report") return SCENES.weekly.label + " · 周报";
  if (key === "weekly" || key === "writing" || key === "project") {
    return SCENES[key].label;
  }
  return key;
}

/**
 * 绑定页固定列出的场景（OD-ARCH-129）。
 *
 * 与场景注册表和 OD-ARCH-108 的流程拆分一致：工作记录一条场景级绑定，
 * 再加「随手记 / 周报」两条流程级绑定，另外两个场景各一条。
 * 用户手写的其它场景键不会被丢掉——保存时原样带回去。
 */
type BindingScene =
  | "weekly"
  | "weekly-note"
  | "weekly-report"
  | "writing"
  | "project";

const BINDING_SCENES: BindingScene[] = [
  "weekly",
  "weekly-note",
  "weekly-report",
  "writing",
  "project",
];

/** 每个场景最多绑定的 Skill 数，与宿主 validate 的上限一致。 */
const MAX_BINDINGS_PER_SCENE = 3;

/** 一句人话说明这条绑定管什么，避免用户对着键名猜。 */
const BINDING_SCENE_HINTS: Record<BindingScene, string> = {
  weekly: "工作记录的通用底线；随手记 / 周报没单独配时用这一条",
  "weekly-note": "只在你随手记时预注入",
  "weekly-report": "只在生成周报时预注入",
  writing: "文案写作对话的底线",
  project: "项目跟进对话的底线",
};

/**
 * 当前 Profile 的 Skill 作用域说明（OD-ARCH-107）。
 *
 * 这一页列的是磁盘上的 Skill 目录——所有 Profile 共用一份。真正决定「这次会预注入
 * 什么」的是 Profile 规则包里的 skill_bindings。两者必须分开讲，否则切了 Profile
 * 页面看着一模一样，用户没法判断到底加载了什么。
 */
function SkillScopeSummary({ caps }: { caps: CapabilityView }) {
  if (caps.standard) {
    return (
      <p className="hint" data-testid="skill-scope-standard">
        当前是<strong>基础标准</strong>：不挂业务场景，因此<strong>不预注入任何 Skill</strong>。
        下面列出的 Skill 仍可由模型在处理任务时按需加载。
      </p>
    );
  }
  if (caps.rules_error) {
    return (
      <p className="hint" data-testid="skill-scope-broken">
        规则包读取失败（{caps.rules_error}），因此现在没有任何场景 Skill 会被预注入。
        修好规则文件后回到这一页重新读取即可。
      </p>
    );
  }
  if (!caps.rules_loaded) {
    return (
      <p className="hint" data-testid="skill-scope-empty">
        当前 Profile 没有规则包，因此不预注入任何 Skill。
        下面列出的 Skill 仍可由模型按需加载。
      </p>
    );
  }
  return (
    <p className="hint" data-testid="skill-scope-bound">
      当前规则包属于 <strong>{caps.rules_profile ?? "未命名"}</strong>：
      共 <strong>{caps.preinjected.length}</strong> 个 Skill 会被预注入（按场景去重），
      它们会在对应场景开始时随提示词一起发出；
      其余 Skill 只在模型需要时自行加载。同一场景里的不同流程（如随手记与周报）
      各绑自己的底线技能，不会互相带上。
    </p>
  );
}

/** 一条 Skill：名称、说明、绑定场景。 */
function SkillRow({ skill }: { skill: CapabilitySkill }) {
  return (
    <div className="provider-item" data-testid={"skill-row-" + skill.id}>
      <span className="name">{skill.name}</span>
      <span className="sub">{skill.description ?? "（没有说明）"}</span>
      <span className="sub">
        {skill.modified_at
          ? "最近修改 " + new Date(skill.modified_at * 1000).toLocaleDateString()
          : "修改时间未知"}
      </span>
      {skill.bindings.length > 0 ? (
        <span className="skill-tags">
          {skill.bindings.map((b) => (
            <span
              className={b.effective ? "skill-tag on" : "skill-tag"}
              key={b.scene}
              data-testid={"skill-tag-" + skill.id + "-" + b.scene}
            >
              {sceneLabel(b.scene)}
              {b.effective ? "" : "（超出预注入上限，不注入）"}
              {b.resolved ? "" : " · 正文缺失"}
            </span>
          ))}
        </span>
      ) : null}
    </div>
  );
}

/** 新建配置时的空白档案（不预填任何真实端点或密钥）。 */
function blankSettings(): ModelSettings {
  return {
    schema: "omnidesk.model-settings.v2",
    providers: [],
    default: { provider: "", model: "" },
    // 旧版遗留字段：宿主已不再按秒数收尾，这里写入默认值只为保持旧配置结构完整。
    task_timeout_seconds: 300,
    // 旧版遗留字段：同上，不再参与任何超时判断。
    search_task_timeout_seconds: 600,
  };
}

/** 新加一个提供商的空白档案。 */
function blankProvider(): ProviderSettings {
  return {
    provider: "",
    display_name: "",
    base_url: "",
    api: "openai-completions",
    api_key_ref: "",
    models: [{ id: "", name: "", contextWindow: null }],
    reasoning_effort: "default",
  };
}

/** Profile 资产分组（OD-ARCH-109）：配置类在前，Skill / 插件在后，和落盘位置无关。 */
const ASSET_GROUPS: Array<{ title: string; kinds: string[] }> = [
  { title: "配置与场景", kinds: ["config", "patch", "decl", "rules"] },
  { title: "Skill", kinds: ["skill"] },
  { title: "插件", kinds: ["plugin"] },
];

/** 资产状态 → 徽标文案与色义。色义沿用软件既有语义，只在小面积色块上体现。 */
const ASSET_STATUS: Record<
  string,
  { label: string; tone: "ok" | "warn" | "bad" | "neutral" | "info" }
> = {
  ready: { label: "就绪", tone: "ok" },
  unchanged: { label: "已就位", tone: "ok" },
  created: { label: "已补写", tone: "info" },
  updated: { label: "已更新", tone: "info" },
  stale: { label: "待更新", tone: "warn" },
  drift: { label: "已修改未覆盖", tone: "warn" },
  missing: { label: "缺失", tone: "bad" },
  missing_source: { label: "源缺失", tone: "bad" },
  version_mismatch: { label: "版本不符", tone: "warn" },
};

/** 长路径只留末尾两段，完整路径放在悬停说明里。 */
function shortPath(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts.length <= 2 ? path : "…/" + parts.slice(-2).join("/");
}

export default function SettingsPanel({ profileEpoch = 0 }: { profileEpoch?: number }) {
  // —— 分页（OD-ARCH-061 S2）：唯一设置入口，一页一件事 ——
  const [tab, setTab] = useState<SettingsTab>("model");

  // —— 成果目录 ——
  const [info, setInfo] = useState<LocalRootInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // —— 库根（OD-ARCH-086）——
  const [libraryInfo, setLibraryInfo] = useState<LibraryRootInfo | null>(null);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [libraryInput, setLibraryInput] = useState("");
  const [libraryBusy, setLibraryBusy] = useState(false);
  const [libraryMessage, setLibraryMessage] = useState<string | null>(null);
  const [librarySaveError, setLibrarySaveError] = useState<string | null>(null);

  // —— 知识库根（OD-KB-001 P1）：只读，独立于库根 ——
  const [vaultStatus, setVaultStatus] = useState<KnowledgeVaultRootStatus | null>(null);
  const [vaultScan, setVaultScan] = useState<KnowledgeVaultScan | null>(null);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [vaultMessage, setVaultMessage] = useState<string | null>(null);
  const [vaultError, setVaultError] = useState<string | null>(null);
  const vaultSkipped = vaultScan
    ? vaultScan.stats.skipped_hidden +
      vaultScan.stats.skipped_excluded +
      vaultScan.stats.skipped_symlink +
      vaultScan.stats.skipped_non_file +
      vaultScan.stats.skipped_depth
    : 0;
  // WebKit 不把状态行的纯文本节点暴露给无障碍树；扫描摘要同时写进状态组的
  // aria-label，窗口级验收才能读到实测统计，而不是只看截图。
  const vaultAria = `知识库状态：${
    vaultStatus?.configured
      ? `已选择 ${vaultStatus.root ?? "不可用"}；${
          vaultStatus.error ? `校验失败 ${vaultStatus.error}` : "目录可用"
        }`
      : "未设置"
  }；${
    vaultScan
      ? `扫描结果 ${vaultScan.stats.files} 个文件（笔记 ${vaultScan.stats.notes}、附件 ${vaultScan.stats.attachments}），跳过 ${vaultSkipped} 项，错误 ${vaultScan.stats.errors} 项`
      : "尚未扫描"
  }`;

  // —— 端点事实 / 权限 ——
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);
  const [factsError, setFactsError] = useState<string | null>(null);
  const [factsLoading, setFactsLoading] = useState(false);
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [permissionNote, setPermissionNote] = useState<string | null>(null);
  const [blocked, setBlocked] = useState<string | null>(null);

  // —— 模型配置（S1）——
  const [view, setView] = useState<ModelSettingsView | null>(null);
  const [form, setForm] = useState<ModelSettings>(blankSettings);
  /** 当前正在编辑的提供商在 form.providers 里的下标；-1 表示还没选。 */
  const [editing, setEditing] = useState(-1);
  const [apiKey, setApiKey] = useState("");
  const [modelBusy, setModelBusy] = useState(false);
  const [modelNote, setModelNote] = useState<string | null>(null);
  const [modelError, setModelError] = useState<string | null>(null);
  // 连接测试（OD-ARCH-054 S1/S2）：结果只说明这一次请求，不代表已保存。
  const [probe, setProbe] = useState<ModelProbeReceipt | null>(null);
  const [probeBusy, setProbeBusy] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);

  // —— 场景声明（S3）——
  const [decl, setDecl] = useState<ProfileDeclPayload | null>(null);
  const [declText, setDeclText] = useState("");
  const [declBusy, setDeclBusy] = useState(false);
  const [declNote, setDeclNote] = useState<string | null>(null);
  const [declError, setDeclError] = useState<string | null>(null);
  /**
   * 当前是否基础标准（OD-ARCH-123）。
   *
   * 基础标准是干净底座：声明可以看、可以取模板，但保存会被宿主拒绝。
   * 这里提前把按钮禁用并把原因写在旁边，避免用户点下去才吃一个错误。
   */
  const [declStandard, setDeclStandard] = useState<boolean | null>(null);

  // —— Profile 规则资产（OD-ARCH-068 S4）——
  // 平台代码不内置行业规则：这里只是规则文件的编辑面。保存后下一轮提交即生效。
  const [rules, setRules] = useState<ProfileRulesView | null>(null);
  const [rulesText, setRulesText] = useState("");
  const [rulesBusy, setRulesBusy] = useState(false);
  const [rulesNote, setRulesNote] = useState<string | null>(null);
  const [rulesError, setRulesError] = useState<string | null>(null);
  /**
   * 场景 → Skill 绑定的界面草稿（OD-ARCH-129）。
   *
   * 点勾选片只改这里，点「保存绑定」才提交：一次提交走一遍校验与备份，
   * 不会因为连点几下面在磁盘上留下几份备份。
   */
  const [bindingDraft, setBindingDraft] = useState<Record<string, string[]>>({});
  const [bindingBusy, setBindingBusy] = useState(false);
  const [bindingNote, setBindingNote] = useState<string | null>(null);
  const [bindingError, setBindingError] = useState<string | null>(null);
  // Profile 自带 Skill 源（OD-ARCH-131）：填名字 → 生成模板 → 勾选绑定时就能选到它。
  const [skillSourceName, setSkillSourceName] = useState("");
  const [skillSourceBusy, setSkillSourceBusy] = useState(false);

  // —— 能力管理（OD-ARCH-072）：Skill 只读展示 + 插件（bundle）启停 ——
  // 只在打开这一页时读一次；启停写的是隔离实例的 profile，重启后生效。
  const [caps, setCaps] = useState<CapabilityView | null>(null);
  const [capsBusy, setCapsBusy] = useState(false);
  const [capsNote, setCapsNote] = useState<string | null>(null);
  const [capsError, setCapsError] = useState<string | null>(null);

  // —— Profile 资产（OD-ARCH-109）：切换即装配，这里只看结果、按需装插件 ——
  const [assets, setAssets] = useState<ProfileAssetsView | null>(null);
  const [assetsBusy, setAssetsBusy] = useState(false);
  const [assetsError, setAssetsError] = useState<string | null>(null);
  const [assetsNote, setAssetsNote] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installReceipts, setInstallReceipts] = useState<
    Record<string, ProfilePluginInstallReceipt>
  >({});
  // 声明插件（OD-ARCH-128）：自建 Profile 从零配齐的最后一项，装成功才登记。
  const [pluginDraft, setPluginDraft] = useState("");
  const [pluginAdding, setPluginAdding] = useState(false);
  const [pluginAddReceipt, setPluginAddReceipt] =
    useState<ProfilePluginAddReceipt | null>(null);
  // 被拒的理由要贴在这一行里：放到页面顶部的 alert 会被滚动位置藏掉。
  const [pluginAddError, setPluginAddError] = useState<string | null>(null);
  // 就地修复（OD-ARCH-113）：只在用户点按钮时写盘。
  const [repairing, setRepairing] = useState(false);
  const [repairReceipt, setRepairReceipt] =
    useState<ProfileAssetsRepairReceipt | null>(null);
  // 自建 Profile 的登记（用于移除入口）；来自同一个 profile_choice_view。
  const [customProfiles, setCustomProfiles] = useState<ProfileCustomEntry[]>([]);
  const [removingProfile, setRemovingProfile] = useState<string | null>(null);
  const [removeReceipt, setRemoveReceipt] = useState<string | null>(null);
  // 行内改名（OD-ARCH-126）：只改显示名，目录名与绑定不动。
  const [renamingProfile, setRenamingProfile] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [renameBusy, setRenameBusy] = useState(false);
  const [renameReceipt, setRenameReceipt] = useState<string | null>(null);
  // 回收站（OD-ARCH-125）：移除是移入 .trash，这里提供软件内的恢复入口。
  const [trash, setTrash] = useState<ProfileTrashEntry[]>([]);
  const [restoringTrash, setRestoringTrash] = useState<string | null>(null);
  const [restoreReceipt, setRestoreReceipt] = useState<string | null>(null);
  // 迁移（OD-ARCH-127）：导出成单文件包、从包导入。同名冲突时就地改名再导。
  const [exportingProfile, setExportingProfile] = useState<string | null>(null);
  const [bundles, setBundles] = useState<ProfileBundleListing[]>([]);
  const [migrateReceipt, setMigrateReceipt] = useState<string | null>(null);
  const [importBusy, setImportBusy] = useState(false);
  // 冲突时暂存待导入的包路径，用户改名后复用。
  const [importConflict, setImportConflict] = useState<{
    path: string;
    note: string;
  } | null>(null);
  const [importNameDraft, setImportNameDraft] = useState("");
  // 停放区（OD-ARCH-117）：切走时不属于目标 Profile 的 Skill 移到这里，切回自动归位。
  const [parked, setParked] = useState<ParkedSkillView[]>([]);
  // 切换时自动补齐缺失插件（OD-ARCH-121）。默认开启；只有这里和设置文件两处状态。
  const [autoInstall, setAutoInstall] = useState<boolean | null>(null);
  const [autoInstallBusy, setAutoInstallBusy] = useState(false);

  // —— 对话分类管理（OD-ARCH-062 S1）——
  // 每行一份草稿：只有点“保存这一行”才写盘，避免输入过程中反复落盘。
  const [categories, setCategories] = useState<ThreadCategoryView[] | null>(null);
  const [catDrafts, setCatDrafts] = useState<
    Record<string, { label: string; folder: string; vault: string }>
  >({});
  const [catBusy, setCatBusy] = useState(false);
  const [catNote, setCatNote] = useState<string | null>(null);
  const [catError, setCatError] = useState<string | null>(null);
  const [catInit, setCatInit] = useState<CategoryInitReceipt | null>(null);
  const [newCatLabel, setNewCatLabel] = useState("");
  const [newCatFolder, setNewCatFolder] = useState("");

  // —— 定时与提醒 ——
  const [sched, setSched] = useState<SchedulerSnapshot | null>(null);
  const [schedError, setSchedError] = useState<string | null>(null);
  const [schedBusy, setSchedBusy] = useState(false);
  const [schedNote, setSchedNote] = useState<string | null>(null);
  const [pendingKey, setPendingKey] = useState(0);

  // —— 当前运行通道（OD-ARCH-075，只读）——
  // 通道由启动参数 OMNIDESK_ENGINE_CHANNEL 决定，界面不提供切换。
  const [channelView, setChannelView] = useState<ProfileChoiceView | null>(null);
  const [channelError, setChannelError] = useState<string | null>(null);

  const refreshAdmission = useCallback(async () => {
    setFactsLoading(true);
    try {
      const r = await host.admissionStatus();
      if (r.facts) setFacts(r.facts);
      setBlocked(r.blocked ? (r.block_reason ?? "原因未提供") : null);
      setFactsError(
        r.ok ? null : r.error?.message ?? r.error?.engine_error_code ?? "无法读取端点信息",
      );
    } catch (e) {
      setFactsError(String(e));
    } finally {
      setFactsLoading(false);
    }
  }, []);

  const refreshModel = useCallback(async () => {
    try {
      const r = await shell.modelSettingsLoad();
      setView(r);
      if (r.settings) {
        setForm({
          ...r.settings,
          providers: r.settings.providers.map((p) => ({ ...p, reasoning_effort: p.reasoning_effort ?? "default" })),
        });
        setEditing((prev) => (prev >= 0 && prev < r.settings!.providers.length ? prev : 0));
      } else {
        setForm(blankSettings());
        setEditing(-1);
      }
      setModelError(r.ok ? null : r.error ?? "读取模型配置失败");
    } catch (e) {
      setView(null);
      setModelError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const refreshDecl = useCallback(async () => {
    try {
      // 同时读一次 Profile 选择（只读、很轻）：基础标准下保存会被宿主拒绝，
      // 按钮要提前禁用并给出原因（OD-ARCH-123）。
      const [r, choice] = await Promise.all([shell.profileDecl(), shell.profileChoiceView()]);
      setDecl(r);
      // null = 当前 Profile 没有场景声明（基础标准）。这是正常状态，不是读取失败。
      setDeclText(r ? r.text : "");
      setDeclStandard(choice.standard === choice.effective);
      setDeclError(null);
    } catch (e) {
      setDecl(null);
      setDeclStandard(null);
      setDeclError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /**
   * 切换 Profile（OD-ARCH-110）后，各分页缓存的是旧 Profile 的资产与能力清单。
   *
   * 这里只清缓存、不写盘：清掉后用户再打开对应分页会重新读一遍真实的磁盘状态
   * （分类 / 能力 / 资产 / 规则都靠自身分页的「打开才读」effect 兜底）。
   *
   * 场景声明是例外，必须当场重读（OD-ARCH-123 审查追加）：它没有分页级兜底，
   * 而是与页面同生在「对话分类」页渲染。用户停在这一页切换 Profile 时，
   * 只清缓存会让这一页一直显示「当前 Profile 没有场景声明」，
   * 而磁盘上刚被装配写好了 decl.yml——界面于是在说假话。
   */
  useEffect(() => {
    if (profileEpoch === 0) return;
    // 分类表随 Profile 走（OD-ARCH-116）：不清掉的话，切回基础标准后这里
    // 仍显示上一个 Profile 的业务分类，点进分类页也不会重读。
    setCategories(null);
    setCatDrafts({});
    setCatError(null);
    setCaps(null);
    setAssets(null);
    setDecl(null);
    setDeclStandard(null);
    setDeclText("");
    setDeclError(null);
    setDeclNote(null);
    setRules(null);
    setChannelView(null);
    setInstallReceipts({});
    setRepairReceipt(null);
    setParked([]);
    setAssetsError(null);
    setCapsError(null);
    void refreshDecl();
  }, [profileEpoch, refreshDecl]);

  /**
   * 读取规则资产（只读）。
   *
   * 三种状态必须分开显示：已加载 / 文件不存在（纯净底座）/ 文件损坏（提交会被拒绝）。
   */
  const refreshRules = useCallback(async () => {
    try {
      const r = await shell.profileRules();
      setRules(r);
      setRulesText(r.text ?? "");
      setRulesError(r.error);
      setBindingDraft(
        Object.fromEntries(
          Object.entries(r.rules?.skill_bindings ?? {}).map(([key, b]) => [
            key,
            [...(b.skills ?? [])],
          ]),
        ),
      );
      setBindingNote(null);
      setBindingError(null);
      if (!r.text && r.loaded === false && !r.error) {
        setRulesNote("当前没有规则文件（纯净底座：零关键词、零铁律）。可点「恢复默认规则包」安装一套。");
      }
    } catch (e) {
      setRules(null);
      setRulesError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /** 读取能力清单（只读）。失败不清空上一份结果，但必须显式报错。 */
  const refreshCaps = useCallback(async () => {
    try {
      const view = await shell.capabilities();
      setCaps(view);
      setCapsError(null);
    } catch (e) {
      setCapsError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /** 读取 Profile 资产清单（只读巡检，不写盘）。 */
  const refreshAssets = useCallback(async () => {
    setAssetsBusy(true);
    try {
      const [view, choice, parkedList, trashList, bundleList] = await Promise.all([
        shell.profileAssetsView(),
        shell.profileChoiceView(),
        shell.profileAssetsParked(),
        shell.profileTrashList(),
        shell.profileExportList(),
      ]);
      setAssets(view);
      setCustomProfiles(choice.custom ?? []);
      setParked(parkedList);
      setTrash(trashList);
      setBundles(bundleList);
      setAssetsError(null);
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    } finally {
      setAssetsBusy(false);
    }
  }, []);

  /** 移除一个自建 Profile：后端会移入 .trash，当前在用的会被拒绝。 */
  const removeProfile = useCallback(
    async (name: string) => {
      setRemovingProfile(name);
      setRemoveReceipt(null);
      setRestoreReceipt(null);
      try {
        const r = await shell.profileRemove(name);
        setRemoveReceipt(r.note);
        await refreshAssets();
      } catch (e) {
        setAssetsError(e instanceof Error ? e.message : String(e));
      } finally {
        setRemovingProfile(null);
      }
    },
    [refreshAssets],
  );

  /** 开始行内改显示名：输入框里的初值是当前显示名。 */
  const startRename = useCallback((entry: ProfileCustomEntry) => {
    setRenamingProfile(entry.name);
    setRenameDraft(entry.label);
    setRenameReceipt(null);
    setRemoveReceipt(null);
  }, []);

  const cancelRename = useCallback(() => {
    setRenamingProfile(null);
    setRenameDraft("");
  }, []);

  /** 保存显示名：后端只改登记表 label 与声明 display_name，目录名不动。 */
  const saveRename = useCallback(
    async (name: string) => {
      if (renameBusy) return;
      setRenameBusy(true);
      setRenameReceipt(null);
      try {
        const r = await shell.profileRename(name, renameDraft);
        setRenameReceipt(r.note);
        setRenamingProfile(null);
        setRenameDraft("");
        await refreshAssets();
      } catch (e) {
        setAssetsError(e instanceof Error ? e.message : String(e));
      } finally {
        setRenameBusy(false);
      }
    },
    [refreshAssets, renameBusy, renameDraft],
  );

  /** 从回收站恢复一个 Profile：后端做两次改名加登记写回，同名已存在会拒绝。 */
  const restoreFromTrash = useCallback(
    async (id: string) => {
      setRestoringTrash(id);
      setRestoreReceipt(null);
      setRemoveReceipt(null);
      try {
        const r = await shell.profileTrashRestore(id);
        setRestoreReceipt(r.note);
        await refreshAssets();
      } catch (e) {
        setAssetsError(e instanceof Error ? e.message : String(e));
      } finally {
        setRestoringTrash(null);
      }
    },
    [refreshAssets],
  );

  /** 导出成单文件迁移包：只含配置与声明，不含插件本体。 */
  const exportProfile = useCallback(async (entry: ProfileCustomEntry) => {
    setExportingProfile(entry.name);
    setMigrateReceipt(null);
    try {
      const r = await shell.profileExport(entry.name, null);
      setMigrateReceipt(
        "已导出「" +
          r.label +
          "」共 " +
          r.files.length +
          " 个配置文件：" +
          r.path +
          "。可在下面这个区块里换台机器导入。",
      );
      await refreshAssets();
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    } finally {
      setExportingProfile(null);
    }
  }, [refreshAssets]);

  /** 选一个迁移包导入；同名冲突时退回改名字段，让用户换一个标识。 */
  const importProfile = useCallback(
    async (path: string, renameTo: string | null) => {
      setImportBusy(true);
      setMigrateReceipt(null);
      try {
        const r = await shell.profileImport(path, renameTo, null);
        setImportConflict(null);
        setImportNameDraft("");
        setMigrateReceipt(
          "已导入「" +
            r.profile.label +
            "」。" +
            (r.plugins.length
              ? "这套工作台声明了 " + r.plugins.length + " 个插件，切过去时缺的会自动补装。"
              : "这套工作台没有声明插件。"),
        );
        await refreshAssets();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        // 同名 / 目录已存在：给出就地改名入口，而不是让用户去改包。
        if (/已经在自定义 Profile 清单里|已经有同名目录|是出厂 Profile 名/.test(msg)) {
          const taken = /「([^」]+)」/.exec(msg)?.[1] ?? "";
          setImportConflict({ path, note: msg });
          setImportNameDraft(taken ? taken + "-2" : "");
        } else {
          setAssetsError(msg);
        }
      } finally {
        setImportBusy(false);
      }
    },
    [refreshAssets],
  );

  const pickImportFile = useCallback(async () => {
    setMigrateReceipt(null);
    try {
      const picked = await openDirectoryDialog({
        directory: false,
        multiple: false,
        filters: [{ name: "OmniDesk Profile 包", extensions: ["json"] }],
        title: "选择要导入的 Profile 包",
      });
      if (typeof picked !== "string" || picked.length === 0) {
        return;
      }
      await importProfile(picked, null);
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    }
  }, [importProfile]);

  /** 安装一个缺失插件：走 DSH 官方插件通道，结果如实展示。 */
  const installPlugin = useCallback(
    async (profile: string, pkg: string) => {
      setInstalling(pkg);
      setAssetsNote(null);
      try {
        const receipt = await shell.profilePluginInstall(profile, pkg);
        setInstallReceipts((prev) => ({ ...prev, [pkg]: receipt }));
        await refreshAssets();
      } catch (e) {
        setAssetsError(e instanceof Error ? e.message : String(e));
      } finally {
        setInstalling(null);
      }
    },
    [refreshAssets],
  );

  /** 就地修复资产：补齐缺失 / 恢复被改过的文件（会写盘，先备份）。 */
  const repairAssets = useCallback(async () => {
    if (!assets) return;
    setRepairing(true);
    setAssetsNote(null);
    setAssetsError(null);
    try {
      const receipt = await shell.profileAssetsRepair(assets.profile);
      setRepairReceipt(receipt);
      setAssets(receipt.report);
      setAssetsNote(receipt.note);
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    } finally {
      setRepairing(false);
    }
  }, [assets]);

  /**
   * 缺的插件挨个装（会联网）：一次点击跑完，逐个回执，失败不中断其余。
   *
   * 开启「切换时自动补齐」后，这是装失败后的手动重试入口，不是唯一路径。
   */
  const installMissingPlugins = useCallback(async () => {
    if (!assets) return;
    setInstalling("batch");
    setAssetsError(null);
    try {
      // 一次调用补齐（OD-ARCH-114）：后端只装缺的，单个失败不中断其余。
      const r = await shell.profilePluginsEnsure(assets.profile);
      const next: Record<string, ProfilePluginInstallReceipt> = {};
      for (const receipt of r.results) next[receipt.package] = receipt;
      setInstallReceipts((prev) => ({ ...prev, ...next }));
      if (r.attempted.length === 0) {
        setAssetsNote("没有缺失的插件，无需安装。");
      } else if (r.ok) {
        setAssetsNote(`已装齐 ${r.attempted.length} 个插件，回执见每一项。`);
      } else {
        setAssetsNote(
          `已尝试 ${r.attempted.length} 个，仍有 ${r.still_missing.length} 个没就绪，回执见每一项。`,
        );
      }
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    }
    setInstalling(null);
    await refreshAssets();
  }, [assets, refreshAssets]);

  /** 本地没装到的插件数：只在有资产清单且非基础标准时才有意义。 */
  const missingPluginCount =
    assets && !assets.standard
      ? assets.items.filter((i) => i.kind === "plugin" && i.status !== "ready").length
      : 0;

  /** 当前资产页看的是不是「自建 Profile」：只有这类才允许声明插件。 */
  const isCustomAssetProfile = Boolean(
    assets &&
      !assets.standard &&
      customProfiles.some((p) => p.name === assets.profile),
  );

  /**
   * 给自建 Profile 声明一个插件（OD-ARCH-128）。
   *
   * 宿主先探测：本地已就绪就只补登记；缺件才联网装，装成功才写登记表，
   * 失败一律不写。这里只负责把回执如实摆出来，并刷新资产清单。
   */
  const addPlugin = useCallback(async () => {
    if (!assets) return;
    const pkg = pluginDraft.trim();
    if (!pkg) return;
    setPluginAdding(true);
    setPluginAddReceipt(null);
    setPluginAddError(null);
    try {
      const receipt = await shell.profilePluginsAdd(assets.profile, pkg);
      setPluginAddReceipt(receipt);
      if (receipt.ok) setPluginDraft("");
    } catch (e) {
      setPluginAddError(e instanceof Error ? e.message : String(e));
    } finally {
      setPluginAdding(false);
    }
    await refreshAssets();
  }, [assets, pluginDraft, refreshAssets]);

  /** 读自动补齐开关：只读，不改设置文件。 */
  const refreshAutoInstall = useCallback(async () => {
    try {
      const prefs = await shell.profileAssemblyPrefs();
      setAutoInstall(prefs.auto_install_plugins);
    } catch {
      // 读不到就按宿主默认（开启）显示，不拦住页面。
      setAutoInstall(true);
    }
  }, []);

  /** 切换开关：写入后以宿主回执为准，不先乐观改界面。 */
  const changeAutoInstall = useCallback(async (enabled: boolean) => {
    setAutoInstallBusy(true);
    try {
      const prefs = await shell.profileAssemblyPrefsSet(enabled);
      setAutoInstall(prefs.auto_install_plugins);
      setAssetsNote(
        prefs.auto_install_plugins
          ? "已开启：切换 Profile 缺插件时会在后台自动联网补齐。"
          : "已关闭：切换 Profile 只提示缺哪些插件，由你点按钮安装。",
      );
    } catch (e) {
      setAssetsError(e instanceof Error ? e.message : String(e));
    } finally {
      setAutoInstallBusy(false);
    }
  }, []);

  // 打开资产页时读一次偏好；顺手订阅后台补装结果，装完刷新清单。
  useEffect(() => {
    if (tab !== "assets") return;
    if (autoInstall === null) void refreshAutoInstall();
    let alive = true;
    let unlisten: (() => void) | null = null;
    onProfilePlugins((ev) => {
      if (!alive || !ev.automatic) return;
      if (ev.error) setAssetsNote("后台自动补齐未执行：" + ev.error);
      else if (ev.ok && ev.still_missing.length === 0) {
        setAssetsNote(
          ev.attempted.length > 0
            ? `后台已补齐 ${ev.attempted.length} 个插件。`
            : "插件已就绪。",
        );
      } else {
        setAssetsNote(
          `后台已尝试 ${ev.attempted.length} 个，仍有 ${ev.still_missing.length} 个未就绪。`,
        );
      }
      void refreshAssets();
    })
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // 订阅失败时手动安装入口仍可用。
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [tab, autoInstall, refreshAutoInstall, refreshAssets]);

  const togglePlugin = useCallback(
    async (name: string, enabled: boolean) => {
      setCapsBusy(true);
      setCapsNote(null);
      setCapsError(null);
      try {
        const receipt = await shell.capabilitySetPlugin({ name, enabled });
        setCapsNote(
          receipt.note +
            (receipt.backup ? `旧文件已备份为 ${receipt.backup.split("/").pop()}。` : ""),
        );
        await refreshCaps();
      } catch (e) {
        setCapsError(e instanceof Error ? e.message : String(e));
      } finally {
        setCapsBusy(false);
      }
    },
    [refreshCaps],
  );

  const openSkillsDir = useCallback(async () => {
    setCapsError(null);
    try {
      const r = await shell.capabilityOpenSkills();
      setCapsNote(r.note);
    } catch (e) {
      setCapsError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const saveRules = useCallback(async () => {
    setRulesBusy(true);
    setRulesNote(null);
    setRulesError(null);
    try {
      const r = await shell.profileRulesSave(rulesText);
      setRulesNote(
        `规则已保存（下一轮提交生效，无需重启）。` +
          (r.backup ? `旧文件已备份为 ${r.backup.split("/").pop()}。` : ""),
      );
      await refreshRules();
    } catch (e) {
      setRulesError(e instanceof Error ? e.message : String(e));
    } finally {
      setRulesBusy(false);
    }
  }, [refreshRules, rulesText]);

  const restoreRules = useCallback(async () => {
    setRulesBusy(true);
    setRulesNote(null);
    setRulesError(null);
    try {
      const r = await shell.profileRulesRestoreDefault();
      setRulesNote(
        `已写入出厂默认规则包。` +
          (r.backup ? `原文件已备份为 ${r.backup.split("/").pop()}。` : ""),
      );
      await refreshRules();
    } catch (e) {
      setRulesError(e instanceof Error ? e.message : String(e));
    } finally {
      setRulesBusy(false);
    }
  }, [refreshRules]);

  /** 勾选 / 取消一个 Skill（OD-ARCH-129）；每场景最多 3 个，与宿主校验同口径。 */
  const toggleBinding = useCallback((scene: string, skill: string) => {
    setBindingNote(null);
    setBindingError(null);
    setBindingDraft((prev) => {
      const current = prev[scene] ?? [];
      const has = current.includes(skill);
      if (!has && current.length >= MAX_BINDINGS_PER_SCENE) {
        setBindingError(
          `「${sceneLabel(scene)}」最多绑定 ${MAX_BINDINGS_PER_SCENE} 个 Skill；先取消一个再选。`,
        );
        return prev;
      }
      const next = has ? current.filter((s) => s !== skill) : [...current, skill];
      return { ...prev, [scene]: next };
    });
  }, []);

  /**
   * 提交整张绑定表（OD-ARCH-129）。
   *
   * 只提交界面上真配过的场景：没出现在草稿里的场景保持宿主原样，
   * 避免「打开设置页就顺手清空了别人写的绑定」。
   */
  const saveBindings = useCallback(async () => {
    setBindingBusy(true);
    setBindingNote(null);
    setBindingError(null);
    try {
      // 只提交「文件里已有的」与「这次点过的」：宿主整表替换，若把五个固定场景
      // 无条件写进去，新 Profile 会凭空多出五行空绑定；反过来漏掉自定义场景键，
      // 又会把用户手写的绑定一起冲掉。
      const payload: { scene: string; skills: string[] }[] = BINDING_SCENES.filter(
        (scene) => scene in bindingDraft,
      ).map((scene) => ({
        scene,
        skills: (bindingDraft[scene] ?? []).filter((s) => Boolean(s)),
      }));
      const extras = Object.keys(bindingDraft).filter(
        (k) => !BINDING_SCENES.includes(k as BindingScene),
      );
      for (const scene of extras) {
        payload.push({ scene, skills: (bindingDraft[scene] ?? []).filter((s) => Boolean(s)) });
      }
      const r = await shell.profileSkillBindingsSave(payload);
      // OD-ARCH-130：保存成功后运行时 skills/ 已按新绑定收敛，回执直接说清补了哪几个，
      // 不让用户以为「还得切一次 Profile 才生效」。
      const synced = r.skills;
      const detail = synced?.note ?? "";
      const failed = synced?.failed?.length
        ? ` 有 ${synced.failed.length} 项没能写入：${synced.failed.join("；")}`
        : "";
      setBindingNote(
        `绑定已保存，下一轮提交生效。` +
          (detail ? ` ${detail}` : "") +
          failed +
          (r.backup ? ` 旧文件已备份为 ${r.backup.split("/").pop()}。` : ""),
      );
      await refreshRules();
    } catch (e) {
      setBindingError(e instanceof Error ? e.message : String(e));
    } finally {
      setBindingBusy(false);
    }
  }, [bindingDraft, refreshRules]);

  /** 打开该 Profile 自带的 Skill 目录（OD-ARCH-131；首次点才建）。 */
  const openSkillSource = useCallback(async () => {
    setSkillSourceBusy(true);
    setBindingNote(null);
    setBindingError(null);
    try {
      const r = await shell.profileSkillSourceOpen();
      setBindingNote(r.note);
    } catch (e) {
      setBindingError(e instanceof Error ? e.message : String(e));
    } finally {
      setSkillSourceBusy(false);
    }
  }, []);

  /** 按名字生成一份自带 Skill 的模板（已存在不覆盖）。 */
  const scaffoldSkill = useCallback(async () => {
    const name = skillSourceName.trim().toLowerCase();
    if (!name) {
      setBindingError("先填一个 Skill 名，例如 office-note。");
      return;
    }
    setSkillSourceBusy(true);
    setBindingNote(null);
    setBindingError(null);
    try {
      const r = await shell.profileSkillScaffold(name);
      setBindingNote(r.note);
      setSkillSourceName("");
      await refreshRules();
    } catch (e) {
      setBindingError(e instanceof Error ? e.message : String(e));
    } finally {
      setSkillSourceBusy(false);
    }
  }, [refreshRules, skillSourceName]);

  const refreshCategories = useCallback(async () => {
    setCatBusy(true);
    try {
      const r = await shell.categorySettingsLoad();
      setCategories(r.categories);
      setCatDrafts(
        Object.fromEntries(
          r.categories.map((c) => [
            c.key,
            { label: c.label, folder: c.folder ?? "", vault: c.vault ?? "" },
          ]),
        ),
      );
      setCatError(r.ok ? null : "宿主返回的分类读取状态不是 ok");
      await refreshThreadCategories();
    } catch (e) {
      // 读不到就保留上一次的列表并明确报错（风险③：不静默降级）。
      setCatError(e instanceof Error ? e.message : String(e));
    } finally {
      setCatBusy(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const r = await shell.localRootInfo();
      setInfo(r);
      setInfoError(r.resolved ? null : r.error);
    } catch (e) {
      setInfo(null);
      setInfoError(String(e));
    }
  }, []);

  const refreshLibrary = useCallback(async () => {
    try {
      const r = await shell.libraryRootInfo();
      setLibraryInfo(r);
      setLibraryError(r.resolved ? null : r.error);
    } catch (e) {
      setLibraryInfo(null);
      setLibraryError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const refreshVault = useCallback(async () => {
    try {
      const r = await shell.knowledgeVaultStatus();
      setVaultStatus(r);
      setVaultError(r.error);
      return r;
    } catch (e) {
      setVaultStatus(null);
      setVaultError(e instanceof Error ? e.message : String(e));
      return null;
    }
  }, []);

  const scanVault = useCallback(async () => {
    setVaultBusy(true);
    setVaultError(null);
    setVaultMessage(null);
    try {
      const r = await shell.knowledgeVaultScan();
      setVaultScan(r);
      setVaultMessage(
        `扫描完成：${r.stats.files} 个文件（笔记 ${r.stats.notes}、附件 ${r.stats.attachments}），` +
          `跳过 ${r.stats.skipped_hidden + r.stats.skipped_excluded + r.stats.skipped_symlink + r.stats.skipped_non_file + r.stats.skipped_depth} 项，` +
          `错误 ${r.stats.errors} 项。` +
          (r.stats.truncated ? " 已达到扫描上限，结果不完整。" : ""),
      );
    } catch (e) {
      setVaultScan(null);
      setVaultError(e instanceof Error ? e.message : String(e));
    } finally {
      setVaultBusy(false);
    }
  }, []);

  /** 用系统目录选择框挑一个已有目录；选完立即保存，再自动扫一次。 */
  const pickVaultDirectory = useCallback(async () => {
    setVaultBusy(true);
    setVaultError(null);
    setVaultMessage(null);
    try {
      const picked = await openDirectoryDialog({
        directory: true,
        multiple: false,
        title: "选择知识库目录（只读）",
      });
      if (typeof picked !== "string" || picked.length === 0) {
        return;
      }
      const r = await shell.setKnowledgeVaultRoot(picked);
      setVaultMessage(r.note + " 当前知识库：" + r.knowledge_vault_root);
      await refreshVault();
      const scan = await shell.knowledgeVaultScan();
      setVaultScan(scan);
    } catch (e) {
      setVaultError(e instanceof Error ? e.message : String(e));
    } finally {
      setVaultBusy(false);
    }
  }, [refreshVault]);

  const clearVault = useCallback(async () => {
    setVaultBusy(true);
    setVaultError(null);
    setVaultMessage(null);
    try {
      await shell.clearKnowledgeVaultRoot();
      setVaultScan(null);
      setVaultMessage("已取消知识库选择；知识库内文件没有改动。");
      await refreshVault();
    } catch (e) {
      setVaultError(e instanceof Error ? e.message : String(e));
    } finally {
      setVaultBusy(false);
    }
  }, [refreshVault]);

  const refreshScheduler = useCallback(async () => {
    try {
      const r = await schedulerApi.getConfig();
      setSched(r);
      setSchedError(null);
    } catch (e) {
      setSched(null);
      setSchedError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /** 保存一行分类。只传这一行改过的字段；失败时把宿主原因原样显示。 */
  const saveCategory = useCallback(
    async (key: string) => {
      const draft = catDrafts[key];
      if (!draft) return;
      setCatBusy(true);
      setCatNote(null);
      setCatError(null);
      try {
        await shell.categoryUpdate({
          key,
          label: draft.label,
          folder: draft.folder,
          vault: draft.vault,
        });
        await refreshCategories();
        setCatNote(`已保存分类「${draft.label.trim()}」。`);
      } catch (e) {
        setCatError(e instanceof Error ? e.message : String(e));
      } finally {
        setCatBusy(false);
      }
    },
    [catDrafts, refreshCategories],
  );

  const addCategory = useCallback(async () => {
    const label = newCatLabel.trim();
    if (!label) {
      setCatError("先给新分类起个名字。");
      return;
    }
    setCatBusy(true);
    setCatNote(null);
    setCatError(null);
    try {
      const r = await shell.categoryAdd({
        label,
        ...(newCatFolder.trim() ? { folder: newCatFolder.trim() } : {}),
      });
      setNewCatLabel("");
      setNewCatFolder("");
      await refreshCategories();
      setCatNote(`已新增分类「${label}」（${r.key}）。它只出现在归类下拉里，不会加 Dock 入口。`);
    } catch (e) {
      setCatError(e instanceof Error ? e.message : String(e));
    } finally {
      setCatBusy(false);
    }
  }, [newCatFolder, newCatLabel, refreshCategories]);

  const removeCategory = useCallback(
    async (key: string, label: string) => {
      setCatBusy(true);
      setCatNote(null);
      setCatError(null);
      try {
        await shell.categoryRemove(key);
        await refreshCategories();
        setCatNote(`已删除分类「${label}」。已用该分类的对话标签保持原样，没有迁移或删除。`);
      } catch (e) {
        setCatError(e instanceof Error ? e.message : String(e));
      } finally {
        setCatBusy(false);
      }
    },
    [refreshCategories],
  );

  const initCategoryFolders = useCallback(async () => {
    setCatBusy(true);
    setCatNote(null);
    setCatError(null);
    try {
      const r = await shell.categoryInitFolders();
      setCatInit(r);
      setCatNote(
        r.created > 0
          ? `已创建 ${r.created} 个缺失目录；其他目录与文件没有改动。`
          : "所有分类目录都已存在，没有创建或改动任何文件。",
      );
    } catch (e) {
      setCatInit(null);
      setCatError(e instanceof Error ? e.message : String(e));
    } finally {
      setCatBusy(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    void refreshLibrary();
    void refreshAdmission();
    void refreshScheduler();
    void refreshModel();
    void refreshDecl();
  }, [refresh, refreshLibrary, refreshAdmission, refreshScheduler, refreshModel, refreshDecl]);

  // 知识库根只在打开成果页时读一次状态；扫描必须由用户手动触发。
  useEffect(() => {
    if (tab === "library") void refreshVault();
  }, [tab, refreshVault]);

  // 对话分类只在打开这一页时读一次，避免与其他分页的读盘互相干扰。
  useEffect(() => {
    if (tab === "categories" && categories === null) void refreshCategories();
  }, [tab, categories, refreshCategories]);

  // OD-ARCH-075：通道只读事实，打开引擎诊断页时读一次。
  // 读取失败必须显示失败原因，不能默认成 web-api 冒充事实。
  const loadChannel = useCallback(() => {
    setChannelError(null);
    shell
      .profileChoiceView()
      .then((r) => {
        setChannelView(r);
        setChannelError(r.warning ?? null);
      })
      .catch((e) => {
        setChannelView(null);
        setChannelError(e instanceof Error ? e.message : String(e));
      });
  }, []);

  useEffect(() => {
    if (tab === "engine" && channelView === null && channelError === null) loadChannel();
  }, [tab, channelView, channelError, loadChannel]);

  // 规则资产同样只在打开该页时读取一次。
  useEffect(() => {
    if (tab === "rules" && rules === null) void refreshRules();
  }, [tab, rules, refreshRules]);

  // 能力管理同理：打开这一页才读，避免启动时多读一次磁盘。
  useEffect(() => {
    if (tab === "capabilities" && caps === null) void refreshCaps();
  }, [tab, caps, refreshCaps]);

  // Profile 资产同理：只读巡检，打开这一页才读。
  useEffect(() => {
    if (tab === "assets" && assets === null) void refreshAssets();
  }, [tab, assets, refreshAssets]);

  const saveModel = useCallback(async () => {
    setModelBusy(true);
    setModelNote(null);
    setModelError(null);
    try {
      const target = form.providers[editing];
      // 默认路由必须落在某个真实存在的提供商的真实模型上。新建/改名时用户常
      // 忘记点「设为默认」，这里在保存前兜底：先看当前编辑项，再按列表顺序找
      // 第一个有模型 id 的提供商。找不到就交给宿主校验并报错。
      const defaultValid =
        !!form.default.provider &&
        form.providers.some(
          (p) => p.provider === form.default.provider && p.models.some((m) => m.id === form.default.model),
        );
      const pickDefault = () => {
        const order = target ? [target, ...form.providers.filter((p) => p !== target)] : form.providers;
        for (const p of order) {
          const first = p.models.find((m) => m.id.trim().length > 0);
          if (p.provider.trim() && first) return { provider: p.provider, model: first.id };
        }
        return form.default;
      };
      const payload = defaultValid ? form : { ...form, default: pickDefault() };
      if (!defaultValid) setForm(payload);
      const r = await shell.modelSettingsSave({
        settings: payload,
        // 密钥只写进当前编辑的提供商；引用名由宿主再校验一次。
        apiKeyRef: target ? target.api_key_ref : null,
        apiKey: apiKey.trim().length > 0 ? apiKey : null,
      });
      // 密钥只在这一次请求里出现过：无论成败都立刻从界面状态里清掉。
      setApiKey("");
      setModelNote(
        (defaultValid ? "" : `已将默认路由设为 ${payload.default.provider} / ${payload.default.model}。`) +
          r.note +
          (r.key_written ? " 密钥已登记。" : " 本次未写入密钥。") +
          (r.key_error ? " 密钥写入失败：" + r.key_error : "") +
          (r.engine_stopped ? "" : " 注意：旧引擎未能确认停止，下次任务可能仍用旧路由。"),
      );
      invalidateEngineFactsCache();
      await refreshModel();
      await refreshAdmission();
    } catch (e) {
      setApiKey("");
      setModelError(e instanceof Error ? e.message : String(e));
    } finally {
      setModelBusy(false);
    }
  }, [form, editing, apiKey, refreshModel, refreshAdmission]);

  const probeConnection = useCallback(async () => {
    const target = form.providers[editing];
    if (!target) {
      setProbe(null);
      setProbeError("先选一个提供商，再测试它的端点。");
      return;
    }
    setProbeBusy(true);
    setProbe(null);
    setProbeError(null);
    try {
      const r = await shell.modelSettingsProbe({
        // 传 provider：端点与已保存路由一致时，DSH 会用该路由已登记的凭据。
        provider: target.provider.trim() || null,
        baseUrl: target.base_url,
        api: target.api,
        // 输入框里的 key 只用于这一次探测；空则沿用 DSH 已登记的凭据。
        apiKey: apiKey.trim().length > 0 ? apiKey : null,
      });
      setProbe(r);
    } catch (e) {
      setProbeError(e instanceof Error ? e.message : String(e));
    } finally {
      setProbeBusy(false);
    }
  }, [form.providers, editing, apiKey]);

  // —— 多提供商编辑（OD-ARCH-065 S4）——
  const current: ProviderSettings | null = form.providers[editing] ?? null;
  const keyOk = useCallback(
    (ref: string) => (ref ? view?.key_status?.[ref]?.configured === true : false),
    [view],
  );

  const updateCurrent = useCallback(
    (patch: Partial<ProviderSettings>) => {
      setForm((prev) => ({
        ...prev,
        providers: prev.providers.map((p, i) => (i === editing ? { ...p, ...patch } : p)),
      }));
    },
    [editing],
  );

  const updateModel = useCallback(
    (index: number, patch: Partial<ModelEntry>) => {
      setForm((prev) => ({
        ...prev,
        providers: prev.providers.map((p, i) =>
          i === editing
            ? { ...p, models: p.models.map((m, j) => (j === index ? { ...m, ...patch } : m)) }
            : p,
        ),
      }));
    },
    [editing],
  );

  const addModel = useCallback(() => {
    setForm((prev) => ({
      ...prev,
      providers: prev.providers.map((p, i) =>
        i === editing ? { ...p, models: [...p.models, { id: "", name: "", contextWindow: null }] } : p,
      ),
    }));
  }, [editing]);

  const removeModel = useCallback(
    (index: number) => {
      setForm((prev) => ({
        ...prev,
        providers: prev.providers.map((p, i) =>
          i === editing && p.models.length > 1
            ? { ...p, models: p.models.filter((_, j) => j !== index) }
            : p,
        ),
      }));
    },
    [editing],
  );

  const addProvider = useCallback(() => {
    setApiKey("");
    setProbe(null);
    setProbeError(null);
    // 选中态跟着新条目走；默认路由由保存时兜底，这里不预先写入空标识。
    setForm((prev) => ({ ...prev, providers: [...prev.providers, blankProvider()] }));
    setEditing(form.providers.length);
  }, [form.providers.length]);

  const removeProvider = useCallback(() => {
    if (!current) return;
    const removingDefault = form.default.provider === current.provider;
    const fallback = form.providers.filter((_, i) => i !== editing)[0];
    setApiKey("");
    setProbe(null);
    setForm((prev) => ({
      ...prev,
      providers: prev.providers.filter((_, i) => i !== editing),
      default: removingDefault
        ? fallback
          ? { provider: fallback.provider, model: fallback.models[0]?.id ?? "" }
          : { provider: "", model: "" }
        : prev.default,
    }));
    setEditing(editing > 0 ? editing - 1 : form.providers.length > 1 ? 0 : -1);
  }, [current, form.default, form.providers, editing]);

  const applyProbeModels = useCallback(() => {
    if (!probe || !current) return;
    const existing = new Map(current.models.filter((m) => m.id).map((m) => [m.id, m]));
    updateCurrent({
      models: probe.models.map((m) => {
        const old = existing.get(m.id);
        return { id: m.id, name: old?.name || m.name || m.id, contextWindow: old?.contextWindow ?? null };
      }),
    });
  }, [probe, current, updateCurrent]);

  const openDecl = useCallback(async () => {
    setDeclBusy(true);
    setDeclNote(null);
    setDeclError(null);
    try {
      const r = await shell.profileDeclOpen();
      setDeclNote(r.note + " " + r.opened);
    } catch (e) {
      setDeclError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeclBusy(false);
    }
  }, []);

  /** 把起步模板灌进编辑框（OD-ARCH-122）：只改内存，仍需用户点「校验并保存」。 */
  const fillStarterDecl = useCallback(async () => {
    setDeclBusy(true);
    setDeclNote(null);
    setDeclError(null);
    try {
      const r = await shell.profileDeclStarter(decl?.decl.display_name ?? undefined);
      setDeclText(r.text);
      setDeclNote(r.note);
    } catch (e) {
      setDeclError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeclBusy(false);
    }
  }, [decl]);

  const saveDecl = useCallback(async () => {
    setDeclBusy(true);
    setDeclNote(null);
    setDeclError(null);
    try {
      const r = await shell.profileDeclSave(declText);
      setDeclNote(
        r.note +
          " 指纹：" +
          r.digest.slice(0, 12) +
          (r.backup ? "（旧文件已备份为 " + r.backup.split("/").pop() + "）" : ""),
      );
      await refreshDecl();
    } catch (e) {
      // 校验失败时宿主不写文件：编辑框内容原样保留，便于用户修正。
      setDeclError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeclBusy(false);
    }
  }, [declText, refreshDecl]);

  const changePermission = useCallback(
    async (category: keyof ModelPermissions, enabled: boolean) => {
      setPermissionBusy(true);
      setPermissionNote(null);
      try {
        const r = await host.setModelPermission(category, enabled);
        setPermissionNote(
          (enabled ? "已开启" : "已关闭") + CATEGORY_LABEL[category] + "权限。" + r.note,
        );
      } catch (e) {
        setPermissionNote(
          "修改未生效：" + String(e) + "（宿主已拒绝新的模型提交，请用上方恢复动作处理。）",
        );
      } finally {
        await refreshAdmission();
        setPermissionBusy(false);
      }
    },
    [refreshAdmission],
  );

  const revokeAll = useCallback(async () => {
    setPermissionBusy(true);
    setPermissionNote(null);
    try {
      const r = await host.revokeAllModelPermissions();
      setPermissionNote(r.note);
    } catch (e) {
      setPermissionNote(
        "撤销未生效：" + String(e) + "（宿主已拒绝新的模型提交，请用上方恢复动作处理。）",
      );
    } finally {
      await refreshAdmission();
      setPermissionBusy(false);
    }
  }, [refreshAdmission]);

  const stopEngine = useCallback(async () => {
    setPermissionBusy(true);
    try {
      const r = await host.shutdownEngine();
      setPermissionNote("已停止引擎：" + r.note + " 屏障已解除，可重新尝试权限变更。");
    } catch (e) {
      setPermissionNote("停止引擎失败：" + String(e));
    } finally {
      await refreshAdmission();
      setPermissionBusy(false);
    }
  }, [refreshAdmission]);

  const saveArtifactRoot = useCallback(async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const r = await shell.setArtifactRoot(input);
      setMessage(r.note + " 当前目录：" + r.artifact_root);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [input, refresh]);

  const saveLibraryRoot = useCallback(async () => {
    setLibraryBusy(true);
    setLibrarySaveError(null);
    setLibraryMessage(null);
    try {
      const r = await shell.setLibraryRoot(libraryInput);
      setLibraryMessage(r.note + " 当前库根：" + r.library_root);
      setLibraryInput("");
      await refreshLibrary();
    } catch (e) {
      setLibrarySaveError(e instanceof Error ? e.message : String(e));
    } finally {
      setLibraryBusy(false);
    }
  }, [libraryInput, refreshLibrary]);

  const openArtifacts = useCallback(async () => {
    setError(null);
    setMessage(null);
    try {
      const r = await shell.openArtifactRoot();
      setMessage("已在系统文件管理器中打开：" + r.opened);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const patchScheduler = useCallback(
    async (patch: Partial<SchedulerConfig>) => {
      if (!sched) return;
      setSchedBusy(true);
      setSchedNote(null);
      try {
        const next: SchedulerConfig = { ...sched.config, ...patch };
        const r = await schedulerApi.setConfig(next);
        setSched(r);
        setSchedError(null);
        setSchedNote("已保存，下一次到点判定起生效。");
      } catch (e) {
        setSchedNote("修改未生效：" + (e instanceof Error ? e.message : String(e)));
        await refreshScheduler();
      } finally {
        setSchedBusy(false);
      }
    },
    [sched, refreshScheduler],
  );

  const cfg = sched?.config ?? null;
  const heartbeatStatus = cfg
    ? cfg.heartbeat_enabled
      ? "每 " + String(cfg.heartbeat_minutes) + " 分钟"
      : "关闭"
    : null;
  /** 当前提供商对应的凭据状态（只含 configured/source/writable）。 */
  const keyStatus = current ? view?.key_status?.[current.api_key_ref] ?? null : null;
  const savedOverlay = view?.overlay_present === true;
  const readyCount = assets
    ? assets.items.filter(
        (i) => i.status === "ready" || i.status === "unchanged",
      ).length
    : 0;

  return (
    <div className="settings-v2 settings-surface">
      <section className="pane settings-pane-header">
        <h2 className="pane-title">
          设置
          <span className="count">本地</span>
        </h2>
        <p className="hint">
          以下设置只作用于本机的 OmniDesk 实例。模型端点与密钥就在这里填写，不需要打开 DSH。
          成果文件不会因为改设置而移动、覆盖或删除；应用配置与成果目录分开存放。
        </p>
        <div className="view-tabs" role="tablist" aria-label="设置分页" data-testid="settings-tabs">
          {SETTINGS_TABS.map((t) => (
            <button
              key={t.key}
              role="tab"
              aria-selected={tab === t.key}
              data-testid={`settings-tab-${t.key}`}
              className={"tab" + (tab === t.key ? " active" : "")}
              onClick={() => setTab(t.key)}
            >
              {t.label}
            </button>
          ))}
        </div>
      </section>

      {/* ============ 1. 模型与端点 ============ */}
      <section className="pane" hidden={tab !== "model"}>
        <CardHead
          title="模型与端点"
          status={
            facts
              ? facts.provider +
                " / " +
                facts.model +
                " · " +
                (CATEGORY_LABEL[facts.category] ?? facts.category)
              : factsLoading
                ? "正在读取端点信息…"
                : factsError
                  ? "端点信息读取失败"
                  : "尚未配置端点"
          }
        >
          <button
            className="ghost"
            onClick={() => void refreshAdmission()}
            disabled={permissionBusy || factsLoading}
          >
            刷新端点信息
          </button>
        </CardHead>

        {!facts && !factsError && !view?.settings ? (
          <div className="alert">
            还没有配置模型端点。在下面填入服务商地址、模型名与密钥，保存后即可使用。
          </div>
        ) : null}

        {modelError ? <div className="alert">模型配置：{modelError}</div> : null}
        {view && !view.ok ? (
          <div className="alert">
            配置文件读取失败，下面显示的是无法加载的状态：{view.error}
            <br />
            原文件未被修改（{view.config_path}）。重新填写并保存才会覆盖它。
          </div>
        ) : null}

        <div className="provider-split">
          <div className="provider-list" data-testid="provider-list">
            <div className="provider-list-head">
              <span>提供商</span>
              <button className="ghost small" onClick={addProvider} disabled={modelBusy}>
                新增
              </button>
            </div>
            {form.providers.length === 0 ? (
              <p className="hint">还没有提供商。点“新增”建一个。</p>
            ) : null}
            {form.providers.map((p, index) => (
              <button
                key={index}
                className={"provider-item" + (index === editing ? " active" : "")}
                onClick={() => {
                  setEditing(index);
                  setApiKey("");
                  setProbe(null);
                  setProbeError(null);
                }}
              >
                <span className="name">{p.display_name.trim() || p.provider || "未命名"}</span>
                <span className="sub mono">{p.provider || "缺标识"}</span>
                <span className="sub">
                  {keyOk(p.api_key_ref) ? "密钥已配置" : "密钥未配置"}
                  {form.default.provider === p.provider ? " · 默认" : ""}
                </span>
              </button>
            ))}
          </div>

          <div className="provider-editor">
            {!current ? (
              <p className="hint">选择左侧的提供商来编辑，或点“新增”。</p>
            ) : (
              <>
                <div className="field">
                  <label className="field-label">服务商标识（provider，也是配置里的键名）</label>
                  <input
                    className="text-input"
                    value={current.provider}
                    spellCheck={false}
                    placeholder="例如 deepseek"
                    onChange={(e) => updateCurrent({ provider: e.target.value })}
                  />
                </div>

                <div className="field">
                  <label className="field-label">显示名称（你自己看的名字）</label>
                  <input
                    className="text-input"
                    value={current.display_name}
                    spellCheck={false}
                    placeholder="例如 DeepSeek 官方"
                    onChange={(e) => updateCurrent({ display_name: e.target.value })}
                  />
                </div>

                <div className="field">
                  <label className="field-label">接口地址（base_url，必须是 http/https 绝对地址）</label>
                  <input
                    className="text-input mono"
                    value={current.base_url}
                    spellCheck={false}
                    placeholder="https://api.example.com/v1"
                    onChange={(e) => updateCurrent({ base_url: e.target.value })}
                  />
                  <p className="hint">
                    不接受把密钥写进地址（user:pass@ 或 URL 查询串）；密钥请填在下面的输入框。
                  </p>
                </div>

                <div className="field">
                  <label className="field-label">接口协议</label>
                  <select
                    className="text-input"
                    value={current.api}
                    onChange={(e) => updateCurrent({ api: e.target.value })}
                  >
                    {API_CHOICES.map((c) => (
                      <option key={c.value} value={c.value}>
                        {c.label}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="field">
                  <label className="field-label">思考等级（可选）</label>
                  <select
                    className="text-input"
                    value={current.reasoning_effort ?? "default"}
                    onChange={(e) => updateCurrent({ reasoning_effort: e.target.value })}
                  >
                    {EFFORT_CHOICES.map((c) => (
                      <option key={c.value} value={c.value}>
                        {c.label}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="field">
                  <label className="field-label">模型（id 发给端点；名称只影响显示）</label>
                  <div className="model-list">
                    {current.models.map((m, index) => (
                      <div className="model-row" key={index}>
                        <input
                          className="text-input mono"
                          value={m.id}
                          spellCheck={false}
                          placeholder="模型 id，例如 deepseek-v4.1-flash"
                          onChange={(e) => updateModel(index, { id: e.target.value })}
                        />
                        <input
                          className="text-input"
                          value={m.name}
                          spellCheck={false}
                          placeholder="显示名称（可留空）"
                          onChange={(e) => updateModel(index, { name: e.target.value })}
                        />
                        <input
                          className="text-input mono"
                          type="number"
                          min={1}
                          value={m.contextWindow ?? ""}
                          placeholder="上下文窗口"
                          data-hover-hint="上下文窗口（token）。只写配置声明，端点是否真支持需另行核实。"
                          onChange={(e) => {
                            const raw = e.target.value.trim();
                            const parsed = raw === "" ? null : Number(raw);
                            updateModel(index, {
                              contextWindow:
                                parsed !== null && Number.isFinite(parsed) && parsed > 0
                                  ? Math.floor(parsed)
                                  : null,
                            });
                          }}
                        />
                        <button
                          className="ghost small"
                          onClick={() => removeModel(index)}
                          disabled={current.models.length <= 1}
                          data-hover-hint={current.models.length <= 1 ? "至少保留一个模型" : "删除这个模型"}
                        >
                          删除
                        </button>
                      </div>
                    ))}
                  </div>
                  <div className="actions">
                    <button className="ghost small" onClick={addModel}>
                      添加模型
                    </button>
                    {probe && probe.ok && probe.models.length > 0 ? (
                      <button
                        className="ghost small"
                        onClick={() => applyProbeModels()}
                        data-hover-hint="用端点返回的模型列表替换当前列表，并保留同名模型已填的窗口值"
                      >
                        用端点返回的 {probe.models.length} 个模型覆盖列表
                      </button>
                    ) : null}
                  </div>
                  <p className="hint">
                    上下文窗口写成 1048576 即 1M。这是给 DSH 的容量声明，不会让端点凭空变长；
                    端点实际不支持时会报错，请按服务商真实能力填写。
                  </p>
                </div>

                <div className="field">
                  <label className="field-label">凭据名（DSH 配置里的 apiKeyEnv，非机密）</label>
                  <input
                    className="text-input mono"
                    value={current.api_key_ref}
                    spellCheck={false}
                    placeholder="例如 DEEPSEEK_API_KEY"
                    onChange={(e) => updateCurrent({ api_key_ref: e.target.value })}
                  />
                  <p className="hint">
                    DSH 的模型配置只存这个引用名，真正的密钥由 DSH 凭据服务保管。每个提供商必须用不同的引用名。
                  </p>
                </div>

                <div className="field">
                  <label className="field-label">API Key（只写不读；保存后输入框立即清空）</label>
                  <input
                    className="text-input mono"
                    type="password"
                    autoComplete="off"
                    value={apiKey}
                    spellCheck={false}
                    placeholder={keyStatus?.configured ? "已配置（留空表示不改动现有密钥）" : "尚未配置"}
                    onChange={(e) => setApiKey(e.target.value)}
                  />
                  <div className="status-row">
                    <span className="k">凭据状态</span>
                    <span className="v">
                      {keyStatus
                        ? keyStatus.configured
                          ? "已配置" +
                            (keyStatus.source ? "（来源：" + keyStatus.source + "）" : "") +
                            (keyStatus.writable ? "，可在此改写" : "，当前来源不可写")
                          : "尚未配置"
                        : view?.key_error
                          ? "无法读取凭据状态（不影响其它设置）"
                          : view
                            ? "尚未配置"
                            : "读取中…"}
                    </span>
                  </div>
                </div>

                <div className="field">
                  <label className="field-label">设为默认路由</label>
                  <div className="type-row">
                    <select
                      className="text-input"
                      value={form.default.model}
                      onChange={(e) => {
                        if (e.target.value) {
                          setForm({ ...form, default: { provider: current.provider, model: e.target.value } });
                        }
                      }}
                    >
                      <option value="">选一个模型作为默认…</option>
                      {form.default.provider === current.provider
                        ? null
                        : <option value="">（当前默认在其它提供商）</option>}
                      {current.models
                        .filter((m) => m.id.trim().length > 0)
                        .map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name.trim() || m.id}
                          </option>
                        ))}
                    </select>
                    <button
                      className="ghost"
                      onClick={() =>
                        setForm({
                          ...form,
                          default: {
                            provider: current.provider,
                            model: form.default.provider === current.provider ? form.default.model : "",
                          },
                        })
                      }
                      disabled={form.default.provider === current.provider}
                    >
                      设为默认
                    </button>
                  </div>
                  <p className="hint">
                    默认路由指 {form.default.provider || "（未设置）"} / {form.default.model || "（未设置）"}。
                  </p>
                </div>

                <div className="actions">
                  <button className="ghost" onClick={removeProvider} disabled={modelBusy}>
                    删除这个提供商
                  </button>
                </div>
                <p className="hint">
                  删除只移除这里的配置，不会删掉 DSH 凭据服务里已保存的密钥。
                  {form.providers.length <= 1 ? " 删掉最后一个提供商后就没有可用路由了。" : ""}
                </p>
              </>
            )}
          </div>
        </div>
        <div className="actions">
          <button className="primary" onClick={() => void saveModel()} disabled={modelBusy}>
            {modelBusy ? "保存中…" : "保存模型配置"}
          </button>
          <button
            className="ghost"
            onClick={() => void probeConnection()}
            disabled={probeBusy || modelBusy || !current || current.base_url.trim().length === 0}
            data-hover-hint="用当前填写的地址与协议，让 DSH 实际请求一次端点"
          >
            {probeBusy ? "测试中…" : "测试连接"}
          </button>
          <button className="ghost" onClick={() => void refreshModel()} disabled={modelBusy}>
            重新读取
          </button>
        </div>
        {probeError ? <div className="alert">测试没有完成：{probeError}</div> : null}
        {probe ? (
          <div className={probe.ok ? "status-card" : "alert"}>
            <div className="status-row">
              <span className="k">{probe.ok ? "连接结果" : "连接失败"}</span>
              <span className="v">
                {/* 成功时 message 本身已经是「连接成功…」，不再重复前缀。 */}
                {probe.ok
                  ? probe.message
                  : (PROBE_LABEL[probe.classification] ?? probe.classification) + " —— " + probe.message}
              </span>
            </div>
            <div className="status-row">
              <span className="k">说明</span>
              <span className="v">{probe.note}没有保存任何设置。</span>
            </div>
            <div className="status-row">
              <span className="k">本次密钥</span>
              <span className="v">
                {KEY_SOURCE_LABEL[probe.key_source ?? "none"] ?? "未知来源"}
                {probe.provider_route ? `（路由 ${probe.provider_route}）` : ""}
              </span>
            </div>
            {probe.route_mismatch ? (
              <div className="status-row">
                <span className="k">注意</span>
                <span className="v">
                  表单里的地址或协议与该提供商的已保存路由不一致，因此没有沿用它的已存密钥。
                </span>
              </div>
            ) : null}
            {probe.builtin_catalog ? (
              <div className="status-row">
                <span className="k">注意</span>
                <span className="v">
                  该提供商名命中 DSH 内置目录，返回的是内置模型表，未发起真实网络请求；这不能作为连通性证明。
                </span>
              </div>
            ) : null}
          </div>
        ) : null}
        {modelNote ? <p className="hint">{modelNote}</p> : null}
        <p className="hint">
          保存会写入两个<strong>非机密</strong>文件（模型配置与 DSH 补丁），并停掉当前引擎；
          下次任务按新路由启动。密钥只交给 DSH 凭据服务，不进入这两个文件、日志或界面回显。
        </p>
        {view ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">模型配置</span>
              <span className="v mono">{view.config_path}</span>
            </div>
            <div className="status-row">
              <span className="k">DSH 补丁</span>
              <span className="v mono">
                {view.overlay_path}
                {savedOverlay ? "（已存在）" : "（尚未写入）"}
              </span>
            </div>
          </div>
        ) : null}

        {factsError ? (
          <div className="alert">
            无法读取端点：{factsError}
            {facts ? "（下面仍是上一次读到的结果，可点「刷新端点信息」重试）" : ""}
          </div>
        ) : null}
        {facts && !facts.configurable ? (
          <div className="alert">
            当前端点位置无法确认，宿主会一律拒绝此类端点。请改成明确的 http/https 地址后保存。
          </div>
        ) : null}
        {facts ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">当前路由</span>
              <span className="v mono">
                {facts.provider} / {facts.model}
              </span>
            </div>
            <div className="status-row">
              <span className="k">端点</span>
              <span className="v mono">{facts.endpoint ?? "无法解析"}</span>
            </div>
            <div className="status-row">
              <span className="k">类别</span>
              <span className="v">
                {CATEGORY_LABEL[facts.category] ?? facts.category}
                {facts.leaves_device === true
                  ? "（内容会发往本机以外的地址）"
                  : facts.leaves_device === false
                    ? "（仅本机回环）"
                    : "（是否离开本机/组织无法确认）"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">判定依据</span>
              <span className="v">{facts.category_reason}</span>
            </div>
            <div className="status-row">
              <span className="k">当前判定</span>
              <span className="v">{facts.allowed ? "允许提交" : "拒绝提交"}</span>
            </div>
          </div>
        ) : null}
      </section>

      {/* ============ 2. Profile 资产（OD-ARCH-109） ============ */}
      <section className="pane" hidden={tab !== "assets"}>
        <CardHead
          title="Profile 资产"
          status={
            assets
              ? assets.standard
                ? "基础标准 · 不安装任何资产"
                : `${assets.label} · 就绪 ${readyCount}/${assets.items.length}`
              : assetsError
                ? "读取失败"
                : "读取中…"
          }
        >
          <button className="ghost" onClick={() => void refreshAssets()} disabled={assetsBusy}>
            重新读取
          </button>
          {assets && !assets.standard ? (
            <button
              className="ghost"
              disabled={assetsBusy || repairing}
              onClick={() => void repairAssets()}
              data-hover-hint="按出厂内容补齐缺失项；被改过的文件会先备份成 .bak-<时间戳> 再写回。插件不在这里装。"
              data-testid="profile-asset-repair"
            >
              {repairing ? "修复中…" : "修复缺失 / 恢复出厂内容"}
            </button>
          ) : null}
        </CardHead>
        <p className="hint">
          OmniDesk 本身是干净底座，专属能力由 Profile 带来。切换 Profile 时会自动补齐它需要的
          配置、场景声明、规则与 Skill；插件在开启自动补齐时由后台联网安装（切换不等安装），
          关掉后就只提示缺哪些、由你点「安装」。
          打开页面只判定、不改文件；缺项或想还原改动时，点「修复缺失 / 恢复出厂内容」。
        </p>
        {assetsError ? <div className="alert">Profile 资产：{assetsError}</div> : null}
        {assetsNote ? <p className="hint">{assetsNote}</p> : null}
        {repairReceipt ? (
          <div
            className={"asset-receipt " + (repairReceipt.ok ? "ok" : "bad")}
            data-testid="profile-asset-repair-receipt"
          >
            <p>
              {repairReceipt.note} 本次补写 / 更新 {repairReceipt.changed} 项
              {repairReceipt.remaining > 0
                ? `，仍有 ${repairReceipt.remaining} 项未就位`
                : "，清单已无缺项"}
              。
            </p>
          </div>
        ) : null}
        {assets?.repo_missing ? <div className="alert">{assets.repo_missing}</div> : null}
        {assets?.summary_note ? <p className="hint">{assets.summary_note}</p> : null}
        {assets && !assets.standard ? (
          <div
            className="status-card"
            data-testid="profile-auto-install-pref"
          >
            <label className="check">
              <input
                type="checkbox"
                checked={autoInstall ?? true}
                disabled={autoInstallBusy || autoInstall === null}
                onChange={(e) => void changeAutoInstall(e.target.checked)}
              />
              <span>
                <strong>切换或启动 Profile 时自动补齐缺失插件</strong>
                <br />
                {autoInstallBusy
                  ? "正在保存…"
                  : autoInstall === false
                    ? "关：只提示缺哪些插件，由你点按钮安装。"
                    : "开：切换后或冷启动时先在后台联网装齐，切换与启动本身都不等安装。"}
                <span className="sub">
                  {" "}
                  基础标准永远不安装任何插件。
                </span>
              </span>
            </label>
          </div>
        ) : null}
        {assets && !assets.standard && missingPluginCount > 0 ? (
          <div className="status-card" data-testid="profile-plugin-missing">
            <div className="status-row">
              <span className="k">缺失插件</span>
              <span className="v">
                有 {missingPluginCount} 个插件本地没装到。点右边会联网、通过 DSH 官方插件通道
                逐个装到「{assets.label}」。
              </span>
            </div>
            <button
              className="ghost"
              disabled={installing !== null}
              onClick={() => void installMissingPlugins()}
              data-testid="profile-plugin-install-missing"
            >
              {installing ? "安装中…" : `一键安装缺失插件（${missingPluginCount}）`}
            </button>
          </div>
        ) : null}

        {assets?.standard ? (
          <div className="status-card" data-testid="profile-assets-standard">
            <div className="status-row">
              <span className="k">当前 Profile</span>
              <span className="v mono">{assets.profile}（基础标准）</span>
            </div>
            <div className="status-row">
              <span className="k">资产</span>
              <span className="v">
                不写入任何文件，也不创建 Profile 目录；Skill 仍可被模型按需加载。
              </span>
            </div>
          </div>
        ) : null}

        {assets && !assets.standard && assets.items.length === 0 ? (
          <p className="hint">
            这个 Profile 没有出厂资产清单，也没有声明 Skill 绑定或额外插件：保持现状，不写入任何文件。
          </p>
        ) : null}

        {assets && !assets.standard ? (
          <div className="asset-groups" data-testid="profile-asset-groups">
            {ASSET_GROUPS.map((group) => {
              const rows = assets.items.filter((i) => group.kinds.includes(i.kind));
              if (rows.length === 0) return null;
              return (
                <div className="provider-list" key={group.title}>
                  <div className="provider-list-head">
                    <span>{group.title}</span>
                    <span className="sub">{rows.length}</span>
                  </div>
                  {rows.map((item) => {
                    const badge = ASSET_STATUS[item.status] ?? {
                      label: item.status,
                      tone: "neutral" as const,
                    };
                    const receipt = installReceipts[item.key.replace(/^plugin:/, "")];
                    return (
                      <div className="asset-row" key={item.key} data-testid={"profile-asset-" + item.key}>
                        <div className="asset-row-main">
                          <div className="asset-row-title">
                            <span className="name" data-hover-hint={item.path}>
                              {item.label}
                            </span>
                            <span
                              className={"asset-badge " + badge.tone}
                              data-hover-hint={item.detail}
                              data-testid={"profile-asset-status-" + item.key}
                            >
                              {badge.label}
                            </span>
                          </div>
                          <p className="sub" data-hover-hint={item.detail}>
                            {item.detail}
                            {item.path ? " · " + shortPath(item.path) : ""}
                          </p>
                          {receipt ? (
                            <div
                              className={"asset-receipt " + (receipt.ok ? "ok" : "bad")}
                              data-testid={"profile-asset-receipt-" + item.key}
                            >
                              <p>{receipt.note}</p>
                              <p className="mono">{receipt.command}</p>
                              {receipt.exit_code !== null ? (
                                <p className="mono">退出码 {receipt.exit_code}</p>
                              ) : null}
                              {receipt.stdout.trim() ? (
                                <pre className="mono">{receipt.stdout.trim()}</pre>
                              ) : null}
                              {receipt.stderr.trim() ? (
                                <pre className="mono">{receipt.stderr.trim()}</pre>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                        {item.kind === "plugin" ? (
                          <button
                            className="ghost"
                            disabled={
                              installing !== null ||
                              item.status === "ready" ||
                              item.status === "unchanged"
                            }
                            onClick={() =>
                              void installPlugin(
                                assets.profile,
                                item.key.replace(/^plugin:/, ""),
                              )
                            }
                            data-hover-hint={
                              item.status === "ready"
                                ? "本地已就绪，无需安装"
                                : "会联网，通过 DSH 官方插件通道装到当前 Profile"
                            }
                            data-testid={"profile-asset-install-" + item.key}
                          >
                            {installing === item.key.replace(/^plugin:/, "")
                              ? "安装中…"
                              : item.status === "ready"
                                ? "已就绪"
                                : "安装"}
                          </button>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        ) : null}

        {/* 自建 Profile 的插件入口（OD-ARCH-128）：登记表与装配同源，装成功才登记。 */}
        {assets && !assets.standard && isCustomAssetProfile ? (
          <div className="provider-list" data-testid="profile-plugin-declare">
            <div className="provider-list-head">
              <span>声明插件</span>
              <span className="sub">装成功才写进这个 Profile 的清单</span>
            </div>
            <div className="asset-row">
              <div className="asset-row-main">
                <input
                  className="text-input asset-row-input mono"
                  value={pluginDraft}
                  maxLength={214}
                  spellCheck={false}
                  placeholder="@scope/plugin 或 plugin-name，可带 @1.2.3"
                  aria-label="要声明并安装的插件包名"
                  onChange={(e) => setPluginDraft(e.target.value)}
                  // 聚焦即全选：上一次被拒的写法不会被接着往后拼。
                  onFocus={(e) => e.currentTarget.select()}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void addPlugin();
                  }}
                  data-testid="profile-plugin-input"
                />
                <p className="sub">
                  本地已就绪就只登记、不联网；缺件才走 DSH 插件通道安装。
                  基础标准与出厂 Profile 不接受这里改动。
                </p>
                {pluginAddReceipt ? (
                  <div
                    className={"asset-receipt " + (pluginAddReceipt.ok ? "ok" : "bad")}
                    role="status"
                    aria-label={pluginAddReceipt.note}
                    data-testid="profile-plugin-receipt"
                  >
                    <p>{pluginAddReceipt.note}</p>
                  </div>
                ) : null}
                {pluginAddError ? (
                  <div
                    className="asset-receipt bad"
                    role="alert"
                    aria-label={pluginAddError}
                    data-testid="profile-plugin-error"
                  >
                    <p>{pluginAddError}</p>
                  </div>
                ) : null}
              </div>
              <div className="asset-row-actions">
                <button
                  className="primary"
                  disabled={pluginAdding || pluginDraft.trim().length === 0}
                  onClick={() => void addPlugin()}
                  data-hover-hint="先探测本地，缺件才联网安装；装不上不会写进清单"
                  data-testid="profile-plugin-add"
                >
                  {pluginAdding ? "处理中…" : "安装并登记"}
                </button>
              </div>
            </div>
          </div>
        ) : null}

        {assets?.notes.length ? (
          <div className="asset-notes">
            {assets.notes.map((n) => (
              <p className="hint" key={n}>
                {n}
              </p>
            ))}
          </div>
        ) : null}

        {parked.length ? (
          <div className="provider-list" data-testid="profile-parked-skills">
            <div className="provider-list-head">
              <span>已停放</span>
              <span className="sub">{parked.length}</span>
            </div>
            {parked.map((p) => (
              <div className="asset-row" key={p.name} data-testid={"profile-parked-" + p.name}>
                <div className="asset-row-main">
                  <div className="asset-row-title">
                    <span className="name">{p.name}</span>
                    <span className="asset-badge neutral" data-hover-hint={p.path}>
                      停放中
                    </span>
                  </div>
                  <p className="sub">
                    不属于当前 Profile，切回需要它的 Profile 会自动归位。
                  </p>
                </div>
              </div>
            ))}
          </div>
        ) : null}

        {/* 自建 Profile 管理：新建入口在顶栏，这里负责列出、移除、改名与迁移 */}
        <div className="provider-list" data-testid="profile-custom-list">
          <div className="provider-list-head">
            <span>自建 Profile</span>
            <span className="asset-row-actions">
              <span className="sub">{customProfiles.length}</span>
              <button
                className="ghost"
                disabled={importBusy}
                onClick={() => void pickImportFile()}
                data-hover-hint="从 .omniprofile.json 迁移包装回一套工作台；插件按清单自动补装"
                data-testid="profile-import-open"
              >
                {importBusy ? "导入中…" : "导入"}
              </button>
            </span>
          </div>
          {customProfiles.length === 0 ? (
            <p className="hint">还没有自建 Profile。可以从顶栏新建，也可以导入迁移包。</p>
          ) : null}
            {customProfiles.map((p) => {
              const inUse = assets?.profile === p.name;
              const renaming = renamingProfile === p.name;
              return (
                <div className="asset-row" key={p.name} data-testid={"profile-custom-" + p.name}>
                  <div className="asset-row-main">
                    <div className="asset-row-title">
                      {renaming ? (
                        <input
                          className="text-input asset-row-input"
                          value={renameDraft}
                          maxLength={32}
                          autoFocus
                          onChange={(e) => setRenameDraft(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") void saveRename(p.name);
                            if (e.key === "Escape") cancelRename();
                          }}
                          aria-label="新的显示名"
                          data-testid={"profile-rename-input-" + p.name}
                        />
                      ) : (
                        <span className="name">{p.label}</span>
                      )}
                      <span className="asset-badge info" data-hover-hint={p.origin}>
                        {p.origin === "standard" ? "基础起手" : "复制 " + p.origin.slice(5)}
                      </span>
                      {inUse ? <span className="asset-badge ok">当前</span> : null}
                    </div>
                    <p className="sub mono">{p.name}</p>
                  </div>
                  {renaming ? (
                    <div className="asset-row-actions">
                      <button
                        className="ghost"
                        onClick={cancelRename}
                        data-testid={"profile-rename-cancel-" + p.name}
                      >
                        取消
                      </button>
                      <button
                        className="primary"
                        disabled={renameBusy || renameDraft.trim().length === 0}
                        data-hover-hint="只改显示名，目录名、Skill 与插件绑定都不动"
                        onClick={() => void saveRename(p.name)}
                        data-testid={"profile-rename-save-" + p.name}
                      >
                        {renameBusy ? "保存中…" : "保存"}
                      </button>
                    </div>
                  ) : (
                    <div className="asset-row-actions">
                      <button
                        className="ghost"
                        disabled={exportingProfile !== null || removingProfile !== null}
                        data-hover-hint="导出成单文件迁移包；不含插件本体，在新机器导入后按清单补装"
                        onClick={() => void exportProfile(p)}
                        data-testid={"profile-custom-export-" + p.name}
                      >
                        {exportingProfile === p.name ? "导出中…" : "导出"}
                      </button>
                      <button
                        className="ghost"
                        disabled={renameBusy || removingProfile !== null}
                        data-hover-hint="改这个工作台的显示名"
                        onClick={() => startRename(p)}
                        data-testid={"profile-custom-rename-" + p.name}
                      >
                        改名
                      </button>
                      <button
                        className="ghost"
                        disabled={removingProfile !== null || inUse}
                        data-hover-hint={
                          inUse
                            ? "这是当前 Profile，先切到别的再移除"
                            : "移入回收站，可在下方一键恢复"
                        }
                        onClick={() => void removeProfile(p.name)}
                        data-testid={"profile-custom-remove-" + p.name}
                      >
                        {removingProfile === p.name ? "移除中…" : "移除"}
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          {importConflict ? (
            <div className="asset-row" data-testid="profile-import-conflict">
              <div className="asset-row-main">
                <div className="asset-row-title">
                  <span className="name">这个包的名字已被占用</span>
                  <span className="asset-badge warn" data-hover-hint={importConflict.note}>
                    需改名
                  </span>
                </div>
                <p className="sub">{importConflict.note}</p>
              </div>
              <div className="asset-row-actions">
                <input
                  className="text-input asset-row-input"
                  value={importNameDraft}
                  maxLength={48}
                  autoFocus
                  onChange={(e) => setImportNameDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && importNameDraft.trim()) {
                      void importProfile(importConflict.path, importNameDraft.trim());
                    }
                    if (e.key === "Escape") setImportConflict(null);
                  }}
                  aria-label="导入用的新标识"
                  data-testid="profile-import-name"
                />
                <button
                  className="ghost"
                  onClick={() => setImportConflict(null)}
                  data-testid="profile-import-cancel"
                >
                  取消
                </button>
                <button
                  className="primary"
                  disabled={importBusy || importNameDraft.trim().length === 0}
                  data-hover-hint="用这个标识新建一套工作台；已有同名工作台不会被覆盖"
                  onClick={() =>
                    void importProfile(importConflict.path, importNameDraft.trim())
                  }
                  data-testid="profile-import-confirm"
                >
                  {importBusy ? "导入中…" : "导入"}
                </button>
              </div>
            </div>
          ) : null}
        </div>
        {renameReceipt ? <p className="hint">{renameReceipt}</p> : null}
        {removeReceipt ? <p className="hint">{removeReceipt}</p> : null}
        {migrateReceipt ? (
          <p className="hint" data-testid="profile-migrate-receipt">
            {migrateReceipt}
          </p>
        ) : null}

        {/* 迁移包（OD-ARCH-127）：导出落在数据根的 exports/ 里，这里直接列出来一键导入。 */}
        <div className="provider-list" data-testid="profile-bundle-list">
          <div className="provider-list-head">
            <span>迁移包</span>
            <span className="asset-row-actions">
              <span className="sub">{bundles.length}</span>
              <button
                className="ghost"
                onClick={() =>
                  void shell
                    .profileExportOpen()
                    .catch((e) =>
                      setAssetsError(e instanceof Error ? e.message : String(e)),
                    )
                }
                data-hover-hint="在系统文件管理器里打开导出目录，方便拷到别的机器"
                data-testid="profile-export-open"
              >
                打开目录
              </button>
            </span>
          </div>
          {bundles.length === 0 ? (
            <p className="hint">
              还没有导出过。点任意一台工作台行尾的「导出」，包会落在这里。
            </p>
          ) : (
            bundles.map((b) => (
              <div className="asset-row" key={b.file} data-testid={"profile-bundle-" + b.profile}>
                <div className="asset-row-main">
                  <div className="asset-row-title">
                    <span className="name">{b.label}</span>
                    <span className="asset-badge neutral" data-hover-hint={b.path}>
                      {(b.bytes / 1024).toFixed(1)} KB
                    </span>
                  </div>
                  <p className="sub mono">
                    {b.profile}
                    {b.exported_at > 0
                      ? " · 导出于 " + new Date(b.exported_at * 1000).toLocaleString()
                      : ""}
                  </p>
                </div>
                <button
                  className="ghost"
                  disabled={importBusy}
                  data-hover-hint="校验后装入这个工作台；同名不会覆盖，会让你改个标识"
                  onClick={() => void importProfile(b.path, null)}
                  data-testid={"profile-bundle-import-" + b.profile}
                >
                  导入这套
                </button>
              </div>
            ))
          )}
        </div>

        {/* 回收站（OD-ARCH-125）：移除是移入而不是删除，这里给回软件内的恢复通道。 */}
        {trash.length ? (
          <div className="provider-list" data-testid="profile-trash-list">
            <div className="provider-list-head">
              <span>回收站</span>
              <span className="sub">{trash.length}</span>
            </div>
            {trash.map((t) => (
              <div className="asset-row" key={t.id} data-testid={"profile-trash-" + t.id}>
                <div className="asset-row-main">
                  <div className="asset-row-title">
                    <span className="name">{t.label}</span>
                    <span
                      className={t.has_meta ? "asset-badge neutral" : "asset-badge warn"}
                      data-hover-hint={t.note}
                    >
                      {t.has_meta ? "可恢复" : "无元数据"}
                    </span>
                  </div>
                  <p className="sub mono">
                    {t.name}
                    {t.trashed_at > 0
                      ? " · 移除于 " + new Date(t.trashed_at * 1000).toLocaleString()
                      : ""}
                  </p>
                </div>
                <button
                  className="ghost"
                  disabled={restoringTrash !== null}
                  data-hover-hint="把配置与资产目录搬回原位，并重新登记为可选 Profile"
                  onClick={() => void restoreFromTrash(t.id)}
                  data-testid={"profile-trash-restore-" + t.id}
                >
                  {restoringTrash === t.id ? "恢复中…" : "恢复"}
                </button>
              </div>
            ))}
          </div>
        ) : null}
        {restoreReceipt ? <p className="hint">{restoreReceipt}</p> : null}
      </section>

      {/* ============ 3. 对话分类 ============ */}
      <section className="pane" hidden={tab !== "categories"}>
        <CardHead
          title="对话分类"
          status={categories ? `${categories.length} 类（本机持久化）` : "读取中…"}
        >
          <button className="ghost" onClick={() => void refreshCategories()} disabled={catBusy}>
            重新读取
          </button>
          <button className="primary" onClick={() => void initCategoryFolders()} disabled={catBusy}>
            初始化目录骨架
          </button>
        </CardHead>
        <p className="hint">
          首页输入框左边的胶囊就是这里的分类。选一个只改变这段本机对话归到哪一类，
          不改模型、不改路由，也不移动任何已有文件。声明里隐藏了某个场景时，对应的内置分类不再出现。
          <strong>这份分类表属于当前 Profile：</strong>切到基础标准只剩「未分类」，
          切回专业 Profile 时你的改名与新增分类会原样恢复（切走时自动回存到该 Profile）。
        </p>

        {catError ? <div className="alert">分类设置：{catError}</div> : null}
        {catNote ? <p className="hint">{catNote}</p> : null}
        {catInit ? (
          <div className="status-card">
            {catInit.items.map((it) => (
              <div className="status-row" key={it.key}>
                <span className="k mono">{it.key}</span>
                <span className="v">
                  {it.path} · {it.created ? "本次新建" : "已存在，未改动"}
                </span>
              </div>
            ))}
          </div>
        ) : null}

        <div data-testid="thread-categories">
          {categories === null ? (
            <p className="hint">还没读到分类设置。</p>
          ) : (
            categories.map((c) => {
              const draft = catDrafts[c.key] ?? { label: c.label, folder: "", vault: "" };
              return (
                <div key={c.key} className="status-card" style={{ marginBottom: 10 }}>
                  <div className="status-row">
                    <span className="k mono">
                      {c.key}
                      {c.builtin ? "" : " · 自定义"}
                    </span>
                    <span className="v">
                      {c.resolved_dir
                        ? "归档到 " + c.resolved_dir
                        : c.resolve_error
                          ? "目录不可用：" + c.resolve_error
                          : "没设归档目录（不影响归类）"}
                    </span>
                  </div>
                  <div className="field">
                    <label className="field-label">显示名（只改名字，不改内部键与已有文件）</label>
                    <input
                      className="text-input"
                      value={draft.label}
                      maxLength={24}
                      onChange={(e) =>
                        setCatDrafts((prev) => ({
                          ...prev,
                          [c.key]: { ...draft, label: e.target.value },
                        }))
                      }
                    />
                  </div>
                  <div className="field">
                    <label className="field-label">
                      成果根下的子文件夹（留空表示不映射；相对路径，不能写 .. 或绝对路径）
                    </label>
                    <input
                      className="text-input"
                      value={draft.folder}
                      placeholder="例如 工作记录"
                      onChange={(e) =>
                        setCatDrafts((prev) => ({
                          ...prev,
                          [c.key]: { ...draft, folder: e.target.value },
                        }))
                      }
                    />
                  </div>
                  <div className="field">
                    <label className="field-label">
                      Obsidian vault 绝对路径（可选；与左边二选一，填了以 vault 为准）
                    </label>
                    <input
                      className="text-input"
                      value={draft.vault}
                      placeholder="/home/你/某个/vault"
                      onChange={(e) =>
                        setCatDrafts((prev) => ({
                          ...prev,
                          [c.key]: { ...draft, vault: e.target.value },
                        }))
                      }
                    />
                  </div>
                  <div className="actions">
                    <button
                      className="primary"
                      onClick={() => void saveCategory(c.key)}
                      disabled={catBusy}
                    >
                      保存这一行
                    </button>
                    {c.builtin ? null : (
                      <button
                        className="ghost"
                        onClick={() => void removeCategory(c.key, c.label)}
                        disabled={catBusy}
                      >
                        删除这个自定义分类
                      </button>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>

        <div className="status-card">
          <p className="hint">
            新增分类只进归类下拉与归档映射，不会自动出现在 Dock；三个业务场景入口仍由下面的声明决定。
          </p>
          <div className="field">
            <label className="field-label">新分类名（1–24 字）</label>
            <input
              className="text-input"
              value={newCatLabel}
              maxLength={24}
              onChange={(e) => setNewCatLabel(e.target.value)}
              placeholder="例如 学习笔记"
            />
          </div>
          <div className="field">
            <label className="field-label">归档子文件夹（可选，留空以后在分类行里补）</label>
            <input
              className="text-input"
              value={newCatFolder}
              onChange={(e) => setNewCatFolder(e.target.value)}
              placeholder="例如 学习笔记"
            />
          </div>
          <div className="actions">
            <button className="primary" onClick={() => void addCategory()} disabled={catBusy}>
              新增分类
            </button>
          </div>
        </div>
        <p className="hint">
          目录规则：成果子文件夹只能落在成果根内，vault 用绝对路径且不能是应用数据目录；
          越界会被拒绝并在这里显示原因。「初始化目录骨架」只建缺失目录，已有目录与文件原样保留。
        </p>

        <CardHead
          title="入口声明"
          status="决定 Dock 显示哪些场景"
        />
        <p className="hint">
          声明只管界面显示哪些入口、叫什么名字，不影响分类标签本身与内部场景键。
        </p>
      </section>

      {/* ============ 声明编辑（归在「对话分类」页） ============ */}
      <section className="pane" hidden={tab !== "categories"}>
        <CardHead
          title="场景声明"
          status={
            decl
              ? decl.decl.display_name + " · " + String(decl.decl.scenes.length) + " 个入口"
              : declError
                ? "声明未能载入"
                : "该 Profile 没有场景声明（标准模式）"
          }
        >
          <button className="ghost" onClick={() => void openDecl()} disabled={declBusy}>
            在系统编辑器打开
          </button>
          <button className="ghost" onClick={() => void refreshDecl()} disabled={declBusy}>
            重新读取
          </button>
          <button
            className="ghost"
            onClick={() => void fillStarterDecl()}
            disabled={declBusy}
            data-hover-hint="把一份五个标准入口的最小声明灌进编辑框；只改编辑框，点「校验并保存」才会落文件。已有声明会被覆盖（旧文件先备份）。"
            data-testid="profile-decl-starter"
          >
            {declBusy ? "读取中…" : "填入起步模板"}
          </button>
        </CardHead>
        <p className="hint">
          这份声明决定左侧 Dock 显示哪些业务场景，以及工作台叫什么名字。它只影响界面入口，
          不改插件、路由或权限。保存前会先校验；校验不过不会写文件，旧文件在写入前自动备份。
          <strong>保存后工作台入口立刻更新，不需要重启；在应用外部手改文件则需回来重新保存或切换 Profile。</strong>
          声明随 Profile 挂载：基础标准下没有这份文件是正常的（只有平台固有入口），
          这里保存一份即属于当前 Profile。
        </p>
        {declStandard ? (
          <p className="hint" data-testid="profile-decl-standard-hint">
            当前是基础标准（干净底座），不挂业务场景，保存会被拒绝。要挂载工作台，
            先到顶栏切到一个 Profile；这里的模板可以先看、先改。
          </p>
        ) : null}
        {declError ? <div className="alert">场景声明：{declError}</div> : null}
        {decl ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">文件</span>
              <span className="v mono">{decl.path}</span>
            </div>
            <div className="status-row">
              <span className="k">载入指纹</span>
              <span className="v mono">{decl.content_sha256.slice(0, 16)}…</span>
            </div>
            <div className="status-row">
              <span className="k">当前入口</span>
              <span className="v">{decl.decl.scenes.map((s) => s.label).join(" / ")}</span>
            </div>
          </div>
        ) : declError ? null : (
          <p className="hint">
            当前 Profile 没有场景声明，Dock 只显示平台固有入口（首页 / 历史会话 / 知识库）。
            需要业务场景时，在下方填入声明并保存，或先在顶栏切到带场景的 Profile。
          </p>
        )}
        <div className="field">
          <label className="field-label">声明内容（YAML）</label>
          <textarea
            className="text-input mono"
            rows={14}
            spellCheck={false}
            value={declText}
            onChange={(e) => setDeclText(e.target.value)}
            placeholder={"当前 Profile 没有场景声明。填入 omnidesk.profile-decl.v1 内容并保存即可为这个 Profile 挂载业务场景。"}
          />
        </div>
        <div className="actions">
          <button
            className="primary"
            onClick={() => void saveDecl()}
            disabled={declBusy || declText.trim().length === 0 || declStandard === true}
            data-testid="profile-decl-save"
            data-hover-hint={
              declStandard
                ? "基础标准是干净底座，不挂业务场景。先在顶栏切到一个 Profile 再保存。"
                : "校验通过后写入当前 Profile，工作台入口立刻更新"
            }
          >
            {declBusy ? "校验并保存中…" : "校验并保存"}
          </button>
        </div>
        {declNote ? <p className="hint">{declNote}</p> : null}
      </section>

      {/* ============ 规则资产（行业规则包，可在此查看与编辑） ============ */}
      <section className="pane" hidden={tab !== "rules"}>
        <CardHead
          title="规则资产"
          status={
            rules
              ? rules.error
                ? "文件损坏，提交会被拒绝"
                : rules.loaded
                  ? "已加载 · " + String(rules.rules?.profile ?? "")
                  : "未加载（纯净底座）"
              : rulesError
                ? "读取失败"
                : "读取中…"
          }
        >
          <button className="ghost" onClick={() => void refreshRules()} disabled={rulesBusy}>
            重新读取
          </button>
          <button
            className="ghost"
            onClick={() => void restoreRules()}
            disabled={rulesBusy || !rules?.default_available || Boolean(rules?.default_blocked_reason)}
            data-testid="rules-restore-default"
            data-hover-hint={
              rules?.default_blocked_reason ?? "写入出厂行业规则包（会先备份现有文件）"
            }
          >
            恢复默认规则包
          </button>
        </CardHead>
        <p className="hint">
          这里放的是<strong>行业规则</strong>：算不能说的词、写作必须遵守的规矩、各场景的默认思考档，
          以及身份证号和手机号是拦下还是脱敏后放行。OmniDesk 本体不含任何行业内容——
          没有这份文件时就是一个干净底座。保存后下一轮提交立即生效，不用重启；旧文件会先备份。
        </p>
        {rules?.default_blocked_reason ? (
          <p className="hint" data-testid="rules-default-blocked">
            {rules.default_blocked_reason}
          </p>
        ) : null}
        {rules?.migration_warning ? (
          <p className="hint" data-testid="rules-migration-warning">
            {rules.migration_warning}
          </p>
        ) : null}
        {rulesError ? <div className="alert">规则资产：{rulesError}</div> : null}
        {/* OD-ARCH-088：旧版（v1）规则包没有场景 Skill 绑定。只提示、不改写文件；
            是否启用出厂绑定由用户显式点「恢复默认规则包」决定。 */}
        {rules?.loaded &&
        rules.rules &&
        Object.keys(rules.rules.skill_bindings ?? {}).length === 0 ? (
          <p
            className="hint"
            role="note"
            aria-label="当前规则包是旧版，未声明场景 Skill 绑定，因此场景技能不会预注入。模型仍可按需自行加载技能——这不代表技能不可用。"
            data-testid="rules-skill-bindings-missing"
          >
            当前规则包是旧版，未声明场景 Skill 绑定，因此场景技能不会预注入。
            模型仍可按需自行加载技能——这不等于技能不可用。
            出厂规则包包含场景 Skill 绑定；需要启用时点上方「恢复默认规则包」（会先备份现有文件）。
          </p>
        ) : null}
        {rules ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">文件</span>
              <span className="v mono">{rules.path}</span>
            </div>
            <div className="status-row">
              <span className="k">指纹</span>
              <span className="v mono">
                {rules.content_sha256 ? rules.content_sha256.slice(0, 16) + "…" : "—"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">红线词</span>
              <span className="v">
                {rules.rules ? String(rules.rules.global.redline_keywords.length) + " 个" : "—"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">写作铁律</span>
              <span className="v">
                {rules.rules ? String(rules.rules.global.writing_rules.length) + " 条" : "—"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">隐私处理</span>
              <span className="v">
                {rules.rules
                  ? "身份证：" +
                    (rules.rules.global.privacy.id_card === "mask" ? "自动脱敏" : "拒绝") +
                    " / 手机号：" +
                    (rules.rules.global.privacy.phone === "mask" ? "自动脱敏" : "拒绝")
                  : "—"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">场景</span>
              <span className="v">
                {rules.rules && Object.keys(rules.rules.scenes).length > 0
                  ? Object.entries(rules.rules.scenes)
                      .map(([key, scene]) => key + "（" + scene.thinking + "）")
                      .join(" / ")
                  : "—"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">Skill 绑定</span>
              <span className="v">
                {rules.rules?.skill_bindings && Object.keys(rules.rules.skill_bindings).length > 0
                  ? Object.entries(rules.rules.skill_bindings)
                      .map(([key, b]) => key + "（" + (b.skills.join(", ") || "无") + "）")
                      .join(" / ")
                  : "—"}
              </span>
            </div>
          </div>
        ) : null}

        {/* ===== 场景 Skill 绑定（OD-ARCH-129）=====
            这是「切过去就是专属工作台」的实际配置面：勾哪个场景绑哪些 Skill，
            保存时由宿主整表替换 skill_bindings，其余规则内容不动。 */}
        {!declStandard ? (
          <div className="binding-block" data-testid="skill-binding-editor">
            <div className="binding-head">
              <span className="binding-title">场景 Skill 绑定</span>
              <span className="sub">
                勾选即预注入；不勾表示该场景只靠模型按需加载。每场景最多{" "}
                {MAX_BINDINGS_PER_SCENE} 个。
              </span>
            </div>
            {(rules?.allowed_skills ?? []).length === 0 ? (
              <p className="hint">当前实例还没有可绑定的 Skill。</p>
            ) : null}
            <div className="binding-rows">
              {BINDING_SCENES.map((scene) => {
                const picked = bindingDraft[scene] ?? [];
                return (
                  <div className="binding-row" key={scene} data-testid={`binding-row-${scene}`}>
                    <div className="binding-row-main">
                      <span className="binding-scene">{sceneLabel(scene)}</span>
                      <span className="sub">{BINDING_SCENE_HINTS[scene]}</span>
                    </div>
                    <div className="binding-chips">
                      {(rules?.allowed_skills ?? []).map((skill) => {
                        const on = picked.includes(skill);
                        const full = !on && picked.length >= MAX_BINDINGS_PER_SCENE;
                        return (
                          <button
                            key={skill}
                            type="button"
                            className={"binding-chip" + (on ? " on" : "")}
                            aria-pressed={on}
                            disabled={bindingBusy || full}
                            data-testid={`binding-chip-${scene}-${skill}`}
                            data-hover-hint={
                              full
                                ? `「${sceneLabel(scene)}」已绑满 ${MAX_BINDINGS_PER_SCENE} 个`
                                : on
                                  ? `取消 ${skill}`
                                  : `在「${sceneLabel(scene)}」里预注入 ${skill}`
                            }
                            onClick={() => toggleBinding(scene, skill)}
                          >
                            {skill}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="actions">
              <button
                className="primary"
                onClick={() => void saveBindings()}
                disabled={bindingBusy || Boolean(rules?.default_blocked_reason)}
                data-hover-hint="保存整张绑定表；只改 skill_bindings，其它规则内容原样保留"
                data-testid="skill-binding-save"
              >
                {bindingBusy ? "保存中…" : "保存绑定"}
              </button>
            </div>
            {/* OD-ARCH-131：这个 Profile 自己的 Skill 源。放在绑定区底部：
                先把正文放进目录，再回到上面的勾选片绑定，顺序与装配一致。 */}
            <div className="binding-source" data-testid="skill-source-block">
              <div className="binding-row-main">
                <span className="binding-scene">Profile 自带 Skill</span>
                <span className="sub">
                  {(rules?.local_skills ?? []).length > 0
                    ? `已有 ${(rules?.local_skills ?? []).length} 个：${(rules?.local_skills ?? []).join("、")}`
                    : "还没有。放进这个目录的 Skill 优先于仓库与内置副本。"}
                </span>
              </div>
              <div className="binding-source-actions">
                <button
                  type="button"
                  className="ghost"
                  onClick={() => void openSkillSource()}
                  disabled={skillSourceBusy}
                  data-testid="skill-source-open"
                  data-hover-hint="打开当前 Profile 的 Skill 目录（首次打开时才创建）"
                >
                  打开目录
                </button>
                <input
                  className="text-input"
                  value={skillSourceName}
                  onChange={(e) => setSkillSourceName(e.target.value)}
                  placeholder="新 Skill 名，如 office-note"
                  spellCheck={false}
                  disabled={skillSourceBusy}
                  data-testid="skill-source-name"
                  data-hover-hint="只允许小写字母、数字、连字符、下划线"
                />
                <button
                  type="button"
                  className="ghost"
                  onClick={() => void scaffoldSkill()}
                  disabled={skillSourceBusy || skillSourceName.trim().length === 0}
                  data-testid="skill-source-scaffold"
                  data-hover-hint="生成一份带 frontmatter 的 SKILL.md 模板；已存在时不覆盖"
                >
                  生成模板
                </button>
              </div>
            </div>
            {bindingError ? (
              <p className="hint" role="alert" data-testid="skill-binding-error">
                {bindingError}
              </p>
            ) : null}
            {bindingNote ? (
              <p className="hint" role="status" data-testid="skill-binding-note">
                {bindingNote}
              </p>
            ) : null}
          </div>
        ) : null}
        <div className="field">
          <label className="field-label">规则内容（JSON）</label>
          <textarea
            className="text-input mono"
            rows={18}
            spellCheck={false}
            value={rulesText}
            onChange={(e) => setRulesText(e.target.value)}
            placeholder={"当前没有规则文件。点上方「恢复默认规则包」可写入一套出厂规则。"}
          />
        </div>
        <div className="actions">
          <button
            className="primary"
            onClick={() => void saveRules()}
            disabled={
              rulesBusy ||
              rulesText.trim().length === 0 ||
              Boolean(rules?.default_blocked_reason)
            }
            data-hover-hint={
              rules?.default_blocked_reason ??
              "校验通过后写入当前 Profile 的 rules.json，旧文件先备份"
            }
            data-testid="rules-save"
          >
            {rulesBusy ? "校验并保存中…" : "校验并保存"}
          </button>
        </div>
        {rulesNote ? <p className="hint">{rulesNote}</p> : null}
      </section>

      {/* ============ 3. 权限 ============ */}

      {/* ============ 能力管理（OD-ARCH-072）============ */}
      <section className="pane" hidden={tab !== "capabilities"}>
        <CardHead
          title="能力管理"
          status={
            caps
              ? `${caps.skills.length} 个 Skill · ${caps.plugins.filter((p) => p.enabled).length} 个启用的插件`
              : capsError
                ? "读取失败"
                : "读取中…"
          }
        >
          <button className="ghost" onClick={() => void refreshCaps()} disabled={capsBusy}>
            重新读取
          </button>
          <button className="ghost" onClick={() => void openSkillsDir()}>
            打开 Skill 目录
          </button>
        </CardHead>
        <p className="hint">
          Skill 是“怎么做事”的说明文件，平时可以直接编辑，改完重启应用生效；新增 Skill
          不在这里做，随 OmniDesk 整体升级发布。插件清单随 OmniDesk 版本固定，
          这里只能查看和启停现有插件，新增插件同样走整体升级。插件开关只决定下次启动要不要加载它，
          改完要<strong>重启应用</strong>才算数。
        </p>
        {capsError ? <div className="alert">能力清单：{capsError}</div> : null}
        {capsNote ? <p className="hint">{capsNote}</p> : null}
        {caps?.notes.length ? (
          <div className="alert">{caps.notes.join("；")}</div>
        ) : null}
        {caps?.missing_bindings.length ? (
          <div className="alert" data-testid="skill-missing-bindings">
            这些绑定找不到对应的 SKILL.md，因此注入不到内容：
            {caps.missing_bindings
              .map((m) => sceneLabel(m.scene) + " → " + m.skill)
              .join("；")}
            。把 Skill 放回目录，或从规则包的 skill_bindings 里去掉这一项。
          </div>
        ) : null}

        <div className="provider-split">
          <div className="provider-list" data-testid="skill-list">
            <div className="provider-list-head">
              <span>Skill（只读）</span>
              <span className="sub">{caps ? caps.skills.length : "—"}</span>
            </div>
            {caps && caps.skills.length === 0 ? (
              <p className="hint">这个实例还没有 Skill。目录：{caps.skills_dir}</p>
            ) : null}
            {caps ? <SkillScopeSummary caps={caps} /> : null}
            {caps?.skills.filter((s) => s.bindings.some((b) => b.effective)).length ? (
              <div className="skill-group" data-testid="skill-group-preinjected">
                <div className="skill-group-head">
                  <span>本 Profile 预注入</span>
                  <span className="sub">
                    {caps.skills.filter((s) => s.bindings.some((b) => b.effective)).length}
                  </span>
                </div>
                {caps.skills
                  .filter((s) => s.bindings.some((b) => b.effective))
                  .map((s) => (
                    <SkillRow key={s.path} skill={s} />
                  ))}
              </div>
            ) : null}
            {caps ? (
              <div className="skill-group" data-testid="skill-group-ondemand">
                <div className="skill-group-head">
                  <span>可被模型按需加载</span>
                  <span className="sub">
                    {caps.skills.filter((s) => !s.bindings.some((b) => b.effective)).length}
                  </span>
                </div>
                {caps.skills.filter((s) => !s.bindings.some((b) => b.effective)).length === 0 ? (
                  <p className="hint">没有未绑定的 Skill：磁盘上的 Skill 都在上面对应的场景里。</p>
                ) : null}
                {caps.skills
                  .filter((s) => !s.bindings.some((b) => b.effective))
                  .map((s) => (
                    <SkillRow key={s.path} skill={s} />
                  ))}
              </div>
            ) : null}
          </div>

          <div className="provider-editor">
            <div className="provider-list" data-testid="plugin-list">
              <div className="provider-list-head">
                <span>插件</span>
                <span className="sub">
                  {caps ? "配置：" + caps.manifest_path.split("/").slice(-3).join("/") : "—"}
                </span>
              </div>
              {caps?.plugins.map((p) => (
                <label className="check" key={p.name}>
                  <input
                    type="checkbox"
                    checked={p.enabled}
                    disabled={capsBusy || (!p.installed && !p.enabled)}
                    onChange={(e) => void togglePlugin(p.name, e.target.checked)}
                  />
                  <span>
                    <strong>{p.name}</strong>
                    {p.version ? <span className="sub"> · {p.version}</span> : null}
                    {p.builtin ? <span className="sub"> · 平台自带</span> : null}
                    {!p.installed ? (
                      <span className="sub"> · 未安装，暂不能启用</span>
                    ) : null}
                    <br />
                    <span className="sub">
                      {p.enabled ? "下次启动会加载" : "已停用（依赖记录还留着）"}
                    </span>
                  </span>
                </label>
              ))}
            </div>

            <div className="provider-list">
              <div className="provider-list-head">
                <span>新增插件与 Skill</span>
                <span className="sub">随版本升级</span>
              </div>
              <p className="hint" style={{ padding: "8px 12px" }}>
                这个版本内置的插件和 Skill 已经定好，界面不提供在线安装或新增入口。
                需要新能力时，通过升级整个 OmniDesk 版本获得，升级前会先在你的机器上验收。
              </p>
            </div>
          </div>
        </div>
      </section>

      <section className="pane" hidden={tab !== "permissions"}>
        <CardHead
          title="模型权限"
          status={
            facts
              ? facts.allowed
                ? "可以使用（已开启至少一类）"
                : "暂不能使用（三类都关着）"
              : "默认关闭"
          }
        >
          <button className="ghost" onClick={() => void revokeAll()} disabled={permissionBusy}>
            全部关闭
          </button>
        </CardHead>
        <p className="hint">
          这里只是“总开关”：哪一类没勾上，用这类模型的对话就发不出去。开关只管能不能用，
          不代表已经获得授权。地址只能告诉你模型在哪，<strong>不能</strong>证明它归谁所有；
          授权和数据去向需要你自己确认。
          位置不明的模型永远不会被放行。
        </p>

        {blocked ? (
          <div className="alert">
            权限变更未完成：未能确认旧引擎已停止（{blocked}）。为避免旧引擎继续按变更前的权限处理新输入，
            已<strong>暂停所有新的对话提交</strong>。
            <div className="actions">
              <button className="ghost" onClick={() => void stopEngine()} disabled={permissionBusy}>
                停止引擎，恢复使用
              </button>
              <button
                className="ghost"
                onClick={() => void refreshAdmission()}
                disabled={permissionBusy}
              >
                刷新状态
              </button>
            </div>
          </div>
        ) : null}

        {facts?.configurable
          ? PERMISSION_ITEMS.map((item) => (
              <label className="check" key={item.key}>
                <input
                  type="checkbox"
                  checked={facts.permissions[item.key]}
                  disabled={permissionBusy}
                  onChange={(e) => void changePermission(item.key, e.target.checked)}
                />
                <span>
                  <strong>{item.title}</strong>
                  <br />
                  {item.detail}
                </span>
              </label>
            ))
          : null}

        {permissionNote ? <p className="hint">{permissionNote}</p> : null}
        <p className="hint">
          勾选只表示“允许这类模型做普通任务”。案件材料、案件线索和其他禁入内容，无论开关怎么设都不能处理；
          这里写了什么也不等于你已获得授权，更不代表内容已经脱敏。
        </p>
      </section>

      {/* ============ 4. 关于：定时与提醒 ============ */}
      <section className="pane" hidden={tab !== "about"}>
        <CardHead title="提醒" status={cfg ? `心跳 ${heartbeatStatus ?? "—"}` : "读取中…"} />
        <p className="hint">
          心跳只在应用运行期间触发，关掉应用就不会执行；改动会影响之后的运行行为。
        </p>

        <details className="tech" open>
          <summary>心跳与规则提醒</summary>
          <p className="hint">
            心跳只在<strong>应用运行期间</strong>触发：关掉应用就不会执行，错过的时间点会在下次打开时补跑一次。
            它只做本地规则扫描（逾期 / 超期）来刷新场景页的规则提醒，不调模型、不写文件、不花 token。
            OD-ARCH-092 起自动复盘已从界面移除：工作记录现在是「随手记直接落盘，明确说整理成周报才生成」，
            不再需要另一条定时调用模型的路径。
          </p>
          {schedError ? <div className="alert">无法读取调度配置：{schedError}</div> : null}
          {cfg ? (
            <>
              <label className="check">
                <input
                  type="checkbox"
                  checked={cfg.heartbeat_enabled}
                  disabled={schedBusy}
                  onChange={(e) =>
                    void patchScheduler({ heartbeat_enabled: e.target.checked })
                  }
                />
                <span>
                  <strong>启用心跳扫描</strong>
                  <br />
                  到点刷新公告栏的规则提醒（拖延 / 超期）。纯本地计算，不花 token、不写文件。
                </span>
              </label>

              <div className="field">
                <label className="field-label">心跳间隔（5–1440 分钟）</label>
                <input
                  className="text-input"
                  type="number"
                  min={5}
                  max={1440}
                  value={cfg.heartbeat_minutes}
                  disabled={schedBusy}
                  onChange={(e) => {
                    const minutes = Number(e.target.value);
                    if (Number.isInteger(minutes) && minutes >= 5 && minutes <= 1440) {
                      void patchScheduler({ heartbeat_minutes: minutes });
                    }
                  }}
                />
              </div>

              <div className="status-card">
                <div className="status-row">
                  <span className="k">心跳</span>
                  <span className="v mono">{heartbeatStatus ?? "—"}</span>
                </div>
                <div className="status-row">
                  <span className="k">下次判定</span>
                  <span className="v">{sched?.hint ?? "—"}</span>
                </div>
                <div className="status-row">
                  <span className="k">上次心跳</span>
                  <span className="v mono">
                    {cfg.last_heartbeat_at > 0
                      ? new Date(cfg.last_heartbeat_at * 1000).toLocaleString()
                      : "从未执行"}
                  </span>
                </div>
              </div>
            </>
          ) : null}
          {schedNote ? <p className="hint">{schedNote}</p> : null}
        </details>
      </section>

      {/* ============ 5. 成果与文档库 ============ */}
      <section className="pane" hidden={tab !== "library"}>
        <CardHead
          title="成果与文档库"
          status={info?.artifact_root ?? (infoError ? "目录不可用" : "读取中…")}
        >
          <button className="ghost" onClick={() => void openArtifacts()}>
            打开目录
          </button>
          <button className="ghost" onClick={() => void refresh()}>
            刷新状态
          </button>
        </CardHead>
        <p className="hint">
          成果是 OmniDesk 自己生成的文稿，保存在仓库之外的本地目录里，可以直接用 Obsidian 打开。
          这里改的是<strong>之后新建成果</strong>的保存位置。
        </p>

        <details className="tech" open>
          <summary>成果目录</summary>
          <p className="hint">
            这里只影响<strong>之后新建的成果</strong>保存到哪。已有文件不会移动、覆盖或删除。
            模型或定时任务发起的写入一律先登记为待确认记录，只有你确认后才落盘。
          </p>
          {infoError ? <div className="alert">当前成果目录不可用：{infoError}</div> : null}
          {info ? (
            <div className="status-card">
              <div className="status-row">
                <span className="k">当前生效</span>
                <span className="v mono">{info.artifact_root ?? "不可用"}</span>
              </div>
              <div className="status-row">
                <span className="k">来源</span>
                <span className="v">
                  {info.source ? SOURCE_LABEL[info.source] ?? info.source : "未知"}
                </span>
              </div>
              {info.configured_root ? (
                <div className="status-row">
                  <span className="k">已保存的选择</span>
                  <span className="v mono">{info.configured_root}</span>
                </div>
              ) : null}
            </div>
          ) : null}
          {info?.source === "env_override" ? (
            <div className="alert">
              当前是开发/测试隔离实例：成果目录被环境变量覆盖为
              {info.env_override ?? "（未知）"}，此处保存的选择要等隔离变量取消后才会生效。
            </div>
          ) : null}
          <div className="field">
            <label className="field-label">改为新的绝对路径（例如 /home/you/Documents/OmniDesk）</label>
            <input
              className="text-input"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder="/home/you/Documents/OmniDesk"
            />
            <p className="hint">
              会被拒绝：仓库内、原版 DSH / agent / Codex 的私有目录、应用自身数据目录、非绝对路径，
              以及指向这些位置的软链接。保存前会在目标目录做一次写入探针并立即清理。
            </p>
          </div>
          <div className="actions">
            <button
              className="primary"
              onClick={() => void saveArtifactRoot()}
              disabled={busy || input.trim().length === 0}
            >
              {busy ? "校验并保存中…" : "校验并保存"}
            </button>
            <button className="ghost" onClick={() => void openArtifacts()}>
              打开当前目录
            </button>
            <button className="ghost" onClick={() => void refresh()}>
              刷新状态
            </button>
          </div>
          {message ? <p className="hint">{message}</p> : null}
          {error ? <div className="alert">{error}</div> : null}
          <div className="status-card">
            <div className="status-row">
              <span className="k">作为 Obsidian vault</span>
              <span className="v">
                在 Obsidian 里选「打开文件夹作为 vault」，指向上面「当前生效」的目录即可。
                新成果带标准 frontmatter（scenario / title / created / tags），Dataview 可直接查；
                已有成果保持原样，不会补写或迁移。
              </span>
            </div>
            <div className="status-row">
              <span className="k">边界</span>
              <span className="v">
                OmniDesk 只写自己生成的成果文件，不读取也不改动你在该 vault 里的其他笔记。
                你在 Obsidian 端的整理、改名、删除，OmniDesk 不感知、不干预。
                Obsidian 本体不由 OmniDesk 安装、检测或启动。
              </span>
            </div>
          </div>
          <p className="hint">
            「含下级目录」是按场景分别设置的开关，在各场景页的成果档案栏里切换，默认只看最外层。
          </p>
        </details>

        <details className="tech">
          <summary>库根（工作记录、周报与 @ 引用）</summary>
          <p className="hint">
            库根是工作记录、周报与 <code>@</code> 引用的共同根，默认
            <span className="mono"> {libraryInfo?.default_root ?? "~/OmniDesk/库"}</span>。
            这里改的是<strong>之后的 @ 引用</strong>指向哪里；技能文件仍写入数据根的
            <span className="mono"> 库/</span> 目录。知识库使用下方单独的
            <strong>「知识库（只读）」</strong>根，与本库根互不影响。
          </p>
          {libraryError ? <div className="alert">当前库根不可用：{libraryError}</div> : null}
          {libraryInfo && libraryInfo.resolved ? (
            <div
              className="status-card"
              role="group"
              aria-label={`库根状态：当前生效 ${libraryInfo.library_root ?? "不可用"}；来源 ${
                libraryInfo.source
                  ? SOURCE_LABEL[libraryInfo.source] ?? libraryInfo.source
                  : "未知"
              }；已保存的选择 ${libraryInfo.configured_root ?? "未设置（使用默认）"}`}
            >
              <div className="status-row">
                <span className="k">当前生效</span>
                <span className="v mono">{libraryInfo.library_root ?? "不可用"}</span>
              </div>
              <div className="status-row">
                <span className="k">来源</span>
                <span className="v">
                  {libraryInfo.source
                    ? SOURCE_LABEL[libraryInfo.source] ?? libraryInfo.source
                    : "未知"}
                </span>
              </div>
              <div className="status-row">
                <span className="k">已保存的选择</span>
                <span className="v mono">{libraryInfo.configured_root ?? "未设置（使用默认）"}</span>
              </div>
            </div>
          ) : null}
          {libraryInfo?.source === "env_override" ? (
            <div className="alert">
              库根被环境变量 OMNIDESK_KNOWLEDGE_ROOT 覆盖为
              {libraryInfo.env_override ?? "（未知）"}，此处保存的选择要等该变量取消后才会生效。
            </div>
          ) : null}
          <div className="field">
            <label className="field-label">改为新的绝对路径（例如 /home/you/OmniDesk/库）</label>
            <input
              className="text-input"
              value={libraryInput}
              onChange={(e) => setLibraryInput(e.target.value)}
              placeholder="/home/you/OmniDesk/库"
              aria-label="库根绝对路径"
            />
            <p className="hint">
              会被拒绝：仓库内、原版 DSH / agent / Codex 的私有目录、应用自身数据目录、非绝对路径，
              以及指向这些位置的软链接。保存前会在目标目录做一次写入探针并立即清理。
              旧路径下的文件不迁移、不覆盖、不删除。
            </p>
          </div>
          <div className="actions">
            <button
              className="primary"
              onClick={() => void saveLibraryRoot()}
              disabled={libraryBusy || libraryInput.trim().length === 0}
            >
              {libraryBusy ? "校验并保存中…" : "校验并保存"}
            </button>
            <button className="ghost" onClick={() => void refreshLibrary()}>
              刷新状态
            </button>
          </div>
          {libraryMessage ? (
            <p className="hint" role="status" aria-live="polite">
              {libraryMessage}
            </p>
          ) : null}
          {librarySaveError ? <div className="alert">{librarySaveError}</div> : null}
          <p className="hint">
            环境变量 <span className="mono">OMNIDESK_KNOWLEDGE_ROOT</span> 优先级高于这里的设置，
            用于开发/测试隔离；取消该变量后，这里的保存选择才会生效。
          </p>
        </details>

        <details className="tech">
          <summary>知识库（只读）</summary>
          <p className="hint">
            这里选一个<strong>已有的</strong> Obsidian 或 Markdown 目录作为知识库。OmniDesk
            只列出文件和读取笔记正文，不写入、不移动、不删除，也不改动
            <span className="mono"> .obsidian/</span> 配置。知识库根独立于上面的库根，
            两边互不迁移、互不覆盖。
          </p>
          <div
            className="status-card"
            role="group"
            aria-label={vaultAria}
          >
            <div className="status-row">
              <span className="k">当前知识库</span>
              <span className="v mono">
                {vaultStatus?.configured ? vaultStatus.root ?? "不可用" : "未设置"}
              </span>
            </div>
            <div className="status-row">
              <span className="k">目录状态</span>
              <span className="v">
                {vaultStatus === null
                  ? "读取中…"
                  : !vaultStatus.configured
                    ? "尚未选择目录"
                    : vaultStatus.error
                      ? `不可用：${vaultStatus.error}`
                      : vaultStatus.is_dir
                        ? "可读取"
                        : "不是目录"}
              </span>
            </div>
            {vaultScan ? (
              <>
                <div className="status-row">
                  <span className="k">扫描结果</span>
                  <span className="v">
                    {vaultScan.stats.files} 个文件（笔记 {vaultScan.stats.notes}、附件{" "}
                    {vaultScan.stats.attachments}），共{" "}
                    {(vaultScan.stats.total_bytes / 1024).toFixed(1)} KiB
                  </span>
                </div>
                <div className="status-row">
                  <span className="k">跳过 / 错误</span>
                  <span className="v">
                    跳过 {vaultSkipped}{" "}
                    项，错误 {vaultScan.stats.errors} 项
                    {vaultScan.stats.truncated ? "（达到扫描上限，结果不完整）" : ""}
                  </span>
                </div>
              </>
            ) : null}
          </div>
          <div className="actions">
            <button
              className="primary"
              onClick={() => void pickVaultDirectory()}
              disabled={vaultBusy}
            >
              {vaultBusy ? "处理中…" : "选择知识库目录"}
            </button>
            <button
              className="ghost"
              onClick={() => void scanVault()}
              disabled={vaultBusy || !vaultStatus?.configured || !!vaultStatus?.error}
            >
              扫描刷新
            </button>
            <button
              className="ghost"
              onClick={() => void clearVault()}
              disabled={vaultBusy || !vaultStatus?.configured}
            >
              取消选择
            </button>
          </div>
          {vaultMessage ? (
            <p className="hint" role="status" aria-live="polite">
              {vaultMessage}
            </p>
          ) : null}
          {vaultError ? <div className="alert">{vaultError}</div> : null}
          <p className="hint">
            只支持 <span className="mono">.md</span> / <span className="mono">.markdown</span> /{" "}
            <span className="mono">.txt</span> 正文；其他文件只列名称和大小。隐藏目录、
            <span className="mono"> .obsidian/</span>、<span className="mono">node_modules</span>{" "}
            等整棵跳过，符号链接不跟随。取消选择只清除设置，知识库内文件保持原样。
          </p>
        </details>
      </section>

      {/* ============ 6. 关于：数据与日志 ============ */}
      <section className="pane" hidden={tab !== "about"}>
        <CardHead title="数据与日志" status="默认折叠" />
        <p className="hint">
          以下路径都在仓库之外。应用自身数据、成果文件与使用日志分开存放。
        </p>

        <details className="tech" open>
          <summary>数据与日志</summary>
          <p className="hint">
            以下路径都在仓库之外。应用自身数据、成果文件与使用日志分开存放；
            待确认写入记录含待写正文，24 小时后自动失效。
          </p>
          {info ? (
            <div className="status-card">
              <div className="status-row">
                <span className="k">应用数据根</span>
                <span className="v mono">{info.app_data_root}</span>
              </div>
              <div className="status-row">
                <span className="k">设置文件</span>
                <span className="v mono">{info.settings_path}</span>
              </div>
              <div className="status-row">
                <span className="k">使用日志</span>
                <span className="v mono">{info.usage_log}</span>
              </div>
              <div className="status-row">
                <span className="k">调度配置</span>
                <span className="v mono">
                  {info.app_data_root.replace(/\/$/, "")}/config/scheduler.v1.json
                </span>
              </div>
              <div className="status-row">
                <span className="k">待确认写入</span>
                <span className="v mono">
                  {info.app_data_root.replace(/\/$/, "")}/engine-state/pending-writes/
                </span>
              </div>
            </div>
          ) : null}
          <div className="actions">
            <button className="ghost" onClick={() => setPendingKey((n) => n + 1)}>
              刷新待确认写入
            </button>
          </div>
          <div className="pending-list">
            <PendingWritePanel refreshKey={pendingKey} />
          </div>
        </details>
      </section>

      {/* ============ 7. 引擎诊断（只读，自引擎页迁入） ============ */}
      <section className="pane" hidden={tab !== "engine"}>
        <CardHead title="引擎诊断" status="只读" />
        <p className="hint">
          这里只读取引擎状态，不能发起任务，也不会把任何文本送给模型。诊断信息用于排查
          "任务没跑起来"这类问题；日常使用不需要打开。
        </p>
        {/* OD-ARCH-075：当前运行通道是启动参数决定的只读事实，界面不提供切换。 */}
        <details className="tech" open>
          <summary>当前通道与能力差异</summary>
          {channelView ? (
            <>
              <p>
                当前通道：
                <strong className="mono">{channelView.channel}</strong>
                {"（由启动参数 OMNIDESK_ENGINE_CHANNEL 决定，界面不能切换）"}
              </p>
              <ul className="hint">
                <li>
                  <strong>Web API</strong>：支持在界面里逐条审批工具调用，也支持取消；取消后引擎会回报任务
                  是否真的终止。
                </li>
                <li>
                  <strong>ACP</strong>：当前不支持在界面里审批工具调用；发出取消只是请求，不代表底层已经确认
                  停止，任务可能仍在运行。
                </li>
              </ul>
              {channelError ? (
                <div className="alert">引擎配置读取有误：{channelError}</div>
              ) : null}
            </>
          ) : channelError ? (
            <div className="alert">
              读不到当前通道：{channelError}
              <div className="actions">
                <button className="ghost" onClick={loadChannel}>
                  重新读取
                </button>
              </div>
            </div>
          ) : (
            <p className="hint">正在读取…</p>
          )}
        </details>
        <EngineTechSlice embedded />
      </section>
    </div>
  );
}
