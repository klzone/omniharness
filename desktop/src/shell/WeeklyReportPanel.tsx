/**
 * 周报动作行（OD-ARCH-091 W2；OD-ARCH-092 收成卡片头动作行）。
 *
 * 状态判定是**确定性文件检查**：只看 库/周报/ 里有没有当周的文件，不调模型、
 * 不猜「大概写过了」。三态口径：
 *   * 未生成——当周没有周报文件；
 *   * 草稿已存在——当周有文件，但这一周还没过完（还可以继续改）；
 *   * 已归档——过去的周有文件（这一周的周报已经定稿）。
 *
 * 「已归档」是时间意义上的归档，不代表文件被冻结：宿主只读不写，改稿仍在周报流程里。
 *
 * OD-ARCH-092：状态卡不再单独占一整块——它并入时间线卡片头，只留一句状态和一个动作。
 * OD-ARCH-093 修订（2026-10-27）：历史归档不再单独成卡，改为并入右栏成果文档库，
 * 由 `ArtifactExplorer` 统一渲染并加「归档」徽标。
 */

import type { WorklogWeekFile } from "../types";
import { shortDate, type ReportState } from "../weekTimeline";

const STATE_HINT: Record<ReportState, string> = {
  missing: "这周还没有周报文件",
  draft: "草稿已存在，可继续改",
  archived: "本周已归档",
};

/**
 * 周报动作行：放进时间线卡片头，替代原来那块独立状态卡。
 * 只有状态、说明和一个主按钮，不再重复大标题。
 */
export function WeeklyReportStatus({
  state,
  files,
  error,
  onRequestReport,
  onOpenReport,
}: {
  state: ReportState;
  files: WorklogWeekFile[];
  error: string | null;
  onRequestReport: () => void;
  onOpenReport: (file: WorklogWeekFile) => void;
}) {
  const latest = files[0] ?? null;
  const hint = error
    ? "读取周报目录失败：" + error
    : STATE_HINT[state] +
      (latest
        ? " · 最近修改 " +
          shortDate(new Date(latest.modified_at * 1000).toISOString().slice(0, 10))
        : "");

  return (
    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[12px]">
      <span className={state === "missing" ? "text-outline" : "text-on-surface-variant"}>
        本周周报 · {state === "missing" ? "未生成" : state === "draft" ? "草稿" : "已归档"}
      </span>
      <span className="text-[12px] text-outline truncate" title={hint}>
        {hint}
      </span>
      <span className="ml-auto flex items-center gap-1.5 shrink-0">
        {latest ? (
          <button
            className="px-2 py-0.5 text-[12px] text-on-surface-variant hover:text-primary hover:underline"
            onClick={() => onOpenReport(latest)}
          >
            查看
          </button>
        ) : null}
        <button
          className="px-2.5 py-0.5 text-[12px] bg-primary text-on-primary hover:opacity-90"
          onClick={onRequestReport}
          title="回首页对话，带上本周记录起草周报"
        >
          {latest ? "重新生成" : "去生成周报"}
        </button>
      </span>
    </div>
  );
}
