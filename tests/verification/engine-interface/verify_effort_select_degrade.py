#!/usr/bin/env python3
"""OD-ARCH-176 回归：真实 DSH 下 selectModel 拒绝未声明模型的显式档位时，
适配器必须优雅降级（按模型默认继续），不得炸掉整轮任务。

背景（2026-11-17 线上回归）：DSH session/selectModel 对 reasoning=false 的模型
拒绝任何显式 reasoningEffort（含 off）。修复后：select_model 失败记 stderr 并
按「跟随模型默认」继续。

证据等级：script（真实 DSH web 进程 + 合成模型端点，零真实凭证、零外网）。
"""
import io
import json
import sys
import time
from pathlib import Path

ADAPTER_DIR = Path(__file__).resolve().parent / "adapter"
REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ADAPTER_DIR))
sys.path.insert(0, str(REPO / "engine"))

import _harness as H
from _mock_model_server import start_mock
from omnidesk_engine import SubmitTaskRequest

EVIDENCE = REPO / "tests/verification/engine-interface/evidence/web-api"
FAILS: list[str] = []


def check(name, ok, detail=""):
    print(("PASS" if ok else "FAIL") + " " + name + ("" if ok else " :: " + str(detail)[:400]))
    if not ok:
        FAILS.append(name)


def main() -> int:
    base_url, ctl, server = start_mock()
    root = H.new_root("arch176_effort_degrade")
    overlay = H.write_overlay(root, "repro", H.web_overlay_text(base_url))
    adapter = H.make_web_adapter(root, [overlay], init_timeout=60, session_timeout=45,
                                 env_extra={H.FAKE_KEY_ENV: H.FAKE_KEY_VALUE})
    tid = None
    captured_err = io.StringIO()
    real_err = sys.stderr
    sys.stderr = captured_err
    state, note = None, ""
    try:
        adapter.initialize()
        receipt = adapter.submit_task(SubmitTaskRequest(
            intent="weekly_report",
            workspace_dir=str(root["cwd"]),
            prompt_text=H.SYNTH_PROMPT,
            reasoning_effort="off",
            provider="fake",
            model="synthetic-model",
        ))
        tid = receipt.get("omni_task_id") if isinstance(receipt, dict) else receipt.omni_task_id
        check("submit_not_rejected", tid is not None, receipt)
        for _ in range(60):
            st = adapter.get_task_status(tid)
            state = st.get("status") if isinstance(st, dict) else getattr(st, "status", None)
            note = str(st.get("note") or "") if isinstance(st, dict) else str(getattr(st, "note", ""))
            if state in ("succeeded", "failed", "interrupted", "cancelled", "unknown"):
                break
            time.sleep(1)
        check("task_reaches_terminal", state is not None, "60s 未到终态")
        check("task_not_failed", state == "succeeded", f"state={state} note={note[:300]}")
        check("degrade_logged", "select_model 被拒" in captured_err.getvalue(),
              captured_err.getvalue()[-300:])
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        (EVIDENCE / "effort-select-degrade.json").write_text(json.dumps({
            "task_id": tid, "state": state, "note_head": note[:300],
        }, ensure_ascii=False, indent=2), encoding="utf-8")
    except Exception as exc:
        check("no_uncaught_engine_error", False, repr(exc)[:500])
    finally:
        sys.stderr = real_err
        try:
            adapter.shutdown(mode="graceful", grace_period=5)
        except Exception:
            pass
        try:
            server.shutdown()
        except Exception:
            pass
    print(f"cases={4} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
