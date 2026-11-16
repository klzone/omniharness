# Pi Agent 设计笔记（第 1–32 章）

> 来源：https://zhanghandong.github.io/pi-book/ 的公开章节（2026-11-16 抓取）。
> 用途：作为 OmniDesk 0.2.30 架构与提示词优化的学习底稿；只记录设计判断与可借鉴/不可借鉴项，不复述源码细节。
> 口径：笔记中的“pi”指书里描述的 pi-mono agent 运行时，不是某个具体客户端。

## 序章 / 前言：这本书是什么，不是什么

**这本书是设计决策记录，不是源码导读，也不是 API 文档。** 每章回答一个“为什么这样设计”的问题，源码只在需要解释取舍时出现；它明确不做框架推荐，也不教 prompt engineering。

**读者与读法**：面向有工程经验、想理解 agent 系统设计的开发者；提供三种路径——快速扫描（1/2/3 章 + 30–32 章）、设计理解（按章读、跳过代码块、重点看“设计问题”和“取舍分析”）、深入实现（配合源码与行号）。

**对我的约束**：学习的是判断方法——每个决策得到了什么、放弃了什么、适不适合 OmniDesk 的场景；不是把 pi 当模板照搬。序章本身已经说明“本书不讲怎么用 pi、不讲 prompt engineering、不做框架推荐”，所以不能把它的某段提示词当成 OmniDesk 的标准答案。


## 第 1 章 不是又一个 LLM 包装器

**核心判断**：pi 是 agent runtime，不是调用库。模型返回后“继续、重试、被打断、上下文满、工具失败、多工具排序”这些问题，一次 API 调用解决不了，必须有一个持续运转的循环。

**结构**：四层——pi-ai（统一调用面）、pi-agent-core（循环与状态）、pi-coding-agent（产品内核）、产品壳。控制流由 LLM 驱动，不由开发者预编排。

**取舍**：运行时比链式框架灵活，但更难预测、更难调试；单 agent + 强工具集优先于多 agent 编排。

**对 OmniDesk**：DSH 已经承担 runtime 的职责，OmniDesk 不应再长出一套循环或工作流引擎。我们的价值在产品壳：记录、复盘、任务、知识、文件边界、审批和界面。**借**：把 DSH 当内核；**不借**：再包一层 agent 框架或 chain。

## 第 2 章 包不是项目

**核心判断**：用 npm 包做硬边界，让分层从“团队约定”变成“编译期约束”。包不是独立项目，是同一系统的层与配件。

**机制**：7 个 workspace 包，lockstep 版本；每个包独立测试、独立导出；渐进式采用（只用 pi-ai、只用 agent-core 都可以）。

**取舍**：得到强制分层、可测试、可 tree-shake；放弃开发环境简单性和发布原子性。

**对 OmniDesk**：我们不是 monorepo，但要有同样的硬边界：desktop（宿主与 UI）不得 import DSH 私有实现，engine（适配层）不得 import UI，runtime 只读。**借**：边界用接口/目录而不是口头约定；**不借**：为了“统一”把 runtime 逻辑复制进产品层。

## 第 3 章 怎样高效阅读这个仓库

**核心判断**：先读内核和类型骨架，再看产品层；不要试图一次读完所有 provider。

**要点**：types.ts 是系统骨架，每个字段对应一个设计决策；Extension 是代码，Skill 是指令文档，二者不是包含关系；自动生成的模型目录只需理解“为什么存在”，不必读内容。

**对 OmniDesk**：研究 DSH 时先读 types.ts、AgentLoopConfig、事件类型和工具接口，再看具体 provider/插件。**借**：以类型和契约为入口；**不借**：按文件数量或目录大小判断重要性。

## 第 4 章 Provider 不是 Adapter

**核心判断**：Provider 是运行时单元，不是注册表条目；认证、模型目录、动态刷新都内聚在 provider 自身。Models 是显式集合，不是全局单例。

**机制**：Model<TApi> 携带 provider/api/baseUrl/auth 等上下文；createProvider 装配；Models 负责路由与 auth 应用；根入口只保留 core，旧全局 API 迁到 compat。

**取舍**：得到无全局可变状态、可测试、可 tree-shake、provider 内聚；放弃一点装配样板和迁移成本。

**对 OmniDesk**：模型设置、凭据引用、模型目录应属于 provider 运行时，而不是散落在 UI 状态或全局变量里。**借**：显式装配、provider 内聚、模型能力随 provider 走；**不借**：为了少写代码重新引入全局单例或隐式注册。

## 第 5 章 消息变换：跨模型交接

**核心判断**：跨 provider 切换不是换 key，而是有损但安全的转录变换；宁可降级 thinking/tool 元数据，也不能让目标 API 拒绝历史。

**机制**：null content 归一化；同模型保留、跨模型降级；thinking 块降级为普通文本；tool call id 归一化；第二遍扫描补齐缺失的 tool result；跳过错误/中止消息。isSameModel 要求 provider + api + model 三者全同。

**取舍**：得到随时切模型、确定性变换、不中断；放弃 thinking 语义保真、合成 tool result 是假数据、严格判断导致同厂换模型也降级。

**对 OmniDesk**：模型切换要当成正式流程，记录“此轮从哪个模型切到哪个模型、哪些元数据被降级”。**借**：可用性优先的有损变换、工具结果必须补齐；**不借**：为了让历史“看起来完整”伪造工具结果却不在 UI 标注。我们的失败卡已经标注未复核，这个方向要保留。

## 第 6 章 统一事件流

**核心判断**：LLM 调用的返回值不是 Promise，而是事件流；成功和失败走同一条事件路径。

**机制**：StreamFunction 契约；EventStream 用 queue/waiting consumer；事件类型覆盖生命周期、文本、thinking、tool call、终止；partial 字段让消费者随时拿到完整快照；must-not-throw 契约；StreamOptions 承载 transport、onPayload、onResponse、timeout、retry、cacheRetention 等旋钮，事件协议保持稳定。

**取舍**：得到录制/回放、统一错误、生产者消费者解耦、流式非流式统一、渐进消费；放弃消费者要写状态机、调试更难、partial 内存开销、must-not-throw 靠自觉。

**对 OmniDesk**：我们的时间线、失败卡、复制/朗读都依赖事件序列，必须保证“事件可回放”和“错误也在事件里”，不要另造一套错误通道。**借**：partial 快照、统一错误事件、稳定契约 + 增长旋钮；**不借**：每个 UI 组件各自维护累积状态，应该由适配层收敛成快照。

## 第 7 章 认证是一等子系统

**核心判断**：认证不是几个 OAuth 工具函数，而是与 provider 对等的能力；API key、OAuth、环境变量、云 profile 都要收敛到一个解析入口。

**机制**：ProviderAuth 的两条腿 resolve/getAuth；OAuthAuth 的 refresh/toAuth 切分；CredentialStore.modify 是唯一写路径，锁下双检过期刷新；交互收敛为 prompt/notify；PKCE 不允许 CLI 持有 client secret；错误保留底层 cause 但有可分类 code。

**取舍**：得到凭证来源统一、并发安全、认证随 provider 内聚、接口可增长；放弃一次破坏性迁移、ai 层职责扩大、交互联合需要宿主处理所有分支、凭证落盘仍在产品层。

**对 OmniDesk**：凭证永远只做“引用名 + 运行时解析”，UI 不读明文；写入走单一通道，避免多处写导致串号；错误要把底层 cause 带出来（我们刚做的 turn/end 错误透传就是同一方向）。**借**：唯一写路径、provider-scoped 认证、错误保留 cause；**不借**：把 key 存进模型配置文件或前端状态。

## 第 8 章 agentLoop：发动机只管转

**核心判断**：循环引擎应该知道尽可能少的东西。它只负责“把消息送进 LLM、拿回响应、有工具调用就执行、决定是否继续”；它不知道会话从哪来、存到哪去、上下文快满、该不该重试。循环内部全程使用 AgentMessage，只在调用 LLM 的边界转换成 LLM Message。

**双层循环**：
- 内层负责“持续工作”：调用 LLM → 执行工具 → 检查 steering 消息 → 再调用 LLM；只要还有工具调用或 steering 消息就继续。
- 外层负责“被唤醒”：内层要停时检查 follow-up 消息；有就设为 pending，重新进入内层。
- Steering 是用户在执行中插入的转向指令，在工具执行后注入；follow-up 是任务完成后的追加任务，只在 agent 本来要退出时消费。两种消息共用 pendingMessages 通道，但消费时机不同。

**关键机制**：
- 错误通过 stopReason 传递，不抛异常；循环只检查 error/aborted，不需要 try-catch。
- 续轮条件不是“有没有 tool call”，而是工具批次的 terminate：只有整批结果都标记 terminate 才停止；单个工具可以主动叫停。
- turn 之间有两个可选回调：prepareNextTurn 可热切换 model/context/thinkingLevel；shouldStopAfterTurn 可优雅停止；abort signal 会下发到这些回调。
- 消息变换分两步：transformContext 在 AgentMessage 层裁剪/注入，convertToLlm 过滤自定义消息并转成 LLM 格式；转换只在 LLM 边界发生。
- streamFn 是唯一与 provider 的耦合点；底层 runLoop 要求必填，公共入口可回退到宿主注入的默认 streamFn，agent 包不 import provider 目录。
- AgentLoopConfig 的字段全是可选能力：convertToLlm 必填，其余不提供就不启用；消息变换回调有 must-not-throw 契约，before/afterToolCall 例外，循环会防御性捕获。
- 事件是唯一输出通道；agent_start/turn_start/message_*/tool_execution_*/turn_end/agent_end 足以重建完整执行过程，支持录制与回放。

**取舍**：得到极致可组合、可测试、关注点分离；放弃会话持久化、自动重试、自动压缩，并要求上层正确组装配置。循环做薄，是为了上层做厚时有自由度。

**对 OmniDesk**：DSH 对应这一层；我们不应该在 UI 或 adapter 里再造循环、重试或压缩。**借**：事件唯一输出、stopReason 错误模型、turn 间热切换、steering/follow-up 的时机区分；**不借**：把 DSH 不提供的 turn 回调假设成存在。需要核对 DSH 是否暴露等价的 turn 间钩子与 abort 下发。

## 第 9 章 工具执行不是插件调用

**核心判断**：工具执行是 prepare → execute → finalize 三段管道，不是“找到工具、调一下”。错误在 prepare 阶段变成可恢复的工具结果，执行阶段允许抛异常，finalize 阶段做审计与修改。

**Prepare**：查找工具、prepareArguments 兼容旧参数、TypeBox 运行时校验、beforeToolCall；任何失败返回 immediate 错误结果，循环不中断。参数错误回到模型，模型下一轮可修正。
**Execute**：带 AbortSignal、onUpdate 部分结果；工具允许抛异常，循环捕获为 isError 结果。
**Finalize**：afterToolCall 字段级覆盖 content/details/isError/terminate；可做审计、脱敏、错误降级、结果增强；工具结果可带 addedToolNames，钉在转录位置解锁后续工具。

**并发语义**：默认 parallel，但 prepare 严格按源顺序串行（权限检查确定性、事件顺序）；execute/finalize 并发，tool_execution_end 按完成顺序到 UI；写回上下文的 ToolResultMessage 按源顺序，保证可重放。单个工具声明 executionMode: sequential 会让整批降级串行。并行工具之间不能有依赖，依赖要靠模型分多轮。

**终止**：工具或 afterToolCall 可设置 terminate；只有整批结果都 terminate 才停止续轮。

**取舍**：得到可观测/可控钩子、错误可恢复、安全与性能兼顾；放弃无钩子时的零开销、钩子异常会伪装成工具失败、并行工具不能共享中间状态。

**对 OmniDesk**：工具错误必须作为可恢复结果回到模型；读并行、写盘/审批/副作用串行；UI 按完成顺序展示，上下文/存储按源顺序；工具结果进模型前脱敏；addedToolNames 式按需解锁工具是方向，但需核对 DSH 是否支持。不要把“审批失败”直接升级成整轮失败。

## 第 10 章 Agent：循环之上的有状态壳

**核心判断**：无状态循环负责“转”，有状态 Agent 负责“记住和协调”。Agent 的状态修改只走 processEvents 一个入口；它只管运行时状态，不管会话存储、压缩、认证、模型选择、提示词拼接和工具注册。

**状态**：MutableAgentState 持有 systemPrompt/model/thinkingLevel/tools/messages/isStreaming/streamingMessage/pendingToolCalls/errorMessage；tools 与 messages 用 getter/setter，赋值时 copy，避免循环引擎改到 Agent 状态；运行时字段对外 readonly，只有 processEvents 能改。

**事件与订阅**：listeners 注册顺序串行 await，保证状态归约和通知顺序一致；listener 收到 AbortSignal，可跳过耗时持久化；agent_end 不等于 idle，waitForIdle 要等所有 listener 处理完 agent_end。

**消息队列**：steeringQueue 与 followUpQueue 分离；每个队列支持 all 与 one-at-a-time（默认），避免三条 steering 同时砸给模型；队列通过 getSteeringMessages/getFollowUpMessages 回调交给循环。

**中止与互斥**：每次 prompt/continue 创建新的 AbortController，signal 传给循环、所有 listener 和工具；abort 一次中止整条链。activeRun 存在时拒绝新 prompt，错误信息直接告诉调用者用 steer/followUp 或等待。

**状态归约**：processEvents 先按事件更新 state，再通知 listener；listener 看到一致状态。pendingToolCalls 用新 Set 赋值，保持 ReadonlySet 不可变语义。

**扩展点**：CustomAgentMessages 是空接口，应用通过声明合并扩展；AgentMessage = LLM Message + 自定义消息；自定义消息在循环内部一等公民，convertToLlm 出门时过滤。这比 any 或泛型更保类型且不增加传参负担。

**Agent 不管什么**：会话持久化、UI、认证、模型选择、压缩、system prompt 拼接、工具注册全部委托给上层。AgentHarness 是另一条“厚壳”路径，把会话/压缩/Skills/ExecutionEnv/重试/models 收进 agent 包；截至 v0.82.1 生产 coding-agent 仍用薄壳 Agent，AgentHarness 只在测试/docs 中使用。

**取舍**：得到清晰职责、可预测状态、多消费者订阅；放弃胖接口、listener 串行拖慢 UI、单线程不能后台常驻。

**对 OmniDesk**：宿主已经承担“厚壳”职责（线程、任务台账、队列、持久化）。不要再在 UI 里维护第二套会话状态；状态修改只走一条路径；订阅者按顺序处理；abort 要贯穿循环、工具和监听器；AgentHarness 是实验路径，不照搬进产品。


## 第 11 章 会话树

**核心判断**：coding agent 的会话不是线性聊天记录。用户会回退、分支、重试、从某轮继续；线性列表要么不能回溯，要么复制整段历史。pi 用 JSONL + parentId 构成一棵树，分支只是“同一父节点下的新子节点”。

**机制**：
- SessionHeader 记录 version/id/timestamp/cwd/parentSession；每个 entry 有短 id（8 位十六进制）和 parentId，null 为根。
- 会话 id 用 UUIDv7：时间有序，便于 provider 按 session 路由和 prompt cache 命中；entry id 短小，便于人读和分支导航。
- 9 种 entry 分三层：核心层影响 LLM context（message、compaction、branch_summary）；元数据层恢复状态（thinking_level_change、model_change、session_info）；扩展层（custom、custom_message、label）。
- buildSessionContext 从叶子沿 parentId 回溯到根，提取最后生效的 thinking/model；有 compaction 时输出“摘要 + firstKeptEntryId 之后的保留消息 + 最新消息”。
- 分支命令 /tree、/fork、/clone；RPC 提供只读 get_entries/get_tree；孤儿 parentId 当作根，防御损坏。
- 版本迁移 v1 线性 → v2 树 → v3。

**取舍**：得到 append-only 容错、人类可读可修复、零依赖、天然分支；放弃随机访问、无索引、文件持续增长、首次全量加载、单进程写入。

**对 OmniDesk**：线程/会话已有自己的存储与产品语义，0.2.30 不改格式、不迁移成 pi 的 session tree。**借**：从某轮继续/分支时只追加节点，不复制历史；上下文按路径构建；**不借**：为了“时间旅行”引入数据库、索引或跨会话查询。

## 第 12 章 Compaction

**核心判断**：compaction 不是简单截断，也不是 runtime 内建；它是产品层策略：用 LLM 摘要旧对话，同时保留文件操作记忆，并且必须承认有损。

**机制**：
- 触发：每次 assistant 响应后估算 context；优先用最近 usage 的真实 token，尾部消息用 chars/4 启发式（图片按 1200 token）；context > window - reserveTokens（默认 16K）时触发。
- 保留：keepRecentTokens 默认 20K；从最新 entry 往回累积，找到合法切点；永远不在 toolResult 处切，否则拆散 tool call/result；切在 turn 中间时做 turn splitting，生成 prefix 摘要并保留后半段。
- 文件操作：extractFileOperations 扫描 read/edit/write/bash，继承上次压缩的 details（fromHook 的压缩不继承），计算 readFiles / modifiedFiles，以 XML 追加到摘要。
- 摘要：把对话序列化成纯文本（tool result 截断 2000 字符）放进 <conversation>，用专门的 summarization system prompt，要求六个章节：Goal、Constraints & Preferences、Progress（Done/In Progress/Blocked）、Key Decisions、Next Steps、Critical Context；有上次摘要时用 update prompt 增量更新，避免“摘要的摘要”。
- 写入 CompactionEntry：summary、firstKeptEntryId、tokensBefore、details、fromHook；重复压缩从上一次保留边界起算，写入前重算 tokensBefore。
- 产品层：prepareCompaction 做 CPU 计算（找切点、提取文件、整理消息），compact 做 I/O（LLM 摘要）；extension 可在两步之间接管。
- 可恢复：摘要请求走 RetryPolicy，暴露 summarization_retry_* 事件；usage 计入会话；摘要请求隔离路由并禁用 prompt caching（新 sessionId + cacheRetention none）。

**取舍**：得到理论无限对话、文件记忆、可定制、增量更新；放弃有损、额外 token、不可逆（原始 entry 仍在 JSONL，但 LLM 看不到）。

**对 OmniDesk**：compaction 由 DSH/产品层负责，0.2.30 不在 UI 里实现自己的压缩，也不假装压缩后仍记得所有细节。**借**：结构化摘要、文件操作记忆、增量更新、prepare/compact 分离；**不借**：把压缩当成无损或隐藏压缩事件。基线必须把 compaction/重试的 token 和时间算进去，否则速度账会失真。

## 第 13 章 三级配置覆盖

**核心判断**：同一工具要适配不同项目，需要全局/项目/目录三级配置；但不同规则类型的合并语义不同，必须让用户知道“当前配置从哪来”。

**机制**：
- settings 深度合并：原始值覆盖、数组完全替换、嵌套对象递归合并；默认值在 getter 中硬编码，文件只存用户显式设置的字段；保存加文件锁，只写修改字段；reload 热加载。
- AGENTS.md 是拼接：全局在前，目录树从远到近，最后当前目录；同一目录 AGENTS.md 优先于 CLAUDE.md；最终注入 <project_context>。
- SYSTEM.md 是替换：项目优先于全局；APPEND_SYSTEM.md 是追加。
- PackageSource 支持字符串或对象，可只启用包里的部分 extensions/skills/prompts/themes。
- 运行时覆盖：CLI 参数 > 项目 settings > 全局 settings > 默认值；pi config -l 显式选择写全局还是项目层。
- Project Trust：项目本地 settings/resources/extensions/packages/skills 默认不无条件加载；defaultProjectTrust 为 ask/always/never，--approve 一次性授权，/trust 会话内切换；信任结果存全局 trust.json，不能放在项目内。
- CONFIG_DIR_NAME 可配置，用于 rebrand。

**取舍**：得到零配置、渐进定制、团队共享、并发安全；放弃配置来源的可解释性、不同合并语义的心智负担、没有 dry run/explain、信任闸门的摩擦。

**对 OmniDesk**：我们的 Profile/模型设置已有分层雏形，但缺少“当前生效配置来自哪一层”的解释入口。0.2.30 不改配置格式、不引入三级目录或 Project Trust 闸门；**借**：默认值、只写修改字段、文件锁、有效配置诊断；**不借**：多层合并却不给用户解释。

## 第 14 章 System Prompt 装配

**核心判断**：system prompt 不是手写字符串，而是默认/自定义底座 + 追加 + 项目上下文 + Skills 元数据 + 工作目录的装配流程；装配函数是纯函数，不做 I/O。

**机制**：
- 默认底座：工具列表只为“可见且提供 toolSnippet”的工具生成；Guidelines 根据工具动态生成并去重，不再教模型在 bash/grep 之间选择；只在“仅有 bash、没有专用探索工具”时补一句兜底。
- appendSystemPrompt 叠加；context files 用 XML 包裹，每个文件带 path；Skills 只注入 name/description/location，正文按需 read，且只有 read 工具可用时才注入。
- 尾部只放 Current working directory；日期被移除以稳定 prompt cache 前缀。
- 自定义 SYSTEM.md 路径：底座完全替换，但 context files、Skills、cwd 仍自动追加；代价是失去默认的工具列表和 guidelines。
- context files 在 session 启动时加载一次，reload 才重新发现；不是每轮扫盘。
- 装配顺序隐含优先级：越靠后的内容越受近因影响，AGENTS.md 的项目规则通常比默认底座更容易被遵守。

**取舍**：得到无限可定制、随环境变化、关注点分离、纯函数可测试；放弃最终形态难预测、prompt 可能很长、拼接顺序隐含优先级、自定义底座失去工具指引。

**对 OmniDesk**：0.2.30 的核心依据。当前 5404 字符里 5281 是插件/bundle 注入，产品人设只有 120 字符；应改为短底座 + 按需 Skill + 稳定前缀，并增加“当前提示词来源”诊断。**借**：分层装配、XML 边界、技能元数据、日期移出、cwd 最后；**不借**：把插件说明塞进常驻底座、把 AGENTS.md 注入用户产品。


## 第 15 章 Extension 系统

**核心判断**：几乎所有产品级能力都外置；核心只做调模型、跑循环、管状态。Extension 是代码模块，能加工具、命令、快捷键、UI、Provider，但不能改循环、状态归约和消息变换管道。

**能力面**：
- 事件订阅：观察型（session_start/message_update 等）与干预型（tool_call/input/context 可 block/transform）。事件数量持续增长，新增 before_provider_headers、agent_settled、session_info_changed。
- 工具注册：ToolDefinition 带 TypeBox schema、promptSnippet、promptGuidelines、execute、renderCall/renderResult；工具选择改为字符串名字白名单 + cwd 工厂；cache-friendly 动态工具加载可保留 prompt 前缀。
- 命令/快捷键/flag：命令 handler 可做会话级操作（newSession/fork/navigateTree），事件 handler 不行；会话替换后旧 ctx 失效，必须用 withSession 回调。
- 消息与持久化：sendMessage（可选是否触发回复）、sendUserMessage（总是触发）、appendEntry（只持久化不进 LLM context）；deliverAs 控制 steer/followUp。
- Provider 注册：可注册完整 provider（认证、模型刷新、过滤、自定义流），不只是改模型定义。
- UI 上下文：select/confirm/input 阻塞式对话，setWidget/setFooter/setHeader/setEditorComponent 改 UI 结构。

**生命周期**：Loader 串行加载、jiti 动态编译、Virtual Modules 提供静态依赖；Runtime 两阶段初始化，setup 阶段 action 方法是 throwing stubs，bindCore 后才注入真实实现；启动时在 system prompt 装配前加载；/reload 重新执行整个加载流程；没有 teardown，用 session_shutdown 清理；闭包状态在 reload 后会丢，需要 appendEntry/session_start 恢复。

**不能做什么**：不能改 runLoop、不能直接改 Agent 状态、不能拦截 transformContext/convertToLlm、session entries 只读（只能追加）。没有沙箱，坏 extension 可 crash 进程。

**取舍**：得到开放但有边界、类型安全；放弃深度定制、进程隔离、两阶段学习成本。

**对 OmniDesk**：DSH 插件对应 Extension。**借**：能力外置但锁核心、类型化事件、工具/Provider 注册边界；**不借**：无沙箱第三方代码、插件市场、让插件改核心循环。0.2.30 不引入新的扩展市场或第三方执行。

## 第 16 章 Skill 机制

**核心判断**：大多数能力扩展是“告诉 LLM 怎么做”，不是“写代码替 LLM 做”。Skill 是带 frontmatter 的 Markdown，六个字段，没有 execute；全部运行时能力就是被 LLM 读取。

**机制**：
- frontmatter 只关心 name、description、disable-model-invocation；正文不自动进 prompt，只有 description 匹配时模型用 read 读取全文。
- disable-model-invocation 让 skill 不出现在 available_skills，只能 /skill:name 显式触发；显式触发用 XML wrapper 包裹指令与用户消息，导出时剥离。
- 发现规则：目录含 SKILL.md 即 skill 包且不再递归；根目录 .md 作为独立 skill；递归子目录找 SKILL.md；尊重 .gitignore/.ignore/.fdignore。
- 名称验证：小写字母数字连字符、≤64、不能首尾连字符或连续连字符；不一致只 warning；symlink 按 realpath 去重。
- 优先级：全局先加载、项目后加载；同名冲突先加载者赢，产生 collision diagnostic，不静默。
- formatSkillsForPrompt：只注入 name/description/location，XML 包裹；只有 read 工具可用时才注入。

**Skill vs MCP vs Extension**：Skill 纯文本、零运行时、依赖模型遵循；MCP 独立进程、可访问外部系统；Extension 同进程代码、可改 UI/工具/Provider、无沙箱。三者互补，不是竞争。

**取舍**：得到零依赖、可审计、版本友好、低创建成本；放弃运行时能力、效果依赖模型、发现机制简单、全局优先可能覆盖项目。

**对 OmniDesk**：知识整理、周报、质检适合 Skill；需要读文件/写盘/检索的由宿主工具做。**借**：只给元数据、正文按需读、collision 诊断、显式触发；**不借**：把 Skill 当权限控制、把全文塞进每轮提示词、让 Skill 承担运行时能力。0.2.30 的 Skill 注入改造直接依据这一章。

## 第 17 章 Resource Loader

**核心判断**：四种外部资源（extensions、skills、prompts、themes）走同一套发现、覆盖、诊断流程；单资源失败不阻塞启动。

**机制**：
- 来源：全局 → 项目 → npm 包 → CLI 额外路径；npm 包最后加载，显式安装优先于隐式全局目录。
- 每种资源返回 diagnostics；冲突不静默，记录 ResourceDiagnostic；extension 冲突也加载，后加载覆盖先加载。
- override 机制：extensionsOverride/skillsOverride/promptsOverride/themesOverride/systemPromptOverride，方便测试与产品壳定制。
- reload 顺序：重载 settings → 解析包路径 → 加载 extensions（执行代码）→ 检测冲突 → 加载 skills/prompts/themes → 加载 AGENTS.md → 解析 system prompt。
- AGENTS.md 沿目录树向上搜索，全局在前、祖先从远到近，全部拼接；同名目录跳过，避免 EISDIR。
- cwd 必须显式传入，去掉 process.cwd() 兜底，支持同进程多工作目录。
- Project Trust 在全局与项目资源之间插入闸门；项目本地资源不再无条件加载。

**取舍**：得到统一心智、渐进降级、可测试；放弃资源类型差异被抹平、加载顺序不够透明。

**对 OmniDesk**：Profile 资产、Skill、提示词应该有统一入口和冲突诊断；不要每个功能各自扫目录。**借**：诊断、override、显式 cwd、来源可解释；**不借**：npm 包资源市场、Project Trust 闸门（0.2.30 不引入）、把资源加载顺序藏起来。

## 第 18 章 Model Registry

**核心判断**：选模型不是选一个 id，而是 provider + api + baseUrl + 成本/上下文/能力/compat/thinkingLevelMap。模型目录构建时生成，运行时动态刷新，models.json 做渐进覆盖。

**机制**：
- Model 字段：id/name/api/provider/baseUrl/cost/contextWindow/maxTokens/input/reasoning/thinkingLevelMap；thinkingLevelMap 把统一档位映射到 provider 值，null 表示不支持，越界用 clampThinkingLevel 收敛。
- 内建目录构建时生成，按 provider 拆包，可 tree-shake；Models 运行时按 model.api 分派。
- 动态目录：ModelsStore 按 provider 存 models/lastModified/checkedAt/etag；条件刷新用 If-None-Match，304 复用缓存；Models.refresh({force}) 强制拉取；per-provider 错误/取消隔离。
- models.json：除 id 外全 optional，可只写最小定义；modelOverrides 支持 partial merge（尤其 cost/compat）。
- ModelRuntime 是异步 Models 门面（读盘、网络、认证、动态目录）；ModelRegistry 降级为面向 extension 的同步 compat 投影。
- compat 层编码“OpenAI 兼容”的光谱差异：store、developer role、reasoning effort、maxTokensField、thinkingFormat、OpenRouter routing 等。
- 图像生成目录与文本同构，但返回 Promise 而非事件流，因为没有有意义的增量。

**取舍**：得到动态性、构建时确定性、渐进覆盖；放弃运行时才知道模型是否可用、compat 维护成本持续增长。

**对 OmniDesk**：模型条目必须带 provider/api/baseUrl/能力/compat/thinking 档位；保存前做可用性预检，不能只存 id。我们遇到的方舟 Agent Plan 404/UnsupportedModel 就是缺少这层。**借**：model = provider+api+params、thinkingLevelMap、compat、etag 条件刷新、最小配置默认值；**不借**：0.2.30 重写模型注册表/路由、引入 npm 模型包或新 provider 市场。


## 第 19 章 工具设计原则：约束即保护

**核心判断**：给 LLM 的接口越受约束，犯错越少。pi 不是“只给一个 bash”，而是 6 个结构化工具（read/write/edit/find/grep/ls）+ bash 后备；工具选择由“哪些工具被激活”表达，不再靠提示词教模型挑工具。

**机制**：
- 每个工具用 TypeBox schema 做运行时参数校验；prepareArguments 处理旧参数兼容；错误作为工具结果返回，模型下一轮可修正。
- 受约束采样（constrainedSampling）：json_schema 的 prefer/require 或 grammar（Lark/regex），由 provider 能力位 supportsStrictTools / supportsGrammarTools 把关；不支持时退回普通 tool call。TypeBox 是事后校验，受约束采样是事前保证。
- 工具激活是名字白名单 + cwd 工厂（createTool(name, cwd)）；工具不再隐式读 process.cwd()，同一进程可并存多个工作目录。
- 统一截断 truncate.ts：2000 行 / 50KB，先触发的生效；read/find/grep 用 truncateHead（保留开头），bash 用 truncateTail（保留结尾）；截断带元信息，不产生半行。
- 每个工具只暴露最小 Operations 接口（readFile/access/writeFile/exec/glob 等），可替换为本地、SSH、Docker、远程 API。

**取舍**：得到更低出错率、可预测 token、跨环境一致；放弃工具选择负担和部分灵活性。高频且易错的操作给专用工具，其余才留给 bash。

**对 OmniDesk**：这不是“工具越多越好”，而是“把易错操作从自由文本里拆出来”。我们的 Excel、知识库、记录写入、文件搜索都应该有宿主可校验的结构化入口；bash/pwsh 只做后备，并必须带 cwd 绑定、超时、输出上限和审批。工具面按动作激活，不在 persona 里写“先用什么工具”。

## 第 20 章 edit：精确替换与并发安全

**核心判断**：不让模型“读出来、改、整文件写回”，因为大文件容易漏内容、改缩进、加空行；edit 用 oldText → newText 的精确替换，oldText 必须唯一、多个 edit 不能重叠、所有 edit 基于原始文件匹配。

**机制**：
- 执行流水线：resolveToCwd → readFile → stripBom → detectLineEnding → normalizeToLF → applyEdits → diff → restoreLineEndings → writeFile。
- fuzzyFindText：精确优先，失败后用 NFKC、去行尾空格、smart quotes/破折号/特殊空格归一化再匹配；模糊匹配只改命中的行块，不整文件重写。
- applyEdits 先验证 oldText 非空、唯一、按位置排序、检测重叠，再从后往前替换，保证前面偏移不变。
- file-mutation-queue 用 Promise 链按 realpath 串行化同一文件的写操作；不同文件仍并行；39 行实现，无锁、自动清理。
- 结果同时给带行号的展示 diff 和可 git apply 的 unified patch。

**取舍**：得到几乎消除写错文件、并发正确、跨平台行尾一致；放弃行号式编辑的表达自由，大范围重构不如 write。

**对 OmniDesk**：知识库、记录、成果文件的修改应尽量走“精确替换 + 宿主校验 + 原子写 + 同文件串行”，不要让模型整文重写；改动前给 diff/摘要，改动后校验唯一匹配和落盘结果。我们的 knowledge_sync 已有冲突检查，方向对，但要补“同文件串行”和“精确替换优先”。

## 第 21 章 read：给模型看文件时的保护

**核心判断**：read 不是 cat。它用 offset/limit、行数与字节双重截断、续读提示、图片缩放和可插拔后端，保护 context 和隐私边界。

**机制**：
- schema 只有 path/offset/limit；offset 从 1 开始；截断同时受 2000 行和 50KB 限制，先触发者生效。
- TruncationResult 带总行数/总字节/输出行数/触发原因；截断后明确告诉模型“还有更多，用 offset 继续”。
- firstLineExceedsLimit：第一行就超 50KB 时不返回半截内容，而是提示改用 bash 的 sed/head -c。
- 图片自动检测 MIME，转 base64，默认缩放到 2000×2000 内；缩放失败优雅降级为文本说明，不让 tool call 失败。
- PDF/Jupyter 等格式不硬塞进 read，交给专用 CLI；ReadOperations 可替换为 SSH/Docker/Git blob。

**取舍**：得到安全探索、增量续读、文本/图片统一；放弃一次读全、图片细节可能丢失。

**对 OmniDesk**：知识库和附件读取必须带页窗与字节上限，不能把整个文件塞进模型；大文件用宿主索引/分页，不靠模型 cat；图片附件缩放后再进模型；PDF/Office 走专用解析，不伪装成 read。

## 第 22 章 bash：万能后备与安全边界

**核心判断**：bash 是后备，不是首选。它提供灵活性，但也是最大安全风险；安全不能靠提示词，要靠执行后端和产品层。

**机制**：
- schema 只有 command/timeout；timeout 可选且无默认值（不同命令耗时差异太大，让模型按命令性质决定）。
- BashOperations.exec 流式 onData、AbortSignal、exitCode；默认本地 shell 用 detached 进程组，超时先 kill 进程树再 reject，避免孤儿进程和输出竞争。
- spawn 前校验 cwd 存在；输出用 truncateTail（保留结尾的错误/结果），截断时把完整输出写到临时文件 fullOutputPath，模型可用 read 取回。
- BashSpawnHook 在 spawn 前改写命令/cwd/env，可做审计、set -e、环境过滤；安全梯度：beforeToolCall 审批 → Executor 隔离（Docker）→ 环境隔离。

**取舍**：得到灵活性兜底、可适配安全模型；放弃结构化输出、默认就是后门、需要产品层实现 sandbox。

**对 OmniDesk**：我们的 bash/pwsh 必须有 cwd 绑定、超时、进程树 kill、环境过滤、输出上限和审批；全量输出落临时文件而不是塞进上下文；不要把“不要执行危险命令”只写在 persona 里。Excel 生成这类任务应优先走受约束的 office/xlsx 入口，bash 只做兜底。

## 第 23 章 find/grep：结构化搜索替代万能 bash

**核心判断**：高频且易错的搜索从 bash 拆成结构化工具，降低出错率、控制 context、保证跨平台和可取消。

**机制**：
- find: pattern/path/limit，默认 1000；grep: pattern/path/glob/ignoreCase/literal/context/limit，默认 100。
- 两者默认尊重 .gitignore 且不可关闭；find 优先 fd，grep 用 ripgrep --json 流式；全程可 AbortSignal 取消。
- grep 三层截断：单行 500 字符、匹配数默认 100、总输出 50KB；matchLimitReached/linesTruncated 让模型知道结果不完整，可用 read 取完整行。
- GrepOperations/FindOperations 可替换，本地实现可同步、远程实现可异步。

**取舍**：得到更低搜索出错率、分层截断、性能与可取消；放弃多两个工具的选择负担，gitignore 可能漏掉构建产物。

**对 OmniDesk**：知识库检索不要给模型一个自由 grep 全库的工具；用宿主索引 + knowledge-search，只在明确需要时给受限搜索。若保留 grep，必须默认忽略依赖/构建目录、带 limit 和行截断，并允许取消。搜索结果不能无限进上下文。


## 第 24 章 pi-tui

**核心判断**：终端 UI 自建而不是套 Ink；Component 接口只要求 render(width) → string[]，用差分渲染和节流控制开销。

**取舍**：得到完全控制、低渲染开销、IME/Kitty 支持；放弃维护成本、声明式 API、大量文本边界处理。

**对 OmniDesk**：我们不是 TUI，但同样的判断适用于 React UI：不要为了“统一”引入重型框架；组件契约要窄，渲染要有节流。**借**：窄接口、差分/节流、能力探测；**不借**：把终端文本处理那套复杂度搬进 Web UI。

## 第 25 章 编辑器组件

**核心判断**：输入框不是 textarea；多行编辑、自动补全、@ 文件引用、! bash、粘贴折叠、IME、wrap、kill ring、undo 叠加起来，交互复杂度远超聊天框。

**取舍**：得到 IDE 般的流畅体验、大粘贴优雅；放弃大量边界测试、维护负担集中。

**对 OmniDesk**：我们的输入条已经支持 @ 引用、附件和模型选择，属于同一类“交互复杂度集中地”。**借**：把复杂度收敛到一个组件、明确状态转换；**不借**：把每个快捷功能分散到多个组件各自处理状态。

## 第 26 章 RPC 模式

**核心判断**：CLI 工具要能被 IDE/Web/自动化驱动，需要 JSON Lines over stdio 的 RPC 协议；命令与事件流分离。

**取舍**：得到任意前端驱动、Extension UI 透明代理；放弃协议版本维护、stdout 污染风险、调试困难。

**对 OmniDesk**：Tauri 与 engine 的关系就是这类 RPC/事件桥。**借**：命令与事件分离、typed client、会话状态显式；**不借**：在协议通道上混入普通日志或第三方输出（我们已要求 stdout 干净）。

## 第 27 章 pi-web-ui

**核心判断**：Web UI 用框架无关的 Web Components 复用 pi-ai/agent-core；浏览器端直接调模型，省后端代理。

**取舍**：得到框架无关、隐私简化、文档预览；放弃生态/工具链、浏览器端 key 安全隐患、依赖体积。

**对 OmniDesk**：我们是 React+Tauri，不需要 Web Components；但“框架无关的协议层”值得保留。**借**：产品壳与内核解耦、按需动态加载重依赖；**不借**：把 key 放浏览器存储，或为了复用强行换 UI 框架。

## 第 28 章 mom（Slack）

**核心判断**：同一内核换产品壳；channel 级 agent、数据隔离、双层记忆/Skill、Docker sandbox、事件调度，核心零修改。

**取舍**：得到安全多租户、内核复用、主动调度；放弃 Slack 消息限制、单 channel 串行、缺少同步审批流。

**对 OmniDesk**：这正是我们该有的形态——同一个 DSH 内核，OmniDesk 壳负责个人数据、审批和成果。**借**：接口适配而非 fork 内核、数据隔离、事件调度；**不借**：把 Slack 那种单 channel 串行模型套到个人工作台（我们需要多线程/多任务）。

## 第 29 章 pods：模型供给侧

**核心判断**：如果产品需要端到端可控，模型部署也可以纳入仓库；vLLM 暴露 OpenAI 兼容端点，agent 层无需改 provider。

**取舍**：得到模型自主权、一键部署、无缝集成；放弃职责边界清晰、没有 GPU 编排、依赖 SSH。

**对 OmniDesk**：不适合把 GPU 部署塞进个人工作台仓库。**借**：任何自部署模型都通过 OpenAI 兼容端点接入；**不借**：扩展到模型运维。

## 第 30 章 极简核心，能力外置

**核心判断**：内核只做调模型、跑循环、管状态；其他能力全部外置。协议式设计优于框架式设计。

**机制**：工具是数据结构，压缩/上下文是回调，产品壳通过事件流订阅；内核约 24%，外围约 76%。

**判断标准**：产品相关 → 外置；多种合理实现 → 外置为回调/接口；需要系统一致性 → 内建。

**取舍**：得到极致适应性、长期可维护；放弃开箱即用、开发速度、上手成本。

**对 OmniDesk**：DSH 是内核，OmniDesk 是产品壳。我们的“能力”应该外置为 Profile/插件/Skill/宿主命令，不要把通用平台、工作流编辑器、Registry、插件市场塞进产品核心。**借**：薄核心、接口组合、事件订阅；**不借**：用“功能丰富”替代架构清晰。

## 第 31 章 反主流选择

**核心判断**：四个“不做”（sub-agents、MCP、permission popup、plan mode）都用工具、回调、事件三种底层机制组合出来。

**理由**：减少内建概念、统一心智模型、允许设计者没预见的组合。

**取舍**：得到组合空间、统一心智；放弃开箱即用、MCP 生态互操作、最佳实践传递。

**对 OmniDesk**：不要因为“别人都有”就内建子代理编排、MCP 市场、计划模式。需要时用工具 + 回调 + 事件组合。**借**：先问“能否用现有机制组合”；**不借**：把每个流行功能都做成一级产品概念。

## 第 32 章 适用边界

**核心判断**：架构适不适合取决于产品形态数量、provider 数量、团队读源码意愿、安全模型、迭代速度；不是“pi 好不好”。

**取舍**：得到清晰决策框架；放弃营销友好叙事。

**对 OmniDesk**：我们是单一产品、单用户、本地优先，不需要 pi 的多产品适配税；但需要它的事务边界、事件流和薄核心。**借**：按边界选架构；**不借**：为了“通用”付出多产品复杂度。

## 全书结论与 0.2.30 候选

1. **产品提示词去插件化**：当前 5404 字符里 4732 是常驻搜索插件；目标是短底座 + 按需 Skill，系统提示词约 200–300 tokens。
2. **Skill 只给元数据**：桌面端现在注入 Skill 全文，改为名称/描述/路径，正文按需读；一个动作最多一个主 Skill。
3. **工具面按动作收敛**：搜索、Office、知识整理都按需加载；读写/审批工具保持显式、可审计。
4. **工具执行语义**：读并行、写串行、审批前置、错误作为可恢复工具结果、结果入上下文前脱敏；需要逐项核对 DSH 是否提供等价钩子。
5. **可观测性**：按阶段记录模型耗时、工具 prepare/execute/finalize、审批等待、缓存命中；没有这张表就不做“优化”。
6. **缓存稳定**：系统提示词前缀不要放日期/频繁变化的信息；日期留 runtime context，工作目录放最后。
7. **不内建新平台**：不新增通用工作流、Registry、插件市场、子代理编排；需要时用现有工具 + 回调 + 事件组合。

> 备注：书实际是 32 章 + 附录，没有第 33 章；本笔记覆盖第 1–32 章。后续可补附录 A–F 的类型与入口速查。

## 跨章设计模式（我的提炼）

**1. 协议，不是框架。** pi 的内核只定义事件、工具接口、循环配置；产品层决定 UI、权限、存储、提示词。判断标准：产品相关的外置，有多种实现的外置，需要系统一致性的才内建。OmniDesk 的对应物是 DSH（内核）+ 宿主适配层 + React/Tauri 产品壳；不要再造第三个 agent 框架。

**2. 薄内核，厚产品。** 内核越薄，产品越能自由组合；代价是上手成本、重复劳动和跨层调试。OmniDesk 要把复杂度放在正确的层：工作记录/任务/知识库的语义在宿主，模型循环在 DSH，提示词与工具面在 Profile，界面在 React。

**3. 事件流是唯一事实源。** 流式事件同时服务 UI、持久化、回放和错误处理；成功和失败都在同一条通道。OmniDesk 的时间线、失败卡、复制/朗读、任务台账都应该从事件派生，而不是各自维护状态。

**4. 工具执行是控制面，不是黑盒调用。** prepare 校验参数与权限，execute 支持中止与部分结果，finalize 做审计/脱敏/结果降级；并行只给独立操作，写操作与审批串行。OmniDesk 需要逐项核对 DSH 是否提供等价钩子；没有的话，只能在工具插件或适配层近似，并如实标注。

**5. 提示词是装配，不是作文。** 底座、追加、项目上下文、Skills 元数据、环境信息分层；纯函数、无 I/O、稳定前缀、XML 边界、日期不进系统提示词。OmniDesk 的目标不是“写得短”，而是“每一层只放该放的东西”；当前 5404 字符里 4732 是插件注入，这才是要修的地方。

**6. 能力是数据或文档。** 工具是数据结构，Skill 是 Markdown；工具激活即选择，不再用提示词教模型选工具；Skill 只给指针，正文按需读。OmniDesk 的搜索、Office、知识整理都应做成按需能力，而不是常驻提示词和常驻工具面。

**7. 配置要能解释。** 分层覆盖、默认值、信任闸门、冲突诊断、只写修改字段；代价是“我的配置从哪来”难以回答。0.2.30 若动提示词和工具面，应同时提供“当前有效提示词/工具/技能来源”视图，否则用户无法判断行为变化。

**8. 先测量，再优化。** 循环、工具、审批、缓存、提示词各自的耗时必须分开记录；没有阶段计时，优化容易打错靶子。OmniDesk 的 397 秒案例应先拆成模型往返、工具执行、审批等待、缓存命中四张账。

**对 OmniDesk 的直接结论：** 我们不缺一个更聪明的 agent 框架，缺的是把 DSH 的协议用对、把产品能力放到正确的层、把提示词和工具面按动作收敛。0.2.30 应先做“可观测的基线”，再做提示词与 Skill 注入的减法，最后才考虑工具执行层面的增强。

## 复核后的“不改清单”（第 0–3 章直接约束）

1. **不替换 DSH，也不移植 pi runtime。** DSH 是 OmniDesk 的执行引擎；pi 的价值是设计判断，不是让我们再养一个 agent 内核。
2. **不把 OmniDesk 改成 pi 的 monorepo/lockstep 拓扑。** 我们保留现有 desktop / engine / runtime / samples 边界；只强化“依赖只向内”，不做包数量和版本机制的模仿。
3. **不内建 pi 明确不做的能力。** 子代理编排、MCP 市场、计划模式、权限弹窗都不是 0.2.30 的内建项；需要时用工具、回调、事件组合。
4. **不改产品数据模型与边界。** 工作记录、任务、知识库、成果目录、线程/会话存储、文件写入规则、审批与脱敏都不因这次学习而改语义。
5. **不改模型路由与凭证。** 不读取、迁移、复制凭证，不把 key 放到前端或提示词；模型选择与 provider 配置仍走现有引用机制。
6. **不改 runtime/。** 不修改 DSH Runtime 源码；DSH 不暴露的钩子只记录缺口，不在产品层伪造。
7. **不照抄 pi 的提示词或工具集。** 只借装配方式、约束原则和边界判断；工具 schema、run_code 约定、会话/压缩协议仍按 DSH 的现实来。
8. **不为了“极简核心”删掉安全与确定性。** 审批、sandbox、文件边界、状态计算、落盘校验保留；这些属于系统一致性，不是可外置的产品偏好。
9. **不把这次学习变成 UI/交互重构。** 0.2.30 的范围是提示词装配与工具约束审计；界面与视觉不在本轮。

