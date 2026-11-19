#!/usr/bin/env python3
"""OD-READ-002 阶段 7 静态核验：真实阅读时长、仪表盘胶囊/继续阅读/R、媒体回填与 RSS enclosure。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
DASH = ROOT / "desktop" / "src" / "shell" / "ReadingDashboard.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
API = ROOT / "desktop" / "src" / "api.ts"
TYPES = ROOT / "desktop" / "src" / "types.ts"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
STORE = ROOT / "desktop" / "src-tauri" / "src" / "reading_store.rs"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
SHELL = ROOT / "desktop" / "src-tauri" / "src" / "task_shell.rs"
HELPER = ROOT / "scripts" / "reading_attachment_extract.py"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    page = PAGE.read_text()
    dash = DASH.read_text()
    css = CSS.read_text()
    api = API.read_text()
    types = TYPES.read_text()
    lib = LIB.read_text()
    store = STORE.read_text()
    rss = RSS.read_text()
    shell = SHELL.read_text()
    helper = HELPER.read_text()

    check("标题右侧两枚胶囊", "reading-dash-pills" in dash and "reading-dash-pill" in dash)
    check("连续天数胶囊", "连续 <b>{stats ? stats.streak_days : " + '"—"' + "}</b> 天" in dash)
    check("本月分钟胶囊", "本月 <b>{stats ? stats.month_minutes : " + '"—"' + "}</b> 分钟" in dash)
    check("目标区使用真实 month_minutes", "const monthMinutes = stats?.month_minutes ?? 0" in dash and "{monthMinutes} /" in dash)
    check("删除估算字样与字数公式", "估算" not in dash and "item.char_count * item.progress_percent" not in dash)

    check("继续阅读按钮存在", 'className="reading-dash-continue"' in dash and "继续阅读" in dash)
    check("继续阅读打开最近在读书籍", "progress_percent > 0" in dash and "onOpenBook(recentBook.book_id)" in dash)
    check("无最近在读书时禁用", "disabled={!recentBook}" in dash)
    check("花瓣中心为大写 R", '<span className="reading-dash-petal-center" aria-hidden="true">' in dash and "\n              R\n" in dash)
    check("R 字 16px 主色", "font-size: 16px" in css and "color: var(--color-primary)" in css)
    check("R 中心不再使用书籍 icon", 'reading-dash-petal-center' in dash and dash.count('<Icon name="book"') <= 2)

    check("作者列始终显示", "meta.author?.trim() || " + '"未知作者"' in dash)
    check("作者回退文案", '未知作者' in dash)

    check("下部两栏 1000px 断点", "@media (max-width: 999px)" in css)
    bottom_block = css.split("@media (max-width: 999px)", 1)[-1]
    check("窄屏下部堆叠", "grid-template-columns: minmax(0, 1fr)" in bottom_block and ".reading-dash-bottom" in bottom_block)
    block_1280 = css.split("@media (max-width: 1280px)", 1)[1].split("@media (max-width: 999px)", 1)[0]
    check("1280 断点不再强制堆叠下部", ".reading-dash-bottom" not in block_1280)

    check("ReadingStats 含 month_minutes/streak_days", "month_minutes: u64" in store and "streak_days: u32" in store)
    check("前端 stats 类型同步", "month_minutes: number" in types and "streak_days: number" in types)
    check("progress daily_seconds 字段", "pub daily_seconds: BTreeMap<String, u64>" in store and "#[serde(default)]" in store)
    check("计时命令 reading_time_add", "fn reading_time_add" in lib and "reading_store::add_reading_time_at" in lib)
    check("计时命令已注册", "reading_time_add," in lib.split("invoke_handler", 1)[-1])
    check("单次计时上限 3600", "seconds > 3_600" in store and "单次阅读计时最多 3600 秒" in store)
    check("前端 readingTimeAdd", "readingTimeAdd(bookId: string, seconds: number)" in api and '"reading_time_add"' in api)
    check("可见且聚焦才累计", "document.visibilityState === \"visible\" && document.hasFocus()" in page)
    check("每 15 秒累计", "pendingSeconds.current += 15" in page and "15_000" in page)
    check("每 60 秒 flush", "60_000" in page and "flushReadingTime" in page)
    check("切书/关闭/隐藏 flush", "visibilitychange" in page and 'window.addEventListener("blur"' in page and "flushReadingTime(id)" in page)
    check("月份分钟四舍五入", "(month_secs + 30) / 60" in store)
    check("连续天数今天为 0 从昨天起算", "today_secs == 0" in store and "day_num -= 1" in store)

    check("refresh_content 存储函数", "pub fn refresh_content_at" in store and "read_reading_source(&book.source_path)" in store)
    check("仅 EPUB/MOBI 回填", 'book.source_format != "epub" && book.source_format != "mobi"' in store)
    check("按 quote 先原段落再全章", "fn relocate_annotations" in store and "chapter.paragraphs.get(annotation.paragraph_index)" in store)
    check("找不到保留原位置并报告 unresolved", "unresolved += 1" in store and "pub unresolved: usize" in store)
    check("media_version 字段", "pub media_version: u32" in store and "media_version: 1" in store)
    check("新旧导入均写 media_version", "media_version: 1," in store and "media_version: 1," in rss)
    check("前端类型 media_version", "media_version?: number" in types)
    check("refresh 命令", "fn reading_refresh_content" in lib and "reading_store::refresh_content_at" in lib)
    check("refresh 命令已注册", "reading_refresh_content," in lib.split("invoke_handler", 1)[-1])
    check("前端 readingRefreshContent", "readingRefreshContent(bookId: string)" in api and '"reading_refresh_content"' in api)
    check(
        "openBook 先显示后回填",
        "先显示旧正文" in page
        and "shell.readingRefreshContent(id)" in page
        and "setBook({ ...refreshed.book" in page
        and "loaded.media_version ?? 0) < 1" in page,
    )
    check("失败置 1 防重复并提示", "mark_media_version(root, book_id)" in store and "正文媒体回填失败：" in page)

    check("EPUB dc:creator 写入 author", '"creator"' in helper and 'payload["author"] = author' in helper)
    check("ReadingSource author 字段", "pub author: String" in shell and "author = out.author" in shell)

    check("RSS enclosure/media:content 解析", "fn entry_image_url" in rss and 'child.name != "enclosure" && child.name != "content"' in rss)
    check("仅 http/https 图片", 'lower_url.starts_with("http://") || lower_url.starts_with("https://")' in rss)
    check("仅 type image/*", 'lower.starts_with("image/")' in rss)
    check("正文无图时前置 ![img]", 'format!("![img]({})", markdown_url(&entry.image_url))' in rss)
    check("旧文章无批注才回填", "book.annotations.is_empty()" in rss and "book.chapters = vec![ReadingChapter" in rss)
    check("RSS 回填保留 progress/read", "book.progress" not in rss.split("book.annotations.is_empty()", 1)[-1].split("write_book", 1)[0])

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
