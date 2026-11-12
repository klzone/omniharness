# OD-WR-505 周期复盘引擎 + Skill 证据

## 基线

- 日期：2026-11-11
- HEAD：`eec28fd635`
- 基线工作区已有 OD-WR-501～504 的未提交与未跟踪改动；本卡沿用原工作区，未执行 reset、clean、commit 或 push。

## 改动文件

- `desktop/src/workIntent.ts`：新增 `period-review` 封闭词表、`weekly/monthly/quarterly/annual` 周期与起止日期推导；周报词表仍先于复盘判定。
- `skills/period-review/SKILL.md`：按周/月/季/年给出 7/9/11/13 段深度模板；每段要求真实 `#wr-` 或 `#t-` 编号。保留 `skills/memory-report/` 原样。
- `desktop/src-tauri/src/review_store.rs`：周期汇集、任务状态统计、既往复盘标题、草稿落盘、引用校验、确认流、草稿 `.bak` 备份、已确认拒绝覆盖；16 项单测。
- `desktop/src-tauri/src/lib.rs`：模块注册与四个 IPC（collect/generate/read/confirm）。
- `desktop/src-tauri/src/profile_assets.rs`、`profile_rules.rs`、`profile_seed.rs`：装配内置 `period-review` Skill。
- `samples/profiles/discipline.rules.json`：新增 `weekly-period-review` 绑定；`weekly-report` 仍绑定 `memory-report`。
- `desktop/src/conversationPrompt.ts`、`HomeCanvas.tsx`、`HomeChat.tsx`、`api.ts`、`types.ts`：复盘材料、深度标题契约、终态校验落盘与草稿重生成确认。
- `tests/verification/desktop/verify_wr_505.py`：词表运行探针与宿主/前端静态契约检查。

## 实跑命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `RUST_TEST_THREADS=4 cargo test --offline --lib` | 1148 passed / 3 failed（均为 `process_owner` 沙箱进程表误报） | `cargo-lib.log` / `cargo-lib.exit` |
| `npx tsc --noEmit` | exit 0 | `tsc.log` / `tsc.exit` |
| `npm run build` | exit 0 | `build.log` / `build.exit` |
| `python3 tests/verification/desktop/verify_wr_504.py` | 41/41 | `verify_wr_504.log` / `.json` |
| `python3 tests/verification/desktop/verify_wr_503.py` | 58/58 | `verify_wr_503.log` / `.json` |
| `python3 tests/verification/desktop/verify_work_record_ui_301.py` | 15/15 | `verify_work_record_ui_301.log` / `.json` |
| `python3 tests/verification/desktop/verify_wr_505.py` | 71/71 | `verify_wr_505.log` / `.json` |

`cargo test` 的 3 项失败是 `process_owner` 在受限沙箱里读不到进程表：`live_instance_owned_by_another_host_is_refused`、`pid_reuse_with_unrelated_cmdline_is_never_killed`、`record_from_other_boot_is_removed_without_killing`。ARCHITECT 已在沙箱外复跑并通过；本卡按要求不修改该模块。

## UI 参考借鉴

参考 Baremetrics 的周期选择与统计卡置顶、Sprout Social 的报告流；本卡只在复盘入口沿用「周期与统计在前、正文在后、生成流程单列」，未新增视觉体系。

## 裁决与已知限制

- 新建 `skills/period-review/` 而非改写 `memory-report`：保留周报旧入口行为，周报词表命中仍走 `report`。
- 已确认复盘拒绝重新生成，不像草稿那样另存；确认版字节保持不变。
- 复盘建议不自动转任务，任务只统计；档案页与确认按钮留给 OD-WR-506。
- 未运行 pytest：本卡没有新增 Python 逻辑或 Python 测试。

## 豁免声明

- 未提交、未推送、未打标签。
- 未修改 `runtime/`、知识库写路径、任务创建通道、`STATUS.md` / `REVIEW.md` / `PLANS.md` / `decisions.md`。
- 未新增豁免；既有 OD-WR-501～504 改动与运行证据全部保留。
