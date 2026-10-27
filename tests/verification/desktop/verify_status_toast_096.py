#!/usr/bin/env python3
"""OD-ARCH-096：构建警告清理与模型切换提示条的静态断言。

证据等级：静态代码检查（读源码文本与正则）。不开窗口、不调用模型。
窗口级行为（提示条出现 / 右下角 / 可手动关闭）由 `verify_model_route_095.py`
在同一窗口里断言；60 秒自动关闭不在这里实测，由常量断言 + 计时实现核对。

用法：python3 tests/verification/desktop/verify_status_toast_096.py [--json PATH]
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
TAURI = DESKTOP / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    toast = SRC / "shell" / "StatusToast.tsx"
    home = SRC / "shell" / "HomeChat.tsx"
    worklog = TAURI / "worklog.rs"
    toast_src = toast.read_text(encoding="utf-8")
    home_src = home.read_text(encoding="utf-8")
    worklog_src = worklog.read_text(encoding="utf-8")

    # --- S01 死代码 ---
    check("append_receipt_failure_impl_removed", "fn failure(" not in worklog_src)
    check("append_receipt_struct_intact", "pub struct AppendReceipt" in worklog_src)

    # --- S02 提示条形态 ---
    check("toast_component_exists", toast.is_file())
    check(
        "toast_auto_dismiss_60s",
        re.search(r"STATUS_TOAST_MS\s*=\s*60_000", toast_src) is not None,
        "60 秒 = 60_000 ms",
    )
    check("toast_timer_uses_settimeout", "window.setTimeout" in toast_src and "STATUS_TOAST_MS" in toast_src)
    check("toast_square_corners", "borderRadius: 0" in toast_src)
    check(
        "toast_no_rounded_class",
        re.search(r'className="fixed bottom-[^\"]*rounded', toast_src) is None,
    )
    check("toast_anchored_bottom_right", "fixed bottom-4 right-4" in toast_src)
    check("toast_portal_to_body", "createPortal" in toast_src and "document.body" in toast_src)
    check("toast_has_close_button", 'aria-label="关闭提示"' in toast_src)
    check("toast_role_status_and_alert", 'role={message.tone === "error" ? "alert" : "status"}' in toast_src)
    # WebKit 不把容器内 <p> 的文字暴露成可访问文本（窗口实测：AT-SPI 里只有空 paragraph），
    # 必须把正文放进容器的无障碍名称，屏幕阅读器才读得到、自动化才断言得到。
    check("toast_exposes_text_to_a11y", "aria-label={message.text}" in toast_src)
    check("toast_uses_theme_tokens", "bg-error" in toast_src and "text-secondary" in toast_src)

    # --- S02 接线 ---
    check("homechat_renders_toast", "<StatusToast message={modelNote}" in home_src)
    check(
        "inline_composer_note_removed",
        'text-[10.5px] text-outline pb-1">{modelNote}' not in home_src,
    )
    check("success_tone_success", '"success"' in home_src and "引擎已按新路由重启" in home_src)
    check(
        "failure_tone_error",
        home_src.count('(e instanceof Error ? e.message : String(e)), "error")') >= 2,
    )
    check(
        "auto_scene_effort_silent",
        "已按场景应用思考等级" not in home_src,
        "切场景自动套档不再弹提示条",
    )
    check(
        "toast_above_dialogs",
        re.search(r"z-\[1[0-9][0-9]\]", toast_src) is not None,
        "z-index 高于既有弹窗 z-[80]/z-[95]",
    )

    passed = all(ok for _, ok, _ in CHECKS)
    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-ARCH-096",
        "passed": passed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [
            {"name": n, "ok": ok, "detail": d} for n, ok, d in CHECKS
        ],
    }
    for name, ok, _ in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name)
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
