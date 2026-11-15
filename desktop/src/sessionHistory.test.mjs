/** OD-UI-714：历史会话分组与卡片文案的纯函数单测。 */

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  groupSessionThreads,
  sessionCategoryLabel,
  threadPreview,
  threadUpdatedLabel,
} from "./sessionHistory.ts";

const category = (key, label) => ({ key, label, hint: label + " 的说明" });
const thread = (id, tag, updatedAt, messages = []) => ({
  id,
  tag,
  projectPath: null,
  title: id,
  preset: "plain",
  messages,
  createdAt: updatedAt,
  updatedAt,
});

test("分类按宿主顺序分组，空分类不渲染", () => {
  const categories = [category("plain", "未分类"), category("weekly", "工作记录"), category("custom:学习", "学习")];
  const groups = groupSessionThreads(
    [
      thread("t2", "weekly", 2),
      thread("t1", "plain", 1),
      thread("t3", "plain", 3),
    ],
    categories,
  );
  assert.deepEqual(groups.map((group) => group.key), ["plain", "weekly"]);
  assert.deepEqual(groups[0].threads.map((item) => item.id), ["t3", "t1"]);
  assert.equal(groups[0].label, "未分类");
});

test("新增分类直接参与分组，未知旧标签不丢弃", () => {
  const categories = [category("plain", "未分类"), category("custom:读书", "读书笔记")];
  const groups = groupSessionThreads(
    [
      thread("t1", "custom:读书", 3),
      thread("t2", "legacy:old", 2),
      thread("t3", "writing", 1),
    ],
    categories,
  );
  assert.deepEqual(groups.map((group) => group.key), ["custom:读书", "writing", "legacy:old"]);
  assert.equal(groups[0].label, "读书笔记");
  assert.equal(groups[1].label, "文案写作");
  assert.equal(groups[2].label, "legacy:old");
});

test("分类名回退：内置名、自定义键与未知键都可读", () => {
  const categories = [category("custom:学习", "学习笔记")];
  assert.equal(sessionCategoryLabel("custom:学习", categories), "学习笔记");
  assert.equal(sessionCategoryLabel("custom:随手", categories), "随手");
  assert.equal(sessionCategoryLabel("weekly", categories), "工作记录");
  assert.equal(sessionCategoryLabel("legacy:old", categories), "legacy:old");
});

test("卡片摘要取最后一条非空消息并截断", () => {
  assert.equal(threadPreview([]), "还没有消息");
  assert.equal(threadPreview([{ text: "  " }, { text: "最后一句" }]), "最后一句");
  const long = "甲".repeat(100);
  assert.equal(threadPreview([{ text: long }]), "甲".repeat(88) + "…");
});

test("更新时间按今天、昨天、本年和跨年显示", () => {
  const now = new Date(2026, 10, 15, 14, 32).getTime();
  assert.equal(threadUpdatedLabel(now, now), "今天 14:32");
  assert.equal(threadUpdatedLabel(new Date(2026, 10, 14, 9, 5).getTime(), now), "昨天 09:05");
  assert.equal(threadUpdatedLabel(new Date(2026, 9, 20, 18, 40).getTime(), now), "10月20日 18:40");
  assert.equal(threadUpdatedLabel(new Date(2025, 11, 31, 23, 0).getTime(), now), "2025年12月31日");
  assert.equal(threadUpdatedLabel(0, now), "时间未知");
});
