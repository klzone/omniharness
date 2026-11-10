# OD-PROC-149a 证据（Windows exe 副本机制）

实施：2026-11-11，本助手亲自执行（未派子代理）。HEAD 起点 `f3f21e68c6`；未提交、未推送。

## 改动

- 新增 `desktop/src-tauri/src/process_alias.rs`：副本复制 / marker（源路径+len+mtime）/ 刷新 / python 探针 / PATH 注入 / 回退开关；`cfg!(windows)` 之外整体透传，核心逻辑以 `select_with(..., windows, probe)` 暴露给单测。
- `desktop/src-tauri/src/lib.rs`：`pub mod process_alias;`。
- `desktop/src-tauri/src/engine_supervisor.rs`：`EngineSupervisor::start()` 用 `select_for_engine` 转换解释器，并把 `alias.env`（PATH、OMNIDESK_NODE）并入 `child_env()` 后 spawn。
- `desktop/src-tauri/src/process_owner.rs`（计划文件清单外的联动点，任务卡正文明确要求）：`is_engine_cmd` 抽出 `is_node_image`，兼容 `omnidesk-node` 前缀与 Windows 反斜杠路径；新增单测。
- `tests/verification/desktop/verify_proc_alias_149.py`：补齐 149a 静态断言段（149b 段由并行代理交付，未改动其逻辑）。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| `export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib` | 101 | 1103 passed / 3 failed；3 项均为既有 process_owner 沙箱失败（`live_instance_owned_by_another_host_is_refused`、`pid_reuse_with_unrelated_cmdline_is_never_killed`、`record_from_other_boot_is_removed_without_killing`，本机读不到自身进程表，与 149 无关）；process_alias 8 项全绿 |
| `cd desktop && npx tsc --noEmit` | 0 | 无输出 |
| `python3 tests/verification/desktop/verify_proc_alias_149.py` | 0 | 34/34 PASS（149a 16 项 + 149b 18 项） |
| `python3 tests/verification/desktop/verify_docx_attachment_148.py` | 0 | 14/14 通过 |
| `python3 tests/verification/desktop/verify_diagnose_omnidesk.py` | 0 | 15/15 |

## 实现要点与限制

- 副本目录 `<data_root>/runtime/bin/`（`OMNIDESK_APP_DATA_ROOT` 或默认 `~/OmniDesk/app`）。
- 用户显式 `OMNIDESK_NODE` / `OMNIDESK_PYTHON` 时零复制、零接管，仅记日志。
- 探针脚本为 `import sys, encodings; print(sys.prefix)`：`import sys` 是内建模块，单独成功不能证明副本还能找到标准库；探针与运行时用同一份 PATH（副本目录 + 原解释器目录）。探针失败删副本与 marker，回退原名。
- 开关：`OMNIDESK_PROC_ALIAS` 取 `off/false/0/no` 时跳过；仅 Windows 生效。
- 未验证：Windows 真机任务管理器显示 `omnidesk-node.exe` / `omnidesk-python.exe`（用户实测项）；Windows 上被复制的 python.exe 的 DLL/标准库解析仅由探针与 PATH 兜底，若真机探针失败会自动回退原名，功能表现同改动前。
