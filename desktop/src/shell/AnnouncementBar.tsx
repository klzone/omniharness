import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { host, invalidateEngineFactsCache, pendingWrites, scheduler, shell } from "../api";
import { sameScenario, type SceneDef } from "../scenes";
import type { SchedulerConfig } from "../types";
import { PendingWritePanel } from "./PendingWriteCard";

/**
 * 提醒 / 建议栏（OD-ARCH-033 v3；OD-ARCH-046 定时与降级）。
 *
 * 两类信息**来源分开**，界面上也分开标注：
 *   * **规则提醒**：纯本地确定性计算（复用既有的周报/项目汇总命令），不花 token、
 *     不依赖模型在线；没有数据就写「暂无」，**不编造**。心跳只刷新这一部分。
 *   * **AI 复盘建议**：定时触发的**模型调用**，对本场景近期成果与对话做一次复盘。
 *
 * OD-ARCH-046 的三条硬规则：
 *   1. 复盘结果**不直接落盘**：先登记为“待确认写操作”，用户确认后才写文件。
 *   2. 模型不可用 / 超时 / 权限关闭时，只退到本地规则提醒，并明确写出降级原因；
 *      **不生成模板冒充复盘**。
 *   3. 定时由宿主的调度配置判定（宿主命令 scheduler_tick），前端运行期定时器
 *      只负责问“到点了吗”。应用关闭期间不执行；错过则下次打开补跑一次。
 */

const REVIEW_TITLE_PREFIX = "AI 复盘建议";

function isReviewArtifact(title: string): boolean {
  return title.trim().startsWith(REVIEW_TITLE_PREFIX);
}

export function AnnouncementBar({
  scene,
  itemsText,
  onOpenArtifact,
}: {
  scene: SceneDef;
  /** 本场景最近一次输入的事项文本（供确定性汇总使用）。 */
  itemsText: string;
  onOpenArtifact?: (fileName: string) => void;
}) {
  const [rule, setRule] = useState<string | null>(null);
  const [ruleError, setRuleError] = useState<string | null>(null);
  const [review, setReview] = useState<{ file: string; title: string; body: string } | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  /** 降级说明：模型不可用时必须可见，不能只留在控制台里。 */
  const [degraded, setDegraded] = useState<string | null>(null);
  /** 宿主调度配置；读不到时为 null，界面显示“不可用”，绝不假装已开启。 */
  const [config, setConfig] = useState<SchedulerConfig | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  /** 待确认写入列表的刷新键：新建记录、定时 tick 之后都要重新读取。 */
  const [pendingTick, setPendingTick] = useState(0);
  /** 宿主回报的模型权限现状：复盘既不能绕过，也不在界面上假装可用。 */
  const [admission, setAdmission] = useState<{ checking: boolean; allowed: boolean; reason: string | null }>({
    checking: true,
    allowed: false,
    reason: null,
  });

  const refreshAdmission = useCallback(async () => {
    try {
      const status = await host.admissionStatus();
      if (!status.ok || !status.facts) {
        setAdmission({ checking: false, allowed: false, reason: status.error?.message ?? "无法读取模型端点信息。" });
        return;
      }
      if (status.blocked) {
        setAdmission({
          checking: false,
          allowed: false,
          reason: status.block_reason ?? "权限变更尚未确认，宿主暂时拒绝新任务。",
        });
        return;
      }
      setAdmission({
        checking: false,
        allowed: status.facts.allowed,
        // category_reason 只说明地址类别，不能当作“为什么不能跑”的理由。
        reason: status.facts.allowed ? null : "模型权限未开启",
      });
    } catch (e) {
      setAdmission({ checking: false, allowed: false, reason: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  const refreshConfig = useCallback(async () => {
    try {
      const r = await scheduler.getConfig();
      setConfig(r.config);
      setConfigError(null);
    } catch (e) {
      setConfig(null);
      setConfigError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refreshAdmission();
    void refreshConfig();
  }, [refreshAdmission, refreshConfig, scene.scenario]);

  // 规则提醒：确定性汇总（不调模型）。心跳到点时也重跑这一条。
  const [ruleTick, setRuleTick] = useState(0);
  useEffect(() => {
    let alive = true;
    const run = async () => {
      if (!scene.summary) {
        setRule(null);
        setRuleError(null);
        return;
      }
      if (!itemsText.trim()) {
        setRule(null);
        setRuleError(null);
        return;
      }
      try {
        const r =
          scene.summary === "weekly"
            ? await shell.weeklySummary(itemsText)
            : await shell.projectSummary(itemsText, null);
        if (!alive) return;
        if (r.ok && r.markdown) {
          setRule(r.markdown.trim());
          setRuleError(null);
        } else {
          setRule(null);
          setRuleError(r.error ?? "确定性汇总没有产出内容");
        }
      } catch (e) {
        if (!alive) return;
        setRule(null);
        setRuleError(e instanceof Error ? e.message : String(e));
      }
    };
    void run();
    return () => {
      alive = false;
    };
  }, [scene.summary, itemsText, ruleTick]);

  // AI 复盘建议：读本场景的成果，取最新一条复盘（含场景子目录）。
  const loadReview = useCallback(async () => {
    try {
      const listing = await shell.offlineArtifacts(true);
      const mine = listing.items.filter((i) => !i.is_dir && sameScenario(i.scenario, scene.scenario));
      const reviews: string[] = [];
      for (const item of mine) {
        try {
          const content = await shell.readOfflineArtifact(item.relative_path);
          const title = content.content.split("\n")[0]?.replace(/^#\s*/, "") ?? "";
          if (isReviewArtifact(title)) reviews.push(item.relative_path);
        } catch {
          /* 单个文件读不到就跳过，不影响其它 */
        }
      }
      setHistory(reviews);
      if (reviews.length > 0) {
        const content = await shell.readOfflineArtifact(reviews[0]);
        const lines = content.content.split("\n");
        setReview({
          file: reviews[0],
          title: lines[0]?.replace(/^#\s*/, "") ?? "",
          body: lines.slice(1).join("\n").trim().slice(0, 400),
        });
      } else {
        setReview(null);
      }
    } catch (e) {
      setNote("读取复盘建议失败：" + (e instanceof Error ? e.message : String(e)));
    }
  }, [scene.scenario]);

  useEffect(() => {
    void loadReview();
  }, [loadReview]);

  const runReview = useCallback(async () => {
    setNote(null);
    setDegraded(null);
    if (!admission.allowed) {
      const reason = admission.reason ?? "模型权限未开启";
      setNote("定时复盘未执行：" + reason + "（复盘会调用模型，必须先到设置里开启对应类别）。");
      setDegraded("AI 暂不可用（" + reason + "），已用本地规则结果。");
      return;
    }
    setBusy(true);
    try {
      // 复盘输入只取本场景的成果与对话（都在仓库外）。
      const listing = await shell.offlineArtifacts(true);
      const mine = listing.items
        .filter((i) => !i.is_dir && sameScenario(i.scenario, scene.scenario))
        .slice(0, 5);
      const parts: string[] = [];
      for (const item of mine) {
        try {
          const content = await shell.readOfflineArtifact(item.relative_path);
          parts.push("【成果 " + item.relative_path + "】\n" + content.content.slice(0, 1200));
        } catch {
          /* 读不到就跳过 */
        }
      }
      if (itemsText.trim()) parts.push("【本场景最近一次输入】\n" + itemsText.slice(0, 2000));
      if (parts.length === 0) {
        setNote("本场景还没有可复盘的成果或输入，复盘未触发（没有调用模型）。");
        setDegraded("AI 暂不可用（没有可复盘的资料），已用本地规则结果。");
        return;
      }
      const prompt =
        "对「" + scene.label + "」做一次复盘：指出进展、卡点与下一步建议，只依据下面这些材料，" +
        "不要编造未出现的数据；用简短中文分条写。\n\n" + parts.join("\n\n");
      const prep = await shell.prepareWorkspaceDir();
      if (!prep.ok || !prep.workspace_dir) {
        setNote("复盘未提交：" + (prep.error ?? "宿主未提供工作目录"));
        setDegraded("AI 暂不可用（无法准备执行目录），已用本地规则结果。");
        return;
      }
      const receipt = await host.submitTask({ promptText: prompt, workspaceDir: prep.workspace_dir });
      // 等终态后取结果（复盘是一次普通任务，走同一条准入与权限判定）。
      let text = "";
      for (let i = 0; i < 60; i += 1) {
        await new Promise((r) => window.setTimeout(r, 2000));
        const status = await host.taskStatus(receipt.omni_task_id);
        if (["succeeded", "failed", "cancelled", "interrupted"].includes(status.status)) {
          if (status.status !== "succeeded") {
            setNote("复盘未成功（" + status.status + "），没有写入建议文件。");
            setDegraded("AI 暂不可用（任务" + status.status + "），已用本地规则结果。");
            return;
          }
          const result = await host.taskResult(receipt.omni_task_id);
          text = result.content ?? "";
          break;
        }
      }
      if (!text.trim()) {
        setNote("复盘没有返回正文，未写入建议文件。");
        setDegraded("AI 暂不可用（没有返回正文），已用本地规则结果。");
        return;
      }
      const title = REVIEW_TITLE_PREFIX + " · " + new Date().toISOString().slice(0, 10);
      // 写确认制：先登记，不落盘；用户点“确认写入”后才由宿主校验并写入。
      await pendingWrites.create({
        action: "save_offline",
        source: "scheduler",
        summary: "保存 " + scene.label + " 的「" + title + "」",
        impact:
          "确认后在成果目录的 " +
          scene.scenario.replace(/_/g, "-") +
          "/ 下新建一个 Markdown 文件；不覆盖任何已有文件。放弃则不产生任何文件。",
        payload: {
          scenario: scene.scenario,
          title,
          markdown: text,
          ai_assisted: true,
        },
      });
      setNote("AI 复盘已生成草稿，等待你确认后才会写入文件（见下方待确认卡片）。");
      // 面板挂载时列表还是空的，这里主动刷新一次，否则卡片不会出现。
      setPendingTick((n) => n + 1);
      await scheduler.markReviewed();
      await refreshConfig();
      await loadReview();
    } catch (e) {
      // 失败不写文件、不产模板；只说明降级。
      const reason = e instanceof Error ? e.message : String(e);
      setNote("复盘失败：" + reason + " 没有写入建议文件。");
      setDegraded("AI 暂不可用（" + reason + "），已用本地规则结果。");
      invalidateEngineFactsCache();
      void refreshAdmission();
    } finally {
      setBusy(false);
    }
  }, [scene, itemsText, loadReview, admission, refreshAdmission, refreshConfig]);

  // 定时与心跳：前端运行期定时器只向宿主问“到点了吗”。
  // 应用关闭期间不执行；错过由 last_*_at 判定，下一次打开补跑一次。
  const runningRef = useRef(false);
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (!alive || runningRef.current) return;
      runningRef.current = true;
      try {
        const t = await scheduler.tick();
        if (!alive) return;
        setConfig(t.config);
        setConfigError(null);
        if (t.heartbeat_due) {
          // 心跳只做本地确定性扫描：刷新规则提醒并记录时间，不调模型、不写文件。
          setRuleTick((n) => n + 1);
          await scheduler.markHeartbeat();
          setConfig((prev) => (prev ? { ...prev, last_heartbeat_at: t.now } : prev));
        }
        // 每轮 tick 顺带刷新待确认列表：定时任务可能刚登记了新记录。
        setPendingTick((n) => n + 1);
        if (t.review_due) {
          await runReview();
        }
      } catch (e) {
        if (alive) setConfigError(e instanceof Error ? e.message : String(e));
      } finally {
        runningRef.current = false;
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), 30000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [runReview]);

  const updateConfig = useCallback(
    async (patch: Partial<SchedulerConfig>) => {
      const base = config;
      if (!base) {
        setNote("调度配置不可用，改动未保存。");
        return;
      }
      const next: SchedulerConfig = { ...base, ...patch };
      try {
        const r = await scheduler.setConfig(next);
        setConfig(r.config);
        setConfigError(null);
        setNote("定时设置已保存：" + r.hint);
      } catch (e) {
        setNote("定时设置未保存：" + (e instanceof Error ? e.message : String(e)));
      }
    },
    [config],
  );

  const scheduleText = useMemo(() => {
    if (!config) return "调度配置不可用";
    if (!config.review_enabled) return "未开启";
    return config.cadence === "daily"
      ? "每天 " + String(config.hour).padStart(2, "0") + ":00"
      : "每周一 " + String(config.hour).padStart(2, "0") + ":00";
  }, [config]);

  return (
    <section className="shrink-0 mb-2 rounded-2xl bg-white/80 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-2">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-[10px] uppercase tracking-wide text-outline">规则提醒</span>
            {rule ? (
              <span className="text-[11.5px] text-on-surface-variant truncate" title={rule}>
                {rule.replace(/\s+/g, " ").slice(0, 220)}
              </span>
            ) : (
              <span className="text-[11.5px] text-outline">
                {ruleError ? "确定性汇总没有产出：" + ruleError : "暂无（按本机时间与最近输入的确定性计算，不花 token）"}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 mt-1">
            <span className="text-[10px] uppercase tracking-wide text-outline">AI 复盘建议</span>
            {review ? (
              <span className="text-[11.5px] text-on-surface-variant truncate" title={review.body}>
                {review.title}：{review.body.replace(/\s+/g, " ").slice(0, 160)}
                {onOpenArtifact ? (
                  <button
                    className="ml-1 text-primary hover:underline"
                    onClick={() => onOpenArtifact(review.file)}
                  >
                    打开
                  </button>
                ) : null}
              </span>
            ) : (
              <span className="text-[11.5px] text-outline">
                {config?.review_enabled ? "还没有复盘建议（下次复盘：" + scheduleText + "）" : "未开启（默认关闭，不调用模型）"}
              </span>
            )}
          </div>
          {history.length > 1 ? (
            <p className="text-[10px] text-outline mt-0.5">历史复盘 {history.length} 条（本场景）</p>
          ) : null}
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          <label className="flex items-center gap-1 text-[10.5px] text-on-surface-variant">
            <input
              type="checkbox"
              checked={config?.review_enabled ?? false}
              disabled={!config || !admission.allowed}
              title={
                !config
                  ? "调度配置不可用"
                  : admission.allowed
                    ? "开启后按频率自动复盘（会调用模型，结果需你确认后写入）"
                    : "模型权限未开启，自动复盘不可用：" + (admission.reason ?? "请到设置里开启")
              }
              onChange={(e) => void updateConfig({ review_enabled: e.target.checked })}
            />
            <span className={admission.allowed && config ? "" : "opacity-60"}>自动复盘</span>
          </label>
          <select
            className="text-[10.5px] px-1.5 py-1 rounded-md border border-surface-variant/30 bg-surface-container-lowest/70"
            value={(config?.cadence ?? "daily") + "@" + (config?.hour ?? 9)}
            disabled={!config}
            onChange={(e) => {
              const [cadence, hour] = e.target.value.split("@");
              void updateConfig({ cadence: cadence as SchedulerConfig["cadence"], hour: Number(hour) });
            }}
            title="复盘频率（应用运行期间触发；错过则下次打开补跑）"
          >
            <option value={"daily@" + (config?.hour ?? 9)}>每天 {config?.hour ?? 9}:00</option>
            <option value={"weekly@" + (config?.hour ?? 9)}>每周一 {config?.hour ?? 9}:00</option>
          </select>
          <button
            className="text-[10.5px] px-2 py-1 rounded-md bg-surface-container hover:bg-surface-container-high disabled:opacity-50 disabled:cursor-not-allowed"
            onClick={() => void runReview()}
            disabled={busy || !admission.allowed}
            title={
              admission.allowed
                ? "立即复盘一次（会调用一次模型；结果需你确认后写入）"
                : "不可用：" + (admission.reason ?? "模型权限未开启")
            }
          >
            {busy ? "复盘中…" : "立即复盘"}
          </button>
        </div>
      </div>
      {!admission.allowed ? (
        <p className="text-[10.5px] text-outline mt-1">
          自动复盘与「立即复盘」需要模型权限；当前未开启
          {admission.reason ? "（" + admission.reason + "）" : ""}。关闭状态下不会调用模型，规则提醒仍由本地计算。
        </p>
      ) : null}
      {configError ? <p className="text-[10.5px] text-outline mt-1">调度配置不可用：{configError}</p> : null}
      {degraded ? <p className="text-[10.5px] text-error mt-1">{degraded}</p> : null}
      {note ? <p className="text-[10.5px] text-outline mt-1">{note}</p> : null}
      <div className="mt-1.5">
        <PendingWritePanel refreshKey={pendingTick} />
      </div>
    </section>
  );
}
