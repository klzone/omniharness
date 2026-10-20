#!/usr/bin/env python3
"""OD-ARCH-034 单元级验证：适配层「新建 / 续接 / 降级」三条分支。

只测**适配层自己的决策**，不启动引擎进程：
  * 不带 engine_session_id → session/new（新建会话）；
  * 带 engine_session_id 且 resume 成功 → session/resume + 同一会话续发；
  * 带 engine_session_id 但 resume 失败 → **降级为新建会话**，并在回执里说明，
    不抛错、不丢历史（历史由调用方重放）。

证据等级：unit（桩替身，无引擎进程、无网络、无凭证）。
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "engine"))

from omnidesk_engine import AcpEngineAdapter, EngineError, EngineErrorCode  # noqa: E402
from omnidesk_engine.acp_adapter import SubmitTaskRequest  # noqa: E402

CHECKS: list[dict] = []


def check(name: str, ok: bool, detail=None) -> bool:
    entry = {"check": name, "pass": bool(ok)}
    if detail is not None:
        entry["detail"] = detail
    CHECKS.append(entry)
    print(("  PASS " if ok else "  FAIL ") + name + ("" if detail is None else "  " + json.dumps(detail, ensure_ascii=False)))
    return bool(ok)


def make_adapter(tmp: Path) -> tuple[AcpEngineAdapter, list]:
    calls: list = []

    def stub_request(method, params=None, timeout=0.0, omni_task_id=None):
        calls.append({"method": method, "params": dict(params or {})})
        if method == "session/new":
            return {"sessionId": "sess-stub-new"}
        if method == "session/resume":
            return {"sessionId": (params or {}).get("sessionId"), "configOptions": []}
        if method == "session/prompt":
            return {}
        return {}

    adapter = AcpEngineAdapter(
        command=["node", "/nonexistent/dsh"],  # 永远不会真的启动
        cwd=str(tmp),
        state_dir=str(tmp / "state"),
    )
    adapter._request = stub_request  # type: ignore[assignment]
    # 不真的发 prompt（避免等待引擎应答）；只记录本轮要发的文本。
    prompts: list = []
    adapter._start_prompt = lambda record, text: prompts.append(text)  # type: ignore[assignment]
    adapter._stub_prompts = prompts  # type: ignore[attr-defined]
    return adapter, calls


def main() -> int:
    tmp = Path(tempfile.mkdtemp(prefix="omni_arch034_unit_"))
    try:
        # --- 分支 1：首轮（无会话号）→ 新建会话 ---
        adapter, calls = make_adapter(tmp)
        receipt = adapter.submit_task(
            SubmitTaskRequest(
                intent="synthetic",
                workspace_dir=str(tmp),
                prompt_text="SYNTH-UNIT-TURN-ONE",
            )
        )
        methods = [c["method"] for c in calls]
        check("new_branch_uses_session_new", methods == ["session/new"], {"methods": methods})
        check("new_branch_returns_session_id", receipt.engine_session_id == "sess-stub-new", receipt.engine_session_id)
        check("new_branch_note_is_new", "建立会话" in receipt.note, receipt.note)
        check("new_branch_receipt_session_mode", receipt.session_mode == "new", receipt.session_mode)
        record = adapter._tasks[receipt.omni_task_id]
        check("new_branch_record_session_mode", record.session_mode == "new", record.session_mode)
        check("new_branch_prompt_sent_once", len(adapter._stub_prompts) == 1, len(adapter._stub_prompts))

        # --- 分支 2：续接既有会话 → session/resume + 同一会话 ---
        adapter2, calls2 = make_adapter(tmp)
        receipt2 = adapter2.submit_task(
            SubmitTaskRequest(
                intent="synthetic",
                workspace_dir=str(tmp),
                prompt_text="SYNTH-UNIT-TURN-TWO",
                engine_session_id="sess-stub-existing",
            )
        )
        methods2 = [c["method"] for c in calls2]
        check("resume_branch_uses_resume", methods2 == ["session/resume"], {"methods": methods2})
        check("resume_branch_no_session_new", "session/new" not in methods2, {"methods": methods2})
        check(
            "resume_branch_keeps_same_session",
            receipt2.engine_session_id == "sess-stub-existing",
            receipt2.engine_session_id,
        )
        check("resume_branch_note_says_resumed", "续接" in receipt2.note, receipt2.note)
        check("resume_branch_receipt_session_mode", receipt2.session_mode == "resumed", receipt2.session_mode)
        record2 = adapter2._tasks[receipt2.omni_task_id]
        check("resume_branch_record_session_mode", record2.session_mode == "resumed", record2.session_mode)
        resume_params = next(c["params"] for c in calls2 if c["method"] == "session/resume")
        check(
            "resume_branch_sends_session_id_and_cwd",
            resume_params.get("sessionId") == "sess-stub-existing"
            and resume_params.get("cwd") == str(tmp),
            {"keys": sorted(resume_params.keys())},
        )
        check(
            "resume_branch_prompt_is_only_current_turn",
            adapter2._stub_prompts == ["SYNTH-UNIT-TURN-TWO"],
            adapter2._stub_prompts,
        )

        # --- 分支 2b：会话在本进程里还活着 → 直接续发，不调 session/resume ---
        # DSH 的 session/resume 只接受「已落盘、可重开」的会话；对活会话会报 invalid_argument，
        # 所以同一进程内续发必须直接再发一次 session/prompt。
        adapter2b, calls2b = make_adapter(tmp)
        first = adapter2b.submit_task(
            SubmitTaskRequest(
                intent="synthetic",
                workspace_dir=str(tmp),
                prompt_text="SYNTH-UNIT-TURN-ONE",
            )
        )
        calls2b.clear()
        second = adapter2b.submit_task(
            SubmitTaskRequest(
                intent="synthetic",
                workspace_dir=str(tmp),
                prompt_text="SYNTH-UNIT-TURN-TWO",
                engine_session_id=first.engine_session_id,
            )
        )
        methods2b = [c["method"] for c in calls2b]
        check("live_session_branch_no_resume_call", "session/resume" not in methods2b, {"methods": methods2b})
        check("live_session_branch_no_new_session", "session/new" not in methods2b, {"methods": methods2b})
        check("live_session_branch_kept_session", second.engine_session_id == first.engine_session_id, second.engine_session_id)
        check("live_session_branch_note", "同一引擎会话" in second.note, second.note)
        check("live_session_branch_receipt_session_mode", second.session_mode == "continued", second.session_mode)
        record2b = adapter2b._tasks[second.omni_task_id]
        check("live_session_branch_record_mode", record2b.session_mode == "continued", record2b.session_mode)

        # --- 分支 3：续接失败 → 降级为新建会话，不抛错 ---
        adapter3, calls3 = make_adapter(tmp)

        def failing_request(method, params=None, timeout=0.0, omni_task_id=None):
            calls3.append({"method": method, "params": dict(params or {})})
            if method == "session/resume":
                raise EngineError(EngineErrorCode.UNKNOWN_SESSION, "synthetic unknown session")
            if method == "session/new":
                return {"sessionId": "sess-stub-fallback"}
            return {}

        adapter3._request = failing_request  # type: ignore[assignment]
        receipt3 = adapter3.submit_task(
            SubmitTaskRequest(
                intent="synthetic",
                workspace_dir=str(tmp),
                prompt_text="SYNTH-UNIT-TURN-THREE",
                engine_session_id="sess-stub-gone",
            )
        )
        methods3 = [c["method"] for c in calls3]
        check("degraded_branch_does_not_raise", receipt3.omni_task_id is not None, receipt3.omni_task_id)
        check(
            "degraded_branch_falls_back_to_new",
            methods3 == ["session/resume", "session/new"],
            {"methods": methods3},
        )
        check(
            "degraded_branch_returns_new_session",
            receipt3.engine_session_id == "sess-stub-fallback",
            receipt3.engine_session_id,
        )
        check("degraded_branch_note_explains", "新建会话" in receipt3.note, receipt3.note)
        check("degraded_branch_receipt_session_mode", receipt3.session_mode == "degraded", receipt3.session_mode)
        record3 = adapter3._tasks[receipt3.omni_task_id]
        check("degraded_branch_record_session_mode", record3.session_mode == "degraded", record3.session_mode)
        payload = adapter3._record_payload(record3)
        check("degraded_branch_ledger_has_session_mode", payload.get("session_mode") == "degraded", payload.get("session_mode"))
    finally:
        try:
            for p in tmp.rglob("*"):
                if p.is_file():
                    p.unlink()
            tmp.rmdir()
        except OSError:
            pass

    failed = [c for c in CHECKS if not c["pass"]]
    print("checks: %d total, %d failed" % (len(CHECKS), len(failed)))
    return 0 if CHECKS and not failed else 1


if __name__ == "__main__":
    sys.exit(main())
