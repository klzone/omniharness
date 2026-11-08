#!/usr/bin/env python3
"""OD-KM-501 写作素材实体：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：模块注册、schema/目录、字段齐全、十类素材类型、五态审核、来源白名单、
sha256 指纹与重复校验、审核记录只追加、CRUD 与过滤、素材篮清理排除、
知识库写路径零引用、无数据库依赖，以及 ≥15 项模块测试存在性。

用法：python3 tests/verification/desktop/verify_material_store_501.py [--json PATH]
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

    source = (RUST / "material_store.rs").read_text(encoding="utf-8")
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")
    cleanup = (RUST / "work_cleanup.rs").read_text(encoding="utf-8")
    production = source.split("#[cfg(test)]")[0]

    check(
        "module_registered",
        "pub mod material_store;" in lib and "pub struct WritingMaterial" in source,
        "lib.rs 已挂 material_store，模块导出 WritingMaterial",
    )
    check(
        "schema_and_dir",
        'MATERIAL_SCHEMA: &str = "omnidesk.material.v1"' in source
        and 'MATERIAL_DIR: &str = "素材篮"' in source
        and 'format!("{id}.md")' in source,
        "work/素材篮/<material_id>.md，schema omnidesk.material.v1",
    )

    struct_block = source.split("pub struct WritingMaterial", 1)[1].split("}", 1)[0]
    required_fields = [
        "material_id",
        "title",
        "content",
        "material_type",
        "source_type",
        "source_id",
        "source_quote",
        "source_location",
        "related_task_ids",
        "related_project_id",
        "related_knowledge_ids",
        "event_date",
        "created_at",
        "updated_at",
        "confidence",
        "review_status",
        "confirmation_status",
        "tags",
    ]
    check(
        "struct_fields",
        all(f"pub {field}:" in struct_block for field in required_fields),
        "WritingMaterial 含任务卡列出的全部字段",
    )

    check(
        "material_types_ten",
        all(
            item in source
            for item in [
                'TYPE_WORK_FACT: &str = "工作事实"',
                'TYPE_WORK_RESULT: &str = "工作成果"',
                'TYPE_DATA: &str = "数据素材"',
                'TYPE_EXPERIENCE: &str = "经验做法"',
                'TYPE_CASE: &str = "典型案例"',
                'TYPE_PROBLEM: &str = "问题不足"',
                'TYPE_POLICY: &str = "制度依据"',
                'TYPE_PLAN: &str = "工作计划"',
                'TYPE_DEPLOYMENT: &str = "领导部署"',
                'TYPE_HIGHLIGHT: &str = "特色亮点"',
            ]
        )
        and "MATERIAL_TYPES: &[&str]" in source,
        "十类素材类型白名单完整",
    )
    check(
        "review_statuses_five",
        all(
            item in source
            for item in [
                'REVIEW_PENDING: &str = "待审核"',
                'REVIEW_CONFIRMED: &str = "已确认"',
                'REVIEW_RETURNED: &str = "已退回"',
                'REVIEW_REJECTED: &str = "已拒绝"',
                'REVIEW_ARCHIVED: &str = "已归档"',
            ]
        )
        and "REVIEW_STATUSES: &[&str]" in source,
        "审核状态五态白名单",
    )
    check(
        "source_types_five",
        all(
            item in production
            for item in [
                'SOURCE_WORK_RECORD: &str = "work_record"',
                'SOURCE_TASK: &str = "task"',
                'SOURCE_PROJECT: &str = "project"',
                'SOURCE_KNOWLEDGE: &str = "knowledge"',
                'SOURCE_USER: &str = "user"',
            ]
        )
        and "SOURCE_TYPES: &[&str]" in production,
        "来源类型白名单 work_record/task/project/knowledge/user",
    )
    check(
        "fingerprint_sha256",
        "use crate::sha256::sha256_hex;" in production
        and "pub fn material_fingerprint(" in production
        and "pub fn normalize_quote(" in production
        and "sha256_hex(format!(" in production,
        "指纹复用 sha256.rs，输入 source_id + 归一化引用",
    )
    check(
        "duplicate_uniqueness",
        "fn duplicate_with(" in production
        and "同指纹" in production
        and "duplicate_with(root, &material.source_id" in production,
        "创建与编辑都校验同指纹唯一性",
    )
    check(
        "review_section_append_only",
        '"## 审核记录"' in production
        and "pub struct ReviewRecord" in production
        and "review_records.push(ReviewRecord" in production
        and "review_records.clear" not in production
        and "review_records: vec![" in production,
        "## 审核记录 只追加：创建写首条，后续 push，不清空历史",
    )
    check(
        "crud_surface",
        all(
            f"pub fn {name}" in production
            for name in [
                "create_material",
                "read_material",
                "list_materials",
                "update_material",
                "delete_material",
                "review_material",
            ]
        ),
        "创建/读取/列表/编辑/删除/审核 六类入口齐全",
    )
    check(
        "filters",
        all(
            f"pub {field}:" in source.split("pub struct MaterialFilter", 1)[1].split("}", 1)[0]
            for field in [
                "material_type",
                "review_status",
                "date_from",
                "date_to",
                "project_id",
                "task_id",
            ]
        )
        and "fn matches_filter(" in production,
        "按类型/状态/时间区间/项目/任务过滤",
    )
    check(
        "atomic_writes",
        "write_new_atomically" in production and "write_replace_atomically" in production,
        "新建与改写都用 worklog_records 的原子写",
    )
    check(
        "cleanup_exclusion",
        "CLEANUP_EXCLUDED_DIRS" in cleanup
        and "pub fn is_cleanup_excluded(" in cleanup
        and "material_store::MATERIAL_DIR" in cleanup
        and cleanup.count("if is_cleanup_excluded(&source)") >= 2,
        "清理预览/执行路径显式排除 work/素材篮/",
    )
    check(
        "cleanup_exclusion_test",
        "fn materials_dir_is_excluded_from_preview_and_apply" in cleanup,
        "素材篮清理排除有模块内测试",
    )
    check(
        "vault_read_only",
        "knowledge_vault" not in production and "vault_write" not in production,
        "素材存储不引用知识库写路径",
    )
    check(
        "no_database_dependency",
        all(word not in production for word in ["rusqlite", "sqlite", "diesel", "sea_orm"]),
        "不引入数据库",
    )
    check(
        "source_quote_required",
        "缺少来源原文引用 source_quote" in production and "source_quote.trim().is_empty()" in production,
        "来源引用必填",
    )
    check(
        "review_status_not_ai_writable",
        "review_status: REVIEW_PENDING.to_string()" in production
        and "pub fn review_material(" in production
        and "ACTION_CONFIRM" in production,
        "创建一律待审核，状态只经 review_material 推进",
    )

    test_count = source.split("#[cfg(test)]")[1].count("#[test]")
    check(
        "tests_minimum_15",
        test_count >= 15,
        f"模块内测试 {test_count} 项（≥15）",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-KM-501",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "failed": failed,
        "checks": [{"name": name, "ok": ok, "detail": detail} for name, ok, detail in CHECKS],
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
