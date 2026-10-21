import { useEffect, useRef, useState } from "react";
import { Icon } from "../components/Icon";
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
  providers: Array<{ id: string; models: string[]; baseURL: string | null }>;
}

export const EFFORT_LEVELS: Array<{ key: string; label: string }> = [
  { key: "default", label: "默认" },
  { key: "off", label: "关闭" },
  { key: "low", label: "低" },
  { key: "medium", label: "中" },
  { key: "high", label: "高" },
];

export function ModelPicker({
  currentProvider,
  currentModel,
  groups,
  catalog = null,
  effort,
  busy,
  onSelectModel,
  onSelectEffort,
}: {
  currentProvider: string | null;
  currentModel: string | null;
  groups: ModelRouteGroup[];
  /** OD-ARCH-048：web-api 通道直读的模型目录；为空或不可用时回退到 groups。 */
  catalog?: EngineModelCatalog | null;
  effort: string;
  busy: boolean;
  onSelectModel: (profile: string, provider: string, model: string) => void;
  onSelectEffort: (effort: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);
  const catalogModels = catalog?.available ? catalog.models ?? [] : [];
  /** 当前模型在目录里声明的思考等级（没有则为空，退回固定档位）。 */
  const currentEfforts =
    catalogModels.find((m) => m.provider === currentProvider && m.id === currentModel)
      ?.efforts ?? [];

  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  const label = currentProvider && currentModel ? `${currentProvider} / ${currentModel}` : "未识别";

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        disabled={busy}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full plate-soft-inset text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container transition-colors disabled:opacity-60"
        title={currentProvider ? `当前路由：${label}（可切换模型与思考等级）` : "当前端点尚未识别"}
      >
        <Icon name="spark" className="w-[13px] h-[13px]" />
        <span className="max-w-[150px] truncate">模型：{label}</span>
        <span className="text-[9px] opacity-70">{open ? "▲" : "▼"}</span>
      </button>

      {open ? (
        <div className="absolute right-0 bottom-full mb-2 w-[330px] rounded-2xl bg-white/95 backdrop-blur-sm ring-1 ring-outline-variant/30 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.25)] p-3 z-50">
          <div className="pb-2 border-b border-surface-variant/20">
            <div className="text-[10px] uppercase tracking-wide text-outline">思考等级</div>
            <div className="pt-1.5 flex flex-wrap gap-1">
              {EFFORT_LEVELS.map((lv) => (
                <button
                  key={lv.key}
                  type="button"
                  onClick={() => {
                    onSelectEffort(lv.key);
                    setOpen(false);
                  }}
                  className={
                    "px-2 py-1 rounded-full text-[11px] transition-colors " +
                    (lv.key === effort
                      ? "bg-primary text-on-primary"
                      : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
                  }
                  title="对应 DSH 的 reasoningEffort：默认＝不传该参数，由模型/Provider 决定"
                >
                  {lv.label}
                </button>
              ))}
            </div>
            <p className="pt-1.5 text-[10px] text-outline leading-relaxed">
              思考等级是请求参数：模型/Provider 不支持时可能直接报错，界面会显示真实原因。
            </p>
            {currentEfforts.length > 0 ? (
              <p className="pt-1 text-[10px] text-outline leading-relaxed">
                当前模型目录声明的等级：
                {currentEfforts.map((e) => e.name || e.id).join(" / ")}
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
                          onSelectModel(g.id, m.provider, m.id);
                          setOpen(false);
                        }}
                        className={
                          "w-full text-left px-2 py-1.5 rounded-lg text-[11.5px] transition-colors " +
                          (active
                            ? "bg-primary/10 text-primary font-medium"
                            : "text-on-surface-variant hover:bg-surface-container")
                        }
                        title={m.description || undefined}
                      >
                        <span className="block truncate">{m.name || m.id}</span>
                        <span className="block truncate text-[10px] text-outline">
                          {m.description ||
                            (m.efforts.length
                              ? "思考等级：" + m.efforts.map((e) => e.id).join(" / ")
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
                            title={p.baseURL ? "端点：" + p.baseURL : undefined}
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
          <p className="pt-2 border-t border-surface-variant/20 text-[10px] text-outline leading-relaxed">
            {catalog?.available
              ? "目录来自 DSH 本地 API 的模型目录（" + (catalog.channel || "web-api") + " 通道）。"
              : catalog && !catalog.available
                ? catalog.reason ?? "当前通道不支持直读模型目录。"
                : "候选来自各 Profile 的配置转储。"}
            切换会重启引擎（路由在引擎启动时确定），并写一份只含非机密的路由覆盖到 OmniDesk 数据根；
            不会修改你在 DSH 里的 Profile。
          </p>
        </div>
      ) : null}
    </div>
  );
}
