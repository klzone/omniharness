/**
 * 项目甘特的分组行纯函数（OD-PRJ-602 S1）。
 *
 * 输入任务列表、项目列表与时间窗，输出「每个有任务的项目一组」的折叠行：
 * 组头汇总带 = 组内最早条左端 → 最晚条右端；组内每个任务一行。
 * 不碰 React、不读时钟、不读文件，同一份输入永远给同一份输出。
 *
 * 坐标口径与旧 ganttRows 一致：created_at 的当天 → due_date 的当天，
 * 两端夹取到时间窗，宽度下限 2%，期限越过右端标记 clamped。
 */

import type { ProjectInfo, WorkTask } from "./types";

/** 甘特时间窗（毫秒时间戳，避免把 Date 对象带进纯函数比较）。 */
export interface TaskGanttTimeline {
  start: number;
  end: number;
  span: number;
}

/** 组内一个任务条；left/width 是百分比，clamped 表示期限越过窗口右端。 */
export interface ProjectGanttRow {
  task: WorkTask;
  left: number;
  width: number;
  clamped: boolean;
  /** 夹取后的起止本地日（毫秒）。 */
  startMs: number;
  endMs: number;
}

/** 一个项目的分组；summary 是浅色汇总带的几何与可见区间文字。 */
export interface ProjectGanttGroup {
  projectId: string;
  projectName: string;
  summary: {
    /** 可见区间起止日，YYYY-MM-DD。 */
    start: string;
    end: string;
    leftPct: number;
    widthPct: number;
  };
  rows: ProjectGanttRow[];
}

/** 没有项目或项目已不存在时的特殊组；空字符串与宿主「未挂接」口径一致。 */
export const UNATTACHED_PROJECT_ID = "";
export const UNATTACHED_PROJECT_NAME = "未挂接项目";

/** 总任务行超过这个数时自动折叠部分组（任务书 OD-PRJ-602 S1.5）。 */
export const AUTO_COLLAPSE_ROW_LIMIT = 30;

const MIN_BAR_WIDTH_PCT = 2;

/** 取本地当天零点；非法日期返回 null（与界面 dayStart 同口径）。 */
function dayStartMs(value: string | null | undefined): number | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

/** 本地零点毫秒转 YYYY-MM-DD，用于组头区间文字。 */
function localIsoDay(ms: number): string {
  const date = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate());
}

/**
 * 单个任务条的几何。
 *
 * 只画带截止日期的任务：created_at → due_date；没有合法截止日期返回 null。
 * 完全落在窗口外的任务不出现，部分越界的条夹到窗口边缘。
 */
export function layoutTaskBar(
  task: WorkTask,
  timeline: TaskGanttTimeline,
): ProjectGanttRow | null {
  if (!task.due_date) return null;
  const dueStart = dayStartMs(task.due_date);
  if (dueStart === null) return null;
  const createdStart = dayStartMs(task.created_at) ?? timeline.start;
  const from = Math.max(createdStart, timeline.start);
  const to = Math.min(dueStart, timeline.end);
  if (to <= timeline.start || from >= timeline.end) return null;
  const left = ((from - timeline.start) / timeline.span) * 100;
  const width = Math.max(MIN_BAR_WIDTH_PCT, ((to - from) / timeline.span) * 100);
  return {
    task,
    left,
    width,
    clamped: dueStart >= timeline.end,
    startMs: from,
    endMs: to,
  };
}

/**
 * 组内排序：按条左端，再按右端，最后按标题，保证同一输入稳定有序。
 * 重叠任务各占一行，不合并、不伪造进度。
 */
function compareRows(a: ProjectGanttRow, b: ProjectGanttRow): number {
  if (a.left !== b.left) return a.left - b.left;
  if (a.endMs !== b.endMs) return a.endMs - b.endMs;
  return a.task.title.localeCompare(b.task.title, "zh-Hans-CN");
}

/**
 * 按项目分组排布甘特行。
 *
 * * 每个有可见任务的项目一个组，组名取项目实体 name。
 * * project_id 为空、或项目已不在列表里的任务，归入「未挂接项目」组。
 * * 没有可见任务的项目不出现。
 * * 组按汇总带左端排序，组内按任务条左端排序。
 */
export function buildProjectGanttGroups(
  tasks: WorkTask[],
  projects: ProjectInfo[],
  timeline: TaskGanttTimeline,
): ProjectGanttGroup[] {
  const projectNames = new Map<string, string>();
  for (const project of projects) {
    projectNames.set(project.project_id, project.name);
  }

  const rowsByProject = new Map<string, ProjectGanttRow[]>();
  for (const task of tasks) {
    const row = layoutTaskBar(task, timeline);
    if (!row) continue;
    const projectId =
      task.project_id && projectNames.has(task.project_id) ? task.project_id : UNATTACHED_PROJECT_ID;
    const bucket = rowsByProject.get(projectId);
    if (bucket) bucket.push(row);
    else rowsByProject.set(projectId, [row]);
  }

  const groups: ProjectGanttGroup[] = [];
  for (const [projectId, rows] of rowsByProject) {
    rows.sort(compareRows);
    const leftPct = Math.min(...rows.map((row) => row.left));
    const rightPct = Math.max(...rows.map((row) => row.left + row.width));
    const startMs = Math.min(...rows.map((row) => row.startMs));
    const endMs = Math.max(...rows.map((row) => row.endMs));
    groups.push({
      projectId,
      projectName:
        projectId === UNATTACHED_PROJECT_ID
          ? UNATTACHED_PROJECT_NAME
          : projectNames.get(projectId) ?? UNATTACHED_PROJECT_NAME,
      summary: {
        start: localIsoDay(startMs),
        end: localIsoDay(endMs),
        leftPct,
        widthPct: Math.max(MIN_BAR_WIDTH_PCT, rightPct - leftPct),
      },
      rows,
    });
  }

  groups.sort((a, b) => {
    if (a.summary.leftPct !== b.summary.leftPct) return a.summary.leftPct - b.summary.leftPct;
    if (a.summary.end !== b.summary.end) return a.summary.end.localeCompare(b.summary.end);
    return a.projectName.localeCompare(b.projectName, "zh-Hans-CN");
  });
  return groups;
}

/**
 * 行数超过上限时建议默认折叠的组：未挂接组 + 最早结束的组。
 * 返回组 id 列表；未超过上限返回空数组（界面据此提示「已折叠 N 组」）。
 */
export function autoCollapseGroups(groups: ProjectGanttGroup[]): string[] {
  const totalRows = groups.reduce((sum, group) => sum + group.rows.length, 0);
  if (totalRows <= AUTO_COLLAPSE_ROW_LIMIT) return [];
  const ids: string[] = [];
  if (groups.some((group) => group.projectId === UNATTACHED_PROJECT_ID)) {
    ids.push(UNATTACHED_PROJECT_ID);
  }
  const earliest = groups
    .filter((group) => group.rows.length > 0)
    .slice()
    .sort((a, b) => {
      if (a.summary.end !== b.summary.end) return a.summary.end.localeCompare(b.summary.end);
      return a.projectId.localeCompare(b.projectId);
    })[0];
  if (earliest && !ids.includes(earliest.projectId)) ids.push(earliest.projectId);
  return ids;
}

/** 组头区间文字：2026-11-01 → 11.01；跨年时保留年。 */
export function summaryRangeLabel(summary: ProjectGanttGroup["summary"]): string {
  const toMd = (day: string) => day.slice(5).replace("-", ".");
  if (summary.start.slice(0, 4) !== summary.end.slice(0, 4)) {
    return summary.start.replace(/-/g, ".") + " → " + summary.end.replace(/-/g, ".");
  }
  return toMd(summary.start) + " → " + toMd(summary.end);
}
