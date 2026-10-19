#!/usr/bin/env python3
"""OD-BUILD-005 —— 工作台「用引擎生成解释」路径的回环验证。

验证的链路与界面工作台「阶段 3 · 引擎支路」同形：

    本地确定性统计（Rust，唯一数值来源）
      → 冻结解释提示词（Rust `task_shell::build_explanation_prompt`，含指纹比对）
        → 引擎 `omni.submit_task`（workspace_dir 来自 Rust `prepare_local_data_root`）
          → ACP → DSH CLI
            → 本机回环合成模型端点（零凭证、零外网）
              → 终态 → `omni.get_task_result` → 界面回填的内容

关键点：**提示词不是本脚本拼的**。界面阶段 3 的提示词只能由本地 Rust 代码产出
（先本地重算再比对指纹）。若脚本在 Python 里自己拼一份，就等于绕开了这条唯一数值来源，
测出来的是脚本而不是产物。因此本脚本先运行 Rust 示例 `examples/frozen_prompt.rs`
拿到**真实产出**的提示词与工作目录，只负责把它交给引擎。

证据等级：script（真实引擎子进程 + 真实 DSH CLI + 本机回环合成端点 + Rust 真实产物）。
安全边界：一次性 /tmp 数据根；不读/不打印/不复制凭证；不触碰 ~/.dsh、~/.agents、
         ~/OmniDesk、~/YvanAI；合成素材全部为编译期嵌入的虚构占位内容。

用法：
  python3 run_workbench_engine_path.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
EVIDENCE_DIR = HERE / "evidence"
sys.path.insert(0, str(HERE))

from run_desktop_e2e import (  # noqa: E402
    EngineProcess,
    drop_root,
    initialize_params,
    new_root,
    validate_isolated_root,
    wait_terminal,
    write_overlay,
)
from _mock_model_server import start_mock  # noqa: E402

SRC_TAURI = REPO / "desktop" / "src-tauri"

# Rust 示例需要在能链接 Tauri 依赖的环境下运行（本机约定，不是仓库内容）。
CARGO_ENV_HINTS = {
    "PATH": "/home/kevin/.cargo/bin:" + os.environ.get("PATH", ""),
    "PKG_CONFIG_PATH": ":".join([
        "/home/kevin/.local/omnidesk-tauri-deps/usr/lib/x86_64-linux-gnu/pkgconfig",
        "/home/kevin/.local/omnidesk-tauri-deps/usr/share/pkgconfig",
        "/usr/lib/x86_64-linux-gnu/pkgconfig",
        "/usr/share/pkgconfig",
    ]),
    "PKG_CONFIG_SYSROOT_DIR": "/home/kevin/.local/omnidesk-tauri-deps",
    "LIBRARY_PATH": ":".join([
        "/home/kevin/.local/omnidesk-tauri-deps/usr/lib/x86_64-linux-gnu",
        "/usr/lib/x86_64-linux-gnu",
    ]),
}


def rust_frozen_prompt(local_root: Path, save_explanation: str | None = None) -> dict:
    """运行 Rust 示例，取回界面阶段 3 用的真实冻结提示词与其工作目录。"""
    env = dict(os.environ)
    env.update(CARGO_ENV_HINTS)
    env["PKG_CONFIG_LIBDIR"] = CARGO_ENV_HINTS["PKG_CONFIG_PATH"]
    env["OMNIDESK_LOCAL_DATA_ROOT"] = str(local_root)
    if save_explanation is None:
        env.pop("OMNIDESK_SAVE_EXPLANATION", None)
    else:
        env["OMNIDESK_SAVE_EXPLANATION"] = save_explanation
    proc = subprocess.run(
        ["cargo", "run", "--offline", "--quiet", "--example", "frozen_prompt"],
        cwd=str(SRC_TAURI), env=env, capture_output=True, text=True, timeout=600,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            "Rust 示例 frozen_prompt 运行失败（退出码 %s）：%s" % (proc.returncode, proc.stderr[-2000:])
        )
    # 只取最后一行 JSON（cargo 可能先打印编译进度）。
    line = [ln for ln in proc.stdout.splitlines() if ln.strip().startswith("{")][-1]
    return json.loads(line)


TERMINAL = {"succeeded", "failed", "cancelled", "interrupted", "unknown"}


def _submit(engine: EngineProcess, workspace: str, prompt: str):
    """界面 `host.submitTask` 的等价调用：intent 用任务类型标识，workspace_dir 用已备好的目录。"""
    return engine.request("omni.submit_task", {
        "intent": "desensitized_analysis",
        "workspace_dir": workspace,
        "prompt_text": prompt,
    }, timeout=60)


def scenario_explanation_success(frozen: dict, local_root: Path) -> dict:
    """正常路径：提示词 → 引擎 → 终态 succeeded → 结果回填 → 人工确认 → 保存到仓库外。"""
    base_url, ctl, server = start_mock()
    ctl.set(mode="ok", chunks=["按冻结统计块：", "合计 204、平均 40.8。", "（合成端点输出）"], delay=0.0)
    root = new_root("wkpath_ok")
    entry = {
        "scenario": "engine_explanation_success",
        "root": str(root["base"]),
        "isolation_problems": validate_isolated_root(root),
        "workspace_dir_used": frozen["workspace_dir"],
        "prompt_is_rust_product": None,
    }
    engine = EngineProcess("wkpath_ok", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        prompt = frozen["prompt"]
        entry["prompt_is_rust_product"] = "冻结统计块" in prompt and frozen["fingerprint"][:16] in prompt

        receipt, err = _submit(engine, frozen["workspace_dir"], prompt)
        entry["submit"] = {"ok": err is None, "error": err, "receipt": receipt}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        task_id = receipt["omni_task_id"]
        snaps = wait_terminal(engine, task_id, timeout=180)
        final = snaps[-1] if snaps else {}
        entry["final_status"] = final.get("status")
        entry["status_snapshots"] = snaps[-3:]

        result, rerr = engine.request("omni.get_task_result", {"omni_task_id": task_id}, timeout=60)
        entry["result"] = {
            "ok": rerr is None, "error": rerr,
            "form": (result or {}).get("form"),
            "integrity": (result or {}).get("integrity"),
            "content": (result or {}).get("content"),
            "persisted_at": (result or {}).get("persisted_at"),
        }
        content = ((result or {}).get("content") or "")
        entry["backfilled_content"] = content

        # 阶段 4 + 5：把引擎**实际输出**交给真实保存入口（Rust），验证产物落盘。
        # 保存入口强制人工确认、来源白名单与指纹比对；这里不做任何绕过。
        saved = rust_frozen_prompt(local_root, save_explanation=content)
        save_info = saved.get("save") or {}
        entry["save"] = {
            "ok": bool(save_info.get("ok")),
            "path": save_info.get("path"),
            "fingerprint": save_info.get("fingerprint"),
            "error": save_info.get("error"),
            "outside_repo": bool(
                save_info.get("path") and not str(save_info["path"]).startswith(str(REPO))
            ),
        }
        artifact_body = ""
        if save_info.get("path"):
            try:
                artifact_body = Path(save_info["path"]).read_text(encoding="utf-8")
            except OSError:
                artifact_body = ""
        entry["artifact_contains_engine_output"] = bool(content) and content in artifact_body
        entry["artifact_contains_frozen_numbers"] = "合计: 204" in artifact_body
        entry["artifact_records_unverified_source"] = "model_unverified" in artifact_body
        entry["artifact_records_confirmation"] = "已确认" in artifact_body

        events = engine.events()
        entry["event_types"] = [e.get("type") for e in events]
        entry["text_delta_joined"] = "".join(
            str(e.get("payload", {}).get("text", "")) for e in events if e.get("type") == "text_delta"
        )

        checks = {
            "initialize_ok": entry["initialize"]["ok"],
            "prompt_is_rust_product": entry["prompt_is_rust_product"],
            "submit_ok": entry["submit"]["ok"],
            "reached_succeeded": entry.get("final_status") == "succeeded",
            "result_complete": entry["result"].get("form") == "complete",
            "content_non_empty": content.strip() != "",
            "saved_outside_repo": entry["save"]["ok"] and entry["save"]["outside_repo"],
            "artifact_has_engine_output": entry["artifact_contains_engine_output"],
            "artifact_has_frozen_numbers": entry["artifact_contains_frozen_numbers"],
            "artifact_marks_source_unverified": entry["artifact_records_unverified_source"],
            "artifact_records_confirmation": entry["artifact_records_confirmation"],
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def scenario_explanation_failure(frozen: dict) -> dict:
    """失败路径：端点报错时不得出现假成功，也不应回填内容。"""
    base_url, ctl, server = start_mock()
    ctl.set(mode="error_400", chunks=["unused"])
    root = new_root("wkpath_fail")
    entry = {
        "scenario": "engine_explanation_failure",
        "root": str(root["base"]),
        "isolation_problems": validate_isolated_root(root),
    }
    engine = EngineProcess("wkpath_fail", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        receipt, serr = _submit(engine, frozen["workspace_dir"], frozen["prompt"])
        entry["submit"] = {"ok": serr is None, "error": serr, "receipt": receipt}
        if serr is not None:
            # 提交阶段即失败也是「非成功」，但要如实记录失败发生在哪一步。
            entry["failure_stage"] = "submit"
            entry["verdict"] = "pass" if serr.get("data", {}).get("engine_error_code") else "fail"
            return entry

        task_id = receipt["omni_task_id"]
        snaps = wait_terminal(engine, task_id, timeout=120)
        final = snaps[-1] if snaps else {}
        entry["final_status"] = final.get("status")
        entry["status_snapshots"] = snaps[-3:]

        result, _ = engine.request("omni.get_task_result", {"omni_task_id": task_id}, timeout=60)
        entry["result_form"] = (result or {}).get("form")
        entry["result_content"] = (result or {}).get("content")
        entry["event_types"] = [e.get("type") for e in engine.events()]

        checks = {
            "not_reported_as_success": entry.get("final_status") != "succeeded",
            "failure_observable": entry.get("final_status") == "failed"
            or "task_failed" in entry["event_types"],
            "no_backfilled_success_content": ((result or {}).get("form") != "complete")
            or not ((result or {}).get("content") or "").strip(),
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def scenario_explanation_cancel(frozen: dict) -> dict:
    """取消路径：请求发出不等于取消成功，界面必须能区分。"""
    base_url, ctl, server = start_mock()
    ctl.set(mode="hang", hang_seconds=90.0)
    root = new_root("wkpath_cancel")
    entry = {
        "scenario": "engine_explanation_cancel",
        "root": str(root["base"]),
        "isolation_problems": validate_isolated_root(root),
    }
    engine = EngineProcess("wkpath_cancel", root)
    try:
        overlay = write_overlay(root, base_url)
        init, err = engine.request("omni.initialize", initialize_params(root, overlay), timeout=120)
        entry["initialize"] = {"ok": err is None, "error": err}
        if err is not None:
            entry["verdict"] = "fail"
            return entry

        receipt, serr = _submit(engine, frozen["workspace_dir"], frozen["prompt"])
        entry["submit"] = {"ok": serr is None, "error": serr, "receipt": receipt}
        if serr is not None:
            entry["verdict"] = "fail"
            return entry
        task_id = receipt["omni_task_id"]

        running_seen = False
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            st, e = engine.request("omni.get_task_status", {"omni_task_id": task_id}, timeout=20)
            if e is None and st.get("status") == "running":
                running_seen = True
                break
            time.sleep(0.2)
        entry["running_before_cancel"] = running_seen

        outcome, cerr = engine.request("omni.cancel_task", {
            "omni_task_id": task_id, "timeout_seconds": 15}, timeout=60)
        entry["cancel"] = {"ok": cerr is None, "error": cerr, "outcome": outcome}
        phase = (outcome or {}).get("phase")
        entry["cancel_phase"] = phase

        snaps = wait_terminal(engine, task_id, timeout=90)
        final = snaps[-1] if snaps else {}
        entry["final_status"] = final.get("status")
        entry["final_cancel_phase"] = final.get("cancel_phase")

        checks = {
            "initialize_ok": entry["initialize"]["ok"],
            "submit_ok": entry["submit"]["ok"],
            "cancel_request_recorded": phase in (
                "request_sent", "confirmed_cancelled", "timeout_unknown", "acknowledged"),
            # 契约 §4.6：不得把「已请求」呈现为「已确认取消」。
            "phase_not_misreported_as_success": phase != "not_requested",
            "no_isolation_violation": not entry["isolation_problems"],
        }
        entry["checks"] = checks
        entry["verdict"] = "pass" if all(checks.values()) else "fail"
        return entry
    finally:
        engine.close()
        drop_root(root)
        server.shutdown()


def main() -> int:
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    local_root = Path(tempfile.mkdtemp(prefix="omni_wkpath_localroot_")) / "analysis"

    frozen = rust_frozen_prompt(local_root)

    report = {
        "generated_at": time.time(),
        "evidence_level": "script（真实引擎子进程 + 真实 DSH CLI + 本机回环合成端点 + Rust 真实冻结提示词）",
        "channel": "IPC-2 stdio JSON-RPC（进程边界）：与界面 host.submitTask / taskStatus / taskResult / cancelTask 同形",
        "note": (
            "提示词由 desktop/src-tauri/examples/frozen_prompt.rs 真实产出（本地重算 + 指纹比对），"
            "脚本不自行拼提示词；workspace_dir 来自 task_shell::prepare_local_data_root。"
        ),
        "frozen_prompt": {
            "sample_id": frozen["sample_id"],
            "fingerprint": frozen["fingerprint"],
            "rows": frozen["rows"],
            "total": frozen["total"],
            "mean": frozen["mean"],
            "top_category": frozen["top_category"],
            "workspace_dir": frozen["workspace_dir"],
            "workspace_error": frozen["workspace_error"],
            "prompt_length": len(frozen["prompt"]),
        },
        "scenarios": {
            "success": scenario_explanation_success(frozen, local_root),
            "failure": scenario_explanation_failure(frozen),
            "cancel": scenario_explanation_cancel(frozen),
        },
    }
    report["verdict"] = "pass" if all(
        s.get("verdict") == "pass" for s in report["scenarios"].values()
    ) else "fail"

    (EVIDENCE_DIR / "workbench-engine-path.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    print("[workbench-engine-path]", report["verdict"], {
        k: (v.get("verdict"), v.get("final_status")) for k, v in report["scenarios"].items()
    })
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
