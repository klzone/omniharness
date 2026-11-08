#!/usr/bin/env python3
"""界面精简（2026-11-08）：去掉历史会话、英文副标题与解释性状态文案。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：
  1. 标准与专业 Profile 都不再有「历史会话」入口；旧 decl 仍可解析，载入后丢弃该入口。
  2. 文案写作页与工作记录页不再渲染英文副标题、卡序号或口径说明。
  3. 首页不再出现技术味状态串（IPC ON/OFF、Token 上下文、工作目录）与未开放的语音按钮。

用法：python3 tests/verification/desktop/verify_ui_simplify_v1.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
RUST = REPO / "desktop" / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    dock = read(SRC / "shell" / "Dock.tsx")
    app = read(SRC / "App.tsx")
    canvas = read(SRC / "shell" / "HomeCanvas.tsx")
    chat = read(SRC / "shell" / "HomeChat.tsx")
    top = read(SRC / "shell" / "TopNav.tsx")
    studio = read(SRC / "shell" / "WritingStudio.tsx")
    strip = read(SRC / "shell" / "SceneStatusStrip.tsx")
    moments = read(SRC / "scenes.ts")
    writing_lib = read(SRC / "writingStudio.ts")
    scene_page = read(SRC / "shell" / "ScenePage.tsx")
    settings = read(SRC / "SettingsPanel.tsx")
    quad = read(SRC / "shell" / "WeeklyQuadrants.tsx")
    timeline = read(SRC / "shell" / "WeeklyTimeline.tsx")
    sample = read(REPO / "samples" / "profiles" / "discipline.yml")
    decl = read(RUST / "profile_decl.rs")
    starter = read(RUST / "profile_starter.rs")

    # ---- 1. 历史会话入口移除 ------------------------------------------------
    check("dock_no_sessions_item", 'scene: "sessions"' not in dock and "历史会话" not in dock,
          "Dock 不再有历史会话入口")
    check("dock_platform_items_two",
          'label: "工作台首页"' in dock and "BUILTIN_KNOWLEDGE_ITEM" in dock
          and "BUILTIN_PLATFORM_ITEMS" in dock,
          "基础标准只剩首页与知识库")
    check("dock_tooltip_label_only",
          "aria-label={item.label}" in dock and "item.note" not in dock,
          "悬停提示只显示入口名，不再带过程说明")
    check("app_no_sessions_branch", 'scene === "sessions"' not in app and "Workspace" not in app,
          "App 不再路由历史会话")
    check("app_managed_no_sessions", '"sessions"' not in app, "声明校准列表不含 sessions")
    check("decl_retired_scene_filtered",
          "RETIRED_SCENES" in decl and "hide_retired" in decl and "sessions" in decl,
          "旧声明允许解析，载入后丢弃 sessions")
    check("starter_no_sessions", "sessions" not in starter and '"project"' in starter,
          "自建 Profile 骨架不再写 sessions")
    check("sample_no_sessions", "id: sessions" not in sample and "历史会话" not in sample,
          "出厂专业 Profile 声明不含 sessions")
    check("canvas_no_sessions_nav", '"sessions"' not in canvas, "首页待办不再跳历史会话")
    check("settings_no_sessions_copy", "历史会话" not in settings,
          "设置页不再提示历史会话入口")

    # ---- 2. 写作页与工作记录页减负 ------------------------------------------
    check("writing_no_subtitle_props", "subtitle" not in studio, "写作页不再接副标题")
    check("writing_no_jargon_titles",
          "状态管线" not in studio and "灵感架" not in studio,
          "改用「稿件」「写作对话」")
    check("writing_no_default_note",
          "STATUS_DEFAULT_NOTE" not in studio and "STATUS_DEFAULT_NOTE" not in writing_lib,
          "不再常驻口径说明")
    check("writing_status_no_english",
          'subtitle: "DRAFTS"' not in writing_lib and 'subtitle: "FINISHED"' not in writing_lib,
          "状态数据不再带英文小标")
    check("weekly_no_subtitles",
          "subtitle" not in quad and "subtitle" not in timeline,
          "工作记录区块不再带英文副标题")
    check("weekly_axes_chinese",
          'label: "紧急"' in quad and 'label: "重要"' in quad
          and "URGENT" not in quad and "IMPORTANT" not in quad,
          "四象限轴标改为中文")
    check("weekly_band_no_subtitle",
          "subtitle" not in strip.split('scene.key === "weekly" ? (')[1].split("</>")[0],
          "四格指标不再带英文副标题")
    check("band_no_index_numbers", 'index="' not in strip and "index:" not in strip,
          "卡面不再叠 01–04 序号")
    check("scenes_no_english_labels",
          not any(s in moments for s in ("WEEKLY LOG", "RECORDS", "OVERDUE", "STUDIO")),
          "场景配置不再保留英文小标")
    check("scene_page_no_legacy_titles",
          "WritingStudioHead" not in scene_page and "状态管线" not in scene_page
          and "灵感架" not in scene_page,
          "场景页不再渲染旧标题或栏目说明")

    # ---- 3. 首页去技术味 -----------------------------------------------------
    check("chat_placeholder_plain", "输入消息…" in chat and "协同探索" not in chat,
          "输入框只写「输入消息…」")
    check("chat_no_voice_entry", 'data-testid="voice-button"' not in chat
          and "语音输入需要配置语音模型端点" not in chat,
          "未开放的语音按钮与说明已移除")
    check("chat_no_tech_status_row",
          "Token 上下文" not in chat and "模型权限：" not in chat and "工作目录：" not in chat,
          "不再常驻权限/用量/目录状态")
    check("nav_no_ipc_words",
          "IPC ON" not in top and "IPC OFF" not in top and 'aria-label={engineHealthy ? "已连接" : "未连接"}' in top,
          "顶栏只用圆点表示连接状态")

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "UI-SIMPLIFY-2026-11-08",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS],
    }
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
