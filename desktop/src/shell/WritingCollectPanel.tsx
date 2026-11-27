import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type { WritingCollectItem, WritingCollectResponse } from "../types";

type Notice = { kind: "ok" | "err"; text: string };

export type WritingCollectPanelProps = {
  writingId: string;
  /** 当前任务是否已经走到可生成大纲的阶段。 */
  canNext?: boolean;
  nextHint?: string;
  onSelected?: () => void;
  onNext?: () => void;
  onOpenMaterials?: () => void;
};

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

function itemAction(item: WritingCollectItem): { kind: "material" | "knowledge"; id: string } | null {
  if (item.kind === "material" && item.material_id) return { kind: "material", id: item.material_id };
  if (item.kind === "knowledge" && item.knowledge_id) return { kind: "knowledge", id: item.knowledge_id };
  return null;
}

/**
 * 写作素材收集（OD-WT-605 的界面接入）：列出宿主推荐候选，用户采纳/移出。
 * 只通过 writing_select_materials 写入可信素材包，不在这里编辑素材正文。
 */
export function WritingCollectPanel({
  writingId,
  canNext = false,
  nextHint,
  onSelected,
  onNext,
  onOpenMaterials,
}: WritingCollectPanelProps) {
  const [data, setData] = useState<WritingCollectResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    if (!writingId) {
      setData(null);
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    setError(null);
    shell
      .writingCollectMaterials({ writing_id: writingId })
      .then((result) => {
        if (!alive) return;
        setData(result);
      })
      .catch((value) => {
        if (alive) setError(errText(value));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [writingId, reloadTick]);

  const selectedCount = useMemo(
    () => (data ? data.selected_material_ids.length + data.selected_knowledge_ids.length : 0),
    [data],
  );

  async function toggle(item: WritingCollectItem) {
    const action = itemAction(item);
    if (!action || busy) return;
    setBusy(action.id);
    setNotice(null);
    try {
      const input = item.selected
        ? action.kind === "material"
          ? { exclude_material_ids: [action.id] }
          : { exclude_knowledge_ids: [action.id] }
        : action.kind === "material"
          ? { adopt_material_ids: [action.id] }
          : { adopt_knowledge_ids: [action.id] };
      const result = await shell.writingSelectMaterials({
        writing_id: writingId,
        ...input,
      });
      setNotice({ kind: "ok", text: result.summary });
      setReloadTick((value) => value + 1);
      onSelected?.();
    } catch (value) {
      setNotice({ kind: "err", text: errText(value) });
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="flex min-w-0 flex-col gap-3" data-writing-collect="">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-2">
        <h2 className="text-[15px] font-semibold tracking-tight text-[color:var(--od-neu-ink)]">素材收集</h2>
        <span className="text-[10px] text-[color:var(--od-neu-ink-3)]">
          已选 {data?.selected_material_ids.length ?? 0} 素材 · {data?.selected_knowledge_ids.length ?? 0} 知识
        </span>
        <button
          type="button"
          className="wr-text-button ml-auto"
          onClick={() => setReloadTick((value) => value + 1)}
          disabled={loading || Boolean(busy)}
        >
          重新检索
        </button>
      </div>

      {error ? (
        <p className="rounded-xl bg-error-container/50 px-3 py-2 text-[10.5px] text-on-error-container">
          读取素材候选失败：{error}
        </p>
      ) : null}
      {notice ? (
        <p className={"rounded-xl px-3 py-2 text-[10.5px] " + (notice.kind === "err" ? "bg-error-container/50 text-on-error-container" : "bg-[color:var(--od-neu-sunken)] text-[color:var(--od-neu-ink-2)]")}>
          {notice.text}
        </p>
      ) : null}
      {data?.knowledge_note ? (
        <p className="text-[10px] text-[color:var(--od-neu-ink-3)]">{data.knowledge_note}</p>
      ) : null}

      {loading ? (
        <p className="rounded-xl bg-[color:var(--od-neu-sunken)] px-3 py-2 text-[10.5px] text-[color:var(--od-neu-ink-3)]">
          正在检索素材…
        </p>
      ) : !data || data.items.length === 0 ? (
        <div className="rounded-xl bg-[color:var(--od-neu-sunken)] px-4 py-6 text-center text-[11px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
          还没有推荐候选。可以先在素材库补充或确认素材，再回来重新检索。
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          {data.items.map((item) => {
            const action = itemAction(item);
            const id = action?.id ?? item.title;
            return (
              <article
                key={item.kind + ":" + id}
                className="rounded-xl border border-[color:var(--od-neu-line)] bg-[color:var(--od-neu-surface)] px-3 py-2.5 shadow-[var(--od-neu-raise-1)]"
                data-writing-collect-item={id}
                data-writing-collect-selected={item.selected ? "true" : "false"}
              >
                <div className="flex items-center gap-1.5">
                  <span className="rounded-full bg-[color:var(--od-neu-accent-soft)] px-1.5 py-[1px] text-[9px] text-[color:var(--od-neu-accent-ink)]">
                    {item.kind === "material" ? "素材" : "知识"}
                  </span>
                  <span className="text-[9px] text-[color:var(--od-neu-ink-3)]">{item.class_label}</span>
                  <span className="ml-auto text-[8.5px] text-[color:var(--od-neu-ink-3)]">{item.review_status}</span>
                </div>
                <div className="mt-1.5 text-[11.5px] font-medium leading-snug text-[color:var(--od-neu-ink)] break-words">
                  {item.title}
                </div>
                {item.content_summary ? (
                  <p className="mt-1 line-clamp-3 text-[10px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                    {item.content_summary}
                  </p>
                ) : null}
                {item.reason_text ? (
                  <p className="mt-1 text-[9.5px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                    {item.reason_text}
                  </p>
                ) : null}
                {item.conflict_hint ? (
                  <p className="mt-1 text-[9.5px] leading-relaxed text-amber-700">冲突：{item.conflict_hint}</p>
                ) : null}
                <div className="mt-2 flex flex-wrap items-center gap-2 text-[9px] text-[color:var(--od-neu-ink-3)]">
                  <span>{item.event_date ?? "未注日期"}</span>
                  {item.suitable_for_direct_quote ? null : <span>仅参考，不宜直接引用</span>}
                  {item.selected ? (
                    <span className="rounded-full bg-[color:var(--od-neu-accent-soft)] px-1.5 py-[1px] text-[8.5px] text-[color:var(--od-neu-accent-ink)]">已采纳</span>
                  ) : null}
                  <button
                    type="button"
                    className="wr-text-button ml-auto"
                    disabled={!action || Boolean(busy)}
                    onClick={() => void toggle(item)}
                  >
                    {busy === id ? "处理中…" : item.selected ? "移出素材包" : "采纳"}
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      )}

      <div className="mt-1 flex flex-wrap items-center gap-2 border-t border-[color:var(--od-neu-line)] pt-3">
        <button
          type="button"
          className="wr-primary-button"
          disabled={selectedCount === 0 || !canNext || !onNext}
          onClick={() => onNext?.()}
        >
          下一步：生成大纲
        </button>
        {onOpenMaterials ? (
          <button type="button" className="wr-ghost-button" onClick={onOpenMaterials}>
            打开素材库
          </button>
        ) : null}
        <span className="text-[9.5px] text-[color:var(--od-neu-ink-3)]">
          {selectedCount === 0 ? "先采纳至少一条素材或知识。" : canNext ? "素材包已就绪。" : nextHint ?? "先确认需求。"}
        </span>
      </div>
    </section>
  );
}
