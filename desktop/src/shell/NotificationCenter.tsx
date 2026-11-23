import { useCallback, useEffect, useRef, useState } from "react";
import { habits, pendingWrites, scheduler, shell } from "../api";
import type { PendingWriteView, WorkTask } from "../types";
import { Icon, type IconName } from "../components/Icon";
import {
  HABITS_CHANGED_EVENT,
  habitBellNotice,
  openHabitWorkspace,
  systemUtcOffsetMinutes,
} from "../habitNotice";
import type { Scene } from "./Dock";

type NoticeKind = "confirm" | "task" | "review" | "habits";

interface NoticeItem {
  id: string;
  kind: NoticeKind;
  title: string;
  detail: string;
  scene: Scene;
  tone?: "error" | "normal";
}

const REFRESH_MS = 60_000;

function localToday(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

function daysUntil(date: string, today: string): number | null {
  const left = Date.parse(date + "T00:00:00");
  const right = Date.parse(today + "T00:00:00");
  if (!Number.isFinite(left) || !Number.isFinite(right)) return null;
  return Math.round((left - right) / 86_400_000);
}

function sceneForScenario(scenario: string): Scene {
  if (scenario === "material_writing") return "writing";
  if (scenario === "project_management") return "project";
  return "report";
}

function taskNotice(task: WorkTask, today: string): NoticeItem | null {
  if (task.state === "已完成" || task.state === "已取消") return null;
  if (!task.due_date) {
    return task.state === "待确认"
      ? {
          id: "task:" + task.id,
          kind: "task",
          title: task.title,
          detail: "待确认任务",
          scene: "project",
        }
      : null;
  }
  const days = daysUntil(task.due_date, today);
  if (days === null || days > 0) return null;
  return {
    id: "task:" + task.id,
    kind: "task",
    title: task.title,
    detail: days === 0 ? "今天到期" : `已逾期 ${-days} 天`,
    scene: "project",
    tone: days < 0 ? "error" : "normal",
  };
}

function pendingNotice(item: PendingWriteView): NoticeItem {
  return {
    id: "pending:" + item.id,
    kind: "confirm",
    title: item.title,
    detail: item.summary || "工作台生成的写入等待你确认",
    scene: sceneForScenario(item.scenario),
  };
}

/**
 * 顶栏通知与待办。
 *
 * 只聚合宿主已经存在的确定性列表：待确认写入、到期/逾期任务与到点的周期复盘。
 * 不调用模型、不生成新任务、不把通知点成空入口；没有内容时如实显示空态。
 */
export function NotificationCenter({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<NoticeItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const aliveRef = useRef(true);

  const refresh = useCallback(async () => {
    if (!aliveRef.current) return;
    setLoading(true);
    const today = localToday();
    const next: NoticeItem[] = [];
    const problems: string[] = [];
    try {
      const pending = await pendingWrites.list();
      next.push(...pending.items.filter((item) => item.status === "pending").map(pendingNotice));
      if (pending.broken > 0) problems.push(`${pending.broken} 条待确认记录无法读取`);
    } catch (e) {
      problems.push("待确认列表：" + String(e));
    }
    try {
      const listing = await shell.taskList({});
      const tasks = listing.items
        .map((task) => taskNotice(task, today))
        .filter((item): item is NoticeItem => item !== null)
        .sort((a, b) => Number(b.tone === "error") - Number(a.tone === "error"));
      next.push(...tasks);
      const unfinished = listing.items.filter(
        (task) => task.state !== "已完成" && task.state !== "已取消",
      ).length;
      if (unfinished > tasks.length) {
        next.push({
          id: "task-overview",
          kind: "task",
          title: `你的任务 · ${unfinished} 项未完成`,
          detail: "到期与逾期优先，其余在项目跟踪查看",
          scene: "project",
        });
      }
      if (listing.broken > 0) problems.push(`${listing.broken} 个任务文件无法读取`);
    } catch (e) {
      problems.push("任务列表：" + String(e));
    }
    try {
      const tick = await scheduler.tick();
      if (tick.review_due) {
        next.push({
          id: "review-due",
          kind: "review",
          title: "周期复盘到点",
          detail: "按调度设置提醒你整理本周工作",
          scene: "report",
        });
      }
    } catch {
      // 调度未启用或读取失败时不制造额外通知。
    }
    try {
      const [candidateList, questionList] = await Promise.all([
        habits.candidates(),
        habits.openQuestions(),
      ]);
      const notice = habitBellNotice(candidateList.items, questionList.items, today, systemUtcOffsetMinutes());
      const overdue = (notice?.overdueQuestions ?? 0) > 0;
      if (notice) {
        next.push({
          id: "habits",
          kind: "habits",
          title: notice.count_items,
          detail: overdue
            ? `${notice.overdueQuestions} 个问题已搁置 7 天以上`
            : "查看候选与未决问题",
          scene: "report",
          tone: overdue ? "error" : "normal",
        });
      }
      if (candidateList.broken + questionList.broken > 0) {
        problems.push(`${candidateList.broken + questionList.broken} 个习惯文件无法读取`);
      }
    } catch (e) {
      problems.push("习惯候选：" + String(e));
    }
    if (!aliveRef.current) return;
    setItems(next);
    setError(problems.length > 0 ? problems.join("；") : null);
    setLoading(false);
  }, []);

  useEffect(() => {
    aliveRef.current = true;
    void refresh();
    const timer = window.setInterval(() => void refresh(), REFRESH_MS);
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    window.addEventListener(HABITS_CHANGED_EVENT, onFocus);
    return () => {
      aliveRef.current = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener(HABITS_CHANGED_EVENT, onFocus);
    };
  }, [refresh]);

  useEffect(() => {
    if (!open) return undefined;
    void refresh();
    const onPointer = (event: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [open, refresh]);

  const count = items.length;
  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="neu-icon-btn relative w-8 h-8 rounded-full text-on-surface-variant hover:text-on-surface flex items-center justify-center"
        data-hover-hint={count > 0 ? `通知与待办：${count} 项` : "通知与待办"}
        aria-label={count > 0 ? `通知与待办，${count} 项需要处理` : "通知与待办"}
        aria-expanded={open}
        data-testid="topnav-notifications"
      >
        <Icon name="bell" className="w-[19px] h-[19px]" />
        {count > 0 ? (
          <span
            className="absolute -right-0.5 -top-0.5 min-w-[15px] rounded-full bg-primary px-1 text-center text-[9px] leading-[15px] text-on-primary"
            data-testid="topnav-notification-count"
          >
            {count > 99 ? "99+" : count}
          </span>
        ) : null}
      </button>
      {open ? (
        <div
          className="absolute right-0 top-10 z-50 w-[320px] max-w-[calc(100vw-24px)] rounded-2xl border border-outline-variant/50 bg-surface p-3 shadow-[0_18px_40px_-22px_rgba(70,55,40,0.45)]"
          data-testid="topnav-notification-popover"
        >
          <div className="mb-2 flex items-center justify-between gap-2">
            <p className="text-[12px] font-semibold text-on-surface">通知与待办</p>
            {loading ? <span className="text-[10px] text-outline">刷新中…</span> : null}
          </div>
          {error ? (
            <p className="mb-2 rounded-lg bg-error-container/50 px-2 py-1.5 text-[10.5px] leading-relaxed text-on-error-container">
              {error}
            </p>
          ) : null}
          {items.length === 0 ? (
            <p className="px-1 py-3 text-[11px] text-outline">目前没有需要处理的事项。</p>
          ) : (
            <div className="flex max-h-[360px] flex-col gap-1 overflow-y-auto">
              {items.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  onClick={() => {
                    setOpen(false);
                    if (item.kind === "habits") openHabitWorkspace();
                    else onNavigate(item.scene);
                  }}
                  className="flex w-full items-start gap-2 rounded-xl px-2 py-2 text-left hover:bg-surface-container-low"
                  data-testid="topnav-notification-item"
                  data-kind={item.kind}
                >
                  <Icon
                    name={(
                      item.kind === "confirm"
                        ? "checklist"
                        : item.kind === "review"
                          ? "trend-up"
                          : item.kind === "habits"
                            ? "spark"
                            : "clock"
                    ) as IconName}
                    className={
                      "mt-0.5 h-3.5 w-3.5 shrink-0 " +
                      (item.tone === "error" ? "text-error" : "text-primary/80")
                    }
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12px] font-medium text-on-surface">{item.title}</span>
                    <span className="mt-0.5 block truncate text-[10.5px] text-outline">{item.detail}</span>
                  </span>
                </button>
              ))}
            </div>
          )}
          <p className="mt-2 border-t border-outline-variant/30 pt-2 text-[10px] text-outline">
            待确认写入、到期任务与到点复盘会自动出现在这里。
          </p>
        </div>
      ) : null}
    </div>
  );
}
