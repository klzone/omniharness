#!/usr/bin/env python3
"""OD-ARCH-177：配套插件推荐清单与技能环境适配（纯文本断言）。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FAILS: list[str] = []


def check(name: str, ok: bool) -> None:
    print(("PASS" if ok else "FAIL") + " " + name)
    if not ok:
        FAILS.append(name)


def main() -> int:
    data = (ROOT / "desktop/src/companionPlugins.ts").read_text(encoding="utf-8")
    panel = (ROOT / "desktop/src/SettingsPanel.tsx").read_text(encoding="utf-8")
    skill = (ROOT / "skills/anysearch/SKILL.md").read_text(encoding="utf-8")

    # 清单：两项常备插件，名称 / 地址 / 建议版本 / 用途齐全。
    check("list_anysearch", "AnySearch CLI" in data and "anysearch-ai/anysearch-skill" in data and "v2.1.0" in data)
    check("list_cost_meter", "dsh-cost-meter" in data and "Han-1413141/dsh-cost-meter" in data and "1.9.6" in data)
    check("list_no_install_promise", "不自动安装" in data or "不打包任何插件实体" in data)

    # 设置页渲染：导入清单 + 区块存在 + 每项有 testid。
    check("panel_imports", 'from "./companionPlugins"' in panel)
    check("panel_section", "配套插件推荐" in panel and 'data-testid="companion-plugins"' in panel)
    check("panel_rows", 'data-testid={"companion-plugin-" + plugin.name}' in panel)
    check("panel_hint", "不在发布包内" in panel)

    # 技能文本：引擎环境预期 + 直接回退 + 不反复重试。
    check("skill_engine_env", "OmniDesk 引擎环境默认" in skill and "ANYSEARCH_SKILL_DIR" in skill)
    check("skill_fallback", "web_fetch" in skill)
    check("skill_no_retry_loop", "反复重试" in skill)
    check("skill_honest_source", "如实标注实际使用的渠道" in skill)

    print(f"cases=11 passed={11 - len(FAILS)} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
