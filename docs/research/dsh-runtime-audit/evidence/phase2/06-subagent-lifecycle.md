# DSH-R2-06 子 Agent 生命周期源码审计

- 任务卡：DSH-R2-06
- source_commit：b830ec950faba0a46dda564be3e2d854afed94f1
- tag：dsh-v0.2.1-rc.5
- source_path：/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo
- package_versions：以下包均为 0.2.1-rc.5（package.json 实测）
  - @deepseek-ai/dsh-tool-subagent（packages/subagent/tool-subagent）
  - @deepseek-ai/dsh-subagent（packages/subagent/subagent）
  - @deepseek-ai/dsh-subagent-spawn-in-process（packages/subagent/subagent-spawn-in-process）
  - @deepseek-ai/dsh-subagent-fork-in-process（packages/subagent/subagent-fork-in-process）
  - @deepseek-ai/dsh-session（packages/core/session）
  - @deepseek-ai/dsh-agent-loop（packages/core/agent-loop）
- 审计时间：2026-11-12
- 方法：只读源码静态审计；不启动子 Agent，不读取真实子 Agent 会话，不改源码。
  未安装依赖、未运行构建或测试。
- 证据等级：S 静态源码；D 官方 README（仅线索）；U 未验证。

---

## 0. 结论摘要

S：一次 delegation 的入口是 tool-subagent 注册的 subagent 工具。工具立即返回一个
activation id，真正的子 Agent 由 ctx.subagents（SubagentRuntime）下的 SubagentManager
创建、观察、结算和回收。spawn 与 fork 是两个 in-process provider：

- spawn：子 Agent 从零上下文开始（inheritsParentContext = false，prepareContinuable 返回空 spec）。
- fork：子 Agent 以父会话最近一个 turn/end 为止的已完成轮前缀作为 seed
  （inheritsParentContext = true），不包含正在进行的 turn。

S：这两条路径都不是 SessionStore.fork / buildForkSeed。fork provider 直接读父 session
snapshotEvents，切到最后一个 turn/end，把 seed 交给 agents.create；边界天然是平衡的，
因此不产生 forked 合成 closers。这是本卡最重要的反证点。

S：子 Agent 的模型默认继承父的最新 request header；工具默认来自父的 preset composition；
权限只继承受限子集（sandbox 显式覆盖、permission preset 的 auto/full、approval 固定 never）。
结果在清理完成后以 settlement 用户消息投递回父 inbox，属于异步通知，不是工具返回值。

---

## 1. 工具入口与参数

S：tool-subagent 在父 Agent 的 tool registry 中注册一个 delegation 工具。

- packages/subagent/tool-subagent/src/index.ts:53-77 —— provider、toolName、agentOptions、
  maxDepth 配置；toolName 默认 subagent。
- index.ts:81-98 —— Schemastery 校验：provider required、toolName、modelSelectionSettings、
  agentOptions（provider/model/reasoningEffort/maxTokens）、agentPreset、allowedAgentPresets、maxDepth。
- index.ts:147-150 —— persona 与 toolFilter 作为工具配置字段已被拒绝，要求改用 Agent Preset；
  这两个字段只保留在历史 descriptor 的冷恢复路径。
- index.ts:213-280 —— 工具参数：cwd、description、prompt、可选 agent_preset、
  可选 provider/model/reasoning_effort。
- index.ts:215-217 —— description 明确“立即返回 id，运行时在完成时通知”，
  所以工具调用不是阻塞等待子 Agent。
- index.ts:283-284 —— 解析 preset 与模型请求。
- index.ts:320-337 —— 解析 maxDepth、组装 request、startActivation、返回 { kind: 'activation', subagentId }。
- index.ts:330-336 —— request 含 cwd、prompt、parent、可选 agentPreset/agentOptions/maxDepth。

S：provider 在启动前做能力与配置校验：

- index.ts:166-179 —— 数值 maxDepth 需要 depthLimit；agentOptions 需要 agentOptions 能力；
  model selection 需要 agentOptions 能力。
- index.ts:158 —— 直接 apply 时也校验 maxDepth 类型。

---

## 2. 启动路径：Service → Manager → Provider

S：SubagentRuntime.startActivation 先校验 provider、能力、父 Agent 身份与 output schema，
再按 provider 是否实现 prepareContinuable 分流到本地或外部执行。

- packages/subagent/subagent/src/index.ts:196-197 —— 默认 maxDepth = 1，maxActiveSubagents = 8。
- index.ts:242-250 —— resolveMaxDepth：provider-managed 返回 undefined；显式值优先；否则读 Host 设置。
- index.ts:269-284 —— startActivation：expectProvider、reserved childId 需要本地 provider、
  assertSubagentMaxDepth、assertCapabilities、父 Agent 必须仍是 exact live instance、
  outputSchema 必须为对象 schema。
- index.ts:631-650 —— assertCapabilities：agentPreset 要求 provider 支持且不 inheritsParentContext；
  agentOptions/outputSchema/maxDepth 分别要求对应 capability。
- index.ts:283-284 —— 有 prepareContinuable 走 manager.startLocal，否则 manager.startExternal。

S：本地启动的关键顺序：

- packages/subagent/subagent/src/manager.ts:379 —— startLocal。
- manager.ts:383 —— continuable 子 Agent 需要 session persistence（caller delivery 允许查可选服务）。
- manager.ts:387 —— childDepth = resolveChildDepth(parent, request.maxDepth)。
- manager.ts:389-400 —— 在第一个 await 前快照 descriptor（mode: continuable、provider、label、
  agentProvider/agentModel/agentReasoningEffort）。
- manager.ts:404 —— 在第一个 await 前捕获委托策略（父后续切换不影响本子）。
- manager.ts:411-416 —— host.prepareContinuable 取得 seed；inheritedEventCount = seed.length。
- manager.ts:445 —— childSessionMeta(parent, childDepth, prepared.seed !== undefined, request.agentPreset)。
- manager.ts:459-466 —— submitMaterialized 提交初始 prompt 并记录 catalog 子项。

S：materializeTracked 的创建窗口：

- manager.ts:851-925 —— 若 create 存在则 agents.create，否则 agents.resume。
- manager.ts:880-897 —— setup：显式 preset 先 mount；新增子会话追加 agent-preset/selected、
  subagent/descriptor、委托策略、working directory；未显式选 preset 时 composeFrom 父。
- manager.ts:897-905 —— outputSchema 时 attachStructuredRuntime。
- manager.ts:928-963 —— Activation 记录 pool、releaseSlot、parent、delivery、result、
  closing、ancestry、ownedChildren、observer；resident.set；校验父仍 live；acquireOwnership；
  observer.start(child)。

S：生命周期事件的发布点：

- packages/subagent/subagent/src/lifecycle.ts:96-116 —— createActivationObserver；start 记录
  local 标记并发 subagent/start，settle 发 subagent/end。
- lifecycle.ts:89-95 —— 创建失败但未 residency 时不发任何 edge；避免报告一个不存在的生命周期。
- manager.ts:956 —— 只有 resident、父仍 live、ownership 获取成功后才 observer.start。

---

## 3. 上下文继承边界（spawn 与 fork 分开回答）

S：这是两套 provider，上下文差异来自 prepareContinuable 的 seed。

| 维度 | spawn（subagent-spawn-in-process） | fork（subagent-fork-in-process） |
| --- | --- | --- |
| inheritsParentContext | false | true |
| prepareContinuable | 返回 {}，没有 seed | 返回 completedTurnPrefix 的 seed（非空时） |
| 父对话可见范围 | 零条父消息 | 最近一个 turn/end 及之前的全部已完成轮 |
| 进行中的 turn | 不可见 | 明确排除（seed 截止到最后 turn/end） |
| 首次无已完成轮 | 从零开始 | seed 为空，也从零开始，但仍保留父 lineage |
| 独立 agentPreset | 支持（capability true） | 不支持（capability false，且 startActivation 拒绝） |
| 子会话 header | parentSession、origin subagent、delegationDepth、agentPreset | 同上；isSeeded 取决于 seed 是否存在 |

源码：

- packages/subagent/subagent-spawn-in-process/src/index.ts:38-46 —— 全能力 provider，
  inheritsParentContext = false。
- subagent-spawn-in-process/src/index.ts:50-54 —— prepareContinuable 返回 {}。
- packages/subagent/subagent-fork-in-process/src/index.ts:46-53 —— completedTurnPrefix：
  snapshotEvents，findLast turn/end，slice(0, lastEnd.seq + 1)。
- subagent-fork-in-process/src/index.ts:60-68 —— capabilities agentPreset: false，
  inheritsParentContext = true。
- subagent-fork-in-process/src/index.ts:72-78 —— 有 seed 才返回 { seed }，否则 {}。

S：fork 的 seed 是“已完成轮前缀”，不是 SessionStore.fork。

- subagent-fork-in-process/src/index.ts:5-6 注释 —— seed 截止到最后 turn/end，当前工具调用轮不平衡，
  不能作为有效子会话重放。
- subagent-fork-in-process/src/index.ts:72-77 注释 —— 前缀只在创建时捕获一次，之后成为子会话
  自有 durable transcript；冷恢复重放该前缀，而不是重新 fork 父的新历史。
- 对照 packages/core/session/src/fork.ts:21-29 —— SessionStore.fork 的 buildForkSeed 会追加
  session/end-seed 与 forked 合成 closers。subagent-fork-in-process 没有调用它。
- manager.ts:445 + packages/core/session/src/index.ts:695-697 —— 由于 seed 存在且
  header.isSeeded，Session 构造器会在 inherited cut 追加 inherited marker；
  inheritedEventCount 仍是 seed 长度。

S：子 Agent 只按自己的事件段产出结果，不会把父 seed 的输出当成自己的答案。

- packages/subagent/subagent/src/activation.ts:146-161 —— captureLocalResult 从
  activation.outputStart 开始读；stopReason 来自 epochStopReason，结构化缺失时把 completed 降为 error。
- subagent-fork-in-process/tests/subagent-fork-in-process.spec.ts:168-178 —— 测试明确覆盖
  “子没有自己的消息时不得返回 seed 的父输出”（测试代码阅读，未运行）。

---

## 4. 模型继承与覆盖

S：默认继承父的最新路由，允许逐次覆盖。

- packages/subagent/subagent/src/child-agent.ts:68-85 —— parentAgentOptionsForDelegation：
  优先父 session.requestHeader()?.config 的 provider/model/reasoningEffort；
  无 header 时回退 parent.options；maxTokens 保留创建选项。
- child-agent.ts:98-119 —— resolveChildAgentOptions：父 provider/model/effort/maxTokens 为底，
  requested 覆盖，最后盖上 subagentDepth；route 改变且未显式 effort 时删除继承 effort。
- packages/subagent/subagent/src/depth.ts:28-36 —— delegationDepthOf = max(header.delegationDepth,
  AgentOptions.subagentDepth)，header 是单调下限。
- tool-subagent/src/index.ts:286-315 —— 有模型请求或配置模型时，preflightChildLlmRoute
  校验路由；provider 在解析期间变化则拒绝。
- manager.ts:1329-1383 —— coldResume 用 descriptor 的 agentProvider/agentModel/
  agentReasoningEffort 重建路由，不重新 dispatch provider。

S：模型选择是可选能力，不是所有 provider 都支持。

- tool-subagent/src/index.ts:161-179 —— modelSelectionSettings 与 agentOptions 能力校验。
- manager.ts:1440-1454 —— 图片内容会按子 Agent 固定模型能力拒绝；不支持 image 时报错。

---

## 5. 工具与权限是否继承（分开回答）

S：工具继承自 preset composition，不是逐工具拷贝。

- child-agent.ts:135-154 —— childSessionMeta：selectedPreset 优先，否则 parent 当前 composedPreset。
- child-agent.ts:186-202 —— applyChildComposition：显式 agentPreset 时 mount 命名 preset；
  省略时 composeFrom 父 context。两者都调用 applyChildOverrides。
- child-agent.ts:204-218 —— applyChildOverrides：注入 SUBAGENT_DELEGATION_CONTEXT；
  历史 descriptor 的 persona 与 toolFilter 在此重放。
- child-agent.ts:160-164 —— toolFilter 只在“冷恢复历史 descriptor”时重新应用；
  新 delegation 的工具组成由 preset 决定。
- 关键差异：spawn 可独立选 preset；fork 的 provider capability agentPreset: false，
  且 index.ts:631-636 会拒绝带 agentPreset 的 fork 请求。

S：权限只继承受限子集，且子侧不能自行扩大。

- child-agent.ts:171-175 —— 子 Agent 的 delegation context：权限范围在启动时固定，
  需要审批的操作会自动拒绝，不能从会话内扩大。
- child-agent.ts:220-255 —— captureDelegatedPolicyOverrides：
  permissionPreset 只继承 auto/danger-full-access；sandboxMode 只取父 session 的显式 override；
  approvalPolicy 固定 never（有 approval 服务时）。
- child-agent.ts:266-279 —— 以 source: 'delegation' 追加 sandbox/mode、approval/policy、
  permission/preset，使子会话可从自己的日志重建策略。
- manager.ts:880-890 —— 这些事件在子会话创建窗口内、fork seed 之后追加，新策略覆盖旧 seed，
  之后子自己的切换仍可覆盖。

S：结论分开表述：

- 上下文：spawn 不继承父对话；fork 继承到最后 turn/end 的已完成轮，不继承进行中的轮。
- 工具：默认继承父 preset composition；spawn 可切换 preset，fork 不可。
- 权限：sandbox 显式 override、permission preset 的 auto/full 可继承；approval 固定 never；
  子不能扩大权限。

---

## 6. 深度限制

S：深度是持久化的单调预算，不是每次从 0 重算。

- depth.ts:28-36 —— max(header.delegationDepth, options.subagentDepth)。
- depth.ts:42-50 —— assertSubagentMaxDepth 只接受非负安全整数。
- child-agent.ts:49-58 —— resolveChildDepth = parent depth + 1；超过 maxDepth 抛
  SubagentDepthError；超出安全整数抛 RangeError。
- subagent/src/index.ts:196 —— Host 默认 maxDepth = 1。
- tool-subagent/src/index.ts:77 —— maxDepth 可为非负整数或 'provider-managed'。
- tool-subagent/src/index.ts:388-392 —— 达到上限时向模型注入“remaining levels 0”的上下文，
  但工具仍保持可见，由运行时拒绝。
- 测试线索：tool-subagent/tests/depth-context.spec.ts:18-23,51-58 —— 覆盖 depth 与 maxDepth
  矩阵，以及“运行时 depth 比历史 header 更深时取更深值”（测试代码阅读，未运行）。

S：默认值 1 的实际含义：顶层 depth 0 可创建 depth 1 子 Agent；
depth 1 再创建 depth 2 会超过默认上限 1 而被拒绝。

---

## 7. 生命周期图与终态

    subagent tool execute
        │  resolve preset / model / maxDepth
        ▼
    SubagentRuntime.startActivation
        │  capability + exact-parent + schema checks
        ├─ provider.prepareContinuable === undefined ──► manager.startExternal
        └─ provider.prepareContinuable !== undefined ──► manager.startLocal
                │  resolveChildDepth / snapshot descriptor / capture policy
                │  provider.prepareContinuable() -> seed
                │  agents.create (or agents.resume for cold continuation)
                │  setup: preset, descriptor, policy, cwd, structured runtime
                ▼
            Activation resident (resident.set)
                │  acquireOwnership(parent, childId)
                │  observer.start -> subagent/start
                ▼
            child runs (inbox -> turn/step -> LLM/tools)
                │  natural settlement watcher: whenIdle + no owned children + no pending inbox
                │  or parent dispose/cancel
                ▼
            finishDisposal
                │  cancel/abort descendants -> wait idle -> flush -> handle.dispose
                │  resident.delete, releaseSlot, releaseOwnership
                │  observer.settle -> subagent/end
                │  result.resolve(captured) / result.reject(capture failure)
                ▼
            notifySettlement -> parent inbox (queue/steer) or inject

S：终态映射来自子会话自己的日志，而不是“teardown 成功”。

- lifecycle.ts:118-159 —— epochStopReason：
  completed → completed；aborted/interrupted → aborted；error → error；
  blocked → refusal；max-tokens → max-tokens；无结束但有 dropped unrun → aborted；
  未识别 reason → error。
- activation.ts:130-141 —— observeLocalFailure 记录子失败位置；之后成功事件可清除该标记。
- activation.ts:146-161 —— captureLocalResult：failureAt 存在则 stopReason = error；
  结构化输出缺失时 completed 降为 error。
- manager.ts:1081-1092 —— settlementState：closing 返回 closed；owned children 或 pending inbox
  返回 wait；否则 ready。
- manager.ts:1024-1077 —— watchLocalSettlement：whenIdle 后在 child lock 下复检，
  最终通过 runMaintenance 关闭，避免与其他 idle 活动竞争。

---

## 8. 结果投递

S：结果不是工具返回值，而是异步 settlement 消息。

- manager.ts:1187-1207 —— notifySettlement：未 announced 或 delivery = caller 时不投递；
  父必须是 exact live instance；父正在 closing 时 inject，否则 sendWaking；
  投递异常只记日志，不让 teardown 失败。
- manager.ts:1221-1228 —— receipt 暴露 result promise 与 dispose，绑定 exact activation。
- packages/subagent/subagent/src/continuation-messages.ts:138-169 —— createSettlementMessage：
  summary + 子最后文本 + structured 结果 + diagnostic，source.kind = subagent-settled，
  form = notice。
- subagent/src/types.ts:47-58 —— SubagentActivation 契约：result 在 teardown 与通知后 settle；
  teardown 失败由 dispose 报告，不替换已捕获结果。
- manager.ts:1172-1183 —— terminal 仅用于 observer/通知；activation.result 仍 resolve
  captured result，除非 capture 本身失败；失败时的 dispose 另行 reject。

S：父侧还可以在运行中通过 send_message 与 continuable 子 Agent 通信；最终结果仍由
manager 的 settlement notice 送达。

- manager.ts:539-612 —— sendMessage/queuePrompt/steerPrompt。
- manager.ts:615-631 —— deliver：closing 或 structured 已提交时拒绝；steer 直接干扰，
  queue 进入 inbox。
- manager.ts:459-464 —— 若子存在相邻 send_message 工具，初始 prompt 会附加
  “完成后用 send_message 汇报”的 guidance。

---

## 9. 冷恢复与 continuation

S：continuable 子 Agent 的恢复不重新 dispatch provider，而是读取持久日志与 descriptor。

- subagent/src/descriptor.ts:48 —— SUBAGENT_DESCRIPTOR_VERSION = 3。
- descriptor.ts:55-89 —— mode: one-shot | continuable；continuable 记录 agentProvider、
  agentModel、agentReasoningEffort、persona、toolFilter。
- descriptor.ts:263 —— foldSubagentDescriptor 从事件序列折叠 descriptor。
- manager.ts:1329-1383 —— coldResume：requireSessionQuery，observeSession，authorizeLineage，
  取 inheritedEventCount 之后的 ownEvents，fold descriptor，要求 mode = continuable；
  独立 preset 从 projection 取；materialize 时用 descriptor 的模型与 persona/toolFilter；
  最后 submitMaterialized。
- manager.ts:1247-1284 —— deliverFollowup：resident 不存在时进入 coldResume；
  closing 时等待 disposal 后重试。
- manager.ts:1387-1416 —— submitMaterialized 失败时 dispose 并抛出，避免泄漏 activation。

S：continuable 需要 session persistence 与 session query；缺失时明确报错。

- manager.ts:1459-1481 —— requirePersistence / requireSessionQuery 的错误码
  PERSISTENCE_UNAVAILABLE / CONTINUATION_UNAVAILABLE。

---

## 10. 回收与取消

S：dispose 是 memoized 的 close 事务，先关闭准入，再停止/回收。

- manager.ts:770-781 —— dispose 调用 close；close 用 activation.closing 保证单次。
- manager.ts:1094-1185 —— finishDisposal：
  external：stop 时 controller.abort({ kind: 'parent' }) 并 run.dispose；
  local：stop 时 agent.cancel({ kind: 'parent' })，递归 dispose ownedChildren，
  中止同 lineage 的 materializations，等待 idle，flushFinalState；
  最后 handle.dispose，resident.delete，releaseSlot，observer.settle，resolve/reject result。
- manager.ts:1209-1214 —— flushFinalState 是 best-effort，失败只告警。
- manager.ts:807-839 —— closingScopes、liveLineage、closingTeardownFor 用于按 lineage
  关闭准入，防止关闭期间新子启动。
- manager.ts:169-176 —— agent/disposed 时按 scope 清理 resident 子项与 drain。
- manager.ts:235-247 —— drain：等待 materializations，关闭 roots，报告所有 teardown 失败。
- manager.ts:252-294 —— waitForChildren 只 join 不取消；空闲但 inbox 仍需唤醒的后代保持 resident。

S：teardown 失败与业务结果分开报告。

- manager.ts:1172-1183 —— teardown 失败构造 ACTIVATION_TEARDOWN_FAILED，但 observer 收到
  error 终态；captured result 仍 resolve（除非 capture failure）。
- subagent/src/types.ts:52-58 —— 契约明确：teardown 失败由 dispose() 报告，
  不替换已捕获结果。

---

## 11. 反证与容易误读的点

1. S：subagent 的 fork 不是 SessionStore.fork。它直接 slice 父日志到最后一个 turn/end，
   没有 buildForkSeed、没有 forked 合成 closers、没有 SessionForkError 边界校验。
   证据：subagent-fork-in-process/src/index.ts:46-53,72-78 对比 core/session/src/fork.ts:21-29。
2. S：fork 只继承已完成轮，不继承当前 turn；因此父正在执行的工具调用不会出现在子上下文。
   证据：subagent-fork-in-process/src/index.ts:49-52。
3. S：工具立即返回，完成通知异步到达；“工具返回 id”不等于“子任务已完成”。
   证据：tool-subagent/src/index.ts:215-217,330-337；manager.ts:1187-1207。
4. S：结果投递是 best-effort。父不再 live、父正在关闭、delivery = caller 时都不会有通知；
   投递异常只记日志。证据：manager.ts:1187-1207。
5. S：子不能扩大权限。approval 固定 never；sandbox 只复制父的显式 override；
   delegation context 明确告知子权限已固定。证据：child-agent.ts:171-175,248-255。
6. S：depth 是单调的持久预算。冷恢复时 header.delegationDepth 仍生效，
   即使新 options 从 0 开始。证据：depth.ts:28-36。
7. D：各包 README 对 spawn/fork 的描述与源码方向一致，但本报告不把 README 当实现证据。

---

## 12. 未验证项

1. U：未启动任何子 Agent，也未运行 subagent 测试；真实 agents.create/resume、whenIdle、
   runMaintenance 的竞态行为只有静态代码证据。
2. U：未审计外部 provider（ACP、Codex、Claude Code、DSH SDK 等）的 start/result/dispose
   语义；本卡只深入 in-process spawn/fork。外部 provider 的 inheritsParentContext 与
   结果 shape 需要单独卡片。
3. U：未逐行审计 ActivationPool 的排队、容量公平性与 releaseSlot 时机；只确认默认上限 8
   与 pool.reserve 调用点（manager.ts:744）。
4. U：未验证 coldResume 在真实 sessionQuery + projection 缺失时的表现；
   只确认缺失服务会报 CONTINUATION_UNAVAILABLE，descriptor 缺失会报 NOT_RESUMABLE。
5. U：未验证父子 dispose 与启动并发时的实际锁行为；源码有 ChildLock / closingScopes / holdOwnership，
   但没有运行证据。
6. U：未确认仓库其他调用方是否使用 SessionStore.fork 创建子 Agent；本卡只证明
   subagent-fork-in-process 不使用它。
7. U：structured_output 的提交与 PTC 嵌套执行细节未完整审计；
   只确认 attachStructuredRuntime 与 capture 提交规则的存在。

---

## 13. 下一张卡不要混入的边界

- 不判断 DSH 是否适合作为 OmniDesk 底座，也不比较 Pi。
- 不展开 Agent Loop 的完整轮次状态机（R2-02）与并行工具调度（R2-05）。
- 不把 in-process provider 的行为外推到 ACP/Codex/Claude 等外部 provider。
- 所有行号仅对应 b830ec950faba0a46dda564be3e2d854afed94f1。

---

## 14. 命令与退出码

以下命令均以只读方式执行；除特别注明外退出码为 0。

    git -C <source> rev-parse HEAD                      -> b830ec950faba0a46dda564be3e2d854afed94f1 (exit 0)
    git -C <source> describe --tags --exact-match HEAD  -> dsh-v0.2.1-rc.5 (exit 0)
    git -C <source> status --short                      -> 空 (exit 0)
    rg --files <subagent 目录>                           -> 文件清单 (exit 0)
    wc -l <相关源码>                                     -> 行数 (exit 0)
    cat -n / sed -n / rg -n <指定文件>                   -> 逐段读取 (exit 0)

未修改源码树，未执行 git 写操作。

---

## 15. 文件与边界

- 本报告是 DSH-R2-06 的唯一输出文件；与本轮 DSH-R2-03 报告合计两份。
- 源码树 /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo 只读。
- 未读取真实子 Agent 会话、凭证、会话正文或用户数据。

