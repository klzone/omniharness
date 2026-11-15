#!/usr/bin/env python3
"""OD-UI-716：项目跟踪页与 OmniDesk 暖白/靛蓝视觉规范对齐的静态检查。

证据等级：源码结构检查；不打开窗口、不读取用户任务正文。
"""
from __future__ import annotations

from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    task = read(SRC / "shell" / "TaskManager.tsx")
    theme = read(SRC / "theme.css")

    check(
        "project_uses_work_record_header",
        'className="wr-hero"' in task
        and 'className="wr-hero-mark"' in task
        and 'className="wr-hero-title"' in task
        and 'className="wr-search ml-auto"' in task,
        "项目跟踪页复用工作记录的页头与搜索材质",
    )
    check(
        "project_has_no_cold_blue_palette",
        not any(color in task for color in ("#3B82F6", "#22D3EE", "#2563EB", "#EAF8FD", "#DCE9FF")),
        "任务象限不再使用冷蓝/青绿渐变",
    )
    check(
        "project_uses_warm_neumorphic_tokens",
        "#f8f8f6" in task
        and "#e8e7e3" in task
        and "#5747f8" in task
        and "NEU_CARD" in task,
        "卡片、凹槽与强调色落到暖白 + 品牌靛蓝",
    )
    check(
        "fake_progress_removed",
        "statePercent" not in task
        and "TASK_STATE_BAR" in task
        and "甘特条表达状态" in task,
        "甘特条表达状态，不再伪造 15%/55% 进度",
    )
    check(
        "quadrant_tones_have_tile",
        "tile:" in task and "tone.tile" in task and "QUADRANT_TONE" in task,
        "四象限图标有柔和底色，不靠大面积色块",
    )
    check(
        "modal_uses_warm_dialog",
        'className="wr-dialog w-full max-w-[520px] p-6"' in task
        and "bg-[rgba(24,26,40,0.3)]" in task,
        "任务编辑弹层复用暖白对话框材质",
    )
    check(
        "work_record_tokens_exist",
        all(token in theme for token in (".wr-hero", ".wr-search", ".wr-dialog")),
        "工作记录/抽屉的设计令牌可供项目页复用",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
