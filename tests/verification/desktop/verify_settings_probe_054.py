#!/usr/bin/env python3
"""OD-ARCH-054 · 设置页连接测试与首次运行引导的真实窗口验收。

覆盖三态（全部使用合成数据，密钥为公开占位串，不读取任何真实凭证）：
  1. 成功：隔离数据根 + 回环合成端点，拉到模型列表并出现候选下拉；
  2. 连不上：`http://127.0.0.1:9/v1`（本机未监听端口）；
  3. 鉴权失败：另一个只对 /models 回 401 的一次性端点。
另有首次运行引导卡（新数据根、无模型配置）截图。

边界：一次性 Xvfb 显示 + 一次性数据根；不碰 ~/.dsh、~/.agents、~/OmniDesk；
不改用户配置；截图不含密钥回显（输入框为 password）。

用法：python3 verify_settings_probe_054.py [--display :96] [--root /tmp/od054-s1]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-054"


SCREEN_W, SCREEN_H = 1440, 1700


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", f"0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def start_rejecting_server() -> tuple[str, ThreadingHTTPServer]:
    """只对 /models 返回 401 的一次性端点（模拟错误密钥）。"""

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args) -> None:
            return

        def _send(self) -> None:
            body = json.dumps({"error": {"message": "invalid api key", "type": "invalid_request_error"}}).encode()
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:
            self._send()

        def do_POST(self) -> None:
            self._send()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    port = server.server_address[1]
    return f"http://127.0.0.1:{port}/v1", server


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":96")
    ap.add_argument("--root", default="/tmp/od054-s1")
    args = ap.parse_args()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    xvfb = start_xvfb(args.display)
    session = AppSession(root=Path(args.root), display=args.display)
    reject_url, reject_server = start_rejecting_server()
    facts: dict = {"cases": {}, "evidence_dir": str(EVIDENCE)}
    try:
        session.prepare()
        x = XSession(args.display)
        mock = session.start_mock()
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到 OmniDesk 主窗口")
        x.focus(win)
        # 一次性显示做成「高屏」，让整张设置表单一次显示完：
        # 滚动后点固定坐标会随上一次状态漂移（前两版都踩到过）。
        x.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        g = x.geometry(win)
        time.sleep(2.0)
        x.shot(EVIDENCE / "01-first-run-banner.png")

        # 进设置：用引导卡上的「去设置」按钮（左侧 Dock 在高屏下会重排，点固定坐标不可靠）。
        x.click(g.x + 321, g.y + 174, settle=2.5)
        time.sleep(1.0)
        x.shot(EVIDENCE / "02-settings-fresh.png")

        # 高屏下一次显示完整表单：各控件坐标固定（窗口内相对坐标，实测于 1440x1700）。
        def fill_and_probe(url: str, tag: str, wait: float, key: str | None = None) -> None:
            x.click(g.x + 750, g.y + 500, settle=0.6)  # 接口地址
            x.select_all_and_clear()
            x.type_ascii(url)
            time.sleep(0.4)
            if key:
                x.click(g.x + 750, g.y + 984, settle=0.4)  # API Key（password 框）
                x.type_ascii(key)
                time.sleep(0.3)
            x.click(g.x + 409, g.y + 1074, settle=0.5)  # 测试连接
            time.sleep(wait)
            x.shot(EVIDENCE / f"{tag}.png")

        # 顺序很关键：成功一次后表单会多出「模型候选」下拉，整张表单下移约 72px。
        # 先跑两个失败态（布局仍是初始表单），最后跑成功态。
        fill_and_probe("http://127.0.0.1:9/v1", "03-probe-unreachable", 12.0)
        facts["cases"]["unreachable"] = {"endpoint_kind": "127.0.0.1:9 (closed)", "expected": "连不上"}

        fill_and_probe(reject_url, "04-probe-auth", 10.0, key="synthetic-wrong-key")
        facts["cases"]["auth"] = {
            "endpoint_kind": "loopback 401 stub",
            "key": "synthetic-wrong-key（占位串，非真实凭证）",
            "expected": "鉴权失败",
        }

        fill_and_probe(mock, "05-probe-ok", 8.0)
        facts["cases"]["ok"] = {"endpoint_kind": "loopback synthetic mock", "expected": "ok + 模型候选下拉"}

        facts["shots"] = sorted(p.name for p in EVIDENCE.glob("*.png"))
        facts["app_alive"] = session.alive()
        (EVIDENCE / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(facts, ensure_ascii=False, indent=2))
        return 0 if session.alive() else 1
    finally:
        session.close()
        reject_server.shutdown()
        xvfb.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
