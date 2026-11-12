#!/usr/bin/env python3
"""OD-WR-508 工作记录批次安全断言（静态源码核对，不读用户数据、不调用模型）。

覆盖：
  * 原始素材不被修订覆盖：RecordPatch 无 raw_input/raw_source，摘要/复盘无 Patch 通道；
  * AI 无确认态写通道：HomeChat/conversationPrompt 不直接 invoke，写均走宿主 IPC；
  * 复盘已确认不覆盖：confirm 后 write_review 拒绝，且单测冻结确认版字节；
  * 引用 ids 真实性：摘要/复盘都有引用校验，并被生成 IPC 路径调用；
  * 测试全虚构：本轮新增测试文件不含真实材料特征词。

用法：python3 tests/verification/desktop/verify_wr_security_508.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RUST = REPO / "desktop" / "src-tauri" / "src"
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def struct_body(text: str, name: str) -> str:
    marker = f"pub struct {name} {{"
    if marker not in text:
        return ""
    return text.split(marker, 1)[1].split("\n}", 1)[0]


def fn_body(text: str, name: str) -> str:
    marker = f"pub fn {name}("
    if marker not in text:
        marker = f"fn {name}("
    if marker not in text:
        return ""
    return text.split(marker, 1)[1].split("\n}\n", 1)[0]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    store = read(RUST / "worklog_records.rs")
    quicklog = read(RUST / "quicklog.rs")
    daily = read(RUST / "daily_summary.rs")
    review = read(RUST / "review_store.rs")
    lib = read(RUST / "lib.rs")
    home = read(SRC / "shell" / "HomeChat.tsx")
    conv = read(SRC / "conversationPrompt.ts")
    archive = read(SRC / "shell" / "ReviewArchive.tsx")

    patch = struct_body(store, "RecordPatch")
    update = fn_body(store, "update_record")
    write_summary = fn_body(daily, "write_summary")
    write_review = fn_body(review, "write_review")
    confirm_review = fn_body(review, "confirm_review")
    confirmed_test = fn_body(review, "confirmed_review_is_never_overwritten")

    # 1. 原始素材只读：补丁结构没有 raw 字段，更新路径不写 raw。
    check(
        "record_patch_has_no_raw_fields",
        patch != "" and "raw_input" not in patch and "raw_source" not in patch and "pub body" in patch,
        "RecordPatch 只包含可修订字段",
    )
    check(
        "update_record_never_assigns_raw",
        update != "" and "raw_input =" not in update and "raw_source =" not in update,
        "update_record 不写 raw_input/raw_source",
    )
    check(
        "raw_write_only_in_create_path",
        "raw_input: Some(text.to_string())" in quicklog and "raw_source: Some(format!(" in quicklog,
        "随手记创建时写入原始输入与来源",
    )
    check(
        "summary_review_have_no_patch_channel",
        "RecordPatch" not in daily
        and "RecordPatch" not in review
        and "update_record" not in daily
        and "update_record" not in review,
        "摘要/复盘模块不构造记录补丁，也不调用更新记录",
    )
    check(
        "raw_preservation_unit_test",
        "fn patch_has_no_raw_input_entry_and_update_preserves_raw" in store,
        "501 单测覆盖补丁无 raw 入口且更新后原文不变",
    )

    # 2. AI 无确认态写通道：前端只调 api 封装，不直接 invoke，也不确认复盘。
    check(
        "homechat_no_direct_invoke",
        "invoke(" not in home and "@tauri-apps/api/core" not in home,
        "HomeChat 不直接调用 Tauri invoke",
    )
    check(
        "conversation_prompt_transport_free",
        "invoke(" not in conv
        and "@tauri-apps" not in conv
        and "fetch(" not in conv
        and "writeFile" not in conv
        and "node:fs" not in conv,
        "conversationPrompt 只组装提示词，无传输/文件写入",
    )
    check(
        "homechat_writes_via_host_wrappers",
        "shell.worklogAppend(" in home
        and "shell.worklogDailySummaryGenerate(ctx.date, text)" in home
        and "shell.worklogPeriodReviewGenerate(" in home,
        "正式写入只经 api 的宿主校验封装",
    )
    check(
        "homechat_has_no_review_confirm",
        "worklogPeriodReviewConfirm" not in home and "confirm_review" not in home,
        "AI 对话链路没有复盘确认调用",
    )
    check(
        "daily_summary_host_validated",
        "fn worklog_daily_summary_generate" in lib
        and "daily_summary::write_summary" in lib
        and "let (body, record_ids) = validate_body(content, &records)?;" in daily,
        "摘要生成 IPC 进入宿主校验",
    )
    check(
        "period_review_host_validated",
        "fn worklog_period_review_generate" in lib
        and "review_store::write_review" in lib
        and "let (body, record_ids, task_ids) = validate_body(content, period, &bundle.records, &bundle.tasks)?;"
        in review,
        "复盘生成 IPC 进入宿主校验",
    )

    # 3. 复盘已确认不覆盖。
    check(
        "confirm_review_exists",
        "pub fn confirm_review" in review and "STATUS_CONFIRMED" in confirm_review,
        "确认流把草稿置为已确认",
    )
    check(
        "write_review_refuses_confirmed",
        "Some(STATUS_CONFIRMED) =>" in write_review
        and "该周期复盘已确认，不允许重新生成覆盖" in write_review,
        "已确认复盘拒绝重新生成",
    )
    check(
        "confirmed_review_unit_test_freezes_bytes",
        confirmed_test != ""
        and "frozen" in confirmed_test
        and "assert_eq!(fs::read_to_string(&receipt.path).unwrap(), frozen" in confirmed_test,
        "505 单测断言确认版字节不变",
    )
    check(
        "review_ui_disables_regen_for_confirmed",
        'disabled={busy || detail.status !== "草稿"}' in archive
        and "已确认复盘不可覆盖" in archive
        and "data-review-confirmed-lock" in archive,
        "档案页对已确认复盘禁用重新生成",
    )

    # 4. 引用 ids 真实性：两套校验函数存在，且生成路径调用它们。
    check(
        "daily_summary_citation_validation",
        "fn validate_body(content: &str, records: &[WorkRecord])" in daily
        and "fn citation_ids(text: &str) -> Vec<String>" in daily
        and "摘要引用了不存在的记录编号" in daily
        and "摘要段落没有标注引用记录" in daily,
        "摘要校验五段结构与 #wr- 引用真实性",
    )
    check(
        "period_review_citation_validation",
        "fn validate_body(" in review
        and "fn citation_ids(text: &str) -> (Vec<String>, Vec<String>)" in review
        and "复盘引用了不存在的记录编号" in review
        and "复盘引用了不存在的任务编号" in review
        and "复盘段落没有标注引用编号" in review,
        "复盘校验记录/任务引用真实性与段落引用",
    )
    check(
        "citation_validation_reached_from_ipc",
        "worklog_daily_summary_generate," in lib
        and "worklog_period_review_generate," in lib
        and "let (body, record_ids) = validate_body(content, &records)?;" in write_summary
        and "let (body, record_ids, task_ids) = validate_body(content, period, &bundle.records, &bundle.tasks)?;"
        in write_review,
        "两个生成 IPC 注册且校验发生在落盘前",
    )
    check(
        "unknown_id_tests_exist",
        "fn write_rejects_unknown_record_id_without_touching_disk" in daily
        and "fn write_rejects_unknown_record_id_without_touching_disk" in review
        and "fn write_rejects_unknown_task_id" in review,
        "摘要/复盘都有未知引用拒绝单测",
    )

    # 5. 本轮新增测试文件不含真实业务材料特征。
    new_tests = [
        HERE / "verify_wr_503.py",
        HERE / "verify_wr_504.py",
        HERE / "verify_wr_505.py",
        HERE / "verify_wr_506.py",
        HERE / "verify_wr_507.py",
        HERE / "verify_wr_detail_502.py",
        RUST / "daily_summary.rs",
        RUST / "review_store.rs",
    ]
    markers = ["案件", "线索", "人名", "身份证", "手机号", "用户成果", "regulations"]
    hits: list[str] = []
    for path in new_tests:
        text = read(path)
        for marker in markers:
            if marker in text:
                hits.append(f"{path.name}:{marker}")
    check(
        "new_tests_use_synthetic_material_only",
        not hits,
        ",".join(hits),
    )
    check(
        "new_tests_have_synthetic_fixtures",
        "\"甲\"" in daily
        and "\"乙\"" in daily
        and "\"甲\"" in review
        and "\"乙\"" in review,
        "摘要/复盘单测使用甲、乙等合成对象",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-508",
        "evidence_level": "静态源码检查",
        "passed": not failed,
        "total": len(CHECKS),
        "passed_count": len(CHECKS) - len(failed),
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
