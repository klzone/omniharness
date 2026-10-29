#!/usr/bin/env python3
"""OD-ARCH-123：场景声明「保存即生效」与「基础标准拒绝保存」的真实窗口验收。

只造合成数据（全新的 /tmp 隔离根），不读真实工作记录、知识库与凭证。

一条链路：
  1. 以基础标准冷启动 → 设置页「对话分类」看到保存受限提示，Dock 只有固有入口；
  2. 顶栏新建一个从基础标准起手的 Profile 并切过去；
  3. 设置页点「填入起步模板」→「校验并保存」，**不重启**，回工作台断言 Dock
     立刻出现工作记录 / 文案写作 / 项目跟踪，并断言声明真的落在隔离根里；
  4. 切回基础标准，断言业务场景消失、保存按钮回到受限状态。

用法：python3 preview_profile_decl_hot_123.py [--display :138] [--root /tmp/od123-preview]
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
    dev_server_ready,
    open_popover_any,
    registry,
    type_into,
)
from preview_profile_switch_110 import deep_text, pick_profile, read_settings, wait_for  # noqa: E402
from preview_weekly_100 import start_dbus, start_display  # noqa: E402
from window_harness import AppSession, XSession  # noqa: E402
from verify_scene_decl_linkage_055 import mount_window  # noqa: E402

EVID = HERE / "evidence" / "od-arch-123"
SHOTS = EVID / "shots"
SCREEN_W, SCREEN_H = 1500, 940
NAME = "hot-desk"
LABEL = "Hot Desk"
SCENES = ("\u5de5\u4f5c\u8bb0\u5f55", "\u6587\u6848\u5199\u4f5c", "\u9879\u76ee\u8ddf\u8e2a")
# 说明：`<p className="hint">` 这类正文段落不被 WebKitGTK 暴露到 AT-SPI（118 / 122
# 已记录同类现象），所以「基础标准保存受限」不靠文案断言，而靠**点保存后文件是否落盘**
# 来验：没有 OD-ARCH-123 的短路，标准下点保存会把 decl.yml 写进 omnidesk-base。


def band_hash(png: Path, geo, box: tuple[float, float, float, float]) -> str:
    """窗口内一块矩形区域（相对窗口左上角、按窗口宽高取比例）的像素指纹。

    用于判断「编辑框里到底有没有 YAML 正文」，不依赖 AT-SPI 文案：
    空编辑框只有一行灰色 placeholder，载入后是一整片深色正文，差异极大。
    """
    import hashlib

    from PIL import Image

    x0, y0, x1, y1 = box
    with Image.open(png) as img:
        crop = img.convert("RGB").crop(
            (
                geo.x + int(geo.w * x0),
                geo.y + int(geo.h * y0),
                geo.x + int(geo.w * x1),
                geo.y + int(geo.h * y1),
            )
        )
        return hashlib.sha256(crop.tobytes()).hexdigest()[:16]


def open_decl_page(pid: int) -> bool:
    """设置 → 对话分类 → 场景声明卡。"""
    for _ in range(5):
        if not click_text(pid, "\u7cfb\u7edf\u504f\u597d\u8bbe\u7f6e"):
            time.sleep(2.0)
            continue
        time.sleep(3.0)
        if not click_text(pid, "\u5bf9\u8bdd\u5206\u7c7b"):
            time.sleep(2.0)
            continue
        if wait_for(lambda: "\u573a\u666f\u58f0\u660e" in deep_text(pid), 20.0, 0.5):
            return True
        time.sleep(2.0)
    return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":138")
    ap.add_argument("--root", default="/tmp/od123-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"\u62d2\u7edd\u6e05\u7406\u975e /tmp \u76ee\u5f55: {root}")
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
    decl_path = root / "app-data" / "profiles" / NAME / "decl.yml"
    base_dir = root / "app-data" / "profiles" / "omnidesk-base"
    try:
        session.prepare()
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = mount_window(session, xs)
        geo = xs.geometry(win)
        time.sleep(2.5)

        # --- 1. 基础标准：保存受限，Dock 无业务场景 -----------------------
        facts["checks"]["boot_base"] = (read_settings(root) or {}).get("engine_profile") is None
        facts["checks"]["decl_page_standard"] = open_decl_page(pid)
        # 标准下「立即保存」必须什么都不写：先灌模板（只改编辑框），再点保存。
        # 按钮本身是 disabled，点击多半不会命中，所以这里不把「点到了」当断言，
        # 只看磁盘：没有 123 的短路，这一步会把 decl.yml 写进 omnidesk-base。
        facts["notes"] = {}
        facts["notes"]["starter_clicked_on_base"] = click_text(pid, "\u586b\u5165\u8d77\u6b65\u6a21\u677f")
        facts["notes"]["save_clicked_on_base"] = click_text(pid, "\u6821\u9a8c\u5e76\u4fdd\u5b58")
        time.sleep(2.0)
        facts["checks"]["base_decl_not_written"] = not (base_dir / "decl.yml").exists()
        facts["checks"]["base_dir_not_created"] = not base_dir.exists()
        facts["standard_text_sample"] = deep_text(pid)[:2500]
        # 声明卡在分类列表下方，滚到底把受限提示拍进截图（文案不暴露给 AT-SPI）。
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h - 160, 12)
        xs.shot(SHOTS / "01-standard-decl-restricted.png")

        # --- 2. 新建一个自建 Profile 并切过去 -----------------------------
        facts["checks"]["popover_opened"] = open_popover_any(pid, xs, ["\u57fa\u7840\u6807\u51c6", "\u6807\u51c6"])
        facts["checks"]["create_form_opened"] = click_text(pid, "\u65b0\u5efa\u81ea\u5b9a\u4e49 Profile", exact=False)
        time.sleep(1.0)
        facts["checks"]["typed_name"] = type_into(pid, xs, "\u65b0 Profile \u540d\u5b57", NAME, alt="\u540d\u5b57\uff0c\u5982 my-work")
        facts["checks"]["typed_label"] = type_into(
            pid, xs, "\u65b0 Profile \u663e\u793a\u540d", LABEL, alt="\u663e\u793a\u540d\uff08\u53ef\u9009\uff09\uff0c\u5982 \u6211\u7684\u5de5\u4f5c\u53f0"
        )
        facts["checks"]["submitted"] = click_text(pid, "\u521b\u5efa")
        facts["checks"]["created_in_registry"] = bool(
            wait_for(
                lambda: any(p.get("name") == NAME for p in registry(root).get("profiles", [])),
                30.0,
                0.5,
            )
        )
        facts["checks"]["switch_confirmed"] = click_text(pid, "\u786e\u8ba4")
        facts["checks"]["switched_to_custom"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5)
        )
        # 122 的起手骨架已经写好声明；这里先删掉它，让本轮走「空声明 → 填模板 → 保存」
        # 的真实路径（删除只发生在 /tmp 隔离根里的合成数据）。
        time.sleep(2.0)
        if decl_path.is_file():
            decl_path.unlink()
        facts["checks"]["decl_cleared_for_test"] = not decl_path.exists()
        facts["checks"]["decl_page_custom"] = open_decl_page(pid)
        facts["checks"]["starter_filled"] = click_text(pid, "\u586b\u5165\u8d77\u6b65\u6a21\u677f")
        time.sleep(1.5)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h - 160, 6)
        xs.shot(SHOTS / "02-custom-decl-starter-filled.png")

        # --- 3. 校验并保存 → 不重启，Dock 立刻更新 ------------------------
        facts["checks"]["save_clicked"] = click_text(pid, "\u6821\u9a8c\u5e76\u4fdd\u5b58")
        facts["checks"]["decl_written"] = bool(wait_for(lambda: decl_path.is_file(), 30.0, 0.5))
        decl_text = decl_path.read_text(encoding="utf-8") if decl_path.is_file() else ""
        facts["checks"]["decl_saved_five_scenes"] = all(
            f"id: {sid}" in decl_text for sid in ("home", "report", "writing", "project", "sessions")
        )
        facts["decl_digest_on_disk"] = decl_text[:0] or f"{len(decl_text)}B"
        xs.shot(SHOTS / "02b-decl-saved.png")
        # 不重启：直接回工作台首页看 Dock。
        facts["checks"]["back_to_home"] = click_text(pid, "\u5de5\u4f5c\u53f0\u9996\u9875")
        facts["checks"]["dock_scenes_without_restart"] = bool(
            wait_for(
                lambda: all(s in deep_text(pid) for s in SCENES),
                30.0,
                0.5,
            )
        )
        facts["dock_text_after_save"] = deep_text(pid)[:2500]
        xs.shot(SHOTS / "03-dock-updated-without-restart.png")

        # --- 4. 切回基础标准：场景消失、保存重新受限 ----------------------
        facts["checks"]["popover_reopened"] = open_popover_any(pid, xs, [LABEL, NAME])
        facts["checks"]["picked_base"] = pick_profile(xs, pid, "\u57fa\u7840\u6807\u51c6", geo)
        facts["checks"]["back_to_base"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") is None, 40.0, 0.5)
        )
        facts["checks"]["base_has_no_scenes"] = bool(
            wait_for(lambda: not all(s in deep_text(pid) for s in SCENES), 30.0, 0.5)
        )
        facts["checks"]["decl_page_base_again"] = open_decl_page(pid)
        time.sleep(1.0)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h - 160, 12)
        facts["checks"]["base_decl_still_absent_after_roundtrip"] = not (base_dir / "decl.yml").exists()
        facts["checks"]["base_dir_still_absent"] = not base_dir.exists()
        xs.shot(SHOTS / "04-back-to-base.png")

        # --- 5. 停在「对话分类」页切换 Profile：声明必须当场重读 -------------
        # 这一页没有「打开时才读」的兜底 effect（其余分页靠 xxx === null 触发），
        # 只清缓存会让它一直显示空编辑框，而磁盘上刚被装配写好 decl.yml。
        #
        # 判据不能用文字匹配：编辑框的 placeholder 本身就写着
        # 「当前 Profile 没有场景声明。填入 omnidesk.profile-decl.v1 …」，
        # 这句在空与非空两种状态下都在 AX 树里（首跑踩过，断言恒假）。
        # 声明卡的 status-card 行也不暴露给 AT-SPI（118 / 122 已记录）。
        # 因此改看**编辑框区域的实际像素**：空（灰色 placeholder）与
        # 载入后的 YAML 正文差异极大，且这一带不含顶栏回执等会自行变化的元素。
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h - 160, 12)
        time.sleep(1.0)
        before = SHOTS / "05a-decl-page-empty.png"
        xs.shot(before)
        DECL_BOX = (0.15, 0.62, 0.85, 0.97)  # 声明编辑框所在区域（按窗口比例）
        facts["decl_band_before"] = band_hash(before, geo, DECL_BOX)
        facts["checks"]["popover_opened_from_decl_page"] = open_popover_any(
            pid, xs, ["基础标准", "标准"]
        )
        facts["checks"]["picked_custom_from_decl_page"] = pick_profile(xs, pid, LABEL, geo)
        facts["checks"]["switched_while_on_decl_page"] = bool(
            wait_for(lambda: (read_settings(root) or {}).get("engine_profile") == NAME, 40.0, 0.5)
        )
        # 切换后页面会重排，重新滚到声明卡再取同一带。
        time.sleep(2.5)
        xs.scroll(geo.x + geo.w // 2, geo.y + geo.h - 160, 12)
        time.sleep(1.5)
        after = SHOTS / "05b-decl-reloaded.png"
        xs.shot(after)
        facts["decl_band_after"] = band_hash(after, geo, DECL_BOX)
        facts["checks"]["decl_reloaded_without_page_switch"] = (
            facts["decl_band_before"] != facts["decl_band_after"]
        )
        facts["decl_page_text_after_switch"] = deep_text(pid)[:2500]
        xs.shot(SHOTS / "05-decl-reloaded-on-profile-switch.png")
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
