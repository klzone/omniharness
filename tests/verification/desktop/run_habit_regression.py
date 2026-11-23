#!/usr/bin/env python3
"""OD-GROW-301 H6：习惯提取/检查回归入口。

改动提取提示词、信号词白名单、宿主校验、规则种类或相关 SKILL 前先跑本脚本。
依次执行三组测试，任一失败退出 1；输出每组 PASS/FAIL 与通过数。

用法：
  python3 tests/verification/desktop/run_habit_regression.py
  python3 tests/verification/desktop/run_habit_regression.py --output <文件>
"""

from __future__ import annotations

import argparse
import contextlib
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
DESKTOP_TAURI = ROOT / "desktop" / "src-tauri"
ENGINE = ROOT / "engine"


def find_cargo() -> str | None:
    found = shutil.which("cargo")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "cargo"
    return str(fallback) if fallback.exists() else None


def run_case(label: str, cmd: list[str], cwd: Path) -> tuple[bool, int, str]:
    env = os.environ.copy()
    cargo_dir = str(Path.home() / ".cargo" / "bin")
    env["PATH"] = cargo_dir + os.pathsep + env.get("PATH", "")
    print(f"\n$ {' '.join(cmd)}")
    print(f"  cwd: {cwd}")
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
    )
    print(proc.stdout.rstrip())
    if proc.stderr.strip():
        print(proc.stderr.rstrip())

    cargo_match = re.search(r"test result: ok\. (\d+) passed", proc.stdout)
    python_match = re.search(r"Ran (\d+) tests", proc.stderr + proc.stdout)
    if cargo_match:
        count = int(cargo_match.group(1))
    elif python_match:
        count = int(python_match.group(1))
    else:
        count = 0
    passed = proc.returncode == 0
    print(f"[{('PASS' if passed else 'FAIL')}] {label}: {count} passed, exit {proc.returncode}")
    return passed, count, label


def main() -> int:
    parser = argparse.ArgumentParser(description="OD-GROW-301 habit regression")
    parser.add_argument("--output", type=Path, help="把完整输出同时写入该文件")
    args = parser.parse_args()

    class Tee:
        def __init__(self) -> None:
            self.parts: list[str] = []

        def write(self, value: str) -> None:
            sys.__stdout__.write(value)
            self.parts.append(value)

        def flush(self) -> None:
            sys.__stdout__.flush()

    tee = Tee()
    with contextlib.redirect_stdout(tee):
        exit_code = _run(args)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text("".join(tee.parts), encoding="utf-8")
    return exit_code


def _run(args: argparse.Namespace) -> int:

    cargo = find_cargo()
    if cargo is None:
        print("[FAIL] 未找到 cargo（PATH 与 ~/.cargo/bin 均无）")
        return 1

    cases = [
        (
            "habit_extract（含 ≥20 组表驱动样例）",
            [cargo, "test", "--lib", "--offline", "--", "habit_extract"],
            DESKTOP_TAURI,
        ),
        (
            "habit_checks",
            [cargo, "test", "--lib", "--offline", "--", "habit_checks"],
            DESKTOP_TAURI,
        ),
        (
            "engine habit_extract",
            [
                sys.executable,
                "-m",
                "unittest",
                "discover",
                "-s",
                "omnidesk_engine",
                "-p",
                "habit_extract_test.py",
            ],
            ENGINE,
        ),
    ]

    print("OmniDesk 习惯回归（OD-GROW-301 H6/G11）")
    results = [run_case(label, cmd, cwd) for label, cmd, cwd in cases]
    total = sum(count for _, count, _ in results)
    failed = [label for passed, _, label in results if not passed]
    print("\n===== 汇总 =====")
    for label, (passed, count, _) in zip([case[0] for case in cases], results):
        print(f"{('PASS' if passed else 'FAIL')}  {label}: {count}")
    print(f"合计 {total} 个测试，{len(failed)} 组失败")
    print("结论：" + ("全部通过" if not failed else "失败 → " + "、".join(failed)))

    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
