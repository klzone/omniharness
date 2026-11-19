#!/usr/bin/env python3
"""OD-READ-002 阶段 3 静态核验：右侧栏折叠图标栏/90 秒收起、首页统计条居中、③④回归点。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
HOME = ROOT / "desktop" / "src" / "shell" / "HomeCanvas.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
STORE = ROOT / "desktop" / "src-tauri" / "src" / "reading_store.rs"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    page = PAGE.read_text()
    home = HOME.read_text()
    css = CSS.read_text()
    rss = RSS.read_text()
    store = STORE.read_text()

    check("panelOpen 初始为 false", "const [panelOpen, setPanelOpen] = useState(false)" in page)
    check("不再按视口宽度决定初始展开", "window.innerWidth >= 1280" not in page)
    check("自动收起为 90 秒", "PANEL_AUTO_CLOSE_MS = 90_000" in page)
    check(
        "面板四类交互均重置计时器",
        'const events = ["click", "keydown", "scroll", "pointerdown"] as const' in page
        and "element.addEventListener(event, activity, true)" in page,
    )
    check("收起态渲染图标栏", 'data-testid="reading-panel-rail"' in page)
    check("图标栏含笔记与 AI 入口", 'aria-label={`展开笔记面板' in page and 'aria-label="展开 AI 面板"' in page)
    check("批注角标上限显示 99+", 'book.annotations.length > 99 ? "99+"' in page)
    check("收起态不再 hidden", ': "hidden"' not in page)
    check("切书时收起右侧栏", "setPanelOpen(false)" in page and "切书时右侧栏回到折叠态" in page)
    check(
        "划线动作条仍展开 notes/AI",
        page.count("setPanelOpen(true)") >= 4
        and 'setSideTab("notes")' in page
        and 'setSideTab("ai")' in page,
    )
    check("面板宽度有过渡", "transition-[width] duration-200 ease-out" in page)

    check("首页统计槽位 shrink-0 + 居中", 'className="shrink-0 flex justify-center pt-4"' in home)
    check("统计条按内容收缩并居中", "width: fit-content" in css and "margin: 0.5rem auto 0.75rem" in css)
    check("统计含字数项", 'label: "字数"' in home)
    check("统计条仍可跳转健脑室", 'onClick={() => onNavigate("reading")}' in home)

    check("③ 图片转 Markdown 标记仍在", "![img]({})" in rss)
    check("③ 富媒体标记不破坏段落偏移测试", "media_marker_img" in (ROOT / "tests/verification/desktop/verify_reading_003_selection.py").read_text())
    check("④ RSS 字数聚合单测仍在", "assert_eq!(stats.chars, imported.book.char_count + rss.book.char_count);" in store)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
