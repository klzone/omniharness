#!/usr/bin/env python3
"""OD-ARCH-139 DataMasker 纯单元测试（全部虚构数据）。

最小断言数：Dict 6 / Rule 6 / Placeholder 5 / Mask 6 / Unmask 6 / Session 3。
运行目录：tests/verification/engine-interface/adapter
分步：python3 -m unittest -v test_security_masker_unit -k Dict
收口：python3 -m unittest -v test_security_masker_unit
"""

from __future__ import annotations

import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.security import DataMasker, DictEntry, load_dictionary
from omnidesk_engine.security import masker as masker_module

SAMPLE = H.REPO / "config" / "sensitive-dict.sample.v1.json"


def write_temp(payload, suffix: str = ".json") -> Path:
    handle = tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=suffix, delete=False)
    with handle:
        if isinstance(payload, str):
            handle.write(payload)
        else:
            json.dump(payload, handle, ensure_ascii=False, indent=2)
    return Path(handle.name)


def id_check_digit(prefix17: str) -> str:
    weights = (7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2)
    total = sum(int(d) * w for d, w in zip(prefix17, weights))
    return "10X98765432"[total % 11]


def valid_id(sequence: int = 55, date: str = "19900307") -> str:
    prefix = "999999" + date + ("%03d" % sequence)
    return prefix + id_check_digit(prefix)


def luhn_ok(value: str) -> bool:
    total = 0
    for index, char in enumerate(reversed(value)):
        digit = int(char)
        if index % 2 == 1:
            digit = digit * 2 - 9 if digit * 2 > 9 else digit * 2
        total += digit
    return total % 10 == 0


def luhn_complete(prefix: str) -> str:
    for digit in "0123456789":
        candidate = prefix + digit
        if luhn_ok(candidate):
            return candidate
    raise AssertionError("无法构造 Luhn 样例")


class DictTests(unittest.TestCase):
    def test_sample_dictionary_loads(self):
        loaded = load_dictionary(SAMPLE)
        self.assertTrue(loaded.ok, loaded.warn)
        self.assertGreaterEqual(len(loaded.entries), 4)
        self.assertTrue(all(e.type in ("PERSON", "BRANCH", "ORG", "CUSTOM") for e in loaded.entries))

    def test_schema_mismatch_rejected(self):
        path = write_temp({"schema": "wrong.schema", "entries": []})
        loaded = load_dictionary(path)
        self.assertFalse(loaded.ok)
        self.assertEqual(loaded.entries, ())
        self.assertIn("schema", loaded.warn)

    def test_duplicate_value_type_merges_aliases(self):
        path = write_temp({
            "schema": "omnidesk.sensitive-dict.v1",
            "entries": [
                {"type": "PERSON", "value": "林青禾", "aliases": ["林青禾同志"]},
                {"type": "PERSON", "value": "林青禾", "aliases": ["小林"]},
            ],
        })
        loaded = load_dictionary(path)
        self.assertTrue(loaded.ok, loaded.warn)
        self.assertEqual(len(loaded.entries), 1)
        self.assertEqual(set(loaded.entries[0].terms), {"林青禾", "林青禾同志", "小林"})

    def test_invalid_type_reports_line_number(self):
        raw = """{
  "schema": "omnidesk.sensitive-dict.v1",
  "entries": [
    {"type": "PERSON", "value": "林青禾"},
    {"type": "OPS", "value": "秘密项目"}
  ]
}
"""
        path = write_temp(raw)
        loaded = load_dictionary(path)
        self.assertFalse(loaded.ok)
        self.assertIn("行", loaded.warn)
        self.assertIn("5", loaded.warn)
        self.assertIn("OPS", loaded.warn)

    def test_missing_file_degrades_with_warning(self):
        missing = Path(tempfile.gettempdir()) / "od139-no-such-dict.json"
        try:
            missing.unlink()
        except FileNotFoundError:
            pass
        masker = DataMasker(dictionary_path=missing)
        result = masker.mask("林青禾")
        self.assertEqual(result.text, "林青禾")
        self.assertTrue(result.stats.warn)
        self.assertIn("不存在", result.stats.warn[0])

    def test_entry_terms_dedupe_value_and_aliases(self):
        entry = DictEntry("PERSON", "林青禾", ("林青禾", "小林", "小林"))
        self.assertEqual(entry.terms, ("林青禾", "小林"))
        with self.assertRaises(ValueError):
            DictEntry("OPS", "秘密项目")


class RuleTests(unittest.TestCase):
    def test_idcard_checksum_valid_and_invalid(self):
        good = valid_id()
        bad = None
        for digit in "0123456789":
            candidate = good[:-1] + digit
            if candidate != good and not luhn_ok(candidate):
                bad = candidate
                break
        self.assertIsNotNone(bad, "应能构造校验码错误且非 Luhn 的 18 位样例")
        masker = DataMasker()
        masked = masker.mask("证件 " + good)
        self.assertIn("[IDCARD_", masked.text)
        self.assertNotIn(good, masked.text)
        invalid = masker.mask("证件 " + bad)
        self.assertNotIn("[IDCARD_", invalid.text)
        self.assertIn(bad, invalid.text)

    def test_luhn_valid_and_invalid_bankcard(self):
        good = luhn_complete("622202000000000")
        bad = good[:-1] + ("0" if good[-1] != "0" else "1")
        self.assertFalse(luhn_ok(bad))
        masker = DataMasker()
        masked = masker.mask("卡号 " + good)
        self.assertIn("[BANKCARD_", masked.text)
        invalid = masker.mask("卡号 " + bad)
        self.assertNotIn("[BANKCARD_", invalid.text)
        self.assertIn(bad, invalid.text)

    def test_phone_boundaries(self):
        masker = DataMasker()
        hit = masker.mask("电话 13800138000")
        self.assertIn("[PHONE_", hit.text)
        for value in ("138001380001", "913800138000"):
            miss = masker.mask("电话 " + value)
            self.assertNotIn("[PHONE_", miss.text)
            self.assertIn(value, miss.text)

    def test_email_length_and_shape(self):
        masker = DataMasker()
        hit = masker.mask("邮箱 lin.qinghe@example.test")
        self.assertIn("[EMAIL_", hit.text)
        too_long = "a" * 250 + "@example.test"
        miss = masker.mask("邮箱 " + too_long)
        self.assertNotIn("[EMAIL_", miss.text)
        no_dot = masker.mask("邮箱 lin@example")
        self.assertNotIn("[EMAIL_", no_dot.text)

    def test_idcard_priority_over_bankcard(self):
        both = None
        for sequence in range(1000):
            candidate = valid_id(sequence)
            if candidate[-1].isdigit() and luhn_ok(candidate):
                both = candidate
                break
        self.assertIsNotNone(both, "应能构造同时满足 GB11643 与 Luhn 的虚构样例")
        masked = DataMasker().mask("号码 " + both)
        self.assertIn("[IDCARD_", masked.text)
        self.assertNotIn("[BANKCARD_", masked.text)

    def test_invalid_18_digits_not_idcard(self):
        bad = "999999199003079999"
        masked = DataMasker().mask(bad)
        self.assertNotIn("[IDCARD_", masked.text)


PLACEHOLDER_RE = re.compile(
    r"\[(?:IDCARD|BANKCARD|PHONE|EMAIL|PERSON|BRANCH|ORG|CUSTOM)_[0-9A-F]{8}\]"
)


class PlaceholderTests(unittest.TestCase):
    def test_same_text_uses_same_placeholder(self):
        result = DataMasker(dictionary_path=SAMPLE).mask("林青禾 与 林青禾")
        found = PLACEHOLDER_RE.findall(result.text)
        self.assertEqual(len(found), 2)
        self.assertEqual(found[0], found[1])
        self.assertEqual(len(result.mapping), 1)

    def test_independent_masks_get_distinct_placeholders(self):
        with mock.patch.object(masker_module.secrets, "token_hex", side_effect=["11111111", "22222222"]):
            one = DataMasker(dictionary_path=SAMPLE).mask("林青禾")
            two = DataMasker(dictionary_path=SAMPLE).mask("林青禾")
        self.assertIn("[PERSON_11111111]", one.text)
        self.assertIn("[PERSON_22222222]", two.text)
        self.assertNotEqual(one.text, two.text)

    def test_repr_hides_original_and_is_count_only(self):
        result = DataMasker(dictionary_path=SAMPLE).mask("林青禾")
        self.assertEqual(repr(result.mapping), "<MaskMapping n=1>")
        self.assertNotIn("林青禾", repr(result.mapping))
        self.assertFalse(hasattr(result.mapping, "to_dict"))

    def test_placeholder_format_matches_design(self):
        result = DataMasker(dictionary_path=SAMPLE).mask("林青禾")
        match = PLACEHOLDER_RE.search(result.text)
        self.assertIsNotNone(match)
        self.assertTrue(PLACEHOLDER_RE.fullmatch(match.group(0)))

    def test_collision_retries_until_unused(self):
        entries = [DictEntry("PERSON", "林青禾"), DictEntry("PERSON", "周岚")]
        with mock.patch.object(
            masker_module.secrets,
            "token_hex",
            side_effect=["DEADBEEF", "DEADBEEF", "CAFEBABE"],
        ):
            result = DataMasker(entries=entries).mask("林青禾 周岚")
        self.assertIn("[PERSON_DEADBEEF]", result.text)
        self.assertIn("[PERSON_CAFEBABE]", result.text)
        self.assertEqual(len(result.mapping), 2)


class MaskTests(unittest.TestCase):
    def test_overlap_longest_wins(self):
        entries = [DictEntry("PERSON", "张三"), DictEntry("PERSON", "张三丰")]
        result = DataMasker(entries=entries).mask("张三丰")
        self.assertEqual(len(PLACEHOLDER_RE.findall(result.text)), 1)
        self.assertEqual(len(result.mapping), 1)
        self.assertGreaterEqual(result.stats.overlap_dropped, 1)
        self.assertIn("[PERSON_", result.text)

    def test_same_name_same_type_single_placeholder(self):
        result = DataMasker(dictionary_path=SAMPLE).mask("林青禾、林青禾")
        found = PLACEHOLDER_RE.findall(result.text)
        self.assertEqual(len(found), 2)
        self.assertEqual(found[0], found[1])
        self.assertEqual(result.stats.by_type.get("PERSON"), 2)

    def test_same_name_different_type_distinct_placeholders(self):
        entries = [
            DictEntry("PERSON", "晨星"),
            DictEntry("CUSTOM", "晨星项目"),
        ]
        result = DataMasker(entries=entries).mask("晨星项目 和 晨星")
        found = PLACEHOLDER_RE.findall(result.text)
        self.assertEqual(len(found), 2)
        self.assertNotEqual(found[0], found[1])
        self.assertEqual(len(result.mapping), 2)
        self.assertIn("[CUSTOM_", result.text)
        self.assertIn("[PERSON_", result.text)

    def test_empty_none_and_long_text_are_safe(self):
        masker = DataMasker(dictionary_path=SAMPLE)
        empty = masker.mask("")
        self.assertEqual(empty.text, "")
        self.assertEqual(len(empty.mapping), 0)
        none = masker.mask(None)
        self.assertEqual(none.text, "")
        long_text = "前" + ("a" * 1000000) + " 13800138000"
        result = masker.mask(long_text)
        self.assertIn("[PHONE_", result.text)
        self.assertNotIn("13800138000", result.text)

    def test_dictionary_and_rules_stack(self):
        result = DataMasker(dictionary_path=SAMPLE).mask(
            "林青禾 13800138000 lin.qinghe@example.test"
        )
        self.assertIn("[PERSON_", result.text)
        self.assertIn("[PHONE_", result.text)
        self.assertIn("[EMAIL_", result.text)
        self.assertEqual(len(result.mapping), 3)

    def test_masked_text_does_not_keep_originals(self):
        result = DataMasker(dictionary_path=SAMPLE).mask("林青禾 13800138000")
        self.assertNotIn("林青禾", result.text)
        self.assertNotIn("13800138000", result.text)
        self.assertEqual(result.stats.replaced, 2)


class UnmaskTests(unittest.TestCase):
    def test_roundtrip_exact(self):
        original = "林青禾 13800138000 lin.qinghe@example.test"
        masked = DataMasker(dictionary_path=SAMPLE).mask(original)
        restored, report = DataMasker().unmask(masked.text, masked.mapping)
        self.assertEqual(restored, original)
        self.assertTrue(report["ok"])
        self.assertEqual(report["restored"], 3)
        self.assertEqual(report["unknown_placeholders"], [])

    def test_unknown_placeholder_preserved(self):
        text = "前 [PHONE_00000000] 后"
        restored, report = DataMasker().unmask(text, masker_module.MaskMapping.empty())
        self.assertEqual(restored, text)
        self.assertFalse(report["ok"])
        self.assertEqual(report["unknown_placeholders"], ["[PHONE_00000000]"])
        self.assertEqual(report["modified_candidates"], 0)

    def test_modified_placeholder_preserved(self):
        masked = DataMasker().mask("13800138000")
        modified = masked.text.replace("]", "X]", 1)
        restored, report = DataMasker().unmask(modified, masked.mapping)
        self.assertEqual(restored, modified)
        self.assertEqual(report["modified_candidates"], 1)
        self.assertEqual(report["unknown_placeholders"], [])

    def test_cross_mapping_restores_nothing(self):
        first = DataMasker(dictionary_path=SAMPLE).mask("林青禾")
        second = DataMasker(dictionary_path=SAMPLE).mask("周岚")
        restored, report = DataMasker().unmask(first.text, second.mapping)
        self.assertEqual(restored, first.text)
        self.assertFalse(report["ok"])
        self.assertEqual(len(report["unknown_placeholders"]), 1)

    def test_validate_mapping_does_not_change_text(self):
        masked = DataMasker(dictionary_path=SAMPLE).mask("林青禾 13800138000")
        before = masked.text
        report = DataMasker().validate_mapping(masked.text, masked.mapping)
        self.assertEqual(masked.text, before)
        self.assertTrue(report["ok"])
        self.assertEqual(report["missing_in_mapping"], 0)
        self.assertEqual(report["stats"]["total_placeholders"], 2)

    def test_type_mismatch_is_unknown(self):
        mapping = masker_module.MaskMapping(
            {"[PHONE_12345678]": masker_module.MappingEntry("EMAIL", "x@example.test")}
        )
        restored, report = DataMasker().unmask("[PHONE_12345678]", mapping)
        self.assertEqual(restored, "[PHONE_12345678]")
        self.assertFalse(report["ok"])
        self.assertEqual(report["unknown_placeholders"], ["[PHONE_12345678]"])


class SessionTests(unittest.TestCase):
    def test_same_scope_reuses_placeholder(self):
        scope = masker_module.SessionScope()
        with mock.patch.object(masker_module.secrets, "token_hex", side_effect=["AAAAAAAA"]):
            first = DataMasker(dictionary_path=SAMPLE).mask("林青禾", session=scope)
            second = DataMasker(dictionary_path=SAMPLE).mask("林青禾", session=scope)
        self.assertIn("[PERSON_AAAAAAAA]", first.text)
        self.assertIn("[PERSON_AAAAAAAA]", second.text)

    def test_different_scopes_are_isolated(self):
        one, two = masker_module.SessionScope(), masker_module.SessionScope()
        with mock.patch.object(
            masker_module.secrets, "token_hex", side_effect=["AAAAAAAA", "BBBBBBBB"]
        ):
            first = DataMasker(dictionary_path=SAMPLE).mask("林青禾", session=one)
            second = DataMasker(dictionary_path=SAMPLE).mask("林青禾", session=two)
        self.assertIn("[PERSON_AAAAAAAA]", first.text)
        self.assertIn("[PERSON_BBBBBBBB]", second.text)
        self.assertNotEqual(first.text, second.text)

    def test_scope_and_request_mapping_do_not_pollute(self):
        scope = masker_module.SessionScope()
        with mock.patch.object(
            masker_module.secrets, "token_hex", side_effect=["AAAAAAAA", "BBBBBBBB"]
        ):
            first = DataMasker(dictionary_path=SAMPLE).mask("林青禾", session=scope)
            second = DataMasker(dictionary_path=SAMPLE).mask("周岚", session=scope)
        self.assertEqual(len(first.mapping), 1)
        self.assertEqual(len(second.mapping), 1)
        self.assertIn("[PERSON_AAAAAAAA]", first.text)
        self.assertIn("[PERSON_BBBBBBBB]", second.text)
        restored, report = DataMasker().unmask(second.text, second.mapping)
        self.assertEqual(restored, "周岚")
        self.assertTrue(report["ok"])
        leaked, leaked_report = DataMasker().unmask(second.text, first.mapping)
        self.assertEqual(leaked, second.text)
        self.assertFalse(leaked_report["ok"])


if __name__ == "__main__":
    unittest.main()
