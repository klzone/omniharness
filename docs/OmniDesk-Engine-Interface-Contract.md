# OmniDesk 引擎接口契约（v0.2 · 已冻结 / 已实现 / 已验证）

- 文档状态：**契约已冻结；实现与验证已完成**（本文件仍描述接口语义，属稳态契约，随实现证据推进只增不删）
- 实现：`engine/omnidesk_engine/`（零第三方依赖）
- 验证：E01–E20 隔离实测 20/20 pass，见 [OmniDesk-Engine-Interface-Test-Matrix.md](OmniDesk-Engine-Interface-Test-Matrix.md) 与 [../tests/reports/p1-engine-adapter-prototype-report.md](../tests/reports/p1-engine-adapter-prototype-report.md)
- 版本：v0.2（语义层 v0.1 未变；v0.2 为状态收口）
- 日期：2026-10-16
- 阶段：OmniDesk P1 第二步（契约设计）+ 第三步（实现与验证）
- 上游依据：[docs/OmniDesk-Engine-Interface-Audit.md](OmniDesk-Engine-Interface-Audit.md)（P1 第一步只读审计）
- 下游产物：[docs/OmniDesk-Engine-Interface-Test-Matrix.md](OmniDesk-Engine-Interface-Test-Matrix.md)（接口验收测试矩阵 v0.2，已执行）

> 本文只定义 OmniDesk 自有的、与具体引擎解耦的接口边界。它**不修改** DSH Runtime，**不实现**完整桌面应用（最小适配器原型见 `engine/omnidesk_engine/`），**不开发** GUI。文中所有能力都必须区分「设计」「审计事实」「当前限制」「待验证」，不得把设计写成已实现，也不得把静态检查写成运行验证。

标注约定：

- `[设计]` —— OmniDesk 侧规范（契约语义）；实现状态以测试矩阵与交付报告为准。
- `[审计事实]` —— 来自第一步审计报告的结论，保留其原始证据等级（`static` / `script` / `model` / `unverified`）。
- `[限制]` —— 底层当前不支持或无法保证的能力。
- `[待验证]` —— 需在第三步用真实进程或真实模型验证，本阶段不执行、不预判结果。

---

## 1. 引擎抽象边界

### 1.1 九条边界原则

1. `[设计]` **OmniDesk 是独立产品。** 它的核心工作流不得依赖 DSH 官方 UI 才能完成。DSH Web UI 只作为过渡期的能力验证环境。
2. `[设计]` **OmniDesk 拥有自己的产品层。** UI、任务状态、产品配置、工作成果管理与用户交互均由 OmniDesk 定义和维护，不寄生于引擎内部对象。
3. `[设计]` **DSH 只是初期可替换的 AI 执行引擎。** 是否继续使用、是否更换，由能力、稳定性与维护成本的证据决定。
4. `[设计]` **ACP v1 是当前首选通信通道，但不是 OmniDesk 的公开产品 API。** 它是适配器内部实现细节，UI 与领域流程不直接消费 ACP 消息结构。
5. `[设计]` **headless 是能力受限的备用通道。** 不得把它伪装成具备完整任务生命周期管理（尤其是取消与恢复）的引擎。
6. `[设计]` **Web 私有协议不作为正式产品契约。** 仅保留为历史或过渡验证信息，不承诺兼容性，本阶段不做在线探测、不新增私有协议依赖。
7. `[设计]` **UI 只依赖 OmniDesk 内部契约。** 禁止 UI 直接依赖 ACP 消息结构、DSH 配置文件或 DSH 内部对象。
8. `[设计]` **引擎可替换。** 更换引擎时应尽量只替换引擎适配器，不重写 UI 与业务流程。
9. `[设计]` **本阶段的契约与语言无关。** Rust + TypeScript 独立桌面目标不改变本阶段范围；本阶段只定义与实现语言无关的接口语义。

### 1.2 分层职责

| 层 | 归属 | 职责 | 明确不做 |
| --- | --- | --- | --- |
| UI 层 | OmniDesk | 呈现任务、状态、结果、错误；发起用户动作 | 不解析 ACP/JSON-RPC 报文，不读 DSH 会话文件 |
| 领域/工作流层 | OmniDesk | 材料写作、知识检索等业务编排；模板与规则选择 | 不直接与引擎进程通信 |
| 应用服务层（任务管理层） | OmniDesk | 任务标识、状态机、事件归一化、结果持久化、取消/恢复仲裁 | 不假设某通道必然具备某能力 |
| 引擎适配层 | OmniDesk | 把统一契约映射到 ACP / headless（/ SDK）；通道能力探测与差异收敛 | 不修改 DSH Runtime；不伪造成功 |
| 引擎 Runtime | DSH（外部依赖） | 实际执行 AI 任务、Skill、模型调用 | （OmniDesk 不修改其代码） |

### 1.3 本阶段边界

- 本阶段交付：本契约文档 + 测试矩阵文档 + 路线图/简报状态同步 + 静态检查 + 交付报告。
- 本阶段**不**交付：适配器实现、协议客户端、桌面 UI、数据库迁移、新依赖项。
- 如果发现必须修改程序代码才能继续，应先记录原因与建议并暂停，等待批准。

---

## 2. 术语与标识分层

不同层次的标识**不得混用**。UI、领域层只看到 `omni_task_id`；其余标识属于适配层内部细节。

| 标识 | 归属 | 含义 | 稳定性 |
| --- | --- | --- | --- |
| `omni_task_id` | OmniDesk | 一次用户提交所对应的任务唯一标识，由 OmniDesk 生成并持久化 | 稳定、可长期引用 |
| `engine_session_id` | 引擎 | DSH 侧会话标识（ACP `sessionId`、headless `session` 事件 `sessionId`） | 引擎相关，可能随引擎更换而变 |
| `engine_request_id` | 协议 | ACP/JSON-RPC 单次请求的关联 ID | 请求级、短生命周期 |
| `engine_turn_id` | 引擎 | 会话内的一次轮次/运行编号（若底层提供） | 引擎相关，可选 |

规则：

1. `[设计]` `submit_task` 返回稳定的 `omni_task_id`；`engine_session_id` 仅作为可选关联字段保留在适配层。
2. `[设计]` UI 与领域层不得依赖 `engine_session_id`、`engine_request_id`、`engine_turn_id` 的格式或存在性。
3. `[审计事实]` ACP 与 headless 的 `sessionId` 由底层生成；SDK 的 `sessionId` 由客户端在 `prompt` 时提供（`static`）。三者语义不同，故不得直接暴露底层字段名。
4. `[设计]` 一个 `omni_task_id` 最多绑定一个当前有效的 `engine_session_id`；重绑定必须记录为一次显式恢复操作，不得静默替换。

---

## 3. 状态模型

### 3.1 状态枚举

`[设计]` OmniDesk 任务状态集：

| 状态 | 含义 | 是否终态 |
| --- | --- | --- |
| `queued` | 已受理，尚未开始执行 | 否 |
| `starting` | 适配层正在建立会话/启动执行 | 否 |
| `running` | 引擎正在执行 | 否 |
| `cancelling` | 已发出取消请求，尚未取得终态证据 | 否 |
| `succeeded` | 已取得完整成功结果并落盘 | 是 |
| `failed` | 任务执行失败，且有明确错误证据 | 是 |
| `cancelled` | 有可靠终态证据表明任务已被取消 | 是 |
| `interrupted` | 客户端/引擎非正常中断，结果不完整且无法确认 | 是 |
| `unknown` | 无法确认任务真实状态（例如恢复后发现旧任务仍在执行） | 是 |

### 3.2 合法状态转换

```text
queued      -> starting | running | cancelled | failed | unknown
starting    -> running | failed | interrupted | unknown
running     -> succeeded | failed | cancelling | interrupted | unknown
cancelling  -> cancelled | failed | interrupted | unknown
succeeded   -> (终态，不可再变)
failed      -> (终态，不可再变)
cancelled   -> (终态，不可再变)
interrupted -> (终态，不可再变)
unknown     -> (终态，不可再变；如需继续须新建任务)
```

规则：

1. `[设计]` 终态**不可**被普通事件意外覆盖。
2. `[设计]` 进入终态前若存在冲突证据，按 3.3 的仲裁优先级处理。

### 3.3 冲突仲裁优先级

`[设计]` 当协议返回、进程状态与本地记录不一致时，按以下优先级判定（高优先级覆盖低优先级）：

1. 引擎明确给出的**终态证据**（例如 ACP 的 stop reason、headless 的终端 `final` 事件 + 退出码）。
2. 引擎明确给出的**结构错误**（JSON-RPC error / `error` 事件）。
3. 操作者显式动作的**确认记录**（取消确认）。
4. 客户端**进程状态**（活/退出）。
5. 本地推断（最低，不得单独作为终态依据）。

### 3.4 三条禁止推断

1. `[设计]` 不得把**进程退出**直接等同于任务成功。
2. `[设计]` 不得把**收到最终文本片段**直接等同于任务成功（须有完整结果与落盘证据）。
3. `[设计]` 不得把 **kill 进程**直接等同为优雅取消（应区分 `cancelled` 与 `process_terminated`。


---

## 4. 最小接口契约

能力标记三态（描述「底层对该能力的支持程度」，与证据等级 `static`/`script`/`model`/`unverified` 配合使用，两者不得互相替代）：

- `protocol_visible` —— 协议或源码可见该方法/字段，但未运行验证。
- `implementation_supported` —— 现有实现确实提供该能力（脚本或运行级证据）。
- `verified` —— 已用真实运行证据验证通过。

### 4.1 引擎能力与初始化

`[设计]` 操作：`initialize()` -> `EngineDescriptor`。

输入：无（或适配层内部配置引用，不暴露给 UI）。

输出 `EngineDescriptor`：

```yaml
engine_kind: dsh-acp | dsh-headless | dsh-sdk        # 适配器内部枚举
engine_version: string          # 例：0.2.1-alpha.2
protocol_version: string        # 例：acp/1
compatibility: compatible | degraded | incompatible
healthy: bool
capabilities:
  submit_task:      capability_state
  get_task_status:  capability_state
  stream_events:    capability_state
  cancel_task:      capability_state
  recover_session:  capability_state
  recover_task:     capability_state
  get_task_result:  capability_state
capability_state: protocol_visible | implementation_supported | verified | partial | unverified | unsupported
```

规则：

1. `[设计]` **不得**仅凭「支持 ACP」推断全部能力均可用；每项能力独立标注。
2. `[设计]` 不支持的能力必须返回标准 `unsupported` 错误，**不得自动回退到语义不同的通道**而隐藏能力差异。
3. `[设计]` 版本与协议版本必须记录，不兼容时明确报错（`protocol_incompatible`）。
4. `[审计事实]` ACP 的 `initialize` 返回 `sessionCapabilities={close,list,resume}`、`authMethods:[]`（`script`）；SDK 的 `initialize` 返回 `serverInfo`（`script`）。这些是**能力声明**，仍需逐项运行验证。
5. `[设计]` 初始化超时、引擎不可用必须映射为 4.9 的标准错误，不得静默重试造成重复启动。

### 4.2 submit_task

`[设计]` 操作：`submit_task(SubmitTaskRequest) -> SubmitTaskReceipt`。

`SubmitTaskRequest`：

| 字段 | 说明 |
| --- | --- |
| `omni_task_id` | 由 OmniDesk 生成；可预分配以便幂等 |
| `idempotency_key` | 幂等键；同一键重复提交不得导致同一任务重复执行 |
| `intent` | 任务类型/用途（例：`material_writing` / `knowledge_retrieval`） |
| `input_ref` | 输入内容引用或受控载荷（引用优先，避免把大段内容写入日志） |
| `workspace_dir` | 工作目录及访问范围 |
| `timeout_policy` | 超时策略（总超时、无输出超时） |
| `requested_at` | 任务创建时间 |

`SubmitTaskReceipt`：

- `omni_task_id`（稳定）
- `status`（初始为 `queued` 或 `starting`）
- `engine_session_id`（可选，适配层内部）
- `accepted_at`

规则：

1. `[设计]` 返回的 `omni_task_id` 必须稳定且可长期引用。
2. `[设计]` **提交回执不确定时不得盲目重试**（可能造成同一任务重复执行）；必须以 `idempotency_key` 去重，无法确认时进入 `unknown`。
3. `[设计]` 不同层次的 ID 不得混用（见第 2 节）。
4. `[审计事实]` headless 为**一进程一任务**（`static` + `script`）；ACP `session/prompt` 与最终结果同源；SDK `session/prompt` 只回 `messageId` 回执（`script`）。适配层须对三种语义做归一化。

### 4.3 get_task_status

`[设计]` 操作：`get_task_status(omni_task_id) -> TaskStatus`。

输出：`{ omni_task_id, status, evidence_source, since, last_event_seq, partial_output_size }`。

规则：

1. `[设计]` 完整状态模型见第 3 节；必须区分执行失败、用户取消、引擎中断与状态未知。
2. `[设计]` 三条禁止推断（3.4）在此处强制执行。
3. `[设计]` 协议返回、进程状态、最终结果不一致时按 3.3 仲裁。
4. `[审计事实]` 各通道状态粒度不同：ACP 有 running/idle + stop reason（`end_turn`/`max_tokens`/`cancelled`）；SDK 只有 `running`/`idle`；headless 只有进程活/退出（`script` / `static`）。
5. `[限制]` headless 通道**无法**区分「成功/失败」的细分状态，只能依赖终端 `final` 事件与退出码；契约须显式标注该降级。

### 4.4 stream_events

`[设计]` 操作：`stream_events(omni_task_id) -> AsyncStream<EngineEvent>`。

归一化事件类型：

| 事件 | 来源 |
| --- | --- |
| `task_started` | 引擎开始执行（归一化） |
| `text_delta` | 增量文本（ACP `session/update` / headless `text` / SDK `session.event`） |
| `progress` | 进度（headless `status` phase / SDK `session.status`） |
| `tool_activity` | 工具调用记录 |
| `task_completed` | 成功终态 |
| `task_failed` | 失败终态 |
| `cancel_requested` | 已发出取消请求（本地事件） |
| `task_cancelled` | 引擎确认取消 |
| `task_interrupted` | 非正常中断 |
| `protocol_error` | 协议层错误 |

事件字段：`{ omni_task_id, seq, occurred_at, type, payload }`。

规则：

1. `[设计]` **对 ACP 不原生提供的事件，不得声称底层原生支持**；须标注「OmniDesk 归一化事件」及其推导规则。
2. `[设计]` 增量文本与最终完整结果**分离**；`text_delta` 不得作为完整结果（见 4.5）。
3. `[设计]` **工具调用记录不得默认等同于工具已成功执行**（headless `tool_call`/`tool_result` 是有界投影，结果可能被截断）。
4. `[设计]` 重复事件、乱序事件按 `seq` 去重与重排；断流须可识别并保留已收到部分输出；解析失败产生 `protocol_error`，不合成成功事件。
5. `[设计]` 若无底层证据，**不得合成成功事件**。
6. `[审计事实]` headless `--json` 事件为**有界投影**（`MAX_STRING_BYTES=8KB`、`MAX_EVENT_BYTES=32KB`），`final` 事件豁免截断（`static`）。

### 4.5 get_task_result

`[设计]` 操作：`get_task_result(omni_task_id) -> TaskResult`。

必须区分的返回形态：

| 形态 | 含义 |
| --- | --- |
| `complete` | 完整成功结果 |
| `partial` | 部分输出（失败或取消时保留的已接收内容） |
| `not_ready` | 结果尚未就绪 |
| `unrecoverable` | 结果不可恢复 |

`TaskResult`：`{ omni_task_id, form, content, persisted_at, integrity: complete|partial, sources[] }`。

规则：

1. `[设计]` **部分结果不得冒充完整成果**；`integrity=partial` 必须显式标注。
2. `[设计]` 失败或取消时保留已接收内容，但标注非完整状态。
3. `[设计]` 定义结果持久化时机与完整性标记；区分「引擎完成」与「本地持久化完成」（见测试场景 E13）。
4. `[设计]` 结果不可恢复时**明确报告**，**不得重新执行任务来伪造恢复结果**。

### 4.6 cancel_task

`[设计]` 操作：`cancel_task(omni_task_id) -> CancelOutcome`。

取消五阶段（严格区分）：

1. `not_requested` —— 尚未发出取消请求。
2. `request_sent` —— 取消请求已发出。
3. `acknowledged` —— 引擎已确认收到取消。
4. `confirmed_cancelled` —— 任务已确认进入 `cancelled` 终态。
5. `timeout_unknown` —— 取消超时或结果未知。

规则：

1. `[设计]` 返回「请求已发送」**不能**显示为「取消成功」。
2. `[设计]` 只有收到可靠终态证据，才允许向上层报告取消完成。
3. `[设计]` 取消失败或超时须给出明确状态。
4. `[设计]` 若底层只支持终止客户端进程，必须**明确说明不能证明远端模型任务已停止**。
5. `[设计]` 取消后**不得自动重新提交**同一任务。
6. `[设计]` 定义取消与成功完成同时发生时的仲裁规则（按 3.3，引擎终态证据优先；若先得成功终态则不得改判为 cancelled）。
7. `[审计事实]` 仅 ACP 支持 `session/cancel`，`interrupted` 映射为 stop reason `cancelled`（`script`）；SDK 实测 `session/cancel` 返回 `-32603 unknown method`（`script`）；headless 无取消旗标（`static`）。
8. `[限制]` headless / SDK 通道的「取消」实为结束进程，契约须暴露为 `process_terminated`，不得谎报为优雅取消。

### 4.7 recover_session 与 recover_task

`[设计]` 两者分离：

- `recover_session(engine_session_id | hint) -> SessionRecovery` —— 重新连接或接管既有会话。
- `recover_task(omni_task_id) -> TaskRecovery` —— 恢复某任务的状态、事件与已有结果。

必须覆盖的情形：

1. 正常关闭后重新启动。
2. 客户端进程被强制终止。
3. 任务正在运行时客户端退出。
4. 任务已成功但本地尚未保存最终结果。
5. 会话存在但原任务状态未知。
6. 会话不存在或引擎拒绝恢复。
7. 恢复期间收到重复事件。
8. 恢复时发现旧任务仍在执行。

硬性要求：

1. `[设计]` **恢复失败不得静默新建会话。**
2. `[设计]` **恢复会话不意味着自动重放中断的任务。**
3. `[设计]` 无法确认执行结果时必须进入 `unknown` 或 `interrupted`。
4. `[设计]` **不允许自动重放状态不明的任务。**
5. `[限制]` 若 DSH/ACP 无法提供某项保证，须在契约中标注限制，**不得用本地推测补成保证**。
6. `[审计事实]` ACP：强杀后新进程仍可 `list` + `resume`，但**进行中的一轮不会自动重放**；未知 id 返回 `-32602 session is not resumable`（`script`）。SDK：无 list/resume，未知 id 在 `prompt` 时**惰性创建**，无法区分恢复与新建（`script`）。headless：同 cwd `--session-id` 可 adopt（turn 1→2），异 cwd/未知 id 明确拒绝（`script`）。

### 4.8 关闭与资源释放

`[设计]` 操作：`shutdown(mode, grace_period) -> ShutdownReport`。

必须分别定义三类不同操作：

1. 关闭客户端连接（只断开适配层与引擎的连接）。
2. 停止本地进程（结束引擎子进程）。
3. 取消远端任务（请求引擎停止执行中的任务）。

规则：

1. `[设计]` 优雅关闭：等待在途事件与结果落盘；有任务执行时按策略（等待/取消/保留）处理。
2. `[设计]` 关闭超时后强制终止需记录后果（可能丢失未落盘输出）。
3. `[设计]` 下次启动按 `recover_session` / `recover_task` 策略恢复；强制终止不触发自动重放。
4. `[审计事实]` ACP `session/close` 返回 `{}`，关闭后仍可 `resume`（`script`）；SDK 有 `shutdown` 方法（`static`）；headless 无显式关闭（进程结束即结束）。

### 4.9 错误模型

`[设计]` 统一错误分类：

| 错误码 | 说明 |
| --- | --- |
| `invalid_argument` | 参数错误 |
| `engine_unavailable` | 引擎不可用 |
| `protocol_incompatible` | 协议不兼容 |
| `unknown_session` | 未知会话 |
| `unknown_task` | 未知任务 |
| `unsupported` | 不支持的能力 |
| `task_failed` | 任务执行失败 |
| `cancel_failed` | 取消失败 |
| `recover_failed` | 恢复失败 |
| `timeout` | 超时 |
| `event_parse_error` | 事件解析失败 |
| `result_persist_failed` | 结果持久化失败 |
| `state_uncertain` | 状态不确定 |

每类错误须定义五属性：

| 属性 | 取值 |
| --- | --- |
| 是否可重试 | yes / no / conditional |
| 是否需要人工介入 | yes / no |
| 是否允许降级 | yes / no |
| 是否可能造成重复执行 | yes / no |
| 是否可能丢失已接收输出 | yes / no |

规则：`[设计]` **重试策略不得默认对写作任务自动重放。**

---

## 5. 通道能力矩阵

五态标记（**不得合并**成简单的「支持/不支持」）：

- `protocol_visible` —— 协议/源码可见该方法或字段（未运行验证）。
- `probe_verified` —— 已有真实进程探测证据（第一步审计）。
- `partial` —— 仅部分支持，存在明确缺口。
- `unverified` —— 尚未验证。
- `unsupported` —— 探测或源码确认不支持。

| 能力 | A headless | B sdk | C acp（首选） | D web |
| --- | --- | --- | --- | --- |
| initialize | probe_verified | probe_verified | probe_verified | static only |
| submit_task | probe_verified | probe_verified | probe_verified | unverified |
| get_task_status | partial | probe_verified | probe_verified | unverified |
| stream_events | probe_verified（有界投影） | probe_verified | probe_verified | unverified |
| cancel_task | unsupported | unsupported | probe_verified | unverified |
| recover_session | partial（同 cwd adopt） | unverified（无法区分新建） | probe_verified | unverified |
| recover_task | partial | unverified | partial（可发现、不自动重放） | unverified |
| get_task_result | probe_verified（final 无损） | partial（需自行拼装） | probe_verified | unverified |
| close / shutdown | unsupported（进程结束即结束） | protocol_visible | probe_verified | unverified |

证据等级：A/B/C 为 `script`（真实进程，无模型推理）；D 为 `static`（源码与帮助面），运行时 `unverified`。上表 A/B/C 项均来自第一步审计，属**历史审计证据**，不是本阶段实测。

### 5.1 ACP（首选，进入后续适配器验证）

- `[设计]` 作为首选通道进入后续适配器验证。
- `[设计]` 取消与恢复**必须用运行证据**验证，不得仅凭 `sessionCapabilities` 声明。
- `[设计]` 明确区分「恢复会话」与「恢复正在执行的任务」。
- `[设计]` 明确关闭语义与异常退出语义。
- `[审计事实]` 方法面：`initialize`/`session/new`/`session/list`/`session/resume`/`session/close`/`session/setConfigOption`/`session/prompt`；通知 `session/cancel`、`session/update`（`static` + `script`）。

### 5.2 headless（有限能力补充通道）

- `[设计]` 仅作有限能力的补充通道，用于不需要取消/恢复的短任务与冒烟测试。
- `[限制]` **无法保证取消能力**（无取消旗标）。
- `[设计]` 定义其适用任务类型：一次性、短时、无需取消/恢复的任务。
- `[设计]` 若用于简单任务，必须说明任务失败或客户端中断时的状态边界。

### 5.3 SDK

- `[限制]` **不得把惰性创建未知会话当作恢复成功。**
- `[设计]` 未知会话 ID 的处理方式：显式返回 `unknown_session`，不静默新建。
- `[限制]` 若 SDK 能力不足以满足完整契约，应**限制用途**，而不是伪装成等价 ACP。

### 5.4 Web 私有协议

- `[设计]` 仅保留为历史或过渡验证信息。
- `[设计]` **不承诺**正式产品兼容性。
- `[设计]` 本阶段**不进行在线探测**，不新增私有协议依赖。

---

## 6. 人格前缀（personaPrefix）与任务用途契约

### 6.1 风险记录

1. `[审计事实]` 当前 `yvan-work`（web 形态）**有效** `personaPrefix` 为 `You are a coding agent powered by the {{model}} model.`（`static` + `script`，离线 `--dump-config` 合成，未调用模型；证据：`tests/verification/engine-interface/evidence/profile-persona-composition.txt`）。
2. `[审计事实]` 分层来源（bundle 补丁栈，后者覆盖前者）：`dsh-base` -> `personaPrefix: ''`；`dsh-web-app` -> 将该值设为 coding agent 前缀；`profiles/yvan-work/cordis.patch.yml` **未**覆盖 `system-prompt.personaPrefix`。
3. `[审计事实]` 对照：`dsh-acp-app` 与 `dsh-headless` 同为 coding agent；`dsh-sdk-app` 无该条目（继承空串）；`dsh-sdk-minimal` 为 `process.env.DSH_SYSTEM_PROMPT ?? 'You are a helpful software engineer assistant.'`。
4. `[限制]` 该值来自 bundle 补丁层，**可由上层 profile 补丁覆盖，无需修改 runtime**。
5. `[设计]` **定性：这是配置层风险，不是模型行为结论。** 不得据源码文本推断模型在非编码任务上的实际输出。

### 6.2 处置原则

1. `[设计]` 本阶段**不修改** Profile，不改 DSH Runtime，不读取或复制凭证。
2. `[设计]` 覆盖 `personaPrefix` 属第三步候选动作，须经批准后用 profile 补丁层实现，并留证据。
3. `[设计]` 在覆盖前，任何「模型是否受此前缀影响」的结论都属 `unverified`。

### 6.3 纯合成测试设计（本阶段只设计，不执行）

测试任务（均使用纯虚构素材，不含真实人员/机构/案件/内部信息）：

| 编号 | 合成任务 | 观察点 |
| --- | --- | --- |
| P-A | 根据虚构事实生成工作总结 | 是否被引导成代码任务口吻；结构是否符合用途 |
| P-B | 根据合成制度回答带原文引用的问题 | 是否给出来源文件与条款位置；是否越界补充 |
| P-C | 面对相互矛盾的数字明确标记待确认 | 是否指出冲突而非二选一 |

控制条件（比较必需满足）：

1. `[设计]` **相同模型**（同一 provider/model/reasoningEffort）。
2. `[设计]` **相同输入**（同一合成素材与提示）。
3. `[设计]` **相同知识资产**（同一模板、风格规则、检索夹具）。
4. `[设计]` **可记录的配置差异**（仅 `personaPrefix` 一项，其余配置指纹一致）。

证据要求：

1. `[设计]` 记录**实际人格前缀**（离线 `--dump-config` 合成）。
2. `[设计]` 记录**有效配置来源**（哪一层补丁生效）。
3. `[设计]` 记录**真实输出**原文与**人工判定**结果。
4. `[限制]` 若模型凭证不可用，记录阻塞与复现步骤；**不得把静态配置检查写成模型行为验证**。
5. `[设计]` 本阶段**只定义测试，不执行**新的模型调用，**不读取或复制凭证**。

---

## 7. 本阶段硬约束（集中声明）

1. **不修改 DSH Runtime**（`runtime/deepseek-harness/`、`runtime/dsh-runtime/`）。
2. **不开发 GUI**，不建立 Core 或插件市场。
3. **不读取、打印、复制或修改任何凭证**（`.credentials.yaml` 绝不触碰）。
4. **不接触仓库外个人数据**；不接入正式 234 份制度库。
5. **案件材料、案件线索或未公开调查信息禁止进入个人 AI 系统。**
6. **不提交、不推送、不打标签**（tag 仅 `discipline-v0.2`）。
7. **不覆盖**现有用户修改。
8. 不新增完整运行时模块、协议客户端、数据库迁移或依赖项。
9. 不启动模型调用，不测试凭证，不运行新的在线协议探测。
10. 每项结论标明证据类型（`static` / `script` / `model` / `unverified`）；引用历史运行证据须注明属历史测试。

---

## 8. 持久化与安全边界

> 本节只定义**逻辑责任与约束**，本阶段**不实现**数据库或文件系统模块。

### 8.1 责任归属

| 数据 | 归属 | 说明 |
| --- | --- | --- |
| `omni_task_id` | OmniDesk | 由 OmniDesk 生成、持久化，长期稳定 |
| 任务状态 | OmniDesk | OmniDesk 自有状态机（第 3 节），不直接暴露引擎内部状态 |
| 会话映射 | OmniDesk 适配层 | `omni_task_id` 与 `engine_session_id` 的映射属适配层内部 |
| 结果元数据 | OmniDesk | 结果形态、完整性标记、来源与落盘时间由 OmniDesk 记录 |

### 8.2 本地任务状态与结果存放

- `[设计]` 本地任务状态与结果保存在**仓库外**的独立数据根（与知识资产同属可迁移文件体系），不得写入 DSH Runtime 目录。
- `[设计]` 临时数据根单独创建（见 8.5），与用户日常数据根隔离。

### 8.3 事件日志脱敏

- `[设计]` 事件日志**不得**包含凭证、敏感材料或完整输入内容；输入以引用（`input_ref`）为主。
- `[设计]` 日志须支持「可复原任务状态」，但不落到「完整还原敏感原文」的粒度。

### 8.4 部分结果保留

- `[设计]` 取消、失败、异常退出时**保留已接收的部分结果**，并标注 `integrity=partial`。
- `[设计]` 不得覆盖原始素材文件；AI 整理后的文稿与原始输入必须是可区分的两份内容。

### 8.5 临时数据根

- `[设计]` 临时数据根通过启动级环境变量（`DSH_HOME` / `DSH_AGENTS_HOME`）指向一次性目录来创建、清理和验证。
- `[审计事实]` 隔离机制由启动级环境变量决定，且探测前后对四个目标（`~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI`）的元数据快照差异全为 0（`script`）。

### 8.6 引擎工作目录与访问范围

- `[设计]` 每次任务须显式声明 `workspace_dir` 及访问范围。
- `[设计]` **引擎不得默认读取任意用户目录**；访问范围须最小化并留证据。

### 8.7 凭证隔离

- `[设计]` **不把会话恢复所需数据与凭证内容混合存储**；恢复元数据不得包含凭证。

### 8.8 数据分级红线

- `[设计]` **不允许案件材料、案件线索或未公开调查信息进入个人 AI 系统。**
- `[设计]` 此红线须同时写入实际执行的 Skill 正文（已由 `material-writing` Skill 承载），不能只停留在产品设计文档。

---

## 9. 与路线图的对应

| 阶段/事项 | 状态 |
| --- | --- |
| 引擎接口只读审计 | **已完成**（第一步报告，结论以报告证据范围为限） |
| 引擎接口契约 | **已冻结**（本文档；实现按其语义落地） |
| 接口测试矩阵 | **已执行**（矩阵 v0.2；E01–E20 全部运行，20/20 pass） |
| 适配器原型 | **已完成**（`engine/omnidesk_engine/`；证据见交付报告） |
| 独立 GUI | **未开始** |
| 正式制度库接入 | **未授权** |
| 发布标签 | **未创建**（仅 `discipline-v0.2`，仍需用户明确批准） |

---

## 10. 变更记录

| 版本 | 日期 | 说明 |
| --- | --- | --- |
| v0.1 | 2026-10-16 | 首版设计稿。定义引擎抽象边界、标识分层、状态模型、9 组最小接口契约、通道能力矩阵、人格前缀测试设计、持久化与安全边界。仅设计，未实现，未运行验证。 |
| v0.2 | 2026-10-16 | 实现与验证收口：按本契约实现 `engine/omnidesk_engine/`，E01–E20 在隔离环境全部运行并留证（20/20 pass）；人格前缀对照 script 臂通过、model 臂因无凭证阻塞。契约语义未变。 |
