#!/usr/bin/env python3
"""OmniDesk 聚合诊断：一次收集进程、端口、网络、模型配置和任务快照。

用途：替代“反复 get_task_status / ps / 端口 / 路由 / 配置”的多轮散查。
本脚本只输出元数据和脱敏后的 endpoint host，不输出任务正文、凭证、提示词
或日志正文。

预算（用户 2026-11-10 P1）：
  * 普通诊断最多 8 个工具轮次。
  * 同一证据只读一次；重读必须有 seq/mtime/pid/port 变化。
  * 本脚本作为第一轮聚合证据；不足时再定向补查，不做无新事实的轮询。

用法：
  python3 scripts/diagnose-omnidesk.py
  python3 scripts/diagnose-omnidesk.py --json
  python3 scripts/diagnose-omnidesk.py --app-root ~/OmniDesk/app --latest 3
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlparse

REDACT_KEYS = ("api-key", "api_key", "apikey", "token", "secret", "password")
SENSITIVE_TASK_KEYS = {
    "content",
    "text",
    "prompt",
    "messages",
    "error_message",
    "workspace_dir",
    "intent",
}


def run(command: list[str], timeout: float = 5.0) -> str:
    try:
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (FileNotFoundError, PermissionError, subprocess.TimeoutExpired, OSError):
        return ""
    return (result.stdout or "") + (result.stderr or "")


def redact_command(line: str) -> str:
    out = line
    for key in REDACT_KEYS:
        out = re.sub(
            rf"--{key}(?:=|\s+)([^\s]+)",
            f"--{key}=<redacted>",
            out,
            flags=re.IGNORECASE,
        )
    return out.rstrip()


def safe_url_host(value: str) -> str | None:
    try:
        parsed = urlparse(value.strip().strip("\"'"))
    except ValueError:
        return None
    if not parsed.scheme or not parsed.hostname:
        return None
    return f"{parsed.scheme}://{parsed.hostname}"


def process_snapshot() -> list[dict[str, object]]:
    output = run(["ps", "-axo", "pid,ppid,etime,command"])
    if not output:
        return []
    rows: list[dict[str, object]] = []
    needles = ("omnidesk", "dsh", "node", "omnidesk_engine", "serve.py")
    for line in output.splitlines():
        if not any(n in line.lower() for n in needles):
            continue
        parts = line.strip().split(None, 3)
        if len(parts) < 4:
            continue
        pid, ppid, etime, command = parts
        if not pid.isdigit():
            continue
        rows.append(
            {
                "pid": int(pid),
                "ppid": ppid,
                "etime": etime,
                "command": redact_command(command)[:240],
            }
        )
    return rows[:30]


def port_snapshot() -> list[str]:
    if shutil.which("lsof"):
        output = run(["lsof", "-nP", "-iTCP", "-sTCP:LISTEN"])
        if output:
            lines = []
            for line in output.splitlines():
                if any(n in line.lower() for n in ("node", "omnidesk", "dsh", "python")):
                    lines.append(redact_command(line)[:240])
            return lines[:30]
    if shutil.which("ss"):
        output = run(["ss", "-ltnp"])
        if output:
            return [redact_command(line)[:240] for line in output.splitlines()[:30]]
    return []


def network_snapshot() -> dict[str, object]:
    info: dict[str, object] = {"os": platform.system()}
    if platform.system() == "Darwin":
        route = run(["route", "-n", "get", "default"])
        interface = None
        for line in route.splitlines():
            if "interface:" in line:
                interface = line.split(":", 1)[1].strip()
                break
        info["default_interface"] = interface
        nwi = run(["scutil", "--nwi"], timeout=4)
        reachable = []
        for line in nwi.splitlines():
            if "reachable" in line.lower() or "interface" in line.lower():
                reachable.append(line.strip())
        info["network_info"] = reachable[:12]
    else:
        route = run(["ip", "route", "show", "default"], timeout=4)
        interface = None
        if route:
            match = re.search(r"dev\s+(\S+)", route)
            if match:
                interface = match.group(1)
        info["default_interface"] = interface
    return info


def model_snapshot(app_root: Path) -> dict[str, object]:
    info: dict[str, object] = {}
    settings_path = app_root / "config" / "settings.v1.json"
    if settings_path.is_file():
        try:
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            settings = {}
        info["engine_profile"] = settings.get("engine_profile")
        info["model_permissions"] = settings.get("model_permissions")
    model_path = app_root / "config" / "omnidesk-model.yml"
    hosts = []
    efforts = []
    model_ids = []
    if model_path.is_file():
        try:
            lines = model_path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            lines = []
        for line in lines:
            effort_match = re.search(r"(reasoning_effort|reasoningEffort):\s*(\S+)", line)
            if effort_match:
                efforts.append(effort_match.group(2).strip("\"'"))
            model_match = re.search(r"\bid:\s*(\S+)", line)
            if model_match:
                model_ids.append(model_match.group(1).strip("\"'"))
            if not re.search(r"(base_url|endpoint|host|url):", line, flags=re.IGNORECASE):
                continue
            value = line.split(":", 1)[1].strip()
            host = safe_url_host(value)
            if host:
                hosts.append(host)
    info["endpoint_hosts"] = sorted(set(hosts))
    info["reasoning_efforts"] = sorted(set(efforts))
    info["model_ids"] = sorted(set(model_ids))
    return info


def task_snapshot(app_root: Path, limit: int) -> list[dict[str, object]]:
    state_dir = app_root / "engine-state"
    if not state_dir.is_dir():
        return []
    files = sorted(
        (p for p in state_dir.glob("omni-*.json") if p.is_file()),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )[:limit]
    now = time.time()
    rows: list[dict[str, object]] = []
    for path in files:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if not isinstance(data, dict):
            continue
        persisted = data.get("persisted_at")
        age = None
        if isinstance(persisted, (int, float)):
            age = round(now - float(persisted), 1)
        row = {
            "file": path.name,
            "omni_task_id": data.get("omni_task_id"),
            "state": data.get("state"),
            "seq": data.get("seq"),
            "turn_end_seq": data.get("turn_end_seq"),
            "pending_approvals": data.get("pending_approvals"),
            "error_code": data.get("error_code"),
            "has_error_message": bool(data.get("error_message")),
            "snapshot_age_seconds": age,
        }
        if data.get("state") in {"queued", "starting", "running"} and age and age > 30:
            row["observer_may_be_stale"] = True
        rows.append(row)
    return rows


def log_snapshot(app_root: Path) -> list[dict[str, object]]:
    logs = app_root / "logs"
    if not logs.is_dir():
        return []
    rows: list[dict[str, object]] = []
    for name in ("engine-stderr.log", "dsh-settings.log", "profile-assets.log"):
        path = logs / name
        if not path.is_file():
            continue
        try:
            stat = path.stat()
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        error_lines = sum(
            1
            for line in text.splitlines()
            if re.search(r"error|failed|timeout|panic|unavailable", line, flags=re.IGNORECASE)
        )
        rows.append(
            {
                "file": name,
                "size_bytes": stat.st_size,
                "mtime": datetime.fromtimestamp(stat.st_mtime, tz=timezone.utc).isoformat(),
                "error_keyword_lines": error_lines,
            }
        )
    return rows


def web_snapshot(app_root: Path) -> dict[str, object]:
    """Search-provider evidence: takeover lines from engine stderr + profile bundles.

    Metadata only: provider ids and bundle names, never task bodies or keys.
    """
    info: dict[str, object] = {}
    engine_profile = model_snapshot(app_root).get("engine_profile")
    info["engine_profile"] = engine_profile

    stderr_path = app_root / "logs" / "engine-stderr.log"
    takeover_lines: list[str] = []
    provider_effective: str | None = None
    if stderr_path.is_file():
        try:
            text = stderr_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            text = ""
        lines = text.splitlines()[-500:]
        for line in lines:
            if "free-search" not in line:
                continue
            takeover_lines.append(line.strip()[:160])
            match = re.search(r"taking over .*?as \"([^\"]+)\"", line)
            if match:
                provider_effective = match.group(1)
            served = re.search(r"search is served by provider \"([^\"]+)\"", line)
            if served:
                provider_effective = served.group(1)
    info["free_search_takeover_lines"] = takeover_lines[-5:]
    info["search_provider_effective"] = provider_effective

    bundles: list[str] = []
    if isinstance(engine_profile, str) and engine_profile:
        candidates = [
            app_root.parent / "runtime" / "dsh-home" / "profiles" / engine_profile / "package.json",
            app_root / "runtime" / "dsh-home" / "profiles" / engine_profile / "package.json",
        ]
        manifest = next((c for c in candidates if c.is_file()), None)
        if manifest is not None:
            try:
                data = json.loads(manifest.read_text(encoding="utf-8"))
                bundles = list(((data.get("dsh") or {}).get("profile") or {}).get("bundles", []))
            except (OSError, ValueError):
                bundles = []
    info["profile_bundles"] = bundles
    return info


def build_report(app_root: Path, latest: int) -> dict[str, object]:
    return {
        "budget": {
            "tool_rounds_max": 8,
            "evidence_read_once": True,
            "reread_requires": "seq / mtime / pid / port change",
            "note": "Run this once; then only targeted follow-ups with new evidence.",
        },
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "environment": {
            "os": platform.system(),
            "os_release": platform.release(),
            "python": sys.version.split()[0],
            "app_root": str(app_root),
            "repo_root": str(Path(__file__).resolve().parents[1]),
        },
        "processes": process_snapshot(),
        "ports": port_snapshot(),
        "network": network_snapshot(),
        "model": model_snapshot(app_root),
        "web": web_snapshot(app_root),
        "tasks": task_snapshot(app_root, latest),
        "logs": log_snapshot(app_root),
    }


def print_text(report: dict[str, object]) -> None:
    print("OmniDesk diagnostic (one aggregated read)")
    budget = report["budget"]
    print(
        f"budget: max {budget['tool_rounds_max']} tool rounds; "
        "re-read only when seq/mtime/pid/port changes"
    )
    env = report["environment"]
    print(f"\n== environment ==\n{env['os']} {env['os_release']} | python {env['python']}")
    print(f"app_root: {env['app_root']}")
    print(f"generated_at: {report['generated_at']}")

    print("\n== processes ==")
    for row in report["processes"]:
        print(f"pid={row['pid']} ppid={row['ppid']} etime={row['etime']} {row['command']}")
    if not report["processes"]:
        print("(none matched)")

    print("\n== listening ports ==")
    for line in report["ports"]:
        print(line)
    if not report["ports"]:
        print("(none matched)")

    print("\n== network ==")
    net = report["network"]
    print(f"default_interface: {net.get('default_interface')}")
    for line in net.get("network_info", []):
        print(line)

    print("\n== model ==")
    for key, value in report["model"].items():
        print(f"{key}: {value}")

    print("\n== latest task snapshots ==")
    for row in report["tasks"]:
        print(
            f"{row['file']}: state={row['state']} seq={row['seq']} "
            f"age={row['snapshot_age_seconds']}s pending={row['pending_approvals']} "
            f"error={row['error_code']}"
            + (" OBSERVER_STALE?" if row.get("observer_may_be_stale") else "")
        )
    if not report["tasks"]:
        print("(none)")

    print("\n== log metadata ==")
    for row in report["logs"]:
        print(
            f"{row['file']}: {row['size_bytes']} bytes, "
            f"error-keyword lines={row['error_keyword_lines']}, mtime={row['mtime']}"
        )
    if not report["logs"]:
        print("(none)")

    print("\n== web search provider ==")
    web = report["web"]
    print(f"engine_profile: {web.get('engine_profile')}")
    print(f"search_provider_effective: {web.get('search_provider_effective')}")
    print(f"profile_bundles: {web.get('profile_bundles')}")
    for line in web.get("free_search_takeover_lines") or []:
        print(f"takeover: {line}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--app-root",
        type=Path,
        default=Path(os.environ.get("HOME", "~")).expanduser() / "OmniDesk" / "app",
    )
    ap.add_argument("--latest", type=int, default=3, help="最近任务快照数量")
    ap.add_argument("--json", action="store_true", help="输出 JSON")
    args = ap.parse_args()

    app_root = args.app_root.expanduser().resolve()
    report = build_report(app_root, max(1, min(args.latest, 20)))
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print_text(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
