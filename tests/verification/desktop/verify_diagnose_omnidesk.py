#!/usr/bin/env python3
"""校验 scripts/diagnose-omnidesk.py：聚合一次、预算可见、不泄露任务正文/凭证。"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SCRIPT = REPO / "scripts" / "diagnose-omnidesk.py"
SECRET = "SYNTHETIC_BUSINESS_TEXT_DO_NOT_EXPOSE"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    check("script_exists", SCRIPT.is_file(), "聚合诊断脚本存在")
    with tempfile.TemporaryDirectory(prefix="od-diag-") as td:
        root = Path(td)
        (root / "config").mkdir()
        (root / "engine-state").mkdir()
        (root / "logs").mkdir()
        (root / "config" / "settings.v1.json").write_text(
            json.dumps(
                {
                    "schema": "omnidesk.settings.v1",
                    "engine_profile": "diag-test",
                    "model_permissions": {"local": True, "private": True, "public": False},
                }
            ),
            encoding="utf-8",
        )
        (root / "config" / "omnidesk-model.yml").write_text(
            "provider: synthetic\nbase_url: https://endpoint.example.com/v1\n"
            "id: synthetic-model\nreasoningEffort: low\napi_key: supersecret\n",
            encoding="utf-8",
        )
        (root / "engine-state" / "omni-diag-0001.json").write_text(
            json.dumps(
                {
                    "omni_task_id": "omni-diag-0001",
                    "state": "starting",
                    "seq": 0,
                    "persisted_at": 1.0,
                    "pending_approvals": 0,
                    "content": SECRET,
                    "error_message": "synthetic error",
                }
            ),
            encoding="utf-8",
        )
        (root / "logs" / "engine-stderr.log").write_text(
            "synthetic error line\n" + SECRET + "\n",
            encoding="utf-8",
        )
        proc = subprocess.run(
            [sys.executable, str(SCRIPT), "--app-root", str(root), "--latest", "1", "--json"],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        check("script_exit_zero", proc.returncode == 0, f"exit={proc.returncode}")
        try:
            report = json.loads(proc.stdout)
        except ValueError:
            report = {}
        check("json_shape", isinstance(report.get("budget"), dict) and isinstance(report.get("tasks"), list),
              "输出包含 budget 与 tasks")
        check("budget_visible", report.get("budget", {}).get("tool_rounds_max") == 8,
              "预算为 8 个工具轮次")
        check("task_metadata_present", bool(report.get("tasks")), "读取到最近任务快照")
        task = (report.get("tasks") or [{}])[0]
        check("task_no_content", "content" not in task and "error_message" not in task,
              "任务正文与错误正文不进入输出")
        check("observer_stale_flag", task.get("observer_may_be_stale") is True,
              "starting 旧快照标记 observer_may_be_stale")
        model = report.get("model", {})
        check("endpoint_host_only", model.get("endpoint_hosts") == ["https://endpoint.example.com"],
              "模型 endpoint 只保留 host")
        check("model_ids_visible", model.get("model_ids") == ["synthetic-model"],
              "模型标识可见，便于判断 thinking 配置")
        check("reasoning_efforts_visible", model.get("reasoning_efforts") == ["low"],
              "思考档位可见，便于判断模型是否声明该档")
        check("no_secret_in_json", "supersecret" not in proc.stdout and SECRET not in proc.stdout,
              "JSON 输出不含凭证或任务正文")
        text_proc = subprocess.run(
            [sys.executable, str(SCRIPT), "--app-root", str(root), "--latest", "1"],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        check("no_secret_in_text", "supersecret" not in text_proc.stdout and SECRET not in text_proc.stdout,
              "文本输出同样不含凭证或任务正文")

    passed = sum(1 for _, ok, _ in CHECKS if ok)
    total = len(CHECKS)
    for name, ok, detail in CHECKS:
        print(("PASS" if ok else "FAIL") + f" {name}" + (f" - {detail}" if detail else ""))
    print()
    print(f"{passed}/{total} checks passed")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
