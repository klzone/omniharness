# OmniDesk Discipline MVP Plan v0.1

> 归档说明：此为早期实施计划，仅保留作历史记录，不是当前计划。现行计划见仓库根目录 PLANS.md；当前目标、状态和验收结论分别见 GOALS.md、STATUS.md 与 REVIEW.md。

> 执行目标：构建 OmniDesk Discipline v0.1 最小可用闭环  
> 执行原则：单步验证、严格收敛、绝不越界

---

## 执行步骤清单

| 步骤 | 任务名称 | 动作描述 | 交付产物 | 验收标准 |
|---|---|---|---|---|
| **Step 1** | 建立 Workstation 规范 | 建立 discipline 领域工作站目录 | `workstations/discipline/` | 包含 profile.yaml, skills/, knowledge/, templates/, memory/ |
| **Step 2** | 创建 Discipline Profile | 编写领域专属 DSH 配置补丁 | `workstations/discipline/profile.yaml` | 规范完备，可被 DSH 装载 |
| **Step 3** | 创建材料写作 Skill | 实现材料写作助手基础技能 | `skills/material-writing/SKILL.md` | 支持素材录入、大纲提炼与初稿生成 |
| **Step 4** | 建立模板库结构 | 创建标准化纪检材料模板 | `templates/discipline/` | 提供总结、汇报、发言、调研标准范式 |
| **Step 5** | 创建知识库 Skill | 实现严格带出处引用的知识检索 | `skills/knowledge-search/SKILL.md` | 强约束格式：答案+原文引用+来源文件+位置 |
| **Step 6** | 建立黄金任务测试集 | 编写基准任务集 | `tests/golden_tasks/` | 包含不少于 20 个基准 Case；素材限合成/公开/经书面授权数据 |
| **Step 7** | 建立工作区备份机制 | 编写一键安全归档脚本 | `scripts/backup-workspace.sh` | 可正确归档工作站与会话数据 |
| **Step 8** | 运行场景验证 | 用合成素材做端到端生成与测试 | `tests/reports/v0.1-validation-report.md` | 记录耗时对比、引用准度与产出质量；未获授权前仅代表技术/评测流程表现 |
