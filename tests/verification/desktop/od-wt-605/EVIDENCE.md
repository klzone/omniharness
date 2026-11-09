# OD-WT-605 实跑证据（2026-11-09）

基线：HEAD `ab000dfb01ab78c53ffc666c8d3a4d262b6265fc`；工作区保留既有未提交改动。

## cargo test --offline --lib（默认线程数）

```text
export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && cargo test --offline --lib
```

结果：`964 passed; 2 failed`，EXIT=101。失败项都是假引擎合成子进程启动时的
`PermissionDenied`（`engine_bridge::tests::event_pump_does_not_starve_host_lock`、
`engine_bridge::tests::unstamped_grant_cannot_start_engine`、
`engine_ipc::tests::spawn_with_stderr_log_captures_child_stderr`），属于任务卡点名的既有
`OMNIDESK_PYTHON` 并发竞态（本轮随机命中 2 项，此前一轮命中 3 项），不是本任务测试失败。原始输出见
`cargo-lib-default-threads.log`（`.gitignore` 忽略 `*.log`，文件保留在本机证据目录）。

## cargo test --offline --lib（RUST_TEST_THREADS=4 复跑）

```text
export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib
```

结果：`966 passed; 0 failed`，EXIT=0。原始输出见 `cargo-lib-4-threads.log`。

## 本任务定向测试

```text
export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib writing_collect
```

结果：`17 passed; 0 failed`，EXIT=0。含场景三两项断言：

- `defaults_to_confirmed_only_and_never_surfaces_pending`：默认结果不含待审核素材。
- `include_pending_marks_pending_review_and_disables_direct_quote`：显式打开后出现，
  `pending_review=true`，`suitable_for_direct_quote=false`。

原始输出见 `writing-collect-tests.log`。

## TypeScript 类型检查

```text
cd desktop && npx tsc --noEmit
```

结果：EXIT=0，无诊断输出。原始输出见 `tsc.log`。

## 证据等级

- 文件事实：源码与类型定义已读。
- 静态检查：`npx tsc --noEmit`。
- 脚本运行：cargo lib 全量（RUST_TEST_THREADS=4）与定向 16 项。
- 未验证：真实窗口联动、真实模型主题提示词质量、真实知识库大规模检索性能。
