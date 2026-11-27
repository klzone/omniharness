# 远程工作专项架构评审报告（Step 15 / 最终报告）

- 日期：2026-11-27
- 性质：汇总 14 项调查与设计结论，回答重点研究问题，给出唯一主方案与实施入口。
- 证据声明：所有现状结论基于本仓库源码与既有验证记录（static/script 级）；标注"未验证/待验证"处已列明验证方法。本次未运行任何修改性操作。

## 一、总结论

### 1. 当前 OmniDesk 架构是否适合改造

**适合，且改造成本集中在访问层而非执行层。** 依据：

- 执行底座（Python Engine + 归一化契约 + DSH + 九态状态机 + 事件 + 恢复）与 UI 完全分离，Engine 本身不依赖 UI（serve.py/task_core.py，static）。
- 数据层是"文件优先 + 原子写 + 哈希校验 + 追加式事件表"，天然契合中央权威模型（task_store.rs / event_log.rs，static）。
- 唯一的硬单机假设是"Engine 由前台 Tauri 宿主以 stdio 子进程启动"，这正是 Phase 1 要解开的点。

### 2. 最推荐的目标架构（唯一主方案）

**Mac mini 上以 launchd 常驻的"无 UI Host Core"（Rust，复用现有 Tauri 宿主核心模块）+ 现有 Python Engine/DSH 子进程链 + 回环 HTTP API（认证令牌 + REST/SSE）+ Tailscale 私网多终端接入 + 复用现有 React 组件的 Web UI。**

要点：

- IPC-2（宿主↔Engine，stdio JSON-RPC）契约不变，这是最大复用点。
- 数据权威全部在 Mac mini 本地文件；终端只有缓存。
- 不引入：多租户、K8s、消息中间件、CRDT、新数据库、第三方自动同步（首期）。

### 3. 最小可用改造范围（Phase 1）

需要修改/新增：

1. src-tauri 宿主核心抽库 → 无 UI host-core 可执行（复用 engine_supervisor/engine_ipc/task_store/scheduler/event_log 等）。
2. launchd plist + 安装脚本（新增）。
3. HTTP API（回环 + Bearer 令牌；health/任务/会话/事件/SSE）（新增）。
4. 最小 Web UI 壳（复用 desktop/src 组件，api.ts 双态）（新增）。

暂不需要修改：Python Engine、DSH Runtime、数据格式、安全护栏（net_guard/host_access 原样复用）、知识库/写作/阅读等业务模块。

### 4. 最终数据架构（权威源及访问方式）

| 数据 | 权威源 | 访问 |
| --- | --- | --- |
| 任务/项目/记录/排期/事件 | Mac mini ~/OmniDesk/work/ 文件 + events.jsonl | API 读写 + 客户端缓存 |
| 成果 workspace | ~/OmniDesk/workspace/ | API + If-Match；离线队列（Phase 4） |
| 知识库 | ~/OmniDesk/库/ | 同上 |
| 附件 | 随所属域目录 | API 下载 |
| Agent 产物 | ~/OmniDesk/app/engine-state/results/ | API 只读 |
| Agent 会话 | ~/OmniDesk/runtime/dsh-home/ | 仅 Engine API（list/resume/fork），禁止文件同步 |
| 引擎台账 | ~/OmniDesk/app/engine-state/ | API 只读 |
| UI 状态/日志/索引 | 各终端本地 | 不同步 |
| 凭证 | ~/.dsh/.credentials.yaml（仅中央，DSH 自管） | 不同步、不备份、不传输 |

### 5. 跨设备工作连续性方案

- 中央 Engine 常驻：客户端断开不影响会话与任务。
- 会话恢复：同数据根 session/resume（已实现并实测）。
- 任务接续：不依赖聊天历史；以任务文件（计划/进展/待办/决策）+ 事件表 + 结果落盘 + Handoff Snapshot（omnidesk.handoff.v1，新增）重建上下文，续做=新建任务引用快照。
- 多模型工作流：Plan 每步声明模型；步骤间经文件域交接（候选/verdict/snapshot），GLM 规划→DeepSeek 执行→GLM 验收各步可换设备、换会话、换模型实例。

### 6. 双向同步方案

- 同步对象：成果与知识库的新建/编辑（Phase 4）；任务元数据/追加段在线乐观并发（If-Match + 409）。
- 机制：客户端 Sync Queue（操作+基线 revision）+ client_op_id 幂等 + events seq 增量失效 + 三方对比冲突裁决。
- 禁止同步：DSH 会话、引擎台账、events.jsonl 文件本体、凭证、UI 状态、日志。
- 删除统一软删。第三方工具（Syncthing）：Phase 4 后按需评估，仅限 workspace/库 两目录，非必须。

### 7. 分阶段实施计划

见 implementation-roadmap.md。阶段依赖：P1 Central Host → P2 Workspace / P3 Continuity（可并行）→ P4 Sync/Offline → P5 Mobile+Reliability。每阶段含目标/范围/验收/风险/回滚，全部无数据库迁移。

### 8. 风险清单

| 类别 | 风险 | 缓解 |
| --- | --- | --- |
| 架构 | Tauri 宿主抽库牵动构建 | 宿主薄壳化；核心库独立测试；失败即回滚单 commit |
| 架构 | 上下文严重冗长后再建会话时交接质量 | Handoff Snapshot 人可读可改；续做 prompt 组装有 Skill 模板 |
| 数据 | 并发写冲突 | If-Match + 409 + 软删；events 单写者约束写进 API 设计 |
| 数据 | 备份与 DSH 会话一致性 | 停机窗口备份或空闲时段备份；恢复演练纳入 Phase 5 验收 |
| 网络 | Tailscale 依赖第三方协调服务 | 免费档 + 可自建 headscale 备选；断网时仅影响远程访问，中央任务不受影响 |
| 安全 | 令牌泄露/设备失窃 | 短效令牌+吊销；终端缓存可清除；全链路 WireGuard |
| 安全 | 远程触发 Agent 高风险操作 | 沿用 action_gate/admission；远程确认通道；命令面不扩大 |
| 维护 | API/Web 壳成为第二个 UI 面 | 组件复用最大化；api.ts 双态收敛调用点 |

### 9. 重点研究问题逐项回答（速览）

**A. 中央 Agent**

1. Engine 能否独立于 UI 运行？——执行逻辑能（已实现）；当前缺独立启动者（宿主启动权是唯一障碍）。
2. Mac mini 能否做中央 Host？——能；数据布局/凭证/会话全部可原地常驻。
3. 关 UI 是否影响任务？——当前影响（宿主退出则 Engine 回收）；Phase 1 后不影响。
4. 是否需调整 Engine 生命周期？——是：由 launchd 托管的 Host Core 拉起（IPC 契约语义不变）。
5. 是否需要系统级服务与自动重启？——需要（launchd KeepAlive），Phase 1 内完成。
6. Agent 执行是否绑定本地设备？——当前绑定"运行宿主的那台设备"；改造后绑定中央 Host，与客户端设备无关。

**B. 远程访问**

1/2. 办公室/家/移动如何连接？——Tailscale 私网，浏览器访问 Web UI。
3. 当前 Desktop UI 能否远程？——不能（IPC-1 是本机 Tauri 通道）；远程用 Web UI（Desktop 可改指 API 作可选）。
4. 是否需要 Web UI？——需要（移动端无 Tauri 客户端）。
5. 复用哪些组件？——desktop/src 全部场景组件，api.ts 层替换数据源。
6. API/WS/文件传输设计？——REST + SSE（Phase 1）→ WebSocket（Phase 2+）；文件走域白名单 API + 附件下载。
7. 防未授权操作？——tailnet 设备准入 + Bearer 令牌 + 操作审计（events）+ Agent 高风险动作沿用门控。

**C. 工作连续性**

1. Session 如何保存？——DSH 数据根 session.v4 文件（中央，Engine 管理）。
2. 如何恢复未完成任务？——interrupted 识别 + snapshot 续做（新建任务）。
3. GLM→DeepSeek 切换？——Plan 步骤模型映射 + 步骤间文件域交接。
4. GLM 验收？——验收步骤读 snapshot+artifacts，产出 verdict。
5. Session 历史是否唯一依据？——否；任务文件/事件/结果/snapshot 才是。
6. 独立于聊天历史的 Task Context？——§5 的 Task Context 对象模型（已有持久层升级）。
7. 上下文耗尽/Agent 退出/Host 重启？——均以"新会话 + snapshot"恢复，不依赖原会话续聊。

**D. 双向同步**——见 §6 与 sync-data-matrix.md / sync-protocol-design.md。

**E. Workspace 及知识库**

1. 工作目录管理？——文件域白名单 + 宿主路径校验（已有机制）。
2. KB 适合中央化？——适合（文件即权威，中央 Agent 需要读它）。
3. 是否需要本地离线副本？——Phase 4 起对库/ 域提供缓存+离线编辑；首期在线即可。
4. 文件附件管理？——API 列举+受限下载，二进制不解析（沿用 KB 计划边界）。
5. 避免多端同步数据库文件？——本就无中央数据库；会话与台账域禁止文件同步，从矩阵上排除。
6. Agent 访问中央 Workspace？——Agent 在中央 Host 本地直读，路径与语义不变。
7. 客户端不破坏执行环境？——客户端无中央文件挂载；一切经校验 API；会话域无文件 API。

**F. 安全及可靠性**

1. 身份验证？——Bearer 令牌（config 落盘 0600）+ tailnet 设备准入。
2. 文件/命令权限？——域白名单 API；命令执行面不因远程扩大（Profile 工具面决定）。
3. 项目隔离？——单用户场景按数据域隔离；API 按域白名单，不做多租户。
4. 密钥保护？——DSH 自管 + 不传输 + 日志打码（既有原则）。
5. 客户端异常不影响中央？——客户端无中央写权限（除队列提交），崩溃只影响本地缓存。
6. 备份恢复？——每日 APFS clone + 保留策略 + Phase 5 恢复演练。
7. 任务中断恢复？——见 §5。

## 十、下一阶段 Codex 实施规划入口（实施总任务指令）

> 以下指令可直接交给新的 Codex 规划/实施会话。本次研究不开始实施。

```
# OmniDesk Phase 1：Central Host + Remote Access 实施任务

背景：先读 docs/architecture/remote-work/ 全部 15 份文档（本专项研究结论），
再读 docs/OmniDesk-Desktop-Engine-IPC.md、docs/OmniDesk-Desktop-Data-Layout.md。

目标：在不改变现有桌面单机行为的前提下，为 Mac mini 增加可由 launchd 托管的
无 UI Host Core，暴露回环 HTTP API（Bearer 认证），提供最小 Web UI，
使办公室/家中设备经 Tailscale 浏览器可查看任务、会话与事件，并提交任务。

范围（按序）：
1. 把 desktop/src-tauri 中与窗口无关的宿主能力（engine_supervisor/engine_ipc/
   task_store/scheduler/event_log/model_routes/app_paths 等）抽成核心库 crate，
   Tauri 宿主变薄壳调用；保持既有测试全绿（cargo test --lib 与现有数量基线）。
2. 新增 host-core 二进制目标：与环境变量启动 Engine，抢 process_owner 锁，
   提供 /api/health（进程/引擎心跳/数据根可写）。
3. 新增 HTTP API（仅回环绑定）：任务 CRUD（复用 task_store 写口，附 ETag）、
   会话 list/resume（经 Engine IPC omni.* 消息）、events.jsonl 按 seq 读、
   SSE 事件流（Engine omni.event 桥接 + seq 补漏）。认证：Bearer 令牌，
   令牌文件 config/api-token（0600），缺失时启动生成。
4. 新增 web/（vite 构建，Host 静态托管）：任务列表/详情、会话列表/恢复、
   事件时间线、任务提交表单。复用 desktop/src 组件，api.ts 增加
   "远程 fetch 数据源"双态。
5. 提供 scripts/install-launchd.sh / uninstall-launchd.sh（RunAtLoad+KeepAlive，
   用户级域）。README 增补 Tailscale 组网与令牌管理说明。

约束：不修改 runtime/ 内 DSH；不修改 engine/omnidesk_engine/（IPC 契约不变）；
不改业务数据格式；不引入数据库；不引入 WebSocket（Phase 2 再做）；
不开放 0.0.0.0；凭证不进 API/日志。全部行为与 docs/architecture/remote-work/
各设计文档一致，冲突时以设计文档为准并回报差异。

验收（每项留运行证据）：
- 杀 host-core → launchd 10s 内拉起 → /api/health 200。
- 提交一个真实任务，期间关闭全部 UI，任务运行到终态且结果落盘可查。
- 无令牌/错令牌请求 401；路径穿越请求被拒。
- cargo test --lib 与 npx tsc --noEmit 通过；新增模块有单测。
- 回滚演练：uninstall-launchd.sh 后桌面单机模式行为不变。
```

## 附：未验证事项与建议验证方法

| 事项 | 状态 | 验证方法 |
| --- | --- | --- |
| 四象限视图产品化完整度 | 待验证 | UI 检查 ProjectBoard/WeeklyTimeline 是否有四象限视图 |
| DSH 会话跨版本兼容（0.2.1→0.2.2 旧会话 resume） | 未验证 | 隔离数据根合成/真实旧会话 resume 实测 |
| SSE 在 Tailscale 弱网下的稳定性 | 未验证 | Phase 1 实测（seq 补漏机制已设计） |
| 备份与 DSH 会话一致性（热备份） | 未验证 | Phase 5 恢复演练 |
| 真实 GLM→DeepSeek→GLM 三步工作流演练 | 未验证 | Phase 3 验收项 |
