import { useState } from "react";

import { host, shell, type KnowledgeSyncReport } from "../api";
import { TERMINAL_STATES, type KnowledgeIngestPrepare, type KnowledgeReviewItem } from "../types";
import { Icon } from "../components/Icon";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

type Phase = "idle" | "preparing" | "running" | "applying" | "done" | "error";

/** 知识库页面的「整理材料」入口：inbox/ 与 raw/ 可读文本进入审核队列。 */
export function KnowledgeIngestPanel({ onApplied }: { onApplied?: () => void } = {}) {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [prepared, setPrepared] = useState<KnowledgeIngestPrepare | null>(null);
  const [staged, setStaged] = useState<KnowledgeReviewItem | null>(null);
  // OD-ARCH-152：模型只写 workspace/staging/kb/，落库由这里触发宿主同步。
  const [syncReport, setSyncReport] = useState<KnowledgeSyncReport | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [confirmAll, setConfirmAll] = useState(false);
  const [message, setMessage] = useState("");

  const prepare = async () => {
    setPhase("preparing");
    setStaged(null);
    setMessage("正在扫描 inbox/ 与 raw/ 并组装整理提示词…");
    try {
      await shell.knowledgeIngestInit();
      const result = await shell.knowledgeIngestPrepare();
      setPrepared(result);
      setPhase("idle");
      setMessage(
        result.files.length > 0
          ? `待整理 ${result.files.length} 份材料（inbox 优先，raw 新→旧，最多每批 5 份）` +
              (result.skipped_processed > 0
                ? `；已处理跳过 ${result.skipped_processed} 份`
                : "") +
              "。"
          : "inbox/ 与 raw/ 没有可整理的可读文本（.md/.txt）。",
      );
    } catch (e) {
      setPhase("error");
      setMessage(String(e));
    }
  };

  const run = async () => {
    setPhase("running");
    setStaged(null);
    setMessage("正在提交整理任务…");
    try {
      const prep = prepared ?? (await shell.knowledgeIngestPrepare());
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      const receipt = await host.submitTask({
        promptText: prep.prompt,
        intent: "knowledge_ingest",
        workspaceDir: workspace.workspace_dir,
        // OD-ARCH-144：红线扫描只针对 inbox 原文；建卡规范本身含边界用词。
        userInputText: prep.user_input_text,
      });
      let status = await host.taskStatus(receipt.omni_task_id);
      // 真实材料可能让模型整理超过 3 分钟；这里不再按 180 秒误报「未完成」。
      // 不自动取消任务；15 分钟仍非终态时如实提示，用户可关闭面板后稍后重试。
      for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(receipt.omni_task_id);
        if (i % 10 === 0) {
          setMessage(`模型正在整理材料…已等待 ${i + 1} 秒（不会自动取消）`);
        }
      }
      if (status.status !== "succeeded") {
        throw new Error(
          `整理任务仍未完成：${status.status}${status.note ? " · " + status.note : ""}` +
            (TERMINAL_STATES.includes(status.status)
              ? ""
              : "；已等待 900 秒，任务没有自动取消，可关闭面板后稍后再试。"),
        );
      }
      const result = await host.taskResult(receipt.omni_task_id);
      if (!result.content.trim()) {
        throw new Error("模型没有返回整理结果。");
      }
      setPhase("applying");
      setMessage("正在校验并送入人工审核…");
      // OD-KB-201 S3：apply 前必须先出审核卡；这里只暂存，不写知识库。
      const item = await shell.knowledgeReviewStage(
        result.content,
        prep.files.map((file) => file.relative_path),
      );
      setStaged(item);
      setPhase("done");
      setMessage(
        `已生成待审核项：${item.card_count} 张卡` +
          (item.rejected_cards.length > 0
            ? `；${item.rejected_cards.length} 张未通过校验已跳过`
            : "") +
          "。请到「知识审核」裁决后落盘。",
      );
      // 通知知识库页面刷新审核队列与扫描结果。
      onApplied?.();
    } catch (e) {
      setPhase("error");
      setMessage(String(e));
    }
  };

  const sync = async () => {
    setConfirmAll(false);
    setSyncing(true);
    setSyncReport(null);
    setMessage("正在把 workspace/staging/kb/ 的暂存卡同步到知识库…");
    try {
      const report = await shell.knowledgeSyncToVault();
      setSyncReport(report);
      setMessage(
        report.ok
          ? report.note
          : report.note + "；冲突与错误项保留在暂存区，处理后重试。",
      );
      if (report.created.length > 0) {
        onApplied?.();
      }
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSyncing(false);
    }
  };

  return (
    <>
      <button
        type="button"
        data-testid="knowledge-ingest-button"
        onClick={() => {
          if (open) {
            setOpen(false);
          } else {
            setOpen(true);
            void prepare();
          }
        }}
        className="inline-flex items-center gap-1 rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
        data-hover-hint="把 inbox/ 与 raw/ 里的可读文本交给模型整理成知识卡；未经审核不落盘"
      >
        <Icon name="spark" className="w-[13px] h-[13px]" />
        <span>整理材料</span>
      </button>
      {open ? (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-4">
          <div
            data-testid="knowledge-ingest-panel"
            className="w-full max-w-lg rounded-xl bg-surface p-4 shadow-xl ring-1 ring-outline-variant/40 space-y-3"
          >
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-[14px] font-medium text-on-surface">整理材料</p>
                <p className="text-[11px] text-outline">inbox/ 全部优先，raw/ 只取可读文本并按新→旧；每批最多 5 份。</p>
              </div>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="text-outline hover:text-on-surface"
                aria-label="关闭整理材料"
              >
                ×
              </button>
            </div>

            {prepared?.files.length ? (
              <ul className="max-h-32 overflow-y-auto rounded-lg bg-surface-container-low p-2 text-[11px] text-on-surface-variant space-y-1">
                {prepared.files.map((file) => (
                  <li key={file.relative_path} className="truncate" title={file.relative_path}>
                    {file.relative_path} · {file.chars} 字
                  </li>
                ))}
              </ul>
            ) : null}

            <p className="text-[11.5px] text-on-surface-variant" role="status">
              {message || "准备就绪。"}
            </p>

            {staged ? (
              <div data-testid="knowledge-ingest-report" className="rounded-lg bg-surface-container-low p-2.5 text-[11.5px] space-y-1">
                <p>待审核 {staged.card_count} 张；尚未写入知识库。</p>
                {staged.cards.map((card) => (
                  <p key={card.relative_path} className="truncate text-on-surface-variant">◦ {card.title} · {card.relative_path}</p>
                ))}
                {staged.rejected_cards.slice(0, 5).map((rejected) => (
                  <p key={rejected.relative_path} className="text-error">× 跳过 {rejected.relative_path}：{rejected.reason}</p>
                ))}
                {staged.parse_error ? <p className="text-error">× {staged.parse_error}</p> : null}
              </div>
            ) : null}

            {syncReport ? (
              <div
                data-testid="knowledge-sync-report"
                className="rounded-lg bg-surface-container-low p-2.5 text-[11.5px] space-y-1"
              >
                {syncReport.created.map((card) => (
                  <p key={card.relative_path} className="truncate text-on-surface-variant">
                    ◦ 已落库 {card.title} · {card.relative_path}
                  </p>
                ))}
                {syncReport.conflicts.slice(0, 5).map((conflict) => (
                  <p key={"c-" + conflict.relative_path} className="text-error">
                    × 冲突 {conflict.relative_path}：{conflict.reason}
                  </p>
                ))}
                {syncReport.errors.slice(0, 5).map((error) => (
                  <p key={"e-" + error.relative_path} className="text-error">
                    × 失败 {error.relative_path}：{error.reason}
                  </p>
                ))}
                {syncReport.created.length === 0 &&
                syncReport.conflicts.length === 0 &&
                syncReport.errors.length === 0 ? (
                  <p className="text-outline">暂存区没有待同步卡片。</p>
                ) : null}
              </div>
            ) : null}

            <div className="flex items-center justify-end gap-2">
              <button
                type="button"
                data-testid="knowledge-sync-button"
                data-hover-hint="把暂存区全部卡片写入知识库；需要再点一次确认。冲突不覆盖"
                onClick={() => {
                  if (!confirmAll) {
                    setConfirmAll(true);
                    setMessage("确认把暂存区全部卡片写入知识库？再点一次「确认全部入库」。");
                    return;
                  }
                  void sync();
                }}
                disabled={syncing}
                className="mr-auto px-3 py-1 rounded-full bg-surface-container text-on-surface text-[11.5px] disabled:opacity-50"
              >
                {syncing ? "正在同步…" : confirmAll ? "确认全部入库" : "全部入库"}
              </button>
              <button
                type="button"
                onClick={() => void prepare()}
                disabled={phase === "preparing" || phase === "running" || phase === "applying"}
                className="px-3 py-1 rounded-full bg-surface-container text-on-surface text-[11.5px] disabled:opacity-50"
              >
                重新扫描
              </button>
              <button
                type="button"
                onClick={() => void run()}
                disabled={!prepared || phase === "preparing" || phase === "running" || phase === "applying"}
                className="px-3 py-1 rounded-full bg-primary text-on-primary text-[11.5px] disabled:opacity-50"
              >
                开始整理
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
