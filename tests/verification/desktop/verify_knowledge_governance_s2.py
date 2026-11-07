#!/usr/bin/env python3
"""OD-KB-201 S2：治理记录接线、脱敏与 AI 无写权静态断言。"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
GOVERNANCE = REPO / "desktop/src-tauri/src/knowledge_governance.rs"
INGEST = REPO / "desktop/src-tauri/src/knowledge_ingest.rs"
LIB = REPO / "desktop/src-tauri/src/lib.rs"
VAULT = REPO / "desktop/src-tauri/src/knowledge_vault.rs"
API = REPO / "desktop/src/api.ts"
ENGINE = REPO / "engine/omnidesk_engine"

CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    governance = GOVERNANCE.read_text(encoding="utf-8") if GOVERNANCE.is_file() else ""
    ingest = INGEST.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    vault = VAULT.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    engine_text = "\n".join(
        p.read_text(encoding="utf-8", errors="replace")
        for p in ENGINE.rglob("*.py")
        if "__pycache__" not in p.parts
    )

    check("governance_module_exists", GOVERNANCE.is_file())
    check("governance_dir_is_vault_relative", 'pub const GOVERNANCE_DIR: &str = "governance"' in governance)
    for constant in ("RAW_ADMISSIONS_FILE", "REVIEW_DECISIONS_FILE", "ADMISSION_RULES_FILE"):
        check("file_" + constant.lower(), constant in governance)
    check("append_and_list_present", "pub fn append_record" in governance and "pub fn list_records" in governance)
    check("append_uses_atomic_vault_write", "vault_write::save_note" in governance and "vault_write::create_note" in governance)
    check("append_masks_privacy", "mask_privacy" in governance and "masked_field" in governance)
    check("append_only_record_kind_writer", "append_record_at" in governance)

    check("raw_admit_loads_policy", "current_rule_policy(&state)" in lib)
    check("raw_admit_uses_masked_policy", "admit_raw_files_with_policy" in ingest)
    check("raw_admit_writes_governance", "KIND_RAW_ADMISSIONS" in ingest and "append_record_at" in ingest)

    handler = lib[lib.find("invoke_handler(tauri::generate_handler![") :]
    check("governance_list_registered", "knowledge_governance_list," in handler)
    forbidden = ["knowledge_governance_append", "knowledge_governance_write", "knowledge_governance_save"]
    check("no_governance_write_command", all(name not in lib for name in forbidden))
    check("api_has_no_governance_write", all(name not in api for name in forbidden))
    check("engine_has_no_governance_write", all(name not in engine_text for name in forbidden))
    check("engine_has_no_governance_path", "governance/" not in engine_text)

    check("scan_skips_governance", 'name == crate::knowledge_governance::GOVERNANCE_DIR' in vault and '"governance"' in vault)

    failed = [c for c in CASES if not c["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"], (" " + case["detail"]) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
