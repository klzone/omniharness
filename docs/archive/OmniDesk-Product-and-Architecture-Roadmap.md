# OmniDesk 产品目标与架构路线图

> 归档说明：此为历史路线图快照，不作为当前状态或实施计划。现行产品目标、阶段状态、计划和决策以仓库根目录的 GOALS.md、PLANS.md、STATUS.md、REVIEW.md 与 docs/decisions.md 为准；本归档的部分相对链接可能失效。
> 状态：历史产品方向决策与分阶段路线图
> 日期：2026-10-15
> 本文明确产品终点与验证顺序；不代表目标模块已经实现，也不授权提前铺开完整桌面应用。

## 1. 产品终点

OmniDesk Discipline 的最终产品是**独立的 AI 纪检监察工作助手桌面应用**。它应拥有自己的品牌、导航、工作流、设置、任务与成果管理体验。用户使用的是 OmniDesk 的业务产品，而不是 DSH 官方界面、套壳或换肤版本。

OmniDesk 控制产品交互、领域工作流、用户数据组织和结果呈现。AI 执行引擎是可替换的实现依赖：初期可复用隔离运行的 DSH，后续是否替换由能力、稳定性和维护成本的证据决定。产品必须保留 DSH 故障或替换时可迁移的知识、模板和工作成果。

更远期，用户希望分别探索 AI 短视频制作桌面应用，以及自适应学习、教育跟踪与训练桌面应用。它们先作为独立产品验证各自用户、领域和工作流。只有实际出现稳定且重复的共性需求后，才评估抽取共享组件；当前不建设通用 OmniDesk Core、多产品平台或插件市场。

## 2. 当前决策与未决事项

- **产品方向已决定**：独立桌面应用是 OmniDesk Discipline 的最终目标；当前 DSH Web UI 仅是业务能力与资产验证环境。
- **技术路线是候选，不是冻结选型**：优先验证 Tauri 2 + React + TypeScript，Rust 作为 Tauri 宿主与必要系统能力的候选实现。阶段 2 必须验证 TypeScript UI 与 Rust 命令/服务之间的真实边界，并检验哪些职责确实需要 Rust；不得因为 Tauri 使用 Rust 就预设业务逻辑都要写 Rust。Electron 与 TypeScript/Node 后端保留为比较选项。跨平台运行、安装升级、系统集成和维护成本未完成实测前，不把候选写成最终技术定案。
- **执行引擎边界已决定**：UI 与领域工作流不得直接依赖 DSH 官方页面、内部配置文件或私有前端组件。通过 OmniDesk 自有的引擎适配边界提交任务、接收结果和呈现状态。
- **组件职责边界**：**Skill** 负责流程与专业规则（怎么写、按什么规则检索）；**MCP** 只用于确有需要且已获准接入的外部工具；**Engine Adapter** 只承担最小连接边界，把任务提交、状态、事件、取消与结果规范化，不新造通用协议，也不把 DSH 内部配置结构暴露给 UI。
- **DSH 适配能力待验证**：CLI 启动成功不代表产品级接口完备。任务提交、流式事件、进度、错误、取消、恢复、会话映射及版本兼容必须逐项验证。若上游没有足够稳定的接口，应记录限制并评估最小外部适配方案，不修改 DSH Runtime。
- **数据权威源不变**：知识、模板和用户成果保存在仓库外的独立数据目录，以可迁移文件格式为权威数据。**文件优先**：只有在出现可观察的并发写入冲突、状态查询效率问题，或文件检索优化后仍达不到既定指标时，才评估引入 SQLite；**触发评估不等于自动采用**，SQLite 也不应无理由复制并取代文件型知识资产。Python 仅在 OCR、文档处理或分析等明确任务需要时作为可选组件。

## 3. 分阶段路线

### 阶段 0：锁定产品方向与工作约定（当前完成）

- **动作**：把独立产品终点、现行约束的调整、技术候选和验证顺序写入项目文档。
- **输入**：现行设计原则、Phase 2A/2B 状态、用户确认的产品目标。
- **产物**：本路线图及相关旧文档的指向性修订。
- **验收**：项目文档不再把“永不自研 GUI”作为永久红线；明确当前不一次性开发完整 GUI，DSH Web UI 只用于过渡验证。

### 阶段 1：收口当前产品验证（进行中）

- **已完成**：修正 `scripts/setup.sh --check` 对底座和 Skill 检查失败状态的吞错问题。正常状态下检查通过；模拟底座与 Skill 检查分别失败时，两项失败均被报告且总退出码非零。
- **已完成（真实只读副作用验证）**：在真实环境与一次性沙箱分别执行 `scripts/setup.sh --check`，并对仓库、`~/OmniDesk`、`~/YvanAI`、`~/.dsh`、`~/.agents` 做检查前后元数据快照比对——全部 changed/added/removed 为 0，失败路径不创建缺失的隔离根。证据见 [tests/reports/phase1-verification-report.md](../tests/reports/phase1-verification-report.md) 第 2 节与 [tests/verification/phase1/](../tests/verification/phase1/)（可重复执行）。
- **已完成（隔离 Skill 同步加载）**：用一次性仓库副本与一次性隔离根、经 DSH 真实 Skill 发现在模型调用前验证“改 Skill → 显式同步 → 隔离实例加载新版本”。证据见同报告第 3.4 节；本轮在 [Git HEAD `17efd20` + 未提交改动] 重跑复现通过。
- **已完成（资产可定位性）**：经 DSH 真实 Skill 发现确认 `material-writing` 的 `references/templates`、`references/writing-style`、`references/excellent-cases` 已被正确物化且路径可达（范例库仅 README，无范例）。这只证明“资产可达”，不等于“模型实际读取”。证据见同报告第 3.5 节。
- **已完成（真实模型端到端，Web UI）**：在隔离实例 Web UI 中，真实模型（`opendesign / deepseek-v4.1-flash` · high）用 `material-writing` 技能，对真实合规法规素材（`中国共产党纪律处分条例2023版.docx`）完成了从「加载技能 → 实际读取模板/风格规则/范例目录 → 解析 .docx 正文 → 生成文稿 → 逐条核验」的完整链路。经独立逐字复核：引用框条文 6/6 逐字通过、行内引用全部命中原文、正文与判断分离、如实声明「未使用历史范例」（范例目录确无范文）、标注 6 项待确认、声明未编造数据且不含案件类信息。证据见 [tests/reports/phase2-webui-writing-e2e-report.md](../tests/reports/phase2-webui-writing-e2e-report.md)。
- **已完成（合成夹具场景 1–6，真实模型）**：`run_material_writing_scenarios.py` 已在隔离实例自身凭证、一次性 /tmp 副本下逐项跑完 6 个场景并留证，证据等级为**真实模型输出**：场景 1/2/3/5/6 自动判定通过；场景 4 自动判定 `structure_present` 误报为 False，但人工核对输出确认模型已正确指出矛盾并拒绝二选一（判定为通过）。证据见 [tests/reports/phase1-verification-report.md](../tests/reports/phase1-verification-report.md) 第七节；原始输出在 [tests/verification/phase1/evidence/material-writing-evidence.json](../tests/verification/phase1/evidence/material-writing-evidence.json)。未修改凭证与路由；原版 `~/.dsh` 快照零变化。
- **已完成（P2 知识检索链路验收，仅合成夹具）**：制定知识检索验收契约 [docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md](OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md)，并用**纯合成虚构制度**（`tests/verification/retrieval/fixtures/`）跑通 6 类场景（精确条款 / 同义问法 / 相似条款干扰 / 多文件冲突 / 过期文件 / 无匹配结果）。经逐字核对：**条款级引用未命中 0**、越界读取 0、每个场景交付文本的来源文件与 `tool_call` 实际触碰文件一致。**检索真实性（命题 A，工具调用级）与引用真实性（命题 B，逐字比对）分别验证且均通过**；判定器经真阳性/真阴性自检证明既有牙齿又不过度误报；以 `--rejudge` 不调用模型即确定性复现 6/6。证据见 [tests/reports/retrieval-verification-report.md](../tests/reports/retrieval-verification-report.md)，原始字节证据在 [tests/verification/retrieval/evidence/raw/](../tests/verification/retrieval/evidence/raw/)。**本轮未读取、未复制、未索引 234 份正式制度文件**；其接入仍待另行立项授权。
- **已记录待修问题**：`scripts/sync-skills.sh` 在隔离根 `dsh-home` 存在但 `skills/` 缺失时，rsync 以退出码 11 失败并告警（`--check` 路径不受影响）。本轮以记录为主，未修改同步脚本。
- 校准简报中的状态与历史证据口径；不把未经当前复核的历史报告或 usage-log 数字作为当前产品能力证明。
- **阶段出口**：核心写作流程可复现，已知问题和证据等级明确；真实只读检查经文件快照验证；否则记录阻塞点，不以文档静态检查替代实测。
  - 现状：只读副作用验证**满足**；加载/同步/资产可达链路可复现（场景 7 + 资产可定位性）；真实模型端到端写作已在隔离 Web UI 中跑通并逐字复核（证据见 tests/reports/phase2-webui-writing-e2e-report.md）；**专用合成夹具场景 1–6 已取得真实模型输出并通过**（证据见 tests/reports/phase1-verification-report.md 第七节）；**P2 知识检索链路亦已用纯合成夹具验收闭环**（6/6、引用未命中 0、越界 0，证据见 tests/reports/retrieval-verification-report.md）。阶段 1 出口条件已满足。

### 阶段 2：验证引擎接口与桌面技术边界

**当前进展（2026-10-16）**：P1 第一步「引擎接口只读审计」已完成（[docs/OmniDesk-Engine-Interface-Audit.md](OmniDesk-Engine-Interface-Audit.md)）；P1 第二步「引擎接口契约设计」已完成（[docs/OmniDesk-Engine-Interface-Contract.md](OmniDesk-Engine-Interface-Contract.md)，契约已冻结；测试矩阵 [docs/OmniDesk-Engine-Interface-Test-Matrix.md](OmniDesk-Engine-Interface-Test-Matrix.md) 已由 v0.2 更新为「已执行」）。P1 第三步「最小 ACP 适配器原型」已完成（[tests/reports/p1-engine-adapter-prototype-report.md](../tests/reports/p1-engine-adapter-prototype-report.md)）：新增可替换引擎适配层 `engine/omnidesk_engine/`（零第三方依赖），实现了契约 §4.1–§4.9 的最小接口；E01–E20 测试矩阵已在隔离环境真实运行并留证，**20/20 判定 pass**（证据 `tests/verification/engine-interface/evidence/adapter/E01.json … E20.json`），测试矩阵状态同步更新为 v0.2「已执行」。取消与恢复的真实语义已取得运行级证据：ACP `session/cancel` 生效并以 `stopReason=cancelled` 落定；SIGKILL 中断后重启只能把在途任务标为 `unknown`（不自动重放），未知会话返回标准 `unknown_session`。已知语义缺口：ACP 只在整轮结束一次性投递 assistant 文本（消息级，非 token 级）；`stopReason=end_turn` 无法区分 completed 与 blocked。E18 人格前缀对照的 **script 臂通过**（离线 `--dump-config` 与合成端点标记探针共同证明补丁层覆盖真正进入模型请求），**model 臂阻塞**（进程环境无任何 API 密钥，隔离根凭证按硬约束未读取），记为 `unverified` 并保留复现步骤。**本阶段桌面技术切片（Step 1–18）已完成并留运行证据**（见下方 2026-10-16 条目）。**完整桌面产品（阶段 3）未开始**；正式 234 份制度库接入未授权；发布标签未创建（仅 `discipline-v0.2`）。

**P1-Desktop Phase 2 实际完成范围（2026-10-16，Step 1–18 全部完成）**：详细证据见 [tests/reports/p2-desktop-engine-slice-report.md](../tests/reports/p2-desktop-engine-slice-report.md)。

- **技术栈与边界冻结**（Step 2–4）：Tauri 2.x + Rust 宿主 + React/TypeScript UI + Python 引擎；禁止把 DSH Electron Desktop 作为 OmniDesk 外壳。文档：[docs/OmniDesk-Desktop-Architecture.md](OmniDesk-Desktop-Architecture.md)、[docs/OmniDesk-Desktop-Engine-IPC.md](OmniDesk-Desktop-Engine-IPC.md)。
- **最小工程与自有数据根**（Step 5–6）：`desktop/` 为有效 Tauri 工程；应用数据根默认 `$HOME/OmniDesk/app` 或 `OMNIDESK_APP_DATA_ROOT`，经 `assert_isolated()` 硬拒绝原版 DSH/agent/Desktop 路径与 DSH 源码树。文档：[docs/OmniDesk-Desktop-Data-Layout.md](OmniDesk-Desktop-Data-Layout.md)。
- **宿主实现**（Step 7–11）：Rust 侧 `engine_supervisor.rs`（子进程生命周期）、`engine_ipc.rs`（stdio JSON-RPC 通道）、`engine_bridge.rs`（事件泵），以及最小任务 UI 与取消操作已落地。
- **端到端链路**（Step 12，`script`）：`python3 tests/verification/desktop/run_desktop_e2e.py` 真实跑通「driver → IPC-2 → Python Engine 子进程 → 真实 DSH `bin.js --profile acp` → 本机合成模型端点」；成功/失败/取消三场景全部 pass。证据 `tests/verification/desktop/evidence/task-chain.json`。
- **数据隔离**（Step 13，`script`）：`~/.dsh`、`~/.agents`、`~/.config/DSH Desktop`、`~/.local/share/io.github.hairyf.deepseek-harness-desktop` 四项受守护路径测试前后零差异，测试数据只在 `/tmp`；证据 `isolation-report.json`（`~/.dsh` 快照有界，截断于 4000 项）。
- **独立窗口身份**（Step 14，`script`+`static`）：真实窗口标题 `OmniDesk Discipline`、类 `omnidesk-desktop`、1280×820；截图为 OmniDesk 自有三栏 UI，构建产物无 DSH Desktop UI 入口、不依赖 DSH Web Server；证据 `ui-identity-report.md` + `ui-identity-window.png`。
- **双实例隔离**（Step 15，`script`）：两实例 PID 互异、根目录不相交、A 的任务对 B 返回标准 `unknown_task`；6/6 通过。证据 `dual-instance-report.json`。
- **自动化测试**（Step 16，`script`）：`cargo test` **23/23 通过**（10 单元 + 13 集成）。

**未验证边界（不得写成“已支持”）**：真实模型输出的“桌面外壳”链路未跑通（本轮用合成端点，真实模型端到端仅在 Web UI 路径验证过）；`recover_task` 能力为 `partial`（重启后在途任务标 `unknown`、不自动重放）；ACP 为消息级（非 token 级）流式；`stopReason=end_turn` 无法区分 completed 与 blocked；跨平台（Windows/macOS）未验证；一次「窗口内跑完整链路」的截图渲染为空白窗口（原因未定位，不影响 Step 14 判定）。

**Rust 采用范围建议**：**go**——保留 Tauri 2 + Rust 宿主 + React/TS UI + Python 引擎四层分工；Rust 价值集中在原生进程生命周期与安全边界，代码面保持当前小规模，不把业务逻辑搬进 Rust。
**结论性质（重要，不得省略）**：以上成功、失败、取消与隔离证据只证明这套**可丢弃技术切片**在受控隔离环境下可行；它**不构成**正式产品化批准，也**不构成**对 Tauri/Rust 的最终选型批准（Electron 与 TypeScript/Node 后端仍是比较项）。**当前不扩大桌面功能**，直到满足下方「阶段 2.5」的产品化联合决策门槛。


在材料写作流程与合成验收输入明确后，不等待全部业务功能完成，单独做可丢弃的技术验证；不先构建完整产品桌面壳或通用引擎平台。用一条合成写作任务贯穿最小调用入口、引擎适配器和简单的 TypeScript 界面，分别验证两个边界：OmniDesk 到 DSH 执行引擎，以及 TypeScript UI 到 Tauri/Rust 宿主。该切片的目的既是证明可行，也是在投入正式桌面产品前发现 Rust 是否必要、必要到什么程度。

验证范围分两组：

1. **引擎边界**：独立进程启动和关闭、提交合成任务、输出接收（若有流式能力）、成功/错误状态、取消、会话关联或恢复能力、版本锁定与隔离数据路径。
2. **桌面调用边界**：React/TypeScript 发起任务并接收状态/结果；验证 Tauri 命令调用、事件传递、错误映射、取消请求与文件保存的最小闭环。比较 Rust 在子进程管理、系统文件访问和安全边界上的实际价值与维护成本，并与 TypeScript/Node 后端或 Electron 主进程方案作同场景对照。目标是让 Rust 只承担经验证需要原生宿主的职责；若 Rust 增加跨语言维护成本却未带来所需能力，应调整桌面选型或缩小 Rust 代码面。

所有能力标明“支持 / 不支持 / 未验证”，并记录接口证据和失败行为。同步检查目标操作系统可用性；至少对 Windows 与 Linux 的 Tauri 系统 WebView、文件选择、字体与安装升级风险做实测或明确列为未验证。
**接口边界注意事项**：当前锁定 DSH 版本的公开 CLI 文档已经提供 `headless --json` 一次性任务入口，以及 `sdk` JSON-RPC stdio 服务入口；阶段 2 可比较两条有文档依据的路径，不必假定只能解析交互式 TUI。Headless 事件提供 Session ID、运行事件与最终文本，但最终文本在任务结束时提交、不是 token 级流式输出，JSON 事件是有界投影，取消由外部进程生命周期控制。SDK 能按 Session 推送事件和状态，但其公开协议没有逐任务取消/关闭方法，也不直接给出 prompt 对应的最终结果；需要验证适配器能否安全确定任务完成与取消边界。另可评估 DSH 自带的 ACP v1：它提供 session close、resume/list 与语义更新，需检验是否适合 OmniDesk 的非编码领域工作流及任务验收契约。路线图中的“OmniDesk 自有最小调用入口”仍是可丢弃的隔离技术原型，不要求引擎接口确认前先实现 Rust/Tauri 桌面壳。若需要读取 DSH 私有会话文件才能补足契约，应记录为耦合风险并比较公开协议扩展、外部适配器或替代引擎，不能将私有格式视为稳定接口。

- **产物**：引擎接口能力矩阵、可丢弃的端到端技术切片、Tauri/Rust 与 Electron/TypeScript 主进程的约束/成本对照，以及“Rust 采用范围 + go / revise / stop”建议。
- **验收**：从 TypeScript 界面经已验证的宿主边界提交一份合成写作任务并可靠取得结果；失败可见且不会泄漏到原版 DSH 数据根。报告明确哪些职责需要 Rust、哪些留在 TypeScript、以及取消/恢复等能力的真实限制。没有证明的能力不得写成“已支持”，阶段 2 的原型不得被描述为正式产品实现。

### 阶段 2.5：项目准入与价值评测基线（当前优先级最高）

本阶段不实现新功能，只建立“能不能合法处理真实材料”和“到底值不值得产品化”两条基线。**产品价值目前仍待验证**：
已有的材料写作与知识检索链路都是在**合成/公开素材**上跑通的，只能说明技术与评测流程可行，**不能**证明真实业务收益。

#### 2.5.1 合规准入门槛（书面授权的硬门槛）

处理真实工作材料的准入门槛是**单位出具的书面授权**。授权必须**逐项取得、逐项记录**，**不得假定已获批准**。
**材料授权不能代替模型、工具或运行环境授权**；**脱敏本身也不代表获准**（脱敏解决的是“数据是否敏感”，授权解决的是“是否允许使用”）。

未取得相应书面授权前，**不得**把真实工作材料或内部文件接入个人 AI 工作台。允许的准备工作仅限：**合成数据**、**许可范围内的公开资料**、以及**经明确批准的数据**。

| 授权项 | 授权对象 | 依据 | 当前状态 |
| --- | --- | --- | --- |
| 数据/材料 | 真实工作材料、内部文件能否进入个人 AI 系统并交由所选模型处理 | 单位书面授权 | **未获** |
| 模型/服务商 | 所选云模型与服务商是否被允许接触对应分级数据 | 单位书面授权 | **未获** |
| 工具 | MCP/插件/外部工具是否被允许接入与调用 | 单位书面授权 | **未获** |
| 个人设备 | 在个人电脑上处理对应分级数据是否被允许 | 单位书面授权 | **未获** |
| 运行环境 | 隔离运行根、数据落盘与日志留存方式是否被认可 | 单位书面授权 | **未获** |

> 状态只能由用户提供的书面授权文件改写；在取得之前保持“未获”。任何一项未获授权，只允许在合成/公开/已批准数据上工作。

#### 2.5.2 价值评测协议

评测协议见 [docs/OmniDesk-Value-Evaluation-Protocol.md](OmniDesk-Value-Evaluation-Protocol.md)（草案）。其要点：任务分层与选择、人工基准工时、
AI 辅助**总**工时（含准备、等待、核验、返工与修正）、质量与合规**硬门槛**、失败与遗漏记录、人工介入点、模型成本与维护成本，
并**预先**定义评分规则与复现条件。
**基线任务集口径修正**：原「20–30 个真实场景脱敏任务」的表述已作废——基准任务只能用**合成、公开或经明确批准**的数据；
未获授权时，评测结果只能说明**技术与评测流程**表现，**不能**宣称真实业务收益。

#### 2.5.3 能力范围限定

**优先限定在已有能力**：材料写作与知识检索。**不**为了凑评测数量而宣称会议整理等未验证工作流“已具备”。
未经验证的能力在文档与评测中一律标为“未验证”，不得计入可用能力清单。

#### 2.5.4 产品化联合决策门槛（全部满足才进入阶段 3）

1. 可复现的**净节省率 > 20%**（按价值评测协议计算，含准备、等待、核验与返工）；
2. **质量与合规指标达标**（含硬门槛，一票否决）；
3. **至少一位同事**愿意在**获准条件**下实际试用；
4. 存在**由桌面应用能解决**的具体痛点（而非仅模型或脚本即可满足）。

满足上述门槛后，才评估 Tauri / Electron 等**产品形态**；**门槛满足不等于自动启动开发**，仍需用户明确批准。
门槛未满足前，阶段 3 不启动。**该门槛不约束作者个人自用**：个人自用可先按「阶段 2.6」推进，使用中先收集低负担的体验与返工记录；门槛仍只决定是否**产品化**。

### 阶段 2.6：个人自用 MVP —— 最小纵向切片（已完成，2026-10-16）

**定位**：把已有桌面技术切片推进为**作者本人可实际操作的最小工作流**，服务**单一用户、个人使用、不分发**。它**不是**阶段 3「产品化」，也不改变阶段 3 的联合决策门槛。

**范围调整**：对**作者个人自用**，**不以「净节省率 > 20%」作为开始使用的前置条件**，使用中先收集**低负担**的体验与返工记录。2.5.4 的联合决策门槛仍**只约束阶段 3 的产品化**；2.5.1 的授权准入**不因本阶段放宽**。

**已完成（实现 + 运行证据）**：
- **共用任务外壳** `omnidesk-task-shell/1`：`task_type / input / stages / result / confirmation / persist` 六字段；任务差异由独立模板/规则表达。**未**实现通用 Agent 平台、工作流编辑器、Registry 或插件体系。文档：[docs/OmniDesk-Task-Shell.md](OmniDesk-Task-Shell.md)。
- **单一纵向试点：脱敏数据分析与生态研判**（`desensitized_analysis`，已实现）；`weekly_report` / `material_writing` / `project_management` 仅保留为 `reserved` **任务类型边界**，**不可执行**。
- **确定性统计由本地代码计算**（`desktop/src-tauri/src/task_shell.rs`，零新增第三方依赖）：分组计数 / 总和 / 均值 / 最小 / 最大 + 稳定指纹；**模型只负责文字解释，不得重算或编造数值**（`frozen_stats_block` + 冻结块与指纹校验）。
- **五段式 UI**（`desktop/src/workbench.tsx`）：选任务类型 → 载入**合成样例**或粘贴输入 → 运行确定性统计 → 生成解释提示词 → 人工确认勾选 + 备注 → 保存到**仓库外** → 轻量使用记录。
- **结果落盘在仓库外**：默认 `~/YvanAI/projects/Omnidesk/analysis`（`OMNIDESK_LOCAL_DATA_ROOT` 可覆盖）；文件名含时间戳与前 12 位指纹；`create_new` 拒绝覆盖已有文件。
- **使用记录不含正文**：仅任务类型、是否完成、是否可用、返工类别、障碍、可选粗略耗时区间；支持逐条删除与清空；**不**默认保存完整提示、输入或输出。
- **数据边界（真实强度）**：写入前做路径校验，硬拒绝原版 DSH / agent / Codex 路径、DSH 源码树与仓库路径；输入侧做红线关键词与证件 / 电话号数字扫描。**该扫描是启发式提示，不是合规控制**，不能替代 2.5.1 的书面授权。

**未完成边界（不得写成“已支持”）**：
- **模型阶段为 `unverified`**：本轮进程环境**无任何 API 密钥**，未读取隔离根凭证，因此「模型是否只解释不重算」「解释文字是否超出证据」**未经真实模型验证**；只验证了提示词内确实含冻结块与指纹。
- 真实 Tauri 窗口在无 GPU 的虚拟显示下渲染为空白（沿袭阶段 2 已记录的限制）；本轮以 vite 开发服务器截图确认**前端**渲染正常，**不等于**已对真实窗口完成视觉验收。
- 周报、通用材料撰写、完整项目管理**本阶段未实现**，其真实材料准入状态**不能由本次实现推断**。

**真实使用的下一步**：在 2.5.1 授权逐项落实后，用**已获准**数据走一次同一工作流，并据轻量使用记录决定是否继续。

### 阶段 3：独立桌面纵向切片

只有满足「阶段 2.5 产品化联合决策门槛」（含净节省率 > 20%、质量合规达标、同事愿意在获准条件下试用、存在桌面应用能解决的具体痛点）后，才构建小而完整的自有桌面流程：OmniDesk 自有窗口和导航、一个材料写作任务、任务状态/错误呈现、结果预览与保存到独立数据目录。优先验证 Tauri 2 + React + TypeScript + Rust；若跨平台或接口证据不支持，再以同一验收场景比较 Electron。桌面 UI 调用 OmniDesk 自己定义的应用服务契约；由后端适配器负责与 DSH 通信，不让 React 界面解析 CLI 私有输出格式或读取 DSH 数据文件。

- **不做**：一次性重写全部 Skill、复制 DSH 全部功能、通用工作流引擎、共享 Core、完整多产品平台。
- **验收**：用户能在 OmniDesk 自有界面完成一个合成写作工作流；成果由 OmniDesk 以可迁移格式保存；更换或停止 DSH 不会使已保存的成果不可读。

### 阶段 4：按证据扩展产品

依据用户实际使用逐项扩展知识检索、文档审查、会议整理和成果管理。每项能力先定义来源、权限、结果格式与测试，再决定是否进入桌面产品。AI 短视频制作与自适应学习、教育跟踪和训练分别立项、分别验证用户需求与核心工作流；在这些产品各自形成可运行切片并出现重复的实际工程需求之前，不决定共享产品核心、领域模型或多产品平台。之后仅对证据显示稳定重复、边界清晰且能降低总维护成本的基础设施评估共享；允许复用通用库或工程工具，不因此强行统一各产品的 UI、领域工作流、数据模型或发布节奏。

## 4. 免费 GPT 与仓库执行职责

用户使用的免费 GPT 聊天无法读取本仓库或执行验证，因此定位为**讨论与计划助手**：可以针对用户粘贴的产品目标、有限文件摘录和 Codex 提供的审计摘要提出问题、比较方案或起草计划；它不能确认仓库当前状态、声称代码已实现、声称测试通过或替用户验收。

仓库内的实际工作由可读取项目的 Codex 环境执行：检查 Git 与文件事实、实现已批准的文档或代码变更、运行适当验证、记录结果与限制。免费 GPT 提出的计划作为建议输入，由 Codex 对照真实仓库、项目原则和可运行证据后再采纳。不得向免费 GPT 提供凭证、真实案件材料或敏感工作数据。

给免费 GPT 的任务应明确写成“基于以下已核实摘要提出计划/审阅方案”，并提供所需事实摘要；不要要求它“检查仓库”“运行测试”或把它的判断当作验证证据。

## 5. 计划变更与项目原则

本路线图仅调整“永久不开发 GUI”这一方向：它被“最终建设自有桌面产品、当前分阶段验证”取代。禁止修改 DSH Runtime、禁止敏感数据进入个人 AI 系统、数据资产独立于运行环境、先验证单一领域工作流、暂不建设通用平台等原则继续有效。

技术候选、阶段计划或架构图均不等于实现状态。每次阶段转移都需由实际文件、运行记录和验收结果支持。
