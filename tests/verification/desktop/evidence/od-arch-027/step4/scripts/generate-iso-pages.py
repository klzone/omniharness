#!/usr/bin/env python3
"""OD-ARCH-027 Step 4 · 生成「无限动画 × 大面积模糊」单因素对照页（受控页面证据）。

在 `baseline-pages/` 下新增 5 个对照页（**不改** `simple.html` / `heavy.html` 原件）：

  iso-blur-only.html        模糊/玻璃 ON，动画 OFF
  iso-anim-only.html        模糊/玻璃 OFF，动画 ON
  iso-anim-blur.html        两者都 ON（应≈ heavy.html）
  iso-anim-pulse-only.html  只开 pulse-subtle(6s)（模糊 OFF）
  iso-anim-float-only.html  只开 float-companion(5s)（模糊 OFF）

关键：所有页面的**布局、尺寸、颜色、层级完全一致**，只有「模糊/玻璃」与「动画」两个因素被切换，
因此可以把「原 ~100%」归因到具体组合。本脚本是唯一生成来源，文件头写明生成参数。

用法：python3 generate-iso-pages.py [--out <baseline-pages 目录>]
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
DEFAULT_OUT = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "baseline-pages"

BLUR = {
    "b1": "filter: blur(130px);",
    "b2": "filter: blur(140px);",
    "b3": "filter: blur(120px);",
}
GLASS = {
    "dock": "backdrop-filter: blur(40px) saturate(170%);",
    "nav": "backdrop-filter: blur(40px) saturate(170%);",
    "pane": "backdrop-filter: blur(20px) saturate(160%);",
    "plate": "backdrop-filter: blur(28px) saturate(170%);",
}
ANIM = {
    "pulse": "animation: pulse-subtle 6s cubic-bezier(.4,0,.6,1) infinite;",
    "float": "animation: float-companion 5s ease-in-out infinite;",
    # 复刻 Tailwind 的 animate-pulse（顶栏状态点）：2s cubic-bezier(.4,0,.6,1) 无限
    "dot": "animation: dot-pulse 2s cubic-bezier(.4,0,.6,1) infinite;",
}

PAGES = {
    "iso-blur-only": {"blur": True, "glass": True, "pulse": False, "float": False, "dot": False},
    "iso-anim-only": {"blur": False, "glass": False, "pulse": True, "float": True, "dot": False},
    "iso-anim-blur": {"blur": True, "glass": True, "pulse": True, "float": True, "dot": False},
    "iso-anim-pulse-only": {"blur": False, "glass": False, "pulse": True, "float": False, "dot": False},
    "iso-anim-float-only": {"blur": False, "glass": False, "pulse": False, "float": True, "dot": False},
    # 与 iso-blur-only 完全相同的静态模糊层，**只多一个 6×6 的脉冲小圆点**（顶栏状态点位置），
    # 用来单因素回答「Step 2 保留的顶栏 backdrop-blur + 顶栏状态点 animate-pulse 是否值得治理」。
    "iso-navpulse": {"blur": True, "glass": True, "pulse": False, "float": False, "dot": True},
}

TEMPLATE = """<!doctype html>
<html lang="zh">
<head>
<meta charset="utf-8" />
<title>OD-ARCH-027 Step 4 单因素对照 · {name}</title>
<!--
  由 tests/verification/desktop/evidence/od-arch-027/step4/scripts/generate-iso-pages.py 生成。
  受控对照页（非产品代码、非 Step 0 归档件）。因素开关：
    模糊/玻璃 = {blur_flag}   动画 = pulse:{pulse_flag} / float:{float_flag}
  场景与 heavy.html 同构（同布局/尺寸/颜色），只切换上述两个因素。
-->
<style>
  html, body {{ margin: 0; height: 100%; background: #f4f5ff; overflow: hidden; }}
  .blob {{ position: fixed; border-radius: 9999px; pointer-events: none; }}
  .b1 {{ top: -15%; left: 20%; width: 60rem; height: 50rem; background: linear-gradient(135deg, #c7d2fe, #ddd6fe); {b1} }}
  .b2 {{ top: 45%; right: -10%; width: 48rem; height: 48rem; background: linear-gradient(315deg, #dbe0ff, #e0e7ff); {b2} }}
  .b3 {{ bottom: -20%; left: 8%; width: 42rem; height: 42rem; background: linear-gradient(45deg, #e8e6ff, #eef0ff); {b3} }}
  .dock {{ position: fixed; left: 20px; top: 40%; width: 64px; height: 320px; border-radius: 9999px;
          background: rgba(255,255,255,.85); {dock} }}
  .nav  {{ position: fixed; left: 120px; right: 40px; top: 24px; height: 56px; border-radius: 9999px;
          background: rgba(255,255,255,.8); {nav} }}
  .panes {{ position: relative; padding: 120px 40px 40px 120px; display: flex; flex-direction: column; gap: 18px; }}
  .pane {{ background: rgba(255,255,255,.78); {pane}
          border: 1px solid rgba(255,255,255,.9); border-radius: 16px; padding: 20px 24px; height: 90px; }}
  .plate {{ background: rgba(255,255,255,.72); {plate}
           border-radius: 16px; padding: 20px 24px; height: 90px; }}
  @keyframes pulse-subtle {{ 0%,100% {{ opacity: 1; transform: scale(1);}} 50% {{ opacity: .75; transform: scale(1.05);}} }}
  @keyframes float-companion {{ 0%,100% {{ transform: translateY(0) rotate(0);}} 50% {{ transform: translateY(-7px) rotate(1.2deg);}} }}
{dot_keyframes}  .pulse {{ position: fixed; right: 60px; bottom: 60px; width: 120px; height: 120px; border-radius: 24px;
           background: #6366f1; {pulse_css} }}
  .mascot {{ position: fixed; right: 220px; bottom: 60px; width: 112px; height: 112px; border-radius: 50%;
            background: #a5b4fc; {float_css} }}
{dot_rule}</style>
</head>
<body>
  <div class="blob b1"></div><div class="blob b2"></div><div class="blob b3"></div>
  <div class="nav"></div><div class="dock"></div>{dot_elem}
  <div class="panes">
    <div class="plate">{name} · plate</div>
    <div class="pane">{name} · pane 1</div>
    <div class="pane">{name} · pane 2</div>
    <div class="pane">{name} · pane 3</div>
    <div class="pane">{name} · pane 4</div>
  </div>
  <div class="pulse"></div><div class="mascot"></div>
</body>
</html>
"""


def main(argv: list) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    args = ap.parse_args(argv[1:])
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    facts = {}
    for name, flags in PAGES.items():
        html = TEMPLATE.format(
            name=name,
            blur_flag=flags["blur"],
            pulse_flag=flags["pulse"],
            float_flag=flags["float"],
            b1=BLUR["b1"] if flags["blur"] else "",
            b2=BLUR["b2"] if flags["blur"] else "",
            b3=BLUR["b3"] if flags["blur"] else "",
            dock=GLASS["dock"] if flags["glass"] else "",
            nav=GLASS["nav"] if flags["glass"] else "",
            pane=GLASS["pane"] if flags["glass"] else "",
            plate=GLASS["plate"] if flags["glass"] else "",
            pulse_css=ANIM["pulse"] if flags["pulse"] else "",
            float_css=ANIM["float"] if flags["float"] else "",
            dot_keyframes=(
                "  @keyframes dot-pulse { 0%,100% { opacity: 1;} 50% { opacity: .5;} }\n"
                if flags.get("dot") else ""
            ),
            dot_rule=(
                "  /* 顶栏状态点（位于 .nav 内，6×6 px，复刻 Tailwind animate-pulse） */\n"
                "  .dot { position: fixed; left: 160px; top: 49px; width: 6px; height: 6px;"
                " border-radius: 9999px; background: #10b981; " + (ANIM["dot"] if flags.get("dot") else "") + " }\n"
                if flags.get("dot") else ""
            ),
            dot_elem='<div class="dot"></div>' if flags.get("dot") else "",
        )
        p = out / f"{name}.html"
        p.write_text(html, encoding="utf-8")
        facts[name] = {"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "flags": flags}
        print(f"  {name}.html  flags={flags}  sha256={facts[name]['sha256'][:16]}")
    (out / "iso-pages-manifest.json").write_text(
        json.dumps(
            {
                "generator": str(HERE),
                "note": "受控对照页；simple.html / heavy.html 原件未改写",
                "pages": facts,
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv))
