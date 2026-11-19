#!/usr/bin/env python3
"""OD-READ-002 阶段 9 静态核验：先显示后回填 + 仪表盘内凹凹槽线。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CSS = ROOT / "desktop" / "src" / "reading.css"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
DASH = ROOT / "desktop" / "src" / "shell" / "ReadingDashboard.tsx"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def block_after(text: str, marker: str, end_marker: str = "}") -> str:
    if marker not in text:
        return ""
    tail = text.split(marker, 1)[1]
    return tail.split(end_marker, 1)[0] if end_marker in tail else tail


def main() -> int:
    css = CSS.read_text()
    lib = LIB.read_text()
    page = PAGE.read_text()
    dash = DASH.read_text()

    metrics = block_after(css, ".reading-dash-metrics {")
    check("中层数据区整体凸起", "box-shadow: var(--rd-raise);" in metrics, metrics)
    check("中层圆角卡", "border-radius: 20px;" in metrics, metrics)
    check("中层内边距", "padding: 24px 26px;" in metrics, metrics)
    check("旧分割线元素已移除", 'className="reading-dash-divider"' not in dash)
    check("旧分割线样式已移除", ".reading-dash-divider {" not in css)

    command = block_after(lib, "async fn rss_refresh_article(", "#[tauri::command")
    check("回填命令为 async", "async fn rss_refresh_article(" in lib)
    check("回填走 spawn_blocking", "spawn_blocking(move || rss_store::refresh_article(&paths, &book_id))" in command, command)
    check("回填等待后台任务", ".await" in command, command)
    check("回填错误正常返回", 'map_err(|e| format!("RSS 文章回填任务异常退出：{e}"))?' in command, command)
    check("回填命令仍已注册", "rss_refresh_article," in lib.split("invoke_handler", 1)[-1])

    open_book = page.split("const openBook = useCallback(", 1)[1].split("[book, refreshLibrary, showToast]", 1)[0]
    initial_set = open_book.find("setBook(loaded);")
    refresh_call = open_book.find("shell.rssRefreshArticle(id)")
    check("初次 readingBook 后立即 setBook", initial_set >= 0)
    check("先显示正文再触发回填", 0 <= initial_set < refresh_call, f"{initial_set} < {refresh_call}")
    launch = open_book[max(0, refresh_call - 200):refresh_call]
    check("回填以独立异步任务启动", "void (async () => {" in launch, launch)
    check("回填期间不占 busy", open_book.find("})();") < open_book.find("setBookBusy(false)"), open_book)
    check("回填期间有小提示", 'showToast("正在补齐文章图片…")' in open_book)
    check("回填成功更新正文", "loaded = await shell.readingBook(id);" in open_book and "setBook(loaded);" in open_book[refresh_call:])
    check("回填成功保持滚动", "scrollRef.current.scrollTop = scrollTop" in open_book and "requestAnimationFrame" in open_book)
    check("回填成功保持进度", "progress: { ...loaded.progress, percent: currentPercent }" in open_book)
    check("回填成功保持批注上下文", not any(
        token in open_book[refresh_call:]
        for token in ("setSelection(null)", "setSelectionPos(null)", "setActiveAnnotation(null)", "setComposer(null)")
    ))
    check("回填失败提示原因", '旧图片回填失败：' in open_book)
    check("回填未完成提示原因", '旧图片回填未完成：' in open_book)
    check("一次标记防循环", "rssRefreshAttempted.current.add(id)" in open_book and "!rssRefreshAttempted.current.has(id)" in open_book)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
