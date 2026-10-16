// OmniDesk 任务工作台（唯一已实现的纵向试点：脱敏数据分析与生态研判）。
//
// 五段固定骨架：输入 → 确定性统计 → 解释 → 人工确认 → 保存到仓库外本地目录。
//
// 硬规矩（本文件只是 UI，真正强制在 desktop/src-tauri/src/task_shell.rs）：
//   1. 数值只来自 task_run_analysis，UI 不做任何统计或补齐；
//   2. 保存前必须人工勾选确认，并如实选择解释来源；
//   3. 使用记录只记枚举与布尔，不记输入输出正文。

import { useCallback, useEffect, useMemo, useState } from "react";
import { shell } from "./api";
import type {
  AnalysisResult,
  ExplanationSource,
  LocalRootInfo,
  SampleInfo,
  SaveArtifactReceipt,
  TaskTypeCatalog,
  UsageRecord,
} from "./types";

/** 与 Rust 侧 fmt_num 保持一致：整数不带小数点，其余最多 6 位小数并去尾零。 */
function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return "NaN";
  if (Number.isInteger(value)) return String(value);
  return value
    .toFixed(6)
    .replace(/0+$/, "")
    .replace(/\.$/, "");
}

const SOURCE_LABEL: Record<ExplanationSource, string> = {
  user: "手写（user）",
  model_unverified: "模型输出 · 未复核（model_unverified）",
};

const REWORK_LABEL: Record<string, string> = {
  none: "无需返工",
  structure: "结构问题",
  facts: "事实/数据问题",
  wording: "表述问题",
  format: "格式问题",
  requirements_misread: "要求理解偏差",
};

const BLOCKER_LABEL: Record<string, string> = {
  none: "无",
  engine_unavailable: "引擎不可用",
  credentials_missing: "模型凭证缺失",
  input_unclear: "输入不清晰",
  template_missing: "缺少适配模板",
  other_technical: "其他技术问题",
};

const DURATION_LABEL: Record<string, string> = {
  under_5m: "5 分钟以内",
  "5_15m": "5–15 分钟",
  "15_30m": "15–30 分钟",
  "30_60m": "30–60 分钟",
  over_60m: "60 分钟以上",
};

const REWORK_OPTIONS = Object.keys(REWORK_LABEL);
const BLOCKER_OPTIONS = Object.keys(BLOCKER_LABEL);
const DURATION_OPTIONS = Object.keys(DURATION_LABEL);

function formatTime(seconds: number): string {
  if (!seconds) return "";
  return new Date(seconds * 1000).toLocaleString("zh-CN", { hour12: false });
}

export default function Workbench() {
  const [catalog, setCatalog] = useState<TaskTypeCatalog | null>(null);
  const [samples, setSamples] = useState<SampleInfo[]>([]);
  const [localRoot, setLocalRoot] = useState<LocalRootInfo | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const [taskType, setTaskType] = useState("desensitized_analysis");
  const [inputText, setInputText] = useState("");
  const [activeSample, setActiveSample] = useState<string | null>(null);

  const [analysis, setAnalysis] = useState<AnalysisResult | null>(null);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [redLineHits, setRedLineHits] = useState<
    Array<{ kind: string; count: number; label: string }>
  >([]);

  const [promptText, setPromptText] = useState<string | null>(null);
  const [promptError, setPromptError] = useState<string | null>(null);

  const [explanation, setExplanation] = useState("");
  const [source, setSource] = useState<ExplanationSource>("user");

  const [confirmed, setConfirmed] = useState(false);
  const [confirmNote, setConfirmNote] = useState("");

  const [receipt, setReceipt] = useState<SaveArtifactReceipt | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [usage, setUsage] = useState<UsageRecord[]>([]);
  const [usagePath, setUsagePath] = useState<string | null>(null);
  const [usageUnparsable, setUsageUnparsable] = useState(0);
  const [usageError, setUsageError] = useState<string | null>(null);
  const [usageBusy, setUsageBusy] = useState(false);
  const [recorded, setRecorded] = useState(false);

  const [recordCompleted, setRecordCompleted] = useState(true);
  const [recordUsable, setRecordUsable] = useState(true);
  const [recordRework, setRecordRework] = useState("none");
  const [recordBlocker, setRecordBlocker] = useState("none");
  const [recordDuration, setRecordDuration] = useState("");

  const refreshUsage = useCallback(async () => {
    try {
      const view = await shell.usageView();
      setUsage(view.records);
      setUsagePath(view.path);
      setUsageUnparsable(view.unparsable_lines);
      setUsageError(null);
    } catch (e) {
      setUsageError(String(e));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [types, sampleCatalog, root] = await Promise.all([
          shell.taskTypes(),
          shell.samples(),
          shell.localRootInfo(),
        ]);
        if (cancelled) return;
        setCatalog(types);
        setSamples(sampleCatalog.samples);
        setLocalRoot(root);
      } catch (e) {
        if (!cancelled) setBootError(String(e));
      }
    })();
    void refreshUsage();
    return () => {
      cancelled = true;
    };
  }, [refreshUsage]);

  /** 载入合成样例：同时清空下游产物，避免解释挂在旧统计上。 */
  const loadSample = useCallback((sample: SampleInfo) => {
    setInputText(sample.content);
    setActiveSample(sample.sample_id);
    setAnalysis(null);
    setAnalysisError(null);
    setRedLineHits([]);
    setPromptText(null);
    setPromptError(null);
    setExplanation("");
    setConfirmed(false);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
  }, []);

  const handleInputChange = useCallback((value: string) => {
    setInputText(value);
    setActiveSample(null);
    // 输入变了，之前算出的统计与提示词立即失效，防止落到过期指纹上。
    setAnalysis(null);
    setAnalysisError(null);
    setRedLineHits([]);
    setPromptText(null);
    setPromptError(null);
    setConfirmed(false);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
  }, []);

  const runAnalysis = useCallback(async () => {
    setAnalysisError(null);
    setReceipt(null);
    setSaveError(null);
    setRecorded(false);
    try {
      const response = await shell.runAnalysis(inputText);
      setRedLineHits(response.red_line_hits);
      if (response.ok && response.result) {
        setAnalysis(response.result);
        setConfirmed(false);
      } else {
        setAnalysis(null);
        setAnalysisError(response.error ?? "统计失败（未返回原因）");
      }
    } catch (e) {
      setAnalysis(null);
      setAnalysisError(String(e));
    }
  }, [inputText]);

  const buildPrompt = useCallback(async () => {
    if (!analysis) return;
    setPromptError(null);
    try {
      const response = await shell.explanationPrompt({
        taskType,
        inputText,
        fingerprint: analysis.fingerprint,
      });
      setPromptText(response.prompt);
    } catch (e) {
      setPromptText(null);
      setPromptError(String(e));
    }
  }, [analysis, taskType, inputText]);

  const handleSave = useCallback(async () => {
    if (!analysis) return;
    setSaveError(null);
    setReceipt(null);
    try {
      const saved = await shell.saveArtifact({
        taskType,
        inputText,
        explanationText: explanation,
        explanationSource: source,
        // 人工确认由界面的勾选显式给出；Rust 保存入口会再次强制校验该布尔值。
        // UI 无法自行证明任务在引擎里成功终态，因此不提供任何"已复核模型来源"选项。
        userConfirmed: confirmed,
        confirmationNote: confirmNote,
        fingerprint: analysis.fingerprint,
      });
      setReceipt(saved);
    } catch (e) {
      setSaveError(String(e));
    }
  }, [analysis, taskType, inputText, explanation, source, confirmed, confirmNote]);

  const recordUsage = useCallback(async () => {
    setUsageBusy(true);
    setUsageError(null);
    try {
      await shell.recordUsage({
        taskType,
        completed: recordCompleted,
        usable: recordUsable,
        reworkKind: recordRework,
        blocker: recordBlocker,
        durationBucket: recordDuration || null,
      });
      setRecorded(true);
      await refreshUsage();
    } catch (e) {
      setUsageError(String(e));
    } finally {
      setUsageBusy(false);
    }
  }, [
    taskType,
    recordCompleted,
    recordUsable,
    recordRework,
    recordBlocker,
    recordDuration,
    refreshUsage,
  ]);

  const deleteRecord = useCallback(
    async (recordId: string) => {
      setUsageBusy(true);
      try {
        await shell.usageDelete(recordId);
        await refreshUsage();
      } catch (e) {
        setUsageError(String(e));
      } finally {
        setUsageBusy(false);
      }
    },
    [refreshUsage],
  );

  const clearRecords = useCallback(async () => {
    setUsageBusy(true);
    try {
      await shell.usageClear();
      await refreshUsage();
    } catch (e) {
      setUsageError(String(e));
    } finally {
      setUsageBusy(false);
    }
  }, [refreshUsage]);

  const implementedTypes = useMemo(
    () => (catalog?.types ?? []).filter((t) => t.status === "implemented"),
    [catalog],
  );
  const reservedTypes = useMemo(
    () => (catalog?.types ?? []).filter((t) => t.status !== "implemented"),
    [catalog],
  );

  const canSave = !!analysis && confirmed && explanation.trim().length > 0;

  return (
    <div className="workbench">
      {bootError ? <div className="alert">加载任务外壳失败：{bootError}</div> : null}

      <section className="pane wb-header">
        <h2 className="pane-title">
          任务类型
          {catalog ? (
            <span className="count mono">{catalog.shell_version}</span>
          ) : null}
        </h2>
        <div className="type-row">
          <select
            className="select"
            value={taskType}
            onChange={(e) => setTaskType(e.target.value)}
          >
            {implementedTypes.map((t) => (
              <option key={t.task_type} value={t.task_type}>
                {t.title}
              </option>
            ))}
          </select>
          <span className="pill succeeded">已实现</span>
          {reservedTypes.length > 0 ? (
            <span className="hint">
              未实现（仅保留边界，不代表获得授权）：
              {reservedTypes.map((t) => t.title).join("、")}
            </span>
          ) : null}
        </div>
        <p className="hint">
          已实现类型的数据范围：仅合成 / 公开 / 经明确批准的数据。禁止案件材料、线索或未公开调查信息。
        </p>
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 1 · 输入
          <span className="count">合成样例或可处理数据</span>
        </h2>
        <div className="actions">
          {samples.map((sample) => (
            <button
              key={sample.sample_id}
              className={activeSample === sample.sample_id ? "primary" : "ghost"}
              onClick={() => loadSample(sample)}
              title={sample.note}
            >
              {sample.title}
            </button>
          ))}
        </div>
        <textarea
          className="prompt"
          value={inputText}
          onChange={(e) => handleInputChange(e.target.value)}
          placeholder={"类别,数值\n示例甲,10\n示例乙,20"}
          rows={8}
        />
        <div className="actions">
          <button className="primary" onClick={runAnalysis} disabled={!inputText.trim()}>
            运行确定性统计
          </button>
        </div>
        {redLineHits.length > 0 ? (
          <div className="alert">
            红线扫描命中（启发式，仅提示，不代表合规判断；不回显命中原文）：
            <ul>
              {redLineHits.map((hit) => (
                <li key={hit.kind}>
                  {hit.label} — {hit.count} 处
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 2 · 确定性统计
          <span className="count">本地代码计算，不调用模型</span>
        </h2>
        {analysisError ? <div className="alert">{analysisError}</div> : null}
        {analysis ? (
          <>
            <div className="status-card">
              <div className="status-row">
                <span className="k">行数</span>
                <span className="v mono">{analysis.rows}</span>
              </div>
              <div className="status-row">
                <span className="k">类别数</span>
                <span className="v mono">{analysis.distinct_categories}</span>
              </div>
              <div className="status-row">
                <span className="k">合计 / 平均</span>
                <span className="v mono">
                  {formatNumber(analysis.total)} / {formatNumber(analysis.mean)}
                </span>
              </div>
              <div className="status-row">
                <span className="k">最小 / 最大</span>
                <span className="v mono">
                  {formatNumber(analysis.min)} / {formatNumber(analysis.max)}
                </span>
              </div>
              <div className="status-row">
                <span className="k">最高类别</span>
                <span className="v mono">{analysis.top_category ?? "（无）"}</span>
              </div>
              <div className="status-row">
                <span className="k">指纹</span>
                <span className="v mono">{analysis.fingerprint}</span>
              </div>
            </div>
            <table className="stat-table">
              <thead>
                <tr>
                  <th>类别</th>
                  <th>条数</th>
                  <th>合计</th>
                  <th>平均</th>
                  <th>最小</th>
                  <th>最大</th>
                </tr>
              </thead>
              <tbody>
                {analysis.by_category.map((stat) => (
                  <tr key={stat.category}>
                    <td>{stat.category}</td>
                    <td className="mono">{stat.count}</td>
                    <td className="mono">{formatNumber(stat.sum)}</td>
                    <td className="mono">{formatNumber(stat.mean)}</td>
                    <td className="mono">{formatNumber(stat.min)}</td>
                    <td className="mono">{formatNumber(stat.max)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        ) : (
          <div className="empty">
            尚未计算。统计值只能由本地确定性代码产生，模型不参与计算。
          </div>
        )}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 3 · 解释
          <span className="count">只解释，不重算</span>
        </h2>
        <div className="actions">
          <button className="ghost" onClick={buildPrompt} disabled={!analysis}>
            生成解释提示词
          </button>
          {promptText ? (
            <button
              className="ghost"
              onClick={() => {
                void navigator.clipboard.writeText(promptText);
              }}
            >
              复制提示词
            </button>
          ) : null}
        </div>
        {promptError ? <div className="alert">{promptError}</div> : null}
        {promptText ? (
          <details className="tech">
            <summary>解释提示词（含冻结统计块，可直接交给模型）</summary>
            <pre className="text">{promptText}</pre>
          </details>
        ) : (
          <p className="hint">
            提示词里的统计块是冻结文本：模型只能解释，不得重算或补充数字。
            当前会话未配置模型凭证时，可自行把提示词交给已获准的模型，再把结果贴回下方。
          </p>
        )}
        <label className="field">
          <span className="field-label">解释来源</span>
          <select
            className="select"
            value={source}
            onChange={(e) => setSource(e.target.value as ExplanationSource)}
          >
            <option value="user">{SOURCE_LABEL.user}</option>
            <option value="model_unverified">{SOURCE_LABEL.model_unverified}</option>
          </select>
        </label>
        <textarea
          className="prompt"
          value={explanation}
          onChange={(e) => setExplanation(e.target.value)}
          placeholder="在此粘贴或撰写解释。不得引入统计块之外的数字。"
          rows={7}
        />
        <p className="hint">
          本阶段只提供「手写」与「模型输出 · 未复核」两种来源。可核验的模型成功来源尚未实现，
          因此不会出现任何"已复核模型"选项，避免把无法证明的状态写成事实。
        </p>
      </section>

      <section className="pane">
        <h2 className="pane-title">
          阶段 4 · 人工确认
          <span className="count">未确认不得保存</span>
        </h2>
        <label className="check">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          <span>
            我已人工复核解释与确定性统计一致，并确认本任务只使用合成 / 公开 / 经明确批准的数据。
          </span>
        </label>
        <label className="field">
          <span className="field-label">确认备注（可选）</span>
          <input
            className="text-input"
            value={confirmNote}
            onChange={(e) => setConfirmNote(e.target.value)}
            placeholder="例如：已核对合计与类别排序"
          />
        </label>

        <h2 className="pane-title">
          阶段 5 · 保存
          <span className="count">仓库外本地目录</span>
        </h2>
        <p className="hint mono">
          产物目录：{localRoot?.artifact_root ?? (localRoot?.error ?? "—")}
        </p>
        <div className="actions">
          <button className="primary" onClick={handleSave} disabled={!canSave}>
            保存产物
          </button>
        </div>
        {!canSave ? (
          <p className="hint">
            需先完成确定性统计、填写解释并勾选人工确认。
          </p>
        ) : null}
        {saveError ? <div className="alert">{saveError}</div> : null}
        {receipt ? (
          <div className="status-card">
            <div className="status-row">
              <span className="k">已保存</span>
              <span className="v mono">{receipt.path}</span>
            </div>
            <div className="status-row">
              <span className="k">字节</span>
              <span className="v mono">{receipt.bytes}</span>
            </div>
            <div className="status-row">
              <span className="k">写入时间</span>
              <span className="v mono">{formatTime(receipt.written_at)}</span>
            </div>
          </div>
        ) : null}
      </section>

      <section className="pane">
        <h2 className="pane-title">
          使用记录
          <span className="count">不含输入输出正文</span>
        </h2>
        <p className="hint mono">记录文件：{usagePath ?? "—"}</p>
        <div className="record-form">
          <label className="check">
            <input
              type="checkbox"
              checked={recordCompleted}
              onChange={(e) => setRecordCompleted(e.target.checked)}
            />
            <span>任务完成</span>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={recordUsable}
              onChange={(e) => setRecordUsable(e.target.checked)}
            />
            <span>结果可用</span>
          </label>
          <label className="field">
            <span className="field-label">返工类别</span>
            <select
              className="select"
              value={recordRework}
              onChange={(e) => setRecordRework(e.target.value)}
            >
              {REWORK_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {REWORK_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">障碍</span>
            <select
              className="select"
              value={recordBlocker}
              onChange={(e) => setRecordBlocker(e.target.value)}
            >
              {BLOCKER_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {BLOCKER_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">耗时（可选）</span>
            <select
              className="select"
              value={recordDuration}
              onChange={(e) => setRecordDuration(e.target.value)}
            >
              <option value="">不填写</option>
              {DURATION_OPTIONS.map((key) => (
                <option key={key} value={key}>
                  {DURATION_LABEL[key]}
                </option>
              ))}
            </select>
          </label>
          <button className="ghost" onClick={recordUsage} disabled={usageBusy}>
            记录一次使用
          </button>
          {recorded ? <span className="hint">已记录。</span> : null}
        </div>

        {usageError ? <div className="alert">{usageError}</div> : null}
        {usageUnparsable > 0 ? (
          <p className="hint">有 {usageUnparsable} 行无法解析，已跳过。</p>
        ) : null}
        <div className="actions">
          <button className="ghost" onClick={refreshUsage} disabled={usageBusy}>
            刷新
          </button>
          <button
            className="ghost"
            onClick={clearRecords}
            disabled={usageBusy || usage.length === 0}
          >
            清空全部记录
          </button>
        </div>
        {usage.length === 0 ? (
          <div className="empty">暂无使用记录。</div>
        ) : (
          <table className="stat-table">
            <thead>
              <tr>
                <th>时间</th>
                <th>完成</th>
                <th>可用</th>
                <th>返工</th>
                <th>障碍</th>
                <th>耗时</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {usage.map((record) => (
                <tr key={record.record_id}>
                  <td className="mono">{formatTime(record.recorded_at)}</td>
                  <td>{record.completed ? "是" : "否"}</td>
                  <td>{record.usable ? "是" : "否"}</td>
                  <td>{REWORK_LABEL[record.rework_kind] ?? record.rework_kind}</td>
                  <td>{BLOCKER_LABEL[record.blocker] ?? record.blocker}</td>
                  <td>
                    {record.duration_bucket
                      ? DURATION_LABEL[record.duration_bucket] ??
                        record.duration_bucket
                      : "—"}
                  </td>
                  <td>
                    <button
                      className="ghost tiny"
                      onClick={() => deleteRecord(record.record_id)}
                      disabled={usageBusy}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
