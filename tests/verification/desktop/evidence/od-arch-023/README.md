# OD-ARCH-023 执行证据：修复模型权限变更与引擎启动之间的并发竞态

- 执行时间：2026-10-18 22:00–23:10（Asia/Shanghai）
- 执行者：OpenCode（用户授权；主工作区 `/mnt/data/Code/MyWorkStation`）
- 对应计划：[PLANS.md](../../../../PLANS.md) 的 **OD-ARCH-023**
- 依据复核：[REVIEW.md](../../../../REVIEW.md) 顶部的 OD-ARCH-022 复核
- 证据等级：脚本/运行（Rust 并发测试 + 假引擎集成测试）+ 构建 + 静态检查

> ⚠️ **覆盖结论修正（OD-ARCH-024，2026-10-18）。** 本文件 §3「计划要求的并发验证」曾把
> “包装器受守卫保护”计为已覆盖，但 ARCHITECT 复核（REVIEW.md OD-ARCH-023）发现
> `with_engine_call` / `with_call_gate` 会在持守卫时再次调用 `is_current()` 而**自锁**，
> 因此当时的 `EngineLifecycle` 纯单元测试与源码字符串测试**都不能代表组合路径可用**；
> 且 `engine_task_status`/`result`/`cancel` 未纳入边界。上述问题由 OD-ARCH-024 修复，
> 组合路径现由宿主包装器测试覆盖（见 [OD-ARCH-024 证据](../od-arch-024/README.md) §2 的
> “临时还原即失败”实测证明）。本文件其余内容保留为当时的执行事实。

## 0. 结论摘要

1. **竞态根因已消除**：新增 `EngineLifecycle` 串行化边界；所有会**启动/触达引擎**的入口
   都在同一守卫内完成「准入判定 → 启动引擎 → 动作」，权限变更必须独占同一把锁。
   因此不存在“先取得旧准入结果、撤权完成后再启动引擎”的时序。
2. **入场清单已覆盖**：任务提交、引擎描述、会话列表、会话恢复、任务台账、任务恢复
   六个启动入口 + 有副作用但不启动引擎的目录准备入口，全部纳入守卫。
3. **凭据绑定权限世代**：`AdmissionGrant` 携带权限世代；未盖章的凭据在
   `EngineBridge::ensure_initialized` 处被拒绝，锁外取得的准入结果无法用于启动引擎。
4. **失败关闭保持**：停止失败 → 保留屏障并拒绝一切引擎调用；确认停止/重启变更后才恢复。
5. 计划要求的回归全部通过；`runtime/` 无差异。

## 1. 缺陷与修复

**缺陷（REVIEW.md OD-ARCH-022 复核）：** `engine_submit_task` 在锁外读取准入事实并取得
`AdmissionGrant`，之后才检查屏障并加锁启动引擎；`engine_describe`、会话列表/恢复、任务
台账/恢复同样在锁外取准入结果且未检查屏障；权限变更则先写设置、再关引擎、最后才置位
屏障。并发时调用可拿着变更前的 grant，在撤权停止引擎之后重新启动并执行。

**修复：**
| 机制 | 说明 |
| --- | --- |
| `EngineLifecycle::begin_call()` | 引擎调用守卫；屏障期间直接拒绝；**存活期间权限变更无法开始** |
| `EngineLifecycle::begin_change()` | 权限变更独占守卫；变更开始即置位屏障，成功清除、失败保留，并推进权限世代 |
| `with_engine_call()` | 守卫内完成「准入（盖章）→ `ensure_initialized` → 动作」；六个启动入口共用 |
| `with_call_gate()` | 守卫内完成「准入（盖章）→ 非启动副作用」；用于 `task_prepare_workspace_dir` |
| `AdmissionGrant.epoch` | 准入凭据绑定当前权限世代；未盖章（0）在 `ensure_initialized` 被拒 |
| 加锁顺序 | 统一为「生命周期 → 宿主」；`engine_admission_status` 先释放宿主锁再读屏障，避免反向死锁 |

## 2. 改动文件

- **新增**：`desktop/src-tauri/src/engine_lifecycle.rs`（含 5 条确定性并发测试）。
- **修改**：
  - `desktop/src-tauri/src/lib.rs`：`HostState` 增加 `lifecycle`；新增 `with_engine_call` /
    `with_call_gate` / `engine_call_blocked`；六个启动命令改为经守卫；`task_prepare_workspace_dir`
    改为经守卫；权限变更与 `engine_shutdown` 改用 `begin_change`；删除旧 `permission_barrier`
    与纯函数 `permission_change_result`；新增源码级接线测试。
  - `desktop/src-tauri/src/admission.rs`：`AdmissionGrant` 增加权限世代、`stamp_epoch()`、
    `GRANT_UNSTAMPED_EPOCH`，以及测试用 `unstamped_for_test()`。
  - `desktop/src-tauri/src/engine_bridge.rs`：`ensure_initialized` 拒绝未盖章凭据；
    新增假引擎集成测试 `unstamped_grant_cannot_start_engine`。
- **未改**：`runtime/`、前端、模型路由、凭证、DSH 数据、第二 worktree、`REVIEW.md`。

## 3. 计划要求的并发验证

| 计划要求 | 覆盖 | 测试 |
| --- | --- | --- |
| 暂停某调用于准入完成之后，执行撤权并确认旧引擎已停，再释放该调用 | ✅ | `engine_lifecycle::tests::call_holding_gate_prevents_change_from_starting`：调用持守卫期间权限变更**无法开始**（非阻塞探针为 `None`），释放后才开始；撤权推进世代，旧世代不再可用 |
| 证明旧 grant 不会启动引擎 | ✅ | `engine_bridge::tests::unstamped_grant_cannot_start_engine`：未盖章凭据被拒且**不初始化子进程**；盖章后的凭据正常启动（对照组） |
| 覆盖停止失败时持续拒绝、恢复后才允许 | ✅ | `stop_failure_keeps_blocking_until_recovered`、`command_tests::prepare_workspace_is_refused_while_permission_change_blocked`（拒绝时**不创建目录**） |
| 变更持锁期间新调用不能开始（结构性互斥） | ✅ | `new_calls_cannot_start_while_change_holds_lock`（跨线程，200ms 内不得开始；变更结束后放行） |
| 所有启动入口都受同一保护 | ✅ | `engine_lifecycle_wiring_tests::engine_starting_commands_go_through_lifecycle_gate`（源码级：六入口必须经 `with_engine_call`、准备目录必须经 `with_call_gate`、变更/停止必须 `begin_change`、准入判定只允许出现在两个守卫内） |

**独立复现命令：**
```bash
cd desktop/src-tauri && cargo test --offline --lib engine_lifecycle -- --nocapture
cd desktop/src-tauri && cargo test --offline --lib unstamped_grant -- --nocapture
cd desktop/src-tauri && cargo test --offline --lib engine_starting_commands_go_through_lifecycle_gate
```

## 4. 实际运行的检查与结果

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline` | **108 lib + 2 admission + 13 host + 3 e2e = 126 通过 / 0 失败**，无警告 | Rust 离线回归 |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功（JS 317.96 kB / CSS 71.95 kB） | 前端构建 |
| `node src-tauri/scripts/dump-model-route.mjs --self-test` | 21/21，`ok: true` | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行（合成端点） |
| `verify_engine_cancel.py` / `verify_ai_draft.py` / `verify_session_continuity.py` | pass / pass / 37 项 0 失败 | 脚本运行 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 无差异 | 文件事实 |

原始输出：`cargo-test-offline.log`、`dump-model-route-self-test.json`。

## 5. 未验证与限制

1. **真实模型输出与质量**未验证（未调用真实端点、未读凭证）。
2. **停止失败**仍以「屏障 → 拒绝 → 恢复」逻辑与假引擎/纯逻辑测试覆盖；
   **未在真实窗口制造一次真实子进程无法退出的场景**（`engine_bridge::shutdown` 的真实失败难以稳定复现）。
3. **端到端窗口验证**未做：未在真实 Tauri 窗口中触发“变更中提交”并观察界面。
4. 仅 Linux；Windows/macOS 未验证。
5. 前端未改动：界面沿用 OD-ARCH-022 的失败提示与“停止引擎以恢复”入口。

## 6. 边界声明

- 未修改 `runtime/`、模型路由、凭证、真实材料或第二 worktree；未提交、未推送、未打标签。
- `REVIEW.md` 未改（留 ARCHITECT 复核）；既有未提交/未跟踪改动全部保留。
