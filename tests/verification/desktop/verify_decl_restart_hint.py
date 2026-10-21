#!/usr/bin/env python3
"""OD-ARCH-052 第 2 步 · 真实窗口验收：声明改动后的"重启生效"提示。

只使用一次性数据根 + 回环合成模型端点 + 一次性 Xvfb 显示：

  1. 冷启动：无提示条；
  2. 应用运行中改写 `profiles/discipline.yml`（只加一行注释，界面渲染不变）：
     5 秒轮询后出现提示条，且用户改动不被覆盖、界面不热重载；
  3. 运行中删除文件：提示条改为"已被删除，重启后重建"；
  4. 重启：重建默认声明、提示条消失（第 2 轮的新内容此时才生效）。

证据等级：真实桌面窗口运行（XTEST 注入 + GDK 抓图）+ 文件事实。
不触碰 runtime/、~/.dsh、~/.agents 或用户日常数据；不读凭证。

用法：python3 verify_decl_restart_hint.py [--display :96] [--root /tmp/od052-hint]
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

from window_harness import AppSession, XSession  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-arch-052"
SHOTS = EVIDENCE / "shots"

# 提示条位于悬浮顶栏下方、内容区之上（实测在窗口内 y≈80..120 处）。
# 按窗口几何取这条横带做像素比对；首次实跑把区域取到了标题栏，四个"指纹"全相同，已修正。
HINT_OFFSET_Y = 78
HINT_HEIGHT = 56


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def band_signature(png: Path, geo) -> str:
    """顶部横带的像素指纹：用来判断提示条是否出现，避免依赖 OCR。"""
    import hashlib

    from PIL import Image

    with Image.open(png) as img:
        crop = img.convert("RGB").crop(
            (geo.x, geo.y + HINT_OFFSET_Y, geo.x + geo.w, geo.y + HINT_OFFSET_Y + HINT_HEIGHT)
        )
        return hashlib.sha256(crop.tobytes()).hexdigest()[:16]


def snap(xs: XSession, session: AppSession, name: str, settle: float = 6.0) -> dict:
    win = xs.find_window("OmniDesk", timeout=90)
    if win is None:
        raise RuntimeError("未找到 OmniDesk 主窗口")
    time.sleep(settle)
    path = SHOTS / name
    geo = xs.shot(path)
    return {
        "shot": str(path),
        "band": band_signature(path, geo),
        "frame": list(geo.as_tuple()),
        "app_alive": session.alive(),
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--display", default=":96")
    parser.add_argument("--root", default="/tmp/od052-hint")
    args = parser.parse_args(argv[1:])

    SHOTS.mkdir(parents=True, exist_ok=True)
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}（本脚本只用一次性数据根）")
    if root.exists():
        shutil.rmtree(root)

    xvfb = start_xvfb(args.display)
    report: dict = {
        "plan": "OD-ARCH-052 第 2 步",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）",
        "display": args.display,
        "rounds": {},
        "checks": {},
        "notes": [
            "提示条判定 = 顶部横带像素指纹变化 + 人工看截图确认文案；脚本不做 OCR。",
            "Dock 入口是否变化由截图人工比对（第 2 轮应保持启动时入口）。",
        ],
    }
    session = AppSession(root=root, display=args.display)
    try:
        session.prepare()
        xs = XSession(args.display)
        decl_path = session.app_data / "profiles" / "discipline.yml"

        # --- 1. 冷启动：不应有提示条 -------------------------------------------
        session.start()
        report["rounds"]["cold"] = snap(xs, session, "01-cold.png")
        report["rounds"]["cold"]["decl_path"] = str(decl_path)
        warm = decl_path.read_text(encoding="utf-8")

        # --- 2. 运行中改文件：出现"重启生效"提示 --------------------------------
        # 只追加注释：文件指纹变化，但界面渲染应与冷启动完全一致，
        # 这样"提示条消失"就能用像素相等来判断，不会被品牌名变化干扰。
        edited = warm + "# 本地调整（验收用）\n"
        decl_path.write_text(edited, encoding="utf-8")
        time.sleep(1.0)
        report["rounds"]["modified_live"] = snap(xs, session, "02-modified-live.png", settle=8.0)
        report["checks"]["hint_appears_after_edit"] = (
            report["rounds"]["modified_live"]["band"] != report["rounds"]["cold"]["band"]
        )
        # 不改热重载：文件在磁盘上的内容保持用户写入的样子。
        report["checks"]["user_edit_not_overwritten"] = (
            decl_path.read_text(encoding="utf-8") == edited
        )
        # --- 3. 运行中删除文件：提示改为"已被删除" ------------------------------
        decl_path.unlink()
        time.sleep(1.0)
        report["rounds"]["missing_live"] = snap(xs, session, "03-deleted-live.png", settle=8.0)
        report["checks"]["hint_changes_when_file_deleted"] = (
            report["rounds"]["missing_live"]["band"]
            != report["rounds"]["modified_live"]["band"]
        )
        report["checks"]["delete_is_not_hot_reloaded"] = not decl_path.exists()
        session.stop_app()
        time.sleep(2.0)

        # --- 4. 重启：重建默认声明、提示消失 ------------------------------------
        session.start()
        report["rounds"]["after_restart"] = snap(xs, session, "04-after-restart.png")
        report["checks"]["hint_cleared_after_restart"] = (
            report["rounds"]["after_restart"]["band"] == report["rounds"]["cold"]["band"]
        )
        report["checks"]["missing_recreated_on_launch"] = decl_path.exists()
        session.stop_app()
    finally:
        session.stop_all()
        xvfb.terminate()

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / "decl-restart-hint.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps({"checks": report["checks"], "rounds": report["rounds"]}, ensure_ascii=False, indent=2))
    failed = [k for k, v in report["checks"].items() if v is False]
    if failed:
        print("FAILED:", ", ".join(failed))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
