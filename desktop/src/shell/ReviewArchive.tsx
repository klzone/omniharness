import { useEffect, useState } from "react";

import { host, shell } from "../api";
import { composeConversationPrompt, type ConversationPreset } from "../conversationPrompt";
import { requestNewThread } from "../sceneThreads";
import { Icon } from "../components/Icon";
import { TERMINAL_STATES } from "../types";
import type {
  PeriodReviewListing,
  PeriodReviewPeriod,
  PeriodReviewRead,
  WorkTask,
} from "../types";
import { MarkdownText } from "./MarkdownText";
import type { Scene } from "./Dock";

type Notice = { kind: "ok" | "err"; text: string };
type PeriodFilter = "all" | PeriodReviewPeriod;
type StatusFilter = "all" | "草稿" | "已确认";
type ReviewTarget = { period: PeriodReviewPeriod; start: string; end: string };

const CARD =
  "bg-white/80 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "rounded-full border border-outline-variant/50 bg-white px-3 py-1.5 text-[11.5px] text-on-surface-variant focus:outline-none focus:ring-1 focus:ring-primary/30";

const PERIOD_FILTERS: Array<{ key: PeriodFilter; label: string }> = [
  { key: "all", label: "全部" },
  { key: "weekly", label: "周" },
  { key: "monthly", label: "月" },
  { key: "quarterly", label: "季" },
  { key: "annual", label: "年" },
];

const STATUS_FILTERS: Array<{ key: StatusFilter; label: string }> = [
  { key: "all", label: "全部状态" },
  { key: "草稿", label: "草稿" },
  { key: "已确认", label: "已确认" },
];

const PERIOD_LABEL: Record<PeriodReviewPeriod, string> = {
  weekly: "周",
  monthly: "月",
  quarterly: "季",
  annual: "年",
};

const PERIOD_FULL_LABEL: Record<PeriodReviewPeriod, string> = {
  weekly: "周度",
  monthly: "月度",
  quarterly: "季度",
  annual: "年度",
};

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

function reviewKey(target: ReviewTarget): string {
  return target.period + ":" + target.start + "_" + target.end;
}

function formatStamp(iso: string | null): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, "0");
  return (
    date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate()) +
    " " + pad(date.getHours()) + ":" + pad(date.getMinutes())
  );
}

export function ReviewArchive({
  focusReview,
  onFocusConsumed,
  onOpenRecord,
  onNavigate,
}: {
  focusReview: ReviewTarget | null;
  onFocusConsumed: () => void;
  onOpenRecord: (recordId: string) => void;
  onNavigate: (scene: Scene) => void;
}) {
  const [listing, setListing] = useState<PeriodReviewListing>({ items: [], broken: 0 });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [periodFilter, setPeriodFilter] = useState<PeriodFilter>("all");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [selected, setSelected] = useState<ReviewTarget | null>(null);
  const [detail, setDetail] = useState<PeriodReviewRead | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [recordTitles, setRecordTitles] = useState<Record<string, string>>({});
  const [tasks, setTasks] = useState<Record<string, WorkTask>>({});
  const [notice, setNotice] = useState<Notice | null>(null);
  const [busy, setBusy] = useState(false);
  const [regenStatus, setRegenStatus] = useState<string | null>(null);
  const [genOpen, setGenOpen] = useState(false);

  /**
   * 交给 AI 继续分析：只预填复盘周期与文档编号，不自动发送。
   * 全局对话采集保持保守不扩：不把对话内容写回复盘或记录，避免任何输入被误记。
   */
  function handReviewToAi() {
    if (!detail || !selected) return;
    requestNewThread(
      "weekly",
      "请继续分析" +
        PERIOD_FULL_LABEL[detail.period] +
        "复盘（" +
        detail.range +
        "），文档 #" +
        detail.period +
        "/" +
        detail.range +
        "。引用记录与任务见复盘档案，不要修改任何文件，只做分析。",
    );
    onNavigate("home");
  }

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .worklogPeriodReviewList()
      .then((result) => {
        if (!alive) return;
        setListing(result);
        setLoadError(null);
      })
      .catch((e) => {
        if (alive) setLoadError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    if (!focusReview) return;
    setSelected(focusReview);
    setNotice(null);
    onFocusConsumed();
  }, [focusReview]);

  useEffect(() => {
    if (!selected) {
      setDetail(null);
      setDetailError(null);
      return;
    }
    let alive = true;
    setDetailLoading(true);
    setDetailError(null);
    shell
      .worklogPeriodReviewRead(selected.period, selected.start, selected.end)
      .then((read) => {
        if (!alive) return;
        setDetail(read);
      })
      .catch((e) => {
        if (alive) setDetailError(errText(e));
      })
      .finally(() => {
        if (alive) setDetailLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected, reloadTick]);

  useEffect(() => {
    if (!detail) {
      setRecordTitles({});
      setTasks({});
      return;
    }
    let alive = true;
    const nextTitles: Record<string, string> = {};
    setRecordTitles({});
    setTasks({});
    detail.record_ids.forEach((id) => {
      shell
        .workRecordRead(id)
        .then((record) => {
          if (!alive) return;
          nextTitles[id] = record.title;
          setRecordTitles({ ...nextTitles });
        })
        .catch(() => {
          // 已删除的引用不阻断详情，下方用编号与「已删除」标注。
        });
    });
    if (detail.task_ids.length > 0) {
      shell
        .taskList({})
        .then((list) => {
          if (!alive) return;
          const map: Record<string, WorkTask> = {};
          list.items.forEach((task) => {
            if (detail.task_ids.includes(task.id)) map[task.id] = task;
          });
          setTasks(map);
        })
        .catch(() => {
          // 任务标题读不到时只显示编号。
        });
    }
    return () => {
      alive = false;
    };
  }, [detail]);

  const filtered = listing.items.filter((item) => {
    if (periodFilter !== "all" && item.period !== periodFilter) return false;
    if (statusFilter !== "all" && item.status !== statusFilter) return false;
    if (fromDate && item.end < fromDate) return false;
    if (toDate && item.start > toDate) return false;
    return true;
  });

  const selectedKey = selected ? reviewKey(selected) : "";
  const missingRecords = detail ? new Set(detail.missing_record_ids) : new Set<string>();
  const missingTasks = detail ? new Set(detail.missing_task_ids) : new Set<string>();

  /** 生成入口只预填首页对话，不在档案页直接调用模型或写文件。 */
  function generate(period: PeriodReviewPeriod) {
    const prompt =
      period === "weekly"
        ? "周复盘"
        : period === "monthly"
          ? "复盘本月"
          : period === "quarterly"
            ? "复盘本季度"
            : "复盘今年";
    requestNewThread("weekly", prompt);
    onNavigate("home");
  }

  function exportMarkdown() {
    if (!detail?.exists || !selected) return;
    const blob = new Blob([detail.body.trim() + "\n"], {
      type: "text/markdown;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = selected.start + "_" + selected.end + ".md";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    setNotice({ kind: "ok", text: "已导出 " + anchor.download + "。" });
  }

  async function regenerate() {
    if (!detail?.exists || !selected || busy) return;
    if (detail.status !== "草稿") {
      setNotice({ kind: "err", text: "已确认复盘不可覆盖。" });
      return;
    }
    setBusy(true);
    setNotice({ kind: "ok", text: "正在汇集复盘材料…" });
    setRegenStatus(null);
    try {
      const bundle = await shell.worklogPeriodReviewCollect(
        selected.period,
        selected.start,
        selected.end,
      );
      const rulesView = await shell.profileRules().catch(() => null);
      const bound = rulesView?.rules?.skill_bindings?.["weekly-period-review"];
      const maxInjected = bound?.max_injected ?? 3;
      const skills = (bound?.skills ?? [])
        .slice(0, maxInjected)
        .map((id) => ({ id, content: rulesView?.skill_contents?.[id] ?? "" }))
        .filter((skill) => skill.content.trim().length > 0);
      const preset: ConversationPreset = {
        key: "weekly",
        label: "工作记录",
        rules: null,
        sections: null,
        writingRules: rulesView?.rules?.global?.writing_rules ?? null,
        thinking: null,
        skills: null,
        skillsByFlow: skills.length > 0 ? { periodReview: skills } : null,
      };
      const promptText = composeConversationPrompt({
        preset,
        turns: [],
        current:
          "重新生成" + PERIOD_FULL_LABEL[bundle.period] + "复盘：" +
          bundle.start + " 至 " + bundle.end,
        mode: "period-review",
        periodReview: {
          period: bundle.period,
          label: PERIOD_FULL_LABEL[bundle.period],
          start: bundle.start,
          end: bundle.end,
          material: bundle.material,
          recordCount: bundle.record_count,
          taskCount: bundle.task_count,
          taskStats: bundle.task_stats,
          priorReviews: bundle.prior_reviews,
        },
      });
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      setNotice({ kind: "ok", text: "已提交生成任务，等待模型…" });
      const receipt = await host.submitTask({
        promptText,
        intent: "weekly_report",
        workspaceDir: workspace.workspace_dir,
        userInputText:
          "重新生成复盘 " + bundle.start + " 至 " + bundle.end,
      });
      const taskId = receipt.omni_task_id;
      let status = await host.taskStatus(taskId);
      for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(taskId);
        if (i % 10 === 0) {
          setRegenStatus("模型正在生成…已等待 " + (i + 1) + " 秒");
        }
      }
      if (status.status !== "succeeded") {
        throw new Error(
          "生成未完成：" + status.status + (status.note ? " · " + status.note : ""),
        );
      }
      const result = await host.taskResult(taskId);
      if (!result.content.trim()) {
        throw new Error("模型没有返回复盘正文，本次未写入。");
      }
      const saved = await shell.worklogPeriodReviewGenerate(
        bundle.period,
        bundle.start,
        bundle.end,
        result.content,
      );
      setNotice({
        kind: "ok",
        text:
          "已写入复盘草稿" +
          (saved.backup ? "，旧草稿已备份为 .bak" : "") +
          "。",
      });
      setReloadTick((value) => value + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
      setRegenStatus(null);
    }
  }

  return (
    <div className="wr-reviews" data-review-archive="">
      <section aria-label="复盘筛选" className="wr-review-toolbar">
        <div className="wr-chipgroup" data-review-period-filters="">
          {PERIOD_FILTERS.map((option) => {
            const active = periodFilter === option.key;
            return (
              <button
                key={option.key}
                type="button"
                onClick={() => setPeriodFilter(option.key)}
                aria-pressed={active}
                data-review-period-filter={option.key}
                className={"wr-chip" + (active ? " is-active" : "")}
              >
                {option.label}
              </button>
            );
          })}
        </div>
        <span className="wr-toolbar-divider" />
        <div className="wr-chipgroup" data-review-status-filters="">
          {STATUS_FILTERS.map((option) => {
            const active = statusFilter === option.key;
            return (
              <button
                key={option.key}
                type="button"
                onClick={() => setStatusFilter(option.key)}
                aria-pressed={active}
                data-review-status-filter={option.key}
                className={"wr-chip" + (active ? " is-active" : "")}
              >
                {option.label}
              </button>
            );
          })}
        </div>
        <span className="wr-review-range">
          <input
            type="date"
            value={fromDate}
            onChange={(e) => setFromDate(e.target.value)}
            aria-label="复盘范围起始"
            data-review-from=""
            className={FIELD}
          />
          <span className="text-[11px] text-outline/70">—</span>
          <input
            type="date"
            value={toDate}
            onChange={(e) => setToDate(e.target.value)}
            aria-label="复盘范围结束"
            data-review-to=""
            className={FIELD}
          />
        </span>
        <div className="relative ml-auto">
          <button
            type="button"
            onClick={() => setGenOpen((open) => !open)}
            aria-expanded={genOpen}
            data-review-generate-entry=""
            className="wr-primary-button"
          >
            <Icon name="plus" className="h-3.5 w-3.5" />
            生成复盘
          </button>
          {genOpen ? (
            <>
              <button
                type="button"
                aria-label="关闭生成选项"
                className="wr-popover-backdrop"
                onClick={() => setGenOpen(false)}
              />
              <div className="wr-popover wr-popover-right" data-review-generate-menu="">
                <p className="wr-popover-title">生成什么周期</p>
                <div className="flex flex-col gap-1">
                  {PERIOD_FILTERS.filter((item) => item.key !== "all").map((item) => (
                    <button
                      key={item.key}
                      type="button"
                      onClick={() => {
                        setGenOpen(false);
                        generate(item.key as PeriodReviewPeriod);
                      }}
                      data-review-generate={item.key}
                      className="wr-generate-option"
                    >
                      <span className="wr-generate-icon" aria-hidden="true">
                        <Icon name="calendar" className="h-3.5 w-3.5" />
                      </span>
                      <span className="min-w-0 flex-1 text-left">
                        <span className="block text-[11.5px] font-medium text-on-surface">
                          {PERIOD_LABEL[item.key as PeriodReviewPeriod]}复盘
                        </span>
                        <span className="block text-[10px] text-outline">
                          {item.key === "weekly"
                            ? "本周记录与任务"
                            : item.key === "monthly"
                              ? "本月记录与任务"
                              : item.key === "quarterly"
                                ? "本季度记录与任务"
                                : "今年记录与任务"}
                        </span>
                      </span>
                      <Icon name="arrow-right" className="h-3 w-3 text-outline" />
                    </button>
                  ))}
                </div>
                <p className="mt-2 text-[10px] leading-relaxed text-outline">
                  会打开首页对话并预填周期，正文生成与确认仍在对话里完成。
                </p>
              </div>
            </>
          ) : null}
        </div>
      </section>

      <div className="wr-review-grid">
        <section
          aria-label="复盘列表"
          className={CARD + " wr-review-list"}
        >
          <div className="wr-review-list-head">
            <h2 className="wr-side-title">复盘档案</h2>
            {!loading && !loadError ? (
              <span className="ml-auto mono text-[10.5px] text-outline">
                {filtered.length} / {listing.items.length}
              </span>
            ) : null}
          </div>
          <div className="wr-review-list-body" data-review-list="">
            {loading ? (
              <p className="px-1.5 py-3 text-[11.5px] text-outline">正在读取复盘档案…</p>
            ) : loadError ? (
              <p className="px-1.5 py-3 text-[11.5px] leading-relaxed text-error">
                读取复盘档案失败：{loadError}
              </p>
            ) : listing.items.length === 0 ? (
              <div className="wr-review-empty" data-review-empty="">
                <p className="text-[11.5px] leading-relaxed text-outline">
                  还没有复盘文档。生成后会自动出现在这里。
                </p>
                <button
                  type="button"
                  onClick={() => generate("monthly")}
                  className="wr-primary-button mt-2"
                >
                  生成本月复盘
                </button>
              </div>
            ) : filtered.length === 0 ? (
              <p className="px-1.5 py-6 text-[11.5px] text-outline">当前筛选没有匹配的复盘。</p>
            ) : (
              <div className="flex flex-col gap-1.5">
                {filtered.map((item) => {
                  const target = {
                    period: item.period,
                    start: item.start,
                    end: item.end,
                  };
                  const active = reviewKey(target) === selectedKey;
                  return (
                    <button
                      key={reviewKey(target)}
                      type="button"
                      onClick={() => {
                        setSelected(target);
                        setNotice(null);
                      }}
                      data-review-item={reviewKey(target)}
                      data-review-item-status={item.status ?? ""}
                      className={"wr-review-item" + (active ? " is-active" : "")}
                    >
                      <span className="flex items-center gap-1.5 min-w-0">
                        <span className="wr-review-period">
                          {PERIOD_LABEL[item.period]}
                        </span>
                        <span className="mono min-w-0 flex-1 truncate text-[11px] text-on-surface-variant">
                          {item.range}
                        </span>
                        <span
                          className={
                            "wr-review-status " +
                            (item.status === "已确认" ? "is-confirmed" : "is-draft")
                          }
                        >
                          {item.status ?? "未知"}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
            {listing.broken > 0 ? (
              <p className="px-1.5 pt-2 text-[10.5px] text-outline" data-review-broken="">
                {listing.broken} 个文件无法读取，未计入列表。
              </p>
            ) : null}
          </div>
        </section>

        <section
          aria-label="复盘详情"
          className={CARD + " wr-review-detail"}
          data-review-detail=""
        >
          {!selected ? (
            <div className="wr-review-placeholder">
              <p className="text-[13px] font-medium text-on-surface">选择一份复盘查看正文。</p>
              <p className="mt-1.5 text-[11px] leading-relaxed text-outline">
                左侧列出周、月、季、年复盘。生成入口在筛选栏右侧，内容由首页对话里的 AI 起草。
              </p>
            </div>
          ) : detailLoading ? (
            <p className="text-[12px] text-outline">正在读取复盘…</p>
          ) : detailError ? (
            <p className="text-[12px] leading-relaxed text-error" data-review-detail-error="">
              读取复盘失败：{detailError}
            </p>
          ) : !detail?.exists ? (
            <p className="text-[12px] leading-relaxed text-outline">
              复盘文件不存在，可能已被移动或删除。
            </p>
          ) : (
            <div className="flex flex-col min-h-0">
              <div className="flex items-start gap-2.5">
                <div className="min-w-0 flex-1">
                  <p className="wr-eyebrow">
                    {PERIOD_FULL_LABEL[detail.period]}复盘
                  </p>
                  <h2 className="mt-0.5 text-[24px] font-semibold leading-snug tracking-tight text-on-surface">
                    {detail.range}
                  </h2>
                </div>
                <span
                  className={
                    "wr-review-status " +
                    (detail.status === "已确认" ? "is-confirmed" : "is-draft")
                  }
                  data-review-detail-status={detail.status ?? ""}
                >
                  {detail.status ?? "未知"}
                </span>
              </div>
              <div className="wr-review-facts">
                <span>生成 {formatStamp(detail.generated_at)}</span>
                <span>确认 {formatStamp(detail.confirmed_at)}</span>
                <span>{detail.record_ids.length} 条记录</span>
                <span>{detail.task_ids.length} 个任务</span>
              </div>
              <div className="mt-3 flex flex-wrap items-center gap-1.5">
                <button
                  type="button"
                  onClick={exportMarkdown}
                  data-review-export=""
                  className="wr-ghost-button"
                >
                  <Icon name="folder" className="h-3.5 w-3.5" />
                  导出 MD
                </button>
                <button
                  type="button"
                  onClick={() => void regenerate()}
                  disabled={busy || detail.status !== "草稿"}
                  data-review-regenerate=""
                  title={
                    detail.status === "已确认"
                      ? "已确认复盘不可覆盖"
                      : "重新生成草稿（旧版备份为 .bak）"
                  }
                  className="wr-primary-button"
                >
                  <Icon name="refresh" className="h-3.5 w-3.5" />
                  重新生成
                </button>
                <button
                  type="button"
                  onClick={handReviewToAi}
                  data-hand-review={reviewKey(selected)}
                  className="wr-ghost-button"
                >
                  <Icon name="spark" className="h-3.5 w-3.5" />
                  交给 AI 继续分析
                </button>
                {busy && regenStatus ? (
                  <span className="text-[10.5px] text-outline" data-review-progress="">
                    {regenStatus}
                  </span>
                ) : null}
              </div>
              {detail.status === "已确认" ? (
                <p
                  className="mt-1.5 text-[10.5px] leading-relaxed text-outline"
                  data-review-confirmed-lock=""
                >
                  已确认复盘不可覆盖。如需更新，请换一个周期。
                </p>
              ) : null}
              {detail.stale ? (
                <p className="mt-1.5 text-[10.5px] leading-relaxed text-error" data-review-stale="">
                  引用的记录或任务已有删除：记录 {detail.missing_record_ids.length} 条，任务{" "}
                  {detail.missing_task_ids.length} 个。
                </p>
              ) : null}
              {notice ? (
                <p
                  className={
                    "mt-2 text-[11px] leading-relaxed " +
                    (notice.kind === "err" ? "text-error" : "text-primary")
                  }
                  data-review-notice={notice.kind}
                >
                  {notice.text}
                </p>
              ) : null}

              <div className="mt-3 pt-3 border-t border-surface-variant/25 flex-1 min-h-0 overflow-y-auto">
                <div data-review-body="">
                  <MarkdownText text={detail.body} />
                </div>

                <section className="mt-4 pt-3 border-t border-surface-variant/25" data-review-records="">
                  <h3 className="text-[11.5px] font-medium text-on-surface">引用记录</h3>
                  {detail.record_ids.length === 0 ? (
                    <p className="mt-1.5 text-[11px] text-outline">本周期没有引用记录。</p>
                  ) : (
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      {detail.record_ids.map((id) => {
                        const missing = missingRecords.has(id);
                        return (
                          <button
                            key={id}
                            type="button"
                            onClick={() => onOpenRecord(id)}
                            data-review-record={id}
                            className={
                              "wr-ref-chip" + (missing ? " is-missing" : "")
                            }
                          >
                            <span className="mono shrink-0 text-outline">{id}</span>
                            <span className="min-w-0 truncate">
                              {recordTitles[id] ?? (missing ? "已删除" : "读取中…")}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </section>

                <section className="mt-4 pt-3 border-t border-surface-variant/25" data-review-tasks="">
                  <h3 className="text-[11.5px] font-medium text-on-surface">关联任务</h3>
                  {detail.task_ids.length === 0 ? (
                    <p className="mt-1.5 text-[11px] text-outline">本周期没有关联任务。</p>
                  ) : (
                    <ul className="mt-2 flex flex-col gap-1">
                      {detail.task_ids.map((id) => {
                        const task = tasks[id];
                        const missing = missingTasks.has(id);
                        return (
                          <li
                            key={id}
                            data-review-task={id}
                            className="wr-linked-task"
                          >
                            <span className="mt-[1px] h-1.5 w-1.5 shrink-0 rounded-full bg-primary/50" />
                            <span className="mono shrink-0 text-[10px] text-outline">{id}</span>
                            <span className="min-w-0 flex-1 truncate text-on-surface">
                              {task?.title ?? (missing ? "已删除" : "读取中…")}
                            </span>
                            <span className="shrink-0 text-[10px] text-outline">
                              {task?.state ?? ""}
                            </span>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </section>
              </div>
            </div>
          )}
          {notice && !detail?.exists ? (
            <p
              className={
                "mt-2 text-[11px] " +
                (notice.kind === "err" ? "text-error" : "text-primary")
              }
            >
              {notice.text}
            </p>
          ) : null}
        </section>
      </div>
    </div>
  );
}
