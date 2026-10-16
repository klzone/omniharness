#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
OmniDesk 文档入库脚本 (ingest-documents.py) v2

把源目录中的文档按「分类文件夹结构」导入 OmniDesk 知识库，并记录来源清单。

规则（与 docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md 一致）：
  - .wps  -> 转成 .docx 后放入目标目录（旧版 OLE 与新版本 OOXML 都能处理）
  - .doc / .docx / .pdf -> 原样复制
  - .et / .gd / .ofd / .emmx / .zip 等其他格式 -> 完全不动（既不转换也不复制）
  - 源目录中的文件绝不被修改、移动或删除（只读）
  - 目标目录中已存在且内容相同的文件默认跳过

性能：.wps 转换采用分批（每批一次 LibreOffice 进程）批量转换，避免逐文件启动。

用法:
  python3 scripts/ingest-documents.py --source <源目录> --dest knowledge/<目标目录> [--dry-run]
退出码：0 正常；1 源目录不存在；2 参数错误；3 有文件转换失败（其余已尽力完成）。
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent

SOFFICE_CANDIDATES = [
    os.environ.get("OMNIDESK_SOFFICE", ""),
    "/home/kevin/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override/soffice",
    "/snap/bin/libreoffice",
    "/usr/bin/libreoffice",
    "/usr/bin/soffice",
]

CONVERTIBLE_EXT = {".wps"}
COPY_EXT = {".doc", ".docx", ".pdf", ".md", ".txt"}
UNTOUCHED_EXT = {".et", ".gd", ".ofd", ".emmx", ".zip", ".xls", ".xlsx", ".ppt",
                 ".pptx", ".csv", ".exe", ".7z", ".rar", ".json"}

BATCH_SIZE = 40


def log(msg=""):
    print(msg, flush=True)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def find_soffice():
    for cand in SOFFICE_CANDIDATES:
        if cand and os.path.exists(cand) and os.access(cand, os.X_OK):
            return cand
    return shutil.which("soffice") or shutil.which("libreoffice")


def is_ooxml_docx(path):
    try:
        with zipfile.ZipFile(path, "r") as zf:
            return "word/document.xml" in zf.namelist()
    except Exception:
        return False


def batch_convert(soffice, jobs, chunk_size=BATCH_SIZE):
    """jobs: list of (src_path, dest_docx_path)。
    返回 (succeeded_dests:set, failures:list[(src,msg)])。"""
    succeeded, failures = set(), []
    for start in range(0, len(jobs), chunk_size):
        chunk = jobs[start:start + chunk_size]
        with tempfile.TemporaryDirectory(prefix="omnidesk_ingest_") as tmp:
            tmpdir = Path(tmp)
            profile = tmpdir / "lo-profile"
            profile.mkdir()
            mapping = {}
            staged = []
            for i, (src, dest) in enumerate(chunk):
                staged_name = f"{i:05d}.doc"
                staged_path = tmpdir / staged_name
                shutil.copy2(src, staged_path)
                mapping[f"{i:05d}"] = (src, dest)
                staged.append(str(staged_path))
            cmd = [soffice, "--headless", "--norestore", "--nolockcheck",
                   "-env:UserInstallation=file://" + str(profile),
                   "--convert-to", "docx"] + staged + ["--outdir", str(tmpdir)]
            try:
                res = subprocess.run(cmd, capture_output=True, text=True, timeout=120 + 15 * len(chunk))
                err = (res.stderr or "").strip()
            except subprocess.TimeoutExpired:
                err = "批次转换超时"
            except Exception as e:
                err = f"调用转换器失败：{e}"
            for idx, (src, dest) in mapping.items():
                produced = tmpdir / f"{idx}.docx"
                if produced.exists() and produced.stat().st_size > 0:
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(produced), str(dest))
                    succeeded.add(str(dest))
                else:
                    failures.append((src, err or "转换器未产出文件"))
        log(f"    批次 {start // chunk_size + 1}/{(len(jobs) + chunk_size - 1) // chunk_size} 完成"
            f"（本批 {len(chunk)} 个，累计成功 {len(succeeded)}）")
    return succeeded, failures


def main():
    ap = argparse.ArgumentParser(description="OmniDesk 文档入库脚本")
    ap.add_argument("--source", required=True, help="源目录（只读，不会被修改）")
    ap.add_argument("--dest", required=True, help="目标目录（知识库下的分类目录）")
    ap.add_argument("--dry-run", action="store_true", help="只预览，不写入任何文件")
    ap.add_argument("--manifest", default=None, help="manifest.json 输出路径（默认写入目标目录）")
    args = ap.parse_args()

    src_dir = Path(args.source).expanduser().resolve()
    dest_dir = Path(args.dest).expanduser()
    dest_dir = (PROJECT_ROOT / dest_dir).resolve() if not dest_dir.is_absolute() else dest_dir.resolve()

    if not src_dir.is_dir():
        log(f"[错误] 源目录不存在：{src_dir}")
        return 1

    allowed_roots = [PROJECT_ROOT, Path.home() / "OmniDesk"]
    if not any(dest_dir == r or r in dest_dir.parents for r in allowed_roots):
        log(f"[错误] 目标目录越界（只允许写入本仓库或 ~/OmniDesk 内）：{dest_dir}")
        return 2
    src_parents = set(src_dir.parents) | {src_dir}
    for forbidden in {Path.home(), Path.home() / ".dsh", Path.home() / ".agents", Path("/"), Path("/mnt/data")}:
        if dest_dir == forbidden or dest_dir in src_parents:
            log(f"[错误] 危险目标目录或与源目录重叠：{dest_dir}")
            return 2

    log("=" * 70)
    log("  OmniDesk 文档入库")
    log(f"  源目录 : {src_dir}")
    log(f"  目标   : {dest_dir}")
    log(f"  模式   : {'DRY-RUN（不写入）' if args.dry_run else '实际写入'}")
    log("=" * 70)

    # ---------------- 阶段 A：扫描分类 ----------------
    convert_jobs = []   # (src, dest_docx)
    copy_jobs = []      # (src, dest_file, same?)
    untouched = []
    records = []
    all_files = sorted(p for p in src_dir.rglob("*") if p.is_file() and not p.is_symlink())

    for src_file in all_files:
        rel = src_file.parent.relative_to(src_dir)
        ext = src_file.suffix.lower()
        rec = {"source": str(src_file), "rel_dir": str(rel), "ext": ext,
               "size": src_file.stat().st_size, "src_sha256": sha256_of(src_file)}

        if ext in CONVERTIBLE_EXT:
            out = dest_dir / rel / (src_file.stem + ".docx")
            rec["output"] = str(out)
            if is_ooxml_docx(src_file):
                same = out.exists() and out.stat().st_size == rec["size"] and sha256_of(out) == rec["src_sha256"]
                rec["action"] = "skipped_same" if same else ("would_copy_as_docx" if args.dry_run else "copied_as_docx")
                if not same and not args.dry_run:
                    copy_jobs.append((src_file, out, False))
                records.append(rec)
            else:
                rec["action"] = "would_convert" if args.dry_run else "convert"
                if not args.dry_run:
                    convert_jobs.append((src_file, out))
                records.append(rec)
        elif ext in COPY_EXT:
            out = dest_dir / rel / src_file.name
            rec["output"] = str(out)
            same = out.exists() and out.stat().st_size == rec["size"] and sha256_of(out) == rec["src_sha256"]
            rec["action"] = "skipped_same" if same else ("would_copy" if args.dry_run else "copy")
            if not same and not args.dry_run:
                copy_jobs.append((src_file, out, out.exists()))
            records.append(rec)
        else:
            rec["action"] = "untouched"
            untouched.append(str(src_file.relative_to(src_dir)))
            records.append(rec)

    n_conv = sum(1 for r in records if r["action"] in ("convert", "would_convert"))
    n_copy = sum(1 for r in records if r["action"] in ("copy", "would_copy", "copied_as_docx", "would_copy_as_docx"))
    n_same = sum(1 for r in records if r["action"] == "skipped_same")
    log(f"  扫描：共 {len(all_files)} 个文件")
    log(f"    wps 待转换   : {n_conv}")
    log(f"    可复制/改名  : {n_copy}")
    log(f"    内容相同跳过 : {n_same}")
    log(f"    其他格式未动 : {len(untouched)}")
    log("")

    if args.dry_run:
        log("  [dry-run] 未写入任何文件。以下为前 25 个待处理示例：")
        for r in records[:25]:
            log(f"    {r['action']:<18} {r['rel_dir']}/{Path(r['source']).name}")
        return 0

    if convert_jobs:
        soffice = find_soffice()
        if not soffice:
            log("[错误] 需要转换 .wps，但未找到可用的 LibreOffice/soffice。可设置 OMNIDESK_SOFFICE。")
            return 3
        log(f"[信息] 转换器：{soffice}")
        log(f"[信息] 开始批量转换 {len(convert_jobs)} 个 .wps（每批 {BATCH_SIZE} 个）...")
        succeeded, failures = batch_convert(soffice, convert_jobs)
        fail_map = {str(s): m for s, m in failures}
        for rec in records:
            if rec["action"] == "convert":
                if rec["output"] in succeeded:
                    rec["action"] = "converted"
                else:
                    rec["action"] = "failed"
                    rec["error"] = fail_map.get(rec["source"], "未知错误")
        stats_fail = len(failures)
    else:
        stats_fail = 0
        failures = []

    if copy_jobs:
        log(f"[信息] 复制 {len(copy_jobs)} 个 doc/docx/pdf ...")
        for src, out, existed in copy_jobs:
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, out)
            for rec in records:
                if rec.get("output") == str(out):
                    rec["action"] = "overwritten" if existed else ("copied" if rec["action"] != "copied_as_docx" else "copied_as_docx")

    stats = {
        "converted": sum(1 for r in records if r["action"] == "converted"),
        "copied": sum(1 for r in records if r["action"] in ("copied", "copied_as_docx")),
        "overwritten": sum(1 for r in records if r["action"] == "overwritten"),
        "skipped_same": n_same,
        "untouched": len(untouched),
        "failed": stats_fail,
    }

    manifest_path = Path(args.manifest).expanduser() if args.manifest else (dest_dir / "manifest.json")
    manifest = {
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(),
        "source_dir": str(src_dir),
        "dest_dir": str(dest_dir),
        "policy": "docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md",
        "note": "源目录只读；.wps 转 .docx，.doc/.docx/.pdf 复制，其余格式不动。",
        "stats": stats,
        "failures": [{"file": str(s.relative_to(src_dir)), "error": m} for s, m in failures],
        "untouched_files": untouched,
        "files": records,
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")

    log("")
    log("-" * 70)
    log("  统计：")
    log(f"    wps 转 docx 成功 : {stats['converted']}")
    log(f"    doc/docx/pdf 复制 : {stats['copied']}")
    log(f"    覆盖更新          : {stats['overwritten']}")
    log(f"    内容相同跳过      : {stats['skipped_same']}")
    log(f"    其他格式未动      : {stats['untouched']}")
    log(f"    转换失败          : {stats['failed']}")
    log(f"    manifest         : {manifest_path}")
    log("-" * 70)
    return 3 if stats["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
