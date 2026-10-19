#!/usr/bin/env python3
"""用 MiniBrowser 做对照：简单页 vs 复刻重特效页 的 WebKitWebProcess 空闲 CPU。"""
import os
import signal
import subprocess
import time
from pathlib import Path

MB = "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser"
BASE = Path("/tmp/od025-evidence/baseline")


def ticks(pid: int):
    try:
        p = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
        return int(p[11]) + int(p[12])
    except Exception:
        return None


def web_processes(root_pid: int) -> list[int]:
    """WebKit 子进程被 bubblewrap 沙箱重挂载父进程，故按命令行全局匹配；
    测量时确保系统上只有本次这一个 WebKit 应用在跑。"""
    out = []
    for cand in Path("/proc").iterdir():
        if not cand.name.isdigit():
            continue
        try:
            cmdline = (cand / "cmdline").read_bytes().replace(b"\0", b" ").decode()
        except Exception:
            continue
        if "webkit2gtk-4.1/WebKitWebProcess" in cmdline or "webkit2gtk-4.1/WebKitGPUProcess" in cmdline:
            out.append(int(cand.name))
    return out


def _is_descendant(pid: int, root: int, depth: int = 4) -> bool:
    for _ in range(depth):
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
            ppid = int(stat.split(") ", 1)[1].split()[1])
        except Exception:
            return False
        if ppid == root:
            return True
        if ppid <= 1:
            return False
        pid = ppid
    return False


def measure(label: str, url: str, env_extra: dict) -> None:
    env = dict(os.environ)
    env["DISPLAY"] = ":95"
    env.update(env_extra)
    proc = subprocess.Popen(
        [MB, url], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True
    )
    try:
        time.sleep(8)
        wps = web_processes(proc.pid)
        if not wps:
            print(f"{label}: 未找到 WebKitWebProcess")
            return
        a = {p: ticks(p) for p in wps}
        t0 = time.monotonic()
        time.sleep(6)
        dt = time.monotonic() - t0
        total = 0.0
        for p in wps:
            b = ticks(p)
            if a[p] is None or b is None:
                continue
            total += 100.0 * (b - a[p]) / 100.0 / dt
        print(f"{label}: WebKitWebProcess 空闲 CPU = {total:.1f}%  (进程数 {len(wps)})")
    finally:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
            proc.wait(timeout=10)
        except Exception:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except Exception:
                pass


if __name__ == "__main__":
    common = {"WEBKIT_DISABLE_DMABUF_RENDERER": "1"}  # 与 dev-ui.sh 默认一致
    measure("简单页 + DMABUF 关（dev-ui.sh 默认）", f"file://{BASE}/simple.html", common)
    measure("重特效页 + DMABUF 关（dev-ui.sh 默认）", f"file://{BASE}/heavy.html", common)
    measure("重特效页 + DMABUF 开（覆盖默认）", f"file://{BASE}/heavy.html", {"WEBKIT_DISABLE_DMABUF_RENDERER": "0"})
