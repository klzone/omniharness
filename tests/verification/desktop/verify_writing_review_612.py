#!/usr/bin/env python3
"""OD-WT-612 核验面板 + 版本管理界面：静态源码检查。

证据等级：静态源码检查。不开窗口、不调用模型、不读用户数据。

覆盖：两个新组件的导出接口、609 核验 IPC 复用、八类结论筛选与统计条、
单条定位与三种处理动作、口径冲突显式并列不合并、608 版本列表/对比/恢复、
恢复确认弹窗、定稿按钮门禁与缺失状态迁移通道的诚实标记、三类空态、
四组件已挂载任务详情、runtime/ 零改动、无新增豁免、无 AI 定稿写通道。

用法：python3 tests/verification/desktop/verify_writing_review_612.py [--json PATH]
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


def git_status(path: str) -> str:
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--", path],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        return result.stdout.strip()
    except OSError:
        return "git-unavailable"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()

    fact_path = SRC / "shell" / "WritingFactCheckPanel.tsx"
    version_path = SRC / "shell" / "WritingVersionPanel.tsx"
    studio_path = SRC / "shell" / "WritingStudio.tsx"
    workbench_path = SRC / "shell" / "WritingWorkbench.tsx"
    fact = fact_path.read_text(encoding="utf-8")
    version = version_path.read_text(encoding="utf-8")
    studio = studio_path.read_text(encoding="utf-8")
    workbench = workbench_path.read_text(encoding="utf-8")
    lib = (REPO / "desktop" / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")

    check(
        "components_exist",
        fact_path.exists()
        and version_path.exists()
        and "export function WritingFactCheckPanel" in fact
        and "export function WritingVersionPanel" in version,
        "两个组件文件存在并导出命名函数",
    )
    check(
        "props_exports",
        "export type WritingFactCheckPanelProps" in fact
        and "export type WritingVersionPanelProps" in version,
        "组件对外导出 props 类型，供 610 挂载时引用",
    )
    check(
        "factcheck_api_reuse",
        "shell.writingFactcheckRun" in fact
        and "shell.writingFactcheckResolve" in fact
        and "invoke(" not in fact
        and "@tauri-apps/api" not in fact,
        "只经 api 客户端调用 609 两条命令",
    )
    conclusions = [
        "有直接来源支持",
        "有间接来源支持",
        "来源存在冲突",
        "来源已过期",
        "无法找到来源",
        "属于分析判断",
        "属于工作计划",
        "属于表达性概括",
    ]
    conclusion_block = fact.split("export const FACTCHECK_CONCLUSIONS", 1)[1].split("];", 1)[0]
    positions = [conclusion_block.find(label) for label in conclusions]
    check(
        "eight_conclusion_tabs",
        all(position >= 0 for position in positions)
        and positions == sorted(positions)
        and '(["全部", ...FACTCHECK_CONCLUSIONS] as Filter[])' in fact
        and 'role="tablist"' in fact
        and "data-factcheck-tab" in fact,
        "八类结论筛选页签顺序与 609 白名单一致，另设全部",
    )
    check(
        "stats_strip",
        "data-factcheck-stats" in fact
        and all(key in fact for key in [
            "direct_source",
            "indirect_source",
            "source_conflict",
            "source_outdated",
            "source_missing",
            "analysis_judgment",
            "work_plan",
            "expressive_summary",
        ])
        and "stats.candidates" in fact
        and "stats.findings" in fact
        and "stats.scope_conflicts" in fact,
        "统计条含候选/结论/冲突对/标注与八类结论计数",
    )
    check(
        "run_and_busy_state",
        'data-testid="factcheck-run"' in fact
        and 'busy === "run" ? "核验中…" : "运行核验"' in fact
        and "disabled={!writingId || Boolean(busy)}" in fact,
        "运行核验按钮有忙碌态且空任务禁用",
    )
    check(
        "no_auto_run_on_mount",
        "writingFactcheckRun"
        not in fact.split("useEffect(() => {", 1)[1].split("}, [writingId, initialResult]);", 1)[0],
        "挂载只回填 initialResult，不自动写核验记录",
    )
    check(
        "fact_text_and_location",
        "record.fact_text?.trim()" in fact
        and "这一条没有可展示的原文片段" in fact
        and "第 {record.section_no} 章 · 第 {record.paragraph_no} 段" in fact
        and "data-factcheck-locate" in fact
        and "onLocate?.(record.section_no, record.paragraph_no)" in fact,
        "单条展示 fact_text 与章节/段落定位，点击交给编辑器跳转",
    )
    check(
        "three_resolutions",
        all(label in fact for label in ["已接受", "已修改", "已删除来源项"])
        and "data-factcheck-resolve" in fact
        and "record_id: record.record_id" in fact
        and "resolution," in fact,
        "三种用户处理动作均走 writing_factcheck_resolve",
    )
    check(
        "resolution_visible",
        "data-factcheck-resolution-label" in fact
        and "record.resolution === \"未处理\"" in fact
        and "pending" in fact,
        "记录显示处理状态并统计待处理数",
    )
    check(
        "conflicts_explicit_no_merge",
        "data-factcheck-conflicts" in fact
        and "data-factcheck-conflict-dot" in fact
        and "bg-error" in fact
        and "两侧并存，不合并口径" in fact
        and "data-factcheck-conflict-side" in fact
        and "onLocate?.(side.section_no, side.paragraph_no)" in fact,
        "口径冲突逐对并列、红点标注、两侧可定位，明确不合并",
    )
    check(
        "empty_states_factcheck",
        'data-factcheck-empty="no-records"' in fact
        and "还没有核验记录" in fact
        and 'data-factcheck-empty="no-filter-match"' in fact
        and 'data-factcheck-empty="all-processed"' in fact
        and "核验结论已全部处理" in fact,
        "无记录/当前筛选无记录/全部已处理三种空态各自成句",
    )
    check(
        "factcheck_no_ai_write",
        all(word not in fact for word in [
            "writingReview",
            "writingSectionConfirm",
            "writingSectionGenerate",
            "engine",
            "prompt",
        ]),
        "核验面板不碰审阅、生成或模型通道",
    )
    check(
        "version_api_reuse",
        "writingVersionList" in version
        and "shell.writingVersionDiff" in version
        and "shell.writingVersionRestore" in version
        and "invoke(" not in version
        and "@tauri-apps/api" not in version,
        "版本列表/对比/恢复均复用 608 三个命令",
    )
    check(
        "version_list_fields",
        "v{version.version}" in version
        and "version.note.trim()" in version
        and "shortTime(version.at)" in version
        and "SOURCE_LABEL[version.source]" in version
        and "SCOPE_LABEL[version.scope]" in version
        and "version.current" in version,
        "版本条目显示编号、说明、时间、来源、快照类型与当前标记",
    )
    check(
        "two_version_select_and_diff",
        "data-version-from" in version
        and "data-version-to" in version
        and "from_version: fromVersion" in version
        and "to_version: toVersion" in version
        and "writingVersionDiff" in version,
        "两版选择后走 writing_version_diff",
    )
    check(
        "paragraph_diff_display",
        "data-version-diff" in version
        and "data-version-change" in version
        and "change.before" in version
        and "change.after" in version
        and all(kind in version for kind in ["新增", "删除", "修改"]),
        "段落级变更按新增/删除/修改展示原文与新文",
    )
    check(
        "diff_empty_state",
        'data-version-diff-empty' in version and "这两版没有段落级差异" in version,
        "对比无变化时给真实空态",
    )
    restore_block = version.split("const restore = async", 1)[1].split("const finalize", 1)[0]
    check(
        "restore_confirm_gate",
        version.count("confirmed: true") == 1
        and "confirmed: true" in restore_block
        and "setPendingRestore(version.version)" in version
        and "testId=\"writing-version-restore-confirm\"" in version
        and "onConfirm={() => void restore()}" in version,
        "列表按钮只打开确认弹窗；confirmed=true 仅出现在用户确认后的恢复调用",
    )
    check(
        "version_empty_state",
        'data-version-empty="no-versions"' in version and "还没有版本快照" in version,
        "无版本时给真实空态",
    )
    check(
        "finalize_status_gate",
        'const finalizeReady = status === "待审核"' in version
        and "disabled={!finalizeReady || !finalizeChannelReady || finalizing}" in version
        and "data-finalize-status={status" in version,
        "只有待审核状态允许点定稿",
    )
    check(
        "finalize_missing_channel_marked",
        "data-finalize-missing-channel" in version
        and "宿主尚未提供写作任务状态迁移命令" in version
        and "onFinalize?: () => void | Promise<void>" in version,
        "宿主无状态迁移 IPC 时不伪造成功，交由 610/613 接入 onFinalize",
    )
    finalize_block = version.split("const finalize = async", 1)[1].split("const finalizeReady", 1)[0]
    check(
        "no_ai_finalize_channel",
        all(word not in version for word in [
            "writingReview",
            "writingSectionConfirm",
            "writingFactcheckRun",
            "engine",
            "prompt",
        ])
        and "shell." not in finalize_block,
        "定稿只调用外部传入的用户动作回调，不走 AI/审阅/生成通道",
    )
    check(
        "no_new_status_ipc",
        "writing_finalize" not in lib
        and "writing_task_transition" not in lib
        and "fn writing_task_update" not in lib,
        "未擅自新增写作任务状态 IPC（登记为 613/614 补口）",
    )
    mounted_imports = [
        'import { WritingOutlineEditor } from "./WritingOutlineEditor";',
        'import { WritingSectionEditor } from "./WritingSectionEditor";',
        'import { WritingFactCheckPanel } from "./WritingFactCheckPanel";',
        'import { WritingVersionPanel } from "./WritingVersionPanel";',
    ]
    check(
        "mounted_in_task_detail",
        "<WritingWorkbench />" in studio
        and workbench_path.exists()
        and all(item in workbench for item in mounted_imports)
        and all(
            tag in workbench
            for tag in [
                "<WritingOutlineEditor writingId={task.writing_id}",
                "<WritingSectionEditor writingId={task.writing_id}",
                "<WritingFactCheckPanel writingId={task.writing_id}",
                "<WritingVersionPanel",
            ]
        )
        and "data-writing-detail-tab={item.key}" in workbench
        and 'key: "outline", label: "大纲"' in workbench
        and 'key: "section", label: "正文"' in workbench
        and 'key: "factcheck", label: "核验"' in workbench
        and 'key: "versions", label: "版本"' in workbench
        and "shell.writingStatusTransition" in workbench
        and "onFinalize={finalizeTask}" in workbench,
        "WritingStudio 经 WritingWorkbench 挂载四个页签：大纲/正文/核验/版本，定稿走状态迁移 IPC",
    )
    check(
        "runtime_untouched",
        git_status("runtime") == "",
        "runtime/ 零改动",
    )
    check(
        "no_new_exemptions",
        all(
            marker not in fact + version
            for marker in ["@ts-ignore", "@ts-expect-error", "eslint-disable", "ts-nocheck", "#[allow("]
        ),
        "两个组件未新增 lint/类型豁免",
    )
    check(
        "no_fake_test_data",
        all(token not in fact + version for token in ["13800138000", "110101", "张三", "李四"]),
        "组件源码不含真实或虚构的个人数据夹具",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    payload = {
        "plan": "OD-WT-612",
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

