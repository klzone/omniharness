#!/usr/bin/env python3
"""OD-ARCH-051 S3/S4 · 纪检 Profile 声明的真实窗口验收。

只使用一次性数据根 + 回环合成模型端点 + 一次性 Xvfb 显示：

  1. 首次启动：宿主在 `<数据根>/profiles/discipline.yml` 创建默认声明，Dock 显示 5 个业务入口；
  2. 改声明（去掉「文案写作」）后重启：Dock 上该入口消失，名称随之变化；
  3. 声明写坏后重启：界面显示错误提示并回退内置入口（不静默）。

证据等级：真实桌面窗口运行（XTEST 注入 + GDK 抓图）+ 文件事实。
不触碰 runtime/、~/.dsh、~/.agents 或用户日常数据；不读凭证。

用法：python3 verify_profile_decl.py [--display :97] [--root /tmp/od051-s3]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-051"
SHOTS = EVIDENCE / "shots"

DEFAULT_DECL = (REPO / "samples" / "profiles" / "discipline.yml").read_text(encoding="utf-8")


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def run_round(session: AppSession, xs: XSession, shot: Path, settle: float = 6.0) -> dict:
    win = xs.find_window("OmniDesk", timeout=90)
    if win is None:
        raise RuntimeError("未找到 OmniDesk 主窗口")
    time.sleep(settle)
    geo = xs.shot(shot)
    return {
        "title": xs.title(win),
        "shot": str(shot),
        "frame": list(geo.as_tuple()),
        "app_alive": session.alive(),
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--display", default=":97")
    parser.add_argument("--root", default="/tmp/od051-s3")
    args = parser.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    xvfb = start_xvfb(args.display)
    report: dict = {
        "plan": "OD-ARCH-051 S3",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）",
        "display": args.display,
        "rounds": {},
    }
    session = AppSession(root=Path(args.root), display=args.display)
    try:
        session.prepare()
        xs = XSession(args.display)

        # --- 1. 首次启动：默认声明 -------------------------------------------------
        session.start()
        report["rounds"]["default"] = run_round(session, xs, SHOTS / "01-default.png")
        decl_path = session.app_data / "profiles" / "discipline.yml"
        report["rounds"]["default"]["decl_path"] = str(decl_path)
        report["rounds"]["default"]["decl_created"] = decl_path.exists()
        if decl_path.exists():
            report["rounds"]["default"]["decl_matches_sample"] = (
                decl_path.read_text(encoding="utf-8") == DEFAULT_DECL
            )
        session.stop_app()
        time.sleep(2.0)

        # --- 2. 用户改声明：去掉「文案写作」，并改名 -------------------------------
        edited = """schema: omnidesk.profile-decl.v1
display_name: 我的纪检工作台

scenes:
  - id: home
    label: 工作台首页
    icon: brand
  - id: report
    label: 周报与随记
    icon: note
    note: 模型起草
  - id: project
    label: 项目跟踪
    icon: checklist
    note: 模型起草
"""
        decl_path.write_text(edited, encoding="utf-8")
        session.start()
        report["rounds"]["edited"] = run_round(session, xs, SHOTS / "02-edited.png")
        session.stop_app()
        time.sleep(2.0)

        # --- 3. 声明写坏：必须提示并回退内置入口 -----------------------------------
        decl_path.write_text("schema: something-else\n", encoding="utf-8")
        session.start()
        report["rounds"]["broken"] = run_round(session, xs, SHOTS / "03-broken.png")
        report["rounds"]["broken"]["decl_untouched"] = (
            decl_path.read_text(encoding="utf-8") == "schema: something-else\n"
        )
        session.stop_app()
    finally:
        session.stop_all()
        xvfb.terminate()

    (EVIDENCE / "profile-decl-run.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(report["rounds"], ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
