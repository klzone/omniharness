#!/usr/bin/env python3
"""确认空闲时到底是哪个进程在烧 CPU（App 主进程 / WebKit 渲染子进程）。"""
import sys
import time
from pathlib import Path

sys.path.insert(0, "/mnt/data/Code/MyWorkStation/tests/verification/desktop")
from window_harness import AppSession, XSession

ROOT = Path("/tmp/od025-evidence/perf2")
app = AppSession(
    root=ROOT,
    display=":95",
    binary=Path("/mnt/data/Code/MyWorkStation/desktop/src-tauri/target/release/omnidesk-desktop"),
    use_dev_server=False,
)
app.prepare()
xs = XSession(":95")


def ticks(pid):
    try:
        p = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
        return int(p[11]) + int(p[12])
    except Exception:
        return None


def cmd(pid):
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode()[:70]
    except Exception:
        return "?"


def descendants(pid):
    out = [pid]
    for cand in Path("/proc").iterdir():
        if not cand.name.isdigit():
            continue
        try:
            stat = (cand / "stat").read_text()
            ppid = int(stat.split(") ", 1)[1].split()[1])
            if ppid == pid:
                out.append(int(cand.name))
        except Exception:
            pass
    return out

try:
    app.start()
    win = xs.find_window("OmniDesk", timeout=180)
    print("window:", win is not None)
    time.sleep(10)
    pids = descendants(app.proc.pid)
    print("进程树:", [(p, cmd(p)) for p in pids])
    a = {p: ticks(p) for p in pids}
    t0 = time.monotonic()
    time.sleep(6)
    dt = time.monotonic() - t0
    print(f"--- 空闲 {dt:.1f}s 内各进程 CPU 占用 ---")
    for p in pids:
        b = ticks(p)
        if a[p] is None or b is None:
            continue
        print(f"  pid={p}  cpu={100.0 * (b - a[p]) / 100.0 / dt:6.1f}%   {cmd(p)}")
finally:
    app.stop_all()
