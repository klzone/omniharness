#!/usr/bin/env python3
"""OD-ARCH-098 S04：项目跟进页两栏改版（卡片列表 + 甘特）的真实窗口验证。

只用 /tmp 下的合成数据：一条合成项目对话，正文按宿主约定写 5 条五列事项
（只有期限 → 里程碑）与 2 条六列事项（有开始日期 → 区间条），其中 1 条已逾期。
不读真实知识库、不打印凭证、不改动用户正在运行的实例。

抓帧与断言：
  01 宽窗两栏（左卡片列表 + 右甘特）
  02 搜索过滤（只留命中的事项）
  03 卡片选中联动（甘特行高亮）
  04 阶段成果次级切换
  05 窄窗 1000 单列

用法：python3 preview_project_098.py [--display :131] [--root /tmp/od098-preview]
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

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-098"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 2200
NARROW_W, NARROW_H = 1000, 2200


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


def dump(nodes, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )


def iso(d: dt.date) -> str:
    return d.isoformat()


def project_lines(today: dt.date) -> tuple[str, list[dict]]:
    """7 条合成事项：5 条旧式五列（里程碑）+ 2 条六列（区间条），含 1 条逾期。"""
    d = lambda n: iso(today + dt.timedelta(days=n))
    rows = [
        ("合成：周报模板与字段说明", "P1", d(-3), "done", "已并入成果目录"),
        ("合成：知识卡索引重建", "P2", d(-9), "in_progress", "补两条失败样本"),
        ("合成：交接文档补全", "P1", d(2), "blocked", "等协作方确认口径"),
        ("合成：下周评审排期 Review", "P3", d(6), "not_started", "先定评审人与时长"),
        ("合成：离线导出对齐", "P2", d(11), "not_started", "列出导出字段"),
    ]
    ranges = [
        ("合成：三栏版式联调", "P1", d(3), d(-2), "in_progress", "收尾窄窗顺序"),
        ("合成：甘特刻度实现", "P3", d(9), d(1), "not_started", "先写纯函数"),
    ]
    # 宿主逐行解析：只接受五列/六列事项行，`#` 开头的行跳过。标题必须用注释行，
    # 否则第一行会被当成格式错误的事项（界面显示成「确定性汇总没有产出」）。
    lines = ["# 本周项目事项（合成）", ""]
    items = []
    for name, owner, due, status, nxt in rows:
        lines.append(f"{name} | {owner} | {due} | {status} | {nxt}")
        items.append({"item": name, "owner": owner, "due": due, "start": None, "status": status})
    for name, owner, due, start, status, nxt in ranges:
        lines.append(f"{name} | {owner} | {due} | {start} | {status} | {nxt}")
        items.append({"item": name, "owner": owner, "due": due, "start": start, "status": status})
    return "\n".join(lines), items


def write_threads(app_data: Path, text: str) -> dict:
    root = app_data / "threads" / "_unassigned"
    root.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    thread = {
        "id": "t098project01",
        "tag": "project",
        "projectPath": None,
        "title": "合成项目事项：两栏版式联调",
        "preset": "project",
        "messages": [
            {
                "id": "m098a",
                "role": "user",
                "text": text,
            }
        ],
        "createdAt": now - 3600,
        "updatedAt": now - 600,
        "version": 1,
    }
    (root / f"{thread['id']}.json").write_text(
        json.dumps(thread, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return thread


def write_artifacts(data_root: Path) -> int:
    data_root.mkdir(parents=True, exist_ok=True)
    today = dt.date.today()
    titles = ["合成阶段成果：两栏骨架", "合成阶段成果：甘特刻度备忘"]
    for i, title in enumerate(titles):
        day = today - dt.timedelta(days=i * 3)
        name = f"offline-project-management-{day.strftime('%Y%m%d')}-{80000 + i * 137:06d}.md"
        (data_root / name).write_text(
            "---\n"
            "scenario: project_management\n"
            f'title: "{title}"\n'
            f"created: {day.isoformat()}T09:00:00Z\n"
            "---\n"
            f"# {title}\n\n仅用于界面验证的合成阶段成果。\n",
            encoding="utf-8",
        )
    return len(titles)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":131")
    ap.add_argument("--root", default="/tmp/od098-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    data_root = root / "local-data"
    today = dt.date.today()
    text, items = project_lines(today)
    facts: dict = {
        "display": args.display,
        "root": str(root),
        "today": today.isoformat(),
        "items": len(items),
        "milestones": sum(1 for i in items if not i["start"]),
        "ranges": sum(1 for i in items if i["start"]),
    }
    facts["artifacts"] = write_artifacts(data_root)

    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    # 走私有 session bus 上自动拉起的 at-spi registry，避免读到宿主机桌面应用。
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_KNOWLEDGE_ROOT": str(data_root),
        },
    )
    try:
        session.prepare()
        write_threads(session.app_data, text)
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
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline and dock is None:
            nodes = app_nodes(pid)
            # 侧栏按钮的无障碍名带场景后缀（如「项目跟踪（模型起草）」），按包含匹配。
            dock = find(nodes, "项目跟踪", contains=True) or find(nodes, "项目跟进", contains=True)
            if dock is None:
                time.sleep(2.0)
        dump(nodes, EVID / "00-home-ax.json")
        if dock is None:
            xs.shot(EVID / "00-home.png")
            raise RuntimeError("找不到「项目跟踪」入口")
        ax.invoke(dock)
        time.sleep(7.0)

        xs.shot(EVID / "01-project-two-column.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "01-project-ax.json")
        facts["ax_projects_heading"] = any("项目事项" in (n.name or "") + (n.text or "") for n in nodes)
        facts["ax_timeline_heading"] = any("项目时间线" in (n.name or "") + (n.text or "") for n in nodes)
        # WebKit 把可选中的卡片暴露成 toggle button，不是 button。
        facts["ax_cards"] = sum(
            1
            for n in nodes
            if n.role in ("button", "toggle button") and "负责人" in (n.name or "")
        )
        facts["ax_today_line"] = any("今天" in (n.name or "") for n in nodes)
        facts["ax_milestone_rows"] = sum(1 for n in nodes if "里程碑" in (n.name or ""))

        # 02：搜索过滤
        box = next(
            (n for n in nodes if n.role in ("entry", "text") and "搜索" in (n.name or "")),
            None,
        ) or find(nodes, "搜索项目事项", contains=True)
        if box is not None:
            ax.invoke(box)
            time.sleep(0.6)
            # 合成窗口的键盘输入只走 keysym，中文无法直接敲入；用一条带 ASCII
            # 关键词的合成事项验证过滤行为，断言仍然核对完整中文标题。
            xs.type_ascii("Review")
            time.sleep(2.0)
            xs.shot(EVID / "02-project-search.png")
            after = app_nodes(pid)
            dump(after, EVID / "02-project-search-ax.json")
            facts["search_cards"] = sum(
                1
                for n in after
                if n.role in ("button", "toggle button") and "负责人" in (n.name or "")
            )
            facts["search_kept_match"] = any("评审排期" in (n.name or "") for n in after)
            xs.select_all_and_clear()
            time.sleep(1.6)
        else:
            facts["search_cards"] = None
            facts["search_kept_match"] = False

        # 03：卡片选中 → 甘特行联动（aria-pressed）
        nodes = app_nodes(pid)
        card = next(
            (n for n in nodes if n.role in ("button", "toggle button") and "负责人" in (n.name or "")),
            None,
        )
        if card is not None:
            ax.invoke(card)
            time.sleep(2.0)
            xs.shot(EVID / "03-project-linked.png")
            after = app_nodes(pid)
            dump(after, EVID / "03-project-linked-ax.json")
            pressed = 0
            for n in after:
                if n.role in ("button", "toggle button"):
                    try:
                        from gi.repository import Atspi

                        # aria-pressed 在 WebKit/ATK 里映射成 PRESSED（不是 CHECKED）。
                        states = n.accessible.get_state_set()
                        if states.contains(Atspi.StateType.PRESSED) or states.contains(
                            Atspi.StateType.CHECKED
                        ):
                            pressed += 1
                    except Exception:
                        pass
            facts["linked_selected"] = pressed >= 1
        else:
            facts["linked_selected"] = False

        # 04：阶段成果次级切换
        nodes = app_nodes(pid)
        art_btn = next(
            (n for n in nodes if n.role in ("button", "toggle button") and (n.name or "").strip() == "阶段成果"),
            None,
        )
        if art_btn is not None:
            ax.invoke(art_btn)
            time.sleep(2.0)
            xs.shot(EVID / "04-project-artifacts.png")
            after = app_nodes(pid)
            dump(after, EVID / "04-project-artifacts-ax.json")
            facts["artifacts_visible"] = any(
                "合成阶段成果" in (n.name or "") + (n.text or "") for n in after
            )
            timeline_btn = next(
                (n for n in after if n.role in ("button", "toggle button") and (n.name or "").strip() == "时间线"),
                None,
            )
            if timeline_btn is not None:
                ax.invoke(timeline_btn)
                time.sleep(1.5)
        else:
            facts["artifacts_visible"] = False

        # 05：窄窗单列（1000 < lg 1024）
        xs.resize(win, NARROW_W, NARROW_H, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "05-project-narrow.png")
        narrow = app_nodes(pid)
        dump(narrow, EVID / "05-project-narrow-ax.json")
        facts["narrow_width"] = NARROW_W
        facts["narrow_cards_and_gantt"] = any(
            "项目事项" in (n.name or "") for n in narrow
        ) and any("项目时间线" in (n.name or "") for n in narrow)

        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("saved", EVID)
        print(json.dumps(facts, ensure_ascii=False))
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
