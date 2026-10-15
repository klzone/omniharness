# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [0.2.1] - 2026-10-15 (discipline-v0.2 / 结构收敛)

### Changed
- **统一资产权威源**：Skill 正文以 `skills/`、材料模板以 `templates/discipline/`、
  写作风格与优秀范例以 `knowledge/` 为唯一开发源；`workstations/discipline/` 下的
  副本（knowledge/、templates/、skills/）已删除，替换为指向说明。
- `workstations/discipline/profile.yaml` 重新标注为「规划声明 · 非生效配置」，
  生效 Profile 源明确为 `profiles/yvan-work/`；其 package.json 更名为 `dsh-profile-yvan-work`。
- 移除与 `profiles/yvan-work/` 完全重复的 `profiles/desktop/` 参考快照。

### Added
- **运行底座锁定与引导**：新增 `runtime/RUNTIME.lock` 与 `scripts/bootstrap-runtime.sh`，
  一条命令即可得到与维护者验证一致的 DSH 版本（0.2.1-alpha.2），无需另找安装包。
- **DSH 源码底本可复现**：新增 `.gitmodules`，`runtime/deepseek-harness` 以子模块形式
  固定在 `dsh-v0.2.1-alpha.2`（commit d7432673），克隆即得同一底本。
- 新增 `data-template/README.md`，使文档承诺的外部数据骨架真实存在于仓库。

### Fixed
- **同步器安全缺陷**：`scripts/sync-skills.sh` 原先会把用户在隔离根自建的 Skill
  当作「多余」删除。现引入托管清单 `.omnidesk-managed`，只删除受同步器管理、
  且源目录已移除的 Skill；用户自有目录一律保留并提示。
- `--check` 不再把未托管目录计为漂移；新增 dry-run 汇总（新增/更新/删除/保留）。
- 同步中途失败会明确提示「可能处于半更新状态」，不再静默。

---

## [0.2.0] - 2026-10-14 (discipline-v0.2)

### Added
- **Discipline 知识库六大结构** (`knowledge/`)：
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

