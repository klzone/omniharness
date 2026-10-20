#!/usr/bin/env python3
"""OD-ARCH-033 验收截图：首页 + 三个场景页（提醒/建议栏 + 双栏 + 对话区）。"""
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

OUT = REPO / "tests/verification/desktop/evidence/od-arch-033/shots"
OUT.mkdir(parents=True, exist_ok=True)

root = Path("/tmp/od033/shots")
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
    ui = s5.AtspiUi(); ui.connect(timeout=60)

    def click(label):
        for r, nm, node, _d in ui.walk():
            if r == "button" and label in nm:
                return ui.click(node)
        return False

    def shot(name):
        p = OUT / f"od033-{name}.png"
        s5.capture_window(win, p)
        print("shot:", p.name)

    shot("01-home")
    for name, label in [("02-weekly", "周报与随记"), ("03-writing", "文案写作"), ("04-project", "项目跟踪")]:
        print("click:", label, click(label))
        time.sleep(2.5)
        shot(name)
    # 项目跟进页的「进度 / 阶段成果」页签
    print("click 阶段成果:", click("阶段成果"))
    time.sleep(1.5)
    shot("05-project-artifacts-tab")
    # 项目与会话模块
    print("click 项目与会话:", click("会话与任务"))
    time.sleep(2.5)
    shot("06-module")
    names = [n for r, n, _x, _d in ui.walk() if r == "button"]
    print("buttons:", json.dumps(names, ensure_ascii=False)[:500])
finally:
    app.stop_all()
