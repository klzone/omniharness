import { useEffect, useState } from "react";

import { host, shell } from "../api";
import { Icon } from "../components/Icon";
import { TERMINAL_STATES } from "../types";
import type {
  RecordFilter,
  RecordListing,
  RecordPatch,
  WorkRecord,
  WorkRecordAiStatus,
  WorkRecordSaved,
} from "../types";
import { MarkdownText } from "./MarkdownText";

type Notice = { kind: "ok" | "err"; text: string };

interface EditForm {
  title: string;
  record_date: string;
  category: string;
  tags: string;
  body: string;
  draft: boolean;
}

const STATUS_LABEL: Record<WorkRecordAiStatus, string> = {
  unanalyzed: "未解析",
  analyzed: "已解析",
  failed: "解析失败",
};

const STATUS_CLASS: Record<WorkRecordAiStatus, string> = {
  unanalyzed: "bg-surface-container text-outline",
  analyzed: "bg-secondary-container/60 text-on-secondary-container",
  failed: "bg-error/10 text-error",
};

const CARD = "bg-white/80 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "rounded-full border border-outline-variant/50 bg-white px-3 py-1.5 text-[11.5px] text-on-surface-variant focus:outline-none focus:ring-1 focus:ring-primary/30";
const FIELD_BLOCK =
  "w-full rounded-xl border border-outline-variant/50 bg-white px-3 py-2 text-[12.5px] text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function todayLocal(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return String(now.getFullYear()) + "-" + pad(now.getMonth() + 1) + "-" + pad(now.getDate());
}

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function maskNote(saved: WorkRecordSaved): string | null {
  if (!saved.masked) return null;
  const parts: string[] = [];
  if (saved.masked_id_card > 0) parts.push(saved.masked_id_card + " 处身份证号");
  if (saved.masked_phone > 0) parts.push(saved.masked_phone + " 处手机号");
  return parts.length > 0 ? "落盘前已掩码 " + parts.join("、") : "落盘前已做隐私掩码";
}

export function WorkRecordLibrary({
  focusRecordId,
  onFocusConsumed,
  onCandidatesReady,
}: {
  focusRecordId: string | null;
  onFocusConsumed: () => void;
  onCandidatesReady: () => void;
}) {
  const [listing, setListing] = useState<RecordListing>({ items: [], broken: 0 });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState<RecordFilter>({});
  const [keywordDraft, setKeywordDraft] = useState("");
  const [categoryOptions, setCategoryOptions] = useState<string[]>([]);
  const [reloadTick, setReloadTick] = useState(0);
  const [selected, setSelected] = useState<WorkRecord | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState<EditForm | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [quickText, setQuickText] = useState("");
  const [quickDate, setQuickDate] = useState(todayLocal());
  const [quickDraft, setQuickDraft] = useState(false);
  const [busy, setBusy] = useState(false);
  const [parsing, setParsing] = useState(false);

  // 关键词检索做 250ms 防抖，避免每敲一个字都触发一次宿主读盘。
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setFilter((prev) => {
        const next: RecordFilter = { ...prev };
        const keyword = keywordDraft.trim();
        if (keyword) next.keyword = keyword;
        else delete next.keyword;
        return next;
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [keywordDraft]);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .workRecordList(filter)
      .then((result) => {
        if (!alive) return;
        setListing(result);
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
  }, [filter, reloadTick]);

  // 分类下拉来自实际记录；新分类保存后随 reloadTick 一起刷新。
  useEffect(() => {
    let alive = true;
    shell
      .workRecordList({})
      .then((result) => {
        if (!alive) return;
        const options = Array.from(
          new Set(
            result.items
              .map((item) => item.category)
              .filter((value): value is string => Boolean(value)),
          ),
        );
        options.sort((a, b) => a.localeCompare(b, "zh-Hans-CN"));
        setCategoryOptions(options);
      })
      .catch(() => {
        // 分类只是筛选辅助；读不到时不影响列表本身。
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    if (!focusRecordId) return;
    let alive = true;
    shell
      .workRecordRead(focusRecordId)
      .then((record) => {
        if (!alive) return;
        setSelected(record);
        setEditing(false);
        setConfirmDelete(false);
        setNotice(null);
      })
      .catch((e) => {
        if (!alive) return;
        setNotice({ kind: "err", text: errText(e) });
      })
      .finally(() => {
        if (alive) onFocusConsumed();
      });
    return () => {
      alive = false;
    };
  }, [focusRecordId]);

  function selectRecord(record: WorkRecord) {
    setSelected(record);
    setEditing(false);
    setForm(null);
    setConfirmDelete(false);
    setNotice(null);
  }

  function patchFilter(patch: RecordFilter) {
    setFilter((prev) => ({ ...prev, ...patch }));
  }

  async function saveQuick() {
    if (!quickText.trim() || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const saved = await shell.workRecordCreate({
        body: quickText,
        record_date: quickDate || null,
        draft: quickDraft,
      });
      setSelected(saved.record);
      setQuickText("");
      setQuickDraft(false);
      setEditing(false);
      setConfirmDelete(false);
      setNotice({ kind: "ok", text: maskNote(saved) ?? "已记录。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  function startEdit() {
    if (!selected) return;
    setForm({
      title: selected.title,
      record_date: selected.record_date,
      category: selected.category ?? "",
      tags: selected.tags.join(", "),
      body: selected.body,
      draft: selected.draft,
    });
    setEditing(true);
    setConfirmDelete(false);
    setNotice(null);
  }

  async function saveEdit() {
    if (!selected || !form || busy) return;
    if (!form.title.trim()) {
      setNotice({ kind: "err", text: "标题不能为空。" });
      return;
    }
    setBusy(true);
    setNotice(null);
    const patch: RecordPatch = {
      title: form.title.trim(),
      record_date: form.record_date || selected.record_date,
      category: form.category,
      tags: form.tags
        .split(/[,，]/)
        .map((tag) => tag.trim())
        .filter(Boolean),
      body: form.body,
      draft: form.draft,
    };
    try {
      const saved = await shell.workRecordUpdate(selected.id, patch);
      setSelected(saved.record);
      setEditing(false);
      setForm(null);
      setNotice({ kind: "ok", text: maskNote(saved) ?? "已保存。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function removeSelected() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.workRecordDelete(selected.id);
      setSelected(null);
      setEditing(false);
      setForm(null);
      setConfirmDelete(false);
      setNotice({ kind: "ok", text: "已移入回收目录。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function parseSelected() {
    if (!selected || parsing) return;
    setParsing(true);
    setNotice({ kind: "ok", text: "正在提交解析任务…" });
    try {
      const prep = await shell.workRecordAnalyzePrepare(selected.id);
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      const receipt = await host.submitTask({
        promptText: prep.prompt,
        intent: "worklog_extract",
        workspaceDir: workspace.workspace_dir,
        userInputText: prep.user_input_text,
      });
      const taskId = receipt.omni_task_id;
      let status = await host.taskStatus(taskId);
      for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(taskId);
        if (i % 10 === 0) {
          setNotice({ kind: "ok", text: "模型正在提取候选…已等待 " + (i + 1) + " 秒" });
        }
      }
      if (status.status !== "succeeded") {
        await shell.workRecordAnalyzeFailed(selected.id).catch(() => undefined);
        throw new Error(
          "解析未完成：" +
            status.status +
            (status.note ? " · " + status.note : ""),
        );
      }
      const result = await host.taskResult(taskId);
      if (!result.content.trim()) {
        await shell.workRecordAnalyzeFailed(selected.id).catch(() => undefined);
        throw new Error("模型没有返回解析结果。");
      }
      const applied = await shell.workRecordAnalyzeApply(selected.id, result.content);
      setSelected(applied.record);
      setReloadTick((n) => n + 1);
      setNotice({
        kind: "ok",
        text:
          applied.staged.length > 0
            ? "已生成 " + applied.staged.length + " 条候选，去「审核」确认。"
            : "没有提取到任务候选。",
      });
      if (applied.staged.length > 0) onCandidatesReady();
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setParsing(false);
    }
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
      {notice ? (
        <div
          role="status"
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

      <div className="max-w-7xl mx-auto md:-translate-x-8 grid grid-cols-1 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)] gap-4 lg:gap-5 items-start">
        <div className="min-w-0 flex flex-col gap-4">
          <section aria-label="快速记录" className={CARD + " px-4 py-4"}>
            <textarea
              value={quickText}
              onChange={(e) => setQuickText(e.target.value)}
              placeholder="记点什么…"
              className="w-full min-h-[88px] resize-y bg-transparent text-[13px] leading-relaxed text-on-surface placeholder:text-outline/70 focus:outline-none"
              data-work-record-quick=""
            />
            <div className="mt-2 flex flex-wrap items-center gap-3">
              <input
                type="date"
                value={quickDate}
                onChange={(e) => setQuickDate(e.target.value)}
                aria-label="记录日期"
                className={FIELD}
              />
              <label className="flex items-center gap-1.5 text-[11.5px] text-on-surface-variant">
                <input
                  type="checkbox"
                  checked={quickDraft}
                  onChange={(e) => setQuickDraft(e.target.checked)}
                  className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                />
                草稿
              </label>
              <button
                onClick={() => void saveQuick()}
                disabled={busy || !quickText.trim()}
                className="ml-auto rounded-full bg-primary px-4 py-1.5 text-[11.5px] font-medium text-on-primary disabled:opacity-40"
              >
                记录
              </button>
            </div>
          </section>

          <section aria-label="筛选" className={CARD + " px-3.5 py-3"}>
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex items-center gap-1.5 rounded-full border border-outline-variant/50 bg-white px-3 py-1.5">
                <Icon name="search" className="h-3.5 w-3.5 text-outline" />
                <input
                  value={keywordDraft}
                  onChange={(e) => setKeywordDraft(e.target.value)}
                  placeholder="检索标题或正文"
                  className="w-[150px] bg-transparent text-[11.5px] text-on-surface placeholder:text-outline/70 focus:outline-none"
                />
              </div>
              <input
                type="date"
                value={filter.from_date ?? ""}
                onChange={(e) => patchFilter({ from_date: e.target.value || undefined })}
                aria-label="起始日期"
                className={FIELD}
              />
              <span className="text-outline/70 text-[11px]">—</span>
              <input
                type="date"
                value={filter.to_date ?? ""}
                onChange={(e) => patchFilter({ to_date: e.target.value || undefined })}
                aria-label="结束日期"
                className={FIELD}
              />
              <select
                value={filter.category ?? ""}
                onChange={(e) => patchFilter({ category: e.target.value || undefined })}
                aria-label="按分类筛选"
                className={FIELD}
              >
                <option value="">全部分类</option>
                {categoryOptions.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
              <select
                value={filter.ai_analysis_status ?? ""}
                onChange={(e) =>
                  patchFilter({
                    ai_analysis_status:
                      (e.target.value as WorkRecordAiStatus) || undefined,
                  })
                }
                aria-label="按解析状态筛选"
                className={FIELD}
              >
                <option value="">全部状态</option>
                <option value="unanalyzed">未解析</option>
                <option value="analyzed">已解析</option>
                <option value="failed">解析失败</option>
              </select>
              <label className="flex items-center gap-1.5 text-[11.5px] text-on-surface-variant">
                <input
                  type="checkbox"
                  checked={filter.draft === true}
                  onChange={(e) => patchFilter({ draft: e.target.checked ? true : undefined })}
                  className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                />
                只看草稿
              </label>
            </div>
          </section>

          <section
            aria-label="记录列表"
            className={CARD + " px-3 py-3 flex flex-col min-h-[320px] max-h-[62vh]"}
          >
            <div className="flex items-baseline gap-2 px-1.5 pb-2.5 border-b border-surface-variant/25">
              <h2 className="text-[15px] font-semibold tracking-tight text-on-surface">记录</h2>
              <span className="mono text-[10.5px] text-outline ml-auto">
                {listing.items.length} 条
              </span>
            </div>
            {loadError ? (
              <p className="px-1.5 pt-3 text-[11.5px] leading-relaxed text-error">
                读取记录失败：{loadError}
              </p>
            ) : loading ? (
              <p className="px-1.5 pt-3 text-[11.5px] text-outline">正在读取记录…</p>
            ) : (
              <div className="pt-2 flex-1 min-h-0 overflow-y-auto flex flex-col gap-1" data-work-record-list="">
                {listing.items.length === 0 ? (
                  <p className="px-1.5 pt-3 text-[12px] text-outline">还没有记录。</p>
                ) : (
                  listing.items.map((record) => {
                    const active = selected?.id === record.id;
                    return (
                      <button
                        key={record.id}
                        onClick={() => selectRecord(record)}
                        data-record-row={record.id}
                        className={
                          "w-full text-left px-3 py-2.5 rounded-xl flex items-center gap-3 transition-colors " +
                          (active ? "bg-primary-fixed/60" : "hover:bg-surface-container/60")
                        }
                      >
                        <span className="mono text-[10.5px] text-outline shrink-0 w-[74px]">
                          {record.record_date}
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block text-[12.5px] font-medium text-on-surface truncate">
                            {record.title}
                          </span>
                          {record.tags.length > 0 || record.category ? (
                            <span className="block mt-0.5 text-[10.5px] text-outline truncate">
                              {[record.category, ...record.tags.slice(0, 2).map((t) => "#" + t)]
                                .filter(Boolean)
                                .join("  ")}
                            </span>
                          ) : null}
                        </span>
                        <span
                          className={
                            "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium " +
                            STATUS_CLASS[record.ai_analysis_status]
                          }
                          data-record-status={record.ai_analysis_status}
                        >
                          {STATUS_LABEL[record.ai_analysis_status]}
                        </span>
                        <span className="shrink-0 flex items-center gap-1 text-[10.5px] text-outline">
                          <Icon name="checklist" className="h-3.5 w-3.5" />
                          {record.linked_task_ids.length}
                        </span>
                      </button>
                    );
                  })
                )}
                {listing.broken > 0 ? (
                  <p className="px-1.5 pt-2 text-[10.5px] text-outline">
                    {listing.broken} 个文件无法读取，未计入列表。
                  </p>
                ) : null}
              </div>
            )}
          </section>
        </div>

        <section
          aria-label="记录详情"
          className={CARD + " px-4 py-4 lg:px-5 lg:py-5 lg:sticky lg:top-2 max-h-[76vh] flex flex-col"}
          data-work-record-detail=""
        >
          {!selected ? (
            <p className="text-[12px] text-outline">选择一条记录查看正文。</p>
          ) : editing && form ? (
            <div className="flex flex-col gap-2.5 min-h-0 overflow-y-auto">
              <input
                value={form.title}
                onChange={(e) => setForm({ ...form, title: e.target.value })}
                aria-label="标题"
                className={FIELD_BLOCK}
              />
              <div className="flex flex-wrap items-center gap-2">
                <input
                  type="date"
                  value={form.record_date}
                  onChange={(e) => setForm({ ...form, record_date: e.target.value })}
                  aria-label="记录日期"
                  className={FIELD}
                />
                <input
                  value={form.category}
                  onChange={(e) => setForm({ ...form, category: e.target.value })}
                  placeholder="分类"
                  aria-label="分类"
                  className={FIELD}
                />
                <label className="flex items-center gap-1.5 text-[11.5px] text-on-surface-variant">
                  <input
                    type="checkbox"
                    checked={form.draft}
                    onChange={(e) => setForm({ ...form, draft: e.target.checked })}
                    className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                  />
                  草稿
                </label>
              </div>
              <input
                value={form.tags}
                onChange={(e) => setForm({ ...form, tags: e.target.value })}
                placeholder="标签，用逗号分隔"
                aria-label="标签"
                className={FIELD_BLOCK}
              />
              <textarea
                value={form.body}
                onChange={(e) => setForm({ ...form, body: e.target.value })}
                aria-label="正文"
                className={FIELD_BLOCK + " min-h-[280px] resize-y leading-relaxed"}
              />
              <div className="flex items-center gap-2 pt-1">
                <button
                  onClick={() => void saveEdit()}
                  disabled={busy}
                  className="rounded-full bg-primary px-4 py-1.5 text-[11.5px] font-medium text-on-primary disabled:opacity-40"
                >
                  保存
                </button>
                <button
                  onClick={() => {
                    setEditing(false);
                    setForm(null);
                  }}
                  className="text-[11.5px] text-on-surface-variant hover:text-primary"
                >
                  取消
                </button>
              </div>
            </div>
          ) : (
            <div className="flex flex-col min-h-0">
              <div className="flex items-start gap-2.5">
                <h2 className="min-w-0 flex-1 text-[18px] font-semibold leading-snug tracking-tight text-on-surface" data-hover-hint={selected.title}>
                  {selected.title}
                </h2>
                <span
                  className={
                    "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium " +
                    STATUS_CLASS[selected.ai_analysis_status]
                  }
                >
                  {STATUS_LABEL[selected.ai_analysis_status]}
                </span>
              </div>
              <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-outline">
                <span className="mono">{selected.record_date}</span>
                {selected.category ? <span>{selected.category}</span> : null}
                {selected.draft ? <span>草稿</span> : null}
                <span className="flex items-center gap-1">
                  <Icon name="checklist" className="h-3.5 w-3.5" />
                  {selected.linked_task_ids.length} 个任务
                </span>
              </div>
              {selected.tags.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {selected.tags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded-full bg-surface-container px-2 py-0.5 text-[10.5px] text-on-surface-variant"
                    >
                      {"#" + tag}
                    </span>
                  ))}
                </div>
              ) : null}
              <div className="mt-3 pt-3 border-t border-surface-variant/25 flex-1 min-h-0 overflow-y-auto">
                <MarkdownText text={selected.body} />
              </div>
              <div className="mt-3 pt-3 border-t border-surface-variant/25 flex items-center gap-2">
                <button
                  onClick={() => void parseSelected()}
                  disabled={parsing || selected.ai_analysis_status === "analyzed"}
                  data-parse-entry=""
                  data-hover-hint={
                    selected.ai_analysis_status === "analyzed"
                      ? "这条记录已经解析过"
                      : "用模型提取任务候选；结果先进入审核"
                  }
                  className="rounded-full bg-surface-container px-3 py-1.5 text-[11px] font-medium text-on-surface-variant hover:bg-surface-container-high disabled:opacity-40"
                >
                  {parsing ? "解析中…" : "解析"}
                </button>
                <button
                  onClick={startEdit}
                  className="rounded-full px-3 py-1.5 text-[11px] font-medium text-primary hover:bg-primary-fixed/50"
                >
                  编辑
                </button>
                {confirmDelete ? (
                  <div className="ml-auto flex items-center gap-2">
                    <button
                      onClick={() => void removeSelected()}
                      disabled={busy}
                      className="rounded-full bg-error px-3 py-1.5 text-[11px] font-medium text-on-error disabled:opacity-40"
                    >
                      确认删除
                    </button>
                    <button
                      onClick={() => setConfirmDelete(false)}
                      className="text-[11px] text-on-surface-variant hover:text-primary"
                    >
                      取消
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={() => setConfirmDelete(true)}
                    className="ml-auto rounded-full px-3 py-1.5 text-[11px] font-medium text-outline hover:text-error hover:bg-error/5"
                  >
                    删除
                  </button>
                )}
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
