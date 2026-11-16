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

OD-ARCH-173 阶段 0：追加只读会话解析（--sessions）。只输出字符数、耗时、
次数与 token 元数据；系统提示词按段落归类后只报每类字符数，不输出任何
提示词、用户消息、工具参数或任务正文。
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
from typing import Mapping
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


# ---------------------------------------------------------------------------
# OD-ARCH-173 阶段 0：DSH 会话（session.v4.jsonl.zstd）只读元数据解析
# ---------------------------------------------------------------------------

SESSION_PROMPT_BUCKETS = (
    "base",
    "persona",
    "web-search",
    "ptc-runtime",
    "tool-guidance",
    "harness-guidance",
    "discipline-rules",
    "other",
)


def default_session_root(app_root: Path) -> Path:
    """默认会话根：<OmniDesk>/runtime/dsh-home/sessions（app_root 的上一级）。"""
    return app_root.parent / "runtime" / "dsh-home" / "sessions"


def decode_session_file(path: Path) -> str | None:
    """解压 session.v4.jsonl.zstd；只接受 .zstd 或 .jsonl 文件。"""
    try:
        if path.suffix == ".jsonl":
            return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    zstd = shutil.which("zstd")
    if not zstd:
        return None
    try:
        result = subprocess.run(
            [zstd, "-dc", str(path)],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode != 0:
        return None
    return result.stdout


def classify_prompt_paragraph(paragraph: str) -> str:
    """按系统提示词的稳定起头归类；未知段落计入 other。"""
    head = paragraph.lstrip().splitlines()[0].strip() if paragraph.strip() else ""
    if head.startswith("You are an AI agent powered by DeepSeek Harness"):
        return "base"
    if head.startswith("You are a coding agent powered by"):
        return "persona"
    if head.startswith("你是 OmniDesk") or "个人工作台账助手" in head:
        return "persona"
    if head.startswith("## Web search"):
        return "web-search"
    if (
        head.startswith("interface Tool")
        or head.startswith("`run_code`")
        or head.startswith("## Writing code for run_code")
        or head.startswith("Inside the program")
        or head.startswith("- Call tools as `await tools.name(args)`")
        or head.startswith("Program-only SDK bindings")
        or head.startswith("```ts")
        or head.startswith("type ToolName = keyof ToolOutputMap")
        or head.startswith("declare class ToolCallError")
        or head.startswith("declare const tools:")
    ):
        return "ptc-runtime"
    if head.startswith(("检索收敛规则", "工具批处理规则", "检索预算规则", "工具选择规则")):
        return "discipline-rules"
    if (
        "DeepSeek Harness Web GUI" in paragraph
        or "runtime context names the DeepSeek Harness" in paragraph
    ):
        return "harness-guidance"
    if head.startswith(
        (
            "Tokens prefixed with @",
            "Use the ",
            "Read an ",
            "Read a file before",
            "Check the [exit code:",
            "web_search results",
            "web_fetch returns",
        )
    ):
        return "tool-guidance"
    return "other"


def system_prompt_segments(text: str) -> dict[str, object]:
    """系统提示词分段字符数（只报分类与字符数，不保留原文）。"""
    buckets: dict[str, int] = {name: 0 for name in SESSION_PROMPT_BUCKETS}
    segments = 0
    start = 0
    length = len(text)
    while start <= length:
        end = text.find("\n\n", start)
        para = text[start:] if end == -1 else text[start:end]
        if para.strip():
            segments += 1
            buckets[classify_prompt_paragraph(para)] += len(para)
        if end == -1:
            break
        start = end + 2
    return {"chars": length, "segments": segments, "buckets": buckets}


def _indexed_name(value: object, depth: int = 0) -> str | None:
    """从事件 data 里取工具名（只返回名字，不取参数或正文）。"""
    if depth > 3 or not isinstance(value, Mapping):
        return None
    for key in ("tool", "toolName", "name", "tool_name"):
        candidate = value.get(key)
        if isinstance(candidate, str) and candidate.strip():
            return candidate.strip()[:80]
    for key in ("call", "toolCall", "data"):
        nested = value.get(key)
        if isinstance(nested, Mapping):
            name = _indexed_name(nested, depth + 1)
            if name:
                return name
    return None


def _usage_numbers(usage: object) -> dict[str, int]:
    out = {"input": 0, "output": 0, "total": 0, "cache_read": 0}
    if not isinstance(usage, Mapping):
        return out
    for key, alias in (
        ("inputTokens", "input"),
        ("input_tokens", "input"),
        ("outputTokens", "output"),
        ("output_tokens", "output"),
        ("totalTokens", "total"),
        ("total_tokens", "total"),
        ("cacheReadTokens", "cache_read"),
        ("cache_read_tokens", "cache_read"),
    ):
        value = usage.get(key)
        if isinstance(value, (int, float)):
            out[alias] += int(value)
    return out


def _message_body(data: object) -> Mapping[str, object]:
    """user/message 的 data 有两种形状：包一层 message 或直接是消息。"""
    if isinstance(data, Mapping):
        nested = data.get("message")
        if isinstance(nested, Mapping):
            return nested
        return data
    return {}


def session_metadata(session_dir: Path) -> dict[str, object] | None:
    path = session_dir / "session.v4.jsonl.zstd"
    if not path.is_file():
        path = session_dir / "session.v4.jsonl"
        if not path.is_file():
            return None
    text = decode_session_file(path)
    if text is None:
        return {"session": session_dir.name, "error": "decode_failed"}

    events: list[Mapping[str, object]] = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            parsed = json.loads(line)
        except ValueError:
            continue
        if isinstance(parsed, Mapping):
            events.append(parsed)

    session_meta = next((e for e in events if e.get("type") == "session"), {})
    provider = None
    model = None
    context_window = None
    for event in events:
        if event.get("type") == "request/context":
            data = event.get("data") or {}
            if isinstance(data, Mapping):
                provider = data.get("provider")
                model = data.get("model")
                context_window = data.get("contextWindow")
                break

    system_text_parts: list[str] = []
    injected: dict[str, int] = {
        "user": 0,
        "runtime-context": 0,
        "skill-catalog": 0,
        "time-context": 0,
        "other": 0,
    }
    turns: dict[int, dict[str, object]] = {}
    tool_calls = 0
    tool_names: list[str] = []
    approvals = 0
    retries = 0
    attempts = 0
    compactions = 0
    event_types: dict[str, int] = {}

    def turn_slot(number: int) -> dict[str, object]:
        return turns.setdefault(
            number,
            {
                "start_ms": None,
                "end_ms": None,
                "first_chunk_ms": None,
                "message_ms": None,
                "rounds": 0,
                "tool_calls": 0,
                "usage": {"input": 0, "output": 0, "total": 0, "cache_read": 0},
                "user_chars": 0,
            },
        )

    for event in events:
        event_type = str(event.get("type") or "")
        event_types[event_type] = event_types.get(event_type, 0) + 1
        time_ms = event.get("time")
        data = event.get("data") or {}

        if event_type == "system/message":
            body = _message_body(data)
            for block in body.get("content") or []:
                if isinstance(block, Mapping) and block.get("type") == "text":
                    system_text_parts.append(str(block.get("text") or ""))
            continue

        if event_type == "user/message":
            body = _message_body(data)
            source = body.get("source") if isinstance(body.get("source"), Mapping) else {}
            kind = str(source.get("kind") or "other")
            chars = sum(
                len(str(block.get("text") or ""))
                for block in (body.get("content") or [])
                if isinstance(block, Mapping)
            )
            injected[kind if kind in injected else "other"] += chars
            if kind == "user" and event.get("data", {}).get("turn") is not None:
                slot = turn_slot(int(data["turn"]))
                slot["user_chars"] = int(slot["user_chars"]) + chars
            continue

        if event_type == "turn/start" and time_ms is not None:
            slot = turn_slot(int(data.get("turn") or 0))
            slot["start_ms"] = time_ms
            continue
        if event_type == "turn/end" and time_ms is not None:
            slot = turn_slot(int(data.get("turn") or 0))
            slot["end_ms"] = time_ms
            continue

        if event_type == "assistant/message":
            slot = turn_slot(int(data.get("turn") or 0))
            slot["rounds"] = int(slot["rounds"]) + 1
            usage = _usage_numbers(data.get("usage"))
            merged = slot["usage"]
            for key, value in usage.items():
                merged[key] += value
            if time_ms is not None:
                slot["message_ms"] = time_ms
            first_chunk = None
            for entry in data.get("stream") or []:
                chunk_time = entry.get("time") if isinstance(entry, Mapping) else None
                if isinstance(chunk_time, (int, float)) and (
                    first_chunk is None or chunk_time < first_chunk
                ):
                    first_chunk = chunk_time
            if first_chunk is not None and slot["first_chunk_ms"] is None:
                slot["first_chunk_ms"] = first_chunk
            continue

        if event_type == "tool/call":
            tool_calls += 1
            name = _indexed_name(data)
            if name:
                tool_names.append(name)
            turn_number = data.get("turn") if isinstance(data, Mapping) else None
            if isinstance(turn_number, (int, float)):
                slot = turn_slot(int(turn_number))
                slot["tool_calls"] = int(slot["tool_calls"]) + 1
            continue

        # DSH 会话里审批请求既有 approval/request（适配层）也有 approval/asked（会话事件）。
        if event_type in {"approval/request", "approval/asked"}:
            approvals += 1
            continue
        if event_type in {"llm/retry", "llm/retry-started"}:
            retries += 1
            continue
        if event_type == "assistant/attempt":
            attempts += 1
            continue
        if "compact" in event_type.lower():
            compactions += 1

    system_prompt = system_prompt_segments("\n".join(system_text_parts))
    turn_rows: list[dict[str, object]] = []
    for number in sorted(turns):
        slot = turns[number]
        start_ms = slot["start_ms"]
        end_ms = slot["end_ms"]
        first_chunk_ms = slot["first_chunk_ms"]
        message_ms = slot["message_ms"]
        total_s = None
        ttft_s = None
        if isinstance(start_ms, (int, float)):
            if isinstance(end_ms, (int, float)):
                total_s = round((end_ms - start_ms) / 1000, 2)
            marker = first_chunk_ms if isinstance(first_chunk_ms, (int, float)) else message_ms
            if isinstance(marker, (int, float)):
                ttft_s = round((marker - start_ms) / 1000, 2)
        turn_rows.append(
            {
                "turn": number,
                "total_seconds": total_s,
                "first_response_seconds": ttft_s,
                "model_rounds": slot["rounds"],
                "tool_calls": slot["tool_calls"],
                "usage": slot["usage"],
                "user_chars": slot["user_chars"],
            }
        )

    return {
        "session": session_dir.name,
        "project": session_dir.parent.name,
        "file": path.name,
        "size_bytes": path.stat().st_size,
        "created_at": session_meta.get("createdAt"),
        "cwd": session_meta.get("cwd"),
        "provider": provider,
        "model": model,
        "context_window": context_window,
        "system_prompt": system_prompt,
        "injected_chars": injected,
        "turns": turn_rows,
        "tool_calls": tool_calls,
        "tool_names": sorted(set(tool_names)),
        "approvals": approvals,
        "retries": retries,
        "assistant_attempts": attempts,
        "compactions": compactions,
        "event_types": event_types,
    }


def session_snapshot(
    session_root: Path, limit: int, session_id: str | None = None
) -> list[dict[str, object]]:
    if not session_root.is_dir():
        return []
    candidates: list[Path] = []
    if session_id:
        for project in session_root.iterdir():
            if not project.is_dir():
                continue
            candidate = project / session_id
            if candidate.is_dir():
                candidates.append(candidate)
    else:
        for project in session_root.iterdir():
            if not project.is_dir():
                continue
            candidates.extend(p for p in project.iterdir() if p.is_dir())
    candidates.sort(
        key=lambda p: p.stat().st_mtime if p.exists() else 0,
        reverse=True,
    )
    rows: list[dict[str, object]] = []
    for path in candidates[:limit]:
        row = session_metadata(path)
        if row is not None:
            rows.append(row)
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


def build_report(
    app_root: Path,
    latest: int,
    sessions: int = 0,
    session_root: Path | None = None,
    session_id: str | None = None,
) -> dict[str, object]:
    report: dict[str, object] = {
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
    if sessions > 0:
        root = session_root if session_root is not None else default_session_root(app_root)
        report["sessions"] = {
            "root": str(root),
            "rows": session_snapshot(root, sessions, session_id),
        }
    return report


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

    sessions = report.get("sessions")
    if not sessions:
        return
    print("\n== recent sessions (metadata only) ==")
    print(f"session_root: {sessions.get('root')}")
    rows = sessions.get("rows") or []
    if not rows:
        print("(none)")
        return
    for row in rows:
        if row.get("error"):
            print(f"{row.get('session')}: error={row.get('error')}")
            continue
        prompt = row.get("system_prompt") or {}
        buckets = prompt.get("buckets") or {}
        bucket_text = " ".join(
            f"{name}={buckets.get(name, 0)}" for name in SESSION_PROMPT_BUCKETS
        )
        print(
            f"{row.get('session')} [{row.get('project')}] "
            f"model={row.get('provider')}/{row.get('model')} "
            f"context={row.get('context_window')}"
        )
        print(
            f"  prompt_chars: system={prompt.get('chars', 0)} "
            f"segments={prompt.get('segments', 0)} | {bucket_text}"
        )
        injected = row.get("injected_chars") or {}
        print(
            "  injected_chars: "
            + " ".join(f"{name}={value}" for name, value in injected.items())
        )
        print(
            f"  tool_calls={row.get('tool_calls')} names={row.get('tool_names')} "
            f"approvals={row.get('approvals')} retries={row.get('retries')} "
            f"attempts={row.get('assistant_attempts')} compactions={row.get('compactions')}"
        )
        for turn in row.get("turns") or []:
            usage = turn.get("usage") or {}
            print(
                f"  turn {turn.get('turn')}: total={turn.get('total_seconds')}s "
                f"first_response={turn.get('first_response_seconds')}s "
                f"rounds={turn.get('model_rounds')} tools={turn.get('tool_calls')} "
                f"in={usage.get('input', 0)} out={usage.get('output', 0)} "
                f"cache_read={usage.get('cache_read', 0)}"
            )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--app-root",
        type=Path,
        default=Path(os.environ.get("HOME", "~")).expanduser() / "OmniDesk" / "app",
    )
    ap.add_argument("--latest", type=int, default=3, help="最近任务快照数量")
    ap.add_argument(
        "--sessions",
        type=int,
        default=0,
        help="解析最近 N 个 DSH 会话的提示词/耗时/token 元数据（0 = 关闭）",
    )
    ap.add_argument(
        "--session-root",
        type=Path,
        default=None,
        help="DSH sessions 目录（默认 app_root 上一级的 runtime/dsh-home/sessions）",
    )
    ap.add_argument("--session-id", default=None, help="只解析指定会话 id")
    ap.add_argument("--json", action="store_true", help="输出 JSON")
    args = ap.parse_args()

    app_root = args.app_root.expanduser().resolve()
    report = build_report(
        app_root,
        max(1, min(args.latest, 20)),
        sessions=max(0, min(args.sessions, 50)),
        session_root=args.session_root.expanduser().resolve() if args.session_root else None,
        session_id=(args.session_id or "").strip() or None,
    )
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print_text(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
