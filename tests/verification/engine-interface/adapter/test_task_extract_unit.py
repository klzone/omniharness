#!/usr/bin/env python3
"""OD-WR-304 任务候选提示词组装单测（全部虚构数据）。"""

from __future__ import annotations

import json
import subprocess
import sys
import unittest

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.task_extract import (
    EXTRACT_SCHEMA,
    JSON_OUTPUT_SPEC,
    assemble_task_extract_prompt,
)


class TaskExtractPromptTests(unittest.TestCase):
    def test_prompt_contains_record_and_spec(self):
        prompt = assemble_task_extract_prompt(
            title="周一复盘",
            body="2026-11-30 前交周报。",
            record_date="2026-11-02",
            category="项目",
            tags=["复盘", "周报"],
        )
        self.assertIn("周一复盘", prompt)
        self.assertIn("2026-11-02", prompt)
        self.assertIn("项目", prompt)
        self.assertIn("复盘", prompt)
        self.assertIn("2026-11-30 前交周报。", prompt)
        self.assertIn(JSON_OUTPUT_SPEC, prompt)

    def test_prompt_has_body_boundaries(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="第一行。第二行。",
            record_date="2026-11-08",
        )
        self.assertIn("---BEGIN---", prompt)
        self.assertIn("---END---", prompt)
        self.assertIn("第一行。第二行。", prompt)

    def test_prompt_marks_missing_optional_metadata(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="正文",
            record_date="2026-11-08",
        )
        self.assertIn("分类：无", prompt)
        self.assertIn("标签：无", prompt)

    def test_spec_requires_quote_and_null_fuzzy_date(self):
        self.assertIn("source_quote", JSON_OUTPUT_SPEC)
        self.assertIn("due_date", JSON_OUTPUT_SPEC)
        self.assertIn("confidence", JSON_OUTPUT_SPEC)
        self.assertIn("模糊时间", JSON_OUTPUT_SPEC)
        self.assertIn("连续片段", JSON_OUTPUT_SPEC)

    def test_prompt_injects_unfinished_tasks(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="正文",
            record_date="2026-11-08",
            tasks=[
                {"id": "t-1", "title": "推进周报", "state": "进行中"},
                {"id": "t-2", "title": "整理材料", "state": "待开始"},
            ],
        )
        self.assertIn("已有未完成任务", prompt)
        self.assertIn("id=t-1", prompt)
        self.assertIn("推进周报", prompt)
        self.assertIn("进行中", prompt)

    def test_spec_mentions_link_suggestion_fields(self):
        self.assertIn("match_task_id", JSON_OUTPUT_SPEC)
        self.assertIn("match_confidence", JSON_OUTPUT_SPEC)
        self.assertIn("match_reason", JSON_OUTPUT_SPEC)
        self.assertIn("suggested_state", JSON_OUTPUT_SPEC)
        self.assertIn("不会自动应用", JSON_OUTPUT_SPEC)

    def test_cli_returns_prompt_json(self):
        payload = {
            "schema": EXTRACT_SCHEMA,
            "record": {
                "id": "wr-1",
                "title": "周一复盘",
                "record_date": "2026-11-02",
                "category": "项目",
                "tags": ["复盘"],
                "body": "2026-11-30 前交周报。",
            },
            "tasks": [
                {"id": "t-1", "title": "推进周报", "state": "进行中"},
            ],
        }
        proc = subprocess.run(
            [sys.executable, "-m", "omnidesk_engine.task_extract"],
            cwd=str(H.REPO / "engine"),
            input=json.dumps(payload, ensure_ascii=False),
            capture_output=True,
            text=True,
            timeout=20,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        data = json.loads(proc.stdout)
        self.assertEqual(data["schema"], EXTRACT_SCHEMA)
        self.assertIn("prompt", data)
        self.assertIn("周一复盘", data["prompt"])
        self.assertIn("id=t-1", data["prompt"])
        self.assertNotIn("candidates", data)


if __name__ == "__main__":
    unittest.main()
