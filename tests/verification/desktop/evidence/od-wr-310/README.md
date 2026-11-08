# OD-WR-310 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 起点库单测：653 passed
- 交付后库单测：660 passed / 0 failed（本任务新增 7 项）

## 改动文件

- 新增 desktop/src-tauri/src/quicklog.rs：append / set_destination / redate / move_many / merge 改写到记录库；按 ISO 周聚合；渲染旧周文件形状供周索引读取。
- 修改 desktop/src-tauri/src/worklog_records.rs：记录 frontmatter 新增 destination。
- 修改 desktop/src-tauri/src/lib.rs：五个随手记命令改调 quicklog；read_week 优先从记录库聚合，保留旧周文件回退；archive_list 合并记录周。
- 修改 desktop/src/types.ts：WorkRecord / RecordDraft / RecordPatch 同步 destination。
- 新增 tests/verification/desktop/verify_worklog_unify_305.py：15 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --tests | 0 | 13 个测试目标全 ok；lib 660 passed / 0 failed |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功 |
| python3 tests/verification/desktop/verify_worklog_unify_305.py | 0 | 15/15 通过 |

原始输出：cargo-test-tests.log、tsc.log、npm-build.log、verify-worklog-unify.log、exit-*.txt。

## 行为

- 判定块协议与 IPC 参数/回执形状不变；界面仍按 week/date/index/expect_text 定位。
- 追加写 work/工作记录/；entry_index 是该日期下已有记录数；去向写入 frontmatter destination。
- 改去向、改日期、批量改日期、并入都直接更新记录；批量移动任一条失败会回滚本批日期改动。
- 周索引按 record_date 的 ISO 周聚合；read_week 把一周记录渲染成旧周文件形状，WeeklyTimeline 与 buildWorklogIndex 无需改协议。
- 旧周文件读函数保留；记录为空时回退旧文件，供迁移期与旧数据回看。
- archive_list 会把记录库中出现的周并入周列表。

## 真实数据

- 309 迁移出的 4 条 W45 记录在本轮补写了 destination（来自周文件存档去向），全部匹配。
- 周视图现在从记录库读取 W45；旧周文件已归档，不再写入。

## 已知限制

- worklog.rs 的旧写函数仍保留源码但不再被命令调用；彻底删除留给后续清理。
- 周报正文生成与周视图的深度切换仍属 311；本轮只保证周索引与周内容聚合。
- 跨文件批量移动不是事务，失败时只回滚 record_date 字段。
- 真实窗口未目视核对。
