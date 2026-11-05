#!/usr/bin/env python3
"""OD-ARCH-140 DataMasker Task 2（Markdown 结构感知）单元测试。

最小断言数：Region 12 / MaskMd 10 / UnmaskMd 8 / Edge 7。
全部数据虚构；不读真实知识库、不启动 DSH、不联网。
"""

from __future__ import annotations

import time
import unittest

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine.security import DataMasker, SessionScope
from omnidesk_engine.security.masker import scan_markdown, scan_markdown_regions

SAMPLE = H.REPO / "config" / "sensitive-dict.sample.v1.json"


def id_check_digit(prefix17: str) -> str:
    weights = (7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2)
    total = sum(int(d) * w for d, w in zip(prefix17, weights))
    return "10X98765432"[total % 11]


def valid_id(sequence: int = 55) -> str:
    prefix = "99999919900307" + ("%03d" % sequence)
    return prefix + id_check_digit(prefix)


def masker() -> DataMasker:
    return DataMasker(dictionary_path=SAMPLE)


def roundtrip(case: unittest.TestCase, original: str) -> None:
    masked = masker().mask_markdown(original)
    restored, report = DataMasker().unmask_markdown(masked.text, masked.mapping)
    case.assertEqual(restored, original)
    case.assertTrue(report["ok"], report)


class RegionTests(unittest.TestCase):
    def test_plain_text_single_region(self):
        self.assertEqual(scan_markdown_regions("abc"), [(0, 3, "text")])

    def test_empty_text_has_no_regions(self):
        self.assertEqual(scan_markdown_regions(""), [])

    def test_frontmatter_region_starts_at_zero(self):
        text = "---\ntitle: 合成\nauthor: 林青禾\n---\n\n正文\n"
        regions = scan_markdown_regions(text)
        self.assertEqual(regions[0][0], 0)
        self.assertEqual(regions[0][2], "frontmatter")
        self.assertTrue(text[regions[0][1]:].startswith("\n正文"))

    def test_unclosed_frontmatter_warns_and_falls_back_to_text(self):
        text = "---\ntitle: 未闭合\n正文 林青禾"
        scan = scan_markdown(text)
        self.assertIn("frontmatter_unclosed", scan.warnings)
        self.assertEqual([(0, len(text), "text")], list(scan.regions))

    def test_frontmatter_must_start_at_file_start(self):
        text = "\n---\ntitle: 合成\n---\n"
        scan = scan_markdown(text)
        self.assertNotIn("frontmatter", [kind for _, _, kind in scan.regions])

    def test_fenced_code_is_one_region(self):
        text = "前\n```py\nx = 1\n```\n后"
        regions = scan_markdown_regions(text)
        code = [r for r in regions if r[2] == "code"]
        self.assertEqual(len(code), 1)
        self.assertEqual(text[code[0][0]:code[0][1]], "```py\nx = 1\n```\n")

    def test_nested_fence_uses_longer_marker(self):
        text = "````\n```\n内部\n```\n````\n"
        regions = scan_markdown_regions(text)
        code = [r for r in regions if r[2] == "code"]
        self.assertEqual(code, [(0, len(text), "code")])

    def test_inline_code_region(self):
        text = "a `code` b"
        regions = scan_markdown_regions(text)
        inline = [r for r in regions if r[2] == "inline_code"]
        self.assertEqual(inline, [(2, 8, "inline_code")])

    def test_wikilink_target_only(self):
        text = "[[目标页]]"
        self.assertEqual(scan_markdown_regions(text), [(0, len(text), "wikilink_target")])

    def test_wikilink_display_split(self):
        text = "[[目标页|显示名]]"
        regions = scan_markdown_regions(text)
        display = [r for r in regions if r[2] == "wikilink_display"]
        self.assertEqual(display, [(6, 9, "wikilink_display")])
        self.assertEqual(text[6:9], "显示名")

    def test_embed_is_whole_target(self):
        text = "![[目标页|显示名]]"
        self.assertEqual(scan_markdown_regions(text), [(0, len(text), "wikilink_target")])

    def test_regions_cover_text_without_overlap(self):
        text = "---\nauthor: 林青禾\n---\n正文 `代码` [[页|显示]]\n```\ncode\n```\n尾"
        regions = scan_markdown_regions(text)
        self.assertEqual(regions[0][0], 0)
        self.assertEqual(regions[-1][1], len(text))
        for before, after in zip(regions, regions[1:]):
            self.assertEqual(before[1], after[0])
        for start, end, kind in regions:
            self.assertIn(kind, {"frontmatter", "code", "inline_code", "wikilink_target", "wikilink_display", "text"})
            self.assertLessEqual(start, end)

    def test_pure_code_document(self):
        text = "```\n林青禾 13800138000\n```\n"
        self.assertEqual(scan_markdown_regions(text), [(0, len(text), "code")])


class MaskMdTests(unittest.TestCase):
    def test_plain_text_structure_preserved(self):
        original = "| 列一 | 列二 |\n| --- | --- |\n| 林青禾 | 13800138000 |\n\n  缩进保留\n<!-- 注释 -->\n"
        masked = masker().mask_markdown(original)
        self.assertIn("| 列一 | 列二 |", masked.text)
        self.assertIn("\n\n  缩进保留", masked.text)
        self.assertIn("<!-- 注释 -->", masked.text)
        restored, report = DataMasker().unmask_markdown(masked.text, masked.mapping)
        self.assertEqual(restored, original)
        self.assertTrue(report["ok"])

    def test_code_block_not_masked(self):
        ident = valid_id()
        original = "正文 林青禾\n```\n留号 %s 林青禾\n```\n尾 林青禾" % ident
        masked = masker().mask_markdown(original)
        self.assertIn(ident, masked.text)
        self.assertIn("留号 %s 林青禾" % ident, masked.text)
        self.assertNotIn("尾 林青禾", masked.text)

    def test_inline_code_not_masked(self):
        original = "正文 林青禾 与 `林青禾 13800138000` 结束"
        masked = masker().mask_markdown(original)
        self.assertIn("`林青禾 13800138000`", masked.text)
        self.assertNotIn("正文 林青禾", masked.text)

    def test_frontmatter_name_masked(self):
        original = "---\nauthor: 林青禾\n---\n正文"
        masked = masker().mask_markdown(original)
        self.assertTrue(masked.text.startswith("---\n"))
        self.assertIn("author: [PERSON_", masked.text)
        self.assertIn("\n---\n正文", masked.text)

    def test_wikilink_target_preserved_display_masked(self):
        original = "[[目标页|林青禾]]"
        masked = masker().mask_markdown(original)
        self.assertTrue(masked.text.startswith("[[目标页|"))
        self.assertTrue(masked.text.endswith("]]"))
        self.assertIn("[PERSON_", masked.text)
        self.assertNotIn("|林青禾]]", masked.text)

    def test_same_entity_across_regions_same_placeholder(self):
        original = "---\nauthor: 林青禾\n---\n正文 林青禾"
        masked = masker().mask_markdown(original)
        placeholders = [part for part in masked.text.split() if part.startswith("[PERSON_")]
        self.assertEqual(len(placeholders), 2)
        self.assertEqual(placeholders[0].rstrip(","), placeholders[1])

    def test_session_passthrough(self):
        scope = SessionScope()
        first = masker().mask_markdown("林青禾", session=scope)
        second = masker().mask_markdown("林青禾", session=scope)
        self.assertEqual(first.text, second.text)
        self.assertIn("[PERSON_", first.text)

    def test_tags_and_urls_not_masked(self):
        original = "#林青禾 https://example.test/林青禾/13800138000 正文 林青禾 13800138000"
        masked = masker().mask_markdown(original)
        self.assertIn("#林青禾", masked.text)
        self.assertIn("https://example.test/林青禾/13800138000", masked.text)
        self.assertNotIn("正文 林青禾 13800138000", masked.text)

    def test_frontmatter_fences_keep_original_position(self):
        original = "---\nsources: [林青禾]\n---\n正文"
        masked = masker().mask_markdown(original)
        self.assertTrue(masked.text.startswith("---\n"))
        self.assertIn("\n---\n正文", masked.text)

    def test_complex_structure_roundtrip(self):
        original = "---\nauthor: 林青禾\n---\n# 标题\n\n正文 13800138000 与 [[页|周岚]]\n\n```\n林青禾\n```\n\n#标签 https://example.test/a"
        roundtrip(self, original)


class UnmaskMdTests(unittest.TestCase):
    def test_roundtrip_complex(self):
        original = "---\nauthor: 林青禾\n---\n正文 [[页|周岚]] 13800138000\n`代码`\n"
        roundtrip(self, original)

    def test_cross_mapping_is_rejected(self):
        first = masker().mask_markdown("林青禾")
        second = masker().mask_markdown("周岚")
        restored, report = DataMasker().unmask_markdown(first.text, second.mapping)
        self.assertEqual(restored, first.text)
        self.assertFalse(report["ok"])

    def test_modified_placeholder_preserved(self):
        masked = masker().mask_markdown("林青禾")
        modified = masked.text.replace("]", "X]", 1)
        restored, report = DataMasker().unmask_markdown(modified, masked.mapping)
        self.assertEqual(restored, modified)
        self.assertGreaterEqual(report["modified_candidates"], 1)

    def test_report_fields_match_task1(self):
        masked = masker().mask_markdown("林青禾 13800138000")
        _, report = DataMasker().unmask_markdown(masked.text, masked.mapping)
        self.assertEqual(
            set(report),
            {"restored", "unknown_placeholders", "modified_candidates", "ok"},
        )

    def test_placeholder_moved_into_code_still_restores(self):
        masked = masker().mask_markdown("林青禾")
        moved = "```\n%s\n```\n" % masked.text
        restored, report = DataMasker().unmask_markdown(moved, masked.mapping)
        self.assertEqual(restored, "```\n林青禾\n```\n")
        self.assertTrue(report["ok"])

    def test_unknown_placeholder_preserved(self):
        text = "[PERSON_00000000]"
        restored, report = DataMasker().unmask_markdown(text, masker().mask_markdown("").mapping)
        self.assertEqual(restored, text)
        self.assertFalse(report["ok"])

    def test_empty_document(self):
        masked = masker().mask_markdown("")
        restored, report = DataMasker().unmask_markdown(masked.text, masked.mapping)
        self.assertEqual(restored, "")
        self.assertTrue(report["ok"])

    def test_unknown_and_modified_counts_are_separate(self):
        text = "[PERSON_00000000] [PERSON_DEADBEEFX]"
        _, report = DataMasker().unmask_markdown(text, masker().mask_markdown("").mapping)
        self.assertEqual(len(report["unknown_placeholders"]), 1)
        self.assertEqual(report["modified_candidates"], 1)


class EdgeTests(unittest.TestCase):
    def test_unclosed_fence_rest_is_code(self):
        text = "前 林青禾\n```\n林青禾 13800138000"
        scan = scan_markdown(text)
        self.assertIn("code_unclosed", scan.warnings)
        masked = masker().mask_markdown(text)
        self.assertIn("```\n林青禾 13800138000", masked.text)
        self.assertNotIn("前 林青禾", masked.text)

    def test_unclosed_inline_code_is_single_line(self):
        text = "前 `林青禾 13800138000\n后 林青禾"
        scan = scan_markdown(text)
        self.assertIn("inline_code_unclosed", scan.warnings)
        masked = masker().mask_markdown(text)
        self.assertIn("`林青禾 13800138000\n", masked.text)
        self.assertNotIn("后 林青禾", masked.text)

    def test_thousand_wikilinks(self):
        text = " ".join("[[页%d]]" % i for i in range(1000))
        regions = scan_markdown_regions(text)
        self.assertGreaterEqual(len(regions), 1000)
        masked = masker().mask_markdown(text)
        self.assertEqual(masked.text, text)

    def test_one_megabyte_document_under_five_seconds(self):
        text = "正文 " + ("a" * 1000000) + " 林青禾 13800138000"
        started = time.monotonic()
        masked = masker().mask_markdown(text)
        self.assertLess(time.monotonic() - started, 5.0)
        self.assertNotIn("13800138000", masked.text)

    def test_nested_backticks_do_not_crash(self):
        text = "````\n```\n林青禾\n```\n````\n"
        regions = scan_markdown_regions(text)
        self.assertEqual([r[2] for r in regions], ["code"])

    def test_weird_document_still_covers_all(self):
        text = "\ufeff---\nnot front\n---\n`x` [[a|b]] #tag https://x.test/a\n"
        regions = scan_markdown_regions(text)
        self.assertEqual(regions[0][0], 0)
        self.assertEqual(regions[-1][1], len(text))

    def test_unclosed_frontmatter_is_plain_text(self):
        text = "---\ntitle: 合成\n林青禾 13800138000"
        masked = masker().mask_markdown(text)
        self.assertIn("[PERSON_", masked.text)
        self.assertIn("[PHONE_", masked.text)


if __name__ == "__main__":
    unittest.main()
