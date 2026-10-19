#!/usr/bin/env python3
"""OD-ARCH-027 Step 5 · 真实桌面回归（用户在用的 Wayland 会话）。

两组**分别记录、不混为一组**：

  * ``--backend wayland``  ：**原生 Wayland**（GDK_BACKEND=wayland，WAYLAND_DISPLAY=用户会话）。
    本机原生 Wayland **没有**可用的截图通道（实测：GNOME Shell Screenshot D-Bus 返回
    AccessDenied、org.gnome.Shell.Eval 返回 false、Gdk 根窗口抓图为全黑），因此该组
    **不产出像素证据**，改用 **AT-SPI** 驱动界面并以**可访问性文本指纹**判定页面切换。
  * ``--backend xwayland`` ：**XWayland**（GDK_BACKEND=x11，DISPLAY=用户会话 :0）。
    窗口在用户桌面上真实显示；像素用 `XGetImage` 读**应用窗口自身**（不是根窗口，
    真实桌面读根窗口是全黑），输入用 XTEST。

两组都只用**一次性隔离数据根**与**回环合成端点**；不写 ~/OmniDesk、不碰真实材料与凭证。
DMABUF 启用/禁用**分别运行、分别记录**。

用法：
  python3 step5-real-desktop.py --label step5-wayland-dmabuf1   --backend wayland  --display wayland-0
  python3 step5-real-desktop.py --label step5-xwayland-dmabuf1 --backend xwayland --display :0
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
OD027 = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027"
STEP5 = OD027 / "step5"
sys.path.insert(0, str(OD027 / "step3" / "scripts"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))

from _step3lib import (  # noqa: E402
    base_env_snapshot,
    frame_signature,
    frame_stats,
    percentile,
    proc_env,
    sample_cpu,
    webkit_process_groups,
    write_json,
)
from window_harness import APP_BIN, AppSession, Geometry, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402

# Dock 按钮的可访问性名称（AT-SPI 可见）→ 场景
SCENE_BUTTONS = {
    "home": "工作台首页",
    "workbench": "任务工作台（合成数据 · 五阶段）",
    "project": "项目跟踪（模型起草）",
    "writing": "文案写作（模型起草）",
    "report": "周报与随记（模型起草）",
    "settings": "成果目录与模型权限",
    "sessions": "会话与任务（只读元数据）",
    "engine": "引擎诊断（只读）",
}
# §5 规定的固定导航路径（每项 ≥5 次）
NAV_PATH = [
    ("workbench", "任务工作台"),
    ("project", "离线流程·项目跟踪"),
    ("settings", "成果目录与模型权限"),
    ("home", "工作台首页"),
]
PAGE_SHOTS = [
    ("home", "首页"), ("workbench", "任务工作台"),
    ("project", "离线流程·项目跟踪"), ("settings", "成果目录与模型权限"),
]


# ---------------------------------------------------------------------------
# AT-SPI：真实桌面上唯一可用的「无像素」交互与状态通道
# ---------------------------------------------------------------------------
class AtspiUi:
    """通过 AT-SPI 找到 OmniDesk 窗口，按可访问性名称点击，并生成文本指纹。"""

    def __init__(self, app_needle: str = "omnidesk") -> None:
        import gi

        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi

        self.Atspi = Atspi
        self.app_needle = app_needle
        self.app = None
        self.last_error = None

    def connect(self, timeout: float = 90.0) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                desktop = self.Atspi.get_desktop(0)
                for i in range(desktop.get_child_count()):
                    node = desktop.get_child_at_index(i)
                    try:
                        name = node.get_name() or ""
                    except Exception:
                        continue
                    if self.app_needle.lower() in name.lower():
                        self.app = node
                        return True
            except Exception as exc:  # noqa: BLE001
                self.last_error = f"{type(exc).__name__}: {exc}"
            time.sleep(1.0)
        return False

    # --- 遍历 ---------------------------------------------------------------
    def walk(self, max_nodes: int = 3000):
        out = []
        stack = [(self.app, 0)]
        while stack and len(out) < max_nodes:
            node, depth = stack.pop()
            try:
                role = node.get_role_name()
                name = node.get_name() or ""
            except Exception:
                continue
            out.append((role, name, node, depth))
            try:
                count = node.get_child_count()
            except Exception:
                continue
            for i in range(count):
                try:
                    stack.append((node.get_child_at_index(i), depth + 1))
                except Exception:
                    pass
        return out

    def find_button(self, name: str):
        for role, nm, node, _d in self.walk():
            if role == "button" and nm == name:
                return node
        for role, nm, node, _d in self.walk():
            if role == "button" and name in nm:
                return node
        return None

    def find_role(self, role: str, name_contains: str = ""):
        for r, nm, node, _d in self.walk():
            if r == role and (not name_contains or name_contains in nm):
                return node
        return None

    def click(self, node) -> bool:
        """用 AT-SPI 动作点击（不需要合成输入，原生 Wayland 也能用）。"""
        if node is None:
            return False
        try:
            action = node.get_action_iface()
        except Exception:
            action = None
        if action is None:
            return False
        try:
            action.do_action(0)
            return True
        except Exception:
            return False

    def text_digest(self) -> str:
        """页面文本指纹：窗口内所有可访问名称的有序摘要。"""
        parts = []
        for role, nm, _node, _d in self.walk():
            if nm and role in {"heading", "button", "text", "entry", "label", "paragraph"}:
                parts.append(role + ":" + nm)
        return hashlib.sha256("\n".join(parts).encode("utf-8")).hexdigest()

    def names(self, limit: int = 60) -> list:
        return [f"{r}:{n}" for r, n, _x, _d in self.walk() if n][:limit]


# ---------------------------------------------------------------------------
# 真实桌面的窗口抓图（XWayland：读应用窗口自身，不读根窗口）
# ---------------------------------------------------------------------------
def capture_window(win, path: Path) -> dict:
    from Xlib import X
    from PIL import Image
    import numpy as np

    g = win.get_geometry()
    raw = win.get_image(0, 0, g.width, g.height, X.ZPixmap, 0xFFFFFFFF)
    arr = np.frombuffer(bytes(raw.data), dtype=np.uint8).reshape(g.height, g.width, 4)
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(arr[..., :3][..., ::-1]).save(str(path))
    luma = arr[..., :3].astype(int)
    return {
        "path": str(path),
        "size": [g.width, g.height],
        "mean_luma": round(float(luma.mean()), 2),
        "max_luma": int(luma.max()),
        "nonblack_fraction": round(float((luma.max(axis=2) > 20).mean()), 4),
    }


class RealXSession(XSession):
    """真实桌面上的 X11 会话：xwininfo 需要 Xauthority（XWayland 会话），并兜底 translate_coords。"""

    def geometry(self, win) -> Geometry:
        import re
        import subprocess

        env = {"DISPLAY": self.display, "PATH": "/usr/bin:/bin"}
        if os.environ.get("XAUTHORITY"):
            env["XAUTHORITY"] = os.environ["XAUTHORITY"]
        out = subprocess.run(
            ["xwininfo", "-id", hex(win.id)], capture_output=True, text=True, env=env
        ).stdout

        def num(key: str):
            m = re.search(key + r":\s*(-?\d+)", out)
            return int(m.group(1)) if m else None

        x, y, w, h = (num("Absolute upper-left X"), num("Absolute upper-left Y"),
                      num("Width"), num("Height"))
        if None not in (x, y, w, h):
            return Geometry(x=x, y=y, w=w, h=h)
        tc = win.translate_coords(self.d.screen().root, 0, 0)
        gw = win.get_geometry()
        return Geometry(x=tc.x, y=tc.y, w=gw.width, h=gw.height)

    def place_inside_root(self, win, margin: int = 40) -> dict:
        """把窗口移到 X 根窗口内的可见位置（真实桌面上窗口可能落在根之外）。"""
        try:
            win.configure(x=margin, y=margin)
            self.d.sync()
            time.sleep(1.2)
            return {"moved_to": [margin, margin]}
        except Exception as exc:  # noqa: BLE001
            return {"move_failed": f"{type(exc).__name__}: {exc}"}


class WaylandAppSession(AppSession):
    """在用户**原生 Wayland 会话**里启动应用：强制 wayland 后端，不回落 X11。"""

    def app_env(self, overlay: Path) -> dict:
        env = dict(os.environ)
        env["GDK_BACKEND"] = "wayland"
        env.pop("DISPLAY", None)          # 强制走 Wayland，不经 XWayland
        env["WAYLAND_DISPLAY"] = self.display
        env.setdefault("XDG_RUNTIME_DIR", "/run/user/%d" % os.getuid())
        env["OMNIDESK_APP_DATA_ROOT"] = str(self.app_data)
        env["OMNIDESK_DSH_HOME"] = str(self.dsh_home)
        env["OMNIDESK_AGENTS_HOME"] = str(self.agents_home)
        env["DSH_HOME"] = str(self.dsh_home)
        env["DSH_AGENTS_HOME"] = str(self.agents_home)
        env["OMNIDESK_DSH_PROFILE"] = "acp"
        env["OMNIDESK_DSH_PATCHES"] = str(overlay)
        env["OMNI_FAKE_KEY"] = "synthetic-placeholder-not-a-credential"
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
        env.update(self.env_extra)
        return env


def session_facts() -> dict:
    """用户会话的显示后端事实与「原生 Wayland 能否抓图」的实测结论。"""
    import subprocess

    facts = {
        "XDG_SESSION_TYPE": os.environ.get("XDG_SESSION_TYPE"),
        "WAYLAND_DISPLAY": os.environ.get("WAYLAND_DISPLAY"),
        "DISPLAY": os.environ.get("DISPLAY"),
        "XDG_RUNTIME_DIR": os.environ.get("XDG_RUNTIME_DIR"),
        "XDG_CURRENT_DESKTOP": os.environ.get("XDG_CURRENT_DESKTOP"),
        "XAUTHORITY_set": bool(os.environ.get("XAUTHORITY")),
    }
    probes = {}
    cmds = {
        "gnome_shell_screenshot_dbus": [
            "gdbus", "call", "--session", "--dest", "org.gnome.Shell.Screenshot",
            "--object-path", "/org/gnome/Shell/Screenshot",
            "--method", "org.gnome.Shell.Screenshot.ScreenshotArea",
            "0", "0", "64", "64", "false", "/tmp/od027-step5-probe.png",
        ],
        "gnome_shell_eval_dbus": [
            "gdbus", "call", "--session", "--dest", "org.gnome.Shell",
            "--object-path", "/org/gnome/Shell",
            "--method", "org.gnome.Shell.Eval", "1+1",
        ],
    }
    for name, cmd in cmds.items():
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
            probes[name] = {
                "exit": r.returncode,
                "stdout": (r.stdout or "").strip()[:200],
                "stderr": (r.stderr or "").strip()[:200],
            }
        except Exception as exc:  # noqa: BLE001
            probes[name] = {"error": f"{type(exc).__name__}: {exc}"}
    facts["wayland_capture_probes"] = probes
    return facts


def nav_latency(ui: AtspiUi, scene: str, repeats: int, timeout: float = 12.0) -> dict:
    """用 AT-SPI 点击 Dock，按文本指纹判定页面切换完成，记录延迟。"""
    samples: list[float] = []
    timeouts = 0
    for _ in range(repeats):
        before = ui.text_digest()
        node = ui.find_button(SCENE_BUTTONS[scene])
        if node is None:
            timeouts += 1
            continue
        t = time.monotonic()
        if not ui.click(node):
            timeouts += 1
            continue
        changed = None
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            time.sleep(0.1)
            if ui.text_digest() != before:
                changed = time.monotonic() - t
                break
        if changed is None:
            timeouts += 1
        else:
            samples.append(round(changed, 3))
    return {
        "n": len(samples),
        "samples_s": samples,
        "p50_s": percentile(samples, 50),
        "p95_s": percentile(samples, 95),
        "timeouts": timeouts,
    }


def _screen_extents(node):
    """取控件的屏幕坐标（AT-SPI 的坐标枚举名在不同版本不同）。"""
    try:
        import gi
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi
    except Exception:
        Atspi = None
    comp = node.get_component_iface()
    for enum_name in ("CoordType", "AtspiCoordType"):
        enum = getattr(Atspi, enum_name, None)
        if enum is None:
            continue
        screen = getattr(enum, "SCREEN", None)
        if screen is not None:
            return comp.get_extents(screen)
    return comp.get_extents(0)


def input_test(ui: AtspiUi, xs, win, settings_file: Path, target: str) -> dict:
    """设置页连续输入：AT-SPI 定位输入框 → XTEST 注入固定串 → AT-SPI 点保存 → 与设置文件比对。"""
    out: dict = {"typed": target}
    fields = [(role, nm, node) for role, nm, node, _d in ui.walk() if role in ("entry", "text")]
    out["editable_fields"] = [f"{r}:{n}" for r, n, _x in fields]
    out["page_buttons"] = [nm for role, nm, _x, _d in ui.walk() if role == "button"][:24]
    entry = None
    # 成果目录输入框的可访问名是它的路径值（以 "/" 开头）；顶栏的全局检索是禁用项，别选它。
    for role, nm, node in fields:
        if nm.startswith("/") or ("目录" in nm) or ("路径" in nm):
            entry = node
            out["chosen_field"] = nm
            break
    if entry is None and fields:
        role, nm, entry = fields[0]
        out["chosen_field"] = nm
    if entry is None:
        out["status"] = "device_failed"
        out["reason"] = "未在设置页找到可编辑输入框"
        return out
    try:
        ext = _screen_extents(entry)
        cx, cy = ext.x + ext.width // 2, ext.y + ext.height // 2
    except Exception as exc:  # noqa: BLE001
        out["status"] = "device_failed"
        out["reason"] = f"取输入框坐标失败：{exc}"
        return out
    out["input_extents"] = [cx, cy]
    xs.focus(win)
    xs.click(cx, cy, settle=0.8)
    xs.select_all_and_clear()
    for ch in target:
        xs.type_ascii(ch, settle=0.02)
    time.sleep(0.6)
    # 主判据：输入框**当前值**（AT-SPI 读文本）比对固定串 → 是否丢字
    value = None
    for getter in (
        lambda: entry.get_text().get_text(0, -1),
        lambda: entry.get_text_iface().get_text(0, -1),
        lambda: entry.get_text(),
    ):
        try:
            got = getter()
        except Exception as exc:  # noqa: BLE001
            out["value_read_error"] = f"{type(exc).__name__}: {exc}"
            continue
        if isinstance(got, str):          # 只接受字符串；绑定不同版本返回值类型不同
            value = got
            break
    out["field_value"] = value
    # 次判据：点「校验并保存」，看设置文件是否落盘（目标必须是合法绝对路径才会被接受）
    save = ui.find_button("保存")
    if save is None:
        for role, nm, node, _d in ui.walk():
            if role == "button" and ("保存" in nm):
                save = node
                break
    if save is None:
        out["status"] = "device_failed"
        out["reason"] = "未找到保存按钮"
        return out
    ui.click(save)
    time.sleep(2.0)
    raw = settings_file.read_text(encoding="utf-8") if settings_file.is_file() else ""
    out["settings_excerpt"] = raw[:300]
    # 主判据：设置文件逐字包含固定串 → 注入过程无丢字
    if target in raw:
        out["status"] = "exact_hit"
    elif len(target) > 6 and target[:6] in raw:
        out["status"] = "partial_prefix_in_file(疑丢字)"
    else:
        out["status"] = "no_match(未落盘/被拒绝)"
    if value is not None:
        out["value_status"] = "exact_hit" if value == target else "mismatch"
    return out


def run(args) -> dict:
    tag = args.label
    root = Path(args.root_base) / tag
    shots = STEP5 / f"shots-{tag}"
    shots.mkdir(parents=True, exist_ok=True)
    payload: dict = {
        "step": "step5", "label": tag, "backend": args.backend, "display": args.display,
        "dmabuf": args.dmabuf, "verdict": "started",
    }
    cls = WaylandAppSession if args.backend == "wayland" else AppSession
    session = cls(
        root=root, display=args.display, artifact_override=True,
        env_extra={"WEBKIT_DISABLE_DMABUF_RENDERER": args.dmabuf},
    )
    session.prepare()
    cfg_dir = session.app_data / "config"
    cfg_dir.mkdir(parents=True, exist_ok=True)
    session.local_data.mkdir(parents=True, exist_ok=True)
    settings = {
        "schema": "omnidesk.settings.v1",
        "artifact_root": str(session.local_data),
        "model_permissions": {"local": True, "private": False, "public": False},
    }
    (cfg_dir / "settings.v1.json").write_text(json.dumps(settings, indent=2), encoding="utf-8")
    payload["preseeded_settings"] = settings
    payload["session_facts"] = session_facts()

    xs = RealXSession(args.display) if args.backend == "xwayland" else None
    try:
        session.start()
        payload["app_pid"] = session.proc.pid

        win = None
        if args.backend == "xwayland":
            win = xs.find_window("OmniDesk", timeout=180)
            if win is None:
                payload["verdict"] = "fail:未找到窗口"
                return payload
            payload["window_move"] = xs.place_inside_root(win)
            geo = xs.geometry(win)
            xs.focus(win)
            payload["window"] = {"title": xs.title(win), "geometry": geo.as_tuple()}
        else:
            ready = False
            for _ in range(120):
                if session.proc.poll() is not None:
                    break
                if webkit_process_groups()["webkit_web_process"]:
                    ready = True
                    break
                time.sleep(1.0)
            payload["webview_ready"] = ready
            if not ready:
                payload["verdict"] = "fail:WebKit 子进程未出现"
                return payload
        time.sleep(args.settle)

        # AT-SPI 连接（两组都用：原生 Wayland 没有其它交互通道）
        ui = AtspiUi()
        if not ui.connect(timeout=90):
            payload["verdict"] = "fail:AT-SPI 未找到应用窗口"
            payload["a11y_error"] = ui.last_error
            return payload
        payload["a11y_sample"] = ui.names(30)

        # 空闲 CPU（停在首页）
        groups = webkit_process_groups()
        groups["app_main"] = [session.proc.pid]
        payload["idle_cpu"] = sample_cpu(groups, args.idle_seconds, curve_every=5.0)
        payload["dmabuf_env"] = {
            "app_main": proc_env(session.proc.pid).get("WEBKIT_DISABLE_DMABUF_RENDERER"),
            "webkit": {
                str(p): proc_env(p).get("WEBKIT_DISABLE_DMABUF_RENDERER")
                for p in groups.get("webkit_web_process", [])
            },
        }
        payload["env"] = base_env_snapshot(args.display, args.dmabuf, {
            "build": "debug + vite dev（前端 dev server，非内嵌产物）",
            "data_root": str(root),
            "artifact_root": str(session.local_data),
            "synthetic_endpoint": session.mock_base_url,
            "model_permissions": "一次性数据根内预置 local=true（合成端点）",
            "display_backend": "原生 Wayland" if args.backend == "wayland" else "XWayland（x11 后端连用户会话 :0）",
            "idle_seconds": args.idle_seconds,
            "note": "真实用户会话；未使用真实材料或凭证；成果只写在一次性隔离根",
        })

        # 导航延迟（AT-SPI 驱动 + 文本指纹）：按**固定循环路径**重复，每次都是一次真实切换
        payload["nav_latency"] = {}
        per_scene: dict[str, list] = {scene: [] for scene, _ in NAV_PATH}
        to_count: dict[str, int] = {scene: 0 for scene, _ in NAV_PATH}
        for _r in range(args.nav_repeats):
            for scene, _label in NAV_PATH:
                res = nav_latency(ui, scene, 1)
                per_scene[scene].extend(res["samples_s"])
                to_count[scene] += res["timeouts"]
        for scene, label in NAV_PATH:
            s = per_scene[scene]
            payload["nav_latency"][label] = {
                "n": len(s), "samples_s": s,
                "p50_s": percentile(s, 50), "p95_s": percentile(s, 95),
                "timeouts": to_count[scene],
            }

        if args.backend == "xwayland":
            payload["pages"] = {}
            for scene, label in PAGE_SHOTS:
                node = ui.find_button(SCENE_BUTTONS[scene])
                ui.click(node)
                time.sleep(1.6)
                p = shots / f"{tag}-page-{scene}.png"
                info = capture_window(win, p)
                payload["pages"][label] = {**info, **frame_stats(p)}
            # 5 帧稳定性
            node = ui.find_button(SCENE_BUTTONS["home"])
            ui.click(node)
            time.sleep(1.5)
            stab = []
            for i in range(5):
                p = shots / f"{tag}-stability-{i}.png"
                capture_window(win, p)
                stab.append({"path": str(p), "sig": frame_signature(p)})
                time.sleep(2.0)
            payload["stability"] = {"frames": stab, "distinct_signatures": len({s["sig"] for s in stab})}
            # 连续输入（设置页）
            node = ui.find_button(SCENE_BUTTONS["settings"])
            ui.click(node)
            time.sleep(1.6)
            target = args.input_text or str(session.local_data)
            payload["input_test"] = input_test(ui, xs, win, cfg_dir / "settings.v1.json", target)
        else:
            payload["pixel_evidence"] = {
                "status": "not_available",
                "reason": "原生 Wayland 无截图通道：GNOME Shell Screenshot D-Bus 被拒、Shell Eval 禁用、"
                          "Gdk 根窗口抓图为全黑（实测见 session_facts.wayland_capture_probes）；"
                          "像素级视觉证据由 XWayland 组单独记录，Xvfb 与真实桌面不混为一组",
            }
            payload["input_test"] = {
                "status": "not_measured",
                "reason": "原生 Wayland 无合成输入通道（无 ydotool/wtype），XTEST 只对 XWayland 生效；"
                          "连续输入校验由 XWayland 组单独记录",
            }
        payload["verdict"] = "ok"
    except Exception as exc:  # noqa: BLE001
        import traceback
        payload["verdict"] = f"error:{type(exc).__name__}:{exc}"
        payload["traceback"] = traceback.format_exc()
    finally:
        try:
            session.stop_all()
        except Exception:
            pass
    return payload


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--backend", default="xwayland", choices=["xwayland", "wayland"])
    ap.add_argument("--display", default=":0")
    ap.add_argument("--dmabuf", default="1", choices=["0", "1"])
    ap.add_argument("--root-base", default="/tmp/od027-step5")
    ap.add_argument("--idle-seconds", type=float, default=60.0)
    ap.add_argument("--nav-repeats", type=int, default=5)
    ap.add_argument("--settle", type=float, default=10.0)
    ap.add_argument("--input-text", default="", help="注入的固定串；留空则用隔离成果目录（合法绝对路径，可被保存接受）")
    args = ap.parse_args(argv[1:])
    p = run(args)
    write_json(STEP5 / f"{args.label}.json", p)
    ic = p.get("idle_cpu", {})
    print(json.dumps({
        "verdict": p.get("verdict"),
        "backend": p.get("backend"),
        "dmabuf": p.get("dmabuf"),
        "webkit_cpu_avg": ic.get("webkit_web_process", {}).get("avg_percent"),
        "app_cpu_avg": ic.get("app_main", {}).get("avg_percent"),
        "nav": {k: {"p50": v.get("p50_s"), "p95": v.get("p95_s"), "to": v.get("timeouts")}
                for k, v in (p.get("nav_latency") or {}).items()},
        "input": (p.get("input_test") or {}).get("status"),
        "stability": (p.get("stability") or {}).get("distinct_signatures"),
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
