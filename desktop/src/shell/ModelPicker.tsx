import { useEffect, useRef, useState } from "react";
import { Icon } from "../components/Icon";
import {
  DEFAULT_EFFORT_KEY,
  effortLabelFor,
  modelEffortOptions,
  resolveModelEffortDeclaration,
} from "../modelEffortEvidence";
import type { EngineModelCatalog } from "../types";

/**
 * 模型选择器（OD-ARCH-035）：与工作台同一套视觉的**自绘**下拉，不用原生 select。
 *
 * 两件事：
 *   1. **模型**：列出各 DSH Profile 里已配置的 provider/模型（不同 Profile 的模型选择互不相通，
 *      只有凭据共享），选中即切换并重启引擎。
 *   2. **思考等级**：对应 DSH 的 `reasoningEffort`（默认 / 关闭 / 低 / 中 / 高）；
 *      「默认」= 不传该参数，由模型或 Provider 自己决定。
 *
 * 诚实性：思考等级是**请求参数**，模型/Provider 不支持时可能被拒（界面会显示真实原因）。
 */
export interface ModelRouteGroup {
  profile: string;
  default: { provider: string | null; model: string | null } | null;
  providers: Array<{
    id: string;
    models: string[];
    baseURL: string | null;
    modelEntries?: Array<Record<string, unknown>>;
  }>;
}

export function ModelPicker({
  currentProvider,
  currentModel,
  groups,
  catalog = null,
  engineProfile = "",
  effort,
  busy,
  loading = false,
  onSelectModel,
  onSelectEffort,
}: {
  currentProvider: string | null;
  currentModel: string | null;
  groups: ModelRouteGroup[];
  /** OD-ARCH-048：web-api 通道直读的模型目录；为空或不可用时回退到 groups。 */
  catalog?: EngineModelCatalog | null;
  /**
   * 引擎当前实际加载的 Profile（OD-ARCH-095 S02）。
   * 目录分组的 id 是 provider，不是 Profile——用它写覆盖文件会让 DSH 以
   * 「profile 不存在」失败。目录分支统一回落到这个真实 Profile。
   */
  engineProfile?: string;
  effort: string;
  busy: boolean;
  /** OD-ARCH-050：首次读取事实期间显示「读取中」，而不是「未识别」。 */
  loading?: boolean;
  onSelectModel: (profile: string, provider: string, model: string) => void;
  onSelectEffort: (effort: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);
  const catalogModels = catalog?.available ? catalog.models ?? [] : [];
  /**
   * OD-ARCH-178 阶段 C：档位按钮由当前模型（provider+model）的声明驱动。
   * 证据解析与提交侧共用 modelEffortEvidence；目录非空为准，目录未声明时才看
   * Profile 原始声明。两者都没有时只渲染「跟随模型默认」。
   */
  const declaration = resolveModelEffortDeclaration({
    catalog,
    routes: groups,
    sourceProfile: engineProfile,
    provider: currentProvider,
    model: currentModel,
  });
  const effortOptions = modelEffortOptions(declaration.levels);
  const activeEffort = effortOptions.some((option) => option.key === effort)
    ? effort
    : DEFAULT_EFFORT_KEY;
  // 只在「用户还带着一个当前模型不支持的档位」时提示一句；未声明且已是默认时
  // 不再堆描述文字，单按钮本身已经说明问题。
  const effortAdvisory =
    effort !== DEFAULT_EFFORT_KEY && !declaration.levels.includes(effort)
      ? declaration.levels.length === 0
        ? "该模型未声明思考档位，已跟随模型默认。"
        : "该模型不支持「" + effortLabelFor(effort) + "」，已跟随模型默认。"
      : null;

  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  const label =
    currentProvider && currentModel
      ? `${currentProvider} / ${currentModel}`
      : loading
        ? "读取中…"
        : "未识别";

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        disabled={busy}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md plate-soft-inset text-on-surface font-body-sm text-[12px] font-medium tracking-normal transition-colors disabled:opacity-60"
        data-hover-hint={currentProvider ? `当前路由：${label}（可切换模型与思考等级）` : "当前端点尚未识别"}
      >
        <Icon name="spark" className="w-[13px] h-[13px]" />
        <span className="max-w-[190px] truncate">模型：{label}</span>
        <span className="text-[9px] opacity-70">{open ? "▲" : "▼"}</span>
      </button>

      {open ? (
        <div className="neu-popover absolute right-0 bottom-full mb-2 w-[248px] rounded-2xl p-3 z-50">
          <div className="pb-2 border-b border-surface-variant/20">
            <div className="text-[10px] uppercase tracking-wide text-outline">思考等级</div>
            <div className="pt-1.5 flex flex-wrap gap-1">
              {effortOptions.map((lv) => (
                <button
                  key={lv.key}
                  type="button"
                  onClick={() => {
                    onSelectEffort(lv.key);
                    setOpen(false);
                  }}
                  className={
                    "neu-effort-btn px-3 py-1.5 rounded-md text-[11.5px] font-medium " +
                    (lv.key === activeEffort ? "neu-effort-active" : "")
                  }
                  aria-pressed={lv.key === activeEffort}
                  data-testid={"effort-option-" + lv.key}
                  data-hover-hint="对应 DSH 的 reasoningEffort：跟随模型默认＝不传该参数，由模型/Provider 决定"
                >
                  {lv.label}
                </button>
              ))}
            </div>
            {effortAdvisory ? (
              <p
                className="pt-1.5 text-[10px] text-outline leading-relaxed"
                data-testid="effort-advisory"
              >
                {effortAdvisory}
              </p>
            ) : null}
          </div>

          <div className="pt-2 max-h-[240px] overflow-y-auto">
            {catalogModels.length > 0 ? (
              (catalog?.groups ?? []).map((g) => (
                <div key={g.id} className="pb-2">
                  <div className="text-[10px] uppercase tracking-wide text-outline px-1 pt-1">
                    {g.name || g.id}
                  </div>
                  {g.models.map((m) => {
                    const active = m.provider === currentProvider && m.id === currentModel;
                    return (
                      <button
                        key={g.id + "|" + m.id}
                        type="button"
                        onClick={() => {
                          onSelectModel(engineProfile, m.provider, m.id);
                          setOpen(false);
                        }}
                        className={
                          "w-full text-left px-2 py-1.5 rounded-lg text-[12px] transition-colors " +
                          (active
                            ? "bg-primary/10 text-primary font-medium"
                            : "text-on-surface-variant hover:bg-surface-container")
                        }
                        data-hover-hint={m.description || undefined}
                      >
                        <span className="block truncate">{m.name || m.id}</span>
                        <span className="block truncate text-[10px] text-outline">
                          {m.description ||
                            ((m.efforts?.length ?? 0) > 0
                              ? "思考等级：" + (m.efforts ?? []).map((e) => e.id).join(" / ")
                              : "未声明思考等级")}
                        </span>
                      </button>
                    );
                  })}
                </div>
              ))
            ) : groups.length === 0 ? (
              <p className="text-[11px] text-outline">没有读到任何模型候选（DSH 配置转储未成功）。</p>
            ) : (
              groups.map((g) => (
                <div key={g.profile} className="pb-2">
                  <div className="text-[10px] uppercase tracking-wide text-outline px-1 pt-1">
                    {g.profile}
                    {g.default?.provider ? ` · 默认 ${g.default.provider}/${g.default.model ?? "?"}` : ""}
                  </div>
                  {g.providers.length === 0 ? (
                    <p className="px-1 text-[11px] text-outline">该 Profile 没有自定义 provider。</p>
                  ) : (
                    g.providers.flatMap((p) =>
                      p.models.map((m) => {
                        const active = p.id === currentProvider && m === currentModel;
                        return (
                          <button
                            key={g.profile + "|" + p.id + "|" + m}
                            type="button"
                            onClick={() => {
                              onSelectModel(g.profile, p.id, m);
                              setOpen(false);
                            }}
                            className={
                              "w-full text-left px-2 py-1.5 rounded-lg text-[11.5px] transition-colors " +
                              (active
                                ? "bg-primary/10 text-primary font-medium"
                                : "text-on-surface-variant hover:bg-surface-container")
                            }
                            data-hover-hint={p.baseURL ? "端点：" + p.baseURL : undefined}
                          >
                            <span className="block truncate">
                              {p.id} / {m}
                            </span>
                            <span className="block truncate text-[10px] text-outline">
                              {p.baseURL ?? "（未声明端点）"}
                            </span>
                          </button>
                        );
                      }),
                    )
                  )}
                </div>
              ))
            )}
          </div>
          {catalog && !catalog.available ? (
            <p className="pt-2 border-t border-surface-variant/20 text-[10px] text-outline leading-relaxed">
              {catalog.reason ?? "当前通道不支持直读模型目录。"}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
