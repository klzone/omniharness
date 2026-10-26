#!/usr/bin/env python3
"""OD-ARCH-088 真实窗口验收：提问卡已答 / 取消两条路径 + 场景 intent + 隐私。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI + 合成模型端点）。
合成端点不是真实模型；所有文本均为虚构标记，不接触任何真实业务材料。

验证链：
  1. 进入「工作记录」场景新开对话，提交一条合成指令；
  2. 合成端点返回 ask_user_question 工具调用 → 引擎发 user-questions/request；
     界面出现提问卡（题干 / 选项 / 说明），且没有自动提交；
  3. 点选项 → 点「提交回答」→ 卡片变「已提交」，合成端点下一次请求带 tool 消息；
  4. 该任务状态文件 intent 为 weekly_report（不再回落 material_writing）；
  5. 新对话再触发一次提问 → 点「取消本次任务」→ 卡片落成已失效（引擎撤回/任务结束）；
  6. 隐私：任务状态 JSON 与 app.log 不含题干、说明、选项或自定义答案正文。

用法：python3 verify_ui_088.py [--display :122] [--root /tmp/od088-ui]
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

EVID = HERE / "evidence" / "od-arch-088"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900

Q_TEXT = "合成测试：是否落盘？"
Q_DETAIL = "合成补充说明"
Q_HEADER = "合成确认"
OPT_A = "先追加"
OPT_A_DESC = "合成选项甲"
OPT_B = "稍后"
OPT_B_DESC = "合成选项乙"
PROMPT = "synthetic weekly note please confirm"
PROMPT_2 = "synthetic second question please confirm OD088-2"
CARD_TITLE = "引擎需要你回答一个问题"


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


def find(nodes: list[ax.Node], name: str, role: str | None = None, contains: bool = True):
    for node in nodes:
        if role is not None and node.role != role:
            continue
        haystacks = (node.name, node.text)
        if any((name in h) if contains else (name == h) for h in haystacks):
            return node
    return None


def find_button(nodes: list[ax.Node], name: str):
    """WebKit 把 <button> 暴露成 `button`，AT-SPI 主题有时报 `push button`。"""
    return find(nodes, name, role="button", contains=True) or find(
        nodes, name, role="push button", contains=True
    )


def all_text(nodes: list[ax.Node]) -> str:
    return "\n".join(n.name + "\n" + n.text for n in nodes)


def question_cards(nodes: list[ax.Node], title: str = CARD_TITLE) -> list[ax.Node]:
    """提问卡是 role=group、aria-label 合并了题面的节点（WebKit 不暴露卡片内段落）。"""
    # aria-label 的 role=group 在 WebKitGTK 的无障碍树里报成 panel。
    return [n for n in nodes if n.role in ("group", "panel") and title in (n.name or "")]


def dump_tree(nodes: list[ax.Node], path: Path) -> None:
    rows = []
    for n in nodes:
        try:
            from gi.repository import Atspi

            e = n.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
            box = {"x": e.x, "y": e.y, "w": e.width, "h": e.height}
        except Exception:
            box = None
        rows.append({"role": n.role, "name": n.name, "text": n.text, "box": box})
    path.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")


def wait_for(predicate, timeout: float, interval: float = 0.75):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(interval)
    return None


def write_v1_rules(session: AppSession) -> Path:
    """只写一次性隔离根里的旧版规则包（schema v1、无 skill_bindings）。"""
    path = session.app_data / "profiles" / "discipline.rules.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.profile-rules.v1",
                "profile": "discipline",
                "global": {},
                "scenes": {},
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":122")
    ap.add_argument("--root", default="/tmp/od088-ui")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 目录: {root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    script_sha = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    facts: dict = {
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 合成模型端点（非真实模型）",
        "script_sha256": script_sha,
        "root": str(root),
        "checks": {},
        "failures": [],
    }

    def check(name: str, ok: bool, measured=None, threshold=None, desc: str = "") -> None:
        passed = bool(ok)
        facts["checks"][name] = {
            "pass": passed,
            "measured": measured,
            "threshold": threshold,
            "desc": desc,
        }
        if not passed:
            facts["failures"].append(f"{name}: measured={measured} threshold={threshold} ({desc})")
            print(f"FAIL [UI-088] {name}: {measured} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [UI-088] {name}: {measured}")

    shutil.copytree(
        SOURCE_HOME,
        root / "dsh-home",
        dirs_exist_ok=True,
        symlinks=True,
        ignore=shutil.ignore_patterns("sessions*", "session_*", "*ledger.json"),
    )
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
        # 本脚本自带一次性 Xvfb；复用 5273 上并不存在的 dev server 只会等到超时。
        # 让 AppSession 启动并回收同仓库 dev server，保证前端就是当前工作区源码。
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            # 库根必须一起隔离：工作记录落在库根而不是成果目录。漏了这条，
            # 合成指令会被 worklog_append 写进用户真实的工作记录文件。
            "OMNIDESK_KNOWLEDGE_ROOT": str(root / "local-data"),
        },
    )
    app_pid = None
    try:
        session.prepare()
        session.start_mock()
        # 必须在 App 启动前设置：首轮模型请求就要返回合成提问。
        session.mock_ctl.set(
            mode="tool_call",
            tool_call_id="call-synth-question-088",
            tool_call_name="ask_user_question",
            tool_call_arguments={
                "questions": [
                    {
                        "id": "q1",
                        "header": Q_HEADER,
                        "question": Q_TEXT,
                        "detail": Q_DETAIL,
                        "options": [
                            {"label": OPT_A, "description": OPT_A_DESC},
                            {"label": OPT_B, "description": OPT_B_DESC},
                        ],
                    }
                ]
            },
            tool_call_followup=["合成", "继续", "完成"],
            # 同一会话已有工具结果时，只有带该标记的新指令才再触发一次合成提问。
            tool_call_trigger="OD088-2",
        )
        write_settings(session)
        rules_path = write_v1_rules(session)

        session.start()
        app_pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=150)
        if win is None:
            raise RuntimeError("主窗口未在规定时间内出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(8)
        ax.init()
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "01-home-ax.json")
        xs.shot(EVID / "01-home.png")

        # --- 进入工作记录场景 ------------------------------------------------
        dock = find(nodes, "工作记录", contains=True)
        if dock is not None:
            ax.invoke(dock)
            time.sleep(3.0)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "02-scene-ax.json")
        xs.shot(EVID / "02-scene.png")
        go = find(nodes, "去对话", contains=True)
        check("scene_page_reached", go is not None, go is not None, True, "进入工作记录场景页")
        if go is not None:
            ax.invoke(go)
            time.sleep(4.0)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "03-composer-ax.json")
        xs.shot(EVID / "03-composer.png")

        # S4-C：场景有预设但旧版规则包没有 skill_bindings 时，对话区给一条低干扰说明。
        # 只断言「未预注入」与「技能不可用」被明确区分，不要求某个具体措辞位置。
        composer_text = all_text(nodes)
        check(
            "skill_not_injected_hint_visible",
            "未预注入" in composer_text and "不代表技能不可用" in composer_text,
            "未预注入" in composer_text and "不代表技能不可用" in composer_text,
            True,
            "对话区显示 Skill 未预注入说明，并区分「技能不可用」",
        )

        # 旧版规则包 → 未预注入说明（S4-C 的低干扰提示，不是错误）。
        check(
            "legacy_rules_loaded",
            rules_path.is_file(),
            str(rules_path),
            "v1 规则包写入隔离根",
            "隔离根规则包为 schema v1（无 skill_bindings）",
        )

        def send_prompt(text: str = PROMPT) -> bool:
            nodes_now = app_nodes(app_pid)
            box = find(nodes_now, "与 OmniDesk 协同探索", contains=True)
            if box is not None:
                ax.invoke(box)
            time.sleep(0.6)
            xs.type_ascii(text, settle=0.03)
            time.sleep(0.6)
            send = find_button(app_nodes(app_pid), "发送指令")
            if send is None:
                return False
            return bool(ax.invoke(send))

        sent = send_prompt()
        check("prompt_sent", sent, sent, True, "在合成对话里提交一条指令")
        if not sent:
            raise RuntimeError("找不到可用的发送按钮")

        card = wait_for(lambda: question_cards(app_nodes(app_pid)) or None, timeout=120)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "04-question-ax.json")
        xs.shot(EVID / "04-question-card.png")
        check(
            "question_card_visible",
            bool(card),
            len(card or []),
            ">=1",
            "提问卡出现在对话流",
        )
        if card is None:
            raise RuntimeError("提问卡未出现，见 app.log 与 04-question-ax.json")
        check("question_text_visible", Q_TEXT in all_text(nodes), Q_TEXT in all_text(nodes), True, "题干显示")
        # 文件事实：DSH 的 tool-ask-user（legacy 与 timed 两套 schema）只转发
        # id/question/header/options/multi_select，不转发 detail。合成端点即使给出
        # detail，引擎也不会送到外壳；卡片组件仍支持 detail（适配层单测覆盖）。
        # 因此这里记录运行时的真实行为，不把「没收到 detail」误判成卡片缺陷。
        check(
            "question_header_visible",
            Q_HEADER in all_text(nodes),
            Q_HEADER in all_text(nodes),
            True,
            "题目 header 显示",
        )
        facts["detail_delivered_by_runtime"] = Q_DETAIL in all_text(nodes)
        facts["detail_note"] = (
            "DSH tool-ask-user 两套 schema 都不转发 detail；窗口证据只能验证 header/题干，"
            "detail 渲染由适配层单测与组件代码覆盖。"
        )
        check("option_a_visible", OPT_A in all_text(nodes), OPT_A in all_text(nodes), True, "选项甲显示")
        check("option_b_visible", OPT_B in all_text(nodes), OPT_B in all_text(nodes), True, "选项乙显示")
        check(
            "card_shows_not_auto",
            "请选择后提交" in all_text(nodes),
            "请选择后提交" in all_text(nodes),
            True,
            "卡片明确说明需要用户选择后提交，不自动选择",
        )

        # 还没有选择时提交按钮应不可用：通过无障碍 states 是否 enabled 判定。
        submit_before = find_button(nodes, "提交回答")
        submit_enabled_before = None
        submit_states_before = None
        if submit_before is not None:
            try:
                from gi.repository import Atspi

                states = submit_before.accessible.get_state_set()
                # GTK 的 disabled 表现为 SENSITIVE=False；ENABLED 对某些主题仍可能为 True。
                submit_states_before = {
                    "enabled": bool(states.contains(Atspi.StateType.ENABLED)),
                    "sensitive": bool(states.contains(Atspi.StateType.SENSITIVE)),
                }
                submit_enabled_before = submit_states_before["sensitive"]
            except Exception:
                submit_enabled_before = None
        check(
            "submit_disabled_without_choice",
            submit_enabled_before is False,
            submit_states_before,
            False,
            "未选择时提交按钮禁用（AT-SPI SENSITIVE 状态）",
        )

        # --- 选择 + 提交 -----------------------------------------------------
        option = find(nodes, OPT_A, role="radio button", contains=True)
        check("option_clickable", option is not None, option is not None, True, "找到可选中的选项甲")
        if option is not None:
            ax.invoke(option)
            time.sleep(1.5)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "05-selected-ax.json")
        xs.shot(EVID / "05-selected.png")
        # 点选项后不应自动提交：仍应能点到「提交回答」，且尚未出现已答文案。
        check(
            "select_does_not_auto_submit",
            "已提交，引擎已收到回答。" not in all_text(nodes),
            "已提交，引擎已收到回答。" in all_text(nodes),
            False,
            "点选项只改变本地选择",
        )

        submit = find_button(nodes, "提交回答")
        check("submit_button_present", submit is not None, submit is not None, True, "提交回答按钮存在")
        if submit is not None:
            ax.invoke(submit)
        answered = wait_for(
            lambda: "已提交，引擎已收到回答。" in all_text(app_nodes(app_pid)),
            timeout=60,
        )
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "06-answered-ax.json")
        xs.shot(EVID / "06-answered.png")
        check("card_answered", answered is not None, answered is not None, True, "宿主确认后卡片变已答")

        reqs = session.mock_ctl.new_requests()
        tool_reqs = [r for r in reqs if r.get("has_tool_message")]
        facts["mock_requests"] = [
            {"has_tool_message": r.get("has_tool_message"), "text_chars": r.get("text_chars")}
            for r in reqs
        ]
        check(
            "answer_reached_engine",
            len(tool_reqs) > 0,
            len(tool_reqs),
            ">=1",
            "第二次模型请求携带 tool 消息（答案已作为工具结果回到引擎）",
        )

        # --- intent 落盘 -----------------------------------------------------
        state_dir = session.app_data / "engine-state"

        def read_states() -> list[dict]:
            out = []
            for p in sorted(state_dir.glob("*.json")):
                try:
                    out.append(json.loads(p.read_text(encoding="utf-8")))
                except Exception:
                    continue
            return out

        states = wait_for(lambda: read_states() or None, timeout=30) or []
        intents = {s.get("omni_task_id"): s.get("intent") for s in states}
        facts["state_intents"] = intents
        check(
            "intent_weekly_report",
            any(v == "weekly_report" for v in intents.values()),
            intents,
            "weekly_report",
            "工作记录场景提交的任务 intent 为 weekly_report",
        )

        # --- 取消路径 --------------------------------------------------------
        wait_for(lambda: find_button(app_nodes(app_pid), "发送指令"), timeout=90)
        new_thread = find(app_nodes(app_pid), "新对话", contains=True)
        if new_thread is not None:
            ax.invoke(new_thread)
            time.sleep(3.0)
        sent2 = send_prompt(PROMPT_2)
        check("second_prompt_sent", sent2, sent2, True, "新对话里再提交一条合成指令")
        def answerable_card():
            ns = app_nodes(app_pid)
            if find_button(ns, "提交回答") is None:
                return None
            cards = question_cards(ns)
            return cards or None

        card2 = wait_for(answerable_card, timeout=120)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "07-second-question-ax.json")
        xs.shot(EVID / "07-second-question.png")
        check(
            "second_question_visible",
            bool(card2),
            len(card2 or []),
            ">=1 且带提交按钮",
            "第二次提问卡出现且可答",
        )
        cancel = find_button(nodes, "取消本次任务")
        check("cancel_button_present", cancel is not None, cancel is not None, True, "卡片提供取消出口")
        if cancel is not None:
            ax.invoke(cancel)

        def closed_text() -> str | None:
            text = all_text(app_nodes(app_pid))
            if "该提问已失效" in text or "任务已结束，该提问不再可答" in text:
                return text
            return None

        closed = wait_for(closed_text, timeout=90)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "08-closed-ax.json")
        xs.shot(EVID / "08-closed.png")
        check("question_closed_after_cancel", closed is not None, closed is not None, True, "取消后卡片落成已失效")
        if closed:
            facts["closed_copy"] = (
                "该提问已失效" if "该提问已失效" in closed else "任务已结束，该提问不再可答"
            )
        # 取消路径下卡片不再提供提交按钮。
        check(
            "closed_card_not_answerable",
            find_button(nodes, "提交回答") is None,
            find_button(nodes, "提交回答") is not None,
            False,
            "已关闭的卡片不再可答",
        )

        # --- 隐私断言 --------------------------------------------------------
        markers = [Q_TEXT, Q_DETAIL, OPT_A, OPT_A_DESC, OPT_B, OPT_B_DESC]
        scanned = []
        leaks = []
        paths = list(state_dir.glob("*.json")) + list((root / "logs").glob("*.log"))
        for path in paths:
            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except Exception:
                continue
            scanned.append(str(path.relative_to(root)))
            for marker in markers:
                if marker and marker in text:
                    leaks.append({"file": path.name, "marker": marker})
        facts["privacy"] = {"scanned": scanned, "leaks": leaks, "markers": markers}
        check(
            "no_question_text_in_state_or_logs",
            not leaks,
            leaks,
            [],
            "任务状态 JSON 与 app.log 不含题干 / 选项 / 答案正文",
        )
        check(
            "state_files_scanned",
            any(p.suffix == ".json" for p in state_dir.glob("*.json")),
            len(list(state_dir.glob("*.json"))),
            ">=1",
            "至少有一个任务状态文件参与隐私扫描",
        )

        # --- S4-C：设置页「规则资产」旧版提示 --------------------------------
        settings_btn = find(app_nodes(app_pid), "系统偏好设置", role="button")
        check("settings_entry_visible", settings_btn is not None, settings_btn is not None, True,
              "顶栏设置入口可见")
        if settings_btn is not None:
            ax.invoke(settings_btn)
            time.sleep(3.0)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "09-settings-ax.json")
        xs.shot(EVID / "09-settings.png")
        rules_tab = find(nodes, "规则资产", role="page tab", contains=False)
        check("rules_tab_visible", rules_tab is not None, rules_tab is not None, True,
              "设置页存在「规则资产」分页")
        if rules_tab is not None:
            ax.invoke(rules_tab)

        def legacy_notice() -> str | None:
            text = all_text(app_nodes(app_pid))
            if "当前规则包是旧版" in text and "未声明场景 Skill 绑定" in text:
                return text
            return None

        notice = wait_for(legacy_notice, timeout=45)
        nodes = app_nodes(app_pid)
        dump_tree(nodes, EVID / "10-rules-legacy-ax.json")
        xs.shot(EVID / "10-rules-legacy.png")
        check(
            "legacy_rules_notice_visible",
            notice is not None,
            notice is not None,
            True,
            "v1 规则包下设置页明确提示未启用场景 Skill 预注入",
        )
        check(
            "legacy_notice_keeps_skills_available_copy",
            notice is not None and "模型仍可按需" in notice,
            notice is not None and "模型仍可按需" in notice,
            True,
            "提示明确技能仍可按需加载，不等同技能不可用",
        )
        facts["rules_file_after_ui"] = rules_path.read_text(encoding="utf-8")
        check(
            "legacy_rules_file_unchanged_by_ui",
            json.loads(facts["rules_file_after_ui"]).get("schema") == "omnidesk.profile-rules.v1",
            facts["rules_file_after_ui"],
            "schema=omnidesk.profile-rules.v1",
            "查看提示不会静默改写用户规则包",
        )

        # 最后确认应用仍存活（不是崩溃后被截图误判）。
        check("app_alive_at_end", session.alive(), session.alive(), True, "验收结束时 App 仍在运行")
    except Exception as exc:  # noqa: BLE001
        facts["error"] = f"{type(exc).__name__}: {exc}"
        check("scenario_completed", False, facts["error"], None, "端到端流程未走完")
        try:
            nodes = app_nodes(app_pid)
            dump_tree(nodes, EVID / "99-failure-ax.json")
            XSession(args.display).shot(EVID / "99-failure.png")
        except Exception:
            pass
    finally:
        try:
            session.stop_all()
        except Exception:
            pass
        for proc in (bus_proc, xvfb):
            try:
                proc.terminate()
                proc.wait(timeout=10)
            except Exception:
                try:
                    proc.kill()
                except Exception:
                    pass

    failed = facts["failures"]
    facts["checks_passed"] = sum(1 for c in facts["checks"].values() if c["pass"])
    facts["checks_total"] = len(facts["checks"])
    facts["result"] = "pass" if not failed else "fail"
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"checks={facts['checks_total']} passed={facts['checks_passed']} failed={len(failed)}")
    print(f"evidence={EVID}")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
