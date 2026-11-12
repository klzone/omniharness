#!/usr/bin/env python3
"""OD-WR-504 工作记录时间线：分组逻辑与界面接线验证。

证据等级：
  * 脚本运行：抽取 WorkRecordLibrary.tsx 的纯函数块，编译后用固定虚构数据
    验证日期倒序、组内 created_at 正序、摘要/分类/来源/时间兜底；
  * 静态源码检查：组头三要素、七要素卡片、筛选前分组、空组不渲染、
    当天/非当天摘要语义与既有 data 属性保留。

不读用户数据、不调用模型、不开窗口。

用法：python3 tests/verification/desktop/verify_wr_504.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
LIBRARY = DESKTOP / "src" / "shell" / "WorkRecordLibrary.tsx"
THEME = DESKTOP / "src" / "theme.css"

CASES: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


PROBE = r"""
const mod = require(process.argv[2]);

function check(name, ok, detail) {
  console.log(JSON.stringify({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) }));
}

const items = [
  { id: "a", record_date: "2026-11-10", created_at: "2026-11-10T02:00:00Z", source: null, raw_source: null },
  { id: "c", record_date: "2026-11-11", created_at: "2026-11-11T01:00:00Z", source: null, raw_source: null },
  { id: "b", record_date: "2026-11-10", created_at: "2026-11-10T09:00:00Z", source: null, raw_source: null },
  { id: "d", record_date: "2026-11-10", created_at: "2026-11-10T09:00:00Z", source: null, raw_source: null },
];
const groups = mod.groupRecordsByDate(items);
check("group_dates_desc", groups.map((g) => g.date).join(",") === "2026-11-11,2026-11-10", groups.map((g) => g.date).join(","));
check("group_inner_created_asc", groups[1].records.map((r) => r.id).join(",") === "a,b,d", groups[1].records.map((r) => r.id).join(","));
check("group_membership", groups[0].records.length === 1 && groups[1].records.length === 3);
const filtered = mod.groupRecordsByDate([items[2]]);
check("filtered_groups_not_fabricated", filtered.length === 1 && filtered[0].date === "2026-11-10" && filtered[0].records.length === 1);

const local = new Date(2026, 10, 11, 9, 5, 0);
check("time_local_hm", mod.recordTimeLabel(local.toISOString()) === "09:05", mod.recordTimeLabel(local.toISOString()));
check("time_invalid_null", mod.recordTimeLabel("not-a-date") === null && mod.recordTimeLabel("") === null);

check("preview_first_line", mod.recordPreview("第一行内容\n第二行内容") === "第一行内容");
const longLine = "记".repeat(70);
const preview = mod.recordPreview(longLine);
check("preview_60_chars", preview.length === 61 && preview.endsWith("…") && preview.startsWith("记".repeat(60)));
check("preview_empty_fallback", mod.recordPreview("") === "（无正文）" && mod.recordPreview("  \n ") === "（无正文）");

check("category_null_fallback", mod.recordCategoryLabel(null) === "未分类" && mod.recordCategoryLabel("  ") === "未分类");
check("category_kept", mod.recordCategoryLabel("工作事项") === "工作事项");
check("source_raw_fallback", mod.recordSourceLabel({ source: null, raw_source: "首页对话" }) === "首页对话");
check("source_prefers_source", mod.recordSourceLabel({ source: "手动记录", raw_source: "首页对话" }) === "手动记录");
check("source_absent_null", mod.recordSourceLabel({ source: null, raw_source: null }) === null);
check("source_optional_missing", mod.recordSourceLabel({}) === null);
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    library = LIBRARY.read_text(encoding="utf-8")
    theme = THEME.read_text(encoding="utf-8")

    check("list_container_attr_kept", 'data-work-record-list=""' in library)
    check("record_row_attr_kept", "data-record-row={record.id}" in library)
    check("record_group_attr", "data-record-group={group.date}" in library)
    check(
        "record_group_count_attr",
        "data-record-group-count={group.records.length}" in library,
    )
    check(
        "grouping_after_host_filter",
        "const groups = groupRecordsByDate(listing.items);" in library
        and ".workRecordList(filter)" in library,
    )
    check("groups_render_from_items_only", "groups.map((group) =>" in library)

    head_start = library.index('<div className="od-record-group-head">')
    head_end = library.index('od-record-group-cards', head_start)
    head = library[head_start:head_end]
    check("head_date", "{group.date}" in head)
    check(
        "head_count",
        "data-record-group-count={group.records.length}" in head
        and "{group.records.length} 条" in head,
    )
    check(
        "head_today_summary_entry",
        'data-daily-summary-generate=""' in head and 'data-daily-summary-view=""' in head,
    )
    check("head_other_day_summary_entry", "data-record-group-summary={group.date}" in head)
    today_branch_start = head.index("{isToday ?")
    other_branch_start = head.index(") : (", today_branch_start)
    today_branch = head[today_branch_start:other_branch_start]
    other_branch = head[other_branch_start:]
    check(
        "today_branch_generate_and_view",
        'data-daily-summary-generate=""' in today_branch
        and 'data-daily-summary-view=""' in today_branch,
    )
    check(
        "other_branch_read_only",
        "data-daily-summary-generate" not in other_branch
        and "data-daily-summary-view" not in other_branch
        and "data-record-group-summary={group.date}" in other_branch,
    )
    check(
        "head_no_extra_copy",
        "当日记录" not in head and "按日期" not in head,
        "组头只保留日期、条数与摘要入口",
    )

    card_start = library.index("const active = selected?.id === record.id;")
    card_end = library.index("listing.broken", card_start)
    card = library[card_start:card_end]
    seven = {
        "title": 'data-record-card-title=""' in card and "record.title.trim()" in card,
        "time": 'data-record-card-time=""' in card
        and "recordTimeLabel(record.created_at)" in card
        and "record.record_date" in card,
        "category": 'data-record-card-category=""' in card
        and "recordCategoryLabel(record.category)" in card,
        "preview": 'data-record-card-preview=""' in card
        and "recordPreview(record.body)" in card,
        "source": 'data-record-card-source=""' in card and "{source ?" in card,
        "linked_tasks": 'data-record-linked-count=""' in card
        and "record.linked_task_ids.length" in card,
        "status": "data-record-status={record.ai_analysis_status}" in card
        and 'data-record-draft=""' in card,
    }
    for name, ok in seven.items():
        check("card_seven:" + name, ok)

    check(
        "today_generate_semantics",
        "function requestSummary()" in library
        and "data-daily-summary-generate" in library
        and "requestNewThread(\"weekly\", \"生成今日摘要\")" in library,
    )
    check(
        "other_day_read_only",
        "if (date === todayLocal())" in library
        and ".worklogDailySummaryRead(date)" in library
        and "当日无摘要。" in library,
    )
    check(
        "summary_links_kept",
        'data-daily-summary-record={id}' in library and "openSummaryRecord(id)" in library,
    )
    check(
        "timeline_styles",
        all(
            name in theme
            for name in (
                ".od-record-group ",
                ".od-record-group-head::before",
                ".od-record-group-cards",
                ".od-record-card",
            )
        ),
    )

    helper_start = library.index("/* ---- OD-WR-504 时间线")
    helper_end = library.index("function changedFieldsLabel", helper_start)
    helpers = library[helper_start:helper_end]
    probe_source = (
        "type WorkRecord = { id: string; record_date: string; created_at: string; "
        "source?: string | null; raw_source?: string | null };\n"
        "interface RecordDateGroup { date: string; records: WorkRecord[] }\n"
        + helpers
    )

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        source = tmp_path / "timelineHelpers.ts"
        source.write_text(probe_source, encoding="utf-8")
        out = tmp_path / "out"
        compiled = run(
            [
                "npx",
                "--no-install",
                "tsc",
                "--ignoreConfig",
                str(source),
                "--outDir",
                str(out),
                "--module",
                "commonjs",
                "--target",
                "es2020",
                "--strict",
            ],
            DESKTOP,
        )
        if compiled.returncode != 0:
            check("helpers_compile", False, compiled.stderr[-1200:])
        else:
            check("helpers_compile", True)
            probe = tmp_path / "probe.js"
            probe.write_text(PROBE, encoding="utf-8")
            proc = run(["node", str(probe), str(out / "timelineHelpers.js")], DESKTOP)
            if proc.returncode != 0:
                check("helpers_probe_exit", False, proc.stderr[-1200:])
            else:
                check("helpers_probe_exit", True)
                for line in proc.stdout.splitlines():
                    if line.strip():
                        CASES.append(json.loads(line))

    failed = [case for case in CASES if not case["ok"]]
    payload = {
        "plan": "OD-WR-504",
        "evidence_level": "脚本运行 + 静态源码检查",
        "passed": not failed,
        "total": len(CASES),
        "passed_count": len(CASES) - len(failed),
        "failed": failed,
        "cases": CASES,
    }
    for case in CASES:
        print(("PASS  " if case["ok"] else "FAIL  ") + case["name"])
    print(f"\n{len(CASES) - len(failed)}/{len(CASES)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
