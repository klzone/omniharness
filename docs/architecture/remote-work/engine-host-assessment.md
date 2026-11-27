# Agent Engine 独立运行能力评估（Step 2）

- 日期：2026-11-27
- 问题：Engine 能否脱离 Desktop UI 运行、长期驻留、独立启动、异常恢复，并在 Mac mini 常驻。

## 1. 现状事实

| 问题 | 现状 | 证据等级 |
| --- | --- | --- |
| Engine 是否有独立入口 | 有进程形态（python3 -m omnidesk_engine.serve），但协议是被 Rust 宿主启动的子进程，stdio JSON-RPC，无 TCP 监听 | static（serve.py 文件头） |
| 是否允许常驻服务 | 当前契约明确禁止：本进程只被 OmniDesk 宿主启动；不注册 systemd，不做常驻服务 | static |
| 能否脱离 UI 工作 | 逻辑上能：Engine 与 DSH 完成任务执行、台账、事件、结果落盘，不依赖 UI。但没有由非宿主进程拉起 Engine 的入口 | static（task_core 无 UI 依赖） |
| 异常恢复 | 宿主可重启 Engine；中断任务标 interrupted/unknown，不自动重放 | static（IPC 契约 3.8） |
| DSH 侧生命周期 | Engine 按需启动 dsh --profile web --port 0（回环、随机端口、token 换 cookie），随 Engine 退出回收 | static + script（web_api_adapter 探针） |
| 进程所有权 | process_owner.py 有 claim_ownership / reclaim_owner / 孤儿回收，防多实例与孤儿 node 进程 | static |
| 窗口关闭行为 | 关闭设置服务 + 清扫孤儿 DSH node（Windows taskkill /T /F） | static（D-111） |

## 2. 关键差距（对照 Mac mini 常驻目标）

1. 启动权在 Tauri 宿主。现在 Engine 生命周期 = 桌面应用生命周期。Mac mini 常驻必须提供无 UI 的启动者。
2. stdio IPC 假设对端是本地宿主。远程客户端无法直接连 stdio；需要一个本机桥接层或让 Engine 增加 TCP/HTTP 面。
3. 无开机自启、无崩溃自动重启、无健康检查。process_owner 解决了同机互斥，没有解决守护。
4. DSH 会话数据在 ~/OmniDesk/runtime/dsh-home/，路径由环境变量下发；常驻化不需要迁移数据，只需要固定同一数据根。
5. 模型凭证由 DSH 自身管理（.credentials.yaml），Engine 不触碰；常驻化对凭证无新增要求。

## 3. 可选路径比较

| 方案 | 做法 | 复用度 | 复杂度 | 结论 |
| --- | --- | --- | --- | --- |
| A. launchd 托管一个 headless 宿主 | 新增无 UI 的 Rust 宿主入口（复用 engine_supervisor/engine_ipc 逻辑），由 launchd KeepAlive 拉起，宿主再启动 Engine | 高：宿主代码大部分复用 | 中：需拆出宿主可执行目标 | 推荐。保持 IPC 契约不变 |
| B. launchd 直接托管 Python Engine | Engine 增加 TCP/HTTP 监听，绕过 Rust | 中 | 中低 | 不推荐：绕开宿主后丢失路径授权、事件桥接与统一命令面，且需改契约语义 |
| C. DSH web profile 直接常驻 | 用户直接用 DSH web UI | 低 | 低 | 不推荐：放弃 OmniDesk 产品层与任务台账，只可作过渡验证 |

## 4. 结论

- Engine 执行逻辑可以在无 UI 的常驻进程内复用（已实现，static）。
- 需要新增的只有三件事：一个无 UI 宿主入口（或把 Tauri 宿主拆出核心库）、launchd 托管配置、健康检查与自动重启。
- 关闭 Desktop UI 是否影响任务：当前会（宿主退出则 Engine 回收）；改造后不会（Engine/宿主由 launchd 托管）。
- 是否需要系统级服务：需要，即 launchd KeepAlive；这是 Phase 1 的一部分，属于必要而非可选。

