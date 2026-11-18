import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { host, shell } from "../api";
import {
  TERMINAL_STATES,
  type ReadingAnnotation,
  type ReadingAnnotationDraft,
  type ReadingBook,
  type ReadingBookMeta,
  type ReadingLibrary,
  type ReadingSavedNote,
} from "../types";
import {
  buildReadingConceptPrompt,
  buildReadingOrganizePrompt,
  readingConceptInputText,
  readingOrganizeInputText,
  stripMarkdownFence,
} from "../readingAi";
import { MarkdownText } from "./MarkdownText";
import { ConfirmDialog } from "./ConfirmDialog";
import { Icon } from "../components/Icon";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

type PendingSelection = {
  chapter_index: number;
  paragraph_index: number;
  start: number;
  end: number;
  quote: string;
};

type ComposerState = {
  title: string;
  draft: ReadingAnnotationDraft;
  text: string;
  editingId: string | null;
};

type AiContext = PendingSelection & {
  chapter_title: string;
  paragraph: string;
};

type AiPhase = "idle" | "running" | "done" | "error";
type SideTab = "notes" | "ai";

function kindLabel(kind: string): string {
  return kind === "highlight" ? "划线" : kind === "bookmark" ? "标记" : "想法";
}

function chapterTitle(book: ReadingBook, index: number): string {
  return book.chapters[index]?.title?.trim() || `第 ${index + 1} 章`;
}

function codeLength(text: string): number {
  return Array.from(text).length;
}

function clampText(text: string, max: number): string {
  const chars = Array.from(text);
  return chars.length <= max ? text : chars.slice(0, max).join("") + "…";
}

function paragraphKey(chapter: number, paragraph: number): string {
  return `${chapter}:${paragraph}`;
}

/** 把一段原文按批注范围切成可渲染片段；重叠时优先显示划线，其次显示想法下划线。 */
function paragraphSegments(text: string, annotations: ReadingAnnotation[]) {
  const chars = Array.from(text);
  const marks = annotations.filter(
    (a) =>
      (a.kind === "highlight" || a.kind === "thought") &&
      a.end > a.start &&
      a.start < chars.length,
  );
  if (marks.length === 0) return [{ text, mark: null as ReadingAnnotation | null, key: "plain" }];
  const points = new Set<number>([0, chars.length]);
  for (const mark of marks) {
    points.add(Math.max(0, Math.min(chars.length, mark.start)));
    points.add(Math.max(0, Math.min(chars.length, mark.end)));
  }
  const ordered = Array.from(points).sort((a, b) => a - b);
  const segments: Array<{ text: string; mark: ReadingAnnotation | null; key: string }> = [];
  for (let i = 0; i < ordered.length - 1; i += 1) {
    const start = ordered[i];
    const end = ordered[i + 1];
    if (start === undefined || end === undefined || end <= start) continue;
    const segmentText = chars.slice(start, end).join("");
    if (!segmentText) continue;
    const covering = marks.filter((a) => a.start <= start && a.end >= end);
    const mark =
      covering.find((a) => a.kind === "highlight") ?? covering.find((a) => a.kind === "thought") ?? null;
    segments.push({ text: segmentText, mark, key: `${start}-${end}-${mark?.id ?? "plain"}` });
  }
  return segments;
}

function annotationPreview(annotation: ReadingAnnotation): string {
  if (annotation.quote.trim()) return clampText(annotation.quote.trim().replace(/\s+/g, " "), 120);
  if (annotation.body.trim()) return clampText(annotation.body.trim().replace(/\s+/g, " "), 120);
  return "（空批注）";
}

function describeAiError(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  if (/getaddrinfo|enotfound|eai_again|dns/i.test(raw)) {
    return (
      "模型端点域名解析失败（getaddrinfo/ENOTFOUND）。读书页没有改动模型配置，" +
      "请检查本机网络、代理或设置中的服务商地址后再试。\n原始错误：" +
      raw
    );
  }
  if (/econnrefused|etimedout|econnreset/i.test(raw)) {
    return "模型端点连接失败（网络超时或被拒绝）。请检查网络/代理后重试。\n原始错误：" + raw;
  }
  return raw;
}

export function ReadingPage() {
  const [library, setLibrary] = useState<ReadingLibrary | null>(null);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [importBusy, setImportBusy] = useState(false);
  const [bookId, setBookId] = useState<string | null>(null);
  const [book, setBook] = useState<ReadingBook | null>(null);
  const [bookBusy, setBookBusy] = useState(false);
  const [bookError, setBookError] = useState<string | null>(null);
  const [sideTab, setSideTab] = useState<SideTab>("notes");
  const [panelOpen, setPanelOpen] = useState(false);
  const [selection, setSelection] = useState<PendingSelection | null>(null);
  const [selectionPos, setSelectionPos] = useState<{ top: number; left: number } | null>(null);
  const [composer, setComposer] = useState<ComposerState | null>(null);
  const [composerBusy, setComposerBusy] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<ReadingAnnotation | null>(null);
  const [activeAnnotation, setActiveAnnotation] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [aiContext, setAiContext] = useState<AiContext | null>(null);
  const [aiQuestion, setAiQuestion] = useState("");
  const [aiPhase, setAiPhase] = useState<AiPhase>("idle");
  const [aiStatus, setAiStatus] = useState<string | null>(null);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiAnswer, setAiAnswer] = useState<string | null>(null);
  const [aiAnswerSaved, setAiAnswerSaved] = useState(false);
  const [organized, setOrganized] = useState<string | null>(null);
  const [organizedSaved, setOrganizedSaved] = useState(false);
  const [organizedReceipt, setOrganizedReceipt] = useState<{ verified: number; unverified: number } | null>(
    null,
  );
  const [aiTaskId, setAiTaskId] = useState<string | null>(null);
  const [noteView, setNoteView] = useState<{ title: string; markdown: string } | null>(null);
  const [exportFile, setExportFile] = useState<{ path: string; content: string } | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const restoredBook = useRef<string | null>(null);
  const loadSeq = useRef(0);
  const lastProgress = useRef(0);
  const progressTimer = useRef<number | null>(null);
  const toastTimer = useRef<number | null>(null);
  const aiSeq = useRef(0);
  const readSelectionRef = useRef<() => void>(() => {});

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3600);
  }, []);

  const refreshLibrary = useCallback(async () => {
    try {
      const loaded = await shell.readingLibrary();
      setLibrary(loaded);
      setLibraryError(null);
    } catch (e) {
      setLibraryError(String(e));
    }
  }, []);

  useEffect(() => {
    void refreshLibrary();
  }, [refreshLibrary]);

  useEffect(() => {
    return () => {
      if (progressTimer.current !== null) window.clearTimeout(progressTimer.current);
      if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    };
  }, []);

  const updateMetaInLibrary = useCallback((meta: ReadingBookMeta) => {
    setLibrary((prev) =>
      prev
        ? {
            ...prev,
            books: prev.books.map((item) => (item.book_id === meta.book_id ? meta : item)),
          }
        : prev,
    );
  }, []);

  const openBook = useCallback(
    async (id: string) => {
      const seq = loadSeq.current + 1;
      loadSeq.current = seq;
      if (book && scrollRef.current && book.book_id !== id) {
        const el = scrollRef.current;
        const max = Math.max(1, el.scrollHeight - el.clientHeight);
        void shell.readingProgressSet(book.book_id, el.scrollTop / max).catch(() => undefined);
      }
      if (progressTimer.current !== null) {
        window.clearTimeout(progressTimer.current);
        progressTimer.current = null;
      }
      setBookId(id);
      setBookBusy(true);
      setBookError(null);
      setBook(null);
      setSelection(null);
      setSelectionPos(null);
      setAiContext(null);
      aiSeq.current += 1;
      setAiTaskId(null);
      setAiPhase("idle");
      setAiStatus(null);
      setAiAnswer(null);
      setOrganized(null);
      setOrganizedReceipt(null);
      setActiveAnnotation(null);
      setComposer(null);
      setPendingDelete(null);
      setNoteView(null);
      setExportFile(null);
      try {
        const loaded = await shell.readingBook(id);
        if (seq !== loadSeq.current) return;
        setBook(loaded);
        lastProgress.current = loaded.progress.percent;
      } catch (e) {
        if (seq !== loadSeq.current) return;
        setBookError(String(e));
      } finally {
        if (seq === loadSeq.current) setBookBusy(false);
      }
    },
    [book],
  );

  useEffect(() => {
    if (!book || restoredBook.current === book.book_id) return undefined;
    const frame = requestAnimationFrame(() => {
      const el = scrollRef.current;
      if (!el) return;
      const max = Math.max(0, el.scrollHeight - el.clientHeight);
      el.scrollTop = Math.max(0, Math.min(1, book.progress.percent)) * max;
      lastProgress.current = book.progress.percent;
      restoredBook.current = book.book_id;
    });
    return () => cancelAnimationFrame(frame);
    // 只在真正打开/切换书时恢复进度；批注更新不能把阅读位置拽回去。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.book_id]);

  const handleScroll = () => {
    if (!book || !scrollRef.current) return;
    const el = scrollRef.current;
    const max = el.scrollHeight - el.clientHeight;
    if (max <= 0) return;
    const percent = Math.max(0, Math.min(1, el.scrollTop / max));
    if (Math.abs(percent - lastProgress.current) < 0.012) return;
    lastProgress.current = percent;
    setSelection(null);
    setSelectionPos(null);
    if (progressTimer.current !== null) window.clearTimeout(progressTimer.current);
    progressTimer.current = window.setTimeout(() => {
      const id = book.book_id;
      void shell
        .readingProgressSet(id, percent)
        .then((meta) => {
          updateMetaInLibrary(meta);
          setBook((prev) =>
            prev && prev.book_id === id
              ? { ...prev, progress: { percent: meta.progress_percent, updated_at: meta.updated_at } }
              : prev,
          );
        })
        .catch(() => undefined);
    }, 1200);
  };

  const importBook = async () => {
    setImportBusy(true);
    try {
      const picked = await openFileDialog({
        multiple: false,
        directory: false,
        title: "选择要读的文件",
        filters: [{ name: "阅读文件", extensions: ["md", "markdown", "txt", "pdf", "docx", "epub"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      const result = await shell.readingImportBook(picked);
      await refreshLibrary();
      setBookId(result.book.book_id);
      await openBook(result.book.book_id);
      showToast(
        result.existing
          ? `《${result.book.title}》已在书架中，已打开。`
          : `已导入《${result.book.title}》：${result.book.chapter_count} 章，${result.book.paragraph_count} 段。`,
      );
    } catch (e) {
      showToast(String(e));
    } finally {
      setImportBusy(false);
    }
  };

  const readSelection = () => {
    const current = window.getSelection();
    if (!current || current.rangeCount === 0 || current.isCollapsed) {
      setSelection(null);
      setSelectionPos(null);
      return;
    }
    const range = current.getRangeAt(0);
    const startNode = range.startContainer;
    const endNode = range.endContainer;
    const startEl = (startNode.nodeType === Node.TEXT_NODE ? startNode.parentElement : (startNode as Element))?.closest(
      "[data-paragraph]",
    ) as HTMLElement | null;
    const endEl = (endNode.nodeType === Node.TEXT_NODE ? endNode.parentElement : (endNode as Element))?.closest(
      "[data-paragraph]",
    ) as HTMLElement | null;
    if (!startEl || startEl !== endEl) {
      setSelection(null);
      setSelectionPos(null);
      if (startEl && endEl && startEl !== endEl) showToast("跨段选择暂不支持，请把选区缩小到同一段。");
      return;
    }
    const chapterIndex = Number(startEl.dataset.chapterIndex);
    const paragraphIndex = Number(startEl.dataset.paragraphIndex);
    if (!Number.isInteger(chapterIndex) || !Number.isInteger(paragraphIndex)) return;
    const prefix = document.createRange();
    prefix.setStart(startEl, 0);
    prefix.setEnd(range.startContainer, range.startOffset);
    const prefixChars = codeLength(prefix.toString());
    const rawChars = Array.from(range.toString());
    let leading = 0;
    while (leading < rawChars.length && /\s/.test(rawChars[leading] ?? "")) leading += 1;
    let trailing = rawChars.length;
    while (trailing > leading && /\s/.test(rawChars[trailing - 1] ?? "")) trailing -= 1;
    if (trailing <= leading) return;
    const quote = rawChars.slice(leading, trailing).join("");
    if (codeLength(quote) > 4000) {
      showToast("选区超过 4000 字，请缩小到一段话。");
      return;
    }
    const rect = range.getBoundingClientRect();
    setSelection({
      chapter_index: chapterIndex,
      paragraph_index: paragraphIndex,
      start: prefixChars + leading,
      end: prefixChars + trailing,
      quote,
    });
    setSelectionPos({
      // 顶部被顶栏占住时放到选区下方，避免工具栏藏在导航条后面。
      top: Math.max(
        64,
        Math.min(window.innerHeight - 52, rect.top >= 64 ? rect.top - 48 : rect.bottom + 8),
      ),
      left: Math.max(12, Math.min(window.innerWidth - 330, rect.left + rect.width / 2 - 150)),
    });
  };
  readSelectionRef.current = readSelection;

  // 鼠标可能在正文外松开（例如拖到侧栏边缘），只挂 article 会漏掉；
  // 在本书打开期间监听 document，延迟一拍等浏览器把选区落定。
  useEffect(() => {
    if (!book) return undefined;
    const handler = (event: MouseEvent) => {
      const target = event.target as Element | null;
      if (target?.closest?.(".reading-selection-bar")) return;
      window.setTimeout(() => readSelectionRef.current(), 0);
    };
    document.addEventListener("mouseup", handler);
    return () => document.removeEventListener("mouseup", handler);
  }, [book?.book_id]);

  const clearSelection = () => {
    window.getSelection()?.removeAllRanges();
    setSelection(null);
    setSelectionPos(null);
  };

  const mergeAnnotation = (annotation: ReadingAnnotation, bookIdAtCall: string) => {
    setBook((prev) =>
      prev && prev.book_id === bookIdAtCall
        ? {
            ...prev,
            annotations: [...prev.annotations, annotation],
            updated_at: annotation.updated_at,
          }
        : prev,
    );
    setLibrary((prev) =>
      prev
        ? {
            ...prev,
            books: prev.books.map((meta) =>
              meta.book_id === bookIdAtCall
                ? { ...meta, annotation_count: meta.annotation_count + 1, updated_at: annotation.updated_at }
                : meta,
            ),
          }
        : prev,
    );
  };

  const addAnnotation = async (draft: ReadingAnnotationDraft, quiet = false) => {
    if (!book) return null;
    const bookIdAtCall = book.book_id;
    try {
      const annotation = await shell.readingAnnotationAdd(bookIdAtCall, draft);
      mergeAnnotation(annotation, bookIdAtCall);
      if (!quiet) showToast(`已添加${kindLabel(annotation.kind)}。`);
      return annotation;
    } catch (e) {
      showToast(String(e));
      return null;
    }
  };

  const addHighlight = async () => {
    if (!selection) return;
    const added = await addAnnotation(
      { kind: "highlight", ...selection, color: "amber", source: "user" },
      true,
    );
    if (added) showToast("已划线；右侧「笔记」可回到原文，也可写想法。");
    clearSelection();
    setSideTab("notes");
    setPanelOpen(true);
  };

  const addBookmark = async () => {
    if (!book || !selection) return;
    const paragraph = book.chapters[selection.chapter_index]?.paragraphs[selection.paragraph_index] ?? "";
    const added = await addAnnotation(
      {
        kind: "bookmark",
        chapter_index: selection.chapter_index,
        paragraph_index: selection.paragraph_index,
        quote: clampText(paragraph.trim(), 80),
        color: "blue",
        source: "user",
      },
      true,
    );
    if (added) showToast("已标记本段；右侧「笔记」可见。");
    clearSelection();
    setSideTab("notes");
    setPanelOpen(true);
  };

  const beginThought = () => {
    if (!selection) return;
    setComposer({
      title: "写思考碎片",
      draft: { kind: "thought", ...selection, color: "green", source: "user" },
      text: "",
      editingId: null,
    });
    setSelectionPos(null);
  };

  const currentAnchor = () => {
    if (!book || !scrollRef.current) return;
    const viewport = scrollRef.current.getBoundingClientRect();
    const paragraphs = scrollRef.current.querySelectorAll<HTMLElement>(
      "[data-chapter-index][data-paragraph-index]",
    );
    let anchor: HTMLElement | null = null;
    for (const item of paragraphs) {
      if (item.getBoundingClientRect().bottom > viewport.top + 24) {
        anchor = item;
        break;
      }
    }
    const chapter = Number(anchor?.dataset.chapterIndex ?? 0);
    const paragraph = Number(anchor?.dataset.paragraphIndex ?? 0);
    return {
      chapter_index: Number.isInteger(chapter) ? chapter : 0,
      paragraph_index: Number.isInteger(paragraph) ? paragraph : 0,
      chapter_title: chapterTitle(book, Number.isInteger(chapter) ? chapter : 0),
      paragraph: book.chapters[chapter]?.paragraphs[paragraph] ?? "",
    };
  };

  const beginStandaloneThought = () => {
    const anchor = currentAnchor();
    if (!anchor) return;
    setComposer({
      title: "写思考碎片",
      draft: {
        kind: "thought",
        chapter_index: anchor.chapter_index,
        paragraph_index: anchor.paragraph_index,
        quote: "",
        color: "green",
        source: "user",
      },
      text: "",
      editingId: null,
    });
  };

  const beginEdit = (annotation: ReadingAnnotation) => {
    setComposer({
      title: "编辑批注",
      draft: {
        kind: annotation.kind,
        chapter_index: annotation.chapter_index,
        paragraph_index: annotation.paragraph_index,
        start: annotation.start,
        end: annotation.end,
        quote: annotation.quote,
        color: annotation.color,
        source: annotation.source,
      },
      text: annotation.body,
      editingId: annotation.id,
    });
  };

  const saveComposer = async () => {
    if (!composer) return;
    if (!book) return;
    const bookIdAtCall = book.book_id;
    const text = composer.text.trim();
    if (composer.draft.kind === "thought" && !text) {
      showToast("思考碎片不能为空。");
      return;
    }
    setComposerBusy(true);
    try {
      if (composer.editingId) {
        const updated = await shell.readingAnnotationUpdate(bookIdAtCall, composer.editingId, {
          body: text,
          color: composer.draft.color,
        });
        setBook((prev) =>
          prev && prev.book_id === bookIdAtCall
            ? { ...prev, annotations: prev.annotations.map((a) => (a.id === updated.id ? updated : a)) }
            : prev,
        );
        showToast("批注已更新。");
      } else {
        await addAnnotation({ ...composer.draft, body: text }, true);
        showToast("思考碎片已保存。");
      }
      setComposer(null);
      clearSelection();
      setSideTab("notes");
      setPanelOpen(true);
    } catch (e) {
      showToast(String(e));
    } finally {
      setComposerBusy(false);
    }
  };

  const removeAnnotation = async () => {
    if (!pendingDelete || !book) return;
    const bookIdAtCall = book.book_id;
    const annotationId = pendingDelete.id;
    try {
      const meta = await shell.readingAnnotationDelete(bookIdAtCall, annotationId);
      setBook((prev) =>
        prev && prev.book_id === bookIdAtCall
          ? { ...prev, annotations: prev.annotations.filter((a) => a.id !== annotationId) }
          : prev,
      );
      updateMetaInLibrary(meta);
      showToast("批注已删除。");
    } catch (e) {
      showToast(String(e));
    } finally {
      setPendingDelete(null);
    }
  };

  const annotationMap = useMemo(() => {
    const map = new Map<string, ReadingAnnotation[]>();
    for (const annotation of book?.annotations ?? []) {
      const key = paragraphKey(annotation.chapter_index, annotation.paragraph_index);
      const list = map.get(key) ?? [];
      list.push(annotation);
      map.set(key, list);
    }
    return map;
  }, [book?.annotations]);

  const groupedAnnotations = useMemo(() => {
    const groups: Array<{ chapterIndex: number; title: string; items: ReadingAnnotation[] }> = [];
    for (const annotation of book?.annotations ?? []) {
      const found = groups.find((group) => group.chapterIndex === annotation.chapter_index);
      if (found) {
        found.items.push(annotation);
      } else {
        groups.push({
          chapterIndex: annotation.chapter_index,
          title: book ? chapterTitle(book, annotation.chapter_index) : `第 ${annotation.chapter_index + 1} 章`,
          items: [annotation],
        });
      }
    }
    groups.sort((a, b) => a.chapterIndex - b.chapterIndex);
    for (const group of groups) group.items.sort((a, b) => a.paragraph_index - b.paragraph_index);
    return groups;
  }, [book]);

  const jumpToAnnotation = (annotation: ReadingAnnotation) => {
    const selector = `[data-chapter-index="${annotation.chapter_index}"][data-paragraph-index="${annotation.paragraph_index}"]`;
    const target = scrollRef.current?.querySelector(selector) as HTMLElement | null;
    if (!target) return;
    target.scrollIntoView({ block: "center", behavior: "smooth" });
    setActiveAnnotation(annotation.id);
    window.setTimeout(() => setActiveAnnotation(null), 2200);
  };

  const askAi = () => {
    if (!book || !selection) return;
    setAiContext({
      ...selection,
      chapter_title: chapterTitle(book, selection.chapter_index),
      paragraph: book.chapters[selection.chapter_index]?.paragraphs[selection.paragraph_index] ?? "",
    });
    setAiQuestion("");
    setAiAnswer(null);
    setAiAnswerSaved(false);
    setAiPhase("idle");
    setAiError(null);
    setSideTab("ai");
    setPanelOpen(true);
    setSelectionPos(null);
  };

  const runTask = useCallback(
    async (prompt: string, inputText: string, intent: string, mode: "concept" | "organize") => {
      const run = aiSeq.current + 1;
      aiSeq.current = run;
      setAiPhase("running");
      setAiStatus("正在提交任务…");
      setAiError(null);
      setAiTaskId(null);
      try {
        const workspace = await shell.prepareWorkspaceDir();
        if (!workspace.ok || !workspace.workspace_dir) {
          throw new Error(workspace.error || "宿主未返回可用工作目录。");
        }
        const receipt = await host.submitTask({
          promptText: prompt,
          intent,
          workspaceDir: workspace.workspace_dir,
          userInputText: inputText,
        });
        if (aiSeq.current !== run) return;
        setAiTaskId(receipt.omni_task_id);
        let status = await host.taskStatus(receipt.omni_task_id);
        for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
          await sleep(1000);
          if (aiSeq.current !== run) return;
          status = await host.taskStatus(receipt.omni_task_id);
          if (i % 5 === 0) setAiStatus(`模型正在处理…已等待 ${i + 1} 秒（不会自动取消）`);
        }
        if (aiSeq.current !== run) return;
        if (status.status !== "succeeded") {
          throw new Error(`任务未成功：${status.status}${status.note ? " · " + status.note : ""}`);
        }
        const result = await host.taskResult(receipt.omni_task_id);
        if (aiSeq.current !== run) return;
        if (!result.content.trim()) throw new Error("模型没有返回内容。");
        if (mode === "concept") {
          setAiAnswer(result.content);
          setAiAnswerSaved(false);
        } else {
          setOrganized(stripMarkdownFence(result.content));
          setOrganizedSaved(false);
          setOrganizedReceipt(null);
        }
        setAiStatus("完成。结果未复核，保存前请自行核对。");
        setAiPhase("done");
      } catch (e) {
        if (aiSeq.current !== run) return;
        setAiError(describeAiError(e));
        setAiPhase("error");
        setAiStatus(null);
      } finally {
        if (aiSeq.current === run) setAiTaskId(null);
      }
    },
    [],
  );

  const askConcept = async () => {
    if (!book) return;
    if (!aiQuestion.trim() && !aiContext?.quote) return;
    let context = aiContext;
    if (!context) {
      const anchor = currentAnchor();
      if (!anchor) return;
      context = {
        ...anchor,
        start: 0,
        end: 0,
        quote: "",
      };
      setAiContext(context);
    }
    const input = {
      bookTitle: book.title,
      chapterTitle: context.chapter_title,
      paragraph: context.paragraph,
      quote: context.quote,
      question: aiQuestion,
    };
    await runTask(
      buildReadingConceptPrompt(input),
      readingConceptInputText(input),
      "reading_concept",
      "concept",
    );
  };

  const organizeBook = async () => {
    if (!book || book.annotations.length === 0) {
      showToast("这本书还没有划线或想法，先留下几条再整理。");
      return;
    }
    setSideTab("ai");
    setPanelOpen(true);
    const input = {
      bookTitle: book.title,
      annotations: book.annotations.map((annotation) => ({
        id: annotation.id,
        kind: annotation.kind,
        chapterTitle: chapterTitle(book, annotation.chapter_index),
        paragraphIndex: annotation.paragraph_index,
        quote: annotation.quote,
        body: annotation.body,
        source: annotation.source,
      })),
    };
    await runTask(
      buildReadingOrganizePrompt(input),
      readingOrganizeInputText(input),
      "reading_organize",
      "organize",
    );
  };

  const cancelAi = async () => {
    aiSeq.current += 1;
    const id = aiTaskId;
    setAiTaskId(null);
    setAiPhase("idle");
    setAiStatus("正在请求取消…");
    if (!id) return;
    try {
      await host.cancelTask(id);
      showToast("已请求取消；最终状态以宿主回读为准。");
    } catch (e) {
      showToast(String(e));
    }
  };

  const saveAiAnswer = async () => {
    if (!aiContext || !aiAnswer) return;
    const annotation = await addAnnotation(
      {
        kind: "thought",
        chapter_index: aiContext.chapter_index,
        paragraph_index: aiContext.paragraph_index,
        start: aiContext.start,
        end: aiContext.end,
        quote: aiContext.quote,
        body: aiAnswer,
        color: "green",
        source: "ai",
      },
      true,
    );
    if (annotation) {
      setAiAnswerSaved(true);
      showToast("AI 解释已存为思考碎片，可在「笔记」里回看。");
    }
  };

  const saveOrganized = async () => {
    if (!book || !organized) return;
    try {
      const note = await shell.readingNoteSave(book.book_id, "AI 整理稿", organized, book.annotations.map((a) => a.id));
      setBook((prev) => (prev ? { ...prev, notes: [...prev.notes, note] } : prev));
      setLibrary((prev) =>
        prev
          ? {
              ...prev,
              books: prev.books.map((meta) =>
                meta.book_id === book.book_id ? { ...meta, note_count: meta.note_count + 1 } : meta,
              ),
            }
          : prev,
      );
      setOrganizedSaved(true);
      setOrganizedReceipt({
        verified: note.verified_citations,
        unverified: note.unverified_citations.length,
      });
      showToast(`整理稿已保存：${note.relative_path}`);
    } catch (e) {
      showToast(String(e));
    }
  };

  const readSavedNote = async (note: ReadingSavedNote) => {
    if (!book) return;
    try {
      const markdown = await shell.readingNoteRead(book.book_id, note.note_id);
      setNoteView({ title: note.title, markdown });
    } catch (e) {
      showToast(String(e));
    }
  };

  const exportNotes = async () => {
    if (!book) return;
    try {
      const file = await shell.readingExportNotes(book.book_id);
      setExportFile({ path: file.relative_path, content: file.content });
      showToast(`已导出：${file.relative_path}`);
    } catch (e) {
      showToast(String(e));
    }
  };

  return (
    <div className="reading-page flex h-full min-h-0 w-full flex-col md:pl-20" data-testid="reading-page">
      <div className="flex min-h-0 flex-1">
        <aside className="reading-shelf hidden w-[238px] shrink-0 flex-col border-r border-outline-variant/40 bg-surface/60 lg:flex">
          <div className="flex items-center justify-between gap-2 border-b border-outline-variant/40 px-3 py-3">
            <div>
              <p className="text-[13px] font-medium text-on-surface">读书</p>
              <p className="text-[10.5px] text-outline">{library ? `${library.books.length} 本` : "读取中"}</p>
            </div>
            <button
              type="button"
              onClick={() => void importBook()}
              disabled={importBusy}
              className="inline-flex items-center gap-1 rounded-lg border border-outline-variant/60 px-2 py-1 text-[11px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-50"
            >
              <Icon name="plus" className="h-3 w-3" />
              {importBusy ? "导入中" : "导入"}
            </button>
          </div>
          <div className="reading-shelf-list min-h-0 flex-1 overflow-y-auto p-2">
            {libraryError ? (
              <p className="rounded-lg bg-error-container/60 p-2 text-[11px] text-on-error-container">{libraryError}</p>
            ) : null}
            {library?.books.length === 0 ? (
              <div className="rounded-lg bg-surface-container-low p-3 text-[11.5px] leading-relaxed text-on-surface-variant">
                <p className="font-medium text-on-surface">书架还是空的</p>
                <p className="mt-1">导入一本 Markdown、TXT、PDF、DOCX 或 EPUB，开始留下划线与想法。</p>
              </div>
            ) : null}
            {library?.books.map((item) => {
              const active = item.book_id === bookId;
              return (
                <button
                  type="button"
                  key={item.book_id}
                  onClick={() => void openBook(item.book_id)}
                  className={
                    "mb-1.5 w-full rounded-lg border px-2.5 py-2 text-left transition-colors " +
                    (active
                      ? "border-primary/40 bg-primary/10"
                      : "border-transparent hover:border-outline-variant/50 hover:bg-surface-container-low")
                  }
                >
                  <div className="flex items-center gap-2">
                    <span className="flex h-7 w-6 items-center justify-center rounded-[4px] bg-surface-container text-primary">
                      <Icon name="book" className="h-3.5 w-3.5" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12px] font-medium text-on-surface">{item.title}</span>
                      <span className="block text-[10px] text-outline">
                        {item.source_format.toUpperCase()} · {item.chapter_count} 章 · {item.annotation_count} 条批注
                      </span>
                    </span>
                  </div>
                  <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-outline-variant/30">
                    <div
                      className="h-full rounded-full bg-primary/70"
                      style={{ width: `${Math.round(item.progress_percent * 100)}%` }}
                    />
                  </div>
                </button>
              );
            })}
          </div>
          {library?.root ? (
            <p className="truncate border-t border-outline-variant/30 px-3 py-2 text-[9.5px] text-outline" title={library.root}>
              阅读目录：{library.root}
            </p>
          ) : null}
        </aside>

        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          {!book && !bookBusy ? (
            <div className="flex min-h-0 flex-1 items-center justify-center px-6">
              <div className="w-full max-w-[560px] rounded-2xl border border-outline-variant/40 bg-surface/70 p-6">
                <p className="text-[16px] font-medium text-on-surface">读书，也把想法留下</p>
                <p className="mt-2 text-[12.5px] leading-relaxed text-on-surface-variant">
                  选择一本书或一个文本文件。阅读时选中一段，可以划线、标记、写思考碎片，或直接问 AI 解释；
                  整理稿和批注只保存在本机阅读目录，保存前由你确认。
                </p>
                <button
                  type="button"
                  onClick={() => void importBook()}
                  disabled={importBusy}
                  className="mt-4 inline-flex items-center gap-1.5 rounded-lg bg-primary px-3.5 py-2 text-[12px] text-on-primary disabled:opacity-50"
                >
                  <Icon name="plus" className="h-3.5 w-3.5" />
                  {importBusy ? "正在导入…" : "选择文件"}
                </button>
              </div>
            </div>
          ) : null}

          {bookBusy ? <p className="p-6 text-[12px] text-on-surface-variant">正在打开书籍…</p> : null}
          {bookError ? (
            <div className="m-6 rounded-lg bg-error-container/60 p-3 text-[12px] text-on-error-container">{bookError}</div>
          ) : null}

          {book ? (
            <>
              <header className="flex items-center gap-3 border-b border-outline-variant/40 bg-surface/75 px-4 py-2.5 md:px-6">
                <select
                  className="max-w-[150px] truncate rounded-lg border border-outline-variant/50 bg-surface px-2 py-1 text-[11px] text-on-surface lg:hidden"
                  value={bookId ?? ""}
                  onChange={(event) => {
                    if (event.target.value) void openBook(event.target.value);
                  }}
                >
                  {library?.books.map((item) => (
                    <option key={item.book_id} value={item.book_id}>
                      {item.title}
                    </option>
                  ))}
                </select>
                <div className="min-w-0 flex-1">
                  <h1 className="truncate text-[15px] font-medium text-on-surface">{book.title}</h1>
                  <p className="truncate text-[10.5px] text-outline">
                    {book.source_format.toUpperCase()} · {book.chapters.length} 章 · {book.annotations.length} 条批注 · 读到
                    {" "}
                    {Math.round(book.progress.percent * 100)}%
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => void importBook()}
                  disabled={importBusy}
                  className="rounded-lg border border-outline-variant/50 p-1.5 text-on-surface-variant hover:text-primary disabled:opacity-50"
                  aria-label="导入阅读文件"
                  title="导入阅读文件"
                >
                  <Icon name="plus" className="h-3.5 w-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => setPanelOpen((value) => !value)}
                  className="rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] text-on-surface-variant hover:text-primary xl:hidden"
                >
                  笔记
                </button>
                <button
                  type="button"
                  onClick={() => void exportNotes()}
                  className="hidden rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] text-on-surface-variant hover:text-primary sm:inline-flex"
                >
                  导出
                </button>
                <button
                  type="button"
                  onClick={beginStandaloneThought}
                  className="hidden rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] text-on-surface-variant hover:text-primary sm:inline-flex"
                  data-testid="reading-add-thought"
                >
                  写想法
                </button>
                <button
                  type="button"
                  onClick={() => void organizeBook()}
                  className="inline-flex items-center gap-1 rounded-lg bg-primary px-2.5 py-1.5 text-[11px] text-on-primary disabled:opacity-50"
                  disabled={book.annotations.length === 0 || aiPhase === "running"}
                >
                  <Icon name="spark" className="h-3 w-3" />
                  整理笔记
                </button>
              </header>

              <div
                ref={scrollRef}
                onScroll={handleScroll}
                onKeyUp={readSelection}
                className="reading-scroll min-h-0 flex-1 overflow-y-auto"
              >
                <article className="mx-auto w-full max-w-[760px] px-6 pb-28 pt-8 md:px-10">
                  {book.chapters.map((chapter, chapterIndex) => (
                    <section key={`${chapterIndex}-${chapter.title}`} data-chapter-section={chapterIndex}>
                      <h2 className="reading-chapter-title">{chapter.title}</h2>
                      {chapter.paragraphs.map((paragraph, paragraphIndex) => {
                        const items = annotationMap.get(paragraphKey(chapterIndex, paragraphIndex)) ?? [];
                        const bookmarked = items.some((item) => item.kind === "bookmark");
                        const segments = paragraphSegments(paragraph, items);
                        return (
                          <p
                            key={`${chapterIndex}-${paragraphIndex}`}
                            data-chapter-index={chapterIndex}
                            data-paragraph-index={paragraphIndex}
                            className={`reading-paragraph ${bookmarked ? "reading-bookmarked" : ""}`}
                          >
                            {segments.map((segment) =>
                              segment.mark ? (
                                <mark
                                  key={segment.key}
                                  data-annotation-id={segment.mark.id}
                                  className={
                                    "reading-mark reading-mark-" +
                                    (segment.mark.color || "amber") +
                                    (segment.mark.kind === "thought" ? " reading-mark-thought" : "") +
                                    (activeAnnotation === segment.mark.id ? " reading-active-mark" : "")
                                  }
                                  title={`${kindLabel(segment.mark.kind)}：${annotationPreview(segment.mark)}`}
                                  onClick={() => {
                                    setSideTab("notes");
                                    setPanelOpen(true);
                                    setActiveAnnotation(segment.mark?.id ?? null);
                                  }}
                                >
                                  {segment.text}
                                </mark>
                              ) : (
                                <span key={segment.key}>{segment.text}</span>
                              ),
                            )}
                          </p>
                        );
                      })}
                    </section>
                  ))}
                </article>
              </div>
            </>
          ) : null}
        </main>

        {book ? (
          <aside
            className={
              "reading-side-panel shrink-0 flex-col border-l border-outline-variant/40 bg-surface/85 " +
              (panelOpen
                ? "fixed bottom-0 right-0 top-[52px] z-40 flex w-[340px] max-w-[92vw] shadow-[-16px_0_40px_-20px_rgba(70,55,40,0.28)] xl:static xl:z-auto xl:shadow-none"
                : "hidden xl:flex")
            }
          >
            <div className="flex items-center border-b border-outline-variant/40">
              <button
                type="button"
                onClick={() => setSideTab("notes")}
                className={
                  "flex-1 px-3 py-2.5 text-[11.5px] " +
                  (sideTab === "notes" ? "border-b-2 border-primary text-primary" : "text-on-surface-variant")
                }
              >
                笔记 {book.annotations.length + book.notes.length}
              </button>
              <button
                type="button"
                onClick={() => setSideTab("ai")}
                className={
                  "flex-1 px-3 py-2.5 text-[11.5px] " +
                  (sideTab === "ai" ? "border-b-2 border-primary text-primary" : "text-on-surface-variant")
                }
              >
                AI
              </button>
              <button
                type="button"
                onClick={() => setPanelOpen(false)}
                className="px-2 text-outline hover:text-on-surface xl:hidden"
                aria-label="关闭笔记面板"
              >
                ×
              </button>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto p-3">
              {sideTab === "notes" ? (
                <div className="space-y-4">
                  {book.annotations.length === 0 ? (
                    <p className="rounded-lg bg-surface-container-low p-3 text-[11.5px] leading-relaxed text-on-surface-variant">
                      选中正文里的文字，浮出的工具栏可以划线、写想法、标记段落或问 AI。
                    </p>
                  ) : null}
                  {groupedAnnotations.map((group) => (
                    <section key={group.chapterIndex}>
                      <p className="mb-1.5 truncate text-[10.5px] font-medium text-outline">{group.title}</p>
                      <div className="space-y-1.5">
                        {group.items.map((annotation) => (
                          <div
                            key={annotation.id}
                            className={
                              "rounded-lg border px-2.5 py-2 " +
                              (activeAnnotation === annotation.id
                                ? "border-primary/50 bg-primary/8"
                                : "border-outline-variant/35 bg-surface-container-lowest")
                            }
                          >
                            <div className="flex items-center gap-1.5 text-[10px] text-outline">
                              <span className={`reading-kind-dot reading-kind-${annotation.kind}`} />
                              <span>{kindLabel(annotation.kind)}</span>
                              <span>· 第 {annotation.paragraph_index + 1} 段</span>
                              {annotation.source === "ai" ? <span className="text-primary">AI</span> : null}
                              <span className="ml-auto flex gap-1">
                                <button
                                  type="button"
                                  onClick={() => jumpToAnnotation(annotation)}
                                  className="text-outline hover:text-primary"
                                  aria-label="回到原文"
                                >
                                  <Icon name="arrow-right" className="h-3 w-3" />
                                </button>
                                <button
                                  type="button"
                                  onClick={() => beginEdit(annotation)}
                                  className="text-outline hover:text-primary"
                                  aria-label="编辑批注"
                                >
                                  <Icon name="edit" className="h-3 w-3" />
                                </button>
                                <button
                                  type="button"
                                  onClick={() => setPendingDelete(annotation)}
                                  className="text-outline hover:text-error"
                                  aria-label="删除批注"
                                >
                                  <Icon name="x" className="h-3 w-3" />
                                </button>
                              </span>
                            </div>
                            {annotation.quote ? (
                              <p className="mt-1 line-clamp-3 text-[11.5px] leading-[1.55] text-on-surface">
                                {annotation.kind === "bookmark" ? annotation.quote : `「${annotation.quote}」`}
                              </p>
                            ) : null}
                            {annotation.body ? (
                              <p className="mt-1 whitespace-pre-wrap text-[11.5px] leading-[1.6] text-on-surface-variant">
                                {annotation.body}
                              </p>
                            ) : null}
                          </div>
                        ))}
                      </div>
                    </section>
                  ))}

                  {book.notes.length > 0 ? (
                    <section>
                      <p className="mb-1.5 text-[10.5px] font-medium text-outline">已保存的整理稿</p>
                      <div className="space-y-1.5">
                        {book.notes.map((note) => (
                          <button
                            type="button"
                            key={note.note_id}
                            onClick={() => void readSavedNote(note)}
                            className="w-full rounded-lg border border-outline-variant/35 bg-surface-container-lowest px-2.5 py-2 text-left text-[11.5px] text-on-surface hover:border-primary/40"
                          >
                            <span className="block truncate">{note.title}</span>
                            <span className="block truncate text-[10px] text-outline">{note.created_at}</span>
                          </button>
                        ))}
                      </div>
                    </section>
                  ) : null}
                </div>
              ) : (
                <div className="space-y-3">
                  <div className="rounded-lg border border-outline-variant/35 bg-surface-container-lowest p-2.5">
                    <p className="text-[11px] font-medium text-on-surface">AI 阅读助手</p>
                    <p className="mt-1 text-[10.5px] leading-relaxed text-outline">
                      只会发送当前选中的引文、问题和批注；模型结果未复核，保存前请核对。
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => void organizeBook()}
                    disabled={book.annotations.length === 0 || aiPhase === "running"}
                    className="inline-flex w-full items-center justify-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-[11.5px] text-on-primary disabled:opacity-50"
                  >
                    <Icon name="spark" className="h-3.5 w-3.5" />
                    整理本书笔记
                  </button>

                  {aiContext ? (
                    <div className="rounded-lg border border-primary/25 bg-primary/5 p-2.5">
                      <p className="line-clamp-2 text-[10.5px] text-outline">{aiContext.chapter_title}</p>
                      <p className="mt-1 line-clamp-3 text-[11.5px] leading-relaxed text-on-surface">
                        {aiContext.quote ? `「${aiContext.quote}」` : "（未选文字，按当前阅读段提问）"}
                      </p>
                      <button
                        type="button"
                        onClick={() => setAiContext(null)}
                        className="mt-1.5 rounded-md px-2 py-0.5 text-[10.5px] text-outline hover:text-on-surface"
                      >
                        清除选区
                      </button>
                    </div>
                  ) : (
                    <p className="rounded-lg bg-surface-container-low p-2.5 text-[11px] leading-relaxed text-on-surface-variant">
                      选中正文后可问 AI；也可以在读完几段后直接整理本书的批注。
                    </p>
                  )}

                  <div className="rounded-lg border border-outline-variant/35 bg-surface-container-lowest p-2.5">
                    <textarea
                      value={aiQuestion}
                      onChange={(event) => setAiQuestion(event.target.value)}
                      placeholder="随时问一个概念，或解释选中的文字…"
                      className="h-16 w-full resize-none rounded-md border border-outline-variant/40 bg-surface px-2 py-1.5 text-[11.5px] text-on-surface outline-none focus:border-primary"
                    />
                    <button
                      type="button"
                      onClick={() => void askConcept()}
                      disabled={aiPhase === "running" || (!aiQuestion.trim() && !aiContext?.quote)}
                      className="mt-1.5 rounded-md bg-primary px-2.5 py-1 text-[11px] text-on-primary disabled:opacity-50"
                    >
                      解释 / 查概念
                    </button>
                  </div>

                  {aiPhase === "running" ? (
                    <div className="rounded-lg bg-surface-container-low p-2.5 text-[11px] text-on-surface-variant">
                      <p>{aiStatus ?? "模型正在处理…"}</p>
                      <button type="button" onClick={() => void cancelAi()} className="mt-1.5 text-[11px] text-error hover:underline">
                        取消任务
                      </button>
                    </div>
                  ) : null}
                  {aiError ? <p className="rounded-lg bg-error-container/60 p-2.5 text-[11px] text-on-error-container">{aiError}</p> : null}
                  {aiStatus && aiPhase !== "running" ? <p className="text-[10.5px] text-outline">{aiStatus}</p> : null}

                  {aiAnswer ? (
                    <div className="rounded-lg border border-outline-variant/35 bg-surface-container-lowest p-2.5" data-testid="reading-ai-answer">
                      <div className="mb-1 flex items-center justify-between">
                        <p className="text-[11px] font-medium text-on-surface">AI 解释（未复核）</p>
                        <button
                          type="button"
                          onClick={() => void saveAiAnswer()}
                          disabled={aiAnswerSaved}
                          className="rounded-md bg-surface-container px-2 py-0.5 text-[10.5px] text-on-surface disabled:opacity-50"
                        >
                          {aiAnswerSaved ? "已存为想法" : "存为想法"}
                        </button>
                      </div>
                      <div className="max-h-[420px] overflow-y-auto">
                        <MarkdownText text={aiAnswer} />
                      </div>
                    </div>
                  ) : null}

                  {organized ? (
                    <div className="rounded-lg border border-outline-variant/35 bg-surface-container-lowest p-2.5" data-testid="reading-organized">
                      <div className="mb-1 flex items-center justify-between gap-2">
                        <p className="text-[11px] font-medium text-on-surface">整理稿（未复核）</p>
                        <button
                          type="button"
                          onClick={() => void saveOrganized()}
                          disabled={organizedSaved}
                          className="shrink-0 rounded-md bg-surface-container px-2 py-0.5 text-[10.5px] text-on-surface disabled:opacity-50"
                        >
                          {organizedSaved ? "已保存" : "保存整理稿"}
                        </button>
                      </div>
                      <div className="max-h-[520px] overflow-y-auto">
                        <MarkdownText text={organized} />
                      </div>
                      {organizedReceipt ? (
                        <p className="mt-1.5 text-[10.5px] text-outline" data-testid="reading-organized-receipt">
                          已核验引文 {organizedReceipt.verified} 条 / 未核验 {organizedReceipt.unverified} 条
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              )}
            </div>
          </aside>
        ) : null}
      </div>

      {selection && selectionPos ? (
        <div
          className="reading-selection-bar fixed z-50 flex items-center gap-0.5 rounded-lg border border-outline-variant/60 bg-surface-bright px-1 py-1 shadow-lg"
          style={{ top: selectionPos.top, left: selectionPos.left }}
          onMouseDown={(event) => event.preventDefault()}
        >
          <button type="button" onClick={() => void addHighlight()} className="rounded-md px-2 py-1 text-[11px] text-on-surface hover:bg-surface-container">
            划线
          </button>
          <button type="button" onClick={beginThought} className="rounded-md px-2 py-1 text-[11px] text-on-surface hover:bg-surface-container">
            想法
          </button>
          <button type="button" onClick={() => void addBookmark()} className="rounded-md px-2 py-1 text-[11px] text-on-surface hover:bg-surface-container">
            标段
          </button>
          <button type="button" onClick={askAi} className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-[11px] text-primary hover:bg-surface-container">
            <Icon name="spark" className="h-3 w-3" />
            问 AI
          </button>
        </div>
      ) : null}

      {composer ? (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/20 p-4">
          <div className="w-full max-w-lg rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl">
            <div className="flex items-center justify-between gap-3">
              <p className="text-[14px] font-medium text-on-surface">{composer.title}</p>
              <button type="button" onClick={() => setComposer(null)} className="text-outline hover:text-on-surface" aria-label="关闭">
                ×
              </button>
            </div>
            {composer.draft.quote ? (
              <blockquote className="mt-3 border-l-2 border-primary/40 pl-2.5 text-[11.5px] leading-relaxed text-on-surface-variant">
                {composer.draft.quote}
              </blockquote>
            ) : null}
            <textarea
              autoFocus
              value={composer.text}
              onChange={(event) => setComposer({ ...composer, text: event.target.value })}
              placeholder="写下你的想法、疑问或要查的概念…"
              className="mt-3 h-36 w-full resize-none rounded-lg border border-outline-variant/50 bg-surface-container-lowest px-3 py-2 text-[12.5px] leading-relaxed text-on-surface outline-none focus:border-primary"
            />
            <div className="mt-3 flex justify-end gap-2">
              <button type="button" onClick={() => setComposer(null)} className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container">
                取消
              </button>
              <button
                type="button"
                onClick={() => void saveComposer()}
                disabled={composerBusy}
                className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-50"
              >
                {composerBusy ? "保存中…" : "保存"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {noteView ? (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/20 p-4">
          <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl">
            <div className="flex items-center justify-between gap-3 border-b border-outline-variant/30 pb-2">
              <p className="text-[13px] font-medium text-on-surface">{noteView.title}</p>
              <button type="button" onClick={() => setNoteView(null)} className="text-outline hover:text-on-surface" aria-label="关闭整理稿">
                ×
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto pt-3">
              <MarkdownText text={noteView.markdown} />
            </div>
          </div>
        </div>
      ) : null}

      {exportFile ? (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/20 p-4">
          <div className="flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl">
            <div className="flex items-center justify-between gap-3 border-b border-outline-variant/30 pb-2">
              <div>
                <p className="text-[13px] font-medium text-on-surface">已导出读书笔记</p>
                <p className="truncate text-[10px] text-outline" title={exportFile.path}>{exportFile.path}</p>
              </div>
              <button type="button" onClick={() => setExportFile(null)} className="text-outline hover:text-on-surface" aria-label="关闭导出结果">
                ×
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto pt-3">
              <MarkdownText text={exportFile.content} />
            </div>
          </div>
        </div>
      ) : null}

      {toast ? (
        <div className="fixed bottom-5 left-1/2 z-[80] max-w-[min(620px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-outline-variant/50 bg-surface-bright px-3 py-2 text-[11.5px] leading-relaxed text-on-surface shadow-lg" role="status">
          {toast}
        </div>
      ) : null}

      <ConfirmDialog
        open={pendingDelete !== null}
        title="删除这条批注？"
        description="删除只会改动本机阅读目录里的这条批注，不会改动导入的原文件。"
        confirmLabel="删除"
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => void removeAnnotation()}
        testId="reading-delete-confirm"
      />
    </div>
  );
}
