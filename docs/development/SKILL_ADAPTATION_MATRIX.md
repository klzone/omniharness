# 技能适配矩阵（草案，待用户批准）

> 判定基准：双会话双模型分工（GLM 规划 / DeepSeek 执行）、不派子代理、轻量低 Token、不新增插件安装。

## 一、Superpowers（obra/superpowers）

| 技能 | 适用角色 | 解决的问题 | 是否引入 | 优先级 | 与其他技能重复 |
| --- | --- | --- | --- | --- | --- |
| brainstorming | GLM | 需求回显、关键分歧前置、一次一问 | 是（规则化，非装技能） | 高 | 与 spec-driven 前两阶段重叠，取其一 |
| writing-plans | GLM | 任务粒度、计划头字段、验收命令前置 | 是 | 高 | 与 planning-and-task-breakdown 互补合并 |
| executing-plans | DeepSeek | 照计划执行、裁决留痕、四类停下条件 | 是（停下条件 + Ruling 格式） | 高 | 与 subagent-driven 的执行纪律部分重复 |
| subagent-driven-development | — | 子代理调度 | 否（双会话模式不符 + 用户约束） | — | 执行纪律部分并入 executing-plans 吸收 |
| test-driven-development | DeepSeek | 缺陷复现先行 | 部分（Prove-It 必用；红灯强制放宽） | 中 | 与 addy TDD 合并为一条规则 |
| verification-before-completion | 双方 | 声明→证据对照、推测措辞视为未验证 | 是 | 高 | 与 addy code-review 审查轴互补 |
| systematic-debugging | DeepSeek | 根因先行、停线 | 是（合并规则） | 高 | 与 debugging-and-error-recovery 重叠，取并集 |
| using-git-worktrees | — | 实施隔离 | 暂缓（出现真正并行开发再评估） | 低 | — |
| requesting/receiving-code-review | GLM | 审查交接礼节 | 否（现行 REVIEW.md 流程已覆盖） | — | — |
| finishing-a-development-branch | — | 分支收尾 | 否（不擅自提交推送的现状不符） | — | — |
| writing-skills / using-superpowers | — | 技能维护/调度 | 否 | — | — |

## 二、Addy Osmani（addyosmani/agent-skills）

| 技能 | 适用角色 | 解决的问题 | 是否引入 | 优先级 | 与其他技能重复 |
| --- | --- | --- | --- | --- | --- |
| spec-driven-development | GLM | 大功能能力地图、阶段把关 | 部分（能力地图用于立项） | 中 | 与 brainstorming 前段重叠 |
| planning-and-task-breakdown | GLM | 依赖图、任务下限（一会话可完成） | 是（与 writing-plans 合并） | 高 | 与 writing-plans 合并 |
| constraint-driven-development | 双方 | 门禁分级、禁悄悄降门槛 | 部分（红线条款进门禁，不建 CONSTRAINTS.md） | 高 | 红线与 verification 部分重合 |
| test-driven-development | DeepSeek | 仓库真实命令发现 | 是（命令清单登记） | 中 | 与 superpowers TDD 合并 |
| code-review-and-quality | GLM | 五轴审查、审批标准 | 是 | 高 | 五轴为 GLM 审查主框架 |
| context-engineering | 双方 | 五层上下文金字塔 | 是（作 CONTEXT_MANAGEMENT 框架） | 高 | — |
| debugging-and-error-recovery | DeepSeek | 停线 + 六步分诊 | 是（与 systematic-debugging 合并） | 高 | 与 systematic-debugging 取并集 |
| code-simplification | DeepSeek | 行为不变简化 | 暂缓（只由计划发起的专项任务） | 低 | 与 code-review 可读性轴部分重叠 |

## 三、共同规则（两个会话）

1. 技能不常驻，按任务卡点名启用；每次会话最多同时激活两个专项技能。
2. 验证命令以 §OMNIDESK_ENGINEERING_STANDARD 3.1 清单为唯一来源。
3. 声明完成必须有本轮实跑证据；推测措辞视为未验证。

## 四、明确不引入清单

subagent 调度、git worktree（暂缓）、CONSTRAINTS.md 独立文件、CLAUDE.md 模板细节、hooks/commands/evals 配套、技能 announce 套话、任何形式的技能常驻加载。
