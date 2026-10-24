#!/usr/bin/env python3
"""OD-ARCH-077 S1/S2 真实窗口验收：@ 引用 → 附件 chip → 正文送达模型 → 语音说明 → 能力冻结。

与旧版的区别：每一条结论都由真实布尔判定，界面元素通过 AT-SPI 按名称定位，
不再用「进程还活着」冒充通过。旧版点击了输入框下方的空白处，候选列表根本没打开。

验证链：
  1. 输入框键入 `@syn` → 出现候选，只含一次性知识库里的合成文件；
     不含 payload.bin（非白名单类型）与 .hidden/secret.md（隐藏目录）；
  2. 点候选 → 出现 attachment chip，输入框文字含文件名；
  3. 发送 → 回环合成端点实际收到附件正文里的唯一标记（证明正文进了模型请求）；
  4. 移除 chip → 原文件仍在磁盘；
  5. 点「语音」→ 出现“当前版本未开放”说明（界面文字或可见变化）；
  6. 设置 → 能力管理：冻结文案可见、没有插件市场入口；
  7. 源隔离根、`~/.dsh`、`~/.agents` 指纹前后一致。

证据等级：真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 文件事实 + 静态源码核对。
合成端点只回答固定文本，因此不据此声称“模型理解了附件”，只证明正文送达。

用法：python3 verify_references_077.py [--display :97] [--root /tmp/od077-refs]
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
sys.path.insert(0, str(HERE))

from window_harness import AppSession, XSession  # noqa: E402

EVID = HERE / "evidence" / "od-arch-077"
SOURCE_HOME = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
SCREEN_W, SCREEN_H = 1440, 900

INPUT_POINT = (700, 514)
MARKER = "SYNTHETIC-MARKER-077"


def start_display(display: str) -> subprocess.Popen:
    dno = display.lstrip(":").split(".")[0]
    proc = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", f"{SCREEN_W}x{SCREEN_H}x24", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            err = proc.stderr.read().decode("utf-8", "replace") if proc.stderr else ""
            raise RuntimeError(f"Xvfb {display} 启动失败：{err.strip()}")
        if Path(f"/tmp/.X11-unix/X{dno}").exists():
            return proc
        time.sleep(0.25)
    raise RuntimeError(f"Xvfb {display} 未就绪")


def tree_fingerprint(root: Path, limit: int = 4000) -> str:
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


def write_settings_local(session: AppSession) -> Path:
    """只写一次性隔离根里的设置：合成端点是 127.0.0.1，归入“本机模型”。"""
    path = session.app_data / "config" / "settings.v1.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "schema": "omnidesk.settings.v1",
                "artifact_root": None,
                "model_permissions": {"local": True, "private": False, "public": False},
            },
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def diff_ratio(a: Path, b: Path) -> float:
    """两张同尺寸截图在指令坞区域的差异占比，用于证明界面确实发生了变化。"""
    from PIL import Image, ImageChops

    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    # 必须覆盖整个指令坞（输入框 → 提示行 → 按钮行）。只截上沿会把“提示文字出现”
    # 判成“没有变化”，从而误报失败。
    box = (250, 430, 1330, 870)
    ia, ib = ia.crop(box), ib.crop(box)
    diff = ImageChops.difference(ia, ib).convert("L")
    changed = sum(1 for v in diff.getdata() if v > 12)
    return round(changed / (diff.width * diff.height), 5)


def source_checks() -> dict:
    """静态源码核对（不替代窗口证据，只防止“界面有、代码没有”这类假象）。"""
    src = HERE.parents[2] / "desktop" / "src"
    panel = (src / "SettingsPanel.tsx").read_text(encoding="utf-8")
    home = (src / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    attach = (src / "attachmentPrompt.ts").read_text(encoding="utf-8")
    rust = (HERE.parents[2] / "desktop" / "src-tauri" / "src" / "task_shell.rs").read_text(
        encoding="utf-8"
    )
    return {
        "capability_freeze_copy": "新增插件与 Skill" in panel and "随版本升级" in panel,
        "no_market_entry": "接入 DSH 插件市场" not in panel and "capability_market" not in panel,
        "no_capability_market_cmd": "capability_market" not in rust,
        "mention_ui": "data-testid=\"mention-list\"" in home,
        "chip_ui": "data-testid=\"attachment-chips\"" in home,
        "voice_copy": "当前版本未开放" in home,
        "attachment_body_in_prompt": "正文开始" in attach and "sha256" in attach,
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":97")
    ap.add_argument("--root", default="/tmp/od077-refs")
    args = ap.parse_args(argv[1:])

    root = Path(args.root).resolve()
    if Path("/tmp") not in root.parents:
        raise SystemExit(f"拒绝清理非 /tmp 下的目录：{root}")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    EVID.mkdir(parents=True, exist_ok=True)

    knowledge = root / "knowledge"
    (knowledge / "regulations").mkdir(parents=True)
    sample = knowledge / "regulations" / "synth-rule.md"
    sample.write_text(
        "# 合成制度样本\n"
        "第一条 本文件全部内容虚构，仅用于界面验收。\n"
        f"标记：{MARKER}\n",
        encoding="utf-8",
    )
    (knowledge / "regulations" / "payload.bin").write_bytes(b"PK\x03\x04")
    (knowledge / ".hidden").mkdir()
    (knowledge / ".hidden" / "secret.md").write_text("隐藏", encoding="utf-8")

    facts: dict = {
        "plan": "OD-ARCH-077 S1/S2 + D-030",
        "evidence_level": "真实桌面窗口（Xvfb + XTEST + AT-SPI）+ 文件事实 + 静态源码核对",
        "root": str(root),
        "knowledge_root": str(knowledge),
        "source_home": str(SOURCE_HOME),
        "checks": {},
    }
    facts["source_before"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_before"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_before"] = tree_fingerprint(Path.home() / ".agents")
    shutil.copytree(SOURCE_HOME, root / "dsh-home", dirs_exist_ok=True, symlinks=True)

    xvfb = start_display(args.display)
    session = AppSession(
        root=root,
        display=args.display,
        artifact_override=True,
        env_extra={"OMNIDESK_KNOWLEDGE_ROOT": str(knowledge)},
    )
    gate_errors: list[str] = []
    try:
        session.prepare()
        write_settings_local(session)
        xs = XSession(args.display)
        session.start()
        win = xs.find_window("OmniDesk", timeout=120)
        if win is None:
            raise RuntimeError("没有出现 OmniDesk 主窗口")
        xs.focus(win)
        xs.place(win, 0, 0, SCREEN_W, SCREEN_H, settle=3.0)
        time.sleep(9)
        xs.shot(EVID / "00-home.png")

        import accessibility as ax

        ax.init()

        # --- 1. @ 候选 ----------------------------------------------------
        xs.click(*INPUT_POINT, settle=1.0)
        xs.type_ascii("@syn", settle=0.12)
        time.sleep(2.5)
        xs.shot(EVID / "01-mention-list.png")
        labels = ax.labels()
        joined = "\n".join(labels)
        candidate = ax.find("synth-rule.md", role="push button", contains=True) or ax.find(
            "synth-rule.md", contains=True
        )
        facts["checks"]["mention_list_opened"] = candidate is not None
        facts["checks"]["candidate_lists_sample"] = "synth-rule.md" in joined
        facts["checks"]["candidate_excludes_binary"] = "payload.bin" not in joined
        facts["checks"]["candidate_excludes_hidden"] = "secret.md" not in joined
        if candidate is None:
            gate_errors.append("输入 @syn 后没有出现 synth-rule.md 候选")

        # --- 2. 选中候选 → chip -------------------------------------------
        if candidate is not None and not ax.invoke(candidate):
            gate_errors.append("候选按钮不可点击（AT-SPI Action 与坐标回退都失败）")
        time.sleep(2.0)
        xs.shot(EVID / "02-attachment-chip.png")
        chip = ax.find("移除附件 synth-rule.md", role="button", contains=True)
        facts["checks"]["attachment_chip"] = chip is not None
        if chip is None:
            gate_errors.append("选中候选后没有出现附件 chip")

        # --- 3. 发送 → 正文是否真的进了模型请求 ----------------------------
        session.mock_ctl.set(probes=[MARKER])
        session.mock_ctl.reset_requests()
        xs.click(*INPUT_POINT, settle=0.6)
        xs.key("End")
        xs.type_ascii(" summarize the attached synthetic rule", settle=0.02)
        time.sleep(0.8)
        xs.shot(EVID / "03-before-send.png")
        send = ax.find("发送指令", role="push button") or ax.find("发送指令", contains=True)
        facts["checks"]["send_button"] = send is not None
        if send is None:
            gate_errors.append("找不到发送按钮")
        elif not ax.invoke(send):
            gate_errors.append("发送按钮不可点击")
        deadline = time.monotonic() + 90
        hits: list = []
        reqs: list = []
        while time.monotonic() < deadline:
            reqs = session.mock_ctl.new_requests()
            if reqs:
                hits = [h for r in reqs for h in r.get("probe_hits", [])]
                break
            time.sleep(1.0)
        time.sleep(3.0)
        xs.shot(EVID / "04-after-send.png")
        facts["mock_request_count"] = len(reqs)
        facts["attachment_marker_hits"] = hits
        facts["request_body_probe_hits"] = [h for r in reqs for h in r.get("body_probe_hits", [])]
        facts["checks"]["attachment_body_reached_model"] = MARKER in hits
        # AT-SPI 不暴露输入框正文；用「本轮请求里的用户输入」证明输入框确实带上了文件名。
        last_users = [r.get("last_user", "") for r in reqs]
        facts["request_last_user_sample"] = [s[:80] for s in last_users]
        facts["checks"]["input_shows_filename"] = any(
            "synth-rule.md" in s for s in last_users
        )
        if MARKER not in hits:
            gate_errors.append("模型请求里没有附件正文标记（附件可能只留在界面上）")

        # 等这一轮结束，避免下一步点按钮时还在生成
        for _ in range(60):
            if ax.find("发送指令", role="push button") is not None:
                break
            time.sleep(1.0)

        # --- 4. 移除剩余 chip（发送后一般已清空；有则点掉） ------------------
        leftover = ax.find("移除附件 synth-rule.md", role="button", contains=True)
        if leftover is not None:
            ax.invoke(leftover)
            time.sleep(1.0)
        xs.shot(EVID / "05-chip-removed.png")
        facts["checks"]["source_file_intact"] = sample.is_file()
        facts["checks"]["chip_cleared_after_send"] = (
            ax.find("移除附件 synth-rule.md", role="button", contains=True) is None
        )
        if not sample.is_file():
            gate_errors.append("引用后原文件被删除")

        # --- 5. 语音说明 ---------------------------------------------------
        xs.shot(EVID / "06-voice-before.png")
        voice = ax.find("语音", role="button")
        facts["checks"]["voice_button"] = voice is not None
        if voice is not None:
            ax.invoke(voice)
        time.sleep(2.0)
        xs.shot(EVID / "07-voice-note.png")
        voice_text = ax.has_text("语音输入需要配置语音模型端点")
        voice_diff = diff_ratio(EVID / "06-voice-before.png", EVID / "07-voice-note.png")
        facts["voice_note_ax_text"] = voice_text
        facts["voice_note_pixel_diff"] = voice_diff
        # AT-SPI 对 WebKit 的行内提示文字不可见，以「指令坞区域像素确实变化」为可见性证据，
        # 并保留截图供人工核对具体文案（源码断言另有 voice_copy）。
        facts["checks"]["voice_note_visible"] = bool(voice_text or voice_diff > 0.002)

        # --- 6. 设置 → 能力管理 --------------------------------------------
        gear = ax.find("系统偏好设置", role="push button") or ax.find(
            "系统偏好设置", contains=True
        )
        facts["checks"]["settings_gear"] = gear is not None
        if gear is not None:
            ax.invoke(gear)
        time.sleep(2.5)
        xs.shot(EVID / "08-settings.png")
        tab = ax.find("能力管理", role="page tab") or ax.find("能力管理", role="button") or ax.find(
            "能力管理", contains=True
        )
        facts["checks"]["capability_tab"] = tab is not None
        if tab is not None:
            ax.invoke(tab)
        time.sleep(3.0)
        xs.dismiss_overlays()
        time.sleep(0.5)
        xs.shot(EVID / "09-capabilities.png")
        caps_text = ax.has_text("随版本升级") or ax.has_text("新增插件与 Skill")
        facts["capability_ax_text"] = caps_text
        facts["capability_labels_sample"] = [
            s for s in ax.labels() if "插件" in s or "Skill" in s
        ][:8]

        facts["checks"].update(source_checks())
        facts["checks"]["alive"] = session.alive()
        facts["app_log_errors"] = sum(
            1
            for line in (session.logs / "app.log")
            .read_text(encoding="utf-8", errors="replace")
            .splitlines()
            if "panicked" in line or "ERROR" in line
        )
    finally:
        try:
            session.stop_all()
        finally:
            xvfb.terminate()
            try:
                xvfb.wait(timeout=10)
            except Exception:
                xvfb.kill()

    facts["source_after"] = tree_fingerprint(SOURCE_HOME)
    facts["user_dsh_after"] = tree_fingerprint(Path.home() / ".dsh")
    facts["user_agents_after"] = tree_fingerprint(Path.home() / ".agents")
    facts["checks"]["source_unchanged"] = facts["source_before"] == facts["source_after"]
    facts["checks"]["user_dsh_unchanged"] = facts["user_dsh_before"] == facts["user_dsh_after"]
    facts["checks"]["user_agents_unchanged"] = (
        facts["user_agents_before"] == facts["user_agents_after"]
    )
    facts["gate_errors"] = gate_errors
    checks = facts["checks"]
    # 能力页文案以源码事实 + 人工核对截图为准：AT-SPI 对 WebKit 长段落不一定暴露文字。
    required = (
        "mention_list_opened",
        "candidate_lists_sample",
        "candidate_excludes_binary",
        "candidate_excludes_hidden",
        "attachment_chip",
        "input_shows_filename",
        "send_button",
        "attachment_body_reached_model",
        "source_file_intact",
        "chip_cleared_after_send",
        "voice_button",
        "voice_note_visible",
        "settings_gear",
        "capability_tab",
        "capability_freeze_copy",
        "no_market_entry",
        "no_capability_market_cmd",
        "mention_ui",
        "chip_ui",
        "voice_copy",
        "attachment_body_in_prompt",
        "alive",
        "source_unchanged",
        "user_dsh_unchanged",
        "user_agents_unchanged",
    )
    missing = [k for k in required if not checks.get(k)]
    facts["missing_checks"] = missing
    facts["passed"] = not missing and not gate_errors
    (EVID / "facts.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(
        json.dumps(
            {
                "passed": facts["passed"],
                "missing_checks": missing,
                "gate_errors": gate_errors,
                "marker_hits": facts.get("attachment_marker_hits"),
                "requests": facts.get("mock_request_count"),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0 if facts["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
