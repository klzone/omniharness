# 分阶段实施计划（Step 14）

- 日期：2026-11-27
- 原则：小步改造；先中央 Host，再远程访问，再统一 Workspace，再任务接续，最后同步/离线与可靠性收尾。允许按现状调整拆分，理由随阶段说明。

## 调整说明

原建议 Phase 5（Mobile + Reliability）拆开：可靠性（launchd/备份）提前到 Phase 1/2 顺势做（中央 Host 本身就要 launchd 与健康检查，避免裸奔窗口）；移动端最后做，因为 Web UI 移动适配依赖前面阶段稳定。

---

## Phase 1：Central Host + Remote Access（最小可用远程）

**阶段目标**：Mac mini 常驻 Host；办公室笔记本经 Tailscale 用 Web 访问任务与会话；关 UI 不影响任务。

**实施范围**：
1. 从 Tauri 宿主抽出无 UI Host Core（Rust lib/bin 目标），复用 engine_supervisor / engine_ipc / task_store / scheduler 等模块。
2. launchd plist（RunAtLoad + KeepAlive）+ 安装/卸载脚本。
3. HTTP API Server（回环绑定）：/api/health、任务 CRUD、会话 list/resume、事件表读、SSE 事件流、Bearer 令牌认证（令牌落 config/，权限 0600）。
4. Tailscale 安装与 tailnet 组网（用户侧操作，文档化）。
5. 最小 Web UI：现有 React 组件的 API 化壳（任务列表/详情、会话列表/恢复、事件时间线），vite 构建由 Host 静态托管。

**涉及模块**：src-tauri 核心（抽库）、新增 host-core bin、新 API 模块、新 web 壳、launchd 配置。
**前置依赖**：无（本阶段即起点）。
**具体任务**：见上；每项独立 commit。
**验收标准**：
- 杀 Host 进程 → launchd 拉起 → /api/health 200。
- 办公室笔记本（tailnet）浏览器完成任务查看与一个任务状态迁移。
- 关闭 Mac mini 上一切 UI → 已提交任务继续执行到终态。
- 令牌缺失/错误请求 401。
**风险**：宿主抽库牵动 Tauri 构建（风险最高项，用"宿主薄壳调用核心库"降低）；SSE 在 Tailscale 偶发断流（客户端自动重连 + seq 补漏缓解）。
**回滚方案**：卸载 launchd 服务即回到桌面单机模式；数据根不变。
**数据库变更**：无。
**影响现有桌面端**：Desktop UI 保留本地模式（直连核心库）与远程模式（指向 API）双态；默认行为不变。

## Phase 2：Unified Remote Workspace

**阶段目标**：成果/知识/附件经 API 读写；文件缓存；Agent 产物远程可见。

**实施范围**：/api/workspace/*、/api/kb/*、/api/reading/*、/api/artifacts/*（只读）；If-Match 并发校验；客户端文件缓存（LRU）；文件上传下载 API。
**涉及模块**：Host Core 文件服务、vault/knowledge 模块 API 化、Web UI 文件浏览/编辑页、api.ts 双态数据源。
**前置依赖**：Phase 1。
**验收标准**：远程新建/编辑一个成果文件并保存成功；并发修改产生 409；附件可下载；Agent 产物在 Web 可见。
**风险**：二进制附件大小上限与传输效率（上限沿用现有 8MiB 类约束起步）。
**回滚**：API 域下线即回 Phase 1 状态，文件数据无变更。
**数据库变更**：无。
**影响桌面端**：文件功能改走同一 API（或保留本地直读），行为不变。

## Phase 3：Task Continuity + Handoff

**阶段目标**：任务中断/Host 重启后可续做；GLM→DeepSeek→GLM 工作流跨会话连续。

**实施范围**：
1. 任务文件新增 ## 计划 / ## 待办 段（schema v1 兼容，旧任务无段照常）。
2. omnidesk.handoff.v1 schema + work/handoff/ 目录 + 快照生成（步骤完成/中断/确认时）。
3. Host 启动扫描台账 → interrupted 任务列"可续做"。
4. "续做"动作 = 按 snapshot 新建任务（新 omni_task_id），prompt 由 snapshot + Skill 组装。
5. Plan 步骤模型→Agent 映射（模型路由走现有 model_routes / DSH selectModel）。
**前置依赖**：Phase 1（中央 Host）；Phase 2 非必须但推荐。
**验收标准**：执行中杀 Host → 重启后可续做清单出现该任务 → 续做产生新任务并携带原 Artifacts 引用；三步工作流（规划/执行/验收）跨会话完成一次真实演练。
**风险**：续做 prompt 组装质量依赖 context_digest；快照 schema 迭代。
**回滚**：schema 字段向后兼容，移除 UI 入口即回退；既有任务文件不受影响。
**数据库变更**：无（文件 schema 增段）。
**影响桌面端**：任务详情页增加计划/待办/续做入口。

## Phase 4：Sync + Offline Capability

**阶段目标**：成果/知识离线新建与编辑；重连同步；冲突可裁决。

**实施范围**：
1. 客户端 Sync Queue（localStorage/IndexedDB）：操作 + 基线 revision。
2. 提交协议：client_op_id 幂等 + If-Match；409 冲突裁决 UI（三方对比）。
3. events seq 增量拉取驱动缓存失效。
4. 删除同步：软删事件 + 缓存清除。
**前置依赖**：Phase 2（文件 API + ETag）。
**验收标准**：飞行模式下新建成果与编辑笔记 → 恢复网络自动提交成功；人为制造冲突 → 裁决界面可用且数据无损。
**风险**：队列顺序与部分失败处理（逐项提交、失败保留并提示）。
**回滚**：关闭离线开关即回纯在线；队列数据不删。
**数据库变更**：无。
**影响桌面端**：Desktop 与 Web 共用同一同步层。

## Phase 5：Mobile Access + Reliability 收尾

**阶段目标**：手机/平板可用的移动 Web；备份/恢复/监控完整。

**实施范围**：
1. Web 移动适配（响应式 + 移动导航；复用现有 React 组件，不新建技术栈）。
2. 备份脚本（每日 APFS clone + 保留策略）+ 恢复演练。
3. 磁盘水位监控接入 /api/health 与通知。
4. 访问日志与审计补全。
**前置依赖**：Phase 1-4。
**验收标准**：手机浏览器完成"查看任务 + 提交一个小任务 + 收到完成通知"；备份恢复演练通过；低磁盘告警可见。
**风险**：移动端推送通知受限于 Web Push（首期用轮询/SSE 页内通知即可，不做原生推送）。
**回滚**：各子项独立，可单独回退。
**数据库变更**：无。
**影响桌面端**：无行为变化。

## 阶段间依赖图

```
P1 Central Host ──► P2 Workspace ──► P4 Sync/Offline ──► P5 Mobile+Reliability
       │
       └──────────► P3 Continuity/Handoff ─────────────────┘
```

P3 与 P2 可并行；P5 依赖 P1-P4 全部。
