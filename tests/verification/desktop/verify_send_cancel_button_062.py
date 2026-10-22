#!/usr/bin/env python3
"""OD-ARCH-062 S2 接线验证：发送与取消合并为一个按钮（真实窗口）。

验证链：
  1. 空闲时按钮是「发送箭头」；
  2. 提交后引擎处于 hang（不返回），按钮变成「停止方块」，且按钮可点；
  3. 点它发出取消，界面出现取消状态文案，任务不再停留在「生成中」；
  4. 全程只用一次性隔离根 + 回环合成端点，不读凭证、不碰真实 ~/.dsh。

证据等级：真实窗口运行（XTEST 注入 + GDK 抓图）+ 文件事实（隔离根内设置）。

用法：python3 verify_send_cancel_button_062.py [--display :97] [--root /tmp/od062-cancel]
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

EVID = HERE / "evidence" / "od-arch-062"
# XTEST 注入只支持 ASCII：提示词保持全 ASCII，避免真实字符注入问题。
PROMPT = "synthetic send-cancel probe: no real information"


def write_settings_public(session: AppSession) -> Path:
    """只写一次性隔离根里的设置：开启公网类别（合成端点不算真实模型）。"""
    path = session.app_data / "config" / "settings.v1.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.settings.v1",
                "artifact_root": None,
                "model_permissions": {"local": True, "private": True, "public": True},
            },
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":99")
    ap.add_argument("--root", default="/tmp/od062-cancel")
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
        write_settings_public(session)
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=120)
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(7)
        x.shot(EVID / "06-idle-send-arrow.png")

        # 输入合成提示词
        x.click(700, 786, settle=1.0)
        x.type_ascii(PROMPT, settle=0.02)
        time.sleep(1.5)
        x.shot(EVID / "07-typed-prompt.png")

        # 让合成端点挂住：提交后应停在「生成中」
        session.mock_ctl.set(mode="hang", hang_seconds=180.0)
        x.click(1285, 831, settle=3.0)
        time.sleep(6)
        x.shot(EVID / "08-generating-stop-button.png")

        # 点同一个按钮取消
        x.click(1285, 831, settle=3.0)
        time.sleep(6)
        x.shot(EVID / "09-after-cancel.png")

        # 任务台账是「取消是否真的送达宿主」的唯一硬证据（界面文案只是它的投影）。
        state_dir = session.app_data / "engine-state"
        ledger = {}
        for path in sorted(state_dir.glob("omni-*.json")):
            try:
                ledger = json.loads(path.read_text(encoding="utf-8"))
            except Exception:
                continue
        facts["task_state"] = ledger.get("state")
        facts["cancel_phase"] = ledger.get("cancel_phase")
        facts["stop_reason"] = ledger.get("stop_reason")
        facts["alive"] = session.alive()
        facts["mock_requests"] = len(session.mock_ctl.new_requests())
    finally:
        session.stop_all()
        xvfb.terminate()
    (EVID / "send-cancel-facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    ok = (
        facts.get("task_state") == "cancelled"
        and facts.get("mock_requests", 0) >= 1
        and facts.get("alive") is True
    )
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
