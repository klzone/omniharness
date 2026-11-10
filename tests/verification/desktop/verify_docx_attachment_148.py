#!/usr/bin/env python3
"""OD-DOCX-148a 附件广收窄取：静态检查 + helper 实跑。

证据等级：静态源码检查 + 合成 docx 运行 helper（不读用户数据、不联网）。

用法：python3 tests/verification/desktop/verify_docx_attachment_148.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"
HELPER = REPO / "scripts" / "reading_attachment_extract.py"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def make_docx(path: Path) -> None:
    doc = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        "<w:body><w:p><w:r><w:t>合成正文 VERIFY-148A</w:t></w:r></w:p>"
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>甲</w:t></w:r></w:p></w:tc>"
        "<w:tc><w:p><w:r><w:t>乙</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    )
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("[Content_Types].xml", '<?xml version="1.0"?><Types/>')
        archive.writestr("word/document.xml", doc)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    shell = read(RUST / "task_shell.rs")
    export = read(RUST / "export_engine.rs")
    types = read(SRC / "types.ts")
    prompt = read(SRC / "attachmentPrompt.ts")
    chat = read(SRC / "shell" / "HomeChat.tsx")

    check(
        "material_kind_fields",
        "pub content_kind: String" in shell
        and "pub note: Option<String>" in shell
        and 'content_kind: "text"' in shell,
        "MaterialRead 带 content_kind/note，文本路径仍返回 text",
    )
    check(
        "docx_uses_embedded_helper",
        "include_str!(\"../../../scripts/reading_attachment_extract.py\")" in shell
        and "run_attachment_extract_helper(&resolved, \"docx\")" in shell,
        "docx 使用 include_str! 内置 helper",
    )
    check(
        "pdf_optional_pypdf",
        "run_attachment_extract_helper(&resolved, \"pdf\")" in shell
        and "content_kind: kind.to_string()" in shell,
        "PDF 走可选 pypdf，失败降级 metadata_only",
    )
    check(
        "images_degrade_to_metadata",
        "图片正文未提取（未做 OCR）" in shell and "metadata_only" in shell,
        "图片与未知二进制不再拒收，改为元数据降级",
    )
    check(
        "no_save_as_wording",
        "另存为" not in shell
        and "另存为" not in chat
        and "只收文本与表格" not in chat,
        "附件通道移除「另存为 / 只收文本」话术",
    )
    check(
        "over_limit_still_rejected",
        "超过单文件上限" in shell and "ATTACHMENT_MAX_BYTES" in shell,
        "超单文件上限仍拒绝",
    )
    check(
        "python_candidates_reused",
        "ExportTools::discover().python_candidates" in shell
        and "OMNIDESK_EXPORT_PYTHON" in export
        and "OMNIDESK_PYTHON" in export,
        "候选解释器沿用 export_engine 顺序",
    )
    check(
        "vendor_pythonpath_two_platforms",
        "OMNIDESK_PYTHON_VENDOR" in shell
        and 'if cfg!(windows) { ";" } else { ":" }' in shell,
        "PYTHONPATH 注入 vendor，Windows 用分号、POSIX 用冒号",
    )
    check(
        "timeout_degrade",
        "ATTACHMENT_HELPER_TIMEOUT" in shell and "child.kill()" in shell,
        "helper 超时被终止并降级，不阻塞发送",
    )
    check(
        "front_types",
        'content_kind: "text" | "metadata_only"' in types
        and "note: string | null" in types,
        "ts MaterialRead 同步 content_kind/note",
    )
    check(
        "prompt_metadata_only",
        "正文未提取；如需正文，请用文件工具按上述路径自行读取" in prompt
        and "contentKind === \"metadata_only\"" in prompt,
        "提示词块对 metadata_only 给明确路径读取指引",
    )
    check(
        "chip_status",
        "正文未提取" in chat and "data-attachment-kind" in chat,
        "前端 chip 显示「正文未提取」状态",
    )
    check(
        "helper_runs_synthetic_docx",
        HELPER.is_file(),
        "helper 文件存在",
    )
    with tempfile.TemporaryDirectory(prefix="od148a-") as tmp:
        docx = Path(tmp) / "fixture.docx"
        make_docx(docx)
        proc = subprocess.run(
            [sys.executable, str(HELPER), str(docx)],
            capture_output=True,
            text=True,
            timeout=20,
            check=False,
        )
        parsed = json.loads(proc.stdout) if proc.stdout.strip() else {}
        check(
            "helper_returns_text",
            proc.returncode == 0
            and parsed.get("kind") == "text"
            and "VERIFY-148A" in str(parsed.get("content") or "")
            and "甲\t乙" in str(parsed.get("content") or ""),
            f"helper 输出：{proc.stdout.strip()[:160]}",
        )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-DOCX-148a",
        "evidence_level": "静态源码检查 + 合成 docx helper 实跑",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
