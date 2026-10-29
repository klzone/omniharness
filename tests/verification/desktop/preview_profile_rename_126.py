#!/usr/bin/env python3
"""OD-ARCH-126：自建 Profile 的显示名能在软件里改（建 → 改名 → 两处都变 → 切过去用）。

125 之前，自建 Profile 的名字建好就定死了：顶栏与设置页读 index.v1.json 的 label，
场景页与资产页读 decl.yml 的 display_name，软件里却没有一个改名入口。本脚本在真实
窗口里验证：点「改名」→ 行内输入新名字 → 保存后登记表与声明同时更新，目录名不动，
而且改完还能切过去用。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_rename_126.py [--display :146] [--root /tmp/od126-preview]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import accessibility as ax  # noqa: E402
from preview_profile_switch_110 import deep_text, read_settings, wait_for  # noqa: E402
from preview_weekly_100 import app_nodes, find, start_dbus, start_display  # noqa: E402
from window_harness import DEV_PORT, AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-126"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "rename-desk"
LABEL = "Rename Desk"
NEW_LABEL = "New Desk"

REGISTRY = Path("app-data") / "profiles" / "index.v1.json"
DECL = Path("app-data") / "profiles" / NAME / "decl.yml"


def registry(root: Path) -> dict:
    path = root / REGISTRY
    if not path.is_file():
        return {"profiles": []}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"profiles": []}


def entry(root: Path) -> dict | None:
    return next((p for p in registry(root).get("profiles", []) if p.get("name") == NAME), None)


def label_now(root: Path) -> str | None:
    item = entry(root)
    return (item or {}).get("label")


def display_name(root: Path) -> str | None:
    path = root / DECL
    if not path.is_file():
        return None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("display_name:"):
            return line.split(":", 1)[1].strip()
    return None


def click_text(pid: int, text: str, exact: bool = True, tries: int = 6) -> bool:
    """按可见文案点击；WebKit 不把 data-testid 暴露给 AT-SPI，只能按文字取。"""
    for _ in range(tries):
        node = find(app_nodes(pid), text, contains=not exact)
        if node is not None and ax.invoke(node):
            return True
        time.sleep(1.2)
    return False


def type_into(pid: int, xs: XSession, hint: str, value: str, alt: str | None = None) -> bool:
    """点进输入框再敲 ASCII：合成窗口无法输入中文。"""
    for candidate in [hint] + ([alt] if alt else []):
        point = ax.focus_point(candidate) or ax.focus_point(candidate, contains=True)
        if point is None:
            node = find(app_nodes(pid), candidate, contains=True)
            if node is None:
                continue
            try:
                from gi.repository import Atspi

                ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
                if ext.width <= 0 or ext.height <= 0:
                    continue
                point = (ext.x + ext.width // 2, ext.y + ext.height // 2)
            except Exception:
                continue
        xs.click(point[0], point[1], settle=0.5)
        xs.type_ascii(value)
        return True
    return False


def open_assets_tab(pid: int) -> bool:
    for _ in range(6):
        if click_text(pid, "系统偏好设置"):
            time.sleep(3.0)
            if click_text(pid, "Profile 资产"):
                if wait_for(lambda: "PROFILE 资产" in deep_text(pid), 15.0, 1.0):
                    return True
        time.sleep(2.0)
    return False


def open_popover(pid: int, xs: XSession, candidates: list[str]) -> bool:
    """点顶栏 Profile 徽标展开弹层。没有边框的窗口首击常被吞掉，补一次。"""
    mark = "选择引擎 Profile"
    for attempt in range(6):
        if mark in deep_text(pid):
            return True
        point = None
        for name in candidates:
            point = ax.focus_point(name, role="push button") or ax.focus_point(name)
            if point is not None:
                break
        if point is None:
            time.sleep(2.0)
            continue
        xs.click(point[0], point[1], settle=1.2)
        if mark in deep_text(pid):
            return True
        if attempt >= 1:
            xs.click(point[0], point[1], settle=1.2)
            if mark in deep_text(pid):
                return True
        time.sleep(1.5)
    return mark in deep_text(pid)


def dev_server_ready() -> bool:
    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", DEV_PORT)) == 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":146")
    ap.add_argument("--root", default="/tmp/od126-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    SHOTS.mkdir(parents=True, exist_ok=True)

    facts: dict = {"root": str(root), "display": args.display, "checks": {}}
    xvfb = start_display(args.display, SCREEN_W, SCREEN_H + 2000)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)

    session = AppSession(
        root=root,
        display=args.display,
        reuse_running_dev_server=dev_server_ready(),
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
        },
    )
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)
        time.sleep(2.5)

        # --- 1. 从干净底座建一个 Profile ---------------------------------
        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["popover_opened"] = open_popover(pid, xs, ["标准", "专业"])
        facts["checks"]["create_form_opened"] = click_text(pid, "新建自定义 Profile", exact=False)
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        facts["checks"]["submitted"] = click_text(pid, "创建")
        facts["checks"]["created"] = bool(wait_for(lambda: entry(root) is not None, 30.0, 0.5))
        facts["checks"]["created_label"] = label_now(root) == LABEL
        facts["checks"]["decl_written"] = display_name(root) == LABEL
        # 创建后弹层直接进「切换到…？」确认；不切，留在底座。
        click_text(pid, "取消")
        time.sleep(1.0)
        xs.shot(SHOTS / "01-created.png")

        # --- 2. 设置页里行内改名 -----------------------------------------
        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        # 行内标题文字与徽标不进无障碍树（118/125 已记录），这里只认行尾按钮。
        facts["checks"]["rename_button_present"] = bool(
            wait_for(lambda: "改名" in deep_text(pid), 20.0, 0.5)
        )
        facts["checks"]["rename_opened"] = click_text(pid, "改名")
        time.sleep(1.0)
        facts["checks"]["input_appeared"] = "新的显示名" in deep_text(pid)
        xs.shot(SHOTS / "02-rename-row.png")
        # 输入框初值是当前显示名，先点进去全清再敲新名字。
        point = ax.focus_point("新的显示名", role="text") or ax.focus_point("新的显示名")
        if point is None:
            node = find(app_nodes(pid), "新的显示名", contains=True)
            if node is not None:
                try:
                    from gi.repository import Atspi

                    ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
                    point = (ext.x + ext.width // 2, ext.y + ext.height // 2)
                except Exception:
                    point = None
        facts["checks"]["focused_input"] = point is not None
        if point is not None:
            xs.click(point[0], point[1], settle=0.5)
            xs.select_all_and_clear()
            xs.type_ascii(NEW_LABEL)
        time.sleep(0.4)
        xs.shot(SHOTS / "03-rename-typed.png")
        facts["checks"]["saved"] = click_text(pid, "保存")
        facts["checks"]["registry_updated"] = bool(
            wait_for(lambda: label_now(root) == NEW_LABEL, 20.0, 0.5)
        )
        facts["checks"]["decl_updated"] = display_name(root) == NEW_LABEL
        facts["checks"]["decl_still_valid"] = "schema: omnidesk.profile-decl.v1" in (root / DECL).read_text(
            encoding="utf-8"
        )
        facts["checks"]["backup_left"] = bool(
            list((root / "app-data" / "profiles" / NAME).glob("decl.yml.bak-*"))
        )
        # 目录名与 profile 标识不动：Skill / 插件 / 分类绑定才不会跟着抖。
        facts["checks"]["dsh_dir_unchanged"] = (root / "dsh-home" / "profiles" / NAME).is_dir()
        facts["entry_after_rename"] = entry(root)
        xs.shot(SHOTS / "04-renamed.png")

        # --- 3. 改完的名字能切过去用 -------------------------------------
        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["popover_reopened"] = open_popover(
            pid, xs, [NEW_LABEL.split()[0], NEW_LABEL, "标准", "专业"]
        )
        facts["checks"]["new_label_in_popover"] = NEW_LABEL in deep_text(pid)
        facts["checks"]["picked"] = click_text(pid, NEW_LABEL, exact=False)
        time.sleep(1.0)
        facts["checks"]["confirmed"] = click_text(pid, "确认")
        facts["checks"]["switched"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 30.0, 0.5)
        )
        facts["checks"]["dock_has_scenes"] = all(
            s in deep_text(pid) for s in ("工作记录", "文案写作", "项目跟踪")
        )
        xs.shot(SHOTS / "05-switched.png")
    finally:
        session.close()
        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(facts, ensure_ascii=False))
        for proc in (bus_proc, xvfb):
            try:
                proc.terminate()
            except Exception:
                pass
    return 0 if all(facts["checks"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
