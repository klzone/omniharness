import { useState } from "react";

import { shell } from "../api";
import type {
  KnowledgeGovernanceRecord,
  KnowledgeQualityReport,
  KnowledgeReviewItem,
  KnowledgeVaultIndex,
  KnowledgeVaultScan,
  KnowledgeVaultSearchResponse,
  TaskRecordSummary,
} from "../types";
import { countPendingReviews, rawFiles, wikiNotes, type KnowledgeConflict } from "./knowledgeHub";

function pct(numerator: number, denominator: number): number | null {
  if (denominator <= 0) return null;
  return Math.max(0, Math.min(100, (numerator / denominator) * 100));
}

function pctText(value: number | null): string {
  return value === null ? "暂无" : `${value.toFixed(1)}%`;
}

function timeText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "暂无";
  return new Date(seconds * 1000).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function stateLabel(state: string): string {
  if (state === "succeeded") return "已完成";
  if (state === "failed") return "失败";
  if (state === "cancelled") return "已取消";
  if (state === "unknown") return "状态未知";
  return state || "未知";
}

export function KnowledgeDashboard({
  scan,
  index,
  reviewItems,
  conflicts,
  taskRecords,
  rawAdmissions,
  quality,
  qualityError,
  loadingError,
  onOpenReview,
  onOpenAssets,
  onOpenAsset,
  onOpenGraph,
  onRebuildIndex,
  onRefresh,
  onRefreshQuality,
}: {
  scan: KnowledgeVaultScan | null;
  index: KnowledgeVaultIndex | null;
  reviewItems: KnowledgeReviewItem[];
  conflicts: KnowledgeConflict[];
  taskRecords: TaskRecordSummary[];
  rawAdmissions: KnowledgeGovernanceRecord[];
  quality: KnowledgeQualityReport | null;
  qualityError: string | null;
  loadingError: string | null;
  onOpenReview: (queue: "new" | "conflict") => void;
  onOpenAssets: () => void;
  onOpenAsset: (relativePath: string) => void;
  onOpenGraph: () => void;
  onRebuildIndex: () => void;
  onRefresh: () => void;
  onRefreshQuality: () => void;
}) {
  const [showTasks, setShowTasks] = useState(false);
  const [showExpired, setShowExpired] = useState(false);
  const [query, setQuery] = useState("");
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchResult, setSearchResult] = useState<KnowledgeVaultSearchResponse | null>(null);
  const raw = rawFiles(scan);
  const wiki = wikiNotes(scan);
  const pending = countPendingReviews(reviewItems);
  const failures = taskRecords.filter(
    (record) => record.state === "failed" || record.state === "unknown",
  );
  const expired = quality?.expired ?? [];
  const risks = failures.length + expired.length;
  const admitted = new Set(
    rawAdmissions
      .filter((record) => record.action === "admit" && record.subject.startsWith("raw/"))
      .map((record) => record.subject),
  );
  const traceableRaw = raw.filter((file) => admitted.has(file.relative_path)).length;
  const sourceRate = pct(traceableRaw, raw.length);
  const associationRate = index
    ? pct(Math.max(0, index.stats.notes - index.stats.orphans), index.stats.notes)
    : null;
  const indexComplete = index ? (index.complete ? 100 : 0) : null;
  const healthTotal = taskRecords.length + expired.length;
  const taskHealth = healthTotal > 0 ? pct(healthTotal - risks, healthTotal) : null;
  const recent: Array<{ key: string; title: string; detail: string; at: number }> = [
    ...reviewItems.slice(0, 4).map((item) => ({
      key: `review-${item.id}`,
      title:
        item.status === "pending"
          ? `整理等待审核 · ${item.card_count} 张卡`
          : `整理审核 · ${stateLabel(item.status)}`,
      detail: item.cards[0]?.title ?? item.id,
      at: item.created_at,
    })),
    ...taskRecords.slice(0, 4).map((record) => ({
      key: `task-${record.omni_task_id}`,
      title: `任务 ${stateLabel(record.state)}`,
      detail: record.omni_task_id,
      at: record.created_at,
    })),
  ]
    .sort((a, b) => b.at - a.at)
    .slice(0, 4);

  const runSearch = async () => {
    const keyword = query.trim();
    if (!keyword || searchBusy) return;
    setSearchBusy(true);
    setSearchError(null);
    try {
      const result = await shell.knowledgeVaultSearch({
        keyword,
        tags: [],
        properties: [],
      });
      setSearchResult(result);
    } catch (error) {
      setSearchError(error instanceof Error ? error.message : String(error));
    } finally {
      setSearchBusy(false);
    }
  };

  return (
    <section className="flex-1 min-h-0 overflow-y-auto" data-testid="knowledge-dashboard">
      <div className="flex items-baseline gap-3 mb-3.5">
        <h2 className="text-[18px] font-semibold tracking-tight text-on-surface">知识驾驶舱</h2>
        <span className="mono text-[10px] text-outline">AI Knowledge Operations</span>
        <button
          type="button"
          className="ml-auto rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary"
          onClick={onRefresh}
        >
          刷新数据
        </button>
      </div>

      {loadingError ? (
        <p className="mb-2 rounded-lg bg-error-container/60 px-3 py-2 text-[11px] text-on-error-container" role="alert">
          {loadingError}
        </p>
      ) : null}

      <section className="mb-3.5 overflow-hidden rounded-[22px] border border-white/85 bg-white/72 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]">
        <div className="grid grid-cols-2 divide-y divide-outline-variant/20 lg:grid-cols-5 lg:divide-x lg:divide-y-0">
        {[
          { id: "dashboard-stat-raw", value: raw.length, label: "原始资料", hint: "raw/", action: onOpenAssets },
          { id: "dashboard-stat-wiki", value: wiki.length, label: "Wiki 知识条目", hint: "知识库/", action: onOpenAssets },
          { id: "dashboard-stat-review", value: pending, label: "待审核", hint: "需人工裁决", action: () => onOpenReview("new") },
          { id: "dashboard-stat-conflict", value: conflicts.length, label: "待处理冲突", hint: "来源或链接异常", action: () => onOpenReview("conflict") },
          { id: "dashboard-stat-task-fail", value: failures.length, label: "AI 任务失败", hint: "任务台账", action: () => setShowTasks(true) },
        ].map((stat) => (
          <button
            key={stat.id}
            type="button"
            data-testid={stat.id}
            onClick={stat.action}
            className="group min-w-0 px-4 py-3.5 text-left transition-colors hover:bg-primary/[0.035]"
          >
            <span className="block mono text-[25px] font-semibold tracking-[-0.03em] text-on-surface group-hover:text-primary">{stat.value}</span>
            <span className="mt-0.5 block text-[11.5px] text-on-surface-variant">{stat.label}</span>
            <span className="mt-1 block mono text-[9.5px] text-outline">{stat.hint}</span>
          </button>
        ))}
        </div>
      </section>

      <div className="grid grid-cols-1 gap-3.5 mb-3.5 lg:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]" data-testid="dashboard-duo">
        <section className="rounded-[20px] border border-white/80 bg-white/70 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]" data-testid="dashboard-todo-card">
          <div className="flex items-center gap-2 px-4 py-3">
            <h3 className="text-[12.5px] font-semibold text-on-surface">需要你处理</h3>
            <button type="button" className="ml-auto mono text-[10px] text-outline hover:text-primary" onClick={() => onOpenReview("new")}>
              全部 →
            </button>
          </div>
          {pending === 0 && conflicts.length === 0 && failures.length === 0 && expired.length === 0 ? (
            <p className="px-4 pb-4 text-[11.5px] text-outline">暂无需要处理的事项。</p>
          ) : (
            <div className="divide-y divide-outline-variant/20">
              {pending > 0 ? (
                <button type="button" onClick={() => onOpenReview("new")} className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]">
                  <span className="h-1.5 w-1.5 rounded-full bg-amber-500" />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[12px] font-medium text-on-surface">知识审核 · {pending} 项待确认</span>
                    <span className="block text-[10.5px] text-outline">AI 提取结果等待人工裁决</span>
                  </span>
                  <span className="mono text-[10px] text-outline">›</span>
                </button>
              ) : null}
              {conflicts.length > 0 ? (
                <button type="button" onClick={() => onOpenReview("conflict")} className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]">
                  <span className="h-1.5 w-1.5 rounded-full bg-error" />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[12px] font-medium text-on-surface">知识冲突 · {conflicts.length} 项</span>
                    <span className="block text-[10.5px] text-outline">同名条目、扫描异常或链接问题</span>
                  </span>
                  <span className="mono text-[10px] text-outline">›</span>
                </button>
              ) : null}
              {failures.length > 0 ? (
                <button type="button" onClick={() => setShowTasks((v) => !v)} className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]">
                  <span className="h-1.5 w-1.5 rounded-full bg-primary" />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[12px] font-medium text-on-surface">AI 任务失败 · {failures.length} 项</span>
                    <span className="block text-[10.5px] text-outline">任务台账中的失败或未知终态</span>
                  </span>
                  <span className="mono text-[10px] text-outline">›</span>
                </button>
              ) : null}
              {expired.length > 0 ? (
                <button type="button" onClick={() => setShowExpired(true)} className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-primary/[0.03]" data-testid="dashboard-expired-todo">
                  <span className="h-1.5 w-1.5 rounded-full bg-amber-500" />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[12px] font-medium text-on-surface">知识可能过期 · {expired.length} 项</span>
                    <span className="block text-[10.5px] text-outline">阈值 {quality?.threshold_days ?? "—"} 天 · 确定性检查</span>
                  </span>
                  <span className="mono text-[10px] text-outline">›</span>
                </button>
              ) : null}
            </div>
          )}
        </section>

        <section className="rounded-[20px] border border-white/80 bg-white/70 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]" data-testid="dashboard-ai-work-card">
          <div className="flex items-center gap-2 px-4 py-3">
            <h3 className="text-[12.5px] font-semibold text-on-surface">AI 最近工作</h3>
            <button type="button" className="ml-auto mono text-[10px] text-outline hover:text-primary" onClick={() => setShowTasks((v) => !v)}>
              运行记录 →
            </button>
          </div>
          {recent.length === 0 ? (
            <p className="px-4 pb-4 text-[11.5px] text-outline">暂无运行记录。</p>
          ) : (
            <div className="divide-y divide-outline-variant/20">
              {recent.map((item) => (
                <div key={item.key} className="flex items-center gap-3 px-4 py-2.5">
                  <span className="h-1.5 w-1.5 rounded-full bg-primary" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[11.5px] text-on-surface">{item.title}</span>
                    <span className="block truncate text-[10px] text-outline">{item.detail}</span>
                  </span>
                  <span className="mono text-[9.5px] text-outline">{timeText(item.at)}</span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>

      <section className="rounded-[20px] border border-white/80 bg-white/70 shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]" data-testid="dashboard-health-card">
        <div className="flex items-center gap-2 px-4 py-3">
          <h3 className="text-[12.5px] font-semibold text-on-surface">知识库健康 · AI 运行质量</h3>
          <span className="ml-auto mono text-[10px] text-outline">四格均为真实数据</span>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 px-4 pb-4">
          {[
            {
              id: "source",
              label: "来源可追溯率",
              value: pctText(sourceRate),
              sub: raw.length ? `${traceableRaw} / ${raw.length} raw 文件有准入记录` : "暂无原始资料",
              bar: sourceRate,
              action: onOpenAssets,
              actionText: "查看缺失 →",
            },
            {
              id: "association",
              label: "知识关联覆盖率",
              value: pctText(associationRate),
              sub: index ? `孤立条目 ${index.stats.orphans}` : "索引尚未构建",
              bar: associationRate,
              action: onOpenGraph,
              actionText: "查看链接索引图 →",
            },
            {
              id: "index",
              label: "索引完整性",
              value: index ? (index.complete ? "完整" : "不完整") : "暂无",
              sub: index ? `第 ${index.generation} 代 · ${index.stats.notes} 篇` : "索引尚未构建",
              bar: indexComplete,
              action: onRebuildIndex,
              actionText: "重建索引 →",
            },
            {
              id: "task",
              label: "AI 任务健康",
              value: risks > 0 ? `${risks} 项风险` : healthTotal > 0 ? "正常" : "暂无",
              sub: healthTotal > 0 ? `${expired.length} 项过期 · ${failures.length} 项失败` : "任务台账与过期检查均无记录",
              bar: taskHealth,
              action: () => {
                setShowTasks(true);
                setShowExpired(true);
              },
              actionText: "查看风险记录 →",
            },
          ].map((cell) => (
            <div key={cell.id} data-testid={`dashboard-health-${cell.id}`}>
              <p className="text-[11.5px] text-on-surface-variant">{cell.label}</p>
              <p className="mt-1 mono text-[16px] font-semibold text-on-surface">{cell.value}</p>
              <div className="mt-2 h-1 overflow-hidden rounded-full bg-outline-variant/25">
                <div
                  className="h-full rounded-full bg-primary"
                  style={{ width: `${cell.bar ?? 0}%` }}
                />
              </div>
              <p className="mt-1 text-[9.5px] text-outline">{cell.sub}</p>
              <button type="button" className="mt-1.5 mono text-[10px] text-primary hover:underline" onClick={cell.action}>
                {cell.actionText}
              </button>
            </div>
          ))}
        </div>
      </section>

      {showExpired ? (
        <section className="mt-3.5 rounded-[20px] border border-white/80 bg-white/70 p-4" data-testid="dashboard-expired-list">
          <div className="flex items-center gap-2">
            <h3 className="text-[12.5px] font-semibold text-on-surface">可能过期 / 质量风险</h3>
            <span className="mono text-[10px] text-outline">阈值 {quality?.threshold_days ?? "—"} 天</span>
            <button type="button" className="ml-auto text-[10.5px] text-outline hover:text-primary" onClick={onRefreshQuality}>
              重新检查
            </button>
            <button type="button" className="text-[10.5px] text-outline hover:text-primary" onClick={() => setShowExpired(false)}>
              收起
            </button>
          </div>
          {qualityError ? (
            <p className="mt-2 text-[11px] text-error" role="alert">质量检查失败：{qualityError}</p>
          ) : null}
          {expired.length === 0 ? (
            <p className="mt-2 text-[11.5px] text-outline">当前阈值内没有发现可能过期的索引条目。</p>
          ) : (
            <div className="mt-2 max-h-[34vh] space-y-1 overflow-y-auto">
              {expired.slice(0, 30).map((item) => (
                <button
                  key={`${item.kind}-${item.subject}-${item.date}`}
                  type="button"
                  onClick={() => onOpenAsset(item.subject)}
                  className="flex w-full items-baseline gap-2 rounded-lg bg-surface-container-low px-2.5 py-1.5 text-left hover:ring-1 hover:ring-primary/35"
                  data-testid="dashboard-expired-item"
                >
                  <span className="flex-none rounded bg-amber-100 px-1.5 py-0.5 mono text-[9px] text-amber-800">
                    {item.kind === "raw" ? "raw" : "条目"}
                  </span>
                  <span className="min-w-0 flex-1 truncate mono text-[10px] text-on-surface-variant">{item.subject}</span>
                  <span className="mono text-[9.5px] text-outline">{item.date} · {item.age_days} 天</span>
                </button>
              ))}
              {expired.length > 30 ? (
                <p className="text-[9.5px] text-outline">另有 {expired.length - 30} 条未展开。</p>
              ) : null}
            </div>
          )}
        </section>
      ) : null}

      <section className="mt-3.5 rounded-[20px] border border-white/80 bg-white/70 p-4" data-testid="dashboard-retrieval">
        <div className="flex items-baseline gap-2">
          <h3 className="text-[12.5px] font-semibold text-on-surface">检索抽检</h3>
          <span className="mono text-[9.5px] text-outline">全文检索 · 非向量</span>
        </div>
        <div className="mt-2 flex items-center gap-2">
          <input
            className="min-w-0 flex-1 rounded-lg border border-outline-variant/50 bg-white px-2.5 py-1.5 text-[11.5px]"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void runSearch();
            }}
            placeholder="输入关键词，验证 AI 检索是否命中正确条目"
            data-testid="dashboard-quality-search"
          />
          <button
            type="button"
            className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
            disabled={searchBusy || query.trim() === ""}
            onClick={() => void runSearch()}
            data-testid="dashboard-quality-search-run"
          >
            {searchBusy ? "检索中…" : "抽检"}
          </button>
        </div>
        {searchError ? (
          <p className="mt-2 text-[11px] text-error" role="alert">{searchError}</p>
        ) : null}
        {searchResult ? (
          <div className="mt-2" data-testid="dashboard-quality-search-results">
            <p className="text-[10.5px] text-on-surface-variant">
              {searchResult.hits.length} 条命中 · 检查 {searchResult.notes_examined}/{searchResult.notes_available} 篇
            </p>
            <div className="mt-1.5 max-h-[32vh] space-y-1 overflow-y-auto">
              {searchResult.hits.slice(0, 30).map((hit) => (
                <button
                  key={hit.relative_path}
                  type="button"
                  onClick={() => onOpenAsset(hit.relative_path)}
                  className="w-full rounded-lg bg-surface-container-low px-2.5 py-1.5 text-left hover:ring-1 hover:ring-primary/35"
                  data-testid="dashboard-quality-search-hit"
                >
                  <p className="mono text-[10px] text-primary break-all">{hit.relative_path}</p>
                  <p className="text-[9.5px] text-outline">命中：{hit.match_areas.join(" / ") || "筛选条件"}</p>
                  {hit.snippet ? (
                    <p className="mt-0.5 break-words text-[10px] leading-relaxed text-on-surface-variant">
                      {hit.snippet_line ? `第 ${hit.snippet_line} 行：` : ""}{hit.snippet}
                    </p>
                  ) : null}
                </button>
              ))}
            </div>
            {searchResult.budget_exhausted ? (
              <p className="mt-1 text-[9.5px] text-amber-800">读取预算耗尽，结果不完整。</p>
            ) : null}
          </div>
        ) : null}
      </section>

      {showTasks ? (
        <section className="mt-3.5 rounded-[20px] border border-white/80 bg-white/70 p-4" data-testid="dashboard-task-records">
          <div className="flex items-center gap-2">
            <h3 className="text-[12.5px] font-semibold text-on-surface">任务记录</h3>
            <button type="button" className="ml-auto text-[10.5px] text-outline hover:text-primary" onClick={() => setShowTasks(false)}>
              收起
            </button>
          </div>
          {taskRecords.length === 0 ? (
            <p className="mt-2 text-[11.5px] text-outline">暂无运行记录。</p>
          ) : (
            <div className="mt-2 max-h-[40vh] space-y-1 overflow-y-auto">
              {taskRecords.slice(0, 12).map((record) => (
                <div key={record.omni_task_id} className="rounded-lg bg-surface-container-low px-2.5 py-1.5" data-testid="dashboard-task-row">
                  <div className="flex items-baseline gap-2">
                    <span className="mono text-[9.5px] text-outline">{timeText(record.created_at)}</span>
                    <span className="min-w-0 flex-1 truncate mono text-[10px] text-on-surface-variant">{record.omni_task_id}</span>
                    <span className="text-[10px] text-on-surface-variant">{stateLabel(record.state)}</span>
                  </div>
                  {record.error_message ? (
                    <p className="mt-0.5 break-words text-[10px] leading-relaxed text-error" data-testid="dashboard-task-error">
                      失败原因：{record.error_message}
                    </p>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </section>
      ) : null}
    </section>
  );
}
