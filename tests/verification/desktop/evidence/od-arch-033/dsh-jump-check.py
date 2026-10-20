#!/usr/bin/env python3
"""验证修复：点「打开 DSH 设置」拉起的 DSH 是否指向**引擎用的那份** DSH_HOME。"""
import importlib.util
import subprocess
import sys
import time
from pathlib import Path

REPO = Path("/mnt/data/Code/MyWorkStation")
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
spec = importlib.util.spec_from_file_location(
    "s5", REPO / "tests/verification/desktop/evidence/od-arch-027/step5/scripts/step5-real-desktop.py")
s5 = importlib.util.module_from_spec(spec); spec.loader.exec_module(s5)

ui = s5.AtspiUi()
print("a11y:", ui.connect(timeout=60))


def click(label):
    for r, nm, node, _d in ui.walk():
        if r == "button" and label in nm:
            return ui.click(node)
    return False


print("click 设置:", click("成果目录与模型权限"))
time.sleep(2.0)
print("click 打开 DSH 设置:", click("打开 DSH 设置"))
time.sleep(8.0)

out = subprocess.run(["pgrep", "-af", "dsh/lib/bin.js web"], capture_output=True, text=True).stdout
print("DSH web 进程:", out.strip() or "(无)")
for line in out.strip().splitlines():
    pid = line.split()[0]
    try:
        env = Path(f"/proc/{pid}/environ").read_bytes().decode("utf-8", "ignore").split("\0")
    except Exception as e:
        print("  读不到 environ:", e)
        continue
    print(" ", pid, [e for e in env if e.startswith(("DSH_HOME=", "DSH_AGENTS_HOME="))])

log = Path.home() / "OmniDesk/app/logs/dsh-settings.log"
print("启动日志存在:", log.is_file())
if log.is_file():
    txt = log.read_text(encoding="utf-8", errors="ignore")
    # 只回显 URL 的 host 部分，避免把 token 打到记录里
    for line in txt.splitlines()[:6]:
        if "http" in line:
            print("  日志行含本地地址:", line.split("?")[0])
        else:
            print("  日志行:", line[:120])
