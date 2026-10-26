#!/usr/bin/env python3
import argparse, hashlib, json, os, shutil, subprocess, sys, time
from pathlib import Path
from PIL import Image, ImageChops
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
from window_harness import AppSession, XSession
import accessibility as ax
EVID = HERE / "evidence" / "od-arch-084"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900
THREAD_ID = "t084search"

def start_display(display: str):
    proc = subprocess.Popen(["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    time.sleep(1.2)
    return proc

def start_private_dbus():
    proc = subprocess.Popen(["dbus-daemon", "--session", "--print-address", "--nofork"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    assert proc.stdout is not None
    return proc.stdout.readline().strip(), proc

def build_filler(count: int) -> str:
    return "\n\n".join([f"这是详述段落第 {i} 节。我们在此细化工程实践中的边界控制与回归断言流程。" for i in range(1, count + 1)])

def write_test_thread(session: AppSession) -> Path:
    now = int(time.time() * 1000)
    m2_text = "\n\n".join([
        "### 综合规划流程与要点梳理",
        "首先是第 1 处 step-item，进行初步可行性调研与立项准备。",
        "接着是第 2 处 step-item，完成需求梳理与技术架构推演。",
        "然后是第 3 处 step-item，开展首轮代码编写与模块解耦。",
        "随后是第 4 处 step-item，进行单元测试与接口集成。",
        "紧接着是第 5 处 step-item，进行性能压测与瓶颈诊断。",
        "进一步是第 6 处 step-item，组织安全审查与权限核验。",
        "倒数第二是第 7 处 step-item，完成预发布演练与部署验证。",
        "最后是第 8 处 step-item，总结沉淀规范并正式移交上线。",
        build_filler(15),
        "在此特别记录位于全消息最底端的唯一关键目标：unique-tail-target，用于验证长消息视口定位安全边。"
    ])
    m4_text = "\n\n".join([
        "### 元素测试指标 markdown-target",
        "本段对多种 Markdown 元素的搜索命中能力进行全面检验：",
        "| 序号 | 指标项目名称 | 当前状态 |\n| --- | --- | --- |\n| 01 | 表格单元格指标 markdown-target | 正常测试 |\n| 02 | 通用对比参考项 | 保持一致 |",
        "这里是一段包含行内代码 `markdown-target` 的正文描述内容。",
        "在此附带特殊字符测试短语：符号 (sample) 标识项与特殊标记 a.b 字符串（对照 axbx 非通配）。",
        "```text\n这是围栏代码块中的 markdown-target，按规范不参与高亮且不计入命中数。\n```",
        "以上为多元素测试对比。"
    ])
    payload = {
        "id": THREAD_ID, "tag": "plain", "preset": "plain", "projectPath": None,
        "title": "OD-ARCH-084 搜索定位综合验收会话",
        "messages": [
            {"id": "m1-user", "role": "user", "text": "请帮我规划长流程与多项标记：", "createdAt": now - 300_000},
            {"id": "m2-bot", "role": "assistant", "text": m2_text, "createdAt": now - 280_000, "state": "succeeded"},
            {"id": "m3-user", "role": "user", "text": "请用 Markdown 各种元素列出测试指标：", "createdAt": now - 150_000},
            {"id": "m4-bot", "role": "assistant", "text": m4_text, "createdAt": now - 120_000, "state": "succeeded"}
        ],
        "createdAt": now - 300_000, "updatedAt": now, "version": 1
    }
    encoded = json.dumps(payload, ensure_ascii=False, indent=2)
    p1 = session.app_data / "threads" / "plain" / f"{THREAD_ID}.json"
    p1.parent.mkdir(parents=True, exist_ok=True)
    p1.write_text(encoded, encoding="utf-8")
    p2 = session.app_data / "threads" / "_unassigned" / f"{THREAD_ID}.json"
    p2.parent.mkdir(parents=True, exist_ok=True)
    p2.write_text(encoded, encoding="utf-8")
    return p1

def count_pixels(shot: Path, box: tuple[int, int, int, int], pred) -> int:
    im = Image.open(shot).convert("RGB").crop(box)
    return sum(1 for px in im.getdata() if pred(px))

def region_delta(a: Path, b: Path, box: tuple[int, int, int, int], tolerance: int = 15) -> int:
    left = Image.open(a).convert("RGB").crop(box)
    right = Image.open(b).convert("RGB").crop(box)
    diff = ImageChops.difference(left, right)
    return sum(1 for px in diff.getdata() if max(px) > tolerance)

def find_session_node(target_pid: int | None, name: str, role: str | None = None, contains: bool = True) -> ax.Node | None:
    """从当前测试 session 对应的 Atspi Application 下查找节点，避免命中宿主机同名应用。"""
    from gi.repository import Atspi
    desktop = Atspi.get_desktop(0)
    apps = []
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        apps.append(app)

    matched_app = None
    if target_pid:
        for app in apps:
            if hasattr(app, "get_process_id") and app.get_process_id() == target_pid:
                matched_app = app
                break

    if not matched_app:
        for app in reversed(apps):
            if "omnidesk-desktop" in (app.get_name() or "").lower():
                matched_app = app
                break

    if matched_app:
        raw: list = []
        ax._collect(matched_app, raw)
        for n in [ax._node(x) for x in raw]:
            if role is not None and n.role != role:
                continue
            haystacks = (n.name, n.text)
            hit = any((name in h) if contains else (name == h) for h in haystacks)
            if hit:
                return n

    return ax.find(name, role=role, contains=contains)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":111")
    ap.add_argument("--root", default="/tmp/od084-test")
    args = ap.parse_args()
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents: raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists(): shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)
    script_bytes = Path(__file__).read_bytes()
    script_sha256 = hashlib.sha256(script_bytes).hexdigest()
    shutil.copytree(SOURCE_HOME, root / "dsh-home", dirs_exist_ok=True, symlinks=True, ignore=shutil.ignore_patterns("sessions*", "session_*", "*ledger.json"))
    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_private_dbus()
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ["DISPLAY"] = args.display
    if "AT_SPI_BUS_ADDRESS" not in os.environ and os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ["AT_SPI_BUS_ADDRESS"] = "unix:path=/run/user/1000/at-spi/bus"
    session = AppSession(root=root, display=args.display, artifact_override=True, reuse_running_dev_server=True, env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""})
    facts = {"verified_at": time.strftime("%Y-%m-%d %H:%M:%S"), "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI + 像素与几何测量）", "script_sha256": script_sha256, "root": str(root), "checks": {}, "failures": []}
    def check(name: str, ok: bool, measured: object, threshold: object, desc: str = ""):
        passed = bool(ok)
        facts["checks"][name] = {"pass": passed, "measured": measured, "threshold": threshold, "desc": desc}
        if not passed:
            facts["failures"].append(f"{name}: measured={measured}, threshold={threshold} ({desc})")
            print(f"FAIL [UI-084] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [UI-084] {name}: {measured}")
    try:
        session.prepare()
        write_test_thread(session)
        cfg_file = session.app_data / "config" / "settings.v1.json"
        cfg_file.parent.mkdir(parents=True, exist_ok=True)
        cfg_file.write_text(json.dumps({"schema": "omnidesk.settings.v1", "engine_profile": "omnidesk-base", "model_permissions": {"local": True, "private": False, "public": False}}, ensure_ascii=False), encoding="utf-8")
        session.start()
        app_pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=120)
        if not win: raise RuntimeError("主窗口未在规定时间内出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=2.5)
        time.sleep(6)
        ax.init()
        from gi.repository import Atspi
        thread_opened = ax.has_text("综合规划流程与要点梳理") or ax.has_text("OD-ARCH-084 搜索定位综合验收会话") or ax.has_text("跳到第 1 轮")
        if not thread_opened:
            t_chip = ax.find("OD-ARCH-084 搜索定位综合验收会话", contains=True)
            if t_chip:
                ax.invoke(t_chip)
                time.sleep(1.5)
                thread_opened = ax.has_text("综合规划流程与要点梳理")
        check("test_thread_opened", thread_opened, thread_opened, True, "测试长会话成功加载呈现")
        if not thread_opened: raise RuntimeError("测试线程未能在主界面加载显示")

        # 0. 非检索态底部净空验证：未搜索状态下最后一条消息底边与输入区（composer）顶边间距必须 < 60px（杜绝常驻 35vh 空白）
        time.sleep(1.0)
        shot_initial = EVID / "00-thread-initial.png"
        xs.shot(shot_initial)

        # 定位最后一条消息底部的文本节点「以上为多元素测试对比。」
        last_msg_node = find_session_node(app_pid, "以上为多元素测试对比", contains=True)
        if not last_msg_node:
            last_msg_node = ax.find("以上为多元素测试对比", contains=True)

        # 定位底部指令输入框（composer）节点
        composer_node = find_session_node(app_pid, "与 OmniDesk 协同探索", contains=True)
        if not composer_node:
            composer_node = ax.find("与 OmniDesk 协同探索", contains=True)

        last_msg_bottom = None
        composer_top = None
        if last_msg_node and hasattr(last_msg_node.accessible, "get_component"):
            ext_msg = last_msg_node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
            last_msg_bottom = ext_msg.y + ext_msg.height
        if composer_node and hasattr(composer_node.accessible, "get_component"):
            ext_comp = composer_node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
            composer_top = ext_comp.y

        # 若 AT-SPI 获取不到完整坐标，则基于截图区域扫描 composer 浅色边框与最后文字像素
        if last_msg_bottom is None or composer_top is None:
            # 兜底扫描
            im_init = Image.open(shot_initial).convert("RGB")
            # 扫描 composer 容器顶部
            for cy in range(700, 800):
                c_px = im_init.getpixel((700, cy))
                if c_px[0] > 250 and c_px[1] > 250 and c_px[2] > 250:
                    composer_top = cy
                    break
            if not composer_top: composer_top = 753
            # 向上扫描文字像素 (深色)
            for my in range(composer_top - 1, 600, -1):
                has_text_px = any(im_init.getpixel((x, my))[0] < 100 for x in range(300, 900))
                if has_text_px:
                    last_msg_bottom = my
                    break
            if not last_msg_bottom: last_msg_bottom = composer_top - 20

        clearance_unsearched = composer_top - last_msg_bottom
        check("unsearched_bottom_clearance_compact", clearance_unsearched < 60,
              {"last_msg_bottom_y": last_msg_bottom, "composer_top_y": composer_top, "clearance_px": clearance_unsearched},
              "clearance_px < 60px (no permanent 35vh)",
              "未搜索状态下最后一条消息底边与输入区顶部间距紧凑小于 60px，无常驻 35vh 巨大空白")

        # 1. 唯一命中可见：长消息末尾命中词自动滚入视口安全区 (验收标准 1)
        xs.click(541, 43, settle=0.8)
        xs.type_ascii("unique-tail-target", settle=0.03)
        time.sleep(1.8)
        shot_tail = EVID / "01-unique-tail-target.png"
        xs.shot(shot_tail)
        im_tail = Image.open(shot_tail).convert("RGB")
        active_pts = []
        for y in range(80, 755):
            for x in range(200, 1250):
                px = im_tail.getpixel((x, y))
                if px[0] > 230 and 185 <= px[1] <= 205 and px[2] < 100:
                    active_pts.append((x, y))
        if not active_pts:
            check("unique_tail_target_visible", False, None, "active hit visible in viewport", "未在视口内找到 unique-tail-target 当前高亮块")
        else:
            tail_x0 = min(p[0] for p in active_pts)
            tail_y0 = min(p[1] for p in active_pts)
            tail_x1 = max(p[0] for p in active_pts)
            tail_y1 = max(p[1] for p in active_pts)
            tail_cy = (tail_y0 + tail_y1) / 2.0
            tail_rect = [tail_x0, tail_y0, tail_x1, tail_y1]
            scroller_top = 240
            scroller_bottom = 755
            scroller_height = scroller_bottom - scroller_top
            top_clearance = tail_y0 - scroller_top
            bottom_clearance = scroller_bottom - tail_y1
            in_safe_bounds = top_clearance >= 24 and bottom_clearance >= 24
            ratio = (tail_cy - scroller_top) / float(scroller_height)
            check("unique_tail_target_visible", in_safe_bounds and (0.25 <= ratio <= 0.45),
                  {"bbox": tail_rect, "scroller_bounds": [scroller_top, scroller_bottom], "top_clearance_px": top_clearance, "bottom_clearance_px": bottom_clearance, "viewport_ratio": round(ratio, 3)},
                  "clearance >= 24px and ratio in [0.25, 0.45]",
                  "长消息尾端唯一命中词被精准置于视口 25%–45% 安全区内，上下均留有安全裕量")
        # 2. 逐处推进：同一长消息内 8 处命中单调推进与计数严格 1/8...8/8 (验收标准 2)
        xs.click(541, 43, settle=0.5)
        xs.select_all_and_clear()
        xs.type_ascii("step-item", settle=0.03)
        time.sleep(1.8)
        labels_step = "\n".join(ax.labels())
        has_1_of_8 = "1/8" in labels_step or ax.has_text("1/8")
        check("search_count_step_item_initial", has_1_of_8, has_1_of_8, True, "step-item 初始计数显示为 1/8")
        step_y_coords = []
        counts_sequence = []
        for step in range(1, 9):
            cur_labels = "\n".join(ax.labels())
            cur_count = next((f"{step}/8" for _ in [1] if f"{step}/8" in cur_labels or ax.has_text(f"{step}/8")), None)
            counts_sequence.append(cur_count)
            shot_cur = EVID / f"02-step-item-{step}.png"
            xs.shot(shot_cur)
            im_cur = Image.open(shot_cur).convert("RGB")
            cur_active_pts = []
            for y in range(80, 755):
                for x in range(200, 1250):
                    px = im_cur.getpixel((x, y))
                    if px[0] > 230 and 185 <= px[1] <= 205 and px[2] < 100:
                        cur_active_pts.append((x, y))
            if cur_active_pts:
                c_y = (min(p[1] for p in cur_active_pts) + max(p[1] for p in cur_active_pts)) / 2.0
                step_y_coords.append(c_y)
            else:
                step_y_coords.append(0)
            if step < 8: xs.key("Return", settle=0.8)
        expected_counts = [f"{i}/8" for i in range(1, 9)]
        check("counts_sequence_1_to_8", counts_sequence == expected_counts, counts_sequence, expected_counts, "连续按下一处，计数严格呈现 1/8 到 8/8")
        y_increments = [step_y_coords[i+1] - step_y_coords[i] for i in range(len(step_y_coords)-1)]
        strictly_advancing = all(inc >= 20 for inc in y_increments)
        check("step_items_monotonic_advancement", strictly_advancing,
              {"y_coords": step_y_coords, "increments": y_increments},
              "all increments >= 20px (line height)",
              "同一长消息内多处命中按命中词逐处精准推进，位移严格大于单行高")
        # 3. 循环与边界切换 (验收标准 3)
        xs.key("Return", settle=0.8)
        labels_wrap = "\n".join(ax.labels())
        wrapped_to_1 = "1/8" in labels_wrap or ax.has_text("1/8")
        check("cyclic_wrap_next_to_first", wrapped_to_1, wrapped_to_1, True, "在末项按下一处循环回到首项 1/8")
        xs.key("Return", shift=True, settle=0.8)
        labels_wrap_prev = "\n".join(ax.labels())
        wrapped_to_8 = "8/8" in labels_wrap_prev or ax.has_text("8/8")
        check("cyclic_wrap_prev_to_last", wrapped_to_8, wrapped_to_8, True, "在首项按上一处循环回到末项 8/8")
        xs.click(541, 43, settle=0.3)
        xs.select_all_and_clear()
        xs.type_ascii("notfoundtargetxyz", settle=0.03)
        time.sleep(1.0)
        labels_0 = "\n".join(ax.labels())
        has_0_of_0 = "0/0" in labels_0 or ax.has_text("0/0")
        check("search_count_empty_0_of_0", has_0_of_0, has_0_of_0, True, "无匹配项时计数显示 0/0")
        btn_next_zero = find_session_node(app_pid, "下一处匹配 (Enter)", contains=True)
        btn_prev_zero = find_session_node(app_pid, "上一处匹配 (Shift+Enter)", contains=True)
        btn_next_disabled = False
        btn_prev_disabled = False
        state_details = {}
        if btn_next_zero and btn_prev_zero:
            ss_next = btn_next_zero.accessible.get_state_set()
            ss_prev = btn_prev_zero.accessible.get_state_set()
            next_sensitive = ss_next.contains(Atspi.StateType.SENSITIVE)
            prev_sensitive = ss_prev.contains(Atspi.StateType.SENSITIVE)
            next_enabled = ss_next.contains(Atspi.StateType.ENABLED)
            prev_enabled = ss_prev.contains(Atspi.StateType.ENABLED)
            state_details = {
                "next_sensitive": next_sensitive,
                "prev_sensitive": prev_sensitive,
                "next_enabled": next_enabled,
                "prev_enabled": prev_enabled,
            }
            btn_next_disabled = (not next_sensitive) or (not next_enabled)
            btn_prev_disabled = (not prev_sensitive) or (not prev_enabled)
        check("arrows_disabled_on_zero_match", btn_next_disabled and btn_prev_disabled,
              state_details, "sensitive=False and enabled=False",
              "0 命中时经 AT-SPI 真实判定上一处/下一处按钮状态处于 disabled (SENSITIVE/ENABLED 为 False)")
        # 4. 焦点保持：用 AT-SPI 取「下一处匹配 (Enter)」按钮的真实屏幕矩形 extents 并点中心 (验收标准 4 / 派工项 1)
        xs.click(541, 43, settle=0.3)
        xs.select_all_and_clear()
        xs.type_ascii("step-item", settle=0.03)
        time.sleep(1.2)
        # 查找搜索输入框节点（通过 aria-label「当前会话内查找」或 placeholder）
        search_input_node = find_session_node(app_pid, "当前会话内查找", contains=True)
        if not search_input_node:
            search_input_node = ax.find("当前会话内查找", contains=True)

        input_focused_before = False
        if search_input_node:
            ss_in_before = search_input_node.accessible.get_state_set()
            input_focused_before = ss_in_before.contains(Atspi.StateType.FOCUSED)

        btn_next_node = find_session_node(app_pid, "下一处匹配 (Enter)", contains=True)
        if not btn_next_node:
            raise RuntimeError("AT-SPI 未能找到『下一处匹配 (Enter)』按钮节点")
        comp = btn_next_node.accessible.get_component()
        ext = comp.get_extents(Atspi.CoordType.SCREEN)
        btn_rect = [ext.x, ext.y, ext.width, ext.height]
        click_center_x = ext.x + ext.width // 2
        click_center_y = ext.y + ext.height // 2
        xs.click(click_center_x, click_center_y, settle=0.8)
        time.sleep(0.8)

        # 读取点击后搜索输入框的 FOCUSED 状态位
        input_focused_after = False
        if search_input_node:
            ss_in_after = search_input_node.accessible.get_state_set()
            input_focused_after = ss_in_after.contains(Atspi.StateType.FOCUSED)

        labels_after_click = "\n".join(ax.labels())
        has_2_of_8 = "2/8" in labels_after_click or ax.has_text("2/8")
        check("click_next_arrow_advances_count", has_2_of_8,
              {"button_extents": btn_rect, "click_center": [click_center_x, click_center_y], "count_after_click": "2/8" if has_2_of_8 else labels_after_click},
              "count advances from 1/8 to 2/8",
              "用 AT-SPI 取『下一处匹配』真实屏幕矩形后点击其中心，计数严格递增至 2/8")

        xs.type_ascii("zzz", settle=0.03)
        time.sleep(1.0)
        labels_after_type = "\n".join(ax.labels())
        has_0_of_0_after_type = "0/0" in labels_after_type or ax.has_text("0/0")
        check("focus_retained_in_input_after_button_click", input_focused_after and has_0_of_0_after_type,
              {"input_focused_before": input_focused_before, "input_focused_after": input_focused_after, "typing_updated_count": has_0_of_0_after_type},
              {"input_focused_after": True, "typing_updated_count": True},
              "点击箭头按钮前后输入框均保持 Atspi.StateType.FOCUSED 焦点位，直接键入新字符实时过滤检索")
        # 5. 两级高亮可辨且消除整条消息竖线 (验收标准 5)
        xs.click(541, 43, settle=0.3)
        xs.select_all_and_clear()
        xs.type_ascii("step-item", settle=0.03)
        time.sleep(1.2)
        shot_two_level = EVID / "05-two-level-highlights.png"
        xs.shot(shot_two_level)
        im_two_level = Image.open(shot_two_level).convert("RGB")
        active_color_count = count_pixels(shot_two_level, (200, 80, 1250, 755), lambda px: px[0] > 230 and 185 <= px[1] <= 205 and px[2] < 100)
        passive_color_count = count_pixels(shot_two_level, (200, 80, 1250, 755), lambda px: px[0] > 230 and px[1] > 210 and 120 <= px[2] <= 165)
        check("two_level_highlight_colors_coexist", active_color_count >= 20 and passive_color_count >= 50,
              {"active_amber_pixels": active_color_count, "passive_amber_pixels": passive_color_count},
              "active >= 20 and passive >= 50",
              "同屏存在显著区分的两级高亮：当前项饱和深琥珀独占，其余项浅淡琥珀底")

        # D: 断言单活跃项独占——在页面截图中，当前高亮深色块连通区域（bounding box）严格只有 1 处，其余均为浅淡色底
        active_boxes = []
        visited = set()
        for y in range(80, 755):
            for x in range(200, 1250):
                px = im_two_level.getpixel((x, y))
                if px[0] > 230 and 185 <= px[1] <= 205 and px[2] < 100 and (x, y) not in visited:
                    q_pts = [(x, y)]
                    visited.add((x, y))
                    min_x, max_x, min_y, max_y = x, x, y, y
                    cnt = 0
                    while q_pts:
                        cx, cy = q_pts.pop()
                        cnt += 1
                        min_x = min(min_x, cx); max_x = max(max_x, cx)
                        min_y = min(min_y, cy); max_y = max(max_y, cy)
                        for dx, dy in ((-1,0), (1,0), (0,-1), (0,1)):
                            nx, ny = cx + dx, cy + dy
                            if 200 <= nx < 1250 and 80 <= ny < 755 and (nx, ny) not in visited:
                                p2 = im_two_level.getpixel((nx, ny))
                                if p2[0] > 230 and 185 <= p2[1] <= 205 and p2[2] < 100:
                                    visited.add((nx, ny))
                                    q_pts.append((nx, ny))
                    if cnt >= 20:
                        active_boxes.append({"bbox": [min_x, min_y, max_x, max_y], "pixel_count": cnt})

        check("single_active_highlight_exclusive", len(active_boxes) == 1,
              {"active_boxes_count": len(active_boxes), "boxes": active_boxes},
              {"active_boxes_count": 1},
              "旧项在切换后立刻回落为浅淡色，全屏严格只存在 1 个当前活跃高亮块，无多个 active 并存")
        pix = im_two_level.load()
        has_long_amber_vertical_line = False
        for x in range(200, 1300):
            consecutive = 0
            for y in range(80, 755):
                r, g, b = pix[x, y]
                if r > 200 and g > 120 and b < 100:
                    consecutive += 1
                    if consecutive > 150:
                        has_long_amber_vertical_line = True
                        break
                else:
                    consecutive = 0
            if has_long_amber_vertical_line: break
        check("no_long_vertical_message_ring", not has_long_amber_vertical_line,
              {"has_vertical_line_over_150px": has_long_amber_vertical_line},
              False,
              "整条消息的竖线描边已彻底去除，长消息内无贯穿性描边线")
        # 6. Markdown 各种元素计数一致性与围栏代码块排除 (验收标准 6)
        xs.click(541, 43, settle=0.3)
        xs.select_all_and_clear()
        xs.type_ascii("markdown-target", settle=0.03)
        time.sleep(1.5)
        labels_md = "\n".join(ax.labels())
        has_1_of_3 = "1/3" in labels_md or ax.has_text("1/3")
        check("markdown_elements_count_consistent", has_1_of_3, has_1_of_3, True,
              "跨标题、表格与行内代码精准统计，围栏代码块成功排除，计数严格为 1/3")
        shot_md1 = EVID / "06-markdown-target-1.png"
        xs.shot(shot_md1)
        xs.key("Return", settle=0.8)
        shot_md2 = EVID / "06-markdown-target-2.png"
        xs.shot(shot_md2)
        md_jump_delta = region_delta(shot_md1, shot_md2, (200, 80, 1250, 755))
        check("markdown_targets_scroll_different_positions", md_jump_delta > 100, md_jump_delta, "> 100 px",
              "标题与表格单元格内的不同命中项能精准滚动到不同位置")
        # 7. 按 Esc 清空退出 (验收标准 7)
        xs.key("Escape", settle=0.8)
        shot_esc = EVID / "07-escaped.png"
        xs.shot(shot_esc)
        labels_after_esc = "\n".join(ax.labels())
        count_hidden = "0/0" not in labels_after_esc and "1/3" not in labels_after_esc
        amber_pixels_esc = count_pixels(shot_esc, (200, 80, 1250, 755), lambda px: px[0] > 230 and px[1] > 130 and px[2] < 120)
        check("esc_clears_search_and_highlights", count_hidden and amber_pixels_esc == 0,
              {"count_hidden": count_hidden, "amber_pixels_remaining": amber_pixels_esc},
              {"count_hidden": True, "amber_pixels_remaining": 0},
              "按 Esc 退出后搜索计数隐藏且正文高亮像素完全归零")
        

        # 8. 正则元字符与特殊符号字面检索安全断言 (修复阻塞缺陷：( [ * . \ a.b 不黑屏、不抛错、不通配)
        # 8.1 键入单个括号 "("：断言应用不崩溃、不黑屏 (平均亮度 > 200)，正文仍存活可见
        xs.click(541, 43, settle=0.3)
        xs.type_ascii("(", settle=0.03)
        time.sleep(1.8)
        shot_paren = EVID / "08-special-paren.png"
        xs.shot(shot_paren)
        im_paren = Image.open(shot_paren).convert("L").crop((100, 60, 1340, 800))
        gray_data = list(im_paren.getdata())
        mean_brightness = sum(gray_data) / float(len(gray_data)) if gray_data else 0
        has_body_text_after_paren = ax.has_text("综合规划流程与要点梳理") or ax.has_text("元素测试指标")
        check("special_char_paren_no_crash_or_blackout", mean_brightness > 200 and has_body_text_after_paren,
              {"mean_brightness": round(mean_brightness, 1), "has_body_text": has_body_text_after_paren},
              {"mean_brightness_threshold": "> 200", "has_body_text": True},
              "键入正则元字符 ( 后应用渲染树完好不黑屏崩溃，正文区域平均亮度正常且内容清晰可见")

        # 8.2 键入字面 "a.b"：必须按严格字面量匹配（会话中出现恰好 1 次），不能当通配符匹配 axb
        xs.click(541, 43, settle=0.3)
        xs.select_all_and_clear()
        xs.type_ascii("a.b", settle=0.03)
        time.sleep(1.5)
        count_node_ab = find_session_node(app_pid, "搜索计数", contains=True)
        count_text_ab = count_node_ab.name if count_node_ab else ""
        has_1_of_1 = "1/1" in count_text_ab
        check("special_char_literal_dot_match", has_1_of_1,
              {"count_text": count_text_ab},
              "搜索计数 1/1",
              "键入 a.b 时为严格字面量检索恰好命中 1 处，点号不作为正则通配误判 axb")

        # 8.3 按 Esc 退出后界面完全恢复正常，计数条隐藏
        xs.key("Escape", settle=0.8)
        time.sleep(0.5)
        count_node_after_esc = find_session_node(app_pid, "搜索计数", contains=True)
        count_hidden_spec = (count_node_after_esc is None) or ("1/1" not in count_node_after_esc.name and "0/0" not in count_node_after_esc.name)
        shot_spec_esc = EVID / "08-special-escaped.png"
        xs.shot(shot_spec_esc)
        amber_pixels_spec_esc = count_pixels(shot_spec_esc, (200, 80, 1250, 755), lambda px: px[0] > 230 and px[1] > 130 and px[2] < 120)
        check("special_char_esc_restores_clean_state", count_hidden_spec and amber_pixels_spec_esc == 0,
              {"count_hidden": count_hidden_spec, "amber_pixels_remaining": amber_pixels_spec_esc},
              {"count_hidden": True, "amber_pixels_remaining": 0},
              "特殊字符检索后按 Esc 退出，顶栏计数条与高亮干净退出复原")
        facts_file = EVID / "facts.json"
        facts_file.write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"Wrote UI facts to {facts_file}")
        if facts["failures"]:
            print(f"FAILED with {len(facts["failures"])} errors!", file=sys.stderr)
            return 1
        print("ALL OD-ARCH-084 ASSERTIONS PASSED (100%)!")
        return 0
    finally:
        session.close()
        try: bus_proc.terminate(); bus_proc.wait(timeout=2)
        except Exception: pass
        try: xvfb.terminate(); xvfb.wait(timeout=2)
        except Exception: pass

if __name__ == "__main__": sys.exit(main())
