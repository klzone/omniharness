# OmniDesk 桌面引擎进程通信契约（Rust 宿主 ↔ Python Engine）

> 状态：**契约草案（设计）**（OmniDesk P1-Desktop Phase 2 · Step 3 + Step 4 产物）
> 日期：2026-10-16
> 上游依据：[OmniDesk-Engine-Interface-Contract.md](OmniDesk-Engine-Interface-Contract.md)（语义层）、[OmniDesk-Desktop-Architecture.md](OmniDesk-Desktop-Architecture.md)（分层与职责）
> 说明：本文只定义**消息语义与生命周期**，不含实现。当前 desktop/ 尚未创建，Python Engine 也无子进程入口（见 Step 1 报告）。文中一切"支持"均需在 Step 8/12 用运行证据确认。

标注约定：`[设计]` = 本契约规定；`[现状]` = 已核实的仓库事实；`[待验证]` = 需在 Step 8+ 用运行证据确认；`[限制]` = 底层能力缺口。

---

## 0. 范围与两条边界

本项目有**两条**进程边界，本文只定义**第二条**：

| 边界 | 名称 | 定义于 | 本文范围 |
| --- | --- | --- | --- |
| IPC-1 | WebView(React/TS) ↔ Rust(Tauri) | Tauri command / event 机制 | 否（Step 10 使用） |
| **IPC-2** | **Rust(Tauri) ↔ Python Engine** | **本文** | **是** |

**硬性约束**：UI **不存在**直达 DSH ACP 的接口。React 只能调用 IPC-1；Rust 只能调用 IPC-2；Python Engine 才能调用 DSH（经 engine/omnidesk_engine/）。任何"UI 直连 ACP/DSH"的实现都违反本契约。

---

## 1. Python Engine 启动方式（Step 4 决策）

`[设计]` **Python Engine 固定为由 OmniDesk Rust 宿主启动的独立子进程。**

### 1.1 生命周期归属

| 项 | 规定 |
| --- | --- |
| 进程所有者 | OmniDesk Rust 宿主（Tauri AppHandle），随应用启动而启动、随应用退出而回收 |
| 常驻性 | **不作为系统级常驻服务**；不注册 systemd/launchd/自启；不监听固定端口 |
| 实例数 | 每个 OmniDesk 实例持有自己的 Engine 子进程（默认 1:1） |
| 崩溃处理 | 宿主检测到 Engine 异常退出后，可自动重启 Engine 子进程；但**不得自动重放**中断的任务（遵从引擎契约 4.7） |
| DSH 关系 | Engine 内部自行按需启动 DSH ACP 子进程；宿主不直接管理 DSH 进程 |

### 1.2 启动形式

`[设计]` 宿主以子进程方式启动 Engine，**首批候选**（Step 8 二选一，须实测后定）：

**候选 A：stdio JSON-RPC（首选）** —— Engine 作为可执行进程（如 python3 -m omnidesk_engine.serve 或独立脚本），用 stdin/stdout 交换 JSON-RPC 2.0 行分隔消息，stderr 留给日志。适配层已有同类实现可复用思路（engine/omnidesk_engine/jsonrpc.py）。

**候选 B：本机 Unix domain socket** —— 宿主创建 socket 路径（位于 OmniDesk 应用数据目录），Engine 连接后按同一消息格式通信。

`[设计]` 无论 A/B，**均不得监听公网或 0.0.0.0**；若使用 TCP 只允许 127.0.0.1 且端口由宿主分配（默认 0）。

`[现状]` Engine 目前**无子进程入口**（无 __main__.py、无 serve 模块）。Step 8 需新增该入口；新增入口只是把现有 AcpEngineAdapter 包成服务，**不改引擎语义、不改 DSH**。

### 1.3 启动参数（`[设计]`）

宿主启动 Engine 时显式下发（不用默认值猜测）：

| 参数 | 含义 |
| --- | --- |
| 应用数据根 | OmniDesk 桌面应用数据目录（Engine 状态、日志落点） |
| DSH 隔离根 | DSH_HOME / DSH_AGENTS_HOME 指向 ~/OmniDesk/runtime/* |
| DSH CLI 入口 | runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js（绝对路径，锁定版本） |
| Profile 名 | acp（引擎通道 Profile） |
| 允许补丁层 | 引擎侧 Profile 补丁路径列表（可选） |
| 工作目录 | Engine 的工作目录（用于 DSH ACP 会话 cwd） |

`[设计]` 凭证**不通过 IPC 传输，不写入参数**。Engine 侧从既有隔离凭证机制读取（宿主不读、不打印、不复制）。

---

## 2. 传输层约定（`[设计]`）

- 编码：UTF-8 文本，消息为**行分隔 JSON**（每条消息一行，以换行符结束）。
- 协议：JSON-RPC 2.0 语义子集（jsonrpc / id / method / params / result / error）。
- 帧方向：双向对等（宿主与 Engine 都可发 request/notification）。
- 大载荷：超过实现阈值时用**引用传递**（input_ref / 结果落盘路径），不塞进单条消息；具体阈值在 Step 8 定。
- 背压：Engine 在宿主未确认时不得无限缓冲；无法投递的事件须可报告（见第 3.3 节事件可靠性）。
- 版本握手：第一条消息必须是第 3.1 节的 omni.initialize，宿主据此校验 protocol_version，不匹配则拒绝启动（protocol_incompatible）。

---

## 3. 七类消息（Step 3 验收要求）

Step 3 要求至少定义**启动、任务提交、事件接收、取消、结果获取、错误返回、Engine 关闭**七类消息。逐条如下。

### 3.1 启动 / 握手：omni.initialize

**方向**：宿主 → Engine（request）

    {
      "method": "omni.initialize",
      "params": {
        "protocol_version": "omnidesk-engine/1",
        "app_data_root": "/home/<user>/OmniDesk/app",
        "engine_state_dir": "/home/<user>/OmniDesk/app/engine-state",
        "log_dir": "/home/<user>/OmniDesk/app/logs",
        "dsh": {
          "dsh_home": "/home/<user>/OmniDesk/runtime/dsh-home",
          "agents_home": "/home/<user>/OmniDesk/runtime/agents-home",
          "cli_entry": "/mnt/data/Code/MyWorkStation/runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js",
          "profile": "acp",
          "patch_files": []
        },
        "cwd": "/mnt/data/Code/MyWorkStation"
      }
    }

**返回**（Engine → 宿主）：

    {
      "result": {
        "protocol_version": "omnidesk-engine/1",
        "engine_kind": "dsh-acp",
        "engine_version": "0.2.1-alpha.2",
        "compatibility": "compatible",
        "healthy": true,
        "capabilities": {
          "submit_task": "implementation_supported",
          "get_task_status": "implementation_supported",
          "stream_events": "implementation_supported",
          "cancel_task": "implementation_supported",
          "recover_session": "implementation_supported",
          "recover_task": "partial",
          "get_task_result": "implementation_supported"
        },
        "detail": { }
      }
    }

`[设计]` 返回结构即引擎契约 §4.1 的 EngineDescriptor。capabilities 取值用契约 §4.1 的三态/五态枚举，**不得**把"支持 ACP"推断为全部能力可用。

### 3.2 任务提交：omni.submit_task

**方向**：宿主 → Engine（request）

    {
      "method": "omni.submit_task",
      "params": {
        "omni_task_id": "omni-<uuid>",
        "idempotency_key": "<client-generated>",
        "intent": "material_writing",
        "workspace_dir": "/abs/path",
        "payload": { "prompt_text": "..." },
        "input_ref": null,
        "timeout_policy": { "total_seconds": 120 }
      }
    }

`[设计]` omni_task_id 由 **OmniDesk 生成**（宿主或 Engine 皆可，但必须稳定且可持久化）；同一 idempotency_key 重复提交不得导致同一任务重复执行。返回引擎契约 §4.2 的 SubmitTaskReceipt：

    { "result": { "omni_task_id": "omni-...", "status": "queued", "accepted_at": 0.0,
                  "engine_session_id": null, "note": "" } }

> engine_session_id 属适配层内部关联字段；`[设计]` **UI 不可见**（IPC-1 不向 React 暴露该字段）。本文出现它只为说明 IPC-2 的完整帧。

### 3.3 事件接收：omni.event

**方向**：Engine → 宿主（notification）

    {
      "method": "omni.event",
      "params": {
        "omni_task_id": "omni-...",
        "seq": 7,
        "occurred_at": 0.0,
        "type": "text_delta",
        "payload": { "text": "..." }
      }
    }

`[设计]` 事件结构即引擎契约 §4.4 的 EngineEvent。事件类型枚举（task_started / text_delta / progress / tool_activity / task_completed / task_failed / cancel_requested / task_cancelled / task_interrupted / protocol_error）以 engine/omnidesk_engine/contract.py 为准。

`[设计]` 可靠性规则：

1. seq 单调递增；宿主按 seq 去重与重排。
2. text_delta 是**消息级归一化投影**，不代表 token 级流式（`[限制]` 来自 ACP 语义）。
3. 宿主要能在**断流**时识别缺口（`[待验证]` Step 8/12 需实测缺口可检测性）。
4. 无底层证据**不得合成**成功事件。

### 3.4 取消：omni.cancel_task

**方向**：宿主 → Engine（request）

    { "method": "omni.cancel_task", "params": { "omni_task_id": "omni-...", "timeout_seconds": 15 } }

返回引擎契约 §4.6 的 CancelOutcome，phase ∈ not_requested / request_sent / acknowledged / confirmed_cancelled / timeout_unknown。

`[设计]` **"请求已发送"不得呈现为"取消成功"。** 只有 confirmed_cancelled 才是取消成功的可靠证据。`[限制]` 当前 ACP 通道下 acknowledged 阶段不可达（底层无 ack 帧）。

### 3.5 结果获取：omni.get_task_result

**方向**：宿主 → Engine（request）

    { "method": "omni.get_task_result", "params": { "omni_task_id": "omni-...", "require_persisted": false } }

返回引擎契约 §4.5 的 TaskResult：form ∈ complete / partial / not_ready / unrecoverable；integrity ∈ complete / partial；含 sources[]、persisted_at。

`[设计]` partial 不得冒充 complete。`[设计]` 大文本结果以**落盘路径 + 元数据**返回，正文按需读取（避免单帧过大）。

配套查询：omni.get_task_status（引擎契约 §4.3 的 TaskStatus）与 omni.stream_events（拉取式补漏，返回 EngineEvent 列表）。`[设计]` 推（omni.event）与拉（omni.stream_events）结合，宿主重连后可补齐缺失事件。

### 3.6 错误返回（统一错误结构）

`[设计]` 任何 request 失败都必须返回标准 JSON-RPC error，且 data 携带引擎契约 §4.9 的错误码与五属性：

    {
      "error": {
        "code": -32000,
        "message": "engine_unavailable",
        "data": {
          "engine_error_code": "engine_unavailable",
          "retryable": "conditional",
          "requires_human": false,
          "degradable": false,
          "may_duplicate_execution": false,
          "may_lose_output": true,
          "omni_task_id": "omni-...",
          "detail": "..."
        }
      }
    }

`[设计]` 错误码集合：invalid_argument / engine_unavailable / protocol_incompatible / unknown_session / unknown_task / unsupported / task_failed / cancel_failed / recover_failed / timeout / event_parse_error / result_persist_failed / state_uncertain。

`[设计]` **重试策略不得默认对写作任务自动重放。**

### 3.7 Engine 关闭：omni.shutdown

**方向**：宿主 → Engine（request）

    { "method": "omni.shutdown", "params": { "mode": "graceful", "grace_period_seconds": 10 } }

返回引擎契约 §4.8 的 ShutdownReport：{ mode, in_flight_before, cancelled, still_running, connection_closed, process_exit_code, forced_terminate, note }。

`[设计]` 区分三类操作：关闭连接、停止本地进程、取消远端任务。

### 3.8 恢复（补充，用于 Step 8/12）

虽不属 Step 3 列举的七类，但契约 §4.7 要求，故在此登记：

| 方法 | 方向 | 返回 |
| --- | --- | --- |
| omni.recover_session | 宿主 → Engine | 引擎契约 §4.7 SessionRecovery |
| omni.recover_task | 宿主 → Engine | 引擎契约 §4.7 TaskRecovery（含 replayed，**恒为 false 以外须有证据**） |

`[设计]` 恢复失败不得静默新建会话；不允许自动重放状态不明的任务。

---

## 4. 语义统一要求

| 维度 | 规定 |
| --- | --- |
| 任务标识 | 全链路只认 omni_task_id；engine_session_id 不出 IPC-2 边界 |
| 状态枚举 | 只用引擎契约 §3.1 的九态；宿主不得自造状态 |
| 错误结构 | 只用第 3.6 节的统一结构；宿主不得把 Engine 原始异常文本当作 UI 文案 |
| 超时 | 每个 request 有显式超时；超时映射为 timeout 错误，不得当作成功 |
| 取消语义 | 五阶段严格区分；request_sent ≠ confirmed_cancelled |
| 恢复语义 | 区分"恢复会话"与"恢复任务"；不自动重放 |
| 终态 | 不得以进程退出推断任务成功（契约 3.4） |

---

## 5. 与 DSH 的关系（不得违反）

1. `[设计]` Engine **不复制、不修改** runtime/deepseek-harness/。
2. `[设计]` Engine 只通过公开 CLI（--profile acp）调用 DSH；不读取 DSH 私有会话文件作为接口。
3. `[设计]` Engine 的数据根固定为隔离根 ~/OmniDesk/runtime/*；绝不写 ~/.dsh、~/.agents。
4. `[设计]` ACP 消息结构（sessionId、stopReason、方法名）不得越过 Engine 适配层；IPC-2 只传归一化字段。

---

## 6. 验收标准（Step 3 / Step 4）

| 要求 | 本轮达成 |
| --- | --- |
| 定义启动消息 | 是 §3.1 |
| 定义任务提交消息 | 是 §3.2 |
| 定义事件接收消息 | 是 §3.3 |
| 定义取消消息 | 是 §3.4 |
| 定义结果获取消息 | 是 §3.5 |
| 定义错误返回消息 | 是 §3.6 |
| 定义 Engine 关闭消息 | 是 §3.7 |
| 不存在 UI 直接调用 DSH ACP 的接口 | 是 §0 硬性约束 + §5 |
| Python Engine 由 Rust 宿主启动 | 是 §1.1 |
| Python Engine 非常驻服务 | 是 §1.1 |
| DSH Runtime 不被复制/修改 | 是 §5 |

---

## 7. 待验证与限制

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 传输实现（stdio / socket） | `[待验证]` | Step 8 实测后二选一 |
| Engine 子进程入口 | `[现状]` 不存在 | Step 8 新增 |
| 断流检测与事件补齐 | `[待验证]` | Step 8/12 实测 |
| acknowledged 取消阶段 | `[限制]` 不可达 | ACP 无 ack 帧 |
| text_delta 是否 token 级 | `[限制]` 否（消息级） | ACP 语义 |
| Windows/macOS 命名管道/socket 差异 | `[待验证]` | 未实测 |

