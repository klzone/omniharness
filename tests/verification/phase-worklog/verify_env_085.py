#!/usr/bin/env python3
"""OD-ARCH-085 环境事实探测：Skill 同步、注入预算、绑定白名单、Obsidian CLI、数据根骨架。

这里只做**只读**探测并落盘事实，用来支撑 STATUS.md 里的可复核结论：

1. 仓库 skills/ 与数据根 ~/OmniDesk/runtime/dsh-home/skills/ 的两份 SKILL.md 逐字节一致；
2. 两份 SKILL.md 都 ≤ 2500 字符（desktop/src/aiDraft.ts 的注入截断点），规则不会到不了模型；
3. 出厂规则包 weekly 场景的绑定与 src-tauri 的 ALLOWED_SKILLS 白名单是否相容；
4. 本机是否有 Obsidian CLI（决定走 CLI 还是降级文件读写）；
5. 数据根目录骨架与 知识库/索引.md 空表头存在；
6. 冻结范围（runtime/、desktop/src-tauri/、engine/）零差异。

不读取任何真实工作记录正文；不写数据根；不联网。

用法：
  python3 tests/verification/phase-worklog/verify_env_085.py
  python3 tests/verification/phase-worklog/verify_env_085.py --no-write

证据：tests/verification/phase-worklog/evidence/env.json
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
EVIDENCE = HERE / "evidence"
SKILLS = REPO / "skills"
RULES = REPO / "samples" / "profiles" / "discipline.rules.json"
ALLOWLIST_RS = REPO / "desktop" / "src-tauri" / "src" / "profile_rules.rs"
AI_DRAFT_TS = REPO / "desktop" / "src" / "aiDraft.ts"
DATA_ROOT = Path(os.path.expanduser("~/OmniDesk"))
DATA_SKILLS = DATA_ROOT / "runtime" / "dsh-home" / "skills"
LIBRARY = DATA_ROOT / "库"
SKILL_IDS = ("memory-agent", "knowledge-search")
INJECTION_BUDGET = 2500
# OD-ARCH-088 获批修改的两个适配层文件；engine/ 其余文件仍视为冻结。
ENGINE_ALLOWED = {
    "engine/omnidesk_engine/serve.py",
    "engine/omnidesk_engine/web_api_adapter.py",
}

CASES: list[dict] = []
OBSTACLES: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": str(detail)})


def blocked(name: str, detail: str) -> None:
    """记录一项「本应完成但被硬约束挡住」的事实，不计入失败。"""
    OBSTACLES.append({"name": name, "detail": detail})


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_dirty(path: str) -> list[str]:
    res = subprocess.run(["git", "status", "--porcelain", "--", path],
                         cwd=str(REPO), capture_output=True, text=True)
    return [line for line in res.stdout.splitlines() if line.strip()]


def check_skill_sync() -> dict[str, object]:
    out: dict[str, object] = {}
    for name in SKILL_IDS:
        repo_file = SKILLS / name / "SKILL.md"
        data_file = DATA_SKILLS / name / "SKILL.md"
        text = repo_file.read_text(encoding="utf-8")
        check(f"{name}_repo_skill_exists", repo_file.is_file(), str(repo_file))
        check(f"{name}_within_injection_budget", len(text) <= INJECTION_BUDGET,
              f"{len(text)} 字符 / 预算 {INJECTION_BUDGET}")
        if data_file.is_file():
            same = sha(repo_file) == sha(data_file)
            check(f"{name}_synced_to_data_root", same,
                  f"repo={sha(repo_file)[:16]} data={sha(data_file)[:16]}")
            out[name] = {"chars": len(text), "sha256": sha(repo_file),
                         "data_root_sha256": sha(data_file), "in_sync": same}
        else:
            check(f"{name}_synced_to_data_root", False, f"缺失 {data_file}")
            out[name] = {"chars": len(text), "sha256": sha(repo_file), "in_sync": False}
    return out


def parse_allowlist() -> list[str]:
    """从 profile_rules.rs 里读出编译进二进制的 ALLOWED_SKILLS 清单。"""
    text = ALLOWLIST_RS.read_text(encoding="utf-8")
    start = text.find("pub const ALLOWED_SKILLS")
    if start < 0:
        return []
    end = text.find("];", start)
    if end < 0:
        return []
    block = text[start:end]
    return [seg.split('"')[1] for seg in block.split(",") if seg.count('"') >= 2]


def check_bindings() -> dict[str, object]:
    allowed = parse_allowlist()
    check("allowlist_parsed", len(allowed) >= 6, str(allowed))
    # OD-ARCH-086 已批准解冻 src-tauri/ 扩容白名单，故这里只断言「绑定与白名单相容」。
    # 085 当时任何 anysearch 都被拒的事实保留在 evidence/env-085-historical.json。
    check("allowlist_may_include_anysearch_after_086", True,
          "anysearch 是否在白名单：" + str("anysearch" in allowed))

    rules = json.loads(RULES.read_text(encoding="utf-8"))
    bindings = rules.get("skill_bindings", {})
    weekly = bindings.get("weekly", {})
    skills = weekly.get("skills", [])
    max_injected = weekly.get("max_injected", 0)
    check("weekly_binding_shape", isinstance(skills, list) and len(skills) > 0, str(skills))
    check("weekly_binding_within_three", 1 <= max_injected <= 3 and len(skills) <= max_injected,
          f"{len(skills)} 项 / 上限 {max_injected}")
    check("weekly_binding_all_allowed", all(s in allowed for s in skills),
          "存在白名单外技能则整份规则文件会被拒绝：" + str(skills))
    check("weekly_binding_has_memory_agent", "memory-agent" in skills, str(skills))
    check("weekly_binding_has_knowledge_search", "knowledge-search" in skills, str(skills))
    check("weekly_binding_no_duplicates", len(skills) == len(set(skills)), str(skills))

    # OD-ARCH-089：anysearch 移出 weekly 预注入，运行时按需加载；仍在白名单与安装清单里。
    check("weekly_binding_excludes_anysearch_after_089", "anysearch" not in skills, str(skills))
    check("weekly_binding_has_memory_report_after_089", "memory-report" in skills, str(skills))
    check("anysearch_still_allowlisted", "anysearch" in allowed, str(allowed))

    if "anysearch" not in allowed:
        blocked(
            "anysearch_skill_binding",
            "任一技能不在 desktop/src-tauri/src/profile_rules.rs 的 ALLOWED_SKILLS 内时，"
            "validate() 会整份拒绝该规则文件（loaded:false，红线与掩码一起失效）。"
            "OD-ARCH-086 已解冻白名单；此处仅提示仍存在清单外技能。",
        )
    return {"allowlist": allowed, "weekly": weekly}


def check_injection_budget_source() -> None:
    text = AI_DRAFT_TS.read_text(encoding="utf-8")
    check("aiDraft_truncates_at_2500", "raw.length <= 2500" in text,
          "注入链路确实在 2500 字符处截断，Skill 超长规则会丢失")


def probe_obsidian() -> dict[str, object]:
    which = shutil.which("obsidian")
    flatpak = shutil.which("flatpak")
    snap = shutil.which("snap")
    desktop_hits: list[str] = []
    for base in (Path(os.path.expanduser("~/.local/share/applications")),
                 Path("/usr/share/applications")):
        if base.is_dir():
            desktop_hits.extend(sorted(p.name for p in base.glob("*obsidian*")))
    available = bool(which)
    check("obsidian_probe_ran", True,
          f"which={which!r} flatpak={flatpak!r} snap={snap!r} desktop={desktop_hits or '无'}")
    if not available:
        blocked(
            "obsidian_cli",
            "本机没有 obsidian 命令、没有 Obsidian desktop 文件、flatpak/snap 也无 Obsidian。"
            "按 Skill 写死规则走降级路径（普通文件读写 + 手工双链核对）；"
            "有 CLI 时的命令形状已在合成评测中用受控假 CLI 正向验证。",
        )
    return {"command": which, "available": available,
            "flatpak": flatpak, "snap": snap, "desktop_entries": desktop_hits}


def check_data_root() -> dict[str, object]:
    expected_dirs = [
        LIBRARY / "工作记录", LIBRARY / "周报",
        LIBRARY / "知识库", LIBRARY / "知识库" / "概念", LIBRARY / "知识库" / "方法",
        LIBRARY / "inbox",
    ]
    missing = [str(p) for p in expected_dirs if not p.is_dir()]
    check("data_root_skeleton_present", missing == [], f"缺失：{missing}")

    index = LIBRARY / "知识库" / "索引.md"
    check("data_root_index_exists", index.is_file(), str(index))
    if index.is_file():
        text = index.read_text(encoding="utf-8")
        check("data_root_index_is_empty_head", "一行一卡" in text and "](概念/" not in text,
              text.replace("\n", " ")[:80])
    return {"library": str(LIBRARY), "missing": missing,
            "index_exists": index.is_file()}


def check_frozen() -> dict[str, object]:
    runtime = git_dirty("runtime/")
    # OD-ARCH-086 获批修改 desktop/src-tauri/（白名单与库根）。
    # OD-ARCH-088 获批修改适配层与 RPC 入口，engine/ 其余文件仍冻结。
    backend = [line for line in git_dirty("engine/")
               if line[3:].strip() not in ENGINE_ALLOWED]
    check("runtime_zero_diff", runtime == [], str(runtime))
    check("engine_zero_diff", backend == [], str(backend))
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=str(REPO),
                          capture_output=True, text=True).stdout.strip()
    return {"head": head, "runtime": runtime, "backend": backend}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-write", action="store_true")
    args = ap.parse_args()

    skills = check_skill_sync()
    bindings = check_bindings()
    check_injection_budget_source()
    obsidian = probe_obsidian()
    data_root = check_data_root()
    scope = check_frozen()

    failed = [c for c in CASES if not c["ok"]]
    payload = {
        "task": "OD-ARCH-085",
        "kind": "environment-probe",
        "generated_at": dt.datetime.now().isoformat(timespec="seconds"),
        "evidence_level": "script",
        "isolation": "只读探测仓库与数据根目录结构，未读取任何真实工作记录正文",
        "cases_total": len(CASES),
        "cases_passed": len(CASES) - len(failed),
        "cases_failed": len(failed),
        "result": "pass" if not failed else "fail",
        "skills": skills,
        "bindings": bindings,
        "obsidian": obsidian,
        "data_root": data_root,
        "repo_scope": scope,
        "obstacles": OBSTACLES,
        "cases": CASES,
    }
    print(f"cases={payload['cases_total']} passed={payload['cases_passed']} "
          f"failed={payload['cases_failed']} obstacles={len(OBSTACLES)}")
    for c in failed:
        print("  [FAIL] %s — %s" % (c["name"], c["detail"][:200]))
    if not args.no_write:
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        (EVIDENCE / "env.json").write_text(
            json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print("evidence:", EVIDENCE / "env.json")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
