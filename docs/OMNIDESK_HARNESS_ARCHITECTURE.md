# OmniDesk Harness 架构设计（六大组件 · 全局与 Agent 专属）

> 状态：OD-ARCH-150 规划成果（设计文档，未实施；候选方案不写成已决定）。吸收 Harness² 思想，不引入其代码、不替换 DSH Runtime。
> 关联：[现状审计](OMNIDESK_CURRENT_ARCHITECTURE_AUDIT.md) · [存储审计](OMNIDESK_STORAGE_ARCHITECTURE_AUDIT.md) · [Graph 设计](OMNIDESK_MULTI_AGENT_GRAPH_DESIGN.md) · [试点设计](WORK_REVIEW_AGENT_DESIGN.md) · [路线图](OMNIDESK_HARNESS_ROADMAP.md)

## 一、Harness 定义与现有对应物

Harness = 控制 Agent 如何运行、访问数据、执行任务、验证结果、沉淀经验的基础设施。逐项对照现状（详细证据见现状审计）：

| 组件 | 已具备（证据） | 差距 |
| --- | --- | --- |
| Guides | SKILL.md×10；rules.json（writing_rules 7 条+红线+掩码）；场景 rules/sections/thinking；persona 检索/批处理/预算规则（D-092/092b）；工程规范 docs/development/*（人类协作用） | 无四层显式分层；无 per-Agent 组装；规则重复（AGENTS.md 与 rules.json 写作规则语义重叠，服务对象不同，可接受） |
| Sensors | 宿主校验器群：task_extract.rs 白名单+长度+去重；writing 模块九态合法性；fact_check.rs 八类结论；knowledge_review 硬约束；红线扫描；cargo/pytest/tsc + verify 脚本群 + golden_tasks | 分散在各模块，无统一验收器注册/执行协议；四大 Agent 专属验收器未成体系 |
| Agentic Loop | DSH Agent Loop（步进/工具调度/compaction，rc.5 实测审计）；OmniDesk 九态任务状态机+合法转换+取消五阶段；discipline-core 精简工具面+预算规则 | 执行预算（步数/token）无宿主级配置；interrupted 恢复有机制（recover_task）但缺「中断后继续」产品语义（D-092 边界） |
| External Memory | 全实体文件存储+稳定 id+修订留痕（见存储审计） | 无 Agent 记忆概念与边界定义；无记忆更新冲突处理（当前由确认制规避） |
| Permissions | admission.rs 默认拒绝+三类端点授权；tool-fs/bash/pwsh preset 开关；workspace-write 沙箱；写确认制 pending_writes；红线扫描+掩码+流式还原 | 无 per-Agent 工具面差异（现全 preset 共用）；Agent 级目录 ACL 缺失 |
| Observability | 台账（state/seq/ttfb/error/evidence_source）；会话 v4 日志；时间线事件全透出（UI-710/712）；usage.jsonl；红线掩码不落日志 | token 未落台账；「用户请求→Agent→Skill→Tool→数据变化→验证→交付」全链路需跨 4 处日志人工拼；无执行级聚合视图 |

**结论：六组件中 Loop/Permissions/Guides 已有实质实现；Sensors、External Memory 登记、Observability 聚合、统一事件是主要补齐项。** 全部可在 OmniDesk 层实现，不动 runtime/。

## 二、Guides 四层管理方案（设计）

```text
L1 全局规则（不变，人类协作）：AGENTS.md + docs/development/*
L2 平台规则（模型可见，静态注入 DSH）：
   persona.suffix 检索/批处理/预算规则 + agent-instructions（cordis.patch.yml）
L3 Agent/场景规则（动态注入）：
   rules.json scenes.<场景>.rules + skill_bindings 绑定的 SKILL.md（≤2500 字符）
L4 项目/任务规则（按需）：附件材料 + 知识检索上下文（≤3000 字）+ 会话上文（≤2500 字）
```

多 Agent 改造要点（候选设计，待批准）：L3 键从「场景键」扩展为「agent 键」——skill_bindings 增加 agent 维度（如 weekly-note 保留场景语义，新增 agent: work-review 声明归属）；每个 Agent 声明自己加载的 L2 追加段（可选）与 L3 Skill 集。**不新建规则管理系统**，继续复用 rules.json + profile_rules.rs 校验（deny_unknown_fields 已保证未知键报错）。

冲突与重复治理：L2 写「怎么做工具」；L3 写「怎么做业务」；SKILL.md 保留现有的数据边界节但引用规则资产而非复制词表（现状已如此）。加载过量控制：max_injected 限 1 + CONTEXT_MAX_CHARS 2500 + 知识上下文 3000 上限已有，Agent 化后沿用。

## 三、Sensors 验收器体系（设计，本期只定义不实现）

### 3.1 统一协议（候选）

```json
{
  "schema": "omnidesk.verdict.v1",
  "validator": "task-candidate.v1",
  "target": {"entity": "tc-...", "record_id": "wr-..."},
  "checks": [{"id": "quote_in_source", "pass": true}, {"id": "no_duplicate", "pass": true}],
  "conclusion": "pass | fail | needs_review",
  "evidence": {"command": "...", "counts": {...}}
```

与 Harness² Propose–Probe–Compose 对齐：propose=模型候选 → probe=Sensors 确定性验证 → compose=用户确认+宿主落盘。现有三处先例（任务候选/素材候选/知识审核）已经隐式跑这个循环；本期抽象为统一 verdict 结构，**先只落在 Work Review 试点**。

### 3.2 四类 Agent 专属验收器（定义试点范围）

| Agent | 验收器（检查 id → 确定性实现基础） |
| --- | --- |
| Work Review | quote_in_source（task_extract.rs 已有逐字校验）；date_shape（is_date_shape）；dedup（dedup_key 指纹已有）；fact_vs_inference（候选必带 uncertainty 字段，缺失即 needs_review）；duplicate_task（match_task_id+match_confidence 已有）；confidence_gate（低于阈值 → needs_review 不直接出候选） |
| Knowledge | source_exists（sources[] 文件存在性）；card_size（knowledge_review 已有 350 字上限先例）；dup_title（索引行比对）；link_resolves（vault_index 目标解析）；expired（knowledge_quality 180 天机制） |
| Writing | citation_verbatim（fact_check.rs 归一化匹配已有）；conflict_pair（口径冲突只提示已有）；no_orphan_facts（无来源结论清单已有）；structure（章节白名单已有） |
| Project | state_legal（task_store 合法转换表已有）；dep_acyclic（task_set_dependency 已有）；schedule_conflict（scheduling.rs 冲突输出已有）；done_evidence（完成态需关联记录/素材引用，候选实现） |

结论：**四类验收器的底层检查函数大半已存在**（每行右侧），缺的是 verdict 统一结构与「验收结论随候选一起暂存/展示」。这是 Phase B/C 的核心增量。

## 四、Agentic Loop：统一执行状态机（设计）

现状：任务级九态已在 contract.py（engine 内）；业务候选级只有隐式状态（candidate.status / review status / pending 状态）。

候选执行状态机（候选设计，统一 propose→compose 生命周期）：

```text
pending → planning → executing → verifying → waiting_approval → completed
                ↘ failed / cancelled（任意态可达）
```

| 状态 | 触发者 | 现状对应 |
| --- | --- | --- |
| pending | 宿主创建候选 | .candidates/*.json status=pending |
| planning/executing | 引擎（模型调用中） | 台账 state running |
| verifying | 宿主验收器跑 verdict | task_extract 校验（隐式，瞬时） |
| waiting_approval | 验证通过，等用户 | candidate status=pending 停留处 |
| completed | 用户确认+宿主落盘 | status=confirmed+confirmed_task_id |
| failed | 校验失败 | ai_analysis_status=failed |
| cancelled | 用户放弃/过期 | 24h TTL（pending_writes 先例） |

预算与防护（现状+候选）：

| 项 | 现状 | 候选补齐 |
| --- | --- | --- |
| 最大执行步数 | 无显式上限；DSH 内部有终止机制（连续 15 步纯工具调用的 persona 规则是提示词级） | 宿主台账加 step_count 字段+上限告警（D-092b 已定检索预算规则，机制可扩展） |
| token/时间预算 | ttfb 已落台账；token 未落 | usage 事件→台账字段 |
| 重试 | 用户手动（重新生成按钮）；无自动重试 | 维持手动（符合确认制，不加自动循环） |
| 超时 | 自动超时已被 OD-ARCH-071 用户裁决取消；终态由底座+用户取消决定 | 维持 |
| 人工介入 | approval_respond / question_answer 双通道已实现 | — |
| 中断恢复 | recover_session/recover_task + 事件补漏游标 | 中断轮次不自动续跑（GOALS 已定），恢复=用户显式续 |
| 幂等 | idempotency_key 字段在台账（本机样例 null）；提交侧未强制 | 候选创建按 dedup_key 幂等（已有） |
| 重复任务防护 | 候选 dedup_key=sha256(record_id+quote)；material 指纹同源 | 已具备 |

**结论：不需要新的执行循环。** 统一状态机是把「候选生命周期」显式化，核心增量只有 verifying（verdict 暂存）与 waiting_approval（已有）两个环节的登记。

## 五、External Memory（边界与登记，复用现有存储）

按存储审计 §四分类执行，本期不新增存储系统、不引入向量库：

| 记忆类型 | 载体 | Agent 可见性 |
| --- | --- | --- |
| 正式业务记忆 | work/ + 库/ 实体文件 | 全 Agent 只读引用；写经宿主+确认 |
| 会话记忆 | DSH session.v4.jsonl | 所属会话；fork 显式共享前缀 |
| 候选暂存（Agent 工作区） | .candidates / knowledge-reviews / pending-writes | 产出 Agent 读写；目标 Agent 经事件感知 |
| 经验记忆（Guides 沉淀） | rules.json / SKILL.md 修订（人工） | 下次注入生效 |
| 索引缓存 | 索引.md / 内存 index | 全 Agent；可重建 |

**Agent 专属记忆 vs 全局记忆边界（设计）：** 专属 = 候选暂存目录（如 Work Review 的任务/素材候选、Knowledge 的知识审核暂存）；全局 = 实体文件 + 索引。Agent 不共享「思考过程记忆」——交接靠结构化候选+事件（见 Graph 设计），不靠读对方会话。**不做**：Agent 私有笔记库、向量化记忆、跨会话自动摘要沉淀（后者属 Phase E 评估项）。

冲突处理（现状已覆盖）：记录冲突并列指出不择一（SKILL.md/规则明文）；事实更新走修订+revisions.jsonl；知识冲突走治理 review-decisions。记忆可信度：全部业务事实带来源 id；候选带 confidence+uncertainty；无来源内容禁止入正式层（红线+验收器）。

## 六、Permissions（Agent 级配置，设计）

现状基线：端点准入（默认拒绝+三类开关）→ preset 工具面（discipline-core）→ workspace-write 沙箱 → 写确认制 → 红线/掩码。五层已成型。

Agent 级差异化（候选设计，全部复用现有机制）：

| Agent | 工具面 | 目录 ACL | 模型通道 |
| --- | --- | --- | --- |
| Work Review | work-review-core：可读/写/编辑文件，无 bash/pwsh（D-097）；删除文件由宿主回收站负责 | 读 work/工作记录+库/；写经宿主动作 | 复用 discipline 端点授权 |
| Knowledge | 读 work/ 候选流+库/；写 knowledge-reviews 暂存 | 同上模式 | 同上 |
| Writing | 现状已满足（write/bash 恢复，D-092a） | work/写作+素材篮 | 同上 |
| Project | 只读任务/排期+建议候选 | work/任务建议暂存 | 同上 |

实现路径（候选）：preset 差异化——为受限 Agent 定义 preset 变体（照 D-092 discipline-core 先例，复制 9 行插件表改开关），或工具级 config 开关。**不新建权限框架**；DSH tool registry 的 allow/deny + OmniDesk 宿主命令准入（各 *_impl 已按路径校验）双保险已存在。Work Review 强制「只读分析+候选写入」不需要新机制——候选写入本来就走宿主 IPC 而非模型工具。

## 七、Observability（设计）

现状盘点：台账 JSON（state/seq/ttfb/evidence_source）+ 会话日志（完整工具调用+文本）+ UI 时间线（用户可见）+ usage.jsonl（人工使用记录）+ dsh-settings.log。

全链路追溯现状：一条真实执行可由 omni_task_id 串起：台账（状态）→ DSH 会话日志（提示词+工具调用，敏感内容经掩码/红线前置）→ 候选 JSON（产物）→ 实体文件（确认后）→ 治理/修订记录。**缺**：跨四处的聚合视图与 token 字段。

候选补齐（Phase B/E）：
1. 台账加 token_in/token_out 字段（来源=usage 事件透传，缺失标 unknown 不估算——延续 GOALS 口径）。
2. 执行聚合视图（设置页或诊断页）：按 omni_task_id 列状态/时长/token/验收结论/用户操作（确认/修正/放弃）。
3. 用户修正记录：候选被「拒绝/修改后确认」时把 decision_note 写候选 JSON（字段已有，确认 UI 传值未接，小改）。
4. 敏感边界维持现状：原文不进日志（红线前置+掩码+工具入参结果不进日志已是铁律）；verdict 只存计数与检查 id 不存原文。
