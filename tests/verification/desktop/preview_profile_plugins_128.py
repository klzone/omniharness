#!/usr/bin/env python3
"""OD-ARCH-128：基础标准规则零写入 + 自建 Profile 声明插件（真实窗口）。

两轮，全程 /tmp 隔离数据根与一次性 Xvfb：
  1. 基础标准：规则资产页不许保存（提示干净底座、按钮禁用），点完也不得新建目录；
  2. 自建 Profile：资产页出现「声明插件」行；先输非法包名，被拒且登记表不动；
     再声明一个本地已就绪的包，只登记不联网，登记表出现该包。

只造合成数据，不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_plugins_128.py [--display :148] [--root /tmp/od128-preview]
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
sys.path.insert(0, str(HERE))

from preview_profile_switch_110 import deep_text, read_settings, wait_for  # noqa: E402
from preview_weekly_100 import start_dbus, start_display  # noqa: E402
from preview_profile_trash_125 import (  # noqa: E402
    click_text,
    dev_server_ready,
    open_assets_tab,
    open_popover_strict,
    pick_and_confirm,
    type_into,
)
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-128"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "plugin-desk"
LABEL = "Plugin Desk"
PKG = "@acme/plugin"


def replace_into(pid: int, xs: XSession, hint: str, value: str) -> bool:
    """点进输入框，清空旧内容再敲新值（第二次声明要覆盖上一次被拒的写法）。"""
    if not type_into(pid, xs, hint, ""):
        return False
    xs.select_all_and_clear()
    xs.type_ascii(value)
    return True


def registry(root: Path) -> dict:
    path = root / "app-data" / "profiles" / "index.v1.json"
    if not path.is_file():
        return {"profiles": []}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"profiles": []}


def entry(root: Path) -> dict | None:
    return next(
        (p for p in registry(root).get("profiles", []) if p.get("name") == NAME), None
    )


def free_display(start: int = 148, span: int = 40, used: set[str] | None = None) -> str:
    """挑一个真正空闲的显示号。

    start_display 只等 /tmp/.X11-unix/X<n> 出现，而上一轮异常退出会把这个套接字
    与 /tmp/.X<n>-lock 留在盘上；再用同一个号，它会在「文件已存在」的瞬间返回，
    实际连上的是一个已经不存在的服务，应用启动后 AT-SPI 全是「Could not obtain
    desktop path or name」，界面判定全部假失败（OD-ARCH-128 实测踩到）。
    这里两个文件都不在才算空闲。

    再加一条「本轮进程内用过的号不再复用」：Xvfb 被 terminate 后套接字要过一会儿
    才消失，中间这个空档里它看起来又是「空闲」的，第二轮就会连到一个正在关闭的
    服务器上（OD-ARCH-128 实测：第二轮 mount_window 90 秒找不到窗口）。
    """
    for n in range(start, start + span):
        if used is not None and f":{n}" in used:
            continue
        if Path(f"/tmp/.X11-unix/X{n}").exists():
            continue
        if Path(f"/tmp/.X{n}-lock").exists():
            continue
        return f":{n}"
    raise RuntimeError(f"{start}-{start + span - 1} 号显示都被占用")


def base_round(display: str, root: Path, bus_addr: str) -> dict:
    """第一轮：基础标准下规则保存被拒，且一个目录都不建。"""
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    facts: dict = {"root": str(root), "checks": {}}
    session = AppSession(
        root=root,
        display=display,
        reuse_running_dev_server=dev_server_ready(),
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "omnidesk-base",
        },
    )
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(display)
        win = mount_window(session, xs)
        xs.geometry(win)
        time.sleep(2.5)

        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["settings_opened"] = click_text(pid, "系统偏好设置")
        time.sleep(2.5)
        facts["checks"]["rules_tab_opened"] = click_text(pid, "规则资产")
        time.sleep(2.0)
        # WebKit 对设置页正文的 AT-SPI 暴露不稳定（109 已记录），拒绝理由以截图为证，
        # 真正的行为约束是下面「一个目录都不写」。这里顺手记一份可见文本供人工核对。
        text = deep_text(pid)
        facts["rules_text_sample"] = text[:4000]
        facts["blocked_hint_in_ax"] = "干净底座" in text
        # 点一下保存按钮（基础标准下应为禁用）；无论点没点中，都不得出现 Profile 目录。
        click_text(pid, "校验并保存", exact=False)
        time.sleep(2.0)
        profiles = root / "app-data" / "profiles"
        dsh_profiles = root / "dsh-home" / "profiles"
        facts["app_profile_dirs"] = (
            sorted(p.name for p in profiles.iterdir() if not p.name.startswith("."))
            if profiles.is_dir()
            else []
        )
        facts["dsh_profile_dirs"] = (
            sorted(p.name for p in dsh_profiles.iterdir()) if dsh_profiles.is_dir() else []
        )
        facts["checks"]["no_app_profile_written"] = facts["app_profile_dirs"] == []
        facts["checks"]["no_dsh_profile_written"] = facts["dsh_profile_dirs"] == []
        xs.shot(SHOTS / "01-rules-blocked.png")
    finally:
        session.close()
    return facts


def custom_round(display: str, root: Path, bus_addr: str) -> dict:
    """第二轮：自建 Profile 的「声明插件」入口与两条拒绝/受理路径。"""
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    facts: dict = {"root": str(root), "checks": {}}
    session = AppSession(
        root=root,
        display=display,
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
        xs = XSession(display)
        win = mount_window(session, xs)
        xs.geometry(win)
        time.sleep(2.5)

        # 先在干净底座上建一个自建 Profile，再切过去（沿用 127 的入口）。
        # 上一轮结束在设置页，这一轮先把界面拉回工作台首页再点顶栏徽标：
        # 设置页正文里带「标准」等字样，人还停在设置页时弹层判定会被干扰。
        facts["checks"]["home_ready"] = bool(
            wait_for(lambda: "工作台首页" in deep_text(pid), 40.0, 0.5)
        )
        for _ in range(3):
            if click_text(pid, "工作台首页"):
                break
            time.sleep(1.5)
        time.sleep(1.5)
        facts["checks"]["popover_opened"] = open_popover_strict(pid, xs, ["标准", "专业"])
        time.sleep(1.0)
        facts["checks"]["create_clicked"] = click_text(pid, "新建自定义 Profile", exact=False)
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        facts["checks"]["submitted"] = click_text(pid, "创建")
        facts["checks"]["created"] = bool(
            wait_for(lambda: entry(root) is not None, 30.0, 0.5)
        )
        click_text(pid, "取消")
        time.sleep(1.0)
        if not facts["checks"]["created"]:
            return facts

        # 切到这个 Profile：装配会写入它的配置与场景声明。
        facts["checks"]["switched"] = False
        for _ in range(4):
            if click_text(pid, "工作台首页"):
                time.sleep(1.5)
                break
        facts["checks"]["popover_reopened"] = open_popover_strict(
            pid, xs, [LABEL, "标准", "专业"]
        )
        facts["checks"]["picked"] = pick_and_confirm(pid, LABEL)
        facts["checks"]["switched"] = bool(
            wait_for(
                lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5
            )
        )

        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        geo = xs.geometry(win)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h // 2, 25, settle=0.5)
        time.sleep(1.0)
        facts["checks"]["declare_row_visible"] = bool(
            wait_for(
                lambda: "要声明并安装的插件包名" in deep_text(pid),
                20.0,
                0.5,
            )
        )

        # 非法包名：宿主必须拒绝，登记表保持原样。
        facts["checks"]["typed_bad_pkg"] = type_into(
            pid,
            xs,
            "要声明并安装的插件包名",
            "plain/path",
            alt="@scope/plugin 或 plugin-name",
        )
        facts["checks"]["bad_submitted"] = click_text(pid, "安装并登记", exact=False)
        time.sleep(3.0)
        facts["bad_text_sample"] = deep_text(pid)[:4000]
        facts["checks"]["bad_rejected_inline"] = "不是合法包名" in facts["bad_text_sample"]
        current = entry(root) or {}
        facts["plugins_after_bad"] = current.get("plugins")
        facts["checks"]["bad_left_registry_alone"] = current.get("plugins") == []
        xs.shot(SHOTS / "02-bad-package.png")

        # 本地已就绪的包：只登记、不联网安装。
        pkg_dir = root / "dsh-home" / "profiles" / NAME / "node_modules" / "@acme" / "plugin"
        pkg_dir.mkdir(parents=True, exist_ok=True)
        (pkg_dir / "package.json").write_text(
            json.dumps({"name": PKG, "version": "1.0.0"}), encoding="utf-8"
        )
        facts["checks"]["typed_good_pkg"] = replace_into(
            pid, xs, "要声明并安装的插件包名", PKG
        )
        facts["checks"]["good_submitted"] = click_text(pid, "安装并登记", exact=False)
        facts["checks"]["registered"] = bool(
            wait_for(lambda: PKG in ((entry(root) or {}).get("plugins") or []), 30.0, 0.5)
        )
        facts["plugins_after_good"] = (entry(root) or {}).get("plugins")
        facts["checks"]["registry_exactly_one"] = (entry(root) or {}).get("plugins") == [PKG]
        xs.shot(SHOTS / "03-package-declared.png")
    finally:
        session.close()
    return facts


PHASES = {"base": base_round, "custom": custom_round}


def run_phase(phase: str, display: str, root: Path, facts_out: Path | None) -> dict:
    """在一个独立进程里跑一轮。

    OD-ARCH-128 实测：两轮串在同一个进程里时，第二轮的应用有时能画出窗口，但进程
    会在中途静默退出（退出码 1、无 traceback，AT-SPI 侧也拿不到文本）；两轮各自
    独立进程跑同一份代码则稳定全过。原因是 GTK / AT-SPI / WebKit 在进程内的初始化
    与拆除只做一次，第二个 AppSession 复用的是已经拆过一半的状态。所以两轮分进程，
    父进程只负责挑显示号与合并结果。
    """
    SHOTS.mkdir(parents=True, exist_ok=True)
    xvfb = start_display(display, SCREEN_W, SCREEN_H + 2000)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)
    try:
        facts = PHASES[phase](display, root, bus_addr)
    finally:
        for proc in (bus_proc, xvfb):
            try:
                proc.terminate()
            except Exception:
                pass
        time.sleep(1.5)
    if facts_out is not None:
        facts_out.write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
    return facts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":148")
    ap.add_argument("--root", default="/tmp/od128-preview")
    ap.add_argument(
        "--phase",
        choices=["both", *PHASES],
        default="both",
        help="both：分两个子进程跑两轮；base/custom：只跑一轮（由父进程调用）",
    )
    ap.add_argument("--facts-out", default=None)
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    SHOTS.mkdir(parents=True, exist_ok=True)

    if args.phase != "both":
        facts = run_phase(
            args.phase,
            args.display,
            root / args.phase,
            Path(args.facts_out) if args.facts_out else None,
        )
        print(json.dumps(facts, ensure_ascii=False))
        return 0 if all(v is True for v in facts["checks"].values()) else 1

    facts: dict = {"display": args.display, "checks": {}}
    merged: dict = {}
    ok = True
    for phase in PHASES:
        display = free_display()
        facts_out = root / f"{phase}-facts.json"
        proc = subprocess.run(
            [
                sys.executable,
                str(Path(__file__).resolve()),
                "--phase",
                phase,
                "--display",
                display,
                "--root",
                str(root),
                "--facts-out",
                str(facts_out),
            ],
            capture_output=True,
            text=True,
            timeout=900,
        )
        section = {}
        if facts_out.is_file():
            section = json.loads(facts_out.read_text(encoding="utf-8"))
        else:
            ok = False
            section = {
                "checks": {"phase_ran": False},
                "stderr_tail": proc.stderr[-2000:],
            }
        merged[phase] = section
        facts["checks"].update(section.get("checks") or {})
        if proc.returncode != 0:
            ok = False
    facts.update(merged)
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False))
    return 0 if ok and all(v is True for v in facts["checks"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())

