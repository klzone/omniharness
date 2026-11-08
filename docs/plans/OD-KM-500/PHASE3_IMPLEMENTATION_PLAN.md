# 第三阶段实施计划与任务卡（OD-KM-500 规划成果三）

> 状态：已批准（2026-11-09 用户裁决：模型 deepseek-v4.1-flash-eco / 思考等级 high 派发子代理执行；-eco 被路由拒绝则停下等用户）。任务卡按 docs/development/IMPLEMENTED_RULES.md §1 模板。

## 阶段目标

建立工作事实、正式任务、项目成果、知识依据和写作素材之间的关联体系；素材经人工审核成为第四阶段文稿生成的可信输入。核心口径：AI 只建议、宿主落盘、用户拥有最终决定权；work/ 临时层永不进知识库。

## 任务卡（OD-KM-501 起，全部含验收命令）

### 批次一（并行）：501 ∥ 502 ∥ 503

**OD-KM-501 写作素材实体 material_store.rs**
- 架构契合：照 schedule_store.rs/availability.rs 模式新增独立存储模块，Markdown+frontmatter+原子写，不引入数据库，不改 knowledge_vault。
- 实施步骤：① material_store.rs：WritingMaterial 结构与字段（见 WRITING_MATERIAL_DESIGN.md §一），素材类型十类白名单、审核状态五态、来源类型白名单；② 重复指纹（source_id+归一化引用 sha256）唯一性校验；③ ## 审核记录 追加段；④ CRUD + 按类型/状态/时间/项目/任务过滤查询；⑤ work_cleanup.rs 增加素材篮目录排除（清理预览与执行都不触碰 work/素材篮/，有测试）。
- 验收命令：cargo test --offline --lib（新增 ≥15 项全绿）；python3 tests/verification/desktop/verify_material_store_501.py 全绿。
- 依赖：无前置；阻塞 504/506/513；与 502/503 并行。

**OD-KM-502 最小项目实体 project_store.rs**
- 架构契合：work/ 项目 Markdown 先例（work/ 下 frontmatter 文件）独立小模块；明确不接管 project_items.rs（项目跟踪事项表不动）。
- 实施步骤：① project_store.rs：work/项目/<project_id>.md（name/goal/status/start/end，schema omnidesk.project.v1）；名称非空、状态白名单（进行中/已完成/已搁置）、起止日期格式校验；② create/update/list（update 不允许改名冲突）；③ 反向派生查询骨架 project_refs：输入 tasks+materials+records，输出按 project_id 分组关联清单（纯函数）。
- 验收命令：cargo test --offline --lib（新增 ≥10 项全绿）。
- 依赖：无前置；阻塞 508/512/513；与 501/503 并行。

**OD-KM-503 关联字段扩展（Task + WorkRecord）**
- 架构契合：照 401 先例 serde default 新字段，旧文件字节不变零迁移。
- 实施步骤：① Task 增 project_id（存在性校验由 508 IPC 层做）与 related_knowledge: Vec<KnowledgeLink>；② WorkRecord 增 related_knowledge: Vec<KnowledgeLink>（KnowledgeLink{id, reason, linked_at}，reason 必填照 importance_reason 先例）；③ 字节不变测试：旧 frontmatter load→save 逐字节一致；④ 库级存在性/格式校验函数。
- 验收命令：cargo test --offline --lib（新增 ≥10 项全绿，含字节不变）。
- 依赖：无前置；阻塞 507/508/513；与 501/502 并行。

### 批次二（并行）：504 ∥ 505 ∥ 508

**OD-KM-504 AI 素材提取搭车（零新增模型调用）**
- 架构契合：照 404 先例扩展 task_extract.py 提示词链路 + task_extract.rs validate 成对校验，搭逐条记录分析调用顺车；引擎脱敏链路（gate_user_text + masker）原样继承，零新增外发通道。
- 实施步骤：① task_extract.py 分析输出增 material_candidates（type 白名单/content/source_quote 必填/event_date/match_material_id/match_reason/knowledge_hints）与 validate_material_payload；提示词补方案 §四约束（只提取有事实依据内容、区分事实/判断/计划/推测、保留原始表述与位置、不把计划写成成果、不夸大、不凭空生成数据案例荣誉）；② task_extract.rs 宿主校验（quote 非空、type 白名单、content ≠ quote 防复制冒充提炼）；③ 候选落暂存照 .candidates 模式；④ 同记录重复分析按指纹关联既有素材不重复创建。
- 验收命令：pytest tests -q（187+ 新增全绿）；cargo test --offline --lib。
- 依赖：阻塞于 501；与 505/508 并行。

**OD-KM-505 知识推荐检索器 knowledge_link.rs**
- 架构契合：复用 vault_search.search_vault 词法检索 + 实体匹配，纯函数组合器；不建向量索引、不动 knowledge_vault 写路径。
- 实施步骤：① suggest_links(record_or_task, knowledge_hints, vault_root) → 候选列表（knowledge_id/title/匹配理由/原文定位 path+行号）；理由标注依据来源（AI 主题建议/记录关键词检索/实体关联）；② 按依据类型排序、去重、上限封顶；③ 候选仅供 507 确认流，本卡不落盘。
- 验收命令：cargo test --offline --lib（新增 ≥10 项全绿，测试用临时 vault 根与虚构条目）。
- 依赖：接口约定依赖 504 的 knowledge_hints 字段（可并行，联调在 507）；与 504/508 并行。

**OD-KM-508 项目 IPC 三命令**
- 架构契合：薄封装 502 实体；照 project_items_read/save 命令先例。
- 实施步骤：① project_create/project_update/project_list；② project_overview（502 反向派生：按项目返回任务/记录/素材/知识计数与列表，素材计数若 506 类型未就绪则接口定型+空实现）；③ project_id 存在性校验接入 Task 保存路径。
- 验收命令：cargo test --offline --lib；npx tsc --noEmit。
- 依赖：阻塞于 502/503；阻塞 512/513；与 504/505 并行。

### 批次三（并行）：506 ∥ 507

**OD-KM-506 素材 IPC 六命令**
- 架构契合：照 405/409 先例挂 lib.rs 命令注册 + api.ts/types.ts 成套封装；所有用户文本过 gate_user_text。
- 实施步骤：① material_extract_from_record（触发既有分析返回素材候选；AI 无直接落盘通道）；② material_create（用户主动收藏：正文+类型+可选来源，gate）；③ material_list（类型/状态/时间/项目/任务过滤）；④ material_get（含来源追溯 payload：源记录/任务/知识条目定位与引用原文）；⑤ material_review（confirm/reject/defer/archive + 退回/拒绝理由必填，追加审核记录段）；⑥ material_edit（正文/类型/标签编辑，gate，追加审核记录）。前端类型与调用封装成套。
- 验收命令：cargo test --offline --lib；npx tsc --noEmit；npm --prefix desktop run build。
- 依赖：阻塞于 501/504；阻塞 509/510/513；与 507 并行。

**OD-KM-507 知识关联 IPC 四命令**
- 架构契合：组合 505 检索器与 503 frontmatter 字段，照候选确认流先例（candidates→confirm）；知识库只读，不回写 vault。
- 实施步骤：① knowledge_link_suggest（对记录/任务组合 504 hints + 505 检索产出候选落暂存）；② knowledge_link_confirm（用户确认后写记录/任务 related_knowledge，理由 gate；候选转正照 307 先例返回半完成报告结构）；③ knowledge_link_remove（移除关联 + 记录留痕）；④ knowledge_link_by_knowledge（反向查询：知识条目 id → 引用它的记录/任务/素材列表）。
- 验收命令：cargo test --offline --lib；npx tsc --noEmit。
- 依赖：阻塞于 503/504/505；阻塞 511/513；与 506 并行。

### 批次四（可并行）：509 ∥ 510 ∥ 511 ∥ 512

**OD-KM-509 第四阶段接口 material_query**
- 架构契合：纯查询命令照 knowledge_quality_report 只读先例；本阶段交付接口层，UI 与第四阶段同源使用。
- 实施步骤：① material_query：输入写作主题关键词/素材类型/时间范围/项目范围/审核状态；默认只返回已确认素材；返回正文+来源（source_type/id/quote/location）+审核状态+关联实体（任务/项目/知识）；② 关键词检索用 vault_search 同款词法引擎作用于素材正文与标题（不外发）；③ 方案场景十二 payload 断言测试。
- 验收命令：cargo test --offline --lib（含场景十二 payload 结构测试）；npx tsc --noEmit。
- 依赖：阻塞于 506；阻塞 513；与 510/511/512 并行。

**OD-KM-510 素材篮页（WritingStudio 新页签）**
- 架构契合：照 TaskManager 页签组先例扩 WritingStudio；复用 506 类型封装；遵守 UI 常驻原则（不堆次要标签，布局传达信息）。
- 实施步骤：① 素材列表（类型分组 + 状态筛选，待审核置顶显式区分）；② 审核操作（批准/退回/拒绝/暂缓，退回与拒绝理由输入）就地完成，不单独建审核工作台页；③ 素材详情（正文/来源追溯「打开原文」跳原记录/任务/知识 + 引用原文展示）；④ 用户收藏新建入口；⑤ 动工前按 BUILDER 模板搜 2–3 个同类产品参考（借鉴要点 ≤2 句入任务记录）。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_material_ui_510.py（列表/详情/审核操作/空态真实断言）全绿。
- 依赖：阻塞于 506；与 511/512 并行。

**OD-KM-511 关联视图扩展（记录/任务详情 + 知识反向区块）**
- 架构契合：照 412 详情面板扩展先例追加区块；复用 SourcePeekSheet 展示知识原文定位。
- 实施步骤：① 工作记录详情 + 任务详情追加「关联知识」区块：AI 推荐候选（附理由与原文定位）、确认/移除操作、已关联清单；② KnowledgeVaultPage 条目详情追加「引用它的工作事实」反向区块（507 数据：记录/任务/素材列表，点击跳转）；③ AI 推荐与用户确认显式区分标注（照「用户指定」标注先例）。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_linkage_ui_511.py 全绿。
- 依赖：阻塞于 507；与 510/512 并行。

**OD-KM-512 项目成果视图（ProjectBoard 扩展）**
- 架构契合：ProjectBoard 追加项目概览区块，不重做事项表；复用 508 project_overview 与素材列表组件。
- 实施步骤：① 项目选择器 + 概览（目标/状态/起止 + 任务/记录/素材/知识计数与列表）；② 素材区块复用 510 列表组件按项目过滤；③ 从素材详情可跳项目；④ 空态真实（无项目/无关联各自空态）。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_project_ui_512.py 全绿。
- 依赖：阻塞于 508（素材计数若依赖 506 类型则该行延至 509 后联调）；与 510/511 并行。

### 收口

**OD-KM-513 全量回归 + 安全核对 + 阶段收口（照 OD-WR-308/OD-QS-413 模式）**
- 架构契合：照既有阶段收口先例，不改业务代码（发现缺陷回修不属本卡）；新增安全 verify 脚本与证据落盘。
- 实施步骤：① 全量命令跑通；② verify_security_phase3.py：素材提取闸门（gate/masker 覆盖新 IPC）、AI 无审核状态写通道、知识库写路径零改动（vault_write 不被素材流触碰）、素材篮清理排除生效、索引重建不损 work/ 数据（场景十一）、无敏感原文日志、测试数据全虚构（场景十敏感样本用合成数据）；③ 十二场景验收对照（方案 §十）：逐场景登记覆盖任务与证据位置，未验证事项如实登记（真实模型素材提取质量、推荐相关性、真实窗口目视、大量素材性能）；④ 证据落 tests/verification/desktop/evidence/od-km-513/。
- 验收命令：cargo test --offline --tests；pytest tests -q；npm --prefix desktop run build；npx tsc --noEmit；全部 verify（含 501/510/511/512/安全）全绿。
- 依赖：阻塞于 501–512，最后执行。

## 依赖图与派发节奏

```
501 ┬─ 504 ┬─ 506 ┬─ 509 ─┐
    │      │      ├─ 510 ─┤
    │      └─ 505 ┬─ 507 ─┼─ 511 ─┤
502 ┼─ 508 ────────┼─ 512 ─┤       ├─ 513
503 ┴──────────────┘       └───────┘
```

串行主线：501 → 504 → 506 → 510 → 513。派发模型：OpenDesign/deepseek-v4.1-flash-eco，思考等级 high；被路由拒绝则停下等用户确认。

## 风险清单

素材提取误判（提示词约束+quote 必填+content≠quote+低置信度待审核+用户审核兜底）；重复素材（指纹+AI match 建议+进展关联不覆盖）；推荐无关（依据类型标注+封顶+确认制+D-082 复审触发）；历史兼容（serde default+字节不变测试）；敏感外发（零新增模型调用+继承脱敏闸门+verify 断言）。

## 边界（不做）

不重建知识库与 Raw/Wiki/Governance/Index 分层；不建向量索引；不做文稿生成（第四阶段）；素材不进知识库；不接管 ProjectBoard 事项表；不修改 runtime/；AI 无任何直接写确认状态的通道。

