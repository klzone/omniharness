#!/usr/bin/env python3
"""OD-ARCH-027 Step 3 共用工具（进程 CPU、一次性 Xvfb、画面异常判据）。

只被 step3-pages.py / step3-app.py 导入；不改动任何共享测试装置与产品代码。
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
OD027 = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027"
STEP3 = OD027 / "step3"
BASE_PAGES = OD027 / "baseline-pages"
DEV_PORT = 5273
SCRIPT_VERSION = "step3 v1"

sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

TICKS_PER_SEC = 100.0  # 本机 USER_HZ=100（与 Step 0/2 口径一致）


# ---------------------------------------------------------------------------
# /proc
# ---------------------------------------------------------------------------


def proc_ticks(pid: int):
    """(utime+stime) ticks；失败返回 None。"""
    try:
        parts = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
        return int(parts[11]) + int(parts[12])
    except Exception:
        return None


def proc_argv0(pid: int) -> str:
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0", 1)[0].decode(errors="replace")
    except Exception:
        return ""


def proc_cmdline(pid: int) -> str:
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace").strip()
    except Exception:
        return ""


def proc_env(pid: int) -> dict:
    """读进程环境变量（同用户可读）。用于核对 DMABUF 取值确实传给了该进程。"""
    out = {}
    try:
        raw = Path(f"/proc/{pid}/environ").read_bytes()
    except Exception:
        return out
    for item in raw.split(b"\0"):
        if b"=" not in item:
            continue
        k, _, v = item.partition(b"=")
        out[k.decode(errors="replace")] = v.decode(errors="replace")
    return out


def procs_by_argv0(kind: str) -> list[int]:
    """按 argv[0] 全局匹配（bubblewrap 会重挂 WebKit 子进程的父进程，不能只按 ppid 找）。"""
    out = []
    for cand in Path("/proc").iterdir():
        if not cand.name.isdigit():
            continue
        if proc_argv0(int(cand.name)).endswith("/" + kind):
            out.append(int(cand.name))
    return out


def webkit_process_groups() -> dict[str, list[int]]:
    return {
        "webkit_web_process": procs_by_argv0("WebKitWebProcess"),
        "webkit_network_process": procs_by_argv0("WebKitNetworkProcess"),
        "webkit_gpu_process": procs_by_argv0("WebKitGPUProcess"),
    }


def assert_no_webkit(where: str) -> None:
    web = procs_by_argv0("WebKitWebProcess")
    if web:
        raise RuntimeError(f"{where}：已存在 WebKitWebProcess {[(p, proc_cmdline(p)[:80]) for p in web]}")


def port_busy(port: int = DEV_PORT) -> bool:
    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


# ---------------------------------------------------------------------------
# CPU 采样
# ---------------------------------------------------------------------------


def sample_cpu(groups: dict[str, list[int]], seconds: float, curve_every: float = 5.0) -> dict:
    """对给定进程组做一次连续采样，返回每组的 60 s 均值（以及可选曲线）。"""
    pids = sorted({p for v in groups.values() for p in v})
    first = {p: proc_ticks(p) for p in pids}
    t0 = time.monotonic()
    curve = []
    nxt = curve_every
    while time.monotonic() - t0 < seconds:
        time.sleep(0.25)
        el = time.monotonic() - t0
        if curve_every and el >= nxt:
            now = {p: proc_ticks(p) for p in pids}
            curve.append(
                {
                    "t": round(el, 1),
                    **{
                        name: round(100.0 * sum((now[p] or first[p]) - first[p] for p in grp) / TICKS_PER_SEC / el, 2)
                        for name, grp in groups.items()
                    },
                }
            )
            nxt += curve_every
    dt = time.monotonic() - t0
    last = {p: proc_ticks(p) for p in pids}
    res = {"window_seconds": round(dt, 2)}
    if curve:
        res["curve"] = curve
        res["curve_every_s"] = curve_every
    for name, grp in groups.items():
        missing = [p for p in grp if first[p] is None or last[p] is None]
        total = sum(last[p] - first[p] for p in grp if first[p] is not None and last[p] is not None)
        res[name] = {
            "pids": grp,
            "process_count": len(grp),
            "cmdlines": {str(p): proc_cmdline(p)[:140] for p in grp},
            "dmabuf_env": {str(p): proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER") for p in grp},
            "avg_percent": round(100.0 * total / TICKS_PER_SEC / dt, 2),
            "missing_pids": missing,
        }
    return res


# ---------------------------------------------------------------------------
# 画面异常判据（黑屏/空白/闪烁）
# ---------------------------------------------------------------------------


def frame_stats(path: Path, crop: tuple[int, int, int, int] | None = None) -> dict:
    """简单的像素判据：平均亮度、暗像素占比、亮度标准差、抽样色彩数。

    判据只用于**标记疑似**异常帧，不替代人工看图；异常帧一律保留截图。
    """
    import numpy as np
    from PIL import Image

    im = np.asarray(Image.open(path).convert("RGB"))
    if crop:
        x0, y0, x1, y1 = crop
        im = im[max(0, y0) : y1, max(0, x0) : x1]
    luma = im.astype(np.float64) @ np.array([0.2126, 0.7152, 0.0722])
    packed = (
        (im[..., 0].astype(np.uint32) << 16) | (im[..., 1].astype(np.uint32) << 8) | im[..., 2].astype(np.uint32)
    )
    sub = packed[::4, ::4]
    mean = float(luma.mean())
    dark = float((luma < 40).mean())
    std = float(luma.std())
    return {
        "mean_luma": round(mean, 2),
        "luma_std": round(std, 2),
        "dark_fraction": round(dark, 4),
        "distinct_colors_subsampled": int(np.unique(sub).size),
        "black_screen_suspect": bool(mean < 40 or dark > 0.5),
        "blank_suspect": bool(std < 3.0),
    }


def frame_signature(path: Path) -> str:
    import hashlib

    return hashlib.sha256(Path(path).read_bytes()).hexdigest()[:32]


def frame_progress(region: tuple[int, int, int, int], samples: int = 30, interval: float = 0.05,
                   stride: int = 401) -> dict:
    """快速连续抓取同一区域，估计「绘制是否在推进」（不是精确帧率）。

    用途：当 CPU 下降时排除「因为绘制帧变少（画面更卡）而 CPU 下降」这一解释。
    返回相邻两帧有多少对发生了变化，以及抽样字节的平均变化比例。
    注意：抓图本身有开销，采样间隔只能到 ~10–40 ms 量级，只能分辨「明显更少帧 / 停摆」。
    """
    import hashlib

    from gi.repository import Gdk

    root = Gdk.get_default_root_window()
    x, y, w, h = region
    sigs: list[str] = []
    deltas: list[float] = []
    prev: bytes | None = None
    intervals: list[float] = []
    for _ in range(samples):
        t = time.monotonic()
        pb = Gdk.pixbuf_get_from_window(root, x, y, w, h)
        if pb is None:
            return {"error": "抓图失败（pixbuf 为空）"}
        data = pb.get_pixels()
        if not isinstance(data, bytes):
            data = bytes(data)
        sigs.append(hashlib.sha256(data).hexdigest()[:16])
        if prev is not None:
            a = prev[::stride]
            b = data[::stride]
            diff = sum(1 for u, v in zip(a, b) if u != v)
            deltas.append(diff / max(1, len(a)))
        prev = data
        intervals.append(time.monotonic() - t)
        time.sleep(interval)
    return {
        "samples": samples,
        "distinct_frames": len(set(sigs)),
        "changed_pairs": sum(1 for i in range(1, len(sigs)) if sigs[i] != sigs[i - 1]),
        "pairs": len(sigs) - 1,
        "mean_delta_fraction": round(sum(deltas) / len(deltas), 5) if deltas else None,
        "max_delta_fraction": round(max(deltas), 5) if deltas else None,
        "capture_seconds_mean": round(sum(intervals) / len(intervals), 4) if intervals else None,
        "region": list(region),
        "note": "近似指标：抓图有开销，只能分辨「明显更少帧 / 停摆」，不是精确帧率",
    }


# ---------------------------------------------------------------------------
# 一次性 Xvfb（只回收本次自己启动的显示）
# ---------------------------------------------------------------------------


class OwnXvfb:
    def __init__(self, display: str, size: str = "1440x1000x24", log: str = "/tmp/opencode/od027-xvfb-step3.log") -> None:
        self.display = display
        self.num = display.lstrip(":")
        self.size = size
        self.log = log
        self.proc: subprocess.Popen | None = None

    def __enter__(self):
        if Path(f"/tmp/.X{self.num}-lock").exists():
            raise RuntimeError(f"{self.display} 已被占用（/tmp/.X{self.num}-lock 存在）")
        fh = open(self.log, "a", buffering=1)
        self.proc = subprocess.Popen(
            ["Xvfb", self.display, "-screen", "0", self.size, "-nolisten", "tcp"],
            stdout=fh, stderr=subprocess.STDOUT, start_new_session=True,
        )
        time.sleep(1.5)
        if self.proc.poll() is not None:
            raise RuntimeError(f"Xvfb 提前退出，见 {self.log}")
        return self

    def __exit__(self, *exc):
        if self.proc is not None and self.proc.poll() is None:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=10)
            except Exception:
                try:
                    os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                except Exception:
                    pass
        return False


def base_env_snapshot(display: str, dmabuf: str, extra: dict | None = None) -> dict:
    import json

    snap = {
        "script": str(HERE),
        "script_version": SCRIPT_VERSION,
        "repo_head": subprocess.run(["git", "rev-parse", "HEAD"], cwd=str(REPO), capture_output=True, text=True).stdout.strip(),
        "display_backend": f"Xvfb {display}（无 GPU / 无 DRI3；GDK_BACKEND=x11，清空 WAYLAND_DISPLAY/SOCKET）",
        "webkit_disable_dmabuf_renderer": dmabuf,
        "generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
        "generated_at_epoch": time.time(),
        "invocation": " ".join(sys.argv),
    }
    if extra:
        snap.update(extra)
    return snap


def write_json(path: Path, payload: dict) -> None:
    import json

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def percentile(values: list[float], p: float):
    import math

    if not values:
        return None
    s = sorted(values)
    if len(s) == 1:
        return round(s[0], 3)
    k = (len(s) - 1) * (p / 100.0)
    f, c = math.floor(k), math.ceil(k)
    if f == c:
        return round(s[int(k)], 3)
    return round(s[f] * (c - k) + s[c] * (k - f), 3)
