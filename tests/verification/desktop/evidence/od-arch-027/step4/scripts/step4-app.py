#!/usr/bin/env python3
"""OD-ARCH-027 Step 4 · Phase B 应用级验证（真实窗口）。

两种模式：
  * 默认（无 `--engine-healthy`）：逐页截图 + 空闲 CPU 60 s + 导航延迟 + 设置页输入测试
    （先「快速注入」再「分块慢速注入」，用设置文件事实比对是否丢字）。
  * `--engine-healthy`：**启动前**把 `model_permissions.local = true` 预写入隔离数据根，
    使 `App.tsx` 的启动期准入通过、`engineHealthy === true`，从而**顶栏状态点的
    `animate-pulse` 真正在跑**（这是 Step 4 单因素对照指向的真实风险场景）。
    记录顶栏状态点颜色（emerald=healthy / amber=offline）、空闲 CPU 60 s 与 5 帧稳定性。

复用 Step 3 的 `_step3lib`（只读导入，不修改 Step 3 证据）。DMABUF 固定 `1`（dev-ui.sh 现状默认）。

用法：
  python3 step4-app.py --label step4-default-run1 --display :96 --own-xvfb
  python3 step4-app.py --label step4-engine-healthy-run1 --engine-healthy --display :96 --own-xvfb
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
OD027 = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027"
STEP4 = OD027 / "step4"
sys.path.insert(0, str(OD027 / "step3" / "scripts"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

from _step3lib import (  # noqa: E402
    base_env_snapshot,
    frame_progress,
    frame_signature,
    frame_stats,
    percentile,
    proc_env,
    sample_cpu,
    webkit_process_groups,
    write_json,
)

from window_harness import APP_BIN, AppSession, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402

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


def goto_scene(xs, win, geo, name, settle=1.6):
    xs.dismiss_overlays()
    xs.focus(win)
    xs.click(geo.x + 1240, geo.y + 405, settle=0.25)
    xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=settle)


def content_sig(xs, geo, tmp) -> str:
    from PIL import Image

    xs.shot(tmp, geo)
    img = Image.open(tmp).crop((60, 120, geo.w, geo.h))
    return hashlib.sha256(img.tobytes()).hexdigest()


def detect_status_chip(png: Path) -> dict:
    """在顶栏条带里找状态点/状态标签的色相（emerald=healthy / amber=offline）。"""
    import numpy as np
    from PIL import Image

    im = np.asarray(Image.open(png).convert("RGB")).astype(int)
    strip = im[20:84, 90:560]
    r, g, b = strip[..., 0], strip[..., 1], strip[..., 2]
    emerald = (g > 105) & (g - r > 25) & (g - b > 15)
    amber = (r > 145) & (r - b > 55) & (g > 95)
    e, a = int(emerald.sum()), int(amber.sum())
    if e > 5 and e >= a:
        state = "healthy(emerald)"
    elif a > 5:
        state = "offline(amber)"
    else:
        state = "unknown"
    return {"emerald_px": e, "amber_px": a, "state": state}


def crop_topbar(src: Path, out: Path, box=(0, 0, 1280, 120)) -> dict:
    """裁出顶栏区域并 2× 放大，用于核对状态点仍存在、颜色仍表达状态。"""
    from PIL import Image

    im = Image.open(src)
    w, h = im.size
    b = (box[0], box[1], min(box[2], w), min(box[3], h))
    crop = im.crop(b)
    crop.save(out)
    up_path = out.with_name(out.stem + "-2x" + out.suffix)
    crop.resize((crop.width * 2, crop.height * 2), Image.LANCZOS).save(up_path)
    return {"crop": str(out), "upscaled_2x": str(up_path), "box": list(b), "source_size": [w, h]}


def nav_latency(xs, win, geo, tmp, repeats) -> dict:
    samples = {n: [] for n, _ in NAV_PATH}
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
        v = samples[name]
        out[name] = {"label": lbl, "n": len(v), "samples_s": v,
                     "p50_s": percentile(v, 50), "p95_s": percentile(v, 95)}
    allv = [x for v in samples.values() for x in v]
    out["_all"] = {"n": len(allv), "p50_s": percentile(allv, 50), "p95_s": percentile(allv, 95), "timeouts": timeouts}
    return out


def input_attempt(d: Driver, xs: XSession, settings_file: Path, target: str, fast: bool) -> dict:
    """一次输入尝试：定位保存按钮 → 点输入框 → 清空 → 输入 → 保存 → 比对设置文件。"""
    out = {"typed": target, "mode": "fast(type_ascii settle=0.02)" if fast else "slow(chunk=4,pause=0.6)"}
    d.scroll_top()
    loc = d.locate_primary(500, 780)
    if loc is None:
        out.update(status="device_failed", reason="未定位到保存按钮")
        return out
    bx, by = loc
    out["save_button"] = [bx, by]
    d.click(bx, by - 76, settle=0.6)
    xs.select_all_and_clear()
    if fast:
        xs.type_ascii(target, settle=0.02)
    else:
        d.type_text(target, settle=0.25, chunk=4, pause=0.6)
    d.click(bx, by, settle=2.2)
    raw = settings_file.read_text(encoding="utf-8") if settings_file.is_file() else ""
    out["settings_excerpt"] = raw[:300]
    if target in raw:
        out["status"] = "exact_hit"
    elif len(target) > 6 and target[:6] in raw:
        out["status"] = "partial_prefix_in_file(疑丢字)"
    else:
        out["status"] = "no_match(未落盘/被拒绝)"
    return out


def run_one(args) -> dict:
    tag = args.label
    root = Path(args.root_base) / tag
    out_json = STEP4 / f"{tag}.json"
    shots = STEP4 / f"shots-{tag}"
    shots.mkdir(parents=True, exist_ok=True)
    payload: dict = {
        "step": "step4", "label": tag, "mode": "engine-healthy" if args.engine_healthy else "default",
        "dmabuf": args.dmabuf, "verdict": "started",
    }
    app = AppSession(root=root, display=args.display,
                     env_extra={"WEBKIT_DISABLE_DMABUF_RENDERER": args.dmabuf})
    app.prepare()
    if args.reduced_motion:
        # 让 GTK 报告「减少动态效果」：GTK 的 gtk-enable-animations 是 WebKitGTK 映射
        # prefers-reduced-motion 的来源。能否真正生效由实测结果说话（可能不生效）。
        cfg_dir = root / "gtk-cfg"
        (cfg_dir / "gtk-3.0").mkdir(parents=True, exist_ok=True)
        (cfg_dir / "gtk-3.0" / "settings.ini").write_text(
            "[Settings]\ngtk-enable-animations=0\n", encoding="utf-8"
        )
        app.env_extra["XDG_CONFIG_HOME"] = str(cfg_dir)
        payload["reduced_motion_setup"] = {
            "method": "XDG_CONFIG_HOME/gtk-3.0/settings.ini: gtk-enable-animations=0",
            "xdg_config_home": str(cfg_dir),
            "note": "GTK 是否把它映射成 prefers-reduced-motion: reduce 由实测结果判断",
        }
    if args.engine_healthy:
        cfg = app.app_data / "config"
        cfg.mkdir(parents=True, exist_ok=True)
        artifacts = root / "artifacts"
        artifacts.mkdir(parents=True, exist_ok=True)
        settings = {"schema": "omnidesk.settings.v1", "artifact_root": str(artifacts),
                    "model_permissions": {"local": True, "private": False, "public": False}}
        (cfg / "settings.v1.json").write_text(json.dumps(settings, indent=2), encoding="utf-8")
        payload["preseeded_settings"] = settings
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
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(args.settle)

        if args.engine_healthy:
            # 只关心顶栏状态点在跑时的空闲代价
            home_png = shots / f"{tag}-home.png"
            d.shot(home_png)
            payload["home_frame"] = {"path": str(home_png), **frame_stats(home_png)}
            payload["status_chip"] = detect_status_chip(home_png)
            payload["engine_children"] = _engine_children()
            goto_scene(xs, win, geo, "home")
            time.sleep(8)
            groups = webkit_process_groups()
            groups["app_main"] = [app.proc.pid]
            payload["idle_cpu"] = sample_cpu(groups, args.idle_seconds, curve_every=5.0)
            payload["paint_progress_home"] = frame_progress((80, 90, 1360, 910), samples=30, interval=0.03)
            stab = []
            for i in range(5):
                p = shots / f"{tag}-stab-{i}.png"
                xs.shot(p, geo)
                stab.append({"path": str(p), "sig": frame_signature(p), "chip": detect_status_chip(p)})
                time.sleep(2.0)
            payload["stability_frames"] = stab
            payload["stability"] = {
                "distinct_signatures": len({s["sig"] for s in stab}),
                "chip_states": [s["chip"]["state"] for s in stab],
            }
            payload["topbar_crop"] = crop_topbar(home_png, shots / f"{tag}-topbar.png")
            payload["nav_latency"] = nav_latency(xs, win, geo, root / "_sig.png", args.repeats)
            payload["nav_latency_note"] = "含 GDK 截图读回开销，只作同口径相对参考"
        else:
            payload["page_frames"] = {}
            for scene, label in PAGE_SHOTS:
                goto_scene(xs, win, geo, scene)
                png = shots / f"{tag}-page-{scene}.png"
                d.shot(png)
                payload["page_frames"][label] = {"path": str(png), **frame_stats(png)}
            goto_scene(xs, win, geo, "home")
            time.sleep(8)
            idle_png = shots / f"{tag}-idle-start.png"
            d.shot(idle_png)
            payload["idle_frame"] = {"path": str(idle_png), **frame_stats(idle_png)}
            payload["status_chip"] = detect_status_chip(idle_png)
            groups = webkit_process_groups()
            groups["app_main"] = [app.proc.pid]
            payload["idle_cpu"] = sample_cpu(groups, args.idle_seconds, curve_every=5.0)
            payload["nav_latency"] = nav_latency(xs, win, geo, root / "_sig.png", args.repeats)
            payload["nav_latency_note"] = "含 GDK 截图读回开销，只作同口径相对参考"
            if args.input_test:
                settings_file = app.app_data / "config" / "settings.v1.json"
                goto_scene(xs, win, geo, "settings")
                tests = {}
                for mode, fast in (("fast", True), ("slow", False)):
                    target = Path(f"/tmp/od027s4/{tag}_{mode}")
                    target.mkdir(parents=True, exist_ok=True)
                    goto_scene(xs, win, geo, "settings")
                    tests[mode] = input_attempt(d, xs, settings_file, str(target), fast)
                    d.shot(shots / f"{tag}-input-{mode}.png")
                payload["input_tests"] = tests
        payload["dmabuf_env_check"] = {
            "app_main": proc_env(app.proc.pid).get("WEBKIT_DISABLE_DMABUF_RENDERER"),
        }
        payload["env"] = base_env_snapshot(args.display, args.dmabuf, {
            "build": "debug + vite dev（前端 dev server，非内嵌产物）",
            "data_root": str(root),
            "binary": str(APP_BIN),
            "binary_mtime": APP_BIN.stat().st_mtime if APP_BIN.exists() else None,
            "window_size": "1280x820（tauri.conf 默认，未 resize）",
            "idle_seconds": args.idle_seconds,
            "nav_repeats": args.repeats,
            "engine_healthy_mode": args.engine_healthy,
            "note": "engine-healthy 模式在启动前预写 model_permissions.local=true，使顶栏状态点 animate-pulse 真正运行",
        })
        payload["verdict"] = "ok"
    except Exception as exc:
        payload["verdict"] = f"error:{type(exc).__name__}: {exc}"
    finally:
        try:
            app.stop_all()
        except Exception:
            pass
    write_json(out_json, payload)
    return payload


def _engine_children() -> list[str]:
    import subprocess

    out = subprocess.run(["ps", "-eo", "pid,args"], capture_output=True, text=True).stdout
    hits = []
    for line in out.splitlines()[1:]:
        if any(k in line for k in ("omnidesk_engine", "dsh-runtime", "deepseek-harness")):
            if "ps -eo" in line:
                continue
            hits.append(line.strip()[:140])
    return hits


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description="Step 4 应用级验证")
    ap.add_argument("--label", required=True)
    ap.add_argument("--display", default=":96")
    ap.add_argument("--root-base", default="/tmp/od027-step4")
    ap.add_argument("--dmabuf", default="1", choices=["0", "1"])
    ap.add_argument("--idle-seconds", type=float, default=60.0)
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--settle", type=float, default=12.0)
    ap.add_argument("--engine-healthy", action="store_true")
    ap.add_argument("--reduced-motion", action="store_true")
    ap.add_argument("--input-test", action="store_true")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])

    from _step3lib import OwnXvfb, assert_no_webkit

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    with ctx:
        assert_no_webkit(f"{args.label} 开始前")
        p = run_one(args)
    ic = p.get("idle_cpu", {})
    web = ic.get("webkit_web_process", {})
    print(f"[{args.label}] verdict={p['verdict']} chip={p.get('status_chip')} "
          f"web={web.get('avg_percent')}% app={ic.get('app_main', {}).get('avg_percent')}% "
          f"nav={(p.get('nav_latency') or {}).get('_all')} "
          f"stability={p.get('stability')} engines={len(p.get('engine_children') or [])}")
    for k, v in (p.get("input_tests") or {}).items():
        print(f"   input[{k}]: {v.get('status')} — typed={v.get('typed')}")
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
