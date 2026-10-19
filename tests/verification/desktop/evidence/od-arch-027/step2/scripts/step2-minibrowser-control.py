#!/usr/bin/env python3
"""OD-ARCH-027 · Step 2 受控补充：MiniBrowser 单因素隔离对照（落盘版）。

**受控补充，不替代应用级证据。** 用途：
  1) 校验「空闲 CPU」测量方法在当前环境仍能识别高成本页面（heavy vs simple）；
  2) 把 CSS 改动作为**单因素**隔离出来：同一 DMABUF 取值下，
     `heavy.html`（改前 CSS 复刻）与 `heavy-after.html`（改后 CSS 复刻）对照。

原件 `heavy.html` / `simple.html`（Step 0 归档）**只读**，不改写。

用法：
  python3 step2-minibrowser-control.py --own-xvfb --display :92 \
      --out step2-minibrowser-control-before.json --label step2-minibrowser-control-before \
      --pages simple,heavy --seconds 40
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
STEP2 = HERE.parents[1]
BASE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "baseline-pages"
MB = "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser"
SCRIPT_VERSION = "step2-minibrowser-control.py v1"

PAGES = {
    "simple": BASE / "simple.html",
    "heavy": BASE / "heavy.html",
    "heavy-after": BASE / "heavy-after.html",
}


def proc_ticks(pid: int):
    try:
        parts = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
        return int(parts[11]) + int(parts[12])
    except Exception:
        return None


def procs_by_argv0(kind: str) -> list[int]:
    out = []
    for cand in Path("/proc").iterdir():
        if not cand.name.isdigit():
            continue
        try:
            raw = Path(f"/proc/{cand.name}/cmdline").read_bytes()
        except Exception:
            continue
        if raw.split(b"\0", 1)[0].decode(errors="replace").endswith("/" + kind):
            out.append(int(cand.name))
    return out


def measure(label: str, url: str, display: str, seconds: float, extra: dict) -> dict:
    env = dict(os.environ)
    env["DISPLAY"] = display
    env["GDK_BACKEND"] = "x11"
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("WAYLAND_SOCKET", None)
    env.update(extra)
    res = {"label": label, "url": str(url), "env": extra, "seconds": seconds}
    # 前置：确保没有别的 WebKit 在跑
    stray = procs_by_argv0("WebKitWebProcess")
    if stray:
        res["error"] = f"测量前已存在 WebKitWebProcess：{stray}"
        return res
    with open(os.devnull, "w") as sink:
        proc = subprocess.Popen(
            [MB, str(url)], env=env, stdout=sink, stderr=subprocess.STDOUT, start_new_session=True
        )
    try:
        time.sleep(10)
        web = procs_by_argv0("WebKitWebProcess")
        gpu = procs_by_argv0("WebKitGPUProcess")
        res["pids"] = {"web": web, "gpu": gpu}
        if not web:
            res["error"] = "未找到 WebKitWebProcess"
            return res
        pids = web + gpu
        first = {p: proc_ticks(p) for p in pids}
        t0 = time.monotonic()
        time.sleep(seconds)
        dt = time.monotonic() - t0
        last = {p: proc_ticks(p) for p in pids}
        for name, group in (("webkit_web_process", web), ("webkit_gpu_process", gpu)):
            total = sum(
                last[p] - first[p] for p in group if first[p] is not None and last[p] is not None
            )
            res[name] = {
                "pids": group,
                "avg_percent": round(100.0 * total / 100.0 / dt, 2),
            }
        res["window_seconds"] = round(dt, 2)
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
    return res


class OwnXvfb:
    def __init__(self, display: str, size: str = "1440x1000x24") -> None:
        self.display, self.size = display, size
        self.num = display.lstrip(":")
        self.proc = None

    def __enter__(self):
        if Path(f"/tmp/.X{self.num}-lock").exists():
            raise RuntimeError(f"{self.display} 已被占用")
        log = open("/tmp/opencode/od027-xvfb-minibrowser.log", "a", buffering=1)
        self.proc = subprocess.Popen(
            ["Xvfb", self.display, "-screen", "0", self.size, "-nolisten", "tcp"],
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
        )
        time.sleep(1.5)
        if self.proc.poll() is not None:
            raise RuntimeError("Xvfb 提前退出")
        return self

    def __exit__(self, *exc):
        if self.proc is not None and self.proc.poll() is None:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=10)
            except Exception:
                pass
        return False


def main(argv: list) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--pages", default="simple,heavy")
    ap.add_argument("--display", default=":92")
    ap.add_argument("--seconds", type=float, default=40.0)
    ap.add_argument("--dmabuf", default="1", help="WEBKIT_DISABLE_DMABUF_RENDERER 取值")
    ap.add_argument("--own-xvfb", action="store_true")
    ap.add_argument("--out-dir", default=str(STEP2))
    args = ap.parse_args(argv[1:])

    payload = {
        "label": args.label,
        "script_version": SCRIPT_VERSION,
        "script_path": str(HERE),
        "note": "MiniBrowser 受控补充：只用于单因素隔离对照，不替代应用级窗口证据。",
        "dmabuf_disable_renderer": args.dmabuf,
        "display": f"Xvfb {args.display}",
        "invocation": " ".join(sys.argv),
        "generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
        "generated_at_epoch": time.time(),
        "measurements": [],
    }
    ctx_extra = {"WEBKIT_DISABLE_DMABUF_RENDERER": args.dmabuf}
    names = [n.strip() for n in args.pages.split(",") if n.strip()]
    if args.own_xvfb:
        with OwnXvfb(args.display):
            for n in names:
                if not PAGES[n].is_file():
                    payload["measurements"].append({"label": n, "error": f"页面不存在：{PAGES[n]}"})
                    continue
                payload["measurements"].append(
                    measure(n, PAGES[n], args.display, args.seconds, ctx_extra)
                )
    else:
        for n in names:
            payload["measurements"].append(
                measure(n, PAGES[n], args.display, args.seconds, ctx_extra)
            )
    out = Path(args.out_dir) / f"{args.label}.json"
    out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    for m in payload["measurements"]:
        print(f"  {m['label']}: {m.get('error') or m.get('webkit_web_process')}")
    print(f"[{args.label}] → {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
