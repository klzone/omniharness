// 设置：新成果保存目录。
//
// 只负责展示与调用宿主命令；路径校验、探针、持久化都在 Rust 侧完成。
// 这里不拼接 shell，不读写文件，也不推断“设置已生效”——一律以宿主返回为准。

import { useCallback, useEffect, useState } from "react";
import { host, shell } from "./api";
import type { AdmissionFacts, LocalRootInfo, ModelCategory, ModelPermissions } from "./types";

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

export default function SettingsPanel() {
  const [info, setInfo] = useState<LocalRootInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);
  const [factsError, setFactsError] = useState<string | null>(null);
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [permissionNote, setPermissionNote] = useState<string | null>(null);
  // OD-ARCH-022：权限变更未确认停止旧引擎时，宿主会置位屏障并拒绝新提交。
  const [blocked, setBlocked] = useState<string | null>(null);

  const refreshAdmission = useCallback(async () => {
    try {
      const r = await host.admissionStatus();
      setFacts(r.facts);
      setBlocked(r.blocked ? (r.block_reason ?? "原因未提供") : null);
      setFactsError(
        r.ok
          ? null
          : r.error?.message ?? r.error?.engine_error_code ?? "无法读取端点信息",
      );
    } catch (e) {
      setFacts(null);
      setFactsError(String(e));
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

  useEffect(() => {
    void refresh();
    void refreshAdmission();
  }, [refresh, refreshAdmission]);

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
          "修改未生效：" + String(e) + "（宿主已拒绝新的模型提交，请用下方恢复动作处理。）",
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
        "撤销未生效：" + String(e) + "（宿主已拒绝新的模型提交，请用下方恢复动作处理。）",
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
      setMessage(`${r.note} 当前目录：${r.artifact_root}`);
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
      setMessage(`已在系统文件管理器中打开：${r.opened}`);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return (
    <div className="workbench ofl-v2 settings-v2">
      <section className="pane wb-header">
        <h2 className="pane-title">
          设置
          <span className="count">本地</span>
        </h2>
        <p className="hint">
          这里只影响<strong>之后新建的成果</strong>保存到哪。已有文件不会移动、覆盖或删除；应用自身的配置和数据目录与成果目录分开存放。
        </p>
      </section>

      <section className="pane">
        <h2 className="pane-title">新成果保存目录</h2>
        {infoError ? <div className="alert">当前成果目录不可用：{infoError}</div> : null}
        {info ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">当前生效</span>
              <span className="v mono">{info.artifact_root ?? "不可用"}</span>
            </div>
            <div className="status-row">
              <span className="k">来源</span>
              <span className="v">{info.source ? SOURCE_LABEL[info.source] ?? info.source : "未知"}</span>
            </div>
            {info.configured_root ? (
              <div className="status-row">
                <span className="k">已保存的选择</span>
                <span className="v mono">{info.configured_root}</span>
              </div>
            ) : null}
            <div className="status-row">
              <span className="k">设置文件</span>
              <span className="v mono">{info.settings_path}</span>
            </div>
            <div className="status-row">
              <span className="k">应用数据根</span>
              <span className="v mono">{info.app_data_root}</span>
            </div>
          </div>
        ) : null}

        {info?.source === "env_override" ? (
          <div className="alert">
            当前是开发/测试隔离实例：成果目录被环境变量覆盖为 {info.env_override ?? "（未知）"}，此处保存的选择要等隔离变量取消后才会成为实际生效目录。
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
            会被拒绝：仓库内、原版 DSH / agent / Codex 的私有目录、应用自身数据目录、非绝对路径，以及指向这些位置的软链接。保存前会在目标目录做一次写入探针并立即清理。
          </p>
        </div>
        <div className="actions">
          <button className="primary" onClick={() => void save()} disabled={busy || input.trim().length === 0}>
            {busy ? "校验并保存中…" : "校验并保存"}
          </button>
          <button className="ghost" onClick={() => void open()}>
            打开当前目录
          </button>
          <button className="ghost" onClick={() => void refresh()}>
            刷新状态
          </button>
        </div>
        {message ? <p className="hint">{message}</p> : null}
        {error ? <div className="alert">{error}</div> : null}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          模型权限
          <span className="count">默认全部关闭</span>
        </h2>
        <p className="hint">
          下面的事实由宿主从实际模型配置读出。只有你在这里开启对应类别后，
          使用模型的任务才会被放行。无法确认位置的端点永远不会被放行。
        </p>
        <p className="hint">
          <strong>端点不在这里配。</strong>
          实际端点来自 DSH 侧的模型配置（Profile 与补丁合成后的结果）；本应用只读取**非机密**的
          provider / model / 地址用于判定，<strong>不保存端点、也不读取凭证值</strong>。
          要换端点就改 DSH 侧那份模型配置，改完回到本页点「刷新端点信息」即可看到新的判定。
        </p>
        <p className="hint">
          说明：类别只能表达“是不是本机回环”。地址范围**不能**证明端点属于哪个组织，
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
              <button className="ghost" onClick={() => void refreshAdmission()} disabled={permissionBusy}>
                刷新状态
              </button>
            </div>
          </div>
        ) : null}

        {factsError ? <div className="alert">无法读取端点：{factsError}</div> : null}

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

        {facts && !facts.configurable ? (
          <div className="alert">
            当前端点位置无法确认，宿主会一律拒绝此类端点，且没有可开启的开关。
            请先把 DSH 侧的模型配置指向一个明确的地址，再回到本页点「刷新端点信息」。
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

        <div className="actions">
          <button className="ghost" onClick={() => void revokeAll()} disabled={permissionBusy}>
            撤销全部模型权限
          </button>
          <button className="ghost" onClick={() => void refreshAdmission()}>
            刷新端点信息
          </button>
        </div>
        {permissionNote ? <p className="hint">{permissionNote}</p> : null}
        <p className="hint">
          开启某一类只代表你允许该位置的模型处理普通任务，不改变案件、线索和其他禁入材料的边界；
          界面上的说明不构成授权，也不代表内容已经脱敏。
        </p>
      </section>
    </div>
  );
}
