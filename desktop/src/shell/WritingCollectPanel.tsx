import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  WritingCollectItem,
  WritingCollectResponse,
  WritingMaterialType,
  WritingWebExtractOutcome,
  WritingWebSearchItem,
} from "../types";

type Notice = { kind: "ok" | "err"; text: string };

export type WritingCollectPanelProps = {
  writingId: string;
  /** 当前任务是否已经走到可生成大纲的阶段。 */
  canNext?: boolean;
  nextHint?: string;
  /** 主题词，供联网检索预填；来自任务主题与标题。 */
  searchTopic?: string;
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

/** 公开来源多是政策原文；认不出时按工作事实落盘，用户可在素材库改类型。 */
function guessMaterialType(text: string): WritingMaterialType {
  if (/(规定|办法|条例|意见|通知|决定|gov\.cn|12371|人民网|新华网)/.test(text)) return "制度依据";
  if (/(数据|统计|指标)/.test(text)) return "数据素材";
  if (/(案例|典型)/.test(text)) return "典型案例";
  return "工作事实";
}

/**
 * 写作素材收集（OD-WT-605 的界面接入）：列出宿主推荐候选，用户采纳/移出。
 * 只通过 writing_select_materials 写入可信素材包，不在这里编辑素材正文。
 */
export function WritingCollectPanel({
  writingId,
  canNext = false,
  nextHint,
  searchTopic = "",
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

  // ---- 联网检索（anysearch）：检索 → 勾选 → 存为素材（待审核）----------
  // 换稿件或重新打开时用当前主题重置检索词，不覆盖用户已输入的查询。
  const [searchQuery, setSearchQuery] = useState(searchTopic);
  const [topicSeen, setTopicSeen] = useState(searchTopic);
  if (searchTopic !== topicSeen) {
    setTopicSeen(searchTopic);
    setSearchQuery(searchTopic);
  }
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchItems, setSearchItems] = useState<WritingWebSearchItem[]>([]);
  const [searchNote, setSearchNote] = useState<string | null>(null);
  const [extracts, setExtracts] = useState<Record<string, WritingWebExtractOutcome>>({});
  const [fetchingUrl, setFetchingUrl] = useState<string | null>(null);
  const [savingUrl, setSavingUrl] = useState<string | null>(null);

  async function runSearch() {
    const query = searchQuery.trim();
    if (!query || searching) return;
    setSearching(true);
    setSearchError(null);
    setSearchItems([]);
    setSearchNote(null);
    setExtracts({});
    try {
      const outcome = await shell.writingWebSearch({ query, max_results: 10 });
      setSearchItems(outcome.items);
      setSearchNote(outcome.note);
    } catch (value) {
      setSearchError(errText(value));
    } finally {
      setSearching(false);
    }
  }

  async function readFullText(item: WritingWebSearchItem) {
    if (!item.url || extracts[item.url] || fetchingUrl) return;
    setFetchingUrl(item.url);
    setSearchError(null);
    try {
      const outcome = await shell.writingWebExtract(item.url);
      setExtracts((prev) => ({ ...prev, [item.url]: outcome }));
    } catch (value) {
      setSearchError(errText(value));
    } finally {
      setFetchingUrl(null);
    }
  }

  /** 用户明确选择的外部结果：一次动作内完成落盘、确认与采纳。 */
  async function saveAsMaterial(item: WritingWebSearchItem) {
    const key = item.url || item.title;
    if (savingUrl) return;
    setSavingUrl(key);
    setNotice(null);
    try {
      let extracted = item.url ? extracts[item.url] : undefined;
      if (!extracted && item.url) {
        extracted = await shell.writingWebExtract(item.url);
        setExtracts((prev) => ({ ...prev, [item.url]: extracted! }));
      }
      const body = extracted?.text ?? item.snippet ?? item.title;
      const created = await shell.materialCreate({
        title: (extracted?.title || item.title).slice(0, 200),
        content: body,
        material_type: guessMaterialType(item.title + " " + item.url),
        source_type: "user",
        source_quote: (item.snippet || body).slice(0, 2000),
        source_location: item.url || null,
        tags: ["联网检索"],
      });
      await shell.materialReview(created.material_id, "confirm", "用户从联网检索存入并采纳");
      await shell.writingSelectMaterials({
        writing_id: writingId,
        adopt_material_ids: [created.material_id],
      });
      setNotice({
        kind: "ok",
        text: "已存入素材库并采纳进本次素材包" + (extracted ? "（全文 " + extracted.chars + " 字）" : "（只取到摘要）") + "。",
      });
      setReloadTick((value) => value + 1);
      onSelected?.();
    } catch (value) {
      setNotice({ kind: "err", text: errText(value) });
    } finally {
      setSavingUrl(null);
    }
  }

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
        <div className="ml-auto flex items-center gap-2">
          {onOpenMaterials ? (
            <button type="button" className="wr-text-button" onClick={onOpenMaterials}>
              素材库
            </button>
          ) : null}
          <button
            type="button"
            className="wr-text-button"
            onClick={() => setReloadTick((value) => value + 1)}
            disabled={loading || Boolean(busy)}
          >
            重新检索
          </button>
        </div>
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

      {/* 联网检索：检索词预填主题；选中后先读全文，再一次完成落盘、确认与采纳。 */}
      <section
        className="rounded-xl border border-[color:var(--od-neu-line)] bg-[color:var(--od-neu-surface)] px-3 py-3"
        data-writing-websearch=""
      >
        <div className="flex items-baseline gap-2">
          <h3 className="text-[12.5px] font-semibold text-[color:var(--od-neu-ink)]">联网检索</h3>
          <span className="text-[10.5px] text-[color:var(--od-neu-ink-3)]">最多 10 条</span>
        </div>
        <p className="mt-1.5 text-[10.5px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
          存入的是本次写作素材库（work/素材篮），不是知识库。存入即确认并采纳，不需要再审批；之后可在素材库编辑或移出。
        </p>
        <div className="mt-2 flex items-start gap-2">
          <input
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void runSearch();
            }}
            className="neu-inset-field min-w-0 flex-1 rounded-lg px-3 py-2 text-[12.5px] text-[color:var(--od-neu-ink)] outline-none"
            placeholder="检索公开资料、规范原文…"
            aria-label="联网检索词"
            data-writing-websearch-input=""
          />
          <button
            type="button"
            className="wr-primary-button shrink-0"
            onClick={() => void runSearch()}
            disabled={searching || !searchQuery.trim()}
            data-writing-websearch-run=""
          >
            {searching ? "检索中…" : "搜索"}
          </button>
        </div>
        {searchError ? (
          <p className="mt-2 text-[11.5px] leading-relaxed text-error">{searchError}</p>
        ) : null}
        {searchNote ? (
          <p className="mt-2 text-[11px] leading-relaxed text-[color:var(--od-neu-ink-3)]">{searchNote}</p>
        ) : null}
        {searchItems.length > 0 ? (
          <ul className="mt-2 flex max-h-[360px] flex-col gap-2 overflow-y-auto pr-0.5">
            {searchItems.map((item) => {
              const key = item.url || item.title;
              const extracted = item.url ? extracts[item.url] : undefined;
              return (
                <li
                  key={key}
                  className="rounded-lg border border-[color:var(--od-neu-line)] bg-[color:var(--od-neu-surface-high)] px-3 py-2"
                  data-writing-websearch-item={key}
                >
                  <div className="text-[12px] font-medium leading-snug text-[color:var(--od-neu-ink)] break-words">
                    {item.title}
                  </div>
                  {item.snippet ? (
                    <p className="mt-0.5 text-[11px] leading-relaxed text-[color:var(--od-neu-ink-3)] line-clamp-2">
                      {item.snippet}
                    </p>
                  ) : null}
                  {item.url ? (
                    <p className="mt-0.5 truncate text-[10px] text-[color:var(--od-neu-ink-3)]">
                      {item.url}
                      {item.date ? " · " + item.date : ""}
                    </p>
                  ) : null}
                  {extracted ? (
                    <details className="mt-1.5">
                      <summary className="cursor-pointer text-[10.5px] text-[color:var(--od-neu-accent-ink)]">
                        已读取全文（{extracted.chars} 字）
                      </summary>
                      <pre className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-[10.5px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                        {extracted.text.slice(0, 1200)}
                      </pre>
                    </details>
                  ) : null}
                  <div className="mt-1.5 flex flex-wrap items-center gap-2">
                    {item.url ? (
                      <button
                        type="button"
                        className="wr-text-button"
                        onClick={() => void readFullText(item)}
                        disabled={fetchingUrl === item.url || Boolean(extracted)}
                        data-writing-websearch-read={key}
                      >
                        {fetchingUrl === item.url ? "读取中…" : extracted ? "已读取全文" : "读取全文"}
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="wr-primary-button"
                      onClick={() => void saveAsMaterial(item)}
                      disabled={savingUrl === key}
                      data-writing-websearch-save={key}
                    >
                      {savingUrl === key ? "保存中…" : "存入并采纳"}
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        ) : null}
      </section>

      {loading ? (
        <p className="rounded-xl bg-[color:var(--od-neu-sunken)] px-3 py-2 text-[10.5px] text-[color:var(--od-neu-ink-3)]">
          正在检索素材…
        </p>
      ) : !data || data.items.length === 0 ? (
        <div className="rounded-xl bg-[color:var(--od-neu-sunken)] px-4 py-6 text-center text-[11px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
          还没有推荐候选。可以先联网检索，或在素材库补充素材。
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
