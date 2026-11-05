import { useState } from "react";

import { host, shell } from "../api";
import { TERMINAL_STATES, type KnowledgeIngestApplyReport, type KnowledgeIngestPrepare } from "../types";
import { Icon } from "../components/Icon";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

type Phase = "idle" | "preparing" | "running" | "applying" | "done" | "error";

/** 首页/工作记录场景的「整理收件箱」入口（OD-ARCH-141 S6）。 */
export function KnowledgeIngestPanel() {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [prepared, setPrepared] = useState<KnowledgeIngestPrepare | null>(null);
  const [report, setReport] = useState<KnowledgeIngestApplyReport | null>(null);
  const [message, setMessage] = useState("");

  const prepare = async () => {
    setPhase("preparing");
    setReport(null);
    setMessage("正在扫描收件箱并组装整理提示词…");
    try {
      await shell.knowledgeIngestInit();
      const result = await shell.knowledgeIngestPrepare();
      setPrepared(result);
      setPhase("idle");
      setMessage(
        result.files.length > 0
          ? `收件箱 ${result.files.length} 份材料待整理（最多每批 5 份）。`
          : "收件箱没有可整理的材料。",
      );
    } catch (e) {
      setPhase("error");
      setMessage(String(e));
    }
  };

  const run = async () => {
    setPhase("running");
    setReport(null);
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
      });
      let status = await host.taskStatus(receipt.omni_task_id);
      for (let i = 0; i < 180 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(receipt.omni_task_id);
      }
      if (status.status !== "succeeded") {
        throw new Error(`整理任务未完成：${status.status}${status.note ? " · " + status.note : ""}`);
      }
      const result = await host.taskResult(receipt.omni_task_id);
      if (!result.content.trim()) {
        throw new Error("模型没有返回整理结果。");
      }
      setPhase("applying");
      setMessage("正在校验并落盘…");
      const applied = await shell.knowledgeIngestApply(result.content);
      setReport(applied);
      setPhase(applied.ok ? "done" : "error");
      setMessage(
        applied.ok
          ? `已建 ${applied.created.length} 张卡，索引${applied.index_updated ? "已更新" : "未更新"}。`
          : "整理结果已校验，但有卡片未通过；请看下方报告。",
      );
    } catch (e) {
      setPhase("error");
      setMessage(String(e));
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
        className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full plate-soft-inset text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container"
        data-hover-hint="把 inbox/ 里的材料交给模型整理成知识卡，并由工作台校验落盘"
      >
        <Icon name="spark" className="w-[13px] h-[13px]" />
        <span>整理收件箱</span>
      </button>
      {open ? (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-4">
          <div
            data-testid="knowledge-ingest-panel"
            className="w-full max-w-lg rounded-xl bg-surface p-4 shadow-xl ring-1 ring-outline-variant/40 space-y-3"
          >
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-[14px] font-medium text-on-surface">整理收件箱</p>
                <p className="text-[11px] text-outline">只处理 inbox/ 前 5 份材料；模型结果必须通过宿主校验。</p>
              </div>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="text-outline hover:text-on-surface"
                aria-label="关闭整理收件箱"
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

            {report ? (
              <div data-testid="knowledge-ingest-report" className="rounded-lg bg-surface-container-low p-2.5 text-[11.5px] space-y-1">
                <p>新建 {report.created.length} 张；跳过 {report.rejected_cards.length} 张；归档 {report.moved.length} 份。</p>
                {report.created.map((card) => (
                  <p key={card.relative_path} className="truncate text-on-surface-variant">✓ {card.relative_path}</p>
                ))}
                {report.rejected_cards.map((card) => (
                  <p key={card.relative_path} className="text-error">× {card.relative_path}：{card.reason}</p>
                ))}
                {report.move_errors.map((item) => (
                  <p key={item.relative_path} className="text-error">× 归档 {item.relative_path}：{item.reason}</p>
                ))}
                {report.errors.map((error) => (
                  <p key={error} className="text-error">× {error}</p>
                ))}
              </div>
            ) : null}

            <div className="flex items-center justify-end gap-2">
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
