#!/usr/bin/env python3
"""
OmniDesk P1 第三步 —— ACP 适配器场景驱动公共工具。

安全边界（本目录共同遵守）：
  * 每个场景使用 /tmp 下一次性数据根；DSH_HOME / DSH_AGENTS_HOME 均指向该根。
  * 不使用、不读取、不打印、不复制任何真实凭证。合成端点仅使用占位字符串。
  * 不修改 DSH Runtime；不触碰 ~/.dsh、~/.agents、~/OmniDesk、~/YvanAI。
  * 合成素材全部为虚构占位内容，不含真实人员、机构、案件或内部信息。
  * 结果中不出现 ACP 内部字段名（原始帧只作为证据单独存放）。

证据等级：static / script / model / unverified
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import time
import traceback
from pathlib import Path

ADAPTER_DIR = Path(__file__).resolve().parent
ENGINE_INTERFACE_DIR = ADAPTER_DIR.parent
REPO = Path(__file__).resolve().parents[4]

sys.path.insert(0, str(REPO / "engine"))
sys.path.insert(0, str(ADAPTER_DIR))

DSH_BIN = REPO / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
EVIDENCE_DIR = ENGINE_INTERFACE_DIR / "evidence" / "adapter"

# 合成占位键：非凭证，仅用于让引擎认为该 provider 已配置。
FAKE_KEY_ENV = "OMNI_FAKE_KEY"
FAKE_KEY_VALUE = "synthetic-placeholder-not-a-credential"

# 虚构素材（无真实单位/人员/案件信息）。
SYNTH_PROMPT = (
    "请基于以下虚构要点写一段材料草稿（全部为合成占位内容，无真实单位与人员）："
    "本年度共完成事项 A 类 12 项、B 类 7 项；主要做法为流程再造与台账化管理；"
    "下一步拟完善复核环节。"
)
SYNTH_PROMPT_2 = "请用一句话说明合成占位材料的写作要点。"
SYNTH_PROMPT_3 = "请就以下合成要点生成小结：事项 A 完成 12 项；事项 A 完成 9 项。"


def write_evidence(name: str, payload: dict) -> Path:
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
    path = EVIDENCE_DIR / (name + ".json")
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return path


def new_root(tag: str) -> dict:
    base = Path(tempfile.mkdtemp(prefix="omni_p1s3_" + tag + "_"))
    root = {
        "base": base,
        "dsh_home": base / "dsh-home",
        "agents": base / "agents",
        "cwd": base / "work",
        "state": base / "state",
    }
    for key in ("dsh_home", "agents", "cwd"):
        root[key].mkdir(parents=True, exist_ok=True)
    root["state"].mkdir(parents=True, exist_ok=True)
    return root


def drop_root(root: dict) -> None:
    """清理一次性数据根。沙箱不提供 rm，因此用 Python 删除。"""
    def _on_error(func, path, exc_info):  # 忽略只读残留
        try:
            os.chmod(path, 0o700)
            func(path)
        except Exception:
            pass
    shutil.rmtree(str(root["base"]), onerror=_on_error)


def adapter_env(root: dict, extra: dict | None = None) -> dict:
    env = dict(os.environ)
    env["DSH_HOME"] = str(root["dsh_home"])
    env["DSH_AGENTS_HOME"] = str(root["agents"])
    env[FAKE_KEY_ENV] = FAKE_KEY_VALUE
    if extra:
        env.update(extra)
    return env


def mock_overlay_text(base_url: str, *, model: str = "synthetic-model",
                      persona_prefix: str | None = None) -> str:
    lines = [
        "- id: acp",
        '  name: "@deepseek-ai/dsh-acp"',
        "  config:",
        "    provider: fake",
        "    model: " + model,
        "- id: llm-pi-ai",
        '  name: "@deepseek-ai/dsh-llm-pi-ai"',
        "  config:",
        "    providers:",
        "      fake:",
        "        displayName: Fake",
        "        apiKeyEnv: " + FAKE_KEY_ENV,
        "        api: openai-completions",
        "        baseURL: " + base_url,
        "        models:",
        "          - id: " + model,
        "            name: " + model,
    ]
    if persona_prefix is not None:
        lines += [
            "- id: system-prompt",
            "  config:",
            "    personaPrefix: " + json.dumps(persona_prefix),
        ]
    return "\n".join(lines) + "\n"


def write_overlay(root: dict, name: str, text: str) -> Path:
    path = root["base"] / (name + ".yml")
    path.write_text(text, encoding="utf-8")
    return path


def make_adapter(root: dict, patches=(), *, profile: str = "acp",
                 state_dir=None, dsh_bin=None, env_extra=None, **kwargs):
    from omnidesk_engine import AcpEngineAdapter

    return AcpEngineAdapter.for_dsh(
        str(dsh_bin if dsh_bin is not None else DSH_BIN),
        cwd=str(root["cwd"]),
        profile=profile,
        patch_files=[str(p) for p in patches],
        env=adapter_env(root, env_extra),
        state_dir=str(state_dir if state_dir is not None else root["state"]),
        **kwargs,
    )


def events_to_list(adapter, omni_task_id, **kwargs) -> list:
    return [e.to_dict() for e in adapter.stream_events(omni_task_id, **kwargs)]


def frames_digest(frames) -> list:
    """只保留帧的类型信息，避免把引擎内部结构带进结论。"""
    out = []
    for frame in frames or []:
        if not isinstance(frame, dict):
            out.append({"_shape": type(frame).__name__})
            continue
        entry = {}
        if "method" in frame:
            entry["method"] = frame.get("method")
        if "id" in frame:
            entry["id"] = frame.get("id")
        if "error" in frame:
            entry["error_code"] = (frame.get("error") or {}).get("code")
            entry["error_message"] = (frame.get("error") or {}).get("message")
        if "result" in frame and isinstance(frame.get("result"), dict):
            entry["result_keys"] = sorted(str(k) for k in frame["result"].keys())
        out.append(entry)
    return out


def sent_digest(frames) -> list:
    out = []
    for frame in frames or []:
        if not isinstance(frame, dict):
            continue
        out.append({"method": frame.get("method"), "id": frame.get("id")})
    return out


def method_counts(frames) -> dict:
    counts: dict = {}
    for frame in frames or []:
        if not isinstance(frame, dict):
            continue
        method = frame.get("method")
        if method:
            counts[str(method)] = counts.get(str(method), 0) + 1
    return counts


def safe_call(func, *args, **kwargs):
    """返回 (ok, value)，异常时把契约错误码结构化记下，便于如实报告失败。"""
    try:
        return True, func(*args, **kwargs)
    except Exception as exc:  # noqa: BLE001 - 需要捕获契约错误以记录证据
        code = getattr(exc, "code", None)
        code_value = getattr(code, "value", None) or (str(code) if code else None)
        return False, {
            "exception": type(exc).__name__,
            "engine_error_code": code_value,
            "message": str(exc),
            "detail": _safe_detail(getattr(exc, "detail", None)),
        }


def _safe_detail(detail):
    if not isinstance(detail, dict):
        return detail
    out = {}
    for key, value in detail.items():
        if key in ("stderr_tail",):
            out[key] = "<omitted from evidence>"
        else:
            out[key] = value
    return out


class ScenarioResult:
    """收集单个场景的证据附件与判定。"""

    def __init__(self, scenario_id: str, title: str, channel: str = "acp"):
        self.scenario_id = scenario_id
        self.title = title
        self.payload = {
            "scenario": scenario_id,
            "title": title,
            "channel": channel,
            "evidence_level": "script",
            "executed": False,
            "started_at": time.time(),
            "checks": [],
            "artifacts": {},
            "notes": [],
            "verdict": "unverified",
        }

    def check(self, name: str, ok, detail=None) -> bool:
        entry = {"check": name, "pass": bool(ok)}
        if detail is not None:
            entry["detail"] = detail
        self.payload["checks"].append(entry)
        return bool(ok)

    def note(self, text: str) -> None:
        self.payload["notes"].append(text)

    def artifact(self, key: str, value) -> None:
        self.payload["artifacts"][key] = value

    def finish(self, *, executed: bool = True) -> dict:
        self.payload["executed"] = executed
        self.payload["finished_at"] = time.time()
        checks = self.payload["checks"]
        if not executed:
            self.payload["verdict"] = "unverified"
        elif checks and all(c["pass"] for c in checks):
            self.payload["verdict"] = "pass"
        else:
            self.payload["verdict"] = "fail"
        return self.payload


def run_scenario(scenario_id: str, title: str, body, *, root: dict | None = None,
                 channel: str = "acp") -> dict:
    """执行一个场景函数，保证异常也落成证据文件。"""
    result = ScenarioResult(scenario_id, title, channel=channel)
    own_root = root is None
    active_root = root if root is not None else new_root(scenario_id.lower())
    result.artifact("data_root", str(active_root["base"]))
    try:
        body(result, active_root)
    except Exception:
        result.note("场景执行抛出未处理异常：\n" + traceback.format_exc(limit=6))
        result.check("scenario_completed_without_exception", False)
    finally:
        payload = result.finish(executed=any(c["check"] for c in result.payload["checks"]))
        write_evidence(scenario_id, payload)
        if own_root:
            drop_root(active_root)
    return payload

