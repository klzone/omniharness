#!/usr/bin/env python3
"""OD-ARCH-091 S4：周报分页的真实窗口预览（只截一帧，不做断言）。

用一次性 Xvfb + 隔离数据根，造一份合成的工作记录与往期周报，进入「工作记录」
场景后抓图。合成文本全部是虚构标记，不接触任何真实业务材料。

用法：python3 preview_weekly_091.py [--display :126] [--root /tmp/od091-preview]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-091"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError("Xvfb 未就绪")


def start_dbus() -> tuple[str, subprocess.Popen]:
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    return proc.stdout.readline().strip(), proc


def app_nodes(pid):
    from gi.repository import Atspi

    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        try:
            if app.get_process_id() != pid:
                continue
        except Exception:
            continue
        raw: list = []
        ax._collect(app, raw)
        return [ax._node(n) for n in raw]
    return []


def find(nodes, name):
    for node in nodes:
        if name in (node.name or "") or name in (node.text or ""):
            return node
    return None


def iso_week(day: dt.date) -> str:
    year, week, _ = day.isocalendar()
    return f"{year}-W{week:02d}"


def write_fixtures(root: Path) -> dict:
    today = dt.date.today()
    week = iso_week(today)
    monday = today - dt.timedelta(days=today.weekday())

    entries = [
        (0, "合成记录：把本周的三项进展整理成一句话，确认去向。", "本周报引用"),
        (1, "合成记录：核对周报模板的五个小节，缺的先用占位标注。", "本周报引用"),
        (3, "合成记录：与协作方确认交付时间，改到下周三。", "知识卡"),
        (3, "合成记录：把线上问题的复现步骤记下来，附两条日志。", "本周报引用"),
        (5, "合成记录：复盘本周两次返工，写清触发条件。", "归档"),
    ]
    by_day: dict[str, list] = {}
    for offset, text, dest in entries:
        day = monday + dt.timedelta(days=offset)
        by_day.setdefault(day.isoformat(), []).append((text, dest))

    lines = [f"# {week}", ""]
    for day in sorted(by_day):
        lines.append(f"## {day}")
        lines.append("")
        for text, dest in by_day[day]:
            lines.append(f"- **事**：{text}")
            lines.append(f"  **去向**：{dest}")
        lines.append("")

    worklog_dir = root / "工作记录"
    worklog_dir.mkdir(parents=True, exist_ok=True)
    (worklog_dir / f"{week}.md").write_text("\n".join(lines), encoding="utf-8")

    report_dir = root / "周报"
    report_dir.mkdir(parents=True, exist_ok=True)
    for back in (1, 2):
        day = monday - dt.timedelta(days=7 * back)
        label = iso_week(day)
        (report_dir / f"{label}-周报.md").write_text(
            f"# {label} 周报\n\n（合成样例，仅用于界面预览。）\n", encoding="utf-8"
        )
    return {"week": week, "root": str(root)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":126")
    ap.add_argument("--root", default="/tmp/od091-preview")
    ap.add_argument(
        "--reuse-dev-server",
        action="store_true",
        help="复用已在 5273 运行的同仓库 dev server（不启动也不终止它）",
    )
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)
    fixtures = write_fixtures(root / "local-data")

    shutil.copytree(
        SOURCE_HOME,
        root / "dsh-home",
        dirs_exist_ok=True,
        symlinks=True,
        ignore=shutil.ignore_patterns("sessions*", "session_*", "*ledger.json"),
    )
    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    if "AT_SPI_BUS_ADDRESS" not in os.environ and os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ["AT_SPI_BUS_ADDRESS"] = "unix:path=/run/user/1000/at-spi/bus"
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=args.reuse_dev_server,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            # 库根必须隔离：工作记录写库根，只设成果目录不够。
            "OMNIDESK_KNOWLEDGE_ROOT": str(root / "local-data"),
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
        time.sleep(8)
        ax.init()
        nodes = []
        dock = None
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and dock is None:
            nodes = app_nodes(pid)
            dock = find(nodes, "工作记录")
            if dock is None:
                time.sleep(2.0)
        (EVID / "00-home-ax.json").write_text(
            json.dumps(
                [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
                ensure_ascii=False, indent=2,
            ), encoding="utf-8",
        )
        if dock is None:
            raise RuntimeError("找不到「工作记录」入口")
        ax.invoke(dock)
        time.sleep(6.0)
        xs.shot(EVID / "01-weekly-timeline.png")
        xs.scroll(720, 700, clicks=10, settle=1.2)
        xs.shot(EVID / "02-weekly-history.png")
        (EVID / "01-weekly-fixtures.json").write_text(
            json.dumps(fixtures, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("saved", EVID / "01-weekly-timeline.png")
        return 0
    finally:
        session.close()
        try:
            xvfb.terminate()
            xvfb.wait(timeout=10)
        except Exception:
            pass
        try:
            bus_proc.terminate()
        except Exception:
            pass


if __name__ == "__main__":
    raise SystemExit(main())
