/** OD-ARCH-154：工具时间线结果事件归并语义的纯函数单测。 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { appendToolActivity } from "./timelineModel.ts";

test("结果事件按 callId 挂回原条目，不改真实工具名", () => {
  let timeline = appendToolActivity([], {
    seq: 1,
    at: 100,
    eventKind: "tool/call",
    toolName: "read_file",
    callId: "call-1",
  });
  timeline = appendToolActivity(timeline, {
    seq: 2,
    at: 101,
    eventKind: "tool/result",
    toolName: "",
    title: "",
    status: "completed",
    callId: "call-1",
  });

  assert.equal(timeline.length, 1);
  assert.equal(timeline[0].title, "read_file");
  assert.equal(timeline[0].status, "完成");
  assert.equal(timeline[0].callId, "call-1");
  assert.deepEqual(
    timeline[0].statusHistory?.map((item) => item.status),
    ["调用", "完成"],
  );
});

test("结果事件无 callId 并入最近命令，不产生未知工具行", () => {
  let timeline = appendToolActivity([], {
    seq: 1,
    at: 100,
    eventKind: "tool/call",
    toolName: "exec_command",
    callId: "call-1",
  });
  timeline = appendToolActivity(timeline, {
    seq: 2,
    at: 102,
    eventKind: "tool/result",
    toolName: "未知工具",
    title: "未知工具",
    callId: "",
  });

  assert.equal(timeline.length, 1);
  assert.equal(timeline[0].title, "exec_command");
  assert.equal(timeline[0].status, "结果");
});

test("结果事件找不到宿主条目时忽略，不新建未知工具行", () => {
  const timeline = appendToolActivity([], {
    seq: 7,
    at: 200,
    eventKind: "tool/result",
    toolName: "未知工具",
    title: "未知工具",
    callId: "orphan-call",
  });

  assert.equal(timeline.length, 0);
});

test("tool/call 缺名仍显示未知工具，后续结果不会覆盖它", () => {
  let timeline = appendToolActivity([], {
    seq: 1,
    at: 300,
    eventKind: "tool/call",
    toolName: "",
    callId: "call-unknown",
  });
  timeline = appendToolActivity(timeline, {
    seq: 2,
    at: 301,
    eventKind: "tool/result",
    toolName: "",
    title: "",
    callId: "call-unknown",
  });

  assert.equal(timeline.length, 1);
  assert.equal(timeline[0].title, "未知工具");
  assert.equal(timeline[0].status, "结果");
});
