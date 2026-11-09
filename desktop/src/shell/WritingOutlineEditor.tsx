// 大纲编辑器（OD-WT-611）。
//
// 三栏：左章节树、中编辑区、右素材映射。确认前把模型返回的大纲按字段拆开编辑，
// 确认时仍提交同一个 JSON 结构给 606；确认后整块锁定，要换稿必须显式点「重新生成」
// 并经过应用内确认弹窗，避免一次误点覆盖已确认大纲。
//
// 边界：只调用 606 的三条 IPC 与 605 只读素材候选；模型调用走既有 host.submitTask，
// 不新增通信层；原文预览复用 SourcePeekSheet。

import { useEffect, useMemo, useState } from "react";

import { host, shell } from "../api";
import { Icon } from "../components/Icon";
import {
  TERMINAL_STATES,
  type WritingCollectItem,
  type WritingOutline,
  type WritingOutlineConfirmed,
  type WritingOutlineSection,
} from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { SourcePeekSheet } from "./SourcePeekSheet";

type Mode = "empty" | "draft" | "confirmed";
type Phase = "idle" | "prepare" | "model" | "confirm";
type Peek = { title: string; body: string };

const CARD =
  "bg-white/78 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "neu-inset-field w-full rounded-lg px-2.5 py-1.5 text-[11.5px] leading-relaxed text-on-surface outline-none placeholder:text-outline/70 disabled:opacity-70";
const SECTION_BUTTON =
  "w-full rounded-xl border px-2.5 py-2 text-left transition ";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function errText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isMissingOutline(message: string): boolean {
  return message.includes("尚未确认大纲") || message.includes("outline.md");
}

/** 模型可能把 JSON 包在代码块里；解析失败时退回首尾大括号之间。 */
function parseJsonLoose(text: string): unknown {
  const trimmed = text.trim();
  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const body = fenced ? fenced[1].trim() : trimmed;
  try {
    return JSON.parse(body);
  } catch {
    const start = body.indexOf("{");
    const end = body.lastIndexOf("}");
    if (start >= 0 && end > start) return JSON.parse(body.slice(start, end + 1));
    throw new Error("模型输出不是合法 JSON。");
  }
}

function stringList(value: unknown, label: string): string[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(label + "必须是数组。");
  return value.map((entry) => {
    if (typeof entry !== "string") throw new Error(label + "只能包含文字。");
    return entry.trim();
  });
}

function parseOutlineResult(text: string): WritingOutline {
  const raw = parseJsonLoose(text);
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error("大纲结果不是 JSON 对象。");
  }
  const obj = raw as Record<string, unknown>;
  if (typeof obj.title !== "string" || !obj.title.trim()) throw new Error("大纲缺少标题。");
  if (typeof obj.writing_purpose !== "string" || !obj.writing_purpose.trim()) {
    throw new Error("大纲缺少写作目的。");
  }
  if (!Array.isArray(obj.sections) || obj.sections.length === 0) {
    throw new Error("大纲 sections 不能为空。");
  }
  const sections = obj.sections.map((entry, index): WritingOutlineSection => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new Error("大纲第 " + (index + 1) + " 章不是对象。");
    }
    const section = entry as Record<string, unknown>;
    if (typeof section.heading !== "string" || !section.heading.trim()) {
      throw new Error("大纲第 " + (index + 1) + " 章缺少标题。");
    }
    if (typeof section.focus !== "string" || !section.focus.trim()) {
      throw new Error("大纲第 " + (index + 1) + " 章缺少写作重点。");
    }
    const no = typeof section.no === "number" && Number.isInteger(section.no) && section.no > 0
      ? section.no
      : index + 1;
    const words = typeof section.expected_words === "number" && Number.isFinite(section.expected_words)
      ? Math.round(section.expected_words)
      : 800;
    return {
      no,
      heading: section.heading.trim(),
      sub_headings: stringList(section.sub_headings, "小节标题"),
      focus: section.focus.trim(),
      material_ids: stringList(section.material_ids, "素材编号"),
      knowledge_ids: stringList(section.knowledge_ids, "知识编号"),
      expected_words: words,
    };
  });
  return {
    title: obj.title.trim(),
    writing_purpose: obj.writing_purpose.trim(),
    core_points: stringList(obj.core_points, "核心观点"),
    sections,
  };
}

function normalizeOutline(outline: WritingOutline): WritingOutline {
  return {
    title: outline.title.trim(),
    writing_purpose: outline.writing_purpose.trim(),
    core_points: outline.core_points.map((point) => point.trim()).filter(Boolean),
    sections: outline.sections.map((section, index) => ({
      no: section.no > 0 ? section.no : index + 1,
      heading: section.heading.trim(),
      sub_headings: section.sub_headings.map((value) => value.trim()).filter(Boolean),
      focus: section.focus.trim(),
      material_ids: section.material_ids.map((value) => value.trim()).filter(Boolean),
      knowledge_ids: section.knowledge_ids.map((value) => value.trim()).filter(Boolean),
      expected_words: Math.round(section.expected_words),
    })),
  };
}

/** 606 只返回提示词；模型调用与等待仍由界面侧承担。 */
async function runOutlinePrompt(prompt: string, onTick: (seconds: number) => void): Promise<string> {
  const workspace = await shell.prepareWorkspaceDir();
  if (!workspace.ok || !workspace.workspace_dir) {
    throw new Error(workspace.error || "宿主未返回可用工作目录。");
  }
  const receipt = await host.submitTask({
    promptText: prompt,
    intent: "material_writing",
    workspaceDir: workspace.workspace_dir,
  });
  let status = await host.taskStatus(receipt.omni_task_id);
  for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
    await sleep(1000);
    status = await host.taskStatus(receipt.omni_task_id);
    if (i % 10 === 0) onTick(i + 1);
  }
  if (status.status !== "succeeded") {
    throw new Error(
      "大纲生成未完成：" + status.status + (status.note ? " · " + status.note : ""),
    );
  }
  const result = await host.taskResult(receipt.omni_task_id);
  if (!result.content.trim()) throw new Error("模型没有返回大纲内容。");
  return result.content;
}

function splitIds(value: string): string[] {
  return value.split(/[,，、]+/).map((id) => id.trim()).filter(Boolean);
}

/** 606 的生成/确认门禁只接受这两个状态；其余状态只能说明锁定原因。 */
function outlineStageOpen(status: string): boolean {
  return status === "素材收集中" || status === "大纲待确认";
}

function StatusChip({ mode }: { mode: Mode }) {
  if (mode === "confirmed") {
    return (
      <span
        className="shrink-0 rounded-full bg-secondary-container/70 px-2 py-0.5 text-[10px] text-on-secondary-container"
        data-outline-badge="confirmed"
      >
        用户已确认
      </span>
    );
  }
  if (mode === "draft") {
    return (
      <span
        className="shrink-0 rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] text-amber-700"
        data-outline-badge="ai"
      >
        AI 建议
      </span>
    );
  }
  return (
    <span className="shrink-0 rounded-full bg-surface-container px-2 py-0.5 text-[10px] text-outline">
      未生成
    </span>
  );
}

function LabeledField({
  label,
  value,
  onChange,
  disabled,
  multiline = false,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
  multiline?: boolean;
  placeholder?: string;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-[10px] text-outline">{label}</span>
      {multiline ? (
        <textarea
          className={FIELD + " min-h-[64px] resize-y"}
          value={value}
          disabled={disabled}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
        />
      ) : (
        <input
          className={FIELD}
          value={value}
          disabled={disabled}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
    </label>
  );
}

export interface WritingOutlineEditorProps {
  writingId: string;
  /** 用户确认大纲后回调；父页面可据此刷新任务状态或页签。 */
  onOutlineConfirmed?: (confirmed: WritingOutlineConfirmed) => void;
  className?: string;
}

/**
 * 大纲编辑器：章节树 + JSON 字段化编辑 + 本章素材映射。
 *
 * export 接口：<WritingOutlineEditor writingId="..." onOutlineConfirmed={...} />
 */
export function WritingOutlineEditor({
  writingId,
  onOutlineConfirmed,
  className,
}: WritingOutlineEditorProps) {
  const [mode, setMode] = useState<Mode>("empty");
  const [phase, setPhase] = useState<Phase>("idle");
  const [outline, setOutline] = useState<WritingOutline | null>(null);
  const [confirmed, setConfirmed] = useState<WritingOutlineConfirmed | null>(null);
  const [selectedNo, setSelectedNo] = useState<number | null>(null);
  const [collectItems, setCollectItems] = useState<WritingCollectItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [peek, setPeek] = useState<Peek | null>(null);
  const [regenOpen, setRegenOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setLoadError(null);
    setMessage("");
    setConfirmed(null);
    setOutline(null);
    setSelectedNo(null);
    setMode("empty");
    shell
      .writingOutlineGet(writingId)
      .then((value) => {
        if (!alive) return;
        setConfirmed(value);
        setOutline(value.outline);
        setSelectedNo(value.outline.sections[0]?.no ?? null);
        setMode("confirmed");
      })
      .catch((error) => {
        if (!alive) return;
        const text = errText(error);
        if (!isMissingOutline(text)) setLoadError(text);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    shell
      .writingCollectMaterials({ writing_id: writingId })
      .then((response) => {
        if (alive) setCollectItems(response.items);
      })
      .catch(() => {
        if (alive) setCollectItems([]);
      });
    return () => {
      alive = false;
    };
  }, [writingId]);

  const selected = useMemo(() => {
    if (!outline) return null;
    return outline.sections.find((section) => section.no === selectedNo) ?? outline.sections[0] ?? null;
  }, [outline, selectedNo]);

  const materialIndex = useMemo(() => {
    const map = new Map<string, WritingCollectItem>();
    for (const item of collectItems) {
      if (item.kind === "material" && item.material_id) map.set(item.material_id, item);
    }
    return map;
  }, [collectItems]);

  const totalWords = useMemo(
    () => (outline ? outline.sections.reduce((sum, section) => sum + section.expected_words, 0) : 0),
    [outline],
  );

  const locked = mode === "confirmed";
  const busy = phase !== "idle";

  function patchOutline(patch: Partial<WritingOutline>) {
    setOutline((previous) => (previous ? { ...previous, ...patch } : previous));
  }

  function patchSection(no: number, patch: Partial<WritingOutlineSection>) {
    setOutline((previous) =>
      previous
        ? {
            ...previous,
            sections: previous.sections.map((section) =>
              section.no === no ? { ...section, ...patch } : section,
            ),
          }
        : previous,
    );
  }

  function addSection() {
    if (!outline || locked) return;
    const no = outline.sections.reduce((max, section) => Math.max(max, section.no), 0) + 1;
    const section: WritingOutlineSection = {
      no,
      heading: "新章节",
      sub_headings: [],
      focus: "",
      material_ids: [],
      knowledge_ids: [],
      expected_words: 800,
    };
    setOutline({ ...outline, sections: [...outline.sections, section] });
    setSelectedNo(no);
  }

  function removeSection(no: number) {
    if (!outline || locked || outline.sections.length <= 1) return;
    const index = outline.sections.findIndex((section) => section.no === no);
    const remaining = outline.sections
      .filter((section) => section.no !== no)
      .map((section, position) => ({ ...section, no: position + 1 }));
    setOutline({ ...outline, sections: remaining });
    const next = remaining[Math.min(index, remaining.length - 1)];
    setSelectedNo(next?.no ?? null);
  }

  async function generateOutline() {
    if (busy) return;
    const previousMode = mode;
    if (
      previousMode === "confirmed" &&
      confirmed &&
      confirmed.status !== "大纲待确认" &&
      confirmed.status !== "素材收集中"
    ) {
      setMessage("当前任务状态为「" + confirmed.status + "」，大纲已锁定，无法重新生成。");
      return;
    }
    setPhase("prepare");
    setMessage("正在组装大纲提示词…");
    try {
      const prompt = await shell.writingOutlineGenerate({ writing_id: writingId });
      setPhase("model");
      setMessage(
        "模型生成中… " + prompt.material_count + " 条素材 · " + prompt.knowledge_count + " 条知识",
      );
      const text = await runOutlinePrompt(prompt.prompt, (seconds) =>
        setMessage("模型生成大纲中… " + seconds + " 秒"),
      );
      const parsed = parseOutlineResult(text);
      setOutline(parsed);
      setSelectedNo(parsed.sections[0]?.no ?? null);
      setMode("draft");
      setMessage("已生成 " + parsed.sections.length + " 章；确认前可直接修改。");
    } catch (error) {
      setMessage("生成失败：" + errText(error));
      if (previousMode === "confirmed") setMode("confirmed");
    } finally {
      setPhase("idle");
    }
  }

  function requestGenerate() {
    if (busy) return;
    if (mode === "empty") {
      void generateOutline();
      return;
    }
    setRegenOpen(true);
  }

  async function confirmOutline() {
    if (!outline || busy || locked) return;
    const normalized = normalizeOutline(outline);
    if (!normalized.title) {
      setMessage("标题不能为空。");
      return;
    }
    if (!normalized.writing_purpose) {
      setMessage("写作目的不能为空。");
      return;
    }
    if (normalized.core_points.length === 0) {
      setMessage("核心观点至少填写一条。");
      return;
    }
    if (normalized.sections.some((section) => !section.heading || !section.focus)) {
      setMessage("每章都必须有标题与写作重点。");
      return;
    }
    if (
      normalized.sections.some(
        (section) => !Number.isFinite(section.expected_words) || section.expected_words <= 0,
      )
    ) {
      setMessage("预计字数必须是正整数。");
      return;
    }
    setPhase("confirm");
    setMessage("正在确认大纲…");
    try {
      const result = await shell.writingOutlineConfirm({
        writing_id: writingId,
        outline: normalized,
      });
      setConfirmed(result);
      setOutline(result.outline);
      setMode("confirmed");
      setSelectedNo((previous) =>
        result.outline.sections.some((section) => section.no === previous)
          ? previous
          : result.outline.sections[0]?.no ?? null,
      );
      setMessage("大纲已确认，写入版本 " + result.version + "。");
      onOutlineConfirmed?.(result);
    } catch (error) {
      setMessage("确认失败：" + errText(error));
    } finally {
      setPhase("idle");
    }
  }

  async function openMaterial(id: string) {
    try {
      const detail = await shell.materialGet(id);
      const parts = [detail.material.content.trim()];
      const location = detail.source.location ? "（" + detail.source.location + "）" : "";
      if (detail.source.quote.trim()) {
        parts.push("来源原文" + location + "：\n" + detail.source.quote.trim());
      }
      setPeek({ title: detail.material.title, body: parts.filter(Boolean).join("\n\n") });
    } catch (error) {
      setMessage("素材原文不可读：" + errText(error));
    }
  }

  async function openKnowledge(id: string) {
    try {
      const note = await shell.knowledgeVaultReadNote(id);
      setPeek({ title: note.file_name || id, body: note.content });
    } catch (error) {
      setMessage("知识原文不可读：" + errText(error));
    }
  }

  const mappedMaterials = selected
    ? selected.material_ids.map((id) => ({ id, item: materialIndex.get(id) ?? null }))
    : [];
  const mappedKnowledge = selected ? selected.knowledge_ids : [];

  return (
    <section
      aria-label="大纲编辑器"
      className={"flex h-full min-h-0 min-w-0 flex-col " + (className ?? "")}
      data-writing-outline-editor=""
      data-outline-mode={mode}
    >
      <header className="flex items-center gap-2 pb-2">
        <h2 className="font-headline-md text-[16px] font-semibold text-on-surface">大纲</h2>
        <StatusChip mode={mode} />
        {locked && confirmed ? (
          <span className="mono text-[10px] text-outline">v{confirmed.version}</span>
        ) : null}
        <span className="mono ml-auto text-[10.5px] text-outline">
          {outline ? outline.sections.length + " 章 · " + totalWords + " 字" : loading ? "读取中" : "—"}
        </span>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 overflow-hidden xl:grid-cols-[220px_minmax(0,1fr)_260px]">
        {/* 左：章节树。 */}
        <aside className={CARD + " flex min-h-0 flex-col overflow-hidden p-2.5"} aria-label="章节树">
          <div className="mb-2 flex items-center gap-2 px-1">
            <span className="text-[11px] font-medium text-on-surface">章节</span>
            {!locked ? (
              <button
                type="button"
                className="ml-auto inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] text-primary hover:bg-primary/10 disabled:opacity-40"
                onClick={addSection}
                disabled={!outline || busy}
                data-outline-add=""
              >
                <Icon name="plus" className="h-3 w-3" />
                章
              </button>
            ) : null}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {outline ? (
              outline.sections.map((section) => {
                const active = selected?.no === section.no;
                return (
                  <button
                    key={section.no}
                    type="button"
                    onClick={() => setSelectedNo(section.no)}
                    className={
                      SECTION_BUTTON +
                      (active
                        ? "border-primary/20 bg-primary/5"
                        : "border-transparent hover:bg-surface-container-low")
                    }
                    data-outline-section={section.no}
                    data-outline-section-active={active ? "true" : "false"}
                  >
                    <span className="flex items-baseline gap-1.5">
                      <span className="mono text-[10px] text-outline">{section.no}</span>
                      <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-on-surface">
                        {section.heading || "未命名章节"}
                      </span>
                    </span>
                    <span className="mt-0.5 block text-[10px] text-outline">
                      {section.expected_words} 字
                      {section.material_ids.length > 0 ? " · 素材 " + section.material_ids.length : ""}
                    </span>
                    {section.sub_headings.length > 0 ? (
                      <span className="mt-0.5 block truncate text-[10px] text-on-surface-variant">
                        {section.sub_headings.join(" · ")}
                      </span>
                    ) : null}
                    <span className="mt-0.5 block truncate text-[10px] text-on-surface-variant">
                      {section.focus || "未填写重点"}
                    </span>
                    <span className="mt-0.5 block truncate text-[10px] text-outline">
                      素材 {section.material_ids.length} · 知识 {section.knowledge_ids.length}
                    </span>
                  </button>
                );
              })
            ) : (
              <p className="px-1 py-2 text-[11px] text-outline">{loading ? "读取中…" : "还没有章节。"}</p>
            )}
          </div>
        </aside>

        {/* 中：编辑区。 */}
        <main className={CARD + " flex min-h-0 flex-col overflow-hidden"}>
          <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3.5">
            {loadError ? (
              <div className="rounded-xl bg-error-container/50 px-3 py-2 text-[11.5px] text-on-error-container">
                {loadError}
              </div>
            ) : null}
            {!outline ? (
              <div className="flex h-full flex-col items-start justify-center gap-3 px-2">
                <p className="text-[13px] text-on-surface-variant">
                  {loading ? "正在读取大纲…" : loadError ? "大纲读取失败。" : "还没有大纲。"}
                </p>
                {!loading ? (
                  <button
                    type="button"
                    onClick={requestGenerate}
                    disabled={busy}
                    className="inline-flex items-center gap-1.5 rounded-full bg-primary px-3.5 py-1.5 text-[11.5px] text-on-primary hover:opacity-90 disabled:opacity-50"
                    data-outline-generate=""
                  >
                    <Icon name="spark" className="h-3.5 w-3.5" />
                    生成大纲
                  </button>
                ) : null}
              </div>
            ) : (
              <div className="space-y-4">
                <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                  <LabeledField
                    label="标题"
                    value={outline.title}
                    disabled={locked}
                    onChange={(value) => patchOutline({ title: value })}
                  />
                  <LabeledField
                    label="写作目的"
                    value={outline.writing_purpose}
                    disabled={locked}
                    onChange={(value) => patchOutline({ writing_purpose: value })}
                  />
                </div>
                <LabeledField
                  label="核心观点（一行一条）"
                  value={outline.core_points.join("\n")}
                  disabled={locked}
                  multiline
                  onChange={(value) => patchOutline({ core_points: value.split("\n") })}
                />

                {selected ? (
                  <div className="border-t border-surface-variant/30 pt-3" data-outline-detail={selected.no}>
                    <div className="mb-2 flex items-baseline gap-2">
                      <span className="text-[11px] font-medium text-on-surface">
                        第 {selected.no} 章
                      </span>
                      <span className="mono text-[10px] text-outline">{selected.expected_words} 字</span>
                      {!locked && outline.sections.length > 1 ? (
                        <button
                          type="button"
                          onClick={() => removeSection(selected.no)}
                          className="ml-auto rounded-full px-2 py-0.5 text-[10px] text-error hover:bg-error-container/40"
                          data-outline-remove=""
                        >
                          删除本章
                        </button>
                      ) : null}
                    </div>
                    <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                      <LabeledField
                        label="章节标题"
                        value={selected.heading}
                        disabled={locked}
                        onChange={(value) => patchSection(selected.no, { heading: value })}
                      />
                      <label className="block">
                        <span className="mb-1 block text-[10px] text-outline">预计字数</span>
                        <input
                          type="number"
                          min={1}
                          className={FIELD}
                          value={selected.expected_words}
                          disabled={locked}
                          onChange={(event) =>
                            patchSection(selected.no, {
                              expected_words: Number(event.target.value),
                            })
                          }
                        />
                      </label>
                    </div>
                    <div className="mt-3 space-y-3">
                      <LabeledField
                        label="小节标题（一行一条）"
                        value={selected.sub_headings.join("\n")}
                        disabled={locked}
                        multiline
                        onChange={(value) => patchSection(selected.no, { sub_headings: value.split("\n") })}
                      />
                      <LabeledField
                        label="写作重点"
                        value={selected.focus}
                        disabled={locked}
                        multiline
                        onChange={(value) => patchSection(selected.no, { focus: value })}
                      />
                      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                        <LabeledField
                          label="素材编号（逗号分隔）"
                          value={selected.material_ids.join(", ")}
                          disabled={locked}
                          onChange={(value) =>
                            patchSection(selected.no, { material_ids: splitIds(value) })
                          }
                        />
                        <LabeledField
                          label="知识编号（逗号分隔）"
                          value={selected.knowledge_ids.join(", ")}
                          disabled={locked}
                          onChange={(value) =>
                            patchSection(selected.no, { knowledge_ids: splitIds(value) })
                          }
                        />
                      </div>
                    </div>
                  </div>
                ) : null}
              </div>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2 border-t border-surface-variant/30 px-4 py-2.5">
            {message ? (
              <p className="min-w-0 flex-1 truncate text-[10.5px] text-on-surface-variant" title={message}>
                {message}
              </p>
            ) : <span className="flex-1" />}
            {outline ? (
              <>
                {locked ? (
                  <button
                    type="button"
                    onClick={requestGenerate}
                    disabled={busy}
                    className="inline-flex items-center gap-1.5 rounded-full border border-outline-variant/60 px-3 py-1 text-[11px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-50"
                    data-outline-regenerate=""
                  >
                    <Icon name="refresh" className="h-3.5 w-3.5" />
                    重新生成
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => void confirmOutline()}
                    disabled={busy}
                    className="inline-flex items-center gap-1.5 rounded-full bg-on-surface px-3.5 py-1.5 text-[11px] text-inverse-on-surface hover:opacity-90 disabled:opacity-50"
                    data-outline-confirm=""
                  >
                    <Icon name="check" className="h-3.5 w-3.5" />
                    确认大纲
                  </button>
                )}
              </>
            ) : null}
          </div>
        </main>

        {/* 右：本章映射。 */}
        <aside className={CARD + " flex min-h-0 flex-col overflow-hidden p-2.5"} aria-label="素材映射">
          <div className="mb-2 flex items-baseline gap-2 px-1">
            <span className="text-[11px] font-medium text-on-surface">本章素材</span>
            <span className="mono text-[10px] text-outline">
              {mappedMaterials.length + mappedKnowledge.length}
            </span>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {!selected ? (
              <p className="px-1 py-2 text-[11px] text-outline">选择章节后显示映射。</p>
            ) : mappedMaterials.length + mappedKnowledge.length === 0 ? (
              <p className="px-1 py-2 text-[11px] text-outline">本章尚未映射素材。</p>
            ) : (
              <div className="space-y-1">
                {mappedMaterials.map(({ id, item }) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => void openMaterial(id)}
                    className="w-full rounded-lg border border-surface-variant/25 bg-surface-container-low/40 px-2 py-1.5 text-left hover:border-primary/25 hover:bg-surface-container-low"
                    data-outline-material={id}
                  >
                    <span className="block truncate text-[11px] font-medium text-on-surface">
                      {item?.title ?? id}
                    </span>
                    <span className="mt-0.5 block truncate text-[10px] text-outline">
                      素材 {id}
                    </span>
                  </button>
                ))}
                {mappedKnowledge.map((id) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => void openKnowledge(id)}
                    className="w-full rounded-lg border border-surface-variant/25 bg-surface-container-low/40 px-2 py-1.5 text-left hover:border-primary/25 hover:bg-surface-container-low"
                    data-outline-knowledge={id}
                  >
                    <span className="block truncate text-[11px] font-medium text-on-surface">{id}</span>
                    <span className="mt-0.5 block text-[10px] text-outline">知识依据</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </aside>
      </div>

      <SourcePeekSheet
        open={peek !== null}
        title={peek?.title ?? ""}
        body={peek?.body ?? ""}
        onClose={() => setPeek(null)}
      />
      <ConfirmDialog
        open={regenOpen}
        title={
          mode === "confirmed" && confirmed && !outlineStageOpen(confirmed.status)
            ? "大纲已锁定"
            : "重新生成大纲"
        }
        description={
          mode === "confirmed"
            ? confirmed && !outlineStageOpen(confirmed.status)
              ? "当前任务状态为「" + confirmed.status + "」；重新生成大纲需要新建写作任务，当前大纲不会被修改。"
              : "会向模型请求一份新的大纲建议；当前已确认大纲不会被直接覆盖，新稿仍需再次确认。"
            : "当前编辑内容会被新的 AI 建议替换。"
        }
        confirmLabel={
          mode === "confirmed" && confirmed && !outlineStageOpen(confirmed.status)
            ? "知道了"
            : "重新生成"
        }
        onCancel={() => setRegenOpen(false)}
        onConfirm={() => {
          setRegenOpen(false);
          if (!(mode === "confirmed" && confirmed && !outlineStageOpen(confirmed.status))) {
            void generateOutline();
          }
        }}
        testId="outline-regenerate-confirm"
      />
    </section>
  );
}
