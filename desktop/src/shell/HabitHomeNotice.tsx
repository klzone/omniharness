import { useCallback, useEffect, useState } from "react";

import { habits } from "../api";
import { Icon } from "../components/Icon";
import {
  HABIT_HOME_DISMISSED_KEY,
  HABITS_CHANGED_EVENT,
  localToday,
  openHabitWorkspace,
  shouldShowHomeNotice,
  systemUtcOffsetMinutes,
  todayCandidateCount,
} from "../habitNotice";
import type { HabitCard } from "../types";

/**
 * 首页习惯提醒条（OD-GROW-301 G5）。
 *
 * 当天出现新候选时显示一条，用户查看或关闭后把本地日期写进 localStorage，
 * 当天不再出现；第二天有新候选时重新出现。没有新候选时完全让出版面。
 */
export function HabitHomeNotice() {
  const [candidates, setCandidates] = useState<HabitCard[]>([]);
  const [dismissed, setDismissed] = useState<string | null>(() => {
    try {
      return window.localStorage.getItem(HABIT_HOME_DISMISSED_KEY);
    } catch {
      return null;
    }
  });

  const load = useCallback(async () => {
    try {
      const listing = await habits.candidates();
      setCandidates(listing.items);
    } catch {
      // 读不到候选时首页不制造提醒，也不覆盖已有数据。
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

  function dismiss() {
    const today = localToday(Date.now() / 1000, systemUtcOffsetMinutes());
    setDismissed(today);
    try {
      window.localStorage.setItem(HABIT_HOME_DISMISSED_KEY, today);
    } catch {
      // 缓存写不进去时只影响本次；关闭按钮仍立即隐藏提醒。
    }
  }

  const offset = systemUtcOffsetMinutes();
  const today = localToday(Date.now() / 1000, offset);
  const counts = todayCandidateCount(candidates, today, offset);
  if (!shouldShowHomeNotice(candidates, today, dismissed, offset)) return null;

  return (
    <section
      className="mb-2 flex items-center gap-2.5 rounded-2xl bg-[color:var(--od-neu-surface)] px-4 py-2.5 shadow-[var(--od-neu-raise-1)]"
      data-habit-home-notice=""
    >
      <Icon name="spark" className="h-4 w-4 shrink-0 text-[var(--od-neu-accent-ink)]" />
      <p className="min-w-0 flex-1 text-[12px] text-on-surface">
        今天有 <strong>{counts}</strong> 条习惯候选
      </p>
      <button
        type="button"
        className="wr-ghost-button"
        onClick={() => {
          dismiss();
          openHabitWorkspace();
        }}
      >
        查看
      </button>
      <button
        type="button"
        className="wr-icon-button"
        aria-label="关闭习惯提醒"
        onClick={dismiss}
      >
        <Icon name="x" className="h-3.5 w-3.5" />
      </button>
    </section>
  );
}

