# 当前实施计划

### OD-ARCH-173（v3，最终版）：0.2.30 成熟、高效、安全基线（已批准，2026-11-17 用户批准执行；完成后允许提交推送并编译 0.2.30）

> **本版定位：** 本节是 0.2.30 的唯一执行计划，取代下方 OD-ARCH-172（v2）。v2 由 DeepSeek 起草（提交 1c3514a），ARCHITECT 复核后确认：三主线（效率/安全/成熟）与"明确不做"边界成立，阶段顺序合理；但存在四处问题——①阶段 1 遗漏 11-16 已合入的 PTC 修复（OD-ARCH-154）与 11-15 已修的思考等级通道（OD-ARCH-153），直接执行会重做或冲突；②阶段 2"Skill 按需加载"依赖 DSH tool-skill 的实际行为，未先验证就承诺验收指标；③阶段 0 的两个基线脚本可与现有 diagnose-omnidesk.py 合并观测口径，避免重复造轮子；④阶段 4 模型预检与 model_settings.rs 现状（provider/api/apiKeyEnv/reasoningEffort 已落）存在字段重叠，需先盘点再补差。本版按此收窄与排序，执行主体为子代理 OpenDesign/deepseek-v4.1-flash-eco + high（用户当轮明确授权派工，符合 AGENTS.md 派工例外条款）。

**与 v2 的差异（先读这里）：**
1. 阶段 0 基线脚本改为"先复用 diagnose-omnidesk.py 的进程/端口/配置元数据，再补提示词分段与时间线拆分"，不新建两套入口。
2. 阶段 1 前置确认两项已合入修复不回退：PTC（tool-presentation/present 已在 discipline-core）与 thinking 等级（agent-default-model 块已写 reasoningEffort）。
3. 阶段 2 拆成两步：先只读验证 DSH tool-skill 能否按需读 Skill 正文（合成会话 + 日志证据），验证通过才改 conversationPrompt 注入策略；不通过则保留全文注入并如实记录缺口，不硬砍。
4. 阶段 4 改为"盘点 model_settings.rs 已有字段与 DSH 侧缺失项，只补差"，不重写已有 provider/api/apiKeyEnv/thinking 字段。
5. 全程不动 runtime/；不改 OD-UI-717 已批准的纸面风格方向（阶段 1 的 persona 精简只改提示词文本，不改样式）。

**阶段 0：基线（只读，半天）——已完成，证据 tests/verification/desktop/evidence/od-arch-173/baseline.md**

**阶段 1 补充裁决（ARCHITECT 复核后追加，2026-11-17）：**
> 1. PTC 与提示词预算冲突：保留 PTC 时 system 15252 字符（PTC 类型注入 12098），去 PTC 时 2983。裁决：保留 PTC（OD-ARCH-154 是已验收的批处理能力，价值大于提示词体积；且 PTC 减少工具往返本身服务同一效率目标）。阶段 1 验收改为"web-search 段归零 + 去 PTC 口径 ≤3000 字符"，≤1000 tokens 目标仅对非 PTC preset 有效。
> 2. 阶段 3 增补任务：检查 discipline.profile.json 的 dependencies 与 profile_assets.rs 的 DISCIPLINE_PLUGINS 中残留的 dsh-free-search 声明，随工具面收敛一并清理（这属于"按动作收敛工具白名单"的同一范围）。
> 3. 阶段 6 增补任务：现有安装的 Profile 需要重播种/迁移才能移除已安装 bundle 中的 dsh-free-search，纳入发布步骤。
> 4. 真实模型复测统一推迟到阶段 6 smoke test（402 余额问题需用户处理；mock 复测已足证提示词结构变化）。
> 1. 记录 HEAD/git status；确认 OD-ARCH-153/154 两项已合入修复在工作树中未回退（grep 即可）。
> 2. 基于 scripts/diagnose-omnidesk.py 扩展只读模式：输出最近一次真实会话的提示词分段字符数、工具调用次数、首轮响应耗时、总时长；不输出正文与凭证。
> 3. 在隔离数据根跑三类合成任务各一次（Excel、周报、知识整理），基线表落 tests/verification/desktop/evidence/od-arch-173/。
> 4. 验收：同一张表能回答"397 秒里模型/工具/审批/压缩/缓存各占多少"，并作为后续阶段对照。

**阶段 1：提示词装配（效率第一刀）**
> 1. discipline-core 与 work-review-core 的 persona 前缀从 "You are a coding agent powered by the {{model}} model." 换成一句 OmniDesk 底座：个人工作台账助手，只读写用户明确指定的文件，日期与统计由宿主计算，不确定时如实说明。
> 2. 从 discipline preset 移除常驻 web-search-free 插件声明（samples/profiles/discipline.profile.patch.yml 第 14 行区域），检索收敛/预算/工具选择规则同步精简（advanced_search 等细分规则随插件一起移除）；保留 tool-web 的 fetch。公开检索由 anysearch Skill 按需承担。处理搜索设置入口，避免死开关。
> 3. 日期不进系统提示词段：核实 time-context 插件实际注入位置，若进 system 段则改为运行上下文注入；cwd 保持放最后。
> 4. 目标：系统提示词 ≤1000 tokens（目标 200–300）。验收用阶段 0 脚本复测。

**阶段 2：Skill 按需加载（先验证，再改）**
> 1. 只读验证：合成会话中挂载一个测试 Skill，检查 DSH tool-skill 是否能让模型主动读取正文（证据：工具调用日志 + 会话产物）。
> 2. 验证通过：conversationPrompt.ts 不再注入 Skill 全文，改为 id/name/description/path 或 ≤300 字符摘要；每个动作最多一个主 Skill。
> 3. 验证不通过：保留全文注入，在 docs/decisions.md 记录缺口与复审触发条件（等 DSH 升级），不改坏现有行为。
> 4. 验收（若通过）：Skill 段 ≤300 字符；周报/复盘/知识整理产出契约不回归；Skill 来源（全局/Profile/动作）与同名冲突可见。
> 5. 执行结果（2026-11-17，ARCHITECT 复核通过）：验证通过并已实施。DSH tool-skill 返回全文（原生 skill_content 包装；PTC 下经 run_code 内 tools.skill），会话自带 2518 字符技能目录（19 个 Skill 的 name+description）。conversationPrompt.ts 全文注入改为 ≤260 字符指针，四动作 user 段降 61–72%。遗留既有验证脚本断言全文注入需随阶段 6 更新（verify_skill_bindings_082、verify_worklog_089、verify_skill_flow_injection_108）；skill-catalog 2518 字符是 DSH 目录注入非桌面注入，压目录需改 tool-skill 配置，超出本阶段范围，记缺口。真实模型行为归入阶段 6 smoke test。

**阶段 3：工具面与安全约束**
> 1. 按动作收敛工具白名单：搜索、Office、知识整理按需；读写、审批、副作用工具显式。
> 2. 核对 read/搜索/bash 的行数与字节上限（pi 基准：2000 行/50KB、grep 单行 500 字符）；可配置的落地，不可配置的记缺口；全量输出落临时文件。
> 3. 路径边界：cwd 显式绑定、允许根校验、写操作同文件串行、原子写、精确替换优先。
> 4. bash 安全：cwd、超时、进程树 kill、环境变量过滤、输出上限、审批保留；bash 只做后备。
> 5. 错误与脱敏：工具错误作为可恢复结果；工具结果进模型前脱敏；审批失败不升级为整轮失败。
> 6. 只读审计 DSH 是否提供 before/after 钩子、按工具串行、审批前置、turn 间热切换、abort 下发；有则配置，无则记缺口，不改 runtime/。
> 7. 交付：工具约束矩阵（每个工具的参数/输出/路径/并发/安全现状与缺口）。
> 8. 执行结果（2026-11-17，ARCHITECT 复核通过）：dsh-free-search 依赖声明已从 discipline.profile.json 与 profile_assets.rs 清除（cost-meter 保留）；工具面盘点结论为无可安全删除项，discipline-core 11 工具 / work-review-core 7 工具保持最小集合；tool-fs/tool-fs-search 显式钉住输出上限（read 2000 行/50KB、grep 单行 500 字节、glob 100 条、grep 250 条）；钩子/审批/串行/abort 审计完成，缺口（读不受 workspace 根约束、无统一脱敏、ask-user 无宿主卡片、无 turn 间热切换）记入 tool-matrix.md；cargo test 1187 全过、mock 四会话 succeeded。D-105 决策已落 docs/decisions.md。

**阶段 4：模型接入成熟度（只补差）**
> 1. 盘点 model_settings.rs 已有字段（provider/api/apiKeyEnv/reasoningEffort/contextWindow 已落）与 DSH 侧仍缺的能力项（成本/compat/模型列表预检），只补缺失项，不重写已有字段。
> 2. 保存/切换前可用性预检：模型列表 404、UnsupportedModel、认证失败分别给出可操作提示；预检不读明文凭证。
> 3. 模型目录缓存只做只读数据（目录/etag/checkedAt），刷新不阻塞启动。
> 4. 验收：模型不可用时在运行前给出明确原因；密钥不出现在 UI、日志、提示词或成果文件。
> 5. 执行结果（2026-11-17，ARCHITECT 复核通过）：Rust save() 前置档位校验（declared_efforts/check_effort_capability，6 单测）；设置页保存前端点预检（401/402/403 阻止，404/网络提示放行）+ 档位收敛（声明 false 自动回落默认）；切换路径 api.ts 用模型目录校验显式档位。cargo test 1193 全过、tsc 0。缺口如实记录：聊天期才暴露的 402/403 保存期发现不了；目录无 ETag/checkedAt 持久缓存（DSH 未暴露版本字段）；真实端点响应文本未验证（归阶段 6）。D-106 决策已落 docs/decisions.md。

**阶段 5：可观测性与配置解释**
> 1. 设置页或只读诊断入口显示：系统提示词分段与 token、工具与 Skill 可见性、配置来源、模型 compat 与 thinking 能力、压缩状态；只显示元数据。
> 2. 任务时间线显示模型往返、工具 prepare/execute、审批等待、压缩/重试、缓存命中；不显示用户正文。
> 3. 验收：用户能回答"这条行为从哪来、时间花在哪"，不需要读源码。
> 4. 执行结果（2026-11-17，ARCHITECT 复核通过）：新增只读诊断命令 diagnostics_metadata（Rust，复用会话解析思路，只出元数据不出正文）；设置页新增「配置解释」页：生效路由/api/thinking 能力/上下文窗口、提示词分段与来源、工具面清单、Skill 概况。tsc 0、npm run build 0、cargo test 1200 全过（diagnostics 7 新单测）。缺口：任务时间线逐轮耗时/审批等待/缓存命中未展示（明确不做范围）；真实窗口截图未完成（macOS 锁屏，黑图证据存 evidence），由用户验收时目视。D-107 已落 docs/decisions.md。

**阶段 6：验证、发布与回退**
> 1. npx tsc --noEmit；npm run build；cargo test --lib；现有 UI/Profile 验证脚本全过。
> 2. 隔离 Profile 的 --dump-config 与启动探针；确认 web-search-free 移除后平台启动、anysearch 可用、Skill 按需加载（若阶段 2 通过）可用。
> 3. 真实模型 smoke test（需用户当轮允许）：Excel、周报、知识整理各一次；记录提示词字符/token、工具往返、首字、总时长、缓存与压缩消耗。
> 4. 版本 0.2.30；提交推送；macOS/Windows 构建；STATUS/REVIEW 写清改前改后、证据与未验证项。
> 5. 回退：恢复 preset-standard、重新挂回 web-search-free、恢复原 persona 与（若改动）Skill 全文注入；保留旧配置备份。
>
> 6. 执行结果（2026-11-17，待 ARCHITECT 复核）：tsc 0、npm run build 0、cargo test --lib 单线程 1200/1200、cargo check 0、Tauri release `--no-bundle` 编译通过；54 个 UI/Profile 脚本中 41 通过、13 项既有失效/环境失败（HEAD 快照复现，逐项见 stage6.md）；三个 Skill 注入脚本已改为指针断言，082 58/58、089 85/85 通过，108 因 macOS 无 Xvfb 仅语法检查。版本与 CHANGELOG 已收口 0.2.30。播种迁移结论：现有安装不自动清理，需「修复缺失 / 恢复出厂内容」（写前备份）或重装；发布工作流重建 seed 后新装包生效。真实模型 smoke 因 402 未执行，Windows 未构建。提交推送与回退路径见 stage6.md。

**验收矩阵（0.2.30 必须可验证）**
> - 效率：系统提示词 ≤1000 tokens（目标 200–300）；Skill 段 ≤300 字符（若阶段 2 通过）；工具输出有上限或明确缺口；基线表能拆分 397 秒；缓存命中被记录。
> - 安全：凭证零读取/零打印/零前端存储；路径根与 cwd 显式；审批与 sandbox 保留；工具结果脱敏；无第三方代码/Extension 市场/npm 资源包。
> - 成熟：提示词/工具/Skill/配置来源可解释；模型不可用有预检；回退路径可用；现有任务与验证脚本无回归；未验证项如实列出。

**明确不做：** 不改 runtime/；不迁移线程/会话为 pi 的 JSONL session tree；不实现自有 compaction；不引入三级配置与 Project Trust 闸门；不引入 Extension 市场、第三方代码、npm 资源包；不重写模型注册表；不改 Tauri/React 通信协议；不新增 RPC/SDK/Web/TUI 宿主；不做 UI/交互重构（OD-UI-717 既有批准范围除外）；不内建子代理编排、MCP 市场、计划模式；不使用真实用户材料做测试。

**风险：** 移除 web-search-free 改变搜索入口；Skill 按需加载可能增加一次工具往返（阶段 2 先验证再决定）；DSH 钩子暴露程度未知（只读审计，有则用无则记）；提示词前缀变化可能影响缓存（阶段 0 基线对照）；模型预检可能因端点不提供列表而只能给出有限结论。每项在阶段 0 基线与阶段 6 回退中记录。

**派工方式：** 每个阶段作为一个独立任务，按 docs/BUILDER-Task-Instruction-Template.md 派给子代理（模型 OpenDesign/deepseek-v4.1-flash-eco，思考等级 high）；阶段 0 与阶段 2 的验证步骤可由子代理执行，但 ARCHITECT 在每阶段完成后独立复核源码 diff 与测试输出后才进入下一阶段；阶段 6 的真实模型 smoke test 与提交推送须用户当轮确认。

> 已完成的旧计划已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。
