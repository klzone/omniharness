import { useEffect, useMemo, useRef, useState } from "react";

import { host, shell } from "../api";
import { Icon } from "../components/Icon";
import { TERMINAL_STATES } from "../types";
import type { ProjectInfo, ProjectOverview, ProjectReviewRead, WorkTask } from "../types";
import { MarkdownText } from "./MarkdownText";

type Notice = { kind: "ok" | "err"; text: string };

export type ProjectReviewSeed = { projectId: string; nonce: number } | null;

const CARD = "bg-[color:var(--od-neu-surface)] shadow-[var(--od-neu-raise-1)]";
const TILE = "bg-[color:var(--od-neu-sunken)] shadow-[var(--od-neu-inset-1)]";
const PROMPT_SECTIONS = [
  "项目概况",
  "任务完成情况",
  "问题与不足",
  "经验与做法",
  "后续建议",
];

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

function shortDate(value: string | null): string {
  if (!value) return "未设置";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (n: number) => String(n).padStart(2, "0");
  return date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate());
}

function progressText(task: WorkTask): string {
  const progress = task.progress
    .map((entry) => "- " + entry.at + " " + entry.text.trim())
    .join("\n");
  const history = task.history
    .map(
      (change) =>
        "- " + change.at + " " + change.from + " → " + change.to +
        (change.note ? "（" + change.note + "）" : ""),
    )
    .join("\n");
  return [
    "状态：" + task.state,
    progress ? "进展原文：\n" + progress : "进展原文：（无）",
    history ? "状态历史：\n" + history : "状态历史：（无）",
  ]
    .filter(Boolean)
    .join("\n");
}

function buildMaterial(
  overview: ProjectOverview,
  tasks: WorkTask[],
): { material: string; taskIds: string[] } {
  const project = overview.project;
  const lines: string[] = [
    "项目编号：" + project.project_id,
    "项目名称：" + project.name,
    "项目状态：" + project.status,
    "起止日期：" + shortDate(project.start) + " 至 " + shortDate(project.end),
    "项目目标：" + (project.goal || "材料未填写"),
    "",
    "项目任务（共 " + overview.tasks.length + " 条，本次材料纳入 " + tasks.length + " 条）：",
  ];
  for (const task of tasks) {
    lines.push("", "#" + task.id + "｜" + task.title, progressText(task));
  }
  return { material: lines.join("\n"), taskIds: tasks.map((task) => task.id) };
}

function buildPrompt(material: string, taskIds: string[]): string {
  return [
    "你在为 OmniDesk 项目跟踪写一份项目复盘。只返回 Markdown 正文，不要 frontmatter、",
    "不要代码块、不要解释说明，也不要输出材料之外的事实。",
    "",
    "必须严格按以下五个二级标题、按此顺序输出，每个标题下都要有正文：",
    PROMPT_SECTIONS.map((title) => "## " + title).join("\n"),
    "",
    "引用规则：",
    "- 只能引用材料里出现过的任务编号，写成 #t-…。",
    "- 不要引用材料里没有的编号；没有可引用的内容就不写编号。",
    "- 材料没写的信息不要编造；确实不足时如实写「材料未提供」。",
    "",
    "可引用的任务编号：" + (taskIds.length > 0 ? taskIds.map((id) => "#" + id).join("、") : "（无）"),
    "",
    "材料：",
    material,
  ].join("\n");
}

export function ProjectReviewCard({
  seed,
  onNotice,
  onChanged,
}: {
  seed?: ProjectReviewSeed;
  onNotice?: (notice: Notice | null) => void;
  onChanged?: () => void;
}) {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [selected, setSelected] = useState("");
  const [review, setReview] = useState<ProjectReviewRead | null>(null);
  const [loading, setLoading] = useState(true);
  const [reviewLoading, setReviewLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [stage, setStage] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [tick, setTick] = useState(0);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement | null>(null);

  function announce(next: Notice | null) {
    setNotice(next);
    onNotice?.(next);
  }

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell.projectList()
      .then((listing) => {
        if (!alive) return;
        setProjects(listing.items);
        setError(null);
        setSelected((current) =>
          current && listing.items.some((project) => project.project_id === current)
            ? current
            : listing.items[0]?.project_id ?? "",
        );
      })
      .catch((e) => {
        if (!alive) return;
        setError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [tick]);

  useEffect(() => {
    if (!seed) return;
    setReviewOpen(false);
    if (!seed.projectId) {
      setSelected("");
      return;
    }
    setSelected(seed.projectId);
  }, [seed]);

  useEffect(() => {
    if (!selected) {
      setReview(null);
      setReviewError(null);
      setReviewLoading(false);
      return;
    }
    let alive = true;
    setReviewLoading(true);
    setReview(null);
    shell.projectReviewRead(selected)
      .then((value) => {
        if (!alive) return;
        setReview(value);
        setReviewError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setReview(null);
        setReviewError(errText(e));
      })
      .finally(() => {
        if (alive) setReviewLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected, tick]);

  const selectedProject = useMemo(
    () => projects.find((project) => project.project_id === selected) ?? null,
    [projects, selected],
  );
  const confirmed = review?.status === "已确认";

  useEffect(() => {
    if (!reviewOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setReviewOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [reviewOpen]);

  useEffect(() => {
    if (!pickerOpen) return;
    const onDown = (event: PointerEvent) => {
      if (!pickerRef.current?.contains(event.target as Node)) setPickerOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setPickerOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [pickerOpen]);

  function refresh() {
    setTick((value) => value + 1);
    onChanged?.();
  }

  function selectProject(projectId: string) {
    setSelected(projectId);
    setReviewOpen(false);
    announce(null);
  }

  async function generateReview() {
    if (!selected || busy) return;
    if (confirmed) {
      announce({ kind: "err", text: "该项目复盘已确认，不可再生成覆盖。" });
      return;
    }
    setBusy(true);
    announce(null);
    setStage("正在汇集项目材料…");
    try {
      const overview = await shell.projectOverview(selected);
      const refs = overview.tasks.slice(0, 20);
      const tasks = await Promise.all(refs.map((ref) => shell.taskRead(ref.id)));
      const { material, taskIds } = buildMaterial(overview, tasks);
      const promptText = buildPrompt(material, taskIds);
      setStage("已提交生成任务，等待模型…");
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      const receipt = await host.submitTask({
        promptText,
        intent: "project_management",
        workspaceDir: workspace.workspace_dir,
        userInputText: "项目复盘 " + selected,
      });
      const taskId = receipt.omni_task_id;
      let status = await host.taskStatus(taskId);
      for (let i = 0; i < 1800 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(taskId);
        if (i === 9) setStage("模型正在生成…已等待 10 秒");
        if (i >= 600 && i % 30 === 0) {
          setStage(
            "模型仍在生成…已等待 " + (i + 1) + " 秒（超过 10 分钟，继续等待，不自动取消）",
          );
        }
      }
      if (status.status !== "succeeded") {
        throw new Error(
          "生成未完成：" + status.status + (status.note ? " · " + status.note : "") +
          (TERMINAL_STATES.includes(status.status) ? "" : "；任务仍在引擎侧，可稍后重试"),
        );
      }
      const result = await host.taskResult(taskId);
      if (!result.content.trim()) {
        throw new Error("模型没有返回复盘正文，本次未写入。");
      }
      const saved = await shell.projectReviewSave(selected, taskIds, result.content);
      announce({
        kind: "ok",
        text: "已写入项目复盘草稿" + (saved.backup ? "，旧草稿已备份为 .bak" : "") + "。",
      });
      setReview(null);
      setReviewLoading(true);
      refresh();
      setReviewOpen(true);
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
      setStage(null);
    }
  }

  async function confirmReview() {
    if (!selected || busy || !review?.exists) return;
    if (confirmed) {
      announce({ kind: "err", text: "这份项目复盘已确认，无需再次确认。" });
      return;
    }
    setBusy(true);
    announce(null);
    try {
      await shell.projectReviewConfirm(selected);
      announce({ kind: "ok", text: "已确认归档；确认后不可再生成覆盖。" });
      refresh();
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function stageKnowledge() {
    if (!review?.exists || busy) return;
    setBusy(true);
    announce(null);
    try {
      await shell.knowledgeReviewStage(review.body, [review.path]);
      announce({ kind: "ok", text: "已提交知识审核队列，请到「知识审核」裁决。" });
      onChanged?.();
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
    <aside aria-label="项目复盘" data-project-review-card="" className={"rounded-[18px] px-5 py-4 " + CARD}>
      <div className="flex items-center gap-2 border-b border-[color:var(--od-neu-line-strong)] pb-3">
        <span className="pj-panel-ic" aria-hidden="true">
          <Icon name="book" className="h-3.5 w-3.5" />
        </span>
        <h3 className="sr-only">项目复盘</h3>
        <span className="ml-auto flex items-center gap-1.5">
          {review?.exists ? (
            <button
              type="button"
              onClick={() => setReviewOpen(true)}
              className={"rounded-full px-2.5 py-1 text-[10px] text-[color:var(--od-neu-ink-2)] " + TILE}
              data-project-review-open=""
            >
              查看
            </button>
          ) : null}
          {review?.status ? (
            <span
              className="rounded-full bg-[color:var(--od-neu-accent-soft)] px-2 py-0.5 text-[10px] text-[color:var(--od-neu-accent-ink)]"
              data-project-review-status={review.status}
            >
              {review.status}
            </span>
          ) : null}
        </span>
      </div>

      {loading ? <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">正在读取项目…</p> : null}
      {error ? (
        <p className="mt-3 rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
          读取项目失败：{error}
        </p>
      ) : null}

      {projects.length === 0 && !loading ? (
        <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">先在下方项目区新建项目，再生成项目复盘。</p>
      ) : null}

      {projects.length > 0 ? (
        <div className="pt-3 flex flex-col gap-2.5">
          <div className="pj-picker" ref={pickerRef}>
            <button
              type="button"
              className="pj-picker-trigger"
              aria-haspopup="listbox"
              aria-expanded={pickerOpen}
              data-project-review-select=""
              data-hover-hint="选择复盘项目"
              onClick={() => setPickerOpen((open) => !open)}
            >
              <span className="pj-picker-value">
                {selectedProject ? (
                  <span
                    className="pj-state-dot shrink-0"
                    data-task-state={selectedProject.status}
                    aria-hidden="true"
                  />
                ) : null}
                <span>{selectedProject?.name ?? "（未挂接项目）"}</span>
              </span>
              <Icon
                name={pickerOpen ? "chevron-up" : "chevron-down"}
                className="pj-picker-chev"
              />
            </button>
            {pickerOpen ? (
              <div className="pj-picker-menu" role="listbox" aria-label="选择复盘项目">
                <button
                  type="button"
                  role="option"
                  aria-selected={selected === ""}
                  className={"pj-picker-item" + (selected === "" ? " is-selected" : "")}
                  onClick={() => {
                    selectProject("");
                    setPickerOpen(false);
                  }}
                >
                  <span className="pj-picker-item-name">（未挂接项目）</span>
                  {selected === "" ? <Icon name="check" className="h-3 w-3" /> : null}
                </button>
                {projects.map((project) => (
                  <button
                    key={project.project_id}
                    type="button"
                    role="option"
                    aria-selected={selected === project.project_id}
                    className={
                      "pj-picker-item" + (selected === project.project_id ? " is-selected" : "")
                    }
                    onClick={() => {
                      selectProject(project.project_id);
                      setPickerOpen(false);
                    }}
                  >
                    <span
                      className="pj-state-dot shrink-0"
                      data-task-state={project.status}
                      aria-hidden="true"
                    />
                    <span className="pj-picker-item-name">{project.name}</span>
                    {selected === project.project_id ? <Icon name="check" className="h-3 w-3" /> : null}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
          {!selected ? (
            <p className="rounded-xl bg-[color:var(--od-neu-sunken)] px-3 py-2 text-[10.5px] leading-relaxed text-[color:var(--od-neu-ink-2)]">
              未挂接项目的任务请先在下方「项目」区挂接，再生成复盘。
            </p>
          ) : null}
          {selectedProject ? (
            <p className="flex items-center gap-1.5 text-[10px] text-[color:var(--od-neu-ink-3)]">
              <span
                className="pj-state-dot shrink-0"
                data-task-state={selectedProject.status}
                title={selectedProject.status}
                aria-label={selectedProject.status}
              />
              <span className="mono">
                {shortDate(selectedProject.start)} → {shortDate(selectedProject.end)}
              </span>
            </p>
          ) : null}
        </div>
      ) : null}

      {reviewLoading ? <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">正在读取项目复盘…</p> : null}
      {reviewError ? (
        <p className="mt-3 rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
          读取项目复盘失败：{reviewError}
        </p>
      ) : null}

      {stage ? (
        <p className="mt-3 rounded-xl bg-[color:var(--od-neu-sunken)] px-3 py-2 text-[11px] leading-relaxed text-[color:var(--od-neu-ink-2)]" role="status">
          {stage}
        </p>
      ) : null}
      {notice ? (
        <p
          role="status"
          className={
            "mt-3 rounded-xl px-3 py-2 text-[11px] leading-relaxed " +
            (notice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-[color:var(--od-neu-sunken)] text-[color:var(--od-neu-ink-2)]")
          }
        >
          {notice.text}
        </p>
      ) : null}

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={!selected || busy || confirmed}
          onClick={() => void generateReview()}
          className="inline-flex items-center gap-1.5 rounded-full bg-[color:var(--od-neu-accent)] px-4 py-1.5 text-[11px] font-medium text-white shadow-[var(--od-neu-raise-1)] disabled:opacity-40"
          data-project-review-generate=""
        >
          <Icon name="spark" className="h-3 w-3" />
          {busy ? "生成中…" : review?.exists ? "重新生成" : "AI 复盘"}
        </button>
      </div>
      {confirmed ? (
        <p className="mt-2 text-[10px] text-[color:var(--od-neu-ink-3)]">已确认的复盘不可再生成覆盖。</p>
      ) : null}
    </aside>

    {reviewOpen && selected ? (
      <div
        className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4"
        role="presentation"
        onClick={() => setReviewOpen(false)}
      >
        <div
          role="dialog"
          aria-modal="true"
          aria-label="项目复盘"
          onClick={(event) => event.stopPropagation()}
          className="wr-dialog flex flex-col overflow-hidden"
          style={{ width: "min(76vw, 1100px)", height: "76vh" }}
          data-project-review-dialog=""
        >
          <div className="wr-dialog-head shrink-0">
            <span className="wr-dialog-mark" aria-hidden="true">
              <Icon name="book" className="h-4 w-4" />
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-[10.5px] text-[color:var(--od-neu-ink-3)]">项目复盘</p>
              <h3 title={selectedProject?.name ?? selected} className="truncate text-[14px] font-semibold text-[color:var(--od-neu-ink)]">
                {selectedProject?.name ?? selected}
              </h3>
              {review?.exists ? (
                <p className="mono mt-0.5 text-[9.5px] text-[color:var(--od-neu-ink-3)]">
                  生成 {review.generated_at ? shortDate(review.generated_at) : "未知"} · 任务 {review.task_ids.length} 条
                </p>
              ) : null}
            </div>
            {review?.status ? (
              <span
                className="rounded-full bg-[color:var(--od-neu-accent-soft)] px-2.5 py-0.5 text-[10px] text-[color:var(--od-neu-accent-ink)]"
                data-project-review-dialog-status={review.status}
              >
                {review.status}
              </span>
            ) : null}
            <button
              type="button"
              aria-label="关闭"
              data-hover-hint="关闭"
              onClick={() => setReviewOpen(false)}
              className={"flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[color:var(--od-neu-ink-3)] " + TILE}
              data-project-review-close=""
            >
              <Icon name="x" className="h-3.5 w-3.5" />
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-7 py-5">
            {reviewLoading ? (
              <p className="text-[11px] text-[color:var(--od-neu-ink-3)]">正在读取项目复盘…</p>
            ) : reviewError ? (
              <p className="rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
                读取项目复盘失败：{reviewError}
              </p>
            ) : review?.exists ? (
              <div className="od-project-review-body">
                <MarkdownText text={review.body} />
              </div>
            ) : (
              <p className="text-[11px] text-[color:var(--od-neu-ink-3)]">暂无项目复盘。</p>
            )}
            {notice ? (
              <p
                role="status"
                className={
                  "mt-4 rounded-xl px-3 py-2 text-[11px] leading-relaxed " +
                  (notice.kind === "err"
                    ? "bg-error-container/70 text-on-error-container"
                    : "bg-[color:var(--od-neu-sunken)] text-[color:var(--od-neu-ink-2)]")
                }
              >
                {notice.text}
              </p>
            ) : null}
          </div>
          {review?.exists ? (
            <div className="wr-dialog-foot shrink-0 justify-end">
              {!confirmed ? (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void confirmReview()}
                  className={"rounded-full px-4 py-1.5 text-[11px] text-[color:var(--od-neu-accent-ink)] " + TILE}
                  data-project-review-confirm=""
                >
                  确认归档
                </button>
              ) : null}
              <button
                type="button"
                disabled={busy}
                onClick={() => void stageKnowledge()}
                className={"rounded-full px-4 py-1.5 text-[11px] text-[color:var(--od-neu-ink-2)] " + TILE}
                data-project-review-knowledge=""
              >
                提炼知识卡
              </button>
            </div>
          ) : null}
        </div>
      </div>
    ) : null}
    </>
  );
}
