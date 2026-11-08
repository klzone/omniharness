# OD-WR-305 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：未修改 STATUS.md / REVIEW.md / PLANS.md。

## 改动文件

- 新增 desktop/src/shell/TaskManager.tsx：六状态筛选、列表、详情、来源跳转、进展与状态历史、状态变更。
- 复用 desktop/src/shell/WorkRecordPage.tsx 的「任务」视图挂载。
- 新增 tests/verification/desktop/verify_task_manage_ui_302.py：15 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| npm --prefix desktop run build | 0 | vite 构建成功（与 303 同一轮，已含本页） |
| python3 tests/verification/desktop/verify_task_manage_ui_302.py | 0 | 15/15 通过 |

原始输出：verify-task-manage.log、exit-verify.txt。

## 设计裁决

- 任务页与记录页同属「工作记录」场景的三个视图，不新增 Dock 入口，避免与声明里的工作记录重复。
- 状态变更用下拉列出全部六状态、交给宿主白名单裁决；非法迁移的拒绝原文显示在页面顶部，满足「报错可见」。
- 不提供新建任务表单：正式任务来自候选确认（OD-WR-306），手工造任务会绕开来源与去重校验。

## 已知限制

- 未在真实 Tauri 窗口目视核对。
- 候选审核与来源采纳属 OD-WR-306/307。
