# 阶段二源码审计复核

- 复核时间：2026-11-12
- 源码 commit：`b830ec950faba0a46dda564be3e2d854afed94f1`
- 源码 tag：`dsh-v0.2.1-rc.5`
- 源码路径：`/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo`
- 复核对象：`evidence/phase2/01-system-prompt-assembly.md` 至 `06-subagent-lifecycle.md`
- 结论：**六份静态源码审计报告通过阶段二证据验收；运行行为仍未验收，阶段三未开始**

## 1. 检查结果

| 任务 | 报告 | 结果 | 关键限制 |
| --- | --- | --- | --- |
| DSH-R2-01 System Prompt | `01-system-prompt-assembly.md` | 通过（静态） | 没有观察 Provider wire body；没有读取实际 Profile/persona |
| DSH-R2-02 Agent Loop | `02-agent-loop.md` | 通过（静态） | 没有运行 turn/step/取消/恢复测试 |
| DSH-R2-03 Session Replay / Projection | `03-session-replay-projection.md` | 通过（静态） | 没有读取真实会话；没有运行持久化/压缩测试 |
| DSH-R2-04 Tool Description / Schema | `04-tool-schema.md` | 通过（静态） | 没有运行工具，也没有观察工具 schema 的真实请求 |
| DSH-R2-05 Parallel Scheduler | `05-parallel-scheduler.md` | 通过（静态） | 没有并发实测；没有验证 abort 时延和资源竞争 |
| DSH-R2-06 Subagent Lifecycle | `06-subagent-lifecycle.md` | 通过（静态） | 没有启动子 Agent；没有验证真实 `agents.create/resume` 竞态 |

## 2. 机械校验

- 六份报告都包含 `source_commit`、`source_path`、`package_versions`、证据等级说明、反证和未验证项。
- 报告共 1,820 行，所有 JSON/源码引用均可解析为 Markdown 文本。
- 脚本扫描报告中 266 个显式 `packages/...:line` 引用，文件全部存在，行号全部在文件范围内（0 个失效引用）。
- 所有实例均使用 rc.5 源码树；源码树 `git status --short` 为空。
- 没有安装依赖、运行 build/test、启动服务、调用模型、读取凭证或会话正文。
- 报告没有出现“迁移决定”“重构决定”或“继续使用 DSH”的最终结论。

## 3. 机制结论

### 3.1 System Prompt Assembly

- `systemPrompt.assemble()` 在每个 step 的 `preStep` 中执行一次；同一 step 内请求重试只 `refreshContext()`，不重新执行 sections/tools/variables 的组装。
- section 先按 `order`、再按名称 code-unit 排序；同层同名重复报错，scoped 同名覆盖 global。
- section 文本中的变量是严格插值；空 section 被丢弃并以双换行连接。
- tools 是独立的 `ToolSchema[]`，不自动拼进 system 正文。
- 请求把 system prompt 作为历史中的 `system/message`，provider 适配器再把前导 system 历史合并到 provider 的 system 槽。
- 证据等级均为 `S`；实际 provider body 和实际 Profile 组合仍为 `U`。

### 3.2 Agent Loop

- 运行时 phase 只有 `idle`、`maintenance`、`running`；turn/step 是 running 内的字段和持久事件边界。
- `turn/start` 在 claim 输入前写入；`preStep` 先 claim inbox，再 assembly 和 `agent/pre-step` waterfall。
- 一个 step 包含一次模型请求及其请求的工具执行；同一步可以在请求失败/取消路径上重试，但不新增 `step/start`。
- max-tokens 在 turn 内粘性；`concludeTurn` 可以让工具 step 直接完成。
- 取消会停止输入、abort 当前 signal、结算已开始的内容；失败先补未回答工具结果，再关闭 step/turn。
- resume 的语义修复由持久日志 closers 补写，不能等同于运行中内存状态恢复。

### 3.3 Session Replay / Projection

- 事件日志的连续 `seq` 是权威来源；追加事件在提交前做无损 JSON 快照。
- surface 通过 append/replace/project 形成模型消息视图；`deriveMessages()` 只投影消息事件，历史日志保留被遮蔽事件。
- 请求上下文由最新 `request/header` 的 config/tools、`deriveMessages()` 和 `toolHistory()` 重建。
- compaction 追加 summary 和 replace 节点，不删除原始事件；摘要质量是模型行为，静态源码无法证明。
- JSONL 只保证物理连续前缀；撕裂尾部被截断，语义 closers 由 agent-loop 修复路径补充。
- resume 与普通会话 fork 是不同边界语义；子 Agent fork 另有一套 seed 规则。

### 3.4 Tool Description / Tool Schema

- `defineTool()` 编译参数和输出声明为 JSON Schema，模型可见投影只保留 `name`、`description`、`parameters` 和可选 `deferLoading`。
- registry 先形成 scope-chain 可见集，再应用相交的 allow/deny 限制；scope 自有注册可覆盖继承工具。
- `native` 模式暴露全部可见 schema；`ptc` 模式只暴露 `run_code` 并从同一可见集生成 SDK 说明。
- `description`、prompt guidance 与 UI presentation 是不同文本通道，不能把 UI 文案当成模型描述。
- DeepSeek 和 pi-ai 的字段映射均在适配层完成；实际 wire body 仍为 `U`。

### 3.5 Parallel Tool Scheduler

- 默认并行上限为 10，限制同时处于 dispatch 的调用数，不是单个 step 的调用总数。
- `prepare`/有序前置策略串行；dispatch/工具主体可以重叠；`tool/call` 与 `tool/result` 按模型顺序提交。
- 首个调用的分类决定组类型；parallel 组内的后续调用在开始前重新分类，遇到 exclusive 就形成 barrier。
- abort 会停止补充、等待已启动调用结算，再为未启动调用写 synthetic call/result，保证 replay 有配对记录。
- 调度器失败会 drain 已启动调用，并由上层 ToolCallRecovery 补保守结果。
- 这些是源码静态行为，未做并发实测。

### 3.6 Subagent Lifecycle

- `subagent` 工具立即返回 activation id，结果通过异步 settlement 消息回父 Agent，不是阻塞式工具返回。
- `spawn` 子 Agent 从零上下文开始；`fork` 子 Agent 只以父会话最后一个 `turn/end` 为止的已完成轮为 seed，不含进行中的 turn。
- 子 Agent 默认继承父的最新模型路由；工具来自 preset composition；spawn 可以独立选 preset，fork 不可以。
- 权限只继承受限子集：sandbox 显式覆盖和 auto/full preset 可继承，approval 固定为 `never`，子 Agent 不能自行扩大权限。
- 默认 `maxDepth = 1`，深度是持久、单调的预算；达到上限时工具仍可见但运行时会拒绝。
- 结果捕获、dispose、父 inbox 通知和 teardown 失败分别处理；真实生命周期竞态仍为 `U`。

## 4. 验收判定

- **阶段二源码证据：通过。** 六份报告满足单机制、源码定位、证据等级、反证和未验证项要求。
- **阶段二运行证据：未完成。** 因为未安装依赖、未运行测试、未观察 Provider，所有运行结论仍需阶段三或后续受控测试补充。
- **不能据现有材料下迁移/改造结论。** 现有证据只说明机制实现与边界，尚未证明这些机制在相同模型和任务条件下造成可重复性能差异。

## 5. 后续门禁

- 阶段三必须先写实验协议并通过用户对 Pi 安装、Provider 和真实模型调用的单独授权。
- 当前不安装 Pi、不启动真实模型、不修改源码、不进入迁移或重构。
