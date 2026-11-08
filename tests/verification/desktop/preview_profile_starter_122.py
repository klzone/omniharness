#!/usr/bin/env python3
"""OD-ARCH-122：自建 Profile 的「起手骨架」与「体检看 Skill」真实窗口验收。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

一条链路走完：
  1. 以基础标准冷启动 → 顶栏新建一个「从基础标准起手」的 Profile；
  2. 断言应用数据根落下 decl.yml（四个标准场景）与 rules.json（零预注入）；
  3. 切过去 → Dock 出现工作记录 / 文案写作 / 项目跟踪，设置页「场景声明」可填起步模板；
  4. 在 rules.json 里加一条 Skill 绑定（模拟用户定制）→「Profile 资产」列出该 Skill，
     目录未就位时状态是「缺失」；
  5. 切回基础标准再切回来 → 装配把 Skill 补齐，资产页变成「就位」，底座仍干净。

用法：python3 preview_profile_starter_122.py [--display :137] [--root /tmp/od122-preview]
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

from preview_profile_custom_118 import (  # noqa: E402
    click_text,
    open_assets_tab,
    open_popover_any,
    registry,
    type_into,
)
from preview_profile_switch_110 import deep_text, pick_profile, read_settings, wait_for  # noqa: E402
from preview_weekly_100 import find, start_dbus, start_display  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-122"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "my-desk"
LABEL = "My Desk"
SKILL = "memory-agent"


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except Exception:
        return ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":137")
    ap.add_argument("--root", default="/tmp/od122-preview")
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
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 干净底座冷启动：环境变量优先级最高，必须留空才轮得到设置文件。
            "OMNIDESK_DSH_PROFILE": "",
        },
    )
    app_profile = root / "app-data" / "profiles" / NAME
    decl_path = app_profile / "decl.yml"
    rules_path = app_profile / "rules.json"
    skill_dir = root / "dsh-home" / "skills" / SKILL
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)
        geo = xs.geometry(win)
        time.sleep(2.5)

        # --- 1. 干净底座 → 新建「从基础标准起手」 -------------------------
        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["popover_opened"] = open_popover_any(pid, xs, ["基础标准", "标准"])
        facts["checks"]["create_form_opened"] = click_text(pid, "新建自定义 Profile", exact=False)
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(
            pid, xs, "新 Profile 名字", NAME, alt="名字，如 my-work"
        )
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "新 Profile 显示名", LABEL, alt="显示名（可选），如 我的工作台"
        )
        xs.shot(SHOTS / "01-create-filled.png")
        facts["checks"]["submitted"] = click_text(pid, "创建")
        facts["checks"]["created_in_registry"] = bool(
            wait_for(
                lambda: any(p.get("name") == NAME for p in registry(root).get("profiles", [])),
                30.0,
                0.5,
            )
        )

        # --- 2. 起手骨架真的落盘，且内容合规 ------------------------------
        facts["checks"]["decl_written"] = bool(wait_for(lambda: decl_path.is_file(), 20.0, 0.5))
        facts["checks"]["rules_written"] = bool(wait_for(lambda: rules_path.is_file(), 20.0, 0.5))
        decl_text = read_text(decl_path)
        facts["decl_sample"] = decl_text
        facts["checks"]["decl_has_schema"] = "schema: omnidesk.profile-decl.v1" in decl_text
        facts["checks"]["decl_has_label"] = f"display_name: {LABEL}" in decl_text
        facts["checks"]["decl_has_four_scenes"] = all(
            f"id: {sid}" in decl_text for sid in ("home", "report", "writing", "project")
        ) and "id: sessions" not in decl_text
        rules = json.loads(read_text(rules_path) or "{}")
        facts["rules_sample"] = read_text(rules_path)
        facts["checks"]["rules_zero_injection"] = rules.get("skill_bindings") == {}
        facts["checks"]["rules_own_profile"] = rules.get("profile") == NAME
        facts["checks"]["rules_schema_v2"] = rules.get("schema") == "omnidesk.profile-rules.v2"
        facts["checks"]["still_clean_base_before_switch"] = (
            (read_settings(root) or {}).get("engine_profile") is None
        )
        xs.shot(SHOTS / "02-created.png")

        # --- 3. 切过去：Dock 出现四个场景入口 ----------------------------
        facts["checks"]["switch_confirmed"] = click_text(pid, "确认")
        facts["checks"]["switched_to_custom"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5)
        )
        facts["checks"]["dock_has_scenes"] = bool(
            wait_for(
                lambda: all(s in deep_text(pid) for s in ("工作记录", "文案写作", "项目跟踪")),
                30.0,
                0.5,
            )
        )
        xs.shot(SHOTS / "03-switched-dock.png")

        # 设置 → 对话分类 → 场景声明：起步模板按钮可用，点了能生成文本。
        facts["checks"]["settings_opened"] = click_text(pid, "系统偏好设置")
        time.sleep(3.0)
        facts["checks"]["categories_tab"] = click_text(pid, "对话分类")
        time.sleep(2.0)
        facts["checks"]["decl_page_shows"] = bool(wait_for(lambda: "场景声明" in deep_text(pid), 15.0, 0.5))
        facts["checks"]["starter_button_clicked"] = click_text(pid, "填入起步模板")
        facts["checks"]["starter_note_shown"] = bool(
            wait_for(lambda: "起步模板" in deep_text(pid), 15.0, 0.5)
        )
        xs.shot(SHOTS / "04-starter-template.png")

        # --- 4. 用户定制：rules.json 绑一个 Skill → 体检要看得见 ----------
        rules["skill_bindings"] = {"writing": {"skills": [SKILL], "max_injected": 1}}
        rules_path.write_text(
            json.dumps(rules, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        # 清掉可能存在的目录，制造「声明了但没就位」的真实缺口。
        if skill_dir.is_symlink():
            skill_dir.unlink()
        elif skill_dir.exists():
            shutil.rmtree(skill_dir)
        facts["checks"]["skill_absent_before_inspect"] = not skill_dir.exists()

        facts["checks"]["assets_tab_opened"] = open_assets_tab(pid)
        time.sleep(1.0)
        click_text(pid, "重新读取")
        # 资产行（asset-row-title / 徽标 / 明细）整段不暴露给 AT-SPI（118 轮已记录同类
        # 问题），这里只能断言「页面正常渲染、没有读取错误」，行内容以截图 05 / 06 作
        # 视觉证据：「Skill · memory-agent 缺失 / 缺 1 个文件」→「就位 · 已就位」。
        facts["checks"]["assets_page_rendered"] = bool(
            wait_for(
                lambda: "PROFILE 资产" in deep_text(pid) and "读取失败" not in deep_text(pid),
                25.0,
                0.5,
            )
        )
        facts["assets_text_sample"] = deep_text(pid)[:3000]
        # 巡检只读：不能自己把目录建出来。
        facts["checks"]["inspect_wrote_nothing"] = not skill_dir.exists()
        xs.shot(SHOTS / "05-assets-missing-skill.png")

        # --- 5. 切回底座再切回来：装配补齐，体检转「就位」 -----------------
        click_text(pid, "工作台首页")
        time.sleep(1.5)
        facts["checks"]["popover_reopened"] = open_popover_any(pid, xs, [LABEL, NAME])
        facts["checks"]["picked_base"] = pick_profile(xs, pid, "基础标准", geo)
        facts["checks"]["back_to_base"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") is None, 30.0, 0.5)
        )
        facts["checks"]["base_has_no_scenes"] = "工作记录" not in deep_text(pid)
        facts["checks"]["popover_reopened_2"] = open_popover_any(pid, xs, ["基础标准", "标准"])
        facts["checks"]["picked_custom_again"] = pick_profile(xs, pid, LABEL, geo)
        facts["checks"]["switched_back"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5)
        )
        facts["checks"]["skill_materialized"] = bool(
            wait_for(lambda: (skill_dir / "SKILL.md").is_file(), 60.0, 1.0)
        )
        facts["checks"]["assets_tab_reopened"] = open_assets_tab(pid)
        time.sleep(1.0)
        click_text(pid, "重新读取")
        # 补齐后切换回执给出后端计数的「已装配 1 项（共 1 项）」，截图 06 显示徽标「就位」。
        facts["checks"]["receipt_reports_assembled"] = "已装配 1 项（共 1 项）" in deep_text(pid)
        facts["assets_text_ready"] = deep_text(pid)[:3000]
        xs.shot(SHOTS / "06-assets-ready-skill.png")
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
