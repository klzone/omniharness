# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [0.2.23] - 2026-11-12

### Added
- 工作记录模块 1.0：原始素材与正式记录分离、修订历史、当日摘要、周/月/季/年周期复盘、复盘档案、记录反链与「交给 AI 继续分析」。
- 工作记录分页新增右栏本月概览、一键本月复盘与四个快捷操作；记录详情新增关联任务管理。

### Changed
- 工作记录分页收敛为「工作记录 / 周期复盘」两个页签；审核候选改为浮层，任务管理回到项目跟踪分页。
- 时间线按日期倒序、组内按当天时间正序；记录卡补齐时间、分类、摘要、来源、关联任务与解析状态。

### Verification
- cargo test --offline --lib：1157/0。
- npx tsc --noEmit 与 npm run build：通过。
- 工作记录相关 verify 脚本 301/502/503/504/505/506/507/508/509 全通过。

## [0.2.31] - 2026-11-17

### Changed
- Skill 优化（OD-ARCH-174）：四场景技能指针各归其位（修 writing/project 可能错拿周报指针）；知识检索专用绑定键 `knowledge → knowledge-search`，宿主检索命中时追加 151 字符检索指针，普通对话同样生效（D-108）；模型技能目录 2518 → 2401 字符（document-review 转纯手点）；核心技能正文重写——memory-agent 1754→1228、memory-report 1099→749、period-review 1820→1268，输出契约与硬约束逐字保留。

### Verified
- 「指针 → 模型调用 skill 读取 → 按契约输出」全链路真实模型验证：模型实际调用 skill(memory-report) 读到重写正文并按五段模板完整产出。
- cargo test 1200 全过；tsc / npm run build 零错误；verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、wr_503 58/58、wr_505 71/71。

### 升级说明
- 已安装用户需在 Profile 资产页点一次「恢复默认规则包」以获得 knowledge 检索绑定。

## [0.2.30] - 2026-11-17

### Added
- 首页 2.0 版式：问候带系统用户名（只读命令 `desktop_user_name`）、四张真实入口卡片、底部真实场景快捷入口、右上淡蓝山海背景插画（`desktop/public/home-scape.png`）。
- 线程记录新增 `forkOrigin`（父会话号与切点），新分支线程顶部显示一行灰色来源说明；普通新建线程不显示。
- 线程存储新增“未知前端字段原样保存与读回”的 Rust 回归测试。
- 设置页新增「配置解释」只读页（`diagnostics_metadata`）：生效路由、接口与地址、思考档位及声明、上下文窗口、提示词分段字符、工具面、Skill 概况，均可回答“这条行为从哪来”；只出元数据，不出提示词正文或用户内容。
- 模型保存与切换前预检：保存时按 Profile 的 `reasoningEfforts` 校验档位，401/402/403 给出认证、额度、套餐权限提示并阻止保存；切换时用 DSH 模型目录校验显式档位。

### Changed
- 授权：OmniDesk 自有代码自 0.2.30 起改用 PolyForm Noncommercial 1.0.0，商业用途须取得书面商业授权；0.2.29 及更早版本仍按 Apache-2.0 授权；`runtime/` 维持上游 MIT。
- 效率（OD-ARCH-173）：discipline preset 系统提示词 5396 → 2983 字符（去 PTC 口径；保留 PTC 时 15252），web-search 段 1919 → 0；随手记/周报/当日摘要/周期复盘的 user 段 2131/1446/1450/2222 → 636/558/487/613 字符（−61%～−72%）；Skill 全文改为 ≤260 字符指针，模型按需调用 `skill` 取正文（PTC 下在 `run_code` 内调 `tools.skill`）。
- 安全（OD-ARCH-173）：discipline Profile 移除 `dsh-free-search` 依赖与宿主插件要求，公开检索改由 anysearch Skill 按需承担；read/grep/glob 输出上限写入 preset（read 2000 行 / 51200 字节 / 单行 2000 字符，grep 250 条 / 单行 500 字节，glob 100 条）；模型保存/切换增加可用性预检，预检不读明文凭证。
- 工程：2026-11-17 之前的测试截图、任务证据与历史报告移出 Git，归档到仓库外；Git 保留测试脚本、夹具与架构契约。
- 仓库清理：个人发言稿、个人写作风格、个人数据根配置和初始化脚本移出 Git；写作风格改为通用模板，开发 Profile 改为中性名称，公开仓库不再包含个人文档。
- 发布：2026-11-17 重写 Git 历史并清理旧个人文档/证据，旧安装包已下线；0.2.30 将从清理后的源码重新构建。
- 发布形态：0.2.18 起只提供安装包（macOS universal dmg、Windows NSIS setup.exe），发布工作流不再组装便携 zip。
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
- `npx tsc --noEmit`、`npm run build`：通过。
- `cargo test --offline --lib -- --test-threads=1`：1200 passed / 0 failed / 1 ignored。
- OD-ARCH-173 相关 `verify_conversation_prompt.py`、`verify_worklog_104.py`、`verify_wr_503.py`、`verify_wr_505.py`、`verify_tool_surface_154.py`、`verify_work_review_213.py`、`verify_profile_assets_109.py` 全通过；Skill 指针相关 `verify_skill_bindings_082.py`、`verify_worklog_089.py` 已按新行为更新断言后通过。

### Known limitations
- 真实模型 smoke test（Excel、周报、知识整理）未执行：账户返回 402 余额不足，属账户状态而非代码失败；提示词结构变化已由 mock 合成会话证明，真实模型行为待账户恢复后复测。
- Windows 安装包未在本机构建验证；macOS 侧完成 tsc、vite、cargo 与 Tauri 编译链路验证。
- 既有安装的 discipline Profile 不会自动删除已装 `dsh-free-search`：宿主装配只在缺失时写入、不覆盖用户文件；升级用户可在资产页点「恢复出厂内容」（写前备份），或重装 Profile。新装包由发布工作流重建 seed 后生效。
- 配置解释页未展示逐轮耗时、审批等待与缓存命中（阶段 5 明确不做时间线改版）；DSH 注入的 Skill 目录（约 2518 字符）仍存在，压目录需改上游 tool-skill 配置。

## [0.2.1] - 2026-10-15 (discipline-v0.2 / 结构收敛)

### Changed
- **统一资产权威源**：Skill 正文以 `skills/`、材料模板以 `templates/discipline/`、
  写作风格与优秀范例以 `knowledge/` 为唯一开发源；`workstations/discipline/` 下的
  副本（knowledge/、templates/、skills/）已删除，替换为指向说明。
- `workstations/discipline/profile.yaml` 重新标注为「规划声明 · 非生效配置」，
  生效 Profile 源明确为 `profiles/omnidesk-dev/`；其 package.json 更名为 `dsh-profile-omnidesk-dev`。
- 移除与 `profiles/omnidesk-dev/` 完全重复的 `profiles/desktop/` 参考快照。

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

