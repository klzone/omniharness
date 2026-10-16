# OmniDesk 知识库 (Knowledge Base)

> 这是个人 AI 工作站的知识资产根目录。
> **入库前先读规则**：[docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md](../docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md)

## 目录一览

| 目录 | 内容 | 数据分级 | 进 Git |
| --- | --- | --- | --- |
| `regulations/` | 法规制度原文、单位内部制度 | 一/二级 | 否（真实业务文件） |
| `policies/` | 上级政策部署 | 一级 | 是 |
| `methodology/` | 方法论沉淀 | 二级 | 是 |
| `writing-style/` | 本人写作规则（风格 / 表述 / 结构 / 避雷） | 二级 | 是 |
| `excellent-cases/` | **脱敏后**的优秀范文 | 二级 | 是 |
| `meeting-materials/` | 会议材料 | 二级 | 是 |
| `reflections/` | **本人原创**心得体会 / 发言 / 汇报 / 新闻稿 | 二级 | 否（本地保留） |
| `learning-materials/` | 学习资料（公开出版物、课件） | 一级 | 否（体积大） |

## 三条最重要的规则

1. **红线数据一律不进库**：案件材料、案件线索、未公开调查信息、涉案人员真实信息、未公开通报、举报内容、干部廉政档案。不得以“脱敏”为由绕过。
2. **心得体会放 `reflections/thoughts/`**，无需脱敏，但只留本地、不上 GitHub。
3. **要让 AI 当范文参考**：先脱敏，再复制一份到 `excellent-cases/`；原文仍留在 `reflections/`。

## 批量入库工具

```bash
# 预览（不写入）
python3 scripts/ingest-documents.py --source "<源目录>" --dest knowledge/regulations --dry-run
# 实际入库（源文件只读；.wps 转 .docx，.doc/.docx/.pdf 复制，其余格式不动）
python3 scripts/ingest-documents.py --source "<源目录>" --dest knowledge/regulations
```

每次批量入库会在目标目录生成 `manifest.json`，记录来源路径、处理动作与时间。

## 统一知识目录约定

知识库**内容**与本仓库的 `knowledge/` 是同一份（仓库即权威源）；隔离实例运行时需要的写作资产由 `scripts/sync-skills.sh` 物化到 Skill 的 `references/` 副本中。

