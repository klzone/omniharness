/**
 * OD-PRJ-604：甘特计划/进度纯函数的表驱动单测。
 * 不读磁盘、不读时钟；所有 now 都在用例里显式给出。
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_PROJECT_WINDOW_DAYS,
  projectProgressRatio,
  projectWindow,
  taskProgressRatio,
} from "./projectGanttProgress.ts";

const DAY_MS = 86_400_000;

function task(extra = {}) {
  return {
    id: "t1",
    title: "任务",
    state: "进行中",
    created_at: "2026-11-01T09:00:00",
    due_date: "2026-11-11",
    ...extra,
  };
}

const NOW_50_PCT = new Date("2026-11-06T00:00:00").getTime();

test("已完成固定为 1，即使尚未到截止日", () => {
  assert.equal(taskProgressRatio(task({ state: "已完成", due_date: "2026-12-31" }), NOW_50_PCT), 1);
});

test("进行中：按创建到截止的时间比例，中点等于 50%", () => {
  assert.equal(taskProgressRatio(task(), NOW_50_PCT), 0.5);
});

test("待开始：即使时间已进入计划区间也保持 0", () => {
  assert.equal(
    taskProgressRatio(task({ state: "待开始" }), NOW_50_PCT),
    0,
  );
});

test("待确认同样保持 0", () => {
  assert.equal(taskProgressRatio(task({ state: "待确认" }), NOW_50_PCT), 0);
});

test("无截止日期返回 0", () => {
  assert.equal(taskProgressRatio(task({ due_date: null }), NOW_50_PCT), 0);
});

test("截止日期非法返回 0", () => {
  assert.equal(taskProgressRatio(task({ due_date: "not-a-date" }), NOW_50_PCT), 0);
});

test("截止不晚于创建日返回 0", () => {
  assert.equal(
    taskProgressRatio(task({ created_at: "2026-11-05T09:00:00", due_date: "2026-11-05" }), NOW_50_PCT),
    0,
  );
});

test("超出截止后夹取到 1", () => {
  assert.equal(taskProgressRatio(task(), new Date("2026-12-01T00:00:00").getTime()), 1);
});

test("早于创建日夹取到 0", () => {
  assert.equal(taskProgressRatio(task(), new Date("2026-10-01T00:00:00").getTime()), 0);
});

test("暂停保留当前时间进度", () => {
  assert.equal(taskProgressRatio(task({ state: "暂停" }), NOW_50_PCT), 0.5);
});

test("已取消保留当前时间进度", () => {
  assert.equal(taskProgressRatio(task({ state: "已取消" }), NOW_50_PCT), 0.5);
});

test("项目有起止日期时直接使用", () => {
  const window = projectWindow({ start: "2026-11-01", end: "2026-12-01" }, NOW_50_PCT);
  assert.equal(window.startMs, new Date("2026-11-01T00:00:00").getTime());
  assert.equal(window.endMs, new Date("2026-12-01T00:00:00").getTime());
});

test("项目缺结束日时默认从开始日起 90 天", () => {
  const startMs = new Date("2026-11-01T00:00:00").getTime();
  const window = projectWindow({ start: "2026-11-01", end: null }, NOW_50_PCT);
  assert.equal(window.startMs, startMs);
  assert.equal(window.endMs, startMs + DEFAULT_PROJECT_WINDOW_DAYS * DAY_MS);
});

test("项目缺开始日时回落到组内最早任务起点并补 90 天", () => {
  const fallback = new Date("2026-11-03T00:00:00").getTime();
  const window = projectWindow({ start: null, end: null }, fallback);
  assert.equal(window.startMs, fallback);
  assert.equal(window.endMs, fallback + 90 * DAY_MS);
});

test("项目结束日早于开始日时退回 90 天区间", () => {
  const startMs = new Date("2026-11-10T00:00:00").getTime();
  const window = projectWindow({ start: "2026-11-10", end: "2026-11-01" }, NOW_50_PCT);
  assert.equal(window.startMs, startMs);
  assert.equal(window.endMs, startMs + 90 * DAY_MS);
});

test("项目进度按项目起止日期计算", () => {
  const ratio = projectProgressRatio(
    { start: "2026-11-01", end: "2026-11-11" },
    [],
    NOW_50_PCT,
  );
  assert.equal(ratio, 0.5);
});

test("项目缺日期时按组内汇总带起点计算默认区间进度", () => {
  const ratio = projectProgressRatio(
    { start: null, end: null },
    [
      {
        startMs: new Date("2026-11-01T00:00:00").getTime(),
        endMs: new Date("2026-11-05T00:00:00").getTime(),
      },
    ],
    new Date("2026-11-16T00:00:00").getTime(),
  );
  assert.equal(ratio, 15 / 90);
});

test("项目进度夹取在 0 与 1 之间", () => {
  assert.equal(
    projectProgressRatio({ start: "2026-11-01", end: "2026-11-11" }, [], NOW_50_PCT - 86_400_000 * 30),
    0,
  );
  assert.equal(
    projectProgressRatio({ start: "2026-11-01", end: "2026-11-11" }, [], new Date("2027-01-01T00:00:00").getTime()),
    1,
  );
});
