#!/usr/bin/env python3
"""OD-ARCH-104：办公 Skill 只随专业 Profile 挂载，基础标准保持官方插件面。

检查两层：
  1. 静态：出厂样例 discipline.profile.patch.yml 只有一条顶层 insert，挂
     @deepseek-ai/dsh-skill-office；omnidesk-base 的补丁里不出现 office。
  2. 运行：用隔离 DSH_HOME 跑 --dump-config，断言 discipline 组装结果含
     skill-office 且未被 disabled，omnidesk-base 完全不含；stderr 无
     "disabling profile plugin"（被兼容性预检静默摘掉的情形）。

依赖本机已存在的 ~/OmniDesk 隔离根与仓库内 dsh-runtime 依赖；不做任何写入。
用法：python3 tests/verification/desktop/verify_plugin_scope_104.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SAMPLE = REPO / "samples" / "profiles" / "discipline.profile.patch.yml"
BASE_SAMPLE = REPO / "samples" / "profiles" / "omnidesk-base.profile.patch.yml"
RUNTIME = REPO / "runtime" / "dsh-runtime"
BIN = RUNTIME / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
OFFICE = RUNTIME / "node_modules" / "@deepseek-ai" / "dsh-skill-office"
DSH_HOME = Path(os.environ.get("OMNIDESK_DSH_HOME", "/home/kevin/OmniDesk/runtime/dsh-home"))
REAL_PATCH = DSH_HOME / "profiles" / "discipline" / "cordis.patch.yml"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def dump(profile: str) -> tuple[int, str, str]:
    env = dict(os.environ)
    env["DSH_HOME"] = str(DSH_HOME)
    proc = subprocess.run(
        ["node", str(BIN), "--profile", profile, "--dump-config"],
        cwd=str(RUNTIME),
        env=env,
        capture_output=True,
        text=True,
        timeout=180,
    )
    return proc.returncode, proc.stdout, proc.stderr


def entry_block(text: str, entry_id: str) -> str:
    """取 dump 里某个顶层条目（"- id: xxx" 到下一个顶层 "- id:" 之前）。"""
    idx = text.find("- id: " + entry_id + "\n")
    if idx < 0:
        return ""
    nxt = re.search(r"^- (?:id:|insert:)", text[idx + 1 :], re.M)
    return text[idx : idx + 1 + nxt.start()] if nxt else text[idx:]


PROBE_JS = """
const { apply } = await import(process.env.OFFICE_LIB)
const registered = []
const ctx = { skills: { registerProvider: (fn) => registered.push(fn()) } }
try {
  apply(ctx, {})
} catch (e) {
  console.log(JSON.stringify({ ok: false, error: String(e) }))
  process.exit(0)
}
const provider = registered[0]
const list = await provider.list()
const one = await provider.get(list[0], {})
console.log(JSON.stringify({
  ok: true,
  provider: provider.name,
  skills: list.map((s) => s.name),
  content_has_kit: /Installed LibreOffice Kit/.test(one.content),
  content_chars: one.content.length,
}))
"""


def runtime_probe() -> dict:
    """真跑一次插件本体：注册 provider 并取一条 Skill 正文，验证默认资源路径可用。"""
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / "probe.mjs"
        script.write_text(PROBE_JS, encoding="utf-8")
        env = dict(os.environ)
        env["OFFICE_LIB"] = str(OFFICE / "lib" / "index.js")
        proc = subprocess.run(
            ["node", str(script)], cwd=str(RUNTIME), env=env, capture_output=True, text=True, timeout=120
        )
        for line in reversed(proc.stdout.strip().split("\n")):
            if line.startswith("{"):
                return json.loads(line)
        return {"ok": False, "error": (proc.stderr or proc.stdout).strip()[-400:]}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    for path in (SAMPLE, BASE_SAMPLE, BIN, OFFICE, REAL_PATCH):
        check(f"exists:{path.name}", path.exists(), str(path))
    if not all(p.exists() for p in (SAMPLE, BASE_SAMPLE, BIN, OFFICE)):
        print(json.dumps({"cases": len(cases), "failed": 1, "result": "fail"}, ensure_ascii=False))
        return 1

    sample = read(SAMPLE)
    base = read(BASE_SAMPLE)

    # ---- 1. 出厂样例 -----------------------------------------------------------
    yaml_lines = [l for l in sample.split("\n") if l.strip() and not l.lstrip().startswith("#")]
    check("sample_is_top_level_list", yaml_lines[0].startswith("- "), "补丁层须是顶层 YAML 数组")
    check("sample_single_insert", sum(1 for l in yaml_lines if l.strip().startswith("- insert:")) == 1, "只应有一条 insert")
    check("sample_no_entry_override", not any(re.match(r"^- id:", l) for l in yaml_lines), "不应改 preset 内既有条目")
    check("sample_names_office", "'@deepseek-ai/dsh-skill-office'" in sample, "name 须是该插件正式包名")
    check("sample_id_stable", "id: skill-office" in sample, "条目 id 固定为 skill-office")
    check("sample_no_disabled", "disabled" not in sample, "挂载后不应自带 disabled")
    check("base_has_no_office", "skill-office" not in base, "基础标准不得挂办公插件")
    check("base_keeps_upstream_plugins", "skill-filesystem" in base and "tool-skill" in base, "基础标准须保持上游技能通道")
    if REAL_PATCH.exists():
        check("real_matches_sample", read(REAL_PATCH) == sample, "实机 discipline 补丁须与出厂样例一致")

    # ---- 2. 插件包与资源 -------------------------------------------------------
    pkg = json.loads(read(OFFICE / "package.json"))
    check("office_version", pkg.get("version", "").startswith("0.2.1"), pkg.get("version", ""))
    assets = OFFICE / "assets"
    for name in ("office-docx", "office-pptx", "office-xlsx", "scripts"):
        check(f"asset:{name}", (assets / name).exists(), str(assets / name))
    kit_cli = RUNTIME / "node_modules" / "@deepseek-ai" / "libreoffice-kit" / "lib" / "cli.js"
    check("libreoffice_kit_cli", kit_cli.exists(), "默认 cli 路径须能解析到真实文件")

    # ---- 3. 组装结果 -----------------------------------------------------------
    if DSH_HOME.exists():
        for profile, want in (("discipline", True), ("omnidesk-base", False)):
            code, out, err = dump(profile)
            check(f"dump_exit:{profile}", code == 0, err.strip()[-300:])
            has = "skill-office" in out
            check(f"dump_office:{profile}", has is want, "discipline 应有、base 应无" if want else "基础标准不得含 office")
            check(f"dump_no_disable_warn:{profile}", "disabling profile plugin" not in err, err.strip()[-300:])
            if want:
                check(
                    "dump_office_top_level",
                    re.search(r"^- id: skill-office$", out, re.M) is not None,
                    "条目须位于宿主层（顶格），不塞进 preset 的 plugins",
                )
                check(
                    "dump_preset_intact",
                    re.search(r"^- id: preset-standard$", out, re.M) is not None
                    and re.search(r"^      - id: tool-skill$", out, re.M) is not None,
                    "preset-standard 及其 tool-skill 须原样保留",
                )
                block = entry_block(out, "skill-office")
                check("dump_office_enabled", block and "disabled" not in block, block[:200])
                check("dump_office_after_patch_comment", "profiles/discipline/cordis.patch.yml" in out, "条目须来自 Profile 补丁层")
        probe = runtime_probe()
        check("probe_applies", bool(probe.get("ok")), str(probe.get("error", ""))[:300])
        skills = probe.get("skills") or []
        check("probe_three_skills", sorted(skills) == ["office-docx", "office-pptx", "office-xlsx"], str(skills))
        check("probe_provider", probe.get("provider") == "dsh-office", str(probe.get("provider")))
        check("probe_kit_paths", bool(probe.get("content_has_kit")), "加载正文须含 Installed LibreOffice Kit 段落")
    else:
        check("dsh_home_present", False, str(DSH_HOME))

    failed = [c for c in cases if not c["ok"]]
    payload = {
        "plan": "OD-ARCH-104",
        "evidence_level": "静态源码检查 + dsh --dump-config 组装结果 + 插件本体直连探针",
        "dsh_home": str(DSH_HOME),
        "cases": len(cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "failed_cases": failed,
    }

    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[PLUGIN-104] " + c["name"] + ((" :: " + c["detail"]) if not c["ok"] else ""))
    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result")}, ensure_ascii=False))

    if args.json:
        out_path = Path(args.json)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps({**payload, "cases_detail": cases}, ensure_ascii=False, indent=2), encoding="utf-8")

    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

