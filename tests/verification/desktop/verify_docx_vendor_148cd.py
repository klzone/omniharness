#!/usr/bin/env python3
"""OD-DOCX-148c / 148d：python-docx+lxml 内置与可选模块体检。

证据等级：静态检查 + 本地 vendor 导入探针。
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"
SCRIPTS = REPO / "scripts"

CHECKS = []


def check(name, ok, detail=""):
    CHECKS.append((name, bool(ok), detail))


def read(path):
    return path.read_text(encoding="utf-8")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()
    requirements = read(SCRIPTS / "python-requirements.txt")
    prepare = read(SCRIPTS / "prepare-runtime-assets.sh")
    platform = read(RUST / "platform.rs")
    env_check = read(SRC / "shell" / "EnvironmentCheck.tsx")
    types = read(SRC / "types.ts")
    vendor = REPO / "runtime" / "python-vendor"

    check(
        "requirements_docx_lxml",
        "python-docx>=1.1,<2" in requirements and "lxml>=5,<7" in requirements,
        "requirements 含 python-docx 与 lxml",
    )
    check(
        "so_delete_limited_to_charset",
        'find "$VENDOR/charset_normalizer" -name' in prepare
        and "-name '*.so' -type f -delete" in prepare
        and 'find "$VENDOR" -name' not in prepare,
        "universal 删除 .so 只作用于 charset_normalizer",
    )
    proc = subprocess.run(
        [sys.executable, "-c", "import docx"],
        env=dict(os.environ, PYTHONPATH=str(vendor)),
        capture_output=True,
        check=False,
    )
    check(
        "vendor_docx_imports",
        vendor.is_dir() and proc.returncode == 0,
        f"PYTHONPATH=vendor python3 -c 'import docx' 通过（{proc.returncode}）",
    )
    check(
        "lxml_extension_present",
        (vendor / "lxml").is_dir() and any((vendor / "lxml").glob("*.so")),
        "lxml C 扩展未被误删",
    )
    check(
        "platform_probes",
        "PY_DOCX_PROBE" in platform
        and "PY_PYPDF_PROBE" in platform
        and "fn python_imports_code(" in platform,
        "platform 探测 python-docx 与 pypdf",
    )
    check(
        "platform_fields",
        "pub python_docx: DependencyStatus" in platform
        and "pub python_pypdf: DependencyStatus" in platform,
        "EnvironmentReport 增加两项可选依赖",
    )
    check(
        "optional_not_blocking",
        "ok: node.ok && python.ok && assets.ok && python_modules.ok," in platform,
        "可选模块不参与整体 ok",
    )
    check(
        "frontend_rows",
        "report.python_docx" in env_check and "report.python_pypdf" in env_check,
        "环境检查页显示两项",
    )
    check(
        "types_synced",
        '"python-docx" | "pypdf"' in types and "python_pypdf" in types,
        "ts 类型同步",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-DOCX-148c/148d",
        "evidence_level": "静态检查 + vendor 导入探针",
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

