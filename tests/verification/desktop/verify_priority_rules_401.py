#!/usr/bin/env python3
"""OD-QS-402 紧急性规则引擎：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：纯函数签名、逾期规则、工作日分档、target 降档、封闭时间模式集、
推定标注、模式外待评估，以及 12 类时间场景的测试存在性。

用法：python3 tests/verification/desktop/verify_priority_rules_401.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    source = (RUST / "priority_rules.rs").read_text(encoding="utf-8")
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")

    check(
        "module_registered",
        "pub mod priority_rules;" in lib and "pub fn compute_urgency(" in source,
        "规则模块已注册并提供 compute_urgency(task, now)",
    )
    production = source.split("#[cfg(test)]")[0]
    check(
        "pure_function_no_io",
        "use std::fs" not in production
        and "Command::new" not in production,
        "生产代码不读写文件、不启动进程（路径参数不算 IO）",
    )
    check(
        "levels",
        all(item in source for item in ['URGENCY_HIGH: &str = "高"', 'URGENCY_MEDIUM: &str = "中"', 'URGENCY_LOW: &str = "低"', 'URGENCY_PENDING: &str = "待评估"']),
        "等级为 高 / 中 / 低 / 待评估 有限枚举",
    )
    check(
        "overdue_is_high_with_days",
        "已逾期" in source and "overdue" in source and "URGENCY_HIGH" in source,
        "逾期判高并给出逾期天数",
    )
    check(
        "workday_thresholds",
        "HIGH_BUSINESS_DAYS" in source
        and "MEDIUM_BUSINESS_DAYS" in source
        and "fn workdays_between(" in source
        and "工作日节奏" in source,
        "按工作日分档，不用固定 24/48 小时",
    )
    check(
        "target_downgrade",
        "DUE_KIND_TARGET" in source and "fn downgrade(" in source and "目标时间，非硬截止" in source,
        "target due 在同日期下降一档",
    )
    check(
        "closed_time_patterns",
        all(item in source for item in ["明天", "后天", "下周", "本月底", "下月底"])
        and "parse_month_day" in source,
        "封闭模式集覆盖 明天/后天/下周X/本月底/下月底/X月X日前",
    )
    check(
        "inferred_marked",
        "pub inferred: bool" in source and "（推定）" in source,
        "推定结果带「推定」标注",
    )
    check(
        "outside_pattern_pending",
        "未识别到封闭时间表达（待评估）" in source and "URGENCY_PENDING.to_string()" in source,
        "模式外与无截止一律待评估",
    )
    check(
        "local_offset_aware",
        "pub fn compute_urgency_at(" in source and "utc_offset_minutes" in source,
        "日期判断可显式传入本地偏移，不按模型服务器时区",
    )
    test_names = [
        "explicit_due_far_away_is_low",
        "explicit_due_within_two_workdays_is_high",
        "target_due_is_one_level_lower",
        "overdue_is_high_with_days",
        "due_today_is_high",
        "no_due_and_no_hint_is_pending",
        "hint_tomorrow_is_inferred_and_marked",
        "hint_month_end_and_next_month_end",
        "hint_next_week_weekday_uses_next_week",
        "hint_tomorrow_afternoon_uses_date_only",
        "hint_soon_urgent_or_unclear_is_pending",
        "month_day_hint_crosses_year_when_past",
        "month_end_handles_leap_february",
        "local_offset_changes_today_boundary",
        "month_end_crosses_december",
    ]
    check(
        "time_scenarios_covered",
        all(name in source for name in test_names) and "#[test]" in source,
        "12 类时间场景（明确日期/月底/下周/明天下午/近期/无时间/跨月/跨年/闰年/时区/模糊）均有测试",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-QS-402",
        "evidence_level": "静态源码检查",
        "passed": not failed,
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
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
