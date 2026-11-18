import { useCallback, useEffect, useState } from "react";

import { shell, type StagedKnowledgeCard } from "../api";
import { Icon } from "../components/Icon";

/**
 * OD-KB-202：模型写在 workspace/staging/kb/ 的暂存卡先进入审核。
 *
 * 只读列表来自宿主；「入库选中」后才调用落库命令并删除对应暂存文件。
 * 未勾选的卡保持原样，冲突/超长由宿主原样报错，不静默覆盖。
 */
export function KnowledgeStagingInbox({ onChanged }: { onChanged?: () => void } = {}) {
  const [cards, setCards] = useState<StagedKnowledgeCard[]>([]);
  const [stagingDir, setStagingDir] = useState("");
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState<"load" | "sync" | "discard" | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<"sync" | "discard" | "all" | null>(null);

  const load = useCallback(async () => {
    setBusy("load");
    setError(null);
    try {
      const list = await shell.knowledgeStagingList();
      setCards(list.cards);
      setStagingDir(list.staging_dir);
      setSelected((prev) => {
        const next: Record<string, boolean> = {};
        for (const card of list.cards) {
          if (prev[card.relative_path]) next[card.relative_path] = true;
        }
        return next;
      });
      if (list.errors.length > 0) {
        setMessage(`暂存目录有 ${list.errors.length} 项无法读取，见下方提示。`);
      }
      return list.cards;
    } catch (e) {
      setError(String(e));
      return [];
    } finally {
      setBusy(null);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const ready = cards.filter((card) => card.ready);
  const selectedPaths = Object.keys(selected).filter((path) => selected[path]);
  const allSelected = ready.length > 0 && ready.every((card) => selected[card.relative_path]);

  const toggle = (path: string) =>
    setSelected((prev) => ({ ...prev, [path]: !prev[path] }));

  const toggleAll = () => {
    if (allSelected) {
      setSelected({});
      return;
    }
    const next: Record<string, boolean> = {};
    for (const card of ready) next[card.relative_path] = true;
    setSelected(next);
  };

  const runSync = async (paths: string[]) => {
    if (paths.length === 0) return;
    setBusy("sync");
    setError(null);
    setMessage("");
    try {
      const report = await shell.knowledgeStagingSync(paths);
      const lines = [
        report.note,
        ...report.created.map((card) => `◦ 已入库 ${card.title} · ${card.relative_path}`),
        ...report.conflicts.map((item) => `× 冲突 ${item.relative_path}：${item.reason}`),
        ...report.errors.map((item) => `× 失败 ${item.relative_path}：${item.reason}`),
      ];
      setMessage(lines.join("\n"));
      await load();
      onChanged?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      setConfirming(null);
    }
  };

  const runDiscard = async (paths: string[]) => {
    if (paths.length === 0) return;
    setBusy("discard");
    setError(null);
    setMessage("");
    try {
      const report = await shell.knowledgeStagingDiscard(paths);
      setMessage(
        `已丢弃 ${report.removed.length} 张` +
          (report.errors.length > 0 ? `；${report.errors.length} 项失败` : ""),
      );
      if (report.errors.length > 0) {
        setError(report.errors.map((item) => `${item.relative_path}：${item.reason}`).join("\n"));
      }
      await load();
      onChanged?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      setConfirming(null);
    }
  };

  if (busy === "load" && cards.length === 0 && !error) {
    return <p className="text-[11px] text-outline">正在读取暂存卡…</p>;
  }
  if (cards.length === 0 && !error) {
    return null;
  }

  return (
    <section
      className="shrink-0 rounded-xl bg-white/70 border border-white/90 px-3 py-2.5"
      data-testid="knowledge-staging-inbox"
    >
      <div className="flex flex-wrap items-baseline gap-2">
        <Icon name="checklist" className="w-[14px] h-[14px] text-primary" />
        <h3 className="text-[12.5px] font-semibold text-on-surface">暂存卡待审核</h3>
        <span className="mono text-[10px] text-outline">{cards.length} 张</span>
        <span className="ml-auto text-[10px] text-outline">入库前请核对标题与正文预览</span>
      </div>

      <div className="mt-1.5 max-h-[240px] overflow-y-auto rounded-lg bg-[color:var(--od-neu-sunken)] p-1.5">
        <label className="flex items-center gap-2 px-1.5 pb-1 text-[10.5px] text-outline cursor-pointer">
          <input type="checkbox" checked={allSelected} onChange={toggleAll} />
          全选可入库卡片
        </label>
        {cards.map((card) => (
          <label
            key={card.relative_path}
            className="flex items-start gap-2 rounded-md px-1.5 py-1 text-[11px] leading-relaxed hover:bg-white/70"
          >
            <input
              type="checkbox"
              className="mt-0.5"
              checked={!!selected[card.relative_path]}
              disabled={!card.ready}
              onChange={() => toggle(card.relative_path)}
            />
            <span className="min-w-0 flex-1">
              <span className="flex items-baseline gap-1.5">
                <span className="truncate font-medium text-on-surface">{card.title}</span>
                <span className="shrink-0 rounded bg-primary/10 px-1 text-[9px] text-primary">{card.kind}</span>
                <span className="shrink-0 mono text-[9.5px] text-outline">{card.chars} 字</span>
              </span>
              <span className="block truncate text-[10.5px] text-on-surface-variant" title={card.preview}>
                {card.preview || "（无正文预览）"}
              </span>
              {!card.ready ? (
                <span className="block text-[10.5px] text-error">不可入库：{card.reason}</span>
              ) : null}
            </span>
          </label>
        ))}
      </div>

      {stagingDir ? (
        <p className="mt-1 truncate text-[9.5px] text-outline" title={stagingDir}>
          暂存：{stagingDir}
        </p>
      ) : null}

      {confirming ? (
        <div className="mt-2 flex items-center gap-2 rounded-lg bg-amber-50/80 px-2 py-1.5 text-[11px] text-amber-900">
          <span>
            {confirming === "all"
              ? `确认把全部 ${ready.length} 张可入库卡片写入知识库？`
              : confirming === "discard"
                ? `确认丢弃选中的 ${selectedPaths.length} 张暂存卡？`
                : `确认把选中的 ${selectedPaths.length} 张卡片写入知识库？`}
          </span>
          <button
            type="button"
            className="ml-auto rounded-md bg-primary px-2.5 py-0.5 text-[10.5px] text-on-primary"
            onClick={() =>
              void (confirming === "discard"
                ? runDiscard(selectedPaths)
                : runSync(confirming === "all" ? ready.map((card) => card.relative_path) : selectedPaths))
            }
          >
            确认
          </button>
          <button
            type="button"
            className="rounded-md px-2 py-0.5 text-[10.5] text-on-surface-variant"
            onClick={() => setConfirming(null)}
          >
            取消
          </button>
        </div>
      ) : null}

      {message ? (
        <pre className="mt-1.5 max-h-24 overflow-y-auto whitespace-pre-wrap rounded-lg bg-surface-container-low px-2 py-1 text-[10.5px] leading-relaxed text-on-surface-variant" data-testid="knowledge-staging-report">
          {message}
        </pre>
      ) : null}
      {error ? (
        <pre className="mt-1.5 max-h-24 overflow-y-auto whitespace-pre-wrap rounded-lg bg-error-container/50 px-2 py-1 text-[10.5px] leading-relaxed text-on-error-container" role="alert">
          {error}
        </pre>
      ) : null}

      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          data-testid="knowledge-staging-sync-selected"
          className="rounded-lg bg-primary px-2.5 py-1 text-[10.5px] text-on-primary disabled:opacity-45"
          disabled={!!busy || selectedPaths.length === 0}
          onClick={() => setConfirming("sync")}
        >
          {busy === "sync" ? "入库中…" : `入库选中（${selectedPaths.length}）`}
        </button>
        <button
          type="button"
          data-testid="knowledge-staging-sync-all"
          className="rounded-lg border border-outline-variant/60 px-2.5 py-1 text-[10.5px] text-on-surface-variant disabled:opacity-45"
          disabled={!!busy || ready.length === 0}
          onClick={() => setConfirming("all")}
        >
          全部入库（{ready.length}）
        </button>
        <button
          type="button"
          data-testid="knowledge-staging-discard"
          className="rounded-lg border border-outline-variant/60 px-2.5 py-1 text-[10.5px] text-on-surface-variant disabled:opacity-45"
          disabled={!!busy || selectedPaths.length === 0}
          onClick={() => setConfirming("discard")}
        >
          丢弃选中
        </button>
        <button
          type="button"
          className="ml-auto rounded-lg px-2 py-1 text-[10.5px] text-outline hover:text-primary disabled:opacity-45"
          disabled={!!busy}
          onClick={() => void load()}
        >
          {busy === "load" ? "读取中…" : "刷新"}
        </button>
      </div>
    </section>
  );
}

