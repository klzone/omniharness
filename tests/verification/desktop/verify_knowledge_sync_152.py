#!/usr/bin/env python3
"""OD-ARCH-152：知识库写入沙箱边界（workspace 暂存 + 宿主同步落库）。

静态检查覆盖宿主同步通道、IPC、界面入口、skill 文案与引擎终态提示；
脚本级端到端用 /tmp 合成根调用真实 Rust `sync_from_workspace`，
不读取、不写入任何真实知识库。
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
VAULT_RS = REPO / "desktop/src-tauri/src/knowledge_vault.rs"
LIB_RS = REPO / "desktop/src-tauri/src/lib.rs"
API = REPO / "desktop/src/api.ts"
PANEL = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"
SKILL = REPO / "skills/knowledge-search/SKILL.md"
ADAPTER = REPO / "engine/omnidesk_engine/web_api_adapter.py"
EVIDENCE = REPO / "tests/verification/desktop/evidence/od-arch-152"
CARGO = Path.home() / ".cargo/bin/cargo"

CASES: list[dict[str, object]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def section(text: str, heading: str) -> str:
    start = text.find(heading)
    if start < 0:
        return ""
    end = text.find("\n## ", start + len(heading))
    return text[start:] if end < 0 else text[start:end]


def static_checks() -> None:
    vault = VAULT_RS.read_text(encoding="utf-8")
    lib = LIB_RS.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    panel = PANEL.read_text(encoding="utf-8")
    skill = SKILL.read_text(encoding="utf-8")
    adapter = ADAPTER.read_text(encoding="utf-8")

    check("sync_fn_public", "pub fn sync_from_workspace" in vault)
    check("sync_staging_const", 'SYNC_STAGING_REL: &str = "staging/kb"' in vault)
    check("sync_reuses_create_note", "vault_write::create_note" in vault)
    check("sync_card_limit", "CARD_MAX_CHARS" in vault)
    check("sync_utf8", "read_to_string" in vault and "UTF-8" in vault)
    check("sync_conflict_no_overwrite", "同步不覆盖" in vault)
    check("sync_index_idempotent", "index_has_card_link" in vault)
    check("sync_cleanup_on_success", "remove_file(&source)" in vault)
    check("sync_keeps_failed", "原样保留" in vault)
    check("sync_allowed_dirs", "知识库/概念" in vault and "知识库/方法" in vault)

    check("command_defined", "fn knowledge_sync_to_vault" in lib)
    check("command_registered", "knowledge_sync_to_vault," in lib)
    check("command_uses_vault_root", "configured_vault_root(&paths)" in lib)
    check("command_uses_workspace", "prepare_local_data_root" in lib)

    check("api_method", "knowledgeSyncToVault()" in api)
    check("api_invokes_command", 'invoke<KnowledgeSyncReport>("knowledge_sync_to_vault")' in api)
    check("panel_button", 'data-testid="knowledge-sync-button"' in panel)
    check("panel_calls_api", "shell.knowledgeSyncToVault()" in panel)
    check("panel_report", 'data-testid="knowledge-sync-report"' in panel)
    check("panel_shows_conflicts", "syncReport.conflicts" in panel and "syncReport.errors" in panel)

    sec4 = section(skill, "## 四、")
    sec6 = section(skill, "## 六、")
    check("skill_write_staging", "staging/kb/知识库/概念/" in sec4 and "当前工作目录" in sec4)
    check("skill_staging_relative_not_prefixed", "`workspace/staging" not in sec4)
    check("skill_no_direct_vault_write", "不要直接写知识库根" in sec4)
    check("skill_no_missing_args", "禁止缺参调用" in sec4 and "`file_path`" in sec4 and "`content`" in sec4)
    check("skill_index_by_host", "不要自己改索引" in sec4 and "knowledge_sync_to_vault" in sec4)
    check("skill_root_from_settings", "knowledge_vault_root" in sec6 and "不猜路径" in sec6)
    check("skill_sections_intact", all(f"## {x}、" in skill for x in ["一", "二", "三", "四", "五", "六", "七"]))
    check("skill_two_hop_intact", "第一跳：只读索引" in skill and "第二跳：按标签定位" in skill)

    check("engine_detector", "def staging_kb_card_count" in adapter)
    check("engine_hint_kind", '"pending_vault_sync"' in adapter)
    check("engine_hint_in_terminal", "staging_kb_card_count(getattr(record, \"workspace_dir\", None))" in adapter)
    check("engine_no_wait", "time.sleep" not in adapter.split("def _succeeded_provenance")[-1].split("def ")[0])
    check("engine_route_untouched", "preset_for_intent" in adapter)


def card(tags: str, body: str) -> str:
    return "---\ntags: [%s]\ndate: 2026-11-15\n---\n%s" % (tags, body)


def run_rust_sync(workspace: Path, vault: Path, log: Path) -> tuple[dict, str]:
    cargo = CARGO if CARGO.is_file() else Path(shutil.which("cargo") or "cargo")
    env = dict(os.environ)
    env["PATH"] = str(Path.home() / ".cargo/bin") + os.pathsep + env.get("PATH", "")
    env["OMNIDESK_SYNC152_WS"] = str(workspace)
    env["OMNIDESK_SYNC152_VAULT"] = str(vault)
    proc = subprocess.run(
        [str(cargo), "test", "--lib", "knowledge_sync_152_e2e", "--", "--nocapture"],
        cwd=REPO / "desktop/src-tauri",
        env=env,
        text=True,
        capture_output=True,
        timeout=600,
    )
    output = proc.stdout + proc.stderr
    log.write_text(output, encoding="utf-8")
    if proc.returncode != 0:
        raise RuntimeError("cargo test 失败，见 " + str(log))
    for line in output.splitlines():
        if line.startswith("SYNC152_REPORT:"):
            return json.loads(line[len("SYNC152_REPORT:") :]), output
    raise RuntimeError("cargo test 输出里没有 SYNC152_REPORT")


def index_rows(index: str, link: str) -> int:
    return sum(1 for line in index.splitlines() if link in line)


def e2e_checks() -> None:
    base = Path(tempfile.mkdtemp(prefix="omnidesk-sync152-verify-"))
    try:
        workspace = base / "workspace"
        vault = base / "vault"
        staging = workspace / "staging/kb/知识库"
        (staging / "概念").mkdir(parents=True)
        (staging / "方法").mkdir(parents=True)
        vault.mkdir()

        first_path = staging / "概念/合成卡一.md"
        second_path = staging / "方法/合成卡二.md"
        first_text = card("合成, 概念", "合成定义。\n\n## 展开\n只用于验证同步通道。")
        second_text = card("合成, 方法", "合成方法：先列步骤，再写结论。")
        first_path.write_text(first_text, encoding="utf-8")
        second_path.write_text(second_text, encoding="utf-8")

        report1, _ = run_rust_sync(workspace, vault, EVIDENCE / "e2e_run1.txt")
        check("e2e_run1_ok", report1.get("ok") is True, json.dumps(report1, ensure_ascii=False))
        check("e2e_run1_created_two", len(report1.get("created", [])) == 2)
        check("e2e_run1_index_updated", report1.get("index_updated") is True)
        landed = [vault / "知识库/概念/合成卡一.md", vault / "知识库/方法/合成卡二.md"]
        check("e2e_cards_landed", all(path.is_file() for path in landed))
        check("e2e_staging_cleaned", not first_path.exists() and not second_path.exists())
        index_path = vault / "知识库/索引.md"
        index = index_path.read_text(encoding="utf-8") if index_path.is_file() else ""
        check("e2e_index_has_two", index_rows(index, "(概念/合成卡一.md)") == 1 and index_rows(index, "(方法/合成卡二.md)") == 1, index[:200])
        check("e2e_body_under_target", len("合成定义。\n\n## 展开\n只用于验证同步通道。".replace("\n", "")) <= 320)

        # 第二轮：同路径冲突 + 超限卡；两者都必须保留暂存文件、不覆盖库内内容。
        first_path.parent.mkdir(parents=True, exist_ok=True)
        (staging / "方法").mkdir(parents=True, exist_ok=True)
        first_path.write_text(card("合成, 概念", "第二次内容，不许覆盖。"), encoding="utf-8")
        over_path = staging / "方法/超限卡.md"
        over_path.write_text(card("超限, 方法", "长" * 421), encoding="utf-8")
        report2, _ = run_rust_sync(workspace, vault, EVIDENCE / "e2e_run2.txt")
        check("e2e_run2_not_ok", report2.get("ok") is False, json.dumps(report2, ensure_ascii=False))
        check("e2e_run2_conflict", len(report2.get("conflicts", [])) == 1, json.dumps(report2, ensure_ascii=False))
        check("e2e_run2_error", len(report2.get("errors", [])) == 1, json.dumps(report2, ensure_ascii=False))
        check("e2e_run2_created_none", not report2.get("created"))
        check("e2e_run2_no_overwrite", "合成定义" in landed[0].read_text(encoding="utf-8"))
        check("e2e_run2_staging_kept", first_path.exists() and over_path.exists())
        index2 = index_path.read_text(encoding="utf-8")
        check("e2e_index_no_duplicates", index_rows(index2, "(概念/合成卡一.md)") == 1)
        check("e2e_overlimit_not_landed", not (vault / "知识库/方法/超限卡.md").exists())
    finally:
        shutil.rmtree(base, ignore_errors=True)


def main() -> int:
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    try:
        static_checks()
    except Exception as exc:  # 静态检查自身失败也要如实记录
        check("static_checks_ran", False, repr(exc))
    try:
        e2e_checks()
    except Exception as exc:
        check("e2e_ran", False, repr(exc))

    for case in CASES:
        print("[%s] %s%s" % (
            "pass" if case["ok"] else "fail",
            case["name"],
            (" " + str(case["detail"])) if case["detail"] else "",
        ))
    failed = [case for case in CASES if not case["ok"]]
    (EVIDENCE / "checks.txt").write_text(
        "\n".join(
            "[%s] %s%s" % (
                "pass" if case["ok"] else "fail",
                case["name"],
                (" " + str(case["detail"])) if case["detail"] else "",
            )
            for case in CASES
        )
        + "\n",
        encoding="utf-8",
    )
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
