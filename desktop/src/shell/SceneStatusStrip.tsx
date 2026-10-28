// 场景指标带（OD-ARCH-060 S1；OD-ARCH-092 由三张等宽卡改为一行紧凑指标）。
//
// 数字全部来自宿主 scene_overview 命令（对已落盘的待确认记录与成果元数据做计数），
// 或宿主只读的工作记录解析（本周条目数、周报状态）。这里**不**调用模型、不估算、
// 不补零：读不到就写「未知」。
//
// 版式：四张独立卡片（OD-ARCH-093 三次修订，2026-10-27）：整排略窄于顶部导航栏、
// 与导航留出空隙、卡与卡之间留白；每张卡一组 OmniDesk 色调（紫 / 青 / 靛 / 琥珀，
// 逾期时转玫红），卡内是方形图标座 + 中文标题 + 等宽英文副标题 + 大号数字 + 说明。
// 待确认写入平时是 0 条（复盘移除后没有别的写入入口），因此**有记录才出现**。
// 2026-10-27 用户第五次反馈：入口并入第四张「逾期」卡，取消四卡下方的独立触发条。

import { useEffect, useState, type ReactNode } from "react";
import { Icon, type IconName } from "../components/Icon";
import type { SceneDef } from "../scenes";
import type { SceneOverview } from "../types";
import { PendingWritePanel } from "./PendingWriteCard";
import { relativeUpdated } from "../writingStudio";

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
        <span className="text-[10px] text-outline truncate" data-hover-hint={hint}>
          {hint}
        </span>
        {action ? <span className="ml-auto shrink-0">{action}</span> : null}
      </div>
    </div>
  );
}

/**
 * 四张指标卡各自的色调（OD-ARCH-093 三次修订）：全部落在 OmniDesk 既有色系里——
 * 主色紫、辅助青、第三色靛，再补一枚与逾期告警同温的琥珀。底色只做很浅的渐变，
 * 让四张卡一眼可分，又不至于比下面的内容区更抢眼。
 */
type BandTone = "violet" | "cyan" | "indigo" | "amber";

const BAND_TONES: Record<
  BandTone,
  {
    shell: string;
    chip: string;
    glow: string;
    subtitle: string;
    number: string;
    track: string;
    bar: string;
  }
> = {
  violet: {
    shell: "border-primary/15 bg-[linear-gradient(158deg,#f4f1ff_0%,#faf8ff_62%,#ffffff_100%)]",
    chip: "text-primary",
    glow: "bg-primary/15",
    subtitle: "text-primary/55",
    number: "text-primary",
    track: "bg-primary/10",
    bar: "bg-primary/85",
  },
  cyan: {
    shell: "border-secondary/15 bg-[linear-gradient(158deg,#eafafc_0%,#f7fdfe_62%,#ffffff_100%)]",
    chip: "text-secondary",
    glow: "bg-secondary/15",
    subtitle: "text-secondary/60",
    number: "text-secondary",
    track: "bg-secondary/10",
    bar: "bg-secondary/85",
  },
  indigo: {
    shell: "border-tertiary/15 bg-[linear-gradient(158deg,#eef0ff_0%,#f8f8ff_62%,#ffffff_100%)]",
    chip: "text-tertiary",
    glow: "bg-tertiary/15",
    subtitle: "text-tertiary/55",
    number: "text-tertiary",
    track: "bg-tertiary/10",
    bar: "bg-tertiary/85",
  },
  amber: {
    shell: "border-amber-500/15 bg-[linear-gradient(158deg,#fff8eb_0%,#fffcf5_62%,#ffffff_100%)]",
    chip: "text-amber-600",
    glow: "bg-amber-400/15",
    subtitle: "text-amber-700/55",
    number: "text-amber-600",
    track: "bg-amber-500/10",
    bar: "bg-amber-500/80",
  },
};

/** 逾期真的存在时整张卡切成告警色，盖掉它原本的琥珀底。 */
const BAND_ALERT = {
  shell: "border-error/20 bg-[linear-gradient(158deg,#fff0ef_0%,#fffafa_64%,#ffffff_100%)]",
  chip: "text-error",
  glow: "bg-error/10",
  subtitle: "text-error/55",
  number: "text-error",
  track: "bg-error/10",
  bar: "bg-error/80",
};

/**
 * 指标带一格（OD-ARCH-099 S01）：左侧图标独占一列，右侧内容列自上而下放标题、
 * 英文副标题与序号、数字与单位、底行提示与进度轨。图标下方不再叠任何文字，
 * 四张卡等高，扫读时先认色相再读数值。图标不带底色座，仅用色相文字着色。
 * 进度条只在分子分母都真实存在时绘制，否则整条隐藏——不拿 0 或满格假装有数据。
 */
function BandMetric({
  icon,
  index,
  label,
  subtitle,
  value,
  unit,
  hint,
  ratio,
  tone = "normal",
  color = "violet",
  action,
}: {
  icon: IconName;
  /** 卡序号（01–04）：做成右上角等宽小标，给整排一个编辑式节奏。 */
  index: string;
  label: string;
  subtitle?: string;
  value: string;
  unit?: string;
  hint: string;
  /** 0~1 的完成比例；null/undefined 表示没有可比的分子分母，不画进度。 */
  ratio?: number | null;
  tone?: "normal" | "alert";
  color?: BandTone;
  action?: ReactNode;
}) {
  const width = typeof ratio === "number" && Number.isFinite(ratio) ? Math.max(0, Math.min(1, ratio)) : null;
  const skin = tone === "alert" ? BAND_ALERT : BAND_TONES[color];
  /*
   * 数值字号随长度收缩：状态值可能是「未生成」这种三字短语，固定 30px 会把左侧
   * 标题和英文副标题挤成省略号。数字卡保持大字号，短状态字降两档仍然醒目。
   */
  const valueSize =
    value.length >= 4
      ? "text-[20px] leading-[1.05]"
      : value.length === 3
        ? "text-[24px] leading-none"
        : "text-[30px] leading-none";
  return (
    <article
      role="group"
      aria-label={`${label}${subtitle ? " " + subtitle : ""}：${value}${unit ? " " + unit : ""}，${hint}`}
      data-band-card={color}
      className={
        "relative overflow-hidden min-w-0 rounded-2xl border px-4 py-3 " +
        "flex items-stretch gap-3.5 shadow-[0_8px_22px_-18px_rgba(35,32,80,0.5)] " + skin.shell
      }
    >
      <span
        aria-hidden="true"
        className={"pointer-events-none absolute -right-8 -top-10 h-24 w-24 rounded-full blur-2xl " + skin.glow}
      />
      <div
        data-band-icon-column
        className="relative shrink-0 w-14 flex flex-col items-center justify-start"
      >
        <Icon name={icon} className={"w-14 h-14 " + skin.chip} />
      </div>
      <div className="relative flex-1 min-w-0 flex flex-col">
        <div className="flex items-start justify-between gap-3 min-w-0">
          <div className="min-w-0 flex-1">
            <div
              className="text-[13px] font-semibold leading-tight tracking-tight text-on-surface truncate"
              data-hover-hint={label}
            >
              {label}
            </div>
            <div className="mt-1 flex items-baseline gap-1.5 min-w-0">
              <span className="mono shrink-0 text-[12px] tracking-[0.12em] text-on-surface-variant/40">
                {index}
              </span>
              {subtitle ? (
                <span
                  data-hover-hint={subtitle}
                  className={
                    "mono text-[12px] uppercase tracking-[0.12em] truncate " + skin.subtitle
                  }
                >
                  {subtitle}
                </span>
              ) : null}
            </div>
          </div>
          <div className="shrink-0 flex items-baseline justify-end gap-1 text-right pl-1">
            <span
              className={
                "font-headline-lg font-semibold tracking-tight tabular-nums whitespace-nowrap " +
                valueSize +
                " " +
                skin.number
              }
            >
              {value}
            </span>
            {unit ? (
              <span className="text-[12px] text-on-surface-variant/70">{unit}</span>
            ) : null}
          </div>
        </div>
        <div className="relative mt-auto pt-2 flex items-center gap-2 min-w-0">
          <span className="text-[12px] text-on-surface-variant/80 leading-snug truncate" data-hover-hint={hint}>
            {hint}
          </span>
          {action ? <span className="ml-auto shrink-0">{action}</span> : null}
        </div>
        {width === null ? null : (
          <div className={"relative mt-1.5 h-[4px] w-full rounded-full overflow-hidden " + skin.track}>
            <div
              className={"h-full rounded-full " + skin.bar}
              style={{ width: `${Math.round(width * 100)}%` }}
            />
          </div>
        )}
      </div>
    </article>
  );
}


/**
 * 写作页专属指标卡（OD-ARCH-094 S01）。
 *
 * 与工作记录页的四色卡刻意区分：这里统一暖纸底 + 墨色字，只用一条左侧色标和
 * 顶部极细的印刷线做层次——像稿纸，而不是仪表盘。数字仍然横向右置，卡片尺寸与
 * 093 一致（整排略窄于导航、卡间留白），保证两个页面是同一个产品。
 */
function WritingMetric({
  icon,
  index,
  label,
  subtitle,
  value,
  unit,
  hint,
  accent,
  action,
}: {
  icon: IconName;
  index: string;
  label: string;
  subtitle?: string;
  value: string;
  unit?: string;
  hint: string;
  accent: "ink" | "teal" | "ochre" | "stone";
  action?: ReactNode;
}) {
  const skin = {
    ink: { rule: "bg-on-surface", dot: "text-on-surface", chip: "border-on-surface/25" },
    teal: { rule: "bg-secondary", dot: "text-secondary", chip: "border-secondary/30" },
    ochre: { rule: "bg-amber-500", dot: "text-amber-700", chip: "border-amber-500/35" },
    stone: {
      rule: "bg-on-surface-variant/40",
      dot: "text-on-surface-variant",
      chip: "border-on-surface-variant/25",
    },
  }[accent];
  const valueSize =
    value.length >= 4
      ? "text-[20px] leading-[1.05]"
      : value.length === 3
        ? "text-[24px] leading-none"
        : "text-[30px] leading-none";
  return (
    <article
      role="group"
      aria-label={`${label}${subtitle ? " " + subtitle : ""}：${value}${unit ? " " + unit : ""}，${hint}`}
      data-writing-metric={accent}
      className={
        "relative overflow-hidden min-w-0 rounded-2xl border border-on-surface/8 " +
        "bg-[linear-gradient(168deg,#fbf9f4_0%,#fdfcf9_58%,#ffffff_100%)] " +
        "px-4 py-3 flex flex-col shadow-[0_10px_24px_-22px_rgba(35,32,26,0.55)]"
      }
    >
      <span aria-hidden="true" className={"absolute left-0 top-0 h-full w-[3px] " + skin.rule} />
      <span aria-hidden="true" className="absolute left-0 top-0 h-px w-full bg-on-surface/10" />
      <div className="relative flex items-start justify-between gap-3 min-w-0 pl-1.5">
        <div className="flex items-center gap-2.5 min-w-0 flex-1">
          <span
            className={
              "shrink-0 w-8 h-8 rounded-lg border bg-white/70 flex items-center justify-center " + skin.chip
            }
          >
            <Icon name={icon} className={"w-[16px] h-[16px] " + skin.dot} />
          </span>
          <div className="min-w-0 flex-1">
            <div className="text-[13px] font-semibold leading-tight tracking-tight text-on-surface truncate" data-hover-hint={label}>
              {label}
            </div>
            <div className="mt-1 flex items-baseline gap-1.5 min-w-0">
              <span className="mono shrink-0 text-[8.5px] tracking-[0.12em] text-on-surface-variant/40">
                {index}
              </span>
              {subtitle ? (
                <span
                  data-hover-hint={subtitle}
                  className="mono text-[8.5px] uppercase tracking-[0.12em] text-on-surface-variant/55 truncate"
                >
                  {subtitle}
                </span>
              ) : null}
            </div>
          </div>
        </div>
        <div className="shrink-0 flex items-baseline justify-end gap-1 text-right">
          <span className={"font-headline-lg font-semibold tracking-tight tabular-nums whitespace-nowrap text-on-surface " + valueSize}>
            {value}
          </span>
          {unit ? <span className="text-[10px] text-on-surface-variant/70">{unit}</span> : null}
        </div>
      </div>
      <div className="relative mt-auto pt-2 flex items-center gap-2 min-w-0 pl-1.5">
        <span className="text-[10.5px] text-on-surface-variant/80 leading-snug truncate" data-hover-hint={hint}>
          {hint}
        </span>
        {action ? <span className="ml-auto shrink-0">{action}</span> : null}
      </div>
    </article>
  );
}

export function SceneStatusStrip({
  scene,
  overview,
  overviewError,
  refreshKey,
  weekEntries,
  weekReportEntries,
  reportLabel,
  onRequestReport,
  writingDraftCount = null,
  writingInspirationCount = null,
  writingNow = Math.floor(Date.now() / 1000),
  onRequestWriting,
}: {
  scene: SceneDef;
  overview: SceneOverview | null;
  overviewError: string | null;
  refreshKey: number;
  /** 本周工作记录条数（来自时间线解析，比成果文件数更贴近「我记了几条」）。 */
  weekEntries: number | null;
  /** 其中去向为「本周报引用」的条数（周报实际会采用的条目）。 */
  weekReportEntries: number | null;
  /** 本周周报状态文案（确定性文件检查的结果）。 */
  reportLabel: string | null;
  /** 去生成 / 重新生成周报（只预填输入框，不自动发送）。 */
  onRequestReport?: () => void;
  /** 写作页：进行中草稿数（来自已读取的成果元数据，未读到给 null）。 */
  writingDraftCount?: number | null;
  /** 写作页：灵感架条目数（既有场景会话数，空场景也是真实 0）。 */
  writingInspirationCount?: number | null;
  /** 写作页「最近更新」的相对时间基准（Unix 秒）。 */
  writingNow?: number;
  /** 去首页开一段新的写作对话（只跳转，不自动发送）。 */
  onRequestWriting?: () => void;
}) {
  const [showPending, setShowPending] = useState(false);
  const pending = overview?.pending ?? 0;
  const expired = overview?.pending_expired ?? 0;
  const waiting = pending + expired;

  useEffect(() => {
    if (!showPending) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setShowPending(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [showPending]);

  const grid =
    "mb-2 bg-white/80 border border-white/90 rounded-2xl " +
    "shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] py-2.5 " +
    "flex flex-col sm:flex-row sm:items-stretch gap-y-2 sm:gap-y-0";
  const divider = "sm:border-l sm:border-surface-variant/70";

  /**
   * 指标带（OD-ARCH-093 三次修订）：四张各自独立的卡，整排比顶部导航（max-w-7xl）
   * 略窄，居中并与导航留出空隙；卡与卡之间同样留白。窄窗两列、手机单列，
   * 不用分隔线把四张卡硬连成一条。
   *
   * `md:-translate-x-8`：主区左内边距（md:pl-28）比导航外壳（sm:px-10）多 32px，
   * 只在主区里 mx-auto 会让整排看起来偏右。位移 32px 后与导航同轴，左右留白等宽。
   */
  const band =
    "mt-4 mb-4 max-w-6xl mx-auto md:-translate-x-8 grid grid-cols-1 items-stretch " +
    "gap-3 sm:grid-cols-2 sm:gap-4 xl:grid-cols-4";

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
      : waiting > 0
        ? `没有逾期 · ${waiting} 条待确认写入`
        : "没有过期或待确认记录"
    : overviewError ?? "正在读取…";

  /** 进周报的比例：只在本周条数与进周报条数都真实读到、且条数大于 0 时成立。 */
  const reportShare =
    weekEntries !== null && weekReportEntries !== null && weekEntries > 0
      ? weekReportEntries / weekEntries
      : null;

  return (
    <div className="mb-2">
      <div className={scene.key === "weekly" || scene.key === "writing" ? band : grid}>
        {scene.key === "weekly" ? (
          <>
            <BandMetric
              icon="note"
              index="01"
              color="violet"
              label="本期工作记录"
              subtitle={scene.subtitles?.records}
              value={weekEntries === null ? "—" : String(weekEntries)}
              unit={weekEntries === null ? undefined : "条"}
              hint={weekEntries === null ? "正在读取本周记录…" : cycleHint}
            />
            <BandMetric
              icon="checklist"
              index="02"
              color="cyan"
              label="进周报条目"
              subtitle={scene.subtitles?.weekly}
              value={weekReportEntries === null ? "—" : String(weekReportEntries)}
              unit={weekReportEntries === null ? undefined : "条"}
              hint={
                weekEntries === null
                  ? "正在读取本周记录…"
                  : weekEntries === 0
                    ? "本周还没有记录"
                    : `占全部记录的 ${Math.round((reportShare ?? 0) * 100)}%`
              }
              ratio={reportShare}
            />
            <BandMetric
              icon="clock"
              index="03"
              color="indigo"
              label="本周周报"
              subtitle={scene.subtitles?.report}
              value={reportLabel ?? "—"}
              hint={
                reportLabel === null
                  ? "正在检查周报目录…"
                  : reportLabel === "未生成"
                    ? "说「整理成周报」才会生成"
                    : "可到右栏成果库查看"
              }
              action={
                onRequestReport ? (
                  <button
                    className="text-[12px] text-primary hover:underline"
                    onClick={onRequestReport}
                    title="回首页对话，带上本周记录起草周报"
                  >
                    {reportLabel === "未生成" ? "去生成" : "重新生成"}
                  </button>
                ) : undefined
              }
            />
            <BandMetric
              icon="trend-up"
              index="04"
              color="amber"
              label="逾期"
              subtitle={scene.subtitles?.overdue}
              value={overview ? String(expired) : "—"}
              unit={overview ? "条" : undefined}
              hint={expiredHint}
              tone={expired > 0 ? "alert" : "normal"}
              action={
                waiting > 0 ? (
                  <button
                    type="button"
                    className="inline-flex items-center gap-1 rounded-md border border-primary/20 bg-white/70 px-1.5 py-0.5 text-[12px] font-medium text-primary hover:border-primary/40 hover:bg-primary/5"
                    onClick={() => setShowPending(true)}
                    aria-expanded={showPending}
                    aria-haspopup="dialog"
                    title="模型或定时任务想写成果时，先登记在这里等你决定"
                  >
                    <Icon name="shield-check" className="h-3.5 w-3.5" />
                    待确认 {waiting}
                  </button>
                ) : undefined
              }
            />
          </>
        ) : scene.key === "writing" ? (
          <>
            <WritingMetric
              icon="book"
              index="01"
              accent="ink"
              label="本月成文"
              subtitle={scene.subtitles?.finished ?? scene.subtitles?.overview}
              value={overview ? String(overview.artifacts_this_month) : "—"}
              unit={overview ? "份" : undefined}
              hint={
                overview
                  ? overview.artifacts_this_month === 0
                    ? "本月还没有保存的写作成果"
                    : "本自然月保存的写作成果"
                  : overviewError ?? "正在读取本机数据…"
              }
            />
            <WritingMetric
              icon="edit"
              index="02"
              accent="teal"
              label="进行中草稿"
              subtitle={scene.subtitles?.draft}
              value={writingDraftCount === null ? "—" : String(writingDraftCount)}
              unit={writingDraftCount === null ? undefined : "份"}
              hint={
                writingDraftCount === null
                  ? "正在读取成果元数据…"
                  : writingDraftCount === 0
                    ? "没有标为草稿的成果"
                    : "frontmatter 标为草稿的成果"
              }
            />
            <WritingMetric
              icon="spark"
              index="03"
              accent="ochre"
              label="灵感待用"
              subtitle={scene.subtitles?.inspiration}
              value={writingInspirationCount === null ? "—" : String(writingInspirationCount)}
              unit={writingInspirationCount === null ? undefined : "段"}
              hint={
                writingInspirationCount === null
                  ? "正在读取对话记录…"
                  : writingInspirationCount === 0
                    ? "灵感架还空着，去聊一句就存住了"
                    : "本场景最近的对话，可随时接着写"
              }
              action={
                onRequestWriting ? (
                  <button className="text-[10px] text-secondary hover:underline" onClick={onRequestWriting}>
                    去写
                  </button>
                ) : undefined
              }
            />
            <WritingMetric
              icon="clock"
              index="04"
              accent="stone"
              label="最近更新"
              subtitle={scene.subtitles?.recent}
              value={relativeUpdated(overview?.latest_artifact_at ?? null, writingNow)}
              hint={
                overview?.latest_artifact_at
                  ? "最近一份成果的修改时间"
                  : overview
                    ? "还没有保存过写作成果"
                    : overviewError ?? "正在读取本机数据…"
              }
            />
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

      </div>

      {showPending && waiting > 0 ? (
        <div
          className="fixed inset-0 z-[80] flex items-center justify-center bg-on-surface/25 px-4 py-6 backdrop-blur-[2px]"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setShowPending(false);
          }}
        >
          <section
            className="flex max-h-[80vh] w-[80vw] max-w-3xl flex-col overflow-hidden rounded-3xl border border-white/90 bg-surface-container-lowest shadow-[0_24px_80px_-28px_rgba(15,23,42,0.45)]"
            role="dialog"
            aria-modal="true"
            aria-labelledby="pending-write-dialog-title"
          >
            <div className="flex items-start justify-between gap-4 border-b border-surface-variant/25 px-5 py-4">
              <div className="min-w-0">
                <h2 id="pending-write-dialog-title" className="text-[17px] font-semibold tracking-tight text-on-surface">
                  待确认写入
                </h2>
                <p className="mt-1 text-[12px] text-on-surface-variant">
                  逐条确认后才会写入成果目录，已过期记录只能放弃。
                </p>
              </div>
              <button
                type="button"
                className="shrink-0 rounded-full bg-surface-container px-3 py-1.5 text-[12px] text-on-surface-variant hover:bg-surface-container-high"
                onClick={() => setShowPending(false)}
              >
                关闭
              </button>
            </div>
            <div className="min-h-0 overflow-y-auto px-5 py-4">
              <PendingWritePanel refreshKey={refreshKey} />
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}

