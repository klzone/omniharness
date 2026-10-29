#!/usr/bin/env python3
"""OD-ARCH-130：保存绑定后立刻把 Skill 放进当前 Profile 的运行时目录（真实窗口）。

两轮分进程，全程 /tmp 隔离数据根与一次性 Xvfb：
  1. 基础标准：规则资产页不出现「场景 Skill 绑定」，点完也不得新建任何 Profile 目录；
  2. 自建 Profile：建一个、切过去，在绑定行里点三个 Skill → 保存 →
     读盘断言 skill_bindings 生效、global / scenes 一字未动、旧文件已备份；
     再点第 4 个（超出上限）确认它进不了文件；
     并断言这三个 Skill 目录**当场**就出现在 dsh-home/skills/ 下（不需要再切一次 Profile）。

只造合成数据，不读真实工作记录、知识库与凭证。

用法：python3 preview_profile_skill_bindings_130.py [--display :159] [--root /tmp/od130-preview]
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
    type_into,
)
from preview_profile_plugins_128 import free_display, open_popover_strict, pick_and_confirm  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-130"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "bind-desk"
LABEL = "Bind Desk"

# 按界面渲染顺序点这三个（ALLOWED_SKILLS 里 memory-agent < memory-report < anysearch）。
PICK = ["memory-agent", "memory-report", "anysearch"]
# 第 4 个：weekly 已绑满 3 个，按钮禁用，点了也不该进文件。
OVERFLOW = "knowledge-search"


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


def open_rules_tab(pid: int) -> bool:
    """设置页 → 规则资产。按钮文案是「规则资产」，不是「Profile 资产」。"""
    # 已经在规则资产页就直接算打开，别再点一次「系统偏好设置」把页面切走。
    if "校验并保存" in deep_text(pid):
        return True
    for _ in range(6):
        if click_text(pid, "系统偏好设置"):
            time.sleep(3.0)
            if click_text(pid, "规则资产"):
                # WebKit 不把说明段落暴露给 AT-SPI，只能拿页面上的按钮文案判定。
                if wait_for(
                    lambda: "恢复默认规则包" in deep_text(pid), 15.0, 1.0
                ):
                    return True
        time.sleep(2.0)
    return False


def base_round(display: str, root: Path, bus_addr: str) -> dict:
    """第一轮：基础标准下没有绑定编辑区，且一个目录都不建。"""
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
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h // 2, 25, settle=0.5)
        time.sleep(1.0)
        text = deep_text(pid)
        facts["rules_text_sample"] = text[:4000]
        facts["checks"]["editor_hidden_in_base"] = "场景 Skill 绑定" not in text
        xs.shot(SHOTS / "01-base-no-editor.png")

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
    finally:
        session.close()
    return facts


def custom_round(display: str, root: Path, bus_addr: str) -> dict:
    """第二轮：自建 Profile 里勾选三个 Skill、保存、读盘核对，再试超限的第 4 个。"""
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

        # 干净底座上建一个自建 Profile，再切过去。
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
        facts["checks"]["created"] = bool(
            wait_for(lambda: entry(root) is not None, 30.0, 0.5)
        )
        click_text(pid, "取消")
        time.sleep(1.0)
        if not facts["checks"]["created"]:
            return facts

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

        before = read_rules(root)
        facts["rules_before"] = before
        facts["checks"]["rules_existed"] = bool(before)

        facts["checks"]["rules_tab_opened"] = open_rules_tab(pid)
        time.sleep(1.5)
        geo = xs.geometry(win)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h // 2, 30, settle=0.5)
        time.sleep(1.0)
        facts["checks"]["editor_visible"] = bool(
            wait_for(lambda: "场景 Skill 绑定" in deep_text(pid), 20.0, 0.5)
        )
        xs.shot(SHOTS / "02-editor.png")

        clicks = {}
        for skill in PICK:
            clicks[skill] = click_text(pid, skill)
            time.sleep(0.8)
        facts["chip_clicks"] = clicks
        # 满 3 个后第 4 个按钮被禁用；点它不该产生任何效果。
        clicks[OVERFLOW] = click_text(pid, OVERFLOW, tries=2)
        time.sleep(1.0)
        xs.shot(SHOTS / "03-picked.png")

        facts["checks"]["saved"] = click_text(pid, "保存绑定", exact=False)
        after = {}
        if wait_for(
            lambda: bool(
                (after.update(read_rules(root)) or True)
                and after.get("skill_bindings", {}).get("weekly", {}).get("skills")
            ),
            30.0,
            0.5,
        ):
            pass
        after = read_rules(root)
        facts["rules_after"] = after
        weekly = (after.get("skill_bindings") or {}).get("weekly") or {}
        facts["weekly_skills"] = weekly.get("skills")
        facts["checks"]["weekly_bound"] = weekly.get("skills") == PICK
        facts["checks"]["max_injected_matches"] = weekly.get("max_injected") == 3
        facts["checks"]["overflow_rejected"] = OVERFLOW not in (weekly.get("skills") or [])
        # 起步骨架写的是 `global: {}`，保存时会被序列化成同一套默认值（空红线词、
        # 空铁律、手机号与身份证脱敏）。策略等价，比的是生效配置而不是字节。
        global_after = after.get("global") or {}
        facts["global_after"] = global_after
        facts["checks"]["global_untouched"] = (
            list(global_after.get("redline_keywords") or []) == []
            and list(global_after.get("writing_rules") or []) == []
            and (global_after.get("privacy") or {}).get("id_card") == "mask"
            and (global_after.get("privacy") or {}).get("phone") == "mask"
        )
        facts["checks"]["scenes_untouched"] = after.get("scenes") == before.get("scenes")
        backups = sorted(
            p.name
            for p in rules_file(root).parent.glob("rules.json.bak-*")
            if p.is_file()
        )
        facts["backups"] = backups
        facts["checks"]["backup_written"] = len(backups) >= 1

        # OD-ARCH-130：保存绑定这一下就该把 Skill 落进运行时目录，不靠下一次切换。
        skills_root = root / "dsh-home" / "skills"
        landed = {}
        for skill in PICK:
            dst = skills_root / skill / "SKILL.md"
            landed[skill] = dst.is_file()
        facts["runtime_skills"] = landed
        facts["checks"]["skills_in_runtime_dir"] = all(landed.values())
        facts["checks"]["managed_registry_written"] = (
            skills_root / ".omnidesk-managed"
        ).is_file()
        facts["runtime_skill_dirs"] = (
            sorted(p.name for p in skills_root.iterdir() if p.is_dir())
            if skills_root.is_dir()
            else []
        )
        # WebKit 不把 .hint 段落暴露给 AT-SPI，回执文案只留样，不作断言；
        # 「是否真的补上」以上面的目录事实为准。
        facts["note_text_sample"] = deep_text(pid)[:4000]
        xs.shot(SHOTS / "04-saved.png")
    finally:
        session.close()
    return facts


PHASES = {"base": base_round, "custom": custom_round}


def run_phase(phase: str, display: str, root: Path, facts_out: Path | None) -> dict:
    """一轮一个独立进程：GTK / AT-SPI / WebKit 在进程内只初始化一次（128 已实测）。"""
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
    ap.add_argument("--display", default=":159")
    ap.add_argument("--root", default="/tmp/od130-preview")
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
