#!/usr/bin/env python3
"""OD-ARCH-059 S2 · 对话中枢归一（首页对话 + 场景页纯档案化）真实窗口验收。

覆盖四件事，全部在 /tmp 下的一次性数据根里（合成对话，无真实材料、无凭证）：

  1. 首页输入区只有场景 chip 与「新对话」，不再并排第二个历史对话下拉框；
     Dock 第二个入口显示「工作记录」；
  2. 预置一条本机对话记录 → 重启后首页自动载入它（历史切换统一在场景页左栏），
     点标题回首页续聊且历史完整；
  3. 场景页不再有对话框（单层档案页截图）；
  4. 在首页用场景 chip 把该对话改挂到「文案写作」，落盘 tag 同步变化；
     再隐藏声明里的「文案写作」重启，同样两次下移只能落到「项目跟进」——
     说明该选项确实从 chip 里消失（文件事实，不靠截图猜）。

证据等级：真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实（线程记录 JSON）。
不碰 runtime/、~/.dsh、~/.agents、~/OmniDesk 或用户日常数据；不读取任何凭证。

用法：python3 verify_conversation_center_059.py [--display :97] [--root /tmp/od059-s2-verify]
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

EVIDENCE = HERE / "evidence" / "od-arch-059" / "S2"
SCREEN_W, SCREEN_H = 1440, 900

# Dock：8 个入口时的实测坐标（窗口左上角为原点）。
DOCK_DX = 47
DOCK_REPORT_DY = 325

THREAD_ID = "t059s2verify"
DEFAULT_DECL = (REPO / "samples" / "profiles" / "discipline.yml").read_text(encoding="utf-8")
HIDDEN_DECL = DEFAULT_DECL.replace(
    """  - id: writing
    label: 文案写作
    icon: book
    note: 模型起草
""",
    "",
)


def start_xvfb(display: str) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(2.0)
    return proc


def thread_payload() -> dict:
    now = int(time.time() * 1000)
    return {
        "id": THREAD_ID,
        "tag": "weekly",
        "projectPath": None,
        "title": "合成验收对话",
        "preset": "weekly",
        "messages": [
            {
                "id": "m1",
                "role": "user",
                "text": "合成样例：本周做了三件事，其中一件未完成。",
            },
            {
                "id": "m2",
                "role": "assistant",
                "text": "合成回复：已按工作记录整理，未完成的一项单列为待跟进。",
            },
        ],
        "createdAt": now - 60000,
        "updatedAt": now,
        "version": 1,
    }


def read_thread(app_data: Path) -> dict | None:
    for path in app_data.glob(f"threads/*/{THREAD_ID}.json"):
        return json.loads(path.read_text(encoding="utf-8"))
    return None


def mount(session: AppSession, xs: XSession):
    win = xs.find_window("OmniDesk", timeout=90)
    if win is None:
        raise RuntimeError("未找到 OmniDesk 主窗口")
    xs.focus(win)
    xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
    time.sleep(2.0)
    return win


def click(xs: XSession, win, dx: int, dy: int, settle: float = 1.2) -> None:
    g = xs.geometry(win)
    point = (g.x + dx, g.y + dy)
    xs.focus(win)
    xs.move(*point)
    time.sleep(0.3)
    # 无边框窗口第一次点击常只用来把焦点交给窗口。
    xs.click(*point, settle=0.4)
    xs.click(*point, settle=settle)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root", default="/tmp/od059-s2-verify")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    EVIDENCE.mkdir(parents=True, exist_ok=True)

    xvfb = start_xvfb(args.display)
    # artifact_override：成果目录也指向一次性目录。不隔离时场景页会列出用户真实
    # 成果文件名，等于把个人数据拍进验收截图（059 S2 首轮即出现）。
    session = AppSession(root=root, display=args.display, artifact_override=True)
    facts: dict = {
        "plan": "OD-ARCH-059 S2",
        "evidence_level": "真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实（线程记录 JSON）",
        "root": str(root),
        "steps": {},
    }
    try:
        session.prepare()
        xs = XSession(args.display)
        decl_path = session.app_data / "profiles" / "discipline.yml"

        # --- 1. 首页：场景 chip / 新对话 / Dock 改名（历史下拉已移除）-------------
        session.start()
        win = mount(session, xs)
        xs.shot(EVIDENCE / "01-home-chips.png")
        facts["steps"]["home"] = {
            "screenshot": str(EVIDENCE / "01-home-chips.png"),
            "app_alive": session.alive(),
            "expect": "输入区只有 场景chip + 新对话（不再有第二个历史对话下拉框）；Dock 第二个入口为「工作记录」",
        }

        # --- 2. 预置对话 → 重启 → 场景页列出 → 回首页续聊 ----------------------
        target = session.app_data / "threads" / "_unassigned" / f"{THREAD_ID}.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            json.dumps(thread_payload(), ensure_ascii=False, indent=2), encoding="utf-8"
        )

        session.stop_app()
        time.sleep(1.5)
        session.start()
        win = mount(session, xs)
        xs.shot(EVIDENCE / "02-home-loaded-thread.png")
        facts["steps"]["loaded"] = {
            "screenshot": str(EVIDENCE / "02-home-loaded-thread.png"),
            "thread_file": str(target),
            "expect": "首页自动载入「[工作记录] 合成验收对话」，两条消息完整",
        }

        click(xs, win, DOCK_DX, DOCK_REPORT_DY, settle=2.5)
        xs.shot(EVIDENCE / "03-scene-archive.png")
        facts["steps"]["scene_page"] = {
            "screenshot": str(EVIDENCE / "03-scene-archive.png"),
            "expect": "左栏「工作记录 · 历史对话」列出该对话；页面无对话框（单层档案页）",
        }

        # 点左栏标题 → 回首页并载入该对话。
        # 坐标按「隔离后的空成果库」校准：公告栏少两行，列表比带真实数据时上移约 22px。
        click(xs, win, 170, 366, settle=2.5)
        xs.shot(EVIDENCE / "04-open-thread-home.png")
        facts["steps"]["open_thread"] = {
            "screenshot": str(EVIDENCE / "04-open-thread-home.png"),
            "expect": "回到首页，显示该对话原有两条消息",
        }

        # --- 3. 首页改挂场景：工作记录 → 文案写作 -------------------------------
        # 原生 select：展开后先按 Home 回到第一项，再按 Down 计数，行为可复现。
        # 选项顺序：自由对话 / 工作记录 / 文案写作 / 项目跟进；两次 Down = 文案写作。
        before = read_thread(session.app_data) or {}
        click(xs, win, 286, 833, settle=1.0)
        xs.key("Home", settle=0.6)
        xs.key("Down", settle=0.4)
        xs.key("Down", settle=0.4)
        xs.key("Return", settle=1.5)
        xs.shot(EVIDENCE / "05-retag-writing.png")
        time.sleep(2.0)
        after = read_thread(session.app_data) or {}
        facts["steps"]["retag"] = {
            "screenshot": str(EVIDENCE / "05-retag-writing.png"),
            "tag_before": before.get("tag"),
            "tag_after": after.get("tag"),
            "messages_after": len(after.get("messages") or []),
            "expect": "tag 由 weekly 变为 writing，消息条数不变",
        }
        session.stop_app()
        time.sleep(1.5)

        # --- 4. 声明隐藏「文案写作」→ 首页 chip 联动 ---------------------------
        # 先把测试副本的 tag 改回 weekly（改的是 /tmp 一次性副本，不是用户数据），
        # 否则重启后 chip 会停在「文案写作」这个已被隐藏的场景上。
        reset = read_thread(session.app_data) or {}
        reset["tag"] = "weekly"
        for path in session.app_data.glob(f"threads/*/{THREAD_ID}.json"):
            path.write_text(json.dumps(reset, ensure_ascii=False, indent=2), encoding="utf-8")
        if HIDDEN_DECL == DEFAULT_DECL:
            raise RuntimeError("声明替换未生效：HIDDEN_DECL 与默认声明相同")
        decl_path.write_text(HIDDEN_DECL, encoding="utf-8")
        session.start()
        win = mount(session, xs)
        # 先展开再看一眼选项（截图留证），再用键盘落到第二项。
        # 两次点击会把原生下拉开了又关，所以用空格显式展开后再截。
        click(xs, win, 286, 833, settle=1.2)
        xs.key("space", settle=1.0)
        xs.shot(EVIDENCE / "06-chip-without-writing.png")
        xs.key("Home", settle=0.6)
        xs.key("Down", settle=0.4)
        xs.key("Down", settle=0.4)
        xs.key("Return", settle=1.5)
        time.sleep(2.0)
        hidden_tag = (read_thread(session.app_data) or {}).get("tag")
        facts["steps"]["decl_linkage"] = {
            "screenshot": str(EVIDENCE / "06-chip-without-writing.png"),
            "decl_path": str(decl_path),
            "tag_after_home_two_downs_with_writing_hidden": hidden_tag,
            "expect": "同为 Home+两次下移：正常时落到 writing，隐藏后落到 project",
        }
        xs.key("Escape", settle=0.6)
        session.stop_app()
        time.sleep(1.0)
        decl_path.write_text(DEFAULT_DECL, encoding="utf-8")
        facts["decl_restored"] = decl_path.read_text(encoding="utf-8") == DEFAULT_DECL
        facts["app_alive_at_end"] = not session.alive()
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    (EVIDENCE / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False, indent=2))
    ok = (
        facts.get("decl_restored")
        and facts["steps"].get("retag", {}).get("tag_after") == "writing"
        and facts["steps"]
        .get("decl_linkage", {})
        .get("tag_after_home_two_downs_with_writing_hidden")
        == "project"
    )
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
