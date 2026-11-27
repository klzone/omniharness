# Task、Project、四象限及排期数据架构评估（Step 4）

- 日期：2026-11-27
- 问题：数据结构、存储方式、关联关系、任务状态、执行记录、访问方式、多终端并发风险。

## 1. 数据结构（static，task_store.rs）

任务文件：`<work_root>/任务/<task_id>.md`，三段式 Markdown + frontmatter：

- frontmatter（schema omnidesk.task.v1）：id、title、state、created_at、updated_at、source_record_id、source_quote、linked_record_ids、category、due_date、hint、importance、importance_source、importance_reason、due_kind、depends_on、estimated_minutes、parent_id、quadrant_override、project_id（本机真实任务文件证实，static）。
- 正文段：## 进展（追加条目）、## 状态历史（追加条目）、## 调整记录。
- 状态白名单：待确认/待开始/进行中/已完成/已取消/暂停。
- AI 任务候选：`<work_root>/.candidates/*.json`，防重键 (record_id, source_quote 哈希)，确认后转正式任务。

其他业务实体：

| 实体 | 存储 | 说明 |
| --- | --- | --- |
| 项目 | work/项目/<id>.md | project_store.rs / project_items.rs |
| 工作记录 | work/工作记录/<wr-id>.md（含子目录） | worklog_records.rs |
| 排期 | schedule_store.rs（work/ 下文件） | 与任务 via due_date/estimated_minutes 关联 |
| 事件表 | work/events.jsonl（追加式，单写者，seq 进程内串行） | event_log.rs |
| 复盘 | work/复盘/weekly、项目 | consolidation.rs |
| 素材篮 | work/素材篮/ | writing_collect.rs |

## 2. 数据关联与访问（static）

- 任务→项目：task.project_id。
- 任务→工作记录：source_record_id + linked_record_ids。
- 候选→任务：candidate_dedup_key 防重。
- 事件→实体：entity{kind,id} + ref{kind,id}，detail 只允许枚举短语（不存正文）。
- 访问方式：全部经 Rust 宿主命令（IPC-1），UI 不直接读写文件；写入用原子替换（write_new_atomically / write_replace_atomically）。

## 3. 执行记录（静态 + 台账双层）

1. 业务层：任务 Markdown 内的进展/状态历史 + events.jsonl。
2. 引擎层：engine-state/omni-*.json 任务台账（60+ 个）+ results/ 结果正文 + events 游标。
两层通过 omni_task_id 与任务/场景关联。

## 4. 多终端并发风险

| 风险 | 评估 |
| --- | --- |
| 双端同时改同一任务文件 | 当前原子写是"最后写者赢"。宿主写前有哈希检查的主要是 vault 笔记；task_store 的替换写没有跨进程版本协商。远程化后需要服务层乐观并发（版本号/etag + 409），文件哈希做兜底 |
| 状态迁移竞态 | 状态白名单在写入口校验，但两个合法迁移并发可能交错（待开始→进行中 vs 待开始→已取消）。单中央写口 + 事件表 seq 可定位，需在 API 层拒绝基于过期快照的迁移 |
| events.jsonl 双写 | 明确单进程假设；多终端必须收敛到单中央写口 |
| 排期与任务不一致 | 两者同属宿主文件写口，单中央写口下风险可控 |

## 5. 四象限现状

任务有 importance 与 quadrant_override 字段，scheduler.rs / priority_rules.rs 存在排期与优先级规则。**四象限视图是否已完整产品化未逐行核对，标记待验证**（验证方法：跑 UI 检查 ProjectBoard/WeeklyTimeline 是否含四象限视图）。这不影响远程化架构判断：四象限是任务字段的投影视图，远程化只需把任务读写 API 化。

## 6. 结论

- 任务/项目/排期/事件全部是文件优先的 Markdown/JSON，权威源清晰，适合直接作为中央数据（Mac mini 本地文件）。
- 远程化核心是把宿主的文件读写口暴露成带鉴权的 API 服务，并在服务层加乐观并发。
- 无需数据库迁移；SQLite 属可选派生索引（全文检索/联表查询加速），首期不引入。
