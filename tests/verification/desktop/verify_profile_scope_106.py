#!/usr/bin/env python3
"""OD-ARCH-106：基础标准 = 干净 DSH 底座，场景/Skill/规则随 Profile 挂载。

这个脚本只做**静态代码检查**：读真实源码与样本，断言关键的隔离边界是否存在。
真实窗口行为（Dock 入口数量、场景预设是否注入 Skill）由窗口脚本覆盖；
声明/规则的读盘与迁移语义由 cargo test 的 `profile_decl` / `profile_rules` 模块测试覆盖。

检查项：
  1. 后端落点：声明与规则按 `<数据根>/profiles/<Profile 名>/` 取，不再读写单文件。
  2. 后端不造默认：`load_decl` 文件缺失返回 `Ok(None)`，函数体内无写盘调用。
  3. 迁移：只对非标准 Profile、目标已存在即跳过、不删除旧文件。
  4. 标准模式下宿主拒绝安装出厂行业规则包，并把原因交给界面。
  5. 前端：无声明时 Dock 只留平台固有入口；场景分类与场景预设一概不出现。
  6. 样本：基础标准 Profile 的插件面与官方 dsh-base 一致（不禁用 skill 插件）。

用法：python3 tests/verification/desktop/verify_profile_scope_106.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

DECL_RS = REPO / "desktop" / "src-tauri" / "src" / "profile_decl.rs"
RULES_RS = REPO / "desktop" / "src-tauri" / "src" / "profile_rules.rs"
LIB_RS = REPO / "desktop" / "src-tauri" / "src" / "lib.rs"
DOCK_TSX = REPO / "desktop" / "src" / "shell" / "Dock.tsx"
CANVAS_TSX = REPO / "desktop" / "src" / "shell" / "HomeCanvas.tsx"
APP_TSX = REPO / "desktop" / "src" / "App.tsx"
SETTINGS_TSX = REPO / "desktop" / "src" / "SettingsPanel.tsx"
API_TS = REPO / "desktop" / "src" / "api.ts"
BASE_PATCH = REPO / "samples" / "profiles" / "omnidesk-base.profile.patch.yml"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def fn_body(text: str, signature: str) -> str | None:
    """取一个 Rust 函数的函数体（按大括号配平），找不到返回 None。"""
    idx = text.find(signature)
    if idx < 0:
        return None
    open_idx = text.find("{", idx)
    if open_idx < 0:
        return None
    depth = 0
    for i in range(open_idx, len(text)):
        ch = text[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return text[open_idx : i + 1]
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    for path in (DECL_RS, RULES_RS, LIB_RS, DOCK_TSX, CANVAS_TSX, APP_TSX, SETTINGS_TSX, API_TS, BASE_PATCH):
        check(f"exists:{path.name}", path.is_file(), str(path))
    if not all(p.is_file() for p in (DECL_RS, RULES_RS, LIB_RS, DOCK_TSX, CANVAS_TSX, APP_TSX, SETTINGS_TSX, API_TS, BASE_PATCH)):
        failed = [c for c in cases if not c["ok"]]
        print(json.dumps({"cases": len(cases), "failed": len(failed), "result": "fail"}, ensure_ascii=False))
        return 1

    decl = read(DECL_RS)
    rules = read(RULES_RS)
    lib = read(LIB_RS)
    dock = read(DOCK_TSX)
    canvas = read(CANVAS_TSX)
    app = read(APP_TSX)
    settings = read(SETTINGS_TSX)
    api = read(API_TS)
    base_patch = read(BASE_PATCH)

    # ---- 1. 后端落点随 Profile -------------------------------------------------
    check("decl_file_name", 'PROFILE_DECL_FILE: &str = "decl.yml"' in decl, "声明文件名应为 decl.yml")
    check("rules_file_name", 'PROFILE_RULES_FILE: &str = "rules.json"' in rules, "规则文件名应为 rules.json")
    check(
        "decl_path_joins_profile_dir",
        "profile_dir.join(PROFILE_DECL_FILE)" in (fn_body(decl, "pub fn decl_path") or ""),
        "decl_path 应把文件名拼到 Profile 目录",
    )
    check(
        "rules_path_joins_profile_dir",
        "profile_dir.join(PROFILE_RULES_FILE)" in (fn_body(rules, "pub fn rules_path") or ""),
        "rules_path 应把文件名拼到 Profile 目录",
    )
    check("profile_asset_dir_uses_profile", 'scope.profiles_root.join(&scope.profile)' in lib, "资产目录须按 Profile 名拼接")

    # ---- 2. 读缺失不造默认 -----------------------------------------------------
    load_body = fn_body(decl, "pub fn load_decl") or ""
    check("load_decl_returns_option", "Result<Option<LoadedDecl>, String>" in decl, "load_decl 应返回 Option")
    check("load_decl_missing_is_none", "NotFound => return Ok(None)" in load_body, "文件缺失必须返回 Ok(None)")
    check("load_decl_never_writes", not re.search(r"fs::write|write_all|create_new|write_atomic", load_body), "load_decl 不得写盘")
    check("decl_no_default_on_load", "DEFAULT_DECL" not in load_body, "加载路径不应引用内置默认声明")
    rules_load_body = fn_body(rules, "pub fn load_rules") or ""
    check("load_rules_never_writes", not re.search(r"fs::write|write_all|create_new|write_atomic", rules_load_body), "load_rules 不得写盘")
    check("load_rules_missing_unloaded", "loaded: false" in rules_load_body, "规则缺失应为 loaded=false")

    # ---- 3. 迁移口径 -----------------------------------------------------------
    for label, text, sig in (
        ("decl", decl, "pub fn migrate_legacy_decl"),
        ("rules", rules, "pub fn migrate_legacy_rules"),
    ):
        body = fn_body(text, sig) or ""
        check(f"{label}_migrate_skips_standard", "profile == standard_profile" in body, "标准 Profile 不迁移")
        check(f"{label}_migrate_never_overwrites", "if target.exists()" in body and "return Ok(false)" in body, "目标已存在即跳过")
        check(f"{label}_migrate_keeps_legacy", not re.search(r"fs::remove_file|remove_dir_all", body), "不得删除旧文件")
    check("migrate_called_before_load", "migrate_legacy_profile_assets(&state);" in lib, "读声明前应先跑一次性迁移")
    migrate_body = fn_body(lib, "fn migrate_legacy_profile_assets") or ""
    check("migrate_skips_standard_mode", "if scope.standard" in migrate_body, "标准模式直接跳过迁移")

    # ---- 4. 标准模式拒绝安装行业规则包 -------------------------------------------
    restore_body = fn_body(lib, "fn profile_rules_restore_default") or ""
    check("restore_rejects_standard", "scope.standard" in restore_body and "Err(" in restore_body, "标准模式应拒绝安装出厂规则包")
    check("rules_view_block_reason", "default_blocked_reason" in rules and "default_blocked_reason" in lib, "宿主应把禁用原因交给界面")
    check("settings_disables_restore", "rules?.default_blocked_reason" in settings, "设置页应按原因禁用按钮")

    # ---- 5. 前端：无声明 = 平台固有入口 -----------------------------------------
    check("dock_has_builtin_items", "BUILTIN_PLATFORM_ITEMS" in dock, "Dock 需要平台固有入口清单")
    check(
        "dock_business_only_on_failure",
        re.search(r":\s*declFailed\s*\n\s*\?\s*DEFAULT_BUSINESS_ITEMS\s*\n\s*:\s*BUILTIN_PLATFORM_ITEMS", dock) is not None,
        "无声明时应显示平台固有入口；只有读取失败才回退完整业务入口",
    )
    check("dock_props_declared_nullable", "declared?: ProfileSceneEntry[] | null" in dock, "声明入参须允许 null（标准模式）")
    check("app_passes_nullable_declared", "declared={decl?.decl.scenes ?? null}" in app, "App 应把无声明表达为 null")
    check("app_has_decl_ready", "declReady" in app, "声明读取完成前不应按默认场景校准")
    check("api_decl_nullable", "Promise<ProfileDeclPayload | null>" in api, "profileDecl 应允许返回 null")

    scene_fn = canvas[canvas.find("function sceneOptions") : canvas.find("export interface HomeCanvasProps")]
    check("scene_options_hides_all_without_decl", ": new Set<string>()" in scene_fn and "declFailed" in scene_fn, "无声明时三个业务分类全部隐藏")
    preset_fn = canvas[canvas.find("const preset: ChatPreset | null") : canvas.find("const categories = useThreadCategories")]
    check("preset_requires_declared_scene", "declaredScenes" in preset_fn and "some((s) => s.id === declId)" in preset_fn, "场景预设须要求入口真的挂在 Profile 上")
    check("preset_requires_loaded_rules", "!rulesView?.loaded" in preset_fn and "return null" in preset_fn, "规则未加载不套场景预设")

    # ---- 5b. 首页把「标准模式」说清楚（S5 界面口径） -------------------------------
    check(
        "home_shows_standard_mode_copy",
        'data-testid="home-standard-mode"' in canvas
        and "标准模式：DSH 底座 + OmniDesk UI，无业务场景" in canvas,
        "首页应显示标准模式说明",
    )
    check(
        "home_standard_copy_gated",
        "declReady && !declFailed && declaredScenes === null" in canvas,
        "标准模式说明须等声明读取完成且确认无声明后才显示",
    )
    check("app_passes_decl_ready", "declReady={declReady}" in app, "App 应把声明读取完成状态传给首页")

    # ---- 6. 基础标准插件面回归官方 ---------------------------------------------
    for plugin in ("skill-filesystem", "tool-skill"):
        m = re.search(r"- id: " + re.escape(plugin) + r"[\s\S]*?(?=\n\s+- id: |\Z)", base_patch)
        check(f"base_enables_{plugin.replace('-', '_')}", m is not None and "disabled: true" not in m.group(0), f"{plugin} 不应禁用")

    failed = [c for c in cases if not c["ok"]]
    payload = {
        "plan": "OD-ARCH-106",
        "evidence_level": "静态代码检查（源码 + 样本）",
        "cases": len(cases),
        "failed": len(failed),
        "result": "pass" if not failed else "fail",
        "failed_cases": failed,
    }

    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[PROFILE-106] " + c["name"] + ((" :: " + c["detail"]) if not c["ok"] else ""))
    print(json.dumps({k: payload[k] for k in ("cases", "failed", "result")}, ensure_ascii=False))

    if args.json:
        out = Path(args.json)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps({**payload, "cases_detail": cases}, ensure_ascii=False, indent=2), encoding="utf-8")

    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
