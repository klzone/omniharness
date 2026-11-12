# 阶段二六张源码审计任务卡

- 状态：可执行；阶段二已获用户选择 1 授权
- 源码路径：`/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo`
- 源码 commit：`b830ec950faba0a46dda564be3e2d854afed94f1`
- 输出目录：`docs/research/dsh-runtime-audit/evidence/phase2/`
- 全局禁止：安装依赖、修改源码树、修改仓库 `runtime/`、运行真实模型、读取凭证/会话正文、作出迁移或改造决定

## 卡片总表

| Step ID | 单一动作 | 输出 |
| --- | --- | --- |
| DSH-R2-01 | 追踪 System Prompt 的组装与请求序列化 | `01-system-prompt-assembly.md` |
| DSH-R2-02 | 追踪 Agent Loop 的轮次状态机 | `02-agent-loop.md` |
| DSH-R2-03 | 追踪 Session Replay 与 Context Projection | `03-session-replay-projection.md` |
| DSH-R2-04 | 追踪 Tool Description 与 Schema 的注册、过滤和序列化 | `04-tool-schema.md` |
| DSH-R2-05 | 追踪并行工具调度的分组、上限、顺序与取消 | `05-parallel-scheduler.md` |
| DSH-R2-06 | 追踪子 Agent 的创建、上下文、模型、工具、终态和回收 | `06-subagent-lifecycle.md` |

## DSH-R2-01：追踪 System Prompt 的组装与请求序列化

- **单一动作**：只追踪一次 Agent 请求的系统提示词如何从各贡献者组装并进入 Provider 请求。
- **输入来源**：源码树中的 `packages/core/system-prompt/src/index.ts`、`packages/core/system-prompt/tests/*`、`packages/core/agent-loop/src/agent.ts`、`packages/llm/llm-pi-ai/src/context.ts`。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/01-system-prompt-assembly.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：`05-source-baseline.md` 的 commit 校验通过。
- **执行范围**：section 注册、排序/去重、模板变量、作用域、每步重建或缓存、工具 schema 注入、Provider 的前/后 system 消息、空/重复 system 内容。
- **明确禁止事项**：不根据 README 中的“model experience”直接下结论；不修改源码；不跑 provider；不读取产品 Profile 或凭证。
- **验收命令或验证方法**：`rg -n 'system|section|render|prompt|tools|schema|scope' <输出>`；逐个复核引用行；`git -C <source> status --short` 应为空。
- **可自动判断的验收标准**：报告含 `source_commit`、`package_versions`；至少 10 个源码 `path:line` 引用；明确区分 `S` 静态源码、`D` README 设计、`U` 未验证；至少列出 3 个未验证点；不得出现“README 已证明实际请求”。
- **必须提交的证据**：报告、源码引用行、实际读取命令与退出码、未验证项清单。

## DSH-R2-02：追踪 Agent Loop 的轮次状态机

- **单一动作**：只追踪一轮用户输入从排队到结束的 Agent Loop 状态迁移。
- **输入来源**：`packages/core/agent-loop/src/{agent.ts,assistant-stream.ts,index.ts,inbox.ts,runtime-context.ts,tool-calls.ts,constants.ts}`；`packages/core/agent-loop/tests/*`；`packages/core/session/src/types.ts` 与 `packages/core/session/src/index.ts`。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/02-agent-loop.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：R2-01 或源码基线已确认；本卡不依赖 R2-01 的结论。
- **执行范围**：turn/step 边界、输入队列、模型请求、assistant 流、tool-call 分支、tool result、下一步、完成、取消、失败、恢复、持久化事件。
- **明确禁止事项**：不改源码；不执行测试或模型；不把 README 的流程图当作状态机证据；不把调度细节混入本报告（留给 R2-05）。
- **验收命令或验证方法**：`rg -n 'turn|step|queue|abort|cancel|resume|finish|completed|tool' <输出>`；复核每个状态转移的引用行。
- **可自动判断的验收标准**：报告含一个文字状态图和一张状态迁移表；至少 12 个 `path:line`；每个终止分支标注完成/取消/失败/恢复；至少 3 个未验证点；不得把 `inbox` 当一个普通数组描述而不检查其边界语义。
- **必须提交的证据**：状态图、迁移表、源码引用、命令退出码、未验证项。

## DSH-R2-03：追踪 Session Replay 与 Context Projection

- **单一动作**：只追踪持久事件日志如何重建下一轮模型上下文并处理压缩、恢复和工具历史。
- **输入来源**：`packages/core/session/src/`；`packages/session/session-projection/src/`；`packages/session/session-persistence-jsonl/src/`；`packages/compaction/compaction-basic/src/`；相关 tests。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/03-session-replay-projection.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：源码基线已确认。
- **执行范围**：事件模型、seq/范围、追加与恢复、torn tail/repair、projection 注册/读取、compaction 边界、tool history、request reconstruction、resume 与 fork 的区别。
- **明确禁止事项**：不读取真实会话；不运行持久化测试；不复制完整日志或源码；不把“JSONL 可解析”当成“语义可恢复”。
- **验收命令或验证方法**：`rg -n 'seq|replay|projection|compaction|resume|repair|tool|history|cache' <输出>`；源码行复核；`git -C <source> status --short` 为空。
- **可自动判断的验收标准**：报告至少 12 个 `path:line`；覆盖“日志→投影→请求上下文”的完整链路；明确列出哪些状态可重放、哪些依赖运行态；至少 3 个未验证点；不出现把 Alpha 研究结论当 rc.5 证据的表述。
- **必须提交的证据**：链路图、源码引用、恢复/压缩边界、未验证项、读取命令退出码。

## DSH-R2-04：追踪 Tool Description 与 Schema 的注册、过滤和序列化

- **单一动作**：只追踪一个工具如何从注册定义变成模型请求中的名称、描述和 JSON Schema。
- **输入来源**：`packages/core/tools/src/{index.ts,schema.ts,types.ts,json-schema.ts,ts-types.ts,py-types.ts,ptc.ts,ptc-calls.ts}`；`packages/core/system-prompt/src/index.ts`；`packages/llm/llm-pi-ai/src/context.ts`；代表性工具包。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/04-tool-schema.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：源码基线已确认。
- **执行范围**：工具注册、作用域/过滤、名称冲突、描述来源、参数 schema 生成、presentation 与模型可读字段的分离、动态/延迟工具、PTC 工具面、Provider 序列化路径。
- **明确禁止事项**：不运行工具；不修改源码；不把 UI presentation 当作模型描述；不把所有工具清单抄成无行号表格。
- **验收命令或验证方法**：`rg -n 'defineTool|schema|json|description|toolFilter|presentation|deferred|ptc|serialize' <输出>`；至少 3 个代表性工具的路径核对。
- **可自动判断的验收标准**：报告至少 12 个 `path:line`；包含工具定义→注册表→Agent 可见集→Provider 请求的四段链路；区分 schema、description、presentation 三种文本；至少 3 个未验证点。
- **必须提交的证据**：链路图、代表性工具对照表、源码引用、未验证项、命令退出码。

## DSH-R2-05：追踪并行工具调度的分组、上限、顺序与取消

- **单一动作**：只追踪一个 assistant step 内多个 tool call 的分组和调度。
- **输入来源**：`packages/core/agent-loop/src/{tool-calls.ts,constants.ts,agent.ts,index.ts}`；`packages/core/tools/src/{index.ts,schema.ts}`；`packages/core/agent-loop/tests/{tool-order.spec.ts,tool-calls.spec.ts,cancel.spec.ts}`；`packages/core/tools/tests/execution-mode.spec.ts`。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/05-parallel-scheduler.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：源码基线已确认；不依赖 R2-02 结论。
- **执行范围**：`parallel`/`exclusive` 分类、默认上限、滚动池、exclusive barrier、重新分类、开始/提交顺序、结果顺序、取消时 synthetic result、调度器失败、context 注入。
- **明确禁止事项**：不运行调度测试；不声称并发实测；不修改源码；不把 `Promise` 的完成顺序当成结果提交顺序。
- **验收命令或验证方法**：`rg -n 'DEFAULT_MAX_PARALLEL|parallel|exclusive|inFlight|commit|abort|synthetic|context|maxParallel' <输出>`；核对 test 文件中的断言与源码路径。
- **可自动判断的验收标准**：报告明确写出默认上限数值；包含一个 4-call 的示例时序表（parallel/exclusive 混合）；至少 10 个 `path:line`；明确“dispatch 重叠、commit 模型序”；至少 3 个未验证点。
- **必须提交的证据**：时序表、状态说明、源码引用、测试代码引用、未验证项。

## DSH-R2-06：追踪子 Agent 的创建、上下文、模型、工具、终态和回收

- **单一动作**：只追踪一次 `subagent` / `subagent_fork` 委托从创建到结果交付和回收。
- **输入来源**：`packages/subagent/tool-subagent/src/`；`packages/subagent/subagent-spawn-in-process/src/`；`packages/subagent/subagent-fork-in-process/src/`；`packages/core/agent-loop/src/agent.ts`；`packages/core/session/src/fork.ts`；相关 tests 和 `packages/subagent/*/README.md` 仅作 `D` 线索。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase2/06-subagent-lifecycle.md`。
- **涉及文件**：只读上述源码；只写输出 Markdown。
- **前置依赖**：源码基线已确认。
- **执行范围**：spawn 与 fork 的上下文差异、agent preset/model/toolFilter、maxDepth、模型继承/覆盖、result delivery、深度限制、provider capability 校验、取消/失败/回收。
- **明确禁止事项**：不启动子 Agent；不读取真实子 Agent 会话；不改源码；不把 README 的“模型经验”当行为证据。
- **验收命令或验证方法**：`rg -n 'spawn|fork|inherit|maxDepth|agentPreset|agentOptions|model|toolFilter|dispose|cancel|result' <输出>`；核对 spawn/fork provider 源码与测试。
- **可自动判断的验收标准**：报告含 spawn 与 fork 对照表；至少 12 个 `path:line`；明确“上下文的继承边界”和“工具/权限是否继承”分开回答；至少 3 个未验证点。
- **必须提交的证据**：spawn/fork 对照表、生命周期图、源码引用、未验证项、命令退出码。

## 统一交付

- 每份报告开头写 `source_commit`、`source_path`、`package_versions`。
- 每个结论标注 `S`、`D` 或 `U`；不得用 `D` 代替 `S`。
- 每份报告末尾写“未验证项 / 反证 / 下一张卡不要混入的边界”。
- 完成一张卡即停止；研究负责人逐卡复核后才允许进入下一轮。
