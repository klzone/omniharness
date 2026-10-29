#!/usr/bin/env python3
"""OD-ARCH-108 真实窗口验收：按流程注入 —— 随手记只带 memory-agent，周报只带 memory-report。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI + 合成模型端点）。
合成端点只记录「探针是否出现在模型请求里」，不保存请求正文；
所有输入均为合成标记，不接触任何真实工作记录。

为什么需要这一层：OD-ARCH-082 的脚本是在 Node 里直接调用编译后的
composeConversationPrompt，属于「同一份源码」的证据，但没有经过界面提交链路。
本轮要在真实窗口里证明：从输入框提交、经 HomeCanvas 取绑定、到模型请求，
随手记那一轮只出现 memory-agent，周报那一轮只出现 memory-report，两轮都不出现
anysearch 正文（它只按需加载）。

两轮都走**产品自身的路径**，不靠脚本伪造意图：
  1. Dock「工作记录」→「去对话」→ 输入 ASCII 合成句（不含「周报」二字）→ 发送；
  2. 回「工作记录」场景页 →「去生成周报」（预填「整理成周报」，中文由产品给出，
     不需要在 X 里打中文）→ 发送。

用法：python3 verify_skill_flow_injection_108.py [--display :131] [--root /tmp/od108-flow]
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

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-108"
SCREEN_W, SCREEN_H = 1440, 900

# 探针取自各 Skill 正文里独有的标题行：命中即证明该 Skill 正文进了这一轮请求。
PROBE_NOTE = "随手记 (Memory Agent)"
PROBE_REPORT = "周报起草 (Memory Report)"
PROBE_SEARCH = "联网检索 (AnySearch)"

NOTE_PROMPT = "synthetic note probe od108 no weekly keyword"
REPORT_MARKER = "整理成周报"


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"Xvfb {display} 启动失败")
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError(f"Xvfb {display} 未就绪")


def start_dbus() -> tuple[str, subprocess.Popen]:
    proc = subprocess.Popen(
        ["dbus-daemon", "--session", "--print-address", "--nofork"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    assert proc.stdout is not None
    return proc.stdout.readline().strip(), proc


def seed_dsh_home(root: Path) -> None:
    """隔离 DSH home：只放最小 profile 清单与仓库 skills，不复制用户实例。"""
    target = root / "dsh-home" / "profiles" / "discipline"
    target.mkdir(parents=True, exist_ok=True)
    shutil.copy(
        REPO / "samples" / "profiles" / "discipline.profile.json", target / "package.json"
    )
    patch = REPO / "samples" / "profiles" / "discipline.profile.patch.yml"
    if patch.is_file():
        shutil.copy(patch, target / "cordis.patch.yml")
    skills = root / "dsh-home" / "skills"
    if not skills.exists():
        shutil.copytree(REPO / "skills", skills)


def seed_profile_assets(session: AppSession) -> tuple[Path, Path]:
    """预置 discipline 的场景声明与规则包（OD-ARCH-106 起宿主不再自动创建）。"""
    prof = session.app_data / "profiles" / "discipline"
    prof.mkdir(parents=True, exist_ok=True)
    decl = prof / "decl.yml"
    shutil.copy(REPO / "samples" / "profiles" / "discipline.yml", decl)
    rules = prof / "rules.json"
    shutil.copy(REPO / "samples" / "profiles" / "discipline.rules.json", rules)
    return decl, rules


def write_settings(session: AppSession) -> Path:
    path = session.app_data / "config" / "settings.v1.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.settings.v1",
                "engine_profile": "omnidesk-base",
                "model_permissions": {"local": True, "private": False, "public": False},
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def app_nodes(pid: int | None) -> list[ax.Node]:
    """只读本次 App 进程的无障碍树，避免命中宿主桌面上的同名应用。"""
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


def find(nodes: list[ax.Node], needle: str, role: str | None = None):
    for node in nodes:
        if role is not None and node.role != role:
            continue
        if any(needle in h for h in (node.name, node.text)):
            return node
    return None


def find_button(nodes: list[ax.Node], needle: str):
    """WebKit 把 button 元素暴露成 button，AT-SPI 主题有时报 push button。"""
    return find(nodes, needle, role="button") or find(nodes, needle, role="push button")


def all_text(nodes: list[ax.Node]) -> str:
    return "\n".join(n.name + "\n" + n.text for n in nodes)


def wait_for(predicate, timeout: float, interval: float = 0.6):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(interval)
    return None


def dump_tree(nodes: list[ax.Node], path: Path) -> None:
    rows = [{"role": n.role, "name": n.name, "text": n.text} for n in nodes]
    path.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":131")
    ap.add_argument("--root", default="/tmp/od108-flow")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 合成模型端点（非真实模型）",
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "root": str(root),
        "probes": {"note": PROBE_NOTE, "report": PROBE_REPORT, "search": PROBE_SEARCH},
        "checks": {},
        "failures": [],
    }

    def check(name: str, ok: bool, measured=None, expected=None, desc: str = "") -> None:
        facts["checks"][name] = {
            "pass": bool(ok),
            "measured": measured,
            "expected": expected,
            "desc": desc,
        }
        if not ok:
            facts["failures"].append(f"{name}: measured={measured} expected={expected} ({desc})")
            print(f"FAIL [FLOW-108] {name}: {measured} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [FLOW-108] {name}: {measured}")

    xvfb = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr

    import socket

    with socket.socket() as s:
        s.settimeout(0.5)
        port_open = s.connect_ex(("127.0.0.1", 5273)) == 0

    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=port_open,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "discipline",
            # 库根必须隔离：随手记落盘会写到库根，漏了这条就碰到用户真实记录。
            "OMNIDESK_KNOWLEDGE_ROOT": str(root / "local-data"),
        },
    )
    app_pid = None
    try:
        session.prepare()
        seed_dsh_home(root)
        session.start_mock()
        # 「整理成周报」也当成探针：它只可能来自产品自己的预填路径，
        # 用来证明周报那一轮确实是「去生成周报」送出去的，而不是脚本自己敲的。
        session.mock_ctl.set(probes=[PROBE_NOTE, PROBE_REPORT, PROBE_SEARCH, REPORT_MARKER])
        write_settings(session)
        seed_profile_assets(session)

        session.start()
        app_pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            raise RuntimeError("主窗口未在规定时间内出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(8)
        ax.init()
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "01-home-ax.json")
        xs.shot(EVID / "01-home.png")

        def scene_to_composer() -> bool:
            ns = app_nodes(app_pid)
            dock = find(ns, "工作记录")
            if dock is None:
                return False
            ax.invoke(dock)
            time.sleep(3.0)
            go = find(app_nodes(app_pid), "去对话")
            if go is None:
                return False
            ax.invoke(go)
            time.sleep(4.0)
            return find(app_nodes(app_pid), "与 OmniDesk 协同探索") is not None

        def send(text: str | None) -> bool:
            if text is not None:
                box = find(app_nodes(app_pid), "与 OmniDesk 协同探索")
                if box is not None:
                    ax.invoke(box)
                time.sleep(0.6)
                xs.type_ascii(text, settle=0.03)
                time.sleep(0.6)
            btn = find_button(app_nodes(app_pid), "发送指令")
            if btn is None:
                return False
            return bool(ax.invoke(btn))

        def request_count() -> int:
            return len(session.mock_ctl.new_requests())

        def new_request(baseline: int):
            """取本轮提交后到达合成端点的第一条请求。

            为什么不按关键词匹配 `last_user`：合成端点只保留前 200 字符，而场景提示词
            是「技能正文在前、用户原话在末尾」，用户在末尾，早被截掉了。按请求序号取
            更可靠：提交前记下条数，提交后第一条新请求就是这一轮。
            """

            def look():
                reqs = session.mock_ctl.new_requests()
                return reqs[baseline] if len(reqs) > baseline else None

            return wait_for(look, timeout=150)

        # --- 第 1 轮：随手记（ASCII 合成句，不含「周报」） ---------------------
        reached = scene_to_composer()
        check("composer_reached", reached, reached, True, "进入工作记录对话输入区")
        if not reached:
            raise RuntimeError("未能进入工作记录对话，见 01-home-ax.json")
        xs.shot(EVID / "02-note-composer.png")
        note_baseline = request_count()
        sent_note = send(NOTE_PROMPT)
        check("note_prompt_sent", sent_note, sent_note, True, "提交一条随手记合成输入")
        if not sent_note:
            raise RuntimeError("找不到发送按钮（随手记轮）")
        note_req = new_request(note_baseline)
        check(
            "note_request_reached_model",
            note_req is not None,
            note_req is not None,
            True,
            "合成端点收到随手记这一轮请求",
        )
        if note_req is None:
            raise RuntimeError("随手记轮没有到达模型，见 logs/app.log")
        note_hits = note_req.get("probe_hits") or []
        facts["note_request"] = {
            "text_chars": note_req.get("text_chars"),
            "probe_hits": note_hits,
            "probe_misses": note_req.get("probe_misses"),
        }
        check(
            "note_injects_memory_agent",
            PROBE_NOTE in note_hits,
            note_hits,
            PROBE_NOTE,
            "随手记轮注入了 memory-agent 正文",
        )
        check(
            "note_excludes_memory_report",
            PROBE_REPORT not in note_hits,
            note_hits,
            "不含 " + PROBE_REPORT,
            "随手记轮不带周报技能",
        )
        check(
            "note_excludes_anysearch",
            PROBE_SEARCH not in note_hits,
            note_hits,
            "不含 " + PROBE_SEARCH,
            "随手记轮不带按需技能（模型自己决定是否读）",
        )
        facts["note_prompt_echoed"] = NOTE_PROMPT in (
            note_req.get("body_probe_hits") or []
        ) or "synthetic" in (note_req.get("last_user") or "")

        wait_for(lambda: find_button(app_nodes(app_pid), "发送指令"), timeout=180)
        time.sleep(2.0)

        # --- 第 2 轮：周报（产品自己的「去生成周报」预填路径） -----------------
        back = find(app_nodes(app_pid), "工作记录")
        check(
            "dock_reachable_for_round2",
            back is not None,
            back is not None,
            True,
            "回到 Dock 上的工作记录入口",
        )
        if back is not None:
            ax.invoke(back)
            time.sleep(3.5)
        report_btn = find_button(app_nodes(app_pid), "生成周报")
        if report_btn is None:
            report_btn = find_button(app_nodes(app_pid), "重新生成")
        dump_tree(app_nodes(app_pid), EVID / "03-weekly-page-ax.json")
        xs.shot(EVID / "03-weekly-page.png")
        check(
            "report_action_present",
            report_btn is not None,
            report_btn is not None,
            True,
            "场景页有「去生成周报」动作",
        )
        if report_btn is None:
            raise RuntimeError("场景页没有周报动作按钮，见 03-weekly-page-ax.json")
        ax.invoke(report_btn)
        time.sleep(5.0)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "04-report-composer-ax.json")
        xs.shot(EVID / "04-report-composer.png")
        # WebKit 不把 input 的当前值放进无障碍树（entry 只有 placeholder），
        # 所以这里不按 AX 文本断言预填，改为在下一段用请求探针证明：
        # 「整理成周报」出现在模型请求里，且它只可能来自产品自己的预填。
        facts["prefill_visible_in_ax"] = REPORT_MARKER in all_text(nodes)
        facts["prefill_note"] = (
            "WebKit 的 entry 只暴露 placeholder，预填值不在无障碍树里；"
            "预填是否生效由请求探针 report_prompt_from_product_prefill 判定，截图 04 为人工可核对的补充证据。"
        )
        report_baseline = request_count()
        sent_report = send(None)
        check("report_prompt_sent", sent_report, sent_report, True, "提交预填的周报指令")
        if not sent_report:
            raise RuntimeError("找不到发送按钮（周报轮）")
        report_req = new_request(report_baseline)
        check(
            "report_request_reached_model",
            report_req is not None,
            report_req is not None,
            True,
            "合成端点收到周报这一轮请求",
        )
        if report_req is None:
            raise RuntimeError("周报轮没有到达模型，见 logs/app.log")
        rep_hits = report_req.get("probe_hits") or []
        facts["report_request"] = {
            "text_chars": report_req.get("text_chars"),
            "probe_hits": rep_hits,
            "probe_misses": report_req.get("probe_misses"),
        }
        check(
            "report_prompt_from_product_prefill",
            REPORT_MARKER in rep_hits,
            rep_hits,
            REPORT_MARKER,
            "周报请求里含「整理成周报」，说明走的是产品「去生成周报」预填，不是脚本拼的输入",
        )
        check(
            "report_injects_memory_report",
            PROBE_REPORT in rep_hits,
            rep_hits,
            PROBE_REPORT,
            "周报轮注入了 memory-report 正文",
        )
        check(
            "report_excludes_memory_agent",
            PROBE_NOTE not in rep_hits,
            rep_hits,
            "不含 " + PROBE_NOTE,
            "周报轮不再背上随手记技能",
        )
        check(
            "report_excludes_anysearch",
            PROBE_SEARCH not in rep_hits,
            rep_hits,
            "不含 " + PROBE_SEARCH,
            "周报轮不带按需技能",
        )

        facts["injected_chars"] = {
            "note": (facts.get("note_request") or {}).get("text_chars"),
            "report": (facts.get("report_request") or {}).get("text_chars"),
        }
        facts["app_alive"] = session.alive()
        check("app_alive_at_end", session.alive(), session.alive(), True, "验收结束时 App 仍存活")
    finally:
        facts["session_log"] = list(session.log)
        facts["app_stop"] = session.stop_app()
        session.stop_all()
        try:
            bus_proc.terminate()
        except Exception:
            pass
        xvfb.terminate()

    facts["result"] = "pass" if not facts["failures"] else "fail"
    (EVID / "flow-injection.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(
        json.dumps(
            {"checks": facts["checks"], "result": facts["result"]}, ensure_ascii=False, indent=2
        )
    )
    return 0 if facts["result"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
