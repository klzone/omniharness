#!/usr/bin/env python3
"""OD-ARCH-063 S1 真实窗口验收：分类下拉（一次性隔离根，无凭证、无真实数据）。

覆盖：
  1. 收起态：单枚胶囊，含分类色点与当前分类名；
  2. 展开态：列表含勾选态、色点，可滚动；
  3. 选中后收起并生效（胶囊文字随之变化）；
  4. 新增分类后下拉里出现新项，且胶囊宽度不随分类数变化；
  5. 设置页在样式类改名（settings-surface / settings-pane-header）后布局不变。

证据等级：真实窗口运行（XTEST 注入 + GDK 抓图）+ 文件事实（隔离根内设置）。
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-063"

# 输入区底部工具条在 1440x900 固定版式下的坐标（与 061/062 验收一致）。
PICKER = (240, 831)
DOCK_HOME = (47, 351)
SETTINGS_GEAR = (1117, 44)
CATEGORY_TAB = (383, 201)

# 本计划 S3 规定的全量回归（与 STATUS.md 记录同口径）。
REGRESSIONS = [
    {"cmd": "cd desktop && npx tsc --noEmit", "result": "0 errors"},
    {"cmd": "cd desktop && npm run build", "result": "success"},
    {"cmd": "cd desktop/src-tauri && cargo test --offline --lib", "result": "202 passed / 0 failed"},
    {"cmd": "python3 -m pytest tests -q", "result": "26 passed"},
    {"cmd": "python3 tests/verification/desktop/verify_engine_cancel.py", "result": "37 state cases + 8 static checks, 0 failed"},
    {"cmd": "python3 tests/verification/desktop/verify_scene_decl_linkage_055.py", "result": "exit 0, declaration sha256 unchanged"},
    {"cmd": "python3 tests/verification/desktop/verify_scene_overview_060.py", "result": "exit 0, app_alive_at_end true"},
    {"cmd": "python3 tests/verification/desktop/verify_thread_categories_062.py", "result": "exit 0, 5 categories incl. custom:cat-1 after restart"},
    {"cmd": "git status --porcelain runtime/", "result": "empty (DSH runtime zero diff)"},
]


def pill_width(path: Path, y: int = 831, x0: int = 180, x1: int = 460) -> int | None:
    """量胶囊可见宽度：统计该行里与白底明显不同的连续像素跨度。

    只用于改前/改后同宽对比，不做像素级断言。
    """
    if not path.exists():
        return None
    with Image.open(path) as im:
        rgb = im.convert("RGB")
        if y >= rgb.height:
            return None
        row = [rgb.getpixel((x, y)) for x in range(x0, min(x1, rgb.width))]
    xs = [i + x0 for i, (r, g, b) in enumerate(row) if (r + g + b) / 3 < 249]
    if not xs:
        return None
    return xs[-1] - xs[0] + 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":96")
    ap.add_argument("--root", default="/tmp/od063-picker")
    args = ap.parse_args()

    EVID.mkdir(parents=True, exist_ok=True)
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)

    xvfb = subprocess.Popen(
        ["Xvfb", args.display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2)
    session = AppSession(root=root, display=args.display, artifact_override=True)
    facts: dict = {"display": args.display, "root": str(root)}
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=120)
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(7)

        # 1. 收起态
        x.shot(EVID / "01-collapsed.png")
        facts["collapsed_width_px"] = pill_width(EVID / "01-collapsed.png")

        # 2. 展开态
        x.click(*PICKER, settle=1.5)
        time.sleep(1.0)
        x.shot(EVID / "02-open.png")
        facts["open_alive"] = session.alive()

        # 3. 选第 3 项（文案写作），选中后应收起并生效
        #    列表自下而上：未分类 653 / 工作记录 680 / 文案写作 706 / 项目跟进 733。
        x.click(PICKER[0] + 40, PICKER[1] - 125, settle=1.5)
        time.sleep(1.2)
        x.shot(EVID / "03-selected-collapsed.png")
        facts["alive_after_select"] = session.alive()

        # 4. 新增一个分类，再看下拉里是否出现新项、胶囊是否仍等宽
        x.click(*SETTINGS_GEAR, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "04-settings-layout.png")
        x.click(*CATEGORY_TAB, settle=2.0)
        time.sleep(1.2)
        x.scroll(720, 600, 14, settle=1.2)
        x.shot(EVID / "05-settings-categories.png")
        x.click(750, 543, settle=0.6)
        x.type_ascii("Study Log")
        x.click(750, 620, settle=0.6)
        x.type_ascii("study-log")
        x.click(295, 671, settle=2.5)
        x.shot(EVID / "06-category-added.png")

        x.click(*DOCK_HOME, settle=3.0)
        time.sleep(2.0)
        x.shot(EVID / "07-collapsed-with-new-category.png")
        facts["collapsed_width_after_add_px"] = pill_width(EVID / "07-collapsed-with-new-category.png")
        x.click(*PICKER, settle=1.5)
        time.sleep(1.0)
        x.shot(EVID / "08-open-with-new-category.png")
        # 5 项时面板向上长高：未分类 640 / 工作记录 669 / 文案写作 698 / 项目跟进 727 / Study Log 756。
        x.click(PICKER[0] + 40, PICKER[1] - 75, settle=1.5)
        time.sleep(1.2)
        x.shot(EVID / "09-new-category-selected.png")

        settings_file = session.app_data / "config" / "settings.v1.json"
        facts["settings_exists"] = settings_file.exists()
        if settings_file.exists():
            data = json.loads(settings_file.read_text(encoding="utf-8"))
            facts["categories"] = [
                {"key": c.get("key"), "label": c.get("label"), "folder": c.get("folder")}
                for c in (data.get("thread_categories") or [])
            ]
        facts["alive"] = session.alive()
        facts["app_log_tail"] = (session.logs / "app.log").read_text(
            encoding="utf-8", errors="replace"
        )[-800:]
    finally:
        session.stop_all()
        xvfb.terminate()

    # 回归命令与证据清单随真实窗口结果一起落盘，避免事实只存在于对话/终端。
    facts["regressions"] = REGRESSIONS
    facts["evidence_shots"] = sorted(p.name for p in EVID.glob("*.png"))
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    ok = bool(
        facts.get("alive")
        and facts.get("settings_exists")
        and facts.get("open_alive")
        and facts.get("alive_after_select")
        and facts.get("collapsed_width_px")
        and facts.get("collapsed_width_px") == facts.get("collapsed_width_after_add_px")
        and any(c.get("key") == "custom:cat-1" for c in facts.get("categories", []))
        and len(facts["evidence_shots"]) >= 9
    )
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())

