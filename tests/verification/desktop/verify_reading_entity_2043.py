#!/usr/bin/env python3
"""OD-HARDEN-2043 任务 E-2：docx / epub 实体膨胀防护的回归。

构造样本全部是合成数据，写在临时目录里；不读任何用户文件。
"""

import json
import pathlib
import subprocess
import sys
import tempfile
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts" / "reading_attachment_extract.py"

FAILS = []
TOTAL = 0

WORD_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"""


def check(name: str, ok: bool, detail: str = "") -> None:
    global TOTAL
    TOTAL += 1
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def run_extract(path: pathlib.Path, fmt: str) -> dict:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--format", fmt, str(path)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if proc.returncode != 0:
        return {"kind": "error", "reason": proc.stderr.strip()}
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        return {"kind": "error", "reason": proc.stdout.strip()}


def document_xml(body: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="' + WORD_NS + '"><w:body>' + body + "</w:body></w:document>"
    )


def write_docx(path: pathlib.Path, xml: str) -> None:
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("[Content_Types].xml", CONTENT_TYPES)
        archive.writestr("word/document.xml", xml)


def write_epub(path: pathlib.Path, chapter: str) -> None:
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("mimetype", "application/epub+zip", compress_type=zipfile.ZIP_STORED)
        archive.writestr(
            "META-INF/container.xml",
            '<?xml version="1.0"?><container version="1.0" '
            'xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
            '<rootfiles><rootfile full-path="OEBPS/content.opf" '
            'media-type="application/oebps-package+xml"/></rootfiles></container>',
        )
        archive.writestr(
            "OEBPS/content.opf",
            '<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0">'
            '<manifest><item id="c1" href="chapter1.xhtml" '
            'media-type="application/xhtml+xml"/></manifest>'
            '<spine><itemref idref="c1"/></spine></package>',
        )
        archive.writestr("OEBPS/chapter1.xhtml", chapter)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="omnidesk-entity-") as tmp:
        tmp_dir = pathlib.Path(tmp)

        normal_docx = tmp_dir / "normal.docx"
        write_docx(
            normal_docx,
            document_xml("<w:p><w:r><w:t>正常段落一</w:t></w:r></w:p>"),
        )
        normal = run_extract(normal_docx, "docx")
        check("docx_normal_text", normal.get("kind") == "text", str(normal)[:120])
        check("docx_normal_content", "正常段落一" in normal.get("content", ""))

        evil_docx = tmp_dir / "evil.docx"
        # 实体声明刻意放在 4 KB 之后：旧的「只看前 4 KB」写法会漏掉它。
        padding = "<!--" + ("x" * 5000) + "-->"
        evil_xml = (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            + padding
            + '<!DOCTYPE w:document [<!ENTITY boom "BOOM">]>'
            + '<w:document xmlns:w="' + WORD_NS + '"><w:body><w:p><w:r><w:t>&boom;</w:t>'
            + "</w:r></w:p></w:body></w:document>"
        )
        write_docx(evil_docx, evil_xml)
        evil = run_extract(evil_docx, "docx")
        check("docx_entity_rejected", evil.get("kind") == "metadata", str(evil)[:120])
        check("docx_entity_reason", "实体" in evil.get("reason", ""), str(evil)[:120])

        normal_epub = tmp_dir / "normal.epub"
        write_epub(
            normal_epub,
            '<?xml version="1.0" encoding="utf-8"?>'
            "<!DOCTYPE html>"
            '<html xmlns="http://www.w3.org/1999/xhtml"><body>'
            "<h1>第一章</h1><p>正常 epub 正文。</p></body></html>",
        )
        epub_normal = run_extract(normal_epub, "epub")
        check("epub_doctype_allowed", epub_normal.get("kind") == "text", str(epub_normal)[:120])
        check("epub_normal_content", "正常 epub 正文" in epub_normal.get("content", ""))

        evil_epub = tmp_dir / "evil.epub"
        write_epub(
            evil_epub,
            '<?xml version="1.0" encoding="utf-8"?>'
            '<!DOCTYPE html [<!ENTITY boom "BOOM">]>'
            '<html xmlns="http://www.w3.org/1999/xhtml"><body><p>&boom;</p></body></html>',
        )
        epub_evil = run_extract(evil_epub, "epub")
        check("epub_entity_rejected", epub_evil.get("kind") == "metadata", str(epub_evil)[:120])

    print(f"cases={TOTAL} failures={len(FAILS)}")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
