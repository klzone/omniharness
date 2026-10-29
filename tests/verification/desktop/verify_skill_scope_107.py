#!/usr/bin/env python3
"""OD-ARCH-107：能力管理页说清「本 Profile 预注入哪些 Skill、哪些只是可按需加载」。

静态检查：读真实源码，断言三件事仍然是分开的——
  1. 后端 Skill 目录仍是全局一份（不按 Profile 隔离，破坏 base=官方 DSH 的边界）；
  2. 「预注入」只由 rules.json 的 skill_bindings + max_injected 决定，且有效顺序可算；
  3. 前端把两种状态分成两组显示，标准模式与规则损坏各有明确文案，绑定缺失会暴露。

真实行为由 cargo test 的 capabilities 模块测试覆盖（scope_marks_effective_and_missing_bindings）。

用法：python3 tests/verification/desktop/verify_skill_scope_107.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

CAPS_RS = REPO / "desktop" / "src-tauri" / "src" / "capabilities.rs"
LIB_RS = REPO / "desktop" / "src-tauri" / "src" / "lib.rs"
SETTINGS_TSX = REPO / "desktop" / "src" / "SettingsPanel.tsx"
TYPES_TS = REPO / "desktop" / "src" / "types.ts"
CSS = REPO / "desktop" / "src" / "settings-panel.css"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def fn_body(text: str, signature: str) -> str:
    idx = text.find(signature)
    if idx < 0:
        return ""
    open_idx = text.find("{", idx)
    if open_idx < 0:
        return ""
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_idx : i + 1]
    return ""


def code_of(text: str) -> str:
    """去掉 // /// //! 注释后的源码，避免注释里提到 rules.json 造成误判。"""
    out = []
    for line in text.split("\n"):
        if line.lstrip().startswith("//"):
            continue
        out.append(line)
    return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    for path in (CAPS_RS, LIB_RS, SETTINGS_TSX, TYPES_TS, CSS):
        check(f"exists:{path.name}", path.is_file(), str(path))
    if not all(p.is_file() for p in (CAPS_RS, LIB_RS, SETTINGS_TSX, TYPES_TS, CSS)):
        print(json.dumps({"cases": len(cases), "failed": 1, "result": "fail"}, ensure_ascii=False))
        return 1

    caps = read(CAPS_RS)
    lib = read(LIB_RS)
    settings = read(SETTINGS_TSX)
    types = read(TYPES_TS)
    css = read(CSS)

    # ---- 1. Skill 目录仍然全局一份 ---------------------------------------------
    list_body = fn_body(caps, "pub fn list_skills")
    check("skills_dir_shared", 'dsh_home.join("skills")' in list_body, "Skill 目录必须是所有 Profile 共用的一份")
    check("skills_dir_not_per_profile", "profiles" not in list_body, "列表不得按 Profile 目录取 Skill（会破坏 base=官方 DSH）")

    # ---- 2. 预注入口径来自规则包 -----------------------------------------------
    scope_body = fn_body(caps, "pub fn apply_scope")
    check("scope_marks_effective", "order <= scene.max_injected" in scope_body, "effective 须按 max_injected 判定")
    check("scope_reports_missing", "MissingBinding" in scope_body and "resolved" in scope_body, "绑定缺失须上报")
    check("scope_does_not_read_files", "read_to_string" not in scope_body and "fs::" not in scope_body, "apply_scope 只做映射，不读文件")
    check(
        "capabilities_does_not_parse_rules",
        "profile_rules::" not in code_of(caps) and "rules.json" not in code_of(caps),
        "capabilities 模块不得自己解析规则文件（避免双份解析）",
    )
    check("load_with_scope_exists", "pub fn load_with_scope" in caps, "应提供带作用域的读取入口")

    # ---- 2b. 命令层复用同一条规则读取 -------------------------------------------
    check("single_rules_reader", "fn profile_rules_view(" in lib, "规则读取应抽成唯一实现")
    check("rules_load_uses_shared", "profile_rules_view(&state)" in fn_body(lib, "fn profile_rules_load"), "profile_rules_load 应复用共享实现")
    scope_fn = fn_body(lib, "fn skill_scope")
    check("skill_scope_uses_shared", "profile_rules_view(state)" in scope_fn, "skill_scope 应复用共享实现")
    check("skill_scope_marks_standard", "profile_asset_scope(state).standard" in scope_fn, "作用域须带上是否标准模式")
    check("capability_view_passes_scope", "load_with_scope(" in fn_body(lib, "fn capability_view"), "能力视图应传入作用域")

    # ---- 3. 前端分组口径 -------------------------------------------------------
    check("group_preinjected", 'data-testid="skill-group-preinjected"' in settings, "应有「本 Profile 预注入」分组")
    check("group_ondemand", 'data-testid="skill-group-ondemand"' in settings, "应有「可被模型按需加载」分组")
    check(
        "groups_partitioned_by_effective",
        settings.count("bindings.some((b) => b.effective)") >= 2,
        "两个分组须按 effective 互斥划分",
    )
    check("standard_copy", 'data-testid="skill-scope-standard"' in settings and "不预注入任何 Skill" in settings, "标准模式须明说零预注入")
    check("standard_copy_keeps_ondemand", "按需加载" in settings, "标准模式须说明仍可按需加载")
    check("broken_rules_copy", 'data-testid="skill-scope-broken"' in settings, "规则损坏时须如实提示")
    check("empty_rules_copy", 'data-testid="skill-scope-empty"' in settings, "无规则包时须如实提示")
    # OD-ARCH-108：绑定按流程拆，界面改成说明「同一场景的各流程各绑自己的底线技能」。
    check(
        "flow_scoped_copy",
        "各绑自己的底线技能" in settings,
        "界面须说明同一场景的不同流程各绑自己的底线技能",
    )
    check(
        "no_stale_first_binding_copy",
        "只取该场景绑定里的第 1 个" not in settings,
        "旧的「取第 1 个」口径已不适用，须删除",
    )
    check(
        "note_preinjected_count",
        "caps.preinjected.length" in settings,
        "既有规则包时须给出预注入总数（不能只说「标记为预注入的」）",
    )
    check("missing_bindings_ui", 'data-testid="skill-missing-bindings"' in settings, "绑定缺失须在界面暴露")
    check("scene_label_mapped", "function sceneLabel(" in settings and "SCENES[key].label" in settings, "场景键须显示为人能读的名字")
    check(
        "flow_scene_labels",
        '"weekly-note"' in settings and '"weekly-report"' in settings,
        "weekly-note / weekly-report 须映射成可读的场景名，不能露出原始键名",
    )

    # ---- 4. 类型与样式 ---------------------------------------------------------
    check("type_skill_id", "id: string;" in types and "bindings: SkillBinding[];" in types, "Skill 条目须带 id 与 bindings")
    check("type_view_scope", "preinjected: string[];" in types and "missing_bindings: MissingSkillBinding[];" in types, "能力视图须带作用域字段")
    check("css_square_tags", "border-radius: 0;" in css and ".settings-surface .skill-tag" in css, "场景标签用方形（不要圆角）")

    failed = [c for c in cases if not c["ok"]]
    payload = {
        "plan": "OD-ARCH-107",
        "evidence_level": "静态代码检查（源码）",
        "cases": len(cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "failed_cases": failed,
    }

    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[SKILL-107] " + c["name"] + ((" :: " + c["detail"]) if not c["ok"] else ""))
    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result")}, ensure_ascii=False))

    if args.json:
        out = Path(args.json)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps({**payload, "cases_detail": cases}, ensure_ascii=False, indent=2), encoding="utf-8")

    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())

