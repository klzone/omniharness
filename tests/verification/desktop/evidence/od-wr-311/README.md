# OD-WR-311 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：d8486784c8df94b17f45f8bdabdb7c7f58e39f70（main，310 提交后）
- 起点库单测：660 passed
- 交付后库单测：660 passed / 0 failed（本任务无新增 cargo 用例）

## 改动文件

- desktop/src-tauri/src/quicklog.rs：新增 WeekView / week_view，按记录库 ISO 周返回结构化条目。
- desktop/src-tauri/src/lib.rs：新增 worklog_week_view IPC 并注册。
- desktop/src/api.ts、desktop/src/types.ts：结构化周视图客户端与类型。
- desktop/src/weekTimeline.ts：parseWeekView 把记录条目转成与 parseWorklog 相同的时间线结构。
- desktop/src/shell/ScenePage.tsx：时间线与四象限优先消费结构化记录；记录为空或读取失败时才回退旧周文件；周报读取仍走 worklogReadWeek（310 已改为记录库渲染）。
- desktop/src/shell/WeeklyTimeline.tsx：新增可选 parsed 入参，优先使用结构化条目，不再只依赖 Markdown。
- 新增 tests/verification/desktop/verify_week_from_records_306.py：12 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 660 passed / 0 failed |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功（586ms） |
| python3 tests/verification/desktop/verify_week_from_records_306.py | 0 | 12/12 通过 |

原始输出：cargo-test-lib.log、tsc.log、npm-build.log、verify-week-from-records.log、exit-*.txt。

## 数据源切换

- ScenePage 先调用 worklog_week_view；有记录时直接使用结构化条目，时间线、四象限、指标带都从同一份解析结果算。
- 只有记录为空或 view 读取失败时才调用 worklog_read_week 回退旧周文件；迁移期旧数据仍可看。
- 周报（HomeChat report）仍调用 worklog_read_week；310 已让该命令优先渲染记录库，因此周报源同样是记录库。
- 真实 W45 的 4 条记录由记录库驱动周视图；旧周文件只在存档里。

## 已知限制

- 旧周文件回退保留，直到旧数据全部迁移并可确认删除。
- 周报提示词仍接收渲染后的 Markdown，不是结构化字段；这是为保持既有模型协议不变。
- 真实窗口未目视核对。
