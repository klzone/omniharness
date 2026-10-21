# OmniDesk/DSH 对照分析（基线：main @ 15aef4f + 048 进行中的未提交改动，未触碰）

| 领域 | OmniDesk/DSH 现状 | 证据 | 缺口 | dsh-java 对应 | 更简单方案 |
|---|---|---|---|---|---|
| A ReAct 内核 | 由 DSH Runtime 承担，OmniDesk 仅有两个适配器（ACP 1242 行 / web-api 1742 行），不拥有循环 | engine/omnidesk_engine/*.py | 无缺口——职责正确在 DSH | ReactLoopAgent | 无需行动 |
| B 工具治理 | 宿主强制写确认（046），来源固定 model_unverified、未确认不落盘 | desktop/src-tauri/src/pending_writes.rs | 执行入口无统一"终检"兜底；未来新增路径可能绕过 | 审批门禁+PermissionCheckNode+PENDING_APPROVAL 执行抛异常 | 执行入口加 fail-closed 校验（P1-1） |
| C 插件 | 无插件体系（既定原则） | AGENTS.md | 无缺口——不建是决策非缺陷 | 三机制并存 | 不借鉴 |
| D 上下文 | 会话由 DSH 管理；OmniDesk 线程持久化为自有日志 | 045 线程落数据根 | 线程日志无版本字段、无迁移测试 | SessionEventLog+FormatMigrationTest | 日志加 version+迁移用例（P1-2） |
| E Token 观测 | 048 将从 usage 帧取实时用量；UI 已有 token 显示 | 045 探针 usage_update 帧 | 会话/任务级累计不落盘 | TokenSnapshot+投影持久化 | 快照落盘（P2） |
| F 部署边界 | 单实例单用户 Tauri | 架构既定 | 无缺口 | 单体对应 | 无需行动 |

## 证据等级
- B/D/E 现状：文件事实 + 046 复核（REVIEW.md）。
- "缺口"判断基于本次静态检索；token 节省类收益均为待验证假设。
- 未运行 dsh-java 任何代码；未运行 OmniDesk 真实模型链路。

