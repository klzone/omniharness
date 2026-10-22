#!/usr/bin/env python3
"""OD-ARCH-059 S1 · 凭据状态收敛（未配置三态）真实窗口验收。

场景：一次性数据根、无任何模型配置（等价于全新安装）。进入原生设置页，
截图「凭据状态」一行，确认页面显示「尚未配置」而不是永久「读取中…」。

边界：一次性 Xvfb + 一次性数据根；不碰 ~/.dsh、~/.agents、~/OmniDesk；
不读取任何真实凭证；合成端点密钥为公开占位串。

用法：python3 verify_credential_state_059.py [--display :95] [--root /tmp/od059-s1]
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

EVIDENCE = HERE / "evidence" / "od-arch-059" / "S1"
SCREEN_W, SCREEN_H = 1440, 1700


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":95")
    ap.add_argument("--root", default="/tmp/od059-s1")
    args = ap.parse_args()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    xvfb = start_xvfb(args.display)
    session = AppSession(root=Path(args.root), display=args.display)
    facts: dict = {"evidence_dir": str(EVIDENCE), "data_root": args.root}
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到 OmniDesk 主窗口")
        x.focus(win)
        x.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        g = x.geometry(win)
        time.sleep(1.5)

        # 全新数据根：首屏引导卡上的「去设置」（与 054 相同入口，避免 Dock 高屏重排）
        x.click(g.x + 321, g.y + 174, settle=2.5)
        time.sleep(1.2)
        x.shot(EVIDENCE / "01-settings-credential-status.png")

        facts["window"] = {"title": x.title(win), "w": g.w, "h": g.h}
        facts["expected_status_text"] = "尚未配置"
        facts["app_alive"] = session.alive()
        (EVIDENCE / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
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
