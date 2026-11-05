#!/usr/bin/env python3
"""OD-ARCH-141 S2 知识库整理提示词组装单测（全部虚构数据）。"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import unittest

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.knowledge_ingest import (
    JSON_OUTPUT_SPEC,
    assemble_ingest_prompt,
)
from omnidesk_engine.security import DataMasker, DictEntry


class IngestPromptTests(unittest.TestCase):
    def setUp(self):
        self.masker = DataMasker(entries=[DictEntry("PERSON", "林青禾")])
        self.files = [{"relative_path": "inbox/合成材料.md", "content": "林青禾 负责专项检查。\n\n```\n林青禾\n```\n"}]
        self.index = "# 知识库索引\n\n## 概念\n- [旧卡](概念/旧卡.md) | 标签：监督 | 旧卡摘要\n"
        self.card_spec = "卡片格式：frontmatter tags/sources/date；正文≤300字。"

    def test_prompt_contains_card_spec_and_index(self):
        result = assemble_ingest_prompt(self.files, self.index, self.card_spec, self.masker)
        self.assertIn(self.card_spec, result.prompt)
        self.assertIn("旧卡摘要", result.prompt)
        self.assertIn("收件箱材料", result.prompt)

    def test_file_content_is_masked(self):
        result = assemble_ingest_prompt(self.files, self.index, self.card_spec, self.masker)
        self.assertNotIn("林青禾 负责", result.prompt)
        self.assertIn("[PERSON_", result.prompt)
        # 代码块保持原样；Task 2 的边界不在提示词层重写。
        self.assertIn("```\n林青禾\n```", result.prompt)

    def test_mapping_is_held_by_result_not_prompt(self):
        result = assemble_ingest_prompt(self.files, self.index, self.card_spec, self.masker)
        self.assertEqual(len(result.mappings), 1)
        self.assertEqual(result.mappings[0][0], "inbox/合成材料.md")
        self.assertNotIn("林青禾", repr(result.mappings[0][1]))
        self.assertNotIn("林青禾", result.prompt.replace("```\n林青禾\n```", ""))

    def test_json_output_constraint_is_explicit(self):
        result = assemble_ingest_prompt(self.files, self.index, self.card_spec, self.masker)
        self.assertIn(JSON_OUTPUT_SPEC.strip(), result.prompt)
        self.assertIn("relative_path", result.prompt)
        self.assertIn("archive_moves", result.prompt)

    def test_multiple_files_keep_order(self):
        files = [
            {"relative_path": "inbox/a.md", "content": "甲材料"},
            {"relative_path": "inbox/b.md", "content": "乙材料"},
        ]
        result = assemble_ingest_prompt(files, self.index, self.card_spec, self.masker)
        self.assertLess(result.prompt.index("inbox/a.md"), result.prompt.index("inbox/b.md"))
        self.assertEqual([p for p, _ in result.masked_files], ["inbox/a.md", "inbox/b.md"])

    def test_same_entity_across_files_shares_one_placeholder(self):
        files = [
            {"relative_path": "inbox/a.md", "content": "林青禾 负责甲事项。"},
            {"relative_path": "inbox/b.md", "content": "林青禾 负责乙事项。"},
        ]
        result = assemble_ingest_prompt(files, self.index, self.card_spec, self.masker)
        placeholders = re.findall(r"\[PERSON_[0-9A-F]{8}\]", result.prompt)
        self.assertEqual(len(placeholders), 2)
        self.assertEqual(placeholders[0], placeholders[1])

    def test_empty_index_gets_placeholder(self):
        result = assemble_ingest_prompt(self.files, "", self.card_spec, self.masker)
        self.assertIn("（索引为空）", result.prompt)

    def test_cli_returns_prompt_without_mapping(self):
        payload = {
            "files": self.files,
            "index": self.index,
            "card_spec": self.card_spec,
        }
        proc = subprocess.run(
            [sys.executable, "-m", "omnidesk_engine.knowledge_ingest"],
            cwd=str(H.REPO / "engine"),
            input=json.dumps(payload, ensure_ascii=False),
            capture_output=True,
            text=True,
            timeout=20,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        data = json.loads(proc.stdout)
        self.assertIn("prompt", data)
        self.assertNotIn("mappings", data)
        self.assertEqual(data["files"][0]["relative_path"], "inbox/合成材料.md")


if __name__ == "__main__":
    unittest.main()
