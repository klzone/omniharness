/**
 * 周报动作行与历史归档（OD-ARCH-091 W2 / W3；OD-ARCH-092 收成卡片头动作行 + 两栏）。
 *
 * 状态判定是**确定性文件检查**：只看 库/周报/ 里有没有当周的文件，不调模型、
 * 不猜「大概写过了」。三态口径：
 *   * 未生成——当周没有周报文件；
 *   * 草稿已存在——当周有文件，但这一周还没过完（还可以继续改）；
 *   * 已归档——过去的周有文件（这一周的周报已经定稿）。
 *
 * 「已归档」是时间意义上的归档，不代表文件被冻结：宿主只读不写，改稿仍在周报流程里。
 *
 * OD-ARCH-092：状态卡不再单独占一整块——它并入时间线卡片头，只留一句状态和一个动作；
 * 历史归档与会话历史同排两栏，各自在栏内滚动，不再把整页往下推。
 */

import type { WorklogWeekFile } from "../types";
import { shortDate, weekMonth, type ReportState } from "../weekTimeline";

/** 2026-W44 → 2026 年 10 月。 */
function monthLabel(week: string): string {
  const ym = weekMonth(week);
  if (!ym) return week;
  const [y, m] = ym.split("-");
  return y + " 年 " + Number(m) + " 月";
}

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
    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[11px]">
      <span className={state === "missing" ? "text-outline" : "text-on-surface-variant"}>
        本周周报 · {state === "missing" ? "未生成" : state === "draft" ? "草稿" : "已归档"}
      </span>
      <span className="text-[10px] text-outline truncate" title={hint}>
        {hint}
      </span>
      <span className="ml-auto flex items-center gap-1.5 shrink-0">
        {latest ? (
          <button
            className="px-2 py-0.5 text-[10.5px] text-on-surface-variant hover:text-primary hover:underline"
            onClick={() => onOpenReport(latest)}
          >
            查看
          </button>
        ) : null}
        <button
          className="px-2.5 py-0.5 text-[10.5px] bg-primary text-on-primary hover:opacity-90"
          onClick={onRequestReport}
          title="回首页对话，带上本周记录起草周报"
        >
          {latest ? "重新生成" : "去生成周报"}
        </button>
      </span>
    </div>
  );
}

/**
 * 历史归档（W3）：按周列历史周报与历史周记录，点击预览。
 *
 * 按月分组，新月份在前。文件名不合约定的由宿主跳过，这里只显示跳过数量，
 * 不去猜它是什么。
 */
export function WeeklyArchive({
  reports,
  weeks,
  skipped,
  currentWeek,
  onOpenReport,
  onOpenSource,
  error,
}: {
  reports: WorklogWeekFile[];
  weeks: WorklogWeekFile[];
  skipped: number;
  currentWeek: string;
  onOpenReport: (file: WorklogWeekFile) => void;
  onOpenSource: (week: string) => void;
  error: string | null;
}) {
  const pastReports = reports.filter((f) => f.week !== currentWeek);
  const pastWeeks = weeks.filter((f) => f.week !== currentWeek);
  const groups = new Map<string, WorklogWeekFile[]>();
  for (const f of pastReports) {
    const key = monthLabel(f.week);
    const list = groups.get(key);
    if (list) list.push(f);
    else groups.set(key, [f]);
  }

  return (
    <section className="bg-white/70 border border-white/90 rounded-2xl px-4 py-3 min-w-0">
      <div className="flex items-baseline gap-2 pb-1.5">
        <h2 className="font-headline-md text-[12.5px] font-semibold text-on-surface">历史归档</h2>
        <span className="mono text-[10px] text-outline">
          {pastReports.length} 份周报 · {pastWeeks.length} 周记录
        </span>
      </div>

      {error ? (
        <p className="text-[10.5px] text-error leading-relaxed pb-1">读取归档失败：{error}</p>
      ) : null}

      {pastReports.length === 0 && pastWeeks.length === 0 ? (
        <p className="text-[10.5px] text-outline leading-relaxed">
          还没有更早的周报。本周的记录在左边，过去的周会按月份收在这里。
        </p>
      ) : (
        <div className="flex flex-col gap-2 max-h-[220px] overflow-y-auto">
          {Array.from(groups.entries()).map(([month, files]) => (
            <div key={month}>
              <div className="text-[9.5px] text-outline pb-1">{month}</div>
              <ul className="flex flex-col">
                {files.map((f) => (
                  <li key={f.file_name}>
                    <button
                      className="w-full flex items-baseline gap-2 text-left px-1.5 py-1 text-[11px] text-on-surface-variant hover:bg-surface-container-low"
                      onClick={() => onOpenReport(f)}
                      title={f.path}
                    >
                      <span className="mono text-[10px] text-outline shrink-0">{f.week}</span>
                      <span className="truncate">{f.file_name}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
          {pastWeeks.length > 0 ? (
            <div className="border-t border-surface-variant/20 pt-1.5">
              <div className="text-[9.5px] text-outline pb-1">周记录（尚未生成周报）</div>
              <ul className="flex flex-wrap gap-1">
                {pastWeeks.slice(0, 24).map((f) => (
                  <li key={f.file_name}>
                    <button
                      className="px-1.5 py-0.5 text-[10px] text-on-surface-variant bg-surface-container hover:bg-surface-container-high"
                      onClick={() => onOpenSource(f.week)}
                      title={f.path}
                    >
                      {f.week}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      )}

      {skipped > 0 ? (
        <p className="pt-1.5 text-[9.5px] text-outline">
          另有 {skipped} 个文件名不符合约定的形状，未列入、也未改动。
        </p>
      ) : null}
    </section>
  );
}

