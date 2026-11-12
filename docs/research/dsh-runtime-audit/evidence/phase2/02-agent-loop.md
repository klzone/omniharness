# DSH-R2-02：Agent Loop 轮次状态机

- step_id：DSH-R2-02
- generated_at：2026-11-12T10:19:56Z
- source_commit：b830ec950faba0a46dda564be3e2d854afed94f1
- source_tag：dsh-v0.2.1-rc.5
- source_path：/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo
- package_versions：
  - @deepseek-ai/dsh-agent-loop：0.2.1-rc.5
  - @deepseek-ai/dsh-tools：0.2.1-rc.5
  - @deepseek-ai/dsh-session：0.2.1-rc.5
  - @deepseek-ai/dsh-agent：0.2.1-rc.5
- 证据等级：S = 静态源码或测试代码；D = README 或官方说明；U = 未验证。本报告没有用 D 级材料证明运行行为。
- 执行边界：只读 /tmp 源码；未安装依赖、未运行测试、未运行模型、未启动服务、未修改源码树或仓库 runtime/。

## 1. 单一动作与入口

本卡只追踪一轮用户输入从排队到结束的 Agent Loop 状态迁移。入口是 ReactLoopAgent 的 send/followup/steer/inject，驱动入口是 kick 与 turn，单步入口是 step。调度细节留给 DSH-R2-05。

核心源码：

- packages/core/agent-loop/src/agent.ts
- packages/core/agent-loop/src/assistant-stream.ts
- packages/core/agent-loop/src/inbox.ts
- packages/core/agent-loop/src/runtime-context.ts
- packages/core/agent-loop/src/index.ts
- packages/core/agent-loop/src/constants.ts
- packages/core/session/src/types.ts
- packages/core/session/src/repair.ts

## 2. 状态图（文字）

~~~text
idle
  | send(followup/steer)+wake, or inject 后另一次 wake
  v
running(turn = lastTurn, step = 0)          [agent.ts:227-234]
  |
  | kick -> turn()
  v
turn/start(turn+1)                          [agent.ts:305-311]
  |
  v
preStep(target, position)                   [agent.ts:267-288]
  |- inbox.claim(target, turn)              [agent.ts:271; inbox.ts:109-114]
  |- systemPrompt.assemble(...)             [agent.ts:272]
  |- agent/pre-step waterfall               [agent.ts:274-284]
  |- reject -------------------------------> turn/end(blocked), driver false
  |- enter(empty at step 0) ---------------> turn/end(completed), no step
  v
step/start(turn, step)                      [agent.ts:331-332]
  |
  v
step(decision)                              [agent.ts:400-561]
  |- prepareRequest + request/header/context
  |- system/message and first-attempt user/message admission
  |- llm.stream(request)
  |- assistant stream attempt
  |- finish error/aborted -> optional agent/request-error retry, same step
  |- finish max-tokens -> step result max-tokens
  |- no tool calls -> step result completed
  |- tool calls -> executeToolCalls(...)
  |     |- concluded -> step result completed
  |     |- otherwise -> step result null (continue to next step)
  v
step/end(turn, step)                        [agent.ts:358]
  |
  |- turnEnds set and next-step empty:
  |     agent/turn-stopping serial hook      [agent.ts:361-364]
  |- turnEnds set and next-step empty again -> break
  |- otherwise -> next step with target next-step
  v
turn/end(turn, reason)                      [agent.ts:385-390]
  |
  |- inbox.hasPending -> new AbortController, step=0, next turn
  |- no pending -> driver false -> idle
~~~

状态写法是“阶段 + 持久边界”，不是独立的运行时枚举。源码中的 Phase 只有 idle、maintenance、running 三种，turn/step 是 running 内的字段：

- agent.ts:42-50：Phase 联合类型。running 持有 abort、turn、step、wakeRequested；maintenance 也持有 abort 和 lastTurn。
- agent.ts:140-142：status 把 idle 和 maintenance 都报告为 idle，running 报告为 running。
- agent.ts:144-152：setPhase 只有在对外 status 变化时才发出 agent/status。

## 3. 状态迁移表

| 当前状态/边界 | 触发 | 源码动作 | 持久或可见事件 | 下一状态 |
| --- | --- | --- | --- | --- |
| idle | send 且 wakeup | 解析 target；wakingAfterAbort 时改投 next-turn；splice 入 inbox；wakeDriver | agent/inbox/spliced；可能 agent/status(running) | running |
| idle | followup | send(next-turn, wakeup=true) | 同上 | running |
| idle | steer | send(next-step, wakeup=true) | 同上 | running |
| idle | inject | send(next-step, wakeup=false) | agent/inbox/spliced，不打开 turn | idle |
| running | kick | while(await turn()) | 无 | running 或 idle |
| running | turn 入口 | phase.turn + 1；append turn/start | turn/start | running(turn 已递增) |
| running | preStep reject | 不再进入 step；return false | turn/end(blocked) | idle |
| running | preStep enter 且 step 0 的 messages 为空 | 不进入 step；return false | turn/end(completed) | idle |
| running | preStep enter | append step/start；phase.step=step | step/start | running(step 已递增) |
| running step | 模型无 tool call | step 返回 completed | assistant/message；step/end | 可能 turn 结束或下一 step |
| running step | 模型有 tool call，工具未 concludeTurn | step 返回 null | tool/call、tool/result；step/end | 下一 step |
| running step | 模型有 tool call，工具 concludeTurn | step 返回 completed | tool/result；step/end | turn 可能结束 |
| running step | finish max-tokens | step 返回 max-tokens | assistant/message；step/end | turn 可能结束 |
| running step | finish error/aborted，且 agent/request-error 选择 retry | continue 同一 step 的 while | assistant/attempt；不结束 step | 同一 step 重试 |
| running step | finish error/aborted，不 retry | 抛 LlmError 或原始错误 | assistant/attempt 或 interrupted assistant/message | turn error/aborted |
| running step | step 抛错 | turn catch 先补 toolRecovery.results()，再抛 | 补 tool/result；step/end；turn/end(error) | idle |
| turn 末尾 | turnEnds 且 inbox.nextStep 为空 | agent/turn-stopping serial hook | agent/turn-stopping | break 或继续 |
| turn 末尾 | inbox.hasPending | 新 AbortController；step=0 | 无 | running 下一 turn |
| turn 末尾 | 无 pending | return false | 无 | idle |
| running | cancel(cause) | 清 inbox（除非 keepInbox），清 wakeRequested，abort 当前 signal | agent/inbox/spliced 或清空事件 | abort 收敛后 idle |
| running | maintenance | 仅 idle 可进入；status 仍为 idle | 无 turn 事件 | maintenance -> idle |
| idle | resume | 读取持久日志；补 interrupted closers | tool/result、step/end、turn/end(interrupted) | 可发布 agent 或失败 |

## 4. 逐段源码追踪

### 4.1 输入队列不是普通数组

inbox 是持久投影，不是进程内数组：

- inbox.ts:21-24：投影 schema 只有 next-turn 与 next-step 两个 readonly 数组。
- inbox.ts:27-65：inboxProjectionDefinition 通过 agent/inbox/spliced 事件折叠；apply 校验 start、removedCount、越界和重复 message.id。
- inbox.ts:74-95：ReactLoopInbox 从 sessionProjections.stateOf(session, 'inbox') 读取，不拥有注册；nextTurn、nextStep、hasPending 都来自投影。
- inbox.ts:109-114：claim 先取走全部 next-step，再在 target=next-turn 时取走 next-turn 的第一个元素，并对每条被 claim 的消息发出 agent/inbox/claimed。
- inbox.ts:167-243：splice 归一化 start/deleteCount，拒绝跨列表重复 id，然后 append agent/inbox/spliced；discardRemoved 为 true 时发出 agent/inbox/discarded。
- inbox.ts:97-101：clear 先清 next-step，再清 next-turn。
- agent.ts:154-161：send 在插入前判断 wakingAfterAbort；如果当前 phase 非 idle 且 abort signal 已中止，则把唤醒输入改投 next-turn。
- agent.ts:163-173：followup=next-turn+wake，steer=next-step+wake，inject=next-step+no wake。

边界语义：
- next-step 在一个 step 边界被全部 claim；next-turn 只在 target=next-turn 时 claim 一个。
- claim 本身是持久 splice，不是内存出队；被 claim 的消息已经离开 pending 列表。
- 重复 id 在写入或重建时都会被拒绝；这防止同一消息同时存在于两个队列。
- inject 不会打开 turn；只有带 wakeup 的 send 才会调用 wakeDriver。

### 4.2 驱动与轮次外层

- agent.ts:214-235：wakeDriver 在非 idle 时只可能 latch wakeRequested；idle 时创建 driver promise，把 phase 设为 running，并调用 loopCtx.agents.withInitiator(this, () => this.kick())。
- agent.ts:237-242：whenIdle 循环等待 activityDone，直到等待期间没有新的 activity 替换它。
- agent.ts:252-265：kick 用 while(await this.turn()) 驱动多轮；catch 吞掉已报告错误或取消；finally 把 running 收敛为 idle，并在 wakeRequested 且 inbox.hasPending 时再次 wakeDriver。
- agent.ts:298-311：turn 先检查 running，读取 signal，计算 turn=phase.turn+1，append turn/start，然后写回 phase.turn。
- agent.ts:312-318：turnEnds 初始为 null；target 初始为 next-turn；循环进入 preStep。
- agent.ts:319-322：preStep 返回 reject 时，turnEnds=blocked 并直接 return false。
- agent.ts:323-329：如果上一 step 已给出 turnEnds，而本次 preStep 的 messages 为空，则 break；如果 step 仍为 0 且 messages 为空，则 turnEnds=completed 并 return false，不消耗模型调用。
- agent.ts:331-332：进入 step 前 append step/start，并设置 phase.step。
- agent.ts:333-336：每个 step 创建一个 ToolCallRecovery，并订阅本 session 的 session/event 来观察 tool call/result。
- agent.ts:340-343：调用 step；max-tokens 是粘性的，后续 completed step 不能把 turnEnds 从 max-tokens 降级。
- agent.ts:344-355：step 抛错时，先把 toolRecovery.results() 逐条 append 为 tool/result，sourceEventSeqs 可附带；如果补写也失败，抛出 AggregateError。
- agent.ts:356-359：finally 停止 recovery 订阅并 append step/end。
- agent.ts:360-366：如果 turnEnds 已设置且 nextStep 为空，先运行 agent/turn-stopping 串行钩子，再检查一次；仍为空则 break；否则 target=next-step，继续同一 turn 的下一步。
- agent.ts:368-383：turn catch 通过 signal 的 cause 判断 aborted；否则把 LlmError.failure 原样保存，其他错误压成 UNKNOWN，并调用 throwError。
- agent.ts:384-390：finally 必须 append turn/end；append 失败会走 throwError。
- agent.ts:392-397：turn 结束后，如果 inbox 还有 pending，则换一个新的 AbortController、清 wakeRequested、step=0，并返回 true；否则返回 false。

结论：一个被唤醒的 turn 至少会打开 turn/start；是否产生 step 取决于 preStep 的 reject/空批处理。turn 内可以有多个 step；turn 之间有明确边界；driver 可以连续跑多个 turn。

### 4.3 单步状态

- agent.ts:400-407：step 只允许 running；取当前 turn/step/signal；renderPrompt(assembly)。
- agent.ts:409-421：step 内是 while(true) 重试循环；每次准备 request、刷新 runtime context、计算 startsRequestSeries，并让 SystemPromptProjection 产生 system/message commits。
- agent.ts:422-424：逐条 append system/message，intent 由投影决定。
- agent.ts:425-440：第一次尝试才把 decision.messages 写入 user/message；runtime-context 消息被合并成一条当前 snapshot；如果还没有 admitted，则在最后补一条。
- agent.ts:442：buildRequest 解析 request/header、request/context，冻结消息，并构造带 signal 的请求。
- agent.ts:443-450：创建 AssistantStreamAttempt，分配 attempt 序号和 stream revision。
- agent.ts:453-461：使用 preparedCall.stream 或 loopCtx.llm.stream；live.start 后逐 chunk push；每个 chunk 前检查 abort。
- agent.ts:462-503：流失败时，如果已 start，则按 abort 与否结算 interrupted assistant/message 或 assistant/attempt；结算失败与原错误合并为 AggregateError，再抛原错误。
- agent.ts:505-527：finish 为 error 或 aborted 时，先结算 assistant/attempt，再走 agent/request-error waterfall；只有 action.kind=retry 才 continue 同一 step，否则抛 LlmError。
- agent.ts:529-547：成功时组装 assistant/message，settle 后写 durable seq；finish=max-tokens 直接返回 max-tokens。
- agent.ts:549-555：从 message.content 过滤 tool-call；没有 tool call 返回 completed；有则 executeToolCalls，工具 concludeTurn 时返回 completed，否则返回 null。
- agent.ts:556-559：如果 live 尚未结束，abandon 后抛错。

结论：重试发生在同一 step 内，不会新增 step/start；一次 step 对应一个 assistant 消息或一个无 surface 的 assistant/attempt；工具调用是否结束 turn 由 concludeTurn 决定。

### 4.4 请求与持久化事件

- agent.ts:563-613：prepareRequest 读取持久 requestHeader；只在同一 provider/model 下恢复 reasoningEffort；合并 options；用 agent/request waterfall 产生 proposedConfig；校验 provider/model；prepareCall；NO_ADAPTER 时允许 middleware 继续。
- agent.ts:615-651：buildRequest 生成 canonicalHeader；首次 request/header 记为 initial 或 resume；后续 header 变化记为 change；显式新 series 记为 series，并可带 startsSeries。
- agent.ts:652-666：工具集变化时，把 tool-addition/tool-removal 作为 developer/message 记录，additions 时带 headerSeq。
- agent.ts:669-683：provider/model/contextWindow/systemPromptUpdate 变化时 append request/context。
- agent.ts:686-703：deepFreeze header 和边界消息；请求包含 config、messages、toolHistory、tools、sessionId、signal。
- types.ts:282-403：SessionEventMap 定义了 turn/start、turn/end、step/start、step/end、user/message、system/message、assistant/message、assistant/attempt、tool/call、tool/result、request/header、request/context 等持久事件。
- types.ts:289：turn/start 在 claim 输入或运行 pre-step 之前打开。
- types.ts:298：turn/end 的 reason 是 TurnEndReason；没有进入 step 的 turn 没有 step/start 或 step/end。
- types.ts:300-302：step/start 是一次 model call 加它请求的工具执行；step/end 关闭该 step。
- types.ts:342-350：assistant/message 带 usage；取消 mid-stream 时用 interrupted:true 保留已交付前缀。
- types.ts:356：assistant/attempt 保留 failed/retried/cancelled/stream-error 但没有 surface message 的尝试。
- types.ts:362：tool/call 保存模型原始 arguments 字符串。
- types.ts:376-386：tool/result 关联 callId，可带 error 和 UI meta；meta 必须 JSON 可序列化。

### 4.5 完成、取消、失败、恢复分支

完成：
- 无 tool call：step 返回 completed（agent.ts:550）。
- 工具 concludeTurn：step 返回 completed（agent.ts:551-555）。
- 正常完成但仍有 next-step：turn 不结束，target=next-step，继续下一步（agent.ts:361-366）。
- turn 正常结束且没有 pending：turn 返回 false，driver 回 idle（agent.ts:392）。
- max-tokens：step 返回 max-tokens，turn 以 max-tokens 结束；该结果在后续 step 中保持粘性（agent.ts:338-343, 547）。

取消：
- cancel 默认清空 inbox，并把当前 phase 的 wakeRequested 置 false，然后 abort（agent.ts:175-181）。
- keepInbox=true 时保留队列，只 abort（agent.ts:175-180）。
- 被 abort 的 turn 在 catch 中写成 aborted + 复制后的 cause（agent.ts:368-374）。
- 取消 mid-stream 时，已交付的可见块写成 interrupted assistant/message；没有可见块则写 assistant/attempt（agent.ts:465-493）。
- 取消后若还有 latched wake 和 pending，kick finally 会重新 wakeDriver（agent.ts:257-263）。
- cancel 在 idle 且无队列时是 no-op；测试 cancel.spec.ts:147 覆盖该语义。

失败：
- step 失败时先补未回答工具的保守结果，再关闭 step，再以 error 结束 turn（agent.ts:344-355, 357-358, 375-383）。
- LlmError 保留 failure；其他错误变成 message + UNKNOWN（agent.ts:375-382）。
- throwError 在 live 边界发出 agent/error（agent.ts:244-250）。
- step/end 的持久 append 失败也会从 finally 抛出，并被 turn catch 归为 error；这一步的 tool/result 可能已经落盘。

恢复：
- resume 读取物理日志后调用 interruptedTurnClosers(persisted)（index.ts:888-892）。
- repair.ts:53-98：扫描最后一个 open turn；先补未回答工具的 tool/result，再补 step/end，最后补 turn/end(interrupted)。
- repair.ts:105-198：ToolCallRecovery 从 assistant/message 的 tool-call 建立 pending，tool/call 记录 callSeq，tool/result 在 append 且 turn/step 匹配时移除。
- types.ts:216-229：interrupted 只由 resume 或冷读合成，loop 不会 live 发出；forked 只由 fork seed 构造产生。
- 测试 resume.spec.ts:479-500 断言 open turn 被补成 turn/end(interrupted)；resume.spec.ts:504-539 断言 tool/result、step/end、turn/end 的顺序和 sourceEventSeqs。

维护态：
- runMaintenance 只允许 idle 进入；设置 maintenance phase、独立 AbortController、activityDone（agent.ts:183-193）。
- finally 回到 idle，并在非 disposed、有 wakeRequested、有 pending 时 wakeDriver（agent.ts:194-203）。
- status 对 maintenance 报告 idle（agent.ts:140-142）。

## 5. 测试代码证据（S，仅阅读，未运行）

以下断言支持上面的静态结论，但不能替代真实运行：

- tests/loop.spec.ts:452-486：简单 turn 的边界顺序是 turn/start -> step/start -> step/end -> turn/end；首个持久事件是 agent/inbox/spliced；最后是 turn/end。
- tests/loop.spec.ts:488-521：一次工具调用产生两次模型请求；第二次请求包含 tool result；日志包含 tool/call 和 tool/result。
- tests/loop.spec.ts:1018-1053：tool 执行期间 steer 进入 next-step；steering 消息在第二个 step/start 之后、第二次请求之前被写入。
- tests/loop.spec.ts:1234-1254：agent/turn-stopping 可以 steer，使同一 turn 再跑一个 step。
- tests/loop.spec.ts:1257-1276：工具 concludeTurn 后只发一次模型请求，但结果仍持久化。
- tests/loop.spec.ts:1413-1436：mid-stream cancel 产生 aborted/user，并提交 interrupted assistant/message。
- tests/loop.spec.ts:1439-1456：max-tokens 作为 turn/end reason 持久化。
- tests/loop.spec.ts:1729-1756：step/end 的 session observer 抛错不会改变后续 step 或 completed 结果。
- tests/loop.spec.ts:1861-1885：模型错误同时出现在 agent/error 和 turn/end(error)。
- tests/cancel.spec.ts:192-216：keepInbox=true 时，已排队 work 在 abort 后保留，并在下一次 wake 执行。
- tests/cancel.spec.ts:218-240：abort 到 idle 窗口内的 waking send 会 latch，随后以 completed 执行。
- tests/cancel.spec.ts:473-491：cancel 清除 queued tail，只执行当前 turn。
- tests/cancel.spec.ts:494-546：assistant/message 之后取消会跳过工具执行，但补齐 tool/call 与 synthetic tool/result，下一轮可重放。
- tests/inbox.spec.ts:82-111：inbox 是共享投影；注销投影后读取会抛错。
- tests/inbox.spec.ts:114-136：越界 splice 和重复 id 在重建时被拒绝。
- tests/inbox.spec.ts:253-276：clear 以 next-step、next-turn 的顺序写 canceled splice。

## 6. 反证与容易被误读的点

1. claim 发生在 pre-step 之前。agent.ts:271 先取走消息，agent.ts:272 才 assemble；因此 pre-step reject、assemble 抛错或 pre-step 期间 abort 时，已 claim 的消息不会自动回到 inbox。默认 cancel 会清空 inbox；keepInbox 也不会恢复已经 claim 的消息。测试 cancel.spec.ts:164 的标题明确说 keepInbox 不恢复已 claim 的 work。
2. blocked 分支 return false。agent.ts:319-322 在检查 inbox.hasPending 之前直接返回；如果还有其他 next-turn，它们留在投影中，但本次 driver 不会自动再开一轮。是否存在其他 wake 触发取决于调用方。
3. 空批处理也会打开 turn。agent.ts:323-329 说明被移除的 waking message 或 pre-step 改写为空仍占用初始 turn 边界，但不消耗模型调用；turn 以 completed 结束。
4. turn/end 是 finally 必写项。agent.ts:384-390 在正常、blocked、aborted、error 路径都尝试写 turn/end；如果持久 append 本身失败，会走 throwError，而不是静默成功。此时日志可能出现 open turn，后续由 resume repair 处理。
5. max-tokens 是 turn 级粘性。agent.ts:338-343 不是“最后一个 step 决定 turn 结果”；先前 step 的 max-tokens 不会被后续 completed 覆盖。
6. step 内重试不新增 step/start。agent.ts:409 的 while 只在 agent/request-error 返回 retry 时 continue；turn/step 编号不变，assistant/attempt 记录失败尝试。
7. turn-stopping 不是 turn 结束事件。它是 turnEnds 已设置且 next-step 为空时的串行钩子；钩子可以 steer，随后同一 turn 继续下一步（agent.ts:361-366）。
8. maintenance 对外仍是 idle。agent.ts:140-142；如果有 UI 只看 status，无法仅凭 status 区分 maintenance 和真正空闲。

## 7. 未验证项（U）

1. U：pre-step reject 消耗已 claim 输入是否是产品预期；本卡只确认代码行为，没有找到对应的用户可见契约或运行验证。
2. U：blocked 且仍有 next-turn 时，是否有上层消费者负责再次 wake；代码路径本身不再调度。
3. U：真实持久化后端在 turn/end 或 step/end append 失败时的恢复效果；源码有 repair，但本卡未运行崩溃/断尾测试。
4. U：真实 Provider 的流式错误、retry 和 interrupted 前缀在非 mock adapter 下的行为；本卡只读了 mock 测试断言。
5. U：whenIdle 只等待 loop activity，不等待持久化 flush；types.ts:292-296 明确要求读取存储的消费者自行 flush。真实 checkpoint 时序未验证。
6. U：status 事件与 durable 事件的相对顺序在不同消费者中的可观察性；本卡未做 UI 或 IPC 运行观察。
7. U：fork seed 的 forked closer 与 resume 的 interrupted closer 在真实 fork 场景中的组合；本卡只追踪了 resume 路径，fork 细节属于 DSH-R2-06。

## 8. 关键源码引用（path:line）

| 引用 | 说明 |
| --- | --- |
| packages/core/agent-loop/src/agent.ts:42-50 | Phase 联合类型：idle/maintenance/running |
| packages/core/agent-loop/src/agent.ts:140-152 | status 与 setPhase 的对外状态语义 |
| packages/core/agent-loop/src/agent.ts:154-173 | send/followup/steer/inject 与 target 选择 |
| packages/core/agent-loop/src/agent.ts:175-181 | cancel 清队列或 keepInbox 后 abort |
| packages/core/agent-loop/src/agent.ts:214-235 | wakeDriver 与 running 建立 |
| packages/core/agent-loop/src/agent.ts:252-265 | kick 的 turn 循环与 idle 收敛 |
| packages/core/agent-loop/src/agent.ts:267-288 | preStep：claim、assemble、pre-step waterfall |
| packages/core/agent-loop/src/agent.ts:298-311 | turn/start 与 turn 编号 |
| packages/core/agent-loop/src/agent.ts:319-329 | blocked 与空批处理完成分支 |
| packages/core/agent-loop/src/agent.ts:331-343 | step/start、ToolCallRecovery、max-tokens 粘性 |
| packages/core/agent-loop/src/agent.ts:344-390 | 失败恢复、step/end、turn-stopping、turn/end |
| packages/core/agent-loop/src/agent.ts:400-461 | step 的请求、system/user 写入与流读取 |
| packages/core/agent-loop/src/agent.ts:462-527 | 流失败、interrupted 前缀、retry 分支 |
| packages/core/agent-loop/src/agent.ts:529-555 | assistant/message、max-tokens、tool-call 分支 |
| packages/core/agent-loop/src/agent.ts:615-703 | request/header、request/context 与冻结请求 |
| packages/core/agent-loop/src/inbox.ts:21-65 | inbox 投影 schema 与 splice 校验 |
| packages/core/agent-loop/src/inbox.ts:97-114 | clear 与 claim 的边界语义 |
| packages/core/agent-loop/src/inbox.ts:198-243 | 归一化 splice、重复 id 拒绝、持久事件 |
| packages/core/agent-loop/src/assistant-stream.ts:49-109 | start/push/settle/abandon |
| packages/core/agent-loop/src/index.ts:57-95 | turnBoundary 投影 |
| packages/core/agent-loop/src/index.ts:888-892 | resume 读取日志并补 interrupted closers |
| packages/core/session/src/types.ts:202-230 | TurnEndReasonMap 全部分支 |
| packages/core/session/src/types.ts:282-403 | 持久事件模型 |
| packages/core/session/src/repair.ts:53-98 | open turn 的合成关闭顺序 |
| packages/core/session/src/repair.ts:105-198 | ToolCallRecovery 的 pending 追踪 |

## 9. 命令与退出码

| 命令 | 退出码 | 用途 |
| --- | --- | --- |
| git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo rev-parse HEAD | 0 | 固定 commit |
| git -C ... describe --tags --exact-match HEAD | 0 | 确认 dsh-v0.2.1-rc.5 |
| git -C ... status --short | 0 | 源码树无已跟踪改动；输出为空 |
| wc -l packages/core/agent-loop/src/*.ts packages/core/agent-loop/tests/*.ts | 0 | 统计审计面 |
| nl -ba packages/core/agent-loop/src/agent.ts | 0 | 读取状态机 |
| nl -ba packages/core/agent-loop/src/inbox.ts | 0 | 读取队列投影 |
| nl -ba packages/core/session/src/types.ts | 0 | 读取 TurnEndReason 与事件模型 |
| nl -ba packages/core/session/src/repair.ts | 0 | 读取恢复 closers |
| rg -n 'turn|step|queue|abort|cancel|resume|finish|completed|tool' 报告 | 0 | 报告自检 |
| python3 -c 'json.load(open(...))' 读取 package.json | 0 | 确认包版本 |

未运行的命令：任何 npm/pnpm/yarn 安装、build、test、tsc、服务启动或模型调用。因此本报告没有 R 级或 M 级证据。

## 10. 边界

- 本报告不评价 Agent Loop 的设计优劣，不提出迁移、重构或改造建议。
- 本报告不分析并行工具调度的分组、上限和提交顺序；这些留给 DSH-R2-05。
- 本报告不分析 System Prompt 的贡献者排序和 Provider 序列化；这些留给 DSH-R2-01。
- 本报告不分析 Session Replay/Projection 的完整链路；这里只引用与 turn 结束和 resume 修复直接相关的部分。
- 本报告不读取真实会话正文、凭证或用户数据；所有结论来自 /tmp 源码与测试代码的静态阅读。

