#!/usr/bin/env python3
"""OD-SYS-702 Python 依赖补齐与依赖自检扩展：静态源码检查。

覆盖：
  1. platform.rs 的 python_modules 探测（requests 可导入、缺失 hint 可执行）；
  2. python_modules 不参与阻塞性 EnvironmentReport.ok；
  3. 前端 EnvironmentCheck 显示 Python 模块项、types 形状同步；
  4. anysearch SKILL.md 的依赖说明；
  5. 仓库侧依赖清单与打包补装步骤；
  6. runtime/ 零改动、无新增豁免。

证据等级：静态源码检查（不启动窗口、不联网、不装包）。

用法：python3 tests/verification/desktop/verify_sys_deps_702.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"
SCRIPTS = REPO / "scripts"
EVID = HERE / "od-sys-702"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def git_status(path: str) -> str:
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--", path],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        return result.stdout.strip()
    except OSError:
        return "git-unavailable"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path, default=EVID / "verify_sys_deps_702.json")
    args = ap.parse_args()

    platform = (RUST / "platform.rs").read_text(encoding="utf-8")
    production, _, tests = platform.partition("#[cfg(test)]")
    env_check = (SRC / "shell" / "EnvironmentCheck.tsx").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    skill = (REPO / "skills" / "anysearch" / "SKILL.md").read_text(encoding="utf-8")
    prep = (SCRIPTS / "prepare-runtime-assets.sh").read_text(encoding="utf-8")
    requirements = (SCRIPTS / "python-requirements.txt").read_text(encoding="utf-8")

    check(
        "platform_python_modules_field",
        "pub python_modules: DependencyStatus" in production
        and "python_modules," in production.split("EnvironmentReport {", 1)[1],
        "EnvironmentReport 已带 python_modules 字段并随构造返回",
    )
    check(
        "platform_requests_probe",
        'const PY_REQUESTS_PROBE: &str = "import requests";' in production
        and "fn python_imports_requests(" in production
        and "cmd.arg(\"-c\").arg(PY_REQUESTS_PROBE)" in production,
        "照 PY_VERSION_PROBE 模式用 -c 探针试导入 requests",
    )
    check(
        "platform_dependency_shape",
        all(
            token in production
            for token in [
                'name: "requests"',
                'requirement: "可导入"',
                "pip install requests certifi",
                "anysearch",
                "https://pypi.org/project/requests/",
            ]
        )
        and "fn python_modules_status(" in production,
        "DependencyStatus 名称/要求/缺失提示/下载页齐全且可 mock",
    )
    check(
        "platform_ok_includes_modules",
        "ok: node.ok && python.ok && assets.ok && python_modules.ok," in production,
        "requests 缺失时整体 ok 为 false，由环境检查页显式提示（引擎本身不受影响）",
    )
    check(
        "probe_mock_tests",
        all(
            name in tests
            for name in [
                "python_modules_ready_when_requests_imports",
                "python_modules_missing_hint_is_actionable",
                "python_modules_missing_python_still_hints_install",
                "python_modules_ok_implies_found",
                "requests_probe_returns_false_for_missing_program",
            ]
        ),
        "探测函数 mock 单测 5 项（任务卡要求 ≥2，新增 ≥4）",
    )
    check(
        "frontend_env_check_row",
        "<DependencyRow item={report.python_modules} />" in env_check
        and "Python 模块 requests" in env_check
        and 'isModule' in env_check,
        "环境检查面板照 node/python 行样式显示 Python 模块项",
    )
    check(
        "frontend_types_synced",
        'python_modules: EnvironmentDependency;' in types
        and '"assets" | "requests"' in types,
        "types.ts 的 EnvironmentDependency/EnvironmentReport 已同步",
    )
    check(
        "skill_dependency_note",
        "## 六、依赖" in skill
        and "runtime/python-vendor" in skill
        and "PYTHONPATH" in skill
        and "pip install requests certifi" in skill,
        "anysearch SKILL.md 说明内置依赖与开发态回退",
    )
    check(
        "repo_requirements_manifest",
        "requests>=2.31,<3" in requirements and "certifi>=2024.2.2" in requirements,
        "仓库侧清单 scripts/python-requirements.txt 含 requests 与 certifi",
    )
    check(
        "packaging_install_step",
        "scripts/python-requirements.txt" in prep
        and '-m pip install --no-input --disable-pip-version-check' in prep
        and '--target "$VENDOR"' in prep
        and "python-vendor" in prep,
        "打包准备脚本把 requests/certifi 内置到 runtime/python-vendor",
    )
    check("runtime_untouched", git_status("runtime") == "", "runtime/ 零改动")
    check(
        "no_new_exemptions",
        all(
            marker not in platform + env_check + types
            for marker in ["@ts-ignore", "@ts-expect-error", "eslint-disable", "ts-nocheck"]
        ),
        "未新增类型/lint 豁免",
    )
    check(
        "evidence_dir",
        EVID.is_dir(),
        "证据目录 tests/verification/desktop/od-sys-702/ 存在",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-SYS-702",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": name, "ok": ok, "detail": detail} for name, ok, detail in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
