#!/usr/bin/env python3
"""OD-ARCH-131：自建 Profile 能带自己的 Skill（真实窗口）。

两轮分进程，全程 /tmp 隔离数据根与一次性 Xvfb：
  1. 基础标准：规则资产页不出现「Profile 自带 Skill」区块，也不得建出 skills/ 目录；
  2. 自建 Profile：建一个、切过去，在绑定区生成一份自带 Skill 模板 →
     读盘确认落在 `<应用数据根>/profiles/<名字>/skills/<skill名>/SKILL.md`，
     模板出现在勾选片里 → 勾选 → 保存 → 断言绑定写进 rules.json，
     且运行时 `dsh-home/skills/<skill名>/SKILL.md` 的正文就是这份自带源。

只造合成数据，不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_skill_source_131.py [--display :159] [--root /tmp/od131-preview]
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
from preview_profile_trash_125 import click_text, dev_server_ready, type_into  # noqa: E402
from preview_profile_plugins_128 import free_display, open_popover_strict, pick_and_confirm  # noqa: E402
from preview_profile_skill_bindings_130 import open_rules_tab  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-131"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "own-desk"
LABEL = "Own Desk"
SKILL = "office-note"


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


def rules_file(root: Path) -> Path:
    return root / "app-data" / "profiles" / NAME / "rules.json"


def read_rules(root: Path) -> dict:
    path = rules_file(root)
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def base_round(display: str, root: Path, bus_addr: str) -> dict:
    """第一轮：基础标准下没有自带 Skill 区块，且一个 skills/ 目录都不建。"""
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
        facts["checks"]["rules_tab_opened"] = open_rules_tab(pid)
        geo = xs.geometry(win)
        for _ in range(3):
            xs.scroll(geo.x + geo.w // 2, geo.y + geo.h // 2, 25, settle=0.5)
        time.sleep(1.0)
        text = deep_text(pid)
        facts["rules_text_sample"] = text[:4000]
        facts["checks"]["source_block_hidden_in_base"] = "Profile 自带 Skill" not in text
        xs.shot(SHOTS / "01-base-no-source-block.png")

        app_profiles = root / "app-data" / "profiles"
        facts["app_profile_dirs"] = (
            sorted(p.name for p in app_profiles.iterdir() if not p.name.startswith("."))
            if app_profiles.is_dir()
            else []
        )
        facts["checks"]["no_app_profile_written"] = facts["app_profile_dirs"] == []
    finally:
        session.close()
    return facts


def custom_round(display: str, root: Path, bus_addr: str) -> dict:
    """第二轮：生成自带 Skill → 勾选 → 保存 → 读盘核对源与运行时正文。"""
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    facts: dict = {"root": str(root), "checks": {}}
    session = AppSession(
        root=root,
        display=display,
        reuse_running_dev_server=dev_server_ready(),
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(display)
        win = mount_window(session, xs)
        xs.geometry(win)
        time.sleep(2.5)

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
        facts["checks"]["create_clicked"] = click_text(
            pid, "新建自定义 Profile", exact=False
        )
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        facts["checks"]["submitted"] = click_text(pid, "创建")
        facts["checks"]["created"] = bool(wait_for(lambda: entry(root) is not None, 30.0, 0.5))
        click_text(pid, "取消")
        time.sleep(1.0)
        if not facts["checks"]["created"]:
            return facts

        for _ in range(4):
            if click_text(pid, "工作台首页"):
                time.sleep(1.5)
                break
        facts["checks"]["popover_reopened"] = open_popover_strict(pid, xs, [LABEL, "标准", "专业"])
        facts["checks"]["picked"] = pick_and_confirm(pid, LABEL)
        facts["checks"]["switched"] = bool(
            wait_for(
                lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5
            )
        )
        time.sleep(2.0)

        facts["checks"]["rules_tab_opened"] = open_rules_tab(pid)
        time.sleep(1.5)
        geo = xs.geometry(win)
        scroll = geo.x + geo.w // 2, geo.y + geo.h // 2
        for _ in range(4):
            xs.scroll(scroll[0], scroll[1], 30, settle=0.4)
        time.sleep(1.0)
        # WebKit 不把说明段落暴露给 AT-SPI（130 已实测），所以「区块在不在」以
        # 它能被操作的事实为准：名字输入框能定位并输入，就是区块渲染出来了。
        facts["checks"]["typed_skill"] = type_into(
            pid, xs, "新 Skill 名", SKILL, alt="新 Skill 名，如 office-note"
        )
        facts["checks"]["source_block_visible"] = facts["checks"]["typed_skill"]
        xs.shot(SHOTS / "02-source-block.png")
        time.sleep(0.6)
        facts["checks"]["scaffold_clicked"] = click_text(pid, "生成模板")
        source = root / "app-data" / "profiles" / NAME / "skills" / SKILL / "SKILL.md"
        facts["source_path"] = str(source)
        facts["checks"]["template_written"] = bool(
            wait_for(lambda: source.is_file(), 25.0, 0.5)
        )
        if source.is_file():
            facts["template_head"] = source.read_text(encoding="utf-8")[:400]
        xs.shot(SHOTS / "03-template-created.png")

        # 新 Skill 应当出现在勾选片里（allowed_skills = 内置 ∪ 自带）。
        facts["checks"]["chip_listed"] = bool(
            wait_for(lambda: SKILL in deep_text(pid), 20.0, 0.5)
        )
        facts["checks"]["chip_clicked"] = click_text(pid, SKILL)
        time.sleep(0.8)
        xs.shot(SHOTS / "04-chip-checked.png")

        facts["checks"]["saved"] = click_text(pid, "保存绑定", exact=False)
        after: dict = {}
        wait_for(
            lambda: bool(
                (after.update(read_rules(root)) or True)
                and any(
                    SKILL in ((v or {}).get("skills") or [])
                    for v in (after.get("skill_bindings") or {}).values()
                )
            ),
            30.0,
            0.5,
        )
        after = read_rules(root)
        facts["rules_after"] = after
        bound = {
            k: (v or {}).get("skills")
            for k, v in (after.get("skill_bindings") or {}).items()
            if SKILL in ((v or {}).get("skills") or [])
        }
        facts["scenes_bound"] = bound
        facts["checks"]["binding_saved"] = bool(bound)

        runtime = root / "dsh-home" / "skills" / SKILL / "SKILL.md"
        facts["runtime_path"] = str(runtime)
        facts["checks"]["skill_in_runtime"] = bool(
            wait_for(lambda: runtime.is_file(), 25.0, 0.5)
        )
        if runtime.is_file() and source.is_file():
            facts["checks"]["runtime_uses_profile_source"] = (
                runtime.read_text(encoding="utf-8") == source.read_text(encoding="utf-8")
            )
        xs.shot(SHOTS / "05-saved.png")
    finally:
        session.close()
    return facts


PHASES = {"base": base_round, "custom": custom_round}


def run_phase(phase: str, display: str, root: Path, facts_out: Path | None) -> dict:
    """一轮一个独立进程：GTK / AT-SPI / WebKit 在进程内只初始化一次。"""
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
        facts_out.write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
    return facts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":159")
    ap.add_argument("--root", default="/tmp/od131-preview")
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
    used: set[str] = set()
    for phase in PHASES:
        display = free_display(used=used)
        used.add(display)
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
        if facts_out.is_file():
            section = json.loads(facts_out.read_text(encoding="utf-8"))
        else:
            ok = False
            section = {"checks": {"phase_ran": False}, "stderr_tail": proc.stderr[-2000:]}
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
