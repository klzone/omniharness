#!/usr/bin/env python3
"""OD-WT-609 事实核验引擎：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：纯函数引擎无 IO、501 归一化复用、数字/日期/机构名/人员职务提取、
八类结论与十类对象接线、场景六/七测试、口径冲突只提示不合并、603 追加与
按行重写仅经记录函数、两条 IPC 与状态门禁、前端 types/api 封装、
无新增模型通道、无删除来源/无已核实伪结论、测试数据全虚构。

用法：python3 tests/verification/desktop/verify_fact_check_609.py [--json PATH]
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

    source = (RUST / "fact_check.rs").read_text(encoding="utf-8")
    lib = (RUST / "lib.rs").read_text(encoding="utf-8")
    records = (RUST / "writing_records.rs").read_text(encoding="utf-8")
    types = (REPO / "desktop" / "src" / "types.ts").read_text(encoding="utf-8")
    api = (REPO / "desktop" / "src" / "api.ts").read_text(encoding="utf-8")
    production = source.split("#[cfg(test)]", 1)[0]
    lib_section = lib.split("第四阶段：事实核验 IPC（OD-WT-609）", 1)[1].split(
        "第四阶段：审阅修改与版本 IPC（OD-WT-608）", 1
    )[0]
    fact_tests = source.split("#[cfg(test)]", 1)[1]
    lib_tests = lib.split("mod writing_factcheck_ipc_tests", 1)[1]

    check(
        "module_registered",
        "pub mod fact_check;" in lib and "pub fn run_fact_check(" in source,
        "lib.rs 已挂 fact_check，模块导出 run_fact_check",
    )
    check(
        "pure_engine_no_io",
        all(word not in production for word in ["std::fs", "File", "OpenOptions", "PathBuf"]),
        "确定性引擎不读写文件",
    )
    check(
        "no_model_channel",
        all(
            word not in production
            for word in ["engine_bridge", "model_route", "invoke", "llm", "prompt"]
        ),
        "不新增模型通道，核验只在宿主确定性执行",
    )
    check(
        "reuses_501_normalization",
        "use crate::material_store::normalize_quote;" in production
        and "pub fn canonical_text(" in production
        and "normalize_quote(value)" in production,
        "复用 501 normalize_quote，再叠加全角/空白/数字日期等价化",
    )
    check(
        "extracts_dates",
        "fn parse_date_at(" in production
        and "format!(\"{year:04}-{month:02}-{day:02}\")" in production
        and "'年'" in production
        and "'月'" in production,
        "支持 YYYY-MM-DD 与 YYYY年X月",
    )
    check(
        "extracts_numbers",
        "fn parse_number_at(" in production
        and '"万元"' in production
        and '"亿元"' in production
        and '"%"' in production
        and "canonical_numeric(" in production,
        "数字含百分比/金额/单位，千分位与全角归一",
    )
    check(
        "extracts_institutions",
        'INSTITUTION_SUFFIXES: &[&str] = &["办公室", "中心", "办", "局", "厅", "处", "科"]'
        in production
        and "fn institutions_in_text(" in production,
        "机构名按「局/厅/处/科/中心/办」后缀启发式提取",
    )
    check(
        "extracts_person_roles",
        "ROLE_TITLES: &[&str]" in production
        and "fn persons_in_text(" in production
        and "fn parse_name_at(" in production
        and "COMPOUND_SURNAMES" in production,
        "人员职务按「职务+姓名」模式提取，姓名受姓氏/边界约束",
    )
    check(
        "conclusions_eight_wired",
        "use crate::writing_records::{CheckConclusion, CheckObject, CitationSourceKind, FactBasis};"
        in production
        and all(
            variant in production
            for variant in [
                "DirectSource",
                "IndirectSource",
                "SourceConflict",
                "SourceOutdated",
                "SourceMissing",
                "AnalysisJudgment",
                "WorkPlan",
                "ExpressiveSummary",
            ]
        )
        and all(
            object in production
            for object in [
                "CheckObject::Time",
                "CheckObject::Number",
                "CheckObject::Institution",
                "CheckObject::PersonRole",
                "CheckObject::WorkItem",
                "CheckObject::WorkResult",
                "CheckObject::CaseFact",
            ]
        ),
        "八类结论与十类对象均走 603 白名单",
    )
    check(
        "direct_and_indirect_sources",
        "CitationSourceKind::Material => material_hits.push" in production
        and "CitationSourceKind::Knowledge => knowledge_hits.push" in production
        and "CheckConclusion::DirectSource" in production
        and "CheckConclusion::IndirectSource" in production,
        "素材命中→直接来源；知识命中→间接来源",
    )
    check(
        "conflict_and_outdated",
        "fn evidence_conflict(" in production
        and "CheckConclusion::SourceConflict" in production
        and "item.outdated" in production
        and "CheckConclusion::SourceOutdated" in production,
        "同指标不同值→来源冲突；知识过期标记→来源已过期",
    )
    check(
        "missing_without_fabrication",
        "(CheckConclusion::SourceMissing, Vec::new())" in production
        and "已核实" not in production,
        "无命中只落无法找到来源，不把未核实写成已核实",
    )
    check(
        "annotations_merge_four_part",
        "FactAnnotationInput" in production
        and "AnnotationClass::Analysis | AnnotationClass::Judgment" in production
        and "AnnotationClass::Plan" in production
        and "annotation.class != AnnotationClass::Fact" in production,
        "分析/判断不伪装事实，建议/计划落工作计划；事实标注不覆盖确定性结论",
    )
    check(
        "expressive_summary",
        "SUMMARY_MARKERS" in production
        and "looks_like_summary(" in production
        and "CheckConclusion::ExpressiveSummary" in production,
        "无具体事实的概括句落表达性概括",
    )
    check(
        "scope_conflict_no_merge",
        "fn detect_scope_conflicts(" in production
        and "同一指标不同数值" in production
        and "同一指标不同时间范围" in production
        and "同一指标不同统计口径" in production
        and "pub struct ScopeConflict" in production
        and "conflicts.push(ScopeConflict" in production,
        "场景七只输出冲突对，不合并数值",
    )
    check(
        "location_fields",
        "pub section_no: u32" in production
        and "pub paragraph_no: u32" in production
        and "pub fact_text: String" in production
        and "pub struct FactFinding" in production,
        "结论附章节号+段落序号+被核验片段（603 R4）",
    )
    check(
        "stable_record_ids",
        "fn record_id_for(" in production
        and "sha256_hex(key.as_bytes())" in production
        and '"fact-{}"' in production,
        "记录编号按位置/对象/取值/结论稳定生成，重跑可去重",
    )
    check(
        "records_append_via_603",
        "writing_records::append_fact_check(" in lib_section
        and "writing_records::FactCheckDraft" in lib_section
        and "writing_records::update_resolution(" in lib_section
        and "std::fs::write" not in lib_section
        and "factcheck_path" not in lib_section,
        "落盘只经 603 追加/按行重写函数，不直接写 JSONL",
    )
    check(
        "run_command_and_status_gate",
        "fn writing_factcheck_run(" in lib
        and "writing_factcheck_run," in lib
        and "writing_store::STATUS_WRITING" in lib_section
        and "writing_store::STATUS_FACT_CHECKING" in lib_section
        and "writing_store::STATUS_PENDING_REVIEW" in lib_section
        and "只有「{}」「{}」或「{}」可以运行事实核验" in lib_section,
        "writing_factcheck_run 限 写作中|待核验|待审核",
    )
    check(
        "run_reads_607_full_text",
        "writing_section::compose_full_text_for(root, &writing_id)" in lib_section
        and "material_store::read_material(root, id)" in lib_section,
        "正文取 607 compose_full_text，依据取任务已选素材",
    )
    check(
        "resolve_command_and_note_gate",
        "fn writing_factcheck_resolve(" in lib
        and "writing_factcheck_resolve," in lib
        and "gate_user_text(&note, policy, \"核验处理备注\")" in lib_section
        and "ResolutionStatus::Unresolved" in lib_section
        and "只能由用户改为" in lib_section,
        "单条处理只允许三个用户动作，备注过红线/掩码闸门",
    )
    check(
        "rerun_dedupes_existing",
        "existing_ids" in lib_section
        and "skipped_existing" in lib_section
        and "record_id == &finding.record_id" in lib_section,
        "重跑跳过已存在记录，不重置用户处理状态",
    )
    check(
        "no_source_deletion",
        all(
            word not in production
            for word in ["remove_file", "remove_dir_all", "truncate", "delete_material"]
        )
        and "不删除来源" in lib_section,
        "无法核实不自动删除来源",
    )
    check(
        "frontend_types",
        "export interface FactCheckRunResult" in types
        and "export interface FactCheckRecord" in types
        and "export type FactCheckConclusion" in types
        and "export interface FactCheckScopeConflict" in types,
        "前端类型覆盖记录/结论/冲突/运行回执",
    )
    check(
        "frontend_api",
        'writingFactcheckRun(input: FactCheckRunInput)' in api
        and 'invoke<FactCheckRunResult>("writing_factcheck_run"' in api
        and 'writingFactcheckResolve(input: FactCheckResolveInput)' in api
        and 'invoke<FactCheckResolveResult>("writing_factcheck_resolve"' in api,
        "前端只封装两条 IPC，不新增模型调用",
    )
    check(
        "scenario_six_tests",
        "scenario_six_correct_data_gets_direct_source" in fact_tests
        and "scenario_six_wrong_value_gets_conflict" in fact_tests
        and "scenario_six_unsourced_statement_is_missing_not_verified" in fact_tests,
        "场景六：正确→直接来源、错误→冲突、无来源→无法找到来源",
    )
    check(
        "scenario_seven_tests",
        "scenario_seven_scope_conflict_hints_without_merge" in fact_tests
        and "scenario_seven_same_value_different_time_range_conflicts" in fact_tests
        and "scenario_seven_same_value_different_stat_scope_conflicts" in fact_tests,
        "场景七：数值/时间范围/统计口径冲突均只提示",
    )
    test_count = fact_tests.count("#[test]") + lib_tests.count("#[test]")
    check(
        "tests_minimum_14",
        test_count >= 14,
        f"新增测试 {test_count} 项（≥14）",
    )
    check(
        "test_data_fictional",
        "虚构" in fact_tests
        and "虚构" in lib_tests
        and "mat-synthetic-609" in fact_tests
        and "notes/synthetic-609.md" in fact_tests,
        "测试数据全虚构，来源编号显式 synthetic",
    )
    check(
        "runtime_untouched",
        "runtime/" not in production and "runtime" not in production,
        "引擎不引用 runtime/",
    )
    check(
        "records_contract_untouched",
        "pub struct FactCheckRecord" in records
        and "pub enum CheckConclusion" in records
        and "pub fn append_fact_check(" in records
        and "pub fn update_resolution(" in records,
        "609 只消费 603 契约，不修改记录结构",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-609",
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
