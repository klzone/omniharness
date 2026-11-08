# OD-WR-316 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围：link_candidate 半完成回执，不引入跨文件事务。

## 改动文件

- desktop/src-tauri/src/task_store.rs：LinkReport 改为 ok / partial / 各项写入标志 / warnings；link_candidate 在记录回链或候选归档失败时返回结构化半完成回执。
- desktop/src/shell/TaskReviewPanel.tsx：显示半完成原因与补救提示。
- desktop/src/types.ts：LinkReport 类型同步。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --tests | 0 | lib 653 passed / 0 failed（含 2 项半完成用例） |
| npx tsc --noEmit | 0 | 无输出 |
| python3 tests/verification/desktop/verify_task_linkage_304.py | 0 | 18/18 通过（含半完成检查） |

原始输出与 309 同轮，见 od-wr-309/cargo-test-tests.log 与 od-wr-309/exit-*.txt。

## 行为

- 成功：ok=true、partial=false，任务/记录/候选三项都回传。
- 记录回链失败：ok=false、partial=true，progress_added 与 task_linked 为真、record_linked 为假；警告提示手动补救。
- 候选归档失败：任务与记录都已回链，但 candidate_archived 为假；警告明确「不要重复采纳，请手动清理」。
- 不做跨文件事务；没有自动回滚已完成的写入。
