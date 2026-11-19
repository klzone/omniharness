#!/usr/bin/env python3
"""OD-READ-002 阶段 6 静态核验：书架显隐/管理、书籍删除、封面回退、远程图片与仪表盘修正。"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
DASH = ROOT / "desktop" / "src" / "shell" / "ReadingDashboard.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
API = ROOT / "desktop" / "src" / "api.ts"
ICON = ROOT / "desktop" / "src" / "components" / "Icon.tsx"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
STORE = ROOT / "desktop" / "src-tauri" / "src" / "reading_store.rs"
SHELL = ROOT / "desktop" / "src-tauri" / "src" / "task_shell.rs"

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
    icon = ICON.read_text()
    lib = LIB.read_text()
    store = STORE.read_text()
    shell = SHELL.read_text()

    first_aside = page.find("<aside")
    shelf_guard = page.rfind("{book ? (", 0, first_aside) >= 0 if first_aside >= 0 else False
    shelf_close = re.search(r"</aside>\s*\)\s*:\s*null\}", page[first_aside:]) if first_aside >= 0 else None
    check("左侧书架仅在 book 非空时渲染", shelf_guard and shelf_close is not None)
    check("仪表盘独占整宽（空态不再与书架并排）", "onViewAll={() => setShelfManagerOpen(true)}" in page)

    check("书架管理弹窗状态与 Esc 关闭", "shelfManagerOpen" in page and "reading-shelf-manager" in page and 'event.key !== "Escape"' in page)
    check("管理弹窗列出全部本地书籍", "shelfBooks.map((meta) =>" in page and "data-testid=\"reading-shelf-manager-row\"" in page)
    check("管理弹窗行含封面/格式/章数/进度", "<BookCover" in page and "sourceFormatLabel(meta.source_format)" in page and "Math.round(meta.progress_percent * 100)" in page)
    check("管理弹窗可打开书籍", "void openBook(meta.book_id)" in page)
    check("书架行删除按钮 stopPropagation + trash", "event.stopPropagation()" in page and "setPendingBookDelete(item)" in page and '<Icon name="trash"' in page and '"trash"' in icon)
    check("确认文案完整", "删除后这本书的批注与笔记一并移除，导出文件保留。" in page)
    check("删除后刷新书架并关闭当前阅读器", "await shell.readingDeleteBook(target.book_id)" in page and "await refreshLibrary()" in page and "bookId === target.book_id" in page and "setBook(null)" in page)

    check("存储层 delete_book_at", "pub fn delete_book_at(root: &Path, book_id: &str)" in store)
    check("删除前校验 book_id", "if !is_safe_id(book_id)" in store and "书编号不合法" in store)
    check("RSS 条目一律拒绝删除", 'if meta.source_format == "rss"' in store)
    delete_start = store.find("pub fn delete_book_at")
    delete_doc_start = store.rfind("/// 删除一本本地书籍", 0, delete_start)
    delete_end = store.find("pub fn import_book_at", delete_start)
    block_start = delete_doc_start if delete_doc_start >= 0 else delete_start
    delete_block = store[block_start:delete_end] if delete_start >= 0 and delete_end > delete_start else ""
    check("删除 books/meta/progress/notes/cover", all(
        token in delete_block for token in (
            'format!("books/{book_id}.json")',
            '{book_id}.cover.',
            'format!("progress/{book_id}.json")',
            "remove_dir_all(&notes_dir)",
            'format!("meta/{book_id}.json")',
        )
    ))
    check("不删除 exports 目录", 'root.join("exports")' not in delete_block and "exports/ 始终保留" in delete_block)
    check("删除部分失败时报告已删项", "未完全成功。已删除" in delete_block and "failures" in delete_block)

    check("读封面缺失时从源文件回提取", "fn extract_cover_from_source" in store and "read_reading_source(&book.source_path)" in store and "store_cover(root, book_id, cover)" in store)
    check("回提取后原子更新 meta.cover", "meta.cover = Some(extracted)" in store and "write_replace_atomically(&meta_path" in store)
    check("提取失败仍返回没有封面", "这本书没有封面。" in store)
    check("DashboardCover 不再因 cover 为空早退", "if (src || !meta.cover) return;" not in dash and "meta.cover?.startsWith" in dash and ".readingCoverImage(meta.book_id)" in dash)
    check("书架封面同样不再早退", "if (!cover || src) return;" not in page and "coverSrcCache.get(bookId)" in page)

    check("宿主命令 reading_delete_book", "fn reading_delete_book" in lib and "reading_store::delete_book_at" in lib)
    check("reading_delete_book 已注册", "reading_delete_book," in lib.split("invoke_handler", 1)[-1])
    check("前端 readingDeleteBook", 'readingDeleteBook(bookId: string)' in api and '"reading_delete_book"' in api)

    check("远程图片命令 reading_remote_media", "fn reading_remote_media" in lib and "pub fn read_remote_media" in shell)
    check("远程图片命令已注册", "reading_remote_media," in lib.split("invoke_handler", 1)[-1])
    check("仅 http/https", 'matches!(current.scheme(), "http" | "https")' in shell and "只允许读取 http/https" in shell)
    check("重定向最多 2 跳且逐跳校验", "Policy::none()" in shell and "hops >= 2" in shell and "图片重定向只允许 http/https" in shell)
    check("10s 超时", "REMOTE_IMAGE_TIMEOUT: Duration = Duration::from_secs(10)" in shell)
    check("32 MiB 媒体上限", "REMOTE_IMAGE_MAX_BYTES: usize = 32 * 1024 * 1024" in shell and "媒体超过 32 MiB 上限" in shell)
    check("Content-Type 白名单", all(mime in shell for mime in ("image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp", "image/svg+xml")))
    check("返回 ReadingMediaPayload", "ReadingMediaPayload" in shell and "base64_encode(&bytes)" in shell)
    check("前端 readingRemoteMedia", "readingRemoteMedia(url: string)" in api and '"reading_remote_media"' in api)
    check("MediaBlock 远程图片走命令并缓存", "isRemoteMediaSrc" in page and "readingRemoteMedia(src)" in page and "remote:${src}" in page)
    check("图片失败文案", "图片读取失败：" in page)

    check("完成判定 0.995", "const READING_COMPLETE = 0.995" in dash and dash.count("READING_COMPLETE") >= 3)
    check("今年已读同口径", "item.progress_percent >= READING_COMPLETE && isSameYear" in dash)
    check("目标条窄时隐藏条内文字", "goalWidth < 12" in dash and "goalCompact ? null" in dash and 'className="reading-dash-goal-below"' in dash)
    check("目标条下方提示样式", ".reading-dash-goal-below" in css)
    dash_block = re.search(r"\.reading-dash \{([^}]*)\}", css)
    check("仪表盘容器可滚动且 min-height 0", bool(dash_block) and "overflow-y: auto" in dash_block.group(1) and "min-height: 0" in dash_block.group(1))
    chart_svg = re.search(r"\.reading-dash-chart svg \{([^}]*)\}", css)
    check("花瓣图高度自适应不截断", bool(chart_svg) and "height: auto" in chart_svg.group(1) and "aspect-ratio: 470 / 350" in chart_svg.group(1) and "min-height: 360px" not in css)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
