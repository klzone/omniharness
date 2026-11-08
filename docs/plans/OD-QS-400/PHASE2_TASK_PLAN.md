# 第二阶段实施计划（PHASE2_TASK_PLAN）

> 任务卡全文以 PLANS.md OD-QS-4xx 小节为权威版本；本文件是同一内容的规划快照，供执行会话只读参考。

## 执行顺序与依赖

```
401 → 402 → 403 → 405 → {410, 412}
 ├── 404（与 402/403 并行）
 ├── 407（与 402–406 并行）
406 → 408 → 409 → {411, 412} → 413
```

- 串行主线：401 → 402 → 403 → 408 → 409 → 413。
- 并行线：402/404/407 三路并行（只阻塞于 401）；406 可与 401 同日开工；410 与 411 可并行。

## 任务卡索引（详细字段/步骤/验收命令见 PLANS.md）

| ID | 交付物 | 允许文件 | 验收命令 | 依赖 |
|---|---|---|---|---|
| OD-QS-401 | Task 字段扩展（importance/due_kind/depends_on/estimated_minutes/parent_id/quadrant_override） | task_store.rs | cargo lib；cargo --tests | 无 |
| OD-QS-402 | 紧急性规则引擎 + 封闭模式时间推定 | priority_rules.rs（新）；verify_priority_rules_401.py（新） | cargo lib；verify | 401 |
| OD-QS-403 | 四象限分类引擎 + QuadrantView | priority_rules.rs | cargo lib | 402 |
| OD-QS-404 | AI 重要性建议（零新增调用） | task_extract.py；task_extract.rs；adapter 测试 | pytest；cargo lib | 401 |
| OD-QS-405 | 优先级 IPC 五命令 | lib.rs；task_store.rs；api.ts；types.ts | cargo lib；tsc | 401–403 |
| OD-QS-406 | 工作时间配置 | availability.rs（新）；lib.rs；SettingsPanel.tsx；api/types | cargo lib；tsc | 无 |
| OD-QS-407 | 排期实体 | schedule_store.rs（新） | cargo lib | 401 |
| OD-QS-408 | 排期引擎 | scheduling.rs（新） | cargo lib | 403/406/407 |
| OD-QS-409 | 排期 IPC + 确认流 | lib.rs；schedule_store.rs；api/types | cargo lib；tsc | 408 |
| OD-QS-410 | 任务四象限页 + 旧占位替换 | TaskManager 组件；styles；verify_task_quadrant_ui_402.py（新）；删除 weeklyQuadrants.ts / WeeklyQuadrants.tsx | npm build；verify | 405 |
| OD-QS-411 | 今日/本周计划页 | 同 410 范围；verify_schedule_ui_403.py（新） | npm build；verify | 409 |
| OD-QS-412 | 任务详情扩展 | TaskManager 详情区 | npm build；tsc | 405/409 |
| OD-QS-413 | 全量回归+安全核对+收口 | tests/verification/ 新增 | cargo --tests；pytest；build；tsc；verify 全绿 | 全部 |

## 纪律提醒（执行会话）

- 按 IMPLEMENTED_RULES §5：入口=获批任务卡+本目录文档+点名文件；裁决留痕；四类停下条件；报告固定模板。
- runtime/ 零改动；不提交不推送；敏感词红线；测试全虚构。
- 410 涉及界面：动工前先搜 2–3 个同类产品布局（借鉴要点 ≤2 句写入报告）。

