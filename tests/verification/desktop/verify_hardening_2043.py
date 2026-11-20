#!/usr/bin/env python3
"""OD-HARDEN-2043：发布门禁、外联校验与宿主授权的静态/脚本验证。

本脚本只做静态源码与 YAML 结构核对，外加本仓库内可离线运行的子检查；
需要真实窗口、Linux 主机或联网的项在 STATUS.md 中标「未验证」。
"""

import json
import re
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
SRC = ROOT / "desktop" / "src-tauri" / "src"

FAILS = []
TOTAL = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global TOTAL
    TOTAL += 1
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def job_block(text: str, name: str) -> str:
    """按两格缩进的 job 名切出 YAML 片段，避免跨 job 误判。"""
    lines = text.splitlines()
    start = None
    for idx, line in enumerate(lines):
        if line.startswith("  " + name + ":"):
            start = idx
            break
    if start is None:
        return ""
    out = [lines[start]]
    for line in lines[start + 1:]:
        if line.strip() and not line.startswith("    ") and not line.startswith("  " + name):
            if line.startswith("  ") and not line.startswith("   "):
                break
            if not line.startswith(" "):
                break
        out.append(line)
    return "\n".join(out)


def main() -> int:
    text = WORKFLOW.read_text(encoding="utf-8")
    test = job_block(text, "test")
    macos = job_block(text, "macos")
    windows = job_block(text, "windows")
    publish = job_block(text, "publish")
    meta = job_block(text, "meta")

    check("workflow_pull_request_trigger", "  pull_request:" in text)
    check("workflow_test_job", test.startswith("  test:"), "test job 缺失")
    check("test_runs_ubuntu", "runs-on: ubuntu-latest" in test)
    apt = [
        "libwebkit2gtk-4.1-dev",
        "libgtk-3-dev",
        "libayatana-appindicator3-dev",
        "librsvg2-dev",
        "patchelf",
    ]
    check("test_apt_deps", all(dep in test for dep in apt), "缺少：" + ",".join(d for d in apt if d not in test))
    check("test_cargo_lib_single_thread", "cargo test --lib -- --test-threads=1" in test)
    check("test_cargo_not_offline", "--offline" not in test, "CI 不应带 --offline")
    check("test_npm_ci_and_build", "npm ci" in test and "npm run build" in test)
    check("test_python_requirements", "scripts/python-requirements.txt" in test)
    check("test_runs_verify_scripts", "python3 verify_hardening_2043.py" in test)
    check("test_script_exists", (ROOT / "tests" / "verification" / "desktop" / "verify_hardening_2043.py").is_file())
    check("no_continue_on_error", "continue-on-error" not in text)
    check("meta_not_on_pull_request", "github.event_name != 'pull_request'" in meta)
    check("macos_needs_test", re.search(r"needs:\s*\[\s*meta\s*,\s*test\s*\]", macos) is not None)
    check("windows_needs_test", re.search(r"needs:\s*\[\s*meta\s*,\s*test\s*\]", windows) is not None)
    check(
        "publish_needs_test",
        re.search(r"needs:\s*\[\s*meta\s*,\s*test\s*,\s*macos\s*,\s*windows\s*\]", publish) is not None,
    )
    check("publish_guarded_by_meta", "needs.meta.outputs.publish == 'true'" in publish)

    # T2：令牌最小化、Action 固定 SHA、Dependabot。
    check("default_permissions_read", "permissions:\n  contents: read" in text)
    check("publish_only_write", "contents: write" in publish)
    other_jobs_write = sum(
        "contents: write" in job_block(text, name)
        for name in ("meta", "test", "macos", "windows")
    )
    check("build_jobs_no_write", other_jobs_write == 0)
    tag_uses = re.findall(r"uses: [^\s@]+@(v\d+|stable|main|master)\s", text)
    check("no_tag_pinned_uses", not tag_uses, ",".join(tag_uses[:3]))
    check("uses_pinned_sha", len(re.findall(r"uses: [^\s@]+@[0-9a-f]{40}(\s|\n)", text)) >= 15)
    check("rust_toolchain_pinned_version", "toolchain: 1.99.1" in test)
    check("dependabot_actions", (ROOT / ".github" / "dependabot.yml").is_file())

    # 任务 C：外联 URL 统一校验。
    guard = (SRC / "net_guard.rs").read_text(encoding="utf-8")
    rss = (SRC / "rss_store.rs").read_text(encoding="utf-8")
    shell = (SRC / "task_shell.rs").read_text(encoding="utf-8")
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    check("net_guard_module_registered", "pub mod net_guard;" in lib)
    check(
        "net_guard_entry",
        "pub fn assert_public_http_url(raw: &str)" in guard
        and "pub fn assert_url_addresses(" in guard
    )
    check("net_guard_denies_cgnat", "CGNAT" in guard and "100" in guard)
    check("net_guard_maps_ipv4_mapped", "to_ipv4_mapped()" in guard)
    check("net_guard_pins_resolved", "fn pin_client(" in guard and ".resolve(" in guard)
    check("rss_manual_redirect", "redirect::Policy::none()" in rss and "MAX_FEED_REDIRECTS" in rss)
    check("rss_revalidates_each_hop", "assert_url_addresses(url, policy)" in rss)
    check("rss_denies_https_downgrade", "rejects_scheme_downgrade" in rss)
    check("rss_private_toggle_persisted", "allow_private_feeds" in rss)
    reading_cmd = (SRC / "commands" / "reading.rs")
    reading_src = reading_cmd.read_text(encoding="utf-8") if reading_cmd.is_file() else ""
    check(
        "rss_toggle_commands",
        "fn rss_settings_get(" in reading_src
        and "fn rss_settings_set_allow_private_feeds(" in reading_src
        and "commands::reading::rss_settings_set_allow_private_feeds," in lib
    )
    check("media_guarded_each_hop", "assert_url_addresses(&current" in shell)
    check("media_public_only", "NetPolicy::PublicOnly" in shell)
    check("media_limit_8_mib", "REMOTE_IMAGE_MAX_BYTES: usize = 8 * 1024 * 1024" in shell)
    check(
        "media_no_stale_32_mib_text",
        "媒体超过 32 MiB" not in shell and '"32 MiB' not in shell,
    )

    # 任务 B：宿主授权的本地读取。
    access = (SRC / "host_access.rs").read_text(encoding="utf-8")
    caps = (ROOT / "desktop" / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8")
    check("host_access_module_registered", "pub mod host_access;" in lib)
    check("host_access_registry", "pub struct HostAccess" in access and "SELECTION_TTL" in access)
    check(
        "host_pick_commands_registered",
        "host_pick_files," in lib and "host_pick_directory," in lib,
    )
    def command_body(fn_name: str) -> str:
        start = lib.find("fn " + fn_name + "(")
        if start < 0:
            return ""
        end = lib.find("\n}\n", start)
        return lib[start:end]

    gated = [
        "task_read_attachment",
        "task_read_material",
        "reading_import_book",
        "profile_import",
        "knowledge_vault_set_root",
        "task_set_library_root",
        "task_set_working_root",
        "task_set_artifact_root",
    ]
    for name in gated:
        body = command_body(name)
        check(
            "gate_" + name,
            "authorize_selected_path(&path)?" in body
            or "authorize_selected_or_known_root(&state, &path)?" in body,
            "命令没有宿主授权门",
        )
    check("material_and_attachment_gated", "fn task_read_material(" in lib and "authorize_selected_or_known_root(&state, &path)" in lib)
    check("dialog_capability_removed", "dialog:allow-open" not in caps)
    check(
        "no_frontend_plugin_dialog",
        all(
            '@tauri-apps/plugin-dialog' not in (ROOT / "desktop" / "src" / name).read_text(encoding="utf-8")
            for name in ["shell/HomeChat.tsx", "shell/ReadingPage.tsx", "shell/KnowledgeVaultPage.tsx", "SettingsPanel.tsx"]
        ),
    )
    check("frontend_uses_host_picks", (ROOT / "desktop" / "src" / "shell" / "HomeChat.tsx").read_text(encoding="utf-8").count("host.pickFiles(") >= 1)
    check("dropped_paths_registered_in_host", "tauri::WindowEvent::DragDrop" in lib)
    for fragment in ["/.ssh", "/.aws", "/.gnupg", "/.kube", "/.config/gcloud", "/.docker", "/library/keychains", "id_rsa", "id_ed25519", ".env"]:
        check("credential_fragment_" + fragment.strip("/."), fragment in shell)

    # 任务 E：小项加固。
    writing = (SRC / "writing_parse.rs").read_text(encoding="utf-8")
    outline = (SRC / "writing_outline.rs").read_text(encoding="utf-8")
    engine = (SRC / "engine_ipc.rs").read_text(encoding="utf-8")
    extract = (ROOT / "scripts" / "reading_attachment_extract.py").read_text(encoding="utf-8")
    check("writing_prompt_no_wait_with_output", "wait_with_output()" not in writing)
    check("writing_prompt_timeout_10s", "HELPER_TIMEOUT: Duration = Duration::from_secs(10)" in writing)
    check("writing_prompt_kills_on_timeout", "child.kill()" in writing and "try_wait()" in writing)
    check("docx_entity_full_scan", 'b"<!ENTITY" in raw.upper()' in extract)
    check("docx_doctype_not_rejected", "docx 正文含 DOCTYPE/ENTITY" not in extract)
    xml_guard = extract[extract.find("def _parse_xml") : extract.find("def _parse_xml") + 400]
    check("epub_entity_full_scan", 'if b"<!ENTITY" in raw.upper():' in xml_guard)
    check("epub_doctype_stripped", "DOCTYPE" in xml_guard)
    check("extract_entity_tests_exist", (ROOT / "tests" / "verification" / "desktop" / "verify_reading_entity_2043.py").is_file())
    check("engine_ipc_spawn_expects_gone", '"spawn stdout reader"' not in engine and '"spawn stderr reader"' not in engine)
    check("rss_html_unwraps_gone", ".chars().next().unwrap()" not in rss and "stack.pop().unwrap()" not in rss)
    check("outline_template_expect_gone", 'expect("内嵌默认类型模板必须是合法 JSON")' not in outline)

    # 任务 D：拆分 lib.rs（第 1 步）。命令名集合必须与基线一致。
    baseline_path = ROOT / "tests" / "verification" / "desktop" / "evidence" / "od-harden-2043" / "commands-baseline.json"
    if baseline_path.is_file():
        baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
        current = []
        for path in sorted(SRC.rglob("*.rs")):
            current.extend(
                re.findall(
                    r"#\[tauri::command[^]]*\]\s*(?:pub(?:\([^)]*\))?\s*)?(?:async )?fn ([a-zA-Z0-9_]+)\(",
                    path.read_text(encoding="utf-8"),
                )
            )
        current = sorted(set(current))
        # 0.2.45：当日摘要下线，两条命令按产品决定移除；其余命令仍须与基线一一对应。
        retired = {"worklog_daily_summary_generate", "worklog_daily_summary_read"}
        # 0.2.45：新增只读描述命令（不启动引擎），用于修复 5 秒轮询重启循环。
        added = {"engine_describe_if_running"}
        expected_names = sorted((set(baseline["names"]) - retired) | added)
        expected_count = baseline["count"] - len(retired) + len(added)
        check("d_modules_exist", (SRC / "commands" / "mod.rs").is_file() and (SRC / "commands" / "reading.rs").is_file())
        check("d_command_count_matches_baseline", len(current) == expected_count, f"{len(current)} != {expected_count}")
        check(
            "d_command_names_match_baseline",
            expected_names == current,
            str(sorted(set(expected_names) ^ set(current))[:5]),
        )

    print(f"cases={TOTAL} failures={len(FAILS)}")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
