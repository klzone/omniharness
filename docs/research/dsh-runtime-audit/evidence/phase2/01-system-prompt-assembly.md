# DSH-R2-01：System Prompt 的组装与请求序列化

- step_id: `DSH-R2-01`
- generated_at: `2026-11-12T10:20:18Z`
- source_commit: `b830ec950faba0a46dda564be3e2d854afed94f1`
- source_path: `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo`
- source_tag: `dsh-v0.2.1-rc.5`
- package_versions: `@deepseek-ai/dsh-system-prompt@0.2.1-rc.5`、`@deepseek-ai/dsh-agent-loop@0.2.1-rc.5`、`@deepseek-ai/dsh-llm-pi-ai@0.2.1-rc.5`、`@deepseek-ai/dsh-llm-deepseek@0.2.1-rc.5`
- 证据口径：本轮只做静态源码与测试代码阅读；没有安装依赖，没有运行测试、服务或模型，也没有观察 Provider 的实际 wire body。

## 1. 结论摘要

1. **一次 Agent step 的 prompt 在 `agent/pre-step` 前组装一次，随后在每次请求尝试中复用。** `ReactLoopAgent.preStep()` 调用 `systemPrompt.assemble()`，并把返回的 `PromptAssembly` 存进 `PreparedStep`；`step()` 只在每个 attempt 调用 `refreshContext()`，没有重新执行 section provider、tool provider 或 `system-prompt/assemble` waterfall。`S`（`packages/core/agent-loop/src/agent.ts:267-288`、`:406-411`；`packages/core/system-prompt/src/index.ts:582-602`）。
2. **组装输入是 sections、contexts、tools、variables 四个独立集合。** Sections 进入 system prompt 文本；contexts 渲染成单独的 runtime-context snapshot，最后作为 user 角色消息进入历史；tools 以独立的 `ToolSchema[]` 进入请求，不会被自动拼进 system prompt 正文；variables 只在 render 阶段插值。`S`（`packages/core/system-prompt/src/index.ts:120-129`、`:290-318`、`:614-688`；`packages/core/agent-loop/src/agent.ts:407-413`）。
3. **Section 排序是确定的：先按 `order` 升序，再按名称的 code-unit 顺序；同名 section 在同一个 layer 内注册会抛错，scoped section 会遮蔽同名 global section。** `S`（`packages/core/system-prompt/src/index.ts:247-250`、`:470-486`、`:630-632`；`packages/core/system-prompt/tests/scoped.spec.ts:33-44`）。
4. **渲染时会丢弃空 section，用两个换行连接非空 section，并严格解析 `{{variable}}`。** 未知变量、已注册但值为 `undefined` 的变量、或带后续 `}}` 的畸形引用都会抛错；`interpolate: false` 保留字面文本，替换后的值不会被二次扫描。`S`（`packages/core/system-prompt/src/index.ts:281-295`、`:339-377`）。
5. **`complete: true` section 是逻辑上的完整 system prompt：assembly 仍运行 waterfall，以便解析 contexts、tools、variables，但 waterfall 返回后只保留该 complete section。多个生效的 complete section 会令 assembly 失败。** `S`（`packages/core/system-prompt/src/index.ts:52-76`、`:652-687`；测试代码 `packages/core/system-prompt/tests/system-prompt.spec.ts:497-523`，未执行）。
6. **Loop 自己构建的请求把 system prompt 放在 `messages` 里的 `system` 角色，`GenerateOptions.system` 保持未设置。** 请求构造只带 `header.config`、`deriveMessages()`、`toolHistory`、`header.tools`、`sessionId` 和 `signal`，没有单独的 `system` 字段。`S`（`packages/llm/llm/src/types.ts:528-550`；`packages/core/agent-loop/src/agent.ts:616-641`、`:686-703`）。
7. **Provider 适配层会把“请求级 system 文本 + 开头连续的 system 历史”合并成一个 provider system 槽；非开头的后续 system 消息则按历史位置处理。** pi-ai 路径：`splitSystemPrompt()` 合并 `options.system` 和前导 system messages，再从 `messages` 中删除这些前导项；DeepSeek 路径：非 `in-history` 时把 leading system 文本记录到 `historySystem`（同一 leading 段若有多个 system 消息，后一个覆盖前一个），`in-history` 时把后置 system 消息作为 `systemUpdates`，在下一条 assistant 前插入。`S`（`packages/llm/llm-pi-ai/src/context.ts:147-167`、`:224-256`；`packages/llm/llm-deepseek/src/serialize.ts:80-130`、`:151-167`）。
8. **空 system content 不是“没有 system 节点”：第一次 admission 即使渲染为空，也会追加一个空的 `system/message` 占住 surface node 0；provider 看到的是没有 system 槽，而不是空字符串。** `S`（`packages/core/agent-loop/src/runtime-context.ts:88-103`；测试代码 `packages/core/agent-loop/tests/loop.spec.ts:632-650`，未执行）。
9. **重复或变化的 system 文本由 `SystemPromptProjection` 决定追加还是改写。** 不具备 `systemPromptUpdate: 'in-history'` 的路由、新的 request series、或渲染为空时，会把有效文本归并到第一个 system node，并清空后续非空 system node；具备 in-history 的延续序列会追加新的非空 system node；文本未变则不写入事件。`S`（`packages/core/agent-loop/src/runtime-context.ts:82-110`；`packages/core/agent-loop/src/agent.ts:415-424`）。

## 2. 调用链

```text
Agent step
  -> systemPrompt.assemble({ agent, scope: agent, signal })
       -> merge global + scope-chain layers
       -> evaluate variables (global first, nearest scope last)
       -> collect sections / contexts / tool schemas
       -> sort sections by order then name
       -> sort/order tool schemas
       -> run scope-filtered 'system-prompt/assemble' waterfall
       -> restore one complete section, if present
       -> apply runtime-context suppression
  -> renderPrompt(assembly)
       -> interpolate variables per section
       -> drop empty sections
       -> join with '\n\n'
  -> prepareRequest() resolves provider/model and route capabilities
  -> refreshContext(assembly) for every attempt (contexts only)
  -> SystemPromptProjection.project(renderedPrompt, routeFacts)
  -> append zero or more 'system/message' surface events
  -> buildRequest(): header.config + messages + toolHistory + header.tools
  -> provider adapter
       -> pi-ai: splitSystemPrompt() / toPiContext()
       -> DeepSeek: serialize()
```

## 3. 关键源码证据

| 结论 | 证据等级 | 源码位置 |
| --- | --- | --- |
| Assembly context 包含可选 `scope` 和本次 turn 的 `signal` | S | `packages/core/system-prompt/src/index.ts:41-50` |
| Section 有 `name`、`order`、静态或函数 `text`、`interpolate`、`complete` | S | `packages/core/system-prompt/src/index.ts:52-76` |
| Context 是独立的动态 material，`required` 可在 suppression 下保留 | S | `packages/core/system-prompt/src/index.ts:78-90` |
| `PromptAssembly` 明确分开 sections、contexts、tools、variables | S | `packages/core/system-prompt/src/index.ts:120-129` |
| Section 与 context 的集中 order 表 | S | `packages/core/system-prompt/src/index.ts:131-179` |
| 变量名规则为 `[a-z][a-z0-9_]*` | S | `packages/core/system-prompt/src/index.ts:195-199` |
| `orderTools()` 对未配置的 tool 名称排序，配置顺序要求且只允许一个 rest 标记 | S | `packages/core/system-prompt/src/index.ts:201-240` |
| Section 先 order、后 name 排序；名称比较是 code-unit 比较 | S | `packages/core/system-prompt/src/index.ts:242-250` |
| `renderPrompt()` 逐 section 插值、丢弃空文本、双换行连接 | S | `packages/core/system-prompt/src/index.ts:281-295` |
| Runtime context 用固定的 `Current runtime context...` 前缀并双换行连接 | S | `packages/core/system-prompt/src/index.ts:302-318` |
| 严格插值：未知、无值、畸形引用均抛错；替换值不递归扫描 | S | `packages/core/system-prompt/src/index.ts:339-377` |
| 每个 scope 的 section/context/variable 注册都有同名重复检查 | S | `packages/core/system-prompt/src/index.ts:394-415` |
| 构造函数注册 `harness:identity`、persona prefix、persona suffix | S | `packages/core/system-prompt/src/index.ts:445-468` |
| `section()` 拒绝非有限 `order`，并通过 scoped layer 注册 | S | `packages/core/system-prompt/src/index.ts:470-486` |
| `context()` / `suppressRuntimeContext()` / `tools()` / `variable()` 的注册入口 | S | `packages/core/system-prompt/src/index.ts:507-570` |
| `refreshContext()` 只刷新 contexts，保留 accepted sections/tools/variables | S | `packages/core/system-prompt/src/index.ts:572-602` |
| `assemble()` 的变量覆盖、section/context 合并、tool 收集与排序 | S | `packages/core/system-prompt/src/index.ts:614-676` |
| Waterfall 后恢复 complete section，并在 suppression 下只保留 required contexts | S | `packages/core/system-prompt/src/index.ts:677-687` |
| Loop 将 provider/model 声明为 prompt variables | S | `packages/core/agent-loop/src/index.ts:389-390` |
| `assembleContextFor()` 同时设置 `agent` 和 `scope` | S | `packages/core/agent/src/dispatch.ts:167-176` |
| `preStep()` 组装 prompt，并让 runtime-context 先作为用户消息候选 | S | `packages/core/agent-loop/src/agent.ts:267-288` |
| 每个 step 只 render 一次，再在每个 attempt 刷新 context | S | `packages/core/agent-loop/src/agent.ts:400-424` |
| Request 构造使用 `header.tools`，并显式不含 `system` 字段 | S | `packages/core/agent-loop/src/agent.ts:616-641`、`:686-703` |
| `SystemPromptProjection.project()` 的 append/replace/clear 规则 | S | `packages/core/agent-loop/src/runtime-context.ts:82-110` |
| Runtime-context snapshot 只在内容变化时产生，并保留 named sections | S | `packages/core/agent-loop/src/runtime-context.ts:113-162` |
| pi-ai 把请求级 system 与前导 system 历史合并，随后从历史移除前导项 | S | `packages/llm/llm-pi-ai/src/context.ts:139-167` |
| pi-ai 后续 system 历史仍按位置转成 pi-ai 的 system message | S | `packages/llm/llm-pi-ai/src/context.ts:220-256` |
| DeepSeek 非 in-history 使用 `historySystem`，in-history 使用 `systemUpdates` | S | `packages/llm/llm-deepseek/src/serialize.ts:80-130` |
| DeepSeek 最终以 `system` 字段发送合并后的文本 | S | `packages/llm/llm-deepseek/src/serialize.ts:151-167` |

## 4. 组装细节

### 4.1 Section 注册、排序、去重与作用域

- `section()` 只检查 `order` 是否为有限数，随后把定义写入调用方 context 所属的 scoped layer。`S`（`packages/core/system-prompt/src/index.ts:478-486`）。
- 同一 layer 内同名注册由 `NamedEntries` 的构造错误抛出；global 层和 scoped 层各自有独立错误文案。`S`（`packages/core/system-prompt/src/index.ts:406-415`）。
- Assembly 先 `merge(scope, layer => layer.sections)`，再按 `order` 和名称排序。Scoped 同名值覆盖 global 值，然后才进入排序。`S`（`packages/core/system-prompt/src/index.ts:630-652`）。
- 测试代码显示，scoped section 遮蔽 global 同名 section 时，global 的 text provider 不会被调用。该测试本轮没有执行，只作为静态断言线索。`S`（`packages/core/system-prompt/tests/scoped.spec.ts:66-79`，未执行）。

### 4.2 模板变量与 render

- `assemble()` 对 global variables 先求值，再按 scope chain 从远到近求值，因此最近 scope 的同名变量获胜。`S`（`packages/core/system-prompt/src/index.ts:619-629`）。
- `renderPrompt()` 对每个 section 执行 `interpolate()`；`interpolate: false` 原样保留。`S`（`packages/core/system-prompt/src/index.ts:290-295`）。
- `interpolate()` 用 `Object.hasOwn()` 检查变量，避免 `constructor`、`__proto__` 这类原型链名称被当作已注册变量。`S`（`packages/core/system-prompt/src/index.ts:360-372`）。
- 如果一个 group 没有闭合，但后面存在 `}}`，会被判为 malformed；替换后的 value 不会被再次扫描。`S`（`packages/core/system-prompt/src/index.ts:348-376`）。

### 4.3 工具 schema 注入

- Tool providers 在 assembly 期间各自返回 `{ schemas, knownNames? }`；assembly 将每个 schema 复制为只含 `name`、`description`、`parameters`、`deferLoading` 的 `ToolSchema`，并 `structuredClone` 参数。`S`（`packages/core/system-prompt/src/index.ts:634-651`）。
- 工具不是 section 文本的一部分；`PromptAssembly.tools` 是独立字段，`renderPrompt()` 只读取 `sections`。`S`（`packages/core/system-prompt/src/index.ts:120-129`、`:290-295`、`:668-675`）。
- `ToolRuntime` 构造时注册 `ctx.systemPrompt.tools(context => this.wireSchemas(context.scope))`，因此 registry 的 scoped visible set 进入 assembly。`S`（`packages/core/tools/src/index.ts:850-861`）。本卡只追踪注入点，完整的 tool schema 生成与 Provider 序列化见 `04-tool-schema.md`。

### 4.4 Waterfall 与 complete section

- `assemble()` 构建 `PromptAssembly` 后调用 scope-filtered `system-prompt/assemble` waterfall；waterfall 返回值为权威值。`S`（`packages/core/system-prompt/src/index.ts:124-129`、`:677-680`）。
- 若存在一个 `complete: true` section，waterfall 仍执行；之后只保留这个 section，`contexts` 和 `tools` 仍保留 waterfall 的结果。`S`（`packages/core/system-prompt/src/index.ts:652-687`）。
- 多个 complete section 会抛错。`S`（`packages/core/system-prompt/src/index.ts:653-656`）。
- 测试代码断言：waterfall listener 即使修改 complete section 的 `text` 并追加 late section，最终仍恢复原始完整 section。该测试未执行。`S`（`packages/core/system-prompt/tests/system-prompt.spec.ts:497-513`，未执行）。

### 4.5 每步重建与缓存边界

- `preStep()` 每个 step 都调用 `assemble()`；`step()` 的重试循环不会重新调用 `assemble()`，只调用 `refreshContext()`。`S`（`packages/core/agent-loop/src/agent.ts:267-288`、`:406-411`）。
- `refreshContext()` 只重新求值已注册 contexts，并把只由 waterfall 添加的 context 保留；sections、tools、variables 保留 accepted assembly。`S`（`packages/core/system-prompt/src/index.ts:582-602`）。
- 测试代码断言 `refreshContext()` 后 sections/tools/variables 是同一个对象引用，context provider 只重新求值。该测试未执行。`S`（`packages/core/system-prompt/tests/system-prompt.spec.ts:54-85`，未执行）。

### 4.6 请求前的 surface 投影

- 第一次 admission 没有 system node 时，`project()` 总是返回一个 append，即使 `rendered === ''`。因此空 prompt 会生成空的 `system/message` 节点，占住 node 0。`S`（`packages/core/agent-loop/src/runtime-context.ts:88-93`）。
- 不具备 in-history、进入新 series、或 render 为空时：先清空后续非空 system node，再按需改写第一个 node。`S`（`packages/core/agent-loop/src/runtime-context.ts:94-100`）。
- 具备 in-history 且仍处于同一 series 时：最新有效文本没变则不写事件；非空变化则 append 新的 system node。`S`（`packages/core/agent-loop/src/runtime-context.ts:101-102`）。
- Loop 在 `prepareRequest()` 之后才决定 `inHistory` 与 `startsSeries`；tool schema 变化在路由没有 `toolUpdate` 时也会触发 series 重开。`S`（`packages/core/agent-loop/src/agent.ts:410-421`）。

### 4.7 Provider 前后 system 消息

- pi-ai 的 `splitSystemPrompt()` 将 `options.system` 与从索引 0 开始连续的 system messages 合并为 provider 的 `systemPrompt`，并把这些前导消息从 `messages` 中切掉。`S`（`packages/llm/llm-pi-ai/src/context.ts:147-157`）。
- 非前导的 system 消息不会进入该合并逻辑；它们在 `appendSharedHistory()` 中按原位置转成 pi-ai system message。`S`（`packages/llm/llm-pi-ai/src/context.ts:224-231`、`:240-256`）。
- DeepSeek 适配器对 `inHistory` 模型把后置 system 消息放入 `systemUpdates`，并要求前一条 wire message 是 user/tool-result；否则返回 unsupported。`S`（`packages/llm/llm-deepseek/src/serialize.ts:83-112`）。
- 如果没有 `in-history`，DeepSeek 将 leading system 文本写入 `historySystem`；同一 leading 段出现多个 system 消息时，后一个覆盖前一个。最终 `historySystem` 与 `options.system` 合并成 `system` 字段。`S`（`packages/llm/llm-deepseek/src/serialize.ts:106-116`、`:151-167`）。

## 5. 空内容、重复内容与可观察边界

| 情形 | 源码行为 | 等级 |
| --- | --- | --- |
| 所有 section 为空 | `renderPrompt()` 返回 `''`；第一次 admission 仍 append 空 system node | S |
| 首次请求无 system node、render 非空 | append 一个 system node | S |
| 首次请求无 system node、render 为空 | append 空 system node，provider 不发送 system 槽 | S |
| 同一 in-history series、prompt 未变 | 不写 system 事件 | S |
| 同一 in-history series、prompt 改变且非空 | append 新的 system node | S |
| 不具备 in-history 或 series 重开 | 清空后续非空节点，改写第一个节点 | S |
| render 为空且已有多个有效 system node | 每个非空后续节点写空 replacement，head 按需写空 | S |
| 多个 `complete: true` section | assembly 抛错 | S |

## 6. README 线索（D，不作为行为证明）

- `packages/core/system-prompt/README.md:12` 声称该 package 让 agent 获得一个有序 system prompt 和每个 model step 的 tool schemas；这只是设计说明。`D`
- `packages/core/system-prompt/README.md:100` 描述 `assemble()` 合并 global/scoped layer、detach tool parameters、排序、运行 waterfall、恢复 complete section；与源码结构一致，但本轮以源码为准。`D`
- `packages/core/system-prompt/README.md:140` 描述 system prompt 作为 surface node 进入 model history、空 rendering 清除所有 active system node；与源码静态路径一致，但未做运行验证。`D`
- `packages/core/agent-loop/README.md:120` 与 `:122` 描述 prompt admission 和 route/series 规则；只作为阅读线索。`D`

## 7. 反证与限制

1. README 的“一个 system prompt”指的是逻辑上的有效 prompt；源码允许 surface 上存在多个 `system/message` 节点，特别是在 `systemPromptUpdate: 'in-history'` 的延续序列中。不能把“一个 prompt”误读成“一条 system 消息”。`S`（`packages/core/agent-loop/src/runtime-context.ts:88-103`）。
2. 测试代码中的空 assembly、in-history、resume 断言只是静态测试代码，本轮没有执行，因此不能标成 `R` 或 `M`。`S`（测试代码未执行）。
3. Waterfall 可以修改 assembly；section 和 tools 的确定性只覆盖 registry 收集后的排序，waterfall listener 追加或重排的部分由其自身保证。`S`（`packages/core/system-prompt/tests/tool-order.spec.ts:81-95`，未执行；源码 `packages/core/system-prompt/src/index.ts:677-687`）。
4. `refreshContext()` 只刷新 contexts，不会重新求值 sections/tools/variables；因此 step 内变化的 section provider 不会在该 step 的重试中生效。`S`（`packages/core/system-prompt/src/index.ts:582-602`）。
5. `renderPrompt()` 会丢弃空 section，所以“注册了几个 section”与“最终发送了几个 section”不是同一数量。`S`（`packages/core/system-prompt/src/index.ts:290-295`）。
6. pi-ai 的 `splitSystemPrompt()` 会把开头连续的多个 system 消息用双换行连接；DeepSeek 非 `in-history` 路径的 `historySystem` 是覆盖赋值。正常 loop 通常只留下一个 leading system node，但两个 adapter 对“多个 leading system 消息”的退化行为不同，不能把其中一个当作另一个的证明。`S`（`packages/llm/llm-pi-ai/src/context.ts:148-157`；`packages/llm/llm-deepseek/src/serialize.ts:106-116`）。
7. 如果以后构造出“非开头 system 消息 + 不支持后置 system 的 route”，行为取决于对应 adapter 的能力声明，本卡未验证所有 adapter。`U`。

## 8. 未验证项

1. **未读取 OmniDesk 的产品 Profile、persona、插件组合和实际 `systemPrompt` 配置**，因此不知道真实部署注册了哪些 sections、contexts、tools，也不声称其顺序与本次源码默认值相同。`U`
2. **未观察实际 Provider 请求体**；pi-ai 和 DeepSeek 的字段映射来自源码，未用真实网络请求或 fixture 运行验证。`U`
3. **未验证 OmniDesk 当前模型的 `systemPromptUpdate` 能力**；`'in-history'` 是否生效取决于 `preparedCall.systemPromptUpdate`。`U`
4. **未验证 `refreshContext()` 在高并发取消、scope dispose、waterfall 重入下的行为**；本轮只读同步源码与测试代码。`U`
5. **未验证 KV cache 或 token 成本**；README 的 cache/token 描述属于 `D` 级设计说明，未做测量。`U`
6. **未验证其他 adapter**（例如非 pi-ai、非 DeepSeek Messages 的路由）如何映射前导/后置 system 消息。`U`

## 9. 命令与实际结果

| 命令 | exit code | 结果摘要 |
| --- | --- | --- |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo rev-parse HEAD` | 0 | `b830ec950faba0a46dda564be3e2d854afed94f1` |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo describe --tags --exact-match HEAD` | 0 | `dsh-v0.2.1-rc.5` |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo status --short` | 0 | 无输出 |
| `rg -n 'assemble' packages/core/agent-loop/src/agent.ts` | 0 | 定位 assembly 使用点 |
| `rg -n 'splitSystemPrompt' packages/llm/llm-pi-ai/src/context.ts` | 0 | 定位 system prompt split 与 tool 过滤入口 |
| `rg -n 'system/message' packages/core/session/src/surface.ts packages/core/session/src/index.ts` | 0 | 定位 system 事件的 surface 与持久化约束 |
| `rg -n 'systemPrompt.tools' packages/core/tools/src/index.ts` | 0 | 定位 tool provider 注入点 |
| `nl -ba packages/core/system-prompt/src/index.ts` | 0 | 读取完整 registry 与 assembly 源码 |
| `nl -ba packages/core/agent-loop/src/runtime-context.ts` | 0 | 读取 `SystemPromptProjection` 与 runtime-context snapshot |
| `sed -n '540,705p' packages/core/agent-loop/src/agent.ts` | 0 | 读取请求构造与 route capability 使用点 |
| `rg -n -e 'source_commit' -e 'package_versions' -e 'DSH-R2-01' -e '未验证项' docs/research/dsh-runtime-audit/evidence/phase2/01-system-prompt-assembly.md` | 0 | 校验报告包含基线、step id 和未知点章节 |

## 10. 边界

- 未安装依赖、未运行 package manager、未运行 build 或测试。
- 未修改 `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo` 源码树，未修改仓库 `runtime/`。
- 未读取 `.env`、凭证、真实会话正文或业务材料。
- 未启动服务、未调用模型、未观察 Provider 网络请求。
- 本文件只讨论 System Prompt 的组装与请求序列化；Tool Schema 的注册、过滤、presentation 分离与 PTC 细节留给 `04-tool-schema.md`。

