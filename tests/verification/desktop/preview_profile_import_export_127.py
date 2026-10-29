#!/usr/bin/env python3
"""OD-ARCH-127：Profile 导出 / 导入（建 → 导出 → 移除 → 从包导入 → 切过去）。

真实窗口里验证：一套自建工作台能导出成单文件包，包落在数据根的 exports/ 里；
移除后在设置页的「迁移包」区块里一键导入回来，登记表、目录、场景入口都回来，
并且能切过去用。全程只造合成数据（全新 /tmp 隔离根）。

用法：python3 preview_profile_import_export_127.py [--display :147] [--root /tmp/od127-preview]
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
from preview_weekly_100 import find, start_dbus, start_display  # noqa: E402
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

EVID = HERE / "evidence" / "od-arch-127"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "migrate-desk"
LABEL = "Migrate Desk"


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


def bundles(root: Path) -> list[Path]:
    folder = root / "app-data" / "exports"
    if not folder.is_dir():
        return []
    return sorted(folder.glob("*.omniprofile.json"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":147")
    ap.add_argument("--root", default="/tmp/od127-preview")
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
        xs.geometry(win)
        time.sleep(2.5)

        # --- 1. 干净底座上建一个自建 Profile -----------------------------
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
        click_text(pid, "取消")
        time.sleep(1.0)

        # --- 2. 在资产页点行尾「导出」 -----------------------------------
        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.5)
        facts["checks"]["export_clicked"] = click_text(pid, "导出")
        facts["checks"]["bundle_written"] = bool(wait_for(lambda: bundles(root), 25.0, 0.5))
        facts["bundle_files"] = [p.name for p in bundles(root)]
        raw = json.loads(bundles(root)[0].read_text(encoding="utf-8")) if bundles(root) else {}
        facts["bundle_schema"] = raw.get("schema")
        facts["bundle_members"] = sorted(
            (f.get("root", "") + "/" + f.get("path", "")) for f in raw.get("files", [])
        )
        facts["checks"]["bundle_schema_ok"] = raw.get("schema") == "omnidesk.profile-bundle.v1"
        facts["checks"]["bundle_has_decl"] = "app/decl.yml" in facts["bundle_members"]
        facts["checks"]["bundle_has_rules"] = "app/rules.json" in facts["bundle_members"]
        facts["checks"]["bundle_has_package"] = "dsh/package.json" in facts["bundle_members"]
        facts["checks"]["bundle_no_node_modules"] = not any(
            "node_modules" in m for m in facts["bundle_members"]
        )
        xs.shot(SHOTS / "01-exported.png")

        # --- 3. 移除这套工作台（移入回收站） -----------------------------
        facts["checks"]["remove_clicked"] = click_text(pid, "移除")
        facts["checks"]["removed_from_registry"] = bool(
            wait_for(lambda: not registered(root), 25.0, 0.5)
        )
        facts["checks"]["bundle_survives_remove"] = bool(bundles(root))
        xs.shot(SHOTS / "02-removed.png")

        # --- 4. 从「迁移包」区块一键导入 ---------------------------------
        time.sleep(2.5)
        geo = xs.geometry(win)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h // 2, 25, settle=0.5)
        time.sleep(1.0)
        # WebKit 不把区块表头（"迁移包"）放进无障碍树，能取到的是行尾按钮。
        facts["checks"]["bundle_row_visible"] = bool(
            wait_for(lambda: "导入这套" in deep_text(pid), 20.0, 0.5)
        )
        facts["checks"]["import_clicked"] = click_text(pid, "导入这套", exact=False)
        facts["checks"]["imported_to_registry"] = bool(
            wait_for(lambda: registered(root), 40.0, 0.5)
        )
        facts["checks"]["dsh_dir_back"] = (root / "dsh-home" / "profiles" / NAME).is_dir()
        facts["checks"]["app_dir_back"] = (root / "app-data" / "profiles" / NAME).is_dir()
        facts["checks"]["decl_restored"] = (
            root / "app-data" / "profiles" / NAME / "decl.yml"
        ).is_file()
        facts["checks"]["no_import_leftover"] = not any(
            p.name.startswith(".import-")
            for p in (root / "dsh-home" / "profiles").iterdir()
        ) and not any(
            p.name.startswith(".import-")
            for p in (root / "app-data" / "profiles").iterdir()
        )
        entry = next(
            (p for p in registry(root).get("profiles", []) if p.get("name") == NAME), None
        )
        facts["imported_entry"] = entry
        facts["checks"]["label_preserved"] = bool(entry and entry.get("label") == LABEL)
        xs.shot(SHOTS / "03-imported.png")

        # --- 5. 导入的这套能切过去用 -------------------------------------
        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["popover_reopened"] = open_popover_strict(
            pid, xs, [LABEL, "标准", "专业"]
        )
        facts["checks"]["listed_in_popover"] = LABEL in deep_text(pid)
        facts["checks"]["picked"] = pick_and_confirm(pid, LABEL)
        facts["checks"]["switched"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5)
        )
        facts["checks"]["dock_has_scenes"] = all(
            s in deep_text(pid) for s in ("工作记录", "文案写作", "项目跟踪")
        )
        xs.shot(SHOTS / "04-switched-to-imported.png")
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

