#!/usr/bin/env python3
"""OD-READ-004：首页统计位置调整 + MOBI/PDF 原生阅读导入验证。

脚本只写 /tmp 一次性文件；不读真实书籍，不联网（PDF 只检查本地 vendor 的 pypdf）。
"""

import importlib.util
import json
import pathlib
import struct
import subprocess
import sys
import tempfile
from typing import Optional

ROOT = pathlib.Path(__file__).resolve().parents[3]
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
HOME = ROOT / "desktop" / "src" / "shell" / "HomeCanvas.tsx"
TASKS = ROOT / "desktop" / "src-tauri" / "src" / "task_shell.rs"
HELPER = ROOT / "scripts" / "reading_attachment_extract.py"
REQS = ROOT / "scripts" / "python-requirements.txt"
LAYOUT = ROOT / "docs" / "OmniDesk-Desktop-Data-Layout.md"
VENDOR = ROOT / "runtime" / "python-vendor"

FAILS = []
TOTAL = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global TOTAL
    TOTAL += 1
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def load_helper():
    spec = importlib.util.spec_from_file_location("reading_helper", HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_mobi(path: pathlib.Path, text: str, compression: int, encoding: int = 65001, encryption: int = 0) -> None:
    if compression == 1:
        data = bytes([0x61, 0x62, 0x63, 0x83, 0x03])
        plain_len = 9
    else:
        data = text.encode("utf-8")
        plain_len = len(data)
    name = b"Test Book"
    record0 = bytearray(16 + 0x78 + len(name))
    record0[0:2] = compression.to_bytes(2, "big")
    record0[4:8] = plain_len.to_bytes(4, "big")
    record0[8:10] = (1).to_bytes(2, "big")
    record0[12:14] = encryption.to_bytes(2, "big")
    record0[16:20] = b"MOBI"
    record0[20:24] = (0x78).to_bytes(4, "big")
    record0[16 + 0x0C:16 + 0x10] = encoding.to_bytes(4, "big")
    record0[16 + 0x44:16 + 0x48] = (16 + 0x78).to_bytes(4, "big")
    record0[16 + 0x48:16 + 0x4C] = len(name).to_bytes(4, "big")
    record0[16 + 0x74:16 + 0x78] = (0).to_bytes(4, "big")
    record0[16 + 0x78:] = name
    records = [bytes(record0), data]
    header = bytearray(78)
    header[0:4] = b"Test"
    header[60:64] = b"BOOK"
    header[64:68] = b"MOBI"
    header[76:78] = len(records).to_bytes(2, "big")
    offset = 78 + 8 * len(records)
    infos = bytearray()
    blobs = []
    for record in records:
        infos += struct.pack(">I", offset) + b"\x00\x00\x00\x00"
        blobs.append(record)
        offset += len(record)
    path.write_bytes(bytes(header) + bytes(infos) + b"".join(blobs))


def build_pdf(path: pathlib.Path) -> None:
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length 44 >>\nstream\nBT /F1 18 Tf 72 720 Td (Hello OmniDesk PDF) Tj ET\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for index, body in enumerate(objects, 1):
        offsets.append(len(out))
        out += f"{index} 0 obj\n".encode() + body + b"\nendobj\n"
    xref_at = len(out)
    out += b"xref\n0 6\n0000000000 65535 f \n"
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += b"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n" + str(xref_at).encode() + b"\n%%EOF\n"
    path.write_bytes(bytes(out))


def run_helper(path: pathlib.Path, fmt: str, python_path: Optional[str] = None) -> dict:
    env = {"PATH": "/usr/bin:/bin"}
    if python_path:
        env["PYTHONPATH"] = python_path
    result = subprocess.run(
        [sys.executable, str(HELPER), str(path), "--format", fmt],
        capture_output=True,
        text=True,
        env=env,
        timeout=60,
    )
    if result.returncode != 0:
        return {"kind": "error", "reason": result.stderr.strip()}
    try:
        return json.loads(result.stdout.strip().splitlines()[-1])
    except Exception as exc:
        return {"kind": "error", "reason": f"{exc}: {result.stdout[:120]}"}


def main() -> int:
    page = PAGE.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    tasks = TASKS.read_text(encoding="utf-8")
    reqs = REQS.read_text(encoding="utf-8")
    layout = LAYOUT.read_text(encoding="utf-8")

    # 1) 首页布局：统计移到对话下方，取消「项目跟踪」快捷入口。
    check("stats_slot_below_chat", 'data-testid="home-reading-stats-slot"' in home)
    check("stats_slot_after_chat", home.find("home-reading-stats-slot") > home.find("<HomeChat"))
    check("no_project_shortcut", "home-quick-pill" not in home)
    check("no_empty_footer", "emptyFooter" not in home)
    check("no_projects_state", "ProjectInfo" not in home and "projects" not in home)
    check("stats_still_navigates", 'onNavigate("reading")' in home)

    # 2) 导入格式：界面过滤器与 Rust 白名单都含 mobi，PDF 依赖已内置。
    check("ui_filter_mobi", '"mobi"' in page)
    check("rust_extract_mobi", '"mobi"' in tasks and "READING_EXTRACT_EXTS" in tasks)
    check("error_message_mobi", "epub / mobi" in tasks)
    check("helper_dispatch_mobi", 'fmt == "mobi"' in scripts_text())
    check("requirements_pypdf", "pypdf" in reqs)
    check("layout_doc_mobi", ".epub/.mobi" in layout)

    base = pathlib.Path(tempfile.mkdtemp(prefix="od-read-004-"))
    module = load_helper()
    compressed = module._palmdoc_decompress(bytes([0x61, 0x62, 0x63, 0x83, 0x03]))
    check("palmdoc_decompress", compressed == b"abcabcabc", repr(compressed))

    plain = base / "plain.mobi"
    build_mobi(plain, "<html><body><p>第一段</p><p>第二段</p></body></html>", 0)
    plain_out = run_helper(plain, "mobi")
    check("mobi_uncompressed_text", plain_out.get("kind") == "text", str(plain_out)[:160])
    content = plain_out.get("content") or ""
    check("mobi_text_strips_html", "第一段" in content and "第二段" in content and "<p>" not in content)

    comp = base / "comp.mobi"
    build_mobi(comp, "", 1)
    comp_out = run_helper(comp, "mobi")
    check("mobi_palmdoc_text", comp_out.get("kind") == "text" and "abcabcabc" in (comp_out.get("content") or ""), str(comp_out)[:160])

    huff = base / "huff.mobi"
    build_mobi(huff, "x", 17480)
    huff_out = run_helper(huff, "mobi")
    check("mobi_huff_rejected", huff_out.get("kind") == "metadata" and "HUFF" in (huff_out.get("reason") or ""), str(huff_out)[:160])

    drm = base / "drm.mobi"
    build_mobi(drm, "text", 0, encryption=2)
    drm_out = run_helper(drm, "mobi")
    check("mobi_drm_rejected", drm_out.get("kind") == "metadata" and "DRM" in (drm_out.get("reason") or ""), str(drm_out)[:160])

    pdf = base / "sample.pdf"
    build_pdf(pdf)
    vendor = str(VENDOR)
    pdf_out = run_helper(pdf, "pdf", python_path=vendor)
    check("pdf_text_native", pdf_out.get("kind") == "text" and "Hello OmniDesk PDF" in (pdf_out.get("content") or ""), str(pdf_out)[:160])

    print(f"cases={TOTAL} failures={len(FAILS)}")
    return 1 if FAILS else 0


def scripts_text() -> str:
    return HELPER.read_text(encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())

