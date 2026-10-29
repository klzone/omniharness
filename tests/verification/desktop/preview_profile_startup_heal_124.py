#!/usr/bin/env python3
"""OD-ARCH-124：已选 Profile 冷启动时自动补齐缺失插件的真实窗口验收。

只造合成数据（全新 /tmp 隔离根 + 桩安装通道），不读真实工作记录、知识库与凭证。
不联网：桩 CLI 收到 `plugin add` 就把包写进 Profile 的 node_modules，
这样验收的是「启动发现缺件 → 后台补装 → 事件回报 → 界面收敛」这条链路，
而不是 npm registry 的可用性。

三条路径：
  1. 已选「纪检专业」且包被删除 → 冷启动后台补回，日志留 startup-autofill；
  2. 基础标准冷启动 → 不装、不写目录（干净底座承诺）；
  3. 开关关闭 → 冷启动只探测不装，包仍然缺，安装入口仍可用。

用法：python3 preview_profile_startup_heal_124.py [--display :139] [--root /tmp/od124-preview]
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

from preview_profile_switch_110 import (  # noqa: E402
    deep_text,
    read_settings,
    wait_for,
)
from preview_weekly_100 import start_dbus, start_display  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-124"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
PROFILE = "discipline"
PACKAGE = "@deepseek-ai/dsh-skill-office"
INSTALL_LOG = "install-calls.txt"


def make_stub_cli(root: Path, real: Path) -> Path:
    """桩 CLI：不联网，收到 plugin add 就把包落到 Profile 的 node_modules。

    之所以不用真 CLI：真安装 100 秒以上且依赖 registry，验收会变成网络测试。
    桩只实现 `plugin --profile <p> add <pkg>` 这一条被验证的路径，
    并记录每次调用，供断言「确实发起了安装」以及「开关关闭时一次都没发起」。
    """
    pkg = root / "cli-anchor" / "node_modules/@deepseek-ai/dsh"
    (pkg / "lib").mkdir(parents=True, exist_ok=True)
    (pkg / "package.json").write_text(
        '{"name":"@deepseek-ai/dsh","version":"0.2.1-alpha.2"}', encoding="utf-8"
    )
    log = root / INSTALL_LOG
    # 不用 f-string：桩脚本里满是 JS 大括号，f-string 会把 `{` 当成替换字段，
    # 在 3.12+ 还会跨行吞掉后面的语句。改用一次性占位替换，字面量更安全。
    dump_json = json.dumps({"plugins": [], "agent": {}})
    template = """#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const argv = process.argv.slice(2);
const log = __LOG__;
fs.appendFileSync(log, argv.join(' ') + '\\n');
// 引擎探测配置会跑 --dump-config：给一份最小可解析的 JSON，不当成安装调用。
if (argv.includes('--dump-config')) {
  console.log(__DUMP__);
  process.exit(0);
}
if (argv[0] !== 'plugin') process.exit(0);
const i = argv.indexOf('--profile');
const profile = i >= 0 ? argv[i + 1] : null;
const spec = argv[argv.length - 1] || '';
const name = spec.split('@').length > 2 ? '@' + spec.slice(1).split('@')[0] : spec.split('@')[0];
const home = process.env.DSH_HOME || '';
if (!profile || !home) {
  console.error('stub: missing profile or DSH_HOME');
  process.exit(2);
}
const dir = path.join(home, 'profiles', profile, 'node_modules', name);
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name, version: '0.2.1-alpha.2' }));
console.log('stub installed ' + name + ' into ' + profile);
"""
    script = template.replace("__LOG__", json.dumps(str(log))).replace(
        "__DUMP__", json.dumps(dump_json)
    )
    bin_path = pkg / "lib/bin.js"
    bin_path.write_text(script, encoding="utf-8")
    bin_path.chmod(0o755)
    return bin_path


def plugin_dir(root: Path, profile: str = PROFILE) -> Path:
    return root / "dsh-home" / "profiles" / profile / "node_modules" / PACKAGE


def remove_plugin(path: Path) -> None:
    """pnpm 把依赖落成符号链接；先 unlink 再删，否则 shutil 会拒绝。"""
    if path.is_symlink():
        path.unlink()
    elif path.exists():
        shutil.rmtree(path)


def read_log(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def install_calls(root: Path) -> list[str]:
    """只数真正的 `plugin add`。

    桩 CLI 同时会收到引擎的 `--dump-config`（宿主每次启动都会探测配置），
    把它算进安装调用会让「开关关闭时不安装」永远为假——那是断言口径错，不是行为错。
    """
    return [
        ln
        for ln in read_log(root / INSTALL_LOG).splitlines()
        if ln.startswith("plugin ") and " add " in ln
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":139")
    ap.add_argument("--root", default="/tmp/od124-preview")
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
    cli = make_stub_cli(root, real_cli)

    facts: dict = {
        "root": str(root),
        "display": args.display,
        "dsh_cli": str(cli),
        "checks": {},
        "notes": {},
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
            # 冷启动要读设置文件里的 Profile，而不是被环境变量钉死。
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_DSH_CLI": str(cli),
        },
    )
    app_settings = root / "app-data" / "config" / "settings.v1.json"
    log_file = root / "app-data" / "logs" / "profile-assets.log"
    try:
        session.prepare()
        # prepare() 只建数据根，不建 config：写设置前自己补上这一层。
        app_settings.parent.mkdir(parents=True, exist_ok=True)
        # --- 0. 准备「上次已经选了纪律专业」的设置 → 首启即自动切换 --
        # 冷启动意味着 Profile 目录上次就已经存在（否则 resolve 会拒绝启动），
        # 这里预置最小清单来还原这个真实前提；缺的是插件，不是 Profile 本身。
        seeded = root / "dsh-home" / "profiles" / PROFILE / "package.json"
        seeded.parent.mkdir(parents=True, exist_ok=True)
        seeded.write_text(
            json.dumps({"name": "omnidesk-discipline-profile", "dsh": {"profile": {}}}),
            encoding="utf-8",
        )
        app_settings.write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "engine_profile": PROFILE,
                    "profile_auto_install_plugins": True,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)
        geo = xs.geometry(win)
        time.sleep(2.5)
        # 判据用启动装配日志里的实际生效 Profile，而不是「设置文件里写了什么」：
        # 设置可以写而实际回落标准，那才是真正的失效形态。
        facts["checks"]["boot_uses_saved_profile"] = bool(
            wait_for(lambda: f"profile={PROFILE}" in read_log(log_file), 60.0, 1.0)
        )
        # 启动装配会写出 Profile 的清单与 Skill；插件由本轮新增的自愈负责。
        facts["checks"]["profile_assets_booted"] = bool(
            wait_for(
                lambda: (root / "dsh-home" / "profiles" / PROFILE / "package.json").is_file(),
                90.0,
                1.0,
            )
        )
        facts["checks"]["plugin_healed_on_startup"] = bool(
            wait_for(lambda: (plugin_dir(root) / "package.json").is_file(), 90.0, 1.0)
        )
        facts["notes"]["install_calls_first_boot"] = install_calls(root)
        facts["checks"]["install_invoked_by_stub"] = bool(install_calls(root))
        facts["checks"]["startup_log_records_autofill"] = "startup-autofill" in read_log(log_file)
        facts["startup_log"] = read_log(log_file)[:2000]
        xs.shot(SHOTS / "01-cold-start-healed.png")

        # --- 1. 删掉插件后重启：已选 Profile 的缺件必须自愈 ----------------
        calls_before = len(install_calls(root))
        remove_plugin(plugin_dir(root))
        facts["checks"]["plugin_removed_before_restart"] = not plugin_dir(root).exists()
        session.stop_app()
        time.sleep(2.0)
        log_before = read_log(log_file)
        session.start()
        pid = session.proc.pid if session.proc else None
        win = mount_window(session, xs)
        facts["checks"]["plugin_healed_after_restart"] = bool(
            wait_for(lambda: (plugin_dir(root) / "package.json").is_file(), 90.0, 1.0)
        )
        facts["notes"]["install_calls_second_boot"] = install_calls(root)
        facts["checks"]["second_boot_installed_again"] = len(install_calls(root)) > calls_before
        facts["checks"]["second_boot_logged"] = "startup-autofill" in read_log(log_file)[len(log_before):]
        xs.shot(SHOTS / "02-restart-healed.png")

        # --- 2. 开关关闭：只探测不联网 -----------------------------------
        calls_before = len(install_calls(root))
        remove_plugin(plugin_dir(root))
        settings = read_settings(root) or {}
        settings["profile_auto_install_plugins"] = False
        app_settings.write_text(json.dumps(settings, ensure_ascii=False, indent=2), encoding="utf-8")
        session.stop_app()
        time.sleep(2.0)
        session.start()
        pid = session.proc.pid if session.proc else None
        win = mount_window(session, xs)
        time.sleep(12.0)
        facts["checks"]["no_install_when_switch_off"] = len(install_calls(root)) == calls_before
        facts["checks"]["plugin_stays_missing_when_off"] = not plugin_dir(root).exists()
        facts["off_text_sample"] = deep_text(pid)[:1500]
        xs.shot(SHOTS / "03-switch-off-no-install.png")

        # --- 3. 基础标准：冷启动不探测、不建目录、不装 ---------------------
        settings = read_settings(root) or {}
        settings["engine_profile"] = None
        settings["profile_auto_install_plugins"] = True
        app_settings.write_text(json.dumps(settings, ensure_ascii=False, indent=2), encoding="utf-8")
        calls_before = len(install_calls(root))
        session.stop_app()
        time.sleep(2.0)
        session.start()
        pid = session.proc.pid if session.proc else None
        win = mount_window(session, xs)
        time.sleep(10.0)
        facts["checks"]["base_no_install"] = len(install_calls(root)) == calls_before
        base_dir = root / "app-data" / "profiles" / "omnidesk-base"
        facts["checks"]["base_no_app_profile_dir"] = not base_dir.exists()
        facts["checks"]["base_uses_standard"] = "profile=omnidesk-base" in read_log(log_file)
        facts["base_text_sample"] = deep_text(pid)[:1500]
        xs.shot(SHOTS / "04-base-clean.png")
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
