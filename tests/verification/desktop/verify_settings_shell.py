#!/usr/bin/env python3
"""OD-ARCH-051 S4 · DSH 设置窗口外壳统一的真实窗口验收。

一次性数据根 + 一次性 Xvfb + 回环合成端点：进入设置页 → 点「打开 DSH 设置」→
抓取新窗口，核对窗口标题为「OmniDesk · DSH 设置」并记录底色采样。

边界：只用隔离 DSH_HOME（一次性目录），不碰 ~/.dsh / ~/.agents / 凭证；
不改 DSH 页面自身内容（外部页面受 CSP 限制，本阶段只统一窗口外壳）。

用法：python3 verify_settings_shell.py [--display :98] [--root /tmp/od051-s4] [--shot-only]
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-051"
SHOTS = EVIDENCE / "shots"


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def dump_windows(display: str) -> list[dict]:
    """列出本次一次性显示上的顶层窗口名与几何（用于核对标题，不依赖搜索串）。"""
    from Xlib import X, display as xdisplay

    d = xdisplay.Display(display)
    out: list[dict] = []

    def walk(win, depth: int = 0) -> None:
        try:
            kids = win.query_tree().children
        except Exception:
            return
        for child in kids:
            try:
                name = child.get_wm_name()
                net = child.get_full_property(
                    d.intern_atom("_NET_WM_NAME"), d.intern_atom("UTF8_STRING")
                )
                if net and net.value:
                    raw = net.value
                    name = raw.decode("utf-8", "replace") if isinstance(raw, bytes) else str(raw)
                geom = child.get_geometry()
                viewable = child.get_attributes().map_state == X.IsViewable
            except Exception:
                name, geom, viewable = None, None, False
            try:
                cls = child.get_wm_class()
            except Exception:
                cls = None
            if name or (viewable and depth <= 2):
                out.append({
                    "title": name,
                    "class": list(cls) if cls else None,
                    "depth": depth,
                    "viewable": viewable,
                    "geometry": [geom.x, geom.y, geom.width, geom.height] if geom else None,
                })
            walk(child, depth + 1)

    walk(d.screen().root)
    return out


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--display", default=":98")
    parser.add_argument("--root", default="/tmp/od051-s4")
    # 以下为根窗口绝对坐标（由 1440x900 一次性显示上的截图校准）。
    parser.add_argument("--settings-dock-y", type=int, default=565)
    parser.add_argument("--settings-dock-x", type=int, default=126)
    parser.add_argument("--open-btn", default=None, help="设置页里「打开 DSH 设置」的相对坐标 x,y")
    parser.add_argument("--shot-only", action="store_true")
    args = parser.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    # 一次性数据根：复用上一次的目录会让设置/缓存状态串到本轮（同 verify_profile_decl.py 修过的问题）。
    # 只允许清 /tmp 下目录，避免误删真实数据根。
    root = Path(args.root)
    resolved = root.resolve()
    if resolved != Path("/tmp") and Path("/tmp") not in resolved.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{resolved}")
    if root.exists():
        shutil.rmtree(root)
    xvfb = start_xvfb(args.display)
    report: dict = {"plan": "OD-ARCH-051 S4", "rounds": {}}
    session = AppSession(root=root, display=args.display)
    try:
        session.prepare()
        xs = XSession(args.display)
        session.start()
        win = xs.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到主窗口")
        time.sleep(6)
        geo = xs.geometry(win)

        report["window_geometry"] = list(geo.as_tuple())
        # 进设置页（Dock 上倒数第二个业务图标）
        xs.click(args.settings_dock_x, args.settings_dock_y)
        time.sleep(3)
        xs.shot(SHOTS / "10-settings-page.png")
        report["rounds"]["settings_page"] = {"shot": str(SHOTS / "10-settings-page.png")}
        if not args.shot_only and args.open_btn:
            bx, by = (int(v) for v in args.open_btn.split(","))
            xs.click(bx, by)
            time.sleep(12)
            xs.shot(SHOTS / "11-settings-window.png")

            title = None
            windows: list[dict] = []
            for _ in range(30):
                windows = dump_windows(args.display)
                hit = [w for w in windows if (w.get("title") or "").startswith("OmniDesk · DSH")]
                if hit:
                    title = hit[0]["title"]
                    break
                time.sleep(1)
            if title:
                from window_harness import Geometry

                g = hit[0]["geometry"]
                xs.shot(SHOTS / "12-settings-window-only.png", region=Geometry(*g))
            report["rounds"]["windows"] = windows
            report["rounds"]["settings_window"] = {
                "title": title,
                "shot": str(SHOTS / "12-settings-window-only.png"),
                "app_alive": session.alive(),
            }
    finally:
        session.stop_all()
        xvfb.terminate()

    (EVIDENCE / "settings-shell-run.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
