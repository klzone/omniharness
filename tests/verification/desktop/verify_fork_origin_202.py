#!/usr/bin/env python3
"""复核 forkOrigin 线程字段、分支提示条与 host 原样落盘约束。"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    threads = read(SRC / "sceneThreads.ts")
    chat = read(SRC / "shell" / "HomeChat.tsx")
    canvas = read(SRC / "shell" / "HomeCanvas.tsx")
    host = read(REPO / "desktop" / "src-tauri" / "src" / "thread_store.rs")

    check("fork_origin_type",
          "interface ForkOrigin" in threads
          and "parentSessionId: string" in threads
          and "atSeq: number | null" in threads,
          "ForkOrigin 类型包含父会话与切点")
    check("fork_origin_normalized_on_read",
          "normalizeForkOrigin(" in threads
          and "raw.parentSessionId" in threads
          and "Number.isSafeInteger(raw.atSeq)" in threads,
          "读回线程时拒绝畸形来源，不猜父会话")
    check("fork_origin_normalized_on_create",
          "forkOrigin: normalizeForkOrigin(input.forkOrigin)" in threads,
          "新建线程时同样归一化，避免当前会话短暂显示空父会话")
    check("branch_callback_has_origin",
          "onBranchNewThread?: (" in chat
          and "origin: ForkOrigin" in chat,
          "HomeChat 分支回调携带 ForkOrigin")
    check("branch_reports_parent_and_seq",
          "parentSessionId:" in chat
          and "atSeq:" in chat
          and "fork.parent_session_id" in chat,
          "分支成功时记录父会话与切点")
    check("fork_banner_rendered",
          'data-testid="fork-origin-banner"' in chat
          and "由分支创建" in chat,
          "新线程顶部有一行分支来源说明")
    banner_block = (
        chat.split("{forkOrigin ? (", 1)[1].split(") : null}", 1)[0]
        if "{forkOrigin ? (" in chat
        else ""
    )
    check("fork_banner_is_inline",
          "mb-1 flex shrink-0 items-center gap-1.5 px-1 text-[10.5px] leading-relaxed text-outline" in banner_block
          and "border border-outline-variant" not in banner_block
          and "bg-surface-container-low" not in banner_block,
          "分支来源是灰色小字，不渲染成边框卡片")
    check("home_passes_fork_origin",
          "forkOrigin={active?.forkOrigin ?? null}" in canvas,
          "首页把线程来源传入对话区")
    check("host_preserves_unknown_field",
          "serde_json::Value" in host
          and "unknown_frontend_fields_survive_save_and_load" in host,
          "Rust 线程存储有原样保留未知字段的回归测试")

    passed = sum(1 for _, ok, _ in CHECKS if ok)
    total = len(CHECKS)
    for name, ok, detail in CHECKS:
        print(("PASS" if ok else "FAIL") + f" {name}" + (f" - {detail}" if detail else ""))
    print()
    print(f"{passed}/{total} checks passed")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
