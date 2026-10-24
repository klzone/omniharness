#!/usr/bin/env python3
"""OD-ARCH-048 —— Web API 适配层纯单元测试（不启动引擎、不联网）。

覆盖：WS 错误帧两种形态、assistant-stream 帧翻译、命令参数顺序、
诊断文本脱敏、turn/end 各终态映射。

证据等级：static（纯 Python 单元测试）。
运行：python3 -m unittest -v test_web_api_unit
"""

from __future__ import annotations

import time
import unittest
import socket
import threading

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine import TaskState
from omnidesk_engine.contract import EventType
from omnidesk_engine.task_core import _TaskRecord
from omnidesk_engine.web_api_adapter import WebApiAdapter


def make_adapter(tmp_state="/tmp/od048-unit-state", **kwargs):
    """构造但不启动：__init__ 不 spawn 进程。"""
    return WebApiAdapter(command=["node", "/nonexistent/bin.js"], cwd="/tmp",
                         state_dir=tmp_state, **kwargs)


def make_record(state=TaskState.RUNNING):
    return _TaskRecord(omni_task_id="omni-unit", intent="unit", workspace_dir="/tmp",
                       idempotency_key=None, created_at=time.time(), state_since=time.time(),
                       state=state, evidence_source="process_state")


class SessionForkTests(unittest.TestCase):
    """OD-ARCH-079：原生会话分支只认 turn/end 的真实序号，失败如实报错。

    不启动引擎、不发网络请求：`_rpc` 用替身，只核对发出去的请求与返回值处理。
    """

    def setUp(self):
        self.adapter = make_adapter(tmp_state="/tmp/od079-unit-state")
        self.record = make_record()
        self.record.engine_session_id = "sess-parent"
        self.calls = []

    def patch_rpc(self, result):
        self.adapter._rpc = lambda method, args, timeout, **kw: (
            self.calls.append((method, dict(args))) or result
        )

    def test_turn_end_seq_records_non_negative_int(self):
        self.adapter._record_turn_end_seq(self.record, {"type": "turn/end", "seq": 42})
        self.assertEqual(self.record.turn_end_seq, 42)

    def test_turn_end_seq_ignores_missing_or_invalid(self):
        for bad in (None, "42", 4.5, True, -1, {"n": 1}):
            with self.subTest(bad=bad):
                record = make_record()
                self.adapter._record_turn_end_seq(record, {"seq": bad})
                self.assertIsNone(record.turn_end_seq)
        # 已有合法值时，后续异常帧不得把它改坏。
        self.adapter._record_turn_end_seq(self.record, {"seq": 7})
        self.adapter._record_turn_end_seq(self.record, {"seq": "x"})
        self.assertEqual(self.record.turn_end_seq, 7)

    def test_turn_end_seq_reaches_task_status(self):
        self.adapter._tasks[self.record.omni_task_id] = self.record
        self.adapter._record_turn_end_seq(self.record, {"seq": 9})
        status = self.adapter.get_task_status(self.record.omni_task_id)
        self.assertEqual(status.engine_turn_end_seq, 9)
        self.assertEqual(status.to_dict()["engine_turn_end_seq"], 9)

    def test_fork_session_sends_native_request(self):
        self.patch_rpc({"sessionId": "sess-child"})
        fork = self.adapter.fork_session("sess-parent", at_seq=12)
        self.assertEqual(
            self.calls[-1],
            ("session/fork", {"request": {"sessionId": "sess-parent", "atSeq": 12}}),
        )
        self.assertEqual(fork.parent_session_id, "sess-parent")
        self.assertEqual(fork.engine_session_id, "sess-child")
        self.assertEqual(fork.at_seq, 12)

    def test_fork_session_omits_at_seq_when_unknown(self):
        self.patch_rpc({"sessionId": "sess-child"})
        self.adapter.fork_session("sess-parent")
        self.assertEqual(self.calls[-1][1], {"request": {"sessionId": "sess-parent"}})

    def test_fork_session_carries_cwd_to_child(self):
        self.adapter._session_cwd["sess-parent"] = "/tmp/ws"
        self.patch_rpc({"sessionId": "sess-child"})
        self.adapter.fork_session("sess-parent", at_seq=3)
        self.assertEqual(self.adapter._session_cwd.get("sess-child"), "/tmp/ws")

    def test_fork_session_rejects_bad_arguments(self):
        self.patch_rpc({"sessionId": "sess-child"})
        for at_seq in (-1, True, "3", 1.5):
            with self.subTest(at_seq=at_seq):
                with self.assertRaises(Exception):
                    self.adapter.fork_session("sess-parent", at_seq=at_seq)
        with self.assertRaises(Exception):
            self.adapter.fork_session("")
        self.assertEqual(self.calls, [], "参数不合法时不得发出 session/fork")

    def test_fork_session_rejects_unusable_reply(self):
        for reply in ({}, {"sessionId": ""}, ["sess-child"], "ok"):
            with self.subTest(reply=reply):
                self.patch_rpc(reply)
                with self.assertRaises(Exception):
                    self.adapter.fork_session("sess-parent", at_seq=1)


class ForkServiceRoutingTests(unittest.TestCase):
    """serve 层：ACP 通道没有 fork 时必须如实回 unsupported，不得退回本地重放。"""

    def service(self):
        import io

        from omnidesk_engine.serve import EngineService

        return EngineService(stdin=io.StringIO(), stdout=io.StringIO())

    def test_acp_channel_reports_unsupported(self):
        svc = self.service()
        svc._adapter = object()  # ACP 适配器没有 fork_session
        svc._channel = "acp"
        svc.m_fork_session("req-1", {"engine_session_id": "sess-1", "at_seq": 2})
        out = svc._stdout.getvalue()
        self.assertIn("unsupported", out)
        self.assertNotIn("result", out)

    def test_web_channel_delegates_at_seq(self):
        svc = self.service()
        seen = {}

        class Adapter:
            def fork_session(self, session_id, at_seq=None):
                seen["args"] = (session_id, at_seq)

                class Fork:
                    def to_dict(self):
                        return {"engine_session_id": "sess-child", "at_seq": at_seq}

                return Fork()

        svc._adapter = Adapter()
        svc._channel = "web-api"
        svc.m_fork_session("req-2", {"engine_session_id": "sess-1", "at_seq": "5"})
        self.assertEqual(seen["args"], ("sess-1", 5))
        self.assertIn("sess-child", svc._stdout.getvalue())


class FrameErrorTests(unittest.TestCase):
    def test_top_level_error(self):
        # 实测形态：{"type":"error","streamId":...,"error":{"code":"session/not-found"}}
        msg = {"type": "error", "streamId": "s1",
               "error": {"code": "session/not-found", "message": "gone"}}
        self.assertEqual(WebApiAdapter._frame_error(msg),
                         {"code": "session/not-found", "message": "gone"})

    def test_nested_value_error(self):
        msg = {"value": {"type": "error", "error": {"code": "unsupported", "message": "no"}}}
        self.assertEqual(WebApiAdapter._frame_error(msg)["code"], "unsupported")

    def test_error_without_payload_still_detected(self):
        self.assertIn("message", WebApiAdapter._frame_error({"type": "error"}))

    def test_normal_frame_has_no_error(self):
        for msg in ({}, {"type": "ack"}, {"value": {"type": "snapshot"}},
                    {"value": {"type": "event", "event": {"type": "turn/end"}}}):
            self.assertIsNone(WebApiAdapter._frame_error(msg), msg)

    def test_not_found_maps_to_unknown_session_and_unsupported(self):
        adapter = make_adapter()
        with self.assertRaises(Exception) as ctx:
            adapter._raise_follow_error({"error": {"code": "session/not-found"}}, "s1")
        self.assertEqual(ctx.exception.code.value, "unknown_session")
        with self.assertRaises(Exception) as ctx2:
            adapter._raise_follow_error({"error": {"code": "session/follow-unsupported"}}, "s1")
        self.assertEqual(ctx2.exception.code.value, "unsupported")
        with self.assertRaises(Exception) as ctx3:
            adapter._raise_follow_error({"error": {"code": "boom", "message": "x"}}, "s1")
        self.assertEqual(ctx3.exception.code.value, "engine_unavailable")


class StreamFrameTests(unittest.TestCase):
    def setUp(self):
        self.adapter = make_adapter()
        self.record = make_record()

    def frame(self, chunk):
        return {"type": "chunk", "chunk": chunk}

    def test_text_delta_appends_and_emits(self):
        reason = self.adapter._on_stream_frame(
            self.record, self.frame({"type": "text-delta", "text": "合成"}))
        self.assertIsNone(reason)
        self.assertEqual(self.record.text_parts, ["合成"])
        self.assertEqual(self.record.text_len, 2)
        events = [e for e in self.record.events if e.type == EventType.TEXT_DELTA]
        self.assertEqual(len(events), 1)
        self.assertTrue(events[0].payload["streamed"])
        self.assertEqual(events[0].payload["chars"], 2)

    def test_empty_delta_ignored(self):
        self.adapter._on_stream_frame(self.record, self.frame({"type": "text-delta", "text": ""}))
        self.assertEqual(self.record.text_parts, [])
        self.assertEqual(self.record.seq, 0)  # 空增量不产生事件

    def test_finish_reason_returned(self):
        reason = self.adapter._on_stream_frame(
            self.record, self.frame({"type": "finish", "reason": {"kind": "max-tokens"}}))
        self.assertEqual(reason, "max-tokens")

    def test_usage_becomes_progress(self):
        self.adapter._on_stream_frame(
            self.record, self.frame({"type": "usage", "usage": {"inputTokens": 3}}))
        progress = [e for e in self.record.events if e.type == EventType.PROGRESS]
        self.assertEqual(progress[0].payload["kind"], "usage")
        self.assertEqual(progress[0].payload["usage"]["inputTokens"], 3)

    def test_stream_end_non_committed_is_progress(self):
        self.adapter._on_stream_frame(self.record, {"type": "end", "outcome": {"kind": "aborted"}})
        progress = [e for e in self.record.events if e.type == EventType.PROGRESS]
        self.assertEqual(progress[0].payload["outcome"], "aborted")

    def test_unknown_chunk_and_frame_are_ignored(self):
        self.assertIsNone(self.adapter._on_stream_frame(self.record, self.frame({"type": "nope"})))
        self.assertIsNone(self.adapter._on_stream_frame(self.record, {"type": "unknown"}))
        self.assertIsNone(self.adapter._on_stream_frame(self.record, {"type": "chunk"}))
        self.assertEqual(self.record.text_len, 0)

    def test_message_text_joins_only_text_blocks(self):
        text = WebApiAdapter._message_text({"content": [
            {"type": "text", "text": "甲"}, {"type": "image", "data": "x"},
            {"type": "text", "text": "乙"}]})
        self.assertEqual(text, "甲乙")
        self.assertEqual(WebApiAdapter._message_text({"content": "raw"}), "")


class SettleTurnTests(unittest.TestCase):
    def settle(self, reason, finish=None, final_text=""):
        adapter = make_adapter()
        record = make_record()
        adapter._settle_turn(record, reason, finish, final_text)
        return record

    def test_completed_is_succeeded(self):
        record = self.settle("completed", "stop", "答")
        self.assertEqual(record.state, TaskState.SUCCEEDED)
        self.assertFalse(record.truncated)
        self.assertEqual(record.text_parts, ["答"])

    def test_completed_with_length_is_truncated(self):
        record = self.settle("completed", "max-tokens", "答")
        self.assertEqual(record.state, TaskState.SUCCEEDED)
        self.assertTrue(record.truncated)

    def test_aborted_is_cancelled(self):
        self.assertEqual(self.settle("aborted").state, TaskState.CANCELLED)

    def test_error_and_failed_are_failed(self):
        for reason in ("error", "failed"):
            record = self.settle(reason)
            self.assertEqual(record.state, TaskState.FAILED, reason)
            self.assertEqual(record.error_code, "task_failed")

    def test_max_tokens_is_succeeded_but_truncated(self):
        record = self.settle("max-tokens")
        self.assertEqual(record.state, TaskState.SUCCEEDED)
        self.assertTrue(record.truncated)

    def test_interrupted_is_interrupted(self):
        self.assertEqual(self.settle("interrupted").state, TaskState.INTERRUPTED)

    def test_blocked_and_unknown_are_not_success(self):
        for reason in ("blocked", "", "future-reason"):
            record = self.settle(reason)
            self.assertEqual(record.state, TaskState.UNKNOWN, reason)

    def test_non_streamed_fallback_is_marked(self):
        record = self.settle("completed", "stop", "整段")
        delta = [e for e in record.events if e.type == EventType.TEXT_DELTA][0]
        self.assertFalse(delta.payload["streamed"])


class RedactionAndCommandTests(unittest.TestCase):
    def test_redact_hides_token(self):
        adapter = make_adapter()
        redacted = adapter._redact("dsh web: http://127.0.0.1:1234/?token=SECRET123&x=1")
        self.assertNotIn("SECRET123", redacted)
        self.assertIn("token=<redacted>", redacted)
        self.assertIn("x=1", redacted)
        self.assertEqual(adapter._redact("plain line"), "plain line")

    def test_redact_handles_token_without_query(self):
        adapter = make_adapter()
        self.assertNotIn("ABC", adapter._redact("url?token=ABC"))

    def test_for_dsh_argument_order(self):
        # `--patch` 必须是 launcher 级参数，排在子命令参数之前。
        adapter = WebApiAdapter.for_dsh(
            "/tmp/bin.js", cwd="/tmp", profile="web",
            patch_files=["/tmp/a.yml", "/tmp/b.yml"], state_dir="/tmp/s")
        cmd = adapter._command
        self.assertEqual(cmd[:4], ["node", "/tmp/bin.js", "--profile", "web"])
        self.assertEqual(cmd[4:8], ["--patch", "/tmp/a.yml", "--patch", "/tmp/b.yml"])
        self.assertEqual(cmd[8:], ["--port", "0", "--no-open"])
        self.assertEqual(adapter._engine_profile, "web")

    def test_for_dsh_rejects_relative_cwd_and_empty_command(self):
        with self.assertRaises(Exception):
            WebApiAdapter(command=[], cwd="/tmp")
        with self.assertRaises(Exception):
            WebApiAdapter(command=["node"], cwd="relative/path")


class SpawnIsolationTests(unittest.TestCase):
    """OD-ARCH-050 回归：DSH 子进程不得继承引擎的协议 stdin。

    继承后 Node 会把共享的“打开文件描述”设为非阻塞，引擎的 `for line in sys.stdin`
    随后读到假 EOF 并安静退出（rc=0、无栈）；表现是“引擎启动后立刻退出”、
    模型目录永远拿不到。
    """

    def test_dsh_child_stdin_is_devnull(self):
        import inspect

        source = inspect.getsource(WebApiAdapter._ensure_started)
        self.assertIn("stdin=subprocess.DEVNULL", source)
        self.assertIn("stderr=subprocess.STDOUT", source)


class SessionHelpersTests(unittest.TestCase):
    def test_highest_turn_reads_record_and_event(self):
        records = [{"turn": 2}, {"event": {"data": {"turn": 5}}}, {"turn": 3}, "junk"]
        self.assertEqual(WebApiAdapter._highest_turn(records), 5)
        self.assertEqual(WebApiAdapter._highest_turn(None), 0)
        self.assertEqual(WebApiAdapter._highest_turn([]), 0)


class ApprovalChannelTests(unittest.TestCase):
    """OD-ARCH-066：审批帧归一化、应答、与 TTFB 埋点。

    只构造适配层对象，不启动引擎、不发网络请求；应答路径用替身替换 `_rpc`。
    """

    def setUp(self):
        self.adapter = make_adapter(tmp_state="/tmp/od066-unit-state")
        self.record = make_record()
        self.record.engine_session_id = "sess-1"
        self.record.submitted_monotonic = time.monotonic()
        self.adapter._sessions["sess-1"] = self.record
        self.adapter._tasks[self.record.omni_task_id] = self.record

    def request_frame(self, event_id="ev-1", agent_id="sess-1"):
        return {
            "type": "waterfall",
            "event": "approval/request",
            "eventId": event_id,
            "agentId": agent_id,
            "request": {
                "toolName": "bash",
                "callId": "call-synth-1",
                "reason": "synthetic approval probe",
            },
        }

    def events(self):
        return list(self.record.events)

    def test_request_becomes_progress_and_pending(self):
        self.adapter._on_approval_request(self.request_frame())
        self.assertEqual(len(self.record.pending_approvals), 1)
        events = self.events()
        self.assertEqual(events[-1].type, EventType.PROGRESS)
        self.assertEqual(events[-1].payload["kind"], "approval_request")
        self.assertEqual(events[-1].payload["tool_name"], "bash")
        self.assertEqual(events[-1].payload["approval_id"], "ev-1")

    def test_duplicate_request_is_ignored(self):
        self.adapter._on_approval_request(self.request_frame())
        before = len(self.events())
        self.adapter._on_approval_request(self.request_frame())
        self.assertEqual(len(self.events()), before)
        self.assertEqual(len(self.record.pending_approvals), 1)

    def test_unmatched_agent_is_not_attributed(self):
        self.adapter._on_approval_request(self.request_frame(agent_id="other") )
        self.assertEqual(len(self.record.pending_approvals), 0)
        self.assertEqual(len(self.events()), 0)

    def test_cancel_event_clears_pending(self):
        self.adapter._on_approval_request(self.request_frame())
        self.adapter._on_approval_cancel("ev-1")
        self.assertEqual(len(self.record.pending_approvals), 0)
        self.assertEqual(self.events()[-1].payload["kind"], "approval_cancelled")

    def test_status_note_mentions_waiting(self):
        self.adapter._on_approval_request(self.request_frame())
        status = self.adapter.get_task_status(self.record.omni_task_id)
        self.assertEqual(status.pending_approvals, 1)
        self.assertIn("等待审批中", status.note)

    def _patch_rpc(self, sink):
        self.adapter._rpc = lambda method, args, timeout, **kw: sink(method, args)

    def test_respond_allow_and_reject_payloads(self):
        calls = []

        def sink(method, args):
            calls.append((method, dict(args)))
            return None

        self._patch_rpc(sink)
        self.adapter._ensure_approval_channel = lambda: type(
            "C", (), {"ensure_started": lambda self_: "client-1"}
        )()

        self.adapter._on_approval_request(self.request_frame())
        receipt = self.adapter.respond_approval(self.record.omni_task_id, "ev-1", True)
        self.assertEqual(calls[-1][0], "$events/result")
        self.assertEqual(calls[-1][1]["outcome"]["value"], "allowed-once")
        self.assertEqual(calls[-1][1]["clientId"], "client-1")
        self.assertEqual(receipt["decision"], "allowed")
        self.assertEqual(receipt["pending_approvals"], 0)

        # 重复答复：已不在待办里，必须拒绝而不是再发一次。
        with self.assertRaises(Exception) as ctx:
            self.adapter.respond_approval(self.record.omni_task_id, "ev-1", True)
        self.assertIn("not pending", str(ctx.exception))

    def test_respond_rejects_foreign_task(self):
        self._patch_rpc(lambda method, args: None)
        self.adapter._ensure_approval_channel = lambda: type(
            "C", (), {"ensure_started": lambda self_: "client-1"}
        )()
        self.adapter._on_approval_request(self.request_frame())
        other = make_record()
        other.omni_task_id = "omni-other"
        self.adapter._tasks["omni-other"] = other
        with self.assertRaises(Exception):
            self.adapter.respond_approval("omni-other", "ev-1", True)

    def test_failed_rpc_keeps_pending(self):
        def boom(method, args, timeout, **kw):
            raise RuntimeError("rpc down")

        self.adapter._rpc = boom
        self.adapter._ensure_approval_channel = lambda: type(
            "C", (), {"ensure_started": lambda self_: "client-1"}
        )()
        self.adapter._on_approval_request(self.request_frame())
        with self.assertRaises(RuntimeError):
            self.adapter.respond_approval(self.record.omni_task_id, "ev-1", True)
        self.assertEqual(len(self.record.pending_approvals), 1)

    def test_terminal_settle_clears_pending(self):
        self.adapter._on_approval_request(self.request_frame())
        self.adapter._settle(self.record, TaskState.SUCCEEDED, "engine_terminal")
        self.assertEqual(len(self.record.pending_approvals), 0)
        self.assertEqual(self.record.state, TaskState.SUCCEEDED)

    def test_ttfb_only_first_delta(self):
        frame = {"type": "chunk", "chunk": {"type": "text-delta", "text": "合成"}}
        self.adapter._on_stream_frame(self.record, frame)
        first = self.record.ttfb_seconds
        self.assertIsNotNone(first)
        self.adapter._on_stream_frame(self.record, frame)
        self.assertEqual(self.record.ttfb_seconds, first)

    def test_no_ttfb_before_any_delta(self):
        self.assertIsNone(self.record.ttfb_seconds)
        status = self.adapter.get_task_status(self.record.omni_task_id)
        self.assertIsNone(status.ttfb_seconds)


class EngineAskTests(unittest.TestCase):
    """OD-ARCH-069：非 approval 的 waterfall 询问必须可见，且绝不自动应答。"""

    def setUp(self):
        self.adapter = make_adapter(tmp_state="/tmp/od069-unit-state")
        self.record = make_record()
        self.record.engine_session_id = "sess-1"
        self.adapter._sessions["sess-1"] = self.record
        self.adapter._tasks[self.record.omni_task_id] = self.record

    def ask_frame(self, event="hook/question", event_id="ask-1", agent_id="sess-1"):
        return {
            "type": "waterfall",
            "event": event,
            "eventId": event_id,
            "agentId": agent_id,
            # 询问正文可能含业务内容：适配层不应把它带进事件。
            "payload": {"question": "合成占位提问"},
        }

    def test_non_approval_waterfall_becomes_visible_progress(self):
        self.adapter._on_engine_ask(self.ask_frame())
        events = list(self.record.events)
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0].type, EventType.PROGRESS)
        self.assertEqual(events[0].payload["kind"], "engine_ask")
        self.assertEqual(events[0].payload["event"], "hook/question")

    def test_ask_event_does_not_carry_question_text(self):
        self.adapter._on_engine_ask(self.ask_frame())
        payload = dict(self.record.events[-1].payload)
        self.assertNotIn("question", payload)
        self.assertNotIn("合成占位提问", str(payload))

    def test_duplicate_ask_is_deduplicated(self):
        self.adapter._on_engine_ask(self.ask_frame())
        self.adapter._on_engine_ask(self.ask_frame())
        self.assertEqual(len(self.record.events), 1)

    def test_ask_for_unknown_session_is_ignored(self):
        self.adapter._on_engine_ask(self.ask_frame(agent_id="other"))
        self.assertEqual(len(self.record.events), 0)

    def test_ask_after_terminal_is_ignored(self):
        self.adapter._settle(self.record, TaskState.SUCCEEDED, "engine_terminal")
        self.adapter._on_engine_ask(self.ask_frame())
        kinds = [e.payload.get("kind") for e in self.record.events]
        self.assertNotIn("engine_ask", kinds)

    def test_ask_never_sends_an_answer_frame(self):
        calls = []
        self.adapter._rpc = lambda method, args, timeout, **kw: calls.append(method)
        self.adapter._on_engine_ask(self.ask_frame())
        self.assertEqual(calls, [])


class TimeoutPolicyTests(unittest.TestCase):
    """2026-10-23：默认不按墙钟收尾；只有显式正整数才设截止。"""

    def setUp(self):
        self.adapter = make_adapter(tmp_state="/tmp/od069-timeout-state")
        self.record = make_record()

    def test_default_timeout_is_none(self):
        """缺省无上限：DSH 还在跑就不能因为耗时被判定 unknown。"""
        self.assertIsNone(self.adapter._task_timeout(self.record))

    def test_timeout_policy_overrides_default(self):
        self.record.timeouts = {"total_seconds": 420}
        self.assertEqual(self.adapter._task_timeout(self.record), 420.0)

    def test_invalid_timeout_means_no_cutoff(self):
        for bad in (0, -5, "abc", 99999):
            self.record.timeouts = {"total_seconds": bad}
            expected = 99999.0 if bad == 99999 else None
            self.assertEqual(self.adapter._task_timeout(self.record), expected, bad)

    def test_timeout_note_states_seconds_and_no_auto_retry(self):
        note = WebApiAdapter._timeout_note(300)
        self.assertIn("300", note)
        self.assertIn("未自动重试", note)
        self.assertIn("重发", note)

    def test_tool_activity_carries_tool_name(self):
        self.adapter._emit(
            self.record,
            EventType.TOOL_ACTIVITY,
            {"kind": "tool/call", "tool_name": "web_search", "status": "running"},
        )
        self.assertEqual(self.record.events[-1].payload["tool_name"], "web_search")

    def test_tool_name_helper_tolerates_unknown_shapes(self):
        self.assertEqual(WebApiAdapter._tool_name_of({}, {}), "")
        self.assertEqual(WebApiAdapter._tool_name_of({"name": "read"}, {}), "read")
        self.assertEqual(WebApiAdapter._tool_name_of({}, {"toolName": "bash"}), "bash")
        self.assertEqual(WebApiAdapter._tool_name_of({"name": "  "}, {}), "")

    # -- OD-ARCH-070：搜索超时与上游错误透传 --------------------------------

    def test_search_tool_no_longer_extends_or_shortens_anything(self):
        """检索取消单独上限后，工具事件不应再改动任何截止时间。"""
        self.assertFalse(hasattr(WebApiAdapter, "_is_search_tool"))
        self.assertFalse(hasattr(WebApiAdapter, "_search_task_timeout"))

    def test_upstream_error_becomes_task_failed_with_message(self):
        """上游 400（内容风险）→ task_failed，且原因可读。"""
        value = {"error": {"code": "upstream_error", "message": "Content Exists Risk"}}
        with self.assertRaises(Exception) as ctx:
            self.adapter._raise_follow_error(value, "s1")
        self.assertEqual(ctx.exception.code.value, "task_failed")
        self.assertIn("Content Exists Risk", ctx.exception.message)

    def test_long_upstream_message_is_truncated_to_300(self):
        value = {"error": {"code": "upstream_error", "message": "x" * 900}}
        with self.assertRaises(Exception) as ctx:
            self.adapter._raise_follow_error(value, "s1")
        message = ctx.exception.message
        self.assertLessEqual(len(message), 310)
        self.assertIn("已截断", message)

    def test_http_400_with_upstream_body_is_task_failed(self):
        """真实 HTTPError 分支：body 是 JSON 且带 upstream_error。"""
        import email.message
        import io
        import urllib.error

        body = b'{"error":{"code":"upstream_error","message":"Content Exists Risk"}}'
        err = urllib.error.HTTPError(
            "http://127.0.0.1/api/x", 400, "Bad Request", email.message.Message(), io.BytesIO(body)
        )
        mapped = self.adapter._map_http_error(err, "session/prompt", None)
        self.assertEqual(mapped.code.value, "task_failed")
        self.assertIn("Content Exists Risk", mapped.message)

    def test_http_500_stays_engine_unavailable(self):
        import email.message
        import io
        import urllib.error

        err = urllib.error.HTTPError(
            "http://127.0.0.1/api/x", 500, "Boom", email.message.Message(), io.BytesIO(b"")
        )
        mapped = self.adapter._map_http_error(err, "session/prompt", None)
        self.assertEqual(mapped.code.value, "engine_unavailable")

    def test_recv_budget_survives_heartbeat_frames(self):
        """OD-ARCH-069 回归：心跳帧不得把 recv 的总等待预算无限续期。

        复现现场：DSH 每 2 秒发一次 ping，旧实现按「每段读取」计时，
        导致 8 秒总超时始终到不了。这里用一个只发 ping 的对端验证。
        """
        from omnidesk_engine.web_api_adapter import WsClient

        server, peer = socket.socketpair()
        stop = threading.Event()

        def heartbeat():
            try:
                while not stop.is_set():
                    peer.sendall(bytes([0x89, 0x80]) + b"\x00\x00\x00\x00")
                    stop.wait(0.2)
            except OSError:
                pass

        thread = threading.Thread(target=heartbeat, daemon=True)
        thread.start()
        client = WsClient.__new__(WsClient)
        client.sock = server
        client._buf = bytearray()
        client._fragments = bytearray()
        client.closed = False
        started = time.monotonic()
        try:
            with self.assertRaises(TimeoutError):
                client.recv(timeout=1.0)
            elapsed = time.monotonic() - started
            self.assertGreaterEqual(elapsed, 0.8)
            self.assertLess(elapsed, 6.0, "心跳不应无限续期等待预算")
        finally:
            stop.set()
            server.close()
            peer.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
