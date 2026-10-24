#!/usr/bin/env python3
"""OmniDesk OD-ARCH-048 —— DSH 本地 Web API 适配通道场景（W01-W12）。

设计：
  * 每个场景一次性 /tmp 数据根；DSH_HOME / DSH_AGENTS_HOME 均指向该根。
  * 本机回环合成端点（_mock_model_server）；零真实凭证、零外网。
  * 证据落 tests/verification/engine-interface/evidence/web-api/。
  * 未取得可靠证据的项一律 unverified，不伪造通过。

用法：
  python3 run_web_api_scenarios.py            # 全部
  python3 run_web_api_scenarios.py W02 W04    # 指定场景

W11/W12（OD-ARCH-079）验证原生会话分支：真实 session/fork、子会话继承上文、
切点不合法时如实失败。
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import _harness as H
from _harness import safe_call
from _mock_model_server import start_mock

from omnidesk_engine import (
    CancelPhase,
    EngineErrorCode,
    ResultForm,
    ResultIntegrity,
    SubmitTaskRequest,
    TaskState,
)

EVIDENCE_DIR = H.ENGINE_INTERFACE_DIR / "evidence" / "web-api"
EVIDENCE_LEVEL = "script（真实 DSH web 进程 + 合成模型端点）"
PERSONA_MARKER = "OMNI_W08_PERSONA_MARKER_SYNTHETIC"


def start(settings=None, probes=None):
    base_url, ctl, server = start_mock()
    if settings:
        ctl.set(**settings)
    if probes:
        ctl.set(probes=list(probes))
    return base_url, ctl, server


def wait_terminal(adapter, task_id, timeout=60.0):
    snaps = []
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status = adapter.get_task_status(task_id)
        snaps.append(status.to_dict())
        if status.status in (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED,
                             TaskState.INTERRUPTED, TaskState.UNKNOWN):
            return snaps
        time.sleep(0.1)
    return snaps


def submit(adapter, root, prompt=None, intent="web-synthetic-task", **kwargs):
    return adapter.submit_task(SubmitTaskRequest(
        intent=intent, workspace_dir=str(root["cwd"]),
        prompt_text=prompt or H.SYNTH_PROMPT, **kwargs))


def make(root, base_url, tag, *, persona_prefix=None, **kwargs):
    overlay = H.write_overlay(
        root, tag, H.web_overlay_text(base_url, persona_prefix=persona_prefix))
    kwargs.setdefault("init_timeout", 60)
    kwargs.setdefault("session_timeout", 45)
    kwargs.setdefault("default_timeout", 90)
    return H.make_web_adapter(root, [overlay], **kwargs)


# ---------------------------------------------------------------------------
# W01 启动与鉴权
# ---------------------------------------------------------------------------


def w01(result, root):
    base_url, ctl, server = start()
    adapter = make(root, base_url, "w01")
    try:
        ok, desc = safe_call(adapter.initialize)
        result.check("initialize_ok", ok, desc if not ok else desc.to_dict())
        if ok:
            result.artifact("descriptor", desc.to_dict())
            result.check("engine_kind_is_web_api", desc.engine_kind == "dsh-web-api", desc.engine_kind)
            result.check("profile_recorded", desc.detail.get("profile") == "web", desc.detail.get("profile"))
        catalog = adapter.model_catalog()
        result.artifact("catalog_summary", {
            "groups": [g["id"] for g in catalog["groups"]],
            "models": [m["id"] for m in catalog["models"]],
            "failures": catalog["failures"],
        })
        result.check("catalog_readable", isinstance(catalog.get("groups"), list) and bool(catalog["models"]),
                     [m["id"] for m in catalog.get("models", [])])
        # token 生命周期：交换 cookie 后即丢弃，不进入诊断文本。
        result.check("token_discarded_after_auth", adapter._token is None, adapter._token is None)
        startup = adapter.engine_stderr()
        result.artifact("startup_tail_redacted", startup[-400:])
        leaks = []
        if "token=" in startup:
            leaks = [seg[:20] for seg in startup.split("token=")[1:] if not seg.startswith("<redacted>")]
        result.check("startup_token_redacted", not leaks, leaks)
        blob = json.dumps({"desc": desc.to_dict() if ok else None, "catalog": catalog}, ensure_ascii=False)
        result.check("no_token_in_public_objects", "token=" not in blob and "cookie" not in blob.lower(),
                     "checked descriptor + catalog")
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W02 真实分批流式
# ---------------------------------------------------------------------------


def w02(result, root):
    base_url, ctl, server = start({"chunks": ["TOK-A ", "TOK-B ", "TOK-C"], "delay": 0.25,
                                   "mode": "slow"})
    adapter = make(root, base_url, "w02")
    try:
        adapter.initialize()
        t0 = time.monotonic()
        receipt = submit(adapter, root, prompt="SYNTHETIC-STREAM-PROMPT")
        stamps = []
        events = []
        for event in adapter.stream_events(receipt.omni_task_id, timeout=120):
            payload = event.to_dict()
            events.append(payload)
            if event.type.value == "text_delta":
                stamps.append({"text": event.payload.get("text"),
                               "streamed": event.payload.get("streamed"),
                               # occurred_at 是 epoch 秒，与 monotonic 不可混算；
                               # 到达时间用消费时刻单独记录，避免伪造时间差。
                               "at_s": round(time.monotonic() - t0, 3)})
        result.artifact("delta_arrivals", stamps)
        result.artifact("event_types", [e["type"] for e in events])
        result.check("three_or_more_deltas", len(stamps) >= 3, len(stamps))
        spread = (stamps[-1]["at_s"] - stamps[0]["at_s"]) if len(stamps) >= 2 else 0.0
        result.check("deltas_arrive_over_time", spread >= 0.3, spread)
        result.check("all_deltas_marked_streamed", all(s["streamed"] is True for s in stamps), stamps)
        seqs = [e["seq"] for e in events]
        result.check("seq_strictly_increasing", seqs == sorted(seqs) and len(set(seqs)) == len(seqs), seqs)
        final = adapter.get_task_result(receipt.omni_task_id)
        joined = "".join(s["text"] for s in stamps)
        result.artifact("final_result", final.to_dict())
        result.check("stream_join_eq_result", joined == final.content, {"join": joined, "final": final.content})
        result.check("terminal_succeeded", adapter.get_task_status(receipt.omni_task_id).status == TaskState.SUCCEEDED,
                     adapter.get_task_status(receipt.omni_task_id).to_dict())
        result.artifact("model_request_count", len(ctl.new_requests()))
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W03 执行失败
# ---------------------------------------------------------------------------


def w03(result, root):
    base_url, ctl, server = start({"mode": "error_400"})
    adapter = make(root, base_url, "w03")
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        result.artifact("status_series", snaps)
        result.check("reached_failed", snaps[-1]["status"] == "failed", snaps[-1])
        result.check("terminal_evidence_is_engine", snaps[-1]["evidence_source"] == "engine_terminal",
                     snaps[-1]["evidence_source"])
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("task_result", got.to_dict())
        result.check("no_fake_content", got.content == "", got.content)
        result.check("not_complete_form", got.form != ResultForm.COMPLETE, got.form.value)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W04 正常取消
# ---------------------------------------------------------------------------


def w04(result, root):
    base_url, ctl, server = start({"mode": "hang", "hang_seconds": 180})
    adapter = make(root, base_url, "w04", default_timeout=120)
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(1.0)
        before = adapter.get_task_status(receipt.omni_task_id).to_dict()
        result.artifact("status_before_cancel", before)
        t0 = time.monotonic()
        cancel = adapter.cancel_task(receipt.omni_task_id, timeout=20)
        elapsed = round(time.monotonic() - t0, 3)
        result.artifact("cancel_outcome", cancel.to_dict())
        result.artifact("cancel_elapsed_s", elapsed)
        result.check("was_running", before["status"] in ("running", "starting"), before["status"])
        result.check("confirmed_cancelled", cancel.phase == CancelPhase.CONFIRMED_CANCELLED, cancel.phase.value)
        result.check("evidence_is_engine_terminal", cancel.evidence == "engine_terminal", cancel.evidence)
        result.check("settled_within_2s", elapsed <= 2.0, elapsed)
        after = adapter.get_task_status(receipt.omni_task_id).to_dict()
        result.artifact("status_after_cancel", after)
        result.check("final_state_cancelled", after["status"] == "cancelled", after["status"])
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W05 取消与完成竞争
# ---------------------------------------------------------------------------


def w05(result, root):
    base_url, ctl, server = start({"chunks": ["甲", "乙", "丙"], "delay": 0.4, "mode": "slow"})
    adapter = make(root, base_url, "w05")
    try:
        adapter.initialize()
        receipt = submit(adapter, root)
        time.sleep(0.8)
        ok, cancel = safe_call(adapter.cancel_task, receipt.omni_task_id, timeout=20)
        result.artifact("cancel_outcome", cancel if not ok else cancel.to_dict())
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        result.artifact("status_series", snaps)
        final = snaps[-1]["status"]
        result.check("terminal_unique", final in ("succeeded", "cancelled", "failed", "interrupted"), final)
        if ok and cancel.phase == CancelPhase.CONFIRMED_CANCELLED:
            result.check("cancel_matches_final", final == "cancelled", {"phase": cancel.phase.value, "final": final})
        elif ok and final == "succeeded":
            result.note("取消请求在任务完成附近发出，引擎以完成落定；按契约保留引擎终态，不改判取消")
            result.check("engine_terminal_wins", True, {"phase": cancel.phase.value, "final": final})
        else:
            result.check("no_contradictory_terminal", not (ok and cancel.final_state is not None
                                                            and cancel.final_state.value != final), final)
        transitions = [s["status"] for s in snaps]
        terminal = ("succeeded", "cancelled", "failed", "interrupted")
        result.check("no_backward_from_terminal",
                     not any(transitions[i] in terminal and transitions[i + 1] not in terminal
                             for i in range(len(transitions) - 1)), transitions)
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="force").to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W06 同一会话连续两轮
# ---------------------------------------------------------------------------


def w06(result, root):
    base_url, ctl, server = start({"chunks": ["FIRST"], "mode": "ok"})
    adapter = make(root, base_url, "w06")
    try:
        adapter.initialize()
        first = submit(adapter, root, prompt="SYNTHETIC-TURN-1")
        wait_terminal(adapter, first.omni_task_id)
        ctl.set(chunks=["SECOND"], delay=0.0, mode="ok")
        second = submit(adapter, root, prompt="SYNTHETIC-TURN-2",
                        engine_session_id=first.engine_session_id)
        result.artifact("second_receipt", second.to_dict())
        result.check("session_mode_continued", second.session_mode == "continued", second.session_mode)
        result.check("same_session_id", second.engine_session_id == first.engine_session_id,
                     {"first": first.engine_session_id, "second": second.engine_session_id})
        wait_terminal(adapter, second.omni_task_id)
        r1 = adapter.get_task_result(first.omni_task_id)
        r2 = adapter.get_task_result(second.omni_task_id)
        result.artifact("results", {"first": r1.content, "second": r2.content})
        result.check("second_turn_has_its_own_text", r2.content == "SECOND", r2.content)
        result.check("first_result_unchanged", r1.content == "FIRST", r1.content)
        result.artifact("model_request_count", len(ctl.new_requests()))
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W07 重启后恢复会话并继续
# ---------------------------------------------------------------------------


def w07(result, root):
    base_url, ctl, server = start({"chunks": ["BEFORE-RESTART"], "mode": "ok"})
    adapter = make(root, base_url, "w07a")
    session_id = None
    try:
        adapter.initialize()
        first = submit(adapter, root, prompt="SYNTHETIC-BEFORE-RESTART")
        wait_terminal(adapter, first.omni_task_id)
        session_id = first.engine_session_id
        result.check("session_id_present", bool(session_id), session_id)
    finally:
        result.artifact("first_shutdown", adapter.shutdown(mode="graceful", grace_period=8).to_dict())

    ctl.set(chunks=["AFTER-RESTART"], delay=0.0, mode="ok")
    adapter2 = make(root, base_url, "w07b")
    try:
        adapter2.initialize()
        listing = adapter2.list_sessions(cwd=str(root["cwd"]))
        result.artifact("session_listing", {
            "total": listing.total,
            "groups": [{"sessions": [s["engine_session_id"] for s in g["sessions"]]}
                       for g in listing.groups],
            "note": listing.note,
        })
        result.check("restarted_engine_lists_session",
                     any(s["engine_session_id"] == session_id
                         for g in listing.groups for s in g["sessions"]), listing.total)
        ok, rec = safe_call(adapter2.recover_session, session_id)
        result.artifact("recover", rec if not ok else rec.to_dict())
        result.check("recover_ok", ok, rec if not ok else rec.to_dict())
        resumed = submit(adapter2, root, prompt="SYNTHETIC-AFTER-RESTART",
                         engine_session_id=session_id)
        result.artifact("resumed_receipt", resumed.to_dict())
        result.check("session_mode_resumed", resumed.session_mode == "resumed", resumed.session_mode)
        result.check("same_session_after_restart", resumed.engine_session_id == session_id,
                     {"before": session_id, "after": resumed.engine_session_id})
        wait_terminal(adapter2, resumed.omni_task_id)
        got = adapter2.get_task_result(resumed.omni_task_id)
        result.artifact("resumed_result", got.to_dict())
        result.check("resumed_turn_produced_new_text", got.content == "AFTER-RESTART", got.content)
    finally:
        result.artifact("second_shutdown", adapter2.shutdown(mode="graceful", grace_period=8).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W08 system/message 不进结果
# ---------------------------------------------------------------------------


def w08(result, root):
    base_url, ctl, server = start({"chunks": ["PUBLIC-ANSWER"], "mode": "ok"},
                                  probes=[PERSONA_MARKER])
    adapter = make(root, base_url, "w08", persona_prefix=PERSONA_MARKER + " ")
    from omnidesk_engine import web_api_adapter as W
    raw_types = []
    original = W.WsClient.recv

    def recording_recv(self, timeout=None):
        message = original(self, timeout=timeout)
        if isinstance(message, dict):
            value = message.get("value")
            kind = value.get("type") if isinstance(value, dict) else message.get("type")
            event = value.get("event") if isinstance(value, dict) else None
            raw_types.append({"kind": kind,
                              "event": event.get("type") if isinstance(event, dict) else None})
        return message

    W.WsClient.recv = recording_recv
    try:
        adapter.initialize()
        receipt = submit(adapter, root, prompt="SYNTHETIC-W08")
        events = [e.to_dict() for e in adapter.stream_events(receipt.omni_task_id, timeout=120)]
        got = adapter.get_task_result(receipt.omni_task_id)
        result.artifact("observed_frame_kinds", raw_types)
        result.artifact("task_result", got.to_dict())
        kinds = {t["kind"] for t in raw_types}
        result.check("system_message_event_observed",
                     any(t.get("event") == "system/message" for t in raw_types)
                     or "system/message" in kinds,
                     sorted(k for k in kinds if k))
        result.check("result_excludes_persona_marker", PERSONA_MARKER not in got.content,
                     got.content[:120])
        result.check("events_exclude_persona_marker",
                     PERSONA_MARKER not in json.dumps(events, ensure_ascii=False),
                     "scanned stream_events payload")
        result.check("answer_still_delivered", got.content == "PUBLIC-ANSWER", got.content)
        reqs = ctl.new_requests()
        result.artifact("model_request_probe", [
            {"messages_hits": q.get("probe_hits"), "body_hits": q.get("body_probe_hits"),
             "body_keys": q.get("body_keys")} for q in reqs])
        in_body = any(PERSONA_MARKER in (q.get("body_probe_hits") or []) for q in reqs)
        in_messages = any(PERSONA_MARKER in (q.get("probe_hits") or []) for q in reqs)
        result.check("persona_marker_not_in_result_or_events",
                     PERSONA_MARKER not in got.content, got.content[:80])
        if in_body and not in_messages:
            result.note("personaPrefix 出现在模型请求体的非 messages 字段中；说明配置生效但注入通道与 ACP 不同。")
        elif not in_body:
            result.note("personaPrefix 未出现在模型请求体中（整包探针也未命中）：该覆盖在 web 通道对模型请求"
                        "没有可见效果。dump-config 生效只能证明配置被接受，不能证明模型收到。")
        else:
            result.note("personaPrefix 出现在模型请求的消息内容中。")
    finally:
        W.WsClient.recv = original
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W09 未知会话
# ---------------------------------------------------------------------------


def w09(result, root):
    base_url, ctl, server = start({"mode": "hang", "hang_seconds": 30})
    adapter = make(root, base_url, "w09", session_timeout=8)
    try:
        adapter.initialize()
        before = len(adapter.task_records())
        t0 = time.monotonic()
        ok, err = safe_call(adapter.recover_session, "synthetic-session-does-not-exist")
        elapsed = round(time.monotonic() - t0, 3)
        result.artifact("unknown_session_error", err)
        result.artifact("elapsed_s", elapsed)
        result.check("recover_unknown_rejected", not ok, err)
        result.check("maps_to_unknown_session", (not ok) and
                     err.get("engine_error_code") == EngineErrorCode.UNKNOWN_SESSION.value, err)
        result.check("fails_fast_before_timeout", elapsed < 6.0, {"elapsed_s": elapsed, "timeout": 8})
        after = len(adapter.task_records())
        result.check("no_task_or_session_created", after == before, {"before": before, "after": after})
        result.artifact("model_request_count", len(ctl.new_requests()))
        result.check("no_model_call", len(ctl.new_requests()) == 0, len(ctl.new_requests()))
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=3).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W10 模型目录归一化
# ---------------------------------------------------------------------------


def w10(result, root):
    base_url, ctl, server = start()
    adapter = make(root, base_url, "w10")
    try:
        adapter.initialize()
        catalog = adapter.model_catalog(refresh=True)
        result.artifact("catalog", catalog)
        models = catalog["models"]
        result.check("models_non_empty", bool(models), len(models))
        result.check("synthetic_model_present",
                     any(m["id"] == "synthetic-model" and m["provider"] == "fake" for m in models),
                     [{"id": m["id"], "provider": m["provider"]} for m in models])
        result.check("every_model_has_provider",
                     all(m["provider"] and m["id"] for m in models),
                     [m for m in models if not (m["provider"] and m["id"])])
        result.check("groups_mirror_models",
                     sum(len(g["models"]) for g in catalog["groups"]) == len(models),
                     {"groups": len(catalog["groups"]), "models": len(models)})
        result.check("no_credentials_in_catalog",
                     not any(k in json.dumps(catalog).lower()
                             for k in ("apikey", "api_key", "authorization", "cookie")),
                     "scanned normalized catalog")
        result.artifact("routable_providers", catalog["routable_providers"])
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W11 原生会话分支（session/fork）并续接子会话
# ---------------------------------------------------------------------------


def w11(result, root):
    """证明「分支到新对话」用的是引擎原生 session/fork，而不是本地重放。

    判定依据全部来自引擎与合成端点的可观测事实：
      * 父会话本轮 turn/end 的 seq 被真实记录（切点存在）；
      * session/fork 返回新的子会话号；
      * 往子会话续接一轮时，模型请求里**带着切点前的上文**（text_chars 大于
        单轮提示词长度、message_count > 2），说明上下文由引擎继承；
      * 父会话结果不被改写。
    """
    marker_parent = "OMNI-W11-PARENT-MARKER"
    marker_child = "OMNI-W11-CHILD-MARKER"
    base_url, ctl, server = start({"chunks": [marker_parent], "mode": "ok"},
                                  probes=[marker_parent, marker_child])
    adapter = make(root, base_url, "w11")
    try:
        adapter.initialize()
        first = submit(adapter, root, prompt="SYNTHETIC-W11-TURN-1 " + marker_parent)
        wait_terminal(adapter, first.omni_task_id)
        status = adapter.get_task_status(first.omni_task_id)
        cut = status.engine_turn_end_seq
        result.artifact("parent", {
            "engine_session_id": first.engine_session_id,
            "turn_end_seq": cut,
            "status": status.to_dict(),
        })
        result.check("turn_end_seq_recorded", isinstance(cut, int) and cut >= 0, cut)
        if not isinstance(cut, int):
            result.check("fork_created_child", False, "没有切点序号，无法继续验证")
            return

        ok, fork = safe_call(adapter.fork_session, first.engine_session_id, at_seq=cut)
        result.artifact("fork", fork if not ok else fork.to_dict())
        result.check("fork_ok", ok, fork if not ok else fork.to_dict())
        if not ok:
            return
        result.check("child_session_is_new",
                     fork.engine_session_id != first.engine_session_id,
                     {"parent": first.engine_session_id, "child": fork.engine_session_id})
        result.check("fork_cut_matches", fork.at_seq == cut, {"ask": cut, "got": fork.at_seq})

        ctl.set(chunks=[marker_child], delay=0.0, mode="ok")
        ctl.reset_requests()
        child = submit(adapter, root, prompt="SYNTHETIC-W11-CHILD-TURN " + marker_child,
                       engine_session_id=fork.engine_session_id)
        result.artifact("child_receipt", child.to_dict())
        result.check("child_submit_continued_same_session",
                     child.engine_session_id == fork.engine_session_id,
                     {"want": fork.engine_session_id, "got": child.engine_session_id})
        result.check("child_not_degraded", child.session_mode != "degraded", child.session_mode)
        wait_terminal(adapter, child.omni_task_id)
        child_result = adapter.get_task_result(child.omni_task_id)
        result.check("child_turn_succeeded", child_result.content == marker_child,
                     child_result.content[:80])

        reqs = ctl.new_requests()
        result.artifact("child_model_requests", [
            {"message_count": q.get("message_count"), "text_chars": q.get("text_chars"),
             "probe_hits": q.get("probe_hits")} for q in reqs])
        inherited = any((q.get("message_count") or 0) > 2 for q in reqs)
        result.check("child_request_carries_inherited_context", inherited,
                     [q.get("message_count") for q in reqs])
        parent_hits = [q for q in reqs if marker_parent in (q.get("probe_hits") or [])]
        result.check("parent_turn_text_present_in_child_request", bool(parent_hits),
                     [q.get("probe_hits") for q in reqs])

        parent_after = adapter.get_task_result(first.omni_task_id)
        result.check("parent_result_unchanged", parent_after.content == marker_parent,
                     parent_after.content[:80])
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


# ---------------------------------------------------------------------------
# W12 分支切点不合法
# ---------------------------------------------------------------------------


def w12(result, root):
    """切点不在源会话里时，必须如实失败（invalid_argument），不假造子会话。"""
    base_url, ctl, server = start({"chunks": ["SYNTH-W12"], "mode": "ok"})
    adapter = make(root, base_url, "w12")
    try:
        adapter.initialize()
        first = submit(adapter, root, prompt="SYNTHETIC-W12")
        wait_terminal(adapter, first.omni_task_id)
        before_records = len(adapter.task_records())
        ok, err = safe_call(adapter.fork_session, first.engine_session_id, at_seq=999_999)
        result.artifact("bad_cut_error", None if ok else err)
        result.check("bad_cut_rejected", not ok)
        if not ok:
            result.check("bad_cut_is_argument_error",
                         err.get("engine_error_code") == EngineErrorCode.INVALID_ARGUMENT.value,
                         err.get("engine_error_code"))
        ok2, err2 = safe_call(adapter.fork_session, "session-does-not-exist", at_seq=0)
        result.artifact("unknown_session_error", None if ok2 else err2)
        result.check("unknown_session_rejected", not ok2)
        if not ok2:
            result.check("unknown_session_mapped",
                         err2.get("engine_error_code") == EngineErrorCode.UNKNOWN_SESSION.value,
                         err2.get("engine_error_code"))
        result.check("no_task_created_by_failed_fork",
                     len(adapter.task_records()) == before_records,
                     {"before": before_records, "after": len(adapter.task_records())})
    finally:
        result.artifact("shutdown", adapter.shutdown(mode="graceful", grace_period=5).to_dict())
        server.shutdown()


SCENARIOS = {
    "W01": ("W01 启动与鉴权", w01),
    "W02": ("W02 真实分批流式", w02),
    "W03": ("W03 执行失败", w03),
    "W04": ("W04 正常取消", w04),
    "W05": ("W05 取消与完成竞争", w05),
    "W06": ("W06 同会话连续两轮", w06),
    "W07": ("W07 重启后恢复会话", w07),
    "W08": ("W08 system/message 不进结果", w08),
    "W09": ("W09 未知会话", w09),
    "W10": ("W10 模型目录归一化", w10),
    "W11": ("W11 原生会话分支并续接子会话", w11),
    "W12": ("W12 分支切点不合法时如实失败", w12),
}


def main(argv):
    selected = [a.upper() for a in argv if a.upper() in SCENARIOS]
    if not selected:
        selected = list(SCENARIOS.keys())
    summary = {}
    for sid in selected:
        title, body = SCENARIOS[sid]
        payload = H.run_scenario(sid, title, body, channel="web-api", evidence_dir=EVIDENCE_DIR)
        summary[sid] = payload["verdict"]
        print(sid, "->", payload["verdict"], flush=True)
    print(json.dumps(summary, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
