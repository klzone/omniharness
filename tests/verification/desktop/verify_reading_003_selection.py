#!/usr/bin/env python3
"""OD-READ-003：选区浮条、首页统计、左栏分页与富媒体标记的静态/脚本验证。"""

import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
HOME = ROOT / "desktop" / "src" / "shell" / "HomeCanvas.tsx"
STORE = ROOT / "desktop" / "src-tauri" / "src" / "reading_store.rs"
HELPER = ROOT / "scripts" / "reading_attachment_extract.py"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def make_epub(base: Path) -> Path:
    target = base / "media.epub"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("mimetype", "application/epub+zip")
        z.writestr(
            "META-INF/container.xml",
            '<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
            '<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>'
            "</rootfiles></container>",
        )
        z.writestr(
            "OEBPS/content.opf",
            '<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf">'
            '<manifest><item id="c1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>'
            '<item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>'
            '<item id="pic" href="images/pic.png" media-type="image/png"/></manifest>'
            '<spine><itemref idref="c1"/></spine></package>',
        )
        z.writestr(
            "OEBPS/text/ch1.xhtml",
            '<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>'
            "<h1>第一章</h1><p>前文<img src=\"../images/pic.png\"/>后文</p></body></html>",
        )
        z.writestr("OEBPS/images/pic.png", b"\x89PNG\r\n")
        z.writestr("OEBPS/images/cover.jpg", b"\xff\xd8\xff\xe0cover")
    return target


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    store = STORE.read_text(encoding="utf-8")
    helper = HELPER.read_text(encoding="utf-8")

    check("paragraph_anchor_attribute", "data-paragraph" in page and "data-chapter-index" in page)
    check("selection_ref_shared", "readSelectionRef.current" in page)
    check("selectionchange_channel", 'document.addEventListener("selectionchange"' in page)
    check("selectionchange_debounce_150", "150" in page and "selectionTimer" in page)
    check("selection_bar_render_condition", "selection && selectionPos" in page and "reading-selection-bar" in page)
    check("selection_bar_mousedown_prevent", "onMouseDown={(event) => event.preventDefault()}" in page)
    check("cross_paragraph_message", "跨段选择暂不支持" in page)

    check("home_stats_bar", 'data-testid="home-reading-stats"' in home and "shell" in home)
    check("home_stats_command", "readingStats()" in home)
    check("home_stats_error_not_zero", "阅读统计读取失败" in home)
    check("shelf_tabs", 'reading-shelf-tab-books' in page and 'reading-shelf-tab-rss' in page)
    check("shelf_tab_labels", "本地书籍" in page and "订阅文章" in page)
    check("rss_group_collapse", "collapsedSources" in page and "rss-source-group" in page)
    check("book_cover_component", "readingCoverImage" in page and "reading-cover" in page)
    check("media_marker_kept_for_offsets", "reading-media-anchor" in page and "token.raw" in page)
    check("inline_markdown_kept_for_offsets", "reading-syntax-anchor" in page)

    check("stats_thought_kind", "KIND_THOUGHT" in store and "thoughts" in store)
    check("stats_completed_threshold", "0.995" in store)
    check("stats_excludes_rss_books", 'meta.source_format != "rss"' in store)
    check("media_marker_img", "![img]" in helper and "odmedia:" in helper)
    check("media_marker_audio_video", "[audio]" in helper or "_media_marker" in helper)
    check("media_path_guard", '".."' in helper)

    base = Path(tempfile.mkdtemp(prefix="od-reading-003-"))
    epub = make_epub(base)
    proc = subprocess.run(
        [sys.executable, str(HELPER), str(epub), "--format", "epub"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    check("epub_helper_exit", proc.returncode == 0, proc.stderr[-400:])
    payload = {}
    try:
        payload = json.loads((proc.stdout.strip().splitlines() or ["{}"])[-1])
    except json.JSONDecodeError:
        pass
    chapters = payload.get("chapters") or []
    body = chapters[0].get("content", "") if chapters else ""
    check("epub_media_marker", "![img](odmedia:OEBPS/images/pic.png)" in body, body[:200])
    check("epub_cover_payload", isinstance(payload.get("cover"), dict) and payload["cover"].get("mime") == "image/jpeg")

    print(f"cases={25} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
