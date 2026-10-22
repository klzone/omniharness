// 场景状态条（OD-ARCH-060 S1）。
//
// 三张卡，数字全部来自宿主的 `scene_overview` 命令（对已落盘的待确认记录与成果
// 元数据做计数）。这里**不**调用模型、不估算、不补零：读不到就写「未知」。

import { useState } from "react";
import type { SceneDef } from "../scenes";
import type { SceneOverview } from "../types";
import { PendingWritePanel } from "./PendingWriteCard";

function shortDate(iso: string): string {
  // 「2026-10-23」→「10-23」；形状不符时原样返回，不猜。
  return /^\d{4}-\d{2}-\d{2}$/.test(iso) ? iso.slice(5) : iso;
}

export function SceneStatusStrip({
  scene,
  overview,
  overviewError,
  refreshKey,
}: {
  scene: SceneDef;
  overview: SceneOverview | null;
  overviewError: string | null;
  refreshKey: number;
}) {
  const [showPending, setShowPending] = useState(false);
  const pending = overview?.pending ?? 0;
  const expired = overview?.pending_expired ?? 0;

  /** 第三张卡：按场景取不同的确定性周期提示。 */
  const cycle = (() => {
    if (!overview) return { title: "本期", value: "未知", hint: overviewError ?? "正在读取本机数据…" };
    if (scene.key === "weekly") {
      return {
        title: "本期工作记录",
        value: `${overview.artifacts_this_week} 份`,
        hint: `本周 ${shortDate(overview.week_start)}–${shortDate(overview.week_deadline)} · 周五截止`,
      };
    }
    if (scene.key === "project") {
      return {
        title: "本月阶段成果",
        value: `${overview.artifacts_this_month} 份`,
        hint: overview.latest_artifact_at
          ? `最近一次保存 ${new Date(overview.latest_artifact_at * 1000).toLocaleDateString("zh-CN")}`
          : "还没有保存过阶段成果",
      };
    }
    return {
      title: "本周产出",
      value: `${overview.artifacts_this_week} 份`,
      hint: `累计 ${overview.artifacts_total} 份`,
    };
  })();

  const card = "bg-white/80 border border-white/90 rounded-2xl px-3 py-2 flex-1 min-w-0";
  const label = "text-[10px] uppercase tracking-wide text-outline";

  return (
    <div className="mb-2">
      <div className="flex flex-col sm:flex-row gap-2">
        <button
          className={card + " text-left hover:bg-white disabled:cursor-default"}
          onClick={() => setShowPending((v) => !v)}
          disabled={!overview}
          title="模型或定时任务想写成果时，先登记在这里等你决定"
        >
          <div className={label}>待确认写入</div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-[17px] font-medium text-on-surface">
              {overview ? pending : "—"}
            </span>
            <span className="text-[10.5px] text-outline">{pending === 1 ? "条等你决定" : "条等你决定"}</span>
          </div>
          <div className="text-[10px] text-outline truncate">
            {overview
              ? expired > 0
                ? `其中 ${expired} 条已过期，只能放弃`
                : "点开查看（不展开时不着色）"
              : overviewError ?? "正在读取…"}
          </div>
        </button>

        <div className={card}>
          <div className={label}>逾期</div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-[17px] font-medium text-on-surface">
              {overview ? expired : "—"}
            </span>
            <span className="text-[10.5px] text-outline">条</span>
          </div>
          <div className="text-[10px] text-outline truncate">
            {overview
              ? "指的是过了 24 小时有效期、不能再确认写入的记录"
              : overviewError ?? "正在读取…"}
          </div>
        </div>

        <div className={card}>
          <div className={label}>{cycle.title}</div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-[17px] font-medium text-on-surface">{cycle.value}</span>
          </div>
          <div className="text-[10px] text-outline truncate" title={cycle.hint}>
            {cycle.hint}
          </div>
        </div>
      </div>
      {showPending ? (
        <div className="mt-2">
          <PendingWritePanel refreshKey={refreshKey} />
        </div>
      ) : null}
    </div>
  );
}
