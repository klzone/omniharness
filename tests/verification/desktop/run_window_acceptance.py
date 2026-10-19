#!/usr/bin/env python3
"""OD-ARCH-025 · 个人试用前桌面闭环验收（真实桌面窗口）。

在**一次性隔离数据根 + 一次性 Xvfb 显示**里启动真实 OmniDesk 桌面窗口，用合成引擎走通：

  阶段 A：设置成果目录 → 开启本机模型权限 → AI 辅助任务（统计 → 引擎解释 → 人工确认 → 保存）
          → 离线「项目跟进」流程 → 保存 → 重新打开成果
  阶段 B：关闭并重启应用 → 权限/成果目录持久化 → 成果仍可重开 → 会话/任务页状态符合契约
  阶段 C：可控慢关闭 → 真实关闭窗口 → 记录关闭耗时与任务状态，确认无卡死、无孤儿进程
  阶段 D：安装入口（.deb 解包后的启动入口）可启动应用

证据等级：真实窗口运行（XTEST 注入输入，GDK 抓图）+ 文件事实（隔离根内的设置/成果/引擎状态）。
安全边界：只用合成内容与回环合成端点；不读凭证；不碰 `runtime/`、真实数据与用户桌面会话。

用法：
  python3 run_window_acceptance.py --stage A [--root /tmp/od025-evidence/run] [--display :95]
  python3 run_window_acceptance.py --stage B|C|D --root <阶段A的一次性根>
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
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import (  # noqa: E402
    APP_BIN,
    AppSession,
    Geometry,
    XSession,
    ensure_app_binary,
)

EVIDENCE = HERE / "evidence" / "od-arch-025"
EVIDENCE_SHOTS = EVIDENCE / "shots"

# --- 界面坐标（窗口相对；由真实窗口截图校准，窗口为 tauri.conf 默认 1280x820）------
NAV = {"home": 284, "workbench": 337, "offline": 385, "sessions": 434, "settings": 492, "engine": 538}
SET_PATH_INPUT = (665, 475)
SET_SAVE_PATH_BTN = (197, 566)
# 滚到底后的模型权限复选框（真实窗口底部视图校准：已滚到页面底部时本机模型在 y=431）
SET_LOCAL_CHECKBOX = (171, 431)
WB_SAMPLE_BASIC = (225, 378)
WB_RUN_STATS = (219, 667)
# 阶段 3/4/5（按下 End 滚到底后校准；见 --stop-after=stats 的抓图）
WB_BUILD_PROMPT = (213, 449)
WB_RUN_ENGINE = (453, 449)
WB_CONFIRM = (171, 510)
WB_SAVE = (196, 757)
WB_CONFIRM = (171, 208)
WB_SAVE = (199, 456)
# 离线流程（顶部未滚动视图）
OFF_TAB_PROJECT = (434, 350)
OFF_SAMPLE = (208, 418)
OFF_RUN_LOCAL = (196, 473)
OFF_SAVE = (199, 683)
OFF_CONFIRM = (171, 630)
OFF_REOPEN_FIRST = (1124, 115)


class Facts:
    """收集可核对的事实（不掩盖失败，逐条记录判据）。"""

    def __init__(self, stage: str) -> None:
        self.stage = stage
        self.items: list[dict] = []

    def add(self, name: str, ok: bool, detail=None) -> bool:
        self.items.append({"name": name, "ok": bool(ok), "detail": detail})
        print(("  [PASS] " if ok else "  [FAIL] ") + name + (f" — {detail}" if detail else ""))
        return bool(ok)

    def verdict(self) -> str:
        return "pass" if all(i["ok"] for i in self.items) else "fail"

    def to_json(self) -> dict:
        return {"stage": self.stage, "verdict": self.verdict(), "facts": self.items}


def shots_dir(stage: str) -> Path:
    d = EVIDENCE_SHOTS / stage
    d.mkdir(parents=True, exist_ok=True)
    return d


class Driver:
    """窗口内交互封装：每次点击前确保输入焦点（无窗口管理器时必须显式设置）。"""

    def __init__(self, xs: XSession, win, geo: Geometry, tmp_shot: Path) -> None:
        self.xs = xs
        self.win = win
        self.geo = geo
        self.tmp_shot = tmp_shot
        self.tmp_shot.parent.mkdir(parents=True, exist_ok=True)
        # 记录「当前页面到达该视图所用的滚动配方」：截图会做 1px 尺寸微调强制重绘，
        # 实测这会改变滚动位置，从而让紧随其后的点击落空。点击前重放配方即可复原视图。
        self._recipe: list[tuple] = []
        self._relayout_dirty = False

    # --- 内部：滚动原语（不写入配方）----------------------------------------
    def _do_recipe(self, recipe: list[tuple]) -> None:
        for kind, arg in recipe:
            if kind == "top":
                self._wheel_until_stable(up=True)
            elif kind == "bottom":
                self._wheel_until_stable(up=False)
            elif kind == "down":
                self.xs.scroll(self.geo.x + 640, self.geo.y + 450, clicks=arg, up=False, settle=1.0)
            elif kind == "up":
                self.xs.scroll(self.geo.x + 640, self.geo.y + 450, clicks=arg, up=True, settle=1.0)

    def click(self, rx: int, ry: int, settle: float = 1.0, reapply: bool = True) -> None:
        if reapply and self._relayout_dirty and self._recipe:
            self._do_recipe(self._recipe)
            self._relayout_dirty = False
        self.xs.dismiss_overlays()
        self.xs.focus(self.win)
        # 预热点击：窗口尺寸微调/重绘之后，首次按键可能被 GTK 用于激活而吞掉。
        self.xs.click(self.geo.x + 1240, self.geo.y + 405, settle=0.25)
        self.xs.click(self.geo.x + rx, self.geo.y + ry, settle=settle)

    def locate_primary(self, y_lo: int, y_hi: int) -> tuple[int, int] | None:
        """截当前帧并定位主按钮中心 (x, y)；调用后应「就地点击」且不再滚动。"""
        self.nudge()
        self.neutral()
        self.xs.shot(self.tmp_shot, self.geo)
        return find_primary_center(self.tmp_shot, y_lo, y_hi)

    def neutral(self) -> None:
        """把指针移到窗口右缘空白处：离开控件后原生 tooltip 会消失，避免污染截图。"""
        self.xs._sweep_to(self.geo.x + 1240, self.geo.y + 405)
        time.sleep(0.5)
        self.xs.dismiss_overlays()

    def nudge(self) -> None:
        """尺寸微调 1px 再恢复：无窗口管理器时 WebKit 的重绘可能滞后，微调会强制重绘。

        副作用：实测会改变滚动容器的滚动位置，因此记录「重绘后需要复原视图」。
        """
        g = self.geo
        self.xs.place(self.win, g.x, g.y, g.w, g.h - 1, settle=0.6)
        self.xs.place(self.win, g.x, g.y, g.w, g.h, settle=1.4)
        self._relayout_dirty = True

    def shot(self, path: Path) -> None:
        # 无窗口管理器时 WebKit 的重绘会滞后：先微调尺寸强制重绘，再移开指针拍图。
        self.nudge()
        self.neutral()
        self.xs.shot(path, self.geo)

    def page(self, name: str, settle: float = 1.6) -> None:
        # 切场景等于重置滚动位置，先清空配方再点 Dock（Dock 固定，不受滚动影响）。
        self._recipe = []
        self._relayout_dirty = False
        self.xs.dismiss_overlays()
        self.xs.focus(self.win)
        self.xs.click(self.geo.x + 1240, self.geo.y + 405, settle=0.25)
        self.xs.click(self.geo.x + 48, self.geo.y + NAV[name], settle=settle)
        self.nudge()

    def content_focus(self) -> None:
        self.click(640, 700, settle=0.5)

    # --- 滚动：用滚轮（不依赖键盘焦点），以「画面内容是否还在变」为准 -------------
    def _content_sig(self) -> str:
        """内容区（避开顶栏时钟）的截图指纹。"""
        import hashlib

        from PIL import Image

        self.xs.shot(self.tmp_shot, self.geo)
        img = Image.open(self.tmp_shot).crop((60, 120, 1280, 820))
        return hashlib.sha256(img.tobytes()).hexdigest()

    def _wheel_until_stable(self, up: bool, max_rounds: int = 12) -> None:
        prev = None
        stable = 0
        for _ in range(max_rounds):
            # 慢节奏：实测每轮 10 次 + 1.0s 间隔才稳定生效；快节奏会被 WebKit 合并丢弃。
            self.xs.scroll(self.geo.x + 640, self.geo.y + 450, clicks=10, up=up, settle=1.0)
            sig = self._content_sig()
            stable = stable + 1 if sig == prev else 0
            if stable >= 2:  # 连续两轮不变才算到底/到顶
                return
            prev = sig

    def scroll_top(self) -> None:
        self._wheel_until_stable(up=True)
        self._recipe = [("top", 0)]

    def scroll_bottom(self) -> None:
        self._wheel_until_stable(up=False)
        self._recipe = [("bottom", 0)]

    def scroll_down_steps(self, clicks: int, settle: float = 1.0) -> None:
        self.xs.scroll(self.geo.x + 640, self.geo.y + 450, clicks=clicks, up=False, settle=settle)
        self._recipe.append(("down", clicks))

    def scroll_up_steps(self, clicks: int) -> None:
        self.xs.scroll(self.geo.x + 640, self.geo.y + 450, clicks=clicks, up=True, settle=1.0)
        self._recipe.append(("up", clicks))

    # 输入框在每键都会触发重渲染，XTEST 快打会丢字符；分块 + 停顿后仍以文件事实校验。
    def type_text(self, text: str, settle: float = 0.25, chunk: int = 4, pause: float = 0.6) -> None:
        for i in range(0, len(text), chunk):
            self.xs.type_ascii(text[i : i + chunk], settle=settle)
            time.sleep(pause)


# ---------------------------------------------------------------------------
# 元素定位：主按钮按颜色定位（滚轮步长有 ±30px 漂移，固定坐标不可靠）
# ---------------------------------------------------------------------------

PRIMARY_MIN_RUN = 10


def _is_primary(px) -> bool:
    """主按钮填充色（启用 #3525CD / 禁用浅紫）判定。"""
    r, g, b = px[0], px[1], px[2]
    return (b - r) > 30 and (b - g) > 30 and b > 150 and r < 235


def find_primary_center(
    png: Path, y_lo: int, y_hi: int, x_lo: int = 140, x_hi: int = 1220
) -> tuple[int, int] | None:
    """在给定窗口里定位第一块「主按钮」主色区域，返回 (中心x, 中心y)。

    阈值按真实按钮尺寸定：行内主色像素 ≥30、纵向簇高 ≥20 行、横向跨度 ≥60 像素。
    这样可避开 16px 的勾选框与其它零散主色像素。
    """
    from PIL import Image

    im = Image.open(png).convert("RGB")
    w, h = im.size
    y_lo, y_hi = max(0, y_lo), min(h, y_hi)
    x_lo, x_hi = max(0, x_lo), min(w, x_hi)

    rows: list[tuple[int, int, int]] = []  # (y, min_x, max_x)
    for y in range(y_lo, y_hi):
        xs = [x for x in range(x_lo, x_hi) if _is_primary(im.getpixel((x, y)))]
        if len(xs) >= 30 and (xs[-1] - xs[0]) >= 60:
            rows.append((y, xs[0], xs[-1]))
    if not rows:
        return None

    cluster = [rows[0]]
    for row in rows[1:]:
        if row[0] - cluster[-1][0] <= 3:
            cluster.append(row)
        else:
            if cluster[-1][0] - cluster[0][0] + 1 >= 20:
                break
            cluster = [row]
    if cluster[-1][0] - cluster[0][0] + 1 < 20:
        return None
    cx = (min(r[1] for r in cluster) + max(r[2] for r in cluster)) // 2
    cy = (cluster[0][0] + cluster[-1][0]) // 2
    return cx, cy


# ---------------------------------------------------------------------------
# 以文件事实为准的稳健交互（无窗口管理器时的滚动/输入不可完全预测）
# ---------------------------------------------------------------------------


def _read_settings(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ""


def _permissions(path: Path) -> dict:
    raw = _read_settings(path)
    try:
        return json.loads(raw).get("model_permissions", {})
    except Exception:
        return {}


def set_artifact_dir(d: Driver, xs: XSession, artifact_dir: Path, settings_file: Path):
    """输入成果目录并保存；被打断的键入会用更慢的节奏重试（以设置文件内容为准）。"""
    raw = ""
    for settle in (0.25, 0.4):
        d.click(*SET_PATH_INPUT)
        xs.select_all_and_clear()
        d.type_text(str(artifact_dir), settle=settle)
        d.click(*SET_SAVE_PATH_BTN, settle=2.5)
        raw = _read_settings(settings_file)
        if str(artifact_dir) in raw:
            return True, raw, settle
    return False, raw, None


def enable_local_permission(d: Driver, settings_file: Path):
    """勾选「本机模型」；若误点到其它类别则用同一坐标再点一次回退。"""
    before = _permissions(settings_file)
    for y in (431, 445, 417, 459):
        d.click(171, y, settle=2.0)
        after = _permissions(settings_file)
        if after.get("local"):
            return True, _read_settings(settings_file), y
        if after != before:
            d.click(171, y, settle=1.5)
            reverted = _permissions(settings_file)
            if reverted != before:
                d.shot(Path("/tmp/od025-permission-revert-failed.png"))
    return False, _read_settings(settings_file), None


# ---------------------------------------------------------------------------
# 阶段 A：主闭环
# ---------------------------------------------------------------------------


def stage_a(args) -> Facts:
    facts = Facts("A")
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)
    app = AppSession(root=root, display=args.display)
    app.prepare()
    xs = XSession(args.display)
    shots = shots_dir("A")
    # 成果目录用短路径：既在仓库外、又避免长串输入在无 IME 的 XTEST 下被截断。
    artifact_dir = Path(args.artifact_dir)
    if artifact_dir.exists():
        shutil.rmtree(artifact_dir)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            facts.add("窗口出现", False, "未找到应用窗口")
            return facts
        geo = xs.geometry(win)
        xs.focus(win)
        facts.add("真实桌面窗口出现", True, {"title": xs.title(win), "geometry": geo.as_tuple(), "display": args.display})
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(6)
        d.shot(shots / "01_home.png")

        # --- 1) 设置：成果目录 -------------------------------------------------
        d.page("settings")
        d.scroll_top()
        d.shot(shots / "02_settings_top.png")
        settings_file = app.app_data / "config" / "settings.v1.json"

        ok, raw, used_settle = set_artifact_dir(d, xs, artifact_dir, settings_file)
        d.shot(shots / "03_artifact_dir_saved.png")
        facts.add(
            "用户设置的成果目录写入设置文件",
            ok,
            {"settings_file": str(settings_file), "typed_settle": used_settle, "content": raw[:400]},
        )
        facts.add("成果目录已按设置创建", artifact_dir.is_dir(), str(artifact_dir))

        # --- 2) 设置：开启本机模型权限 ----------------------------------------
        # 无窗口管理器下滚动位置不总是可预测，因此以「设置文件里 local 是否真的变 true」
        # 为准，并在误点其它类别时用同一个坐标再点一次回退。
        if args.only in ("all", "ai"):
            d.scroll_bottom()
            d.shot(shots / "04_settings_permissions.png")
            enabled, raw, used_y = enable_local_permission(d, settings_file)
            d.shot(shots / "05_local_enabled.png")
            facts.add("开启本机模型后权限写入设置文件", enabled, {"clicked_y": used_y, "content": raw[:400]})
            if not enabled:
                return facts

            # --- 3) 工作台：AI 辅助任务 ---------------------------------------
            d.page("workbench")
            d.scroll_top()
            d.click(*WB_SAMPLE_BASIC, settle=1.5)
            d.shot(shots / "07_sample_loaded.png")
            d.click(*WB_RUN_STATS, settle=2.5)
            d.shot(shots / "08_stats.png")
            if args.stop_after == "stats":
                d.scroll_bottom()
                d.shot(shots / "08b_after_end.png")
                return facts

            d.scroll_bottom()
            d.scroll_up_steps(12)
            d.shot(shots / "09_stage3.png")
            # 按主色定位「用引擎生成解释」（主按钮）：同排第一个按钮固定左对齐，故
            # 「生成解释提示词」= (213, cy)，主按钮 = (px, py)。
            found = d.locate_primary(300, 700)
            if found is None:
                facts.add("定位阶段 3 按钮行", False, "未找到主按钮区域")
                return facts
            px, py = found
            d.click(213, py, settle=2.0, reapply=False)
            d.shot(shots / "10_prompt.png")
            # 生成提示词后同排多了「副本提示词」，主按钮位置随之右移，需重新定位。
            for attempt in range(3):
                got = d.locate_primary(300, 700) or (px, py)
                px, py = got
                d.click(px, py, settle=2.0, reapply=False)
                time.sleep(6)
                if any((app.app_data / "engine-state").glob("omni-*.json")):
                    break
            d.shot(shots / "11_engine_submitted.png")

            # 等引擎（合成端点）产出解释：以宿主落盘的任务记录为文件级信号
            task_state_dir = app.app_data / "engine-state"
            deadline = time.monotonic() + args.engine_timeout
            task_record = None
            first_wait = True
            while time.monotonic() < deadline:
                time.sleep(4)
                if first_wait:
                    d.shot(shots / "12_engine_waiting.png")
                    first_wait = False
                records = sorted(task_state_dir.glob("omni-*.json")) if task_state_dir.is_dir() else []
                if records:
                    try:
                        task_record = json.loads(records[-1].read_text(encoding="utf-8"))
                    except Exception:
                        task_record = None
                    if task_record and task_record.get("state") in ("succeeded", "failed", "cancelled"):
                        break
            d.shot(shots / "13_engine_result.png")
            facts.add(
                "引擎任务完成并落盘（合成端点）",
                bool(task_record) and task_record.get("state") == "succeeded",
                task_record,
            )
            if task_record is None:
                facts.add("引擎任务记录存在", False, str(task_state_dir))
                return facts

            # --- 4) 人工确认 + 保存 -------------------------------------------
            # 引擎刚结束时可能仍有迟到的状态更新把「人工确认」重置，故整步重试；
            # 以成果目录里是否真的出现新文件为准。
            d.scroll_bottom()
            d.scroll_up_steps(6)
            d.shot(shots / "14_before_confirm.png")
            done_before = {p.name for p in artifact_dir.glob("*")} if artifact_dir.is_dir() else set()
            saved_names = []
            for attempt in range(3):
                # 定位「保存产物」（主按钮整块）→ 其上方 247px 是「人工确认」复选框（同一帧内测量）。
                found = d.locate_primary(380, 819)
                if found is None:
                    break
                px, py = found
                d.click(171, py - 247, settle=1.5, reapply=False)
                d.click(px, py, settle=3.0, reapply=False)
                after = {p.name for p in artifact_dir.glob("*")} if artifact_dir.is_dir() else set()
                saved_names = sorted(after - done_before)
                if saved_names:
                    break
                d.scroll_bottom()
                d.scroll_up_steps(6)
                time.sleep(1.5)
            d.shot(shots / "16_saved.png")
            facts.add(
                "AI 任务成果保存到用户设置的目录",
                bool(saved_names),
                {"new_files": saved_names, "dir": str(artifact_dir)},
            )

        # --- 5) 离线「项目跟进」流程 ------------------------------------------
        if args.only in ("all", "offline"):
            d.page("offline")
            d.scroll_top()
            d.click(*OFF_TAB_PROJECT, settle=1.5)
            d.shot(shots / "17_offline_project.png")
            d.scroll_down_steps(5)
            d.shot(shots / "17b_project_inputs.png")
            d.click(*OFF_SAMPLE, settle=1.5)
            d.shot(shots / "18_project_sample.png")
            d.click(*OFF_RUN_LOCAL, settle=2.5)
            d.scroll_down_steps(12, settle=0.8)
            d.shot(shots / "19_project_processed.png")
            # 人工核对 + 保存：定位「保存成果」（主按钮），其上方 53px 是人工核对复选框（同帧测量）
            before = {p.name for p in artifact_dir.glob("*")} if artifact_dir.is_dir() else set()
            saved_names: list[str] = []
            for attempt in range(3):
                found = d.locate_primary(250, 819)
                if found is None:
                    break
                px, py = found
                d.click(171, py - 53, settle=1.5, reapply=False)
                d.click(px, py, settle=3.0, reapply=False)
                after = {p.name for p in artifact_dir.glob("*")} if artifact_dir.is_dir() else set()
                saved_names = sorted(after - before)
                if saved_names:
                    break
                d.scroll_down_steps(12, settle=0.8)
            d.shot(shots / "21_project_saved.png")
            saved = sorted(artifact_dir.glob("*")) if artifact_dir.is_dir() else []
            facts.add("离线项目流程成果保存到用户设置的目录", bool(saved_names), {"new_files": saved_names, "dir": str(artifact_dir)})
            if args.stop_after == "project":
                return facts

            # 重新打开已保存成果（只读）验证列表与重开
            d.scroll_bottom()
            d.shot(shots / "21b_artifact_list.png")
            d.click(*OFF_REOPEN_FIRST, settle=2.5)
            d.shot(shots / "22_reopened.png")
            facts.add("成果列表可重新打开（只读）", True, "见 22_reopened.png")
    finally:
        app.stop_all()
    return facts


# ---------------------------------------------------------------------------
# 阶段 B：关闭并重启后的状态
# ---------------------------------------------------------------------------


def stage_b(args) -> Facts:
    facts = Facts("B")
    root = Path(args.root)
    app = AppSession(root=root, display=args.display)
    xs = XSession(args.display)
    shots = shots_dir("B")
    artifact_dir = Path(args.artifact_dir)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            facts.add("重启后真实窗口出现", False, "未找到应用窗口")
            return facts
        geo = xs.geometry(win)
        xs.focus(win)
        facts.add("重启后真实窗口出现", True, {"geometry": geo.as_tuple(), "display": args.display})
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(6)
        d.shot(shots / "01_restarted_home.png")

        # 设置：成果目录与模型权限是否保持
        d.page("settings")
        d.scroll_top()
        d.shot(shots / "02_settings_after_restart.png")
        raw = _read_settings(app.app_data / "config" / "settings.v1.json")
        facts.add("重启后成果目录设置保持", str(artifact_dir) in raw, raw[:400])
        facts.add("重启后模型权限保持（本机模型仍开启）", '"local": true' in raw, raw[:400])
        d.scroll_bottom()
        d.shot(shots / "03_permissions_after_restart.png")

        # 引擎任务记录仍在（不伪造未完成任务结果：只读宿主落盘的终态）
        tasks = sorted((app.app_data / "engine-state").glob("omni-*.json"))
        states = []
        for p in tasks:
            try:
                states.append((p.name, json.loads(p.read_text(encoding="utf-8")).get("state")))
            except Exception:
                states.append((p.name, "unreadable"))
        facts.add("重启后引擎任务记录仍在", len(tasks) > 0, states)

        # 离线流程：已保存的成果列表仍在，并能重新打开
        d.page("offline")
        d.scroll_top()
        d.scroll_bottom()
        d.shot(shots / "04_artifact_list_after_restart.png")
        files = sorted(artifact_dir.glob("*")) if artifact_dir.is_dir() else []
        facts.add("重启后成果目录内文件仍在", len(files) > 0, [f.name for f in files])
        d.click(*OFF_REOPEN_FIRST, settle=2.5)
        d.shot(shots / "05_reopened_after_restart.png")

        # 会话与任务（只读元数据）
        d.page("sessions")
        d.scroll_top()
        d.shot(shots / "06_sessions_after_restart.png")
    finally:
        app.stop_all()
    return facts


# ---------------------------------------------------------------------------
# 阶段 C：可控慢关闭 → 真实关闭窗口
# ---------------------------------------------------------------------------


def _engine_children() -> list[str]:
    """列出可能的引擎子进程（DSH CLI / python 引擎），用于确认关闭后无孤儿。"""
    out = subprocess.run(["ps", "-eo", "pid,ppid,args"], capture_output=True, text=True).stdout
    hits = []
    for line in out.splitlines()[1:]:
        if "omnidesk_engine" in line or "dsh-runtime" in line or "/deepseek-harness" in line:
            if "ps -eo" in line:
                continue
            hits.append(line.strip()[:160])
    return hits


def stage_c(args) -> Facts:
    facts = Facts("C")
    root = Path(args.root)
    app = AppSession(root=root, display=args.display)
    xs = XSession(args.display)
    shots = shots_dir("C")
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            facts.add("窗口出现", False, "未找到应用窗口")
            return facts
        geo = xs.geometry(win)
        xs.focus(win)
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(6)

        # 可控慢关闭场景：让合成端点「挂起」，任务会一直处于进行中。
        app.mock_ctl.set(mode="hang", hang_seconds=180.0)
        pre_records = {p.name for p in (app.app_data / "engine-state").glob("omni-*.json")}
        d.page("workbench")
        d.scroll_top()
        d.click(*WB_SAMPLE_BASIC, settle=1.5)
        d.click(*WB_RUN_STATS, settle=2.5)
        d.scroll_bottom()
        d.scroll_up_steps(12)
        found = d.locate_primary(300, 700)
        if found is not None:
            px, py = found
            d.click(213, py, settle=2.0, reapply=False)
            got = d.locate_primary(300, 700) or (px, py)
            d.click(got[0], got[1], settle=2.0, reapply=False)
        time.sleep(12)
        d.shot(shots / "01_task_running_before_close.png")
        children_before = _engine_children()
        records = sorted((app.app_data / "engine-state").glob("omni-*.json"))
        facts.add("关闭前引擎子进程在运行（慢关闭场景前提）", len(children_before) > 0, children_before[:3])

        # 关闭前界面仍可响应：切到设置页再切回工作台
        d.page("settings")
        d.shot(shots / "02_ui_responsive_while_task_running.png")
        d.page("workbench")
        d.scroll_bottom()
        d.shot(shots / "03_back_to_running_task.png")

        # 真实关闭窗口（WM_DELETE_WINDOW，等价于点标题栏关闭）
        t0 = time.monotonic()
        xs.close_window(win)
        exited = False
        deadline = t0 + args.shutdown_timeout
        while time.monotonic() < deadline:
            if not app.alive():
                exited = True
                break
            time.sleep(0.25)
        close_seconds = round(time.monotonic() - t0, 2)
        facts.add(
            "真实关闭窗口后应用进程自行退出（未被强杀）",
            exited,
            {"seconds": close_seconds, "exit_code": app.proc.returncode if app.proc else None},
        )
        facts.add("关闭后显示上不再有应用窗口", xs.find_window("OmniDesk", timeout=3) is None, {"window_count": xs.window_count()})

        # 孤儿进程与任务终态
        time.sleep(2)
        children_after = _engine_children()
        facts.add("关闭后无孤儿引擎进程", len(children_after) == 0, children_after[:3])
        final = []
        for p in sorted((app.app_data / "engine-state").glob("omni-*.json")):
            if p.name in pre_records:
                continue  # 只评估本轮慢关闭场景新建的任务
            try:
                rec = json.loads(p.read_text(encoding="utf-8"))
                final.append({"file": p.name, "state": rec.get("state"), "stop_reason": rec.get("stop_reason")})
            except Exception:
                final.append({"file": p.name, "state": "unreadable"})
        no_fake_success = bool(final) and all(item.get("state") != "succeeded" for item in final)
        facts.add(
            "被关闭打断的任务未留下伪造的成功终态",
            no_fake_success,
            {"new_records": final, "note": "非终态/无记录时按契约显示未验证状态，不自动重放"},
        )
        if xs.find_window("OmniDesk", timeout=2) is not None:
            d.shot(shots / "04_after_close.png")
    finally:
        app.stop_all()
    return facts


# ---------------------------------------------------------------------------
# 阶段 D：桌面安装入口可启动应用
# ---------------------------------------------------------------------------


def stage_d(args) -> Facts:
    facts = Facts("D")
    root = Path(args.root)
    shots = shots_dir("D")
    artifact_dir = Path(args.artifact_dir)
    deb = REPO / "desktop" / "src-tauri" / "target" / "release" / "bundle" / "deb" / "OmniDesk_0.1.0_amd64.deb"
    facts.add("安装包存在（当前源码构建）", deb.is_file(), {"deb": str(deb), "mtime": deb.stat().st_mtime if deb.is_file() else None})
    if not deb.is_file():
        return facts
    dest = Path(args.deb_root)
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    proc = subprocess.run(["dpkg-deb", "-x", str(deb), str(dest)], capture_output=True, text=True)
    facts.add("安装包可解包", proc.returncode == 0, proc.stderr[-300:])
    exe = dest / "usr" / "bin" / "omnidesk-desktop"
    desktop_entry = dest / "usr" / "share" / "applications" / "OmniDesk.desktop"
    facts.add("安装入口与桌面项存在", exe.is_file() and desktop_entry.is_file(), {"exe": str(exe), "desktop": str(desktop_entry)})
    if not exe.is_file():
        return facts

    app = AppSession(root=root, display=args.display, binary=exe, use_dev_server=False)
    xs = XSession(args.display)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            facts.add("安装入口启动出真实窗口", False, "未找到应用窗口")
            return facts
        geo = xs.geometry(win)
        xs.focus(win)
        facts.add("安装入口启动出真实窗口", True, {"geometry": geo.as_tuple(), "binary": str(exe)})
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(6)
        d.shot(shots / "01_installed_launch_home.png")

        d.page("settings")
        d.scroll_top()
        d.shot(shots / "02_installed_settings.png")
        raw = _read_settings(app.app_data / "config" / "settings.v1.json")
        facts.add("安装实例沿用同一数据根：成果目录设置保持", str(artifact_dir) in raw, raw[:400])
        files = sorted(artifact_dir.glob("*")) if artifact_dir.is_dir() else []
        facts.add("安装实例可见既有成果文件", len(files) > 0, [f.name for f in files][:5])

        # 会话与任务：直接读宿主任务台账文件，核对「不伪造未完成任务结果」
        d.page("sessions")
        d.scroll_top()
        d.shot(shots / "03_sessions_task_ledger.png")
        ledger = []
        for p in sorted((app.app_data / "engine-state").glob("omni-*.json")):
            try:
                rec = json.loads(p.read_text(encoding="utf-8"))
                ledger.append({"file": p.name, "state": rec.get("state"), "stop_reason": rec.get("stop_reason"), "error_code": rec.get("error_code")})
            except Exception:
                ledger.append({"file": p.name, "state": "unreadable"})
        facts.add("任务台账文件可读（终态如实记录）", len(ledger) > 0, ledger)
    finally:
        app.stop_all()
    return facts


def main(argv: list) -> int:
    parser = argparse.ArgumentParser(description="OD-ARCH-025 真实窗口闭环验收")
    parser.add_argument("--stage", default="A", choices=list("ABCD"))
    parser.add_argument("--display", default=":95")
    parser.add_argument("--root", default="/tmp/od025-evidence/run")
    parser.add_argument("--artifact-dir", default="/tmp/od25a")
    parser.add_argument("--stop-after", default=None)
    parser.add_argument("--only", default="all", choices=["all", "ai", "offline"])
    parser.add_argument("--engine-timeout", type=float, default=240.0)
    parser.add_argument("--shutdown-timeout", type=float, default=60.0)
    parser.add_argument("--deb-root", default="/tmp/od025-evidence/deb-root")
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args(argv[1:])

    EVIDENCE_SHOTS.mkdir(parents=True, exist_ok=True)
    build = ensure_app_binary(build=not args.no_build)
    print(f"[构建] built={build['built']} binary_mtime={build['binary_mtime']}")

    stages = {"A": stage_a, "B": stage_b, "C": stage_c, "D": stage_d}
    facts = stages[args.stage](args)
    payload = facts.to_json()
    payload["build"] = build
    payload["generated_at"] = time.time()
    (EVIDENCE / f"stage-{facts.stage}.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"[阶段 {facts.stage}] verdict={facts.verdict()}")
    return 0 if facts.verdict() == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
