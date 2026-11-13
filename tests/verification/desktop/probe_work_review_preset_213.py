#!/usr/bin/env python3
"""OD-HAR-213：Work Review 场景在合成端点下的 preset 接线探针。

一次性 /tmp 数据根；只装 work-review-core 与合成模型端点；不读真实会话、不碰凭证。
证据落 evidence/od-har-213/preset-probe.json（仅状态与 preset 名）。
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "tests/verification/engine-interface/adapter"))
sys.path.insert(0, str(REPO / "engine"))

import _harness as H  # noqa: E402
from _mock_model_server import start_mock  # noqa: E402
from omnidesk_engine import SubmitTaskRequest  # noqa: E402

EVIDENCE = HERE / "evidence" / "od-har-213"
ZSTD = Path("/opt/homebrew/bin/zstd")


def main() -> int:
    root = H.new_root("har213")
    profile_dir = root["dsh_home"] / "profiles" / "discipline"
    profile_dir.mkdir(parents=True)
    (profile_dir / "package.json").write_text(
        json.dumps(
            {
                "name": "dsh-profile-discipline",
                "private": True,
                "dsh": {"profile": {"bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"]}},
            }
        ),
        encoding="utf-8",
    )

    block = (REPO / "samples/profiles/discipline.profile.patch.yml").read_text(encoding="utf-8")
    match = re.search(
        r"- insert:\n(    - id: preset-work-review-core.*?)(?:^# 检索收敛规则|\Z)",
        block,
        re.M | re.S,
    )
    if match is None:
        print(json.dumps({"result": "fail", "reason": "preset block missing"}))
        return 1
    (profile_dir / "cordis.patch.yml").write_text(match.group(0).rstrip() + "\n", encoding="utf-8")
    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "route", H.web_overlay_text(base_url))
    adapter = H.make_web_adapter(root, [overlay], profile="discipline")
    facts: dict = {"result": "fail"}
    try:
        adapter.initialize()
        receipt = adapter.submit_task(
            SubmitTaskRequest(
                intent="weekly_report",
                workspace_dir=str(root["cwd"]),
                prompt_text="合成：今天完成了合成事项 A。",
            )
        )
        deadline = time.time() + 90
        status = None
        while time.time() < deadline:
            status = adapter.get_task_status(receipt.omni_task_id)
            if status.status.value in ("succeeded", "failed", "cancelled"):
                break
            time.sleep(0.4)
        sessions = list((root["dsh_home"] / "sessions").rglob("session.v4.jsonl.zstd"))
        agent_preset = None
        if sessions and ZSTD.exists():
            out = subprocess.run(
                [str(ZSTD), "-dc", str(sessions[0])],
                capture_output=True,
                text=True,
            ).stdout.splitlines()
            if out:
                try:
                    agent_preset = json.loads(out[0]).get("agentPreset")
                except ValueError:
                    agent_preset = None
        facts = {
            "result": "pass"
            if status is not None
            and status.status.value == "succeeded"
            and agent_preset == "work-review-core"
            else "fail",
            "terminal_state": None if status is None else status.status.value,
            "session_mode": receipt.session_mode,
            "agent_preset": agent_preset,
            "sessions_written": len(sessions),
            "evidence_level": "script（隔离 /tmp 数据根 + 合成端点，零真实凭证）",
        }
    finally:
        server.shutdown()
        H.drop_root(root)

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / "preset-probe.json").write_text(
        json.dumps(facts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(facts, ensure_ascii=False))
    return 0 if facts["result"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
