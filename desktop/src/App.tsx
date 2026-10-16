import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { host, onTaskEvent } from "./api";
import Workbench from "./workbench";
import {
  TERMINAL_STATES,
  type CancelOutcome,
  type EngineDescriptor,
  type EngineHealth,
  type SubmitTaskReceipt,
  type TaskEvent,
  type TaskResult,
  type TaskState,
  type TaskStatus,
} from "./types";

/** 取消阶段的中文文案。刻意区分「已请求」与「已确认」，不得把前者写成取消成功。 */
const CANCEL_PHASE_LABEL: Record<string, string> = {
  not_requested: "未请求取消",
  request_sent: "取消请求已发送（尚未确认）",
  acknowledged: "引擎已收到取消请求（非最终确认）",
  confirmed_cancelled: "取消已确认",
  timeout_unknown: "取消结果未知（超时）",
};

const STATE_LABEL: Record<TaskState, string> = {
  queued: "排队中",
  starting: "启动中",
  running: "执行中",
  cancelling: "取消中",
  succeeded: "已完成",
  failed: "已失败",
  cancelled: "已取消",
  interrupted: "已中断",
  unknown: "状态未知",
};

function formatTime(seconds: number): string {
  if (!seconds) return "";
  return new Date(seconds * 1000).toLocaleTimeString("zh-CN", { hour12: false });
}

export default function App() {
  // 默认进入任务工作台（本阶段唯一已实现的纵向流程）；
  // 「引擎技术切片」保留原有的提交/事件/结果面板，用于排查底座问题。
  const [view, setView] = useState<"workspace" | "engine">("workspace");
  const [promptText, setPromptText] = useState("");
  const [descriptor, setDescriptor] = useState<EngineDescriptor | null>(null);
  const [health, setHealth] = useState<EngineHealth | null>(null);
  const [descriptorError, setDescriptorError] = useState<string | null>(null);

  const [taskId, setTaskId] = useState<string | null>(null);
  const [status, setStatus] = useState<TaskStatus | null>(null);
  const [events, setEvents] = useState<TaskEvent[]>([]);
  const [result, setResult] = useState<TaskResult | null>(null);
  const [cancel, setCancel] = useState<CancelOutcome | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scrollRef = useRef<HTMLDivElement | null>(null);

  // 启动时读取引擎描述与健康状态（只读，不提交任务）。
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [d, h] = await Promise.all([host.describe(), host.health()]);
        if (!cancelled) {
          setDescriptor(d);
          setHealth(h);
        }
      } catch (e) {
        if (!cancelled) setDescriptorError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 订阅宿主转发的事件，只保留当前任务的事件。
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let closed = false;
    onTaskEvent((event) => {
      setEvents((prev) => {
        if (prev.some((e) => e.seq === event.seq)) return prev;
        return [...prev, event].sort((a, b) => a.seq - b.seq);
      });
    }).then((fn) => {
      if (closed) fn();
      else unlisten = fn;
    });
    return () => {
      closed = true;
      if (unlisten) unlisten();
    };
  }, []);

  // 任务进入终态后刷新一次状态快照与最终结果。
  useEffect(() => {
    const current = status?.status;
    if (!taskId || !current || !TERMINAL_STATES.includes(current)) return;
    let stop = false;
    (async () => {
      try {
        const fresh = await host.taskStatus(taskId);
        if (!stop) setStatus(fresh);
      } catch {
        /* 状态刷新失败不改写已有状态 */
      }
    })();
    return () => {
      stop = true;
    };
  }, [taskId, status?.status]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [events.length]);

  const refreshStatus = useCallback(async (id: string) => {
    try {
      const fresh = await host.taskStatus(id);
      setStatus(fresh);
      return fresh;
    } catch {
      return null;
    }
  }, []);

  const handleSubmit = useCallback(async () => {
    const text = promptText.trim();
    if (!text) {
      setError("请先输入任务内容。");
      return;
    }
    setBusy(true);
    setError(null);
    setEvents([]);
    setResult(null);
    setCancel(null);
    setStatus(null);
    try {
      const receipt: SubmitTaskReceipt = await host.submitTask({ promptText: text });
      setTaskId(receipt.omni_task_id);
      setStatus({
        omni_task_id: receipt.omni_task_id,
        status: receipt.status,
        state_since: receipt.accepted_at,
        cancel_phase: "not_requested",
        note: receipt.note,
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [promptText]);

  const handleCancel = useCallback(async () => {
    if (!taskId) return;
    setError(null);
    try {
      const outcome = await host.cancelTask(taskId);
      setCancel(outcome);
      // 取消请求发出后立刻回读真实状态，不自行推断终态。
      await refreshStatus(taskId);
    } catch (e) {
      setError(String(e));
    }
  }, [taskId, refreshStatus]);

  const handleFetchResult = useCallback(async () => {
    if (!taskId) return;
    setError(null);
    try {
      setResult(await host.taskResult(taskId));
    } catch (e) {
      setError(String(e));
    }
  }, [taskId]);

  const streamedText = useMemo(
    () =>
      events
        .filter((e) => e.type === "text_delta")
        .map((e) => String((e.payload as { text?: unknown }).text ?? ""))
        .join(""),
    [events],
  );

  const currentState = status?.status ?? null;
  const canCancel =
    !!taskId && !!currentState && !TERMINAL_STATES.includes(currentState);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <img className="brand-mark" src="/omnidesk-monomark.svg" alt="" />
          <div className="brand-text">
            <span className="brand-name">OmniDesk</span>
            <span className="brand-sub">Discipline · 个人自用 MVP</span>
          </div>
        </div>
        <nav className="view-tabs">
          <button
            className={view === "workspace" ? "tab active" : "tab"}
            onClick={() => setView("workspace")}
          >
            任务工作台
          </button>
          <button
            className={view === "engine" ? "tab active" : "tab"}
            onClick={() => setView("engine")}
          >
            引擎技术切片
          </button>
        </nav>
        <div className="engine-chip" title={health?.app_data_root ?? ""}>
          {descriptor ? (
            <>
              <span className={descriptor.healthy ? "dot ok" : "dot bad"} />
              <span className="mono">{descriptor.engine_kind}</span>
              <span className="sep">·</span>
              <span className="mono">{descriptor.engine_version}</span>
              <span className="sep">·</span>
              <span className="mono">{descriptor.compatibility}</span>
            </>
          ) : (
            <>
              <span className="dot bad" />
              <span>{descriptorError ? "引擎未就绪" : "正在检测引擎…"}</span>
            </>
          )}
        </div>
      </header>

      {view === "workspace" ? (
        <Workbench />
      ) : (
      <main className="layout">
        <section className="pane input-pane">
          <h2 className="pane-title">任务</h2>
          <textarea
            className="prompt"
            value={promptText}
            onChange={(e) => setPromptText(e.target.value)}
            placeholder="输入一个纯合成任务。禁止输入案件材料、线索或未公开调查信息。"
            rows={10}
          />
          <div className="actions">
            <button
              className="primary"
              onClick={handleSubmit}
              disabled={busy || !promptText.trim()}
            >
              {busy ? "提交中…" : "提交任务"}
            </button>
            <button className="ghost" onClick={handleCancel} disabled={!canCancel}>
              取消任务
            </button>
            <button
              className="ghost"
              onClick={handleFetchResult}
              disabled={!taskId}
            >
              读取结果
            </button>
          </div>

          <div className="status-card">
            <div className="status-row">
              <span className="k">任务 ID</span>
              <span className="v mono">{taskId ?? "—"}</span>
            </div>
            <div className="status-row">
              <span className="k">状态</span>
              <span className="v">
                {currentState ? (
                  <>
                    <span className={"pill " + currentState}>
                      {STATE_LABEL[currentState]}
                    </span>
                    {status?.note ? (
                      <span className="hint">{status.note}</span>
                    ) : null}
                  </>
                ) : (
                  "—"
                )}
              </span>
            </div>
            <div className="status-row">
              <span className="k">取消</span>
              <span className="v">
                {cancel
                  ? CANCEL_PHASE_LABEL[cancel.phase] ?? cancel.phase
                  : CANCEL_PHASE_LABEL[status?.cancel_phase ?? "not_requested"]}
                {cancel?.note ? <span className="hint">{cancel.note}</span> : null}
              </span>
            </div>
          </div>

          {error ? <div className="alert">{error}</div> : null}

          <details className="tech">
            <summary>引擎与应用路径</summary>
            <dl>
              <dt>应用数据根</dt>
              <dd className="mono">{health?.app_data_root ?? "—"}</dd>
              <dt>DSH 数据根</dt>
              <dd className="mono">{health?.dsh_home ?? "—"}</dd>
              <dt>Engine PID</dt>
              <dd className="mono">{health?.engine_pid ?? "—"}</dd>
            </dl>
          </details>
        </section>

        <section className="pane stream-pane">
          <h2 className="pane-title">
            事件流
            <span className="count">{events.length} 条</span>
          </h2>
          <div className="stream" ref={scrollRef}>
            {events.length === 0 ? (
              <div className="empty">
                尚未提交任务。事件到达后会按序号显示在这里。
              </div>
            ) : (
              events.map((e) => (
                <div className="event" key={e.seq}>
                  <span className="seq mono">#{e.seq}</span>
                  <span className={"etype " + e.type}>{e.type}</span>
                  <span className="etime mono">{formatTime(e.occurred_at)}</span>
                  <pre className="epayload">
                    {JSON.stringify(e.payload, null, 2)}
                  </pre>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="pane result-pane">
          <h2 className="pane-title">输出</h2>
          {streamedText ? (
            <div className="block">
              <h3>流式文本</h3>
              <pre className="text">{streamedText}</pre>
            </div>
          ) : null}
          {result ? (
            <div className="block">
              <h3>
                任务结果
                <span className="pill small">{result.form}</span>
                <span className="pill small">{result.integrity}</span>
              </h3>
              {result.persisted_at ? (
                <p className="hint">
                  落盘时间 {formatTime(result.persisted_at)}
                </p>
              ) : (
                <p className="hint">未落盘（仅进程内结果）</p>
              )}
              {result.content_omitted ? (
                <p className="hint">
                  正文较长（{result.content_length} 字符），已按路径返回，未内联。
                </p>
              ) : (
                <pre className="text">{result.content || "（空）"}</pre>
              )}
            </div>
          ) : (
            <div className="empty">点击「读取结果」获取已终态任务的完整输出。</div>
          )}
        </section>
      </main>
      )}
    </div>
  );
}
