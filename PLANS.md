# 当前实施计划

### OD-ARCH-174：Skill 优化——指针补全、目录瘦身、正文重写（待批准，2026-11-17）

> **触发：** 0.2.30 完成后用户要求优化 Skill。勘察证据：①真实模型 smoke（f1e06a3）确认知识整理任务不读 Skill，根因是 knowledge 场景在 rules.json 无 skill_bindings，模型没有"该用哪个技能"的指引；②每次任务注入 skill-catalog 2518 字符（10 技能），其中 4 个（anysearch/document-review/material-writing/knowledge-search）从不被模型自主调用，目录对它们是纯开销；③写材料/项目场景与周报共用默认分支的指针来源（presetSkillsFor(preset,"report")），边界情况可能拿错技能指针；④10 个 Skill 正文约 36KB，是全文注入时代写的冗长文风，按需读取时代可收紧。
>
> **用户裁决（2026-11-17）：** ①知识库场景只需要检索知识卡（绑定 knowledge-search），知识卡沉淀发生在任意场景（工作/写作/无分类），不是独立场景的职责；②阶段 C 必做。
>
> **派工：** 子代理 OpenDesign/deepseek-v4.1-flash-eco + high，按 BUILDER 模板逐阶段派发；每阶段 ARCHITECT 复核后进下一阶段。真实模型验证用现行可用 key。

**阶段 A：指针补全与绑定修正（低风险）**
> 1. 修 writing/project 场景指针来源：conversationPrompt.ts 默认分支（mode=report）不应无条件走 presetSkillsFor(preset, "report")；按当前场景取各自绑定（writing→material-writing、project→project-agent），周报场景维持 memory-report。改动后用 Node 直接调组装器验证四场景指针各归其位。
> 2. knowledge 场景补绑定：knowledgeContext 命中时（宿主已检出的检索上下文），在 knowledgeBlock 追加 knowledge-search 的技能指针（≤260 字符，含"两跳检索/先读索引.md"要点）；无命中不注入。不动 rules.json 的场景结构（knowledge 不是独立场景，是横切能力）。
> 3. 验证：verify_conversation_prompt / verify_skill_bindings_082 断言更新并全过；tsc 0。
> 4. 执行结果（2026-11-17，ARCHITECT 复核：代码与测试通过，发现注入点策略缺口，见第 5 条）：defaultSceneSkills 修正四场景指针串号；knowledgeSkillPointer 落地（151 字符）；tsc 0、verify_conversation_prompt 33/33、verify_skill_bindings_082 62/62、worklog_089 85/85、ai_draft 27/27。
> 5. **A2 追加（ARCHITECT 裁决）：** docs/decisions.md D-104 口径"knowledge-search 不进任何绑定，纯按需"的前提已变化——阶段 2 造出了指针注入点（260 字符不再是 4901 字正文）。裁决：新增专用绑定键 `knowledge`（知识检索流程），HomeCanvas 读取后作为独立参数传给 composeConversationPrompt（不混入场景级 skills）；出厂 discipline.rules.json 增加 `knowledge → knowledge-search` 绑定；同步更新 D-104 决策记录（注明复审触发：OD-ARCH-174 指针机制）。这样 knowledge 指针在默认 Profile 即生效，且不污染四场景主指针。
> 6. **A2 执行结果（2026-11-17，ARCHITECT 复核通过）：** knowledge 专用绑定键落地（rules.json + HomeCanvas 独立读取 + HomeChat 透传 + ConversationPreset.knowledgeSkills）；普通对话 preset=null 也注入知识指针；D-108 决策记录完整（含复审触发）。tsc 0；verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、cargo 1200 全过。已安装 Profile 的 rules.json 迁移待用户「恢复默认规则包」。

**阶段 B：目录瘦身（低风险）**
> 1. 对从不被模型自主调用的 4 个技能（anysearch/document-review/material-writing/knowledge-search）在 SKILL.md frontmatter 加 disable-model-invocation: true；保留 user-invocable。效果：模型自主目录不再列它们（估省约 400 字符），流程绑定与用户手点不受影响（绑定注入走 skill_contents，不走目录）。
> 2. 同步仓库 skills/ 与 sync-skills.sh 校验；mock 会话复测目录字符（预期 2518 → 约 2000）。
> 3. 验证：mock 前后对比落 evidence/od-arch-174/；相关 verify 脚本全过。
> 4. **B 中途裁决（ARCHITECT，基于阻断性发现）：** disable-model-invocation 同时关掉目录条目与 skill 工具执行（dsh-tool-skill execute 二次校验），与指针机制冲突。裁决按"能力优先、目录开销其次"收缩：material-writing、knowledge-search 撤回标记（它们的绑定指针要求模型调 skill 读正文）；anysearch 撤回标记（阶段 1 计划明确公开检索由模型按需走 anysearch，自主可见是能力的一部分）；只保留 document-review 的标记（纯用户手点场景，无绑定指针指向它，目录省约 110 字符）。目录 2518 → 约 2410，收益缩小但零能力损失；后续若 DSH 拆分"目录可见"与"可执行"两个开关，再回来收这 4 个。
> 5. **B 执行结果（2026-11-17）：** 4 标记实验证明 disable-model-invocation 使 skill 工具返回 "not available for model invocation"（阶段 A/A2 的指针会失效）；按裁决撤回 3 个、保留 document-review；mock 目录 2518 → 2410 字符；verify 33/33、69/69、86/86 全过。
> 6. **B 最终（复核通过）：** 实测目录 2518 → 2401 字符（−117）；仅 document-review 从模型目录消失；sync --check 一致；mock 三任务 succeeded。既有缺口记录：sync-skills.sh 在 macOS 需绕过 BSD realpath（未修）。
> 7. **C 执行结果（2026-11-17，ARCHITECT 复核通过，含真实模型端到端验证）：** 三技能正文压至原 63–70%（memory-agent 1754→1228、memory-report 1099→749、period-review 1820→1268），契约与硬约束逐字保留（082 69/69、089 86/86、503 58/58、505 71/71）。**真实模型端到端**（新驱动 run_custom_prompt.py 送桌面组装器产物）：模型收到指针后实际调用 skill(name=memory-report) 读到重写正文，随后按五段模板产出（本周概述/事项进展/问题与风险/下步计划/待确认事项全在）——"指针→按需读取→按契约输出"全链路真实模型确认。knowledge 任务的模型未触发 skill 读取属模型自主决策（宿主检索上下文已直接给出），非回归。
>
> **OD-ARCH-174 完成总结：** 四场景指针各归其位；knowledge 专用绑定键落地（D-108）；目录 2518→2401；三核心技能正文 −30~37%；真实模型全链路验证通过。已知缺口：sync-skills.sh macOS realpath 兼容（未修）；已安装 Profile 的 rules.json 需「恢复默认规则包」获得 knowledge 绑定；阶段 C 其余 7 个技能未重写（anysearch/document-review 等按需技能留待后续按使用频率决定）。

**阶段 C：核心技能正文重写（中风险，用户必做项）**
> 1. 重写 memory-agent / memory-report / period-review 三个核心技能正文：删除全文注入时代的冗余（重复平台契约的写盘边界说明、解释"平台会替你做什么"的段落、与 conversationPrompt 指针重复的读取指引），保留输出契约（五段模板/判定结构/标题深度）与硬约束（引用编号/不虚构）。
> 2. 重写原则：模型通过 skill 工具读全文时，读到的是"纯任务指令"；平台契约（写盘边界、宿主计算）只在指针与平台提示词出现一次，不在技能正文重复。
> 3. 每个技能重写后：先 diff 说明删了什么留了什么，再跑对应 verify 脚本（082/503/505）确认输出契约断言不变；三个全过才继续下一个。
> 4. 真实模型回归（现行 key）：随手记/周报/复盘各 1 次，对照 0.2.30 smoke 的产出结构；结果落 evidence/od-arch-174/。
>
> **明确不做：** 不动 knowledge-agent（沉淀型技能留给用户手点场景，本轮不改正文）；不改 DSH tool-skill 的目录机制（disable-model-invocation 是既有配置项）；不改 runtime/；不动凭证；知识卡"任意场景沉淀"的流程不变（卡片落盘仍由 knowledge-search 的建卡路径 + 宿主执行）。
>
> **验收：** A/B/C 每阶段 mock+脚本证据齐全；C 结束后真实模型三类任务产出契约与 0.2.30 smoke 一致；目录与注入总量显著下降；全部验证脚本通过或既有失效如实记录。

### OD-ARCH-173（v3，最终版）：0.2.30 成熟、高效、安全基线（已完成并通过验收，2026-11-17；release 提交 1d07d91bba，验收记录见 REVIEW.md）

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
