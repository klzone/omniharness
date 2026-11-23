import assert from "node:assert/strict";
import { test } from "node:test";

import {
  consolidationLocalDate,
  emptyConsolidationState,
  nextStep,
} from "./consolidationFlow.ts";

const TODAY = "2026-11-23";

function state(overrides = {}) {
  return { ...emptyConsolidationState(TODAY), ...overrides };
}

const TABLE = [
  { name: "到点且无在途对话：开始准备", overrides: { tickDue: true }, step: "prepare" },
  { name: "未到点：保持空闲", overrides: { tickDue: false }, step: "idle" },
  { name: "到点但有在途对话：让位", overrides: { tickDue: true, runningChat: true }, step: "idle" },
  { name: "未到点且有在途对话：保持空闲", overrides: { tickDue: false, runningChat: true }, step: "idle" },
  { name: "今天已跑：未到点也不重跑", overrides: { attemptDate: TODAY }, step: "idle" },
  { name: "今天已跑：到点仍不重跑", overrides: { attemptDate: TODAY, tickDue: true }, step: "idle" },
  { name: "空摘要：记完成", overrides: { phase: "prepare", digestEmpty: true }, step: "done" },
  { name: "非空摘要：进入提交", overrides: { phase: "prepare", digestEmpty: false }, step: "submit" },
  { name: "提交被拒：不重试", overrides: { phase: "submit", submitRejected: true }, step: "failed" },
  { name: "提交成功：等待终态", overrides: { phase: "submit", submitRejected: false }, step: "await" },
  { name: "等待中未终态：继续等", overrides: { phase: "await" }, step: "await" },
  { name: "等待中成功：进入落结果", overrides: { phase: "await", taskDone: true, taskSucceeded: true }, step: "ingest" },
  { name: "等待中失败：终止", overrides: { phase: "await", taskDone: true, taskSucceeded: false }, step: "failed" },
  { name: "等待超时：终止", overrides: { phase: "await", timedOut: true }, step: "failed" },
  { name: "超时优先于成功：终止", overrides: { phase: "await", timedOut: true, taskDone: true, taskSucceeded: true }, step: "failed" },
  { name: "记录结果后完成", overrides: { phase: "ingest" }, step: "done" },
  { name: "完成相位保持完成", overrides: { phase: "done" }, step: "done" },
  { name: "失败相位保持失败", overrides: { phase: "failed" }, step: "failed" },
  { name: "重启补跑：今天尚无尝试且到点", overrides: { tickDue: true, attemptDate: null }, step: "prepare" },
  { name: "重启补跑：昨天尝试过且今天到点", overrides: { tickDue: true, attemptDate: "2026-11-22" }, step: "prepare" },
  { name: "昨天尝试过但今天未到点：等待", overrides: { tickDue: false, attemptDate: "2026-11-22" }, step: "idle" },
  { name: "重复触发：等待中但今天已跑", overrides: { phase: "await", attemptDate: TODAY }, step: "idle" },
  { name: "重复触发：提交中但今天已跑", overrides: { phase: "submit", attemptDate: TODAY }, step: "idle" },
  { name: "重复触发：准备中但今天已跑", overrides: { phase: "prepare", attemptDate: TODAY }, step: "idle" },
  { name: "重复触发：完成但今天已跑", overrides: { phase: "done", attemptDate: TODAY }, step: "idle" },
  { name: "重复触发：失败但今天已跑", overrides: { phase: "failed", attemptDate: TODAY }, step: "idle" },
  { name: "空摘要配合在途对话仍先让位", overrides: { attemptDate: null, tickDue: true, runningChat: true }, step: "idle" },
  { name: "准备中不重新判到点或对话", overrides: { phase: "prepare", digestEmpty: false, tickDue: false, runningChat: true }, step: "submit" },
];

test("nextStep 全转移表（28 行）", () => {
  for (const row of TABLE) {
    assert.equal(nextStep(state(row.overrides)), row.step, row.name);
  }
});

test("今日已跑对所有相位都回 idle", () => {
  for (const phase of ["idle", "prepare", "submit", "await", "ingest", "done", "failed"]) {
    assert.equal(
      nextStep(state({ phase, attemptDate: TODAY, tickDue: true, taskDone: true })),
      "idle",
      phase,
    );
  }
});

test("纯函数不改入参", () => {
  const input = state({ phase: "await", taskDone: true, taskSucceeded: true });
  const before = JSON.stringify(input);
  assert.equal(nextStep(input), "ingest");
  assert.equal(JSON.stringify(input), before);
});

test("本地日期按整理配置的 UTC 偏移换算", () => {
  const at = Date.UTC(2026, 10, 22, 16, 30) / 1000;
  assert.equal(consolidationLocalDate(at, 480), "2026-11-23");
  assert.equal(consolidationLocalDate(at, 0), "2026-11-22");
  assert.equal(consolidationLocalDate(Number.NaN, 480), "");
});
