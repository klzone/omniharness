#!/usr/bin/env python3
"""OD-ARCH-121：切换 Profile 后自动补齐缺失插件的真实窗口验收。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

验证两条路径：
  1. 开关默认开启 → 切到专业 Profile，切换立刻生效，缺的插件由后台补装，
     装完缺件卡自己消失；
  2. 在设置页关掉开关 → 再切一次（先删掉已装目录模拟缺失），界面只提示缺几项，
     不自动联网，用户仍可点「安装」。

用法：python3 preview_profile_autofill_121.py [--display :137] [--root /tmp/od121-preview]
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
    open_profile_popover,
    pick_profile,
    popover_open,
    read_settings,
    wait_for,
)
from preview_weekly_100 import app_nodes, find, start_dbus, start_display  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-121"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940


def make_forwarding_cli(root: Path, real: Path) -> Path:
    """造一个「带版本号的隔离锚点」，把命令转发给仓库内真实 DSH CLI。

    为什么必须这么干：插件探测的第一个候选目录就是 CLI 自己所在的 node_modules
    （见 profile_assets::candidates）。直接指向仓库运行时，那个目录里本来就有
    @deepseek-ai/dsh-skill-office，探测永远「已就绪」——缺件恒为 0，自动补齐这条
    路径根本不会被触发。锚点里只放 dsh 包的版本号，安装仍走真实 CLI 与真实网络。
    """
    pkg = root / "cli-anchor" / "node_modules/@deepseek-ai/dsh"
    (pkg / "lib").mkdir(parents=True, exist_ok=True)
    (pkg / "package.json").write_text(
        '{"name":"@deepseek-ai/dsh","version":"0.2.1-alpha.2"}', encoding="utf-8"
    )
    wrapper = (
        "const { spawnSync } = require('node:child_process');\n"
        f"const r = spawnSync(process.execPath, [{str(real)!r}, ...process.argv.slice(2)], "
        "{ stdio: 'inherit', env: process.env });\n"
        "process.exit(r.status === null ? 1 : r.status);\n"
    )
    bin = pkg / "lib/bin.js"
    bin.write_text(wrapper, encoding="utf-8")
    return bin


def click_text(pid: int, text: str, exact: bool = True, tries: int = 5) -> bool:
    """按可见文案点击；WebKit 不把 data-testid 暴露给 AT-SPI，只能按文字取。"""
    for _ in range(tries):
        node = find(app_nodes(pid), text, contains=not exact)
        if node is not None and ax.invoke(node):
            return True
        time.sleep(1.2)
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


def toggle_auto_install(pid: int, xs: XSession) -> bool:
    """点「（切换或启动 Profile 时）自动补齐缺失插件」开关。

    WebKit 把 label 与内部 input 分成两个无障碍节点，label 常没有可执行动作；
    先试 invoke，失败就按坐标点一次。

    124 之后这个开关同时管冷启动补装，标题从「切换 Profile 时…」改成
    「切换或启动 Profile 时…」；这里用后缀做包含匹配，两种标题都能命中。
    """
    label = "自动补齐缺失插件"
    for _ in range(4):
        node = find(app_nodes(pid), label, contains=True)
        if node is not None and ax.invoke(node):
            return True
        point = ax.focus_point(label, contains=True)
        if point is not None:
            xs.click(point[0] + 10, point[1], settle=1.0)
            return True
        time.sleep(1.5)
    return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":137")
    ap.add_argument("--root", default="/tmp/od121-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    SHOTS.mkdir(parents=True, exist_ok=True)

    real_cli = (
        Path(__file__).resolve().parents[3]
        / "runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js"
    )
    if not real_cli.is_file():
        raise SystemExit(f"找不到仓库内的 DSH CLI：{real_cli}")
    cli = make_forwarding_cli(root, real_cli)
    facts: dict = {
        "root": str(root),
        "display": args.display,
        "dsh_cli": str(cli),
        "checks": {},
    }
    xvfb = start_display(args.display, SCREEN_W, SCREEN_H + 2000)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)

    session = AppSession(
        root=root,
        display=args.display,
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 干净底座冷启动：环境变量是最高优先级开关，留空才轮得到设置文件。
            "OMNIDESK_DSH_PROFILE": "",
            # 探测要用隔离锚点，否则运行时自带的同名包会让「缺件」恒为 0。
            "OMNIDESK_DSH_CLI": str(cli),
        },
    )
    dsh = root / "dsh-home" / "profiles" / "discipline"
    plugin = dsh / "node_modules/@deepseek-ai/dsh-skill-office"
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)
        geo = xs.geometry(win)
        time.sleep(2.5)

        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        xs.shot(SHOTS / "01-base.png")

        # --- 路径 1：开关默认开启 → 后台自动补齐 -------------------------
        facts["checks"]["popover_opened"] = open_profile_popover(xs, pid, geo)
        facts["checks"]["picked_discipline"] = pick_profile(xs, pid, "纪检专业", geo)
        facts["checks"]["switch_saved"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == "discipline", 45.0, 0.5)
        )
        # 切换回执要当场说明「已在后台补齐」，用户才知道不用再点。
        facts["checks"]["receipt_says_background"] = bool(
            wait_for(lambda: "已在后台自动补齐" in deep_text(pid), 20.0, 0.5)
        )
        xs.shot(SHOTS / "02-switched-autofill.png")
        facts["checks"]["plugin_installed_in_background"] = bool(
            wait_for(lambda: (plugin / "package.json").is_file(), 180.0, 2.0)
        )
        facts["install_seconds"] = round(time.time() - (session.started_at or time.time()), 1)
        # 装齐后缺件卡整块消失：判据用「安装入口按钮」而不是标题文字，
        # WebKit 对纯文本段落与按钮暴露给无障碍树的方式不一样。
        facts["checks"]["missing_card_cleared"] = bool(
            wait_for(lambda: "安装缺失插件" not in deep_text(pid), 60.0, 1.0)
        )
        facts["checks"]["receipt_reports_done"] = bool(
            wait_for(lambda: "已在后台补齐" in deep_text(pid), 60.0, 1.0)
        )
        xs.shot(SHOTS / "03-autofill-done.png")

        # --- 路径 2：关掉开关 → 只提示不自动装 ---------------------------
        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        facts["checks"]["toggle_found"] = toggle_auto_install(pid, xs)
        facts["checks"]["switch_persisted_off"] = bool(
            wait_for(
                lambda: (read_settings(root) or {}).get("profile_auto_install_plugins") is False,
                20.0,
                0.5,
            )
        )
        xs.shot(SHOTS / "04-switch-off.png")

        # 删掉已装目录，模拟「这个包本地没有」。断开网络不现实，所以用删除代替。
        # pnpm 把依赖落成指向 store 的符号链接，rmtree 会拒绝，必须先 unlink。
        if plugin.is_symlink():
            plugin.unlink()
        elif plugin.exists():
            shutil.rmtree(plugin)
        facts["checks"]["plugin_removed_for_test"] = not plugin.exists()

        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["picked_base"] = open_profile_popover(xs, pid, geo) and pick_profile(
            xs, pid, "基础标准", geo
        )
        facts["checks"]["back_to_base"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") is None, 30.0, 0.5)
        )
        facts["checks"]["popover_reopened"] = open_profile_popover(xs, pid, geo)
        facts["checks"]["picked_discipline_again"] = pick_profile(xs, pid, "纪检专业", geo)
        facts["checks"]["switched_again"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == "discipline", 45.0, 0.5)
        )
        # 关掉开关时不许再自作主张联网：回执不出现「后台」，缺件卡留在界面上。
        time.sleep(6.0)
        text = deep_text(pid)
        facts["off_text_sample"] = text[:2000]
        facts["checks"]["no_background_claim"] = "已在后台自动补齐" not in text
        # 切换后界面会重读场景，弹层可能已经收起；缺件卡在弹层里，按用户的实际操作再打开一次。
        if "安装缺失插件" not in text:
            open_profile_popover(xs, pid, geo)
            time.sleep(1.5)
            text = deep_text(pid)
            facts["off_text_sample"] = text[:2000]
        facts["checks"]["missing_card_visible"] = "安装缺失插件" in text
        facts["checks"]["not_installed_when_off"] = not plugin.exists()
        xs.shot(SHOTS / "05-switch-off-missing.png")
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
