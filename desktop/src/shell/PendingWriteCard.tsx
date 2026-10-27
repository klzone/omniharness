// 待确认写操作卡片（OD-ARCH-046）。
//
// 模型或定时任务想写成果文件时，**不能直接写**：宿主先登记一条待确认记录，
// 这里把它渲染成一张卡片，由用户决定“确认写入”还是“放弃”。
//
// 卡片只展示宿主给出的摘要（来源、场景、影响范围、字数、过期时间），**不渲染正文**，
// 避免把待写内容复制到界面日志或别处。确认后展示宿主回报的真实落盘路径。

import { useCallback, useEffect, useState } from "react";
import { pendingWrites } from "../api";
import type { PendingWriteView } from "../types";

const SOURCE_LABEL: Record<string, string> = {
  model: "模型发起",
  scheduler: "定时任务发起",
};

/** 场景键 → 中文名。界面统一中文，英文副标题只出现在页面小标题里。 */
const SCENARIO_LABEL: Record<string, string> = {
  weekly_report: "工作记录",
  weekly: "工作记录",
  material_writing: "文案写作",
  writing: "文案写作",
  project_management: "项目跟进",
  project: "项目跟进",
};

function clock(secs: number): string {
  if (!Number.isFinite(secs) || secs <= 0) return "—";
  const d = new Date(secs * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * 一张待确认卡片。
 *
 * `onChanged` 用于通知外层刷新列表（确认或放弃之后状态就变了）。
 */
export function PendingWriteCard({
  item,
  onChanged,
  onConfirmed,
}: {
  item: PendingWriteView;
  onChanged?: () => void;
  /** 确认成功后把真实落盘路径交给外层，用于在列表刷新后仍显示一次回执。 */
  onConfirmed?: (item: PendingWriteView, path: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const confirm = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await pendingWrites.confirm(item.id);
      setPath(r.path);
      onConfirmed?.(item, r.path);
      onChanged?.();
    } catch (e) {
      // 失败如实显示：过期、已消费、正文被改动、写盘失败都会走到这里。
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [item.id, item, onChanged, onConfirmed]);

  const reject = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await pendingWrites.reject(item.id);
      onChanged?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [item.id, onChanged]);

  const expired = item.status === "expired";

  return (
    <div
      className={
        // 2026-10-27：用户要求弹窗内的条目用方形底色，不再用圆角底片。
        "border px-3.5 py-3 " +
        (expired
          ? "border-surface-variant/40 bg-surface-container/60"
          : "border-primary/25 bg-primary/5")
      }
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[12px] uppercase tracking-wide text-outline">
              {SOURCE_LABEL[item.source] ?? item.source}
            </span>
            <span className="text-[12px] px-1.5 py-0.5 bg-surface-container text-on-surface-variant">
              {SCENARIO_LABEL[item.scenario] ?? item.scenario}
            </span>
            <span className="text-[12px] font-medium text-on-surface truncate">{item.title}</span>
          </div>
          <p className="text-[12px] text-on-surface-variant mt-1">{item.summary}</p>
          <p className="text-[12px] text-outline mt-0.5">{item.impact}</p>
          <p className="text-[12px] text-outline mt-0.5">
            {item.chars} 字 · 登记 {clock(item.created_at)} · 有效至 {clock(item.expires_at)}
            {expired ? " · 已过期，不能再执行" : ""}
          </p>
          {path ? (
            <p className="text-[12px] text-on-surface-variant mt-1 break-all">
              已写入：{path}
            </p>
          ) : null}
          {error ? <p className="text-[12px] text-error mt-1">{error}</p> : null}
        </div>
        {item.status === "pending" && !path ? (
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              className="text-[12px] px-2 py-1 bg-primary text-on-primary disabled:opacity-50"
              onClick={() => void confirm()}
              disabled={busy}
            >
              {busy ? "处理中…" : "确认写入"}
            </button>
            <button
              className="text-[12px] px-2 py-1 bg-surface-container hover:bg-surface-container-high disabled:opacity-50"
              onClick={() => void reject()}
              disabled={busy}
            >
              放弃
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}

/**
 * 待确认记录面板：只显示 pending 的记录（已过期的不给确认按钮，只作提示）。
 *
 * 没有记录时不占地方，直接不渲染。
 */
export function PendingWritePanel({ refreshKey = 0 }: { refreshKey?: number }) {
  const [items, setItems] = useState<PendingWriteView[]>([]);
  /**
   * 刚确认的条目：宿主把记录标成 consumed 后就不再出现在列表里，
   * 但用户需要看到一次“写到了哪个文件”的回执，所以在这里留一份快照。
   */
  const [justConfirmed, setJustConfirmed] = useState<Record<string, { item: PendingWriteView; path: string }>>({});
  const [broken, setBroken] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let alive = true;
    const run = async () => {
      try {
        const r = await pendingWrites.list();
        if (!alive) return;
        setItems(r.items);
        setBroken(r.broken);
        setError(null);
      } catch (e) {
        if (!alive) return;
        setError(e instanceof Error ? e.message : String(e));
      }
    };
    void run();
    return () => {
      alive = false;
    };
  }, [refreshKey, tick]);

  const visible = items.filter((i) => i.status === "pending" || i.status === "expired");
  if (error) {
    return <p className="text-[12px] text-outline">读取待确认记录失败：{error}</p>;
  }
  const receipts = Object.values(justConfirmed).filter((r) => !visible.some((v) => v.id === r.item.id));

  if (visible.length === 0 && receipts.length === 0 && broken === 0) return null;

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <span className="text-[12px] uppercase tracking-wide text-outline">待确认写入</span>
        <span className="text-[12px] text-outline">
          {visible.filter((i) => i.status === "pending").length} 条待你决定
          {broken > 0 ? ` · ${broken} 条记录损坏` : ""}
        </span>
      </div>
      {visible.map((item) => (
        <PendingWriteCard
          key={item.id}
          item={item}
          onChanged={() => setTick((n) => n + 1)}
          onConfirmed={(it, path) => setJustConfirmed((prev) => ({ ...prev, [it.id]: { item: it, path } }))}
        />
      ))}
      {receipts.map(({ item, path }) => (
        <div
          key={"done-" + item.id}
          className="border border-surface-variant/40 bg-surface-container/60 px-3.5 py-2.5"
        >
          <p className="text-[12px] text-on-surface-variant">
            已写入：<span className="break-all">{path}</span>
          </p>
          <p className="text-[12px] text-outline mt-0.5">{item.summary}（这条记录已结束，不再出现在待确认列表里）</p>
        </div>
      ))}
    </div>
  );
}
