# dsh-java 源码审计记录（证据：仓库 linlannet/dsh-java @ 3b9f729，浅克隆至 /tmp/dsh-java-audit）

## A. ReAct 执行内核
ReactLoopAgent.java（domain/agent/service/run，1115 行）：收件箱唤醒 → turn 内流式调 LLM → 工具 → 回填循环；四个流式回调（delta/reasoning/toolEvent/finish）；disposed 标记 + phase + turnCount。调用链与注释一致，属成熟实现。**但该循环属执行引擎职责，OmniDesk 由 DSH（Node）承担，无落点。**
测试：ReactLoopAgentScopeDispatchTest、ReactLoopAgentCheckpointTest 存在。

## B. 工具执行与权限治理
ToolCallExecutor.java（850 行）为唯一工具执行入口：
- 五阶段管线 PRE→GUARD→AROUND→POST→RESULT（L135-155），PRE Hook 可阻断（L200-216），审批门禁在真实执行前（L188-196 checkApproval）。
- **审批默认放行**：字段默认 allowAll()（L65、L126-127；RuntimeApprovalGate.java L12/L56）。审批是可选配置，非默认强制。
- 任务链兜底：README §3.5，PermissionCheckNode→ApprovalPolicyService；PENDING_APPROVAL 状态在执行入口直接抛异常防绕过（状态机 CREATED→PENDING_APPROVAL→QUEUED→RUNNING→COMPLETED/FAILED）。
- 已核风险点：静态 TOOL_WORKER = newCachedThreadPool（无界）；全仓 Executors.new* 共 16 处。

## C. 插件机制
三种并存：Java SPI（JavaPluginLoader/PluginHotReloader，ClassLoader 热载）、Node Bridge（JsonRpcPluginToolBridge）、MCP；插件工具注册为 plugin__<id>__<tool> 后走统一 Hook/守卫链路（README §3.5）。生命周期 install→activate→run 有状态管理与测试（8 个插件测试类）。**机制完整但对 OmniDesk 无需求：单用户工作台无第二插件生态，且违反"不做插件市场"既定决策。**

## D. 上下文管理
BasicCompactionEngine：压力阈值+最小消息数触发 → LLM 摘要替换原区间。配套 ToolResultPruner（工具结果裁剪）、ImageOffloader（图片卸载）。测试齐备（BasicCompactionEngineTest 等 3 个）。
**无压缩前后任务连续性/节省量实测证据；对 OmniDesk 无直接落点（上下文由 DSH 管理），但"工具结果先裁剪再进历史"思想可用于未来自有长线程摘要。**

## E. 事件日志与可观测性
SessionEventLogService：append → log.append → store.append → triggerProjection；格式迁移有专门测试（SessionFormatMigrationTest、MigrationCatalogTest、LegacyEventAdapterTest）。0.2.1 新增 OTel、会话全文检索、统计/标题/大纲投影。
Token 计量：SessionTokenMeterService、TokenSnapshot、ReplayAwareTokenMeter + JdbcTokenSnapshotRepository（持久化快照）。

## F. 部署边界
单体 Spring Boot，H2 或 MySQL；无分布式调度。线程池 16 处（多 cached/fixed，上限不统一）；工具超时默认 300s。单实例内存态在单用户场景可接受。

## 总体质量判定
有实际工程价值：测试密度高、状态机有防绕过兜底、事件日志带迁移测试。主要短板：审批默认放行、无界线程池、预览期接口不稳定。

