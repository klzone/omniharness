# OmniDesk P1 第三步交付报告：最小 ACP 适配器原型

- 报告日期：2026-10-16
- 阶段：OmniDesk P1 第三步（最小 ACP 适配器原型 + E01–E20 运行验证 + E18 人格前缀对照）
- 上游依据：
  - [docs/OmniDesk-Engine-Interface-Audit.md](../../docs/OmniDesk-Engine-Interface-Audit.md)（第一步：只读审计）
  - [docs/OmniDesk-Engine-Interface-Contract.md](../../docs/OmniDesk-Engine-Interface-Contract.md)（第二步：接口契约）
  - [docs/OmniDesk-Engine-Interface-Test-Matrix.md](../../docs/OmniDesk-Engine-Interface-Test-Matrix.md)（第二步：测试矩阵 E01–E20）
  - [docs/OmniDesk-Product-and-Architecture-Roadmap.md](../../docs/OmniDesk-Product-and-Architecture-Roadmap.md)（阶段 2 验收口径）

> 本报告严格区分**静态检查**、**脚本/隔离环境实测**、**真实模型输出**三类证据。凡未取得可靠运行的结论一律标注为 `unverified` 或 `unsupported`，不以设计或配置文本推断运行行为。

---

## 1. 本阶段实际交付了什么

### 1.1 引擎适配层（可替换边界）

新增 `engine/omnidesk_engine/` 包，只依赖 Python 标准库（零第三方依赖），实现契约 §4.1–§4.9 定义的最小引擎接口：

| 文件 | 作用 |
| --- | --- |
| `engine/omnidesk_engine/__init__.py` | 对外导出面；明确「引擎内部结构不越过本层」 |
| `engine/omnidesk_engine/contract.py` | 统一状态机、事件类型、结果形态/完整性、标准错误码、任务 ID 生成 |
| `engine/omnidesk_engine/acp_adapter.py` | ACP v1 适配器：会话/任务生命周期、事件归一化、取消、恢复、关闭 |
| `engine/omnidesk_engine/jsonrpc.py` | 仅标准库的 JSON-RPC over stdio 传输层 |

对外接口（`AcpEngineAdapter`）：

- `initialize()` → `EngineDescriptor`（能力三态声明）
- `submit_task(SubmitTaskRequest)` → `SubmitTaskReceipt`
- `get_task_status(omni_task_id)` → `TaskStatus`
- `stream_events(omni_task_id, ...)` → `EngineEvent` 列表
- `get_task_result(omni_task_id, *, require_persisted=False)` → `TaskResult`
- `cancel_task(omni_task_id, *, timeout=...)` → `CancelOutcome`
- `recover_session(engine_session_id, *, cwd=...)` → `SessionRecovery`
- `recover_task(omni_task_id, ...)` → `TaskRecovery`
- `shutdown(mode=..., grace_period=...)` → `ShutdownReport`

上层只看到归一化后的 `omni_task_id`、九态状态、事件与标准错误；`engine_session_id` / 请求 ID / 轮次 ID 均封在适配层内部，不进入未来 UI。

### 1.2 场景驱动与合成端点（可重复执行）

| 文件 | 作用 |
| --- | --- |
| `tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py` | E01–E20 场景驱动（真实 DSH 引擎进程 + 合成模型端点） |
| `tests/verification/engine-interface/adapter/_harness.py` | 一次性数据根、环境构造、补丁层生成、证据落盘 |
| `tests/verification/engine-interface/adapter/_mock_model_server.py` | 本机回环 OpenAI 兼容合成端点（零凭证、零外网、确定性；支持标记探针） |
| `tests/verification/engine-interface/adapter/_in_flight_child.py` | 「客户端被强杀」子进程夹具（E07/E10） |
| `tests/verification/engine-interface/evidence/adapter/E01.json … E20.json` | 每个场景的真实输入、原始帧/事件摘要、标准化结果、判定与证据路径 |

运行方式：

    cd /mnt/data/Code/MyWorkStation
    python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py          # 全部
    python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py E05 E18  # 指定场景

### 1.3 文档更新

- 新增本报告。
- 更新 [docs/OmniDesk-Engine-Interface-Test-Matrix.md](../../docs/OmniDesk-Engine-Interface-Test-Matrix.md)：执行状态列由「全部未执行」改为逐项真实状态。
- 更新 [docs/OmniDesk-Product-and-Architecture-Roadmap.md](../../docs/OmniDesk-Product-and-Architecture-Roadmap.md)：阶段 2 进展由「适配器原型尚未开始」改为已完成并给出证据。
- 新增 `engine/README.md`：适配层的定位、接口与边界说明。

---

## 2. 场景执行总表（E01–E20）

全部场景以 `script` 级证据执行：真实 DSH ACP 引擎进程 + 本机回环合成模型端点，每场景独立一次性 `/tmp` 数据根。

| 编号 | 场景 | 状态 | 证据等级 | 关键判定 |
| --- | --- | --- | --- | --- |
| E01 | 正常提交 | 已执行-通过 | script | 稳定 `omni_task_id`；ID 分层；幂等键不重复提交 |
| E02 | 流式输出 | 已执行-通过 | script | `seq` 严格递增；增量拼接 == 最终结果；**按消息粒度而非 token 级** |
| E03 | 正常完成 | 已执行-通过 | script | `succeeded` + `form=complete`；不由进程退出推断成功 |
| E04 | 执行失败 | 已执行-通过 | script | 落定 `failed`；无假成功；错误码标准化 |
| E05 | 用户取消 | 已执行-通过 | script | 取消阶段不伪造 `acknowledged`；终态由引擎确认 |
| E06 | 取消与完成竞争 | 已执行-通过 | script | 终态唯一；无矛盾终态；终态后不回退 |
| E07 | 客户端强制退出 | 已执行-通过 | script | 重启后 `recover_task` 得 `unknown`；`replayed=false`；无自动重放 |
| E08 | 会话恢复 | 已执行-通过 | script | 同一 session 恢复、不隐式新建；恢复不重放任务 |
| E09 | 未知会话 | 已执行-通过 | script | 映射标准 `unknown_session`；不新建会话、不发 prompt |
| E10 | 中断任务 | 已执行-通过 | script | 标记不确定；无自动重放；未知时不给假完整结果 |
| E11 | 事件断流 | 已执行-通过 | script | 无重复 `seq`；部分输出保留；断连后不重复拼接 |
| E12 | 重复事件 | 已执行-通过 | script | 重复帧被去重；不重复拼接；任务不重跑 |
| E13 | 最终结果未落盘 | 已执行-通过 | script | 落盘失败被记录；不重跑伪造落盘；`require_persisted` 抛标准码 |
| E14 | 优雅关闭 | 已执行-通过 | script | 在途状态如实上报；不误报为已取消 |
| E15 | 关闭超时 | 已执行-通过 | script | 强制终止被记录；不谎报优雅关闭；不报成功 |
| E16 | 引擎不可用 | 已执行-通过 | script | 映射 `engine_unavailable`；不产生幻影任务；无静默降级 |
| E17 | 不支持的能力 | 已执行-通过 | script | headless 无取消旗标且初始化失败非静默；SDK `cancel`/`resume` 实测 `-32603 unknown method` |
| E18 | 人格前缀对照 | 已执行-通过（script 臂）/ 阻塞（model 臂） | script + **unverified(model)** | 补丁层确实覆盖并进入模型请求；真实模型对照因凭证不可用阻塞 |
| E19 | 结果完整性 | 已执行-通过 | script | 部分输出不冒充完整；截断标 `partial`；不可恢复标 `unrecoverable` |
| E20 | 安全隔离 | 已执行-通过 | script | 四目标无我方痕迹；缺失隔离根未被创建；runtime 零改动；工作区清单不变 |

汇总：**20/20 场景判定通过**；其中 E18 的 model 臂为**阻塞未验证**（见 §5），其 script 臂通过。

**复现性再跑（当前工作区）**：在上述留证完成后，于同一工作区再完整执行一次
`python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py`，结果同为 **20/20 pass**；
该次运行中 E20 的四目标元数据差异为 `(added, removed, changed) = (0, 0, 0)`、`own_trace_paths` 全为空、
`runtime_diff` 全 0、工作区清单前后均为 27 项且 `delta=[]`。即结论在当前 HEAD + 未提交改动上可复现，而非一次性偶发。

逐场景断言清单（每项均为真）保存在各自证据文件的 `checks` 字段；例如：

- E01：`initialize_ok`、`capabilities_declared`、`submit_ok`、`omni_task_id_stable`、`id_layers_distinct`、`accepted_state_is_early`、`idempotency_same_when_key_repeats`。
- E07：`client_forced_exit`、`recover_succeeds`、`status_is_interrupted_or_unknown`、`never_reported_succeeded`、`replay_flag_false`、`no_auto_replay_prompt`。
- E20：`no_own_trace__dsh_home`、`no_own_trace__agents_home`、`no_own_trace__omnidesk_root`、`no_own_trace__yvanai_root`、`no_missing_root_created`、`repo_working_tree_status_unchanged`、`runtime_not_modified_by_rerun`。

---

## 3. 已运行 / 未运行 / 不适用

- **已运行（script 级）**：E01–E20 全部。
- **已运行（model 级）**：无。E18 的 model 臂因凭证不可用阻塞（详见 §5）。
- **未运行**：
  - Windows / macOS 上的通道与适配层行为（仅在本机 Linux 验证）。
  - 同 Session 并发活锁（headless 两活进程持同一 Session）——留待后续，非本阶段契约必需。
  - 「真实引擎在整轮中途失败并保留已交付文本」：实测 ACP 通道只在整轮结束一次性投递 assistant 文本，无法自然构造该时序；E19 臂 C 以**桩级（stub-level）**触发适配层整形分支，并在报告中明示其证据性质。
- **不适用**：
  - Web 私有 Remote 协议（契约已排除，不作产品 API）。
  - Tauri / React / Rust 桌面边界（属阶段 3，不在本阶段范围）。

---

## 4. ACP 取消、恢复、关闭的真实行为（本轮实测）

**取消（cancel_task）**

- `session/cancel` 是**通知帧**（无响应体），适配层按五阶段语义跟踪：`requested → notification_sent → engine_observed → confirmed/…`。
- `acknowledged` 阶段在 ACP 上**不可达**（没有可用的引擎级回执）；适配层**不得伪造**该阶段，`phase_never_fake_acknowledged` 在 E05 中为真。
- 终态取消以引擎的终态证据为准（`interrupted → cancelled`），不把「发出通知」当作「已取消」。
- headless 通道**没有取消能力**（帮助面无取消旗标，且初始化本身失败非静默）；SDK 通道 `session/cancel` 实测返回 `-32603 unknown DeepSeek Harness SDK runtime method: session/cancel`。适配层在这两条通道上**如实失败**，不做语义不等价的降级。

**恢复（recover_session / recover_task）**

- `session/resume` 可恢复**既有**会话，返回同一 `engine_session_id`；适配层不隐式新建会话（E08 中 `session/new`/`session/prompt` 计数为 0）。
- 客户端被 SIGKILL 强杀后，新进程重新加载同一 `state_dir` 时，适配层依据本地**非终态**记录得出 `unknown`（`evidence_source=process_state`），`replayed=false`，**不自动重放**中断轮次（E07/E10）。
- 未知会话返回标准错误 `unknown_session`（底层 `Invalid params` 映射），不新建、不发送 prompt（E09）。
- SDK 通道 `session/resume` 实测同为 `-32603 unknown method`，不具备 ACP 语义的恢复能力。

**关闭（shutdown）**

- 优雅关闭：在途任务状态**如实上报**，不误报为已取消（E14）。
- 超时强制终止：记录为强制终止，**不谎报**优雅关闭，也不报成功（E15）。
- `session/close` 返回 `{}`；关闭后仍可 `session/resume`（本阶段实测）。

**仍然存在、无法可靠取消或恢复的情形**

1. 非 ACP 通道（headless / SDK）没有语义等价的取消与恢复；适配层选择如实 `unsupported`，不提供降级替代。
2. 客户端进程被强杀时，在途轮次**无法可靠恢复为成功**：适配层只能给出 `unknown`，且不自动重放。这是刻意选择（宁可标记不确定，也不伪造结果）。
3. ACP 的 `end_turn` **无法区分 `completed` 与 `blocked`**（引擎 `turnEndToStopReason` 的已知语义缺口）。适配层不据此推断成功，需结合结果与事件判断；该缺口已如实记录。

---

## 5. E18 人格前缀对照：script 臂通过，model 臂阻塞

**script 臂（通过，证据等级 script）**

- 离线 `--dump-config` 实测：默认 `acp` Profile 的 `personaPrefix` 为 `You are a coding agent powered by the {{model}} model.`；经补丁层覆盖后变为覆盖值（不含 coding agent 字样）。
- 合成端点标记探针实测：覆盖臂的模型请求文本**命中覆盖标记**、**不再命中基线前缀**；对照臂（不覆盖）请求文本**命中基线前缀**、不命中覆盖标记。
- 结论：补丁层覆盖不是「只写在文件里」，而是**真正进入模型请求**；且覆盖仅作用于本阶段的隔离/一次性实例，未改动原版 `~/.dsh`、`~/.agents` 或 DSH Runtime。

**model 臂（阻塞，标注 unverified）**

- 阻塞原因：脚本运行环境的进程环境变量中**没有任何 `*_API_KEY`**；隔离根的凭证库存在，但按硬约束**不读取、不复制、不修改**，也不擅自切换模型路由。因此真实模型对照未能执行。
- **未把静态检查当作模型行为验证**；路线图与契约中「人格前缀是否影响非编码任务输出」的结论在此之前仍属 `unverified`。
- 复现步骤（具备凭证后）：

      # 仅通过进程环境注入，不写入仓库、不复制凭证文件
      export OPENDESIGN_API_KEY=...        # 或隔离实例实际声明的 provider key
      # 以相同合成输入分别运行两次：默认 personaPrefix 与覆盖后的 personaPrefix
      # 对照两次原始模型输出、实际生效配置来源，并进行人工判定
      # 结束时核实隔离配置与原版环境边界未被破坏

  对照应使用 P-A/P-B/P-C 三类合成任务（虚构工作总结 / 合成制度问答 / 事实冲突待确认），条件为：同一模型、同一输入、同一知识资产，配置差异仅 `personaPrefix` 一项。

---

## 6. 与契约的逐条对应

| 契约章节 | 覆盖场景 | 本轮结论 |
| --- | --- | --- |
| 4.1 能力与初始化 | E01, E16, E17 | 能力三态声明可用；不可用通道如实失败 |
| 4.2 submit_task | E01, E16 | 受理状态早于终态；幂等键有效；失败不留幻影任务 |
| 4.3 get_task_status | E03, E04, E07, E10, E13 | 九态与转换表按合同生效 |
| 4.4 stream_events | E02, E11, E12 | `seq` 单调；去重；断流保留部分输出；**非 token 级** |
| 4.5 get_task_result | E03, E04, E19 | 完整性标记正确；不冒充完整 |
| 4.6 cancel_task | E05, E06, E17 | 五阶段语义成立；不伪造 `acknowledged` |
| 4.7 recover_session/recover_task | E07, E08, E09, E10 | 恢复不重放；未知会话标准错误 |
| 4.8 关闭与资源释放 | E14, E15 | 在途/超时如实上报 |
| 4.9 错误模型 | E04, E09, E13, E16, E17 | 标准错误码 + 五属性 |
| 第 6 节 人格前缀 | E18 | script 臂通过；model 臂阻塞 |
| 第 8 节 持久化与安全边界 | E13, E20 | 落盘失败可见；四目标零副作用 |

---

## 7. 失败案例、已知限制与后续风险

1. **流式粒度**：ACP 只在整轮结束时投递 assistant 文本，`text_delta` 为**消息级**而非 token 级。UI 不得声称 token 级流式；进度反馈需依赖 `progress`/`usage_update` 等事件而非文本增量。
2. **`end_turn` 语义缺口**：无法从 `stopReason` 区分 `completed` 与 `blocked`。
3. **取消的 `acknowledged` 不可达**：UI 不应展示「引擎已确认取消」这类中间态。
4. **强杀场景不可恢复**：客户端被强杀后，在途任务只能判为 `unknown`；这是刻意不伪造结果。
5. **E12 的真实性边界**：DSH 在交付前会聚合连续相同增量，重复帧通常到不了适配层；E12 证明的是**适配层去重分支正确**，而非底层一定产生重复帧。
6. **E19 臂 C 为桩级**：仅覆盖适配层结果整形分支；真实失败路径见臂 A（流内错误丢弃整轮输出 → `unrecoverable`）。
7. **平台覆盖**：仅在 Linux 本机验证；Windows/macOS 为 `unverified`。

---

## 8. 本次新增 / 修改的文件

**新增**

- `engine/omnidesk_engine/{__init__,contract,acp_adapter,jsonrpc}.py`
- `engine/README.md`
- `tests/verification/engine-interface/adapter/{_harness,_mock_model_server,_in_flight_child,run_engine_adapter_scenarios}.py`
- `tests/verification/engine-interface/evidence/adapter/E01.json … E20.json`
- `tests/reports/p1-engine-adapter-prototype-report.md`（本文件）

**修改**

- `docs/OmniDesk-Engine-Interface-Test-Matrix.md`（执行状态列）
- `docs/OmniDesk-Product-and-Architecture-Roadmap.md`（阶段 2 进展状态）
- `docs/ARCHITECTURE.md`（在五层架构说明处补入引擎适配层的定位与边界）
- `docs/OmniDesk-Next-Phase-Brief.md`（P1 第二/第三步状态同步）
- `docs/OmniDesk-Engine-Interface-Contract.md`（状态收口为「已冻结/已实现/已验证」，语义未变）
- `tests/reports/p1-engine-interface-contract-report.md`（补注：文中「仅设计/未执行」为第二步成文当时状态）

**未修改**：`runtime/deepseek-harness/`、`runtime/dsh-runtime/` 下任何文件；`~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI` 无我方写入；未读取/复制/修改任何凭证；未 `git add`、未提交、未推送、未打标签。

---

## 9. 证据等级一览

| 结论 | 证据等级 | 位置 |
| --- | --- | --- |
| 适配层接口实现与状态机 | static | `engine/omnidesk_engine/`、契约文档 |
| E01–E17、E19、E20 场景行为 | script（真实引擎进程 + 合成端点） | `evidence/adapter/E*.json` |
| E18 补丁层覆盖并进入模型请求 | script（标记探针） | `evidence/adapter/E18.json` |
| E18 真实模型对照 | **unverified（阻塞）** | `evidence/adapter/E18.json` 的 model 臂字段 |
| 隔离零副作用 | script（元数据快照比对 + 空闲对照） | `evidence/adapter/E20.json` |
| 非 ACP 通道不支持能力 | script（raw JSON-RPC 实测） | `evidence/adapter/E17.json` |

---

## 10. 结论与阶段出口条件

- **接口可用性**：契约定义的 6 项最小能力（submit / status / stream / cancel / recover / result）与关闭语义，均在 ACP 通道上取得 `script` 级运行证据；适配层对不支持的能力如实返回 `unsupported`，无伪造。
- **隔离完好**：E01–E19 复跑前后，`~/.agents`、`~/OmniDesk`、`~/YvanAI` 元数据零差异；`~/.dsh` 的差异路径**不含本阶段痕迹特征**（归因于本机正在运行的原版 DSH Desktop 自身写入——空闲对照窗口实测零漂移，而复跑窗口出现 `/mnt/data/AI资料` 会话写入，与本阶段 `/tmp` 数据根无关）；缺失的隔离根未被创建；`runtime/` 零改动；工作区清单不变。
- **未达标项**：E18 model 臂因凭证不可用阻塞，**不影响** script 臂结论，但「人格前缀对非编码任务输出的实际影响」仍为 `unverified`。
- **是否满足阶段 2 出口条件**：引擎边界侧的验收要点（任务提交、事件/结果、错误、取消、恢复、隔离、版本锁定）已由真实运行证据支撑；**桌面调用边界（React/TS → Tauri/Rust）不在本阶段范围**，尚未开始。因此阶段 2 尚**未整体出口**，但「引擎接口技术切片」这一子项已完成并可复核。

