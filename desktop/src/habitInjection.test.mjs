import assert from "node:assert/strict";
import { test } from "node:test";

import {
  HABIT_INJECT_INTRO,
  HABIT_LAST_INJECTED_KEY,
  HABIT_LAST_INJECTED_MAX_ROWS,
  HABIT_MAX_CARDS,
  HABIT_MAX_CHARS,
  injectHabits,
  injectionLogLine,
  recordHabitInjection,
  rollInjectionLog,
  selectHabitCards,
} from "./habitInjection.ts";

function card(id, overrides = {}) {
  return {
    schema: "omnidesk.habit-card.v1",
    id,
    kind: "preference",
    scope: { type: "scene", scene: "report" },
    text: "习惯 " + id,
    evidence: { thread_id: "t1", message_id: "m1", date: "2026-11-20" },
    origin: "explicit",
    status: "active",
    created_at: "2026-11-20T02:00:00Z",
    confirmed_at: "2026-11-20T02:00:00Z",
    last_seen_at: "2026-11-20T02:00:00Z",
    ...overrides,
  };
}

const PROMPT = "用户：帮我把这周的记录整理一下。";

test("只注入生效卡：候选、停用、归档卡不出现", () => {
  const cards = [
    card("global", { scope: { type: "global" }, confirmed_at: "2026-11-22T02:00:00Z" }),
    card("report", { confirmed_at: "2026-11-21T02:00:00Z" }),
    card("writing", { scope: { type: "scene", scene: "writing" } }),
    card("candidate", { status: "candidate" }),
    card("archived", { status: "archived" }),
    card("disabled", { status: "disabled" }),
  ];
  const result = injectHabits(PROMPT, cards, "report");
  assert.deepEqual(result.injected, ["global", "report"]);
  for (const blocked of ["writing", "candidate", "archived", "disabled"]) {
    assert.ok(!result.prompt.includes("习惯 " + blocked), blocked + " 不应出现在提示词");
  }
  assert.equal(result.skipped.length, 0);
});

test("场景匹配：global 全场景生效，scene 卡只在声明场景生效", () => {
  const cards = [
    card("global", { scope: { type: "global" } }),
    card("report", { scope: { type: "scene", scene: "report" } }),
    card("project", { scope: { type: "scene", scene: "project" } }),
  ];
  assert.deepEqual(selectHabitCards(cards, "report").map((c) => c.id), ["global", "report"]);
  assert.deepEqual(selectHabitCards(cards, "project").map((c) => c.id), ["global", "project"]);
  assert.deepEqual(selectHabitCards(cards, "").map((c) => c.id), ["global"]);
});

test("排序：最近确认优先，同时间按编号稳定排序", () => {
  const cards = [
    card("old", { confirmed_at: "2026-11-01T02:00:00Z" }),
    card("new", { confirmed_at: "2026-11-05T02:00:00Z" }),
    card("mid", { confirmed_at: "2026-11-03T02:00:00Z" }),
    card("tie-b", { confirmed_at: "2026-11-03T02:00:00Z" }),
  ];
  assert.deepEqual(selectHabitCards(cards, "report").map((c) => c.id), [
    "new",
    "mid",
    "tie-b",
    "old",
  ]);
});

test("数量上限：最多 8 张，超出的卡记为 skipped", () => {
  const cards = Array.from({ length: 10 }, (_, index) =>
    card("c" + index, {
      text: "习惯" + index,
      confirmed_at: "2026-11-" + String(28 - index).padStart(2, "0") + "T02:00:00Z",
    }),
  );
  const result = injectHabits(PROMPT, cards, "report");
  assert.equal(result.injected.length, HABIT_MAX_CARDS);
  assert.equal(result.skipped.length, 2);
  const bulletLines = result.prompt.split("\n").filter((line) => line.startsWith("- "));
  assert.equal(bulletLines.length, HABIT_MAX_CARDS);
});

test("字数上限：卡文字合计不超过 600，超出整张跳过并继续看后续短卡", () => {
  const cards = Array.from({ length: 7 }, (_, index) =>
    card("c" + index, {
      text: "习".repeat(100),
      confirmed_at: "2026-11-" + String(28 - index).padStart(2, "0") + "T02:00:00Z",
    }),
  );
  const result = injectHabits(PROMPT, cards, "report");
  assert.equal(result.injected.length, 6);
  assert.ok(result.skipped.includes("c6"));
  const used = result.injected.length * 100;
  assert.ok(used <= HABIT_MAX_CHARS);
  assert.ok(used + 100 > HABIT_MAX_CHARS);
});

test("空列表（开关关闭时 active 列表为空）：原样返回，不注入", () => {
  const result = injectHabits(PROMPT, [], "report");
  assert.equal(result.prompt, PROMPT);
  assert.deepEqual(result.injected, []);
  assert.deepEqual(result.skipped, []);

  const onlyCandidates = [card("c1", { status: "candidate" }), card("c2", { status: "archived" })];
  const blocked = injectHabits(PROMPT, onlyCandidates, "report");
  assert.equal(blocked.prompt, PROMPT);
  assert.deepEqual(blocked.injected, []);
});

test("注入段落：固定文案打头，每卡一行「- {text}」，原提示词保持完整", () => {
  const cards = [card("a", { text: "周报先写事实，再写判断。" })];
  const result = injectHabits(PROMPT, cards, "report");
  const expected = HABIT_INJECT_INTRO + "\n- 周报先写事实，再写判断。\n\n" + PROMPT;
  assert.equal(result.prompt, expected);
  assert.ok(result.prompt.startsWith(HABIT_INJECT_INTRO));
  assert.ok(result.prompt.endsWith(PROMPT));
});

test("统计记录：日期加卡编号，localStorage 滚动保留最近 50 行", () => {
  assert.equal(injectionLogLine(["a", "b"], "2026-11-23"), "2026-11-23\ta,b");
  const previous = Array.from({ length: 60 }, (_, index) => "row-" + index).join("\n");
  const rows = rollInjectionLog(previous, "2026-11-23\ta,b");
  assert.equal(rows.length, HABIT_LAST_INJECTED_MAX_ROWS);
  assert.equal(rows.at(-1), "2026-11-23\ta,b");
  assert.equal(rows[0], "row-11");
});

test("recordHabitInjection：写 localStorage 键；空注入不写；写入失败不外抛", () => {
  const store = new Map();
  const storage = {
    getItem: (key) => (store.has(key) ? store.get(key) : null),
    setItem: (key, value) => store.set(key, value),
  };
  recordHabitInjection(["a", "b"], { storage, now: new Date(2026, 10, 23) });
  assert.equal(store.get(HABIT_LAST_INJECTED_KEY), "2026-11-23\ta,b");

  const saved = store.get(HABIT_LAST_INJECTED_KEY);
  recordHabitInjection([], { storage, now: new Date(2026, 10, 24) });
  assert.equal(store.get(HABIT_LAST_INJECTED_KEY), saved);

  const broken = {
    getItem: () => null,
    setItem: () => {
      throw new Error("quota");
    },
  };
  assert.doesNotThrow(() => recordHabitInjection(["c"], { storage: broken }));
});
