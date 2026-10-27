#!/usr/bin/env python3
"""OD-ARCH-094 S05：文案写作页三栏改版的真实窗口验证（一次性 Xvfb）。

只用 /tmp 下的合成数据：合成写作成果（草稿 / 成文 / 已发布 / 无 frontmatter /
非法值 / 超过 90 天未动）与两条合成写作会话。不读真实知识库、不读正式资料、
不打印凭证、不改动用户正在运行的实例。

抓帧与断言：
  01 宽窗三栏（四卡 + 继续创作 / 灵感架 + 状态管线 + 成果库）
  02 状态筛选（草稿组）
  03 90 天未动提醒点击筛选
  04 灵感架点击回首页并定位对应会话
  05 窄窗 1000 单列

用法：python3 preview_writing_094.py [--display :129] [--root /tmp/od094-preview]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-094"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 2900
NARROW_W, NARROW_H = 1000, 2740

INSPIRATION_TITLE = "合成灵感：一句没写完的话"


def start_display(display: str, w: int = ROOT_W, h: int = ROOT_H) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{w}x{h}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError("Xvfb 未就绪")


def start_dbus() -> tuple[str, subprocess.Popen]:
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    return proc.stdout.readline().strip(), proc


def app_nodes(pid):
    from gi.repository import Atspi

    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        try:
            if app.get_process_id() != pid:
                continue
        except Exception:
            continue
        raw: list = []
        ax._collect(app, raw)
        return [ax._node(n) for n in raw]
    return []


def blob(node) -> str:
    return (node.name or "") + "\n" + (node.text or "")


def find(nodes, name, contains: bool = True):
    for node in nodes:
        a, b = node.name or "", node.text or ""
        if (name in a or name in b) if contains else (name == a or name == b):
            return node
    return None


def all_text(nodes) -> str:
    return "\n".join(blob(n) for n in nodes)


def node_pos(node) -> tuple[int, int, int, int] | None:
    from gi.repository import Atspi

    try:
        ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return None
    if ext.width <= 0 or ext.height <= 0:
        return None
    return (ext.x, ext.y, ext.width, ext.height)


def dump(nodes, path: Path) -> None:
    path.write_text(
        json.dumps(
            [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )


def write_artifacts(data_root: Path) -> dict:
    """合成 6 份写作成果；全部文件名符合宿主只读列举的约定形状。"""
    data_root.mkdir(parents=True, exist_ok=True)
    now = time.time()
    day = 86400.0
    today = dt.datetime.now(dt.timezone.utc).date()
    cases = [
        ("合成草稿：关于创造力的三种开场", "draft", "ai", 1.0, today),
        ("合成成文：我为什么每周留两小时空白", "finished", "blog", 3.0, today),
        ("合成已发布：给新读者的一封信", "published", "website", 6.0, today),
        (None, None, None, 9.0, today),  # 无 frontmatter
        ("合成旧文：合法性边界检查", "unknown-status", "乱写类型", 12.0, today),
        ("合成旧文：三个月前写下的观察", "finished", "insight", 120.0, today),
    ]
    written = []
    for i, (title, status, kind, age_days, _day) in enumerate(cases):
        stamp = (today - dt.timedelta(days=int(age_days) % 27)).strftime("%Y%m%d")
        name = f"offline-material-writing-{stamp}-{90000 + i * 137:06d}.md"
        if title is None:
            body = "# 没有 frontmatter 的旧稿\n\n只有正文，没有任何元数据。\n"
        else:
            body = (
                "---\n"
                "scenario: material_writing\n"
                f'title: "{title}"\n'
                f"status: {status}\n"
                f"type: {kind}\n"
                "---\n"
                f"# {title}\n\n"
                "这是一份只用于界面验证的合成稿件正文。\n"
            )
        path = data_root / name
        path.write_text(body, encoding="utf-8")
        mtime = now - age_days * day
        os.utime(path, (mtime, mtime))
        written.append({"file": name, "status": status, "type": kind, "age_days": age_days})
    return {"count": len(written), "items": written}


def write_threads(app_data: Path) -> list[dict]:
    """两条合成写作会话：一条用于灵感架跳转，一条更早。"""
    root = app_data / "threads" / "_unassigned"
    root.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    threads = [
        {
            "id": "t094inspiration01",
            "tag": "writing",
            "projectPath": None,
            "title": INSPIRATION_TITLE,
            "preset": "writing",
            "messages": [],
            "createdAt": now - 7200,
            "updatedAt": now - 600,
            "version": 1,
        },
        {
            "id": "t094inspiration02",
            "tag": "writing",
            "projectPath": None,
            "title": "合成灵感：旧结构还能怎么拆",
            "preset": "writing",
            "messages": [],
            "createdAt": now - 86400 * 3,
            "updatedAt": now - 86400 * 2,
            "version": 1,
        },
    ]
    for item in threads:
        (root / f"{item['id']}.json").write_text(
            json.dumps(item, ensure_ascii=False, indent=2), encoding="utf-8"
        )
    return threads


def stage_counts(text: str) -> dict[str, int]:
    """从中栏阶段卡的可见文字里读出三组计数。"""
    out: dict[str, int] = {}
    for label in ("草稿", "成文", "已发布"):
        m = re.search(r"(\d+)\s*份?\s*" + label, text)
        if m:
            out[label] = int(m.group(1))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":129")
    ap.add_argument("--root", default="/tmp/od094-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    data_root = root / "local-data"
    facts: dict = {"display": args.display, "root": str(root)}
    facts["artifacts"] = write_artifacts(data_root)

    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    if "AT_SPI_BUS_ADDRESS" not in os.environ and os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ["AT_SPI_BUS_ADDRESS"] = "unix:path=/run/user/1000/at-spi/bus"
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )
    try:
        session.prepare()
        facts["threads"] = write_threads(session.app_data)
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=150)
        if win is None:
            raise RuntimeError("主窗口未出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        if not xs.wait_rendered(win, timeout=120):
            raise RuntimeError("界面未渲染完成")
        ax.init()
        nodes = []
        dock = None
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline and dock is None:
            nodes = app_nodes(pid)
            dock = find(nodes, "文案写作")
            if dock is None:
                time.sleep(2.0)
        dump(nodes, EVID / "00-home-ax.json")
        if dock is None:
            xs.shot(EVID / "00-home.png")
            raise RuntimeError("找不到「文案写作」入口")
        ax.invoke(dock)
        time.sleep(6.0)

        # 01：宽窗三栏 + 四卡。
        xs.shot(EVID / "01-writing-three-column.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "01-writing-ax.json")
        text = all_text(nodes)
        facts["metrics_present"] = {
            label: label in text for label in ("本月成文", "进行中草稿", "灵感待用", "最近更新")
        }
        facts["board_present"] = all(
            label in text for label in ("继续创作", "灵感架", "状态管线", "成果文档库")
        )
        facts["default_note_present"] = "按「成文」显示" in text
        facts["stage_counts"] = stage_counts(text)
        facts["stage_sum"] = sum(facts["stage_counts"].values())
        facts["artifact_total"] = facts["artifacts"]["count"]
        facts["stage_sum_ok"] = facts["stage_sum"] == facts["artifact_total"]
        facts["type_labels_present"] = all(
            label in text for label in ("AI 知识", "blog", "网站内容", "学习心得", "未分类")
        )
        facts["continue_title_from_frontmatter"] = "关于创造力的三种开场" in text
        facts["stale_reminder_present"] = "超过 90 天未动" in text
        facts["stale_badge_present"] = "90 天未动" in text
        facts["inspiration_present"] = INSPIRATION_TITLE in text

        # 02：点「草稿」阶段卡筛出草稿组。
        draft_btn = None
        for n in nodes:
            if n.role in ("push button", "button", "toggle button") and "草稿" in blob(n):
                if re.search(r"\d+\s*份?\s*草稿", blob(n)):
                    draft_btn = n
                    break
        if draft_btn is not None:
            ax.invoke(draft_btn)
            time.sleep(2.5)
            xs.shot(EVID / "02-draft-filter.png")
            after = app_nodes(pid)
            dump(after, EVID / "02-draft-ax.json")
            atext = all_text(after)
            # 右栏成果库始终显示全部条目；这里只看中栏管线条目（不带文件名的那种）。
            pipeline_rows = [
                blob(n)
                for n in after
                if n.role in ("push button", "button", "toggle button")
                and "offline-material-writing-" not in blob(n)
            ]
            joined = "\n".join(pipeline_rows)
            facts["draft_filter_kept_only_draft"] = (
                "关于创造力的三种开场" in joined and "三个月前写下的观察" not in joined
            )
            facts["draft_filter_applied"] = (
                sum(1 for row in pipeline_rows if "合成" in row and "继续对话" not in row) == 1
            )
            ax.invoke(draft_btn)
            time.sleep(2.0)
        else:
            facts["draft_filter_applied"] = False
            facts["draft_filter_kept_only_draft"] = False

        # 03：90 天未动提醒点击筛选。
        nodes = app_nodes(pid)
        stale_btn = next(
            (
                n
                for n in nodes
                if "超过 90 天未动" in blob(n)
                and n.role in ("push button", "button", "toggle button")
            ),
            None,
        )
        if stale_btn is not None:
            ax.invoke(stale_btn)
            time.sleep(2.5)
            xs.shot(EVID / "03-stale-filter.png")
            after = app_nodes(pid)
            dump(after, EVID / "03-stale-ax.json")
            atext = all_text(after)
            facts["stale_filter_applied"] = True
            pipeline_rows = [
                blob(n)
                for n in after
                if n.role in ("push button", "button", "toggle button")
                and "offline-material-writing-" not in blob(n)
            ]
            joined = "\n".join(pipeline_rows)
            facts["stale_filter_kept_only_stale"] = (
                "三个月前写下的观察" in joined and "关于创造力的三种开场" not in joined
            )
            ax.invoke(stale_btn)
            time.sleep(2.0)
        else:
            facts["stale_filter_applied"] = False
            facts["stale_filter_kept_only_stale"] = False

        # 04：灵感架点击回首页并定位对应会话。
        nodes = app_nodes(pid)
        inspiration_btn = next(
            (
                n
                for n in nodes
                if n.role in ("push button", "button") and INSPIRATION_TITLE in blob(n)
            ),
            None,
        )
        if inspiration_btn is not None:
            ax.invoke(inspiration_btn)
            time.sleep(4.0)
            xs.shot(EVID / "04-inspiration-home.png")
            after = app_nodes(pid)
            dump(after, EVID / "04-inspiration-ax.json")
            atext = all_text(after)
            # 首页不显示会话标题；但归类下拉会带出被打开会话的标签（writing），
            # 且首页输入区出现，作为「回首页并载入该会话」的可见证据。
            facts["inspiration_navigated_home"] = "欢迎回到 OmniDesk" in atext
            facts["inspiration_thread_selected"] = "本次对话归到哪一类，当前：文案写作" in atext
            # 回写作页，继续做窄窗验证。
            back = find(app_nodes(pid), "文案写作")
            if back is not None:
                ax.invoke(back)
                time.sleep(3.5)
        else:
            facts["inspiration_navigated_home"] = False
            facts["inspiration_thread_selected"] = False

        # 05：窄窗单列。
        xs.resize(win, NARROW_W, NARROW_H, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "05-narrow-1000.png")
        narrow = app_nodes(pid)
        dump(narrow, EVID / "05-narrow-ax.json")
        positions = []
        for label in ("继续创作", "灵感架", "状态管线", "成果文档库"):
            hit = next(
                (
                    n
                    for n in narrow
                    if n.role == "landmark" and (n.name or n.text or "").strip() == label
                ),
                None,
            ) or find(narrow, label)
            pos = node_pos(hit) if hit is not None else None
            if pos is None:
                facts.setdefault("narrow_missing", []).append(label)
            else:
                positions.append((pos[1], label, pos[0], pos[2]))
        positions.sort()
        facts["narrow_order"] = [label for _, label, _, _ in positions]
        # 单列按区块容器的左缘判断：标题本身有不同内边距，左缘差 15px 不代表两列。
        lefts = [x for _, _, x, _ in positions]
        facts["narrow_left_spread"] = max(lefts) - min(lefts) if lefts else 0
        facts["narrow_left_edges"] = {label: x for _, label, x, _ in positions}
        rights = [x + w for _, _, x, w in positions]
        facts["narrow_max_right"] = max(rights) if rights else 0
        facts["narrow_overflow"] = facts["narrow_max_right"] > NARROW_W
        facts["narrow_single_column"] = (
            len(positions) == 4
            and facts["narrow_left_spread"] <= 40
            and [y for y, _, _, _ in positions] == sorted(y for y, _, _, _ in positions)
            and not facts["narrow_overflow"]
        )

        facts["passed"] = all(
            [
                all(facts["metrics_present"].values()),
                facts["board_present"],
                facts["default_note_present"],
                facts["stage_sum_ok"],
                facts["type_labels_present"],
                facts["continue_title_from_frontmatter"],
                facts["stale_reminder_present"],
                facts["stale_badge_present"],
                facts["inspiration_present"],
                facts["draft_filter_applied"],
                facts["draft_filter_kept_only_draft"],
                facts["stale_filter_applied"],
                facts["stale_filter_kept_only_stale"],
                facts["inspiration_navigated_home"],
                facts["inspiration_thread_selected"],
                facts["narrow_single_column"],
            ]
        )
        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("saved", EVID)
        print(json.dumps(facts, ensure_ascii=False))
        return 0 if facts["passed"] else 1
    finally:
        session.close()
        try:
            xvfb.terminate()
            xvfb.wait(timeout=10)
        except Exception:
            pass
        try:
            bus_proc.terminate()
        except Exception:
            pass


if __name__ == "__main__":
    raise SystemExit(main())
