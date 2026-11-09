#!/usr/bin/env python3
"""Deterministic DOCX authoring for OmniDesk writing export (OD-WT-613).

This script follows the office-docx skill: python-docx, font assignment on the
eastAsia axis, paragraph styles for headings, and a reopen check after saving.
It reads one JSON payload and writes one DOCX. It never calls a model, network,
or shell.
"""

import json
import sys

from docx import Document
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml.ns import qn
from docx.shared import Cm, Pt


def set_run_font(run, family: str, size_pt: float, bold: bool = False) -> None:
    run.font.name = family
    run.font.size = Pt(size_pt)
    run.font.bold = bold
    run._element.rPr.rFonts.set(qn("w:eastAsia"), family)


def set_paragraph_font(paragraph, family: str, size_pt: float, bold: bool = False) -> None:
    for run in paragraph.runs:
        set_run_font(run, family, size_pt, bold)
    paragraph.paragraph_format.line_spacing = 1.0


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: writing_docx_export.py payload.json output.docx", file=sys.stderr)
        return 2
    with open(sys.argv[1], "r", encoding="utf-8") as handle:
        payload = json.load(handle)
    style = payload["style"]
    blocks = payload["blocks"]

    body_family = style["body_font"]
    heading_family = style["heading_font"]
    body_size = float(style["body_size_pt"])
    line_height = float(style.get("line_height", 1.8))
    margin_cm = float(style.get("margin_cm", 2.6))
    heading_align = style.get("heading_align", "left")

    document = Document()
    section = document.sections[0]
    section.top_margin = Cm(margin_cm)
    section.bottom_margin = Cm(margin_cm)
    section.left_margin = Cm(margin_cm)
    section.right_margin = Cm(margin_cm)

    normal = document.styles["Normal"]
    normal.font.name = body_family
    normal.font.size = Pt(body_size)
    normal._element.rPr.rFonts.set(qn("w:eastAsia"), body_family)
    normal.paragraph_format.line_spacing = line_height

    align = {
        "center": WD_ALIGN_PARAGRAPH.CENTER,
        "left": WD_ALIGN_PARAGRAPH.LEFT,
        "right": WD_ALIGN_PARAGRAPH.RIGHT,
        "justify": WD_ALIGN_PARAGRAPH.JUSTIFY,
    }

    for block in blocks:
        kind = block["kind"]
        text = block.get("text", "")
        if kind == "h1":
            paragraph = document.add_paragraph()
            paragraph.alignment = align.get(heading_align, WD_ALIGN_PARAGRAPH.LEFT)
            run = paragraph.add_run(text)
            set_run_font(run, heading_family, body_size + 5, True)
            paragraph.paragraph_format.space_after = Pt(14)
            paragraph.paragraph_format.line_spacing = 1.2
        elif kind == "h2":
            paragraph = document.add_paragraph()
            paragraph.alignment = WD_ALIGN_PARAGRAPH.LEFT
            run = paragraph.add_run(text)
            set_run_font(run, heading_family, body_size + 2, True)
            paragraph.paragraph_format.space_before = Pt(14)
            paragraph.paragraph_format.space_after = Pt(8)
            paragraph.paragraph_format.line_spacing = 1.35
        elif kind == "quote":
            paragraph = document.add_paragraph()
            run = paragraph.add_run(text)
            set_run_font(run, body_family, body_size - 1)
            paragraph.paragraph_format.left_indent = Cm(1.0)
            paragraph.paragraph_format.right_indent = Cm(1.0)
            paragraph.paragraph_format.line_spacing = line_height
        elif kind == "bullet":
            paragraph = document.add_paragraph()
            run = paragraph.add_run("• " + text)
            set_run_font(run, body_family, body_size)
            paragraph.paragraph_format.left_indent = Cm(0.8)
            paragraph.paragraph_format.line_spacing = line_height
        else:
            paragraph = document.add_paragraph()
            lines = text.split("\n")
            for index, line in enumerate(lines):
                if index:
                    paragraph.add_run().add_break()
                run = paragraph.add_run(line)
                set_run_font(run, body_family, body_size)
            paragraph.paragraph_format.first_line_indent = Pt(body_size * 2)
            paragraph.paragraph_format.line_spacing = line_height
            paragraph.paragraph_format.space_after = Pt(0)

    document.save(sys.argv[2])

    reopened = Document(sys.argv[2])
    if len(reopened.paragraphs) < len(blocks):
        print("DOCX reopen check failed: paragraph count is smaller than blocks", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
