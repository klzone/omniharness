#!/usr/bin/env python3
"""OD-READ-002 阶段 8 静态核验：顶部始终并排 + RSS 旧图片占位回填。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CSS = ROOT / "desktop" / "src" / "reading.css"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
API = ROOT / "desktop" / "src" / "api.ts"
TYPES = ROOT / "desktop" / "src" / "types.ts"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def block_after(text: str, marker: str) -> str:
    return text.split(marker, 1)[1] if marker in text else ""


def main() -> int:
    css = CSS.read_text()
    lib = LIB.read_text()
    rss = RSS.read_text()
    api = API.read_text()
    types = TYPES.read_text()
    page = PAGE.read_text()

    base_top = css.split(".reading-dash-top {", 1)[1].split("}", 1)[0]
    check("顶部基准为两列", "grid-template-columns" in base_top and "minmax(0, 0.95fr)" in base_top and "minmax(0, 1.05fr)" in base_top, base_top)
    check("1280 断点不再把顶部改单列", ".reading-dash-top" not in block_after(css, "@media (max-width: 1280px)").split("@media (max-width: 900px)", 1)[0])
    check("1280 断点不再把 hero 左内边距归零", ".reading-dash-hero" not in block_after(css, "@media (max-width: 1280px)").split("@media (max-width: 900px)", 1)[0])
    narrow = block_after(css, "@media (max-width: 900px)").split("@media", 1)[0]
    check("≤900 缩小顶部间距", ".reading-dash-top" in narrow and "gap: 20px" in narrow, narrow)
    check("≤900 缩小数字字号", ".reading-dash-hero-number" in narrow and "font-size: 64px" in narrow, narrow)

    check("回填结果结构 updated/reason", "pub struct RssArticleRefreshResult" in rss and "pub updated: bool" in rss and "pub reason: String" in rss)
    check("回填入口读取文章元数据", "pub fn refresh_article" in rss and "read_article_meta_at" in rss and "article_path(root, book_id)" in rss)
    check("重新抓取所属 feed", "subscription_feed_url(root, &meta)" in rss and "fetch_feed_xml(&client, &feed_url)" in rss)
    check("按 guid 或 link 匹配条目", 'entry.guid.trim() == meta.guid.trim()' in rss and 'entry.link.trim() == meta.link.trim()' in rss)
    check("匹配后重算 html_to_paragraphs", "html_to_paragraphs(&entry.content_html, &base_url)" in rss)
    check("更新 chapters/author/source_sha256", "book.chapters = vec![ReadingChapter" in rss and "book.author = author" in rss and "book.source_sha256 = sha256_hex(entry.content_html.as_bytes())" in rss)
    check("保留 progress/imported_at", "book.progress" not in rss.split("fn refresh_article_with_xml_at", 1)[1].split("pub fn fetch", 1)[0] and "imported_at" not in rss.split("fn refresh_article_with_xml_at", 1)[1].split("pub fn fetch", 1)[0])
    check("有批注不改文件", 'if !book.annotations.is_empty()' in rss and '文章已有批注，未回填正文。' in rss)
    check("未匹配返回 false 原因", '订阅源中没有匹配的文章条目' in rss and "updated: false" in rss)
    check("正文一致不重复写", "订阅源正文与本地一致，无需回填。" in rss)
    check("RSS 回填置 media_version", "book.media_version = 1" in rss)

    check("html_to_paragraphs 接收 base_url", "fn html_to_paragraphs(html: &str, base_url: &str)" in rss)
    check("sanitize_html 接收 base_url", "fn sanitize_html(html: &str, base_url: &str)" in rss)
    check("sanitize_media_url 接收 base_url", "fn sanitize_media_url(src: &str, base_url: &str)" in rss)
    check("相对地址解析器", "fn resolve_media_url" in rss and 'value.starts_with("//")' in rss and "value.starts_with('/')" in rss)
    check("相对图片走 markdown 图片标记", 'format!("![img]({})", markdown_url(&url))' in rss)
    check("仅 http/https 放行", 'resolved_lower.starts_with("http://") || resolved_lower.starts_with("https://")' in rss)
    check("解析失败不生成媒体标记", "let resolved = resolve_media_url(value, base_url)?;" in rss)

    check("refresh_article 命令", "fn rss_refresh_article" in lib and "rss_store::refresh_article(&paths, &book_id)" in lib)
    check("refresh_article 已注册", "rss_refresh_article," in lib.split("invoke_handler", 1)[-1])
    check("前端 api rssRefreshArticle", "rssRefreshArticle(bookId: string): Promise<RssArticleRefreshResult>" in api and '"rss_refresh_article"' in api)
    check("前端类型同步", "export interface RssArticleRefreshResult" in types and "updated: boolean" in types and "reason: string" in types)

    check("旧占位检测", "function hasLegacyImagePlaceholder" in page and 'paragraph.includes("[图片：")' in page)
    check("openBook 仅对无批注 rss 触发", "loaded.source_format === \"rss\"" in page and "loaded.annotations.length === 0" in page)
    check("openBook 调用 rssRefreshArticle", "shell.rssRefreshArticle(id)" in page)
    check("成功后重载一次", "if (result.updated)" in page and "loaded = await shell.readingBook(id)" in page)
    check("一次标记防循环", "rssRefreshAttempted.current.add(id)" in page and "!rssRefreshAttempted.current.has(id)" in page)
    check("失败 toast 原因", "旧图片回填失败：" in page and "旧图片回填未完成：" in page)

    check("单测：相对图片解析", "fn resolves_relative_image_sources_against_entry_link" in rss)
    check("单测：回填成功保留进度", "fn refresh_article_rebuilds_body_and_keeps_progress" in rss and "assert_eq!(updated.progress.percent, 0.42)" in rss)
    check("单测：有批注跳过", "fn refresh_article_skips_when_annotations_exist" in rss)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
