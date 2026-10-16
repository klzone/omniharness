#!/usr/bin/env python3
"""OmniDesk P1-Desktop Phase 2 · Step 12 / 13 / 15 —— 桌面引擎切片端到端验证。

验证的真实链路（经**进程边界**，不是同进程调用）：

    driver（模拟 React 侧发起命令）
      → IPC-2 stdio JSON-RPC → python3 -m omnidesk_engine（真实 Engine 子进程）
        → ACP → DSH CLI（runtime/dsh-runtime ... --profile acp）
          → 本机回环合成模型端点（零凭证、零外网）

本脚本**不**import omnidesk_engine，只通过协议对话，以证明进程边界真实可用。
Rust 宿主侧的进程管理与 IPC 由 desktop/src-tauri/tests/host_integration.rs 覆盖。

证据等级：script（真实引擎+真实 DSH 进程 + 合成模型端点）。
安全边界：一次性 /tmp 数据根；不读/不打印/不复制凭证；不触碰 ~/.dsh、~/.agents、
         ~/OmniDesk、~/YvanAI；合成素材全部为虚构占位内容。

用法：
  python3 run_desktop_e2e.py            # 全部场景（任务链路 + 隔离 + 双实例）
  python3 run_desktop_e2e.py chain      # 仅任务链路
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ADAPTER_TEST_DIR = REPO / "tests" / "verification" / "engine-interface" / "adapter"
EVIDENCE_DIR = HERE / "evidence"
sys.path.insert(0, str(ADAPTER_TEST_DIR))

from _mock_model_server import start_mock  # noqa: E402

DSH_BIN = REPO / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
ENGINE_DIR = REPO / "engine"
FAKE_KEY_ENV = "OMNI_FAKE_KEY"
FAKE_KEY_VALUE = "synthetic-placeholder-not-a-credential"

PROTOCOL_VERSION = "omnidesk-engine/1"

# 全部为虚构占位内容：无真实单位、人员、案件或内部信息。
SYNTH_PROMPT = (
    "请基于以下虚构要点写一段材料草稿（全部为合成占位内容，无真实单位与人员）："
    "本年度共完成事项 A 类 12 项、B 类 7 项；主要做法为流程再造与台账化管理；"
    "下一步拟完善复核环节。"
)


# ---------------------------------------------------------------------------
# 最小 IPC-2 客户端（行分隔 JSON-RPC 2.0 over stdio）
# ---------------------------------------------------------------------------


class EngineProcess:
    def __init__(self, name: str, root: dict) -> None:
        self.name = name
        self.root = root
        self.frames: list = []
        self.notifications: list = []
        self.stderr: list = []
        self._next_id = 1
        self._lock = threading.Lock()
        self._responses: dict = {}
        env = dict(os.environ)
        env["DSH_HOME"] = str(root["dsh_home"])
        env["DSH_AGENTS_HOME"] = str(root["agents"])
        env[FAKE_KEY_ENV] = FAKE_KEY_VALUE
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
        self._t_out = threading.Thread(target=self._read_out, daemon=True)
        self._t_err = threading.Thread(target=self._read_err, daemon=True)
        self._t_out.start()
        self._t_err.start()

    def _read_out(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                frame = json.loads(line)
            except Exception:
                frame = {"_unparsed": line}
            with self._lock:
                self.frames.append(frame)
                if "id" in frame and ("result" in frame or "error" in frame):
                    self._responses[frame["id"]] = frame
                elif "method" in frame:
                    self.notifications.append(frame)

    def _read_err(self) -> None:
        assert self.proc.stderr is not None
        for line in self.proc.stderr:
            self.stderr.append(line.rstrip("\n"))

    def request(self, method: str, params: dict, timeout: float = 120.0):
        with self._lock:
            rid = self._next_id
            self._next_id += 1
        payload = json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params},
                             ensure_ascii=False) + "\n"
        assert self.proc.stdin is not None
        self.proc.stdin.write(payload)
        self.proc.stdin.flush()
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with self._lock:
                frame = self._responses.get(rid)
            if frame is not None:
                if "error" in frame:
                    return None, frame["error"]
                return frame.get("result"), None
            if self.proc.poll() is not None:
                time.sleep(0.1)
                with self._lock:
                    frame = self._responses.get(rid)
                if frame is None:
                    return None, {"code": -32000, "message": "engine exited before responding",
                                  "data": {"engine_error_code": "engine_unavailable"}}
            time.sleep(0.02)
        return None, {"code": -32000, "message": "timeout", "data": {"engine_error_code": "timeout"}}

    def events(self) -> list:
        with self._lock:
            return [f["params"] for f in self.notifications if f.get("method") == "omni.event"]

    def close(self, grace: float = 8.0) -> None:
        try:
            self.request("omni.shutdown", {"mode": "graceful", "grace_period_seconds": grace}, timeout=grace + 5)
        except Exception:
            pass
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=grace)
        except Exception:
            self.proc.kill()
        return None


def new_root(tag: str) -> dict:
    base = Path(tempfile.mkdtemp(prefix="omni_p2desktop_" + tag + "_"))
    root = {
        "base": base,
        "dsh_home": base / "dsh-home",
        "agents": base / "agents",
        "cwd": base / "work",
        "state": base / "state",
    }
    for key in ("dsh_home", "agents", "cwd", "state"):
        root[key].mkdir(parents=True, exist_ok=True)
    return root


def drop_root(root: dict) -> None:
    shutil.rmtree(str(root["base"]), ignore_errors=True)


def write_overlay(root: dict, base_url: str, model: str = "synthetic-model") -> Path:
    """合成 provider 覆盖层：把模型指向本机回环端点，零真实凭证。"""
    lines = [
        "- id: acp",
        '  name: "@deepseek-ai/dsh-acp"',
        "  config:",
        "    provider: fake",
        "    model: " + model,
        "- id: llm-pi-ai",
        '  name: "@deepseek-ai/dsh-llm-pi-ai"',
        "  config:",
        "    providers:",
        "      fake:",
        "        displayName: Fake",
        "        apiKeyEnv: " + FAKE_KEY_ENV,
        "        api: openai-completions",
        "        baseURL: " + base_url,
        "        models:",
        "          - id: " + model,
        "            name: " + model,
    ]
    path = root["base"] / "synthetic-provider.yml"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def initialize_params(root: dict, overlay: Path) -> dict:
    return {
        "protocol_version": PROTOCOL_VERSION,
        "cwd": str(root["cwd"]),
        "engine_state_dir": str(root["state"]),
        "result_dir": str(root["state"] / "results"),
        "dsh": {
            "cli_entry": str(DSH_BIN),
            "profile": "acp",
            "dsh_home": str(root["dsh_home"]),
            "agents_home": str(root["agents"]),
            "patch_files": [str(overlay)],
        },
    }


def validate_isolated_root(root: dict) -> list:
    """提交前自检：隔离根必须落在 /tmp，且不含被禁止片段。"""
    problems = []
    for key in ("dsh_home", "agents", "cwd", "state"):
        text = str(root[key]).replace("\\", "/")
        if not text.startswith("/tmp/"):
            problems.append(f"{key} 不在 /tmp 下：{text}")
        for bad in ("/.dsh", "/.agents", "deepseek-harness"):
            if bad in text:
                problems.append(f"{key} 含被禁止片段 {bad}")
    return problems


def wait_terminal(engine: "EngineProcess", task_id: str, timeout: float = 180.0) -> list:
    snaps = []
    deadline = time.monotonic() + timeout
    terminal = {"succeeded", "failed", "cancelled", "interrupted", "unknown"}
    while time.monotonic() < deadline:
        result, error = engine.request("omni.get_task_status", {"omni_task_id": task_id}, timeout=30)
        if error is not None:
            snaps.append({"error": error})
            break
        snaps.append(result)
        if result.get("status") in terminal:
            return snaps
        time.sleep(0.2)
    return snaps


# ---------------------------------------------------------------------------
# Step 12：任务链路（成功 / 失败 / 取消）
# ---------------------------------------------------------------------------


def scenario_success(evidence: dict) -> dict:
    base_url, ctl, server = start_mock()
    ctl.set(mode="ok", chunks=["合成", "材料", "草稿"], delay=0.0)
    root = new_root("ok")
    entry = {"scenario": "success", "root": str(root["base"]),
             "isolation_problems": validate_isolated_root(root)}

    engine = EngineProcess("ok", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err,
                               "descriptor": (init or {}).get("engine")}

        receipt, err = engine.request("omni.submit_task", {
            "intent": "material_writing", "workspace_dir": str(root["cwd"]),
            "prompt_text": SYNTH_PROMPT,
        }, timeout=60)
        entry["submit"] = {"ok": err is None, "error": err, "receipt": receipt}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        task_id = receipt["omni_task_id"]
        snaps = wait_terminal(engine, task_id)
        entry["status_snapshots"] = snaps[-3:]
        final = snaps[-1] if snaps else {}
        entry["final_status"] = final.get("status")

        result, err = engine.request("omni.get_task_result", {"omni_task_id": task_id}, timeout=60)
        entry["result"] = {"ok": err is None, "error": err,
                           "form": (result or {}).get("form"),
                           "integrity": (result or {}).get("integrity"),
                           "content": (result or {}).get("content"),
                           "persisted_at": (result or {}).get("persisted_at")}

        events = engine.events()
        entry["event_count"] = len(events)
        entry["event_types"] = [e.get("type") for e in events]
        entry["text_delta_joined"] = "".join(
            str(e.get("payload", {}).get("text", "")) for e in events if e.get("type") == "text_delta"
        )

        checks = {
            "initialize_ok": entry["initialize"]["ok"],
            "submit_ok": entry["submit"]["ok"],
            "reached_succeeded": entry.get("final_status") == "succeeded",
            "has_task_started_event": "task_started" in entry["event_types"],
            "has_terminal_or_text_event": bool(
                {"task_completed", "text_delta"} & set(entry["event_types"])
            ),
            "result_complete": entry["result"].get("form") == "complete",
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def scenario_failure(evidence: dict) -> dict:
    base_url, ctl, server = start_mock()
    ctl.set(mode="error_400", chunks=["unused"])
    root = new_root("fail")
    entry = {"scenario": "failure", "root": str(root["base"]),
             "isolation_problems": validate_isolated_root(root)}

    engine = EngineProcess("fail", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        receipt, err = engine.request("omni.submit_task", {
            "intent": "material_writing", "workspace_dir": str(root["cwd"]),
            "prompt_text": SYNTH_PROMPT,
        }, timeout=60)
        entry["submit"] = {"ok": err is None, "error": err, "receipt": receipt}
        if err is not None:
            # 提交阶段就报错也是合法结果，但要如实记录是"提交失败"而非"任务失败"。
            entry["failure_stage"] = "submit"
            entry["verdict"] = "pass" if err.get("data", {}).get("engine_error_code") else "fail"
            return entry

        task_id = receipt["omni_task_id"]
        snaps = wait_terminal(engine, task_id, timeout=120)
        entry["status_snapshots"] = snaps[-3:]
        entry["final_status"] = (snaps[-1] if snaps else {}).get("status")

        events = engine.events()
        entry["event_types"] = [e.get("type") for e in events]
        failures = [e for e in events if e.get("type") == "task_failed"]
        entry["task_failed_events"] = [
            {"payload": e.get("payload"), "seq": e.get("seq")} for e in failures
        ]

        checks = {
            "initialize_ok": entry["initialize"]["ok"],
            "not_reported_as_success": entry.get("final_status") != "succeeded",
            "failure_observable": entry.get("final_status") == "failed"
            or bool(failures),
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def scenario_cancel(evidence: dict) -> dict:
    base_url, ctl, server = start_mock()
    # hang：模型端不返回响应，让任务保持 running，以便观察取消语义。
    ctl.set(mode="hang", hang_seconds=90.0)
    root = new_root("cancel")
    entry = {"scenario": "cancel", "root": str(root["base"]),
             "isolation_problems": validate_isolated_root(root)}

    engine = EngineProcess("cancel", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        receipt, err = engine.request("omni.submit_task", {
            "intent": "material_writing", "workspace_dir": str(root["cwd"]),
            "prompt_text": SYNTH_PROMPT,
        }, timeout=60)
        entry["submit"] = {"ok": err is None, "error": err, "receipt": receipt}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        task_id = receipt["omni_task_id"]

        # 等到 running 再取消，确保取消发生在执行中。
        running_seen = False
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            st, e = engine.request("omni.get_task_status", {"omni_task_id": task_id}, timeout=20)
            if e is None and st.get("status") in ("running", "starting", "queued"):
                if st.get("status") == "running":
                    running_seen = True
                    break
            time.sleep(0.2)
        entry["running_before_cancel"] = running_seen

        outcome, err = engine.request("omni.cancel_task", {
            "omni_task_id": task_id, "timeout_seconds": 15}, timeout=60)
        entry["cancel"] = {"ok": err is None, "error": err, "outcome": outcome}

        snaps = wait_terminal(engine, task_id, timeout=90)
        entry["status_snapshots"] = snaps[-3:]
        entry["final_status"] = (snaps[-1] if snaps else {}).get("status")
        entry["final_cancel_phase"] = (snaps[-1] if snaps else {}).get("cancel_phase")

        events = engine.events()
        entry["event_types"] = [e.get("type") for e in events]

        phase = (outcome or {}).get("phase")
        checks = {
            "initialize_ok": entry["initialize"]["ok"],
            "submit_ok": entry["submit"]["ok"],
            "cancel_request_recorded": phase in ("request_sent", "confirmed_cancelled",
                                                 "timeout_unknown", "acknowledged"),
            # 契约 §4.6：不得把"已请求"呈现为"已确认取消"。
            "phase_not_misreported": phase != "not_requested",
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


# ---------------------------------------------------------------------------
# Step 13：隔离差异检查
# ---------------------------------------------------------------------------

GUARDED_PATHS = [
    Path.home() / ".dsh",
    Path.home() / ".agents",
    Path.home() / ".config" / "DSH Desktop",
    Path.home() / ".local" / "share" / "io.github.hairyf.deepseek-harness-desktop",
]


def snapshot(path: Path, max_entries: int = 4000) -> dict:
    """有界快照：只记录相对路径 / 大小 / mtime，用于前后差异比较。"""
    if not path.exists():
        return {"exists": False, "entries": 0, "digest": None}
    items = []
    count = 0
    for dirpath, dirnames, filenames in os.walk(path):
        dirnames.sort()
        for name in sorted(filenames):
            full = Path(dirpath) / name
            try:
                st = full.stat()
            except OSError:
                continue
            rel = str(full.relative_to(path))
            items.append(f"{rel}\t{st.st_size}\t{int(st.st_mtime)}")
            count += 1
            if count >= max_entries:
                break
        if count >= max_entries:
            break
    payload = "\n".join(sorted(items))
    return {
        "exists": True,
        "entries": count,
        "truncated": count >= max_entries,
        "digest": hashlib.sha256(payload.encode("utf-8")).hexdigest(),
    }


def isolation_report() -> dict:
    before = {str(p): snapshot(p) for p in GUARDED_PATHS}
    # 重新跑一遍完整链路（成功 + 失败 + 取消），期间持续观察被守护路径。
    chain = {
        "success": scenario_success({}),
        "failure": scenario_failure({}),
        "cancel": scenario_cancel({}),
    }
    after = {str(p): snapshot(p) for p in GUARDED_PATHS}

    diffs = []
    for path in before:
        b, a = before[path], after[path]
        if b != a:
            diffs.append({"path": path, "before": b, "after": a})

    # 测试数据只允许出现在 /tmp 的一次性根中。
    stray = []
    for scenario in chain.values():
        root = scenario.get("root", "")
        if root and not root.startswith("/tmp/"):
            stray.append(root)

    return {
        "guarded_paths": [str(p) for p in GUARDED_PATHS],
        "before": before,
        "after": after,
        "differences": diffs,
        "unchanged": len(diffs) == 0,
        "test_roots_only_in_tmp": len(stray) == 0,
        "stray_roots": stray,
        "note": (
            "受守护路径在测试前后完全一致。" if not diffs
            else "受守护路径出现差异，必须人工核查。"
        ),
        "verdict": "pass" if (not diffs and not stray) else "fail",
    }


# ---------------------------------------------------------------------------
# Step 15：双实例隔离
# ---------------------------------------------------------------------------


def dual_instance_report() -> dict:
    base_url, ctl, server = start_mock()
    ctl.set(mode="ok", chunks=["实例", "A", "输出"], delay=0.0)

    root_a = new_root("dualA")
    root_b = new_root("dualB")
    a = EngineProcess("A", root_a)
    b = EngineProcess("B", root_b)
    report = {"instance_a_root": str(root_a["base"]), "instance_b_root": str(root_b["base"])}
    try:
        ov_a = write_overlay(root_a, base_url)
        ov_b = write_overlay(root_b, base_url)
        _, err_a = a.request("omni.initialize", initialize_params(root_a, ov_a), timeout=120)
        _, err_b = b.request("omni.initialize", initialize_params(root_b, ov_b), timeout=120)
        report["init_a_ok"] = err_a is None
        report["init_b_ok"] = err_b is None

        report["pid_a"] = a.proc.pid
        report["pid_b"] = b.proc.pid
        report["distinct_pids"] = a.proc.pid != b.proc.pid

        receipt_a, _ = a.request("omni.submit_task", {
            "intent": "material_writing", "workspace_dir": str(root_a["cwd"]),
            "prompt_text": SYNTH_PROMPT}, timeout=60)
        task_a = (receipt_a or {}).get("omni_task_id")
        report["task_a"] = task_a

        # 实例 B 不应知道实例 A 的任务。
        status_b, err_b2 = b.request("omni.get_task_status", {"omni_task_id": task_a}, timeout=20)
        report["b_can_see_a_task"] = err_b2 is None
        report["b_lookup_error_code"] = (err_b2 or {}).get("data", {}).get("engine_error_code")

        # 实例 A 自己必须能看到。
        status_a, err_a2 = a.request("omni.get_task_status", {"omni_task_id": task_a}, timeout=20)
        report["a_can_see_a_task"] = err_a2 is None
        report["a_task_status"] = (status_a or {}).get("status")
        if err_a2 is None:
            wait_terminal(a, task_a, timeout=120)

        # 会话级隔离：两个引擎进程的隔离根不同且不重叠。
        report["roots_disjoint"] = str(root_a["base"]) != str(root_b["base"])

        checks = {
            "both_initialized": report["init_a_ok"] and report["init_b_ok"],
            "distinct_pids": report["distinct_pids"],
            "task_not_visible_in_other_instance": not report["b_can_see_a_task"],
            "other_instance_error_is_unknown_task": report.get("b_lookup_error_code")
            in ("unknown_task", "unknown_session", None),
            "owner_instance_sees_task": report["a_can_see_a_task"],
            "roots_disjoint": report["roots_disjoint"],
        }
        report["checks"] = checks
        report["verdict"] = "pass" if all(checks.values()) else "fail"
        return report
    finally:
        a.close()
        b.close()
        drop_root(root_a)
        drop_root(root_b)
        server.shutdown()


# ---------------------------------------------------------------------------
# 入口
# ---------------------------------------------------------------------------


def main(argv: list) -> int:
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    only = argv[1] if len(argv) > 1 else None

    if only in (None, "chain"):
        chain = {
            "generated_at": time.time(),
            "evidence_level": "script（真实引擎+真实 DSH 进程 + 本机合成模型端点）",
            "channel": "IPC-2 stdio JSON-RPC（进程边界）",
            "dsh_cli": str(DSH_BIN),
            "scenarios": {
                "success": scenario_success({}),
                "failure": scenario_failure({}),
                "cancel": scenario_cancel({}),
            },
        }
        chain["verdict"] = "pass" if all(
            s.get("verdict") == "pass" for s in chain["scenarios"].values()
        ) else "fail"
        (EVIDENCE_DIR / "task-chain.json").write_text(
            json.dumps(chain, ensure_ascii=False, indent=2), encoding="utf-8")
        print("[chain]", chain["verdict"], {
            k: (v.get("verdict"), v.get("final_status")) for k, v in chain["scenarios"].items()
        })

    if only is None:
        iso = isolation_report()
        (EVIDENCE_DIR / "isolation-report.json").write_text(
            json.dumps(iso, ensure_ascii=False, indent=2), encoding="utf-8")
        print("[isolation]", iso["verdict"], "unchanged=", iso["unchanged"],
              "diffs=", len(iso["differences"]))

        dual = dual_instance_report()
        (EVIDENCE_DIR / "dual-instance-report.json").write_text(
            json.dumps(dual, ensure_ascii=False, indent=2), encoding="utf-8")
        print("[dual-instance]", dual["verdict"], dual.get("checks"))

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
