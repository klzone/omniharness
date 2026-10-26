#!/usr/bin/env python3
"""OD-KB-001 P3 隔离验收：双链解析、可重建索引与关系导航。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI）+ Rust 单元/宿主测试实测。
全部读写限定在 /tmp 下一次性合成根；不读取真实 Vault、正式资料或 Obsidian 配置，
不改 runtime/，不启动模型与网络服务。

覆盖：
  1. 未构建索引时关系面板不展示任何关系事实；
  2. 构建后统计（笔记/已解析/未解析/歧义/孤立/完整性/generation）逐项匹配；
  3. 正链状态 resolved / unresolved / ambiguous / external / attachment 可区分；
  4. 标题锚点、块锚点定位到目标笔记的正确行；
  5. 缺失锚点回退到目标笔记顶部并提示，不打开错误笔记；
  6. 反向链接跳到来源笔记的引用行（P3 修复点），不跳到目标笔记；
  7. 别名解析、孤立清单、Home 第 18 行不支持语法问题可见；
  8. 外部增删改后：刷新扫描只更新列表，重建索引才更新关系与 generation；
  9. 源文件、附件与 .obsidian 字节不变（除脚本明确改写/新增的文件）。

用法：python3 verify_knowledge_vault_p3.py [--display :135] [--root /tmp/odkb-p3-ui]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))

from window_harness import APP_BIN, AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-kb-001-p3"
SCREEN_W, SCREEN_H = 1440, 900

HOME = (
    "---\n"
    "title: Home\n"
    "aliases: [Start, 起点]\n"
    "---\n"
    "\n"
    "# Home Head\n"
    "\n"
    "[[notes/Alpha]]\n"
    "[[notes/Alpha#Details]]\n"
    "[[notes/Alpha#^blk-7]]\n"
    "[[Beta|shown-name]]\n"
    "[[Launch]]\n"
    "[[Duplicated]]\n"
    "[[MissingNote]]\n"
    "[[notes/Alpha#NoSuchHeading]]\n"
    "[[assets/diagram.png]]\n"
    "[web](https://example.com/x)\n"
    "[文字][标签]\n"
)

ALPHA = (
    "# Alpha Head\n"
    "\n"
    "## Details\n"
    "\n"
    "alpha body line five\n"
    "\n"
    "alpha block line seven ^blk-7\n"
    "\n"
    "back to [[Home]]\n"
)

BETA = "[[notes/Alpha#Details]]\n"

STARTER = (
    "---\n"
    "aliases: [Launch]\n"
    "---\n"
    "\n"
    "[[Orphan]]\n"
    "[[Home]]\n"
)

ORPHAN = "# Orphan\n\n没有入链，但会被 Starter 引用。\n"

DUPLICATED = "# Duplicated\n"


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
    (root / "Home.md").write_text(HOME, encoding="utf-8")
    (root / "Beta.md").write_text(BETA, encoding="utf-8")
    (root / "Starter.md").write_text(STARTER, encoding="utf-8")
    (root / "Orphan.md").write_text(ORPHAN, encoding="utf-8")
    (root / "Duplicated.md").write_text(DUPLICATED, encoding="utf-8")
    (root / "notes" / "Alpha.md").write_text(ALPHA, encoding="utf-8")
    (root / "notes" / "Duplicated.md").write_text(DUPLICATED, encoding="utf-8")
    (root / "assets").mkdir()
    (root / "assets" / "diagram.png").write_bytes(b"\x89PNG\r\n\x1a\n synthetic")


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
    commands = [
        ["cargo", "test", "--offline", "--lib", "vault_link"],
        ["cargo", "test", "--offline", "--lib", "vault_index"],
        ["cargo", "test", "--offline", "--test", "knowledge_vault_index_host"],
    ]
    log_parts: list[str] = []
    results: list[dict] = []
    for cmd in commands:
        proc = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, env=env)
        out = proc.stdout + proc.stderr
        log_parts.append("== " + " ".join(cmd) + " ==\n" + out)
        match = re.search(r"test result: ok\. (\d+) passed; (\d+) failed", out)
        results.append(
            {
                "command": " ".join(cmd),
                "exit_code": proc.returncode,
                "passed": int(match.group(1)) if match else 0,
                "failed": int(match.group(2)) if match else -1,
            }
        )
    (EVID / "host-test.log").write_text("\n".join(log_parts), encoding="utf-8")
    return {
        "results": results,
        "all_ok": all(r["exit_code"] == 0 and r["failed"] == 0 for r in results),
        "log": str((EVID / "host-test.log").relative_to(REPO)),
    }


def build_app_binary() -> dict:
    """按当前源码构建 debug 二进制。

    工具链脚本输出的是带 `$PATH` 的 export 行；必须在 shell 里 eval，不能像
    window_harness.ensure_app_binary 那样按 `=` 切分后直接塞进 env（会把 PATH
    写成字面量 `$PATH`，导致 pkg-config 找不到）。
    """
    setup = REPO / "scripts" / "setup-desktop-toolchain.sh"
    cmd = (
        f'eval "$(bash {setup} --print-env)" && cargo build --offline'
    )
    proc = subprocess.run(
        ["bash", "-c", cmd],
        cwd=str(REPO / "desktop" / "src-tauri"),
        capture_output=True,
        text=True,
    )
    tail = "\n".join((proc.stdout + proc.stderr).splitlines()[-8:])
    (EVID / "app-build.log").write_text(proc.stdout + proc.stderr, encoding="utf-8")
    return {
        "built": proc.returncode == 0,
        "exit_code": proc.returncode,
        "log_tail": tail,
        "binary_mtime": APP_BIN.stat().st_mtime if APP_BIN.exists() else None,
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":135")
    ap.add_argument("--root", default="/tmp/odkb-p3-ui")
    ap.add_argument("--no-build", action="store_true", help="复用现有 debug 二进制")
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

    build = {"skipped": True, "built": True}
    if not args.no_build:
        build = build_app_binary()
        build.pop("skipped", None)
    if not build.get("binary_mtime"):
        build["binary_mtime"] = APP_BIN.stat().st_mtime if APP_BIN.exists() else None
    host = run_host_tests()

    facts: dict = {
        "plan": "OD-KB-001 P3",
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ Rust 单元/宿主测试实测",
        "root": str(root),
        "synthetic_vault": str(vault),
        "app_build": build,
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
            print(f"FAIL [KB-P3] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [KB-P3] {name}: {measured}")

    check(
        "app_binary_built",
        bool(build.get("built", True)) and bool(build.get("binary_mtime")),
        build,
        {"built": True, "binary_mtime": "present"},
        "按当前工作区源码构建 debug 二进制，窗口运行的是本次代码",
    )
    check(
        "host_tests_pass",
        host["all_ok"],
        {"results": host["results"]},
        {"all_exit_code": 0, "all_failed": 0},
        "P3 解析、索引、宿主集成测试",
    )
    for result in host["results"]:
        if result["command"].endswith("vault_link"):
            check("parser_tests_pass", result["exit_code"] == 0 and result["passed"] >= 12, result, {"passed": ">=12"}, "解析器含不支持语法用例")
        if result["command"].endswith("vault_index"):
            check("index_tests_pass", result["exit_code"] == 0 and result["passed"] >= 17, result, {"passed": ">=17"}, "索引含预算/重建/原文保真用例")
        if result["command"].endswith("knowledge_vault_index_host"):
            check("host_index_tests_pass", result["exit_code"] == 0 and result["passed"] >= 5, result, {"passed": ">=5"}, "宿主关系查询与零写入")

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
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            "OMNIDESK_DSH_PROFILE": "",
            "OMNIDESK_KNOWLEDGE_ROOT": str(root / "unused-library"),
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

    def visible(node: ax.Node) -> bool:
        try:
            e = extents(node)
        except Exception:
            return False
        if e.width <= 0 or e.height <= 0:
            return False
        cx, cy = e.x + e.width // 2, e.y + e.height // 2
        return 0 <= cx < SCREEN_W and 0 <= cy < SCREEN_H

    def click(node: ax.Node | None, settle: float = 0.8) -> bool:
        if node is None or xs is None:
            return False
        if visible(node):
            e = extents(node)
            xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=settle)
            return True
        # 关系面板在 xl 三栏下独立滚动；离屏按钮用无障碍 Action 触发。
        try:
            action = node.accessible.get_action()
            if action is not None and action.get_n_actions() > 0:
                return bool(action.do_action(0))
        except Exception:
            return False
        return False

    def exact(name: str, role: str = "button") -> ax.Node | None:
        return find(nodes(), name, role=role, contains=False)

    def click_exact(name: str, role: str = "button", settle: float = 0.8) -> bool:
        return click(exact(name, role=role), settle=settle)

    def scroll_relations(down: bool = True, clicks: int = 4) -> None:
        assert xs is not None
        xs.scroll(SCREEN_W - 120, 640, clicks, up=not down, settle=0.5)

    def reveal(label: str, role: str = "button") -> ax.Node | None:
        # 右栏是独立滚动容器：每次先回到顶部，再向下找，避免上一次查找留下的滚动位置。
        scroll_relations(down=False, clicks=16)
        node = find(nodes(), label, role=role, contains=False)
        if node is not None and visible(node):
            return node
        for _ in range(10):
            scroll_relations(down=True)
            node = find(nodes(), label, role=role, contains=False)
            if node is not None and visible(node):
                return node
        return None

    def click_relation(label: str, settle: float = 1.2) -> bool:
        node = reveal(label)
        return click(node, settle=settle) if node is not None else False

    def open_vault_page() -> None:
        click(find(nodes(), "知识库", role="button"), settle=1.0)
        time.sleep(3.5)

    def open_note(file_name: str) -> bool:
        ok = click(find(nodes(), file_name, role="button"), settle=1.2)
        time.sleep(2.2)
        return ok

    def note_heading() -> str | None:
        node = find(nodes(), ".md", role="heading")
        return node.name if node is not None else None

    def outgoing_labels() -> list[str]:
        out: list[str] = []
        for n in nodes():
            name = n.name or ""
            if any(name.startswith(p) for p in ("已解析：", "未解析：", "歧义：", "外部链接：", "附件：")):
                if "；第 " in name and name not in out:
                    out.append(name)
        return out

    def backlink_labels() -> list[str]:
        out: list[str] = []
        for n in nodes():
            name = n.name or ""
            if name.startswith("来自 ") and " 第 " in name and name not in out:
                out.append(name)
        return out

    def orphan_labels() -> list[str]:
        out: list[str] = []
        for n in nodes():
            name = n.name or ""
            if name.startswith("孤立笔记：") and name not in out:
                out.append(name)
        return out

    def stats_label() -> str | None:
        node = find(nodes(), "关系索引：", contains=True)
        return node.name if node is not None else None

    def index_generation() -> int | None:
        match = re.search(r"第 (\d+) 代", all_text(nodes()))
        return int(match.group(1)) if match else None

    try:
        session.prepare()
        settings_path = session.app_data / "config" / "settings.v1.json"
        settings_path.parent.mkdir(parents=True, exist_ok=True)
        settings_path.write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "artifact_root": None,
                    "library_root": str(root / "unused-library"),
                    "knowledge_vault_root": str(vault),
                    "model_permissions": {"local": True, "private": False, "public": False},
                },
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )

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
        time.sleep(2.0)
        nodes_now = nodes()
        shot("00-before-index.png")
        dump_tree(nodes_now, EVID / "tree-00-before-index.json")
        check(
            "no_relations_before_build",
            find(nodes_now, "还没有关系索引", contains=True) is not None
            and stats_label() is None
            and not outgoing_labels(),
            {
                "placeholder": find(nodes_now, "还没有关系索引", contains=True) is not None,
                "stats": stats_label(),
                "outgoing": outgoing_labels(),
            },
            {"placeholder": True, "stats": None, "outgoing": []},
            "未构建前不展示任何关系事实",
        )

        click_exact("构建关系索引", settle=1.2)
        time.sleep(3.2)
        nodes_now = nodes()
        shot("01-index-built.png")
        dump_tree(nodes_now, EVID / "tree-01-index-built.json")
        expected_stats = (
            "关系索引：7 篇笔记；10 条已解析；1 条未解析；1 条歧义；2 篇孤立；完整"
        )
        check(
            "initial_index_stats",
            stats_label() == expected_stats and index_generation() == 1,
            {"stats": stats_label(), "generation": index_generation()},
            {"stats": expected_stats, "generation": 1},
            "7 篇笔记 / 10 resolved / 1 unresolved / 1 ambiguous / 2 孤立 / 完整",
        )
        issue_node = find(nodes_now, "索引问题：", contains=True)
        check(
            "unsupported_issue_visible",
            issue_node is not None
            and "Home.md" in issue_node.name
            and "第 18 行" in issue_node.name
            and "引用式链接" in issue_node.name,
            {"issue": issue_node.name if issue_node else None},
            {"path": "Home.md", "line": 18, "kind": "引用式链接"},
            "Home 第 18 行不支持语法显式可见，未被静默忽略",
        )

        open_note("Home.md")
        time.sleep(2.0)
        nodes_now = nodes()
        shot("02-home-relations.png")
        dump_tree(nodes_now, EVID / "tree-02-home-relations.json")
        labels = outgoing_labels()
        expected_out = {
            "已解析：notes/Alpha；第 8 行",
            "已解析：notes/Alpha；第 9 行",
            "已解析：notes/Alpha；第 10 行",
            "已解析：shown-name · Beta；第 11 行",
            "已解析：Launch；第 12 行",
            "歧义：Duplicated；第 13 行",
            "未解析：MissingNote；第 14 行",
            "已解析：notes/Alpha；第 15 行",
            "附件：assets/diagram.png；第 16 行",
            "外部链接：web · https://example.com/x；第 17 行",
        }
        check(
            "outgoing_statuses",
            set(labels) == expected_out,
            {"labels": sorted(labels), "missing": sorted(expected_out - set(labels)), "extra": sorted(set(labels) - expected_out)},
            {"exact_set": sorted(expected_out)},
            "10 条正链状态与行号逐项匹配",
        )
        check(
            "missing_anchor_marked",
            "标题锚点不存在" in all_text(nodes()),
            {"visible": [n.name for n in nodes() if "锚点不存在" in (n.name or "")][:3]},
            {"contains": "标题锚点不存在"},
            "失效锚点在关系面板显式说明",
        )
        orphans = orphan_labels()
        check(
            "orphans_match_index",
            set(orphans) == {"孤立笔记：Duplicated.md", "孤立笔记：notes/Duplicated.md"},
            {"orphans": sorted(orphans)},
            {"exact_set": ["孤立笔记：Duplicated.md", "孤立笔记：notes/Duplicated.md"]},
            "孤立清单与索引一致，被 Starter 引用的 Orphan 不在其中",
        )

        # 标题锚点：Home 第 9 行 → Alpha 第 3 行。
        clicked = click_relation("已解析：notes/Alpha；第 9 行")
        time.sleep(2.6)
        shot("03-title-anchor.png")
        dump_tree(nodes(), EVID / "tree-03-title-anchor.json")
        heading = note_heading()
        notice = find(nodes(), "已定位到第 3 行。", contains=True)
        check(
            "title_anchor_jumps_to_line",
            clicked and heading == "notes/Alpha.md" and notice is not None,
            {"clicked": clicked, "note": heading, "notice": notice.name if notice else None},
            {"note": "notes/Alpha.md", "notice": "已定位到第 3 行。"},
            "标题锚点跳到目标笔记第 3 行",
        )
        alpha_backlinks = backlink_labels()
        check(
            "alpha_backlinks_listed",
            len(alpha_backlinks) == 5
            and sum(1 for x in alpha_backlinks if x.startswith("来自 Home.md 第 ")) == 4
            and any("来自 Beta.md 第 1 行" in x for x in alpha_backlinks),
            {"backlinks": sorted(alpha_backlinks)},
            {"count": 5, "from_home": 4, "from_beta": True},
            "Alpha 的 5 条反链来自 Home 4 条与 Beta 1 条",
        )

        open_note("Home.md")
        clicked = click_relation("已解析：notes/Alpha；第 10 行")
        time.sleep(2.6)
        shot("04-block-anchor.png")
        notice = find(nodes(), "已定位到第 7 行。", contains=True)
        check(
            "block_anchor_jumps_to_line",
            clicked and note_heading() == "notes/Alpha.md" and notice is not None,
            {"clicked": clicked, "note": note_heading(), "notice": notice.name if notice else None},
            {"note": "notes/Alpha.md", "notice": "已定位到第 7 行。"},
            "块锚点 ^blk-7 跳到第 7 行",
        )

        open_note("Home.md")
        clicked = click_relation("已解析：notes/Alpha；第 15 行")
        time.sleep(2.6)
        shot("05-missing-anchor-fallback.png")
        notice = find(nodes(), "已打开目标笔记顶部，但链接里的标题或块锚点不存在。", contains=True)
        check(
            "missing_anchor_falls_back_to_note_top",
            clicked and note_heading() == "notes/Alpha.md" and notice is not None,
            {"clicked": clicked, "note": note_heading(), "notice": notice.name if notice else None},
            {"note": "notes/Alpha.md", "notice": "顶部回退提示"},
            "失效锚点只回退到目标笔记顶部，不打开错误笔记",
        )

        open_note("Home.md")
        ok = click_relation("来自 notes/Alpha.md 第 9 行：back to [[Home]]")
        time.sleep(2.8)
        shot("06-backlink-jump.png")
        dump_tree(nodes(), EVID / "tree-06-backlink-jump.json")
        notice = find(nodes(), "已定位到第 9 行。", contains=True)
        check(
            "backlink_jumps_to_source_line",
            ok and note_heading() == "notes/Alpha.md" and notice is not None,
            {
                "clicked": ok,
                "note": note_heading(),
                "notice": notice.name if notice else None,
                "backlinks": sorted(backlink_labels()),
            },
            {"note": "notes/Alpha.md", "notice": "已定位到第 9 行。"},
            "反向链接跳到来源笔记 Alpha 的引用行，而不是目标 Home",
        )

        open_note("Home.md")
        clicked = click_relation("已解析：Launch；第 12 行")
        time.sleep(2.6)
        shot("07-alias-target.png")
        check(
            "alias_resolves_to_note",
            clicked and note_heading() == "Starter.md",
            {"clicked": clicked, "note": note_heading()},
            {"note": "Starter.md"},
            "别名 Launch 解析到 Starter.md",
        )

        open_note("Home.md")
        click_relation("歧义：Duplicated；第 13 行", settle=1.0)
        time.sleep(1.8)
        shot("08-ambiguous-no-jump.png")
        check(
            "ambiguous_does_not_jump",
            note_heading() == "Home.md",
            {"note": note_heading()},
            {"note": "Home.md"},
            "歧义条目不生成错误跳转",
        )
        click_relation("未解析：MissingNote；第 14 行", settle=1.0)
        time.sleep(1.6)
        check(
            "unresolved_does_not_jump",
            note_heading() == "Home.md",
            {"note": note_heading()},
            {"note": "Home.md"},
            "未解析条目不生成错误跳转",
        )

        # 外部增删改：刷新扫描只更新列表；重建索引才更新关系与 generation。
        home_text = (vault / "Home.md").read_text(encoding="utf-8")
        (vault / "Home.md").write_text(
            home_text.replace("[[MissingNote]]", "[[Orphan]]"), encoding="utf-8"
        )
        (vault / "notes" / "Extra.md").write_text("[[Home]]\n", encoding="utf-8")
        (vault / "Orphan.md").write_text(
            ORPHAN + "外部追加一行，不含链接。\n", encoding="utf-8"
        )
        (vault / "Starter.md").write_text(
            STARTER + "[[notes/Extra]]\n", encoding="utf-8"
        )
        click_exact("刷新扫描", settle=1.2)
        time.sleep(3.0)
        shot("09-rescanned-before-rebuild.png")
        dump_tree(nodes(), EVID / "tree-09-rescanned.json")
        text = all_text(nodes())
        check(
            "scan_refresh_lists_new_note_without_reindex",
            "Extra.md" in text
            and stats_label() == expected_stats
            and index_generation() == 1,
            {
                "extra_listed": "Extra.md" in text,
                "stats": stats_label(),
                "generation": index_generation(),
            },
            {"extra_listed": True, "stats": expected_stats, "generation": 1},
            "刷新扫描看到 Extra，但关系索引仍保持旧 generation",
        )

        click_exact("重建关系索引", settle=1.2)
        time.sleep(3.4)
        shot("10-rebuilt.png")
        dump_tree(nodes(), EVID / "tree-10-rebuilt.json")
        # 重建后当前选中笔记仍是 Home，关系应立刻切到新快照。
        expected_after = (
            "关系索引：8 篇笔记；13 条已解析；0 条未解析；1 条歧义；2 篇孤立；完整"
        )
        labels = outgoing_labels()
        check(
            "rebuild_updates_stats_and_generation",
            stats_label() == expected_after and index_generation() == 2,
            {"stats": stats_label(), "generation": index_generation()},
            {"stats": expected_after, "generation": 2},
            "外部增删改后重建：8 篇 / 13 resolved / 0 unresolved / 2 孤立 / 完整",
        )
        check(
            "rebuild_updates_relations",
            "已解析：Orphan；第 14 行" in labels
            and "未解析：MissingNote；第 14 行" not in labels
            and len(labels) == 10,
            {"labels": sorted(labels)},
            {"has": "已解析：Orphan；第 14 行", "count": 10},
            "Home 第 14 行从未解析变为已解析 Orphan",
        )
    finally:
        session.stop_all()
        bus.terminate()
        xvfb.terminate()

    after = tree_snapshot(vault)
    untouched = [
        ".obsidian/app.json",
        ".trash/deleted.md",
        "node_modules/skip.md",
        "assets/diagram.png",
        "notes/Alpha.md",
        "Beta.md",
        "Duplicated.md",
        "notes/Duplicated.md",
    ]
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
        "未编辑笔记、附件、Obsidian 配置、回收站与排除目录字节不变",
    )
    expected_changed = {"Home.md", "Starter.md", "Orphan.md", "notes/Extra.md"}
    before_files = {k for k, v in before_scan.items() if v["kind"] == "file"}
    after_files = {k for k, v in after.items() if v["kind"] == "file"}
    changed_files = {
        rel
        for rel in before_files & after_files
        if before_scan[rel]["sha256"] != after[rel]["sha256"]
    }
    check(
        "only_expected_files_written",
        changed_files == {"Home.md", "Starter.md", "Orphan.md"}
        and after_files - before_files == {"notes/Extra.md"},
        {
            "changed": sorted(changed_files),
            "added": sorted(after_files - before_files),
            "removed": sorted(before_files - after_files),
        },
        {"changed": ["Home.md", "Orphan.md", "Starter.md"], "added": ["notes/Extra.md"]},
        "脚本自身只改/增明确列出的合成文件，应用没有写入知识库",
    )

    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"checks={len(facts['checks'])} failures={len(facts['failures'])}")
    return 0 if not facts["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
