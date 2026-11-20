#!/usr/bin/env python3
"""OD-READ-002 阶段 11 静态核验：正文阅读设置与 RSS 文末模块。"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
TYPES = ROOT / "desktop" / "src" / "types.ts"
MAIN = ROOT / "desktop" / "src" / "main.tsx"
PKG = ROOT / "desktop" / "package.json"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
ICON = ROOT / "desktop" / "src" / "components" / "Icon.tsx"

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
    tail = source[index + len(start):]
    stop = tail.find(end)
    return tail if stop < 0 else tail[:stop]


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    css = CSS.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    main_tsx = MAIN.read_text(encoding="utf-8")
    package = json.loads(PKG.read_text(encoding="utf-8"))
    rss = RSS.read_text(encoding="utf-8")
    icon = ICON.read_text(encoding="utf-8")

    # 1. 字体随包自托管，运行时无外链。
    deps = package.get("dependencies", {})
    check("依赖含思源黑体", "@fontsource-variable/noto-sans-sc" in deps)
    check("依赖含思源宋体", "@fontsource-variable/noto-serif-sc" in deps)
    check("依赖含霞鹜文楷", "lxgw-wenkai-webfont" in deps)
    check("main.tsx 引入三款字体", "@fontsource-variable/noto-sans-sc" in main_tsx and "@fontsource-variable/noto-serif-sc" in main_tsx and "lxgw-wenkai-webfont" in main_tsx)

    # 2. 阅读设置：字体、字号、排列，仅正文。
    check("字体选项完整", all(label in page for label in ["系统默认", "思源黑体", "思源宋体", "霞鹜文楷"]))
    check("字体栈含 Noto 与 LXGW", '"Noto Sans SC Variable"' in page and '"Noto Serif SC Variable"' in page and '"LXGW WenKai"' in page)
    check("字号范围 14..26", "READER_MIN_SIZE = 14" in page and "READER_MAX_SIZE = 26" in page)
    check("三种排列方式", all(item in page for item in ["align-left", "align-justify", "align-right"]))
    check("设置持久化到 localStorage", "omnidesk.reading.settings.v1" in page and "window.localStorage.setItem" in page)
    check("顶栏 Aa 设置入口", 'data-testid="reading-settings-toggle"' in page and "阅读设置" in page)
    check("恢复默认", 'data-testid="reading-settings-reset"' in page and "恢复默认" in page)
    check("设置不含宽度/颜色", "宽度" not in block(page, "reading-settings-popover", "<button") and "颜色" not in block(page, "reading-settings-popover", "<button"))
    check("设置只落到正文变量", "--rd-reader-font" in page and "--rd-reader-size" in page and "--rd-reader-align" in page)
    paragraph_rule = block(css, ".reading-page .reading-paragraph {", "}")
    check("正文规则使用字体变量", "font-family: var(--rd-reader-font" in paragraph_rule)
    check("正文规则使用字号变量", "font-size: var(--rd-reader-size" in paragraph_rule)
    check("正文规则使用对齐变量", "text-align: var(--rd-reader-align" in paragraph_rule)
    title_rule = block(css, ".reading-page .reading-chapter-title {", "}")
    check("章节标题不跟随阅读设置", "--rd-reader" not in title_rule)
    check("窄屏不再强制左对齐", "@media (max-width: 640px)" not in css or "text-align: left" not in block(css, "@media (max-width: 640px)", "}"))
    check("对齐图标已注册", all(name in icon for name in ["align-left", "align-justify", "align-right"]))

    # 3. RSS 附件模型与解析。
    check("前端附件类型", "export interface RssAttachment" in types and "attachments?: RssAttachment[]" in types)
    check("Rust 附件类型", "pub struct RssAttachment" in rss and "pub attachments: Vec<RssAttachment>" in rss)
    check("旧元数据缺省附件", re.search(r"serde\(default\)\]\s*pub attachments: Vec<RssAttachment>", rss) is not None)
    check("解析 enclosure 与 media:content", "fn entry_attachments" in rss and 'child.name != "enclosure" && child.name != "content"' in rss)
    check("附件跳过图片", "lower.starts_with(\"image/\")" in rss and "medium == \"image\"" in rss)
    check("附件只收 http(s)", "starts_with(\"http\")" in block(rss, "fn entry_attachments", "fn attachment_mime"))
    check("旧文章补附件元数据", "write_article_meta_at" in rss and "meta.attachments = entry.attachments.clone()" in rss)
    check("附件上限 20 条", "attachments.len() >= 20" in rss)

    # 4. RSS 文末模块，仅订阅文章显示。
    footer_cond = block(page, "{book.source_format === \"rss\" ? (", "</footer>")
    check("文末模块仅 RSS", bool(footer_cond))
    check("附件模块", 'data-testid="rss-article-attachments"' in footer_cond and "AttachmentItem" in footer_cond)
    check("结束标记", 'data-testid="rss-end-mark"' in footer_cond and "<span>完</span>" in footer_cond)
    check("预计阅读时间", 'data-testid="rss-read-time"' in footer_cond and "分钟读完" in footer_cond)
    check("下一篇文章标题", 'data-testid="rss-next-article"' in footer_cond and "reading-next-title" in footer_cond)
    check("阅读时间按字数估算", "function estimatedReadingMinutes" in page and "Math.ceil(chars / 400)" in page)
    check("下一篇同源且更早", "item.subscription_id === current.subscription_id" in page and "stamp(item) < currentStamp" in page)
    check("下一篇文章可打开", "openBook(rssNextArticle.book_id)" in page)
    check("本地书不渲染文末模块", "book.source_format === \"rss\" ? (\n                    <footer" in page)
    check("附件可打开原文件", "打开原文件" in page and "openExternalUrl(attachment.url)" in page)
    check("原文链接", 'data-testid="rss-article-source-link"' in page and "打开原文" in page)
    check("音频/视频附件复用 MediaBlock", "kind === \"audio\" || kind === \"video\"" in page and "<MediaBlock bookId={bookId} kind={kind}" in page)

    # 5. 样式
    check("设置弹层样式", ".reading-settings-popover" in css and ".reading-settings-option.is-active" in css)
    check("文末结束线样式", ".reading-end-mark::before" in css and ".reading-end-mark::after" in css)
    check("下一篇文章卡样式", ".reading-next-article" in css and ".reading-next-title" in css)
    check("附件井样式", ".reading-article-attachments" in css and ".reading-attachment-open" in css)

    print(f"cases={len(CASES)} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
