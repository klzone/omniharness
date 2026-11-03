#!/usr/bin/env python3
"""OD-ARCH-134 —— Windows 安装器、防闪窗与依赖体检（静态口径）。

只读仓库文件；不构建、不联网、不启动应用。真正装上向导、在无 node 的
Windows 上看到检查页，属于用户实测项（见 PLANS.md 134 的验收标准）。

证据等级：static。
运行：python3 tests/verification/desktop/verify_windows_packaging_134.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
RUST = REPO / "desktop" / "src-tauri" / "src"
WORKFLOW = REPO / ".github" / "workflows" / "release.yml"
CONF = REPO / "desktop" / "src-tauri" / "tauri.conf.json"
API_TS = REPO / "desktop" / "src" / "api.ts"
APP_TSX = REPO / "desktop" / "src" / "App.tsx"
PAGE_TSX = REPO / "desktop" / "src" / "shell" / "EnvironmentCheck.tsx"
SYNC_SH = REPO / "scripts" / "sync-app-version.sh"
EVIDENCE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-134" / "facts.json"

CASES: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def windows_job_block(workflow: str) -> str:
    """取 workflow 里 windows job 的文本（到下一个顶层 job 为止）。"""
    start = workflow.find("\n  windows:")
    if start < 0:
        return ""
    rest = workflow[start + 1 :]
    end = rest.find("\n  publish:")
    return rest if end < 0 else rest[:end]


def main() -> int:
    workflow = read(WORKFLOW)
    conf_text = read(CONF)
    conf = json.loads(conf_text)
    platform_rs = read(RUST / "platform.rs")
    lib_rs = read(RUST / "lib.rs")
    api = read(API_TS)
    app = read(APP_TSX)
    page = read(PAGE_TSX)
    win = windows_job_block(workflow)

    # 1) 发布产物：Windows 走 NSIS 安装器
    record("wf_windows_builds_nsis", "--bundles nsis" in win, "windows job 构建命令")
    record("wf_windows_no_longer_bundleless", "--no-bundle" not in workflow, "--no-bundle 已移除")
    record(
        "wf_uploads_setup_exe",
        "bundle/nsis/*.exe" in win,
        "upload-artifact 含安装器",
    )
    record("wf_publish_ships_exe", "artifacts/*.exe" in workflow, "Release 上传 exe")
    record("wf_body_mentions_setup", "x64-setup.exe" in workflow or "setup.exe" in workflow)

    # 2) 安装器配置
    nsis = conf.get("bundle", {}).get("windows", {}).get("nsis", {})
    record("conf_nsis_install_mode", nsis.get("installMode") == "both", str(nsis.get("installMode")))
    record("conf_nsis_chinese", "SimpChinese" in (nsis.get("languages") or []), str(nsis.get("languages")))
    version = (REPO / "VERSION").read_text(encoding="utf-8").strip()
    record("conf_version_matches_version_file", conf.get("version") == version, f"{conf.get('version')} vs {version}")
    record("sync_script_exists", SYNC_SH.is_file())
    record(
        "wf_syncs_version_in_both_jobs",
        workflow.count("scripts/sync-app-version.sh") >= 2,
        "macOS 与 Windows 各一次",
    )

    # 3) 防闪窗
    record("rust_create_no_window_const", "0x0800_0000" in platform_rs)
    record(
        "rust_hide_console_exists",
        "pub fn hide_console(command: &mut Command)" in platform_rs,
    )
    record(
        "rust_hide_console_noop_off_windows",
        re.search(r'#\[cfg\(not\(target_os = "windows"\)\)\]\s*let _ = command;', platform_rs)
        is not None,
        "非 Windows 平台空操作",
    )
    for filename, label in [
        ("engine_ipc.rs", "引擎子进程"),
        ("dsh_settings.rs", "DSH 设置服务"),
        ("model_routes.rs", "模型路由脚本"),
        ("route_classifier.rs", "端点分类器"),
        ("profile_assets.rs", "插件安装"),
    ]:
        source = read(RUST / filename)
        record(f"rust_{filename}_hides_console", "platform::hide_console(" in source, label)
    opener_windows = platform_rs.split('cfg(target_os = "windows")]\nfn spawn_opener', 1)[-1]
    record(
        "rust_windows_opener_hides_console",
        "hide_console(&mut command)" in opener_windows.split("\n}\n", 1)[0],
    )

    # 4) 依赖解析
    record("rust_windows_node_candidates", "fn windows_node_candidates()" in platform_rs)
    record("rust_windows_node_program_files", 'join("nodejs").join("node.exe")' in platform_rs)
    record("rust_windows_node_nvm", 'join("nvm")' in platform_rs)
    record("rust_windows_node_scoop", 'join("scoop")' in platform_rs)
    record("rust_windows_python_localappdata", 'join("Programs").join("Python")' in platform_rs)
    record("rust_windows_py_launcher", "fn python_from_launcher()" in platform_rs)
    record(
        "rust_environment_report_shape",
        all(token in platform_rs for token in ['pub struct EnvironmentReport', "pub node: DependencyStatus", "pub python: DependencyStatus"]),
    )
    record("rust_python_requirement_310", "(3, 10)" in platform_rs and "Python 3.10+" in platform_rs)

    # 5) 接线：命令、前端调用、页面渲染
    record("lib_registers_environment_check", "fn environment_check()" in lib_rs and "environment_check," in lib_rs)
    record("api_calls_environment_check", 'invoke<EnvironmentReport>("environment_check")' in api)
    record("app_renders_environment_check", "<EnvironmentCheck" in app and "envReport.ok" in app)
    record("page_has_testid", 'data-testid="environment-check"' in page)
    record("page_links_downloads", "openExternalUrl(item.download)" in page)
    record("page_mentions_env_vars", "OMNIDESK_NODE" in page and "OMNIDESK_PYTHON" in page)

    # 6) 文件管理器打开统一走平台层（Windows 上 xdg-open 不存在）
    record(
        "lib_uses_platform_open_more_than_once",
        lib_rs.count("crate::platform::open_target(&dir)") >= 2,
    )
    record("lib_no_xdg_loop_left", '("xdg-open", Vec::<String>::new())' not in lib_rs)

    failed = [c for c in CASES if not c["ok"]]
    report = {
        "plan": "OD-ARCH-134",
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
