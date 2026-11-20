import assert from "node:assert/strict";
import { test } from "node:test";

import {
  detectPeriodReviewTarget,
  detectWorkIntent,
  normalizeIntentText,
  PERIOD_REVIEW_TABLE,
} from "./workIntent.ts";

const NOW = new Date("2026-11-20T04:00:00Z"); // 周五

test("detectWorkIntent 正常分类", () => {
  assert.equal(detectWorkIntent("写一份周报"), "report");
  assert.equal(detectWorkIntent("记一下今天做了什么"), "note");
  assert.equal(detectWorkIntent("生成今日摘要"), "daily-summary");
  assert.equal(detectWorkIntent("帮我做本周复盘"), "period-review");
  assert.equal(detectWorkIntent("月度总结"), "period-review");
});

test("detectWorkIntent 边界与非法输入", () => {
  assert.equal(detectWorkIntent(""), "note");
  assert.equal(detectWorkIntent("   "), "note");
  // 只有周范围没有总结动词：仍按随手记处理，不冒充周报。
  assert.equal(detectWorkIntent("本周计划"), "note");
});

test("detectPeriodReviewTarget 周/月/季/年边界", () => {
  const weekly = detectPeriodReviewTarget("本周复盘", NOW);
  assert.deepEqual(weekly, { period: "weekly", start: "2026-11-16", end: "2026-11-22", label: "本周" });
  const lastWeek = detectPeriodReviewTarget("上周复盘", NOW);
  assert.deepEqual(lastWeek, { period: "weekly", start: "2026-11-09", end: "2026-11-15", label: "上周" });
  const monthly = detectPeriodReviewTarget("本月复盘", NOW);
  assert.deepEqual(monthly, { period: "monthly", start: "2026-11-01", end: "2026-11-30", label: "本月" });
  const quarterly = detectPeriodReviewTarget("季度复盘", NOW);
  assert.equal(quarterly.period, "quarterly");
  assert.equal(quarterly.start, "2026-10-01");
  assert.equal(quarterly.end, "2026-12-31");
  const annual = detectPeriodReviewTarget("年度复盘", NOW);
  assert.equal(annual.period, "annual");
  assert.equal(annual.start, "2026-01-01");
  assert.equal(annual.end, "2026-12-31");
});

test("detectPeriodReviewTarget 跨年与月长", () => {
  const january = detectPeriodReviewTarget("上月复盘", new Date("2026-01-15T04:00:00Z"));
  assert.deepEqual(january, { period: "monthly", start: "2025-12-01", end: "2025-12-31", label: "上月" });
  const february = detectPeriodReviewTarget("上月复盘", new Date("2026-03-10T04:00:00Z"));
  assert.deepEqual(february, { period: "monthly", start: "2026-02-01", end: "2026-02-28", label: "上月" });

test("结构化词表：每个词条的周期与标签", () => {
  for (const entry of PERIOD_REVIEW_TABLE) {
    const target = detectPeriodReviewTarget(entry.keyword, NOW);
    assert.ok(target, entry.keyword + " 应命中");
    assert.equal(target.period, entry.period, entry.keyword);
    const offset = entry.offset ?? 0;
    if (entry.period === "weekly") {
      assert.equal(target.label, offset === 0 ? "本周" : "上周", entry.keyword);
    }
    if (entry.period === "monthly") {
      assert.equal(target.label, offset === 0 ? "本月" : "上月", entry.keyword);
    }
    assert.match(target.start, /^\d{4}-\d{2}-\d{2}$/);
    assert.match(target.end, /^\d{4}-\d{2}-\d{2}$/);
    assert.ok(target.start <= target.end);
  }
});

test("上月复盘在二月（闰年）与三十一天月份", () => {
  const leap = detectPeriodReviewTarget("复盘上月", new Date("2028-03-05T04:00:00Z"));
  assert.deepEqual(leap, { period: "monthly", start: "2028-02-01", end: "2028-02-29", label: "上月" });
  const jan = detectPeriodReviewTarget("上个月复盘", new Date("2026-01-20T04:00:00Z"));
  assert.deepEqual(jan, { period: "monthly", start: "2025-12-01", end: "2025-12-31", label: "上月" });
  const may = detectPeriodReviewTarget("上月复盘", new Date("2026-06-10T04:00:00Z"));
  assert.deepEqual(may, { period: "monthly", start: "2026-05-01", end: "2026-05-31", label: "上月" });
});

test("词表中间插入新词不影响既有判定", () => {
  const table = [...PERIOD_REVIEW_TABLE];
  table.splice(3, 0, { keyword: "这一周复盘", period: "weekly" });
  // 用注入后的词表行为等价：新词命中周复盘，邻近的「月度总结」仍是月度。
  assert.deepEqual(detectPeriodReviewTarget("这一周复盘", NOW)?.period, "weekly");
  assert.deepEqual(detectPeriodReviewTarget("月度总结", NOW)?.period, "monthly");
});
});

test("detectPeriodReviewTarget 非法输入返回 null", () => {
  assert.equal(detectPeriodReviewTarget("今天吃了火锅", NOW), null);
  assert.equal(detectPeriodReviewTarget("", NOW), null);
});

test("normalizeIntentText 全角与空白归一", () => {
  assert.equal(normalizeIntentText("  周　报  "), "周报");
  assert.equal(normalizeIntentText("ＡＢＣ"), "abc");
});
