#!/usr/bin/env python3
"""OD-UI-714：历史会话平台入口与分类归档页的静态检查。

证据等级：源码结构检查 + 纯函数单测；不打开真实窗口，不读取用户线程正文。

覆盖：
  1. 历史会话与知识库一样是平台固有入口，不受 Profile 场景声明管辖。
  2. App 路由到 SessionHistoryPage；旧声明里的 sessions 不会造成重复。
  3. 分类块来自当前 Profile 分类表，新增/未知分类不丢失线程。
  4. 单条记录提供继续、归档、删除三项操作；删除前确认，并说明引擎会话边界。
  5. 页面用 OmniDesk 新拟物令牌做模块化分类板；抽屉是 80% 高度的浮层。
"""
from __future__ import annotations

import subprocess
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
    dock = read(SRC / "shell" / "Dock.tsx")
    app = read(SRC / "App.tsx")
    page = read(SRC / "shell" / "SessionHistoryPage.tsx")
    helper = read(SRC / "sessionHistory.ts")
    theme = read(SRC / "theme.css")
    test_file = SRC / "sessionHistory.test.mjs"

    check(
        "dock_history_builtin",
        "BUILTIN_HISTORY_ITEM" in dock
        and 'scene: "sessions"' in dock
        and 'label: "历史会话"' in dock,
        "历史会话是 Dock 内置项",
    )
    check(
        "dock_platform_items_include_history_knowledge",
        'BUILTIN_PLATFORM_ITEMS' in dock
        and 'BUILTIN_HISTORY_ITEM' in dock.split('BUILTIN_PLATFORM_ITEMS', 1)[1]
        and 'BUILTIN_KNOWLEDGE_ITEM' in dock.split('BUILTIN_PLATFORM_ITEMS', 1)[1],
        "基础标准也有历史会话与知识库",
    )
    check(
        "dock_dedupes_platform_scenes",
        '["sessions", "knowledge"].includes' in dock and "seen.add" in dock,
        "旧声明里的 sessions/knowledge 不会重复",
    )
    check(
        "app_routes_history_page",
        'SessionHistoryPage' in app and 'scene === "sessions"' in app,
        "App 已路由历史会话页",
    )
    check(
        "history_not_profile_managed",
        'const managed = ["home", "report", "writing", "project"] as const' in app,
        "历史会话不随 Profile 声明校准",
    )
    check(
        "page_reads_thread_store",
        'listThreads' in page and 'onThreadStoreChange' in page
        and 'retagThread' in page and 'removeThread' in page,
        "页面接通本机线程的读、归档与删除",
    )
    check(
        "page_uses_profile_categories",
        'useThreadCategories' in page and 'categories.map' in page,
        "分类菜单来自当前 Profile 分类表",
    )
    check(
        "page_three_actions",
        all(text in page for text in ("继续对话", "归档对话", "删除对话")),
        "单条记录提供三项操作",
    )
    check(
        "archive_is_category_retag",
        'retagThread' in page and '已归档到' in page and 'archiveThread' in page,
        "归档只重设分类，不移动或删除线程文件",
    )
    check(
        "delete_confirms_and_names_boundary",
        'ConfirmDialog' in page
        and 'session-delete-confirm' in page
        and '引擎侧会话不会在这里改动' in page,
        "删除有确认，并说明引擎会话边界",
    )
    check(
        "inflight_delete_blocked",
        'selected.inflightTaskId' in page and '结束或取消后才能删除' in page,
        "在途任务未结束时不能删除记录",
    )
    check(
        "unknown_tags_not_dropped",
        'custom:' in helper and 'EXTRA_ORDER' in helper and 'localeCompare' in helper,
        "未知或旧分类仍作为额外分组显示",
    )
    check(
        "category_order_is_host_order",
        'for (const category of categories)' in helper
        and 'if (!items || items.length === 0) continue' in helper,
        "分类按宿主顺序、空分类不渲染",
    )
    check(
        "page_uses_omnidesk_material",
        all(token in page for token in ('sh-masthead', 'sh-mast-mark', 'wr-drawer', 'sh-drawer')),
        "页头与抽屉使用 OmniDesk 材质",
    )
    check(
        "category_board_is_modular",
        'sh-board' in page
        and 'repeat(auto-fit, minmax(420px, 1fr))' in theme,
        "分类是模块化卡片网格，不是从上到下平铺",
    )
    check(
        "entries_use_timeline",
        all(token in page for token in ('sh-entry-list', 'sh-entry-dot', 'sh-entry-title')),
        "分类卡内对话按时间线排列",
    )
    check(
        "category_cards_collapsible",
        'expanded[group.key]' in page and 'sh-show-more' in page and '展开其余' in page,
        "分类卡默认收起多余对话",
    )
    check(
        "future_category_tone",
        'CATEGORY_TONES' in page and 'categoryToneClass' in page and 'tone-neutral' in page,
        "未来分类有稳定的轻量识别色",
    )
    check(
        "drawer_height_is_80_percent",
        'top-[10%]' in page and 'h-[80%]' in page and 'right-4' in page,
        "抽屉高度为当前界面的 80%",
    )
    check(
        "history_css_present",
        all(token in theme for token in (".sh-groups", ".sh-card", ".sh-primary-action", ".sh-danger-button")),
        "历史会话样式已落地",
    )
    check("history_unit_test_file", test_file.is_file(), "分组纯函数单测存在")

    proc = subprocess.run(
        ["node", "--experimental-strip-types", "--test", "src/sessionHistory.test.mjs"],
        cwd=REPO / "desktop",
        capture_output=True,
        text=True,
        check=False,
    )
    check(
        "history_unit_tests_pass",
        proc.returncode == 0,
        (proc.stdout + proc.stderr).strip().splitlines()[-1] if (proc.stdout + proc.stderr).strip() else "no output",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
