#!/usr/bin/env python3
"""OD-ARCH-099：周报页四项微调的真实窗口预览（一次性 Xvfb）。

只用 /tmp 下的合成数据：合成本周工作记录（含不同去向与一条无去向）、合成往期周报、
合成成果 Markdown。不读真实知识库、不打印凭证、不改动用户正在运行的实例。

抓帧：
  01 宽窗：四张指标卡的图标独占左列、右侧文字与数字、成果卡 48px 序号块
  02 指标卡图标列特写（裁切）
  03 时间线方形去向开关（点选前）
  04 点选后统计与开关同步（点选后）
  05 成果库文件名 → 居中预览浮层
  06 窄窗 1000：指标卡两列、成果卡序号块

用法：python3 preview_weekly_099.py [--display :129] [--root /tmp/od099-preview]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, Geometry, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-099"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 2900
NARROW_W, NARROW_H = 1000, 2740


def start_display(display: str, w: int = ROOT_W, h: int = ROOT_H) -> subprocess.Popen:
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


def find(nodes, name, contains=True):
    for node in nodes:
        a, b = node.name or "", node.text or ""
        if (name in a or name in b) if contains else (name == a or name == b):
            return node
    return None


def node_pos(node) -> tuple[int, int, int, int] | None:
    from gi.repository import Atspi

    try:
        ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return None
    if ext.width <= 0 or ext.height <= 0:
        return None
    return (ext.x, ext.y, ext.width, ext.height)


def iso_week(day: dt.date) -> str:
    year, week, _ = day.isocalendar()
    return f"{year}-W{week:02d}"


def write_worklog(root: Path) -> dict:
    today = dt.date.today()
    week = iso_week(today)
    monday = today - dt.timedelta(days=today.weekday())
    entries = [
        (-2, "合成记录：整理上周遗留的两项核对，补上结论与出处，这一条故意写得比较长，用来确认中栏卡片在窄一些的列宽下仍然只占两行而不是把整页拉长。", "本周报引用"),
        (-1, "合成记录：把本周进度写成三句话，确认哪几条要被周报引用。", "本周报引用"),
        (-1, "合成记录：把复现步骤记进知识卡，附两条日志摘要。", "知识卡"),
        (0, "合成记录：今天要确定周报的五个小节标题。", "本周报引用"),
        (0, "合成记录：把待归档的旧记录收进归档，不再逐条翻。", "归档"),
        (1, "合成记录：明天把初稿交给协作方看一眼。", "本周报引用"),
        (2, "合成记录：这条没有写去向，用来确认四象限会把它单列为待归类。"),
    ]
    by_day: dict[str, list] = {}
    for offset, text, *rest in entries:
        day = monday + dt.timedelta(days=max(0, min(6, today.weekday() + offset)))
        by_day.setdefault(day.isoformat(), []).append((text, rest[0] if rest else None))

    lines = [f"# {week}", ""]
    for day in sorted(by_day):
        lines.append(f"## {day}")
        lines.append("")
        for text, dest in by_day[day]:
            lines.append(f"- **事**：{text}")
            if dest:
                lines.append(f"  **去向**：{dest}")
            lines.append("")

    worklog_dir = root / "工作记录"
    worklog_dir.mkdir(parents=True, exist_ok=True)
    worklog_file = worklog_dir / f"{week}.md"
    worklog_file.write_text("\n".join(lines), encoding="utf-8")

    report_dir = root / "周报"
    report_dir.mkdir(parents=True, exist_ok=True)
    for back in (1, 2, 3):
        label = iso_week(monday - dt.timedelta(days=7 * back))
        (report_dir / f"{label}-周报.md").write_text(
            f"# {label} 周报\n\n（合成样例，仅用于界面预览。）\n", encoding="utf-8"
        )
    return {"week": week, "entries": len(entries), "today": today.isoformat(), "worklog": str(worklog_file)}


def write_artifacts(data_root: Path) -> int:
    data_root.mkdir(parents=True, exist_ok=True)
    today = dt.date.today()
    stems = [
        ("合成成果：本周周报初稿（五个小节）", 0),
        ("合成成果：交付节奏复盘与下周安排", 2),
        ("合成成果：模板字段说明", 9),
    ]
    for i, (title, back) in enumerate(stems):
        day = today - dt.timedelta(days=back)
        name = f"offline-weekly-report-{day.strftime('%Y%m%d')}-{100000 + i * 137:06d}.md"
        body = (
            "---\n"
            "scenario: weekly_report\n"
            f'title: "{title}"\n'
            f"created: {day.isoformat()}T09:00:00Z\n"
            "tags: [omnidesk, weekly_report]\n"
            "omnidesk_version: preview\n"
            "---\n"
            f"# {title}\n\n"
            "- 场景：weekly_report\n"
            "- 数据边界：仅合成数据\n\n"
            + "这是一份只用于界面预览的合成成果正文。\n" * (i + 3)
        )
        (data_root / name).write_text(body, encoding="utf-8")
    return len(stems)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":129")
    ap.add_argument("--root", default="/tmp/od099-preview")
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
    original = Path(facts["worklog"]).read_text(encoding="utf-8")

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
        nodes = []
        dock = None
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and dock is None:
            nodes = app_nodes(pid)
            dock = find(nodes, "工作记录")
            if dock is None:
                time.sleep(2.0)
        if dock is None:
            xs.shot(EVID / "00-home.png")
            raise RuntimeError("找不到「工作记录」入口")
        ax.invoke(dock)
        time.sleep(6.0)

        # 01：宽窗全景。
        xs.shot(EVID / "01-weekly-wide.png", Geometry(0, 0, SCREEN_W, SCREEN_H))
        nodes = app_nodes(pid)
        (EVID / "01-weekly-ax.json").write_text(
            json.dumps([{"role": n.role, "name": n.name, "text": n.text} for n in nodes], ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        texts = "\n".join((n.name or "") + (n.text or "") for n in nodes)
        facts["band_cards_present"] = all(t in texts for t in ("本期工作记录", "进周报条目", "本周周报", "逾期"))

        # 02：指标卡特写。按第一张卡的文字找容器几何，再裁掉纵向下方。
        first = find(nodes, "本期工作记录")
        pos = node_pos(first) if first is not None else None
        if pos is not None:
            x, y, w, h = pos
            crop_w = min(SCREEN_W - max(0, x - 40), w + 200)
            crop = (max(0, x - 40), max(0, y - 24), crop_w, 150)
            facts["band_crop"] = list(crop)
            try:
                xs.shot(EVID / "02-band-icon-column.png", Geometry(*crop))
            except Exception as e:  # 裁切只是证据增强，失败不阻断整体
                facts["band_crop_error"] = str(e)

        # 03：时间线方形去向开关（点选前）。
        switches = [n for n in nodes if n.role in ("switch", "check box") or (n.name or "").startswith(("已进周报：", "不进周报："))]
        facts["switch_nodes"] = len(switches)
        xs.shot(EVID / "03-destination-switch-before.png", Geometry(0, 0, SCREEN_W, SCREEN_H))
        before_entry = next((n for n in switches if "本周进度写成三句话" in (n.name or "")), None)
        if before_entry is None:
            before_entry = switches[0] if switches else None
        if before_entry is not None:
            p = node_pos(before_entry)
            if p is not None:
                try:
                    xs.shot(
                        EVID / "03b-switch-detail.png",
                        Geometry(max(0, p[0] - 320), max(0, p[1] - 40),
                                 min(760, SCREEN_W - max(0, p[0] - 320)), 140),
                    )
                except Exception:
                    pass
            ax.invoke(before_entry)
            time.sleep(2.5)
            after_nodes = app_nodes(pid)
            xs.shot(EVID / "04-destination-switch-after.png", Geometry(0, 0, SCREEN_W, SCREEN_H))
            after_switches = [
                n for n in after_nodes
                if n.role in ("switch", "check box") or (n.name or "").startswith(("已进周报：", "不进周报："))
            ]
            facts["switch_toggled"] = len(after_switches) == len(switches)
            (EVID / "04-after-ax.json").write_text(
                json.dumps([{"role": n.role, "name": n.name, "text": n.text} for n in after_nodes], ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
        else:
            facts["switch_toggled"] = False

        # 05：点成果卡文件名 → 居中预览浮层。
        nodes = app_nodes(pid)
        artifact = find(nodes, "合成成果：本周周报初稿")
        if artifact is not None:
            ap_pos = node_pos(artifact)
            facts["artifact_card_pos"] = list(ap_pos) if ap_pos else None
            ax.invoke(artifact)
            time.sleep(2.5)
            xs.shot(EVID / "05-artifact-sheet-centered.png", Geometry(0, 0, SCREEN_W, SCREEN_H))
            sheet_nodes = app_nodes(pid)
            dialog = next((n for n in sheet_nodes if n.role == "dialog"), None)
            dpos = node_pos(dialog) if dialog is not None else None
            facts["sheet_centered_geom"] = list(dpos) if dpos else None
            if dpos:
                facts["sheet_horizontal_gap"] = [dpos[0], SCREEN_W - (dpos[0] + dpos[2])]
                facts["sheet_vertical_gap"] = [dpos[1], SCREEN_H - (dpos[1] + dpos[3])]
                facts["sheet_centered"] = abs(facts["sheet_horizontal_gap"][0] - facts["sheet_horizontal_gap"][1]) <= 40 \
                    and abs(facts["sheet_vertical_gap"][0] - facts["sheet_vertical_gap"][1]) <= 60
            close_btn = next(
                (n for n in sheet_nodes if (n.name or "").strip() in ("关闭预览", "关闭")), None
            )
            facts["sheet_has_close"] = close_btn is not None
            if close_btn is not None:
                ax.invoke(close_btn)
                time.sleep(1.5)
        else:
            facts["sheet_centered"] = False
            facts["artifact_found"] = False

        # 06：窄窗 1000。
        xs.resize(win, NARROW_W, NARROW_H, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "06-narrow-1000.png", Geometry(0, 0, NARROW_W, NARROW_H))
        narrow_nodes = app_nodes(pid)
        narrow_text = "\n".join((n.name or "") + (n.text or "") for n in narrow_nodes)
        facts["narrow_band_present"] = "本期工作记录" in narrow_text

        final = Path(facts["worklog"]).read_text(encoding="utf-8")
        changed = [i for i, (a, b) in enumerate(zip(original.splitlines(), final.splitlines())) if a != b]
        facts["worklog_line_count_same"] = len(original.splitlines()) == len(final.splitlines())
        facts["worklog_changed_lines"] = changed
        facts["worklog_changed_text"] = [final.splitlines()[i] for i in changed]
        facts["worklog_backups"] = [p.name for p in Path(facts["worklog"]).parent.glob("*.md.bak-*")]
        status = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=str(HERE.parents[2]),
            capture_output=True,
            text=True,
        ).stdout
        facts["runtime_touched"] = any(line.split(maxsplit=1)[-1].startswith("runtime/") for line in status.splitlines() if line.strip())

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
