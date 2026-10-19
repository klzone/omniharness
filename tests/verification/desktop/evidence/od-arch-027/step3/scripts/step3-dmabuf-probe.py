#!/usr/bin/env python3
"""OD-ARCH-027 Step 3 · 环境探针：`WEBKIT_DISABLE_DMABUF_RENDERER` 在本沙箱是否真的可控。

背景：Step 3 的 A 组（MiniBrowser，heavy.html）在 `=1` 与 `=0` 下结果**完全相同**
（约 100%，且两种情况下都**没有** WebKitGPUProcess）。需要判断这是
  (b) 因为 Xvfb 无 DRI3/GPU，DMABUF 渲染路径根本无法启用，还是
  (a) 因为当前 WebKit 把该变量当作「存在即禁用」，`=0` 其实没有启用。

做法：同一页（heavy.html）、同一显示，四种处理各 1 次，并**保留 MiniBrowser 的标准错误**
（`G_MESSAGES_DEBUG=all`）以便查找 DMABuf / EGL / GBM 相关消息：
  * `unset`  完全不设该变量（DMABUF 渲染的默认状态）
  * `0`      设 `=0`（计划里「启用」的写法）
  * `1`      设 `=1`（dev-ui.sh 现状默认）
  * `force`  设 `WEBKIT_FORCE_DMABUF_RENDERER=1`（强制启用旋钮）

这是**受控补充/环境探针**：只说明本沙箱里变量能否造成差异，不替代应用级证据。
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
TOKENS = ("dmabuf", "DMABuf", "EGL", "egl", "GBM", "gbm", "shm", "SHM", "Accelerated", "accelerat", "gl ", "GL ")


def run_treatment(name: str, display: str, seconds: float, page: str) -> dict:
    url = BASE_PAGES / page
    log = STEP3 / f"step3-probe-{name}.log"
    png = STEP3 / f"step3-probe-{name}.png"
    res = {"treatment": name, "page": page, "page_file": str(url), "stderr_log": str(log), "screenshot": str(png)}
    assert_no_webkit(f"probe {name} 开始前")
    env = dict(os.environ)
    env["DISPLAY"] = display
    env["GDK_BACKEND"] = "x11"
    env["G_MESSAGES_DEBUG"] = "all"
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    env.pop("WEBKIT_DISABLE_DMABUF_RENDERER", None)
    env.pop("WEBKIT_FORCE_DMABUF_RENDERER", None)
    if name == "0":
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "0"
    elif name == "1":
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
    elif name == "force":
        env["WEBKIT_FORCE_DMABUF_RENDERER"] = "1"
    res["env_set"] = {
        k: env.get(k) for k in ("WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_FORCE_DMABUF_RENDERER")
    }
    try:
        with open(log, "w") as fh:
            proc = subprocess.Popen([MB, str(url)], env=env, stdout=fh, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            time.sleep(12)
            groups = webkit_process_groups()
            res["process_set"] = groups
            if not groups["webkit_web_process"]:
                res["error"] = "未找到 WebKitWebProcess"
                return res
            res["cpu"] = sample_cpu(groups, seconds, curve_every=None)
            res["pid_env_check"] = {
                str(p): {
                    "WEBKIT_DISABLE_DMABUF_RENDERER": proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER"),
                    "WEBKIT_FORCE_DMABUF_RENDERER": proc_env(p).get("WEBKIT_FORCE_DMABUF_RENDERER"),
                }
                for p in groups["webkit_web_process"] + groups["webkit_gpu_process"]
            }
            from window_harness import XSession

            XSession(display).shot(png)
            res["frame"] = frame_stats(png)
            # 绘制推进近似指标（无窗口管理器时 MiniBrowser 窗口固定在 0,0，约 1030x770）
            from _step3lib import frame_progress

            res["paint_progress"] = frame_progress((0, 0, 1030, 770), samples=30, interval=0.03)
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
        # 从标准错误里找渲染路径相关消息
        text = log.read_text(errors="replace")
        hits = [ln.strip()[:200] for ln in text.splitlines() if any(t in ln for t in TOKENS)]
        res["stderr_matching_lines"] = hits[:40]
        res["stderr_line_count"] = len(text.splitlines())
    except Exception as exc:
        res["error"] = f"{type(exc).__name__}: {exc}"
    res["env"] = base_env_snapshot(display, res["env_set"].get("WEBKIT_DISABLE_DMABUF_RENDERER") or "unset",
                                   {"treatment": name, "seconds": seconds})
    return res


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description="Step 3 环境探针：DMABUF 变量可控性")
    ap.add_argument("--treatments", default="unset,0,1,force")
    ap.add_argument("--page", default="heavy.html")
    ap.add_argument("--seconds", type=float, default=30.0)
    ap.add_argument("--display", default=":93")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])

    from _step3lib import OwnXvfb

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    out = []
    with ctx:
        for name in [t.strip() for t in args.treatments.split(",") if t.strip()]:
            r = run_treatment(name, args.display, args.seconds, args.page)
            cpu = r.get("cpu", {})
            pp = r.get("paint_progress", {})
            print(
                f"  [probe {name:6s}] env={r.get('env_set')} web={cpu.get('webkit_web_process', {}).get('avg_percent')}% "
                f"gpu_n={cpu.get('webkit_gpu_process', {}).get('process_count')} "
                f"luma={r.get('frame', {}).get('mean_luma')} "
                f"paint: changed_pairs={pp.get('changed_pairs')}/{pp.get('pairs')} mean_delta={pp.get('mean_delta_fraction')} "
                f"{('ERR:' + r['error']) if r.get('error') else ''}"
            )
            write_json(STEP3 / f"step3-probe-{name}.json", r)
            out.append({k: r.get(k) for k in ("treatment", "env_set", "error", "stderr_line_count")} | {
                "web_avg_percent": cpu.get("webkit_web_process", {}).get("avg_percent"),
                "gpu_count": cpu.get("webkit_gpu_process", {}).get("process_count"),
                "paint_progress": r.get("paint_progress"),
                "stderr_matching_lines": r.get("stderr_matching_lines", [])[:10],
            })
    write_json(STEP3 / "step3-probe-summary.json",
               base_env_snapshot(args.display, "probe", {"treatments": out}))
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
