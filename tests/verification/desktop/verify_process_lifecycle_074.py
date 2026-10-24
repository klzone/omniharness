#!/usr/bin/env python3
"""OD-ARCH-074 真实隔离运行验证：引擎所有权登记与孤儿进程回收。

全部使用 /tmp 下的临时数据根与合成 provider 覆盖层；不接触 ~/.dsh、
~/.agents、用户正在运行的 OmniDesk 实例，也不读取任何凭证。

证据等级：脚本/运行测试（真实启动隔离 DSH 进程，真实发信号）。
运行：python3 tests/verification/desktop/verify_process_lifecycle_074.py
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ENGINE_DIR = REPO / "engine"
DSH_BIN = (
    REPO / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
)
EVIDENCE = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-074"
FAKE_KEY_ENV = "OMNI_FAKE_KEY"
FAKE_KEY = "synthetic-placeholder-not-a-credential"

sys.path.insert(0, str(ENGINE_DIR))
from omnidesk_engine import process_owner as po  # noqa: E402


def make_root() -> dict:
    base = Path(tempfile.mkdtemp(prefix="od074-run-"))
    root = {
        "base": base,
        "dsh_home": base / "dsh-home",
        "agents": base / "agents-home",
        "state": base / "engine-state",
        "cwd": base / "workspace",
    }
    for key in ("dsh_home", "agents", "state", "cwd"):
        root[key].mkdir(parents=True, exist_ok=True)
    return root


def write_overlay(root: dict) -> Path:
    lines = [
        "- id: llm-pi-ai",
        '  name: "@deepseek-ai/dsh-llm-pi-ai"',
        "  config:",
        "    providers:",
        "      fake:",
        "        displayName: Fake",
        "        apiKeyEnv: " + FAKE_KEY_ENV,
        "        api: openai-completions",
        "        baseURL: http://127.0.0.1:9/v1",
        "        models:",
        "          - id: synthetic-model",
        "            name: synthetic-model",
    ]
    path = root["base"] / "synthetic-provider.yml"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


class Engine:
    def __init__(self, name: str, root: dict, overlay: Path):
        self.name = name
        self.root = root
        self._id = 0
        env = dict(os.environ)
        env["DSH_HOME"] = str(root["dsh_home"])
        env["DSH_AGENTS_HOME"] = str(root["agents"])
        env[FAKE_KEY_ENV] = FAKE_KEY
        self.proc = subprocess.Popen(
            [sys.executable, "-m", "omnidesk_engine"],
            cwd=str(ENGINE_DIR),
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.overlay = overlay
        self.stderr: list[str] = []

    def send(self, method: str, params: dict) -> int:
        self._id += 1
        payload = json.dumps(
            {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params},
            ensure_ascii=False,
        )
        assert self.proc.stdin is not None
        self.proc.stdin.write(payload + "\n")
        self.proc.stdin.flush()
        return self._id

    def read_response(self, rid: int, timeout: float = 180.0) -> dict:
        assert self.proc.stdout is not None
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                break
            frame = json.loads(line)
            if frame.get("id") == rid:
                return frame
        raise RuntimeError(self.name + ": 未收到响应 id=" + str(rid))

    def initialize(self) -> dict:
        rid = self.send(
            "omni.initialize",
            {
                "protocol_version": "omnidesk-engine/1",
                "cwd": str(self.root["cwd"]),
                "engine_state_dir": str(self.root["state"]),
                "result_dir": str(self.root["state"] / "results"),
                "dsh": {
                    "cli_entry": str(DSH_BIN),
                    "profile": "web",
                    "channel": "web-api",
                    "dsh_home": str(self.root["dsh_home"]),
                    "agents_home": str(self.root["agents"]),
                    "patch_files": [str(self.overlay)],
                },
            },
        )
        return self.read_response(rid)

    def shutdown(self) -> None:
        try:
            rid = self.send("omni.shutdown", {"mode": "graceful", "grace_period_seconds": 5})
            self.read_response(rid, timeout=30)
        except Exception:
            pass
        try:
            self.proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=10)


def group_pids(pgid: int) -> list[int]:
    return [pid for pid, _ in po.group_members(pgid)] if pgid > 1 else []


def wait_empty(pgid: int, timeout: float = 20.0) -> bool:
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if not group_pids(pgid):
            return True
        time.sleep(0.1)
    return not group_pids(pgid)


def main() -> int:
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    report = {"checks": [], "root": "", "limitations": []}

    def record(name: str, verdict: str, **detail) -> None:
        report["checks"].append({"check": name, "verdict": verdict, **detail})
        print("[" + verdict + "] " + name, detail.get("note", ""))

    if not DSH_BIN.exists():
        record("dsh_runtime_present", "blocked", note="找不到隔离 DSH bin.js")
        (EVIDENCE / "process-lifecycle-074.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        return 1

    root = make_root()
    report["root"] = str(root["base"])
    overlay = write_overlay(root)
    owner_file = po.owner_path(root["state"])

    # --- A. 正常启动：登记存在，子进程组已记录 --------------------------
    engine = Engine("primary", root, overlay)
    reply = engine.initialize()
    ok = "result" in reply
    record(
        "A_initialize_ok",
        "pass" if ok else "fail",
        note="" if ok else str(reply.get("error"))[:400],
    )
    if not ok:
        engine.shutdown()
        (EVIDENCE / "process-lifecycle-074.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        return 1

    rec = po.read_owner(root["state"])
    record(
        "B_owner_record_written",
        "pass" if rec and rec.pid == engine.proc.pid else "fail",
        note="pid=%s pgid=%s child_pid=%s child_pgid=%s"
        % (rec and rec.pid, rec and rec.pgid, rec and rec.child_pid, rec and rec.child_pgid),
    )
    record(
        "C_owner_file_mode_0600",
        "pass" if owner_file.exists() and (owner_file.stat().st_mode & 0o777) == 0o600 else "fail",
    )
    record(
        "D_engine_pgid_is_own_session",
        "pass" if rec and rec.pgid == rec.pid else "fail",
        note="引擎自身为进程组组长",
    )
    child_pgid = rec.child_pgid if rec else 0
    child_members = group_pids(child_pgid)
    record(
        "E_dsh_child_group_live",
        "pass" if child_members else "fail",
        note="child_pgid=%s members=%s" % (child_pgid, child_members),
    )

    # --- B. 同根第二个实例必须被拒 --------------------------------------
    second = Engine("secondary", root, overlay)
    reply2 = second.initialize()
    refused = "error" in reply2
    record(
        "F_second_instance_refused",
        "pass" if refused else "fail",
        note=str(reply2.get("error", {}).get("message", ""))[:200],
    )
    record(
        "G_first_engine_still_alive",
        "pass" if engine.proc.poll() is None else "fail",
    )
    second.shutdown()

    # --- C. 优雅关闭：登记删除，无进程残留 ------------------------------
    engine_pid = engine.proc.pid
    engine.shutdown()
    record(
        "H_owner_record_removed_on_shutdown",
        "pass" if not owner_file.exists() else "fail",
    )
    record(
        "I_engine_group_drained",
        "pass" if wait_empty(engine_pid, 20) else "fail",
        note="pgid=%s members=%s" % (engine_pid, group_pids(engine_pid)),
    )
    record(
        "J_child_group_drained",
        "pass" if wait_empty(child_pgid, 20) else "fail",
        note="child_pgid=%s members=%s" % (child_pgid, group_pids(child_pgid)),
    )

    # --- D. 强杀引擎（模拟宿主崩溃）：登记留痕，子进程组成孤儿 ---------
    crashed = Engine("crashed", root, overlay)
    reply3 = crashed.initialize()
    record(
        "K_second_boot_initialize_ok",
        "pass" if "result" in reply3 else "fail",
        note=str(reply3.get("error", {}))[:300] if "error" in reply3 else "",
    )
    rec2 = po.read_owner(root["state"])
    orphan_pgid = rec2.child_pgid if rec2 else 0
    orphan_engine_pid = crashed.proc.pid
    crashed.proc.send_signal(signal.SIGKILL)
    crashed.proc.wait(timeout=15)
    time.sleep(0.5)
    record(
        "L_owner_record_survives_crash",
        "pass" if owner_file.exists() else "fail",
    )
    record(
        "M_orphan_child_group_left_behind",
        "pass" if group_pids(orphan_pgid) else "fail",
        note="child_pgid=%s members=%s" % (orphan_pgid, group_pids(orphan_pgid)),
    )
    state = po.inspect_owner(root["state"]).status
    record(
        "N_crash_state_is_orphan_group",
        "pass" if state in ("orphan_group", "pid_reused") else "fail",
        note="inspect=" + state,
    )

    # --- E. 宿主侧回收遗留进程组 ----------------------------------------
    try:
        outcome = po.reclaim_owner(root["state"], grace=5.0, our_host_pid=os.getpid())
        record("O_reclaim_outcome", "pass", action=outcome["action"], note=outcome["action"])
    except po.OwnerConflict as exc:
        record("O_reclaim_outcome", "fail", note=str(exc))
    record(
        "P_orphan_group_reclaimed",
        "pass" if wait_empty(orphan_pgid, 20) else "fail",
        note="child_pgid=%s members=%s" % (orphan_pgid, group_pids(orphan_pgid)),
    )
    record("Q_engine_group_gone_after_crash", "pass" if wait_empty(orphan_engine_pid, 10) else "fail")
    record("R_record_removed_after_reclaim", "pass" if not owner_file.exists() else "fail")

    # --- F. 边界自检 ----------------------------------------------------
    text_root = str(root["base"])
    record("S_root_is_temp_only", "pass" if text_root.startswith("/tmp/") else "fail", note=text_root)
    record(
        "T_no_original_dsh_paths",
        "pass" if "/.dsh" not in text_root and "/.agents" not in text_root else "fail",
    )
    record(
        "U_engine_state_under_temp_root",
        "pass" if str(root["state"]).startswith(text_root) else "fail",
    )

    failed = [c for c in report["checks"] if c["verdict"] != "pass"]
    report["summary"] = {
        "total": len(report["checks"]),
        "passed": len(report["checks"]) - len(failed),
        "failed": len(failed),
    }
    out = EVIDENCE / "process-lifecycle-074.json"
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("\n证据：" + str(out))
    print("合计 %d 项，失败 %d 项" % (report["summary"]["total"], len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
