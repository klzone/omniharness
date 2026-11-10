# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [Unreleased] - 2026-11-10

### Added
- 首页 2.0 版式：问候带系统用户名（只读命令 `desktop_user_name`）、四张真实入口卡片、底部真实场景快捷入口、右上淡蓝山海背景插画（`desktop/public/home-scape.png`）。
- 线程记录新增 `forkOrigin`（父会话号与切点），新分支线程顶部显示一行灰色来源说明；普通新建线程不显示。
- 线程存储新增“未知前端字段原样保存与读回”的 Rust 回归测试。

### Changed
- 首页二次修订：去掉四张入口卡片、问候下方说明句与手掌表情；底部胶囊改为最近五个项目；对话开始后收起问候区，历史记录占满上方空间。
- 首页按新参考图重排：顶栏 30px 圆角；左侧 Dock 收进胶囊导轨，当前项为品牌靛实心圆；输入面板改为 26px 凸起带淡紫光晕；空态标语改为两侧分隔线夹一行演示夏行楷（靛蓝灰 `#615A85`）。
- Neo-Skeuomorphism 2.0 校准：象牙灰底材、近白凸起面、品牌靛 `#5747F8`；顶栏 34px 圆角，输入面板 24px，空态标语 30px / 0.12em 字距。
- 首页新对话空态标语改用品牌靛蓝，不再使用黑色正文色；保持演示夏行楷与 700 加粗。

### Fixed
- 首页归类菜单、模型下拉和 Profile 弹层统一收窄：归类菜单 176px，模型 248px，Profile 190–250px。
- 分支来源在读取与创建两端都走同一 `normalizeForkOrigin`，空父会话号或非法切点按“无来源”处理，避免当前会话谎称“由分支创建”。

### Verification
- `verify_fork_origin_202.py`：9/9。
- `cargo test --offline --lib thread_store::tests`：13/13。
- `npm run build`：通过。

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

