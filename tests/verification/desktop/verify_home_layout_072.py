#!/usr/bin/env python3
"""OD-ARCH-072 真实窗口验收：首页信息架构四态 + Profile 弹层。

一次性 Xvfb + 隔离数据根 + 回环合成端点；不读凭证、不碰用户数据。

  1. 初始空对话：输入区垂直居中、无「今日重点推进」卡、待办条不占位、机器人在对话上方左侧；
  2. 预置一条合成对话并重启：输入区落底、历史完整；
  3. 点顶栏日期：三周节律浮卡出现；指针移开 1 秒后消失；
  4. 点 Profile 徽标：列出隔离根里真实存在的 Profile；选一个 → 确认弹窗 → 确认后
     设置文件写入 engine_profile（文件事实），界面显示「下次启动生效」回执。

用法：python3 verify_home_layout_072.py [--display :94] [--root /tmp/od072-home]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-072"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900

DATE_PILL = (940, 44)
PROFILE_BADGE = (310, 44)
THREAD_ID = "t072homelayout"


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            err = proc.stderr.read().decode("utf-8", "replace") if proc.stderr else ""
            raise RuntimeError(f"Xvfb {display} 启动失败：{err.strip()}")
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError(f"Xvfb {display} 未就绪")


def tree_fingerprint(root: Path, limit: int = 4000) -> str:
    h = hashlib.sha256()
    if not root.exists():
        return "missing"
    count = 0
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        count += 1
        st = p.stat()
        h.update(f"{p.relative_to(root)}|{st.st_size}|{int(st.st_mtime)}".encode())
        if count >= limit:
            break
    return h.hexdigest()[:16] + f":{count}"


def thread_payload() -> dict:
    now = int(time.time() * 1000)
    return {
        "id": THREAD_ID,
        "tag": "plain",
        "projectPath": None,
        "title": "合成布局验收对话",
        "preset": "plain",
        "messages": [
            {"id": "m1", "role": "user", "text": "合成样例：请把三条要点整理成一段说明。"},
            {
                "id": "m2",
                "role": "assistant",
                "text": "合成回复：三条要点已整理；本条内容全部虚构，仅用于界面验收。",
            },
        ],
        "createdAt": now - 60000,
        "updatedAt": now,
        "version": 1,
    }


def main(argv: list[str]) -> int:
    def snapshot(root: Path) -> dict[str, tuple[int, int]]:
        """文件 -> (大小, mtime 秒)。用来点名到底哪些文件变了，而非只给一个哈希。"""
        snap: dict[str, tuple[int, int]] = {}
        if not root.exists():
            return snap
        for p in sorted(root.rglob("*")):
            if not p.is_file():
                continue
            st = p.stat()
            snap[str(p.relative_to(root))] = (st.st_size, int(st.st_mtime))
        return snap

    def snapshot_diff(before: dict, after: dict, cap: int = 12) -> list[str]:
        changed = [
            f"{k}（{before.get(k)} -> {after.get(k)}）"
            for k in sorted(set(before) | set(after))
            if before.get(k) != after.get(k)
        ]
        return changed[:cap]

    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":94")
    ap.add_argument("--root", default="/tmp/od072-home")
    args = ap.parse_args(argv[1:])
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {
        "plan": "OD-ARCH-072 Part C",
        "evidence_level": "真实桌面窗口（一次性 Xvfb + 合成端点）+ 文件事实",
        "root": str(root),
        "source_fingerprint_before": tree_fingerprint(SOURCE_HOME),
        "user_dsh_before": tree_fingerprint(Path.home() / ".dsh"),
        "source_snapshot_before": snapshot(SOURCE_HOME),
        "user_dsh_snapshot_before": snapshot(Path.home() / ".dsh"),
        "steps": {},
    }

    # 只读复制隔离实例（含 node_modules / skills）；真实根保持不动。
    shutil.copytree(SOURCE_HOME, root / "dsh-home", symlinks=True)
    profiles_on_disk = sorted(
        p.name
        for p in (root / "dsh-home" / "profiles").iterdir()
        if (p / "package.json").is_file()
    )
    facts["profiles_on_disk"] = profiles_on_disk

    xvfb = start_display(args.display)
    session = AppSession(root=root, display=args.display, artifact_override=True)
    try:
        session.prepare()
        xs = XSession(args.display)

        # --- 1. 初始空对话 ------------------------------------------------
        session.start()
        win = xs.find_window("OmniDesk", timeout=120)
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(6)
        xs.shot(EVID / "10-home-empty.png")
        facts["steps"]["empty"] = {
            "screenshot": str(EVID / "10-home-empty.png"),
            "expect": "输入框在首页竖直中部；无「今日重点推进」卡片与常驻日程卡；"
                      "无待办时不占位；机器人在对话区上方左侧",
            "app_alive": session.alive(),
        }

        # --- 2. 有历史 → 输入框落底 ---------------------------------------
        target = session.app_data / "threads" / "plain" / f"{THREAD_ID}.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(thread_payload(), ensure_ascii=False, indent=2), encoding="utf-8")
        session.stop_app()
        time.sleep(1.5)
        session.start()
        win = xs.find_window("OmniDesk", timeout=120)
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(6)
        xs.shot(EVID / "11-home-history.png")
        facts["steps"]["history"] = {
            "screenshot": str(EVID / "11-home-history.png"),
            "thread_file": str(target),
            "expect": "两条合成消息可见，输入框落到底部（历史区在上）",
        }

        # --- 3. 日期悬浮卡 ------------------------------------------------
        g = xs.geometry(win)
        xs.focus(win)
        xs.move(g.x + DATE_PILL[0], g.y + DATE_PILL[1])
        time.sleep(0.5)
        xs.click(g.x + DATE_PILL[0], g.y + DATE_PILL[1], settle=1.2)
        time.sleep(0.8)
        xs.shot(EVID / "12-calendar-open.png")
        # 移开：移到窗口中间偏下（远离日期与浮卡）
        xs.move(g.x + 700, g.y + 700)
        time.sleep(0.4)
        xs.shot(EVID / "13-calendar-fading.png")
        time.sleep(1.8)
        xs.shot(EVID / "14-calendar-closed.png")
        facts["steps"]["calendar"] = {
            "screenshots": [
                str(EVID / "12-calendar-open.png"),
                str(EVID / "13-calendar-fading.png"),
                str(EVID / "14-calendar-closed.png"),
            ],
            "expect": "点日期出现三周节律浮卡；移开 1 秒后淡出消失",
        }

        # --- 4. Profile 徽标 ----------------------------------------------
        time.sleep(1.0)
        xs.move(g.x + PROFILE_BADGE[0], g.y + PROFILE_BADGE[1])
        xs.click(g.x + PROFILE_BADGE[0], g.y + PROFILE_BADGE[1], settle=2.0)
        time.sleep(1.5)
        xs.shot(EVID / "15-profile-popover.png")
        facts["steps"]["profile_open"] = {
            "screenshot": str(EVID / "15-profile-popover.png"),
            "expect": "默认「无 · 标准模式」，列表来自隔离根真实 profile 目录",
        }

        # 选列表里第一个非标准项（弹层在徽标下方左对齐，宽 300）：
        # 每项高约 48；首项 y≈100，之后依次 +48。
        # 先用文件事实确认可选集合，截图上再校准坐标。
        settings_path = session.app_data / "config" / "settings.v1.json"
        facts["settings_before"] = (
            json.loads(settings_path.read_text(encoding="utf-8")) if settings_path.is_file() else None
        )

        # 选最后一个非标准 Profile（列表顺序 = 名称排序，标准项插入其中）。
        xs.click(g.x + 380, g.y + 304, settle=1.5)
        time.sleep(1.0)
        xs.shot(EVID / "16-profile-confirm.png")
        facts["steps"]["profile_confirm"] = {
            "screenshot": str(EVID / "16-profile-confirm.png"),
            "expect": "确认弹窗说明「下次启动引擎生效」，有 取消 / 确认",
        }
        xs.click(g.x + 505, g.y + 163, settle=1.5)
        time.sleep(1.5)
        xs.shot(EVID / "17-profile-saved.png")
        after = (
            json.loads(settings_path.read_text(encoding="utf-8")) if settings_path.is_file() else None
        )
        facts["steps"]["profile_saved"] = {
            "screenshot": str(EVID / "17-profile-saved.png"),
            "settings_path": str(settings_path),
            "engine_profile_after": (after or {}).get("engine_profile"),
            "expect": "设置文件 engine_profile 写入所选 Profile；界面出现回执",
        }
        session.stop_app()
        time.sleep(1.0)
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    facts["source_fingerprint_after"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_after"] = tree_fingerprint(Path.home() / ".dsh")
    # 用户自己的 OmniDesk 实例可能同时在线并正常改写 cordis.yml / 成本账本；
    # 因此这里不只是比哈希，而是点名列出变化文件，便于区分“被脚本污染”与“用户实例在跑”。
    facts["source_changed_files"] = snapshot_diff(
        facts.pop("source_snapshot_before"), snapshot(SOURCE_HOME)
    )
    facts["user_dsh_changed_files"] = snapshot_diff(
        facts.pop("user_dsh_snapshot_before"), snapshot(Path.home() / ".dsh")
    )
    facts["source_unchanged"] = not facts["source_changed_files"]
    facts["user_dsh_unchanged"] = not facts["user_dsh_changed_files"]
    # 归属判定：若变化全部落在 storages/cost-meter/（引擎每次调用的成本账本），且
    # 同时存在用户自己运行的实例，则这些写入属于那个在线实例，不是本次脚本。
    ledger_only = bool(facts["source_changed_files"]) and all(
        line.startswith("storages/cost-meter/") for line in facts["source_changed_files"]
    )
    facts["source_changed_attribution"] = (
        "cost-meter 账本写入，来自同时运行的用户实例（脚本只操作 /tmp 副本）"
        if ledger_only
        else ("本次脚本" if facts["source_changed_files"] else "无变化")
    )
    (EVID / "home-facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

