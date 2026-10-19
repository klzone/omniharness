#!/usr/bin/env python3
"""OD-ARCH-027 Step 3 · B 组：应用级 DMABUF 独立对照（真实 OmniDesk 窗口）。

在同一一次性 Xvfb（无 GPU）上，用真实 debug+vite dev 桌面窗口，分别在
`WEBKIT_DISABLE_DMABUF_RENDERER=1`（现状默认）与 `=0`（启用）下各跑 ≥3 次，记录：
  * 空闲 60 s 的 WebKitWebProcess / App 主进程 / 网络进程 / GPU 进程 CPU 均值；
  * 导航延迟 P50/P95（固定路径 首页→任务工作台→离线→设置→首页，含截图读回开销）；
  * 画面异常：逐页截图 + 5 帧稳定性采样，做黑屏/空白/疑似闪烁判据；
  * 输入响应（可选）：设置页输入固定 ASCII 串并保存，以设置文件是否**逐字**包含该串为准。

DMABUF 覆盖方式：`AppSession.env_extra` 在 `app_env()` 中最后 `env.update(...)`，
因此**无需改动共享的 window_harness.py**（其默认值仍为 1）。本脚本同时读取 App 与
WebKitWebProcess 的 /proc/<pid>/environ 核对变量确实生效。

用法：
  python3 step3-app.py --dmabuf 1 --runs 3 --display :94 --own-xvfb
  python3 step3-app.py --dmabuf 0 --runs 3 --display :94 --own-xvfb
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _step3lib import (  # noqa: E402
    REPO,
    SCRIPT_VERSION,
    STEP3,
    base_env_snapshot,
    frame_signature,
    frame_stats,
    percentile,
    proc_env,
    write_json,
)

sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
from window_harness import APP_BIN, AppSession, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402

# Dock 项名 → 窗口内相对 y（1280x820；与 Step 2 校准一致，见 step2/calibration.json）
NAV_Y = {
    "home": 235, "report": 285, "writing": 335, "project": 385,
    "workbench": 435, "sessions": 485, "settings": 535, "engine": 585,
}
NAV_PATH = [
    ("workbench", "任务工作台"),
    ("project", "离线流程·项目跟踪"),
    ("settings", "成果目录与模型权限"),
    ("home", "工作台首页"),
]
PAGE_SHOTS = [
    ("home", "首页"), ("workbench", "任务工作台"),
    ("project", "离线流程·项目跟踪"), ("settings", "成果目录与模型权限"),
]


def goto_scene(xs: XSession, win, geo, name: str, settle: float = 1.6) -> None:
    xs.dismiss_overlays()
    xs.focus(win)
    xs.click(geo.x + 1240, geo.y + 405, settle=0.25)
    xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=settle)


def content_sig(xs: XSession, geo, tmp: Path) -> str:
    from PIL import Image

    xs.shot(tmp, geo)
    img = Image.open(tmp).crop((60, 120, geo.w, geo.h))
    return hashlib.sha256(img.tobytes()).hexdigest()


def nav_latency(xs, win, geo, tmp, repeats: int) -> dict:
    samples: dict[str, list[float]] = {n: [] for n, _ in NAV_PATH}
    timeouts = 0
    for _ in range(repeats):
        for name, _lbl in NAV_PATH:
            before = content_sig(xs, geo, tmp)
            t = time.monotonic()
            xs.dismiss_overlays()
            xs.focus(win)
            xs.click(geo.x + 1240, geo.y + 405, settle=0.05)
            xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=0.05)
            changed = None
            for _ in range(80):
                time.sleep(0.15)
                if content_sig(xs, geo, tmp) != before:
                    changed = time.monotonic() - t
                    break
            if changed is None:
                timeouts += 1
            else:
                samples[name].append(round(changed, 3))
    out = {}
    for name, lbl in NAV_PATH:
        vals = samples[name]
        out[name] = {"label": lbl, "n": len(vals), "samples_s": vals,
                     "p50_s": percentile(vals, 50), "p95_s": percentile(vals, 95)}
    allv = [v for vals in samples.values() for v in vals]
    out["_all"] = {"n": len(allv), "p50_s": percentile(allv, 50),
                   "p95_s": percentile(allv, 95), "timeouts": timeouts}
    return out


def input_test(d: Driver, xs: XSession, app: AppSession, run_tag: str) -> dict:
    """设置页输入固定 ASCII 串 → 保存 → 以设置文件是否逐字包含该串判定是否丢字。"""
    target = Path(f"/tmp/od027s3/{run_tag}")
    target.mkdir(parents=True, exist_ok=True)
    typed = str(target)
    settings_file = app.app_data / "config" / "settings.v1.json"
    out: dict = {"typed": typed, "settings_file": str(settings_file)}
    try:
        d.scroll_top()
        loc = d.locate_primary(500, 780)
        if loc is None:
            out.update(status="device_failed", reason="未定位到「保存路径修改」主按钮")
            return out
        bx, by = loc
        out["save_button"] = [bx, by]
        attempts = []
        for dy in (76, 66, 86, 96, 56):
            d.click(bx, by - dy, settle=0.6)
            xs.select_all_and_clear()
            d.type_text(typed, settle=0.25)
            d.click(bx, by, settle=2.2)
            raw = settings_file.read_text(encoding="utf-8") if settings_file.is_file() else ""
            hit = typed in raw
            partial = (not hit) and len(typed) > 6 and typed[:6] in raw
            attempts.append({"input_offset_dy": dy, "exact_hit": hit, "partial_prefix_in_file": partial,
                             "settings_excerpt": raw[:300]})
            if hit:
                out.update(status="ok", attempts=attempts,
                           note="设置文件逐字包含所输入路径 → 未观察到丢字")
                return out
            if partial:
                out.update(status="lost_chars_suspect", attempts=attempts,
                           note="设置文件含前缀但不含完整串 → 疑丢字")
                return out
        out.update(status="device_failed", attempts=attempts,
                   reason="多次尝试后设置文件仍未出现该串（装置未生效，不能判定丢字）")
    except Exception as exc:
        out.update(status="device_failed", reason=f"{type(exc).__name__}: {exc}")
    return out


def run_one(dmabuf: str, run_idx: int, args) -> dict:
    tag = f"step3-B-dmabuf{dmabuf}-run{run_idx}"
    root = Path(args.root_base) / f"dmabuf{dmabuf}-run{run_idx}"
    out = STEP3 / f"{tag}.json"
    shots = STEP3 / "shots"
    shots.mkdir(parents=True, exist_ok=True)
    payload: dict = {
        "step": "step3", "group": "B-app", "subject": "真实 OmniDesk 桌面窗口",
        "dmabuf": dmabuf, "run": run_idx, "verdict": "started",
    }
    app = AppSession(root=root, display=args.display,
                     env_extra={"WEBKIT_DISABLE_DMABUF_RENDERER": dmabuf})
    app.prepare()
    xs = XSession(args.display)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            payload["verdict"] = "fail:未找到窗口"
            return payload
        geo = xs.geometry(win)
        xs.focus(win)
        payload["window"] = {"title": xs.title(win), "geometry": geo.as_tuple()}
        payload["app_pid"] = app.proc.pid
        payload["dmabuf_env_check"] = {
            "app_main": proc_env(app.proc.pid).get("WEBKIT_DISABLE_DMABUF_RENDERER"),
        }
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(args.settle)

        # --- 逐页截图 + 画面异常判据 ---
        payload["page_frames"] = {}
        for scene, label in PAGE_SHOTS:
            goto_scene(xs, win, geo, scene)
            png = shots / f"{tag}-page-{scene}.png"
            d.shot(png)
            payload["page_frames"][label] = {"path": str(png), **frame_stats(png)}

        # --- 回首页、稳定，然后连续 idle-seconds 秒量空闲 CPU（不受截图干扰）---
        goto_scene(xs, win, geo, "home")
        time.sleep(8)
        idle_png = shots / f"{tag}-idle-start.png"
        d.shot(idle_png)
        payload["idle_frame"] = {"path": str(idle_png), **frame_stats(idle_png)}

        from _step3lib import sample_cpu, webkit_process_groups

        groups = webkit_process_groups()
        groups["app_main"] = [app.proc.pid]
        payload["idle_cpu"] = sample_cpu(groups, args.idle_seconds, curve_every=5.0)
        payload["dmabuf_env_check"].update(
            {f"pid_{p}": proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER")
             for p in groups["webkit_web_process"] + groups["webkit_gpu_process"]}
        )
        end_png = shots / f"{tag}-idle-end.png"
        d.shot(end_png)
        payload["idle_end_frame"] = {"path": str(end_png), **frame_stats(end_png)}

        # --- 空闲期 5 帧稳定性（疑似闪烁）---
        stab = []
        for i in range(5):
            p = shots / f"{tag}-stab-{i}.png"
            xs.shot(p, geo)
            stab.append({"path": str(p), "sig": frame_signature(p), **frame_stats(p)})
            time.sleep(2.0)
        payload["stability_frames"] = stab
        payload["stability"] = {
            "distinct_signatures": len({s["sig"] for s in stab}),
            "luma_min": min(s["mean_luma"] for s in stab),
            "luma_max": max(s["mean_luma"] for s in stab),
            "note": "5 帧间隔 2 s；distinct_signatures=1 表示静态画面稳定（无闪烁迹象）",
        }

        # --- 导航延迟 ---
        payload["nav_latency"] = nav_latency(xs, win, geo, root / "_sig.png", args.repeats)
        payload["nav_latency_note"] = (
            "点击 → 轮询内容帧签名变化；每次轮询含 GDK 稳定截图读回（≥0.35 s 量级），"
            "该延迟是交互端到端上界，含截图开销，只作同口径相对参考。"
        )

        # --- 输入响应（可选）---
        if args.input_test:
            goto_scene(xs, win, geo, "settings")
            payload["input_test"] = input_test(d, xs, app, f"B_dmabuf{dmabuf}_run{run_idx}")

        payload["env"] = base_env_snapshot(args.display, dmabuf, {
            "build": "debug + vite dev（前端 dev server，非内嵌产物）",
            "data_root": str(root),
            "binary": str(APP_BIN),
            "binary_mtime": APP_BIN.stat().st_mtime if APP_BIN.exists() else None,
            "window_size": "1280x820（tauri.conf 默认，未 resize）",
            "idle_seconds": args.idle_seconds,
            "nav_repeats": args.repeats,
            "dmabuf_override_mechanism": "AppSession.env_extra（window_harness.py 未改动，其默认仍为 1）",
            "wayland_comparison": "未做：本沙箱为无 GPU 的一次性 Xvfb，只能跑 GDK_BACKEND=x11；Wayland↔XWayland 对照留待 Step 5 由用户在真实桌面执行",
        })
        payload["verdict"] = "ok"
    except Exception as exc:
        payload["verdict"] = f"error:{type(exc).__name__}: {exc}"
    finally:
        try:
            app.stop_all()
        except Exception:
            pass
    write_json(out, payload)
    return payload


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description="Step 3 B 组：应用级 DMABUF 对照")
    ap.add_argument("--dmabuf", required=True, choices=["0", "1"])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--display", default=":94")
    ap.add_argument("--root-base", default="/tmp/od027-step3")
    ap.add_argument("--idle-seconds", type=float, default=60.0)
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--settle", type=float, default=12.0)
    ap.add_argument("--input-test", action="store_true")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])

    from _step3lib import OwnXvfb, assert_no_webkit

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    runs = []
    with ctx:
        for i in range(1, args.runs + 1):
            assert_no_webkit(f"B dmabuf={args.dmabuf} run{i} 开始前")
            p = run_one(args.dmabuf, i, args)
            ic = p.get("idle_cpu", {})
            web = ic.get("webkit_web_process", {})
            gpu = ic.get("webkit_gpu_process", {})
            luma = p.get("idle_frame", {}).get("mean_luma")
            print(
                f"  [B dmabuf={args.dmabuf} run{i}] verdict={p['verdict']} "
                f"web={web.get('avg_percent')}% gpu={gpu.get('avg_percent')}(n={gpu.get('process_count')}) "
                f"nav={p.get('nav_latency', {}).get('_all')} luma={luma} "
                f"black={p.get('idle_frame', {}).get('black_screen_suspect')}"
            )
            runs.append({
                "run": i, "verdict": p["verdict"], "json": str(STEP3 / f"step3-B-dmabuf{args.dmabuf}-run{i}.json"),
                "web_avg_percent": web.get("avg_percent"),
                "web_count": web.get("process_count"),
                "gpu_avg_percent": gpu.get("avg_percent"),
                "gpu_count": gpu.get("process_count"),
                "network_count": ic.get("webkit_network_process", {}).get("process_count"),
                "app_main_avg_percent": ic.get("app_main", {}).get("avg_percent"),
                "nav_all": p.get("nav_latency", {}).get("_all"),
                "idle_mean_luma": luma,
                "black_screen_suspect": p.get("idle_frame", {}).get("black_screen_suspect"),
                "stability": p.get("stability"),
                "input_test": (p.get("input_test") or {}).get("status"),
                "dmabuf_env_check": p.get("dmabuf_env_check"),
            })
    write_json(STEP3 / f"step3-B-summary-dmabuf{args.dmabuf}.json",
               base_env_snapshot(args.display, args.dmabuf, {"runs": runs, "script_version": SCRIPT_VERSION}))
    print(f"[B dmabuf={args.dmabuf}] {len(runs)} run(s) 完成")
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
