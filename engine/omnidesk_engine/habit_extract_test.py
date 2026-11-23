#!/usr/bin/env python3
"""OD-GROW-301 G3 习惯提示词组装单测（全部合成数据）。"""

from __future__ import annotations

import json
import subprocess
import sys
import unittest

from omnidesk_engine.habit_extract import (
    HABIT_CANDIDATES_SCHEMA,
    SIGNAL_WORDS,
    UNCERTAIN_WORDS,
    build_habit_prompt,
)


class HabitPromptTests(unittest.TestCase):
    def test_prompt_contains_grounding_rules(self):
        prompt = build_habit_prompt(
            "【线程】合成记录\n- [m-1] 以后都用清单收尾。",
            [{"thread_id": "thread-1", "title": "合成记录"}],
        )
        self.assertIn("只保存能指到某条用户消息的内容", prompt)
        self.assertIn("不得根据话题、语气、使用的工具推断用户特质或偏好", prompt)
        self.assertIn("一次性请求不得变成长期偏好", prompt)
        self.assertIn("拿不准就不要", prompt)
        self.assertIn("不提取判断、假设、结论性观点", prompt)
        self.assertIn("用户转述别人的话、AI 建议而用户没采纳的，一律不算", prompt)

    def test_prompt_contains_output_contract(self):
        prompt = build_habit_prompt("- [m-1] 合成摘要", [])
        self.assertIn(HABIT_CANDIDATES_SCHEMA, prompt)
        for field in (
            '"items"',
            '"text"',
            '"kind"',
            '"scope_hint"',
            '"message_id"',
            '"cue"',
            '"check"',
            '"open_questions"',
        ):
            self.assertIn(field, prompt)
        self.assertIn("banned_phrases", prompt)
        self.assertIn("paragraph_max_chars", prompt)
        self.assertIn("required_phrases", prompt)
        self.assertIn("只输出一个纯 JSON 对象", prompt)

    def test_prompt_lists_signal_and_uncertain_words(self):
        prompt = build_habit_prompt("- [m-1] 合成摘要", [])
        for word in SIGNAL_WORDS:
            self.assertIn(word, prompt)
        for word in UNCERTAIN_WORDS:
            self.assertIn(word, prompt)
        self.assertIn("items 最多 10 条", prompt)
        self.assertIn("open_questions 最多 10 条", prompt)

    def test_cli_returns_prompt_json(self):
        payload = {
            "digest_text": "【线程】合成记录\n- [m-1] 以后都用清单收尾。",
            "threads_meta": [{"thread_id": "thread-1", "title": "合成记录"}],
        }
        proc = subprocess.run(
            [sys.executable, "-m", "omnidesk_engine.habit_extract"],
            input=json.dumps(payload, ensure_ascii=False),
            capture_output=True,
            text=True,
            timeout=20,
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        data = json.loads(proc.stdout)
        self.assertEqual(data["schema"], HABIT_CANDIDATES_SCHEMA)
        self.assertIn("thread_id=thread-1", data["prompt"])
        self.assertIn("以后都用清单收尾。", data["prompt"])


if __name__ == "__main__":
    unittest.main()
