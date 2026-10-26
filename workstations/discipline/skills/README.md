# 工作站技能（已收敛）

本目录**不再保存副本**。

Discipline 工作站的 Skill 正文唯一开发源是仓库根的 [`skills/`](../../../skills/)：

| Skill | 用途 |
| --- | --- |
| `skills/material-writing/` | 材料写作助手 |
| `skills/document-review/` | 材料质量评估 |
| `skills/knowledge-search/` | 知识检索（带来源引用） |
| `skills/knowledge-agent/` | 知识库整合 |
| `skills/memory-agent/` | 随手记（逐条增量落盘 + 缺项追问） |
| `skills/memory-report/` | 周报起草（读当周记录，确认后落盘） |
| `skills/anysearch/` | 联网检索薄包装（运行时按需加载，不预注入） |
| `skills/project-agent/` | 项目巡检与进度追踪 |

> 注意：DSH 只发现 `skills/<name>/SKILL.md` 形式的技能，不会扫描
> `workstations/` 目录。此处先前的 `*.md` 副本既不会被加载，也已落后于权威源。

同步到运行时请执行 `scripts/sync-skills.sh`。

