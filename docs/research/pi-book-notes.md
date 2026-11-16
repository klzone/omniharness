# Pi Agent 设计笔记（第 1–32 章）

> 来源：https://zhanghandong.github.io/pi-book/ 的公开章节（2026-11-16 抓取）。
> 用途：作为 OmniDesk 0.2.30 架构与提示词优化的学习底稿；只记录设计判断与可借鉴/不可借鉴项，不复述源码细节。
> 口径：笔记中的“pi”指书里描述的 pi-mono agent 运行时，不是某个具体客户端。

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

**核心判断**：循环引擎是无状态纯函数，只接收 context + config，输出事件流；它不管理会话、不重试、不压缩上下文。

**机制**：双层循环（内层忙碌、外层等待）；消息变换只发生在 LLM 边界；stream 函数底层必填；AgentLoopConfig 承载转换、工具执行、回调；事件是唯一输出通道。

**取舍**：得到极致可组合、可测试、关注点分离；放弃会话持久化、自动重试、自动 compaction，且要求上层正确组装配置。

**对 OmniDesk**：DSH 的 loop 对应这一层；重试、压缩、会话持久化在 DSH 上层或我们的适配层，不能塞回 loop。**借**：薄循环、事件唯一输出、纯函数可测试；**不借**：在产品层再造一个循环或把重试逻辑塞进 UI。

## 第 9 章 工具执行不是插件调用

**核心判断**：工具执行是 prepare → execute → finalize 三段管道，不是“找到工具、调一下”。

**机制**：prepare 做工具查找、参数预处理、schema 运行时校验、beforeToolCall；校验失败立即变成错误工具结果，循环不中断。execute 带 abort signal、onUpdate 部分结果。finalize 用 afterToolCall 做字段级覆盖（content/details/isError/terminate），并传播 addedToolNames 解锁后续工具。

**并发**：默认 parallel，但 prepare 串行以保证权限检查确定性；execute/finalize 并发，生命周期事件按完成顺序到 UI；写回上下文的 tool-result 按源顺序。单个工具声明 sequential 会让整批降级串行。

**取舍**：得到可观测/可控钩子、错误可恢复、安全与性能兼顾；放弃无钩子时的零开销、钩子错误会伪装成工具失败、并行工具之间不能有依赖。

**对 OmniDesk**：工具错误必须作为可恢复结果回到模型；读操作可并行、写盘/审批/有副作用操作应串行；UI 展示完成顺序，上下文与存储保持源顺序；工具结果进入模型前做脱敏。我们依赖 DSH 的管道，能借的是这些语义和分层，不能假设 DSH 一定暴露同名钩子，需要逐项核对。

## 第 10 章 Agent：循环之上的有状态壳

**核心判断**：无状态循环负责“转”，有状态 Agent 负责“记住和协调”。Agent 的状态变更只走 processEvents 一个入口。

**机制**：MutableAgentState、事件订阅（有序、受 signal 保护）、消息队列（prompt/steer/followUp 两种节奏）、一个 AbortController 管全局中止、互斥锁禁止重入；还有 SessionStorage/SessionRepo 两层、RetryPolicy 与 retry 事件、models-only 认证。

**取舍**：得到清晰职责、可预测状态、多个消费者并行观察；放弃胖接口（30+ 方法）、listener 串行会拖慢 UI、单线程不能后台常驻。

**对 OmniDesk**：宿主已经承担了状态壳的角色（任务台账、线程、队列）。我们要保证状态修改只有一条路径，UI 只订阅事件而不直接改 agent 状态；不要把会话存储、重试、压缩塞回循环。**借**：状态归约器、单中止控制、订阅解耦；**不借**：在 UI 里维护第二套会话状态。

## 第 11 章 会话树

**核心判断**：coding agent 的会话不是线性聊天记录，而是可分支的树；JSONL + parentId 是够用的极简方案。

**机制**：9 种 entry 类型分核心/元数据/扩展三层；buildSessionContext 从树构建 LLM 消息；/tree、/fork、/clone 三种分支；RPC 只读 get_entries/get_tree。

**取舍**：得到 append-only 容错、人类可读可修复、零依赖、天然分支；放弃随机访问、无索引、文件持续增长、首次全量加载、单进程写入。

**对 OmniDesk**：我们已经有线程与场景会话，分支/派生对话的价值在于“从某轮继续”，不要复制整段历史。**借**：parentId 树、append-only、按路径构建上下文；**不借**：把会话树做成数据库或引入索引，除非真的需要跨会话查询。

## 第 12 章 Compaction

**核心判断**：压缩不是简单摘要；它要保留文件操作记忆、结构化更新摘要、并且是产品层策略而不是 runtime 内建。

**机制**：按 token 阈值触发；从后往前找合法切点；turn splitting；文件操作追踪；previousSummary 增量更新；prepare 与 compact 分离，extension 可接管；retry/会计/路由隔离。

**取舍**：得到理论无限对话、保留文件记忆、可定制、避免摘要的摘要；放弃有损、额外 token 消耗、不可逆。

**对 OmniDesk**：长会话的压缩应由 DSH/产品层负责，UI 不应假装历史全在。**借**：结构化摘要 + 文件操作追踪 + 增量更新；**不借**：把“压缩后仍能记住所有细节”当承诺，必须在界面说明有损。

## 第 13 章 三级配置覆盖

**核心判断**：配置分层是让同一工具适配不同项目的手段；全局、项目、目录三级，默认值让零配置也能启动。

**机制**：settings 深度合并（对象递归、数组替换），AGENTS.md 拼接，SYSTEM.md 替换；目录树向上发现；文件锁和只写修改字段保证并发安全；Project Trust 是第四道闸门。

**取舍**：得到零配置、渐进定制、团队共享、并发安全；放弃配置来源的可解释性、不同合并语义带来的心智负担、没有 dry run、信任闸门摩擦。

**对 OmniDesk**：我们的 Profile 和模型设置已有分层雏形，但需要能回答“当前生效配置从哪来”。**借**：默认值、分层覆盖、文件锁、信任闸门；**不借**：三层合并却不给用户一个解释入口。0.2.30 若做提示词预算，应同时提供“当前装配来源”视图。

## 第 14 章 System Prompt 装配

**核心判断**：系统提示词不是一块手写文本，而是默认/自定义底座 + 追加 + 项目上下文 + Skills 元数据 + 工作目录的装配流程；函数本身不做 I/O。

**机制**：工具列表只为提供了说明的可见工具生成；guidelines 动态生成并去重，不再教模型在工具之间选择；skills 只注入 name/description/location，正文按需 read；project context 用 XML 标签；工作目录放最后；日期被移除以稳定 prompt cache。

**取舍**：得到无限可定制、随环境变化、关注点分离、纯函数可测试；放弃最终形态难预测、prompt 可能很长、拼接顺序隐含优先级、自定义底座会失去工具指引。

**对 OmniDesk**：这正是 0.2.30 的核心依据。我们现在的 5404 字符里 4732 是常驻 web-search 插件，只有 120 字符产品人设；应该换成短产品底座，Skills 只给元数据，项目上下文不进产品提示词，日期留在 runtime context。**借**：装配流程、XML 边界、技能延迟加载、缓存稳定前缀；**不借**：把插件说明塞进常驻底座。

## 第 15 章 Extension 系统

**核心判断**：能力可以外置，但核心规则不可改；Extension 能加工具、命令、事件、持久化、Provider，但不能改循环、状态归约、消息变换。

**机制**：Loader → Runner → Wrapper 三层；两阶段初始化；26 种事件类型；类型安全合约；没有沙箱，靠信任和审计。

**取舍**：得到开放但有边界、类型安全；放弃深度定制能力、进程隔离、上手成本。

**对 OmniDesk**：DSH 插件对应 Extension，Skill 对应文档能力。**借**：开放能力但锁核心；**不借**：把产品核心规则做成可被插件随意覆盖，或引入无沙箱的任意代码执行。

## 第 16 章 Skill 用文档替代代码

**核心判断**：大多数能力扩展是“告诉 LLM 怎么做”，不是“写代码替 LLM 做”；Skill 是纯文本，零依赖、可审计、可版本控制。

**机制**：frontmatter + 目录发现；disable-model-invocation 控制可见性；formatSkillsForPrompt 只注入元数据；来源优先级、symlink 去重、gitignore。

**取舍**：得到零风险、版本友好、低创建成本、渐进复杂；放弃运行时能力、效果依赖模型遵循、发现机制简单、全局优先可能覆盖项目。

**对 OmniDesk**：知识整理、周报、质检这些流程适合 Skill；需要运行时能力的（读文件、写盘、检索）应由宿主工具做。**借**：Skill 只做指令、正文按需加载、可审计；**不借**：把 Skill 当权限控制或把全文塞进每轮提示词。

## 第 17 章 Resource Loader

**核心判断**：所有外部资源（extensions、skills、prompts、themes）走同一套发现、覆盖、诊断流程；单个资源失败不阻塞启动。

**取舍**：得到统一心智、渐进降级、可测试；放弃资源类型差异被抹平、加载顺序不够透明。

**对 OmniDesk**：我们的 Profile 资产、Skill、模型覆盖也应该有统一入口和冲突诊断；不要每个功能各自扫目录。**借**：统一发现、diagnostic、override；**不借**：统一接口导致简单资源承担过度设计。

## 第 18 章 Model Registry

**核心判断**：选模型 = provider + api + 参数，不只是一个 id；内建目录构建时生成，models.json 做渐进覆盖。

**机制**：ModelsStore 与 etag 条件刷新；model runtime 做 model/auth 门面；compat 层处理 OpenAI 兼容差异；扩展可注册 provider。

**取舍**：得到动态性、离线确定性、渐进覆盖；放弃运行时才知道模型可用、compat 维护成本持续增长。

**对 OmniDesk**：我们已经遇到“模型 id 看起来对、端点其实不支持”的问题。**借**：模型条目要带 provider/api/能力/compat，保存前做可用性预检；**不借**：把模型名当唯一标识。方舟 Agent Plan 的 404/UnsupportedModel 就是缺少这层预检的代价。

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

