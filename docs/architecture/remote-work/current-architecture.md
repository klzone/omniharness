# OmniDesk 现有系统架构基线（Step 1）

- 日期：2026-11-27
- 性质：只读调查结论，证据等级：static = 文件/源码事实；script = 脚本或运行证据；unverified = 未验证。

## 1. 总体判断

OmniDesk 当前是一个单机桌面 AI 工作系统：React + TypeScript UI → Rust/Tauri 宿主 → Python Engine（engine/omnidesk_engine/）→ DSH Runtime（隔离数据根）。所有状态、任务、会话、知识库与工作成果都保存在本机 ~/OmniDesk/ 与仓库内配置中。不存在任何远程访问、多终端、中央 Host 或同步机制。

对远程工作目标而言：执行底座（Python Engine + DSH + 数据布局）与 UI 是分离的，可复用；宿主生命周期与 IPC 形态（stdio 子进程）是单机假设，是主要改造点。

## 2. 四层运行架构（static）

```
React + TypeScript UI（desktop/src/）
        │ Tauri command / event（IPC-1）
Rust 宿主（desktop/src-tauri/）
        │ stdio 行分隔 JSON-RPC（IPC-2）
Python Engine（engine/omnidesk_engine/，8,800+ 行）
        │ 公开 CLI（node bin.js --profile web / acp）
DSH Runtime（runtime/dsh-runtime/，锁定 0.2.2-rc.5，只读依赖）
```

关键源码文件：

| 层 | 文件 | 职责 |
| --- | --- | --- |
| UI | desktop/src/App.tsx 及 shell/*.tsx | 场景页、任务/写作/知识界面 |
| IPC-1 | desktop/src/api.ts + src-tauri commands | Tauri 命令调用与类型 |
| 宿主 | src-tauri/src/engine_supervisor.rs | 引擎子进程启动/关闭 |
| 宿主 | src-tauri/src/engine_ipc.rs | 与 Python 的 JSON-RPC 通道 |
| 宿主 | src-tauri/src/app_paths.rs | 应用数据根解析与隔离校验 |
| Engine | engine/omnidesk_engine/serve.py | IPC-2 服务端入口（stdio JSON-RPC） |
| Engine | engine/omnidesk_engine/task_core.py | 任务/会话语义核心（995 行） |
| Engine | engine/omnidesk_engine/acp_adapter.py | ACP 通道适配器 |
| Engine | engine/omnidesk_engine/web_api_adapter.py | DSH 本地 Web API 适配器（3,277 行，默认通道） |
| 引擎底层 | runtime/dsh-runtime/ | DSH CLI，web profile 本地 HTTP + WebSocket |

## 3. 数据布局（static + 本机 ls 证实）

| 路径 | 内容 | 权威性 |
| --- | --- | --- |
| ~/OmniDesk/app/ | ui-state/ logs/ engine-state/ config/ | 桌面应用数据根 |
| ~/OmniDesk/app/engine-state/omni-*.json | 任务台账（本机 60+ 个） | 引擎任务状态权威 |
| ~/OmniDesk/app/engine-state/results/ | 任务结果正文落盘 | 结果权威 |
| ~/OmniDesk/app/engine-state/dsh-settings.json | 引擎设置 | 设置权威 |
| ~/OmniDesk/work/ | 任务 Markdown（任务/）、工作记录、项目、复盘、events.jsonl | 业务数据权威（文件优先） |
| ~/OmniDesk/workspace/ | 成果文件、写作导出、artifacts/ | 成果权威（Obsidian vault 兼容） |
| ~/OmniDesk/库/（library_root） | 知识库 | 知识权威（文件为源） |
| ~/OmniDesk/reading/ | 阅读数据 | 读书域权威 |
| ~/OmniDesk/runtime/dsh-home/ | DSH_HOME：sessions/ profiles/ skills/ storages/ | 引擎会话权威 |
| ~/OmniDesk/runtime/agents-home/ | DSH_AGENTS_HOME | 引擎 agents 权威 |
| ~/.dsh/.credentials.yaml | 模型凭证（DSH 管理） | 凭证，绝不进 Git / 传输 |

文件写入普遍具备工程规范：原子写（临时文件 + rename）、frontmatter schema（omnidesk.task.v1 等）、追加式事件表 work/events.jsonl（thread_store.rs / task_store.rs / event_log.rs，static）。

## 4. 引擎生命周期（static，约束明确）

- Engine 只能由 Rust 宿主以子进程方式启动（serve.py 明示不注册 systemd、不做常驻服务、只经 stdin/stdout 通信、不监听任何 TCP/UDP 端口）。
- 默认通道 web-api：Engine 按需启动 dsh --profile web --port 0 --no-open 本地实例，令牌与 cookie 只留在内存（web_api_adapter.py 文件头，script 证据：探针实测）。
- 宿主退出则引擎回收；窗口关闭有进程收尾（decisions.md D-111）。
- Engine 崩溃可由宿主重启，但不得自动重放中断任务（IPC 契约 3.8）。

这是当前最重要的单机假设：任务执行生命周期被绑定在一个前台桌面进程上。

## 5. 业务能力现状（与远程目标对照）

| 能力 | 状态 | 证据 |
| --- | --- | --- |
| 任务管理（Markdown 任务文件、六状态、进展/状态历史追加） | 已实现 | task_store.rs |
| 排期 / 优先级 / 四象限字段 | 部分实现（任务字段有 importance/quadrant_override/due，scheduler.rs 存在；四象限视图完整性未逐行核对） | task_store.rs / scheduler.rs |
| 多 Agent 协作（四逻辑 Agent + 事件表，方案 B） | 设计已定，事件表已实施（D-094） | OMNIDESK_MULTI_AGENT_GRAPH_DESIGN.md + decisions.md |
| 引擎任务台账 / 事件游标 / 结果落盘 | 已实现 | engine-state/ 实物 + task_core.py |
| 会话恢复（同数据根 session/resume） | 已实现并实测（合成数据） | OmniDesk-Session-Import-Compatibility.md（script） |
| 会话导入 / 跨数据根迁移 | 未实现（DSH 无公开导入入口，明确停做） | 同上 |
| 知识库（可配置根、有界文件访问、vault 浏览/编辑按 P1/P2 推进） | 部分实现 | knowledge_vault*.rs、OmniDesk-Knowledge-Base-Plan.md |
| 远程访问 / Web 服务 / 同步 | 未实现 | 全仓检索无监听服务/同步代码（仅 DSH 内部 web profile 本地实例） |
| 外联网络安全护栏 | 已实现（net_guard.rs：地址校验、公网/私网分档、防 DNS 重绑定） | decisions.md D-108 |
| 宿主路径授权注册表（文件选择 30 分钟 TTL） | 已实现 | host_access.rs |
| 自动备份 | 仅历史约定 scripts/backup-sessions.sh；当前运行目录备份机制未验证 | docs/ARCHITECTURE.md |

## 6. 文档与代码的一致性备注

- docs/ARCHITECTURE.md 自标历史基线，其中 UI 不得直接调用 DSH ACP、Python 是唯一 DSH 调用方的约束与代码一致。
- D-110 精简掉了 DSH 的 subagent / schedule 插件，但 OmniDesk 自有 Rust scheduler 不受影响。
- D-109 把 DSH 升到 0.2.2-rc.5；web_api_adapter.py 的 token/cookie 处理已适配。

## 7. 对后续 Step 的输入

- Engine 与 DSH 调用面（归一化契约、九态、事件、恢复）是可复用资产，远程化不需要重写。
- 宿主（Rust/Tauri）承担的进程管理、路径授权、任务文件写入是单机耦合点，远程化需要在 Engine/服务侧另立访问层，或让宿主变成中央进程。
- 业务数据全部是 Markdown/JSON 文件 + 少量 JSON 台账，天然适合中央文件为权威 + 客户端缓存的模型。

