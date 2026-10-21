import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { host, onTaskEvent } from "../api";
import type { AdmissionFacts, EngineDescriptor, EngineHealth, TaskEvent } from "../types";
import { Icon } from "../components/Icon";

function formatTime(seconds: number): string {
  if (!seconds) return "";
  return new Date(seconds * 1000).toLocaleTimeString("zh-CN", { hour12: false });
}

/** 端点类别的中文标签；只区分“本机回环 / 非回环”，不作组织归属承诺。 */
const CATEGORY_LABEL: Record<string, string> = {
  local: "本机回环",
  private: "非回环 · 私有/链路本地地址",
  public: "非回环 · 公网地址",
  unknown: "无法确认",
};

/**
 * 引擎技术切片：只读诊断面板（OD-ARCH-005）。
 *
 * 本视图保留引擎描述、健康快照、事件流与错误信息，但**不再提供任何文本提交**：
 * 宿主尚未提供可核实的端点隔离事实，因此任何面向用户的输入入口都不得把文本
 * 送进模型。此前这里的自由文本框可直接调用 `host.submitTask`，绕过了首页的
 * 准入限制，现已移除。
 *
 * 合成链路验证由专用脚本承担（`tests/verification/desktop/`），不需要保留一个
 * 可绕过准入的界面入口。
 */
export function EngineTechSlice({ onClose }: { onClose: () => void }) {
  const [descriptor, setDescriptor] = useState<EngineDescriptor | null>(null);
  const [health, setHealth] = useState<EngineHealth | null>(null);
  const [descriptorError, setDescriptorError] = useState<string | null>(null);
  const [events, setEvents] = useState<TaskEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [facts, setFacts] = useState<AdmissionFacts | null>(null);

  const scrollRef = useRef<HTMLDivElement | null>(null);

  const refresh = useCallback(async () => {
    setBusy(true);
    setError(null);
    // OD-ARCH-008：引擎描述会拉起子进程，端点未确认时宿主直接拒绝。
    // 健康快照是只读的，必须单独读取，否则描述被拒绝时会连“引擎未启动”都看不到。
    const [a, h] = await Promise.allSettled([host.admissionStatus(), host.health()]);
    if (a.status === "fulfilled") {
      setFacts(a.value.facts);
    } else {
      setFacts(null);
    }
    let allowed = false;
    let blockedReason = "尚未读取到端点信息";
    if (a.status === "fulfilled") {
      allowed = Boolean(a.value.facts?.allowed);
      if (!allowed) {
        blockedReason = a.value.error?.message ?? "当前端点未获授权（请到设置中确认）";
      }
    } else {
      blockedReason = String(a.reason);
    }
    if (allowed) {
      try {
        setDescriptor(await host.describe());
        setDescriptorError(null);
      } catch (e) {
        setDescriptor(null);
        setDescriptorError(String(e));
      }
    } else {
      setDescriptor(null);
      setDescriptorError(blockedReason);
    }
    if (h.status === "fulfilled") {
      setHealth(h.value);
    } else {
      setHealth(null);
      setError(String(h.reason));
    }
    setBusy(false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let closed = false;
    onTaskEvent((event) => {
      setEvents((prev) => {
        if (prev.some((e) => e.seq === event.seq)) return prev;
        return [...prev, event].sort((a, b) => a.seq - b.seq);
      });
    }).then((fn) => {
      if (closed) fn();
      else unlisten = fn;
    });
    return () => {
      closed = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [events.length]);

  const streamedText = useMemo(
    () =>
      events
        .filter((e) => e.type === "text_delta")
        .map((e) => String((e.payload as { text?: unknown }).text ?? ""))
        .join(""),
    [events],
  );

  return (
    <div className="fixed inset-0 z-50 bg-inverse-surface/40 backdrop-blur-sm p-6 sm:p-10 overflow-y-auto">
      <div className="mx-auto max-w-6xl bg-surface-container-lowest rounded-2xl shadow-2xl border border-white/80 overflow-hidden">
        <header className="flex items-center justify-between px-5 py-3.5 border-b border-surface-container">
          <div className="flex items-center space-x-2.5">
            <div className="w-7 h-7 rounded-full bg-primary/10 text-primary flex items-center justify-center">
              <Icon name="terminal" className="w-4 h-4" />
            </div>
            <h2 className="font-headline-md text-[15px] font-semibold text-on-surface">
              引擎技术切片
            </h2>
            <span className="font-label-caps text-[10px] text-outline">
              只读诊断，不接受输入
            </span>
          </div>
          <div className="flex items-center space-x-3">
            <span className="font-label-caps text-[11px] text-outline">
              {descriptor
                ? `${descriptor.engine_kind} · ${descriptor.engine_version} · ${descriptor.compatibility}`
                : descriptorError
                  ? "引擎未就绪"
                  : "正在检测引擎…"}
            </span>
            <button
              onClick={() => void refresh()}
              disabled={busy}
              className="px-3 py-1.5 rounded-full bg-surface-container text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
            >
              {busy ? "刷新中…" : "刷新"}
            </button>
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-full bg-surface-container text-on-surface-variant font-label-caps text-[11px] hover:bg-surface-container-high transition-colors"
            >
              关闭
            </button>
          </div>
        </header>

        <div className="grid md:grid-cols-3 gap-px bg-surface-container">
          <section className="bg-surface-container-lowest p-4">
            <h3 className="font-label-caps text-[11px] text-outline mb-2">准入</h3>
            <div
              className={
                "rounded-xl px-3 py-2 text-[12px] leading-relaxed " +
                (facts?.allowed
                  ? "bg-surface-container-low text-on-surface"
                  : "bg-error-container text-on-error-container")
              }
            >
              {facts?.allowed
                ? "宿主已确认当前端点的类别与你的授权，提交仍由宿主逐次判定。"
                : facts
                  ? "当前端点未获授权（" +
                    (CATEGORY_LABEL[facts.category] ?? facts.category) +
                    "）：" +
                    facts.category_reason
                  : "尚未读取到端点信息；引擎不会启动，输入不会发送。"}
            </div>
            <p className="mt-3 text-[11px] leading-relaxed text-on-surface-variant">
              本页已移除自由文本提交，只读取引擎状态。需要跑合成链路，请使用验证脚本；
              需要日常任务，请回到「本地数据分析」，其引擎步骤同样经过统一准入闸门。
            </p>

            <dl className="mt-4 space-y-1.5 font-label-caps text-[11px]">
              <div className="flex justify-between gap-3">
                <dt className="text-outline">引擎运行</dt>
                <dd className="text-on-surface">
                  {health ? (health.running ? "运行中" : "未启动") : "—"}
                </dd>
              </div>
              <div className="flex justify-between gap-3">
                <dt className="text-outline">协议</dt>
                <dd className="text-on-surface truncate">
                  {health?.protocol_version ?? "—"}
                </dd>
              </div>
              <div className="flex justify-between gap-3">
                <dt className="text-outline">应用数据根</dt>
                <dd className="text-on-surface truncate">{health?.app_data_root ?? "—"}</dd>
              </div>
              <div className="flex justify-between gap-3">
                <dt className="text-outline">DSH 隔离根</dt>
                <dd className="text-on-surface truncate">{health?.dsh_home ?? "—"}</dd>
              </div>
            </dl>
            {health?.note ? (
              <p className="mt-2 text-[11px] text-outline">{health.note}</p>
            ) : null}
            {health?.stderr_tail && health.stderr_tail.length > 0 ? (
              <div className="mt-3">
                <div className="text-[10px] uppercase tracking-wide text-outline">
                  引擎输出（最后 {health.stderr_tail.length} 行）
                </div>
                <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-surface-container p-2 text-[10.5px] text-on-surface-variant">
                  {health.stderr_tail.join("\n")}
                </pre>
              </div>
            ) : null}

            {descriptorError ? (
              <div className="mt-3 rounded-lg bg-error-container text-on-error-container px-3 py-2 text-[12px]">
                引擎状态：{descriptorError}
              </div>
            ) : null}
            {error ? (
              <div className="mt-3 rounded-lg bg-error-container text-on-error-container px-3 py-2 text-[12px]">
                {error}
              </div>
            ) : null}
          </section>

          <section className="bg-surface-container-lowest p-4">
            <h3 className="font-label-caps text-[11px] text-outline mb-2">
              事件流 <span className="text-outline-variant">{events.length} 条</span>
            </h3>
            <div
              className="h-[24rem] overflow-y-auto rounded-xl bg-inverse-surface/95 p-3 space-y-2"
              ref={scrollRef}
            >
              {events.length === 0 ? (
                <div className="text-[11px] text-inverse-on-surface/80">
                  当前没有任何任务事件。本页不能发起任务；事件只来自别处已获准的运行。
                </div>
              ) : (
                events.map((e) => (
                  <div key={e.seq} className="text-[11px] font-label-caps text-inverse-on-surface/85">
                    <div className="flex items-center gap-2">
                      <span className="text-inverse-on-surface/50">#{e.seq}</span>
                      <span className="text-secondary-fixed-dim">{e.type}</span>
                      <span className="text-inverse-on-surface/40">{formatTime(e.occurred_at)}</span>
                    </div>
                    <pre className="whitespace-pre-wrap break-all text-inverse-on-surface/70">
                      {JSON.stringify(e.payload)}
                    </pre>
                  </div>
                ))
              )}
            </div>
          </section>

          <section className="bg-surface-container-lowest p-4">
            <h3 className="font-label-caps text-[11px] text-outline mb-2">流式文本（只读）</h3>
            {streamedText ? (
              <pre className="max-h-[24rem] overflow-y-auto rounded-lg bg-surface-container-low p-2 text-[11px] whitespace-pre-wrap">
                {streamedText}
              </pre>
            ) : (
              <div className="text-[12px] text-on-surface-variant">
                暂无流式文本。本页不提供「读取结果」入口，完整结果请在发起任务的页面查看。
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
