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
    MATERIAL_TYPES,
    assemble_task_extract_prompt,
    validate_material_payload,
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

    def test_prompt_injects_existing_material_titles_for_match(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="正文",
            record_date="2026-11-08",
            materials=[
                {"id": "m-1", "title": "虚构素材甲", "material_type": "工作事实"},
                {"id": "m-2", "title": "虚构素材乙", "material_type": "数据素材"},
            ],
        )
        self.assertIn("已有素材（可用于判断重复", prompt)
        self.assertIn("id=m-1 | 标题：虚构素材甲 | 类型：工作事实", prompt)
        self.assertIn("id=m-2 | 标题：虚构素材乙 | 类型：数据素材", prompt)
        self.assertIn("match_material_id 只能引用下列编号", prompt)

    def test_prompt_states_no_existing_materials(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="正文",
            record_date="2026-11-08",
        )
        self.assertIn("已有素材：无。", prompt)

    def test_spec_mentions_link_suggestion_fields(self):
        self.assertIn("match_task_id", JSON_OUTPUT_SPEC)
        self.assertIn("match_confidence", JSON_OUTPUT_SPEC)
        self.assertIn("match_reason", JSON_OUTPUT_SPEC)
        self.assertIn("suggested_state", JSON_OUTPUT_SPEC)
        self.assertIn("不会自动应用", JSON_OUTPUT_SPEC)

    def test_spec_requires_importance_fields_and_constraints(self):
        self.assertIn('"importance"', JSON_OUTPUT_SPEC)
        self.assertIn('"importance_reason"', JSON_OUTPUT_SPEC)
        self.assertIn("不要仅因为领导提出就判「高」", JSON_OUTPUT_SPEC)
        self.assertIn("不要仅因为没有截止日期就判「低」", JSON_OUTPUT_SPEC)
        self.assertIn("100 字", JSON_OUTPUT_SPEC)

    def test_spec_contains_material_candidates_and_fact_constraints(self):
        self.assertIn("material_candidates", JSON_OUTPUT_SPEC)
        self.assertIn("material_type", JSON_OUTPUT_SPEC)
        self.assertIn("knowledge_hints", JSON_OUTPUT_SPEC)
        self.assertIn("match_material_id", JSON_OUTPUT_SPEC)
        self.assertIn("不得凭空生成", JSON_OUTPUT_SPEC)
        self.assertIn("不得把计划写成成果", JSON_OUTPUT_SPEC)
        self.assertIn("不得把「准备做」写成「已经做成」", JSON_OUTPUT_SPEC)
        self.assertIn("content 与 source_quote", JSON_OUTPUT_SPEC)
        for material_type in MATERIAL_TYPES:
            self.assertIn(material_type, JSON_OUTPUT_SPEC)

    def test_prompt_mentions_material_candidates(self):
        prompt = assemble_task_extract_prompt(
            title="记录",
            body="今天整理周报。",
            record_date="2026-11-08",
        )
        self.assertIn("写作素材候选", prompt)
        self.assertIn('"material_candidates"', prompt)

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

    def test_cli_injects_materials_payload(self):
        payload = {
            "schema": EXTRACT_SCHEMA,
            "record": {
                "id": "wr-2",
                "title": "素材复盘",
                "record_date": "2026-11-08",
                "tags": [],
                "body": "虚构正文。",
            },
            "tasks": [],
            "materials": [
                {"id": "m-9", "title": "虚构已有素材", "material_type": "工作成果"},
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
        self.assertIn("id=m-9 | 标题：虚构已有素材 | 类型：工作成果", data["prompt"])


class MaterialPayloadValidationTests(unittest.TestCase):
    BODY = "今天整理周报。2026-11-30 前交总结。"

    def _payload(self, **overrides):
        candidate = {
            "material_type": "工作事实",
            "content": "周报整理已完成并归档",
            "source_quote": "今天整理周报",
            "event_date": None,
            "match_material_id": None,
            "match_reason": None,
            "knowledge_hints": ["周报", "复盘"],
            "confidence": 0.7,
        }
        candidate.update(overrides)
        return {"material_candidates": [candidate]}

    def test_accepts_valid_candidate_and_normalizes_hints(self):
        data = self._payload(knowledge_hints=[" 周报 ", "复盘", "周报", ""])
        out = validate_material_payload(data, self.BODY)
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0]["material_type"], "工作事实")
        self.assertEqual(out[0]["knowledge_hints"], ["周报", "复盘"])
        self.assertEqual(out[0]["confidence"], 0.7)

    def test_rejects_content_that_copies_quote(self):
        data = self._payload(content="今天整理周报")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("content 与 source_quote 相同", str(ctx.exception))
        data = self._payload(content="今天 整理 周报")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("content 与 source_quote 相同", str(ctx.exception))

    def test_rejects_non_whitelist_type(self):
        data = self._payload(material_type="新闻稿")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("十类白名单", str(ctx.exception))

    def test_rejects_quote_not_in_body(self):
        data = self._payload(source_quote="记录里没有这句话")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("连续原文", str(ctx.exception))

    def test_rejects_unstated_or_malformed_event_date(self):
        data = self._payload(event_date="2026-11-29")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("不在正文里", str(ctx.exception))
        data = self._payload(event_date="11月30日")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("形状不正确", str(ctx.exception))

    def test_rejects_orphan_match_reason(self):
        data = self._payload(match_reason="同一事项")
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("没有关联素材编号", str(ctx.exception))

    def test_rejects_bad_hint_limits(self):
        data = self._payload(knowledge_hints=["一", "二", "三", "四", "五", "六", "七", "八", "九"])
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("主题关键词过多", str(ctx.exception))
        data = self._payload(
            knowledge_hints=["这个主题关键词写得太长了已经超过二十四个汉字的限制"]
        )
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("主题关键词过长", str(ctx.exception))

    def test_rejects_too_many_material_candidates(self):
        candidate = self._payload()["material_candidates"][0]
        data = {"material_candidates": [dict(candidate) for _ in range(21)]}
        with self.assertRaises(ValueError) as ctx:
            validate_material_payload(data, self.BODY)
        self.assertIn("素材候选过多", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
