#!/usr/bin/env python3
"""OD-KB-001 P4 隔离验收：全局/局部知识图谱与关系导航。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI）+ 纯函数单测与合成基准实测。
全部读写限定在 /tmp 下一次性合成根；不读取真实 Vault、正式资料或 Obsidian 配置，
不改 runtime/，不启动模型与网络服务。

覆盖：
  1. 未建索引时图谱页给出引导，不画空图；
  2. 图谱节点/连线计数与索引 stats 一致，虚节点可区分；
  3. 筛选（目录/标签/孤立/高连接度/问题引用）只影响视图，索引 generation 不变；
  4. 局部图谱以当前笔记为中心，度数可调，中心被筛掉时有明确提示；
  5. 点击节点打开对应笔记；
  6. 2000 篇合成规模降级为简化布局，界面仍可交互并打开笔记；
  7. 转换/布局耗时实测（P4-S04 阈值）；
  8. 源文件、附件与 .obsidian 字节不变。

用法：python3 verify_knowledge_vault_p4.py [--display :137] [--root /tmp/odkb-p4-ui]
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

EVID = HERE / "evidence" / "od-kb-001-p4"
SCREEN_W, SCREEN_H = 1440, 900

# 阈值（P4-S04 登记值）：2000 篇级首次可用视图不阻塞主线程。
THRESHOLD_FIRST_VIEW_MS = 400.0
THRESHOLD_FILTER_MS = 120.0
THRESHOLD_LOCAL_MS = 150.0

HOME = (
    "---\n"
    "title: Home\n"
    "tags: [index, 核心]\n"
    "aliases: [Start]\n"
    "---\n"
    "\n"
    "[[notes/Alpha]]\n"
    "[[notes/Alpha#Details]]\n"
    "[[Beta]]\n"
    "[[notes/Gamma]]\n"
    "[[MissingNote]]\n"
    "[[Duplicated]]\n"
    "[web](https://example.com/x)\n"
    "[[assets/diagram.png]]\n"
)

ALPHA = (
    "# Alpha Head\n"
    "\n"
    "## Details\n"
    "\n"
    "alpha body line five\n"
    "\n"
    "back to [[Home]]\n"
)

BETA = "---\ntags: [core]\n---\n\n[[Home]]\n[[notes/Gamma]]\n"

GAMMA = "---\ntags: [work]\n---\n\n[[Beta]]\n"

ORPHAN = "---\ntags: [work]\n---\n\n没有入链的孤立笔记。\n"

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
    (root / ".obsidian" / "app.json").write_text(json.dumps({"legacyEditor": False}), encoding="utf-8")
    (root / ".trash").mkdir()
    (root / ".trash" / "deleted.md").write_text("# 已删除\n", encoding="utf-8")
    (root / "node_modules").mkdir()
    (root / "node_modules" / "skip.md").write_text("# 跳过\n", encoding="utf-8")
    (root / "notes").mkdir()
    (root / "Home.md").write_text(HOME, encoding="utf-8")
    (root / "Beta.md").write_text(BETA, encoding="utf-8")
    (root / "Orphan.md").write_text(ORPHAN, encoding="utf-8")
    (root / "Duplicated.md").write_text(DUPLICATED, encoding="utf-8")
    (root / "notes" / "Alpha.md").write_text(ALPHA, encoding="utf-8")
    (root / "notes" / "Gamma.md").write_text(GAMMA, encoding="utf-8")
    (root / "notes" / "Duplicated.md").write_text(DUPLICATED, encoding="utf-8")
    (root / "assets").mkdir()
    (root / "assets" / "diagram.png").write_bytes(b"\x89PNG\r\n\x1a\n synthetic")


def build_large_vault(root: Path, notes: int) -> None:
    """2000 篇级合成 Vault：只为规模降级与响应性，不追求语义丰富。

    每篇两条出链（同目录下一篇、下一目录同序号篇），因此已解析边数恰为 2×notes。
    """
    root.mkdir(parents=True)
    for index in range(12):
        (root / f"d{index}").mkdir()
    per_dir: dict[int, list[str]] = {i: [] for i in range(12)}
    for index in range(notes):
        directory = index % 12
        name = f"note-{index:04d}.md"
        per_dir[directory].append(name)
    for directory, names in per_dir.items():
        for position, name in enumerate(names):
            target_a = names[(position + 1) % len(names)]
            stem_a = target_a[:-3]
            stem_b = per_dir[(directory + 1) % 12][position % len(per_dir[(directory + 1) % 12])][:-3]
            # 索引解析第 1 层匹配「完整相对路径」，因此这里带 .md 写全路径；
            # 不带扩展名的嵌套全路径不会被第 3 层识别（见 STATUS.md 的 P3 语义边界记录）。
            body = (
                f"# {name}\n\n"
                f"[[d{directory}/{stem_a}.md]]\n"
                f"[[d{(directory + 1) % 12}/{stem_b}.md]]\n"
            )
            (root / f"d{directory}" / name).write_text(body, encoding="utf-8")


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


def run_unit_tests() -> dict:
    cwd = REPO / "desktop" / "src" / "shell"
    commands = [
        ["node", "--experimental-strip-types", "--test", "knowledgeGraph.test.mjs"],
        ["node", "--experimental-strip-types", "--test", "knowledgeGraphLayout.test.mjs"],
    ]
    log_parts: list[str] = []
    results: list[dict] = []
    for cmd in commands:
        proc = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True)
        out = proc.stdout + proc.stderr
        log_parts.append("== " + " ".join(cmd) + " ==\n" + out)
        tests = re.search(r"# tests (\d+)", out)
        passed = re.search(r"# pass (\d+)", out)
        failed = re.search(r"# fail (\d+)", out)
        results.append(
            {
                "command": " ".join(cmd),
                "exit_code": proc.returncode,
                "tests": int(tests.group(1)) if tests else 0,
                "passed": int(passed.group(1)) if passed else 0,
                "failed": int(failed.group(1)) if failed else -1,
            }
        )
    (EVID / "unit-test.log").write_text("\n".join(log_parts), encoding="utf-8")
    return {
        "results": results,
        "all_ok": all(r["exit_code"] == 0 and r["failed"] == 0 and r["passed"] > 0 for r in results),
    }


def run_benchmark() -> dict:
    proc = subprocess.run(
        ["node", "--experimental-strip-types", "knowledge-graph-p4.mjs"],
        cwd=str(REPO / "desktop" / "bench"),
        capture_output=True,
        text=True,
    )
    (EVID / "bench.log").write_text(proc.stdout + proc.stderr, encoding="utf-8")
    cases = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                cases.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return {"exit_code": proc.returncode, "cases": cases}


def build_app_binary() -> dict:
    setup = REPO / "scripts" / "setup-desktop-toolchain.sh"
    cmd = f'eval "$(bash {setup} --print-env)" && cargo build --offline'
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
    ap.add_argument("--display", default=":137")
    ap.add_argument("--root", default="/tmp/odkb-p4-ui")
    ap.add_argument("--no-build", action="store_true", help="复用现有 debug 二进制")
    ap.add_argument("--large-notes", type=int, default=2000)
    ap.add_argument(
        "--evidence-dir",
        default=None,
        help="证据输出目录；独立审查请指向 evidence/od-kb-001-p4-review，避免覆盖原始证据",
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
    large = (root / "large-vault").resolve()
    build_large_vault(large, args.large_notes)
    before_scan = tree_snapshot(vault)
    before_large = tree_snapshot(large)

    build = {"skipped": True, "built": True}
    if not args.no_build:
        build = build_app_binary()
        build.pop("skipped", None)
    if not build.get("binary_mtime"):
        build["binary_mtime"] = APP_BIN.stat().st_mtime if APP_BIN.exists() else None
    host = run_host_tests()
    units = run_unit_tests()
    bench = run_benchmark()

    facts: dict = {
        "plan": "OD-KB-001 P4",
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 纯函数单测 + 合成基准实测",
        "root": str(root),
        "synthetic_vault": str(vault),
        "large_vault": str(large),
        "large_notes": args.large_notes,
        "app_build": build,
        "host_tests": host,
        "unit_tests": units,
        "benchmark": bench,
        "thresholds": {
            "first_view_ms": THRESHOLD_FIRST_VIEW_MS,
            "filter_ms": THRESHOLD_FILTER_MS,
            "local_ms": THRESHOLD_LOCAL_MS,
        },
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
            print(f"FAIL [KB-P4] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [KB-P4] {name}: {measured}")

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
        "P3 解析/索引/宿主测试作为 P4 回归",
    )
    check(
        "unit_tests_pass",
        units["all_ok"],
        {"results": units["results"]},
        {"all_failed": 0, "passed": ">0"},
        "P4 图数据转换与布局纯函数单测",
    )

    cases = {case["case"]: case for case in bench.get("cases", [])}
    large_case = cases.get(f"{args.large_notes}notes")
    check(
        "benchmark_cases_present",
        len(cases) >= 3,
        {"cases": sorted(cases)},
        {"count": ">=3"},
        "200/1000/2000 三档合成规模都完成实测",
    )
    if large_case is not None:
        check(
            "large_first_view_within_budget",
            large_case["firstViewMs"] <= THRESHOLD_FIRST_VIEW_MS,
            large_case["firstViewMs"],
            {"max_ms": THRESHOLD_FIRST_VIEW_MS},
            f"{args.large_notes} 篇规模：转换 + 筛选 + 首次布局总耗时",
        )
        check(
            "large_layout_simplified",
            large_case["simplified"] is True and large_case["folded"] > 0,
            {"simplified": large_case["simplified"], "folded": large_case["folded"]},
            {"simplified": True, "folded": ">0"},
            "超过 600 节点阈值时降级为简化布局，界面可据此提示",
        )
        check(
            "layout_is_deterministic",
            large_case["stable"] is True,
            {"stable": large_case["stable"]},
            {"stable": True},
            "同输入重排坐标完全一致（无随机数）",
        )
        check(
            "graph_stats_match_index",
            large_case["graphStats"]["notes"] == large_case["notes"]
            and large_case["graphStats"]["resolvedEdges"] == large_case["edges"]
            and large_case["graphStats"]["danglingResolved"] == 0,
            large_case["graphStats"],
            {"notes": large_case["notes"], "resolved": large_case["edges"], "dangling": 0},
            "转换后的节点/边数与索引统计一致",
        )
    if "200notes" in cases:
        check(
            "small_first_view_within_budget",
            cases["200notes"]["firstViewMs"] <= THRESHOLD_FIRST_VIEW_MS,
            cases["200notes"]["firstViewMs"],
            {"max_ms": THRESHOLD_FIRST_VIEW_MS},
            "200 篇规模首次视图耗时",
        )
        check(
            "small_layout_not_simplified",
            cases["200notes"]["simplified"] is False
            and cases["200notes"]["placed"] == cases["200notes"]["notes"],
            {"simplified": cases["200notes"]["simplified"], "placed": cases["200notes"]["placed"]},
            {"simplified": False, "placed": 200},
            "阈值内不降级，全部节点参与布局",
        )

    bus_addr, bus = start_dbus()
    xvfb = start_display(args.display)
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = bus_addr
    os.environ["DISPLAY"] = args.display
    if os.path.exists("/run/user/1000/at-spi/bus"):
        os.environ.setdefault("AT_SPI_BUS_ADDRESS", "unix:path=/run/user/1000/at-spi/bus")

    def make_session(session_root: Path, vault_root: Path) -> AppSession:
        return AppSession(
            root=session_root,
            display=args.display,
            artifact_override=True,
            reuse_running_dev_server=True,
            env_extra={
                "DBUS_SESSION_BUS_ADDRESS": bus_addr,
                "OMNIDESK_DSH_PROFILE": "",
                "OMNIDESK_KNOWLEDGE_ROOT": str(session_root / "unused-library"),
            },
        )

    def write_settings(path: Path, vault_root: Path) -> None:
        """写入隔离的设置文件；knowledge_vault_root 指向本次合成 Vault。

        知识库根只来自该设置项：OMNIDESK_KNOWLEDGE_ROOT 覆盖的是 library_root（成果库），
        与知识库无关，因此两者各自指向 /tmp 下不同目录。
        """
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "artifact_root": None,
                    "library_root": str(path.parents[3] / "unused-library"),
                    "knowledge_vault_root": str(vault_root),
                    "model_permissions": {"local": True, "private": False, "public": False},
                },
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )

    session = make_session(root / "small-session", vault)

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

    def open_vault_page() -> None:
        click(find(nodes(), "知识库", role="button"), settle=1.0)
        time.sleep(3.5)

    def open_graph(settle: float = 1.6) -> bool:
        # 「笔记 / 图谱」切换按钮带 aria-pressed，WebKit 暴露为 toggle button 而不是 button；
        # 按 role="button" 查找会静默失败并留在笔记视图，因此这里不限定 role。
        ok = click_exact("图谱", role="toggle button", settle=settle)
        if not ok:
            ok = click_exact("图谱", settle=settle)
        time.sleep(1.4)
        return ok and on_graph()

    def on_graph() -> bool:
        # 「还没有关系索引」这句在笔记视图侧栏也有，不能用来判断当前是哪个视图；
        # 图谱视图的独有标记是「关系图谱」标题与其无索引引导的第二段。
        return find(nodes(), "关系图谱", contains=True) is not None or graph_empty_guide() is not None

    def graph_empty_guide() -> ax.Node | None:
        return find(nodes(), "图谱只从当前关系索引绘制", contains=True)

    def graph_count_text() -> str | None:
        node = find(nodes(), "图谱视图：", contains=True)
        return node.name if node is not None else None

    def wait_until(predicate, timeout: float = 20.0, interval: float = 0.5) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                if predicate():
                    return True
            except Exception:
                pass
            time.sleep(interval)
        return False

    def counts() -> dict:
        text = graph_count_text() or ""
        out: dict = {"raw": text}
        for key, pattern in (
            ("notes", r"图谱视图：(\d+) 篇笔记"),
            ("placeholders", r"；(\d+) 个未解析或歧义"),
            ("edges", r"；(\d+) 条连线"),
            ("hidden", r"筛选隐藏 (\d+) 条连线"),
        ):
            match = re.search(pattern, text)
            out[key] = int(match.group(1)) if match else None
        return out

    def node_labels() -> list[str]:
        out: list[str] = []
        for n in nodes():
            name = n.name or ""
            if name.startswith(("笔记：", "未解析引用：", "歧义引用：")):
                out.append(name)
        return out

    def click_node(prefix: str, settle: float = 1.6) -> bool:
        for n in nodes():
            if (n.name or "").startswith(prefix):
                return click(n, settle=settle)
        return False

    def select_option(label: str, downs: int, settle: float = 1.4) -> bool:
        """对原生 <select>：聚焦后用 Home 回到第一项，再按 Down 计数后回车。

        WebKit 把 <select> 暴露为 combo box；展开的原生下拉菜单不在无障碍树里，
        所以键盘选择比点击菜单项更可靠。
        """
        node = find(nodes(), label, role="combo box", contains=False)
        if node is None:
            node = find(nodes(), label, contains=False)
        if node is None or xs is None:
            return False
        e = extents(node)
        if e.width <= 0 or e.height <= 0:
            return False
        xs.click(e.x + e.width // 2, e.y + e.height // 2, settle=0.5)
        xs.key("Home", settle=0.35)
        for _ in range(downs):
            xs.key("Down", settle=0.3)
        xs.key("Return", settle=settle)
        # 实测：原生 <select> 展开再回车后，WebKit 会吞掉紧随其后的第一次合成点击
        # （同坐标第二次点击才生效，见 evidence 里的 probe 说明）。这里点一下侧栏
        # 非交互标题做一次无害的中性点击，把指针状态交还给页面。
        header = find(nodes(), "图谱筛选", role="heading", contains=False)
        if header is not None:
            he = extents(header)
            if he.width > 0 and he.height > 0:
                xs.click(he.x + he.width // 2, he.y + he.height // 2, settle=0.5)
        return True

    def zoom_level() -> str | None:
        node = find(nodes(), "%", contains=True)
        return node.name if node is not None else None

    def first_node(prefix: str) -> ax.Node | None:
        for n in nodes():
            if (n.name or "").startswith(prefix):
                return n
        return None

    def center_of(node: ax.Node | None) -> tuple[int, int] | None:
        if node is None:
            return None
        try:
            e = extents(node)
        except Exception:
            return None
        if e.width <= 0 or e.height <= 0:
            return None
        return (e.x + e.width // 2, e.y + e.height // 2)

    def note_heading() -> str | None:
        node = find(nodes(), ".md", role="heading")
        return node.name if node is not None else None

    def activate_ax(node: ax.Node | None) -> bool:
        """只通过无障碍动作激活，不合成坐标点击。"""
        if node is None:
            return False
        try:
            action = node.accessible.get_action()
            if action is None or action.get_n_actions() <= 0:
                return False
            return bool(action.do_action(0))
        except Exception:
            return False

    def simplified_numbers() -> dict:
        node = find(nodes(), "已简化显示", contains=True)
        text = node.name if node is not None else ""
        placed = re.search(r"只布局入链最多的\s*(\d+)\s*个节点", text)
        folded = re.search(r"另有\s*(\d+)\s*个未展开", text)
        total = re.search(r"当前视图共\s*(\d+)\s*个节点", text)
        return {
            "text": text,
            "total": int(total.group(1)) if total else None,
            "placed": int(placed.group(1)) if placed else None,
            "folded": int(folded.group(1)) if folded else None,
        }

    try:
        session.prepare()
        settings_path = session.app_data / "config" / "settings.v1.json"
        write_settings(settings_path, vault)

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
        open_graph()
        shot("00-graph-no-index.png")
        dump_tree(nodes(), EVID / "tree-00-no-index.json")
        guide = graph_empty_guide()
        check(
            "graph_guides_without_index",
            guide is not None
            and find(nodes(), "vault-graph-node", contains=True) is None,
            {
                "graph_view": guide is not None,
                "nodes": len(node_labels()),
            },
            {"guide": True, "nodes": 0},
            "未建索引时给出引导，不画空图",
        )

        click_exact("构建关系索引", settle=1.2)
        if not wait_until(lambda: (counts()["notes"] or 0) > 0, 30.0):
            shot("01-graph-build-attempt.png")
            dump_tree(nodes(), EVID / "tree-01-build-attempt.json")
            alive = session.alive()
            busy = find(nodes(), "构建中", contains=True) is not None
            failure = find(nodes(), "索引构建失败", contains=True)
            raise RuntimeError(
                "小 Vault 索引构建后图谱未出现笔记节点："
                f"counts={counts()} app_alive={alive} busy={busy} "
                f"index_error={failure.name if failure is not None else None}"
            )
        time.sleep(1.2)
        shot("01-graph-built.png")
        dump_tree(nodes(), EVID / "tree-01-built.json")
        initial = counts()
        # 合成夹具（源码事实）：7 篇笔记；Home 的 4 条 resolved + Alpha/Beta/Gamma 各 1/2/1 条
        # = 8 条已解析边；另有 2 条虚边（MissingNote 未解析、Duplicated 歧义），故图上连线 10 条。
        check(
            "graph_counts_match_index",
            initial["notes"] == 7 and initial["edges"] == 10 and initial["placeholders"] == 2,
            initial,
            {"notes": 7, "edges": 10, "placeholders": 2},
            "7 篇笔记 / 8 条已解析边 + 2 条虚边 / 未解析+歧义各一条",
        )
        labels = node_labels()
        check(
            "placeholder_nodes_distinguishable",
            any(label.startswith("未解析引用：MissingNote") for label in labels)
            and any(label.startswith("歧义引用：Duplicated") for label in labels),
            {"labels": sorted(labels)},
            {"unresolved": "MissingNote", "ambiguous": "Duplicated"},
            "未解析与歧义在图里各自成为带状态的虚节点",
        )
        check(
            "note_nodes_carry_link_counts",
            any("笔记：notes/Alpha.md；入链 2" in label for label in labels)
            and any("孤立" in label and "Orphan.md" in label for label in labels),
            {"labels": sorted(labels)},
            {"alpha": "入链 2", "orphan": True},
            "笔记节点的入链数（Home 两条指向 Alpha）与孤立标记来自索引",
        )

        def canvas_box() -> tuple[int, int, int, int] | None:
            node = find(nodes(), "关系图谱画布", contains=True)
            if node is None:
                return None
            e = extents(node)
            if e.width <= 0 or e.height <= 0:
                return None
            return (e.x, e.y, e.width, e.height)

        def nodes_inside_canvas() -> dict:
            box = canvas_box()
            if box is None:
                return {"canvas": None, "outside": [], "count": 0}
            cx, cy, cw, ch = box
            outside = []
            for n in nodes():
                if not (n.name or "").startswith(("笔记：", "未解析引用：", "歧义引用：")):
                    continue
                try:
                    e = extents(n)
                except Exception:
                    continue
                # 节点矩形必须完整落在画布内，允许 1px 边框取整误差。
                if (
                    e.x < cx - 1
                    or e.y < cy - 1
                    or e.x + e.width > cx + cw + 1
                    or e.y + e.height > cy + ch + 1
                ):
                    outside.append({"name": n.name.split("；")[0], "box": [e.x, e.y, e.width, e.height]})
            return {"canvas": [cx, cy, cw, ch], "outside": outside, "count": len(outside)}

        inside = nodes_inside_canvas()
        check(
            "nodes_laid_out_inside_canvas",
            inside["canvas"] is not None and inside["count"] == 0,
            inside,
            {"outside": 0},
            "布局按画布实测尺寸计算，所有节点都在可视画布内",
        )
        generation_before = find(nodes(), "第 1 代", contains=True) is not None

        # 根目录筛选：<select> 用哨兵值区分「全部目录」和「（根目录）」。
        # 若两者共用空串，选中根目录会静默恢复全图，这里必须看到筛选真的收敛。
        root_selected = select_option("目录筛选", downs=1)
        time.sleep(1.2)
        shot("02a-graph-root-directory.png")
        dump_tree(nodes(), EVID / "tree-02a-root-directory.json")
        root_counts = counts()
        # 根目录 4 篇：Home、Beta、Orphan、Duplicated；两条虚引用边 + Home↔Beta 两条边。
        check(
            "root_directory_filter_is_distinct",
            root_selected
            and root_counts["notes"] == 4
            and root_counts["edges"] == 4
            and root_counts["placeholders"] == 2
            and root_counts["hidden"] == 6,
            {"selected": root_selected, **root_counts},
            {"notes": 4, "edges": 4, "placeholders": 2, "hidden": 6},
            "选择「（根目录）」只留根目录笔记，不退回全部目录",
        )
        click_exact("清空筛选", settle=1.2)
        time.sleep(1.0)

        # 读屏激活：只调用无障碍 action，不合成鼠标坐标。按钮必须能打开笔记。
        ax_target = first_node("笔记：Orphan.md")
        ax_fired = activate_ax(ax_target)
        time.sleep(2.0)
        shot("02b-graph-a11y-activate.png")
        check(
            "node_accessibility_action_opens_note",
            ax_fired and note_heading() == "Orphan.md",
            {"do_action": ax_fired, "note": note_heading()},
            {"note": "Orphan.md"},
            "读屏/无障碍动作可直接激活节点打开笔记（不依赖指针坐标）",
        )
        if not open_graph():
            raise RuntimeError("读屏激活后未能回到图谱视图")
        time.sleep(1.0)
        restored = counts()
        check(
            "a11y_activation_keeps_index",
            restored["notes"] == 7 and restored["edges"] == 10,
            restored,
            {"notes": 7, "edges": 10},
            "从笔记返回图谱后索引与视图事实不变",
        )

        # 目录筛选：只留 notes/ 三篇（Alpha、Gamma、notes/Duplicated），范围内没有两端都保留的边。
        # 选项顺序固定为「全部目录」+ graph.directories（字典序）："" 排在 d0… 之前，
        # 这里的目标是第三项 notes/，故 Home + 两次 Down。
        ok = select_option("目录筛选", downs=2)
        time.sleep(1.2)
        shot("02-graph-directory-filter.png")
        dump_tree(nodes(), EVID / "tree-02-directory-filter.json")
        filtered = counts()
        check(
            "directory_filter_converges_view",
            filtered["notes"] == 3 and filtered["edges"] == 0,
            {"applied": ok, **filtered},
            {"notes": 3, "edges": 0},
            "目录筛选只留 notes/ 的 Alpha、Gamma 与 Duplicated；跨目录边随之收敛",
        )
        open_note = click_node("笔记：notes/Alpha.md", settle=2.2)
        time.sleep(2.0)
        shot("03-node-opens-note.png")
        check(
            "node_click_opens_note",
            open_note and note_heading() == "notes/Alpha.md",
            {"clicked": open_note, "note": note_heading()},
            {"note": "notes/Alpha.md"},
            "点击图谱节点打开对应笔记",
        )

        # 回到图谱：索引与 generation 不变。
        # 筛选状态属于视图本地 state（组件在切回笔记视图时卸载），重新进入图谱回到完整视图；
        # 这里核对的是「切视图不重建索引」，不把状态重置当作失败。
        open_graph()
        after_open = counts()
        check(
            "view_switch_keeps_index",
            find(nodes(), "第 1 代", contains=True) is not None
            and after_open["notes"] == 7,
            {"generation": "第 1 代" if find(nodes(), "第 1 代", contains=True) else None, **after_open},
            {"generation": "第 1 代", "notes": 7},
            "切换视图不重建索引；重进图谱为完整视图（筛选是视图本地状态）",
        )

        click_exact("清空筛选", settle=1.4)
        time.sleep(1.0)
        cleared = counts()
        check(
            "clear_filters_restores",
            cleared["notes"] == 7 and cleared["edges"] == 10,
            cleared,
            {"notes": 7, "edges": 10},
            "清空筛选恢复完整视图",
        )

        # 局部图谱：以当前笔记（Alpha）为中心，默认 1 度。
        click_exact("当前笔记", settle=1.4)
        time.sleep(1.0)
        shot("04-graph-local.png")
        dump_tree(nodes(), EVID / "tree-04-local.json")
        local = counts()
        # 1 度邻域 = {Alpha, Home}；虚节点是 Home 的邻居，不在 1 度内，因此只画 2 篇 3 条边
        # （Home→Alpha 两条 + Alpha→Home 一条）。
        check(
            "local_scope_centered_on_note",
            local["notes"] == 2
            and local["edges"] == 3
            and local["placeholders"] == 0
            and find(nodes(), "中心：notes/Alpha.md", contains=True) is not None,
            {"center": find(nodes(), "中心：notes/Alpha.md", contains=True) is not None, **local},
            {"notes": 2, "edges": 3, "placeholders": 0, "center": "notes/Alpha.md"},
            "局部图谱以当前笔记为中心，1 度只含 Home",
        )

        # 度数改到 3：Beta、Gamma 进入 2 度邻域，Alpha↔Home↔Beta/Gamma 的边一并出现。
        if select_option("局部图谱度数", downs=2):
            time.sleep(1.2)
            shot("05-graph-depth-3.png")
            expanded = counts()
            check(
                "depth_expands_neighborhood",
                expanded["notes"] == 4 and expanded["edges"] == 10 and expanded["placeholders"] == 2,
                {"depth1": {"notes": local["notes"], "edges": local["edges"]}, "depth3": expanded},
                {"notes": 4, "edges": 10, "placeholders": 2},
                "度数从 1 加到 3 后纳入 Beta、Gamma 及 Home 的两条虚引用边",
            )
        else:
            check(
                "depth_expands_neighborhood",
                False,
                {"depth": "不可见"},
                {"visible": True},
                "度数控件必须可操作",
            )

        click_exact("全局", settle=1.2)
        time.sleep(0.8)

        # 孤立筛选。
        orphans = find(nodes(), "只看孤立笔记", role="check box")
        check(
            "orphan_filter_matches_index",
            click(orphans, settle=1.4),
            {"clicked": orphans is not None},
            {"clicked": True},
            "孤立筛选控件可操作（结果在下一个断言核对）",
        )
        time.sleep(1.0)
        shot("06-graph-orphans.png")
        orphan_counts = counts()
        orphan_labels = node_labels()
        # 索引事实：Orphan.md 与两篇同名 Duplicated.md 都没有入链（歧义引用不算入链）。
        check(
            "orphan_filter_result",
            orphan_counts["notes"] == 3
            and orphan_counts["edges"] == 0
            and sorted(label for label in orphan_labels if label.startswith("笔记："))
            == ["笔记：Duplicated.md；入链 0；孤立", "笔记：Orphan.md；入链 0；孤立", "笔记：notes/Duplicated.md；入链 0；孤立"],
            {"counts": orphan_counts, "labels": sorted(orphan_labels)},
            {"notes": 3, "edges": 0, "notes_list": ["Duplicated.md", "Orphan.md", "notes/Duplicated.md"]},
            "孤立筛选与索引 orphans 逐条一致（歧义引用不产生入链）",
        )
        click_exact("清空筛选", settle=1.2)
        time.sleep(0.8)

        # 拖动节点：位置变化而不打开笔记，计数不变。
        drag_target = first_node("笔记：Orphan.md")
        before_drag = center_of(drag_target)
        dragged = False
        if before_drag is not None and xs is not None:
            xs.drag(before_drag[0], before_drag[1], before_drag[0] + 90, before_drag[1] + 60, settle=1.0)
            time.sleep(0.8)
            after_drag = center_of(first_node("笔记：Orphan.md"))
            dragged = (
                after_drag is not None
                and abs(after_drag[0] - before_drag[0] - 90) <= 14
                and abs(after_drag[1] - before_drag[1] - 60) <= 14
            )
        shot("07-graph-dragged.png")
        dump_tree(nodes(), EVID / "tree-07-dragged.json")
        drag_counts = counts()
        check(
            "node_drag_moves_without_opening",
            dragged
            and drag_counts["notes"] == 7
            and find(nodes(), "中心：", contains=True) is None
            and find(nodes(), "关系图谱", contains=True) is not None,
            {
                "before": before_drag,
                "after": center_of(first_node("笔记：Orphan.md")),
                "still_on_graph": find(nodes(), "关系图谱", contains=True) is not None,
                **drag_counts,
            },
            {"moved_by": [90, 60], "notes": 7, "opened_note": False},
            "拖动节点只改视图坐标，不触发打开笔记、不丢其他节点",
        )

        # 缩放按钮：百分比随之变化，节点与边数不变。
        zoom_before = zoom_level()
        click_exact("放大图谱", role="button", settle=1.0)
        zoom_after = zoom_level()
        shot("08-graph-zoomed.png")
        zoom_counts = counts()
        check(
            "zoom_changes_scale_only",
            zoom_before is not None
            and zoom_after is not None
            and zoom_before != zoom_after
            and zoom_counts["notes"] == 7
            and zoom_counts["edges"] == 10,
            {"before": zoom_before, "after": zoom_after, **zoom_counts},
            {"scale_changed": True, "notes": 7, "edges": 10},
            "缩放只改视图比例，节点/连线事实不变",
        )
        click_exact("适应窗口", role="button", settle=0.9)

        # 当前笔记不在索引里：建索引后新建一篇笔记，再切回图谱选「当前笔记」。
        # 必须显示明确提示和空图，而不是静默用全局图冒充局部关系。
        session.stop_all()
        time.sleep(1.5)
        center_vault = (root / "center-vault").resolve()
        build_vault(center_vault)
        session = make_session(root / "center-session", center_vault)
        session.prepare()
        write_settings(session.app_data / "config" / "settings.v1.json", center_vault)
        session.start()
        pid = session.proc.pid if session.proc else 0
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("中心缺失会话没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        open_vault_page()
        time.sleep(2.0)
        open_graph(settle=1.8)
        click_exact("构建关系索引", settle=1.5)
        if not wait_until(lambda: (counts()["notes"] or 0) > 0, 30.0):
            raise RuntimeError(f"中心缺失会话索引构建失败：{counts()}")
        time.sleep(1.0)
        click_exact("笔记", role="toggle button", settle=1.4)
        time.sleep(1.6)
        click_exact("新建笔记", settle=1.0)
        time.sleep(1.0)
        path_entry = find(nodes(), "笔记/新想法.md", role="entry")
        if path_entry is None:
            path_entry = find(nodes(), "相对路径（父目录必须已存在）")
        if path_entry is None:
            raise RuntimeError("找不到新建笔记路径输入框")
        pe = extents(path_entry)
        xs.click(pe.x + pe.width // 2, pe.y + pe.height // 2, settle=0.4)
        xs.select_all_and_clear()
        xs.type_ascii("notes/not-indexed.md")
        time.sleep(0.6)
        click_exact("创建笔记", settle=1.2)
        time.sleep(3.0)
        created_not_indexed = (center_vault / "notes" / "not-indexed.md").is_file()
        generation_still_first = find(nodes(), "第 1 代", contains=True) is not None
        open_graph(settle=1.8)
        time.sleep(1.4)
        click_exact("当前笔记", settle=1.4)
        time.sleep(1.4)
        shot("09-graph-center-missing.png")
        dump_tree(nodes(), EVID / "tree-09-center-missing.json")
        missing_counts = counts()
        missing_banner = find(nodes(), "不在关系索引中", contains=True)
        check(
            "local_scope_reports_missing_center",
            created_not_indexed
            and generation_still_first
            and missing_banner is not None
            and missing_counts["notes"] == 0
            and missing_counts["edges"] == 0
            and len(node_labels()) == 0,
            {
                "created": created_not_indexed,
                "index_generation_unchanged": generation_still_first,
                "banner": missing_banner is not None,
                **missing_counts,
                "nodes": len(node_labels()),
            },
            {"banner": True, "notes": 0, "edges": 0, "nodes": 0},
            "中心不在索引时给出提示并画空图，不退回全局 7 篇笔记",
        )

        # 2000 篇规模：另起一个隔离会话（独立数据根、settings 指向大 Vault），
        # 不复用上一会话的设置页，避免「重新读取」依赖界面流程。
        session.stop_all()
        time.sleep(1.5)
        session = make_session(root / "large-session", large)
        session.prepare()
        write_settings(session.app_data / "config" / "settings.v1.json", large)
        session.start()
        pid = session.proc.pid if session.proc else 0
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("大 Vault 会话没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        open_vault_page()
        time.sleep(2.0)
        open_graph(settle=1.8)
        big_started = time.monotonic()
        click_exact("构建关系索引", settle=1.5)
        if not wait_until(lambda: (counts()["notes"] or 0) > 0, 90.0):
            raise RuntimeError(f"{args.large_notes} 篇索引构建后图谱未出现节点：{counts()}")
        big_ready = time.monotonic() - big_started
        time.sleep(1.5)
        shot("07-graph-large.png")
        dump_tree(nodes(), EVID / "tree-07-large.json")
        big = counts()
        simplified = find(nodes(), "已简化显示", contains=True)
        big_numbers = simplified_numbers()
        check(
            "large_vault_renders_simplified",
            simplified is not None
            and big["notes"] == args.large_notes
            and big_numbers["total"] == args.large_notes
            and big_numbers["placed"] == 600
            and big_numbers["folded"] == args.large_notes - 600,
            {
                "simplified": simplified is not None,
                "wall_seconds": round(big_ready, 1),
                "numbers": big_numbers,
                **big,
            },
            {"simplified": True, "notes": args.large_notes, "placed": 600, "folded": args.large_notes - 600},
            f"{args.large_notes} 篇合成 Vault 初始只布局 600 个核心节点，其余折叠并计数",
        )
        facts["large_first_view_wall_seconds"] = round(big_ready, 2)

        # 降级不是死路：界面提供「展开更多」，每次增加 600 的布局预算，
        # 折叠数随之下降，但筛选/索引事实不变。
        before_expand = simplified_numbers()
        expand_button = find(nodes(), "展开更多", role="button", contains=False)
        expanded_ok = click(expand_button, settle=2.0)
        time.sleep(2.0)
        shot("07a-graph-large-expanded.png")
        dump_tree(nodes(), EVID / "tree-07a-large-expanded.json")
        after_expand = simplified_numbers()
        expanded_counts = counts()
        check(
            "large_vault_expands_progressively",
            expanded_ok
            and before_expand["placed"] is not None
            and after_expand["placed"] is not None
            and after_expand["placed"] > before_expand["placed"]
            and (before_expand["folded"] or 0) > (after_expand["folded"] or 0)
            and expanded_counts["notes"] == args.large_notes,
            {"before": before_expand, "after": after_expand, **expanded_counts},
            {"placed": "increases", "folded": "decreases", "notes": args.large_notes},
            "大图可手动展开更多节点，不会一次布局全部 2000 个",
        )

        # 折叠按入链数选核心节点，具体哪篇在外围不可预知；600 个按钮在画布上会互相覆盖，
        # 取「中心点半径内没有更晚渲染节点覆盖」的第一个笔记节点，避免点到叠在上面的邻居。
        def unoccluded_note() -> ax.Node | None:
            """返回一个中心点未被任何后绘制节点覆盖的笔记节点。

            DOM 后渲染的兄弟节点盖在上面，所以命中测试要按绘制顺序检查：
            中心点落在某个更靠后的节点矩形内，就说明这次点击不会落到它身上。
            """
            boxes = []
            for n in nodes():
                if not (n.name or "").startswith("笔记："):
                    continue
                try:
                    e = extents(n)
                except Exception:
                    continue
                boxes.append((n, e.x, e.y, e.width, e.height))
            for index, (node, x, y, w, h) in enumerate(boxes):
                cx, cy = x + w // 2, y + h // 2
                covered = any(
                    ox <= cx <= ox + ow and oy <= cy <= oy + oh
                    for _, ox, oy, ow, oh in boxes[index + 1 :]
                )
                if not covered:
                    return node
            return None

        big_note = unoccluded_note()
        big_note_label = big_note.name if big_note is not None else ""
        big_note_path = big_note_label[len("笔记：") :].split("；")[0] if big_note_label else ""
        opened_big = click(big_note, settle=2.2)
        time.sleep(2.0)
        shot("08-large-opens-note.png")
        check(
            "large_vault_node_opens_note",
            opened_big and big_note_path.endswith(".md") and note_heading() == big_note_path,
            {"clicked": opened_big, "clicked_label": big_note_label, "note": note_heading()},
            {"note": big_note_path},
            "降级视图里节点仍可打开笔记",
        )
    finally:
        session.stop_all()
        bus.terminate()
        xvfb.terminate()

    after = tree_snapshot(vault)
    changed = [
        rel
        for rel in before_scan
        if before_scan[rel].get("sha256") != after.get(rel, {}).get("sha256")
    ]
    check(
        "synthetic_vault_untouched",
        not changed,
        {"changed": changed},
        [],
        "P4 全程没有写入合成 Vault（含附件、Obsidian 配置与排除目录）",
    )
    after_large = tree_snapshot(large)
    large_changed = [
        rel
        for rel in before_large
        if before_large[rel].get("sha256") != after_large.get(rel, {}).get("sha256")
    ]
    check(
        "large_vault_untouched",
        not large_changed,
        {"changed": large_changed[:5], "count": len(large_changed)},
        [],
        "2000 篇合成 Vault 在索引与图谱操作后字节不变",
    )

    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"checks={len(facts['checks'])} failures={len(facts['failures'])}")
    return 0 if not facts["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

