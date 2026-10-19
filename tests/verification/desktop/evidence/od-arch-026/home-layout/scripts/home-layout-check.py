#!/usr/bin/env python3
"""OD-ARCH-026 首页版式回归 · 真实窗口验证（一次性 Xvfb + 隔离数据根 + 回环合成端点）。

检查：
  1. 首页初始空对话截图（顶部三模块 + 全宽对话 + 顶部渐隐）。
  2. 默认静态：连续 5 帧（间隔 2 s）签名相同 → 首页没有持续动画/重绘。
  3. 输入一条消息：截图（输入已填入）→ 发送 → 等待助手消息 → 截图。
  4. 四个页面逐页截图（首页 / 任务工作台 / 离线流程·项目跟踪 / 设置）。
  5. 空闲 60 s CPU（引擎健康 + 默认静态场景）。

复用（只读导入）：`window_harness`（AppSession/XSession）、`run_window_acceptance.Driver`、
OD-ARCH-027 step3 的 `_step3lib`（CPU 采样与截图判据）；不修改任何既有文件。

用法：
  python3 home-layout-check.py --label home-layout-run1 --display :97 --own-xvfb
  python3 home-layout-check.py --label calibrate --display :97 --own-xvfb --screenshot-only
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
EVID = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-026" / "home-layout"
SHOTS = EVID / "shots"

sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "step3" / "scripts"))

from window_harness import APP_BIN, AppSession, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402
from _step3lib import (  # noqa: E402
    OwnXvfb,
    assert_no_webkit,
    base_env_snapshot,
    frame_signature,
    frame_stats,
    proc_env,
    sample_cpu,
    webkit_process_groups,
    write_json,
)

NAV_Y = {
    "home": 235, "report": 285, "writing": 335, "project": 385,
    "workbench": 435, "sessions": 485, "settings": 535, "engine": 585,
}
PAGES = [("home", "首页"), ("workbench", "任务工作台"), ("project", "离线流程·项目跟踪"), ("settings", "成果目录与模型权限")]


def goto_scene(xs, win, geo, name, settle=1.6):
    xs.dismiss_overlays()
    xs.focus(win)
    xs.click(geo.x + 1240, geo.y + 405, settle=0.25)
    xs.click(geo.x + 48, geo.y + NAV_Y[name], settle=settle)
    # 把指针移开 Dock，避免 hover 提示遮住正文（仅为截图干净，不改变行为）
    xs.move(geo.x + 900, geo.y + 300)
    time.sleep(0.6)


def newest_task(app_data: Path) -> dict | None:
    """读取最新的任务台账记录（文件事实，非界面推断）。"""
    files = sorted((app_data / "engine-state").glob("omni-*.json"), key=lambda p: p.stat().st_mtime)
    if not files:
        return None
    try:
        d = json.loads(files[-1].read_text(encoding="utf-8"))
    except Exception:
        return None
    return {
        "file": str(files[-1]),
        "omni_task_id": d.get("omni_task_id"),
        "state": d.get("state"),
        "stop_reason": d.get("stop_reason"),
        "error_message": d.get("error_message"),
        "content": str(d.get("content") or "")[:120],
        "created_at": d.get("created_at"),
    }


def message_fraction(png: Path, box=(200, 392, 1262, 645)) -> float:
    """消息区非背景像素占比（只用于判断「消息区是否已有内容」）。"""
    import numpy as np
    from PIL import Image

    im = np.asarray(Image.open(png).convert("L")).astype(int)
    reg = im[box[1]:box[3], box[0]:box[2]]
    med = float(np.median(reg))
    return round(float((np.abs(reg - med) > 12).mean()), 5)


def run(args) -> dict:
    tag = args.label
    root = Path(args.root_base) / tag
    payload: dict = {"step": "od-arch-026-home-layout", "label": tag, "verdict": "started"}
    app = AppSession(root=root, display=args.display,
                     env_extra={"WEBKIT_DISABLE_DMABUF_RENDERER": args.dmabuf})
    app.prepare()
    # 预置「本机模型权限已开启」：一次性数据根内的一次性合成端点，不涉及用户真实配置。
    cfg = app.app_data / "config"
    cfg.mkdir(parents=True, exist_ok=True)
    artifacts = root / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    settings = {
        "schema": "omnidesk.settings.v1",
        "artifact_root": str(artifacts),
        "model_permissions": {"local": True, "private": False, "public": False},
    }
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
        d = Driver(xs, win, geo, root / "_sig.png")
        payload["window"] = {"title": xs.title(win), "geometry": geo.as_tuple()}
        payload["app_pid"] = app.proc.pid
        time.sleep(args.settle)

        def shot(name: str):
            p = SHOTS / f"{tag}-{name}.png"
            d.shot(p)
            return {"path": str(p), **frame_stats(p)}

        home_empty = shot("01-home-empty")
        payload["home_empty"] = home_empty

        # 默认静态：连续 5 帧签名
        stab = []
        for i in range(5):
            p = SHOTS / f"{tag}-20-stability-{i}.png"
            xs.shot(p, geo)
            stab.append({"path": str(p), "sig": frame_signature(p)})
            time.sleep(2.0)
        payload["stability"] = {
            "frames": stab,
            "distinct_signatures": len({s["sig"] for s in stab}),
            "note": "1 = 首页无持续动画/重绘",
        }
        if args.screenshot_only:
            payload["verdict"] = "ok:screenshot-only"
            return payload

        # 输入一条消息
        ix, iy = args.input_xy
        xs.click(geo.x + ix, geo.y + iy, settle=0.5)
        xs.type_ascii(args.prompt, settle=0.02)
        payload["chat_input"] = {"xy": [ix, iy], "typed": args.prompt}
        payload["home_input_filled"] = shot("02-home-input-filled")

        xs.key("Return", settle=0.3)
        time.sleep(1.5)
        payload["home_after_send"] = shot("03-home-after-send")

        # 等待任务终态（文件事实）+ 消息区出现内容
        terminal = {"succeeded", "failed", "cancelled", "interrupted"}
        task = None
        waited = 0.0
        while waited < args.reply_timeout:
            task = newest_task(app.app_data)
            if task and task.get("state") in terminal:
                break
            time.sleep(1.0)
            waited += 1.0
        time.sleep(3.0)
        payload["task_record"] = task
        final_shot = shot("04-home-reply")
        payload["home_reply"] = final_shot
        payload["message_area"] = {
            "empty_fraction": message_fraction(Path(payload["home_empty"]["path"])),
            "after_send_fraction": message_fraction(Path(payload["home_after_send"]["path"])),
            "reply_fraction": message_fraction(Path(final_shot["path"])),
            "note": "消息区非背景像素占比；空对话应接近 0",
        }
        payload["reply"] = {
            "task_state": (task or {}).get("state"),
            "stop_reason": (task or {}).get("stop_reason"),
            "content_excerpt": (task or {}).get("content"),
            "waited_s": waited,
        }

        # 四个页面
        payload["pages"] = {}
        for scene, label in PAGES:
            goto_scene(xs, win, geo, scene)
            payload["pages"][label] = shot(f"10-page-{scene}")
        goto_scene(xs, win, geo, "home")
        time.sleep(5)

        # 空闲 CPU（引擎健康 + 默认静态）
        groups = webkit_process_groups()
        groups["app_main"] = [app.proc.pid]
        payload["idle_cpu"] = sample_cpu(groups, args.idle_seconds, curve_every=5.0)
        payload["idle_shot"] = shot("30-idle")
        payload["dmabuf_env"] = {
            "app_main": proc_env(app.proc.pid).get("WEBKIT_DISABLE_DMABUF_RENDERER"),
        }
        payload["env"] = base_env_snapshot(args.display, args.dmabuf, {
            "build": "debug + vite dev（前端 dev server，非内嵌产物）",
            "data_root": str(root),
            "binary": str(APP_BIN),
            "window_size": "1280x820（tauri.conf 默认，未 resize）",
            "synthetic_endpoint": app.mock_base_url,
            "model_permissions": "一次性数据根内预置 local=true（合成端点）",
            "idle_seconds": args.idle_seconds,
            "note": "真实窗口；未截取用户桌面；未使用真实材料或凭证",
        })
        payload["verdict"] = "ok"
    except Exception as exc:
        payload["verdict"] = f"error:{type(exc).__name__}: {exc}"
    finally:
        try:
            app.stop_all()
        except Exception:
            pass
    return payload


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root-base", default="/tmp/od26-home")
    ap.add_argument("--dmabuf", default="1", choices=["0", "1"])
    ap.add_argument("--settle", type=float, default=12.0)
    ap.add_argument("--idle-seconds", type=float, default=60.0)
    ap.add_argument("--reply-timeout", type=float, default=90.0)
    ap.add_argument("--prompt", default="synthetic layout check: reply in one short sentence")
    ap.add_argument("--input-xy", default="430,676")
    ap.add_argument("--screenshot-only", action="store_true")
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])
    args.input_xy = tuple(int(v) for v in args.input_xy.split(","))
    SHOTS.mkdir(parents=True, exist_ok=True)

    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    with ctx:
        assert_no_webkit(f"{args.label} 开始前")
        p = run(args)
    write_json(EVID / f"{args.label}.json", p)
    ic = p.get("idle_cpu", {})
    print(f"[{args.label}] verdict={p['verdict']} "
          f"stability={p.get('stability', {}).get('distinct_signatures')} "
          f"reply={p.get('reply')} "
          f"web={ic.get('webkit_web_process', {}).get('avg_percent')}% "
          f"app={ic.get('app_main', {}).get('avg_percent')}%")
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
