#!/usr/bin/env python3
"""OD-ARCH-142 S4：脱敏统计与回程还原提示的静态断言。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
TYPES = REPO / "desktop/src/types.ts"
HOME = REPO / "desktop/src/shell/HomeChat.tsx"


def main() -> int:
    types = TYPES.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    cases = [
        ("types_masker_stats", "by_type: Record<string, number>" in types and "privacy_masked" in types),
        ("notice_reads_masker_counts", "masked?.masker?.by_type" in home),
        ("notice_labels_entities", 'PERSON: "人名"' in home and 'BRANCH: "机构"' in home),
        ("unmask_event_handled", 'p.kind === "masker_unmask"' in home),
        ("warning_banner", 'data-testid="masker-warning"' in home),
        ("warning_only_on_not_ok", "report.ok === false" in home),
        ("warning_dismissible", 'aria-label="关闭脱敏还原提示"' in home),
    ]
    failed = [name for name, ok in cases if not ok]
    for name, ok in cases:
        print(("PASS  " if ok else "FAIL  ") + name)
    print("%d/%d 通过" % (len(cases) - len(failed), len(cases)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

