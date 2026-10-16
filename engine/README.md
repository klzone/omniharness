# OmniDesk 引擎适配层（engine/）

本目录是 OmniDesk 自有应用与底层执行引擎之间的**可替换边界**。它不是 DSH 的一部分，也不修改 DSH。

## 1. 定位

    OmniDesk UI / 领域工作流
              |
              |  （只使用本层对外接口）
              v
    engine/omnidesk_engine/          <-- 本目录
              |
              |  （公开 CLI：node .../dsh/lib/bin.js --profile acp）
              v
    DSH Runtime（隔离数据根，只读依赖，不修改）

上层只看到归一化的 `omni_task_id`、九态状态、事件与标准错误码。引擎侧的 `engine_session_id`、JSON-RPC 请求 ID、轮次 ID 全部封在适配层内部，**不越过本层**，未来 UI 不解析引擎私有格式。

## 2. 内容

| 文件 | 作用 |
| --- | --- |
| `omnidesk_engine/__init__.py` | 对外导出面 + 设计约束说明 |
| `omnidesk_engine/contract.py` | 语义层：状态机、合法转换、冲突仲裁优先级、事件类型、结果形态与完整性、取消五阶段、标准错误码、标识分层 |
| `omnidesk_engine/acp_adapter.py` | ACP v1 适配器：会话/任务生命周期、事件归一化、取消、恢复、关闭 |
| `omnidesk_engine/jsonrpc.py` | 仅标准库实现的 JSON-RPC over stdio 传输层 |

契约正文见 [docs/OmniDesk-Engine-Interface-Contract.md](../docs/OmniDesk-Engine-Interface-Contract.md)，验收矩阵见 [docs/OmniDesk-Engine-Interface-Test-Matrix.md](../docs/OmniDesk-Engine-Interface-Test-Matrix.md)。

## 3. 对外接口

`AcpEngineAdapter` 提供契约 §4.1–§4.9 的最小面：

- `initialize()` → `EngineDescriptor`（能力三态：supported / unsupported / unverified）
- `submit_task(SubmitTaskRequest)` → `SubmitTaskReceipt`
- `get_task_status(omni_task_id)` → `TaskStatus`
- `stream_events(omni_task_id, ...)` → `EngineEvent` 列表
- `get_task_result(omni_task_id, *, require_persisted=False)` → `TaskResult`
- `cancel_task(omni_task_id, *, timeout=...)` → `CancelOutcome`
- `recover_session(engine_session_id, *, cwd=...)` → `SessionRecovery`
- `recover_task(omni_task_id, ...)` → `TaskRecovery`
- `shutdown(mode=..., grace_period=...)` → `ShutdownReport`

能力不足时返回明确的标准错误码（例如 `unsupported`、`engine_unavailable`、`unknown_session`），**不做语义不等价的静默降级**。

## 4. 约束

- 只依赖 Python 标准库，**零第三方依赖**。
- 不修改 DSH Runtime；只通过公开 CLI 入口调用。
- 不读取、不打印、不复制任何凭证内容。
- 不把引擎内部消息结构暴露给未来 UI。
- 未取得运行证据的能力标为 `unsupported` / `unverified`，不以配置文本推断运行行为。

## 5. 运行与测试

适配层本身是库，没有独立入口。驱动测试在：

    cd <repo>
    python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py            # E01–E20 全部
    python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py E05 E18    # 指定场景

驱动使用**一次性临时数据根**（`/tmp/omni_p1s3_*`）与本机回环合成模型端点（零凭证、零外网），绝不写入 `~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI`。

## 6. 证据位置

- 逐场景证据：`tests/verification/engine-interface/evidence/adapter/E01.json … E20.json`（含真实输入、原始帧/事件、标准化结果、判定与证据路径）
- 交付报告：[tests/reports/p1-engine-adapter-prototype-report.md](../tests/reports/p1-engine-adapter-prototype-report.md)
- 隔离副作用探针：`tests/verification/engine-interface/probes/probe_isolation_sideeffects.py`

## 7. 已知语义缺口

- ACP 只在整轮结束一次性投递 assistant 文本，属**消息级**而非 token 级流式；适配层把 `text_delta` 标注为归一化投影，不声称 token 级。
- `stopReason=end_turn` 无法区分 `completed` 与 `blocked`；终态判定不单凭进程退出。
- 客户端被 SIGKILL 后，引擎无终止记录，重启只能把在途任务标为 `unknown`；会话仍可 `resume`，但**不自动重放**。
- `recover_task` 的 `get_task_result` 在无输出时返回 `unrecoverable`，不伪造完整结果。

