# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [0.2.0] - 2026-10-14 (discipline-v0.2)

### Added
- **Discipline 知识库六大结构** (`workstations/discipline/knowledge/` 与 `knowledge/`)：
  - `regulations/`：法规制度库
  - `policies/`：上级政策与工作要点库
  - `writing-style/`：个人写作风格库
  - `excellent-cases/`：历史优秀范文库
  - `meeting-materials/`：会议纪要素材库
  - `methodology/`：纪检工作方法论库
- **个人写作风格规范资产** (`knowledge/writing-style/`)：
  - `style-guide.md`：10 大核心写作铁律
  - `common-phrases.md`：高频规范表达与对仗语料
  - `structure-patterns.md`：成熟段落三步递进与机制提炼模型
  - `avoid-list.md`：去 AI 味与空泛表述避坑清单
- **技能升级与新增**：
  - 升级 `skills/material-writing/SKILL.md` (v0.2)：三位一体上下文驱动（模板+风格库+案例比对）
  - 新增 `skills/document-review/SKILL.md`：四维度文稿质检评估能力（结构、事实、避坑、合规）
- **评测与测试基准**：
  - 黄金任务集扩展至 40 个真实脱敏用例 (`tests/golden_tasks/tasks.json`)
  - 建立 20 次真实履职人机协同工时记录 (`memory/usage-log/`)，平均单项提效 87.1%
  - 建立 30 题法规检索抽查测试报告 (`tests/reports/citation-audit-report.md`)，准确率 96.7%
- **版本归档机制**：
  - 增加 `VERSION` 文件与 Git Tag `discipline-v0.2`

---

## [0.1.0] - 2026-10-14 (discipline-v0.1)

### Added
- 建立 `workstations/discipline/` 工作站规范与 Profile 定制配置
- 确立《OmniDesk 系统设计原则与设计方案 v0.3》（先产品后平台、数据分级保护）
- 落地第一版 `material-writing` 与 `knowledge-search` 技能
- 确立 3 份基础公文模板 (`templates/discipline/`) 与首批 20 个黄金任务基准
- 实现会话与工作区自动镜像归档脚本 (`scripts/backup-workspace.sh`)
- 冻结系统 Phase 0 五层解耦架构与三大红线

