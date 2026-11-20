#!/usr/bin/env python3
"""0.2.45 首页/工作记录微调：统计条隐藏、当日摘要下线、引擎标签、通知中心、复盘确认。"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DESKTOP = ROOT / "desktop"
SRC = DESKTOP / "src"
TAURI = DESKTOP / "src-tauri" / "src"

FAILS = []
CASES = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append(name)
    print(("PASS " if ok else "FAIL ") + name + ((" :: " + detail) if not ok else ""))
    if not ok:
        FAILS.append(name)


def main() -> int:
    home = (SRC / "shell" / "HomeCanvas.tsx").read_text(encoding="utf-8")
    app = (SRC / "App.tsx").read_text(encoding="utf-8")
    engine_labels = (SRC / "engineLabels.ts").read_text(encoding="utf-8")
    tech = (SRC / "shell" / "EngineTechSlice.tsx").read_text(encoding="utf-8")
    topnav = (SRC / "shell" / "TopNav.tsx").read_text(encoding="utf-8")
    notices = (SRC / "shell" / "NotificationCenter.tsx").read_text(encoding="utf-8")
    archive = (SRC / "shell" / "ReviewArchive.tsx").read_text(encoding="utf-8")
    library = (SRC / "shell" / "WorkRecordLibrary.tsx").read_text(encoding="utf-8")
    page = (SRC / "shell" / "WorkRecordPage.tsx").read_text(encoding="utf-8")
    intent = (SRC / "workIntent.ts").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    lib_rs = (TAURI / "lib.rs").read_text(encoding="utf-8")

    # 1. 首页阅读统计条只在空白对话显示。
    check("有对话时隐藏阅读统计", "conversationStarted ? null : (" in home)
    check("统计条仍保留空态入口", 'data-testid="home-reading-stats-slot"' in home and 'data-testid="home-reading-stats"' in home)

    # 2. 当日摘要彻底下线。
    check("前端无当日摘要代码", "daily-summary" not in intent and "DailySummary" not in api and "每日摘要" not in home)
    check("工作记录页无摘要入口", "data-daily-summary" not in library and "生成当日摘要" not in library)
    check("当日摘要文件已删除", not (SRC / "dailySummary.ts").exists() and not (TAURI / "daily_summary.rs").exists())
    check("宿主无摘要命令与模块", "daily_summary" not in lib_rs and "worklog_daily_summary" not in lib_rs)
    check("旧口令按随手记", "生成今日摘要" not in intent and "daily-summary" not in intent)
    check("工作记录页文案已更新", "工作记录（时间线 + 详情）" in page and "当日摘要" not in page)
    check("内置 Skill 清单已移除", "daily-summary" not in (TAURI / "profile_rules.rs").read_text(encoding="utf-8") and "daily-summary" not in (TAURI / "profile_seed.rs").read_text(encoding="utf-8") and "daily-summary" not in (TAURI / "profile_assets.rs").read_text(encoding="utf-8"))
    check("样本规则已解绑", "daily-summary" not in (ROOT / "samples" / "profiles" / "discipline.rules.json").read_text(encoding="utf-8"))
    check("摘要 Skill 文件已删除", not (ROOT / "skills" / "daily-summary" / "SKILL.md").exists())

    # 3. 引擎状态中文标签。
    check("引擎型号中文化", "function engineKindLabel" in engine_labels and 'return "DSH Web API"' in engine_labels)
    check("兼容性中文化", "function engineCompatibilityLabel" in engine_labels and '"协议兼容"' in engine_labels)
    check("未暴露版本不显示", 'version !== "unknown"' in engine_labels and "parts.push(version)" in engine_labels and "版本未知" not in engine_labels)
    check("顶栏使用中文标签", "engineSummaryLabel(descriptor)" in app)
    check("技术切片使用中文标签", "engineSummaryLabel(descriptor)" in tech)

    # 4. 通知与待办。
    check("通知组件存在", (SRC / "shell" / "NotificationCenter.tsx").is_file())
    check("顶栏接入通知组件", 'NotificationCenter' in topnav and "<NotificationCenter onNavigate={onNavigate} />" in topnav)
    check("移除未实现占位", "未实现：通知与待办" not in topnav)
    check("App 传入导航", "onNavigate={selectScene}" in app)
    check("聚合待确认写入", "pendingWrites.list()" in notices and 'filter((item) => item.status === "pending")' in notices)
    check("聚合到期任务", "shell.taskList({})" in notices and "今天到期" in notices and "已逾期" in notices)
    check("未完成任务总览", "你的任务 ·" in notices and 'id: "task-overview"' in notices)
    check("聚合到点复盘", "scheduler.tick()" in notices and "tick.review_due" in notices)
    check("通知徽标与空态", 'data-testid="topnav-notification-count"' in notices and "目前没有需要处理的事项" in notices)
    check("通知可跳转场景", "onNavigate(item.scene)" in notices and "scene: \"project\"" in notices)

    # 5. 周复盘草稿确认与重复生成。
    check("复盘确认按钮", 'data-review-confirm=""' in archive and "确认归档" in archive)
    check("确认调用宿主命令", "shell.worklogPeriodReviewConfirm(" in archive)
    check("草稿提示归档", 'data-review-draft-hint=""' in archive and "确认归档后会锁定" in archive)
    check("已确认禁止覆盖", "已确认复盘不可覆盖" in archive and "detail.status !== \"草稿\"" in archive)
    check("重新生成草稿", "data-review-regenerate=""" in archive and "worklogPeriodReviewGenerate(" in archive)

    print(f"cases={len(CASES)} failed={len(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    raise SystemExit(main())
