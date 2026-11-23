import assert from "node:assert/strict";
import { test } from "node:test";

import {
  acceptAllDecisions,
  defaultScopeForCard,
  rejectDecision,
  setScopeChoice,
  toggleCandidateScope,
} from "./habitCardsView.ts";

function card(id, scope = { type: "scene", scene: "report" }) {
  return {
    schema: "omnidesk.habit-card.v1",
    id,
    kind: "preference",
    scope,
    text: "候选 " + id,
    evidence: { thread_id: "t1", message_id: "m1", date: "2026-11-23" },
    origin: "explicit",
    status: "candidate",
    created_at: "2026-11-23T02:00:00Z",
    last_seen_at: "2026-11-23T02:00:00Z",
  };
}

test("默认范围取候选自身场景，场景内与全局可来回切换", () => {
  assert.deepEqual(defaultScopeForCard(card("a")), { type: "scene", scene: "report" });
  assert.deepEqual(defaultScopeForCard(card("b", { type: "global" })), { type: "global" });
  assert.deepEqual(toggleCandidateScope({ type: "scene", scene: "report" }, "report"), {
    type: "global",
  });
  assert.deepEqual(toggleCandidateScope({ type: "global" }, "writing"), {
    type: "scene",
    scene: "writing",
  });
});

test("范围选择按候选编号写入且不改原对象", () => {
  const before = { a: { type: "global" } };
  const after = setScopeChoice(before, "b", { type: "scene", scene: "report" });
  assert.deepEqual(before, { a: { type: "global" } });
  assert.deepEqual(after, {
    a: { type: "global" },
    b: { type: "scene", scene: "report" },
  });
});

test("全部接受沿用已选范围，未选的候选使用默认范围", () => {
  const cards = [card("a"), card("b", { type: "global" })];
  const decisions = acceptAllDecisions(
    cards,
    { a: { type: "global" } },
  );
  assert.deepEqual(decisions, [
    { id: "a", action: "accept", scope: { type: "global" } },
    { id: "b", action: "accept", scope: { type: "global" } },
  ]);
});

test("驳回可带原因代码，也可不填", () => {
  assert.deepEqual(rejectDecision("a", "too_absolute"), {
    id: "a",
    action: "reject",
    reason_code: "too_absolute",
  });
  assert.deepEqual(rejectDecision("b", null), {
    id: "b",
    action: "reject",
    reason_code: null,
  });
});

