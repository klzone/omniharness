#!/usr/bin/env python3
"""OD-ARCH-025 附加测量：debug+vite dev 与 release+内嵌产物 的界面响应对比（调试/取证用）。

测量口径：
  * 空闲 CPU：窗口停在首页 5 秒，读 /proc/<app pid>/stat 的 utime+stime 增量（含 WebKit 子进程）。
  * 导航延迟：点 Dock 切场景后，每 150 ms 抓一帧内容区，检测到画面变化为止，取中位数。
"""
import hashlib
import sys
import time
from pathlib import Path

sys.path.insert(0, "/mnt/data/Code/MyWorkStation/tests/verification/desktop")
from window_harness import APP_BIN, AppSession, XSession
from run_window_acceptance import Driver, NAV

ROOT = Path("/tmp/od025-evidence/perf")
DOCK_ORDER = ["workbench", "offline", "settings", "sessions", "home"]


def cpu_seconds(pid: int) -> float:
    try:
        parts = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
        return (int(parts[11]) + int(parts[12])) / 100.0  # utime + stime（ticks→秒）
    except Exception:
        return -1.0


def webkit_children(pid: int) -> list[int]:
    out = []
    try:
        for line in Path("/proc").iterdir():
            if not line.name.isdigit():
                continue
            try:
                stat = (line / "stat").read_text()
                ppid = int(stat.split(") ", 1)[1].split()[1])
                if ppid == pid:
                    out.append(int(line.name))
            except Exception:
                pass
    except Exception:
        pass
    return out


def run(label: str, binary, use_dev_server: bool) -> dict:
    app = AppSession(root=ROOT / label, display=":95", binary=binary, use_dev_server=use_dev_server)
    app.prepare()
    xs = XSession(":95")
    res = {"label": label, "binary": str(binary), "use_dev_server": use_dev_server}
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            res["error"] = "未找到窗口"
            return res
        geo = xs.geometry(win)
        xs.focus(win)
        d = Driver(xs, win, geo, ROOT / label / "_sig.png")
        time.sleep(8)

        pid = app.proc.pid
        kids = webkit_children(pid)
        res["app_pid"] = pid
        res["webkit_children"] = len(kids)
        c0 = cpu_seconds(pid) + sum(cpu_seconds(k) for k in kids)
        t0 = time.monotonic()
        time.sleep(5)
        c1 = cpu_seconds(pid) + sum(cpu_seconds(k) for k in kids)
        dt = time.monotonic() - t0
        res["idle_cpu_percent"] = round(100.0 * (c1 - c0) / dt, 1)

        lat = []
        for name in DOCK_ORDER:
            before = d._content_sig()
            t = time.monotonic()
            xs.dismiss_overlays()
            xs.focus(win)
            xs.click(geo.x + 1240, geo.y + 405, settle=0.05)
            xs.click(geo.x + 48, geo.y + NAV[name], settle=0.05)
            changed = None
            for _ in range(60):  # 最多 9 秒
                time.sleep(0.15)
                if d._content_sig() != before:
                    changed = time.monotonic() - t
                    break
            lat.append((name, round(changed, 2) if changed else None))
        res["nav_latency_s"] = lat
        vals = [v for _, v in lat if v]
        res["nav_latency_median_s"] = round(sorted(vals)[len(vals) // 2], 2) if vals else None
    finally:
        app.stop_all()
    return res


if __name__ == "__main__":
    import json

    out = {}
    out["debug+vite_dev"] = run("debug", APP_BIN, True)
    out["release+embedded"] = run(
        "release",
        Path("/mnt/data/Code/MyWorkStation/desktop/src-tauri/target/release/omnidesk-desktop"),
        False,
    )
    print(json.dumps(out, ensure_ascii=False, indent=2))
