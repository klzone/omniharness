# DSH-R2-04：Tool Description、Schema 的注册、过滤与序列化

- step_id: `DSH-R2-04`
- generated_at: `2026-11-12T10:20:18Z`
- source_commit: `b830ec950faba0a46dda564be3e2d854afed94f1`
- source_path: `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo`
- source_tag: `dsh-v0.2.1-rc.5`
- package_versions: `@deepseek-ai/dsh-tools@0.2.1-rc.5`、`@deepseek-ai/dsh-system-prompt@0.2.1-rc.5`、`@deepseek-ai/dsh-agent-loop@0.2.1-rc.5`、`@deepseek-ai/dsh-llm@0.2.1-rc.5`、`@deepseek-ai/dsh-llm-pi-ai@0.2.1-rc.5`、`@deepseek-ai/dsh-llm-deepseek@0.2.1-rc.5`
- 证据口径：只做静态源码与测试代码阅读；没有安装依赖，没有运行测试、工具、服务或模型，也没有观察 Provider 的实际 wire body。

## 1. 结论摘要

1. **一个工具的可调用定义来自 `defineTool()`：它把 author-facing 参数 DSL 编译成 raw JSON Schema，把输出 DSL 编译成另一份 raw JSON Schema，并保留 `name`、`description`、`execute` 和可选执行/展示元数据。** `S`（`packages/core/tools/src/schema.ts:547-631`）。
2. **`ToolDefinition` 继承 `ToolSchema`，但只有 `name`、`description`、`parameters`、`deferLoading` 会进入 `ToolSchema` 投影。** `output`、`execute`、`timeoutMs`、`isConcurrencySafe`、`presentCall`、`presentResult`、`finalizeContent`、`projectContent` 都在 runtime 定义上，不能通过这些字段自动进入模型请求。`S`（`packages/core/tools/src/index.ts:223-300`、`:1289-1302`）。
3. **注册按 scope 分层：同一 layer 内同名注册抛错；scoped 注册遮蔽继承来的同名工具；`run_code` 是保留名。** `S`（`packages/core/tools/src/index.ts:735-750`、`:1065-1095`、`:1186-1216`）。
4. **过滤只作用于“继承的工具面”：global 和祖先 scope 的工具可被 `allow`/`deny` 交集过滤；当前 scope 自己的注册不受该 scope 的过滤器影响；保留的 PTC transport 也不受过滤。** `S`（`packages/core/tools/src/index.ts:1098-1131`、`:1164-1226`）。
5. **模型可见集合由 `wireSchemas(scope)` 生成：`native` 返回所有 visible tool 的 schema 和 pre-restriction `knownNames`；`ptc` 只返回 `run_code`，`knownNames` 也只有 `run_code`。** `S`（`packages/core/tools/src/index.ts:1015-1037`）。
6. **System prompt assembly 是工具 schema 进入请求的注册通道，但 schema 不拼进 prompt 正文。** `ToolRuntime` 向 `ctx.systemPrompt.tools()` 注册 provider；`assemble()` 收集、克隆、排序 tool schema 到独立的 `PromptAssembly.tools`，`renderPrompt()` 只渲染 sections。`S`（`packages/core/tools/src/index.ts:850-861`；`packages/core/system-prompt/src/index.ts:634-675`）。
7. **工具描述有三条不同的文本通道：schema 里的 `description`、工具插件单独注册的 prompt guidance section、以及仅用于 UI/replay 的 `presentCall`/`presentResult`/`presentationMeta`。** 三者不能混为一谈；前两者可能进入模型，presentation 不进入 `ToolSchema`。`S`（`packages/core/tools/src/index.ts:282-300`、`:1289-1302`；代表性工具 `packages/fs/tool-fs/src/read.ts:68-84`）。
8. **`deferLoading: true` 会从 registry 保留到 `ToolSchema` 和请求 header；是否真正延迟由 route 的 `toolUpdate` 与 LLM seam 决定。** 不支持 tool update 的路由会移除该标记并删除 developer update 消息；支持的路由会保留声明，并在可激活时通过 tool-addition 消息暴露。`S`（`packages/core/tools/src/index.ts:1290-1301`；`packages/llm/llm/src/content.ts:424-491`）。
9. **Provider 序列化只保留名称、描述、参数 schema 和必要的 deferred 标记。** DeepSeek Messages 把 `parameters` 映射为 `input_schema`，`deferLoading` 映射为 `defer_loading: true`；pi-ai 把 `parameters` 映射为 `Tool.parameters`，并在初始 tools 列表中过滤掉 deferred 工具。`S`（`packages/llm/llm-deepseek/src/serialize.ts:160-167`；`packages/llm/llm-pi-ai/src/context.ts:130-167`）。
10. **PTC 模式是另一条“工具面”路径：模型只直接看到 `run_code`，其余工具名由生成的 `tools:sdk` 文本声明；实际调用通过 run_code 内的嵌套 dispatch 进入同一个 visible set。** `S`（`packages/core/tools/src/index.ts:671-695`、`:899-916`、`:1019-1036`；`packages/core/tools/src/ptc.ts:32-69`、`:330-351`、`:680-694`）。

## 2. 四段链路

```text
1. 工具定义
   defineTool({ name, description, parameters, output, execute, ... })
     -> parameterSchemaSpecToJsonSchema(parameters)
     -> valueSchemaSpecToJsonSchema(output.schema)
     -> ToolDefinition (runtime; extends ToolSchema)

2. 注册表
   ToolRuntime.register(definition)
     -> layer.tools.insert(name, definition)
     -> same-layer duplicate / reserved run_code rejected
     -> view(scope): inherited + restrictions + own layer + PTC transport

3. Agent 可见集
   systemPrompt.tools(context => wireSchemas(context.scope))
     -> native: all visible schemas + knownNames
     -> ptc: run_code only + knownNames [run_code]
     -> assemble(): detach name/description/parameters/deferLoading
     -> orderTools(): lexicographic or configured toolOrder
     -> PromptAssembly.tools

4. Provider 请求
   agent-loop buildRequest(): header.tools = assembly.tools
     -> llm seam projectToolUpdates(): route toolUpdate + ToolHistory
     -> pi-ai: Tool { name, description, parameters }
     -> DeepSeek Messages: { name, description, input_schema, defer_loading? }
```

## 3. 代表性工具对照

| 工具 | 源码位置 | 模型可见名称/描述 | 参数 schema | 独立的 prompt guidance | UI presentation |
| --- | --- | --- | --- | --- | --- |
| `read` | `packages/fs/tool-fs/src/read.ts:77-84` | 名称 `read`；描述为固定字面量 “Read a UTF-8 text file...” | `file_path` string required；`offset`、`limit` number optional | `tool:read` section，只有该 scope 可见 `read` 时才输出文本（`:68-75`） | `presentResult` / `presentCall`（`:173-205`） |
| `bash` | `packages/shell/tool-bash/src/index.ts:368-402` | 名称 `bash`；描述来自 `bashDescription()`（`:90-99`） | `description`、`command` required；`timeoutMs`、`workdir` 固定出现；`run_in_background` 依 jobs 能力，`sandbox_permissions`/`justification` 依 sandbox 配置 | `tool:bash` section，固定提示检查 exit code（`:250-255`） | `presentBashCall` / `presentBashResult`（`:532-533`） |
| `glob` | `packages/fs/tool-fs-search/src/glob.ts:306-319` | 名称 `glob`；描述包含 `caps.maxResults` 和 over-cap 行为 | `pattern` string required；`path` string optional | `tool:glob` section，scope 可见 `glob` 时才输出（`:296-303`） | `presentGlobCall` / `presentGlobResult`（`:350-351`） |
| `web_search` | `packages/web/tool-web/src/search.ts:325-335` | 名称 `web_search`；描述为固定字面量 | `queries` array required；每项 string | `tool:web_search` section，内容随 `web_fetch` 是否可见变化（`:315-323`） | `presentSearchCall` / `presentSearchResult`（`:374-375`） |

说明：`bash` 的 `description`、`timeoutMs`、`workdir` 等参数会因插件配置变化；`glob` 的描述文本包含部署上限；`read`/`glob`/`web_search` 的 guidance section 通过 `ctx.tools.get(name, scope)` 判断是否输出。这些都属于“定义来自运行时配置”的 `S` 级事实，不代表任何具体 OmniDesk Profile 的实际值。

## 4. Schema 的生成与投影

### 4.1 `defineTool()` 编译参数与输出

- `DefineToolOptions.parameters` 是 `ParameterSchemaSpec`，`description` 明确要求 “Human-readable description sent to the model”。`S`（`packages/core/tools/src/schema.ts:482-500`）。
- `defineTool()` 调用 `parameterSchemaSpecToJsonSchema(options.parameters)` 和 `valueSchemaSpecToJsonSchema(options.output.schema)`，保存到 `ToolDefinition.parameters` 与 `ToolDefinition.output.schema`。`S`（`packages/core/tools/src/schema.ts:554-602`）。
- 参数 schema 以 `type: 'object'` 为根，`properties` 来自编译结果；只有标了 `required: true` 的属性进入 `required` 数组。`S`（`packages/core/tools/src/schema.ts:416-426`、`:449-458`）。
- 编译过程把重复/循环对象视为 author error，并用 `Object.defineProperty` 安装属性，避免 `__proto__` 赋值语义。`S`（`packages/core/tools/src/schema.ts:242-263`、`:274-327`）。
- JSON Schema 子集包括 scalar、`object`、`array`、`items`、`enum`、`const`、`oneOf`、`required` 和 annotation。`S`（`packages/core/tools/src/json-schema.ts:31-56`）。
- 注册时只对 `output.schema` 调用 `assertSupportedJsonSchema`；输入 `parameters` 的严格编译保证来自 `defineTool()`，不是 `ToolRuntime.register()` 的通用校验。`S`（`packages/core/tools/src/index.ts:1071-1095`）。

### 4.2 Runtime 定义与模型可见字段的分离

- `ToolDefinition extends ToolSchema`，额外字段有 `output`、`execute`、`projectContent`、`finalizeContent`、`timeoutMs`、`isConcurrencySafe`、`presentCall`、`presentResult`。`S`（`packages/core/tools/src/index.ts:223-300`）。
- JSDoc 明确 `timeoutMs` 绝不发送给模型；`isConcurrencySafe` 是“never model-visible”的调度元数据。`S`（`packages/core/tools/src/index.ts:260-281`）。
- `schemaOf()` 用显式解构白名单，只返回 `name`、`description`、`parameters`、可选 `deferLoading`。`S`（`packages/core/tools/src/index.ts:1289-1302`）。
- `systemPrompt.assemble()` 再次构造 `ToolSchema`，只复制上述四个字段，并对参数执行 `structuredClone`。`S`（`packages/core/system-prompt/src/index.ts:638-651`）。
- 测试代码断言 `schemas()` 不含 `execute`、`presentCall`、`presentResult`，也不含 `timeoutMs`；本轮未执行。`S`（`packages/core/tools/tests/tools.spec.ts:66-100`、`:144-153`，未执行）。

### 4.3 schema、description、presentation 三种文本

| 文本通道 | 来源 | 是否进入 native `ToolSchema` | 证据 |
| --- | --- | --- | --- |
| `description` | `defineTool({ description })` 或直接注册的 `ToolDefinition.description` | 是，原样进入 `ToolSchema.description` | `packages/core/tools/src/schema.ts:482-488`；`packages/core/tools/src/index.ts:1291-1300` |
| `parameters` JSON Schema | author DSL 编译得到的 raw schema | 是，进入 `ToolSchema.parameters`；Provider 可能改名为 `input_schema` | `packages/core/tools/src/schema.ts:449-458`；`packages/llm/llm-deepseek/src/serialize.ts:160-167` |
| prompt guidance | 工具插件调用 `ctx.systemPrompt.section()` 注册独立 section | 否；它只进入 prompt 的 section 文本 | `packages/fs/tool-fs/src/read.ts:68-75`；`packages/core/system-prompt/src/index.ts:290-295` |
| UI presentation | `presentCall()` / `presentResult()` / `output.presentationMeta()` | 否；`schemaOf()` 不读取这些字段 | `packages/core/tools/src/index.ts:282-300`、`:1289-1302` |
| 输出 schema | `output.schema` | native 工具 schema 不含 `output`；PTC SDK 生成时会读取它 | `packages/core/tools/src/index.ts:223-226`；`packages/core/tools/src/ts-types.ts:312-333` |

- `read` 的 `presentResult` 从持久化 `meta` 还原 read 卡片，属于 UI/replay 通道；它不影响模型收到的工具 schema。`S`（`packages/fs/tool-fs/src/read.ts:166-205`）。
- `bash` 的 `presentBashCall` 使用 `command` 作为 terminal 标题，把 `description` 作为卡片描述；这是 UI 展示，不是工具 `description` 的替代。`S`（`packages/shell/tool-bash/src/index.ts:106-143`、`:532-533`）。
- `presentationMeta` 的结果会持久化在 `tool/result` 或 `tool/ptc-dispatch`，供 Host/Client 还原；不是模型内容。`S`（`packages/core/tools/src/index.ts:302-315`）。

## 5. 注册、作用域、命名冲突与过滤

### 5.1 注册与命名

- `register()` 要求 `output` 有 `render`，可选 `presentationMeta` 必须是函数；并校验 `timeoutMs` 为正有限数。`S`（`packages/core/tools/src/index.ts:1071-1084`）。
- `run_code` 无条件保留，任何 layer 注册或遮蔽该名称都失败。`S`（`packages/core/tools/src/index.ts:1085-1090`）。
- 同一 layer 内重名由 `NamedEntries` 的 duplicate 检查抛错；global 与 scoped 层的错误文案不同。`S`（`packages/core/tools/src/index.ts:735-750`）。
- `view(scope)` 先构造 inherited map：从 global 开始，随后按祖先 scope 覆盖同名项；然后应用整条 scope chain 上的 restriction 交集；最后写入当前 scope 自己的工具，允许 scoped 遮蔽 inherited。`S`（`packages/core/tools/src/index.ts:1186-1216`）。
- `knownNames` 保留 pre-restriction 名称，因此 `toolOrder` 可以合法列出一个在当前 scope 被 restriction 隐藏的工具；`restrictableNames` 只包含 global/ancestor 的工具。`S`（`packages/core/tools/src/index.ts:1200-1216`；`packages/core/system-prompt/tests/scoped.spec.ts:190-204`，未执行）。

### 5.2 作用域过滤

- `restrict()` 必须通过 scoped context 调用；`allow`/`deny` 至少提供一个，空过滤器会报错。`S`（`packages/core/tools/src/index.ts:1098-1114`）。
- 过滤器在注册时编译为 `Set`，多个 restriction 在 `admits()` 中逐一检查，所以是交集语义。`S`（`packages/core/tools/src/index.ts:708-712`、`:758-765`、`:1115-1131`）。
- 过滤器命名的工具必须在 `restrictableNames` 中，否则抛错；`run_code` 不能被命名。`S`（`packages/core/tools/src/index.ts:1119-1126`）。
- 当前 scope 自己注册的工具在过滤之后写入 visible，因此“child 的 structured-output 工具”不会被 parent/child 的 inherited filter 剥离。`S`（`packages/core/tools/src/index.ts:1171-1182`、`:1210-1216`）。

### 5.3 `toolOrder` 与重复名称

- 未配置 `toolOrder` 时，system prompt 对 provider 收集到的工具按名称 code-unit 排序。`S`（`packages/core/system-prompt/src/index.ts:226-240`）。
- 配置 `toolOrder` 时，必须含且只含一个 `<unlisted-tools>`；列出的名称若不在 `knownNames` 中，`assemble()` 抛错。`S`（`packages/core/system-prompt/src/index.ts:208-240`、`:274-278`）。
- `orderTools()` 只做排序，不按名称去重。测试代码明确构造两个同名 `dup`，并断言稳定排序保留两者；本轮未执行。`S`（`packages/core/system-prompt/tests/tool-order.spec.ts:74-78`，未执行；源码 `packages/core/system-prompt/src/index.ts:226-240`）。
- 正常 `ToolRuntime` 路径的 `view().visible` 是 `Map<string, ToolDefinition>`，因此同 scope 的正常注册不会产生重名；重复名称风险来自自定义 `systemPrompt.tools()` provider 或绕过 `defineTool` 的 raw definition。`S`（`packages/core/tools/src/index.ts:714-722`、`:1186-1216`）。
- `register()` 没有对工具名做通用正则/长度校验；除 `run_code` 保留名外，名称限制落在 Provider 或上层部署。`U`（本卡未找到统一名称校验；不能推断实际 Provider 会接受任意名称）。

## 6. 延迟/动态工具与 Provider 序列化

### 6.1 `deferLoading`

- `ToolSchema.deferLoading?: true` 的注释说明它请求延迟加载，并要求用 `tool-addition` 记录；它是工具声明的一部分，不是 prompt section。`S`（`packages/llm/llm/src/types.ts:491-502`）。
- `defineTool()` 仅在 `options.deferLoading === true` 时写入；`schemaOf()` 同样只在 true 时保留。`S`（`packages/core/tools/src/schema.ts:595`；`packages/core/tools/src/index.ts:1300`）。
- `projectToolUpdates()` 在 route 不支持 tool update 时，移除 `deferLoading` 并删除所有 developer update 消息；在支持且 `ToolHistory` 存在时，根据历史构造 declarations，并为后续新增项加上 `deferLoading: true`；`ToolHistory` 缺失时退回当前 tools。`S`（`packages/llm/llm/src/content.ts:381-446`）。
- 已 available 的 baseline 工具不会因重复的 `tool-addition` 再次变成“新工具”；只有尚未 offered 且已声明的工具会被发出。`S`（`packages/llm/llm/src/content.ts:449-491`）。
- pi-ai 的 `toolsOf()` 过滤掉 `deferLoading` 的 schema；`appendDeveloper()` 则从 `options.tools` 找到被 addition 引用的工具，写入 pi-ai system message 的 `toolsAdded`。`S`（`packages/llm/llm-pi-ai/src/context.ts:130-136`、`:170-200`）。
- DeepSeek 序列化把 `deferLoading` 映射成 `defer_loading: true`，并在测试代码中固定该字段和 `input_schema` 映射；本轮未执行测试。`S`（`packages/llm/llm-deepseek/src/serialize.ts:160-167`；`packages/llm/llm-deepseek/tests/serialize.spec.ts:75-80`，未执行）。

### 6.2 Provider 字段映射

| 层 | 字段 | 源码 |
| --- | --- | --- |
| Harness `ToolSchema` | `name`、`description`、`parameters`、`deferLoading` | `packages/llm/llm/src/types.ts:491-502` |
| LLM seam | `projectToolUpdates()` 接收 messages、tools、route `toolUpdate`、`toolHistory` | `packages/llm/llm/src/index.ts:1106-1118`；`packages/llm/llm/src/content.ts:424-491` |
| pi-ai | `{ name, description, parameters }`；deferred 工具不在初始 `tools` | `packages/llm/llm-pi-ai/src/context.ts:130-167` |
| DeepSeek Messages | `{ name, description, input_schema: parameters, defer_loading? }` | `packages/llm/llm-deepseek/src/serialize.ts:160-167` |

- `systemPrompt.assemble()` 在把 schema 交给 Agent 前还会按 `toolOrder` 排序；Agent 的 `buildRequest()` 把 `assembly.tools` 放进 `header.tools`，然后 LLM seam 才做 route-specific tool projection。`S`（`packages/core/agent-loop/src/agent.ts:616-641`、`:686-703`；`packages/llm/llm/src/index.ts:1106-1118`）。
- `request/header` 的 `header.tools` 是完整声明；空数组在 canonical header 中会变成“没有 tools 字段”。`S`（`packages/core/session/src/request-header.ts:14-30`）。

## 7. PTC 工具面

- `ToolPresentationMode` 为 `native | ptc`，默认 `native`；`ToolRuntime` 构造时把 `mode` 与 `maxParallelSubCalls` 写入配置。`S`（`packages/core/tools/src/index.ts:671-695`、`:808-814`）。
- `wireSchemas()` 在 native 返回所有 visible schema；在 PTC 先要求存在 `ctx.ptcRuntime` 且有 SDK renderer，再只返回 `run_code`，并把 `knownNames` 收窄为 `[run_code]`。`S`（`packages/core/tools/src/index.ts:1015-1037`、`:1053-1062`）。
- `run_code` 描述和 `code` 参数描述由 runtime language 的 flavor 决定；静态 getter 在 schema 投影时重新解析，避免模型看到与 SDK 语言不一致的 schema。`S`（`packages/core/tools/src/ptc.ts:32-69`、`:330-351`、`:766-789`）。
- `sdkSection()` 用 calling scope 的 visible tools 生成 `tools:sdk`，并排除 `run_code` 自身；`renderToolsSdk()` 按名称排序，同时生成参数类型和输出类型。`S`（`packages/core/tools/src/index.ts:899-916`；`packages/core/tools/src/ts-types.ts:300-333`）。
- PTC 程序内的 binding 使用 `registry.schemas(exec.agent)`，并排除 `run_code`；所以 SDK 声明的工具和实际可绑定的工具来自同一个 scope view。`S`（`packages/core/tools/src/ptc.ts:680-694`）。
- `presentAs(mode)` 必须通过 scoped context 调用；最近 scope 的 mode 覆盖祖先/默认值，PTC mode 同时注册 `tools:ptc-only` 和 `tools:sdk` section。`S`（`packages/core/tools/src/index.ts:925-936`、`:974-1013`）。
- PTC 的 `run_code` 是保留 transport，不能注册或遮蔽，也不受 scope restriction 过滤；模型直接调用其他工具时会在 execution 解析阶段被 collapse 成 `UNKNOWN_TOOL`。`S`（`packages/core/tools/src/index.ts:1085-1090`、`:938-945`、`:1242-1259`）。

## 8. README 线索（D，不作为行为证明）

- `packages/core/tools/README.md:12` 声称模型看到每个允许工具的声明名称、描述和参数 schema，且按 agent restriction 收窄；这是设计说明。`D`
- `packages/core/tools/README.md:109` 声称 registry 把 typed definition 投影到 model-facing `ToolSchema`，`output`、`execute`、`finalizeContent`、`timeoutMs` 和 presentation callback 不进入 wire；源码中的 `schemaOf()` 与该描述一致，但仍以源码为准。`D`
- `packages/core/tools/README.md:168` 描述 native 模式下的 exact name/description/JSON Schema；源码确实把四元组投影到 `ToolSchema`，但实际 Provider 还需自行映射字段名。`D`
- `packages/core/tools/README.md:182` 描述 PTC 的 `run_code` 与 SDK section；源码中 `wireSchemas()`、`sdkSection()` 可对应，但具体 SDK 文本未被本卡执行或渲染。`D`
- `packages/core/system-prompt/README.md:160` 描述 tool sections 与 schema providers 是分离输入；源码中 `PromptAssembly.sections` 与 `.tools` 分离，支持该描述。`D`

## 9. 反证与限制

1. **README 说模型看到 “exact” 的 name/description/JSON Schema；源码在 native 路径还带有 `deferLoading` 标记，且 pi-ai 的初始 tools 列表会过滤 deferred 工具。** “exact” 不能解释为“所有字段都原样进入所有 Provider 的所有请求”。`S`（`packages/core/tools/src/index.ts:1290-1301`；`packages/llm/llm-pi-ai/src/context.ts:135-136`）。
2. **`defineTool()` 会编译并校验参数 schema，但 `ToolRuntime.register()` 本身没有对 `parameters` 调用 `assertSupportedJsonSchema`。** 直接构造 raw `ToolDefinition` 的路径只由 `schemaOf()` 的 lossless-JSON 检查兜底；正常第一方工具走 `defineTool()`。`S`（`packages/core/tools/src/index.ts:1071-1095`、`:1289-1302`）。
3. **`systemPrompt.assemble()` 的 `orderTools()` 不按名称去重。** 自定义 tool provider 返回同名 schema 时，assembly 会保留多个；测试代码明确了稳定排序保留同名项。正常 `ToolRuntime` 的 `Map` 视图不会产生这个情形。`S`（`packages/core/system-prompt/src/index.ts:226-240`；`packages/core/system-prompt/tests/tool-order.spec.ts:74-78`，未执行）。
4. **工具 `description` 不是稳定静态文本。** `bash` 的描述来自函数，`glob` 的描述包含部署上限；配置变化会改变 schema 文本。`S`（`packages/shell/tool-bash/src/index.ts:90-99`、`:368-402`；`packages/fs/tool-fs-search/src/glob.ts:305-319`）。
5. **`description` 的运行时“非空”没有被 `defineTool()` 强制。** 类型要求是 `string`，测试中也使用空字符串构造 deferred 工具；Provider 是否接受空描述未在本卡验证。`S`（`packages/core/tools/src/schema.ts:482-488`；`packages/core/tools/tests/tools.spec.ts:55-60`，未执行）。
6. **PTC 的 `tools:sdk` 会把输出 schema 变成模型可见文本，native 的 `ToolSchema` 则没有 `output` 字段。** 因而“输出 schema 永不进入模型”只对 native 工具声明成立，不能外推到 PTC SDK。`S`（`packages/core/tools/src/ts-types.ts:312-333`；`packages/core/tools/src/index.ts:1289-1302`）。
7. **`toolFilter` 是 subagent/上层组合的概念，不是本轮 `ToolRuntime` 的模型字段。** 本卡只追踪 `tools.restrict`；subagent 如何把 `toolFilter` 映射到 scoped restrict 属于 `DSH-R2-06`，不能混用结论。`U`。

## 10. 未验证项

1. **未读取 OmniDesk 的实际工具组合、`toolOrder`、`mode`、`ptcRuntime` 和 scope/preset 配置**，因此不声称当前产品实际向模型发送了哪些工具或哪个 presentation mode。`U`
2. **未运行工具、未调用 Provider、未捕获 wire body**；`name`、`description`、`parameters`、`input_schema` 的映射来自源码和静态测试代码，不是运行证据。`U`
3. **未验证 pi-ai 实际使用的 provider API 与 schema 兼容性**；`Tool.parameters` 的最终 wire 形状由第三方 pi-ai 决定，本卡只确认 harness 传给它的结构。`U`
4. **未验证 DeepSeek 或其他 Provider 对空描述、超长描述、特殊工具名、复杂 `oneOf` schema 的接受程度**。`U`
5. **未验证 `toolUpdate` 在真实模型路由上的取值**，因此无法判断实际请求是完整声明、deferred 声明，还是 developer tool update。`U`
6. **未执行 `system-prompt`/`tools` 的测试**；报告中引用的测试断言只是静态测试代码，不能作为 `R` 级通过证据。`U`
7. **未验证 PTC SDK 的实际渲染文本、语言 flavor 和 runtime 能力**；只确认生成路径读取 visible tools 和 `output.schema`。`U`
8. **未验证第三方 MCP 或动态注册工具是否都经过 `defineTool()`**；如果绕过 `defineTool()`，参数 schema 的严格性取决于该 producer。`U`

## 11. 命令与实际结果

| 命令 | exit code | 结果摘要 |
| --- | --- | --- |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo rev-parse HEAD` | 0 | `b830ec950faba0a46dda564be3e2d854afed94f1` |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo describe --tags --exact-match HEAD` | 0 | `dsh-v0.2.1-rc.5` |
| `git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo status --short` | 0 | 无输出 |
| `rg -n 'deferLoading' packages/core/tools/src/index.ts` | 0 | 定位 schema 投影中的 deferred 标记 |
| `rg -n 'parameterSchemaSpecToJsonSchema' packages/core/tools/src/schema.ts` | 0 | 定位参数 schema 编译 |
| `rg -n 'wireSchemas' packages/core/tools/src/index.ts` | 0 | 定位 native/PTC 可见集投影 |
| `rg -n 'projectToolUpdates' packages/llm/llm/src/content.ts` | 0 | 定位 route tool update 投影 |
| `rg -n 'defineTool' packages/fs/tool-fs/src/read.ts` | 0 | 定位代表性 `read` 工具定义 |
| `nl -ba packages/core/tools/src/schema.ts` | 0 | 读取 DSL、编译、`defineTool()` 全文 |
| `nl -ba packages/core/tools/src/index.ts` | 0 | 读取 registry、view、schemas、PTC 入口 |
| `nl -ba packages/core/tools/src/ptc.ts` | 0 | 读取 `run_code` transport 与 SDK schema getter |
| `rg -n -e 'source_commit' -e 'package_versions' -e 'DSH-R2-04' -e '未验证项' docs/research/dsh-runtime-audit/evidence/phase2/04-tool-schema.md` | 0 | 校验报告包含基线、step id 和未知点章节 |

## 12. 边界

- 未安装依赖、未运行 package manager、未运行 build 或测试。
- 未修改 `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo` 源码树，未修改仓库 `runtime/`。
- 未读取 `.env`、凭证、真实会话正文或业务材料。
- 未启动服务、未调用模型、未运行任何工具。
- 本文件只讨论 Tool Description 与 Schema 的注册、过滤和序列化；System Prompt 的组装细节见 `01-system-prompt-assembly.md`，Subagent 生命周期与 `toolFilter` 的上下文继承见 `06-subagent-lifecycle.md`。

