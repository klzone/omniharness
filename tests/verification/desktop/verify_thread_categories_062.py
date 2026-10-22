#!/usr/bin/env python3
"""OD-ARCH-062 S1 真实窗口验收（一次性隔离根 + 合成数据，无凭证、无真实数据）。"""
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

EVID = HERE / "evidence" / "od-arch-062"


def open_settings(x: XSession) -> None:
    x.click(1117, 44, settle=2.5)
    time.sleep(1.5)


def open_categories(x: XSession) -> None:
    x.click(383, 201, settle=2.0)
    time.sleep(1.2)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":98")
    ap.add_argument("--root", default="/tmp/od062-window")
    args = ap.parse_args()
    EVID.mkdir(parents=True, exist_ok=True)
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)
    xvfb = subprocess.Popen(
        ["Xvfb", args.display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(2)
    # artifact_override：把成果根指到一次性根里，避免「初始化骨架」写进真实 ~/OmniDesk。
    session = AppSession(root=root, display=args.display, artifact_override=True)
    facts: dict = {}
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(6)
        open_settings(x)
        open_categories(x)
        x.shot(EVID / "01-categories-edit.png")
        # 滚到「新增分类」卡片
        x.scroll(720, 600, 12, settle=1.2)
        x.shot(EVID / "01b-categories-new.png")
        # 填新分类名（ASCII 可输入）+ 归档子文件夹
        x.click(750, 728, settle=0.6)
        x.type_ascii("Study Log")
        x.click(750, 806, settle=0.6)
        x.type_ascii("study-log")
        x.shot(EVID / "02-new-category-filled.png")
        x.click(292, 858, settle=2.5)
        x.shot(EVID / "03-new-category-added.png")
        # 初始化目录骨架（只建缺失；已存在跳过）
        x.scroll(720, 400, 60, up=True, settle=1.5)
        x.shot(EVID / "03b-back-to-top.png")
        x.click(1202, 293, settle=3.0)
        x.shot(EVID / "03c-init-receipt.png")

        # 回首页，点分类胶囊展开下拉（OD-ARCH-063 起 chip 收进下拉），确认新分类出现在列表里
        x.click(47, 351, settle=3.0)
        time.sleep(2)
        x.click(240, 831, settle=1.5)
        time.sleep(1.2)
        x.shot(EVID / "04-chip-dropdown.png")
        # 5 项时面板向上长高，列表项自下而上：未分类 640 / 工作记录 669 / 文案/698 / 项目/727 / Study Log 742。
        x.click(240, 742, settle=1.5)
        time.sleep(1.0)
        x.shot(EVID / "04b-chip-selected.png")

        settings_file = session.app_data / "config" / "settings.v1.json"
        # 重启，确认分类仍在（持久化）
        session.stop_app()
        time.sleep(2)
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(6)
        x.shot(EVID / "05-after-restart-chip.png")
        facts["alive_after_restart"] = session.alive()
        facts["settings_file"] = str(settings_file)
        facts["settings_exists"] = settings_file.exists()
        if settings_file.exists():
            data = json.loads(settings_file.read_text(encoding="utf-8"))
            cats = data.get("thread_categories") or []
            facts["categories"] = [
                {"key": c.get("key"), "label": c.get("label"), "folder": c.get("folder")}
                for c in cats
            ]
        facts["alive_after_add"] = session.alive()
        data_root = session.local_data
        facts["local_data_root"] = str(data_root)
        facts["created_dirs"] = sorted(
            str(p.relative_to(data_root)) for p in data_root.rglob("*") if p.is_dir()
        )
    finally:
        session.close()
        xvfb.terminate()
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    ok = bool(
        facts.get("alive_after_restart")
        and facts.get("settings_exists")
        and any(c.get("folder") == "study-log" for c in facts.get("categories", []))
    )
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
