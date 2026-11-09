import { useEffect, useMemo, useState } from "react";

import { host, shell } from "../api";
import { TERMINAL_STATES } from "../types";
import { WritingFactCheckPanel } from "./WritingFactCheckPanel";
import { WritingOutlineEditor } from "./WritingOutlineEditor";
import { WritingSectionEditor } from "./WritingSectionEditor";
import { WritingVersionPanel } from "./WritingVersionPanel";
import { WritingArchivePanel } from "./WritingArchivePanel";
import type {
  WritingDocumentType,
  WritingRequirementDraft,
  WritingTask,
  WritingTaskStatus,
} from "../types";

type Notice = { kind: "ok" | "err"; text: string };

const STATUS_ORDER: WritingTaskStatus[] = [
  "需求待整理",
  "素材收集中",
  "大纲待确认",
  "写作中",
  "待核验",
  "待审核",
  "已定稿",
  "已归档",
  "已取消",
];

const DOCUMENT_TYPES: WritingDocumentType[] = [
  "月度总结",
  "年度总结",
  "专项汇报",
  "经验交流",
  "典型案例",
  "调研报告",
  "工作方案",
  "领导讲话",
  "工作简报",
  "专题分析",
  "会议发言",
  "个人述职",
];

const STATUS_SKIN: Record<WritingTaskStatus, string> = {
  需求待整理: "text-amber-700 bg-amber-500/15 ring-1 ring-amber-500/25",
  素材收集中: "text-primary bg-primary/10",
  大纲待确认: "text-amber-700 bg-amber-500/10",
  写作中: "text-primary bg-primary/10",
  待核验: "text-orange-700 bg-orange-500/10",
  待审核: "text-amber-700 bg-amber-500/15 ring-1 ring-amber-500/25",
  已定稿: "text-tertiary bg-tertiary/10",
  已归档: "text-outline bg-surface-container",
  已取消: "text-outline bg-surface-container",
};

const FIELD =
  "neu-inset-field w-full rounded-lg px-2.5 py-1.5 text-[11.5px] leading-relaxed text-on-surface outline-none placeholder:text-outline/70";
const BTN_PRIMARY =
  "rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary transition hover:opacity-90 disabled:opacity-40";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function shortTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return (date.getMonth() + 1) + "-" + pad(date.getDate()) + " " + pad(date.getHours()) + ":" + pad(date.getMinutes());
}

function statusFrom(value: string): WritingTaskStatus {
  return STATUS_ORDER.includes(value as WritingTaskStatus)
    ? (value as WritingTaskStatus)
    : "需求待整理";
}

function StatusChip({ value }: { value: string }) {
  const status = statusFrom(value);
  return (
    <span
      className={"shrink-0 rounded-full px-1.5 py-[1px] text-[9.5px] font-medium " + STATUS_SKIN[status]}
      data-writing-task-status-label={status}
    >
      {status}
    </span>
  );
}

function TypePill({ value }: { value: string }) {
  return (
    <span className="shrink-0 rounded-full bg-primary/10 px-1.5 py-[1px] text-[9.5px] text-primary">
      {value}
    </span>
  );
}

function materialCount(task: WritingTask): number {
  return task.selected_material_ids.length + task.selected_knowledge_ids.length;
}

function TaskRow({ task, onOpen }: { task: WritingTask; onOpen: (id: string) => void }) {
  const closed = task.status === "已定稿" || task.status === "已归档";
  return (
    <button
      type="button"
      onClick={() => onOpen(task.writing_id)}
      className={
        "group w-full min-w-0 rounded-xl border px-3 py-2 text-left transition " +
        (closed
          ? "border-surface-variant/20 bg-surface-container-low/30 hover:border-primary/20 hover:bg-surface-container-low"
          : "border-surface-variant/25 bg-surface-container-low/40 hover:border-primary/25 hover:bg-surface-container-low")
      }
      data-writing-task={task.writing_id}
      data-writing-task-status={task.status}
    >
      <span className="flex items-start gap-2 min-w-0">
        <span className="min-w-0 flex-1 text-[12.5px] font-medium leading-snug text-on-surface line-clamp-2 break-words">
          {task.title}
        </span>
        <StatusChip value={task.status} />
      </span>
      <span className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-outline">
        <TypePill value={task.document_type} />
        {task.writing_period ? <span>{task.writing_period}</span> : null}
        <span>素材 {materialCount(task)}</span>
        <span className="ml-auto">{shortTime(task.updated_at)}</span>
      </span>
    </button>
  );
}

/**
 * 写作工作台（OD-WT-610）：写作页「任务」页签。
 *
 * 列表按 601 九态顺序分组，进行中在前、已定稿/已归档置后；类型筛选只在
 * 当前视图上收窄。新建走 604 的解析/创建链路，模型调用沿用首页同一条
 * host.submitTask 通道。任务详情先显示真实元数据，大纲/正文入口调用
 * 606/607 的只读命令，能读则显示简表，读不到如实给出行内外提示。
 */
export function WritingWorkbench() {
  const [items, setItems] = useState<WritingTask[]>([]);
  const [broken, setBroken] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [typeFilter, setTypeFilter] = useState<WritingDocumentType | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);

  const [input, setInput] = useState("");
  const [parsing, setParsing] = useState(false);
  const [parseHint, setParseHint] = useState<string | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [draft, setDraft] = useState<WritingRequirementDraft | null>(null);
  const [answers, setAnswers] = useState<string[]>([]);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .writingList({})
      .then((listing) => {
        if (!alive) return;
        setItems(listing.items);
        setBroken(listing.broken);
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

  const visible = useMemo(
    () => (typeFilter ? items.filter((task) => task.document_type === typeFilter) : items),
    [items, typeFilter],
  );

  const groups = useMemo(() => {
    return STATUS_ORDER.map((status) => ({
      status,
      items: visible
        .filter((task) => task.status === status)
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at)),
    })).filter((group) => group.items.length > 0);
  }, [visible]);

  const selectedTask = selected ? items.find((task) => task.writing_id === selected) ?? null : null;

  function resetCreate() {
    setDraft(null);
    setAnswers([]);
    setParseError(null);
    setParseHint(null);
  }

  function patchDraft<K extends keyof WritingRequirementDraft>(
    key: K,
    value: WritingRequirementDraft[K],
  ) {
    setDraft((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  async function parseRequirement() {
    const text = input.trim();
    if (!text || parsing) return;
    setParsing(true);
    setParseError(null);
    setParseHint(null);
    setDraft(null);
    setAnswers([]);
    setNotice(null);
    try {
      const prep = await shell.writingRequirementPrepare(text);
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      const receipt = await host.submitTask({
        promptText: prep.prompt,
        intent: "material_writing",
        workspaceDir: workspace.workspace_dir,
        userInputText: prep.user_input_text,
      });
      const taskId = receipt.omni_task_id;
      let status = await host.taskStatus(taskId);
      for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(taskId);
        if (i % 10 === 0) setParseHint("模型正在解析需求…已等待 " + (i + 1) + " 秒");
      }
      if (status.status !== "succeeded") {
        throw new Error(
          "解析未完成：" + status.status + (status.note ? " · " + status.note : ""),
        );
      }
      const result = await host.taskResult(taskId);
      if (!result.content.trim()) {
        throw new Error("模型没有返回解析结果。");
      }
      const applied = await shell.writingRequirementApply(result.content);
      setDraft(applied.draft);
      setAnswers(applied.draft.questions.map(() => ""));
    } catch (e) {
      setParseError(errText(e));
    } finally {
      setParsing(false);
      setParseHint(null);
    }
  }

  async function createTask() {
    if (!draft || creating) return;
    const title = draft.title?.trim() ?? "";
    const documentType = draft.document_type;
    if (!title || !documentType) {
      setParseError("文稿类型和标题确定后才能创建任务。");
      return;
    }
    setCreating(true);
    setParseError(null);
    try {
      const extraRequirements = draft.questions
        .map((question, index) => {
          const answer = (answers[index] ?? "").trim();
          return answer ? question + " → " + answer : "";
        })
        .filter(Boolean);
      const created = await shell.writingCreateFromRequirement({
        ...draft,
        title,
        document_type: documentType,
        requirements: [...draft.requirements, ...extraRequirements],
      });
      setDraft(null);
      setAnswers([]);
      setInput("");
      setSelected(null);
      setTypeFilter(null);
      setReloadTick((n) => n + 1);
      setNotice({
        kind: "ok",
        text:
          "已创建「" +
          created.task.title +
          "」。" +
          (created.masked
            ? "敏感信息已脱敏（身份证 " + created.masked_id_card + " / 电话 " + created.masked_phone + "）。"
            : ""),
      });
    } catch (e) {
      setParseError(errText(e));
    } finally {
      setCreating(false);
    }
  }

  if (selectedTask) {
    return (
      <TaskDetail
        task={selectedTask}
        onBack={() => setSelected(null)}
        onChanged={() => setReloadTick((value) => value + 1)}
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-col pt-3" data-writing-workbench="">
      {notice ? (
        <div
          role="status"
          className={
            "mb-2 rounded-xl px-2.5 py-1.5 text-[10.5px] leading-relaxed " +
            (notice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-surface-container text-on-surface-variant")
          }
          data-writing-notice={notice.kind}
        >
          {notice.text}
        </div>
      ) : null}

      <section className="rounded-xl border border-primary/20 bg-primary/5 p-2.5" data-writing-create="">
        {draft ? (
          <div className="flex flex-col gap-1.5" data-writing-draft="">
            <div className="flex items-center gap-1.5">
              <span className="text-[10.5px] text-outline">核对需求</span>
              <button type="button" onClick={resetCreate} className="ml-auto text-[10.5px] text-primary hover:underline">
                重新描述
              </button>
            </div>

            {parseError ? (
              <p className="text-[10.5px] leading-relaxed text-error" data-writing-parse-error="">
                未能完成：{parseError}
              </p>
            ) : null}

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5">
              <select
                value={draft.document_type ?? ""}
                onChange={(event) =>
                  patchDraft(
                    "document_type",
                    (event.target.value || null) as WritingDocumentType | null,
                  )
                }
                className={FIELD}
                aria-label="文稿类型"
                data-writing-draft-type=""
              >
                <option value="">文稿类型</option>
                {DOCUMENT_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {type}
                  </option>
                ))}
              </select>
              <input
                value={draft.title ?? ""}
                onChange={(event) => patchDraft("title", event.target.value || null)}
                className={FIELD}
                placeholder="标题"
                aria-label="写作标题"
                data-writing-draft-title=""
              />
              <input
                value={draft.topic ?? ""}
                onChange={(event) => patchDraft("topic", event.target.value || null)}
                className={FIELD}
                placeholder="主题"
                aria-label="写作主题"
              />
              <input
                value={draft.writing_purpose ?? ""}
                onChange={(event) => patchDraft("writing_purpose", event.target.value || null)}
                className={FIELD}
                placeholder="写作目的"
                aria-label="写作目的"
              />
              <input
                value={draft.audience ?? ""}
                onChange={(event) => patchDraft("audience", event.target.value || null)}
                className={FIELD}
                placeholder="受众"
                aria-label="目标受众"
              />
              <input
                value={draft.writing_period ?? ""}
                onChange={(event) => patchDraft("writing_period", event.target.value || null)}
                className={FIELD}
                placeholder="周期，如 2026-11"
                aria-label="写作周期"
              />
              <input
                value={draft.expected_length ?? ""}
                onChange={(event) => patchDraft("expected_length", event.target.value || null)}
                className={FIELD}
                placeholder="预计篇幅"
                aria-label="预计篇幅"
              />
              <input
                value={draft.writing_style ?? ""}
                onChange={(event) => patchDraft("writing_style", event.target.value || null)}
                className={FIELD}
                placeholder="写作风格"
                aria-label="写作风格"
              />
            </div>

            <textarea
              value={draft.requirements.join("\n")}
              onChange={(event) =>
                patchDraft(
                  "requirements",
                  event.target.value
                    .split("\n")
                    .map((line) => line.trim())
                    .filter(Boolean),
                )
              }
              rows={2}
              className={FIELD}
              placeholder="写作要求，一行一条"
              aria-label="写作要求"
            />

            {draft.questions.length > 0 ? (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/5 p-2" data-writing-questions="">
                <p className="text-[10px] text-amber-700">待补充</p>
                <div className="mt-1 flex flex-col gap-1">
                  {draft.questions.map((question, index) => (
                    <div key={String(index) + "-" + question} className="flex flex-col gap-0.5">
                      <span className="text-[10.5px] leading-relaxed text-on-surface-variant break-words">{question}</span>
                      <input
                        value={answers[index] ?? ""}
                        onChange={(event) =>
                          setAnswers((prev) => {
                            const next = [...prev];
                            next[index] = event.target.value;
                            return next;
                          })
                        }
                        className={FIELD}
                        placeholder="补充这一项"
                        aria-label={"补充：" + question}
                        data-writing-question-answer={index}
                      />
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <p className="text-[10px] text-outline" data-writing-questions-empty="">
                没有待补充问题。
              </p>
            )}

            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => void createTask()}
                disabled={creating || !draft.title?.trim() || !draft.document_type}
                className={BTN_PRIMARY}
                data-writing-create-submit=""
              >
                {creating ? "正在创建…" : "创建任务"}
              </button>
              <span className="text-[10px] text-outline">类型和标题确定后即可创建</span>
            </div>
          </div>
        ) : (
          <>
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              rows={2}
              className={FIELD + " resize-y"}
              placeholder="描述要写什么，例如：十一月份试点推进月度总结，向上级汇报，1200 字左右"
              aria-label="写作需求"
              data-writing-requirement-input=""
            />
            <div className="mt-1.5 flex items-center gap-2">
              <button
                type="button"
                onClick={() => void parseRequirement()}
                disabled={parsing || !input.trim()}
                className={BTN_PRIMARY}
                data-writing-parse=""
              >
                {parsing ? "正在解析…" : "解析需求"}
              </button>
              {parseHint ? (
                <span className="text-[10px] text-outline" data-writing-parse-hint="">
                  {parseHint}
                </span>
              ) : null}
            </div>
            {parseError ? (
              <div className="mt-1.5 flex flex-col gap-1" data-writing-parse-error="">
                <p className="text-[10.5px] leading-relaxed text-error">解析失败：{parseError}</p>
                <button
                  type="button"
                  onClick={() => void parseRequirement()}
                  disabled={parsing || !input.trim()}
                  className="self-start text-[10px] text-primary hover:underline"
                >
                  重试
                </button>
              </div>
            ) : null}
          </>
        )}
      </section>

      <div className="mt-2.5 flex flex-wrap items-center gap-1" data-writing-type-filters="">
        <button
          type="button"
          onClick={() => setTypeFilter(null)}
          aria-pressed={typeFilter === null}
          className={
            "rounded-full px-2.5 py-1 text-[10.5px] transition " +
            (typeFilter === null
              ? "bg-on-surface text-inverse-on-surface"
              : "text-on-surface-variant hover:bg-surface-container")
          }
        >
          全部 {items.length}
        </button>
        {DOCUMENT_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            onClick={() => setTypeFilter(typeFilter === type ? null : type)}
            aria-pressed={typeFilter === type}
            className={
              "rounded-full px-2.5 py-1 text-[10.5px] transition " +
              (typeFilter === type
                ? "bg-on-surface text-inverse-on-surface"
                : "text-on-surface-variant hover:bg-surface-container")
            }
            data-writing-type-filter={type}
          >
            {type}
          </button>
        ))}
        <button
          type="button"
          onClick={() => setReloadTick((n) => n + 1)}
          className="ml-auto rounded-full px-2.5 py-1 text-[10px] text-outline transition hover:bg-surface-container"
          data-writing-refresh=""
        >
          刷新
        </button>
      </div>

      <div className="mt-2.5 min-h-0 flex-1 overflow-y-auto pr-0.5 max-h-[52vh]" data-writing-task-list="">
        {loading ? (
          <p className="text-[11.5px] text-outline">正在读取写作任务…</p>
        ) : loadError ? (
          <div className="flex flex-col gap-1.5" data-writing-list-error="">
            <p className="text-[11.5px] leading-relaxed text-error">读取写作任务失败：{loadError}</p>
            <button
              type="button"
              onClick={() => setReloadTick((n) => n + 1)}
              className="self-start text-[10.5px] text-primary hover:underline"
            >
              重试
            </button>
          </div>
        ) : visible.length === 0 ? (
          <p className="text-[11.5px] leading-relaxed text-outline" data-writing-empty="">
            {typeFilter
              ? "这个类型下还没有任务。"
              : "还没有写作任务。在上面描述要写什么，解析后创建。"}
          </p>
        ) : (
          <div className="flex flex-col gap-2.5">
            {groups.map((group) => (
              <section key={group.status} data-writing-status-group={group.status}>
                <div className="flex items-baseline gap-2 px-1 pb-1">
                  <h3 className="text-[11px] font-medium text-on-surface-variant">{group.status}</h3>
                  <span className="mono text-[9.5px] text-outline">{group.items.length}</span>
                </div>
                <ul className="flex flex-col gap-1.5">
                  {group.items.map((task) => (
                    <li key={task.writing_id}>
                      <TaskRow task={task} onOpen={setSelected} />
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        )}

        {broken > 0 ? (
          <p className="mt-2 text-[10px] text-amber-700" data-writing-list-broken="">
            另有 {broken} 个任务文件读不出来，未计入列表。
          </p>
        ) : null}
      </div>
    </div>
  );
}

type DetailTab = "outline" | "section" | "factcheck" | "versions" | "archive";

const DETAIL_TABS: { key: DetailTab; label: string }[] = [
  { key: "outline", label: "大纲" },
  { key: "section", label: "正文" },
  { key: "factcheck", label: "核验" },
  { key: "versions", label: "版本" },
  { key: "archive", label: "归档" },
];

function TaskDetail({
  task,
  onBack,
  onChanged,
}: {
  task: WritingTask;
  onBack: () => void;
  onChanged?: () => void;
}) {
  const [tab, setTab] = useState<DetailTab>("outline");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setTab("outline");
    setError(null);
  }, [task.writing_id]);

  async function finalizeTask() {
    setError(null);
    try {
      await shell.writingStatusTransition({
        writing_id: task.writing_id,
        to: "已定稿",
        reason: "用户定稿",
      });
      onChanged?.();
    } catch (e) {
      setError(errText(e));
      throw e;
    }
  }

  return (
    <div className="flex min-h-0 flex-col pt-3" data-writing-task-detail={task.writing_id}>
      <div className="flex items-center gap-2 pb-2.5">
        <button type="button" onClick={onBack} className="text-[11px] text-primary hover:underline">
          ← 任务列表
        </button>
        <span className="ml-auto text-[10px] text-outline">{shortTime(task.updated_at)}</span>
      </div>

      <h3 className="text-[14px] font-semibold leading-snug text-on-surface break-words">{task.title}</h3>
      <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
        <StatusChip value={task.status} />
        <TypePill value={task.document_type} />
        {task.writing_period ? (
          <span className="text-[10.5px] text-on-surface-variant">{task.writing_period}</span>
        ) : null}
        <span className="text-[10.5px] text-outline">
          素材 {task.selected_material_ids.length} · 知识 {task.selected_knowledge_ids.length}
        </span>
      </div>

      <div
        className="mt-2.5 flex items-center gap-1.5 border-b border-surface-variant/30 pb-1.5"
        role="tablist"
        aria-label="写作任务视图"
        data-writing-detail-tabs=""
      >
        {DETAIL_TABS.map((item) => (
          <button
            key={item.key}
            type="button"
            role="tab"
            aria-selected={tab === item.key}
            onClick={() => setTab(item.key)}
            className={
              "rounded-full px-2.5 py-1 text-[11px] transition " +
              (tab === item.key
                ? "bg-on-surface text-inverse-on-surface"
                : "text-on-surface-variant hover:bg-surface-container")
            }
            data-writing-detail-tab={item.key}
          >
            {item.label}
          </button>
        ))}
      </div>

      {error ? (
        <p className="mt-2 text-[10.5px] leading-relaxed text-error" data-writing-detail-error="">
          {error}
        </p>
      ) : null}

      <div className="mt-2.5 min-h-0">
        {tab === "outline" ? (
          <WritingOutlineEditor writingId={task.writing_id} onOutlineConfirmed={() => onChanged?.()} />
        ) : tab === "section" ? (
          <WritingSectionEditor writingId={task.writing_id} onSectionConfirmed={() => onChanged?.()} />
        ) : tab === "factcheck" ? (
          <WritingFactCheckPanel writingId={task.writing_id} />
        ) : tab === "versions" ? (
          <WritingVersionPanel
            writingId={task.writing_id}
            status={task.status}
            onRestored={() => onChanged?.()}
            onFinalize={finalizeTask}
          />
        ) : (
          <WritingArchivePanel
            writingId={task.writing_id}
            status={task.status}
            onArchived={() => onChanged?.()}
          />
        )}
      </div>

      {task.writing_purpose ? (
        <p className="mt-2.5 whitespace-pre-wrap text-[10.5px] leading-relaxed text-outline break-words" data-writing-purpose="">
          {task.writing_purpose}
        </p>
      ) : null}
    </div>
  );
}

