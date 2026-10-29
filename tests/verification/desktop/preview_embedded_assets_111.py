#!/usr/bin/env python3
"""OD-ARCH-111：模拟「分发到用户机器」——仓库根不存在时仍能装出完整工作台。

真实释放版二进制 + 一次性 Xvfb + /tmp 隔离数据根，把 `OMNIDESK_REPO_ROOT`
指向一个不存在的目录（模拟 deb/AppImage 装机后源码路径不存在），引擎目录与
CLI 显式指定（这两样在真实安装里由安装器提供）。

断言：8 个 Skill 的 SKILL.md 齐备、写作 references 齐备、装配日志无源缺失；
并对照 `repo_root` 存在时（开发机）装出的内容逐字节一致。

用法：python3 preview_embedded_assets_111.py [--display :134] [--root /tmp/od111-preview]
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from preview_weekly_100 import start_dbus, start_display  # noqa: E402
from window_harness import AppSession, REPO, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-111"
SKILLS = [
    "anysearch",
    "document-review",
    "knowledge-agent",
    "knowledge-search",
    "material-writing",
    "memory-agent",
    "memory-report",
    "project-agent",
]
REFERENCES = [
    ("material-writing", "references/templates/report-template.md"),
    ("material-writing", "references/writing-style/style-guide.md"),
    ("document-review", "references/excellent-cases/README.md"),
]


def wait_for(predicate, timeout: float = 90.0, interval: float = 1.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return True
        time.sleep(interval)
    return False


def run(display: str, root: Path, bus_addr: str) -> dict:
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    facts: dict = {"root": str(root), "checks": {}}
    ghost = root / "no-such-repo"
    binary = REPO / "desktop/src-tauri/target/release/omnidesk-desktop"
    facts["binary"] = str(binary)
    facts["binary_bytes"] = binary.stat().st_size if binary.is_file() else None
    session = AppSession(
        root=root,
        display=display,
        use_dev_server=False,
        binary=binary if binary.is_file() else None,
        env_extra={
            "DBUS_SESSION_BUS_ADDRESS": bus_addr,
            # 模拟装机环境：源码树路径不存在。
            "OMNIDESK_REPO_ROOT": str(ghost),
            # 这两样在真实安装里由安装器提供，不是仓库路径。
            "OMNIDESK_ENGINE_DIR": str(REPO / "engine"),
            "OMNIDESK_DSH_CLI": str(
                REPO / "runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js"
            ),
            "OMNIDESK_DSH_PROFILE": "discipline",
        },
    )
    try:
        session.prepare()
        session.start()
        log = root / "app-data" / "logs" / "profile-assets.log"
        facts["assembly_log_seen"] = wait_for(lambda: log.is_file(), timeout=90.0)
        facts["startup_log"] = log.read_text(encoding="utf-8") if log.is_file() else None
        # 装配是同步写盘的，日志出现即已完成；再给一点点余量。
        time.sleep(1.0)
        facts["ghost_exists"] = ghost.exists()
        skills_root = root / "dsh-home" / "skills"
        facts["skill_dirs"] = (
            sorted(p.name for p in skills_root.iterdir()) if skills_root.is_dir() else []
        )
        facts["missing_skill_md"] = [
            name
            for name in SKILLS
            if not (skills_root / name / "SKILL.md").is_file()
        ]
        facts["missing_references"] = [
            f"{skill}/{rel}" for skill, rel in REFERENCES if not (skills_root / skill / rel).is_file()
        ]
        facts["profile_dirs"] = (
            sorted(p.name for p in (root / "dsh-home/profiles").iterdir())
            if (root / "dsh-home/profiles").is_dir()
            else []
        )
        facts["app_profile_files"] = (
            sorted(p.name for p in (root / "app-data/profiles/discipline").iterdir())
            if (root / "app-data/profiles/discipline").is_dir()
            else []
        )
        facts["dsh_profile_files"] = (
            sorted(p.name for p in (root / "dsh-home/profiles/discipline").iterdir())
            if (root / "dsh-home/profiles/discipline").is_dir()
            else []
        )
        # 内容一致性：装出来的 Skill 与仓库权威源逐字节相同（内嵌没走样）。
        mismatched = []
        for name in SKILLS:
            src = REPO / "skills" / name / "SKILL.md"
            dst = skills_root / name / "SKILL.md"
            if src.is_file() and dst.is_file() and src.read_bytes() != dst.read_bytes():
                mismatched.append(name)
        facts["content_mismatch"] = mismatched

        facts["checks"]["repo_root_absent"] = not facts["ghost_exists"]
        facts["checks"]["assembly_ran"] = bool(facts["startup_log"])
        facts["checks"]["all_skills_present"] = facts["missing_skill_md"] == []
        facts["checks"]["references_present"] = facts["missing_references"] == []
        facts["checks"]["config_present"] = (
            "discipline" in facts["profile_dirs"]
            and "package.json" in facts["dsh_profile_files"]
            and "cordis.patch.yml" in facts["dsh_profile_files"]
            and "decl.yml" in facts["app_profile_files"]
            and "rules.json" in facts["app_profile_files"]
        )
        facts["checks"]["content_matches_repo"] = facts["content_mismatch"] == []
        facts["checks"]["no_missing_source"] = (
            facts["startup_log"] is not None and "missing_source" not in facts["startup_log"]
        )
        return facts
    finally:
        session.stop_all()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":134")
    ap.add_argument("--root", default="/tmp/od111-preview")
    args = ap.parse_args()
    EVID.mkdir(parents=True, exist_ok=True)
    xproc = start_display(args.display)
    bus_addr, bus_proc = start_dbus()
    display = args.display
    try:
        facts = run(display, Path(args.root), bus_addr)
    finally:
        for proc in (bus_proc, xproc):
            try:
                proc.terminate()
                proc.wait(timeout=10)
            except Exception:
                pass
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    for key, value in facts["checks"].items():
        print(f"{'PASS' if value else 'FAIL'} {key}")
    passed = sum(1 for v in facts["checks"].values() if v)
    total = len(facts["checks"])
    print(f"\n{passed}/{total} passed")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())

