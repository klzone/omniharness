# 材料模板（已收敛）

本目录**不再保存副本**。

Discipline 工作站的材料模板唯一权威源是仓库根的 [`templates/discipline/`](../../../templates/discipline/)：

| 模板文件 | 用途 |
| --- | --- |
| `work-summary-template.md` | 工作总结 |
| `report-template.md` | 调研/汇报报告 |
| `speech-template.md` | 交流发言/讲话稿 |

模板由 `scripts/sync-skills.sh` 物化到 Skill 运行时的 `references/templates/` 目录，
Skill 通过相对路径读取。请勿在运行时副本上直接编辑。

