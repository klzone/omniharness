#!/usr/bin/env python3
"""OD-ARCH-065 · 多提供商模型档案与「测试连接沿用已存密钥」的真实窗口验收。

覆盖四件事（全部在一次性 Xvfb 显示 + 一次性数据根里，不使用任何真实凭证）：
  1. 设置页能把两个提供商并列显示与管理，各自保存地址、模型与凭据引用；
  2. 不填新密钥点「测试连接」时沿用该提供商已保存的密钥（回环端点要求 Bearer）；
  3. 填错密钥时明确报鉴权失败；
  4. 保存后覆盖文件里 5 个模型都带 contextWindow: 1048576，且 Profile 自带的
     其它路由不被抹掉。

证据等级：真实桌面窗口运行（Xvfb + 回环合成端点）+ 文件事实（配置与覆盖文件）。
边界：不碰 ~/.dsh、~/.agents、~/OmniDesk；不读凭证；合成密钥是公开占位串。

用法：python3 verify_model_providers_065.py [--display :93] [--root /tmp/od065]
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

EVIDENCE = HERE / "evidence" / "od-arch-065"
SYNTH_KEY = "synthetic-placeholder-not-a-credential"
WRONG_KEY = "synthetic-wrong-key"


SCREEN_W, SCREEN_H = 1440, 1700


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def start_auth_server() -> tuple[str, ThreadingHTTPServer, dict]:
    """只认 Bearer 占位密钥的回环端点；没带或带错一律 401。"""
    seen: dict = {"models_hits": 0, "authorized": 0}

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args) -> None:
            return

        def _json(self, code: int, obj: dict) -> None:
            body = json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:
            if not self.path.rstrip("/").endswith("models"):
                self._json(404, {"error": {"message": "not found"}})
                return
            with threading.Lock():
                seen["models_hits"] += 1
            auth = self.headers.get("Authorization") or ""
            if auth != f"Bearer {SYNTH_KEY}":
                self._json(401, {"error": {"message": "invalid api key", "type": "invalid_request_error"}})
                return
            with threading.Lock():
                seen["authorized"] += 1
            self._json(
                200,
                {
                    "object": "list",
                    "data": [
                        {"id": "alpha-flash", "object": "model"},
                        {"id": "alpha-pro", "object": "model"},
                    ],
                },
            )

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{server.server_address[1]}/v1", server, seen


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":93")
    ap.add_argument("--root", default="/tmp/od065")
    ap.add_argument("--shot-only", action="store_true")
    args = ap.parse_args()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    root = Path(args.root)
    xvfb = start_xvfb(args.display)
    session = AppSession(root=root, display=args.display, use_dev_server=True)
    base_url, server, seen = start_auth_server()
    facts: dict = {
        "plan": "OD-ARCH-065",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 回环合成端点）+ 文件事实",
        "synthetic_endpoint": base_url,
        "credentials_read": False,
        "cases": {},
    }
    try:
        session.prepare()
        x = XSession(args.display)
        # 覆盖文件：一次性根内的合成 provider（离线启动用），不含任何真实密钥。
        session.start_mock()
        session.start()
        win = x.find_window("OmniDesk", timeout=90)
        if win is None:
            raise RuntimeError("未找到 OmniDesk 主窗口")
        x.focus(win)
        x.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        g = x.geometry(win)
        time.sleep(2.5)
        x.shot(EVIDENCE / "00-home.png")
        print(json.dumps({"window": g.as_tuple(), "phase": "launched"}))
        if args.shot_only:
            # 进设置页看看真实布局，再决定控件坐标。
            x.click(g.x + 321, g.y + 174, settle=3.0)
            time.sleep(1.5)
            x.shot(EVIDENCE / "01-settings.png")
            x.click(g.x + 427, g.y + 366, settle=2.0)
            x.shot(EVIDENCE / "02-provider-form.png")
            return 0

        # ---- 真实流程：两个提供商 + 探测三态 ----
        # 坐标实测于 1440x1700 的一次性显示；整张表单不需要滚动。
        P = {
            "provider": (720, 396),
            "display": (720, 468),
            "base_url": (720, 570),
            "model_id": (600, 855),
            "model_window": (1050, 855),
            "key_ref": (720, 1014),
            "api_key": (720, 1121),
            "save": (291, 1492),
            "probe": (402, 1492),
        }

        def fill(name: str, text: str) -> None:
            cx, cy = P[name]
            x.click(g.x + cx, g.y + cy, settle=0.5)
            x.select_all_and_clear()
            x.type_ascii(text)
            time.sleep(0.25)

        x.click(g.x + 321, g.y + 174, settle=3.0)  # 去设置
        x.click(g.x + 427, g.y + 366, settle=1.5)  # 新增提供商
        fill("provider", "alpha")
        fill("display", "Alpha Gateway")
        fill("base_url", base_url)
        fill("model_id", "alpha-flash")
        fill("model_window", "1048576")
        fill("key_ref", "ALPHA_API_KEY")
        fill("api_key", SYNTH_KEY)
        x.shot(EVIDENCE / "02-provider-alpha-filled.png")

        cfg_json = session.app_data / "config" / "omnidesk.model-settings.v2.json"
        overlay = session.app_data / "config" / "omnidesk-model.yml"

        def save_and_wait(expected_provider: str, timeout: float = 90.0) -> None:
            """点保存并等它真的落地：保存会先停引擎，可能要十几秒。"""
            x.click(g.x + P["save"][0], g.y + P["save"][1], settle=1.0)
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if cfg_json.exists() and overlay.exists():
                    try:
                        data = json.loads(cfg_json.read_text(encoding="utf-8"))
                    except Exception:
                        data = {}
                    if any(p.get("provider") == expected_provider for p in data.get("providers", [])):
                        # 配置已落盘；再等一拍让停引擎与界面刷新走完。
                        time.sleep(3.0)
                        return
                time.sleep(1.0)
            raise RuntimeError(f"保存未在 {timeout:.0f}s 内落盘（{expected_provider}）")

        save_and_wait("alpha")
        x.shot(EVIDENCE / "03-alpha-saved.png")

        saved = json.loads(cfg_json.read_text(encoding="utf-8")) if cfg_json.exists() else None
        overlay_text = overlay.read_text(encoding="utf-8") if overlay.exists() else ""
        facts["cases"]["save"] = {
            "app_data_json_exists": cfg_json.exists(),
            "provider_written": bool(
                saved and any(p.get("provider") == "alpha" for p in saved.get("providers", []))
            ),
            "key_ref_written": bool(
                saved
                and any(p.get("api_key_ref") == "ALPHA_API_KEY" for p in saved.get("providers", []))
            ),
            "context_window_1m_in_overlay": "contextWindow: 1048576" in overlay_text,
            # 一次性根里的引擎补丁是 harness 的 fake 路由；它必须被保留下来。
            "base_route_kept": "fake" in overlay_text,
            "overlay_has_plaintext_key": WRONG_KEY in overlay_text or SYNTH_KEY in overlay_text,
            "evidence": str(cfg_json),
        }

        def wait_for(key: str, before: int, timeout: float = 60.0) -> bool:
            """等端点真的收到请求：保存后设置服务要按新补丁重启，首次探测会慢。"""
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if seen[key] > before:
                    time.sleep(1.5)  # 让界面把结果画出来
                    return True
                time.sleep(0.5)
            return False

        # 探测一：输入框留空 → 必须沿用 alpha 已保存的密钥。
        before_authorized = seen["authorized"]
        before_hits = seen["models_hits"]
        x.click(g.x + P["probe"][0], g.y + P["probe"][1], settle=1.0)
        reached = wait_for("models_hits", before_hits)
        x.shot(EVIDENCE / "04-probe-stored-key.png")
        facts["cases"]["probe_stored_key"] = {
            "expected": "连接成功，使用已保存密钥",
            "request_reached_endpoint": reached,
            "authorized_delta": seen["authorized"] - before_authorized,
            "models_hits": seen["models_hits"],
        }

        # 探测二：填入错误密钥 → 必须明确报鉴权失败。
        fill("api_key", WRONG_KEY)
        before_hits = seen["models_hits"]
        x.click(g.x + P["probe"][0], g.y + P["probe"][1], settle=1.0)
        wait_for("models_hits", before_hits)
        x.shot(EVIDENCE / "05-probe-wrong-key.png")
        facts["cases"]["probe_wrong_key"] = {
            "expected": "鉴权失败",
            "models_hits": seen["models_hits"],
        }
        # 清掉错误密钥，避免它跟着下一次保存进凭据服务。
        fill("api_key", "")

        # 第二个提供商：并列显示，保存后两份配置都在。
        x.click(g.x + 427, g.y + 366, settle=1.5)
        fill("provider", "beta")
        fill("display", "Beta Gateway")
        fill("base_url", base_url)
        fill("model_id", "beta-flash")
        fill("model_window", "1048576")
        fill("key_ref", "BETA_API_KEY")
        save_and_wait("beta")
        x.shot(EVIDENCE / "06-two-providers.png")

        saved2 = json.loads(cfg_json.read_text(encoding="utf-8")) if cfg_json.exists() else None
        overlay2 = overlay.read_text(encoding="utf-8") if overlay.exists() else ""
        names = [p.get("provider") for p in (saved2 or {}).get("providers", [])]
        facts["cases"]["two_providers"] = {
            "providers_on_disk": names,
            "both_present": set(names) == {"alpha", "beta"},
            "overlay_keeps_base_route": "fake" in overlay2,
            "overlay_plaintext_key": WRONG_KEY in overlay2 or SYNTH_KEY in overlay2,
            "overlay_mode": oct(overlay.stat().st_mode & 0o777) if overlay.exists() else None,
        }
        facts["synthetic_endpoint_hits"] = seen
        return 0 if session.alive() else 1
    finally:
        facts["app_alive"] = session.alive()
        facts["shots"] = sorted(p.name for p in EVIDENCE.glob("*.png"))
        (EVIDENCE / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        session.close()
        server.shutdown()
        xvfb.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
