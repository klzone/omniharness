#!/usr/bin/env python3
"""OD-SYS-703 等待计时器与引擎空闲提示：静态源码检查。

覆盖：
  1. 计时器起点取活跃任务消息上的 startedAtMs，不再用组件挂载时刻；
  2. 切页恢复时立即用真实起点算出已等待秒数，并随 activeStartedAtMs 重建；
  3. 无活跃任务且最近一次 taskStatus 轮询已到终态时，显示引擎闲置提示；
  4. 提示只读引擎健康并引导用户去设置页，不包含任何自动停止逻辑；
  5. runtime/ 零改动，本次改动未新增 lint/测试豁免。

证据等级：静态源码检查（不启动窗口、不联网）。

用法：python3 tests/verification/desktop/verify_sys_timer_703.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
HOME_CHAT = REPO / "desktop" / "src" / "shell" / "HomeChat.tsx"
SETTINGS = REPO / "desktop" / "src" / "SettingsPanel.tsx"
EVID = HERE / "od-sys-703"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def git_lines(*args: str) -> list[str]:
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        return [line for line in result.stdout.splitlines() if line.strip()]
    except OSError:
        return ["git-unavailable"]


def timer_block(source: str) -> str:
    """只截取「生成中的计时」effect，避免把别处的 Date.now() 误算进来。"""
    start = source.find("// 生成中的计时")
    if start < 0:
        return ""
    tail = source[start:]
    end_marker = "[activeTaskId, activeStartedAtMs]);"
    end = tail.find(end_marker)
    if end < 0:
        return ""
    return tail[: end + len(end_marker)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path, default=EVID / "verify_sys_timer_703.json")
    args = ap.parse_args()

    source = HOME_CHAT.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    block = timer_block(source)

    check(
        "timer_reads_message_started_at",
        "const activeStartedAtMs = useMemo" in source
        and 'm.taskId === activeTaskId && typeof m.startedAtMs === "number"' in source
        and "return hit?.startedAtMs ?? null;" in source,
        "计时起点来自 activeTaskId 对应消息的 startedAtMs",
    )
    check(
        "timer_uses_active_start_not_mount_time",
        bool(block)
        and "Date.now() - activeStartedAtMs" in block
        and "const started = Date.now()" not in block
        and "Date.now() - started" not in block,
        "计时公式为 now - startedAtMs，不再以本次挂载时刻为起点",
    )
    check(
        "timer_rebuilds_on_active_start",
        bool(block)
        and "[activeTaskId, activeStartedAtMs]" in block
        and "window.setInterval(tick, 500)" in block
        and "tick();" in block,
        "effect 依赖 activeStartedAtMs，重建时先立即算一次真实等待秒数",
    )
    check(
        "no_mount_time_start_fallback",
        "startedAtMs: Date.now()" not in source
        and "started = Date.now()" not in source,
        "恢复在途任务时不再伪造挂载时刻作为起点",
    )
    check(
        "idle_hint_text",
        "引擎闲置中，不占用后台计算；可到设置页停止引擎释放内存" in source,
        "提示文案与任务卡一致",
    )
    check(
        "idle_hint_condition",
        "const terminalAt = lastTerminalPollRef.current;" in source
        and "lastTerminalPollRef.current = Date.now();" in source
        and "if (activeTaskId) {" in source
        and "setEngineIdleHint(false);" in source,
        "无活跃任务 + 最近一次 taskStatus 轮询到终态才进入提示；新任务开始即收起",
    )
    check(
        "idle_hint_checks_engine_running",
        "const health = await host.health();" in source
        and "setEngineIdleHint(Boolean(health.running));" in source,
        "显示前读宿主健康事实，引擎未运行时不算闲置",
    )
    check(
        "idle_hint_debounced",
        "const ENGINE_IDLE_HINT_DELAY_MS = 30_000;" in source
        and "const ENGINE_IDLE_RECHECK_MS = 60_000;" in source
        and "ENGINE_IDLE_HINT_DELAY_MS - (Date.now() - terminalAt)" in source,
        "任务结束后静默 30 秒再提示，此后每 60 秒复核引擎是否仍在运行",
    )
    check(
        "idle_hint_render_and_entry",
        'data-testid="engine-idle-hint"' in source
        and 'onNavigate("settings")' in source,
        "提示为轻量一行，并提供用户主动去设置页的入口",
    )
    check(
        "no_auto_stop_logic",
        not any(
            token in source
            for token in (
                "host.shutdownEngine",
                "host.stopEngine",
                "shutdown_engine(",
                "stop_engine(",
                '"engine_stop"',
            )
        ),
        "HomeChat 不包含自动停止引擎的调用；状态变更只由用户触发",
    )
    check(
        "settings_manual_stop_entry",
        "停止引擎释放内存" in settings
        and settings.count("host.shutdownEngine()") == 1
        and 'onClick={() => void stopEngine()}' in settings,
        "设置页的引擎诊断区提供手动停止入口；shutdownEngine 只在一次点击回调里调用",
    )
    check(
        "runtime_unchanged",
        not git_lines("status", "--porcelain", "--", "runtime"),
        "runtime/ 零改动",
    )
    diff = git_lines("diff", "--unified=0", "--", "desktop/src/shell/HomeChat.tsx")
    added = [line[1:].strip() for line in diff if line.startswith("+") and not line.startswith("+++")]
    check(
        "no_new_exemptions",
        not any("eslint-disable" in line or "@ts-ignore" in line for line in added),
        "本次新增行未加入 lint/类型检查豁免",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    for name, ok, detail in CHECKS:
        print(("PASS" if ok else "FAIL") + " " + name + (" — " + detail if detail else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} passed")

    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(
        json.dumps(
            {
                "task": "OD-SYS-703",
                "evidence_level": "static-source",
                "passed": len(CHECKS) - len(failed),
                "total": len(CHECKS),
                "failed": failed,
                "checks": [
                    {"name": name, "ok": ok, "detail": detail}
                    for name, ok, detail in CHECKS
                ],
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
