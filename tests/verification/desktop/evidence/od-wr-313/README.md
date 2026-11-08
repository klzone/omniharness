# OD-WR-313 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：d8486784c8df94b17f45f8bdabdb7c7f58e39f70（main，311 前）
- 起点库单测：666 passed
- 交付后库单测：672 passed / 0 failed（本任务新增 6 项）

## 改动文件

- 新增 desktop/src-tauri/src/work_cleanup.rs：候选圈定、retention 配置、预览、执行移入回收目录，6 项单测。
- desktop/src-tauri/src/lib.rs：work_cleanup_preview / work_cleanup_apply IPC 与注册。
- desktop/src/api.ts、desktop/src/types.ts：清理预览/执行客户端与类型。
- desktop/src/SettingsPanel.tsx：临时层清理的预览与勾选确认入口。
- 新增 tests/verification/desktop/verify_work_cleanup_307.py：15 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 672 passed / 0 failed |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功（804ms） |
| python3 tests/verification/desktop/verify_work_cleanup_307.py | 0 | 15/15 通过 |

原始输出：cargo-test-lib.log、tsc.log、npm-build.log、verify-work-cleanup.log、exit-*.txt；库测试无 warning。

## 候选规则

- 记录：record_date 落在已归档报告 source_period 覆盖的周/月/年区间内。
- 任务：state 为已完成或已取消，且 updated_at 超过 retention 天数。
- 永不入选：draft 记录、被未完成任务引用的记录、还有待审核候选的记录。
- 保留期：work/retention.v1.json 的 days，默认 90，范围 1–3650；配置损坏时整次拒绝。

## 行为

- 预览：只读记录/任务/报告 frontmatter 与候选目录，不创建 .trash、不写 retention 文件。
- 执行：重新计算候选后再逐项 move 到 work/.trash/工作记录或任务；不做直接删除。
- 必须显式 confirm=true；启动流程不自动清理。
- 部分移动失败会如实列在 failures，ok=false。

## 已知限制

- 本轮没有在真实数据上执行清理；设置页入口需要用户预览并勾选确认。
- 报告 source_period 缺失或无法解析时，该报告覆盖期不参与记录清理。
- 任务年龄取 updated_at；完成后再追加进展会延后清理时间。
- 跨文件移动不是事务，部分失败只报告不自动回滚。
