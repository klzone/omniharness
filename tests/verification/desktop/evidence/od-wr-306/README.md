# OD-WR-306 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：未修改 STATUS.md / REVIEW.md / PLANS.md。本轮只改前端，Rust 零改动。

## 布局参照

审核台按 KnowledgeReviewCenter 的两栏结构：左侧候选队列、右侧详情与动作，详情用浅色卡面和原文证据块；候选详情展示置信度、不确定原因与原文连续片段高亮。

## 改动文件

- 新增 desktop/src/shell/TaskReviewPanel.tsx：待审核列表、置信度/不确定原因、原文证据高亮、字段就地编辑、采纳/拒绝、来源跳转、批量采纳。
- 修改 desktop/src/shell/WorkRecordPage.tsx：新增「审核」视图，解析完成后自动切过去。
- 修改 desktop/src/shell/WorkRecordLibrary.tsx：解析按钮接上 304 的 prepare / submit / poll / apply / failed 链路。
- 修改 desktop/src/api.ts、desktop/src/types.ts：analyze 三段 IPC 的客户端方法与类型。
- 新增 tests/verification/desktop/verify_task_review_ui_303.py：16 项静态检查。
- 更新 tests/verification/desktop/verify_work_record_ui_301.py：解析入口检查从「不可用」改为「已接线」，重新跑 15/15。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| npm --prefix desktop run build | 0 | vite 构建成功（含 tsc，2.61s） |
| python3 tests/verification/desktop/verify_task_review_ui_303.py | 0 | 16/16 通过 |
| python3 tests/verification/desktop/verify_work_record_ui_301.py | 0 | 15/15 通过（更新后复跑） |

原始输出：npm-build.log、verify-task-review.log、exit-*.txt。

## 裁决

- 批量采纳逐条调用同一个 taskCandidateConfirm IPC，不新增批量旁路；每条都走宿主校验与去重。
- 解析链路沿用知识库的 prepare → 提交 → 轮询 → 落地模式；准入拒绝或红线拒绝不置 failed，任务终态失败或空结果才置 failed 并允许重试。
- 「关联采纳与拒绝」在本任务落地为候选采纳/拒绝与来源记录跳转；匹配建议与双向 ID 回写属 OD-WR-307。
- 候选详情不预填模型 note 作为确认备注；模型 note 只读展示，确认备注由用户填写。

## 已知限制

- 真实模型输出与真实窗口未验证；界面观感仍需人工验收。
- 307 的匹配建议、状态变更建议与双向链接尚未实现。
- 候选列表为了证据高亮会一次性读取全部记录正文；记录量大时需要改按需读取。
