#!/usr/bin/env python3
"""OD-ARCH-093 S10：周报分页四项小改的真实窗口预览（一次性 Xvfb）。

只用 /tmp 下的合成数据：1 条合成待确认写入 + 3 段合成对话。不读真实知识库、
不打印凭证、不改动用户正在运行的实例。

抓帧：
  05 逾期卡内出现「待确认 1」入口（四卡下方不再有独立触发条）
  06 待确认弹窗（条目底色为方形）
  07 历史对话展开（删除按钮在每条标题左侧）
  08 分类下拉展开（浮层完整显示，不被列表裁切）

用法：python3 preview_weekly_093_s10.py [--display :206] [--root /tmp/od093-s10]
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

EVID = HERE / "evidence" / "od-arch-093"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 1500


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
    for n in nodes:
        hay = (n.name or "") + "\n" + (n.text or "")
        if (name in hay) if contains else (name == (n.name or "").strip()):
            return n
    return None


def node_pos(node) -> tuple[int, int, int, int] | None:
    from gi.repository import Atspi

    try:
        comp = node.accessible.get_component()
        ext = comp.get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return None
    if ext.width <= 0 or ext.height <= 0:
        return None
    return ext.x, ext.y, ext.width, ext.height


def iso_week(day: dt.date) -> str:
    year, week, _ = day.isocalendar()
    return f"{year}-W{week:02d}"


def write_worklog(root: Path) -> dict:
    today = dt.date.today()
    week = iso_week(today)
    monday = today - dt.timedelta(days=today.weekday())
    entries = [
        (-1, "合成记录：整理上周遗留的两项核对，补上结论与出处。", "本周报引用"),
        (0, "合成记录：今天要确定周报的五个小节标题。", "本周报引用"),
        (1, "合成记录：明天把初稿交给协作方看一眼。", "归档"),
    ]
    by_day: dict[str, list] = {}
    for offset, text, dest in entries:
        day = monday + dt.timedelta(days=max(0, min(6, today.weekday() + offset)))
        by_day.setdefault(day.isoformat(), []).append((text, dest))
    lines = [f"# {week}", ""]
    for day in sorted(by_day):
        lines.append(f"## {day}")
        lines.append("")
        for text, dest in by_day[day]:
            lines.append(f"- **事**：{text}")
            lines.append(f"  **去向**：{dest}")
            lines.append("")
    d = root / "工作记录"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"{week}.md").write_text("\n".join(lines), encoding="utf-8")
    return {"week": week, "entries": len(entries), "today": today.isoformat()}


def seed_pending(app_data: Path) -> int:
    pending_dir = app_data / "engine-state" / "pending-writes"
    pending_dir.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    record = {
        "schema": "omnidesk.pending-write.v1",
        "id": "preview093-s10-pending",
        "action": "save_offline",
        "source": "model",
        "summary": "把本周进度整理成周报初稿",
        "impact": "只在成果目录新增一份 Markdown，不覆盖既有文件。",
        "payload": {
            "scenario": "weekly_report",
            "title": "合成预览：待确认写入条目",
            "markdown": "# 合成预览\n\n这是一条只用于界面预览的合成记录。\n",
            "ai_assisted": True,
        },
        "created_at": now - 60,
        "expires_at": now + 3600,
        "status": "pending",
        "resolved_at": None,
    }
    (pending_dir / "preview093-s10-pending.json").write_text(
        json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return 1


def seed_threads(app_data: Path) -> int:
    now = time.time()
    d = app_data / "threads" / "_unassigned"
    d.mkdir(parents=True, exist_ok=True)
    titles = [
        "合成对话：周报结构与上周遗留",
        "合成对话：把复盘写成三段",
        "合成对话：归档与引用怎么分",
    ]
    for i, title in enumerate(titles):
        record = {
            "id": f"preview093-s10-t{i}",
            "tag": "weekly",
            "projectPath": None,
            "title": title,
            "preset": "weekly",
            "messages": [{"id": "m1", "role": "user", "text": "合成输入"}],
            "createdAt": now - 3600 + i,
            "updatedAt": now - 60 * i,
        }
        (d / f"preview093-s10-t{i}.json").write_text(
            json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8"
        )
    return len(titles)


def dump(nodes, name: str) -> None:
    (EVID / name).write_text(
        json.dumps(
            [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":206")
    ap.add_argument("--root", default="/tmp/od093-s10")
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
        # 本脚本自己起、自己收前端服务；不复用常驻实例，避免误连用户正在用的窗口。
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_KNOWLEDGE_ROOT": str(data_root),
        },
    )
    try:
        session.prepare()
        facts["pending_seeded"] = seed_pending(session.app_data)
        facts["threads_seeded"] = seed_threads(session.app_data)
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
            xs.shot(EVID / "s10-00-home.png")
            raise RuntimeError("找不到「工作记录」入口")

        # 09：首页输入区的分类下拉与周报页共用同一组件，必须确认 portal 改动没有
        # 把首页那处也弄坏（原来靠 bottom-full 向上弹，现在按可视空间自动上下翻转）。
        home_pickers = [
            n for n in nodes if (n.name or "").startswith("本次对话归到哪一类")
        ]
        facts["home_category_pickers"] = len(home_pickers)
        if home_pickers:
            home_before = {(n.role, (n.name or "").strip()) for n in nodes}
            ax.invoke(home_pickers[0])
            time.sleep(2.0)
            xs.shot(EVID / "09-home-category-menu.png")
            home_opened = app_nodes(pid)
            dump(home_opened, "09-home-category-ax.json")
            home_opts = [
                n
                for n in home_opened
                if n.role in ("option", "list item")
                and (n.name or "").strip()
                and (n.role, (n.name or "").strip()) not in home_before
            ]
            facts["home_category_option_count"] = len(home_opts)
            facts["home_category_options"] = [n.name for n in home_opts]
            home_positions = [node_pos(n) for n in home_opts]
            facts["home_category_all_visible"] = bool(home_positions) and all(
                p is not None
                and p[1] >= 0
                and p[1] + p[3] <= SCREEN_H
                and p[0] >= 0
                and p[0] + p[2] <= SCREEN_W
                for p in home_positions
            )
            xs.key("Escape")
            time.sleep(1.0)
        else:
            facts["home_category_option_count"] = 0
            facts["home_category_all_visible"] = False

        ax.invoke(dock)
        time.sleep(6.0)

        nodes = app_nodes(pid)
        dump(nodes, "05-overdue-pending-ax.json")
        xs.shot(EVID / "05-overdue-pending-card.png")
        # 逾期卡内的「待确认 N」按钮；旧版是四卡下方整条横栏。
        # 卡片容器（panel）的可访问名称里也含「待确认」，用包含匹配会命中容器本身、
        # 点击无效；WebKit 把这枚小按钮暴露成 combo box，因此只排除容器类角色。
        trigger = next(
            (
                n
                for n in nodes
                if n.role in ("button", "combo box", "push button")
                and "待确认" in (n.name or "")
            ),
            None,
        )
        facts["pending_trigger_present"] = trigger is not None
        facts["pending_trigger_role"] = trigger.role if trigger is not None else None
        facts["pending_trigger_pos"] = node_pos(trigger) if trigger is not None else None
        overdue = find(nodes, "逾期")
        facts["overdue_pos"] = node_pos(overdue) if overdue is not None else None

        # 06：待确认弹窗。
        if trigger is not None:
            ax.invoke(trigger)
            time.sleep(3.0)
            xs.shot(EVID / "06-pending-dialog.png")
            after = app_nodes(pid)
            dump(after, "06-pending-ax.json")
            facts["pending_dialog_open"] = any("确认写入" in (n.name or "") for n in after)
            # WebKit 不把弹窗里的条目正文单独暴露成 AX 节点（只有按钮可读），
            # 因此「有没有条目」以「每条记录自带确认/放弃按钮」为脚本判据；
            # 条目底色与中文场景名以 06-pending-dialog.png 截图为证。
            facts["pending_dialog_item_actions"] = sum(
                1 for n in after if (n.name or "").strip() == "确认写入"
            )
            facts["pending_dialog_reject_actions"] = sum(
                1 for n in after if (n.name or "").strip() == "放弃"
            )
            facts["pending_dialog_item_evidence"] = "button-pair + screenshot"
            xs.key("Escape")
            time.sleep(1.5)
        else:
            facts["pending_dialog_open"] = False

        # 07：展开历史对话。
        nodes = app_nodes(pid)
        toggle = find(nodes, "展开对话列表")
        facts["history_toggle_present"] = toggle is not None
        if toggle is not None:
            ax.invoke(toggle)
            time.sleep(2.5)
        xs.shot(EVID / "07-history-thread-rows.png")
        nodes = app_nodes(pid)
        dump(nodes, "07-history-ax.json")
        facts["history_titles"] = sum(1 for n in nodes if "合成对话：" in (n.name or ""))
        # 删除按钮的 aria-label 形如「删除对话：…」；它在标题左侧。
        dels = [n for n in nodes if (n.name or "").startswith("删除对话：")]
        facts["delete_buttons"] = len(dels)
        facts["delete_positions"] = [node_pos(n) for n in dels]
        title_nodes = [
            n
            for n in nodes
            if (n.name or "").startswith("合成对话：") and n.role == "button"
        ]
        facts["title_positions"] = [node_pos(n) for n in title_nodes]
        if dels and title_nodes:
            dpos = [p for p in facts["delete_positions"] if p]
            tpos = [p for p in facts["title_positions"] if p]
            facts["delete_left_of_title"] = bool(dpos and tpos and dpos[0][0] < tpos[0][0])
        else:
            facts["delete_left_of_title"] = False

        # 08：分类下拉。
        pickers = [
            n
            for n in nodes
            if (n.name or "").startswith("本次对话归到哪一类")
            or "scene-category-picker" in (n.name or "")
        ]
        facts["category_pickers"] = len(pickers)
        if pickers:
            before_open = {(n.role, (n.name or "").strip()) for n in nodes}
            ax.invoke(pickers[0])
            time.sleep(2.0)
            xs.shot(EVID / "08-category-menu.png")
            opened = app_nodes(pid)
            dump(opened, "08-category-ax.json")
            # WebKit 把浮层里的选项暴露成 list item（不是 option）：用「展开前后差集」
            # 取新增条目，既不依赖固定分类名，也能覆盖用户自定义分类。
            opts = [
                n
                for n in opened
                if n.role in ("option", "list item")
                and (n.name or "").strip()
                and (n.role, (n.name or "").strip()) not in before_open
            ]
            facts["category_option_count"] = len(opts)
            positions = [(n.name or "", node_pos(n)) for n in opts]
            facts["category_options"] = [
                {"name": name, "pos": pos} for name, pos in positions
            ]
            facts["category_options_all_visible"] = all(
                pos is not None
                and pos[1] >= 0
                and pos[1] + pos[3] <= SCREEN_H
                and pos[0] >= 0
                and pos[0] + pos[2] <= SCREEN_W
                for _, pos in positions
            )
            xs.key("Escape")
            time.sleep(1.0)
        else:
            facts["category_option_count"] = 0
            facts["category_options_all_visible"] = False

        (EVID / "facts-s10.json").write_text(
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
