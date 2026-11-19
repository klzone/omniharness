import { useEffect, useMemo, useState } from "react";
import { shell } from "../api";
import type {
  ReadingBookMeta,
  ReadingLibrary,
  ReadingStats,
  RssArticleMeta,
  RssPresetSource,
  RssSubscription,
} from "../types";
import { Icon } from "../components/Icon";

type DashboardProps = {
  library: ReadingLibrary | null;
  stats: ReadingStats | null;
  subscriptions: RssSubscription[];
  rssArticles: RssArticleMeta[];
  importBusy: boolean;
  onImport: () => void;
  onViewAll: () => void;
  onOpenBook: (bookId: string) => void;
  onManageSubscriptions: () => void;
  onCustomFeed: () => void;
  onSubscribePreset: (source: RssPresetSource) => Promise<void>;
};

const coverSrcCache = new Map<string, string>();
const MONTHLY_GOAL_MINUTES = 1000;
/** 与界面展示的 100% 对齐：四舍五入后满 100 才算读完。 */
const READING_COMPLETE = 0.995;
const PETAL_ANGLES = [-90, -18, 54, 126, 198];
const PETAL_BASE_FILLS = [
  "rgba(232,176,75,.16)",
  "rgba(216,185,140,.18)",
  "rgba(184,91,56,.14)",
  "rgba(192,138,106,.13)",
  "rgba(143,162,113,.15)",
];
const PETAL_LABEL_POINTS = [
  [235, 6, 23],
  [412, 104, 121],
  [350, 308, 325],
  [120, 308, 325],
  [58, 104, 121],
] as const;
const PETAL_PATH = "M0 0 C 36 -40, 126 -46, 160 0 C 126 46, 36 40, 0 0";

function sourceFormatLabel(format: string): string {
  return format === "rss" ? "订阅文章" : format.toUpperCase();
}

function parseDate(value: string): Date | null {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatMonthDay(value: string): string {
  const date = parseDate(value);
  if (!date) return value.slice(0, 10) || "—";
  return `${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function formatCompact(value: number): string {
  if (value >= 100_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 10_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toLocaleString("zh-CN");
}

function isSameYear(value: string, now: Date): boolean {
  const date = parseDate(value);
  return !!date && date.getFullYear() === now.getFullYear();
}

function articleMonthTime(article: RssArticleMeta): number | null {
  if (article.published_ts > 0) return article.published_ts * 1000;
  const fetched = parseDate(article.fetched_at);
  return fetched ? fetched.getTime() : null;
}

function articleInMonth(article: RssArticleMeta, now: Date): boolean {
  const time = articleMonthTime(article);
  if (time === null) return false;
  const date = new Date(time);
  return date.getFullYear() === now.getFullYear() && date.getMonth() === now.getMonth();
}

function shortSourceName(name: string): string {
  return name.length > 9 ? name.slice(0, 9) + "…" : name;
}

function DashboardCover({ meta, className = "" }: { meta: ReadingBookMeta; className?: string }) {
  const [src, setSrc] = useState<string | null>(() => coverSrcCache.get(meta.book_id) ?? null);

  useEffect(() => {
    if (src) return;
    if (meta.cover?.startsWith("data:")) {
      coverSrcCache.set(meta.book_id, meta.cover);
      setSrc(meta.cover);
      return;
    }
    let alive = true;
    shell
      .readingCoverImage(meta.book_id)
      .then((payload) => {
        if (!alive) return;
        const dataUrl = `data:${payload.mime};base64,${payload.data_base64}`;
        coverSrcCache.set(meta.book_id, dataUrl);
        setSrc(dataUrl);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [meta.book_id, meta.cover, src]);

  return (
    <span className={`reading-dash-cover ${className}`}>
      {src ? (
        <img src={src} alt="" />
      ) : (
        <span className="reading-dash-cover-fallback" aria-hidden="true">
          {meta.title.trim().charAt(0) || "书"}
        </span>
      )}
    </span>
  );
}

function CompletionRing() {
  return (
    <svg className="reading-dash-ring" viewBox="0 0 44 44" aria-hidden="true">
      <circle cx="22" cy="22" r="17" fill="none" stroke="#e2d6c2" strokeWidth="3" />
      <circle
        cx="22"
        cy="22"
        r="17"
        fill="none"
        stroke="var(--color-primary)"
        strokeWidth="3"
        strokeLinecap="round"
        strokeDasharray="106.8 106.8"
        transform="rotate(-90 22 22)"
      />
      <text x="22" y="26" textAnchor="middle">
        100%
      </text>
    </svg>
  );
}

export function ReadingDashboard({
  library,
  stats,
  subscriptions,
  rssArticles,
  importBusy,
  onImport,
  onViewAll,
  onOpenBook,
  onManageSubscriptions,
  onCustomFeed,
  onSubscribePreset,
}: DashboardProps) {
  const now = useMemo(() => new Date(), []);
  const [presetSources, setPresetSources] = useState<RssPresetSource[]>([]);
  const [presetError, setPresetError] = useState<string | null>(null);
  const [subscribing, setSubscribing] = useState<string | null>(null);

  const books = library?.books ?? [];
  const shelfBooks = books.filter((item) => item.source_format !== "rss");
  const recentBook = useMemo(
    () =>
      shelfBooks
        .filter((item) => item.progress_percent > 0)
        .slice()
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0] ?? null,
    [shelfBooks],
  );
  const finishedBooks = useMemo(
    () =>
      shelfBooks
        .filter((item) => item.progress_percent >= READING_COMPLETE)
        .slice()
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
        .slice(0, 4),
    [shelfBooks],
  );
  const yearFinished = shelfBooks.filter(
    (item) => item.progress_percent >= READING_COMPLETE && isSameYear(item.updated_at, now),
  ).length;
  const monthMinutes = stats?.month_minutes ?? 0;
  const goalPercent = Math.round((monthMinutes / MONTHLY_GOAL_MINUTES) * 100);
  const goalWidth = Math.min(100, Math.max(0, goalPercent));
  const goalRemaining = Math.max(0, MONTHLY_GOAL_MINUTES - monthMinutes);
  const goalCompact = goalWidth < 12;

  const readArticleCount = rssArticles.filter((item) => item.read).length;
  const monthlyReadArticles = rssArticles.filter(
    (item) => item.read && articleInMonth(item, now),
  );
  const monthlyTotal = monthlyReadArticles.length;
  const subscriptionById = new Map(subscriptions.map((item) => [item.id, item]));
  const petalSources = useMemo(() => {
    const counts = new Map<string, { name: string; count: number }>();
    for (const article of monthlyReadArticles) {
      const id = article.subscription_id || article.feed_url;
      const subscription = subscriptionById.get(article.subscription_id);
      const name = subscription?.name || article.author || article.feed_url;
      const current = counts.get(id);
      if (current) current.count += 1;
      else counts.set(id, { name, count: 1 });
    }
    return Array.from(counts.values())
      .sort((a, b) => b.count - a.count)
      .slice(0, PETAL_ANGLES.length);
  }, [monthlyReadArticles, subscriptionById]);

  const empty = library !== null && books.length === 0 && subscriptions.length === 0;

  useEffect(() => {
    if (!empty) return;
    let alive = true;
    shell
      .rssPresetSources()
      .then((sources) => {
        if (alive) setPresetSources(sources);
      })
      .catch((error) => {
        if (alive) setPresetError(String(error));
      });
    return () => {
      alive = false;
    };
  }, [empty]);

  const subscribe = async (source: RssPresetSource) => {
    if (subscribing) return;
    setSubscribing(source.url);
    setPresetError(null);
    try {
      await onSubscribePreset(source);
    } catch (error) {
      setPresetError(String(error));
    } finally {
      setSubscribing(null);
    }
  };

  if (empty) {
    return (
      <div className="reading-dash reading-dash-empty" data-testid="reading-dashboard">
        <header className="reading-dash-header">
          <h1>健脑室</h1>
          <div className="reading-dash-pills">
            <span className="reading-dash-pill">
              连续 <b>{stats ? stats.streak_days : "—"}</b> 天
            </span>
            <span className="reading-dash-pill">
              本月 <b>{stats ? stats.month_minutes : "—"}</b> 分钟
            </span>
          </div>
        </header>
        <div className="reading-dash-empty-grid">
          <section className="reading-dash-empty-card">
            <span className="reading-dash-empty-illustration">
              <Icon name="book" className="h-8 w-8" />
            </span>
            <p>导入第一本书</p>
            <button type="button" onClick={onImport} disabled={importBusy} className="reading-dash-action">
              {importBusy ? "导入中…" : "选择文件"}
            </button>
          </section>
          <section className="reading-dash-empty-card">
            <span className="reading-dash-empty-illustration">
              <Icon name="radar" className="h-8 w-8" />
            </span>
            <p>订阅第一篇资讯</p>
            <button type="button" onClick={onCustomFeed} className="reading-dash-action">
              输入 RSS 地址
            </button>
          </section>
        </div>
        <div className="reading-dash-preset-row" aria-label="精选作者">
          {presetSources.map((source) => (
            <button
              type="button"
              key={source.url}
              onClick={() => void subscribe(source)}
              disabled={subscribing !== null}
              className="reading-dash-preset-chip"
            >
              {source.name}
            </button>
          ))}
          {presetSources.length === 0 && presetError ? (
            <span className="reading-dash-preset-error">{presetError}</span>
          ) : null}
        </div>
      </div>
    );
  }

  return (
    <div className="reading-dash" data-testid="reading-dashboard">
      <header className="reading-dash-header">
        <h1>健脑室</h1>
        <div className="reading-dash-pills">
          <span className="reading-dash-pill">
            连续 <b>{stats ? stats.streak_days : "—"}</b> 天
          </span>
          <span className="reading-dash-pill">
            本月 <b>{stats ? stats.month_minutes : "—"}</b> 分钟
          </span>
        </div>
      </header>

      <section className="reading-dash-top">
        <div className="reading-dash-hero">
          {recentBook ? (
            <DashboardCover meta={recentBook} className="reading-dash-hero-cover" />
          ) : (
            <span className="reading-dash-cover reading-dash-hero-cover reading-dash-cover-empty">
              <Icon name="book" className="h-5 w-5" />
            </span>
          )}
          <div className="reading-dash-hero-copy">
            <div className="reading-dash-hero-number">
              {recentBook ? Math.round(recentBook.progress_percent * 100) : 0}
              <small>%</small>
            </div>
            <p className="reading-dash-hero-sub">
              <b>{recentBook?.title ?? "还没有正在读的书"}</b>
            </p>
            {recentBook ? (
              <p className="reading-dash-hero-meta">
                {sourceFormatLabel(recentBook.source_format)} · {recentBook.chapter_count} 章 · 上次阅读
                {" "}
                {formatMonthDay(recentBook.updated_at)}
              </p>
            ) : null}
          </div>
        </div>

        <div className="reading-dash-goal">
          <div className="reading-dash-goal-top">
            <strong>本月阅读目标</strong>
            <span>
              {monthMinutes} / {MONTHLY_GOAL_MINUTES} 分钟 · 已完成 {goalPercent}%
            </span>
          </div>
          <div className="reading-dash-goal-track">
            <div className="reading-dash-goal-fill" style={{ width: `${goalWidth}%` }}>
              {goalCompact ? null : goalRemaining > 0 ? `还差 ${goalRemaining} 分钟` : "已达成"}
            </div>
          </div>
          {goalCompact ? (
            <p className="reading-dash-goal-below">
              {goalRemaining > 0 ? `还差 ${goalRemaining} 分钟` : "已达成"}
            </p>
          ) : null}
          <button
            type="button"
            className="reading-dash-continue"
            disabled={!recentBook}
            onClick={() => recentBook && onOpenBook(recentBook.book_id)}
          >
            继续阅读
          </button>
        </div>
      </section>

      <section className="reading-dash-metrics">
        <div className="reading-dash-metric">
          <label>阅读字数</label>
          <div className="reading-dash-metric-num">
            {stats ? formatCompact(stats.chars) : "—"}
            <small>字</small>
          </div>
        </div>
        <div className="reading-dash-metric">
          <label>今年已读</label>
          <div className="reading-dash-metric-num">
            {yearFinished}
            <small>本</small>
          </div>
        </div>
        <div className="reading-dash-metric">
          <label>批注</label>
          <div className="reading-dash-metric-num">
            {stats ? stats.annotations.toLocaleString("zh-CN") : "—"}
            <small>条</small>
          </div>
        </div>
        <div className="reading-dash-metric">
          <label>订阅文章</label>
          <div className="reading-dash-metric-num">
            {stats ? stats.rss_articles.toLocaleString("zh-CN") : "—"}
            <small>篇</small>
            <span className="reading-dash-metric-note">{readArticleCount} 读完</span>
          </div>
        </div>
        <div className="reading-dash-metric">
          <label>整理笔记</label>
          <div className="reading-dash-metric-num">
            {stats ? stats.notes.toLocaleString("zh-CN") : "—"}
            <small>篇</small>
          </div>
        </div>
      </section>

      <section className="reading-dash-bottom">
        <section className="reading-dash-col-left" aria-label="已读书籍">
          <div className="reading-dash-block-head reading-dash-block-head-right">
            <button type="button" onClick={onImport} disabled={importBusy} className="reading-dash-action">
              + 导入书籍
            </button>
            <button type="button" onClick={onViewAll} className="reading-dash-action">
              查看全部
            </button>
          </div>
          {finishedBooks.length === 0 ? (
            <p className="reading-dash-empty-note">还没有读完的书。</p>
          ) : (
            finishedBooks.map((meta) => (
              <button
                type="button"
                key={meta.book_id}
                onClick={() => onOpenBook(meta.book_id)}
                className="reading-dash-book-row"
              >
                <DashboardCover meta={meta} className="reading-dash-row-cover" />
                <span className="reading-dash-row-main">
                  <b>{meta.title}</b>
                  <small>
                    {sourceFormatLabel(meta.source_format)} · {meta.chapter_count} 章 ·
                    {formatMonthDay(meta.updated_at)} 读完
                  </small>
                </span>
                <span className="reading-dash-author">
                  <i>著</i>
                  {meta.author?.trim() || "未知作者"}
                </span>
                <span className="reading-dash-date">{formatMonthDay(meta.updated_at)}</span>
                <CompletionRing />
              </button>
            ))
          )}
        </section>

        <section className="reading-dash-col-right">
          <div className="reading-dash-block-head">
            <strong>喜欢的 RSS 订阅</strong>
            <button type="button" onClick={onManageSubscriptions} className="reading-dash-action">
              管理订阅
            </button>
          </div>
          <div className="reading-dash-chart">
            <svg viewBox="0 0 470 350" preserveAspectRatio="xMidYMid meet" role="img" aria-label="本月订阅文章阅读分布">
              <defs>
                {[0, 1, 2, 3, 4].map((index) => (
                  <radialGradient key={index} id={`reading-dash-petal-grad-${index}`} cx="35%" cy="35%" r="70%">
                    <stop offset="0%" stopColor={["#E8B04B", "#D8B98C", "#B85B38", "#C08A6A", "#8FA271"][index]} stopOpacity="0.72" />
                    <stop offset="100%" stopColor={["#E8B04B", "#D8B98C", "#B85B38", "#C08A6A", "#8FA271"][index]} stopOpacity="0.2" />
                  </radialGradient>
                ))}
              </defs>
              <g transform="translate(235 175)">
                <g className="reading-dash-petals-base">
                  {PETAL_ANGLES.map((angle, index) => (
                    <g key={angle} transform={`rotate(${angle})`}>
                      <path d={PETAL_PATH} fill={PETAL_BASE_FILLS[index]} />
                    </g>
                  ))}
                </g>
                <g className="reading-dash-petals-data">
                  {petalSources.map((source, index) => {
                    const scale = monthlyTotal > 0 ? source.count / monthlyTotal : 0;
                    return (
                      <g key={source.name + index} transform={`rotate(${PETAL_ANGLES[index]}) scale(${scale})`}>
                        <path d={PETAL_PATH} fill={`url(#reading-dash-petal-grad-${index})`} />
                      </g>
                    );
                  })}
                </g>
                <circle cx="0" cy="0" r="24" fill="#F1E8D6" stroke="rgba(255,255,255,.8)" strokeWidth="1" />
              </g>
              {petalSources.map((source, index) => {
                const percent = monthlyTotal > 0 ? Math.round((source.count / monthlyTotal) * 100) : 0;
                const [x, percentY, labelY] = PETAL_LABEL_POINTS[index];
                return (
                  <g key={source.name + "-label"}>
                    <text x={x} y={percentY} textAnchor="middle" className="reading-dash-chart-percent">
                      {percent}%
                    </text>
                    <text x={x} y={labelY} textAnchor="middle" className="reading-dash-chart-label">
                      {shortSourceName(source.name)}
                    </text>
                  </g>
                );
              })}
            </svg>
            <span className="reading-dash-petal-center" aria-hidden="true">
              R
            </span>
          </div>
          {monthlyTotal === 0 ? (
            <p className="reading-dash-empty-note">本月还没有读完的订阅文章。</p>
          ) : null}
        </section>
      </section>
    </div>
  );
}
