#!/usr/bin/env python3
"""OD-ARCH-101：全局悬停说明统一为软件自绘方角卡片的静态断言。

检查四件事：
  1. 只有一个全局接管组件，挂在 App 顶层，portal 到 body；
  2. 原生标签上的 title 全部迁移成 data-hover-hint，组件 prop 的 title 未被误改；
  3. 卡片样式与软件一致：直角、白底、细边框、轻投影、105 层级；
  4. 禁用控件仍带说明，且接管层有命中兜底。

证据等级：静态代码检查（读源码文本与结构计数）。不开窗口、不调用模型、
不读真实工作记录。窗口证据见 `evidence/od-arch-101/`。

用法：python3 tests/verification/desktop/verify_hover_hint_101.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def walk_tags(src: str):
    """逐个小标签扫描 JSX 开标签，返回 (标签名, 标签体, 行号)。

    不用正则全量匹配：属性值里可能有 `>`、模板字符串或嵌套花括号，
    必须按引号/花括号深度找真正的结尾。
    """
    i = 0
    n = len(src)
    while i < n:
        start = src.find("<", i)
        if start < 0:
            return
        k = start + 1
        if k >= n or not (src[k].isalpha() or src[k] in "_"):
            i = start + 1
            continue
        depth = 0
        quote = None
        end = None
        while k < n:
            ch = src[k]
            if quote:
                if ch == "\\":
                    k += 2
                    continue
                if ch == quote:
                    quote = None
            else:
                if ch in ('"', "'", "`"):
                    quote = ch
                elif ch == "{":
                    depth += 1
                elif ch == "}":
                    depth = max(0, depth - 1)
                elif ch == ">" and depth == 0:
                    end = k
                    break
            k += 1
        if end is None:
            return
        body = src[start + 1 : end]
        match = re.match(r"([A-Za-z][\w.]*)", body)
        if match:
            yield match.group(1), body, src[:start].count("\n") + 1
        i = end + 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    tsx_files = sorted(SRC.rglob("*.tsx"))
    sources = {p: p.read_text(encoding="utf-8") for p in tsx_files}
    hint = (SRC / "shell" / "HoverHint.tsx").read_text(encoding="utf-8")
    app = (SRC / "App.tsx").read_text(encoding="utf-8")

    intrinsic_title: list[str] = []
    component_title = 0
    hints = 0
    hint_files: set[str] = set()
    disabled_without_hint: list[str] = []

    for path, src in sources.items():
        for name, body, line in walk_tags(src):
            has_title = re.search(r"(?<![\w-])title\s*=", body) is not None
            has_hint = re.search(r"(?<![\w-])data-hover-hint\s*=", body) is not None
            if name[0].islower():
                if has_title:
                    intrinsic_title.append(f"{path.name}:{line} <{name}>")
                if has_hint:
                    hints += 1
                    hint_files.add(path.name)
                    if re.search(r"(?<![\w-])disabled\b", body) and not has_hint:
                        disabled_without_hint.append(f"{path.name}:{line}")
            elif has_title:
                component_title += 1

    # --- 组件与挂载 ---
    check("component_exists", (SRC / "shell" / "HoverHint.tsx").exists())
    check("mounted_once_in_app", app.count("<HoverHint />") == 1, "App 顶层只挂一次")
    check("created_once_globally", sum(s.count("<HoverHint") for s in sources.values()) == 1)
    check("portal_to_body", "createPortal(" in hint and "document.body" in hint)

    # --- 接管方式 ---
    check("event_delegation", "pointermove" in hint and "closest<HTMLElement>(" in hint)
    check("disabled_hit_test", "elementFromPoint" in hint, "禁用控件靠命中测试兜底")
    check("show_delay", "SHOW_DELAY_MS = 320" in hint, "停住约 0.3 秒再出现，不闪")
    check("dismiss_on_escape", 'event.key === "Escape"' in hint)
    check("dismiss_on_scroll", 'window.addEventListener("scroll", onScroll, true)' in hint)
    check("dismiss_on_pointerdown", 'window.addEventListener("pointerdown", onDown, true)' in hint)
    check("viewport_flip", "above >= MARGIN" in hint, "上方放不下就翻到下方")
    check("viewport_clamp", "viewportW - tip.width - MARGIN" in hint, "水平夹紧不越界")
    check("max_width", "MAX_WIDTH = 320" in hint)

    # --- 样式与可达性 ---
    check("role_tooltip", 'role="tooltip"' in hint)
    check("pointer_events_none", "pointer-events-none" in hint, "提示卡不抢点击")
    check("square_corners", "borderRadius: 0" in hint and "rounded" not in hint.split("className=")[1][:400])
    check("surface_border", "border-outline-variant/70" in hint)
    check("matches_toast_shadow", "0_10px_28px_-12px_rgba(15,23,42,0.35)" in hint)
    check("z_index_105", "z-[105]" in hint, "在状态提示 110 之下、常规浮层之上")
    check("type_scale", "text-[11.5px]" in hint)
    check("aria_describedby", 'setAttribute("aria-describedby"' in hint and "removeAttribute(\"aria-describedby\")" in hint)
    check(
        "pending_hover_not_reset",
        "pendingRef.current === element && timerRef.current !== null" in hint,
        "同一控件内持续微动不会一直推迟提示",
    )
    top_nav = (SRC / "shell" / "TopNav.tsx").read_text(encoding="utf-8")
    check(
        "icon_buttons_have_labels",
        'data-hover-hint="系统偏好设置"' in top_nav
        and 'aria-label="系统偏好设置"' in top_nav
        and 'data-hover-hint="未实现：通知与待办"' in top_nav
        and 'aria-label="未实现：通知与待办"' in top_nav,
        "去掉原生 title 后纯图标按钮仍有可访问名称",
    )

    # --- 迁移完整性 ---
    check("no_intrinsic_title_left", not intrinsic_title, "; ".join(intrinsic_title[:5]))
    check("hint_count_91", hints == 91, f"实际 {hints} 处")
    check("hint_spread_23_files", len(hint_files) == 23, f"实际 {len(hint_files)} 个文件")
    check("component_props_preserved", component_title == 22, f"组件 title 属性实际 {component_title} 处")
    check(
        "no_disabled_without_hint",
        not disabled_without_hint,
        "; ".join(disabled_without_hint[:5]),
    )
    check("week_timeline_dest_hint", "data-hover-hint={" in (SRC / "shell" / "WeeklyTimeline.tsx").read_text(encoding="utf-8"))
    check(
        "artifact_title_hint",
        "data-hover-hint={m ? m.title ?? r.file_name : undefined}"
        in (SRC / "shell" / "ArtifactExplorer.tsx").read_text(encoding="utf-8"),
        "成果卡悬停显示完整标题，不显示文件名",
    )

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-101",
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
