#!/usr/bin/env python3
"""P2：任务状态快照不应只在终态落盘。"""
from __future__ import annotations

import json
import tempfile
import time
import unittest
from pathlib import Path

import _harness  # noqa: F401 - 把 engine/ 加入 sys.path

from omnidesk_engine.contract import EventType, TaskState
from omnidesk_engine.task_core import TaskCoreBase, _TaskRecord


class TaskStatusSnapshotTest(unittest.TestCase):
    def test_event_updates_persisted_seq_and_last_event_at(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            core = TaskCoreBase(state_dir=tmp)
            record = _TaskRecord(
                omni_task_id="omni-snapshot-test",
                intent="diagnostic_test",
                workspace_dir=tmp,
                idempotency_key=None,
                state=TaskState.STARTING,
                evidence_source="test",
                created_at=time.time(),
                state_since=time.time(),
            )
            core._tasks[record.omni_task_id] = record

            core._emit(record, EventType.TEXT_DELTA, {"text": "synthetic"})

            path = Path(tmp) / "omni-snapshot-test.json"
            self.assertTrue(path.is_file(), "事件后应落盘状态快照")
            payload = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(payload["seq"], 1)
            self.assertEqual(payload["last_event_seq"], 1)
            self.assertIsNotNone(payload.get("last_event_at"))
            self.assertEqual(payload["state"], "starting")

            status = core.get_task_status(record.omni_task_id)
            self.assertEqual(status.last_event_seq, 1)
            self.assertIsNotNone(status.last_event_at)

            core._emit(record, EventType.TOOL_ACTIVITY, {"tool_name": "synthetic"})
            payload = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(payload["seq"], 2)
            self.assertEqual(payload["last_event_seq"], 2)


if __name__ == "__main__":
    unittest.main()
