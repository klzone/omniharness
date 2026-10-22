#!/usr/bin/env python3
"""OD-ARCH-059 S3 · Obsidian vault 兼容的真实窗口验收（设置页指引与边界）。

一次性数据根 + 一次性 Xvfb + 合成端点；只用合成数据，不读取任何凭证或真实材料。

核对三件事：
  1. 设置页「成果目录」区可见当前生效目录与来源；
  2. 同区出现「作为 Obsidian vault」接入指引与「边界」声明（只写自己的成果、
     不读不改 vault 中其他笔记、不安装/检测/启动 Obsidian）；
  3. 成果档案页有同一口径的一句话接入提示（截图取自「工作记录」场景页）。

frontmatter 与改绑行为由 Rust 单测覆盖（见 task_shell.rs），本脚本只负责界面证据。

用法：python3 verify_vault_binding_059.py --display :97 --root /tmp/od059-s3-verify [--calibrate]
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

EVIDENCE = HERE / "evidence" / "od-arch-059" / "S3"
SCREEN_W, SCREEN_H = 1440, 3600


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def warm_gdk(display: str) -> None:
    """先显式打开 GDK 显示，避免首次抓图时默认屏尚未就绪（harness 抓到 None 屏）。"""
    import gi

    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk

    if Gdk.Display.get_default() is None:
        Gdk.Display.open(display)


def click(xs: XSession, win, dx: int, dy: int, settle: float = 1.2) -> None:
    g = xs.geometry(win)
    xs.focus(win)
    point = (g.x + dx, g.y + dy)
    xs.move(*point)
    time.sleep(0.3)
    xs.click(*point, settle=0.4)
    xs.click(*point, settle=settle)


def click_once(xs: XSession, win, dx: int, dy: int, settle: float = 1.2) -> None:
    """单击一次（用于折叠标题这类开关：连点两下会开了又关）。"""
    g = xs.geometry(win)
    xs.focus(win)
    point = (g.x + dx, g.y + dy)
    xs.move(*point)
    time.sleep(0.3)
    xs.click(*point, settle=settle)


def scroll_page(xs: XSession, win, steps: int, settle: float = 1.0) -> None:
    """整页滚动：点页面左侧空白处让焦点离开输入框，再用 Page_Down 翻页。

    校准记录：内容列中间是 YAML textarea，滚轮会被它吃掉；边缘空白处接收不到滚轮。
    键盘翻页对整页滚动容器稳定生效。
    """
    g = xs.geometry(win)
    xs.focus(win)
    xs.click(g.x + 170, g.y + 860, settle=0.4)
    for _ in range(steps):
        xs.key("Page_Down", settle=0.12)
    time.sleep(settle)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root", default="/tmp/od059-s3-verify")
    ap.add_argument("--calibrate", action="store_true", help="滚动逐段截图，用于校准坐标")
    ap.add_argument("--scroll", type=int, default=0, help="进入设置页后向下翻页的次数（高屏默认不需要）")
    ap.add_argument("--summary-x", type=int, default=264, help="「成果目录」折叠标题的相对 x")
    ap.add_argument("--summary-y", type=int, default=2780, help="「成果目录」折叠标题的相对 y")
    ap.add_argument("--dock-x", type=int, default=47, help="Dock 图标列的相对 x")
    ap.add_argument("--dock-y", type=int, default=1682, help="Dock 第二项（工作记录）的相对 y")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    EVIDENCE.mkdir(parents=True, exist_ok=True)

    xvfb = start_xvfb(args.display)
    # artifact_override：把可见的成果目录也指向一次性目录，避免读档列表里出现用户真实成果文件名。
    session = AppSession(root=root, display=args.display, artifact_override=True)
    facts: dict = {
        "plan": "OD-ARCH-059 S3",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）",
        "root": str(root),
        "steps": {},
    }
    try:
        session.prepare()
        xs = XSession(args.display)
        warm_gdk(args.display)
        session.start()
        win = xs.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(2.5)

        if args.calibrate:
            xs.shot(EVIDENCE / "cal-00-home.png")
            click(xs, win, 321, 174, settle=2.5)
            xs.shot(EVIDENCE / "cal-01-settings-top.png")
            return 0

        # 1) 首屏 → 设置页（首次运行引导卡上的「去设置」）
        xs.shot(EVIDENCE / "01-home-first-run.png")
        click(xs, win, 321, 174, settle=2.5)
        xs.shot(EVIDENCE / "02-settings-top.png")
        facts["steps"]["settings_open"] = {
            "screenshot": "02-settings-top.png",
            "expect": "进入 OmniDesk 原生设置页（非 DSH 完整应用）",
        }

        # 2) 滚到「成果目录」折叠区并展开
        if args.scroll:
            scroll_page(xs, win, args.scroll, settle=1.2)
        xs.shot(EVIDENCE / "03-settings-artifacts-section.png")
        click_once(xs, win, args.summary_x, args.summary_y, settle=1.5)
        time.sleep(1.2)
        xs.shot(EVIDENCE / "04-vault-guidance.png")
        facts["steps"]["vault_guidance"] = {
            "screenshot": "04-vault-guidance.png",
            "expect": "当前生效目录 / 来源 / 作为 Obsidian vault 指引 / 边界声明",
        }

        # 3) 成果档案页的接入提示
        click(xs, win, args.dock_x, args.dock_y, settle=2.5)  # Dock 第二项：工作记录
        xs.shot(EVIDENCE / "06-scene-artifacts-hint.png")
        facts["steps"]["scene_hint"] = {
            "screenshot": "06-scene-artifacts-hint.png",
            "expect": "成果档案栏含 Obsidian 接入提示，且不出现对话框",
        }
        facts["app_alive"] = session.alive()
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    (EVIDENCE / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0 if facts.get("app_alive") else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
