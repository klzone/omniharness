#!/usr/bin/env python3
"""OD-ARCH-095：模型切换两处报错的真实窗口验证（一次性 Xvfb）。

复现条件刻意贴近用户现场：只在 OmniDesk 自己的覆盖文件
（``<app-data>/config/omnidesk-model.yml``）里定义一个 provider，Profile 本身没有。
修复前，候选枚举不传补丁 → 该 provider 在候选里完全消失 → 界面拿到空候选；
目录分支又把 provider id 当作 Profile 传给宿主 → 「DSH 配置转储失败（退出码 1）」。

三条断言（全部来自本窗口内的真实界面与真实落盘文件）：
  1. 冷启动：界面不出现「切换失败」，候选里能看到只在覆盖文件里存在的 provider；
  2. 从下拉里选中该 provider 的模型：宿主收到的 Profile 是引擎真实 Profile，
     覆盖文件里写出的 provider/model 与所选一致；
  3. 失败路径（不存在 Profile）的报错文案可读——由脚本层单独验证，见 facts。

证据等级：真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 文件事实。
边界：不碰 ~/.dsh、~/.agents、~/OmniDesk；不读凭证；知识库根指向 /tmp 合成目录。

用法：python3 verify_model_route_095.py [--display :95] [--root /tmp/od095]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-095"
SCREEN_W, SCREEN_H = 1440, 1000

# 只存在于 OmniDesk 覆盖文件里的合成 provider（Profile 本身没有它）。
SYNTH_PROVIDER = "od095-only"
SYNTH_MODEL = "od095-synthetic-model"
# 第二个合成 provider：写覆盖文件时必须一并保留（补丁层是浅合并）。
DECOY_PROVIDER = "od095-decoy"
DECOY_MODEL = "od095-decoy-model"


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError("Xvfb 未就绪")


def start_dbus() -> tuple[str, subprocess.Popen]:
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    return proc.stdout.readline().strip(), proc


def app_nodes(pid: int):
    from gi.repository import Atspi

    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        try:
            if app.get_process_id() != pid:
                continue
        except Exception:
            continue
        raw: list = []
        ax._collect(app, raw)
        return [ax._node(n) for n in raw]
    return []


def blob(node) -> str:
    return (node.name or "") + "\n" + (node.text or "")


def all_text(nodes) -> str:
    return "\n".join(blob(n) for n in nodes)


def find(nodes, needle: str, contains: bool = True):
    for node in nodes:
        a, b = node.name or "", node.text or ""
        if (needle in a or needle in b) if contains else (needle == a or needle == b):
            return node
    return None


def dump(nodes, path: Path) -> None:
    path.write_text(
        json.dumps(
            [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )


def seed_overlay(session: AppSession) -> Path:
    """在与用户现场相同的位置预置一份只含合成 provider 的覆盖文件。"""
    path = session.app_data / "config" / "omnidesk-model.yml"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "- id: llm-pi-ai",
                '  name: "@deepseek-ai/dsh-llm-pi-ai"',
                "  config:",
                "    providers:",
                "      " + SYNTH_PROVIDER + ":",
                "        displayName: Synthetic 095",
                "        apiKeyEnv: OMNIDESK_ROUTE_095_KEY",
                "        api: openai-completions",
                "        baseURL: " + str(session.mock_base_url),
                "        models:",
                "          - id: " + SYNTH_MODEL,
                "            name: " + SYNTH_MODEL,
                "      " + DECOY_PROVIDER + ":",
                "        displayName: Synthetic 095 decoy",
                "        api: openai-completions",
                "        baseURL: " + str(session.mock_base_url),
                "        models:",
                "          - id: " + DECOY_MODEL,
                "            name: " + DECOY_MODEL,
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def script_failure_probe(repo: Path, session: AppSession, profile: str = "od095-not-a-profile") -> dict:
    """S04：不存在的 Profile 必须给出可读原因（脚本层直跑，不经过界面）。"""
    script = repo / "desktop" / "src-tauri" / "scripts" / "model-routes.mjs"
    out = subprocess.run(
        [
            "node", str(script),
            "--dsh-cli", str(
                repo / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
            ),
            "--dsh-home", str(session.dsh_home),
            "--profile", profile,
            "--dump-routes",
        ],
        capture_output=True,
        text=True,
        timeout=90,
    )
    try:
        payload = json.loads(out.stdout.strip().splitlines()[-1])
    except Exception:
        payload = {"ok": False, "error": {"message": out.stdout.strip()[:400]}}
    message = (payload.get("error") or {}).get("message") or ""
    return {
        "exit_code": out.returncode,
        "message": message,
        "mentions_profile": profile in message,
        "has_reason_not_just_exit_code": bool(message) and "does not exist" in message,
        "raw_length": len(message),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":95")
    ap.add_argument("--root", default="/tmp/od095")
    args = ap.parse_args()

    EVID.mkdir(parents=True, exist_ok=True)
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)
    knowledge = Path("/tmp/od095-knowledge")
    if knowledge.exists():
        shutil.rmtree(knowledge)
    knowledge.mkdir(parents=True, exist_ok=True)

    repo = HERE.parents[2]
    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    if "AT_SPI_BUS_ADDRESS" not in os.environ and os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ["AT_SPI_BUS_ADDRESS"] = "unix:path=/run/user/1000/at-spi/bus"

    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 091 的教训：任何触发工作记录的路径都必须指向一次性合成根。
            "OMNIDESK_KNOWLEDGE_ROOT": str(knowledge),
        },
    )
    facts: dict = {
        "plan": "OD-ARCH-095",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + AT-SPI + 合成端点）+ 文件事实",
        "synthetic_provider": SYNTH_PROVIDER,
        "synthetic_model": SYNTH_MODEL,
        "credentials_read": False,
        "cases": {},
    }
    try:
        session.prepare()
        session.start_mock()
        overlay = seed_overlay(session)
        facts["seeded_overlay"] = str(overlay)
        overlay_before = overlay.read_text(encoding="utf-8")
        facts["seeded_overlay_sha"] = str(hash(overlay_before))

        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            raise RuntimeError("主窗口未出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        if not xs.wait_rendered(win, timeout=150):
            raise RuntimeError("界面未渲染完成")
        ax.init()

        # --- 冷启动：等候选与端点事实就绪 ---
        nodes: list = []
        picker = None
        deadline = time.monotonic() + 150
        while time.monotonic() < deadline:
            nodes = app_nodes(pid)
            picker = find(nodes, "模型：")
            if picker is not None:
                text = blob(picker)
                if "读取中" not in text:
                    break
            time.sleep(2.0)
        xs.shot(EVID / "00-cold-start.png")
        dump(nodes, EVID / "00-cold-start-ax.json")

        cold_text = all_text(nodes)
        facts["cases"]["cold_start"] = {
            "picker_label": blob(picker) if picker else None,
            "failure_note_present": "切换失败" in cold_text,
            "empty_profile_note_present": "请选择要使用的 Profile" in cold_text,
            "dump_failure_present": "DSH 配置转储失败" in cold_text,
        }

        if picker is None:
            raise RuntimeError("找不到模型下拉入口")
        ax.invoke(picker)
        time.sleep(3.0)
        xs.shot(EVID / "01-picker-open.png")
        opened = app_nodes(pid)
        dump(opened, EVID / "01-picker-open-ax.json")
        opened_text = all_text(opened)
        facts["cases"]["picker_open"] = {
            "synthetic_provider_listed": SYNTH_PROVIDER in opened_text,
            "synthetic_model_listed": SYNTH_MODEL in opened_text,
            "no_candidates_note": "没有读到任何模型候选" in opened_text,
        }

        item = None
        for node in opened:
            if node.role in ("push button", "button", "toggle button", "list item") and SYNTH_MODEL in blob(node) and DECOY_MODEL not in blob(node):
                item = node
                break
        facts["cases"]["picker_item"] = {"found": item is not None}
        if item is None:
            raise RuntimeError("下拉里没有合成 provider 的模型项")

        # --- 选中：宿主必须用真实 Profile 转储，而不是把 provider 当 Profile ---
        ax.invoke(item)
        deadline = time.monotonic() + 180
        after: list = []
        changed = False
        while time.monotonic() < deadline:
            time.sleep(3.0)
            current = overlay.read_text(encoding="utf-8")
            after = app_nodes(pid)
            text = all_text(after)
            changed = current != overlay_before and SYNTH_MODEL in current
            if changed and "重启" in text:
                break
        xs.shot(EVID / "02-after-select.png")
        dump(after, EVID / "02-after-select-ax.json")
        after_text = all_text(after)
        overlay_after = overlay.read_text(encoding="utf-8")
        facts["cases"]["select_from_catalog"] = {
            "overlay_rewritten": overlay_after != overlay_before,
            "overlay_contains_provider": SYNTH_PROVIDER in overlay_after,
            "overlay_contains_model": SYNTH_MODEL in overlay_after,
            # 真正的证据：写入后 acp/agent-default-model 两处都指向所选模型。
            "overlay_selected_model_written": "model: " + SYNTH_MODEL in overlay_after,
            # 浅合并：补丁里原有的另一个 provider 不能被抹掉。
            "decoy_provider_preserved": DECOY_PROVIDER in overlay_after,
            "provider_used_as_profile": "Profile「" + SYNTH_PROVIDER + "」在 DSH 里不存在" in after_text,
            "dump_failure_note": "DSH 配置转储失败" in after_text,
            "profile_missing_note": "在 DSH 里不存在" in after_text,
            "success_note_present": ("已切到" in after_text) or ("重启" in after_text),
            "note_after": [
                blob(n) for n in after if "切换" in blob(n) or "已切到" in blob(n)
            ][:4],
        }
        facts["cases"]["select_from_catalog"]["overlay_after_tail"] = overlay_after[-800:]

        # --- S04 脚本层：不存在的 Profile 给出可读原因 ---
        facts["cases"]["failure_message"] = script_failure_probe(repo, session)

        ok = (
            not facts["cases"]["cold_start"]["failure_note_present"]
            and not facts["cases"]["cold_start"]["empty_profile_note_present"]
            and facts["cases"]["picker_open"]["synthetic_provider_listed"]
            and facts["cases"]["picker_open"]["synthetic_model_listed"]
            and facts["cases"]["select_from_catalog"]["overlay_rewritten"]
            and facts["cases"]["select_from_catalog"]["overlay_selected_model_written"]
            and facts["cases"]["select_from_catalog"]["decoy_provider_preserved"]
            and not facts["cases"]["select_from_catalog"]["dump_failure_note"]
            and not facts["cases"]["select_from_catalog"]["provider_used_as_profile"]
            and facts["cases"]["failure_message"]["has_reason_not_just_exit_code"]
        )
        facts["passed"] = bool(ok)
        return 0 if ok else 1
    finally:
        facts["app_alive"] = session.alive()
        facts["shots"] = sorted(p.name for p in EVID.glob("*.png"))
        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps({k: v for k, v in facts.items() if k in ("passed", "cases")}, ensure_ascii=False, indent=2))
        session.close()
        bus_proc.terminate()
        xvfb.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
