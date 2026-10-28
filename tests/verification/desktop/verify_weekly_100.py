#!/usr/bin/env python3
"""OD-ARCH-100：周报页五处视觉与减冗修订的静态断言。

对应五处改动：
  S01 指标卡图标去掉底色座，图标放大到 56px；
  S02 英文正文/标题换 Inter，等宽小标换 IBM Plex Mono（仍为自托管）；
  S03 去向开关改圆形，去掉可见的「进周报」文字，语义留在悬停/读屏；
  S04 成果库去掉搜索框与 query 过滤（顶部导航已有搜索）；
  S05 去掉「另有 N 个归档文件名不符合约定」提示。

证据等级：静态代码检查（读源码文本）。不开窗口、不调用模型、不读真实工作记录。
窗口级证据见 `evidence/od-arch-100/`。

用法：python3 tests/verification/desktop/verify_weekly_100.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def strip_comments(src: str) -> str:
    """去掉 // 与 /* */ 注释：断言针对渲染出来的内容，不能被解释性注释命中。"""
    without_block = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"(?m)^\s*//.*$", "", without_block)


def band_metric_body(src: str) -> str:
    start = src.find("function BandMetric(")
    if start < 0:
        return ""
    end = src.find("function WritingMetric(", start)
    return src[start:end if end > 0 else len(src)]


def band_tones_body(src: str) -> str:
    start = src.find("const BAND_TONES")
    end = src.find("function BandMetric(", start)
    return src[start:end] if start >= 0 and end > start else ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    strip = (SRC / "shell" / "SceneStatusStrip.tsx").read_text(encoding="utf-8")
    timeline = (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8")
    art = (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8")
    scene = (SRC / "shell" / "ScenePage.tsx").read_text(encoding="utf-8")
    theme = (SRC / "theme.css").read_text(encoding="utf-8")
    styles = (SRC / "styles.css").read_text(encoding="utf-8")
    md_css = (SRC / "markdown.css").read_text(encoding="utf-8")
    main_tsx = (SRC / "main.tsx").read_text(encoding="utf-8")
    pkg = json.loads((DESKTOP / "package.json").read_text(encoding="utf-8"))

    band = band_metric_body(strip)
    tones = band_tones_body(strip)
    band_jsx = strip_comments(band)
    timeline_jsx = strip_comments(timeline)
    art_jsx = strip_comments(art)

    # --- S01 图标去底、放大 ---
    chip_lines = [ln.strip() for ln in tones.splitlines() if 'chip: "' in ln]
    check("s01_five_chips", len(chip_lines) == 5, f"找到 {len(chip_lines)} 个 chip 字段")
    check(
        "s01_chip_has_no_background",
        not any("bg-" in ln for ln in chip_lines) and all("text-" in ln for ln in chip_lines),
        "chip 只剩文字色，没有任何背景色类",
    )
    check(
        "s01_icon_full_size",
        'Icon name={icon} className={"w-14 h-14 "' in band_jsx,
        "图标本体 56px",
    )
    check(
        "s01_no_rounded_seat",
        'className={"w-14 h-14 rounded-2xl' not in band_jsx
        and "shadow-[0_2px_10px_-5px" not in band_jsx
        and 'Icon name={icon} className={"w-14 h-14 "' in band_jsx,
        "图标不再套圆角色块与投影",
    )
    check("s01_column_width_kept", 'data-band-icon-column' in band and "w-14" in band, "列宽仍为 56px")
    check(
        "s01_writing_metric_untouched",
        "data-band-icon-column" not in strip[strip.find("function WritingMetric(") :],
        "写作页指标卡未改动",
    )

    # --- S02 英文字体换厚 ---
    check("s02_packages_declared",
          "@fontsource-variable/inter" in pkg.get("dependencies", {})
          and "@fontsource/ibm-plex-mono" in pkg.get("dependencies", {}),
          "Inter 与 IBM Plex Mono 进依赖")
    check("s02_theme_uses_inter",
          theme.count('"Inter Variable", "Inter"') >= 8 and '"Sora"' not in theme
          and '"Plus Jakarta Sans"' not in theme,
          "所有 sans 令牌改用 Inter，旧字族清空")
    check("s02_theme_uses_plex_mono",
          "--font-label-caps: \"IBM Plex Mono\"" in theme and "JetBrains Mono" not in theme,
          "等宽令牌改为 IBM Plex Mono")
    check("s02_styles_mono_swapped",
          '"IBM Plex Mono"' in styles and "JetBrains Mono" not in styles,
          ".mono 同步换字族")
    check("s02_fonts_self_hosted",
          '@fontsource-variable/inter/wght.css' in main_tsx
          and "@fontsource/ibm-plex-mono/latin-500.css" in main_tsx
          and "http" not in main_tsx.split("import \"./theme.css\"")[0],
          "字体来自本地包，运行时零外链")
    check("s02_old_fonts_unimported",
          "@fontsource/sora/" not in main_tsx and "@fontsource/plus-jakarta-sans/" not in main_tsx,
          "不再引入旧字族文件")
    check("s02_old_font_packages_removed",
          not {"@fontsource/sora", "@fontsource/plus-jakarta-sans",
               "@fontsource/jetbrains-mono"} & set(pkg.get("dependencies", {})),
          "旧字族依赖已从 package.json 移除，不再随包分发")
    check("s02_markdown_code_uses_plex_mono",
          '"IBM Plex Mono"' in md_css and "JetBrains Mono" not in md_css,
          "正文代码块的内联等宽字体同步换族")
    check("s02_cjk_fallback_kept",
          '"Noto Sans SC"' in theme and '"PingFang SC"' in theme,
          "中文仍走系统字体回退栈")

    # --- S03 圆形去向开关 ---
    check("s03_round_button", "rounded-full" in timeline_jsx, "去向开关为圆形")
    check(
        "s03_no_visible_label",
        '"保存中…" : "进周报"' not in timeline_jsx and ">进周报</span>" not in timeline_jsx,
        "不再有可见的「进周报」文字",
    )
    check(
        "s03_tooltip_kept",
        "已进周报：计入「进周报条目」" in timeline_jsx and "不进周报：仍留在本周记录里" in timeline_jsx,
        "悬停提示保留两种状态的说明",
    )
    check(
        "s03_accessibility_kept",
        'role="switch"' in timeline_jsx and "aria-checked={checked}" in timeline_jsx
        and "aria-label={(checked ?" in timeline_jsx and "focus-visible:ring-2" in timeline_jsx,
        "角色、勾选态、标签、焦点环都还在",
    )
    check("s03_failure_still_inline", "failure" in timeline_jsx and "text-error" in timeline_jsx)
    check("s03_missing_destination_static", "无去向" in timeline_jsx and "border-dashed" in timeline_jsx)

    # --- S04 成果库去搜索 ---
    check("s04_no_search_input", 'placeholder="搜索成果或归档"' not in art_jsx)
    check("s04_no_query_state", "const [query" not in art_jsx and "setQuery" not in art_jsx)
    check("s04_no_text_filter", "toLowerCase().includes(" not in art_jsx, "不再按输入文本过滤")
    check(
        "s04_toolbar_single_row",
        "flex flex-wrap min-w-0 items-center gap-1.5" in art_jsx and "data-artifact-range" in art_jsx,
        "工具栏合并为一行",
    )
    check(
        "s04_keeps_range_view_subdirs",
        'data-artifact-view="cards"' in art_jsx and 'data-artifact-view="list"' in art_jsx
        and "checked={includeSubdirs}" in art_jsx and "onToggleSubdirs" in art_jsx,
        "卡片/列表、范围与含子目录保留",
    )
    check("s04_page_resets_on_range", "}, [range]);" in art_jsx, "翻页状态只随范围重置")
    check("s04_all_reuse_sites_intact", scene.count("<ArtifactExplorer") >= 3, "四个复用点仍在")

    # --- S05 去掉归档提示 ---
    check("s05_text_gone", "不符合约定" not in art_jsx)
    check("s05_prop_gone", "archiveSkipped" not in art_jsx, "prop 与默认值都已移除")
    check("s05_caller_cleaned", "archiveSkipped" not in scene, "调用点不再传该 prop")
    check("s05_archive_error_kept", "archiveError" in art_jsx and "读取历史归档失败" in art_jsx,
          "真正读失败的原因仍如实显示")

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-100",
        "passed": passed,
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
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

