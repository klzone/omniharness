/**
 * OD-ARCH-098 S02：项目甘特纯函数的单测。
 *
 * node --experimental-strip-types --test desktop/src/projectGantt.test.mjs
 * 全部夹具是手写字面量；不读磁盘、不读时钟、不调模型。
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  DAY_TICK_MAX_DAYS,
  MIN_WINDOW_DAYS,
  filterItems,
  fromDayNumber,
  ganttTicks,
  ganttWindow,
  isMilestone,
  layoutItem,
  layoutItems,
  pctOf,
  toDayNumber,
  todayPct,
} from "./projectGantt.ts";

function item(extra = {}) {
  return {
    item: "合成事项",
    owner: "P1",
    due: "2026-10-20",
    start: null,
    status: "in_progress",
    next_step: "补齐说明",
    overdue: false,
    ...extra,
  };
}

test("日期解析拒绝不存在的日历日", () => {
  assert.equal(toDayNumber("2026-10-20"), toDayNumber("2026-10-20"));
  assert.equal(fromDayNumber(toDayNumber("2026-10-20")), "2026-10-20");
  for (const bad of ["2026-02-30", "2026-13-01", "2026-00-10", "2026-10-32", "2026/10/20", "26-10-20", "", null, undefined]) {
    assert.equal(toDayNumber(bad), null, String(bad));
  }
});

test("空列表仍然给出可渲染的窗口", () => {
  const win = ganttWindow([], "2026-10-20");
  assert.ok(win.dayCount >= MIN_WINDOW_DAYS);
  assert.ok(win.startDay <= toDayNumber("2026-10-20"));
  assert.ok(win.endDay >= toDayNumber("2026-10-20"));
  assert.equal(win.granularity, "day");
});

test("窗口覆盖最早与最晚条目，并留出两侧留白", () => {
  const win = ganttWindow(
    [
      item({ start: "2026-10-01", due: "2026-10-05" }),
      item({ due: "2026-11-20" }),
    ],
    "2026-10-10",
  );
  assert.ok(win.startDay < toDayNumber("2026-10-01"));
  assert.ok(win.endDay > toDayNumber("2026-11-20"));
  // 跨度 60 天 → 切周刻度。
  assert.ok(win.dayCount > DAY_TICK_MAX_DAYS);
  assert.equal(win.granularity, "week");
});

test("有开始日期画区间条，没有开始日期画里程碑", () => {
  const win = ganttWindow([], "2026-10-20");
  const range = layoutItem(item({ start: "2026-10-15", due: "2026-10-19" }), win);
  assert.equal(range.kind, "range");
  assert.equal(range.spanDays, 4);
  assert.ok(range.widthPct > 0);
  assert.ok(range.leftPct <= range.duePct);

  const milestone = layoutItem(item({ start: null, due: "2026-10-19" }), win);
  assert.equal(milestone.kind, "milestone");
  assert.equal(milestone.widthPct, 0);
  assert.equal(milestone.leftPct, milestone.duePct);

  // 单日区间（开始 = 期限）仍是有宽度的区间条，不会退化成零宽。
  const sameDay = layoutItem(item({ start: "2026-10-19", due: "2026-10-19" }), win);
  assert.equal(sameDay.kind, "range");
  assert.equal(sameDay.spanDays, 0);
  assert.ok(sameDay.widthPct > 0);
});

test("开始日期晚于期限按里程碑处理，不画反向条", () => {
  const win = ganttWindow([], "2026-10-20");
  const reversed = item({ start: "2026-10-25", due: "2026-10-19" });
  assert.equal(isMilestone(reversed), true);
  const bar = layoutItem(reversed, win);
  assert.equal(bar.kind, "milestone");
  assert.ok(bar.widthPct >= 0);
});

test("条形不溢出窗口左右边界", () => {
  const win = ganttWindow([item({ start: "2026-09-01", due: "2026-12-31" })], "2026-10-20");
  const bar = layoutItem(item({ start: "2026-09-01", due: "2026-12-31" }), win);
  assert.ok(bar.leftPct >= 0 && bar.leftPct <= 100);
  assert.ok(bar.widthPct >= 0);
  assert.ok(bar.leftPct + bar.widthPct <= 100.0001);
});

test("今天竖线只在窗口内出现", () => {
  const win = ganttWindow([item({ due: "2026-10-20" })], "2026-10-18");
  assert.notEqual(todayPct("2026-10-18", win), null);
  assert.equal(todayPct("2030-01-01", win), null);
  assert.equal(todayPct("not-a-date", win), null);
});

test("百分比位置是确定性的且夹在 0–100", () => {
  const win = ganttWindow([item({ due: "2026-10-20" })], "2026-10-18");
  assert.equal(pctOf(win.startDay, win), 0);
  assert.equal(pctOf(win.endDay, win), 100);
  assert.equal(pctOf(win.startDay - 10, win), 0);
  assert.equal(pctOf(win.endDay + 10, win), 100);
});

test("刻度落在窗口内且按粒度切换", () => {
  const shortWin = ganttWindow([item({ due: "2026-10-20" })], "2026-10-18");
  const dayTicks = ganttTicks(shortWin);
  assert.ok(dayTicks.length >= 2);
  for (const t of dayTicks) assert.ok(t.leftPct >= 0 && t.leftPct <= 100.0001);

  const longWin = ganttWindow([item({ start: "2026-08-01", due: "2026-11-30" })], "2026-10-20");
  const weekTicks = ganttTicks(longWin);
  assert.ok(weekTicks.length >= 2);
  // 周刻度对齐周一（1 = 周一）。
  for (const t of weekTicks) {
    assert.equal(new Date(`${t.date}T00:00:00Z`).getUTCDay(), 1, t.date);
  }
  assert.match(weekTicks[0].date, /^\d{4}-\d{2}-\d{2}$/);
});

test("批量排布顺序与输入一致", () => {
  const items = [item({ item: "甲" }), item({ item: "乙" }), item({ item: "丙" })];
  const win = ganttWindow(items, "2026-10-20");
  const bars = layoutItems(items, win);
  assert.deepEqual(bars.map((b) => b.item.item), ["甲", "乙", "丙"]);
});

test("搜索按事项、负责人与下一步过滤", () => {
  const items = [
    item({ item: "合成甲", owner: "P1", next_step: "等评审" }),
    item({ item: "合成乙", owner: "P2", next_step: "补材料" }),
  ];
  assert.equal(filterItems(items, "").length, 2);
  assert.deepEqual(filterItems(items, "甲").map((i) => i.item), ["合成甲"]);
  assert.deepEqual(filterItems(items, "p2").map((i) => i.item), ["合成乙"]);
  assert.deepEqual(filterItems(items, "评审").map((i) => i.item), ["合成甲"]);
  assert.equal(filterItems(items, "没有这个词").length, 0);
});
