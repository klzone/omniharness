#!/usr/bin/env python3
"""OD-KB-202：暂存知识卡进入「知识审核」再入库的静态与单元验证。"""

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
VAULT = ROOT / "desktop" / "src-tauri" / "src" / "knowledge_vault.rs"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
API = ROOT / "desktop" / "src" / "api.ts"
PAGE = ROOT / "desktop" / "src" / "shell" / "KnowledgeVaultPage.tsx"
INBOX = ROOT / "desktop" / "src" / "shell" / "KnowledgeStagingInbox.tsx"
INGEST = ROOT / "desktop" / "src" / "shell" / "KnowledgeIngestPanel.tsx"

FAILS = []
TOTAL = 0


def check(name: str, ok: bool, detail: str = "") -> None:
    global TOTAL
    TOTAL += 1
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    vault = VAULT.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    page = PAGE.read_text(encoding="utf-8")
    inbox = INBOX.read_text(encoding="utf-8")
    ingest = INGEST.read_text(encoding="utf-8")

    check("list_function", "pub fn list_staged_cards" in vault)
    check("sync_selected_function", "pub fn sync_selected_from_workspace" in vault)
    check("discard_function", "pub fn discard_staged_cards" in vault)
    check("staging_view_types", "pub struct StagedCardView" in vault and "pub struct StagingList" in vault)
    check("selected_filter", "let mut staged" in vault and "staged.retain" in vault and "available" in vault)
    check("path_guard", "check_staged_card_path" in vault and "暂存路径是符号链接，拒绝删除" in vault)
    check("commands_registered", "knowledge_staging_list" in lib and "knowledge_staging_sync" in lib and "knowledge_staging_discard" in lib)
    check("governance_trace", "approve_card" in lib and "来源 staging" in lib)
    check("api_methods", "knowledgeStagingList" in api and "knowledgeStagingSync" in api and "knowledgeStagingDiscard" in api)
    check("review_page_mounts_inbox", "KnowledgeStagingInbox" in page and "暂存卡待审核" in inbox)
    check("inbox_actions", "入库选中" in inbox and "丢弃选中" in inbox and "全部入库" in inbox)
    check("inbox_confirm", 'confirming === "all"' in inbox and "确认" in inbox)
    check("ingest_panel_confirm_all", "确认全部入库" in ingest)

    # 真实跑一次 Rust 定向单测：选中同步只入库勾选卡、未选保留、丢弃与路径穿越拒绝。
    env = dict(os.environ)
    env["PATH"] = str(pathlib.Path.home() / ".cargo" / "bin") + ":" + env.get("PATH", "")
    env["OMNIDESK_REPO_ROOT"] = str(ROOT)
    result = subprocess.run(
        [
            "cargo",
            "test",
            "--offline",
            "--lib",
            "knowledge_vault::tests::staging_list_selected_sync_and_discard",
            "--",
            "--test-threads=1",
        ],
        cwd=ROOT / "desktop" / "src-tauri",
        env=env,
        capture_output=True,
        text=True,
        timeout=300,
    )
    tail = (result.stdout + result.stderr).strip().splitlines()[-3:]
    check("rust_staging_test", result.returncode == 0, " | ".join(tail))

    print(f"cases={TOTAL} failures={len(FAILS)}")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())

