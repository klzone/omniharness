import { useEffect, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import type {
  KnowledgeLink,
  KnowledgeLinkCandidate,
  KnowledgeLinkSubjectKind,
} from "../types";
import { SourcePeekSheet } from "./SourcePeekSheet";

type PanelNotice = { kind: "ok" | "err"; text: string };
type Peek = { title: string; body: string };

function errText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function linkedAtText(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    date.getFullYear() +
    "-" +
    pad(date.getMonth() + 1) +
    "-" +
    pad(date.getDate()) +
    " " +
    pad(date.getHours()) +
    ":" +
    pad(date.getMinutes())
  );
}

/** 只显示命中行附近的原文，行号与候选里的定位一致。 */
function excerpt(content: string, line: number | null): string {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  if (line === null) {
    const shown = lines.slice(0, 60);
    return shown
      .map((text, index) => `${index + 1}: ${text}`)
      .join("\n") + (lines.length > shown.length ? "\n…" : "");
  }
  const target = Math.max(1, line);
  const start = Math.max(1, target - 4);
  const end = Math.min(lines.length, target + 10);
  return (
    `第 ${target} 行的原文定位：\n` +
    lines
      .slice(start - 1, end)
      .map((text, index) => {
        const number = start + index;
        return `${number === target ? "▶" : " "} ${number}: ${text}`;
      })
      .join("\n")
  );
}

/**
 * 记录/任务详情里的「关联知识」区块（OD-KM-511）。
 *
 * 候选由 507 的 suggest 命令暂存，界面只用返回的候选；确认必须填理由，
 * 回执里的 record/task 直接替换本地关联清单，不依赖 updated_at 刷新。
 * 原文定位复用 SourcePeekSheet，路径与行号全部来自候选。
 */
export function KnowledgeLinkPanel({
  subjectKind,
  subjectId,
  links,
  onLinksChanged,
  onNotice,
}: {
  subjectKind: KnowledgeLinkSubjectKind;
  subjectId: string;
  links: KnowledgeLink[];
  onLinksChanged: (links: KnowledgeLink[]) => void;
  onNotice?: (notice: PanelNotice | null) => void;
}) {
  const [candidates, setCandidates] = useState<KnowledgeLinkCandidate[] | null>(null);
  const [reasons, setReasons] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [status, setStatus] = useState<PanelNotice | null>(null);
  const [peek, setPeek] = useState<Peek | null>(null);
  const [peekBusy, setPeekBusy] = useState(false);

  // 详情切换到另一条记录/任务时，上一条的候选与理由不能留在面板里。
  useEffect(() => {
    setCandidates(null);
    setReasons({});
    setStatus(null);
    setPeek(null);
  }, [subjectKind, subjectId]);

  function announce(notice: PanelNotice | null) {
    setStatus(notice);
    onNotice?.(notice);
  }

  async function suggest() {
    if (busy) return;
    setBusy("suggest");
    announce(null);
    try {
      const report = await shell.knowledgeLinkSuggest(subjectKind, subjectId, []);
      // 再次推荐时宿主会跳过仍在待处理的候选；界面保留它们，避免候选凭空消失。
      setCandidates((prev) => {
        const merged = [...(prev ?? [])];
        for (const candidate of report.staged) {
          if (!merged.some((item) => item.id === candidate.id)) merged.push(candidate);
        }
        return merged;
      });
      const skipped = report.skipped_duplicates.length;
      announce({
        kind: "ok",
        text:
          report.staged.length === 0
            ? skipped > 0
              ? `没有新的推荐候选；${skipped} 条已关联或已在待处理候选里。`
              : "没有新的推荐候选。"
            : `已生成 ${report.staged.length} 条推荐候选，确认后才写入关联。` +
              (skipped > 0 ? `跳过 ${skipped} 条重复候选。` : ""),
      });
    } catch (error) {
      announce({ kind: "err", text: "推荐失败：" + errText(error) });
    } finally {
      setBusy(null);
    }
  }

  async function confirm(candidate: KnowledgeLinkCandidate) {
    if (busy) return;
    const reason = (reasons[candidate.id] ?? "").trim();
    if (!reason) {
      announce({ kind: "err", text: "确认关联必须填写理由。" });
      return;
    }
    setBusy(candidate.id);
    announce(null);
    try {
      const report = await shell.knowledgeLinkConfirm(candidate.id, reason);
      const next =
        report.record?.related_knowledge ?? report.task?.related_knowledge ?? null;
      if (next) onLinksChanged(next);
      setCandidates((prev) => (prev ? prev.filter((item) => item.id !== candidate.id) : prev));
      setReasons((prev) => {
        const copy = { ...prev };
        delete copy[candidate.id];
        return copy;
      });
      announce(
        report.ok
          ? { kind: "ok", text: "已确认关联：" + candidate.title + "。" }
          : {
              kind: "err",
              text:
                "关联已写入，但回执不完整：" +
                ((report.error ?? report.warnings.join("；")) ||
                  "请重新打开详情核对。"),
            },
      );
    } catch (error) {
      announce({ kind: "err", text: "确认关联失败：" + errText(error) });
    } finally {
      setBusy(null);
    }
  }

  async function remove(link: KnowledgeLink) {
    if (busy) return;
    setBusy("remove:" + link.id);
    announce(null);
    try {
      const report = await shell.knowledgeLinkRemove(subjectKind, subjectId, link.id);
      const next =
        report.record?.related_knowledge ?? report.task?.related_knowledge ?? null;
      if (next) onLinksChanged(next);
      announce(
        report.ok
          ? { kind: "ok", text: "已移除关联：" + link.id + "。" }
          : {
              kind: "err",
              text:
                "移除未完整完成：" +
                ((report.error ?? report.warnings.join("；")) ||
                  "请重新打开详情核对。"),
            },
      );
    } catch (error) {
      announce({ kind: "err", text: "移除关联失败：" + errText(error) });
    } finally {
      setBusy(null);
    }
  }

  async function openPeek(knowledgeId: string, line: number | null) {
    if (peekBusy) return;
    setPeekBusy(true);
    announce(null);
    try {
      const note = await shell.knowledgeVaultReadNote(knowledgeId);
      setPeek({
        title: line === null ? knowledgeId : `${knowledgeId} · 第 ${line} 行`,
        body: excerpt(note.content, line),
      });
    } catch (error) {
      announce({ kind: "err", text: "读取知识原文失败：" + errText(error) });
    } finally {
      setPeekBusy(false);
    }
  }

  return (
    <section
      aria-label="关联知识"
      data-knowledge-links=""
      className="mt-3 max-h-[45vh] overflow-y-auto rounded-xl border border-outline-variant/35 bg-surface-container-low/45 px-3 py-2.5"
    >
      <div className="flex items-center gap-2">
        <h3 className="text-[12px] font-semibold text-on-surface">关联知识</h3>
        <span className="mono text-[10px] text-outline" data-knowledge-link-count="">
          {links.length}
        </span>
        <button
          type="button"
          onClick={() => void suggest()}
          disabled={busy !== null}
          data-knowledge-suggest=""
          className="ml-auto rounded-full bg-surface-container px-2.5 py-1 text-[10.5px] font-medium text-on-surface-variant hover:bg-surface-container-high disabled:opacity-40"
        >
          {busy === "suggest" ? "推荐中…" : "AI 推荐"}
        </button>
      </div>

      {status ? (
        <p
          role="status"
          data-knowledge-notice={status.kind}
          className={
            "mt-1.5 text-[10.5px] leading-relaxed " +
            (status.kind === "err" ? "text-error" : "text-on-surface-variant")
          }
        >
          {status.text}
        </p>
      ) : null}

      {links.length === 0 ? (
        <p className="mt-1.5 text-[10.5px] text-outline" data-knowledge-links-empty="">
          还没有关联知识。
        </p>
      ) : (
        <ul className="mt-1.5 flex max-h-[200px] flex-col gap-1.5 overflow-y-auto" data-knowledge-linked-list="">
          {links.map((link) => (
            <li
              key={link.id}
              className="rounded-lg bg-white/70 px-2 py-1.5"
              data-knowledge-linked={link.id}
            >
              <div className="flex items-start gap-1.5">
                <span className="min-w-0 flex-1">
                  <span className="flex flex-wrap items-center gap-1.5">
                    <span
                      className="rounded-full bg-primary-fixed/70 px-1.5 py-0.5 text-[9px] font-medium text-on-primary-fixed-variant"
                      data-link-source="user"
                    >
                      用户确认
                    </span>
                    <span className="mono text-[9.5px] text-primary break-all">{link.id}</span>
                  </span>
                  <span className="mt-0.5 block text-[10.5px] leading-relaxed text-on-surface-variant break-words">
                    {link.reason}
                  </span>
                  <span className="mt-0.5 block mono text-[9px] text-outline">
                    确认于 {linkedAtText(link.linked_at)}
                  </span>
                </span>
                <button
                  type="button"
                  onClick={() => void openPeek(link.id, null)}
                  disabled={peekBusy || busy !== null}
                  className="flex shrink-0 items-center gap-1 text-[10px] text-on-surface-variant hover:text-primary disabled:opacity-40"
                  data-knowledge-peek={link.id}
                >
                  <Icon name="book" className="h-3 w-3" />
                  原文
                </button>
                <button
                  type="button"
                  onClick={() => void remove(link)}
                  disabled={busy !== null}
                  className="shrink-0 text-[10px] text-outline hover:text-error disabled:opacity-40"
                  data-knowledge-remove={link.id}
                >
                  {busy === "remove:" + link.id ? "移除中…" : "移除"}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-2 border-t border-surface-variant/25 pt-2" data-knowledge-candidates-block="">
        <p className="text-[10px] font-medium text-on-surface">推荐候选</p>
        {candidates === null ? (
          <p className="mt-0.5 text-[10.5px] text-outline" data-knowledge-candidates-empty="">
            还没有推荐候选。点「AI 推荐」从知识库查找。
          </p>
        ) : candidates.length === 0 ? (
          <p className="mt-0.5 text-[10.5px] text-outline" data-knowledge-candidates-empty="">
            没有新的推荐候选。
          </p>
        ) : (
          <ul className="mt-1 flex max-h-[260px] flex-col gap-1.5 overflow-y-auto" data-knowledge-candidates="">
            {candidates.map((candidate) => (
              <li
                key={candidate.id}
                className="rounded-lg border border-outline-variant/35 bg-white/70 px-2 py-1.5"
                data-knowledge-candidate={candidate.id}
              >
                <div className="flex flex-wrap items-center gap-1.5">
                  <span
                    className="rounded-full bg-surface-container px-1.5 py-0.5 text-[9px] font-medium text-on-surface-variant"
                    data-link-source="ai"
                  >
                    AI 推荐
                  </span>
                  <span className="min-w-0 flex-1 text-[10.5px] font-medium text-on-surface break-words">
                    {candidate.title}
                  </span>
                  <span className="mono text-[9px] text-outline">{candidate.reason}</span>
                </div>
                <p className="mt-0.5 text-[10px] leading-relaxed text-on-surface-variant break-words">
                  {candidate.reason_text}
                </p>
                <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
                  <span className="mono text-[9px] text-outline break-all">
                    {candidate.path} · 第 {candidate.line} 行
                  </span>
                  <button
                    type="button"
                    onClick={() => void openPeek(candidate.knowledge_id, candidate.line)}
                    disabled={peekBusy || busy !== null}
                    className="text-[10px] text-primary hover:underline disabled:opacity-40"
                    data-knowledge-candidate-peek={candidate.id}
                  >
                    查看原文
                  </button>
                </div>
                <div className="mt-1 flex items-end gap-1.5">
                  <input
                    value={reasons[candidate.id] ?? ""}
                    onChange={(event) =>
                      setReasons((prev) => ({ ...prev, [candidate.id]: event.target.value }))
                    }
                    placeholder="确认理由（必填）"
                    aria-label={`关联理由：${candidate.title}`}
                    data-knowledge-reason={candidate.id}
                    className="min-w-0 flex-1 rounded-lg border border-outline-variant/50 bg-white px-2 py-1 text-[10.5px] text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30"
                  />
                  <button
                    type="button"
                    onClick={() => void confirm(candidate)}
                    disabled={busy !== null || !(reasons[candidate.id] ?? "").trim()}
                    className="shrink-0 rounded-full bg-primary px-2.5 py-1 text-[10px] font-medium text-on-primary disabled:opacity-40"
                    data-knowledge-confirm={candidate.id}
                  >
                    {busy === candidate.id ? "确认中…" : "确认"}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <SourcePeekSheet
        open={peek !== null}
        title={peek?.title ?? ""}
        body={peek?.body ?? ""}
        onClose={() => setPeek(null)}
      />
    </section>
  );
}
