// 事实核验面板（OD-WT-612）。
//
// 数据口径：结论、定位、依据、处理状态全部来自 609/603 的 types 与 api；
// 本组件只调用 writing_factcheck_run / writing_factcheck_resolve，不新增
// IPC、不调用模型、不改正文。口径冲突照 411 先例：逐对并列展示，红点标记，
// 明确不合并数值与口径。

import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  FactCheckAnnotationInput,
  FactCheckConclusion,
  FactCheckKnowledgeEvidenceInput,
  FactCheckRecord,
  FactCheckResolution,
  FactCheckRunResult,
  FactCheckScopeConflict,
  FactCheckStats,
} from "../types";

export const FACTCHECK_CONCLUSIONS: FactCheckConclusion[] = [
  "有直接来源支持",
  "有间接来源支持",
  "来源存在冲突",
  "来源已过期",
  "无法找到来源",
  "属于分析判断",
  "属于工作计划",
  "属于表达性概括",
];

const STAT_KEY: Record<FactCheckConclusion, keyof FactCheckStats> = {
  有直接来源支持: "direct_source",
  有间接来源支持: "indirect_source",
  来源存在冲突: "source_conflict",
  来源已过期: "source_outdated",
  无法找到来源: "source_missing",
  属于分析判断: "analysis_judgment",
  属于工作计划: "work_plan",
  属于表达性概括: "expressive_summary",
};

const CONCLUSION_SKIN: Record<FactCheckConclusion, string> = {
  有直接来源支持: "text-tertiary bg-tertiary/10",
  有间接来源支持: "text-primary bg-primary/10",
  来源存在冲突: "text-error bg-error-container/60",
  来源已过期: "text-amber-700 bg-amber-500/15",
  无法找到来源: "text-error bg-error-container/60",
  属于分析判断: "text-on-surface-variant bg-surface-container-high",
  属于工作计划: "text-primary bg-primary/10",
  属于表达性概括: "text-on-surface-variant bg-surface-container-high",
};

const RESOLUTIONS: Array<Exclude<FactCheckResolution, "未处理">> = [
  "已接受",
  "已修改",
  "已删除来源项",
];

const EMPTY_STATS: FactCheckStats = {
  candidates: 0,
  findings: 0,
  scope_conflicts: 0,
  annotations_used: 0,
  annotations_skipped: 0,
  direct_source: 0,
  indirect_source: 0,
  source_conflict: 0,
  source_outdated: 0,
  source_missing: 0,
  analysis_judgment: 0,
  work_plan: 0,
  expressive_summary: 0,
};

type Filter = "全部" | FactCheckConclusion;

export type WritingFactCheckPanelProps = {
  writingId: string;
  /** 界面侧最近一次生成稿的四分法标注，随核验请求传入。 */
  annotations?: FactCheckAnnotationInput[];
  /** 知识引用依据；缺省时只按任务已选素材核验。 */
  knowledgeEvidence?: FactCheckKnowledgeEvidenceInput[];
  /** 610 挂载时把上一次运行结果带进来，避免重复写盘。 */
  initialResult?: FactCheckRunResult | null;
  /** 定位到章节/段落；由编辑器承担跳转。 */
  onLocate?: (sectionNo: number, paragraphNo: number) => void;
  /** 运行成功后回传完整结果，供页面缓存。 */
  onResult?: (result: FactCheckRunResult) => void;
};

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

function shortTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getMonth() + 1}-${date.getDate()} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function CountChip({ label, count }: { label: FactCheckConclusion; count: number }) {
  return (
    <span
      className={"inline-flex items-center gap-1 rounded-full px-2 py-[2px] text-[10px] " + CONCLUSION_SKIN[label]}
      data-factcheck-stat={STAT_KEY[label]}
    >
      <span>{label}</span>
      <span className="mono tabular-nums">{count}</span>
    </span>
  );
}

function ConflictSide({
  side,
  onLocate,
}: {
  side: FactCheckScopeConflict["left"];
  onLocate?: (sectionNo: number, paragraphNo: number) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onLocate?.(side.section_no, side.paragraph_no)}
      className="min-w-0 flex-1 rounded-lg border border-error/20 bg-white/85 px-2.5 py-2 text-left hover:border-error/45"
      data-factcheck-conflict-side={`${side.section_no}:${side.paragraph_no}`}
    >
      <span className="flex items-baseline gap-1.5">
        <span className="mono text-[10.5px] text-error">{side.value}</span>
        <span className="ml-auto mono text-[9.5px] text-outline">
          {side.section_no}/{side.paragraph_no}
        </span>
      </span>
      <span className="mt-1 block text-[10.5px] text-outline">
        {side.time_scope ?? "未注时间"} · {side.stat_scope ?? "未注口径"}
      </span>
      <span className="mt-1 block text-[11px] leading-relaxed text-on-surface-variant line-clamp-2">
        {side.fact_text}
      </span>
    </button>
  );
}

export function WritingFactCheckPanel({
  writingId,
  annotations = [],
  knowledgeEvidence = [],
  initialResult = null,
  onLocate,
  onResult,
}: WritingFactCheckPanelProps) {
  const [records, setRecords] = useState<FactCheckRecord[]>(initialResult?.records ?? []);
  const [conflicts, setConflicts] = useState<FactCheckScopeConflict[]>(
    initialResult?.conflicts ?? [],
  );
  const [stats, setStats] = useState<FactCheckStats>(initialResult?.stats ?? EMPTY_STATS);
  const [filter, setFilter] = useState<Filter>("全部");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>(initialResult?.warnings ?? []);
  const [maskedNote, setMaskedNote] = useState<string | null>(null);

  useEffect(() => {
    setFilter("全部");
    if (!initialResult) {
      setRecords([]);
      setConflicts([]);
      setStats(EMPTY_STATS);
      setWarnings([]);
      setMaskedNote(null);
      return;
    }
    setRecords(initialResult.records);
    setConflicts(initialResult.conflicts);
    setStats(initialResult.stats);
    setWarnings(initialResult.warnings);
    setMaskedNote(
      initialResult.masked
        ? `已遮蔽 ${initialResult.masked_id_card + initialResult.masked_phone} 项敏感信息。`
        : null,
    );
  }, [writingId, initialResult]);

  const pending = useMemo(
    () => records.filter((record) => record.resolution === "未处理").length,
    [records],
  );
  const allProcessed = records.length > 0 && pending === 0;
  const visible = useMemo(
    () => (filter === "全部" ? records : records.filter((record) => record.conclusion === filter)),
    [records, filter],
  );

  const run = async () => {
    if (!writingId || busy) return;
    setBusy("run");
    setError(null);
    setMessage(null);
    setWarnings([]);
    try {
      const result = await shell.writingFactcheckRun({
        writing_id: writingId,
        annotations,
        knowledge_evidence: knowledgeEvidence,
      });
      setRecords(result.records);
      setConflicts(result.conflicts);
      setStats(result.stats);
      setWarnings(result.warnings);
      setMaskedNote(
        result.masked
          ? `已遮蔽 ${result.masked_id_card + result.masked_phone} 项敏感信息。`
          : null,
      );
      setMessage(
        result.appended > 0
          ? `新增 ${result.appended} 条核验结论`
          : result.skipped_existing > 0
            ? `没有新增结论；${result.skipped_existing} 条已有记录保持不变`
            : "核验完成",
      );
      onResult?.(result);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
    }
  };

  const resolve = async (
    record: FactCheckRecord,
    resolution: Exclude<FactCheckResolution, "未处理">,
  ) => {
    if (busy || record.resolution === resolution) return;
    setBusy(`${record.record_id}:${resolution}`);
    setError(null);
    try {
      const result = await shell.writingFactcheckResolve({
        writing_id: writingId,
        record_id: record.record_id,
        resolution,
        note: null,
      });
      setRecords((current) =>
        current.map((item) => (item.record_id === record.record_id ? result.record : item)),
      );
      setMessage(`已标记「${resolution}」`);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section
      aria-label="事实核验"
      className="flex min-w-0 flex-col gap-3"
      data-testid="writing-factcheck-panel"
      data-factcheck-writing-id={writingId}
    >
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h2 className="text-[15px] font-semibold tracking-tight text-on-surface">事实核验</h2>
        <span className="mono text-[10px] text-outline">
          {records.length} 条 · 待处理 {pending}
        </span>
        <button
          type="button"
          onClick={() => void run()}
          disabled={!writingId || Boolean(busy)}
          className="ml-auto rounded-lg bg-primary px-3.5 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
          data-testid="factcheck-run"
        >
          {busy === "run" ? "核验中…" : "运行核验"}
        </button>
      </div>

      {error ? (
        <p className="rounded-xl bg-error-container/60 px-3 py-2 text-[11px] text-on-error-container" role="alert">
          {error}
        </p>
      ) : null}
      {message ? (
        <p className="rounded-xl bg-surface-container-low px-3 py-2 text-[11px] text-on-surface-variant" role="status">
          {message}
        </p>
      ) : null}
      {maskedNote ? <p className="text-[10.5px] text-outline">{maskedNote}</p> : null}
      {warnings.length > 0 ? (
        <div className="rounded-xl bg-amber-50 px-3 py-2 text-[10.5px] leading-relaxed text-amber-900" data-factcheck-warnings>
          {warnings.slice(0, 5).map((warning) => (
            <p key={warning}>{warning}</p>
          ))}
        </div>
      ) : null}

      <div className="rounded-xl bg-surface-container-low/55 px-3 py-2.5" data-factcheck-stats>
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-[10.5px] text-outline">
          <span>候选 <span className="mono text-on-surface">{stats.candidates}</span></span>
          <span>结论 <span className="mono text-on-surface">{stats.findings}</span></span>
          <span>冲突对 <span className="mono text-error">{stats.scope_conflicts}</span></span>
          <span>标注 <span className="mono text-on-surface">{stats.annotations_used}</span></span>
        </div>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {FACTCHECK_CONCLUSIONS.map((conclusion) => (
            <CountChip key={conclusion} label={conclusion} count={stats[STAT_KEY[conclusion]]} />
          ))}
        </div>
      </div>

      {allProcessed ? (
        <p className="rounded-xl bg-tertiary/10 px-3 py-2 text-[11px] text-tertiary" data-factcheck-empty="all-processed">
          核验结论已全部处理。
        </p>
      ) : null}

      {conflicts.length > 0 ? (
        <section aria-label="口径冲突" className="rounded-xl bg-error-container/30 px-3 py-2.5" data-factcheck-conflicts>
          <div className="flex flex-wrap items-center gap-2">
            <span className="h-2 w-2 rounded-full bg-error" aria-hidden data-factcheck-conflict-dot />
            <h3 className="text-[12px] font-semibold text-on-error-container">口径冲突 · {conflicts.length} 对</h3>
            <span className="ml-auto text-[10px] text-on-error-container/80">两侧并存，不合并口径</span>
          </div>
          <div className="mt-2 flex flex-col gap-2">
            {conflicts.map((conflict, index) => (
              <div
                key={`${conflict.metric}-${index}`}
                className="rounded-lg bg-white/70 px-2.5 py-2"
                data-factcheck-conflict={conflict.metric}
              >
                <p className="text-[10.5px] text-on-error-container">
                  {conflict.metric} · {conflict.reason}
                </p>
                <div className="mt-1.5 flex flex-col gap-1.5 sm:flex-row">
                  <ConflictSide side={conflict.left} onLocate={onLocate} />
                  <ConflictSide side={conflict.right} onLocate={onLocate} />
                </div>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <div className="flex gap-1 overflow-x-auto border-b border-outline-variant/45 pb-0.5" role="tablist" aria-label="核验结论筛选">
        {(["全部", ...FACTCHECK_CONCLUSIONS] as Filter[]).map((key) => {
          const count = key === "全部" ? records.length : records.filter((record) => record.conclusion === key).length;
          return (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={filter === key}
              onClick={() => setFilter(key)}
              className={
                "shrink-0 border-b-2 px-2.5 py-1.5 text-[11px] " +
                (filter === key
                  ? "border-primary font-medium text-primary"
                  : "border-transparent text-on-surface-variant hover:text-on-surface")
              }
              data-factcheck-tab={key}
            >
              {key}
              <span className="ml-1 mono text-[9.5px] text-outline">{count}</span>
            </button>
          );
        })}
      </div>

      {records.length === 0 ? (
        <div className="rounded-[18px] border border-white/80 bg-white/70 px-5 py-9 text-center text-[12px] text-outline" data-factcheck-empty="no-records">
          还没有核验记录。点「运行核验」开始。
        </div>
      ) : visible.length === 0 ? (
        <div className="rounded-[18px] border border-white/80 bg-white/70 px-5 py-9 text-center text-[12px] text-outline" data-factcheck-empty="no-filter-match">
          该结论下暂无记录。
        </div>
      ) : (
        <div className="flex flex-col gap-2" data-factcheck-list>
          {visible.map((record) => (
            <article
              key={record.record_id}
              className="rounded-xl border border-surface-variant/25 bg-surface-container-low/35 px-3 py-2.5"
              data-factcheck-record={record.record_id}
              data-factcheck-conclusion={record.conclusion}
              data-factcheck-resolution={record.resolution}
            >
              <div className="flex flex-wrap items-center gap-1.5">
                <span className={"rounded-full px-1.5 py-[1px] text-[9.5px] font-medium " + CONCLUSION_SKIN[record.conclusion]}>
                  {record.conclusion}
                </span>
                <span className="rounded-full bg-outline-variant/25 px-1.5 py-[1px] text-[9.5px] text-outline">
                  {record.check_object}
                </span>
                <button
                  type="button"
                  onClick={() => onLocate?.(record.section_no, record.paragraph_no)}
                  className="mono text-[10px] text-primary hover:underline"
                  data-factcheck-locate={`${record.section_no}:${record.paragraph_no}`}
                >
                  第 {record.section_no} 章 · 第 {record.paragraph_no} 段
                </button>
                <span className="ml-auto text-[9.5px] text-outline">{shortTime(record.checked_at)}</span>
                <span
                  className={
                    "rounded-full px-1.5 py-[1px] text-[9.5px] " +
                    (record.resolution === "未处理"
                      ? "bg-amber-500/15 text-amber-700"
                      : "bg-tertiary/10 text-tertiary")
                  }
                  data-factcheck-resolution-label={record.resolution}
                >
                  {record.resolution}
                </span>
              </div>

              <p
                className="mt-1.5 text-[11.5px] leading-relaxed text-on-surface break-words"
                data-factcheck-fact-text
              >
                {record.fact_text?.trim() || "这一条没有可展示的原文片段。"}
              </p>

              {record.basis.length > 0 ? (
                <div className="mt-1.5 flex flex-col gap-1" data-factcheck-basis>
                  {record.basis.slice(0, 3).map((basis, index) => (
                    <p key={`${basis.source_kind}-${basis.source_id}-${index}`} className="text-[10px] leading-relaxed text-outline break-words">
                      <span className="text-primary">{basis.source_kind === "material" ? "素材" : "知识"}</span>
                      <span className="mono"> {basis.source_id}</span>
                      <span> · {basis.quote}</span>
                    </p>
                  ))}
                </div>
              ) : null}

              <div className="mt-2 flex flex-wrap items-center gap-1.5" data-factcheck-actions>
                {RESOLUTIONS.map((resolution) => (
                  <button
                    key={resolution}
                    type="button"
                    onClick={() => void resolve(record, resolution)}
                    disabled={Boolean(busy) || record.resolution === resolution}
                    className="rounded-lg border border-outline-variant/55 bg-white/75 px-2.5 py-1 text-[10.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-40"
                    data-factcheck-resolve={resolution}
                  >
                    {busy === `${record.record_id}:${resolution}` ? "处理中…" : resolution}
                  </button>
                ))}
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

