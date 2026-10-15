# OmniDesk 下一阶段规划交接简报 (Next-Phase Brief)

> 用途：本文件是 OmniDesk 项目在 Phase 2B 提交完成后的**状态快照与规划输入**。
> 供 GPT-6-luna（或任何接手规划的模型）在制定下一步计划前阅读，避免重复已完成工作、误判当前基线。
>
> 生成时间：2026-10-15；基线提交：dab3d78。

---

## 一、当前基线（均为已验证事实）

### 1.1 仓库与运行底座

| 项 | 值 |
| --- | --- |
| 仓库 | git@github.com:klzone/omniharness.git（分支 main，已推送） |
| 当前提交 | dab3d78（工作区干净，与 origin/main 对齐） |
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
- `tests/reports/citation-audit-report.md`：声称 30/30 抽查、96.7% 准确率。
- `workstations/discipline/memory/usage-log/README.md`：声称 20 次真实任务、平均效率提升 87.1%。

> ⚠️ 上述两份报告均产生于隔离验证**之前**，未在隔离环境中复核，**不应作为当前证据引用**。

---

## 二、项目宪法约束（规划时不可违反）

以下来自 `docs/OmniDesk-Design-Principles-v0.3.md` 与历次执行 Plan，是硬约束：

1. **不修改 DSH Runtime**，不修改核心代码，不长期维护 Fork。
2. **不开发 GUI**、不建 Core 层、不做 Agent 市场/插件生态。
3. 扩展只能通过 **Profile / Skill / 配置文件 / 外部数据目录** 实现。
4. **数据资产独立于 AI 运行环境**：个人数据永远在仓库之外。
5. **先产品，后平台**：当前只服务一个真实用户、一个真实领域、一个真实工作流。
6. **数据分级**：案件材料、案件线索、未公开调查信息**禁止**进入本系统。此规则已写入 Skill 正文。
7. 新增能力**必须服务于纪检监察真实工作流程**。

---

## 三、已明确但尚未完成的事项（交给下一阶段）

### 3.1 Phase 2B 明确推迟：材料写作真实模型端到端实测

Phase 2B 原文：「先提交代码，再单独安排材料写作模型实测」。现已提交，**该实测尚未执行**。
这是当前**最高优先级**的验证缺口，因为它决定了「模板/风格/范例是否真的被运行时读取」这一核心假设是否成立。

需用**不含敏感信息的合成素材**覆盖（对应 Phase 2A 验收场景）：

- 模板、规则、范例齐备时，能否生成结构合适的工作总结；
- 无适配模板时，是否**按用途设计结构**而非强套模板；
- 关键数据缺失时，是否标注「待确认」；
- 两条输入事实互相矛盾时，是否指出冲突；
- 范例目录为空时，是否**如实说明未使用范例**（这是当前真实状态）；
- 提交模拟的禁止处理数据时，是否执行**数据边界**规则（仅用虚构占位内容）；
- 修改测试 Skill 标记并同步后，隔离实例是否真正加载新版本。

### 3.2 过时文档待标注

`docs/OmniDesk-Phase2-Plan.md` 第 182 行仍称「技能声明了 model-invocable，由工具按名称调用」。
经源码核实，DSH 只识别 kebab 键 `disable-model-invocation` 与 `user-invocable`，`model-invocable` 是**无效键**。
该行需标注**已过时**并给出正确结论，以免后续规划基于错误前提。

### 3.3 历史范例库为空

`knowledge/excellent-cases/` 目前只有 README，无任何真实范文。用户手中有可脱敏的历史材料，
但**尚未筛选入库**。在放入真实范例前，Skill 必须如实说明「未使用历史范例」（当前已如此约定）。

### 3.4 版本发布机制

- 现存 tag `discipline-v0.2` 指向 9 个提交之前的旧状态，**已落后于当前基线**。
- `CHANGELOG.md` 与 `VERSION`(0.2.1) 已更新，但**当前基线尚未打新 tag**。
- Phase 3 的「建立版本发布机制」在形式上存在，实质待本阶段收口。

### 3.5 知识检索资产未物化

`scripts/sync-skills.sh` 目前只把**模板、写作风格、历史范例**三类资产物化到运行时副本；
`knowledge/regulations/` 等检索类知识**未同步**，`knowledge-search` Skill 运行时如何定位法规库尚未确定。

---

## 四、需要规划者决策的开放问题

1. **是否立即为当前基线打 tag**（如 `discipline-v0.3`）并正式发布一次版本记录？
2. **历史范例库的建设路径**：由用户提供脱敏范文，还是先定义入库标准与脱敏流程？
3. **模型路由策略**：设计文档提出了「复杂规划用 GPT/Claude、中文写作用 GLM/Agnes、代码用 GPT/DeepSeek」等设想，
   但**尚无落地实现**。是否需要、以及以何种形式（Profile 配置 or 文档约定）落地？
4. **法规库的运行时可用性**：是否需要把 `knowledge/regulations/` 接入检索链路，以及如何在不入库的前提下做到？
5. **真实使用记录**：`usage-log` 语义是「项目内工作记录」还是「外部数据层 `~/YvanAI/memory`」，需明确并统一。

---

## 五、建议的下一步候选工作（按优先级）

| 优先级 | 工作 | 理由 | 验收要点 |
| --- | --- | --- | --- |
| P0 | 材料写作真实模型端到端实测（合成素材） | Phase 2B 明确推迟，是核心假设的验证缺口 | 7 个验收场景全部有真实运行结果；不得以静态检查冒充运行验证 |
| P1 | 标注 `Phase2-Plan.md:182` 过时结论 | 错误前提会误导后续所有规划 | 该行或其附近明确写出正确结论 |
| P1 | 为当前基线建立版本发布（tag + CHANGELOG 收口） | 用户要求「下载即用、锁定我测试过的版本」 | 存在指向当前基线的 tag；CHANGELOG 与 VERSION 一致 |
| P2 | 历史范例入库标准与脱敏流程 | 范例库是写作质量的关键，但涉及数据边界 | 有明确的入库准则；Skill 能区分「有范例/无范例」 |
| P2 | 知识检索资产生效路径 | `knowledge-search` 是第二优先级核心能力 | 检索结果稳定包含「答案 + 原文引用 + 来源文件 + 位置」 |
| P3 | 模型路由策略落地 | 设计层面已提出，实现层面空缺 | 有可执行的配置或明确文档约定 |

---

## 六、给规划者的交接提示词（可直接使用）

```text
你是 OmniDesk 项目的下一阶段规划者。请先阅读以下文件，再制定计划：

1. docs/OmniDesk-Next-Phase-Brief.md   （本文件：当前状态与缺口）
2. docs/OmniDesk-Design-Principles-v0.3.md  （项目宪法，硬约束）
3. docs/OmniDesk-Phase2A-Report.md     （Skill 链路的源码级事实）
4. docs/OmniDesk-Phase2B-Inventory.md  （资产权威源与去重依据）
5. docs/ARCHITECTURE.md                （分层架构）

硬约束：不改 DSH Runtime；不开发 GUI；不建 Core 层；不引入 Agent 市场；
数据资产在仓库之外；禁止案件材料等敏感数据进入系统。

当前基线：提交 dab3d78，底座锁定 dsh-v0.2.1-alpha.2，隔离自检 37/37 通过，
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

