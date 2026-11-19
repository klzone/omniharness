import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type JSX, type MouseEvent as ReactMouseEvent } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { host, shell } from "../api";
import {
  TERMINAL_STATES,
  type ReadingAnnotation,
  type ReadingAnnotationDraft,
  type ReadingBook,
  type ReadingBookMeta,
  type ReadingLibrary,
  type ReadingStats,
  type ReadingSavedNote,
  type RssArticleMeta,
  type RssPresetSource,
  type RssSubscription,
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
import { ReadingDashboard } from "./ReadingDashboard";
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

const PANEL_AUTO_CLOSE_MS = 90_000;
const RSS_AUTO_REFRESH_MS = 30 * 60 * 1000;
const RSS_SHELF_RETENTION_MS = 30 * 24 * 60 * 60 * 1000;

function kindLabel(kind: string): string {
  return kind === "highlight" ? "划线" : kind === "bookmark" ? "标记" : "想法";
}

function chapterTitle(book: ReadingBook, index: number): string {
  return book.chapters[index]?.title?.trim() || `第 ${index + 1} 章`;
}

function sourceFormatLabel(format: string): string {
  return format === "rss" ? "订阅文章" : format.toUpperCase();
}

function articleDateLabel(meta?: RssArticleMeta): string {
  if (!meta) return "";
  const stamp = meta.published_ts > 0 ? meta.published_ts * 1000 : Date.parse(meta.fetched_at);
  if (!Number.isFinite(stamp)) return "";
  const date = new Date(stamp);
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

/** 旧抓取把正文图片压成了“[图片：…]”文本；命中且无批注时交给后端重新抓取回填。 */
function hasLegacyImagePlaceholder(book: ReadingBook): boolean {
  return book.chapters.some((chapter) =>
    chapter.paragraphs.some((paragraph) => paragraph.includes("[图片：")),
  );
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
      "模型端点域名解析失败（getaddrinfo/ENOTFOUND）。健脑室没有改动模型配置，" +
      "请检查本机网络、代理或设置中的服务商地址后再试。\n原始错误：" +
      raw
    );
  }
  if (/econnrefused|etimedout|econnreset/i.test(raw)) {
    return "模型端点连接失败（网络超时或被拒绝）。请检查网络/代理后重试。\n原始错误：" + raw;
  }
  return raw;
}

// ---------------------------------------------------------------------------
// 富媒体（OD-READ-003）：段落里保留 `![img](...)` / `[audio](...)` / `[video](...)`
// 行内标记；划线偏移仍按含标记的原文计算。媒体字节按需取，组件内缓存。
// ---------------------------------------------------------------------------

type MediaKind = "image" | "audio" | "video";

type ParagraphToken =
  | { kind: "text"; value: string }
  | { kind: MediaKind; value: string; raw: string };

const MEDIA_MARKER_RE = /!\[img\]\(([^)\n]+)\)|\[(audio|video)\]\(([^)\n]+)\)/g;

function splitMediaMarkers(text: string): ParagraphToken[] {
  const tokens: ParagraphToken[] = [];
  let last = 0;
  for (const match of text.matchAll(MEDIA_MARKER_RE)) {
    const index = match.index ?? 0;
    if (index > last) tokens.push({ kind: "text", value: text.slice(last, index) });
    if (match[1]) tokens.push({ kind: "image", value: match[1], raw: match[0] });
    else
      tokens.push({
        kind: match[2] === "audio" ? "audio" : "video",
        value: match[3] ?? "",
        raw: match[0],
      });
    last = index + match[0].length;
  }
  if (last < text.length) tokens.push({ kind: "text", value: text.slice(last) });
  return tokens;
}

function decodeMediaRef(src: string): string {
  try {
    return decodeURIComponent(src);
  } catch {
    return src;
  }
}

function isDirectMediaSrc(src: string): boolean {
  return /^data:/i.test(src);
}

function isRemoteMediaSrc(src: string): boolean {
  return /^https?:/i.test(src);
}

const mediaSrcCache = new Map<string, string>();

function MediaBlock({
  bookId,
  kind,
  src,
  onOpenImage,
}: {
  bookId: string;
  kind: MediaKind;
  src: string;
  onOpenImage: (src: string) => void;
}) {
  const cacheKey = `${bookId}:${src}`;
  const [resolved, setResolved] = useState<string | null>(() => mediaSrcCache.get(cacheKey) ?? null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (resolved) return;
    if (isDirectMediaSrc(src)) {
      mediaSrcCache.set(cacheKey, src);
      setResolved(src);
      return;
    }
    if (isRemoteMediaSrc(src)) {
      const remoteKey = `remote:${src}`;
      const cached = mediaSrcCache.get(remoteKey);
      if (cached) {
        setResolved(cached);
        return;
      }
      let alive = true;
      setLoading(true);
      setError(null);
      shell
        .readingRemoteMedia(src)
        .then((payload) => {
          if (!alive) return;
          const dataUrl = `data:${payload.mime};base64,${payload.data_base64}`;
          mediaSrcCache.set(remoteKey, dataUrl);
          setResolved(dataUrl);
        })
        .catch((e) => {
          if (alive) setError(String(e));
        })
        .finally(() => {
          if (alive) setLoading(false);
        });
      return () => {
        alive = false;
      };
    }
    if (!src.startsWith("odmedia:")) {
      setError("不支持的媒体地址。");
      return;
    }
    let alive = true;
    setLoading(true);
    setError(null);
    const ref = decodeMediaRef(src.slice("odmedia:".length));
    shell
      .readingMediaExtract(bookId, ref)
      .then((payload) => {
        if (!alive) return;
        const dataUrl = `data:${payload.mime};base64,${payload.data_base64}`;
        mediaSrcCache.set(cacheKey, dataUrl);
        setResolved(dataUrl);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
    // src/bookId 变化才重新取；resolved 只用于跳过已缓存结果。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bookId, src]);

  if (error) {
    return (
      <span className="reading-media-error" data-testid="reading-media-error">
        {kind === "image" ? "图片读取失败：" : "媒体读取失败："}
        {error}
      </span>
    );
  }
  if (!resolved) {
    return <span className="reading-media-loading">{loading ? "正在读取媒体…" : "媒体未加载"}</span>;
  }
  if (kind === "image") {
    return (
      <figure className="reading-media-figure reading-media-image">
        <button type="button" className="reading-media-image-button" onClick={() => onOpenImage(resolved)}>
          <img src={resolved} alt="正文图片" loading="lazy" />
        </button>
      </figure>
    );
  }
  return (
    <figure className="reading-media-figure">
      {kind === "audio" ? (
        <audio className="reading-media-audio" controls preload="none" src={resolved} />
      ) : (
        <video className="reading-media-video" controls preload="metadata" src={resolved} />
      )}
    </figure>
  );
}

const INLINE_MARKDOWN_RE = /\[([^\]\n]+)\]\((https?:\/\/[^)\n]+)\)|\*\*([^*\n]+)\*\*|`([^`\n]+)`/g;

/**
 * 段落内行内 Markdown 的偏移保真渲染。
 *
 * ReactMarkdown 会把 `**`/反引号/链接目标从 DOM 文本里移除，导致选区前缀长度
 * 与原文不一致、划线偏移错位；这里把格式符号放进视觉隐藏的锚点 span，
 * 让 DOM 文本严格等于原段落，同时仍显示粗体/行内代码/链接。笔记与 AI 结果
 * 继续走 MarkdownText 的 react-markdown 管线。
 */
function InlineMarkdown({ text }: { text: string }) {
  if (!text.includes("*") && !text.includes("`") && !text.includes("[")) return <>{text}</>;
  const nodes: Array<string | JSX.Element> = [];
  let last = 0;
  for (const match of text.matchAll(INLINE_MARKDOWN_RE)) {
    const index = match.index ?? 0;
    if (index > last) nodes.push(text.slice(last, index));
    if (match[1] && match[2]) {
      nodes.push(
        <Fragment key={`link-${index}`}>
          <span className="reading-syntax-anchor">[</span>
          <a href={match[2]} target="_blank" rel="noreferrer noopener">{match[1]}</a>
          <span className="reading-syntax-anchor">]({match[2]})</span>
        </Fragment>,
      );
    } else if (match[3]) {
      nodes.push(
        <Fragment key={`strong-${index}`}>
          <span className="reading-syntax-anchor">**</span>
          <strong>{match[3]}</strong>
          <span className="reading-syntax-anchor">**</span>
        </Fragment>,
      );
    } else if (match[4]) {
      nodes.push(
        <Fragment key={`code-${index}`}>
          <span className="reading-syntax-anchor">`</span>
          <code className="reading-inline-code">{match[4]}</code>
          <span className="reading-syntax-anchor">`</span>
        </Fragment>,
      );
    }
    last = index + match[0].length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return <>{nodes}</>;
}

function RichParagraphText({
  bookId,
  text,
  onOpenImage,
}: {
  bookId: string;
  text: string;
  onOpenImage: (src: string) => void;
}) {
  const tokens = splitMediaMarkers(text);
  if (tokens.length === 1 && tokens[0]?.kind === "text") {
    return <InlineMarkdown text={text} />;
  }
  return (
    <>
      {tokens.map((token, index) =>
        token.kind === "text" ? (
          <InlineMarkdown key={index} text={token.value} />
        ) : (
          <Fragment key={`${bookId}-${index}`}>
            {/* 标记原文保留在 DOM 文本里（视觉隐藏），Range.toString 才能保住划线偏移。 */}
            <span className="reading-media-anchor" aria-hidden="true">
              {token.raw}
            </span>
            <MediaBlock
              bookId={bookId}
              kind={token.kind}
              src={token.value}
              onOpenImage={onOpenImage}
            />
          </Fragment>
        ),
      )}
    </>
  );
}

const coverSrcCache = new Map<string, string>();

function BookCover({
  bookId,
  cover,
  title,
  className,
}: {
  bookId: string;
  cover?: string | null;
  title: string;
  className?: string;
}) {
  // 旧书目 meta.cover 为空：仍向宿主请求一次，由后端按需从源文件回提取。
  const [src, setSrc] = useState<string | null>(() => coverSrcCache.get(bookId) ?? null);
  useEffect(() => {
    if (src) return;
    if (cover?.startsWith("data:")) {
      coverSrcCache.set(bookId, cover);
      setSrc(cover);
      return;
    }
    let alive = true;
    shell
      .readingCoverImage(bookId)
      .then((payload) => {
        if (!alive) return;
        const dataUrl = `data:${payload.mime};base64,${payload.data_base64}`;
        coverSrcCache.set(bookId, dataUrl);
        setSrc(dataUrl);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
    // 封面按书号取一次；失败时保留暖底首字占位。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bookId, cover]);
  return (
    <span className={`reading-cover ${className ?? ""}`}>
      {src ? (
        <img src={src} alt={`${title} 封面`} loading="lazy" />
      ) : (
        <span className="reading-cover-fallback" aria-hidden="true">
          {(Array.from(title.trim())[0] ?? "书").toUpperCase()}
        </span>
      )}
    </span>
  );
}

export function ReadingPage() {
  const [library, setLibrary] = useState<ReadingLibrary | null>(null);
  const [readingStats, setReadingStats] = useState<ReadingStats | null>(null);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [importBusy, setImportBusy] = useState(false);
  const [bookId, setBookId] = useState<string | null>(null);
  const [book, setBook] = useState<ReadingBook | null>(null);
  const [bookBusy, setBookBusy] = useState(false);
  const [bookError, setBookError] = useState<string | null>(null);
  const [sideTab, setSideTab] = useState<SideTab>("notes");
  const [panelOpen, setPanelOpen] = useState(false);
  const [panelWidth, setPanelWidth] = useState(320);
  const panelResize = useRef<{ x: number; width: number } | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const panelCloseTimer = useRef<number | null>(null);
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
  const [subscriptions, setSubscriptions] = useState<RssSubscription[]>([]);
  const [rssArticles, setRssArticles] = useState<RssArticleMeta[]>([]);
  const [subscriptionOpen, setSubscriptionOpen] = useState(false);
  const [subscribeUrl, setSubscribeUrl] = useState("");
  const [subscribeName, setSubscribeName] = useState("");
  const [rssBusy, setRssBusy] = useState<"add" | "refresh" | null>(null);
  const [rssMessage, setRssMessage] = useState<string | null>(null);
  const [rssError, setRssError] = useState<string | null>(null);
  const [pendingSubscriptionDelete, setPendingSubscriptionDelete] = useState<RssSubscription | null>(null);
  const [shelfManagerOpen, setShelfManagerOpen] = useState(false);
  const [pendingBookDelete, setPendingBookDelete] = useState<ReadingBookMeta | null>(null);
  const [shelfTab, setShelfTab] = useState<"books" | "rss">("books");
  const [shelfQuery, setShelfQuery] = useState("");
  const [collapsedSources, setCollapsedSources] = useState<Record<string, boolean>>({});
  const [renamingBook, setRenamingBook] = useState<ReadingBookMeta | null>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [renameBusy, setRenameBusy] = useState(false);
  const [favoriteBusy, setFavoriteBusy] = useState(false);
  const [lightbox, setLightbox] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const shelfSearchRef = useRef<HTMLInputElement | null>(null);
  const restoredBook = useRef<string | null>(null);
  const rssRefreshAttempted = useRef<Set<string>>(new Set());
  const subscriptionsRef = useRef<RssSubscription[]>([]);
  const lastAutoRefreshRef = useRef(0);
  const autoRefreshRunning = useRef(false);
  const randomSourceExpanded = useRef(false);
  const loadSeq = useRef(0);
  const lastProgress = useRef(0);
  const progressTimer = useRef<number | null>(null);
  const timeBookId = useRef<string | null>(null);
  const pendingSeconds = useRef(0);
  const toastTimer = useRef<number | null>(null);
  const aiSeq = useRef(0);
  const readSelectionRef = useRef<() => void>(() => {});

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3600);
  }, []);

  const flushReadingTime = useCallback((id: string | null) => {
    const seconds = pendingSeconds.current;
    pendingSeconds.current = 0;
    if (!id || seconds <= 0) return;
    void shell
      .readingTimeAdd(id, Math.min(seconds, 3600))
      .then(() => shell.readingStats())
      .then((stats) => setReadingStats(stats))
      .catch(() => undefined);
  }, []);

  // 真实阅读时长：仅窗口可见且聚焦时每 15 秒累计，每 60 秒落盘一次；
  // 切书、关闭、失焦或页面隐藏时立即 flush，后台时间不计入。
  useEffect(() => {
    const id = book?.book_id ?? null;
    if (!id) return undefined;
    timeBookId.current = id;
    pendingSeconds.current = 0;
    let activeSince =
      document.visibilityState === "visible" && document.hasFocus() ? performance.now() : null;
    const tick = window.setInterval(() => {
      if (activeSince === null) return;
      if (document.visibilityState !== "visible" || !document.hasFocus()) {
        flushReadingTime(timeBookId.current);
        activeSince = null;
        return;
      }
      if (performance.now() - activeSince >= 15_000) {
        pendingSeconds.current += 15;
        activeSince = performance.now();
      }
    }, 15_000);
    const flush = window.setInterval(() => {
      flushReadingTime(timeBookId.current);
    }, 60_000);
    const onVisibility = () => {
      if (document.visibilityState !== "visible") {
        flushReadingTime(timeBookId.current);
        activeSince = null;
      } else if (document.hasFocus()) {
        activeSince = performance.now();
      }
    };
    const onBlur = () => {
      flushReadingTime(timeBookId.current);
      activeSince = null;
    };
    const onFocus = () => {
      if (document.visibilityState === "visible") activeSince = performance.now();
    };
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(tick);
      window.clearInterval(flush);
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      flushReadingTime(id);
    };
  }, [book?.book_id, flushReadingTime]);

  const resetPanelCloseTimer = useCallback(() => {
    if (panelCloseTimer.current !== null) window.clearTimeout(panelCloseTimer.current);
    panelCloseTimer.current = window.setTimeout(() => {
      panelCloseTimer.current = null;
      setPanelOpen(false);
    }, PANEL_AUTO_CLOSE_MS);
  }, []);

  // 只统计右侧栏内的操作；正文滚动不续命，避免一直滚动时面板永不收起。
  useEffect(() => {
    const clear = () => {
      if (panelCloseTimer.current !== null) {
        window.clearTimeout(panelCloseTimer.current);
        panelCloseTimer.current = null;
      }
    };
    if (!panelOpen || !book) {
      clear();
      return clear;
    }
    const element = panelRef.current;
    if (!element) {
      clear();
      return clear;
    }
    const activity = () => resetPanelCloseTimer();
    const events = ["click", "keydown", "scroll", "pointerdown"] as const;
    for (const event of events) element.addEventListener(event, activity, true);
    resetPanelCloseTimer();
    return () => {
      for (const event of events) element.removeEventListener(event, activity, true);
      clear();
    };
  }, [book?.book_id, panelOpen, resetPanelCloseTimer]);

  const refreshLibrary = useCallback(async () => {
    try {
      const loaded = await shell.readingLibrary();
      setLibrary(loaded);
      setLibraryError(null);
      const stats = await shell.readingStats();
      setReadingStats(stats);
    } catch (e) {
      setLibraryError(String(e));
    }
  }, []);

  const refreshRss = useCallback(async () => {
    try {
      const [loadedSubscriptions, loadedArticles] = await Promise.all([
        shell.rssSubscriptionsList(),
        shell.rssArticlesList(),
      ]);
      setSubscriptions(loadedSubscriptions);
      setRssArticles(loadedArticles);
      setRssError(null);
    } catch (e) {
      setRssError(String(e));
    }
  }, []);

  useEffect(() => {
    subscriptionsRef.current = subscriptions;
  }, [subscriptions]);

  // 后台刷新：不占 rssBusy，不阻塞阅读；成功后只刷新书架与文章列表。
  const autoRefreshRss = useCallback(async () => {
    if (autoRefreshRunning.current) return;
    autoRefreshRunning.current = true;
    lastAutoRefreshRef.current = Date.now();
    try {
      const report = await shell.rssFetch();
      await Promise.all([refreshLibrary(), refreshRss()]);
      if (report.added > 0) showToast(`后台刷新新增 ${report.added} 篇文章。`);
    } catch (e) {
      setRssError(String(e));
    } finally {
      autoRefreshRunning.current = false;
    }
  }, [refreshLibrary, refreshRss, showToast]);

  const maybeAutoRefreshRss = useCallback(() => {
    const now = Date.now();
    // 本次会话已经触发过就等下一个 30 分钟，避免失败后反复重试。
    if (now - lastAutoRefreshRef.current < RSS_AUTO_REFRESH_MS) return;
    const list = subscriptionsRef.current;
    if (list.length === 0) return;
    const stamps = list
      .map((item) => Date.parse(item.last_fetched_at))
      .filter((stamp) => Number.isFinite(stamp));
    const latest = stamps.length ? Math.max(...stamps) : 0;
    if (latest && now - latest < RSS_AUTO_REFRESH_MS) return;
    void autoRefreshRss();
  }, [autoRefreshRss]);

  useEffect(() => {
    maybeAutoRefreshRss();
  }, [subscriptions, maybeAutoRefreshRss]);

  useEffect(() => {
    const onFocus = () => maybeAutoRefreshRss();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [maybeAutoRefreshRss]);

  useEffect(() => {
    void refreshLibrary();
    void refreshRss();
  }, [refreshLibrary, refreshRss]);

  // 书架管理弹窗：Esc 关闭；确认删除对话框盖在上面时交给 ConfirmDialog 自己处理。
  useEffect(() => {
    if (!shelfManagerOpen) return undefined;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || pendingBookDelete) return;
      event.preventDefault();
      setShelfManagerOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shelfManagerOpen, pendingBookDelete]);

  useEffect(() => {
    if (!renamingBook) return undefined;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || renameBusy) return;
      event.preventDefault();
      setRenamingBook(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [renamingBook, renameBusy]);

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
      // 切书时右侧栏回到折叠态，旧书的批注/输入不跨书保留。
      setPanelOpen(false);
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
        let loaded = await shell.readingBook(id);
        if (seq !== loadSeq.current) return;
        // 先显示旧正文，避免旧 EPUB 的媒体回填把「正在打开书籍…」拖上十几秒。
        setBook(loaded);
        lastProgress.current = loaded.progress.percent;
        if (
          (loaded.source_format === "epub" || loaded.source_format === "mobi") &&
          (loaded.media_version ?? 0) < 1
        ) {
          showToast("正在补齐正文媒体…");
          void (async () => {
            try {
              const refreshed = await shell.readingRefreshContent(id);
              if (seq !== loadSeq.current) return;
              const scrollTop = scrollRef.current?.scrollTop ?? 0;
              const currentPercent = lastProgress.current;
              setBook({ ...refreshed.book, progress: { ...refreshed.book.progress, percent: currentPercent } });
              requestAnimationFrame(() => {
                if (seq !== loadSeq.current || !scrollRef.current) return;
                scrollRef.current.scrollTop = scrollTop;
              });
              if (refreshed.unresolved > 0) {
                showToast(`已补齐正文媒体；${refreshed.unresolved} 条批注未能按原文重新定位。`);
              }
            } catch (e) {
              if (seq !== loadSeq.current) return;
              showToast(`正文媒体回填失败：${String(e)}`);
            }
          })();
        }
        if (loaded.source_format === "rss") {
          void shell.rssArticleMarkRead(id).then((meta) => {
            setRssArticles((prev) => {
              if (prev.some((item) => item.book_id === meta.book_id)) {
                return prev.map((item) => (item.book_id === meta.book_id ? meta : item));
              }
              return [...prev, meta];
            });
          }).catch(() => undefined);
        }
        if (
          loaded.source_format === "rss" &&
          loaded.annotations.length === 0 &&
          hasLegacyImagePlaceholder(loaded) &&
          !rssRefreshAttempted.current.has(id)
        ) {
          rssRefreshAttempted.current.add(id);
          showToast("正在补齐文章图片…");
          // 初次 readingBook 已完成并显示正文；回填在后台跑，不占 busy。
          void (async () => {
            try {
              const result = await shell.rssRefreshArticle(id);
              if (seq !== loadSeq.current) return;
              if (result.updated) {
                loaded = await shell.readingBook(id);
                if (seq !== loadSeq.current) return;
                const scrollTop = scrollRef.current?.scrollTop ?? 0;
                const currentPercent = lastProgress.current;
                loaded = {
                  ...loaded,
                  progress: { ...loaded.progress, percent: currentPercent },
                };
                setBook(loaded);
                lastProgress.current = currentPercent;
                requestAnimationFrame(() => {
                  if (seq !== loadSeq.current || !scrollRef.current) return;
                  scrollRef.current.scrollTop = scrollTop;
                });
                void refreshLibrary();
              } else if (result.reason.trim()) {
                showToast(`旧图片回填未完成：${result.reason}`);
              }
            } catch (e) {
              if (seq !== loadSeq.current) return;
              showToast(`旧图片回填失败：${String(e)}`);
            }
          })();
        }
      } catch (e) {
        if (seq !== loadSeq.current) return;
        setBookError(String(e));
      } finally {
        if (seq === loadSeq.current) setBookBusy(false);
      }
    },
    [book, refreshLibrary, showToast],
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
        filters: [{ name: "阅读文件", extensions: ["md", "markdown", "txt", "pdf", "docx", "epub", "mobi"] }],
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

  const addRssSubscription = async () => {
    const url = subscribeUrl.trim();
    if (!url) {
      setRssError("请填写 RSS / Atom 地址。");
      return;
    }
    setRssBusy("add");
    setRssError(null);
    setRssMessage(null);
    try {
      const added = await shell.rssSubscriptionAdd(url, subscribeName.trim() || undefined);
      await refreshRss();
      setSubscribeUrl("");
      setSubscribeName("");
      setRssMessage(`已添加「${added.name}」；点「刷新全部」开始拉取。`);
    } catch (e) {
      setRssError(String(e));
    } finally {
      setRssBusy(null);
    }
  };

  const removeRssSubscription = async () => {
    if (!pendingSubscriptionDelete) return;
    const id = pendingSubscriptionDelete.id;
    try {
      await shell.rssSubscriptionRemove(id);
      await refreshRss();
      setRssMessage("订阅已删除；已拉取的文章仍留在书架。");
    } catch (e) {
      setRssError(String(e));
    } finally {
      setPendingSubscriptionDelete(null);
    }
  };

  const deleteBook = async () => {
    if (!pendingBookDelete) return;
    const target = pendingBookDelete;
    try {
      await shell.readingDeleteBook(target.book_id);
      await refreshLibrary();
      if (bookId === target.book_id) {
        // 取消尚未返回的打开请求，并关闭阅读器；书架随 book=null 自动隐藏。
        loadSeq.current += 1;
        if (progressTimer.current !== null) {
          window.clearTimeout(progressTimer.current);
          progressTimer.current = null;
        }
        // 书已被删除：丢弃尚未落盘的计时，避免 cleanup 又创建 progress 文件。
        pendingSeconds.current = 0;
        setBookId(null);
        setBook(null);
        setBookBusy(false);
        setBookError(null);
        setPanelOpen(false);
        setSelection(null);
        setSelectionPos(null);
      }
      showToast(`已删除《${target.title}》；导出文件保留在阅读目录。`);
    } catch (e) {
      showToast(String(e));
    } finally {
      setPendingBookDelete(null);
    }
  };

  const toggleArticleFavorite = useCallback(async () => {
    if (!book || book.source_format !== "rss" || favoriteBusy) return;
    const current = rssArticles.find((item) => item.book_id === book.book_id);
    const next = !(current?.favorite ?? false);
    setFavoriteBusy(true);
    try {
      const meta = await shell.rssArticleSetFavorite(book.book_id, next);
      setRssArticles((prev) => {
        if (prev.some((item) => item.book_id === meta.book_id)) {
          return prev.map((item) => (item.book_id === meta.book_id ? meta : item));
        }
        return [...prev, meta];
      });
      showToast(meta.favorite ? "已收藏" : "已取消收藏");
    } catch (e) {
      showToast(String(e));
    } finally {
      setFavoriteBusy(false);
    }
  }, [book, favoriteBusy, rssArticles, showToast]);

  const openRenameDialog = (meta: ReadingBookMeta) => {
    setRenamingBook(meta);
    setRenameTitle(meta.title);
  };

  const saveRename = async () => {
    if (!renamingBook || renameBusy) return;
    const title = renameTitle.trim();
    if (!title) {
      showToast("书名不能为空。");
      return;
    }
    setRenameBusy(true);
    try {
      const meta = await shell.readingRenameBook(renamingBook.book_id, title);
      await refreshLibrary();
      setBook((prev) => (prev && prev.book_id === meta.book_id ? { ...prev, title: meta.title } : prev));
      setRenamingBook(null);
      showToast(`已改名为《${meta.title}》。`);
    } catch (e) {
      showToast(String(e));
    } finally {
      setRenameBusy(false);
    }
  };

  const subscribePreset = async (source: RssPresetSource) => {
    try {
      const added = await shell.rssSubscriptionAdd(source.url, source.name);
      await refreshRss();
      showToast(`已订阅「${added.name}」。`);
    } catch (e) {
      showToast(String(e));
      throw e;
    }
  };

  const runRssFetch = async (subscriptionId?: string) => {
    if (rssBusy === "refresh") return;
    setRssBusy("refresh");
    setRssError(null);
    setRssMessage(subscriptionId ? "正在拉取这个订阅…" : `正在拉取 ${subscriptions.length} 个订阅…`);
    try {
      const report = await shell.rssFetch(subscriptionId);
      await Promise.all([refreshLibrary(), refreshRss()]);
      let summary = `已拉取 ${report.fetched} 个订阅，新增 ${report.added} 篇`;
      if (report.skipped) summary += `，跳过重复 ${report.skipped} 篇`;
      if (report.failed) summary += `，失败 ${report.failed} 个`;
      setRssMessage(summary);
      if (report.errors.length) {
        setRssError(report.errors.map((item) => `${item.name}：${item.message}`).join("\n"));
      }
      showToast(summary);
    } catch (e) {
      setRssError(String(e));
      showToast(String(e));
    } finally {
      setRssBusy(null);
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
    // 拖选/双击/键盘选择在部分 WebView 里 mouseup 之后选区才落定；
    // selectionchange 去抖作为第二通道，两条路都收敛到 readSelectionRef。
    let selectionTimer: number | null = null;
    const onSelectionChange = () => {
      if (selectionTimer !== null) window.clearTimeout(selectionTimer);
      selectionTimer = window.setTimeout(() => {
        selectionTimer = null;
        readSelectionRef.current();
      }, 150);
    };
    document.addEventListener("mouseup", handler);
    document.addEventListener("selectionchange", onSelectionChange);
    return () => {
      document.removeEventListener("mouseup", handler);
      document.removeEventListener("selectionchange", onSelectionChange);
      if (selectionTimer !== null) window.clearTimeout(selectionTimer);
    };
  }, [book?.book_id]);

  const clearSelection = () => {
    window.getSelection()?.removeAllRanges();
    setSelection(null);
    setSelectionPos(null);
  };

  const beginPanelResize = (event: ReactMouseEvent) => {
    event.preventDefault();
    panelResize.current = { x: event.clientX, width: panelWidth };
    const move = (ev: MouseEvent) => {
      const state = panelResize.current;
      if (!state) return;
      const next = state.width + (state.x - ev.clientX);
      const max = Math.min(640, Math.max(300, window.innerWidth - 80));
      setPanelWidth(Math.max(300, Math.min(max, next)));
    };
    const up = () => {
      panelResize.current = null;
      document.removeEventListener("mousemove", move);
      document.removeEventListener("mouseup", up);
      document.body.style.userSelect = "";
    };
    document.body.style.userSelect = "none";
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
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

  const expandShelf = (tab?: "books" | "rss", focusSearch = false) => {
    if (tab) setShelfTab(tab);
    setPanelOpen(false);
    if (focusSearch) {
      window.setTimeout(() => shelfSearchRef.current?.focus(), 240);
    }
  };

  const rssMetaById = useMemo(
    () => new Map(rssArticles.map((item) => [item.book_id, item])),
    [rssArticles],
  );
  const articleTimestamp = (item: ReadingBookMeta): number => {
    const meta = rssMetaById.get(item.book_id);
    if (meta?.published_ts && meta.published_ts > 0) return meta.published_ts * 1000;
    const fetched = meta ? Date.parse(meta.fetched_at) : NaN;
    return Number.isFinite(fetched) ? fetched : 0;
  };
  const shelfBooks = (library?.books ?? []).filter((item) => item.source_format !== "rss");
  const shelfArticles = (library?.books ?? [])
    .filter((item) => item.source_format === "rss")
    .filter((item) => {
      // 收藏永远显示；未收藏的文章只保留最近 30 天。时间缺失时按保留处理。
      if (rssMetaById.get(item.book_id)?.favorite) return true;
      const stamp = articleTimestamp(item);
      if (!stamp) return true;
      return Date.now() - stamp <= RSS_SHELF_RETENTION_MS;
    })
    .slice()
    .sort((a, b) => {
      const left = articleTimestamp(a) || Date.parse(a.updated_at) || 0;
      const right = articleTimestamp(b) || Date.parse(b.updated_at) || 0;
      return right - left || b.updated_at.localeCompare(a.updated_at);
    });
  const shelfNeedle = shelfQuery.trim().toLocaleLowerCase();
  const matchingBooks = shelfNeedle
    ? shelfBooks.filter((item) =>
        `${item.title} ${item.source_name} ${item.author}`.toLocaleLowerCase().includes(shelfNeedle),
      )
    : shelfBooks;
  const matchingArticles = shelfNeedle
    ? shelfArticles.filter((item) =>
        `${item.title} ${item.source_name} ${item.author}`.toLocaleLowerCase().includes(shelfNeedle),
      )
    : shelfArticles;
  const rssGroups = useMemo(() => {
    const groups = new Map<string, ReadingBookMeta[]>();
    for (const item of matchingArticles) {
      const name = item.source_name?.trim() || rssMetaById.get(item.book_id)?.author || "订阅文章";
      const list = groups.get(name) ?? [];
      list.push(item);
      groups.set(name, list);
    }
    return Array.from(groups.entries()).sort((a, b) => {
      const left = a[1][0] ? articleTimestamp(a[1][0]) : 0;
      const right = b[1][0] ? articleTimestamp(b[1][0]) : 0;
      return right - left || a[0].localeCompare(b[0]);
    });
  }, [matchingArticles, rssMetaById]);

  useEffect(() => {
    if (randomSourceExpanded.current || rssGroups.length === 0) return;
    randomSourceExpanded.current = true;
    const pick = rssGroups[Math.floor(Math.random() * rssGroups.length)];
    if (!pick) return;
    const sourceName = pick[0];
    setCollapsedSources((prev) => ({ ...prev, [sourceName]: false }));
  }, [rssGroups]);

  const currentArticleFavorite =
    book && book.source_format === "rss" ? Boolean(rssMetaById.get(book.book_id)?.favorite) : false;

  return (
    <div className="reading-page flex h-full min-h-0 w-full flex-col md:pl-20" data-testid="reading-page">
      <div className="flex min-h-0 flex-1 gap-2 p-2 xl:gap-3 xl:p-3">
        {book ? (
        <aside
          className={
            "reading-shelf hidden shrink-0 flex-col lg:flex " +
            (panelOpen ? "reading-shelf-is-rail w-14" : "w-[288px]")
          }
        >
          <div className="reading-shelf-rail" aria-label="书架快捷入口">
            <button
              type="button"
              onClick={() => expandShelf("books")}
              className={"reading-shelf-rail-button" + (shelfTab === "books" ? " is-active" : "")}
              aria-label="展开书架并切到本地书籍"
              aria-pressed={shelfTab === "books"}
              title="本地书籍"
            >
              <Icon name="book" className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => expandShelf("rss")}
              className={"reading-shelf-rail-button" + (shelfTab === "rss" ? " is-active" : "")}
              aria-label="展开书架并切到订阅文章"
              aria-pressed={shelfTab === "rss"}
              title="订阅文章"
            >
              <Icon name="radar" className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => expandShelf(undefined, true)}
              className="reading-shelf-rail-button"
              aria-label="展开书架并搜索"
              title="搜索书架与文章"
            >
              <Icon name="search" className="h-3.5 w-3.5" />
            </button>
          </div>
          <div className="reading-shelf-full">
          <div className="flex items-center justify-between gap-2 border-b border-outline-variant/40 px-3 py-2.5">
            <div className="min-w-0">
              <p className="text-[16px] font-bold tracking-[0.08em] text-on-surface">健脑室</p>
              <p className="truncate text-[12.5px] text-outline">
                {library ? `${shelfBooks.length} 本 · ${shelfArticles.length} 篇` : "读取中"}
              </p>
            </div>
          </div>
          <div className="reading-shelf-search" data-testid="reading-shelf-search">
            <Icon name="search" className="h-3 w-3 shrink-0" />
            <input
              ref={shelfSearchRef}
              value={shelfQuery}
              onChange={(event) => setShelfQuery(event.target.value)}
              placeholder="搜索书架与文章"
              aria-label="搜索书架与文章"
            />
            {shelfQuery ? (
              <button
                type="button"
                onClick={() => setShelfQuery("")}
                className="text-outline hover:text-on-surface"
                aria-label="清除搜索"
              >
                ×
              </button>
            ) : null}
          </div>
          <div className="reading-shelf-tabs" data-testid="reading-shelf-tabs">
            <button
              type="button"
              onClick={() => setShelfTab("books")}
              className={shelfTab === "books" ? "is-active" : ""}
              data-testid="reading-shelf-tab-books"
            >
              本地书籍 {shelfBooks.length}
            </button>
            <button
              type="button"
              onClick={() => setShelfTab("rss")}
              className={shelfTab === "rss" ? "is-active" : ""}
              data-testid="reading-shelf-tab-rss"
            >
              订阅文章 {shelfArticles.length}
            </button>
          </div>
          {shelfTab === "books" ? (
            <div className="flex items-center justify-end gap-1 border-b border-outline-variant/30 px-2 py-1.5">
              <button
                type="button"
                onClick={() => void importBook()}
                disabled={importBusy}
                className="inline-flex items-center gap-1 rounded-lg border border-outline-variant/60 px-2 py-1 text-[12.5px] text-on-surface-variant hover:border-primary hover:text-primary disabled:opacity-50"
              >
                <Icon name="plus" className="h-3 w-3" />
                {importBusy ? "导入中" : "导入"}
              </button>
            </div>
          ) : null}
          <div className="reading-shelf-list min-h-0 flex-1 overflow-y-auto p-2" data-testid="reading-shelf-list">
            {libraryError ? (
              <p className="rounded-lg bg-error-container/60 p-2 text-[11px] text-on-error-container">{libraryError}</p>
            ) : null}
            {shelfTab === "books" ? (
              <>
                {matchingBooks.length === 0 ? (
                  <p className="mb-2 rounded-lg bg-surface-container-low p-2.5 text-[12.5px] leading-relaxed text-on-surface-variant">
                    {shelfNeedle ? "没有匹配的书。" : "还没有导入的书。"}
                  </p>
                ) : null}
                {matchingBooks.map((item) => {
                  const active = item.book_id === bookId;
                  return (
                    <div
                      key={item.book_id}
                      className={
                        "reading-shelf-item relative w-full " + (active ? "is-active" : "")
                      }
                      data-testid="reading-book-card"
                    >
                      <button
                        type="button"
                        onClick={() => void openBook(item.book_id)}
                        className="flex w-full items-start gap-2 pr-14 text-left"
                      >
                        <BookCover
                          bookId={item.book_id}
                          cover={item.cover}
                          title={item.title}
                          className="reading-cover-shelf"
                        />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-[14px] font-semibold text-on-surface">{item.title}</span>
                          <span className="block text-[12px] text-outline">
                            {sourceFormatLabel(item.source_format)} · {item.chapter_count} 章 · {item.annotation_count} 条批注
                          </span>
                          <span className="reading-shelf-progress">
                            <span
                              className="reading-shelf-progress-fill"
                              style={{ width: `${Math.round(item.progress_percent * 100)}%` }}
                            />
                          </span>
                        </span>
                      </button>
                      <button
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          openRenameDialog(item);
                        }}
                        className="reading-shelf-edit"
                        aria-label={`重命名《${item.title}》`}
                        title="重命名书籍"
                        data-testid="reading-shelf-edit"
                      >
                        <Icon name="edit" className="h-3 w-3" />
                      </button>
                      <button
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          setPendingBookDelete(item);
                        }}
                        className="reading-shelf-delete"
                        aria-label={`删除《${item.title}》`}
                        title="删除书籍"
                        data-testid="reading-shelf-delete"
                      >
                        <Icon name="trash" className="h-3 w-3" />
                      </button>
                    </div>
                  );
                })}
              </>
            ) : (
              <>
                {matchingArticles.length === 0 ? (
                  <p className="rounded-lg bg-surface-container-low p-2.5 text-[12.5px] leading-relaxed text-on-surface-variant">
                    {shelfNeedle ? "没有匹配的订阅文章。" : "在仪表盘添加订阅源，文章会在后台自动更新。"}
                  </p>
                ) : null}
                {rssGroups.map(([sourceName, items]) => {
                  const collapsed = collapsedSources[sourceName] ?? true;
                  return (
                    <section key={sourceName} className="mb-1.5" data-testid="rss-source-group">
                      <button
                        type="button"
                        onClick={() =>
                          setCollapsedSources((prev) => ({ ...prev, [sourceName]: !collapsed }))
                        }
                        className="flex w-full items-center gap-1 px-1 py-1 text-left text-[12.5px] text-on-surface-variant hover:text-primary"
                        aria-expanded={!collapsed}
                      >
                        <Icon name={collapsed ? "chevron-down" : "chevron-up"} className="h-3 w-3 shrink-0" />
                        <span className="min-w-0 flex-1 truncate">{sourceName}</span>
                        <span className="shrink-0 text-[11.5px] text-outline">{items.length}</span>
                      </button>
                      {collapsed
                        ? null
                        : items.map((item) => {
                            const active = item.book_id === bookId;
                            const meta = rssMetaById.get(item.book_id);
                            return (
                              <button
                                type="button"
                                key={item.book_id}
                                onClick={() => void openBook(item.book_id)}
                                className={
                                  "reading-shelf-item relative w-full text-left " + (active ? "is-active" : "")
                                }
                                data-testid="reading-article-item"
                              >
                                {!meta?.read ? (
                                  <span className="absolute right-2 top-2 h-1.5 w-1.5 rounded-full bg-primary" aria-label="未读" />
                                ) : null}
                                <span className="block truncate pr-3 text-[13.5px] font-medium text-on-surface">
                                  {item.title}
                                </span>
                                <span className="mt-0.5 flex min-w-0 items-center gap-1 text-[11.5px] text-outline">
                                  {meta?.favorite ? (
                                    <Icon name="star" filled className="h-3 w-3 shrink-0 text-primary" />
                                  ) : null}
                                  <span className="shrink-0 rounded bg-primary/10 px-1 text-[11px] font-medium text-primary">RSS</span>
                                  {articleDateLabel(meta) ? (
                                    <span className="shrink-0">· {articleDateLabel(meta)}</span>
                                  ) : null}
                                  <span className="shrink-0">· {item.annotation_count} 条批注</span>
                                </span>
                              </button>
                            );
                          })}
                    </section>
                  );
                })}
              </>
            )}
          </div>
          {library?.root ? (
            <p className="truncate border-t border-outline-variant/30 px-3 py-2 text-[9.5px] text-outline" title={library.root}>
              阅读目录：{library.root}
            </p>
          ) : null}
          </div>
        </aside>
        ) : null}

        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          {!book && !bookBusy ? (
            <ReadingDashboard
              library={library}
              stats={readingStats}
              subscriptions={subscriptions}
              rssArticles={rssArticles}
              importBusy={importBusy}
              onImport={() => void importBook()}
              onViewAll={() => setShelfManagerOpen(true)}
              onOpenBook={(id) => void openBook(id)}
              onManageSubscriptions={() => setSubscriptionOpen(true)}
              onCustomFeed={() => setSubscriptionOpen(true)}
              onSubscribePreset={subscribePreset}
            />
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
                {book.source_format === "epub" ? (
                  <BookCover
                    key={book.book_id}
                    bookId={book.book_id}
                    cover={(library?.books ?? []).find((item) => item.book_id === book.book_id)?.cover}
                    title={book.title}
                    className="reading-cover-header hidden sm:inline-flex"
                  />
                ) : null}
                <div className="min-w-0 flex-1">
                  <h1 className="truncate text-[15px] font-medium text-on-surface">{book.title}</h1>
                  <p className="truncate text-[10.5px] text-outline">
                    {sourceFormatLabel(book.source_format)} · {book.chapters.length} 章 · {book.annotations.length} 条批注 · 读到
                    {" "}
                    {Math.round(book.progress.percent * 100)}%
                  </p>
                  <span className="reading-header-progress" aria-hidden="true">
                    <span style={{ width: `${Math.round(book.progress.percent * 100)}%` }} />
                  </span>
                </div>
                {book.source_format === "rss" ? (
                  <button
                    type="button"
                    onClick={() => void toggleArticleFavorite()}
                    disabled={favoriteBusy}
                    className={
                      "rounded-lg border p-1.5 disabled:opacity-50 " +
                      (currentArticleFavorite
                        ? "border-primary/60 bg-primary/10 text-primary"
                        : "border-outline-variant/50 text-on-surface-variant hover:text-primary")
                    }
                    aria-label={currentArticleFavorite ? "取消收藏文章" : "收藏文章"}
                    title={currentArticleFavorite ? "取消收藏" : "收藏文章"}
                    data-testid="reading-article-favorite"
                  >
                    <Icon name="star" filled={currentArticleFavorite} className="h-3.5 w-3.5" />
                  </button>
                ) : (
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
                )}
                <button
                  type="button"
                  onClick={() => {
                    setSideTab("notes");
                    setPanelOpen(true);
                  }}
                  className="rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] font-semibold text-on-surface-variant hover:text-primary"
                  aria-label="展开笔记面板"
                >
                  笔记
                </button>
                <button
                  type="button"
                  onClick={() => void exportNotes()}
                  className="hidden rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] font-semibold text-on-surface-variant hover:text-primary sm:inline-flex"
                >
                  导出
                </button>
                <button
                  type="button"
                  onClick={beginStandaloneThought}
                  className="hidden rounded-lg border border-outline-variant/50 px-2.5 py-1.5 text-[11px] font-semibold text-on-surface-variant hover:text-primary sm:inline-flex"
                  data-testid="reading-add-thought"
                >
                  写想法
                </button>
                <button
                  type="button"
                  onClick={() => void organizeBook()}
                  className="inline-flex items-center gap-1 rounded-lg bg-primary px-2.5 py-1.5 text-[11px] font-semibold text-on-primary disabled:opacity-50"
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
                className="reading-scroll reading-paper min-h-0 flex-1 overflow-y-auto"
              >
                <article
                  className="reading-article mx-auto w-full max-w-[760px]"
                  data-source-format={book.source_format}
                >
                  {book.chapters.map((chapter, chapterIndex) => (
                    <section key={`${chapterIndex}-${chapter.title}`} data-chapter-section={chapterIndex}>
                      <h2 className="reading-chapter-title">{chapter.title}</h2>
                      {chapter.paragraphs.map((paragraph, paragraphIndex) => {
                        const items = annotationMap.get(paragraphKey(chapterIndex, paragraphIndex)) ?? [];
                        const bookmarked = items.some((item) => item.kind === "bookmark");
                        const segments = paragraphSegments(paragraph, items);
                        const quoteLike =
                          book.source_format === "rss" && /^[「“>]/.test(paragraph.trim());
                        return (
                          <p
                            key={`${chapterIndex}-${paragraphIndex}`}
                            data-paragraph
                            data-chapter-index={chapterIndex}
                            data-paragraph-index={paragraphIndex}
                            className={`reading-paragraph ${bookmarked ? "reading-bookmarked" : ""} ${
                              quoteLike ? "reading-quote" : ""
                            }`}
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
                                  <RichParagraphText
                                    bookId={book.book_id}
                                    text={segment.text}
                                    onOpenImage={setLightbox}
                                  />
                                </mark>
                              ) : (
                                <span key={segment.key}>
                                  <RichParagraphText
                                    bookId={book.book_id}
                                    text={segment.text}
                                    onOpenImage={setLightbox}
                                  />
                                </span>
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
            ref={panelRef}
            style={panelOpen ? { width: panelWidth } : undefined}
            aria-label="笔记与 AI 面板"
            className={
              "reading-side-panel relative flex shrink-0 flex-col transition-[width] duration-200 ease-out " +
              (panelOpen
                ? "fixed bottom-0 right-0 top-[52px] z-40 flex max-w-[92vw] overflow-hidden shadow-[-16px_0_40px_-20px_rgba(70,55,40,0.28)] xl:static xl:z-auto xl:shadow-none"
                : "fixed bottom-0 right-0 top-[52px] z-30 w-10 bg-surface/70 xl:static xl:z-auto")
            }
          >
            {panelOpen ? (
              <>
            <div
              className="reading-panel-resizer"
              onMouseDown={beginPanelResize}
              onDoubleClick={() => setPanelWidth((w) => (w >= 460 ? 320 : 560))}
              title="拖动调整面板宽度；双击切换宽/窄"
              aria-label="调整右侧面板宽度"
            />
            <div className="reading-panel-tabs">
              <button
                type="button"
                onClick={() => setSideTab("notes")}
                className={
                  "reading-panel-tab " + (sideTab === "notes" ? "is-active" : "")
                }
              >
                笔记 {book.annotations.length + book.notes.length}
              </button>
              <button
                type="button"
                onClick={() => setSideTab("ai")}
                className={
                  "reading-panel-tab " + (sideTab === "ai" ? "is-active" : "")
                }
              >
                AI
              </button>
              <button
                type="button"
                onClick={() => setPanelOpen(false)}
                className="px-2 text-outline hover:text-on-surface"
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
                              "reading-note-card " + (activeAnnotation === annotation.id ? "is-active" : "")
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
                            className="reading-note-card w-full text-left text-[11.5px] text-on-surface"
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
                  <div className="reading-ai-well p-2.5">
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
                    <div className="reading-ai-well reading-ai-selection p-2.5">
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

                  <div className="reading-ai-well p-2.5">
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
                    <div className="reading-ai-well p-2.5" data-testid="reading-ai-answer">
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
                    <div className="reading-ai-well p-2.5" data-testid="reading-organized">
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
              </>
            ) : (
              <div
                className="flex flex-col items-center gap-2 py-3"
                data-testid="reading-panel-rail"
              >
                <button
                  type="button"
                  onClick={() => {
                    setSideTab("notes");
                    setPanelOpen(true);
                  }}
                  className={
                    "reading-panel-rail-button relative " + (sideTab === "notes" ? "is-active" : "")
                  }
                  aria-label={`展开笔记面板${book.annotations.length > 0 ? `，${book.annotations.length} 条批注` : ""}`}
                  title="笔记"
                >
                  <Icon name="note" className="h-3.5 w-3.5" />
                  {book.annotations.length > 0 ? (
                    <span className="absolute -right-1 -top-1 min-w-[15px] rounded-full bg-primary px-1 text-center text-[9px] leading-[15px] text-on-primary">
                      {book.annotations.length > 99 ? "99+" : book.annotations.length}
                    </span>
                  ) : null}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setSideTab("ai");
                    setPanelOpen(true);
                  }}
                  className={
                    "reading-panel-rail-button " + (sideTab === "ai" ? "is-active" : "")
                  }
                  aria-label="展开 AI 面板"
                  title="AI"
                >
                  <Icon name="spark" className="h-3.5 w-3.5" />
                </button>
              </div>
            )}
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
                <p className="text-[13px] font-medium text-on-surface">已导出阅读笔记</p>
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

      {shelfManagerOpen ? (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/20 p-4">
          <div
            className="flex max-h-[86vh] w-full max-w-2xl flex-col rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl"
            role="dialog"
            aria-modal="true"
            aria-label="书架管理"
            data-testid="reading-shelf-manager"
          >
            <div className="flex items-center justify-between gap-3 border-b border-outline-variant/30 pb-2">
              <div>
                <p className="text-[13px] font-medium text-on-surface">书架管理</p>
                <p className="text-[10px] text-outline">{shelfBooks.length} 本本地书籍</p>
              </div>
              <button
                type="button"
                onClick={() => setShelfManagerOpen(false)}
                className="text-outline hover:text-on-surface"
                aria-label="关闭书架管理"
              >
                ×
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto py-1">
              {shelfBooks.length === 0 ? (
                <p className="mt-2 rounded-lg bg-surface-container-low p-3 text-[11px] text-on-surface-variant">
                  还没有导入的书。
                </p>
              ) : null}
              {shelfBooks.map((meta) => (
                <div
                  key={meta.book_id}
                  className="flex items-center gap-3 border-b border-outline-variant/25 py-2 last:border-b-0"
                  data-testid="reading-shelf-manager-row"
                >
                  <button
                    type="button"
                    onClick={() => {
                      setShelfManagerOpen(false);
                      void openBook(meta.book_id);
                    }}
                    className="flex min-w-0 flex-1 items-center gap-3 text-left"
                  >
                    <BookCover
                      bookId={meta.book_id}
                      cover={meta.cover}
                      title={meta.title}
                      className="reading-cover-shelf"
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[14px] font-semibold text-on-surface">{meta.title}</span>
                      <span className="block text-[12px] text-outline">
                        {sourceFormatLabel(meta.source_format)} · {meta.chapter_count} 章 ·
                        {Math.round(meta.progress_percent * 100)}%
                      </span>
                    </span>
                  </button>
                  <button
                    type="button"
                    onClick={() => openRenameDialog(meta)}
                    className="shrink-0 rounded p-1.5 text-outline hover:text-primary"
                    aria-label={`重命名《${meta.title}》`}
                    title="重命名书籍"
                    data-testid="reading-shelf-manager-edit"
                  >
                    <Icon name="edit" className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() => setPendingBookDelete(meta)}
                    className="shrink-0 rounded p-1.5 text-outline hover:text-error"
                    aria-label={`删除《${meta.title}》`}
                    title="删除书籍"
                    data-testid="reading-shelf-manager-delete"
                  >
                    <Icon name="trash" className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      {renamingBook ? (
        <div className="fixed inset-0 z-[65] flex items-center justify-center bg-black/25 p-4">
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void saveRename();
            }}
            className="w-full max-w-sm rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl"
            role="dialog"
            aria-modal="true"
            aria-label="重命名书籍"
            data-testid="reading-rename-dialog"
          >
            <div className="flex items-center justify-between gap-3 border-b border-outline-variant/30 pb-2">
              <p className="text-[13px] font-medium text-on-surface">重命名书籍</p>
              <button
                type="button"
                onClick={() => setRenamingBook(null)}
                disabled={renameBusy}
                className="text-outline hover:text-on-surface disabled:opacity-50"
                aria-label="关闭重命名"
              >
                ×
              </button>
            </div>
            <label className="mt-3 block text-[11px] text-on-surface-variant" htmlFor="reading-rename-title">
              书名
            </label>
            <input
              id="reading-rename-title"
              autoFocus
              value={renameTitle}
              onChange={(event) => setRenameTitle(event.target.value)}
              maxLength={300}
              className="mt-1.5 h-9 w-full rounded-lg border border-outline-variant/50 bg-surface-container-lowest px-3 text-[13px] text-on-surface outline-none focus:border-primary"
              data-testid="reading-rename-input"
            />
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setRenamingBook(null)}
                disabled={renameBusy}
                className="rounded-lg px-3 py-1.5 text-[11.5px] text-on-surface-variant hover:bg-surface-container disabled:opacity-50"
                data-testid="reading-rename-cancel"
              >
                取消
              </button>
              <button
                type="submit"
                disabled={renameBusy || !renameTitle.trim()}
                className="rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary disabled:opacity-50"
                data-testid="reading-rename-save"
              >
                {renameBusy ? "保存中…" : "保存"}
              </button>
            </div>
          </form>
        </div>
      ) : null}

      {subscriptionOpen ? (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/20 p-4">
          <div className="flex max-h-[86vh] w-full max-w-2xl flex-col rounded-xl border border-outline-variant/40 bg-surface p-4 shadow-xl">
            <div className="flex items-center justify-between gap-3 border-b border-outline-variant/30 pb-2">
              <div>
                <p className="text-[13px] font-medium text-on-surface">订阅</p>
                <p className="text-[10px] text-outline">{subscriptions.length} 个来源</p>
              </div>
              <button
                type="button"
                onClick={() => setSubscriptionOpen(false)}
                className="text-outline hover:text-on-surface"
                aria-label="关闭订阅"
              >
                ×
              </button>
            </div>

            <div className="grid gap-1.5 border-b border-outline-variant/30 py-3 sm:grid-cols-[1fr_140px_auto]">
              <input
                value={subscribeUrl}
                onChange={(event) => setSubscribeUrl(event.target.value)}
                placeholder="RSS / Atom 地址"
                className="h-8 min-w-0 rounded-md border border-outline-variant/50 bg-surface-container-lowest px-2 text-[11.5px] text-on-surface outline-none focus:border-primary"
              />
              <input
                value={subscribeName}
                onChange={(event) => setSubscribeName(event.target.value)}
                placeholder="名称（可选）"
                className="h-8 min-w-0 rounded-md border border-outline-variant/50 bg-surface-container-lowest px-2 text-[11.5px] text-on-surface outline-none focus:border-primary"
              />
              <button
                type="button"
                onClick={() => void addRssSubscription()}
                disabled={rssBusy === "add" || !subscribeUrl.trim()}
                className="h-8 rounded-md bg-primary px-3 text-[11px] text-on-primary disabled:opacity-50"
              >
                {rssBusy === "add" ? "添加中" : "添加"}
              </button>
            </div>

            {rssMessage ? <p className="py-2 text-[11px] text-on-surface-variant" role="status">{rssMessage}</p> : null}
            {rssError ? (
              <p className="my-2 max-h-24 overflow-y-auto whitespace-pre-wrap rounded-lg bg-error-container/60 p-2 text-[10.5px] leading-relaxed text-on-error-container">
                {rssError}
              </p>
            ) : null}

            <div className="min-h-0 flex-1 overflow-y-auto py-2">
              {subscriptions.length === 0 ? (
                <p className="rounded-lg bg-surface-container-low p-3 text-[11px] text-on-surface-variant">
                  还没有订阅。
                </p>
              ) : null}
              {subscriptions.map((item) => (
                <div
                  key={item.id}
                  className="mb-1 flex items-start gap-2 rounded-lg border border-transparent px-2 py-1.5 hover:border-outline-variant/40 hover:bg-surface-container-low"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 items-center gap-1.5">
                      <span className="truncate text-[11.5px] font-medium text-on-surface">{item.name}</span>
                      <span className="shrink-0 text-[9.5px] text-outline">{item.group}</span>
                    </div>
                    <p className="truncate text-[9.5px] text-outline" title={item.url}>{item.url}</p>
                    {item.last_error ? (
                      <p className="mt-1 line-clamp-2 text-[9.5px] leading-relaxed text-error" title={item.last_error}>
                        {item.last_error}
                      </p>
                    ) : null}
                  </div>
                  <button
                    type="button"
                    onClick={() => setPendingSubscriptionDelete(item)}
                    className="shrink-0 rounded p-1 text-outline hover:text-error"
                    aria-label={`删除订阅 ${item.name}`}
                    title="删除订阅"
                  >
                    <Icon name="x" className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>

            <div className="flex items-center justify-between gap-2 border-t border-outline-variant/30 pt-2">
              <p className="text-[9.5px] text-outline">只请求订阅地址；文章正文不发外部服务。</p>
              <button
                type="button"
                onClick={() => void runRssFetch()}
                disabled={rssBusy === "refresh" || subscriptions.length === 0}
                className="inline-flex items-center gap-1 rounded-lg bg-primary px-3 py-1.5 text-[11px] text-on-primary disabled:opacity-50"
              >
                <Icon name="refresh" className="h-3 w-3" />
                {rssBusy === "refresh" ? "拉取中" : "刷新全部"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {lightbox ? (
        <div
          className="reading-lightbox fixed inset-0 z-[70] flex items-center justify-center bg-black/75 p-4"
          onClick={() => setLightbox(null)}
          role="dialog"
          aria-label="查看图片"
        >
          <img src={lightbox} alt="正文图片大图" onClick={(event) => event.stopPropagation()} />
          <button
            type="button"
            onClick={() => setLightbox(null)}
            className="reading-lightbox-close"
            aria-label="关闭图片"
          >
            ×
          </button>
        </div>
      ) : null}

      {toast ? (
        <div className="fixed bottom-5 left-1/2 z-[80] max-w-[min(620px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-outline-variant/50 bg-surface-bright px-3 py-2 text-[11.5px] leading-relaxed text-on-surface shadow-lg" role="status">
          {toast}
        </div>
      ) : null}

      <ConfirmDialog
        open={pendingBookDelete !== null}
        title="删除这本书？"
        description="删除后这本书的批注与笔记一并移除，导出文件保留。"
        confirmLabel="删除"
        onCancel={() => setPendingBookDelete(null)}
        onConfirm={() => void deleteBook()}
        testId="reading-book-delete-confirm"
      />

      <ConfirmDialog
        open={pendingDelete !== null}
        title="删除这条批注？"
        description="删除只会改动本机阅读目录里的这条批注，不会改动导入的原文件。"
        confirmLabel="删除"
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => void removeAnnotation()}
        testId="reading-delete-confirm"
      />

      <ConfirmDialog
        open={pendingSubscriptionDelete !== null}
        title="删除这个订阅？"
        description="只会移除订阅源；已经拉取的文章和批注仍保留在书架中。"
        confirmLabel="删除"
        onCancel={() => setPendingSubscriptionDelete(null)}
        onConfirm={() => void removeRssSubscription()}
        testId="rss-subscription-delete-confirm"
      />
    </div>
  );
}
