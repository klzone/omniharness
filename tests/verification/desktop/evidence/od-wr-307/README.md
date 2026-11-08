# OD-WR-307 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 起点库单测：632 passed
- 交付后库单测：643 passed / 0 failed（本任务新增 11 项）
- 起点 pytest：184 passed；交付后 186 passed，30 subtests passed（新增 2 项）

## 改动文件

- engine/omnidesk_engine/task_extract.py：提示词注入未完成任务摘要，输出规范加入 match_task_id / match_confidence / match_reason / suggested_state。
- desktop/src-tauri/src/task_extract.rs：解析时加载未完成任务，校验关联目标、关联置信度、状态建议合法性。
- desktop/src-tauri/src/task_store.rs：候选新增关联字段；任务新增 linked_record_ids；link_candidate 追加进展并双向回写；dismiss_match 保留候选。
- desktop/src-tauri/src/lib.rs：task_candidate_link / task_candidate_dismiss_match IPC；解析准备注入任务摘要。
- desktop/src/api.ts、desktop/src/types.ts：关联采纳/拒绝方法与 LinkReport、候选关联字段、任务 linked_record_ids。
- desktop/src/shell/TaskReviewPanel.tsx：关联建议区、低置信提示、状态建议只读、采纳关联/作为新任务采纳/拒绝关联/拒绝候选。
- 新增 tests/verification/desktop/verify_task_linkage_304.py：16 项静态检查。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 643 passed / 0 failed |
| pytest tests -q | 0 | 186 passed，30 subtests passed |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功（1.18s） |
| python3 tests/verification/desktop/verify_task_linkage_304.py | 0 | 16/16 通过 |

原始输出：cargo-test-lib.log、pytest.log、tsc.log、npm-build.log、verify-task-linkage.log、exit-*.txt。

> 后续：OD-WR-316 把 LinkReport 改为结构化半完成回执；verify_task_linkage_304.py 同步扩到 18 项，见 od-wr-316 证据。

## 联动语义

- 采纳关联：宿主先追加进展（只追加，不覆盖既有进展与状态历史），再把记录 ID 写进任务的 linked_record_ids、把任务 ID 写进记录的 linked_task_ids，候选归档为 linked。
- 拒绝关联：只清空候选的关联字段，候选仍留在待审核，之后可以作为新任务采纳。
- 状态建议：只校验是否为当前状态允许的下一步，界面标注「不会自动应用」，不调用状态迁移。
- 低置信度关联：界面标红提示人工判断；所有候选本来就必须人工裁决。
- 三日记事归一任务：单测连续三天记录关联同一任务，任务进展保留三条、任务与三条记录双向 ID 齐全。

## 已知限制

- 文件写入不是事务：任务进展写成功后若记录回写失败，会出现进展已追加但记录未回链的中间态；本轮不做跨文件回滚。
- 真实模型匹配质量未验证；只验证结构校验与合成数据。
- 关联建议只注入最多 30 条未完成任务摘要，任务很多时可能漏掉合适匹配。
- 真实窗口未目视核对。
