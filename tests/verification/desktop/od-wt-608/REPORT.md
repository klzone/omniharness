# OD-WT-608 审阅修改 IPC：执行证据

## 基线

- 任务开始时 HEAD：`ab000dfb01`（工作区存在 601-607 的未提交改动）。
- 执行期间工作区外部出现提交 `821b1a4cf3`（wip(writing)，含 601-607 与 608 后端基线），不是本会话执行；本会话未 commit、未 push。
- 最终 HEAD：`821b1a4cf3`；本任务的测试、前端封装等改动仍未提交。

## 交付

- `desktop/src-tauri/src/writing_review.rs`：十维白名单与 `ReviewFinding`；审阅提示词；应用问题复用 607 `confirm_section`；版本列表（note + 段落差异摘要）、版本对比、显式确认恢复；12 项模块测试（含场景八）。
- `desktop/src-tauri/src/lib.rs`：`writing_review` / `writing_review_apply_finding` / `writing_version_list` / `writing_version_diff` / `writing_version_restore` 五个命令、闸门覆盖、命令注册、3 项 IPC 闸门测试。
- `desktop/src/types.ts` / `desktop/src/api.ts`：审阅与版本类型、五个调用封装。

## 实跑命令

```bash
export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib
# test result: ok. 1044 passed; 0 failed; 0 ignored
# PLANS.md 记录的 607 收口为 1004/0；本任务新增 15 项。其余增量包含并行 609 的事实核验测试（fact_check.rs 与 lib.rs 接线由 609 会话写入，本会话未改；其测试仍在增加，最终全量数字以 609 收口为准）。

cd desktop && npx tsc --noEmit
# exit 0（无输出）
```

原始输出：`cargo-test-lib.txt`、`tsc.txt`。

## 场景八

`scenario_eight_versions_queryable_restorable_and_history_kept` 覆盖：连续两次应用审阅问题产生 v4/v5；v2→v4 段落差异可查；恢复 v2 生成 v6；v1-v6 全部仍在列表中且 note 非空；当前正文回到 v2 结构，v4 历史内容未被改写，任务状态保持「待核验」并追加「恢复 v2」。

## 裁决记录

- 应用问题直接复用 607 `confirm_section`（任务卡允许的“直接裁决复用其确认函数”），未改 607 结构；留痕经 601 `update_writing_task` 追加，记录 action 为「编辑」、reason 为「应用审阅问题 …」，状态不迁移。
- 恢复当前版本被拒绝（无意义重复快照），任务卡未明说，属范围保护。
- 版本段落差异只在同文稿类型（大纲/正文）之间计算；每类首版无前序同类型版本时 `diff=null`。
- 大纲类与正文类版本对比明确拒绝，不做无意义段落比较。
- `writing_review_ipc_tests` 放在 `lib.rs` 文件末尾的既有测试区，而不是命令区中间：`engine_lifecycle_wiring_tests` 以第一个 `#[cfg(test)]` 作为生产代码边界，命令区中间插入测试模块会被它截断（本会话实测出现 2 项既有测试失败，回移后 0 failed）。

## 未做与已知限制

- 未接真实模型审阅输出（模型调用在界面侧；本任务只交付宿主提示词与问题校验/应用路径）。
- 未实现 612 的版本面板与核验面板界面。
- 版本列表会读取全部快照以计算差异摘要；长历史下开销随版本数增长。
- 未做真实窗口目视核对（本任务无界面改动）。
- 并行 609 的 `fact_check.rs` 与 `lib.rs` 事实核验接线不在本任务改动范围内，本会话未修改；全量测试计数包含其测试。

## 豁免声明

未新增 lint/测试豁免，未删除或跳过既有测试，未改低验收阈值；未改 `runtime/`，未新增模型通道，测试数据全部虚构。
