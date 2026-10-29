#!/usr/bin/env python3
"""OD-ARCH-110：真实窗口里切换 Profile 立即换工作台（一次性 Xvfb + 隔离根）。

一轮内做三件事，全程只重开窗口一次（切换本身不重启应用）：
  1. 以 `omnidesk-base` 冷启动：干净底座，Dock 只有平台入口，没有场景声明；
  2. 顶栏切到「纪检专业」：设置写入 discipline、资产装配到位、Dock 立刻出现专业场景；
  3. 切回「基础标准」：设置清空、场景退回干净底座。

只造合成数据，不读真实工作记录与知识库，不碰用户正在运行的实例。

用法：python3 preview_profile_switch_110.py [--display :135] [--root /tmp/od110-preview]
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
from preview_weekly_100 import app_nodes, find, start_dbus, start_display  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-110"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940


def deep_text(pid: int, max_depth: int = 48) -> str:
    """汇总无障碍文本；设置页 / Dock 层级比首屏深，24 层会截掉正文。"""
    from gi.repository import Atspi

    out: list[str] = []

    def walk(node, depth: int) -> None:
        if depth > max_depth:
            return
        try:
            name = node.get_name() or ""
        except Exception:
            name = ""
        try:
            t = node.get_text()
            text = (t.get_text(0, t.get_character_count()) or "") if t is not None else ""
        except Exception:
            text = ""
        if name or text:
            out.append(name + "\n" + text)
        try:
            count = node.get_child_count()
        except Exception:
            return
        for i in range(count):
            try:
                walk(node.get_child_at_index(i), depth + 1)
            except Exception:
                continue

    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        try:
            if app.get_process_id() != pid:
                continue
        except Exception:
            continue
        walk(app, 0)
        break
    return "\n".join(out)


def wait_for(predicate, timeout: float, interval: float = 0.5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(interval)
    return None


def battle(pid: int) -> str:
    return deep_text(pid)


def popover_open(pid: int) -> bool:
    """弹层是否已展开：列表项文字是可靠的可见信号。"""
    text = battle(pid)
    return "纪检专业" in text or "通道默认" in text or "基础标准" in text


def open_profile_popover(xs: XSession, pid: int, geo) -> bool:
    """点顶栏 Profile 徽标。

    WebKit 在这台机器上不把 `data-testid` 暴露到 AT-SPI，徽标在树里只是一个
    文本为「标准」/「专业」的按钮；因此按可见文案取坐标点击，而不是靠 testid。
    冷启动时首屏要几秒才进树，做有限重试。
    """
    for attempt in range(6):
        if popover_open(pid):
            return True
        point = (
            ax.focus_point("标准", role="push button")
            or ax.focus_point("专业", role="push button")
            or ax.focus_point("标准")
            or ax.focus_point("专业")
        )
        if point is not None:
            xs.click(point[0], point[1], settle=1.2)
        else:
            time.sleep(2.0)
            continue
        if popover_open(pid):
            return True
        # 无边框窗口首击常被吃掉（只交焦点），补一次。
        if attempt >= 1:
            xs.click(point[0], point[1], settle=1.2)
            if popover_open(pid):
                return True
        time.sleep(1.5)
    return popover_open(pid)


def pick_profile(xs: XSession, pid: int, label: str, geo) -> bool:
    """在弹层里点某个 Profile，再点确认。"""
    for _ in range(5):
        item = find(app_nodes(pid), label, contains=True)
        if item is not None:
            ax.invoke(item)
            time.sleep(1.0)
            if "确认" in battle(pid):
                break
        time.sleep(0.8)
    else:
        return False
    for _ in range(4):
        confirm = find(app_nodes(pid), "确认", contains=False)
        if confirm is not None and ax.invoke(confirm):
            return True
        time.sleep(0.8)
    return False


def read_settings(root: Path) -> dict | None:
    path = root / "app-data" / "config" / "settings.v1.json"
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def category_keys(root: Path) -> list[str]:
    """当前生效设置里的分类键（OD-ARCH-116：分类表随 Profile 走）。"""
    settings = read_settings(root)
    if settings is None:
        # 全新安装还没有设置文件：宿主默认值就是干净底座（只有未分类）。
        return ["plain"]
    cats = settings.get("thread_categories") or []
    return [c.get("key", "") for c in cats if isinstance(c, dict)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":135")
    ap.add_argument("--root", default="/tmp/od110-preview")
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
        # 端口 5273 空着就自己起一份 dev server（结束时由 stop_all 回收，不留后台进程）。
        # 若用户自己的实例正占着该端口，改成 True 即可复用同一份源码，不启动也不终止它。
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 以「干净底座」冷启动：这一刻不该有任何 Profile 资产。
            # 必须清空 `OMNIDESK_DSH_PROFILE`：环境变量是最高优先级开关，
            # 留着它就会把顶栏切换的结果盖掉（实测过的坑）。空串 = 未设置，
            # 于是 from_env 走「设置 → 通道默认」，无设置时就是 omnidesk-base。
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

        # --- 第 1 步：干净底座 -------------------------------------------
        base_text = battle(pid)
        facts["base_text_sample"] = base_text[:2500]
        facts["checks"]["boot_base_settings_null"] = (read_settings(root) or {}).get(
            "engine_profile"
        ) is None
        facts["checks"]["boot_base_no_scenes"] = "工作记录" not in base_text
        facts["checks"]["boot_base_no_assets"] = not (root / "dsh-home" / "profiles").joinpath(
            "discipline"
        ).exists()
        # OD-ARCH-116：干净底座的分类表只有「未分类」，不挂业务分类。
        facts["checks"]["boot_base_categories_only_plain"] = category_keys(root) == ["plain"]
        facts["base_categories"] = category_keys(root)
        xs.shot(SHOTS / "01-base-dock.png")

        # --- 第 2 步：切到纪检专业 ---------------------------------------
        facts["checks"]["popover_opened"] = open_profile_popover(xs, pid, geo)
        xs.shot(SHOTS / "02-popover.png")
        facts["checks"]["pick_discipline"] = pick_profile(xs, pid, "纪检专业", geo)
        xs.shot(SHOTS / "03-confirm.png")

        dsh = root / "dsh-home" / "profiles" / "discipline"
        app = root / "app-data" / "profiles" / "discipline"
        skills = root / "dsh-home" / "skills"

        def switched():
            files = [
                dsh / "package.json",
                dsh / "cordis.patch.yml",
                app / "decl.yml",
                app / "rules.json",
                app / ".assets.v1.json",
            ]
            skill_dirs = list(skills.iterdir()) if skills.is_dir() else []
            return (
                (read_settings(root) or {}).get("engine_profile") == "discipline"
                and all(p.is_file() for p in files)
                and len(skill_dirs) >= 8
            )

        facts["checks"]["switch_assembled"] = bool(wait_for(switched, 45.0, 0.5))
        facts["settings_after_switch"] = read_settings(root)
        facts["files"] = {
            "package.json": (dsh / "package.json").is_file(),
            "cordis.patch.yml": (dsh / "cordis.patch.yml").is_file(),
            "decl.yml": (app / "decl.yml").is_file(),
            "rules.json": (app / "rules.json").is_file(),
        }
        facts["skills"] = sorted(p.name for p in skills.iterdir()) if skills.is_dir() else []
        facts["checks"]["discipline_categories_restored"] = bool(
            wait_for(
                lambda: category_keys(root) == ["plain", "weekly", "writing", "project"],
                20.0,
                0.5,
            )
        )
        facts["checks"]["discipline_categories_file"] = (
            app / "categories.json"
        ).is_file()

        # 不重启应用：同一个窗口里 Dock 应已换成专业场景。
        facts["checks"]["dock_switched_without_restart"] = bool(
            wait_for(lambda: "工作记录" in battle(pid), 25.0, 0.5)
        )
        facts["after_switch_text_sample"] = battle(pid)[:2500]
        xs.shot(SHOTS / "04-dock-discipline.png")

        # --- 第 3 步：切回基础标准 ---------------------------------------
        facts["checks"]["popover_reopened"] = open_profile_popover(xs, pid, geo)
        facts["checks"]["pick_base"] = pick_profile(xs, pid, "基础标准", geo)
        facts["checks"]["switch_back_saved"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") is None, 25.0, 0.5)
        )
        facts["checks"]["dock_back_to_base"] = bool(
            wait_for(lambda: "工作记录" not in battle(pid), 20.0, 0.5)
        )
        facts["checks"]["base_categories_only_plain_again"] = bool(
            wait_for(lambda: category_keys(root) == ["plain"], 20.0, 0.5)
        )
        # OD-ARCH-115：底座干净不能只看 Dock——DSH 会把 <dsh-home>/skills/ 当全局
        # 用户根无条件扫描，业务 Skill 留在那里就等于还在加载。切回基础后必须真空。
        parked = root / "app-data" / "profiles" / ".parked-skills"
        facts["checks"]["base_skills_really_parked"] = bool(
            wait_for(
                lambda: (not skills.is_dir() or not list(skills.iterdir()))
                and parked.is_dir()
                and len([p for p in parked.iterdir() if p.is_dir()]) >= 8,
                20.0,
                0.5,
            )
        )
        facts["skills_after_base"] = (
            sorted(p.name for p in skills.iterdir()) if skills.is_dir() else []
        )
        facts["parked_skills"] = (
            sorted(p.name for p in parked.iterdir() if p.is_dir()) if parked.is_dir() else []
        )
        xs.shot(SHOTS / "05-dock-base-again.png")
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
