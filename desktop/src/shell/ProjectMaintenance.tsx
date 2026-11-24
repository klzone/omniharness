import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { ProjectInfo, ProjectOverview, ProjectStatus, WorkTask } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";

type Notice = { kind: "ok" | "err"; text: string };
type ProjectAction =
  | { kind: "reassign"; taskId: string; title: string; projectId: string; label: string }
  | { kind: "delete"; taskId: string; title: string };

const PAGE_SIZE = 8;
const NEU_CARD =
  "bg-[color:var(--od-neu-surface)] shadow-[var(--od-neu-raise-1)]";
const NEU_TILE = "bg-[color:var(--od-neu-sunken)] shadow-[var(--od-neu-inset-1)]";
const NEU_INSET = "bg-[color:var(--od-neu-sunken)] shadow-[var(--od-neu-inset-1)]";
const STATUSES: ProjectStatus[] = ["进行中", "已完成", "已搁置"];

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

function ProjectStatusChip({ status }: { status: string }) {
  const tone =
    status === "已完成"
      ? "bg-[color:var(--od-neu-accent-soft)] text-[color:var(--od-neu-accent-ink)]"
      : status === "已搁置"
        ? "bg-[color:var(--od-neu-sunken)] text-[color:var(--od-neu-ink-3)]"
        : "bg-[color:var(--od-neu-accent-soft)] text-[color:var(--od-neu-accent-ink)]";
  return (
    <span className={"shrink-0 rounded-full px-2 py-0.5 text-[10px] " + tone} data-project-status={status}>
      {status}
    </span>
  );
}

function TaskStateChip({ state }: { state: WorkTask["state"] }) {
  return <span className="shrink-0 text-[10px] text-[color:var(--od-neu-ink-3)]">{state}</span>;
}

export function ProjectMaintenance({
  onNotice,
  onChanged,
}: {
  onNotice?: (notice: Notice | null) => void;
  onChanged?: () => void;
}) {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [selected, setSelected] = useState("");
  const [overview, setOverview] = useState<ProjectOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [overviewLoading, setOverviewLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [tick, setTick] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [pending, setPending] = useState<ProjectAction | null>(null);

  const [newProjectOpen, setNewProjectOpen] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [projectGoal, setProjectGoal] = useState("");
  const [projectStatus, setProjectStatus] = useState<ProjectStatus>("进行中");
  const [projectStart, setProjectStart] = useState("");
  const [projectEnd, setProjectEnd] = useState("");
  const [projectError, setProjectError] = useState<string | null>(null);

  const [addTaskOpen, setAddTaskOpen] = useState(false);
  const [addMode, setAddMode] = useState<"existing" | "new">("existing");
  const [existingTaskId, setExistingTaskId] = useState("");
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [newTaskDue, setNewTaskDue] = useState("");
  const [addTaskError, setAddTaskError] = useState<string | null>(null);

  function announce(next: Notice | null) {
    setNotice(next);
    onNotice?.(next);
  }

  /** OD-UI-900：两个弹层都支持 Escape 关闭。 */
  useEffect(() => {
    if (!newProjectOpen && !addTaskOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setNewProjectOpen(false);
        setAddTaskOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [newProjectOpen, addTaskOpen]);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    Promise.all([shell.projectList(), shell.taskList()])
      .then(([listing, taskListing]) => {
        if (!alive) return;
        setProjects(listing.items);
        setTasks(taskListing.items);
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
    if (!selected) {
      setOverview(null);
      setOverviewError(null);
      setOverviewLoading(false);
      return;
    }
    let alive = true;
    setOverviewLoading(true);
    setOverview(null);
    shell.projectOverview(selected)
      .then((value) => {
        if (!alive) return;
        setOverview(value);
        setOverviewError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setOverview(null);
        setOverviewError(errText(e));
      })
      .finally(() => {
        if (alive) setOverviewLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected, tick]);

  const selectedProject = projects.find((project) => project.project_id === selected) ?? null;
  const taskMap = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);
  const projectRows = useMemo(() => {
    if (!overview) return [];
    return overview.tasks.map((ref) => ({
      id: ref.id,
      title: taskMap.get(ref.id)?.title ?? ref.title,
      state: taskMap.get(ref.id)?.state ?? null,
      due: taskMap.get(ref.id)?.due_date ?? null,
      projectId: taskMap.get(ref.id)?.project_id ?? overview.project.project_id,
    }));
  }, [overview, taskMap]);

  const totalPages = Math.max(1, Math.ceil(projectRows.length / PAGE_SIZE));
  const pageRows = projectRows.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  useEffect(() => {
    setPage((current) => Math.min(current, Math.max(1, Math.ceil(projectRows.length / PAGE_SIZE))));
  }, [projectRows.length]);

  const unassignedTasks = useMemo(
    () =>
      tasks
        .filter((task) => !task.project_id && task.state !== "已完成" && task.state !== "已取消")
        .sort((a, b) => (b.updated_at > a.updated_at ? 1 : -1)),
    [tasks],
  );

  const projectNameOf = (projectId: string) =>
    projects.find((project) => project.project_id === projectId)?.name ?? projectId;

  function refresh() {
    setTick((value) => value + 1);
    onChanged?.();
  }

  function changeProject(projectId: string) {
    setSelected(projectId);
    setPage(1);
    announce(null);
  }

  async function createProject() {
    if (busy) return;
    const name = projectName.trim();
    if (!name) {
      setProjectError("项目名称必填。");
      return;
    }
    setBusy(true);
    setProjectError(null);
    try {
      const created = await shell.projectCreate({
        name,
        goal: projectGoal.trim(),
        status: projectStatus,
        start: projectStart || null,
        end: projectEnd || null,
      });
      setNewProjectOpen(false);
      setProjectName("");
      setProjectGoal("");
      setProjectStatus("进行中");
      setProjectStart("");
      setProjectEnd("");
      setSelected(created.project_id);
      refresh();
      announce({ kind: "ok", text: "已新建项目「" + created.name + "」。" });
    } catch (e) {
      setProjectError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function addTask() {
    if (!selected || busy) return;
    setBusy(true);
    setAddTaskError(null);
    try {
      if (addMode === "existing") {
        if (!existingTaskId) {
          throw new Error("请先选择要挂接的任务。");
        }
        await shell.taskUpdate(existingTaskId, { project_id: selected });
        announce({ kind: "ok", text: "任务已挂接到「" + projectNameOf(selected) + "」。" });
      } else {
        const title = newTaskTitle.trim();
        if (!title) throw new Error("任务标题必填。");
        await shell.taskCreate({
          title,
          due_date: newTaskDue || null,
          project_id: selected,
        });
        announce({ kind: "ok", text: "已新建任务并挂接到「" + projectNameOf(selected) + "」。" });
      }
      setAddTaskOpen(false);
      setExistingTaskId("");
      setNewTaskTitle("");
      setNewTaskDue("");
      refresh();
    } catch (e) {
      setAddTaskError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirmPending() {
    if (!pending || busy) return;
    setBusy(true);
    try {
      if (pending.kind === "delete") {
        await shell.taskDelete(pending.taskId);
        announce({ kind: "ok", text: "已删除任务「" + pending.title + "」，文件在数据根垃圾桶。" });
      } else {
        await shell.taskUpdate(pending.taskId, { project_id: pending.projectId });
        announce({
          kind: "ok",
          text:
            "任务「" + pending.title + "」已" +
            (pending.projectId ? "换绑到「" + pending.label + "」。" : "摘除项目挂接。"),
        });
      }
      setPending(null);
      refresh();
    } catch (e) {
      announce({ kind: "err", text: errText(e) });
      setPending(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <aside aria-label="项目维护" data-project-maintenance="" className={"rounded-[18px] px-5 py-4 " + NEU_CARD}>
      <div className="flex items-center gap-2 border-b border-[color:var(--od-neu-line-strong)] pb-3">
        <span className="pj-panel-ic" aria-hidden="true">
          <Icon name="folder" className="h-3.5 w-3.5" />
        </span>
        <h3 className="sr-only">项目</h3>
        <span className="mono ml-auto text-[10px] text-[color:var(--od-neu-ink-3)]">{projects.length}</span>
        <button
          type="button"
          onClick={() => {
            setNewProjectOpen(true);
            setProjectError(null);
          }}
          className={"flex items-center gap-1 rounded-full px-2.5 py-1 text-[10.5px] text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
          data-project-new=""
        >
          <Icon name="plus" className="h-3 w-3" />
        </button>
      </div>

      {loading ? <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">正在读取项目…</p> : null}
      {error ? (
        <p className="mt-3 rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
          读取项目失败：{error}
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

      {!loading && !error && projects.length === 0 ? (
        <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">还没有项目，先新建一个。</p>
      ) : null}

      {projects.length > 0 ? (
        <div className="pt-3 flex flex-col gap-2.5">
          <select
            value={selected}
            onChange={(event) => changeProject(event.target.value)}
            aria-label="选择项目"
            data-project-select=""
            className={"h-9 rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
          >
            {projects.map((project) => (
              <option key={project.project_id} value={project.project_id}>
                {project.name}
              </option>
            ))}
          </select>

          {selectedProject ? (
            <div className={"rounded-xl px-3 py-2.5 " + NEU_INSET} data-project-summary={selectedProject.project_id}>
              <div className="flex items-center gap-2">
                <span title={selectedProject.name} className="min-w-0 flex-1 truncate text-[12px] font-semibold text-[color:var(--od-neu-ink)]">
                  {selectedProject.name}
                </span>
                <ProjectStatusChip status={selectedProject.status} />
                <button
                  type="button"
                  onClick={() => {
                    setAddTaskOpen(true);
                    setAddTaskError(null);
                  }}
                  className={"flex h-7 w-7 items-center justify-center rounded-full text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
                  data-project-add-task=""
                  aria-label={"添加任务到 " + selectedProject.name}
                  title="添加任务"
                >
                  <Icon name="plus" className="h-3.5 w-3.5" />
                </button>
              </div>
              <p className="mono mt-1 text-[10px] text-[color:var(--od-neu-ink-3)]">
                {shortDate(selectedProject.start)} → {shortDate(selectedProject.end)}
              </p>
              {selectedProject.goal ? (
                <p className="mt-1 text-[11px] leading-relaxed text-[color:var(--od-neu-ink-2)]">{selectedProject.goal}</p>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}

      {overviewLoading ? <p className="pt-3 text-[11px] text-[color:var(--od-neu-ink-3)]">正在读取项目任务…</p> : null}
      {overviewError ? (
        <p className="mt-3 rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">
          读取项目任务失败：{overviewError}
        </p>
      ) : null}

      {overview ? (
        <div className="pt-3" data-project-tasks="">
          <div className="flex items-center justify-between gap-2 pb-1.5">
            <span className="text-[10.5px] text-[color:var(--od-neu-ink-3)]">项目任务 · {projectRows.length}</span>
            <span className="mono text-[10px] text-[color:var(--od-neu-ink-3)]">
              {page} / {totalPages}
            </span>
          </div>
          {projectRows.length === 0 ? (
            <p className="pt-2 text-[11px] text-[color:var(--od-neu-ink-3)]">这个项目还没有任务。</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {pageRows.map((row) => (
                <li
                  key={row.id}
                  data-project-task-row={row.id}
                  className="flex items-center gap-2 rounded-lg bg-[color:var(--od-neu-surface)] px-2 py-1.5 shadow-[var(--od-neu-raise-1)]"
                >
                  <span className="min-w-0 flex-1">
                    <span title={row.title} className="block truncate text-[11.5px] font-medium text-[color:var(--od-neu-ink)]">
                      {row.title}
                    </span>
                    <span className="mono mt-0.5 block truncate text-[9.5px] text-[color:var(--od-neu-ink-3)]">
                      {row.due ? "截止 " + shortDate(row.due) : "未设截止"} · {projectNameOf(row.projectId)}
                    </span>
                  </span>
                  {row.state ? <TaskStateChip state={row.state} /> : null}
                  <select
                    value={row.projectId}
                    aria-label={"调整 " + row.title + " 的项目"}
                    onChange={(event) => {
                      const projectId = event.target.value;
                      setPending({
                        kind: "reassign",
                        taskId: row.id,
                        title: row.title,
                        projectId,
                        label: projectId ? projectNameOf(projectId) : "",
                      });
                    }}
                    className={"h-8 max-w-[92px] rounded-full px-2 text-[10px] text-[color:var(--od-neu-ink-2)] " + NEU_INSET}
                  >
                    <option value="">（未挂接）</option>
                    {projects.map((project) => (
                      <option key={project.project_id} value={project.project_id}>
                        {project.name}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    aria-label={"删除任务 " + row.title}
                    onClick={() => setPending({ kind: "delete", taskId: row.id, title: row.title })}
                    className={"flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[color:var(--od-neu-ink-3)] " + NEU_TILE}
                  >
                    <Icon name="trash" className="h-3.5 w-3.5" />
                  </button>
                </li>
              ))}
            </ul>
          )}
          {projectRows.length > PAGE_SIZE ? (
            <div className="flex items-center justify-center gap-2 pt-2">
              <button
                type="button"
                disabled={page <= 1}
                onClick={() => setPage((value) => Math.max(1, value - 1))}
                className={"rounded-full px-2.5 py-1 text-[10.5px] text-[color:var(--od-neu-ink-2)] disabled:opacity-40 " + NEU_TILE}
                data-project-page-prev=""
              >
                上一页
              </button>
              <button
                type="button"
                disabled={page >= totalPages}
                onClick={() => setPage((value) => Math.min(totalPages, value + 1))}
                className={"rounded-full px-2.5 py-1 text-[10.5px] text-[color:var(--od-neu-ink-2)] disabled:opacity-40 " + NEU_TILE}
                data-project-page-next=""
              >
                下一页
              </button>
            </div>
          ) : null}
        </div>
      ) : null}

      {newProjectOpen ? (
        <div className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4" role="presentation" onClick={() => setNewProjectOpen(false)}>
          <div
            role="dialog"
            aria-modal="true"
            aria-label="新建项目"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog w-full max-w-[460px] p-5"
            data-project-dialog="new"
          >
            <h3 className="text-[14px] font-semibold text-on-surface">新建项目</h3>
            <div className="mt-3 flex flex-col gap-3">
              <label className="block">
                <span className="block pb-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">名称</span>
                <input
                  value={projectName}
                  onChange={(event) => setProjectName(event.target.value)}
                  placeholder="项目名称"
                  className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                  data-project-name=""
                />
              </label>
              <label className="block">
                <span className="block pb-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">目标</span>
                <textarea
                  value={projectGoal}
                  onChange={(event) => setProjectGoal(event.target.value)}
                  placeholder="这个项目要达成什么（可选）"
                  rows={3}
                  className={"w-full resize-none rounded-xl px-3 py-2 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                />
              </label>
              <div className="grid grid-cols-2 gap-3">
                <label className="block">
                  <span className="block pb-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">开始日期</span>
                  <input
                    type="date"
                    value={projectStart}
                    onChange={(event) => setProjectStart(event.target.value)}
                    className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                  />
                </label>
                <label className="block">
                  <span className="block pb-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">结束日期</span>
                  <input
                    type="date"
                    value={projectEnd}
                    onChange={(event) => setProjectEnd(event.target.value)}
                    className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                  />
                </label>
              </div>
              <label className="block">
                <span className="block pb-1 text-[10.5px] text-[color:var(--od-neu-ink-3)]">状态</span>
                <select
                  value={projectStatus}
                  onChange={(event) => setProjectStatus(event.target.value as ProjectStatus)}
                  className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                >
                  {STATUSES.map((status) => (
                    <option key={status} value={status}>
                      {status}
                    </option>
                  ))}
                </select>
              </label>
              {projectError ? (
                <p className="rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">{projectError}</p>
              ) : null}
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setNewProjectOpen(false)}
                className={"rounded-full px-4 py-2 text-[11.5px] text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                取消
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => void createProject()}
                className="rounded-full bg-[color:var(--od-neu-accent)] px-5 py-2 text-[11.5px] font-medium text-white shadow-[var(--od-neu-raise-1)] disabled:opacity-50"
              >
                {busy ? "创建中…" : "创建项目"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {addTaskOpen ? (
        <div className="pj-modal-overlay fixed inset-0 z-50 flex items-center justify-center px-4" role="presentation" onClick={() => setAddTaskOpen(false)}>
          <div
            role="dialog"
            aria-modal="true"
            aria-label="添加任务"
            onClick={(event) => event.stopPropagation()}
            className="wr-dialog w-full max-w-[460px] p-5"
            data-project-dialog="add-task"
          >
            <h3 className="text-[14px] font-semibold text-on-surface">添加到「{projectNameOf(selected)}」</h3>
            <div className="mt-3 flex gap-2">
              {(["existing", "new"] as const).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  onClick={() => setAddMode(mode)}
                  className={
                    "rounded-full px-3 py-1.5 text-[11px] " +
                    (addMode === mode
                      ? "bg-[color:var(--od-neu-accent-soft)] font-medium text-[color:var(--od-neu-accent-ink)]"
                      : NEU_TILE + " text-[color:var(--od-neu-ink-2)]")
                  }
                >
                  {mode === "existing" ? "挂接现有任务" : "新建任务"}
                </button>
              ))}
            </div>
            <div className="mt-3 flex flex-col gap-3">
              {addMode === "existing" ? (
                unassignedTasks.length === 0 ? (
                  <p className="text-[11px] text-[color:var(--od-neu-ink-3)]">没有未挂接项目的活动任务，可改用「新建任务」。</p>
                ) : (
                  <select
                    value={existingTaskId}
                    onChange={(event) => setExistingTaskId(event.target.value)}
                    aria-label="选择现有任务"
                    className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                    data-project-existing-task=""
                  >
                    <option value="">选择任务</option>
                    {unassignedTasks.map((task) => (
                      <option key={task.id} value={task.id}>
                        {task.title}（{task.state}）
                      </option>
                    ))}
                  </select>
                )
              ) : (
                <>
                  <input
                    value={newTaskTitle}
                    onChange={(event) => setNewTaskTitle(event.target.value)}
                    placeholder="任务标题"
                    className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                    data-project-task-title=""
                  />
                  <input
                    type="date"
                    value={newTaskDue}
                    onChange={(event) => setNewTaskDue(event.target.value)}
                    aria-label="任务截止日期"
                    className={"h-9 w-full rounded-full px-3 text-[11.5px] text-[color:var(--od-neu-ink)] " + NEU_INSET}
                  />
                </>
              )}
              {addTaskError ? (
                <p className="rounded-xl bg-error-container/70 px-3 py-2 text-[11px] text-on-error-container">{addTaskError}</p>
              ) : null}
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setAddTaskOpen(false)}
                className={"rounded-full px-4 py-2 text-[11.5px] text-[color:var(--od-neu-ink-2)] " + NEU_TILE}
              >
                取消
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => void addTask()}
                className="rounded-full bg-[color:var(--od-neu-accent)] px-5 py-2 text-[11.5px] font-medium text-white shadow-[var(--od-neu-raise-1)] disabled:opacity-50"
              >
                {busy ? "提交中…" : addMode === "existing" ? "挂接任务" : "创建并挂接"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <ConfirmDialog
        open={pending !== null}
        title={
          pending?.kind === "delete"
            ? "删除任务？"
            : pending?.projectId
              ? "换绑任务？"
              : "摘除项目挂接？"
        }
        description={
          pending?.kind === "delete"
            ? "将删除任务「" + pending.title + "」的文件，可在数据根垃圾桶找回。"
            : "任务「" + (pending?.title ?? "") + "」将" +
              (pending?.projectId ? "换绑到「" + pending.label + "」。" : "从当前项目摘除。")
        }
        confirmLabel={pending?.kind === "delete" ? "删除" : pending?.projectId ? "换绑" : "摘除"}
        onConfirm={() => void confirmPending()}
        onCancel={() => setPending(null)}
        testId="project-task-confirm"
      />
    </aside>
  );
}
