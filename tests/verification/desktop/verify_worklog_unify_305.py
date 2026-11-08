#!/usr/bin/env python3
"""OD-WR-310 随手记链路切换：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：随手记写记录库、记录去向字段、周索引按 ISO 周聚合、旧周文件写路径退役、
旧读函数保留、UI 判定块协议与 IPC 形状不变。

用法：python3 tests/verification/desktop/verify_worklog_unify_305.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
RUST = REPO / "desktop" / "src-tauri" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    lib = read(RUST / "lib.rs")
    quick = read(RUST / "quicklog.rs")
    records = read(RUST / "worklog_records.rs")
    legacy = read(RUST / "worklog.rs")
    api = read(SRC / "api.ts")
    chat = read(SRC / "shell" / "HomeChat.tsx")
    types = read(SRC / "types.ts")

    check(
        "quicklog_module_present",
        all(
            token in quick
            for token in ("pub fn append(", "pub fn week_entries(", "pub fn move_many(")
        )
        and "pub fn merge(" in quick
        and "pub fn render_week_markdown(" in quick,
        "随手记记录库模块存在",
    )
    check(
        "append_writes_records",
        "worklog_records::create_record(root" in quick
        and "destination: Some(dest.clone())" in quick,
        "追加写入记录库并带去向",
    )
    check(
        "record_destination_field",
        "pub destination: Option<String>" in records
        and 'destination: {}' in records,
        "记录 frontmatter 新增 destination",
    )
    check(
        "commands_use_quicklog",
        all(
            token in lib
            for token in (
                "quicklog::append(",
                "quicklog::set_destination(",
                "quicklog::redate(",
                "quicklog::move_many(",
                "quicklog::merge(",
            )
        ),
        "五个随手记命令改走记录库",
    )
    check(
        "legacy_write_paths_retired",
        not any(
            token in lib
            for token in (
                "worklog::append_entry",
                "worklog::set_destination(",
                "worklog::move_entry",
                "worklog::move_entries",
                "worklog::merge_entry",
            )
        ),
        "旧周文件写路径不再被命令调用",
    )
    check(
        "legacy_read_retained",
        "worklog::read_week" in lib and '"source": "legacy"' in lib,
        "旧读函数保留作迁移回退",
    )
    check(
        "week_index_from_records",
        "quicklog::week_entries" in lib
        and "render_week_markdown" in lib
        and "quicklog::list_record_weeks" in lib,
        "周索引与周内容从记录库聚合",
    )
    check(
        "iso_week_aggregation",
        "iso_week_of_days" in quick and "pub fn week_of_date(" in quick,
        "按 record_date 的 ISO 周聚合",
    )
    check(
        "protocol_and_ipc_unchanged",
        all(
            token in api
            for token in (
                "worklogAppend(",
                "worklogSetDestination(",
                "worklogRedate(",
                "worklogMoveMany(",
                "worklogMerge(",
            )
        )
        and all(token in chat for token in ("shell.worklogAppend", "shell.worklogMerge", "shell.worklogMoveMany")),
        "判定块协议与 IPC 形状不变",
    )
    check(
        "types_have_destination",
        "destination: string | null;" in types and "destination?: string" in types,
        "前端记录类型同步 destination",
    )
    check(
        "move_batch_rolls_back",
        "已回滚本批改动" in quick and "snapshots" in quick,
        "批量改日期失败时回滚本批",
    )
    check(
        "no_weekly_file_write_in_quicklog",
        "week_path" not in quick and "append_entry" not in quick,
        "记录库模块不写周文件",
    )
    check(
        "week_markdown_legacy_shape",
        '- **事**：' in quick and '## {}（续）' in quick,
        "记录库可渲染旧周文件形状供周索引读取",
    )
    check(
        "tests_cover_operations",
        all(
            name in quick
            for name in (
                "append_writes_record_with_destination_and_index",
                "set_destination_requires_matching_text",
                "redate_moves_within_week_and_reports_new_index",
                "merge_appends_single_line_without_overwriting",
                "move_many_rolls_back_when_one_request_mismatches",
            )
        ),
        "五个写入操作的用例齐全",
    )
    check(
        "legacy_week_file_parser_still_present",
        "pub fn read_week" in legacy and "pub fn list_weeks" in legacy,
        "旧周文件读函数仍在，供迁移与回看",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-310",
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
