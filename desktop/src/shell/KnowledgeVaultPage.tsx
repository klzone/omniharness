import { useCallback, useEffect, useRef, useState } from "react";
import { shell } from "../api";
import type {
  KnowledgeVaultFile,
  KnowledgeVaultIndex,
  KnowledgeVaultIndexLink,
  KnowledgeVaultNote,
  KnowledgeVaultNoteLinks,
  KnowledgeVaultParsedNote,
  KnowledgeVaultPropertyValue,
  KnowledgeVaultRootStatus,
  KnowledgeVaultSaveOutcome,
  KnowledgeVaultScan,
  KnowledgeVaultSearchResponse,
} from "../types";
import { MarkdownText } from "./MarkdownText";
import { ConfirmDialog } from "./ConfirmDialog";
import { KnowledgeGraphView } from "./KnowledgeGraphView";

type EditorMode = "edit" | "preview" | "raw";
type VaultMode = "notes" | "graph";

type VaultConfirm = {
  title: string;
  description: string;
  confirmLabel: string;
  action: () => void;
};

type VaultFocus = { path: string; line: number };

const LINK_STATUS_LABEL: Record<string, string> = {
  resolved: "已解析",
  unresolved: "未解析",
  ambiguous: "歧义",
  external: "外部链接",
  attachment: "附件",
};

function linkLabel(link: KnowledgeVaultIndexLink): string {
  const target = link.target_raw === "" ? "本篇" : link.target_raw;
  return link.display ? `${link.display} · ${target}` : target;
}

function errText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function propertyValueText(value: KnowledgeVaultPropertyValue): string {
  switch (value.kind) {
    case "null":
      return "空值";
    case "bool":
      return value.value ? "是" : "否";
    case "list":
      return value.value.length > 0 ? value.value.join("、") : "（空列表）";
    default:
      return value.value === "" ? "（空字符串）" : value.value;
  }
}

function parseTags(raw: string): string[] {
  return raw
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
}

/**
 * 知识库分页（OD-KB-001 P2-S04/P2-S05）。
 *
 * 设计参考（screenshots-design-inspiration，2026-10-26）：Evernote 的左列表、
 * 中正文、右辅助信息三栏，以及 Notion 的轻量页面树与低装饰行内操作。
 *
 * 只调用宿主已校验的知识库命令；不访问网络、不读附件正文、不写本地缓存。
 * 草稿只保存在组件内存里：切换笔记或离开页面必须先确认，重启不会恢复草稿。
 * 保存后一律重新读取宿主文件，界面不拿本地草稿冒充已落盘内容。
 */
export function KnowledgeVaultPage({
  onOpenSettings,
  onDirtyChange,
}: {
  onOpenSettings: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const [status, setStatus] = useState<KnowledgeVaultRootStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [scan, setScan] = useState<KnowledgeVaultScan | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanBusy, setScanBusy] = useState(false);

  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [note, setNote] = useState<KnowledgeVaultNote | null>(null);
  const [parsed, setParsed] = useState<KnowledgeVaultParsedNote | null>(null);
  const [noteBusy, setNoteBusy] = useState(false);
  const [noteError, setNoteError] = useState<string | null>(null);
  const [mode, setMode] = useState<EditorMode>("edit");
  const [draft, setDraft] = useState("");

  const [saveBusy, setSaveBusy] = useState(false);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<string | null>(null);
  const [rereadBusy, setRereadBusy] = useState(false);
  // 草稿丢弃一律先走应用内确认：WebKitGTK 下 window.confirm 不阻塞，会静默丢草稿。
  const [confirm, setConfirm] = useState<VaultConfirm | null>(null);

  const [keyword, setKeyword] = useState("");
  const [tagFilter, setTagFilter] = useState("");
  const [propertyKey, setPropertyKey] = useState("");
  const [propertyValue, setPropertyValue] = useState("");
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchResult, setSearchResult] = useState<KnowledgeVaultSearchResponse | null>(null);

  const [newPath, setNewPath] = useState("");
  const [newContent, setNewContent] = useState("");
  const [newBusy, setNewBusy] = useState(false);
  const [newError, setNewError] = useState<string | null>(null);
  const [newMessage, setNewMessage] = useState<string | null>(null);

  const [attachmentBusy, setAttachmentBusy] = useState<string | null>(null);
  const [attachmentMessage, setAttachmentMessage] = useState<string | null>(null);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);

  // P3：内存关系索引。只由显式刷新触发，离开页面不保留。
  const [index, setIndex] = useState<KnowledgeVaultIndex | null>(null);
  const [indexBusy, setIndexBusy] = useState(false);
  const [indexError, setIndexError] = useState<string | null>(null);
  const [relations, setRelations] = useState<KnowledgeVaultNoteLinks | null>(null);
  const [relationsBusy, setRelationsBusy] = useState(false);
  const [relationsError, setRelationsError] = useState<string | null>(null);
  // 已解析链接的跳转定位：行号在目标笔记加载后滚动并显示。
  const [focus, setFocus] = useState<VaultFocus | null>(null);
  const [focusNotice, setFocusNotice] = useState<string | null>(null);
  // 图谱只读当前索引快照；切回「笔记」保留原有选中与草稿状态。
  const [vaultMode, setVaultMode] = useState<VaultMode>("notes");

  const dirty = note !== null && draft !== note.content;
  const loadSeq = useRef(0);
  const editorRef = useRef<HTMLTextAreaElement | null>(null);
  // 保存请求是异步的：用 ref 读取「此刻」的草稿，避免闭包里拿到提交瞬间的旧值。
  const draftRef = useRef("");

  const updateDraft = (value: string) => {
    draftRef.current = value;
    setDraft(value);
  };

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    return () => onDirtyChange?.(false);
  }, [onDirtyChange]);

  useEffect(() => {
    if (!dirty) return;
    const guard = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);

  const loadNote = useCallback(
    async (
      relativePath: string,
      options?: { force?: boolean; focusLine?: number | null; anchorMissing?: boolean },
    ) => {
      if (!options?.force && dirty && relativePath !== selectedPath) {
        setConfirm({
          title: "切换笔记？",
          description: "当前笔记有未保存的修改。切换会丢弃这些修改，但不会改动磁盘文件。",
          confirmLabel: "切换并丢弃",
          action: () => void loadNote(relativePath, { ...options, force: true }),
        });
        return;
      }
      const seq = ++loadSeq.current;
      setSelectedPath(relativePath);
      setNoteBusy(true);
      setNoteError(null);
      setSaveMessage(null);
      setSaveError(null);
      setConflict(null);
      setFocusNotice(null);
      setNote(null);
      setParsed(null);
      try {
        const [loaded, doc] = await Promise.all([
          shell.knowledgeVaultReadNote(relativePath),
          shell.knowledgeVaultParseNote(relativePath),
        ]);
        if (seq !== loadSeq.current) return;
        setNote(loaded);
        setParsed(doc);
        updateDraft(loaded.content);
        setMode("edit");
        setFocus(
          options?.focusLine
            ? { path: relativePath, line: Math.max(1, options.focusLine) }
            : null,
        );
        if (options?.focusLine) {
          setFocusNotice(
            options.anchorMissing
              ? "已打开目标笔记顶部，但链接里的标题或块锚点不存在。"
              : `已定位到第 ${Math.max(1, options.focusLine)} 行。`,
          );
        }
      } catch (error) {
        if (seq !== loadSeq.current) return;
        setNoteError(errText(error));
      } finally {
        if (seq === loadSeq.current) setNoteBusy(false);
      }
    },
    [dirty, selectedPath],
  );

  const loadScan = useCallback(async () => {
    setScanBusy(true);
    setScanError(null);
    try {
      const result = await shell.knowledgeVaultScan();
      setScan(result);
    } catch (error) {
      setScanError(errText(error));
    } finally {
      setScanBusy(false);
    }
  }, []);

  const rebuildIndex = useCallback(async () => {
    setIndexBusy(true);
    setIndexError(null);
    try {
      const built = await shell.knowledgeVaultRebuildIndex();
      setIndex(built);
      return built;
    } catch (error) {
      setIndexError(errText(error));
      return null;
    } finally {
      setIndexBusy(false);
    }
  }, []);

  /** 只跳转宿主已解析出目标路径的引用；未解析/歧义/外部/附件都不提供跳转。 */
  const openIndexLink = (link: KnowledgeVaultIndexLink, fromBacklink = false) => {
    if (link.status !== "resolved" || !link.target_path) return;
    const hasAnchor = Boolean(link.anchor);
    // 回链跳到来源行，锚点是目标笔记里的位置，与来源笔记无关，不能拿来判缺失。
    const anchorMissing = !fromBacklink && hasAnchor && link.anchor_status !== "exact";
    // 反向链接要定位到来源笔记里写下这条引用的那一行，而不是目标笔记。
    const line = fromBacklink ? link.line : hasAnchor ? link.anchor_line ?? 1 : null;
    // 反向链接的目标是这条引用的来源笔记；正向链接才跳到解析出的目标。
    const path = fromBacklink ? link.source_path : link.target_path;
    const nextFocus = line ? { path, line: Math.max(1, line) } : null;
    if (path === selectedPath && note) {
      setMode("edit");
      setFocus(nextFocus);
      setFocusNotice(
        nextFocus
          ? anchorMissing
            ? "已停留在当前笔记顶部，但链接里的标题或块锚点不存在。"
            : `已定位到第 ${nextFocus.line} 行。`
          : null,
      );
      return;
    }
    void loadNote(path, { focusLine: line, anchorMissing });
  };

  // 目标笔记渲染完成后滚动到锚点行；只滚动编辑器，不选中或改动草稿。
  useEffect(() => {
    if (!focus || mode !== "edit" || focus.path !== selectedPath) return;
    const el = editorRef.current;
    if (!el) return;
    const lineHeight = Number.parseFloat(window.getComputedStyle(el).lineHeight) || 18;
    el.scrollTop = Math.max(0, (focus.line - 3) * lineHeight);
    el.focus({ preventScroll: true });
  }, [focus, mode, selectedPath]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setStatusError(null);
      try {
        const current = await shell.knowledgeVaultStatus();
        if (cancelled) return;
        setStatus(current);
        if (current.configured && !current.error) {
          await loadScan();
        }
      } catch (error) {
        if (!cancelled) setStatusError(errText(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [loadScan]);

  // 打开页面时读取已发布快照；没有就等用户显式刷新，不在后台自动扫描。
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const current = await shell.knowledgeVaultIndexStatus();
        if (!cancelled) setIndex(current);
      } catch (error) {
        if (!cancelled) setIndexError(errText(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // 索引或当前笔记变化时刷新关系视图；只读当前快照，不再访问磁盘。
  useEffect(() => {
    if (!selectedPath || !index) {
      setRelations(null);
      setRelationsError(null);
      return;
    }
    let cancelled = false;
    (async () => {
      setRelationsBusy(true);
      setRelationsError(null);
      try {
        const view = await shell.knowledgeVaultNoteLinks(selectedPath);
        if (!cancelled) setRelations(view);
      } catch (error) {
        if (!cancelled) {
          setRelations(null);
          setRelationsError(errText(error));
        }
      } finally {
        if (!cancelled) setRelationsBusy(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedPath, index]);

  /** 放弃草稿：说明写的是「恢复成磁盘版本」，所以必须回到宿主重新读取。 */
  const discardDraftToDisk = async () => {
    if (!note) return;
    const seq = loadSeq.current;
    try {
      const [loaded, doc] = await Promise.all([
        shell.knowledgeVaultReadNote(note.relative_path),
        shell.knowledgeVaultParseNote(note.relative_path),
      ]);
      if (seq !== loadSeq.current) return;
      setNote(loaded);
      setParsed(doc);
      updateDraft(loaded.content);
      setSaveError(null);
      setConflict(null);
      setSaveMessage("已放弃本次草稿，并重新读取磁盘版本；磁盘文件未改动。");
    } catch (error) {
      setSaveError(`放弃修改后重新读取失败：${errText(error)}；草稿仍保留在内存中。`);
    }
  };

  const discardDraft = () => {
    if (!note || !dirty) return;
    setConfirm({
      title: "放弃未保存的修改？",
      description: "草稿会丢弃，并重新读取磁盘上的当前版本；磁盘文件不会被改动。",
      confirmLabel: "确认放弃",
      action: () => void discardDraftToDisk(),
    });
  };

  /** 回到新建表单：始终从页面头部可达，不必先清空当前选择。 */
  const openNewForm = () => {
    loadSeq.current += 1;
    setSelectedPath(null);
    setNote(null);
    setParsed(null);
    updateDraft("");
    setNoteBusy(false);
    setNoteError(null);
    setSaveMessage(null);
    setSaveError(null);
    setConflict(null);
    setMode("edit");
    setFocus(null);
    setFocusNotice(null);
  };

  const startNew = () => {
    if (dirty) {
      setConfirm({
        title: "新建笔记？",
        description: "当前笔记有未保存的修改。新建会丢弃这些修改，但不会改动磁盘文件。",
        confirmLabel: "新建并丢弃",
        action: openNewForm,
      });
      return;
    }
    openNewForm();
  };

  const saveNote = async () => {
    if (!note || !dirty || saveBusy) return;
    const editingPath = note.relative_path;
    const submitted = draft;
    const seq = loadSeq.current;
    setSaveBusy(true);
    setSaveError(null);
    setConflict(null);
    setSaveMessage(null);
    try {
      const outcome: KnowledgeVaultSaveOutcome = await shell.knowledgeVaultSaveNote(
        editingPath,
        submitted,
        note.sha256,
      );
      const [loaded, doc] = await Promise.all([
        shell.knowledgeVaultReadNote(editingPath),
        shell.knowledgeVaultParseNote(editingPath),
      ]);
      // 保存期间若已切到别的笔记，不能把旧笔记的结果写回当前页面。
      if (seq !== loadSeq.current) return;
      setNote(loaded);
      setParsed(doc);
      // 保存请求发出后仍可能收到新的输入（例如键盘事件已在队列里）。只有草稿仍是
      // 提交的那一份时才用落盘内容替换；否则保留更新的草稿，并明确说明磁盘版本。
      const replaced = draftRef.current === submitted;
      if (replaced) updateDraft(loaded.content);
      setSaveMessage(
        replaced
          ? `已保存 ${outcome.relative_path}（${outcome.bytes} 字节，sha256 ${outcome.sha256.slice(0, 12)}…）；` +
              "下方内容来自宿主保存后的重新读取。"
          : `已保存提交时的版本 ${outcome.relative_path}；保存期间又有新的输入，已保留在草稿里，尚未落盘。`,
      );
      await loadScan();
    } catch (error) {
      const message = errText(error);
      if (message.includes("[conflict]")) {
        setConflict(message);
        setSaveError("未写入：文件已被外部修改。当前草稿仍在内存中，磁盘文件没有被覆盖。");
      } else {
        setSaveError(`未写入：${message}`);
      }
    } finally {
      setSaveBusy(false);
    }
  };

  const rereadExternal = async () => {
    if (!note || rereadBusy) return;
    setConfirm({
      title: "重新读取外部版本？",
      description: "磁盘上的外部版本会替换当前草稿；磁盘文件不会被改动。",
      confirmLabel: "重新读取",
      action: () => void applyExternalReread(),
    });
  };

  const applyExternalReread = async () => {
    if (!note || rereadBusy) return;
    setRereadBusy(true);
    setSaveError(null);
    setConflict(null);
    try {
      const [loaded, doc] = await Promise.all([
        shell.knowledgeVaultReadNote(note.relative_path),
        shell.knowledgeVaultParseNote(note.relative_path),
      ]);
      setNote(loaded);
      setParsed(doc);
      updateDraft(loaded.content);
      setSaveMessage("已重新读取磁盘版本；当前内容来自宿主读取结果。");
      await loadScan();
    } catch (error) {
      setSaveError(`重新读取失败：${errText(error)}`);
    } finally {
      setRereadBusy(false);
    }
  };

  const runSearch = async () => {
    setSearchBusy(true);
    setSearchError(null);
    try {
      const result = await shell.knowledgeVaultSearch({
        keyword: keyword.trim(),
        tags: parseTags(tagFilter),
        properties: propertyKey.trim()
          ? [{ key: propertyKey.trim(), value: propertyValue.trim() }]
          : [],
      });
      setSearchResult(result);
    } catch (error) {
      setSearchError(errText(error));
    } finally {
      setSearchBusy(false);
    }
  };

  const clearSearch = () => {
    setKeyword("");
    setTagFilter("");
    setPropertyKey("");
    setPropertyValue("");
    setSearchError(null);
    setSearchResult(null);
  };

  const createNote = async () => {
    if (newBusy) return;
    const relativePath = newPath.trim();
    if (!relativePath) {
      setNewError("请填写知识库内的 Markdown 相对路径，例如 笔记/新想法.md。");
      return;
    }
    if (dirty) {
      setConfirm({
        title: "新建并打开这篇笔记？",
        description: "当前笔记有未保存的修改。新建并切换会丢弃这些修改，但不会改动磁盘文件。",
        confirmLabel: "新建并丢弃",
        action: () => void createNoteNow(relativePath),
      });
      return;
    }
    await createNoteNow(relativePath);
  };

  const createNoteNow = async (relativePath: string) => {
    if (newBusy) return;
    setNewBusy(true);
    setNewError(null);
    setNewMessage(null);
    try {
      const created = await shell.knowledgeVaultCreateNote(relativePath, newContent);
      setNewPath("");
      setNewContent("");
      setNewMessage(`已新建 ${created.relative_path}；内容来自宿主返回。`);
      await loadScan();
      await loadNote(created.relative_path, { force: true });
      // loadNote 会清空保存提示；新建结果必须继续可见，不能静默消失。
      setSaveMessage(`已新建 ${created.relative_path}；内容来自宿主返回。`);
    } catch (error) {
      setNewError(`新建失败：${errText(error)}`);
    } finally {
      setNewBusy(false);
    }
  };

  const openAttachment = async (file: KnowledgeVaultFile) => {
    if (attachmentBusy) return;
    setAttachmentBusy(file.relative_path);
    setAttachmentMessage(null);
    setAttachmentError(null);
    try {
      const opened = await shell.knowledgeVaultOpenAttachment(file.relative_path);
      setAttachmentMessage(`已交给系统处理程序：${opened}`);
    } catch (error) {
      setAttachmentError(`打开失败：${errText(error)}`);
    } finally {
      setAttachmentBusy(null);
    }
  };

  const notes = scan?.files.filter((file) => file.kind === "note") ?? [];
  const attachments = scan?.files.filter((file) => file.kind === "attachment") ?? [];
  const skippedCount = scan
    ? scan.stats.skipped_hidden +
      scan.stats.skipped_excluded +
      scan.stats.skipped_symlink +
      scan.stats.skipped_non_file +
      scan.stats.skipped_depth
    : 0;

  return (
    <main
      className="flex-1 min-h-0 flex flex-col gap-2.5 px-6 md:pl-28 md:pr-10 pb-5"
      data-testid="knowledge-vault-page"
    >
      <header className="flex-none rounded-2xl bg-white/80 border border-white/90 px-4 py-3 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)]">
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="font-headline-md text-[16px] font-semibold text-on-surface">知识库</h1>
          <span className="mono text-[10px] text-outline">本机 Markdown · 显式保存</span>
          <div
            className="ml-2 flex items-center gap-0.5 rounded-lg bg-surface-container p-0.5"
            role="group"
            aria-label="知识库视图"
          >
            <button
              className={
                "rounded-md px-2.5 py-1 text-[11px] " +
                (vaultMode === "notes"
                  ? "bg-white text-on-surface shadow-sm"
                  : "text-on-surface-variant hover:text-primary")
              }
              onClick={() => setVaultMode("notes")}
              aria-pressed={vaultMode === "notes"}
              data-testid="vault-view-notes"
            >
              笔记
            </button>
            <button
              className={
                "rounded-md px-2.5 py-1 text-[11px] " +
                (vaultMode === "graph"
                  ? "bg-white text-on-surface shadow-sm"
                  : "text-on-surface-variant hover:text-primary")
              }
              onClick={() => setVaultMode("graph")}
              aria-pressed={vaultMode === "graph"}
              data-testid="vault-view-graph"
            >
              图谱
            </button>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <button
              className="rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
              onClick={startNew}
              disabled={!status?.configured || !!status?.error}
              data-testid="vault-new-start"
            >
              新建笔记
            </button>
            <button
              className="rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
              onClick={() => void loadScan()}
              disabled={scanBusy || !status?.configured || !!status?.error}
              data-testid="vault-refresh"
            >
              {scanBusy ? "扫描中…" : "刷新扫描"}
            </button>
            <button
              className="rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
              onClick={() => void rebuildIndex()}
              disabled={indexBusy || !status?.configured || !!status?.error}
              data-testid="vault-index-refresh"
            >
              {indexBusy ? "构建中…" : index ? "重建关系索引" : "构建关系索引"}
            </button>
            <button
              className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:text-primary"
              onClick={onOpenSettings}
            >
              知识库设置
            </button>
          </div>
        </div>
        <div
          className="mt-2 rounded-xl bg-surface-container-low px-3 py-2 text-[11px] leading-relaxed text-on-surface-variant ring-1 ring-outline-variant/30"
          role="status"
          aria-live="polite"
          aria-label={
            status?.configured
              ? `知识库根 ${status.root ?? "不可用"}；${
                  status.error ? `不可用 ${status.error}` : "已配置"
                }`
              : "知识库根未设置"
          }
          data-testid="vault-root-status"
        >
          <span className="k text-outline">库根 </span>
          <span className="mono break-all">
            {status === null
              ? "读取中…"
              : status.configured
                ? status.root ?? "不可用"
                : "未设置"}
          </span>
          {status?.configured && status.error ? (
            <span className="ml-2 text-error">不可用：{status.error}</span>
          ) : null}
          {status?.configured && !status.error && scan ? (
            <span className="ml-2">
              {scan.stats.files} 个文件（笔记 {scan.stats.notes}、附件 {scan.stats.attachments}），
              共 {formatBytes(scan.stats.total_bytes)}，跳过 {skippedCount} 项，错误{" "}
              {scan.stats.errors} 项
              {scan.stats.truncated ? "（达到扫描上限，结果不完整）" : ""}
            </span>
          ) : null}
        </div>
        {statusError ? (
          <p className="mt-2 text-[11.5px] text-error" role="alert">
            无法读取知识库状态：{statusError}
          </p>
        ) : null}
        {scanError ? (
          <p className="mt-2 text-[11.5px] text-error" role="alert">
            扫描失败：{scanError}
          </p>
        ) : null}
      </header>

      {status && !status.configured ? (
        <section
          className="flex-1 min-h-0 flex items-center justify-center"
          role="note"
          aria-label="未设置知识库目录：去设置选择目录"
          data-testid="vault-unset-hint"
        >
          <div className="max-w-lg rounded-2xl bg-white/85 border border-white/90 px-6 py-8 text-center shadow-[0_8px_28px_-12px_rgba(15,23,42,0.12)]">
            <p className="text-[14px] font-semibold text-on-surface">还没有选择知识库目录</p>
            <p className="mt-2 text-[12px] leading-relaxed text-on-surface-variant">
              知识库使用一个已有的 Markdown 或 Obsidian 目录。OmniDesk 只读取笔记正文，
              不会创建、移动或删除其中的文件。
            </p>
            <button
              className="mt-4 rounded-lg bg-primary px-4 py-2 text-[12px] text-on-primary"
              onClick={onOpenSettings}
            >
              去设置选择目录
            </button>
          </div>
        </section>
      ) : status?.configured && status.error ? (
        <section className="flex-1 min-h-0 flex items-center justify-center">
          <div className="max-w-lg rounded-2xl bg-white/85 border border-white/90 px-6 py-8 text-center shadow-[0_8px_28px_-12px_rgba(15,23,42,0.12)]">
            <p className="text-[14px] font-semibold text-on-surface">知识库目录不可用</p>
            <p className="mt-2 text-[12px] leading-relaxed text-error">{status.error}</p>
            <p className="mt-2 text-[11.5px] text-on-surface-variant">
              请到设置页重新选择目录；这里不会访问该路径或改动任何文件。
            </p>
            <button className="mt-4 rounded-lg bg-primary px-4 py-2 text-[12px] text-on-primary" onClick={onOpenSettings}>
              去设置
            </button>
          </div>
        </section>
      ) : status?.configured ? (
        vaultMode === "graph" ? (
          <KnowledgeGraphView
            index={index}
            selectedPath={selectedPath}
            indexBusy={indexBusy}
            indexError={indexError}
            onRequestIndex={() => void rebuildIndex()}
            onOpenNote={(relativePath) => {
              if (relativePath === selectedPath && note) {
                setVaultMode("notes");
                return;
              }
              // 未保存草稿的确认由 loadNote 统一处理；确认后仍停在图谱里会看不到结果。
              setVaultMode("notes");
              void loadNote(relativePath);
            }}
          />
        ) : (
        <div className="flex-1 min-h-0 overflow-y-auto xl:overflow-hidden">
          <div className="h-full min-h-0 grid grid-cols-1 xl:grid-cols-[minmax(230px,290px)_minmax(0,1.35fr)_minmax(240px,310px)] gap-2.5">
            <aside className="min-h-0 flex flex-col gap-2.5">
              <section className="rounded-2xl bg-white/80 border border-white/90 p-3 flex flex-col min-h-[220px] xl:flex-1 xl:min-h-0">
                <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">笔记</h2>
                  <span className="mono text-[10px] text-outline">{notes.length}</span>
                </div>
                <div className="pt-2 flex-1 min-h-0 overflow-y-auto" data-testid="vault-note-list">
                  {notes.length === 0 ? (
                    <p className="text-[11px] text-on-surface-variant">扫描结果里没有 Markdown 笔记。</p>
                  ) : (
                    notes.map((file) => (
                      <button
                        key={file.relative_path}
                        onClick={() => void loadNote(file.relative_path)}
                        aria-current={selectedPath === file.relative_path ? "true" : undefined}
                        className={
                          "w-full text-left rounded-lg px-2.5 py-2 mb-1 " +
                          (selectedPath === file.relative_path
                            ? "bg-primary/10 text-primary"
                            : "text-on-surface-variant hover:bg-surface-container-low")
                        }
                        data-testid="vault-note-item"
                        data-path={file.relative_path}
                      >
                        <span className="block text-[12px] font-medium break-words">{file.file_name}</span>
                        <span className="block mono text-[9.5px] text-outline break-all">
                          {file.relative_path}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              </section>

              <section className="rounded-2xl bg-white/80 border border-white/90 p-3 flex flex-col max-h-[26vh] xl:max-h-[28%]">
                <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">附件</h2>
                  <span className="mono text-[10px] text-outline">{attachments.length}</span>
                </div>
                <div className="pt-2 min-h-0 overflow-y-auto">
                  {attachments.length === 0 ? (
                    <p className="text-[11px] text-on-surface-variant">没有附件。</p>
                  ) : (
                    attachments.map((file) => (
                      <div key={file.relative_path} className="flex items-center gap-1.5 py-1">
                        <span className="min-w-0 flex-1 mono text-[10px] text-on-surface-variant break-all">
                          {file.relative_path}
                          <span className="ml-1 text-outline">({formatBytes(file.bytes)})</span>
                        </span>
                        <button
                          className="flex-none rounded border border-outline-variant/50 px-1.5 py-0.5 text-[10px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-45"
                          onClick={() => void openAttachment(file)}
                          disabled={attachmentBusy !== null}
                          data-testid="vault-open-attachment"
                          data-path={file.relative_path}
                        >
                          {attachmentBusy === file.relative_path ? "打开中…" : "打开"}
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </section>

              {attachmentMessage || attachmentError ? (
                <p
                  className={
                    "text-[10.5px] leading-relaxed rounded-lg px-2 py-1.5 " +
                    (attachmentError
                      ? "bg-error-container/60 text-on-error-container"
                      : "bg-surface-container-low text-on-surface-variant")
                  }
                  role="status"
                  aria-live="polite"
                >
                  {attachmentError ?? attachmentMessage}
                </p>
              ) : null}
            </aside>

            <section className="min-h-0 flex flex-col rounded-2xl bg-white/85 border border-white/90 overflow-hidden">
              {noteBusy ? (
                <p className="p-4 text-[11.5px] text-outline">正在读取笔记…</p>
              ) : noteError ? (
                <div className="p-4">
                  <p className="text-[12px] text-error" role="alert">
                    读取失败：{noteError}
                  </p>
                </div>
              ) : !note ? (
                <div className="p-5 text-[11.5px] leading-relaxed text-on-surface-variant">
                  <p className="text-[13px] font-semibold text-on-surface">选择一篇笔记</p>
                  <p className="mt-1">从左侧列表打开原文、预览和属性。编辑只保存在内存里，明确保存后才写盘。</p>
                  <div className="mt-4 rounded-xl bg-surface-container-low p-3">
                    <p className="text-[12px] font-semibold text-on-surface">新建笔记</p>
                    <label className="mt-2 block text-[10.5px] text-outline" htmlFor="vault-new-path">
                      相对路径（父目录必须已存在）
                    </label>
                    <input
                      id="vault-new-path"
                      className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 mono text-[11px]"
                      value={newPath}
                      onChange={(event) => setNewPath(event.target.value)}
                      placeholder="笔记/新想法.md"
                      data-testid="vault-new-path"
                    />
                    <label className="mt-2 block text-[10.5px] text-outline" htmlFor="vault-new-content">
                      初始内容
                    </label>
                    <textarea
                      id="vault-new-content"
                      className="mt-1 w-full min-h-20 rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 mono text-[11px]"
                      value={newContent}
                      onChange={(event) => setNewContent(event.target.value)}
                      placeholder="# 新笔记&#10;&#10;"
                      data-testid="vault-new-content"
                    />
                    <button
                      className="mt-2 rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
                      onClick={() => void createNote()}
                      disabled={newBusy}
                      data-testid="vault-new-create"
                    >
                      {newBusy ? "创建中…" : "创建笔记"}
                    </button>
                    {newError ? (
                      <p className="mt-2 text-[11px] text-error" role="alert" aria-label={newError}>
                        {newError}
                      </p>
                    ) : null}
                    {newMessage ? (
                      <p className="mt-2 text-[11px] text-on-surface-variant" role="status">
                        {newMessage}
                      </p>
                    ) : null}
                  </div>
                </div>
              ) : (
                <>
                  <div className="flex-none px-3.5 py-2.5 border-b border-surface-variant/20">
                    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                      <h2 className="text-[13px] font-semibold text-on-surface break-all" data-testid="vault-note-path">
                        {note.relative_path}
                      </h2>
                      <span className="mono text-[9.5px] text-outline">
                        {formatBytes(note.bytes)} · sha256 {note.sha256.slice(0, 10)}…
                      </span>
                      {dirty ? (
                        <span
                          className="rounded-full bg-amber-100 px-2 py-0.5 text-[9.5px] text-amber-800"
                          role="status"
                          aria-label="未保存草稿"
                          data-testid="vault-unsaved"
                        >
                          未保存草稿
                        </span>
                      ) : (
                        <span className="text-[9.5px] text-outline">与磁盘一致</span>
                      )}
                    </div>
                    <div className="mt-2 flex flex-wrap items-center gap-1">
                      {(
                        [
                          ["edit", "编辑"],
                          ["preview", "预览"],
                          ["raw", "原文"],
                        ] as const
                      ).map(([key, label]) => (
                        <button
                          key={key}
                          onClick={() => setMode(key)}
                          className={
                            "rounded px-2 py-1 text-[10.5px] " +
                            (mode === key
                              ? "bg-primary text-on-primary"
                              : "bg-surface-container text-on-surface-variant")
                          }
                          data-testid={`vault-mode-${key}`}
                        >
                          {label}
                        </button>
                      ))}
                      <span className="ml-auto text-[9.5px] text-outline">
                        {mode === "edit" ? "仅内存草稿" : "已保存版本"}
                      </span>
                    </div>
                  </div>

                  <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3">
                    {mode === "edit" ? (
                      <textarea
                        ref={editorRef}
                        className="w-full h-full min-h-[300px] resize-none rounded-xl border border-outline-variant/40 bg-surface-container-lowest px-3 py-2.5 mono text-[11.5px] leading-relaxed outline-none focus:border-primary"
                        value={draft}
                        onChange={(event) => updateDraft(event.target.value)}
                        spellCheck={false}
                        aria-label="笔记草稿（包含 Frontmatter 原始文本）"
                        data-testid="vault-editor"
                      />
                    ) : mode === "preview" ? (
                      <div
                        aria-label={`预览正文：${parsed?.body ?? ""}`}
                        data-testid="vault-preview"
                      >
                        {dirty ? (
                          <p className="mb-2 rounded-lg bg-amber-50 px-2 py-1.5 text-[10.5px] text-amber-800">
                            当前有未保存草稿；预览显示的是磁盘上已保存的版本。保存后预览会自动更新。
                          </p>
                        ) : null}
                        {parsed ? (
                          <MarkdownText text={parsed.body} />
                        ) : (
                          <p className="text-[11px] text-outline">没有解析结果。</p>
                        )}
                      </div>
                    ) : (
                      <pre
                        className="whitespace-pre-wrap break-words mono text-[11px] leading-relaxed text-on-surface"
                        aria-label={`原文：${parsed?.raw ?? note.content}`}
                        data-testid="vault-raw"
                      >
                        {parsed?.raw ?? note.content}
                      </pre>
                    )}
                  </div>

                  <div className="flex-none border-t border-surface-variant/20 px-3.5 py-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <button
                        className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-45"
                        onClick={() => void saveNote()}
                        disabled={!dirty || saveBusy}
                        data-testid="vault-save"
                      >
                        {saveBusy ? "保存中…" : "保存"}
                      </button>
                      <button
                        className="rounded-lg border border-outline-variant/60 px-3 py-1.5 text-[11.5px] text-on-surface-variant disabled:opacity-45"
                        onClick={discardDraft}
                        disabled={!dirty}
                        data-testid="vault-discard"
                      >
                        放弃修改
                      </button>
                      {conflict ? (
                        <button
                          className="rounded-lg border border-error/50 px-3 py-1.5 text-[11.5px] text-error"
                          onClick={() => void rereadExternal()}
                          disabled={rereadBusy}
                          data-testid="vault-reread-external"
                        >
                          {rereadBusy ? "重新读取中…" : "重新读取外部版本"}
                        </button>
                      ) : null}
                      <span className="ml-auto text-[9.5px] text-outline">
                        保存前会重新比对打开时的 sha256
                      </span>
                    </div>
                    {saveError ? (
                      <p
                        className="mt-2 text-[11px] leading-relaxed text-error"
                        role="alert"
                        aria-label={saveError}
                        data-testid="vault-save-error"
                      >
                        {saveError}
                      </p>
                    ) : null}
                    {conflict ? (
                      <p
                        className="mt-1 mono text-[9.5px] leading-relaxed text-error/80 break-all"
                        role="alert"
                        aria-label={conflict}
                        data-testid="vault-conflict"
                      >
                        {conflict}
                      </p>
                    ) : null}
                    {saveMessage ? (
                      <p className="mt-2 text-[10.5px] leading-relaxed text-on-surface-variant" role="status" aria-live="polite">
                        {saveMessage}
                      </p>
                    ) : null}
                  </div>
                </>
              )}

              {!note ? (
                <div className="flex-none border-t border-surface-variant/20 px-3.5 py-2">
                  <p className="text-[10.5px] text-on-surface-variant">
                    新建只接受根内相对 Markdown 路径；父目录必须已存在，宿主不会隐式创建目录。
                  </p>
                </div>
              ) : null}
            </section>

            <aside className="min-h-0 flex flex-col gap-2.5 overflow-y-auto">
              <section
                className="rounded-2xl bg-white/80 border border-white/90 p-3"
                data-testid="vault-relations"
              >
                <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">链接关系</h2>
                  <span
                    className="mono text-[10px] text-outline"
                    role="status"
                    aria-label={
                      index
                        ? `关系索引第 ${index.generation} 代`
                        : relations
                          ? `关系索引第 ${relations.generation} 代`
                          : "关系索引未构建"
                    }
                    data-testid="vault-index-generation"
                  >
                    {index ? `第 ${index.generation} 代` : "未构建"}
                  </span>
                </div>
                {indexError ? (
                  <p className="pt-2 text-[10.5px] leading-relaxed text-error" role="alert">
                    索引构建失败：{indexError}
                  </p>
                ) : null}
                {!index ? (
                  <p
                    className="pt-2 text-[11px] leading-relaxed text-on-surface-variant"
                    role="status"
                    aria-label="还没有关系索引。点上方「构建关系索引」从当前笔记原文生成；索引只存在内存里，刷新前不会反映外部改动。"
                    data-testid="vault-index-empty"
                  >
                    还没有关系索引。点上方「构建关系索引」从当前笔记原文生成；索引只存在内存里，刷新前不会反映外部改动。
                  </p>
                ) : (
                  <div className="pt-2">
                    <p
                      className="text-[10.5px] leading-relaxed text-on-surface-variant"
                      role="status"
                      aria-label={`关系索引：${index.stats.notes} 篇笔记；${index.stats.resolved} 条已解析；${index.stats.unresolved} 条未解析；${index.stats.ambiguous} 条歧义；${index.stats.orphans} 篇孤立；${index.complete ? "完整" : "不完整"}`}
                      data-testid="vault-index-stats"
                    >
                      {index.stats.notes} 篇 · 已解析 {index.stats.resolved} · 未解析{" "}
                      {index.stats.unresolved} · 歧义 {index.stats.ambiguous} · 孤立 {index.stats.orphans}
                    </p>
                    {index.incomplete_reason ? (
                      <p
                        className="mt-1.5 rounded bg-amber-50 px-2 py-1 text-[10px] leading-relaxed text-amber-900"
                        role="alert"
                        data-testid="vault-index-incomplete"
                      >
                        索引不完整：{index.incomplete_reason}。未收录的笔记没有关系数据。
                      </p>
                    ) : null}
                    {(() => {
                      // 解析问题按当前笔记优先展示；没有选中时就展示全体。
                      const scoped = selectedPath
                        ? index.issues.filter((issue) => issue.relative_path === selectedPath)
                        : index.issues;
                      if (scoped.length === 0) return null;
                      const shown = scoped.slice(0, 4);
                      return (
                        <div
                          className="mt-1.5 rounded bg-amber-50/70 px-2 py-1.5"
                          data-testid="vault-index-issues"
                          role="status"
                          aria-label={`索引问题：共 ${scoped.length} 条，${shown
                            .map((issue) => `${issue.relative_path || "库"}：${issue.message}`)
                            .join("；")}`}
                        >
                          <p className="text-[10px] font-semibold text-amber-900">
                            索引问题 <span className="mono">{scoped.length}</span>
                          </p>
                          <ul className="mt-0.5 space-y-0.5">
                            {shown.map((issue, position) => (
                              <li
                                key={`${issue.relative_path}-${issue.code}-${position}`}
                                className="text-[9.5px] leading-relaxed text-amber-900 break-words"
                                data-code={issue.code}
                              >
                                {issue.relative_path ? `${issue.relative_path}：` : ""}
                                {issue.message}
                              </li>
                            ))}
                          </ul>
                          {scoped.length > shown.length ? (
                            <p className="mt-0.5 text-[9px] text-amber-900/80">
                              另有 {scoped.length - shown.length} 条未展开。
                            </p>
                          ) : null}
                        </div>
                      );
                    })()}
                    {!selectedPath ? (
                      <p className="mt-1.5 text-[10.5px] text-outline">选中一篇笔记后显示它与各处的链接。</p>
                    ) : relationsBusy && !relations ? (
                      <p className="mt-1.5 text-[10.5px] text-outline">读取关系…</p>
                    ) : relationsError ? (
                      <p className="mt-1.5 text-[10.5px] leading-relaxed text-error" role="alert">
                        {relationsError}
                      </p>
                    ) : relations ? (
                      <div className="mt-2 space-y-2">
                        {relations.note.orphan ? (
                          <p
                            className="rounded bg-surface-container-low px-2 py-1 text-[10px] text-on-surface-variant"
                            data-testid="vault-orphan-note"
                          >
                            这篇笔记没有其他笔记指向它（孤立笔记）。
                          </p>
                        ) : null}
                        {focusNotice ? (
                          <p
                            className="rounded bg-surface-container-low px-2 py-1 text-[10px] leading-relaxed text-on-surface-variant"
                            role="status"
                            aria-label={focusNotice}
                            data-testid="vault-focus-notice"
                          >
                            {focusNotice}
                          </p>
                        ) : null}
                        {(() => {
                          const fallbacks = relations.outgoing.filter(
                            (link) => link.anchor && link.anchor_status !== "exact",
                          );
                          if (fallbacks.length === 0) return null;
                          const summary = fallbacks
                            .map(
                              (link) =>
                                `第 ${link.line} 行的${link.block_anchor ? "块" : "标题"}锚点不存在，已回退到目标笔记顶部`,
                            )
                            .join("；");
                          return (
                            <p
                              className="rounded bg-amber-50 px-2 py-1 text-[10px] leading-relaxed text-amber-900"
                              role="status"
                              aria-label={`锚点回退：${summary}`}
                              data-testid="vault-anchor-fallbacks"
                            >
                              锚点回退：{summary}
                            </p>
                          );
                        })()}
                        <div>
                          <h3 className="text-[10.5px] font-semibold text-on-surface">
                            指向别处 <span className="mono text-[9.5px] text-outline">{relations.outgoing.length}</span>
                          </h3>
                          {relations.outgoing.length === 0 ? (
                            <p className="mt-1 text-[10px] text-outline">这篇笔记没有链接。</p>
                          ) : (
                            <ul className="mt-1 space-y-1" data-testid="vault-outgoing-links">
                              {relations.outgoing.map((link) => {
                                const canOpen = link.status === "resolved" && Boolean(link.target_path);
                                return (
                                  <li key={link.id}>
                                    <button
                                      className={
                                        "w-full rounded-lg px-2 py-1.5 text-left " +
                                        (canOpen
                                          ? "bg-surface-container-low hover:ring-1 hover:ring-primary/40"
                                          : "bg-surface-container-low/60")
                                      }
                                      onClick={() => openIndexLink(link)}
                                      disabled={!canOpen}
                                      aria-label={`${LINK_STATUS_LABEL[link.status] ?? link.status}：${linkLabel(link)}；第 ${link.line} 行`}
                                      data-testid="vault-outgoing-link"
                                      data-status={link.status}
                                      data-target={link.target_path ?? ""}
                                    >
                                      <span className="flex items-baseline gap-1">
                                        <span className="mono text-[9px] text-outline">{link.line}</span>
                                        <span className="min-w-0 flex-1 text-[10.5px] text-on-surface break-all">
                                          {linkLabel(link)}
                                        </span>
                                        <span
                                          className={
                                            "flex-none text-[9px] " +
                                            (link.status === "resolved"
                                              ? "text-primary"
                                              : link.status === "ambiguous" || link.status === "unresolved"
                                                ? "text-amber-800"
                                                : "text-outline")
                                          }
                                        >
                                          {LINK_STATUS_LABEL[link.status] ?? link.status}
                                        </span>
                                      </span>
                                      {link.status === "ambiguous" && link.candidates.length > 0 ? (
                                        <span className="mt-0.5 block text-[9px] leading-relaxed text-outline break-all">
                                          候选：{link.candidates.join("、")}
                                        </span>
                                      ) : null}
                                      {link.anchor && link.anchor_status !== "exact" ? (
                                        <span className="mt-0.5 block text-[9px] text-amber-800">
                                          {link.block_anchor ? "块" : "标题"}锚点不存在，只回退到笔记顶部
                                        </span>
                                      ) : null}
                                      {link.status === "attachment" ? (
                                        <span className="mt-0.5 block mono text-[9px] text-outline break-all">
                                          附件：{link.attachment_path ?? link.target_raw}
                                        </span>
                                      ) : null}
                                    </button>
                                  </li>
                                );
                              })}
                            </ul>
                          )}
                        </div>
                        <div>
                          <h3 className="text-[10.5px] font-semibold text-on-surface">
                            反向链接 <span className="mono text-[9.5px] text-outline">{relations.backlinks.length}</span>
                          </h3>
                          {relations.backlinks.length === 0 ? (
                            <p className="mt-1 text-[10px] text-outline">没有笔记链接到这篇。</p>
                          ) : (
                            <ul className="mt-1 space-y-1" data-testid="vault-backlinks">
                              {relations.backlinks.map((link) => (
                                <li key={`back-${link.id}`}>
                                  <button
                                    className="w-full rounded-lg bg-surface-container-low px-2 py-1.5 text-left hover:ring-1 hover:ring-primary/40"
                                    onClick={() => openIndexLink(link, true)}
                                    aria-label={`来自 ${link.source_path} 第 ${link.line} 行：${link.context}`}
                                    data-testid="vault-backlink"
                                    data-source={link.source_path}
                                  >
                                    <span className="block mono text-[9.5px] text-primary break-all">
                                      {link.source_path} · 第 {link.line} 行
                                    </span>
                                    {link.context ? (
                                      <span className="mt-0.5 block text-[10px] text-on-surface-variant break-words">
                                        {link.context}
                                      </span>
                                    ) : null}
                                  </button>
                                </li>
                              ))}
                            </ul>
                          )}
                        </div>
                        <div>
                          <h3 className="text-[10.5px] font-semibold text-on-surface">
                            孤立笔记 <span className="mono text-[9.5px] text-outline">{index.stats.orphans}</span>
                          </h3>
                          {index.stats.orphans === 0 ? (
                            <p className="mt-1 text-[10px] text-outline">每篇笔记都至少有一条入链。</p>
                          ) : (
                            <ul className="mt-1 space-y-1" data-testid="vault-orphans">
                              {index.notes
                                .filter((item) => item.orphan)
                                .slice(0, 8)
                                .map((item) => (
                                  <li key={`orphan-${item.relative_path}`}>
                                    <button
                                      className="w-full rounded-lg bg-surface-container-low px-2 py-1 text-left hover:ring-1 hover:ring-primary/40"
                                      onClick={() => {
                                        if (item.relative_path === selectedPath) return;
                                        void loadNote(item.relative_path);
                                      }}
                                      aria-label={`孤立笔记：${item.relative_path}`}
                                      data-testid="vault-orphan-item"
                                    >
                                      <span className="block mono text-[9.5px] text-primary break-all">
                                        {item.relative_path}
                                      </span>
                                    </button>
                                  </li>
                                ))}
                            </ul>
                          )}
                        </div>
                      </div>
                    ) : null}
                  </div>
                )}
              </section>

              <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
                <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">属性</h2>
                  <span className="mono text-[10px] text-outline">{parsed?.properties.length ?? 0}</span>
                </div>
                {!parsed ? (
                  <p className="pt-2 text-[11px] text-outline">选中笔记后显示。</p>
                ) : (
                  <>
                    {parsed.properties.length === 0 ? (
                      <p className="pt-2 text-[11px] text-on-surface-variant">没有识别到可展示属性。</p>
                    ) : (
                      <dl
                        className="pt-2 space-y-1.5"
                        aria-label={`属性列表：${parsed.properties
                          .map((property) => `${property.key}=${propertyValueText(property.value)}`)
                          .join("；")}`}
                        data-testid="vault-properties"
                      >
                        {parsed.properties.map((property) => (
                          <div key={property.key} className="rounded-lg bg-surface-container-low px-2 py-1.5">
                            <dt className="mono text-[9.5px] text-outline break-all">{property.key}</dt>
                            <dd className="text-[11px] text-on-surface break-words">
                              {propertyValueText(property.value)}
                            </dd>
                          </div>
                        ))}
                      </dl>
                    )}
                    {/* 属性值必须能定位到它来自哪篇笔记；路径由宿主读取结果给出。 */}
                    <p
                      className="mt-2 mono text-[9.5px] leading-relaxed text-outline break-all"
                      aria-label={`属性来源：${note?.relative_path ?? ""}`}
                      data-testid="vault-property-source"
                    >
                      来源：{note?.relative_path ?? ""}
                    </p>
                  </>
                )}
              </section>

              <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
                <h2 className="text-[12.5px] font-semibold text-on-surface">标签与别名</h2>
                <div
                  className="pt-2 flex flex-wrap gap-1"
                  aria-label={`标签：${parsed && parsed.tags.length > 0 ? parsed.tags.join("、") : "无"}；别名：${
                    parsed && parsed.aliases.length > 0 ? parsed.aliases.join("、") : "无"
                  }`}
                  data-testid="vault-tags"
                >
                  {parsed && parsed.tags.length > 0 ? (
                    parsed.tags.map((tag) => (
                      <span key={tag} className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] text-primary">
                        #{tag}
                      </span>
                    ))
                  ) : (
                    <span className="text-[11px] text-outline">没有标签</span>
                  )}
                </div>
                {parsed && parsed.aliases.length > 0 ? (
                  <p className="mt-2 text-[10.5px] text-on-surface-variant break-words">
                    别名：{parsed.aliases.join("、")}
                  </p>
                ) : null}
                {parsed?.frontmatter ? (
                  <p className="mt-2 mono text-[9.5px] text-outline">
                    Frontmatter 第 {parsed.frontmatter.start_line}–{parsed.frontmatter.end_line} 行 ·{" "}
                    {parsed.frontmatter.status}
                  </p>
                ) : null}
              </section>

              <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
                <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">解析警告</h2>
                  <span className="mono text-[10px] text-outline">{parsed?.warnings.length ?? 0}</span>
                </div>
                {!parsed || parsed.warnings.length === 0 ? (
                  <p className="pt-2 text-[11px] text-on-surface-variant">没有解析警告。</p>
                ) : (
                  <ul className="pt-2 space-y-1.5" data-testid="vault-warnings">
                    {parsed.warnings.map((warning, index) => (
                      <li key={`${warning.code}-${index}`} className="rounded-lg bg-amber-50 px-2 py-1.5 text-[10.5px] leading-relaxed text-amber-900">
                        <span className="mono text-[9px]">{warning.code}</span>
                        {warning.key ? <span className="mono text-[9px]"> · {warning.key}</span> : null}
                        <span className="block">{warning.message}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
                <h2 className="text-[12.5px] font-semibold text-on-surface">搜索与筛选</h2>
                <label className="mt-2 block text-[10px] text-outline" htmlFor="vault-search-keyword">
                  关键词（字面子串，不区分大小写）
                </label>
                <input
                  id="vault-search-keyword"
                  className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 text-[11px]"
                  value={keyword}
                  onChange={(event) => setKeyword(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void runSearch();
                  }}
                  placeholder="正文、路径、属性或标签"
                  data-testid="vault-search-keyword"
                />
                <label className="mt-2 block text-[10px] text-outline" htmlFor="vault-search-tags">
                  标签（逗号分隔，父标签含下级）
                </label>
                <input
                  id="vault-search-tags"
                  className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 text-[11px]"
                  value={tagFilter}
                  onChange={(event) => setTagFilter(event.target.value)}
                  placeholder="项目, 待办"
                  data-testid="vault-search-tags"
                />
                <div className="mt-2 grid grid-cols-2 gap-1.5">
                  <div>
                    <label className="block text-[10px] text-outline" htmlFor="vault-search-property-key">
                      属性键
                    </label>
                    <input
                      id="vault-search-property-key"
                      className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 mono text-[10.5px]"
                      value={propertyKey}
                      onChange={(event) => setPropertyKey(event.target.value)}
                      placeholder="status"
                      data-testid="vault-search-property-key"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] text-outline" htmlFor="vault-search-property-value">
                      属性值（可空）
                    </label>
                    <input
                      id="vault-search-property-value"
                      className="mt-1 w-full rounded-lg border border-outline-variant/50 bg-white px-2 py-1.5 text-[10.5px]"
                      value={propertyValue}
                      onChange={(event) => setPropertyValue(event.target.value)}
                      placeholder="进行中"
                      data-testid="vault-search-property-value"
                    />
                  </div>
                </div>
                <div className="mt-2 flex items-center gap-2">
                  <button
                    className="rounded-lg bg-primary px-3 py-1.5 text-[11px] text-on-primary disabled:opacity-45"
                    onClick={() => void runSearch()}
                    disabled={searchBusy}
                    data-testid="vault-search-run"
                  >
                    {searchBusy ? "搜索中…" : "搜索"}
                  </button>
                  <button className="text-[10.5px] text-on-surface-variant hover:text-primary" onClick={clearSearch}>
                    清空
                  </button>
                </div>
                {searchError ? (
                  <p className="mt-2 text-[10.5px] text-error" role="alert">
                    {searchError}
                  </p>
                ) : null}
                {searchResult ? (
                  <div className="mt-2" data-testid="vault-search-results">
                    <p
                      className="text-[10.5px] text-on-surface-variant"
                      role="status"
                      aria-label={`搜索统计：${searchResult.hits.length} 条命中；检查 ${searchResult.notes_examined}/${searchResult.notes_available} 篇`}
                      data-testid="vault-search-stats"
                    >
                      {searchResult.hits.length} 条命中 · 检查 {searchResult.notes_examined}/
                      {searchResult.notes_available} 篇 · 读取 {formatBytes(searchResult.bytes_read)} /
                      {formatBytes(searchResult.budget_bytes)}
                    </p>
                    {searchResult.budget_exhausted ? (
                      <p className="mt-1 rounded bg-amber-50 px-2 py-1 text-[10px] text-amber-900" data-testid="vault-search-budget">
                        读取预算已耗尽：还有笔记没有检查，结果不完整。
                      </p>
                    ) : null}
                    {searchResult.hits_truncated ? (
                      <p className="mt-1 rounded bg-amber-50 px-2 py-1 text-[10px] text-amber-900">
                        命中数已达到上限，只显示前 {searchResult.hits.length} 条。
                      </p>
                    ) : null}
                    <div className="mt-1.5 space-y-1.5 max-h-[30vh] overflow-y-auto">
                      {searchResult.hits.map((hit) => (
                        <button
                          key={hit.relative_path}
                          className="w-full rounded-lg bg-surface-container-low px-2 py-1.5 text-left hover:ring-1 hover:ring-primary/40"
                          onClick={() => void loadNote(hit.relative_path)}
                          aria-label={`搜索命中：${hit.relative_path}；区域：${
                            hit.match_areas.length > 0 ? hit.match_areas.join(" / ") : "筛选条件"
                          }`}
                          data-testid="vault-search-hit"
                          data-path={hit.relative_path}
                        >
                          <span className="block mono text-[10px] text-primary break-all">{hit.relative_path}</span>
                          <span className="block text-[9.5px] text-outline">
                            命中：{hit.match_areas.length > 0 ? hit.match_areas.join(" / ") : "筛选条件"}
                          </span>
                          {hit.snippet ? (
                            <span className="block mt-0.5 text-[10px] text-on-surface-variant break-words">
                              {hit.snippet_line ? `第 ${hit.snippet_line} 行：` : ""}
                              {hit.snippet}
                            </span>
                          ) : null}
                        </button>
                      ))}
                    </div>
                    {searchResult.skipped.length > 0 ? (
                      <p className="mt-1 text-[9.5px] text-outline">
                        跳过 {searchResult.skipped.length} 个文件（预算或读取失败）。
                      </p>
                    ) : null}
                  </div>
                ) : null}
              </section>

              {scan && (scan.errors.length > 0 || scan.skipped.length > 0) ? (
                <section className="rounded-2xl bg-white/80 border border-white/90 p-3">
                  <h2 className="text-[12.5px] font-semibold text-on-surface">扫描明细</h2>
                  {scan.errors.length > 0 ? (
                    <ul className="mt-1.5 space-y-1">
                      {scan.errors.slice(0, 10).map((item) => (
                        <li key={item.relative_path} className="text-[10px] text-error break-all">
                          {item.relative_path}：{item.message}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                  {scan.skipped.length > 0 ? (
                    <ul className="mt-1.5 space-y-1">
                      {scan.skipped.slice(0, 10).map((item) => (
                        <li key={`${item.relative_path}-${item.reason}`} className="text-[10px] text-outline break-all">
                          {item.relative_path}：{item.reason}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                  {scan.stats.details_truncated ? (
                    <p className="mt-1 text-[9.5px] text-outline">明细已达到上限，仅显示部分条目。</p>
                  ) : null}
                </section>
              ) : null}
            </aside>
          </div>
        </div>
        )
      ) : (
        <section className="flex-1 min-h-0 flex items-center justify-center">
          <p className="text-[11.5px] text-outline">正在读取知识库设置…</p>
        </section>
      )}

      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.title ?? ""}
        description={confirm?.description ?? ""}
        confirmLabel={confirm?.confirmLabel ?? "继续"}
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          const action = confirm?.action;
          setConfirm(null);
          action?.();
        }}
        testId="vault-confirm"
      />
    </main>
  );
}
