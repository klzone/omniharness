#!/usr/bin/env python3
"""OD-KB-001 P2 隔离验收：浏览、属性、搜索与安全编辑。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI）+ Rust 宿主测试实测。
全部读写限定在 /tmp 下一次性合成根；不读取真实 Vault、正式资料或 Obsidian 配置，
不改 runtime/，不启动模型，不使用真实附件处理程序（PATH 注入合成 xdg-open）。

覆盖：
  1. Dock 进入知识库分页，三栏（列表 / 正文 / 属性）与设置引导；
  2. 属性、标签、别名、来源路径与原文/预览/编辑三种模式；
  3. 新建、保存后宿主重新读取、外部修改冲突不覆盖磁盘；
  4. 放弃修改/切换笔记/新建/离开分页的确认可取消、可确认（应用内对话框）；
  5. 关键词、标签、属性筛选；刷新扫描反映外部增删；
  6. 重启不恢复未保存草稿；附件只有点击后才交给合成打开器；
  7. 越界/隐藏/符号链接/非 Markdown 拒绝与搜索预算指标（宿主测试实测）。

用法：python3 verify_knowledge_vault_p2.py [--display :134] [--root /tmp/odkb-p2-ui]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-kb-001-p2"
SCREEN_W, SCREEN_H = 1440, 900


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
        if any((name in h) if contains else (name == h) for h in (node.name, node.text)):
            return node
    return None


def extents(node: ax.Node):
    from gi.repository import Atspi

    return node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)


def all_text(nodes: list[ax.Node]) -> str:
    return "\n".join(n.name + "\n" + n.text for n in nodes)


def clipboard_text() -> str:
    """读取本次一次性显示上的 CLIPBOARD；WebKitGTK 的 textarea 不把值暴露给 AT-SPI。"""
    try:
        import gi

        gi.require_version("Gtk", "3.0")
        from gi.repository import Gdk, Gtk

        clip = Gtk.Clipboard.get(Gdk.Atom.intern("CLIPBOARD", False))
        return clip.wait_for_text() or ""
    except Exception:
        return ""


def dump_tree(nodes: list[ax.Node], path: Path) -> None:
    rows = []
    for n in nodes:
        try:
            e = extents(n)
            box = {"x": e.x, "y": e.y, "w": e.width, "h": e.height}
        except Exception:
            box = None
        rows.append({"role": n.role, "name": n.name, "text": n.text, "box": box})
    path.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")


def tree_snapshot(root: Path) -> dict:
    import hashlib

    out: dict = {}
    for path in sorted(root.rglob("*")):
        rel = str(path.relative_to(root))
        st = path.lstat()
        if path.is_symlink():
            out[rel] = {"kind": "symlink", "target": os.readlink(path)}
        elif path.is_dir():
            out[rel] = {"kind": "dir", "mtime_ns": st.st_mtime_ns}
        else:
            out[rel] = {
                "kind": "file",
                "size": st.st_size,
                "mtime_ns": st.st_mtime_ns,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
    return out


ALPHA = (
    "---\n"
    "title: 合成阿尔法\n"
    "status: 进行中\n"
    "tags:\n"
    "  - project/alpha\n"
    "  - todo\n"
    "aliases: [jia, Alpha]\n"
    "unknown_key: 保留原样\n"
    "code: alpha42\n"
    "---\n"
    "\n"
    "# 阿尔法\n"
    "\n"
    "银河计划是合成夹具里的关键词 milkyway。\n"
)

BETA = (
    "---\n"
    "tags: [未闭合\n"
    "---\n"
    "\n"
    "# 贝塔\n"
    "\n"
    "这段正文在无效 Frontmatter 下仍需可搜索：betakey。\n"
)

GAMMA = "# 伽马\n\n没有任何 Frontmatter。\n"


def build_vault(root: Path) -> None:
    (root / ".obsidian").mkdir(parents=True)
    (root / ".obsidian" / "app.json").write_text(
        json.dumps({"legacyEditor": False}), encoding="utf-8"
    )
    (root / ".trash").mkdir()
    (root / ".trash" / "deleted.md").write_text("# 已删除\n", encoding="utf-8")
    (root / "node_modules").mkdir()
    (root / "node_modules" / "skip.md").write_text("# 跳过\n", encoding="utf-8")
    (root / "notes").mkdir()
    (root / "notes" / "alpha.md").write_text(ALPHA, encoding="utf-8")
    (root / "notes" / "beta.md").write_text(BETA, encoding="utf-8")
    (root / "notes" / "gamma.md").write_text(GAMMA, encoding="utf-8")
    (root / "assets").mkdir()
    (root / "assets" / "diagram.png").write_bytes(b"\x89PNG\r\n\x1a\n synthetic")
    # 根外符号链接：扫描与写入都必须拒绝。
    outside = root.parent / "outside-secret.png"
    outside.write_bytes(b"outside")
    os.symlink(outside, root / "assets" / "escape.png")


def run_host_tests() -> dict:
    env = dict(os.environ)
    setup = REPO / "scripts" / "setup-desktop-toolchain.sh"
    if setup.is_file():
        printed = subprocess.run(
            ["bash", "-c", f'eval "$(bash {setup} --print-env)" && env -0'],
            capture_output=True,
            text=True,
            cwd=str(REPO),
        )
        if printed.returncode == 0:
            for item in printed.stdout.split("\0"):
                if "=" in item:
                    key, _, value = item.partition("=")
                    env[key] = value
    cwd = REPO / "desktop" / "src-tauri"
    write = subprocess.run(
        ["cargo", "test", "--offline", "--test", "knowledge_vault_write_host", "--", "--nocapture"],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        env=env,
    )
    search = subprocess.run(
        [
            "cargo",
            "test",
            "--offline",
            "--lib",
            "vault_search::tests::synthetic_scale_search_metric",
            "--",
            "--nocapture",
        ],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        env=env,
    )
    log = (
        "== knowledge_vault_write_host ==\n"
        + write.stdout
        + write.stderr
        + "\n== vault_search::synthetic_scale_search_metric ==\n"
        + search.stdout
        + search.stderr
    )
    (EVID / "host-test.log").write_text(log, encoding="utf-8")
    passed = re.search(r"test result: ok\. (\d+) passed", write.stdout)
    metric = None
    for line in search.stdout.splitlines():
        if line.startswith("KB_P2_SEARCH_METRIC "):
            metric = json.loads(line[len("KB_P2_SEARCH_METRIC "):])
    return {
        "write_exit_code": write.returncode,
        "write_passed": int(passed.group(1)) if passed else 0,
        "search_exit_code": search.returncode,
        "search_metric": metric,
        "log": str((EVID / "host-test.log").relative_to(REPO)),
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":134")
    ap.add_argument("--root", default="/tmp/odkb-p2-ui")
    ap.add_argument(
        "--evidence-dir",
        default=None,
        help="证据输出目录；回归请指向独立目录，避免覆盖原始证据",
    )
    args = ap.parse_args(argv[1:])

    global EVID
    EVID = Path(args.evidence_dir).resolve() if args.evidence_dir else EVID
    allowed_evidence = (HERE / "evidence").resolve()
    if allowed_evidence not in EVID.parents:
        raise SystemExit(f"证据目录必须在 {allowed_evidence} 之下：{EVID}")

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    vault = (root / "synth-vault").resolve()
    build_vault(vault)
    before_scan = tree_snapshot(vault)

    # 合成附件打开器：只记录收到的参数，绝不调用真实处理程序。
    fake_bin = root / "fake-bin"
    fake_bin.mkdir()
    marker = root / "opened-attachment.txt"
    fake_open = fake_bin / "xdg-open"
    fake_open.write_text(
        "#!/bin/sh\nprintf '%s\\n' \"$1\" >> " + str(marker) + "\nexit 0\n",
        encoding="utf-8",
    )
    fake_open.chmod(fake_open.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)

    host = run_host_tests()

    facts: dict = {
        "plan": "OD-KB-001 P2",
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ Rust 宿主测试实测",
        "root": str(root),
        "synthetic_vault": str(vault),
        "host_tests": host,
        "environment": {
            "platform": os.uname().sysname,
            "machine": os.uname().machine,
            "python": sys.version.split()[0],
        },
        "checks": {},
        "failures": [],
        "screenshots": [],
    }

    def check(name: str, ok: bool, measured: object, threshold: object, desc: str = "") -> None:
        facts["checks"][name] = {
            "pass": bool(ok),
            "measured": measured,
            "threshold": threshold,
            "desc": desc,
        }
        if not ok:
            facts["failures"].append(f"{name}: measured={measured}, threshold={threshold} ({desc})")
            print(f"FAIL [KB-P2] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [KB-P2] {name}: {measured}")

    check(
        "host_write_tests_pass",
        host["write_exit_code"] == 0 and host["write_passed"] >= 7,
        {"exit_code": host["write_exit_code"], "passed": host["write_passed"]},
        {"exit_code": 0, "passed": ">=7"},
        "宿主创建/保存/冲突/失败清理/附件边界集成测试",
    )
    check(
        "search_metric_collected",
        host["search_exit_code"] == 0 and host["search_metric"] is not None,
        host["search_metric"],
        {"exit_code": 0, "notes": 1200, "budget_bytes": 33554432},
        "1200 篇合成笔记的关键词搜索实测（含读取预算与耗时）",
    )

    bus_addr, bus = start_dbus()
    xvfb = start_display(args.display)
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ["DISPLAY"] = args.display
    if os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ.setdefault("AT_SPI_BUS_ADDRESS", "unix:path=/run/user/1000/at-spi/bus")

    env_path = str(fake_bin) + ":" + os.environ.get("PATH", "/usr/bin:/bin")
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        reuse_running_dev_server=True,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "PATH": env_path,
        },
    )

    xs: XSession | None = None
    pid = 0

    def shot(name: str) -> None:
        assert xs is not None
        xs.shot(EVID / name)
        facts["screenshots"].append(name)

    def nodes() -> list[ax.Node]:
        return app_nodes(pid)

    def click(node: ax.Node | None, settle: float = 0.8) -> bool:
        if node is None or xs is None:
            return False
        e = extents(node)
        if e.width <= 0 or e.height <= 0:
            return False
        xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=settle)
        return True

    def click_named(name: str, role: str = "button", contains: bool = True, settle: float = 0.8) -> bool:
        return click(find(nodes(), name, role=role, contains=contains), settle=settle)

    def scroll_until_visible(
        name: str, role: str | None = None, contains: bool = True
    ) -> ax.Node | None:
        """把目标滚进可视区再返回它。

        P3 新增的「链接关系」卡片会把右栏内容推到首屏之外；AT-SPI 给的是屏幕绝对
        坐标，直接点窗口外的点不会产生任何事件。这里滚动到目标可见位置后再交给
        调用方点击。
        """
        for _ in range(14):
            node = find(nodes(), name, role=role, contains=contains)
            if node is None:
                return None
            e = extents(node)
            if e.width > 0 and e.height > 0 and e.y >= 0 and e.y + e.height <= SCREEN_H - 12:
                return node
            if xs is None:
                return None
            xs.scroll(1120, SCREEN_H // 2, 3, up=e.y < 0, settle=0.5)
            time.sleep(0.3)
        return None

    def click_scrolled(
        name: str, role: str = "button", contains: bool = True, settle: float = 0.8
    ) -> bool:
        return click(scroll_until_visible(name, role=role, contains=contains), settle=settle)

    def focus_entry(name: str, contains: bool = True) -> bool:
        node = scroll_until_visible(name, role="entry", contains=contains)
        if node is None or xs is None:
            return False
        e = extents(node)
        xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.4)
        return True

    def editor_node() -> ax.Node | None:
        return next((n for n in nodes() if n.role == "entry" and "草稿" in n.name), None)

    def ctrl_key(letter: str) -> None:
        assert xs is not None
        ctrl = xs.d.keysym_to_keycode(xs._XK.string_to_keysym("Control_L"))
        code = xs.d.keysym_to_keycode(xs._XK.string_to_keysym(letter))
        xs._xtest.fake_input(xs.d, xs._X.KeyPress, ctrl)
        xs.d.sync()
        xs._xtest.fake_input(xs.d, xs._X.KeyPress, code)
        xs.d.sync()
        xs._xtest.fake_input(xs.d, xs._X.KeyRelease, code)
        xs.d.sync()
        xs._xtest.fake_input(xs.d, xs._X.KeyRelease, ctrl)
        xs.d.sync()
        time.sleep(0.25)

    def editor_text() -> str:
        """选中编辑器全部内容并复制到本显示剪贴板，再收回选区。"""
        node = editor_node()
        if node is None or xs is None:
            return ""
        e = extents(node)
        xs.click(e.x + e.width // 2, e.y + e.height // 3, settle=0.35)
        ctrl_key("a")
        ctrl_key("c")
        value = clipboard_text()
        xs.key("End")
        return value

    def open_vault_page() -> None:
        click_named("知识库", role="button")
        time.sleep(3.5)

    def open_note(file_name: str) -> None:
        click(find(nodes(), file_name, role="button"), settle=1.2)
        time.sleep(2.2)

    def type_in_editor(text: str) -> None:
        node = editor_node()
        if node is None or xs is None:
            raise RuntimeError("找不到笔记编辑器")
        e = extents(node)
        xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.5)
        xs.key("End")
        xs.type_ascii(text, settle=0.03)
        time.sleep(0.8)

    def dialog_open() -> bool:
        return any(n.role in ("alert", "alertdialog", "dialog") for n in nodes())

    def search_hits() -> dict[str, str]:
        """命中区：按钮 aria-label 形如「搜索命中：<路径>；区域：...」。"""
        out: dict[str, str] = {}
        for n in nodes():
            name = n.name or ""
            if name.startswith("搜索命中："):
                body = name[len("搜索命中：") :]
                path, _, area = body.partition("；区域：")
                out[path.strip()] = area.strip()
        return out

    def search_hits_after_scroll() -> dict[str, str]:
        """结果列表在右栏折叠区里，可能要滚动后才会进入无障碍树。"""
        for _ in range(8):
            hits = search_hits()
            if hits:
                return hits
            stats = [n for n in nodes() if (n.name or "").startswith("搜索统计：")]
            if stats and "0 条命中" in (stats[0].name or ""):
                return hits
            if xs is None:
                return hits
            xs.scroll(1120, SCREEN_H // 2, 3, settle=0.5)
            time.sleep(0.4)
        return search_hits()

    try:
        session.prepare()
        settings_path = session.app_data / "config" / "settings.v1.json"
        settings_path.parent.mkdir(parents=True, exist_ok=True)

        def write_settings(vault_root: str | None) -> None:
            cfg = {
                "schema": "omnidesk.settings.v1",
                "artifact_root": None,
                "library_root": str(root / "unused-library"),
                "model_permissions": {"local": True, "private": False, "public": False},
            }
            if vault_root:
                cfg["knowledge_vault_root"] = vault_root
            settings_path.write_text(
                json.dumps(cfg, ensure_ascii=False) + "\n", encoding="utf-8"
            )

        # 第一次启动：未设置知识库根，验证引导。
        write_settings(None)
        session.start()
        pid = session.proc.pid if session.proc else 0
        xs = XSession(args.display)
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        ax.init()

        open_vault_page()
        nodes_now = nodes()
        shot("00-vault-unset.png")
        dump_tree(nodes_now, EVID / "tree-00-unset.json")
        text = all_text(nodes_now)
        check(
            "unset_root_guidance",
            find(nodes_now, "未设置知识库目录：去设置选择目录", contains=True) is not None,
            {
                "has_guidance": find(
                    nodes_now, "未设置知识库目录：去设置选择目录", contains=True
                )
                is not None
            },
            True,
            "未设置根时有明确设置引导",
        )
        check(
            "no_write_before_open",
            not marker.exists(),
            {"marker_exists": marker.exists()},
            False,
            "附件未被点击前不得调用打开器",
        )

        # 第二次启动：配置合成根。
        session.stop_app()
        write_settings(str(vault))
        session.start()
        pid = session.proc.pid if session.proc else 0
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("重启后没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        ax.init()

        open_vault_page()
        time.sleep(2.0)
        nodes_now = nodes()
        shot("01-vault-list.png")
        dump_tree(nodes_now, EVID / "tree-01-list.json")
        text = all_text(nodes_now)
        check(
            "dock_entry_and_three_columns",
            "知识库" in text
            and find(nodes_now, "笔记", role="heading") is not None
            and find(nodes_now, "属性", role="heading") is not None
            and find(nodes_now, "附件", role="heading") is not None,
            {
                "notes_heading": find(nodes_now, "笔记", role="heading") is not None,
                "properties_heading": find(nodes_now, "属性", role="heading") is not None,
                "attachments_heading": find(nodes_now, "附件", role="heading") is not None,
            },
            True,
            "从 Dock 进入知识库，列表/正文/属性三栏与附件面板同屏",
        )
        check(
            "scan_listed_notes",
            find(nodes_now, "alpha.md", role="button") is not None
            and find(nodes_now, "beta.md", role="button") is not None
            and find(nodes_now, "gamma.md", role="button") is not None,
            {"note_count_hint": "3 expected"},
            True,
            "三篇合成笔记进入列表",
        )
        check(
            "hidden_paths_not_listed",
            ".obsidian/app.json" not in text
            and "deleted.md" not in text
            and "skip.md" not in text,
            {
                "obsidian": ".obsidian/app.json" in text,
                "trash": "deleted.md" in text,
                "excluded": "skip.md" in text,
            },
            {"obsidian": False, "trash": False, "excluded": False},
            "隐藏/回收站/排除目录不进列表",
        )

        open_note("alpha.md")
        nodes_now = nodes()
        shot("02-note-properties.png")
        dump_tree(nodes_now, EVID / "tree-02-note.json")
        text = all_text(nodes_now)
        property_node = find(nodes_now, "属性列表：", contains=True)
        tag_node = find(nodes_now, "标签：", contains=True)
        check(
            "properties_and_source",
            property_node is not None
            and "status=进行中" in property_node.name
            and "title=合成阿尔法" in property_node.name
            and tag_node is not None
            and "project/alpha" in tag_node.name
            and "属性来源：notes/alpha.md" in text,
            {
                "properties": property_node.name if property_node else None,
                "tags": tag_node.name if tag_node else None,
                "source": "属性来源：notes/alpha.md" in text,
            },
            True,
            "Frontmatter 属性、标签与来源路径可定位",
        )
        check(
            "unknown_key_preserved",
            property_node is not None and "unknown_key=保留原样" in property_node.name,
            {
                "unknown_key_visible": property_node is not None
                and "unknown_key=保留原样" in property_node.name
            },
            True,
            "未知键保留并展示，未被静默丢弃",
        )

        click_named("预览", role="button", contains=False)
        time.sleep(1.2)
        shot("03-preview.png")
        preview_node = find(nodes(), "预览正文：", contains=True)
        check(
            "preview_renders_body",
            preview_node is not None and "银河计划" in preview_node.name,
            {"preview": preview_node.name if preview_node else None},
            True,
            "预览渲染正文，属性面板保持可定位",
        )

        click_named("原文", role="button", contains=False)
        time.sleep(1.2)
        shot("04-raw.png")
        raw_node = find(nodes(), "原文：", contains=True)
        check(
            "raw_keeps_frontmatter",
            raw_node is not None
            and "unknown_key: 保留原样" in raw_node.name
            and "aliases" in raw_node.name,
            {"raw": (raw_node.name[:200] if raw_node else None)},
            True,
            "原文模式保留 Frontmatter 原始文本",
        )
        click_named("编辑", role="button", contains=False)
        time.sleep(1.0)

        # 保存：追加标记后保存，宿主重新读取落盘内容。
        mark = "save-mark-42"
        type_in_editor(" " + mark)
        time.sleep(0.8)
        dirty_visible = find(nodes(), "未保存草稿") is not None
        click_named("保存", role="button", contains=False, settle=1.2)
        time.sleep(3.0)
        disk_alpha = (vault / "notes" / "alpha.md").read_text(encoding="utf-8")
        shot("05-saved.png")
        dump_tree(nodes(), EVID / "tree-05-saved.json")
        check(
            "save_writes_and_rereads",
            mark in disk_alpha and mark in editor_text() and not find(nodes(), "未保存草稿"),
            {
                "mark_in_disk": mark in disk_alpha,
                "mark_in_editor": mark in editor_text(),
                "unsaved_badge": find(nodes(), "未保存草稿") is not None,
            },
            {"mark_in_disk": True, "mark_in_editor": True, "unsaved_badge": False},
            "保存落盘并由宿主重新读取后显示",
        )
        check(
            "dirty_badge_before_save",
            dirty_visible,
            {"unsaved_badge": dirty_visible},
            True,
            "编辑后显示未保存草稿标记",
        )

        # 外部修改 → 保存冲突且不覆盖磁盘。
        external = "# 外部版本\n\n由验收脚本直接写入。\n"
        (vault / "notes" / "alpha.md").write_text(external, encoding="utf-8")
        type_in_editor(" conflict-draft")
        time.sleep(0.8)
        click_named("保存", role="button", contains=False, settle=1.2)
        time.sleep(3.0)
        nodes_now = nodes()
        shot("06-conflict.png")
        dump_tree(nodes_now, EVID / "tree-06-conflict.json")
        text = all_text(nodes_now)
        check(
            "external_conflict_rejected",
            (vault / "notes" / "alpha.md").read_text(encoding="utf-8") == external
            and (
                "未写入：文件已被外部修改" in text
                or find(nodes_now, "未写入：文件已被外部修改", contains=True) is not None
            )
            and "conflict-draft" in editor_text(),
            {
                "disk_unchanged": (vault / "notes" / "alpha.md").read_text(encoding="utf-8")
                == external,
                "conflict_message": "未写入：文件已被外部修改" in text
                or find(nodes_now, "未写入：文件已被外部修改", contains=True) is not None,
                "draft_kept": "conflict-draft" in editor_text(),
            },
            {"disk_unchanged": True, "conflict_message": True, "draft_kept": True},
            "哈希冲突不写盘，草稿保留在内存",
        )

        # 确认框：取消后草稿仍在。
        click_named("放弃修改", role="button", contains=False, settle=1.0)
        time.sleep(1.0)
        shot("07-confirm-discard.png")
        dump_tree(nodes(), EVID / "tree-07-confirm.json")
        check(
            "discard_confirm_visible",
            dialog_open() and "放弃未保存的修改？" in all_text(nodes()),
            {"dialog": dialog_open()},
            True,
            "放弃修改先弹出应用内确认",
        )
        click_named("取消", role="button", contains=False, settle=1.0)
        time.sleep(1.0)
        shot("08-confirm-cancelled.png")
        check(
            "discard_cancel_keeps_draft",
            "conflict-draft" in editor_text() and not dialog_open(),
            {"draft_kept": "conflict-draft" in editor_text(), "dialog": dialog_open()},
            {"draft_kept": True, "dialog": False},
            "取消放弃后草稿完整保留",
        )

        # 取消切换笔记同样保留草稿。
        click(find(nodes(), "gamma.md", role="button"), settle=1.0)
        time.sleep(1.0)
        shot("09-confirm-switch.png")
        check(
            "switch_confirm_visible",
            dialog_open() and "切换笔记？" in all_text(nodes()),
            {"dialog": dialog_open()},
            True,
            "切换笔记前确认",
        )
        click_named("取消", role="button", contains=False, settle=1.0)
        time.sleep(1.0)
        check(
            "switch_cancel_keeps_note",
            "conflict-draft" in editor_text(),
            {"draft_kept": "conflict-draft" in editor_text()},
            True,
            "取消切换仍停留在原笔记草稿",
        )

        # 确认放弃 → 回到磁盘版本（对话框按钮是「确认放弃」）。
        click_named("放弃修改", role="button", contains=False, settle=1.0)
        time.sleep(0.8)
        click_named("确认放弃", role="button", contains=False, settle=1.0)
        time.sleep(1.6)
        shot("10-discarded.png")
        check(
            "discard_accept_reverts",
            editor_text() == external and not find(nodes(), "未保存草稿"),
            {
                "editor_matches_disk": editor_text() == external,
                "unsaved_badge": find(nodes(), "未保存草稿") is not None,
            },
            {"editor_matches_disk": True, "unsaved_badge": False},
            "确认放弃后恢复磁盘内容",
        )

        # 冲突测试把 alpha 换成了无 Frontmatter 的外部版本；搜索用例需要原始合成夹具。
        (vault / "notes" / "alpha.md").write_text(ALPHA, encoding="utf-8")
        click_named("刷新扫描", role="button", contains=False, settle=1.2)
        time.sleep(2.0)

        # 新建笔记（无草稿，直接创建）。
        click_named("新建笔记", role="button", contains=False, settle=1.0)
        time.sleep(1.0)
        path_entry = find(nodes(), "笔记/新想法.md", role="entry")
        if path_entry is None:
            path_entry = find(nodes(), "相对路径（父目录必须已存在）")
        if path_entry is not None and xs is not None:
            e = extents(path_entry)
            xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.4)
            xs.select_all_and_clear()
            xs.type_ascii("notes/new-note.md")
        content_entry = find(nodes(), "初始内容")
        if content_entry is not None and xs is not None:
            e = extents(content_entry)
            xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.4)
            xs.type_ascii("# new synthetic note")
        time.sleep(0.5)
        shot("11-new-form.png")
        click_named("创建笔记", role="button", contains=False, settle=1.2)
        time.sleep(3.0)
        shot("12-created.png")
        created_path = vault / "notes" / "new-note.md"
        check(
            "create_note_persists",
            created_path.is_file()
            and "# new synthetic note" in created_path.read_text(encoding="utf-8")
            and "notes/new-note.md" in all_text(nodes()),
            {
                "file_exists": created_path.is_file(),
                "content_on_disk": created_path.is_file()
                and "# new synthetic note" in created_path.read_text(encoding="utf-8"),
            },
            True,
            "新建笔记落盘并切换到该笔记",
        )

        # 新建已存在目标：必须拒绝且不改动原文件。
        before_existing = (vault / "notes" / "alpha.md").read_text(encoding="utf-8")
        click_named("新建笔记", role="button", contains=False, settle=1.0)
        time.sleep(1.0)
        path_entry = find(nodes(), "笔记/新想法.md", role="entry") or find(
            nodes(), "相对路径（父目录必须已存在）"
        )
        if path_entry is not None and xs is not None:
            e = extents(path_entry)
            xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.4)
            xs.select_all_and_clear()
            xs.type_ascii("notes/alpha.md")
        click_named("创建笔记", role="button", contains=False, settle=1.2)
        time.sleep(2.5)
        nodes_now = nodes()
        shot("13-create-exists.png")
        text = all_text(nodes_now)
        check(
            "create_existing_rejected",
            ("新建失败" in text or find(nodes_now, "新建失败", contains=True) is not None)
            and (vault / "notes" / "alpha.md").read_text(encoding="utf-8") == before_existing,
            {
                "error_visible": "新建失败" in text
                or find(nodes_now, "新建失败", contains=True) is not None,
                "disk_unchanged": (vault / "notes" / "alpha.md").read_text(encoding="utf-8")
                == before_existing,
            },
            {"error_visible": True, "disk_unchanged": True},
            "新建已存在目标被拒绝且原文件不变",
        )

        # 搜索：关键词、标签、属性。
        if xs is not None and focus_entry("关键词（字面子串，不区分大小写）"):
            xs.select_all_and_clear()
            xs.type_ascii("milkyway")
        click_scrolled("搜索", role="button", contains=False, settle=1.2)
        time.sleep(3.0)
        shot("14-search-keyword.png")
        hits = search_hits_after_scroll()
        check(
            "keyword_search_hits_body",
            "notes/alpha.md" in hits and "body" in hits.get("notes/alpha.md", ""),
            {"hits": hits, "alpha_areas": hits.get("notes/alpha.md")},
            True,
            "正文关键词命中并标明文件与命中区域",
        )

        # 清空后按标签筛选。
        click_scrolled("清空", role="button", contains=False, settle=0.8)
        time.sleep(0.8)
        if xs is not None and focus_entry("标签（逗号分隔"):
            xs.type_ascii("project")
        click_scrolled("搜索", role="button", contains=False, settle=1.2)
        time.sleep(2.5)
        shot("15-search-tag.png")
        hits = search_hits_after_scroll()
        check(
            "tag_filter_parent_match",
            "notes/alpha.md" in hits and "notes/gamma.md" not in hits,
            {
                "hits": hits,
                "alpha_hit": "notes/alpha.md" in hits,
                "gamma_excluded": "notes/gamma.md" not in hits,
            },
            {"alpha_hit": True, "gamma_excluded": True},
            "标签筛选支持父标签包含下级",
        )

        # 属性筛选：code=alpha42（ASCII，便于合成键盘注入；中文值由宿主测试覆盖）。
        click_scrolled("清空", role="button", contains=False, settle=0.8)
        time.sleep(0.8)
        if xs is not None and focus_entry("属性键"):
            xs.select_all_and_clear()
            xs.type_ascii("code")
        if xs is not None and focus_entry("属性值（可空）"):
            xs.select_all_and_clear()
            xs.type_ascii("alpha42")
        click_scrolled("搜索", role="button", contains=False, settle=1.2)
        time.sleep(2.5)
        shot("16-search-property.png")
        hits = search_hits_after_scroll()
        check(
            "property_filter_matches",
            "notes/alpha.md" in hits and "notes/gamma.md" not in hits,
            {
                "hits": hits,
                "alpha_hit": "notes/alpha.md" in hits,
                "gamma_excluded": "notes/gamma.md" not in hits,
            },
            {"alpha_hit": True, "gamma_excluded": True},
            "属性键与值同时匹配",
        )

        click_named("清空", role="button", contains=False, settle=0.8)
        time.sleep(0.8)

        # 附件：点击才调用合成打开器。
        check(
            "attachment_not_opened_yet",
            not marker.exists(),
            {"marker_exists": marker.exists()},
            False,
            "浏览/搜索过程中附件未被打开",
        )
        open_buttons = [
            n for n in nodes() if n.role == "button" and n.name == "打开"
        ]
        attachment_nodes = [
            n for n in nodes() if "assets/diagram.png" in (n.name + n.text)
        ]
        if open_buttons:
            click(open_buttons[0], settle=1.0)
            time.sleep(2.0)
        shot("17-attachment-opened.png")
        opened = marker.read_text(encoding="utf-8").strip() if marker.exists() else ""
        check(
            "attachment_opened_on_click",
            bool(opened) and str(vault) in opened and "diagram.png" in opened,
            {"marker": opened, "attachment_listed": bool(attachment_nodes)},
            {"vault_path": True, "file": "assets/diagram.png"},
            "附件由固定 xdg-open 以独立参数打开，未走 shell 拼接",
        )
        check(
            "attachment_body_not_read",
            "synthetic" not in all_text(nodes()),
            {"body_leaked": "synthetic" in all_text(nodes())},
            False,
            "界面不回传附件正文",
        )

        # 刷新扫描：外部新增/删除后可见。
        (vault / "notes" / "delta.md").write_text("# 德尔塔\n\n新加入。\n", encoding="utf-8")
        (vault / "notes" / "beta.md").unlink()
        click_named("刷新扫描", role="button", contains=False, settle=1.2)
        time.sleep(3.0)
        shot("18-refreshed.png")
        dump_tree(nodes(), EVID / "tree-18-refreshed.json")
        text = all_text(nodes())
        check(
            "refresh_sees_add_and_delete",
            "delta.md" in text and "beta.md" not in text,
            {"delta_listed": "delta.md" in text, "beta_listed": "beta.md" in text},
            {"delta_listed": True, "beta_listed": False},
            "刷新扫描反映外部增删",
        )

        # 未保存草稿 → 离开分页需确认；取消后仍在知识库。
        open_note("gamma.md")
        type_in_editor(" leave-draft")
        time.sleep(0.8)
        click_named("工作台首页", role="button", settle=1.2)
        time.sleep(1.2)
        shot("19-leave-confirm.png")
        check(
            "leave_confirm_visible",
            dialog_open() and "离开知识库？" in all_text(nodes()),
            {"dialog": dialog_open()},
            True,
            "离开分页前确认，不静默丢弃草稿",
        )
        click_named("取消", role="button", contains=False, settle=1.2)
        time.sleep(1.0)
        check(
            "leave_cancel_stays",
            "leave-draft" in editor_text()
            and any(n.role == "heading" and n.name == "知识库" for n in nodes()),
            {
                "draft_kept": "leave-draft" in editor_text(),
                "still_vault": any(
                    n.role == "heading" and n.name == "知识库" for n in nodes()
                ),
            },
            {"draft_kept": True, "still_vault": True},
            "取消离开后停留且草稿保留",
        )

        # 重启不恢复未保存草稿。
        session.stop_app()
        session.start()
        pid = session.proc.pid if session.proc else 0
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("重启后没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        ax.init()
        open_vault_page()
        open_note("gamma.md")
        shot("20-restarted.png")
        text = all_text(nodes())
        check(
            "restart_drops_unsaved_draft",
            "leave-draft" not in editor_text()
            and "leave-draft" not in text
            and "没有任何 Frontmatter。" in editor_text(),
            {
                "draft_gone": "leave-draft" not in text,
                "disk_content_shown": "没有任何 Frontmatter。" in editor_text(),
            },
            {"draft_gone": True, "disk_content_shown": True},
            "重启后草稿不恢复，显示磁盘版本",
        )
    finally:
        session.stop_all()
        bus.terminate()
        xvfb.terminate()

    after = tree_snapshot(vault)
    # 验收过程中只有明确的新建/保存动作允许写入；隐藏目录与附件不得被改动。
    untouched = [".obsidian/app.json", ".trash/deleted.md", "node_modules/skip.md", "assets/diagram.png"]
    changed_untouched = [
        rel
        for rel in untouched
        if before_scan.get(rel, {}).get("sha256") != after.get(rel, {}).get("sha256")
    ]
    check(
        "untouched_entries_intact",
        not changed_untouched,
        {"changed": changed_untouched},
        [],
        "Obsidian 配置、回收站、排除目录与附件内容均未被改写",
    )
    check(
        "obsidian_config_unchanged",
        json.loads((vault / ".obsidian" / "app.json").read_text(encoding="utf-8")).get(
            "legacyEditor"
        )
        is False,
        (vault / ".obsidian" / "app.json").read_text(encoding="utf-8"),
        "原样保留",
    )

    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"checks={len(facts['checks'])} failures={len(facts['failures'])}")
    return 0 if not facts["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
