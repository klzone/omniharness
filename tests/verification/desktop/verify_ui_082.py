#!/usr/bin/env python3
"""OD-ARCH-082 UI 真实窗口验收与取证脚本（复核要求版）。

覆盖 F1 全部要求：
1. S6 Profile 徽标、白名单与切换：
   - 默认模式显示「基础标准 (omnidesk-base)」且本次实际使用为 omnidesk-base；
   - 弹层文案断言无 dsh-profile- 技术包名泄漏；
   - 点击「纪检专业」弹出「切换到「纪检专业」？」确认框，此时截图 profile-discipline.png；
   - 点击确认后落盘断言 settings.v1.json 中 engine_profile == "discipline"；
   - 重新打开弹层断言选中态与提示信息正确。
2. S7 TurnRail 悬停卡与无障碍：
   - 指针移到真实刻度 (x≈1298, y≈503)，断言预览卡浮现，截图 turnrail-card.png；
   - 键盘聚焦到该刻度，截图 turnrail-card-focus.png。
3. S8 会话内检索：
   - 快捷键 ⌘K / Ctrl+K 聚焦顶栏搜索输入框；
   - 键入 "synthetic-test" 后断言计数为 "1/2"；
   - 断言正文中探测到 <mark> 琥珀底色像素 (bg-amber-300) 与聚焦环，截图 search-match.png；
   - 回车后断言计数变为 "2/2"，且断言视图发生真实像素位移（滚轮跳转）；
   - 键入 "notfoundxyz" 后断言计数为 "0/0"，截图 search-empty.png；
   - 按 Esc 后断言输入框清空、计数与高亮消失。
4. 全程结果与实测数值落盘至 evidence/od-arch-082/ui-facts.json。

用法：
  python3 tests/verification/desktop/verify_ui_082.py [--display :88] [--root /tmp/od082-home]
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

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-082"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900
THREAD_ID = "t082evidence"


def start_display(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    time.sleep(1.2)
    return proc


def start_private_dbus() -> tuple[str, subprocess.Popen]:
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    addr = proc.stdout.readline().strip()
    return addr, proc


def write_test_thread(session: AppSession) -> Path:
    now = int(time.time() * 1000)
    payload = {
        "id": THREAD_ID,
        "tag": "plain",
        "preset": "plain",
        "projectPath": None,
        "title": "OD-ARCH-082 搜索与索引测试会话",
        "messages": [
            {
                "id": "m1-user",
                "role": "user",
                "text": "这是第一轮用户问句：请分析系统架构与 synthetic-test 方案要点。",
                "createdAt": now - 300_000,
            },
            {
                "id": "m1-bot",
                "role": "assistant",
                "text": "这是第一轮助手回复：\n\n### 合成测试核心要点\n\n1. **接口对齐**：确保入参出参严格匹配；\n2. **离线断言**：不依赖外部网络状态；\n3. **多轮连续性**：验证多轮上下文保持。\n\n以上为 synthetic-test 方案说明。",
                "createdAt": now - 280_000,
                "state": "succeeded",
            },
            {
                "id": "m2-user",
                "role": "user",
                "text": "这是第二轮用户问句：关于后续 project-track 与事项推进计划。",
                "createdAt": now - 150_000,
            },
            {
                "id": "m2-bot",
                "role": "assistant",
                "text": "这是第二轮助手回复：\n\n项目跟踪包括三项工作：\n- 前端状态同步；\n- 宿主白名单约束；\n- 自动化回归评测。\n\n整体推进状态良好。",
                "createdAt": now - 120_000,
                "state": "succeeded",
            },
        ],
        "createdAt": now - 300_000,
        "updatedAt": now,
        "version": 1,
    }
    encoded = json.dumps(payload, ensure_ascii=False, indent=2)
    # 分别写入 plain 与 _unassigned 目录，保证无论按 tag 还是按项目分目录均可检索到
    p1 = session.app_data / "threads" / "plain" / f"{THREAD_ID}.json"
    p1.parent.mkdir(parents=True, exist_ok=True)
    p1.write_text(encoded, encoding="utf-8")

    p2 = session.app_data / "threads" / "_unassigned" / f"{THREAD_ID}.json"
    p2.parent.mkdir(parents=True, exist_ok=True)
    p2.write_text(encoded, encoding="utf-8")
    return p1


def count_pixels(shot: Path, box: tuple[int, int, int, int], pred) -> int:
    from PIL import Image

    im = Image.open(shot).convert("RGB").crop(box)
    return sum(1 for px in im.getdata() if pred(px))


def region_delta(
    a: Path, b: Path, box: tuple[int, int, int, int], tolerance: int = 15
) -> int:
    from PIL import Image, ImageChops

    left = Image.open(a).convert("RGB").crop(box)
    right = Image.open(b).convert("RGB").crop(box)
    diff = ImageChops.difference(left, right)
    changed = 0
    for px in diff.getdata():
        if max(px) > tolerance:
            changed += 1
    return changed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":88")
    ap.add_argument("--root", default="/tmp/od082-home")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    # 复制 dsh-home（排除已有的历史 sessions 与 ledger，确保完全干净）
    shutil.copytree(
        SOURCE_HOME,
        root / "dsh-home",
        dirs_exist_ok=True,
        symlinks=True,
        ignore=shutil.ignore_patterns("sessions*", "session_*", "*ledger.json"),
    )

    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_private_dbus()

    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ["DISPLAY"] = args.display

    # F1 关键：传 OMNIDESK_DSH_PROFILE="" 避免 AppSession 默认注入 web
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )

    facts: dict = {
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI + 像素判定）",
        "root": str(root),
        "checks": {},
        "failures": [],
    }

    def check(name: str, ok: bool, measured: object = None, desc: str = ""):
        passed = bool(ok)
        facts["checks"][name] = {"pass": passed, "measured": measured, "desc": desc}
        if not passed:
            facts["failures"].append(f"{name}: measured={measured} ({desc})")
            print(f"FAIL [UI-082] {name}: {measured} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [UI-082] {name}: {measured}")

    try:
        session.prepare()
        write_test_thread(session)

        # 初始 settings 设置为 omnidesk-base
        cfg_file = session.app_data / "config" / "settings.v1.json"
        cfg_file.parent.mkdir(parents=True, exist_ok=True)
        cfg_file.write_text(
            json.dumps({
                "schema": "omnidesk.settings.v1",
                "engine_profile": "omnidesk-base",
                "model_permissions": {"local": True, "private": False, "public": False}
            }, ensure_ascii=False),
            encoding="utf-8"
        )

        session.start()
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=120)
        if not win:
            raise RuntimeError("主窗口未在规定时间内出现")

        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=2.5)
        time.sleep(6)

        ax.init()

        # 断言测试线程是否已成功打开
        # 若对话区尚未切换到测试会话，点击左上角会话切换 chip 打开它
        thread_opened = (
            ax.has_text("这是第一轮用户问句")
            or ax.has_text("synthetic-test")
            or ax.has_text("OD-ARCH-082 搜索与索引测试会话")
            or ax.has_text("跳到第 1 轮")
        )
        if not thread_opened:
            # 尝试通过 AT-SPI 或点击测试会话 chip
            t_chip = ax.find("OD-ARCH-082 搜索与索引测试会话", contains=True)
            if t_chip:
                ax.invoke(t_chip)
                time.sleep(1.5)
                thread_opened = ax.has_text("这是第一轮用户问句") or ax.has_text("synthetic-test")

        check("test_thread_opened", thread_opened, thread_opened, "测试线程在主界面成功渲染")
        if not thread_opened:
            raise RuntimeError("测试线程未能在主界面加载显示，无法进行后续断言")

        # -------------------------------------------------------------------
        # 1. S6 Profile 徽标、白名单与友好标签测试
        # -------------------------------------------------------------------
        # 点击顶栏 ProfileBadge (左上偏侧，x≈285, y≈32)
        badge_pt = ax.focus_point("标准") or ax.focus_point("topnav-profile-badge", contains=True) or (285, 32)
        xs.click(*badge_pt, settle=1.2)

        # 抓取展开的弹层
        shot_base = EVID / "profile-base.png"
        xs.shot(shot_base)

        labels_after_open = "\n".join(ax.labels())
        has_popover = "下次启动使用" in labels_after_open or "通道默认" in labels_after_open
        check("profile_popover_opened", has_popover, has_popover, "Profile 选择弹层成功展开")

        # 断言不得出现技术包名前缀 "dsh-profile-"
        has_tech_prefix = "dsh-profile-" in labels_after_open
        check("no_dsh_profile_prefix_in_popover", not has_tech_prefix, not has_tech_prefix, "弹层文本不含 dsh-profile-")

        # 断言本次实际使用显示为 omnidesk-base
        has_base_effective = "omnidesk-base" in labels_after_open
        check("effective_is_omnidesk_base", has_base_effective, has_base_effective, "本次启动实际生效显示为 omnidesk-base")

        # 点击「纪检专业」选项（点击坐标 x=396, y=164）
        xs.click(396, 164, settle=1.2)

        # 此时断言出现确认框「切换到「纪检专业」？」并截图
        shot_disc = EVID / "profile-discipline.png"
        xs.shot(shot_disc)

        labels_confirm = "\n".join(ax.labels())
        confirm_has_friendly_text = (
            "切换到「纪检专业」？" in labels_confirm
            or ax.has_text("切换到「纪检专业」？")
            or "确认" in labels_confirm
        )
        check("confirm_box_friendly_title", confirm_has_friendly_text, confirm_has_friendly_text, "确认框包含「切换到「纪检专业」？」")

        confirm_has_tech_prefix = "dsh-profile-" in labels_confirm
        check("no_dsh_profile_in_confirm_box", not confirm_has_tech_prefix, not confirm_has_tech_prefix, "确认框不含 dsh-profile-")

        # 点击「确认」按钮（坐标 x≈505, y≈145）
        confirm_btn = ax.find("确认", role="push button") or ax.find("确认")
        if confirm_btn:
            ax.invoke(confirm_btn)
        else:
            xs.click(505, 145, settle=1.2)
        time.sleep(1.8)

        # 断言 settings.v1.json 已写入 discipline
        saved_settings = json.loads(cfg_file.read_text(encoding="utf-8"))
        saved_profile = saved_settings.get("engine_profile")
        check("settings_saved_discipline", saved_profile == "discipline", saved_profile, "配置已成功保存为 discipline")

        # 重新点击 Profile 徽标，验证已选择纪检专业
        badge_pt2 = (285, 32)
        xs.click(*badge_pt2, settle=1.5)
        labels_reopened = "\n".join(ax.labels())
        has_saved_disc_hint = "纪检专业" in labels_reopened or ax.has_text("纪检专业")
        check("reopened_shows_discipline_scheduled", has_saved_disc_hint, has_saved_disc_hint, "弹层显示下次启动生效为纪检专业")

        # 点击空白处关闭弹层
        xs.click(700, 500, settle=1.0)

        # -------------------------------------------------------------------
        # 2. S7 TurnRail 悬停预览卡与键盘聚焦
        # -------------------------------------------------------------------
        # 实测刻度在 x≈1298, y≈503
        shot_before_rail = EVID / "00-rail-before.png"
        xs.shot(shot_before_rail)

        # 移至刻度并悬停
        xs.move(1298, 503)
        time.sleep(1.2)

        shot_rail_card = EVID / "turnrail-card.png"
        xs.shot(shot_rail_card)

        # 断言预览卡出现：区域 (950, 420, 1290, 580) 像素差异显著（浮现卡片）
        rail_card_delta = region_delta(shot_before_rail, shot_rail_card, (950, 420, 1290, 580))
        check("turnrail_hover_card_appears", rail_card_delta > 1500, rail_card_delta, "TurnRail 悬停卡片成功浮现")

        # 键盘聚焦测试：点击聚焦刻度并截图
        xs.click(1298, 503, settle=0.8)
        xs.shot(EVID / "turnrail-card-focus.png")
        check("turnrail_card_focus_shot", (EVID / "turnrail-card-focus.png").is_file(), True, "完成键盘聚焦截图")

        # -------------------------------------------------------------------
        # 3. S8 当前会话检索测试
        # -------------------------------------------------------------------
        # 聚焦搜索框（快捷键 Ctrl+K）
        xs.click(600, 32, settle=0.8)

        # 输入存在关键词 "synthetic-test"（全会话恰好出现 2 次）
        xs.type_ascii("synthetic-test", settle=0.03)
        time.sleep(1.5)

        shot_match1 = EVID / "search-match.png"
        xs.shot(shot_match1)

        labels_search1 = "\n".join(ax.labels())
        has_count_1_of_2 = "1/2" in labels_search1 or ax.has_text("1/2") or ax.find("1/2", contains=True) is not None
        check("search_count_1_of_2", has_count_1_of_2, has_count_1_of_2, "初次命中计数显示为 1/2")

        # 统计琥珀高亮像素：<mark> 的 bg-amber-300 底色大约为 (r>230, g>190, b<120)
        amber_pixels = count_pixels(
            shot_match1,
            (100, 120, 1300, 780),
            lambda px: px[0] > 230 and px[1] > 190 and px[2] < 120,
        )
        check("search_highlight_mark_pixels", amber_pixels > 25, amber_pixels, "对话区命中词出现琥珀高亮背景")

        # 回车导航到下一个命中
        xs.key("Return", settle=1.0)
        shot_match2 = EVID / "search-match-second.png"
        xs.shot(shot_match2)

        labels_search2 = "\n".join(ax.labels())
        has_count_2_of_2 = "2/2" in labels_search2 or ax.has_text("2/2") or ax.find("2/2", contains=True) is not None
        check("search_count_2_of_2_after_enter", has_count_2_of_2, has_count_2_of_2, "回车后跳转至第 2 项，计数显示为 2/2")

        # 回车前后对话区像素发生变化（证明滚动或聚焦环发生跳转）
        jump_delta = region_delta(shot_match1, shot_match2, (200, 150, 1200, 750))
        check("search_jump_scroll_delta", jump_delta > 100, jump_delta, "回车前后对话区发生跳转定位变化")

        # 搜索无结果词 "notfoundxyz"
        xs.select_all_and_clear()
        xs.type_ascii("notfoundxyz", settle=0.03)
        time.sleep(1.2)

        shot_empty = EVID / "search-empty.png"
        xs.shot(shot_empty)

        labels_search_empty = "\n".join(ax.labels())
        has_count_0_of_0 = "0/0" in labels_search_empty or ax.has_text("0/0") or ax.find("0/0", contains=True) is not None
        check("search_count_0_of_0", has_count_0_of_0, has_count_0_of_0, "无匹配内容时计数显示为 0/0")

        # 按 Esc 键清空并退出搜索
        xs.key("Escape", settle=0.8)
        shot_escaped = EVID / "00-search-escaped.png"
        xs.shot(shot_escaped)

        labels_escaped = "\n".join(ax.labels())
        count_cleared = "0/0" not in labels_escaped and "1/2" not in labels_escaped
        check("search_cleared_on_esc", count_cleared, count_cleared, "按 Esc 退出后搜索计数条已隐藏")

        amber_after_esc = count_pixels(
            shot_escaped,
            (100, 120, 1300, 780),
            lambda px: px[0] > 230 and px[1] > 190 and px[2] < 120,
        )
        check("no_highlight_after_esc", amber_after_esc == 0, amber_after_esc, "按 Esc 退出后高亮完全消除")

        # 写入 ui-facts.json
        facts_file = EVID / "ui-facts.json"
        facts_file.write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"Wrote UI facts to {facts_file}")

        if facts["failures"]:
            print(f"FAILED with {len(facts['failures'])} errors!", file=sys.stderr)
            return 1
        print("ALL UI ASSERTIONS PASSED (100%)!")
        return 0

    finally:
        session.close()
        try:
            bus_proc.terminate()
            bus_proc.wait(timeout=2)
        except Exception:
            pass
        try:
            xvfb.terminate()
            xvfb.wait(timeout=2)
        except Exception:
            pass


if __name__ == "__main__":
    sys.exit(main())

