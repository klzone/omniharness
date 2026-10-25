#!/usr/bin/env python3
"""OD-ARCH-082 S6: Agent preset 条目序列比对校验脚本。"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
UPSTREAM_PRESET = (
    REPO / "runtime" / "deepseek-harness" / "packages" / "bundle" / "web-app" / "presets" / "standard.patch.yml"
)
BASE_PATCH = REPO / "samples" / "profiles" / "omnidesk-base.profile.patch.yml"


def extract_top_plugins(text: str, expected_indent: int) -> list[str]:
    lines = text.splitlines()
    ids = []
    in_plugins = False
    for line in lines:
        if "plugins:" in line:
            in_plugins = True
            continue
        if in_plugins:
            indent = len(line) - len(line.lstrip())
            if indent == expected_indent and line.lstrip().startswith("- id:"):
                ids.append(line.strip().split(":")[1].strip())
    return ids


def extract_all_plugins(text: str) -> list[str]:
    lines = text.splitlines()
    ids = []
    in_plugins = False
    for line in lines:
        if "plugins:" in line:
            in_plugins = True
            continue
        if in_plugins and "- id:" in line:
            ids.append(line.strip().split(":")[1].strip())
    return ids


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases = []
    def check(name: str, ok: bool, detail: str = ""):
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    check("upstream_exists", UPSTREAM_PRESET.is_file(), str(UPSTREAM_PRESET))
    check("base_exists", BASE_PATCH.is_file(), str(BASE_PATCH))

    if not UPSTREAM_PRESET.is_file() or not BASE_PATCH.is_file():
        print(json.dumps({"cases": len(cases), "failed": 1, "result": "fail"}))
        return 1

    up_top = extract_top_plugins(UPSTREAM_PRESET.read_text(encoding="utf-8"), 10)
    base_top = extract_top_plugins(BASE_PATCH.read_text(encoding="utf-8"), 6)

    up_all = extract_all_plugins(UPSTREAM_PRESET.read_text(encoding="utf-8"))
    base_all = extract_all_plugins(BASE_PATCH.read_text(encoding="utf-8"))
    base_text = BASE_PATCH.read_text(encoding="utf-8")

    check("upstream_top_count_21", len(up_top) == 21, str(len(up_top)))
    check("base_top_count_matches", len(base_top) == len(up_top), f"{len(base_top)} vs {len(up_top)}")
    check("top_sequence_matches_upstream", base_top == up_top, f"{base_top} vs {up_top}")

    check("upstream_all_count_31", len(up_all) == 31, str(len(up_all)))
    check("base_all_count_matches", len(base_all) == len(up_all), f"{len(base_all)} vs {len(up_all)}")
    check("all_sequence_matches_upstream", base_all == up_all, f"{base_all} vs {up_all}")

    check("skill_filesystem_in_list", "skill-filesystem" in base_top, "")
    check("tool_skill_in_list", "tool-skill" in base_top, "")

    m_sf = re.search(r"- id: skill-filesystem[\s\S]*?disabled: true", base_text)
    check("skill_filesystem_disabled", m_sf is not None, "未找到 disabled: true")

    m_ts = re.search(r"- id: tool-skill[\s\S]*?disabled: true", base_text)
    check("tool_skill_disabled", m_ts is not None, "未找到 disabled: true")

    failed = [c for c in cases if not c["ok"]]
    payload = {
        "cases": len(cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "upstream_plugin_count": len(up_top),
        "base_plugin_count": len(base_top),
        "failed_cases": failed,
    }

    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result")}, ensure_ascii=False))
    for c in failed:
        print("FAIL", c["name"], c["detail"])
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

