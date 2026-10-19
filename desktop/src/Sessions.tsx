// 会话与任务连续性视图（OD-ARCH-017）。
//
// 两条只读链路：
//   1. 引擎持久化会话清单（公开 ACP session/list，只取元数据）；
//   2. OmniDesk 自己的任务台账（只取元数据，主动丢弃正文）。
//
// 硬规矩：
//   * 不显示、不请求会话正文；
//   * “列表可见”不等于“可恢复”，重开结果以引擎实际返回为准；
//   * 重启后没有终态证据的任务显示 unknown/interrupted，不自动重放；
//   * 端点准入未确认时，宿主会拒绝启动引擎，这里如实显示原因。

import { useCallback, useEffect, useState } from "react";
import { host } from "./api";
import type {
  SessionListing,
  SessionSummary,
  TaskLedgerListing,
  TaskRecordSummary,
} from "./types";

const STATE_TEXT: Record<string, string> = {
  queued: "排队中",
  starting: "启动中",
  running: "执行中",
  cancelling: "取消中",
  succeeded: "已完成",
  failed: "已失败",
  cancelled: "已取消",
  interrupted: "已中断",
  unknown: "状态未知（未自动重放）",
};

export default function Sessions() {
  const [sessions, setSessions] = useState<SessionListing | null>(null);
  const [sessionsError, setSessionsError] = useState<string | null>(null);
  const [admission, setAdmission] = useState<{ checked: boolean; allowed: boolean; reason: string | null }>({
    checked: false,
    allowed: false,
    reason: null,
  });
  const [sessionsBusy, setSessionsBusy] = useState(false);

  const [ledger, setLedger] = useState<TaskLedgerListing | null>(null);
  const [ledgerError, setLedgerError] = useState<string | null>(null);
  const [ledgerBusy, setLedgerBusy] = useState(false);

  const [resumeNote, setResumeNote] = useState<Record<string, string>>({});
  const [recoverNote, setRecoverNote] = useState<Record<string, string>>({});

  const loadSessions = useCallback(async () => {
    setSessionsBusy(true);
    setSessionsError(null);
    try {
      setSessions(await host.listSessions());
    } catch (e) {
      setSessions(null);
      setSessionsError(String(e));
    } finally {
      setSessionsBusy(false);
    }
  }, []);

  const loadLedger = useCallback(async () => {
    setLedgerBusy(true);
    setLedgerError(null);
    try {
      setLedger(await host.taskRecords(100));
    } catch (e) {
      setLedger(null);
      setLedgerError(String(e));
    } finally {
      setLedgerBusy(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const r = await host.admissionStatus();
        if (cancelled) return;
        const reason = r.ok
          ? r.facts?.allowed
            ? null
            : "模型端点尚未在设置中开启权限。"
          : r.error?.message ?? r.error?.engine_error_code ?? "无法读取端点信息。";
        setAdmission({ checked: true, allowed: Boolean(r.ok && r.facts?.allowed), reason });
      } catch (e) {
        if (!cancelled) setAdmission({ checked: true, allowed: false, reason: String(e) });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!admission.allowed) return;
    void loadSessions();
    void loadLedger();
  }, [admission.allowed, loadSessions, loadLedger]);

  const reopen = useCallback(async (s: SessionSummary) => {
    setResumeNote((prev) => ({ ...prev, [s.engine_session_id]: "正在请求重开…" }));
    try {
      const r = await host.recoverSession(
        s.engine_session_id,
        s.workspace_token,
      );
      setResumeNote((prev) => ({
        ...prev,
        [s.engine_session_id]: r.resumed
          ? "已重开会话（" +
            (r.workspace_label || "当前工作区") +
            "）；重开不等于任务续跑，不会自动重放"
          : "引擎未确认重开，状态未知。",
      }));
    } catch (e) {
      setResumeNote((prev) => ({
        ...prev,
        [s.engine_session_id]: "重开失败：" + String(e),
      }));
    }
  }, []);

  const recover = useCallback(async (r: TaskRecordSummary) => {
    setRecoverNote((prev) => ({ ...prev, [r.omni_task_id]: "正在读取状态…" }));
    try {
      const out = await host.recoverTask(r.omni_task_id);
      setRecoverNote((prev) => ({
        ...prev,
        [r.omni_task_id]:
          (STATE_TEXT[out.status.status] ?? out.status.status) +
          "（证据：" +
          out.evidence +
          "；" +
          (out.replayed ? "发生了重放！" : "未自动重放") +
          "）" +
          (out.status.note ? " " + out.status.note : ""),
      }));
    } catch (e) {
      setRecoverNote((prev) => ({
        ...prev,
        [r.omni_task_id]: "读取失败：" + String(e),
      }));
    }
  }, []);

  const blocked = !admission.allowed;

  return (
    <div className="max-w-5xl mx-auto py-8 px-2 space-y-6">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold text-on-surface">会话与任务</h1>
        <p className="text-sm text-on-surface-variant leading-relaxed">
          这里只显示元数据：会话编号、所属工作区、任务状态。不会读取会话正文，
          也不会自动重放中断的任务。
        </p>
      </header>

      {blocked ? (
        <div className="rounded-2xl bg-surface-container-lowest/80 ring-1 ring-outline-variant/40 p-5 space-y-2">
          <p className="text-sm font-medium text-on-surface">
            {admission.checked ? "模型权限未开启，当前无法读取会话清单和任务台账。" : "正在读取模型端点信息…"}
          </p>
          <p className="text-xs text-on-surface-variant leading-relaxed">
            {admission.reason ?? ""}
            {admission.checked
              ? "读取会话清单需要启动引擎进程，宿主在执行前统一判定；这不是空清单，也不是“没有会话”。请到设置中确认端点并开启对应权限。"
              : ""}
          </p>
        </div>
      ) : null}

      <section className="rounded-2xl bg-surface-container-lowest/80 ring-1 ring-outline-variant/40 p-5 space-y-4">
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-base font-semibold text-on-surface">引擎会话</h2>
          <button
            className="text-xs px-3 py-1.5 rounded-full ring-1 ring-outline-variant/60 hover:bg-surface-container"
            onClick={() => void loadSessions()}
            disabled={blocked || sessionsBusy}
          >
            {sessionsBusy ? "读取中…" : "刷新清单"}
          </button>
        </div>
        {sessionsError ? (
          <p className="text-xs text-error">读取失败：{sessionsError}</p>
        ) : null}
        {sessions ? (
          <>
            <p className="text-xs text-on-surface-variant">
              共 {sessions.total} 条；无效记录 {sessions.skipped_invalid} 条。
              {sessions.note}
            </p>
            {sessions.groups.length === 0 ? (
              <p className="text-xs text-on-surface-variant">引擎没有返回任何会话。</p>
            ) : null}
            {sessions.groups.map((g) => (
              <div key={g.workspace_token} className="space-y-2">
                <p className="text-xs font-mono text-on-surface-variant break-all">
                  {g.workspace_label || "工作区"}
                </p>
                <ul className="space-y-2">
                  {g.sessions.map((s) => (
                    <li
                      key={s.engine_session_id}
                      className="flex flex-wrap items-center gap-2 text-xs"
                    >
                      <span className="font-mono break-all">{s.engine_session_id}</span>
                      <span className="text-on-surface-variant">
                        {s.metadata_valid ? "记录完整" : "记录无效"} · 恢复状态未验证
                      </span>
                      <button
                        className="px-2 py-1 rounded-full ring-1 ring-outline-variant/60 hover:bg-surface-container"
                        onClick={() => void reopen(s)}
                      >
                        重开
                      </button>
                      {resumeNote[s.engine_session_id] ? (
                        <span className="text-on-surface-variant basis-full">
                          {resumeNote[s.engine_session_id]}
                        </span>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </>
        ) : null}
      </section>

      <section className="rounded-2xl bg-surface-container-lowest/80 ring-1 ring-outline-variant/40 p-5 space-y-4">
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-base font-semibold text-on-surface">OmniDesk 任务记录</h2>
          <button
            className="text-xs px-3 py-1.5 rounded-full ring-1 ring-outline-variant/60 hover:bg-surface-container"
            onClick={() => void loadLedger()}
            disabled={blocked || ledgerBusy}
          >
            {ledgerBusy ? "读取中…" : "刷新记录"}
          </button>
        </div>
        {ledgerError ? (
          <p className="text-xs text-error">读取失败：{ledgerError}</p>
        ) : null}
        {ledger ? (
          <>
            <p className="text-xs text-on-surface-variant">
              共 {ledger.records.length} 条；损坏记录 {ledger.skipped_invalid} 条。
              {ledger.note}
            </p>
            {ledger.records.length === 0 ? (
              <p className="text-xs text-on-surface-variant">本地任务台账为空。</p>
            ) : null}
            <ul className="space-y-2">
              {ledger.records.map((r) => (
                <li
                  key={r.omni_task_id}
                  className="flex flex-wrap items-center gap-2 text-xs"
                >
                  <span className="font-mono break-all">{r.omni_task_id}</span>
                  <span className="text-on-surface-variant">
                    {STATE_TEXT[r.state] ?? r.state}
                  </span>
                  <button
                    className="px-2 py-1 rounded-full ring-1 ring-outline-variant/60 hover:bg-surface-container"
                    onClick={() => void recover(r)}
                  >
                    读取状态
                  </button>
                  {recoverNote[r.omni_task_id] ? (
                    <span className="text-on-surface-variant basis-full">
                      {recoverNote[r.omni_task_id]}
                    </span>
                  ) : null}
                </li>
              ))}
            </ul>
          </>
        ) : null}
      </section>

      <p className="text-xs text-on-surface-variant leading-relaxed">
        旧版 DSH 会话目前没有公开的导入接口。按计划只记录兼容性结论，
        不复制、不改写 DSH 原生会话文件，因此这里不提供“导入旧会话”按钮。
      </p>
    </div>
  );
}
