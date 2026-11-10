#!/usr/bin/env python3
"""OD-UI-202：新拟物材质落地（顶栏 + 首页）静态核对。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据；真实窗口观感另由
macOS 重建 App 截图核对（见 PLANS.md/REVIEW.md 记录）。

用法：python3 tests/verification/desktop/verify_ui_neumorphism_202.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
PUBLIC = REPO / "desktop" / "public"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    theme = read(SRC / "theme.css")
    app = read(SRC / "App.tsx")
    top = read(SRC / "shell" / "TopNav.tsx")
    dock = read(SRC / "shell" / "Dock.tsx")
    window = read(SRC / "shell" / "WindowChrome.tsx")
    chat = read(SRC / "shell" / "HomeChat.tsx")
    canvas = read(SRC / "shell" / "HomeCanvas.tsx")
    category = read(SRC / "shell" / "CategoryPicker.tsx")
    model = read(SRC / "shell" / "ModelPicker.tsx")
    markdown = read(SRC / "markdown.css")
    settings_css = read(SRC / "settings-panel.css")

    # ---- 1. 令牌与双阴影 ------------------------------------------------
    tokens = [
        "--od-neu-bg: #f0efec",
        "--od-neu-surface: #f8f8f6",
        "--od-neu-sunken: #e8e7e3",
        "--od-neu-focus: #3525cd",
    ]
    check("theme_warm_tokens", all(t in theme for t in tokens),
          "暖白灰底材、凹陷面与品牌靛焦点色齐备")
    check("theme_raise_pair",
          "var(--od-neu-raise-1)" in theme
          and "var(--od-neu-raise-2)" in theme
          and "var(--od-neu-raise-3)" in theme,
          "凸起 L1/L2/L3 成对阴影令牌存在")
    check("theme_inset_pair",
          "--od-neu-inset-1:" in theme
          and "--od-neu-inset-2:" in theme
          and theme.count("var(--od-neu-inset-1)") >= 2,
          "凹陷 L1/L2 成对内阴影存在")
    check("theme_focus_visible",
          "outline: 2px solid var(--od-neu-focus)" in theme,
          "控件焦点环 2px 且使用品牌靛")
    check("theme_legacy_plate_materials",
          ".plate-sculpted" in theme
          and "background: var(--od-neu-surface)" in theme
          and ".plate-soft-inset" in theme
          and "box-shadow: var(--od-neu-inset-1)" in theme,
          "旧玻璃构件切到新拟物材质，共享胶囊自动统一")
    check("theme_no_dead_chat_panel", ".neu-chat-panel" not in theme,
          "未使用的整幅对话面板类已移除，避免空态出现巨大空白卡片")
    check("global_surface_map",
          "[class~=\"bg-white\"]" in theme
          and "[class*=\"bg-surface-container-low\"]" in theme
          and "background-color: var(--od-neu-surface) !important" in theme,
          "全应用纯白与旧容器映射到暖灰表面")
    check("global_shadow_map",
          "[class*=\"shadow-[\"]" in theme
          and ".shadow-md," in theme
          and "var(--od-neu-raise-2) !important" in theme,
          "旧任意阴影与 Tailwind 阴影统一映射到新拟物成对阴影")
    check("global_no_blur",
          "[class*=\"backdrop-blur\"]" in theme
          and "backdrop-filter: none !important" in theme,
          "全应用移除残留背景模糊")
    check("settings_pane_neu",
          ".settings-surface .pane" in settings_css
          and "var(--od-neu-raise-2)" in settings_css
          and "var(--od-neu-surface)" in settings_css,
          "设置页主卡片切到暖面与新拟物阴影")
    check("settings_no_lavender_fill",
          "rgba(242, 243, 255" not in settings_css
          and "background: rgba(255, 255, 255" not in settings_css,
          "设置页不再保留旧淡紫/纯白硬编码背景")

    # ---- 1b. 中文四层字体 ------------------------------------------------
    check("type_soft_display_heading",
          "Source Han Sans Rounded SC" in theme
          and "思源柔黑" in theme
          and "--font-display-hero" in theme,
          "大标题优先思源柔黑，缺字体回退思源黑")
    check("type_body_sans",
          "--font-body-md" in theme
          and '"Source Han Sans SC"' in theme
          and '"Source Han Serif SC"' not in theme.split("--font-body-md:")[1].split(";")[0],
          "正文与控件优先思源黑体，宋体不进入界面")
    check("type_kai_out_of_ui",
          '"FZKai-Z03"' in theme
          and "--font-accent-kai" in theme
          and '"FZKai-Z03"' not in theme.split("--font-headline-md:")[1].split(";")[0],
          "方正楷体只保留为引用/落款字体，不进入标题层级")
    check("type_serif_export_only",
          "--font-report-serif" in theme
          and '"Source Han Serif SC"' in theme.split("--font-report-serif:")[1].split(";")[0],
          "思源宋体只留给导出报告与打印文档")
    check("type_cjk_brush_slogan",
          '"Yanshi Xiaxingkai"' in theme
          and "--font-slogan" in theme
          and '--font-slogan: "Inter Variable"' in theme,
          "标语优先演示夏行楷，但拉丁先走 Inter，避免英文变成手写体")
    check("type_body_line_height", "leading-[1.65]" in app,
          "全局正文行高 1.65")
    check("type_slogan_bold_empty_state",
          "font-slogan" in chat
          and "font-weight: 700" in theme
          and "font-slogan" in theme,
          "新对话空态标语使用加粗行楷，标语不再发虚")
    check("type_slogan_self_hosted",
          '@font-face' in theme
          and 'font-family: "Slidexiaxing"' in theme
          and (PUBLIC / "fonts" / "Slidexiaxing-Regular.ttf").is_file(),
          "演示夏行楷已本地自托管，首页标语不依赖系统字体安装")
    check("type_slogan_license",
          (PUBLIC / "fonts" / "OFL-SlideFont.txt").is_file(),
          "随包保留演示夏行楷的 SIL OFL 授权文本")
    check("type_table_controls_sans",
          "font-family: var(--font-body-sm)" in markdown,
          "Markdown 表格回到无衬线，避免密集数据用宋体")
    check("type_mono_cjk_fallback",
          '"IBM Plex Mono", "Source Han Sans SC"' in theme
          and "--font-mono:" in theme
          and "--font-label-caps:" in theme,
          "等宽/标签字体都接中文黑体回退，避免日历与卡片标签落到宋体")
    check("type_markdown_body_font",
          ".md-body {" in markdown
          and "font-family: var(--font-body-md)" in markdown,
          "Markdown 正文显式使用界面字体，不依赖父级继承")

    # ---- 2. 顶栏 --------------------------------------------------------
    check("topnav_raised_bar",
          "neu-nav" in top and "rounded-[30px]" in top,
          "顶栏为凸起圆角长条")
    check("topnav_inset_search",
          "neu-inset-field" in top and "neu-inset-pill" in top,
          "搜索与 Profile 徽标为凹陷材质")
    check("topnav_tactile_buttons",
          "neu-chip" in top and "neu-icon-btn" in top and "neu-kbd" in top,
          "日期、设置/通知与快捷键键帽为凸起")
    check("topnav_popovers_solid",
          top.count("neu-popover") >= 2 and "backdrop-blur" not in top,
          "日期与 Profile 弹层使用实色凸起面，不再模糊")
    check("topnav_no_glass",
          "backdrop-blur-2xl" not in top and "bg-surface-container-lowest/80" not in top,
          "顶栏不再使用玻璃拟态")

    # ---- 3. 应用底材与首页 ----------------------------------------------
    check("app_warm_base",
          "app-scape-base" in app and "app-scape-mist" in app and "home-scape.png" in app,
          "应用底材为暖白风景层，首页插画单独挂载")
    check("home_composer_inset",
          "neu-composer" in chat and "neu-send-btn" in chat,
          "输入区为带光晕的凸起面板，发送按钮凸起")
    check("home_messages_materials",
          "neu-msg-accent" in chat and "neu-msg-assistant" in chat,
          "用户消息轻凸、助手消息浅面，不全做成厚卡片")
    check("home_no_backdrop",
          "backdrop-blur" not in chat and "backdrop-blur" not in canvas,
          "首页对话与画布不再使用背景模糊")
    check("pickers_match_topnav",
          "neu-popover" in category and "neu-popover" in model
          and "backdrop-blur" not in category and "backdrop-blur" not in model,
          "归类与模型弹层与顶栏弹层同材质")

    # ---- 4. Dock 与窗口控制 ----------------------------------------------
    check("dock_free_floating",
          "neu-rail" in dock and "neu-dock-btn" in dock and "neu-dock-active" in dock,
          "左侧入口收进胶囊导轨，当前项为品牌靛实心圆")
    check("dock_no_container_glass",
          "bg-surface-container-lowest/95" not in dock and "shadow-[" not in dock,
          "Dock 不再有玻璃胶囊容器")
    check("window_controls_tactile",
          "neu-window-btn" in window and "neu-window-close" in window,
          "窗口按钮凸起，关闭按钮保留危险色悬停")

    passed = sum(1 for _, ok, _ in CHECKS if ok)
    total = len(CHECKS)
    for name, ok, detail in CHECKS:
        print(("PASS" if ok else "FAIL") + f" {name}" + (f" — {detail}" if detail else ""))
    print()
    print(f"{passed}/{total} checks passed")

    if args.json:
        args.json.write_text(
            json.dumps(
                {
                    "task": "OD-UI-202",
                    "passed": passed,
                    "total": total,
                    "checks": [
                        {"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS
                    ],
                },
                ensure_ascii=False,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
