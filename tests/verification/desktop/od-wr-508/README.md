# OD-WR-508 收口证据

日期：2026-11-11
基线：`eec28fd635`（OD-WR-500 规划）；501～507 的未提交改动全部保留，未 reset、未 clean、未 commit、未 push。

`desktop/src-tauri/tests/profile_assets_host.rs` 的三处 8→10 断言由 ARCHITECT 亲自修改并复跑（21/0）；本卡未触碰该文件。

## 全量回归

| 命令 | 结果 | 退出码 | 日志 |
| --- | --- | --- | --- |
| `RUST_TEST_THREADS=4 cargo test --offline --lib` | 1154 passed / 3 failed（均为 `process_owner` 沙箱进程表误报） | 101 | `cargo-lib.log` / `cargo-lib.exit` |
| `RUST_TEST_THREADS=4 cargo test --offline --tests --no-fail-fast` | lib 同上；集成测试 67 passed / 0 failed（含 `profile_assets_host` 21/0） | 101 | `cargo-tests.log` / `cargo-tests.exit` |
| `/tmp/omnidesk-pytest-venv/bin/pytest tests -q` | 230 passed / 4 failed（均为 `process_owner` 沙箱进程表误报） | 1 | `pytest.log` / `pytest.exit` |
| 同上，忽略 `test_process_owner_unit.py` | 223 passed / 0 failed | 0 | `pytest-no-process-owner.log` / `.exit` |
| `cd desktop && npx tsc --noEmit` | 0 failed | 0 | `tsc.log` / `tsc.exit` |
| `cd desktop && npm --prefix . run build` | 构建成功 | 0 | `build.log` / `build.exit` |

`cargo test --offline --tests` 不加 `--no-fail-fast` 时会在 lib 的 3 项沙箱误报后停止；上表使用 `--no-fail-fast` 让全部 target 跑完。集成测试逐项：main 0、admission 2、capabilities 1、host_integration 13、knowledge_ingest 1、knowledge_vault_host 6、knowledge_vault_index 5、knowledge_vault_write 10、profile_assets 21、profile_categories 4、task_shell 3、thread_store 1。

### 已知沙箱误报

cargo（3 项）：

- `process_owner::tests::live_instance_owned_by_another_host_is_refused`
- `process_owner::tests::pid_reuse_with_unrelated_cmdline_is_never_killed`
- `process_owner::tests::record_from_other_boot_is_removed_without_killing`

pytest（4 项，同一类进程表限制）：

- `test_claim_writes_private_record_and_release_removes_it`
- `test_orphan_child_group_is_reclaimed_and_bystander_survives`
- `test_record_child_group_persists_pid_and_pgid`
- `test_second_instance_on_same_root_is_refused`

## 既有 verify 脚本

| 脚本 | 结果 | 退出码 | 证据 |
| --- | --- | --- | --- |
| `verify_wr_503.py` | 58/58 | 0 | `verify_wr_503.log` / `.json` |
| `verify_wr_504.py` | 41/41 | 0 | `verify_wr_504.log` / `.json` |
| `verify_wr_505.py` | 71/71 | 0 | `verify_wr_505.log` / `.json` |
| `verify_wr_506.py` | 58/58 | 0 | `verify_wr_506.log` / `.json` |
| `verify_wr_507.py` | 33/33 | 0 | `verify_wr_507.log` / `.json` |
| `verify_wr_detail_502.py` | 17/17 | 0 | `verify_wr_detail_502.log` / `.json` |
| `verify_work_record_ui_301.py` | 15/15 | 0 | `verify_work_record_ui_301.log` / `.json` |
| `verify_wr_security_308.py` | 13/13 | 0 | `verify_wr_security_308.log` / `.json` |
| `verify_worklog_089.py` | 83/83 | 0 | `verify_worklog_089.log` / `.json` |
| `verify_weekly_091.py` | 67/67 | 0 | `verify_weekly_091.log` / `.json` |
| `verify_weekly_092.py` | 28/29（`metric_uses_entry_count` 陈旧断言） | 1 | `verify_weekly_092.log` / `.json` |
| `verify_weekly_093.py` | 无法运行：缺少 `WeeklyQuadrants.tsx` | 1 | `verify_weekly_093.log` |
| `verify_weekly_097.py` | 无法运行：缺少 `WeeklyQuadrants.tsx` | 1 | `verify_weekly_097.log` |
| `verify_weekly_099.py` | 无法运行：缺少 `WeeklyQuadrants.tsx` | 1 | `verify_weekly_099.log` |
| `verify_weekly_100.py` | 32/32 | 0 | `verify_weekly_100.log` / `.json` |
| `verify_week_from_records_306.py` | 12/12 | 0 | `verify_week_from_records_306.log` / `.json` |
| `verify_work_cleanup_307.py` | 15/15 | 0 | `verify_work_cleanup_307.log` / `.json` |
| `verify_worklog_unify_305.py` | 15/15 | 0 | `verify_worklog_unify_305.log` / `.json` |
| `verify_wr_security_508.py` | 21/21 | 0 | `verify_wr_security_508.log` / `.json` |

`verify_weekly_092/093/097/099` 的失败是既有脚本与当前代码的陈旧引用：`WeeklyQuadrants.tsx` 已在早期提交删除，092 的指标断言仍按旧 `ScenePage` 结构；均非 501～507 引入。

## 安全核对（508）

`verify_wr_security_508.py` 21/21 通过，覆盖五组：

- 原始素材只读：`RecordPatch` 无 `raw_input` / `raw_source`，`update_record` 不写 raw，创建路径（quicklog）写原始输入，摘要/复盘无 Patch 通道，501 单测存在。
- AI 无确认态写通道：`HomeChat` 不直接 `invoke`，`conversationPrompt` 无传输/文件写入，正式写入只经 `api.ts` 的宿主封装，对话链路无复盘确认调用，摘要/复盘生成 IPC 均进入宿主校验。
- 已确认复盘不覆盖：`confirm_review` 存在，`write_review` 拒绝已确认，505 单测冻结确认版字节，档案页禁用重新生成。
- 引用真实性：摘要/复盘的 `validate_body` 与 `citation_ids` 存在，生成 IPC 注册且校验在落盘前，未知引用单测存在。
- 测试全虚构：本轮新增测试文件不含真实材料特征词，摘要/复盘单测使用甲、乙等合成对象。

## 已知限制

- 复盘确认只有宿主 IPC 与 `api.ts` 封装，档案页没有确认按钮；界面暂时无法把草稿标为已确认。
- 未做真实模型质量验收；摘要/复盘的语言质量与引用选择需真实模型实测。
- 时间线视觉与跳转体验未做真实窗口目视验收。
- 摘要/复盘引用失效只标记，不自动修复。
- 旧记录没有 `raw_input` 时无法使用「重新整理」。
- `process_owner` 的 cargo 3 项、pytest 4 项为沙箱误报。

## 豁免声明

- 未提交、未推送、未打标签。
- 未修改实现代码、既有测试断言、`runtime/`、`STATUS.md` / `REVIEW.md` / `PLANS.md` / `decisions.md`。
- 508 只新增 `verify_wr_security_508.py`、本目录证据与 `docs/plans/OD-WR-500/HANDOFF.md`。
