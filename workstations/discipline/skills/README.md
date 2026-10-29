# 工作站技能（已收敛）

本目录**不再保存副本**。

Discipline 工作站的 Skill 正文唯一开发源是仓库根的 [`skills/`](../../../skills/)：

| Skill | 用途 | 绑定 / 按需 |
| --- | --- | --- |
| `skills/material-writing/` | 材料写作助手 | 绑定 `writing` |
| `skills/document-review/` | 材料质量评估 | 按需 |
| `skills/knowledge-search/` | 知识检索（带来源引用） | 按需 |
| `skills/knowledge-agent/` | 知识库整合 | 按需 |
| `skills/memory-agent/` | 随手记（逐条增量落盘 + 缺项追问） | 绑定 `weekly-note` |
| `skills/memory-report/` | 周报起草（读当周记录，确认后落盘） | 绑定 `weekly-report` |
| `skills/anysearch/` | 联网检索薄包装 | 按需 |
| `skills/project-agent/` | 项目巡检与进度追踪 | 绑定 `project` |

绑定（`rules.json` 的 `skill_bindings`）只写每条流程**离不开的那一个**技能，正文随该流程的提示词一起发出；
其余技能由模型在需要时自行读取。旧键 `weekly` 保留为随手记的兼容别名，指向 `memory-agent`。

> 注意：DSH 只发现 `skills/<name>/SKILL.md` 形式的技能，不会扫描
> `workstations/` 目录。此处先前的 `*.md` 副本既不会被加载，也已落后于权威源。

同步到运行时请执行 `scripts/sync-skills.sh`。

