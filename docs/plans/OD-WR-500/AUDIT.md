# 工作记录模块现状审计报告（OD-WR-500，2026-11-11）

> 审计方式：实读源码与真实数据目录，不依据历史文档推断。

## 一、已实现能力清单

### 数据层
- WorkRecord 实体（worklog_records.rs）：id/title/record_date(YYYY-MM-DD)/created_at/updated_at/category/tags/source/destination/ai_analysis_status/linked_task_ids/draft/related_knowledge(OD-KM-503)/body。serde default，旧文件兼容。
- 存储：work/工作记录/ 每条记录一个 .md（wr-<ts>-<n>.md），frontmatter+正文；删除走 work/.trash/。
- 记录库 CRUD 完整：create/read/list(RecordFilter 日期闭区间)/update/delete；周索引按 ISO 周聚合（quicklog.rs）。
- 旧周文件（库/工作记录/YYYY-Wxx.md）已迁移（OD-WR-310），周报在 库/周报/。
- 关联：linked_task_ids（任务双向回写 OD-WR-307）、related_knowledge（OD-KM-503 稳定 id+理由+时间）。

### AI 采集链路
- 首页 HomeChat：工作记录场景（preset=weekly）下 detectWorkIntent 判定 note/report（workIntent.ts 纯函数）。
- note → quicklog 链路（OD-ARCH-104/WR-310）：读当周索引 → memory-agent Skill 判语义 → 宿主校验 → 落盘；判定前不写任何文件；缺项只问（Skill 约束）。
- report → memory-report 周报起草：读当周记录、只取「本周报引用」条目、五段模板、全文先给用户确认、确认后才落盘 库/周报/。
- AI 任务提取：task_extract 链路（OD-WR-301~306）从记录提取任务候选，审核台确认。

### UI 层
- WorkRecordPage 四页签：记录（WorkRecordLibrary 记录库）/ 审核（TaskReviewPanel 候选审核）/ 任务（TaskManager）/ 周视图（ScenePage 按周档案）。
- WorkRecordLibrary：列表+分类筛选（category 自由文本，动态收集选项）+状态筛选（ai_analysis_status）+详情侧栏（就地编辑 title/date/category/tags/body）+ broken 文件计数。
- destination「本周报引用」标记（OD-WR-310）。

## 二、与任务书对照的缺口

| # | 任务书要求 | 现状 | 缺口 |
|---|-----------|------|------|
| 1 | 原始输入与正式记录分离 | quicklog 判定后 body 即正式内容，原始输入未单独保存；source 只记迁移来源 | 需新增原始素材字段/文件 |
| 2 | 当日摘要 | 无 | 需新增（Skill+聚合+存储） |
| 3 | 时间线按日期分组视图 | 列表平铺+筛选，无按日分组时间线 | UI 改造 |
| 4 | 详情弹窗/抽屉 | 右侧常驻详情栏（非弹窗） | 交互改造（可选，侧栏可用） |
| 5 | 月/季/年复盘 + 自定义范围 | 仅周报（memory-report） | 需新增 period-review Skill 与链路 |
| 6 | 复盘历史档案（版本/引用/重新生成不覆盖） | 周报无版本管理、无档案页 | 需新增 |
| 7 | 首页全局对话采集 | 仅工作记录场景（preset=weekly）触发 intent；全局对话不采集 | 需扩展 intent 判定范围（保守判定） |
| 8 | 关联文稿 | 文稿可引用记录（第四阶段素材链路已通），记录侧无文稿反向标识 | 轻量补齐（related_documents 或经素材篮间接关联） |

## 三、结论

骨架完整（实体/CRUD/采集/任务提取/周报/关联），缺口集中在：原始素材分离、当日摘要、周期复盘扩展（月季年）、复盘档案、时间线 UI。全部可在现有架构上增量实现，无需重建。

