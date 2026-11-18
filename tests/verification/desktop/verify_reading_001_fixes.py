#!/usr/bin/env python3
"""OD-READ-001 回归：EPUB DOCTYPE/div 正文、Windows 无控制台、选区工具栏与 AI 网络错误提示。"""

import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HELPER = ROOT / "scripts" / "reading_attachment_extract.py"
TASK_SHELL = ROOT / "desktop" / "src-tauri" / "src" / "task_shell.rs"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def make_epub(base: Path) -> Path:
    target = base / "doctype.epub"
    with zipfile.ZipFile(target, "w") as z:
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
            '<item id="c2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/></manifest>'
            '<spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>',
        )
        # 第一章：真实 EPUB 常见的 DOCTYPE + `<p>`。
        z.writestr(
            "OEBPS/text/ch1.xhtml",
            '<?xml version="1.0" encoding="utf-8"?>'
            '<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">'
            '<html xmlns="http://www.w3.org/1999/xhtml"><body><h1>第一章</h1>'
            "<p>第一段正文。</p><p>第二段正文。</p></body></html>",
        )
        # 第二章：没有 <p>，章节名与正文都在 <div> 里。
        z.writestr(
            "OEBPS/text/ch2.xhtml",
            '<?xml version="1.0" encoding="utf-8"?><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml">'
            "<body><div>第二章</div><div>只有 div 的正文一。</div><div>只有 div 的正文二。</div></body></html>",
        )
    return target


def main() -> int:
    base = Path(tempfile.mkdtemp(prefix="od-reading-001-"))
    epub = make_epub(base)
    proc = subprocess.run(
        [sys.executable, str(HELPER), str(epub), "--format", "epub"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    check("epub_helper_exit", proc.returncode == 0, proc.stderr[-500:])
    payload = {}
    try:
        payload = json.loads((proc.stdout.strip().splitlines() or ["{}"])[-1])
    except json.JSONDecodeError:
        pass
    check("epub_text_kind", payload.get("kind") == "text", str(payload)[:300])
    chapters = payload.get("chapters") or []
    check("epub_doctype_chapter", len(chapters) >= 1 and chapters[0].get("title") == "第一章", str(chapters)[:300])
    check("epub_div_chapter", len(chapters) >= 2 and "只有 div 的正文一" in chapters[1].get("content", ""), str(chapters)[:300])

    shell = TASK_SHELL.read_text(encoding="utf-8")
    check("windows_no_console", "CREATE_NO_WINDOW" in shell and "creation_flags" in shell)

    page = PAGE.read_text(encoding="utf-8")
    check("selection_document_listener", 'document.addEventListener("mouseup"' in page)
    check("selection_bar_exists", "reading-selection-bar" in page)
    check("highlight_opens_notes", page.count('setSideTab("notes")') >= 2)
    check("ai_dns_hint", "getaddrinfo/ENOTFOUND" in page or "getaddrinfo" in page)

    print(f"cases={9} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
