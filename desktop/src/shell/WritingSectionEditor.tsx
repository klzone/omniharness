// 分章写作界面（OD-WT-611）。
//
// 三栏：左章节选择、中章节稿编辑、右本章素材与引用。章节生成与修订都先让
// 607 返回提示词，由界面侧走既有 host.submitTask 调模型；结果只作「AI 建议」
// 预览，用户点确认后才经 writingSectionConfirm 写入正文与快照。确认后自动
// 带四分法标注调用 609 核验（annotations 随车传入），失败只提示、不阻断确认。
//
// 右栏引用只展示本会话拿到或刚确认写入的那批 603 记录来源（607 get 未暴露
// 已落盘 citations 读取），不伪造看不到的历史明细。

import { useEffect, useMemo, useState } from "react";

import { host, shell } from "../api";
import { Icon } from "../components/Icon";
import {
  TERMINAL_STATES,
  type FactCheckAnnotationInput,
  type WritingCollectResponse,
  type WritingOutlineConfirmed,
  type WritingReviseKind,
  type WritingSectionCitation,
  type WritingSectionConfirmed,
  type WritingSectionDraft,
  type WritingSectionSegment,
  type WritingSectionState,
} from "../types";
import { SourcePeekSheet } from "./SourcePeekSheet";

type Phase = "idle" | "prepare" | "model" | "confirm" | "revise";
type DraftOrigin = "generated" | "revised";
type Peek = { title: string; body: string };

const CARD =
  "bg-white/78 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "neu-inset-field w-full rounded-lg px-2.5 py-1.5 text-[11.5px] leading-relaxed text-on-surface outline-none placeholder:text-outline/70 disabled:opacity-70";

const REVISE_KINDS: WritingReviseKind[] = ["精简", "扩展", "语气", "术语统一", "消除重复"];
const SEGMENT_KINDS = new Set(["事实", "分析", "判断", "建议"]);

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function errText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isMissingOutline(message: string): boolean {
  return message.includes("尚未确认大纲") || message.includes("outline.md");
}

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

function parseCitations(value: unknown): WritingSectionCitation[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error("章节稿 citations 必须是数组。");
  return value.map((entry, index) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new Error("citations[" + index + "] 不是对象。");
    }
    const citation = entry as Record<string, unknown>;
    if (citation.source_kind !== "material" && citation.source_kind !== "knowledge") {
      throw new Error("citations[" + index + "] 来源类型不合法。");
    }
    const sourceId = typeof citation.source_id === "string" ? citation.source_id.trim() : "";
    const quote = typeof citation.quote === "string" ? citation.quote.trim() : "";
    if (!sourceId) throw new Error("citations[" + index + "] 缺少来源编号。");
    if (!quote) throw new Error("citations[" + index + "] 缺少引用原文。");
    return { source_kind: citation.source_kind, source_id: sourceId, quote };
  });
}

function parseSegments(value: unknown): WritingSectionSegment[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error("章节稿 segments 必须是数组。");
  return value.map((entry, index) => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
      throw new Error("segments[" + index + "] 不是对象。");
    }
    const segment = entry as Record<string, unknown>;
    const kind = typeof segment.kind === "string" ? segment.kind.trim() : "";
    if (!SEGMENT_KINDS.has(kind)) {
      throw new Error("segments[" + index + "] 类型必须是事实/分析/判断/建议。");
    }
    const text = typeof segment.text === "string" ? segment.text.trim() : "";
    if (!text) throw new Error("segments[" + index + "] 缺少对应正文片段。");
    const quote = typeof segment.quote === "string" && segment.quote.trim() ? segment.quote.trim() : null;
    return {
      kind: kind as WritingSectionSegment["kind"],
      text,
      quote,
    };
  });
}

function parseSectionDraft(text: string, sectionNo: number): WritingSectionDraft {
  const raw = parseJsonLoose(text);
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error("章节稿不是 JSON 对象。");
  }
  const obj = raw as Record<string, unknown>;
  const content = typeof obj.content === "string" ? obj.content.trim() : "";
  if (!content) throw new Error("章节稿正文为空。");
  return {
    section_no: sectionNo,
    content,
    citations: parseCitations(obj.citations),
    segments: parseSegments(obj.segments),
  };
}

async function runSectionPrompt(prompt: string, onTick: (seconds: number) => void): Promise<string> {
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
      "章节生成未完成：" + status.status + (status.note ? " · " + status.note : ""),
    );
  }
  const result = await host.taskResult(receipt.omni_task_id);
  if (!result.content.trim()) throw new Error("模型没有返回章节稿。");
  return result.content;
}

/** 607 的段落口径：空行分段。 */
function paragraphsOf(content: string | null | undefined): string[] {
  if (!content) return [];
  return content
    .split("\n\n")
    .map((paragraph) => paragraph.trim())
    .filter(Boolean);
}

function annotationsFor(draft: WritingSectionDraft): FactCheckAnnotationInput[] {
  return draft.segments
    .filter((segment) => segment.text.trim())
    .map((segment) => ({
      section_no: draft.section_no,
      kind: segment.kind,
      text: segment.text.trim(),
    }));
}

function truncate(value: string, limit: number): string {
  return value.length <= limit ? value : value.slice(0, limit) + "…";
}

function SegmentPill({ segment }: { segment: WritingSectionSegment }) {
  return (
    <span
      className="inline-flex max-w-full items-center gap-1 rounded-full bg-surface-container px-1.5 py-0.5 text-[9.5px] text-on-surface-variant"
      data-segment-kind={segment.kind}
    >
      <span className="shrink-0 text-primary">{segment.kind}</span>
      <span className="truncate">{truncate(segment.text, 24)}</span>
    </span>
  );
}

export interface WritingSectionEditorProps {
  writingId: string;
  /** 章节确认后回调；父页面可刷新任务状态或核验页签。 */
  onSectionConfirmed?: (confirmed: WritingSectionConfirmed) => void;
  className?: string;
}

/**
 * 分章写作界面：章节选择 + 生成/修订预览 + 本章素材与引用。
 *
 * export 接口：<WritingSectionEditor writingId="..." onSectionConfirmed={...} />
 */
export function WritingSectionEditor({
  writingId,
  onSectionConfirmed,
  className,
}: WritingSectionEditorProps) {
  const [outline, setOutline] = useState<WritingOutlineConfirmed | null>(null);
  const [sectionStates, setSectionStates] = useState<Record<number, WritingSectionState>>({});
  const [statesLoaded, setStatesLoaded] = useState(false);
  const [sectionLoading, setSectionLoading] = useState(false);
  const [selectedNo, setSelectedNo] = useState<number | null>(null);
  const [collect, setCollect] = useState<WritingCollectResponse | null>(null);
  const [collectLoaded, setCollectLoaded] = useState(false);
  const [collectError, setCollectError] = useState<string | null>(null);
  const [draft, setDraft] = useState<WritingSectionDraft | null>(null);
  const [draftOrigin, setDraftOrigin] = useState<DraftOrigin>("generated");
  const [confirmedCitations, setConfirmedCitations] = useState<Record<number, WritingSectionCitation[]>>({});
  const [phase, setPhase] = useState<Phase>("idle");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [factcheckNote, setFactcheckNote] = useState<string | null>(null);
  const [reviseKind, setReviseKind] = useState<WritingReviseKind>("精简");
  const [reviseParagraph, setReviseParagraph] = useState("whole");
  const [reviseInstruction, setReviseInstruction] = useState("");
  const [peek, setPeek] = useState<Peek | null>(null);
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setLoadError(null);
    setMessage("");
    setOutline(null);
    setSectionStates({});
    setStatesLoaded(false);
    setSectionLoading(false);
    setSelectedNo(null);
    setCollect(null);
    setCollectLoaded(false);
    setCollectError(null);
    setDraft(null);
    setConfirmedCitations({});
    setFactcheckNote(null);
    void (async () => {
      try {
        let confirmedOutline: WritingOutlineConfirmed | null = null;
        try {
          confirmedOutline = await shell.writingOutlineGet(writingId);
        } catch (error) {
          const text = errText(error);
          if (!alive) return;
          if (!isMissingOutline(text)) setLoadError(text);
        }
        if (!alive) return;
        if (!confirmedOutline) return;
        setOutline(confirmedOutline);
        setSelectedNo(confirmedOutline.outline.sections[0]?.no ?? null);
        const results = await Promise.allSettled(
          confirmedOutline.outline.sections.map((section) =>
            shell.writingSectionGet(writingId, section.no),
          ),
        );
        if (!alive) return;
        const next: Record<number, WritingSectionState> = {};
        for (const result of results) {
          if (result.status === "fulfilled") next[result.value.section_no] = result.value;
        }
        setSectionStates(next);
        setStatesLoaded(true);
      } finally {
        if (alive) setLoading(false);
      }
    })();
    shell
      .writingCollectMaterials({ writing_id: writingId })
      .then((response) => {
        if (!alive) return;
        setCollect(response);
        setCollectError(null);
        setCollectLoaded(true);
      })
      .catch((error) => {
        if (!alive) return;
        setCollect(null);
        setCollectError(errText(error));
        setCollectLoaded(true);
      });
    return () => {
      alive = false;
    };
  }, [writingId]);

  useEffect(() => {
    setDraft(null);
    setFactcheckNote(null);
    setMessage("");
    setReviseInstruction("");
    setReviseParagraph("whole");
  }, [selectedNo]);

  useEffect(() => {
    if (selectedNo === null) {
      setSectionLoading(false);
      return;
    }
    let alive = true;
    setSectionLoading(true);
    shell
      .writingSectionGet(writingId, selectedNo)
      .then((state) => {
        if (!alive) return;
        setSectionStates((previous) => ({ ...previous, [state.section_no]: state }));
      })
      .catch((error) => {
        if (alive) setMessage("章节状态读取失败：" + errText(error));
      })
      .finally(() => {
        if (alive) setSectionLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [writingId, selectedNo, reloadTick]);

  const selected = useMemo(() => {
    if (!outline) return null;
    return outline.outline.sections.find((section) => section.no === selectedNo) ?? null;
  }, [outline, selectedNo]);

  const current = selectedNo !== null ? sectionStates[selectedNo] ?? null : null;
  const confirmedContent = current?.content?.trim() ? current.content : null;
  const paragraphs = useMemo(() => paragraphsOf(confirmedContent), [confirmedContent]);

  const effectiveMaterialIds = useMemo(() => {
    if (!selected) return [];
    if (selected.material_ids.length > 0) return selected.material_ids;
    return collect?.selected_material_ids ?? [];
  }, [selected, collect]);

  const effectiveKnowledgeIds = useMemo(() => {
    if (!selected) return [];
    if (selected.knowledge_ids.length > 0) return selected.knowledge_ids;
    return collect?.selected_knowledge_ids ?? [];
  }, [selected, collect]);

  const materialRows = useMemo(
    () =>
      effectiveMaterialIds.map((id) => {
        const hit = collect?.items.find(
          (item) => item.kind === "material" && item.material_id === id,
        );
        return { id, title: hit?.title ?? id, summary: hit?.content_summary ?? "" };
      }),
    [effectiveMaterialIds, collect],
  );

  const hasKnownCitations =
    selectedNo !== null && Object.prototype.hasOwnProperty.call(confirmedCitations, selectedNo);
  const knownCitations = draft?.citations ?? (hasKnownCitations && selectedNo !== null
    ? confirmedCitations[selectedNo]
    : null);
  const activeCitations = knownCitations ?? [];
  const citationsUnknown = draft === null && knownCitations === null && confirmedContent !== null;
  const fallbackMaterials =
    selected !== null && selected.material_ids.length === 0 && effectiveMaterialIds.length > 0;

  const busy = phase !== "idle";

  function resolveSourceTitle(citation: WritingSectionCitation): string {
    if (citation.source_kind === "material") {
      return materialRows.find((row) => row.id === citation.source_id)?.title ?? citation.source_id;
    }
    return citation.source_id;
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

  function openSource(citation: WritingSectionCitation) {
    if (citation.source_kind === "material") void openMaterial(citation.source_id);
    else void openKnowledge(citation.source_id);
  }

  async function generateSection() {
    if (!selected || busy) return;
    setPhase("prepare");
    setMessage("正在组装本章提示词…");
    try {
      const prompt = await shell.writingSectionGenerate({
        writing_id: writingId,
        section_no: selected.no,
      });
      setPhase("model");
      setMessage(
        "模型撰写第 " + prompt.section_no + " 章中… " + prompt.material_count + " 条素材 · " +
          prompt.knowledge_count + " 条知识",
      );
      const text = await runSectionPrompt(prompt.prompt, (seconds) =>
        setMessage("模型撰写第 " + prompt.section_no + " 章中… " + seconds + " 秒"),
      );
      setDraft(parseSectionDraft(text, selected.no));
      setDraftOrigin("generated");
      setFactcheckNote(null);
      setMessage("已生成第 " + selected.no + " 章建议稿；确认后才写入正文。");
    } catch (error) {
      setMessage("章节生成失败：" + errText(error));
    } finally {
      setPhase("idle");
    }
  }

  async function confirmDraft() {
    if (!draft || busy) return;
    setPhase("confirm");
    setMessage("正在确认第 " + draft.section_no + " 章…");
    const confirmedNo = draft.section_no;
    const citations = draft.citations;
    const annotations = annotationsFor(draft);
    try {
      const result = await shell.writingSectionConfirm({ writing_id: writingId, draft });
      setSectionStates((previous) => ({
        ...previous,
        [result.section_no]: {
          writing_id: writingId,
          section_no: result.section_no,
          heading: result.heading,
          content: result.content,
          confirmed: true,
          status: result.status,
          version: result.version,
          updated_at: result.updated_at,
          masked: result.masked,
          masked_id_card: result.masked_id_card,
          masked_phone: result.masked_phone,
        },
      }));
      setConfirmedCitations((previous) => ({ ...previous, [confirmedNo]: citations }));
      setDraft(null);
      setMessage(
        "第 " + result.section_no + " 章已确认，写入 " + result.citations_added + " 条引用。" +
          (result.all_sections_confirmed ? "全部章节已确认。" : ""),
      );
      try {
        onSectionConfirmed?.(result);
      } catch {
        // 父页面回调失败不应把已成功的确认回滚成失败提示。
      }
      // 609 遗留：章节四分法标注随确认稿一起送入核验；不阻断确认结果。
      try {
        const factcheck = await shell.writingFactcheckRun({
          writing_id: writingId,
          annotations,
        });
        setFactcheckNote(
          "带标注核验：新增 " + factcheck.appended + " 条，采用标注 " +
            factcheck.stats.annotations_used + " 条。",
        );
      } catch (error) {
        setFactcheckNote("自动核验未完成：" + errText(error));
      }
      setReloadTick((tick) => tick + 1);
    } catch (error) {
      setMessage("章节确认失败：" + errText(error));
    } finally {
      setPhase("idle");
    }
  }

  async function reviseSection() {
    if (!selected || !confirmedContent || busy) return;
    setPhase("revise");
    setMessage("正在组装修订提示词…");
    try {
      const prompt = await shell.writingSectionRevise({
        writing_id: writingId,
        section_no: selected.no,
        paragraph_no: reviseParagraph === "whole" ? null : Number(reviseParagraph),
        instruction_kind: reviseKind,
        instruction: reviseInstruction.trim(),
      });
      setPhase("model");
      setMessage("模型修订中… " + prompt.instruction_kind);
      const text = await runSectionPrompt(prompt.prompt, (seconds) =>
        setMessage("模型修订中… " + seconds + " 秒"),
      );
      setDraft(parseSectionDraft(text, selected.no));
      setDraftOrigin("revised");
      setFactcheckNote(null);
      setMessage("已生成修订建议；确认应用后会新建版本并保留原文。");
    } catch (error) {
      setMessage("修订失败：" + errText(error));
    } finally {
      setPhase("idle");
    }
  }

  return (
    <section
      aria-label="分章写作"
      className={"flex h-full min-h-0 min-w-0 flex-col " + (className ?? "")}
      data-writing-section-editor=""
    >
      <header className="flex items-center gap-2 pb-2">
        <h2 className="font-headline-md text-[16px] font-semibold text-on-surface">分章写作</h2>
        {selected ? (
          <span className="mono text-[10px] text-outline">
            第 {selected.no} 章 · {selected.expected_words} 字
          </span>
        ) : null}
        <span className="mono ml-auto text-[10.5px] text-outline">
          {outline ? outline.outline.sections.length + " 章" : loading ? "读取中" : "—"}
        </span>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 overflow-hidden xl:grid-cols-[210px_minmax(0,1fr)_270px]">
        {/* 左：章节选择。 */}
        <aside className={CARD + " flex min-h-0 flex-col overflow-hidden p-2.5"} aria-label="章节选择">
          <div className="mb-2 flex items-baseline gap-2 px-1">
            <span className="text-[11px] font-medium text-on-surface">章节</span>
            <span className="mono text-[10px] text-outline">
              {outline ? outline.outline.sections.length : 0}
            </span>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {outline ? (
              outline.outline.sections.map((section) => {
                const active = selectedNo === section.no;
                const state = sectionStates[section.no];
                const draftHere = active && draft !== null;
                return (
                  <button
                    key={section.no}
                    type="button"
                    onClick={() => setSelectedNo(section.no)}
                    className={
                      "w-full rounded-xl border px-2.5 py-2 text-left transition " +
                      (active
                        ? "border-primary/20 bg-primary/5"
                        : "border-transparent hover:bg-surface-container-low")
                    }
                    data-section-tree-item={section.no}
                  >
                    <span className="flex items-baseline gap-1.5">
                      <span className="mono text-[10px] text-outline">{section.no}</span>
                      <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-on-surface">
                        {section.heading}
                      </span>
                    </span>
                    <span className="mt-0.5 flex items-center gap-1 text-[9.5px]">
                      {draftHere ? (
                        <span className="rounded-full bg-amber-500/15 px-1.5 py-0.5 text-amber-700">
                          AI 建议
                        </span>
                      ) : state?.confirmed ? (
                        <span className="rounded-full bg-secondary-container/70 px-1.5 py-0.5 text-on-secondary-container">
                          已确认
                        </span>
                      ) : !statesLoaded ? (
                        <span className="text-outline">读取中</span>
                      ) : (
                        <span className="text-outline">未生成</span>
                      )}
                    </span>
                  </button>
                );
              })
            ) : (
              <p className="px-1 py-2 text-[11px] text-outline">
                {loading ? "读取中…" : "还没有章节。"}
              </p>
            )}
          </div>
        </aside>

        {/* 中：本章稿。 */}
        <main className={CARD + " flex min-h-0 flex-col overflow-hidden"}>
          <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3.5">
            {loadError ? (
              <div className="rounded-xl bg-error-container/50 px-3 py-2 text-[11.5px] text-on-error-container">
                {loadError}
              </div>
            ) : null}
            {!outline ? (
              <div className="flex h-full flex-col justify-center gap-2 px-2">
                <p className="text-[13px] text-on-surface-variant">
                  {loading ? "正在读取大纲…" : loadError ? "大纲读取失败。" : "尚未确认大纲。"}
                </p>
                {!loading && !loadError ? (
                  <p className="text-[11.5px] text-outline">确认大纲后再开始分章写作。</p>
                ) : null}
              </div>
            ) : !selected ? (
              <p className="px-2 py-3 text-[12px] text-outline">还没有章节。</p>
            ) : !current && sectionLoading ? (
              <p className="px-2 py-3 text-[12px] text-outline" data-section-loading="">
                正在读取本章…
              </p>
            ) : (
              <div className="space-y-4">
                <div className="flex flex-wrap items-baseline gap-2">
                  <span className="mono text-[10px] text-outline">{selected.no}</span>
                  <h3 className="min-w-0 flex-1 text-[14px] font-semibold leading-snug text-on-surface">
                    {selected.heading}
                  </h3>
                </div>
                <p className="text-[11px] leading-relaxed text-on-surface-variant">{selected.focus}</p>
                {selected.sub_headings.length > 0 ? (
                  <p className="text-[10.5px] text-outline">{selected.sub_headings.join(" · ")}</p>
                ) : null}

                {draft ? (
                  <div
                    className="rounded-xl border border-amber-500/25 bg-amber-500/5 px-3 py-3"
                    data-section-draft=""
                    data-draft-origin={draftOrigin}
                  >
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] text-amber-700">
                        AI 建议{draftOrigin === "revised" ? " · 修订" : ""}
                      </span>
                      <span className="mono text-[10px] text-outline">{draft.citations.length} 条引用</span>
                      <span className="mono ml-auto text-[10px] text-outline">
                        {draft.content.length} 字
                      </span>
                    </div>
                    <div className="mt-2 space-y-2">
                      {paragraphsOf(draft.content).map((paragraph, index) => (
                        <p key={index} className="whitespace-pre-wrap text-[12.5px] leading-relaxed text-on-surface">
                          {paragraph}
                        </p>
                      ))}
                    </div>
                    {draft.segments.length > 0 ? (
                      <div className="mt-2 flex flex-wrap gap-1" data-draft-segments="">
                        {draft.segments.map((segment, index) => (
                          <SegmentPill key={index} segment={segment} />
                        ))}
                      </div>
                    ) : null}
                    <div className="mt-3 flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() => void confirmDraft()}
                        disabled={busy}
                        className="inline-flex items-center gap-1.5 rounded-full bg-on-surface px-3 py-1.5 text-[11px] text-inverse-on-surface hover:opacity-90 disabled:opacity-50"
                        data-section-confirm=""
                      >
                        <Icon name="check" className="h-3.5 w-3.5" />
                        {draftOrigin === "revised" ? "应用修订" : "确认采用"}
                      </button>
                      <button
                        type="button"
                        onClick={() => setDraft(null)}
                        disabled={busy}
                        className="rounded-full px-2.5 py-1 text-[10.5px] text-outline hover:bg-surface-container disabled:opacity-50"
                        data-section-discard=""
                      >
                        丢弃
                      </button>
                    </div>
                  </div>
                ) : null}

                {confirmedContent ? (
                  <div
                    className="rounded-xl border border-surface-variant/30 bg-surface-container-low/40 px-3 py-3"
                    data-section-confirmed=""
                  >
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="rounded-full bg-secondary-container/70 px-2 py-0.5 text-[10px] text-on-secondary-container">
                        用户确认
                      </span>
                      {current ? (
                        <span className="mono text-[10px] text-outline">v{current.version}</span>
                      ) : null}
                    </div>
                    <div className="mt-2 space-y-2">
                      {paragraphs.map((paragraph, index) => (
                        <p key={index} className="whitespace-pre-wrap text-[12.5px] leading-relaxed text-on-surface">
                          {paragraph}
                        </p>
                      ))}
                    </div>
                  </div>
                ) : null}

                {!draft && !confirmedContent ? (
                  <div className="rounded-xl border border-dashed border-outline-variant/50 px-3 py-5" data-section-empty="">
                    <p className="text-[12px] text-on-surface-variant">本章还没有稿件。</p>
                    <p className="mt-1 text-[11px] text-outline">生成后先以 AI 建议预览，确认后才写入正文。</p>
                  </div>
                ) : null}

                {!draft ? (
                  <button
                    type="button"
                    onClick={() => void generateSection()}
                    disabled={busy}
                    className="inline-flex items-center gap-1.5 rounded-full bg-primary px-3.5 py-1.5 text-[11.5px] text-on-primary hover:opacity-90 disabled:opacity-50"
                    data-section-generate=""
                  >
                    <Icon name="spark" className="h-3.5 w-3.5" />
                    {confirmedContent ? "重新生成本章" : "生成本章"}
                  </button>
                ) : null}

                {confirmedContent && !draft ? (
                  <div className="border-t border-surface-variant/30 pt-3" data-section-revise-panel="">
                    <div className="mb-1.5 flex flex-wrap items-center gap-1.5">
                      <span className="text-[11px] font-medium text-on-surface">修订指令</span>
                      <span className="text-[10px] text-outline">只基于已确认正文</span>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <select
                        className={FIELD}
                        value={reviseKind}
                        disabled={busy}
                        onChange={(event) => setReviseKind(event.target.value as WritingReviseKind)}
                        data-section-revise-kind=""
                      >
                        {REVISE_KINDS.map((kind) => (
                          <option key={kind} value={kind}>
                            {kind}
                          </option>
                        ))}
                      </select>
                      <select
                        className={FIELD}
                        value={reviseParagraph}
                        disabled={busy}
                        onChange={(event) => setReviseParagraph(event.target.value)}
                        data-section-revise-scope=""
                      >
                        <option value="whole">整章</option>
                        {paragraphs.map((_paragraph, index) => (
                          <option key={index} value={String(index + 1)}>
                            第 {index + 1} 段
                          </option>
                        ))}
                      </select>
                    </div>
                    <textarea
                      className={FIELD + " mt-2 min-h-[54px] resize-y"}
                      value={reviseInstruction}
                      disabled={busy}
                      placeholder="补充说明（可选）"
                      onChange={(event) => setReviseInstruction(event.target.value)}
                      data-section-revise-instruction=""
                    />
                    <div className="mt-2">
                      <button
                        type="button"
                        onClick={() => void reviseSection()}
                        disabled={busy}
                        className="inline-flex items-center gap-1.5 rounded-full border border-primary/30 px-3 py-1 text-[11px] text-primary hover:bg-primary/10 disabled:opacity-50"
                        data-section-revise=""
                      >
                        <Icon name="refresh" className="h-3.5 w-3.5" />
                        生成修订建议
                      </button>
                    </div>
                  </div>
                ) : null}
              </div>
            )}
          </div>

          <div className="border-t border-surface-variant/30 px-4 py-2.5">
            {message || factcheckNote ? (
              <p className="text-[10.5px] text-on-surface-variant" data-section-message="">
                {message || factcheckNote}
              </p>
            ) : null}
            {message && factcheckNote ? (
              <p className="mt-1 text-[10px] text-outline" data-section-factcheck="">
                {factcheckNote}
              </p>
            ) : null}
          </div>
        </main>

        {/* 右：本章素材与引用。 */}
        <aside className={CARD + " flex min-h-0 flex-col overflow-hidden"} aria-label="本章素材与引用">
          <div className="flex items-center gap-2 border-b border-surface-variant/25 px-3 py-2">
            <span className="text-[11px] font-medium text-on-surface">本章素材</span>
            <span className="mono text-[10px] text-outline">{materialRows.length + effectiveKnowledgeIds.length}</span>
            {fallbackMaterials ? (
              <span className="ml-auto text-[9.5px] text-outline">沿用任务素材</span>
            ) : null}
          </div>
          <div className="max-h-[45%] min-h-0 overflow-y-auto px-2.5 py-2" data-section-materials="">
            {!selected ? (
              <p className="px-1 py-2 text-[11px] text-outline">选择章节后显示素材。</p>
            ) : materialRows.length + effectiveKnowledgeIds.length === 0 ? (
              <p className="px-1 py-2 text-[11px] text-outline">
                {!collectLoaded && selected.material_ids.length === 0
                  ? "读取中…"
                  : collectError && selected.material_ids.length === 0
                    ? "素材未加载。"
                    : "本章没有素材。"}
              </p>
            ) : (
              <div className="space-y-1">
                {materialRows.map((row) => (
                  <button
                    key={row.id}
                    type="button"
                    onClick={() => void openMaterial(row.id)}
                    className="w-full rounded-lg border border-surface-variant/25 bg-surface-container-low/40 px-2 py-1.5 text-left hover:border-primary/25 hover:bg-surface-container-low"
                    data-section-material={row.id}
                  >
                    <span className="block truncate text-[11px] font-medium text-on-surface">{row.title}</span>
                    <span className="mt-0.5 block truncate text-[10px] text-outline">{row.summary || row.id}</span>
                  </button>
                ))}
                {effectiveKnowledgeIds.map((id) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => void openKnowledge(id)}
                    className="w-full rounded-lg border border-surface-variant/25 bg-surface-container-low/40 px-2 py-1.5 text-left hover:border-primary/25 hover:bg-surface-container-low"
                    data-section-knowledge={id}
                  >
                    <span className="block truncate text-[11px] font-medium text-on-surface">{id}</span>
                    <span className="mt-0.5 block text-[10px] text-outline">知识依据</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="flex items-center gap-2 border-y border-surface-variant/25 px-3 py-2">
            <span className="text-[11px] font-medium text-on-surface">本章引用</span>
            <span className="mono text-[10px] text-outline">{activeCitations.length}</span>
            {draft ? (
              <span className="ml-auto rounded-full bg-amber-500/15 px-1.5 py-0.5 text-[9.5px] text-amber-700">
                AI 建议
              </span>
            ) : confirmedContent ? (
              <span className="ml-auto rounded-full bg-secondary-container/70 px-1.5 py-0.5 text-[9.5px] text-on-secondary-container">
                已确认
              </span>
            ) : null}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-2.5 py-2" data-section-citations="">
            {activeCitations.length > 0 ? (
              <div className="space-y-2">
                {activeCitations.map((citation, index) => (
                  <div
                    key={String(citation.source_kind) + "-" + citation.source_id + "-" + index}
                    className="rounded-lg border border-surface-variant/25 bg-surface-container-low/40 px-2 py-1.5"
                    data-section-citation={index}
                  >
                    <div className="flex items-baseline gap-1.5">
                      <span className="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-[9.5px] text-primary">
                        {citation.source_kind === "material" ? "素材" : "知识"}
                      </span>
                      <button
                        type="button"
                        onClick={() => openSource(citation)}
                        className="min-w-0 truncate text-[10.5px] text-on-surface hover:text-primary"
                        data-section-citation-source={citation.source_id}
                      >
                        {resolveSourceTitle(citation)}
                      </button>
                    </div>
                    <p className="mt-1 line-clamp-3 text-[10px] leading-relaxed text-outline">
                      {citation.quote}
                    </p>
                  </div>
                ))}
              </div>
            ) : citationsUnknown ? (
              <p className="px-1 py-2 text-[10.5px] text-outline" data-section-citations-empty="">
                本章引用明细未在当前界面加载。
              </p>
            ) : confirmedContent ? (
              <p className="px-1 py-2 text-[10.5px] text-outline">本章没有引用。</p>
            ) : (
              <p className="px-1 py-2 text-[10.5px] text-outline">本章还没有引用。</p>
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
    </section>
  );
}
