#!/usr/bin/env python3
"""OD-ARCH-092 S6：两栏版式的真实窗口预览（抓三帧，不做断言）。

用一次性 Xvfb + 隔离数据根（OMNIDESK_KNOWLEDGE_ROOT 指到 /tmp），造一份合成本周
工作记录与两份往期周报，进入「工作记录」场景后抓：
  01 默认两栏（左时间线 / 右成果库）
  02 展开待确认写入（合成 1 条 pending）
  03 1280×820 窄窗（检查无横向溢出）

合成文本全部是虚构标记，不接触任何真实业务材料；脚本结束清理自己的 Xvfb / dbus /
dev server，不动用户正在运行的实例。

用法：python3 preview_weekly_092.py [--display :127] [--root /tmp/od092-preview]
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

EVID = HERE / "evidence" / "od-arch-092"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900


def start_display(display: str, w: int = SCREEN_W, h: int = SCREEN_H) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{w}x{h}x24", "-nolisten", "tcp"],
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

    # 5 条合成记录，跨 3 天：指标带「本期工作记录」应显示 5 条。
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
    return {"week": week, "entries": len(entries), "root": str(root)}


def seed_pending(app_data: Path) -> int:
    """造 1 条合成待确认写入，用来预览「有记录才出现」的窄条。"""
    pending_dir = app_data / "engine-state" / "pending-writes"
    pending_dir.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    record = {
        "schema": "omnidesk.pending-write.v1",
        "id": "preview092-pending",
        "action": "worklog_append",
        "status": "pending",
        "created_at": now - 60,
        "expires_at": now + 3600,
        "source": "model",
        "scenario": "weekly_report",
        "payload": {
            "scenario": "weekly_report",
            "title": "合成预览：待确认写入条目",
            "body": "这是一条只用于界面预览的合成记录。",
            "file_name": "preview092-synthetic.md",
        },
    }
    (pending_dir / "preview092-pending.json").write_text(
        json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":127")
    ap.add_argument("--root", default="/tmp/od092-preview")
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
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            # 库根必须隔离：工作记录写库根，只设成果目录不够。
            "OMNIDESK_KNOWLEDGE_ROOT": str(root / "local-data"),
        },
    )
    try:
        session.prepare()
        fixtures["pending_seeded"] = seed_pending(session.app_data)
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
            ),
            encoding="utf-8",
        )
        if dock is None:
            raise RuntimeError("找不到「工作记录」入口")
        ax.invoke(dock)
        time.sleep(6.0)

        # 01：默认两栏。
        xs.shot(EVID / "01-weekly-two-column.png")

        # 02：展开待确认写入（指标带出现该项并点开）。
        nodes = app_nodes(pid)
        awaiting = find(nodes, "待确认写入")
        if awaiting is not None:
            ax.invoke(awaiting)
            time.sleep(2.0)
            xs.shot(EVID / "02-pending-expanded.png")
            fixtures["pending_expanded"] = True
        else:
            fixtures["pending_expanded"] = False

        # 03：1280×820 窄窗，检查两栏按 lg 断点堆叠 / 不溢出。
        xs.resize(win, 1280, 820, settle=2.5)
        time.sleep(2.0)
        xs.shot(EVID / "03-narrow-1280.png")

        (EVID / "01-weekly-fixtures.json").write_text(
            json.dumps(fixtures, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("saved", EVID / "01-weekly-two-column.png")
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

