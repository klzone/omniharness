# OmniDesk (OmniHarness) 系统架构规范与边界约束

> 版本：v1.0.0 (Phase 0 历史架构基线)
> 状态：历史基线；当前产品方向见 [GOALS.md](../GOALS.md)，架构决策见 [decisions.md](decisions.md)。本文件只说明早期 DSH 验证架构。
> 适用工程：OmniDesk / Yvan AI Workbench

> 当前架构图描述的是既有 DSH 工作站分层，不代表目标桌面应用已经实现。原“复用 DSH UI、禁止自研 GUI”的阶段决策已更新；具体技术仍须通过路线图中的接口与跨平台验证。

> **引擎适配层（2026-10-16 新增）**：面向未来的自有桌面应用，OmniDesk 已在仓库内建立可替换的引擎边界
> `engine/omnidesk_engine/`，位于 UI Layer 与 DSH Runtime 之间。它只通过 DSH 公开 CLI 入口
> (`--profile acp`) 通信，把引擎内部的会话 ID、JSON-RPC 请求 ID 与轮次 ID 全部封在层内，向上只暴露归一化的
> `omni_task_id`、九态状态、事件与标准错误码。上层（UI/领域工作流）不得直接解析 CLI 私有输出或读取 DSH 数据文件。
> 该层零第三方依赖，不修改 DSH Runtime。接口契约见 [OmniDesk-Engine-Interface-Contract.md](OmniDesk-Engine-Interface-Contract.md)，
> 验收矩阵与实测结果见 [OmniDesk-Engine-Interface-Test-Matrix.md](OmniDesk-Engine-Interface-Test-Matrix.md) 与
> [../tests/reports/p1-engine-adapter-prototype-report.md](../tests/reports/p1-engine-adapter-prototype-report.md)。
> 边界说明见 [../engine/README.md](../engine/README.md)。

---

## 一、系统五层架构模型

OmniDesk 采用严格的单向解耦依赖拓扑，禁止逆向依赖与跨层越权：

```
┌────────────────────────────────────────────────────────┐
│                      UI Layer                          │
│        (Desktop App Shell / Web Browser / CLI)         │
└───────────────────────────┬────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────┐
│                External Data Layer                     │
│    (~/YvanAI: knowledge, memory, projects, backups)    │
└───────────────────────────▲────────────────────────────┘
                            │ (读写通用 Markdown/JSON)
┌───────────────────────────┴────────────────────────────┐
│                    Skills Layer                        │
│    (memory, knowledge, project, writing, coding)       │
└───────────────────────────┬────────────────────────────┘
                            │ (声明式注册 / ctx.skills)
┌───────────────────────────▼────────────────────────────┐
│                   Profile Layer                        │
│  (cordis.patch.yml: 模型路由, UI偏好, 工具裁剪, 插件管理) │
└───────────────────────────┬────────────────────────────┘
                            │ (Cordis 插件树装配)
┌───────────────────────────▼────────────────────────────┐
│                    DSH Runtime                         │
│  (runtime/deepseek-harness: AgentLoop, Seams, Session) │
└────────────────────────────────────────────────────────┘
```

---

## 二、各层级职责与依赖契约

| 层级 | 物理位置 | 核心职责 | 输入与依赖 | 输出与暴露 |
|---|---|---|---|---|
| **1. DSH Runtime** | `runtime/deepseek-harness` | 提供底层 Agent 执行循环 (AgentLoop)、会话持久化 (Session Log)、能力缝隙 (Capability Seams)、沙箱与子进程调度 | 纯上游源码 (v0.2.1-alpha.2)，无外部项目依赖 | 提供底座运行环境与 Cordis 容器 API |
| **2. Profile Layer** | `profiles/` (映射至 `~/.dsh/profiles/`) | 声明式定制：配置多厂商模型路由 (`llm-pi-ai`)、UI 偏好、工具启停、注入外部插件 | 依赖 DSH Runtime 提供的配置规范 (`cordis.patch.yml`) | 向 Runtime 交付有效的插件装配方案 |
| **3. Skills Layer** | `skills/` (映射至 `~/.dsh/skills/`) | 业务级 Agent 行为指令：五大特种智能体（记忆、知识、项目、写作、开发）的纯 Markdown 提示词契约 | 依赖 Profile 启用的文件操作与会话工具 | 驱动 Agent 对外部数据层进行结构化操作 |
| **4. External Data Layer** | 独立于代码库，固定在 `~/YvanAI/` | 用户的永久核心数字资产：个人知识库、长期决策记忆、项目日志、备份归档 | 接收 Skills Layer 规范化的读写操作 | 通用格式 (Markdown/JSON)，天然支持多工具接入 |
| **5. UI Layer** | `desktop/`（独立桌面应用）、Web 页面或 CLI | 面向用户的操作界面、交互式画布与状态看板 | 经 OmniDesk 自有应用服务/引擎适配层调用 Runtime | 提供人机协同操作入口 |

> **UI 层命名收敛（Phase 2 Step 2）**：早期占位名 `apps/desktop` 已统一为 `desktop/`；
> 桌面技术栈（React + TypeScript UI、Rust + Tauri 2 宿主、Python AI 引擎）与跨层职责见
> [OmniDesk-Desktop-Architecture.md](OmniDesk-Desktop-Architecture.md)。UI **不得**直接调用 DSH ACP，也不得嵌入 DSH Electron Desktop 或 DSH Web 页面。

---

## 三、架构红线（Three Inviolable Constraints）

为了保证整个系统的终身可升级性与数据绝对安全，确立三条绝对禁止事项：

### 1. 绝对禁止修改 DSH Runtime 核心代码
- **红线说明**：`runtime/deepseek-harness/` 目录必须保持与官方发布版本完全一致的只读底座状态。
- **违规行为**：修改 `packages/core/`、直接篡改 `agent-loop` 逻辑、在 runtime 源码内硬编码个人逻辑。
- **合规路径**：所有个性化扩展必须通过 **Profile 补丁 (`cordis.patch.yml`)**、**声明式 Skill (`SKILL.md`)** 或 **标准 MCP 协议** 进行外置装配。

### 2. 绝对禁止将个人知识库放入 DSH 内部
- **红线说明**：知识资产是终身积累的，而 DSH 只是底层的执行引擎。
- **违规行为**：在 `~/.dsh/` 内部新建知识目录，或依赖 DSH 内部私有存储来存放工作文档。
- **合规路径**：知识库一律落脚于 `~/YvanAI/knowledge/`，与 Obsidian、Hugo 静态站点直接互通。DSH 升级、清除缓存或卸载均不伤及知识一毫一厘。

> **知识库目录约定与入库规则**：知识资产的分类、分级（公开 / 内部 / 敏感）与脱敏要求见
> [docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md](OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md)。
> 三级敏感数据（案件材料、案件线索、未公开调查信息、涉案人员信息等）**一律不得进入知识库**，
> 命中后应移出至 `~/OmniDesk/quarantine/` 并改用单位批准的合规系统处理。

### 3. 绝对禁止将用户个人数据放入 Git 项目代码库
- **红线说明**：本项目仓库只管理**框架、配置模板、Agent 技能与自动化工具代码**，绝不追踪用户的个人动态数据。
- **违规行为**：将个人会话日志、敏感 API Key、日常备忘直接提交至 `MyWorkStation` Git 仓库。
- **合规路径**：
  - 动态用户数据统一由 `~/YvanAI/` 托管；
  - 敏感凭证存放在 `~/.dsh/.credentials.yaml`，由 `.gitignore` 严格阻断；
  - 本仓库只保留 `data-template/` 目录作为初始化的目录骨架与说明；真实数据由 `scripts/init-yvan-data.sh` 生成在仓库之外的 `~/YvanAI/`。

---

## 四、升级防灾与解耦验证机制

1. **上游升级安全策略**：
   - 当 DSH 官方发布新版本（如 v0.2.2 或正式版）时，只需在 `runtime/deepseek-harness` 切换对应 Git Tag，或者在独立测试 Profile (`yvan-work`) 中先行校验。
   - `cordis.patch.yml` 和 `skills/` 与源码完全分离，即使重装 DSH，只需执行 `scripts/sync-skills.sh` 即可瞬间还原全部工作流。
2. **灾难恢复验证**：
   - 依赖 `scripts/backup-sessions.sh` 定期将会话与 workspace 索引转储至 `~/YvanAI/backups/`，对抗 Electron 进程意外退出导致的会话丢失风险。
