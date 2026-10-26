#!/usr/bin/env python3
"""OD-ARCH-025 · 真实桌面窗口驱动装置（一次性 Xvfb 显示 + XTEST 注入 + GDK 抓图）。

用途：在**真实 Tauri 桌面窗口**里完成合成验收（不是同进程调用、也不是只跑脚本）。

为什么需要它：本机没有 `xdotool` / `import` / `scrot`，X 侧只有 `xwininfo` / `xprop` / `xrandr`，
图形侧可用 PyGObject（Gdk 抓图）与 python-xlib（XTEST 注入）。本文件把这两条能力封成最小 API。

安全边界：
  * 只在本脚本创建的一次性 Xvfb 显示上操作，不触碰用户正在使用的桌面会话；
  * 数据根固定为仓库外的一次性目录；不读凭证、不接真实端点、不用真实材料；
  * 进程回收按本次记录的「PID + 启动时刻 + 命令行」核对身份，不误杀其它实例。

默认**不**设置 `OMNIDESK_LOCAL_DATA_ROOT`：该变量会覆盖「用户设置的成果目录」（见
desktop/src-tauri/src/settings.rs 的优先级），而 OD-ARCH-025 要验证的正是用户设置是否真正生效。
"""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parent.parents[2]
DESKTOP = REPO / "desktop"
APP_BIN = DESKTOP / "src-tauri" / "target" / "debug" / "omnidesk-desktop"
DEV_PORT = 5273
MOCK_HELPER = REPO / "tests" / "verification" / "engine-interface" / "adapter"
FAKE_KEY_ENV = "OMNI_FAKE_KEY"
FAKE_KEY_VALUE = "synthetic-placeholder-not-a-credential"

sys.path.insert(0, str(MOCK_HELPER))


# ---------------------------------------------------------------------------
# X 侧：窗口定位、抓图、XTEST 注入
# ---------------------------------------------------------------------------


@dataclass
class Geometry:
    x: int
    y: int
    w: int
    h: int

    def as_tuple(self) -> tuple[int, int, int, int]:
        return (self.x, self.y, self.w, self.h)


class XSession:
    """一次性显示上的 X 操作（抓图必须与注入用同一个 DISPLAY）。"""

    def __init__(self, display: str) -> None:
        self.display = display
        # 本机是 Wayland 会话（环境里 GDK_BACKEND=wayland、WAYLAND_DISPLAY=wayland-0）。
        # 不强制 X11 的话，GTK/WebKit 会优先连用户的 Wayland 会话，而不是本次的一次性
        # Xvfb 显示——那既破坏隔离，也可能干扰用户正在使用的桌面。实测确认过该行为。
        os.environ["DISPLAY"] = display
        os.environ["GDK_BACKEND"] = "x11"
        os.environ.pop("WAYLAND_DISPLAY", None)
        from Xlib import X, XK, display as xdisplay
        from Xlib.ext import xtest

        self._X = X
        self._XK = XK
        self._xtest = xtest
        self.d = xdisplay.Display(display)

    # --- 窗口 ---------------------------------------------------------------
    def find_window(self, title_substr: str = "OmniDesk", timeout: float = 60.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            found = self._search(self.d.screen().root, title_substr)
            if found is not None:
                return found
            time.sleep(0.3)
        return None

    def _search(self, win, substr: str):
        try:
            name = win.get_wm_name()
        except Exception:
            name = None
        if name and substr in name:
            try:
                if win.get_attributes().map_state == self._X.IsViewable:
                    return win
            except Exception:
                pass
        try:
            children = win.query_tree().children
        except Exception:
            return None
        for child in children:
            found = self._search(child, substr)
            if found is not None:
                return found
        return None

    def geometry(self, win) -> Geometry:
        """窗口在根窗口中的绝对几何（用 xwininfo，避免 translate_coords 的符号歧义）。"""
        import re

        out = subprocess.run(
            ["xwininfo", "-id", hex(win.id)],
            capture_output=True,
            text=True,
            env={"DISPLAY": self.display, "PATH": "/usr/bin:/bin"},
        ).stdout

        def num(key: str) -> int:
            m = re.search(key + r":\s*(-?\d+)", out)
            if not m:
                raise RuntimeError(f"xwininfo 未给出 {key}")
            return int(m.group(1))

        return Geometry(
            x=num("Absolute upper-left X"),
            y=num("Absolute upper-left Y"),
            w=num("Width"),
            h=num("Height"),
        )

    def title(self, win) -> str:
        try:
            return win.get_wm_name() or ""
        except Exception:
            return ""

    def focus(self, win) -> None:
        """显式把 X 输入焦点给应用窗口。

        本机 Xvfb 上**没有窗口管理器**，因而没有 WM 发送 FocusIn；GTK 会认为顶层窗口
        未获得焦点并丢弃键盘事件（鼠标点击仍可用）。实测：不设置焦点时按钮点击可用、
        但输入框打字与部分按钮无反应。这里显式设置，等价于最小窗口管理器的行为。
        """
        self.d.set_input_focus(win, self._X.RevertToPointerRoot, self._X.CurrentTime)
        self.d.sync()
        time.sleep(0.2)

    def focused_window(self):
        return self.d.get_input_focus().focus

    def dismiss_overlays(self) -> int:
        """回收 GTK 的原生 tooltip 等 override-redirect 覆盖窗口。

        无窗口管理器时这些弹窗不会自动消失，会**盖住界面并吃掉点击**（实测踩到过）。
        """
        root = self.d.screen().root
        count = 0
        try:
            children = root.query_tree().children
        except Exception:
            return 0
        for w in children:
            try:
                attrs = w.get_attributes()
            except Exception:
                continue
            if attrs.override_redirect and attrs.map_state == self._X.IsViewable:
                try:
                    w.unmap()
                    count += 1
                except Exception:
                    pass
        if count:
            self.d.sync()
            time.sleep(0.2)
        return count

    # --- 抓图 ---------------------------------------------------------------
    def shot(self, path: Path, region: Geometry | None = None) -> Geometry:
        """抓图（连续两帧一致才算稳定）。

        实测：软件渲染 + 合成层下，`pixbuf_get_from_window` 偶尔会抓到**半更新**的帧
        （不同区域来自不同次渲染），直接读数会误判界面状态。这里重复抓取直到两帧字节
        完全相同，最多 6 次。
        """
        import gi

        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk

        root = Gdk.get_default_root_window()
        if region is None:
            region = Geometry(0, 0, root.get_width(), root.get_height())
        path.parent.mkdir(parents=True, exist_ok=True)
        prev = None
        for _ in range(6):
            pb = Gdk.pixbuf_get_from_window(root, region.x, region.y, region.w, region.h)
            if pb is None:
                raise RuntimeError("抓图失败：pixbuf 为空")
            pb.savev(str(path), "png", [], [])
            data = path.read_bytes()
            if prev is not None and prev == data:
                return region
            prev = data
            time.sleep(0.35)
        return region

    def resize(self, win, w: int, h: int, settle: float = 2.0) -> None:
        """调整窗口尺寸（同一二进制；用于一次会话内看到完整页面，减少滚动）。"""
        win.configure(width=w, height=h)
        self.d.sync()
        time.sleep(settle)

    def wait_rendered(self, win, timeout: float = 90.0, min_colors: int = 12) -> bool:
        """等首屏真正画出内容，返回是否等到。

        为什么需要它：本仓库根的 `desktop/src-tauri/target` 有几十万文件，Vite 冷启动
        在开发态要 8 秒上下（实测 HEAD 与当前工作区一致，属环境因素，不是某次改动）。
        固定 sleep 会在 WebKit 还只有一层纯色底的时候抓图、读文件，于是出现「空白截图」
        和「声明文件还没被创建」这类假失败。这里按**像素多样性**判断渲染完成：空白
        WebKit 窗口只有一种底色，渲染完才有文字与卡片的多色内容。

        这只放宽「什么时候看」，不放宽断言本身：超时仍返回 False，由调用方失败。
        """
        import gi

        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            geo = self.geometry(win)
            root = Gdk.get_default_root_window()
            pb = Gdk.pixbuf_get_from_window(root, geo.x, geo.y, geo.w, geo.h)
            if pb is not None:
                pixels = pb.get_pixels()
                channels = pb.get_n_channels()
                rowstride = pb.get_rowstride()
                colors = set()
                # 每 8 像素采一个点即可区分「纯色底」与「有内容」。
                for y in range(0, pb.get_height(), 8):
                    row = y * rowstride
                    for x in range(0, pb.get_width(), 8):
                        off = row + x * channels
                        colors.add(pixels[off:off + 3])
                        if len(colors) > min_colors:
                            break
                    if len(colors) > min_colors:
                        break
                if len(colors) > min_colors:
                    time.sleep(0.6)
                    return True
            time.sleep(0.4)
        return False

    def place(self, win, x: int, y: int, w: int, h: int, settle: float = 2.0) -> None:
        """同时设置窗口位置与尺寸（无窗口管理器时需自行摆放）。"""
        win.configure(x=x, y=y, width=w, height=h)
        self.d.sync()
        time.sleep(settle)

    def close_window(self, win) -> None:
        """按窗口管理器的正常路径请求关闭窗口（WM_DELETE_WINDOW），等价于点标题栏关闭。

        这样走的才是应用真实的「窗口销毁」路径（Tauri 的 Destroyed 回调），
        而不是直接杀进程。
        """
        from Xlib import protocol

        wm_protocols = self.d.intern_atom("WM_PROTOCOLS")
        wm_delete = self.d.intern_atom("WM_DELETE_WINDOW")
        ev = protocol.event.ClientMessage(
            window=win,
            client_type=wm_protocols,
            data=(32, [wm_delete, self._X.CurrentTime, 0, 0, 0]),
        )
        win.send_event(ev, event_mask=self._X.NoEventMask)
        self.d.flush()

    def window_count(self) -> int:
        try:
            return len(self.d.screen().root.query_tree().children)
        except Exception:
            return -1

    # --- 注入 ---------------------------------------------------------------
    def move(self, x: int, y: int) -> None:
        self._xtest.fake_input(self.d, self._X.MotionNotify, x=int(x), y=int(y))
        self.d.sync()

    def click(self, x: int, y: int, button: int = 1, settle: float = 0.6) -> None:
        self._sweep_to(x, y)
        time.sleep(0.15)
        self._xtest.fake_input(self.d, self._X.ButtonPress, button)
        self.d.sync()
        time.sleep(0.08)
        self._xtest.fake_input(self.d, self._X.ButtonRelease, button)
        self.d.sync()
        time.sleep(settle)

    def _sweep_to(self, x: int, y: int, steps: int = 8) -> None:
        """分步移动指针再落点：单次跳转不会产生悬停/交叉事件，滚轮会被忽略。"""
        try:
            cur = self.d.screen().root.query_pointer()
            sx, sy = cur.root_x, cur.root_y
        except Exception:
            sx, sy = x, y
        for i in range(1, steps + 1):
            self.move(sx + (x - sx) * i // steps, sy + (y - sy) * i // steps)
            time.sleep(0.02)

    def scroll(self, x: int, y: int, clicks: int, up: bool = False, settle: float = 0.6) -> None:
        button = 4 if up else 5
        self._sweep_to(x, y)
        time.sleep(0.2)
        for _ in range(clicks):
            self._xtest.fake_input(self.d, self._X.ButtonPress, button)
            self.d.sync()
            self._xtest.fake_input(self.d, self._X.ButtonRelease, button)
            self.d.sync()
            time.sleep(0.18)
        time.sleep(settle)

    def drag(self, x1: int, y1: int, x2: int, y2: int, steps: int = 12, settle: float = 0.6) -> None:
        """按住左键从 (x1,y1) 拖到 (x2,y2) 再松开。

        WebKit 的 pointer 事件依赖真实的 press/move/release 序列；单次跳转移动不会产生
        pointermove，因此这里分步移动并逐点 sync。
        """
        self._sweep_to(x1, y1)
        time.sleep(0.12)
        self._xtest.fake_input(self.d, self._X.ButtonPress, 1)
        self.d.sync()
        time.sleep(0.08)
        for i in range(1, steps + 1):
            self.move(x1 + (x2 - x1) * i // steps, y1 + (y2 - y1) * i // steps)
            time.sleep(0.02)
        time.sleep(0.08)
        self._xtest.fake_input(self.d, self._X.ButtonRelease, 1)
        self.d.sync()
        time.sleep(settle)

    def _keycode(self, keysym_name: str) -> tuple[int, bool]:
        ks = self._XK.string_to_keysym(keysym_name)
        if ks == 0:
            raise RuntimeError(f"未知 keysym：{keysym_name}")
        return self.d.keysym_to_keycode(ks), False

    def key(self, keysym_name: str, shift: bool = False, settle: float = 0.25) -> None:
        code, _ = self._keycode(keysym_name)
        shift_code, _ = self._keycode("Shift_L")
        if shift:
            self._xtest.fake_input(self.d, self._X.KeyPress, shift_code)
            self.d.sync()
        self._xtest.fake_input(self.d, self._X.KeyPress, code)
        self.d.sync()
        time.sleep(0.03)
        self._xtest.fake_input(self.d, self._X.KeyRelease, code)
        self.d.sync()
        if shift:
            self._xtest.fake_input(self.d, self._X.KeyRelease, shift_code)
            self.d.sync()
        time.sleep(settle)

    # 仅 ASCII：合成输入一律使用 ASCII，避免依赖输入法。
    _NAMED = {
        " ": ("space", False),
        "\n": ("Return", False),
        "\t": ("Tab", False),
        ",": ("comma", False),
        ".": ("period", False),
        "-": ("minus", False),
        "/": ("slash", False),
        "|": ("bar", True),
        ":": ("colon", True),
        ";": ("semicolon", False),
        "_": ("underscore", True),
        "(": ("parenleft", True),
        ")": ("parenright", True),
        "'": ("apostrophe", False),
        '"': ("quotedbl", True),
        "!": ("exclam", True),
        "?": ("question", True),
        "=": ("equal", False),
        "+": ("plus", True),
        "*": ("asterisk", True),
        "#": ("numbersign", True),
        "%": ("percent", True),
        "&": ("ampersand", True),
        "@": ("at", True),
        "$": ("dollar", True),
    }

    def type_ascii(self, text: str, settle: float = 0.02) -> None:
        for ch in text:
            if ch in self._NAMED:
                name, shift = self._NAMED[ch]
            elif ch.isupper():
                name, shift = ch.lower(), True
            else:
                name, shift = ch, False
            self.key(name, shift=shift, settle=settle)

    def select_all_and_clear(self) -> None:
        """Ctrl+A → BackSpace（清空当前聚焦输入框）。"""
        ctrl_code, _ = self._keycode("Control_L")
        a_code, _ = self._keycode("a")
        self._xtest.fake_input(self.d, self._X.KeyPress, ctrl_code)
        self.d.sync()
        self._xtest.fake_input(self.d, self._X.KeyPress, a_code)
        self.d.sync()
        self._xtest.fake_input(self.d, self._X.KeyRelease, a_code)
        self.d.sync()
        self._xtest.fake_input(self.d, self._X.KeyRelease, ctrl_code)
        self.d.sync()
        time.sleep(0.1)
        self.key("BackSpace")


# ---------------------------------------------------------------------------
# 应用会话：合成端点 + 前端 dev server + 真实窗口
# ---------------------------------------------------------------------------


def _proc_start_time(pid: int) -> str:
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
        rest = stat.split(") ", 1)[1].split()
        return rest[19]
    except Exception:
        return ""


def _cmdline(pid: int) -> str:
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode().strip()
    except Exception:
        return ""


@dataclass
class AppSession:
    """一次性隔离根 + 合成端点 + 前端 dev server + 桌面 App。"""

    root: Path
    display: str
    artifact_override: bool = False
    env_extra: dict = field(default_factory=dict)
    # 阶段 D 用安装入口（.deb 解包后的二进制）：不需要前端 dev server（release 内嵌前端资源）
    binary: Path | None = None
    use_dev_server: bool = True
    # 端口 5273 可能被用户自己正在运行的 OmniDesk 开发服务占用。此时允许复用那份
    # **同仓库同源码**的 dev server（不启动、也不终止它），其余数据根/进程/会话总线仍隔离。
    reuse_running_dev_server: bool = False
    dev_port: int = DEV_PORT
    # 进程句柄
    proc: subprocess.Popen | None = None
    vite: subprocess.Popen | None = None
    mock_ctl: object | None = None
    mock_server: object | None = None
    mock_base_url: str | None = None
    log: list = field(default_factory=list)
    started_at: float | None = None

    # --- 目录 ---------------------------------------------------------------
    @property
    def app_data(self) -> Path:
        return self.root / "app-data"

    @property
    def local_data(self) -> Path:
        return self.root / "local-data"

    @property
    def dsh_home(self) -> Path:
        return self.root / "dsh-home"

    @property
    def agents_home(self) -> Path:
        return self.root / "agents"

    @property
    def logs(self) -> Path:
        return self.root / "logs"

    @property
    def xdg(self) -> Path:
        """WebKitGTK 的 localStorage / 缓存根（默认在 $XDG_DATA_HOME/<identifier>）。

        OD-ARCH-059 S3 实测：不隔离时 App 会把线程缓存写进用户真实的
        `~/.local/share/dev.omnidesk.discipline`，导致旧对话名漏进验收截图。
        """
        return self.root / "xdg"

    def prepare(self) -> None:
        for p in (
            self.root,
            self.app_data,
            self.dsh_home,
            self.agents_home,
            self.logs,
            self.xdg / "data",
            self.xdg / "config",
            self.xdg / "cache",
        ):
            p.mkdir(parents=True, exist_ok=True)

    def _note(self, message: str) -> None:
        self.log.append(message)

    # --- 合成端点 -----------------------------------------------------------
    def start_mock(self) -> str:
        from _mock_model_server import start_mock

        base_url, ctl, server = start_mock()
        self.mock_base_url, self.mock_ctl, self.mock_server = base_url, ctl, server
        self._note(f"合成端点：{base_url}")
        return base_url

    def write_overlay(self) -> Path:
        path = self.root / "synthetic-provider.yml"
        path.write_text(
            "\n".join(
                [
                    # 与 scripts/dev-ui.sh 同口径：web-api 通道走 agent 默认模型覆盖，
                    # `acp` 层只对 dsh-acp app 生效，web app 不加载它。
                    "- id: agent-default-model",
                    '  name: "@deepseek-ai/dsh-agent-default-model"',
                    "  config:",
                    "    provider: fake",
                    "    model: synthetic-model",
                    "- id: llm-pi-ai",
                    '  name: "@deepseek-ai/dsh-llm-pi-ai"',
                    "  config:",
                    "    providers:",
                    "      fake:",
                    "        displayName: Fake (synthetic)",
                    "        apiKeyEnv: " + FAKE_KEY_ENV,
                    "        api: openai-completions",
                    "        baseURL: " + str(self.mock_base_url),
                    "        models:",
                    "          - id: synthetic-model",
                    "            name: synthetic-model",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        return path

    # --- 前端 dev server ----------------------------------------------------
    def start_vite(self, timeout: float = 90.0) -> None:
        if self.reuse_running_dev_server:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if self._port_ready():
                    self._note(f"复用已在运行的前端 dev server（127.0.0.1:{self.dev_port}，未启动也未终止）")
                    return
                time.sleep(0.5)
            raise RuntimeError(f"复用的前端 dev server 未在 127.0.0.1:{self.dev_port} 就绪")
        log = open(self.logs / "vite.log", "a", buffering=1)
        env = dict(os.environ)
        env["DISPLAY"] = self.display
        self.vite = subprocess.Popen(
            ["npm", "run", "dev"],
            cwd=str(DESKTOP),
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self._port_ready():
                return
            if self.vite.poll() is not None:
                raise RuntimeError("前端 dev server 提前退出，见 logs/vite.log")
            time.sleep(0.5)
        raise RuntimeError("前端 dev server 未在超时内就绪")

    def _port_ready(self) -> bool:
        import socket

        with socket.socket() as s:
            s.settimeout(0.5)
            return s.connect_ex(("127.0.0.1", self.dev_port)) == 0

    # --- App ----------------------------------------------------------------
    def app_env(self, overlay: Path) -> dict:
        env = dict(os.environ)
        env["DISPLAY"] = self.display
        env["OMNIDESK_APP_DATA_ROOT"] = str(self.app_data)
        env["OMNIDESK_DSH_HOME"] = str(self.dsh_home)
        env["OMNIDESK_AGENTS_HOME"] = str(self.agents_home)
        env["DSH_HOME"] = str(self.dsh_home)
        env["DSH_AGENTS_HOME"] = str(self.agents_home)
        # OD-ARCH-048 起引擎默认走 web-api 通道，Profile 必须是 `web`（DSH 内置 web app
        # 才会打印 `dsh web: ...`）。此前这里固定写 `acp`，与默认通道错配，DSH 会以 acp
        # app 启动、永远不宣布地址，验收窗口只能看到 `engine_unavailable`。
        env["OMNIDESK_ENGINE_CHANNEL"] = "web-api"
        env["OMNIDESK_DSH_PROFILE"] = "web"
        env["OMNIDESK_DSH_PATCHES"] = str(overlay)
        env[FAKE_KEY_ENV] = FAKE_KEY_VALUE
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
        # 强制 X11 走本次一次性显示，隔离用户的 Wayland 会话（见 XSession 的说明）。
        env["GDK_BACKEND"] = "x11"
        # WebKitGTK 的本地存储默认落在用户真实目录；验收窗口必须连它一起隔离。
        env["XDG_DATA_HOME"] = str(self.xdg / "data")
        env["XDG_CONFIG_HOME"] = str(self.xdg / "config")
        env["XDG_CACHE_HOME"] = str(self.xdg / "cache")
        env.pop("WAYLAND_DISPLAY", None)
        env.pop("WAYLAND_SOCKET", None)
        if self.artifact_override:
            env["OMNIDESK_LOCAL_DATA_ROOT"] = str(self.local_data)
        env.update(self.env_extra)
        return env

    def start(self, wait_title: str = "OmniDesk", timeout: float = 90.0) -> None:
        if self.mock_base_url is None:
            self.start_mock()
        overlay = self.write_overlay()
        if self.vite is None and self.use_dev_server:
            self.start_vite()
        log = open(self.logs / "app.log", "a", buffering=1)
        self.proc = subprocess.Popen(
            [str(self.binary or APP_BIN)],
            cwd=str(REPO),
            env=self.app_env(overlay),
            stdout=log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
        )
        self.started_at = time.time()
        self._note(f"App PID={self.proc.pid}（数据根 {self.root}）")

    def alive(self) -> bool:
        return self.proc is not None and self.proc.poll() is None

    # --- 回收 ---------------------------------------------------------------
    def stop_app(self, timeout: float = 30.0) -> dict:
        """按身份核对后终止 App，返回耗时信息。"""
        info = {"pid": None, "already_exited": False, "exit_code": None, "wait_seconds": None}
        if self.proc is None:
            return info
        info["pid"] = self.proc.pid
        if self.proc.poll() is not None:
            info["already_exited"] = True
            info["exit_code"] = self.proc.returncode
            return info
        start = time.monotonic()
        os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
        try:
            self.proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
            self.proc.wait(timeout=10)
        info["wait_seconds"] = round(time.monotonic() - start, 2)
        info["exit_code"] = self.proc.returncode
        self.proc = None
        return info

    def stop_all(self) -> None:
        self.stop_app()
        if self.vite is not None:
            try:
                os.killpg(os.getpgid(self.vite.pid), signal.SIGTERM)
                self.vite.wait(timeout=15)
            except Exception:
                try:
                    os.killpg(os.getpgid(self.vite.pid), signal.SIGKILL)
                except Exception:
                    pass
            self.vite = None
        if self.mock_server is not None:
            try:
                self.mock_server.shutdown()
            except Exception:
                pass
            self.mock_server = None

    def close(self) -> None:
        self.stop_all()


# ---------------------------------------------------------------------------
# 便捷入口（供验收脚本与手工排查共用）
# ---------------------------------------------------------------------------


def ensure_app_binary(build: bool = True) -> dict:
    """按当前源码构建（或复用）桌面二进制，返回构建事实。"""
    facts = {"built": False, "log_tail": "", "binary_mtime": None}
    if build:
        env = dict(os.environ)
        env_out = subprocess.run(
            ["bash", str(REPO / "scripts" / "setup-desktop-toolchain.sh"), "--print-env"],
            capture_output=True,
            text=True,
            env=env,
        )
        if env_out.returncode == 0:
            for line in env_out.stdout.splitlines():
                if line.startswith("export "):
                    body = line[len("export "):]
                    key, _, value = body.partition("=")
                    env[key] = value.strip().strip('"')
        proc = subprocess.run(
            ["cargo", "build", "--offline"],
            cwd=str(REPO / "desktop" / "src-tauri"),
            capture_output=True,
            text=True,
            env=env,
        )
        facts["built"] = proc.returncode == 0
        facts["log_tail"] = "\n".join((proc.stdout + proc.stderr).splitlines()[-8:])
    if APP_BIN.exists():
        facts["binary_mtime"] = APP_BIN.stat().st_mtime
    return facts


def main(argv: list) -> int:
    """手工排查入口：--shot <png> 抓一张当前窗口图。"""
    import argparse

    parser = argparse.ArgumentParser(description="OD-ARCH-025 窗口驱动装置")
    parser.add_argument("--display", default=":95")
    parser.add_argument("--shot", default=None, help="抓一张窗口截图到该路径")
    parser.add_argument("--title", default="OmniDesk")
    parser.add_argument("--click", default=None, help="窗口内相对坐标 x,y")
    parser.add_argument("--type", default=None, help="向当前焦点输入的 ASCII 文本")
    args = parser.parse_args(argv[1:])

    xs = XSession(args.display)
    win = xs.find_window(args.title, timeout=20)
    if win is None:
        print("未找到应用窗口")
        return 2
    geo = xs.geometry(win)
    print(f"窗口标题={xs.title(win)!r} 几何={geo.as_tuple()}")
    if args.click:
        dx, dy = (int(v) for v in args.click.split(","))
        xs.click(geo.x + dx, geo.y + dy)
        print(f"已点击窗口内 ({dx},{dy})")
    if args.type:
        xs.type_ascii(args.type)
        print(f"已输入 {len(args.type)} 个字符")
    if args.shot:
        xs.shot(Path(args.shot), geo)
        print(f"截图已保存 {args.shot}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
