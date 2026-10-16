# OmniDesk 引擎接口现状审计报告（P1 第一步）

- 审计日期：2026-10-16
- 审计对象：仓库内锁定的 DSH 底座 `dsh-v0.2.1-alpha.2`（见 `runtime/RUNTIME.lock`）
- 审计方式：只读源码追踪 + 只读进程级探测（全部使用一次性 `DSH_HOME`，`cwd=/tmp`）
- 本次**未**安装、未覆盖、未迁移、未删除任何 DSH 或用户数据，未修改 `runtime/deepseek-harness`，未读取、打印或复制任何凭证，未接触仓库外个人数据，未接入正式制度库
- 本文对应 P1 计划的**第一步（只读审计）**。（原文此处写「第二步 / 第三步均未开始」为**成文当时**状态；其后第二步契约已完成、第三步最小适配器原型已实现并通过 E01–E20 验证，当前状态以 [OmniDesk-Engine-Interface-Test-Matrix.md](OmniDesk-Engine-Interface-Test-Matrix.md) 与 [../tests/reports/p1-engine-adapter-prototype-report.md](../tests/reports/p1-engine-adapter-prototype-report.md) 为准。）

本报告严格区分四类内容：**已由源码证实的事实**、**已由实测证实的事实**、**尚未证实的推断与边界**、**推荐方案及实施计划**。每项结论标注证据等级：`static`（静态/文件事实）、`script`（脚本或隔离进程实测）、`model`（真实模型输出）、`unverified`。

> 判定原则：**CLI 能够启动，不等于存在产品级接口。** 本报告只记录被源码或真实进程证实的能力，不以帮助文本推断未测行为，也不把未取证的通道写成“已支持”。

---

## 0. 结论摘要

**在不修改 DSH Runtime 的前提下，现有公开入口足以支撑 OmniDesk 最小引擎契约的多数能力；但四条候选通道各有缺口，任何一条都还不是“完整引擎接口”。**

支撑结论的四条硬事实：

1. DSH 提供四条可编程入口：`headless`（一次性任务 + NDJSON 事件）、`sdk`（JSON-RPC stdio）、`acp`（Agent Client Protocol v1，JSON-RPC stdio）、`web`（Host HTTP/WebSocket）。四者能力并不等价。
2. 三条 stdio 通道中，**只有 ACP v1 同时具备取消（`session/cancel`）、恢复（`session/list` + `session/resume`）与关闭（`session/close`）语义**；headless 与 sdk 都没有逐任务取消方法。
3. 隔离机制由启动级环境变量 `DSH_HOME` / `DSH_AGENTS_HOME` 决定；本审计全部探测使用一次性临时数据根，前后对 `~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI` 的元数据快照差异均为 0。
4. `web` 通道业务能力最全，但其会话/任务控制由**未发布的私有 Remote 协议**承载，不能当作稳定公开接口。

六项最小契约能力的初步结论（详细取证见第 3、4 节）：

| 能力 | 最合适通道 | 现状 | 主要缺口 |
| --- | --- | --- | --- |
| submit_task | acp / headless / sdk | 支持 | headless 为一进程一任务 |
| get_task_status | sdk / acp | 支持 | headless 仅有进程活/退出 |
| stream_events | headless / acp / sdk | 支持 | headless 为有界投影，非 token 级 |
| cancel_task | acp | 仅 acp 支持 | headless、sdk 无取消方法 |
| recover_task | acp | 部分/支持 | sdk 无法区分恢复与新建 |
| get_task_result | headless / acp | 支持 | sdk 需自行拼装 |

因此推荐以 **ACP v1** 作为 OmniDesk 引擎适配器的首选目标通道，`headless` 作为无状态兜底；引擎适配边界由 OmniDesk 自有后端承担，UI 不解析 CLI 私有输出格式。

---

## 1. 审计对象与版本基线

| 项目 | 值 | 证据 |
| --- | --- | --- |
| 源 tag | `dsh-v0.2.1-alpha.2` | `static`（`runtime/RUNTIME.lock`） |
| commit | `d743267388641bc76f17c45ce8b4c231aed1d32c` | `static` |
| npm 包 | `@deepseek-ai/dsh@0.2.1-alpha.2` | `static` |
| node engine | `^22.19.0 || >=24.0.0` | `static` |
| 安装目录 | `runtime/dsh-runtime`（gitignore） | `static` |
| 实测版本 | `--version` -> `0.2.1-alpha.2` | `script`（`evidence/raw/A0-version.txt`） |

审计入口二进制定为 `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js`，与隔离启动器 `scripts/omnidesk-dev.sh` 的首选入口一致（`static`）。

---

## 2. 已由源码证实的事实（static）

### 2.1 CLI 顶层接口面

源码帮助面（`evidence/raw/A0-help.txt`）：

```
Usage: dsh [--profile] <name> [options] [app-args...]
       dsh plugin --profile <name> <pnpm-args...>

  -V, --version
  --profile <name>
  --from-default-profile <name>
  --patch <path>
  --dump-config
  --dump-config-schema
  --dump-default-config
```

要点：`dsh <name>` 等价于 `dsh --profile <name>`，`web` / `headless` / `acp` / `sdk` 都是 Profile 名而非子命令；Profile 是一个按序叠加的插件 bundle 补丁栈。

### 2.2 Profile 模板与 bundle 组合

shipped profile 模板（`@deepseek-ai/dsh-app-boot`）：`acp=[dsh-base,dsh-acp-app]`、`web=[dsh-base,dsh-web-app]`、`headless=[dsh-base,dsh-headless]`、`sdk=[dsh-base,dsh-sdk-app]`、`sdk-minimal=[dsh-sdk-minimal]`。`DEFAULT_PROFILE_BUNDLES=["@deepseek-ai/dsh-base"]`。

### 2.3 各通道协议方法注册

- SDK（`@deepseek-ai/dsh-sdk-protocol` / `dsh-sdk-jsonrpc-server`）注册的方法集合为：`initialize`、`session/prompt`、`session/wait`、`shutdown`；通知为 `session/created`、`session/event`、`session/status`、`subagent.started`、`subagent.finished`。**不含任何取消或关闭方法。**
- ACP（`@deepseek-ai/dsh-acp` + `@agentclientprotocol/sdk@1.4.0`）侧方法：`initialize`、`authenticate`、`session/new`、`session/list`、`session/resume`、`session/close`、`session/setConfigOption`、`session/prompt`；通知 `session/cancel`、`session/update`。`AcpSession` 类同时实现 `cancel()` 与 `close()`。
- headless（`@deepseek-ai/dsh-headless`）不是协议服务，而是一次性任务运行器：帮助面只提供 `--json` 与 `--session-id` 两个旗标，**没有取消旗标**。

### 2.4 headless 事件投影与上限

`dsh-headless/lib/json-stream-*.js` 定义：

- 事件类型：`session`、`status`（phase 为 `turn_start` / `step_start` / `step_end` / `turn_end`）、`thinking`、`text`、`tool_call`、`tool_result`、`final`；错误另有 `error`。
- `MAX_STRING_BYTES = 8 * 1024`、`MAX_EVENT_BYTES = 32 * 1024`；终端 `final` 事件豁免截断。
- `--json` 事件写入 stdout，诊断写入 stderr。

结论：headless 事件是**有界投影**，不是无损 token 级流；但最终文本通过终端 `final` 事件无损给出。

### 2.5 人格前缀（personaPrefix）

`dsh-headless/cordis.patch.yml` 与 `dsh-acp-app/cordis.patch.yml` 均将 `system-prompt` 的 `personaPrefix` 固定为：

```
You are a coding agent powered by the {{model}} model.
```

源码事实：这是 bundle 补丁层的默认值，**可被上层 profile 补丁覆盖**（补丁层按序叠加）。本审计未修改 runtime，也未在非编码领域对该前缀的实际影响做独立验证（见第 5 节）。

### 2.6 隔离数据根

用户数据根由进程环境变量 `DSH_HOME` / `DSH_AGENTS_HOME` 决定（解析优先级：显式配置 -> 环境变量 -> `~/.dsh` / `~/.agents`），且这两个变量属启动级变量，只能由继承的进程环境提供。该结论已由既有的 [OmniDesk 与 DSH 运行隔离审计报告](OmniDesk-DSH-Isolation-Audit.md) 证实，本次沿用，不重复论证。

---

## 3. 已由实测证实的事实（script）

复现命令（一条命令重跑四条通道，约 50 秒）：

```
python3 tests/verification/engine-interface/probes/probe_engine_channels.py
```

落盘证据：`tests/verification/engine-interface/evidence/channel-probe.json`、`.../headless-recovery.json`、`.../isolation-sideeffects.json`，原始字节在 `.../evidence/raw/`。

> 可复现性修正：首版探测使用固定临时数据根与固定会话名，重复运行会命中上一次的既有会话，导致 SDK 通道误报 `session ... already exists`。本报告对应的证据已改为**每次运行使用进程级独立数据根**后重新采集，SDK 通道如实返回入队回执与事件流。此修正本身说明：会话存在性可被底层检出，固定会话名会被拒绝。

### 3.1 四通道探测总览

| 通道 | 协议 | 探测结果 |
| --- | --- | --- |
| A headless | 一次性 CLI + NDJSON | 正常启动，事件流完整，无取消能力 |
| B sdk | JSON-RPC stdio | 正常启动，入队回执 + 事件/状态通知，无取消方法 |
| C acp | ACP v1 JSON-RPC stdio | 正常启动，new/list/resume/close/cancel 均可用 |
| D web | Host HTTP/WS | 仅静态核对帮助面与源码，未联机调用 |

### 3.2 A 通道：headless（script）

- 无凭证运行 `--json ping`：首个事件为 `session`（含底层生成的 `sessionId` 与 `cwd`），随后 4 条 `status`，末尾 `final`（`text` 为空），退出码 1；stderr 首行 `dsh: MISSING_CREDENTIAL: ...`。
- 未知 `--session-id`：返回 `{"type":"error","message":"session ... does not exist; omit --session-id to start a new Session"}`，退出码 1，**不会静默新建**。
- 取消：帮助面无取消旗标，源码只提供进程生命周期与 Session 写锁，取消只能由外部结束进程实现。

### 3.3 B 通道：sdk（script）

- `initialize` -> `{serverInfo:{name:"deepseek-harness-sdk-runtime",version:"0.0.1"}}`。
- `session/prompt` -> 入队回执 `{messageId:"64c261fd-..."}`，**不直接返回最终结果**。
- 事件通知：`session.event`、`session.status`；状态序列为 `running` -> `idle`；`session.event` 中出现 `turn/end`（无凭证时 reason 为 `MISSING_CREDENTIAL`）。
- `session/cancel` -> `-32603 unknown DeepSeek Harness SDK runtime method: session/cancel`（协议无该方法）。
- `session/wait` -> `{}`，只等待静默，不返回最终文本（最终文本需客户端自行从 `session.event` 拼装）。
- 恢复：协议没有 `list` / `resume`；未知 `sessionId` 在 `prompt` 时会惰性创建，**无法区分恢复与新建**。

### 3.4 C 通道：acp（script）

- `initialize` -> `agentInfo.name="deepseek-harness-acp"`，`sessionCapabilities={close:{},list:{},resume:{}}`，`promptCapabilities` 全 false，`authMethods:[]`。
- `session/new` -> `{sessionId, configOptions}`（含 model / reasoning_effort 选项）。
- `session/prompt`（无凭证）-> JSON-RPC error `-32603 Internal error: turn failed: ... no API key ...`。注意：**prompt 请求本身与最终结果同源**（不像 SDK 只给回执）。
- `session/cancel`：作为**通知帧**发送（无响应），连接在取消后仍存活；`dsh-acp` 将 `interrupted` 映射为 stop reason `cancelled`。
- `session/close` -> `{}`；关闭后 `session/resume` 仍成功，证明状态可重新接管。
- 未知 id：`session/resume` -> `-32602 session is not resumable: ...`；`session/close` -> `-32602 unknown session: ...`。
- **崩溃恢复**：进程被 `SIGKILL` 后，新 ACP 进程仍能 `session/list` 列出该会话并 `session/resume` 成功。结论：**状态可发现、可重新接管；但进行中的一轮不会自动重放**（被强杀进程没有机会写终止记录）。

### 3.5 D 通道：web（static）

- 帮助面提供 `--host`（具体 IP 字面量，拒绝通配）、`--port`（0 让 OS 选）、`--no-open`、`--public-url`、`--trusted-host`、`--tls-cert`/`--tls-key`。
- 业务能力（会话 `prompt`/`cancel`/`follow`/`list`/`create`/`fork`/`rename`/`select-model`/`resume`/`close`，任务 `list`/`follow`/`kill`）由私有 Remote 协议（`/api` 前缀 + WebSocket mux + HTTP bridge，`BrowserAuth` 鉴权）承载。
- 本审计**未**联机调用该接口，故其运行时行为记为 `unverified`。属**私有、未发布内部接口**，采用须记为耦合风险。

### 3.6 headless 接管语义（script，`headless-recovery.json`）

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 首轮 `--json ping` | 退出码 1，生成 `session-93b7621d-...` | `script` |
| 同 cwd 带 `--session-id` 接管 | adopt 成功，事件流延续原 Session（turn 编号 1 -> 2） | `script` |
| 异 cwd 接管 | 明确拒绝：`session ... was recorded in "...", not "..."` | `script` |
| 未知 id | 明确拒绝，不静默新建 | `script` |
| 并发持有同一 Session 的活进程 | **未实测**，留待第三步 | `unverified` |

### 3.7 隔离副作用核对（script，`isolation-sideeffects.json`）

一次完整通道探测前后，对四个目标做元数据快照比对：`~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI` 的 added / removed / changed **全为 0**；`repo_git_status_changed: false`。

结论：本审计的引擎探测对本机原版 DSH、隔离根与用户数据中枢**无写入副作用**。

---

## 4. 六能力对照矩阵

| 能力 | A headless | B sdk | C acp | D web |
| --- | --- | --- | --- | --- |
| submit_task | 支持（一进程一任务，session id 底层生成） | 支持（`session/prompt` -> `messageId` 回执） | 支持（`session/prompt`，与结果同源） | 支持（`session/prompt`） |
| get_task_status | 弱（进程活/退出） | 支持（`session.status` idle/running） | 支持（agent status + stop reason） | 支持（`session/follow` + control stream） |
| stream_events | 支持（NDJSON，有界，`final` 无损） | 支持（`session.event`） | 支持（语义 `session/update`） | 支持（`follow` 含 `assistantStream`） |
| cancel_task | **不支持**（只能 kill 进程） | **不支持**（实测 -32603） | **支持**（`session/cancel` 通知 + `interrupted`->`cancelled`） | 支持（`session/cancel` + job `kill`） |
| recover_task | 部分（同 cwd `--session-id` adopt；异 cwd/未知 id 明确拒绝） | 部分（`wait` 既有 session，但未知 id 惰性新建，无法区分） | **支持**（`list` + `resume`；强杀后可 resume，但不自动重放） | 支持（`list`/`follow`/`resume`） |
| get_task_result | 支持（终端 `final` 无损） | 弱（需自行从 `session.event` 拼装） | 支持（`PromptResponse` stop reason + updates 拼装） | 支持（follow 终态 + 历史重建） |

证据等级：A/B/C 全部为 `script`（真实进程，无模型推理）；D 为 `static`（源码与帮助面，运行时 `unverified`）。

---

## 5. 尚未证实的推断与边界（unverified）

以下内容**没有**被本次审计证实，不得当作已支持：

1. **web `/api` 的运行时行为**：只核对了帮助面与源码，未联机调用；其鉴权、流式与取消语义均为 `unverified`。
2. **同 Session 并发活锁**：headless 只测了串行接管，未测两个活进程同时持有同一 Session 的行为。
3. **非编码领域下的 personaPrefix 影响**：headless 与 acp 的默认 `personaPrefix` 是“coding agent”。它是否影响材料写作等非编码工作流的输出，需在第二步/第三步用真实模型实测；**本审计不据源码文本推断模型行为**。
4. **token 级流式**：三条通道都不提供逐 token 流；若 UI 需要，需由适配层做节流，不得声称底层 token 级流式。
5. **有凭证时的 turn 语义**：本次全部探测在无凭证下进行，记录的是 `MISSING_CREDENTIAL` 与对应的终止事件；有凭证的成功路径在既有 material-writing / retrieval 报告中已用真实模型跑通，但那些走的是 web 通道，未在 acp/headless 下复验。
6. **跨平台**：只在 Linux 本机验证；Windows/macOS 上 ACP/SDK 的可用性为 `unverified`。
7. **`-32603 unknown ... method` 的稳定性**：取消方法缺失是当前版本的实测结论，未来版本可能变化，需在锁定版本上复测。

---

## 6. 对最小契约的初步映射（非契约定稿）

本节只做**现状到能力的映射**，为第二步的契约定义提供输入，**不构成契约**。

- **任务标识**：ACP 的 `sessionId` 与 headless 的 `session` 事件 `sessionId` 都由底层生成；SDK 的 `sessionId` 由客户端在 `prompt` 时提供。统一标识应由适配层归一化，不直接暴露底层字段名。
- **状态枚举**：ACP 有 running/idle 级状态与 stop reason（`end_turn`/`max_tokens`/`cancelled`）；SDK 只有 idle/running；headless 只有进程活/退出。契约应定义 OmniDesk 自有状态集，并显式标注 headless 无法区分“完成/失败”的细分状态。
- **错误结构**：三条通道风格不一（headless 的 `error` 事件 + 出口码；SDK 的 JSON-RPC error；ACP 的 JSON-RPC error + stop reason）。契约需统一错误码，不让 UI 解析 `-32603` 之类底层码。
- **超时与取消**：只有 ACP 具备真实取消；headless/sdk 的“取消”实为结束进程。契约必须区分 `cancelled` 与 `process_terminated`，避免把 kill 谎报为优雅取消。
- **恢复**：只有 ACP 能区分“恢复既有任务”与“新建”；SDK 与 headless 在此点均有歧义或限制。契约应把恢复建模为**可发现 + 不自动重放**。

---

## 7. 推荐方案及理由

### 7.1 首选 ACP v1 作为引擎适配目标通道

理由：它是三条公开 stdio 通道中**唯一**同时提供取消、恢复、关闭与语义更新的通道，最贴合最小契约。风险：默认人格前缀为“coding agent”，需在第二步用 profile 补丁层覆盖（不修改 runtime），并在第三步用非编码任务实测其影响。

### 7.2 headless 作为无状态兜底

理由：实现最简单（一次进程一次任务，`final` 无损），适合不需要取消/恢复的短任务与冒烟测试。限制：无取消、无跨进程任务状态细化。

### 7.3 web 通道不作为稳定接口

理由：业务能力虽最全，但承载于未发布的私有 Remote 协议与私有前端组件，采用即耦合内部实现，违反“引擎可替换、UI 不依赖 DSH 私有前端”的原则。仅可作为过渡验证环境。

### 7.4 引擎适配边界由 OmniDesk 自有后端承担

理由：把 ACP/SDK/headless 的差异收敛在适配层内部，UI 只面对 OmniDesk 自有契约，不解析 CLI 私有输出格式，不读取 DSH 私有会话文件。若某能力必须读私有会话文件才能补足，应记为耦合风险并评估替代方案，不得把私有格式当稳定接口。

### 7.5 不修改 DSH Runtime 的可行路径

所有差异都可在 OmniDesk 侧解决：通过 `--profile` 选择通道、通过 profile 补丁层覆盖 `personaPrefix` 与 persona 相关配置、通过启动级环境变量隔离数据根。以上均不触碰 runtime 代码。

---

## 8. 实施步骤（第二步、第三步，待确认后执行）

**本步到此为止。** 以下为后续计划概要，需另行确认：

- **第二步（最小接口契约）**：定义 `submit_task` / `get_task_status` / `stream_events` / `cancel_task` / `recover_task` / `get_task_result`，统一任务标识、状态枚举、错误结构、超时与取消语义；先定契约，再验证底层；不把 DSH 内部配置结构暴露给 UI。
- **第三步（最小适配器原型）**：仅用隔离环境与纯合成写作任务，测试提交、输出、错误、取消；测试进程意外中断后的任务状态识别；不支持安全恢复则记录限制、**不自动重放**；留存可复核输入/输出/事件/判定，且不含凭证。
- **第四步（验证与交付）**：每项结论标注证据等级，报告接口能力、真实运行证据与复现步骤、未支持能力与阻塞、文件改动与测试结果，以及对 Tauri 2 + React + TS + Rust 架构的影响。

---

## 9. 验收标准（本步）

- [x] 已读项目宪法、路线图、架构与交接简报。
- [x] 已记录分支、HEAD、工作区改动与既有测试（21 项改动保留，未提交、未推送）。
- [x] 已读隔离启动脚本、模型调用路径、Skill 调用方式与既有验证报告。
- [x] 已实测 DSH CLI 能力与缺失能力，未据文档推断未测行为。
- [x] 已核实探测无副作用（四目标快照差异全 0）。
- [x] 已输出本审计报告，完成后暂停汇报。

---

## 10. 风险清单

| 风险 | 等级 | 说明 | 缓解 |
| --- | --- | --- | --- |
| headless/sdk 无取消 | 高 | 无法优雅取消，只能结束进程 | 优先 ACP；契约区分 cancelled 与 process_terminated |
| SDK 无法区分恢复与新建 | 中 | 未知 id 惰性创建 | 以 ACP 的 list/resume 为准 |
| ACP personaPrefix 为 coding agent | 中 | 可能影响非编码工作流 | profile 补丁层覆盖 + 第三步实测 |
| web 私有接口耦合 | 中 | 未发布内部协议，随版本变动 | 仅作过渡验证，不作稳定接口 |
| 崩溃后不自动重放 | 中 | 进行中的一轮无终止记录 | 契约建模为可发现 + 不自动重放 |
| 版本升级改变协议 | 中 | 方法集合可能变化 | 锁定版本 + 升级后重跑本探测 |

---

## 11. 明确不在本次范围

未修改 DSH Runtime；未开发 GUI；未建立 Core 或插件市场；未读取、打印、复制或修改任何凭证；未接触仓库外个人数据；未接入 234 份正式制度库；未提交、未推送、未打标签；未覆盖现有用户修改。

---

## 12. 关键文件与复现命令索引

探测脚本：

- `tests/verification/engine-interface/probes/_common.py`
- `tests/verification/engine-interface/probes/probe_engine_channels.py`
- `tests/verification/engine-interface/probes/probe_isolation_sideeffects.py`
- `tests/verification/engine-interface/probes/probe_headless_recovery.py`

证据文件：

- `tests/verification/engine-interface/evidence/channel-probe.json`
- `tests/verification/engine-interface/evidence/headless-recovery.json`
- `tests/verification/engine-interface/evidence/isolation-sideeffects.json`
- `tests/verification/engine-interface/evidence/raw/`（A0 帮助/版本、A1/A2 原始 stdout+stderr）

复现命令：

```
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/engine-interface/probes/probe_engine_channels.py
python3 tests/verification/engine-interface/probes/probe_isolation_sideeffects.py
python3 tests/verification/engine-interface/probes/probe_headless_recovery.py
```

相关源码（只读参照，未修改）：

- `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-headless/`
- `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-sdk-protocol/`、`dsh-sdk-jsonrpc-server/`
- `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-acp/`、`dsh-acp-app/`
- `runtime/dsh-runtime/node_modules/@agentclientprotocol/sdk/`
- `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-api-session-controller/`、`dsh-api-job-controller/`
