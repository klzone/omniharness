import { useCallback, useEffect, useMemo, useState } from "react";
import { host, shell } from "../api";
import type { SceneDef } from "../scenes";

/**
 * 提醒 / 建议栏（OD-ARCH-033 v3）。
 *
 * 两类信息**来源分开**，界面上也分开标注：
 *   * **规则提醒**：纯本地确定性计算（复用既有的周报/项目汇总命令），不花 token、
 *     不依赖模型在线；没有数据就写「暂无」，**不编造**。
 *   * **AI 复盘建议**：定时触发的**模型调用**，对本场景近期成果与对话做一次复盘，
 *     结论存成本场景的一份成果文件；**默认关闭**，每次复盘前先说明「会调用一次模型」。
 *
 * 边界：
 *   * 复盘输入只取**本场景**的成果与对话记录（都在仓库外数据根 / 本机前端存储里）。
 *   * 「趋势热点」需要外部数据源，第一版**不做**，也不假装有。
 *   * 权限照旧由宿主按三档设置判定；没开权限时提交会被拒，界面如实显示原因。
 */

const SETTINGS_KEY = "omnidesk.scenes.review.v1";
const REVIEW_TITLE_PREFIX = "AI 复盘建议";

interface ReviewSettings {
  enabled: boolean;
  cadence: "daily" | "weekly";
  hour: number;
  lastRunAt: number;
}

const DEFAULT_SETTINGS: ReviewSettings = { enabled: false, cadence: "daily", hour: 9, lastRunAt: 0 };

function loadSettings(): ReviewSettings {
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<ReviewSettings>) };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

function saveSettings(next: ReviewSettings) {
  try {
    window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
  } catch {
    /* 存不下只是不持久化 */
  }
}

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
  const [settings, setSettings] = useState<ReviewSettings>(loadSettings);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  // 规则提醒：确定性汇总（不调模型）。
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
  }, [scene.summary, itemsText]);

  // AI 复盘建议：读本场景的成果，取最新一条复盘。
  const loadReview = useCallback(async () => {
    try {
      const listing = await shell.offlineArtifacts();
      const mine = listing.items.filter((i) => i.scenario === scene.scenario);
      const reviews: string[] = [];
      for (const item of mine) {
        try {
          const content = await shell.readOfflineArtifact(item.file_name);
          const title = content.content.split("\n")[0]?.replace(/^#\s*/, "") ?? "";
          if (isReviewArtifact(title)) reviews.push(item.file_name);
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

  // 定时复盘：应用运行期间由前端定时器触发；错过则在下次打开时补跑一次。
  useEffect(() => {
    if (!settings.enabled) return;
    const due = () => {
      const now = new Date();
      if (now.getHours() < settings.hour) return false;
      const last = settings.lastRunAt ? new Date(settings.lastRunAt) : null;
      if (!last) return true;
      const days = Math.floor((now.getTime() - last.getTime()) / 86400000);
      if (settings.cadence === "daily") return days >= 1;
      return days >= 7;
    };
    if (due()) void runReview();
    const timer = window.setInterval(() => {
      if (due()) void runReview();
    }, 60000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.enabled, settings.cadence, settings.hour, itemsText]);

  const runReview = useCallback(async () => {
    setBusy(true);
    setNote(null);
    try {
      // 复盘输入只取本场景的成果与对话（都在仓库外）。
      const listing = await shell.offlineArtifacts();
      const mine = listing.items.filter((i) => i.scenario === scene.scenario).slice(0, 5);
      const parts: string[] = [];
      for (const item of mine) {
        try {
          const content = await shell.readOfflineArtifact(item.file_name);
          parts.push("【成果 " + item.file_name + "】\n" + content.content.slice(0, 1200));
        } catch {
          /* 读不到就跳过 */
        }
      }
      if (itemsText.trim()) parts.push("【本场景最近一次输入】\n" + itemsText.slice(0, 2000));
      if (parts.length === 0) {
        setNote("本场景还没有可复盘的成果或输入，复盘未触发（没有调用模型）。");
        return;
      }
      const prompt =
        "对「" + scene.label + "」做一次复盘：指出进展、卡点与下一步建议，只依据下面这些材料，" +
        "不要编造未出现的数据；用简短中文分条写。\n\n" + parts.join("\n\n");
      const prep = await shell.prepareWorkspaceDir();
      if (!prep.ok || !prep.workspace_dir) {
        setNote("复盘未提交：" + (prep.error ?? "宿主未提供工作目录"));
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
            return;
          }
          const result = await host.taskResult(receipt.omni_task_id);
          text = result.content ?? "";
          break;
        }
      }
      if (!text.trim()) {
        setNote("复盘没有返回正文，未写入建议文件。");
        return;
      }
      const title = REVIEW_TITLE_PREFIX + " · " + new Date().toISOString().slice(0, 10);
      await shell.saveOffline({
        scenario: scene.scenario,
        title,
        markdown: text,
        userConfirmed: true,
        aiAssisted: true,
      });
      const next = { ...settings, lastRunAt: Date.now() };
      setSettings(next);
      saveSettings(next);
      await loadReview();
    } catch (e) {
      setNote("复盘失败：" + (e instanceof Error ? e.message : String(e)));
    } finally {
      setBusy(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scene, itemsText, settings, loadReview]);

  const scheduleText = useMemo(
    () =>
      settings.cadence === "daily"
        ? "每天 " + String(settings.hour).padStart(2, "0") + ":00"
        : "每周一 " + String(settings.hour).padStart(2, "0") + ":00",
    [settings.cadence, settings.hour],
  );

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
                {settings.enabled ? "还没有复盘建议（下次复盘：" + scheduleText + "）" : "未开启（默认关闭，不调用模型）"}
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
              checked={settings.enabled}
              onChange={(e) => {
                const next = { ...settings, enabled: e.target.checked };
                setSettings(next);
                saveSettings(next);
              }}
            />
            自动复盘
          </label>
          <select
            className="text-[10.5px] px-1.5 py-1 rounded-md border border-surface-variant/30 bg-surface-container-lowest/70"
            value={settings.cadence + "@" + settings.hour}
            onChange={(e) => {
              const [cadence, hour] = e.target.value.split("@");
              const next = {
                ...settings,
                cadence: cadence as ReviewSettings["cadence"],
                hour: Number(hour),
              };
              setSettings(next);
              saveSettings(next);
            }}
            title="复盘频率（应用运行期间触发；错过则下次打开补跑）"
          >
            <option value={"daily@" + settings.hour}>每天 {settings.hour}:00</option>
            <option value={"weekly@" + settings.hour}>每周一 {settings.hour}:00</option>
          </select>
          <button
            className="text-[10.5px] px-2 py-1 rounded-md bg-surface-container hover:bg-surface-container-high disabled:opacity-60"
            onClick={() => void runReview()}
            disabled={busy}
            title="立即复盘一次（会调用一次模型）"
          >
            {busy ? "复盘中…" : "立即复盘"}
          </button>
        </div>
      </div>
      {note ? <p className="text-[10.5px] text-outline mt-1">{note}</p> : null}
    </section>
  );
}
