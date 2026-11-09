// 写作版本面板（OD-WT-612）。
//
// 列表照 409 排期历史先例：每条版本给出编号、说明、时间、来源与快照类型，
// 当前版单独标注；对比走 writing_version_diff 的段落级变更；恢复必须先经
// 应用内确认弹窗，再把 confirmed=true 交给 writing_version_restore。
//
// 定稿按钮只表达用户动作，不接任何 AI 写通道。宿主目前没有写作任务状态
// 迁移 IPC，610 挂载时可传 onFinalize；未传时按钮保持禁用并在报告登记补口。

import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  WritingParagraphChange,
  WritingTaskStatus,
  WritingVersionDiff,
  WritingVersionList,
  WritingVersionRestored,
  WritingVersionSource,
  WritingVersionSummary,
} from "../types";
import { ConfirmDialog } from "./ConfirmDialog";

const SOURCE_LABEL: Record<WritingVersionSource, string> = {
  user: "用户",
  ai: "AI",
};

const SCOPE_LABEL: Record<WritingVersionSummary["scope"], string> = {
  outline: "大纲",
  section: "章节",
  full: "全文",
};

const CHANGE_SKIN: Record<WritingParagraphChange["kind"], string> = {
  新增: "text-tertiary bg-tertiary/10",
  删除: "text-error bg-error-container/60",
  修改: "text-amber-700 bg-amber-500/15",
};

export type WritingVersionPanelProps = {
  writingId: string;
  /** 当前写作任务状态；只有「待审核」允许定稿。 */
  status?: WritingTaskStatus;
  /** 恢复成功回执，供页面刷新任务卡。 */
  onRestored?: (result: WritingVersionRestored) => void;
  /**
   * 定稿动作由 610/613 接入宿主状态迁移命令后传入。未传时按钮禁用，
   * 不伪造成功，也不走 AI 或审阅通道。
   */
  onFinalize?: () => void | Promise<void>;
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

function defaultRange(versions: WritingVersionSummary[]): {
  from: number | null;
  to: number | null;
} {
  const sorted = [...versions].sort((a, b) => b.version - a.version);
  const current = sorted.find((entry) => entry.current) ?? sorted[0];
  if (!current) return { from: null, to: null };
  const previous = sorted.find(
    (entry) => entry.version < current.version && entry.scope === current.scope,
  );
  return { from: previous?.version ?? current.version, to: current.version };
}

function ChangeRow({ change }: { change: WritingParagraphChange }) {
  return (
    <div
      className="rounded-lg bg-white/80 px-2.5 py-2"
      data-version-change={change.kind}
      data-paragraph-no={change.paragraph_no}
    >
      <div className="flex items-center gap-1.5">
        <span className="mono text-[10px] text-outline">第 {change.paragraph_no} 段</span>
        <span className={"rounded-full px-1.5 py-[1px] text-[9.5px] " + CHANGE_SKIN[change.kind]}>
          {change.kind}
        </span>
      </div>
      {change.before ? (
        <p className="mt-1 text-[10.5px] leading-relaxed text-outline break-words whitespace-pre-wrap">
          <span className="mr-1 rounded bg-outline-variant/25 px-1 text-[9px]">原</span>
          {change.before}
        </p>
      ) : null}
      {change.after ? (
        <p className="mt-1 text-[11px] leading-relaxed text-on-surface-variant break-words whitespace-pre-wrap">
          <span className="mr-1 rounded bg-primary/10 px-1 text-[9px] text-primary">新</span>
          {change.after}
        </p>
      ) : null}
    </div>
  );
}

export function WritingVersionPanel({
  writingId,
  status,
  onRestored,
  onFinalize,
}: WritingVersionPanelProps) {
  const [list, setList] = useState<WritingVersionList | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [fromVersion, setFromVersion] = useState<number | null>(null);
  const [toVersion, setToVersion] = useState<number | null>(null);
  const [diff, setDiff] = useState<WritingVersionDiff | null>(null);
  const [pendingRestore, setPendingRestore] = useState<number | null>(null);
  const [finalizeOpen, setFinalizeOpen] = useState(false);
  const [finalizing, setFinalizing] = useState(false);

  useEffect(() => {
    if (!writingId) {
      setList(null);
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    setList(null);
    setDiff(null);
    setError(null);
    shell
      .writingVersionList(writingId)
      .then((result) => {
        if (!alive) return;
        setList(result);
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

  useEffect(() => {
    if (!list || list.versions.length === 0) {
      setFromVersion(null);
      setToVersion(null);
      setDiff(null);
      return;
    }
    const range = defaultRange(list.versions);
    setFromVersion(range.from);
    setToVersion(range.to);
    setDiff(null);
  }, [list]);

  const versions = useMemo(
    () => (list ? [...list.versions].sort((a, b) => b.version - a.version) : []),
    [list],
  );

  const compare = async () => {
    if (!writingId || busy) return;
    if (fromVersion === null || toVersion === null) {
      setMessage("先选择两个版本。");
      return;
    }
    if (fromVersion === toVersion) {
      setMessage("起点与终点需要是两个不同版本。");
      return;
    }
    setBusy("diff");
    setError(null);
    setMessage(null);
    try {
      const result = await shell.writingVersionDiff({
        writing_id: writingId,
        from_version: fromVersion,
        to_version: toVersion,
      });
      setDiff(result);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
    }
  };

  const restore = async () => {
    if (!writingId || pendingRestore === null || busy) return;
    const version = pendingRestore;
    setBusy("restore");
    setError(null);
    setMessage(null);
    try {
      const result = await shell.writingVersionRestore({
        writing_id: writingId,
        version,
        confirmed: true,
      });
      setMessage(`已恢复到 v${result.restored_version}；新当前版本 v${result.new_version.version}`);
      onRestored?.(result);
      setReloadToken((value) => value + 1);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(null);
      setPendingRestore(null);
    }
  };

  const finalize = async () => {
    if (!onFinalize || finalizing) return;
    setFinalizing(true);
    setError(null);
    try {
      await onFinalize();
      setMessage("已定稿。");
    } catch (e) {
      setError(errText(e));
    } finally {
      setFinalizing(false);
      setFinalizeOpen(false);
    }
  };

  const finalizeReady = status === "待审核";
  const finalizeChannelReady = Boolean(onFinalize);

  return (
    <section
      aria-label="版本历史"
      className="flex min-w-0 flex-col gap-3"
      data-testid="writing-version-panel"
      data-version-writing-id={writingId}
    >
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h2 className="text-[15px] font-semibold tracking-tight text-on-surface">版本历史</h2>
        <span className="mono text-[10px] text-outline">
          {list ? `v${list.current_version} · ${versions.length} 版` : loading ? "读取中" : "暂无"}
        </span>
        <div className="ml-auto flex items-center gap-1.5">
          <button
            type="button"
            onClick={() => setFinalizeOpen(true)}
            disabled={!finalizeReady || !finalizeChannelReady || finalizing}
            className="rounded-lg border border-primary/35 px-3 py-1.5 text-[11.5px] text-primary hover:bg-primary/10 disabled:opacity-45"
            data-finalize-button
            data-finalize-status={status ?? ""}
            data-finalize-missing-channel={finalizeChannelReady ? undefined : "true"}
          >
            {finalizing ? "定稿中…" : "定稿"}
          </button>
        </div>
      </div>

      {!finalizeChannelReady ? (
        <p className="text-[10px] text-outline" data-finalize-note="missing-channel">
          宿主尚未提供写作任务状态迁移命令。
        </p>
      ) : status !== "待审核" ? (
        <p className="text-[10px] text-outline" data-finalize-note="not-pending-review">
          只有「待审核」的写作任务可以定稿。
        </p>
      ) : null}

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

      {loading ? (
        <p className="text-[11px] text-outline">正在读取版本记录…</p>
      ) : versions.length === 0 ? (
        <div
          className="rounded-[18px] border border-white/80 bg-white/70 px-5 py-9 text-center text-[12px] text-outline"
          data-version-empty="no-versions"
        >
          还没有版本快照。确认大纲或正文后会生成。
        </div>
      ) : (
        <>
          <div className="flex flex-col gap-1.5" data-version-list>
            {versions.map((version) => (
              <article
                key={version.version}
                className="rounded-xl border border-surface-variant/25 bg-surface-container-low/35 px-3 py-2"
                data-version-item={version.version}
              >
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="mono text-[11px] font-semibold text-on-surface">v{version.version}</span>
                  {version.current ? (
                    <span className="rounded-full bg-primary/10 px-1.5 py-[1px] text-[9.5px] text-primary">当前</span>
                  ) : null}
                  <span className="rounded-full bg-outline-variant/25 px-1.5 py-[1px] text-[9.5px] text-outline">
                    {SCOPE_LABEL[version.scope]}
                  </span>
                  <span className="text-[10px] text-on-surface-variant">{SOURCE_LABEL[version.source]}</span>
                  <span className="mono text-[9.5px] text-outline">{shortTime(version.at)}</span>
                  <button
                    type="button"
                    onClick={() => setPendingRestore(version.version)}
                    disabled={version.current || Boolean(busy)}
                    className="ml-auto rounded-lg border border-outline-variant/55 bg-white/75 px-2.5 py-1 text-[10.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-40"
                    data-version-restore={version.version}
                  >
                    {version.current ? "当前版" : "恢复"}
                  </button>
                </div>
                <p className="mt-1 text-[11px] leading-relaxed text-on-surface break-words">
                  {version.note.trim() || "无说明"}
                </p>
                {version.diff && version.diff.changed_paragraphs > 0 ? (
                  <p className="mt-0.5 text-[9.5px] text-outline" data-version-diff-summary={version.version}>
                    较 v{version.diff.previous_version} · 新增 {version.diff.added} · 删除 {version.diff.removed} · 修改 {version.diff.modified}
                  </p>
                ) : null}
              </article>
            ))}
          </div>

          <section aria-label="版本对比" className="rounded-xl bg-surface-container-low/50 px-3 py-2.5" data-version-compare>
            <div className="flex flex-wrap items-center gap-1.5">
              <select
                className="neu-inset-field rounded-lg px-2 py-1 text-[11px] text-on-surface outline-none"
                value={fromVersion ?? ""}
                onChange={(event) => {
                  setFromVersion(Number(event.target.value));
                  setDiff(null);
                }}
                aria-label="对比起点版本"
                data-version-from
              >
                {versions.map((version) => (
                  <option key={version.version} value={version.version}>
                    v{version.version} · {SCOPE_LABEL[version.scope]} · {version.note.trim() || "无说明"}
                  </option>
                ))}
              </select>
              <span className="text-[10px] text-outline">→</span>
              <select
                className="neu-inset-field rounded-lg px-2 py-1 text-[11px] text-on-surface outline-none"
                value={toVersion ?? ""}
                onChange={(event) => {
                  setToVersion(Number(event.target.value));
                  setDiff(null);
                }}
                aria-label="对比终点版本"
                data-version-to
              >
                {versions.map((version) => (
                  <option key={version.version} value={version.version}>
                    v{version.version} · {SCOPE_LABEL[version.scope]} · {version.note.trim() || "无说明"}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => void compare()}
                disabled={Boolean(busy) || fromVersion === toVersion}
                className="rounded-lg bg-primary px-3 py-1.5 text-[11px] text-on-primary disabled:opacity-45"
                data-version-diff-button
              >
                {busy === "diff" ? "对比中…" : "对比"}
              </button>
            </div>

            {diff ? (
              <div className="mt-2.5" data-version-diff>
                <div className="flex flex-wrap items-baseline gap-2">
                  <h3 className="text-[12px] font-semibold text-on-surface">
                    v{diff.from.version} → v{diff.to.version}
                  </h3>
                  <span className="mono text-[9.5px] text-outline">
                    段落 {diff.diff.changed_paragraph_nos.length} · 新增 {diff.diff.changes.filter((entry) => entry.kind === "新增").length}
                    {" · "}删除 {diff.diff.changes.filter((entry) => entry.kind === "删除").length}
                    {" · "}修改 {diff.diff.changes.filter((entry) => entry.kind === "修改").length}
                  </span>
                </div>
                {diff.diff.changes.length === 0 ? (
                  <p className="mt-1.5 text-[11px] text-outline" data-version-diff-empty>
                    这两版没有段落级差异。
                  </p>
                ) : (
                  <div className="mt-2 flex flex-col gap-1.5">
                    {diff.diff.changes.map((change) => (
                      <ChangeRow key={`${change.paragraph_no}-${change.kind}`} change={change} />
                    ))}
                  </div>
                )}
              </div>
            ) : null}
          </section>
        </>
      )}

      <ConfirmDialog
        open={pendingRestore !== null}
        title="恢复历史版本"
        description={
          pendingRestore === null
            ? "恢复会新增一个当前版本，历史版本仍保留。"
            : `将 v${pendingRestore} 的内容恢复为新的当前版本；历史版本仍保留。`
        }
        confirmLabel="恢复"
        onConfirm={() => void restore()}
        onCancel={() => setPendingRestore(null)}
        testId="writing-version-restore-confirm"
      />
      <ConfirmDialog
        open={finalizeOpen}
        title="确认定稿"
        description="定稿后写作任务进入「已定稿」，才可进入导出与归档流程。"
        confirmLabel="确认定稿"
        onConfirm={() => void finalize()}
        onCancel={() => setFinalizeOpen(false)}
        testId="writing-finalize-confirm"
      />
    </section>
  );
}

