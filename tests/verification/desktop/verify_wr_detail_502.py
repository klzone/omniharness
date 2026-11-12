#!/usr/bin/env python3
"""OD-WR-502 记录详情修订历史与重新整理：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：原始素材只读区块与旧记录隐藏、修订历史只读 IPC 与展示字段、
重新整理确认与首页预填、五类快捷 chips、页面导航接线。
运行时行为由 tsc / build 与后续真实窗口验收覆盖。

用法：python3 tests/verification/desktop/verify_wr_detail_502.py [--json PATH]
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

    library = read(SRC / "shell" / "WorkRecordLibrary.tsx")
    page = read(SRC / "shell" / "WorkRecordPage.tsx")
    api = read(SRC / "api.ts")
    types = read(SRC / "types.ts")
    store = read(RUST / "worklog_records.rs")
    host = read(RUST / "lib.rs")

    check(
        "types_record_has_readonly_raw_fields",
        "raw_input: string | null;" in types
        and "raw_source: string | null;" in types
        and "只读" in types,
        "WorkRecord 类型带 raw_input / raw_source，标注只读",
    )
    check(
        "types_revision_summary_contract",
        "interface WorkRecordRevision" in types
        and "changed_fields: string[];" in types
        and "note: string;" in types,
        "修订摘要类型与宿主返回结构成套",
    )
    check(
        "api_readonly_revisions_ipc",
        "workRecordRevisions(id: string)" in api
        and 'invoke<WorkRecordRevision[]>("worklog_revisions"' in api
        and "{ recordId: id }" in api,
        "前端封装调用 worklog_revisions(record_id)",
    )
    check(
        "host_revision_summary_excludes_sha",
        "pub struct RevisionSummary" in store
        and "pub at: String" in store
        and "pub changed_fields: Vec<String>" in store
        and "pub note: String" in store
        and "old_body_sha256" not in store.split("pub struct RevisionSummary", 1)[1].split("}", 1)[0],
        "宿主摘要只含 at / changed_fields / note，不含正文哈希",
    )
    check(
        "host_reads_revisions_file_with_legacy_empty",
        "pub fn read_revision_summaries" in store
        and "if !path.exists()" in store
        and "return Ok(Vec::new())" in store
        and "revisions.jsonl" in store,
        "缺失 revisions.jsonl 的旧记录返回空列表",
    )
    check(
        "host_command_registered",
        "fn worklog_revisions(" in host
        and "worklog_records::read_revision_summaries" in host
        and "worklog_revisions," in host,
        "只读命令已注册到 Tauri handler",
    )
    check(
        "raw_input_section_rendered",
        "selected.raw_input?.trim() ?" in library
        and 'data-raw-input=""' in library
        and "原始输入" in library
        and "原始素材，不可修改" in library
        and "selected.raw_source" in library
        and 'data-raw-source=""' in library,
        "原始素材折叠区块显示 raw_input 与 raw_source",
    )
    check(
        "legacy_record_hides_raw_section",
        "selected.raw_input?.trim() ? (" in library
        and "\n                ) : null}" in library,
        "没有 raw_input 的旧记录不渲染该区块",
    )
    check(
        "revision_history_rendered_without_sha",
        "data-revision-history" in library
        and "修订历史" in library
        and "data-revision-item" in library
        and "revisionTime(" in library
        and "changedFieldsLabel(" in library
        and "old_body_sha256" not in library
        and "sha256" not in library,
        "修订历史只显示时间与变更字段，不显示 sha 细节",
    )
    check(
        "revision_history_reads_selected_record",
        ".workRecordRevisions(recordId)" in library
        and "setRevisionsLoading(true)" in library
        and "修订历史读取失败" in library
        and "还没有修订记录。" in library,
        "切换记录时读取历史，失败与空态如实显示",
    )
    check(
        "reorganize_entry_visible_for_raw_records",
        'data-reorganize-entry=""' in library
        and "selected.raw_input?.trim() ?" in library
        and "重新整理" in library,
        "有原始素材的记录显示重新整理入口",
    )

    reorg_start = library.find("function reorganizeSelected")
    reorg_end = library.find("  return (", reorg_start)
    reorg = library[reorg_start:reorg_end] if reorg_start >= 0 and reorg_end > reorg_start else ""
    check(
        "reorganize_confirm_explains_body_rewrite",
        "title=\"依据原始素材重新整理\"" in library
        and "重写正文" in library
        and "原始素材不会被修改" in library,
        "确认框说明会依据原始素材重写正文",
    )
    check(
        "reorganize_prefills_home_chat",
        "请依据以下原始素材重新整理这条记录：" in reorg
        and 'requestNewThread(' in reorg
        and 'onNavigate("home")' in reorg
        and ".workRecordUpdate(" not in reorg,
        "确认后打开首页对话并预填素材，不直接改正文",
    )
    check(
        "category_chips_five_suggestions",
        'data-category-chips=""' in library
        and all(
            label in library
            for label in ("工作事项", "学习记录", "工作思考", "问题与经验", "个人生活")
        )
        and "setForm({ ...form, category })" in library,
        "五类建议 chips 点击填入分类",
    )
    check(
        "category_remains_free_text",
        "onChange={(e) => setForm({ ...form, category: e.target.value })}" in library
        and 'placeholder="分类"' in library,
        "快捷 chips 不锁死分类输入，仍可自由填写",
    )
    check(
        "page_passes_navigation",
        "onNavigate={onNavigate}" in page and "<WorkRecordLibrary" in page,
        "记录库获得首页导航回调",
    )
    check(
        "rust_tests_cover_revision_summary",
        "missing_revision_history_reads_as_empty_for_legacy_record" in store
        and "revision_summaries_keep_order_and_omit_body_hash" in store
        and "corrupt_revision_line_fails_instead_of_hiding_history" in store,
        "新增修订读取函数测试至少两项（实际三项）",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WR-502",
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
