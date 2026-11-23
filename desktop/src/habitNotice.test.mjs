import assert from "node:assert/strict";
import { test } from "node:test";

import {
  habitBellNotice,
  localDateFromUtcIso,
  shouldShowHomeNotice,
  sortOpenQuestions,
  todayCandidateCount,
} from "./habitNotice.ts";

const TODAY = "2026-11-23";
const OFFSET = 480;

function candidate(id, overrides = {}) {
  return {
    schema: "omnidesk.habit-card.v1",
    id,
    kind: "preference",
    scope: { type: "scene", scene: "report" },
    text: "候选 " + id,
    evidence: { thread_id: "t1", message_id: "m1", date: TODAY },
    origin: "explicit",
    status: "candidate",
    created_at: "2026-11-23T02:00:00Z",
    last_seen_at: "2026-11-23T02:00:00Z",
    ...overrides,
  };
}

function question(id, lastSeen, status = "open") {
  return {
    schema: "omnidesk.open-question.v1",
    id,
    text: "问题 " + id,
    thread_id: "t1",
    message_id: "m1",
    first_seen: "2026-11-01T02:00:00Z",
    last_seen: lastSeen,
    status,
  };
}

test("没有候选也没有未决问题：铃铛条目不出现", () => {
  assert.equal(habitBellNotice([], [], TODAY, OFFSET), null);
});

test("铃铛条目按候选与未决问题计数，并计入红标", () => {
  const notice = habitBellNotice(
    [candidate("a"), candidate("b"), candidate("c", { status: "archived" })],
    [question("q1", "2026-11-22T02:00:00Z"), question("q2", "2026-11-10T02:00:00Z")],
    TODAY,
    OFFSET,
  );
  assert.deepEqual(notice, {
    count_items: "2 条习惯候选，2 个未决问题",
    redBadge: true,
    overdueQuestions: 1,
  });
});

test("候选与问题处理完后条目消失", () => {
  assert.equal(
    habitBellNotice(
      [candidate("a", { status: "active" })],
      [question("q1", "2026-11-22T02:00:00Z", "closed")],
      TODAY,
      OFFSET,
    ),
    null,
  );
});

test("7 天未动的问题排前，其余按搁置天数从旧到新", () => {
  const sorted = sortOpenQuestions(
    [
      question("recent", "2026-11-20T02:00:00Z"),
      question("old", "2026-11-10T02:00:00Z"),
      question("edge", "2026-11-16T02:00:00Z"),
      question("closed", "2026-11-01T02:00:00Z", "closed"),
    ],
    TODAY,
    OFFSET,
  );
  assert.deepEqual(
    sorted.map((item) => [item.question.id, item.idleDays, item.overdue]),
    [
      ["old", 13, true],
      ["edge", 7, true],
      ["recent", 3, false],
    ],
  );
});

test("当天有新候选才显示首页提醒，关闭当天不再显示", () => {
  const cards = [candidate("a")];
  assert.equal(todayCandidateCount(cards, TODAY, OFFSET), 1);
  assert.equal(shouldShowHomeNotice(cards, TODAY, null, OFFSET), true);
  assert.equal(shouldShowHomeNotice(cards, TODAY, TODAY, OFFSET), false);
  assert.equal(shouldShowHomeNotice(cards, TODAY, "2026-11-22", OFFSET), true);
});

test("跨 UTC 午夜按本地日期判断新候选", () => {
  assert.equal(localDateFromUtcIso("2026-11-23T17:00:00Z", OFFSET), "2026-11-24");
  const cards = [candidate("a", { created_at: "2026-11-23T17:00:00Z" })];
  assert.equal(todayCandidateCount(cards, TODAY, OFFSET), 0);
  assert.equal(todayCandidateCount(cards, "2026-11-24", OFFSET), 1);
});

