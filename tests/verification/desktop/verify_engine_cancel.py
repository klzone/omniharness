#!/usr/bin/env python3
"""OD-ARCH-016：工作台“取消引擎任务”入口的离线验证。

只验证两件可离线证明的事：

1. 取消按钮状态模型与取消结果文案：从 `desktop/src/workbench.tsx` 的
   `engine-cancel-model` 区域抽取真实源码，编译后用 Node 实际执行；
2. 静态源码边界：按钮只受取消状态模型控制、取消请求有同步防重入、
   取消路径不改写解释正文。

**不**验证真实宿主取消、真实模型任务或窗口交互；那些属于运行测试。
不读取凭证，不访问网络，不启动引擎，不启动用户正在使用的实例。

用法：
  python3 tests/verification/desktop/verify_engine_cancel.py [--json PATH]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
DESKTOP = HERE.parents[2] / "desktop"
WORKBENCH = DESKTOP / "src" / "workbench.tsx"

BEGIN = "// #region engine-cancel-model"
END = "// #endregion engine-cancel-model"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd, cwd=str(cwd), capture_output=True, text=True, timeout=180
    )


def extract_region(source: str) -> str:
    start = source.find(BEGIN)
    stop = source.find(END)
    if start < 0 or stop < 0 or stop <= start:
        raise SystemExit("找不到 engine-cancel-model 区域，源码结构已变化。")
    return source[start : stop + len(END)] + "\n"


def body_of(source: str, name: str) -> str:
    """取出某个 useCallback 常量的函数体，用于静态边界检查。"""
    marker = "const " + name + " = useCallback(async () => {"
    start = source.find(marker)
    if start < 0:
        marker = "export async function " + name + "("
        start = source.find(marker)
    if start < 0:
        raise SystemExit(f"找不到 {name} 的实现。")
    end = source.find("\n}\n", start) if marker.startswith("export") else source.find("});", start)
    if end < 0:
        raise SystemExit(f"{name} 的实现没有被正确闭合。")
    return source[start:end]


def compile_and_probe(region: str) -> dict:
    """把抽取出的真实源码编译到临时目录，用 Node 跑全部确定性分支。"""
    tmp = Path(tempfile.mkdtemp(prefix="od-cancel-"))
    src = tmp / "engineCancelModel.ts"
    src.write_text(region, encoding="utf-8")
    compile_result = run(
        [
            "npx",
            "tsc",
            str(src),
            "--outDir",
            str(tmp / "out"),
            "--module",
            "commonjs",
            "--target",
            "ES2022",
            "--skipLibCheck",
            "--ignoreConfig",
        ],
        DESKTOP,
    )
    compiled = tmp / "out" / "engineCancelModel.js"
    if compile_result.returncode != 0 or not compiled.exists():
        return {
            "compile_exit": compile_result.returncode,
            "compile_stderr": compile_result.stderr[-2000:],
            "probe_exit": None,
            "cases": [],
        }

    probe = tmp / "probe.cjs"
    probe.write_text(
        "const m = require(" + json.dumps(str(compiled)) + ");\n"
        "const out = [];\n"
        "function btn(name, input, expect) {\n"
        "  const got = m.engineCancelButtonState(input);\n"
        "  out.push({ kind: 'button', name, passed:\n"
        "    got.enabled === expect.enabled &&\n"
        "    (expect.label === undefined || got.label === expect.label),\n"
        "    got, expect });\n"
        "}\n"
        "function msg(name, input, expect) {\n"
        "  const got = m.describeEngineCancelOutcome(input);\n"
        "  out.push({ kind: 'message', name, passed:\n"
        "    got.confirmed === expect.confirmed &&\n"
        "    (expect.contains === undefined || got.message.includes(expect.contains)),\n"
        "    got, expect });\n"
        "}\n"
"const base = { hasTask: true, status: 'running', requestInFlight: false, lastPhase: null, requestSent: false };\n"
        "btn('no-task', { ...base, hasTask: false, status: null }, { enabled: false });\n"
        "btn('no-status', { ...base, status: null }, { enabled: false, label: '等待任务状态…' });\n"
        "btn('running', { ...base }, { enabled: true, label: '取消引擎任务' });\n"
        "btn('queued', { ...base, status: 'queued' }, { enabled: true });\n"
        "btn('starting', { ...base, status: 'starting' }, { enabled: true });\n"
"btn('cancelling', { ...base, status: 'cancelling', requestSent: true }, { enabled: false, label: '等待取消结果…' });\n"
"btn('cancelling-request-not-sent', { ...base, status: 'cancelling' }, { enabled: false, label: '等待取消结果…' });\n"
"btn('request-sent-host-lagging', { ...base, status: 'running', requestSent: true }, { enabled: false, label: '等待取消结果…' });\n"
        "btn('in-flight', { ...base, requestInFlight: true }, { enabled: false, label: '正在请求取消…' });\n"
        "btn('in-flight-terminal', { ...base, status: 'succeeded', requestInFlight: true }, { enabled: false, label: '正在请求取消…' });\n"
        "btn('succeeded', { ...base, status: 'succeeded' }, { enabled: false, label: '任务已结束' });\n"
        "btn('failed', { ...base, status: 'failed' }, { enabled: false });\n"
        "btn('cancelled', { ...base, status: 'cancelled' }, { enabled: false });\n"
        "btn('interrupted', { ...base, status: 'interrupted' }, { enabled: false });\n"
        "btn('unknown', { ...base, status: 'unknown' }, { enabled: false });\n"
"btn('retry-after-timeout', { ...base, status: 'cancelling', lastPhase: 'timeout_unknown', requestSent: true }, { enabled: true, label: '重试取消' });\n"
"btn('retry-in-flight', { ...base, status: 'cancelling', lastPhase: 'timeout_unknown', requestSent: true, requestInFlight: true }, { enabled: false });\n"
        "msg('confirmed', { phase: 'confirmed_cancelled', final_state: null, note: '' }, { confirmed: true, contains: '取消已确认' });\n"
        "msg('final-cancelled', { phase: 'request_sent', final_state: 'cancelled', note: '' }, { confirmed: true, contains: '取消已确认' });\n"
        "msg('race-succeeded', { phase: 'request_sent', final_state: 'succeeded', note: '任务已完成' }, { confirmed: false, contains: 'succeeded' });\n"
        "msg('timeout', { phase: 'timeout_unknown', final_state: null, note: '' }, { confirmed: false, contains: '取消未确认' });\n"
        "msg('acknowledged', { phase: 'acknowledged', final_state: 'running', note: '' }, { confirmed: false, contains: '取消未确认' });\n"
        "msg('failed-final', { phase: 'request_sent', final_state: 'failed', note: '' }, { confirmed: false });\n"
        "(async () => {\n"
        "  const ipc = { cancel: 0, status: 0 };\n"
        "  async function cancelCase(name, snapshot, behaviour, expect) {\n"
        "    ipc.cancel = 0; ipc.status = 0;\n"
        "    const seen = { started: 0, settled: 0, outcome: null, status: null, error: null };\n"
        "    const fake = {\n"
        "      async cancelTask() { ipc.cancel += 1; if (behaviour.cancelThrows) throw new Error('synthetic-cancel-failure'); return behaviour.outcome; },\n"
        "      async taskStatus() { ipc.status += 1; if (behaviour.statusThrows) throw new Error('synthetic-status-failure'); return behaviour.status; },\n"
        "    };\n"
        "    const result = await m.runEngineCancelRequest(snapshot, {\n"
        "      host: fake,\n"
        "      onRequestStarted: () => { seen.started += 1; },\n"
        "      onOutcome: (o) => { seen.outcome = o; },\n"
        "      onStatus: (s) => { seen.status = s; },\n"
        "      onError: (e) => { seen.error = e; },\n"
        "      onSettled: () => { seen.settled += 1; },\n"
        "    });\n"
        "    const msg = seen.outcome ? m.describeEngineCancelOutcome(seen.outcome) : null;\n"
        "    out.push({ kind: 'ipc', name, passed:\n"
        "      ipc.cancel === expect.cancel && ipc.status === expect.status &&\n"
        "      result === expect.result && seen.started === expect.started &&\n"
        "      seen.settled === expect.settled &&\n"
        "      (expect.confirmed === undefined || (msg && msg.confirmed === expect.confirmed)) &&\n"
        "      (expect.statusValue === undefined || (seen.status && seen.status.status === expect.statusValue)) &&\n"
        "      (expect.errorContains === undefined || (seen.error && seen.error.includes(expect.errorContains))),\n"
        "      got: { ipc, result, started: seen.started, settled: seen.settled, status: seen.status, error: seen.error }, expect });\n"
        "  }\n"
        "  const run = { taskId: 'omni-synthetic-1', status: 'running', requestInFlight: false, lastPhase: null, requestSent: false };\n"
        "  await cancelCase('first-cancel', run,\n"
        "    { outcome: { phase: 'request_sent', final_state: null, note: '' }, status: { status: 'cancelling' } },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, confirmed: false, statusValue: 'cancelling' });\n"
        "  await cancelCase('repeat-click-while-waiting', { ...run, requestInFlight: true },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 0, status: 0, result: 'skipped', started: 0, settled: 0 });\n"
        "  await cancelCase('repeat-click-host-lagging', { ...run, requestSent: true },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 0, status: 0, result: 'skipped', started: 0, settled: 0 });\n"
        "  await cancelCase('retry-after-timeout', { ...run, status: 'cancelling', lastPhase: 'timeout_unknown', requestSent: true },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, confirmed: true, statusValue: 'cancelled' });\n"
        "  await cancelCase('already-succeeded', { ...run, status: 'succeeded' },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 0, status: 0, result: 'skipped', started: 0, settled: 0 });\n"
        "  await cancelCase('task-failed-first', { ...run, status: 'failed' },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 0, status: 0, result: 'skipped', started: 0, settled: 0 });\n"
        "  await cancelCase('cancel-not-confirmed', run,\n"
        "    { outcome: { phase: 'acknowledged', final_state: 'running', note: '' }, status: { status: 'cancelling' } },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, confirmed: false, statusValue: 'cancelling' });\n"
        "  await cancelCase('race-completed-first', run,\n"
        "    { outcome: { phase: 'request_sent', final_state: 'succeeded', note: '任务已完成' }, status: { status: 'succeeded' } },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, confirmed: false, statusValue: 'succeeded' });\n"
        "  await cancelCase('cancel-ipc-failure', run,\n"
        "    { cancelThrows: true, status: { status: 'running' } },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, errorContains: '未获确认', statusValue: 'running' });\n"
        "  await cancelCase('status-readback-failure', run,\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, statusThrows: true },\n"
        "    { cancel: 1, status: 1, result: 'sent', started: 1, settled: 1, confirmed: true });\n"
        "  await cancelCase('no-task', { ...run, taskId: null, status: null },\n"
        "    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: { status: 'cancelled' } },\n"
        "    { cancel: 0, status: 0, result: 'skipped', started: 0, settled: 0 });\n"
        "  process.stdout.write(JSON.stringify(out));\n"
        "})().catch((e) => { process.stderr.write(String(e)); process.exit(1); });\n",
        encoding="utf-8",
    )
    probe_result = run(["node", str(probe)], tmp)
    try:
        cases = json.loads(probe_result.stdout) if probe_result.stdout else []
    except json.JSONDecodeError:
        cases = []
    return {
        "compile_exit": compile_result.returncode,
        "probe_exit": probe_result.returncode,
        "probe_stderr": probe_result.stderr[-2000:],
        "cases": cases,
    }


def static_checks(source: str) -> list[dict]:
    checks: list[dict] = []
    region_of_gate = extract_region(source)

    def check(name: str, passed: bool, detail: str) -> None:
        checks.append({"name": name, "passed": passed, "detail": detail})

    button_attr = re.search(
        r"onClick=\{cancelEngineExplanation\}\s+disabled=\{([^}]+)\}", source
    )
    check(
        "取消按钮只由取消状态模型控制",
        bool(button_attr) and button_attr.group(1).strip() == "!engineCancelView.enabled",
        "disabled=" + (button_attr.group(1).strip() if button_attr else "<未找到>"),
    )
    check(
        "取消按钮未被 engineBusy 阻塞",
        "disabled={!engineCancelView.enabled || engineBusy}" not in source,
        "按钮 disabled 表达式中不含 engineBusy",
    )

    cancel_body = body_of(source, "cancelEngineExplanation")
    cancel_body = cancel_body[: cancel_body.find("\n  }, [")] if "\n  }, [" in cancel_body else cancel_body
    runner = body_of(source, "runEngineCancelRequest")
    guard = runner.find("effects.onRequestStarted();")
    first_await = runner.find("await effects.host.cancelTask")
    reset = runner.find("effects.onSettled();", first_await)
    check(
        "取消请求有同步防重入锁",
        guard >= 0 and first_await > guard and reset > first_await,
        "置位在首个 await 之前，复位在 await 之后",
    )
    check(
        "取消路径不改写解释正文与来源",
        "setExplanation" not in cancel_body and "setSource" not in cancel_body,
        "cancelEngineExplanation 内无 setExplanation/setSource",
    )
    check(
        "取消中只有明确超时才允许重试",
        'input.status === "cancelling"' in region_of_gate
        and 'input.lastPhase === "timeout_unknown"' in region_of_gate,
        "普通 cancelling 在点击入口即被拒绝，仅 timeout_unknown 放行一次重试",
    )
    check(
        "已发出取消请求后不重复发送",
        'if (input.requestSent) return { send: false, reason: "already_requested" };'
        in region_of_gate,
        "宿主状态滞后（仍为 running）时按 requestSent 拒绝第二次请求",
    )
    check(
        "取消入口调用共享发送路径而非直接 IPC",
        "runEngineCancelRequest(" in cancel_body
        and "host.cancelTask" not in cancel_body,
        "cancelEngineExplanation 内无直接 host.cancelTask 调用",
    )
    check(
        "只在宿主确认后展示取消成功",
        "describeEngineCancelOutcome(engineCancel)" in source,
        "取消结果文案来自 describeEngineCancelOutcome",
    )
    return checks


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args()

    source = WORKBENCH.read_text(encoding="utf-8")
    region = extract_region(source)
    probe = compile_and_probe(region)
    checks = static_checks(source)

    cases = probe.get("cases", [])
    failed_cases = [c for c in cases if not c.get("passed")]
    failed_checks = [c for c in checks if not c["passed"]]
    ok = (
        probe.get("compile_exit") == 0
        and probe.get("probe_exit") == 0
        and cases
        and not failed_cases
        and not failed_checks
    )

    report = {
        "plan": "OD-ARCH-016",
        "evidence_level": "code static check + compiled source execution (no engine/model run)",
        "source": str(WORKBENCH),
        "compile_exit": probe.get("compile_exit"),
        "probe_exit": probe.get("probe_exit"),
        "state_cases_total": len(cases),
        "state_cases_failed": len(failed_cases),
        "static_checks_total": len(checks),
        "static_checks_failed": len(failed_checks),
        "static_checks": checks,
        "state_cases": cases,
        "result": "pass" if ok else "fail",
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(
        f"state cases: {len(cases)} total, {len(failed_cases)} failed; "
        f"static checks: {len(checks)} total, {len(failed_checks)} failed; "
        f"compile_exit={probe.get('compile_exit')} probe_exit={probe.get('probe_exit')}"
    )
    for c in failed_cases:
        print("  FAIL case:", c["name"], json.dumps(c, ensure_ascii=False))
    for c in failed_checks:
        print("  FAIL check:", c["name"], c["detail"])
    print("RESULT:", report["result"])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
