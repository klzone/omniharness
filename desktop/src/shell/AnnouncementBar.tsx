import { useEffect, useRef, useState } from "react";
import { scheduler, shell } from "../api";
import type { SceneDef } from "../scenes";

/**
 * 规则提醒栏（OD-ARCH-033 v3；OD-ARCH-046 心跳；OD-ARCH-092 收为一行）。
 *
 * 只剩**规则提醒**：纯本地确定性计算（复用既有工作记录/项目汇总命令），不花 token、
 * 不依赖模型在线；没有数据就写「暂无」，**不编造**。心跳只负责到点重算这一条。
 *
 * OD-ARCH-092：用户裁定移除自动复盘的界面入口（开关 / 频率 / 立即复盘 / 建议行）。
 * 理由是复盘与「随手记落盘 → 明确说整理周报才生成周报」的新链路重复，且默认关闭、
 * 从未启用。宿主侧的 `scheduler.rs` / `pending_writes.rs` 字段与单测全部保留，
 * 这里只是不再提供触发入口；心跳仍是纯本地扫描。
 *
 * 定时由宿主的调度配置判定（宿主命令 scheduler_tick），前端运行期定时器只负责问
 * “到点了吗”。应用关闭期间不执行；错过则下次打开补跑一次。
 */

export function AnnouncementBar({
  scene,
  itemsText,
  onPendingChanged,
}: {
  scene: SceneDef;
  /** 本场景最近一次输入的事项文本（供确定性汇总使用）。 */
  itemsText: string;
  /** 定时 tick 后通知外层刷新指标带与待确认面板。 */
  onPendingChanged?: () => void;
}) {
  const [rule, setRule] = useState<string | null>(null);
  const [ruleError, setRuleError] = useState<string | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);

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
        setConfigError(null);
        if (t.heartbeat_due) {
          // 心跳只做本地确定性扫描：刷新规则提醒并记录时间，不调模型、不写文件。
          setRuleTick((n) => n + 1);
          await scheduler.markHeartbeat();
        }
        // 每轮 tick 顺带刷新待确认列表。
        onPendingChanged?.();
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
  }, [onPendingChanged]);

  /**
   * 一行提醒（OD-ARCH-092）。没有提醒、也没有读取错误时整行不渲染——
   * 空占位比空卡片更浪费，也不该让人以为系统在暗示什么。
   */
  const text = rule
    ? rule.replace(/\s+/g, " ").slice(0, 220)
    : ruleError
      ? "确定性汇总没有产出：" + ruleError
      : configError
        ? "心跳判定不可用：" + configError
        : "";

  if (!text) return null;

  return (
    <section className="shrink-0 mb-2 rounded-2xl bg-white/80 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-2">
      <div className="flex items-baseline gap-2 min-w-0">
        <span className="text-[10px] uppercase tracking-wide text-outline shrink-0">规则提醒</span>
        <span
          className={"text-[11.5px] truncate " + (rule ? "text-on-surface-variant" : "text-outline")}
          title={rule ?? text}
        >
          {text}
        </span>
      </div>
    </section>
  );
}
