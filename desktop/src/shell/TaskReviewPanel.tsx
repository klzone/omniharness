import { useEffect, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { TaskCandidate, WorkRecord, WorkTask } from "../types";

type Notice = { kind: "ok" | "err"; text: string };

const CARD = "bg-white/80 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "rounded-full border border-outline-variant/50 bg-white px-3 py-1.5 text-[11.5px] text-on-surface-variant focus:outline-none focus:ring-1 focus:ring-primary/30";
const FIELD_BLOCK =
  "w-full rounded-xl border border-outline-variant/50 bg-white px-3 py-2 text-[12.5px] text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30";

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function confidenceText(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  return Math.round(value * 100) + "%";
}

function EvidenceBody({ record, quote }: { record: WorkRecord; quote: string }) {
  const index = record.body.indexOf(quote);
  if (index < 0) {
    return (
      <div className="text-[11.5px] leading-relaxed text-on-surface-variant">
        <p>来源原文已变化，下面这段候选引用不再能定位：</p>
        <p className="mt-1 rounded bg-error/5 px-2 py-1 text-error">{quote}</p>
      </div>
    );
  }
  return (
    <p className="text-[11.5px] leading-relaxed text-on-surface-variant whitespace-pre-wrap">
      {record.body.slice(0, index)}
      <mark className="rounded bg-primary-fixed px-0.5 text-on-primary-fixed-variant">
        {quote}
      </mark>
      {record.body.slice(index + quote.length)}
    </p>
  );
}

export function TaskReviewPanel({
  embedded = false,
  onOpenRecord,
}: {
  /** 浮层内使用时去掉整页留白，只保留内部滚动。 */
  embedded?: boolean;
  onOpenRecord: (recordId: string) => void;
}) {
  const [items, setItems] = useState<TaskCandidate[]>([]);
  const [records, setRecords] = useState<Record<string, WorkRecord>>({});
  const [tasks, setTasks] = useState<Record<string, WorkTask>>({});
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [category, setCategory] = useState("");
  const [dueDate, setDueDate] = useState("");
  const [note, setNote] = useState("");
  const [rejectNote, setRejectNote] = useState("");
  const [progressText, setProgressText] = useState("");

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([shell.taskCandidateList(), shell.workRecordList({}), shell.taskList({})])
      .then(([candidates, recordsResult, tasksResult]) => {
        if (!alive) return;
        setItems(candidates.items);
        const map: Record<string, WorkRecord> = {};
        for (const record of recordsResult.items) map[record.id] = record;
        setRecords(map);
        const taskMap: Record<string, WorkTask> = {};
        for (const task of tasksResult.items) taskMap[task.id] = task;
        setTasks(taskMap);
        setLoadError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setLoadError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    const stillVisible = selectedId !== null && items.some((item) => item.id === selectedId);
    if (stillVisible) return;
    setSelectedId(items.length > 0 ? items[0].id : null);
  }, [items, selectedId]);

  useEffect(() => {
    setSelectedIds((prev) => prev.filter((id) => items.some((item) => item.id === id)));
  }, [items]);

  const selected = items.find((item) => item.id === selectedId) ?? null;

  useEffect(() => {
    if (!selected) {
      setTitle("");
      setCategory("");
      setDueDate("");
      setNote("");
      setRejectNote("");
      setProgressText("");
      return;
    }
    setTitle(selected.title);
    setCategory(selected.category ?? "");
    setDueDate(selected.due_date ?? "");
    setNote("");
    setRejectNote("");
    const candidate = items.find((item) => item.id === selectedId);
    setProgressText(candidate?.title ?? "");
  }, [selectedId]);

  function toggleBatch(id: string) {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((value) => value !== id) : [...prev, id],
    );
  }

  async function confirmSingle() {
    if (!selected || busy) return;
    if (!title.trim()) {
      setNotice({ kind: "err", text: "任务标题不能为空。" });
      return;
    }
    setBusy("confirm");
    setNotice(null);
    try {
      const saved = await shell.taskCandidateConfirm(selected.id, {
        title: title.trim(),
        category,
        due_date: dueDate,
        note,
      });
      setNotice({ kind: "ok", text: "已采纳为任务：" + saved.task.title });
      setSelectedId(null);
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(null);
    }
  }

  async function rejectSingle() {
    if (!selected || busy) return;
    setBusy("reject");
    setNotice(null);
    try {
      await shell.taskCandidateReject(selected.id, rejectNote || undefined);
      setNotice({ kind: "ok", text: "已拒绝这条候选。" });
      setSelectedId(null);
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(null);
    }
  }

  async function confirmBatch() {
    if (selectedIds.length === 0 || busy) return;
    setBusy("batch");
    setNotice(null);
    let ok = 0;
    const failures: string[] = [];
    for (const id of selectedIds) {
      try {
        await shell.taskCandidateConfirm(id, {});
        ok += 1;
      } catch (e) {
        failures.push(errText(e));
      }
    }
    setSelectedIds([]);
    setReloadTick((n) => n + 1);
    if (failures.length === 0) {
      setNotice({ kind: "ok", text: "已采纳 " + ok + " 条候选。" });
    } else {
      setNotice({
        kind: "err",
        text:
          "已采纳 " +
          ok +
          " 条；" +
          failures.length +
          " 条未通过：" +
          failures.slice(0, 3).join("；"),
      });
    }
    setBusy(null);
  }

  async function linkSingle() {
    if (!selected || busy || !selected.match_task_id) return;
    setBusy("link");
    setNotice(null);
    try {
      const report = await shell.taskCandidateLink(
        selected.id,
        progressText,
        note || undefined,
      );
      if (!report.ok) {
        setNotice({
          kind: "err",
          text:
            (report.error ?? "关联只完成了一部分。") +
            (report.warnings.length > 0 ? " " + report.warnings.join("；") : ""),
        });
        setReloadTick((n) => n + 1);
        return;
      }
      setNotice({
        kind: "ok",
        text: "已追加进展到：" + (report.task?.title ?? "目标任务"),
      });
      setSelectedId(null);
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(null);
    }
  }

  async function dismissMatch() {
    if (!selected || busy || !selected.match_task_id) return;
    setBusy("dismiss");
    setNotice(null);
    try {
      await shell.taskCandidateDismissMatch(selected.id);
      setNotice({ kind: "ok", text: "已拒绝关联建议；候选仍在待审核。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(null);
    }
  }

  const sourceRecord = selected ? records[selected.record_id] ?? null : null;
  const matchedTask =
    selected?.match_task_id ? tasks[selected.match_task_id] ?? null : null;
  const matchUsable = Boolean(
    matchedTask && ["待开始", "进行中", "暂停"].includes(matchedTask.state),
  );
  const allSelected = items.length > 0 && selectedIds.length === items.length;

  return (
    <div
      className={
        embedded
          ? "min-h-0 flex-1 overflow-y-auto px-4 pb-4"
          : "flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6"
      }
      data-candidate-panel=""
    >
      {notice ? (
        <div
          role="status"
          data-review-notice={notice.kind}
          className={
            "max-w-7xl mx-auto md:-translate-x-8 mb-3 rounded-2xl px-4 py-2 text-[11.5px] leading-relaxed " +
            (notice.kind === "err"
              ? "bg-error-container/70 text-on-error-container ring-1 ring-error/30"
              : "bg-surface-container text-on-surface-variant ring-1 ring-outline-variant/40")
          }
        >
          {notice.text}
        </div>
      ) : null}

      <div
        className={
          "mx-auto grid grid-cols-1 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)] gap-4 lg:gap-5 items-start" +
          (embedded ? "" : " max-w-7xl md:-translate-x-8")
        }
      >
        <section
          aria-label="待审核候选"
          className={CARD + " px-3 py-3 flex flex-col min-h-[360px] max-h-[72vh]"}
        >
          <div className="flex items-center gap-2 px-1.5 pb-2.5 border-b border-surface-variant/25">
            <label className="flex items-center gap-1.5 text-[10.5px] text-outline">
              <input
                type="checkbox"
                checked={allSelected}
                onChange={(e) => setSelectedIds(e.target.checked ? items.map((item) => item.id) : [])}
                className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
              />
              全选
            </label>
            <span className="mono text-[10.5px] text-outline">{items.length} 条</span>
            <button
              type="button"
              onClick={() => void confirmBatch()}
              disabled={selectedIds.length === 0 || Boolean(busy)}
              data-candidate-batch=""
              className="ml-auto rounded-full bg-primary px-3 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
            >
              {busy === "batch" ? "采纳中…" : "采纳选中 " + selectedIds.length}
            </button>
          </div>
          {loadError ? (
            <p className="px-1.5 pt-3 text-[11.5px] leading-relaxed text-error">
              读取候选失败：{loadError}
            </p>
          ) : loading ? (
            <p className="px-1.5 pt-3 text-[11.5px] text-outline">正在读取候选…</p>
          ) : (
            <div className="pt-2 flex-1 min-h-0 overflow-y-auto flex flex-col gap-1" data-candidate-list="">
              {items.length === 0 ? (
                <p className="px-1.5 pt-3 text-[12px] text-outline">没有待审核候选。</p>
              ) : (
                items.map((item) => {
                  const active = item.id === selectedId;
                  return (
                    <div
                      key={item.id}
                      data-candidate-row={item.id}
                      className={
                        "flex items-center gap-2 rounded-xl px-2 py-1.5 " +
                        (active ? "bg-primary-fixed/60" : "hover:bg-surface-container/60")
                      }
                    >
                      <input
                        type="checkbox"
                        checked={selectedIds.includes(item.id)}
                        onChange={() => toggleBatch(item.id)}
                        className="h-3.5 w-3.5 shrink-0 accent-[color:var(--color-primary)]"
                      />
                      <button
                        type="button"
                        onClick={() => setSelectedId(item.id)}
                        className="min-w-0 flex-1 text-left px-1 py-1"
                      >
                        <span className="block text-[12.5px] font-medium text-on-surface truncate">
                          {item.title}
                        </span>
                        <span className="block mt-0.5 text-[10.5px] text-outline truncate">
                          {item.uncertainty ? item.uncertainty : item.source_quote}
                        </span>
                      </button>
                      <span
                        className="shrink-0 rounded-full bg-surface-container px-2 py-0.5 mono text-[10px] text-on-surface-variant"
                        data-candidate-confidence={item.id}
                      >
                        {confidenceText(item.confidence)}
                      </span>
                    </div>
                  );
                })
              )}
            </div>
          )}
        </section>

        <section
          aria-label="候选详情"
          className={CARD + " px-4 py-4 lg:px-5 lg:py-5 lg:sticky lg:top-2 max-h-[78vh] flex flex-col"}
          data-candidate-detail=""
        >
          {!selected ? (
            <p className="text-[12px] text-outline">选择一条候选查看证据与字段。</p>
          ) : (
            <div className="flex flex-col min-h-0">
              <div className="flex items-start gap-2.5">
                <h2 className="min-w-0 flex-1 text-[18px] font-semibold leading-snug tracking-tight text-on-surface">
                  {selected.title}
                </h2>
                <span className="shrink-0 rounded-full bg-primary/10 px-2.5 py-0.5 text-[11px] font-medium text-primary">
                  {confidenceText(selected.confidence)}
                </span>
              </div>
              <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-outline">
                {sourceRecord ? (
                  <button
                    type="button"
                    onClick={() => onOpenRecord(selected.record_id)}
                    className="text-primary hover:underline"
                    data-candidate-source=""
                  >
                    {sourceRecord.record_date} · {sourceRecord.title}
                  </button>
                ) : (
                  <span>来源记录不可用</span>
                )}
                {selected.category ? <span>{selected.category}</span> : null}
                {selected.due_date ? <span>截止 {selected.due_date}</span> : null}
              </div>

              <section aria-label="原文证据" className="mt-3">
                <h3 className="text-[12px] font-semibold text-on-surface">原文证据</h3>
                {sourceRecord ? (
                  <div
                    className="mt-1.5 max-h-[26vh] overflow-y-auto rounded-xl bg-surface-container/50 px-3 py-2"
                    data-candidate-evidence=""
                  >
                    <EvidenceBody record={sourceRecord} quote={selected.source_quote} />
                  </div>
                ) : (
                  <p className="mt-1 text-[11.5px] text-error">来源记录已删除或不可读，不能采纳。</p>
                )}
              </section>

              {selected.uncertainty ? (
                <p className="mt-2.5 rounded-xl bg-error/5 px-3 py-2 text-[11.5px] leading-relaxed text-on-error-container">
                  不确定原因：{selected.uncertainty}
                </p>
              ) : null}
              {selected.note ? (
                <p className="mt-2 text-[11.5px] leading-relaxed text-on-surface-variant">
                  候选备注：{selected.note}
                </p>
              ) : null}

              {selected.verdict ? (
                <div
                  data-candidate-verdict=""
                  className="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10.5px] text-outline"
                >
                  <span className="mono">
                    {selected.verdict.conclusion === "pass" ? "验收通过" : "需人工确认"}
                  </span>
                  {selected.verdict.checks
                    .filter((check) => !check.pass)
                    .map((check) => (
                      <span key={check.id} className="mono text-error">
                        {check.id}
                      </span>
                    ))}
                </div>
              ) : null}

              {selected.match_task_id ? (
                <section
                  aria-label="关联建议"
                  data-candidate-match=""
                  className="mt-2.5 rounded-xl bg-primary-fixed/30 px-3 py-2.5"
                >
                  <div className="flex items-center gap-2">
                    <p className="min-w-0 flex-1 text-[11.5px] text-on-surface">
                      可能关联：
                      {matchedTask
                        ? matchedTask.title + "（" + matchedTask.state + "）"
                        : "任务已不可用"}
                    </p>
                    <span className="mono text-[10px] text-outline">
                      关联 {confidenceText(selected.match_confidence)}
                    </span>
                  </div>
                  {selected.match_reason ? (
                    <p className="mt-1 text-[11px] leading-relaxed text-on-surface-variant">
                      理由：{selected.match_reason}
                    </p>
                  ) : null}
                  {selected.match_confidence !== null && selected.match_confidence < 0.5 ? (
                    <p className="mt-1 text-[11px] text-error">关联置信度低，需人工判断。</p>
                  ) : null}
                  {matchedTask && !matchUsable ? (
                    <p className="mt-1 text-[11px] text-error">
                      关联任务已结束，不能追加进展；可拒绝关联后作为新任务采纳。
                    </p>
                  ) : null}
                  {selected.suggested_state && matchedTask ? (
                    <p className="mt-1 text-[11px] text-on-surface-variant">
                      AI 建议状态：{matchedTask.state} → {selected.suggested_state}
                      （不会自动应用）
                    </p>
                  ) : null}
                  <label className="mt-2 block text-[10.5px] text-outline">
                    追加到任务的进展
                  </label>
                  <textarea
                    value={progressText}
                    onChange={(e) => setProgressText(e.target.value)}
                    aria-label="追加进展内容"
                    className={FIELD_BLOCK + " mt-1 min-h-[54px] resize-y leading-relaxed"}
                  />
                </section>
              ) : null}

              <div className="mt-3 flex flex-col gap-2 min-h-0 overflow-y-auto">
                <input
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  aria-label="任务标题"
                  className={FIELD_BLOCK}
                />
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    value={category}
                    onChange={(e) => setCategory(e.target.value)}
                    placeholder="分类"
                    aria-label="分类"
                    className={FIELD}
                  />
                  <input
                    type="date"
                    value={dueDate}
                    onChange={(e) => setDueDate(e.target.value)}
                    aria-label="截止日期"
                    className={FIELD}
                  />
                </div>
                <input
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                  placeholder="确认备注（可选）"
                  aria-label="确认备注"
                  className={FIELD_BLOCK}
                />
              </div>

              <div className="mt-3 flex items-center gap-2">
                <input
                  value={rejectNote}
                  onChange={(e) => setRejectNote(e.target.value)}
                  placeholder="拒绝理由（可选）"
                  aria-label="拒绝理由"
                  className={FIELD + " flex-1"}
                />
              </div>

              <div className="mt-3 pt-3 border-t border-surface-variant/25 flex items-center gap-2" data-candidate-actions="">
                {selected.match_task_id ? (
                  <>
                    <button
                      type="button"
                      onClick={() => void linkSingle()}
                      disabled={Boolean(busy) || !matchUsable}
                      data-candidate-link=""
                      className="rounded-full bg-primary px-4 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
                    >
                      {busy === "link" ? "追加中…" : "采纳关联"}
                    </button>
                    <button
                      type="button"
                      onClick={() => void confirmSingle()}
                      disabled={Boolean(busy) || !sourceRecord}
                      className="rounded-full border border-outline-variant/60 bg-white/75 px-3 py-1.5 text-[11px] font-medium text-on-surface-variant disabled:opacity-40"
                    >
                      作为新任务采纳
                    </button>
                    <button
                      type="button"
                      onClick={() => void dismissMatch()}
                      disabled={Boolean(busy)}
                      data-candidate-dismiss=""
                      className="rounded-full px-3 py-1.5 text-[11px] text-outline hover:text-primary disabled:opacity-40"
                    >
                      {busy === "dismiss" ? "处理中…" : "拒绝关联"}
                    </button>
                    <button
                      type="button"
                      onClick={() => void rejectSingle()}
                      disabled={Boolean(busy)}
                      className="rounded-full px-3 py-1.5 text-[11px] font-medium text-outline hover:text-error hover:bg-error/5 disabled:opacity-40"
                    >
                      拒绝候选
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => void confirmSingle()}
                      disabled={Boolean(busy) || !sourceRecord}
                      className="rounded-full bg-primary px-4 py-1.5 text-[11px] font-medium text-on-primary disabled:opacity-40"
                    >
                      {busy === "confirm" ? "采纳中…" : "采纳为任务"}
                    </button>
                    <button
                      type="button"
                      onClick={() => void rejectSingle()}
                      disabled={Boolean(busy)}
                      className="rounded-full px-3 py-1.5 text-[11px] font-medium text-outline hover:text-error hover:bg-error/5 disabled:opacity-40"
                    >
                      拒绝
                    </button>
                  </>
                )}
                <span className="ml-auto flex items-center gap-1 text-[10.5px] text-outline">
                  <Icon name="shield-check" className="h-3.5 w-3.5" />
                  确认后写入任务文件
                </span>
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
