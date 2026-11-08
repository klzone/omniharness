import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type {
  MaterialDetail,
  MaterialReviewAction,
  MaterialTraceEntity,
  WritingMaterial,
  WritingMaterialReviewStatus,
  WritingMaterialSourceType,
  WritingMaterialType,
} from "../types";
import { SourcePeekSheet } from "./SourcePeekSheet";

type Notice = { kind: "ok" | "err"; text: string };

const MATERIAL_TYPES: WritingMaterialType[] = [
  "工作事实",
  "工作成果",
  "数据素材",
  "经验做法",
  "典型案例",
  "问题不足",
  "制度依据",
  "工作计划",
  "领导部署",
  "特色亮点",
];

const REVIEW_STATUSES: WritingMaterialReviewStatus[] = [
  "待审核",
  "已确认",
  "已退回",
  "已拒绝",
  "已归档",
];

const SOURCE_LABELS: Record<WritingMaterialSourceType, string> = {
  work_record: "工作记录",
  task: "任务",
  project: "项目",
  knowledge: "知识",
  user: "收藏",
};

const STATUS_SKIN: Record<WritingMaterialReviewStatus, string> = {
  待审核: "text-amber-700 bg-amber-500/15 ring-1 ring-amber-500/25",
  已确认: "text-tertiary bg-tertiary/10",
  已退回: "text-orange-700 bg-orange-500/10",
  已拒绝: "text-error bg-error-container/60",
  已归档: "text-outline bg-surface-container",
};

const FIELD =
  "neu-inset-field w-full rounded-lg px-2.5 py-1.5 text-[11.5px] leading-relaxed text-on-surface outline-none placeholder:text-outline/70";

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function shortTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getMonth() + 1}-${date.getDate()} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function StatusChip({ value }: { value: WritingMaterialReviewStatus }) {
  return (
    <span
      className={"shrink-0 rounded-full px-1.5 py-[1px] text-[9.5px] font-medium " + STATUS_SKIN[value]}
      data-material-status-label={value}
    >
      {value}
    </span>
  );
}

function TypePill({ value }: { value: WritingMaterialType }) {
  return (
    <span className="shrink-0 rounded-full bg-primary/10 px-1.5 py-[1px] text-[9.5px] text-primary">
      {value}
    </span>
  );
}

function MaterialRow({
  item,
  showType,
  onOpen,
}: {
  item: WritingMaterial;
  showType: boolean;
  onOpen: (id: string) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen(item.material_id)}
      className={
        "group w-full min-w-0 rounded-xl border px-3 py-2 text-left transition " +
        (item.review_status === "待审核"
          ? "border-amber-500/25 bg-amber-500/5 hover:bg-amber-500/10"
          : "border-surface-variant/25 bg-surface-container-low/40 hover:border-primary/25 hover:bg-surface-container-low")
      }
      data-material-item={item.material_id}
      data-material-item-status={item.review_status}
    >
      <span className="flex items-start gap-2 min-w-0">
        <span className="min-w-0 flex-1 text-[12.5px] font-medium leading-snug text-on-surface line-clamp-2 break-words">
          {item.title}
        </span>
        <StatusChip value={item.review_status} />
      </span>
      <span className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-outline">
        {showType ? <TypePill value={item.material_type} /> : null}
        <span>{SOURCE_LABELS[item.source_type]}</span>
        <span>{shortTime(item.updated_at)}</span>
        <span className="ml-auto text-primary opacity-0 group-hover:opacity-100">查看</span>
      </span>
    </button>
  );
}

/**
 * 素材篮（OD-KM-510）：写作页「素材篮」页签内容。
 *
 * 列表按类型分组、按审核状态筛选；待审核永远单独置顶，颜色之外还有文字标签。
 * 详情在当前面板内打开，来源引用与「打开原文」都在这里；审核动作就地完成，
 * 退回/拒绝必须先写理由。所有数据来自 506 的 material_list / material_get /
 * material_review / material_create，本组件不直连 Tauri invoke，也不造假数据。
 */
export function WritingMaterialBasket() {
  const [items, setItems] = useState<WritingMaterial[]>([]);
  const [broken, setBroken] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<WritingMaterialReviewStatus | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<MaterialDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailTick, setDetailTick] = useState(0);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [creating, setCreating] = useState(false);
  const [draftTitle, setDraftTitle] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [draftType, setDraftType] = useState<WritingMaterialType>("工作事实");
  const [reviewAction, setReviewAction] = useState<"return" | "reject" | null>(null);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [peek, setPeek] = useState<{ title: string; body: string } | null>(null);
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .materialList({})
      .then((listing) => {
        if (!alive) return;
        setItems(listing.items);
        setBroken(listing.broken);
        setError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    if (!selected) {
      setDetail(null);
      return;
    }
    let alive = true;
    setDetailLoading(true);
    setDetailError(null);
    shell
      .materialGet(selected)
      .then((result) => {
        if (alive) {
          setDetail(result);
          setDetailError(null);
        }
      })
      .catch((e) => {
        if (alive) {
          setDetail(null);
          setDetailError(errText(e));
        }
      })
      .finally(() => {
        if (alive) setDetailLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected, detailTick]);

  const counts = useMemo(() => {
    const base = { all: items.length } as Record<WritingMaterialReviewStatus | "all", number>;
    for (const status of REVIEW_STATUSES) {
      base[status] = items.filter((item) => item.review_status === status).length;
    }
    return base;
  }, [items]);

  const visible = useMemo(
    () => (statusFilter ? items.filter((item) => item.review_status === statusFilter) : items),
    [items, statusFilter],
  );
  const pending = visible.filter((item) => item.review_status === "待审核");
  const rest = visible.filter((item) => item.review_status !== "待审核");
  const groups = MATERIAL_TYPES.map((type) => ({
    type,
    items: rest.filter((item) => item.material_type === type),
  })).filter((group) => group.items.length > 0);

  const refreshList = () => setReloadTick((n) => n + 1);

  const runReview = async (
    materialId: string,
    action: MaterialReviewAction,
    reviewReason?: string,
  ) => {
    setBusy(true);
    try {
      const updated = await shell.materialReview(materialId, action, reviewReason ?? null);
      setDetail((prev) => (prev ? { ...prev, material: updated } : prev));
      setReviewAction(null);
      setReason("");
      setNotice({ kind: "ok", text: `素材已${action === "confirm" ? "批准" : action === "return" ? "退回" : action === "reject" ? "拒绝" : "暂缓"}。` });
      refreshList();
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  };

  const saveCollection = async () => {
    const content = draftContent.trim();
    const title = draftTitle.trim() || content.split("\n")[0]?.trim().slice(0, 60) || "";
    if (!content || !title) {
      setNotice({ kind: "err", text: "收藏素材需要标题或正文。" });
      return;
    }
    setBusy(true);
    try {
      const saved = await shell.materialCreate({
        title,
        content,
        material_type: draftType,
        source_type: "user",
        source_quote: content,
        tags: [],
      });
      setCreating(false);
      setDraftTitle("");
      setDraftContent("");
      setDraftType("工作事实");
      setStatusFilter(null);
      setSelected(saved.material_id);
      setNotice({ kind: "ok", text: "已收藏，等待审核。" });
      refreshList();
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  };

  const previewTask = async (taskId: string) => {
    const task = await shell.taskRead(taskId);
    const lines = [
      `任务：${task.title}`,
      `状态：${task.state}`,
      task.due_date ? `截止：${task.due_date}` : "",
      task.source_quote ? `\n原文引用：\n${task.source_quote}` : "",
      task.progress.length
        ? `\n进展：\n${task.progress.map((entry) => `${shortTime(entry.at)} · ${entry.text}`).join("\n")}`
        : "",
    ].filter(Boolean);
    return { title: task.title, body: lines.join("\n") };
  };

  const previewKnowledge = async (trace: MaterialTraceEntity) => {
    const note = await shell.knowledgeVaultReadNote(trace.location ?? trace.id);
    return { title: trace.title ?? trace.id, body: note.content };
  };

  const openOriginal = async () => {
    if (!detail) return;
    const trace = detail.source;
    if (!trace.entity.found) {
      setNotice({ kind: "err", text: trace.entity.note ?? "来源原文不可读。" });
      return;
    }
    setBusy(true);
    try {
      let preview: { title: string; body: string };
      switch (trace.source_type) {
        case "work_record": {
          const record = await shell.workRecordRead(trace.source_id);
          preview = { title: trace.entity.title ?? record.title, body: record.body };
          break;
        }
        case "task":
          preview = await previewTask(trace.source_id);
          break;
        case "knowledge":
          preview = await previewKnowledge(trace.entity);
          break;
        case "project":
          preview = {
            title: trace.entity.title ?? trace.source_id,
            body: trace.excerpt ?? trace.quote ?? "项目原文没有可读正文。",
          };
          break;
        case "user":
          preview = { title: "用户收藏", body: detail.material.content };
          break;
      }
      setPeek(preview);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  };

  const openRelatedTask = async (entity: MaterialTraceEntity) => {
    setBusy(true);
    try {
      setPeek(await previewTask(entity.id));
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  };

  const openRelatedKnowledge = async (entity: MaterialTraceEntity) => {
    if (!entity.found) {
      setNotice({ kind: "err", text: entity.note ?? "知识条目不可读。" });
      return;
    }
    setBusy(true);
    try {
      setPeek(await previewKnowledge(entity));
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  };

  const renderDetail = () => {
    if (!selected) return null;
    if (detailLoading) {
      return (
        <div className="pt-4" data-material-detail-loading="">
          <button
            type="button"
            onClick={() => setSelected(null)}
            className="text-[11px] text-primary hover:underline"
          >
            ← 列表
          </button>
          <p className="mt-2 text-[11.5px] text-outline">正在读取素材详情…</p>
        </div>
      );
    }
    if (detailError || !detail) {
      return (
        <div className="pt-4" data-material-detail-loading="">
          <button
            type="button"
            onClick={() => setSelected(null)}
            className="text-[11px] text-primary hover:underline"
          >
            ← 列表
          </button>
          <p className="mt-2 text-[11.5px] leading-relaxed text-error">读取素材详情失败：{detailError ?? "未返回详情。"}</p>
          <button
            type="button"
            onClick={() => setDetailTick((n) => n + 1)}
            className="mt-1.5 text-[10.5px] text-primary hover:underline"
          >
            重试
          </button>
        </div>
      );
    }

    const { material, source, related_tasks, related_project, related_knowledge } = detail;
    const canConfirm = material.review_status !== "已确认" && material.review_status !== "已归档";
    const canReturn = material.review_status === "待审核" || material.review_status === "已确认";
    const canReject = material.review_status === "待审核" || material.review_status === "已退回";
    const canDefer = material.review_status === "待审核";
    const reviewActionLabel = reviewAction === "return" ? "退回" : reviewAction === "reject" ? "拒绝" : "";

    return (
      <div className="flex min-h-0 flex-col" data-material-detail={material.material_id}>
        <div className="flex items-center gap-2 pb-2.5">
          <button
            type="button"
            onClick={() => setSelected(null)}
            className="text-[11px] text-primary hover:underline"
          >
            ← 列表
          </button>
          <StatusChip value={material.review_status} />
          <TypePill value={material.material_type} />
          <span className="mono ml-auto text-[9.5px] text-outline">{shortTime(material.updated_at)}</span>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto pr-0.5 max-h-[52vh]">
          <h3 className="text-[14px] font-semibold leading-snug text-on-surface break-words">{material.title}</h3>
          <p
            className="mt-2 whitespace-pre-wrap text-[11.5px] leading-[1.75] text-on-surface-variant break-words"
            data-material-content=""
          >
            {material.content}
          </p>

          <div className="mt-3 rounded-xl border border-surface-variant/35 bg-surface-container-low/45 px-3 py-2.5" data-material-source="">
            <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
              <span className="text-[10.5px] font-medium text-on-surface">
                来源 · {SOURCE_LABELS[source.source_type]}
              </span>
              <span className="mono text-[9.5px] text-outline truncate">{source.source_id}</span>
              {source.location ? (
                <span className="mono ml-auto max-w-full truncate text-[9.5px] text-outline" data-hover-hint={source.location}>
                  {source.location}
                </span>
              ) : null}
            </div>
            {source.entity.title ? (
              <p className="mt-1 text-[11px] text-on-surface-variant break-words">{source.entity.title}</p>
            ) : null}
            {source.entity.note ? (
              <p className="mt-1 text-[10.5px] text-amber-700">{source.entity.note}</p>
            ) : null}

            <div className="mt-2 rounded-lg bg-white/80 px-2.5 py-2" data-material-quote="">
              <p className="text-[9.5px] text-outline">引用原文</p>
              <p className="mt-1 whitespace-pre-wrap text-[11px] leading-[1.7] text-on-surface-variant break-words">
                {source.quote}
              </p>
            </div>
            {!source.quote_found ? (
              <p className="mt-1.5 text-[10px] text-amber-700" data-material-quote-missing="">
                引用未能在来源原文中定位；这里仍保留收藏时的原文。
              </p>
            ) : null}
            {source.excerpt ? (
              <div className="mt-2" data-material-excerpt="">
                <p className="text-[9.5px] text-outline">上下文</p>
                <p className="mt-0.5 whitespace-pre-wrap text-[10.5px] leading-[1.7] text-on-surface-variant">
                  {source.excerpt}
                </p>
              </div>
            ) : null}
            <button
              type="button"
              onClick={() => void openOriginal()}
              disabled={busy || !source.entity.found}
              className="mt-2 rounded-full border border-primary/30 px-2.5 py-1 text-[10.5px] text-primary transition hover:bg-primary/10 disabled:opacity-40"
              data-material-open-source=""
            >
              打开原文
            </button>
          </div>

          {related_tasks.length > 0 || related_project || related_knowledge.length > 0 ? (
            <div className="mt-3 flex flex-col gap-1.5" data-material-related="">
              {related_tasks.length > 0 ? (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] text-outline">任务</span>
                  {related_tasks.map((entity) => (
                    <button
                      key={entity.id}
                      type="button"
                      onClick={() => void openRelatedTask(entity)}
                      disabled={busy || !entity.found}
                      className="max-w-full truncate rounded-full bg-surface-container px-2 py-0.5 text-[10px] text-on-surface-variant hover:bg-surface-container-high disabled:opacity-50"
                      data-hover-hint={entity.title ?? entity.id}
                    >
                      {entity.title ?? entity.id}
                    </button>
                  ))}
                </div>
              ) : null}
              {related_project ? (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] text-outline">项目</span>
                  <span className="rounded-full bg-surface-container px-2 py-0.5 text-[10px] text-on-surface-variant">
                    {related_project.title ?? related_project.id}
                  </span>
                </div>
              ) : null}
              {related_knowledge.length > 0 ? (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] text-outline">知识</span>
                  {related_knowledge.map((entity) => (
                    <button
                      key={entity.id}
                      type="button"
                      onClick={() => void openRelatedKnowledge(entity)}
                      disabled={busy || !entity.found}
                      className="max-w-full truncate rounded-full bg-surface-container px-2 py-0.5 text-[10px] text-on-surface-variant hover:bg-surface-container-high disabled:opacity-50"
                      data-hover-hint={entity.title ?? entity.id}
                    >
                      {entity.title ?? entity.id}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}

          {material.review_records.length > 0 ? (
            <div className="mt-3" data-material-review-records="">
              <p className="text-[9.5px] text-outline">审核记录</p>
              <ul className="mt-1 flex flex-col gap-1">
                {[...material.review_records].reverse().map((record, index) => (
                  <li key={`${record.at}-${index}`} className="text-[10px] leading-relaxed text-on-surface-variant break-words">
                    <span className="mono text-outline">{shortTime(record.at)}</span>
                    {" · "}
                    {record.action}
                    {record.reason ? ` · ${record.reason}` : null}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>

        <div className="mt-2.5 border-t border-surface-variant/25 pt-2.5">
          {reviewAction ? (
            <div className="flex flex-col gap-1.5">
              <textarea
                value={reason}
                onChange={(event) => setReason(event.target.value)}
                rows={2}
                className={FIELD}
                aria-label={`${reviewActionLabel}理由`}
                placeholder={`${reviewActionLabel}理由（必填）`}
                data-material-review-reason=""
              />
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => void runReview(material.material_id, reviewAction, reason.trim())}
                  disabled={busy || !reason.trim()}
                  className="rounded-full bg-on-surface px-3 py-1 text-[10.5px] text-inverse-on-surface disabled:opacity-40"
                  data-material-review-submit={reviewAction}
                >
                  提交{reviewActionLabel}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setReviewAction(null);
                    setReason("");
                  }}
                  className="rounded-full px-2.5 py-1 text-[10.5px] text-on-surface-variant hover:bg-surface-container"
                >
                  取消
                </button>
              </div>
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-1.5" data-material-review-actions="">
              {canConfirm ? (
                <button
                  type="button"
                  onClick={() => void runReview(material.material_id, "confirm")}
                  disabled={busy}
                  className="rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary transition hover:opacity-90 disabled:opacity-40"
                  data-material-review-action="confirm"
                >
                  批准
                </button>
              ) : null}
              {canReturn ? (
                <button
                  type="button"
                  onClick={() => {
                    setReviewAction("return");
                    setReason("");
                  }}
                  disabled={busy}
                  className="rounded-full border border-amber-500/35 px-3 py-1 text-[10.5px] text-amber-700 transition hover:bg-amber-500/10 disabled:opacity-40"
                  data-material-review-action="return"
                >
                  退回
                </button>
              ) : null}
              {canReject ? (
                <button
                  type="button"
                  onClick={() => {
                    setReviewAction("reject");
                    setReason("");
                  }}
                  disabled={busy}
                  className="rounded-full border border-error/30 px-3 py-1 text-[10.5px] text-error transition hover:bg-error-container/50 disabled:opacity-40"
                  data-material-review-action="reject"
                >
                  拒绝
                </button>
              ) : null}
              {canDefer ? (
                <button
                  type="button"
                  onClick={() => void runReview(material.material_id, "defer")}
                  disabled={busy}
                  className="rounded-full px-3 py-1 text-[10.5px] text-on-surface-variant transition hover:bg-surface-container disabled:opacity-40"
                  data-material-review-action="defer"
                >
                  暂缓
                </button>
              ) : null}
              {!canConfirm && !canReturn && !canReject && !canDefer ? (
                <span className="text-[10.5px] text-outline">这条素材当前没有可执行的审核动作。</span>
              ) : null}
            </div>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="flex min-h-0 flex-col pt-3" data-writing-material-basket="">
      {notice ? (
        <div
          role="status"
          className={
            "mb-2 rounded-xl px-2.5 py-1.5 text-[10.5px] leading-relaxed " +
            (notice.kind === "err"
              ? "bg-error-container/70 text-on-error-container"
              : "bg-surface-container text-on-surface-variant")
          }
          data-material-notice={notice.kind}
        >
          {notice.text}
        </div>
      ) : null}

      {selected ? (
        renderDetail()
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-1" data-material-filters="">
            <button
              type="button"
              onClick={() => setStatusFilter(null)}
              aria-pressed={statusFilter === null}
              className={
                "rounded-full px-2.5 py-1 text-[10.5px] transition " +
                (statusFilter === null
                  ? "bg-on-surface text-inverse-on-surface"
                  : "text-on-surface-variant hover:bg-surface-container")
              }
            >
              全部 {counts.all}
            </button>
            {REVIEW_STATUSES.map((status) => (
              <button
                key={status}
                type="button"
                onClick={() => setStatusFilter(statusFilter === status ? null : status)}
                aria-pressed={statusFilter === status}
                className={
                  "rounded-full px-2.5 py-1 text-[10.5px] transition " +
                  (statusFilter === status
                    ? "bg-on-surface text-inverse-on-surface"
                    : "text-on-surface-variant hover:bg-surface-container")
                }
                data-material-status-filter={status}
              >
                {status} {counts[status]}
              </button>
            ))}
            <button
              type="button"
              onClick={() => setCreating((value) => !value)}
              className="neu-chip ml-auto rounded-full px-2.5 py-1 text-[10.5px] text-primary"
              aria-expanded={creating}
              data-material-create-toggle=""
            >
              {creating ? "收起收藏" : "收藏素材"}
            </button>
          </div>

          {creating ? (
            <div className="mt-2 flex flex-col gap-1.5 rounded-xl border border-primary/20 bg-primary/5 p-2.5" data-material-create="">
              <input
                value={draftTitle}
                onChange={(event) => setDraftTitle(event.target.value)}
                className={FIELD}
                placeholder="标题（留空取正文首行）"
                aria-label="素材标题"
              />
              <div className="flex items-center gap-1.5">
                <select
                  value={draftType}
                  onChange={(event) => setDraftType(event.target.value as WritingMaterialType)}
                  className="neu-inset-field shrink-0 rounded-lg px-2.5 py-1.5 text-[11.5px] text-on-surface outline-none"
                  aria-label="素材类型"
                >
                  {MATERIAL_TYPES.map((type) => (
                    <option key={type} value={type}>
                      {type}
                    </option>
                  ))}
                </select>
                <span className="text-[10px] text-outline">保存后进入待审核</span>
              </div>
              <textarea
                value={draftContent}
                onChange={(event) => setDraftContent(event.target.value)}
                rows={4}
                className={FIELD}
                placeholder="素材正文；收藏时正文同时作为来源引用"
                aria-label="素材正文"
                data-material-create-content=""
              />
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => void saveCollection()}
                  disabled={busy || !draftContent.trim()}
                  className="rounded-full bg-primary px-3 py-1 text-[10.5px] text-on-primary disabled:opacity-40"
                  data-material-create-submit=""
                >
                  收藏
                </button>
                <button
                  type="button"
                  onClick={() => setCreating(false)}
                  className="rounded-full px-2.5 py-1 text-[10.5px] text-on-surface-variant hover:bg-surface-container"
                >
                  取消
                </button>
              </div>
            </div>
          ) : null}

          <div className="mt-2.5 min-h-0 flex-1 overflow-y-auto pr-0.5 max-h-[52vh]">
            {loading ? (
              <p className="text-[11.5px] text-outline">正在读取素材篮…</p>
            ) : error ? (
              <div className="flex flex-col gap-1.5">
                <p className="text-[11.5px] leading-relaxed text-error">读取素材篮失败：{error}</p>
                <button
                  type="button"
                  onClick={refreshList}
                  className="self-start text-[10.5px] text-primary hover:underline"
                >
                  重试
                </button>
              </div>
            ) : visible.length === 0 ? (
              <p className="text-[11.5px] leading-relaxed text-outline" data-material-empty="">
                {statusFilter
                  ? "这个筛选下还没有素材。"
                  : "素材篮还是空的。从工作记录提取，或点「收藏素材」记录一条。"}
              </p>
            ) : (
              <div className="flex flex-col gap-2.5" data-material-list="">
                {pending.length > 0 ? (
                  <section className="rounded-xl border border-amber-500/25 bg-amber-500/5 p-1.5" data-material-pending="">
                    <div className="flex items-baseline gap-2 px-1 pb-1.5">
                      <h3 className="text-[11px] font-semibold text-amber-700">待审核</h3>
                      <span className="mono text-[9.5px] text-amber-700/80">{pending.length} 条</span>
                    </div>
                    <ul className="flex flex-col gap-1.5">
                      {pending.map((item) => (
                        <li key={item.material_id}>
                          <MaterialRow item={item} showType onOpen={setSelected} />
                        </li>
                      ))}
                    </ul>
                  </section>
                ) : null}

                {groups.map((group) => (
                  <section key={group.type} data-material-group={group.type}>
                    <div className="flex items-baseline gap-2 px-1 pb-1">
                      <h3 className="text-[11px] font-medium text-on-surface-variant">{group.type}</h3>
                      <span className="mono text-[9.5px] text-outline">{group.items.length}</span>
                    </div>
                    <ul className="flex flex-col gap-1.5">
                      {group.items.map((item) => (
                        <li key={item.material_id}>
                          <MaterialRow item={item} showType={false} onOpen={setSelected} />
                        </li>
                      ))}
                    </ul>
                  </section>
                ))}
              </div>
            )}

            {broken > 0 ? (
              <p className="mt-2 text-[10px] text-amber-700" data-material-broken="">
                另有 {broken} 条素材读不出来，未计入列表。
              </p>
            ) : null}
          </div>
        </>
      )}

      <SourcePeekSheet
        open={peek !== null}
        title={peek?.title ?? ""}
        body={peek?.body ?? ""}
        onClose={() => setPeek(null)}
      />
    </div>
  );
}
