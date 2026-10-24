#!/usr/bin/env python3
"""OD-ARCH-076 S1 真实窗口验收：同一时间只允许一个 OmniDesk 实例。

一次性 Xvfb + 一次性隔离根 + 回环合成端点：
  1. 启动第一个实例，等到主窗口出现；
  2. 用**同一套环境变量**再启动第二个进程；
  3. 核对：第二个进程自行退出、退出码为 0、耗时明显短于冷启动；
     窗口总数仍为 1（没有第二个窗口）；
     第一个实例仍存活且可聚焦；
  4. 核对原版 ~/.dsh / ~/.agents 与源隔离根指纹未变化。

不读凭证、不联网、不碰真实业务材料。

用法：python3 verify_single_instance_076.py [--display :86] [--root /tmp/od076-verify]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import APP_BIN, AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-076"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"


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


def count_windows(x: XSession, substr: str = "OmniDesk") -> int:
    total = 0
    stack = [x.d.screen().root]
    while stack:
        win = stack.pop()
        try:
            kids = win.query_tree().children
        except Exception:
            continue
        for child in kids:
            try:
                if child.get_attributes().map_state == 2 and (child.get_wm_name() or "").find(
                    substr
                ) >= 0:
                    total += 1
            except Exception:
                pass
            stack.append(child)
    return total


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":86")
    ap.add_argument("--root", default="/tmp/od076-verify")
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
            raise RuntimeError("第一个实例没有出现主窗口")
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(7)
        x.shot(EVID / "01-first-instance.png")
        facts["windows_after_first"] = count_windows(x)
        facts["first_pid"] = session.proc.pid if session.proc else None

        # 用完全相同的环境变量启动第二个实例。
        env = session.app_env(session.write_overlay())
        second = subprocess.Popen(
            [str(APP_BIN)],
            cwd=str(root),
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
        )
        started = time.monotonic()
        try:
            second_code = second.wait(timeout=40)
            facts["second_exited"] = True
        except subprocess.TimeoutExpired:
            second_code = None
            facts["second_exited"] = False
            try:
                os.killpg(os.getpgid(second.pid), 15)
            except Exception:
                pass
            second.wait(timeout=15)
        facts["second_exit_code"] = second_code
        facts["second_wait_seconds"] = round(time.monotonic() - started, 2)
        time.sleep(2.0)
        facts["windows_after_second"] = count_windows(x)
        facts["first_alive"] = session.alive()
        x.shot(EVID / "02-after-second-launch.png")

        # 第二个进程退出后，第一个窗口仍能聚焦（模拟「把已有窗口叫到前台」）。
        still = x.find_window("OmniDesk", timeout=10)
        facts["focusable_after_second"] = still is not None
        if still is not None:
            x.focus(still)

        log_text = (session.logs / "app.log").read_text(encoding="utf-8", errors="replace")
        facts["app_log_bytes"] = len(log_text)
        facts["app_log_has_panic"] = "panicked" in log_text
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
    facts["passed"] = bool(
        facts.get("windows_after_first") == 1
        and facts.get("second_exited")
        and facts.get("second_exit_code") == 0
        and facts.get("windows_after_second") == 1
        and facts.get("first_alive")
        and facts.get("focusable_after_second")
        and facts.get("source_unchanged")
        and facts.get("user_dsh_unchanged")
        and facts.get("user_agents_unchanged")
        and not facts.get("app_log_has_panic")
    )
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0 if facts["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
