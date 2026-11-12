# DSH Runtime 专项研究

- 状态：**阶段一有条件通过；阶段二源码审计通过；阶段三未开始**
- 开工快照：2026-11-12 15:05 +08:00；`main`；HEAD `5af890a84333a50a743347f45a66c9642b1dcf05`；工作区干净
- 研究负责人：本会话执行规划、任务拆解、证据审核与阶段验收
- 执行角色：DeepSeek V4.1 Flash，只按任务卡执行
- 当前技术结论：**未作决定**。继续 DSH、专用化改造 DSH、迁移 Pi 均仍是候选

## 研究问题

判断 DSH Runtime 是否适合作为 OmniDesk 的长期运行底座，并把 DSH 与 Pi 在同模型、同任务、同工具条件下的差异归因到可验证的机制、配置或插件层。

必须分开的六个机制：

1. System Prompt Assembly
2. Agent Loop
3. Session Replay / Context Projection
4. Tool Description / Tool Schema
5. Parallel Tool Scheduler
6. Subagent Lifecycle

## 边界

- 本轮只读研究和研究文档编写。
- 不修改产品源码、数据库结构、生产数据或 `runtime/` 内 DSH Runtime。
- 不安装或升级依赖、插件或 Pi。
- 不启动源码重构、框架迁移或模型调用实验。
- 不读取、打印或复制凭证；不读取会话正文；不接触真实业务材料。
- 不使用官方最新分支或最新包结构替代本地锁定版本。
- 官方 README、官方设计说明、历史审计和模型经验只能作为候选线索，不能单独充当源码行为证据。

## 证据等级

| 标记 | 含义 | 最低要求 |
| --- | --- | --- |
| `F` 文件事实 | 路径、版本、哈希、文件状态、配置值 | 可复核的路径与命令 |
| `S` 静态代码 | 源码或编译产物的具体行为 | 文件、行号、被追踪的入口或符号 |
| `R` 脚本/运行 | 实际进程、命令、接口或日志观察 | 命令、退出码、时间、隔离条件 |
| `M` 真实模型 | 真实 Provider 的请求与输出 | 模型、Provider、参数、工具权限、任务条件与原始记录 |
| `D` 官方说明 | 官方 README、设计文档或发布说明 | 版本、链接、原文位置；不能单独证明实现行为 |
| `U` 待验证 | 尚无足够证据 | 明确的未知点和下一步验证方法 |

同一结论可以引用多个等级，但不得把 `D` 写成 `S`，也不得把请求次数、Token 数量或任务完成率合成为一个“性能”指标。

## 文档索引

- [00-baseline.md](00-baseline.md)：当前版本、来源、Profile、模型路由和研究缺口
- [01-master-plan.md](01-master-plan.md)：五阶段进入条件、任务、交付物和验收标准
- [02-task-cards-phase1.md](02-task-cards-phase1.md)：第一批只读任务卡
- [03-phase1-supplement.md](03-phase1-supplement.md)：执行修订与补充任务卡
- [04-phase1-review.md](04-phase1-review.md)：首批证据复核记录
- [05-source-baseline.md](05-source-baseline.md)：rc.5 精确源码与 commit 基线
- [06-phase2-task-cards.md](06-phase2-task-cards.md)：六张单动作源码审计任务卡
- [07-phase2-review.md](07-phase2-review.md)：六份静态源码审计复核
- [08-dsh-core-architecture-map.md](08-dsh-core-architecture-map.md)：初步核心架构图
- [09-discipline-lite-plan.md](09-discipline-lite-plan.md)：Discipline 精简 Profile / Preset 优化计划（待审定）
- [evidence/README.md](evidence/README.md)：证据落盘格式与命名规则

## 当前门禁

- `G0` 研究目录与基线已建立。
- `G1` 阶段一验收：等待第一批任务卡执行结果，由研究负责人独立复核。
- `G2` 阶段二源码门禁：已满足；rc.5 源码在 `/tmp` 固定到 `b830ec95`。
- `G3` 阶段三运行门禁：安装 Pi、启动真实模型对照或改变 Provider/凭证前，必须取得用户单独授权。
- `G4` 实施门禁：任何源码改造或迁移，必须等阶段五决策和用户批准。

共享状态文件仍是项目协作的权威入口；本目录只承载专项研究材料，不替代 `GOALS.md`、`PLANS.md`、`STATUS.md`、`REVIEW.md` 或 `docs/decisions.md`。
