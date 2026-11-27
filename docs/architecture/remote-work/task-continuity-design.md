# 跨设备任务连续性及 Agent Handoff 设计（Step 10）

- 日期：2026-11-27
- 问题：建立独立于模型上下文的持久化任务连续性机制；覆盖任务全生命周期对象与各类中断场景。

## 1. 设计核心

任务连续性的事实源不是聊天历史，而是**任务文件 + 事件表 + 结果落盘**三份已有持久层（Step 3/4 已证实存在）。本设计把它们升级为显式的 Task Context 对象，增加 Handoff Snapshot 作为会话无关的任务交接载体。

## 2. Task Context 对象模型

| 对象 | 载体（复用） | 内容 |
| --- | --- | --- |
| Task Definition | work/任务/<id>.md frontmatter | id、title、category、due、importance、project_id、depends_on |
| Plan | 任务文件 ## 进展 前新增 ## 计划 段（或 JSON 附文件，Phase 3 决定） | 目标、步骤清单、每步负责 Agent（GLM/DeepSeek/GLM）、验收标准 |
| Progress | ## 进展 追加条目 + events.jsonl task.* 事件 | 已完成步骤、当前步骤 |
| Decisions | ## 调整记录 追加条目 | 关键决策与理由（防上下文丢失后重走弯路） |
| Execution Records | engine-state/omni-*.json 台账 + results/ | omni_task_id、engine_session_id、状态、结果路径、token 用量 |
| Artifacts | workspace/artifacts/ + engine-state/results/ | 产物文件路径清单（任务文件里登记相对路径） |
| Review Results | verdict（Multi-Agent 设计已有 schema 草案） | 验收 Agent 的结论、checks、decision_note |
| Pending Actions | 任务文件 ## 待办 段（新增） | 下一步动作、阻塞项、需要人工确认的事项 |
| Handoff Snapshot | work/handoff/<task_id>/snapshot-<n>.json | 见 §4 |

## 3. 恢复场景与机制

| 场景 | 机制 | 现有基础 |
| --- | --- | --- |
| Agent 中断（模型超时/断流） | 任务标 interrupted（终态）；从 Progress 最后完成步重建上下文，**新建任务**续做，引用原任务 Artifacts | 九态状态机 + 不自动重放（契约 3.8） |
| 模型切换（GLM 规划→DeepSeek 执行→GLM 验收） | Plan 里每步声明模型/Agent；步骤切换=新提交（submit_task 带 handoff snapshot 引用），Engine 端 selectModel 或独立会话 | DSH session/selectModel 存在（D-109 验证）；候选/verdict 交接物已有 schema |
| Session 退出/客户端切换 | 会话随中央 Engine 存活，客户端断开无影响；重连 stream_events 补漏 | 会话在中央数据根（Step 3） |
| Host 重启 | 中断任务进 interrupted/unknown；Host 启动时扫描台账，列出"可续做任务"（有 Plan+Progress+未完 Pending Actions），经用户确认后按快照新建任务 | 台账 + 事件表已在；新增启动扫描 |
| 上下文耗尽 | 不依赖原会话续聊；新会话 + Handoff Snapshot 重建任务上下文 | 结构化交接原则（Multi-Agent 设计 §二.1） |

## 4. Handoff Snapshot 结构（JSON，新增 schema omnidesk.handoff.v1）

```json
{
  "schema": "omnidesk.handoff.v1",
  "task_id": "t-...",
  "created_at": "...",
  "plan_ref": "任务文件路径 + 计划段哈希",
  "completed_steps": ["step-1", "step-2"],
  "current_step": "step-3",
  "decisions": ["..."],
  "artifacts": [{"path": "workspace/artifacts/...", "hash": "..."}],
  "review_results": [{"step": "step-2", "verdict": "approved", "note": "..."}],
  "pending_actions": ["step-3: 用 deepseek 执行初稿", "step-4: 用 glm 验收"],
  "context_digest": "3-5 句任务背景摘要（模型生成，人工可改）",
  "source_session": {"engine_session_id": "...", "note": "仅溯源，恢复不依赖"}
```

规则：

1. snapshot 是**人可读可改**的 JSON，落中央文件域（work/handoff/），自动进入数据矩阵的中央实时域。
2. 恢复 = 新 submit_task，prompt 由 snapshot + 对应 Skill 模板组装；原 engine_session_id 只作溯源。
3. snapshot 由宿主在关键节点（步骤完成、中断、人工确认）自动生成，模型可更新 context_digest 但不接管其他字段。

## 5. 多 Agent 工作流的连续性（GLM 规划 / DeepSeek 执行 / GLM 验收）

- 规划（GLM）：产出 Plan 段 + 初始 snapshot。
- 执行（DeepSeek）：按 Plan 单步提交，产物写 artifacts，事件表记录。
- 验收（GLM）：读 snapshot + artifacts + verdict，产出 Review Results；未过则回 Pending Actions。
- 每步之间通过文件域交接，任何一步的模型实例都可以是"新开的"，满足跨设备/跨会话要求。

## 6. 与现有代码的关系

- 复用：九态状态机、任务文件三段式（追加段模式扩展）、events.jsonl、engine-state 台账、候选/verdict schema。
- 新增：Plan/Pending 段、handoff/ 目录与 schema、Host 启动扫描、模型→步骤映射。
- 不做：通用工作流引擎、跨会话消息注入（DSH 无导入接口，明确停做）。
