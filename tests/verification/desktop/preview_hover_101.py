#!/usr/bin/env python3
"""OD-ARCH-101：全局悬停说明的真实窗口预览（一次性 Xvfb）。

只用 /tmp 下的合成数据：复用 OD-ARCH-100 的合成本周记录与合成成果，
不读真实知识库、不打印凭证、不改动用户正在运行的实例。

抓帧：
  01 工作记录页：时间线条目悬停（截断正文的完整说明）
  02 顶部导航：Profile 徽标悬停
  03 首页：禁用态发送按钮悬停（验证禁用控件也能提示）
  04 成果库标题悬停 + 视口左下角的翻转样例

用法：python3 preview_hover_101.py [--display :131] [--root /tmp/od101-preview]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, Geometry, XSession  # noqa: E402
import accessibility as ax  # noqa: E402
from preview_weekly_100 import (  # noqa: E402
    app_nodes,
    find,
    node_pos,
    start_dbus,
    start_display,
    write_artifacts,
    write_worklog,
)

EVID = HERE / "evidence" / "od-arch-101"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 2900


def tooltips(nodes, expected: str | None = None) -> list:
    """AT-SPI 里 WebKit 可能把 tooltip 暴露成 `tool tip` 等变体，按角色或文本一起认。"""
    hits = [n for n in nodes if (n.role or "").replace(" ", "").lower() == "tooltip"]
    if expected:
        hits += [n for n in nodes if expected in (n.name or "") and n not in hits]
    return hits


def settle_hover(xs: XSession, x: int, y: int, seconds: float = 1.0) -> None:
    """多步小幅移动后停住：覆盖真实指针的细微抖动，确认不会一直重置提示。"""
    for step in range(6):
        xs.move(x + step % 2, y - step % 3)
        time.sleep(0.09)
    xs.move(x, y)
    time.sleep(seconds)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":131")
    ap.add_argument("--root", default="/tmp/od101-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    data_root = root / "local-data"
    facts: dict = {"display": args.display, "root": str(root)}
    facts.update(write_worklog(data_root))
    facts["artifacts"] = write_artifacts(data_root)

    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_KNOWLEDGE_ROOT": str(data_root),
        },
    )
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=150)
        if win is None:
            raise RuntimeError("主窗口未出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        if not xs.wait_rendered(win, timeout=120):
            raise RuntimeError("界面未渲染完成")
        ax.init()

        # --- 首页：禁用态发送按钮（空输入时 disabled） ---
        nodes: list = []
        send = None
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and send is None:
            nodes = app_nodes(pid)
            send = find(nodes, "发送指令", contains=False) or find(nodes, "发送指令")
            if send is None:
                time.sleep(2.0)
        if send is not None:
            p = node_pos(send)
            facts["send_geometry"] = list(p) if p else None
            if p is not None:
                settle_hover(xs, p[0] + p[2] // 2, p[1] + p[3] // 2, 1.2)
                tips = tooltips(app_nodes(pid), "当前权限下不发送输入")
                facts["disabled_hint_visible"] = bool(tips)
                facts["disabled_hint_text"] = tips[0].name if tips else None
                left = max(0, p[0] - 420)
                xs.shot(
                    EVID / "01-disabled-send-hover.png",
                    Geometry(left, max(0, p[1] - 90), min(560, SCREEN_W - left), 220),
                )
                xs.move(20, 20)
                time.sleep(0.5)

        # --- 顶部导航 Profile 徽标 ---
        facts["home_buttons"] = [f"{n.role}:{n.name}" for n in nodes if "button" in (n.role or "")][:40]
        badge = find(nodes, "引擎 Profile", contains=False) or find(nodes, "引擎 Profile")
        if badge is None:
            badge = next(
                (n for n in nodes if (n.name or "").startswith(("专业", "纪检", "个人", "标准"))),
                None,
            )
        p = node_pos(badge) if badge is not None else None
        if p is not None:
            settle_hover(xs, p[0] + p[2] // 2, p[1] + p[3] // 2, 1.1)
            tips = tooltips(app_nodes(pid), "引擎 Profile")
            facts["profile_hint_visible"] = bool(tips)
            facts["profile_hint_text"] = tips[0].name if tips else None
            xs.shot(
                EVID / "02-topnav-profile-hover.png",
                Geometry(max(0, p[0] - 260), 0, min(720, SCREEN_W), 150),
            )
            xs.move(20, 20)
            time.sleep(0.5)

        # --- 工作记录页：时间线条目（正文被截断，悬停看全文） ---
        dock = find(app_nodes(pid), "工作记录")
        if dock is None:
            raise RuntimeError("找不到「工作记录」入口")
        ax.invoke(dock)
        time.sleep(6.0)
        nodes = app_nodes(pid)
        entry = next(
            (n for n in nodes if n.role == "paragraph" and "合成记录" in (n.name or n.text or "")),
            None,
        )
        if entry is None:
            entry = next((n for n in nodes if "合成记录" in (n.name or "")), None)
        p = node_pos(entry) if entry is not None else None
        if p is not None:
            settle_hover(xs, p[0] + min(p[2] // 2, 120), p[1] + p[3] // 2, 1.2)
            tips = tooltips(app_nodes(pid), "整理上周遗留")
            facts["entry_hint_visible"] = bool(tips)
            facts["entry_hint_text"] = tips[0].name if tips else None
            left = max(0, p[0] - 120)
            xs.shot(
                EVID / "03-timeline-entry-hover.png",
                Geometry(left, max(0, p[1] - 140), min(820, SCREEN_W - left), 340),
            )

        # --- 成果库标题：悬停显示完整标题（不显示文件名） ---
        nodes = app_nodes(pid)
        card = next((n for n in nodes if "合成成果" in (n.name or "")), None)
        p = node_pos(card) if card is not None else None
        if p is not None:
            settle_hover(xs, p[0] + p[2] // 2, p[1] + p[3] // 2, 1.2)
            tips = tooltips(app_nodes(pid), "成果")
            facts["artifact_hint_visible"] = bool(tips)
            facts["artifact_hint_text"] = tips[0].name if tips else None
            xs.shot(
                EVID / "04-artifact-hover.png",
                Geometry(max(0, p[0] - 300), max(0, p[1] - 140), min(760, SCREEN_W), 340),
            )

        facts["no_native_title_left"] = all(
            "title" not in (n.role or "") for n in nodes
        )
        (EVID / "facts.json").write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
        print("saved", EVID)
        print(json.dumps(facts, ensure_ascii=False))
        return 0
    finally:
        session.close()
        try:
            xvfb.terminate()
        except Exception:
            pass
        try:
            bus_proc.terminate()
        except Exception:
            pass


if __name__ == "__main__":
    raise SystemExit(main())
