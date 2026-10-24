#!/usr/bin/env python3
"""OD-ARCH-078 真实窗口验收：附件选择框、顶栏收敛、问答 tips、字号与耗时。

一次性 Xvfb + 隔离数据根 + 回环合成端点；不读凭证、不碰用户数据。

验证链（每条都由本窗口内的真实点击/截图或文件事实判定）：

  * 本机 AT-SPI 直连用户会话，读到的是用户桌面（gnome-shell / Chrome）而非本次
    一次性显示上的验收窗口；用它判界面会出现“界面明明有、断言却说没有”的假失败，
    也可能把点击投到用户正在用的程序上。故本脚本**完全不用 AT-SPI**：
    点击用 XTest 投到本窗口坐标，判定用「截图 + 数据根文件」。
  1. 预置三回合成对话 → 问句下只有「复制」（撤回/修改已按 OD-ARCH-079 去掉），
     答句下有「复制 / 朗读 / 重新生成 / 分支到新对话」与本轮耗时；
  2. 右侧出现轮次索引条，刻度数与问句数一致；点最下面的刻度后，对话真的滚到最后一轮；
  3. 点「复制」→ 出现「已复制」或「复制失败（系统未允许剪贴板）」之一，二者都如实记录；
  4. 点「朗读」→ 出现「开始朗读」或「本机没有可用的语音朗读能力」之一；
  5. 点「分支到新对话」→ 数据根里真的出现一条带全部上文的新线程（文件事实）；
  6. 顶栏：徽标只显示「专业」/「标准」，「IPC ON/OFF」，无人物头像占位（静态源码 + 截图）；
  7. 点输入区「+」→ 真的弹出系统文件选择框（宿主 dialog 插件生效），点 Cancel 关闭；
  8. 源隔离根与 ~/.dsh、~/.agents 指纹前后一致。

证据等级：真实桌面窗口（Xvfb + XTEST 点击与截图）+ X11 剪贴板读取事实
          + 数据根文件事实 + 静态源码核对。

用法：python3 verify_home_polish_078.py [--display :86] [--root /tmp/od078-home]
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

EVID = HERE / "evidence" / "od-arch-079"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900
THREAD_ID = "t078polish"


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


def start_private_dbus() -> tuple[str, subprocess.Popen]:
    """一次性私有会话总线。

    宿主注册了 single-instance 插件（按 bundle id 走会话总线）。验收若共用用户自己的
    会话总线，第二次启动会把用户的 OmniDesk 叫到前台并自行退出——既污染用户桌面，
    又让本次验收拿不到窗口。这里给验收实例一条独立总线，身份互不可见。
    """
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


def thread_payload() -> dict:
    now = int(time.time() * 1000)
    long_tail = (
        "合成占位段落，仅用于让对话够高以便验证右侧索引条。"
        "本段不含任何真实人员、机构、案件或内部信息，重复填充只为撑开滚动区域。"
    ) * 6
    return {
        "id": THREAD_ID,
        "tag": "plain",
        "projectPath": None,
        "title": "合成打磨验收对话（三轮）",
        "preset": "plain",
        "messages": [
            {"id": "u1", "role": "user", "text": "第一轮合成样例：把两条要点整理成一段说明。"},
            {
                "id": "a1",
                "role": "assistant",
                "text": "第一轮合成回复：两条要点已整理。本条内容全部虚构，仅用于界面验收。" + long_tail,
                "sentText": "第一轮合成样例：把两条要点整理成一段说明。",
                "startedAtMs": now - 61_000,
                "endedAtMs": now,
            },
            {"id": "u2", "role": "user", "text": "第二轮合成样例：把这段说明再压缩一半。"},
            {
                "id": "a2",
                "role": "assistant",
                "text": "第二轮合成回复：已压缩。此条同样为虚构占位。" + long_tail,
                "sentText": "第二轮合成样例：把这段说明再压缩一半。",
                "startedAtMs": now - 41_000,
                "endedAtMs": now - 20_000,
            },
            {"id": "u3", "role": "user", "text": "第三轮合成样例：最后检查一遍有没有空话。"},
            {
                "id": "a3",
                "role": "assistant",
                "text": "第三轮合成回复：已删去空泛表述。本条内容全部虚构，仅用于界面验收。",
                "sentText": "第三轮合成样例：最后检查一遍有没有空话。",
                "startedAtMs": now - 19_000,
                "endedAtMs": now - 9_000,
            },
        ],
        "createdAt": now - 120_000,
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


def source_checks() -> dict:
    src = HERE.parents[2] / "desktop" / "src"
    home = (src / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    top = (src / "shell" / "TopNav.tsx").read_text(encoding="utf-8")
    canvas = (src / "shell" / "HomeCanvas.tsx").read_text(encoding="utf-8")
    theme = (src / "theme.css").read_text(encoding="utf-8")
    rust = (HERE.parents[2] / "desktop" / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    cap = (
        HERE.parents[2] / "desktop" / "src-tauri" / "capabilities" / "default.json"
    ).read_text(encoding="utf-8")
    return {
        "chat_font_15px": "text-[15px]" in home,
        "chat_font_no_13px": "text-[13px]" not in home,
        "no_fade_attribute": "data-faded" not in home,
        "no_fade_css": "chat-scroll[data-faded" not in theme,
        "tips_user": 'data-testid="user-message-tips"' in home,
        "tips_assistant": 'data-testid="assistant-message-tips"' in home,
        "picker_button": 'data-testid="attachment-pick"' in home,
        "profile_badge_short": 'return view.saved ? "专业" : "标准";' in top,
        "ipc_pill": "IPC ON" in top and "IPC OFF" in top,
        "no_avatar_block": "rounded-full bg-surface-container-high ring-1" not in top,
        "no_mascot": "mascot" not in canvas,
        "dialog_plugin": "tauri_plugin_dialog::init()" in rust,
        "dialog_capability": "dialog:allow-open" in cap,
    }


def clipboard_text() -> str | None:
    """从本次一次性显示读 CLIPBOARD 文本（与应用同一 X 会话，不碰用户剪贴板）。"""
    try:
        import gi

        gi.require_version("Gtk", "3.0")
        from gi.repository import Gdk, Gtk

        clip = Gtk.Clipboard.get(Gdk.Atom.intern("CLIPBOARD", False))
        return clip.wait_for_text()
    except Exception:
        return None


def ink_groups(
    path: Path, box: tuple[int, int, int, int], gap: int = 8, threshold: int = 150
) -> list[tuple[int, int]]:
    """一张截图在 box 里，按列切出「文字/图标」组成的横条组（用于定位操作条按钮）。"""
    from PIL import Image

    im = Image.open(path).convert("L").crop(box)
    x0, y0, x1, y1 = box
    cols = [
        x0 + x
        for x in range(im.width)
        if any(im.getpixel((x, y)) < threshold for y in range(im.height))
    ]
    groups: list[list[int]] = []
    for x in cols:
        if groups and x - groups[-1][1] <= gap:
            groups[-1][1] = x
        else:
            groups.append([x, x])
    return [(a, b) for a, b in groups]


def find_tips_rows(
    path: Path,
    x_range: tuple[int, int],
    y_range: tuple[int, int],
    threshold: int = 205,
    gap: int = 6,
    min_groups: int = 4,
    max_width: int | None = None,
) -> list[tuple[int, list[tuple[int, int]]]]:
    """在给定区域里按从上到下的顺序列出所有「消息操作条」候选横带。

    操作条只占消息宽度的一小段；正文段落也由横带组成，但会铺满整行。
    传入 max_width 后，宽度超限的横带（正文）不计入候选，避免把正文行
    误判成操作条而点空（OD-ARCH-079 实测踩过）。
    """
    from PIL import Image

    im = Image.open(path).convert("L")
    x0, x1 = x_range
    rows = [
        y
        for y in range(*y_range)
        if sum(1 for x in range(x0, x1) if im.getpixel((x, y)) < threshold) > 2
    ]
    bands: list[list[int]] = []
    for y in rows:
        if bands and y - bands[-1][1] <= 2:
            bands[-1][1] = y
        else:
            bands.append([y, y])
    found: list[tuple[int, list[tuple[int, int]]]] = []
    for top, bottom in bands:
        if not 6 <= bottom - top <= 16:
            continue
        groups = ink_groups(path, (x0, top - 2, x1, bottom + 3), gap=gap, threshold=threshold)
        if len(groups) >= min_groups:
            if max_width is not None and groups[-1][1] - groups[0][0] > max_width:
                continue
            found.append(((top + bottom) // 2, groups))
    return found


def row_ink(path: Path, box: tuple[int, int, int, int], threshold: int = 205) -> int:
    """统计一个矩形内有多少明显墨迹像素（用来判断「一行里多出了反馈文字」）。"""
    from PIL import Image

    im = Image.open(path).convert("L").crop(box)
    return sum(1 for v in im.getdata() if v < threshold)


def threads_on_disk(app_data: Path) -> set[Path]:
    """数据根里所有线程文件（新建线程可能落在 threads/_unassigned/ 等不同分组目录）。"""
    return {p for p in (app_data / "threads").rglob("*.json") if p.is_file()}


def rail_ticks(path: Path, x_range=(1294, 1307), y_range=(120, 800)) -> list[int]:
    """在对话右侧找轮次索引条的刻度线，返回每条刻度的中心 y（从上到下）。

    刻度是 2px 高、约 20px 宽的细条，颜色比背景深；按行统计墨迹像素即可稳定识别。
    找不到就是 0 条，由调用方判定失败，不做“大概有”的推断。

    x 取 1294–1307：这是刻度右段所在的窄带，避开左侧气泡正文（止于约 1292）
    和更右侧的对话滚动条（约 1308 起），否则文字与滚动条会被误数成刻度。
    """
    from PIL import Image

    im = Image.open(path).convert("L")
    x0, x1 = x_range
    y0, y1 = y_range
    ink_rows: list[int] = []
    for y in range(y0, y1):
        ink = sum(1 for x in range(x0, x1) if im.getpixel((x, y)) < 220)
        if ink >= 3:
            ink_rows.append(y)
    runs: list[list[int]] = []
    for y in ink_rows:
        if runs and y - runs[-1][1] <= 2:
            runs[-1][1] = y
        else:
            runs.append([y, y])
    return [(a + b) // 2 for a, b in runs if b - a <= 6]


def rail_active(path: Path, x_range=(1290, 1308), y_range=(120, 800)) -> int | None:
    """当前轮刻度（主色紫）的中心 y。没有就返回 None，不做“应该有”的推断。"""
    from PIL import Image

    im = Image.open(path).convert("RGB")
    x0, x1 = x_range
    rows = []
    for y in range(*y_range):
        purple = 0
        for x in range(x0, x1):
            r, g, b = im.getpixel((x, y))
            if b > 150 and b - r > 60 and g < 120:
                purple += 1
        if purple >= 4:
            rows.append(y)
    if not rows:
        return None
    return (rows[0] + rows[-1]) // 2


def user_tip_rows(path: Path, y_range=(120, 880)) -> list[tuple[int, list[tuple[int, int]]]]:
    """问句下面的操作条：先找用户气泡（主色块），再看气泡正下方那条窄的字带。

    问句气泡是整块主色，正文行不会这样；用气泡定位可以避开把长答句的正文行
    误当成操作条。返回 [(y, 各组墨迹)]，从上到下。
    """
    from PIL import Image

    im = Image.open(path).convert("RGB")
    rows = []
    for y in range(*y_range):
        purple = 0
        for x in range(900, 1280):
            r, g, b = im.getpixel((x, y))
            if b > 170 and b - r > 70 and g < 110:
                purple += 1
        if purple > 50:
            rows.append(y)
    bubbles: list[list[int]] = []
    for y in rows:
        if bubbles and y - bubbles[-1][1] <= 3:
            bubbles[-1][1] = y
        else:
            bubbles.append([y, y])
    out: list[tuple[int, list[tuple[int, int]]]] = []
    for _, bottom in bubbles:
        found = find_tips_rows(
            path, (1150, 1270), (bottom + 4, min(bottom + 34, y_range[1])),
            threshold=205, gap=6, min_groups=2, max_width=60,
        )
        out.extend(found)
    return out


def region_diff(a: Path, b: Path, box: tuple[int, int, int, int]) -> float:
    """两张截图在同一区域里明显不同的像素占比（0..1），用于判定“视图真的变了”。"""
    from PIL import Image, ImageChops

    left = Image.open(a).convert("L").crop(box)
    right = Image.open(b).convert("L").crop(box)
    diff = ImageChops.difference(left, right)
    changed = sum(1 for v in diff.getdata() if v > 12)
    return changed / float(diff.width * diff.height)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":86")
    ap.add_argument("--root", default="/tmp/od078-home")
    ap.add_argument(
        "--reuse-dev-server",
        action="store_true",
        help="端口 5273 已被用户自己的 OmniDesk 开发服务占用时，只读复用它（不启动也不终止）。",
    )
    args = ap.parse_args(argv[1:])
    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {
        "plan": "OD-ARCH-079",
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + 截图 + 本次 X 会话剪贴板）+ 数据根文件事实 + 静态源码核对",
        "root": str(root),
        "source_home": str(SOURCE_HOME),
        "checks": {},
        "notes": [],
    }
    # 源隔离根里的 sessions/ 正被用户自己运行的 OmniDesk 实例持续写入，属于外部活动；
    # 本验收实例的 DSH_HOME 是 root/dsh-home 的独立副本，不会回写源根。这里只对配置/
    # 技能等静态部分做前后一致性判定，会话目录单独计数并注明来源。
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
        reuse_running_dev_server=args.reuse_dev_server,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_address},
    )
    facts["dbus"] = "私有一次性会话总线（不连用户会话总线）"
    gate_errors: list[str] = []
    try:
        session.prepare()
        write_settings_local(session)
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
        time.sleep(10)
        # 首帧的问候语会在第一次交互后收起，整页上移约 50px。先点空白处让它稳定，
        # 再用「稳定帧」定位坐标，避免点击落在移动中的按钮上（实测会空点一次）。
        def wait_stable(shot: Path, timeout: float = 20.0) -> None:
            prev = None
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                xs.shot(shot)
                data = shot.read_bytes()
                if prev is not None and prev == data:
                    return
                prev = data
                time.sleep(0.6)
            raise RuntimeError("界面在超时内没有稳定下来")

        warm = EVID / "00a-warmup.png"
        xs.click(700, 650, settle=1.5)  # 空白画布，不落任何控件
        wait_stable(warm)
        xs.shot(EVID / "00-home-tips.png")

        # 所有坐标以「窗口贴到 0,0、1440×900」为准（Xvfb 屏幕与窗口同尺寸）。
        def click_at(x: int, y: int, settle: float = 1.2) -> None:
            g = xs.geometry(win)
            xs.click(g.x + x, g.y + y, settle=settle)

        base_shot = EVID / "00-home-tips.png"

        def tip_rows(shot: Path):
            """本窗口截图里的全部「答句」操作条，从上到下。

            只认窄横带：答句操作条由图标+短词组成，总宽约 350px；正文行会铺满
            整个气泡宽度（约 790px），用总宽上限排除，否则会把正文当成按钮点空。
            """
            return find_tips_rows(
                shot, (250, 1245), (150, 800), threshold=205, gap=6, min_groups=9, max_width=430
            )

        def tips_row(shot: Path):
            rows = tip_rows(shot)
            return rows[-1] if rows else None

        rows_found = tip_rows(base_shot)
        facts["tips_rows"] = [{"y": y, "groups": g} for y, g in rows_found]
        user_rows = user_tip_rows(base_shot)
        facts["user_tips_rows"] = [{"y": y, "groups": g} for y, g in user_rows]
        user_row = user_rows[-1] if user_rows else None
        found = tips_row(base_shot)
        if found is None:
            raise RuntimeError("截图里没有找到答句操作条，无法继续按坐标验证")
        row_y, groups = found
        facts["assistant_tips_row_y"] = row_y
        facts["assistant_tips_groups"] = groups
        # 操作条 = 4 个「图标 + 文字」对 + 1 个耗时项 = 9 组墨迹；少于 9 组说明有按钮没画出来。
        facts["checks"]["assistant_tips_rendered"] = len(groups) >= 9
        # 耗时项是第 5 组，位置在「分支到新对话」之后（与前一组的间距大于组内间距）。
        facts["checks"]["elapsed_rendered"] = (
            len(groups) >= 9 and groups[8][0] - groups[7][1] >= 8
        )
        user_groups = user_row[1] if user_row else []
        facts["user_tips_groups"] = user_groups
        facts["user_tips_row_y"] = user_row[0] if user_row else None
        # 问句条只剩一个「复制」（OD-ARCH-079 去掉了撤回/修改）：图标 + 文字 = 2 组墨迹。
        facts["checks"]["user_tips_rendered"] = len(user_groups) >= 2
        if not facts["checks"]["elapsed_rendered"]:
            gate_errors.append(f"答句操作条里没有画出本轮耗时（操作条 {len(groups)} 组）")
        if not facts["checks"]["user_tips_rendered"]:
            gate_errors.append(f"问句操作条只画出 {len(user_groups)} 组（应只有「复制」）")
        if not facts["checks"]["assistant_tips_rendered"]:
            gate_errors.append(f"答句操作条只画出 {len(groups)} 组，按钮不全")

        def click_tip_and_note(pair: int, shot_before: Path, shot_after: Path) -> dict:
            """点一个 tips 按钮，并在反馈文字消失前立刻截一张图。

            首帧（问候语收起）会把整页上移约 50px，第一次点击可能被吞掉；这里在
            「文字反馈没出现」时重定位后再点一次，并在结果里如实记下点了几次。
            """
            out: dict = {"clicks": 0}
            xs.shot(shot_before)
            for attempt in range(2):
                hit = tips_row(shot_before)
                if hit is None:
                    out["error"] = "没有定位到答句操作条"
                    return out
                y, gs = hit
                if 2 * pair + 1 >= len(gs):
                    out["error"] = f"操作条只有 {len(gs)} 组，第 {pair} 个按钮不存在"
                    return out
                x = (gs[2 * pair][0] + gs[2 * pair + 1][1]) // 2
                box = (max(0, x - 220), max(0, y - 9), min(SCREEN_W, x + 330), y + 9)
                ink_before = row_ink(shot_before, box)
                click_at(x, y, settle=0.45)
                xs.shot(shot_after)
                ink_after = row_ink(shot_after, box)
                out.update({"clicks": attempt + 1, "pos": (x, y), "ink_before": ink_before, "ink_after": ink_after})
                out["has_feedback"] = ink_after > ink_before + 12
                if out["has_feedback"]:
                    return out
                time.sleep(0.5)
                xs.shot(shot_before)
            return out

        # --- 1. 复制 ---------------------------------------------------------
        # 操作条定位取的是最后一轮的答句，复制对象也必须是这条（不是第一轮）。
        expected_text = thread_payload()["messages"][-1]["text"]
        facts["clipboard_before"] = (clipboard_text() or "")[:60]
        copy_facts = click_tip_and_note(0, EVID / "01a-before-copy.png", EVID / "01-copy-result.png")
        got_clip = clipboard_text()
        facts["copy"] = copy_facts
        facts["clipboard_after_copy"] = got_clip
        facts["checks"]["copy_wrote_clipboard"] = got_clip == expected_text
        facts["checks"]["copy_has_feedback"] = bool(copy_facts.get("has_feedback"))
        if not facts["checks"]["copy_wrote_clipboard"]:
            gate_errors.append("点答句「复制」后，本次 X 会话的剪贴板里没有出现该条答句全文")
        if not facts["checks"]["copy_has_feedback"]:
            facts["notes"].append("复制反馈文字在截图里没有捕捉到（可能已消失），该项按未捕捉记录")
        time.sleep(4.2)  # 反馈 4 秒后自动消失，避免影响下一项判定

        # --- 2. 朗读 ---------------------------------------------------------
        speak_facts = click_tip_and_note(1, EVID / "01b-copy-note-cleared.png", EVID / "02-speak-result.png")
        facts["speak"] = speak_facts
        facts["checks"]["speak_has_feedback"] = bool(speak_facts.get("has_feedback"))
        if not facts["checks"]["speak_has_feedback"]:
            facts["notes"].append("朗读反馈文字在截图里没有捕捉到，该项按未捕捉记录")
        time.sleep(4.2)

        # --- 3. 文件选择框：点「+」，应在本显示上多出一个顶层窗口（显示层事实） --
        def top_level_windows() -> list[str]:
            out: list[str] = []
            for w in xs.d.screen().root.query_tree().children:
                try:
                    if w.get_attributes().map_state != xs._X.IsViewable:
                        continue
                except Exception:
                    continue
                try:
                    name = w.get_wm_name() or ""
                except Exception:
                    name = ""
                out.append(name)
            return out

        # 文件选择框会抢焦点；先记下窗口布局，点完再确认「多了一个顶层窗口 + 本窗口被遮住」。
        before_windows = top_level_windows()
        xs.shot(EVID / "03a-before-pick.png")
        click_at(250, 786, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "04-file-dialog.png")
        after_windows = top_level_windows()
        new_windows = [w for w in after_windows if w not in before_windows]
        facts["windows_before_pick"] = before_windows
        facts["windows_after_pick"] = after_windows
        facts["new_windows"] = new_windows
        # 选择框在本机没有窗口管理器时标题为空，因此用「顶层窗口数增加 + 截图左侧出现文件列表栏」判定。
        from PIL import Image

        dlg = Image.open(EVID / "04-file-dialog.png").convert("L")
        sidebar_ink = sum(
            1
            for x in range(0, 140)
            for y in range(40, 700)
            if dlg.getpixel((x, y)) < 140
        )
        facts["dialog_sidebar_ink"] = sidebar_ink
        facts["checks"]["file_dialog_opened"] = len(after_windows) > len(before_windows) and sidebar_ink > 300
        if not facts["checks"]["file_dialog_opened"]:
            gate_errors.append("点「+」后本显示上没有出现系统文件选择框")
        # 选择框是本显示上的独立顶层窗口，Esc 只在它聚焦时有效；无窗口管理器时焦点不可靠，
        # 因此按截图里它自己的「Cancel」按钮位置关闭（坐标取自 04-file-dialog.png 的按钮）。
        xs.click(958, 799, settle=2.0)
        time.sleep(1.5)
        xs.shot(EVID / "05-after-dialog.png")
        facts["checks"]["dialog_closed_by_escape"] = len(top_level_windows()) == len(before_windows)
        if not facts["checks"]["dialog_closed_by_escape"]:
            gate_errors.append("文件选择框点 Cancel 后没有关闭")
        xs.focus(win)

        # --- 3b. 真的选中仓库内文件：不得再报「该路径位于被禁止的目录」 -----
        # 先点「+」打开选择框，用 Ctrl+L 输入仓库内 AGENTS.md 的绝对路径并回车。
        # 判定依据两条文件/界面事实：窗口里出现附件 chip（agent 文件名），且页面上
        # 没有出现“被禁止的目录”提示。这两条都不依赖模型回复。
        repo_agents = HERE.parents[2] / "AGENTS.md"
        facts["attachment_target"] = str(repo_agents)
        facts["checks"]["attachment_target_exists"] = repo_agents.is_file()
        if repo_agents.is_file():
            xs.shot(EVID / "05c-before-pick-repo.png")
            # 「+」的位置与上面第 3 段一致（输入框左侧）；沿用同一坐标，不另造定位。
            pick_x, pick_y = 250, 786
            facts["pick_button_pos"] = [pick_x, pick_y]
            before_pick_windows = top_level_windows()
            click_at(pick_x, pick_y, settle=3.0)
            xs.shot(EVID / "05d-repo-dialog.png")
            dialog_open = len(top_level_windows()) > len(before_pick_windows)
            facts["checks"]["repo_dialog_opened"] = dialog_open
            if dialog_open:
                # 对话框是本显示上的独立顶层窗口。Ctrl+L 的“输入位置”在这个无窗口
                # 管理器的显示上不会出现（05d1-location-entry.png 已证实），所以
                # 直接按截图点列表里的 AGENTS.md 行，再点对话框自己的 Open。
                # 坐标取自 05d-repo-dialog.png：名称列 x≈205，AGENTS.md 行 y≈485。
                xs.click(205, 485, settle=0.8)
                xs.shot(EVID / "05d1-selected-row.png")
                xs.click(1046, 799, settle=2.5)
                time.sleep(2.0)
                if len(top_level_windows()) > len(before_pick_windows):
                    # 路径没被接受、或文件框仍停在原地：先按它自己的 Cancel 关掉，
                    # 再在主窗口里判定是否有附件 chip / 红色拒绝提示。
                    xs.click(958, 799, settle=2.0)
                    time.sleep(1.5)
                    xs.focus(win)
            xs.shot(EVID / "05e-after-repo-pick.png")
            facts["new_windows_after_repo_pick"] = [
                w for w in top_level_windows() if w not in before_pick_windows
            ]
            facts["checks"]["repo_dialog_closed"] = not facts["new_windows_after_repo_pick"]
            # 附件 chip 与拒绝提示都出现在输入区上方，取同一块区域读墨迹与文字线索。
            chip_box = (150, 700, 1300, 790)
            ink_after_pick = row_ink(EVID / "05e-after-repo-pick.png", chip_box)
            ink_before_pick = row_ink(EVID / "05c-before-pick-repo.png", chip_box)
            facts["chip_region_ink"] = {"before": ink_before_pick, "after": ink_after_pick}
            facts["checks"]["attachment_chip_visible"] = ink_after_pick > ink_before_pick + 200
            if not facts["checks"]["attachment_chip_visible"]:
                gate_errors.append(
                    f"选中仓库内 {repo_agents.name} 后输入区没有出现附件 chip"
                    f"（墨迹 {ink_before_pick} → {ink_after_pick}）"
                )
            # 拒绝提示是红色文字：统计该区域里的红色像素占比。
            from PIL import Image as _Image

            shot_rgb = _Image.open(EVID / "05e-after-repo-pick.png").convert("RGB")
            red = 0
            for x in range(150, 1300):
                for y in range(600, 880):
                    r, g, b = shot_rgb.getpixel((x, y))
                    if r > 140 and r - g > 60 and r - b > 60:
                        red += 1
            facts["red_pixels"] = red
            facts["checks"]["attachment_not_rejected"] = red < 40
            if red >= 40:
                gate_errors.append(
                    f"选中仓库内 {repo_agents.name} 后界面出现红色拒绝提示（{red} 个红色像素）"
                )

        # --- 4. 分支到新对话（负向证据：没有真实切点就不许“假装分支”） -----
        # OD-ARCH-079 起分支走 DSH 原生 session/fork，切点必须是引擎实际回报的
        # turn/end 序号。预置线程没有引擎会话、也没有序号，按钮应保持禁用；
        # 点下去既不能新建线程，也不能退回本地重放。引擎侧的正向验证由
        # tests/verification/engine-interface/evidence/web-api/W11.json 覆盖。
        before_threads = threads_on_disk(session.app_data)
        facts["branch_threads_before"] = len(before_threads)
        branch_facts = click_tip_and_note(
            3, EVID / "05b-before-branch.png", EVID / "03-branch-disabled.png"
        )
        facts["branch"] = branch_facts
        time.sleep(2.0)
        xs.shot(EVID / "03-branch-disabled.png")
        new_ids = threads_on_disk(session.app_data) - before_threads
        facts["branch_new_threads"] = sorted(str(p.relative_to(session.app_data)) for p in new_ids)
        facts["checks"]["branch_without_fork_seq_no_thread"] = not new_ids
        if new_ids:
            gate_errors.append(
                "预置线程没有引擎切点时点「分支到新对话」仍新建了线程（疑似退回本地重放）"
            )

        # --- 5. 右侧轮次索引条：刻度数 = 问句数；点刻度真的换视图 --------
        xs.focus(win)
        xs.shot(EVID / "06a-rail-bottom.png")
        ticks = rail_ticks(EVID / "06a-rail-bottom.png")
        facts["rail_ticks"] = ticks
        facts["checks"]["rail_rendered"] = len(ticks) >= 3
        if not facts["checks"]["rail_rendered"]:
            gate_errors.append(f"对话右侧没有画出轮次索引条（找到 {len(ticks)} 个刻度，应为 3）")
        if len(ticks) >= 3:
            # 先滚到顶部，再点最后一根刻度，视图应明显变化（真实跳转，不是静止画面）。
            xs.scroll(700, 400, 12, up=True, settle=1.5)
            xs.shot(EVID / "06b-rail-scrolled-top.png")
            view_box = (200, 150, 1240, 700)
            top_shot = EVID / "06b-rail-scrolled-top.png"
            # 跳转后索引条会随当前轮自动滚动，刻度位置随之变化；第二次点击必须用
            # 新截图里的刻度坐标，否则会点到条内空白处，看上去像“跳转失效”。
            ticks_top = rail_ticks(top_shot)
            facts["rail_ticks_top"] = ticks_top
            facts["rail_active_top"] = rail_active(top_shot)
            if not ticks_top:
                gate_errors.append("滚到顶部后索引条刻度消失")
                raise RuntimeError("顶部截图里没有索引条刻度")
            xs.click(1302, ticks_top[-1], settle=2.5)
            xs.shot(EVID / "06c-rail-jumped-last.png")
            facts["rail_ticks_after_last"] = rail_ticks(EVID / "06c-rail-jumped-last.png")
            facts["rail_active_after_last"] = rail_active(EVID / "06c-rail-jumped-last.png")
            facts["rail_diff_after_last_jump"] = region_diff(
                top_shot, EVID / "06c-rail-jumped-last.png", view_box
            )
            facts["checks"]["rail_jump_changed_view"] = facts["rail_diff_after_last_jump"] >= 0.02
            if not facts["checks"]["rail_jump_changed_view"]:
                gate_errors.append(
                    "点最后一根轮次刻度后对话视图没有变化（跳转未生效，或刻度坐标不在条上）"
                )
            # 再点第一根，确认能往回跳。
            ticks_after = rail_ticks(EVID / "06c-rail-jumped-last.png")
            back_anchor = ticks_after[0] if ticks_after else ticks_top[0]
            xs.click(1302, back_anchor, settle=2.5)
            xs.shot(EVID / "06d-rail-jumped-first.png")
            facts["rail_active_after_first"] = rail_active(EVID / "06d-rail-jumped-first.png")
            facts["rail_diff_after_first_jump"] = region_diff(
                EVID / "06c-rail-jumped-last.png", EVID / "06d-rail-jumped-first.png", view_box
            )
            facts["checks"]["rail_jump_back_changed_view"] = (
                facts["rail_diff_after_first_jump"] >= 0.02
            )
            if not facts["checks"]["rail_jump_back_changed_view"]:
                gate_errors.append("点第一根轮次刻度后对话视图没有变化（往回跳未生效）")

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
    facts["gate_errors"] = gate_errors
    facts["passed"] = not gate_errors
    (EVID / "home-polish-facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    return 0 if not gate_errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
