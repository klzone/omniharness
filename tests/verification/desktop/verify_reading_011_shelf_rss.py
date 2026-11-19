#!/usr/bin/env python3
"""OD-READ-002 阶段 10 静态核验：侧栏排版、自动刷新、30 天过滤、收藏、改名与随机展开。"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
DASH = ROOT / "desktop" / "src" / "shell" / "ReadingDashboard.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
ICON = ROOT / "desktop" / "src" / "components" / "Icon.tsx"
TYPES = ROOT / "desktop" / "src" / "types.ts"
API = ROOT / "desktop" / "src" / "api.ts"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
STORE = ROOT / "desktop" / "src-tauri" / "src" / "reading_store.rs"

FAILS = []
CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append(name)
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def block(source: str, start: str, end: str) -> str:
    index = source.find(start)
    if index < 0:
        return ""
    tail = source[index:]
    stop = tail.find(end, len(start))
    return tail if stop < 0 else tail[:stop]


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    dash = DASH.read_text(encoding="utf-8")
    css = CSS.read_text(encoding="utf-8")
    icon = ICON.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    rss = RSS.read_text(encoding="utf-8")
    store = STORE.read_text(encoding="utf-8")

    # 1. 侧栏排版
    check("健脑室标题 16px/700/.08em", 'text-[16px] font-bold tracking-[0.08em]' in page)
    check("书籍标题 14px semibold", 'text-[14px] font-semibold text-on-surface' in page)
    check("书籍副信息 12px", 'block text-[12px] text-outline' in page)
    check("侧栏 tabs 12.5px", page.count("12.5px") >= 1 and css.count("font-size: 12.5px") >= 2)
    check("搜索占位 13px", "font-size: 13px" in css)
    check("文章行标题 13.5px", 'text-[13.5px]' in page)
    check("文章行时间/徽标 11.5px", page.count("text-[11.5px]") >= 1)

    # 2. 移除两个按钮，保留仪表盘管理入口
    check("侧栏无拉取全部按钮", "拉取全部" not in page)
    check("侧栏无订阅管理按钮", "订阅管理" not in page)
    check("仪表盘保留管理订阅", "管理订阅" in dash and "onManageSubscriptions" in dash)
    check("空态文案不再指向旧按钮", "在仪表盘添加订阅源" in page and "或点「拉取全部」" not in page)

    # 3. 30 分钟自动刷新
    check("30 分钟常量", "RSS_AUTO_REFRESH_MS = 30 * 60 * 1000" in page)
    check("读取 last_fetched_at 最大值", "last_fetched_at" in page and "Math.max(...stamps)" in page)
    check("挂载后检查订阅", "useEffect(() => {\n    maybeAutoRefreshRss();" in page)
    check("聚焦时再次检查", 'window.addEventListener("focus", onFocus)' in page)
    check("后台调用 rssFetch", "await shell.rssFetch()" in page)
    check("触发时间防重复", "lastAutoRefreshRef.current" in page and "RSS_AUTO_REFRESH_MS) return" in page)
    check("后台刷新后刷新列表", "await Promise.all([refreshLibrary(), refreshRss()])" in page)

    # 4. 30 天过滤 + 收藏
    check("30 天保留常量", "RSS_SHELF_RETENTION_MS = 30 * 24 * 60 * 60 * 1000" in page)
    check("published_ts 优先，否则 fetched_at", "meta.published_ts > 0" in page and "Date.parse(meta.fetched_at)" in page)
    check("favorite 豁免 30 天", "rssMetaById.get(item.book_id)?.favorite) return true" in page)
    check("超期未收藏不显示", "Date.now() - stamp <= RSS_SHELF_RETENTION_MS" in page)
    check("分组计数用过滤后的文章", "for (const item of matchingArticles)" in page and "items.length" in page)
    check("Rust favorite serde default", bool(re.search(r"#[\s\S]{0,20}serde\(default\)\][\s\S]{0,10}pub favorite: bool", rss)))
    favorite_body = block(rss, "fn set_article_favorite_at", "\nfn article_path")
    check("favorite 只写 articles/<id>.json", "article_path(root, book_id)" in favorite_body and "write_book" not in favorite_body)
    check("favorite 命令与注册", "fn rss_article_set_favorite" in lib and "rss_article_set_favorite," in lib)
    check("前端 rssArticleSetFavorite", "rssArticleSetFavorite(bookId: string, favorite: boolean)" in api)
    check("前端类型 favorite", re.search(r"read: boolean;\s*[^}]*favorite: boolean;", types) is not None)
    check("侧栏文章行收藏星标", "meta?.favorite ?" in page and '<Icon name="star" filled' in page)

    # 5. 阅读区头栏
    check("头栏四个按钮加粗", page.count("text-[11px] font-semibold") >= 4)
    check("star 图标存在且可实心", 'star: (' in icon and 'filled?: boolean' in icon and 'fill={filled ? "currentColor" : "none"}' in icon)
    check("RSS 头栏替换为收藏按钮", 'book.source_format === "rss" ?' in page and 'data-testid="reading-article-favorite"' in page)
    check("收藏按钮实心/主色与 toast", "filled={currentArticleFavorite}" in page and "已取消收藏" in page)
    check("本地书保留导入 +", 'aria-label="导入阅读文件"' in page and '<Icon name="plus"' in page)

    # 6. 书籍改名
    rename_body = block(store, "pub fn rename_book_at", "/// 删除一本本地书籍")
    check("Rust rename_book_at", bool(rename_body))
    check("改名拒绝 rss", "source_format == \"rss\"" in rename_body)
    check("改名长度 1..=300", "(1..=300).contains(&title_chars)" in rename_body)
    check("改名写 books 与 meta 标题", 'book_value["title"]' in rename_body and 'meta_value["title"]' in rename_body)
    check("改名命令与注册", "fn reading_rename_book" in lib and "reading_rename_book," in lib)
    check("前端 readingRenameBook", "readingRenameBook(bookId: string, title: string)" in api)
    check("侧栏铅笔按钮", 'data-testid="reading-shelf-edit"' in page and '<Icon name="edit"' in page)
    check("管理弹窗铅笔按钮", 'data-testid="reading-shelf-manager-edit"' in page)
    check("改名弹窗输入与保存", 'data-testid="reading-rename-dialog"' in page and 'data-testid="reading-rename-input"' in page and 'data-testid="reading-rename-save"' in page)
    check("保存后刷新 library 与标题", "await refreshLibrary();" in page and "{ ...prev, title: meta.title }" in page)

    # 7. 订阅源折叠与随机展开一次
    check("初始折叠为 true", "collapsedSources[sourceName] ?? true" in page)
    check("随机挑一个来源", "Math.random() * rssGroups.length" in page)
    check("ref 防重复展开", "randomSourceExpanded.current" in page and "if (randomSourceExpanded.current || rssGroups.length === 0) return;" in page)

    print(f"cases={len(CASES)} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
