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
import base64
from html import unescape
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

    # OD-HARDEN-2043：实体声明可能在 4 KB 之后，必须扫全文；DOCTYPE 不拒绝。
    if b"<!ENTITY" in raw.upper():
        return _metadata("docx 正文含实体声明，拒绝解析")
    if b"<!DOCTYPE" in raw[:4096].upper():
        raw = re.sub(
            rb"<!DOCTYPE[^>\[]*(?:\[[\s\S]*?\])?>",
            b"",
            raw,
            count=1,
            flags=re.IGNORECASE,
        )
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
    # EPUB 的 XHTML/OPF 常带 DOCTYPE。DOCTYPE 本身不是内容，剥掉后按普通 XML 解析；
    # 但任何内部实体声明（<!ENTITY …>）一律拒绝，避免实体展开。
    if b"<!ENTITY" in raw.upper():
        return None
    if b"<!DOCTYPE" in raw[:4096].upper():
        raw = re.sub(
            rb"<!DOCTYPE[^>\[]*(?:\[[\s\S]*?\])?>",
            b"",
            raw,
            count=1,
            flags=re.IGNORECASE,
        )
    try:
        return ET.fromstring(raw)
    except ET.ParseError:
        return None


_HTML_BLOCK_TAGS = {
    "p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "blockquote", "td", "pre", "figcaption",
}

MAX_EPUB_MEDIA_BYTES = 10 * 1024 * 1024
MAX_EPUB_COVER_BYTES = 2 * 1024 * 1024
MEDIA_MIME = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
    ".bmp": "image/bmp",
    ".mp3": "audio/mpeg",
    ".m4a": "audio/mp4",
    ".aac": "audio/aac",
    ".ogg": "audio/ogg",
    ".oga": "audio/ogg",
    ".wav": "audio/wav",
    ".flac": "audio/flac",
    ".mp4": "video/mp4",
    ".m4v": "video/mp4",
    ".webm": "video/webm",
    ".ogv": "video/ogg",
    ".mov": "video/quicktime",
}


def _mime_for_name(name: str) -> Optional[str]:
    return MEDIA_MIME.get(posixpath.splitext(name.lower())[1])


def _markdown_url(value: str) -> str:
    # 行内标记用 `)` 分隔，URL 内的空格与右括号必须先编码，避免前端截断。
    return value.replace(" ", "%20").replace(")", "%29")


def _media_marker(kind: str, src: str) -> str:
    if kind == "img":
        return f"![img]({_markdown_url(src)})"
    return f"[{kind}]({_markdown_url(src)})"


def _resolve_media_src(src: Optional[str], base_dir: str, names: dict[str, zipfile.ZipInfo]) -> Optional[str]:
    raw = (src or "").strip()
    if not raw or raw.startswith("#"):
        return None
    if raw.lower().startswith("data:"):
        return raw
    if re.match(r"^https?:", raw, re.IGNORECASE):
        return raw
    joined = posixpath.normpath(posixpath.join(base_dir, unquote(raw)))
    safe = _safe_zip_name(joined)
    if safe is None or safe not in names:
        return None
    return "odmedia:" + safe


def _inline_html_text(
    node: ET.Element,
    base_dir: str,
    names: dict[str, zipfile.ZipInfo],
) -> str:
    pieces: list[str] = []
    if node.text:
        pieces.append(node.text)
    for child in list(node):
        tag = _local_name(child.tag)
        if tag in ("script", "style", "svg", "head"):
            pass
        elif tag == "img":
            src = _resolve_media_src(child.get("src"), base_dir, names)
            if src:
                pieces.append(_media_marker("img", src))
        elif tag in ("audio", "video"):
            src = child.get("src")
            if not src:
                for item in child.iter():
                    if _local_name(item.tag) == "source" and item.get("src"):
                        src = item.get("src")
                        break
            resolved = _resolve_media_src(src, base_dir, names)
            if resolved:
                pieces.append(_media_marker(tag, resolved))
        elif tag == "br":
            pieces.append("\n")
        else:
            pieces.append(_inline_html_text(child, base_dir, names))
        if child.tail:
            pieces.append(child.tail)
    return "".join(pieces)


def _html_title(root: ET.Element) -> Optional[str]:
    for node in root.iter():
        if _local_name(node.tag) in ("h1", "h2", "h3"):
            title = re.sub(r"\s+", " ", "".join(node.itertext())).strip()
            if title:
                return title
    # 有些 EPUB 章节标题是 <div>第二章</div>，没有 h1/h2/h3；只认短且像章节名的 div。
    for node in root.iter():
        if _local_name(node.tag) != "div":
            continue
        title = re.sub(r"\s+", " ", "".join(node.itertext())).strip()
        if title and len(title) <= 40 and (
            re.match(r"^第[一二三四五六七八九十百千0-9]+[章节卷篇]", title)
            or re.match(r"^(chapter|part)\s+", title, re.IGNORECASE)
        ):
            return title
    return None


def _html_blocks(
    root: ET.Element,
    base_dir: str = "",
    names: Optional[dict[str, zipfile.ZipInfo]] = None,
) -> list[str]:
    """按常见块级节点取正文；跳过 script/style/svg，避免把样式或矢量路径当正文。

    图片/音频/视频改成段落内的行内标记（odmedia:<zip 内路径>），文字仍是纯文本，
    这样划线偏移和现有段落切分不变；前端识别标记后按需取字节渲染。
    """
    names = names or {}
    blocks: list[str] = []

    def has_block_child(node: ET.Element) -> bool:
        for child in node.iter():
            if child is node:
                continue
            if _local_name(child.tag) in _HTML_BLOCK_TAGS:
                return True
        return False

    def walk(node: ET.Element) -> None:
        tag = _local_name(node.tag)
        if tag in ("script", "style", "svg", "head"):
            return
        # 有些 EPUB 用 <div> 直接包正文（没有 <p>）。没有块级子节点时把 div 当一个块；
        # 有块级子节点时继续下钻，避免重复收整段。
        if tag == "div":
            if not has_block_child(node):
                text = re.sub(r"\s+", " ", _inline_html_text(node, base_dir, names)).strip()
                if text:
                    blocks.append(text)
                return
            for child in list(node):
                walk(child)
            return
        if tag in _HTML_BLOCK_TAGS:
            # 只取最外层块，避免 <li><p>…</p></li> 重复。
            text = re.sub(r"\s+", " ", _inline_html_text(node, base_dir, names)).strip()
            if text:
                blocks.append(text)
            return
        if tag == "img":
            src = _resolve_media_src(node.get("src"), base_dir, names)
            if src:
                blocks.append(_media_marker("img", src))
            return
        if tag in ("audio", "video"):
            src = node.get("src")
            if not src:
                for item in node.iter():
                    if _local_name(item.tag) == "source" and item.get("src"):
                        src = item.get("src")
                        break
            resolved = _resolve_media_src(src, base_dir, names)
            if resolved:
                blocks.append(_media_marker(tag, resolved))
            return
        for child in list(node):
            walk(child)

    walk(root)
    if not blocks:
        # 极端兜底：没有可识别的块级节点时，取整篇可见文字，至少让正文可读。
        text = re.sub(r"\s+", " ", _inline_html_text(root, base_dir, names)).strip()
        if text:
            blocks.append(text)
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
        author = ""
        for node in opf.iter():
            if _local_name(node.tag) == "creator":
                text = "".join(node.itertext()).strip()
                if text:
                    author = text
                    break
        base = posixpath.dirname(_safe_zip_name(rootfile) or rootfile)
        manifest: dict[str, str] = {}
        image_items: list[tuple[str, str, str]] = []
        cover_id = ""
        for node in opf.iter():
            if _local_name(node.tag) == "meta" and (node.get("name") or "").lower() == "cover":
                cover_id = (node.get("content") or "").strip()
            if _local_name(node.tag) == "item":
                item_id = node.get("id")
                href = node.get("href")
                media = (node.get("media-type") or "").lower()
                if item_id and href and media in ("application/xhtml+xml", "text/html"):
                    manifest[item_id] = posixpath.normpath(posixpath.join(base, unquote(href)))
                if item_id and href and media.startswith("image/"):
                    image_items.append((
                        item_id,
                        href,
                        (node.get("properties") or "").lower(),
                    ))
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

        cover: Optional[dict[str, Any]] = None
        cover_candidates = [
            item for item in image_items
            if "cover-image" in item[2]
        ]
        if not cover_candidates and cover_id:
            cover_candidates = [item for item in image_items if item[0] == cover_id]
        if not cover_candidates:
            cover_candidates = [
                item for item in image_items
                if "cover" in item[0].lower() or "cover" in item[1].lower()
            ]
        if cover_candidates:
            item_id, href, _ = cover_candidates[0]
            cover_name = _safe_zip_name(posixpath.normpath(posixpath.join(base, unquote(href))))
            info = names.get(cover_name) if cover_name else None
            if info is not None and info.file_size <= MAX_EPUB_COVER_BYTES:
                try:
                    raw_cover = archive.read(info)
                except (KeyError, OSError, RuntimeError):
                    raw_cover = None
                mime = _mime_for_name(cover_name or "")
                if raw_cover is not None and mime and mime.startswith("image/"):
                    cover = {
                        "name": posixpath.basename(cover_name or "cover"),
                        "mime": mime,
                        "data_base64": base64.b64encode(raw_cover).decode("ascii"),
                    }

        chapters: list[dict[str, str]] = []
        total_chars = 0
        for name in spine[:MAX_EPUB_DOCS]:
            raw_doc = read_entry(name)
            doc = _parse_xml(raw_doc) if raw_doc is not None else None
            if doc is None:
                continue
            blocks = _html_blocks(doc, posixpath.dirname(_safe_zip_name(name) or name), names)
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
        payload = _text(content, chapters)
        if author:
            payload["author"] = author
        if cover:
            payload["cover"] = cover
        return payload


def extract_epub_media(path: Path, media_ref: str, max_bytes: int = MAX_EPUB_MEDIA_BYTES) -> dict[str, Any]:
    """读取 EPUB 内一个媒体条目；只接受包内相对路径，拒绝 `..` 与绝对路径。"""
    safe = _safe_zip_name(media_ref)
    if safe is None:
        return _metadata("媒体路径不合法")
    try:
        archive = zipfile.ZipFile(path)
    except (zipfile.BadZipFile, OSError) as exc:
        return _metadata(f"EPUB 解压失败：{exc}")
    with archive:
        try:
            info = archive.getinfo(safe)
        except KeyError:
            return _metadata("EPUB 中找不到该媒体")
        if info.file_size > max_bytes:
            return _metadata(f"媒体超过 {max_bytes // 1024} KB 上限")
        mime = _mime_for_name(safe)
        if not mime or not mime.split("/", 1)[0] in ("image", "audio", "video"):
            return _metadata("媒体类型不在白名单内")
        try:
            raw = archive.read(info)
        except (KeyError, OSError, RuntimeError) as exc:
            return _metadata(f"读取媒体失败：{exc}")
    return {
        "kind": "media",
        "mime": mime,
        "bytes": len(raw),
        "data_base64": base64.b64encode(raw).decode("ascii"),
    }


PALMDOC_UNCOMPRESSED = 0
PALMDOC_COMPRESSED = 1
PALMDOC_COMPRESSED_ALT = 2
MOBI_MAX_PDB_RECORDS = 20000


def _palmdoc_decompress(data: bytes) -> bytes:
    """PalmDOC LZ77 解压（MOBI 正文最常见的压缩方式，纯标准库实现）。

    规则：0x00 输出字面 0；0x01–0x08 原样拷贝随后的 1–8 字节；0x09–0x7F 输出自身；
    0x80–0xBF 是两个字节的回指（距离 11 位、长度 3–10 位）；0xC0–0xFF 输出自身。
    数据损坏时中止并返回已解出的部分，由调用方按实际内容判断，不抛异常。
    """
    out = bytearray()
    index = 0
    total = len(data)
    while index < total:
        byte = data[index]
        index += 1
        if byte == 0:
            out.append(0)
        elif byte <= 8:
            chunk = data[index:index + byte]
            index += byte
            out.extend(chunk)
        elif byte <= 0x7F:
            out.append(byte)
        elif byte <= 0xBF:
            if index >= total:
                break
            second = data[index]
            index += 1
            distance = ((byte & 0x38) << 5) | second
            length = (byte & 0x07) + 3
            if distance <= 0 or distance > len(out):
                break
            for _ in range(length):
                out.append(out[-distance])
        else:
            out.append(byte)
    return bytes(out)


def _mobi_pdb_records(raw: bytes) -> Optional[list[bytes]]:
    """按 Palm Database 头部切出各记录；任何越界都整体拒绝。"""
    if len(raw) < 78:
        return None
    count = int.from_bytes(raw[76:78], "big")
    if count <= 0 or count > MOBI_MAX_PDB_RECORDS:
        return None
    offsets: list[int] = []
    for index in range(count):
        start = 78 + index * 8
        if start + 4 > len(raw):
            return None
        value = int.from_bytes(raw[start:start + 4], "big")
        if value < 78 or value >= len(raw):
            return None
        offsets.append(value)
    records: list[bytes] = []
    for index, start in enumerate(offsets):
        end = offsets[index + 1] if index + 1 < len(offsets) else len(raw)
        if end < start:
            return None
        records.append(raw[start:end])
    return records


def _mobi_image_mime(raw: bytes) -> Optional[str]:
    if raw.startswith(b"\xff\xd8\xff"):
        return "image/jpeg"
    if raw.startswith(b"\x89PNG\r\n\x1a"):
        return "image/png"
    if raw.startswith(b"GIF87a") or raw.startswith(b"GIF89a"):
        return "image/gif"
    return None


def _mobi_cover(record0: bytes, records: list[bytes], mobi_start: int, header_len: int, first_image: int) -> Optional[dict[str, Any]]:
    """EXTH 里的封面偏移（201/202）+ 首张图片记录号；失败不阻断正文。"""
    if first_image <= 0 or first_image >= len(records):
        return None
    flags_pos = mobi_start + 0x74
    if header_len < 0x78 or flags_pos + 4 > len(record0):
        return None
    flags = int.from_bytes(record0[flags_pos:flags_pos + 4], "big")
    if not flags & 0x40:
        return None
    pos = mobi_start + header_len
    if record0[pos:pos + 4] != b"EXTH":
        # 少数文件在 MOBI 头与 EXTH 之间有对齐填充。
        for shift in range(1, 5):
            if record0[pos + shift:pos + shift + 4] == b"EXTH":
                pos += shift
                break
        else:
            return None
    exth_len = int.from_bytes(record0[pos + 4:pos + 8], "big")
    count = int.from_bytes(record0[pos + 8:pos + 12], "big")
    if exth_len <= 12 or count <= 0 or count > 10000:
        return None
    end = min(pos + exth_len, len(record0))
    cursor = pos + 12
    cover_offset: Optional[int] = None
    thumb_offset: Optional[int] = None
    for _ in range(count):
        if cursor + 8 > end:
            break
        record_type = int.from_bytes(record0[cursor:cursor + 4], "big")
        record_len = int.from_bytes(record0[cursor + 4:cursor + 8], "big")
        if record_len < 8 or cursor + record_len > end:
            break
        payload = record0[cursor + 8:cursor + record_len]
        if payload:
            if record_type == 201 and cover_offset is None:
                cover_offset = int.from_bytes(payload[:4], "big")
            elif record_type == 202 and thumb_offset is None:
                thumb_offset = int.from_bytes(payload[:4], "big")
        cursor += record_len
    offset = cover_offset if cover_offset is not None else thumb_offset
    if offset is None:
        return None
    record_index = first_image + offset
    if record_index < 0 or record_index >= len(records):
        return None
    raw_cover = records[record_index]
    if not raw_cover or len(raw_cover) > MAX_EPUB_COVER_BYTES:
        return None
    mime = _mobi_image_mime(raw_cover)
    if mime is None:
        return None
    return {
        "name": "mobi-cover." + ("jpg" if mime == "image/jpeg" else "png" if mime == "image/png" else "gif"),
        "mime": mime,
        "data_base64": base64.b64encode(raw_cover).decode("ascii"),
    }


_MOBI_BLOCK_RE = re.compile(r"<(?:p|div|br|h[1-6]|li|tr|blockquote|section|article)\b[^>]*>", re.IGNORECASE)
_MOBI_LI_RE = re.compile(r"<li\b[^>]*>", re.IGNORECASE)
_MOBI_TAG_RE = re.compile(r"<[^>]+>")
_MOBI_SCRIPT_RE = re.compile(r"<(script|style)\b[\s\S]*?</\1>", re.IGNORECASE)


def _mobi_html_to_text(html: str) -> str:
    text = _MOBI_SCRIPT_RE.sub("\n\n", html)
    text = _MOBI_LI_RE.sub("\n\n· ", text)
    text = _MOBI_BLOCK_RE.sub("\n\n", text)
    text = _MOBI_TAG_RE.sub("", text)
    text = unescape(text).replace("\r", "\n").replace("\u00a0", " ")
    lines = [re.sub(r"[ \t]+", " ", line).strip() for line in text.split("\n")]
    return "\n".join(line for line in lines if line).strip()


def extract_mobi(path: Path) -> dict[str, Any]:
    max_file = _limit("OMNIDESK_READING_MAX_BYTES", 32 * 1024 * 1024)
    try:
        if path.stat().st_size > max_file:
            return _metadata("MOBI 超过阅读导入大小上限，正文未提取")
        raw = path.read_bytes()
    except OSError as exc:
        return _metadata(f"读取 MOBI 失败：{exc}")
    records = _mobi_pdb_records(raw)
    if not records:
        return _metadata("MOBI 不是有效的 Palm Database 文件")
    record0 = records[0]
    if len(record0) < 16:
        return _metadata("MOBI 头不完整")
    compression = int.from_bytes(record0[0:2], "big")
    text_length = int.from_bytes(record0[4:8], "big")
    text_records = int.from_bytes(record0[8:10], "big")
    encryption = int.from_bytes(record0[12:14], "big")
    if encryption != 0:
        return _metadata("MOBI 带 DRM 加密，正文未提取")
    if compression not in (PALMDOC_UNCOMPRESSED, PALMDOC_COMPRESSED, PALMDOC_COMPRESSED_ALT):
        return _metadata(f"MOBI 使用 HUFF/CDIC 压缩（{compression}），当前未支持")
    if text_records <= 0 or 1 + text_records > len(records):
        return _metadata("MOBI 正文记录数不合法")
    mobi_start = 16
    header_len = 0
    first_image = 0
    encoding = 1252
    if record0[mobi_start:mobi_start + 4] == b"MOBI" and len(record0) >= mobi_start + 0x18:
        header_len = int.from_bytes(record0[mobi_start + 4:mobi_start + 8], "big") if len(record0) >= mobi_start + 8 else 0
        if header_len >= 0x10:
            encoding = int.from_bytes(record0[mobi_start + 0x0C:mobi_start + 0x10], "big")
        if header_len >= 0x64:
            first_image = int.from_bytes(record0[mobi_start + 0x60:mobi_start + 0x64], "big")
    pieces: list[str] = []
    for index in range(1, 1 + text_records):
        record = records[index]
        chunk = record if compression == PALMDOC_UNCOMPRESSED else _palmdoc_decompress(record)
        if not chunk:
            continue
        pieces.append(chunk.decode("utf-8" if encoding == 65001 else "cp1252", errors="replace"))
    raw_text = "".join(pieces)
    if len(raw_text) > MAX_EPUB_TEXT_CHARS:
        raw_text = raw_text[:MAX_EPUB_TEXT_CHARS]
    text = _mobi_html_to_text(raw_text)
    if not text:
        return _metadata("MOBI 未提取到可读正文")
    payload = _text(text)
    cover = _mobi_cover(record0, records, mobi_start, header_len, first_image)
    if cover:
        payload["cover"] = cover
    return payload


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
    if fmt == "mobi":
        return extract_mobi(path)
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
    args = sys.argv[1:]
    media_ref: Optional[str] = None
    max_media = MAX_EPUB_MEDIA_BYTES
    path_hint: Optional[str] = None
    index = 0
    while index < len(args):
        arg = args[index]
        if arg == "--media" and index + 1 < len(args):
            media_ref = args[index + 1]
            index += 2
            continue
        if arg == "--max-media-bytes" and index + 1 < len(args):
            try:
                max_media = max(1, int(args[index + 1]))
            except ValueError:
                pass
            index += 2
            continue
        if path_hint is None:
            path_hint = arg
        index += 1
    if media_ref:
        if not path_hint:
            print(json.dumps(_metadata("缺少 EPUB 路径"), ensure_ascii=False))
            return 0
        result = extract_epub_media(Path(path_hint), media_ref, max_media)
        print(json.dumps(result, ensure_ascii=False))
        return 0
    path, explicit, error = _payload()
    if error is not None or path is None:
        print(json.dumps(_metadata(error or "缺少附件路径"), ensure_ascii=False))
        return 0
    result = extract(path, _format_of(path, explicit))
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
