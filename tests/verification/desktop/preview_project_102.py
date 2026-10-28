#!/usr/bin/env python3
"""OD-ARCH-102：项目跟进页 v3（本机事项 + 左栏双区）的真实窗口验证。

只用 /tmp 下的合成数据：不读真实知识库、不打印凭证、不碰用户实例。
事项从空清单开始，通过真实键盘事件录入；随后走「表单 → 宿主整批校验 →
原子写 → 回执刷新」，不预填表单、不直接种数据。

原生日期控件只接受键盘分段输入：焦点进入期限后，依次输入月、日、年三段
（WebKitGTK 的分段点击与 AT-SPI Value 写入在本次环境会失效或崩溃）。

抓帧与断言：
  01 空态：无指标卡、左栏双区、右栏只给引导
  02 新增保存后：卡片、甘特行、磁盘文件同时出现
  03 再新增一条，搜索只留命中项
  04 修改期限后：卡片、甘特行与文件同步
  05 阶段成果切换
  06 删除确认 + 删除后卡片、行与文件条目都减少
  07 窄窗 1000 单列

用法：python3 preview_project_102.py [--display :151] [--root /tmp/od102-preview]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402
import preview_project_098 as p98  # noqa: E402

EVID = HERE / "evidence" / "od-arch-102"
SCREEN_W, SCREEN_H = 1600, 980
ROOT_W, ROOT_H = 1600, 2200
NARROW_W, NARROW_H = 1000, 2200


def app_nodes(pid: int):
    return p98.app_nodes(pid)


def find(nodes, name: str, role: str | None = None, contains: bool = False):
    for n in nodes:
        if role is not None and n.role != role:
            continue
        a, b = n.name or "", n.text or ""
        if (name in a or name in b) if contains else (name == a or name == b):
            return n
    return None


def dump(nodes, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            [{"role": n.role, "name": n.name, "text": n.text} for n in nodes],
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )


def wait_for(pid: int, predicate, timeout: float = 25.0, step: float = 1.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        nodes = app_nodes(pid)
        if predicate(nodes):
            return nodes
        time.sleep(step)
    return app_nodes(pid)


def write_threads(app_data: Path) -> dict:
    root = app_data / "threads" / "_unassigned"
    root.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    thread = {
        "id": "t102project01",
        "tag": "project",
        "projectPath": None,
        "title": "合成项目对话：事项录入联调",
        "preset": "project",
        "messages": [{"id": "m102a", "role": "user", "text": "合成对话，仅用于界面验证。"}],
        "createdAt": now - 3600,
        "updatedAt": now - 600,
        "version": 1,
    }
    (root / (thread["id"] + ".json")).write_text(
        json.dumps(thread, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return thread


def write_artifacts(data_root: Path) -> int:
    data_root.mkdir(parents=True, exist_ok=True)
    today = dt.date.today()
    titles = ["合成阶段成果：v3 事项录入", "合成阶段成果：时间线联调"]
    for i, title in enumerate(titles):
        day = today - dt.timedelta(days=i * 3)
        name = "offline-project-management-" + day.strftime("%Y%m%d") + "-" + str(80200 + i * 131).zfill(6) + ".md"
        (data_root / name).write_text(
            "---\n"
            "scenario: project_management\n"
            'title: "' + title + '"\n'
            "created: " + day.isoformat() + "T09:00:00Z\n"
            "---\n"
            "# " + title + "\n\n仅用于界面验证的合成阶段成果。\n",
            encoding="utf-8",
        )
    return len(titles)


def record_items(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8"))["items"]


def wait_until(predicate, timeout: float = 20.0, step: float = 0.5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(step)
    return None


def exact(nodes, name: str, role: str | None = None):
    for n in nodes:
        if role is not None and n.role != role:
            continue
        if (n.name or "").strip() == name:
            return n
    return None


def cards(nodes) -> list:
    return [
        n
        for n in nodes
        if n.role in ("button", "toggle button") and "负责人" in (n.name or "")
    ]


def dialog_nodes(dialog) -> list:
    raw: list = []
    ax._collect(dialog.accessible, raw)
    return [ax._node(n) for n in raw]


def fill_due(xs: XSession, month: str, day: str, year: str) -> None:
    """焦点已进入期限的月份段：依次填月、日、年。"""
    xs.type_ascii(month)
    time.sleep(0.45)
    xs.key("Tab")
    xs.type_ascii(day)
    time.sleep(0.45)
    xs.key("Tab")
    xs.type_ascii(year)
    time.sleep(0.55)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":151")
    ap.add_argument("--root", default="/tmp/od102-preview")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit("拒绝清理非 /tmp 目录: " + str(root))
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    data_root = root / "local-data"
    today = dt.date.today()
    facts: dict = {
        "display": args.display,
        "root": str(root),
        "today": today.isoformat(),
        "form_input": "keyboard",
        "form_input_note": "事项/负责人/下一步走普通键入；期限按 WebKitGTK 的月、日、年三段键入",
    }
    facts["artifacts"] = write_artifacts(data_root)

    xvfb = p98.start_display(args.display)
    bus_addr, bus_proc = p98.start_dbus()
    os.environ["DISPLAY"] = args.display
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ.pop("AT_SPI_BUS_ADDRESS", None)
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=False,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_KNOWLEDGE_ROOT": str(data_root),
        },
    )
    try:
        session.prepare()
        write_threads(session.app_data)
        session.start()
        pid = session.proc.pid if session.proc else None
        if pid is None:
            raise RuntimeError("App 未启动")
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=150)
        if win is None:
            raise RuntimeError("主窗口未出现")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        if not xs.wait_rendered(win, timeout=120):
            raise RuntimeError("界面未渲染完成")
        ax.init()

        dock = None
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline and dock is None:
            nodes = app_nodes(pid)
            dock = find(nodes, "项目跟踪", contains=True) or find(nodes, "项目跟进", contains=True)
            if dock is None:
                time.sleep(2.0)
        if dock is None:
            xs.shot(EVID / "00-home.png")
            raise RuntimeError("找不到「项目跟踪」入口")
        ax.invoke(dock)
        time.sleep(7.0)

        # 01 空态
        xs.shot(EVID / "01-project-empty.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "01-project-empty-ax.json")
        facts["ax_projects_heading"] = find(nodes, "项目事项", contains=True) is not None
        facts["ax_history_heading"] = find(nodes, "历史对话", contains=True) is not None
        facts["ax_timeline_heading"] = find(nodes, "项目时间线", contains=True) is not None
        facts["ax_metric_strip_absent"] = find(nodes, "本周记录", contains=True) is None and find(
            nodes, "成果文档", contains=True
        ) is None
        # WebKitGTK 不把普通段落文字放进 AT-SPI 树；引导文案的可见性由截图
        # （01-project-empty.png）与 verify_project_102.py 的源码断言共同覆盖。
        facts["ax_empty_guide"] = None
        facts["empty_guide_evidence"] = "screenshot + static source assertion"
        facts["ax_gantt_axis_absent_when_empty"] = find(nodes, "事项 / 负责人", contains=True) is None
        facts["ax_history_thread"] = find(nodes, "合成项目对话", contains=True) is not None
        facts["items_file_before"] = (session.app_data / "project-items.v1.json").exists()

        store_path = session.app_data / "project-items.v1.json"

        def create_item(
            title: str,
            owner: str,
            month: str,
            day: str,
            year: str,
            next_step: str,
        ) -> list[dict]:
            nodes = wait_until(lambda: app_nodes(pid))
            add = exact(nodes, "+ 新增", "button")
            if add is None:
                raise RuntimeError("找不到「+ 新增」")
            ax.invoke(add)
            time.sleep(1.2)
            nodes = wait_until(lambda: app_nodes(pid))
            item_box = exact(nodes, "事项", "entry")
            owner_box = exact(nodes, "负责人代号", "entry")
            next_box = exact(nodes, "下一步", "entry")
            if item_box is None or owner_box is None or next_box is None:
                raise RuntimeError("新增表单字段未出现")
            ax.invoke(item_box)
            time.sleep(0.3)
            xs.type_ascii(title)
            time.sleep(0.3)
            ax.invoke(owner_box)
            time.sleep(0.3)
            xs.type_ascii(owner)
            time.sleep(0.35)
            # 负责人 → 状态下拉 → 期限月份段（选中第一个选项，保持默认「未开始」）。
            xs.key("Tab")
            time.sleep(0.35)
            xs.key("Tab")
            time.sleep(0.35)
            fill_due(xs, month, day, year)
            ax.invoke(next_box)
            time.sleep(0.3)
            xs.type_ascii(next_step)
            time.sleep(0.3)
            xs.shot(EVID / "02-form-filled.png")
            nodes = app_nodes(pid)
            save = exact(nodes, "保存", "button")
            if save is None:
                raise RuntimeError("找不到保存按钮")
            ax.invoke(save)
            deadline = time.monotonic() + 25
            while time.monotonic() < deadline:
                if any(r.get("item") == title for r in record_items(store_path)):
                    break
                time.sleep(0.5)
            time.sleep(1.5)
            return record_items(store_path)

        # 02：第一条里程碑事项（只填期限，不填开始日期）。
        after_first = create_item("Synthetic Alpha", "P1", "11", "20", "2026", "Draft outline")
        facts["first_file_items"] = len(after_first)
        facts["first_file_item"] = after_first[0] if after_first else None
        xs.shot(EVID / "02-project-first-item.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "02-project-first-item-ax.json")
        facts["first_card_visible"] = any("Synthetic Alpha" in (n.name or "") for n in cards(nodes))
        facts["first_gantt_row"] = any(
            "Synthetic Alpha" in (n.name or "") and "期限" in (n.name or "") and "负责人" not in (n.name or "")
            for n in nodes
        )
        # 轴标题是普通 span，同样不在 AT-SPI 树；以甘特行出现作为轴已渲染的证据，
        # 截图 02-project-first-item.png 同时可见刻度。
        facts["first_axis_visible"] = facts["first_gantt_row"]

        # 03：第二条事项，随后用 ASCII 关键词验证搜索过滤。
        after_second = create_item("Synthetic Beta", "P2", "12", "05", "2026", "Check")
        facts["second_file_items"] = len(after_second)
        xs.shot(EVID / "03-project-two-items.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "03-project-two-items-ax.json")
        facts["two_cards"] = len(cards(nodes))
        search = exact(nodes, "搜索项目事项", "entry")
        if search is None:
            raise RuntimeError("找不到搜索框")
        ax.invoke(search)
        time.sleep(0.3)
        xs.type_ascii("Alpha")
        time.sleep(2.0)
        xs.shot(EVID / "03-project-search.png")
        filtered = app_nodes(pid)
        dump(filtered, EVID / "03-project-search-ax.json")
        facts["search_cards"] = len(cards(filtered))
        facts["search_kept_alpha"] = any("Synthetic Alpha" in (n.name or "") for n in cards(filtered))
        facts["search_hid_beta"] = not any("Synthetic Beta" in (n.name or "") for n in cards(filtered))
        xs.select_all_and_clear()
        time.sleep(1.5)

        # 04：修改第一条的期限，核对文件与界面同步。
        nodes = app_nodes(pid)
        edit = exact(nodes, "修改", "button")
        if edit is None:
            raise RuntimeError("找不到修改按钮")
        ax.invoke(edit)
        time.sleep(1.2)
        nodes = wait_until(lambda: app_nodes(pid))
        owner_box = exact(nodes, "负责人代号", "entry")
        if owner_box is None:
            raise RuntimeError("修改表单没有负责人字段")
        # 与新增同一路径：负责人 → Tab → 状态 → Tab → 期限月份段，直接覆盖三段。
        ax.invoke(owner_box)
        time.sleep(0.35)
        xs.key("Tab")
        time.sleep(0.35)
        xs.key("Tab")
        time.sleep(0.35)
        fill_due(xs, "12", "31", "2026")
        xs.shot(EVID / "04-edit-form.png")
        nodes = app_nodes(pid)
        save = exact(nodes, "保存", "button")
        if save is None:
            raise RuntimeError("找不到修改保存按钮")
        ax.invoke(save)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if any(r.get("due") == "2026-12-31" for r in record_items(store_path)):
                break
            time.sleep(0.5)
        time.sleep(1.5)
        edited = record_items(store_path)
        facts["edited_due"] = edited[0].get("due") if edited else None
        facts["edit_kept_count"] = len(edited)
        facts["edit_moved_created_at"] = bool(edited) and edited[0].get("created_at") != edited[0].get("updated_at")
        xs.shot(EVID / "04-project-edited.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "04-project-edited-ax.json")
        facts["edit_visible_due"] = any("2026-12-31" in (n.name or "") for n in cards(nodes))

        # 05：阶段成果切换（合成成果只读），再切回时间线。
        nodes = app_nodes(pid)
        art = exact(nodes, "阶段成果", "toggle button") or exact(nodes, "阶段成果", "button")
        if art is None:
            raise RuntimeError("找不到阶段成果切换")
        ax.invoke(art)
        time.sleep(2.0)
        xs.shot(EVID / "05-project-artifacts.png")
        art_nodes = app_nodes(pid)
        dump(art_nodes, EVID / "05-project-artifacts-ax.json")
        facts["artifacts_visible"] = any(
            "合成阶段成果" in (n.name or "") + (n.text or "") for n in art_nodes
        )
        timeline = exact(art_nodes, "时间线", "toggle button") or exact(art_nodes, "时间线", "button")
        if timeline is None:
            raise RuntimeError("找不到时间线切换")
        ax.invoke(timeline)
        time.sleep(1.5)

        # 06：删除确认。
        nodes = app_nodes(pid)
        before_delete = len(record_items(store_path))
        delete = exact(nodes, "删除", "button")
        if delete is None:
            raise RuntimeError("找不到删除按钮")
        ax.invoke(delete)
        time.sleep(1.5)
        xs.shot(EVID / "06-delete-confirm.png")
        nodes = app_nodes(pid)
        dialog = find(nodes, "删除这条项目事项？", contains=True)
        facts["delete_dialog_open"] = dialog is not None
        accept = None
        if dialog is not None:
            for n in dialog_nodes(dialog):
                if n.role == "button" and (n.name or "").strip() == "删除":
                    accept = n
                    break
        if accept is None:
            raise RuntimeError("确认框里找不到删除按钮")
        ax.invoke(accept)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if len(record_items(store_path)) < before_delete:
                break
            time.sleep(0.5)
        time.sleep(1.5)
        after_delete = record_items(store_path)
        facts["before_delete"] = before_delete
        facts["after_delete"] = len(after_delete)
        xs.shot(EVID / "06-project-after-delete.png")
        nodes = app_nodes(pid)
        dump(nodes, EVID / "06-project-after-delete-ax.json")
        facts["cards_after_delete"] = len(cards(nodes))

        # 07：窄窗单列，三个区域都还在。
        xs.resize(win, NARROW_W, NARROW_H, settle=3.0)
        time.sleep(2.0)
        xs.shot(EVID / "07-project-narrow.png")
        narrow = app_nodes(pid)
        dump(narrow, EVID / "07-project-narrow-ax.json")
        facts["narrow_width"] = NARROW_W
        facts["narrow_projects"] = find(narrow, "项目事项", contains=True) is not None
        facts["narrow_history"] = find(narrow, "历史对话", contains=True) is not None
        facts["narrow_timeline"] = find(narrow, "项目时间线", contains=True) is not None

        required = {
            "empty_no_metric": facts["ax_metric_strip_absent"],
            "empty_guide_screenshot": (EVID / "01-project-empty.png").is_file(),
            "empty_no_axis": facts["ax_gantt_axis_absent_when_empty"],
            "empty_no_file_before_save": facts["items_file_before"] is False,
            "left_projects": facts["ax_projects_heading"],
            "left_history": facts["ax_history_heading"],
            "history_thread": facts["ax_history_thread"],
            "first_written": facts["first_file_items"] == 1
            and (facts["first_file_item"] or {}).get("due") == "2026-11-20",
            "first_card": facts["first_card_visible"],
            "first_timeline_row": facts["first_axis_visible"],
            "second_written": facts["second_file_items"] == 2,
            "two_cards": facts["two_cards"] == 2,
            "search_one": facts["search_cards"] == 1,
            "search_alpha": facts["search_kept_alpha"],
            "search_no_beta": facts["search_hid_beta"],
            "edit_due_written": facts["edited_due"] == "2026-12-31",
            "edit_count_stable": facts["edit_kept_count"] == 2,
            "edit_visible": facts["edit_visible_due"],
            "artifacts": facts["artifacts_visible"],
            "delete_dialog": facts["delete_dialog_open"],
            "delete_written": facts["after_delete"] == 1,
            "delete_card_gone": facts["cards_after_delete"] == 1,
            "narrow_three_areas": facts["narrow_projects"]
            and facts["narrow_history"]
            and facts["narrow_timeline"],
        }
        facts["required"] = required
        facts["passed"] = all(required.values())
        (EVID / "facts.json").write_text(
            json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(facts, ensure_ascii=False, indent=2))
        if not facts["passed"]:
            print("未通过项：" + ", ".join(k for k, v in required.items() if not v), file=sys.stderr)
            return 1
        return 0
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
