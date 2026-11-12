import { useRef, useState } from "react";

import { Icon, type IconName } from "../components/Icon";
import type { PeriodReviewPeriod } from "../types";
import type { Scene } from "./Dock";
import { ReviewArchive } from "./ReviewArchive";
import { TaskReviewPanel } from "./TaskReviewPanel";
import { WorkRecordLibrary } from "./WorkRecordLibrary";

type WorkTab = "records" | "reviews";

type ReviewTarget = { period: PeriodReviewPeriod; start: string; end: string };

const TABS: Array<{ key: WorkTab; label: string; icon: IconName }> = [
  { key: "records", label: "工作记录", icon: "clock" },
  { key: "reviews", label: "周期复盘", icon: "trend-up" },
];

/**
 * 工作记录分页（OD-WR-509 视觉重构）。
 *
 * 两个页签：工作记录（时间线 + 详情 + 当日摘要）与周期复盘（档案 + 生成）。
 * 审核候选改为从工作记录页右栏或记录详情打开的浮层，任务管理回到项目跟踪分页；
 * 页面不再出现独立的通用聊天框，录入仍以首页对话为主。
 */
export function WorkRecordPage({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const [tab, setTab] = useState<WorkTab>("records");
  const [focusRecordId, setFocusRecordId] = useState<string | null>(null);
  const [focusReview, setFocusReview] = useState<ReviewTarget | null>(null);
  const [monthCount, setMonthCount] = useState(0);
  const [auditOpen, setAuditOpen] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const pageRef = useRef<HTMLDivElement | null>(null);

  function selectTab(next: WorkTab) {
    setTab(next);
    pageRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  }

  function closeAudit() {
    setAuditOpen(false);
    setRefreshToken((value) => value + 1);
  }

  return (
    <div
      ref={pageRef}
      className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-10"
      data-work-record-page=""
    >
      <div className="mx-auto w-full max-w-[1400px]">
        <header className="wr-hero" data-work-record-hero="">
          <div className="wr-hero-mark" aria-hidden="true">
            <Icon name="note" className="h-6 w-6" />
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="wr-hero-title">工作记录</h1>
            <p className="wr-hero-sub">记录工作轨迹，沉淀思考与成长</p>
          </div>
          <span className="wr-hero-pill" data-month-count={monthCount}>
            <Icon name="trend-up" className="h-3.5 w-3.5" />
            本月记录
            <span className="wr-hero-pill-number">{monthCount}</span>
            条
          </span>
        </header>

        <nav className="wr-tabbar" aria-label="工作记录视图">
          {TABS.map((item) => {
            const active = tab === item.key;
            return (
              <button
                key={item.key}
                type="button"
                onClick={() => selectTab(item.key)}
                aria-current={active ? "page" : undefined}
                data-work-tab={item.key}
                className={"wr-tab" + (active ? " is-active" : "")}
              >
                <span className="wr-tab-icon" aria-hidden="true">
                  <Icon name={item.icon} className="h-4 w-4" />
                </span>
                {item.label}
              </button>
            );
          })}
        </nav>

        {tab === "reviews" ? (
          <ReviewArchive
            focusReview={focusReview}
            onFocusConsumed={() => setFocusReview(null)}
            onNavigate={onNavigate}
            onOpenRecord={(recordId) => {
              setFocusRecordId(recordId);
              selectTab("records");
            }}
          />
        ) : (
          <WorkRecordLibrary
            focusRecordId={focusRecordId}
            onFocusConsumed={() => setFocusRecordId(null)}
            onCandidatesReady={() => setAuditOpen(true)}
            onOpenAudit={() => setAuditOpen(true)}
            refreshToken={refreshToken}
            onOverview={(overview) => setMonthCount(overview.month)}
            onOpenReview={(target) => {
              setFocusReview(target);
              selectTab("reviews");
            }}
            onNavigate={onNavigate}
          />
        )}
      </div>

      {auditOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-3 md:p-8"
          data-work-audit=""
        >
          <button
            type="button"
            aria-label="关闭审核候选"
            onClick={closeAudit}
            className="absolute inset-0 cursor-default bg-[rgba(24,26,40,0.3)]"
          />
          <section
            aria-label="审核候选"
            className="wr-audit-modal relative flex h-[86vh] w-[min(1120px,96vw)] flex-col overflow-hidden"
          >
            <header className="wr-audit-head">
              <span className="wr-audit-mark" aria-hidden="true">
                <Icon name="shield-check" className="h-4 w-4" />
              </span>
              <div className="min-w-0 flex-1">
                <h2 className="text-[16px] font-semibold tracking-tight text-on-surface">
                  审核候选
                </h2>
                <p className="mt-0.5 text-[10.5px] text-outline">
                  AI 从记录中提取的任务候选，确认后才写入任务文件
                </p>
              </div>
              <button
                type="button"
                onClick={closeAudit}
                className="wr-icon-button"
                aria-label="关闭"
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </header>
            <TaskReviewPanel
              embedded
              onOpenRecord={(recordId) => {
                setFocusRecordId(recordId);
                selectTab("records");
                closeAudit();
              }}
            />
          </section>
        </div>
      ) : null}
    </div>
  );
}
