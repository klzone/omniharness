#!/usr/bin/env python3
"""OD-HAR-211/212/213 静态与组装一致性验收。

只读仓库文件与引擎模块，不启动 DSH、不调模型、不写用户数据。
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
RULES = ROOT / "samples/profiles/discipline.rules.json"
PATCH = ROOT / "samples/profiles/discipline.profile.patch.yml"
SEED = ROOT / "runtime/dsh-home-seed/profiles/discipline/cordis.patch.yml"
SCENES = ROOT / "desktop/src/scenes.ts"
ADAPTER = ROOT / "engine/omnidesk_engine/web_api_adapter.py"
EXTRACT = ROOT / "engine/omnidesk_engine/task_extract.py"
SEARCH = ROOT / "desktop/src-tauri/src/worklog_search.rs"

cases: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    cases.append({"name": name, "ok": bool(ok), "detail": detail})


def main() -> int:
    rules = json.loads(RULES.read_text(encoding="utf-8"))
    agent = (rules.get("agents") or {}).get("work-review")
    check("rules_v3", rules.get("schema") == "omnidesk.profile-rules.v3", rules.get("schema"))
    check("agent_declared", isinstance(agent, dict) and agent.get("label") == "工作记录复盘", json.dumps(agent, ensure_ascii=False)[:200])
    if isinstance(agent, dict):
        scenes = set(agent.get("scenes") or [])
        check("agent_covers_weekly", "weekly" in scenes, ",".join(sorted(scenes)))
        check(
            "agent_scenes_declared",
            scenes.issubset(set(rules.get("scenes") or {}) | {
                key for key in (rules.get("skill_bindings") or {})
                if key.startswith("weekly-")
            } | {"weekly"}),
            ",".join(sorted(scenes)),
        )
        bound = set()
        for key, binding in (rules.get("skill_bindings") or {}).items():
            if key == "weekly" or key.startswith("weekly-"):
                bound.update(binding.get("skills") or [])
        check("agent_skills_match_bindings", set(agent.get("skills") or []) == bound, json.dumps(sorted(bound)))

    # 2026-11-13 定稿：Work Review 使用无 shell、可写文件的 preset。
    def block(text: str, preset_id: str) -> str:
        start = text.find("- id: " + preset_id + "\n")
        if start < 0:
            return ""
        nxt = re.search(r"^- (?:id:|insert:)", text[start + 1 :], re.M)
        return text[start : start + 1 + nxt.start()] if nxt else text[start:]

    sample_block = block(PATCH.read_text(encoding="utf-8"), "preset-work-review-core")
    check("preset_sample_exists", bool(sample_block))
    check("preset_id", "id: work-review-core" in sample_block)
    check("preset_can_write", "write: true" in sample_block and "edit: true" in sample_block)
    check("preset_no_shell", "- id: tool-bash" not in sample_block and "- id: tool-pwsh" not in sample_block)
    check("preset_keeps_read", "read: true" in sample_block and "- id: tool-fs-search" in sample_block)
    check("preset_seed_exists", bool(block(SEED.read_text(encoding="utf-8"), "preset-work-review-core")))
    adapter_src = ADAPTER.read_text(encoding="utf-8")
    sys.path.insert(0, str(ROOT / "engine"))
    from omnidesk_engine.web_api_adapter import preset_for_intent  # noqa: E402

    check("intent_maps_to_preset", preset_for_intent("weekly_report", "discipline") == "work-review-core")
    check("other_profile_unchanged", preset_for_intent("weekly_report", "omnidesk-base") is None)
    check("other_intent_unchanged", preset_for_intent("material_writing", "discipline") is None)

    scene_src = SCENES.read_text(encoding="utf-8")
    weekly_block = scene_src[scene_src.find("weekly: {") : scene_src.find("writing: {")]
    check("scene_agent_binding", 'agent: "work-review"' in weekly_block, "")
    check("scene_agent_field_declared", "agent?: string" in scene_src, "")

    history = [
        {
            "id": "wr-history-1",
            "title": "上周周报素材",
            "record_date": "2026-11-05",
            "category": "文案",
            "tags": ["周报"],
            "reason": "同分类；同标签：周报",
        }
    ]
    payload = {
        "record": {
            "id": "wr-curr-1",
            "title": "整理周报",
            "record_date": "2026-11-12",
            "category": "文案",
            "tags": ["周报"],
            "body": "今天整理周报，补充项目进展。",
        },
        "history": history,
        "history_body_secret": "不应出现在提示词里的历史正文",
    }
    proc = subprocess.run(
        [sys.executable, "-m", "omnidesk_engine.task_extract"],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        cwd=str(ROOT / "engine"),
        timeout=30,
    )
    check("prompt_assembles", proc.returncode == 0, proc.stderr[-200:])
    prompt = json.loads(proc.stdout)["prompt"] if proc.returncode == 0 else ""
    check("prompt_has_history", "相关历史记录" in prompt and "wr-history-1" in prompt)
    check("prompt_metadata_only", "history_body_secret" not in prompt and "不应出现在提示词里" not in prompt)

    search_src = SEARCH.read_text(encoding="utf-8")
    check("history_metadata_fields", all(field in search_src for field in ("pub id", "pub title", "pub record_date", "pub reason")))
    check("history_has_no_body_field", "pub body" not in search_src)
    check("history_cap", "MAX_HISTORY_HITS: usize = 5" in search_src)

    check(
        "session_create_passes_preset",
        'request_body["agentPreset"] = agent_preset' in adapter_src and '"session/create"' in adapter_src,
    )

    extract_src = EXTRACT.read_text(encoding="utf-8")
    check("history_param_declared", "history: Sequence[Mapping[str, Any]] = ()" in extract_src)
    check("history_in_cli_payload", 'payload.get("history")' in extract_src)

    failed = [case for case in cases if not case["ok"]]
    print(json.dumps({"cases": len(cases), "failed": len(failed), "result": "pass" if not failed else "fail", "failures": failed}, ensure_ascii=False))
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
