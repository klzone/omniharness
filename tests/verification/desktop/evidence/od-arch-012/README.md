# OD-ARCH-012 证据索引：成果写入失败与刷盘失败清理

- 采集时间：2026-10-18（Asia/Shanghai）
- 采集者：BUILDER
- 对应计划：[PLANS.md](../../../../../PLANS.md) 的 **OD-ARCH-012**
- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`
- 结论范围：**只补足 OD-ARCH-011 保存路径的写入/刷盘失败清理证据**。模型准入保持关闭，未调用模型。

---

## 1. 实现与注入边界

`desktop/src-tauri/src/task_shell.rs` 新增 crate 内私有 `NewFileOps`，包含 `write`、`sync`、`remove` 三个函数指针：

- 生产入口 `write_new_file_durable(target, content)` 签名不变，固定转发到 `NewFileOps::REAL`（真实 `write_all`、`sync_all`、`remove_file`）。
- 故障注入只通过测试内构造的 `NewFileOps` 值传给 `write_new_file_durable_with`。
- 该结构体不是 `pub`，不实现 `Default`，不从环境变量、UI、普通配置或请求参数读取；宿主命令与前端无法替换它。
- 保留 `create_new(true)`：拒绝覆盖既有文件；失败清理只删除本次新建的目标文件。

源码事实核对：`NewFileOps` 位于 `task_shell.rs:1678`，生产常量 `REAL` 位于 `1686`，生产入口位于 `1697`。

---

## 2. 三类失败场景与判定

| 场景 | 注入 | 断言与结果 | 证据等级 |
| --- | --- | --- | --- |
| 写入失败 | 先写入一半字节，再返回 `WriteZero` 错误 | 返回「写入产物失败…已删除本次未完成的半成品」；半成品不存在；同目录既有文件内容不变；错误文本不含正文 | Rust 运行（单元测试） |
| 刷盘失败 | 内容已写入，`sync_all` 返回错误 | 返回清理成功后缀；文件不存在；目录为空；错误文本不含正文 | Rust 运行（单元测试） |
| 清理失败 | `sync` 与 `remove` 均注入失败 | 返回「清理半成品失败」；残留文件确实存在；不把失败报告成原子成功 | Rust 运行（单元测试） |
| 生产入口不可注入 | 编译期签名断言 + 真实写入 | `write_new_file_durable` 只有 `(target, content)`；正常写盘成功 | Rust 运行（单元测试） |

测试名称见 `od012_targeted_final.log`：

- `write_failure_removes_only_the_partial_file`
- `sync_failure_removes_the_written_file`
- `cleanup_failure_is_reported_honestly_not_as_success`
- `failing_write_ops_cannot_be_reached_from_production_entry_points`

**限制：** 清理失败时文件会残留，这是文件系统语义；代码只保证如实报告，不承诺跨文件系统或断电级原子性。

---

## 3. 测试有效性变异检查

为证明新增测试不是空跑，做了两次受控变异，并在每次结束后立即还原源码（SHA-256 均为 `54c2ed22…b455`）：

| 变异 | 预期破绽 | 实际结果 | 日志 |
| --- | --- | --- | --- |
| `remove` 改为永远返回成功、不真正删除 | 写入失败与刷盘失败测试应失败 | `43 passed / 2 failed`；恰好是这两个用例失败 | `od012_mutation.log` |
| 清理结果被忽略、仍报告「已删除」 | 清理失败测试应失败 | `44 passed / 1 failed`；`cleanup_failure_is_reported_honestly_not_as_success` 失败 | `od012_mutation2.log` |

还原后重跑定向测试：`45 passed / 0 failed`（`od012_targeted_final.log`）。变异只用于验证测试有效性，未留在源码中。

---

## 4. 计划要求的完整回归

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --offline`（`desktop/src-tauri`） | lib **72 passed / 0 failed**；`host_integration` **13 passed / 0 failed**；`task_shell_e2e` **3 passed / 0 failed**；doc tests 0 | `od012_full_rust.log` |
| `cargo test --offline task_shell` | **45 passed / 0 failed** | `od012_targeted_final.log` |
| `npm run build`（`desktop`） | 通过（tsc + vite，23 模块） | `od012_frontend.log` |
| `cargo run --offline --example evaluate_offline_scenarios` | 通过；三场景与准入拒绝输出符合预期 | `od012_example.log` |
| `git diff --check HEAD` | 无输出，退出码 0 | 见 STATUS.md |
| `git diff -- runtime/` | 0 行；`runtime/` 无未跟踪改动 | 见 STATUS.md |

所有 Cargo/npm 命令均先执行 `eval "$(scripts/setup-desktop-toolchain.sh --print-env)"`。

---

## 5. 边界核对

- 未修改 `runtime/`、`ENDPOINT_ISOLATION_CONFIRMED`、模型路由、端点、凭证或用户数据。
- 未调用模型；离线示例与单元测试均只使用合成材料。
- 未读取旧会话或旧成果正文；未启动用户正在使用的 DSH 实例。
- `~/.dsh` mtime 保持 `2026-10-15 20:17:43`，`~/.agents` 保持 `2026-09-23 15:12:11`（执行前后一致，未读取内容）。
- 未提交、未推送、未打标签；`REVIEW.md` 未改动。

---

## 6. 未验证与限制

- **模型写作仍未验证：** 端点准入关闭，本计划不授权模型调用。
- 用途准入保持原状：周报、一般写作、项目跟进仍是 `reserved`，不构成正式能力授权。
- 只验证 Linux 本机文件系统上的失败注入；未验证磁盘满、权限突变、断电等真实故障。
- 未做跨平台验证。
- `rustfmt` 组件在当前工具链未安装，未运行格式化检查；`cargo test` 编译通过。源码沿用仓库现有排版风格。
