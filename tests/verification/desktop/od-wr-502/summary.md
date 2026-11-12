# OD-WR-502 验证记录

- 基线：HEAD `eec28fd6357b4a8490613fa4694115e6e04c42d4`。开工时工作区已有 OD-WR-501 的未提交改动与 `od-wr-501/` 证据，未重置、未覆盖。
- 验收命令 1：`export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib`
  - 结果：退出码 0；`test result: ok. 1123 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`。
  - 原始输出：`cargo-test-lib.txt`。新增修订读取测试 3 项：`missing_revision_history_reads_as_empty_for_legacy_record`、`revision_summaries_keep_order_and_omit_body_hash`、`corrupt_revision_line_fails_instead_of_hiding_history`。
- 验收命令 2：`cd desktop && npx tsc --noEmit`
  - 结果：退出码 0，无输出；原始输出文件 `tsc.txt`（空文件即无诊断）。
- 验收命令 3：`cd desktop && npm --prefix . run build`
  - 结果：退出码 0；`✓ built in 1.76s`；输出含 chunk > 500 kB 警告。原始输出：`build.txt`。
- 验收命令 4：`python3 tests/verification/desktop/verify_wr_detail_502.py`
  - 结果：退出码 0；17/17 通过。原始输出：`verify.txt`；结构化结果：`verify.json`。
- 回归静态检查：`python3 tests/verification/desktop/verify_work_record_ui_301.py` 15/15 通过。
- 证据等级：Rust 与 Python 结果为脚本运行；前端类型检查与构建为脚本运行；界面运行时行为未在本会话开真实窗口目视。

## UI 参考

用 `screenshots-design-inspiration` 查询并查看了 Notion（Web，Note Detail / Updates 入口）、Linear（Web，Issue Detail / activity 侧栏）与 Airtable（Web，record detail / 折叠面板）的截图。借鉴 Notion 把历史作为低权重入口放在详情内、Linear 用紧凑时间行承载活动、Airtable 用折叠区块收纳补充信息；本卡按现有详情栏与 theme token 落地，不引入新布局层。
