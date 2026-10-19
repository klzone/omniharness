#!/usr/bin/env python3
"""OD-ARCH-027 Step 3 · A 组：页面级 DMABUF 独立对照（MiniBrowser，决定性、可复现）。

在同一一次性 Xvfb（无 GPU）上，用 Step 0 归档的 `baseline-pages/{simple,heavy}.html`
（原件只读）分别测 `WEBKIT_DISABLE_DMABUF_RENDERER=1`（现状默认）与 `=0`（启用），
每档 ≥3 次独立运行。记录 `WebKitWebProcess` / `WebKitGPUProcess` / `WebKitNetworkProcess`
的 CPU 均值、进程数与**进程集合差异**，并对每次运行抓整屏截图做黑屏/空白判据。

注意：`simple.html` / `heavy.html` 原件不改写。

用法：
  python3 step3-pages.py --dmabuf 1 --runs 3 --seconds 30 --display :93
  python3 step3-pages.py --dmabuf 0 --runs 3 --seconds 30 --display :93 --own-xvfb
"""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _step3lib import (  # noqa: E402
    BASE_PAGES,
    SCRIPT_VERSION,
    STEP3,
    assert_no_webkit,
    base_env_snapshot,
    frame_stats,
    proc_env,
    sample_cpu,
    webkit_process_groups,
    write_json,
)

MB = "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser"
PAGES = {"simple": "simple.html", "heavy": "heavy.html"}


def run_one(page: str, dmabuf: str, run_idx: int, display: str, seconds: float) -> dict:
    out = STEP3 / f"step3-A-{page}-dmabuf{dmabuf}-run{run_idx}.json"
    png = STEP3 / f"step3-A-{page}-dmabuf{dmabuf}-run{run_idx}.png"
    url = BASE_PAGES / PAGES[page]
    res: dict = {
        "step": "step3",
        "group": "A-pages",
        "subject": "MiniBrowser（页面级，决定性对照）",
        "page": page,
        "page_file": str(url),
        "page_sha256": __import__("hashlib").sha256(url.read_bytes()).hexdigest(),
        "dmabuf": dmabuf,
        "run": run_idx,
        "seconds": seconds,
        "screenshot": str(png),
    }
    assert_no_webkit(f"run {run_idx} 开始前")
    env = dict(os.environ)
    env["DISPLAY"] = display
    env["GDK_BACKEND"] = "x11"
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    env["WEBKIT_DISABLE_DMABUF_RENDERER"] = dmabuf
    with open(os.devnull, "w") as sink:
        proc = subprocess.Popen(
            [MB, str(url)], env=env, stdout=sink, stderr=subprocess.STDOUT, start_new_session=True
        )
    try:
        time.sleep(12)
        groups = webkit_process_groups()
        res["process_set"] = {k: v for k, v in groups.items()}
        if not groups["webkit_web_process"]:
            res["error"] = "未找到 WebKitWebProcess"
            return res
        res["cpu"] = sample_cpu(groups, seconds, curve_every=5.0)
        res["cpu"]["app_main_note"] = "页面级对照：MiniBrowser 无 OmniDesk 主进程，不统计"
        # 抓图（整屏；MiniBrowser 是显示上唯一应用）
        from window_harness import XSession

        xs = XSession(display)
        xs.shot(png)
        res["frame"] = frame_stats(png)
        res["pid_env_check"] = {
            str(p): proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER")
            for p in groups["webkit_web_process"] + groups["webkit_gpu_process"]
        }
    except Exception as exc:  # 不掩盖失败
        res["error"] = f"{type(exc).__name__}: {exc}"
    finally:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
            proc.wait(timeout=10)
        except Exception:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except Exception:
                pass
        time.sleep(2)
    res["env"] = base_env_snapshot(display, dmabuf, {"seconds": seconds, "run": run_idx, "page": page})
    write_json(out, res)
    return res


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description="Step 3 A 组：页面级 DMABUF 对照")
    ap.add_argument("--pages", default="simple,heavy")
    ap.add_argument("--dmabuf", default="1", choices=["0", "1"])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--seconds", type=float, default=30.0)
    ap.add_argument("--display", default=":93")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])

    from _step3lib import OwnXvfb

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    names = [n.strip() for n in args.pages.split(",") if n.strip()]
    summary = []
    with ctx:
        for page in names:
            for i in range(1, args.runs + 1):
                r = run_one(page, args.dmabuf, i, args.display, args.seconds)
                cpu = r.get("cpu", {})
                web = cpu.get("webkit_web_process", {})
                gpu = cpu.get("webkit_gpu_process", {})
                print(
                    f"  [A dmabuf={args.dmabuf} {page} run{i}] web={web.get('avg_percent')}% "
                    f"(n={web.get('process_count')}) gpu={gpu.get('avg_percent')}% (n={gpu.get('process_count')}) "
                    f"frame={r.get('frame', {}).get('mean_luma')} black={r.get('frame', {}).get('black_screen_suspect')} "
                    f"{('ERROR: ' + r['error']) if r.get('error') else ''}"
                )
                summary.append(
                    {
                        "page": page,
                        "dmabuf": args.dmabuf,
                        "run": i,
                        "web_avg_percent": web.get("avg_percent"),
                        "web_count": web.get("process_count"),
                        "gpu_avg_percent": gpu.get("avg_percent"),
                        "gpu_count": gpu.get("process_count"),
                        "network_count": cpu.get("webkit_network_process", {}).get("process_count"),
                        "mean_luma": r.get("frame", {}).get("mean_luma"),
                        "black_screen_suspect": r.get("frame", {}).get("black_screen_suspect"),
                        "error": r.get("error"),
                        "json": str(STEP3 / f"step3-A-{page}-dmabuf{args.dmabuf}-run{i}.json"),
                    }
                )
    write_json(STEP3 / f"step3-A-summary-dmabuf{args.dmabuf}.json",
               base_env_snapshot(args.display, args.dmabuf, {"runs": summary, "script_version": SCRIPT_VERSION}))
    print(f"[A dmabuf={args.dmabuf}] {len(summary)} run(s) 完成")
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
