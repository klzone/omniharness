#!/usr/bin/env python3
"""OD-ARCH-110：切换 Profile 立即生效 —— 静态边界检查。

只读源码，断言「Profile 是换工作台，不是存个名字等重启」的几条硬边界：
  1. 切换入口先立生命周期屏障，再换配置，最后停旧引擎（顺序不可颠倒）；
  2. 新配置来自**同一次** `EngineConfig::from_env`，同时写进 supervisor 与只读快照；
  3. 旧引擎停不下来时保留屏障并如实回报，不谎称已切换；
  4. 回执带 `engine_stopped` / `effective`，前端据此决定是否刷新场景；
  5. 前端切换成功后重读场景声明、重算 Dock、重探引擎描述，并清掉设置页缓存；
  6. 文案口径统一为「切换后立即生效」，不再出现「下次启动生效」。

用法：python3 tests/verification/desktop/verify_profile_switch_110.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src-tauri" / "src"
UI = REPO / "desktop" / "src"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default="")
    args = ap.parse_args(argv[1:])

    lib = read(SRC / "lib.rs")
    sup = read(SRC / "engine_supervisor.rs")
    choice = read(SRC / "profile_choice.rs")
    app = read(UI / "App.tsx")
    topnav = read(UI / "shell" / "TopNav.tsx")
    settings = read(UI / "SettingsPanel.tsx")
    api = read(UI / "api.ts")
    types = read(UI / "types.ts")

    checks: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str) -> None:
        checks.append((name, bool(ok), detail))

    # 取切换函数体：从定义处切到下一个顶层 fn，避免手写字符窗口随改动失效。
    start = lib.find("fn profile_choice_set(")
    end = lib.find("\n/// 只读：当前（或指定）Profile 的资产就绪情况", start)
    body = lib[start:end] if 0 <= start < end else ""
    # 再放宽到「切回退」常量出现处，兼容尾部注释变动。
    if body and "engine_stopped" not in body:
        body = lib[start : start + 9000]

    check("switch_body_found", len(body) > 2000, f"profile_choice_set 函数体 {len(body)} 字符")

    # 1. 顺序：屏障 → 换配置 → 停引擎
    i_barrier = body.find("begin_change()")
    i_adopt = body.find("adopt_engine_config(")
    i_stop = body.find("stop_engine(")
    check(
        "barrier_before_config_swap_before_stop",
        0 <= i_barrier < i_adopt < i_stop,
        "先后顺序为 begin_change → adopt_engine_config → stop_engine",
    )
    check(
        "barrier_precedes_config_comment",
        "屏障先立起来再换配置" in body,
        "源码注明为什么必须先立屏障（避免新规则配旧引擎）",
    )

    # 2. 同一份配置写两处
    adopt_at = lib.find("fn adopt_engine_config(")
    adopt = lib[adopt_at : adopt_at + 900] if adopt_at >= 0 else ""
    check(
        "adopt_writes_supervisor_and_snapshot",
        "config_mut() = fresh.clone()" in adopt and "*config.lock().unwrap() = fresh" in adopt,
        "adopt_engine_config 同时写 supervisor 与只读快照",
    )
    check(
        "adopt_takes_config_by_value",
        "fresh: EngineConfig" in adopt,
        "按值接收同一份 EngineConfig，避免两次 from_env 得出不同结果",
    )
    check(
        "switch_reparses_once",
        body.count("EngineConfig::from_env") == 1,
        "切换路径只解析一次引擎配置",
    )

    # 3. 失败关闭
    check(
        "stop_failure_keeps_barrier",
        "change.fail(" in body and "change.succeed()" in body,
        "停止成功才 succeed；失败走 fail 并保留屏障",
    )
    check(
        "rollback_on_config_parse_failure",
        "previous_saved" in body and "切换已回滚" in body,
        "配置解析失败时把设置回滚到切换前",
    )

    # 4. 回执字段
    for field in ("\"engine_stopped\"", "\"effective\""):
        check(
            "receipt_" + field.strip('"'),
            field in body,
            f"回执包含 {field.strip(chr(34))}",
        )

    # 5. 前端接线
    check(
        "app_reloads_decl_after_switch",
        "loadDecl" in app and "useCallback" in app and "onProfileChanged" in app,
        "App 把读声明抽成 loadDecl 并在切换后重跑",
    )
    check(
        "app_bumps_epoch_and_reprobes",
        "setProfileEpoch" in app and "probeEngine" in app,
        "切换后 bump profileEpoch 并重探引擎描述",
    )
    check(
        "topnav_calls_back_when_stopped",
        "r.engine_stopped !== false" in topnav and "onProfileChanged?.()" in topnav,
        "TopNav 仅在旧引擎确已停止时通知 App 刷新",
    )
    check(
        "settings_clears_cache_on_epoch",
        "profileEpoch" in settings and "useEffect" in settings,
        "SettingsPanel 按 profileEpoch 清空自己的缓存并重读",
    )
    check(
        "types_have_new_fields",
        "engine_stopped" in types and "effective" in types,
        "前端类型声明包含新回执字段",
    )
    check(
        "api_comment_updated",
        "立即停止旧引擎" in api or "立即生效" in api,
        "前端 API 注释说明切换即停旧引擎",
    )

    # 6. 文案口径
    check(
        "ui_says_immediate",
        "切换后立即生效" in topnav,
        "Profile 弹层写「切换后立即生效」",
    )
    check(
        "no_next_start_copy",
        "下次启动" not in topnav and "下次启动" not in choice,
        "顶栏与后端 note 不再出现「下次启动」",
    )
    check(
        "confirm_warns_interruption",
        "正在生成的内容" in topnav,
        "确认弹窗如实提示正在生成的内容会被中断",
    )
    check(
        "view_note_states_effective",
        "当前生效的 Profile" in lib,
        "profile_choice_view 的 note 改为「当前生效的 Profile」",
    )

    # 7. 不越界
    check(
        "does_not_touch_runtime",
        "runtime/" not in body,
        "切换逻辑不触碰 runtime 内 DSH",
    )
    check(
        "supervisor_from_env_still_used",
        "EngineConfig::from_env" in sup,
        "配置解析仍走既有 from_env 优先级",
    )

    passed = sum(1 for _, ok, _ in checks if ok)
    for name, ok, detail in checks:
        print(("PASS " if ok else "FAIL ") + name + " — " + detail)
    print(f"\n{passed}/{len(checks)} passed")

    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {"passed": passed, "total": len(checks), "checks": [
                    {"name": n, "ok": ok, "detail": d} for n, ok, d in checks
                ]},
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv))
