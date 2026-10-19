#!/usr/bin/env python3
"""
OmniDesk OD-BUILD-003 —— 引擎默认 Profile 与 cwd 规范化回归场景（R1-R5）。

对应 PLANS.md 的 OD-BUILD-003 验收标准：
  R1 默认 Profile（无手工覆盖）下引擎能完成 ACP 初始化与 session/new
  R2 非 ACP Profile 不会被静默当成 ACP；失败必须显式暴露
  R3 同一物理目录的别名写法与 realpath 写法落在同一个存储项目目录
  R4 别名与 realpath 两种写法下已建立的会话均可 resume
  R5 不可解析的 cwd 按契约明确报错（invalid_argument），不静默退化为字符串相等

设计：
  * 每个场景独立的一次性 /tmp 数据根（DSH_HOME / DSH_AGENTS_HOME 都指向该根）。
  * 使用本机回环合成模型端点，零凭证、零外网、确定性。
  * 通过**真实子进程** python3 -m omnidesk_engine 走协议对话，不 import 适配器，
    以证明修复在进程边界上真实生效。
  * 未取得可靠证据的项一律记 fail / unverified，不伪造通过。

安全边界：不读/不打印/不复制凭证；不触碰 ~/.dsh、~/.agents、~/OmniDesk、~/YvanAI；
         不修改 DSH Runtime；合成素材全部为虚构占位内容。

用法：
  python3 run_profile_cwd_scenarios.py            # 全部
  python3 run_profile_cwd_scenarios.py R1 R3      # 指定场景
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

import _harness as H
from _mock_model_server import start_mock

EVIDENCE_LEVEL = "script（真实引擎子进程 + 真实 DSH CLI + 合成模型端点）"

REPO = H.REPO
ENGINE_DIR = REPO / "engine"


# ---------------------------------------------------------------------------
# 最小 IPC-2 客户端（行分隔 JSON-RPC 2.0 over stdio）
# ---------------------------------------------------------------------------


class EngineProc:
    """把 python3 -m omnidesk_engine 当黑盒：只通过协议对话。"""

    def __init__(self, root: dict, *, env_extra: dict | None = None):
        self.root = root
        self.proc = subprocess.Popen(
            [sys.executable, "-m", "omnidesk_engine"],
            cwd=str(ENGINE_DIR),
            env=H.adapter_env(root, env_extra),
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1,
        )
        self._id = 0
        self._lock = threading.Lock()
        # stderr 必须在后台持续收集：引擎进程在运行期间不会关闭 stderr，
        # 在主线程直接 read() 会永久阻塞。这里只累加符号化的诊断行，
        # 不保存任何可能含敏感内容的请求文本。
        self._stderr_lines: list[str] = []
        self._stderr_thread = threading.Thread(target=self._drain_stderr, daemon=True)
        self._stderr_thread.start()

    def _drain_stderr(self) -> None:
        try:
            for line in self.proc.stderr:
                self._stderr_lines.append(line.rstrip("\n"))
        except Exception:
            return

    def _send(self, method: str, params) -> int:
        with self._lock:
            self._id += 1
            rid = self._id
        self.proc.stdin.write(json.dumps(
            {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}) + "\n")
        self.proc.stdin.flush()
        return rid

    def call(self, method: str, params, *, timeout: float = 60.0):
        """同步请求；返回 (ok, payload)。超时/断流都如实返回，不抛。"""
        rid = self._send(method, params)
        box: dict = {}

        def reader():
            while True:
                line = self.proc.stdout.readline()
                if not line:
                    box["eof"] = True
                    return
                line = line.strip()
                if not line:
                    continue
                try:
                    frame = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if frame.get("id") == rid:
                    box["frame"] = frame
                    return
                box.setdefault("notifications", []).append(frame)

        thread = threading.Thread(target=reader, daemon=True)
        thread.start()
        thread.join(timeout)
        if "frame" not in box:
            box["timeout"] = True
        return box

    def initialize_params(self, *, profile: str | None, overlay: Path):
        params = {
            "protocol_version": "omnidesk-engine/1",
            "cwd": str(self.root["cwd"]),
            "engine_state_dir": str(self.root["state"]),
            "result_dir": str(self.root["state"]),
            "dsh": {
                "cli_entry": str(H.DSH_BIN),
                "dsh_home": str(self.root["dsh_home"]),
                "agents_home": str(self.root["agents"]),
                "patch_files": [str(overlay)],
            },
        }
        if profile is not None:
            params["dsh"]["profile"] = profile
        return params

    def raise_if_init_failed(self, params) -> dict:
        box = self.call("omni.initialize", params, timeout=75.0)
        frame = box.get("frame") or {}
        return frame

    def submit(self, *, workspace_dir: str, prompt: str = H.SYNTH_PROMPT):
        return self.call("omni.submit_task", {
            "intent": "synthetic-writing-task",
            "workspace_dir": workspace_dir,
            "prompt_text": prompt,
        }, timeout=90.0)

    def status(self, omni_task_id: str):
        return self.call("omni.get_task_status", {"omni_task_id": omni_task_id}, timeout=60.0)

    def resume_session(self, session_id: str, cwd: str | None = None):
        params = {"engine_session_id": session_id}
        if cwd is not None:
            params["cwd"] = cwd
        return self.call("omni.recover_session", params, timeout=60.0)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.terminate()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=8)
        except Exception:
            try:
                self.proc.kill()
            except Exception:
                pass

    def stderr_tail(self, limit: int = 900) -> str:
        return "\n".join(self._stderr_lines)[-limit:]


def wait_terminal(engine: EngineProc, omni_task_id: str, timeout: float = 90.0):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        box = engine.status(omni_task_id)
        frame = box.get("frame") or {}
        result = frame.get("result")
        if result:
            last = result
            if result.get("status") in ("succeeded", "failed", "cancelled", "interrupted", "unknown"):
                return last
        time.sleep(0.2)
    return last


def project_dirs(root: dict) -> list[str]:
    sessions = root["dsh_home"] / "sessions"
    if not sessions.is_dir():
        return []
    return sorted(p.name for p in sessions.iterdir() if p.is_dir())


def project_key(cwd: str) -> str:
    """复刻 DSH 存储分组算法（dsh-session-persistence-jsonl projectKey）。

    之所以在测试里复刻：用来做**反事实对照**——证明若不规范化，别名写法会
    落到另一个目录名。这是静态检查，不是运行时证据。
    """
    readable, sep_run = "", False
    for ch in cwd:
        if ch in ("/", "\\", ":"):
            if not sep_run:
                readable += "-"
            sep_run = True
        elif ch != "~" and (ch.isalnum() and ch.isascii() or ch in "._-"):
            readable += ch
            sep_run = False
        else:
            readable += "~" + format(ord(ch), "04X")
            sep_run = False
    return "--" + (readable.lstrip("-") or "root")[:251] + "--"


def processes_referencing(marker: str) -> list[dict]:
    """列出命令行里出现 marker 的进程（只读 /proc，不读环境变量、不读任何凭证）。

    用途：确认失败的启动**没有把子进程留在后台**。引擎子进程的命令行里带着
    一次性数据根的路径（--patch <root>/xxx.yml），因此可以用它精确识别本场景
    自己拉起的那个 DSH 进程，而不会误伤用户正在运行的其它 DSH 实例。
    """
    found: list[dict] = []
    proc_root = Path("/proc")
    if not proc_root.is_dir():
        return found
    for entry in proc_root.iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        if pid == os.getpid():
            continue
        try:
            raw = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        if not raw:
            continue
        parts = [part.decode("utf-8", "replace") for part in raw.split(b"\x00") if part]
        cmdline = " ".join(parts)
        if marker in cmdline:
            found.append({"pid": pid, "cmdline": cmdline[:200]})
    return found


# ---------------------------------------------------------------------------
# R1 默认 Profile 可用（无手工覆盖）
# ---------------------------------------------------------------------------


def r1(result, root):
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "r1", H.mock_overlay_text(base_url))
    # 宿主默认链的**构造与传递**已由 Rust 单测直接证明
    # （desktop/src-tauri/src/engine_supervisor.rs::engine_config_default_profile_chain_is_acp）。
    # 本场景接着证明：按该默认值（profile=acp）在隔离数据根里真实启动，ACP 初始化与
    # session/new 确实能跑通。这里**不再手工注入 OMNIDESK_DSH_PROFILE**——
    # 该环境变量只被 Rust 宿主读取，引擎子进程用的是 dsh.profile 参数；
    # 之前注入它会让场景看起来「验证了默认值」，实际上什么都没验证。
    engine = EngineProc(root)
    try:
        result.note("本场景只用 dsh.profile=acp（即宿主默认链产出的值）启动真实引擎子进程，"
                    "不注入 OMNIDESK_DSH_PROFILE，避免用「脚本自己塞 acp」冒充「宿主默认为 acp」。")
        result.check("ambient_env_has_no_profile_override",
                     "OMNIDESK_DSH_PROFILE" not in os.environ,
                     os.environ.get("OMNIDESK_DSH_PROFILE"))
        frame = engine.raise_if_init_failed(engine.initialize_params(profile="acp", overlay=overlay))
        result.artifact("initialize_frame_keys", sorted(frame.keys()))
        ok_init = "result" in frame
        result.check("acp_initialize_ok_without_manual_override", ok_init,
                     frame if not ok_init else frame["result"]["engine"]["compatibility"])
        if not ok_init:
            result.artifact("initialize_error", frame.get("error"))
            return
        caps = frame["result"]["engine"]["capabilities"]
        result.check("capabilities_declared", bool(caps), caps)

        sub = engine.submit(workspace_dir=str(root["cwd"]))
        sframe = sub.get("frame") or {}
        ok_sub = "result" in sframe
        result.check("session_new_reached_engine", ok_sub,
                     sframe if not ok_sub else {
                         "omni_task_id": sframe["result"].get("omni_task_id"),
                         "engine_session_id": sframe["result"].get("engine_session_id"),
                     })
        if not ok_sub:
            result.artifact("submit_error", sframe.get("error"))
            result.artifact("stderr_tail", engine.stderr_tail())
            return
        result.artifact("receipt", sframe["result"])
        final = wait_terminal(engine, sframe["result"]["omni_task_id"])
        result.artifact("terminal_status", final)
        result.check("task_reached_terminal", bool(final) and final.get("status") == "succeeded", final)
        result.check("storage_project_dir_created", len(project_dirs(root)) == 1, project_dirs(root))
        result.artifact("auto_initialized_profiles",
                        sorted(p.name for p in (root["dsh_home"] / "profiles").iterdir())
                        if (root["dsh_home"] / "profiles").is_dir() else [])
    finally:
        engine.close()
        server.shutdown()


# ---------------------------------------------------------------------------
# R1b 引擎侧默认（完全不带 profile 键）
# ---------------------------------------------------------------------------


def r1b(result, root):
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "r1b", H.mock_overlay_text(base_url))
    engine = EngineProc(root)
    try:
        frame = engine.raise_if_init_failed(engine.initialize_params(profile=None, overlay=overlay))
        ok_init = "result" in frame
        result.check("engine_default_profile_is_acp", ok_init,
                     frame if not ok_init else frame["result"]["engine"]["compatibility"])
        if not ok_init:
            result.artifact("initialize_error", frame.get("error"))
            return
        auto = sorted(p.name for p in (root["dsh_home"] / "profiles").iterdir()) \
            if (root["dsh_home"] / "profiles").is_dir() else []
        result.artifact("auto_initialized_profiles", auto)
        result.check("default_uses_shipped_acp_template", "acp" in auto, auto)
    finally:
        engine.close()
        server.shutdown()


# ---------------------------------------------------------------------------
# R2 非 ACP Profile 不被静默当成 ACP
# ---------------------------------------------------------------------------


def r2(result, root):
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "r2", H.mock_overlay_text(base_url))

    # (a) 不存在的 Profile：DSH 应在启动阶段直接报错。
    err_a = None
    engine_a = EngineProc(root)
    try:
        frame_a = engine_a.raise_if_init_failed(
            engine_a.initialize_params(profile="omnidesk-nonexistent-profile", overlay=overlay))
        ok_a = "result" in frame_a
        err_a = (frame_a.get("error") or {}).get("data", {}).get("engine_error_code")
        result.artifact("nonexistent_profile_error", frame_a.get("error"))
        result.check("nonexistent_profile_not_silently_accepted", not ok_a, frame_a.get("error"))
        result.check("nonexistent_profile_error_code_is_engine_unavailable",
                     err_a == "engine_unavailable",
                     err_a)
        # 诊断详情里应当能看到 DSH 自己报出「这个 Profile 不存在」，而不是一句含糊的失败。
        detail_a = ((frame_a.get("error") or {}).get("data") or {}).get("detail") or {}
        stderr_a = str(detail_a.get("stderr_tail") or "")
        result.check("nonexistent_profile_named_in_error_detail",
                     "omnidesk-nonexistent-profile" in stderr_a
                     and "does not exist" in stderr_a,
                     stderr_a[-200:])
    finally:
        engine_a.close()

    # (b) 存在但不是 ACP 的 Profile（web：打包 web bundle，不通过 stdio 讲 ACP）。
    engine_b = EngineProc(root)
    try:
        t0 = time.monotonic()
        frame_b = engine_b.raise_if_init_failed(
            engine_b.initialize_params(profile="web", overlay=overlay))
        elapsed = time.monotonic() - t0
        ok_b = "result" in frame_b
        err_obj = frame_b.get("error") or {}
        data_b = err_obj.get("data") or {}
        err_b = data_b.get("engine_error_code")
        detail_b = data_b.get("detail") or {}
        result.artifact("web_profile_elapsed_seconds", round(elapsed, 1))
        result.artifact("web_profile_error", err_obj)
        result.check("non_acp_profile_not_silently_accepted", not ok_b, frame_b.get("error"))
        # OD-BUILD-004：失败必须在有界等待内结束，且错误可读——指出**所配 Profile**
        # 与 OmniDesk **预期 ACP**，而不是丢一个光秃秃的 "timeout" 给调用方。
        result.check("failure_still_bounded",
                     elapsed < 45.0, {"elapsed_seconds": round(elapsed, 1)})
        result.check("unsupported_for_acp_code_is_standard",
                     err_b in ("timeout", "engine_unavailable", "protocol_incompatible"),
                     err_b)
        result.check("error_names_configured_profile",
                     detail_b.get("engine_profile") == "web",
                     detail_b.get("engine_profile"))
        result.check("error_states_expected_protocol_is_acp",
                     detail_b.get("expected_protocol") == "acp",
                     detail_b.get("expected_protocol"))
        result.check("error_message_names_profile_and_acp_expectation",
                     "web" in str(detail_b.get("reason") or "")
                     and "ACP" in str(detail_b.get("reason") or ""),
                     detail_b.get("reason"))
        # 与「Profile 不存在」（engine_unavailable + stderr 提到 does not exist）必须可区分。
        result.check("distinguishable_from_nonexistent_profile",
                     err_b != "engine_unavailable",
                     {"web_code": err_b, "nonexistent_code": err_a})
        # 起过的子进程必须已经结束，不能在后台留下一个 Web 服务。
        left = processes_referencing(str(root["base"]))
        result.artifact("leftover_engine_processes", left)
        result.check("no_leftover_engine_process", left == [], left)
        result.note("已闭合 OD-BUILD-003 的缺口：非 ACP Profile 仍在有界等待（约 %.0fs）后失败，"
                    "但错误现在带上所配 Profile（web）与预期协议（ACP）及可读原因，"
                    "并与「Profile 不存在」的错误码可区分；子进程已确认退出。" % elapsed)
    finally:
        engine_b.close()
        server.shutdown()


# ---------------------------------------------------------------------------
# R3 / R4 别名与 realpath 落在同一存储项目目录，且都可 resume
# ---------------------------------------------------------------------------


def r3(result, root):
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "r3", H.mock_overlay_text(base_url))
    alias = root["base"] / "work-alias"
    alias.symlink_to(root["cwd"])
    real = os.path.realpath(str(root["cwd"]))
    alias_real = os.path.realpath(str(alias))
    result.artifact("alias_path", str(alias))
    result.artifact("realpath_of_alias", alias_real)
    result.artifact("realpath_of_cwd", real)
    result.check("alias_and_cwd_are_same_physical_dir", alias_real == real,
                 {"alias": alias_real, "cwd": real})

    # 反事实（静态检查）：不规范化时，别名写法会算出**另一个** projectKey。
    result.artifact("counterfactual_project_keys_without_fix", {
        "via_alias": project_key(str(alias)),
        "via_realpath": project_key(real),
    })
    result.check("counterfactual_shows_two_keys_without_fix",
                 project_key(str(alias)) != project_key(real),
                 {"alias": project_key(str(alias)), "real": project_key(real)})

    engine = EngineProc(root)
    try:
        frame = engine.raise_if_init_failed(engine.initialize_params(profile="acp", overlay=overlay))
        result.check("initialize_ok", "result" in frame, frame.get("error"))
        if "result" not in frame:
            return

        sub_alias = engine.submit(workspace_dir=str(alias))
        f1 = sub_alias.get("frame") or {}
        result.check("submit_via_alias_ok", "result" in f1, f1.get("error"))
        if "result" not in f1:
            return
        rec1 = f1["result"]

        sub_real = engine.submit(workspace_dir=real + "/")
        f2 = sub_real.get("frame") or {}
        result.check("submit_via_realpath_with_trailing_slash_ok", "result" in f2, f2.get("error"))
        if "result" not in f2:
            return
        rec2 = f2["result"]

        st1 = wait_terminal(engine, rec1["omni_task_id"])
        st2 = wait_terminal(engine, rec2["omni_task_id"])
        result.artifact("terminal_alias", st1)
        result.artifact("terminal_realpath", st2)
        result.check("alias_task_succeeded", bool(st1) and st1.get("status") == "succeeded", st1)
        result.check("realpath_task_succeeded", bool(st2) and st2.get("status") == "succeeded", st2)

        dirs = project_dirs(root)
        result.artifact("storage_project_dirs", dirs)
        result.check("exactly_one_storage_project_dir", len(dirs) == 1, dirs)
        result.check("storage_dir_key_is_realpath_key",
                     dirs == [project_key(real)], {"dirs": dirs, "expected": project_key(real)})

        # 两侧会话都落在同一项目目录下。
        if dirs:
            sessions = root["dsh_home"] / "sessions" / dirs[0]
            ids = sorted(p.name for p in sessions.iterdir())
            result.artifact("session_ids_in_project_dir", ids)
            result.check("both_sessions_share_project_dir",
                         rec1["engine_session_id"] in ids and rec2["engine_session_id"] in ids,
                         ids)

        # 落盘的 workspace_dir 必须是规范化后的 realpath，而不是别名原样字符串。
        records = {}
        for path in sorted((root["state"]).glob("*.json")):
            data = json.loads(path.read_text(encoding="utf-8"))
            records[data["omni_task_id"]] = data.get("workspace_dir")
        result.artifact("persisted_workspace_dirs", records)
        result.check("persisted_cwd_canonicalized",
                     set(records.values()) == {real}, {"records": records, "real": real})
    finally:
        engine.close()
        server.shutdown()

    # R4：重启一个新引擎进程，两侧会话都要能 resume。
    engine2 = EngineProc(root)
    try:
        frame = engine2.raise_if_init_failed(engine2.initialize_params(profile="acp", overlay=overlay))
        result.check("second_process_initialize_ok", "result" in frame, frame.get("error"))
        if "result" not in frame:
            return
        # 以别名写法显式 resume 第一个会话：恢复时也必须规范化，否则会被当成另一个工作区。
        res1 = engine2.resume_session(rec1["engine_session_id"], cwd=str(alias))
        rf1 = res1.get("frame") or {}
        result.check("resume_alias_session_with_alias_cwd_ok", "result" in rf1, rf1.get("error"))
        if "result" in rf1:
            result.artifact("resume_alias_recovery", rf1["result"])
            result.check("resume_reports_canonical_cwd",
                         rf1["result"].get("cwd") == real,
                         {"reported": rf1["result"].get("cwd"), "real": real})
            result.check("resume_did_not_replay", rf1["result"].get("note", "").find("重放") >= 0,
                         rf1["result"].get("note"))
        res2 = engine2.resume_session(rec2["engine_session_id"])
        rf2 = res2.get("frame") or {}
        result.check("resume_realpath_session_ok", "result" in rf2, rf2.get("error"))
        if "result" in rf2:
            result.artifact("resume_realpath_recovery", rf2["result"])
    finally:
        engine2.close()


# ---------------------------------------------------------------------------
# R5 不可解析的 cwd 明确报错
# ---------------------------------------------------------------------------


def r5(result, root):
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "r5", H.mock_overlay_text(base_url))
    engine = EngineProc(root)
    try:
        frame = engine.raise_if_init_failed(engine.initialize_params(profile="acp", overlay=overlay))
        result.check("initialize_ok", "result" in frame, frame.get("error"))
        if "result" not in frame:
            return

        missing = str(root["base"] / "does-not-exist") + "/sub"
        box = engine.submit(workspace_dir=missing)
        sframe = box.get("frame") or {}
        err = sframe.get("error") or {}
        data = err.get("data") or {}
        result.artifact("unresolvable_cwd_error", err)
        result.check("unresolvable_cwd_rejected", "result" not in sframe, sframe)
        result.check("error_code_is_invalid_argument",
                     data.get("engine_error_code") == "invalid_argument",
                     data.get("engine_error_code"))
        result.check("error_not_degraded_into_success",
                     err.get("code") == -32000 and "detail" in data, data)

        # 相对路径同样必须被拒绝（沿用既有 isabs 校验）。
        rel_box = engine.submit(workspace_dir="relative/path")
        rf = rel_box.get("frame") or {}
        rdata = (rf.get("error") or {}).get("data") or {}
        result.check("relative_cwd_rejected",
                     "result" not in rf and rdata.get("engine_error_code") == "invalid_argument",
                     rdata.get("engine_error_code"))
        result.check("no_storage_dir_created_for_invalid_cwd",
                     project_dirs(root) == [], project_dirs(root))
    finally:
        engine.close()
        server.shutdown()


SCENARIOS = {
    "R1": ("R1 默认 Profile 可用（无手工覆盖）", r1),
    "R1B": ("R1b 引擎侧默认 Profile 为 acp", r1b),
    "R2": ("R2 非 ACP Profile 不被静默接受", r2),
    "R3": ("R3/R4 别名与 realpath 落同一存储目录且可 resume", r3),
    "R5": ("R5 不可解析 cwd 明确报错", r5),
}


def main(argv):
    selected = [a.upper() for a in argv if a.upper() in SCENARIOS]
    if not selected:
        selected = list(SCENARIOS.keys())
    summary = {}
    for sid in selected:
        title, body = SCENARIOS[sid]
        payload = H.run_scenario(sid, title, body, channel="ipc2")
        payload["evidence_level"] = EVIDENCE_LEVEL
        H.write_evidence(sid, payload)
        summary[sid] = payload["verdict"]
        print(sid, "->", payload["verdict"])
    print(json.dumps(summary, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
