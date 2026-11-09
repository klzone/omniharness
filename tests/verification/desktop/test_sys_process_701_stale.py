#!/usr/bin/env python3
"""OD-SYS-701：引擎启动时的僵尸任务记录清理单测。

测试数据全部为合成记录，只落在临时目录；不读取、不改写真实 engine-state。

运行：python3 tests/verification/desktop/test_sys_process_701_stale.py -v
"""
from __future__ import annotations

import io
import json
import os
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "engine"))

from omnidesk_engine.contract import TERMINAL_STATES
from omnidesk_engine.task_core import (
    STALE_TASK_RECORD_SECONDS,
    TaskCoreBase,
    mark_stale_task_records,
)

NOW = 1_700_000_000.0


class StaleTaskRecordTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="od-sys-701-stale-")
        self.dir = Path(self._tmp.name)
        self.addCleanup(self._tmp.cleanup)

    def _write(
        self,
        task_id: str,
        state: str,
        age_seconds: float,
        *,
        content: str = "合成任务正文，不得改动",
    ) -> Path:
        stamp = NOW - age_seconds
        payload = {
            "omni_task_id": task_id,
            "state": state,
            "created_at": stamp,
            "state_since": stamp,
            "persisted_at": stamp,
            "content": content,
            "intent": "synthetic",
        }
        path = self.dir / (task_id + ".json")
        path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
        return path

    def test_expired_running_is_marked_and_content_preserved(self) -> None:
        path = self._write("omni-stale-001", "running", STALE_TASK_RECORD_SECONDS + 3600)
        before = json.loads(path.read_text(encoding="utf-8"))
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, ["omni-stale-001"])
        after = json.loads(path.read_text(encoding="utf-8"))
        self.assertIs(after.get("stale"), True)
        self.assertEqual(after["content"], before["content"], "正文不得改动")
        for key, value in before.items():
            self.assertEqual(after[key], value, f"字段 {key} 不得改动")
        self.assertTrue(path.is_file(), "不得删除记录文件")

    def test_fresh_running_is_left_alone(self) -> None:
        path = self._write("omni-fresh-002", "starting", STALE_TASK_RECORD_SECONDS - 3600)
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, [])
        self.assertNotIn("stale", json.loads(path.read_text(encoding="utf-8")))

    def test_old_terminal_record_is_not_marked(self) -> None:
        terminal = sorted(state.value for state in TERMINAL_STATES)[0]
        path = self._write("omni-done-003", terminal, STALE_TASK_RECORD_SECONDS + 7200)
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, [])
        self.assertNotIn("stale", json.loads(path.read_text(encoding="utf-8")))

    def test_already_stale_record_is_not_rewritten(self) -> None:
        path = self._write("omni-already-004", "running", STALE_TASK_RECORD_SECONDS + 3600)
        data = json.loads(path.read_text(encoding="utf-8"))
        data["stale"] = True
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        before = path.read_text(encoding="utf-8")
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, [])
        self.assertEqual(path.read_text(encoding="utf-8"), before)

    def test_malformed_record_is_skipped_without_blocking_others(self) -> None:
        (self.dir / "omni-broken-005.json").write_text("{not json", encoding="utf-8")
        self._write("omni-valid-006", "running", STALE_TASK_RECORD_SECONDS + 60)
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, ["omni-valid-006"])

    def test_sweep_prints_exactly_one_line(self) -> None:
        self._write("omni-log-a", "running", STALE_TASK_RECORD_SECONDS + 60)
        self._write("omni-log-b", "queued", STALE_TASK_RECORD_SECONDS + 60)
        capture = io.StringIO()
        with redirect_stderr(capture):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(sorted(marked), ["omni-log-a", "omni-log-b"])
        lines = [line for line in capture.getvalue().splitlines() if line.strip()]
        self.assertEqual(len(lines), 1, "一次启动只打一行日志")
        self.assertIn("stale", lines[0])

    def test_missing_directory_is_noop(self) -> None:
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir / "missing", now_ts=NOW)
        self.assertEqual(marked, [])

    def test_task_core_startup_sweeps_state_dir(self) -> None:
        path = self._write("omni-startup-007", "running", STALE_TASK_RECORD_SECONDS + 60)
        # 构造 TaskCoreBase 即模拟引擎启动；真实时间下这条 49 小时前的记录也会过期。
        with redirect_stderr(io.StringIO()):
            TaskCoreBase(state_dir=str(self.dir))
        self.assertIs(json.loads(path.read_text(encoding="utf-8")).get("stale"), True)

    def test_marked_record_keeps_original_permissions(self) -> None:
        path = self._write("omni-mode-008", "running", STALE_TASK_RECORD_SECONDS + 60)
        os.chmod(path, 0o600)
        with redirect_stderr(io.StringIO()):
            marked = mark_stale_task_records(self.dir, now_ts=NOW)
        self.assertEqual(marked, ["omni-mode-008"])
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)


if __name__ == "__main__":
    unittest.main()
