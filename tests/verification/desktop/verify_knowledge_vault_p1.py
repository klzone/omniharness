#!/usr/bin/env python3
"""OD-KB-001 P1 隔离验收：知识库根只读访问（设置页入口 + 合成根扫描）。

证据等级：真实桌面窗口（一次性 Xvfb + XTEST + AT-SPI）+ Rust 宿主集成测试实测。
所有读写都限定在 /tmp 下一次性合成根；不读取 ~/OmniDesk/库、正式知识库或真实
Obsidian vault，不改 runtime/，不启动模型。

验证链：
  1. 未设置知识库：设置页显示「未设置」；
  2. 预置合成 Obsidian 根（含 .obsidian/、附件、子目录）后重启：显示根路径与目录状态；
  3. 点击「扫描刷新」：显示文件/笔记/跳过统计，.obsidian 不被读取；
  4. 扫描前后知识库目录树（内容 + 大小 + mtime）完全一致；
  5. 宿主集成测试的冷扫描指标（规模/耗时）从实测输出采集，不写目标值。

用法：python3 verify_knowledge_vault_p1.py [--display :123] [--root /tmp/odkb-p1-ui]
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

from window_harness import AppSession, XSession  # noqa: E402
import accessibility as ax  # noqa: E402

EVID = HERE / "evidence" / "od-kb-001-p1"
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
    """相对路径 → 大小 / mtime / sha256；符号链接只记目标。"""
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


def build_obsidian_vault(root: Path) -> None:
    (root / ".obsidian").mkdir(parents=True)
    (root / ".obsidian" / "app.json").write_text(
        json.dumps({"legacyEditor": False, "newFileLocation": "folder"}), encoding="utf-8"
    )
    (root / ".obsidian" / "workspace.json").write_text(
        json.dumps({"main": {"id": "synthetic"}, "active": "synthetic"}), encoding="utf-8"
    )
    (root / ".trash").mkdir()
    (root / ".trash" / "deleted.md").write_text("# 已删\n不应被读取。\n", encoding="utf-8")
    (root / "index.md").write_text(
        "# 合成首页\n\n- [[项目甲]]\n- [[会议记录]]\n", encoding="utf-8"
    )
    (root / "note.txt").write_text("纯文本合成笔记。\n", encoding="utf-8")
    (root / "项目").mkdir()
    (root / "项目" / "项目甲.md").write_text("# 项目甲\n\n状态：合成。\n", encoding="utf-8")
    (root / "会议" / "2026-10").mkdir(parents=True)
    (root / "会议" / "2026-10" / "周会.md").write_text(
        "# 周会\n\n只有合成内容。\n", encoding="utf-8"
    )
    (root / "assets").mkdir()
    (root / "assets" / "diagram.png").write_bytes(b"\x89PNG\r\n\x1a\n synthetic")
    (root / "node_modules").mkdir()
    (root / "node_modules" / "ignored.md").write_text("# 应跳过\n", encoding="utf-8")


def run_host_metric() -> dict:
    """跑宿主集成测试，采集冷扫描规模/耗时实测；失败则记录原因，不伪造数据。"""
    env = dict(os.environ)
    setup = REPO / "scripts" / "setup-desktop-toolchain.sh"
    if setup.is_file():
        # 脚本输出里含 $PATH / $PKG_CONFIG_PATH 引用，必须交给 shell 求值；
        # 逐行字符串解析会把它们当成字面量，导致 pkg-config 找不到。
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
    proc = subprocess.run(
        ["cargo", "test", "--offline", "--test", "knowledge_vault_host", "--", "--nocapture"],
        cwd=str(REPO / "desktop" / "src-tauri"),
        capture_output=True,
        text=True,
        env=env,
    )
    (EVID / "host-test.log").write_text(proc.stdout + proc.stderr, encoding="utf-8")
    metric = None
    for line in proc.stdout.splitlines():
        if line.startswith("KB_P1_SCAN_METRIC "):
            metric = json.loads(line[len("KB_P1_SCAN_METRIC "):])
    passed = re.search(r"test result: ok\. (\d+) passed", proc.stdout)
    return {
        "exit_code": proc.returncode,
        "passed": int(passed.group(1)) if passed else 0,
        "metric": metric,
        "log": str((EVID / "host-test.log").relative_to(REPO)),
    }


def open_library_tab(pid: int) -> list[ax.Node]:
    nodes = app_nodes(pid)
    settings_btn = find(nodes, "系统偏好设置", "button")
    if settings_btn is None:
        raise RuntimeError("找不到设置入口")
    ax.invoke(settings_btn)
    time.sleep(2.5)
    nodes = app_nodes(pid)
    tab = find(nodes, "成果与文档库", "page tab", contains=False)
    if tab is None:
        raise RuntimeError("找不到「成果与文档库」分页")
    ax.invoke(tab)
    time.sleep(2.5)
    return app_nodes(pid)


def vault_panel_label(nodes: list[ax.Node]) -> str:
    """知识库状态组的 aria-label。

    WebKitGTK 不把状态行里的纯文本 span 暴露给无障碍树，但带 aria-label 的
    role=group 容器会暴露，所以窗口级断言读这个标签，截图作为人工可视证据。
    """
    for node in nodes:
        if node.role in ("panel", "group") and node.name.startswith("知识库状态："):
            return node.name
    return ""


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":123")
    ap.add_argument("--root", default="/tmp/odkb-p1-ui")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    vault = (root / "synth-vault").resolve()
    build_obsidian_vault(vault)
    before = tree_snapshot(vault)

    host = run_host_metric()
    facts: dict = {
        "plan": "OD-KB-001 P1",
        "verified_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ Rust 宿主集成测试实测",
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
            print(f"FAIL [KB-P1] {name}: {measured} vs {threshold} ({desc})", file=sys.stderr)
        else:
            print(f"PASS [KB-P1] {name}: {measured}")

    check(
        "host_integration_tests_pass",
        host["exit_code"] == 0 and host["passed"] >= 6,
        {"exit_code": host["exit_code"], "passed": host["passed"]},
        {"exit_code": 0, "passed": ">=6"},
        "宿主只读接口集成测试（含设置→扫描→读取与边界拒绝）",
    )
    check(
        "cold_scan_metric_collected",
        host["metric"] is not None,
        host["metric"],
        {"synthetic_files": 1201, "runs": 3},
        "冷扫描规模/耗时为实测值",
    )

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
        reuse_running_dev_server=False,
        env_extra={"DBUS_SESSION_BUS_ADDRESS": bus_addr, "OMNIDESK_DSH_PROFILE": ""},
    )
    try:
        session.prepare()
        settings_path = session.app_data / "config" / "settings.v1.json"
        settings_path.parent.mkdir(parents=True, exist_ok=True)
        # 第一次启动：明确不带知识库根，验证未设置状态。
        settings_path.write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "artifact_root": None,
                    "library_root": str(root / "unused-library"),
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

        open_library_tab(pid)
        summary = find(app_nodes(pid), "知识库（只读）")
        if summary is not None:
            ax.invoke(summary)
            time.sleep(1.2)
        xs.scroll(700, 740, clicks=20, settle=1.2)
        nodes = app_nodes(pid)
        xs.shot(EVID / "00-vault-unset.png")
        dump_tree(nodes, EVID / "tree-00-unset.json")
        text = all_text(nodes)
        label = vault_panel_label(nodes)
        check(
            "unset_state_visible",
            "知识库（只读）" in text and "未设置" in label and "尚未扫描" in label,
            {"has_card": "知识库（只读）" in text, "label": label},
            True,
            "未设置时状态清晰",
        )

        # 第二次启动：预置合成 Obsidian 根。
        session.stop_app()
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
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("重启后没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        ax.init()

        nodes = open_library_tab(pid)
        summary = find(nodes, "知识库（只读）")
        check("vault_card_visible", summary is not None, summary is not None, True)
        if summary is None:
            raise RuntimeError("找不到知识库卡片")
        ax.invoke(summary)
        time.sleep(1.5)
        xs.scroll(700, 740, clicks=20, settle=1.2)
        nodes = app_nodes(pid)
        xs.shot(EVID / "01-vault-configured.png")
        dump_tree(nodes, EVID / "tree-01-configured.json")
        text = all_text(nodes)
        label = vault_panel_label(nodes)
        check(
            "shows_vault_root",
            str(vault) in label,
            label,
            True,
            "显示已保存的知识库根",
        )
        check(
            "shows_directory_readable",
            "目录可用" in label,
            label,
            True,
            "目录状态为可读取",
        )

        buttons = [n for n in nodes if n.role == "button" and n.name == "扫描刷新"]
        check("scan_button_found", len(buttons) == 1, len(buttons), 1)
        if len(buttons) != 1:
            raise RuntimeError("找不到「扫描刷新」按钮")
        ext = extents(buttons[0])
        check(
            "scan_button_in_viewport",
            0 <= ext.y and ext.y + ext.height <= SCREEN_H,
            {"y": ext.y, "h": ext.height},
            f"y+h <= {SCREEN_H}",
        )
        xs.click(ext.x + ext.width // 2, ext.y + ext.height // 2, settle=1.0)
        time.sleep(3.0)
        nodes = app_nodes(pid)
        xs.shot(EVID / "02-vault-scanned.png")
        dump_tree(nodes, EVID / "tree-02-scanned.json")
        text = all_text(nodes)
        label = vault_panel_label(nodes)
        check(
            "scan_reports_counts",
            "扫描结果 5 个文件" in label
            and "笔记 4" in label
            and "附件 1" in label
            and "跳过 3 项" in label
            and "错误 0 项" in label,
            {
                "label": label,
                "has_message": "扫描完成" in text,
            },
            {"files": 5, "notes": 4, "attachments": 1, "skipped": 3, "errors": 0},
            "合成根共 5 个可展示文件（4 笔记 + 1 附件）",
        )
        check(
            "hides_obsidian_detail",
            ".obsidian/app.json" not in text and "deleted.md" not in text,
            {"app_json_listed": ".obsidian/app.json" in text, "trash_listed": "deleted.md" in text},
            {"app_json_listed": False, "trash_listed": False},
            ".obsidian 与 .trash 不进结果",
        )
    finally:
        session.stop_all()
        bus.terminate()
        xvfb.terminate()

    after = tree_snapshot(vault)
    changed = [k for k in sorted(set(before) | set(after)) if before.get(k) != after.get(k)]
    check(
        "vault_tree_unchanged",
        after == before,
        {"changed_entries": changed},
        [],
        "扫描/读取前后内容、大小与 mtime 完全一致",
    )
    check(
        "obsidian_config_intact",
        (vault / ".obsidian" / "app.json").is_file()
        and json.loads((vault / ".obsidian" / "app.json").read_text(encoding="utf-8")).get(
            "legacyEditor"
        )
        is False,
        (vault / ".obsidian" / "app.json").read_text(encoding="utf-8"),
        "原样保留",
        "不改 Obsidian 配置",
    )
    settings_after = json.loads(
        (root / "app-data" / "config" / "settings.v1.json").read_text(encoding="utf-8")
    )
    check(
        "library_root_not_rewritten",
        settings_after.get("library_root") == str(root / "unused-library"),
        settings_after.get("library_root"),
        str(root / "unused-library"),
        "知识库使用不改写库根",
    )
    check(
        "vault_setting_persisted",
        settings_after.get("knowledge_vault_root") == str(vault),
        settings_after.get("knowledge_vault_root"),
        str(vault),
    )

    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"checks={len(facts['checks'])} failures={len(facts['failures'])}")
    return 0 if not facts["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

