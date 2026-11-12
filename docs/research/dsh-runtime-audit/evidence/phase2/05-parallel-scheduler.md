# DSH-R2-05：并行工具调度的分组、上限、顺序与取消

- step_id：DSH-R2-05
- generated_at：2026-11-12T10:19:56Z
- source_commit：b830ec950faba0a46dda564be3e2d854afed94f1
- source_tag：dsh-v0.2.1-rc.5
- source_path：/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo
- package_versions：
  - @deepseek-ai/dsh-agent-loop：0.2.1-rc.5
  - @deepseek-ai/dsh-tools：0.2.1-rc.5
  - @deepseek-ai/dsh-session：0.2.1-rc.5
- 证据等级：S = 静态源码或测试代码；D = README 或官方说明；U = 未验证。本报告没有用 D 级材料证明运行行为。
- 执行边界：只读 /tmp 源码；未安装依赖、未运行测试、未运行模型、未启动服务、未修改源码树或仓库 runtime/。

## 1. 单一动作与结论范围

本卡只追踪一个 assistant step 内多个 tool call 的分组和调度。结论集中在：

- parallel/exclusive 如何分类；
- 默认上限是多少；
- 滚动池如何补充；
- exclusive barrier 如何形成；
- 未启动调用如何重新分类；
- tool/call 与 tool/result 的开始/提交顺序；
- 取消时如何补 synthetic result；
- 调度器失败时如何停止和收敛；
- additional context 在哪里进入下一步。

本卡不分析 turn/step 状态机；那属于 DSH-R2-02。也不分析 Session Replay 或 subagent。

核心入口：

- packages/core/agent-loop/src/agent.ts:549-555：assistant message 过滤出 tool-call 后调用 executeToolCalls。
- packages/core/agent-loop/src/tool-calls.ts:60-102：executeToolCalls 按当前 mode 分组并串行推进 next。
- packages/core/agent-loop/src/tool-calls.ts:122-247：runGroup 实现 exclusive barrier、parallel 滚动池、模型序提交、取消和失败收敛。
- packages/core/agent-loop/src/constants.ts:5-6：DEFAULT_MAX_PARALLEL_TOOL_CALLS = 10。
- packages/core/agent-loop/src/index.ts:303-308 与 345-346：maxParallelToolCalls 配置、最小 1、步长 1、默认 10。
- packages/core/tools/src/index.ts:1304-1319：executionMode 的 fail-closed 分类。
- packages/core/tools/src/index.ts:466-475：prepare/dispatch/finalize/finish 的内部调度接口。

## 2. 调度模型

### 2.1 调用生命周期

一个 assistant step 内的每个 tool call 按以下阶段移动：

~~~text
model tool-call block
  -> PlannedCall { block, exec }              [tool-calls.ts:20-24, 72-81]
  -> executionMode(exec)                      [tool-calls.ts:89; tools/index.ts:1311-1319]
  -> group: parallel 组 或 exclusive 单例      [tool-calls.ts:90]
  -> startCall(index)
       -> append tool/call                     [tool-calls.ts:168]
       -> started++                            [tool-calls.ts:169]
       -> scheduler.prepare(exec)              [tool-calls.ts:170]
       -> dispatch 或 post-result/final-result [tool-calls.ts:172-196]
  -> slots[index] settled                      [tool-calls.ts:174-192]
  -> commitReady() 按 index 连续提交            [tool-calls.ts:147-160]
       -> finalize 或 finish                   [tool-calls.ts:153-154]
       -> append tool/result                   [tool-calls.ts:156]
       -> acceptContext(additionalContexts)    [tool-calls.ts:157]
  -> committed++                               [tool-calls.ts:159]
~~~

重要区别：tool/call 在 prepare 之前写入；dispatch 只覆盖工具体，prepare 中的 pre-execute/guard 仍然串行。tool-calls.ts:1-11 的模块注释也明确写了“Dispatch may overlap, while policy, results, and result context remain model-ordered”，但注释本身不作为行为证据，下面的代码与测试代码才是证据。

### 2.2 parallel 与 exclusive

- tools/src/index.ts:268-281：ToolDefinition.isConcurrencySafe 是纯同步分类器；只有精确返回 true 才允许 overlap；省略、异常、非 true 都按 exclusive。
- tools/src/index.ts:355-361：ToolExecutionMode 只有 parallel 和 exclusive 两种 kind。
- tools/src/index.ts:1304-1319：executionMode 先解析当前可见工具；没有分类器、未知工具、隐藏工具、分类器抛错或返回非 true 都返回 exclusive。
- tools/src/index.ts:811-814：ToolRuntime 的 maxParallelSubCalls 默认 10；这是 PTC run_code 子调用的上限，和 agent loop 的 maxParallelToolCalls 是两个配置。
- tool-calls.ts:88-90：组的第一项决定 mode；parallel 时取 planned.slice(next)，exclusive 时只取第一项。
- tool-calls.ts:204-205：parallel 组内，后续调用在开始前重新读取 executionMode；如果不再是 parallel，就停止补充，把它留给外层下一组。

因此，exclusive 的行为是“单独成组并形成顺序屏障”，不是“取消已经开始的 parallel 兄弟”。已经开始的 parallel 调用会先跑完并提交，exclusive 才在下一组开始。

### 2.3 默认上限

默认上限是 10：

- constants.ts:5-6：export const DEFAULT_MAX_PARALLEL_TOOL_CALLS = 10。
- index.ts:303-308：Config 注释说明省略时默认 DEFAULT_MAX_PARALLEL_TOOL_CALLS；1 表示串行。
- index.ts:345-346：z.number().step(1).min(1).default(DEFAULT_MAX_PARALLEL_TOOL_CALLS).volatile()。
- tool-calls.ts:132：每个 group 开始时读取 ctx.agentLoop.config.maxParallelToolCalls.get()。

上限的含义是“同时处于 dispatch 的调用数”，不是“本 step 的 tool call 总数”。tool-calls.ts:199-213 的 fillPool 条件使用 inFlight.size < maxParallelToolCalls；prepare 串行执行，dispatch 才进入 inFlight。

### 2.4 滚动池与 barrier

- tool-calls.ts:163：inFlight 是 index -> Promise 的 Map。
- tool-calls.ts:199-213：fillPool 在未 abort、还有未启动调用、inFlight 未满时继续启动；每次 startCall 后尝试 commitReady，再检查 abort。
- tool-calls.ts:219-231：先 fillPool，然后 Promise.race(inFlight.values())；任何一个 dispatch 结束就移除该 index、检查调度失败、提交连续就绪结果、再次 fillPool。
- tool-calls.ts:146-160：committed 只沿 index 连续前进；后面的 slot 先完成也不能越过前面的 slot 提交结果。
- tool-calls.ts:238-243：如果 abort 已发生，停止补充，等待 started 调用结算；然后为 group.slice(started) 补 synthetic call/result，并返回 aborted。
- tool-calls.ts:245：非 abort 的正常路径要求 committed === started；否则抛调度器不变量错误。

## 3. 4-call 示例时序表

示例：模型在同一 assistant step 依次给出 c1、c2、c3、c4：

- c1 = parallel-safe read
- c2 = parallel-safe read
- c3 = exclusive write
- c4 = parallel-safe read
- maxParallelToolCalls = 2

| 时刻/触发 | 调度器动作 | 事件或结果 | 顺序含义 |
| --- | --- | --- | --- |
| T0：executeToolCalls(next=0) | c1 的 mode=parallel，group 取 [c1,c2,c3,c4] | 尚未写事件 | 组的第一项决定这是 parallel 组 |
| T1：fillPool | 启动 c1：append tool/call c1，prepare，dispatch | tool/call:c1 | c1 进入 inFlight，占 1 个槽 |
| T2：fillPool | 启动 c2：append tool/call c2，prepare，dispatch | tool/call:c2 | c1/c2 的 dispatch 可以重叠 |
| T3：fillPool 检查 c3 | c3 的 executionMode=exclusive，break | 无 | c3 不进入本组，等待 barrier |
| T4：c2 先完成 | Promise.race 返回 c2；commitReady 发现 slot0 未完成 | 无 tool/result | 后完成的调用不能越过 c1 提交 |
| T5：c1 完成 | commitReady 连续提交 c1、c2 | tool/result:c1，然后 tool/result:c2 | 结果按模型序，而不是完成序 |
| T6：runGroup 返回 | consumed=2，aborted=false | 无 | 外层 next 推进到 c3 |
| T7：下一组 c3 | c3 单独成组；prepare/dispatch/commit | tool/call:c3，tool/result:c3 | exclusive barrier 在 c1/c2 提交后单独执行 |
| T8：下一组 c4 | c4 单独成组；prepare/dispatch/commit | tool/call:c4，tool/result:c4 | c3 完成后才启动 c4 |

这张表只描述静态代码允许的顺序，不是实测时序。测试 tool-calls.spec.ts:126-151 用 gated tool 断言了 exclusive barrier 的三组顺序；tool-calls.spec.ts:229-251 断言了“后调用先完成也不能先提交”；tool-calls.spec.ts:279-309 断言了 cap=2 时 c1、c2 先启动，c1 完成后才补 c3。

## 4. 顺序与上限细节

### 4.1 开始顺序

- tool-calls.ts:168：startCall 首先 append tool/call，返回的 seq 保存在 callSeqs[index]。
- tool-calls.ts:169-170：started 递增后才 await prepare。
- tool-calls.ts:171-196：prepare 返回 dispatch、post-result 或 final-result；dispatch 才进入 inFlight。
- tool-calls.ts:199-207：fillPool 串行调用 startCall，因此 pre-execute、guard 和 tool/call 写入按模型顺序发生；dispatch 体可以重叠。

结论：开始顺序是模型顺序；prepare 顺序也是模型顺序；只有 dispatch 体可能交错。

### 4.2 提交顺序

- tool-calls.ts:147-160：commitReady 只处理 slots[committed]，直到遇到 undefined 才停止。
- tool-calls.ts:153-154：需要 post-execute 的 slot 走 finalize；已经 final 的走 finish。
- tool-calls.ts:156：appendToolResult 使用 callSeqs[committed] 作为 sourceEventSeqs。
- tool-calls.ts:157：additionalContexts 在对应 result 提交后立即 accept；accept 的实际动作是 inbox.splice(next-step)，durable user/message 要到下一 step admission 才写入。
- tool-calls.ts:158-159：concluded 取 OR；committed 只在提交后递增。

结论：结果提交是模型顺序；上下文 accept 也是模型顺序，且位于对应 tool/result 之后。由于 accept 只是把消息排入 next-step，日志中的 user/message 会集中在全部 tool/result 之后、下一 step 打开之后出现。测试 tool-calls.spec.ts:230-251 与 383-409 分别覆盖结果顺序和上下文顺序。

### 4.3 上限的精确边界

- tool-calls.ts:200：条件只看 inFlight.size < maxParallelToolCalls。
- tool-calls.ts:163 与 174-184：只有 dispatch 的 promise 进入 inFlight。
- tool-calls.ts:187-192：post-result/final-result 直接填 slot，不进入 inFlight。
- tool-calls.ts:132：上限在每个 group 开始时读取一次，group 中途不重新读取。

因此 cap=10 不会限制“本 step 最多 10 个 tool call”；它限制同时 dispatch 的工具体数量。prepare 阶段本身是串行的，不占 inFlight 槽。配置为 1 时，第二次 dispatch 要等第一次 dispatch 结算并提交后才开始；测试 tool-calls.spec.ts:311-328 覆盖这一点。

### 4.4 exclusive 重新分类

- tool-calls.ts:204-205：在 parallel 组内，nextToStart > 0 时重新读取下一个调用的 mode；不是 parallel 就 break。
- tool-calls.ts:90：外层下一轮如果第一项是 exclusive，只取这一项。
- tool-calls.ts:94-95：next 按 consumed 推进；被重新分类的调用没有被消费，留给下一组。
- 测试 tool-calls.spec.ts:153-192：exclusive barrier 替换工具后，后续调用按新分类依次启动。
- 测试 tool-calls.spec.ts:194-226：结果观察器把下一个调用改成 exclusive 后，滚动池停止补充，等已启动调用结算后才启动 exclusive。

注意：重新分类发生在“开始前”，不是“运行中”。已经 dispatch 的调用不会因为后来注册表变化而被撤回。

## 5. 取消

### 5.1 abort 的调度路径

- agent.ts:551-554：executeToolCalls 收到当前 step 的 signal。
- tool-calls.ts:139：aborted 初始为 signal.aborted。
- tool-calls.ts:200：fillPool 在 aborted 时不再启动新调用。
- tool-calls.ts:221-231：已经 inFlight 的调用继续等待结算；每个结算后仍尝试 commitReady。
- tool-calls.ts:238-243：abort 收敛后，为 group.slice(started) 的每个调用 appendSkippedToolCall。
- tool-calls.ts:96-99：runGroup 返回 aborted 后，executeToolCalls 为 planned.slice(next) 的剩余调用补 synthetic，然后返回。
- tool-calls.ts:250-260：appendSkippedToolCall 先 append tool/call，再 append 一条 isError=true 的 tool/result；文本为 “Error: tool call aborted before dispatch”；error name=AbortError，code=TOOL_ABORTED_BEFORE_DISPATCH。

### 5.2 已启动与未启动

已启动：

- 已经 append tool/call 并进入 prepare/dispatch 的调用，会等待结算；其真实结果仍由 commitReady 按模型序提交。
- 如果工具体在 abort 后返回错误，结果仍是该调用的结果；调度器不会用 synthetic 覆盖已启动调用。

未启动：

- 未进入 startCall 的调用收到 synthetic tool/call + tool/result，配对保持 replay 可重建。
- c3 这样的 exclusive barrier 在 parallel 组 abort 后不会启动；测试 tool-calls.spec.ts:590-630 断言 exclusive 未执行，但 c3 仍有 synthetic 结果。

### 5.3 取消时上下文

- tool-calls.ts:157：只有已提交 result 的 additionalContexts 才进入 acceptContext。
- agent.ts:551-554：acceptContext 实际执行 inbox.splice(next-step, ...)，把上下文排到下一步。
- 测试 tool-calls.spec.ts:522-588：abort 后，已启动 c1/c2 的 additionalContexts 仍被接收并停在 next-step；未启动 c3/c4 只有 synthetic error，没有工具上下文。
- 测试 tool-calls.spec.ts:571-575 明确断言 next-step 中是 ctx-c1、ctx-c2。

结论：取消时，已启动调用的结果与上下文优先结算；未启动调用只补 synthetic error，不执行工具，也不产生工具上下文。

## 6. 调度器失败

### 6.1 失败路径

- tool-calls.ts:142-144：throwSchedulerFailure 在存在 schedulerFailure 时抛出第一个错误。
- tool-calls.ts:179-181：dispatch promise 的 reject 写入 schedulerFailure ??= { error }，不立刻打断其他已启动调用。
- tool-calls.ts:208、210、224、226：prepare、commitReady 或 race 之后检查并抛出第一个失败。
- tool-calls.ts:232-236：catch 中记录第一个失败，Promise.allSettled 等待所有 inFlight 结算，然后抛第一个失败。
- tool-calls.ts:245：正常路径若 committed !== started，抛 “tool-call scheduler: uncommitted settled calls”。

### 6.2 上层如何收尾

- agent.ts:344-355：step 捕获错误后，用 ToolCallRecovery.results() 为 assistant 中仍未回答的调用补保守结果；补写失败时抛 AggregateError。
- agent.ts:356-359：finally 关闭 recovery 订阅并 append step/end。
- agent.ts:375-383：turn 把调度器错误记录成 turn/end(error)，非 LlmError 压成 UNKNOWN。
- repair.ts:105-198：ToolCallRecovery 按 assistant/message 的 tool-call 建立 pending；tool/call 记录 callSeq；tool/result 在 append 且 turn/step 匹配时移除。
- 测试 tool-calls.spec.ts:633-723：execution-mode、prepare、finalize 三个阶段失败时，已提交结果保留，未提交调用分别得到 TOOL_NOT_STARTED 或 TOOL_OUTCOME_UNKNOWN，最后是 tool/result、step/end、turn/end。
- 测试 tool-calls.spec.ts:808-879：调度失败后先 drain 已启动 body，turn/end 在 drain 之后才出现；三个调用都得到 TOOL_OUTCOME_UNKNOWN。

结论：调度器失败的策略是“停止补充、等待已启动、抛第一个失败、由 step 补保守结果”。它不是“立即中断所有工具”或“自动重试”。

## 7. 测试代码证据（S，仅阅读，未运行）

- tests/tool-calls.spec.ts:109-124：三个 parallel-safe 调用全部在任何一个完成前启动。
- tests/tool-calls.spec.ts:126-151：parallel、exclusive、parallel 形成三组，顺序为 r-start-A1、r-end-A1、w-A2、r-start-A3、r-end-A3。
- tests/tool-calls.spec.ts:153-192：exclusive barrier 替换工具后，pending 调用重新分类。
- tests/tool-calls.spec.ts:194-226：结果观察器把下一个调用改成 exclusive 后，滚动池停止补充。
- tests/tool-calls.spec.ts:230-251：c2 先完成时不提交，c1 完成后按 c1、c2 提交 tool/result。
- tests/tool-calls.spec.ts:253-270：derived history 的 tool 结果仍按模型序配对。
- tests/tool-calls.spec.ts:279-309：cap=2 时先启动 c1、c2；c1 完成后启动 c3；结果仍为 c1..c4。
- tests/tool-calls.spec.ts:311-328：cap=1 时没有第二次 start 在第一次结算之前发生。
- tests/tool-calls.spec.ts:383-409：additional contexts 按模型序注入，且在所有 tool/result 之后。
- tests/tool-calls.spec.ts:459-488：signal 在 parallel 组前已 abort 时，不启动任何工具，两个调用都得到 ABORTED_BEFORE_DISPATCH。
- tests/tool-calls.spec.ts:490-520：pre-execute 期间 abort 时，不 dispatch，兄弟调用停止启动，两个调用都得到 synthetic。
- tests/tool-calls.spec.ts:522-588：abort 后已启动结果提交，未启动补 synthetic，已启动上下文停在 next-step。
- tests/tool-calls.spec.ts:590-630：parallel 组 abort 后，exclusive barrier 不执行，但仍有 synthetic 结果。
- tests/tool-calls.spec.ts:633-723：execution-mode/prepare/finalize 失败后的保守结果与 turn/end(error)。
- tests/tool-calls.spec.ts:808-879：dispatch 失败后先 drain，再出现 turn/end(error)。
- tests/execution-mode.spec.ts:27-67：只有精确 true 是 parallel；未声明、未知、false 都是 exclusive。

这些是测试代码断言，不是运行结果。本卡没有执行 vitest，也没有启动 mock adapter 之外的任何东西。

## 8. 反证与容易被误读的点

1. “上限 10”不是“每 step 最多 10 个 tool call”。tool-calls.ts:200 只看 inFlight.size；post-result/final-result 不进入 inFlight；prepare 本身串行。一个 step 可以有多于 10 个调用依次 prepare/commit，只要同时 dispatch 的数量不超过上限。
2. “started”不等于“dispatch 已开始”。tool-calls.ts:168-170 先 append tool/call、started++，再 await prepare。prepare 失败时该调用已经被记为 started，因此恢复结果是 TOOL_OUTCOME_UNKNOWN，而不是 TOOL_NOT_STARTED；测试 tool-calls.spec.ts:684-689 的 prepare 分支体现了这一点。
3. pre-execute 是串行的，不是并行的。tool-calls.ts:199-207 逐个 await startCall，而 startCall 在 170 行 await prepare；因此 policy/guard 观察顺序是模型序。只有 dispatch body 通过 inFlight 重叠。
4. Promise 完成顺序不是提交顺序。tool-calls.ts:147-160 的 committed 只能沿连续 index 前进；测试 tool-calls.spec.ts:230-251 用“c2 先完成”直接反证了“完成即提交”的误读。
5. exclusive 不是抢占式。tool-calls.ts:88-93 让 exclusive 单独成组；已经 dispatch 的 parallel 兄弟必须先结算并提交，exclusive 才开始。源码没有 abort 已启动兄弟的路径。
6. abort 不是立即返回。tool-calls.ts:221-231 仍等待 inFlight；tool-calls.ts:238-243 在 drain 后才补 synthetic。已启动工具如果忽略 signal，调度器会等待它结算。
7. 取消时的 synthetic 结果也有 tool/call。tool-calls.ts:250-260 先 appendToolCall 再 appendToolResult；不是只补一条孤立 result。
8. 调度器失败后不是所有调用都得到同一种错误码。已提交的真实结果保留；已 append tool/call 但无结果的调用是 TOOL_OUTCOME_UNKNOWN；从未 append tool/call 的调用是 TOOL_NOT_STARTED；测试 tool-calls.spec.ts:685-689 明确列出三种情况。
9. additionalContexts 的接收点绑定在 commit。tool-calls.ts:157 只在 result 提交后 accept；调度器失败或调用未提交时不会接收该结果的上下文。abort 时已启动并提交的调用仍会接收，测试 tool-calls.spec.ts:522-575。
10. executionMode 的分类器没有运行时纯度检查。tools/src/index.ts:1304-1319 只捕获抛错并 fail closed；分类器如果有副作用，源码不会阻止。工具定义把它描述为 pure（tools/src/index.ts:268-281），但这是契约，不是强制隔离。

## 9. 未验证项（U）

1. U：真实工具体的并发时序和资源竞争；本卡只读 gated mock 测试，没有运行并发实测。
2. U：真实 Provider 下同一步多个 tool call 的到达顺序是否总与模型消息中的 block 顺序一致；本卡从 message.content 过滤 tool-call，未验证 provider 流式组装的所有边界。
3. U：已启动工具忽略 abort signal 时的取消延迟；代码会等待 drain，但没有超时或强制终止路径。
4. U：调度器失败时，未提交结果的 additionalContexts 是否应该保留；代码路径不接收，产品语义未验证。
5. U：acceptContext 抛错（例如 next-step 重复 id）时，runGroup 会把它归为调度失败还是上层错误；本卡没有找到对应测试。
6. U：maxParallelToolCalls 是 volatile 配置；tool-calls.ts:132 每个 group 只读一次，运行中改配置对当前 group 的影响未验证。
7. U：PTC run_code 的 maxParallelSubCalls 与 loop 的 maxParallelToolCalls 叠加时的实际上限；两者都默认 10，但本卡没有追踪 PTC 子调度的完整路径。
8. U：executionMode 分类器在真实工具中是否存在副作用或昂贵计算；源码只按同步函数调用，没有隔离或缓存。
9. U：调度器失败与持久化 tool/result append 同时失败时的最终日志形态；agent.ts:344-355 有 AggregateError 路径，但本卡未运行该组合。

## 10. 关键源码引用（path:line）

| 引用 | 说明 |
| --- | --- |
| packages/core/agent-loop/src/agent.ts:549-555 | assistant step 过滤 tool-call 并调用 executeToolCalls |
| packages/core/agent-loop/src/tool-calls.ts:20-39 | PlannedCall、Slot、GroupOutcome |
| packages/core/agent-loop/src/tool-calls.ts:60-102 | executeToolCalls 外层分组与 abort 返回 |
| packages/core/agent-loop/src/tool-calls.ts:88-95 | 第一项决定 mode，next 按 consumed 推进 |
| packages/core/agent-loop/src/tool-calls.ts:122-144 | runGroup 初始化、cap 读取、失败持有 |
| packages/core/agent-loop/src/tool-calls.ts:147-160 | commitReady 的模型序提交与上下文 |
| packages/core/agent-loop/src/tool-calls.ts:163-197 | inFlight 与 startCall 的 prepare/dispatch |
| packages/core/agent-loop/src/tool-calls.ts:199-213 | 滚动池、cap、重新分类、abort 检查 |
| packages/core/agent-loop/src/tool-calls.ts:219-236 | race、drain、调度失败收敛 |
| packages/core/agent-loop/src/tool-calls.ts:238-246 | abort 的 synthetic 与正常返回值 |
| packages/core/agent-loop/src/tool-calls.ts:250-290 | appendSkippedToolCall、appendToolCall、appendToolResult |
| packages/core/agent-loop/src/constants.ts:5-6 | DEFAULT_MAX_PARALLEL_TOOL_CALLS=10 |
| packages/core/agent-loop/src/index.ts:303-308 | maxParallelToolCalls 配置说明 |
| packages/core/agent-loop/src/index.ts:345-346 | 配置校验：min 1、step 1、default 10 |
| packages/core/tools/src/index.ts:268-281 | isConcurrencySafe 契约 |
| packages/core/tools/src/index.ts:355-361 | ToolExecutionMode 联合 |
| packages/core/tools/src/index.ts:466-475 | ToolRuntimeScheduler 接口 |
| packages/core/tools/src/index.ts:687-694 | PTC maxParallelSubCalls 默认 10 |
| packages/core/tools/src/index.ts:811-814 | ToolRuntime 配置默认值 |
| packages/core/tools/src/index.ts:1304-1319 | executionMode fail-closed 分类 |
| packages/core/tools/src/schema.ts:503-508 | defineTool 的 isConcurrencySafe 选项 |
| packages/core/session/src/repair.ts:105-198 | ToolCallRecovery 与保守结果 |
| packages/core/session/src/repair.ts:157-197 | TOOL_NOT_STARTED / TOOL_OUTCOME_UNKNOWN 结果构造 |

## 11. 命令与退出码

| 命令 | 退出码 | 用途 |
| --- | --- | --- |
| git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo rev-parse HEAD | 0 | 固定 commit |
| git -C ... describe --tags --exact-match HEAD | 0 | 确认 dsh-v0.2.1-rc.5 |
| git -C ... status --short | 0 | 源码树无已跟踪改动；输出为空 |
| nl -ba packages/core/agent-loop/src/tool-calls.ts | 0 | 读取调度器 |
| nl -ba packages/core/agent-loop/src/constants.ts | 0 | 读取默认上限 |
| nl -ba packages/core/agent-loop/src/index.ts | 0 | 读取配置校验 |
| nl -ba packages/core/tools/src/index.ts | 0 | 读取 executionMode 与 scheduler 接口 |
| nl -ba packages/core/tools/src/schema.ts | 0 | 读取 isConcurrencySafe 定义 |
| rg -n 'DEFAULT_MAX_PARALLEL|parallel|exclusive|inFlight|commit|abort|synthetic|context|maxParallel' 报告 | 0 | 报告自检 |
| python3 -c 'json.load(open(...))' 读取 package.json | 0 | 确认包版本 |

未运行的命令：任何 npm/pnpm/yarn 安装、build、test、tsc、服务启动或模型调用。因此本报告没有 R 级或 M 级证据。

## 12. 边界

- 本报告不评价调度器设计优劣，不提出迁移、重构或改造建议。
- 本报告不分析 turn/step 状态机；turn 如何进入 step、如何结束、如何取消属于 DSH-R2-02。
- 本报告不分析 Session Replay/Projection 的完整链路；只引用 ToolCallRecovery 与 tool/call、tool/result 的持久配对。
- 本报告不分析 subagent 的工具隔离；相关边界留给 DSH-R2-06。
- 本报告不读取真实会话正文、凭证或用户数据；所有结论来自 /tmp 源码与测试代码的静态阅读。

