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


if __name__ == "__main__":
    unittest.main(verbosity=2)
