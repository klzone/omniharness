import { useCallback, useEffect, useMemo, useState } from "react";

import { consolidation, shell, habits, type ConsolidationConfig } from "../api";
import type { HabitCard, ModelRouteListing } from "../types";
import { Icon } from "../components/Icon";
import {
  HABITS_CHANGED_EVENT,
  HABITS_OPEN_EVENT,
  localDateFromUtcIso,
  notifyHabitsChanged,
  systemUtcOffsetMinutes,
} from "../habitNotice";
import { scopeLabel } from "../habitCardsView";
import { HABIT_LAST_INJECTED_KEY } from "../habitInjection";
import { buildMetrics, formatMetric, formatRejectionTop, formatViolationTrend } from "../metricsView";
import { HabitDigestPanel } from "./HabitDigestPanel";

const CARD = "wr-side-card";

function sourceLabel(sources: string[]): string {
  return sources
    .map((source) =>
      source === "plain_threads"
        ? "未分类对话"
        : source === "project_tasks"
          ? "项目任务"
          : source,
    )
    .join("、");
}

/**
 * 习惯页（OD-GROW-301 G5）：候选处理、生效卡与每日整理开关放在同一浮层。
 *
 * 不改应用路由结构：顶栏按钮或首页提醒打开本浮层；数据全部来自宿主命令。
 * 命中与违反次数宿主尚未提供读取命令，界面只显示最近出现时间，并如实标注 G11。
 */
export function HabitCardsPage({ onClose }: { onClose: () => void }) {
  const [active, setActive] = useState<HabitCard[]>([]);
  const [config, setConfig] = useState<ConsolidationConfig | null>(null);
  const [routes, setRoutes] = useState<ModelRouteListing | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [listing, saved, routeList] = await Promise.all([
        habits.active(),
        consolidation.getConfig(),
        shell.listModelRoutes().catch(() => null),
      ]);
      setActive(listing.items);
      setConfig(saved);
      setRoutes(routeList);
      setError(listing.broken > 0 ? `${listing.broken} 个习惯文件无法读取` : null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const refresh = () => void load();
    window.addEventListener(HABITS_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(HABITS_CHANGED_EVENT, refresh);
  }, [load]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const providers = useMemo(() => {
    const merged = new Map<string, ModelRouteListing["profiles"][number]["providers"][number]>();
    for (const profile of routes?.profiles ?? []) {
      for (const provider of profile.providers) {
        if (!merged.has(provider.id)) merged.set(provider.id, provider);
      }
    }
    return [...merged.values()];
  }, [routes]);
  const models =
    providers.find((provider) => provider.id === config?.provider)?.models ?? [];
  const metrics = useMemo(() => buildMetrics({}), []);
  const injectedRows = useMemo(() => {
    try {
      return (localStorage.getItem(HABIT_LAST_INJECTED_KEY) ?? "").split("\n").filter(Boolean).length;
    } catch {
      return 0;
    }
  }, [loading]);

  async function updateCard(id: string, action: "disable" | "archive" | "reuse") {
    setBusy(true);
    setError(null);
    try {
      await habits.updateCard(id, action);
      notifyHabitsChanged();
      await load();
    } catch (e) {
      setError("更新习惯卡失败：" + String(e));
    } finally {
      setBusy(false);
    }
  }

  async function saveConfig() {
    if (!config) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await consolidation.setConfig(config);
      setConfig(saved);
      notifyHabitsChanged();
    } catch (e) {
      setError("保存整理设置失败：" + String(e));
    } finally {
      setBusy(false);
    }
  }

  function patchConfig(patch: Partial<ConsolidationConfig>) {
    setConfig((state) => (state ? { ...state, ...patch } : state));
  }

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-[rgba(40,35,30,0.32)] p-4 backdrop-blur-[2px]"
      role="dialog"
      aria-modal="true"
      aria-label="习惯"
      data-habit-workspace=""
    >
      <div className="wr-dialog flex max-h-[86vh] w-[min(1180px,96vw)] flex-col overflow-hidden">
        <header className="wr-dialog-head">
          <span className="wr-dialog-mark" aria-hidden="true">
            <Icon name="spark" className="h-4 w-4" />
          </span>
          <h1 className="text-[15px] font-semibold text-on-surface">习惯</h1>
          {loading ? <span className="ml-2 text-[10.5px] text-outline">读取中…</span> : null}
          <button
            type="button"
            className="wr-icon-button ml-auto"
            aria-label="关闭"
            onClick={onClose}
          >
            <Icon name="x" className="h-4 w-4" />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          {error ? (
            <p className="mb-3 rounded-xl bg-error-container/50 px-3 py-2 text-[11px] leading-relaxed text-on-error-container">
              {error}
            </p>
          ) : null}
          <div className="grid grid-cols-1 items-start gap-4 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
            <div className="flex min-w-0 flex-col gap-4">
              <HabitDigestPanel />
            </div>

            <div className="flex min-w-0 flex-col gap-4">
              <section className={CARD} data-habit-cards="">
                <div className="flex items-baseline gap-2">
                  <h2 className="wr-side-title">生效卡</h2>
                  <span className="mono ml-auto text-[11px] text-outline">{active.length}</span>
                </div>
                <p className="mt-1 text-[10.5px] text-outline">命中与违反次数待 G11 接入统计读取。</p>
                <div className="mt-3 flex flex-col gap-2">
                  {active.length === 0 ? (
                    <p className="wr-empty-state">还没有生效卡。接受候选后出现在这里。</p>
                  ) : (
                    active.map((card) => (
                      <article key={card.id} className="wr-task-row !items-start" data-habit-card={card.id}>
                        <div className="min-w-0 flex-1">
                          <p className="text-[12px] font-medium leading-relaxed text-on-surface">{card.text}</p>
                          <p className="mt-1 text-[10.5px] text-outline">
                            {scopeLabel(card.scope)}
                            {" · "}
                            {card.status === "active" ? "生效" : "停用"}
                            {" · 最近出现 "}
                            {localDateFromUtcIso(card.last_seen_at, systemUtcOffsetMinutes())}
                          </p>
                        </div>
                        <div className="flex shrink-0 flex-col gap-1">
                          {card.status === "active" ? (
                            <button
                              type="button"
                              className="wr-text-button"
                              disabled={busy}
                              onClick={() => void updateCard(card.id, "disable")}
                            >
                              停用
                            </button>
                          ) : (
                            <button
                              type="button"
                              className="wr-ghost-button"
                              disabled={busy}
                              onClick={() => void updateCard(card.id, "reuse")}
                            >
                              复用
                            </button>
                          )}
                          <button
                            type="button"
                            className="wr-text-button"
                            disabled={busy}
                            onClick={() => void updateCard(card.id, "archive")}
                          >
                            归档
                          </button>
                        </div>
                      </article>
                    ))
                  )}
                </div>
              </section>

              {config ? (
                <section className={CARD} data-habit-consolidation="">
                  <div className="flex items-center gap-2">
                    <h2 className="wr-side-title">每日整理</h2>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={config.enabled}
                      aria-label="每日整理"
                      onClick={() => patchConfig({ enabled: !config.enabled })}
                      className={
                        "ml-auto h-5 w-9 rounded-full p-0.5 transition-colors " +
                        (config.enabled ? "bg-[var(--od-neu-accent)]" : "bg-[var(--od-neu-sunken)]")
                      }
                    >
                      <span
                        className={
                          "block h-4 w-4 rounded-full bg-white shadow-sm transition-transform " +
                          (config.enabled ? "translate-x-4" : "")
                        }
                      />
                    </button>
                  </div>

                  {config.enabled ? (
                    <div className="mt-3 flex flex-col gap-2">
                      <p className="text-[10.5px] text-on-surface-variant">
                        将读取{sourceLabel(config.sources)}。
                      </p>
                      <div className="flex items-center gap-2">
                        <label className="text-[11px] text-outline" htmlFor="habit-hour">
                          时间
                        </label>
                        <input
                          id="habit-hour"
                          type="number"
                          min={0}
                          max={23}
                          value={config.hour}
                          onChange={(event) => patchConfig({ hour: Number(event.target.value) })}
                          className="w-14 rounded-lg border-0 bg-[var(--od-neu-sunken)] px-2 py-1 text-[11px] text-on-surface shadow-[var(--od-neu-inset-1)]"
                        />
                        <span className="text-[11px] text-outline">:</span>
                        <input
                          type="number"
                          min={0}
                          max={59}
                          value={config.minute}
                          onChange={(event) => patchConfig({ minute: Number(event.target.value) })}
                          aria-label="分钟"
                          className="w-14 rounded-lg border-0 bg-[var(--od-neu-sunken)] px-2 py-1 text-[11px] text-on-surface shadow-[var(--od-neu-inset-1)]"
                        />
                      </div>
                      <div className="flex flex-wrap items-center gap-2">
                        <select
                          aria-label="服务商"
                          value={config.provider ?? ""}
                          onChange={(event) =>
                            patchConfig({ provider: event.target.value || null, model: null })
                          }
                          className="min-w-0 flex-1 rounded-lg border-0 bg-[var(--od-neu-sunken)] px-2 py-1 text-[11px] text-on-surface shadow-[var(--od-neu-inset-1)]"
                        >
                          <option value="">跟随当前路由</option>
                          {providers.map((provider) => (
                            <option key={provider.id} value={provider.id}>
                              {provider.displayName || provider.id}
                            </option>
                          ))}
                          {config.provider &&
                          !providers.some((provider) => provider.id === config.provider) ? (
                            <option value={config.provider}>{config.provider}</option>
                          ) : null}
                        </select>
                        <select
                          aria-label="模型"
                          value={config.model ?? ""}
                          onChange={(event) => patchConfig({ model: event.target.value || null })}
                          className="min-w-0 flex-1 rounded-lg border-0 bg-[var(--od-neu-sunken)] px-2 py-1 text-[11px] text-on-surface shadow-[var(--od-neu-inset-1)]"
                        >
                          <option value="">跟随当前路由</option>
                          {models.map((model) => (
                            <option key={model} value={model}>
                              {model}
                            </option>
                          ))}
                        </select>
                      </div>
                    </div>
                  ) : null}

                  <button
                    type="button"
                    className="wr-primary-button mt-3"
                    disabled={busy}
                    onClick={() => void saveConfig()}
                  >
                    保存
                  </button>
                </section>
              ) : null}
            </div>
          </div>

          {/* OD-GROW-301 G11：只读指标区；宿主未提供统计读取命令时如实显示缺失。 */}
          <section className={CARD + " mt-4"} data-habit-metrics="">
            <h2 className="wr-side-title">指标</h2>
            <dl className="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-[10.5px]">
              <dt className="text-outline">接受率</dt>
              <dd className="text-on-surface">{formatMetric(metrics.acceptRate, "%")}</dd>
              <dt className="text-outline">驳回原因 Top3</dt>
              <dd className="text-on-surface">{formatRejectionTop(metrics.rejectionTop)}</dd>
              <dt className="text-outline">违反趋势（最近 7 次）</dt>
              <dd className="text-on-surface">{formatViolationTrend(metrics.violationTrend)}</dd>
              <dt className="text-outline">整理成功率</dt>
              <dd className="text-on-surface">{formatMetric(metrics.consolidationSuccessRate, "%")}</dd>
            </dl>
            <p className="mt-1 text-[10px] leading-relaxed text-outline">
              {injectedRows > 0 ? `最近注入 ${injectedRows} 次（本地记录）` : ""}
              统计读取待 G11 宿主命令；缺失显示 —。
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}

/** 顶栏与首页提醒共用的打开入口；监听不看路由，关掉浮层即回到原页面。 */
export function HabitsOverlayHost() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const onOpen = () => setOpen(true);
    window.addEventListener(HABITS_OPEN_EVENT, onOpen);
    return () => window.removeEventListener(HABITS_OPEN_EVENT, onOpen);
  }, []);
  return open ? <HabitCardsPage onClose={() => setOpen(false)} /> : null;
}

