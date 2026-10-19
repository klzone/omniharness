#!/usr/bin/env python3
"""OD-ARCH-027 · Step 2 桌面渲染性能测量（落盘版，替代 Step 0 只打印到 stdout 的旧脚本）。

口径（计划 §6；本脚本内嵌版本标识 step2-perf.py v1）：
  * 主口径：**debug + vite dev**，`WEBKIT_DISABLE_DMABUF_RENDERER` 保持默认 "1"
    （Step 2 只动 CSS，不与 DMABUF 变量混同）。
  * 空闲 CPU：窗口停在首页，稳定后**连续 60 s**，分别统计
    `WebKitWebProcess` / `WebKitNetworkProcess` / `WebKitGPUProcess` / App 主进程
    的 /proc/<pid>/stat 的 utime+stime 增量 ÷ 墙钟，输出 60 s 平均百分比。
    WebKit 子进程可能被 bubblewrap 重挂父进程，故**按 cmdline 全局匹配**；
    运行前确认系统上只有本次这一个 WebKit 应用。
  * 导航延迟：固定路径 首页→任务工作台→离线(项目跟踪)→设置→首页，每项 ≥5 次，
    输出 P50/P95。方式沿用「点击后轮询内容帧签名直到变化」。
    **该延迟含 GDK 截图读回开销，只作相对参考，不是纯渲染延迟。**
  * 逐页截图：首页/任务工作台/离线/设置。
  * 每次运行落盘：结果 JSON（含环境快照）+ 截图 + 脚本路径 + 二进制路径与 mtime。

只使用合成端点与仓库外一次性数据根；不读凭证、不碰 runtime/、不改第二 worktree。

用法：
  python3 step2-perf.py --label step2-before-debug-vite --variant before
  python3 step2-perf.py --label step2-after-debug-vite  --variant after
  python3 step2-perf.py --calibrate            # 逐个点击 Dock 项并抓图（校验坐标）
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
STEP2 = HERE.parents[1]              # .../evidence/od-arch-027/step2
# 向上定位仓库根（含 desktop/src-tauri），避免硬编码层数
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
DESKTOP = REPO / "desktop"
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

from window_harness import APP_BIN, AppSession, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402

SCRIPT_VERSION = "step2-perf.py v1"

# Dock 项名 → 窗口内相对 y（1280x820，按 Dock 布局与实测校准；见 evidence README）
NAV_Y = {
    "home": 235,
    "report": 285,
    "writing": 335,
    "project": 385,     # 「离线流程」入口（项目跟踪），渲染 .ofl-v2
    "workbench": 435,
    "sessions": 485,
    "settings": 535,
    "engine": 585,
}
# 固定导航路径（首页 → 任务工作台 → 离线 → 设置 → 首页）
NAV_PATH = [
    ("workbench", "任务工作台"),
    ("project", "离线流程·项目跟踪"),
    ("settings", "成果目录与模型权限"),
    ("home", "工作台首页"),
]
PAGE_SHOTS = [
    ("home", "home", "首页"),
    ("workbench", "workbench", "任务工作台"),
    ("project", "offline", "离线流程·项目跟踪"),
    ("settings", "settings", "成果目录与模型权限"),
]

TICKS_PER_SEC = 100.0  # 本机 USER_HZ=100（与 Step 0 旧脚本口径一致）


# ---------------------------------------------------------------------------
# /proc 辅助
# ---------------------------------------------------------------------------


def proc_ticks(pid: int):
    """返回 (utime+stime) ticks；失败返回 None。"""
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
        parts = stat.split(") ", 1)[1].split()
        return int(parts[11]) + int(parts[12])
    except Exception:
        return None


def proc_cmdline(pid: int) -> str:
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode().strip()
    except Exception:
        return ""


def webkit_pids(kind: str) -> list[int]:
    """按 argv[0]（真实可执行文件路径）全局匹配 WebKit 子进程。

    bubblewrap 沙箱会把 WebKit 子进程重挂到别的父进程下，故不能只按 ppid 找；
    用 argv[0] 而不是整条 cmdline，避免把「命令行里恰好含该字符串」的其它进程算进来。
    """
    out = []
    for cand in Path("/proc").iterdir():
        if not cand.name.isdigit():
            continue
        try:
            raw = Path(f"/proc/{cand.name}/cmdline").read_bytes()
        except Exception:
            continue
        argv0 = raw.split(b"\0", 1)[0].decode(errors="replace")
        if argv0.endswith("/" + kind):
            out.append(int(cand.name))
    return out


def percentile(values: list[float], p: float):
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


# ---------------------------------------------------------------------------
# 内容帧签名（与 run_window_acceptance.Driver._content_sig 同一裁剪口径）
# ---------------------------------------------------------------------------


def content_sig(xs: XSession, geo, tmp: Path) -> str:
    from PIL import Image

    xs.shot(tmp, geo)
    img = Image.open(tmp).crop((60, 120, geo.w, geo.h))
    return hashlib.sha256(img.tobytes()).hexdigest()


def goto_scene(xs: XSession, win, geo, name: str, settle: float = 1.6) -> None:
    """切场景：点 Dock 项（坐标见 NAV_Y）。

    不复用 run_window_acceptance.Driver.page —— 它的 NAV 常量对应旧的 6 项 Dock，
    与 OD-ARCH-026 之后的 8 项 Dock 不一致，会点错入口。
    """
    xs.dismiss_overlays()
    xs.focus(win)
    xs.click(geo.x + 1240, geo.y + 405, settle=0.25)
    xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=settle)


# ---------------------------------------------------------------------------
# Xvfb 生命周期（只管理本次自己启动的显示）
# ---------------------------------------------------------------------------


class OwnXvfb:
    def __init__(self, display: str, size: str = "1440x1000x24") -> None:
        self.display = display
        self.num = display.lstrip(":")
        self.size = size
        self.proc: subprocess.Popen | None = None
        self.started = False

    def __enter__(self):
        if Path(f"/tmp/.X{self.num}-lock").exists():
            raise RuntimeError(f"{self.display} 已被占用（/tmp/.X{self.num}-lock 存在）")
        log = open("/tmp/opencode/od027-xvfb.log", "a", buffering=1)
        self.proc = subprocess.Popen(
            ["Xvfb", self.display, "-screen", "0", self.size, "-nolisten", "tcp"],
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        self.started = True
        time.sleep(1.5)
        if self.proc.poll() is not None:
            raise RuntimeError("Xvfb 提前退出，见 /tmp/opencode/od027-xvfb.log")
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


# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------


def env_snapshot(app: AppSession, display: str, args) -> dict:
    binary = app.binary or APP_BIN
    return {
        "build": "debug + vite dev（前端 dev server，非内嵌产物）",
        "use_dev_server": True,
        "binary": str(binary),
        "binary_mtime": binary.stat().st_mtime if binary.exists() else None,
        "data_root": str(app.root),
        "display_backend": f"Xvfb {display}（无窗口管理器；GDK_BACKEND=x11，清空 WAYLAND_DISPLAY/SOCKET）",
        "webkit_disable_dmabuf_renderer": "1（AppSession 固定值，与 scripts/dev-ui.sh 默认一致；Step 2 保持 DMABUF 默认，不与变量混同）",
        "window_size": f"{args.win_w}x{args.win_h}（tauri.conf 默认，未 resize）",
        "idle_seconds": args.idle_seconds,
        "nav_repeats": args.repeats,
        "script_path": str(HERE),
        "script_version": SCRIPT_VERSION,
        "repo_head": subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=str(REPO), capture_output=True, text=True
        ).stdout.strip(),
        "invocation": " ".join(sys.argv),
        "generated_at": time.strftime("%Y-%m-%d %H:%M:%S%z"),
        "generated_at_epoch": time.time(),
    }


def idle_cpu(app: AppSession, args, display: str) -> dict:
    """连续 idle_seconds 秒统计各进程 CPU（60 s 平均 + 5 s 采样曲线）。"""
    app_pid = app.proc.pid
    web = webkit_pids("WebKitWebProcess")
    net = webkit_pids("WebKitNetworkProcess")
    gpu = webkit_pids("WebKitGPUProcess")
    groups = {
        "app_main": [app_pid],
        "webkit_web_process": web,
        "webkit_network_process": net,
        "webkit_gpu_process": gpu,
    }
    pids = sorted({p for v in groups.values() for p in v})
    first = {p: proc_ticks(p) for p in pids}
    t0 = time.monotonic()
    curve = []
    next_sample = 5.0
    while True:
        elapsed = time.monotonic() - t0
        if elapsed >= args.idle_seconds:
            break
        time.sleep(0.5)
        if time.monotonic() - t0 >= next_sample:
            now = {p: proc_ticks(p) for p in pids}
            dt = time.monotonic() - t0
            curve.append(
                {
                    "t": round(dt, 1),
                    **{
                        name: round(
                            100.0 * sum((now[p] or first[p]) - first[p] for p in group) / TICKS_PER_SEC / dt, 1
                        )
                        for name, group in groups.items()
                    },
                }
            )
            next_sample += 5.0
    dt = time.monotonic() - t0
    last = {p: proc_ticks(p) for p in pids}
    result = {"window_seconds": round(dt, 2), "curve_5s": curve}
    for name, group in groups.items():
        missing = [p for p in group if first[p] is None or last[p] is None]
        total = sum((last[p] - first[p]) for p in group if first[p] is not None and last[p] is not None)
        result[name] = {
            "pids": group,
            "cmdlines": {str(p): proc_cmdline(p)[:120] for p in group},
            "avg_percent": round(100.0 * total / TICKS_PER_SEC / dt, 2),
            "missing_pids": missing,
        }
    result["pids_total"] = len(pids)
    return result


def nav_latency(xs: XSession, app: AppSession, win, geo, tmp: Path, args) -> dict:
    """固定路径导航延迟；含截图读回开销，只作相对参考。"""
    samples: dict[str, list[float]] = {name: [] for name, _ in NAV_PATH}
    timeouts = 0
    for _ in range(args.repeats):
        for name, _label in NAV_PATH:
            before = content_sig(xs, geo, tmp)
            t = time.monotonic()
            xs.dismiss_overlays()
            xs.focus(win)
            xs.click(geo.x + 1240, geo.y + 405, settle=0.05)
            xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=0.05)
            changed = None
            for _ in range(80):  # 最多 ~12 s
                time.sleep(0.15)
                if content_sig(xs, geo, tmp) != before:
                    changed = time.monotonic() - t
                    break
            if changed is None:
                timeouts += 1
            else:
                samples[name].append(round(changed, 3))
    out = {}
    for name, label in NAV_PATH:
        vals = samples[name]
        out[name] = {
            "label": label,
            "n": len(vals),
            "samples_s": vals,
            "p50_s": percentile(vals, 50),
            "p95_s": percentile(vals, 95),
        }
    allv = [v for vals in samples.values() for v in vals]
    out["_all"] = {
        "n": len(allv),
        "p50_s": percentile(allv, 50),
        "p95_s": percentile(allv, 95),
        "timeouts": timeouts,
    }
    return out


def run(args) -> dict:
    shots = args.shots_dir
    shots.mkdir(parents=True, exist_ok=True)

    # 前置：确认没有其它 WebKit 应用在跑（否则全局匹配会串台）
    pre_web = webkit_pids("WebKitWebProcess")
    if pre_web:
        raise RuntimeError(f"测量前已存在 WebKitWebProcess：{[(p, proc_cmdline(p)[:80]) for p in pre_web]}")

    app = AppSession(root=Path(args.root), display=args.display)
    app.prepare()
    xs = XSession(args.display)
    payload: dict = {"label": args.label, "variant": args.variant, "verdict": "started"}
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            payload["verdict"] = "fail:未找到窗口"
            return payload
        geo = xs.geometry(win)
        xs.focus(win)
        payload["window"] = {"title": xs.title(win), "geometry": geo.as_tuple()}
        d = Driver(xs, win, geo, Path(args.root) / "_sig.png")
        time.sleep(args.settle)

        # --- 逐页截图（带 nudge 强制重绘，与 Step 0 一致） ---
        for scene, fname, label in PAGE_SHOTS:
            goto_scene(xs, win, geo, scene)
            d.shot(shots / f"{args.label}-page-{fname}.png")
            payload.setdefault("page_shots", {})[label] = str(
                shots / f"{args.label}-page-{fname}.png"
            )

        # --- 回到首页并稳定，然后连续 idle_seconds 秒量空闲 CPU ---
        goto_scene(xs, win, geo, "home")
        time.sleep(8)
        d.shot(shots / f"{args.label}-idle-start.png")
        payload["idle_cpu"] = idle_cpu(app, args, args.display)
        d.shot(shots / f"{args.label}-idle-end.png")

        # --- 导航延迟 ---
        payload["nav_latency"] = nav_latency(xs, app, win, geo, Path(args.root) / "_sig.png", args)
        payload["nav_latency_note"] = (
            "点击 → 轮询内容帧签名变化；每次轮询含 GDK 稳定截图读回（≥0.35 s 量级），"
            "因此该延迟是交互端到端上界，含截图开销，只作 before/after 相对参考。"
        )
        payload["env"] = env_snapshot(app, args.display, args)
        payload["verdict"] = "ok"
    finally:
        app.stop_all()
    return payload


def calibrate(args) -> dict:
    """逐个点击 Dock 项并抓图，用于校验 NAV_Y 坐标（补充证据，不作性能证据）。"""
    out_dir = STEP2 / "calibration"
    out_dir.mkdir(parents=True, exist_ok=True)
    app = AppSession(root=Path(args.root), display=args.display)
    app.prepare()
    xs = XSession(args.display)
    res: dict = {"dock_y": NAV_Y, "shots": {}}
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            res["error"] = "未找到窗口"
            return res
        geo = xs.geometry(win)
        xs.focus(win)
        res["geometry"] = geo.as_tuple()
        for i, name in enumerate(NAV_Y):
            p = out_dir / f"{i:02d}-{name}.png"
            xs.dismiss_overlays()
            xs.focus(win)
            xs.click(geo.x + 1240, geo.y + 405, settle=0.05)
            xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=2.0)
            # 轻量强制重绘（不发滚轮，避免污染布局）
            xs.place(win, geo.x, geo.y, geo.w, geo.h - 1, settle=0.6)
            xs.place(win, geo.x, geo.y, geo.w, geo.h, settle=1.4)
            xs.shot(p, geo)
            res["shots"][name] = str(p)
    finally:
        app.stop_all()
    return res


def main(argv: list) -> int:
    parser = argparse.ArgumentParser(description="OD-ARCH-027 Step 2 性能测量（落盘）")
    parser.add_argument("--label", default=None, help="输出前缀，如 step2-before-debug-vite")
    parser.add_argument("--variant", default=None, choices=[None, "before", "after"])
    parser.add_argument("--display", default=":91")
    parser.add_argument("--root", default="/tmp/od027-step2/run")
    parser.add_argument("--out-dir", default=str(STEP2))
    parser.add_argument("--shots-dir", default=None)
    parser.add_argument("--idle-seconds", type=float, default=60.0)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--settle", type=float, default=12.0)
    parser.add_argument("--win-w", type=int, default=1280)
    parser.add_argument("--win-h", type=int, default=820)
    parser.add_argument("--own-xvfb", action="store_true", help="由本脚本启动并回收一次性 Xvfb")
    parser.add_argument("--calibrate", action="store_true")
    args = parser.parse_args(argv[1:])
    if args.shots_dir is None:
        args.shots_dir = Path(args.out_dir)

    if args.calibrate:
        ctx = OwnXvfb(args.display) if args.own_xvfb else _nullcontext()
        with ctx:
            payload = calibrate(args)
        (Path(args.out_dir) / "calibration.json").write_text(
            json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if not args.label or not args.variant:
        parser.error("--label/--variant 必填（非 --calibrate 时）")

    ctx = OwnXvfb(args.display) if args.own_xvfb else _nullcontext()
    with ctx:
        payload = run(args)
    out = Path(args.out_dir) / f"{args.label}.json"
    out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"[{args.label}] verdict={payload.get('verdict')} → {out}")
    if payload.get("idle_cpu"):
        for k in ("app_main", "webkit_web_process", "webkit_network_process", "webkit_gpu_process"):
            g = payload["idle_cpu"].get(k, {})
            print(f"  {k}: {g.get('avg_percent')}%  pids={g.get('pids')}")
    if payload.get("nav_latency"):
        print(f"  nav_all: {payload['nav_latency']['_all']}")
    return 0


class _nullcontext:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
