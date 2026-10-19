# OmniDesk Discipline

OmniDesk Discipline 是面向个人专业工作的独立 AI 工作台。它拥有自己的任务体验、领域规则、数据组织和成果管理；DSH 只是当前可替换的执行引擎，不是产品 UI，也不是 OmniDesk 的业务边界。

## 当前状态

- 当前仓库包含自有 Tauri/React/Rust 桌面技术切片和个人 MVP 纵向试点。它们证明了部分技术链路，不代表正式桌面产品已获批准。
- ARCHITECT 已独立复验 **OD-BUILD-002 通过（计划范围）**，桌面数据根拒绝仓库内落点的遗漏已关闭；这不等于完整 MVP 已验收。见 [REVIEW.md](REVIEW.md)。
- OD-BUILD-003/004 已完成实现并经 ARCHITECT 独立复核通过（计划范围）：cwd 规范化、宿主默认 Profile 链（acp）与非 ACP Profile 的可读错误均已证实。下一项待批准计划为 OD-BUILD-005（把引擎链路接进任务流程），见 [PLANS.md](PLANS.md)。真实 Tauri 窗口内的完整任务路径与真实模型输出质量仍未验证。
- 当前仅使用合成数据及明确许可范围内的公开数据。用户转述许可只覆盖满足指定脱敏要求、个人数据分析与生态研判且不得分发；不能推定其他任务、模型服务商、工具、设备或运行环境已获准。

## 共享项目状态

- [AGENTS.md](AGENTS.md)：长期协作规则与硬边界
- [GOALS.md](GOALS.md)：产品目标、当前阶段和范围
- [PLANS.md](PLANS.md)：唯一当前实施计划及审批状态
- [STATUS.md](STATUS.md)：BUILDER 执行记录
- [REVIEW.md](REVIEW.md)：ARCHITECT 独立审查与验收结论
- [docs/decisions.md](docs/decisions.md)：已定架构决策与未决事项

详细工程契约、数据布局、审计报告和测试证据保留在 docs/、tests/reports/、tests/verification/。历史规划和路线材料归档在 docs/archive/。

## 关键架构边界

- 不修改 runtime/deepseek-harness/；不把 DSH 内部格式当作 OmniDesk 产品契约。
- 共用任务外壳可服务相似的周报、材料撰写和日常项目管理，但每种能力仍需独立规则、权限与评测；目前仅有脱敏数据分析与生态研判试点。
- 文件优先；只有出现可观察的并发冲突、状态查询瓶颈或文件检索评测不达标时，才评估 SQLite。
- 不建设通用 Agent 平台、工作流编辑器、Registry、插件市场、微服务或复杂模型路由层，除非后续证据和单独批准支持。
- 作者个人试用不以 20% 节时为门槛；扩展到同事或启动产品化需另行评估价值、质量、授权和具体产品痛点。

## 验证入口

- 桌面 MVP 审查与限制：[REVIEW.md](REVIEW.md)
- 桌面架构与运行边界：[docs/OmniDesk-Desktop-Architecture.md](docs/OmniDesk-Desktop-Architecture.md)、[docs/OmniDesk-Desktop-Data-Layout.md](docs/OmniDesk-Desktop-Data-Layout.md)、[docs/OmniDesk-Desktop-Engine-IPC.md](docs/OmniDesk-Desktop-Engine-IPC.md)
- 阶段和适配器证据：[tests/reports/](tests/reports/)
- DSH 隔离运行引导：[scripts/setup.sh](scripts/setup.sh)
