import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  WritingCollectItem,
  WritingCollectResponse,
  WritingMaterialType,
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
  const [pickedUrls, setPickedUrls] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  async function runSearch() {
    const query = searchQuery.trim();
    if (!query || searching) return;
    setSearching(true);
    setSearchError(null);
    setSearchItems([]);
    setSearchNote(null);
    setPickedUrls([]);
    try {
      const outcome = await shell.writingWebSearch({ query, max_results: 6 });
      setSearchItems(outcome.items);
      setSearchNote(outcome.note);
    } catch (value) {
      setSearchError(errText(value));
    } finally {
      setSearching(false);
    }
  }

  async function savePickedAsMaterials() {
    if (!pickedUrls.length || saving) return;
    setSaving(true);
    setNotice(null);
    let saved = 0;
    let failed = 0;
    for (const item of searchItems) {
      if (!pickedUrls.includes(item.url)) continue;
      try {
        await shell.materialCreate({
          title: item.title.slice(0, 200),
          content: (item.snippet + (item.url ? "\n来源：" + item.url : "")).trim(),
          material_type: "工作事实" satisfies WritingMaterialType,
          source_type: "user",
          source_quote: item.snippet || item.title,
          source_location: item.url || null,
          tags: ["联网检索"],
        });
        saved += 1;
      } catch {
        failed += 1;
      }
    }
    setPickedUrls([]);
    setSaving(false);
    if (saved > 0) {
      setNotice({
        kind: "ok",
        text:
          "已存入素材篮 " + saved + " 条（待审核）。" +
          (failed > 0 ? failed + " 条失败，多为重复或触发红线。" : "到素材库确认后可采纳进素材包。"),
      });
      onSelected?.();
    } else {
      setNotice({ kind: "err", text: "保存失败，多为重复或触发红线。" });
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

      {/* 联网检索：检索词预填主题，结果勾选后存为素材篮待审核素材。 */}
      <section
        className="rounded-xl border border-[color:var(--od-neu-line)] bg-[color:var(--od-neu-surface)] px-3 py-3"
        data-writing-websearch=""
      >
        <div className="flex items-baseline gap-2">
          <h3 className="text-[12.5px] font-semibold text-[color:var(--od-neu-ink)]">联网检索</h3>
          <span className="text-[10.5px] text-[color:var(--od-neu-ink-3)]">anysearch</span>
        </div>
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
          <>
            <ul className="mt-2 flex max-h-[280px] flex-col gap-2 overflow-y-auto pr-0.5">
              {searchItems.map((item) => {
                const picked = pickedUrls.includes(item.url);
                return (
                  <li
                    key={item.url || item.title}
                    className="rounded-lg border border-[color:var(--od-neu-line)] bg-[color:var(--od-neu-surface-high)] px-3 py-2"
                    data-writing-websearch-item={item.url || item.title}
                  >
                    <label className="flex items-start gap-2">
                      <input
                        type="checkbox"
                        checked={picked}
                        onChange={() =>
                          setPickedUrls((prev) =>
                            picked ? prev.filter((url) => url !== item.url) : [...prev, item.url],
                          )
                        }
                        className="mt-0.5 shrink-0"
                        aria-label="选中这条检索结果"
                      />
                      <span className="min-w-0">
                        <span className="block text-[12px] font-medium leading-snug text-[color:var(--od-neu-ink)] break-words">
                          {item.title}
                        </span>
                        {item.snippet ? (
                          <span className="mt-0.5 block text-[11px] leading-relaxed text-[color:var(--od-neu-ink-3)] line-clamp-2">
                            {item.snippet}
                          </span>
                        ) : null}
                        {item.url ? (
                          <span className="mt-0.5 block truncate text-[10px] text-[color:var(--od-neu-ink-3)]">
                            {item.url}
                            {item.date ? " · " + item.date : ""}
                          </span>
                        ) : null}
                      </span>
                    </label>
                  </li>
                );
              })}
            </ul>
            <div className="mt-2 flex items-center gap-2">
              <button
                type="button"
                className="wr-primary-button"
                onClick={() => void savePickedAsMaterials()}
                disabled={saving || pickedUrls.length === 0}
                data-writing-websearch-save=""
              >
                {saving ? "正在保存…" : "存入素材篮（" + pickedUrls.length + "）"}
              </button>
              <span className="text-[10.5px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                按收藏落盘，先到素材库确认再采纳。
              </span>
            </div>
          </>
        ) : null}
      </section>

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
