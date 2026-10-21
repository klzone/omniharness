#!/usr/bin/env python3
"""OD-ARCH-053 S2 · 原生设置页真实窗口验收（四区截图）。

一次性数据根 + 一次性 Xvfb + 回环合成端点；进入 OmniDesk 原生设置页，
按四个分区（模型与端点 / 场景声明 / 权限 / 关于·高级）各抓一张截图。

边界：不碰 ~/.dsh、~/.agents、~/OmniDesk 用户数据；不读取凭证；
合成端点密钥为占位串；截图只含本地路径与合成值。

用法：python3 verify_native_settings_053.py [--display :97] [--root /tmp/od053-s2]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-053"


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root", default="/tmp/od053-s2")
    args = ap.parse_args()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    xvfb = start_xvfb(args.display)
    session = AppSession(root=Path(args.root), display=args.display)
    facts: dict = {}
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到 OmniDesk 主窗口")
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=2.5)
        g = x.geometry(win)
        facts["window"] = {"title": x.title(win), **{k: v for k, v in zip(("x", "y", "w", "h"), g.as_tuple())}}

        # 左侧 Dock 的「成果目录与模型权限」入口（settings 场景）
        x.click(g.x + 48, g.y + 588, settle=2.5)
        time.sleep(1.2)
        x.shot(EVIDENCE / "01-settings-top.png")

        # 逐段滚动，覆盖四区
        for idx, clicks in ((2, 4), (3, 8), (4, 14)):
            x.scroll(g.x + g.w // 2, g.y + g.h // 2, clicks, settle=1.0)
            x.shot(EVIDENCE / f"0{idx}-settings-scroll{clicks}.png")
        facts["shots"] = sorted(p.name for p in EVIDENCE.glob("*.png"))

        # 设置页内不应出现 DSH 主入口按钮（降级到折叠区后的文案核验由阅读确认）
        facts["app_alive"] = session.alive()
        (EVIDENCE / "facts.json").write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(facts, ensure_ascii=False, indent=2))
        return 0 if session.alive() else 1
    finally:
        session.close()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()


if __name__ == "__main__":
    raise SystemExit(main())
