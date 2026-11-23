/**
 * 项目甘特的计划与进度几何（OD-PRJ-604）。
 *
 * 全部函数只接收显式时间戳，不读时钟、不读文件，便于表驱动测试。
 * 任务进度按 created_at → due_date 的时间比例计算；项目缺少日期时用
 * 「起点 + 90 天」的默认计划区间，起点优先取项目开始日，其次取组内最早任务。
 */

import type { ProjectInfo, WorkTask } from "./types";

const DAY_MS = 86_400_000;
export const DEFAULT_PROJECT_WINDOW_DAYS = 90;

function parseDayStart(value: string | null | undefined): number | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

function timeRatio(startMs: number, endMs: number, nowMs: number): number {
  if (endMs <= startMs) return 0;
  return Math.max(0, Math.min(1, (nowMs - startMs) / (endMs - startMs)));
}

/**
 * 单个任务的完成比例。
 *
 * * 已完成固定为 1。
 * * 待确认、待开始固定为 0，避免把等待时间画成已完成工作量。
 * * 进行中、已取消、暂停保留当前时间进度。
 * * 无截止日、截止不晚于创建日或日期非法时返回 0。
 */
export function taskProgressRatio(task: WorkTask, nowMs: number): number {
  if (task.state === "已完成") return 1;
  if (task.state === "待确认" || task.state === "待开始") return 0;
  const created = parseDayStart(task.created_at);
  const due = parseDayStart(task.due_date);
  if (created === null || due === null || due <= created) return 0;
  return timeRatio(created, due, nowMs);
}

/**
 * 项目的计划区间。
 *
 * 优先使用 project.start/project.end；缺少结束日时默认 90 天；
 * 缺少开始日时使用 fallbackStartMs。结束日不晚于开始日时同样退回 90 天，
 * 保证计划带始终有可见宽度。
 */
export function projectWindow(
  project: Pick<ProjectInfo, "start" | "end"> | null | undefined,
  fallbackStartMs: number,
): { startMs: number; endMs: number } {
  const startMs = parseDayStart(project?.start) ?? fallbackStartMs;
  const endMs = parseDayStart(project?.end);
  if (endMs === null || endMs <= startMs) {
    return { startMs, endMs: startMs + DEFAULT_PROJECT_WINDOW_DAYS * DAY_MS };
  }
  return { startMs, endMs };
}

/**
 * 项目汇总进度：有项目起止日期时按项目区间，否则按组内任务汇总带区间。
 * 两者都缺时从 nowMs 起算默认 90 天，当前进度为 0。
 */
export function projectProgressRatio(
  project: Pick<ProjectInfo, "start" | "end"> | null | undefined,
  rows: readonly { startMs: number; endMs: number }[],
  nowMs: number,
): number {
  const rowStartMs = rows.length > 0 ? Math.min(...rows.map((row) => row.startMs)) : null;
  const fallbackStartMs = rowStartMs ?? nowMs;
  const { startMs, endMs } = projectWindow(project, fallbackStartMs);
  return timeRatio(startMs, endMs, nowMs);
}
