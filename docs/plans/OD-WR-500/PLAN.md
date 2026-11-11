# 工作记录模块分阶段实施 Plan（OD-WR-500 规划成果八/九/十）

> 派发：子代理 deepseek-v4.1-flash-eco / high，单卡单会话；ARCHITECT 逐卡独立复检。批次 WR-501 → WR-506 → WR-507 → WR-508。

---

**OD-WR-501 原始素材分离 + 修订历史**
- 架构契合：照 OD-KM-503 serde default 先例；revisions.jsonl 照 citations 先例。
- 允许修改：worklog_records.rs、quicklog.rs、相关测试；禁止：UI 层、task_extract、知识库。
- 步骤：① WorkRecord 增 raw_input/raw_source（serde default）；② RecordDraft/RecordPatch 同步；③ quicklog 落盘时写原始输入与来源；④ update_record 每次修订追加 revisions.jsonl（work/工作记录/<id>/）；⑤ 字节不变测试 + 单测 ≥8 项。
- 验收：cargo test --offline --lib（0 failed）。
- 回归：现有记录库全部测试不回归。

**OD-WR-502 记录详情修订历史 UI + 重新整理**
- 架构契合：WorkRecordLibrary 详情栏扩展。
- 允许修改：WorkRecordLibrary.tsx、api.ts/types.ts；禁止：引擎协议。
- 步骤：① 详情栏显示 raw_input（折叠「原始输入」区块，只读标注）与修订历史列表；② 「重新整理」按钮：携带 raw_input 重新调用 memory-agent 整理 → 候选确认后更新 body（追加修订记录）；③ 类型快捷 chips（建议五类，可输入自定义）。
- 验收：tsc/build 0；新增 verify 脚本全绿。
- 依赖：501。

**OD-WR-503 当日摘要**
- 架构契合：新 Skill daily-summary + quicklog 式宿主校验落盘；摘要存储 work/摘要/。
- 允许修改：skills/daily-summary/（新建）、worklog_records.rs 或新 daily_summary.rs、quicklog/IPC、HomeChat intent、测试；禁止：知识库、任务实体。
- 步骤：① intent 增 daily-summary（封闭词表：生成今日摘要/总结今天/今日摘要）；② 宿主汇集当天记录 → daily-summary Skill → 校验（五段+引用 record_ids 必须真实存在）→ 落盘 work/摘要/YYYY-MM-DD.md（重新生成需确认，旧版 .bak）；③ IPC + 前端时间线组头「当日摘要」入口与展示（引用记录可点开）。
- 验收：pytest（Skill 单测）+ cargo（落盘/校验）+ tsc/build + verify 全绿。
- 依赖：501。

**OD-WR-504 时间线视图改造**
- 架构契合：WorkRecordLibrary 列表改造为按日分组时间线；样式照 Neo-Skeuomorphism 2.0。
- 允许修改：WorkRecordLibrary.tsx、theme.css、verify；禁止：数据层。
- 步骤：① 按日期倒序分组渲染（组头=日期+记录数+摘要入口）；② 卡片展示任务书模块 A 七要素；③ 搜索/筛选保留；④ 旧消息兼容。
- 验收：tsc/build + verify（分组/卡片要素/兼容断言）全绿。
- 依赖：503（摘要入口）。

**OD-WR-505 周期复盘引擎 + Skill**
- 架构契合：memory-report 升级为参数化 period-review；复盘存储 work/复盘/；宿主汇集器照当日摘要模式。
- 允许修改：skills/memory-report/、新 review_store.rs 或并入 worklog、HomeChat intent、IPC、测试；禁止：知识库写路径、任务创建。
- 步骤：① intent 增 period-review 封闭词表（月度总结/月报/季度复盘/年度总结/复盘本月 等）；② 宿主汇集：周期内记录/任务状态统计/已确认成果/既往复盘标题；③ Skill 按 period 深度模板生成初稿 → 校验（引用 ids 真实）→ 落盘草稿；④ 确认流：status=已确认 + confirmed_at；重生成只对草稿；⑤ 周报兼容：memory-report 旧入口保持，词表命中「周报」仍走周流程（复用新引擎 period=weekly）。
- 验收：pytest + cargo + verify（含「已确认不覆盖」断言）全绿。
- 依赖：503。

**OD-WR-506 复盘历史档案页 + 记录↔文稿关联**
- 架构契合：WorkRecordPage「复盘」页签（历史列表+详情抽屉）。
- 允许修改：WorkRecordPage.tsx、新 ReviewArchive 组件、api/types、worklog（document_ids 反向查询）；禁止：引擎协议。
- 步骤：① 档案列表（周期类型筛选/时间筛选）；② 详情（版本[草稿/确认]/引用记录/关联任务/导出 MD）；③ 重新生成（生成新草稿不覆盖确认版）；④ 记录详情关联文稿标识（复盘/素材链路反向）。
- 验收：tsc/build + verify 全绿。
- 依赖：505。

**OD-WR-507 「交给 AI 继续分析」+ 全局衔接**
- 步骤：① 记录/复盘详情加「交给 AI 继续分析」：携带对象 id 转首页对话并预填上下文（照 341 行既有预填模式）；② 全局对话采集保持保守不扩（登记为设计决策，理由：避免任何输入误记）。
- 验收：tsc/build + verify 全绿。
- 依赖：504/506。

**OD-WR-508 收口：全量回归 + 安全核对 + HANDOFF**
- 步骤：① 全量命令；② verify_security_wr5：原始素材不被修订覆盖断言、AI 无确认态写通道、复盘已确认不覆盖、引用 ids 真实性、测试全虚构；③ HANDOFF 文档；④ 证据落 od-wr-508/。
- 验收：cargo --tests + pytest + build + tsc + 全部 verify 全绿。
- 依赖：501-507。

---

**风险**：intent 词表误判（保守封闭枚举+现有 note 兜底）；旧周文件兼容（迁移已完成，只读不回写）；摘要/复盘引用失效（stale 标记）；重新生成覆盖（只对草稿，已确认生成新文件）。
**明确不做**：不建分页聊天框；不改知识库定位；不自动建任务；不做全文多版本存储；不为视觉重构架构。

