#!/usr/bin/env python3
"""OD-ARCH-055 S3 · 场景声明联动核对（真实窗口，一次性数据根）。

只做三件事，全部在 /tmp 下的一次性根里：

  1. 默认声明启动，截图（四个业务入口 + 基础设施入口）；
  2. 在副本声明里隐藏「文案写作」→ 重启 → 截图，并点原「文案写作」位置的 Dock 图标，
     确认落到的是剩余入口（无 router / 无深链，界面没有可直达已隐藏场景的路径）；
  3. 把声明恢复为初始字节 → 重启 → 截图，并比对前后 sha256 完全一致。

证据等级：真实桌面窗口运行（Xvfb + 合成端点）+ 文件事实（sha256）。
不触碰 runtime/、~/.dsh、~/.agents 或用户日常数据；不读凭证。

用法：python3 verify_scene_decl_linkage_055.py [--display :96] [--root /tmp/od055-s3]
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

EVIDENCE = HERE / "evidence" / "od-arch-055"
SHOTS = EVIDENCE / "shots"

# Dock 图标中心相对窗口左上角的偏移（实测，8 个入口时）。第 3 项在默认声明里是「文案写作」。
# Dock 是垂直居中的固定列表：少一个入口，整体高度少 50px，所有图标因此各下移 25px。
# 这是 A/B 对照的坐标依据——同一个「序号」在两个声明下指向不同场景。
DOCK_DX = 47
DOCK_SLOT3_DY_8 = 333
DOCK_ROW_STEP = 50
DOCK_KEEP_SHIFT = 25  # 入口从 8 个变成 7 个时的下移量

DEFAULT_DECL = (REPO / "samples" / "profiles" / "discipline.yml").read_text(encoding="utf-8")

# 隐藏「文案写作」后的声明：保持其余入口与顺序不变，便于对照截图。
HIDDEN_DECL = """schema: omnidesk.profile-decl.v1
display_name: OmniDesk Discipline

scenes:
  - id: home
    label: 工作台首页
    icon: brand
  - id: report
    label: 工作记录
    icon: note
    note: 模型起草
  - id: project
    label: 项目跟踪
    icon: checklist
    note: 模型起草
  - id: sessions
    label: 项目与会话
    icon: history
    note: 引擎项目 · 对话
"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def round_shot(session: AppSession, xs: XSession, shot: Path, settle: float = 7.0) -> dict:
    win = mount_window(session, xs)
    time.sleep(settle)
    geo = xs.shot(shot)
    return {
        "shot": str(shot),
        "frame": list(geo.as_tuple()),
        "title": xs.title(win),
        "app_alive": session.alive(),
        "window": list(xs.geometry(win).as_tuple()),
    }


def mount_window(session: AppSession, xs: XSession):
    """等窗口出现，返回其句柄（不搬动窗口，避免影响用户可见位置）。"""
    win = xs.find_window("OmniDesk", timeout=90)
    if win is None:
        raise RuntimeError("未找到 OmniDesk 主窗口")
    time.sleep(2.0)
    return win


def click_slot3(xs: XSession, win, dy: int) -> tuple[int, int]:
    geo = xs.geometry(win)
    point = (geo.x + DOCK_DX, geo.y + dy)
    xs.focus(win)
    # 无边框窗口：第一次点击常被吃掉（只用来把焦点交给窗口），所以先移指针、再点两次。
    time.sleep(0.8)
    xs.move(*point)
    time.sleep(0.4)
    xs.click(*point, settle=0.5)
    xs.click(*point)
    return point


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--display", default=":96")
    parser.add_argument("--root", default="/tmp/od055-s3")
    args = parser.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    root = Path(args.root)
    resolved = root.resolve()
    if Path("/tmp") not in resolved.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{resolved}（本脚本只用一次性数据根）")
    if root.exists():
        shutil.rmtree(root)

    xvfb = start_xvfb(args.display)
    report: dict = {
        "plan": "OD-ARCH-055 S3",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实（sha256）",
        "display": args.display,
        "root": str(root),
        "rounds": {},
        "notes": [
            "Dock 入口可见性由截图人工比对判定，不是 DOM 断言。",
            "无 router / hash / 深链：隐藏场景后界面没有 URL 或命令可直达该页面。",
            "演练结束把声明恢复为初始字节，sha256 前后比对见 decl_sha256_*。",
        ],
    }
    session = AppSession(root=root, display=args.display)
    try:
        session.prepare()
        xs = XSession(args.display)
        decl_path = session.app_data / "profiles" / "discipline.yml"

        # --- 1. 默认声明 ---------------------------------------------------------
        session.start()
        report["rounds"]["default"] = round_shot(session, xs, SHOTS / "01-default.png")
        report["rounds"]["default"]["decl_path"] = str(decl_path)
        report["rounds"]["default"]["decl_matches_sample"] = (
            decl_path.exists() and decl_path.read_text(encoding="utf-8") == DEFAULT_DECL
        )
        initial_sha = sha256(decl_path)
        report["decl_sha256_before"] = initial_sha
        # A 侧：同一像素位置点第 3 个入口，默认声明下应打开「文案写作」。
        win = mount_window(session, xs)
        report["rounds"]["default"]["click_point"] = list(
            click_slot3(xs, win, DOCK_SLOT3_DY_8)
        )
        time.sleep(4.0)
        report["rounds"]["default"]["click_slot3"] = round_shot(
            session, xs, SHOTS / "02-default-slot3-writing.png", settle=2.0
        )
        session.stop_app()
        time.sleep(2.0)

        # --- 2. 隐藏「文案写作」 -------------------------------------------------
        decl_path.write_text(HIDDEN_DECL, encoding="utf-8")
        hidden_sha = sha256(decl_path)
        session.start()
        report["rounds"]["hidden"] = round_shot(session, xs, SHOTS / "03-writing-hidden.png")
        report["rounds"]["hidden"]["decl_sha256"] = hidden_sha
        report["rounds"]["hidden"]["decl_byte_identical_to_edit"] = (
            decl_path.read_text(encoding="utf-8") == HIDDEN_DECL
        )
        # B 侧：同一个像素位置，隐藏后应落到「项目跟踪」而不是被隐藏的场景。
        # 界面无 router / hash / 深链，因此不存在用 URL 或命令直达被隐藏页面的路径。
        win = mount_window(session, xs)
        report["rounds"]["hidden"]["click_point"] = list(
            click_slot3(xs, win, DOCK_SLOT3_DY_8 + DOCK_KEEP_SHIFT)
        )
        time.sleep(4.0)
        report["rounds"]["hidden"]["click_old_writing_slot"] = round_shot(
            session, xs, SHOTS / "04-hidden-slot3-project.png", settle=2.0
        )
        session.stop_app()
        time.sleep(2.0)

        # --- 3. 恢复声明 ---------------------------------------------------------
        decl_path.write_text(DEFAULT_DECL, encoding="utf-8")
        session.start()
        report["rounds"]["restored"] = round_shot(session, xs, SHOTS / "05-restored.png")
        restored_sha = sha256(decl_path)
        report["decl_sha256_after"] = restored_sha
        report["decl_restored_identical"] = (
            restored_sha == initial_sha and decl_path.read_text(encoding="utf-8") == DEFAULT_DECL
        )
        session.stop_app()
    finally:
        session.stop_all()
        xvfb.terminate()

    (EVIDENCE / "scene-decl-linkage-run.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps({k: v for k, v in report.items() if k != "rounds"}, ensure_ascii=False, indent=2))
    return 0 if report.get("decl_restored_identical") else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
