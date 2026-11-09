#!/usr/bin/env python3
"""OD-WT-604 写作需求解析提示词与形状校验单测（全部虚构数据）。"""

from __future__ import annotations

import json
import subprocess
import sys
import unittest

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.writing_extract import (
    DOCUMENT_TYPES,
    EXTRACT_SCHEMA,
    JSON_OUTPUT_SPEC,
    assemble_writing_requirement_prompt,
    validate_writing_payload,
)


def complete_payload(**overrides):
    payload = {
        "document_type": "月度总结",
        "title": "虚构月份试点总结",
        "topic": "虚构试点推进情况",
        "writing_purpose": "向上级汇报本月进展",
        "audience": "部门同事",
        "writing_period": "2026-11",
        "expected_length": "1200 字",
        "writing_style": "简洁平实",
        "requirements": ["只写已确认事实", "三点以内"],
        "questions": [],
        "confidence": 0.8,
    }
    payload.update(overrides)
    return payload


class WritingRequirementPromptTests(unittest.TestCase):
    def test_prompt_contains_text_spec_and_boundaries(self):
        prompt = assemble_writing_requirement_prompt(
            "下个月要交一份部门月度总结，面向领导，一千二百字左右。"
        )
        self.assertIn(JSON_OUTPUT_SPEC, prompt)
        self.assertIn("---BEGIN---", prompt)
        self.assertIn("---END---", prompt)
        self.assertIn("部门月度总结", prompt)
        self.assertIn("只输出草稿", prompt)

    def test_prompt_lists_all_twelve_document_types(self):
        prompt = assemble_writing_requirement_prompt("写一份总结。")
        self.assertEqual(len(DOCUMENT_TYPES), 12)
        for document_type in DOCUMENT_TYPES:
            self.assertIn("- " + document_type, prompt)

    def test_spec_forbids_guessing_and_requires_questions(self):
        self.assertIn("不得猜测", JSON_OUTPUT_SPEC)
        self.assertIn("不要编造", JSON_OUTPUT_SPEC)
        self.assertIn("questions", JSON_OUTPUT_SPEC)
        self.assertIn("文稿类型", JSON_OUTPUT_SPEC)
        self.assertIn("不得为空", JSON_OUTPUT_SPEC)
        self.assertIn("不创建任务", JSON_OUTPUT_SPEC)
        self.assertIn("不写入任何文件", JSON_OUTPUT_SPEC)

    def test_prompt_rejects_empty_or_overlong_text(self):
        with self.assertRaises(ValueError) as ctx:
            assemble_writing_requirement_prompt("   ")
        self.assertIn("不能为空", str(ctx.exception))
        with self.assertRaises(ValueError) as ctx:
            assemble_writing_requirement_prompt("长" * 20_001)
        self.assertIn("过长", str(ctx.exception))

    def test_cli_returns_prompt_json_without_draft(self):
        payload = {"schema": EXTRACT_SCHEMA, "text": "虚构写作需求：月度总结。"}
        proc = subprocess.run(
            [sys.executable, "-m", "omnidesk_engine.writing_extract"],
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
        self.assertIn("虚构写作需求", data["prompt"])
        self.assertNotIn("document_type", data, "CLI 只应返回提示词，不返回解析草稿")


class WritingPayloadValidationTests(unittest.TestCase):
    def test_accepts_complete_payload(self):
        out = validate_writing_payload(complete_payload())
        self.assertEqual(out["document_type"], "月度总结")
        self.assertEqual(out["requirements"], ["只写已确认事实", "三点以内"])
        self.assertEqual(out["questions"], [])
        self.assertEqual(out["confidence"], 0.8)

    def test_rejects_document_type_outside_whitelist(self):
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(complete_payload(document_type="新闻稿"))
        self.assertIn("十二类白名单", str(ctx.exception))

    def test_rejects_empty_document_type_string(self):
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(complete_payload(document_type="  "))
        self.assertIn("必须填 null", str(ctx.exception))

    def test_unknown_type_requires_type_question(self):
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(
                complete_payload(document_type=None, questions=["篇幅希望多长？"])
            )
        self.assertIn("询问文稿类型", str(ctx.exception))

    def test_accepts_unknown_type_with_type_question(self):
        payload = complete_payload(
            document_type=None,
            questions=["这份材料属于哪种文稿类型？"],
        )
        out = validate_writing_payload(payload)
        self.assertIsNone(out["document_type"])
        self.assertEqual(out["questions"], ["这份材料属于哪种文稿类型？"])

    def test_missing_core_field_requires_questions(self):
        payload = complete_payload(audience=None)
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(payload)
        self.assertIn("questions 不得为空", str(ctx.exception))
        payload["questions"] = ["目标受众是谁？"]
        out = validate_writing_payload(payload)
        self.assertIsNone(out["audience"])
        self.assertEqual(out["questions"], ["目标受众是谁？"])

    def test_rejects_missing_questions_or_requirements(self):
        payload = complete_payload()
        payload.pop("questions")
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(payload)
        self.assertIn("缺少 questions", str(ctx.exception))
        payload = complete_payload()
        payload.pop("requirements")
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(payload)
        self.assertIn("缺少 requirements", str(ctx.exception))

    def test_rejects_empty_or_overlong_items(self):
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(complete_payload(questions=[" "]))
        self.assertIn("空条目", str(ctx.exception))
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(
                complete_payload(topic="长" * 401)
            )
        self.assertIn("过长", str(ctx.exception))
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(
                complete_payload(requirements=["长" * 201])
            )
        self.assertIn("条目过长", str(ctx.exception))
        with self.assertRaises(ValueError) as ctx:
            validate_writing_payload(
                complete_payload(questions=[f"问题{i}" for i in range(11)])
            )
        self.assertIn("过多", str(ctx.exception))

    def test_rejects_bad_confidence(self):
        for value in (1.2, -0.1, True):
            with self.assertRaises(ValueError) as ctx:
                validate_writing_payload(complete_payload(confidence=value))
            message = str(ctx.exception)
            self.assertTrue(
                "0 到 1" in message or "数字" in message, message
            )

    def test_deduplicates_requirements_and_questions(self):
        payload = complete_payload(
            requirements=[" 只写事实 ", "只写事实", "三点以内"],
            questions=["受众是谁？", "受众是谁？"],
            audience=None,
        )
        out = validate_writing_payload(payload)
        self.assertEqual(out["requirements"], ["只写事实", "三点以内"])
        self.assertEqual(out["questions"], ["受众是谁？"])

    def test_all_string_fields_may_be_null_when_questions_present(self):
        payload = complete_payload(
            document_type=None,
            title=None,
            topic=None,
            writing_purpose=None,
            audience=None,
            writing_period=None,
            expected_length=None,
            writing_style=None,
            requirements=[],
            questions=[
                "这份材料属于哪种文稿类型？",
                "标题、主题、目的、受众、时间范围与篇幅分别是什么？",
            ],
            confidence=0.2,
        )
        out = validate_writing_payload(payload)
        self.assertIsNone(out["document_type"])
        self.assertIsNone(out["title"])
        self.assertEqual(out["requirements"], [])
        self.assertEqual(len(out["questions"]), 2)


if __name__ == "__main__":
    unittest.main()
