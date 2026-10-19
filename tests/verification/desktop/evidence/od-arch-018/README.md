# OD-ARCH-018 证据索引

日期：2026-10-18　执行：BUILDER　状态：**已执行，等待 ARCHITECT 独立复核**

本目录只放可复核的结果与截图，不含真实工作材料、真实会话正文、凭证或私人路径清单。全局搜索框、模型端点仍不可用，入口保持关闭。

## 执行环境

- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，开工 HEAD `5ffc710`，开工时 81 项未提交/未跟踪改动全部保留，未提交、未推送、未打标签。
- 隔离运行根：`/tmp/od018-evidence/run2/`（一次性合成根，含独立 `dsh-home`、`agents`、`app-data`、`synthetic-provider.yml`），成果落盘到该根下 `local-data/`，位于仓库之外。
- 启动方式：`DISPLAY=:97 OMNIDESK_DEV_ROOT=/tmp/od018-evidence/run2 bash scripts/dev-ui.sh --no-build`，Xvfb 虚拟屏 1440×1000，窗口 1280×820。
- 未读取、未列出、未修改 `~/.dsh`、`~/.agents`、`~/OmniDesk` 或任何真实会话；未调用任何真实模型端点。

## 本轮修复的缺陷

**成果列表永远为空（真实缺陷）。** `desktop/src-tauri/src/task_shell.rs` 原先用 `rsplit_once('-')` 拆解文件名，把 `offline-weekly-report-20261018-060901.md` 的「场景名」错误地解析成 `report`，导致已保存的成果在“已保存的成果”列表中永远不出现。

修复：新增 `split_offline_artifact_name()`，按已登记的三个场景前缀（`weekly-report` / `material-writing` / `project-management`）匹配，列表与读取共用同一解析函数，不再依赖分隔符位置。用真实窗口验证：修复前列表为空，修复后三份成果全部列出且「重新打开」成功。

同时修正 `desktop/src/offline-flows.css` 的成果列表布局：`.record-list` / `.record-item` 改为两行网格，长文件名可换行、按钮固定右侧，不再相互挤压。

## 真实桌面窗口验收（证据等级：真实窗口运行，鼠标键盘由 XTEST 注入）

| 场景 | 输入 | 可观察结果 | 截图 |
| --- | --- | --- | --- |
| 周记录 → 周报 | 合成逐日记录（含完成/待办/受阻） | 本地确定性汇总；界面显示「模型步骤：受准入阻塞、未验证：本次未调用任何模型」；勾选人工确认后保存成功，列表出现，重开为只读原文 | `01`–`06` |
| 一般文案结构 | 合成「向合成评审小组说明本周合成样例整理进度」 | 输出含「模板：未使用模板（没有与用途匹配的模板，按用途组织）」与「待确认」小节；保存成功、重开一致 | `07`–`12` |
| 项目跟进 | 合成 4 项事项（P1/P2/P3 代号，无真实姓名） | 事项总数 4；状态 done 1 / in_progress 1 / blocked 1 / not_started 1；逾期 2；表格含负责人代号、期限、状态、下一步；列出 1 条受阻、2 条逾期需人工确认；逾期基于给定判定日 2026-10-18；保存后列表共 3 项、重开内容与统计一致 | `13`–`21` |
| 会话与任务页隐私显示 | 准入关闭状态 | 只显示「端点未确认，当前无法读取会话清单和任务台账」，未显示绝对路径、未显示自由文本 intent、未显示会话正文；页面说明不自动重放中断任务、旧会话导入不提供 | `22`, `23` |

三种场景的界面均明确标注「离线工作流程（合成数据）· 不调用模型」，并在结果区单独说明模型步骤受准入阻塞。离线流程不是 AI 写作，未验证生成质量。

**关于路径显示：** 离线流程页会显示本次成果的保存目录与保存回执路径（截图中为一次性临时根 `/tmp/od018-evidence/run2/...`）。这是用户自己要确认的落盘位置，属于有意显示；会话页与任务页不显示任何绝对路径或工作区路径。

窗口内三条合成成果（均在仓库外）：

- `local-data/offline-weekly-report-20261018-060901.md`（986 字节）
- `local-data/offline-material-writing-20261018-061420.md`（1038 字节）
- `local-data/offline-project-management-20261018-062645.md`（1059 字节）

## 脚本与静态检查

| 检查 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline task_shell::tests::offline_artifact` | 4 通过 / 0 失败（列表与重开往返、过滤无关文件、拒绝路径穿越与超大文件、符号链接不可读不可列） | Rust 编译 + 单元测试 |
| `cargo test --offline`（完整） | 92 通过（76 lib + 13 host_integration + 3 e2e）/ 0 失败 | Rust 编译 + 单元/集成测试 |
| `npx tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 退出码 0 | 构建实测 |
| `cargo run --offline --example evaluate_offline_scenarios` | 三场景通过；准入拒绝生效；日期边界（闰年、非法日期）正确 | Rust 运行实测 |
| `python3 tests/verification/desktop/verify_session_continuity.py` | 37/37 通过 | 一次性隔离根 + 本机回环合成 ACP 实测 |
| `python3 tests/verification/desktop/verify_engine_cancel.py` | 状态/IPC 34/34，静态 8/8 | 编译真实源码后 Node 执行 + 静态检查 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0，无差异 | 文件事实 |
| 两侧 `ENDPOINT_ISOLATION_CONFIRMED` | `desktop/src/engineAdmission.ts` 与 `desktop/src-tauri/src/admission.rs` 均为 `false` | 静态代码检查 |

原始日志（本目录）：`rust-artifact-tests.log`、`rust-cargo-test.log`、`frontend-build.log`、`offline-scenarios.log`。

## 复现命令

```bash
cd /mnt/data/Code/MyWorkStation
cd desktop && npx tsc --noEmit && npm run build
cd src-tauri && cargo test --offline
cargo run --offline --example evaluate_offline_scenarios
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/desktop/verify_session_continuity.py
python3 tests/verification/desktop/verify_engine_cancel.py

# 隔离桌面窗口（一次性合成根；结束后 --stop 再关闭 Xvfb）
Xvfb :97 -screen 0 1440x1000x24 &
DISPLAY=:97 OMNIDESK_DEV_ROOT=/tmp/od018-evidence/run2 bash scripts/dev-ui.sh --no-build
DISPLAY=:97 OMNIDESK_DEV_ROOT=/tmp/od018-evidence/run2 bash scripts/dev-ui.sh --stop
```

## 证据清单

| 文件 | 内容 | 证据等级 |
| --- | --- | --- |
| `shots/00-home.png` … `shots/23-task-records-page.png` | 24 张窗口截图，按场景与时间顺序命名 | 真实窗口（Xvfb + XTEST 注入输入） |
| `rust-artifact-tests.log` | 4 项成果列表/读取定向测试 | Rust 编译 + 单元测试 |
| `rust-cargo-test.log` | 完整 `cargo test --offline` 输出 | Rust 编译 + 单元/集成测试 |
| `frontend-build.log` | `npm run build` 输出 | 构建实测 |
| `offline-scenarios.log` | 三场景离线评估示例输出 | Rust 运行实测 |

## 未验证事项

- **真实模型生成与写作质量：** 端点准入关闭，三个场景的模型步骤均未执行，界面显示「受准入阻塞、未验证」。离线流程的产出质量不能代表模型写作能力。
- **取消按钮的真实窗口交互：** 准入关闭时任务无法进入可取消状态，未做窗口内实际取消；取消语义由前端替身测试（34/34）覆盖，Windows/真实模型链路下的取消仍为未验证。
- **跨平台：** 仅在 Linux + Xvfb 无硬件加速的虚拟屏上验证，未在其他显示环境或操作系统验证。
- **人工点击：** 本轮窗口交互由 XTEST 注入坐标，等效于真实输入事件，但未由人工手点确认观感与可读性。
- **会话列表真实数据：** 准入关闭，会话页只验证了拒绝显示，未验证真实会话元数据在界面中的隐私表现。

## 与 Plan 的偏差

未发现需要 ARCHITECT 决策的偏差。补充修复了成果列表解析缺陷（属于计划内的普通实现缺陷），未扩大功能、未新增依赖、未修改协议。
