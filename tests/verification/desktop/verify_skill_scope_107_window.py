#!/usr/bin/env python3
"""OD-ARCH-107 真实窗口 smoke：能力管理页要说清「预注入」与「按需加载」。

两轮对照，各自一次性数据根 + 合成端点 + 一次性 Xvfb：

  1. 基础标准（OMNIDESK_DSH_PROFILE=omnidesk-base，无 rules.json / 无 decl.yml）：
     能力管理页显示「基础标准」口径，Skill 全部落在「可被模型按需加载」组；
  2. discipline（预置样例 rules.json 与 decl.yml）：出现「本 Profile 预注入」组，
     被绑定的 Skill 带场景标签。

判定方式：截图 + 「预注入」标签的强调色像素统计（标签底色 rgba(79,70,229,.1)
≈ rgb(235,235,252)、描边 ≈ rgb(165,161,241)，与设置页其余色块可区分），
外加文件事实（标准模式不自动创建规则/声明文件）。

证据等级：真实桌面窗口运行（Xvfb + 合成端点）+ 文件事实。

用法：python3 verify_skill_scope_107_window.py [--display :94] [--root /tmp/od107-scope]
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import DEV_PORT, AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-107"
SHOTS = EVIDENCE / "shots"

# 顶栏齿轮与「能力管理」分页（1440x900 固定版式，由 072 与 107 校准）。
SETTINGS_GEAR = (1164, 44)
CAP_TAB = (556, 201)
# Skill 左栏在窗口内的横向范围（用于统计标签强调色，避开右侧插件区）。
SKILL_COL_X = (240, 470)


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def port_open() -> bool:
    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", DEV_PORT)) == 0


def accent_pixels(shot: Path, y0: int = 430, y1: int = 880) -> int:
    """统计「预注入」标签的强调色像素。

    只认标签自己的两种颜色：底色 rgba(79,70,229,.1) 合成后 ≈ (235,235,252)，
    描边 rgba(79,70,229,.45) 合成后 ≈ (165,161,241)。左栏面板本身是 (226,225,235)，
    「偏冷色」这种宽松判据会把它算进来（实测标准轮误报 884 像素）。
    """
    from PIL import Image

    im = Image.open(shot).convert("RGB")
    px = im.load()
    count = 0
    for y in range(y0, min(y1, im.size[1])):
        for x in range(SKILL_COL_X[0], SKILL_COL_X[1]):
            r, g, b = px[x, y]
            if (
                abs(r - 235) <= 2 and abs(g - 235) <= 2 and abs(b - 252) <= 2
            ) or (
                abs(r - 165) <= 6 and abs(g - 161) <= 6 and abs(b - 241) <= 6
            ):
                count += 1
    return count


def seed_dsh_home(root: Path, profile: str) -> None:
    """隔离 DSH home：放一份最小 package.json 与仓库 skills（不复制用户实例）。"""
    home = root / "dsh-home"
    target = home / "profiles" / profile
    target.mkdir(parents=True, exist_ok=True)
    shutil.copy(REPO / "samples" / "profiles" / f"{profile}.profile.json", target / "package.json")
    skills_dir = home / "skills"
    if not skills_dir.exists():
        shutil.copytree(REPO / "skills", skills_dir)


def open_capabilities(session: AppSession, display: str, shot: Path) -> dict:
    xs = XSession(display)
    win = xs.find_window("OmniDesk", timeout=120)
    if win is None:
        return {"window_found": False}
    xs.focus(win)
    xs.place(win, 0, 0, 1440, 900, settle=3.0)
    time.sleep(8.0)
    xs.click(*SETTINGS_GEAR, settle=3.0)
    time.sleep(1.5)
    xs.click(*CAP_TAB, settle=3.0)
    time.sleep(3.0)
    xs.shot(shot)
    return {"window_found": True, "shot": str(shot), "accent_pixels": accent_pixels(shot)}


def run_round(root: Path, display: str, profile: str, shot: Path, seed_rules: bool) -> dict:
    session = AppSession(
        root=root,
        display=display,
        env_extra={"OMNIDESK_DSH_PROFILE": profile},
        reuse_running_dev_server=port_open(),
    )
    try:
        session.prepare()
        seed_dsh_home(root, profile)
        if seed_rules:
            prof = session.app_data / "profiles" / "discipline"
            prof.mkdir(parents=True, exist_ok=True)
            shutil.copy(REPO / "samples" / "profiles" / "discipline.yml", prof / "decl.yml")
            shutil.copy(REPO / "samples" / "profiles" / "discipline.rules.json", prof / "rules.json")
        session.start()
        facts = open_capabilities(session, display, shot)
        facts["alive"] = session.alive()
        facts["profile"] = profile
        facts["root"] = str(root)
        return facts
    finally:
        session.stop_all()


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":94")
    ap.add_argument("--root", default="/tmp/od107-scope")
    args = ap.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)

    xvfb = start_xvfb(args.display)
    report: dict = {
        "plan": "OD-ARCH-107",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实",
        "rounds": {},
        "checks": {},
    }
    try:
        std_root = root / "standard"
        std = run_round(std_root, args.display, "omnidesk-base", SHOTS / "01-standard-capabilities.png", False)
        report["rounds"]["standard"] = std
        std_rules = std_root / "app-data" / "profiles" / "omnidesk-base" / "rules.json"
        std_decl = std_root / "app-data" / "profiles" / "omnidesk-base" / "decl.yml"
        report["checks"]["standard_window_alive"] = std.get("alive") is True
        # 标准轮实测 0；discipline 轮实测 695（底色 553 + 描边 142）。
        report["checks"]["standard_no_preinjected_tag"] = std.get("accent_pixels", 999) == 0
        report["checks"]["standard_no_rules_created"] = not std_rules.exists()
        report["checks"]["standard_no_decl_created"] = not std_decl.exists()
        time.sleep(2.0)

        disc = run_round(root / "discipline", args.display, "discipline", SHOTS / "02-discipline-capabilities.png", True)
        report["rounds"]["discipline"] = disc
        report["checks"]["discipline_window_alive"] = disc.get("alive") is True
        report["checks"]["discipline_has_preinjected_tag"] = disc.get("accent_pixels", 0) >= 200
    finally:
        xvfb.terminate()

    failed = [k for k, v in report["checks"].items() if v is False]
    report["result"] = "pass" if not failed else "fail"
    (EVIDENCE / "skill-scope-window.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({"rounds": report["rounds"], "checks": report["checks"], "result": report["result"]}, ensure_ascii=False, indent=2))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
