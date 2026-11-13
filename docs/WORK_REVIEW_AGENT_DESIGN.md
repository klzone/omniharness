# Work Review Agent（工作记录复盘 Agent）试点设计

> 状态：OD-ARCH-150 规划成果（只设计，不实施）。首个 Multi-Agent 试点。
> 关联：[Harness 设计](OMNIDESK_HARNESS_ARCHITECTURE.md) · [Graph 设计](OMNIDESK_MULTI_AGENT_GRAPH_DESIGN.md) · [路线图](OMNIDESK_HARNESS_ROADMAP.md)
>
> 实施进度（2026-11-13）：历史检索、场景→Agent 声明、verdict、事件、12 项合成基准与测量脚本均已建立；工具面为「可写文件、无 shell」（D-097），删除文件由宿主回收站负责。真实模型评测未做。

## 一、定位与职责边界

Work Review Agent 是 OmniDesk 第一个显式 Agent Profile：把**已存在的**工作记录场景能力（memory-agent 随手记判定、task_extract 候选提取、daily-summary 当日摘要、period-review 周期复盘、memory-report 周报）组装成一个有独立配置、权限、验收器与记忆边界的专业角色。

**负责**（用户清单逐项，标注现有基础）：

| 职责 | 现有基础 |
| --- | --- |
| 工作记录理解/事实提取 | memory-agent 判定块+宿主逐字校验（已实现） |
| 工作进展归纳/完成与待办识别 | daily-summary/period-review 引用+宿主任务统计（已实现） |
| 任务候选发现 | task_extract 链路（已实现，含置信度+不确定性+去重+已有任务匹配） |
| 重点工作识别 | period-review 汇总模板（已实现） |
| 工作问题发现 | 复盘「问题与不足」段（模板已有，质量待 Golden Task 验证） |
| 阶段性复盘 | period-review 周/月/季/年（已实现） |
| 历史工作关联 | 知识关联推荐链路可复用于记录↔记录关联（部分：现有推荐面向知识，历史记录关联是增量） |
| 写作素材候选发现 | material_extract_from_record 同车返回（已实现） |

**不负责**（由权限与确认制强制，非提示词约定）：不改原始记录（raw_input 不可变+模型无写权限）；不直接创建正式任务（候选→用户确认→宿主落盘）；不改任务状态/排期（无对应工具）；不更新已审核知识（无知识写工具）；不自动纪律审查结论（数据边界红线，Skill 明文）。

## 二、工作流程（现状映射 + 增量）

```text
用户提交工作记录
→ Work Review Agent 接收            [现状：首页/工作记录场景 intent=note]
→ 加载相关工作上下文                [现状：当周索引+既有任务+素材 id 注入；增量：相关历史记录检索]
→ 分析工作事实                      [现状：memory-agent/task_extract 模型调用]
→ 检索相关历史记录                  [增量：work/ 内只读词法检索（复用 vault_search 模式）]
→ 识别任务进展                      [现状：match_task_id+match_confidence]
→ 形成结构化候选结果                [现状：tc-*.json；增量：verdict 检查明细]
→ Sensors 验证                      [增量：verdict.v1 统一验收（quote/日期/去重/事实推断区分/置信度门）]
→ 用户审核                          [现状：TaskReviewPanel]
→ 写入对应业务实体                  [现状：确认→task_store 落盘+双向回写]
→ 保存执行记录和来源关系            [现状：候选已处理归档；增量：事件行]
```

增量项只有三处：历史记录检索、verdict 统一验收、事件登记。其余全部复用。

## 三、Agent 专属配置（复用现有机制，逐项）

| 配置 | 设计 | 载体 |
| --- | --- | --- |
| Agent Profile | rules.json 新增 agents.work-review：绑定场景（工作记录+首页随手记）、Skill 集（memory-agent、daily-summary、period-review、按需 knowledge-search/anysearch）、工具面变体（只读+无 bash） | profile-rules.v2 → v3（向后兼容） |
| System Guide | 场景规则（rules.json scenes.weekly.rules 已有）+ SKILL.md 绑定注入 | 现状 |
| 专属 Skills | 上述 Skill 集；material-writing/document-review 不在集内 | skill_bindings |
| 工具权限 | preset 变体 work-review-core：discipline-core 基础上 tool-fs write/edit=false、去 bash/pwsh/fs-search；保留 read/skill/ask-user/web | cordis.patch preset 行（D-092 模式） |
| 上下文加载规则 | 当周索引+未完成任务（≤30）+素材 id（≤50）+历史检索命中（增量，预算 ≤2000 字符） | task_extract.rs MAX_* 先例 |
| Memory Scope | 读：work/工作记录、work/任务、库/知识库（只读）；写：work/.candidates、work/摘要、work/复盘（经确认） | 目录 ACL=宿主命令校验 |
| Sensors | verdict 任务候选验收器集（Harness §3.2 第一行） | verdict.v1 |
| 执行预算 | 单记录一次分析调用（现状）；不自动循环；超预算只标记 needs_review | 台账+verdict |
| 人工审批规则 | 全部候选等待确认；confidence<阈值或 uncertainty 非空 → 显式标「需补充信息」；确认 24h TTL 照 pending_writes 先例 | 候选 status 字段 |
| 输出 Schema | omnidesk.task-candidates.v1 + material-candidates.v1（已有）+ verdict.v1（新增） | 不新造 |

**不为配置建管理系统**：全部落在 rules.json + preset patch + 既有 IPC，与 D-054~D-060 的 Profile 资产模式一致。

## 四、记忆边界

- 专属记忆：work/.candidates/（任务+素材候选）与 work/摘要、work/复盘 草稿——Work Review 的工作区。
- 全局记忆：work/工作记录、work/任务、库/知识库（只读引用，写经确认）。
- 不共享其他 Agent 会话；Knowledge Agent 感知 Work Review 产出只经事件（work_record.analyzed / task_candidate.created）。

## 五、试点验证集（Golden Tasks 设计）

从 tests/golden_tasks 模式扩展（合成场景，不用真实敏感数据外发）：

| # | 用例类型 | 输入形态 | 基准答案 | 评价指标 |
| --- | --- | --- | --- | --- |
| WR-G01 | 单条记录事实提取 | 一段含 2-3 个事实的记录 | 事实清单+逐字引用 | 引用命中率、事实召回 |
| WR-G02 | 多条记录合并复盘 | 5-8 条同周记录 | 周复盘草稿 | 引用完整率、无编造 |
| WR-G03 | 已完成事项识别 | 含「已完成/搞定」等记录 | 完成清单 | 准确率 |
| WR-G04 | 未完成事项识别 | 含待办暗示记录 | 待办清单 | 准确率 |
| WR-G05 | 同一任务连续进展 | 跨 3 天同一事项记录 | 进展链+任务匹配 | match_task_id 正确率 |
| WR-G06 | 重复任务识别 | 两条相似记录 | 去重判定 | dedup 指纹+模型判定一致率 |
| WR-G07 | 模糊时间识别 | 「下周一之前」「月底前」 | hint 保存不换算（402 口径） | 是否未擅自换算 |
| WR-G08 | 事实与推断区分 | 混合记录 | fact/推断分列 | uncertainty 字段正确性 |
| WR-G09 | 工作成果识别 | 含量化成果记录 | 素材候选（工作成果类） | 类型正确率 |
| WR-G10 | 敏感内容处理 | 含红线词的记录 | 整体拒绝不产生候选 | 拒绝率 100% |
| WR-G11 | 历史知识关联 | 提及制度名记录 | knowledge link 候选 | 命中正确率 |
| WR-G12 | 低置信度处理 | 语焉不详记录 | needs_review 不出正式候选 | 置信度门行为 |

基准答案人工标注一次、封存于 tests/golden_tasks/work-review/；评价脚本确定性复现（沿用 Value-Evaluation-Protocol 的证据等级与预注册纪律）；**敏感边界：全部合成，真实记录不进外部模型测试**。

## 六、评价指标（Phase E 测量口径）

| 指标 | 定义 | 来源 |
| --- | --- | --- |
| 事实提取准确率 | 候选引用逐字命中记录原文比例 | verdict quote_in_source 统计 |
| 任务识别准确率 | 用户确认且事后未撤销的候选比例（+匹配任务正确率） | 候选 status 统计 |
| 重复识别准确率 | dedup 判定与基准一致率 | WR-G06 |
| 引用完整率 | 复盘/摘要段落编号真实存在比例 | 现有引用校验扩展 |
| 用户修正率 | 确认前被编辑的候选比例 | decision_note+修订 diff |
| 执行成功率 | 分析调用 task_failed 占比 | 台账 error_code |
| Token 消耗 | 每次分析 usage（unknown：DSH usage 字段先验证） | 台账新增字段 |
| 平均执行时间 | 提交→verdict 完成 | 台账 ttfb+state_since |

## 七、明确不做（本试点）

不建 Agent 间自动链（记录→候选→任务→素材已由用户确认驱动）；不做自动定时复盘调度变更（scheduler 已有机制不动）；不引入向量检索；不改 golden_tasks 既有 40 项；不做历史记录全文重建索引。
