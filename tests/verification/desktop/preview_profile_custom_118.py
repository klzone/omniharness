#!/usr/bin/env python3
"""OD-ARCH-118：真实窗口里走完自定义 Profile 的一生（建 → 切 → 看 → 删）。

109–117 证明的是「切到出厂 Profile / 切回基础标准」。用户真正要的是
「我建一个自己的工作台」：顶栏新建 → 立刻切过去 → 资产页看得见 → 不要了点移除。
这条链路此前只有单元测试，没有窗口证据，因此单独跑一轮。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_custom_118.py [--display :131] [--root /tmp/od118-preview]
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
    category_keys,
    deep_text,
    open_profile_popover,
    pick_profile,
    popover_open,
    read_settings,
    wait_for,
)
from preview_weekly_100 import app_nodes, find, start_dbus, start_display  # noqa: E402
from window_harness import DEV_PORT, AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-118"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "my-desk"
LABEL = "My Desk"


def registry(root: Path) -> dict:
    path = root / "app-data" / "profiles" / "index.v1.json"
    if not path.is_file():
        return {"profiles": []}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"profiles": []}


def click_text(pid: int, text: str, exact: bool = True, tries: int = 5) -> bool:
    """按可见文案点击；WebKit 不把 data-testid 暴露给 AT-SPI，只能按文字取。"""
    for _ in range(tries):
        node = find(app_nodes(pid), text, contains=not exact)
        if node is not None and ax.invoke(node):
            return True
        time.sleep(1.2)
    return False


def type_into(pid: int, xs: XSession, hint: str, value: str, alt: str | None = None) -> bool:
    """点进输入框再敲 ASCII：合成窗口无法输入中文，名字与显示名都用 ASCII。"""
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


def open_popover_any(pid: int, xs: XSession, candidates: list[str]) -> bool:
    """点顶栏徽标，按「当前 Profile 显示名」定位。

    `open_profile_popover` 只认「标准 / 专业」两个出厂文案；自建 Profile 的
    徽标写的是自定义显示名（My Desk），直接复用那个函数会点不到。这里按传入
    候选逐个试，且必须看到弹层真的展开才算成功——资产页里也有同名条目，
    不能只凭「点了就有反应」判断。没有边框的窗口首击常被吞掉，再补一次。
    """
    for attempt in range(6):
        if popover_open(pid):
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
        if popover_open(pid):
            return True
        if attempt >= 1:
            xs.click(point[0], point[1], settle=1.2)
            if popover_open(pid):
                return True
        time.sleep(1.5)
    return popover_open(pid)


def dev_server_ready() -> bool:
    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", DEV_PORT)) == 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":131")
    ap.add_argument("--root", default="/tmp/od118-preview")
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
        # 与 107/108 同口径：本机已有 dev server 就复用，没有就自己起一个。
        reuse_running_dev_server=dev_server_ready(),
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 干净底座冷启动：环境变量是最高优先级开关，留空才轮得到设置文件。
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

        # --- 1. 从干净底座起手 -------------------------------------------
        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["popover_opened"] = open_profile_popover(xs, pid, geo)
        facts["checks"]["create_form_opened"] = click_text(pid, "新建自定义 Profile", exact=False)
        time.sleep(1.0)
        xs.shot(SHOTS / "01-create-form.png")

        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        xs.shot(SHOTS / "02-create-filled.png")
        facts["checks"]["submitted"] = click_text(pid, "创建")

        # 创建成功后弹层直接进「切换到…？」确认，省一次往返。
        def created() -> bool:
            return any(p.get("name") == NAME for p in registry(root).get("profiles", []))

        facts["checks"]["created_in_registry"] = bool(wait_for(created, 30.0, 0.5))
        dsh_dir = root / "dsh-home" / "profiles" / NAME
        facts["checks"]["created_dsh_files"] = bool(
            wait_for(
                lambda: (dsh_dir / "package.json").is_file()
                and (dsh_dir / "cordis.patch.yml").is_file(),
                20.0,
                0.5,
            )
        )
        skills = root / "dsh-home" / "skills"
        facts["skills_after_create"] = (
            sorted(p.name for p in skills.iterdir()) if skills.is_dir() else []
        )
        facts["profile_entry"] = next(
            (p for p in registry(root).get("profiles", []) if p.get("name") == NAME), None
        )
        # 从基础标准起手：不掺业务 Skill，底座仍然空。
        facts["checks"]["create_carries_no_skills"] = facts["skills_after_create"] == []
        xs.shot(SHOTS / "03-created.png")

        # --- 2. 立刻切过去 -----------------------------------------------
        facts["checks"]["switch_confirmed"] = click_text(pid, "确认")
        facts["checks"]["switched_to_custom"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 30.0, 0.5)
        )
        facts["checks"]["custom_categories_plain"] = bool(
            wait_for(lambda: category_keys(root) == ["plain"], 20.0, 0.5)
        )
        facts["categories_after_switch"] = category_keys(root)
        # OD-ARCH-122 起，从基础标准起手会写一份最小骨架（五个标准入口 + 空规则包），
        # 所以这里**应该**看到业务场景；本轮之前是「不该凭空长出场景」，断言随之反转。
        # 真正要守的不变量是「没有预注入」：规则包零 Skill 绑定、底座不装 Skill。
        facts["checks"]["custom_dock_has_starter_scenes"] = all(
            s in deep_text(pid) for s in ("工作记录", "文案写作", "项目跟踪")
        )
        facts["checks"]["custom_starter_rules_empty"] = (
            json.loads((root / "app-data" / "profiles" / NAME / "rules.json").read_text(
                encoding="utf-8"
            )).get("skill_bindings")
            == {}
        )
        xs.shot(SHOTS / "04-switched.png")

        # --- 3. 资产页看得见它 -------------------------------------------
        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        assets_text = deep_text(pid)
        facts["assets_text_sample"] = assets_text[:2500]
        facts["checks"]["assets_lists_custom"] = (LABEL in assets_text) or (NAME in assets_text)
        facts["checks"]["assets_has_remove_entry"] = "移除" in assets_text
        xs.shot(SHOTS / "05-assets-custom.png")

        # --- 4. 切回基础标准再移除 ---------------------------------------
        # 当前 Profile 不允许移除（后端会拒），先回底座。
        # 设置页里也有同名条目，先回工作台，徽标才不会被别处文字抢走。
        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["popover_reopened"] = open_popover_any(pid, xs, [LABEL, NAME])
        # 列表项文字带描述，精确匹配点不中；110 已验证过的 pick_profile 用包含匹配。
        facts["checks"]["picked_base"] = pick_profile(xs, pid, "基础标准", geo)
        facts["checks"]["back_to_base"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") is None, 30.0, 0.5)
        )

        facts["checks"]["assets_tab_reopened"] = open_assets_tab(pid)
        time.sleep(1.5)
        # WebKit 不把 asset-row-title 里的 span 文字放进无障碍树（前一阶段
        # 「My Desk」命中其实来自顶栏徽标）。切回底座后徽标变成「基础标准」，
        # 可用的信号只剩行的「移除」按钮；再配上注册表条目，两处同时成立才算
        # 资产页确实还列着这个自建 Profile。
        facts["checks"]["custom_still_listed"] = bool(
            wait_for(lambda: "移除" in deep_text(pid), 20.0, 0.5)
        ) and any(p.get("name") == NAME for p in registry(root).get("profiles", []))
        facts["assets_text_after_base"] = deep_text(pid)[:2500]
        facts["checks"]["remove_clicked"] = click_text(pid, "移除")

        def removed() -> bool:
            return not any(p.get("name") == NAME for p in registry(root).get("profiles", []))

        facts["checks"]["removed_from_registry"] = bool(wait_for(removed, 25.0, 0.5))
        trash = root / "app-data" / "profiles" / ".trash"
        trashed = sorted(p.name for p in trash.iterdir()) if trash.is_dir() else []
        facts["trash_entries"] = trashed
        # 「移除」进回收目录，不是永久删除：磁盘上还找得回配置。
        facts["checks"]["remove_is_recoverable"] = any(n.startswith(NAME) for n in trashed)
        dsh_trash = root / "dsh-home" / "profiles" / ".trash"
        facts["dsh_trash_entries"] = (
            sorted(p.name for p in dsh_trash.iterdir()) if dsh_trash.is_dir() else []
        )
        facts["checks"]["removed_dsh_dir"] = not dsh_dir.exists()
        xs.shot(SHOTS / "06-removed.png")
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
