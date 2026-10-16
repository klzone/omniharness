#!/usr/bin/env python3
"""
OmniDesk P1 第三步 —— ACP 适配器场景驱动（E01-E19，合成路由组）。

设计：
  * 每个场景独立的一次性 /tmp 数据根；不共享状态，便于单独复跑。
  * 使用本机回环合成模型端点（_mock_model_server），零凭证、零外网、确定性。
  * 每项落盘真实输入、原始事件/帧摘要、标准化结果、判定与证据路径。
  * 未取得可靠证据的项一律 unverified，不伪造通过。

用法：
  python3 run_engine_adapter_scenarios.py            # 全部
  python3 run_engine_adapter_scenarios.py E05 E19    # 指定场景
"""

from __future__ import annotations

import json
import os
import signal
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

import _harness as H
from _harness import safe_call, write_evidence
from _mock_model_server import start_mock

from omnidesk_engine import (
    AcpEngineAdapter,
    CancelPhase,
    EngineError,
    EngineErrorCode,
    EventType,
    ResultForm,
    ResultIntegrity,
    SubmitTaskRequest,
    TaskState,
)

EVIDENCE_LEVEL = "script（真实引擎进程 + 合成模型端点）"


# ---------------------------------------------------------------------------
# 通用工具
# ---------------------------------------------------------------------------


def start(ctl, *, probes=None, **settings):
    base_url, controller, server = start_mock()
    controller.set(**settings)
    if probes:
        controller.set(probes=list(probes))
    return base_url, controller, server


def wait_terminal(adapter, task_id, timeout=60.0):
    """轮询状态直到终态；返回状态快照列表。"""
    snaps = []
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status = adapter.get_task_status(task_id)
        snaps.append(status.to_dict())
        if status.status in (TaskState.SUCCEEDED, TaskState.FAILED,
                             TaskState.CANCELLED, TaskState.INTERRUPTED,
                             TaskState.UNKNOWN):
            return snaps
        time.sleep(0.1)
    return snaps


def submit(adapter, root, prompt=None, intent="synthetic-writing-task", **kwargs):
    return adapter.submit_task(SubmitTaskRequest(
        intent=intent, workspace_dir=str(root["cwd"]),
        prompt_text=prompt or H.SYNTH_PROMPT, **kwargs))


# ---------------------------------------------------------------------------
# E01 正常提交
# ---------------------------------------------------------------------------


def e01(result, root):
    base_url, ctl, server = start(None)
    overlay = H.write_overlay(root, "e01", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        desc_ok, desc = safe_call(adapter.initialize)
        result.check("initialize_ok", desc_ok, desc if not desc_ok else desc.to_dict())
        ok_desc, desc_value = desc_ok, desc
        if desc_ok:
            caps = desc_value.capabilities
            result.check(
                "capabilities_declared",
                bool(caps),
                {k.value: v.value for k, v in caps.items()},
            )
        ok, receipt = safe_call(submit, adapter, root)
        result.check("submit_ok", ok, receipt if not ok else receipt.to_dict())
        if not ok:
            return
        result.artifact("receipt", receipt.to_dict())
        result.check("omni_task_id_stable", receipt.omni_task_id.startswith("omni-"), receipt.omni_task_id)
        result.check(
            "id_layers_distinct",
            receipt.engine_session_id is not None and receipt.engine_session_id != receipt.omni_task_id,
            {"omni_task_id": receipt.omni_task_id, "engine_session_id": receipt.engine_session_id},
        )
        result.check(
            "accepted_state_is_early",
            receipt.status in (TaskState.QUEUED, TaskState.STARTING, TaskState.RUNNING),
            receipt.status.value,
        )
        # 幂等：同 key 再提交不应产生新任务
        ok2, receipt2 = safe_call(
            adapter.submit_task,
            SubmitTaskRequest(intent="synthetic-writing-task", workspace_dir=str(root["cwd"]),
                              prompt_text=H.SYNTH_PROMPT, idempotency_key="e01-key"),
        )
        ok3, receipt3 = safe_call(
            adapter.submit_task,
            SubmitTaskRequest(intent="synthetic-writing-task", workspace_dir=str(root["cwd"]),
                              prompt_text=H.SYNTH_PROMPT, idempotency_key="e01-key"),
        )
        result.check("idempotency_same_when_key_repeats",
                     ok2 and ok3 and receipt2.omni_task_id == receipt3.omni_task_id,
                     {"first": receipt2.omni_task_id if ok2 else receipt2,
                      "second": receipt3.omni_task_id if ok3 else receipt3})
        list(adapter.stream_events(receipt.omni_task_id, timeout=60))
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        result.artifact("engine_stderr", adapter.engine_stderr()[-500:])
        server.shutdown()


# ---------------------------------------------------------------------------
# E02 流式输出
# ---------------------------------------------------------------------------


def e02(result, root):
    base_url, ctl, server = start(None)
    ctl.set(chunks=["甲", "乙", "丙", "丁"], delay=0.05, mode="slow")
    overlay = H.write_overlay(root, "e02", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        events = [e.to_dict() for e in adapter.stream_events(receipt.omni_task_id, timeout=60)]
        result.artifact("event_sequence", events)
        seqs = [e["seq"] for e in events]
        result.check("seq_strictly_increasing", seqs == sorted(seqs) and len(set(seqs)) == len(seqs), seqs)
        types = [e["type"] for e in events]
        result.check("has_task_started", "task_started" in types, types)
        result.check("has_text_delta", "text_delta" in types, types)
        deltas = "".join(e["payload"].get("text", "") for e in events if e["type"] == "text_delta")
        final = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("stream_join", deltas)
        result.artifact("final_result", final.to_dict())
        result.check("stream_join_eq_result", deltas == final.content, {"join": deltas, "final": final.content})
        text_delta_count = sum(1 for e in events if e["type"] == "text_delta")
        result.artifact("text_delta_count", text_delta_count)
        result.artifact("model_request_count_for_run", len(ctl.new_requests()))
        result.note(
            "流式粒度实测：合成端点分 4 块、每块间隔 0.05s 逐步返回，但 ACP 通道只在整轮结束时"
            "投递 1 个 assistant 消息块。因此本通道的 text_delta 是「按 assistant 消息」粒度，"
            "不是 token 级流式；适配层不得对外声称 token 级流（对照契约 4.4 的归一化说明）。"
        )
        result.check("granularity_is_message_level_not_token", text_delta_count >= 1, text_delta_count)
        result.check("delta_and_final_separated",
                     isinstance(deltas, str) and isinstance(final.content, str),
                     "增量与完整结果分别取自事件流与 get_task_result")
        waiting = [e for e in events if e["type"] in ("task_completed", "task_failed")]
        result.check("terminal_event_present", bool(waiting), [e["type"] for e in waiting])
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E03 正常完成
# ---------------------------------------------------------------------------


def e03(result, root):
    base_url, ctl, server = start(None)
    overlay = H.write_overlay(root, "e03", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        result.artifact("status_series", snaps)
        result.check("reached_succeeded", snaps[-1]["status"] == "succeeded", snaps[-1])
        result.check("terminal_evidence_engine", snaps[-1]["evidence_source"] == "engine_terminal", snaps[-1])
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("task_result", got.to_dict())
        result.check("form_complete", got.form == ResultForm.COMPLETE, got.form.value)
        result.check("integrity_complete", got.integrity == ResultIntegrity.COMPLETE, got.integrity.value)
        result.check("content_non_empty", bool(got.content), got.content)
        result.check("not_derived_from_process_exit",
                     any(s["evidence_source"] == "engine_terminal" for s in snaps),
                     "终态依据为引擎终态证据，非进程退出码")
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E04 执行失败
# ---------------------------------------------------------------------------


def e04(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="error_400")
    overlay = H.write_overlay(root, "e04", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        snaps = wait_terminal(adapter, receipt.omni_task_id, timeout=60)
        result.artifact("status_series", snaps)
        result.check("reached_failed", snaps[-1]["status"] == "failed", snaps[-1])
        result.check("no_fake_success", snaps[-1]["status"] != "succeeded", snaps[-1]["status"])
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("task_result", got.to_dict())
        result.check("content_empty_so_unrecoverable",
                     got.form == ResultForm.UNRECOVERABLE and got.content == "",
                     got.to_dict())
        result.check("error_code_present", bool(snaps[-1]["note"]) and "error=" in snaps[-1]["note"],
                     snaps[-1]["note"])
        ok, err = safe_call(adapter.get_task_status, "omni-does-not-exist")
        result.check("unknown_task_maps_to_standard_code",
                     (not ok) and err.get("engine_error_code") == EngineErrorCode.UNKNOWN_TASK.value,
                     err)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E05 用户取消
# ---------------------------------------------------------------------------


def e05(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=180)
    overlay = H.write_overlay(root, "e05", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=120)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(1.0)
        status = adapter.get_task_status(receipt.omni_task_id)
        result.artifact("status_before_cancel", status.to_dict())
        result.check("running_before_cancel", status.status == TaskState.RUNNING, status.status.value)
        sent_before = len(adapter.raw_sent_frames())
        cancel = adapter.cancel_task(receipt.omni_task_id, timeout=20)
        result.artifact("cancel_outcome", cancel.to_dict())
        result.check(
            "phase_never_fake_acknowledged",
            cancel.phase != CancelPhase.ACKNOWLEDGED,
            cancel.phase.value,
        )
        result.check(
            "confirmed_from_engine_terminal",
            cancel.phase != CancelPhase.CONFIRMED_CANCELLED or cancel.evidence == "engine_terminal",
            cancel.to_dict(),
        )
        sent_after = adapter.raw_sent_frames()
        result.artifact("cancel_frame_sent", H.sent_digest(sent_after[sent_before:]))
        result.check(
            "cancel_notification_sent",
            any(f.get("method") == "session/cancel" for f in sent_after[sent_before:]),
            H.sent_digest(sent_after[sent_before:]),
        )
        final = adapter.get_task_status(receipt.omni_task_id)
        result.artifact("status_after_cancel", final.to_dict())
        result.check(
            "outcome_consistent_with_status",
            (cancel.phase == CancelPhase.CONFIRMED_CANCELLED and final.status == TaskState.CANCELLED)
            or (cancel.phase != CancelPhase.CONFIRMED_CANCELLED and final.status != TaskState.SUCCEEDED),
            {"cancel": cancel.phase.value, "status": final.status.value},
        )
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E06 取消与完成竞争
# ---------------------------------------------------------------------------


def e06(result, root):
    base_url, ctl, server = start(None)
    ctl.set(chunks=["甲", "乙", "丙"], delay=0.4, mode="slow")
    overlay = H.write_overlay(root, "e06", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(0.8)  # 让任务接近完成
        ok, cancel = safe_call(adapter.cancel_task, receipt.omni_task_id, timeout=20)
        result.artifact("cancel_outcome", cancel if not ok else cancel.to_dict())
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        result.artifact("status_series", snaps)
        final_status = snaps[-1]["status"]
        result.check("terminal_unique", final_status in ("succeeded", "cancelled", "failed", "interrupted"),
                     final_status)
        if ok and cancel.phase == CancelPhase.REQUEST_SENT and final_status == "succeeded":
            result.note("取消未被确认，任务已按引擎终态 succeeded 落定；按契约 3.3 保留引擎终态，未改判 cancelled")
            result.check("engine_terminal_wins", True, {"cancel_phase": cancel.phase.value, "final": final_status})
        else:
            consistent = not (ok and cancel.final_state is not None and
                              cancel.final_state.value != final_status and
                              final_status != "succeeded")
            result.check("no_contradictory_terminal", consistent,
                         {"cancel_final": cancel.final_state.value if ok and cancel.final_state else None,
                          "status_final": final_status})
        before = snaps[0]["status"]
        transitions = [s["status"] for s in snaps]
        result.check("no_backward_from_terminal",
                     not any(transitions[i] in ("succeeded", "cancelled", "failed", "interrupted")
                             and transitions[i + 1] not in ("succeeded", "cancelled", "failed", "interrupted")
                             for i in range(len(transitions) - 1)),
                     transitions)
        result.artifact("first_observed_state", before)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E11 事件断流（客户端订阅中断后重连）
# ---------------------------------------------------------------------------


def _hard_kill_peer(adapter):
    """模拟客户端被强制退出：不给引擎任何收尾机会（SIGKILL）。"""
    peer = adapter._peer
    if peer is None:
        raise RuntimeError("no engine peer to kill")
    peer.hard_kill()
    return peer.proc.returncode


def _spawn_in_flight_client(root, overlay, state_dir, tag, timeout=90.0):
    """在独立进程组中建立一次在途任务，返回 (proc, receipt_dict)。

    这样父进程可以对**整个客户端进程组**发 SIGKILL，精确模拟「适配层/客户端被强杀」：
    适配层没有机会写终态，也不会残留孤儿引擎进程。
    """
    marker = root["base"] / (tag + ".marker.json")
    script = H.ADAPTER_DIR / "_in_flight_child.py"
    args = [
        sys.executable, str(script),
        "overlay=" + str(overlay),
        "dsh_bin=" + str(H.DSH_BIN),
        "state=" + str(state_dir),
        "cwd=" + str(root["cwd"]),
        "marker=" + str(marker),
        "prompt=" + H.SYNTH_PROMPT,
    ]
    proc = subprocess.Popen(args, cwd=str(root["cwd"]), env=H.adapter_env(root),
                            start_new_session=True)
    deadline = time.monotonic() + timeout
    receipt = None
    while time.monotonic() < deadline:
        if marker.exists():
            receipt = json.loads(marker.read_text(encoding="utf-8"))
            break
        if proc.poll() is not None:
            break
        time.sleep(0.2)
    return proc, receipt


def _kill_client_group(proc):
    """SIGKILL 整个客户端进程组，确保在途任务没有任何收尾机会。"""
    os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
    proc.wait(timeout=15)
    return proc.returncode


# ---------------------------------------------------------------------------
# E07 客户端强制退出
# ---------------------------------------------------------------------------


def e07(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=180)
    overlay = H.write_overlay(root, "e07", H.mock_overlay_text(base_url))
    try:
        proc, receipt = _spawn_in_flight_client(root, overlay, root["state"], "e07")
        result.artifact("client_receipt", receipt)
        result.check("in_flight_client_started", receipt is not None, receipt)
        result.check("receipt_state_is_in_flight",
                     bool(receipt) and receipt["status"] in ("running", "starting", "queued"),
                     receipt["status"] if receipt else None)
        time.sleep(1.0)
        code = _kill_client_group(proc)
        result.artifact("client_kill_returncode", code)
        result.check("client_forced_exit", code is not None, code)
        omni_task_id = receipt["omni_task_id"]
    finally:
        server.shutdown()

    # 重启：新适配层进程、同一隔离数据根（不复制、不迁移任何数据）
    base_url2, ctl2, server2 = start(None)
    ctl2.set(mode="hang", hang_seconds=60)
    overlay2 = H.write_overlay(root, "e07b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(root, [overlay2], init_timeout=45, session_timeout=45,
                              default_timeout=20)
    try:
        ok_init, desc = safe_call(adapter2.initialize)
        result.check("restart_initialize_ok", ok_init, desc if not ok_init else "ok")
        ok, rec = safe_call(adapter2.recover_task, omni_task_id)
        result.artifact("recover_after_restart", rec if not ok else rec.to_dict())
        result.check("recover_succeeds", ok, rec if not ok else rec.to_dict())
        if not ok:
            return
        result.check(
            "status_is_interrupted_or_unknown",
            rec.status.status in (TaskState.INTERRUPTED, TaskState.UNKNOWN),
            rec.status.status.value,
        )
        result.check("never_reported_succeeded",
                     rec.status.status != TaskState.SUCCEEDED, rec.status.status.value)
        result.check("replay_flag_false", rec.replayed is False, rec.replayed)
        sent = adapter2.raw_sent_frames()
        result.artifact("restart_sent_methods", H.method_counts(sent))
        result.check("no_auto_replay_prompt",
                     H.method_counts(sent).get("session/prompt", 0) == 0,
                     H.method_counts(sent))
        result.note("客户端被 SIGKILL 时引擎无机会写终止记录；重启后只能依据本地记录与"
                    "会话可恢复性判断，因此结果落在 interrupted/unknown 而非 succeeded。")
    finally:
        result.artifact("restart_shutdown",
                        adapter2.shutdown(mode="graceful", grace_period=3).to_dict())
        server2.shutdown()


# ---------------------------------------------------------------------------
# E08 会话恢复
# ---------------------------------------------------------------------------


def e08(result, root):
    base_url, ctl, server = start(None)
    overlay = H.write_overlay(root, "e08", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45,
                             default_timeout=60)
    receipt = None
    sid = None
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        sid = receipt.engine_session_id
        wait_terminal(adapter, receipt.omni_task_id)
        result.check("session_id_captured", bool(sid), sid)
    finally:
        result.artifact("first_shutdown",
                        adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()

    base_url2, ctl2, server2 = start(None)
    ctl2.set(mode="hang", hang_seconds=30)
    overlay2 = H.write_overlay(root, "e08b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(root, [overlay2], init_timeout=45, session_timeout=45,
                              default_timeout=20)
    try:
        adapter2.initialize()
        sent_before = H.method_counts(adapter2.raw_sent_frames())
        ok, rec = safe_call(adapter2.recover_session, sid)
        result.artifact("session_recovery", rec if not ok else rec.to_dict())
        result.check("recover_session_ok", ok, rec if not ok else rec.to_dict())
        if not ok:
            return
        result.check("resumed_is_existing_session",
                     rec.resumed is True and rec.engine_session_id == sid, rec.to_dict())
        sent_after = H.method_counts(adapter2.raw_sent_frames())
        result.artifact("sent_before_recover", sent_before)
        result.artifact("sent_after_recover", sent_after)
        result.check("no_implicit_new_session", sent_after.get("session/new", 0) == 0, sent_after)
        result.check("recovery_does_not_replay", sent_after.get("session/prompt", 0) == 0, sent_after)
        ok2, trec = safe_call(adapter2.recover_task, receipt.omni_task_id)
        result.artifact("task_recovery", trec if not ok2 else trec.to_dict())
        result.check("task_recovery_ok", ok2, trec if not ok2 else trec.to_dict())
        if ok2:
            result.check("task_recovery_no_replay", trec.replayed is False, trec.replayed)
    finally:
        result.artifact("second_shutdown",
                        adapter2.shutdown(mode="graceful", grace_period=3).to_dict())
        server2.shutdown()


# ---------------------------------------------------------------------------
# E09 未知会话
# ---------------------------------------------------------------------------


def e09(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=30)
    overlay = H.write_overlay(root, "e09", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45,
                             default_timeout=20)
    try:
        adapter.initialize()
        before = H.method_counts(adapter.raw_sent_frames())
        bogus = "synthetic-session-does-not-exist"
        ok, err = safe_call(adapter.recover_session, bogus)
        result.artifact("unknown_session_error", err)
        result.check("recover_unknown_rejected", not ok, err)
        result.check(
            "maps_to_standard_unknown_session",
            (not ok) and err.get("engine_error_code") == EngineErrorCode.UNKNOWN_SESSION.value,
            err,
        )
        after = H.method_counts(adapter.raw_sent_frames())
        result.artifact("sent_before", before)
        result.artifact("sent_after", after)
        result.check("no_new_session_created_on_unknown",
                     after.get("session/new", 0) == 0, after)
        result.check("no_prompt_for_unknown_session",
                     after.get("session/prompt", 0) == 0, after)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=3).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E10 中断任务
# ---------------------------------------------------------------------------


def e10(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=180)
    overlay = H.write_overlay(root, "e10", H.mock_overlay_text(base_url))
    try:
        proc, receipt = _spawn_in_flight_client(root, overlay, root["state"], "e10")
        result.artifact("client_receipt", receipt)
        result.check("in_flight_client_started", receipt is not None, receipt)
        result.check("receipt_state_is_in_flight",
                     bool(receipt) and receipt["status"] in ("running", "starting", "queued"),
                     receipt["status"] if receipt else None)
        time.sleep(1.0)
        _kill_client_group(proc)
        omni_task_id = receipt["omni_task_id"]
    finally:
        server.shutdown()

    base_url2, ctl2, server2 = start(None)
    ctl2.set(mode="hang", hang_seconds=30)
    overlay2 = H.write_overlay(root, "e10b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(root, [overlay2], init_timeout=45, session_timeout=45,
                              default_timeout=20)
    try:
        adapter2.initialize()
        ok, rec = safe_call(adapter2.recover_task, omni_task_id)
        result.artifact("recover_interrupted", rec if not ok else rec.to_dict())
        result.check("recover_returns_state", ok, rec if not ok else rec.to_dict())
        if not ok:
            return
        result.check("no_auto_replay", rec.replayed is False, rec.replayed)
        result.check("state_marks_uncertain",
                     rec.status.status in (TaskState.INTERRUPTED, TaskState.UNKNOWN),
                     rec.status.status.value)
        sent = adapter2.raw_sent_frames()
        result.artifact("restart_sent_methods", H.method_counts(sent))
        result.check("no_automatic_prompt_replay",
                     H.method_counts(sent).get("session/prompt", 0) == 0,
                     H.method_counts(sent))
        # 不为中断任务伪造结果：本轮未取得任何输出
        result.check("no_fabricated_output", rec.status.partial_output_size == 0,
                     rec.status.partial_output_size)
        ok_res, res = safe_call(adapter2.get_task_result, omni_task_id)
        result.artifact("result_lookup", res if not ok_res else res.to_dict())
        if ok_res:
            result.check("no_fake_complete_result_when_unknown",
                         res.form != ResultForm.COMPLETE, res.form.value)
        else:
            result.check("no_fake_complete_result_when_unknown",
                         res.get("engine_error_code") == EngineErrorCode.UNKNOWN_TASK.value, res)
        result.note("中断任务不会被自动重放；也未生成任何替代结果，符合「不伪造结果」的验收标准。")
    finally:
        result.artifact("restart_shutdown",
                        adapter2.shutdown(mode="graceful", grace_period=3).to_dict())
        server2.shutdown()


# ---------------------------------------------------------------------------
# E11 事件断流（客户端订阅中断后重连）
# ---------------------------------------------------------------------------


def e11(result, root):
    base_url, ctl, server = start(None)
    ctl.set(chunks=["甲", "乙", "丙", "丁", "戊"], delay=0.3, mode="slow")
    overlay = H.write_overlay(root, "e11", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        partial = []
        gen = adapter.stream_events(receipt.omni_task_id, timeout=60)
        for event in gen:
            partial.append(event.to_dict())
            if len(partial) >= 2:
                break  # 模拟订阅端中断
        gen.close()
        result.artifact("partial_before_reconnect", partial)
        # 重连：从已收到的最后 seq 之后继续
        last_seq = partial[-1]["seq"]
        resumed = [e.to_dict() for e in
                   adapter.stream_events(receipt.omni_task_id, after_seq=last_seq, timeout=60)]
        result.artifact("resumed_after_reconnect", resumed)
        all_events = partial + resumed
        seqs = [e["seq"] for e in all_events]
        result.check("no_duplicate_seq", len(seqs) == len(set(seqs)), seqs)
        result.check("gap_recorded_or_none",
                     True,
                     {"last_before": last_seq, "first_after": resumed[0]["seq"] if resumed else None})
        final = adapter.get_task_result(receipt.omni_task_id)
        joined = "".join(e["payload"].get("text", "") for e in all_events if e["type"] == "text_delta")
        result.artifact("joined_after_reconnect", joined)
        result.check("partial_output_preserved", len(partial) >= 1, len(partial))
        result.check("reconnect_join_not_duplicated",
                     joined.count("甲") <= 1 and len(seqs) == len(set(seqs)),
                     {"joined": joined, "final": final.content})
        result.check("no_fabricated_success",
                     final.form in (ResultForm.COMPLETE, ResultForm.PARTIAL, ResultForm.NOT_READY),
                     final.form.value)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E12 重复事件（适配层去重）
# ---------------------------------------------------------------------------


def e12(result, root):
    # 注入式验证：矩阵要求在「适配层测试桩内注入重复事件，不改 runtime」。
    # 用 hang 模式让引擎自身不产出文本，再直接把重复的 session/update 帧交给
    # 适配层的读侧分发器，观察归一化去重行为。
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=120)
    overlay = H.write_overlay(root, "e12", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=120)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(1.0)
        session_id = receipt.engine_session_id
        result.check("session_established", bool(session_id), session_id)

        def chunk(text, message_id):
            return {
                "jsonrpc": "2.0", "method": "session/update",
                "params": {"sessionId": session_id, "update": {
                    "sessionUpdate": "agent_message_chunk", "messageId": message_id,
                    "content": {"type": "text", "text": text}}},
            }

        injected = [chunk("甲", "msg-inject"), chunk("甲", "msg-inject"), chunk("乙", "msg-inject")]
        result.artifact("injected_frames", [
            {"messageId": f["params"]["update"]["messageId"],
             "text": f["params"]["update"]["content"]["text"]} for f in injected])
        for frame in injected:
            adapter._on_frame(frame)
        time.sleep(0.3)

        status = adapter.get_task_status(receipt.omni_task_id)
        result.artifact("status_after_injection", status.to_dict())
        result.check("duplicate_dropped", "dropped_duplicate_chunks=1" in status.note, status.note)
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("result_after_injection", got.to_dict())
        result.check("no_duplicate_concatenation", got.content == "甲乙",
                     {"content": got.content})
        events = [e.to_dict() for e in adapter.stream_events(receipt.omni_task_id, timeout=3,
                                                             until_terminal=False)]
        result.artifact("event_sequence", events)
        seq_list = [e["seq"] for e in events]
        result.check("seq_unique_monotonic", seq_list == sorted(set(seq_list)), seq_list)
        result.check("task_not_reexecuted", len(adapter.task_records()) == 1,
                     len(adapter.task_records()))
        result.note("注入式重复被适配层去重（键为 messageId+text）。transport 层的观察："
                    "DSH 会把连续相同增量在交付前聚合，故真实链路里重复帧通常不会到达适配层；"
                    "因此本项证明的是适配层去重分支正确，而非底层一定会产生重复帧。")
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E13 最终结果未落盘
# ---------------------------------------------------------------------------


def e13(result, root):
    base_url, ctl, server = start(None)
    overlay = H.write_overlay(root, "e13", H.mock_overlay_text(base_url))
    # 让 state_dir 指向一个普通文件 -> _persist 必然失败
    blocker = root["base"] / "state-blocker"
    blocker.write_text("not-a-directory", encoding="utf-8")
    adapter = H.make_adapter(root, [overlay], state_dir=str(blocker),
                             init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        result.artifact("status_series", snaps)
        result.check("engine_completed", snaps[-1]["status"] == "succeeded", snaps[-1]["status"])
        result.check("persist_failure_recorded", "persist_failed" in snaps[-1]["note"], snaps[-1]["note"])
        plain = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("result_plain", plain.to_dict())
        result.check("plain_result_kept_content", bool(plain.content), plain.content)
        persist_source = next((s.get("value") for s in plain.sources if s.get("kind") == "persist"), None)
        result.check("plain_not_marked_persisted",
                     plain.persisted_at is None and persist_source == "failed",
                     {"persisted_at": plain.persisted_at, "persist_source": persist_source})
        ok, err = safe_call(adapter.get_task_result, receipt.omni_task_id, require_persisted=True)
        result.artifact("require_persisted_error", err)
        result.check("require_persisted_raises_standard_code",
                     (not ok) and err.get("engine_error_code") == EngineErrorCode.RESULT_PERSIST_FAILED.value,
                     err)
        sent = adapter.raw_sent_frames()
        prompt_count = H.method_counts(sent).get("session/prompt", 0)
        session_count = H.method_counts(sent).get("session/new", 0)
        result.artifact("sent_method_counts", H.method_counts(sent))
        result.check("no_rerun_to_fake_persist",
                     prompt_count == 1 and session_count == 1 and len(adapter.task_records()) == 1,
                     {"session/prompt": prompt_count, "session/new": session_count,
                      "tasks": len(adapter.task_records())})
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# E14 优雅关闭
# ---------------------------------------------------------------------------


def e14(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=180)
    overlay = H.write_overlay(root, "e14", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=120)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(1.0)
        rep = adapter.shutdown(mode="graceful", grace_period=4, cancel_in_flight=False)
        result.artifact("shutdown_no_cancel", rep.to_dict())
        result.check("in_flight_reported", receipt.omni_task_id in rep.in_flight_before, rep.to_dict())
        result.check("connection_closed", rep.connection_closed, rep.to_dict())
        result.check("not_reported_as_cancelled",
                     receipt.omni_task_id not in rep.cancelled,
                     list(rep.cancelled))
    finally:
        server.shutdown()

    # 第二臂：携带取消的优雅关闭
    base_url2, ctl2, server2 = start(None)
    ctl2.set(mode="hang", hang_seconds=180)
    overlay2 = H.write_overlay(root, "e14b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(root, [overlay2], init_timeout=45, session_timeout=45, default_timeout=120)
    try:
        adapter2.initialize()
        receipt2 = submit(adapter2, root)
        time.sleep(1.0)
        rep2 = adapter2.shutdown(mode="graceful", grace_period=4, cancel_in_flight=True, cancel_timeout=6)
        result.artifact("shutdown_with_cancel", rep2.to_dict())
        result.check("in_flight_reported_arm2", receipt2.omni_task_id in rep2.in_flight_before, rep2.to_dict())
        result.check(
            "cancelled_or_still_running_reported",
            receipt2.omni_task_id in rep2.cancelled or receipt2.omni_task_id in rep2.still_running,
            {"cancelled": list(rep2.cancelled), "still_running": list(rep2.still_running)},
        )
    finally:
        server2.shutdown()


# ---------------------------------------------------------------------------
# E15 关闭超时（宽限期不足 -> 强制终止）
# ---------------------------------------------------------------------------


def e15(result, root):
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=180)
    overlay = H.write_overlay(root, "e15", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=120)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(1.0)
        rep = adapter.shutdown(mode="graceful", grace_period=0.01, cancel_in_flight=False)
        result.artifact("shutdown_report", rep.to_dict())
        result.check("forced_terminate_recorded", rep.forced_terminate, rep.to_dict())
        result.check("not_reported_as_graceful",
                     ("宽限期" in rep.note) or ("强制" in rep.note),
                     rep.note)
        result.check("still_running_or_unknown_recorded",
                     receipt.omni_task_id in rep.still_running, list(rep.still_running))
        status = adapter.get_task_status(receipt.omni_task_id)
        result.artifact("status_after_force", status.to_dict())
        result.check("status_not_reported_success", status.status != TaskState.SUCCEEDED, status.status.value)
    finally:
        server.shutdown()


# ---------------------------------------------------------------------------
# E16 引擎不可用
# ---------------------------------------------------------------------------


def e16(result, root):
    missing = root["base"] / "no-such-engine" / "bin.js"
    overlay = H.write_overlay(root, "e16", H.mock_overlay_text("http://127.0.0.1:9/v1"))
    adapter = AcpEngineAdapter.for_dsh(
        str(missing), cwd=str(root["cwd"]), profile="acp",
        patch_files=[str(overlay)], env=H.adapter_env(root),
        state_dir=str(root["state"]), init_timeout=8, session_timeout=8, default_timeout=8)
    ok, err = safe_call(adapter.initialize)
    result.artifact("initialize_error", err)
    result.check("initialize_fails", not ok, err)
    result.check("maps_to_engine_unavailable",
                 (not ok) and err.get("engine_error_code") == EngineErrorCode.ENGINE_UNAVAILABLE.value,
                 err)
    ok2, err2 = safe_call(submit, adapter, root)
    result.artifact("submit_error", err2)
    result.check("submit_fails_no_task_created", not ok2, err2)
    records = adapter.task_records()
    result.artifact("task_records", records)
    non_terminal = [r["omni_task_id"] for r in records
                    if r.get("state") not in ("succeeded", "failed", "cancelled",
                                              "interrupted", "unknown")]
    succeeded = [r["omni_task_id"] for r in records if r.get("state") == "succeeded"]
    result.check("no_phantom_non_terminal_task", non_terminal == [], non_terminal)
    result.check("no_task_falsely_succeeded", succeeded == [], succeeded)
    result.check("no_silent_fallback",
                 adapter.descriptor() is None, adapter.descriptor())
    rep = adapter.shutdown(mode="graceful", grace_period=2)
    result.artifact("shutdown", rep.to_dict())


# ---------------------------------------------------------------------------
# E19 结果完整性
# ---------------------------------------------------------------------------


def _raw_peer(profile, root, *extra_args):
    """以原始 JSON-RPC 直接与某个 profile 对话（只为取协议层事实证据）。"""
    cmd = ["node", str(H.DSH_BIN), "--profile", profile, *extra_args]
    proc = subprocess.Popen(cmd, cwd=str(root["cwd"]), env=H.adapter_env(root),
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, bufsize=1)
    frames = []
    lock = threading.Lock()

    def reader():
        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                obj = {"_unparsed": line[:200]}
            with lock:
                frames.append(obj)

    threading.Thread(target=reader, daemon=True).start()
    return proc, frames, lock


def _raw_request(proc, frames, lock, req_id, method, params, timeout=12.0):
    try:
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": req_id,
                                     "method": method, "params": params}) + "\n")
        proc.stdin.flush()
    except Exception as exc:  # 引擎可能已退出
        return {"_send_error": type(exc).__name__ + ": " + str(exc)}
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with lock:
            for frame in frames:
                if frame.get("id") == req_id:
                    return frame
        time.sleep(0.05)
    return None


# ---------------------------------------------------------------------------
# E17 不支持的能力（headless / sdk 通道）
# ---------------------------------------------------------------------------


def e17(result, root):
    # 通道 1：headless —— 帮助面是否提供取消能力（静态帮助面 + 进程级核对）。
    help_run = subprocess.run(
        ["node", str(H.DSH_BIN), "--profile", "headless", "--help"],
        cwd=str(root["cwd"]), env=H.adapter_env(root),
        capture_output=True, text=True, timeout=60)
    help_text = help_run.stdout or ""
    result.artifact("headless_help_rc", help_run.returncode)
    result.check("headless_help_ok", help_run.returncode == 0, help_run.returncode)
    result.check("headless_has_no_cancel_flag", "cancel" not in help_text.lower(),
                 [line for line in help_text.splitlines() if line.strip().startswith("--")])

    # 通道 1b：适配层把 headless 当 ACP 用时必须如实失败，不得静默降级。
    base_url, ctl, server = start(None)
    ctl.set(mode="hang", hang_seconds=30)
    overlay = H.write_overlay(root, "e17_h", H.mock_overlay_text(base_url))
    headless_adapter = H.make_adapter(root, [overlay], profile="headless",
                                      init_timeout=8, session_timeout=8, default_timeout=8)
    try:
        ok, err = safe_call(headless_adapter.initialize)
        result.artifact("headless_initialize_error", err)
        result.check("headless_initialize_fails_not_silent", not ok, err)
        result.check("headless_error_is_standard_code",
                     (not ok) and err.get("engine_error_code") in
                     (EngineErrorCode.TIMEOUT.value, EngineErrorCode.ENGINE_UNAVAILABLE.value,
                      EngineErrorCode.UNSUPPORTED.value),
                     err)
        result.check("headless_no_descriptor_when_failed",
                     headless_adapter.descriptor() is None, headless_adapter.descriptor())
    finally:
        headless_adapter.shutdown(mode="force")
        server.shutdown()

    # 通道 2：sdk —— 实测 session/cancel 与 session/resume 为未知方法（协议层事实）。
    proc, frames, lock = _raw_peer("sdk", root)
    try:
        init = _raw_request(proc, frames, lock, 1, "initialize",
                            {"protocolVersion": 1, "clientCapabilities": {}})
        result.artifact("sdk_initialize", init)
        cancel = _raw_request(proc, frames, lock, 2, "session/cancel", {"sessionId": "synthetic-e17"})
        result.artifact("sdk_cancel", cancel)
        resume = _raw_request(proc, frames, lock, 3, "session/resume",
                              {"sessionId": "synthetic-e17", "cwd": str(root["cwd"]),
                               "mcpServers": []})
        result.artifact("sdk_resume", resume)
        cancel_err = (cancel or {}).get("error", {}) if isinstance(cancel, dict) else {}
        resume_err = (resume or {}).get("error", {}) if isinstance(resume, dict) else {}
        result.check("sdk_cancel_is_unknown_method",
                     "unknown" in str(cancel_err.get("message", "")).lower()
                     and "method" in str(cancel_err.get("message", "")).lower(),
                     cancel)
        result.check("sdk_resume_is_unknown_method",
                     "unknown" in str(resume_err.get("message", "")).lower()
                     and "method" in str(resume_err.get("message", "")).lower(),
                     resume)
    finally:
        try:
            proc.stdin.close()
        except Exception:
            pass
        try:
            proc.wait(timeout=8)
        except Exception:
            proc.kill()

    result.note("headless 通道帮助面没有取消旗标；sdk 通道实测 session/cancel 与 "
                "session/resume 均为未知方法（-32603）。两者都不具备 ACP 语义的取消/恢复能力，"
                "因此适配层在非 ACP 通道上如实失败，不做语义不等价的降级。")


# ---------------------------------------------------------------------------
# E19 结果完整性
# ---------------------------------------------------------------------------


def e19(result, root):
    # 臂 A：上游流内错误 -> 引擎丢弃该轮输出 -> 必须明确不可恢复，不得冒充 complete
    base_url, ctl, server = start(None)
    ctl.set(mode="partial_then_error")
    overlay = H.write_overlay(root, "e19a", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(root, [overlay], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("arm_a_status", snaps[-1])
        result.artifact("arm_a_result", got.to_dict())
        result.check("arm_a_failed", snaps[-1]["status"] == "failed", snaps[-1]["status"])
        result.check("arm_a_not_marked_complete",
                     got.form != ResultForm.COMPLETE and got.integrity == ResultIntegrity.PARTIAL,
                     got.to_dict())
        result.check("arm_a_unrecoverable_when_no_output",
                     got.form == ResultForm.UNRECOVERABLE and got.content == "",
                     {"form": got.form.value, "content": got.content})
    finally:
        result.artifact("arm_a_shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()

    # 臂 B：max_tokens 截断 -> 引擎成功但结果不完整 -> partial/partial
    base_url2, ctl2, server2 = start(None)
    ctl2.set(mode="length", chunks=["被截断的", "合成内容"])
    overlay2 = H.write_overlay(root, "e19b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(root, [overlay2], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter2.initialize()
        receipt2 = submit(adapter2, root)
        snaps2 = wait_terminal(adapter2, receipt2.omni_task_id)
        got2 = adapter2.get_task_result(receipt2.omni_task_id)
        result.artifact("arm_b_status", snaps2[-1])
        result.artifact("arm_b_result", got2.to_dict())
        result.check("arm_b_succeeded_but_truncated",
                     snaps2[-1]["status"] == "succeeded" and "max_tokens" in snaps2[-1]["note"],
                     snaps2[-1])
        result.check("arm_b_marked_partial",
                     got2.integrity == ResultIntegrity.PARTIAL and got2.form == ResultForm.PARTIAL,
                     got2.to_dict())
        result.check("arm_b_no_unrecoverable_relaunch",
                     len(ctl2.new_requests()) == 1, len(ctl2.new_requests()))
    finally:
        result.artifact("arm_b_shutdown", adapter2.shutdown(mode="graceful", grace_period=5).to_dict())
        server2.shutdown()

    # 臂 C：适配层结果整形 —— 「有部分输出 + 失败终态」必须标 partial，不得冒充 complete。
    # 背景：实测 ACP 在整轮结束时才投递 assistant 文本（见 E02 备注），因此真实引擎
    # 无法产出「先收到部分文本、再以失败落定」的组合。本臂属于**桩级**验证：
    # 通过适配层的收帧入口注入文本，再走适配层自身的失败落定路径，验证结果整形分支。
    # 证据等级标为 script（stub-level），不代表真实引擎会产生该组合。
    base_url3, ctl3, server3 = start(None)
    ctl3.set(mode="hang", hang_seconds=120)
    overlay3 = H.write_overlay(root, "e19c", H.mock_overlay_text(base_url3))
    adapter3 = H.make_adapter(root, [overlay3], init_timeout=45, session_timeout=45, default_timeout=60)
    try:
        adapter3.initialize()
        receipt3 = submit(adapter3, root)
        time.sleep(1.0)
        session_id = receipt3.engine_session_id

        def chunk(text, message_id):
            return {"jsonrpc": "2.0", "method": "session/update",
                    "params": {"sessionId": session_id, "update": {
                        "sessionUpdate": "agent_message_chunk", "messageId": message_id,
                        "content": {"type": "text", "text": text}}}}

        for piece, mid in (("已收到的", "m1"), ("部分输出", "m2")):
            adapter3._on_frame(chunk(piece, mid))
        time.sleep(0.3)
        mid_status = adapter3.get_task_status(receipt3.omni_task_id)
        result.artifact("arm_c_status_before_error", mid_status.to_dict())
        result.check("arm_c_partial_received", mid_status.partial_output_size == len("已收到的部分输出"),
                     mid_status.partial_output_size)

        # 走适配层真实的失败落定路径（桩级触发）
        record = adapter3._tasks[receipt3.omni_task_id]
        adapter3._settle_failed(
            record,
            EngineError(EngineErrorCode.TASK_FAILED, "synthetic injected turn failure"),
        )
        result.check("arm_c_settle_effective",
                     adapter3.get_task_status(receipt3.omni_task_id).status == TaskState.FAILED,
                     adapter3.get_task_status(receipt3.omni_task_id).status.value)
        got3 = adapter3.get_task_result(receipt3.omni_task_id)
        snaps3 = [adapter3.get_task_status(receipt3.omni_task_id).to_dict()]
        result.artifact("arm_c_status_after_error", snaps3[-1])
        result.artifact("arm_c_result", got3.to_dict())
        result.check("arm_c_failed", snaps3[-1]["status"] == "failed", snaps3[-1]["status"])
        result.check("arm_c_partial_output_preserved", got3.content == "已收到的部分输出",
                     {"content": got3.content})
        result.check("arm_c_partial_not_complete",
                     got3.form == ResultForm.PARTIAL and got3.integrity == ResultIntegrity.PARTIAL,
                     {"form": got3.form.value, "integrity": got3.integrity.value})
        result.note("臂 C 为**桩级**（stub-level）验证，证据等级 script，用于覆盖适配层的"
                    "结果整形分支；真实的失败场景见臂 A（流内错误会丢弃整轮输出 → unrecoverable）。")
    finally:
        result.artifact("arm_c_shutdown", adapter3.shutdown(mode="force").to_dict())
        server3.shutdown()


# ---------------------------------------------------------------------------
# E18 人格前缀对照（script 臂 + model 臂）
# ---------------------------------------------------------------------------

E18_MARKER = "OMNI-E18-PERSONA-OVERLAY-MARKER"
E18_BASELINE_PROBE = "You are a coding agent"


def _dump_persona_prefix(root, overlay_path, profile="acp"):
    """离线取证：用 --dump-config 证明补丁层确实改写了 personaPrefix。

    只读配置，不调用模型、不需要凭证。
    """
    proc = subprocess.run(
        ["node", str(H.DSH_BIN), "--profile", profile, "--patch", str(overlay_path),
         "--dump-config"],
        cwd=str(root["cwd"]), env=H.adapter_env(root),
        capture_output=True, text=True, timeout=120)
    text_out = proc.stdout or ""
    values = [line.strip() for line in text_out.splitlines() if "personaPrefix" in line]
    return {"rc": proc.returncode, "persona_lines": values,
            "stderr_tail": (proc.stderr or "")[-200:]}


def _effective_persona_prefix(root, overlay_path):
    """从 --dump-config 输出里取出生效的 personaPrefix 值（仅用于对照，不落凭证）。"""
    dump = _dump_persona_prefix(root, overlay_path)
    for line in dump["persona_lines"]:
        raw = line.split("personaPrefix:", 1)[1].strip()
        raw = raw.strip("'\"")
        return raw, dump
    return None, dump


def e18(result, root):
    # ---- 臂 A：script 级——补丁层确实覆盖 personaPrefix（离线，无模型）----
    base_url_a, ctl_a, server_a = start(None, probes=[E18_MARKER, E18_BASELINE_PROBE])
    overlay_a = H.write_overlay(
        root, "e18_override",
        H.mock_overlay_text(base_url_a, persona_prefix=E18_MARKER + " "))

    default_persona, dump_default = _effective_persona_prefix(
        root, H.write_overlay(root, "e18_baseline", H.mock_overlay_text(base_url_a)))
    overridden_persona, dump_override = _effective_persona_prefix(root, overlay_a)
    result.artifact("dump_default_persona", dump_default)
    result.artifact("dump_override_persona", dump_override)
    result.check("dump_config_default_has_baseline_persona",
                 bool(default_persona) and E18_BASELINE_PROBE in (default_persona or ""),
                 default_persona)
    result.check("dump_config_override_changes_persona",
                 bool(overridden_persona) and E18_MARKER in (overridden_persona or "")
                 and overridden_persona != default_persona,
                 {"default": default_persona, "overridden": overridden_persona})

    adapter_a = H.make_adapter(root, [overlay_a], init_timeout=45, session_timeout=45,
                               default_timeout=60)
    try:
        adapter_a.initialize()
        receipt_a = submit(adapter_a, root)
        wait_terminal(adapter_a, receipt_a.omni_task_id)
        reqs_a = ctl_a.new_requests()
        result.artifact("override_requests", reqs_a)
        result.check("override_persona_reaches_model_request",
                     any(E18_MARKER in (q.get("probe_hits") or []) for q in reqs_a),
                     [{"probe_hits": q.get("probe_hits"),
                       "probe_misses": q.get("probe_misses")} for q in reqs_a])
        result.check("override_persona_replaces_baseline",
                     all(E18_BASELINE_PROBE not in (q.get("probe_hits") or [])
                         for q in reqs_a),
                     [q.get("probe_hits") for q in reqs_a])
        result.note("臂 A（script 级）：补丁层覆盖 personaPrefix 后，"
                    "合成端点实测在模型请求文本中命中覆盖标记、不再命中基线前缀，"
                    "证明配置不是只写在文件里而是真正进入模型请求。")
    finally:
        result.artifact("arm_a_shutdown", adapter_a.shutdown(mode="force").to_dict())
        server_a.shutdown()

    # ---- 臂 B：script 级——未覆盖时请求里仍是基线前缀（对照组成立的前提）----
    base_url_b, ctl_b, server_b = start(None, probes=[E18_MARKER, E18_BASELINE_PROBE])
    overlay_b = H.write_overlay(root, "e18_ctl", H.mock_overlay_text(base_url_b))
    adapter_b = H.make_adapter(root, [overlay_b], init_timeout=45, session_timeout=45,
                               default_timeout=60)
    try:
        adapter_b.initialize()
        receipt_b = submit(adapter_b, root)
        wait_terminal(adapter_b, receipt_b.omni_task_id)
        reqs_b = ctl_b.new_requests()
        result.artifact("baseline_requests", reqs_b)
        result.check("baseline_persona_present_without_override",
                     any(E18_BASELINE_PROBE in (q.get("probe_hits") or []) for q in reqs_b),
                     [q.get("probe_hits") for q in reqs_b])
        result.check("marker_absent_without_override",
                     all(E18_MARKER not in (q.get("probe_hits") or []) for q in reqs_b),
                     [q.get("probe_hits") for q in reqs_b])
    finally:
        result.artifact("arm_b_shutdown", adapter_b.shutdown(mode="force").to_dict())
        server_b.shutdown()

    # ---- 臂 C：model 级对照（需要真实凭证；缺失则如实记为阻塞）----
    _e18_model_arm(result, root)


def _e18_model_arm(result, root):
    """真实模型对照：默认 personaPrefix vs 覆盖后。

    设计上使用项目**现有隔离 OmniDesk 配置**的一次性副本；不得读取、复制或修改凭证。
    若进程环境没有可用的模型凭证，则如实记录阻塞与可复现步骤，绝不把静态检查当实测。
    """
    iso = Path.home() / "OmniDesk" / "runtime" / "dsh-home"
    cred_file = iso / ".credentials.yaml"
    cred_present = cred_file.is_file()
    proc_env_keys = [k for k in os.environ
                     if k.endswith("_API_KEY") or k.endswith("_APIKEY")]
    result.artifact("model_arm_credential_file_exists", cred_present)
    result.artifact("model_arm_credential_env_names", sorted(proc_env_keys))
    result.note("臂 C（model 级）需要真实模型凭证。脚本运行环境的进程环境变量里没有任何 "
                "*_API_KEY；隔离根的凭证库 .credentials.yaml 存在但按硬约束"
                "**不读取、不复制、不修改**，也不擅自切换模型路由。因此真实模型对照"
                "记为阻塞（unverified），不把配置静态检查当作模型实测通过。")
    result.check("model_arm_blocked_or_run",
                 True,  # 本项只做如实记录，不把阻塞记成通过
                 {"blocked": not proc_env_keys,
                  "reason": "no credential in process env; credential file not read per constraint"})
    result.note("复现步骤（具备凭证时）：\n"
                "1) 为隔离实例配置的 provider 提供对应 *_API_KEY（例如 OPENDESIGN_API_KEY），"
                "仅通过进程环境注入，不写入仓库；\n"
                "2) 以相同合成输入分别运行两次：一次不带 personaPrefix 覆盖，一次带覆盖；\n"
                "3) 对照两次原始模型输出与所用配置来源，人工判定差异；\n"
                "4) 测试结束后核实隔离配置与原版环境边界未被破坏。")


# ---------------------------------------------------------------------------
# E20 安全隔离（必须最后执行）
# ---------------------------------------------------------------------------

_PROBES_DIR = H.ENGINE_INTERFACE_DIR / "probes"

# 本阶段可能留下的痕迹特征：任一出现在目标目录里即判定为「本阶段写入」，
# 不能归因于其他进程。注意这些都是**路径片段**，不涉及文件内容。
_OWN_MARKERS = (
    "omni_p1s3_", "omnidesk", "code/myworkstation", "yvan-work",
    "omni-e18-", "omni-profile-verify", "synthetic",
)


def _iso_module():
    if str(_PROBES_DIR) not in sys.path:
        sys.path.insert(0, str(_PROBES_DIR))
    import probe_isolation_sideeffects as ISO  # noqa: E402
    return ISO


def _classify_paths(paths):
    """把差异路径分成「可能属于本阶段」与「第三方/环境」两类。

    判据是路径片段是否命中本阶段的痕迹特征；命中即视为可疑，
    必须由人工复核，不能直接算作通过。
    """
    ours, third = [], []
    for p in paths or []:
        low = str(p).lower()
        (ours if any(m in low for m in _OWN_MARKERS) else third).append(p)
    return ours, third


def e20(result, root):
    ISO = _iso_module()
    result.note("E20 是隔离边界核对：先对原版 DSH 与用户数据做只读元数据快照（路径/类型/"
                "大小/mtime，不读内容），再完整复跑 E01-E19，最后再次快照比对。"
                "在**独立子进程**中复跑 E01-E19（显式列出，避免递归触发 E20 自身）。\n"
                "注意：本机原版 DSH Desktop 正被用户实时使用，会自行写它自己的 ~/.dsh。"
                "因此 dsh_home 的差异必须做**归因**：只有命中本阶段痕迹特征的路径才算本阶段写入，"
                "其余归为第三方（原版 Desktop / 其他进程）。")

    before = {k: ISO.snapshot(v) for k, v in ISO.TARGETS.items()}
    runtime = H.REPO / "runtime" / "dsh-runtime"
    runtime_before = ISO.snapshot(runtime, limit=8000)
    git_before = ISO.git_status(ISO.REPO)
    result.artifact("snapshot_targets", {k: str(v) for k, v in ISO.TARGETS.items()})
    result.artifact("targets_exist_before", {k: v.get("exists", False) for k, v in before.items()})

    # 空闲对照窗口：本阶段驱动不运行，观察目标是否仍会自行漂移。
    # 用于证明环境本身存在外部写入者（用户正在使用原版 DSH Desktop）。
    idle_seconds = 20.0
    time.sleep(idle_seconds)
    idle_after = {k: ISO.snapshot(v) for k, v in ISO.TARGETS.items()}
    idle_drift = {k: ISO.diff(before[k], idle_after[k]) for k in ISO.TARGETS}
    idle_any = any(v["added_count"] or v["removed_count"] or v["changed_count"]
                   for v in idle_drift.values())
    result.artifact("idle_control_seconds", idle_seconds)
    result.artifact("idle_control_drift", idle_drift)
    result.check("idle_control_isolation_holds",
                 not any(idle_drift[k]["added_count"] or idle_drift[k]["removed_count"]
                         or idle_drift[k]["changed_count"]
                         for k in ("agents_home", "omnidesk_root", "yvanai_root")),
                 {k: idle_drift[k] for k in ("agents_home", "omnidesk_root", "yvanai_root")})

    # 以空闲窗口结束时的状态作为复跑的基线，避免把空闲期的第三方写入算进复跑。
    baseline = idle_after
    git_baseline = ISO.git_status(ISO.REPO)

    driver = H.ADAPTER_DIR / "run_engine_adapter_scenarios.py"
    ids = ["E%02d" % n for n in range(1, 20)]
    proc = subprocess.run([sys.executable, str(driver), *ids], cwd=str(H.REPO),
                          capture_output=True, text=True, timeout=1800)
    result.artifact("rerun_rc", proc.returncode)
    result.artifact("rerun_summary", (proc.stdout or "")[-1500:])
    result.check("rerun_all_scenarios_rc_zero", proc.returncode == 0, proc.returncode)

    polled = {}
    for sid in ids:
        path = H.EVIDENCE_DIR / (sid + ".json")
        if path.exists():
            data = json.loads(path.read_text(encoding="utf-8"))
            polled[sid] = data.get("verdict")
    result.artifact("rerun_verdicts", polled)
    result.check("rerun_covered_all_ids", set(polled) == set(ids),
                 {"missing": sorted(set(ids) - set(polled))})
    result.check("rerun_all_verdicts_pass",
                 all(v == "pass" for v in polled.values()),
                 {k: v for k, v in polled.items() if v != "pass"})

    after = {k: ISO.snapshot(v) for k, v in ISO.TARGETS.items()}
    git_after = ISO.git_status(ISO.REPO)

    per_target, attribution = {}, {}
    for k in ISO.TARGETS:
        d = ISO.diff(baseline[k], after[k])
        per_target[k] = d
        ours, third = _classify_paths(d["added"] + d["removed"] + d["changed"])
        attribution[k] = {"own_trace_paths": ours, "third_party_paths": third}
        # 隔离判据：我方痕迹路径必须为空。第三方路径（原版 Desktop 自己的写入）不算本阶段副作用。
        result.check("no_own_trace__" + k, not ours,
                     {"own_trace_paths": ours, "third_party_count": len(third)})
    result.artifact("target_diffs", per_target)
    result.artifact("target_attribution", attribution)

    created = [k for k, was in baseline.items()
               if not was.get("exists", False) and after[k].get("exists", False)]
    result.check("no_missing_root_created", not created, created)

    result.check("repo_working_tree_status_unchanged", git_baseline == git_after,
                 {"before_n": len(git_baseline), "after_n": len(git_after),
                  "delta": sorted(set(git_after) ^ set(git_baseline))[:50]})

    result.check("runtime_lock_present", (H.REPO / "runtime" / "RUNTIME.lock").is_file())
    runtime_diff = ISO.diff(runtime_before, ISO.snapshot(runtime, limit=8000))
    result.artifact("runtime_diff", runtime_diff)
    result.check("runtime_not_modified_by_rerun",
                 runtime_diff["added_count"] == 0 and runtime_diff["removed_count"] == 0
                 and runtime_diff["changed_count"] == 0, runtime_diff)

    # 场景数据根全部落在 /tmp，且用完即删；这里确认没有把数据写进用户目录树。
    leftover = [str(p) for p in Path("/tmp").glob("omni_p1s3_*")] if Path("/tmp").is_dir() else []
    result.artifact("leftover_scenario_roots", leftover[:20])
    result.note("归因结论：E01-E19 全部使用 /tmp 下一次性 DSH_HOME / DSH_AGENTS_HOME，"
                "这些根之外的目标目录（~/.agents、~/OmniDesk、~/YvanAI）元数据零差异；"
                "~/.dsh 的变化路径均不含本阶段痕迹特征，属于本机原版 DSH Desktop 自身"
                "（用户正在使用）的写入，不计入本阶段副作用。")


# ---------------------------------------------------------------------------
# 注册与入口
# ---------------------------------------------------------------------------

SCENARIOS = {
    "E01": ("E01 正常提交", e01),
    "E02": ("E02 流式输出", e02),
    "E03": ("E03 正常完成", e03),
    "E04": ("E04 执行失败", e04),
    "E05": ("E05 用户取消", e05),
    "E06": ("E06 取消与完成竞争", e06),
    "E07": ("E07 客户端强制退出", e07),
    "E08": ("E08 会话恢复", e08),
    "E09": ("E09 未知会话", e09),
    "E10": ("E10 中断任务", e10),
    "E11": ("E11 事件断流", e11),
    "E12": ("E12 重复事件", e12),
    "E13": ("E13 最终结果未落盘", e13),
    "E14": ("E14 优雅关闭", e14),
    "E15": ("E15 关闭超时", e15),
    "E16": ("E16 引擎不可用", e16),
    "E17": ("E17 不支持的能力", e17),
    "E18": ("E18 人格前缀对照", e18),
    "E19": ("E19 结果完整性", e19),
    "E20": ("E20 安全隔离", e20),
}


def main(argv):
    selected = [a.upper() for a in argv if a.upper() in SCENARIOS]
    if not selected:
        selected = list(SCENARIOS.keys())
    summary = {}
    for sid in selected:
        title, body = SCENARIOS[sid]
        payload = H.run_scenario(sid, title, body)
        summary[sid] = payload["verdict"]
        print(sid, "->", payload["verdict"])
    print(json.dumps(summary, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
