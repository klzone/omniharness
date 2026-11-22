/**
 * OD-PRJ-602 S1：项目甘特分组纯函数的单测。
 *
 * node --experimental-strip-types --test desktop/src/projectGanttGroups.test.mjs
 * 全部夹具是手写字面量；不读磁盘、不读时钟、不调模型。
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AUTO_COLLAPSE_ROW_LIMIT,
  UNATTACHED_PROJECT_ID,
  UNATTACHED_PROJECT_NAME,
  autoCollapseGroups,
  buildProjectGanttGroups,
  layoutTaskBar,
  summaryRangeLabel,
} from "./projectGanttGroups.ts";

const TIMELINE = {
  start: new Date("2026-11-01T00:00:00").getTime(),
  end: new Date("2026-12-01T00:00:00").getTime(),
  span: new Date("2026-12-01T00:00:00").getTime() - new Date("2026-11-01T00:00:00").getTime(),
};

function project(project_id, name) {
  return { project_id, name };
}

function task(id, extra = {}) {
  return {
    id,
    title: "任务 " + id,
    state: "待开始",
    created_at: "2026-11-01T09:00:00",
    due_date: "2026-11-05",
    project_id: null,
    ...extra,
  };
}

test("重叠：同组两个交叠任务各占一行，不合并", () => {
  const groups = buildProjectGanttGroups(
    [
      task("a", { title: "甲任务", created_at: "2026-11-01T09:00:00", due_date: "2026-11-10", project_id: "p1" }),
      task("b", { title: "乙任务", created_at: "2026-11-05T09:00:00", due_date: "2026-11-15", project_id: "p1" }),
    ],
    [project("p1", "项目甲")],
    TIMELINE,
  );
  assert.equal(groups.length, 1);
  assert.equal(groups[0].projectName, "项目甲");
  assert.equal(groups[0].rows.length, 2);
  const [first, second] = groups[0].rows;
  assert.ok(first.left < second.left);
  assert.ok(second.left < first.left + first.width, "第二条左端落在第一条内部");
  assert.ok(groups[0].summary.widthPct > first.width, "汇总带覆盖最晚条右端");
});

test("接续：前条结束日等于后条开始日，后条左端贴合前条右端", () => {
  const groups = buildProjectGanttGroups(
    [
      task("a", { created_at: "2026-11-01T09:00:00", due_date: "2026-11-10", project_id: "p1" }),
      task("b", { created_at: "2026-11-10T09:00:00", due_date: "2026-11-20", project_id: "p1" }),
    ],
    [project("p1", "项目甲")],
    TIMELINE,
  );
  const [first, second] = groups[0].rows;
  assert.ok(Math.abs(second.left - (first.left + first.width)) < 1e-9);
});

test("间隔：前条结束后到后条开始留白，不画假条", () => {
  const groups = buildProjectGanttGroups(
    [
      task("a", { created_at: "2026-11-01T09:00:00", due_date: "2026-11-05", project_id: "p1" }),
      task("b", { created_at: "2026-11-10T09:00:00", due_date: "2026-11-20", project_id: "p1" }),
    ],
    [project("p1", "项目甲")],
    TIMELINE,
  );
  const [first, second] = groups[0].rows;
  assert.equal(groups[0].rows.length, 2);
  assert.ok(second.left > first.left + first.width + 1, "两段之间保留时间间隔");
});

test("未挂接：空 project_id 与已删除项目都归入「未挂接项目」组", () => {
  const groups = buildProjectGanttGroups(
    [
      task("a", { project_id: null, created_at: "2026-11-02T09:00:00" }),
      task("b", { project_id: "missing", created_at: "2026-11-03T09:00:00" }),
      task("c", { project_id: "p1", created_at: "2026-11-04T09:00:00" }),
    ],
    [project("p1", "项目甲")],
    TIMELINE,
  );
  assert.equal(groups.length, 2);
  const unattached = groups.find((group) => group.projectId === UNATTACHED_PROJECT_ID);
  assert.ok(unattached);
  assert.equal(unattached.projectName, UNATTACHED_PROJECT_NAME);
  assert.equal(unattached.rows.length, 2);
});

test("空项目不出现，组按汇总带左端排序", () => {
  const groups = buildProjectGanttGroups(
    [
      task("late", { project_id: "pLate", created_at: "2026-11-12T09:00:00", due_date: "2026-11-18" }),
      task("early", { project_id: "pEarly", created_at: "2026-11-02T09:00:00", due_date: "2026-11-08" }),
    ],
    [project("pEarly", "项目早"), project("pLate", "项目晚"), project("pEmpty", "空项目")],
    TIMELINE,
  );
  assert.equal(groups.length, 2);
  assert.ok(!groups.some((group) => group.projectId === "pEmpty"));
  assert.equal(groups[0].projectId, "pEarly");
  assert.equal(groups[1].projectId, "pLate");
});

test("clamp：任务跨出窗口两端时夹到 0% 与右端，并标记 clamped", () => {
  const row = layoutTaskBar(
    task("wide", { created_at: "2026-10-20T09:00:00", due_date: "2026-12-10" }),
    TIMELINE,
  );
  assert.ok(row);
  assert.equal(row.left, 0);
  assert.equal(row.clamped, true);
  assert.ok(row.left + row.width <= 100 + 1e-9);
  const groups = buildProjectGanttGroups(
    [task("wide", { created_at: "2026-10-20T09:00:00", due_date: "2026-12-10" })],
    [],
    TIMELINE,
  );
  assert.equal(groups[0].summary.start, "2026-11-01");
  // 右端是时间窗的排他边界（12 月 1 日 00:00），与刻度终点同一口径。
  assert.equal(groups[0].summary.end, "2026-12-01");
});

test("完全落在窗口外的任务不进入分组", () => {
  const groups = buildProjectGanttGroups(
    [
      task("before", { created_at: "2026-10-01T09:00:00", due_date: "2026-10-20" }),
      task("after", { created_at: "2026-12-05T09:00:00", due_date: "2026-12-20" }),
    ],
    [],
    TIMELINE,
  );
  assert.equal(groups.length, 0);
});

test("组头区间文字同年显示 MM.DD，跨年保留年份", () => {
  assert.equal(
    summaryRangeLabel({ start: "2026-11-01", end: "2026-11-28", leftPct: 0, widthPct: 100 }),
    "11.01 → 11.28",
  );
  assert.equal(
    summaryRangeLabel({ start: "2026-12-20", end: "2027-01-05", leftPct: 0, widthPct: 100 }),
    "2026.12.20 → 2027.01.05",
  );
});

test("超过 30 行才自动折叠未挂接组与最早结束的组", () => {
  const rows = (count) =>
    Array.from({ length: count }, (_, index) => ({
      task: task("r" + index),
      left: index,
      width: 2,
      clamped: false,
      startMs: TIMELINE.start,
      endMs: TIMELINE.start,
    }));
  const groups = [
    {
      projectId: UNATTACHED_PROJECT_ID,
      projectName: UNATTACHED_PROJECT_NAME,
      summary: { start: "2026-11-01", end: "2026-11-28", leftPct: 0, widthPct: 50 },
      rows: rows(16),
    },
    {
      projectId: "p1",
      projectName: "项目甲",
      summary: { start: "2026-11-02", end: "2026-11-10", leftPct: 2, widthPct: 30 },
      rows: rows(15),
    },
  ];
  assert.deepEqual(autoCollapseGroups(groups), [UNATTACHED_PROJECT_ID, "p1"]);

  const small = [{ ...groups[0], rows: rows(AUTO_COLLAPSE_ROW_LIMIT) }];
  assert.deepEqual(autoCollapseGroups(small), []);
});
