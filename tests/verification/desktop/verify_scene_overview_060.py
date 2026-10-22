#!/usr/bin/env python3
"""OD-ARCH-060 · 场景页 v2（状态条 + 文档库 + 会话历史折叠）真实窗口验收。

全部数据由本脚本在 /tmp 一次性目录里合成：无真实材料、无凭证、不碰用户日常数据。
证据等级：真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实（宿主记录的 JSON/成果文件）。

用法：python3 verify_scene_overview_060.py [--display :97] [--root /tmp/od060-verify]
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

EVIDENCE = HERE / "evidence" / "od-arch-060"
SCREEN_W, SCREEN_H = 1440, 900

# Dock 坐标（1440×900，OD-ARCH-061 复标定：5 个业务入口，行距 50，垂直居中）。
# 实测像素：首页 350 / 工作记录 400 / 文案写作 450 / 项目跟踪 500 / 历史会话 550。
# 旧值（325/375/425）是 8 入口时代的坐标，061 去掉 3 个图标后整体下移 75px。
DOCK_DX = 47
DOCK_WEEKLY_DY = 400
DOCK_WRITING_DY = 450
DOCK_PROJECT_DY = 500

NOW = time.time()
WEEK_FILES = [
    # 新格式（058 frontmatter）：标题来自 frontmatter
    ("offline-weekly-report-20261020-090000.md", "weekly_report", "合成周事项整理", True),
    ("offline-weekly-report-20261021-140000.md", "weekly_report", "合成第二份工作记录", True),
    ("offline-material-writing-20261022-100000.md", "material_writing", "合成文案初稿", True),
    ("offline-project-management-20261019-160000.md", "project_management", "合成项目阶段成果", True),
    # 老格式（无 frontmatter）：按文件名显示，标「无元数据」
    ("offline-project-management-20250105-080000.md", "project_management", "", False),
]


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def seed_artifacts(local_data: Path) -> list[str]:
    local_data.mkdir(parents=True, exist_ok=True)
    written = []
    for i, (name, scenario, title, new_format) in enumerate(WEEK_FILES):
        body = "# 合成样例\n\n本文件由验收脚本生成，内容与真实工作无关。\n\n- 事项 A：完成\n- 事项 B：进行中\n"
        if new_format:
            text = (
                "---\n"
                f"scenario: {scenario}\n"
                f"title: \"{title}\"\n"
                f"created: 2026-10-2{i % 3}T0{i % 9}:00:00Z\n"
                "tags:\n  - omnidesk\n"
                "omnidesk_version: 0.1.0\n"
                "---\n"
                + body
            )
        else:
            text = body
        (local_data / name).write_text(text, encoding="utf-8")
        written.append(name)
    # 本周窗口内的文件按名字里的日期排序，但列表按 mtime 倒序，这里把时间拉开
    for i, name in enumerate(written):
        ts = NOW - 3600 * (i + 1)
        import os

        os.utime(local_data / name, (ts, ts))
    return written


def seed_pending(app_data: Path) -> list[str]:
    pending_dir = app_data / "engine-state" / "pending-writes"
    pending_dir.mkdir(parents=True, exist_ok=True)

    def record(rid: str, expires_at: float, scenario: str, title: str) -> dict:
        return {
            "schema": "omnidesk.pending-write.v1",
            "id": rid,
            "action": "save_offline",
            "source": "scheduler",
            "summary": f"合成待确认：{title}",
            "impact": "合成验收记录，仅写入一次性目录。",
            "payload": {
                "scenario": scenario,
                "title": title,
                "markdown": "## 合成正文\n\n这是验收脚本合成的待写入内容。\n",
                "ai_assisted": False,
            },
            "created_at": NOW - 600,
            "expires_at": expires_at,
            "status": "pending",
            "resolved_at": None,
        }

    ids = ["verify060-pending", "verify060-expired"]
    (pending_dir / f"{ids[0]}.json").write_text(
        json.dumps(record(ids[0], NOW + 12 * 3600, "weekly_report", "合成待确认成果"),
                   ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    (pending_dir / f"{ids[1]}.json").write_text(
        json.dumps(record(ids[1], NOW - 7200, "weekly_report", "合成已过期成果"),
                   ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    return ids


def mount(xs: XSession) -> object:
    win = xs.find_window("OmniDesk", timeout=90)
    if win is None:
        raise RuntimeError("未找到 OmniDesk 主窗口")
    xs.focus(win)
    xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
    time.sleep(2.0)
    return win


def click(xs: XSession, win, dx: int, dy: int, settle: float = 1.2) -> None:
    g = xs.geometry(win)
    point = (g.x + dx, g.y + dy)
    xs.focus(win)
    xs.move(*point)
    time.sleep(0.3)
    xs.click(*point, settle=0.4)
    xs.click(*point, settle=settle)


def tap(xs: XSession, win, dx: int, dy: int, settle: float = 1.2) -> None:
    """单次点击：用于开关类控件（连点两次会把刚展开的面板又收起来）。"""
    g = xs.geometry(win)
    point = (g.x + dx, g.y + dy)
    xs.focus(win)
    xs.move(*point)
    time.sleep(0.3)
    xs.click(*point, settle=settle)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root", default="/tmp/od060-verify")
    ap.add_argument("--click", action="append", default=[], help="x,y 追加一次点击后再截一张")
    ap.add_argument("--probe", action="store_true", help="只起窗口并截三张场景图，用于校准坐标")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    EVIDENCE.mkdir(parents=True, exist_ok=True)

    xvfb = start_xvfb(args.display)
    session = AppSession(root=root, display=args.display, artifact_override=True)
    facts: dict = {
        "plan": "OD-ARCH-060",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实",
        "root": str(root),
        "steps": {},
    }
    try:
        session.prepare()
        facts["artifacts_seeded"] = seed_artifacts(session.local_data)
        facts["pending_seeded"] = seed_pending(session.app_data)
        xs = XSession(args.display)
        session.start()
        win = mount(xs)
        xs.shot(EVIDENCE / "00-home.png")

        for key, dy in (("weekly", DOCK_WEEKLY_DY), ("writing", DOCK_WRITING_DY), ("project", DOCK_PROJECT_DY)):
            click(xs, win, DOCK_DX, dy, settle=2.5)
            xs.shot(EVIDENCE / f"01-scene-{key}.png")
            facts["steps"][f"scene_{key}"] = {
                "screenshot": str(EVIDENCE / f"01-scene-{key}.png"),
                "dock_dy": dy,
            }

        if args.probe:
            for i, spec in enumerate(args.click):
                dx, dy = (int(v) for v in spec.split(","))
                click(xs, win, dx, dy, settle=2.0)
                xs.shot(EVIDENCE / f"99-probe-{i}.png")
            facts["app_alive_at_end"] = session.alive()
        else:
            # 回工作记录页，先做筛选（此时页面版式未展开，搜索框在 y=507）。
            click(xs, win, DOCK_DX, DOCK_WEEKLY_DY, settle=2.5)
            click(xs, win, 180, 507, settle=0.8)
            xs.select_all_and_clear()
            xs.type_ascii("20261021", settle=0.05)
            time.sleep(1.5)
            xs.shot(EVIDENCE / "03-filter-applied.png")
            facts["steps"]["filter"] = {
                "screenshot": str(EVIDENCE / "03-filter-applied.png"),
                "query": "20261021",
                "expect": "只保留 offline-weekly-report-20261021-140000.md 一条，计数变 1 份",
            }

            # 清空过滤，再切到列表视图（旧文件仍按文件名显示）。
            click(xs, win, 180, 507, settle=0.6)
            xs.select_all_and_clear()
            time.sleep(1.0)
            click(xs, win, 1275, 507, settle=1.5)
            xs.shot(EVIDENCE / "04-list-view.png")
            facts["steps"]["list_view"] = {
                "screenshot": str(EVIDENCE / "04-list-view.png"),
                "expect": "列表视图列出全部 2 份本场景成果，日期与字节数可见",
            }

            # 最后展开待确认面板（展开会把下方内容整体下推，所以放在最后一步）。
            tap(xs, win, 300, 240, settle=2.5)
            xs.shot(EVIDENCE / "02-pending-panel.png")
            facts["steps"]["pending_panel"] = {
                "screenshot": str(EVIDENCE / "02-pending-panel.png"),
                "expect": "展开后列出 2 条合成待确认记录（1 条可确认、1 条已过期只能放弃）",
            }
            facts["app_alive_at_end"] = session.alive()
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    fname = "probe-facts.json" if args.probe else "facts.json"
    (EVIDENCE / fname).write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    if args.probe:
        return 0
    ok = bool(facts.get("app_alive_at_end")) and len(facts["steps"]) >= 5
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
