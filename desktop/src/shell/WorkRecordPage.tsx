import { useState } from "react";

import type { Scene } from "./Dock";
import { ScenePage } from "./ScenePage";
import { TaskManager } from "./TaskManager";
import { TaskReviewPanel } from "./TaskReviewPanel";
import { WorkRecordLibrary } from "./WorkRecordLibrary";

type WorkTab = "records" | "review" | "tasks" | "weekly";

const TABS: Array<{ key: WorkTab; label: string }> = [
  { key: "records", label: "记录" },
  { key: "review", label: "审核" },
  { key: "tasks", label: "任务" },
  { key: "weekly", label: "周视图" },
];

/**
 * 工作记录工作区（OD-WR-303 / OD-WR-305）。
 *
 * 「记录」是新的记录库；「审核」是 AI 候选审核台；「任务」是确认后的任务清单；
 * 「周视图」保留既有的按周档案页（OD-ARCH-093），直到 OD-WR-310/311 切换数据源。
 */
export function WorkRecordPage({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const [tab, setTab] = useState<WorkTab>("records");
  const [focusRecordId, setFocusRecordId] = useState<string | null>(null);

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <nav
        className="px-6 md:pl-28 md:pr-12 pt-4 pb-1"
        aria-label="工作记录视图"
      >
        <div className="max-w-7xl mx-auto md:-translate-x-8 flex items-center gap-1">
          {TABS.map((item) => {
            const active = tab === item.key;
            return (
              <button
                key={item.key}
                onClick={() => setTab(item.key)}
                aria-current={active ? "page" : undefined}
                data-work-tab={item.key}
                className={
                  "rounded-full px-3.5 py-1.5 text-[12px] font-medium transition-colors " +
                  (active
                    ? "bg-primary text-on-primary"
                    : "text-on-surface-variant hover:bg-surface-container")
                }
              >
                {item.label}
              </button>
            );
          })}
        </div>
      </nav>

      {tab === "weekly" ? (
        <ScenePage scene="weekly" onNavigate={onNavigate} />
      ) : tab === "review" ? (
        <TaskReviewPanel
          onOpenRecord={(recordId) => {
            setFocusRecordId(recordId);
            setTab("records");
          }}
        />
      ) : tab === "tasks" ? (
        <TaskManager
          onOpenRecord={(recordId) => {
            setFocusRecordId(recordId);
            setTab("records");
          }}
        />
      ) : (
        <WorkRecordLibrary
          focusRecordId={focusRecordId}
          onFocusConsumed={() => setFocusRecordId(null)}
          onCandidatesReady={() => setTab("review")}
        />
      )}
    </div>
  );
}
