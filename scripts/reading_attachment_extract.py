#!/usr/bin/env python3
"""附件正文提取 helper（OD-DOCX-148b / OD-READ-001）。

stdin 或参数接收文件路径与格式，stdout 输出一行 JSON：
  {"kind": "text", "content": "..."}
  {"kind": "text", "content": "...", "chapters": [{"title": "...", "content": "..."}]}
  {"kind": "metadata", "reason": "..."}

docx / epub 只用标准库（zipfile + ElementTree），不依赖 python-docx；pdf 仅在
pypdf 可导入时逐页提取，否则如实降级为 metadata。纯文本按 UTF-8 / UTF-16 /
GB18030 顺序尝试解码。本脚本只读文件，不写盘，不联网，不解析外部实体。

用法：
  python3 scripts/reading_attachment_extract.py <path> [--format docx|pdf|epub|txt|md]
  echo '{"path":"...","format":"docx"}' | python3 scripts/reading_attachment_extract.py
"""

from __future__ import annotations

import json
import os
import posixpath
import re
import sys
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any, Optional
from urllib.parse import unquote

MAX_FILE_BYTES = 50 * 1024 * 1024
MAX_DOCX_UNCOMPRESSED = 200 * 1024 * 1024
MAX_DOCUMENT_XML = 80 * 1024 * 1024
MAX_EPUB_TOTAL_UNCOMPRESSED = 200 * 1024 * 1024
MAX_EPUB_DOCS = 600
MAX_EPUB_TEXT_CHARS = 12_000_000
WORD_NS = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def _limit(name: str, fallback: int) -> int:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return fallback
    try:
        value = int(raw)
    except ValueError:
        return fallback
    return value if value > 0 else fallback


def _vendor_path() -> Optional[Path]:
    """把随包 python-vendor 放到 sys.path 首位；不存在时返回 None。"""
    candidates = []
    env = os.environ.get("OMNIDESK_PYTHON_VENDOR", "").strip()
    if env:
        candidates.append(Path(env))
    here = Path(__file__).resolve()
    candidates.append(here.parent.parent / "runtime" / "python-vendor")
    for candidate in candidates:
        if candidate.is_dir():
            text = str(candidate)
            if text not in sys.path:
                sys.path.insert(0, text)
            return candidate
    return None


def _metadata(reason: str) -> dict[str, Any]:
    return {"kind": "metadata", "reason": reason}


def _text(content: str, chapters: Optional[list[dict[str, str]]] = None) -> dict[str, Any]:
    payload: dict[str, Any] = {"kind": "text", "content": content}
    if chapters:
        payload["chapters"] = chapters
    return payload


def _format_of(path: Path, explicit: Optional[str]) -> str:
    if explicit:
        return explicit.strip().lower().lstrip(".")
    suffix = path.suffix.lower().lstrip(".")
    if suffix in ("docx", "docm"):
        return "docx"
    if suffix in ("md", "markdown"):
        return "md"
    if suffix in ("txt", "text"):
        return "txt"
    return suffix


def _paragraph_text(paragraph: ET.Element) -> str:
    pieces: list[str] = []
    for node in paragraph.iter():
        if node.tag == WORD_NS + "t" and node.text:
            pieces.append(node.text)
        elif node.tag == WORD_NS + "tab":
            pieces.append("\t")
        elif node.tag in (WORD_NS + "br", WORD_NS + "cr"):
            pieces.append("\n")
    return "".join(pieces).strip()


def _table_text(table: ET.Element) -> str:
    rows: list[str] = []
    for row in table.iter(WORD_NS + "tr"):
        cells: list[str] = []
        for cell in row.iter(WORD_NS + "tc"):
            parts = [
                _paragraph_text(paragraph)
                for paragraph in cell.iter(WORD_NS + "p")
            ]
            cells.append(" ".join(part for part in parts if part))
        if cells:
            rows.append("\t".join(cells))
    return "\n".join(rows)


def extract_docx(path: Path) -> dict[str, Any]:
    max_file = _limit("OMNIDESK_ATTACHMENT_MAX_BYTES", MAX_FILE_BYTES)
    max_total = _limit("OMNIDESK_DOCX_MAX_UNCOMPRESSED", MAX_DOCX_UNCOMPRESSED)
    max_xml = _limit("OMNIDESK_DOCX_MAX_DOCUMENT_XML", MAX_DOCUMENT_XML)
    try:
        if path.stat().st_size > max_file:
            return _metadata("docx 超过附件大小上限，正文未提取")
    except OSError as exc:
        return _metadata(f"读取 docx 失败：{exc}")
    try:
        with zipfile.ZipFile(path) as archive:
            infos = archive.infolist()
            total = sum(item.file_size for item in infos)
            if total > max_total:
                return _metadata("docx 解压后总量超限，正文未提取")
            names = {item.filename for item in infos}
            if "word/document.xml" not in names:
                return _metadata("docx 缺少 word/document.xml")
            info = archive.getinfo("word/document.xml")
            if info.file_size > max_xml:
                return _metadata("docx 正文 XML 超限，正文未提取")
            raw = archive.read(info)
    except (zipfile.BadZipFile, OSError, RuntimeError) as exc:
        return _metadata(f"docx 解压失败：{exc}")

    head = raw[:4096].upper()
    if b"<!DOCTYPE" in head or b"<!ENTITY" in head:
        return _metadata("docx 正文含 DOCTYPE/ENTITY，拒绝解析")
    try:
        root = ET.fromstring(raw)
    except ET.ParseError as exc:
        return _metadata(f"docx 正文 XML 解析失败：{exc}")

    body = root.find(WORD_NS + "body")
    if body is None:
        return _metadata("docx 缺少 body")
    blocks: list[str] = []
    for child in body:
        if child.tag == WORD_NS + "p":
            text = _paragraph_text(child)
            if text:
                blocks.append(text)
        elif child.tag == WORD_NS + "tbl":
            text = _table_text(child)
            if text:
                blocks.append(text)
    content = "\n".join(blocks).strip()
    if not content:
        return _metadata("docx 没有可提取的正文")
    return _text(content)


def _decode_text_bytes(raw: bytes) -> Optional[str]:
    """按常见中文纯文本编码尝试解码；全部失败时返回 None，不猜内容。"""
    if raw.startswith(b"\xef\xbb\xbf"):
        raw = raw[3:]
    if raw.startswith(b"\xff\xfe") or raw.startswith(b"\xfe\xff"):
        try:
            return raw.decode("utf-16")
        except UnicodeDecodeError:
            pass
    for encoding in ("utf-8", "gb18030", "big5"):
        try:
            return raw.decode(encoding)
        except (UnicodeDecodeError, LookupError):
            continue
    return None


def extract_plain_text(path: Path) -> dict[str, Any]:
    max_file = _limit("OMNIDESK_READING_MAX_BYTES", 32 * 1024 * 1024)
    try:
        if path.stat().st_size > max_file:
            return _metadata("纯文本超过阅读导入大小上限，正文未提取")
        raw = path.read_bytes()
    except OSError as exc:
        return _metadata(f"读取纯文本失败：{exc}")
    text = _decode_text_bytes(raw)
    if text is None:
        return _metadata("纯文本无法按 UTF-8 / UTF-16 / GB18030 解码")
    text = text.replace("\r\n", "\n").replace("\r", "\n").strip()
    if not text:
        return _metadata("纯文本没有可读的正文")
    return _text(text)


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1].lower()


def _safe_zip_name(name: str) -> Optional[str]:
    name = unquote(name).replace("\\", "/").lstrip("/")
    if not name or ".." in name.split("/"):
        return None
    return name


def _parse_xml(raw: bytes) -> Optional[ET.Element]:
    head = raw[:4096].upper()
    if b"<!DOCTYPE" in head or b"<!ENTITY" in head:
        return None
    try:
        return ET.fromstring(raw)
    except ET.ParseError:
        return None


_HTML_BLOCK_TAGS = {
    "p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "blockquote", "td", "pre",
}


def _html_title(root: ET.Element) -> Optional[str]:
    for node in root.iter():
        if _local_name(node.tag) in ("h1", "h2", "h3"):
            title = re.sub(r"\s+", " ", "".join(node.itertext())).strip()
            if title:
                return title
    return None


def _html_blocks(root: ET.Element) -> list[str]:
    """按常见块级节点取正文；跳过 script/style/svg，避免把样式或矢量路径当正文。"""
    blocks: list[str] = []

    def walk(node: ET.Element) -> None:
        tag = _local_name(node.tag)
        if tag in ("script", "style", "svg", "head"):
            return
        if tag in _HTML_BLOCK_TAGS:
            # 只取最外层块，避免 <li><p>…</p></li> 重复。
            text = re.sub(r"\s+", " ", "".join(node.itertext())).strip()
            if text:
                blocks.append(text)
            return
        for child in list(node):
            walk(child)

    walk(root)
    return blocks


def extract_epub(path: Path) -> dict[str, Any]:
    max_file = _limit("OMNIDESK_READING_MAX_BYTES", 32 * 1024 * 1024)
    max_total = _limit("OMNIDESK_EPUB_MAX_UNCOMPRESSED", MAX_EPUB_TOTAL_UNCOMPRESSED)
    try:
        if path.stat().st_size > max_file:
            return _metadata("EPUB 超过阅读导入大小上限，正文未提取")
        archive = zipfile.ZipFile(path)
    except (zipfile.BadZipFile, OSError) as exc:
        return _metadata(f"EPUB 解压失败：{exc}")
    with archive:
        infos = archive.infolist()
        total = sum(item.file_size for item in infos)
        if total > max_total:
            return _metadata("EPUB 解压后总量超限，正文未提取")
        names = {item.filename: item for item in infos}

        def read_entry(name: str) -> Optional[bytes]:
            safe = _safe_zip_name(name)
            if safe is None or safe not in names:
                return None
            info = names[safe]
            if info.file_size > MAX_DOCUMENT_XML:
                return None
            try:
                return archive.read(info)
            except (KeyError, OSError, RuntimeError):
                return None

        container_raw = read_entry("META-INF/container.xml")
        rootfile: Optional[str] = None
        if container_raw is not None:
            container = _parse_xml(container_raw)
            if container is not None:
                for node in container.iter():
                    if _local_name(node.tag) == "rootfile" and node.get("full-path"):
                        rootfile = node.get("full-path")
                        break
        if rootfile is None:
            # 缺 container.xml 的窄兼容：找唯一的 .opf，仍只读包内文件。
            opfs = [n for n in names if n.lower().endswith(".opf")]
            if len(opfs) != 1:
                return _metadata("EPUB 缺少 container.xml，且无法唯一确定 OPF")
            rootfile = opfs[0]

        opf_raw = read_entry(rootfile)
        opf = _parse_xml(opf_raw) if opf_raw is not None else None
        if opf is None:
            return _metadata("EPUB OPF 无法解析")
        base = posixpath.dirname(_safe_zip_name(rootfile) or rootfile)
        manifest: dict[str, str] = {}
        for node in opf.iter():
            if _local_name(node.tag) == "item":
                item_id = node.get("id")
                href = node.get("href")
                media = (node.get("media-type") or "").lower()
                if item_id and href and media in ("application/xhtml+xml", "text/html"):
                    manifest[item_id] = posixpath.normpath(posixpath.join(base, unquote(href)))
        spine: list[str] = []
        for node in opf.iter():
            if _local_name(node.tag) == "itemref":
                idref = node.get("idref")
                if idref and idref in manifest:
                    spine.append(manifest[idref])
        if not spine:
            spine = sorted(
                n for n in names
                if n.lower().endswith((".xhtml", ".html", ".htm")) and _safe_zip_name(n) == n
            )
        if not spine:
            return _metadata("EPUB 没有可读的 XHTML 文档")

        chapters: list[dict[str, str]] = []
        total_chars = 0
        for name in spine[:MAX_EPUB_DOCS]:
            raw_doc = read_entry(name)
            doc = _parse_xml(raw_doc) if raw_doc is not None else None
            if doc is None:
                continue
            blocks = _html_blocks(doc)
            if not blocks:
                continue
            title = _html_title(doc) or posixpath.splitext(posixpath.basename(name))[0] or "正文"
            if blocks[0] == title:
                blocks = blocks[1:]
            content = "\n".join(blocks).strip()
            if not content:
                continue
            total_chars += len(content)
            if total_chars > MAX_EPUB_TEXT_CHARS:
                break
            chapters.append({"title": title, "content": content})
        if not chapters:
            return _metadata("EPUB 未提取到可读正文（可能是影印或仅图片）")
        content = "\n\n".join(f"# {ch['title']}\n\n{ch['content']}" for ch in chapters)
        return _text(content, chapters)


def extract_pdf(path: Path) -> dict[str, Any]:
    _vendor_path()
    try:
        import pypdf  # type: ignore
    except Exception:
        return _metadata("pypdf 不可导入，PDF 正文未提取")
    max_file = _limit("OMNIDESK_ATTACHMENT_MAX_BYTES", MAX_FILE_BYTES)
    try:
        if path.stat().st_size > max_file:
            return _metadata("PDF 超过附件大小上限，正文未提取")
        reader = pypdf.PdfReader(str(path))
        pages: list[str] = []
        for page in reader.pages:
            try:
                pages.append(page.extract_text() or "")
            except Exception:
                pages.append("")
    except Exception as exc:
        return _metadata(f"PDF 解析失败：{type(exc).__name__}")
    content = "\n\n".join(page.strip() for page in pages if page.strip()).strip()
    if not content:
        return _metadata("PDF 未提取到文本（可能是扫描件）")
    return _text(content)


def extract(path: Path, fmt: str) -> dict[str, Any]:
    if not path.is_file():
        return _metadata("附件文件不存在")
    if fmt == "docx":
        return extract_docx(path)
    if fmt == "pdf":
        return extract_pdf(path)
    if fmt == "epub":
        return extract_epub(path)
    if fmt in ("md", "markdown", "txt", "text"):
        return extract_plain_text(path)
    return _metadata(f"格式 {fmt or 'unknown'} 没有内置提取器")


def _payload() -> tuple[Optional[Path], Optional[str], Optional[str]]:
    args = sys.argv[1:]
    explicit: Optional[str] = None
    path_arg: Optional[str] = None
    index = 0
    while index < len(args):
        arg = args[index]
        if arg == "--format" and index + 1 < len(args):
            explicit = args[index + 1]
            index += 2
            continue
        if path_arg is None:
            path_arg = arg
        index += 1
    if path_arg is None and not sys.stdin.isatty():
        try:
            data = json.load(sys.stdin)
        except Exception as exc:
            return None, None, f"stdin 不是合法 JSON：{exc}"
        if isinstance(data, dict):
            path_arg = str(data.get("path") or "") or None
            explicit = str(data.get("format") or "") or explicit
    if not path_arg:
        return None, explicit, "缺少附件路径"
    return Path(path_arg), explicit, None


def main() -> int:
    path, explicit, error = _payload()
    if error is not None or path is None:
        print(json.dumps(_metadata(error or "缺少附件路径"), ensure_ascii=False))
        return 0
    result = extract(path, _format_of(path, explicit))
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
