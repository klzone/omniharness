# 知识库（已收敛）

本目录**不再保存副本**。

Discipline 工作站的领域知识唯一权威源是仓库根的 [`knowledge/`](../../../knowledge/)：

| 分类 | 权威源路径 | 用途 |
| --- | --- | --- |
| 法规制度 | `knowledge/regulations/` | 纪检法规、制度文件（真实业务文件已被 `.gitignore` 排除，不入库） |
| 上级政策 | `knowledge/policies/` | 上级政策与工作要点 |
| 写作风格 | `knowledge/writing-style/` | 个人写作风格规则（唯一开发源） |
| 优秀范例 | `knowledge/excellent-cases/` | 经审核的历史范文（唯一开发源） |
| 会议素材 | `knowledge/meeting-materials/` | 会议纪要素材 |
| 方法论 | `knowledge/methodology/` | 纪检工作方法论 |

## 为什么不再放副本

先前此处维护了一份 `knowledge/` 的副本，Phase 2A 后两份副本开始分叉（此处为旧版），
存在「改一处、另一处不生效」的风险。现在统一为单一权威源。

需要同步到运行时，请执行 `scripts/sync-skills.sh`。

