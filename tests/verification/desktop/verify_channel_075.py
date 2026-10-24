#!/usr/bin/env python3
"""OD-ARCH-075 真实窗口验收：设置页「引擎诊断」当前通道说明。

一次性 Xvfb + 一次性隔离根（只读复制本机 OmniDesk 隔离实例）+ 回环合成端点：
  1. 进设置 → 引擎诊断，截图；
  2. 核对界面出现「当前通道」「web-api」与两条能力差异说明；
  3. 核对设置页不再出现「DSH 原生界面（诊断用）」入口（OD-ARCH-076 S2 同页验证）；
  4. 核对原版 ~/.dsh、~/.agents 与源隔离根指纹未变化。

不读凭证、不联网、不碰真实业务材料。

用法：python3 verify_channel_075.py [--display :88] [--root /tmp/od075-verify]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-075"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"

SETTINGS_GEAR = (1117, 44)
ENGINE_TAB = (1078, 201)


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            err = proc.stderr.read().decode("utf-8", "replace") if proc.stderr else ""
            raise RuntimeError(f"Xvfb {display} 启动失败：{err.strip()}")
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            try:
                import socket

                with socket.socket(socket.AF_UNIX) as s:
                    s.settimeout(0.5)
                    if s.connect_ex(f"/tmp/.X11-unix/X{dno}") == 0:
                        return proc
            except OSError:
                pass
        time.sleep(0.25)
    raise RuntimeError(f"Xvfb {display} 未在超时内就绪")


def tree_fingerprint(root: Path, limit: int = 4000) -> str:
    h = hashlib.sha256()
    if not root.exists():
        return "missing"
    count = 0
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        count += 1
        st = p.stat()
        h.update(f"{p.relative_to(root)}|{st.st_size}|{int(st.st_mtime)}".encode())
        if count >= limit:
            break
    return h.hexdigest()[:16] + f":{count}"


def check_settings_dom() -> dict:
    """静态核对设置页源码：通道说明存在、DSH 原生界面入口已移除。

    这是源码级检查，不替代截图；两者一起作为本步证据。
    """
    panel = (HERE.parents[2] / "desktop" / "src" / "SettingsPanel.tsx").read_text(
        encoding="utf-8"
    )
    return {
        "has_channel_block": "当前通道与能力差异" in panel,
        "has_web_api_note": "支持在界面里逐条审批工具调用" in panel,
        "has_acp_note": "当前不支持在界面里审批工具调用" in panel,
        # 只锚定引擎诊断页那个入口；能力管理页的插件说明文案由 OD-ARCH-077 处理。
        "no_dsh_native_entry": "DSH 原生界面（诊断用）" not in panel,
        "no_open_dsh_settings": "openDshSettings" not in panel,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":88")
    ap.add_argument("--root", default="/tmp/od075-verify")
    ap.add_argument("--tab-x", type=int, default=ENGINE_TAB[0])
    ap.add_argument("--tab-y", type=int, default=ENGINE_TAB[1])
    args = ap.parse_args()
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {"display": args.display, "root": str(root)}
    facts["source_before"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_before"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_before"] = tree_fingerprint(Path.home() / ".agents")
    shutil.copytree(SOURCE_HOME, root / "dsh-home", symlinks=True)

    xvfb = start_display(args.display)
    session = AppSession(root=root, display=args.display, artifact_override=True)
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("没有出现 OmniDesk 主窗口")
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(7)

        x.click(*SETTINGS_GEAR, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "01-settings-tabs.png")
        x.click(args.tab_x, args.tab_y, settle=2.5)
        time.sleep(4.0)
        # 收起原生 tooltip 等覆盖窗，避免它盖住「当前通道」块。
        x.dismiss_overlays()
        time.sleep(0.6)
        x.shot(EVID / "02-engine-tab.png")
        caps = EVID / "02-engine-tab.png"
        facts["engine_tab_shot"] = caps.name
        # 删除设置页里的 DSH 原生界面入口后，整页高度会明显变短；
        # 这里不做像素断言，只记录截图路径供人工核对（文字断言见下面的 DOM 校验）。
        facts["settings_dom_checked"] = check_settings_dom()
        facts["alive"] = session.alive()
        log_text = (session.logs / "app.log").read_text(encoding="utf-8", errors="replace")
        facts["app_log_bytes"] = len(log_text)
        facts["app_log_has_error"] = "ERROR" in log_text or "panicked" in log_text
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    facts["source_after"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_after"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_after"] = tree_fingerprint(Path.home() / ".agents")
    facts["source_unchanged"] = facts["source_before"] == facts["source_after"]
    facts["user_dsh_unchanged"] = facts["user_dsh_before"] == facts["user_dsh_after"]
    facts["user_agents_unchanged"] = facts["user_agents_before"] == facts["user_agents_after"]
    checks = facts.get("settings_dom_checked") or {}
    passed = bool(
        facts["alive"]
        and facts["source_unchanged"]
        and facts["user_dsh_unchanged"]
        and facts["user_agents_unchanged"]
        and all(checks.values())
    )
    facts["passed"] = passed
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({k: facts[k] for k in ("passed", "settings_dom_checked", "alive")}, ensure_ascii=False, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
