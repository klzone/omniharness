#!/usr/bin/env python3
"""OD-PRJ-602 静态验收：项目甘特分组、界面令牌化与数据一致性。

只读源码、跑分组纯函数单测，不启动应用、不写数据根。输出同时写入
tests/verification/desktop/evidence/od-prj-602/verify_output.txt。

证据等级：文件事实 + 静态检查 + 单测脚本运行。
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
DESKTOP = ROOT / "desktop"
SRC = DESKTOP / "src"
TASK_MANAGER = SRC / "shell" / "TaskManager.tsx"
PROJECT_REVIEW_CARD = SRC / "shell" / "ProjectReviewCard.tsx"
PROJECT_MAINTENANCE = SRC / "shell" / "ProjectMaintenance.tsx"
SCENE_PAGE = SRC / "shell" / "ScenePage.tsx"
THEME_CSS = SRC / "theme.css"
GROUPS_TS = SRC / "projectGanttGroups.ts"
GROUPS_TEST = SRC / "projectGanttGroups.test.mjs"
LIB_RS = DESKTOP / "src-tauri" / "src" / "lib.rs"
PROJECT_ITEMS_RS = DESKTOP / "src-tauri" / "src" / "project_items.rs"
TYPES_TS = SRC / "types.ts"
KNOWLEDGE_LINK_RS = DESKTOP / "src-tauri" / "src" / "knowledge_link_store.rs"
PROJECT_BOARD = SRC / "shell" / "ProjectBoard.tsx"
EVIDENCE = ROOT / "tests" / "verification" / "desktop" / "evidence" / "od-prj-602"

RESULTS: list[tuple[str, bool, str]] = []
HEX = re.compile(r"#[0-9a-fA-F]{3,8}")


def check(name: str, ok: bool, detail: str = "") -> None:
    RESULTS.append((name, ok, detail))


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def has(path: pathlib.Path, *needles: str) -> tuple[bool, str]:
    text = read(path)
    missing = [needle for needle in needles if needle not in text]
    return (not missing, "缺少：" + "、".join(missing) if missing else "")


def main() -> int:
    task_manager = read(TASK_MANAGER)
    review_card = read(PROJECT_REVIEW_CARD)
    maintenance = read(PROJECT_MAINTENANCE)
    scene_page = read(SCENE_PAGE)
    theme = read(THEME_CSS)
    groups = read(GROUPS_TS)
    lib = read(LIB_RS)

    # S1 纯函数：分组键是 task.project_id，未挂接用空 id，汇总带浅色无状态。
    check(
        "S1 分组纯函数与未挂接口径",
        GROUPS_TS.exists()
        and all(
            needle in groups
            for needle in (
                "export function buildProjectGanttGroups",
                "export function layoutTaskBar",
                "export function autoCollapseGroups",
                "export const UNATTACHED_PROJECT_ID = \"\"",
                "export const UNATTACHED_PROJECT_NAME = \"未挂接项目\"",
                "task.project_id",
                "AUTO_COLLAPSE_ROW_LIMIT = 30",
                "summary",
            )
        ),
    )

    check(
        "S1 单测覆盖重叠/接续/间隔/未挂接/空项目/clamp",
        GROUPS_TEST.exists()
        and all(
            needle in read(GROUPS_TEST)
            for needle in ("重叠", "接续", "间隔", "未挂接", "空项目", "clamp")
        ),
    )

    unit = subprocess.run(
        ["node", "--experimental-strip-types", "--test", "src/projectGanttGroups.test.mjs"],
        cwd=DESKTOP,
        text=True,
        capture_output=True,
    )
    check(
        "S1 分组单测实际运行通过",
        unit.returncode == 0,
        (unit.stdout.strip().splitlines() or ["无输出"])[-1] if unit.returncode == 0 else unit.stdout[-400:] + unit.stderr[-400:],
    )

    # S1 组件接入：分组数据来自 taskList + projectList，坐标逻辑只保留一处。
    check(
        "S1 甘特组件使用分组纯函数与项目列表",
        all(
            needle in task_manager
            for needle in (
                'from "../projectGanttGroups"',
                "buildProjectGanttGroups(tasks, projects",
                "shell.projectList()",
                "shell.taskList()",
                "data-project-gantt-group={group.projectId}",
                "data-project-gantt-group-header={group.projectId}",
                "data-task-gantt-row={task.id}",
                "aria-expanded={!collapsed}",
                # OD-PRJ-604：rangeLabel 取自局部 summary（项目计划区间），纯函数未变。
                "summaryRangeLabel(summary)",
                "已折叠 {collapsedCount} 组，展开查看",
            )
        ),
    )
    check(
        "S1 不再保留第二套条形坐标换算",
        "Math.max(start.getTime(), timeline.start.getTime())" not in task_manager
        and "rows.slice(0, 8)" not in task_manager
        and "layoutTaskBar" in groups,
    )
    check(
        "S1 today 竖线贯穿分组与空态文案",
        "todayPercent" in task_manager
        and "pointer-events-none absolute -top-2 bottom-0" in task_manager
        and "当前范围内没有带截止日期的任务" in task_manager,
    )

    # S2 状态条语义类与令牌化。
    state_map = re.search(r"const TASK_STATE_BAR: Record<WorkTaskState, string> = {(.*?)};", task_manager, re.S)
    class_names = re.findall(r'"od-task-bar--[a-z-]+"', state_map.group(1)) if state_map else []
    check(
        "S2 六个状态条走语义类",
        len(class_names) == 6
        and all(
            needle in state_map.group(1)
            for needle in (
                "od-task-bar--pending-confirm",
                "od-task-bar--not-started",
                "od-task-bar--in-progress",
                "od-task-bar--done",
                "od-task-bar--cancelled",
                "od-task-bar--paused",
            )
        ),
    )
    check(
        "S2 theme.css 六个语义类色值保留",
        all(
            needle in theme
            for needle in (
                ".od-task-bar--pending-confirm",
                ".od-task-bar--not-started",
                ".od-task-bar--in-progress",
                ".od-task-bar--done",
                ".od-task-bar--cancelled",
                ".od-task-bar--paused",
            )
        )
        and all(
            color in theme for color in ("#c0855f", "#8a7f6e", "#a24e2c", "#3f7d5a", "#b5a896", "#8a5a12")
        ),
    )
    leftovers = {
        "TaskManager.tsx": HEX.findall(task_manager),
        "ProjectReviewCard.tsx": HEX.findall(read(PROJECT_REVIEW_CARD)),
        "ProjectMaintenance.tsx": HEX.findall(read(PROJECT_MAINTENANCE)),
        "ScenePage.tsx": HEX.findall(scene_page),
    }
    check(
        "S2 页面去掉写死 hex 与靛蓝渐变",
        all(not colors for colors in leftovers.values())
        and all(
            "linear-gradient(145deg,#9481ff,#5747f8)" not in source
            for source in (task_manager, review_card, maintenance)
        )
        and "bg-[color:var(--od-neu-accent)]" in task_manager,
        "" if all(not colors for colors in leftovers.values()) else str(leftovers),
    )
    check(
        "S2 弹窗材质与响应式布局保留",
        'className="wr-dialog w-full max-w-[520px] p-6"' in task_manager
        and "pj-main" in task_manager
        and "@media (max-width: 1279px)" in theme,
    )

    # S3 ScenePage 死代码清除，宿主事项命令与文件保留。
    check(
        "S3 ScenePage 清除 projectItems 死代码",
        all(
            needle not in scene_page
            for needle in (
                "projectItemsRead",
                "projectItemsSave",
                "projectRecords",
                "saveProjectItems",
                "ProjectItemRecord",
            )
        ),
    )
    check(
        "S3 宿主 project-items 命令与文件保留",
        PROJECT_ITEMS_RS.exists()
        and "PROJECT_ITEMS_FILE" in read(PROJECT_ITEMS_RS)
        and all(needle in lib for needle in ("fn project_items_read(", "fn project_items_save(", "project_items_read,", "project_items_save,")),
    )
    check(
        "S3 ProjectBoard 历史文件保留且不挂载",
        PROJECT_BOARD.exists() and "ProjectBoard" not in scene_page,
    )

    # S3 甘特 / 维护 / 复盘同源：projectList + taskList + task.project_id。
    check(
        "S3 甘特与维护区同源 taskList/projectList",
        all(needle in maintenance for needle in ("shell.projectList()", "shell.taskList()", "shell.projectOverview"))
        and all(needle in task_manager for needle in ("shell.projectList()", "shell.taskList()", "task.project_id")),
    )
    check(
        "S3 复盘项目列表同源 projectList，无第二套存储",
        "shell.projectList()" in review_card
        and "projectItems" not in review_card
        and "projectItems" not in maintenance,
    )
    check(
        "S3 复盘知识卡 source 指向复盘 md 实路径",
        "shell.knowledgeReviewStage(review.body, [review.path])" in review_card,
    )
    check(
        "S3 knowledge 聚合字段与 knowledge_link 一致",
        "related_knowledge: KnowledgeLink[]" in read(TYPES_TS)
        and all(
            needle in lib
            for needle in (
                "for link in &task.related_knowledge",
                "for link in &record.related_knowledge",
                "knowledge_count: knowledge.len()",
                "link.reason.clone()",
            )
        )
        and "related_knowledge" in read(KNOWLEDGE_LINK_RS),
    )

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
