#!/usr/bin/env python3
"""OD-ARCH-031 改版后的界面核对：首页（无目录树/无分栏）+ 项目与会话模块。"""
import importlib.util
import json
import sys
import time
from pathlib import Path

REPO = Path("/mnt/data/Code/MyWorkStation")
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
from window_harness import AppSession  # noqa: E402

spec = importlib.util.spec_from_file_location(
    "s5", REPO / "tests/verification/desktop/evidence/od-arch-027/step5/scripts/step5-real-desktop.py")
s5 = importlib.util.module_from_spec(spec); spec.loader.exec_module(s5)

OUT = REPO / "tests/verification/desktop/evidence/od-arch-031/shots"
OUT.mkdir(parents=True, exist_ok=True)

root = Path("/tmp/od031/shots")
app = AppSession(root=root, display=":0", artifact_override=True)
app.prepare()
cfg = app.app_data / "config"
cfg.mkdir(parents=True, exist_ok=True)
app.local_data.mkdir(parents=True, exist_ok=True)
(cfg / "settings.v1.json").write_text(json.dumps({
    "schema": "omnidesk.settings.v1", "artifact_root": str(app.local_data),
    "model_permissions": {"local": True, "private": False, "public": False}}, indent=2), encoding="utf-8")

xs = s5.RealXSession(":0")
try:
    app.start()
    win = xs.find_window("OmniDesk", timeout=180)
    xs.place_inside_root(win); xs.focus(win); time.sleep(10)
    s5.capture_window(win, OUT / "od031-home-no-rail.png")
    ui = s5.AtspiUi(); ui.connect(timeout=60)
    # 进入「会话与任务」（项目与会话模块）
    clicked = False
    for r, nm, node, _d in ui.walk():
        if r == "button" and "会话与任务" in nm:
            clicked = ui.click(node)
            break
    print("clicked sessions tab:", clicked)
    time.sleep(2.5)
    s5.capture_window(win, OUT / "od031-module-projects.png")
    names = [n for r, n, _x, _d in ui.walk() if r == "button"]
    print("module buttons:", json.dumps(names, ensure_ascii=False)[:600])
finally:
    app.stop_all()
