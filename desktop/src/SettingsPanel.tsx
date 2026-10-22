// 设置（OD-ARCH-061 S2）：唯一设置入口，按功能分页：
// 模型与端点 / 对话分类 / 成果与文档库 / 权限 / 引擎诊断 / 关于（高级）。
//
// 只负责展示与调用宿主命令；路径校验、协议校验、凭据写入、原子落盘都在 Rust 侧完成。
// 这里不拼接 shell、不读写文件，也不推断“设置已生效”——一律以宿主返回为准。
// 密钥是单向输入：保存后立刻清空输入框，界面只显示“已配置 / 来源”。

import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  host,
  invalidateEngineFactsCache,
  onDshSettingsClosed,
  scheduler as schedulerApi,
  shell,
} from "./api";
import { PendingWritePanel } from "./shell/PendingWriteCard";
import { EngineTechSlice } from "./shell/EngineTechSlice";
import { refreshThreadCategories } from "./threadCategories";
import type {
  AdmissionFacts,
  CategoryInitReceipt,
  LocalRootInfo,
  ModelCategory,
  ModelPermissions,
  ModelProbeReceipt,
  ModelSettings,
  ModelSettingsView,
  ProfileDeclPayload,
  SchedulerCadence,
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
type SettingsTab = "model" | "categories" | "library" | "permissions" | "engine" | "about";

const SETTINGS_TABS: Array<{ key: SettingsTab; label: string }> = [
  { key: "model", label: "模型与端点" },
  { key: "categories", label: "对话分类" },
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

const CADENCE_LABEL: Record<SchedulerCadence, string> = {
  daily: "每天",
  weekly: "每周一",
  once: "仅一次",
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
    title: "本机模型（localhost / 127.0.0.1 回环地址）",
    detail: "可就地判定：内容不经过本机以外的地址。仍只用于允许范围内的材料。",
  },
  {
    key: "private",
    title: "非回环地址 · 私有 / 链路本地网段",
    detail:
      "该端点不是本机回环。地址范围只能说明它不是公网地址；无法据此判断它属于哪个组织、是否在你的单位网络内，也无法确认实际转发路径。请自行确认发布方的授权与数据去向后再开启。",
  },
  {
    key: "public",
    title: "非回环地址 · 公网",
    detail:
      "该端点不是本机回环，内容会发往本机以外的地址；是否离开你所在的组织无法从地址判断（也可能经过单位内代理）。请自行确认发布方的授权与数据去向后再开启。",
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
        {status ? <p className="card-status mono">{status}</p> : null}
      </div>
      {children ? <div className="actions card-head-actions">{children}</div> : null}
    </div>
  );
}

/** 新建配置时的空白表单（不预填任何真实端点或密钥）。 */
function blankSettings(): ModelSettings {
  return {
    schema: "omnidesk.model-settings.v1",
    provider: "",
    display_name: "",
    base_url: "",
    api: "openai-completions",
    default_model: "",
    api_key_ref: "",
    reasoning_effort: "default",
  };
}

export default function SettingsPanel() {
  // —— 分页（OD-ARCH-061 S2）：唯一设置入口，一页一件事 ——
  const [tab, setTab] = useState<SettingsTab>("model");

  // —— 成果目录 ——
  const [info, setInfo] = useState<LocalRootInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  // —— DSH 原生界面（诊断用，默认折叠）——
  const [dshNote, setDshNote] = useState<string | null>(null);
  const [dshError, setDshError] = useState<string | null>(null);
  const [dshBusy, setDshBusy] = useState(false);

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
        setForm({ ...r.settings, reasoning_effort: r.settings.reasoning_effort ?? "default" });
      } else {
        setForm(blankSettings());
      }
      setModelError(r.ok ? null : r.error ?? "读取模型配置失败");
    } catch (e) {
      setView(null);
      setModelError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const refreshDecl = useCallback(async () => {
    try {
      const r = await shell.profileDecl();
      setDecl(r);
      setDeclText(r.text);
      setDeclError(null);
    } catch (e) {
      setDecl(null);
      setDeclError(e instanceof Error ? e.message : String(e));
    }
  }, []);

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
    void refreshAdmission();
    void refreshScheduler();
    void refreshModel();
    void refreshDecl();
  }, [refresh, refreshAdmission, refreshScheduler, refreshModel, refreshDecl]);

  // 诊断用的 DSH 窗口关闭后重读端点事实（窗口只可能从「关于」里打开）。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void onDshSettingsClosed(() => {
      invalidateEngineFactsCache();
      void refreshAdmission();
      void refreshModel();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshAdmission, refreshModel]);

  // 对话分类只在打开这一页时读一次，避免与其他分页的读盘互相干扰。
  useEffect(() => {
    if (tab === "categories" && categories === null) void refreshCategories();
  }, [tab, categories, refreshCategories]);

  const saveModel = useCallback(async () => {
    setModelBusy(true);
    setModelNote(null);
    setModelError(null);
    try {
      const r = await shell.modelSettingsSave({
        settings: form,
        apiKey: apiKey.trim().length > 0 ? apiKey : null,
      });
      // 密钥只在这一次请求里出现过：无论成败都立刻从界面状态里清掉。
      setApiKey("");
      setModelNote(
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
  }, [form, apiKey, refreshModel, refreshAdmission]);

  const probeConnection = useCallback(async () => {
    setProbeBusy(true);
    setProbe(null);
    setProbeError(null);
    try {
      const r = await shell.modelSettingsProbe({
        baseUrl: form.base_url,
        api: form.api,
        // 输入框里的 key 只用于这一次探测；空则沿用 DSH 已登记的凭据。
        apiKey: apiKey.trim().length > 0 ? apiKey : null,
      });
      setProbe(r);
    } catch (e) {
      setProbeError(e instanceof Error ? e.message : String(e));
    } finally {
      setProbeBusy(false);
    }
  }, [form.base_url, form.api, apiKey]);

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

  const openDshSettings = useCallback(async () => {
    setDshBusy(true);
    setDshError(null);
    setDshNote(null);
    try {
      const r = await shell.openDshSettings();
      setDshNote(
        r.note +
          "\n实例端口：" +
          r.port +
          (r.reused ? "（复用已开的实例）" : "（本次新起）") +
          "\n启动日志：" +
          r.log_path,
      );
    } catch (e) {
      setDshError(String(e));
    } finally {
      setDshBusy(false);
    }
  }, []);

  const cfg = sched?.config ?? null;
  const reviewStatus = cfg
    ? cfg.review_enabled
      ? "每" + CADENCE_LABEL[cfg.cadence] + " " + String(cfg.hour) + ":00"
      : "关闭"
    : null;
  const heartbeatStatus = cfg
    ? cfg.heartbeat_enabled
      ? "每 " + String(cfg.heartbeat_minutes) + " 分钟"
      : "关闭"
    : null;
  const keyStatus = view?.key_status ?? null;
  const savedOverlay = view?.overlay_present === true;

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

        <div className="field">
          <label className="field-label">服务商标识（provider，也是配置里的键名）</label>
          <input
            className="text-input"
            value={form.provider}
            spellCheck={false}
            placeholder="例如 deepseek"
            onChange={(e) => setForm({ ...form, provider: e.target.value })}
          />
        </div>

        <div className="field">
          <label className="field-label">显示名称（你自己看的名字）</label>
          <input
            className="text-input"
            value={form.display_name}
            spellCheck={false}
            placeholder="例如 DeepSeek 官方"
            onChange={(e) => setForm({ ...form, display_name: e.target.value })}
          />
        </div>

        <div className="field">
          <label className="field-label">接口地址（base_url，必须是 http/https 绝对地址）</label>
          <input
            className="text-input mono"
            value={form.base_url}
            spellCheck={false}
            placeholder="https://api.example.com/v1"
            onChange={(e) => setForm({ ...form, base_url: e.target.value })}
          />
          <p className="hint">
            不接受把密钥写进地址（user:pass@ 或 URL 查询串）；密钥请填在下面的输入框。
          </p>
        </div>

        <div className="field">
          <label className="field-label">接口协议</label>
          <select
            className="text-input"
            value={form.api}
            onChange={(e) => setForm({ ...form, api: e.target.value })}
          >
            {API_CHOICES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </div>

        <div className="field">
          <label className="field-label">默认模型（模型 id）</label>
          <input
            className="text-input mono"
            value={form.default_model}
            spellCheck={false}
            placeholder="例如 deepseek-v4.1-flash"
            onChange={(e) => setForm({ ...form, default_model: e.target.value })}
          />
          {probe && probe.models.length > 0 ? (
            <>
              <select
                className="text-input"
                value={probe.models.some((m) => m.id === form.default_model) ? form.default_model : ""}
                onChange={(e) => {
                  if (e.target.value) setForm({ ...form, default_model: e.target.value });
                }}
              >
                <option value="">从端点返回的 {probe.models.length} 个模型里选一个…</option>
                {probe.models.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name === m.id ? m.id : m.name + "（" + m.id + "）"}
                  </option>
                ))}
              </select>
              <p className="hint">上面的输入框仍然可以直接手填；这个列表只是端点自己报的候选。</p>
            </>
          ) : null}
        </div>

        <div className="field">
          <label className="field-label">思考等级（可选）</label>
          <select
            className="text-input"
            value={form.reasoning_effort ?? "default"}
            onChange={(e) => setForm({ ...form, reasoning_effort: e.target.value })}
          >
            {EFFORT_CHOICES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </div>

        <div className="field">
          <label className="field-label">凭据名（DSH 配置里的 apiKeyEnv，非机密）</label>
          <input
            className="text-input mono"
            value={form.api_key_ref}
            spellCheck={false}
            placeholder="例如 DEEPSEEK_API_KEY"
            onChange={(e) => setForm({ ...form, api_key_ref: e.target.value })}
          />
          <p className="hint">
            DSH 的模型配置只存这个引用名，真正的密钥由 DSH 凭据服务保管。
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
            placeholder={
              keyStatus?.configured ? "已配置（留空表示不改动现有密钥）" : "尚未配置"
            }
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
                    ? // OD-ARCH-056：Rust 侧在「没有已存配置」时返回 key_status=null 且无
                      // key_error，属正常的「未配置」。此前这里落到「读取中…」会永久不收敛。
                      "尚未配置"
                    : "读取中…"}
            </span>
          </div>
        </div>

        <div className="actions">
          <button className="primary" onClick={() => void saveModel()} disabled={modelBusy}>
            {modelBusy ? "保存中…" : "保存模型配置"}
          </button>
          <button
            className="ghost"
            onClick={() => void probeConnection()}
            disabled={probeBusy || modelBusy || form.base_url.trim().length === 0}
            title="用当前填写的地址与协议，让 DSH 实际请求一次端点"
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

      {/* ============ 2. 对话分类 ============ */}
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
                : "读取中…"
          }
        >
          <button className="ghost" onClick={() => void openDecl()} disabled={declBusy}>
            在系统编辑器打开
          </button>
          <button className="ghost" onClick={() => void refreshDecl()} disabled={declBusy}>
            重新读取
          </button>
        </CardHead>
        <p className="hint">
          这份声明决定左侧 Dock 显示哪些业务场景，以及工作台叫什么名字。它只影响界面入口，
          不改插件、路由或权限。保存前会先校验；校验不过不会写文件，旧文件在写入前自动备份。
          <strong>保存后需重启应用生效。</strong>
        </p>
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
        ) : null}
        <div className="field">
          <label className="field-label">声明内容（YAML）</label>
          <textarea
            className="text-input mono"
            rows={14}
            spellCheck={false}
            value={declText}
            onChange={(e) => setDeclText(e.target.value)}
          />
        </div>
        <div className="actions">
          <button
            className="primary"
            onClick={() => void saveDecl()}
            disabled={declBusy || declText.trim().length === 0}
          >
            {declBusy ? "校验并保存中…" : "校验并保存"}
          </button>
        </div>
        {declNote ? <p className="hint">{declNote}</p> : null}
      </section>

      {/* ============ 3. 权限 ============ */}
      <section className="pane" hidden={tab !== "permissions"}>
        <CardHead
          title="模型权限"
          status={
            facts
              ? facts.allowed
                ? "当前允许提交（至少一类已开启）"
                : "当前拒绝提交（三类均关闭）"
              : "默认全部关闭"
          }
        >
          <button className="ghost" onClick={() => void revokeAll()} disabled={permissionBusy}>
            撤销全部模型权限
          </button>
        </CardHead>
        <p className="hint">
          下面的事实由宿主从实际模型配置读出。只有你在这里开启对应类别后，使用模型的任务才会被放行。
          无法确认位置的端点永远不会被放行。类别只能表达“是不是本机回环”，地址范围
          <strong>不能</strong>证明端点属于哪个组织；请自行确认发布方的授权与数据去向。
        </p>

        {blocked ? (
          <div className="alert">
            权限变更未完成：未能确认旧引擎已停止（{blocked}）。为避免旧引擎继续按变更前的权限处理新输入，
            宿主已<strong>拒绝一切新的模型提交</strong>。
            <div className="actions">
              <button className="ghost" onClick={() => void stopEngine()} disabled={permissionBusy}>
                停止引擎以恢复
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
          开启某一类只代表你允许该位置的模型处理普通任务，不改变案件、线索和其他禁入材料的边界；
          界面上的说明不构成授权，也不代表内容已经脱敏。
        </p>
      </section>

      {/* ============ 4. 关于：定时与提醒 ============ */}
      <section className="pane" hidden={tab !== "about"}>
        <CardHead title="定时与提醒" status={cfg ? `复盘 ${reviewStatus ?? "—"} · 心跳 ${heartbeatStatus ?? "—"}` : "读取中…"} />
        <p className="hint">
          定时与心跳只在应用运行期间触发，关掉应用就不会执行；改动会影响之后的运行行为。
        </p>

        <details className="tech" open>
          <summary>定时与提醒</summary>
          <p className="hint">
            定时与心跳只在<strong>应用运行期间</strong>触发：关掉应用就不会执行，错过的时间点会在下次打开时补跑一次。
            心跳只做本地规则扫描（逾期 / 超期），不调模型、不写文件。AI 复盘会调用一次模型，产出先登记为待确认写入，
            <strong>你点确认之后才会存成文件</strong>。
          </p>
          {schedError ? <div className="alert">无法读取调度配置：{schedError}</div> : null}
          {cfg ? (
            <>
              <label className="check">
                <input
                  type="checkbox"
                  checked={cfg.review_enabled}
                  disabled={schedBusy}
                  onChange={(e) => void patchScheduler({ review_enabled: e.target.checked })}
                />
                <span>
                  <strong>启用定时 AI 复盘</strong>
                  <br />
                  默认关闭。开启后按频率调用一次模型；模型不可用或权限关闭时不执行、不产模板。
                </span>
              </label>

              <div className="field">
                <label className="field-label">复盘频率</label>
                <select
                  className="text-input"
                  value={cfg.cadence}
                  disabled={schedBusy}
                  onChange={(e) =>
                    void patchScheduler({ cadence: e.target.value as SchedulerCadence })
                  }
                >
                  <option value="daily">每天</option>
                  <option value="weekly">每周一</option>
                  <option value="once">仅一次（到点后不再重复）</option>
                </select>
              </div>

              <div className="field">
                <label className="field-label">触发小时（本地时间，0–23）</label>
                <input
                  className="text-input"
                  type="number"
                  min={0}
                  max={23}
                  value={cfg.hour}
                  disabled={schedBusy}
                  onChange={(e) => {
                    const hour = Number(e.target.value);
                    if (Number.isInteger(hour) && hour >= 0 && hour <= 23) {
                      void patchScheduler({ hour });
                    }
                  }}
                />
              </div>

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
                  <span className="k">复盘 / 心跳</span>
                  <span className="v mono">
                    {reviewStatus ?? "—"} · {heartbeatStatus ?? "—"}
                  </span>
                </div>
                <div className="status-row">
                  <span className="k">下次判定</span>
                  <span className="v">{sched?.hint ?? "—"}</span>
                </div>
                <div className="status-row">
                  <span className="k">上次复盘</span>
                  <span className="v mono">
                    {cfg.last_review_at > 0
                      ? new Date(cfg.last_review_at * 1000).toLocaleString()
                      : "从未执行"}
                  </span>
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
        <EngineTechSlice embedded />
        <details className="tech" open>
          <summary>DSH 原生界面（诊断用）</summary>
          <p className="hint">
            这里会打开<strong>完整的 DSH Web 应用</strong>，不是只打开设置页。日常配置模型请用本页上方的表单；
            只有排查 DSH 自身问题、需要看 DSH 原生会话或插件状态时才用到它。
          </p>
          <div className="actions">
            <button className="ghost" onClick={() => void openDshSettings()} disabled={dshBusy}>
              {dshBusy ? "正在打开 DSH…" : "打开 DSH 原生界面（诊断用）"}
            </button>
          </div>
          {dshNote ? <p className="hint mono">{dshNote}</p> : null}
          {dshError ? <div className="alert">打不开 DSH：{dshError}</div> : null}
        </details>
      </section>
    </div>
  );
}
