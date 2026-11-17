// 模型思考档位的单一目录口径（OD-ARCH-178 阶段 B+C）。
//
// 同一份证据同时供三处使用：
//   1. ModelPicker 渲染按钮（只显示当前模型声明过的档位）；
//   2. HomeChat 提交前的证据门（未声明一律不传 reasoningEffort）；
//   3. api.ts 的最后一道防线（调用方漏传证据时静默降级为 null）。
//
// 纯函数、无依赖：界面与离线验证脚本执行同一份源码。

import type { EngineModelCatalog } from "./types";

/**
 * 档位证据（与 OD-ARCH-176 的 `WorkIntentEffortEvidence` 同形）。
 * 目录优先；目录没有给出非空声明时再看 Profile 原始声明。
 */
export interface WorkIntentEffortEvidence {
  /** 引擎目录声明的档位；空数组/缺省表示目录没有给出声明。 */
  catalogLevels?: string[] | null;
  /**
   * Profile 原始 `reasoningEfforts`：缺省 = 未声明；`false` = 明确关闭档位；
   * 非空数组 = 声明的档位集合。`false` 和空集合不能当作拒绝 `off` 的证据。
   */
  profileLevels?: string[] | false | null;
}

/**
 * 证据是否明确声明「不支持 requested」。未声明、目录不可用、空集合与
 * `reasoningEfforts: false` 都保持原判断，不构成反对证据。
 */
export function effortEvidenceExcludes(
  evidence: WorkIntentEffortEvidence | null | undefined,
  requested: string,
): boolean {
  const catalogLevels = evidence?.catalogLevels;
  if (Array.isArray(catalogLevels) && catalogLevels.length > 0) {
    return !catalogLevels.includes(requested);
  }
  const profileLevels = evidence?.profileLevels;
  if (Array.isArray(profileLevels) && profileLevels.length > 0) {
    return !profileLevels.includes(requested);
  }
  return false;
}

/**
 * 证据是否明确声明「支持 requested」。未声明 / 目录不可用一律视为不支持——
 * 真实 DSH 的 session/selectModel 会拒绝未声明模型的任何显式档位。
 */
export function effortEvidenceIncludes(
  evidence: WorkIntentEffortEvidence | null | undefined,
  requested: string,
): boolean {
  const catalogLevels = evidence?.catalogLevels;
  if (Array.isArray(catalogLevels) && catalogLevels.length > 0) {
    return catalogLevels.includes(requested);
  }
  const profileLevels = evidence?.profileLevels;
  if (Array.isArray(profileLevels) && profileLevels.length > 0) {
    return profileLevels.includes(requested);
  }
  return false;
}

/** 已知档位的中文标签；未识别的声明档位原样显示 id。 */
const EFFORT_LABELS: Record<string, string> = {
  off: "关闭",
  minimal: "最低",
  low: "低",
  medium: "中",
  high: "高",
  xhigh: "很高",
  max: "最高",
};

export const DEFAULT_EFFORT_KEY = "default";
export const DEFAULT_EFFORT_LABEL = "跟随模型默认";

export function effortLabelFor(effort: string | null | undefined): string {
  const key = (effort ?? "").trim();
  if (!key) return DEFAULT_EFFORT_LABEL;
  return EFFORT_LABELS[key] ?? key;
}

export interface ModelEffortOption {
  key: string;
  label: string;
}

/**
 * 选择器按钮：档位只来自声明集合（OD-ARCH-181 微调）。
 * 声明非空时不再提供「跟随模型默认」——模型档位已由用户在设置页逐模型
 * 手动声明，选择器直接给出可用的档位；声明为空时保留单按钮作兜底，
 * 避免选择器出现空档位区。
 */
export function modelEffortOptions(
  levels: readonly string[] | null | undefined,
): ModelEffortOption[] {
  const seen = new Set<string>();
  const declared: ModelEffortOption[] = [];
  for (const raw of levels ?? []) {
    const key = (raw ?? "").trim();
    if (!key || key === DEFAULT_EFFORT_KEY || seen.has(key)) continue;
    seen.add(key);
    declared.push({ key, label: effortLabelFor(key) });
  }
  if (declared.length === 0) {
    return [{ key: DEFAULT_EFFORT_KEY, label: DEFAULT_EFFORT_LABEL }];
  }
  return declared;
}

export interface ModelEffortDeclaration {
  /** 可用于界面与提交门禁的档位集合。 */
  levels: string[];
  /** 证据来源：目录优先；目录没有非空声明时才看 Profile。 */
  source: "catalog" | "profile" | null;
  catalogLevels: string[] | null;
  profileLevels: string[] | false | null;
}

/** 只读的证据来源：Profile 标识与各 provider 的非机密模型条目。 */
export interface ModelRouteEvidenceSource {
  profile: string;
  providers: Array<{
    id: string;
    modelEntries?: Array<Record<string, unknown>>;
  }>;
}

/**
 * 从已加载的模型目录与 Profile 原始声明解析当前模型的档位集合。
 *
 * 两层顺序与 OD-ARCH-175/176 的证据门完全一致：目录非空即为准；目录为空或
 * 缺省时才读 Profile 的 `modelEntries.reasoningEfforts`。两处都没有声明时
 * 返回空集合——界面只显示「跟随模型默认」，提交侧不传档位。
 */
export function resolveModelEffortDeclaration(input: {
  catalog?: EngineModelCatalog | null;
  routes?: readonly ModelRouteEvidenceSource[] | null;
  sourceProfile?: string | null;
  provider?: string | null;
  model?: string | null;
}): ModelEffortDeclaration {
  const provider = (input.provider ?? "").trim();
  const model = (input.model ?? "").trim();
  if (!provider || !model) {
    return { levels: [], source: null, catalogLevels: null, profileLevels: null };
  }

  const catalogEntry =
    input.catalog?.available === true
      ? (input.catalog.models ?? []).find(
          (m) => m.provider === provider && m.id === model,
        )
      : undefined;
  const catalogLevels = Array.isArray(catalogEntry?.efforts)
    ? catalogEntry.efforts.map((e) => e.id)
    : null;

  const profile = input.routes?.find((p) => p.profile === input.sourceProfile);
  const sourceProvider = profile?.providers.find((p) => p.id === provider);
  const modelEntry = sourceProvider?.modelEntries?.find(
    (e) => e && typeof e === "object" && e.id === model,
  );
  const declared = modelEntry?.reasoningEfforts;
  let profileLevels: string[] | false | null = null;
  if (declared === false) {
    profileLevels = false;
  } else if (declared && typeof declared === "object" && !Array.isArray(declared)) {
    profileLevels = Object.keys(declared as Record<string, unknown>);
  }

  if (catalogLevels && catalogLevels.length > 0) {
    return { levels: catalogLevels, source: "catalog", catalogLevels, profileLevels };
  }
  if (Array.isArray(profileLevels) && profileLevels.length > 0) {
    return { levels: profileLevels, source: "profile", catalogLevels, profileLevels };
  }
  return { levels: [], source: null, catalogLevels, profileLevels };
}

/**
 * 生成 OD-ARCH-176 同形的两层证据；目录给出非空声明时不再看 Profile，
 * 与旧实现保持相同的短路顺序。
 */
export function effortEvidenceFromLoaded(input: {
  catalog?: EngineModelCatalog | null;
  routes?: readonly ModelRouteEvidenceSource[] | null;
  sourceProfile?: string | null;
  provider?: string | null;
  model?: string | null;
}): WorkIntentEffortEvidence {
  const declaration = resolveModelEffortDeclaration(input);
  return {
    catalogLevels: declaration.catalogLevels,
    profileLevels:
      declaration.catalogLevels && declaration.catalogLevels.length > 0
        ? null
        : declaration.profileLevels,
  };
}

/**
 * 提交前的档位裁决：只有证据明确支持才下发，否则静默返回 null。
 * 未声明、目录不可用、Profile 明确关闭或声明集合不含该档位都属于这种情况。
 */
export function gatedReasoningEffort(
  requested: string | null | undefined,
  evidence: WorkIntentEffortEvidence | null | undefined,
): string | null {
  const effort = (requested ?? "").trim();
  if (!effort || effort === DEFAULT_EFFORT_KEY) return null;
  return effortEvidenceIncludes(evidence, effort) ? effort : null;
}
