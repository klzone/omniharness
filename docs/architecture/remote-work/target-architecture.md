# 中央 Agent Host 及多终端目标架构（Step 9）

- 日期：2026-11-27
- 原则：区分当前已有 / 最小改造 / 未来完整目标三个形态，不一次性实施。

## 1. 目标架构图

```
                家庭 Mac mini（中央 Agent Host，launchd 常驻）
┌──────────────────────────────────────────────────────────────┐
│  OmniDesk Host Core（无 UI 宿主，Rust，由 Tauri 宿主核心抽出）   │
│  ├─ Engine Supervisor（复用 engine_supervisor.rs）              │
│  ├─ Engine IPC（复用 engine_ipc.rs，stdio JSON-RPC）            │
│  ├─ Task Service（复用 task_store.rs / scheduler.rs / events）  │
│  ├─ Project / Knowledge / File Service（复用既有 *_store.rs）   │
│  ├─ HTTP API Server（新增：认证 + REST/SSE，回环绑定）           │
│  └─ Event Bridge（events.jsonl + Engine 事件 → SSE）            │
│           │ stdio JSON-RPC（IPC-2 不变）                        │
│  Python Engine（engine/omnidesk_engine/ 不变）                  │
│           │ node bin.js --profile web                           │
│  DSH Runtime（不变，数据根 ~/OmniDesk/runtime/dsh-home）        │
│  数据：~/OmniDesk/{app,work,workspace,库,reading,runtime}       │
└──────────────────────────────────────────────────────────────┘
            │ Tailscale 私网（WireGuard）
     ┌──────┼──────────┬─────────────┐
办公室笔记本      家中设备      移动设备（手机/平板）
┌──────────┐  ┌──────────┐  ┌──────────┐
│ Web UI    │  │ Web UI    │  │ Web UI    │
│ 或 Desktop│  │（主用）   │  │（移动版） │
│ 本地缓存  │  │ 本地缓存  │  │ 只读缓存  │
└──────────┘  └──────────┘  └──────────┘
```

## 2. 模块职责与运行位置

| 模块 | 运行位置 | 职责 | 现有代码复用 |
| --- | --- | --- | --- |
| Central Host（launchd 服务） | Mac mini | 开机自启、守护 Host Core | 新增 launchd plist |
| Host Core（无 UI 宿主） | Mac mini | 启动/监督 Engine，承载全部业务服务 | 从 Tauri 宿主拆出核心库（engine_supervisor/engine_ipc/task_store/scheduler 等均无 Tauri 依赖或可轻拆） |
| Engine | Mac mini 子进程 | 任务执行、会话、台账、结果落盘 | engine/omnidesk_engine/ 原样 |
| Session Manager | Mac mini（Engine 内） | 会话 list/resume/fork | 原样复用 |
| Task Service | Mac mini（Host Core 内） | 任务 CRUD、状态迁移、事件表 | task_store.rs / event_log.rs |
| Project / Knowledge / File Service | Mac mini | 各域文件读写与校验 | project_store / knowledge_vault / vault_* |
| Model Router | Mac mini（Engine 内经 DSH） | 模型路由与覆盖 | model_routes.rs / dsh-settings |
| API Server | Mac mini | 认证令牌 + REST + SSE，回环绑定 | 新增（Rust axum/tiny_http 或复用 local_http.rs 思路扩展） |
| Web UI | Mac mini 静态 + 各端浏览器 | 远程操作界面 | 复用 desktop/src React 组件，替换 api.ts 的 Tauri 调用为 fetch |
| Desktop UI | 各终端（可选） | 本地 Tauri 外壳，指向中央 API | 现有 desktop/ 改数据源 |
| Event Service | Mac mini | SSE 推送事件流 | events.jsonl + Engine 事件 |
| Storage | Mac mini 本地文件 | 见数据布局（Step 1 §3） | 不变 |

## 3. 通信路径

1. 终端 → API：HTTPS over Tailscale（私网 IP），Bearer 认证令牌。
2. API → Engine：stdio JSON-RPC（IPC-2 契约不变，这是最大复用点）。
3. Engine → DSH：CLI + 本地 web profile（回环，token/cookie 内存态）。
4. 事件流：Engine omni.event → Host Core 事件桥 → SSE 到终端。

## 4. 启动依赖链

launchd → Host Core → Engine → DSH（按需）→ 随 Host Core 退出回收。数据根由环境变量固定（OMNIDESK_APP_DATA_ROOT 等现有变量原样使用）。

## 5. 三形态对照

| 模块 | 当前已有 | 最小改造后（Phase 1） | 未来完整目标 |
| --- | --- | --- | --- |
| Host | Tauri 前台宿主 | + 无 UI 核心 + launchd | + 健康检查/备份/监控 |
| 访问 | 本机 IPC | + HTTP API + 令牌 + Tailscale | + WebSocket 实时流 |
| UI | Tauri Desktop | Desktop + 最小 Web（任务/会话/成果） | 完整 Web + 移动适配 |
| 会话 | 单机恢复 | 中央 Engine 常驻可恢复 | + Handoff 快照 |
| 同步 | 无 | 无（纯在线） | 本地缓存 + 同步队列 + 冲突处理 |

## 6. 复用关系总结

- 原样复用：Engine 全部、DSH、数据布局、任务/项目/知识文件格式、安全护栏（net_guard/host_access/isolated）。
- 抽出复用：Tauri 宿主的非 UI 逻辑（进程管理、存储、调度）。
- 新增：launchd 托管、HTTP API、认证、Web UI 壳、（Phase 4+）缓存与同步。
- 不做：多租户、K8s、消息中间件、CRDT、分布式数据库。
