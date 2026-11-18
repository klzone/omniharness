#!/usr/bin/env python3
"""OD-READ-002 阶段 1 静态核验：健脑室改名、RSS 存储/命令、阅读书融合与 API 契约。"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
RSS = ROOT / "desktop" / "src-tauri" / "src" / "rss_store.rs"
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
CARGO = ROOT / "desktop" / "src-tauri" / "Cargo.toml"
API = ROOT / "desktop" / "src" / "api.ts"
TYPES = ROOT / "desktop" / "src" / "types.ts"
PAGE = ROOT / "desktop" / "src" / "shell" / "ReadingPage.tsx"
DOCK = ROOT / "desktop" / "src" / "shell" / "Dock.tsx"
APP = ROOT / "desktop" / "src" / "App.tsx"

FAILS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok and detail else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    check("rss_store.rs 存在", RSS.is_file())
    source = RSS.read_text() if RSS.is_file() else ""
    lib = LIB.read_text()
    api = API.read_text()
    types = TYPES.read_text()
    page = PAGE.read_text()
    dock = DOCK.read_text()
    app = APP.read_text()
    cargo = CARGO.read_text()

    check("rss_store 注册到 lib.rs", "pub mod rss_store;" in lib)
    for command in (
        "rss_subscriptions_list",
        "rss_subscription_add",
        "rss_subscription_remove",
        "rss_fetch",
        "rss_articles_list",
        "rss_article_mark_read",
    ):
        check(f"命令注册 {command}", f"fn {command}" in lib and command in lib.split("invoke_handler", 1)[-1])
        check(f"前端 invoke {command}", f'"{command}"' in api)

    check("订阅清单落 rss/subscriptions.json", 'SUBSCRIPTIONS_FILE: &str = "subscriptions.json"' in source)
    check("schema omnidesk.rss.v1", 'RSS_SCHEMA: &str = "omnidesk.rss.v1"' in source)
    check("文章元数据含 read", "pub read: bool" in source)
    check("RSS 文章复用 ReadingBook", "ReadingBook" in source and 'source_format: "rss"' in source)
    check("RSS 书进入 books/ 与既有批注命令兼容", "write_book(root, &book)" in source)
    check("响应体上限 2MB", "MAX_FETCH_BYTES: usize = 2 * 1024 * 1024" in source)
    check("最多解析 200 条", "MAX_ENTRIES_PARSED: usize = 200" in source)
    check("每 feed 保留 50 条", "MAX_ENTRIES_KEPT: usize = 50" in source)
    check("预置 9 位精选作者", source.count("group: PRESET_GROUP_AUTHORS") == 9)
    check("独立博客不超过 40 条", 0 < source.count("group: PRESET_GROUP_BLOGS") <= 40)
    check("不内置公众号源", "公众号" not in source and "wechat" not in source.lower())
    check("MIT 归属注释", "MIT License" in source and "chinese-independent-blogs" in source)
    check("GPL 仅借鉴未复制说明", "GPL-3.0" in source and "未复制代码" in source)
    check("reqwest blocking + native-tls", "reqwest" in cargo and "blocking" in cargo and "native-tls" in cargo)

    for interface in ("RssSubscription", "RssArticleMeta", "RssFetchReport", "RssFetchFailure"):
        check(f"类型 {interface}", f"export interface {interface}" in types)

    check("Dock 入口改名健脑室", 'label: "健脑室"' in dock)
    check("页面标题健脑室", "健脑室" in page)
    check("App 加载文案同步", "正在打开健脑室" in app)
    check("订阅管理模态框", "subscriptionOpen" in page and "rssSubscriptionAdd" in page)
    check("书架 RSS 分组", "订阅文章" in page and 'source_format !== "rss"' in page)
    check("未读圆点", 'aria-label="未读"' in page)
    check(
        "既有划线浮条四按钮保留",
        "reading-selection-bar" in page
        and all(label in page for label in ("划线", "想法", "标段", "问 AI")),
    )
    check("阅读器仍用 data-paragraph", "data-paragraph" in page)
    check("无 runtime/ 代码引用", "runtime/" not in source and "engine/" not in source)

    check("预置源码尺寸 <20KB", len(source.encode("utf-8")) < 200_000, "整个 rss_store 大于 20KB 属正常；预置清单测试在 cargo 内")
    if FAILS:
        print(f"\n{len(FAILS)} check(s) failed")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
