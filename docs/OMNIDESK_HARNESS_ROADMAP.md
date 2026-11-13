# OmniDesk Harness + Multi-Agent 最小化改造路线（Phase A–F）

> 状态：OD-ARCH-150 规划成果（路线与任务卡，未实施；每阶段须用户批准后执行）。
> 实施进度（2026-11-13）：Phase A/B/C 已实施；Phase D（OD-HAR-221/222 事件反馈与引用体检）与 Phase E（OD-HAR-231/232 12 例合成基准与指标测量）已实施。Work Review preset 最终为「可写文件、无 shell」（D-097）；删除文件仍由宿主回收站负责。真实模型评测未做（需授权）。
> 关联：[现状审计](OMNIDESK_CURRENT_ARCHITECTURE_AUDIT.md) · [存储审计](OMNIDESK_STORAGE_ARCHITECTURE_AUDIT.md) · [Harness 设计](OMNIDESK_HARNESS_ARCHITECTURE.md) · [Graph 设计](OMNIDESK_MULTI_AGENT_GRAPH_DESIGN.md) · [试点设计](WORK_REVIEW_AGENT_DESIGN.md)

## 阶段总览

| Phase | 内容 | 依赖 | 规模预估 |
| --- | --- | --- | --- |
| A | 现状审计与基础规范（本文档组即 Phase A 产出） | 无 | 本轮已完成 |
| B | 共享 Harness 基础（verdict 协议/事件表/Agent 声明/预算字段） | A 批准 | 4 张任务卡 |
| C | Work Review Agent 试点 | B1-B3 | 3 张任务卡 |
| D | 事件与交接机制（跨 Agent） | C 验收 | 2 张任务卡 |
| E | Golden Task 评估与改进 | C/D | 2 张任务卡 |
| F | 后续三 Agent 扩展规划 | C 试点通过+用户批准 | 另行规划 |

原则：每张任务卡单一交付目标+确切验收命令+实跑证据落盘；runtime/、生产数据、凭证、模型路由始终不动；不建通用平台。

## Phase B：共享 Harness 基础（任务卡）

### OD-HAR-201：verdict.v1 验收协议 + 任务候选验收器接入

- **允许修改：** engine/omnidesk_engine/contract.py（或新增 verdict.py）、desktop/src-tauri/src/task_extract.rs（verdict 暂存字段）、types.ts、TaskReviewPanel.tsx（展示检查明细）、对应验证脚本与证据目录。
- **单一动作：** 定义 omnidesk.verdict.v1 结构；任务候选校验流程产出 checks 明细（quote_in_source/date_shape/dedup/confidence）随候选 JSON 暂存并在审核台展示。
- **验收命令：** `cd desktop/src-tauri && cargo test --offline --lib task_extract`（新增 verdict 用例全绿）；`cd desktop && npx tsc --noEmit` 0；新增 verify_harness_verdict_201.py 全过。
- **依赖：** 无。

### OD-HAR-202：events.jsonl 轻量事件表（宿主追加，无消费方）

- **允许修改：** 新增 desktop/src-tauri/src/event_log.rs、lib.rs 注册 event_log_append（内部调用）、在任务候选/确认/工作记录创建等 3-5 个现有动作点追加事件；验证脚本。
- **单一动作：** 定义事件行 schema（seq/ts/type/actor/entity/ref/status），在现有宿主动作点确定性追加；提供只读查询命令。
- **验收命令：** `cargo test --offline --lib event_log` 全绿（含原子追加+损坏行跳过）；新增 verify_event_log_202.py 全过。
- **依赖：** 无（与 201 并行）。

### OD-HAR-203：Agent 声明段（profile-rules.v3）

- **允许修改：** samples/profiles/discipline.rules.json、desktop/src-tauri/src/profile_rules.rs（v3+兼容 v1/v2 加载）、SettingsPanel 规则资产页（只读展示 agents 段）、verify 脚本。
- **单一动作：** rules.json 增 agents 段（id/label/scenes/skills/permission_profile 引用）；v2 文件原样可加载；首个声明 agents.work-review = 现有工作记录场景 Skill 集的别名（行为零变化）。
- **验收命令：** `cargo test --offline --lib profile_rules`（v1/v2/v3 兼容用例）全绿；verify_profile_rules_068.py 保持全过。
- **依赖：** 无（与 201/202 并行）。

### OD-HAR-204：执行可观测补齐（token 字段+执行聚合查询）

- **允许修改：** engine/omnidesk_engine/task_core.py（usage 事件→台账字段）、web_api_adapter.py（usage payload 规范化）、contract.py、lib.rs（台账查询）、验证脚本。
- **单一动作：** 把 usage 流事件规范化落台账 token_in/token_out（缺失=unknown 不估算）；新增按 omni_task_id 聚合查询命令（状态/时长/token/错误）。
- **验收命令：** `python3 -m pytest tests/verification/engine-interface -q` 全绿（真实 usage 形态先以探针确认）；`cargo test --offline --lib` 全绿。
- **依赖：** 无（与 201-203 并行）；**前置事实核查：** 先用合成端点固定 usage 块字段（探针任务卡，半天）。

**Phase B 出口标准：** 四卡全部独立验收通过；无行为变化（events 与 agents 段为纯增量登记）；REVIEW 复核通过。

## Phase C：Work Review Agent 试点（任务卡）

### OD-HAR-211：work-review-core 权限变体

- **允许修改：** samples/profiles/discipline.profile.patch.yml、runtime/dsh-home-seed 对应文件、数据根 patch（备份后改）、profile_choice/相关校验脚本。
- **单一动作：** 按 D-092 模式新增 work-review-core preset（discipline-core 基础上去 write/edit/bash/pwsh/fs-search），Agent 声明引用之；--dump-config 探针确认工具面差异。
- **验收命令：** 临时 DSH_HOME --dump-config rc=0 且断言工具面；verify_profile_scope_106.py 适配后全过。
- **依赖：** OD-HAR-203。

### OD-HAR-212：历史记录检索 + 上下文扩展

- **允许修改：** 新增 desktop/src-tauri/src/worklog_search.rs（只读词法检索，复用 vault_search 模式与预算上限）、lib.rs 命令、task_extract.rs 注入相关历史（预算上限）、验证脚本。
- **单一动作：** 记录分析时检索相关历史记录（标题/标签/实体命中），注入候选提示词上下文（≤2000 字符），命中留痕进候选 JSON。
- **验收命令：** `cargo test --offline --lib worklog_search` 全绿；worklog_record_analyze 集成用例（合成记录）全过。
- **依赖：** OD-HAR-201。

### OD-HAR-213：Work Review Agent 端到端接通

- **允许修改：** conversationPrompt.ts（agent 键注入）、scenes.ts/workIntent.ts（intent→agent 映射）、TaskReviewPanel（verdict 展示联动）、验证脚本与证据。
- **单一动作：** 工作记录场景意图映射到 agents.work-review 声明：只读工具面+专属 Skill 注入+verdict 验收+事件登记全链路真实跑通（合成端点）。
- **验收命令：** tsc 0；npm build 0；cargo lib 全绿；新增 verify_work_review_213.py（隔离数据根+合成模型端点全链路）全过；证据落 tests/verification/desktop/evidence/od-har-213/。
- **依赖：** 211+212。

## Phase D：事件与交接机制

### OD-HAR-221：确认/复核反馈事件闭环

- **单一动作：** 用户确认/拒绝/修改候选时，decision_note 与 rejected 原因写事件（含修正 diff 摘要）；Knowledge/Project 侧挂只读事件订阅（启动时装载最近 N 条）。
- **验收命令：** cargo event_log 用例+新 verify 脚本全过。
- **依赖：** 202+213。

### OD-HAR-222：跨实体引用完整性验收器（只读体检）

- **单一动作：** 确定性扫描（dangling linked_task_ids / related_knowledge / source_record_id / project_id），产出体检报告命令（只读，不修复）。
- **验收命令：** cargo 用例含构造的 dangling 夹具；新 verify 脚本全过。
- **依赖：** 202。

## Phase E：评估与改进

### OD-HAR-231：Work Review Golden Task 集（合成）

- **单一动作：** 按 WORK_REVIEW_AGENT_DESIGN §五 建 12 用例+基准答案+确定性评价脚本；红线用例（WR-G10）100% 拒绝为硬门槛。
- **验收命令：** 新增评价脚本对合成夹具全过（不调模型的部分）；模型相关用例登记预注册口径。
- **依赖：** 213。

### OD-HAR-232：试点测量与改进报告

- **单一动作：** 按 §六 指标实测一轮（合成端点全流程+可选真实端点须用户另行授权），产出改进清单；规则修订（如有）过 Golden 回归。
- **验收命令：** 测量脚本输出完整指标表；REVIEW 复核。
- **依赖：** 231。

## Phase F：后续三 Agent 扩展（只登记顺序，另行规划）

触发条件：Work Review 试点 Phase E 指标达标+用户批准。推荐顺序与理由：

1. **Knowledge Agent**：与 Work Review 共享候选流与治理链路，事件订阅价值最高；
2. **Writing Agent**：能力已最完整（604-615），主要是 Agent 化组装；
3. **Project Agent**：依赖任务/排期数据成熟+前两者产出。

每 Agent 复用 Phase B 全部基础设施；预计各自 2-3 张任务卡（届时按当时代码现实重规划，不沿用本预估）。

## 风险登记

| 风险 | 缓解 |
| --- | --- |
| rules.json v3 迁移破坏用户既有文件 | serde default+向后兼容加载（203 卡明确 v1/v2 原样可加载） |
| preset 变体漂移（升级 DSH） | 沿用 082 的校验脚本模式，漂移即失败 |
| 事件表写放大/损坏 | 追加式+原子 append+损坏行跳过（202 卡测试项） |
| token 字段缺失（DSH usage 形态未知） | 204 前置探针；unknown 不估算 |
| 试点范围膨胀 | Phase C 只接通现有能力；新分析能力（历史检索）预算封顶 |
| 经验污染 | 人工修订+Golden 回归门槛（Graph 设计 §六） |
