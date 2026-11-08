# OD-WR-301 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 实施前库单测：565 passed（本轮实跑）
- 实施后库单测：606 passed，新增 41 项
- 交付模块：desktop/src-tauri/src/worklog_records.rs、desktop/src-tauri/src/task_store.rs
- 注册改动：desktop/src-tauri/src/lib.rs 增加两行 pub mod

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 606 passed / 0 failed（exit-lib.txt） |
| cargo test --offline --tests | 0 | 13 个测试目标全部 ok / 0 failed（exit-tests.txt） |

原始输出：cargo-test-lib.log、cargo-test-tests.log。

## 测试分布（本轮新增 41 项）

- worklog_records：19 项（frontmatter 往返、日期默认、标题推导、非法 id、重复 id、坏文件计数、更新、删除进回收目录、日期/分类/关键词/草稿/状态过滤、排序、特殊字符、临时文件清理）
- task_store：22 项（任务建文件、初始状态白名单、标题校验、来源记录与片段约束、状态迁移白名单与终态、进展追加、列表过滤、候选不进正式清单、候选暂存、去重键、quote 校验、确认/拒绝、删除、元数据更新、记录改写后拒绝暂存）

## 已知偶发失败（非本任务代码）

cargo test --offline --tests 的一次中途运行在 process_owner::tests::live_instance_owned_by_another_host_is_refused 失败（605 passed / 1 failed），该测试独立运行通过，随后整轮重跑通过。根因定位：该测试用 sh -c "sleep 30" omnidesk_engine 冒充引擎，轮询确认 cmdline 后写 owner.json；macOS 的 sh 可能以 exec 方式执行单条命令，argv 在轮询与 reclaim 之间变成 sleep 30，identity_ok 变假，错误落到「无法核验身份」而非「另一个 OmniDesk 实例」。修复需改 process_owner.rs 测试夹具，超出本任务允许文件范围，未改动，留待规划会话另立任务。

## 未做

- 未接 working_root 设置与 IPC（属 OD-WR-302/后续任务）。
- 未实现 AI 解析、quote 语义校验之外的模型质量验证（OD-WR-304）。
- 未更新 STATUS.md/REVIEW.md（由规划会话复核）。
