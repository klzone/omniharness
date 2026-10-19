# OD-ARCH-024 执行证据：修复生命周期自锁并补齐宿主并发验证

- 执行时间：2026-10-18 23:20–23:50（Asia/Shanghai）
- 执行者：OpenCode（用户授权；主工作区 `/mnt/data/Code/MyWorkStation`）
- 对应计划：[PLANS.md](../../../../PLANS.md) 的 **OD-ARCH-024**
- 依据复核：[REVIEW.md](../../../../REVIEW.md) 顶部 OD-ARCH-023 结论（不通过）
- 证据等级：脚本/运行（Rust 组合路径测试，当前源码新编译）+ 构建 + 静态检查

## 0. 结论摘要

1. **自锁已修复**：`with_engine_call` / `with_call_gate` 不再于守卫存活期间调用
   `EngineLifecycle::is_current()`（该方法会再次锁同一把非可重入互斥锁）。世代校验改为
   守卫自带的非重入方法 `CallGuard::current_epoch()`，**权限世代一致性保障未削弱**。
2. **组合路径已被测试覆盖**：新增 4 条宿主包装器测试，使用**真实**包装器 + **假引擎** +
   **真实路由分类**；旧实现下其中一条会以“自锁/超时”失败（本回合实测，见 §2）。
3. **状态/结果/取消的生命周期语义已明确并落实**：三者**不取得生命周期守卫、不检查屏障**
   （只共用宿主锁；权限变更只在有界的 ≤10 s 关闭等待期间持有该锁），因此权限变更不会把
   取消堵死；同时它们**不会**启动引擎。
4. **源码接线测试扩展**：新增对这三个命令的检查（不得取得守卫、必须复用 `*_impl`）。
5. 计划要求的回归全部通过；`runtime/` 无差异。

## 1. 缺陷与修复

| # | 缺陷（REVIEW.md OD-ARCH-023） | 修复 |
| --- | --- | --- |
| 1 | `with_engine_call` / `with_call_gate` 持 `CallGuard` 时调用 `lifecycle.is_current()` → 同线程重入非可重入互斥锁 → **受保护命令自锁** | 新增 `CallGuard::current_epoch()`（直接读守卫已持有的状态，不再加锁）；两个包装器改用它；`EngineLifecycle::is_current()` 保留但加注“不得在持守卫时调用” |
| 2 | `engine_task_status` / `engine_task_result` / `engine_cancel_task` 未纳入生命周期边界，语义未定义 | 明确语义（见 §3）并改造为 `task_status_impl` / `task_result_impl` / `cancel_task_impl`（命令与测试共用同一实现） |

**注意**：修复没有删除权限世代校验——准入凭据仍绑定世代、未盖章凭据仍被
`EngineBridge::ensure_initialized` 拒绝（`grant_not_stamped`）。

## 2. 「测试确实能捕获自锁」的实测证明

为确认新增测试不是装饰，本回合做了**一次临时还原实验**：

1. 把 `with_engine_call` 的世代校验临时还原为旧写法 `lifecycle.is_current(...)`；
2. 运行 `cargo test --offline --lib describe_wrapper_returns_without_self_lock -- --nocapture`；
3. 实测结果：**`with_engine_call 自锁/超时（OD-ARCH-024 回归）: Timeout`，90.00s 后 FAILED**；
4. 立即恢复修复版源码，并清理实验遗留的假引擎子进程。

因此该测试在旧实现下会失败、在新实现下通过，可作为“无自锁”的判据。

## 3. 状态 / 结果 / 取消的生命周期语义（本次定义）

| 命令 | 启动引擎 | 取得生命周期守卫 | 检查屏障 | 权限变更期间行为 |
| --- | --- | --- | --- | --- |
| `engine_task_status` | 否 | **否** | **否** | 照常返回引擎的真实答复（引擎已停则返回结构化错误）；不被生命周期锁阻塞 |
| `engine_task_result` | 否 | **否** | **否** | 同上 |
| `engine_cancel_task` | 否 | **否** | **否** | **仍可请求取消**：权限变更不会把取消永久堵住 |

理由：三者既不启动引擎也不创建引擎资源，唯一共用资源是宿主互斥锁；权限变更只在有界的
关闭等待（≤10 s）期间持有它。若在屏障期间拒绝它们，反而会使用户在引擎可能仍在运行时
无法读状态或取消。

## 4. 改动文件

- 修改：`desktop/src-tauri/src/engine_lifecycle.rs`（`CallGuard::current_epoch`，注释警示）、
  `desktop/src-tauri/src/lib.rs`（两个包装器改非重入校验；三个任务命令抽出 `*_impl` 并写明语义；
  新增 `host_wrapper_tests`；扩展源码接线测试）、`PLANS.md`、`STATUS.md`。
- 新增：`tests/verification/desktop/evidence/od-arch-024/`（`README.md`、`cargo-test-offline.log`、
  `dump-model-route-self-test.json`）。
- 修正：`tests/verification/desktop/evidence/od-arch-023/README.md` 增加“覆盖结论修正”说明。
- 未改：`runtime/`、前端、模型路由、凭证、用户数据、真实材料、第二 worktree、`REVIEW.md`。

## 5. 测试清单与「是否当前源码新编译」

**是当前源码新编译**（非旧预编译二进制）：

| 证据 | 值 |
| --- | --- |
| 本次运行日志开头 | `Compiling omnidesk-desktop v0.1.0 … Finished test profile … in 9.19s` |
| 测试二进制 | `target/debug/deps/omnidesk_desktop_lib-afcdf36f070ad598`，mtime **23:44:08** |
| 最后一次源码改动 | `src/lib.rs` mtime **23:43:41**（早于二进制） |

新增/变更测试（均通过）：

| 测试 | 覆盖 |
| --- | --- |
| `host_wrapper_tests::describe_wrapper_returns_without_self_lock` | 真实 `with_engine_call`：准入 → 启动（假引擎）→ 动作闭环且**不自锁**（90 s 上限，旧实现实测超时失败） |
| `host_wrapper_tests::call_gate_wrapper_returns_without_self_lock` | 真实 `with_call_gate`：准入 → 动作闭环且不自锁；**不启动引擎** |
| `host_wrapper_tests::in_flight_call_blocks_permission_change` | 调用在途（已进入动作）时权限变更**无法开始**；释放后可开始 |
| `host_wrapper_tests::barrier_refuses_wrappers_but_task_commands_stay_usable` | 屏障期间包装器被拒（`permission_change_blocked`）；状态/结果/取消**不被拒、不被堵**；恢复后放行 |
| `engine_lifecycle_wiring_tests::engine_starting_commands_go_through_lifecycle_gate` | 六启动入口须经 `with_engine_call`；目录准备须经 `with_call_gate`；变更/停止须 `begin_change`；三个任务命令**不得**取得守卫且须复用 `*_impl`；准入判定只允许出现在两个守卫内 |
| `engine_lifecycle::tests::*`（5 条） | 屏障语义、停止失败持续拒绝与恢复、持守卫时变更无法开始、跨线程互斥、世代推进 |

## 6. 实际运行的检查与结果

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline`（当前源码新编译） | **112 lib + 2 admission + 13 host + 3 e2e = 130 通过 / 0 失败**，无警告 | Rust 离线回归 |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功（JS 317.96 kB / CSS 71.95 kB） | 前端构建 |
| `node src-tauri/scripts/dump-model-route.mjs --self-test` | 21/21 | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行（合成端点） |
| `verify_engine_cancel.py` / `verify_ai_draft.py` / `verify_session_continuity.py` | pass / pass / 37 项 0 失败 | 脚本运行 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 无差异 | 文件事实 |

原始输出：`cargo-test-offline.log`、`dump-model-route-self-test.json`。

## 7. 未验证与限制

1. **真实模型输出与质量**未验证（未调用真实端点、未读凭证）。
2. **停止失败**仍以屏障逻辑与假引擎/组合测试覆盖，**未在真实窗口制造真实子进程无法退出**。
3. **端到端窗口验证**未做（未在真实 Tauri 窗口触发“变更中提交/取消”）。
4. 宿主包装器测试使用**假引擎**：验证的是宿主生命周期与接线，不验证真实 DSH/ACP 行为。
5. 仅 Linux；Windows/macOS 未验证。

## 8. 边界声明

未修改 `runtime/`、模型路由、凭证、用户数据、真实材料或第二 worktree；未提交、未推送、
未打标签；`REVIEW.md` 未改（留 ARCHITECT 复核）。

---

# 补测与收尾（2026-10-19，同一证据索引）

本补充记录 OD-ARCH-024 的**定向补测与行为落实**：用可控假引擎建立**真实运行中的任务**，
确定性地并发触发关闭与取消，据结果落实一致语义。

## A. 发现的真实问题

| 现象 | 根因（源码事实） |
| --- | --- |
| 关闭期间的取消会**排队到关闭之后**才返回 | `bridge.shutdown()` 需要 `&mut EngineBridge`，因此**全程持有宿主锁**；`stop()` 又全程持有 supervisor 锁。取消命令先抢宿主锁 → 只能等关闭结束 |
| 关闭后返回**误导性**错误 | `stop()` 先 `channel.take()`，之后 `rpc()` 命中 `!initialized` 分支，旧消息为“引擎尚未初始化，请先调用 engine_describe”，与实际（刚刚被关闭）不符 |

**实测（反向验证）**：临时移除“关闭中”检查后，
`cancel_during_shutdown_returns_promptly_and_is_not_sent_to_engine` 以
**`取消不得排队等待关闭（等了 4.031304621s）`** 失败——正是要消除的行为。

## B. 落实的并发行为

| 命令 | 关闭进行中（`is_shutting_down()`） | 说明 |
| --- | --- | --- |
| `cancel` | 立即返回 `engine_shutting_down`，消息明确“**取消请求未发送**” | **不排队**、不送入正在关闭的引擎 |
| `status` / `result` | 立即返回 `engine_shutting_down`，消息明确“未发送” | 不排队 |
| 关闭之前发起的 `cancel` | **送达引擎**（假引擎方法日志可见 `omni.cancel_task`） | 正常路径不受影响 |

实现要点：
1. `EngineLifecycle` 增加**无锁可读**的原子信号 `shutting_down`（`mark/clear/is_shutting_down`）；
2. 新增 `stop_engine()`：先取 supervisor 句柄（**宿主锁只短暂持有**）→ 置位“关闭中”信号 →
   `stop()`（**关闭等待期间不持有宿主锁**）→ 重新取锁 `mark_stopped()` → 清除信号；
   `apply_model_permission_change` 与 `engine_shutdown` 共用它；
3. 关闭等待上限不变（`stop_engine(..., Duration::from_secs(10))`，`stop` 内部优雅请求 + 终止等待）；
4. `engine_bridge::rpc` 的“未初始化”错误改为如实描述“引擎未运行或已停止（可能正被关闭），本次请求未发送”。

## C. 新增/扩展的测试

| 测试 | 断言 |
| --- | --- |
| `host_wrapper_tests::cancel_is_delivered_before_shutdown_for_a_running_task` | 真实提交建立运行中任务 → 取消 → **假引擎方法日志中确有 `omni.cancel_task`**；回执 `final_state=cancelled`、`phase=confirmed_cancelled`（不是“不存在的任务 ID + 只看错误码”） |
| `host_wrapper_tests::cancel_during_shutdown_returns_promptly_and_is_not_sent_to_engine` | 用 `OMNI_TEST_SHUTDOWN_DELAY_MS=4000` 制造关闭窗口：取消/状态/结果均在 **<1.5 s** 内返回 `engine_shutting_down` 且消息含“未发送”；且方法日志显示 **`omni.shutdown` 之后没有再出现 `omni.cancel_task`** |
| `engine_lifecycle_wiring_tests::engine_starting_commands_go_through_lifecycle_gate`（扩展） | 三个 `*_impl` 必须含 `is_shutting_down()`；`stop_engine` 必须**先** `mark_shutting_down()` **再** `stop(grace)`，且含 `clear_shutting_down()` |

假引擎夹具同步扩展（测试基础设施）：新增 `omni.get_task_status` / `omni.get_task_result` /
`omni.cancel_task`，方法日志 `OMNI_TEST_METHOD_LOG`，关闭延迟 `OMNI_TEST_SHUTDOWN_DELAY_MS`。

## D. 本轮运行的命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline`（当前源码新编译：日志 `Compiling … in 1m 13s`；二进制 `…afcdf36f070ad598` mtime **00:33:49** > 最后一次源码改动 `engine_lifecycle.rs` **00:33:30** / `lib.rs` **00:33:14**） | **114 lib + 2 admission + 13 host + 3 e2e = 132 通过 / 0 失败**，无警告 |
| `npx --no-install tsc --noEmit` | 退出码 0 |
| `npm run build` | 成功 |
| `dump-model-route.mjs --self-test` | 21/21 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass |
| `verify_engine_cancel.py` / `verify_ai_draft.py` / `verify_session_continuity.py` | pass / pass / 37 项 0 失败 |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 |
| HEAD | `5ffc710`（未提交、未推送、未打标签） |

原始输出：`cargo-test-offline-supplement.log`、`dump-model-route-self-test.json`。

## E. 补充限制

1. 关闭窗口由**假引擎**的可配置延迟制造；真实 DSH 的关闭耗时不同，但结论（信号先于 stop 置位、
   关闭期间不持宿主锁、取消立即返回）与具体耗时无关。
2. “关闭中”信号是**原子**读取；若取消命令恰好在信号置位前通过了检查，它仍会等待本次有界关闭
   （≤10 s）后返回 `engine_unavailable`（消息已如实说明“未运行或已停止”），不会在关闭后才被送入引擎。
3. 未在真实 Tauri 窗口触发该并发场景；真实模型、跨平台仍未验证。

---

# 第三轮：可线性化的最小并发边界（2026-10-19）

针对 ARCHITECT 新指出的缺口：**“任务命令先检查关闭标志，再获取 supervisor 锁；关闭流程
可能在两者之间先取得该锁，使请求排队到关闭之后。”** 本轮把判定搬进锁内，并做确定性时序验证。

## A. 根因与结构性问题

| 项 | 第二轮实现（已被指出） | 第三轮实现 |
| --- | --- | --- |
| 关闭中判定位置 | `EngineLifecycle` 的无锁原子标志，在**取锁之前**读取 | `EngineSupervisor::with_channel` **在 supervisor 锁内**读取 `stopping` |
| 判定与发送的关系 | 两次独立操作，中间有窗口 | 同一把锁内的同一次临界区，**无窗口** |
| 关闭是否持锁等待 | supervisor 锁被 `stop()` 全程持有（约 25 s 上界） | 锁内只做 `begin_stop()`（置位标志 + 摘除通道，O(µs)）；耗时收尾 `finish_stop()` 在**锁外** |

结论：命令只会有两种结果——**关闭前送达**，或**在锁内看到“正在关闭”并立即返回未发送**。

## B. 落实的代码边界

| 位置 | 变更 |
| --- | --- |
| `engine_ipc.rs` | 新增 `EngineRpcError::shutting_down()`：`engine_error_code = engine_shutting_down`、`detail.engine_detail.sent_to_engine = false` |
| `engine_supervisor.rs` | 新增字段 `stopping` 与 `is_stopping()`、`begin_stop()`（锁内：置位 + 摘除通道）、`end_stop()`、`finish_stop(channel, grace)`（锁外慢路径）；`with_channel` 在锁内拒绝；`start()` 在关闭未收尾时拒绝启动新实例 |
| `engine_bridge.rs` | `rpc()` 持锁后**先**判 `is_stopping()`（稳定返回 `engine_shutting_down`），再判 `initialized`；`shutdown()` 改为两段式；新增 `#[cfg(test)]` 取得 supervisor 锁**之前**的同步点 |
| `engine_lifecycle.rs` | **移除**上一轮的无锁原子标志：判定统一到 supervisor 锁内，避免两套机制 |
| `lib.rs` | `stop_engine()` 两段式（宿主锁/ supervisor 锁都只短暂持有）；三个 `*_impl` **去掉锁外预检**；接线测试改为断言“锁内边界”不变量 |

## C. 三条宿主级测试

| 测试 | 断言 |
| --- | --- |
| `cancel_is_delivered_before_shutdown_for_a_running_task` | 真实提交建立运行中任务后取消 → 假引擎方法日志确有 `omni.cancel_task`；`final_state=cancelled`、`phase=confirmed_cancelled`（**送达**路径） |
| `task_commands_return_promptly_when_shutdown_is_in_progress` | 关闭进入协议阶段后：取消/状态/结果均 **<1.5 s** 返回 `engine_shutting_down` + “未发送” + `sent_to_engine=false`；`omni.shutdown` 之后不再出现 `omni.cancel_task` |
| `task_commands_are_refused_when_shutdown_wins_the_engine_lock`（**确定性时序同步点**） | 先让关闭完成 `begin_stop()` 并进入耗时阶段；把取消线程停在“**已进入发送路径、尚未取得 supervisor 锁**”的同步点后放行 → 取消**未送达**、**<1.5 s** 返回明确结论；状态/结果同样 |

接线测试新增不变量：`stop_engine` 必须 `begin_stop()` 在前、`finish_stop(` 在后且**不得**出现 `.stop(grace)`；
三个 `*_impl` **不得**含 `is_shutting_down`（禁止锁外预检）；`engine_supervisor.rs` 的 `with_channel`
必须含 `self.stopping` 判定与 `shutting_down(`，`begin_stop` 必须先 `self.stopping = true` 再 `self.channel.take()`。

## D. 反向验证（变异测试，均已还原）

| 变异 | 预期 | 实测 |
| --- | --- | --- |
| A：把耗时关闭放回 supervisor 锁内（旧行为） | 补测必须失败 | **失败**：`关闭先取得引擎锁时，取消也不得排队等待（等了 8.042621669s）` |
| B：只移除 `with_channel` 的 `stopping` 判定 | 仍应通过（双层判定） | 通过（bridge 层仍拒绝）——说明两层防御独立有效 |
| C：移除 bridge 与 supervisor **两处**判定 | 补测必须失败 | **失败**：返回 `engine_unavailable`（“引擎尚未启动”），断言 `engine_shutting_down` 不成立 |

## E. 本轮命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline`（日志 `Compiling …`；测试二进制 `…afcdf36f070ad598` mtime **06:50:43** > 最后一次源码改动 **06:50:41**） | **115 lib + 2 admission + 13 host + 3 e2e = 133 通过 / 0 失败**，无警告 |
| `npx --no-install tsc --noEmit` | 退出码 0 |
| `npm run build` | 成功 |
| `dump-model-route.mjs --self-test` | 21/21 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass |
| `verify_engine_cancel.py` | state 34/34 + static 8/8，pass |
| `verify_ai_draft.py` | 22 cases / 0 failed |
| `verify_session_continuity.py` | 37 checks / 0 failed |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 |
| HEAD | `5ffc710`（未提交、未推送、未打标签） |

原始输出：`cargo-test-offline-round3.log`、`dump-model-route-self-test.json`。

## F. 边界与未验证

1. 关闭窗口仍由**假引擎**的可配置延迟制造；真实 DSH 关闭耗时不同，但结构性结论
   （判定在锁内、耗时收尾在锁外）与具体耗时无关。
2. 单次关闭的耗时上界仍为 `grace + 5s`（协议关闭）+ `grace`（进程终止），`grace = 10s`
   时约 25 s；本轮保证的是**这段时间不占用宿主锁与 supervisor 锁**。
3. 未在真实 Tauri 窗口触发该并发场景；真实模型、跨平台仍未验证。
4. `EngineSupervisor::stop()`（单段式）保留为测试使用的便捷包装；宿主生产路径只用两段式。

---

# 第四轮：窗口销毁关闭路径补齐（2026-10-19，按 PLANS.md 批准计划）

## A. 缺口（ARCHITECT 复核）

`lib.rs` 的 `WindowEvent::Destroyed` 回调先 `state.lock()` 再 `host.bridge.shutdown(5s)`：
`bridge.shutdown()` 自身已把协议关闭移到 supervisor 锁外，但**调用方手里的宿主锁**会一直占着
直到关闭结束——状态/结果/取消入口都要同一把宿主锁，因此窗口关闭期间仍被排队（失去第三轮的快速语义）。

## B. 修正

| 位置 | 变更 |
| --- | --- |
| `lib.rs` | 新增 `on_window_destroyed_shutdown(&SharedHost)`：**复用共享入口 `stop_engine()`**；窗口销毁回调只调它，不再自行加锁、不再直连 `bridge.shutdown()` |
| `engine_bridge.rs` | `EngineBridge::shutdown()` 文档补充：**调用方不得在持有宿主锁时等待本方法**（否则等待会占着那把锁） |
| `lib.rs`（接线测试） | 新增不变量：生产前缀（文件开头 → `run()` 之后第一个测试模块）**不得出现 `bridge.shutdown(`**；窗口回调（去注释后）必须调用 `on_window_destroyed_shutdown(`，且不得含 `state.lock()` / `bridge.shutdown(` |

至此**关闭入口唯一**：命令路径 `stop_engine()`、窗口销毁路径 `on_window_destroyed_shutdown()` → `stop_engine()`，
两条路径都不在等待关闭时持有宿主锁。

## C. 确定性宿主级回归

`host_wrapper_tests::window_destroy_shutdown_does_not_hold_host_lock_while_waiting`：

1. 假引擎建立**真实在途任务**，并设 `OMNI_TEST_SHUTDOWN_DELAY_MS=8000`；
2. 后台执行**生产回调实际使用的入口** `on_window_destroyed_shutdown()`；
3. 等假引擎收到 `omni.shutdown`（关闭等待开始）后断言：
   - **宿主锁 <1.5 s 可取得**（验收标准 1：旧实现会持锁到关闭结束）；
   - 取消/状态/结果 **<1.5 s** 返回 `engine_shutting_down`，消息含“未发送”且 `sent_to_engine=false`（验收标准 2）；
   - `omni.shutdown` 之后引擎**没有**收到 `omni.cancel_task`（未把请求送进正在关闭的引擎）；
   - 收尾完成后 `health()` 不再报告引擎在运行。

## D. 反向验证（变异，已还原）

把 `on_window_destroyed_shutdown` 改回第四轮前的写法（`state.lock()` + `bridge.shutdown()`）：

| 检出层 | 实测 |
| --- | --- |
| 行为测试 | **失败**：`关闭等待期间宿主锁应可立刻取得（等了 8.041135831s）` |
| 接线测试 | **失败**：生产前缀出现 `bridge.shutdown(` |

两条独立证据同时命中复核所指的缺陷。

## E. 本轮命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline`（测试二进制 `…afcdf36f070ad598` mtime **07:35:34** > 最后一次源码改动 `lib.rs` **07:35:29**） | **116 lib + 2 admission + 13 host + 3 e2e = 134 通过 / 0 失败**，无警告 |
| `npx --no-install tsc --noEmit` | 退出码 0 |
| `npm run build` | 成功 |
| `dump-model-route.mjs --self-test` | 21/21 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass |
| `verify_engine_cancel.py` | state 34/34 + static 8/8，pass |
| `verify_ai_draft.py` | 22 cases / 0 failed |
| `verify_session_continuity.py` | 37 checks / 0 failed |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 |
| HEAD | `5ffc710`（未提交、未推送、未打标签） |

原始输出：`cargo-test-offline-round4.log`、`dump-model-route-self-test.json`。

## F. 边界

1. 窗口销毁回调仍在 Tauri 事件循环线程上**同步**等待关闭（与修正前一致）；本轮保证的是**不占用宿主锁**，
   而非改成异步关闭。
2. 单元测试覆盖的是生产回调实际调用的同一入口函数 `on_window_destroyed_shutdown()`；
   未在真实窗口中实际销毁窗口验证，真实模型与跨平台仍未验证。
