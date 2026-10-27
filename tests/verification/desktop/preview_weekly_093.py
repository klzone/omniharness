#!/usr/bin/env python3
"""OD-ARCH-093 S05：工作记录页三栏版式的真实窗口预览（一次性 Xvfb）。

只用 /tmp 下的合成数据：合成本周工作记录（含不同去向与一条无去向）、合成往期周报、
合成成果 Markdown。不读真实知识库、不打印凭证、不改动用户正在运行的实例。

抓帧：
  01 宽窗三栏（四栏指标带 + 左四象限/对话 + 中时间线 + 右成果卡，归档并入成果库）
  02 四象限点击筛选联动中栏
  03 窄窗 1000 单列（四象限 → 本周记录 → 成果库 → 历史对话）
  04 成果库列表视图（筛选保留）

用法：python3 preview_weekly_093.py [--display :128] [--root /tmp/od093-preview]
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

EVID = HERE / "evidence" / "od-arch-093"
SCREEN_W, SCREEN_H = 1600, 980
# 虚拟屏要高过窄窗整页：单列页面约 2400px，窗口不够高就只能截到上半段。
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
    """元素屏幕外接矩形（x, y, w, h）；取不到返回 None。"""
    from gi.repository import Atspi

    try:
        ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return None
    if ext.width <= 0 or ext.height <= 0:
        return None
    return (ext.x, ext.y, ext.width, ext.height)


def container_pos(node, window_w: int) -> tuple[int, int, int, int] | None:
    """从标题节点向上找最近的「像一张卡」的祖先（宽度 > 300px）。

    WebKit 在这里不让 section 带名字，直接按 role=section 找会全部落空；
    祖先链能稳定拿到卡片容器的外接矩形，用它判断横向溢出。

    只认「比窗口窄、且不是整页高」的那一层：再往上就是铺满窗口的主滚动区，
    它的宽度等于窗口宽度，拿来判溢出会把整页容器误当成卡片。
    """
    cur = node
    for _ in range(6):
        try:
            cur = cur.accessible.get_parent()
        except Exception:
            return None
        if cur is None:
            return None
        pos = node_pos(ax._node(cur))
        if pos is not None and 300 < pos[2] < window_w and pos[3] < 1800:
            return pos
    return None


def iso_week(day: dt.date) -> str:
    year, week, _ = day.isocalendar()
    return f"{year}-W{week:02d}"


def write_worklog(root: Path) -> dict:
    today = dt.date.today()
    week = iso_week(today)
    monday = today - dt.timedelta(days=today.weekday())
    # 相对今天取日期：昨天及更早 = 已过日期（先做/回看），今天及以后 = 排期/可缓。
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
    (worklog_dir / f"{week}.md").write_text("\n".join(lines), encoding="utf-8")

    report_dir = root / "周报"
    report_dir.mkdir(parents=True, exist_ok=True)
    for back in (1, 2, 3):
        label = iso_week(monday - dt.timedelta(days=7 * back))
        (report_dir / f"{label}-周报.md").write_text(
            f"# {label} 周报\n\n（合成样例，仅用于界面预览。）\n", encoding="utf-8"
        )
    return {"week": week, "entries": len(entries), "today": today.isoformat()}


def write_artifacts(data_root: Path) -> int:
    """合成 3 份本场景成果：编号卡要能读到标题、日期与字数。"""
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
    ap.add_argument("--display", default=":128")
    ap.add_argument("--root", default="/tmp/od093-preview")
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

    # 不复制真实 DSH Home：那会带出真实模型配置与私有数据。AppSession.prepare()
    # 在 root 下建空 dsh-home，配合合成端点即可渲染界面。
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
            # 库根与成果根都必须隔离：工作记录写库根，只设成果目录不够。
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
        (EVID / "00-home-ax.json").write_text(
            json.dumps(
                [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        if dock is None:
            xs.shot(EVID / "00-home.png")
            raise RuntimeError("找不到「工作记录」入口")
        ax.invoke(dock)
        time.sleep(6.0)

        # 01：宽窗三栏。
        xs.shot(EVID / "01-weekly-three-column.png")
        nodes = app_nodes(pid)
        (EVID / "01-weekly-ax.json").write_text(
            json.dumps(
                [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        # 顶部四栏在 WebKit 里不暴露为四个独立段落，因此每栏用 role=group + aria-label
        # 把「标签 + 数字 + 说明」合成一个可读节点；这里逐栏核对，四个都在才算 AX 可读。
        facts["ax_width_ready"] = all(
            any(t in (n.name or "") + (n.text or "") for n in nodes)
            for t in ("本期工作记录", "进周报条目", "本周周报", "逾期")
        )
        facts["ax_quadrant_present"] = all(
            any(t in (n.name or "") + (n.text or "") for n in nodes) for t in ("第1", "第4")
        ) or sum(1 for n in nodes if any(t in (n.name or "") for t in ("先做", "排期", "回看", "可缓"))) >= 4
        facts["ax_history_present"] = any("历史对话" in (n.name or "") + (n.text or "") for n in nodes)
        facts["ax_artifacts_present"] = any("成果文档库" in (n.name or "") + (n.text or "") for n in nodes)
        # 归档并入成果库（2026-10-27 用户反馈）：列表里要能读到「归档」徽标，
        # 而不是另找一张「历史归档」卡。
        facts["ax_archive_badge_present"] = any(
            "归档" in (n.name or "") + (n.text or "") for n in nodes
        )
        facts["ax_standalone_archive_absent"] = not any(
            (n.name or "").strip() == "历史归档" for n in nodes
        )
        facts["ax_entries"] = sum(
            1 for n in nodes if "合成记录" in (n.name or "") + (n.text or "")
        )

        # 02：点第一格筛选。
        cell = None
        for name in ("先做", "排期", "回看", "可缓"):
            cell = find(nodes, name)
            if cell is not None:
                break
        if cell is not None:
            ax.invoke(cell)
            time.sleep(2.5)
            xs.shot(EVID / "02-quadrant-filter.png")
            # WebKit 的 AX 树在布局变化后会慢一拍重建：轮询到筛选控件出现为止，
            # 避免把「读得太早」误判成「筛选没生效」。
            after = []
            applied = False
            for _ in range(12):
                after = app_nodes(pid)
                applied = any(
                    "只看" in (n.name or "") + (n.text or "")
                    or "取消筛选" in (n.name or "") + (n.text or "")
                    for n in after
                )
                if applied:
                    break
                time.sleep(0.75)
            facts["filter_applied"] = applied
            facts["filter_entries"] = sum(
                1 for n in after if "合成记录" in (n.name or "") + (n.text or "")
            )
            (EVID / "02-quadrant-ax.json").write_text(
                json.dumps(
                    [{"role": n.role, "name": n.name, "text": n.text} for n in after],
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )
            # 中栏标题里的「可见/总数 条」是筛选结果最直接的读数。
            for n in after:
                m = re.search(r"(\d+)\s*/\s*(\d+)\s*条", (n.name or "") + (n.text or ""))
                if m:
                    facts["filter_visible"], facts["filter_total"] = int(m.group(1)), int(m.group(2))
                    break
            ax.invoke(find(after, "取消筛选") or cell)
            time.sleep(2.0)
        else:
            facts["filter_applied"] = False

        # 03：窄窗单列。1000 < lg 断点 1024，才真正落到单列；1280 仍是三栏。
        xs.resize(win, NARROW_W, NARROW_H, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "03-narrow-1000.png")
        facts["narrow_width"] = NARROW_W
        narrow_nodes = app_nodes(pid)
        (EVID / "03-narrow-ax.json").write_text(
            json.dumps(
                [{"role": n.role, "name": n.name, "text": n.text} for n in narrow_nodes],
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        order = []
        blocks = []
        for label in ("事项四象限", "本周记录", "成果文档库", "历史对话"):
            # 优先取区块标题节点：卡片容器的左缘与标题可能相差内边距，
            # 「是否同一列」应比较各区块标题的左侧沟槽。
            hit = next(
                (n for n in narrow_nodes if n.role == "heading" and (n.name or n.text or "").strip() == label),
                None,
            ) or find(narrow_nodes, label)
            pos = node_pos(hit) if hit is not None else None
            if pos is None:
                facts.setdefault("narrow_missing", []).append(label)
            else:
                order.append((pos[1], label, pos[0], pos[2]))
                # 溢出要量区块本身：优先按 section 的可访问名称直接取卡片容器；
                # 取不到才从标题向上找，标题宽度本身不代表卡片宽度。
                card = next(
                    (
                        n
                        for n in narrow_nodes
                        if n.role == "section" and (n.name or "").strip() == label
                    ),
                    None,
                )
                cpos = node_pos(card) if card is not None else (
                    container_pos(hit, NARROW_W) if hit is not None else None
                )
                if cpos is not None:
                    blocks.append((label,) + cpos)
        order.sort()
        facts["narrow_order"] = [label for _, label, _, _ in order]
        facts["narrow_positions"] = {label: [x, y, w] for y, label, x, w in order}
        # 单列判定：所有区块标题共用同一条左侧沟槽（左缘相差不超过 12px），且纵向严格递增。
        # 不能比标题文字的横向区间——标题宽度不同，三栏布局下也会“重叠”。
        facts["narrow_columns"] = len(order)
        lefts = [x for _, _, x, _ in order]
        ys = [y for y, _, _, _ in order]
        facts["narrow_left_spread"] = max(lefts) - min(lefts) if lefts else 0
        # 横向溢出：取得区块容器时量容器右缘，否则退回标题右缘并标记来源。
        facts["narrow_block_count"] = len(blocks)
        if blocks:
            facts["narrow_blocks"] = {
                label: [x, y, w, h] for label, x, y, w, h in blocks
            }
            facts["narrow_max_right"] = max(x + w for _, x, _, w, _ in blocks)
            facts["narrow_overflow_basis"] = "section"
        else:
            facts["narrow_max_right"] = max((x + w for _, _, x, w in order), default=0)
            facts["narrow_overflow_basis"] = "heading"
        facts["narrow_overflow"] = facts["narrow_max_right"] > NARROW_W
        facts["narrow_single_column"] = (
            len(order) == 4
            and facts["narrow_left_spread"] <= 12
            and ys == sorted(ys)
        )

        # 04：成果库列表视图（确认筛选与切换仍可用）。
        xs.resize(win, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(1.5)
        nodes = app_nodes(pid)
        # 必须按「按钮名恰好是列表」定位：左栏还有「展开对话列表」按钮，
        # 用包含匹配会误点它，截出来的是对话展开态而不是成果列表视图。
        list_btn = next(
            (
                n
                for n in nodes
                if n.role == "button" and (n.name or "").strip() == "列表"
            ),
            None,
        )
        if list_btn is not None:
            ax.invoke(list_btn)
            time.sleep(2.0)
            xs.shot(EVID / "04-artifact-list.png")
            after_list = app_nodes(pid)
            # 列表视图才显示字节数；卡片视图没有这一列，用它作为视图真的切换了的证据。
            facts["list_view_clicked"] = True
            facts["list_view_confirmed"] = any(
                "字节" in (n.name or "") + (n.text or "") for n in after_list
            )
        else:
            facts["list_view_clicked"] = False
            facts["list_view_confirmed"] = False

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
