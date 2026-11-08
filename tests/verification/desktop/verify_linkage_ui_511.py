#!/usr/bin/env python3
"""OD-KM-511 关联视图扩展：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：记录/任务详情的关联知识区块、候选理由与原文定位、确认/移除操作、
已关联清单、AI 推荐与用户确认的显式标注、真实空态、回执直接更新本地状态、
知识条目详情反向区块，以及 507 的 api/types 复用。

用法：python3 tests/verification/desktop/verify_linkage_ui_511.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    panel_path = SRC / "shell" / "KnowledgeLinkPanel.tsx"
    record = (SRC / "shell" / "WorkRecordLibrary.tsx").read_text(encoding="utf-8")
    task = (SRC / "shell" / "TaskManager.tsx").read_text(encoding="utf-8")
    vault = (SRC / "shell" / "KnowledgeVaultPage.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    types = (SRC / "types.ts").read_text(encoding="utf-8")
    panel = panel_path.read_text(encoding="utf-8") if panel_path.exists() else ""

    check(
        "panel_module_present",
        panel_path.exists()
        and "export function KnowledgeLinkPanel" in panel
        and "data-knowledge-links" in panel,
        "新增 KnowledgeLinkPanel，记录/任务详情共用同一关联区块",
    )
    check(
        "source_peek_reused",
        'from "./SourcePeekSheet"' in panel
        and "<SourcePeekSheet" in panel
        and "excerpt(note.content, line)" in panel,
        "知识原文定位复用 SourcePeekSheet，按候选行号截取上下文",
    )
    check(
        "suggest_ipc",
        "shell.knowledgeLinkSuggest(subjectKind, subjectId, [])" in panel,
        "推荐候选调用 507 的 knowledge_link_suggest",
    )
    check(
        "confirm_ipc",
        "shell.knowledgeLinkConfirm(candidate.id, reason)" in panel,
        "确认调用 knowledge_link_confirm",
    )
    check(
        "remove_ipc",
        "shell.knowledgeLinkRemove(subjectKind, subjectId, link.id)" in panel,
        "移除调用 knowledge_link_remove",
    )
    check(
        "confirm_reason_required",
        "确认理由（必填）" in panel
        and "if (!reason)" in panel
        and 'disabled={busy !== null || !(reasons[candidate.id] ?? "").trim()}' in panel,
        "确认理由必填：空理由在界面直接拦住",
    )
    check(
        "candidate_reason_and_location",
        "candidate.reason_text" in panel
        and "candidate.path" in panel
        and "candidate.line" in panel
        and "查看原文" in panel,
        "候选附理由与 路径·行号 定位，可打开原文",
    )
    check(
        "ai_user_labels_distinct",
        "AI 推荐" in panel
        and "用户确认" in panel
        and 'data-link-source="ai"' in panel
        and 'data-link-source="user"' in panel,
        "AI 推荐与用户确认分别标注，不用同一视觉标签混在一起",
    )
    check(
        "receipt_updates_local_state",
        "report.record?.related_knowledge ?? report.task?.related_knowledge" in panel
        and "onLinksChanged(next)" in panel,
        "确认/移除后从回执直接更新本地关联清单",
    )
    check(
        "no_updated_at_refresh",
        "setReloadTick" not in panel
        and "selected.updated_at" not in panel
        and "report.record?.updated_at" not in panel,
        "关联区块不依赖 updated_at，也不触发整页刷新",
    )
    check(
        "linked_empty_state",
        "还没有关联知识。" in panel and "data-knowledge-links-empty" in panel,
        "无关联时有真实空态",
    )
    check(
        "panel_resets_on_subject_change",
        "[subjectKind, subjectId]" in panel
        and "setCandidates(null)" in panel
        and "setReasons({})" in panel,
        "切换记录/任务时清空上一条的候选与理由",
    )
    check(
        "candidate_empty_states",
        "还没有推荐候选。点「AI 推荐」从知识库查找。" in panel
        and "没有新的推荐候选。" in panel
        and "data-knowledge-candidates-empty" in panel,
        "未推荐与无新候选分别空态",
    )
    check(
        "work_record_detail_panel",
        "KnowledgeLinkPanel" in record
        and 'subjectKind="record"' in record
        and "links={selected.related_knowledge}" in record
        and "data-work-record-detail" in record,
        "工作记录详情追加关联知识区块，保留原详情结构",
    )
    check(
        "task_detail_panel",
        "KnowledgeLinkPanel" in task
        and 'subjectKind="task"' in task
        and "links={selected.related_knowledge}" in task
        and "data-task-detail" in task,
        "任务详情追加关联知识区块，保留原详情结构",
    )
    check(
        "parents_update_listing_from_receipt",
        "item.id === selected.id ? { ...item, related_knowledge: links } : item" in record
        and "item.id === selected.id ? { ...item, related_knowledge: links } : item" in task,
        "记录与任务都用回执更新详情和列表里的同一对象",
    )
    check(
        "vault_reverse_query",
        "knowledgeLinkByKnowledge(selectedPath)" in vault and "shell" in vault,
        "知识条目详情调用 knowledge_link_by_knowledge 反向查询",
    )
    check(
        "vault_reverse_block",
        "引用它的工作事实" in vault
        and 'data-testid="vault-work-references"' in vault
        and "data-work-reference-group" in vault,
        "知识条目详情追加「引用它的工作事实」反向区块",
    )
    check(
        "vault_reverse_three_groups",
        'label: "工作记录"' in vault
        and 'label: "任务"' in vault
        and 'label: "素材"' in vault,
        "反向区块覆盖记录、任务、素材三类来源",
    )
    check(
        "vault_reverse_locating_info",
        "reference.source_title" in vault
        and "reference.source_id" in vault
        and "reference.reason" in vault
        and "reference.linked_at" in vault,
        "每条引用显示标题、编号、理由与关联时间",
    )
    check(
        "vault_reverse_empty_and_broken",
        "还没有记录、任务或素材引用这篇知识条目。" in vault
        and "data-testid=\"vault-work-references-empty\"" in vault
        and "另有 {broken} 条引用无法读取，未计入列表。" in vault,
        "无引用空态与损坏引用提示都真实可见",
    )
    check(
        "vault_reverse_clears_stale",
        vault.count("setWorkReferences(null)") >= 2,
        "切换知识条目时先清空上一条的引用，避免显示过期列表",
    )
    check(
        "api_types_reuse_507",
        "knowledgeLinkSuggest(" in api
        and "knowledgeLinkConfirm(" in api
        and "knowledgeLinkRemove(" in api
        and "knowledgeLinkByKnowledge(" in api
        and "export interface KnowledgeLinkCandidate" in types
        and "export interface KnowledgeLinkReferences" in types,
        "直接复用 507 的 api.ts / types.ts，不另造协议",
    )
    check(
        "no_new_exemptions",
        "@ts-ignore" not in panel
        and "@ts-expect-error" not in panel
        and "eslint-disable" not in panel,
        "新增界面代码未引入类型或 lint 豁免",
    )
    try:
        diff = subprocess.run(
            ["git", "diff", "--name-only", "HEAD", "--", "runtime"],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        runtime_clean = diff.returncode == 0 and not diff.stdout.strip()
        runtime_detail = "runtime/ 无改动" if runtime_clean else f"runtime/ 改动：{diff.stdout.strip()}"
    except Exception as error:  # pragma: no cover - 只用于给出可读证据
        runtime_clean = False
        runtime_detail = f"无法核对 runtime/：{error}"
    check("runtime_untouched", runtime_clean, runtime_detail)

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-KM-511",
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
