import { useCallback, useEffect, useMemo, useState } from "react";

import { habits } from "../api";
import { Icon } from "../components/Icon";
import {
  HABITS_CHANGED_EVENT,
  localToday,
  notifyHabitsChanged,
  sortOpenQuestions,
  systemUtcOffsetMinutes,
} from "../habitNotice";
import {
  HABIT_REJECT_REASONS,
  acceptAllDecisions,
  acceptDecision,
  defaultScopeForCard,
  rejectDecision,
  scopeLabel,
  setScopeChoice,
  type HabitRejectReason,
} from "../habitCardsView";
import { listThreads } from "../sceneThreads";
import type { HabitCard, HabitScope, OpenQuestion, PendingReviewItem } from "../types";

const CARD = "wr-side-card";
const CHIP = "wr-chip";

function threadLabel(id: string, threads: Map<string, { title: string }>): string {
  const title = threads.get(id)?.title;
  return title ? title : id.slice(0, 10);
}

/**
 * 每日习惯整理面板（OD-GROW-301 G5）。
 *
 * 候选、未决问题与待审定条目都来自宿主已有命令；界面只做选择与提交。
 * 驳回原因可选，提交后由宿主写驳回哈希，界面不重复实现。
 */
export function HabitDigestPanel() {
  const [candidates, setCandidates] = useState<HabitCard[]>([]);
  const [questions, setQuestions] = useState<OpenQuestion[]>([]);
  const [pending, setPending] = useState<PendingReviewItem[]>([]);
  const [choices, setChoices] = useState<Record<string, HabitScope>>({});
  const [rejecting, setRejecting] = useState<string | null>(null);
  const [reason, setReason] = useState<HabitRejectReason | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const threads = useMemo(
    () => new Map(listThreads().map((thread) => [thread.id, thread])),
    [],
  );
  const offset = systemUtcOffsetMinutes();
  const today = localToday(Date.now() / 1000, offset);
  const openAges = useMemo(
    () => sortOpenQuestions(questions, today, offset),
    [questions, today, offset],
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [candidateList, questionList, pendingList] = await Promise.all([
        habits.candidates(),
        habits.openQuestions(),
        habits.pendingReviews(),
      ]);
      setCandidates(candidateList.items.filter((card) => card.status === "candidate"));
      setQuestions(questionList.items);
      setPending(pendingList.items);
      setChoices((previous) => {
        const next = { ...previous };
        for (const card of candidateList.items) {
          if (!next[card.id]) next[card.id] = defaultScopeForCard(card);
        }
        return next;
      });
      const broken =
        candidateList.broken + questionList.broken + pendingList.broken;
      setError(broken > 0 ? `${broken} 个习惯文件无法读取` : null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const refresh = () => void load();
    window.addEventListener(HABITS_CHANGED_EVENT, refresh);
    window.addEventListener("focus", refresh);
    return () => {
      window.removeEventListener(HABITS_CHANGED_EVENT, refresh);
      window.removeEventListener("focus", refresh);
    };
  }, [load]);

  async function submit(label: string, task: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await task();
      notifyHabitsChanged();
      await load();
    } catch (e) {
      setError(label + "失败：" + String(e));
    } finally {
      setBusy(false);
    }
  }

  function reject(card: HabitCard) {
    setRejecting(null);
    void submit("驳回候选", () => habits.decide([rejectDecision(card.id, reason)]));
    setReason(null);
  }

  return (
    <>
      <section className={CARD} data-habit-digest="">
        <div className="wr-side-head">
          <span className="wr-side-mark" aria-hidden="true">
            <Icon name="spark" className="h-4 w-4" />
          </span>
          <h2 className="wr-side-title">习惯候选</h2>
          <span className="mono ml-auto text-[11px] text-outline">{candidates.length}</span>
          <button
            type="button"
            className="wr-primary-button ml-1"
            disabled={busy || candidates.length === 0}
            onClick={() => void submit("全部接受", () => habits.decide(acceptAllDecisions(candidates, choices)))}
          >
            全部接受
          </button>
        </div>

        {error ? <p className="wr-load-state mt-3 text-error">{error}</p> : null}

        <div className="mt-3 flex flex-col gap-2">
          {loading && candidates.length === 0 ? (
            <p className="wr-load-state">正在读取候选…</p>
          ) : candidates.length === 0 ? (
            <p className="wr-empty-state">没有待处理的候选。</p>
          ) : (
            candidates.map((card) => {
              const scope = choices[card.id] ?? defaultScopeForCard(card);
              return (
                <article key={card.id} className="wr-task-row !items-start" data-habit-candidate={card.id}>
                  <div className="min-w-0 flex-1">
                    <p className="text-[12px] font-medium leading-relaxed text-on-surface">{card.text}</p>
                    <p className="mt-1 text-[10.5px] text-outline">
                      {card.evidence.date} · {threadLabel(card.evidence.thread_id, threads)}
                    </p>
                    <div className="mt-1.5 flex items-center gap-1.5">
                      <div className="wr-chipgroup">
                        <button
                          type="button"
                          className={CHIP + (scope.type === "scene" ? " is-active" : "")}
                          onClick={() =>
                            setChoices((state) =>
                              setScopeChoice(state, card.id, {
                                type: "scene",
                                scene:
                                  scope.type === "scene"
                                    ? scope.scene
                                    : threads.get(card.evidence.thread_id)?.tag ?? "report",
                              }),
                            )
                          }
                        >
                          仅本场景
                        </button>
                        <button
                          type="button"
                          className={CHIP + (scope.type === "global" ? " is-active" : "")}
                          onClick={() => setChoices((state) => setScopeChoice(state, card.id, { type: "global" }))}
                        >
                          全局
                        </button>
                      </div>
                      <span className="text-[10px] text-outline">{scopeLabel(scope)}</span>
                    </div>
                    {rejecting === card.id ? (
                      <div className="mt-2 flex flex-wrap items-center gap-1.5">
                        {HABIT_REJECT_REASONS.map((item) => (
                          <button
                            key={item.code}
                            type="button"
                            className={CHIP + (reason === item.code ? " is-active" : "")}
                            onClick={() => setReason(item.code)}
                          >
                            {item.label}
                          </button>
                        ))}
                        <button type="button" className="wr-text-button" onClick={() => setReason(null)}>
                          不填
                        </button>
                        <button type="button" className="wr-ghost-button" disabled={busy} onClick={() => reject(card)}>
                          确认驳回
                        </button>
                        <button type="button" className="wr-text-button" onClick={() => setRejecting(null)}>
                          取消
                        </button>
                      </div>
                    ) : null}
                  </div>
                  <div className="flex shrink-0 flex-col gap-1">
                    <button
                      type="button"
                      className="wr-ghost-button"
                      disabled={busy}
                      onClick={() => void submit("接受候选", () => habits.decide([acceptDecision(card.id, scope)]))}
                    >
                      接受
                    </button>
                    <button
                      type="button"
                      className="wr-text-button"
                      disabled={busy}
                      onClick={() => {
                        setRejecting(card.id);
                        setReason(null);
                      }}
                    >
                      驳回
                    </button>
                  </div>
                </article>
              );
            })
          )}
        </div>
      </section>

      {openAges.length > 0 ? (
        <section className={CARD} data-habit-questions="">
          <div className="flex items-baseline gap-2">
            <h2 className="wr-side-section-title">未决问题</h2>
            <span className="mono ml-auto text-[11px] text-outline">{openAges.length}</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {openAges.map(({ question, idleDays, overdue }) => (
              <article key={question.id} className="wr-task-row !items-start" data-habit-question={question.id}>
                <div className="min-w-0 flex-1">
                  <p className="text-[12px] leading-relaxed text-on-surface">{question.text}</p>
                  <p className={"mt-1 text-[10.5px] " + (overdue ? "text-error" : "text-outline")}>
                    搁置 {idleDays} 天 · {threadLabel(question.thread_id, threads)}
                  </p>
                </div>
                <div className="flex shrink-0 gap-1">
                  <button
                    type="button"
                    className="wr-text-button"
                    disabled={busy}
                    onClick={() => void submit("标记已定", () => habits.setOpenQuestionStatus(question.id, "closed"))}
                  >
                    已定
                  </button>
                  <button
                    type="button"
                    className="wr-text-button"
                    disabled={busy}
                    onClick={() => void submit("放弃问题", () => habits.setOpenQuestionStatus(question.id, "dropped"))}
                  >
                    放弃
                  </button>
                </div>
              </article>
            ))}
          </div>
        </section>
      ) : null}

      {pending.length > 0 ? (
        <section className={CARD} data-habit-pending="">
          <div className="flex items-baseline gap-2">
            <h2 className="wr-side-section-title">待审定</h2>
            <span className="mono ml-auto text-[11px] text-outline">{pending.length}</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {pending.map((item) => (
              <article key={item.id} className="wr-task-row !items-start" data-habit-pending-item={item.id}>
                <p className="min-w-0 flex-1 text-[11.5px] leading-relaxed text-on-surface-variant">{item.text}</p>
                <div className="flex shrink-0 gap-1">
                  <button
                    type="button"
                    className="wr-ghost-button"
                    disabled={busy}
                    onClick={() => void submit("批准条目", () => habits.decidePendingReview(item.id, "approve"))}
                  >
                    批准
                  </button>
                  <button
                    type="button"
                    className="wr-text-button"
                    disabled={busy}
                    onClick={() => void submit("排除条目", () => habits.decidePendingReview(item.id, "exclude"))}
                  >
                    排除
                  </button>
                </div>
              </article>
            ))}
          </div>
        </section>
      ) : null}
    </>
  );
}

