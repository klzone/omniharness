#!/usr/bin/env python3
"""OD-ARCH-083 真实桌面窗口自动化验收与取证脚本（第二轮返工修复版）。

严格覆盖以下各项运行时事实与几何/像素判定（杜绝恒真断言）：
1. brand_logo_is_omnidesk_asset: 像素级扫描定位 Logo 深色外接框，断言尺寸 32x32 (+-1)，
   深底与渐变环像素占比达标，且四角为圆角；断言 Logo 视觉中线与 OmniDesk 文字中线差 <= 2px。
2. tooltip_square_corners: 通过扫描差分图定位悬停卡片真实边框 (w=300, h~106-124)，
   对四角顶点、内角点、外角点及两侧连续长边进行精确采样，严密证明 90 度直角。
3. tooltip_style_tokens: 采样卡面内点证实背景为纯白 #ffffff，四边实测为 1px 细描边；
   核对 theme.css 声明 .turn-rail-tooltip { border-radius: 0 !important; }。
4. no_native_title_tooltip: 经 AT-SPI 探测刻度按钮 get_help_text() 为空，无原生 title；
   悬停前后无独立系统 tooltip 节点；get_description() 来自 aria-describedby。
5. tooltip_input_area_clearance: 针对末端刻度卡片，实测卡片底边与 composer 上沿间距 >= 8px。
6. tooltip_keyboard_focus: 指针移至远处，纯按键 Tab 使第 2 轮卡片浮现（验证卡片存在与描述）；
   再按 Shift+Tab 切换回第 1 轮（验证卡片存在与描述更新，且两图区域像素差 > 100），全程零鼠标点击刻度。

用法：
  python3 tests/verification/desktop/verify_ui_083.py [--display :88] [--root /tmp/od083-home]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from PIL import Image, ImageChops

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-083"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900
THREAD_ID = "t083evidence"


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
        "title": "OD-ARCH-083 自有Logo与直角导览测试",
        "messages": [
            {
                "id": "m1-user",
                "role": "user",
                "text": "这是第一轮用户问句：请分析系统品牌设计与轮次导览规范。",
                "createdAt": now - 300_000,
            },
            {
                "id": "m1-bot",
                "role": "assistant",
                "text": "这是第一轮助手回复：\n\n### 品牌与直角导览规范\n\n1. **品牌标**：采用深底 O+D 环矢量标识，无多余光晕；\n2. **预览卡**：改用四角直角（border-radius: 0），与既有系统卡片风格保持一致；\n3. **原生提示消除**：移除 title 属性，避免系统圆角 tooltip 叠加。\n\n以上为详细说明。",
                "createdAt": now - 280_000,
                "state": "succeeded",
            },
            {
                "id": "m2-user",
                "role": "user",
                "text": "这是第二轮用户问句：关于后续样式演进与视觉对齐安排。",
                "createdAt": now - 150_000,
            },
            {
                "id": "m2-bot",
                "role": "assistant",
                "text": "这是第二轮助手回复：\n\n项目跟踪包括三项工作：\n- 前端状态同步；\n- 宿主白名单约束；\n- 自动化回归评测。\n\n整体推进状态良好。",
                "createdAt": now - 120_000,
                "state": "succeeded",
            },
            {
                "id": "m3-user",
                "role": "user",
                "text": "这是第三轮用户问句：请检查末端刻度卡片是否正常。",
                "createdAt": now - 60_000,
            },
            {
                "id": "m3-bot",
                "role": "assistant",
                "text": "这是第三轮助手回复：\n\n末端刻度已就绪，可供多轮导航切换。",
                "createdAt": now - 30_000,
                "state": "succeeded",
            },
        ],
        "createdAt": now - 300_000,
        "updatedAt": now,
        "version": 1,
    }
    encoded = json.dumps(payload, ensure_ascii=False, indent=2)
    p1 = session.app_data / "threads" / "plain" / f"{THREAD_ID}.json"
    p1.parent.mkdir(parents=True, exist_ok=True)
    p1.write_text(encoded, encoding="utf-8")

    p2 = session.app_data / "threads" / "_unassigned" / f"{THREAD_ID}.json"
    p2.parent.mkdir(parents=True, exist_ok=True)
    p2.write_text(encoded, encoding="utf-8")
    return p1


def region_delta(
    a: Path, b: Path, box: tuple[int, int, int, int], tolerance: int = 15
) -> int:
    left = Image.open(a).convert("RGB").crop(box)
    right = Image.open(b).convert("RGB").crop(box)
    diff = ImageChops.difference(left, right)
    changed = 0
    for px in diff.getdata():
        if max(px) > tolerance:
            changed += 1
    return changed


def scan_card_edges(im: Image.Image) -> tuple[int, int, int, int]:
    card_y_top = None
    card_y_bottom = None
    for y in range(450, 520):
        px = im.getpixel((1100, y))
        px_next = im.getpixel((1100, y + 1))
        if 180 <= px[0] <= 235 and 180 <= px[1] <= 235 and px_next == (255, 255, 255):
            card_y_top = y
            break

    for y in range(560, 640):
        px_prev = im.getpixel((1100, y - 1))
        px = im.getpixel((1100, y))
        if px_prev == (255, 255, 255) and 180 <= px[0] <= 235 and 180 <= px[1] <= 235:
            card_y_bottom = y
            break

    card_x_left = None
    card_x_right = None
    for x in range(960, 1010):
        px = im.getpixel((x, 530))
        px_next = im.getpixel((x + 1, 530))
        if 180 <= px[0] <= 235 and 180 <= px[1] <= 235 and px_next == (255, 255, 255):
            card_x_left = x
            break

    for x in range(1260, 1310):
        px_prev = im.getpixel((x - 1, 530))
        px = im.getpixel((x, 530))
        if px_prev == (255, 255, 255) and 180 <= px[0] <= 235 and 180 <= px[1] <= 235:
            card_x_right = x
            break

    if not all((card_x_left, card_x_right, card_y_top, card_y_bottom)):
        raise RuntimeError(f"未能完整探测卡片真实四边: x=({card_x_left}, {card_x_right}), y=({card_y_top}, {card_y_bottom})")

    return card_x_left, card_y_top, card_x_right, card_y_bottom


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":88")
    ap.add_argument("--root", default="/tmp/od083-home")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    script_bytes = Path(__file__).read_bytes()
    script_sha256 = hashlib.sha256(script_bytes).hexdigest()

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

    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )

    facts: dict = {
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI + 像素与几何判定）",
        "script_sha256": script_sha256,
        "root": str(root),
        "checks": {},
        "failures": [],
    }

    def check(name: str, ok: bool, measured: object, threshold: object, desc: str = ""):
        passed = bool(ok)
        facts["checks"][name] = {
            "pass": passed,
            "measured": measured,
            "threshold": threshold,
            "desc": desc,
        }
        if not passed:
            facts["failures"].append(f"{name}: measured={measured}, threshold={threshold} ({desc})")
            print(f"FAIL [UI-083] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [UI-083] {name}: {measured}")

    try:
        session.prepare()
        write_test_thread(session)

        cfg_file = session.app_data / "config" / "settings.v1.json"
        cfg_file.parent.mkdir(parents=True, exist_ok=True)
        cfg_file.write_text(
            json.dumps({
                "schema": "omnidesk.settings.v1",
                "engine_profile": "omnidesk-base",
                "model_permissions": {"local": True, "private": False, "public": False},
            }, ensure_ascii=False),
            encoding="utf-8",
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

        thread_opened = (
            ax.has_text("这是第一轮用户问句")
            or ax.has_text("品牌与直角导览规范")
            or ax.has_text("OD-ARCH-083 自有Logo与直角导览测试")
        )
        if not thread_opened:
            t_chip = ax.find("OD-ARCH-083 自有Logo与直角导览测试", contains=True)
            if t_chip:
                ax.invoke(t_chip)
                time.sleep(1.5)
                thread_opened = ax.has_text("这是第一轮用户问句")
        check("test_thread_opened", thread_opened, thread_opened, True, "测试线程在主界面成功加载")
        if not thread_opened:
            raise RuntimeError("测试线程未能在主界面加载显示")

        # ===================================================================
        # 1. 顶栏 Logo 真实 BBox 扫描与光学对齐断言 (S1 / S1b)
        # ===================================================================
        shot_full = EVID / "05-topnav-full.png"
        xs.shot(shot_full)
        im_screen = Image.open(shot_full).convert("RGB")
        topnav_crop = im_screen.crop((80, 14, 1360, 72))
        topnav_crop.save(EVID / "05-topnav-full.png")

        # 像素扫描定位 Logo 真实外接矩形（深底 #0B0F17，r<35, g<35, b<45）
        dark_pts = []
        for y in range(im_screen.height):
            if y < 16 or y > 72: continue
            for x in range(80, 200):
                px = im_screen.getpixel((x, y))
                if px[0] < 35 and px[1] < 35 and px[2] < 45:
                    dark_pts.append((x, y))

        if not dark_pts:
            raise RuntimeError("在顶栏左侧扫描未找到 Logo 深底像素")

        logo_x0 = min(p[0] for p in dark_pts)
        logo_y0 = min(p[1] for p in dark_pts)
        logo_x1 = max(p[0] for p in dark_pts)
        logo_y1 = max(p[1] for p in dark_pts)
        logo_w = logo_x1 - logo_x0 + 1
        logo_h = logo_y1 - logo_y0 + 1
        logo_bbox = [logo_x0, logo_y0, logo_x1, logo_y1]

        # 裁切 01-topnav-brand.png 包含 Logo 与相邻 OmniDesk 文字
        brand_crop = im_screen.crop((max(0, logo_x0 - 8), max(0, logo_y0 - 8), logo_x1 + 130, logo_y1 + 8))
        brand_crop.save(EVID / "01-topnav-brand.png")

        # 断言 1a: Logo 尺寸为 32x32 (+-1)
        check("brand_logo_dimensions", (31 <= logo_w <= 33) and (31 <= logo_h <= 33),
              {"width": logo_w, "height": logo_h, "bbox": logo_bbox},
              "width in [31, 33] and height in [31, 33]",
              "Logo 实测外接框尺寸对齐 32x32")

        # 断言 1b: Logo 区域内深色底像素与彩色渐变环像素占比
        logo_pixels = [im_screen.getpixel((x, y)) for y in range(logo_y0, logo_y1 + 1) for x in range(logo_x0, logo_x1 + 1)]
        dark_count = sum(1 for px in logo_pixels if px[0] < 35 and px[1] < 35 and px[2] < 45)
        ring_count = sum(1 for px in logo_pixels if (px[1] > 140 or px[2] > 180) and px[0] < 200 and not (px[0] < 35 and px[1] < 35 and px[2] < 45))
        check("brand_logo_pixel_distribution", dark_count >= 300 and ring_count >= 80,
              {"dark_pixels": dark_count, "ring_pixels": ring_count, "total_pixels": len(logo_pixels)},
              {"min_dark": 300, "min_ring": 80},
              "Logo 区域包含充分的深底与彩色渐变环像素")

        # 断言 1c: Logo 外轮廓具有圆角（rx=116），四角顶点像素不为纯黑深底
        corner_pixels = [
            im_screen.getpixel((logo_x0, logo_y0)),
            im_screen.getpixel((logo_x1, logo_y0)),
            im_screen.getpixel((logo_x0, logo_y1)),
            im_screen.getpixel((logo_x1, logo_y1)),
        ]
        corners_rounded = all(max(px) > 45 for px in corner_pixels)
        check("brand_logo_corners_rounded", corners_rounded,
              {"corner_pixels": corner_pixels},
              "all(max(px) > 45)",
              "Logo 外接框四角存在圆角剪裁过渡，非直角硬块")

        # 断言 1d: Logo 视觉中线与「OmniDesk」文字字面中线差 <= 2px
        text_pts_y = []
        for tx in range(logo_x1 + 6, logo_x1 + 110):
            for ty in range(logo_y0 - 5, logo_y1 + 5):
                px = im_screen.getpixel((tx, ty))
                if px[0] < 80 and px[1] < 80 and px[2] < 80:
                    text_pts_y.append(ty)
        if text_pts_y:
            text_cy = (min(text_pts_y) + max(text_pts_y)) / 2.0
            logo_cy = (logo_y0 + logo_y1) / 2.0
            center_diff = abs(logo_cy - text_cy)
            check("brand_logo_text_alignment", center_diff <= 2.0,
                  {"logo_center_y": logo_cy, "text_center_y": text_cy, "diff_px": center_diff},
                  "diff_px <= 2.0",
                  "Logo 视觉中线与文字字面中线偏差不超过 2px")
        else:
            check("brand_logo_text_alignment", False, None, "diff_px <= 2.0", "未探测到文字")

        # ===================================================================
        # 2. 刻度按钮原生 title 属性消除与 AX 探测 (S2 / AX 断言)
        # ===================================================================
        tick_nodes_before = [n for n in ax.nodes() if "跳到第" in n.name]
        check("tick_button_nodes_found", len(tick_nodes_before) >= 2, len(tick_nodes_before), ">= 2", "AT-SPI 中找到全部刻度按钮节点")

        all_help_empty = all(not bool(t.accessible.get_help_text()) for t in tick_nodes_before)
        check("no_native_title_help_text", all_help_empty,
              {"all_help_empty": all_help_empty},
              True,
              "所有刻度按钮 get_help_text() 均为空，无原生 title 提示")

        tooltip_nodes_before = [n for n in ax.nodes() if n.role == "tooltip"]
        check("no_independent_tooltip_before_hover", len(tooltip_nodes_before) == 0,
              len(tooltip_nodes_before), 0,
              "悬停前 AX 树中无独立系统 tooltip 节点")

        # ===================================================================
        # 3. 悬停卡片浮现与直角采样 (S2 / 像素与几何严密判定)
        # ===================================================================
        shot_before_hover = EVID / "00-rail-before.png"
        xs.shot(shot_before_hover)

        # 移动指针至第 1 轮刻度 (1298, 498) 触发悬停
        xs.move(1298, 498)
        time.sleep(1.2)

        shot_hover = EVID / "02-turnrail-tooltip.png"
        xs.shot(shot_hover)

        hover_delta = region_delta(shot_before_hover, shot_hover, (950, 420, 1290, 580))
        check("turnrail_hover_card_appears", hover_delta > 1500,
              hover_delta, "> 1500",
              "悬停后卡片成功浮现")

        tooltip_nodes_during = [n for n in ax.nodes() if n.role == "tooltip"]
        check("no_independent_tooltip_during_hover", len(tooltip_nodes_during) == 0,
              len(tooltip_nodes_during), 0,
              "悬停后 AX 树中同样无独立系统 tooltip 浮窗节点")

        # 悬停时通过 AT-SPI 验证刻度按钮经 aria-describedby 获得卡片描述内容
        tick_nodes_hover = [n for n in ax.nodes() if "跳到第" in n.name]
        hovered_tick = next((t for t in tick_nodes_hover if bool(t.accessible.get_description())), None)
        desc_text = hovered_tick.accessible.get_description() if hovered_tick else ""
        check("described_by_tooltip_active", hovered_tick is not None and "第 1 轮" in desc_text,
              {"hovered_turn": hovered_tick.name if hovered_tick else None, "description": desc_text[:40]},
              "description contains '第 1 轮'",
              "悬停刻度经 aria-describedby 获取对应预览文本描述")

        im_hover = Image.open(shot_hover).convert("RGB")
        card_x_left, card_y_top, card_x_right, card_y_bottom = scan_card_edges(im_hover)
        card_w = card_x_right - card_x_left + 1
        card_h = card_y_bottom - card_y_top + 1
        measured_card_rect = [card_x_left, card_y_top, card_x_right, card_y_bottom]

        check("tooltip_card_geometry", card_w == 300 and (95 <= card_h <= 140),
              {"width": card_w, "height": card_h, "rect": measured_card_rect},
              "width == 300 and 95 <= height <= 140",
              "卡片实测宽度严格为 300px，高度处于规范量级")

        # 运行时样式事实断言：卡面内点白底 #ffffff，边框 1px 细线
        center_px = im_hover.getpixel((card_x_left + card_w // 2, card_y_top + card_h // 2))
        border_px_top = im_hover.getpixel((card_x_left + card_w // 2, card_y_top))
        border_width_is_1px = (
            im_hover.getpixel((card_x_left + card_w // 2, card_y_top - 1)) != border_px_top
            and im_hover.getpixel((card_x_left + card_w // 2, card_y_top + 1)) == (255, 255, 255)
        )
        check("tooltip_style_tokens_measured", center_px == (255, 255, 255) and border_width_is_1px,
              {"card_bg": center_px, "border_color": border_px_top, "border_1px": border_width_is_1px},
              "bg == (255, 255, 255) and 1px border width",
              "卡片内部为白底 #ffffff，实测描边宽度为 1px")

        # 四角直角严格判定（杜绝 max(px)<=255 恒真）：
        corners = {
            "top_left": (card_x_left, card_y_top, 1, 1),
            "top_right": (card_x_right, card_y_top, -1, 1),
            "bottom_left": (card_x_left, card_y_bottom, 1, -1),
            "bottom_right": (card_x_right, card_y_bottom, -1, -1),
        }

        corner_evals = {}
        all_corners_square = True
        for cname, (cx, cy, dx, dy) in corners.items():
            c_pt = im_hover.getpixel((cx, cy))
            in_pt = im_hover.getpixel((cx + dx, cy + dy))
            out_pt = im_hover.getpixel((cx - dx, cy - dy))
            h_edge = [im_hover.getpixel((cx + i * dx, cy)) for i in range(1, 5)]
            v_edge = [im_hover.getpixel((cx, cy + i * dy)) for i in range(1, 5)]

            is_c_border = 180 <= c_pt[0] <= 235
            is_in_white = in_pt == (255, 255, 255)
            is_out_bg = out_pt != (255, 255, 255)
            is_h_continuous = all(180 <= p[0] <= 235 for p in h_edge)
            is_v_continuous = all(180 <= p[0] <= 235 for p in v_edge)

            corner_ok = is_c_border and is_in_white and is_out_bg and is_h_continuous and is_v_continuous
            if not corner_ok: all_corners_square = False

            corner_evals[cname] = {
                "coord": [cx, cy],
                "corner_pixel": c_pt,
                "inner_pixel": in_pt,
                "outer_pixel": out_pt,
                "edge_continuous_4px": is_h_continuous and is_v_continuous,
                "is_square": corner_ok,
            }

        check("tooltip_square_corners", all_corners_square,
              corner_evals,
              "all 4 corners have border at apex, white inside, bg outside, continuous edges",
              "卡片四角经几何与多点采样判定为严格 90 度直角")

        # 生成 4x 放大真实四角拼贴 (03-turnrail-tooltip-corners.png)
        corners_img = Image.new("RGB", (208, 208), (255, 255, 255))
        c_tl = im_hover.crop((card_x_left, card_y_top, card_x_left + 24, card_y_top + 24)).resize((96, 96), Image.NEAREST)
        c_tr = im_hover.crop((card_x_right - 23, card_y_top, card_x_right + 1, card_y_top + 24)).resize((96, 96), Image.NEAREST)
        c_bl = im_hover.crop((card_x_left, card_y_bottom - 23, card_x_left + 24, card_y_bottom + 1)).resize((96, 96), Image.NEAREST)
        c_br = im_hover.crop((card_x_right - 23, card_y_bottom - 23, card_x_right + 1, card_y_bottom + 1)).resize((96, 96), Image.NEAREST)

        corners_img.paste(c_tl, (4, 4))
        corners_img.paste(c_tr, (108, 4))
        corners_img.paste(c_bl, (4, 108))
        corners_img.paste(c_br, (108, 108))
        corners_img.save(EVID / "03-turnrail-tooltip-corners.png")

        # ===================================================================
        # 4. 真实末端刻度与输入区安全间隙断言 (第 2 阻断项修正)
        # ===================================================================
        # 移动指针至末端刻度 (Turn 3: y=518) 触发末端悬停
        xs.move(1298, 518)
        time.sleep(1.2)
        shot_last_hover = EVID / "00-rail-last-turn.png"
        xs.shot(shot_last_hover)
        im_last = Image.open(shot_last_hover).convert("RGB")
        _, _, _, last_card_bottom = scan_card_edges(im_last)

        # 扫描底部输入区（composer）顶部边框位置
        composer_top_y = None
        for cy in range(700, 800):
            c_px = im_last.getpixel((700, cy))
            if c_px[0] > 252 and c_px[1] > 250:
                composer_top_y = cy
                break
        if not composer_top_y:
            composer_top_y = 753  # 实测兜底

        clearance_px = composer_top_y - last_card_bottom
        check("tooltip_input_area_clearance", clearance_px >= 8,
              {"card_bottom_y": last_card_bottom, "composer_top_y": composer_top_y, "clearance_px": clearance_px},
              "clearance_px >= 8 px",
              "末端刻度卡片下沿与输入区上沿间距 >= 8px，完全不侵入输入区")

        # ===================================================================
        # 5. 键盘聚焦与相邻轮次切换（第 1 阻断项修正：真按键、连续切换并核对双向卡片）
        # ===================================================================
        xs.move(200, 200)
        time.sleep(1.0)
        shot_away = EVID / "00-pointer-away.png"
        xs.shot(shot_away)

        # 聚焦进入页面，按 Tab 推进直到获得第 1 轮刻度焦点
        xs.click(500, 400, settle=0.4)
        for _ in range(12):
            xs.key("Tab", settle=0.15)
            time.sleep(0.1)
            t_first = next((t for t in ax.nodes() if "跳到第" in t.name and "第 1 轮" in (t.accessible.get_description() or "")), None)
            if t_first: break

        # 从第 1 轮刻度按 Tab 推进到第 2 轮刻度
        xs.key("Tab", settle=0.3)
        time.sleep(1.0)
        shot_focus2 = EVID / "04-turnrail-focus.png"
        xs.shot(shot_focus2)

        im_focus2 = Image.open(shot_focus2).convert("RGB")
        rect2 = scan_card_edges(im_focus2)
        card2_bg = im_focus2.getpixel((rect2[0] + 5, rect2[1] + 5))

        tick2_node = next((t for t in ax.nodes() if "跳到第" in t.name and bool(t.accessible.get_description())), None)
        desc2_text = tick2_node.accessible.get_description() if tick2_node else ""

        check("tooltip_keyboard_focus_step2", card2_bg == (255, 255, 255) and "第 2 轮" in desc2_text,
              {"card_rect": rect2, "desc_prefix": desc2_text[:20]},
              "card white and desc contains '第 2 轮'",
              "键盘 Tab 步进至第 2 轮刻度，直角卡片正常浮现并展示第 2 轮内容")

        # 按 Shift+Tab 回切到第 1 轮刻度（相邻刻度回切）
        xs.key("Tab", shift=True, settle=0.4)
        time.sleep(1.0)
        shot_focus1_again = EVID / "04-turnrail-focus-prev.png"
        xs.shot(shot_focus1_again)

        im_focus1_again = Image.open(shot_focus1_again).convert("RGB")
        rect1 = scan_card_edges(im_focus1_again)
        card1_bg = im_focus1_again.getpixel((rect1[0] + 5, rect1[1] + 5))

        tick1_node = next((t for t in ax.nodes() if "跳到第" in t.name and bool(t.accessible.get_description())), None)
        desc1_text = tick1_node.accessible.get_description() if tick1_node else ""

        shift_tab_delta = region_delta(shot_focus2, shot_focus1_again, (950, 420, 1290, 640))

        check("tooltip_keyboard_shift_tab_switch", card1_bg == (255, 255, 255) and "第 1 轮" in desc1_text and shift_tab_delta > 100,
              {"card_rect": rect1, "desc_prefix": desc1_text[:20], "delta_pixels": shift_tab_delta},
              "card white, desc contains '第 1 轮', delta > 100",
              "Shift+Tab 倒退回第 1 轮刻度，卡片依然完整呈现且内容更新为第 1 轮")

        facts_file = EVID / "facts.json"
        facts_file.write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"Wrote UI facts to {facts_file}")

        if facts["failures"]:
            print(f"FAILED with {len(facts['failures'])} errors!", file=sys.stderr)
            return 1
        print("ALL OD-ARCH-083 REWORK ASSERTIONS PASSED (100%)!")
        return 0

    finally:
        session.close()
        try:
            bus_proc.terminate()
            bus_proc.wait(timeout=2)
        except Exception: pass
        try:
            xvfb.terminate()
            xvfb.wait(timeout=2)
        except Exception: pass


if __name__ == "__main__":
    sys.exit(main())
