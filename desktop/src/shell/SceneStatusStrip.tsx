// 场景指标带（OD-ARCH-060 S1；OD-ARCH-092 由三张等宽卡改为一行紧凑指标）。
//
// 数字全部来自宿主 scene_overview 命令（对已落盘的待确认记录与成果元数据做计数），
// 或宿主只读的工作记录解析（本周条目数、周报状态）。这里**不**调用模型、不估算、
// 不补零：读不到就写「未知」。
//
// 版式：一行「小标题 + 数字 + 一句灰字」，项之间用细分隔线断开，异常项才着色。
// 待确认写入平时是 0 条（复盘移除后没有别的写入入口），因此**有记录才出现**，
// 不再常驻占一整格。

import { useState, type ReactNode } from "react";
import type { SceneDef } from "../scenes";
import type { SceneOverview } from "../types";
import { PendingWritePanel } from "./PendingWriteCard";

function shortDate(iso: string): string {
  // 「2026-10-23」→「10-23」；形状不符时原样返回，不猜。
  return /^\d{4}-\d{2}-\d{2}$/.test(iso) ? iso.slice(5) : iso;
}

/** 一格指标：小标题（可带一个动作）、数值、一句说明。 */
function Metric({
  label,
  value,
  unit,
  hint,
  tone = "normal",
  action,
}: {
  label: string;
  value: string;
  unit?: string;
  hint: string;
  tone?: "normal" | "alert";
  action?: ReactNode;
}) {
  return (
    <div className="h-full px-4">
      <div className="text-[10px] uppercase tracking-wide text-outline">{label}</div>
      <div className="flex items-baseline gap-1 mt-0.5">
        <span
          className={
            "text-[15px] font-semibold leading-tight " +
            (tone === "alert" ? "text-error" : "text-on-surface")
          }
        >
          {value}
        </span>
        {unit ? <span className="text-[10.5px] text-outline">{unit}</span> : null}
      </div>
      {/* 动作放在说明行右端：贴在上方标签行会和右邻列的标题挤在一起。 */}
      <div className="flex items-baseline gap-2">
        <span className="text-[10px] text-outline truncate" title={hint}>
          {hint}
        </span>
        {action ? <span className="ml-auto shrink-0">{action}</span> : null}
      </div>
    </div>
  );
}

export function SceneStatusStrip({
  scene,
  overview,
  overviewError,
  refreshKey,
  weekEntries,
  reportLabel,
  onRequestReport,
}: {
  scene: SceneDef;
  overview: SceneOverview | null;
  overviewError: string | null;
  refreshKey: number;
  /** 本周工作记录条数（来自时间线解析，比成果文件数更贴近「我记了几条」）。 */
  weekEntries: number | null;
  /** 本周周报状态文案（确定性文件检查的结果）。 */
  reportLabel: string | null;
  /** 去生成 / 重新生成周报（只预填输入框，不自动发送）。 */
  onRequestReport?: () => void;
}) {
  const [showPending, setShowPending] = useState(false);
  const pending = overview?.pending ?? 0;
  const expired = overview?.pending_expired ?? 0;
  const waiting = pending + expired;

  const grid =
    "mb-2 bg-white/80 border border-white/90 rounded-2xl " +
    "shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] py-2.5 " +
    "flex flex-col sm:flex-row sm:items-stretch gap-y-2 sm:gap-y-0";
  const divider = "sm:border-l sm:border-surface-variant/70";

  /** weekly 与其它场景各取自己的确定性口径，语义不和别的场景混着用。 */
  const cycleHint = (() => {
    if (!overview) return overviewError ?? "正在读取本机数据…";
    if (scene.key === "weekly") {
      return "本周 " + shortDate(overview.week_start) + "–" + shortDate(overview.week_deadline);
    }
    if (scene.key === "project") {
      return overview.latest_artifact_at
        ? "最近保存 " + new Date(overview.latest_artifact_at * 1000).toLocaleDateString("zh-CN")
        : "还没有保存过阶段成果";
    }
    return "累计 " + overview.artifacts_total + " 份";
  })();

  const expiredHint = overview
    ? expired > 0
      ? "过了 24 小时有效期，只能放弃"
      : "没有过期记录"
    : overviewError ?? "正在读取…";

  return (
    <div className="mb-2">
      <div className={grid}>
        {scene.key === "weekly" ? (
          <>
            {/* 每一项都包一层 flex-1：Metric 自己是 flex 子项，不给外层等宽的话
                第一项会把剩余宽度全吃掉，右侧几项挤在一起。 */}
            <div className="flex-1 min-w-0">
              <Metric
                label="本期工作记录"
                value={weekEntries === null ? "—" : String(weekEntries)}
                unit={weekEntries === null ? undefined : "条"}
                hint={weekEntries === null ? "正在读取本周记录…" : cycleHint}
              />
            </div>
            <div className={divider + " flex-1 min-w-0"}>
              <Metric
                label="本周周报"
                value={reportLabel ?? "—"}
                hint={
                  reportLabel === null
                    ? "正在检查周报目录…"
                    : reportLabel === "未生成"
                      ? "说「整理成周报」才会生成"
                      : "可到右侧成果库查看"
                }
                action={
                  onRequestReport ? (
                    <button
                      className="text-[10px] text-primary hover:underline"
                      onClick={onRequestReport}
                      title="回首页对话，带上本周记录起草周报"
                    >
                      {reportLabel === "未生成" ? "去生成" : "重新生成"}
                    </button>
                  ) : undefined
                }
              />
            </div>
            <div className={divider + " flex-1 min-w-0"}>
              <Metric
                label="逾期"
                value={overview ? String(expired) : "—"}
                unit="条"
                hint={expiredHint}
                tone={expired > 0 ? "alert" : "normal"}
              />
            </div>
          </>
        ) : (
          <>
            <div className="flex-1 min-w-0">
              <Metric
                label={scene.key === "project" ? "本月阶段成果" : "本周产出"}
                value={
                  overview
                    ? String(
                        scene.key === "project"
                          ? overview.artifacts_this_month
                          : overview.artifacts_this_week,
                      )
                    : "—"
                }
                unit={overview ? "份" : undefined}
                hint={cycleHint}
              />
            </div>
            <div className={divider + " flex-1 min-w-0"}>
              <Metric
                label="逾期"
                value={overview ? String(expired) : "—"}
                unit="条"
                hint={expiredHint}
                tone={expired > 0 ? "alert" : "normal"}
              />
            </div>
          </>
        )}

        {/* 待确认写入：没有记录就整格不出现，避免一个永远为 0 的常驻模块。 */}
        {waiting > 0 ? (
          <div className={divider + " flex-1 min-w-0"}>
            <button
              className="w-full text-left px-3"
              onClick={() => setShowPending((v) => !v)}
              aria-expanded={showPending}
              title="模型或定时任务想写成果时，先登记在这里等你决定"
            >
              <div className="flex items-baseline gap-1">
                <span className="text-[10px] uppercase tracking-wide text-outline">待确认写入</span>
                <span className="ml-auto shrink-0 text-[10px] text-outline">
                  {showPending ? "收起" : "点开"}
                </span>
              </div>
              <div className="flex items-baseline gap-1 mt-0.5">
                <span
                  className={
                    "text-[15px] font-semibold leading-tight " +
                    (expired > 0 ? "text-error" : "text-on-surface")
                  }
                >
                  {waiting}
                </span>
                <span className="text-[10.5px] text-outline">条</span>
              </div>
              <div className="text-[10px] text-outline truncate">
                {expired > 0 ? "其中 " + expired + " 条已过期" : "等你决定"}
              </div>
            </button>
          </div>
        ) : null}
      </div>
      {showPending && waiting > 0 ? (
        <div className="mb-2">
          <PendingWritePanel refreshKey={refreshKey} />
        </div>
      ) : null}
    </div>
  );
}

