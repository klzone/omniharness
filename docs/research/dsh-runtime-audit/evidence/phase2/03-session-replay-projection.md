# DSH-R2-03 Session Replay 与 Context Projection 源码审计

- 任务卡：DSH-R2-03
- source_commit：b830ec950faba0a46dda564be3e2d854afed94f1
- tag：dsh-v0.2.1-rc.5
- source_path：/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo
- package_versions：以下包均为 0.2.1-rc.5（package.json 实测）
  - @deepseek-ai/dsh-session（packages/core/session）
  - @deepseek-ai/dsh-session-projection（packages/session/session-projection）
  - @deepseek-ai/dsh-session-persistence-jsonl（packages/session/session-persistence-jsonl）
  - @deepseek-ai/dsh-compaction-basic（packages/compaction/compaction-basic）
- 审计时间：2026-11-12
- 方法：只读源码静态审计；未安装依赖、未运行构建或测试、未读取任何真实会话目录。

证据等级：S 静态源码；D 官方 README/设计说明（仅作线索）；U 未验证。本文结论默认以 S 为准，
README 只用于提示待核对的行为点。

---

## 0. 结论摘要

S：rc.5 的 Session 将持久事件日志作为唯一真源。每个事件带连续 seq，消息历史由 surfaceOp
标记的有序 surface 折叠得出，模型请求由 deriveMessages() + 最新 request/header（config/tools）+
toolHistory() 重建。压缩不删除日志，而是追加 compaction/summary 与一个 replace 的
user/message，让模型可见表面遮蔽被摘要的区间。JSONL 后端只保证物理上可读的连续前缀，
语义上的崩溃修复（补齐未闭合的 tool call / step / turn）由 agent-loop 以合成事件追加。

S：可重放的是事件日志及由其纯函数折叠出的消息、header、工具历史和 inbox；不能从日志重放的是
写缓冲、租约、进行中的模型流、派生缓存与生命周期对象。

---

## 1. 事件模型与序列契约

S：当前格式版本为 4，写死在 SESSION_FORMAT_VERSION。

- packages/core/session/src/types.ts:90 —— SESSION_FORMAT_VERSION = 4。
- types.ts:93-95 —— SessionHeader 明确位于会话事件日志之外。
- types.ts:95-132 —— header 承载 parentSession、isSeeded、origin: 'subagent'、delegationDepth、agentPreset。
- types.ts:494-512 —— 事件信封 { type, seq, time, data }，以及条件字段 surfaceOp / sourceEventSeqs 与 ignorable。
- types.ts:282 —— SessionEventMap。主要成员：turn/start(:289)、turn/end(:298)、step/start(:300)、
  step/end(:302)、user/message(:310)、developer/message(:312)、system/message(:331)、
  assistant/message(:342)、assistant/attempt(:356)、tool/call(:362)、tool/result(:376)、
  request/header(:391)、request/context(:403)、session/end-seed(:428)。
- types.ts:440-446 —— 只有 system/message、developer/message、user/message、assistant/message、
  tool/result 是 surface 事件。
- types.ts:463-486 —— SurfaceOp 为 'append' 或 { op: 'replace', startSeq, endSeq }。

S：seq 的连续性由 Session.append 直接以 seq = this.log.length 构造；seed 必须从 0 连续。

- packages/core/session/src/index.ts:798 —— append() 的公开签名与约束。
- index.ts:808 附近 —— dataSnapshot = snapshotJsonValue(data)，失败即拒绝。
- index.ts:819 附近 —— 事件按 seq = this.log.length、time = Date.now() 构造并 deepFreeze。
- index.ts:652 附近 —— seed 逐条校验 snapshot.seq !== index 即抛错。

S：存储侧 sourceEventSeqs 使用无损区间编码（连续三段以上折叠为 [start, end]），
解码时展开并校验严格递增。

- packages/core/session/src/seq-ranges.ts:18 —— encodeSeqRanges。
- seq-ranges.ts:37 —— decodeSeqRanges（含 maxEntries 上限与 start <= end 校验）。

---

## 2. 日志 → Surface → 模型消息

S：Surface 是日志上的有序视图，不复制事件；foldSurface 重放全部事件并按 surfaceOp
生成当前节点序列、替换历史与投影消息。

- packages/core/session/src/surface.ts:594 —— foldSurface(events, projections) 从头折叠。
- surface.ts:567-575 —— append 推入节点；replace 用 splice(startIdx, ...) 把区间替换成当前节点；
  project 只更新消息投影。
- surface.ts:330 —— assertSourceEventReferences：sourceEventSeqs 必须覆盖全部被遮蔽节点。
- surface.ts:612 —— SurfaceManager 是增量折叠器，构造时接收完整日志或窗口。
- surface.ts:699 —— _assertProjections()：用于解释日志的消息投影一旦被移除，读取即抛错，
  要求带拥有该事件的插件恢复会话。

S：单个事件到消息的投影规则：user/message 原样返回；system/developer/assistant 空内容
返回 null；tool/result 返回 message；其他事件（边界、attempt、日志记录）返回 null。

- surface.ts:120 —— deriveEventMessage。
- surface.ts:135-150 —— 空 system/developer/assistant 不进入 transcript；assistant/attempt
  与 turn/step 边界不产生消息。

S：Session.deriveMessages() 按 surface.nodes 顺序取事件并投影，缓存按
surface.contentGeneration 失效；返回新数组，但消息对象是共享且深冻结的。

- index.ts:984 —— deriveMessages()。
- index.ts:988-993 —— generation 变化时清空缓存并重新从头投影。
- index.ts:1000 附近 —— 空投影为 null 时不入列。

链路总览：

    JSONL / memory events (contiguous seq)
            │  Session.append / restore seed
            ▼
    immutable SessionEvent log ──► SurfaceManager.fold (append | replace | project)
            │                               │
            │                               ├─► Surface.nodes (当前模型可见节点顺序)
            │                               └─► projectedMessages (插件消息解释)
            │
            ├─ request/header   ──► foldRequestHeader ──► EpochHeader(config, adapterDefaults, tools)
            ├─ developer/message ─► ToolHistoryProjection ─► ToolHistory(tools, updates)
            ├─ request/context ──► 最新路由元数据（不参与请求重建）
            └─ turn/step/tool/assistant 事件 ──► 追溯/崩溃修复/审计，非模型消息
            ▼
    Session.deriveMessages() ──► Agent.buildRequest() ──► LlmCallConfig + messages + tools + toolHistory

---

## 3. 请求上下文重建

S：请求头以最新完整快照折叠；日志不保存 system 文本（EpochHeader.system?: never，
types.ts:241-252），系统提示词是 system/message surface 节点。

- packages/core/session/src/request-header.ts:21 —— canonicalHeader：空 tools/空 adapterDefaults 归一为缺省。
- request-header.ts:43 —— headerEquals：config、adapterDefaults、tools（按顺序 JSON 比较）。
- request-header.ts:63 —— foldRequestHeader：纯离线折叠，跳过非 header 事件。
- index.ts:912 —— Session.requestHeader() 增量缓存（headerFoldSeq）。
- index.ts:933 —— Session.requestContext() 折叠最新 request/context。
- index.ts:953 —— Session.toolHistory() 增量折叠 request/header 与 developer/message。

S：Agent Loop 每步重建请求：先解析/绑定路由，再记录 request/header，随后把
session.deriveMessages()、session.toolHistory() 与 header 的 tools 一起放入请求。

- packages/core/agent-loop/src/agent.ts:564 —— prepareRequest。
- agent.ts:616 —— buildRequest。
- agent.ts:637/644/650 —— 首次 initial|resume、变化时 change、仅有新消息序列时 series。
- agent.ts:660 附近 —— 工具增减以 developer/message 记录，additions 必须带 headerSeq。
- agent.ts:677 附近 —— request/context 仅在 provider/model/contextWindow/systemPromptUpdate 变化时追加。
- agent.ts:688 —— boundaryMessages = session.deriveMessages()。
- agent.ts:695-700 —— 组装 config + messages + toolHistory + tools + sessionId + signal。

S：系统提示词写入 surface node 0：systemPrompt.project(...) 返回的提交逐个
session.append('system/message', ...)（agent.ts:423）。system/message 事件注释明确了
不能路由 in-history 的模型会把非空后置节点清空/重写 head 的规则（types.ts:331 附近注释）。

---

## 4. 工具历史

S：ToolHistoryProjection 与模型能力无关地折叠 header 和 developer 消息。

- packages/core/session/src/tool-history.ts:8 —— 投影类。
- tool-history.ts:26 —— apply：request/header 建立 baseline；developer/message 解析
  tool-addition 并从历史 header 取出完整定义；缺少定义直接抛错（:49）。
- tool-history.ts:36 —— series、startsSeries 或同名定义变化时重置 baseline。
- tool-history.ts:68 —— snapshot()：若 active 与 available 不一致（旧会话无更新事件），
  返回纯 { tools, updates: [] }。

S：对模型可见的工具列表来自当前 header 的 tools；toolHistory 只提供历史声明的重建，
供 provider 适配层决定如何呈现更新序列（本卡未追踪适配层序列化）。

---

## 5. 压缩边界与替换语义

S：压缩选择从首个非 system surface 节点开始，保留最近 retainTokens 尾部，并在
tool-call/result 配对边界之外截断。

- packages/compaction/compaction-basic/src/region.ts:117 —— selectCompactableRange。
- region.ts:137-147 —— 从尾向前累计 token，直到达到 retainTokens；system/message 在 node 0 时排除。
- region.ts:145 —— toolPairingBalancedBefore 保证不切开工具配对。
- region.ts:349-353 —— 显式范围也校验 start/end 的配对平衡。

S：压缩事务：同步追加 compaction/start 作为锁，摘要后追加 compaction/summary，再追加一个
user/message 作为替换节点，最后关闭 compaction/end；失败只尝试一次 end，留下可检测的未闭合 start。

- region.ts:210 —— session.append('compaction/start', lifecycle)。
- region.ts:491 —— session.append('compaction/summary', ...)，含 shadowedRange、shadowedSeqs、
  shadowedTokenCount、provider/model/usage。
- region.ts:506-508 —— session.append('user/message', checkpointMessage, { surfaceOp: { op: 'replace', ... } })；
  sourceEventSeqs 同时引用 start、summary 与全部被遮蔽节点。
- region.ts:237/245 —— 成功/失败结束。
- region.ts:554-567 —— inspectCompactionEntryState 反向扫描，识别 open turn、未配对的
  compaction/start 与最新 session/end-seed。

S：压缩不会删除原始事件：replace 只改变 surface 节点集合，日志仍保留被遮蔽事件。
人类 transcript 应使用 append-origin 事件，而不是模型可见 surface（surface.ts:78-93）。

S：自动触发按路由模型上下文窗口计算阈值，并可选先执行 model-free 的 tool result prune。

- packages/compaction/compaction-basic/src/index.ts:266 —— compactIfNeeded(agent, trigger, signal)。
- index.ts:292 —— 取可选 toolResultPruner。
- index.ts:305-310 —— context-overflow 直接 prune 后以 retainTokens = 0 强制压缩一次。
- index.ts:319 —— 压力阈值判断；index.ts:327 prune 后重新测量。
- 具体 pruner 的裁剪规则未在本卡读取，列为 U。

---

## 6. JSONL 持久化：追加、读取、撕裂尾部

S：后端以不可变物理代际存放日志，每个会话一个目录；写句柄串行化追加并持有跨进程写锁。

- packages/session/session-persistence-jsonl/src/storage.ts:118 —— read(offset, length, options)。
- storage.ts:187 —— append(events) 先快照整批，再进入串行链。
- storage.ts:319 —— persistContiguous：校验 assertContiguous(:323)、修复撕裂尾部(:328-336)、写批次(:338)。
- storage.ts:274 —— enqueueLive：每事件 structuredClone 后进入有界批缓冲。
- storage.ts:288 —— drainLive：合并并发 drain，失败保留批次。
- storage.ts:223 —— close：先 drain 再释放租约，失败聚合为 AggregateError。
- storage.ts:172-175 —— readCurrent 拒绝存储日志长度小于已观察前缀（防回退）。

S：撕裂尾部（torn tail）的识别与修复。

- packages/session/session-persistence-jsonl/src/format.ts:386 —— SessionLogScanner 逐行扫描。
- format.ts:462-473 —— finish() 忽略最后一条无换行的记录作为撕裂尾部，返回 committedBytes。
- format.ts:532 —— scanLog(buffer) 一次扫描完整或撕裂缓冲。
- packages/session/session-persistence-jsonl/src/index.ts:809 —— decodeStoredLog 返回 tornTruncateTo；
  只有 committedBytes < buffer.byteLength 时才非空（:838）。
- index.ts:942 —— truncateTornTail 物理截断并告警。
- index.ts:1262 附近 —— 首次物化临时写 + fsync + 发布；:1296-1299 使用 link() 防覆盖。

S：语义修复不属于存储层：JSONL 只返回物理有效前缀，agent-loop 恢复时计算
interruptedTurnClosers 并把合成结果作为普通批次追加。

- packages/core/session/src/repair.ts:53 —— openTurnClosers(events, cause)。
- repair.ts:88-96 —— 先生成未回答工具调用的错误结果，再补 step/end 与 turn/end，seq 接续。
- repair.ts:157 —— ToolCallRecovery.results()：按 assistant 顺序生成错误结果，
  区分 ToolOutcomeUnknownError（已开始）与 ToolNotStartedError（未开始）。
- repair.ts:209 —— interruptedTurnClosers 是 cause: 'interrupted' 的公开入口。
- packages/core/agent-loop/src/index.ts:891 —— resume 读取冷日志后计算 closers；:892 通过同一写句柄追加。

---

## 7. Resume 与 Fork 的区别

S：两者都通过 seed 重建 Session，但 lineage、边界闭包和标记不同。

| 维度 | Resume | Fork |
| --- | --- | --- |
| 输入 | 完整存储日志（handle.read(0, undefined)） | 源会话的包含式前缀 [0, boundary] |
| 边界处理 | interruptedTurnClosers 以 interrupted 因果补齐 | buildForkSeed 以 forked 因果补齐 |
| 标记 | 构造器追加无 tag 的 session/end-seed {} | 前缀尾部追加 session/end-seed { inherited: true } |
| header | 复制存储 header，isSeeded 保持原值 | isSeeded: true、parentSession 指向源 |
| 继承计数 | 存储中的 inheritedEventCount 原样保留 | resolved + 1（仅计入复制的前缀） |
| 子会话身份 | 同一 id 重新打开 | 新 id（可显式指定） |

源码：

- packages/core/session/src/fork.ts:21 —— buildForkSeed(events, boundary)：copy prefix、追加
  session/end-seed { inherited: true }、再 openTurnClosers(prefix, { kind: 'forked' })。
- packages/core/session/src/index.ts:1380 —— SessionStore.fork；:1385 取 snapshot，
  :1389 调 buildForkSeed，:1390 传 inheritedEventCount: boundary + 1。
- index.ts:695-697 —— 构造器：seeded 且未带 marker 时追加 { inherited: true }；
  普通 restore 追加 {} 的 resume marker。
- index.ts:669-690 —— seeded 必须带 seed 与 inherited count；标记必须是最后一个 inherited marker。
- packages/core/agent-loop/src/index.ts:891-900 —— resume 的 closers 与
  SessionPreparation.create(... seed: [...persisted, ...closers] ...)。

S：两个细节值得区分。

1. fork 的合成 closers 位于 inheritedEventCount 之外，属子会话自有历史；源会话在边界前
   被保留的事件完全不变（fork.ts:10-29 注释与实现）。
2. 源码注释明确 torn tail 修复只丢弃从未确认的不完整片段；崩溃前已持久化的完整事件保留，
   resume 不继续未完成的 turn，而是关闭它（packages/session/session-persistence/README.md:61，
   D 线索；实现见 agent-loop/src/index.ts:891 与 repair.ts:53）。

---

## 8. 哪些状态可重放，哪些依赖运行态

S：可重放（纯日志 + 纯折叠）。

| 状态 | 来源 | 重建入口 |
| --- | --- | --- |
| 消息历史 | surface 事件 | deriveMessages()（index.ts:984） |
| 当前 surface 顺序 | surfaceOp | foldSurface / SurfaceManager.nodes（surface.ts:594/674） |
| 请求 header | request/header | foldRequestHeader（request-header.ts:63） |
| 路由元数据 | request/context | requestContext()（index.ts:933） |
| 工具历史 | header + developer | ToolHistoryProjection（tool-history.ts:8） |
| pending inbox | agent/inbox/spliced | inbox projection（agent-loop/src/inbox.ts:27-65） |
| 子 Agent 深度/预设 | header | SessionHeader（types.ts:118-131） |

S：依赖运行态、不能只靠日志恢复。

| 状态 | 原因 | 证据 |
| --- | --- | --- |
| 进行中的模型流 | 只有 settled 后才写 assistant/message 或 assistant/attempt | agent.ts:468-509；types.ts:342-356 |
| 写缓冲 / 定时器 | JSONL 内存队列，崩溃即丢未 flush 部分 | storage.ts:274-308 |
| 跨进程写锁 | 内核锁与进程生命周期绑定 | storage.ts:967-990；lease.ts（本卡未逐行审计） |
| 派生缓存 | header/tool/message 缓存可按 generation 重建 | index.ts:912/933/953/984 |
| 插件消息投影 | 必须由拥有事件的插件重新注册，否则读取抛错 | surface.ts:699-709 |
| 投影缓存行 | 只是 fold shortcut，版本/序号不符即丢弃 | session-projection/src/index.ts:426-541 |

S：ctx.sessionProjections 是事件驱动的纯折叠注册表：每个已提交事件驱动所有已注册单元；
冷读时从 checkpoint 尾部前向重放，行不可用则从 0 重折。

- packages/session/session-projection/src/index.ts:199 —— 注册表服务。
- index.ts:220-222 —— 订阅 session/event，调用 drive。
- index.ts:496 —— restore(checkpoint, events, baseSeq, header, inheritedEventCount)。
- index.ts:515-519 —— 行缺失/版本不符/越过日志末端且 baseSeq > 0 时抛错，要求从 0 重读。
- index.ts:658 —— drive：先补齐前缀，再 apply 当前事件；只有状态引用变化才计算 client view。

---

## 9. 反证与容易误读的点

1. S：日志可解析不等于语义可恢复。JSONL 只拒绝结构损坏与未知必需事件；未闭合的
   turn/tool 仍是一个物理有效前缀，需要 interruptedTurnClosers 才能成为模型可接受的上下文。
   证据：format.ts:462-473 只按换行收尾；repair.ts:53-97 才补齐语义。
2. S：Session 不是只要日志就能重建。SessionHeader 不在事件日志内（types.ts:93-95），
   丢失 header 就丢失 parentSession/isSeeded/delegationDepth/agentPreset。
3. S：压缩不是删除历史。原始事件保留；replace 只影响模型可见 surface。
   若用 surface 生成人类 transcript，会丢掉已被遮蔽的对话（surface.ts:78-93 明确警告）。
4. S：assistant/attempt 与 turn/step 边界是可追溯记录，不进入 provider transcript；
   不要把事件条数当作消息条数。
5. S：恢复时的组合依赖：缺少 MESSAGE_PROJECTION_EVENT_TYPES（当前仅 image/offload，
   known-event-types.ts:88-90）的解释器，带该事件的会话会拒绝派生，而不是静默降级。
6. D：README 把 resume 通过写句柄追加 closers 描述为唯一崩溃方案；源码与之一致
   （agent-loop/src/index.ts:891），但 README 本身不作为实现证据。

---

## 10. 未验证项

1. U：未实际创建/读取 JSONL 会话，也未运行 repair.spec.ts、surface.spec.ts 等；
   因此真实文件上的 torn tail 截断与重写只有静态代码证据，没有运行证据。
2. U：未验证真实 provider 对 toolHistory 的序列化方式；模型请求中的工具历史是否被适配层
   全部使用，取决于 llm-pi-ai 适配器，本卡未追踪。
3. U：未验证 compaction-basic 在不同 contextWindow/推理档位下的实际阈值与摘要质量；
   只确认了计算入口与边界选择。
4. U：未审计 session-query 在冷读时内存补齐 closers 的实现；本卡只覆盖 agent-loop resume 路径。
5. U：未验证并发写者/外部进程篡改下的实际行为；源码有进程内串行链与内核租约，但未做
   文件系统级实验。
6. U：未验证插件投影缓存（session-projection-cache）的真实持久化后端与失配恢复路径。

---

## 11. 下一张卡不要混入的边界

- 本卡不判断 DSH 是否适合作为 OmniDesk 底座，也不比较 Pi。
- 不把本卡的静态结论外推到其他 commit/tag；所有行号仅对应 b830ec95。
- 不在本卡展开 Agent Loop 的完整状态机（R2-02）、工具 schema 注册（R2-04）、并行调度（R2-05）、
  子 Agent 生命周期（R2-06）。

---

## 12. 命令与退出码

以下命令均以只读方式执行；除特别注明外退出码为 0。

    git -C <source> rev-parse HEAD                      -> b830ec950faba0a46dda564be3e2d854afed94f1 (exit 0)
    git -C <source> describe --tags --exact-match HEAD  -> dsh-v0.2.1-rc.5 (exit 0)
    git -C <source> status --short                      -> 空 (exit 0)
    rg --files <session/compaction 目录>                 -> 文件清单 (exit 0)
    wc -l <相关源码>                                     -> 行数 (exit 0)
    cat -n / sed -n / rg -n <指定文件>                   -> 逐段读取 (exit 0)

一次正则拼写错误导致 rg 退出 2（unclosed group），随后以修正后的表达式重跑并取得结果；
未修改任何源码文件。

---

## 13. 文件与边界

- 本报告是本次任务唯一写入的文件（与 R2-06 报告合计两份）。
- 源码树 /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo 只读，未执行 git 写操作。
- 未读取真实会话目录、凭证、会话正文或用户数据。

