# OD-WT-610 写作工作台页：执行证据

日期：2026-11-09
证据等级：静态源码检查 + 前端构建 + Rust 编译检查。未开真实窗口、未调用真实模型、未读用户业务数据。

## UI 参考（screenshots-design-inspiration MCP）

检索关键词：writing task dashboard / manuscript manager、writing workspace document list editor、task list grouped status filter。
命中产品：Flow（任务追踪/看板信息密度）、Dovetail（侧栏导航 + 文档工作区层级）、Asana（分组任务列表 + 标签筛选）。

借鉴要点（2 句）：用状态分组和紧凑类型 chip 让任务推进阶段一眼可读；详情留在同一卡片内切换，回到列表不丢筛选。

## 改动文件

- desktop/src/shell/WritingWorkbench.tsx（新增）
- desktop/src/shell/WritingStudio.tsx（页签组新增「任务」）
- desktop/src/api.ts（新增 writingList 封装）
- desktop/src-tauri/src/lib.rs（新增只读 writing_list 命令并注册）
- tests/verification/desktop/verify_writing_ui_610.py（新增）
- tests/verification/desktop/verify_writing_ui_610.json（本轮静态检查证据）

## 裁决

Ruling：任务卡①要求写作任务列表，但 604-609 没有把 601 的 list_writing_tasks 暴露给界面；lib.rs/api.ts 的只读约束与该交付项冲突。决定仅追加只读 writing_list 命令与前端封装，不改 604-609 既有封装。错了的代价：与只读约束冲突，审查需单独确认；若 611/612 同时添加同名命令会产生合并冲突。

## 实跑命令

- cd desktop && npm --prefix . run build -> BUILD_EXIT=0（仅既有 >500kB chunk 提示）
- cd desktop && npx tsc --noEmit -> TSC_EXIT=0
- python3 tests/verification/desktop/verify_writing_ui_610.py -> 26/26 通过，VERIFY_EXIT=0
- cd desktop/src-tauri && cargo check --offline --lib -> CARGO_EXIT=0
- cargo test --offline --lib writing_store -> 20 passed / 0 failed

## 未验证

- 真实 Tauri 窗口目视与 1280×820 截图。
- 真实模型解析与创建任务的端到端输出（模型通道本轮未调用）。
- 611 大纲/正文编辑器挂载（本轮按任务卡只留只读简表入口）。

