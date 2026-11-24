#!/usr/bin/env python3
"""OD-PRJ-601 静态验收：任务完成确认、项目复盘、项目维护区与统计卡口径。

只读源码与新增测试，不启动应用、不写数据根。输出同时写入
tests/verification/desktop/evidence/od-prj-601/verify_output.txt。
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
DESKTOP = ROOT / "desktop"
TASK_MANAGER = DESKTOP / "src" / "shell" / "TaskManager.tsx"
PROJECT_REVIEW_CARD = DESKTOP / "src" / "shell" / "ProjectReviewCard.tsx"
PROJECT_MAINTENANCE = DESKTOP / "src" / "shell" / "ProjectMaintenance.tsx"
PROJECT_REVIEW_STORE = DESKTOP / "src-tauri" / "src" / "project_review_store.rs"
LIB_RS = DESKTOP / "src-tauri" / "src" / "lib.rs"
API_TS = DESKTOP / "src" / "api.ts"
TYPES_TS = DESKTOP / "src" / "types.ts"
EVIDENCE = ROOT / "tests" / "verification" / "desktop" / "evidence" / "od-prj-601"

RESULTS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    RESULTS.append((name, ok, detail))


def contains(path: pathlib.Path, *needles: str) -> tuple[bool, str]:
    text = path.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    return (not missing, "缺少：" + "、".join(missing) if missing else "")


def main() -> int:
    task_manager = TASK_MANAGER.read_text(encoding="utf-8")
    review_card = PROJECT_REVIEW_CARD.read_text(encoding="utf-8")
    maintenance = PROJECT_MAINTENANCE.read_text(encoding="utf-8")
    store = PROJECT_REVIEW_STORE.read_text(encoding="utf-8")
    lib = LIB_RS.read_text(encoding="utf-8")

    # S1：四象限过滤终态任务。
    check(
        "S1 四象限过滤已完成/已取消",
        all(
            needle in task_manager
            for needle in (
                'const archived = new Set<WorkTaskState>(["已完成", "已取消"]);',
                "return !task || !archived.has(task.state);",
            )
        ),
    )

    # S2：完成入口 + 二次确认 + 两步迁移。
    check(
        "S2 标记完成入口与二次确认",
        all(
            needle in task_manager
            for needle in (
                'data-task-complete-entry=""',
                'title="标记任务完成？"',
                "完成后将从四象限归档",
                'testId="task-complete-confirm"',
                'shell.taskTransition(editing, "已完成")',
            )
        ),
    )
    check(
        "S2 待确认任务先转待开始",
        all(
            needle in task_manager
            for needle in (
                # OD-PRJ-604：preState 三元改为多行（待开始/暂停 → 进行中）。
                'task.state === "待确认"',
                '? "待开始"',
                '? "进行中"',
                'shell.taskTransition(editing, preState)',
                '已转为" + preState + "，但',
            )
        ),
        "暂停任务按宿主状态机先转进行中，属计划偏差，STATUS.md 已记录",
    )

    # S3：宿主项目复盘存档。
    check(
        "S3 复盘 schema、路径与固定标题",
        all(
            needle in store
            for needle in (
                'omnidesk.project-review.v1',
                'pub const REVIEW_DIR: &str = "复盘";',
                'pub const PROJECT_REVIEW_DIR: &str = "项目";',
                '"项目概况"',
                '"任务完成情况"',
                '"问题与不足"',
                '"经验与做法"',
                '"后续建议"',
            )
        ),
    )
    check(
        "S3 复盘校验、备份与状态机",
        all(
            needle in store
            for needle in (
                "is_safe_id",
                "MAX_BODY_BYTES: usize = 1024 * 1024",
                "task_citations",
                "write_replace_atomically",
                "write_new_atomically",
                ".bak",
                "STATUS_DRAFT",
                "STATUS_CONFIRMED",
                "confirm_project_review",
            )
        ),
    )
    check(
        "S3 宿主命令注册",
        all(
            needle in lib
            for needle in (
                "fn project_review_read(",
                "fn project_review_save(",
                "fn project_review_confirm(",
                "project_review_read,",
                "project_review_save,",
                "project_review_confirm,",
                "pub mod project_review_store;",
            )
        ),
    )
    check(
        "S3 前端复盘面板接线",
        all(
            needle in review_card
            for needle in (
                '"项目概况"',
                '"任务完成情况"',
                '"问题与不足"',
                '"经验与做法"',
                '"后续建议"',
                "shell.projectOverview",
                "shell.taskRead",
                "shell.prepareWorkspaceDir",
                'intent: "project_management"',
                "host.submitTask",
                "host.taskStatus",
                "host.taskResult",
                "shell.projectReviewSave",
                "shell.projectReviewConfirm",
                "shell.knowledgeReviewStage",
                "（未挂接项目）",
                'data-project-review-card=""',
            )
        ),
    )
    check(
        "S3 已完成列表复盘入口与右列挂载",
        all(
            needle in task_manager
            for needle in (
                'data-task-review-entry=""',
                # OD-PRJ-604：挂载增加 onChanged 即时刷新。
                "<ProjectReviewCard",
                "<ProjectMaintenance",
                'onChanged={() => setTick((n) => n + 1)}',
            )
        ),
    )

    # S4：项目维护区。
    check(
        "S4 项目维护 CRUD 接线",
        all(
            needle in maintenance
            for needle in (
                "shell.projectCreate",
                "shell.projectList",
                "shell.projectOverview",
                "shell.taskList",
                "shell.taskCreate",
                "shell.taskUpdate",
                "shell.taskDelete",
                "project_id: selected",
                "project_id: pending.projectId",
                "ConfirmDialog",
                "可在数据根垃圾桶找回",
            )
        ),
    )
    check(
        "S4 每页 8 条与分页控件",
        all(
            needle in maintenance
            for needle in (
                "const PAGE_SIZE = 8;",
                "projectRows.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE)",
                'data-project-page-prev=""',
                'data-project-page-next=""',
            )
        ),
    )
    check(
        "S4 新建项目字段与状态徽标",
        all(
            needle in maintenance
            for needle in (
                'data-project-dialog="new"',
                'data-project-name=""',
                "开始日期",
                "结束日期",
                "ProjectStatusChip",
                "进行中",
                "已完成",
                "已搁置",
            )
        ),
    )
    check(
        "S4 添加任务支持挂接或新建",
        all(
            needle in maintenance
            for needle in (
                'data-project-dialog="add-task"',
                "挂接现有任务",
                "新建任务",
                'data-project-existing-task=""',
                'data-project-task-title=""',
            )
        ),
    )

    # S5（OD-UI-900 修订）：统计改为状态胶囊条，数字口径仍来自 stats.*。
    ok = (
        "stats.done" in task_manager
        and "stats.running" in task_manager
        and "stats.overdue" in task_manager
        and 'data-pj-chip-done=""' in task_manager
        and 'data-pj-chip-running=""' in task_manager
        and 'data-pj-chip-overdue=""' in task_manager
    )
    check("S5 状态胶囊条数字口径未改（OD-UI-900）", ok)

    # IPC 封装与类型。
    check(
        "IPC 封装与类型",
        all(
            needle in API_TS.read_text(encoding="utf-8")
            for needle in (
                '"project_review_read"',
                '"project_review_save"',
                '"project_review_confirm"',
                "projectReviewRead",
                "projectReviewSave",
                "projectReviewConfirm",
            )
        )
        and all(
            needle in TYPES_TS.read_text(encoding="utf-8")
            for needle in ("interface ProjectReviewRead", "interface ProjectReviewReceipt")
        ),
    )

    # 硬约束：runtime 未被改动。
    runtime = subprocess.run(
        ["git", "status", "--porcelain", "runtime/"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    check("硬约束 runtime/ 无改动", runtime.stdout.strip() == "", runtime.stdout.strip())

    lines = []
    failed = 0
    for name, ok, detail in RESULTS:
        status = "PASS" if ok else "FAIL"
        if not ok:
            failed += 1
        line = f"[{status}] {name}" + (f" — {detail}" if detail else "")
        lines.append(line)
        print(line)
    summary = f"\n{len(RESULTS) - failed}/{len(RESULTS)} passed; {failed} failed"
    lines.append(summary)
    print(summary)

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / "verify_output.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
