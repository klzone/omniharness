# 第四阶段实施计划与任务卡（OD-WT-600 规划成果四）

> 状态：已批准（2026-11-09，15 卡版本）。任务卡按 docs/development/IMPLEMENTED_RULES.md §1 模板。派发模型：OpenDesign/deepseek-v4.1-flash-eco，思考等级 high。

## 阶段目标

建立与 OmniDesk 业务数据深度关联的个人 AI 写作工作台：写作任务 → 素材收集 → 大纲确认 → 分章节写作 → 事实核验 → 审阅修订 → 定稿 → 导出（MD/Word 公文版/PDF 阅读版/公众号 HTML）→ 归档（MD 进知识库）→ 知识回流。

## 任务卡（OD-WT-601 起）

### 批次一（并行）：601 ∥ 603，602 随后

**OD-WT-601 写作任务实体 writing_store.rs**
- 架构契合：照 material_store.rs 模式新增独立存储模块，Markdown+frontmatter+原子写，不引入数据库，不改前三阶段交付结构。
- 实施步骤：① writing_store.rs：WritingTask 结构（writing_id/title/document_type/writing_purpose/audience/writing_period/topic/requirements/expected_length/writing_style/selected_material_ids/selected_knowledge_ids/document_id/status/created_at/updated_at）、九态白名单（需求待整理/素材收集中/大纲待确认/写作中/待核验/待审核/已定稿/已归档/已取消）、document_type 白名单（十二类起步：月度总结/年度总结/专项汇报/经验交流/典型案例/调研报告/工作方案/领导讲话/工作简报/专题分析/会议发言/个人述职）；② CRUD+按状态/类型/时间过滤；③ 状态迁移合法性表（大纲待确认→写作中须经大纲确认动作；归档仅从已定稿；取消可从多数状态进入），非法迁移拒绝并留痕；④ 「## 状态记录」追加段；⑤ work_cleanup 排除 work/写作/（有测试）。
- 验收命令：cargo test --offline --lib（新增 ≥14 项全绿）；python3 tests/verification/desktop/verify_writing_store_601.py 全绿。
- 依赖：无前置；阻塞 602/604/605/606；与 603 并行。

**OD-WT-603 引用与核验记录 writing_records.rs**
- 架构契合：追加式 JSONL（照 507 移除记录先例），独立小模块。
- 实施步骤：① CitationRecord（id/章节号+段落序号/来源类型 material|knowledge/来源 id/引用原文/时间）与 FactCheckRecord（核验对象/结论八类白名单/依据/处理状态/时间）结构；② 追加/按段落查询/按来源查询；③ 全量重算骨架（从正文段落+素材引用重推，纯函数）；④ 处理状态仅用户可改（枚举校验）。
- 验收命令：cargo test --offline --lib（新增 ≥10 项全绿）。
- 依赖：无前置；阻塞 609；与 601 并行。

**OD-WT-602 文稿与版本存储 document_store.rs**
- 架构契合：work/写作/<wid>/ 目录结构，文件优先；复用 frontmatter 纪律与 sha256.rs。
- 实施步骤：① outline.md/正文.md 读写（正文更新必须经版本快照函数）；② 版本快照/列表/读取/恢复（恢复=旧版内容落为新当前版+新快照，历史不删）；③ manifest.json 读写（版本号/时间/来源 user|ai/范围/说明/导出回执清单）；④ 段落级差异对比（空行分段比对，输出变更段落序号与前后文，不引外部 diff 库）。
- 验收命令：cargo test --offline --lib（新增 ≥12 项全绿，含快照-恢复-不丢历史）。
- 依赖：阻塞于 601；阻塞 606/607/608；与 603 并行。

### 批次二（并行）：604 ∥ 605 → 606

**OD-WT-604 写作需求解析（零新增模型通道）**
- 架构契合：照 504/506 先例：engine 侧 writing_extract.py 提示词+validate；宿主侧校验函数；模型调用在界面侧。
- 实施步骤：① 引擎侧解析输出（document_type/主题/目的/受众/时间范围/篇幅/重点/风格/缺失项 questions）；提示词约束：缺失关键条件列入 questions 不臆测；② 宿主校验（白名单+缺失项必输出）；③ writing_parse_requirement IPC：返回结构化草稿+问题清单，用户确认后创建 WritingTask；④ 前端类型封装。
- 验收命令：pytest tests -q（200+ 新增全绿）；cargo test --offline --lib；npx tsc --noEmit。
- 依赖：阻塞于 601；与 605 并行；阻塞 610。

**OD-WT-605 智能素材收集 IPC**
- 架构契合：组合 509 material_query + knowledge_link/vault_search + 508 project_overview，纯查询组装器。
- 实施步骤：① writing_collect_materials IPC：输入 writing_id+过滤条件，输出推荐素材（内容/来源/时间/审核状态/关联/推荐理由/数据冲突提示/是否适合直接引用）；理由按依据类型标注（D-079 口径）；② writing_select_materials：用户采纳/排除/标注重点，写回 frontmatter selected_* 字段；③ 五类区分标注落返回结构；④ 场景三断言：未确认素材不进可信集合。
- 验收命令：cargo test --offline --lib（新增 ≥8 项含场景三）；npx tsc --noEmit。
- 依赖：阻塞于 601；与 604 并行；阻塞 606/610。

**OD-WT-606 大纲生成与确认 IPC**
- 架构契合：宿主组大纲提示词（需求+素材包+类型模板）；模板数据 config/writing-templates.v1.json（照 flow_pack 仓库外可编辑先例）。
- 实施步骤：① writing_outline_generate：校验素材包非空→构建提示词（AnalyzePrompt 形状）→大纲 JSON 校验（标题/目的/核心观点/章节树/各章重点/素材映射/预计篇幅）；② 大纲候选暂存→writing_outline_confirm（可编辑后确认）→outline.md+状态迁移+版本快照；③ 四套类型模板（总结/交流/调研/汇报）数据文件+校验；④ 场景四测试。
- 验收命令：cargo test --offline --lib（新增 ≥8 项）；pytest tests -q；npx tsc --noEmit。
- 依赖：阻塞于 601/602/605；阻塞 607。

### 批次三：607 → 608 ∥ 609

**OD-WT-607 分章节写作 IPC**
- 架构契合：照 606 模式；每章提示词=确认大纲节+该章素材+知识依据+上下章摘要（不全量塞历史）；material-writing Skill 四分法进提示词。
- 实施步骤：① writing_section_generate（校验大纲已确认，否则拒绝）；② writing_section_confirm：章节稿→正文对应节+版本快照+citations.jsonl 本章引用；③ 全文组合函数（大纲顺序拼接，未确认章节显式占位）；④ writing_section_revise：段落/章节修改指令（精简/扩展/语气/术语/消除重复），修订建议候选，应用时新建快照原文保留；⑤ 提示词禁止清单照方案 §六。
- 验收命令：cargo test --offline --lib（新增 ≥12 项含场景五）；pytest tests -q；npx tsc --noEmit；npm --prefix desktop run build。
- 依赖：阻塞于 606；阻塞 608/609/611。

**OD-WT-608 审阅修改 IPC**
- 架构契合：薄封装 607 revise + 603 记录；十维审阅检查单作为 AI 审阅提示词。
- 实施步骤：① writing_review IPC（十维检查单，问题清单定位+维度+建议）；② 修订应用统一走 607 快照机制；③ writing_version_list/diff/restore IPC（恢复需确认）。
- 验收命令：cargo test --offline --lib（新增 ≥8 项含场景八：多次修改版本差异可查、历史可恢复）。
- 依赖：阻塞于 607；与 609 并行。

**OD-WT-609 事实核验引擎 fact_check.rs**
- 架构契合：确定性引擎纯函数（照 priority_rules 先例）+ 603 记录落盘；AI 分类标注搭写作调用顺车。
- 实施步骤：① 确定性核验：提取正文数字/日期/机构名/人员职务 → 与素材 source_quote 及知识引用规范化匹配（复用 501 归一化）→ 八类结论；② 口径冲突检测（场景七，不合并只提示）；③ 结果定位章节+段落序号落 factcheck.jsonl；④ writing_factcheck_run + writing_factcheck_resolve（用户处理）；⑤ AI 分类标注（事实/分析/判断/计划/概括）章节生成顺带产出核验时合并；⑥ 场景六测试（正确数据→直接来源、错误数据→冲突、无来源→无法核实不判已核实）。
- 验收命令：cargo test --offline --lib（新增 ≥14 项覆盖场景六/七）；python3 tests/verification/desktop/verify_fact_check_609.py 全绿。
- 依赖：阻塞于 603/607；与 608 并行。

### 批次四（并行）：610 ∥ 611 ∥ 612 → 613 → 614

**OD-WT-610 写作工作台页（WritingStudio 扩展）**
- 架构契合：照 510 页签先例扩 WritingStudio；UI 常驻原则；动工前搜 2-3 个同类产品参考（≤2 句要点）。
- 实施步骤：① 写作任务列表（状态分组+类型筛选）；② 新建流程：自然语言→604 解析→问题清单补全→创建任务；③ 任务详情入口；④ 空态真实。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_writing_ui_610.py 全绿。
- 依赖：阻塞于 604/605/606；与 611/612 并行。

**OD-WT-611 大纲编辑器 + 分章写作界面**
- 架构契合：三栏布局（左章节树/中编辑器/右素材依据——方案 §七）；复用 SourcePeekSheet；章节确认状态显式标注。
- 实施步骤：① 大纲树编辑（确认前可改，确认后锁定需显式重开）；② 章节编辑器（生成/确认/修改指令入口、AI 建议与用户内容区分标注）；③ 右栏本章素材（原文可点开）与引用列表。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_writing_editor_611.py 全绿。
- 依赖：阻塞于 607；与 610/612 并行。

**OD-WT-612 核验面板 + 版本管理界面**
- 架构契合：核验面板照 411 冲突显式先例；版本列表照 409 排期历史先例。
- 实施步骤：① 核验面板（八类筛选/定位跳转/处理操作）；② 版本面板（列表/段落级对比/恢复确认弹窗）；③ 定稿确认动作（状态→已定稿+快照）。
- 验收命令：npm --prefix desktop run build；npx tsc --noEmit；python3 tests/verification/desktop/verify_writing_review_612.py 全绿。
- 依赖：阻塞于 608/609；与 610/611 并行；阻塞 613。

**OD-WT-613 导出引擎与样式模板**
- 架构契合：MD 导出纯宿主函数；DOCX 复用 DSH office-docx 技能（D-042）优先 + textutil 兜底；PDF 走 soffice headless/打印流；公众号 HTML 宿主确定性渲染；样式包 config/writing-export-styles/（照 flow_pack 先例）。
- 实施步骤：① export_engine.rs：MD 三态导出（正式稿剥离内部 ID/AI 调试信息断言；审阅稿带来源标记；工作稿含核验结果）；② 公文 DOCX 双路径（office-docx 技能转换+textutil 简化版如实标注）；③ PDF 转换；④ 公众号 HTML 模板渲染；⑤ 三套样式模板文件+加载校验；⑥ 导出前敏感词扫描（场景十三）；⑦ 导出回执写 manifest；⑧ writing_export IPC+前端封装。
- 验收命令：cargo test --offline --lib（新增 ≥14 项）；npx tsc --noEmit；python3 tests/verification/desktop/verify_writing_export_613.py 全绿（含真实产出文件结构断言）。
- 依赖：阻塞于 609/612；阻塞 614。

**OD-WT-614 归档与知识回流**
- 架构契合：vault_archive 新增文稿入口（照 archive_artifact_text 模式）；知识回流走既有治理流程。
- 实施步骤：① writing_archive IPC：校验已定稿→MD 干净版走 vault 报告目录→归档记录关联写作任务/素材/知识/核验结果/版本历史/导出清单（场景十）；② 归档页：导出清单展示（软引导先导出后归档，硬门禁仅校验已定稿）+ 归档确认 + 知识回流入口（用户勾选进治理流程，场景十一）；③ 状态→已归档终态。
- 验收命令：cargo test --offline --lib（新增 ≥8 项）；npx tsc --noEmit；python3 tests/verification/desktop/verify_writing_archive_614.py 全绿。
- 依赖：阻塞于 613；阻塞 615。

### 收口

**OD-WT-615 全量回归 + 安全核对 + 阶段收口**
- 架构契合：照 513/413 模式；不改业务代码。
- 实施步骤：① 全量命令；② verify_security_phase4.py：新 IPC 闸门覆盖、AI 无定稿/归档/导出写通道（三项仅用户触发）、大纲未确认不得生成正文（断言）、未确认素材不进事实集合（场景三）、导出敏感检查+导出不外发（本地处理无网络调用断言）、归档仅 MD（DOCX/PDF/HTML 不入 vault 断言）、无敏感日志、测试全虚构、无新增豁免；③ 十三场景对照表落 evidence README；未验证事项如实登记（真实模型写作质量、真实窗口目视、长文稿多章性能、DOCX 真实格式兼容、office-docx 技能真实会话调用）；④ 证据落 evidence/od-wt-615/。
- 验收命令：cargo test --offline --tests；pytest tests -q；npm --prefix desktop run build；npx tsc --noEmit；全部 verify 全绿。
- 依赖：阻塞于 601-614，最后执行。

## 依赖图与派发节奏

批次一：601 ∥ 603 → 602。批次二：604 ∥ 605 → 606。批次三：607 → 608 ∥ 609。批次四：610 ∥ 611 ∥ 612 → 613 → 614。收口：615。
串行主线：601 → 602 → 606 → 607 → 609 → 612 → 613 → 614 → 615。

## 风险清单

AI 虚构成果/领导讲话（禁止清单+四分法+核验引擎+用户定稿兜底）；长文稿上下文超限（分章+按需加载+上下章摘要）；版本膨胀（关键节点快照）；口径合并错误（冲突提示不合并）；未确认素材混入（默认已确认+场景三断言）；导出格式兼容（office-docx 优先 textutil 兜底，如实标注）；office-docx 技能需引擎通道（模板+内容注入为确定性操作，若实测须模型轮次按 D-088 模式）。

## 边界（不做）

不重建前三阶段；不做多人协作/复杂排版/自动发布/公文流转；外部搜索不作为正式事实；Token 消耗不估算（引擎不报 usage，只记调用次数）。

