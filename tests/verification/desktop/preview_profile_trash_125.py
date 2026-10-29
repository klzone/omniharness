#!/usr/bin/env python3
"""OD-ARCH-125：回收站是真的恢复通道（建 → 删 → 恢复 → 切过去）。

124 之前，移除 Profile 的回执写着「已移入回收目录，需要时可以从 .trash 恢复」，
可软件里没有任何恢复入口——用户只能自己进文件系统改名。本脚本在真实窗口里
验证：删掉的 Profile 出现在设置页的回收站里，点「恢复」后回到可选清单、
目录回来、还能切过去用。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_trash_125.py [--display :142] [--root /tmp/od125-preview]
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
from preview_profile_switch_110 import (  # noqa: E402
    deep_text,
    read_settings,
    wait_for,
)
from preview_weekly_100 import find, start_dbus, start_display  # noqa: E402
from preview_weekly_100 import app_nodes  # noqa: E402
from window_harness import DEV_PORT, AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-125"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "trash-desk"
LABEL = "Trash Desk"


def registry(root: Path) -> dict:
    path = root / "app-data" / "profiles" / "index.v1.json"
    if not path.is_file():
        return {"profiles": []}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"profiles": []}


def registered(root: Path) -> bool:
    return any(p.get("name") == NAME for p in registry(root).get("profiles", []))


# 弹层自己的无障碍名（role=dialog 的 aria-label）。表头那句「切换后立即生效」
# 在 WebKit 的无障碍树里取不到，只能用这个。
POPOVER_MARK = "选择引擎 Profile"


def popover_showing(pid: int) -> bool:
    """弹层是否真的展开。

    不能沿用 110 的 `popover_open`：它认「基础标准 / 纪检专业」，而设置页正文里
    本来就有「基础标准」这几个字，人还在设置页时会误判成弹层已开（OD-ARCH-125
    实测踩到过）。这里只认弹层自己的表头文案。
    """
    return POPOVER_MARK in deep_text(pid)


def open_popover_strict(pid: int, xs: XSession, candidates: list[str]) -> bool:
    """点顶栏徽标展开弹层；必须看到弹层表头才算成功。"""
    for attempt in range(6):
        if popover_showing(pid):
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
        if popover_showing(pid):
            return True
        # 无边框窗口首击常被吃掉（只交焦点），补一次。
        if attempt >= 1:
            xs.click(point[0], point[1], settle=1.2)
            if popover_showing(pid):
                return True
        time.sleep(1.5)
    return popover_showing(pid)


def pick_and_confirm(pid: int, label: str) -> bool:
    """点列表项，再点「确认」。列表项文字带说明，用包含匹配。"""
    for _ in range(6):
        item = find(app_nodes(pid), label, contains=True)
        if item is not None and ax.invoke(item):
            break
        time.sleep(0.8)
    else:
        return False
    for _ in range(6):
        confirm = find(app_nodes(pid), "确认", contains=False)
        if confirm is not None and ax.invoke(confirm):
            return True
        time.sleep(0.8)
    return False


def click_text(pid: int, text: str, exact: bool = True, tries: int = 5) -> bool:
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


def dev_server_ready() -> bool:
    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", DEV_PORT)) == 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":142")
    ap.add_argument("--root", default="/tmp/od125-preview")
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
        geo = xs.geometry(win)
        time.sleep(2.5)

        # --- 1. 从干净底座建一个 Profile ---------------------------------
        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["popover_opened"] = open_popover_strict(pid, xs, ["标准", "专业"])
        facts["checks"]["create_form_opened"] = click_text(pid, "新建自定义 Profile", exact=False)
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        facts["checks"]["submitted"] = click_text(pid, "创建")
        facts["checks"]["created"] = bool(wait_for(lambda: registered(root), 30.0, 0.5))
        # 创建后弹层直接进「切换到…？」确认；不切，留在底座。
        click_text(pid, "取消")
        time.sleep(1.0)

        # --- 2. 在资产页移除它 -------------------------------------------
        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        facts["checks"]["remove_clicked"] = click_text(pid, "移除")
        facts["checks"]["removed_from_registry"] = bool(
            wait_for(lambda: not registered(root), 25.0, 0.5)
        )
        app_trash = root / "app-data" / "profiles" / ".trash"
        dsh_trash = root / "dsh-home" / "profiles" / ".trash"
        app_ids = sorted(p.name for p in app_trash.iterdir()) if app_trash.is_dir() else []
        dsh_ids = sorted(p.name for p in dsh_trash.iterdir()) if dsh_trash.is_dir() else []
        facts["app_trash_ids"] = app_ids
        facts["dsh_trash_ids"] = dsh_ids
        facts["checks"]["trash_has_item"] = any(n.startswith(NAME) for n in app_ids)
        # 元数据要跟目录放在一起，恢复时显示名与插件清单才不会丢。
        meta_dir = next((app_trash / n for n in app_ids if n.startswith(NAME)), None)
        facts["checks"]["trash_meta_written"] = bool(
            meta_dir is not None and (meta_dir / "trash.v1.json").is_file()
        )

        # --- 3. 回收站区块出现在设置页，点「恢复」 -----------------------
        # WebKit 不把 asset-row-title 里的文字（显示名）与徽标放进无障碍树，
        # 这一行能取到的只有行尾按钮；磁盘上是否有待恢复条目另由 trash_has_item 断言。
        def trash_row_visible() -> bool:
            return "恢复" in deep_text(pid)

        facts["checks"]["trash_section_visible"] = bool(wait_for(trash_row_visible, 25.0, 0.5))
        xs.shot(SHOTS / "01-trash-listed.png")
        facts["restore_visible"] = click_text(pid, "恢复")
        facts["checks"]["restored_to_registry"] = bool(wait_for(lambda: registered(root), 25.0, 0.5))
        facts["checks"]["dsh_dir_back"] = (root / "dsh-home" / "profiles" / NAME).is_dir()
        facts["checks"]["app_dir_back"] = (root / "app-data" / "profiles" / NAME).is_dir()
        facts["checks"]["trash_emptied"] = not any(
            p.is_dir() and p.name.startswith(NAME) for p in app_trash.iterdir()
        ) if app_trash.is_dir() else True
        facts["checks"]["no_meta_in_profile"] = not (
            root / "app-data" / "profiles" / NAME / "trash.v1.json"
        ).exists()
        entry = next((p for p in registry(root).get("profiles", []) if p.get("name") == NAME), None)
        facts["restored_entry"] = entry
        facts["checks"]["label_preserved"] = bool(entry and entry.get("label") == LABEL)
        xs.shot(SHOTS / "02-restored.png")

        # --- 4. 恢复出来的 Profile 真的能切过去用 -----------------------
        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["popover_reopened"] = open_popover_strict(
            pid, xs, [LABEL.split()[0], LABEL, "标准", "专业"]
        )
        facts["checks"]["restored_listed_in_popover"] = LABEL in deep_text(pid)
        facts["checks"]["picked_restored"] = pick_and_confirm(pid, LABEL)
        facts["checks"]["switched_to_restored"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 30.0, 0.5)
        )
        facts["checks"]["restored_dock_has_scenes"] = all(
            s in deep_text(pid) for s in ("工作记录", "文案写作", "项目跟踪")
        )
        xs.shot(SHOTS / "03-switched-to-restored.png")
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
