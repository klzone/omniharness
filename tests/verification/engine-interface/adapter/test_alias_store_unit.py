#!/usr/bin/env python3
"""OD-ARCH-185 稳定别名表单测（全部虚构数据）。"""

from __future__ import annotations

import json
import os
import stat
import tempfile
import unittest
from pathlib import Path

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.security.alias_store import AliasStore
from omnidesk_engine.security.masker import DataMasker, MaskMapping


class AliasStoreTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.path = Path(self.tmp.name) / "map.json"

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_same_entity_keeps_same_alias_across_sessions(self) -> None:
        text = "与张瑞山谈话了解情况。"
        first = DataMasker(alias_store=AliasStore(self.path)).mask(text)
        self.assertIn("张**", first.text)
        second = DataMasker(alias_store=AliasStore(self.path)).mask(text)
        self.assertEqual(first.text, second.text, "跨实例必须稳定")
        self.assertEqual(first.stats.by_type.get("NAME"), 1)

    def test_two_people_get_distinct_aliases(self) -> None:
        text = "名单：张瑞山、李汶桦。"
        result = DataMasker(alias_store=AliasStore(self.path)).mask(text)
        aliases = [alias for alias, _ in AliasStore(self.path).aliases()]
        self.assertEqual(len(aliases), 2)
        self.assertEqual(len(set(aliases)), 2)
        self.assertNotEqual(result.text.count("张**"), 0)

    def test_unmask_restores_document_alias_without_session_mapping(self) -> None:
        masked = DataMasker(alias_store=AliasStore(self.path)).mask(
            "与张瑞山谈话。"
        ).text
        restored, report = DataMasker(alias_store=AliasStore(self.path)).unmask(
            masked, MaskMapping.empty()
        )
        self.assertIn("张瑞山", restored)
        self.assertGreaterEqual(report.get("alias_restored", 0), 1)

    def test_amount_is_not_persisted(self) -> None:
        DataMasker(alias_store=AliasStore(self.path)).mask(
            "与张瑞山谈话；涉及金额1000万元。"
        )
        data = json.loads(self.path.read_text(encoding="utf-8"))
        self.assertEqual(
            [e for e in data["entries"] if e["type"] == "AMOUNT"], [],
            "金额不持久化，避免映射表失控",
        )

    @unittest.skipIf(os.name == "nt", "Windows 不保证 POSIX 权限位")
    def test_file_permissions_are_owner_only(self) -> None:
        DataMasker(alias_store=AliasStore(self.path)).mask("与张瑞山谈话。")
        mode = stat.S_IMODE(self.path.stat().st_mode)
        self.assertEqual(mode, 0o600)

    def test_corrupt_map_is_tolerated(self) -> None:
        self.path.write_text("{not json", encoding="utf-8")
        store = AliasStore(self.path)
        result = DataMasker(alias_store=store).mask("与张瑞山谈话。")
        self.assertIn("张", result.text)
        self.assertEqual(len(store.aliases()), 1, "损坏文件应重建而不是丢实体")


if __name__ == "__main__":
    unittest.main()
