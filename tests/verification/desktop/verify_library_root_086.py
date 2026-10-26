#!/usr/bin/env python3
"""OD-ARCH-086 真实窗口验收：设置页显示并保存库根，@ 引用跟随新库根。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI）+ 隔离设置文件事实。
所有写入都落在 /tmp 下的一次性根，不读取也不改动 ~/OmniDesk/库 与旧 YvanAI 路径。

验证链：
  1. 设置 → 成果与文档库 → 展开「库根」卡片，显示当前生效路径、来源与已保存选择；
  2. 在输入框填新绝对路径 → 校验并保存 → 状态行变为新路径；
  3. 设置文件只改 library_root，模型权限等其他字段保留；写探针无残留；
  4. 旧库根文件原地保留（不迁移、不覆盖、不删除）；
  5. 返回首页输入 @ → 只列出新库根内的合成文件，不再列出旧库根文件。

用法：python3 verify_library_root_086.py [--display :121] [--root /tmp/od086-ui]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession, Geometry  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-arch-086"
SCREEN_W, SCREEN_H = 1440, 900
INPUT_POINT = (700, 514)


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


def extents(node: ax.Node):
    from gi.repository import Atspi

    ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
    return ext


def all_text(nodes: list[ax.Node]) -> str:
    return "\n".join(n.name + "\n" + n.text for n in nodes)


def dump_tree(nodes: list[ax.Node], path: Path) -> None:
    """留一份完整无障碍树快照，便于判断是「界面没显示」还是「AT-SPI 没暴露」。"""
    rows = []
    for n in nodes:
        try:
            e = extents(n)
            box = {"x": e.x, "y": e.y, "w": e.width, "h": e.height}
        except Exception:
            box = None
        rows.append({"role": n.role, "name": n.name, "text": n.text, "box": box})
    path.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")


def pixel_diff_ratio(path_a: Path, path_b: Path) -> float:
    """两张同尺寸截图中明显不同的像素占比（用于确认输入框真的变了）。"""
    import gi

    gi.require_version("GdkPixbuf", "2.0")
    from gi.repository import GdkPixbuf

    a = GdkPixbuf.Pixbuf.new_from_file(str(path_a))
    b = GdkPixbuf.Pixbuf.new_from_file(str(path_b))
    if (a.get_width(), a.get_height()) != (b.get_width(), b.get_height()):
        return 1.0
    pa, pb = a.get_pixels(), b.get_pixels()
    stride, ch = a.get_rowstride(), a.get_n_channels()
    w, h = a.get_width(), a.get_height()
    changed = 0
    total = w * h
    for y in range(h):
        row_a = pa[y * stride: y * stride + w * ch]
        row_b = pb[y * stride: y * stride + w * ch]
        for x in range(w):
            ia = x * ch
            if abs(row_a[ia] - row_b[ia]) + abs(row_a[ia + 1] - row_b[ia + 1]) + abs(
                row_a[ia + 2] - row_b[ia + 2]
            ) > 30:
                changed += 1
    return changed / total


def read_settings(session: AppSession) -> dict:
    path = session.app_data / "config" / "settings.v1.json"
    return json.loads(path.read_text(encoding="utf-8"))


def tree_snapshot(root: Path) -> dict:
    if not root.exists():
        return {}
    return {
        str(p.relative_to(root)): p.read_text(encoding="utf-8", errors="replace")[:200]
        for p in sorted(root.rglob("*"))
        if p.is_file()
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":121")
    ap.add_argument("--root", default="/tmp/od086-ui")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    old_library = (root / "library-old").resolve()
    new_library = (root / "library-new").resolve()
    (old_library / "知识库").mkdir(parents=True)
    (old_library / "知识库" / "synth-old-card.md").write_text(
        "# 合成旧卡\n旧库根文件，切换后不应再出现在 @ 候选里。\n", encoding="utf-8"
    )

    facts: dict = {
        "plan": "OD-ARCH-086",
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 隔离设置文件事实",
        "root": str(root),
        "old_library": str(old_library),
        "new_library": str(new_library),
        "checks": {},
        "failures": [],
    }

    def check(name: str, ok: bool, measured: object, threshold: object, desc: str = "") -> None:
        passed = bool(ok)
        facts["checks"][name] = {
            "pass": passed,
            "measured": measured,
            "threshold": threshold,
            "desc": desc,
        }
        if not passed:
            facts["failures"].append(f"{name}: measured={measured}, threshold={threshold} ({desc})")
            print(f"FAIL [086] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [086] {name}: {measured}")

    old_before = tree_snapshot(old_library)
    bus_addr, bus = start_dbus()
    xvfb = start_display(args.display)
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ["DISPLAY"] = args.display
    if os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ.setdefault("AT_SPI_BUS_ADDRESS", "unix:path=/run/user/1000/at-spi/bus")

    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )
    try:
        session.prepare()
        settings_path = session.app_data / "config" / "settings.v1.json"
        settings_path.parent.mkdir(parents=True, exist_ok=True)
        settings_path.write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "artifact_root": None,
                    "library_root": str(old_library),
                    "model_permissions": {"local": True, "private": False, "public": False},
                },
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )
        session.start()
        pid = session.proc.pid if session.proc else None
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        ax.init()

        nodes = app_nodes(pid)
        settings_btn = find(nodes, "系统偏好设置", "button")
        check("settings_entry_visible", settings_btn is not None, settings_btn is not None, True)
        if settings_btn is None:
            raise RuntimeError("找不到设置入口")
        ax.invoke(settings_btn)
        time.sleep(2.5)
        nodes = app_nodes(pid)
        tab = find(nodes, "成果与文档库", "page tab", contains=False)
        check("library_tab_visible", tab is not None, tab is not None, True)
        if tab is None:
            raise RuntimeError("找不到「成果与文档库」分页")
        ax.invoke(tab)
        time.sleep(2.5)

        nodes = app_nodes(pid)
        summary = find(nodes, "库根（工作记录、周报与 @ 引用）")
        check("library_card_visible", summary is not None, summary is not None, True)
        if summary is None:
            raise RuntimeError("找不到库根卡片")
        ax.invoke(summary)
        time.sleep(1.5)
        nodes = app_nodes(pid)
        xs.shot(EVID / "00-library-card-expanded.png")
        dump_tree(nodes, EVID / "tree-00-expanded.json")

        # 卡片在页面底部，展开后的状态行与输入框在首屏之外。先把内容区滚到底，
        # 否则点击坐标会落在正文上（Ctrl+A 会选中整页文字）。
        xs.scroll(700, 740, clicks=12, settle=1.2)
        xs.shot(EVID / "00b-library-card-scrolled.png")
        nodes = app_nodes(pid)
        dump_tree(nodes, EVID / "tree-00b-scrolled.json")
        text = all_text(nodes)
        check("shows_effective_old_root", str(old_library) in text, str(old_library) in text, True,
              "展开后显示当前生效库根")
        check("shows_source_label", "你在设置中选择的目录" in text,
              "你在设置中选择的目录" in text, True, "来源标为已保存的用户设置")

        entries = []
        for n in nodes:
            if n.role != "entry" or n.name != "库根绝对路径":
                continue
            try:
                e = extents(n)
            except Exception:
                continue
            if 0 <= e.y and 0 <= e.x and e.x + e.width <= SCREEN_W and e.y + e.height <= SCREEN_H:
                entries.append(n)
        # 顶栏的「当前会话内查找」、成果目录输入框也在同一页；按 aria-label 定位库根输入框，
        # 并要求它已进入视口。
        entries.sort(key=lambda n: extents(n).y)
        check("library_input_visible", len(entries) >= 1, len(entries), ">=1")
        if not entries:
            raise RuntimeError("库根输入框不可定位")
        library_entry = entries[-1]
        ext = extents(library_entry)
        xs.click(ext.x + ext.width // 2, ext.y + ext.height // 2, settle=0.6)
        xs.shot(EVID / "01a-input-focused.png")
        xs.select_all_and_clear()
        xs.shot(EVID / "01b-input-cleared.png")
        xs.type_ascii(str(new_library), settle=0.02)
        time.sleep(0.8)
        xs.shot(EVID / "01-new-root-typed.png")
        cleared_ratio = pixel_diff_ratio(EVID / "01a-input-focused.png", EVID / "01b-input-cleared.png")
        typed_ratio = pixel_diff_ratio(EVID / "01b-input-cleared.png", EVID / "01-new-root-typed.png")
        facts["input_pixel_diff"] = {"cleared_ratio": round(cleared_ratio, 6),
                                    "typed_ratio": round(typed_ratio, 6)}
        # 清空与键入都只影响输入框内的字符：整屏变化应为千分之一量级，
        # 若 Ctrl+A 选中了整页会接近 100%（旧版脚本就是这样暴露问题的）。
        check("input_only_changed", typed_ratio < 0.01, round(typed_ratio, 6), "< 0.01",
              "键入没有造成整页选中/重排")
        check("input_typing_changed_pixels", typed_ratio > 0.0002, round(typed_ratio, 6),
              "> 0.0002", "输入框内容确实变化")

        nodes = app_nodes(pid)
        dump_tree(nodes, EVID / "tree-01-typed.json")
        save_buttons = [
            n for n in nodes
            if n.role == "button" and n.name == "校验并保存" and n.text == ""
        ]
        # 成果目录与库根各有一个「校验并保存」；库根在下方，取纵坐标更大者。
        save_buttons.sort(key=lambda n: extents(n).y)
        check("two_save_buttons_found", len(save_buttons) >= 2, len(save_buttons), ">=2")
        if len(save_buttons) < 2:
            raise RuntimeError("找不到库根的校验并保存按钮")
        save_ext = extents(save_buttons[-1])
        check(
            "library_save_button_in_viewport",
            0 <= save_ext.y and save_ext.y + save_ext.height <= SCREEN_H,
            {"y": save_ext.y, "h": save_ext.height},
            f"y+h <= {SCREEN_H}",
        )
        xs.click(save_ext.x + save_ext.width // 2, save_ext.y + save_ext.height // 2, settle=0.8)
        time.sleep(3.0)

        nodes = app_nodes(pid)
        dump_tree(nodes, EVID / "tree-02-saved.json")
        after_text = all_text(nodes)
        dialog_alerts = [n.name for n in nodes if n.role == "alert"]
        check(
            "saved_root_shown",
            str(new_library) in after_text and not dialog_alerts,
            {"state_hint": str(new_library) in after_text, "alerts": dialog_alerts},
            True,
            "保存后状态行显示新库根",
        )
        # 提示段落不进 WebKit 无障碍树（截图 02 可见）。这条改用源码静态核对，
        # 证据等级是「静态代码检查」，不冒充窗口断言。
        lib_rs = (HERE.parents[2] / "desktop" / "src-tauri" / "src" / "lib.rs").read_text(
            encoding="utf-8"
        )
        panel_tsx = (HERE.parents[2] / "desktop" / "src" / "SettingsPanel.tsx").read_text(
            encoding="utf-8"
        )
        check(
            "save_note_scope_truthful_static",
            '仅影响之后的 @ 引用；既有文件不会移动、覆盖或删除。' in lib_rs
            and "技能写入；既有文件" not in lib_rs
            and "之后的 @ 引用</strong>指向哪里" in panel_tsx
            and "之后的引用与技能写入</strong>" not in panel_tsx,
            "静态源码文案口径一致（截图 02 可视核对）",
            True,
            "提示只声称 @ 引用范围，不虚称技能写入跟随",
        )
        xs.shot(EVID / "02-library-saved.png")

        settings = read_settings(session)
        check("settings_persisted_library_root", settings.get("library_root") == str(new_library),
              settings.get("library_root"), str(new_library))
        check("settings_preserve_model_permissions",
              settings.get("model_permissions", {}).get("local") is True,
              settings.get("model_permissions"), "local=true 保留")
        check("new_root_created", new_library.is_dir(), new_library.is_dir(), True)
        probes = [p.name for p in new_library.rglob(".omnidesk-write-probe*")] if new_library.exists() else []
        check("write_probe_cleaned", not probes, probes, "[]")
        check("old_root_files_untouched", tree_snapshot(old_library) == old_before,
              tree_snapshot(old_library) == old_before, True, "旧库不迁移、不覆盖、不删除")
        check("old_root_still_exists", (old_library / "知识库" / "synth-old-card.md").is_file(),
              (old_library / "知识库" / "synth-old-card.md").is_file(), True)

        # 新库放一张合成卡，回到首页验证 @ 引用跟随新库根。
        (new_library / "知识库").mkdir(parents=True, exist_ok=True)
        (new_library / "知识库" / "synth-new-card.md").write_text(
            "# 合成新卡\n切换后的库根文件，应出现在 @ 候选里。\n", encoding="utf-8"
        )
        nodes = app_nodes(pid)
        home_btn = find(nodes, "工作台首页", "button")
        check("home_entry_visible", home_btn is not None, home_btn is not None, True)
        if home_btn is None:
            raise RuntimeError("找不到工作台首页入口")
        ax.invoke(home_btn)
        time.sleep(2.5)
        nodes = app_nodes(pid)
        dump_tree(nodes, EVID / "tree-03-home.json")
        home_entries = [
            n for n in nodes
            if n.role == "entry" and "模型权限" in (n.name or "")
        ]
        check("home_input_visible", len(home_entries) == 1, len(home_entries), 1)
        if len(home_entries) != 1:
            raise RuntimeError("首页消息输入框不可唯一定位")
        home_ext = extents(home_entries[0])
        xs.click(home_ext.x + home_ext.width // 2, home_ext.y + home_ext.height // 2, settle=0.8)
        xs.type_ascii("@synth-", settle=0.12)
        time.sleep(2.5)
        xs.shot(EVID / "03-mention-new-library.png")
        nodes = app_nodes(pid)
        dump_tree(nodes, EVID / "tree-03-mention.json")
        text = all_text(nodes)
        check("mention_lists_new_library_file", "synth-new-card.md" in text,
              "synth-new-card.md" in text, True, "@ 引用来自新库根")
        check("mention_excludes_old_library_file", "synth-old-card.md" not in text,
              "synth-old-card.md" in text, False, "切换后不再列旧库根")
    finally:
        session.stop_all()
        bus.terminate()
        xvfb.terminate()

    (EVID / "facts.json").write_text(json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"checks={len(facts['checks'])} failures={len(facts['failures'])}")
    return 0 if not facts["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
