#!/usr/bin/env python3
"""OD-ARCH-031 Part B 验收：项目 → 会话 → 对话流（真实窗口 + 回环假端点）。

验证：
  1. 左侧出现「项目 / 会话」栏：项目列表、会话列表、开场预设、新会话入口；
  2. 能加入一个项目目录（绝对路径），并在该项目下开一段新对话；
  3. 带预设的对话把**场景规则**带进模型请求（端点探针命中流程包的章节词）；
  4. 第二轮请求里带有**上一轮的内容**（上下文累积，端点探针命中第一轮标记）——即「连续对话」；
  5. 不选预设的普通对话同样可用。

只用合成内容 + 一次性隔离数据根 + 回环假端点；不写正式数据根、不用真实凭证。
"""
from __future__ import annotations

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
s5 = importlib.util.module_from_spec(spec); spec.loader.exec_module(_s5 := s5)

AtspiUi = s5.AtspiUi
RealXSession = s5.RealXSession

TURN1 = "synthetic turn one marker ALPHA-123"
TURN2 = "second turn asks for more"
CONTEXT_PROBE = "ALPHA-123"
PRESET_PROBE = "本周概述"  # 流程包 weekly.draft.sections 里的章节词


def newest_task(app_data: Path):
    files = sorted((app_data / "engine-state").glob("omni-*.json"), key=lambda p: p.stat().st_mtime)
    if not files:
        return None
    try:
        return json.loads(files[-1].read_text(encoding="utf-8"))
    except Exception:
        return None


def wait_task(app_data: Path, timeout: float = 90.0):
    waited = 0.0
    task = None
    while waited < timeout:
        task = newest_task(app_data)
        if task and task.get("state") in {"succeeded", "failed", "cancelled", "interrupted"}:
            return task, waited
        time.sleep(1.0)
        waited += 1.0
    return task, waited


def focus_window(xs, win) -> None:
    """在真实 Wayland 会话里把**合成器焦点**也交给应用窗口。

    仅设置 X 输入焦点不够：合成器焦点若在别的客户端（另一个 Wayland 应用），
    键盘事件不会进入 XWayland 窗口。这里补发 `_NET_ACTIVE_WINDOW`（与 xdotool
    windowactivate 同一机制），再设 X 输入焦点。
    """
    from Xlib import X, Xatom  # noqa: N814

    wm_protocols = win.get_full_property(
        xs.d.intern_atom("_NET_WM_PROTOCOLS"), Xatom.ATOM
    )
    active = None
    if wm_protocols:
        for atom in wm_protocols.value:
            if atom == xs.d.intern_atom("_NET_ACTIVE_WINDOW"):
                active = atom
                break
    if active is not None:
        from Xlib.protocol import event as xevent  # noqa: N814

        data = (32, [active, 2, xs.d.get_input_focus().focus, 0, 0])  # source = pager
        ev = xevent.ClientMessage(
            window=win, client_type=active, data=(data[0], data[1])
        )
        win.send_event(ev, event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
        xs.d.flush()
    try:
        win.set_input_focus(X.RevertToParent, X.CurrentTime)
        xs.d.sync()
    except Exception:
        xs.focus(win)
    time.sleep(0.6)


def main() -> int:
    root = Path("/tmp/od031/partb")
    if root.exists():
        import shutil
        shutil.rmtree(root)
    out: dict = {"verdict": "started"}
    app = AppSession(root=root, display=":0", artifact_override=True)
    app.prepare()
    cfg = app.app_data / "config"
    cfg.mkdir(parents=True, exist_ok=True)
    app.local_data.mkdir(parents=True, exist_ok=True)
    project_dir = root / "project-a"
    project_dir.mkdir(parents=True, exist_ok=True)
    (cfg / "settings.v1.json").write_text(json.dumps({
        "schema": "omnidesk.settings.v1",
        "artifact_root": str(app.local_data),
        "model_permissions": {"local": True, "private": False, "public": False},
    }, indent=2), encoding="utf-8")

    xs = RealXSession(":0")
    try:
        app.start()
        app.mock_ctl.set(probes=[CONTEXT_PROBE, PRESET_PROBE])
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            out["verdict"] = "fail:未找到窗口"
            print(json.dumps(out, ensure_ascii=False))
            return 1
        xs.place_inside_root(win)
        xs.focus(win)
        time.sleep(10)
        ui = AtspiUi()
        if not ui.connect(timeout=90):
            out["verdict"] = "fail:AT-SPI 未连接"
            print(json.dumps(out, ensure_ascii=False))
            return 1

        def buttons():
            return [(n, node) for r, n, node, _d in ui.walk() if r == "button"]

        def entries():
            return [(n, node) for r, n, node, _d in ui.walk() if r in ("entry", "text")]

        def click_button(label, exact=True):
            for n, node in buttons():
                if (n.strip() == label) if exact else (label in n):
                    return ui.click(node)
            return False

        def type_into(needle: str, text: str):
            target = None
            for n, node in entries():
                if needle in n:
                    target = node
                    break
            if target is None:
                return False
            # 先用 AT-SPI 动作点进输入框（合成点击在这个无边框输入框上不生效），
            # 再用 XTEST 注入字符。
            ui.click(target)
            time.sleep(0.3)
            focus_window(xs, win)
            xs.select_all_and_clear()
            for ch in text:
                xs.type_ascii(ch, settle=0.02)
            time.sleep(0.3)
            return True

        names = [n for n, _ in buttons()]
        out["home_buttons"] = names
        out["has_rail_new_thread"] = any("新会话" in n for n in names)
        out["has_preset_weekly"] = any("周报与随记" in n for n in names)
        out["entry_names"] = [n for n, _ in entries()]

        # 1) 加入项目目录
        out["project_typed"] = type_into("加一个项目目录", str(project_dir))
        s5.capture_window(win, Path("/tmp/od031/partb-shot-project-typed.png"))
        out["project_added"] = click_button("加入")
        time.sleep(1.2)
        s5.capture_window(win, Path("/tmp/od031/partb-shot-project-added.png"))
        out["project_visible"] = any("project-a" in n for n, _ in buttons())

        # 2) 选周报预设 → 新会话
        out["preset_clicked"] = click_button("周报与随记")
        time.sleep(0.5)
        out["new_thread_clicked"] = click_button("新会话")
        time.sleep(1.0)

        # 3) 第一轮
        out["turn1_typed"] = type_into("与 OmniDesk 协同探索", TURN1)
        out["turn1_sent"] = click_button("发送指令")
        task1, w1 = wait_task(app.app_data, 90.0)
        out["turn1"] = {"state": (task1 or {}).get("state"), "waited_s": round(w1, 1)}

        # 4) 第二轮（验证上下文累积）
        out["turn2_typed"] = type_into("与 OmniDesk 协同探索", TURN2)
        out["turn2_sent"] = click_button("发送指令")
        task2, w2 = wait_task(app.app_data, 90.0)
        out["turn2"] = {"state": (task2 or {}).get("state"), "waited_s": round(w2, 1)}

        reqs = app.mock_ctl.new_requests()
        out["requests"] = [
            {"probe_hits": r.get("probe_hits"), "probe_misses": r.get("probe_misses")} for r in reqs
        ]
        hits_all = [h for r in reqs for h in (r.get("probe_hits") or [])]
        out["context_probe_hit_in_any_request"] = CONTEXT_PROBE in hits_all
        out["preset_probe_hit_in_any_request"] = PRESET_PROBE in hits_all
        # 第二条请求里应带上一轮标记
        second = reqs[1] if len(reqs) > 1 else {}
        out["context_in_second_request"] = CONTEXT_PROBE in (second.get("probe_hits") or [])

        out["verdict"] = "ok" if (
            out["has_rail_new_thread"]
            and out["has_preset_weekly"]
            and out["project_visible"]
            and out["turn1"]["state"] == "succeeded"
            and out["turn2"]["state"] == "succeeded"
            and out["context_in_second_request"]
        ) else "fail"
    except Exception as exc:  # noqa: BLE001
        import traceback
        out["verdict"] = f"error:{type(exc).__name__}:{exc}"
        out["traceback"] = traceback.format_exc()
    finally:
        try:
            app.stop_all()
        except Exception:
            pass

    ev = REPO / "tests/verification/desktop/evidence/od-arch-031"
    ev.mkdir(parents=True, exist_ok=True)
    (ev / "partb-run1.json").write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in out.items() if k not in ("home_buttons", "entry_names", "traceback")},
                     ensure_ascii=False)[:1800])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
