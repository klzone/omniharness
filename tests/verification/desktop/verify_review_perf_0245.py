#!/usr/bin/env python3
"""0.2.45 周复盘性能修复：只读探测、编码统一、getpgrp 容错与分段计时。"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "desktop" / "src"
TAURI = ROOT / "desktop" / "src-tauri" / "src"
ENGINE = ROOT / "engine" / "omnidesk_engine"

FAILS = []
CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append(name)
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    app = (SRC / "App.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    chat = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    lib = (TAURI / "lib.rs").read_text(encoding="utf-8")
    bridge = (TAURI / "engine_bridge.rs").read_text(encoding="utf-8")
    supervisor = (TAURI / "engine_supervisor.rs").read_text(encoding="utf-8")
    ipc = (TAURI / "engine_ipc.rs").read_text(encoding="utf-8")
    owner = (ENGINE / "process_owner.py").read_text(encoding="utf-8")
    web = (ENGINE / "web_api_adapter.py").read_text(encoding="utf-8")
    serve = (ENGINE / "serve.py").read_text(encoding="utf-8")
    po_test = (ROOT / "tests" / "verification" / "engine-interface" / "adapter" / "test_process_owner_unit.py").read_text(encoding="utf-8")

    # 1. 轮询不得拉起引擎。
    check("宿主只读描述命令", "async fn engine_describe_if_running" in lib and "engine_describe_if_running," in lib)
    check("桥接只读方法", "pub fn descriptor_if_initialized" in bridge and "self.initialized" in bridge)
    check("前端只读 API", "describeIfRunning(): Promise<EngineDescriptor | null>" in api and '"engine_describe_if_running"' in api)
    probe = app.split("probeEngine", 1)[-1].split("selectScene", 1)[0]
    check("App 首探只读并单次预热", "host.describeIfRunning()" in probe and "engineWarmupStarted.current" in probe and "d = await host.describe()" in probe)
    check("App 轮询使用只读", "const d = await host.describeIfRunning();" in app)
    check("引擎未启动文案", '"引擎尚未启动"' in app)

    # 2. 编码统一。
    check("引擎子进程固定 UTF-8", '"PYTHONUTF8"' in supervisor and '"PYTHONIOENCODING"' in supervisor and "utf-8:replace" in supervisor)
    check("DSH stdout 固定 UTF-8", 'encoding="utf-8"' in web and 'errors="replace"' in web)
    check("协议流固定 UTF-8", "stream.reconfigure(encoding=\"utf-8\"" in serve)
    check("Rust stdout 坏字节不终止线程", "read_until(b'\\n', &mut raw)" in ipc and "from_utf8_lossy(&raw)" in ipc)
    check("Rust stderr 坏字节不终止线程", "from_utf8_lossy(&raw).trim_end()" in ipc)
    check("坏字节单测", "stderr_reader_survives_non_utf8_bytes" in ipc)

    # 3. getpgrp 容错。
    check("非 POSIX 不调 getpgrp", 'if os.name != "posix":\n        return 0' in owner)
    check("getpgrp OSError 回退", "except (AttributeError, OSError):" in owner)
    check("getpgrp 单测", "test_current_pgid_falls_back_when_getpgrp_fails" in po_test)

    # 4. 分段计时。
    check("消息包含分段计时", "timing?: {" in chat and "classifyMs" in chat and "materialMs" in chat and "submitMs" in chat)
    check("判定/素材/提交计时", "classifyMs: Math.max(0, routedStartedAt - startedAtMs)" in chat and "materialMs: Math.max(0, submitStartedAt - routedStartedAt)" in chat and "materialMs: Math.max(0, submitStartedAt - turnStartedAtMs)" in chat)
    check("分类上下文保留起点", "startedAtMs: number;" in chat and "startedAtMs: turnStartedAtMs" in chat)
    check("路由提交使用原始起点", "submitRoutedWorkIntent(route.action, ctx.userText, ctx.picked, target, ctx.startedAtMs)" in chat)
    check("分段计时展示", "判定 {durationText(m.timing.classifyMs)" in chat and "素材 {durationText(m.timing.materialMs)" in chat and "· 提交 {durationText(m.timing.submitMs)" in chat)
    check("总耗时仍保留", "elapsedText(m.startedAtMs, m.endedAtMs)" in chat)

    print(f"cases={len(CASES)} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
