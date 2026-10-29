#!/usr/bin/env python3
"""OD-ARCH-106 真实窗口 smoke：基础标准只有平台固有入口，专业 Profile 才有业务场景。

一次性数据根 + 合成端点 + 一次性 Xvfb，两轮对照：

  1. 基础标准（OMNIDESK_DSH_PROFILE=omnidesk-base，且数据根下没有任何 decl.yml）：
     Dock 只有 3 项（首页 / 历史会话 / 知识库），宿主不会写默认声明；
  2. 同一数据根切到 discipline 并预置声明：Dock 出现 6 项（5 个声明入口 + 知识库），
     证明场景是「随 Profile 挂载」而不是平台固定。

入口数量由截图里 Dock 图标带自动统计（与 055 同一套量法），不依赖 DOM。
证据等级：真实桌面窗口运行（Xvfb + 合成端点）+ 文件事实。

用法：python3 verify_profile_scope_106_window.py [--display :96] [--root /tmp/od106-scope]
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
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import detect_dock_centers, mount_window  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-106"
SHOTS = EVIDENCE / "shots"
DEFAULT_DECL = (REPO / "samples" / "profiles" / "discipline.yml").read_text(encoding="utf-8")


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def count_dock(session: AppSession, xs: XSession, shot: Path) -> dict:
    win = mount_window(session, xs)
    time.sleep(5.0)
    xs.shot(shot)
    # 必须用**窗口**几何（xs.shot 返回的是整屏帧几何）：窗口在整屏上不一定从 (0,0) 开始，
    # 用整屏几何会把扫描列落到桌面黑边里，量到 0 个图标带。
    geo = xs.geometry(win)
    # 先量一个足够大的上限，再按实际带数判断；带数不符就抛错，避免静默通过。
    from PIL import Image

    px = Image.open(shot).convert("RGB").load()
    height = Image.open(shot).size[1]
    bands = 0
    start = None
    for y in range(geo.y + 1, min(height, geo.y + geo.h - 1)):
        dark = 0
        for x in range(geo.x + 47 - 14, geo.x + 47 + 15):
            r, g, b = px[x, y]
            if r < 200 and b < 235:
                dark += 1
        if dark >= 3 and start is None:
            start = y
        elif dark < 3 and start is not None:
            if y - start >= 6:
                bands += 1
            start = None
    return {"shot": str(shot), "dock_bands": bands, "app_alive": session.alive()}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":96")
    ap.add_argument("--root", default="/tmp/od106-scope")
    args = ap.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)

    xvfb = start_xvfb(args.display)
    report: dict = {
        "plan": "OD-ARCH-106",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实",
        "root": str(root),
        "rounds": {},
        "checks": {},
    }
    try:
        # --- 1. 基础标准 -------------------------------------------------------
        std = AppSession(
            root=root,
            display=args.display,
            env_extra={"OMNIDESK_DSH_PROFILE": "omnidesk-base"},
        )
        try:
            std.prepare()
            xs = XSession(args.display)
            std.start()
            report["rounds"]["standard"] = count_dock(std, xs, SHOTS / "01-standard.png")
            std_decl = root / "app-data" / "profiles" / "omnidesk-base" / "decl.yml"
            report["rounds"]["standard"]["decl_path"] = str(std_decl)
            report["rounds"]["standard"]["decl_absent"] = not std_decl.exists()
            report["checks"]["standard_dock_is_three"] = report["rounds"]["standard"]["dock_bands"] == 3
            report["checks"]["standard_no_decl_created"] = not std_decl.exists()
        finally:
            std.stop_all()
        time.sleep(2.0)

        # --- 2. 同一数据根切 discipline + 预置声明 -----------------------------
        decl = root / "app-data" / "profiles" / "discipline" / "decl.yml"
        decl.parent.mkdir(parents=True, exist_ok=True)
        decl.write_text(DEFAULT_DECL, encoding="utf-8")
        prof = AppSession(
            root=root,
            display=args.display,
            env_extra={"OMNIDESK_DSH_PROFILE": "discipline"},
        )
        try:
            prof.prepare()
            xs = XSession(args.display)
            prof.start()
            report["rounds"]["discipline"] = count_dock(prof, xs, SHOTS / "02-discipline.png")
            report["checks"]["discipline_dock_is_six"] = report["rounds"]["discipline"]["dock_bands"] == 6
            report["checks"]["discipline_decl_untouched"] = decl.read_text(encoding="utf-8") == DEFAULT_DECL
        finally:
            prof.stop_all()
    finally:
        xvfb.terminate()

    failed = [k for k, v in report["checks"].items() if v is False]
    report["result"] = "pass" if not failed else "fail"
    (EVIDENCE / "profile-scope-window.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps({**report["rounds"], "checks": report["checks"], "result": report["result"]}, ensure_ascii=False, indent=2))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
