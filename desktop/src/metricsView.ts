/**
 * 习惯页指标（OD-GROW-301 G11）：纯函数，不读文件、不调命令。
 *
 * 输入只接受宿主已经给出的数字；任一必需数字缺失就返回 null / 空数组，
 * 由界面显示 —，不做比例估算，也不补零。
 */

export interface HabitMetricsInput {
  candidates?: {
    accepted?: number | null;
    rejected?: number | null;
    reasons?: Record<string, number>;
  };
  violations?: Record<string, { count?: number | null; lastDate?: string | null }>;
  consolidations?: { success?: number | null; fail?: number | null; tokens?: number | null };
}

export interface HabitMetricsData {
  acceptRate: number | null;
  rejectionTop: Array<{ code: string; count: number }>;
  violationTrend: number[];
  consolidationSuccessRate: number | null;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function percent(part: number, total: number): number | null {
  if (total <= 0) return null;
  return Math.round((part / total) * 1000) / 10;
}

/** 接受率 = 接受 /（接受 + 驳回），单位 %；两边任一缺失或分母为 0 时返回 null。 */
function acceptRate(candidates: HabitMetricsInput["candidates"]): number | null {
  const accepted = finiteNumber(candidates?.accepted);
  const rejected = finiteNumber(candidates?.rejected);
  if (accepted === null || rejected === null) return null;
  return percent(accepted, accepted + rejected);
}

/** 驳回原因按次数降序取前三；次数相同按原因码升序，保证结果稳定。 */
export function rejectionTop(
  reasons: Record<string, number> | undefined,
): HabitMetricsData["rejectionTop"] {
  const top = Object.entries(reasons ?? {})
    .map(([code, count]) => ({ code, count: finiteNumber(count) }))
    .filter((entry): entry is { code: string; count: number } =>
      entry.count !== null && entry.count > 0,
    )
    .sort((a, b) => b.count - a.count || a.code.localeCompare(b.code));
  return top.slice(0, 3);
}

/**
 * 违反趋势：按卡片最近违反日期合并计数，取最近 7 个日期升序。
 * 没有日期或计数的卡片不参与，空结果返回 []（界面显示 —）。
 */
function violationTrend(violations: HabitMetricsInput["violations"]): number[] {
  const byDate = new Map<string, number>();
  for (const entry of Object.values(violations ?? {})) {
    const date = typeof entry?.lastDate === "string" ? entry.lastDate.trim() : "";
    const count = finiteNumber(entry?.count);
    if (date.length === 0 || count === null) continue;
    byDate.set(date, (byDate.get(date) ?? 0) + count);
  }
  return [...byDate.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .slice(-7)
    .map(([, count]) => count);
}

/** 整理成功率 = 成功 /（成功 + 失败），单位 %；缺失或分母为 0 时返回 null。 */
function consolidationSuccessRate(
  consolidations: HabitMetricsInput["consolidations"],
): number | null {
  const success = finiteNumber(consolidations?.success);
  const fail = finiteNumber(consolidations?.fail);
  if (success === null || fail === null) return null;
  return percent(success, success + fail);
}

export function buildMetrics(input: HabitMetricsInput): HabitMetricsData {
  return {
    acceptRate: acceptRate(input.candidates),
    rejectionTop: rejectionTop(input.candidates?.reasons),
    violationTrend: violationTrend(input.violations),
    consolidationSuccessRate: consolidationSuccessRate(input.consolidations),
  };
}

const REJECTION_LABELS: Record<string, string> = {
  too_absolute: "太绝对",
  not_mine: "不是我说的",
};

function trimmedNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

/** 数值缺失显示 —；有值时按单位格式化，不做四舍五入以外的修饰。 */
export function formatMetric(value: number | null, unit = ""): string {
  return value === null || !Number.isFinite(value) ? "—" : trimmedNumber(value) + unit;
}

/** 驳回原因 Top3 显示为「太绝对 × 3」；没有数据时显示 —。 */
export function formatRejectionTop(top: HabitMetricsData["rejectionTop"]): string {
  if (top.length === 0) return "—";
  return top
    .map((entry) => `${REJECTION_LABELS[entry.code] ?? entry.code} × ${entry.count}`)
    .join("、");
}

/** 违反趋势显示为「1 → 2 → 0」；没有数据时显示 —。 */
export function formatViolationTrend(trend: number[]): string {
  return trend.length === 0 ? "—" : trend.join(" → ");
}
