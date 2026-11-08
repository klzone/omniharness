# OD-KM-508 验收证据（2026-11-09）

任务：项目 IPC 三命令 + project_overview + Task 保存路径项目存在性校验。
执行方式：本会话亲自执行，未派子代理。

## 实跑命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --offline --lib` | 856 passed / 0 failed / 0 ignored | `cargo-lib.log` |
| `npx tsc --noEmit` | 退出码 0，无输出 | `tsc.log` |

## 新增测试（5 项，均在 `project_ipc_tests`）

1. `project_create_gates_user_text_without_writing_on_red_line`
2. `task_create_rejects_missing_project_with_project_hint`
3. `task_update_rejects_missing_project_and_leaves_file_unchanged`
4. `task_save_accepts_existing_project_and_overview_counts_it`
5. `project_overview_derives_records_materials_and_knowledge`

其中第 2、3、4 项覆盖任务卡要求的 project_id 存在性校验（≥3 项）。测试数据全部虚构。

## 偶发噪音记录

本轮中途一次全量 `cargo test --offline --lib` 出现
`engine_supervisor::tests::supervisor_channel_initialize_and_stop` 失败，原因为
启动假引擎时 `Os { code: 13, kind: PermissionDenied }`；立即复跑即 0 failed。
该现象与 REVIEW.md 已登记的 macOS 偶发噪音一致，不属本次改动范围。
