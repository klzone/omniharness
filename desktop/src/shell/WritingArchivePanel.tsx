// 写作归档与知识回流面板（OD-WT-614）。
//
// 归档前只读展示导出清单与关联信息：未导出时给出软引导，硬门禁只校验
// 「已定稿」。执行归档必须经应用内确认弹窗；知识回流只提交用户勾选的条目，
// 进入既有 raw 收件待审队列，不自动生成知识条目，也不自动定稿。

import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  WritingArchiveOutcome,
  WritingKnowledgeSelection,
  WritingTaskStatus,
} from "../types";
import { ConfirmDialog } from "./ConfirmDialog";

export type WritingArchivePanelProps = {
  writingId: string;
  status?: WritingTaskStatus;
  /** 归档成功后供页面刷新任务卡。 */
  onArchived?: (result: WritingArchiveOutcome) => void;
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

export function WritingArchivePanel({
  writingId,
  status,
  onArchived,
}: WritingArchivePanelProps) {
  const [outcome, setOutcome] = useState<WritingArchiveOutcome | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [checked, setChecked] = useState<Record<number, boolean>>({});
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    if (!writingId) {
      setOutcome(null);
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    setError(null);
    shell
      .writingArchive({ writing_id: writingId, preview: true })
      .then((result) => {
        if (!alive) return;
        setOutcome(result);
        setChecked({});
      })
      .catch((e) => {
        if (alive) setError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [writingId, reloadToken]);

  const association = outcome?.association ?? null;
  const candidates: WritingKnowledgeSelection[] = association?.knowledge_candidates ?? [];
  const selected = useMemo(
    () => candidates.filter((_, index) => checked[index]),
    [candidates, checked],
  );
  const exportFormats = useMemo(() => {
    const formats = new Set((association?.export_receipts ?? []).map((receipt) => receipt.format));
    return Array.from(formats);
  }, [association]);
  const ready = status === "已定稿" && Boolean(outcome?.can_archive);

  const archive = async () => {
    if (!writingId || busy) return;
    setBusy("archive");
    setError(null);
    setMessage(null);
    try {
      const result = await shell.writingArchive({ writing_id: writingId, preview: false });
      setOutcome(result);
      setMessage(`已归档：${result.vault?.target_path ?? "知识库报告目录"}`);
      onArchived?.(result);
      setReloadToken((value) => value + 1);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
      setConfirmOpen(false);
    }
  };

  const submitFeedback = async () => {
    if (!writingId || busy) return;
    if (selected.length === 0) {
      setMessage("先勾选要回流的知识内容。");
      return;
    }
    setBusy("feedback");
    setError(null);
    setMessage(null);
    try {
      const result = await shell.writingKnowledgeFeedback({
        writing_id: writingId,
        selections: selected.map((item) => ({
          title: item.title,
          content: item.content,
          source_quote: item.source_quote ?? "",
        })),
      });
      const rejected = result.rejected.length > 0 ? `；${result.rejected.length} 条被拒` : "";
      setMessage(`已送入待审队列 ${result.admitted.length} 条${rejected}。${result.note}`);
      setChecked({});
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section
      aria-label="归档"
      className="flex min-w-0 flex-col gap-3"
      data-testid="writing-archive-panel"
      data-archive-writing-id={writingId}
    >
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h2 className="text-[15px] font-semibold tracking-tight text-on-surface">归档</h2>
        <span className="mono text-[10px] text-outline">
          {association ? `v${association.version}` : loading ? "读取中" : "暂无"}
        </span>
        <button
          type="button"
          onClick={() => setConfirmOpen(true)}
          disabled={!ready || Boolean(busy)}
          className="ml-auto rounded-lg border border-primary/35 px-3 py-1.5 text-[11.5px] text-primary hover:bg-primary/10 disabled:opacity-45"
          data-archive-button
          data-archive-ready={ready ? "true" : "false"}
        >
          {busy === "archive" ? "归档中…" : association?.already_archived ? "已归档" : "归档"}
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

      {association?.already_archived ? (
        <p className="rounded-xl bg-tertiary/10 px-3 py-2 text-[11px] text-tertiary break-all" data-archive-existing>
          已归档：{association.archive_path ?? "知识库报告目录"}
        </p>
      ) : outcome && !outcome.can_archive && outcome.blocked_reason ? (
        <p className="rounded-xl bg-amber-500/10 px-3 py-2 text-[11px] text-amber-700" data-archive-blocked>
          {outcome.blocked_reason}
        </p>
      ) : null}

      <section aria-label="导出清单" className="rounded-xl bg-surface-container-low/50 px-3 py-2.5" data-archive-exports>
        <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
          <h3 className="text-[12px] font-semibold text-on-surface">导出</h3>
          <span className="mono text-[9.5px] text-outline">
            {(association?.export_receipts.length ?? 0)} 份
          </span>
          {exportFormats.length > 0 ? (
            <span className="text-[10px] text-on-surface-variant">{exportFormats.join(" / ")}</span>
          ) : null}
        </div>
        {association && association.export_receipts.length > 0 ? (
          <div className="mt-1.5 flex flex-col gap-1" data-archive-export-list>
            {association.export_receipts.map((receipt, index) => (
              <div
                key={`${receipt.format}-${receipt.at}-${index}`}
                className="rounded-lg bg-white/80 px-2.5 py-1.5"
                data-archive-export-format={receipt.format}
              >
                <span className="mono text-[10px] text-on-surface">{receipt.format}</span>
                <span className="ml-1.5 text-[9.5px] text-outline">{shortTime(receipt.at)}</span>
                <p className="mt-0.5 text-[9.5px] leading-snug text-on-surface-variant break-all">
                  {receipt.path}
                </p>
              </div>
            ))}
          </div>
        ) : (
          <p className="mt-1.5 text-[10.5px] text-outline" data-archive-export-empty>
            尚未导出。建议先导出；归档仍可继续。
          </p>
        )}
      </section>

      {association ? (
        <section aria-label="归档关联" className="rounded-xl bg-surface-container-low/50 px-3 py-2.5" data-archive-association>
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[10px] text-on-surface-variant">
            <span>素材 {association.selected_material_ids.length}</span>
            <span>知识 {association.selected_knowledge_ids.length}</span>
            <span>
              核验 {association.factcheck_counts.reduce((sum, item) => sum + item.count, 0)}
            </span>
            <span className="mono text-outline">v{association.version}</span>
          </div>
          <div className="mt-1.5 flex flex-wrap gap-1" data-archive-factcheck-counts>
            {association.factcheck_counts.map((item) => (
              <span
                key={item.conclusion}
                className="rounded-full bg-white/80 px-1.5 py-[1px] text-[9.5px] text-on-surface-variant"
                data-archive-factcheck-conclusion={item.conclusion}
              >
                {item.conclusion} {item.count}
              </span>
            ))}
          </div>
        </section>
      ) : null}

      {status === "已定稿" || status === "已归档" ? (
        <section aria-label="知识回流" className="rounded-xl bg-surface-container-low/50 px-3 py-2.5" data-knowledge-feedback>
          <div className="flex flex-wrap items-baseline gap-x-2">
            <h3 className="text-[12px] font-semibold text-on-surface">知识回流</h3>
            <span className="text-[9.5px] text-outline">勾选后可复用内容</span>
            <button
              type="button"
              onClick={() => void submitFeedback()}
              disabled={selected.length === 0 || Boolean(busy)}
              className="ml-auto rounded-lg bg-primary px-3 py-1.5 text-[11px] text-on-primary disabled:opacity-45"
              data-knowledge-feedback-submit
            >
              {busy === "feedback" ? "提交中…" : `提交 ${selected.length}`}
            </button>
          </div>
          {candidates.length === 0 ? (
            <p className="mt-1.5 text-[10.5px] text-outline" data-knowledge-feedback-empty>
              还没有可回流的正文段落。
            </p>
          ) : (
            <div className="mt-1.5 flex flex-col gap-1" data-knowledge-feedback-list>
              {candidates.map((candidate, index) => (
                <label
                  key={`${candidate.title}-${index}`}
                  className="flex cursor-pointer items-start gap-2 rounded-lg bg-white/80 px-2.5 py-1.5"
                  data-knowledge-feedback-candidate={index}
                >
                  <input
                    type="checkbox"
                    className="mt-0.5"
                    checked={Boolean(checked[index])}
                    onChange={(event) =>
                      setChecked((current) => ({ ...current, [index]: event.target.checked }))
                    }
                    data-knowledge-feedback-check={index}
                  />
                  <span className="min-w-0">
                    <span className="block text-[11px] font-medium text-on-surface break-words">
                      {candidate.title}
                    </span>
                    <span className="mt-0.5 block text-[10px] leading-snug text-outline line-clamp-2 break-words">
                      {candidate.content}
                    </span>
                  </span>
                </label>
              ))}
            </div>
          )}
        </section>
      ) : null}

      <ConfirmDialog
        open={confirmOpen}
        title="确认归档"
        description="归档只把干净 Markdown 写入知识库「报告/」，任务进入「已归档」终态。DOCX / PDF / HTML 不会写入知识库。"
        confirmLabel="确认归档"
        onConfirm={() => void archive()}
        onCancel={() => setConfirmOpen(false)}
        testId="writing-archive-confirm"
      />
    </section>
  );
}
