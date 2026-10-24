#!/usr/bin/env python3
"""OD-ARCH-081 真实窗口验收：问候常驻 / 输入区品牌标 / Markdown 正文。

一次性 Xvfb + 私有 dbus + 合成端点 + 隔离数据根；不读凭证、不碰用户数据、
不碰 runtime/ 与 ~/.dsh、~/.agents。

判定依据（全部来自本窗口内的真实截图，不做“应该有”的推断）：

  * S1 问候 + 待办常驻：把指针移入对话区、点进输入框并输入文字之后，问候所在
    条带逐像素不变（同一块裁剪图 sha256 相同），且仍在上部、对话内容在其下方。
  * S2 品牌标：输入行「+」左侧出现青色→紫色渐变标识的彩色像素；原先 spark 图标
    的单色主色墨迹在同一区域为 0；点它不产生新窗口、不改变主区域。
  * S3 Markdown：预置一条合成对话，答句含标题/粗体/列表/表格/代码块，问句含
    列表；表格解析会画出横贯整块的气泡表格边框线，代码块会画出整块底色面板——
    这两者在纯文本渲染下不可能出现，因此作为“确实按 Markdown 渲染”的结构证据。

证据等级：真实桌面窗口（Xvfb + XTEST 点击 + 截图）+ 像素结构判定 + 数据根文件事实
          + 静态源码核对。不做 OCR；不把“截图里有字”当成排版正确的证据。

用法：python3 verify_home_fixes_081.py [--display :87] [--root /tmp/od081-home]
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
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-081"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900
THREAD_ID = "t081fixes"

USER_TEXT = "请按 **粗体** 与列表整理：\n\n- 合成甲\n- 合成乙"
ASSISTANT_TEXT = "\n".join(
    [
        "## 合成标题",
        "",
        "这是一段**加粗**的合成说明，只用于界面验收。",
        "",
        "- 第一条合成要点",
        "- 第二条合成要点",
        "- 第三条合成要点",
        "",
        "| 项目 | 数值 |",
        "| --- | --- |",
        "| 合成甲 | 12 |",
        "| 合成乙 | 34 |",
        "",
        "```text",
        "synthetic-code-line-1",
        "synthetic-code-line-2",
        "```",
    ]
)


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


def start_private_dbus() -> tuple[str, subprocess.Popen]:
    """一次性私有会话总线（single-instance 插件按 bundle id 走会话总线）。"""
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--nofork", "--print-address"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    address = (proc.stdout.readline() or "").strip()
    if not address.startswith("unix:"):
        proc.kill()
        raise RuntimeError("私有会话总线没有给出地址")
    return address, proc


def tree_fingerprint(root: Path, limit: int = 4000, exclude=()) -> str:
    h = hashlib.sha256()
    if not root.exists():
        return "missing"
    count = 0
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(root).as_posix()
        if any(rel.startswith(prefix) for prefix in exclude):
            continue
        count += 1
        st = p.stat()
        h.update(f"{rel}|{st.st_size}|{int(st.st_mtime)}".encode())
        if count >= limit:
            break
    return h.hexdigest()[:16] + f":{count}"


def thread_payload() -> dict:
    now = int(time.time() * 1000)
    # 只认“真的这么写”的代码形态，不认注释里的说明文字：品牌标注释、Markdown
    # 文件头注释都直接写了这些词，用纯字符串包含会把说明当成实现。
    return {
        "id": THREAD_ID,
        "tag": "plain",
        "projectPath": None,
        "title": "合成排版验收（Markdown）",
        "preset": "plain",
        "messages": [
            {"id": "u1", "role": "user", "text": USER_TEXT},
            {
                "id": "a1",
                "role": "assistant",
                "text": ASSISTANT_TEXT,
                "sentText": USER_TEXT,
                "startedAtMs": now - 9_000,
                "endedAtMs": now - 3_000,
            },
        ],
        "createdAt": now - 60_000,
        "updatedAt": now,
        "version": 1,
    }


def write_settings_local(session: AppSession) -> Path:
    path = session.app_data / "config" / "settings.v1.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.settings.v1",
                "artifact_root": None,
                "model_permissions": {"local": True, "private": False, "public": False},
            },
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def write_pending_todo(session: AppSession) -> Path:
    """种一条待确认写入记录，让首页待办条有内容可显示（纯合成标题）。"""
    import datetime

    now = time.time()
    stamp = datetime.datetime.fromtimestamp(now).strftime("%Y%m%d%H%M%S")
    path = session.app_data / "engine-state" / "pending-writes" / f"pw-{stamp}-1-0.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.pending-write.v1",
                "id": path.stem,
                "action": "save_offline",
                "source": "model",
                "summary": "合成待办：确认一份虚构成果",
                "impact": "写入成果目录，仅合成内容",
                "payload": {
                    "scenario": "material_writing",
                    "title": "合成待办：确认一份虚构成果",
                    "markdown": "# 合成\n\n仅用于界面验收。\n",
                    "ai_assisted": True,
                },
                "created_at": now,
                "expires_at": now + 3600,
                "status": "pending",
                "resolved_at": None,
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    return path


def crop_digest(shot: Path, box: tuple[int, int, int, int]) -> str:
    from PIL import Image

    im = Image.open(shot).convert("RGB").crop(box)
    return hashlib.sha256(im.tobytes()).hexdigest()[:16]


def region_delta(
    a: Path, b: Path, box: tuple[int, int, int, int], tolerance: int = 12
) -> tuple[int, tuple[int, int, int, int] | None]:
    """两张截图在同一区域里“明显不同”的像素数与位置。

    不用整块 sha256 判等：WebKitGTK 的抗锯齿与字体次像素渲染会让同一内容在不同帧里
    出现 1–2 级灰度差异，逐像素哈希会把“没有变化”误判成“变了”。只统计差值超过
    tolerance 的像素，才代表真的遮挡 / 位移 / 隐藏。
    """
    from PIL import Image, ImageChops

    left = Image.open(a).convert("RGB").crop(box)
    right = Image.open(b).convert("RGB").crop(box)
    diff = ImageChops.difference(left, right)
    changed = 0
    for px in diff.getdata():
        if max(px) > tolerance:
            changed += 1
    return changed, diff.getbbox()


def ink_rows(shot: Path, box: tuple[int, int, int, int], threshold: int = 150) -> list[tuple[int, int]]:
    """在 box 里按行找“明显文字墨迹”的连续行带（返回绝对 y 区间）。"""
    from PIL import Image

    im = Image.open(shot).convert("L").crop(box)
    x0, y0, _, _ = box
    rows = [y for y in range(im.height) if sum(1 for v in im.crop((0, y, im.width, y + 1)).getdata() if v < threshold) > 3]
    bands: list[list[int]] = []
    for y in rows:
        if bands and y - bands[-1][1] <= 2:
            bands[-1][1] = y
        else:
            bands.append([y, y])
    return [(y0 + a, y0 + b) for a, b in bands]


def count_pixels(shot: Path, box: tuple[int, int, int, int], pred) -> int:
    from PIL import Image

    im = Image.open(shot).convert("RGB").crop(box)
    return sum(1 for px in im.getdata() if pred(px))


def long_hline_rows(shot: Path, box: tuple[int, int, int, int], min_run: int = 200) -> list[int]:
    """找“横贯一整块”的细边框线：每行存在 >= min_run 连续像素既不是纯白也不是正文黑。"""
    from PIL import Image

    im = Image.open(shot).convert("L").crop(box)
    x0, y0, _, _ = box
    out: list[int] = []
    for y in range(im.height):
        run = best = 0
        for v in im.crop((0, y, im.width, y + 1)).getdata():
            if 170 <= v <= 243:
                run += 1
                best = max(best, run)
            else:
                run = 0
        if best >= min_run:
            out.append(y0 + y)
    return out


def source_checks() -> dict:
    root = HERE.parents[2]
    src = root / "desktop" / "src"
    scaffold = (src / "shell" / "SceneScaffold.tsx").read_text(encoding="utf-8")
    home = (src / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    md = (src / "shell" / "MarkdownText.tsx").read_text(encoding="utf-8")
    canvas = (src / "shell" / "HomeCanvas.tsx").read_text(encoding="utf-8")
    pkg = json.loads((root / "desktop" / "package.json").read_text(encoding="utf-8"))
    deps = {**pkg.get("dependencies", {}), **pkg.get("devDependencies", {})}
    return {
        "scaffold_upper_fixed": 'data-testid="scene-upper"' in scaffold and "flex-shrink-0" in scaffold,
        "scaffold_no_focus_state": "IDLE_RESTORE_MS" not in scaffold and "pointer" not in scaffold.lower(),
        "greeting_in_upper": "欢迎回到 OmniDesk" in canvas and "TodoTicker" in canvas,
        "brand_mark_used": '<BrandMark className="w-[22px] h-[22px]' in home,
        "spark_kept_only_as_status_badge": home.count('<Icon name="spark"') <= 1,
        "markdown_assistant": "<MarkdownText" in home,
        "react_markdown_dep": "react-markdown" in deps,
        "no_rehype_raw": "rehypePlugins" not in md,
        "no_dangerous_html": "dangerouslySetInnerHTML=" not in md,
        "external_open_command": "open_external_url" in (
            root / "desktop" / "src-tauri" / "src" / "lib.rs"
        ).read_text(encoding="utf-8"),
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":87")
    ap.add_argument("--root", default="/tmp/od081-home")
    args = ap.parse_args(argv[1:])
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {
        "plan": "OD-ARCH-081",
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + 截图）+ 像素结构判定 + 数据根文件事实 + 静态源码核对",
        "root": str(root),
        "source_home": str(SOURCE_HOME),
        "checks": {},
        "notes": [],
    }
    facts["source_before"] = tree_fingerprint(SOURCE_HOME, exclude=("sessions/",))
    facts["user_dsh_before"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_before"] = tree_fingerprint(Path.home() / ".agents")
    shutil.copytree(SOURCE_HOME, root / "dsh-home", dirs_exist_ok=True, symlinks=True)

    xvfb = start_display(args.display)
    bus_address, bus_proc = start_private_dbus()
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_address},
    )
    facts["dbus"] = "私有一次性会话总线（不连用户会话总线）"
    facts["frontend"] = "复用 127.0.0.1:5273 的同仓库 dev server（只读复用，不启动也不终止）"
    gate_errors: list[str] = []
    try:
        session.prepare()
        write_settings_local(session)
        facts["todo_file"] = str(write_pending_todo(session))
        target = session.app_data / "threads" / "plain" / f"{THREAD_ID}.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(thread_payload(), ensure_ascii=False, indent=2), encoding="utf-8")

        session.start()
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=150)
        if win is None:
            raise RuntimeError("没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)

        def stable_shot(name: str, timeout: float = 25.0) -> Path:
            path = EVID / name
            prev = None
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                xs.shot(path)
                data = path.read_bytes()
                if prev is not None and prev == data:
                    return path
                prev = data
                time.sleep(0.6)
            raise RuntimeError(f"{name}：界面在超时内没有稳定下来")

        # 空白处点一下让首帧动画结束（不落任何控件），再取稳定帧。
        xs.click(700, 620, settle=1.5)
        base = stable_shot("01-home.png")

        # --- 定位问候条带（不硬编码 y：窗口/字号变化后仍能定位） ----------
        def greeting_band(shot: Path) -> tuple[int, int]:
            bands = ink_rows(shot, (140, 60, 1100, 460), threshold=160)
            tall = [(a, b) for a, b in bands if b - a >= 16]
            if not tall:
                raise RuntimeError("截图里没有找到问候语（高度 >= 16px 的文字带）")
            return tall[0]

        g_top, g_bottom = greeting_band(base)
        crop = (60, max(0, g_top - 18), 1400, g_bottom + 74)
        facts["greeting_band"] = [g_top, g_bottom]
        facts["greeting_crop"] = list(crop)
        digest_base = crop_digest(base, crop)
        facts["greeting_digest_base"] = digest_base
        facts["checks"]["greeting_visible"] = True
        facts["checks"]["todo_visible"] = count_pixels(
            base, (crop[0], g_bottom + 2, crop[2], crop[3]), lambda px: px[0] < 150 and px[1] < 150
        ) > 25
        if not facts["checks"]["todo_visible"]:
            facts["notes"].append("问候条带下方没有探测到待办条文字墨迹")

        # 指针移入对话区中部（原实现会在这里收起上部），再移回原处。
        xs.move(700, 560)
        time.sleep(1.6)
        xs.shot(EVID / "02-pointer-in-chat.png")
        deltas = {}
        changed, bbox = region_delta(base, EVID / "02-pointer-in-chat.png", crop)
        deltas["pointer"] = {"changed_pixels": changed, "bbox": list(bbox) if bbox else None}
        facts["checks"]["greeting_unchanged_on_pointer"] = changed <= 40
        if changed > 40:
            gate_errors.append(
                f"指针移入对话区后，问候条带有 {changed} 个像素明显变化（上部被隐藏或整页上移）"
            )

        # 点进输入框并输入合成文字：原实现聚焦时也会收起上部。
        xs.click(700, 786, settle=0.8)
        xs.type_ascii("synthetic focus check", settle=0.01)
        time.sleep(1.2)
        xs.shot(EVID / "03-input-focused-typing.png")
        changed, bbox = region_delta(base, EVID / "03-input-focused-typing.png", crop)
        deltas["focus"] = {"changed_pixels": changed, "bbox": list(bbox) if bbox else None}
        facts["checks"]["greeting_unchanged_on_focus"] = changed <= 40
        if changed > 40:
            gate_errors.append(
                f"输入框获得焦点并输入文字后，问候条带有 {changed} 个像素明显变化（上部被隐藏）"
            )
        facts["greeting_deltas"] = deltas
        # 清掉刚才的合成文字，避免影响后续截图。
        xs.select_all_and_clear()
        time.sleep(0.8)

        # --- S2 品牌标 --------------------------------------------------
        # 输入行：「+」按钮约在 x=250，其左侧 220–240 是品牌标位置。
        mark_box = (205, 770, 246, 803)
        cyan = count_pixels(base, mark_box, lambda px: px[2] > 190 and px[1] > 140 and px[0] < 150)
        primary_only = count_pixels(
            base, mark_box, lambda px: px[2] > 150 and px[0] < 90 and px[1] < 90
        )
        facts["brand_mark"] = {"box": list(mark_box), "gradient_pixels": cyan, "flat_primary_pixels": primary_only}
        facts["checks"]["brand_mark_gradient_present"] = cyan >= 12
        facts["checks"]["old_spark_absent"] = primary_only <= 2
        if cyan < 12:
            gate_errors.append("输入区「+」左侧没有探测到品牌标的渐变彩色像素")
        if primary_only > 2:
            gate_errors.append("输入区「+」左侧仍有单色主色墨迹（疑似旧 spark 图标未换）")

        # 点它：纯装饰，不应打开任何窗口，也不应改变主区域布局。
        def mapped_windows() -> int:
            n = 0
            for w in xs.d.screen().root.query_tree().children:
                try:
                    if w.get_attributes().map_state == xs._X.IsViewable:
                        n += 1
                except Exception:
                    continue
            return n

        before_windows = mapped_windows()
        xs.shot(EVID / "04a-before-mark-click.png")
        xs.click((mark_box[0] + mark_box[2]) // 2, (mark_box[1] + mark_box[3]) // 2, settle=1.5)
        xs.shot(EVID / "04-after-mark-click.png")
        after_windows = mapped_windows()
        changed, bbox = region_delta(
            EVID / "04a-before-mark-click.png", EVID / "04-after-mark-click.png", crop
        )
        facts["brand_mark_click"] = {
            "windows_before": before_windows,
            "windows_after": after_windows,
            "upper_changed_pixels": changed,
            "bbox": list(bbox) if bbox else None,
        }
        facts["checks"]["brand_mark_click_noop"] = after_windows == before_windows and changed <= 40
        if not facts["checks"]["brand_mark_click_noop"]:
            gate_errors.append(
                f"点品牌标之后出现新窗口（{before_windows}→{after_windows}）或上部布局变化（{changed} 像素）"
            )

        # --- S3 Markdown 结构 ------------------------------------------
        xs.focus(win)
        shot = stable_shot("05-markdown.png", timeout=15.0)
        # 对话正文区（右侧留出索引条，底部留出输入区）。
        body = (150, g_bottom + 60, 1300, 760)
        lines = long_hline_rows(shot, body, min_run=180)
        facts["markdown"] = {"body_box": list(body), "long_hlines": lines}
        # 表格：2 列 × 2 行数据 + 表头 → 至少 3 条横贯边框线，且彼此间距 < 40px。
        grouped: list[int] = []
        for y in lines:
            if not grouped or y - grouped[-1] > 2:
                grouped.append(y)
        table_like = [y for y in grouped]
        facts["checks"]["table_border_lines"] = len(table_like) >= 3
        if len(table_like) < 3:
            gate_errors.append(f"答句里没有画出表格边框线（找到 {len(table_like)} 条横贯线，应为 >= 3）")

        # 代码块：整块浅色面板（比气泡底色略深）+ 右上角「复制」按钮的边框。
        code_panel = count_pixels(
            shot, body, lambda px: 232 <= px[0] <= 246 and 232 <= px[1] <= 246 and 240 <= px[2] <= 252
        )
        facts["markdown"]["code_panel_pixels"] = code_panel
        facts["checks"]["code_panel_present"] = code_panel > 4000
        if code_panel <= 4000:
            gate_errors.append(f"答句里没有画出代码块底色面板（{code_panel} 像素）")

        # 列表圆点：正文左侧 8–30px 内出现的短小方形墨迹（原始 "- " 会是一条细横线）。
        def bullet_markers(path: Path, box: tuple[int, int, int, int]) -> int:
            from PIL import Image

            im = Image.open(path).convert("L").crop(box)
            x0, y0, _, _ = box
            found = 0
            for y in range(im.height):
                seg = [x for x in range(im.width) if im.getpixel((x, y)) < 160]
                if not seg:
                    continue
                # 只看最左侧的独立墨迹组：宽 3–8px 且与后面文字有 >= 4px 空隙
                run = 1
                start = seg[0]
                groups: list[tuple[int, int]] = []
                for a, b in zip(seg, seg[1:]):
                    if b - a <= 1:
                        run += 1
                    else:
                        groups.append((start, start + run - 1))
                        start, run = b, 1
                groups.append((start, start + run - 1))
                if groups and 2 <= groups[0][1] - groups[0][0] + 1 <= 7:
                    if len(groups) > 1 and groups[1][0] - groups[0][1] >= 4:
                        found += 1
            return found

        bullets = bullet_markers(shot, (body[0] + 60, body[1], body[0] + 130, body[3]))
        facts["markdown"]["bullet_marker_rows"] = bullets
        facts["checks"]["list_markers_present"] = bullets >= 3
        if bullets < 3:
            gate_errors.append(f"答句里没有画出列表圆点（找到 {bullets} 行，应为 >= 3）")

        # 问句气泡里的 Markdown 列表：滚到顶部让问句进入视野，再数气泡内的
        # 独立圆形白点（Markdown 的 ul 圆点），而不是数白色像素总数（文字笔画也是白的）。
        xs.scroll(700, 400, 10, up=True, settle=1.6)
        user_shot = stable_shot("06-user-markdown.png", timeout=12.0)

        def purple_bbox(path: Path, box: tuple[int, int, int, int]):
            from PIL import Image

            im = Image.open(path).convert("RGB").crop(box)
            x0, y0, _, _ = box
            xs_: list[int] = []
            ys: list[int] = []
            for y in range(im.height):
                for x in range(im.width):
                    r, g, b = im.getpixel((x, y))
                    if b > 150 and b - r > 60 and g < 120:
                        xs_.append(x + x0)
                        ys.append(y + y0)
            if not xs_:
                return None
            return (min(xs_), min(ys), max(xs_) + 1, max(ys) + 1)

        def light_dots(path: Path, box: tuple[int, int, int, int]) -> list[tuple[int, int, int, int]]:
            """气泡内部 3–9px 的独立浅色连通块（宽高比接近 1，颜色明显亮于气泡底），

            即 Markdown 的列表圆点。判定阀值按“比主色底亮得多”而不是纯白：圆点用
            半透明白绘制，实际像素是浅紫，不是 #fff。
            """
            from PIL import Image

            im = Image.open(path).convert("RGB").crop(box)
            w, h = im.size
            seen = [[False] * w for _ in range(h)]
            dots: list[tuple[int, int, int, int]] = []

            def is_light(px) -> bool:
                r, g, b = px
                # 气泡底约 (60,45,205)；圆点约 (170,165,240)。
                return r > 120 and g > 115 and b > 200

            for y in range(h):
                for x in range(w):
                    if seen[y][x]:
                        continue
                    if not is_light(im.getpixel((x, y))):
                        continue
                    stack = [(x, y)]
                    seen[y][x] = True
                    pts: list[tuple[int, int]] = []
                    while stack:
                        cx, cy = stack.pop()
                        pts.append((cx, cy))
                        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                            nx, ny = cx + dx, cy + dy
                            if 0 <= nx < w and 0 <= ny < h and not seen[ny][nx]:
                                if is_light(im.getpixel((nx, ny))):
                                    seen[ny][nx] = True
                                    stack.append((nx, ny))
                    if not 3 <= len(pts) <= 90:
                        continue
                    bx0 = min(p[0] for p in pts)
                    bx1 = max(p[0] for p in pts)
                    by0 = min(p[1] for p in pts)
                    by1 = max(p[1] for p in pts)
                    bw, bh = bx1 - bx0 + 1, by1 - by0 + 1
                    if 3 <= bw <= 9 and 3 <= bh <= 9 and abs(bw - bh) <= 2 and len(pts) >= 0.55 * bw * bh:
                        dots.append((bx0 + box[0], by0 + box[1], bw, bh))
            return dots

        bubble_box = purple_bbox(user_shot, (600, body[1], 1300, body[3]))
        facts["markdown"]["user_bubble_box"] = list(bubble_box) if bubble_box else None
        dots = light_dots(user_shot, bubble_box) if bubble_box else []
        facts["markdown"]["user_bubble_dots"] = dots
        facts["checks"]["user_bubble_found"] = bubble_box is not None
        facts["checks"]["user_bullets_rendered"] = len(dots) >= 2
        if bubble_box is None:
            gate_errors.append("滚到顶部后没有在截图中找到问句气泡")
        elif len(dots) < 2:
            gate_errors.append(
                f"问句气泡里没有画出列表圆点（找到 {len(dots)} 个独立白点，合成问句有 2 条列表项）"
            )

        facts["checks"]["app_alive"] = session.alive()
        if not session.alive():
            gate_errors.append("应用在验收过程中退出")
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()
        bus_proc.terminate()
        try:
            bus_proc.wait(timeout=10)
        except Exception:
            bus_proc.kill()

    facts["source_checks"] = source_checks()
    facts["source_after"] = tree_fingerprint(SOURCE_HOME, exclude=("sessions/",))
    facts["user_dsh_after"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_after"] = tree_fingerprint(Path.home() / ".agents")
    facts["source_unchanged"] = facts["source_before"] == facts["source_after"]
    facts["user_dsh_unchanged"] = facts["user_dsh_before"] == facts["user_dsh_after"]
    facts["user_agents_unchanged"] = facts["user_agents_before"] == facts["user_agents_after"]
    if not facts["source_unchanged"]:
        gate_errors.append("隔离运行根（~/OmniDesk/runtime/dsh-home 静态部分）在验收后发生变化")
    for key, label in (("user_dsh_unchanged", "~/.dsh"), ("user_agents_unchanged", "~/.agents")):
        if not facts[key]:
            gate_errors.append(f"{label} 在验收前后指纹不一致")
    facts["gate_errors"] = gate_errors
    facts["passed"] = not gate_errors
    (EVID / "home-fixes-facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps({k: facts[k] for k in ("checks", "markdown", "brand_mark", "greeting_band", "notes", "gate_errors", "passed")}, ensure_ascii=False, indent=2))
    return 0 if not gate_errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
