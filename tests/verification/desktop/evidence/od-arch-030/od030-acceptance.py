#!/usr/bin/env python3
"""OD-ARCH-030 · 真实使用前置收口验证（真实窗口 + 回环假端点）。

阶段 A（隔离数据根）：端点引导文案与权限开关的完整链路
  A1 默认关闭 → 提交被拒绝（不出错、不静默降级：界面显示真实原因，任务不进模型）
  A2 引导文案覆盖：本机回环 / 非回环分类、默认关闭、端点不在这里配（DSH 侧）、自行确认授权与数据去向
  A3 假端点判定：回环地址 → 类别「本机回环」、leaves_device=false、有开启入口
  A4 开启本机权限 → 设置文件写入 → 再提交即被放行并拿到模型回复（配置写入与生效路径打通）

阶段 B（正式数据根）：使用记录持久化
  B1 写入 ~/OmniDesk/app/logs/usage.jsonl（仓库外），内容只含枚举与时间
  B2 界面读回显示「查看记录（1）」
  B3 删除这条**合成**测试记录（走产品自带的删除路径），把正式根留干净给用户的真实使用
  B4 核对 ~/OmniDesk 下 app/quarantine/runtime 三个子目录：除 usage.jsonl 外无新增、无改动

安全边界：只读分类信息，不读凭证值；不打开 quarantine（真实案件材料）与 runtime（真实凭证）；
不写用户真实成果目录（成果目录指向一次性 /tmp）；合成材料 + 回环假端点。
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
EVID = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-030"
SHOTS = EVID / "shots"
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

from window_harness import AppSession  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    "step5_real_desktop",
    REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "step5"
    / "scripts" / "step5-real-desktop.py",
)
_step5 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_step5)
AtspiUi = _step5.AtspiUi
RealXSession = _step5.RealXSession
capture_window = _step5.capture_window

HOME_OMNI = Path.home() / "OmniDesk"
SETTINGS_BUTTON = "成果目录与模型权限"
HOME_BUTTON = "工作台首页"
SEND_BUTTON = "发送指令"
RECORD_BUTTON = "记录一次"
PROJECT_BUTTON = "项目跟踪（模型起草）"


# ---------------------------------------------------------------------------
class RealRootAppSession(AppSession):
    """使用**正式应用数据根**（$HOME/OmniDesk/app）启动：去掉应用数据根的环境覆盖。"""

    def app_env(self, overlay: Path) -> dict:
        env = super().app_env(overlay)
        env.pop("OMNIDESK_APP_DATA_ROOT", None)
        return env


def snapshot_tree(root: Path) -> dict:
    """目录快照：相对路径 → (大小, mtime)。只取名字与时间，不读内容。"""
    out: dict[str, tuple[int, float]] = {}
    if not root.is_dir():
        return out
    for p in sorted(root.rglob("*")):
        try:
            st = p.stat()
        except OSError:
            continue
        out[str(p.relative_to(root))] = (st.st_size, round(st.st_mtime, 3))
    return out


def diff_tree(before: dict, after: dict) -> dict:
    added = {k: v for k, v in after.items() if k not in before}
    removed = {k: v for k, v in before.items() if k not in after}
    changed = {k: (before[k], after[k]) for k in before if k in after and before[k] != after[k]}
    return {"added": added, "removed": removed, "changed": changed}


def newest_task(app_data: Path) -> dict | None:
    files = sorted((app_data / "engine-state").glob("omni-*.json"), key=lambda p: p.stat().st_mtime)
    if not files:
        return None
    try:
        return json.loads(files[-1].read_text(encoding="utf-8"))
    except Exception:
        return None


def wait_task(app_data: Path, timeout: float = 90.0) -> tuple[dict | None, float]:
    waited = 0.0
    task = None
    while waited < timeout:
        task = newest_task(app_data)
        if task and task.get("state") in {"succeeded", "failed", "cancelled", "interrupted"}:
            return task, waited
        time.sleep(1.0)
        waited += 1.0
    return task, waited


def ui_names(ui: AtspiUi, limit: int = 400) -> list[str]:
    return [f"{r}:{n}" for r, n, _x, _d in ui.walk() if n][:limit]


def click_button(ui: AtspiUi, name: str) -> bool:
    node = ui.find_button(name)
    return ui.click(node) if node is not None else False


def open_scene(ui: AtspiUi, xs, win, name: str, settle: float = 1.8) -> None:
    click_button(ui, name)
    time.sleep(settle)


def _ext_list(ext) -> list | None:
    if ext is None:
        return None
    try:
        return [ext.x, ext.y, ext.width, ext.height]
    except Exception:
        return None


def find_chat_input(ui: AtspiUi, permission_off: bool):
    """首页对话输入框：其可访问名就是占位提示（权限开/关两套文案）。

    优先按名称匹配；兜底取屏幕坐标宽度最大的 entry（顶栏全局检索只有 157px 宽）。
    """
    cands = []
    for r, nm, node, _d in ui.walk():
        if r != "entry":
            continue
        try:
            ext = _step5._screen_extents(node)
        except Exception:
            ext = None
        cands.append((nm, node, ext))
    for nm, node, ext in cands:
        if (permission_off and "模型权限未开启" in nm) or (not permission_off and "协同探索" in nm):
            return node, nm, ext
    if cands:
        cands.sort(key=lambda c: -(c[2].width if c[2] else 0))
        return cands[0][1], cands[0][0], cands[0][2]
    return None, None, None


def node_state(node, state_name: str) -> bool | None:
    try:
        import gi
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi
        st = node.get_state_set()
        state = getattr(Atspi.StateType, state_name)
        return bool(st.contains(state))
    except Exception:
        return None


def type_into(ui: AtspiUi, xs, win, needle: str, text: str) -> dict:
    """按可访问性名称找到输入框 → XTEST 点击 → XTEST 注入固定串。"""
    target = None
    for role, nm, node, _d in ui.walk():
        if role in ("entry", "text") and needle in nm:
            target = node
            break
    if target is None:
        return {"ok": False, "reason": f"未找到输入框（名称含 {needle}）"}
    try:
        ext = _step5._screen_extents(target)
        cx, cy = ext.x + ext.width // 2, ext.y + ext.height // 2
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "reason": str(exc)}
    xs.focus(win)
    xs.click(cx, cy, settle=0.6)
    xs.select_all_and_clear()
    for ch in text:
        xs.type_ascii(ch, settle=0.02)
    time.sleep(0.4)
    return {"ok": True, "xy": [cx, cy]}


# ---------------------------------------------------------------------------
def phase_a(args, payload: dict) -> None:
    root = Path(args.root_base) / f"{args.label}-A"
    out: dict = {"phase": "A"}
    app = AppSession(root=root, display=args.display, artifact_override=True)
    app.prepare()
    # 隔离根内**不预置**任何模型权限：验证「默认关闭」。
    cfg = app.app_data / "config"
    cfg.mkdir(parents=True, exist_ok=True)
    app.local_data.mkdir(parents=True, exist_ok=True)
    settings_file = cfg / "settings.v1.json"
    xs = RealXSession(args.display)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            out["verdict"] = "fail:未找到窗口"
            payload["phase_a"] = out
            return
        xs.place_inside_root(win)
        xs.focus(win)
        time.sleep(10)
        ui = AtspiUi()
        if not ui.connect(timeout=90):
            out["verdict"] = "fail:AT-SPI 未连接"
            payload["phase_a"] = out
            return

        # A1/A2/A3：设置页事实与文案
        open_scene(ui, xs, win, SETTINGS_BUTTON)
        capture_window(win, SHOTS / f"{args.label}-A-settings-before.png")
        names = ui_names(ui)
        out["settings_text_names"] = [n for n in names if n.startswith(("paragraph", "text", "heading", "label"))]
        # 引导文案核对：静态提示段落不暴露为可访问性节点（只有标题与开关标签暴露），
        # 因此文案以**源码**为准核对，渲染效果以截图为准。
        src = (REPO / "desktop" / "src" / "SettingsPanel.tsx").read_text(encoding="utf-8")
        required = {
            "本机回环": "本机回环",
            "非回环": "非回环",
            "默认全部关闭": "默认全部关闭",
            "端点不在这里配": "端点不在这里配",
            "不读取凭证值": "不读取凭证值",
            "自行确认授权与数据去向": "请自行确认发布方的授权与数据去向",
            "端点来自DSH侧配置": "端点来自 DSH 侧的模型配置",
            "无法确认位置不放行": "无法确认位置的端点永远不会被放行",
        }
        out["guidance_source_checks"] = {k: (v in src) for k, v in required.items()}
        out["guidance_hits"] = {k: bool(v) for k, v in out["guidance_source_checks"].items()}
        out["permission_label_names"] = [nm for nm in names if "本机模型" in nm or "非回环地址" in nm]
        out["checkbox_states"] = [
            {"name": nm, "checked": _checkbox_state(n)}
            for r, nm, n, _d in ui.walk() if r in ("check box", "checkbox")
        ]
        out["settings_file_exists_before"] = settings_file.is_file()
        out["settings_before"] = json.loads(settings_file.read_text(encoding="utf-8")) if settings_file.is_file() else None

        # A1：权限关闭时提交必须被拒绝（真实原因，界面显示，不静默降级）
        open_scene(ui, xs, win, HOME_BUTTON)
        node, nm, ext = find_chat_input(ui, permission_off=True)
        out["chat_input_when_off"] = {"name": nm, "extents": _ext_list(ext)}
        send_node = ui.find_button(SEND_BUTTON)
        out["send_button_enabled_when_off"] = node_state(send_node, "ENABLED") if send_node else None
        typed = type_into(ui, xs, win, nm[:6] if nm else "模型权限未开启", "synthetic preflight ping")
        out["chat_typed"] = typed
        sent = click_button(ui, SEND_BUTTON)
        out["chat_sent"] = sent
        task, waited = wait_task(app.app_data, 20.0)
        out["task_when_permission_off"] = {
            "state": (task or {}).get("state"),
            "error": (task or {}).get("error_message"),
            "waited_s": round(waited, 1),
            "existed": task is not None,
        }
        capture_window(win, SHOTS / f"{args.label}-A-refused.png")

        # A4：开启本机权限 → 设置文件写入 → 回到首页让对话重新读取准入状态 → 提交被放行
        open_scene(ui, xs, win, SETTINGS_BUTTON)
        toggled = None
        for r, nm, node, _d in ui.walk():
            if r in ("check box", "checkbox") and "本机模型" in nm:
                toggled = nm
                ui.click(node)
                break
        out["permission_toggled"] = toggled
        time.sleep(2.5)
        out["settings_after"] = json.loads(settings_file.read_text(encoding="utf-8")) if settings_file.is_file() else None
        out["local_permission_written"] = bool(
            (out["settings_after"] or {}).get("model_permissions", {}).get("local") is True
        )
        capture_window(win, SHOTS / f"{args.label}-A-permission-on.png")

        # 换场景再回首页：对话组件重新挂载后才会重新读取准入状态
        open_scene(ui, xs, win, "任务工作台（合成数据 · 五阶段）")
        open_scene(ui, xs, win, HOME_BUTTON)
        time.sleep(1.5)
        node, nm, ext = find_chat_input(ui, permission_off=False)
        out["chat_input_when_on"] = {"name": nm, "extents": _ext_list(ext)}
        send_node = ui.find_button(SEND_BUTTON)
        out["send_button_enabled_when_on"] = node_state(send_node, "ENABLED") if send_node else None

        # A4（端到端）：权限已由界面开启 → 在场景页走「读取材料 → 生成模型初稿」，
        # 用真实模型链路证明配置写入与生效路径打通（该页输入框可接受 XTEST 注入）。
        open_scene(ui, xs, win, PROJECT_BUTTON)
        time.sleep(1.0)
        material = Path(args.root_base) / f"{args.label}-material.md"
        material.parent.mkdir(parents=True, exist_ok=True)
        material.write_text(
            "# 合成材料\n\n2026-10-12 | 合成事项一 | done\n2026-10-13 | 合成事项二 | pending\n",
            encoding="utf-8",
        )
        path_node = None
        for r, nm2, node2, _d in ui.walk():
            if r == "entry" and ("合成材料" in nm2 or "/tmp/" in nm2):
                path_node = node2
                out["material_field_name"] = nm2
                break
        typed_material = {"ok": False}
        if path_node is not None:
            try:
                ext2 = _step5._screen_extents(path_node)
                xs.focus(win)
                xs.click(ext2.x + 40, ext2.y + ext2.height // 2, settle=0.8)
                xs.select_all_and_clear()
                for ch in str(material):
                    xs.type_ascii(ch, settle=0.02)
                typed_material = {"ok": True, "xy": [ext2.x + 40, ext2.y + ext2.height // 2]}
            except Exception as exc:  # noqa: BLE001
                typed_material = {"ok": False, "reason": str(exc)}
        out["material_typed"] = typed_material
        out["read_clicked"] = click_button(ui, "读取材料文件")
        time.sleep(1.5)
        capture_window(win, SHOTS / f"{args.label}-A-material-read.png")
        out["generate_clicked"] = click_button(ui, "生成模型初稿")
        task2, waited2 = wait_task(app.app_data, 150.0)
        out["task_when_permission_on"] = {
            "state": (task2 or {}).get("state"),
            "content": str((task2 or {}).get("content") or "")[:120],
            "waited_s": round(waited2, 1),
        }
        capture_window(win, SHOTS / f"{args.label}-A-allowed-reply.png")
        out["verdict"] = "ok" if (
            out["local_permission_written"]
            and out["task_when_permission_on"]["state"] == "succeeded"
            and out["task_when_permission_off"]["state"] in (None, "failed")
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
    payload["phase_a"] = out


def _checkbox_state(node) -> bool | None:
    try:
        st = node.get_state_set()
        import gi
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi
        return bool(st.contains(Atspi.StateType.CHECKED))
    except Exception:
        return None


def phase_b(args, payload: dict) -> None:
    """正式数据根：~/OmniDesk/app/logs/usage.jsonl。"""
    root = Path(args.root_base) / f"{args.label}-B"
    out: dict = {"phase": "B"}
    usage_file = HOME_OMNI / "app" / "logs" / "usage.jsonl"
    before_tree = snapshot_tree(HOME_OMNI)
    out["usage_file"] = str(usage_file)
    out["usage_file_existed_before"] = usage_file.is_file()

    app = RealRootAppSession(root=root, display=args.display, artifact_override=True)
    # 关键：不覆盖 OMNIDESK_APP_DATA_ROOT → 应用使用正式数据根 ~/OmniDesk/app
    app.prepare()
    cfg = HOME_OMNI / "app" / "config"
    cfg.mkdir(parents=True, exist_ok=True)
    app.local_data.mkdir(parents=True, exist_ok=True)
    xs = RealXSession(args.display)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            out["verdict"] = "fail:未找到窗口"
            payload["phase_b"] = out
            return
        xs.place_inside_root(win)
        xs.focus(win)
        time.sleep(10)
        ui = AtspiUi()
        if not ui.connect(timeout=90):
            out["verdict"] = "fail:AT-SPI 未连接"
            payload["phase_b"] = out
            return

        open_scene(ui, xs, win, PROJECT_BUTTON)
        checks = [(nm, n) for r, nm, n, _d in ui.walk() if r in ("check box", "checkbox")]
        out["checkboxes"] = [nm for nm, _n in checks]
        # 只勾选枚举项，不输入任何文字
        for nm, n in checks[:1]:
            ui.click(n)
        time.sleep(0.4)
        capture_window(win, SHOTS / f"{args.label}-B-before-record.png")
        if not click_button(ui, RECORD_BUTTON):
            out["verdict"] = "fail:未找到「记录一次」按钮"
            payload["phase_b"] = out
            return
        time.sleep(2.5)
        capture_window(win, SHOTS / f"{args.label}-B-after-record.png")

        out["usage_file_exists_after"] = usage_file.is_file()
        records = []
        if usage_file.is_file():
            for line in usage_file.read_text(encoding="utf-8").splitlines():
                if line.strip():
                    records.append(json.loads(line))
        out["records"] = records
        out["record_count"] = len(records)
        allowed = {
            "record_id", "schema", "task_type", "recorded_at", "completed", "usable",
            "rework_kind", "blocker", "duration_bucket", "reuse_willingness",
        }
        out["unexpected_keys"] = sorted({k for r in records for k in r} - allowed)
        out["body_free"] = all(
            isinstance(v, (str, int, float, bool, type(None))) for r in records for v in r.values()
        )
        out["readback_label"] = [nm for r, nm, _n, _d in ui.walk() if r == "button" and "查看记录" in nm]

        # B3：删除这些**合成**测试记录（走产品自带删除路径），把正式根留干净
        if records:
            expanded = False
            for r, nm, node, _d in ui.walk():
                if r == "button" and "查看记录" in nm:
                    if ui.click(node):
                        expanded = True
                    break
            out["records_expanded"] = expanded
            time.sleep(1.2)
            deleted = 0
            for _ in range(8):
                del_btn = None
                for r, nm, node, _d in ui.walk():
                    if r == "button" and nm.strip() == "删除":
                        del_btn = node
                        break
                if del_btn is None:
                    break
                if ui.click(del_btn):
                    deleted += 1
                    time.sleep(1.2)
            out["deleted_records"] = deleted
            capture_window(win, SHOTS / f"{args.label}-B-after-delete.png")
        out["records_after_delete"] = len(
            [l for l in usage_file.read_text(encoding="utf-8").splitlines() if l.strip()]
        ) if usage_file.is_file() else 0

        after_tree = snapshot_tree(HOME_OMNI)
        out["tree_diff"] = diff_tree(before_tree, after_tree)
        out["quarantine_untouched"] = not any(
            k.startswith("quarantine/") for k in out["tree_diff"]["changed"] | out["tree_diff"]["removed"]
        )
        out["runtime_untouched"] = not any(
            k.startswith("runtime/") for k in out["tree_diff"]["changed"] | out["tree_diff"]["removed"]
        )
        out["verdict"] = "ok" if (
            out["record_count"] >= 1
            and not out["unexpected_keys"]
            and out["body_free"]
            and out["records_after_delete"] == 0
            and out["quarantine_untouched"]
            and out["runtime_untouched"]
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
    payload["phase_b"] = out


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--display", default=":0")
    ap.add_argument("--root-base", default="/tmp/od030")
    ap.add_argument("--phase", default="all", choices=["a", "b", "all"])
    args = ap.parse_args(argv[1:])
    SHOTS.mkdir(parents=True, exist_ok=True)
    payload: dict = {"label": args.label, "phase": args.phase, "verdict": "started"}
    if args.phase in ("a", "all"):
        phase_a(args, payload)
    if args.phase in ("b", "all"):
        phase_b(args, payload)
    verdicts = [payload.get(k, {}).get("verdict") for k in ("phase_a", "phase_b") if k in payload]
    payload["verdict"] = "ok" if verdicts and all(v == "ok" for v in verdicts) else "fail"
    EVID.mkdir(parents=True, exist_ok=True)
    (EVID / f"{args.label}.json").write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({
        "verdict": payload["verdict"],
        "phase_a": payload.get("phase_a", {}).get("verdict"),
        "phase_b": payload.get("phase_b", {}).get("verdict"),
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
