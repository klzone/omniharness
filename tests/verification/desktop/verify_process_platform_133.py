#!/usr/bin/env python3
"""OD-ARCH-133 —— macOS 上的进程核验与回收（静态口径 + 真实进程证据）。

只读仓库源码、只在临时目录里起一个可丢弃的假引擎进程；不启动 DSH、
不接触 ~/.dsh、~/.agents、~/OmniDesk。

证据等级：static + 本机假进程（macOS 走 ps / sysctl）。
运行：python3 tests/verification/desktop/verify_process_platform_133.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
RUST_SRC = REPO / "desktop" / "src-tauri" / "src"
PLATFORM_RS = RUST_SRC / "platform.rs"
OWNER_RS = RUST_SRC / "process_owner.rs"
TASK_SHELL_RS = RUST_SRC / "task_shell.rs"
OWNER_PY = REPO / "engine" / "omnidesk_engine" / "process_owner.py"
LIFECYCLE_PY = REPO / "tests" / "verification" / "desktop" / "verify_process_lifecycle_074.py"
UNIT_PY = (
    REPO
    / "tests"
    / "verification"
    / "engine-interface"
    / "adapter"
    / "test_process_owner_unit.py"
)
EVIDENCE = (
    REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-133" / "facts.json"
)

CASES: list[dict] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    platform_rs = read(PLATFORM_RS)
    owner_rs = read(OWNER_RS)
    task_shell_rs = read(TASK_SHELL_RS)
    owner_py = read(OWNER_PY)

    # --- A. 后端集中在 platform.rs ---------------------------------------
    for fn in ("boot_id", "process_stat", "process_cmd", "process_table"):
        record(
            "platform_macos_%s" % fn,
            'cfg(target_os = "macos")' in platform_rs and ("pub fn %s(" % fn) in platform_rs,
        )
    record(
        "platform_windows_returns_empty",
        platform_rs.count('cfg(not(any(target_os = "linux", target_os = "macos")))') >= 4,
        "Windows 不参与回收，查不到即不动",
    )
    record("platform_proc_row", "pub struct ProcRow" in platform_rs)
    record(
        "platform_boottime_parser",
        "parse_boottime_sec" in platform_rs and 'format!("darwin:{}", sec)' in platform_rs,
    )

    # --- B. 判定层不再直接读 procfs，杀进程参数不变 -----------------------
    record("owner_no_procfs", "/proc/sys/kernel/random/boot_id" not in owner_rs and '"/proc"' not in owner_rs)
    record(
        "owner_delegates_platform",
        all(
            marker in owner_rs
            for marker in (
                "crate::platform::boot_id()",
                "crate::platform::process_stat(pid)",
                "crate::platform::process_cmd(pid)",
                "crate::platform::process_table()",
            )
        ),
    )
    record(
        "owner_kill_group_unchanged",
        'Command::new("/bin/kill")' in owner_rs and 'format!("-{}", pgid)' in owner_rs,
    )
    record(
        "owner_refuses_unverifiable",
        "拒绝启动，不杀进程" in owner_rs and "另一个 OmniDesk 实例正在使用同一数据根" in owner_rs,
    )

    # --- C. 引擎侧同步 ---------------------------------------------------
    record("py_darwin_flag", '_DARWIN = sys.platform == "darwin"' in owner_py)
    for fn in ("_darwin_boot_id", "_darwin_stat", "_darwin_cmdline", "_darwin_group_members"):
        record("py_%s" % fn, ("def %s(" % fn) in owner_py)
    record(
        "py_dispatch",
        owner_py.count("if _DARWIN:") >= 4,
        "boot_id / _proc_stat / _proc_cmdline / group_members 四处分流",
    )
    record(
        "py_linux_path_kept",
        '_BOOT_ID_PATH = "/proc/sys/kernel/random/boot_id"' in owner_py
        and 'Path("/proc/%d/stat" % pid)' in owner_py,
    )

    # --- D. 只改测试口径的两处 -------------------------------------------
    record(
        "task_shell_assertion_canonical",
        "let root_real = dir.canonicalize()" in task_shell_rs,
    )
    lifecycle = read(LIFECYCLE_PY)
    record(
        "lifecycle_uses_platform_tempdir",
        "tempfile.gettempdir()" in lifecycle and '"/tmp/"' not in lifecycle,
    )
    unit = read(UNIT_PY)
    record(
        "unit_waits_for_exec",
        "omnidesk_engine" in unit and "deadline = time.monotonic() + 5.0" in unit,
    )

    # --- E. 真实进程：引擎侧在 macOS 上能核验自己 ------------------------
    sys.path.insert(0, str(REPO / "engine"))
    from omnidesk_engine import process_owner as po  # noqa: E402

    boot = po.boot_id()
    record("py_boot_id_darwin", boot.startswith("darwin:") and boot != "", boot)

    pid = os.getpid()
    stat = po._proc_stat(pid)
    record("py_stat_self", stat is not None and stat[0] != "Z" and stat[1] == os.getpgrp(), repr(stat))
    cmd = po._proc_cmdline(pid)
    record("py_cmdline_self", bool(cmd), " ".join(cmd)[:40])

    members = po.group_members(os.getpgrp())
    record("py_group_members_contains_self", any(m[0] == pid for m in members), "members=%d" % len(members))
    record("py_group_members_rejects_illegal", po.group_members(1) == [] and po.group_members(0) == [])

    # --- F. 真实进程：登记 → 核验为 active_engine → 退出后判 pid_reused ----
    root = Path(tempfile.mkdtemp(prefix="od133-"))
    fake = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(60)", "omnidesk_engine-od133"],
        start_new_session=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if any("omnidesk_engine" in a for a in po._proc_cmdline(fake.pid)):
                break
            time.sleep(0.02)
        fake_cmd = po._proc_cmdline(fake.pid)
        (root / po.OWNER_FILENAME).write_text(
            json.dumps(
                {
                    "pid": fake.pid,
                    "pgid": fake.pid,
                    "host_pid": 0,
                    "boot_id": boot,
                    "channel": "web-api",
                    "started_at": time.time(),
                    "cmdline": fake_cmd,
                    "state_dir": str(root),
                    "child_pid": 0,
                    "child_pgid": 0,
                }
            ),
            encoding="utf-8",
        )
        inspected = po.inspect_owner(root)
        record("py_inspect_active_engine", inspected.status == "active_engine", inspected.status)

        os.kill(fake.pid, 9)
        fake.wait(timeout=10)
        record("py_inspect_after_exit", po.inspect_owner(root).status == "pid_reused")
    finally:
        if fake.poll() is None:
            fake.kill()
            fake.wait(timeout=5)
        for item in root.glob("*"):
            try:
                item.unlink()
            except OSError:
                pass
        try:
            root.rmdir()
        except OSError:
            pass

    failed = [c for c in CASES if not c["ok"]]
    report = {
        "plan": "OD-ARCH-133",
        "platform": sys.platform,
        "cases": CASES,
        "summary": {"total": len(CASES), "failed": len(failed)},
        "passed": not failed,
    }
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for case in CASES:
        mark = "pass" if case["ok"] else "fail"
        suffix = (" " + case["detail"]) if case["detail"] else ""
        print("[%s] %s%s" % (mark, case["name"], suffix))
    print("\n证据：%s" % EVIDENCE)
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
