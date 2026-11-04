#!/usr/bin/env python3
"""OD-ARCH-136 —— macOS 应用自带运行时、安装根识别收紧（静态 + 本机构建证据）。

0.2.7 的 macOS 包把构建机路径显示成了安装根：`.app` 自己不带运行时，找不到
同目录的 engine/runtime 时退回了 CARGO_MANIFEST_DIR。这里核对四件事：
macOS 资源清单、构建前准备顺序、免安装脚本去重、安装根识别的两条布局与兜底收紧。

证据等级：static + 本机 `.app` 结构（若已构建）。
运行：python3 tests/verification/desktop/verify_macos_bundle_136.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SRC_TAURI = REPO / "desktop" / "src-tauri"
MAC_CONF = SRC_TAURI / "tauri.macos.conf.json"
PLATFORM = SRC_TAURI / "src" / "platform.rs"
WORKFLOW = REPO / ".github" / "workflows" / "release.yml"
PORTABLE = REPO / "scripts" / "build-portable-package.sh"
PAGE = REPO / "desktop" / "src" / "shell" / "EnvironmentCheck.tsx"
APP = SRC_TAURI / "target" / "release" / "bundle" / "macos" / "OmniDesk.app"
EVIDENCE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-136" / "facts.json"

CASES: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    portable = PORTABLE.read_text(encoding="utf-8")
    platform_rs = PLATFORM.read_text(encoding="utf-8")
    page = PAGE.read_text(encoding="utf-8")

    record("mac_conf_exists", MAC_CONF.is_file(), "macOS 平台专属配置")
    mac = json.loads(MAC_CONF.read_text(encoding="utf-8")) if MAC_CONF.is_file() else {}
    resources = ((mac.get("bundle") or {}).get("resources")) or {}
    for source, target in {
        "../../engine": "engine",
        "../../runtime/RUNTIME.lock": "runtime/RUNTIME.lock",
        "../../runtime/dsh-runtime": "runtime/dsh-runtime",
        "../../runtime/dsh-home-seed": "runtime/dsh-home-seed",
    }.items():
        record(f"mac_resource_{target.replace('/', '_')}", resources.get(source) == target, f"{source} → {target}")

    # macOS job 必须在构建前准备资产，否则资源清单指向不存在的目录。
    mac_start = workflow.find("\n  macos:")
    mac_block = workflow[mac_start:workflow.find("\n  windows:")] if mac_start >= 0 else ""
    build_at = mac_block.find("npx tauri build")
    prepare_at = mac_block.find("scripts/prepare-runtime-assets.sh")
    record("wf_macos_prepares_assets", prepare_at > 0, "macOS job 调准备脚本")
    record("wf_macos_prepare_before_build", 0 < prepare_at < build_at if build_at > 0 else False)

    # 免安装脚本：macOS 不再复制同目录运行时（.app 已自带）。
    record("portable_skips_mac_duplicates", 'if [ "$IS_MAC" -eq 1 ]; then' in portable)
    record("portable_mac_message", "运行时已内置于 OmniDesk.app" in portable)
    record(
        "portable_requires_bundled_runtime",
        "Contents/Resources/engine/omnidesk_engine" in portable
        and "Contents/Resources/runtime/dsh-runtime" in portable,
        "打包前校验 .app 真的自带运行时，拒绝旧的空壳构建",
    )

    # 安装根识别：两条布局 + 收紧兜底。
    record("find_bundle_root_exists", "fn find_bundle_root(start: &Path)" in platform_rs)
    record("find_bundle_root_checks_resources", 'join("Resources")' in platform_rs)
    record("bundle_root_guard", "fn looks_like_bundle_root" in platform_rs)
    record(
        "compile_fallback_validated",
        "if looks_like_bundle_root(&candidate)" in platform_rs,
        "编译期路径必须真的含运行时才算数",
    )
    record(
        "tests_cover_app_layout",
        "find_bundle_root_accepts_app_resources_layout" in platform_rs
        and "find_bundle_root_accepts_portable_layout" in platform_rs,
    )

    # 文案：运行时缺失不能叫「版本过低」。
    record("label_not_version_for_assets", 'isAssets ? "不完整" : "版本过低"' in page)
    record("assets_row_hides_requirement", "{isAssets ? null : (" in page)

    # 本机构建证据（可选）：.app 内资源必须齐全。
    if APP.is_dir():
        res = APP / "Contents" / "Resources"
        record("built_app_has_engine", (res / "engine" / "omnidesk_engine").is_dir())
        record("built_app_has_runtime_lock", (res / "runtime" / "RUNTIME.lock").is_file())
        record("built_app_has_dsh_runtime", (res / "runtime" / "dsh-runtime").is_dir())
        record("built_app_has_seed", (res / "runtime" / "dsh-home-seed").is_dir())
        record("built_app_has_no_sibling_engine", not (APP.parent / "engine").exists(), "同目录不应再有 engine")
    else:
        record("built_app_present", False, "本机尚未构建 .app（可跳过）")

    failed = [c for c in CASES if not c["ok"]]
    report = {
        "plan": "OD-ARCH-136",
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
