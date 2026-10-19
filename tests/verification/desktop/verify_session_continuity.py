#!/usr/bin/env python3
"""OD-ARCH-017：会话发现、重开与重启后任务状态恢复的合成纵向验证。

只使用一次性 /tmp 数据根、本机回环合成模型端点与虚构文本：
  * 真实启动仓库内锁定的 DSH 引擎（公开 ACP），不使用真实凭证或外网；
  * 不触碰 ~/.dsh、~/.agents、~/OmniDesk 或任何真实会话；
  * 只通过 session/list 与 session/resume 取元数据，不读取会话正文；
  * 证据只记录计数、状态与哈希，不写路径正文或会话内容。

证据等级：script（真实引擎进程 + 合成模型端点）。

用法：
  python3 tests/verification/desktop/verify_session_continuity.py [--json PATH]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ADAPTER_DIR = REPO / "tests" / "verification" / "engine-interface" / "adapter"
sys.path.insert(0, str(REPO / "engine"))
sys.path.insert(0, str(ADAPTER_DIR))

import _harness as H  # noqa: E402
from _harness import safe_call  # noqa: E402
from _mock_model_server import start_mock  # noqa: E402

from omnidesk_engine import (  # noqa: E402
    EngineErrorCode,
    SubmitTaskRequest,
    TaskState,
)

TERMINAL = {
    TaskState.SUCCEEDED,
    TaskState.FAILED,
    TaskState.CANCELLED,
    TaskState.INTERRUPTED,
    TaskState.UNKNOWN,
}


def tree_fingerprint(root: Path) -> dict:
    """对隔离根下所有文件做内容哈希；只用于证明来源未被改写。"""
    digest = hashlib.sha256()
    count = 0
    if root.exists():
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            count += 1
            digest.update(path.relative_to(root).as_posix().encode("utf-8"))
            digest.update(b"\0")
            with path.open("rb") as fh:
                for chunk in iter(lambda: fh.read(1 << 20), b""):
                    digest.update(chunk)
            digest.update(b"\0")
    return {"files": count, "sha256": digest.hexdigest()}


def file_hashes(root: Path) -> dict[str, str]:
    """逐个文件的短哈希，用于说明变更只落在引擎自己的会话存储。"""
    out: dict[str, str] = {}
    if root.exists():
        for path in sorted(root.rglob("*")):
            if path.is_file():
                out[path.relative_to(root).as_posix()] = hashlib.sha256(
                    path.read_bytes()
                ).hexdigest()[:16]
    return out


def wait_terminal(adapter, task_id: str, timeout: float = 90.0) -> list:
    snaps = []
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status = adapter.get_task_status(task_id)
        snaps.append(status.to_dict())
        if status.status in TERMINAL:
            return snaps
        time.sleep(0.1)
    return snaps


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args()

    root = H.new_root("arch018")
    # 目录名与 intent 都放入哨兵：响应、界面和证据均不得出现。
    secret_cwd = Path(root["base"]) / "SYNTH-PRIVATE-PROJECT-NAME"
    secret_cwd.mkdir(parents=True, exist_ok=True)
    root["cwd"] = secret_cwd
    PRIVATE_SENTINELS = ["SYNTH-PRIVATE-PROJECT-NAME", str(Path(root["base"]).resolve())]
    INTENT_SENTINEL = "SYNTH-PRIVATE-INTENT-TEXT"
    checks: list[dict] = []
    artifacts: dict = {}

    def check(name: str, ok: bool, detail=None) -> None:
        entry = {"check": name, "pass": bool(ok)}
        if detail is not None:
            entry["detail"] = detail
        checks.append(entry)

    base_url, ctl, server = start_mock()
    overlay = H.write_overlay(root, "arch017", H.mock_overlay_text(base_url))
    adapter = H.make_adapter(
        root, [overlay], init_timeout=60, session_timeout=60, default_timeout=90
    )

    receipt = None
    session_id = None
    try:
        ok_init, desc = safe_call(adapter.initialize)
        check("initialize_ok", ok_init, desc if not ok_init else "ok")
        if not ok_init:
            raise SystemExit("engine initialize failed; cannot continue")

        ok, receipt = safe_call(
            adapter.submit_task,
            SubmitTaskRequest(
                intent=INTENT_SENTINEL,
                workspace_dir=str(root["cwd"]),
                prompt_text=H.SYNTH_PROMPT,
            ),
        )
        check("submit_ok", ok, receipt if not ok else "ok")
        if not ok:
            raise SystemExit("synthetic submit failed; cannot continue")
        session_id = receipt.engine_session_id
        check("session_id_captured", bool(session_id), bool(session_id))
        snaps = wait_terminal(adapter, receipt.omni_task_id)
        check(
            "first_run_reached_success",
            bool(snaps) and snaps[-1]["status"] == TaskState.SUCCEEDED.value,
            snaps[-1]["status"] if snaps else None,
        )
    finally:
        artifacts["first_shutdown"] = adapter.shutdown(
            mode="graceful", grace_period=5
        ).to_dict()
        server.shutdown()

    if receipt is None or session_id is None:
        payload = {
            "plan": "OD-ARCH-017",
            "evidence_level": "script (real engine + synthetic endpoint)",
            "checks": checks,
            "artifacts": artifacts,
            "result": "fail",
        }
        _write(args.json, payload)
        print("RESULT: fail (setup)")
        return 1

    omni_task_id = receipt.omni_task_id
    home_before = tree_fingerprint(root["dsh_home"])
    files_before = file_hashes(root["dsh_home"])

    # 重启：新适配层进程，同一隔离数据根；先只做只读列表。
    base_url2, ctl2, server2 = start_mock()
    overlay2 = H.write_overlay(root, "arch017b", H.mock_overlay_text(base_url2))
    adapter2 = H.make_adapter(
        root, [overlay2], init_timeout=60, session_timeout=60, default_timeout=60
    )
    try:
        ok_init2, desc2 = safe_call(adapter2.initialize)
        check("restart_initialize_ok", ok_init2, desc2 if not ok_init2 else "ok")

        ok_list, listing = safe_call(adapter2.list_sessions)
        artifacts["listing_error"] = None if ok_list else listing
        check("session_list_ok", ok_list, "ok" if ok_list else listing)
        if ok_list:
            artifacts["listing_meta"] = {
                "total": listing.total,
                "groups": len(listing.groups),
                "skipped_invalid": listing.skipped_invalid,
                "next_cursor_present": listing.next_cursor is not None,
            }
            ids = [
                s["engine_session_id"]
                for g in listing.groups
                for s in g["sessions"]
            ]
            check("listed_session_found", session_id in ids, {"total": listing.total})
            check(
                "listing_groups_have_no_absolute_path",
                all(
                    str(root["cwd"]) not in g["workspace_label"]
                    and g["workspace_label"] != os.path.basename(str(root["cwd"]))
                    for g in listing.groups
                ),
                [g["workspace_label"] for g in listing.groups],
            )
            check(
                "listing_has_no_message_body",
                all(
                    set(s.keys())
                    == {
                        "engine_session_id",
                        "workspace_label",
                        "workspace_token",
                        "created_at",
                        "workspace_exists",
                        "metadata_valid",
                        "resume_status",
                    }
                    for g in listing.groups
                    for s in g["sessions"]
                ),
                "fields are metadata only",
            )
            listing_json = json.dumps(listing.to_dict(), ensure_ascii=False)
            check(
                "listing_response_has_no_path_or_project_sentinel",
                all(s not in listing_json for s in PRIVATE_SENTINELS),
                "absolute path and project-name sentinels absent",
            )
            check(
                "listing_group_has_no_workspace_dir",
                all("workspace_dir" not in g for g in listing.groups),
                "no workspace_dir key on groups",
            )
            check(
                "listing_does_not_claim_resumable",
                all(
                    s["resume_status"] == "unverified"
                    for g in listing.groups
                    for s in g["sessions"]
                ),
                "resume_status=unverified before any resume call",
            )
        home_after_list = tree_fingerprint(root["dsh_home"])
        check(
            "list_is_read_only_on_source",
            home_after_list == home_before,
            {"before": home_before, "after": home_after_list},
        )
        files_after_list = file_hashes(root["dsh_home"])

        sent_before = H.method_counts(adapter2.raw_sent_frames())
        ok_resume, resumed = safe_call(adapter2.recover_session, session_id)
        artifacts["resume_error"] = None if ok_resume else resumed
        artifacts["resume_result"] = resumed.to_dict() if ok_resume else None
        check("resume_session_ok", ok_resume, "ok" if ok_resume else resumed)
        if ok_resume:
            check("resume_returns_same_session", resumed.engine_session_id == session_id, True)
            resume_json = json.dumps(resumed.to_dict(), ensure_ascii=False)
            check(
                "resume_response_has_no_absolute_cwd",
                all(s not in resume_json for s in PRIVATE_SENTINELS)
                and "cwd" not in resumed.to_dict(),
                "no cwd field, no path sentinel",
            )
        sent_after = H.method_counts(adapter2.raw_sent_frames())
        artifacts["sent_methods"] = sent_after
        check(
            "resume_does_not_create_new_session",
            sent_after.get("session/new", 0) == sent_before.get("session/new", 0),
            "session/new count unchanged",
        )
        check(
            "resume_does_not_replay_prompt",
            sent_after.get("session/prompt", 0) == sent_before.get("session/prompt", 0),
            "session/prompt count unchanged across resume",
        )

        ok_unknown, unknown = safe_call(
            adapter2.recover_session, "synthetic-session-does-not-exist"
        )
        artifacts["unknown_session_error"] = unknown
        check("unknown_session_rejected", not ok_unknown, bool(unknown))
        check(
            "unknown_session_maps_to_standard_code",
            (not ok_unknown)
            and unknown.get("engine_error_code")
            == EngineErrorCode.UNKNOWN_SESSION.value,
            {"code": unknown.get("engine_error_code") if not ok_unknown else None},
        )

        ok_task, task_rec = safe_call(adapter2.recover_task, omni_task_id)
        artifacts["task_recovery"] = task_rec.to_dict() if ok_task else task_rec
        check("recover_task_ok", ok_task, "ok" if ok_task else task_rec)
        if ok_task:
            check(
                "recover_task_terminal_known",
                task_rec.status.status == TaskState.SUCCEEDED,
                task_rec.status.status.value,
            )
            check("recover_task_no_replay", task_rec.replayed is False, task_rec.replayed)

        ok_ledger, ledger = safe_call(adapter2.list_task_records)
        artifacts["ledger_error"] = None if ok_ledger else ledger
        check("task_ledger_ok", ok_ledger, "ok" if ok_ledger else ledger)
        if ok_ledger:
            ids = [r.omni_task_id for r in ledger.records]
            check("ledger_contains_task", omni_task_id in ids, {"count": len(ids)})
            check(
                "ledger_is_metadata_only",
                all("content" not in r.to_dict() for r in ledger.records),
                "no content field in any record",
            )
            check(
                "ledger_does_not_return_intent",
                all("intent" not in r.to_dict() for r in ledger.records),
                "no intent field in any record",
            )
            check(
                "ledger_response_has_no_intent_sentinel",
                INTENT_SENTINEL
                not in json.dumps(ledger.to_dict(), ensure_ascii=False),
                "intent sentinel absent from response",
            )

        # 损坏台账：不得猜造状态。
        state_dir = Path(root["state"])
        bad = state_dir / "omni-synthetic-corrupt.json"
        bad.write_text("{not json", encoding="utf-8")
        ok_bad, ledger_bad = safe_call(adapter2.list_task_records)
        check(
            "corrupt_ledger_counted_not_fabricated",
            ok_bad and ledger_bad.skipped_invalid >= 1,
            {"skipped_invalid": ledger_bad.skipped_invalid if ok_bad else None},
        )
        ok_bad_task, bad_task = safe_call(
            adapter2.recover_task, "omni-synthetic-corrupt"
        )
        check("corrupt_task_not_reported_known", not ok_bad_task, bad_task)
        bad.unlink()

        # 中断任务：进程内没有终态证据时，必须落到 unknown/interrupted。
        interrupted_id = "omni-synthetic-interrupted"
        (state_dir / (interrupted_id + ".json")).write_text(
            json.dumps(
                {
                    "omni_task_id": interrupted_id,
                    "state": "running",
                    "intent": "synthetic-writing-task",
                    "created_at": time.time(),
                    "state_since": time.time(),
                    "engine_session_id": None,
                    "cancel_phase": "not_requested",
                    "content": "SYNTHETIC-CONTENT-MUST-NOT-LEAK",
                }
            ),
            encoding="utf-8",
        )
        ok_int, int_rec = safe_call(adapter2.recover_task, interrupted_id)
        artifacts["interrupted_recovery"] = int_rec.to_dict() if ok_int else int_rec
        check("interrupted_task_recovered", ok_int, "ok" if ok_int else int_rec)
        if ok_int:
            check(
                "interrupted_task_not_marked_succeeded",
                int_rec.status.status in (TaskState.UNKNOWN, TaskState.INTERRUPTED),
                int_rec.status.status.value,
            )
            check("interrupted_task_no_replay", int_rec.replayed is False, True)
        ok_int_ledger, int_ledger = safe_call(adapter2.list_task_records)
        if ok_int_ledger:
            check(
                "ledger_does_not_leak_content",
                "SYNTHETIC-CONTENT-MUST-NOT-LEAK"
                not in json.dumps(int_ledger.to_dict(), ensure_ascii=False),
                "synthetic content sentinel absent",
            )

        # 重开是引擎自己的原生写入：DSH 会在其会话存储里追加恢复记录。
        # 这不是 OmniDesk 改写用户数据，但必须如实记录，不能声称“零写入”。
        files_after_resume = file_hashes(root["dsh_home"])
        changed = sorted(
            k
            for k in set(files_after_list) | set(files_after_resume)
            if files_after_list.get(k) != files_after_resume.get(k)
        )
        artifacts["resume_changed_file_kinds"] = sorted(
            {
                "session-store"
                if k.startswith("sessions/")
                else "session-projection-cache"
                if k.startswith("storages/session_projcache/")
                else "other"
                for k in changed
            }
        )
        artifacts["resume_changed_file_count"] = len(changed)
        check(
            "resume_writes_only_engine_session_store",
            all(
                k.startswith("sessions/") or k.startswith("storages/session_projcache/")
                for k in changed
            ),
            {"changed": len(changed)},
        )
        check(
            "resume_did_not_touch_other_dsh_home_files",
            all(files_after_list.get(k) == v for k, v in files_after_resume.items() if k not in changed)
            and all(k in files_after_resume for k in files_after_list if k not in changed),
            "non-session files unchanged",
        )
        home_after = tree_fingerprint(root["dsh_home"])
        check(
            "source_hash_matches_recorded_deltas",
            home_after["files"] == home_before["files"],
            {"files_before": home_before["files"], "files_after": home_after["files"]},
        )
    finally:
        artifacts["second_shutdown"] = adapter2.shutdown(
            mode="graceful", grace_period=5
        ).to_dict()
        server2.shutdown()
        H.drop_root(root)

    failed = [c for c in checks if not c["pass"]]
    payload = {
        "plan": "OD-ARCH-017",
        "evidence_level": "script (real engine process + loopback synthetic model endpoint)",
        "checks_total": len(checks),
        "checks_failed": len(failed),
        "checks": checks,
        "artifacts": artifacts,
        "result": "pass" if checks and not failed else "fail",
    }
    _write(args.json, payload)
    print(
        "checks: %d total, %d failed; RESULT: %s"
        % (len(checks), len(failed), payload["result"])
    )
    for c in failed:
        print("  FAIL:", c["check"], json.dumps(c.get("detail"), ensure_ascii=False)[:400])
    return 0 if payload["result"] == "pass" else 1


def _write(path: Path | None, payload: dict) -> None:
    if not path:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
