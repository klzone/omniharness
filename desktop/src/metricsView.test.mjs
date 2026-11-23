import assert from "node:assert/strict";
import { test } from "node:test";

import {
  buildMetrics,
  formatMetric,
  formatRejectionTop,
  formatViolationTrend,
  rejectionTop,
} from "./metricsView.ts";

test("接受率：按接受与驳回合计计算，分母为 0 或数字缺失返回 null", () => {
  assert.equal(buildMetrics({ candidates: { accepted: 24, rejected: 16 } }).acceptRate, 60);
  assert.equal(buildMetrics({ candidates: { accepted: 14, rejected: 6 } }).acceptRate, 70);
  assert.equal(buildMetrics({ candidates: { accepted: 1, rejected: 2 } }).acceptRate, 33.3);
  assert.equal(buildMetrics({ candidates: { accepted: 0, rejected: 0 } }).acceptRate, null);
  assert.equal(buildMetrics({ candidates: { accepted: 5 } }).acceptRate, null);
  assert.equal(buildMetrics({}).acceptRate, null);
});

test("驳回原因 Top3：按次数降序，同次数按原因码升序，只留前三", () => {
  const top = rejectionTop({ too_absolute: 3, not_mine: 5, stale: 2, other: 1, zero: 0 });
  assert.deepEqual(top, [
    { code: "not_mine", count: 5 },
    { code: "too_absolute", count: 3 },
    { code: "stale", count: 2 },
  ]);
  assert.deepEqual(rejectionTop({ b: 2, a: 2 }), [
    { code: "a", count: 2 },
    { code: "b", count: 2 },
  ]);
  assert.deepEqual(rejectionTop(undefined), []);
});

test("违反趋势：同日合并计数，窗口只取最近 7 个日期并按日期升序", () => {
  const trend = buildMetrics({
    violations: {
      a: { count: 1, lastDate: "2026-11-01" },
      b: { count: 2, lastDate: "2026-11-03" },
      c: { count: 3, lastDate: "2026-11-03" },
      d: { count: 2, lastDate: "2026-11-04" },
      e: { count: 1, lastDate: "2026-11-05" },
      f: { count: 0, lastDate: "2026-11-06" },
      g: { count: 2, lastDate: "2026-11-07" },
      h: { count: 1, lastDate: "2026-11-08" },
      i: { count: 3, lastDate: "2026-11-09" },
      j: { count: 1, lastDate: "2026-11-10" },
    },
  }).violationTrend;
  assert.deepEqual(trend, [2, 1, 0, 2, 1, 3, 1]);
  assert.deepEqual(buildMetrics({ violations: {} }).violationTrend, []);
});

test("整理成功率：成功 /（成功 + 失败），分母为 0 或缺失返回 null", () => {
  assert.equal(
    buildMetrics({ consolidations: { success: 6, fail: 4, tokens: 1200 } })
      .consolidationSuccessRate,
    60,
  );
  assert.equal(
    buildMetrics({ consolidations: { success: 0, fail: 0 } }).consolidationSuccessRate,
    null,
  );
  assert.equal(
    buildMetrics({ consolidations: { fail: 3 } }).consolidationSuccessRate,
    null,
  );
});

test("缺失数据一律显示 —，有数值时带单位", () => {
  const empty = buildMetrics({});
  assert.equal(formatMetric(empty.acceptRate, "%"), "—");
  assert.equal(formatMetric(empty.consolidationSuccessRate, "%"), "—");
  assert.equal(formatRejectionTop(empty.rejectionTop), "—");
  assert.equal(formatViolationTrend(empty.violationTrend), "—");
  assert.equal(formatMetric(60, "%"), "60%");
  assert.equal(formatMetric(33.3, "%"), "33.3%");
  assert.equal(
    formatRejectionTop([{ code: "not_mine", count: 5 }, { code: "too_absolute", count: 3 }]),
    "不是我说的 × 5、太绝对 × 3",
  );
  assert.equal(formatViolationTrend([1, 2, 0]), "1 → 2 → 0");
});
