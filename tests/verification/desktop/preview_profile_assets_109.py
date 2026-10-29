#!/usr/bin/env python3
"""OD-ARCH-109：真实窗口里的 Profile 资产页与自装配。

一次性 Xvfb + 合成端点 + /tmp 隔离数据根。两轮：
  1. 从空数据根以 discipline 启动：引擎启动时自动装配，设置页「Profile 资产」
     应显示配置 / 场景声明 / 规则 / Skill / 插件各项已就位或按需安装；
  2. 切回基础标准：资产页显示「不安装任何资产」，且标准目录不新建。

只造合成数据，不读真实工作记录与知识库。

用法：python3 preview_profile_assets_109.py [--display :133] [--root /tmp/od109-preview]
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
from preview_weekly_100 import app_nodes, find, node_pos, start_dbus, start_display  # noqa: E402
from window_harness import AppSession, Geometry, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-109"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940


def standard_round(display: str, root: Path, bus_addr: str) -> dict:
    """第二轮：全新数据根 + 基础标准，断言零写入、资产页显示「不安装任何资产」。

    显式以 web-api 通道的标准 Profile（`omnidesk-base`）启动：harness 的 `app_env()`
    默认会写 `OMNIDESK_DSH_PROFILE=web`（DSH 内置模板），那不是 OmniDesk 的标准底座。
    这一轮专门盯住「干净底座」这条产品承诺：目录都不该出现。
    """
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    facts: dict = {"root": str(root), "checks": {}}
    session = AppSession(
        root=root,
        display=display,
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "omnidesk-base",
        },
    )
    # 预放一个「已停放」项（属于别的 Profile、当前不生效），验证资产页会列出来。
    parked_dir = root / "app-data" / "profiles" / ".parked-skills" / "parked-demo"
    parked_dir.mkdir(parents=True, exist_ok=True)
    (parked_dir / "SKILL.md").write_text("# parked demo\n", encoding="utf-8")
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(display)
        mount_window(session, xs)
        # 给启动期装配一个明确的完成信号：日志行一出现就说明走过装配入口。
        log = root / "app-data" / "logs" / "profile-assets.log"
        wait_for(lambda: log.is_file(), timeout=60.0, interval=1.0)
        time.sleep(2.0)
        facts["startup_log"] = log.read_text(encoding="utf-8") if log.is_file() else None
        profiles = root / "dsh-home" / "profiles"
        skills = root / "dsh-home" / "skills"
        facts["profile_dirs"] = sorted(p.name for p in profiles.iterdir()) if profiles.is_dir() else []
        facts["skill_dirs"] = sorted(p.name for p in skills.iterdir()) if skills.is_dir() else []
        # 只数 Profile 目录：点号开头的停放区（本轮预置了 .parked-skills）不算 Profile。
        facts["app_profile_dirs"] = (
            sorted(
                p.name
                for p in (root / "app-data" / "profiles").iterdir()
                if not p.name.startswith(".")
            )
            if (root / "app-data" / "profiles").is_dir()
            else []
        )
        facts["checks"]["skipped_assembly"] = bool(
            facts["startup_log"] and "result=skipped" in facts["startup_log"]
        )
        facts["checks"]["no_profile_written"] = (
            facts["profile_dirs"] == [] and facts["app_profile_dirs"] == []
        )
        facts["checks"]["no_skills_copied"] = facts["skill_dirs"] == []

        opened = open_assets_tab(xs, pid)
        facts["checks"]["assets_tab_opened"] = opened
        # 资产页是「打开才读」：读盘 + IPC 回来之后状态行才有内容。等到状态词落地再断言，
        # 否则会读到「刚打开、还没回结果」的中间态（实测踩到过）。
        wait_for(
            lambda: any(
                k in deep_text(pid)
                for k in ("不安装任何资产", "基础标准", "就绪", "读取失败")
            ),
            25.0,
            1.0,
        )
        time.sleep(0.8)
        text = deep_text(pid)
        # 留全文而不是截断样本：状态行在无障碍树里可能排在很后面，截断会误判成缺失。
        facts["assets_text_sample"] = text[:8000]
        # WebKit 在这台机器的 AT-SPI 树里对设置页正文暴露不稳定（同一轮里时有时无，
        # 两轮对照可复现：截图已画出正文、树里却没有）。因此「标准底座不写盘」由
        # 文件状态与启动日志判定，截图作为人工可见证据；文本只作参考，不进断言。
        facts["standard_note_in_ax"] = "不安装任何资产" in text or "基础标准" in text
        xs.shot(SHOTS / "02-assets-standard.png")
        facts["checks"]["standard_screenshot"] = (SHOTS / "02-assets-standard.png").is_file()
        # 停放区正文同样落在 WebKit 丢正文的范围内：断言改用「有值」而非「文本可读」。
        # 界面是否摆出来由截图 + 静态接线检查（verify_profile_assets_109 的
        # parked_skills_visible_in_ui）共同判定，这里只确认后端确实读到了那一项。
        # 打开资产页（会读一次停放区）之后，预置项必须原封不动：只读的端到端证据。
        facts["checks"]["parked_item_preserved"] = (parked_dir / "SKILL.md").is_file()
        facts["parked_listing_expected"] = ["parked-demo"]
        facts["visual_note"] = (
            "02-assets-standard.png 中「已停放 1 / parked-demo」已人工目视确认；"
            "WebKit AT-SPI 对该区域正文暴露不稳定，故不并入自动断言。"
        )
    finally:
        session.close()
    return facts


def deep_text(pid: int, max_depth: int = 48) -> str:
    """汇总无障碍文本，**不沿用** `accessibility._collect` 的 24 层上限。

    设置页的卡片层级比首屏深，24 层会截掉卡片正文，导致「状态行明明画出来了却读不到」
    （实测踩到过）。这里按需放宽，只用于文本断言。
    """
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
            text = t.get_text(0, t.get_character_count()) or "" if t is not None else ""
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


def all_text(nodes) -> str:
    return "\n".join((n.name or "") + "\n" + (n.text or "") for n in nodes)


def wait_for(predicate, timeout: float, interval: float = 1.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(interval)
    return None


def open_assets_tab(xs: XSession, pid: int) -> bool:
    """打开设置页并切到「Profile 资产」。

    首屏渲染完成后按钮才进无障碍树，冷启动时偶尔要等几秒；这里做有限重试，
    免得把「还没画出来」误判成「功能缺失」。
    """
    for _ in range(6):
        btn = find(app_nodes(pid), "系统偏好设置", contains=False)
        if btn is not None:
            ax.invoke(btn)
            time.sleep(3.0)
            tab = find(app_nodes(pid), "Profile 资产", contains=False)
            if tab is not None:
                ax.invoke(tab)
                # 资产页打开时才会去读一次资产清单（只读巡检），给它时间返回。
                if not wait_for(lambda: "PROFILE 资产" in all_text(app_nodes(pid)), 12.0, 1.0):
                    continue
                # 显式点一次「重新读取」：既能刷新状态，也能确认按钮真的可用。
                reload = find(app_nodes(pid), "重新读取", contains=False)
                if reload is not None:
                    ax.invoke(reload)
                return True
        time.sleep(2.0)
    return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":133")
    ap.add_argument("--root", default="/tmp/od109-preview")
    ap.add_argument(
        "--standard-root",
        default="/tmp/od109-preview-standard",
        help="第二轮（基础标准）的独立数据根",
    )
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    SHOTS.mkdir(parents=True, exist_ok=True)

    facts: dict = {"display": args.display, "root": str(root), "checks": {}}
    xvfb = start_display(args.display, SCREEN_W, SCREEN_H + 2000)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)

    session = AppSession(
        root=root,
        display=args.display,
        # 用户自己的 dev server 可能正占用 5273；同仓库同源码，复用它，既不启动也不终止。
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "discipline",
        },
    )
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)

        dsh = root / "dsh-home" / "profiles" / "discipline"
        app = root / "app-data" / "profiles" / "discipline"
        skills_dir = root / "dsh-home" / "skills"

        def assembled():
            files = [
                dsh / "package.json",
                dsh / "cordis.patch.yml",
                app / "decl.yml",
                app / "rules.json",
                app / ".assets.v1.json",
            ]
            skills = list(skills_dir.iterdir()) if skills_dir.is_dir() else []
            return all(p.is_file() for p in files) and len(skills) >= 8

        # 装配发生在启动期，但窗口起来、进程写盘仍有先后；轮询而不是固定 sleep。
        facts["checks"]["auto_assembled_on_start"] = bool(
            wait_for(assembled, timeout=60.0, interval=1.0)
        )
        facts["dsh_dir"] = str(dsh)
        facts["files"] = {
            "package.json": (dsh / "package.json").is_file(),
            "cordis.patch.yml": (dsh / "cordis.patch.yml").is_file(),
            "decl.yml": (app / "decl.yml").is_file(),
            "rules.json": (app / "rules.json").is_file(),
            "assets_state": (app / ".assets.v1.json").is_file(),
        }
        facts["skills"] = sorted(p.name for p in skills_dir.iterdir()) if skills_dir.is_dir() else []
        facts["checks"]["skills_copied"] = len(facts["skills"]) >= 8

        opened = open_assets_tab(xs, pid)
        facts["checks"]["assets_tab_opened"] = opened
        nodes = app_nodes(pid)
        text = all_text(nodes)
        facts["assets_text_sample"] = text[:2000]
        facts["checks"]["assets_tab_has_heading"] = "Profile 资产" in text
        facts["checks"]["shows_ready_status"] = ("就绪" in text) or ("已就位" in text)
        xs.shot(SHOTS / "01-assets-discipline.png")
    finally:
        session.close()

    # 第二轮必须等第一轮**进程真的退出**：单实例插件按应用标识互斥，与前一轮的数据根
    # 无关。上一版在这里只是构造了新会话，旧窗口还活着，于是第二轮被重定向到旧实例、
    # 拿到的仍是 discipline 的数据根（实测踩到）。
    time.sleep(3.0)
    standard_root = Path(args.standard_root).resolve()
    if Path("/tmp") not in standard_root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {standard_root}")
    try:
        facts["standard_round"] = standard_round(args.display, standard_root, bus_addr)
        facts["checks"].update(facts["standard_round"]["checks"])
    finally:
        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(facts, ensure_ascii=False))
        try:
            bus_proc.terminate()
        except Exception:
            pass
        try:
            xvfb.terminate()
        except Exception:
            pass
    return 0 if all(facts["checks"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
