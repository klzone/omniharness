#!/usr/bin/env python3
"""OD-READ-002 阶段 4 静态核验：暖纸新拟物变量、纸面、书架 see-saw、侧栏与浮动条。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CSS = ROOT / "desktop" / "src" / "reading.css"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    css = CSS.read_text()
    page = PAGE.read_text()

    for token in ("--rd-raise:", "--rd-raise-sm:", "--rd-inset:", "--rd-inset-sm:", "--rd-ring:"):
        check(f"设计变量 {token}", token in css)
    check("变量作用域在 .reading-page", ".reading-page {" in css and "--rd-raise" in css)
    check("阅读纸面类存在", "reading-paper" in page and ".reading-page .reading-paper {" in css)
    check("纸面 26px 大圆角", "border-radius: 26px" in css)
    check("纸面 34/44 内边距", "padding: 34px 44px 30px" in css)
    check("正文宋体栈", '"Songti SC", "Noto Serif SC"' in css)
    check(
        "正文沿用字体/字号/对齐变量",
        "font-family: var(--rd-reader-font" in css
        and "font-size: var(--rd-reader-size" in css
        and "text-align: var(--rd-reader-align" in css,
    )
    check("划线琥珀软底", "rgba(212, 164, 74, 0.4)" in css)
    check("想法主色下划线", "reading-mark-thought" in css and "border-bottom: 1.5px solid" in css)
    check("媒体内凹窗 18px", ".reading-page .reading-media-figure" in css and "box-shadow: var(--rd-inset)" in css and "border-radius: 18px" in css)
    check("RSS 引文井 + 主色左缘", ".reading-page .reading-quote" in css and "border-left: 3px solid var(--color-primary)" in css)
    check("章节头进度轨内凹", "reading-header-progress" in page and ".reading-page .reading-header-progress" in css and "--rd-inset-sm" in css)
    check("书架 288px 展开宽度", 'w-[288px]' in page)
    check("书架 56px 折叠宽度", 'w-14' in page and "reading-shelf-is-rail" in page)
    check("书架 see-saw 220ms 过渡", "transition: width var(--rd-speed) ease" in css and "--rd-speed: 220ms" in css)
    check("书架三枚圆钮", "reading-shelf-rail-button" in page and page.count("reading-shelf-rail-button") >= 3)
    check("书架圆钮 aria-label", 'aria-label="展开书架并切到本地书籍"' in page and 'aria-label="展开书架并搜索"' in page)
    check("搜索胶囊内凹", 'data-testid="reading-shelf-search"' in page and ".reading-page .reading-shelf-search" in css and "box-shadow: var(--rd-inset)" in css)
    check("书架卡 --rd-raise-sm", ".reading-page .reading-shelf-item" in css and "box-shadow: var(--rd-raise-sm)" in css)
    check("选中卡 --rd-inset + --rd-ring", ".reading-page .reading-shelf-item.is-active" in css and "var(--rd-ring)" in css)
    check("进度轨内凹 + 主色填充", ".reading-page .reading-shelf-progress" in css and "--rd-inset-sm" in css and ".reading-shelf-progress-fill" in css)
    check("右侧面板默认 320", "const [panelWidth, setPanelWidth] = useState(320)" in page)
    check("右侧面板卡 --rd-raise", ".reading-page .reading-side-panel" in css and "box-shadow: var(--rd-raise)" in css)
    check("面板 tabs 内凹/选中凸起", ".reading-page .reading-panel-tabs" in css and ".reading-page .reading-panel-tab.is-active" in css)
    check("批注卡/AI 井内凹", ".reading-page .reading-note-card" in css and ".reading-page .reading-ai-well" in css)
    check("面板收起 40px 图标栏", 'w-10' in page and 'data-testid="reading-panel-rail"' in page)
    check("面板 rail 圆钮样式", ".reading-panel-rail-button" in css and "--rd-raise-sm" in css)
    check("浮动条 14px raised 胶囊", ".reading-page .reading-selection-bar" in css and "border-radius: 14px" in css and "--rd-raise-sm" in css)
    check("浮动条 hover/active 与主色", ".reading-selection-bar button:hover" in css and ".reading-selection-bar button:active" in css and "color: var(--color-primary)" in css)
    check("划线锚点结构未改", "data-paragraph" in page and "data-chapter-index" in page and "data-paragraph-index" in page)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
