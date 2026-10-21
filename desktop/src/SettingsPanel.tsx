// 设置：模型与端点 / 模型权限 / 定时与提醒 / 成果目录 / 数据与日志。
//
// 只负责展示与调用宿主命令；路径校验、探针、权限判定、调度到点与持久化都在 Rust 侧完成。
// 这里不拼接 shell，不读写文件，也不推断“设置已生效”——一律以宿主返回为准。

import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  host,
  invalidateEngineFactsCache,
  onDshSettingsClosed,
  scheduler as schedulerApi,
  shell,
} from "./api";
import { PendingWritePanel } from "./shell/PendingWriteCard";
import type {
  AdmissionFacts,
  LocalRootInfo,
  ModelCategory,
  ModelPermissions,
  SchedulerCadence,
  SchedulerConfig,
  SchedulerSnapshot,
} from "./types";

const SOURCE_LABEL: Record<string, string> = {
  user: "你在设置中选择的目录",
  default: "默认目录",
  env_override: "开发/测试隔离变量覆盖（本条设置暂不生效）",
};

// 类别只区分“本机回环 / 非回环”，不承诺组织归属：地址特征无法证明端点属于
// 哪个组织，也无法证明数据是否离开所在组织。
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
      "该端点不是本机回环。地址范围只能说明它不是公网地址；**无法**据此判断它属于哪个组织、是否在你的单位网络内，也无法确认实际转发路径。请自行确认发布方的授权与数据去向后再开启。",
  },
  {
    key: "public",
    title: "非回环地址 · 公网",
    detail:
      "该端点不是本机回环，内容会发往本机以外的地址；是否离开你所在的组织无法从地址判断（也可能经过单位内代理）。请自行确认发布方的授权与数据去向后再开启。",
  },
];

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

export default function SettingsPanel() {
  // —— 成果目录 ——
  const [info, setInfo] = useState<LocalRootInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // —— 模型与端点 / 权限 ——
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);
  const [factsError, setFactsError] = useState<string | null>(null);
  /** OD-ARCH-050：区分「加载中」与「读不出来」。 */
  const [factsLoading, setFactsLoading] = useState(false);
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [permissionNote, setPermissionNote] = useState<string | null>(null);
  // OD-ARCH-022：权限变更未确认停止旧引擎时，宿主会置位屏障并拒绝新提交。
  const [blocked, setBlocked] = useState<string | null>(null);
  // OD-ARCH-031：跳转到 DSH 原有设置界面登记 key。固定动作，宿主不读写凭证。
  const [dshNote, setDshNote] = useState<string | null>(null);
  const [dshError, setDshError] = useState<string | null>(null);
  const [dshBusy, setDshBusy] = useState(false);

  // —— 定时与提醒（OD-ARCH-046）——
  const [sched, setSched] = useState<SchedulerSnapshot | null>(null);
  const [schedError, setSchedError] = useState<string | null>(null);
  const [schedBusy, setSchedBusy] = useState(false);
  const [schedNote, setSchedNote] = useState<string | null>(null);
  const [pendingKey, setPendingKey] = useState(0);

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

  const refreshAdmission = useCallback(async () => {
    setFactsLoading(true);
    try {
      const r = await host.admissionStatus();
      // OD-ARCH-050：读取失败时保留上一次已知事实，不把界面闪成「未识别」。
      if (r.facts) setFacts(r.facts);
      setBlocked(r.blocked ? (r.block_reason ?? "原因未提供") : null);
      setFactsError(
        r.ok
          ? null
          : r.error?.message ?? r.error?.engine_error_code ?? "无法读取端点信息",
      );
    } catch (e) {
      setFactsError(String(e));
    } finally {
      setFactsLoading(false);
    }
  }, []);

  /** 改为跟随 DSH 设置：归档 OmniDesk 覆盖文件并停引擎，下次任务用 DSH 的默认模型。 */
  const followDsh = useCallback(async () => {
    setDshBusy(true);
    setDshError(null);
    setDshNote(null);
    try {
      const r = await shell.clearModelRoute();
      setDshNote(r.note);
      invalidateEngineFactsCache();
      await refreshAdmission();
    } catch (e) {
      setDshError(String(e));
    } finally {
      setDshBusy(false);
    }
  }, [refreshAdmission]);

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

  useEffect(() => {
    void refresh();
    void refreshAdmission();
    void refreshScheduler();
  }, [refresh, refreshAdmission, refreshScheduler]);

  // OD-ARCH-050：设置窗口关闭后重新读端点事实——用户在 DSH 里改的模型要立刻可见。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void onDshSettingsClosed(() => {
      invalidateEngineFactsCache();
      void refreshAdmission();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshAdmission]);

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
        // 宿主在“未能确认旧引擎已停止”时失败关闭并置位屏障：如实显示，不写成成功。
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

  /** 恢复路径：显式停止引擎；确认停止后宿主自动解除屏障。 */
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

  const save = useCallback(async () => {
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

  const open = useCallback(async () => {
    setError(null);
    setMessage(null);
    try {
      const r = await shell.openArtifactRoot();
      setMessage("已在系统文件管理器中打开：" + r.opened);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /**
   * 调度配置改动：立即写宿主配置并回报真实结果。
   * 失败时重新读取宿主已保存的值，界面不保留未生效的假状态。
   */
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

  return (
    <div className="workbench ofl-v2 settings-v2">
      <section className="pane wb-header">
        <h2 className="pane-title">
          设置
          <span className="count">本地</span>
        </h2>
        <p className="hint">
          所有设置都只作用于本机的 OmniDesk 实例。成果文件不会因为改设置而移动、覆盖或删除；
          应用自身的配置数据与成果目录分开存放。
        </p>
      </section>

      {/* 1. 模型与端点 */}
      <section className="pane">
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
                  : "尚未读取端点信息"
          }
        >
          <button className="primary" onClick={() => void openDshSettings()} disabled={dshBusy}>
            {dshBusy ? "正在打开 DSH…" : "打开 DSH 设置"}
          </button>
          <button className="ghost" onClick={() => void followDsh()} disabled={dshBusy}>
            改为跟随 DSH 设置
          </button>
          <button
            className="ghost"
            onClick={() => void refreshAdmission()}
            disabled={permissionBusy}
          >
            刷新端点信息
          </button>
        </CardHead>
        <p className="hint">
          <strong>端点不在这里配。</strong>
          实际端点来自 DSH 侧的模型配置（Profile 与补丁合成后的结果）；本应用只读取
          <strong>非机密</strong>的 provider / model / 地址用于判定，<strong>不保存端点、也不读取凭证值</strong>。
          要换端点就改 DSH 侧那份模型配置，改完回到本页点「刷新端点信息」即可看到新的判定。
        </p>
        {dshNote ? <p className="hint mono">{dshNote}</p> : null}
        {dshError ? <div className="alert">打不开 DSH 设置：{dshError}</div> : null}
        {factsError ? (
          <div className="alert">
            无法读取端点：{factsError}
            {facts ? "（下面仍是上一次读到的结果，可点「刷新端点信息」重试）" : ""}
          </div>
        ) : null}
        {!facts && !factsError && factsLoading ? (
          <div className="hint">正在读取端点信息…（引擎启动期间也能读到，不需要重启应用）</div>
        ) : null}
        {!facts && !factsError && !factsLoading ? (
          <div className="alert">
            端点事实还没读出来。先点「刷新端点信息」；需要改 key 或换端点时，点「打开 DSH 设置」。
          </div>
        ) : null}
        {facts && !facts.configurable ? (
          <div className="alert">
            当前端点位置无法确认，宿主会一律拒绝此类端点，且没有可开启的开关。
            请先把 DSH 侧的模型配置指向一个明确的地址（可点「打开 DSH 设置」），
            再点「刷新端点信息」。
          </div>
        ) : null}
        {facts ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">模型</span>
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

      {/* 2. 模型权限 */}
      <section className="pane">
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
          下面的事实由宿主从实际模型配置读出。只有你在这里开启对应类别后，
          使用模型的任务才会被放行。无法确认位置的端点永远不会被放行。
          类别只能表达“是不是本机回环”，地址范围<strong>不能</strong>证明端点属于哪个组织，
          也不能证明数据是否离开你所在的组织；请自行确认发布方的授权与数据去向。
        </p>

        {blocked ? (
          <div className="alert">
            权限变更未完成：未能确认旧引擎已停止（{blocked}）。为避免旧引擎继续按变更前的权限
            处理新输入，宿主已<strong>拒绝一切新的模型提交</strong>。
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

      {/* 3. 定时与提醒 */}
      <section className="pane">
        <CardHead
          title="定时与提醒"
          status={
            cfg
              ? "AI 复盘" + (reviewStatus ?? "—") + " · 心跳" + (heartbeatStatus ?? "—")
              : schedError
                ? "调度配置读取失败"
                : "读取中…"
          }
        >
          <button className="ghost" onClick={() => void refreshScheduler()} disabled={schedBusy}>
            刷新调度配置
          </button>
        </CardHead>
        <p className="hint">
          定时与心跳只在<strong>应用运行期间</strong>触发：关掉应用就不会执行，错过的时间点会在下次打开时补跑一次。
          心跳只做本地规则扫描（逾期 / 超期），不调模型、不写文件。
          AI 复盘会调用一次模型，产出先登记为待确认写入，<strong>你点确认之后才会存成文件</strong>。
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
                默认关闭。开启后按下面的频率调用一次模型，生成本场景复盘建议；模型不可用或权限关闭时
                <strong>不执行、不产模板</strong>，只在公告栏说明原因并保留本地规则提醒。
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
                onChange={(e) => void patchScheduler({ heartbeat_enabled: e.target.checked })}
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
              <div className="status-row">
                <span className="k">配置文件</span>
                <span className="v mono">config/scheduler.v1.json</span>
              </div>
            </div>
          </>
        ) : null}
        {schedNote ? <p className="hint">{schedNote}</p> : null}
      </section>

      {/* 4. 成果目录 */}
      <section className="pane">
        <CardHead title="成果目录" status={info ? info.artifact_root ?? "不可用" : "读取中…"}>
          <button className="ghost" onClick={() => void open()}>
            打开当前目录
          </button>
          <button className="ghost" onClick={() => void refresh()}>
            刷新状态
          </button>
        </CardHead>
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
            {info.env_override ?? "（未知）"}，此处保存的选择要等隔离变量取消后才会成为实际生效目录。
          </div>
        ) : null}

        <div className="field">
          <label className="field-label">
            改为新的绝对路径（例如 /home/you/Documents/OmniDesk）
          </label>
          <input
            className="text-input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="/home/you/Documents/OmniDesk"
          />
          <p className="hint">
            会被拒绝：仓库内、原版 DSH / agent / Codex 的私有目录、应用自身数据目录、非绝对路径，以及指向这些位置的软链接。保存前会在目标目录做一次写入探针并立即清理。
          </p>
        </div>
        <div className="actions">
          <button
            className="primary"
            onClick={() => void save()}
            disabled={busy || input.trim().length === 0}
          >
            {busy ? "校验并保存中…" : "校验并保存"}
          </button>
        </div>
        {message ? <p className="hint">{message}</p> : null}
        {error ? <div className="alert">{error}</div> : null}
        <p className="hint">
          「含下级目录」是按场景分别设置的开关，在各场景页的成果档案栏里切换，默认只看最外层。
        </p>
      </section>

      {/* 5. 数据与日志 */}
      <section className="pane">
        <CardHead title="数据与日志" status={info ? info.app_data_root : "读取中…"}>
          <button className="ghost" onClick={() => setPendingKey((n) => n + 1)}>
            刷新待确认写入
          </button>
        </CardHead>
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
        <div className="pending-list">
          <PendingWritePanel refreshKey={pendingKey} />
        </div>
      </section>
    </div>
  );
}

