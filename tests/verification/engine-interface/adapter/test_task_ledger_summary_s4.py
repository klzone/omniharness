#!/usr/bin/env python3
"""OD-KB-201 S4：任务台账摘要保留失败原因与意图字段。"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import _harness  # noqa: F401 - 把 engine/ 加入 sys.path

from omnidesk_engine.task_core import TaskCoreBase, TaskRecordSummary


class TaskLedgerSummaryS4(unittest.TestCase):
    def test_summary_exposes_failure_reason_without_intent(self) -> None:
        summary = TaskRecordSummary(
            omni_task_id="omni-s4",
            state="failed",
            intent="knowledge_ingest",
            created_at=1.0,
            state_since=2.0,
            engine_session_id=None,
            cancel_phase="not_requested",
            persistence="recorded",
            error_code="upstream_error",
            error_message="synthetic upstream timeout",
        )
        payload = summary.to_dict()
        self.assertNotIn("intent", payload, "intent 是私有分类，不应暴露到台账响应")
        self.assertEqual(payload["error_code"], "upstream_error")
        self.assertEqual(payload["error_message"], "synthetic upstream timeout")

    def test_list_task_records_reads_failure_fields_from_ledger(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "omni-s4.json"
            path.write_text(
                json.dumps(
                    {
                        "omni_task_id": "omni-s4",
                        "state": "failed",
                        "intent": "knowledge_ingest",
                        "created_at": 1.0,
                        "state_since": 2.0,
                        "engine_session_id": None,
                        "cancel_phase": "not_requested",
                        "error_code": "upstream_error",
                        "error_message": "synthetic upstream timeout",
                    },
                    ensure_ascii=False,
                ),
                encoding="utf-8",
            )
            core = TaskCoreBase(state_dir=tmp)
            listing = core.list_task_records(limit=10)
            self.assertEqual(len(listing.records), 1)
            payload = listing.to_dict()["records"][0]
            self.assertNotIn("intent", payload)
            self.assertEqual(payload["error_message"], "synthetic upstream timeout")


if __name__ == "__main__":
    unittest.main()
