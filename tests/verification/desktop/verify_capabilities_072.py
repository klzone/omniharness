#!/usr/bin/env python3
"""OD-ARCH-072 真实窗口验收：设置页「能力管理」分页。

在一次性隔离根里启动真实 OmniDesk 桌面窗口（一次性 Xvfb + 回环合成端点）：
  1. 进设置 → 能力管理，截图核对 Skill 列表、插件列表（含版本）与市场开关；
  2. 点停用 dsh-cost-meter → 截图 → 核对隔离 profile 的 package.json（bundles 少一项、dependencies 仍在、有 .bak）；
  3. 点启用 → 截图 → 核对已恢复；
  4. 核对源数据根（~/.dsh、~/.agents、用户 ~/OmniDesk/runtime）未被写入。

隔离根从用户 OmniDesk 隔离实例**只读复制**一份（含 node_modules），所有写操作只落在 /tmp 副本里。
不读凭证、不联网、不碰真实业务材料。

用法：python3 verify_capabilities_072.py [--display :96] [--root /tmp/od072-verify]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-072"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"

SETTINGS_GEAR = (1117, 44)
CAP_TAB = (556, 201)
# 插件复选框（1440x900 固定版式；由 02-capabilities.png 校准）
COST_METER_CHECK = (531, 727)
# 滚回页顶后，市场开关的复选框落在窗口底部一行（由 06b-back-to-top.png 校准）。
MARKET_CHECK = (531, 875)


def start_display(display: str) -> subprocess.Popen:
    """起一次性 Xvfb，并等到 socket 真的可以连（不等死时间）。"""
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            err = proc.stderr.read().decode("utf-8", "replace") if proc.stderr else ""
            raise RuntimeError(f"Xvfb {display} 启动失败：{err.strip()}")
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            try:
                import socket

                with socket.socket(socket.AF_UNIX) as s:
                    s.settimeout(0.5)
                    if s.connect_ex(f"/tmp/.X11-unix/X{dno}") == 0:
                        return proc
            except OSError:
                pass
        time.sleep(0.25)
    raise RuntimeError(f"Xvfb {display} 未在超时内就绪（可能有残留锁 /tmp/.X{dno}-lock）")


def tree_fingerprint(root: Path, limit: int = 4000) -> str:
    """原版数据根的轻量指纹（只读，不进入文件内容以外的地方）。"""
    h = hashlib.sha256()
    if not root.exists():
        return "missing"
    count = 0
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        count += 1
        st = p.stat()
        h.update(f"{p.relative_to(root)}|{st.st_size}|{int(st.st_mtime)}".encode())
        if count >= limit:
            break
    return h.hexdigest()[:16] + f":{count}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":96")
    ap.add_argument("--root", default="/tmp/od072-verify")
    ap.add_argument("--tab-x", type=int, default=CAP_TAB[0])
    ap.add_argument("--tab-y", type=int, default=CAP_TAB[1])
    args = ap.parse_args()
    root = Path(args.root)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    facts: dict = {"display": args.display, "root": str(root)}
    facts["source_fingerprint_before"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_before"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_before"] = tree_fingerprint(Path.home() / ".agents")

    # 只读复制隔离实例（含 node_modules 与 skills）。
    shutil.copytree(SOURCE_HOME, root / "dsh-home", symlinks=True)
    profile = root / "dsh-home" / "profiles" / "web"
    manifest = profile / "package.json"
    before = json.loads(manifest.read_text(encoding="utf-8"))
    facts["bundles_before"] = list(before["dsh"]["profile"]["bundles"])
    facts["dependencies_before"] = dict(before.get("dependencies") or {})
    facts["skills_on_disk"] = sorted(
        p.name for p in (root / "dsh-home" / "skills").iterdir() if (p / "SKILL.md").is_file()
    )

    xvfb = start_display(args.display)
    session = AppSession(root=root, display=args.display, artifact_override=True)
    try:
        session.prepare()
        x = XSession(args.display)
        session.start()
        win = x.find_window("OmniDesk", timeout=120)
        x.focus(win)
        x.place(win, 0, 0, 1440, 900, settle=3.0)
        time.sleep(7)
        x.shot(EVID / "00-home.png")

        x.click(*SETTINGS_GEAR, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "01-settings-tabs.png")
        print("CALIBRATE: 看 01-settings-tabs.png 定位「能力管理」标签")
        x.click(args.tab_x, args.tab_y, settle=2.5)
        time.sleep(2.0)
        x.shot(EVID / "02-capabilities.png")

        # 停用 dsh-cost-meter：只看隔离副本里的 package.json 变化
        x.click(*COST_METER_CHECK, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "04-cost-meter-disabled.png")
        after_disable = json.loads(manifest.read_text(encoding="utf-8"))
        facts["bundles_after_disable"] = list(after_disable["dsh"]["profile"]["bundles"])
        facts["dependencies_after_disable"] = dict(after_disable.get("dependencies") or {})
        facts["backups_after_disable"] = sorted(p.name for p in profile.glob("package.json.bak-*"))
        # 只比对摘要：profile 补丁里可能有端点/环境变量名，内容不落进仓库证据。
        def sha(path: Path) -> str:
            return hashlib.sha256(path.read_bytes()).hexdigest()

        facts["patch_sha"] = sha(profile / "cordis.patch.yml")[:16]
        facts["patch_sha_unchanged"] = facts["patch_sha"] == sha(
            SOURCE_HOME / "profiles" / "web" / "cordis.patch.yml"
        )[:16]

        # 重新启用
        x.click(*COST_METER_CHECK, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "05-cost-meter-enabled.png")
        final = json.loads(manifest.read_text(encoding="utf-8"))
        facts["bundles_after_enable"] = list(final["dsh"]["profile"]["bundles"])
        facts["backups_total"] = sorted(p.name for p in profile.glob("package.json.bak-*"))

        # 滚到 Skill 列表底部，确认 6 个 Skill 都在界面上
        x.scroll(700, 700, 12, settle=1.5)
        x.shot(EVID / "06-skills-scrolled.png")

        # 市场开关放最后：拒绝提示会把下面的控件整体推下去，先做完插件开关再点它。
        x.scroll(700, 400, 25, up=True, settle=2.0)
        time.sleep(1.0)
        x.shot(EVID / "06b-back-to-top.png")
        manifest_sha_before = hashlib.sha256(manifest.read_bytes()).hexdigest()
        backups_before = sorted(p.name for p in profile.glob("package.json.bak-*"))
        x.click(*MARKET_CHECK, settle=2.5)
        time.sleep(1.5)
        x.shot(EVID / "07-market-refused.png")
        facts["market_manifest_unchanged"] = (
            hashlib.sha256(manifest.read_bytes()).hexdigest() == manifest_sha_before
        )
        facts["market_no_new_backup"] = (
            sorted(p.name for p in profile.glob("package.json.bak-*")) == backups_before
        )
        facts["alive"] = session.alive()
        # 日志只留最后一行错误级别信息，避免把路径/配置明细带进证据。
        log_text = (session.logs / "app.log").read_text(encoding="utf-8", errors="replace")
        facts["app_log_bytes"] = len(log_text)
        facts["app_log_has_error"] = "ERROR" in log_text or "panicked" in log_text
        facts["engine_log_has_cost_meter"] = any(
            "dsh-cost-meter" in p.read_text(encoding="utf-8", errors="replace")
            for p in session.logs.glob("*.log")
        )
    finally:
        session.stop_all()
        xvfb.terminate()
        try:
            xvfb.wait(timeout=10)
        except Exception:
            xvfb.kill()

    facts["source_fingerprint_after"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_after"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_after"] = tree_fingerprint(Path.home() / ".agents")
    facts["source_unchanged"] = (
        facts["source_fingerprint_before"] == facts["source_fingerprint_after"]
    )
    facts["user_dsh_unchanged"] = facts["user_dsh_before"] == facts["user_dsh_after"]
    facts["user_agents_unchanged"] = facts["user_agents_before"] == facts["user_agents_after"]
    facts["passed"] = bool(
        facts["alive"]
        and facts["bundles_after_disable"]
        == [
            "@deepseek-ai/dsh-base",
            "@deepseek-ai/dsh-web-app",
            "dsh-free-search",
        ]
        and facts["dependencies_after_disable"] == facts["dependencies_before"]
        and facts["bundles_after_enable"] == facts["bundles_before"]
        and len(facts["backups_total"]) >= 2
        and facts["patch_sha_unchanged"]
        and facts["source_unchanged"]
        and facts["user_dsh_unchanged"]
        and facts["user_agents_unchanged"]
        and len(facts["skills_on_disk"]) == 6
        and facts["market_manifest_unchanged"]
        and facts["market_no_new_backup"]
    )
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({"passed": facts["passed"], **{k: v for k, v in facts.items() if k in (
        "bundles_after_disable", "bundles_after_enable", "backups_total",
        "patch_sha_unchanged", "source_unchanged", "user_dsh_unchanged",
        "user_agents_unchanged", "skills_on_disk", "alive")}},
        ensure_ascii=False, indent=2))
    return 0 if facts["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
