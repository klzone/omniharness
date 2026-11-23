/**
 * 每日整理无人值守运行器（OD-GROW-301 G4）。不渲染任何可见界面。
 *
 * 每 30 秒问宿主到点没有。到点、且没有在途对话时才开始当天唯一一次整理。
 * 空摘要只记完成；非空摘要用配置的 provider/model 提交 habit_consolidation。
 * 任务终态后把模型原始输出写入 localStorage，宿主校验与暂存留给 G5。
 */

import { useEffect, useRef } from "react";

import {
  consolidation,
  host,
  shell,
  type ConsolidationCollectResult,
  type ConsolidationConfig,
} from "../api";
import {
  CONSOLIDATION_LAST_OUTPUT_KEY,
  CONSOLIDATION_LAST_RUN_KEY,
  CONSOLIDATION_NOTICE_EVENT,
  consolidationLocalDate,
  emptyConsolidationState,
  nextStep,
  type ConsolidationNotice,
} from "../consolidationFlow";
import { effortEvidenceFromLoaded } from "../modelEffortEvidence";
import { listThreads } from "../sceneThreads";
import { TERMINAL_STATES, type SubmitTaskReceipt } from "../types";

const TICK_MS = 30_000;
const POLL_MS = 1_000;
const TASK_LIMIT_MS = 1_800_000;
const MAX_OUTPUT_CHARS = 20_000;
const DEFAULT_EFFORT = "low";

function readLastRun(): string | null {
  try {
    const value = window.localStorage.getItem(CONSOLIDATION_LAST_RUN_KEY);
    return value && /^\d{4}-\d{2}-\d{2}$/.test(value) ? value : null;
  } catch {
    return null;
  }
}

function writeLastRun(date: string) {
  try {
    window.localStorage.setItem(CONSOLIDATION_LAST_RUN_KEY, date);
  } catch (e) {
    console.warn("[consolidation] 无法记录最后运行日期：" + errorText(e));
  }
}

/** 只认线程档案里的在途任务号；这是工作台当前唯一的全局对话运行标记。 */
function hasRunningChat(): boolean {
  return listThreads().some((thread) => Boolean(thread.inflightTaskId));
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function notify(notice: ConsolidationNotice) {
  try {
    window.dispatchEvent(
      new CustomEvent<ConsolidationNotice>(CONSOLIDATION_NOTICE_EVENT, { detail: notice }),
    );
  } catch {
    // 事件通道不可用时仍保留 console 记录，不弹系统错误框。
  }
}

function chosenEffort(config: ConsolidationConfig): string {
  const configured = (config.reasoning_effort ?? "").trim();
  if (configured) return configured;
  if ((config.model ?? "").toLowerCase().includes("eco")) return "off";
  return DEFAULT_EFFORT;
}

async function effortEvidence(provider: string | null, model: string | null) {
  if (!provider || !model) return null;
  const [catalogResult, routesResult] = await Promise.allSettled([
    shell.listEngineModels(),
    shell.listModelRoutes(),
  ]);
  const catalog = catalogResult.status === "fulfilled" ? catalogResult.value : null;
  const routes = routesResult.status === "fulfilled" ? routesResult.value : null;
  return effortEvidenceFromLoaded({
    catalog,
    routes: routes?.profiles ?? null,
    sourceProfile: routes?.engine_profile ?? null,
    provider,
    model,
  });
}

/** 写入本轮模型原始输出；只保留前 20000 个字符。G5 负责校验与暂存。 */
function storeRawOutput(content: string): number {
  const clipped = Array.from(content).slice(0, MAX_OUTPUT_CHARS).join("");
  window.localStorage.setItem(CONSOLIDATION_LAST_OUTPUT_KEY, clipped);
  return Array.from(clipped).length;
}

function reportFailure(today: string, reason: string) {
  const message = reason.trim() || "未知原因";
  writeLastRun(today);
  console.warn("[consolidation] 本次整理未完成：" + message);
  notify({ kind: "failure", message, at: Date.now() });
}

async function runCycle(nowSeconds: number, alive: () => boolean): Promise<void> {
  let config: ConsolidationConfig;
  try {
    config = await consolidation.getConfig();
  } catch (e) {
    console.warn("[consolidation] 读取整理配置失败：" + errorText(e));
    return;
  }
  // 读配置可能让出事件循环；真正开始收集前再确认一次没有对话在跑。
  if (!alive() || hasRunningChat()) return;

  const today = consolidationLocalDate(nowSeconds, config.utc_offset_minutes);
  if (!today) {
    console.warn("[consolidation] 无法确定今天的本地日期，本次不运行。");
    return;
  }

  const state = emptyConsolidationState(today);
  state.tickDue = true;
  state.runningChat = false;
  state.attemptDate = readLastRun() === today ? today : null;
  if (nextStep(state) !== "prepare") return;

  state.phase = "prepare";
  let collected: ConsolidationCollectResult;
  try {
    collected = await consolidation.collect(nowSeconds, config);
  } catch (e) {
    reportFailure(today, "收集整理摘要失败：" + errorText(e));
    return;
  }
  if (!alive()) return;

  // 收集已落盘水位与 last_run_at；今天无论后续成败都不再自动重跑。
  writeLastRun(today);
  state.digestEmpty = collected.digest_text.trim() === "";
  let step = nextStep(state);
  if (step === "done") {
    console.info(
      "[consolidation] 今日没有需要整理的摘录，已记完成，不调用模型。",
    );
    notify({ kind: "empty", message: "今日没有需要整理的摘录", at: Date.now() });
    return;
  }
  if (step !== "submit") return;

  const provider = (config.provider ?? "").trim() || null;
  const model = (config.model ?? "").trim() || null;
  const reasoningEffort = chosenEffort(config);
  const evidence = await effortEvidence(provider, model);
  if (!alive()) return;

  let receipt: SubmitTaskReceipt;
  try {
    receipt = await host.submitTask({
      promptText: collected.digest_text,
      intent: "habit_consolidation",
      reasoningEffort,
      effortEvidence: evidence,
      provider,
      model,
    });
  } catch (e) {
    state.submitRejected = true;
    reportFailure(today, "整理任务提交被拒：" + errorText(e));
    return;
  }

  state.phase = "await";
  step = nextStep(state);
  if (step !== "await") {
    reportFailure(today, "整理任务未进入等待状态。");
    return;
  }

  const taskId = receipt.omni_task_id;
  const startedAt = Date.now();
  let lastPollError = "";
  while (alive() && step === "await") {
    try {
      const status = await host.taskStatus(taskId);
      lastPollError = "";
      if ((TERMINAL_STATES as readonly string[]).includes(status.status)) {
        state.taskDone = true;
        state.taskSucceeded = status.status === "succeeded";
      }
    } catch (e) {
      const message = errorText(e);
      if (message !== lastPollError) {
        console.warn("[consolidation] 整理任务状态读取失败：" + message);
        lastPollError = message;
      }
    }

    if (Date.now() - startedAt >= TASK_LIMIT_MS) state.timedOut = true;
    step = nextStep(state);
    if (step === "await") await sleep(POLL_MS);
  }
  if (!alive()) return;

  if (step === "failed") {
    if (state.timedOut) {
      await host.cancelTask(taskId, 5).catch((e: unknown) => {
        console.warn("[consolidation] 超时后取消失败：" + errorText(e));
      });
    }
    reportFailure(
      today,
      state.timedOut
        ? "整理任务超过 1800 秒未结束。"
        : "整理任务没有成功结束（" + (state.taskDone ? "终态失败" : "未知") + "）。",
    );
    return;
  }

  if (step !== "ingest") {
    reportFailure(today, "整理任务进入了非预期的等待结果。");
    return;
  }

  state.phase = "ingest";
  step = nextStep(state);
  if (step !== "done") {
    reportFailure(today, "整理结果未进入记录阶段。");
    return;
  }

  let content: string;
  try {
    const result = await host.taskResult(taskId);
    if (result.content_omitted) {
      reportFailure(today, "整理任务结果是省略正文的回执，G4 无法记录 raw 内容。");
      return;
    }
    content = typeof result.content === "string" ? result.content : "";
  } catch (e) {
    reportFailure(today, "读取整理任务结果失败：" + errorText(e));
    return;
  }
  if (!content.trim()) {
    reportFailure(today, "整理任务结果没有正文，本次不记录空输出。");
    return;
  }

  let outputChars: number;
  try {
    outputChars = storeRawOutput(content);
  } catch (e) {
    reportFailure(today, "记录整理输出失败：" + errorText(e));
    return;
  }

  // G4 只记录 raw；G5 再接宿主校验与暂存。
  console.info(
    "[consolidation] 整理完成：" + collected.stats.thread_count + " 条线程，" +
      collected.stats.message_count + " 条发言，" + collected.stats.char_count +
      " 字摘要；模型输出 " + outputChars + " 字已记为待校验结果。",
  );
  notify({
    kind: "done",
    message: "整理完成，等待宿主校验与暂存",
    at: Date.now(),
    threads: collected.stats.thread_count,
    messages: collected.stats.message_count,
    chars: collected.stats.char_count,
    outputChars,
  });
}

export function ConsolidationRunner() {
  const runningRef = useRef(false);

  useEffect(() => {
    let alive = true;

    const tick = async () => {
      if (!alive || runningRef.current) return;
      runningRef.current = true;
      try {
        const due = await consolidation.tick();
        if (!alive || !due.due) return;
        if (hasRunningChat()) return;
        await runCycle(due.now, () => alive);
      } catch (e) {
        console.warn("[consolidation] 运行器本轮检查失败：" + errorText(e));
      } finally {
        runningRef.current = false;
      }
    };

    void tick();
    const timer = window.setInterval(() => void tick(), TICK_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  return null;
}
