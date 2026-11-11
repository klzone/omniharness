# 工作记录模块产品需求与信息架构 + 数据模型与生命周期设计（OD-WR-500 规划成果二/三）

## 一、信息架构（IA）

工作记录分页（WorkRecordPage）调整为三页签（既有四页签收敛）：

1. **时间线**（默认，改造自现有记录库）：按日期倒序分组的时间线；组头=日期+当日摘要入口；条目=记录卡片（标题/时间/类型/摘要/来源/关联/确认状态）。保留月份筛选、关键词搜索、类型筛选（category 动态收集，支持自定义分类，不写死组件）。
2. **复盘**（新增）：周期选择（周/月/季/年/自定义范围）→ 生成流程 → 复盘历史档案列表。
3. **审核**（保留）：AI 任务候选审核台。

任务页签保留在任务场景（不重复）。周视图（ScenePage）保留为「周档案」入口从时间线组头进入。

## 二、数据模型与生命周期

### 原始素材与正式记录分离（任务书约束 4）

WorkRecord 增两个 serde default 字段（旧文件零迁移）：
- raw_input: Option<String> —— 首页对话的原始输入原文（模型整理前）；手工/迁移记录为 None。
- raw_source: Option<String> —— 来源描述（"首页对话 2026-11-11 09:30" / "周文件迁移:2026-W45"）。

生命周期：raw_input 只在创建时写入，后续任何修订/重新整理都不改写（重新整理只更新 body 并在修订记录注明"依据原始素材重新整理"）。正文（body）= AI 整理后可人工修订的正式内容。

### 修订历史（详情要求 8）

work/工作记录/<id>/revisions.jsonl（追加式，照 citations 先例）：每次 update_record 追加 {at, field_count, old_body_sha, note}；正文大版本以 sha 锚定，详情可对比当前与最近一次。不做全文多版本存储（临时层轻量优先）。

### 当日摘要（模块 C）

work/摘要/YYYY-MM-DD.md（frontmatter: schema omnidesk.daily-summary.v1, date, record_ids[], generated_at, status）。AI 依据当天正式记录生成五段（概况/重点/思考/待跟进/任务关联），每段附引用 record_ids；record_ids 引用不存在时摘要失效标 stale。生成只由用户点「生成当日摘要」触发；已有摘要重新生成需确认，不覆盖（旧版 .bak）。

### 周期复盘（模块 D/E）

work/复盘/<period>-<key>.md（如 weekly-2026-W46 / monthly-2026-11 / quarterly-2026-Q4 / annual-2026 / custom-<ts>；schema omnidesk.review.v1）。frontmatter: period, range_start/range_end, status(草稿|已确认), record_ids[], task_ids[], document_ids[], generated_at, confirmed_at, revision。

流程：选周期 → 宿主汇集（记录 body 索引、任务状态统计、已确认成果、既往复盘标题）→ AI 按周期深度模板生成初稿 → 用户审核修订 → 确认归档（status=已确认，confirmed_at 落盘）。重新生成只在草稿态进行；已确认复盘重生成生成新草稿文件（旧确认版不动）。周/月/季/年模板深度不同（Skill 定义）。

### 记录类型（模块 A）

category 保持自由文本 + 动态收集；建议值（工作事项/学习记录/工作思考/问题与经验/个人生活）写入 Skill 提示词与 UI 快捷 chips，不锁死组件。新分类随使用自然出现。

## 三、跨模块关联

- 记录↔任务：linked_task_ids 既有双向回写，维持；复盘的新行动建议 → 转为任务候选（走既有审核台，AI 不直接建任务）。
- 记录↔知识：related_knowledge 既有；复盘/记录推荐进知识库走既有知识治理流程。
- 记录↔文稿：文稿素材链路已通（素材篮/记录引用）；复盘归档文档 document_ids 记录关联；第四阶段归档通道复用。

## 四、首页 AI 对话与 Skill 交互

- intent 判定扩展（保守原则不变）：note/report 之外新增 "daily-summary"（生成今日摘要/总结今天）与 "period-review"（月度总结/季度复盘/年度总结/复盘本月 等，词表照 REPORT_KEYWORDS 模式封闭枚举）。工作记录场景触发；全局对话首期不自动采集（约束 1 的保守实现：全局对话维持普通对话，采集只在明确的工作记录场景与专业操作内发生——避免任何输入被误记）。
- Skill 合并决策（已核对现有资产 memory-agent/memory-report）：
  - work-record-organizer：不新建——memory-agent 即是，扩展提示词补记录类型建议与原始素材透传。
  - daily-summary：新建（生成当日摘要，读当日记录，输出五段+引用）。
  - period-review：新建一个（参数化周期），替代拆成四个——周复盘即 memory-report 的升级版（保留 memory-report 名字兼容，提示词改为周期参数化；月/季/年由同一 Skill 按模板深度区分）。避免四个近似 Skill 的维护负担。
- 分页专业操作：整理记录/生成当日摘要/生成周期复盘/继续讨论（携带对象 id 转首页对话并预填上下文）——沿用 HomeChat 既有 submit 通道与 notePending 回执机制。

