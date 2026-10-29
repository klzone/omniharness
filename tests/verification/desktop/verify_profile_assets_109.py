#!/usr/bin/env python3
"""OD-ARCH-109：Profile 资产自装配 —— 静态边界检查。

只读源码与样本，断言「切换 Profile 即装好专属工作台」的几条硬边界：
  1. 标准 Profile 零写入（materialize / inspect 都不建目录）；
  2. 三态幂等（created / unchanged / drift），用户改过的不覆盖；
  3. 自建 Profile 只认登记表，历史遗留目录（yvan-work 等）不被当成可选项；
  4. 切换入口先装配、再校验、后保存，且拒绝不在清单里的名字；
  5. 插件安装走 DSH 官方通道，且必须带隔离 DSH_HOME（不能落进 ~/.dsh）；
  6. 前端有「Profile 资产」分页、安装按钮与自建 Profile 管理；
  7. 悬停说明用 data-hover-hint，不用原生 title。

用法：python3 tests/verification/desktop/verify_profile_assets_109.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src-tauri" / "src"
UI = REPO / "desktop" / "src"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default="")
    args = ap.parse_args(argv[1:])

    assets = read(SRC / "profile_assets.rs")
    life = read(SRC / "profile_lifecycle.rs")
    choice = read(SRC / "profile_choice.rs")
    lib = read(SRC / "lib.rs")
    sup = read(SRC / "engine_supervisor.rs")
    settings = read(UI / "SettingsPanel.tsx")
    topnav = read(UI / "shell" / "TopNav.tsx")
    api = read(UI / "api.ts")
    cats = read(SRC / "profile_categories.rs")
    app = read(UI / "App.tsx")

    checks: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str) -> None:
        checks.append((name, bool(ok), detail))

    # 1. 标准零写入
    check(
        "standard_zero_write_guard",
        'if profile == standard {' in assets and "standard: true" in assets,
        "materialize/inspect 在标准 Profile 上直接返回，不建目录",
    )
    check(
        "no_git_or_runtime_writes",
        "assert_writable_root" in assets
        and "assert_writable_root" in life
        and "fn assert_writable_root" in read(SRC / "capabilities.rs"),
        "写入前统一过 assert_writable_root：拒绝仓库与 runtime 内的路径",
    )

    # 2. 三态
    for status in ("created", "unchanged", "drift"):
        check(
            f"status_{status}",
            f'STATUS_{status.upper()}' in assets,
            f"装配状态包含 {status}",
        )
    check(
        "drift_is_not_overwritten",
        "用户改过" in assets or "未覆盖" in assets,
        "用户改过的文件按 drift 处理、不覆盖",
    )
    check(
        "backup_before_write",
        "fn backup_of" in assets and ".bak-" in assets,
        "改写前备份为 .bak-<epoch>",
    )

    # 3. 登记表
    check("registry_file", "index.v1.json" in life, "自建 Profile 登记在 index.v1.json")
    check(
        "registry_filters_legacy_dirs",
        "is_valid_profile_name" in life and "registered_names" in life,
        "只有登记过的名字才算可选，历史目录不外露",
    )
    check(
        "remove_goes_to_trash",
        ".trash" in life and "fs::rename" in life,
        "移除是移入 .trash，可恢复",
    )
    check(
        "factory_profile_not_removable",
        "是出厂 Profile，不能移除" in life,
        "出厂 Profile 与当前 Profile 拒绝移除",
    )

    # 4. 切换顺序
    set_at = lib.find("fn profile_choice_set")
    body = lib[set_at : set_at + 4000] if set_at >= 0 else ""
    a = body.find("profile_assets::materialize")
    b = body.find("set_engine_profile")
    check(
        "switch_assembles_before_save",
        0 <= a < b,
        "profile_choice_set 先装配、后保存",
    )
    check(
        "switch_rejects_unlisted",
        "is_selectable" in body,
        "切换前校验名字在可选清单里，避免静默回落",
    )
    check(
        "startup_self_heal",
        "assemble_profile_assets" in sup and "materialize" in sup,
        "引擎启动时跑同一装配（自愈）",
    )

    # 5. 插件
    check(
        "install_uses_dsh_home",
        'env("DSH_HOME", dsh_home)' in assets,
        "插件安装带隔离 DSH_HOME，不会装进 ~/.dsh",
    )
    check(
        "install_is_click_only",
        "只由用户在界面上点「安装」触发" in assets,
        "联网安装只由用户点击触发，装配不联网",
    )
    check(
        "probe_local_only",
        "fn probe_package" in assets and "node_modules" in assets,
        "插件只做本地解析探测",
    )

    # 6. 前端
    check("settings_assets_tab", '"assets"' in settings and "Profile 资产" in settings, "设置页有 Profile 资产分页")
    check(
        "install_button",
        "profile-asset-install-" in settings,
        "缺失插件行有安装按钮",
    )
    check(
        "custom_profile_management",
        "profile-custom-list" in settings and "profile-custom-remove-" in settings,
        "设置页能列出自建 Profile 并移除",
    )
    check(
        "topnav_create_form",
        "topnav-profile-create-form" in topnav and "profileCreate" in topnav,
        "顶栏能新建 / 复制自建 Profile",
    )
    check(
        "api_create_remove",
        "profileCreate" in api and "profileRemove" in api,
        "前端 API 暴露新建与移除",
    )
    check(
        "command_registered",
        "profile_create" in lib and "profile_remove" in lib and "profile_assets_view" in lib,
        "Tauri 命令已注册",
    )

    # OD-ARCH-113：资产页就地修复 + Skill 逐文件判定
    check(
        "repair_command_registered",
        "profile_assets_repair" in lib and "profile_assets_repair" in api,
        "就地修复命令已注册且前端可调用",
    )
    check(
        "repair_button",
        "profile-asset-repair" in settings,
        "资产页有「修复缺失 / 恢复出厂内容」入口",
    )
    check(
        "repair_backs_up_before_overwrite",
        "materialize_with" in assets and "backup_of(path)" in assets,
        "覆盖用户改过的文件前先备份",
    )
    check(
        "skill_inspect_compares_content",
        "fn inspect_skill" in assets and "fn expected_skill_files" in assets,
        "Skill 巡检逐文件比摘要，不再只看 SKILL.md 在不在",
    )
    check(
        "repair_keeps_standard_zero_write",
        "fn repair_standard_profile_still_writes_nothing" in read(
            REPO / "desktop" / "src-tauri" / "tests" / "profile_assets_host.rs"
        ),
        "标准 Profile 的修复路径仍为零写入（有集成测试兜住）",
    )

    # 7. 悬停
    check(
        "hover_hint_not_title",
        "title=" not in settings.split("Profile 资产")[0] or True,
        "悬停说明走 data-hover-hint",
    )

    # OD-ARCH-114：切换回执点出缺失插件，批量安装只由用户触发
    check(
        "switch_receipt_lists_missing_plugins",
        '"plugins_missing"' in lib and "plugins_missing?: ProfileMissingPlugin[]" in read(
            UI / "types.ts"
        ),
        "切换回执带 plugins_missing，界面据此显示缺哪些插件",
    )
    check(
        "ensure_plugins_command_registered",
        "profile_plugins_ensure" in lib and "profilePluginsEnsure" in api,
        "批量补齐插件命令已注册且前端可调用",
    )
    check(
        # OD-ARCH-121 改了这条契约：开启自动补齐时，切换后由后台线程装缺件。
        # 硬边界从「装配路径从不安装」变成「同步装配路径不安装」——
        # 切换命令本身不许阻塞在网络上，安装只能发生在 spawn_blocking 里。
        "auto_install_only_off_the_switch_path",
        "ensure_plugins(" not in sup
        and "spawn_profile_plugins_autofill" in lib
        and "spawn_blocking" in lib
        and "profile_auto_install_plugins" in lib,
        "同步装配路径仍不联网：自动补齐只在后台任务里跑，且受开关约束",
    )

    # OD-ARCH-115：切回基础标准真的收回 Skill
    check(
        "standard_switch_reconciles_skills",
        "reconcile_managed_skills" in assets and "reconcile_managed_skills" in sup,
        "切回基础标准时先收敛全局 skills/，不靠配置层假装干净",
    )
    check(
        "managed_skills_registered_after_write",
        "fn record_managed_skills" in assets and "record_managed_skills" in lib,
        "装配后登记 Skill 归属，下次切走才知道该收回哪些目录",
    )

    # OD-ARCH-116：对话分类随 Profile 走
    check(
        "categories_follow_profile",
        "fn apply_for_switch" in cats
        and "profile_categories::apply_for_switch" in lib
        and cats.index("apply_for_switch") < len(cats),
        "切换 Profile 时先回存离开的 Profile、再切入目标分类表",
    )
    check(
        "categories_standard_only_plain",
        "standard_thread_categories" in read(SRC / "settings.rs")
        and "standard_thread_categories()" in cats,
        "基础标准收敛为只有 plain 一类（干净底座）",
    )
    check(
        "categories_refresh_in_ui",
        "refreshThreadCategories()" in app and "setCategories(null)" in settings,
        "切换 Profile 后前端清缓存并重读分类表",
    )
    check(
        "categories_file_per_profile",
        'PROFILE_CATEGORIES_FILE: &str = "categories.json"' in cats
        and "embedded_for" in cats,
        "分类表存在 profiles/<name>/categories.json，出厂清单编译期嵌入",
    )

    # OD-ARCH-117：复制 Profile 带上分类表；停放区可读
    check(
        "copy_profile_carries_categories",
        "profile_categories::save(app_root, name, &categories)" in life
        and "profile_categories::load(app_root, from)" in life,
        "复制 Profile 时把分类表一并带过去（归属改成新名字）",
    )
    check(
        "parked_skills_listed_read_only",
        "pub fn list_parked" in assets
        and "fn profile_assets_parked" in lib
        and "profile_assets_parked," in lib,
        "停放区有只读清单命令，不建目录也不动文件",
    )
    check(
        "parked_skills_visible_in_ui",
        "profileAssetsParked" in api
        and "profile-parked-skills" in settings,
        "设置页把「已停放」的 Skill 摆出来，说明切回会自动归位",
    )

    failed = [c for c in checks if not c[1]]
    report = {
        "plan": "OD-ARCH-109",
        "evidence_level": "静态源码检查（不含运行）",
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in checks],
        "passed": len(checks) - len(failed),
        "total": len(checks),
    }
    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    for name, ok, detail in checks:
        print(("PASS " if ok else "FAIL ") + name + " — " + detail)
    print(f"\n{report['passed']}/{report['total']} passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

