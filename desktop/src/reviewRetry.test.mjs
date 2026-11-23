import assert from "node:assert/strict";
import { test } from "node:test";

import {
  isRetryableReviewFailure,
  nextReviewStep,
  reviewRetryPrompt,
} from "./reviewRetry.ts";

const TABLE = [
  { name: "首轮待提交", state: { attempt: 0, phase: "idle" }, step: "submit" },
  { name: "重试轮待提交", state: { attempt: 1, phase: "idle" }, step: "submit" },
  { name: "提交后等待", state: { attempt: 0, phase: "submitting" }, step: "await" },
  { name: "等待模型", state: { attempt: 0, phase: "awaiting" }, step: "await" },
  { name: "首轮成功进入落草稿", state: { attempt: 0, phase: "succeeded" }, step: "ingest" },
  { name: "落草稿过程中保持 ingest", state: { attempt: 0, phase: "ingesting" }, step: "ingest" },
  { name: "重试成功仍进入落草稿", state: { attempt: 1, phase: "succeeded" }, step: "ingest" },
  { name: "首轮模型失败进入重试", state: { attempt: 0, phase: "failed", failReason: "生成未完成：failed" }, step: "retry" },
  { name: "首轮校验失败进入重试", state: { attempt: 0, phase: "failed", failReason: "复盘缺少段落：本周完成，未写入" }, step: "retry" },
  { name: "首轮空结果进入重试", state: { attempt: 0, phase: "failed", failReason: "模型没有返回复盘正文，本次未写入。" }, step: "retry" },
  { name: "重试失败终止", state: { attempt: 1, phase: "failed", failReason: "复盘段落没有正文：本周反思，未写入" }, step: "failed" },
  { name: "重复触发不叠加", state: { attempt: 1, phase: "failed", failReason: "生成未完成：failed" }, step: "failed" },
  { name: "未知错误不重试", state: { attempt: 0, phase: "failed", failReason: "读取复盘失败" }, step: "failed" },
];

test("nextReviewStep 全转移表", () => {
  for (const row of TABLE) {
    assert.equal(nextReviewStep(row.state), row.step, row.name);
  }
});

test("用户取消与准入拒绝不重试", () => {
  const reasons = [
    "生成未完成：cancelled",
    "生成未完成：interrupted · 取消阶段：timeout_unknown",
    "用户取消",
    "endpoint_unconfirmed：当前模型端点未获得你的使用许可",
    "endpoint_unknown：模型端点类别未知",
    "red_line_rejected：输入命中红线",
    "pending_write_blocked：同场景存在未决确认卡",
    "workspace_unavailable：准备引擎工作目录失败",
  ];
  for (const failReason of reasons) {
    assert.equal(isRetryableReviewFailure(failReason), false, failReason);
    assert.equal(
      nextReviewStep({ attempt: 0, phase: "failed", failReason }),
      "failed",
      failReason,
    );
  }
});

test("attempt 边界只在首轮产生一次重试", () => {
  const failReason = "生成未完成：failed";
  assert.equal(nextReviewStep({ attempt: 0, phase: "failed", failReason }), "retry");
  assert.equal(nextReviewStep({ attempt: 1, phase: "failed", failReason }), "failed");
  assert.equal(nextReviewStep({ attempt: 2, phase: "failed", failReason }), "failed");
});

test("重试提示词追加失败原因并去掉多余尾空白", () => {
  const prompt = reviewRetryPrompt("原始提示词\n\n", "复盘缺少段落：本周完成");
  assert.equal(
    prompt,
    "原始提示词\n\n上一次尝试失败，原因：复盘缺少段落：本周完成；请修正后重新输出",
  );
});
