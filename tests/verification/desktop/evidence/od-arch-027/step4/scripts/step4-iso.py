#!/usr/bin/env python3
"""OD-ARCH-027 Step 4 · Phase A：无限动画 × 大面积模糊「单因素对照」（MiniBrowser）。

对 `baseline-pages/` 下 5 个同构对照页各跑 ≥3 次、每次 ≥30 s，记录
`WebKitWebProcess` / `WebKitGPUProcess` CPU、进程集合、截图判据，以及「绘制推进」近似指标
（用于区分「CPU 花在持续重绘」与「只是静态合成」）。

复用 Step 3 的 `_step3lib`（只读导入，不修改 Step 3 证据）。`--dmabuf 1`（与 dev-ui.sh 现状默认一致）。

用法：
  python3 step4-iso.py --pages iso-blur-only,iso-anim-only,iso-anim-blur,iso-anim-pulse-only,iso-anim-float-only \
      --runs 3 --seconds 30 --own-xvfb --display :95
"""

from __future__ import annotations

import argparse
import hashlib
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
OD027 = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027"
STEP4 = OD027 / "step4"
BASE_PAGES = OD027 / "baseline-pages"
sys.path.insert(0, str(OD027 / "step3" / "scripts"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

from _step3lib import (  # noqa: E402
    assert_no_webkit,
    base_env_snapshot,
    frame_progress,
    frame_stats,
    proc_env,
    sample_cpu,
    webkit_process_groups,
    write_json,
)

MB = "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser"


def run_one(page: str, run_idx: int, args) -> dict:
    url = BASE_PAGES / f"{page}.html"
    tag = f"step4-A-iso-{page}-run{run_idx}"
    png = STEP4 / f"{tag}.png"
    res: dict = {
        "step": "step4", "group": "A-iso", "subject": "MiniBrowser 单因素对照（受控页面证据）",
        "page": page, "page_file": str(url),
        "page_sha256": hashlib.sha256(url.read_bytes()).hexdigest() if url.is_file() else None,
        "dmabuf": args.dmabuf, "run": run_idx, "seconds": args.seconds, "screenshot": str(png),
        "note": "受控页面证据，不直接等于真实应用；布局/尺寸/颜色在 5 个页面间完全一致",
    }
    if not url.is_file():
        res["error"] = f"页面不存在：{url}"
        return res
    assert_no_webkit(f"{tag} 开始前")
    env = dict(os.environ)
    env["DISPLAY"] = args.display
    env["GDK_BACKEND"] = "x11"
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    env["WEBKIT_DISABLE_DMABUF_RENDERER"] = args.dmabuf
    with open(os.devnull, "w") as sink:
        proc = subprocess.Popen([MB, str(url)], env=env, stdout=sink, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        time.sleep(12)
        groups = webkit_process_groups()
        res["process_set"] = {k: list(v) for k, v in groups.items()}
        if not groups["webkit_web_process"]:
            res["error"] = "未找到 WebKitWebProcess"
            return res
        res["cpu"] = sample_cpu(groups, args.seconds, curve_every=5.0)
        res["pid_env_check"] = {
            str(p): proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER")
            for p in groups["webkit_web_process"] + groups["webkit_gpu_process"]
        }
        from window_harness import XSession

        XSession(args.display).shot(png)
        res["frame"] = frame_stats(png, crop=(0, 0, 1024, 768))
        res["paint_progress"] = frame_progress((0, 0, 1030, 770), samples=30, interval=0.03)
    except Exception as exc:
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
    res["env"] = base_env_snapshot(args.display, args.dmabuf, {
        "seconds": args.seconds, "run": run_idx, "page": page,
        "harness": "MiniBrowser；复用 step3/_step3lib（只读导入）",
    })
    write_json(STEP4 / f"{tag}.json", res)
    return res


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description="Step 4 Phase A：单因素对照")
    ap.add_argument("--pages", default="iso-blur-only,iso-anim-only,iso-anim-blur,iso-anim-pulse-only,iso-anim-float-only")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--seconds", type=float, default=30.0)
    ap.add_argument("--dmabuf", default="1", choices=["0", "1"])
    ap.add_argument("--display", default=":95")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])

    from _step3lib import OwnXvfb

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    summary = []
    pages = [p.strip() for p in args.pages.split(",") if p.strip()]
    with ctx:
        for page in pages:
            for i in range(1, args.runs + 1):
                r = run_one(page, i, args)
                cpu = r.get("cpu", {})
                web = cpu.get("webkit_web_process", {})
                gpu = cpu.get("webkit_gpu_process", {})
                pp = r.get("paint_progress", {})
                print(
                    f"  [A {page:22s} run{i}] web={web.get('avg_percent')}% (n={web.get('process_count')}) "
                    f"gpu_n={gpu.get('process_count')} luma={r.get('frame', {}).get('mean_luma')} "
                    f"paint={pp.get('changed_pairs')}/{pp.get('pairs')} "
                    f"{('ERR:' + r['error']) if r.get('error') else ''}"
                )
                summary.append({
                    "page": page, "run": i,
                    "web_avg_percent": web.get("avg_percent"),
                    "web_count": web.get("process_count"),
                    "gpu_avg_percent": gpu.get("avg_percent"),
                    "gpu_count": gpu.get("process_count"),
                    "network_count": cpu.get("webkit_network_process", {}).get("process_count"),
                    "mean_luma": r.get("frame", {}).get("mean_luma"),
                    "paint_changed_pairs": pp.get("changed_pairs"),
                    "paint_pairs": pp.get("pairs"),
                    "paint_mean_delta": pp.get("mean_delta_fraction"),
                    "error": r.get("error"),
                    "json": str(STEP4 / f"step4-A-iso-{page}-run{i}.json"),
                })
    agg = {}
    for s in summary:
        agg.setdefault(s["page"], []).append(s["web_avg_percent"])
    agg_mean = {
        k: round(sum(v) / len(v), 2)
        for k, v in agg.items()
        if all(x is not None for x in v)
    }
    write_json(STEP4 / "step4-A-iso-summary.json", base_env_snapshot(args.display, args.dmabuf, {
        "runs": summary,
        "aggregate_web_percent": {
            k: {"n": len(v), "min": min(v), "max": max(v), "mean": round(sum(v) / len(v), 2)}
            for k, v in agg.items() if all(x is not None for x in v)
        },
        "note": "受控页面证据；不直接等于真实应用",
    }))
    print("[A] 各页 WebKitWebProcess 均值：", agg_mean)
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
