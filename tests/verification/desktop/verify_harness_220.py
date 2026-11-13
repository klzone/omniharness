#!/usr/bin/env python3
"""OD-HAR-221/222/231/232 静态与可运行检查。"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
LIB = ROOT / "desktop/src-tauri/src/lib.rs"
EVENT = ROOT / "desktop/src-tauri/src/event_log.rs"
INTEGRITY = ROOT / "desktop/src-tauri/src/integrity.rs"
API = ROOT / "desktop/src/api.ts"
SETTINGS = ROOT / "desktop/src/SettingsPanel.tsx"
GOLDEN = ROOT / "tests/golden_tasks/work-review"

cases: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    cases.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    lib = LIB.read_text(encoding="utf-8")
    check("integrity_module", "pub mod integrity;" in lib)
    check("event_command_registered", "event_log_recent," in lib)
    check("integrity_command_registered", "work_integrity_check," in lib)

    event = EVENT.read_text(encoding="utf-8")
    check("event_corrupt_skip", "skipped_invalid" in event)
    check("event_metadata_only", "detail: Option<String>" in event or "pub detail" in event)

    integrity = INTEGRITY.read_text(encoding="utf-8")
    check("integrity_read_only", "fn check(" in integrity and "write" not in integrity.split("pub fn check")[0])
    check("integrity_caps", "MAX_ISSUES" in integrity)

    check("decision_detail", 'Some("confirmed")' in lib and 'Some("rejected")' in lib)

    api = API.read_text(encoding="utf-8")
    check("api_event_log", "eventLogRecent" in api and '"event_log_recent"' in api)
    check("api_integrity", "workIntegrityCheck" in api and '"work_integrity_check"' in api)

    settings = SETTINGS.read_text(encoding="utf-8")
    check("settings_handoff_panel", "交接与体检" in settings and "loadHandoffDiagnostics" in settings)

    check("golden_cases_12", len(json.loads((GOLDEN / "cases.json").read_text(encoding="utf-8"))["cases"]) == 12)
    score = subprocess.run([sys.executable, str(GOLDEN / "score_work_review.py")], capture_output=True, text=True, timeout=30)
    scored = json.loads(score.stdout) if score.stdout else {}
    check("golden_score_pass", score.returncode == 0 and scored.get("result") == "pass", score.stderr[-120:])
    measure = subprocess.run(
        [sys.executable, str(GOLDEN / "measure_work_review.py"), "--fixtures"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    measured = json.loads(measure.stdout) if measure.stdout else {}
    check("measure_runs", measure.returncode == 0 and bool(measured.get("parts")), measure.stderr[-120:])

    failed = [case for case in cases if not case["ok"]]
    print(json.dumps({"cases": len(cases), "failed": len(failed), "result": "pass" if not failed else "fail", "failures": failed}, ensure_ascii=False))
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
