#!/usr/bin/env python3
"""OD-SYS-701：Windows 进程树回收与僵尸会话清理验收。

macOS 无法真实创建 Windows Job Object，因此本脚本做三件事：
  1. 静态断言 cfg(windows) 下的 Job Object 实现与 EngineChannel 挂接存在；
  2. 静态核对退出路径（窗口销毁 → stop_engine → finish_stop）与 pgid=0 跳过逻辑；
  3. 实跑僵尸记录清理单测（临时目录合成数据）。

真实 Job Object 行为只能在 Windows 出包后人工验证，结果如实登记在报告里。

用法：python3 tests/verification/desktop/verify_sys_process_701.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src-tauri" / "src"
JOB_RS = SRC / "job_object.rs"
IPC_RS = SRC / "engine_ipc.rs"
SUP_RS = SRC / "engine_supervisor.rs"
OWNER_RS = SRC / "process_owner.rs"
LIB_RS = SRC / "lib.rs"
OWNER_PY = REPO / "engine" / "omnidesk_engine" / "process_owner.py"
TASK_PY = REPO / "engine" / "omnidesk_engine" / "task_core.py"
STALE_TEST = HERE / "test_sys_process_701_stale.py"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def fn_body(text: str, signature: str) -> str:
    idx = text.find(signature)
    if idx < 0:
        return ""
    open_idx = text.find("{", idx)
    if open_idx < 0:
        return ""
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_idx : i + 1]
    return ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    for path in (JOB_RS, IPC_RS, SUP_RS, OWNER_RS, LIB_RS, OWNER_PY, TASK_PY, STALE_TEST):
        check(f"exists:{path.name}", path.is_file(), str(path))
    if not all(p.is_file() for p in (JOB_RS, IPC_RS, SUP_RS, OWNER_RS, LIB_RS, OWNER_PY, TASK_PY, STALE_TEST)):
        failed = sum(1 for c in cases if not c["ok"])
        print(json.dumps({"cases": len(cases), "failed": failed, "result": "fail"}, ensure_ascii=False))
        return 1

    job = read(JOB_RS)
    ipc = read(IPC_RS)
    sup = read(SUP_RS)
    owner = read(OWNER_RS)
    lib = read(LIB_RS)
    owner_py = read(OWNER_PY)
    task_py = read(TASK_PY)
    stale_test = read(STALE_TEST)

    # ---- 1. Windows Job Object 实现 -------------------------------------------
    check("cfg_windows_impl", "#[cfg(windows)]" in job, "必须有 Windows 实现")
    check("cfg_not_windows_noop", "#[cfg(not(windows))]" in job, "非 Windows 必须是空实现")
    check(
        "kill_on_job_close_flag",
        "JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE" in job and "0x0000_2000" in job,
        "必须设置 kill-on-close 限制",
    )
    check("create_job_object", "CreateJobObjectW" in job, "必须创建 Job Object")
    check(
        "assign_process_to_job",
        "AssignProcessToJobObject" in job and "OpenProcess" in job,
        "spawn 后必须把子进程挂进 Job",
    )
    check(
        "drop_closes_handle",
        "impl Drop for JobObject" in job and "CloseHandle" in job,
        "Job 句柄必须由 Drop 关闭",
    )
    check("job_send_sync", "unsafe impl Send for JobObject" in job, "句柄需可跨线程持有")

    # ---- 2. EngineChannel 挂接 -------------------------------------------------
    check("lib_declares_job_module", "pub mod job_object;" in lib, "模块必须注册到 crate")
    struct_body = fn_body(ipc, "pub struct EngineChannel")
    check(
        "channel_holds_job",
        "job: crate::job_object::JobObject" in struct_body,
        "Job 句柄必须由 EngineChannel 持有",
    )
    spawn_body = fn_body(ipc, "pub fn spawn_with_stderr_log")
    check(
        "spawn_assigns_job",
        "JobObject::create_for_process(pid)" in spawn_body,
        "引擎 spawn 后必须挂接 Job Object",
    )
    check(
        "spawn_fails_closed",
        "child.kill()" in spawn_body and "Job Object 挂接失败" in spawn_body,
        "挂接失败时清理刚 spawn 的子进程并返回错误",
    )
    terminate_body = fn_body(ipc, "pub fn terminate")
    hard_body = fn_body(ipc, "pub fn hard_kill")
    check(
        "terminate_relies_on_drop",
        "child.kill()" in terminate_body and "CloseHandle" not in terminate_body,
        "terminate 只杀主进程；整树回收由通道 Drop 关闭 Job 句柄完成",
    )
    check(
        "hard_kill_relies_on_drop",
        "child.kill()" in hard_body and "CloseHandle" not in hard_body,
        "hard_kill 不得自行关闭 Job（Drop 链路统一负责）",
    )

    # ---- 3. 退出路径完整 -------------------------------------------------------
    window_body = fn_body(lib, "fn on_window_destroyed_shutdown")
    check("window_shutdown_calls_stop", "stop_engine(state" in window_body, "窗口销毁必须走 stop_engine")
    check(
        "window_destroyed_wired",
        "on_window_destroyed_shutdown(state.inner())" in lib,
        "Destroyed 事件必须接到统一收尾入口",
    )
    stop_body = fn_body(lib, "fn stop_engine")
    check("stop_engine_begin_stop", "begin_stop()" in stop_body, "关闭入口必须先摘除通道")
    check("stop_engine_finish_stop", "EngineSupervisor::finish_stop" in stop_body, "关闭入口必须调用 finish_stop")
    finish_body = fn_body(sup, "pub fn finish_stop")
    check("finish_stop_terminates", ".terminate(grace)" in finish_body, "finish_stop 必须终止主进程")
    check(
        "finish_stop_cleanup_group",
        "cleanup_engine_group" in finish_body,
        "finish_stop 必须保留非 Windows 的进程组兜底",
    )
    check(
        "drop_path_complete",
        "impl Drop for EngineSupervisor" in sup and "cleanup_engine_group" in sup,
        "supervisor 析构路径也必须收尾",
    )
    check(
        "pgid_zero_skips_reclaim",
        "record.pgid <= 0" in fn_body(owner, "pub fn cleanup_engine_group"),
        "宿主必须把 pgid=0 当作无组可回收",
    )
    check(
        "windows_job_object_wording",
        "Windows 由 Job Object 负责回收" in sup and "Windows Job Object" in owner,
        "日志必须照实说明 Windows 由 Job Object 回收",
    )
    current_pgid_py = owner_py.split("def current_pgid", 1)[1].split("\ndef ", 1)[0]
    check(
        "python_current_pgid_zero",
        "return 0" in current_pgid_py and "Windows" in current_pgid_py,
        "process_owner.py 在 Windows 必须返回 0 并说明无 POSIX 进程组",
    )

    # ---- 4. 僵尸记录清理 -------------------------------------------------------
    check(
        "stale_threshold_48h",
        "STALE_TASK_RECORD_SECONDS = 48 * 60 * 60" in task_py,
        "阈值必须是 48 小时",
    )
    check("stale_flag_written", 'data["stale"] = True' in task_py, "必须写回 stale=true")
    check(
        "stale_sweep_on_startup",
        "mark_stale_task_records(self._state_dir)" in task_py,
        "TaskCore 启动时必须扫描 engine-state",
    )
    sweep_body = task_py.split("def mark_stale_task_records", 1)[1].split("class TaskCoreBase", 1)[0]
    check("stale_sweep_keeps_content", '"content"' not in sweep_body, "清理逻辑不得改写 content")
    test_methods = re.findall(r"\n    def (test_[a-zA-Z0-9_]+)\(", stale_test)
    check("stale_tests_at_least_3", len(test_methods) >= 3, f"找到 {len(test_methods)} 项")
    job_tests = len(re.findall(r"\n    #\[test\]\n    fn ", job))
    owner_tests = len(re.findall(r"\n    #\[test\]\n    fn cleanup_", owner))
    check("job_object_tests_present", job_tests >= 3, f"job_object.rs 有 {job_tests} 项")
    check("cleanup_skip_tests_present", owner_tests >= 3, f"process_owner.rs 有 {owner_tests} 项 cleanup 测试")

    # 清理单测实跑，失败即整体红。
    run = subprocess.run(
        [sys.executable, str(STALE_TEST), "-v"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    combined = (run.stdout or "") + (run.stderr or "")
    check("stale_unittest_passes", run.returncode == 0, combined.strip().splitlines()[-1] if combined.strip() else "无输出")

    # runtime/ 是红线：本轮改动不得触碰。
    status = subprocess.run(
        ["git", "status", "--porcelain", "--", "runtime"],
        cwd=str(REPO),
        capture_output=True,
        text=True,
        timeout=30,
    )
    check("runtime_untouched", not (status.stdout or "").strip(), (status.stdout or "").strip())

    failed = [c for c in cases if not c["ok"]]
    for case in cases:
        print(("PASS " if case["ok"] else "FAIL ") + case["name"] + ((" — " + case["detail"]) if case["detail"] and not case["ok"] else ""))
    print("\n%d checks, %d failed" % (len(cases), len(failed)))
    if args.json:
        Path(args.json).write_text(
            json.dumps({"cases": cases, "failed": len(failed), "result": "pass" if not failed else "fail"}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
