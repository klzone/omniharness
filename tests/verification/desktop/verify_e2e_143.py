#!/usr/bin/env python3
"""OD-ARCH-143 S2：合成数据全链路验收。

覆盖：知识库初始化/整理/索引/归档、词典人名出站占位符、假模型回文还原、
随手记与周报/四象限回归。只用 /tmp 合成数据与回环假模型，不接触真实 Vault。
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parents[3]
ADAPTER = REPO / "tests/verification/engine-interface/adapter"
EVIDENCE = REPO / "tests/verification/desktop/evidence/od-arch-143"
SENSITIVE = "林青禾"
PLACEHOLDER = "[PERSON_A1B2C3D4]"

sys.path.insert(0, str(ADAPTER))
import _harness as H  # noqa: E402
from _mock_model_server import start_mock  # noqa: E402

from omnidesk_engine import SubmitTaskRequest, TaskState  # noqa: E402
from omnidesk_engine.security import masker as masker_mod  # noqa: E402


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["PATH"] = str(Path.home() / ".cargo/bin") + os.pathsep + env.get("PATH", "")
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=180, env=env)


def wait_terminal(adapter, task_id: str, timeout: float = 60.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status = adapter.get_task_status(task_id)
        if status.status in (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
                             TaskState.INTERRUPTED, TaskState.UNKNOWN):
            return status
        time.sleep(0.1)
    return adapter.get_task_status(task_id)


def main() -> int:
    checks = []

    def check(name: str, ok: bool, detail="") -> None:
        checks.append({"name": name, "ok": bool(ok), "detail": str(detail)})

    rust = run(["cargo", "test", "--offline", "--test", "knowledge_ingest_e2e_143"],
               REPO / "desktop/src-tauri")
    check("rust_knowledge_ingest_e2e", rust.returncode == 0, rust.stdout[-800:] + rust.stderr[-800:])

    root = H.new_root("od143")
    server = None
    adapter = None
    adapter_facts = {}
    try:
        dictionary = root["base"] / "sensitive-dict.v1.json"
        dictionary.write_text(json.dumps({
            "schema": "omnidesk.sensitive-dict.v1",
            "entries": [{"type": "PERSON", "value": SENSITIVE}],
        }, ensure_ascii=False), encoding="utf-8")
        base_url, ctl, server = start_mock()
        overlay = H.write_overlay(root, "od143", H.web_overlay_text(base_url))
        ctl.set(mode="ok", chunks=[PLACEHOLDER, " 负责专项检查。"], delay=0.0)
        with mock.patch.object(masker_mod.secrets, "token_hex", return_value="A1B2C3D4"):
            adapter = H.make_web_adapter(
                root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60,
                env_extra={"OMNIDESK_SENSITIVE_DICT": str(dictionary)},
            )
            adapter.initialize()
            receipt = adapter.submit_task(SubmitTaskRequest(
                intent="synthetic-e2e", workspace_dir=str(root["cwd"]),
                prompt_text="请总结" + SENSITIVE + "的专项检查工作表现。",
            ))
            status = wait_terminal(adapter, receipt.omni_task_id)
            events = [e.to_dict() for e in adapter.stream_events(receipt.omni_task_id, timeout=60)]
            result = adapter.get_task_result(receipt.omni_task_id)
            requests = ctl.new_requests()
        outbound = json.dumps(requests, ensure_ascii=False)
        stderr = adapter.engine_stderr() if adapter is not None else ""
        adapter_facts = {
            "status": status.status.value,
            "result_has_original": SENSITIVE in result.content,
            "result_has_placeholder": PLACEHOLDER in result.content,
            "outbound_has_original": SENSITIVE in outbound,
            "outbound_has_placeholder": PLACEHOLDER in outbound,
            "stderr_has_original": SENSITIVE in stderr,
            "text_deltas_have_original": any(
                SENSITIVE in e.get("payload", {}).get("text", "")
                for e in events if e.get("type") == "text_delta"),
        }
        check("adapter_terminal_succeeded", status.status == TaskState.SUCCEEDED, status.status.value)
        check("outbound_placeholder", adapter_facts["outbound_has_placeholder"], "")
        check("outbound_no_original", not adapter_facts["outbound_has_original"], "")
        check("stderr_no_original", not adapter_facts["stderr_has_original"], "")
        check("answer_restored", adapter_facts["result_has_original"] and not adapter_facts["result_has_placeholder"], "")
        check("stream_restored", adapter_facts["text_deltas_have_original"], "")
    except Exception as exc:  # noqa: BLE001 - 证据里保留可读失败原因
        check("adapter_fake_model_e2e", False, type(exc).__name__ + ": " + str(exc))
    finally:
        if adapter is not None:
            try:
                adapter.shutdown(mode="graceful", grace_period=5)
            except Exception:
                pass
        if server is not None:
            server.shutdown()
        H.drop_root(root)

    worklog = run([sys.executable, "tests/verification/desktop/verify_worklog_089.py"], REPO)
    check("worklog_regression", worklog.returncode == 0, (worklog.stdout + worklog.stderr)[-500:])
    weekly = run([sys.executable, "tests/verification/desktop/verify_weekly_093.py"], REPO)
    check("weekly_quadrant_regression", weekly.returncode == 0, (weekly.stdout + weekly.stderr)[-500:])
    artifacts = run([sys.executable, "tests/verification/desktop/verify_weekly_100.py"], REPO)
    check("artifact_regression", artifacts.returncode == 0, (artifacts.stdout + artifacts.stderr)[-500:])

    passed = all(c["ok"] for c in checks)
    payload = {
        "plan": "OD-ARCH-143",
        "step": "S2",
        "passed": passed,
        "checks": checks,
        "adapter": adapter_facts,
        "rust_exit": rust.returncode,
        "worklog_exit": worklog.returncode,
        "weekly_exit": weekly.returncode,
        "artifact_exit": artifacts.returncode,
    }
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / "facts.json").write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    for item in checks:
        print(("PASS  " if item["ok"] else "FAIL  ") + item["name"] +
              (("  — " + item["detail"]) if not item["ok"] and item["detail"] else ""))
    print("%d/%d 通过" % (sum(1 for c in checks if c["ok"]), len(checks)))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

