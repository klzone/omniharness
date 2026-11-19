#!/usr/bin/env python3
"""OD-READ-002 阶段 5 静态核验：健脑室仪表盘结构、数据口径、花瓣图与入口。"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DASH = ROOT / "desktop" / "src" / "shell" / "ReadingDashboard.tsx"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
CSS = ROOT / "desktop" / "src" / "reading.css"
API = ROOT / "desktop" / "src" / "api.ts"
TYPES = ROOT / "desktop" / "src" / "types.ts"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    dash = DASH.read_text()
    page = PAGE.read_text()
    css = CSS.read_text()
    api = API.read_text()
    types = TYPES.read_text()
    lib = LIB.read_text()
    rss = RSS.read_text()

    check("仪表盘组件存在", DASH.is_file() and "export function ReadingDashboard" in dash)
    check("ReadingPage 使用仪表盘替换空态", "<ReadingDashboard" in page and "reading-dashboard" in dash)
    check("五格按指定顺序", all(
        dash.find(f"<label>{label}</label>") >= 0 for label in ("阅读字数", "今年已读", "批注", "订阅文章", "整理笔记")
    ) and dash.find("<label>阅读字数</label>") < dash.find("<label>今年已读</label>") < dash.find("<label>批注</label>") < dash.find("<label>订阅文章</label>") < dash.find("<label>整理笔记</label>"))
    check("本月目标固定 1000 分钟", "MONTHLY_GOAL_MINUTES = 1000" in dash)
    check("本月分钟用真实统计", "stats?.month_minutes" in dash)
    check("不再字数估算", "item.char_count * item.progress_percent" not in dash and "估算" not in dash and "/ 300" not in dash)
    check("目标进度无黑色刻度针", "goal-knob" not in dash and "goal-knob" not in css)
    check("阅读字数 K/M 格式化", "function formatCompact" in dash and "value >= 10_000" in dash and "value >= 100_000_000" in dash)
    check("今年已读口径", "READING_COMPLETE" in dash and "0.995" in dash and "isSameYear(item.updated_at, now)" in dash)
    check("订阅文章读数与读完", "stats.rss_articles" in dash and "readArticleCount" in dash and "读完" in dash)
    check("整理笔记读数", "stats.notes" in dash)
    check("无虚构涨跌", "上月" not in dash and "本周" not in dash)
    check("最近在读书籍", "progress_percent > 0" in dash and "updated_at.localeCompare" in dash)
    check("最近四本已读书", ".slice(0, 4)" in dash)
    check("封面缩进与封面宽度", "padding-left: 61px" in css and "width: 61px" in css)
    check("封面缺省首字", "meta.title.trim().charAt(0)" in dash and "coverSrcCache" in dash)
    check("作者前置著字", "<i>著</i>" in dash)
    check("100% 完成环", "CompletionRing" in dash and "100%" in dash)
    check("左右头部按钮右对齐", "reading-dash-block-head-right" in dash and "justify-content: flex-end" in css)
    check("导入/查看全部/管理订阅入口", "onImport" in dash and "onViewAll" in dash and "onManageSubscriptions" in dash)
    check("查看全部打开书架管理", 'onViewAll={() => setShelfManagerOpen(true)}' in page and "reading-shelf-manager" in page)
    check("点击书籍卡打开书", "onOpenBook(meta.book_id)" in dash and "onOpenBook" in page)
    check("花瓣固定底色层", "reading-dash-petals-base" in dash)
    check("花瓣数据层按比例缩放", "reading-dash-petals-data" in dash and "scale(${scale})" in dash and "source.count / monthlyTotal" in dash)
    check("五瓣方向", "const PETAL_ANGLES = [-90, -18, 54, 126, 198]" in dash)
    check("瓣外百分比与来源名", "reading-dash-chart-percent" in dash and "reading-dash-chart-label" in dash)
    check("中心凸起圆钮", "reading-dash-petal-center" in dash and "--rd-raise-sm" in css)
    check("本月已读过滤", "item.read && articleInMonth(item, now)" in dash and "published_ts > 0" in dash)
    check("空态两张卡", "reading-dash-empty-card" in dash and "导入第一本书" in dash and "订阅第一篇资讯" in dash)
    check("精选作者 chips", "reading-dash-preset-chip" in dash and "onSubscribePreset" in dash)
    check(
        "宿主命令 rss_preset_sources",
        "fn rss_preset_sources" in lib
        and "rss_preset_sources," in lib.split("invoke_handler", 1)[-1],
    )
    check("宿主只暴露 9 位精选作者", "pub fn preset_sources()" in rss and "PRESET_GROUP_AUTHORS" in rss)
    check("前端命令与类型", '"rss_preset_sources"' in api and "interface RssPresetSource" in types)
    check("仪表盘样式作用域", ".reading-dash {" in css and ".reading-dash-top" in css and ".reading-dash-metrics" in css)

    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
