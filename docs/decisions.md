# OmniDesk 架构决策记录

> 这里只登记已确定的决策及仍待裁决的冲突；候选方案、实施状态与审查结论分别记录在 PLANS.md、STATUS.md、REVIEW.md。

## 已确定

### D-012：DSH 会话恢复与 OmniDesk 任务恢复是不同能力
- 状态：已确定的设计边界；具体锁定版本能力须逐版本验证。
- 决策：分别描述历史可读、恢复既有 DSH session、确认中断任务状态、从断点继续任务；不能将 ACP session resume 等同于任务续跑。没有可靠终态证据时不得自动重放或伪报成功。
- 理由：现有 OmniDesk 适配器代码将非终态任务恢复为 unknown，明确不自动重放；ACP 恢复会话不能证明中断轮次结果。
- 影响：会话持久化设计须注明恢复保证、丢失窗口、事件完整性和版本兼容边界。
- 来源：engine/omnidesk_engine/acp_adapter.py、docs/OmniDesk-Engine-Interface-Test-Matrix.md。

### D-000：个人 MVP 先经独立审查，再进入试用
- 状态：OD-BUILD-002 在其范围内复验通过；完整 Tauri 工作路径待复核。
- 决策：作者自用不以 20% 节时为前置条件，但 MVP 必须通过关键代码边界审查及必要运行路径复核后，才进入严格受限的个人试用。OD-ARCH-001 的 R1–R3 与 OD-BUILD-002 补充的 R4 已有范围性通过结论；真实 Tauri 纵向路径、真实模型解释质量及授权适用范围仍分别受限或未验证。
- 影响：真实 Tauri 任务路径复核及适用授权边界明确前，不将现有 MVP 描述为可用于真实工作材料的日常工具。
- 来源：REVIEW.md、GOALS.md。

### D-001：OmniDesk 是独立个人工作台
- 状态：已确定。
- 决策：OmniDesk 有自己的任务体验、领域规则、数据组织与成果管理；不是 DSH 套壳、换皮或复制品。
- 理由：产品价值在于把专业工作组织成可靠、可复核的流程。
- 影响：DSH 的 UI、私有数据结构和运行时内部实现不得成为 OmniDesk 的产品界面或领域契约。
- 来源：GOALS.md 与归档的历史产品路线材料。

### D-002：DSH 是可替换的执行引擎依赖，Runtime 保持不变
- 状态：已确定。
- 决策：早期可以通过 OmniDesk 自有适配边界复用 DSH；不修改 runtime/ 内 DSH Runtime，不长期维护 Fork。
- 影响：任务、成果与领域规则应可独立于 DSH 保存和迁移。
- 来源：AGENTS.md、GOALS.md、docs/OmniDesk-Desktop-Architecture.md。

### D-003：当前复用最小任务外壳，单一试点验证
- 状态：已确定为当前实现方向；关键代码边界已有分项通过结论，完整 Tauri 使用路径待复核后再判断是否进入受限个人试用。
- 决策：共用任务生命周期；任务差异由各自模板/规则表达。先验证“脱敏数据分析与生态研判”；不建设通用 Agent 平台、工作流编辑器、Registry 或插件市场。
- 影响：周报、材料写作、项目管理目前不能因共用外壳而视为已实现或已获准处理真实数据。
- 来源：docs/OmniDesk-Task-Shell.md 与 tests/reports/personal-mvp-vertical-slice-report.md；实现须经 REVIEW.md 独立审查。

### D-004：安全与用途授权从严解释
- 状态：原则已确定；具体许可范围按用户提供的答复摘要记录。
- 决策：不得扩大推定许可。当前答复仅覆盖满足其脱敏条件、用于数据分析与生态研判的个人分析使用，并明确不得分发；其他用途、端点、工具、设备或环境按证据逐项判断。
- 影响：脱敏、提示词、启发式扫描不等于授权或代码强制控制。
- 来源：用户在本任务中的答复及 docs/OmniDesk-Authorization-and-DataFlow-Gap-Audit.md。仓库未声称持有授权原件。

### D-005：个人使用先于产品化量化门槛
- 状态：已确定。
- 决策：20% 净节省率不是作者开始个人使用的前提；使用中低负担收集体验和返工信息。扩大试用或产品化另作决策。
- 来源：GOALS.md 与用户确定的个人先试用原则。

### D-006：工作区差异必须保全
- 状态：持续有效。
- 决策：任何任务先记录 Git 状态；不重置、清理、覆盖或擅自提交用户/BUILDER 的已有改动。
- 来源：用户明确工作规则与 AGENTS.md。

### D-007：任务外壳可复用，专业能力分别界定
- 状态：已确定为设计原则，后续能力仍需分别实现和验收。
- 决策：周报、材料撰写、日常项目管理可共享基本任务生命周期、状态呈现与输入输出组织；领域规则、权限、模板、评测和授权须独立定义。当前只实现脱敏数据分析与生态研判试点。
- 影响：共用骨架不代表其他任务类型可执行，也不扩大用户转述的许可范围。
- 来源：GOALS.md、用户确认的产品范围。

### D-008：Skill 与 MCP 分工，适配层保持最小
- 状态：已确定。
- 决策：Skill 定义专业流程、规则、输入输出和质量检查；MCP 只承接确有需要且已获准的外部工具接入；Engine Adapter 负责最小任务生命周期与事件转换，不扩展为通用通信平台。
- 影响：MCP 细节不得扩散进领域工作流；不因协议演进预建通用层。
- 来源：用户确认的 OmniDesk 架构原则；GOALS.md。

### D-009：文件优先，数据库由实际瓶颈触发评估
- 状态：已确定。
- 决策：当前以 Markdown、JSON/YAML 与 Git 保存可版本化资产；仅在并发写入冲突、状态查询瓶颈或文件检索评测优化后仍不达标时评估 SQLite。向量库、图数据库不提前引入。
- 影响：达到触发条件只启动方案评估，不自动批准迁移或采用数据库。
- 来源：用户确认的 OmniDesk 架构原则；GOALS.md。

### D-010：产品化由收益、质量、意愿和痛点联合触发
- 状态：已确定为进入 Tauri 评估的门槛；当前尚未满足或验证。
- 决策：净节省工时超过 20%、质量与合规指标达标、至少一位同事明确愿意实际使用、且存在桌面应用可解决的真实痛点，四项需同时满足后才进入产品化评估；满足后仍需另行批准。
- 影响：个人 MVP 可先在审查通过和许可边界内自用，不以 20% 为前置条件；产品化门槛不扩大数据授权。
- 来源：用户确认的产品路线与 GOALS.md。

### D-013：会话存储按原始 cwd 分组，工作区/ACP 按 realpath；OmniDesk 必须规范化 cwd
- 状态：已确定（OD-ARCH-003 锁定版本源码证实）。
- 决策：DSH 会话**存储**按**原始 cwd 字符串**分组（`session-persistence-jsonl/format.ts` 的 `projectKey`），而**工作区注册表**与 **ACP `session/list`/`session/resume`** 按 `fs.realpath` 规范化后的路径分组。同一物理目录的别名写法与真实路径写法会产生两个存储项目目录，但共享同一工作区身份。OmniDesk 在向引擎提交或恢复任务前**必须对 cwd 做 realpath 规范化**，使存储分组与工作区/ACP 身份一致。
- 理由：这是“磁盘上有会话、界面里看不到/找不到”的已证实根因之一；修复只需规范化路径，**无需复制对话正文**。
- 影响：引擎适配层需要单一 cwd 规范化落点；不可解析路径按现有契约明确报错，不静默退化为字符串相等。跨写法 resume 本身由 DSH 的 realpath canon 允许，但分组仍会分裂。
- 来源：OD-ARCH-003（REVIEW.md 顶部同名章节），`packages/session/session-persistence-jsonl/src/format.ts`、`packages/workspace/workspace/src/{index,paths}.ts`、`packages/acp/acp/src/index.ts`。

### D-014：原生会话格式为已发布格式，DSH 内部状态不可当作可丢弃
- 状态：已确定（锁定版本文件事实）。
- 决策：锁定版本的会话格式 V4 已是**已发布**格式（`latestReleasedVersion: 4`、`evidenceTag: dsh-v0.2.0-rc.2`）。该版本文档明确写明 alpha/beta/rc 产品发布即建立已发布格式义务，GitHub 的 prerelease 标记并不会让持久化用户数据变成可丢弃。因此 v0.4 中“DSH 内部状态可丢弃”的表述不成立，隔离运行根中的会话属于不可随意丢弃的数据。
- 理由：已发布格式承担兼容义务；把原生状态当一次性缓存会低估迁移与丢失风险。
- 影响：任何迁移、覆盖、删除隔离运行根的会话都须单独评估并获用户明确授权；不得据“内部状态可丢弃”简化备份与保留策略。
- 来源：OD-ARCH-003；`runtime/deepseek-harness/docs/session-format-status.md`。

### D-015：恢复语义四分解，禁止无证据自动重放
- 状态：已确定（OD-ARCH-003 隔离脚本运行证实）。
- 决策：严格区分“历史可读”“恢复既有 DSH session”“确认中断任务状态”“从断点续跑”四种能力。原生 resume **只收束**中断轮次（补写 `step/end`、`turn/end`、`session/end-seed`），**不续跑**未完成轮次；中断轮次不留下 assistant 文本。没有可靠终态证据时不得自动重放或伪报成功。
- 理由：隔离一次性合成验证显示收束记录在 resume 时刻生成，而非中断时刻；`recover_task` 对非终态标 `unknown` 且不重放。
- 影响：会话持久化设计须注明恢复保证、丢失窗口、事件完整性与版本兼容边界。这是对 D-012 的锁定版本证据补强。
- 来源：OD-ARCH-003（`/tmp/arch003/out/restart-recovery.json` 等一次性运行输出）；`engine/omnidesk_engine/acp_adapter.py`。

### D-016：Rust 桌面宿主默认使用 ACP Profile
- 状态：已实施于当前未提交工作区；ARCHITECT 独立复核通过（计划范围，见 REVIEW.md）。宿主默认传递已由 `EngineConfig::from_env()` 直接单测证明。
- 决策：Rust EngineSupervisor 子进程默认 Profile 为 `acp`；显式 `OMNIDESK_DSH_PROFILE` 仍可覆盖。Web 开发入口使用的 `yvan-work` 是另一条路径，不作为 ACP 子进程默认值。
- 理由：锁定的 `yvan-work` 是 Web bundle，不能响应宿主预期的 ACP stdio 握手。
- 影响：需保持默认值测试经过 `EngineConfig::from_env()`、`child_env()` 和 `initialize_params()`；不能只凭引擎自身默认值或手动注入环境变量验收。
- 来源：OD-BUILD-003 实现与 REVIEW.md 独立审查；补齐计划见 PLANS.md OD-BUILD-004。

### D-017：端点准入暂无宿主可核实事实，保持默认拒绝
- 状态：已确定（OD-ARCH-009 源码审计结论）；实现方案待批准。
- 决策：在存在宿主可核验的端点、服务商、设备、运行环境与用途书面证据之前，设计与验收要求是默认拒绝；仅隔离 `DSH_HOME` 不构成端点隔离，脱敏与提示词也不构成准入。**该拒绝要求尚未在主工作区的所有入口核实实现**（OD-ARCH-009 独立复核）；因此不得把它当作已落实的代码保护。
- 理由：`EngineChannel::spawn` 与 `serve.py` 均继承完整宿主环境，`DEEPSEEK_BASE_URL`、凭证与代理变量可改变数据去向；`OMNIDESK_DSH_PATCHES` 等普通环境变量可注入 provider/baseURL；DSH 默认挂载 web、bash、fs 与遥测通道。
- 影响：未来若实现，须在宿主构成白名单子进程环境，校验实际生效端点与 patch 内容，合成模式限定回环且不得同时持有真实凭证；无有效证据即拒绝启动。评估方案不等于批准实现。
- 来源：OD-ARCH-009（STATUS.md、REVIEW.md），`desktop/src-tauri/src/engine_ipc.rs`、`engine_supervisor.rs`、`engine/omnidesk_engine/serve.py`、`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-base/cordis.patch.yml`。

### D-018：新成果默认落 OmniDesk 数据区，旧 YvanAI 成果保持原样- 状态：已实施于当前未提交工作区；主体代码审查及 OD-ARCH-012 失败清理补充验证均已通过（见 REVIEW.md）。
- 决策：桌面新生成的离线成果与分析成果默认写入 `~/OmniDesk/workspace/artifacts`；`OMNIDESK_LOCAL_DATA_ROOT` 仍可显式覆盖，供隔离测试使用。旧 `~/YvanAI/projects/Omnidesk/analysis` 仅作为历史位置，其中的既有文件**不读取正文、不迁移、不改名、不覆盖**。
- 理由：此前默认值指向旧 YvanAI 路径，会让新成果与旧试点数据混在一处，既难说明归属，也让「迁移」与「继续使用」产生歧义。改为明确的 OmniDesk 位置，同时不动既有文件，是最小且可回退的边界。
- 影响：**不构成资产统一已完成。** 旧目录仍可能持有真实成果，是否迁移须另行评估并经用户明确授权。两份保存路径共用同一个写入函数：`create_new` 防覆盖，写入/刷盘失败时删除本次半成品。
- 来源：OD-ARCH-011；`desktop/src-tauri/src/task_shell.rs`（`DEFAULT_LOCAL_SUBDIR`、`resolve_local_data_root`、`write_new_file_durable`）及对应单元测试。

### D-019：对话为中枢；三类场景由模型起草，代码只做确定性与边界
- 状态：产品方向已确定（OD-ARCH-026，用户已批准执行）；实现位于当前未提交工作区，待 ARCHITECT 独立复核。
- 决策：首页以模型对话为主要工作入口，左侧悬浮图标进入周报与随记、文案写作、项目跟踪。三类场景共享同一交互骨架与任务生命周期，各自由**可编辑、带版本**的流程包（`omnidesk.offline-flows.v1`）规则驱动起草。内容整理与提炼由模型完成；代码只负责权限、任务状态、文件读写、来源记录、日期/状态计数/逾期判定等确定性工作与文件边界控制。成果只能来自模型初稿并经人工确认后保存，不得用本地模板拼接冒充模型产出；确定性输出单独成栏、标注「代码计算」，不作为成果。模型权限首次严格关闭，用户可按自身授权在设置中逐项开启，由宿主在提交时强制执行。
- 理由：OD-ARCH-025 独立复核判定旧形态（首页仪表盘 + 本地确定性汇总）不是用户需要的 AI 工作台，且本地汇总不能冒充模型产出。
- 影响：材料只读用户显式给出的**单个**文件（绝对路径、扩展名白名单、大小上限、UTF-8），不扫描目录、不递归、不解析数据库；流程包新增的 `draft.rules/sections` 为可选字段，旧包缺省时回落到内置默认规则，不可关闭的写作规则（不得虚构、禁入案件材料、待确认等）不由流程包控制。模型权限判定不扩大 D-004 所记录的许可范围，也不等于端点隔离证明。
- 来源：用户批准的 OD-ARCH-026（PLANS.md）；GOALS.md；`desktop/src/shell/HomeChat.tsx`、`desktop/src/OfflineFlows.tsx`、`desktop/src/aiDraft.ts`、`desktop/src-tauri/src/task_shell.rs`、`desktop/src-tauri/src/flow_pack.rs`。

### D-020：窗口外框一体化，并与确认的首页共同作为 UI 基线
- 状态：已实施并经用户现场验收（2026-10-19）；未提交、未推送、未打标签。
- 决策：主窗口采用 Tauri frameless（`decorations: false`），最小化/最大化/关闭由顶栏自绘控件承担；顶栏作为拖动区；用 8 个边缘/角落热区调用 `startResizeDragging` 补回缩放；窗口相关能力按最小集授予（minimize / toggle-maximize / close / is-maximized / start-dragging / start-resize-dragging / internal-toggle-maximize）。该外框与当前“对话中枢”首页共同构成**已确认 UI 基线**（GOALS.md）。
- 理由：用户要求系统标题栏融入软件，而不是保留系统外框；无边框后窗口管理器不再提供边框缩放，必须显式补回，否则会退化成不可缩放窗口。
- 影响：`decorations` 与权限编译在 Rust 二进制中，**改动必须重建并完整重启，前端 HMR 不生效**；Xvfb 无窗口管理器，不能证明系统装饰消失，真实桌面效果只能由用户本机会话验收。回退路径：`decorations` 改回 `true`、移除顶栏控件与缩放热区后重建。用户已确认重启后系统标题栏消失、拖动与— □ × 均正常；双击最大化、边缘缩放、最大化图标切换未被逐项点名验证。
- 来源：用户批准与验收记录（PLANS.md、STATUS.md、REVIEW.md）；`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/capabilities/default.json`、`desktop/src/shell/WindowChrome.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/App.tsx`。

### D-021：项目级固定双 Agent——超级Plan（规划）与超级劳工（执行）
- 状态：已实施于当前未提交工作区（2026-10-19，用户明确要求并逐项指定模型）；未提交、未推送、未打标签。
- 决策：在仓库内以 OpenCode V2 项目级 agent 固定两个角色，均不设 `default_agent`、不改全局模型路由与凭证。**超级Plan**（ID `super-plan`，model `opencode/glm-5.3-flash-free`）负责只读调研代码库与文档、需求分类、产出计划写入 `PLANS.md`，经用户明确批准后派工；权限为「默认全拒后逐条放行」：read/grep/glob、仅 `PLANS.md` 的 edit、只读 git 命令、仅 `super-worker` 的 subagent，并对 `.env*`/`*credentials*`/`*secret*`/`*.pem`/`*.key` 追加 read deny。**超级劳工**（ID `super-worker`，model `opencode/deepseek-v4.1-flash-free`）负责在获批范围内实施，权限等同 build 默认，边界靠 prompt 红线约束（范围刚性、禁改 `runtime/`、禁读凭证、禁触真实业务材料、开工记快照、执行记录写 `STATUS.md`、完成即停等待复核）。
- 理由：把「规划」与「执行」分成两个可识别、模型固定、权限分层的角色，使计划产出与实施证据可分别审查，降低单会话既规划又动手的越界风险；模型按用户指定分工（规划用 GLM-5.3-Flash Free，执行用 DeepSeek V4.1 Flash Free），不使用会话继承模型。
- 影响：两 agent 的 system prompt 内嵌 AGENTS.md 纪律，形成**配置级**防线而非代码级安全控制；super-worker 保留编辑与 shell 能力，若出现越界应先加权限白名单。规划方唯一可写文件是 `PLANS.md`，因此计划登记与状态更新仍由 ARCHITECT 落盘。当前已验证：两 agent 可被发现加载、super-plan 只读与单条只读 git 可用、未放行命令被拒、未越界写入、frontmatter 静态解析通过；**挂起未验证**（GLM/DeepSeek 限流，用户 2026-10-19 指示暂缓测试）：`git rev-parse*` 补入后复测、管道/复合命令匹配语义、super-plan→super-worker 派工端到端链路。复审触发条件：任一 agent 出现越界写入、模型绑定被改、或 OpenCode 升级改变 agent 配置语义。
- 来源：用户指令（2026-10-19）；PLANS.md「工具链计划 T-001」；STATUS.md 与 REVIEW.md 同名小节；`.opencode/agents/super-plan.md`、`.opencode/agents/super-worker.md`。

### D-021：三场景模型起草闭环的验收口径与已知边界
- 状态：已确定（OD-ARCH-028，用户 2026-10-19 批准执行；ARCHITECT 复核通过）。
- 决策：产品形态的口径从「三场景由模型起草」推进到「三个场景的完整闭环已在真实窗口被证明」：用户明确选择材料 → 模型起草 → 人工编辑 → 人工确认 → 写到仓库外数据目录（不覆盖）→ 可读回。**失败与取消不得产生任何成果文件，也不得显示模型正文。** 材料读取的白名单是**扩展名级**（绝对路径、普通文件、md/txt/csv/tsv/json/yaml/log、≤512 KiB、UTF-8、禁 DSH/agent/仓库），**不是内容级**：宿主不检测所选文件里是否含凭证，这一点必须公开为已知边界，不得表述为「已做凭证防护」。
- 理由：OD-ARCH-026 只在「周报」一个场景上点完了界面闭环，写作与项目只有引擎层证据；本轮补齐界面闭环并增加失败/取消负例，避免「三个模块都能用」成为未经验证的推断。
- 影响：真实模型质量、真实 Wayland 桌面、输入法与粘贴仍未验证；试点评价中的**人工澄清数与返工轮数必须由用户首次真实使用时逐次记录**，自动化验收结果（记 0）不得当作体验数据；引擎未上报 usage 时，Token 一律记「未知」，不估算、不累加。
- 来源：OD-ARCH-028 证据索引 `tests/verification/desktop/evidence/od-arch-028/README.md` 与 REVIEW.md 同名小节；`desktop/src/shell/OfflineFlows.tsx`、`desktop/src/aiDraft.ts`、`desktop/src-tauri/src/task_shell.rs`。

## 当前未决事项
- **由模型调用预制 Python 处理表格/文档/数据库、以及一般数据库接入：后置未决。** OD-ARCH-026 只实现三类场景的模型起草闭环，不实现该能力；另行规划与验收。
- **候选方向：个人工作台与 Discipline 能力包分层（OD-ARCH-003 已校准，仍待用户确认产品边界）。** 提议底层优先解决作者个人的会话可找回、升级可控和数据自主；纪检监察领域能力作为可选且按用途单独授权的能力包。OD-ARCH-003 已明确：合规政策必须绑定下层（通用底层同样接触会话正文、配置、插件与备份），且“DSH 内部状态可丢弃”不成立（见 D-014）。方向可作为采纳候选，但**产品边界仍待用户确认**后方可冻结。来源：用户提交的《OmniDesk 综合规划 v0.4》、OD-ARCH-003（REVIEW.md）。
- **独立逐消息追加日志：维持 no-go（暂缓），非阻塞。** OD-ARCH-003 已取得锁定版本/隔离合成证据：原生持久化覆盖历史可读、可列出、可 resume、恢复后可持续写入；真正缺口是可见性（b 类）与路径身份（c 类），均无需复制正文。仅当稳定复现故障显示原生持久化 + 备份 + 元数据索引仍达不到目标，且明确需复制正文时，才重新评估；触发评估不等于自动采用。理由与风险见 REVIEW.md 顶部第 5 节。
- 正式分发产品的最终技术栈仍未冻结。现存 Tauri/Rust/React/TypeScript/Python 工程是技术切片证据，不构成产品化批准；桌面产品化须另行满足门槛并获用户批准。
- ~/OmniDesk（隔离运行根）与 ~/YvanAI（既有外部数据/旧试点路径）在历史实现文档中语义不同。不得直接改名、迁移或推断当前桌面 MVP 已统一使用该布局；具体目录迁移须单独计划。
- 授权范围按用户提供的答复摘要严格解释：个人数据分析与生态研判、符合所述脱敏条件、不得分发。其他用途、模型、工具、设备和运行环境均不由该摘要自动授权。
