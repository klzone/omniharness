# Agent Session 持久化与跨设备接续能力评估（Step 3）

- 日期：2026-11-27
- 问题：Session 存储在哪、能否恢复、能否跨设备访问、是否绑定设备、并发/锁风险。

## 1. 现状事实

| 项 | 现状 | 证据等级 |
| --- | --- | --- |
| DSH 会话存储 | ~/OmniDesk/runtime/dsh-home/sessions/<编码后的工作区>/<sessionId>/session.v4.jsonl.zstd + storages/session_projcache/ 投影缓存 | static（DSH-Session-Import-Compatibility.md） |
| 会话格式 | DSH 内部私有格式，带版本号、压缩、与投影缓存联动 | static |
| 公开恢复接口 | ACP session/list、session/resume、session/fork 可用；session/import 不存在 | static（核对 dsh-acp/lib/index.js 1362-1365 行） |
| 同数据根内恢复 | 已实测（合成数据，verify_session_continuity.py）；重开不新建会话、不重放提示词 | script |
| 跨数据根/跨版本导入 | 未实现且明确停做（依赖内部格式，升级风险） | static（结论文档） |
| 会话与任务绑定 | engine_session_id 由 Engine 适配层持有，UI 不可见；omni_task_id 与 engine_session_id 一对一（可显式恢复重绑） | static（契约 §2） |
| OmniDesk 侧线程记录 | <app 根>/threads/<项目分目录>/<thread-id>.json，versioned，原子写，跳过/报告异常文件 | static（thread_store.rs） |
| 会话历史用途 | Agent 启动时读最近 events.jsonl 事件装配上下文（不依赖完整聊天历史） | static（Multi-Agent 设计 §四） |
| 并发/锁 | process_owner.py 单实例互斥；event_log.rs 序号进程内串行；跨进程并发写明确不在范围 | static |

## 2. 跨设备接续的关键判断

1. 会话数据本身**不绑定设备**，只绑定数据根路径。把 ~/OmniDesk/runtime/dsh-home 放在中央 Host 上，任何终端经 API 请求 Engine 恢复会话在技术上成立（前提：Engine 在中央运行）。
2. 会话历史**不应也不需要**成为任务恢复的唯一依据。已有三份独立持久层：任务文件（work/任务）、事件表（events.jsonl）、结果落盘（engine-state/results/）。
3. 模型切换：DSH 支持按会话 selectModel（D-109 验证过 RPC 存在），GLM 规划→DeepSeek 执行→GLM 验收可在同一会话切模型或分多个会话+结构化交接（候选/verdict JSON）。
4. Session 退出/客户端切换：只要中央 Engine 持续运行，客户端断开不影响会话状态；重连后用 stream_events 补漏（推拉结合，契约 3.3/3.5）。
5. Host 重启：中断任务进 interrupted/unknown 终态，不可自动重放；恢复依赖"任务文件 + 事件 + 结果"重建上下文新建任务，这部分在 Step 10 设计。

## 3. 并发与锁风险（多终端场景）

| 风险 | 现状 | 远程化影响 |
| --- | --- | --- |
| 同一数据根多实例 DSH | process_owner 互斥；OD-ARCH-074 明确反对同根多实例 | 多终端同时触发 Engine 启动会撞互斥；必须收敛为单一中央 Engine 实例 |
| 同一 Markdown 任务文件双端写 | 原子写 + 保存前哈希检查（vault 写入有外部变更检测；task_store 写入走 write_new_atomically/write_replace_atomically） | 需在服务层做乐观并发（版本号/etag），文件哈希兜底，见 Step 11 |
| events.jsonl 跨进程写 | 明确单进程假设 | 单中央 Engine 下成立；多进程写会破序号，远程化后必须保持单写者 |
| DSH session 文件直接读 | UI/领域层禁止读 DSH 内部文件（契约红线） | 远程化不改变该约束，会话只能经 Engine API 访问 |

## 4. 结论

- 会话持久化与同数据根恢复：已实现、可复用。
- 跨设备访问：需中央 Engine 常驻 + API 暴露（session/list、resume、stream_events 已具备），Engine 层改动很小。
- Session 历史不是恢复唯一依据；已有的任务文件/事件表/结果落盘三份持久层才是接续的核心，这正是"独立于模型上下文的 Task Context"雏形。
- 需要补的：跨会话模型切换的产品化编排（GLM 规划→DeepSeek 执行→GLM 验收），属 Step 10；单实例收敛与并发写控制属 Step 11。
