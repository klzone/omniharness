# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [0.2.32] - 2026-11-17

### Added
- 工作记录场景自然语言意图重构（OD-ARCH-176）：每轮先由模型按统一意图契约分类（事实 / 指令 / 闲聊 / 复盘 / 周报 / 当日摘要），只有事实才落盘；指令与闲聊转为对话回复并带本周记录索引；复盘请求在工作记录场景内直接汇集材料出稿（OD-WR-505）；拿不准走提问卡确认。陈述句启发式快路径退出该场景，周报场景关键词快速通道不变。
- 模型运行期思考档位自动回退（OD-ARCH-175）：未声明档位的模型其显式档位被引擎运行期拒绝时，宿主自动重写覆盖文件去掉档位、重启引擎并提示重发；同一 (路由, 档位) 只触发一次；切换前的阻断式预检改为本地回退 + 一句话告知。
- 设置页「配套插件推荐」（OD-ARCH-177）：AnySearch CLI 与 dsh-cost-meter 的名称、上游地址、建议版本；仓库不打包插件实体，不自动安装、不联网检查版本。
- 工作记录场景思考档位策略：分类 / 分析 / 指令回复默认 off 模型直出（按会话覆盖，不重启引擎），用户手动选档即按所选；声明集合不含 off 的模型自动降级为跟随默认。

### Changed
- skills/period-review：工作台汇集材料随请求送达时直接起草，不再输出等待类话术；skills/anysearch：引擎环境无本机 CLI 属预期，直接回退 web_fetch 并如实标注来源与限制。
- 模型目录契约：`ModelCatalogEntry.efforts` 改为可选（归一化目录可能缺项）。

### Verification
- cargo test --lib：1209/0（并发运行偶现 3 个引擎 spawn 类环境性失败，单线程与复跑均不复现）。
- npx tsc --noEmit、npm run build 通过；verify_arch_176 70/70、model_route_precheck_175 12/12 + 11/11、companion_plugins_177 11/11、worklog 089 86/86 / 103 52/52 / 104 73/73 / 505 71/71、conversationPrompt 33/33、skill_bindings 69/69；home_refine_202 39/40（唯一失败为既有项）。
- 升级说明：已安装用户需在 Profile 资产页点「恢复默认规则包」获得更新后的 period-review / anysearch 技能文本。

## [0.2.34] - 2026-11-17

### Added
- 思考档位声明进发行链（OD-ARCH-178 阶段 A）：宿主已验证清单（opendesign / deepseek-v4.1-flash 的 7 档实测映射）并入 modelCatalog 与覆盖文件渲染，Profile 已声明不覆盖。
- 全部提交路径接入两层档位证据门（OD-ARCH-178 阶段 B）：普通对话 / 未分类场景纳入证据门，未声明模型不传档位、静默降级；api 层 gatedReasoningEffort 作为末道防线。
- ModelPicker 目录驱动（OD-ARCH-178 阶段 C，DSH 原版行为）：档位按钮按当前模型声明集合渲染，未声明模型只显示「跟随模型默认」。
- discipline preset persona 新增自查收敛规则（OD-ARCH-179）：OmniDesk 自查类问题先读 app/config 配置文件；bash 禁扫 storages/ 与 node_modules/。

### Changed
- 档位证据解析收敛到单一模块 modelEffortEvidence.ts（目录优先、Profile 兜底），界面渲染与提交门禁同口径。

### Verification
- cargo test --lib 单线程 1217/0；tsc / npm run build 通过；verify_arch_178 20+24、arch_176 71/71、model_route_precheck_175 12+12、companion_plugins_177 11/11、home_refine_202 39/40（既有失败）。
- 真实 DSH 双向验证：声明档位后 selectModel 接受 off 且任务成功（正向新增用例 PASS）；未声明模型传 off 优雅降级不炸任务（verify_effort_select_degrade 4/4）。

### 升级说明
- 已安装用户需在 Profile 资产页点「修复缺失 / 恢复出厂内容」获得档位声明与自查规则；升级后模型选择器只显示当前模型声明过的思考档位。

## [0.2.33] - 2026-11-17

### Fixed
- 工作记录场景每轮报 engine_unavailable 的线上回归（OD-ARCH-176）：真实 DSH 的 session/selectModel 会拒绝未声明思考档位模型的任何显式档位（含 off），此前「未声明即传 off」的设计使该场景所有消息必然失败，且错误被兜底映射成 engine_unavailable 误导排查。现在档位必须被两层证据「证明支持」才下发（未声明模型不传档位——本就没有思考可关，不传即等效直出）；适配器对 select_model 失败优雅降级（记 stderr、按模型默认继续，不炸整轮）；session/model-unavailable 错误改判 invalid_argument，呈现真实原因而非误报引擎不可用。

### Verification
- 新增真实 DSH 回归 verify_effort_select_degrade.py 4/4：selectModel 拒绝 → stderr 降级记录 → 任务按默认档成功完成。
- verify_arch_176 71/71（档位用例按「证明支持才下发」新语义更新）；cargo test --lib 1209/0（单线程）；tsc / npm run build 通过。

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

