# OD-WT-602 证据（2026-11-09）

- 基线：HEAD `ab000dfb01`，工作区保留 601/603 未提交改动。
- 交付：`desktop/src-tauri/src/document_store.rs`，`lib.rs` 仅追加 `pub mod document_store;`。

## 实跑命令与结果

1. `cargo test --offline --lib document_store::tests`
   - 17 passed；0 failed；922 filtered out。日志：`cargo_test_document_store.log`。
2. `cargo test --offline --lib`（`RUST_TEST_THREADS=4`）
   - 939 passed；0 failed；0 ignored。日志：`cargo_test_lib.log`。
3. `cargo test --offline --lib`（默认测试线程数）
   - 936 passed；3 failed；0 ignored。失败日志：`cargo_test_lib_default_threads.log`。

## 默认并发说明

默认测试线程数下，同一 suite 出现过 3 项与本次交付无关的失败：
`engine_bridge::tests::event_pump_does_not_starve_host_lock`、
`engine_bridge::tests::unstamped_grant_cannot_start_engine`、
`engine_ipc::tests::spawn_with_stderr_log_captures_child_stderr`，均为
`PermissionDenied`。根因是既有 `command_tests::build_state_seeds_bundled_home_before_model_settings`
在并行测试期间把进程级 `OMNIDESK_PYTHON` 指向未加执行位的临时 stub；这些引擎测试
恰好同时解析该环境变量。单独运行三项均通过；限制为 4 个测试线程后全 suite 通过。
本次未修改该既有测试。
