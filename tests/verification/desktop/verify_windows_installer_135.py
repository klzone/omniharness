#!/usr/bin/env python3
"""OD-ARCH-135 —— Windows 安装器要带运行时资产（静态口径）。

用户装完只有 omnidesk-desktop.exe 与 uninstall.exe：NSIS 只打包了二进制，
engine / runtime / 首启 seed 一个都没写进资源清单。这里核对四件事：
Windows 专属资源清单存在且指向真实目录、准备脚本存在、CI 在构建前调用它、
免安装包与安装器共用同一份准备逻辑。

证据等级：static。
运行：python3 tests/verification/desktop/verify_windows_installer_135.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SRC_TAURI = REPO / "desktop" / "src-tauri"
WIN_CONF = SRC_TAURI / "tauri.windows.conf.json"
MAIN_CONF = SRC_TAURI / "tauri.conf.json"
WORKFLOW = REPO / ".github" / "workflows" / "release.yml"
PREPARE = REPO / "scripts" / "prepare-runtime-assets.sh"
PORTABLE = REPO / "scripts" / "build-portable-package.sh"
GITIGNORE = REPO / ".gitignore"
RUST = SRC_TAURI / "src"
PAGE = REPO / "desktop" / "src" / "shell" / "EnvironmentCheck.tsx"
EVIDENCE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-135" / "facts.json"

CASES: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    portable = PORTABLE.read_text(encoding="utf-8")
    prepare = PREPARE.read_text(encoding="utf-8") if PREPARE.is_file() else ""
    gitignore = GITIGNORE.read_text(encoding="utf-8")
    dsh_settings = (RUST / "dsh_settings.rs").read_text(encoding="utf-8")
    lib_rs = (RUST / "lib.rs").read_text(encoding="utf-8")
    platform_rs = (RUST / "platform.rs").read_text(encoding="utf-8")
    page = PAGE.read_text(encoding="utf-8")

    record("win_conf_exists", WIN_CONF.is_file(), "Windows 平台专属配置")
    win = json.loads(WIN_CONF.read_text(encoding="utf-8")) if WIN_CONF.is_file() else {}
    resources = (((win.get("bundle") or {}).get("resources")) or {})
    record("win_conf_has_resources", bool(resources), str(list(resources)[:4]))

    # 资源源路径必须真实存在（相对 src-tauri 目录），否则 tauri build 会直接报错。
    required = {
        "../../engine": "engine",
        "../../runtime/RUNTIME.lock": "runtime/RUNTIME.lock",
        "../../runtime/dsh-runtime": "runtime/dsh-runtime",
        "../../runtime/dsh-home-seed": "runtime/dsh-home-seed",
    }
    for source, target in required.items():
        record(f"resource_mapped_{target.replace('/', '_')}", resources.get(source) == target, f"{source} → {target}")

    record("engine_dir_exists", (REPO / "engine" / "omnidesk_engine").is_dir())
    record("runtime_lock_exists", (REPO / "runtime" / "RUNTIME.lock").is_file())

    # 准备脚本：裁剪 + seed，一处实现两处调用。
    record("prepare_script_exists", PREPARE.is_file())
    record("prepare_builds_seed", "dsh-home-seed" in prepare and "npm install" in prepare)
    record("prepare_prunes_typescript", "-name '*.ts'" in prepare and "-name '*.map'" in prepare)
    record("prepare_seed_profiles", "omnidesk-base" in prepare and "web" in prepare)
    record("seed_is_gitignored", "runtime/dsh-home-seed" in gitignore)

    # CI：Windows job 必须在 tauri build 之前准备资产，否则资源清单指向空目录。
    win_start = workflow.find("\n  windows:")
    win_block = workflow[win_start:] if win_start >= 0 else ""
    build_at = win_block.find("npx tauri build")
    prepare_at = win_block.find("scripts/prepare-runtime-assets.sh")
    record("wf_windows_runs_prepare", prepare_at > 0, "windows job 调准备脚本")
    record(
        "wf_prepare_before_build",
        0 < prepare_at < build_at if build_at > 0 else False,
        "准备必须先于 tauri build",
    )

    # 免安装包复用同一逻辑，避免两份实现漂移。
    record("portable_calls_prepare", "prepare-runtime-assets.sh" in portable)
    record("portable_no_duplicate_prune", portable.count("-name '*.ts'") == 0)

    # 同类缺陷一：Windows 没有 kill -0，进程存活检查恒 false → 保存密钥报
    # 「地址没有留在内存里」。改为端口可达即视为可用，进程表不再参与判定。
    record("reusable_uses_port_only", "if port_listening(running.port)" in dsh_settings)
    record("reusable_no_process_alive", "process_alive(" not in dsh_settings)
    record("no_kill_zero_probe", 'arg("-0")' not in dsh_settings)

    # 同类缺陷二：启动失败在 GUI 下无声。改为系统消息框。
    record("fatal_startup_dialog", "fn report_fatal_startup" in lib_rs)
    record("fatal_dialog_windows", "PresentationFramework" in lib_rs and "MessageBox" in lib_rs)
    record("fatal_dialog_called", "report_fatal_startup(&message)" in lib_rs)

    # 安装完整性进入体检页，缺资源时直接说「重新安装」。
    record("report_has_assets", "pub assets: DependencyStatus" in platform_rs)
    record("assets_hint_reinstall", "安装不完整" in platform_rs and "重新运行安装程序" in platform_rs)
    record("page_renders_assets", "report.assets" in page)

    # 主配置仍不挂资源：macOS 走 .app + 免安装目录，重复打包会让包体翻倍。
    main_conf = json.loads(MAIN_CONF.read_text(encoding="utf-8"))
    record("main_conf_has_no_resources", "resources" not in (main_conf.get("bundle") or {}))

    failed = [c for c in CASES if not c["ok"]]
    report = {
        "plan": "OD-ARCH-135",
        "platform": sys.platform,
        "cases": CASES,
        "summary": {"total": len(CASES), "failed": len(failed)},
        "passed": not failed,
    }
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for case in CASES:
        mark = "pass" if case["ok"] else "fail"
        suffix = (" " + case["detail"]) if case["detail"] else ""
        print("[%s] %s%s" % (mark, case["name"], suffix))
    print("\n证据：%s" % EVIDENCE)
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
