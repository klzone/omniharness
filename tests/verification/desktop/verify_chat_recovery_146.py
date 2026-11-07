#!/usr/bin/env python3
"""OD-ARCH-146：对话后台存活、输入框换行、整理入口迁移的静态接线检查。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
THREADS = REPO / "desktop/src/sceneThreads.ts"
HOME = REPO / "desktop/src/shell/HomeChat.tsx"
CANVAS = REPO / "desktop/src/shell/HomeCanvas.tsx"
VAULT = REPO / "desktop/src/shell/KnowledgeVaultPage.tsx"
PANEL = REPO / "desktop/src/shell/KnowledgeIngestPanel.tsx"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    threads = THREADS.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    canvas = CANVAS.read_text(encoding="utf-8")
    vault = VAULT.read_text(encoding="utf-8")
    panel = PANEL.read_text(encoding="utf-8")

    # S1：在途任务跨页面存续
    check("thread_has_inflight_field", "inflightTaskId?: string | null;" in threads)
    check("thread_inflight_setter", "export function setThreadInflightTask" in threads)
    check("canvas_passes_inflight", "inflightTaskId={active?.inflightTaskId ?? null}" in canvas)
    check("canvas_writes_inflight", "setThreadInflightTask(id, taskId)" in canvas)
    check("home_accepts_inflight", "inflightTaskId = null" in home)
    check("home_restores_polling", "setActiveTaskId(taskId);" in home and "OD-ARCH-146 S1" in home)
    check("home_restores_placeholder", "prev.some((m) => m.taskId === taskId)" in home)
    check("home_records_task_id", "inflightCbRef.current?.(receipt.omni_task_id);" in home)
    check("home_clears_on_settle", "inflightCbRef.current?.(null);" in home)

    # S2：输入框多行自适应
    check("composer_is_textarea", "<textarea" in home and "<input" not in home)
    check("composer_testid", 'data-testid="chat-input"' in home)
    check("composer_autogrow", "Math.min(el.scrollHeight, 96)" in home)
    check("composer_shift_enter", 'e.key === "Enter" && !e.shiftKey' in home)

    # S3：整理入口迁移
    check("home_no_ingest_panel", "<KnowledgeIngestPanel" not in home)
    check("vault_has_ingest_panel", "<KnowledgeIngestPanel onApplied=" in vault)
    check("ingest_refreshes_scan", "onApplied?.();" in panel)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
