# OmniDesk 下一阶段规划交接简报 (Next-Phase Brief)

> 用途：本文件是 OmniDesk 项目在 Phase 2B 提交完成后的**状态快照与规划输入**。
> 归档说明：本文件是历史状态快照，不作为当前状态或实施计划。现行信息以仓库根目录的共享状态文件为准；本归档的部分相对链接可能失效。
> 产品终点和阶段路线以 [OmniDesk-Product-and-Architecture-Roadmap.md](OmniDesk-Product-and-Architecture-Roadmap.md) 为准。
>
> 更新：2026-10-16；Phase 2B 实现基线：dab3d78；当前文档提交：17efd20（后续更新以 Git HEAD 为准）。

---

## 一、当前基线（均为已验证事实）

### 1.1 仓库与运行底座

| 项 | 值 |
| --- | --- |
| 仓库 | git@github.com:klzone/omniharness.git（分支 main，已推送） |
| Phase 2B 实现基线 | dab3d78 |
| 本简报初始版本提交 | 17efd20（此后若有提交，以当前 Git HEAD 为准） |
| 版本号 | VERSION = 0.2.1 |
| 锁定底座 | DSH `dsh-v0.2.1-alpha.2`，commit `d7432673` |
| 底座源码 | `runtime/deepseek-harness` 子模块（.gitmodules 固定到该 tag） |
| 可运行 CLI | `scripts/bootstrap-runtime.sh` 装入 `runtime/dsh-runtime/`（已 gitignore） |
| 一键初始化 | `scripts/setup.sh`（底座 -> 数据骨架 -> 技能同步） |

**「下载即用」已成立**：克隆仓库后执行 `scripts/setup.sh`，即可得到与维护者实机验证一致的同一底座版本；
不追「最新版」，也不需要另行到官网下载安装包。

### 1.2 运行隔离（本项目最关键的架构承诺）

| 路径 | 角色 | 说明 |
| --- | --- | --- |
| `~/OmniDesk/` | 隔离运行根 | 含 `runtime/dsh-home/`（技能、Profile 的运行时副本） |
| `~/YvanAI/` | 外部数据中枢 | 知识、记忆、项目、模板、备份（与运行层解耦） |
| `~/.dsh`、`~/.agents` | **原版 DSH，绝不触碰** | 用户日常使用的 DSH 数据 |

隔离自检脚本 `scripts/omnidesk-isolation-check.sh`：**通过 37 项，失败 0 项**，
并在每轮自检前后确认原版 `~/.dsh`、`~/.agents` 零变化。

### 1.3 内容资产现状

| 资产 | 唯一权威源 | 现状 |
| --- | --- | --- |
| Skill 正文 | `skills/<name>/SKILL.md` | **6 个**：material-writing、document-review、knowledge-search、knowledge-agent、memory-agent、project-agent |
| 材料模板 | `templates/discipline/` | 3 份：工作总结、调研/汇报报告、交流发言 |
| 写作风格规则 | `knowledge/writing-style/` | style-guide（14 条铁律）、common-phrases、structure-patterns、avoid-list |
| 历史范例 | `knowledge/excellent-cases/` | **仅有 README，尚无任何范例文件**（已如实标注） |
| 法规制度库 | `knowledge/regulations/` | 真实 `.wps`/`.docx` 文件在本地，已被 `.gitignore` 排除，**不入库** |
| 其他知识分类 | `knowledge/{policies,meeting-materials,methodology}/` | 仅 README 说明 |
| 生效 Profile | `profiles/yvan-work/` | 生效配置源（`cordis.patch.yml` + `package.json`） |
| 规划声明 | `workstations/discipline/profile.yaml` | **非生效配置**，仅为领域规划，已加头注 |

### 1.4 测试基准现状

- `tests/golden_tasks/tasks.json`：**已含 40 个任务**（GT-01 ~ GT-40），覆盖材料写作、法规检索、文稿审查、
  讲话稿、调研报告、PPT 提纲、学习心得、document-review 共 8 类。Phase 3 的「扩充到 40 个」目标**已达成**。
- **任务口径说明**：上述 40 项是**基准任务**（合成/公开口径的场景描述），**不是**真实工作材料的脱敏副本。
  在未取得单位书面授权前，其结果只代表**技术与评测流程**表现，不得宣称真实业务收益。
- `tests/reports/citation-audit-report.md`：声称 30/30 抽查、96.7% 准确率。
- `workstations/discipline/memory/usage-log/README.md`：声称 20 次真实任务、平均效率提升 87.1%。

> ⚠️ 上述两份报告均产生于隔离验证**之前**，未在隔离环境中复核，**不应作为当前证据引用**。

### 1.5 Phase 1 收口验证（当前只读检查与隔离加载已复核）

- 报告：`tests/reports/phase1-verification-report.md`；可重复脚本与证据：`tests/verification/phase1/`。
- **只读副作用验证已关闭**：`scripts/setup.sh --check` 正常路径退出码 0；合成失败路径退出码 1 且底座与 Skill
  两项失败均被报告；检查前后对仓库、`~/OmniDesk`、`~/YvanAI`、`~/.dsh`（排除 sessions/）、`~/.agents`
  的元数据快照差异为 0，失败路径不创建缺失的隔离根。
- **隔离 Skill 同步加载已验证**：在一次性副本 + 一次性隔离根中，经 DSH 真实 Skill 发现机制，确认「改 Skill →
  显式同步 → 隔离实例加载新版本正文」（未发生模型推理，属脚本/隔离实测级证据）。
- **资产可定位性已验证（脚本级）**：经 DSH 真实 Skill 发现确认 `material-writing` 的 `references/templates`、
  `references/writing-style`、`references/excellent-cases` 已正确物化且路径可达（范例库仅 README，无范例）。
  这只证明「资产可达」，**不等于**「模型实际读取」。
- **材料写作真实模型端到端生成已完成（Web UI 实测）**：在隔离实例 Web UI 中用真实模型
  （`opendesign / deepseek-v4.1-flash` · high）对真实合规法规素材完成了完整链路（加载技能 → 实际读取
  模板/风格规则/范例目录 → 解析 .docx → 生成 → 逐条核验）。独立复核：引用框条文 6/6 逐字通过、行内引用
  全部命中原文、事实与判断分离、如实声明「未使用历史范例」、标注 6 项待确认、声明未编造数据且不含案件类信息。
  证据见 `tests/reports/phase2-webui-writing-e2e-report.md`。
- **场景 1–6 执行框架已就位**：合成素材 `tests/verification/phase1/fixtures/`（全部虚构占位）+ 执行器
  `tests/verification/phase1/run_material_writing_scenarios.py`；提供凭证后运行即可产出 `model` 级证据。
- 本轮在 [Git HEAD `17efd20` + 未提交改动] 重跑上述只读检查、场景 7 与资产可定位性，结论一致。
- **新记录待修问题**：`scripts/sync-skills.sh` 在隔离根 `dsh-home` 存在但 `skills/` 缺失时，rsync 以退出码
  11 失败并告警（`--check` 不受影响）。本轮以记录为主，未修改同步脚本。

---

## 二、项目宪法约束（规划时不可违反）

以下是继续有效的安全与范围约束；产品终点和阶段路线以产品路线图为准：

1. **不修改 DSH Runtime**，不修改核心代码，不长期维护 Fork。
2. **最终建设 OmniDesk 自有桌面应用**；当前阶段不铺开完整 GUI，先完成产品验证与引擎接口技术切片。不建通用 Core 层，不做 Agent 市场/插件生态。详见产品路线图。
3. 当前 DSH 验证阶段的定制通过 **Profile / Skill / 配置文件 / 外部数据目录** 实现；独立桌面阶段的 OmniDesk 自有 UI 与应用服务在 DSH 外实现，通过已验证适配边界调用引擎。
4. **数据资产独立于 AI 运行环境**：个人数据永远在仓库之外。
5. **先产品，后平台**：当前只服务一个真实用户、一个真实领域、一个真实工作流。
6. **数据分级**：案件材料、案件线索、未公开调查信息**禁止**进入本系统。此规则已写入 Skill 正文。
7. 新增能力**必须服务于纪检监察真实工作流程**。
8. **合规准入（书面授权）**：处理真实工作材料需**单位书面授权**，且须**逐项记录**（数据/材料、模型/服务商、工具、个人设备、运行环境），**不得假定已获批准**；材料授权不代替模型、工具或环境授权，**脱敏不等于获准**。未获授权前只使用合成、公开或明确批准的数据。逐项状态表见产品路线图阶段 2.5。
9. **组件职责**：Skill 负责流程与专业规则；MCP 只接入确有需要且已获准的外部工具；Engine Adapter 保持最小连接边界，**不新造通用协议**。
10. **文件优先**：SQLite 仅在出现可观察的并发写入冲突、状态查询效率问题，或文件检索优化后仍不达标时评估；**触发评估 ≠ 自动采用**。
11. **产品化是联合决策门槛**：净节省率 > 20% + 质量与合规达标 + 至少一位同事愿在获准条件下试用 + 存在桌面应用能解决的具体痛点；满足门槛后**不自动启动开发**。

---

## 三、已明确但尚未完成的事项（交给下一阶段）

### 3.1 Phase 2B 明确推迟：材料写作真实模型端到端实测

Phase 2B 原文：「先提交代码，再单独安排材料写作模型实测」。现已提交。（「该实测尚未执行」是本简报**成文当时**的状态；其后已完成，见下方「当前状态」。）
这是当前**最高优先级**的验证缺口，因为它决定了「模板/风格/范例是否真的被运行时读取」这一核心假设是否成立。
**当前状态（2026-10-16 更新）**：第 7 项（Skill 修改后隔离实例加载新版本）已在一次性隔离环境中通过真实 DSH 发现机制验证；
真实模型端到端写作已于隔离实例 Web UI 中跑通并逐字复核（见 `tests/reports/phase2-webui-writing-e2e-report.md`）；
Phase 2A 规定的**专用合成夹具场景 1–6 已于 2026-10-16 用执行器逐项跑完并取得真实模型输出**：5 项自动判定通过，1 项（场景 4）自动判定误报、经人工复核确认通过。详见 `tests/reports/phase1-verification-report.md` 第七节。
**执行框架**：合成素材（`tests/verification/phase1/fixtures/`）+ 执行器（`tests/verification/phase1/run_material_writing_scenarios.py`）；
本轮以 `--allow-credential-copy` 使用**隔离实例自身凭证**，在 /tmp 的一次性 `dsh-home` 副本中调用，不写入 `~/OmniDesk`、不触碰原版 `~/.dsh`；未更改凭证与路由。
**该项（原 P0 验证缺口）就此关闭。**

需用**不含敏感信息的合成素材**覆盖（对应 Phase 2A 验收场景）：

- 模板、规则、范例齐备时，能否生成结构合适的工作总结；
- 无适配模板时，是否**按用途设计结构**而非强套模板；
- 关键数据缺失时，是否标注「待确认」；
- 两条输入事实互相矛盾时，是否指出冲突；
- 范例目录为空时，是否**如实说明未使用范例**（这是当前真实状态）；
- 提交模拟的禁止处理数据时，是否执行**数据边界**规则（仅用虚构占位内容）；
- 修改测试 Skill 标记并同步后，隔离实例是否真正加载新版本。

### 3.2 过时文档结论（已完成）

`docs/OmniDesk-Phase2-Plan.md` 第 182 行仍称「技能声明了 model-invocable，由工具按名称调用」。
经源码核实，DSH 只识别 kebab 键 `disable-model-invocation` 与 `user-invocable`，`model-invocable` 是**无效键**。
该结论已在当前仓库提交 `17efd20` 中标注过时并给出正确结论；后续不应再把它列为待办。

### 3.3 历史范例库为空

`knowledge/excellent-cases/` 目前只有 README，无任何真实范文。用户手中有可脱敏的历史材料，
但**尚未筛选入库**。在放入真实范例前，Skill 必须如实说明「未使用历史范例」（当前已如此约定）。

### 3.4 版本发布机制

- 现存 tag `discipline-v0.2` 指向 9 个提交之前的旧状态，**已落后于当前基线**。
- `CHANGELOG.md` 与 `VERSION`(0.2.1) 已更新，但**当前基线尚未打新 tag**。
- Phase 3 的「建立版本发布机制」在形式上存在，实质待本阶段收口。

### 3.5 知识检索链路已验收（合成夹具），正式制度接入待立项

**2026-10-16 更新**：已制定知识检索验收契约（`docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md`），
并用**纯合成虚构制度**跑通 6 类场景（`tests/verification/retrieval/`），取得真实模型输出级证据：
条款级引用未命中 0、越界读取 0、来源文件与 `tool_call` 实际触碰文件一致，6/6 通过。
检索真实性（命题 A）与引用真实性（命题 B）**分别验证且均通过**；`--rejudge` 可确定性复现。详见
`tests/reports/retrieval-verification-report.md`。

仍**未完成**：`scripts/sync-skills.sh` 目前只把**模板、写作风格、历史范例**三类资产物化到运行时副本；
`knowledge/regulations/` 下的 **234 份正式制度文件未读取、未复制、未索引**。其接入须在本验收方案先行跑通后
**另行立项授权**，并先明确文件处理、索引存放（放外部数据目录，不入 DSH、不入仓库）、更新机制、权限边界与模型数据路径。
该授权属**单位书面授权**范畴，须逐项覆盖数据/模型/工具/设备/环境；**未获授权前不得读取、复制或索引**这些文件。
### 3.6 个人自用 MVP 最小纵向切片（已完成实现，2026-10-16）

路线图新增「阶段 2.6：个人自用 MVP」。它把已有桌面技术切片推进为**作者本人可实际操作的最小工作流**（单一用户、个人使用、不分发），
**不是**阶段 3 产品化，也不改变 2.5.4 的产品化联合门槛。

**范围调整（重要）**：对**作者个人自用**，**不以「净节省率 > 20%」作为开始使用的前置条件**；先在真实使用中收集**低负担**的体验与返工记录。
2.5.1 的**书面授权准入不因本阶段放宽**。

**已完成**：共用任务外壳 `omnidesk-task-shell/1`（六字段：`task_type / input / stages / result / confirmation / persist`，文档 `docs/OmniDesk-Task-Shell.md`）；
单一纵向试点 **脱敏数据分析与生态研判**；确定性统计全部由本地 Rust 代码计算（分组计数/总和/均值/最小/最大 + 稳定指纹）；
五段式 UI（选类型 → 载入合成样例 → 运行统计 → 生成解释提示词 → 人工确认 → 保存到仓库外 → 轻量使用记录）；
结果默认落盘 `~/YvanAI/projects/Omnidesk/analysis`（仓库外，`OMNIDESK_LOCAL_DATA_ROOT` 可覆盖）；使用记录**不含输入输出正文**，可逐条删除/清空。
`weekly_report` / `material_writing` / `project_management` 仅作 `reserved` 任务类型边界，**不可执行**。

**证据等级**：Rust 与前端构建为 `script`；源码事实为 `static`；**模型阶段为 `unverified`**（本轮进程环境无任何 API 密钥，未读取隔离根凭证），
因此「模型只解释不重算」**未经真实模型验证**。真实 Tauri 窗口在无 GPU 虚显下渲染为空白（阶段 2 已记录），本轮以 vite 开发服务器截图确认**前端**渲染，不等于真实窗口视觉验收。

**未完成边界**：周报、通用材料撰写、完整项目管理本阶段**未实现**；其真实材料准入**不能由本次实现推断**。输出侧红线扫描是**启发式提示，不是合规控制**。

---

## 四、需要规划者决策的开放问题

1. **是否立即为当前基线打 tag**（如 `discipline-v0.3`）并正式发布一次版本记录？
2. **历史范例库的建设路径**：由用户提供脱敏范文，还是先定义入库标准与脱敏流程？
3. **模型路由策略**：设计文档提出了「复杂规划用 GPT/Claude、中文写作用 GLM/Agnes、代码用 GPT/DeepSeek」等设想，
   但**尚无落地实现**。是否需要、以及以何种形式（Profile 配置 or 文档约定）落地？
4. **法规库的运行时可用性**（部分推进）：检索链路的**契约与合成验收已闭环**（见 3.5）；剩余决策是把 `knowledge/regulations/` 下的 234 份正式制度文件接入检索链路的具体方式与授权。
5. **真实使用记录**：`usage-log` 语义是「项目内工作记录」还是「外部数据层 `~/YvanAI/memory`」，需明确并统一。
6. **评审优先级的迁移**：当前阶段 1 收口与知识检索验收均已完成，**下一阶段优先级最高的是路线图「阶段 2.5 项目准入与价值评测基线」**——先逐项落实书面授权、再按 [价值评测协议](OmniDesk-Value-Evaluation-Protocol.md) 建立价值基线，而不是继续扩大桌面功能。

---

## 五、建议的下一步候选工作（按优先级）

| 优先级 | 工作 | 理由 | 验收要点 |
| --- | --- | --- | --- |
| **P0（新，最高）** | 阶段 2.5：项目准入与价值评测基线 | 产品价值仍待验证；书面授权是处理真实材料的硬门槛 | 逐项授权状态表 + 价值评测协议草案 + 产品化联合门槛，见路线图阶段 2.5 与 [价值评测协议](OmniDesk-Value-Evaluation-Protocol.md) |
| **P0（新）** | 阶段 2.6：用**已获准**数据跑一次个人自用 MVP 工作流 | 实现已完成，但真实使用尚未发生；模型阶段仍为 `unverified` | 在 2.5.1 授权逐项落实后走一遍同一工作流，并据轻量使用记录决定是否继续 |
| ~~P0~~ ✅ | 用专用合成夹具补齐材料写作场景 1–6 | **已完成（2026-10-16）**：执行器逐项跑完，5 通过 + 1 人工复核通过，证据等级 `model` | 已留证：`tests/verification/phase1/evidence/material-writing-evidence.json`、报告第七节 |
| P1 | 验证独立桌面到 DSH 的引擎接口技术切片 | 最终产品方向已更新，需尽早排除引擎集成风险 | 依路线图验证任务、输出、错误、取消/恢复及隔离；不宣称未验证能力 |
|  | ↳ 第一步：引擎接口只读审计 | — | **已完成（2026-10-16）**：`docs/OmniDesk-Engine-Interface-Audit.md` |
|  | ↳ 第二步：引擎接口契约设计 | — | **已完成（2026-10-16）**：`docs/OmniDesk-Engine-Interface-Contract.md` + 测试矩阵 `docs/OmniDesk-Engine-Interface-Test-Matrix.md`（矩阵已由 v0.2 更新为「已执行」，E01–E20 均已运行） |
|  | ↳ 第三步：最小适配器原型 | — | **已完成（2026-10-16，经用户授权）**：适配层 `engine/omnidesk_engine/` + E01–E20 隔离实测 20/20 pass；E18 model 臂阻塞记为 `unverified`。证据 `tests/reports/p1-engine-adapter-prototype-report.md` |
| P1 | 为当前基线建立版本发布（tag + CHANGELOG 收口） | 用户要求「下载即用、锁定我测试过的版本」 | 存在指向当前基线的 tag；CHANGELOG 与 VERSION 一致 |
| P2 | 历史范例入库标准与脱敏流程 | 范例库是写作质量的关键，但涉及数据边界 | 有明确的入库准则；Skill 能区分「有范例/无范例」 |
| ~~P2~~ ✅ | 知识检索链路验收（合成夹具） | `knowledge-search` 是第二优先级核心能力 | **已完成（2026-10-16）**：6/6 通过、引用未命中 0、越界 0，证据等级 `model` |
| P2 | 234 份正式制度文件接入检索链路 | 承接已验收的检索链路，接入正式知识资产 | 先定文件处理/索引/更新/权限/数据路径；经授权后方可读取 |
| P3 | 模型路由策略落地 | 设计层面已提出，实现层面空缺 | 有可执行的配置或明确文档约定 |

> `setup.sh --check` 已修复失败吞错；成功路径、合成底座/Skill 双失败路径、非零总退出码，以及真实检查流程的只读文件副作用（五根快照零差异）均已通过复核。隔离 Skill 同步加载亦已验证（DSH 真实发现机制读取新版本正文）。材料写作的真实模型端到端链路已在隔离 Web UI 中跑通并逐字复核（见 `tests/reports/phase2-webui-writing-e2e-report.md`）；**专用合成夹具场景 1–6 亦已取得真实模型输出并通过**（见 `tests/reports/phase1-verification-report.md` 第七节），原 P0 缺口已关闭。**知识检索链路亦已用纯合成夹具验收闭环（6/6，引用未命中 0，见 `tests/reports/retrieval-verification-report.md`）**，且**未触碰 234 份正式制度文件**。独立桌面是最终产品目标，但此表不授权提前全面开发 GUI。下一阶段的完整顺序、技术验证范围及免费 GPT 的职责边界见产品路线图。

---

## 六、给规划者的交接提示词（可直接使用）

```text
你是 OmniDesk 项目的计划讨论助手。你无法读取仓库或运行测试，只能根据下列由用户提供的已核实摘要讨论计划；不得声称检查了仓库或验证了实现。仓库事实、实施和验收由可读取项目的 Codex 环境负责。最终产品目标是 OmniDesk 自有桌面应用；DSH 是早期可替换执行引擎。

请基于用户粘贴的路线图摘要和当前问题，比较方案或提出计划：

1. docs/OmniDesk-Next-Phase-Brief.md   （本文件：当前状态与缺口）
2. docs/OmniDesk-Design-Principles-v0.3.md  （项目宪法，硬约束）
3. docs/OmniDesk-Phase2A-Report.md     （Skill 链路的源码级事实）
4. docs/OmniDesk-Phase2B-Inventory.md  （资产权威源与去重依据）
5. docs/ARCHITECTURE.md                （分层架构）

硬约束：不改 DSH Runtime；当前不铺开完整 GUI，但最终目标是自有桌面应用；不建通用 Core 层；不引入 Agent 市场；
数据资产在仓库之外；禁止案件材料等敏感数据进入系统。

当前已知实现基线：提交 dab3d78；简报初始文档提交 17efd20。底座锁定 dsh-v0.2.1-alpha.2；历史隔离自检记录为 37/37（执行前需由 Codex 在当前状态复核），
黄金任务已有 40 个，Skill 6 个，历史范例库为空。

请针对第三节列出的缺口，输出分阶段执行计划，每个 Step 给出：
单一动作、输入来源、输出目标、验收标准。不要重复已完成的工作。
```

---

## 七、交接时的环境事实（供参考）

- 隔离实例启动：`scripts/omnidesk-dev.sh`（默认端口 3210，使用锁定底座）。
- 技能同步：`scripts/sync-skills.sh`（默认只读检查，`--sync` 显式执行；只删托管清单内的技能）。
- 底座校验：`scripts/bootstrap-runtime.sh --check`。
- 隔离自检：`scripts/omnidesk-isolation-check.sh`。
- 用户正在使用的原版 DSH 桌面实例**始终运行中**，`~/.dsh/sessions` 会持续变化，
  这属于该实例的正常行为，**不是** OmniDesk 造成的回归。
