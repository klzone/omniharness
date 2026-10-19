// 轻量使用记录：只记枚举与粗耗时，不记输入、输出、标题、路径或会话。
//
// 记录默认不写；用户点“记录一次”才追加。记录保存在仓库外的 OmniDesk 数据根，
// 可以在这里查看摘要、逐条删除或整体清除。本组件不参与任何计算，也不调用模型。

import { useCallback, useEffect, useState } from "react";
import { shell } from "./api";
import type { OfflineScenario, UsageRecord } from "./types";

const SCENARIO_LABEL: Record<OfflineScenario, string> = {
  weekly_report: "周记录 → 周报",
  material_writing: "一般文案结构",
  project_management: "项目跟进",
};

const REWORK_LABEL: Record<string, string> = {
  none: "没有返工",
  structure: "调整结构",
  facts: "补充或核对事实",
  wording: "修改措辞",
  format: "调整格式",
  requirements_misread: "需求理解偏差",
};

const BLOCKER_LABEL: Record<string, string> = {
  none: "没有障碍",
  engine_unavailable: "引擎不可用",
  credentials_missing: "凭证缺失",
  input_unclear: "输入不清楚",
  template_missing: "缺少合适模板",
  other_technical: "其他技术问题",
};

const DURATION_LABEL: Record<string, string> = {
  under_5m: "5 分钟以内",
  "5_15m": "5–15 分钟",
  "15_30m": "15–30 分钟",
  "30_60m": "30–60 分钟",
  over_60m: "超过 60 分钟",
};

const REWORK_OPTIONS = Object.keys(REWORK_LABEL);
const BLOCKER_OPTIONS = Object.keys(BLOCKER_LABEL);
const DURATION_OPTIONS = Object.keys(DURATION_LABEL);

function formatTime(secs: number): string {
  const d = new Date(secs * 1000);
  return Number.isNaN(d.getTime()) ? "—" : d.toLocaleString();
}

export default function UsageLog({ scenario }: { scenario: OfflineScenario }) {
  const [records, setRecords] = useState<UsageRecord[]>([]);
  const [path, setPath] = useState<string | null>(null);
  const [unparsable, setUnparsable] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [recorded, setRecorded] = useState(false);

  const [completed, setCompleted] = useState(true);
  const [usable, setUsable] = useState(true);
  const [rework, setRework] = useState("none");
  const [blocker, setBlocker] = useState("none");
  const [duration, setDuration] = useState("");
  const [reuseWillingness, setReuseWillingness] = useState(false);
  const [expanded, setExpanded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const view = await shell.usageView();
      setRecords(view.records);
      setPath(view.path);
      setUnparsable(view.unparsable_lines);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const record = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await shell.recordUsage({
        taskType: scenario,
        completed,
        usable,
        reworkKind: rework,
        blocker,
        durationBucket: duration || null,
        reuseWillingness,
      });
      setRecorded(true);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [scenario, completed, usable, rework, blocker, duration, reuseWillingness, refresh]);

  const remove = useCallback(
    async (id: string) => {
      setBusy(true);
      try {
        await shell.usageDelete(id);
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const clear = useCallback(async () => {
    setBusy(true);
    try {
      await shell.usageClear();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [refresh]);

  return (
    <section className="pane">
      <h2 className="pane-title">
        使用记录（可选）
        <span className="count">不含输入输出正文</span>
      </h2>
      <p className="hint">
        只记录场景、是否完成、是否可用、返工类别、障碍类别、可选粗耗时和是否愿意再用；不记录文字内容、标题、路径或会话。
        不点“记录一次”就不会写入。
      </p>
      <p className="hint mono">记录文件：{path ?? "—"}</p>
      <div className="record-form">
        <label className="check">
          <input type="checkbox" checked={completed} onChange={(e) => setCompleted(e.target.checked)} />
          <span>本次流程完成</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={usable} onChange={(e) => setUsable(e.target.checked)} />
          <span>结果可以直接用</span>
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={reuseWillingness}
            onChange={(e) => setReuseWillingness(e.target.checked)}
          />
          <span>类似任务还愿意再用</span>
        </label>
        <label className="field">
          <span className="field-label">返工类别</span>
          <select className="select" value={rework} onChange={(e) => setRework(e.target.value)}>
            {REWORK_OPTIONS.map((key) => (
              <option key={key} value={key}>
                {REWORK_LABEL[key]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">障碍</span>
          <select className="select" value={blocker} onChange={(e) => setBlocker(e.target.value)}>
            {BLOCKER_OPTIONS.map((key) => (
              <option key={key} value={key}>
                {BLOCKER_LABEL[key]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">耗时（可选粗略区间）</span>
          <select className="select" value={duration} onChange={(e) => setDuration(e.target.value)}>
            <option value="">不填写</option>
            {DURATION_OPTIONS.map((key) => (
              <option key={key} value={key}>
                {DURATION_LABEL[key]}
              </option>
            ))}
          </select>
        </label>
        <div className="actions">
          <button className="ghost" onClick={() => void record()} disabled={busy}>
            记录一次
          </button>
          <button className="ghost" onClick={() => setExpanded((v) => !v)} disabled={busy}>
            {expanded ? "收起记录" : `查看记录（${records.length}）`}
          </button>
        </div>
        {recorded ? <span className="hint">已记录。</span> : null}
      </div>

      {error ? <div className="alert">{error}</div> : null}
      {unparsable > 0 ? <p className="hint">有 {unparsable} 行无法解析，已跳过。</p> : null}
      {expanded ? (
        <>
          <div className="actions">
            <button className="ghost" onClick={() => void refresh()} disabled={busy}>
              刷新
            </button>
            <button className="ghost" onClick={() => void clear()} disabled={busy || records.length === 0}>
              清空全部记录
            </button>
          </div>
          {records.length === 0 ? (
            <p className="hint">还没有记录。</p>
          ) : (
            <table className="stat-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>场景</th>
                  <th>完成</th>
                  <th>可用</th>
                  <th>返工</th>
                  <th>障碍</th>
                  <th>耗时</th>
                  <th>愿意再用</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {records.map((r) => (
                  <tr key={r.record_id}>
                    <td className="mono">{formatTime(r.recorded_at)}</td>
                    <td>{SCENARIO_LABEL[r.task_type as OfflineScenario] ?? r.task_type}</td>
                    <td>{r.completed ? "是" : "否"}</td>
                    <td>{r.usable ? "是" : "否"}</td>
                    <td>{REWORK_LABEL[r.rework_kind] ?? r.rework_kind}</td>
                    <td>{BLOCKER_LABEL[r.blocker] ?? r.blocker}</td>
                    <td>
                      {r.duration_bucket
                        ? DURATION_LABEL[r.duration_bucket] ?? r.duration_bucket
                        : "—"}
                    </td>
                    <td>{r.reuse_willingness ? "是" : "否"}</td>
                    <td>
                      <button className="ghost tiny" onClick={() => void remove(r.record_id)} disabled={busy}>
                        删除
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      ) : null}
    </section>
  );
}
