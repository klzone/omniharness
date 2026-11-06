#!/usr/bin/env python3
"""OD-ARCH-145：starting 兜底、取消清会话、引擎 stderr 日志的静态断言。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HOME = (REPO / "desktop/src/shell/HomeChat.tsx").read_text(encoding="utf-8")
CANVAS = (REPO / "desktop/src/shell/HomeCanvas.tsx").read_text(encoding="utf-8")
THREADS = (REPO / "desktop/src/sceneThreads.ts").read_text(encoding="utf-8")
IPC = (REPO / "desktop/src-tauri/src/engine_ipc.rs").read_text(encoding="utf-8")
SUPERVISOR = (REPO / "desktop/src-tauri/src/engine_supervisor.rs").read_text(encoding="utf-8")
ADAPTER = (REPO / "engine/omnidesk_engine/web_api_adapter.py").read_text(encoding="utf-8")


def main() -> int:
    cases = [
        ("stderr_log_path", "engine-stderr.log" in SUPERVISOR and "stderr_log_path" in SUPERVISOR),
        ("stderr_rotation", "ENGINE_STDERR_MAX_BYTES" in IPC and "rotate_stderr_log" in IPC),
        ("stderr_spawn_wired", "spawn_with_stderr_log" in SUPERVISOR and "spawn_with_stderr_log" in IPC),
        ("starting_timeout_hint", 'data-testid="starting-timeout-hint"' in HOME),
        ("starting_timeout_120s", "120_000" in HOME and "startingTooLong" in HOME),
        ("starting_hint_no_auto_kill", "可点取消重试" in HOME and "连续两次建议重启应用" in HOME),
        ("cancel_clears_session", "setLocalEngineSessionId(null)" in HOME and "onEngineSessionId?.(null)" in HOME),
        ("cancel_replays_history", "setReplayNextTurn(true)" in HOME),
        ("thread_session_nullable", "setThreadEngineSession(id: string, engineSessionId: string | null)" in THREADS),
        ("canvas_accepts_null", "setThreadEngineSession(id, sid)" in CANVAS),
        ("engine_starting_cancel", "was_starting" in ADAPTER and "TaskState.FAILED" in ADAPTER),
        ("cancel_marked_confirmed", "CONFIRMED_CANCELLED" in ADAPTER),
    ]
    failed = [name for name, ok in cases if not ok]
    for name, ok in cases:
        print(("PASS  " if ok else "FAIL  ") + name)
    print("%d/%d 通过" % (len(cases) - len(failed), len(cases)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

