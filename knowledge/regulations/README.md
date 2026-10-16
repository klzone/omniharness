# 法规制度库 (Regulations)

## 目录职能
存放权威、现行有效的党内法规、国家法律法规与本单位内部制度文本。

## 当前收录（2026-10-16 批量入库）

从单位制度汇编导入，按原文件夹分类，**共 232 份文件**：

| 分类 | 来源 |
| --- | --- |
| `一、现行常用有效制度55项/` | 党中央制度、派驻组制度、深圳分行纪委制度 |
| `1、农行派驻组制度汇编/` | 组织法规、领导法规、自身建设、监督保障 |
| `2、湖南分行纪委制度汇编/` | 湖南省分行各类实施细则与通知 |
| `3、深圳分行纪委制度汇编/` | 深圳市分行制度 |
| `处分条例/` | 纪律处分条例 2015/2018/2023 版、党内监督条例、监督执纪工作规则 |
| `二、拟废止制度5项/` | 拟废止制度（保留原始，供比对） |
| `三、拟增加制度1项/` | 拟增加制度 |

## 转换说明
- `.wps` 已转成 `.docx`（可被 `knowledge-search` 读取）；
- `.doc/.docx/.pdf` 原样保留；
- `.et/.gd/.ofd/.emmx/.zip` 等格式**未转换、未入库**（见 manifest 的 `untouched_files`）；
- **源文件未被修改**；完整来源与处理记录见本目录 `manifest.json`。

## ⚠️ 已隔离的三级敏感数据
原 `问题线索分析组-专项报告/`（含案件分析、责任追究台账、涉诉分析，涉及真实人名、
支行名、案涉企业与金额）**已移出知识库**，存放于 `~/OmniDesk/quarantine/red-line-case-data/`。
依据：[入库与脱敏规则](../../docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md)。
该目录不会被任何 Skill 检索到，也不在 Git 仓库内。

## 使用准则
1. 知识库助手 (`knowledge-search`) 检索时的法定依据源；条文必须保持原文，严禁篡改。
2. 内部制度仅供本人工作使用，不得外传；本目录内容**不入 Git**（`.gitignore` 已排除）。
3. 重新入库或增量更新：
   ```bash
   python3 scripts/ingest-documents.py --source "<源目录>" --dest knowledge/regulations --dry-run
   python3 scripts/ingest-documents.py --source "<源目录>" --dest knowledge/regulations
   ```
