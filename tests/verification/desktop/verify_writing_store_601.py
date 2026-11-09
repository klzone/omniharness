#!/usr/bin/env python3
"""OD-WT-601 写作任务实体：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：模块注册、schema/目录、字段齐全、九态顺序、十二类文体、CRUD 与过滤、
状态迁移表与大纲确认门禁、归档/取消约束、非法迁移留痕、状态记录只追加、
原子写、写作目录清理排除、无数据库依赖、无知识库写路径，以及 ≥14 项模块测试。

用法：python3 tests/verification/desktop/verify_writing_store_601.py [--json PATH]
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


def block(source: str, marker: str) -> str:
    """取 marker 之后到第一个独立 '}' 之间的源码片段。"""
    return source.split(marker, 1)[1].split("}", 1)[0]


def ordered_positions(text: str, names: list[str]) -> bool:
    positions = [text.find(name) for name in names]
    return all(position >= 0 for position in positions) and positions == sorted(positions)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    source = (RUST / "writing_store.rs").read_text(encoding="utf-8")
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")
    cleanup = (RUST / "work_cleanup.rs").read_text(encoding="utf-8")
    production = source.split("#[cfg(test)]")[0]

    check(
        "module_registered",
        "pub mod writing_store;" in lib and "pub struct WritingTask" in source,
        "lib.rs 已挂 writing_store，模块导出 WritingTask",
    )
    check(
        "schema_and_dir",
        'WRITING_SCHEMA: &str = "omnidesk.writing.v1"' in production
        and 'WRITING_DIR: &str = "写作"' in production
        and 'format!("{id}.md")' in production,
        "work/写作/<writing_id>.md，schema omnidesk.writing.v1",
    )

    struct_block = block(source, "pub struct WritingTask")
    required_fields = [
        "writing_id",
        "title",
        "document_type",
        "writing_purpose",
        "audience",
        "writing_period",
        "topic",
        "requirements",
        "expected_length",
        "writing_style",
        "selected_material_ids",
        "selected_knowledge_ids",
        "document_id",
        "status",
        "created_at",
        "updated_at",
    ]
    check(
        "struct_fields",
        all(f"pub {field}:" in struct_block for field in required_fields),
        "WritingTask 含任务卡列出的全部字段",
    )
    check(
        "serde_deny_unknown_fields",
        "#[serde(deny_unknown_fields)]" in production
        and "struct WritingFront" in production,
        "frontmatter 解析结构照 material_store 启用 deny_unknown_fields",
    )

    status_constants = [
        ("STATUS_NEEDS_ORGANIZING", "需求待整理"),
        ("STATUS_COLLECTING_MATERIALS", "素材收集中"),
        ("STATUS_OUTLINE_PENDING", "大纲待确认"),
        ("STATUS_WRITING", "写作中"),
        ("STATUS_FACT_CHECKING", "待核验"),
        ("STATUS_PENDING_REVIEW", "待审核"),
        ("STATUS_FINALIZED", "已定稿"),
        ("STATUS_ARCHIVED", "已归档"),
        ("STATUS_CANCELLED", "已取消"),
    ]
    status_list = production.split("pub const WRITING_STATUSES", 1)[1].split("];", 1)[0]
    check(
        "statuses_nine_in_card_order",
        all(f'{name}: &str = "{value}"' in production for name, value in status_constants)
        and ordered_positions(status_list, [name for name, _ in status_constants]),
        "九态白名单完整，WRITING_STATUSES 顺序与卡面一致",
    )

    document_types = [
        ("TYPE_MONTHLY_SUMMARY", "月度总结"),
        ("TYPE_ANNUAL_SUMMARY", "年度总结"),
        ("TYPE_SPECIAL_REPORT", "专项汇报"),
        ("TYPE_EXPERIENCE_EXCHANGE", "经验交流"),
        ("TYPE_TYPICAL_CASE", "典型案例"),
        ("TYPE_RESEARCH_REPORT", "调研报告"),
        ("TYPE_WORK_PLAN", "工作方案"),
        ("TYPE_LEADER_SPEECH", "领导讲话"),
        ("TYPE_WORK_BRIEF", "工作简报"),
        ("TYPE_TOPIC_ANALYSIS", "专题分析"),
        ("TYPE_MEETING_SPEECH", "会议发言"),
        ("TYPE_PERSONAL_REPORT", "个人述职"),
    ]
    type_list = production.split("pub const DOCUMENT_TYPES", 1)[1].split("];", 1)[0]
    check(
        "document_types_twelve_in_card_order",
        all(f'{name}: &str = "{value}"' in production for name, value in document_types)
        and ordered_positions(type_list, [name for name, _ in document_types]),
        "十二类文稿白名单完整，DOCUMENT_TYPES 顺序与卡面一致",
    )

    check(
        "crud_surface",
        all(
            f"pub fn {name}" in production
            for name in [
                "create_writing_task",
                "read_writing_task",
                "list_writing_tasks",
                "update_writing_task",
                "delete_writing_task",
            ]
        ),
        "创建/读取/列表/编辑/删除 五类入口齐全",
    )
    filter_block = block(source, "pub struct WritingTaskFilter")
    check(
        "filters_status_type_time",
        all(f"pub {field}:" in filter_block for field in ["status", "document_type", "date_from", "date_to"])
        and "fn matches_filter(" in production
        and "fn validate_filter(" in production,
        "按状态/类型/创建日期区间过滤",
    )

    check(
        "transition_table",
        "pub fn is_allowed_transition(" in production
        and "(STATUS_OUTLINE_PENDING, STATUS_WRITING) => outline_confirmed" in production
        and "(STATUS_FINALIZED, STATUS_ARCHIVED) => true" in production
        and "if to == STATUS_CANCELLED" in production
        and "from != STATUS_ARCHIVED && from != STATUS_CANCELLED" in production,
        "迁移表含大纲门禁、归档仅从已定稿、取消排除已归档",
    )
    check(
        "outline_confirmation_parameter",
        "pub fn transition_writing_status(" in production
        and "outline_confirmed: bool" in production
        and "必须确认大纲" in production,
        "大纲待确认→写作中由 is_outline_confirmed 参数放行",
    )
    check(
        "archive_and_cancel_messages",
        "只有已定稿可以归档" in production and "已归档任务不能取消" in production,
        "归档与取消的非法迁移有明确拒绝文案",
    )
    rejection = production.split("fn record_rejection(", 1)[1].split(
        "pub fn transition_writing_status", 1
    )[0]
    check(
        "illegal_transition_logged",
        "ACTION_REJECT" in rejection
        and "status_records.push(StatusRecord" in rejection
        and "persist(root, &task)" in rejection
        and "status_records.push(StatusRecord" in production,
        "非法迁移拒绝时追加拒绝记录并落盘，状态保持不变",
    )

    check(
        "status_section_append_only",
        '"## 状态记录"' in production
        and "pub struct StatusRecord" in production
        and 'status_records.push(StatusRecord' in production
        and "status_records.clear" not in production
        and "status_records: vec![" in production,
        "## 状态记录 只追加：创建写首条，后续 push，不清空历史",
    )
    patch_block = block(source, "pub struct WritingTaskPatch")
    check(
        "status_not_patchable",
        "pub status:" not in patch_block
        and "status: STATUS_NEEDS_ORGANIZING.to_string()" in production,
        "创建默认需求待整理，状态不在编辑补丁里，只能经迁移动作",
    )

    check(
        "atomic_writes",
        "write_new_atomically" in production
        and "write_replace_atomically" in production
        and "fn persist(root" in production,
        "新建与改写都用 worklog_records 的原子写",
    )
    check(
        "timestamps_validated",
        "fn is_timestamp_shape(" in production
        and "创建时间形状不正确" in production
        and "更新时间形状不正确" in production,
        "created_at/updated_at 与状态记录时间做形状校验",
    )
    check(
        "link_ids_normalized",
        "fn normalize_link_ids(" in production
        and "selected_material_ids" in production
        and "selected_knowledge_ids" in production
        and "trimmed" in production,
        "选中素材/知识编号去空白、去重并拒控制字符",
    )

    check(
        "cleanup_exclusion",
        "CLEANUP_EXCLUDED_DIRS" in cleanup
        and "pub fn is_cleanup_excluded(" in cleanup
        and "writing_store::WRITING_DIR" in cleanup
        and cleanup.count("if is_cleanup_excluded(&source)") >= 2,
        "清理预览/执行路径显式排除 work/写作/",
    )
    check(
        "cleanup_exclusion_test",
        "fn writing_dir_is_excluded_from_preview_and_apply" in cleanup
        and "写作任务文件不得被清理移动" in cleanup
        and "写作任务文件字节不得变化" in cleanup,
        "写作目录清理排除有模块内测试，断言不移动且字节不变",
    )

    check(
        "no_database_dependency",
        all(word not in production for word in ["rusqlite", "sqlite", "diesel", "sea_orm"]),
        "不引入数据库",
    )
    check(
        "vault_write_path_untouched",
        all(word not in production for word in ["knowledge_vault", "vault_write", "vault_archive"]),
        "写作存储不引用知识库写路径",
    )
    check(
        "runtime_untouched",
        "runtime" not in production.replace("不改变前三阶段交付结构", ""),
        "写作模块不引用 runtime/ 实现",
    )

    test_count = source.split("#[cfg(test)]")[1].count("#[test]")
    check(
        "tests_minimum_14",
        test_count >= 14,
        f"模块内测试 {test_count} 项（≥14）",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-601",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [
            {"name": name, "ok": ok, "detail": detail}
            for name, ok, detail in CHECKS
        ],
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
